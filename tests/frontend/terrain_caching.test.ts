import { describe, it, expect, beforeEach, vi } from 'vitest';
import { generateTerrain, generateTerrainAsync, clearTerrainCache, TerrainData } from '../../src/components/Landscape/utils/terrainGenerator';
import { serializeChunk, deserializeChunk, TERRAIN_SCHEMA_VERSION } from '../../src/components/Landscape/utils/terrainSerializer';
import { terrainCacheDB, TerrainCacheDatabase } from '../../src/components/Landscape/utils/terrainCacheDB';

// Polyfill IndexedDB for JSDOM / Node environment if not present
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

describe('Milestone 1: Map Caching & Persistence System (IndexedDB)', () => {
  beforeEach(async () => {
    clearTerrainCache();
    await terrainCacheDB.clearAllCache();
  });

  describe('1. Binary Serialization & Deserialization (terrainSerializer)', () => {
    it('should convert TerrainData to SerializedTerrainChunk and back losslessly', () => {
      const terrain = generateTerrain(50, 50, 'test-seed-123');
      const serialized = serializeChunk(terrain, 0, 0);

      expect(serialized.version).toBe(TERRAIN_SCHEMA_VERSION);
      expect(serialized.width).toBe(50);
      expect(serialized.height).toBe(50);
      expect(serialized.elevation).toBeInstanceOf(ArrayBuffer);
      expect(serialized.moisture).toBeInstanceOf(ArrayBuffer);
      expect(serialized.temperature).toBeInstanceOf(ArrayBuffer);
      expect(serialized.biomes).toBeInstanceOf(ArrayBuffer);
      expect(serialized.hydroFlags).toBeInstanceOf(ArrayBuffer);

      const deserialized = deserializeChunk(serialized);

      expect(deserialized.width).toBe(terrain.width);
      expect(deserialized.height).toBe(terrain.height);
      expect(deserialized.seed).toBe(terrain.seed);
      expect(deserialized.cached).toBe(true);

      // Verify cell-by-cell data preservation
      for (let y = 0; y < terrain.height; y++) {
        for (let x = 0; x < terrain.width; x++) {
          const orig = terrain.grid[y][x];
          const dest = deserialized.grid[y][x];

          expect(dest.x).toBe(orig.x);
          expect(dest.y).toBe(orig.y);
          expect(dest.elevation).toBeCloseTo(orig.elevation, 4);
          expect(dest.moisture).toBeCloseTo(orig.moisture, 0);
          expect(dest.temperature).toBeCloseTo(orig.temperature, 4);
          expect(dest.biome).toBe(orig.biome);

          const origRiver = orig.isRiver ? (typeof orig.isRiver === 'number' ? orig.isRiver : true) : false;
          const destRiver = dest.isRiver ? (typeof dest.isRiver === 'number' ? dest.isRiver : true) : false;
          expect(destRiver).toEqual(origRiver);
          expect(dest.isWaterfall).toBe(orig.isWaterfall);
          expect(dest.isLake).toBe(orig.isLake);
        }
      }

      // Verify flora placements preservation
      expect(deserialized.flora.length).toBe(terrain.flora.length);
      for (let i = 0; i < terrain.flora.length; i++) {
        expect(deserialized.flora[i].type).toBe(terrain.flora[i].type);
        expect(deserialized.flora[i].x).toBeCloseTo(terrain.flora[i].x, 3);
        expect(deserialized.flora[i].y).toBeCloseTo(terrain.flora[i].y, 3);
        expect(deserialized.flora[i].scale).toBeCloseTo(terrain.flora[i].scale, 3);
      }
    });
  });

  describe('2. IndexedDB Storage Operations (terrainCacheDB)', () => {
    it('should open database, put chunk, get chunk, and clear cache', async () => {
      const db = await terrainCacheDB.open();
      expect(db).not.toBeNull();

      const terrain = generateTerrain(40, 40, 'db-seed-1');
      const serialized = serializeChunk(terrain, 0, 0);

      await terrainCacheDB.putChunk('db-seed-1', 0, 0, serialized);

      const retrieved = await terrainCacheDB.getChunk('db-seed-1', 0, 0);
      expect(retrieved).not.toBeNull();
      expect(retrieved?.seed).toBe('db-seed-1');
      expect(retrieved?.width).toBe(40);

      // Test clearMapCache
      await terrainCacheDB.clearMapCache('db-seed-1');
      const clearedRetrieved = await terrainCacheDB.getChunk('db-seed-1', 0, 0);
      expect(clearedRetrieved).toBeNull();
    });

    it('should clear all cache entries with clearAllCache()', async () => {
      const terrain1 = generateTerrain(30, 30, 'seed-a');
      const terrain2 = generateTerrain(30, 30, 'seed-b');

      await terrainCacheDB.putChunk('seed-a', 0, 0, serializeChunk(terrain1));
      await terrainCacheDB.putChunk('seed-b', 0, 0, serializeChunk(terrain2));

      expect(await terrainCacheDB.getChunk('seed-a', 0, 0)).not.toBeNull();
      expect(await terrainCacheDB.getChunk('seed-b', 0, 0)).not.toBeNull();

      await terrainCacheDB.clearAllCache();

      expect(await terrainCacheDB.getChunk('seed-a', 0, 0)).toBeNull();
      expect(await terrainCacheDB.getChunk('seed-b', 0, 0)).toBeNull();
    });
  });

  describe('3. Fast-Path Cache Hit vs Miss Integration', () => {
    it('should execute procedural generation on cache miss and return cached data on hit', async () => {
      const seed = 'fast-path-seed-999';

      // 1. Initial call: Cache Miss
      const missResult = await generateTerrainAsync(40, 40, seed);
      expect(missResult.cached).toBe(false);

      // Wait briefly for background IndexedDB write
      await new Promise((r) => setTimeout(r, 20));

      // Clear in-memory cache to force reading from IndexedDB
      clearTerrainCache(seed);

      // 2. Second call: Cache Hit from IndexedDB
      const hitResult = await generateTerrainAsync(40, 40, seed);
      expect(hitResult.cached).toBe(true);
      expect(hitResult.width).toBe(40);
      expect(hitResult.height).toBe(40);
      expect(hitResult.grid[0][0].elevation).toBeCloseTo(missResult.grid[0][0].elevation, 4);
    });
  });

  describe('4. Schema Version Invalidation', () => {
    it('should invalidate and return null if schema version changes', async () => {
      const seed = 'schema-version-test';
      const terrain = generateTerrain(30, 30, seed);
      const serialized = serializeChunk(terrain, 0, 0);

      // Intentionally tamper version number to simulate old schema version
      const oldVersionChunk = {
        ...serialized,
        version: 99999, // mismatch with TERRAIN_SCHEMA_VERSION = 1
      };

      await terrainCacheDB.putChunk(seed, 0, 0, oldVersionChunk as any);

      // Reading chunk should return null due to version mismatch
      const retrieved = await terrainCacheDB.getChunk(seed, 0, 0);
      expect(retrieved).toBeNull();
    });
  });
});
