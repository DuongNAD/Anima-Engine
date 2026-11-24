import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
  generateTerrain,
  generateTerrainAsync,
  clearTerrainCache,
  TerrainData,
} from '../../src/components/Landscape/utils/terrainGenerator';
import {
  serializeChunk,
  deserializeChunk,
  TERRAIN_SCHEMA_VERSION,
  BIOME_LIST,
  FLORA_TYPE_LIST,
} from '../../src/components/Landscape/utils/terrainSerializer';
import { terrainCacheDB } from '../../src/components/Landscape/utils/terrainCacheDB';

// Polyfill IndexedDB for Vitest / JSDOM environment if not natively present
if (typeof globalThis !== 'undefined' && !(globalThis as any).indexedDB) {
  class MockIDBIndex {
    constructor(private name: string, private keyPath: string, private store: MockIDBObjectStore) {}

    getAllKeys(query?: any) {
      const keys: any[] = [];
      for (const [key, val] of this.store.data.entries()) {
        if (val && val[this.keyPath] === query) {
          keys.push(key);
        }
      }
      const req: any = { result: keys, onsuccess: null, onerror: null };
      setTimeout(() => req.onsuccess?.(), 0);
      return req;
    }

    openCursor() {
      const entries = Array.from(this.store.data.entries()).sort(
        (a, b) => (a[1][this.keyPath] || 0) - (b[1][this.keyPath] || 0)
      );
      let idx = 0;
      const store = this.store;
      const req: any = { result: null, onsuccess: null, onerror: null };

      const makeCursor = (): any => {
        if (idx >= entries.length) return null;
        const [k, v] = entries[idx];
        return {
          key: v[this.keyPath],
          primaryKey: k,
          value: v,
          delete: () => {
            store.data.delete(k);
          },
          continue: () => {
            idx++;
            req.result = makeCursor();
            setTimeout(() => req.onsuccess?.(), 0);
          },
        };
      };

      setTimeout(() => {
        req.result = makeCursor();
        req.onsuccess?.();
      }, 0);
      return req;
    }
  }

  class MockIDBObjectStore {
    public data = new Map<string, any>();
    public indexes = new Map<string, MockIDBIndex>();

    constructor(public name: string, public keyPath: string) {}

    createIndex(indexName: string, keyPath: string, options?: any) {
      const idx = new MockIDBIndex(indexName, keyPath, this);
      this.indexes.set(indexName, idx);
      return idx;
    }

    index(indexName: string) {
      return this.indexes.get(indexName)!;
    }

    get(key: string) {
      const req: any = { result: this.data.get(key), onsuccess: null, onerror: null };
      setTimeout(() => req.onsuccess?.(), 0);
      return req;
    }

    put(value: any) {
      const key = value[this.keyPath] ?? value.key;
      this.data.set(key, value);
      const req: any = { result: key, onsuccess: null, onerror: null };
      setTimeout(() => req.onsuccess?.(), 0);
      return req;
    }

    delete(key: string) {
      this.data.delete(key);
      const req: any = { result: undefined, onsuccess: null, onerror: null };
      setTimeout(() => req.onsuccess?.(), 0);
      return req;
    }

    clear() {
      this.data.clear();
      const req: any = { result: undefined, onsuccess: null, onerror: null };
      setTimeout(() => req.onsuccess?.(), 0);
      return req;
    }

    count() {
      const req: any = { result: this.data.size, onsuccess: null, onerror: null };
      setTimeout(() => req.onsuccess?.(), 0);
      return req;
    }
  }

  class MockIDBTransaction {
    public oncomplete: (() => void) | null = null;
    public onerror: ((err: any) => void) | null = null;
    public error: any = null;

    constructor(private stores: Map<string, MockIDBObjectStore>, storeNames: string[]) {
      setTimeout(() => this.oncomplete?.(), 5);
    }

    objectStore(name: string) {
      return this.stores.get(name)!;
    }
  }

  class MockIDBDatabase {
    public objectStoreNames = {
      contains: (name: string) => this.stores.has(name),
    };
    public stores = new Map<string, MockIDBObjectStore>();

    createObjectStore(name: string, options: { keyPath: string }) {
      const store = new MockIDBObjectStore(name, options.keyPath);
      this.stores.set(name, store);
      return store;
    }

    transaction(storeNames: string | string[], mode?: string) {
      const names = Array.isArray(storeNames) ? storeNames : [storeNames];
      return new MockIDBTransaction(this.stores, names);
    }
  }

  class MockIDBFactory {
    private dbs = new Map<string, MockIDBDatabase>();

    open(name: string, version: number) {
      let db = this.dbs.get(name);
      const isNew = !db;
      if (!db) {
        db = new MockIDBDatabase();
        this.dbs.set(name, db);
      }
      const req: any = {
        result: db,
        onsuccess: null,
        onerror: null,
        onupgradeneeded: null,
      };
      setTimeout(() => {
        if (isNew) {
          req.onupgradeneeded?.({ oldVersion: 0, newVersion: version, target: req });
        }
        req.onsuccess?.();
      }, 0);
      return req;
    }
  }

  (globalThis as any).indexedDB = new MockIDBFactory();
}

