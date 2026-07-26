import { test, expect } from '@playwright/test';
import { installDeterministicTauri } from './tauri-mock';

// ---------------------------------------------------------------------------------------
// The app must not shout at its own console.
//
// # What this caught
//
// Two deprecation streams, both firing continuously while the scene ran rather than once at
// startup — so they were not stale advice, they were the renderer telling us the code had already
// stopped doing what it said:
//
//   THREE.WebGLShadowMap: PCFSoftShadowMap has been deprecated. Using PCFShadowMap instead.
//   Graphics#beginFill / #endFill / #lineStyle deprecated since PixiJS v8
//
// The Three one meant the soft shadow filter had not been in use for some time; the app asked for
// it, three substituted a different one, and the visual difference went unnoticed because nobody
// reads a wall of repeated warnings. The Pixi ones meant every redraw at 30 Hz was going through
// v7 compatibility stubs.
//
// # Why "owned" is the word
//
// A browser console is not ours alone. Vite's client logs over HMR, Chromium emits GPU driver
// performance messages, React advertises its devtools. Failing on all of it would make this gate
// something people disable. `isOwned` keeps it to messages that name our own dependencies or come
// from our own bundle, which is the set we can actually do something about.
// ---------------------------------------------------------------------------------------

/** Substrings that mark a console message as coming from Anima or a library Anima drives. */
const OWNED_MARKERS = [
  'THREE.',
  'PixiJS',
  'Graphics#',
  'deprecated',
  'Deprecation',
  '/src/',
  'anima',
  'Anima',
];

/** Noise from the toolchain and the host, which this project does not control. */
const NOT_OURS = [
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
 * One entry, and it is listed individually rather than pattern-matched away so that adding to
 * this list is a visible act with a reason attached.
 *
 * `THREE.Clock` — constructed by `@react-three/fiber` itself
 * (`dist/events-776716bd.esm.js:1308`, and typed as `clock: THREE.Clock` on its store), not by
 * anything in `src/`. three 0.184 deprecated the class in favour of `THREE.Timer`. Silencing it
 * needs react-three-fiber 9, which requires React 19 — a framework upgrade, not a hardening step,
 * and exactly the kind of change CLAUDE.md records as out of scope for a pass like this. It fires
 * once per page rather than per frame, so it is not the log-spam class of problem the other two
 * were.
 */
const ACCEPTED_THIRD_PARTY = [
  'THREE.Clock: This module has been deprecated',
];

interface Captured {
  type: string;
  text: string;
}

function isOwned(msg: Captured): boolean {
  if (NOT_OURS.some((n) => msg.text.includes(n))) return false;
  if (ACCEPTED_THIRD_PARTY.some((a) => msg.text.includes(a))) return false;
  return OWNED_MARKERS.some((m) => msg.text.includes(m));
}

function summarise(messages: Captured[]): string {
  const counts = new Map<string, number>();
  for (const m of messages) counts.set(m.text, (counts.get(m.text) ?? 0) + 1);
  return [...counts.entries()]
    .sort((a, b) => b[1] - a[1])
    .map(([text, n]) => `  ${n}x [${text.slice(0, 160)}]`)
    .join('\n');
}

function watch(page: import('@playwright/test').Page): Captured[] {
  const seen: Captured[] = [];
  page.on('console', (m) => {
    const type = m.type();
    if (type !== 'warning' && type !== 'error') return;
    seen.push({ type, text: m.text() });
  });
  page.on('pageerror', (e) => seen.push({ type: 'pageerror', text: String(e) }));
  return seen;
}

test('the 2D dashboard runs without warnings of its own', async ({ page }) => {
  const seen = watch(page);
  await installDeterministicTauri(page);

  await page.goto('/', { timeout: 30_000 });
  await expect(page.locator('h1')).toHaveText('Anima-Engine Control Center', { timeout: 15_000 });
  await expect(page.locator('canvas').first()).toBeVisible({ timeout: 15_000 });

  // The Pixi deprecations fired per redraw, so a single frame would under-report them. Give the
  // viewport time to run its draw loop.
  await page.waitForTimeout(3000);

  const owned = seen.filter(isOwned);
  expect(owned.length, `Anima-owned console output on the dashboard:\n${summarise(owned)}`).toBe(0);
});

test('the landscape scene runs without warnings of its own', async ({ page }) => {
  const seen = watch(page);

  await page.goto('/landscape.html', { timeout: 60_000 });
  // The world generates off-thread on a cold cache; wait for the scene, not a fixed delay.
  await expect(page.locator('[data-testid="world-showcase"] canvas').first()).toBeVisible({
    timeout: 180_000,
  });

  // The shadow-map warning fired on every shadow rebuild, which is per frame while the sun moves.
  await page.waitForTimeout(4000);

  const owned = seen.filter(isOwned);
  expect(
    owned.length,
    `Anima-owned console output on the landscape scene:\n${summarise(owned)}`,
  ).toBe(0);
});
