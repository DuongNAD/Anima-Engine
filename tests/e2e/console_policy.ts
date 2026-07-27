import type { Page } from '@playwright/test';

// ---------------------------------------------------------------------------------------
// One definition of "is this console message ours", shared by every browser-scope entry point.
//
// # Why this moved out of the spec
//
// It used to live inside `console_hygiene.spec.ts`, which meant it only governed the two pages that
// spec drives. `global-setup.ts` also opens the dashboard — to warm Vite's module graph — and
// nothing looked at what that page said. On 2026-07-27 it was saying quite a lot:
//
//   Failed to load lineage graph:     TypeError: Cannot read properties of undefined (reading 'invoke')
//   Failed to load chronicle history: TypeError: Cannot read properties of undefined (reading 'invoke')
//   Failed to listen to event chronicle-event:  TypeError: ... (reading 'transformCallback')
//   Failed to listen to event migration-event:  TypeError: ... (reading 'transformCallback')
//
// twice each (React 18 StrictMode double-invokes effects in dev). The run still reported
// `18 passed`, because the messages came from setup rather than from a test.
//
// # Two separate problems, and it is worth not conflating them
//
// 1. **Nothing watched setup.** This is what actually let the four messages through. The classifier
//    was fine; it simply was not applied to that page, because it lived inside a spec file and
//    `global-setup.ts` is not a spec. The fix is that the warm-up page is now watched too, and the
//    transport it was missing is installed.
//
// 2. **The classifier leaned on a stack string.** The original rule asked whether the message
//    contained one of `THREE.`, `PixiJS`, `deprecated`, `/src/`, `anima`… For these four it would
//    have matched — Playwright's `msg.text()` includes the stack, and the stack names
//    `/src/App.tsx`. But that is luck, not design: an error logged without an `Error` object, or
//    one thrown entirely inside a dependency, carries no `/src/` and would have been disowned. The
//    marker list was written for two *deprecation warning* streams and works well for those; for
//    errors it is the wrong question. An application error is ours until one of the two lists below
//    disowns it. A warning still has to look like ours, because the browser and the toolchain warn
//    about plenty we do not control.
// ---------------------------------------------------------------------------------------

export interface CapturedMessage {
  /** `console` message type, or `pageerror` for an uncaught exception. */
  type: string;
  text: string;
}

/** Noise from the toolchain and the host, which this project does not control. */
export const NOT_OURS = [
  '[vite]',
  'React DevTools',
  'Download the React DevTools',
  'GL Driver Message',
  'WebGL-0x',
  'Slow network is detected',
];

/**
 * Third-party deprecations this project cannot fix from its own code.
 *
 * Listed individually rather than pattern-matched away, so that adding to this list is a visible
 * act with a reason attached.
 *
 * `THREE.Clock` — constructed by `@react-three/fiber` itself (`dist/events-*.esm.js`, and typed as
 * `clock: THREE.Clock` on its store), not by anything in `src/`. three 0.184 deprecated the class in
 * favour of `THREE.Timer`. Silencing it needs react-three-fiber 9, which requires React 19 — a
 * framework upgrade, not a hardening step. It fires once per page rather than per frame, so it is
 * not the log-spam class of problem the deprecations this gate was built for were.
 */
export const ACCEPTED_THIRD_PARTY = ['THREE.Clock: This module has been deprecated'];

/**
 * Substrings that mark a **warning** as coming from Anima or a library Anima drives.
 *
 * Only warnings are filtered this way. See the header for why errors are not.
 */
export const OWNED_WARNING_MARKERS = [
  'THREE.',
  'PixiJS',
  'Graphics#',
  'deprecated',
  'Deprecation',
  '/src/',
  'anima',
  'Anima',
];

/** Whether this project is answerable for `msg`. */
export function isOwned(msg: CapturedMessage): boolean {
  if (NOT_OURS.some((n) => msg.text.includes(n))) return false;
  if (ACCEPTED_THIRD_PARTY.some((a) => msg.text.includes(a))) return false;
  // An error, or an uncaught exception, is ours unless one of the two lists above disowned it.
  if (msg.type === 'error' || msg.type === 'pageerror') return true;
  return OWNED_WARNING_MARKERS.some((m) => msg.text.includes(m));
}

/** A stable, de-duplicated rendering of what was captured, for a failure message. */
export function summarise(messages: CapturedMessage[]): string {
  const counts = new Map<string, number>();
  for (const m of messages) counts.set(m.text, (counts.get(m.text) ?? 0) + 1);
  return [...counts.entries()]
    .sort((a, b) => b[1] - a[1])
    .map(([text, n]) => `  ${n}x [${text.slice(0, 200)}]`)
    .join('\n');
}

/** Start recording warnings, errors and uncaught exceptions from `page`. */
export function watchConsole(page: Page): CapturedMessage[] {
  const seen: CapturedMessage[] = [];
  page.on('console', (m) => {
    const type = m.type();
    if (type !== 'warning' && type !== 'error') return;
    seen.push({ type, text: m.text() });
  });
  page.on('pageerror', (e) => seen.push({ type: 'pageerror', text: String(e) }));
  return seen;
}
