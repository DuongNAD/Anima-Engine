import { generateWorld, WORLD_GEN_VERSION } from './worldGen';
import type { World, WorldGenOptions } from './worldGen';

// ---------------------------------------------------------------------------------------
// Persistent cache for the huge SoA World.
//
// IndexedDB serialises with the structured clone algorithm, which stores TypedArrays /
// ArrayBuffers as raw BINARY (no JSON.stringify). A 1024^2 world (~17 MB of typed arrays)
// round-trips quickly and compactly, so subsequent app starts skip generation entirely and
// read the world straight back into RAM.
// ---------------------------------------------------------------------------------------

const DB_NAME = 'anima-world';
const STORE = 'worlds';

const memo = new Map<string, World>();

function keyFor(seed: string | number, size: number, shape: string): string {
  return `world:v${WORLD_GEN_VERSION}:${seed}:${size}:${shape}`;
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

function idbGet(key: string): Promise<World | undefined> {
  return openDB().then(
    (db) =>
      new Promise<World | undefined>((resolve, reject) => {
        const tx = db.transaction(STORE, 'readonly');
        const req = tx.objectStore(STORE).get(key);
        req.onsuccess = () => resolve(req.result as World | undefined);
        req.onerror = () => reject(req.error);
      }),
  );
}

function idbSet(key: string, value: World): Promise<void> {
  return openDB().then(
    (db) =>
      new Promise<void>((resolve, reject) => {
        const tx = db.transaction(STORE, 'readwrite');
        // The structured clone here stores the TypedArrays as binary, not text.
        tx.objectStore(STORE).put(value, key);
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      }),
  );
}

function isValid(w: World | undefined, size: number): w is World {
  const n = size * size;
  // Check EVERY field the renderer relies on — not just the version — so a cache written by an
  // intermediate/partial build (e.g. hot-reloaded mid-refactor under the same version number)
  // is rejected and regenerated instead of crashing the scene with a missing array/field.
  return (
    !!w &&
    w.version === WORLD_GEN_VERSION &&
    w.size === size &&
    w.elevation?.length === n &&
    w.moisture?.length === n &&
    w.temperature?.length === n &&
    w.flow?.length === n &&
    w.water?.length === n &&
    w.shore?.length === n &&
    w.biome?.length === n &&
    Array.isArray(w.lakeBasins) &&
    typeof w.seaLevel === 'number'
  );
}

/** Synchronous: memoized generate (for environments without IndexedDB, e.g. tests). */
export function getMemoizedWorld(seed: string | number, opts: WorldGenOptions = {}): World {
  const size = opts.size ?? 1024;
  const shape = opts.shape ?? 'continent';
  const key = keyFor(seed, size, shape);
  let w = memo.get(key);
  if (!w) {
    w = generateWorld(seed, opts);
    memo.set(key, w);
  }
  return w;
}

/**
 * Generate the world OFF the main thread in a Web Worker (so the UI stays responsive during
 * a fresh ~1s build), transferring the TypedArray buffers back zero-copy. Falls back to a
 * synchronous generation if Workers are unavailable or the worker errors.
 */
function generateWorldAsync(seed: string | number, opts: WorldGenOptions): Promise<World> {
  const size = opts.size ?? 1024;
  return new Promise((resolve) => {
    if (typeof Worker === 'undefined') {
      resolve(generateWorld(seed, opts));
      return;
    }
    let worker: Worker;
    try {
      worker = new Worker(new URL('./worldGen.worker.ts', import.meta.url), { type: 'module' });
    } catch {
      resolve(generateWorld(seed, opts));
      return;
    }
    let settled = false;
    const finishSync = () => {
      if (settled) return;
      settled = true;
      try {
        worker.terminate();
      } catch {
        /* ignore */
      }
      resolve(generateWorld(seed, opts));
    };
    worker.onmessage = (e: MessageEvent<World>) => {
      if (settled) return;
      settled = true;
      worker.terminate();
      // Guard against a worker built from stale/partial code: regenerate synchronously if the
      // payload is missing any field the renderer needs.
      resolve(isValid(e.data, size) ? e.data : generateWorld(seed, opts));
    };
    worker.onerror = finishSync;
    try {
      worker.postMessage({ seed, opts });
    } catch {
      finishSync();
    }
  });
}

/**
 * Returns the world, preferring (1) the in-memory memo, then (2) the IndexedDB cache from a
 * previous session, and only (3) running the heavy generator (in a Web Worker) as a last
 * resort — after which the result is persisted so future starts read it straight back.
 */
export async function loadOrGenerateWorld(seed: string | number, opts: WorldGenOptions = {}): Promise<World> {
  const size = opts.size ?? 1024;
  const shape = opts.shape ?? 'continent';
  const key = keyFor(seed, size, shape);

  const memoized = memo.get(key);
  if (memoized) return memoized;

  if (typeof indexedDB !== 'undefined') {
    try {
      const cached = await idbGet(key);
      if (isValid(cached, size)) {
        memo.set(key, cached);
        return cached;
      }
    } catch {
      /* fall through to generation */
    }
  }

  const generated = await generateWorldAsync(seed, opts);
  memo.set(key, generated);
  if (typeof indexedDB !== 'undefined') {
    idbSet(key, generated).catch(() => {
      /* best-effort persistence; rendering does not depend on it */
    });
  }
  return generated;
}

/** Drop the cached world(s) so the next load regenerates (e.g. after changing the seed). */
export function clearWorldCache(): Promise<void> {
  memo.clear();
  if (typeof indexedDB === 'undefined') return Promise.resolve();
  return new Promise((resolve) => {
    const req = indexedDB.deleteDatabase(DB_NAME);
    req.onsuccess = () => resolve();
    req.onerror = () => resolve();
    req.onblocked = () => resolve();
  });
}
