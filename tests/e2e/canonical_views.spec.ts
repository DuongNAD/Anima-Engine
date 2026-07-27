import { test, expect } from '@playwright/test';
import { mkdirSync, writeFileSync, readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
import type { Browser, Page } from '@playwright/test';
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
import { MAP_EVIDENCE_GLOBAL } from '../../src/components/Landscape/utils/mapEvidence';
import { encodeCanonicalPng, type RawFramebuffer } from './canonicalImage';

// ---------------------------------------------------------------------------------------
// The canonical map views: eight real WebGL renders of the shipped world.
//
// # Why these are captured here and not generated
//
// `AGENTS.md` makes canonical before/after views a hard gate for any map work, and an earlier pass
// could not satisfy it: `map_manifest.json` named eight PNGs that did not exist, and the honest
// repair at the time was to mark them `captured: false`. That is a truthful placeholder, not a gate.
//
// The two ways to produce them are not equivalent. Rendering an orthographic raster from worldgen
// data is cheap and is a picture of *a different thing* — it shares no code with the renderer, so it
// cannot show a renderer defect, which is most of what a visual review looks for. Driving the real
// scene in real Chromium with real WebGL costs a browser and produces evidence about the thing under
// review. This does the second.
//
// It stays inside CLAUDE.md's prohibition: `landscape.html` is a frontend Vite entry. No Tauri
// process, no Bevy, no `cargo run` — the machine-crashing path is untouched.
//
// # The acceptance criterion
//
// **Every view is shot twice, from two fresh browser contexts, and the two saved PNGs must be
// byte-identical.** Same bytes, same SHA-256, or the view is not written at all.
//
// An earlier pass measured a handful of pixels differing by one level of 255 between runs, decided
// the GPU's MSAA resolve was responsible, and shipped two tolerance constants plus a pixel-difference
// comparator. All of it is gone. `captureMode.ts` records why the reasoning was wrong and lists the
// sources of variance that were removed instead — no multisample resolve, no dithering, no undefined
// buffer read, no alpha compositing, a stopped render loop and exactly one final render, a fixed
// clock, a fixed frame delta, seeded randomness, and the harness's own PNG encoder so the saved bytes
// are a function of the framebuffer alone.
//
// The two shots are *equivalently* clean: each gets its own browser context, so each has an empty
// IndexedDB, generates the 2048² world from scratch, and runs a fresh JS realm and render clock.
// Neither is a warm re-load of the other.
//
// # What is pinned, and where
//
//   world       `sharedWorld.ts` — seed "seed", 2048², continent
//   pose        `CANONICAL_VIEW_CAMERAS`, the same record the manifest publishes
//   day clock   fixed hour, and `speed = 0` so the day does not advance mid-shot
//   render clock`sceneClock.sceneElapsed` — a fixed 12.5 s for every animated component
//   frame delta `sceneClock.sceneDelta` — a fixed step for everything that integrates one
//   randomness  `sceneClock.makeSceneRandom` — seeded streams for stars and precipitation
//   weather     fixed; precipitation is animated and would differ frame to frame
//   viewport    fixed device size and scale factor
//   frame count the loop stops on frame 90 and renders once more; the read is always of frame 91
// ---------------------------------------------------------------------------------------

const ROOT = resolve(__dirname, '../..');
const OUT_DIR = resolve(ROOT, 'map-views');
const EVIDENCE_PATH = resolve(ROOT, 'artifacts/map_evidence.json');

/**
 * Capture viewport. 1280×720 is large enough for a reviewer to judge terrain silhouettes and biome
 * boundaries, and small enough that eight PNGs stay a reasonable thing to commit.
 * `deviceScaleFactor: 1` because the app clamps DPR to 1.5 and a scaled capture would change
 * resolution with whatever machine ran it.
 */
const VIEWPORT = { width: 1280, height: 720 };

/** Renderer substrings that mean "this is a software rasteriser, not the shipped path". */
const SOFTWARE_RENDERERS = ['swiftshader', 'llvmpipe', 'software'];

/**
 * A capture of a failed scene is a black rectangle, and a black rectangle is also what a fabricated
 * screenshot looks like. Nothing this small is a render of a 2048² world.
 */
const MIN_PLAUSIBLE_BYTES = 20_000;

/**
 * The navigation/collision evidence record, injected into the page before it loads.
 *
 * Read from disk rather than bundled into the app: it is evidence about the world, it has no business
 * in `dist/`, and injecting it is what binds the image to the record — the overlay draws this
 * polyline or it draws nothing.
 */
const EVIDENCE = JSON.parse(readFileSync(EVIDENCE_PATH, 'utf8')) as unknown;

const sha = (b: Buffer): string => createHash('sha256').update(b).digest('hex');

/** Everything a capture needs done to a fresh page, before any of the app's code runs. */
async function preparePage(page: Page): Promise<string[]> {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(
    ([flag, record]) => {
      (window as unknown as Record<string, unknown>)[flag as string] = record;
    },
    [MAP_EVIDENCE_GLOBAL, EVIDENCE] as [string, unknown],
  );
  return errors;
}

/** One capture: the encoded PNG and anything that went wrong while producing it. */
interface Shot {
  png: Buffer;
  width: number;
  height: number;
  errors: string[];
}

/** Drive one canonical view to its settled frame and encode the framebuffer. */
async function shoot(page: Page, view: string, errors: string[]): Promise<Shot> {
  const url =
    `/landscape.html?capture=1&view=${view}` +
    `&t=${CAPTURE_DEFAULT_TIME_OF_DAY}` +
    `&weather=${CAPTURE_DEFAULT_WEATHER}` +
    `&quality=${CAPTURE_DEFAULT_QUALITY}`;
  await page.goto(url, { waitUntil: 'load', timeout: 120_000 });

  // The world is generated off-thread on first visit; the page shows a placeholder until then.
  //
  // `.first()` is load-bearing and not a shrug at a strict-mode violation: the page has three
  // canvases — the R3F scene, the compass ribbon and the minimap, the last two being 2D HUD widgets.
  // The scene is the first, inside `world-showcase`. Shooting either of the others would produce a
  // 192-pixel thumbnail that still looks like a map.
  const canvas = page.locator('[data-testid="world-showcase"] canvas').first();
  await expect(canvas).toBeVisible({ timeout: 180_000 });
  // Wait for it to be laid out, don't assert it already is. R3F sizes the canvas from a
  // ResizeObserver on its container, so between mount and the first observation the element is at the
  // HTML default 300x150 — which is also, unhelpfully, about the size of a HUD widget. Polling
  // separates "not measured yet" from "matched the wrong canvas".
  await expect
    .poll(async () => (await canvas.boundingBox())?.width, {
      message: 'the scene canvas must fill the viewport, not be a HUD widget',
      timeout: 30_000,
    })
    .toBe(VIEWPORT.width);

  // Fail closed on a software rasteriser. See the note in `capture.config.ts`: a SwiftShader capture
  // is 173x slower *and* is a picture of a renderer nobody ships.
  const renderer = await page.evaluate(() => {
    const gl = document.createElement('canvas').getContext('webgl2');
    if (!gl) return 'no-webgl2';
    const ext = gl.getExtension('WEBGL_debug_renderer_info');
    return String(ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER));
  });
  const software = SOFTWARE_RENDERERS.find((s) => renderer.toLowerCase().includes(s));
  expect(
    software,
    `WebGL is running on a software rasteriser (${renderer}). Canonical views must be captured on ` +
      `real hardware — run this on a machine with a GPU. Do not relax this into a skip: a software ` +
      `capture looks like evidence and is not.`,
  ).toBeUndefined();

  // Settled, stopped, and rendered once more. See `CaptureReadySignal` in `WorldShowcase.tsx`: the
  // flag is set last, after `setFrameloop('never')` and the final `gl.render`, so observing it means
  // the drawing buffer holds a frame nothing will overwrite.
  await page.waitForFunction(
    (flag) => (window as unknown as Record<string, unknown>)[flag] === true,
    CAPTURE_READY_FLAG,
    { timeout: 120_000 },
  );

  // The world the manifest describes is in the page, and — for the two views that have one — so is
  // the evidence record the overlay draws. An injection that silently failed would produce a
  // `navigation` view with no route in it, which is the exact defect this overlay repairs.
  const state = await page.evaluate((flag) => {
    const w = window as unknown as { __world?: unknown } & Record<string, unknown>;
    return { hasWorld: Boolean(w.__world), hasEvidence: Boolean(w[flag]) };
  }, MAP_EVIDENCE_GLOBAL);
  expect(state.hasWorld, 'the landscape page must expose the generated world').toBe(true);
  expect(state.hasEvidence, 'the evidence record must reach the page').toBe(true);

  // The drawing buffer itself, read in the page. No `requestAnimationFrame` wait: with the loop
  // stopped there is no next frame to wait for, and `preserveDrawingBuffer` keeps this content valid
  // for as long as it takes to read it.
  const read = await page.evaluate(async () => {
    const el = document.querySelector('[data-testid="world-showcase"] canvas') as HTMLCanvasElement;
    const gl = (el.getContext('webgl2') ?? el.getContext('webgl')) as WebGLRenderingContext;
    const w = gl.drawingBufferWidth;
    const h = gl.drawingBufferHeight;
    const buf = new Uint8Array(w * h * 4);
    gl.readPixels(0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, buf);
    let bin = '';
    const CHUNK = 0x8000;
    for (let i = 0; i < buf.length; i += CHUNK) {
      bin += String.fromCharCode(...buf.subarray(i, i + CHUNK));
    }
    return { w, h, b64: btoa(bin) };
  });

  // A drawing buffer at some other resolution still encodes to a valid PNG, and to a canonical view
  // that no longer matches the resolution the manifest publishes.
  expect(
    { width: read.w, height: read.h },
    'the drawing buffer must be exactly the canonical capture resolution',
  ).toEqual(VIEWPORT);

  const fb: RawFramebuffer = {
    width: read.w,
    height: read.h,
    rgba: Buffer.from(read.b64, 'base64'),
  };
  return { png: encodeCanonicalPng(fb), width: fb.width, height: fb.height, errors };
}

