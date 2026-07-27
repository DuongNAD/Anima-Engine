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
  invoke: (cmd: string, args: TauriInvokeArgs) => Promise<unknown>;
  transformCallback: (callback: (event: TauriEventEnvelope) => void) => number;
  unregisterCallback: (id: number) => void;
  convertFileSrc: (filePath: string) => string;
}

/** The subset of `__TAURI_EVENT_PLUGIN_INTERNALS__` the frontend reaches for. */
export interface TauriEventPluginInternalsMock {
  unregisterListener: () => void;
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
  }
}
