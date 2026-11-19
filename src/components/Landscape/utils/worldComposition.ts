// What the shared world is made of, measured, and the bands it has to stay inside.
//
// # Why this exists
//
// The shared world used to be 62% ocean, with the hot biomes almost absent and 450 waterfalls on
// every slope. PR #161 fixed that with the `supercontinent` shape, but nothing recorded what it
// achieved: `artifacts/map_evidence.json` held only the navigation and collision claims. A later edit
// to the generator could bring the old world back without failing anything.
//
// So the composition is computed here, written into the evidence record by
// `scripts/gen_map_evidence.ts`, and `tests/frontend/mapEvidence.test.ts` recomputes it and checks it
// against `COMPOSITION_BANDS`.
//
// The bands come from measuring PR #161's world (seed "seed", 2048², worldgen 22). They are not
// targets from outside the project. They are wide enough to let the world be retuned, and narrow
// enough that losing an environment, or drifting back towards the old ocean world, fails. Changing a
// band is a decision: say why in the same commit.

import { Biome, BIOME_COUNT, type World } from './worldGen';

/**
 * Composition of one generated world, as whole-cell counts so a committed record compares exactly
 * with a recomputed one. `compositionFractions` turns the counts into fractions of the map.
 */
export interface WorldComposition {
  /** Cells in the world (size²). */
  cells: number;
  /** Cells per biome, keyed by the `Biome` enum name. `Ocean` is below sea level, not a lake. */
  biomeCells: Record<string, number>;
  /** Largest 4-connected region of each biome, in cells, keyed by the `Biome` enum name. */
  largestRegionCells: Record<string, number>;
  waterfallCount: number;
  caveCount: number;
  /**
   * Smallest distance between two cave mouths, in cells, rounded to 3 decimals; 0 when there are
   * fewer than two.
   */
  caveMinSpacingCells: number;
}

/** Fraction of the map per biome, keyed by the `Biome` enum name. */
export function compositionFractions(c: WorldComposition): Record<string, number> {
  const out: Record<string, number> = {};
  for (const [name, n] of Object.entries(c.biomeCells)) out[name] = n / c.cells;
  return out;
}

/**
 * Biomes that must each form a visible region. All of them except the water bodies and the strips
 * that follow them: `Ocean` and `Lake` are water, `River` is a ribbon one or two cells wide and
 * `Beach` a strip along the coast, so their largest region measures connectivity, not visibility.
 */
export const REGION_BIOMES: ReadonlyArray<Biome> = Array.from(
  { length: BIOME_COUNT },
  (_, b) => b as Biome,
).filter((b) => b !== Biome.Ocean && b !== Biome.Lake && b !== Biome.River && b !== Biome.Beach);

/** The bands a world's composition must lie inside. */
export interface CompositionBands {
  /** Inclusive [min, max] fraction of ocean cells. */
  oceanFraction: readonly [number, number];
  /**
   * Smallest acceptable largest-region size for every biome in `REGION_BIOMES`, as a fraction of all
   * cells (so the same band applies to a smaller test world).
   */
  minLargestRegionFraction: number;
  /** Inclusive [min, max] waterfall count. */
  waterfalls: readonly [number, number];
  /** Inclusive [min, max] cave-mouth count. */
  caves: readonly [number, number];
  /** Smallest acceptable distance between two cave mouths, as a fraction of the world's size. */
  minCaveSpacingFraction: number;
}

/**
 * Measured on PR #161's world (seed "seed", 2048², worldgen 22); the full numbers are the
 * `composition` section of `artifacts/map_evidence.json`.
 * - Ocean 0.450: band 0.40-0.50. The old `continent` shape is 0.620.
 * - Largest region per biome in `REGION_BIOMES`: the smallest is Swamp at 5,783 cells (then Bog
 *   6,346, Alpine 7,610, Mangrove 8,603). The floor is 2,000 cells at 2048² (0.048% of the map,
 *   about a 45 × 45 patch). A biome that vanishes, or survives only as scattered cells, fails it.
 * - 52 waterfalls: band 20-80. The old world had 450.
 * - 45 cave mouths: band 30-48. `SUPERCONTINENT.caves.max` caps the count at 48.
 * - Closest two cave mouths 40 cells apart (`SUPERCONTINENT.caves.spacing` at 2048²). The floor is
 *   1% of the world's edge, 20.5 cells at 2048². The old world had two mouths 2 cells apart.
 *
 * Negative controls, run when these bands were set: the same seed with
 * `SUPERCONTINENT.landFraction` at 0.38 fails ocean (0.619) and the Swamp and Bog floors; the old
 * `continent` shape fails ocean, three biome floors, waterfalls, caves and cave spacing.
 */
