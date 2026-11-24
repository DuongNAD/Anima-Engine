import { describe, it, expect, beforeEach } from 'vitest';
import React from 'react';
import { render, fireEvent } from '@testing-library/react';
import {
  generateTerrain,
  generateTerrainAsync,
  clearTerrainCache,
} from '../../src/components/Landscape/utils/terrainGenerator';
import {
  serializeChunk,
  deserializeChunk,
} from '../../src/components/Landscape/utils/terrainSerializer';
import { terrainCacheDB } from '../../src/components/Landscape/utils/terrainCacheDB';
import { LandscapeControlsOverlay } from '../../src/components/Landscape/LandscapeControlsOverlay';

describe('Map Caching & UI Performance E2E Suite (M3)', () => {
  beforeEach(() => {
    clearTerrainCache();
  });

  describe('Tier 1: Feature Coverage', () => {
    it('T1.1: verify IndexedDB binary serialization', () => {
      const terrain = generateTerrain(32, 32, 'serialize_seed');
      const serialized = serializeChunk(terrain, 0, 0);

      expect(serialized.width).toBe(32);
      expect(serialized.height).toBe(32);
      expect(serialized.elevation instanceof ArrayBuffer).toBe(true);
      expect(serialized.biomes instanceof ArrayBuffer).toBe(true);
      expect(serialized.moisture instanceof ArrayBuffer).toBe(true);
      expect(serialized.temperature instanceof ArrayBuffer).toBe(true);
      expect(serialized.hydroFlags instanceof ArrayBuffer).toBe(true);
      expect(serialized.flora.types instanceof ArrayBuffer).toBe(true);
      expect(serialized.flora.transforms instanceof ArrayBuffer).toBe(true);
    });

    it('T1.2: verify IndexedDB binary deserialization', () => {
      const original = generateTerrain(32, 32, 'deserialize_seed');
      const serialized = serializeChunk(original, 0, 0);
      const deserialized = deserializeChunk(serialized);

      expect(deserialized.width).toBe(original.width);
      expect(deserialized.height).toBe(original.height);
      expect(deserialized.cached).toBe(true);
      expect(deserialized.grid[0][0].elevation).toBeCloseTo(original.grid[0][0].elevation, 4);
      expect(deserialized.grid[0][0].biome).toBe(original.grid[0][0].biome);
      expect(deserialized.flora.length).toBe(original.flora.length);
    });

    it('T1.3: verify cache hit latency fast-path in generateTerrainAsync', async () => {
      const seed = 'fastpath_seed';
      const width = 32;
      const height = 32;

      // First call generates & caches in memory
      const t1 = await generateTerrainAsync(width, height, seed);
      expect(t1.cached).toBe(false);

      // Second call hits fast-path cache
      const start = performance.now();
      const t2 = await generateTerrainAsync(width, height, seed);
      const elapsed = performance.now() - start;

      expect(t2.cached).toBe(true);
      expect(elapsed).toBeLessThan(50); // Fast path returns < 50ms
    });

    it('T1.4: verify clear terrain cache state changes', async () => {
      const seed = 'clear_cache_seed';
      const width = 32;
      const height = 32;

      await generateTerrainAsync(width, height, seed);
      clearTerrainCache(seed);

      // Subsequent call after clear must be cache miss
      const fresh = generateTerrain(width, height, seed);
      expect(fresh.cached).toBe(false);
    });

    it('T1.5: verify UI cache control state changes in LandscapeControlsOverlay', () => {
      let regenerated = false;
      const { container, getByText } = render(
        <LandscapeControlsOverlay
          weather="clear"
          onWeatherChange={() => {}}
          speed={1.0}
          onSpeedChange={() => {}}
          volume={1.0}
          onVolumeChange={() => {}}
          isMuted={false}
          onMuteToggle={() => {}}
          cameraMode="orbit"
          onCameraModeChange={() => {}}
          timeOfDay={12}
          onRegenerateCache={() => {
            regenerated = true;
          }}
          cacheStatus="Cached (IndexedDB)"
        />
      );

      expect(container.textContent).toContain('Cache: Cached (IndexedDB)');

      const regenButton = getByText(/Regen/i) || getByText(/Clear/i);
      expect(regenButton).toBeTruthy();

      fireEvent.click(regenButton);
      expect(regenerated).toBe(true);
    });
  });

  describe('Tier 2: Boundary & Corner Cases', () => {
    it('T2.1: special character and extreme seeds ("!@#$", "", 0, 2147483647)', () => {
      const seeds = ['!@#$', '', 0, 2147483647];
      seeds.forEach((seed) => {
        const terrain = generateTerrain(16, 16, seed);
        const serialized = serializeChunk(terrain, 0, 0);
        const deserialized = deserializeChunk(serialized);
        expect(deserialized.width).toBe(16);
      });
    });

    it('T2.2: serializing terrain with 0 flora items', () => {
      const terrain = generateTerrain(16, 16, 'zero_flora');
      terrain.flora = []; // Force zero flora

      const serialized = serializeChunk(terrain, 0, 0);
      const deserialized = deserializeChunk(serialized);

      expect(deserialized.flora.length).toBe(0);
    });

    it('T2.3: deserialization when waterY ArrayBuffer is omitted', () => {
      const terrain = generateTerrain(16, 16, 'no_water_y');
      const serialized = serializeChunk(terrain, 0, 0);
      delete (serialized as any).waterY;

      const deserialized = deserializeChunk(serialized);
      expect(deserialized.grid[0][0].waterY).toBeUndefined();
    });

    it('T2.4: multiple sequential clearTerrainCache calls', () => {
      expect(() => {
        clearTerrainCache();
        clearTerrainCache();
        clearTerrainCache('some_seed');
      }).not.toThrow();
    });

    it('T2.5: concurrent generateTerrainAsync calls return consistent data', async () => {
      const seed = 'concurrent_seed';
      const [t1, t2] = await Promise.all([
        generateTerrainAsync(32, 32, seed),
        generateTerrainAsync(32, 32, seed),
      ]);

      expect(t1.grid[0][0].elevation).toBeCloseTo(t2.grid[0][0].elevation, 4);
    });
  });

  describe('Tier 3: Cross-Feature Combinations', () => {
    it('T3.1: serialization + cache purge + re-generation sequence', () => {
      const terrain1 = generateTerrain(32, 32, 'cross_flow');
      const serialized = serializeChunk(terrain1, 0, 0);

      clearTerrainCache('cross_flow');

      const deserialized = deserializeChunk(serialized);
      expect(deserialized.grid[5][5].elevation).toBeCloseTo(terrain1.grid[5][5].elevation, 4);
    });
  });

  describe('Tier 4: Real-World Application Scenarios', () => {
    it('T4.1: full engine initialization, map regeneration, cache clearance, and state re-hydration sequence', async () => {
      // Step 1: Initial map generation
      const terrainInitial = await generateTerrainAsync(32, 32, 'user_session_1');
      expect(terrainInitial.grid.length).toBe(32);

      // Step 2: Cache clearance trigger
      clearTerrainCache('user_session_1');

      // Step 3: Regenerate terrain with new seed
      const terrainRegen = await generateTerrainAsync(32, 32, 'user_session_2');
      expect(terrainRegen.grid.length).toBe(32);

      // Step 4: Verify state re-hydration
      expect(terrainRegen.seed).toBe('user_session_2');
    });
  });
});