/** Shoot `view` in a brand-new browser context: empty IndexedDB, fresh realm, cold world. */
async function shootClean(browser: Browser, view: string): Promise<Shot> {
  const ctx = await browser.newContext({ viewport: VIEWPORT, deviceScaleFactor: 1 });
  try {
    const page = await ctx.newPage();
    const errors = await preparePage(page);
    return await shoot(page, view, errors);
  } finally {
    await ctx.close();
  }
}

test.describe('canonical map views', () => {
  test.use({ viewport: VIEWPORT, deviceScaleFactor: 1 });

  // Serial: each shot generates a 2048² world, and parallel workers would compete for the GPU as well
  // as the CPU. It also keeps the console output in view order.
  test.describe.configure({ mode: 'serial' });

  for (const view of CANONICAL_VIEW_IDS) {
    test(`captures ${view}, identically, twice`, async ({ browser }) => {
      const first = await shootClean(browser, view);
      const second = await shootClean(browser, view);

      expect(first.errors, `page errors on the first capture of ${view}`).toEqual([]);
      expect(second.errors, `page errors on the second capture of ${view}`).toEqual([]);

      const a = sha(first.png);
      const b = sha(second.png);
      console.log(`${view}: sha256 ${a} (${first.png.length} bytes), second run ${b}`);
      expect(
        b,
        `${view} rendered differently on two equivalently clean loads. That is a reproducibility ` +
          `failure, not an acceptable variation, and it is not to be absorbed by a tolerance: find ` +
          `the source. The known sources are enumerated in ` +
          `src/components/Landscape/utils/captureMode.ts — an unpinned clock or delta, an unseeded ` +
          `random, a per-frame eased value, or a context flag that stopped being capture-only.`,
      ).toBe(a);

      expect(
        first.png.length,
        `${view}.png is implausibly small — did the scene render?`,
      ).toBeGreaterThan(MIN_PLAUSIBLE_BYTES);

      // Written only after the two runs agree, so no file on disk is ever an unreproduced capture.
      mkdirSync(OUT_DIR, { recursive: true });
      writeFileSync(resolve(OUT_DIR, `${view}.png`), first.png);

      // The pose this image was shot from, recorded next to it. `gen_world_manifest.ts` reads the
      // same record, so a manifest that describes a different pose than the capture used is not
      // representable.
      expect(CANONICAL_VIEW_CAMERAS[view]).toBeDefined();
    });
  }
});
