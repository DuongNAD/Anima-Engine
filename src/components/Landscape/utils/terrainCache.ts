import { generateTerrain } from './terrainGenerator';
import type { TerrainData } from './terrainGenerator';

// Bump this whenever the procedural generator changes so previously cached worlds (which
// were produced by the old algorithm) are discarded instead of rendered.
export const TERRAIN_CACHE_VERSION = 2;

const DB_NAME = 'anima-landscape';
const STORE = 'terrain';

// In-memory memo: one generated TerrainData per (seed,size) for the whole session, so the
// terrain is computed at most once even though several components consume it.
const memo = new Map<string, TerrainData>();

function keyFor(seed: string | number, width: number, height: number): string {
  return `v${TERRAIN_CACHE_VERSION}:${seed}:${width}x${height}`;
}

/** Synchronous: returns the memoized terrain, generating (and memoizing) on first call. */
export function getMemoizedTerrain(width: number, height: number, seed: string | number): TerrainData {
  const key = keyFor(seed, width, height);
  let t = memo.get(key);
  if (!t) {
    t = generateTerrain(width, height, seed);
    memo.set(key, t);
  }
  return t;
}

function openDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains(STORE)) db.createObjectStore(STORE);
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

function idbGet(key: string): Promise<TerrainData | undefined> {
  return openDB().then(
    (db) =>
      new Promise<TerrainData | undefined>((resolve, reject) => {
        const tx = db.transaction(STORE, 'readonly');
        const req = tx.objectStore(STORE).get(key);
        req.onsuccess = () => resolve(req.result as TerrainData | undefined);
        req.onerror = () => reject(req.error);
      }),
  );
}

function idbSet(key: string, value: TerrainData): Promise<void> {
  return openDB().then(
    (db) =>
      new Promise<void>((resolve, reject) => {
        const tx = db.transaction(STORE, 'readwrite');
        tx.objectStore(STORE).put(value, key);
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      }),
  );
}

/**
 * Returns the landscape terrain, preferring (1) the in-memory memo, then (2) the IndexedDB
 * cache persisted by a previous session, and only (3) running the heavy procedural generator
 * as a last resort — after which the result is persisted so future runs skip generation.
 */
export async function loadOrGenerateTerrain(
  width: number,
  height: number,
  seed: string | number,
): Promise<TerrainData> {
  const key = keyFor(seed, width, height);
  const memoized = memo.get(key);
  if (memoized) return memoized;

  if (typeof indexedDB !== 'undefined') {
    try {
      const cached = await idbGet(key);
      if (cached && cached.grid && cached.grid.length === height) {
        memo.set(key, cached);
        return cached;
      }
    } catch {
      /* fall through to generation */
    }
  }

  const generated = generateTerrain(width, height, seed);
  memo.set(key, generated);
  if (typeof indexedDB !== 'undefined') {
    idbSet(key, generated).catch(() => {
      /* best-effort persistence; rendering does not depend on it */
    });
  }
  return generated;
}

/** Flatten per-cell elevations into the heightmap that CameraControls expects. */
export function heightDataFromTerrain(terrain: TerrainData): Float32Array {
  const { width, height, grid } = terrain;
  const data = new Float32Array(width * height);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      data[y * width + x] = grid[y]?.[x]?.elevation ?? 0;
    }
  }
  return data;
}

/** Clears both the in-memory memo and the IndexedDB store (e.g. to regenerate a fresh world). */
export function clearTerrainCache(): Promise<void> {
  memo.clear();
  if (typeof indexedDB === 'undefined') return Promise.resolve();
  return new Promise((resolve) => {
    const req = indexedDB.deleteDatabase(DB_NAME);
    req.onsuccess = () => resolve();
    req.onerror = () => resolve();
    req.onblocked = () => resolve();
  });
}
