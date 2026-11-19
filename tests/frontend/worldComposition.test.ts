import { describe, it, expect } from 'vitest';
import { Biome, BIOME_COUNT, type World } from '@/components/Landscape/utils/worldGen';
import {
  COMPOSITION_BANDS,
  REGION_BIOMES,
  compositionFractions,
  compositionViolations,
  measureComposition,
  type WorldComposition,
} from '@/components/Landscape/utils/worldComposition';

// Unit tests for the composition gate (C5). `mapEvidence.test.ts` runs the gate on the real 2048²
// shared world; this file checks, without generating a world, that the measurement counts what it
// says and that each band actually fails when it should. A gate that passes the real world proves
// little unless it is also shown to fail a bad one.

/** A world with only the fields `measureComposition` reads. */
function tinyWorld(
  size: number,
  biomes: number[],
  extra: { waterfalls?: number; caves?: Array<[number, number]> } = {},
): World {
  const caves = extra.caves ?? [];
  return {
    size,
    biome: Uint8Array.from(biomes),
    waterfallCount: extra.waterfalls ?? 0,
    caveCount: caves.length,
    caveX: Float32Array.from(caves.map((c) => c[0])),
    caveZ: Float32Array.from(caves.map((c) => c[1])),
  } as unknown as World;
}

/** A composition just inside every band, at 2048², to perturb one field at a time. */
function goodComposition(): WorldComposition {
  const cells = 2048 * 2048;
  const biomeCells: Record<string, number> = {};
  const largestRegionCells: Record<string, number> = {};
  for (let b = 0; b < BIOME_COUNT; b++) {
    biomeCells[Biome[b]] = 0;
    largestRegionCells[Biome[b]] = 0;
  }
  biomeCells.Ocean = Math.round(0.45 * cells);
  for (const b of REGION_BIOMES) {
    biomeCells[Biome[b]] = 10_000;
    largestRegionCells[Biome[b]] = 5_000;
  }
  return {
    cells,
    biomeCells,
    largestRegionCells,
    waterfallCount: 52,
    caveCount: 45,
    caveMinSpacingCells: 40,
  };
}

describe('measureComposition', () => {
  it('counts cells per biome and labels regions 4-connected, not 8-connected', () => {
    // 4×4. Desert forms a diagonal line: four cells touching only at corners, so four regions of
    // one (8-connected it would be one region of four). The diagonal also cuts the Steppe in two,
    // six cells above it and five below (8-connected they would join through the corners into 11).
    const D = Biome.Desert;
    const S = Biome.Steppe;
    const O = Biome.Ocean;
    const world = tinyWorld(4, [D, S, S, S, S, D, S, S, S, S, D, S, O, S, S, D]);
    const c = measureComposition(world);
    expect(c.cells).toBe(16);
    expect(c.biomeCells.Desert).toBe(4);
    expect(c.biomeCells.Steppe).toBe(11);
    expect(c.biomeCells.Ocean).toBe(1);
    expect(c.largestRegionCells.Desert).toBe(1);
    expect(c.largestRegionCells.Steppe).toBe(6);
    expect(c.largestRegionCells.Jungle).toBe(0);
    expect(compositionFractions(c).Steppe).toBeCloseTo(11 / 16, 12);
  });

  it('does not join regions across the left and right edges of a row', () => {
    // 3×3, Jungle at the end of row 0 and the start of row 1: neighbours by index, not on the map.
    const J = Biome.Jungle;
    const G = Biome.Grassland;
    const c = measureComposition(tinyWorld(3, [G, G, J, J, G, G, G, G, G]));
    expect(c.largestRegionCells.Jungle).toBe(1);
  });

  it('measures the closest pair of cave mouths, and 0 with fewer than two', () => {
    const G = Biome.Grassland;
    const flat = Array.from({ length: 16 }, () => G);
    const three = measureComposition(
      tinyWorld(4, flat, {
        caves: [
          [0, 0],
          [3, 4],
          [10, 0],
        ],
      }),
    );
    expect(three.caveCount).toBe(3);
    expect(three.caveMinSpacingCells).toBe(5);
    expect(measureComposition(tinyWorld(4, flat, { caves: [[1, 1]] })).caveMinSpacingCells).toBe(0);
  });
});

describe('compositionViolations', () => {
  it('passes a composition inside every band', () => {
    expect(compositionViolations(goodComposition(), 2048)).toEqual([]);
  });

  it('fails an ocean world like the one before PR #161, and a world with too little sea', () => {
    const wet = goodComposition();
    wet.biomeCells.Ocean = Math.round(0.62 * wet.cells);
    expect(compositionViolations(wet, 2048).join('\n')).toMatch(/ocean fraction 0\.620/);
    const dry = goodComposition();
    dry.biomeCells.Ocean = Math.round(0.3 * dry.cells);
    expect(compositionViolations(dry, 2048).join('\n')).toMatch(/ocean fraction 0\.300/);
  });

  it('fails when any one environment vanishes or survives only as scattered cells', () => {
    for (const b of REGION_BIOMES) {
      const c = goodComposition();
      c.largestRegionCells[Biome[b]] = 1_999;
      const v = compositionViolations(c, 2048);
      expect(v, Biome[b]).toHaveLength(1);
      expect(v[0]).toContain(`${Biome[b]}: largest region 1999 cells`);
    }
  });

  it('does not ask strips and water bodies for a region (Ocean, Lake, River, Beach)', () => {
    const c = goodComposition();
    for (const name of ['Lake', 'River', 'Beach']) c.largestRegionCells[name] = 0;
    expect(compositionViolations(c, 2048)).toEqual([]);
    expect(REGION_BIOMES).not.toContain(Biome.Ocean);
    expect(REGION_BIOMES).toHaveLength(BIOME_COUNT - 4);
  });

  it('fails the waterfall clutter of the old world, and a world with almost none', () => {
    const many = goodComposition();
    many.waterfallCount = 450;
    expect(compositionViolations(many, 2048)).toEqual(['450 waterfalls, outside [20, 80]']);
    const few = goodComposition();
    few.waterfallCount = 5;
    expect(compositionViolations(few, 2048)).toEqual(['5 waterfalls, outside [20, 80]']);
  });

  it('fails too few or too many cave mouths, and two mouths crowded together', () => {
    const few = goodComposition();
    few.caveCount = 10;
    expect(compositionViolations(few, 2048)).toEqual(['10 cave mouths, outside [30, 48]']);
    const many = goodComposition();
    many.caveCount = 50;
    expect(compositionViolations(many, 2048)).toEqual(['50 cave mouths, outside [30, 48]']);
    const crowded = goodComposition();
    crowded.caveMinSpacingCells = 2;
    expect(compositionViolations(crowded, 2048)).toEqual([
      'two cave mouths 2.0 cells apart, below 20.5',
    ]);
  });

  it('scales the size-relative bands with the world, so a smaller world is judged alike', () => {
    // The region floor is 2,000 cells at 2048²; at 512² the same fraction is 125 cells.
    const c = goodComposition();
    c.cells = 512 * 512;
    c.biomeCells.Ocean = Math.round(0.45 * c.cells);
    for (const b of REGION_BIOMES) c.largestRegionCells[Biome[b]] = 125;
    c.caveMinSpacingCells = 5.12;
    expect(compositionViolations(c, 512)).toEqual([]);
    c.largestRegionCells.Swamp = 124;
    expect(compositionViolations(c, 512)).toEqual(['Swamp: largest region 124 cells, below the floor of 125']);
    expect(COMPOSITION_BANDS.minLargestRegionFraction * 2048 * 2048).toBeCloseTo(2000, 9);
  });
});
