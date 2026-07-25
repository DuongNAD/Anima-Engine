// The explorer's camera drives where the simulation spends its brain inference. Two things can go
// wrong here and neither one throws:
//
//  1. The scale. Render space is 1200 units across, the backend world is 200. Send the camera
//     position raw and every focus lands outside the world, which reads as "nowhere near anything"
//     — so every agent tiers Cold and quietly stops thinking. It would look like a working feature.
//  2. The throttle. Too eager and it is an IPC message per frame; too lazy and the focus never
//     follows the explorer at all.

import { describe, it, expect } from 'vitest';
import {
  DEFAULT_XZ_BOUNDS,
  renderXzToWorldXz,
} from '../components/Landscape/utils/coordinate';
import {
  focusForViewport,
  focusFromLookAt,
  sendLodFocusNow,
  shouldSend,
  FOCUS_OFF,
  MIN_MOVE_WORLD_UNITS,
} from '../utils/lodFocus';

const RENDER_SIZE = 1200;

describe('renderXzToWorldXz', () => {
  it('maps the render extent onto the world extent, corner for corner', () => {
    const half = RENDER_SIZE / 2;
    expect(renderXzToWorldXz(0, 0, RENDER_SIZE)).toEqual([0, 0]);
    expect(renderXzToWorldXz(-half, -half, RENDER_SIZE)).toEqual([-100, -100]);
    expect(renderXzToWorldXz(half, half, RENDER_SIZE)).toEqual([100, 100]);
  });

  it('is the pure scale the coordinate contract promises — no rotation, no shear', () => {
    // A pure diagonal scale moves X with X only and Z with Z only, by one constant factor each.
    const [x1] = renderXzToWorldXz(300, 0, RENDER_SIZE);
    const [, z1] = renderXzToWorldXz(0, 300, RENDER_SIZE);
    expect(x1).toBeCloseTo(50, 10);
    expect(z1).toBeCloseTo(50, 10);
    // Moving along X must not disturb Z.
    const [, zOnXMove] = renderXzToWorldXz(300, 0, RENDER_SIZE);
    expect(zOnXMove).toBeCloseTo(0, 10);
  });

  it('round-trips against the cell mapping the backend uses', () => {
    // The world coordinate this produces must be the same one `cellCenterToWorldXz` would give for
    // the cell the explorer is standing over — otherwise render and simulation disagree about
    // where "here" is, which is the whole failure the coordinate contract exists to prevent.
    for (const [rx, rz] of [
      [0, 0],
      [123.5, -456.25],
      [-599.9, 599.9],
    ]) {
      const [wx, wz] = renderXzToWorldXz(rx, rz, RENDER_SIZE);
      expect(wx).toBeGreaterThanOrEqual(DEFAULT_XZ_BOUNDS.minX);
      expect(wx).toBeLessThanOrEqual(DEFAULT_XZ_BOUNDS.maxX);
      expect(wz).toBeGreaterThanOrEqual(DEFAULT_XZ_BOUNDS.minZ);
      expect(wz).toBeLessThanOrEqual(DEFAULT_XZ_BOUNDS.maxZ);
    }
  });

  it('falls back to the world centre rather than emitting NaN for a degenerate extent', () => {
    // The showcase renders before its world has finished loading. A NaN focus would be handed
    // straight to the tiering code, where a non-finite distance reads as Cold.
    expect(renderXzToWorldXz(10, 10, 0)).toEqual([0, 0]);
    expect(renderXzToWorldXz(10, 10, Number.NaN)).toEqual([0, 0]);
  });
});

describe('focusFromLookAt', () => {
  it('drops the camera height', () => {
    // The orbit camera sits at y = RENDER_SIZE * 0.5. Passing that through would put the observer
    // 600 units above a world 10 units tall, past the 50-unit hot radius from the air alone.
    const focus = focusFromLookAt(0, 0, RENDER_SIZE);
    expect(focus.center[1]).toBe(0);
    expect(focus.enabled).toBe(true);
  });

  it('produces the [x, y, z] tuple the Rust side deserialises', () => {
    const focus = focusFromLookAt(600, -600, RENDER_SIZE);
    expect(focus.center).toHaveLength(3);
    expect(focus.center).toEqual([100, 0, -100]);
  });

  it('clamps a look-at point outside the world to its edge, never past it', () => {
    // Observed in the running showcase: the default orbit camera sits at z ≈ +960 on a 1200-wide
    // scene, which maps to z ≈ +260 in a world that ends at 100. An unclamped focus there is
    // farther from every agent than the cold radius, so the whole population stops thinking — and
    // this page draws no agents, so nothing on screen would look wrong.
    const past = focusFromLookAt(0, 960, RENDER_SIZE);
    expect(past.center[2]).toBe(100);

    const shore = focusFromLookAt(-5000, 5000, RENDER_SIZE);
    expect(shore.center[0]).toBe(-100);
    expect(shore.center[2]).toBe(100);
  });

  it('never emits a non-finite centre', () => {
    const bad = focusFromLookAt(Number.NaN, Number.POSITIVE_INFINITY, RENDER_SIZE);
    expect(bad.center.every(Number.isFinite)).toBe(true);
  });
});