describe('Challenger 1 M3: Binary Serialization, IndexedDB & Seed Generation Empirical Suite', () => {
  beforeEach(async () => {
    clearTerrainCache();
    await terrainCacheDB.clearAllCache();
    vi.restoreAllMocks();
  });

  describe('Task 1.1: Binary Serialization & Exact Heightmap / Biome Roundtrip Equality', () => {
    it('asserts exact Float32 heightmap and Uint8 biome equality across serialization roundtrip', () => {
      const width = 60;
      const height = 60;
      const seed = 'm3-empirical-serialization-test';

      const originalTerrain = generateTerrain(width, height, seed);
      const serialized = serializeChunk(originalTerrain, 0, 0);

      // Verify TypedArrays in serialized chunk
      const rawElevationArr = new Float32Array(serialized.elevation);
      const rawBiomesArr = new Uint8Array(serialized.biomes);

      expect(rawElevationArr.length).toBe(width * height);
      expect(rawBiomesArr.length).toBe(width * height);

      const deserializedTerrain = deserializeChunk(serialized);

      // Re-extract Float32 and Uint8 arrays from deserialized terrain
      const size = width * height;
      const origElev = new Float32Array(size);
      const destElev = new Float32Array(size);
      const origBiomeIdx = new Uint8Array(size);
      const destBiomeIdx = new Uint8Array(size);

      const biomeMap: Record<string, number> = {};
      BIOME_LIST.forEach((b, idx) => { biomeMap[b] = idx; });

      let totalMismatchElev = 0;
      let totalMismatchBiome = 0;

      for (let y = 0; y < height; y++) {
        for (let x = 0; x < width; x++) {
          const idx = y * width + x;
          const cellOrig = originalTerrain.grid[y][x];
          const cellDest = deserializedTerrain.grid[y][x];

          origElev[idx] = cellOrig.elevation;
          destElev[idx] = cellDest.elevation;
          origBiomeIdx[idx] = biomeMap[cellOrig.biome] ?? 2;
          destBiomeIdx[idx] = biomeMap[cellDest.biome] ?? 2;

          // Exact Float32 equality
          if (origElev[idx] !== destElev[idx]) {
            totalMismatchElev++;
          }
          // Exact Uint8 biome index equality
          if (origBiomeIdx[idx] !== destBiomeIdx[idx]) {
            totalMismatchBiome++;
          }
        }
      }

      expect(totalMismatchElev).toBe(0);
      expect(totalMismatchBiome).toBe(0);

      // Assert bit-for-bit array buffer equality
      for (let i = 0; i < size; i++) {
        expect(destElev[i]).toBe(origElev[i]);
        expect(destBiomeIdx[i]).toBe(origBiomeIdx[i]);
      }
    });
  });

  describe('Task 1.2: Cache Store and Cache Purge API (clearTerrainCache)', () => {
    it('verifies cache store, hit, selective purge, and full purge', async () => {
      const seedA = 'cache-test-seed-alpha';
      const seedB = 'cache-test-seed-beta';

      // 1. Initial generation (Cache Miss)
      const terrainA1 = await generateTerrainAsync(40, 40, seedA);
      expect(terrainA1.cached).toBe(false);

      // Wait briefly for IndexedDB write
      await new Promise((r) => setTimeout(r, 20));

      // 2. Subsequent call (Cache Hit from Memory)
      const terrainA2 = await generateTerrainAsync(40, 40, seedA);
      expect(terrainA2.cached).toBe(true);

      // 3. Clear memory cache for seedA, test IndexedDB recovery
      clearTerrainCache(seedA);

      const terrainA3 = await generateTerrainAsync(40, 40, seedA);
      expect(terrainA3.cached).toBe(true);
      expect(terrainA3.grid[0][0].elevation).toBeCloseTo(terrainA1.grid[0][0].elevation, 4);

      // 4. Generate seedB
      const terrainB1 = await generateTerrainAsync(40, 40, seedB);
      expect(terrainB1.cached).toBe(false);

      // 5. Purge all cache via clearTerrainCache()
      clearTerrainCache();
      await terrainCacheDB.clearAllCache();

      // 6. Verify subsequent requests are fresh cache misses
      const terrainA4 = await generateTerrainAsync(40, 40, seedA);
      expect(terrainA4.cached).toBe(false);

      const terrainB2 = await generateTerrainAsync(40, 40, seedB);
      expect(terrainB2.cached).toBe(false);
    });
  });

  describe('Task 1.3: Seed Regeneration & Map Determinism / Distinctness', () => {
    it('verifies seed determinism (same seed = identical map) and distinctness (different seeds = different maps)', () => {
      const seed1 = 'm3-seed-1111';
      const seed2 = 'm3-seed-2222';
      const seed3 = 'm3-seed-3333';

      clearTerrainCache();

      const map1A = generateTerrain(50, 50, seed1);
      clearTerrainCache();
      const map1B = generateTerrain(50, 50, seed1);

      clearTerrainCache();
      const map2 = generateTerrain(50, 50, seed2);

      clearTerrainCache();
      const map3 = generateTerrain(50, 50, seed3);

      // Determinism test: map1A vs map1B
      let diff1A_1B = 0;
      for (let y = 0; y < 50; y++) {
        for (let x = 0; x < 50; x++) {
          if (map1A.grid[y][x].elevation !== map1B.grid[y][x].elevation) {
            diff1A_1B++;
          }
        }
      }
      expect(diff1A_1B).toBe(0);

      // Distinctness test: map1A vs map2, map1A vs map3
      let sumDiff1_2 = 0;
      let sumDiff1_3 = 0;
      let biomeDiffCount1_2 = 0;

      for (let y = 0; y < 50; y++) {
        for (let x = 0; x < 50; x++) {
          sumDiff1_2 += Math.abs(map1A.grid[y][x].elevation - map2.grid[y][x].elevation);
          sumDiff1_3 += Math.abs(map1A.grid[y][x].elevation - map3.grid[y][x].elevation);

          if (map1A.grid[y][x].biome !== map2.grid[y][x].biome) {
            biomeDiffCount1_2++;
          }
        }
      }

      const meanDiff1_2 = sumDiff1_2 / 2500;
      const meanDiff1_3 = sumDiff1_3 / 2500;

      expect(meanDiff1_2).toBeGreaterThan(5.0); // Significant elevation difference
      expect(meanDiff1_3).toBeGreaterThan(5.0);
      expect(biomeDiffCount1_2).toBeGreaterThan(100); // Significant biome difference
    });
  });

  describe('Task 1.4: Empirical Benchmarking Suite', () => {
    it('collects exact execution times for serialization, deserialization, generation, and cache access', async () => {
      const width = 50;
      const height = 50;
      const seed = 'benchmark-seed';

      // 1. Generation time (Cache miss)
      clearTerrainCache();
      const t0 = performance.now();
      const terrain = generateTerrain(width, height, seed);
      const tGen = performance.now() - t0;

      // 2. Serialization time
      const t1 = performance.now();
      const serialized = serializeChunk(terrain, 0, 0);
      const tSer = performance.now() - t1;

      // 3. Deserialization time
      const t2 = performance.now();
      const deserialized = deserializeChunk(serialized);
      const tDeser = performance.now() - t2;

      // 4. IndexedDB store time
      const t3 = performance.now();
      await terrainCacheDB.putChunk(seed, 0, 0, serialized);
      const tStore = performance.now() - t3;

      // 5. IndexedDB get time
      const t4 = performance.now();
      const retrieved = await terrainCacheDB.getChunk(seed, 0, 0);
      const tGet = performance.now() - t4;

      // 6. In-memory cache hit time
      const t5 = performance.now();
      const memHit = await generateTerrainAsync(width, height, seed);
      const tMemHit = performance.now() - t5;

      console.log('--- EMPIRICAL BENCHMARK DATA ---');
      console.log(`Terrain Generation (50x50): ${tGen.toFixed(3)} ms`);
      console.log(`Binary Serialization:        ${tSer.toFixed(3)} ms`);
      console.log(`Binary Deserialization:      ${tDeser.toFixed(3)} ms`);
      console.log(`IndexedDB Put Chunk:         ${tStore.toFixed(3)} ms`);
      console.log(`IndexedDB Get Chunk:         ${tGet.toFixed(3)} ms`);
      console.log(`In-Memory Cache Hit:         ${tMemHit.toFixed(3)} ms`);
      console.log('--------------------------------');

      expect(deserialized.grid.length).toBe(height);
      expect(retrieved).not.toBeNull();
      expect(memHit.cached).toBe(true);
      expect(tSer).toBeLessThan(10.0);
      expect(tDeser).toBeLessThan(10.0);
    });
  });
});
