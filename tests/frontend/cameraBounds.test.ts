import { describe, it, expect } from 'vitest';
import {
  SPECTATOR_FRAMING_FRACTION,
  TERRAIN_FOOTPRINT_FRACTION,
  clampToCameraBounds,
  horizontalLimit,
  isGroundedMode,
  isOverTerrain,
  type BoundedCameraMode,
} from '@/components/Landscape/utils/cameraBounds';
import { CANONICAL_VIEW_CAMERAS } from '@/components/Landscape/utils/mapManifest';
import { FLORA_RADIUS_REFERENCE_EXTENT } from '@/components/Landscape/utils/floraClearance';

// The walk-boundary policy, tested.
//
// The escape it closes: the terrain mesh spans ±renderSize/2, one clamp of 0.75·renderSize applied to
// every mode, and `surfaceHeight` returns sea level off the mesh. So walking west past the shoreline
// continued for another quarter of a world over open water with plausible ground underfoot. It was
// found by looking at a screenshot and fixed with an inline ternary in a `useFrame` body — a number
// chosen from a picture, in the one place a test cannot reach. This is that number, reachable.

const RENDER = FLORA_RADIUS_REFERENCE_EXTENT; // 1200, the shipped scene extent
const SPECTATORS: BoundedCameraMode[] = ['orbit', 'fly', 'top', 'cine'];

describe('camera bounds — a walker gets the mesh, a photographer gets room to frame it', () => {
  it('confines walk to the terrain footprint exactly', () => {
    expect(horizontalLimit('walk', RENDER)).toBe(RENDER * TERRAIN_FOOTPRINT_FRACTION);
    expect(horizontalLimit('walk', RENDER)).toBe(600);
  });

  it('lets every spectator mode pull back beyond the footprint', () => {
    for (const mode of SPECTATORS) {
      expect(horizontalLimit(mode, RENDER), mode).toBe(RENDER * SPECTATOR_FRAMING_FRACTION);
      expect(horizontalLimit(mode, RENDER), mode).toBeGreaterThan(horizontalLimit('walk', RENDER));
    }
  });

  it('classifies exactly one mode as grounded', () => {
    expect(isGroundedMode('walk')).toBe(true);
    for (const mode of SPECTATORS) expect(isGroundedMode(mode), mode).toBe(false);
  });

  it('never lets a walker leave the mesh, on either axis or both', () => {
    // The specific escape: 300 units past the western shoreline.
    expect(clampToCameraBounds('walk', RENDER, -900, 0)).toEqual({ x: -600, z: 0 });
    expect(clampToCameraBounds('walk', RENDER, 0, 900)).toEqual({ x: 0, z: 600 });
    expect(clampToCameraBounds('walk', RENDER, 5000, -5000)).toEqual({ x: 600, z: -600 });
  });

  it('leaves an in-bounds position untouched', () => {
    expect(clampToCameraBounds('walk', RENDER, -128.7, -93.5)).toEqual({ x: -128.7, z: -93.5 });
    expect(clampToCameraBounds('orbit', RENDER, 700, -700)).toEqual({ x: 700, z: -700 });
  });

  it('agrees with `isOverTerrain` at every clamped walk position', () => {
    // The property that matters: whatever a walker's requested position, the clamped result is over
    // ground that exists. A clamp that allowed the boundary itself but not `isOverTerrain` would be
    // the same defect one unit further out.
    for (const x of [-5000, -601, -600, -599, 0, 599, 600, 601, 5000]) {
      for (const z of [-5000, -600, 0, 600, 5000]) {
        const c = clampToCameraBounds('walk', RENDER, x, z);
        expect(isOverTerrain(RENDER, c.x, c.z), `(${x}, ${z}) -> (${c.x}, ${c.z})`).toBe(true);
      }
    }
  });

  it('does not claim ground off the mesh', () => {
    expect(isOverTerrain(RENDER, 600, 600)).toBe(true);
    expect(isOverTerrain(RENDER, 600.001, 0)).toBe(false);
    expect(isOverTerrain(RENDER, 0, -600.001)).toBe(false);
  });

  it('scales with the scene rather than hard-coding 600', () => {
    expect(horizontalLimit('walk', 400)).toBe(200);
    expect(clampToCameraBounds('walk', 400, 999, 0)).toEqual({ x: 200, z: 0 });
  });

  it('is the wider limit that the spectator canonical views need', () => {
    // `overview` sits at canonical y/z 112 on a ±100 world — 672 render units out, past the footprint
    // and inside the spectator limit. If the walk limit were applied to every mode this pose would be
    // unreachable, which is why there are two limits rather than one.
    const overviewRender = (CANONICAL_VIEW_CAMERAS.overview.position[2] / 200) * RENDER;
    expect(overviewRender).toBeGreaterThan(horizontalLimit('walk', RENDER));
    expect(overviewRender).toBeLessThanOrEqual(horizontalLimit('orbit', RENDER));
  });
});
