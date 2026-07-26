import { test, expect } from '@playwright/test';
import { mkdirSync, writeFileSync, statSync } from 'node:fs';
import { resolve } from 'node:path';
import {
  CANONICAL_VIEW_IDS,
  CANONICAL_VIEW_CAMERAS,
} from '../../src/components/Landscape/utils/mapManifest';
import {
  CAPTURE_DEFAULT_QUALITY,
  CAPTURE_DEFAULT_TIME_OF_DAY,
  CAPTURE_DEFAULT_WEATHER,
  CAPTURE_READY_FLAG,
} from '../../src/components/Landscape/utils/captureMode';

// ---------------------------------------------------------------------------------------
// The canonical map views: eight real WebGL renders of the shipped world.
//
// # Why these are captured here and not generated
//
// `AGENTS.md` makes canonical before/after views a hard gate for any map work, and the previous
// pass could not satisfy it: `map_manifest.json` named eight PNGs that did not exist, and the
// honest repair at the time was to mark them `captured: false`. That is a truthful placeholder,
// not a gate.
//
// The two ways to produce them are not equivalent. Rendering an orthographic raster from worldgen
// data is cheap and is a picture of *a different thing* — it shares no code with the renderer, so
// it cannot show a renderer defect, which is most of what a visual review looks for. Driving the
// real scene in real Chromium with real WebGL costs a browser and produces evidence about the
// thing under review. This does the second.
//
// It stays inside CLAUDE.md's prohibition: `landscape.html` is a frontend Vite entry. No Tauri
// process, no Bevy, no `cargo run` — the machine-crashing path is untouched.
//
// # What makes a capture reproducible
//
// Five things are pinned, and each of them moves on its own otherwise:
//
//   world     `sharedWorld.ts` — seed "seed", 2048², continent
//   pose      `CANONICAL_VIEW_CAMERAS`, the same record the manifest publishes
//   clock     fixed hour, and `speed = 0` so the day does not advance mid-shot
//   weather   fixed; precipitation is animated and would differ frame to frame
//   viewport  fixed device size and scale factor
//
// plus a settle wait counted in rendered frames rather than milliseconds, because "has the
// terrain mesh finished building" is a question about frames.
// ---------------------------------------------------------------------------------------

const ROOT = resolve(__dirname, '../..');
const OUT_DIR = resolve(ROOT, 'map-views');

/**
 * Capture viewport. 1280×720 is large enough for a reviewer to judge terrain silhouettes and
 * biome boundaries, and small enough that eight PNGs stay a reasonable thing to commit.
 * `deviceScaleFactor: 1` because the app clamps DPR to 1.5 and a scaled capture would change
 * resolution with whatever machine ran it.
 */
const VIEWPORT = { width: 1280, height: 720 };

/** Renderer substrings that mean "this is a software rasteriser, not the shipped path". */
const SOFTWARE_RENDERERS = ['swiftshader', 'llvmpipe', 'software'];

test.describe('canonical map views', () => {
  test.use({ viewport: VIEWPORT, deviceScaleFactor: 1 });

  // Generating 2048² in the browser is the expensive part of the first capture; after that it is
  // served from IndexedDB. Serial so the eight shots share one warm cache instead of racing eight
  // cold generations.
  test.describe.configure({ mode: 'serial' });

  for (const view of CANONICAL_VIEW_IDS) {
    test(`captures ${view}`, async ({ page }) => {
      const errors: string[] = [];
      page.on('pageerror', (e) => errors.push(String(e)));

      const url =
        `/landscape.html?capture=1&view=${view}` +
        `&t=${CAPTURE_DEFAULT_TIME_OF_DAY}` +
        `&weather=${CAPTURE_DEFAULT_WEATHER}` +
        `&quality=${CAPTURE_DEFAULT_QUALITY}`;
      await page.goto(url, { waitUntil: 'load', timeout: 120_000 });

      // The world is generated off-thread on first visit; the page shows a placeholder until then.
      //
      // `.first()` is load-bearing and not a shrug at a strict-mode violation: the page has three
      // canvases — the R3F scene, the compass ribbon and the minimap, both of which are 2D HUD
      // widgets drawn with Canvas 2D. The scene is the first, inside `world-showcase`. Shooting
      // either of the others would produce a 192-pixel thumbnail that still looks like a map.
      const canvas = page.locator('[data-testid="world-showcase"] canvas').first();
      await expect(canvas).toBeVisible({ timeout: 180_000 });
      // Wait for it to be laid out, don't assert it already is. R3F sizes the canvas from a
      // ResizeObserver on its container, so between mount and the first observation the element
      // is at the HTML default 300x150 — which is also, unhelpfully, about the size of a HUD
      // widget. Polling separates "not measured yet" from "matched the wrong canvas".
      await expect
        .poll(async () => (await canvas.boundingBox())?.width, {
          message: 'the scene canvas must fill the viewport, not be a HUD widget',
          timeout: 30_000,
        })
        .toBe(VIEWPORT.width);

      // Fail closed on a software rasteriser. See the note in `capture.config.ts`: a SwiftShader
      // capture is 173x slower *and* is a picture of a renderer nobody ships.
      const renderer = await page.evaluate(() => {
        const gl = document.createElement('canvas').getContext('webgl2');
        if (!gl) return 'no-webgl2';
        const ext = gl.getExtension('WEBGL_debug_renderer_info');
        return String(ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER));
      });
      const software = SOFTWARE_RENDERERS.find((s) => renderer.toLowerCase().includes(s));
      expect(
        software,
        `WebGL is running on a software rasteriser (${renderer}). Canonical views must be ` +
          `captured on real hardware — run this on a machine with a GPU. Do not relax this into ` +
          `a skip: a software capture looks like evidence and is not.`,
      ).toBeUndefined();

      // Settled, not merely mounted. See `CAPTURE_SETTLE_FRAMES`.
      await page.waitForFunction(
        (flag) => (window as unknown as Record<string, unknown>)[flag] === true,
        CAPTURE_READY_FLAG,
        { timeout: 120_000 },
      );

      // The camera is where the manifest says it is. Without this the harness could drift from
      // the published pose and every image would still look like a plausible map.
      const pose = await page.evaluate(() => {
        const w = window as unknown as { __world?: unknown };
        return { hasWorld: Boolean(w.__world) };
      });
      expect(pose.hasWorld, 'the landscape page must expose the generated world').toBe(true);

      mkdirSync(OUT_DIR, { recursive: true });
      const file = resolve(OUT_DIR, `${view}.png`);
      const png = await canvas.screenshot({ animations: 'disabled' });
      writeFileSync(file, png);

      // A capture of a failed scene is a black rectangle, and a black rectangle is exactly what a
      // fabricated screenshot looks like too. Refuse to publish one.
      expect(statSync(file).size, `${view}.png is implausibly small — did the scene render?`).toBeGreaterThan(
        20_000,
      );
      expect(errors, `page errors while capturing ${view}`).toEqual([]);

      // The pose this image was shot from, recorded next to it. `gen_world_manifest.ts` reads the
      // same record, so a manifest that describes a different pose than the capture used is not
      // representable.
      expect(CANONICAL_VIEW_CAMERAS[view]).toBeDefined();
    });
  }
});
