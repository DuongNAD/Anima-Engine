import { TERRAIN_SCHEMA_VERSION, SerializedTerrainChunk } from './terrainSerializer';

export const DB_NAME = 'AnimaEngine_TerrainCache_v1';
export const DB_VERSION = 1;

export class TerrainCacheDatabase {
  private dbPromise: Promise<IDBDatabase | null> | null = null;

  /**
   * Helper to check if IndexedDB is available.
   */
  private get indexedDBInstance(): IDBFactory | null {
    if (typeof window !== 'undefined' && window.indexedDB) {
      return window.indexedDB;
    }
    if (typeof globalThis !== 'undefined' && (globalThis as any).indexedDB) {
      return (globalThis as any).indexedDB;
    }
    return null;
  }

  /**
   * Opens the IndexedDB database connection.
   */
  public async open(): Promise<IDBDatabase | null> {
    const idb = this.indexedDBInstance;
    if (!idb) {
      return null;
    }

    if (this.dbPromise) {
      return this.dbPromise;
    }

    this.dbPromise = new Promise((resolve, reject) => {
      try {
        const request = idb.open(DB_NAME, DB_VERSION);

        request.onupgradeneeded = () => {
          const db = request.result;

          if (!db.objectStoreNames.contains('maps')) {
            db.createObjectStore('maps', { keyPath: 'seed' });
          }

          if (!db.objectStoreNames.contains('chunks')) {
            const chunkStore = db.createObjectStore('chunks', { keyPath: 'key' });
            chunkStore.createIndex('seedIndex', 'seed', { unique: false });
            chunkStore.createIndex('accessIndex', 'lastAccessed', { unique: false });
          }
        };

        request.onsuccess = () => {
          resolve(request.result);
        };

        request.onerror = () => {
          this.dbPromise = null;
          reject(request.error || new Error('Failed to open IndexedDB database'));
        };
      } catch (err) {
        this.dbPromise = null;
        resolve(null);
      }
    });

    return this.dbPromise;
  }

  /**
   * Generates a primary key for a chunk.
   */
  public getChunkKey(seed: string | number, chunkX: number, chunkY: number): string {
    return `${seed}_${chunkX}_${chunkY}`;
  }

  /**
   * Retrieves a cached chunk from IndexedDB.
   */
  public async getChunk(
    seed: string | number,
    chunkX = 0,
    chunkY = 0
  ): Promise<SerializedTerrainChunk | null> {
    const db = await this.open();
    if (!db) return null;

    const key = this.getChunkKey(seed, chunkX, chunkY);

    return new Promise((resolve) => {
      try {
        const tx = db.transaction(['chunks'], 'readwrite');
        const store = tx.objectStore('chunks');
        const getReq = store.get(key);

        getReq.onsuccess = () => {
          const record = getReq.result;
          if (!record) {
            resolve(null);
            return;
          }

          // Check schema version match
          if (record.version !== TERRAIN_SCHEMA_VERSION) {
            // Stale schema version, delete and return null
            store.delete(key);
            resolve(null);
            return;
          }

          // Update lastAccessed timestamp for LRU management
          record.lastAccessed = Date.now();
          store.put(record);

          resolve(record as SerializedTerrainChunk);
        };

        getReq.onerror = () => {
          resolve(null);
        };
      } catch (err) {
        resolve(null);
      }
    });
  }

  /**
   * Stores a serialized terrain chunk in IndexedDB.
   */
  public async putChunk(
    seed: string | number,
    chunkX: number,
    chunkY: number,
    data: SerializedTerrainChunk
  ): Promise<void> {
    const db = await this.open();
    if (!db) return;

    const key = this.getChunkKey(seed, chunkX, chunkY);
    const chunkRecord = {
      ...data,
      key,
      seed,
      chunkX,
      chunkY,
      lastAccessed: Date.now(),
    };

    const mapRecord = {
      seed,
      createdAt: Date.now(),
      version: TERRAIN_SCHEMA_VERSION,
      width: data.width,
      height: data.height,
    };

    return new Promise((resolve, reject) => {
      try {
        const tx = db.transaction(['maps', 'chunks'], 'readwrite');
        const mapsStore = tx.objectStore('maps');
        const chunksStore = tx.objectStore('chunks');

        mapsStore.put(mapRecord);
        chunksStore.put(chunkRecord);

        tx.oncomplete = () => {
          this.manageLRUQuota().catch(() => {});
          resolve();
        };

        tx.onerror = () => {
          reject(tx.error);
        };
      } catch (err) {
        resolve();
      }
    });
  }

