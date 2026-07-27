// Which runtime the browser bundle finds itself in.
//
// A component that behaves differently under Vitest is a compromise, not a design — `Minimap` caps
// its grid at 100x100 there because a 2048-cell scan per animation frame under jsdom is minutes of
// wall clock. But the *detection* was `(globalThis as any).process?.env?.VITEST`, and `any` there is
// doing more than it looks: `process` genuinely does not exist in a browser, so every step of that
// chain has to be optional, and `any` made all of them optional by making none of them checked.

/**
 * The Node-ish globals a browser bundle may find itself next to.
 *
 * Every member optional, because in a real browser none of them are there. Vitest runs the bundle in
 * jsdom with Node's `process` still in scope, which is exactly what makes this detectable at all.
 */
interface MaybeNodeGlobals {
  process?: { env?: Record<string, string | undefined> };
}

/** True when this bundle is running inside the Vitest jsdom environment. */
export function isUnderVitest(): boolean {
  if (typeof globalThis === 'undefined') return false;
  return Boolean((globalThis as MaybeNodeGlobals).process?.env?.VITEST);
}
