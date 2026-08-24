import type { Page } from '@playwright/test';
// The `Window` augmentation this file installs against. Imported for the side effect so the
// dependency is visible rather than ambient, exactly as the specs do.
import './tauri-mock-types';
import type { RecordedInvocation, TauriEventEnvelope, TauriInvokeArgs } from './tauri-mock-types';
import type { ChronicleEvent } from '../../src/types/generated/ChronicleEvent';
import type { EcosystemState } from '../../src/types/generated/EcosystemState';
import type { EnvironmentalState } from '../../src/types/generated/EnvironmentalState';
import type { LegacyImportListing } from '../../src/types/generated/LegacyImportListing';
import type { MapElitesGridState } from '../../src/types/generated/MapElitesGridState';
import type { PheromoneGridState } from '../../src/types/generated/PheromoneGridState';
import type { RaycastTelemetry } from '../../src/types/generated/RaycastTelemetry';
import type { SimulationStatus } from '../../src/types/generated/SimulationStatus';

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
  invocations(): Promise<RecordedInvocation[]>;
  /** Commands invoked at least once. */
  invokedCommands(): Promise<string[]>;
}

/**
 * Deterministic replies, keyed by command.
 *
 * Fixed values, never randomised: a spec that asserts "the tick counter shows 1234" is only
 * meaningful if 1234 is what the transport said.
 *
 * **Typed by the generated bindings, not by hand.** `src/types/generated/*` is what `ts-rs`
 * derives from the Rust structs, so annotating each reply makes a wrong shape a compile error.
 * That is not hypothetical caution: the first version of this table invented
 * `{ resolution, cells, coverage, best_fitness }` for `get_map_elites_grid` and
 * `{ danger, food }` for `get_pheromone_grid`. Both are plausible and neither is the contract —
 * the real shapes are `{ grid: Record<string, EliteIndividualState>, grid_resolution }` and
 * `{ grid: number[], width, height }`. The app crashed into its own error boundary with
 * `Cannot read properties of undefined (reading '0,0')`, and every spec then failed on a missing
 * `<h1>`, which reads like a broken app rather than a mock describing an API that never existed.
 */
