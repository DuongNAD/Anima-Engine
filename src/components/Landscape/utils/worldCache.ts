import { generateWorld, WORLD_GEN_VERSION } from './worldGen';
import type { World, WorldGenOptions } from './worldGen';
import { worldToArtifact } from './worldArtifact';

// Resolution of the shared World Artifact handed to the sim backend (the render world is far larger;
// the sim only needs a modest grid, and downsampling keeps the IPC payload small).
const SIM_ARTIFACT_SIZE = 256;

/**
 * Best-effort: hand the just-loaded world to the Rust backend (via the `save_world_artifact` command)
 * so the SIMULATION runs on the SAME authoritative world that is rendered — the crux of unifying the
 * two worlds. No-ops silently outside a Tauri context (the browser-only showcase, tests, SSR).
 */
async function persistWorldArtifact(world: World): Promise<void> {
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const buf = worldToArtifact(world, SIM_ARTIFACT_SIZE);
    await invoke('save_world_artifact', { bytes: Array.from(new Uint8Array(buf)) });
  } catch {
    /* not under Tauri, or backend unavailable — rendering is unaffected */
  }
}

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
    w.slope?.length === n &&
    w.water?.length === n &&
    w.riverAmt?.length === n &&
    w.shore?.length === n &&
    w.biome?.length === n &&
    Array.isArray(w.lakeBasins) &&
    typeof w.seaLevel === 'number'
  );
}

const IS_TEST_BUILD = (import.meta as { env?: { MODE?: string } }).env?.MODE === 'test';

/** Synchronous: memoized generate (for test environments without IndexedDB/Workers). */
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
 * a fresh ~1s build and the main bundle ships generateWorld only inside the worker chunk),
 * transferring the TypedArray buffers back zero-copy. If the worker is unavailable or fails,
 * fall back synchronously to main-thread generation instead of hanging.
 */
function generateWorldAsync(seed: string | number, opts: WorldGenOptions): Promise<World> {
  const size = opts.size ?? 1024;
  const runFallback = (
    resolve: (w: World) => void,
    reject: (reason?: unknown) => void,
    reason: string,
    errDetail?: unknown,
  ) => {
    if (!IS_TEST_BUILD || errDetail !== undefined) {
      console.error(`[worldCache] ${reason}; falling back to main-thread world generation.`, errDetail);
    }
    try {
      resolve(generateWorld(seed, opts));
    } catch (fallbackErr: unknown) {
      console.error('[worldCache] Main-thread world generation fallback failed:', fallbackErr);
      reject(
        fallbackErr instanceof Error
          ? fallbackErr
          : new Error(`${reason}: ${String(fallbackErr)}`),
      );
    }
  };
  return new Promise((resolve, reject) => {
    if (typeof Worker === 'undefined') {
      runFallback(resolve, reject, 'Web Workers unavailable for world generation');
      return;
    }
    let worker: Worker;
    try {
      worker = new Worker(new URL('./worldGen.worker.ts', import.meta.url), { type: 'module' });
    } catch (err: unknown) {
      runFallback(resolve, reject, 'Failed to construct world generation worker', err);
      return;
    }
    let settled = false;
    const finishFallback = (reason: string, errDetail?: unknown) => {
      if (settled) return;
      settled = true;
      try {
        worker.terminate();
      } catch {
        /* ignore */
      }
      runFallback(resolve, reject, reason, errDetail);
    };
    worker.onmessage = (e: MessageEvent<World>) => {
      if (settled) return;
      settled = true;
      worker.terminate();
      // Guard against a worker built from stale/partial code: fall back to main-thread generation
      // if the payload is missing any field the renderer needs.
      if (isValid(e.data, size)) {
        resolve(e.data);
      } else {
        runFallback(resolve, reject, 'World generation worker returned invalid payload');
      }
    };
    worker.onerror = (ev: ErrorEvent) =>
      finishFallback('World generation worker error', ev?.error ?? ev?.message ?? ev);
    try {
      worker.postMessage({ seed, opts });
    } catch (err: unknown) {
      finishFallback('Failed to post world generation request to worker', err);
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
        void persistWorldArtifact(cached);
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
  void persistWorldArtifact(generated);
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
