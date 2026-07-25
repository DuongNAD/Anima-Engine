// ---------------------------------------------------------------------------------------
// Where the explorer is standing, told to the simulation.
//
// The backend spends its per-tick brain inference by distance from a focus point
// (`core/simulation_lod.rs`): near it agents think every tick, far from it less often or not at
// all. Nothing sets that focus by itself — a headless run has no camera — so until this file
// existed the whole tier sat switched off in the shipped app.
//
// Two things are worth knowing before reading further.
//
// **This page does not draw agents.** `landscape.html` renders terrain, water and wildlife; the
// agent views are `App.tsx` and `PixiViewport.tsx` on `index.html`. So the focus follows an
// explorer walking the same world the simulation runs on (the showcase hands it over via
// `save_world_artifact`), but you will not *see* the tiering here. That is a reason to be careful
// about what this claims, not a reason for it to be wrong.
//
// **Off is a state, not an absence.** The camera stops sending on unmount, and a focus that simply
// stopped being updated would leave the simulation tiered forever around wherever the explorer
// last stood — every distant agent frozen out of thinking because a page closed. So leaving sends
// an explicit disabled focus, which the backend reads as uniform detail.
// ---------------------------------------------------------------------------------------

import {
  DEFAULT_XZ_BOUNDS,
  renderXzToWorldXz,
  type XZBounds,
} from '../components/Landscape/utils/coordinate';

/** The payload of the `set_lod_focus` Tauri command. `center` is `[x, y, z]` because that is how
 * glam's `Vec3` deserialises — pinned backend-side by
 * `the_focus_deserialises_from_the_json_the_frontend_sends`. */
export interface LodFocusPayload {
  enabled: boolean;
  center: [number, number, number];
}

/** A focus that turns tiering off: every agent thinks every tick, as before simulation LOD. */
export const FOCUS_OFF: LodFocusPayload = { enabled: false, center: [0, 0, 0] };

/**
 * How far, in world units, the explorer must move before the focus is worth resending.
 *
 * The backend's default hot radius is 50 world units, so a metre of camera drift cannot change any
 * agent's tier. Sending it anyway would put an IPC message on the wire every frame to move a
 * threshold nothing crosses.
 */
export const MIN_MOVE_WORLD_UNITS = 2;

/** How often the camera is sampled, in ms. Independent of the render loop: the focus is a hint
 * about where detail belongs, and four updates a second is finer than tiers can respond. */
export const SAMPLE_INTERVAL_MS = 250;

/**
 * The focus for an explorer **looking at** render-space `(lookX, lookZ)`.
 *
 * Three corrections are baked in here, and each one was a way to ship something that runs and is
 * silently wrong.
 *
 * **The look-at point, not the camera position.** `CameraView` carries both. In orbit mode the
 * camera is pulled back off the terrain to frame the whole continent — measured at `z ≈ +960` on a
 * scene 1200 wide — which maps to `z ≈ +260` in a world that ends at 100. Focusing there puts every
 * agent past the cold radius, so the entire population stops thinking, and the view looks fine
 * because this page does not draw agents. The rig's `target` is where attention actually is: the
 * orbit centre, or 60 units ahead of a walking explorer.
 *
 * **Clamped into the world.** A look-at point can still leave the world legitimately — walking to
 * the shore and looking out to sea. The nearest point inside the world is the honest reading of
 * that; "nowhere" is not, because "nowhere" silently means "no agent is near the observer".
 *
 * **`y` is zero.** Tiering measures distance in three dimensions against a 50-unit hot radius, in a
 * world 10 units tall. A camera hovering 600 units up would be cold from altitude alone.
 */
export function focusFromLookAt(
  lookX: number,
  lookZ: number,
  renderSize: number,
  bounds: XZBounds = DEFAULT_XZ_BOUNDS,
): LodFocusPayload {
  const [rawX, rawZ] = renderXzToWorldXz(lookX, lookZ, renderSize, bounds);
  return {
    enabled: true,
    center: [clampInto(rawX, bounds.minX, bounds.maxX), 0, clampInto(rawZ, bounds.minZ, bounds.maxZ)],
  };
}

/** Nearest point in `[lo, hi]`, with a non-finite input reading as the middle rather than
 * propagating: a NaN focus reaches the tiering code as a non-finite distance, which is Cold. */
function clampInto(v: number, lo: number, hi: number): number {
  return Number.isFinite(v) ? Math.min(Math.max(v, lo), hi) : (lo + hi) / 2;
}

