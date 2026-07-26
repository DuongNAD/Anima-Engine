import { defineConfig, devices } from '@playwright/test';
import * as path from 'path';

// # The port this suite runs on, and why it is not 5173
//
// This config used to hard-code 5173 with `reuseExistingServer: !process.env.CI`, i.e. "locally,
// adopt whatever is already listening". That is not a theoretical hazard. Measured on this machine
// on 2026-07-27: port 5173 was held by a **Vite dev server belonging to `E:\Project\LIVA`**, an
// unrelated project. A local `npm run test:e2e` at that moment would have driven LIVA's app,
// navigated successfully, found none of Anima's DOM, and reported the mismatch as skips — a green
// wall that tested nothing.
//
// Two changes close it:
//
//   1. **Own port, never adopted.** `reuseExistingServer` is false everywhere, not just in CI, so
//      Playwright always starts the server it tests. The port defaults to 5177 rather than 5173 so
//      an ordinary `npm run dev` session can keep running alongside the suite; `strictPort` means a
//      collision is a loud failure rather than a silent hop to another port.
//   2. **Identity is proven, not assumed.** `global-setup.ts` fetches the served page and refuses to
//      continue unless it is Anima. A wrong app is a hard failure. It must never be a skip: a skip
//      is what let this go unnoticed.
//
// Override with `ANIMA_E2E_PORT` if 5177 is taken.
const PORT = Number(process.env.ANIMA_E2E_PORT ?? 5177);
const BASE_URL = `http://127.0.0.1:${PORT}`;

// The repo root: `tests/` is its own npm package and has no dev server of its own.
const REPO_ROOT = path.resolve(__dirname, '../..');

export default defineConfig({
  testDir: './',
  timeout: 30 * 1000,
  expect: {
    timeout: 5000,
  },
  reporter: 'list',
  // Pays for Vite's cold compile once, before any spec's navigation timeout starts, AND asserts the
  // server is serving Anima. See the file for both.
  globalSetup: path.resolve(__dirname, './global-setup.ts'),
  use: {
    headless: true,
    // Specs navigate with a relative path so the port lives in exactly one place.
    baseURL: BASE_URL,
  },
  webServer: {
    // `--strictPort` so a busy port fails instead of Vite silently choosing another one and the
    // suite then testing nothing at the URL it believes in.
    command: `npm run dev -- --port ${PORT} --strictPort`,
    cwd: REPO_ROOT,
    url: BASE_URL,
    // Never adopt a server this suite did not start. See the note at the top of this file.
    reuseExistingServer: false,
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

export { BASE_URL, PORT };
