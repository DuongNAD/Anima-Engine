import { describe, it, expect } from 'vitest';
import { grazePosition, grazeYaw } from '../../src/components/Landscape/utils/grazing';

const anchor = { x: 120, z: -45 };

function distanceFromAnchor(t: number, radius = 7, seed = 31, speed = 0.11): number {
  const p = grazePosition(anchor, radius, seed, t, speed);
  return Math.hypot(p.x - anchor.x, p.z - anchor.z);
}

describe('grazing path', () => {
  it('never leaves the radius its habitat probe approved', () => {
    // The probe checked eight compass points at this radius and found dry, walkable ground. A path
    // that exceeded it would put deer in the lake — the exact failure the probe exists to prevent,
    // reintroduced one layer later.
    let worst = 0;
    for (let t = 0; t < 400; t += 0.05) worst = Math.max(worst, distanceFromAnchor(t));
    expect(worst).toBeLessThanOrEqual(7 + 1e-9);
  });

  it('actually uses the space, rather than trembling in the middle', () => {
    // The bug being fixed was an animal that moved 0.04 units and read as scenery. A path that
    // stays bounded but never travels would pass the test above and fail the eye.
    let worst = 0;
    for (let t = 0; t < 400; t += 0.05) worst = Math.max(worst, distanceFromAnchor(t));
    expect(worst).toBeGreaterThan(5);
  });

  it('is a pure function of time, so a frozen clock freezes the herd', () => {
    // Capture mode pins `sceneElapsed`. If this drew on anything else — a counter, Math.random, the
    // real clock — the canonical views would stop being byte-reproducible and the gate that proves
    // it would start failing for a reason nobody would look for in a deer.
    const a = grazePosition(anchor, 7, 31, 12.5, 0.11);
    const b = grazePosition(anchor, 7, 31, 12.5, 0.11);
    expect(a).toEqual(b);
  });

  it('moves between two different moments', () => {
    const a = grazePosition(anchor, 7, 31, 10, 0.11);
    const b = grazePosition(anchor, 7, 31, 25, 0.11);
    expect(Math.hypot(a.x - b.x, a.z - b.z)).toBeGreaterThan(0.5);
  });

  it('does not walk a herd in lockstep', () => {
    // Same anchor, same radius, different seed: two animals side by side must not share a path, or
    // a herd reads as one rigid object sliding around.
    const a = grazePosition(anchor, 7, 31, 40, 0.11);
    const b = grazePosition(anchor, 7, 32, 40, 0.11);
    expect(Math.hypot(a.x - b.x, a.z - b.z)).toBeGreaterThan(0.5);
  });

  it('pins an animal whose surroundings were not walkable', () => {
    // radius 0 is the probe saying "there is nowhere safe to go from here".
    for (const t of [0, 5, 60, 900]) {
      expect(grazePosition(anchor, 0, 31, t, 0.11)).toEqual(anchor);
    }
  });
});

describe('grazing facing', () => {
  it('faces along the direction of travel', () => {
    const t = 17;
    const here = grazePosition(anchor, 7, 31, t, 0.11);
    const next = grazePosition(anchor, 7, 31, t + 0.35, 0.11);
    expect(grazeYaw(anchor, 7, 31, t, 0.11, 1.234)).toBeCloseTo(
      Math.atan2(next.x - here.x, next.z - here.z),
      10,
    );
  });

  it('keeps the placed yaw when the animal cannot move', () => {
    // Returning 0 here would snap every motionless deer to face north, which is worse than the
    // scattered facings placement gave them.
    expect(grazeYaw(anchor, 0, 31, 17, 0.11, 1.234)).toBe(1.234);
  });
});
