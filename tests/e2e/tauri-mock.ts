import type { Page } from '@playwright/test';

// ---------------------------------------------------------------------------------------
// A deterministic, in-page Tauri IPC transport for browser E2E.
//
// # What this replaces, and why
//
// Five specs used to `spawn('src-tauri/target/release/anima-engine')`, wait a second, and then
// drive an ordinary Vite page with Playwright. Nothing connected the two. The page was served by
// Vite over HTTP and had no `__TAURI_INTERNALS__` at all, so every `invoke` in it rejected; the
// spawned process rendered into its own webview that Playwright never touched. The specs then
// caught the resulting assertion failures and called `test.skip()`, so the whole arrangement
// reported as green-with-skips while proving nothing about IPC in either process.
//
// The honest split is:
//
//   * **browser scope** (this file) — the real frontend, in a real browser, against an IPC
//     transport whose replies are fixed and stated here. It proves the UI reads, renders and
//     reacts to the contract. Zero skips: if the app does not render, that is a failure.
//   * **real backend scope** — `real_backend.spec.ts`, which needs a Tauri WebDriver session and
//     is only declared when explicitly required, so it fails closed instead of skipping.
//
// # How it attaches
//
// `@tauri-apps/api` 2.11 routes everything through three functions on `window.__TAURI_INTERNALS__`:
// `invoke` (core.js:202), `transformCallback` (core.js:72) and `unregisterCallback` (core.js:118).
// `listen()` is not special — it is `invoke('plugin:event|listen', { event, handler })` where
// `handler` is a callback id from `transformCallback` (event.js:76). Implementing those three is
// therefore the whole surface, and it is the same surface the real webview implements.
//
// Installed with `addInitScript` so it exists before any module executes: `@tauri-apps/api` is
// imported at module scope by `App.tsx`, and a transport that arrives after the first `invoke`
// is a race.
// ---------------------------------------------------------------------------------------

/** Handle for driving the mock from a spec. */
export interface TauriMockHandle {
  /** Push an event to every listener registered for `name`. */
  emit(name: string, payload: unknown): Promise<void>;
  /** Every command invoked so far, in order. */
  invocations(): Promise<Array<{ cmd: string; args: unknown }>>;
  /** Commands invoked at least once. */
  invokedCommands(): Promise<string[]>;
}

/**
 * Deterministic replies, keyed by command.
 *
 * Fixed values, never randomised: a spec that asserts "the tick counter shows 1234" is only
 * meaningful if 1234 is what the transport said. Shapes follow `src/types/generated/*`, which
 * `ts-rs` derives from the Rust structs, so a backend change that alters the contract shows up
 * as a type error here rather than as a mock that quietly describes an older API.
 */
const DETERMINISTIC_REPLIES: Record<string, unknown> = {
  get_simulation_status: { running: false, tick_count: 1234, avg_tick_time_ms: 4.2, fps: 60 },
  get_map_elites_grid: { resolution: 2, cells: [], coverage: 0, best_fitness: 0 },
  get_pheromone_grid: { width: 2, height: 2, danger: [0, 0, 0, 0], food: [0, 0, 0, 0] },
  get_active_raycasts: [],
  get_environmental_elements: [],
  get_lineage_graph: { nodes: [], edges: [] },
  get_chronicle_history: [],
  get_ecosystem_state: {
    producer_biomass: 100,
    herbivore_biomass: 50,
    carnivore_biomass: 10,
    detritus_pool: 25,
    total_energy: 185,
  },
  get_terrain_map: { width: 2, height: 2, cells: [0, 0, 0, 0] },
  toggle_simulation: true,
  toggle_evolution: true,
  update_evolution_settings: null,
  trigger_migration: null,
  save_simulation_state: null,
  load_simulation_state: null,
  save_world_artifact: null,
};

/**
 * Install the transport on `page`. Call before the first navigation.
 *
 * `overrides` replaces individual command replies; a value of `undefined` is a real reply of
 * `undefined`, not "use the default", so a spec can assert how the UI handles an empty answer.
 */
export async function installDeterministicTauri(
  page: Page,
  overrides: Record<string, unknown> = {},
): Promise<TauriMockHandle> {
  const replies = { ...DETERMINISTIC_REPLIES, ...overrides };

  await page.addInitScript((table: Record<string, unknown>) => {
    interface MockState {
      callbacks: Map<number, (payload: unknown) => void>;
      listeners: Map<string, number[]>;
      invocations: Array<{ cmd: string; args: unknown }>;
      nextCallbackId: number;
      nextEventId: number;
    }
    const state: MockState = {
      callbacks: new Map(),
      listeners: new Map(),
      invocations: [],
      nextCallbackId: 1,
      nextEventId: 1,
    };

    const internals = {
      transformCallback(callback: (payload: unknown) => void, once?: boolean): number {
        const id = state.nextCallbackId++;
        state.callbacks.set(id, (payload: unknown) => {
          if (once) state.callbacks.delete(id);
          callback(payload);
        });
        return id;
      },
      unregisterCallback(id: number): void {
        state.callbacks.delete(id);
      },
      convertFileSrc(filePath: string, protocol = 'asset'): string {
        return `${protocol}://localhost/${encodeURIComponent(filePath)}`;
      },
      invoke(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
        state.invocations.push({ cmd, args: args ?? null });

        // The event plugin. `listen()` compiles to this; there is no separate channel.
        if (cmd === 'plugin:event|listen') {
          const name = String(args?.event);
          const handler = Number(args?.handler);
          const list = state.listeners.get(name) ?? [];
          list.push(handler);
          state.listeners.set(name, list);
          return Promise.resolve(state.nextEventId++);
        }
        if (cmd === 'plugin:event|unlisten') {
          const name = String(args?.event);
          state.listeners.set(name, []);
          return Promise.resolve(null);
        }

        if (cmd in table) return Promise.resolve(table[cmd]);
        // An unmocked command is a gap in this table, not something to paper over: a silent
        // `undefined` would let a spec pass while the UI read nothing.
        return Promise.reject(
          new Error(
            `[tauri-mock] no deterministic reply for command "${cmd}". Add it to ` +
              `DETERMINISTIC_REPLIES in tests/e2e/tauri-mock.ts.`,
          ),
        );
      },
    };

    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = internals;

    // The spec-facing side. Kept separate from `__TAURI_INTERNALS__` so nothing the app can
    // reach depends on it.
    (window as unknown as Record<string, unknown>).__animaTauriMock = {
      emit(name: string, payload: unknown): number {
        const list = state.listeners.get(name) ?? [];
        for (const id of list) {
          state.callbacks.get(id)?.({ event: name, id, payload });
        }
        return list.length;
      },
      invocations: () => state.invocations,
      listenerCount: (name: string) => (state.listeners.get(name) ?? []).length,
    };
  }, replies);

  return {
    async emit(name: string, payload: unknown): Promise<void> {
      await page.evaluate(
        ([n, p]) =>
          (
            window as unknown as { __animaTauriMock: { emit(n: string, p: unknown): number } }
          ).__animaTauriMock.emit(n as string, p),
        [name, payload] as [string, unknown],
      );
    },
    async invocations(): Promise<Array<{ cmd: string; args: unknown }>> {
      return page.evaluate(
        () =>
          (
            window as unknown as {
              __animaTauriMock: { invocations(): Array<{ cmd: string; args: unknown }> };
            }
          ).__animaTauriMock.invocations(),
      );
    },
    async invokedCommands(): Promise<string[]> {
      const calls = await this.invocations();
      return [...new Set(calls.map((c) => c.cmd))];
    },
  };
}
