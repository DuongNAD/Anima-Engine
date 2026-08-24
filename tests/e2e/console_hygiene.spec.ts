import { test, expect } from '@playwright/test';
import { installDeterministicTauri } from './tauri-mock';
import { isOwned, summarise, watchConsole } from './console_policy';

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
//
// # Where the rule lives now
//
// `console_policy.ts`, because this spec was not the only page the suite opens. `global-setup.ts`
// warms the module graph by loading the dashboard, and on 2026-07-27 that page was throwing four
// `TypeError`s — it was the one dashboard load with no Tauri transport installed — while the run
// reported `18 passed`. The classifier was never the problem; nothing was applying it to that page.
// Sharing it is what makes "every browser-scope page is held to this standard" true rather than
// true-of-the-two-pages-this-file-happens-to-drive.
// ---------------------------------------------------------------------------------------

test('the 2D dashboard runs without warnings of its own', async ({ page }) => {
  const seen = watchConsole(page);
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
  // The per-test timeout in `playwright.config.ts` is 30 s, and this test's own waits already
  // declare that it needs far longer: 60 s to load, 180 s for a cold-cache world to generate
  // off-thread, then 4 s of frames. Those numbers were unreachable — the global cap killed the test
  // first — so what the spec asked for and what it got disagreed, and the disagreement was invisible
  // on a machine fast enough to finish inside 30 s anyway.
  //
  // CI is not that machine: `ubuntu-latest` renders this scene in software. Measured on run
  // 30282820349, twice: 31.6 s and 34.8 s against the 30 s cap, having passed at under 30 s on the
  // run before. A threshold that a change in load can cross is not a gate, it is a coin toss.
  //
  // This is not a threshold raised until the test passes. It is the test's own declared budget,
  // applied. A landscape that never renders still fails, on the 180 s wait below, and that failure
  // means what it says.
  test.setTimeout(240_000);

  const seen = watchConsole(page);

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
