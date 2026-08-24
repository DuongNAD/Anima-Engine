// Test-only DOM metadata for the R3F landscape components.
//
// The Vitest suite renders these components with `@react-three/fiber` mocked, so the
// intrinsic elements (`<group>`, `<mesh>`, ...) become plain jsdom DOM nodes and the
// tests assert on `data-*` attributes. The REAL react-three-fiber renderer, however,
// parses any prop containing "-" as a nested property path (e.g. `rotation-x`), so a
// `data-weather` prop is read as `instance.data.weather` and throws at runtime.
//
// `testAttrs` returns the `data-*` map only under jsdom (the test environment) and an
// empty object in a real browser, so the components stay testable without crashing R3F.
export function testAttrs(
  attrs: Record<string, string | number>,
): Record<string, string | number> {
  if (
    typeof navigator !== 'undefined' &&
    typeof navigator.userAgent === 'string' &&
    navigator.userAgent.includes('jsdom')
  ) {
    return attrs;
  }
  return {};
}