  /**
   * Clears all cached chunks for a specific map seed.
   */
  public async clearMapCache(seed: string | number): Promise<void> {
    const db = await this.open();
    if (!db) return;

    // Collect all variations of seed to query (string / number)
    const seedsToMatch: Array<string | number> = [seed];
    if (typeof seed === 'number') {
      seedsToMatch.push(String(seed));
    } else if (typeof seed === 'string') {
      const numSeed = Number(seed);
      if (!isNaN(numSeed) && seed.trim() !== '') {
        seedsToMatch.push(numSeed);
      }
    }

    // Step 1: Retrieve all keys for the matching seed in a dedicated readonly transaction
    const keysSet = new Set<IDBValidKey>();
    for (const s of seedsToMatch) {
      const retrievedKeys = await new Promise<IDBValidKey[]>((resolve) => {
        try {
          const tx = db.transaction(['chunks'], 'readonly');
          const chunksStore = tx.objectStore('chunks');
          const seedIndex = chunksStore.index('seedIndex');
          const req = seedIndex.getAllKeys(s as IDBValidKey);

          req.onsuccess = () => {
            resolve(req.result || []);
          };
          req.onerror = () => {
            resolve([]);
          };
        } catch (err) {
          resolve([]);
        }
      });

      retrievedKeys.forEach((k) => keysSet.add(k));
    }

    // Step 2: Delete maps record and all retrieved chunk keys in a dedicated readwrite transaction
    return new Promise<void>((resolve, reject) => {
      try {
        const tx = db.transaction(['maps', 'chunks'], 'readwrite');
        const mapsStore = tx.objectStore('maps');
        const chunksStore = tx.objectStore('chunks');

        for (const s of seedsToMatch) {
          mapsStore.delete(s as IDBValidKey);
        }

        const deleteReqs: IDBRequest[] = [];
        for (const key of keysSet) {
          deleteReqs.push(chunksStore.delete(key as IDBValidKey));
        }

        let txCompleted = false;
        let pendingRequests = deleteReqs.length;

        const checkResolution = () => {
          if (txCompleted && pendingRequests <= 0) {
            resolve();
          }
        };

        if (deleteReqs.length === 0) {
          tx.oncomplete = () => resolve();
          tx.onerror = () => reject(tx.error);
        } else {
          deleteReqs.forEach((req) => {
            req.onsuccess = () => {
              pendingRequests--;
              checkResolution();
            };
            req.onerror = () => {
              pendingRequests--;
              checkResolution();
            };
          });

          tx.oncomplete = () => {
            txCompleted = true;
            checkResolution();
          };

          tx.onerror = () => {
            reject(tx.error);
          };
        }
      } catch (err) {
        resolve();
      }
    });
  }

  /**
   * Clears all terrain cache entries across all maps.
   */
  public async clearAllCache(): Promise<void> {
    const db = await this.open();
    if (!db) return;

    return new Promise((resolve, reject) => {
      try {
        const tx = db.transaction(['maps', 'chunks'], 'readwrite');
        const mapsReq = tx.objectStore('maps').clear();
        const chunksReq = tx.objectStore('chunks').clear();

        let txDone = false;
        let mapsDone = false;
        let chunksDone = false;

        const checkDone = () => {
          if (txDone && mapsDone && chunksDone) {
            resolve();
          }
        };

        mapsReq.onsuccess = mapsReq.onerror = () => {
          mapsDone = true;
          checkDone();
        };

        chunksReq.onsuccess = chunksReq.onerror = () => {
          chunksDone = true;
          checkDone();
        };

        tx.oncomplete = () => {
          txDone = true;
          checkDone();
        };

        tx.onerror = () => reject(tx.error);
      } catch (err) {
        resolve();
      }
    });
  }

  /**
   * Alias for clearAllCache to support plural naming convention.
   */
  public async clearAllCaches(): Promise<void> {
    return this.clearAllCache();
  }


  /**
   * LRU eviction management to prevent storage quota overrun.
   */
  public async manageLRUQuota(maxChunks = 200): Promise<void> {
    const db = await this.open();
    if (!db) return;

    return new Promise((resolve) => {
      try {
        const tx = db.transaction(['chunks'], 'readwrite');
        const chunksStore = tx.objectStore('chunks');
        const accessIndex = chunksStore.index('accessIndex');

        const countReq = chunksStore.count();
        countReq.onsuccess = () => {
          const count = countReq.result;
          if (count <= maxChunks) {
            resolve();
            return;
          }

          const excess = count - maxChunks;
          let deleted = 0;

          const cursorReq = accessIndex.openCursor(); // Ascending order (oldest first)
          cursorReq.onsuccess = () => {
            const cursor = cursorReq.result;
            if (cursor && deleted < excess) {
              cursor.delete();
              deleted++;
              cursor.continue();
            } else {
              resolve();
            }
          };
        };

        tx.onerror = () => resolve();
      } catch (err) {
        resolve();
      }
    });
  }
}

export const terrainCacheDB = new TerrainCacheDatabase();
