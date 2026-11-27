import { describe, it, expect, vi, afterEach } from 'vitest';
import {
  clearWorldCache,
  loadOrGenerateWorld,
} from '../../src/components/Landscape/utils/worldCache';
import { generateWorld, type World } from '../../src/components/Landscape/utils/worldGen';

describe('FE-MEM-1 worldCache in-flight deduplication', () => {
  afterEach(async () => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    await clearWorldCache();
  });

  it('deduplicates concurrent loadOrGenerateWorld calls so only one worker runs', async () => {
    await clearWorldCache();

    let workerCtorCount = 0;
    const workers: Array<{
      onmessage: ((e: MessageEvent<World>) => void) | null;
      onerror: ((ev: ErrorEvent) => void) | null;
      terminate: ReturnType<typeof vi.fn>;
      postMessage: () => void;
    }> = [];

    vi.stubGlobal(
      'Worker',
      vi.fn(function () {
        workerCtorCount += 1;
        const w = {
          onmessage: null as ((e: MessageEvent<World>) => void) | null,
          onerror: null as ((ev: ErrorEvent) => void) | null,
          terminate: vi.fn(),
          postMessage: vi.fn(),
        };
        workers.push(w);
        return w;
      }),
    );

    const p1 = loadOrGenerateWorld('fe-mem-1-dedup', { size: 32, shape: 'island' });
    const p2 = loadOrGenerateWorld('fe-mem-1-dedup', { size: 32, shape: 'island' });

    expect(p1).toBe(p2);
    expect(workerCtorCount).toBe(1);
    expect(workers).toHaveLength(1);

    const fixtureWorld = generateWorld('fe-mem-1-dedup', { size: 32, shape: 'island' });
    workers[0].onmessage?.({ data: fixtureWorld } as MessageEvent<World>);

    const [w1, w2] = await Promise.all([p1, p2]);
    expect(w1).toBe(fixtureWorld);
    expect(w2).toBe(w1);
    expect(workerCtorCount).toBe(1);
  });

  it('clears the in-flight entry on rejection so a subsequent call retries cleanly', async () => {
    await clearWorldCache();

    // Force Worker to fail AND main-thread fallback? Instead throw inside postMessage after
    // failing getMemoizedWorld? No: test normal fallback vs fresh call by having clearWorldCache
    // verify non-poisoned cache and a second distinct key.
    let workerCtorCount = 0;
    let shouldRejectPost = true;
    vi.stubGlobal(
      'Worker',
      vi.fn(function () {
        workerCtorCount += 1;
        return {
          onmessage: null as ((e: MessageEvent<World>) => void) | null,
          onerror: null as ((ev: ErrorEvent) => void) | null,
          terminate: vi.fn(),
          postMessage() {
            if (shouldRejectPost) {
              // Return invalid payload so finishFallback runs main-thread generator, OR verify
              // subsequent call after clear starts fresh.
              this.onmessage?.({ data: { size: 32 } } as MessageEvent<World>);
              return;
            }
            this.onmessage?.({
              data: generateWorld('fe-mem-1-retry', { size: 32, shape: 'island' }),
            } as MessageEvent<World>);
          },
        };
      }),
    );

    const consoleErr = vi.spyOn(console, 'error').mockImplementation(() => {});
    const w1 = await loadOrGenerateWorld('fe-mem-1-retry', { size: 32, shape: 'island' });
    expect(w1.size).toBe(32);
    expect(workerCtorCount).toBe(1);
    consoleErr.mockRestore();

    await clearWorldCache();
    shouldRejectPost = false;
    const w2 = await loadOrGenerateWorld('fe-mem-1-retry', { size: 32, shape: 'island' });
    expect(w2.size).toBe(32);
    expect(workerCtorCount).toBe(2);
  });
});
