import { loadOrGenerateWorld } from '../../../src/components/Landscape/utils/worldCache';
import { SHARED_WORLD_SEED, SHARED_WORLD_OPTS } from '../../../src/utils/sharedWorld';

/**
 * Explicit `beforeAll` hook timeout for warming the 2048x2048 shared world.
 * World generation is one-time fixture cost, not behaviour under test; GitHub Actions
 * ubuntu runners vary by roughly 40% in single-thread speed, so leaving cold generation
 * inside the first test body can exceed the 15 s per-test timeout.
 */
export const WARM_SHARED_WORLD_BUDGET_MS = 60_000;

export async function warmSharedWorld(): Promise<void> {
  const startMs = Date.now();
  await loadOrGenerateWorld(SHARED_WORLD_SEED, SHARED_WORLD_OPTS);
  const elapsedMs = Date.now() - startMs;
  // Flush microtasks/macrotask so loadOrGenerateWorld's fire-and-forget persistWorldArtifact
  // (`await import('@tauri-apps/api/core')` -> `invoke('save_world_artifact', ...)`) settles
  // inside beforeAll rather than leaking an async mock invoke call into the first test.
  await new Promise((resolve) => setTimeout(resolve, 0));
  console.info(`[warmSharedWorld] ${elapsedMs}ms`);
}