export const COMPOSITION_BANDS: CompositionBands = {
  oceanFraction: [0.4, 0.5],
  minLargestRegionFraction: 2000 / (2048 * 2048),
  waterfalls: [20, 80],
  caves: [30, 48],
  minCaveSpacingFraction: 0.01,
};

/** Measure a world's composition. Labels 4-connected same-biome regions in one pass, O(cells). */
export function measureComposition(world: World): WorldComposition {
  const { size, biome } = world;
  const cells = size * size;

  const counts = new Int32Array(BIOME_COUNT);
  for (let i = 0; i < cells; i++) counts[biome[i]]++;

  const largest = new Int32Array(BIOME_COUNT);
  const seen = new Uint8Array(cells);
  const queue = new Int32Array(cells);
  for (let start = 0; start < cells; start++) {
    if (seen[start]) continue;
    const b = biome[start];
    seen[start] = 1;
    let head = 0;
    let tail = 0;
    queue[tail++] = start;
    while (head < tail) {
      const i = queue[head++];
      const x = i % size;
      if (x > 0 && !seen[i - 1] && biome[i - 1] === b) {
        seen[i - 1] = 1;
        queue[tail++] = i - 1;
      }
      if (x < size - 1 && !seen[i + 1] && biome[i + 1] === b) {
        seen[i + 1] = 1;
        queue[tail++] = i + 1;
      }
      if (i >= size && !seen[i - size] && biome[i - size] === b) {
        seen[i - size] = 1;
        queue[tail++] = i - size;
      }
      if (i < cells - size && !seen[i + size] && biome[i + size] === b) {
        seen[i + size] = 1;
        queue[tail++] = i + size;
      }
    }
    if (tail > largest[b]) largest[b] = tail;
  }

  const biomeCells: Record<string, number> = {};
  const largestRegionCells: Record<string, number> = {};
  for (let b = 0; b < BIOME_COUNT; b++) {
    const name = Biome[b];
    biomeCells[name] = counts[b];
    largestRegionCells[name] = largest[b];
  }

  let caveMinSpacingCells = 0;
  if (world.caveCount >= 2) {
    let best = Infinity;
    for (let a = 0; a < world.caveCount; a++) {
      for (let c = a + 1; c < world.caveCount; c++) {
        const d = Math.hypot(world.caveX[a] - world.caveX[c], world.caveZ[a] - world.caveZ[c]);
        if (d < best) best = d;
      }
    }
    caveMinSpacingCells = Math.round(best * 1000) / 1000;
  }

  return {
    cells,
    biomeCells,
    largestRegionCells,
    waterfallCount: world.waterfallCount,
    caveCount: world.caveCount,
    caveMinSpacingCells,
  };
}

/**
 * Every way a composition falls outside the bands, as readable sentences; empty when it is inside.
 * `size` is the world's edge length in cells, used for the size-relative bands.
 */
export function compositionViolations(
  c: WorldComposition,
  size: number,
  bands: CompositionBands = COMPOSITION_BANDS,
): string[] {
  const out: string[] = [];
  const ocean = (c.biomeCells[Biome[Biome.Ocean]] ?? 0) / c.cells;
  const [oLo, oHi] = bands.oceanFraction;
  if (ocean < oLo || ocean > oHi) {
    out.push(`ocean fraction ${ocean.toFixed(3)} is outside [${oLo}, ${oHi}]`);
  }
  const minRegion = Math.ceil(bands.minLargestRegionFraction * c.cells);
  for (const b of REGION_BIOMES) {
    const name = Biome[b];
    const got = c.largestRegionCells[name] ?? 0;
    if (got < minRegion) {
      out.push(`${name}: largest region ${got} cells, below the floor of ${minRegion}`);
    }
  }
  const [wLo, wHi] = bands.waterfalls;
  if (c.waterfallCount < wLo || c.waterfallCount > wHi) {
    out.push(`${c.waterfallCount} waterfalls, outside [${wLo}, ${wHi}]`);
  }
  const [cLo, cHi] = bands.caves;
  if (c.caveCount < cLo || c.caveCount > cHi) {
    out.push(`${c.caveCount} cave mouths, outside [${cLo}, ${cHi}]`);
  }
  const minSpacing = bands.minCaveSpacingFraction * size;
  if (c.caveCount >= 2 && c.caveMinSpacingCells < minSpacing) {
    out.push(
      `two cave mouths ${c.caveMinSpacingCells.toFixed(1)} cells apart, below ${minSpacing.toFixed(1)}`,
    );
  }
  return out;
}
