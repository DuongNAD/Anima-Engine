// @ts-expect-error -- Vite `?raw` import; this package's tsconfig has no vite/client types.
import floraPlaceRs from '../../../../src-tauri/crates/anima-worldgen/src/bake/flora_place.rs?raw';
import { describe, expect, it } from 'vitest';
import {
  BAKE_SPECIES,
  BEACH_BARE_M,
  CONIFER_MIN_ELEV_M,
  LOOK_SPECIES,
  TREELINE_M,
  chooseLookShape,
  chooseUnderstoryShape,
  type FloraHabitat,
} from './floraLook';

// prototype, to be replaced — WV2-LOOK r9 habitat rules for decorative flora.

const DRAWS = Array.from({ length: 200 }, (_, i) => (i + 0.5) / 200);
const ALL_BAKE = Object.values(BAKE_SPECIES);
const BIOMES = [3, 4, 5, 6, 7, 8, 10, 15, 16, 17, 18, 19, 21];

function shapesFor(env: FloraHabitat, bakeIds: readonly number[] = ALL_BAKE): Set<number> {
  const out = new Set<number>();
  for (const id of bakeIds) for (const r of DRAWS) out.add(chooseLookShape(id, env, r));
  return out;
}

describe('flora look: bake species ids', () => {
  it('match FloraSpeciesId in bake/flora_place.rs', () => {
    const rs = String(floraPlaceRs);
    const body = /pub enum FloraSpeciesId \{([\s\S]*?)\}/.exec(rs)?.[1] ?? '';
    const parsed = new Map<string, number>();
    for (const m of body.matchAll(/(\w+)\s*=\s*(\d+)/g)) parsed.set(m[1], Number(m[2]));
    expect(parsed.size).toBe(8);
    for (const [name, id] of Object.entries(BAKE_SPECIES)) expect(parsed.get(name)).toBe(id);
  });
});

describe('flora look: habitat rules', () => {
  it('puts no conifer below the montane belt, whatever the archetype', () => {
    for (const biome of BIOMES) {
      for (const elevM of [2, 50, 300, CONIFER_MIN_ELEV_M - 1]) {
        for (const distSeaM of [80, 400, 5000]) {
          expect(shapesFor({ elevM, distSeaM, slope: 0.05, biome }).has(LOOK_SPECIES.Pine)).toBe(false);
        }
      }
    }
  });

  it('keeps the beach bare of woody plants except mangroves on the tidal flats', () => {
    for (const biome of BIOMES) {
      const got = shapesFor(
        { elevM: 1, distSeaM: BEACH_BARE_M - 5, slope: 0.02, biome },
        ALL_BAKE.filter((id) => id !== BAKE_SPECIES.MangroveProp),
      );
      expect([...got]).toEqual([-1]);
    }
    const mangrove = shapesFor({ elevM: 0.5, distSeaM: 10, slope: 0.02, biome: 15 }, [BAKE_SPECIES.MangroveProp]);
    expect(mangrove.has(LOOK_SPECIES.Mangrove)).toBe(true);
  });

  it('draws mangroves only for the mangrove archetype', () => {
    for (const elevM of [0.5, 5, 400]) {
      for (const distSeaM of [60, 150, 900]) {
        const others = ALL_BAKE.filter((id) => id !== BAKE_SPECIES.MangroveProp);
        expect(shapesFor({ elevM, distSeaM, slope: 0.05, biome: 7 }, others).has(LOOK_SPECIES.Mangrove)).toBe(false);
      }
    }
  });

  it('puts palms and strand scrub, not the inland forest, in the salt-spray zone', () => {
    const got = shapesFor({ elevM: 6, distSeaM: 120, slope: 0.05, biome: 7 }, [BAKE_SPECIES.RainforestCanopy]);
    expect(got.has(LOOK_SPECIES.Palm)).toBe(true);
    expect(got.has(LOOK_SPECIES.Bush)).toBe(true);
    expect(got.has(LOOK_SPECIES.Jungle)).toBe(false);
    expect(got.has(LOOK_SPECIES.Pine)).toBe(false);
  });

  it('grows no trees above the treeline', () => {
    const trees = [
      LOOK_SPECIES.Pine,
      LOOK_SPECIES.Broadleaf,
      LOOK_SPECIES.Jungle,
      LOOK_SPECIES.Acacia,
      LOOK_SPECIES.Palm,
      LOOK_SPECIES.Mangrove,
      LOOK_SPECIES.Willow,
      LOOK_SPECIES.TreeFern,
    ];
    const got = shapesFor({ elevM: TREELINE_M + 200, distSeaM: 20000, slope: 0.1, biome: 18 });
    for (const t of trees) expect(got.has(t)).toBe(false);
    expect(got.has(LOOK_SPECIES.Rosette)).toBe(true);
  });

  it('mixes several plants inside one archetype (no monoculture)', () => {
    const got = shapesFor({ elevM: 250, distSeaM: 8000, slope: 0.05, biome: 7 }, [BAKE_SPECIES.RainforestCanopy]);
    expect(got.size).toBeGreaterThanOrEqual(4);
  });

  it('gives forest stands an understory and the strand none', () => {
    const inland: FloraHabitat = { elevM: 300, distSeaM: 6000, slope: 0.05, biome: 7 };
    const under = new Set(DRAWS.map((r) => chooseUnderstoryShape(BAKE_SPECIES.RainforestCanopy, inland, r)));
    expect(under.has(LOOK_SPECIES.TreeFern)).toBe(true);
    const strand: FloraHabitat = { elevM: 4, distSeaM: 100, slope: 0.05, biome: 7 };
    expect(new Set(DRAWS.map((r) => chooseUnderstoryShape(BAKE_SPECIES.RainforestCanopy, strand, r)))).toEqual(
      new Set([-1]),
    );
  });
});
