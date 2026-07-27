import { test, expect } from '@playwright/test';
import { installDeterministicTauri } from './tauri-mock';
import type { TauriMockHandle } from './tauri-mock';

// ---------------------------------------------------------------------------------------
// What used to be five "live IPC" specs, made honest.
//
// # What they actually did
//
// Each one spawned `src-tauri/target/release/anima-engine`, slept a second, then drove an
// ordinary Vite page with Playwright. Nothing connected the two: the page was served over HTTP
// with no `__TAURI_INTERNALS__` in it, so every `invoke` rejected, while the spawned process
// rendered into a webview Playwright never touched. When the resulting assertions failed, the
// specs caught the error and called `test.skip()`. So a suite that proved nothing about IPC in
// either process reported as green-with-skips, and the CI workflow separately described these
// specs as using a page-level IPC stub and needing no binary — a contradiction nobody could see
// because the run was always green.
//
// # What they do now
//
// One browser scope, one contract. `installDeterministicTauri` implements the three functions
// `@tauri-apps/api` actually routes through, with fixed replies stated in that file, so the real
// frontend runs against a real transport whose answers are known. That proves what a browser test
// can prove: the UI issues the commands the contract names, renders their payloads, and reacts to
// events.
//
// **Zero skips.** Absent UI is a failure. `global-setup.ts` has already proven the served app is
// Anima, so a missing heading means the app is broken, not that the environment is wrong — and
// converting that into a skip is the exact mechanism that hid the problem above.
//
// Genuine backend coverage lives in `real_backend.spec.ts`, which is only declared when it is
// explicitly required, so it fails closed rather than skipping.
// ---------------------------------------------------------------------------------------

const APP_TITLE = 'Anima-Engine Control Center';

let mock: TauriMockHandle;

test.beforeEach(async ({ page }) => {
  mock = await installDeterministicTauri(page);
  await page.goto('/', { timeout: 30_000 });
  // Every spec below depends on the app having rendered. Asserting it once, here, means a broken
  // app fails every spec loudly instead of each one deciding for itself what to do about it.
  await expect(page.locator('h1')).toHaveText(APP_TITLE, { timeout: 15_000 });
});

test('the dashboard queries the simulation status contract and renders it', async ({ page }) => {
  // The command surface `PROJECT.md` documents, exercised through the real frontend.
  await expect
    .poll(async () => (await mock.invokedCommands()).includes('get_simulation_status'), {
      message: 'the app must poll get_simulation_status',
      timeout: 15_000,
    })
    .toBe(true);

  // 1234 is the tick count the mock returns; seeing it on screen proves the payload reached the
  // UI rather than merely that a command was issued.
  await expect(page.getByText('1234', { exact: false }).first()).toBeVisible({ timeout: 15_000 });

  const statusHeader = page.locator('h2', {
    hasText: 'Trạng thái Mô phỏng (Simulation Status)',
  });
  await expect(statusHeader).toBeVisible();
  await expect(
    page.locator('h2', { hasText: 'Bảng đo lường từ xa (5 Agents đầu tiên)' }),
  ).toBeVisible();
});

test('Phase 3: the telemetry panel is laid out and subscribes to its event streams', async ({
  page,
}) => {
  await expect(page.locator('canvas').first()).toBeVisible();

  const phase3Panel = page.locator('[data-testid="phase3-panel"]');
  await expect(phase3Panel).toBeVisible();
  await expect(phase3Panel.locator('h2')).toHaveText(
    'Phase 3: Socialization & Emergent Behaviors',
  );
  await expect(phase3Panel.locator('h3', { hasText: 'Pheromone Heatmap' })).toBeVisible();
  await expect(phase3Panel.locator('h3', { hasText: 'Sensor Beams (Raycasts)' })).toBeVisible();
  await expect(phase3Panel.locator('h3', { hasText: 'Combat Event Log' })).toBeVisible();
  await expect(page.locator('[data-testid="combat-log"]')).toBeVisible();

  // The subscriptions themselves are IPC: `listen()` compiles to `plugin:event|listen`.
  await expect
    .poll(async () => (await mock.invokedCommands()).includes('plugin:event|listen'), {
      message: 'the app must subscribe to backend events',
      timeout: 15_000,
    })
    .toBe(true);
});