describe('shouldSend', () => {
  const at = (x: number, z: number) => focusFromLookAt(x, z, RENDER_SIZE);

  it('always sends the first focus', () => {
    expect(shouldSend(null, at(0, 0))).toBe(true);
  });

  it('holds back movement too small to change any agent tier', () => {
    const a = at(0, 0);
    // One render unit is a sixth of a world unit — far under the threshold, and far under the
    // 50-unit hot radius it would have to cross to matter.
    expect(shouldSend(a, at(1, 0))).toBe(false);
  });

  it('sends once the explorer has actually gone somewhere', () => {
    const a = at(0, 0);
    // MIN_MOVE_WORLD_UNITS in world units is six times that in render units.
    expect(shouldSend(a, at(MIN_MOVE_WORLD_UNITS * 6 + 1, 0))).toBe(true);
  });

  it('always sends a change of enabled, in both directions', () => {
    const on = at(0, 0);
    expect(shouldSend(on, FOCUS_OFF)).toBe(true);
    expect(shouldSend(FOCUS_OFF, on)).toBe(true);
  });

  it('does not chatter while disabled', () => {
    expect(shouldSend(FOCUS_OFF, FOCUS_OFF)).toBe(false);
  });

  it('refuses a non-finite focus instead of steering the simulation with it', () => {
    const a = at(0, 0);
    const nan = { enabled: true, center: [Number.NaN, 0, 0] as [number, number, number] };
    expect(shouldSend(a, nan)).toBe(false);
  });
});

describe('sendLodFocusNow', () => {
  // Runs before anything in this file has resolved the Tauri bridge, which is the state a plain
  // browser is permanently in.
  it('reports nothing sent rather than throwing while the page is unloading', () => {
    expect(sendLodFocusNow(FOCUS_OFF)).toBe(false);
  });
});

describe('focusForViewport', () => {
  // The agent viewport draws the agents, so unlike the landscape showcase a wrong answer here is
  // visible as sluggish behaviour. The rule these pin: never degrade an agent that is on screen.
  const HOT = 50;

  it('focuses on the view centre when everything visible fits inside the hot radius', () => {
    const focus = focusForViewport(20, -30, 10, HOT);
    expect(focus.enabled).toBe(true);
    expect(focus.center).toEqual([20, 0, -30]);
  });

  it('gives up tiering rather than degrade an agent the user can see', () => {
    // Zoomed out far enough that the screen corner is outside the hot band, some visible agent
    // would drop to Warm or Cold. Uniform detail is the correct answer, not a cheaper one.
    expect(focusForViewport(0, 0, HOT + 0.1, HOT).enabled).toBe(false);
    // Exactly at the radius is still safe — the boundary is inclusive on the backend too.
    expect(focusForViewport(0, 0, HOT, HOT).enabled).toBe(true);
  });

  it('does not tier at all when the backend never told us the radius', () => {
    // A plain browser, or a failed `get_lod_bands`. Guessing 50 here would be a second definition
    // of a backend constant, drifting silently the day someone changes `LodBands::default`.
    expect(focusForViewport(0, 0, 1, null).enabled).toBe(false);
    expect(focusForViewport(0, 0, 1, Number.NaN).enabled).toBe(false);
  });

  it('clamps a centre outside the world instead of pointing off the map', () => {
    expect(focusForViewport(999, -999, 1, HOT).center).toEqual([100, 0, -100]);
  });

  it('refuses a non-finite centre', () => {
    expect(focusForViewport(Number.NaN, 0, 1, HOT).enabled).toBe(false);
  });
});
