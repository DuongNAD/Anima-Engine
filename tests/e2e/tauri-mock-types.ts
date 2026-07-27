// The shape of the Tauri v2 internals the e2e specs inject into the browser.
//
// Two specs each opened with the same `declare global` block, and both wrote it in `any`: the
// callback map, the emitter payload and both `__TAURI_*` globals. That is the mock the *entire*
// adversarial e2e suite runs against — a command name typo inside it produces a mock that silently
// answers `undefined` and a test that passes for the wrong reason.
//
// So the contract is declared once, here. The global augmentation lands wherever this file is part
// of the program, and the specs import it explicitly so the dependency is visible rather than
// ambient.
//
// # Why the injected functions can still be annotated with these
//
// `page.addInitScript` serialises a *compiled* function. Type annotations are gone by then, so a
// type-only reference inside the injected body costs nothing at runtime — which is what lets the
// mock be checked at all despite never being importable from the browser side.

/** Arguments Tauri passes to `invoke`. `plugin:event|listen` is the one shape the mock reads. */
export interface TauriInvokeArgs {
  /** Event name, for `plugin:event|listen`. */
  event?: string;
  /** Callback id previously returned by `transformCallback`. */
  handler?: number;
  [key: string]: unknown;
}

/** What a Tauri event listener is called with. */
export interface TauriEventEnvelope {
  event: string;
  payload: unknown;
  id: number;
}

/** The subset of `__TAURI_INTERNALS__` the frontend reaches for. */
export interface TauriInternalsMock {
  /** `args` is not optional: `@tauri-apps/api`'s `invoke` defaults it to `{}` before calling here. */
  invoke: (cmd: string, args: TauriInvokeArgs) => Promise<unknown>;
  /** `once` is real — `listen`'s one-shot form passes it — and a mock that drops it never unsubscribes. */
  transformCallback: (callback: (event: TauriEventEnvelope) => void, once?: boolean) => number;
  unregisterCallback: (id: number) => void;
  convertFileSrc: (filePath: string, protocol?: string) => string;
}

/**
 * The subset of `__TAURI_EVENT_PLUGIN_INTERNALS__` the frontend reaches for.
 *
 * Both parameters are named. `unlisten()` calls this with the event name and the id it was given,
 * and a declaration that took neither described a function that would have had to drop every
 * listener for the event — which is the bug `tauri-mock.ts` documents having fixed at runtime while
 * this type still said otherwise.
 */
export interface TauriEventPluginInternalsMock {
  unregisterListener: (event: string, eventId: number) => void;
}

/**
 * One command as the transport recorded it.
 *
 * `args` stays `unknown`: what a spec asserts about it varies per command, and narrowing is the
 * spec's job.
 */
export interface RecordedInvocation {
  cmd: string;
  args: unknown;
}

/**
 * The spec-facing side of the deterministic transport.
 *
 * Deliberately not part of `__TAURI_INTERNALS__`, so nothing the app can reach depends on it. It is
 * declared here rather than described inline at each `page.evaluate` because it was described four
 * times, three of them partially, and a `page.evaluate` returns whatever its callback claims —
 * there is no runtime check on the other side of that boundary to catch a wrong one.
 */
export interface AnimaTauriMock {
  /** Deliver `payload` to every listener registered for `name`; returns how many were called. */
  emit(name: string, payload: unknown): number;
  /** Every command invoked so far, in order. */
  invocations(): RecordedInvocation[];
  /** How many listeners are registered for `name`. */
  listenerCount(name: string): number;
}

declare global {
  interface Window {
    /** Callback ids subscribed to each event name. */
    __mock_listeners: Record<string, number[]>;
    /** Callbacks by the id `transformCallback` handed out. */
    __mock_callbacks: Map<number, (event: TauriEventEnvelope) => void>;
    __mock_callback_counter: number;
    /** Test-side entry point: deliver `payload` to everything listening for `eventName`. */
    __mock_emit: (eventName: string, payload: unknown) => void;
    __TAURI_INTERNALS__: TauriInternalsMock;
    __TAURI_EVENT_PLUGIN_INTERNALS__: TauriEventPluginInternalsMock;
    /** Installed by `installDeterministicTauri`; absent on a page that did not ask for it. */
    __animaTauriMock: AnimaTauriMock;
  }
}