/**
 * The focus for a 2D agent viewport centred on world `(x, z)` and showing `visibleHalfExtent` world
 * units out to its farthest corner.
 *
 * Unlike the landscape showcase, this view **draws the agents**, so a mistake here is visible as
 * sluggish behaviour rather than invisible. One rule follows from that and decides everything:
 * *never degrade an agent that is on screen.* Tiering below `Hot` starts beyond `hotRadius`, so a
 * focus is only safe when the farthest visible point is inside that radius. Zoomed out past it,
 * uniform detail is the correct answer — every agent on screen is one the user is looking at.
 *
 * `hotRadius` comes from the backend (`get_lod_bands`) rather than a literal here. A `null` — the
 * command failed, or this is a plain browser — means the safe radius is unknown, and unknown reads
 * as "do not tier", never as "tier anyway".
 */
export function focusForViewport(
  x: number,
  z: number,
  visibleHalfExtent: number,
  hotRadius: number | null,
  bounds: XZBounds = DEFAULT_XZ_BOUNDS,
): LodFocusPayload {
  if (hotRadius === null || !Number.isFinite(hotRadius)) return FOCUS_OFF;
  if (!Number.isFinite(visibleHalfExtent) || visibleHalfExtent > hotRadius) return FOCUS_OFF;
  if (!Number.isFinite(x) || !Number.isFinite(z)) return FOCUS_OFF;
  // Already in world units — no projection to undo, only the same clamp the other entry point uses.
  return { enabled: true, center: [clampInto(x, bounds.minX, bounds.maxX), 0, clampInto(z, bounds.minZ, bounds.maxZ)] };
}

/**
 * Whether `next` is a big enough change from the last sent focus to be worth an IPC round trip.
 *
 * A change of `enabled` always sends: turning tiering on or off is the whole point, and it is not a
 * movement that a distance threshold could ever measure.
 */
export function shouldSend(
  last: LodFocusPayload | null,
  next: LodFocusPayload,
  minMove: number = MIN_MOVE_WORLD_UNITS,
): boolean {
  if (!last) return true;
  if (last.enabled !== next.enabled) return true;
  if (!next.enabled) return false;
  const dx = next.center[0] - last.center[0];
  const dz = next.center[2] - last.center[2];
  if (!Number.isFinite(dx) || !Number.isFinite(dz)) return false;
  return Math.hypot(dx, dz) >= minMove;
}

type InvokeFn = (cmd: string, args: unknown) => Promise<unknown>;

/** Resolved once, then reused — see {@link sendLodFocusNow} for why the caching is load-bearing
 * rather than an optimisation. */
let cachedInvoke: InvokeFn | null = null;

/**
 * Send a focus to the backend. Resolves to `true` if it was delivered.
 *
 * No-ops silently outside Tauri — the browser showcase, tests, SSR — exactly as
 * `persistWorldArtifact` does for the world artifact. A page that renders terrain in a plain
 * browser has no simulation to steer, and that is not an error worth surfacing.
 */
export async function sendLodFocus(focus: LodFocusPayload): Promise<boolean> {
  try {
    if (!cachedInvoke) {
      const { invoke } = await import('@tauri-apps/api/core');
      cachedInvoke = invoke as unknown as InvokeFn;
    }
    await cachedInvoke('set_lod_focus', { focus });
    return true;
  } catch {
    return false;
  }
}

/**
 * The backend's hot radius in world units, or `null` if it cannot be asked.
 *
 * Read once and reused. `null` is the safe answer everywhere it appears — {@link focusForViewport}
 * treats an unknown radius as "do not tier", so a plain browser or a failed command loses the
 * optimisation rather than silently degrading agents against a guessed number.
 */
export async function fetchHotRadius(): Promise<number | null> {
  try {
    if (!cachedInvoke) {
      const { invoke } = await import('@tauri-apps/api/core');
      cachedInvoke = invoke as unknown as InvokeFn;
    }
    const bands = (await cachedInvoke('get_lod_bands', {})) as { hot_radius?: number } | null;
    const r = bands?.hot_radius;
    return typeof r === 'number' && Number.isFinite(r) && r > 0 ? r : null;
  } catch {
    return null;
  }
}

/**
 * Send a focus without awaiting anything, for `pagehide`.
 *
 * Leaving the page is the one moment the async version cannot serve. `sendLodFocus` begins with
 * `await import(...)`, and a document being torn down does not survive long enough for a dynamic
 * import to resolve — measured: the disable simply never reached `invoke`, and the simulation was
 * left tiered around a camera that no longer existed. Calling the already-resolved `invoke`
 * synchronously posts the message before the context goes away.
 *
 * Returns `false` if nothing has been sent successfully yet, which also covers "not under Tauri" —
 * in which case there is no focus set and nothing to turn off.
 */
export function sendLodFocusNow(focus: LodFocusPayload): boolean {
  if (!cachedInvoke) return false;
  try {
    // Fire-and-forget: nothing can await on a document that is going away. The rejection is
    // swallowed rather than left unhandled, which would surface as a console error at the exact
    // moment nobody is left to read it.
    void Promise.resolve(cachedInvoke('set_lod_focus', { focus })).catch(() => {});
    return true;
  } catch {
    return false;
  }
}
