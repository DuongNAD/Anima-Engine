import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  OCEAN_SHELF_BAND_FRACTION,
  distanceOutsideFootprint,
  oceanShelfFalloff,
} from '@/components/Landscape/utils/oceanShelf';

// The world used to have a visible edge, drawn in water.
//
// The ocean shader multiplied its terrain-height sample by a binary inside/outside step, so the sea
// floor fell the whole water column across one texel at the footprint boundary. Depth, colour and
// alpha all jumped with it: a hard turquoise-to-navy line, straight, on all four sides. Independent
// review rejected canonical views for it three times, and the first two repairs treated it as a
// framing problem — which it never was. No camera pose photographs a bounded world without its
// boundary; `overview`'s subject *is* the whole bounded world.
//
// What is asserted here is the property the fix rests on — the floor is continuous at the boundary and
// reaches abyssal smoothly — plus that the shader still says so. A reference implementation that has
// drifted from the GLSL it describes is worse than not having one.

const HERE = dirname(fileURLToPath(import.meta.url));
const WATER_SRC = readFileSync(
  resolve(HERE, '../../src/components/Landscape/WorldWater.tsx'),
  'utf8',
);

const RENDER_SIZE = 1200;
const BAND = RENDER_SIZE * OCEAN_SHELF_BAND_FRACTION;

describe('ocean shelf falloff', () => {
  it('is exactly 1 at and inside the footprint boundary', () => {
    // The continuity that removes the line. Inside, the floor is the sampled terrain height; at the
    // boundary the ramp must not have started, or the seam simply moves inward by an epsilon.
    expect(oceanShelfFalloff(0, BAND)).toBe(1);
    expect(oceanShelfFalloff(-50, BAND)).toBe(1);
    expect(oceanShelfFalloff(1e-6, BAND)).toBeCloseTo(1, 9);
  });

  it('reaches abyssal at the end of the band and stays there', () => {
    expect(oceanShelfFalloff(BAND, BAND)).toBe(0);
    expect(oceanShelfFalloff(BAND * 2, BAND)).toBe(0);
    expect(oceanShelfFalloff(RENDER_SIZE * 15, BAND)).toBe(0);
  });

  it('descends monotonically, with no step anywhere along the band', () => {
    // "No step" is the whole claim. Sampling at a fine granularity and bounding the largest
    // single-sample change is how a discontinuity would be caught: the old binary step would show up
    // here as a jump of 1.0 between two adjacent samples.
    const samples = 2000;
    let previous = oceanShelfFalloff(-1, BAND);
    let largestDrop = 0;
    for (let i = 0; i <= samples; i++) {
      const v = oceanShelfFalloff((i / samples) * BAND * 1.2, BAND);
      expect(v).toBeLessThanOrEqual(previous + 1e-12);
      largestDrop = Math.max(largestDrop, previous - v);
      previous = v;
    }
    expect(previous).toBe(0);
    // 1.5/samples is smoothstep's maximum slope (1.5) times the sample spacing.
    expect(largestDrop).toBeLessThan((1.5 / samples) * 1.2 + 1e-9);
  });

  it('finishes well before the water fades into fog', () => {
    // If the ramp only completed past the fog distance, the transition would be hidden rather than
    // fixed — and it would reappear on any change that pushed the fog back.
    const fogNear = RENDER_SIZE * 0.8;
    expect(BAND).toBeLessThan(fogNear);
  });

  it('measures distance to a square footprint, not to its centre', () => {
    const half = RENDER_SIZE / 2;
    expect(distanceOutsideFootprint(0, 0, RENDER_SIZE)).toBe(0);
    expect(distanceOutsideFootprint(half, half, RENDER_SIZE)).toBe(0);
    expect(distanceOutsideFootprint(half + 30, 0, RENDER_SIZE)).toBe(30);
    expect(distanceOutsideFootprint(0, -half - 30, RENDER_SIZE)).toBe(30);
    // A corner is `half` from the boundary on both axes at once; the larger overrun is the one that
    // says how far past the data this fragment is.
    expect(distanceOutsideFootprint(half + 30, half + 5, RENDER_SIZE)).toBe(30);
  });
});

describe('the ocean shader matches this reference', () => {
  it('no longer multiplies the height sample by a binary inside/outside step', () => {
    // The exact expression that produced the cut plane. Naming it here means a revert cannot pass.
    expect(WATER_SRC).not.toMatch(/float\s+inMap\s*=\s*step\(/);
    expect(WATER_SRC).not.toMatch(/uHeightMap[^;]*\)\s*\.r\s*\*\s*inMap/);
  });

  it('ramps the floor over the shelf band instead', () => {
    expect(WATER_SRC).toMatch(/uniform\s+float\s+uShelfBand;/);
    expect(WATER_SRC).toMatch(/float\s+shelf\s*=\s*1\.0\s*-\s*smoothstep\(\s*0\.0\s*,\s*uShelfBand\s*,/);
    expect(WATER_SRC).toMatch(/uHeightMap[^;]*\)\s*\.r\s*\*\s*shelf/);
  });

  it('feeds the band from the shared constant rather than a second literal', () => {
    expect(WATER_SRC).toMatch(/uShelfBand:\s*\{\s*value:\s*renderSize\s*\*\s*OCEAN_SHELF_BAND_FRACTION\s*\}/);
  });
});