test('Phase 4: lineage, chronicle and migration panels render', async ({ page }) => {
  await expect(page.locator('[data-testid="lineage-svg-container"]')).toBeVisible();

  const chroniclePanel = page.locator('[data-testid="chronicle-timeline-panel"]');
  await expect(chroniclePanel).toBeVisible();
  await expect(chroniclePanel.locator('h2')).toContainText('Mother Nature Chronicle');

  await expect(page.locator('[data-testid="migration-panel"]')).toBeVisible();
});

test('Phase 5: the Pixi viewport mounts alongside the chronicle and telemetry panels', async ({
  page,
}) => {
  await expect(page.locator('canvas').first()).toBeVisible();

  const chroniclePanel = page.locator('[data-testid="chronicle-timeline-panel"]');
  await expect(chroniclePanel).toBeVisible();
  await expect(chroniclePanel.locator('h2')).toContainText('Mother Nature Chronicle');

  await expect(page.locator('[data-testid="phase3-panel"]')).toBeVisible();
});

test('Phase 6: persistence, camera and environmental controls are present', async ({ page }) => {
  await expect(page.locator('[data-testid="save-state-button"]')).toBeVisible();
  await expect(page.locator('[data-testid="load-state-button"]')).toBeVisible();
  await expect(page.locator('[data-testid="filepath-input"]')).toBeVisible();

  await expect(page.locator('[data-testid="zoom-in-button"]')).toBeVisible();
  await expect(page.locator('[data-testid="zoom-out-button"]')).toBeVisible();
  await expect(page.locator('[data-testid="pan-button"]')).toBeVisible();

  await expect(page.locator('[data-testid="environmental-elements-container"]')).toBeVisible();
});

test('a pre-confinement save can be found, chosen and imported from the UI', async ({ page }) => {
  // The migration end to end, through the real frontend.
  //
  // `list_legacy_saves` and `import_legacy_save` were registered as commands with no caller: no way
  // to learn where the drop directory is, no way to see what is in it, no way to name the result. A
  // command nobody can invoke is not a feature, and nothing in the backend's own tests could say so.
  const panel = page.locator('[data-testid="legacy-import-panel"]');
  await expect(panel).toBeHidden();

  await page.locator('[data-testid="legacy-import-open"]').click();
  await expect(panel).toBeVisible();

  // Where to put the old file. This is the whole reason the panel exists: the authorising act is a
  // copy the page cannot perform, so the user has to be told the destination.
  await expect(page.locator('[data-testid="legacy-import-dir"]')).toContainText('legacy-import');
  await expect
    .poll(async () => (await mock.invokedCommands()).includes('list_legacy_saves'), {
      timeout: 15_000,
    })
    .toBe(true);

  // Files present but not importable are reported, not hidden.
  await expect(page.locator('[data-testid="legacy-import-ignored"]')).toContainText('notes.txt');

  await page.locator('[data-testid="legacy-import-select"]').selectOption('second_world.json');
  await page.locator('[data-testid="legacy-import-save-as"]').fill('restored');
  await page.locator('[data-testid="legacy-import-run"]').click();

  await expect
    .poll(
      async () =>
        (await mock.invocations()).find((c) => c.cmd === 'import_legacy_save')?.args ?? null,
      {
        message: 'the panel must invoke import_legacy_save with the chosen file and destination',
        timeout: 15_000,
      },
    )
    .toEqual({ legacy_name: 'second_world.json', save_as: 'restored' });

  // The name the backend says it wrote lands in the save field, so the next action — Load State —
  // works without the user retyping a name they were never shown.
  await expect(page.locator('[data-testid="legacy-import-ok"]')).toContainText('restored.json');
  await expect(page.locator('[data-testid="filepath-input"]')).toHaveValue('restored.json');
});

test('a chronicle event pushed over the event channel reaches the UI', async ({ page }) => {
  // The half a command-only test cannot reach: the app has to be *listening*, and it has to
  // render what arrives. The previous specs could not test this at all — their pages had no
  // transport, so no subscription ever completed.
  await expect(page.locator('[data-testid="chronicle-timeline-panel"]')).toBeVisible();

  await expect
    .poll(async () => (await mock.invokedCommands()).includes('plugin:event|listen'), {
      timeout: 15_000,
    })
    .toBe(true);

  await mock.emit('chronicle-event', {
    id: 'evt-e2e-1',
    tick: 4242,
    title: 'E2E deterministic chronicle entry',
    description: 'Emitted by the browser-scope IPC mock.',
    severity: 'info',
  });

  await expect(page.getByText('E2E deterministic chronicle entry').first()).toBeVisible({
    timeout: 15_000,
  });
});
