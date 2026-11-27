// prototype, to be replaced — unit tests for L0HeightTileCache bilinear/cross-tile & LRU behavior.

import { describe, expect, it } from 'vitest';
import { L0HeightTileCache } from './l0HeightCache';
import { TILE_ACTIVE_SIZE, type L0HeightManifest, type TileManifestEntry } from './types';

function makeManifest(tilesPerSide = 2): L0HeightManifest {
  const entries = new Map<string, TileManifestEntry>();
  for (let ty = 0; ty < tilesPerSide; ty += 1) {
    for (let tx = 0; tx < tilesPerSide; tx += 1) {
      entries.set(`${tx}:${ty}`, {
        layer: 'height',
        mip: 0,
        tx,
        ty,
        path: `tiles/height/L0/t_${ty}_${tx}.ktx2`,
        byte_len: 10,
        sha256: '00'.repeat(32),
      });
    }
  }
  return {
    tilesBaseUrl: '/world-v2-big',
    gridSize: tilesPerSide * TILE_ACTIVE_SIZE,
    tilesPerSide,
    entries,
  };
}

describe('L0HeightTileCache', () => {
  it('interpolates bilinearly within and across tile boundaries and evicts LRU entries', () => {
    const cache = new L0HeightTileCache(makeManifest(2), { maxTiles: 4 });
    const t00 = new Float32Array(TILE_ACTIVE_SIZE * TILE_ACTIVE_SIZE);
    const t10 = new Float32Array(TILE_ACTIVE_SIZE * TILE_ACTIVE_SIZE);
    const t01 = new Float32Array(TILE_ACTIVE_SIZE * TILE_ACTIVE_SIZE);
    const t11 = new Float32Array(TILE_ACTIVE_SIZE * TILE_ACTIVE_SIZE);
    for (let y = 0; y < TILE_ACTIVE_SIZE; y += 1) {
      for (let x = 0; x < TILE_ACTIVE_SIZE; x += 1) {
        t00[y * TILE_ACTIVE_SIZE + x] = x * 2 + y * 4;
        t10[y * TILE_ACTIVE_SIZE + x] = (TILE_ACTIVE_SIZE + x) * 2 + y * 4;
        t01[y * TILE_ACTIVE_SIZE + x] = x * 2 + (TILE_ACTIVE_SIZE + y) * 4;
        t11[y * TILE_ACTIVE_SIZE + x] = (TILE_ACTIVE_SIZE + x) * 2 + (TILE_ACTIVE_SIZE + y) * 4;
      }
    }
    cache.setDecodedTile(0, 0, t00);
    cache.setDecodedTile(1, 0, t10);
    cache.setDecodedTile(0, 1, t01);
    expect(cache.sampleHeightM(0.5, 0.5)).toBeNull();
    cache.setDecodedTile(1, 1, t11);

    // Sample right across the tile seam (255 -> 256 in both axes)
    const span = 2 * TILE_ACTIVE_SIZE - 1;
    const hSeam = cache.sampleHeightM(255.5 / span, 255.5 / span);
    expect(hSeam).toBeCloseTo(255.5 * 2 + 255.5 * 4, 3);
    expect(cache.isRegionReady(0.4, 0.4, 0.6, 0.6)).toBe(true);

    // LRU cap evicts the oldest untouched tile when inserting a 5th key
    const small = new L0HeightTileCache(makeManifest(3), { maxTiles: 4 });
    for (let k = 0; k < 5; k += 1) {
      small.setDecodedTile(k % 3, Math.floor(k / 3), new Float32Array(TILE_ACTIVE_SIZE * TILE_ACTIVE_SIZE));
    }
    expect(small.tileCount).toBe(4);
    expect(small.sampleHeightM(0, 0)).toBeNull();
  });
});
