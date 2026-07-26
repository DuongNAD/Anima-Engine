import { defineConfig, devices } from '@playwright/test';
import * as path from 'path';

// ---------------------------------------------------------------------------------------
// Canonical map view capture — a separate config, and separate on purpose.
//
// # Why this is not part of `npm run test:e2e`
//
// It needs a GPU. Measured on this machine, 2026-07-27, capturing the shipped 2048² world with
// ~117 000 flora instances and the shadow pass on:
//
//   default headless Chromium   0.27 fps   ANGLE / Vulkan / SwiftShader (software)
//   --use-angle=d3d11          46.73 fps   ANGLE / NVIDIA RTX 5060 Ti / Direct3D11
//
// A 173x difference. Eight views at 0.27 fps is over half an hour of software rasterisation, and
// what it produces is not even the right picture: SwiftShader is a different rasteriser, so a
// capture made on it is evidence about a renderer nobody runs.
//
// So this is a **producer**, run deliberately on a machine with a GPU
// (`npm run capture:views`), and the resulting PNGs are committed. What CI verifies is the
// artifact: `mapManifestEvidence.test.ts` checks every view claiming `captured: true` exists and
// hashes to the checksum the manifest declares. That is a gate a GPU-less runner can hold.
//
// # Why it is fail-closed about the GPU
//
// `canonical_views.spec.ts` reads back `UNMASKED_RENDERER_WEBGL` and fails if it sees a software
// renderer. Falling back quietly would produce eight plausible-looking images from the wrong
// rasteriser — which is precisely the class of "looks like evidence, is not" this pass exists to
// remove.
// ---------------------------------------------------------------------------------------

const PORT = Number(process.env.ANIMA_CAPTURE_PORT ?? 5178);
const BASE_URL = `http://127.0.0.1:${PORT}`;
const REPO_ROOT = path.resolve(__dirname, '../..');

export default defineConfig({
  testDir: './',
  testMatch: /canonical_views\.spec\.ts/,
  // A cold capture pays for generating 2048² in the browser before the first shot.
  timeout: 5 * 60 * 1000,
  expect: { timeout: 10_000 },
  reporter: 'list',
  // One worker: the eight views share the IndexedDB world cache, and parallel workers would each
  // generate their own 2048² world.
  workers: 1,
  use: {
    headless: true,
    baseURL: BASE_URL,
    launchOptions: {
      // Real GPU, not SwiftShader. See the note above for the measurement.
      args: [
        '--use-gl=angle',
        '--use-angle=d3d11',
        '--enable-gpu',
        '--ignore-gpu-blocklist',
      ],
    },
  },
  webServer: {
    command: `npm run dev -- --port ${PORT} --strictPort`,
    cwd: REPO_ROOT,
    url: BASE_URL,
    reuseExistingServer: false,
    timeout: 120 * 1000,
    stdout: 'pipe',
    stderr: 'pipe',
  },
  projects: [{ name: 'canonical-capture', use: { ...devices['Desktop Chrome'] } }],
});

export { BASE_URL, PORT };
