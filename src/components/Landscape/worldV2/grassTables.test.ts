// prototype, to be replaced — unit tests for WV2-LOOK r12 biome grass tables, mask, and GLSL chunks.
import { describe, expect, it } from 'vitest';
import type { World } from '../utils/worldGen';
import { WaterKind, type WorldV2LoadedBundle } from './types';
import {
  BIOME_GRASS_COVER,
  BIOME_GRASS_DRY,
  BIOME_GRASS_HEIGHT,
  BIOME_HERB_SUITABILITY,
  GRASS_FIELD_GLSL,
  buildWorldV2GrassMask,
} from './grassTables';

function makeDummyBundle(size = 8): WorldV2LoadedBundle {
  const n = size * size;
  const biomeIdsRgba = new Uint8Array(n * 4);
  const biomeWeightsRgba = new Uint8Array(n * 4);
  const heightM = new Float32Array(n).fill(180);
  const normalRg = new Uint8Array(n * 2).fill(128);
  const shoreSdfRg = new Uint8Array(n * 2);
  const waterKind = new Uint8Array(n);
  for (let i = 0; i < n; i += 1) {
    // Default inland Grassland (biome 4), gentle slope, 500 m inland
    biomeIdsRgba[i * 4] = 4;
    biomeWeightsRgba[i * 4] = 255;
    shoreSdfRg[i * 2] = 255;
  }
  return {
    extentM: 81920,
    elevSpanM: 7200,
    elevMinM: -1500,
    size,
    mip: 1,
    seed: 1,
    biomeIdsRgba,
    biomeWeightsRgba,
    heightM,
    normalRg,
    shoreSdfRg,
    waterKind,
    waterHeightM: new Float32Array(n),
    colorRgba: new Uint8Array(n * 4),
    lightingRgba: new Uint8Array(n * 4),
    horizonRgba: new Uint8Array(n * 4),
    hydroFlowRgba: new Uint8Array(n * 4),
    flora: [],
    caveNetwork: null,
  } as unknown as WorldV2LoadedBundle;
}

describe('grassTables WV2-LOOK r12', () => {
  it('keeps cover strictly 0 on bare/aquatic biomes and normalized elsewhere', () => {
    for (const bare of [0, 1, 2, 11, 12, 13, 14, 15, 19, 20]) {
      expect(BIOME_GRASS_COVER[bare]).toBe(0);
    }
    expect(BIOME_GRASS_COVER[4]).toBe(1.0);
    expect(BIOME_GRASS_HEIGHT[3]).toBeGreaterThan(BIOME_GRASS_HEIGHT[4]);
    expect(BIOME_GRASS_DRY[3]).toBeGreaterThan(BIOME_GRASS_DRY[4]);
    expect(BIOME_HERB_SUITABILITY[4]).toBe(1.0);
  });

  it('zeros cover on water and steep slopes while keeping inland grassland lush', () => {
    const bundle = makeDummyBundle(8);
    // Mark first texel as River water, last texel as steep Rock cliff
    bundle.waterKind[0] = WaterKind.River;
    bundle.biomeIdsRgba[63 * 4] = 11;
    const world = {
      size: 8,
      slope: new Float32Array(64).fill(0.02),
      water: new Float32Array(64),
      riverAmt: new Uint8Array(64),
    } as unknown as World;
    world.slope[1] = 0.45; // steep grassland slope

    const tex = buildWorldV2GrassMask(bundle, world);
    const data = tex.image.data as Uint8Array;
    const px = (sx: number, sy: number) => {
      const mx = Math.round((sx / 7) * 511);
      const my = Math.round((sy / 7) * 511);
      const o = (my * 512 + mx) * 4;
      return [data[o], data[o + 1], data[o + 2], data[o + 3]];
    };
    expect(px(0, 0)[0]).toBe(0); // water
    expect(px(1, 0)[0]).toBe(0); // steep slope
    expect(px(7, 7)[0]).toBe(0); // bare rock biome
    const [cov, h, dry, herb] = px(3, 3);
    expect(cov).toBeGreaterThan(240);
    expect(h).toBeGreaterThan(120);
    expect(dry).toBeLessThan(90);
    expect(herb).toBeGreaterThan(240);
  });

  it('exports Voronoi clumping, wind gust, and shared albedo GLSL helpers', () => {
    expect(GRASS_FIELD_GLSL).toContain('lkGrassClump');
    expect(GRASS_FIELD_GLSL).toContain('lkGrassWind');
    expect(GRASS_FIELD_GLSL).toContain('lkGrassAlbedo');
  });
});
