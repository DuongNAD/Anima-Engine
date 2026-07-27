import { describe, it, expect } from 'vitest';
import { biomeAt, findSpawn } from '../components/Landscape/utils/worldSample';
import type { BiomeSource, SpawnSource } from '../components/Landscape/utils/worldSample';
import { BIOME_COUNT, BIOME_NAMES_VI, BIOME_EMOJI, Biome } from '../components/Landscape/utils/worldGen';

/**
 * A tiny world whose only populated field is `biome` — which is all `biomeAt` reads.
 *
 * `BiomeSource`, not `World`: these fixtures never had the other twenty-odd fields and never needed
 * them, so the type they used to claim was one no value here satisfied.
 */
function biomeSource(size: number, biome?: Uint8Array): BiomeSource {
  return { size, biome };
}

/** No trees. `findSpawn` clears flora before it accepts a cell, so the fixtures must say so. */
const NO_FLORA = {
  floraCount: 0,
  floraX: new Float32Array(0),
  floraZ: new Float32Array(0),
  floraScale: new Float32Array(0),
  floraType: new Uint8Array(0),
} satisfies Omit<SpawnSource, 'size' | 'seaLevel'>;

describe('biome HUD tables', () => {
  it('has one name and one emoji per biome', () => {
    expect(BIOME_NAMES_VI.length).toBe(BIOME_COUNT);
    expect(BIOME_EMOJI.length).toBe(BIOME_COUNT);
    // every entry is a non-empty string
    expect(BIOME_NAMES_VI.every((n) => typeof n === 'string' && n.length > 0)).toBe(true);
    expect(BIOME_EMOJI.every((e) => typeof e === 'string' && e.length > 0)).toBe(true);
  });
});

describe('biomeAt', () => {
  const renderSize = 100; // terrain spans [-50, 50]

  it('reads the nearest cell for an in-bounds world position', () => {
    // 2x2 grid: [Ocean, Desert / Forest, Snow]
    const biome = new Uint8Array([Biome.Ocean, Biome.Desert, Biome.Forest, Biome.Snow]);
    const w = biomeSource(2, biome);
    // top-left corner (u,v≈0) -> cell (0,0) = Ocean
    expect(biomeAt(w, -50, -50, renderSize)).toBe(Biome.Ocean);
    // bottom-right corner (u,v≈1) -> cell (1,1) = Snow
    expect(biomeAt(w, 50, 50, renderSize)).toBe(Biome.Snow);
    // top-right -> cell (1,0) = Desert
    expect(biomeAt(w, 50, -50, renderSize)).toBe(Biome.Desert);
  });

  it('returns Ocean (0) off the map', () => {
    const biome = new Uint8Array([Biome.Forest, Biome.Forest, Biome.Forest, Biome.Forest]);
    const w = biomeSource(2, biome);
    expect(biomeAt(w, 9999, 0, renderSize)).toBe(Biome.Ocean);
    expect(biomeAt(w, 0, -9999, renderSize)).toBe(Biome.Ocean);
  });

  it('returns Ocean (0) when the biome field is missing or too small', () => {
    const tooSmall = biomeSource(4, new Uint8Array(2)); // length < size*size
    expect(biomeAt(tooSmall, 0, 0, renderSize)).toBe(Biome.Ocean);
    // The other half of the guard, reachable now that the parameter admits a world without one.
    expect(biomeAt(biomeSource(4), 0, 0, renderSize)).toBe(Biome.Ocean);
  });
});

describe('findSpawn', () => {
  const renderSize = 100;

  it('picks a dry land cell, not open ocean', () => {
    const size = 4;
    const n = size * size;
    const biome = new Uint8Array(n).fill(Biome.Ocean);
    const elevation = new Float32Array(n).fill(0.1); // below sea
    const slope = new Float32Array(n).fill(0);
    const shore = new Float32Array(n).fill(0);
    // one inviting land cell at (gx=2, gy=1)
    const land = 1 * size + 2;
    biome[land] = Biome.Grassland;
    elevation[land] = 0.6;
    const w: SpawnSource = {
      ...NO_FLORA,
      size,
      biome,
      elevation,
      slope,
      shore,
      seaLevel: 0.3,
    };

    const s = findSpawn(w, renderSize);
    // The chosen spot must be on land (the only non-ocean, above-sea cell).
    expect(biomeAt(w, s.x, s.z, renderSize)).toBe(Biome.Grassland);
    expect(s.x).toBeCloseTo((2 / 3 - 0.5) * renderSize, 5);
    expect(s.z).toBeCloseTo((1 / 3 - 0.5) * renderSize, 5);
  });

  it('falls back to the origin for an all-ocean world', () => {
    const size = 3;
    const n = size * size;
    const w: SpawnSource = {
      ...NO_FLORA,
      size,
      biome: new Uint8Array(n).fill(Biome.Ocean),
      elevation: new Float32Array(n).fill(0.05),
      slope: new Float32Array(n),
      shore: new Float32Array(n),
      seaLevel: 0.3,
    };
    expect(findSpawn(w, renderSize)).toEqual({ x: 0, z: 0 });
  });
});
