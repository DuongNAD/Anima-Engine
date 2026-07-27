// The two sources of frame-to-frame variation in the landscape scene, and how capture mode removes
// them.
//
// # What was wrong with "the clock is pinned"
//
// `WorldShowcase` pauses the day/night clock in capture mode (`speed = 0`), and that was described
// as pinning the clock. It pins one clock. The scene has two more:
//
//   * `state.clock.getElapsedTime()` — r3f's own render clock, which starts when the Canvas mounts
//     and never stops. `WorldWater` drives its swell/foam `uTime` from it; the terrain shader, the
//     waterfall curtains, vegetation sway, birds, fish and wildlife all animate from it too. Two
//     captures taken at different points in page startup are different pictures.
//   * `Math.random()` — `WorldSky` places its stars with it and `WorldWeather` seeds precipitation
//     with it, both in a `useMemo`, so the positions are stable within a page and different on the
//     next load.
//
// Neither is visible in a single screenshot, which is exactly why "the images look right" was not
// evidence that they were reproducible. The gate is `canonical_views.spec.ts` shooting each view
// twice from clean page loads and comparing SHA-256; this module is what makes that pass.
//
// # Why the frozen time is not zero
//
// Zero is the one value where every sine-driven animation is at its rest position simultaneously:
// flat water, unswayed grass, wildlife mid-stride at phase 0. That is a picture of the scene's
// initial conditions rather than of the scene, and a reviewer looking for "does the water read as
// water" would be shown the one frame where it does not. A fixed non-zero time is just as
// reproducible and shows the animated state.
//
// # Why this reads the URL itself
//
// The alternative is for `WorldShowcase` to call a setter while rendering, and have every child
// component depend on having been rendered after it. That is an ordering contract between a parent's
// render body and its children's `useMemo`s — true today, silently broken by a future
// `<Suspense>` boundary. Deriving the same answer from the same URL in both places has no ordering.

import { isCaptureRequested } from './captureMode';

/**
 * Scene animation time, in seconds, used for every capture.
 *
 * 12.5 s is far enough in that nothing is at its initial-condition rest pose and short enough to
 * stay inside the float precision where a shader's `sin(uTime * k)` is still smooth.
 */
export const CAPTURE_FIXED_ELAPSED = 12.5;

/** Minimal shape of the r3f render clock this module reads. */
export interface ElapsedClock {
  getElapsedTime(): number;
}

let frozen: boolean | null = null;

/**
 * True when this page is a canonical-view capture and the scene must hold still.
 *
 * Memoised: it is read once per animated component per frame, and the answer cannot change within a
 * page (the capture request is fixed at load — see `captureMode.ts`).
 */
export function isSceneTimeFrozen(): boolean {
  if (frozen === null) {
    frozen = typeof window !== 'undefined' && isCaptureRequested(window.location.search);
  }
  return frozen;
}

/**
 * Forget the memoised capture decision.
 *
 * For tests, which need to exercise both branches in one process. Production code has no reason to
 * call it: the capture request cannot change within a page load.
 */
export function resetSceneClockCache(): void {
  frozen = null;
}

/**
 * Scene animation time for this frame: the real render clock, or the frozen capture time.
 *
 * Every `state.clock.getElapsedTime()` in an animated `World*` component goes through here. A
 * component that reads the clock directly still animates correctly and quietly makes the captures
 * irreproducible, so `sceneClock.test.ts` asserts that none do.
 */
export function sceneElapsed(clock: ElapsedClock): number {
  return isSceneTimeFrozen() ? CAPTURE_FIXED_ELAPSED : clock.getElapsedTime();
}

/**
 * A per-frame smoothing factor, or `1` under capture — snap instead of ease.
 *
 * Freezing the clock is not enough on its own, and the reproducibility gate is what showed it:
 * `overview`, `collision` and `biome_transition` still differed between two runs after every animated
 * component was reading a fixed time. The remaining variation was `WorldWeather` easing the scene fog
 * toward its target with `k = min(1, delta * 2)`. That integrates *frame deltas*, so after a fixed
 * ninety frames the fog has converged by an amount that depends on how fast those ninety frames
 * happened to run — invisible in one image, a different image every time.
 *
 * A smoothed value's whole purpose is to hide a discontinuity from a viewer over time, and a still
 * frame has no time. The settled value is both the reproducible answer and the correct one.
 */
export function sceneSmoothing(k: number): number {
  return isSceneTimeFrozen() ? 1 : k;
}

/**
 * Frame step used for every integrated quantity under capture.
 *
 * 1/60 s, the nominal frame the scene is authored against. The particular value barely matters; that
 * it is a constant is the whole point.
 */
export const CAPTURE_FIXED_DELTA = 1 / 60;

/**
 * The frame delta, or a fixed step under capture.
 *
 * `sceneElapsed` covers everything that reads *absolute* time. This covers everything that reads the
 * *interval*, which is a separate and equally fatal channel: `x += delta * speed` after ninety frames
 * lands wherever those ninety frames' real durations put it.
 *
 * Cloud drift and precipitation advance that way, and both happen to be invisible at the canonical
 * defaults — clouds because capture sets `speed = 0`, precipitation because the canonical weather is
 * clear. "Unreachable at the current defaults" is not a property to rest a byte-identity gate on:
 * change one default and the gate goes red for a reason nobody will look for here.
 *
 * `sceneClock.test.ts` asserts structurally that no animated `World*` component consumes r3f's delta
 * without passing it through this.
 */
export function sceneDelta(delta: number): number {
  return isSceneTimeFrozen() ? CAPTURE_FIXED_DELTA : delta;
}

/** FNV-1a over a stream name — small, dependency-free, and stable across runs. */
function hashStream(name: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < name.length; i++) {
    h ^= name.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  // Non-zero: mulberry32 seeded with 0 still produces a usable sequence, but a distinct seed per
  // stream is the whole point and 0 is the value an empty name would collide on.
  return (h >>> 0) || 0x9e3779b9;
}

/** mulberry32 — 32-bit state, uniform in [0, 1), adequate for scattering scenery. */
function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * A random source for scattering scenery: `Math.random` normally, a seeded stream under capture.
 *
 * Named streams rather than one shared generator, because a shared one would make each consumer's
 * output depend on how many numbers every other consumer drew first — reproducible only as long as
 * component mount order never changes. `makeSceneRandom('sky.stars')` gives the same sequence
 * whatever else mounts.
 *
 * @param stream - Stable identifier, conventionally `component.purpose`.
 */
export function makeSceneRandom(stream: string): () => number {
  if (!isSceneTimeFrozen()) return Math.random;
  return mulberry32(hashStream(stream));
}