const DETERMINISTIC_REPLIES: Record<string, unknown> = {
  get_simulation_status: {
    running: false,
    tick_count: 1234,
    avg_tick_time_ms: 4.2,
    fps: 60,
  } satisfies SimulationStatus,
  get_map_elites_grid: {
    grid: { '0,0': { fitness: 1.5, features: [0.25, 0.75] } },
    grid_resolution: 8,
  } satisfies MapElitesGridState,
  get_pheromone_grid: {
    grid: [0, 0, 0, 0],
    width: 2,
    height: 2,
  } satisfies PheromoneGridState,
  get_active_raycasts: [] satisfies RaycastTelemetry[],
  get_environmental_elements: { elements: [] } satisfies EnvironmentalState,
  get_lineage_graph: { nodes: [], edges: [] },
  get_chronicle_history: [] satisfies ChronicleEvent[],
  get_ecosystem_state: {
    detritus: 25,
    plants: 100,
    animals: 60,
    total: 185,
    prey_count: 40,
    predator_count: 20,
    shannon: 0.9,
    simpson: 0.6,
    prey_mass: 1.2,
    predator_mass: 2.4,
    niche_divergence: 0.5,
    archive_coverage: 0.3,
  } satisfies EcosystemState,
  get_terrain_map: { width: 2, height: 2, cells: [0, 0, 0, 0] },
  // The LOD focus pair. Neither appears in a literal `invoke("...")` in `src/` — `lodFocus.ts`
  // resolves `invoke` into a cached function and calls it through a variable — so a grep for
  // command names misses them. Leaving them out of this table was not a harmless gap: the mock
  // rejects unknown commands, `sendLodFocus` runs on a timer, and the resulting stream of
  // unhandled rejections tore the page down mid-suite. Every spec then failed on a missing `h1`,
  // which reads like the app is broken rather than like the transport is incomplete.
  get_lod_bands: { bands: [] },
  set_lod_focus: null,
  toggle_simulation: true,
  toggle_evolution: true,
  update_evolution_settings: null,
  trigger_migration: null,
  save_simulation_state: null,
  load_simulation_state: null,
  save_world_artifact: null,
  // The legacy-save migration. Typed by the generated binding for the same reason as the rest: the
  // `ignored` field was added to `LegacyImportListing` to stop the listing hiding files it could not
  // open, and a hand-written reply here would keep passing after a rename.
  list_legacy_saves: {
    directory: 'C:\\Users\\test\\AppData\\Roaming\\com.anima.engine\\legacy-import',
    names: ['old_world.json', 'second_world.json'],
    ignored: ['notes.txt'],
  } satisfies LegacyImportListing,
  // What the backend reports it wrote — the normalised name, not the one that was typed.
  import_legacy_save: 'restored.json',
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
      callbacks: Map<number, (event: TauriEventEnvelope) => void>;
      listeners: Map<string, Array<{ handler: number; eventId: number }>>;
      invocations: RecordedInvocation[];
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

    const internals: Window['__TAURI_INTERNALS__'] = {
      transformCallback(callback: (event: TauriEventEnvelope) => void, once?: boolean): number {
        const id = state.nextCallbackId++;
        state.callbacks.set(id, (event: TauriEventEnvelope) => {
          if (once) state.callbacks.delete(id);
          callback(event);
        });
        return id;
      },
      unregisterCallback(id: number): void {
        state.callbacks.delete(id);
      },
      convertFileSrc(filePath: string, protocol = 'asset'): string {
        return `${protocol}://localhost/${encodeURIComponent(filePath)}`;
      },
      invoke(cmd: string, args: TauriInvokeArgs): Promise<unknown> {
        state.invocations.push({ cmd, args: args ?? null });

        // The event plugin. `listen()` compiles to this; there is no separate channel.
        if (cmd === 'plugin:event|listen') {
          const name = String(args?.event);
          const eventId = state.nextEventId++;
          const list = state.listeners.get(name) ?? [];
          list.push({ handler: Number(args?.handler), eventId });
          state.listeners.set(name, list);
          return Promise.resolve(eventId);
        }
        if (cmd === 'plugin:event|unlisten') {
          // The real plugin removes ONE listener by id, not all of them for the event. Clearing
          // the list would make a component's own teardown silence every other subscriber.
          const name = String(args?.event);
          const eventId = Number(args?.eventId);
          const list = state.listeners.get(name) ?? [];
          state.listeners.set(name, list.filter((e) => e.eventId !== eventId));
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

    window.__TAURI_INTERNALS__ = internals;

    // A second global, and not an optional one.
    //
    // `unlisten()` calls `window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener(event, id)`
    // *before* it invokes `plugin:event|unlisten` (event.js:43), with no guard. Without it every
    // teardown threw `Cannot read properties of undefined (reading 'unregisterListener')` — and
    // React 18 StrictMode mounts twice, so the very first render unsubscribed and threw. The
    // rejections took down the whole page.
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener(event: string, eventId: number): void {
        const list = state.listeners.get(event);
        if (!list) return;
        state.listeners.set(
          event,
          list.filter((entry) => entry.eventId !== eventId),
        );
      },
    };

    // The spec-facing side. Kept separate from `__TAURI_INTERNALS__` so nothing the app can
    // reach depends on it.
    window.__animaTauriMock = {
      emit(name: string, payload: unknown): number {
        const list = state.listeners.get(name) ?? [];
        for (const entry of list) {
          state.callbacks.get(entry.handler)?.({ event: name, id: entry.eventId, payload });
        }
        return list.length;
      },
      invocations: () => state.invocations,
      listenerCount: (name: string) => (state.listeners.get(name) ?? []).length,
    };
  }, replies);

  return {
    async emit(name: string, payload: unknown): Promise<void> {
      // A named tuple rather than an inline pair: `page.evaluate` takes exactly one argument, so
      // the two have to travel together, and annotating the variable is what types the destructured
      // halves inside the browser callback.
      const call: [string, unknown] = [name, payload];
      await page.evaluate(([n, p]) => window.__animaTauriMock.emit(n, p), call);
    },
    async invocations(): Promise<RecordedInvocation[]> {
      return page.evaluate(() => window.__animaTauriMock.invocations());
    },
    async invokedCommands(): Promise<string[]> {
      const calls = await page.evaluate(() => window.__animaTauriMock.invocations());
      return [...new Set(calls.map((c) => c.cmd))];
    },
  };
}
