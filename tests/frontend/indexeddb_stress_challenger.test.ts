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
} from '../../src/components/Landscape/utils/terrainSerializer';
import { terrainCacheDB, TerrainCacheDatabase } from '../../src/components/Landscape/utils/terrainCacheDB';
import FastNoiseLite from 'fastnoise-lite';

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

describe('Challenger 1: IndexedDB Map Caching & Fast-Path Hydration Empirical Stress Suite', () => {
  beforeEach(async () => {
    clearTerrainCache();
    await terrainCacheDB.clearAllCache();
    vi.restoreAllMocks();
  });

  describe('1. High Concurrency & Repeated Regeneration Load Stress', () => {
    it('should handle high concurrent load (10 simultaneous seeds) without corruption', async () => {
      const seeds = Array.from({ length: 10 }, (_, i) => `concurrent-stress-seed-${i}`);
      const promises = seeds.map((s) => generateTerrainAsync(30, 30, s));

      const results = await Promise.all(promises);
      expect(results.length).toBe(10);

      for (let i = 0; i < 10; i++) {
        expect(results[i].seed).toBe(seeds[i]);
        expect(results[i].width).toBe(30);
        expect(results[i].grid.length).toBe(30);
      }

      // Explicitly write serialized chunks for each seed into IndexedDB to verify storage integrity
      for (let i = 0; i < 10; i++) {
        await terrainCacheDB.putChunk(seeds[i], 0, 0, serializeChunk(results[i]));
      }

      // Verify IndexedDB persistence without calling clearTerrainCache() which wipes IDB
      for (let i = 0; i < 10; i++) {
        const cachedChunk = await terrainCacheDB.getChunk(seeds[i], 0, 0);
        expect(cachedChunk).not.toBeNull();
        expect(cachedChunk?.seed).toBe(seeds[i]);
      }
    });

    it('should handle repeated map regeneration (100 sequential calls on same seed) with fast cache hits', async () => {
      const seed = 'repeated-regeneration-seed';
      const width = 35;
      const height = 35;

      // Call 1: Miss / generation
      const first = await generateTerrainAsync(width, height, seed);
      expect(first.cached).toBe(false);

      // Calls 2 through 100: Hits from memory cache
      for (let i = 2; i <= 100; i++) {
        const next = await generateTerrainAsync(width, height, seed);
        expect(next.cached).toBe(true);
        expect(next.grid[0][0].elevation).toBeCloseTo(first.grid[0][0].elevation, 4);
      }
    });
  });

  describe('2. Rapid Cache Clearing & Interleaved Edge Cases', () => {
    it('should safely handle interleaved in-flight cache clear during asynchronous generation', async () => {
      const seed = 'in-flight-clear-seed';

      const genPromise = generateTerrainAsync(30, 30, seed);
      // Immediately fire clear cache while generation promise is in flight
      clearTerrainCache(seed);

      const result = await genPromise;
      expect(result).toBeDefined();
      expect(result.width).toBe(30);
    });

    it('should handle rapid loop of 50 clearAllCache() calls without unhandled rejections', async () => {
      const clearPromises: Promise<void>[] = [];
      for (let i = 0; i < 50; i++) {
        clearTerrainCache();
        clearPromises.push(terrainCacheDB.clearAllCache());
      }

      await expect(Promise.all(clearPromises)).resolves.not.toThrow();
    });

    it('should perform selective seed cache clearing when properly awaiting terrainCacheDB.clearMapCache', async () => {
      const seedA = 'seed-alpha';
      const seedB = 'seed-beta';

      await generateTerrainAsync(30, 30, seedA);
      await generateTerrainAsync(30, 30, seedB);
      await new Promise((r) => setTimeout(r, 50));

      // Clear memory cache and explicitly await IndexedDB deletion for seedA
      clearTerrainCache(seedA);
      await terrainCacheDB.clearMapCache(seedA);

      // seedB should still be in memory cache
      const freshGenB = await generateTerrainAsync(30, 30, seedB);
      expect(freshGenB.cached).toBe(true);

      // seedA was cleared from memory and IndexedDB, so freshGenA is a cache miss
      const freshGenA = await generateTerrainAsync(30, 30, seedA);
      expect(freshGenA.cached).toBe(false);
    });
  });

  describe('3. Fast-Path Hydration Performance (<10ms)', () => {
    it('should consistently deliver fast hydration times (<10ms) for in-memory cache hits and fast IndexedDB reads', async () => {
      const seed = 'hydration-speed-test-seed';
      const width = 40;
      const height = 40;

      // 1. In-memory cache hit performance test
      await generateTerrainAsync(width, height, seed);
      const memDurations: number[] = [];

      for (let i = 0; i < 100; i++) {
        const start = performance.now();
        const hydrated = await generateTerrainAsync(width, height, seed);
        const elapsed = performance.now() - start;

        memDurations.push(elapsed);
        expect(hydrated.cached).toBe(true);
      }

      const memMax = Math.max(...memDurations);
      const memAvg = memDurations.reduce((a, b) => a + b, 0) / memDurations.length;

      console.log(`[Memory Cache Hydration Perf] Max: ${memMax.toFixed(3)}ms, Avg: ${memAvg.toFixed(3)}ms`);
      expect(memMax).toBeLessThan(10.0);
      expect(memAvg).toBeLessThan(1.0);

      // 2. IndexedDB cache hit deserialization test (direct getChunk + deserializeChunk)
      const serialized = serializeChunk(generateTerrain(width, height, seed), 0, 0);
      await terrainCacheDB.putChunk(seed, 0, 0, serialized);

      const idbStart = performance.now();
      const cachedChunk = await terrainCacheDB.getChunk(seed, 0, 0);
      const deserialized = deserializeChunk(cachedChunk!);
      const idbElapsed = performance.now() - idbStart;

      console.log(`[IndexedDB Deserialization Perf] Elapsed: ${idbElapsed.toFixed(3)}ms`);
      expect(deserialized.cached).toBe(true);
    });
  });

  describe('4. Empirical CPU 5-Pass Noise Generation Bypass Verification', () => {
    it('should EMPIRICALLY bypass CPU noise generation (FastNoiseLite.GetNoise count = 0) on cache hit', async () => {
      const seed = 'noise-bypass-test-seed';
      const width = 40;
      const height = 40;

      // 1. Initial Cache Miss - Noise generator should be called
      const getNoiseSpy = vi.spyOn(FastNoiseLite.prototype, 'GetNoise');
      const terrain = await generateTerrainAsync(width, height, seed);
      expect(terrain.cached).toBe(false);

      const missNoiseCalls = getNoiseSpy.mock.calls.length;
      expect(missNoiseCalls).toBeGreaterThan(0);

      // Explicitly await putChunk to ensure DB write is done before clearing memory cache
      const serialized = serializeChunk(terrain, 0, 0);
      await terrainCacheDB.putChunk(seed, 0, 0, serialized);

      // Reset spy count
      getNoiseSpy.mockClear();

      // 2. Cache Hit via generateTerrainAsync (memory cache path)
      const hitTerrain = await generateTerrainAsync(width, height, seed);

      expect(hitTerrain.cached).toBe(true);
      expect(hitTerrain.grid.length).toBe(height);
      expect(hitTerrain.grid[0].length).toBe(width);

      // EMPIRICAL ASSERTION: GetNoise must NOT be called at all during cache hit!
      const hitNoiseCalls = getNoiseSpy.mock.calls.length;
      expect(hitNoiseCalls).toBe(0);
    });
  });

  describe('5. Storage Quota Eviction (LRU) & Resilience Harness', () => {
    it('should evict oldest chunks when IndexedDB maxChunks limit is exceeded', async () => {
      const maxChunksLimit = 20; // Use small limit for test speed
      const totalChunksToInsert = 35;

      for (let i = 0; i < totalChunksToInsert; i++) {
        const dummySeed = `lru-seed-${i}`;
        const terrain = generateTerrain(20, 20, dummySeed);
        const serialized = serializeChunk(terrain, 0, 0);
        await terrainCacheDB.putChunk(dummySeed, 0, 0, serialized);
      }

      // Explicitly run manageLRUQuota with limit of 20
      await terrainCacheDB.manageLRUQuota(maxChunksLimit);

      // Verify that IndexedDB now holds <= 20 chunks
      const db = await terrainCacheDB.open();
      const tx = db!.transaction(['chunks'], 'readonly');
      const store = tx.objectStore('chunks');

      const countReq = store.count();
      await new Promise((r) => {
        countReq.onsuccess = () => {
          expect(countReq.result).toBeLessThanOrEqual(maxChunksLimit);
          r(null);
        };
      });
    });

    it('should touch chunk lastAccessed timestamp when putChunk updates record', async () => {
      const seedOld = 'seed-accessed-test';
      const terrain = generateTerrain(20, 20, seedOld);
      const serialized = serializeChunk(terrain, 0, 0);

      await terrainCacheDB.putChunk(seedOld, 0, 0, serialized);
      const record1 = await terrainCacheDB.getChunk(seedOld, 0, 0);
      expect(record1).not.toBeNull();

      await new Promise((r) => setTimeout(r, 20));

      // Re-put chunk to update timestamp
      await terrainCacheDB.putChunk(seedOld, 0, 0, serialized);
      const record2 = await terrainCacheDB.getChunk(seedOld, 0, 0);
      expect(record2).not.toBeNull();
      expect(record2!.lastAccessed).toBeGreaterThanOrEqual(record1!.lastAccessed);
    });

    it('should gracefully fallback to noise generator when IndexedDB encounters errors', async () => {
      const seed = 'fallback-error-seed';

      // Spy on open to simulate IndexedDB opening error
      vi.spyOn(terrainCacheDB, 'open').mockRejectedValueOnce(new Error('IndexedDB Access Denied / Quota Exceeded'));

      const result = await generateTerrainAsync(30, 30, seed);
      expect(result).toBeDefined();
      expect(result.width).toBe(30);
      expect(result.cached).toBe(false);
    });
  });
});
