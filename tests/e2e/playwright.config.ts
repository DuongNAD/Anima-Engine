import { defineConfig, devices } from '@playwright/test';
import * as path from 'path';

// Every spec here navigates to the Vite dev server on 5173, and until this config grew a `webServer`
// block nothing started one — so each spec hit its `catch` and called `test.skip()`. Combined with
// there being no `test:e2e` script at the repo root and no CI step, the whole directory was inert.
//
// Two of the seven specs (phase3_adversarial, phase6_adversarial_stress) stub the Tauri IPC inside
// the page via `addInitScript` and need nothing but this server; they are real gates now. The other
// five spawn `src-tauri/target/release/anima-engine` and skip themselves when it is absent, which is
// what happens on a machine that has not run `cargo build --release --features desktop`. That is
// reported as a skip rather than a pass — see the note in the CI workflow.
export default defineConfig({
  testDir: './',
  timeout: 30 * 1000,
  expect: {
    timeout: 5000,
  },
  reporter: 'list',
  // Pays for Vite's cold compile once, before any spec's 5 s navigation timeout starts. See the
  // file for why the specs otherwise skipped themselves claiming the server was unreachable.
  globalSetup: path.resolve(__dirname, './global-setup.ts'),
  use: {
    headless: true,
    baseURL: 'http://localhost:5173',
  },
  webServer: {
    command: 'npm run dev',
    // The repo root: `tests/` is its own npm package and has no dev server of its own.
    cwd: path.resolve(__dirname, '../..'),
    url: 'http://localhost:5173',
    // vite.config.ts sets strictPort on 5173, so a second server cannot come up alongside a running
    // one. Locally that means reuse; in CI there is never one to reuse and a stray listener on 5173
    // should fail loudly rather than be silently adopted.
    reuseExistingServer: !process.env.CI,
    timeout: 120 * 1000,
    stdout: 'pipe',
    stderr: 'pipe',
  },
  projects: [
    {
      name: 'tauri-e2e',
      use: {
        ...devices['Desktop Chrome'],
      },
    },
  ],
});
