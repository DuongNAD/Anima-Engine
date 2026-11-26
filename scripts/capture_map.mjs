#!/usr/bin/env node
// ---------------------------------------------------------------------------------------
// Map visual check — PROTOTYPE. `npm run capture:map -- [flags]`
//
// Opens `landscape.html` with the synthetic creature fixture in a real browser, flies the camera to
// about eight views derived from the generated world (overview, horizon, coast, highest peak, river
// valley, forest, waterfall, creature close-up), and writes per view a PNG, frame-rate figures and
// the renderer's draw-call/triangle counters, plus the console errors of the whole run. A contact
// sheet (`index.html`) puts it all on one page; `--baseline <previous run dir>` puts the previous run
// beside it, view by view, with FPS deltas.
//
// What it is for: reviewing a visual change to `src/components/Landscape/` without a human taking
// screenshots by hand — run once before the change, once after with `--baseline`, and look.
//
// What it is NOT:
//   * not a gate. Nothing here passes or fails on what the picture looks like.
//   * not the canonical-view evidence (`npm run capture:views`, byte-identical, fixed poses). These
//     views move with the world on purpose and the scene keeps animating while it is captured.
//   * not the shipped app. This is `landscape.html` in Chrome under Vite dev, not the Tauri webview,
//     and FPS on this machine says nothing about the Iris Xe target. Headless FPS is indicative only:
//     the run records the WebGL renderer string and flags a software rasteriser (SwiftShader).
//   * not simulation output. Creatures come from `?replay=synthetic` — procedural fixture data.
//
// How the camera is placed: the script installs a wrapper on `window.__worldScene.onBeforeRender`
// (the scene hook the page already publishes for tooling; `installFrameCounters` uses the same
// mechanism) and pins the camera there, after every camera controller has run for the frame. Nothing
// in `src/` was added for this tool, so a baseline can be captured from an older commit too.
//
// Cross-platform: plain Node, no shell. Vite is started as `node node_modules/vite/bin/vite.js`
// (what `npm run dev` runs) so it can be stopped cleanly on Windows as well.
// ---------------------------------------------------------------------------------------

import { spawn, execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const VIEW_IDS = ['overview', 'horizon', 'coast', 'peak', 'river', 'forest', 'waterfall', 'creature'];
// Opt-in views: accepted by --views, never part of the default run, so default runs and their
// baselines keep the same set, order and poses.
//   coastnear  low camera (~50-70 m above the waterline) just offshore of the `coast` beach,
//              looking along the shore: the surf zone, swash and beach at close range.
//   vista      low oblique (~2.2 km above the sea) along the main massif, sky and horizon in frame.
const OPTIONAL_VIEW_IDS = ['coastnear', 'vista'];

const HELP = `Map visual check (prototype) — screenshots + FPS of ~8 world views.

  npm run capture:map -- [flags]

  --agents N          synthetic creatures (default 200)
  --segments K        segments per creature, 3..15 (default 5)
  --quality Q         high | medium | low | auto (default high; auto changes tier mid-run)
  --viewport WxH      browser viewport (default 1280x720)
  --seconds S         FPS sampling window per view (default 3)
  --views a,b,...     subset of: ${VIEW_IDS.join(', ')}; opt-in: ${OPTIONAL_VIEW_IDS.join(', ')}
  --baseline DIR      previous run folder to compare against (before/after + FPS deltas)
  --label TEXT        name for this run on the contact sheet (e.g. "after B1")
  --out DIR           output folder (default artifacts/map-captures/<timestamp>)
  --port P            dev server port (default 5173)
  --world-root DIR    World v2 pack under public/ (default world-v2; e.g. world-v2-big, dev-only switch)
  --headed            show the browser window
  --vsync             keep the display's frame cap (default: uncapped, so a fast GPU shows headroom)
  --hud               keep the HTML overlays (HUD, minimap, compass) in the screenshots
  --help              this text
`;

// ------------------------------------------------------------------------------- arguments

function parseArgs(argv) {
  const opts = {
    agents: 200,
    segments: 5,
    quality: 'high',
    viewport: { width: 1280, height: 720 },
    seconds: 3,
    views: VIEW_IDS,
    baseline: null,
    label: '',
    out: null,
    port: 5173,
    worldRoot: 'world-v2',
    headed: false,
    hud: false,
    vsync: false,
  };
  const need = (i, flag) => {
    if (i + 1 >= argv.length) throw new Error(`${flag} needs a value`);
    return argv[i + 1];
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    switch (a) {
      case '--help':
      case '-h':
        process.stdout.write(HELP);
        process.exit(0);
        break;
      case '--agents':
        opts.agents = int(need(i, a), a, 0, 5000);
        i++;
        break;
      case '--segments':
        opts.segments = int(need(i, a), a, 3, 15);
        i++;
        break;
      case '--quality': {
        const q = need(i, a);
        if (!['high', 'medium', 'low', 'auto'].includes(q)) throw new Error(`--quality: ${q}`);
        opts.quality = q;
        i++;
        break;
      }
      case '--viewport': {
        const m = /^(\d+)x(\d+)$/i.exec(need(i, a));
        if (!m) throw new Error('--viewport must look like 1280x720');
        opts.viewport = { width: Number(m[1]), height: Number(m[2]) };
        i++;
        break;
      }
      case '--seconds':
        opts.seconds = Math.max(0.5, Number(need(i, a)) || 3);
        i++;
        break;
      case '--views': {
        const list = need(i, a).split(',').map((s) => s.trim()).filter(Boolean);
        const bad = list.filter((v) => !VIEW_IDS.includes(v) && !OPTIONAL_VIEW_IDS.includes(v));
        if (bad.length) {
          throw new Error(`--views: unknown ${bad.join(', ')} (known: ${[...VIEW_IDS, ...OPTIONAL_VIEW_IDS].join(', ')})`);
        }
        opts.views = list;
        i++;
        break;
      }
      case '--baseline':
        opts.baseline = path.resolve(need(i, a));
        i++;
        break;
      case '--label':
        opts.label = need(i, a);
        i++;
        break;
      case '--out':
        opts.out = path.resolve(need(i, a));
        i++;
        break;
      case '--world-root': {
        const r = need(i, a);
        if (!/^[A-Za-z0-9_-]{1,64}$/.test(r)) throw new Error(`--world-root: ${r}`);
        opts.worldRoot = r;
        i++;
        break;
      }
      case '--port':
        opts.port = int(need(i, a), a, 1, 65535);
        i++;
        break;
      case '--headed':
        opts.headed = true;
        break;
      case '--hud':
        opts.hud = true;
        break;
      case '--vsync':
        opts.vsync = true;
        break;
      default:
        throw new Error(`unknown flag ${a}\n\n${HELP}`);
    }
  }
  return opts;
}

function int(raw, flag, lo, hi) {
  const n = Number(raw);
  if (!Number.isInteger(n) || n < lo || n > hi) throw new Error(`${flag} must be an integer in [${lo}, ${hi}]`);
  return n;
}

function stamp(d = new Date()) {
  const p = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}-${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}`;
}

function git(args) {
  try {
    return execFileSync('git', args, { cwd: REPO_ROOT, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
  } catch {
    return null;
  }
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ------------------------------------------------------------------------------- dev server

/**
 * Which checkout a running Vite dev server serves, read from the absolute module path that the
 * React dev transform writes into `/src/landscape.tsx` (the `fileName` of each `jsxDEV` call), with
 * the inline source map as a fallback. `up: false` when nothing answers. Port 5173 is every Vite
 * project's default and another worktree of this repo may hold it, so "something answers" is not
 * "this code is being served".
 */
async function servedCheckout(base) {
  let body;
  try {
    const res = await fetch(`${base}/src/landscape.tsx`, { signal: AbortSignal.timeout(2000) });
    if (!res.ok) return { up: true, root: null };
    body = await res.text();
  } catch {
    return { up: false, root: null };
  }
  const isAbs = (p) => path.isAbsolute(p) || /^[A-Za-z]:\//.test(p);
  for (const m of body.matchAll(/"([^"\n]+?)\/src\/landscape\.tsx"/g)) {
    const root = m[1].replace(/\\\\/g, '/').replace(/^\/@fs/, '');
    if (isAbs(root)) return { up: true, root };
  }
  const sm = /sourceMappingURL=data:application\/json;base64,([A-Za-z0-9+/=]+)/.exec(body);
  if (sm) {
    try {
      const map = JSON.parse(Buffer.from(sm[1], 'base64').toString('utf8'));
      for (const src of map.sources ?? []) {
        const abs = String(src).replace(/^file:\/\//, '').replace(/\\/g, '/');
        if (abs.endsWith('/src/landscape.tsx') && isAbs(abs)) return { up: true, root: abs.slice(0, -'/src/landscape.tsx'.length) };
      }
    } catch {
      // fall through: unverified
    }
  }
  return { up: true, root: null };
}

const samePath = (a, b) =>
  path.resolve(a).replace(/\\/g, '/').toLowerCase() === path.resolve(b).replace(/\\/g, '/').toLowerCase();

async function ensureDevServer(port) {
  const base = `http://127.0.0.1:${port}`;
  const probe = await servedCheckout(base);
  if (probe.up) {
    if (probe.root && !samePath(probe.root, REPO_ROOT)) {
      throw new Error(
        `port ${port} is serving another checkout (${probe.root}), not ${REPO_ROOT}.\n` +
          `Stop that server or pass --port <free port> so this checkout gets its own.`,
      );
    }
    return { base, child: null, reused: true, verified: probe.root !== null };
  }
  const viteBin = path.join(REPO_ROOT, 'node_modules', 'vite', 'bin', 'vite.js');
  if (!fs.existsSync(viteBin)) throw new Error(`vite not installed (${viteBin}); run npm install first`);
  const child = spawn(process.execPath, [viteBin, '--host', '127.0.0.1', '--port', String(port), '--strictPort'], {
    cwd: REPO_ROOT,
    stdio: ['ignore', 'pipe', 'pipe'],
    windowsHide: true,
  });
  let log = '';
  child.stdout.on('data', (d) => (log += d));
  child.stderr.on('data', (d) => (log += d));
  const t0 = Date.now();
  while (Date.now() - t0 < 60_000) {
    if (child.exitCode !== null) throw new Error(`vite exited early:\n${log}`);
    if ((await servedCheckout(base)).up) return { base, child, reused: false, verified: true };
    await sleep(300);
  }
  child.kill();
  throw new Error(`vite did not answer on ${base} within 60 s:\n${log}`);
}

// ------------------------------------------------------------------------------- browser

const BROWSER_ARGS = [
  '--ignore-gpu-blocklist',
  // A headed window behind other windows would otherwise get its rAF throttled, which reads as FPS.
  '--disable-background-timer-throttling',
  '--disable-renderer-backgrounding',
  '--disable-backgrounding-occluded-windows',
];

// Without these every view on a fast machine reads 60 (or 120) and a change that costs 30% is
// invisible. Uncapped numbers are headroom, not what a user's display shows.
const UNCAP_ARGS = ['--disable-gpu-vsync', '--disable-frame-rate-limit'];

async function launchBrowser(headed, vsync) {
  const args = vsync ? BROWSER_ARGS : [...BROWSER_ARGS, ...UNCAP_ARGS];
  let chromium;
  try {
    ({ chromium } = await import('@playwright/test'));
  } catch {
    throw new Error('@playwright/test is not installed in this checkout (root devDependency); run npm install');
  }
  const tried = [];
  // An installed Google Chrome first: no download, and it is the browser a person would open.
  try {
    const browser = await chromium.launch({ channel: 'chrome', headless: !headed, args });
    return { browser, kind: 'chrome' };
  } catch (e) {
    tried.push(`chrome channel: ${String(e.message).split('\n')[0]}`);
  }
  try {
    const browser = await chromium.launch({ headless: !headed, args });
    return { browser, kind: 'playwright-chromium' };
  } catch (e) {
    tried.push(`playwright chromium: ${String(e.message).split('\n')[0]}`);
  }
  throw new Error(
    `no usable browser.\n  ${tried.join('\n  ')}\n` +
      `Install Google Chrome, or ask the owner before downloading Playwright's Chromium ` +
      `(npx playwright install chromium, ~150 MB).`,
  );
}

// ------------------------------------------------------------------------------- page-side code
// Everything below in this section runs inside the page (Playwright serialises the function
// source), so each function is self-contained.

/** Wrap the scene's onBeforeRender once so `window.__mapCapture.pose` pins the camera. */
function pageInstallPin() {
  const scene = window.__worldScene;
  if (!scene) return false;
  const st = (window.__mapCapture = window.__mapCapture || { pose: null, follow: null, renderer: null });
  if (scene.onBeforeRender && scene.onBeforeRender.__mapCapturePin) return true;
  const prev = scene.onBeforeRender;
  const pin = function (renderer, sc, camera, target) {
    if (typeof prev === 'function') prev.call(this, renderer, sc, camera, target);
    st.renderer = renderer;
    let p = st.pose;
    const f = st.follow;
    if (p && f && f.mesh) {
      // Creature close-up: instance slots are re-packed every tick, so follow the instance nearest
      // to where the subject was last frame rather than a fixed index.
      const a = f.mesh.instanceMatrix ? f.mesh.instanceMatrix.array : null;
      const e = f.mesh.matrixWorld.elements;
      let best = null;
      let bestD = (f.maxStep || 6) ** 2;
      for (let i = 0; a && i < f.mesh.count; i++) {
        if (Math.abs(a[i * 16]) + Math.abs(a[i * 16 + 1]) + Math.abs(a[i * 16 + 2]) < 1e-4) continue;
        const lx = a[i * 16 + 12];
        const ly = a[i * 16 + 13];
        const lz = a[i * 16 + 14];
        const x = e[0] * lx + e[4] * ly + e[8] * lz + e[12];
        const y = e[1] * lx + e[5] * ly + e[9] * lz + e[13];
        const z = e[2] * lx + e[6] * ly + e[10] * lz + e[14];
        const d = (x - f.last[0]) ** 2 + (y - f.last[1]) ** 2 + (z - f.last[2]) ** 2;
        if (d < bestD) {
          bestD = d;
          best = [x, y, z];
        }
      }
      if (best) f.last = best;
      p = {
        position: [f.last[0] + f.offset[0], f.last[1] + f.offset[1], f.last[2] + f.offset[2]],
        target: f.last,
      };
      st.followed = f.last;
    }
    if (p && camera && camera.position) {
      camera.position.set(p.position[0], p.position[1], p.position[2]);
      camera.lookAt(p.target[0], p.target[1], p.target[2]);
      camera.updateMatrixWorld(true);
    }
  };
  pin.__mapCapturePin = true;
  scene.onBeforeRender = pin;
  return true;
}

function pageSetPose(pose) {
  const st = window.__mapCapture;
  st.pose = pose;
  st.follow = null;
  st.followed = null;
}

/** Resolve after `minFrames` animation frames AND `minMs`, or after `capMs` regardless. */
function pageWaitFrames({ minFrames, minMs, capMs }) {
  return new Promise((resolve) => {
    const t0 = performance.now();
    let n = 0;
    const done = () => resolve({ frames: n, ms: performance.now() - t0 });
    const cap = setTimeout(done, capMs);
    const tick = (now) => {
      n++;
      if ((n >= minFrames && now - t0 >= minMs) || now - t0 >= capMs) {
        clearTimeout(cap);
        done();
      } else requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
}

/** rAF timestamps over `ms`, plus the frame counters seen on each frame. */
function pageSampleFrames(ms) {
  return new Promise((resolve) => {
    const times = [];
    const totals = [];
    const t0 = performance.now();
    const finish = () => {
      const c = window.__animaFrameCounters || null;
      const st = window.__mapCapture || {};
      const info = st.renderer && st.renderer.info ? st.renderer.info.render : null;
      resolve({
        times,
        totals,
        counters: c ? JSON.parse(JSON.stringify(c)) : null,
        rendererInfo: info ? { calls: info.calls, triangles: info.triangles } : null,
      });
    };
    const cap = setTimeout(finish, ms + 8000);
    const tick = (now) => {
      times.push(now);
      const c = window.__animaFrameCounters;
      if (c && c.total) totals.push([c.total.calls, c.total.triangles]);
      if (now - t0 < ms) requestAnimationFrame(tick);
      else {
        clearTimeout(cap);
        finish();
      }
    };
    requestAnimationFrame(tick);
  });
}

function pageHideHud() {
  const st = window.__mapCapture || {};
  let el = st.renderer && st.renderer.domElement;
  if (!el) return false;
  while (el && el.parentElement && el !== document.body) {
    for (const sib of el.parentElement.children) {
      if (sib !== el && sib.tagName !== 'SCRIPT' && sib.tagName !== 'STYLE') sib.style.setProperty('visibility', 'hidden', 'important');
    }
    el = el.parentElement;
  }
  return true;
}

function pageGlInfo() {
  const st = window.__mapCapture || {};
  const r = st.renderer;
  if (!r || !r.getContext) return null;
  const gl = r.getContext();
  const ext = gl.getExtension('WEBGL_debug_renderer_info');
  return {
    vendor: ext ? gl.getParameter(ext.UNMASKED_VENDOR_WEBGL) : gl.getParameter(gl.VENDOR),
    renderer: ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER),
    version: gl.getParameter(gl.VERSION),
    pixelRatio: r.getPixelRatio ? r.getPixelRatio() : null,
  };
}

/**
 * Read the generated world and derive one camera pose per view. Feature targets come from world
 * data (highest peak, a river cell, a waterfall, the densest trees, ...), so the views follow the
 * world if generation changes. A view whose feature does not exist comes back `skipped` with why.
 */
/** Extent (manifest) and elevation span (format constants) of the World v2 bake, or null. */
function readWorldMetrics(worldRoot = 'world-v2') {
  const out = { extentM: null, elevSpanM: null };
  try {
    out.extentM = JSON.parse(fs.readFileSync(path.join(REPO_ROOT, 'public', worldRoot, 'index.json'), 'utf8')).extent_m ?? null;
  } catch {
    /* no World v2 tiles staged: the metre-based views fall back to the v1 scale guess below */
  }
  try {
    const src = fs.readFileSync(path.join(REPO_ROOT, 'src/components/Landscape/utils/worldArtifactV3.ts'), 'utf8');
    const lo = Number(/WV2_ELEV_MIN_M\s*=\s*(-?[\d.]+)/.exec(src)?.[1]);
    const hi = Number(/WV2_ELEV_MAX_M\s*=\s*(-?[\d.]+)/.exec(src)?.[1]);
    if (Number.isFinite(lo) && Number.isFinite(hi) && hi > lo) out.elevSpanM = hi - lo;
  } catch {
    /* format constants not found */
  }
  return out;
}

function pageAnalyseWorld() {
  const BIOMES = [
    'Ocean', 'Beach', 'Desert', 'Savanna', 'Grassland', 'Shrubland', 'Forest', 'Jungle', 'Taiga',
    'Tundra', 'Swamp', 'Rock', 'Snow', 'River', 'Lake', 'Mangrove', 'Chaparral', 'Steppe', 'Alpine',
    'Badlands', 'Glacier', 'Bog',
  ];
  const w = window.__world;
  const scene = window.__worldScene;
  const N = w.size;
  const sea = w.seaLevel;
  const notes = [];
  const metrics = window.__mapCaptureWorld || {};

  // ---- scale: terrain extent from the mesh itself, so a changed RENDER_SIZE is followed.
  let R = 1200;
  let maxY = -Infinity;
  const terrain = scene.getObjectByName('world-terrain') || scene.getObjectByName('world-terrain-lod');
  if (terrain) {
    let minX = Infinity;
    let maxX = -Infinity;
    terrain.updateMatrixWorld(true);
    terrain.traverse((o) => {
      const g = o.geometry;
      if (!g || typeof g.computeBoundingBox !== 'function') return;
      if (!g.boundingBox) g.computeBoundingBox();
      const lo = g.boundingBox.min.clone().applyMatrix4(o.matrixWorld);
      const hi = g.boundingBox.max.clone().applyMatrix4(o.matrixWorld);
      minX = Math.min(minX, lo.x, hi.x);
      maxX = Math.max(maxX, lo.x, hi.x);
      maxY = Math.max(maxY, lo.y, hi.y);
    });
    if (maxX > minX) R = maxX - minX;
  } else notes.push('terrain mesh not found by name; assumed render size 1200');
  let maxE = 0;
  let peakIdx = 0;
  for (let gy = 2; gy < N - 2; gy++) {
    for (let gx = 2; gx < N - 2; gx++) {
      const i = gy * N + gx;
      if (w.elevation[i] > maxE) {
        maxE = w.elevation[i];
        peakIdx = i;
      }
    }
  }
  let H = R * 0.14;
  if (Number.isFinite(maxY) && maxE > 0) {
    const est = maxY / maxE;
    if (est > R * 0.05 && est < R * 0.4) H = est;
    else notes.push(`height scale from mesh looked wrong (${est.toFixed(1)}); assumed 0.14 x size`);
  }

  // ---- sampling helpers (render space: x, z in [-R/2, R/2], y up)
  const cellToR = (g) => (g / (N - 1) - 0.5) * R;
  const cellUnitToR = R / N; // flora / waterfall coordinates are in cell units around the origin
  const idxAt = (x, z) => {
    const u = x / R + 0.5;
    const v = z / R + 0.5;
    if (u < 0 || u > 1 || v < 0 || v > 1) return -1;
    return Math.round(v * (N - 1)) * N + Math.round(u * (N - 1));
  };
  const elevAt = (x, z) => {
    const u = x / R + 0.5;
    const v = z / R + 0.5;
    if (u < 0 || u > 1 || v < 0 || v > 1) return sea;
    const fx = u * (N - 1);
    const fy = v * (N - 1);
    const x0 = Math.floor(fx);
    const y0 = Math.floor(fy);
    const x1 = Math.min(N - 1, x0 + 1);
    const y1 = Math.min(N - 1, y0 + 1);
    const tx = fx - x0;
    const ty = fy - y0;
    const E = w.elevation;
    return (
      E[y0 * N + x0] * (1 - tx) * (1 - ty) +
      E[y0 * N + x1] * tx * (1 - ty) +
      E[y1 * N + x0] * (1 - tx) * ty +
      E[y1 * N + x1] * tx * ty
    );
  };
  const surfAt = (x, z) => {
    const i = idxAt(x, z);
    const lake = i >= 0 && w.water ? w.water[i] : 0;
    return Math.max(elevAt(x, z), sea, lake || 0) * H;
  };
  const biomeAt = (x, z) => {
    const i = idxAt(x, z);
    return i < 0 ? 'off-map' : BIOMES[w.biome[i]] ?? `biome ${w.biome[i]}`;
  };
  // Downhill direction (render-space azimuth) from the elevation gradient.
  const downhillAz = (x, z) => {
    const d = R / N * 3;
    const gx = elevAt(x + d, z) - elevAt(x - d, z);
    const gz = elevAt(x, z + d) - elevAt(x, z - d);
    if (Math.abs(gx) + Math.abs(gz) < 1e-9) return null;
    return Math.atan2(-gz, -gx);
  };
  const azTo = (from, to) => Math.atan2(to[2] - from[2], to[0] - from[0]);
  const deg = (r) => Math.round((r * 180) / Math.PI);

  // Coarse canopy grid (4-unit bins holding at least one tree), so a line of sight can be refused
  // for passing through a crown. TREE_TOP is a rough crown height, not read from the flora meshes.
  const TREES = new Set([0, 1, 2, 5, 6]); // Pine, Round, Jungle, Acacia, Palm
  const CBIN = 4;
  const TREE_TOP = 7;
  const cside = Math.ceil(R / CBIN);
  const canopy = new Uint16Array(cside * cside);
  const cbin = (x, z) => {
    const bx = Math.floor((x + R / 2) / CBIN);
    const bz = Math.floor((z + R / 2) / CBIN);
    return bx < 0 || bz < 0 || bx >= cside || bz >= cside ? -1 : bz * cside + bx;
  };
  for (let k = 0; k < w.floraCount; k++) {
    if (!TREES.has(w.floraType[k])) continue;
    const b = cbin(w.floraX[k] * (R / N), w.floraZ[k] * (R / N));
    if (b >= 0 && canopy[b] < 65535) canopy[b]++;
  }
  const treesNear = (x, z, r) => {
    let n = 0;
    for (let dz = -r; dz <= r; dz += CBIN) {
      for (let dx = -r; dx <= r; dx += CBIN) {
        const b = cbin(x + dx, z + dz);
        if (b >= 0) n += canopy[b];
      }
    }
    return n;
  };
  const insideMap = (x, z) => Math.abs(x) <= R * 0.48 && Math.abs(z) <= R * 0.48;

  /**
   * Put the camera `dist` away from `target` at `pitchDeg` above the horizontal, trying azimuths
   * nearest `prefAz` first and then steeper pitches, until the camera is above ground and the line
   * of sight to the target clears the terrain.
   */
  const frame = (target, dist, pitchDeg, prefAz, opts = {}) => {
    const base = prefAz ?? 0;
    const blocks = (x, y, z, t) => {
      if (surfAt(x, z) > y - 0.3) return true;
      // Crowns count only away from the subject: a forest view looks *at* trees.
      if (opts.trees && t < 0.9) {
        const b = cbin(x, z);
        if (b >= 0 && canopy[b] > 0 && elevAt(x, z) * H + TREE_TOP > y) return true;
      }
      return false;
    };
    const steps = 16;
    const order = [0];
    for (let k = 1; k <= steps / 2; k++) order.push(k, -k);
    for (const pitch of [pitchDeg, pitchDeg + 12, pitchDeg + 25, 70]) {
      const pr = (pitch * Math.PI) / 180;
      for (const k of order) {
        const az = base + (k * 2 * Math.PI) / steps;
        const pos = [
          target[0] + dist * Math.cos(pr) * Math.cos(az),
          target[1] + dist * Math.sin(pr),
          target[2] + dist * Math.cos(pr) * Math.sin(az),
        ];
        const clearance = Math.max(1.5, dist * 0.03);
        if (!insideMap(pos[0], pos[2])) continue; // past the edge the camera looks at the void
        if (pos[1] < surfAt(pos[0], pos[2]) + clearance) continue;
        let blocked = false;
        for (let s = 1; s < 40 && !blocked; s++) {
          const t = s / 40;
          if (t > 0.97) break;
          const x = pos[0] + (target[0] - pos[0]) * t;
          const y = pos[1] + (target[1] - pos[1]) * t;
          const z = pos[2] + (target[2] - pos[2]) * t;
          if (blocks(x, y, z, t)) blocked = true;
        }
        if (!blocked) {
          return { position: pos, target, azimuthDeg: deg(az), pitchDeg: pitch, lineOfSight: 'clear' };
        }
      }
    }
    const pr = (70 * Math.PI) / 180;
    return {
      position: [target[0] + dist * Math.cos(pr) * Math.cos(base), target[1] + dist * Math.sin(pr), target[2] + dist * Math.cos(pr) * Math.sin(base)],
      target,
      azimuthDeg: deg(base),
      pitchDeg: 70,
      lineOfSight: 'blocked (fallback: steep view)',
    };
  };
  const rnd = (v) => v.map((n) => Math.round(n * 10) / 10);
  const views = {};
  const skip = (id, reason) => (views[id] = { status: 'skipped', reason });
  const put = (id, pose, feature) =>
    (views[id] = { status: 'ok', pose: { position: rnd(pose.position), target: rnd(pose.target) }, framing: { azimuthDeg: pose.azimuthDeg, pitchDeg: pose.pitchDeg, lineOfSight: pose.lineOfSight }, feature });

  // 1. overview: the whole square from high above, map edges included.
  views.overview = {
    status: 'ok',
    pose: { position: rnd([0, R * 0.8, R * 0.92]), target: [0, 0, rnd([R * 0.04])[0]] },
    feature: 'whole map from above, south side',
  };

  // Home: the land cell nearest the map centre that a person could stand on.
  const WATERY = new Set([0, 13, 14, 12, 20]); // Ocean, River, Lake, Snow, Glacier
  let home = null;
  let bestD = Infinity;
  for (let gy = 0; gy < N; gy += 4) {
    for (let gx = 0; gx < N; gx += 4) {
      const i = gy * N + gx;
      if (w.elevation[i] < sea + 0.02 || WATERY.has(w.biome[i]) || (w.slope && w.slope[i] > 0.2) || (w.water && w.water[i] > 0)) continue;
      const x = cellToR(gx);
      const z = cellToR(gy);
      const d = x * x + z * z;
      if (d < bestD) {
        bestD = d;
        home = [x, z];
      }
    }
  }
  const peak = [cellToR(peakIdx % N), maxE * H, cellToR(Math.floor(peakIdx / N))];

  // 2. horizon: eye height over land near the centre, looking towards the highest peak.
  if (!home) skip('horizon', 'no dry, gentle land cell found');
  else {
    const eye = [home[0], surfAt(home[0], home[1]) + Math.max(12, H * 0.1), home[1]]; // above the canopy
    const az = azTo(eye, peak);
    const far = R * 0.4;
    views.horizon = {
      status: 'ok',
      pose: { position: rnd(eye), target: rnd([eye[0] + far * Math.cos(az), eye[1] - far * Math.tan((4 * Math.PI) / 180), eye[2] + far * Math.sin(az)]) },
      feature: `low over ${biomeAt(home[0], home[1])} near the map centre, facing the highest peak`,
    };
  }

  // 3. coast: the beach nearest the map centre that faces the OPEN SEA, seen from the sea side.
  // Open sea = below-sea cells connected to the map border (flood fill on a half-resolution grid),
  // so a sand patch beside an inland pan, lagoon or lake does not count. A beach qualifies when the
  // points ~200 m and ~600 m seaward of it (down the local slope) are both open sea.
  {
    const OS = 2; // open-sea grid step, in cells
    const M = Math.ceil(N / OS);
    const open = new Uint8Array(M * M);
    const wet = (gx, gy) => {
      const i = gy * N + gx;
      return w.elevation[i] < sea && !(w.water && w.water[i] > 0) && w.biome[i] !== 14 && w.biome[i] !== 13;
    };
    const queue = new Int32Array(M * M);
    let qh = 0;
    let qt = 0;
    const seed = (mx, my) => {
      const k = my * M + mx;
      if (open[k] || !wet(Math.min(N - 1, mx * OS), Math.min(N - 1, my * OS))) return;
      open[k] = 1;
      queue[qt++] = k;
    };
    for (let m = 0; m < M; m++) {
      seed(m, 0);
      seed(m, M - 1);
      seed(0, m);
      seed(M - 1, m);
    }
    while (qh < qt) {
      const k = queue[qh++];
      const mx = k % M;
      const my = (k - mx) / M;
      if (mx > 0) seed(mx - 1, my);
      if (mx < M - 1) seed(mx + 1, my);
      if (my > 0) seed(mx, my - 1);
      if (my < M - 1) seed(mx, my + 1);
    }
    const openSeaAt = (x, z) => {
      const u = x / R + 0.5;
      const v = z / R + 0.5;
      if (u < 0 || u > 1 || v < 0 || v > 1) return true; // past the border is ocean
      const mx = Math.min(M - 1, Math.floor((u * (N - 1)) / OS));
      const my = Math.min(M - 1, Math.floor((v * (N - 1)) / OS));
      return open[my * M + mx] === 1;
    };
    const uPerM = metrics.extentM ? R / metrics.extentM : R / 40960;
    const facesOpenSea = (x, z) => {
      const az = downhillAz(x, z);
      if (az === null) return false;
      for (const m of [200, 600]) {
        if (!openSeaAt(x + m * uPerM * Math.cos(az), z + m * uPerM * Math.sin(az))) return false;
      }
      return true;
    };
    let c = null;
    let d0 = Infinity;
    for (let gy = 0; gy < N; gy += 2) {
      for (let gx = 0; gx < N; gx += 2) {
        const i = gy * N + gx;
        const isBeach = w.biome[i] === 1;
        const isShore = w.shore && w.shore[i] > 0.9 && w.elevation[i] > sea && w.elevation[i] < sea + 0.02;
        if (!isBeach && !isShore) continue;
        const x = cellToR(gx);
        const z = cellToR(gy);
        const d = x * x + z * z;
        if (d < d0 && facesOpenSea(x, z)) {
          d0 = d;
          c = [x, z];
        }
      }
    }
    if (!c) skip('coast', 'no beach or shoreline cell facing the open sea');
    else {
      const t = [c[0], surfAt(c[0], c[1]), c[1]];
      put('coast', frame(t, 70, 20, downhillAz(c[0], c[1])), `${biomeAt(c[0], c[1])} facing the open sea nearest the map centre, from the sea side`);

      // coastnear (opt-in): just offshore of the same beach, low over the water, looking along
      // the shoreline (whichever direction keeps the line of sight nearest the waterline).
      const seaAz = downhillAz(c[0], c[1]);
      if (!metrics.extentM) skip('coastnear', 'no World v2 manifest: map extent unknown');
      else if (seaAz === null) skip('coastnear', 'flat shore cell: no seaward direction');
      else {
        const seaY = sea * H;
        const unitsPerM = R / metrics.extentM; // horizontal units per metre
        // Walk seaward from the beach cell to the waterline (the Beach biome can sit inland of it).
        let wl = null;
        for (let m = 0; m <= 2000 && !wl; m += 20) {
          const x = c[0] + m * unitsPerM * Math.cos(seaAz);
          const z = c[1] + m * unitsPerM * Math.sin(seaAz);
          if (elevAt(x, z) < sea) wl = [x, z];
        }
        const base = wl ?? c;
        const out = 160 * unitsPerM; // ~160 m offshore of the waterline
        const uYc = H / (metrics.elevSpanM || 7200); // vertical units per metre
        const eyeY = seaY + 55 * uYc; // ~55 m above the waterline
        const pos = [base[0] + out * Math.cos(seaAz), eyeY, base[1] + out * Math.sin(seaAz)];
        let best = null;
        for (const sgn of [1, -1]) {
          const tan = seaAz + (sgn * Math.PI) / 2;
          const along = 1400 * unitsPerM; // look ~1.4 km along the shore
          const tx = base[0] + along * Math.cos(tan) - 40 * unitsPerM * Math.cos(seaAz);
          const tz = base[1] + along * Math.sin(tan) - 40 * unitsPerM * Math.sin(seaAz);
          const err = Math.abs(elevAt(tx, tz) - sea);
          if (insideMap(tx, tz) && (!best || err < best.err)) best = { err, target: [tx, seaY + 2 * uYc, tz] };
        }
        if (!best || !insideMap(pos[0], pos[2]) || surfAt(pos[0], pos[2]) > eyeY - 0.2) {
          skip('coastnear', 'no clear low viewpoint offshore of the coast beach');
        } else {
          put(
            'coastnear',
            { position: pos, target: best.target, azimuthDeg: deg(azTo(pos, best.target)), pitchDeg: 0, lineOfSight: 'unchecked (low over water)' },
            `${biomeAt(c[0], c[1])} shore at close range, ~55 m above the water, looking along the shoreline`,
          );
        }
      }
    }
  }

  // vista (opt-in): along the main massif from a low oblique height, sky and horizon in frame.
  // The massif axis is the principal axis of the high ground (top 40% of the land relief); the
  // camera sits off one end of it, pushed toward the lower side, ~2.2 km above the sea.
  {
    let n = 0;
    let mx = 0;
    let mz = 0;
    const pts = [];
    const thr = sea + (maxE - sea) * 0.6;
    for (let gy = 0; gy < N; gy += 8) {
      for (let gx = 0; gx < N; gx += 8) {
        if (w.elevation[gy * N + gx] < thr) continue;
        const x = cellToR(gx);
        const z = cellToR(gy);
        pts.push([x, z]);
        mx += x;
        mz += z;
        n++;
      }
    }
    if (n < 8) skip('vista', 'no massif (too little high ground)');
    else {
      mx /= n;
      mz /= n;
      let sxx = 0;
      let szz = 0;
      let sxz = 0;
      for (const [x, z] of pts) {
        sxx += (x - mx) * (x - mx);
        szz += (z - mz) * (z - mz);
        sxz += (x - mx) * (z - mz);
      }
      const ang = 0.5 * Math.atan2(2 * sxz, sxx - szz);
      const ax = [Math.cos(ang), Math.sin(ang)];
      const len = Math.sqrt(Math.max(sxx, szz) / n) * 2;
      let side = [-ax[1], ax[0]];
      if (elevAt(mx + side[0] * len * 0.5, mz + side[1] * len * 0.5) > elevAt(mx - side[0] * len * 0.5, mz - side[1] * len * 0.5)) {
        side = [-side[0], -side[1]];
      }
      const uY = H / (metrics.elevSpanM || 7200); // vertical units per metre
      const seaY = sea * H;
      let pos = [mx - ax[0] * len * 0.9 + side[0] * len * 0.45, seaY + 2200 * uY, mz - ax[1] * len * 0.9 + side[1] * len * 0.45];
      // Keep the camera on the map and above the ground.
      const lim = R * 0.46;
      pos = [Math.max(-lim, Math.min(lim, pos[0])), pos[1], Math.max(-lim, Math.min(lim, pos[2]))];
      pos[1] = Math.max(pos[1], surfAt(pos[0], pos[2]) + 600 * uY);
      const target = [mx + ax[0] * len * 0.9, seaY + 700 * uY, mz + ax[1] * len * 0.9];
      put(
        'vista',
        { position: pos, target, azimuthDeg: deg(azTo(pos, target)), pitchDeg: 0, lineOfSight: 'unchecked (oblique)' },
        `along the main massif (${n} high cells), ~2.2 km above the sea`,
      );
    }
  }

  // 4. peak: the highest cell (snow cap if there is one), from the side facing the map centre,
  // 0.15 of C01's map away (~6.1 km) on every extent.
  const c01Share = metrics.extentM ? 40960 / metrics.extentM : 1;
  put('peak', frame(peak, R * 0.15 * c01Share, 15, azTo(peak, [0, 0, 0])), `highest point, elevation ${maxE.toFixed(3)} (${biomeAt(peak[0], peak[2])})`);

  // 5. river valley: the biggest inland river cell, looking upstream from below it.
  {
    let r = null;
    for (const margin of [0.06, 0.03, 0.01]) {
      let best = -1;
      for (let gy = 0; gy < N; gy += 2) {
        for (let gx = 0; gx < N; gx += 2) {
          const i = gy * N + gx;
          if (w.riverAmt[i] < 180 || w.elevation[i] < sea + margin || (w.water && w.water[i] > 0)) continue;
          if (w.flow[i] > best) {
            best = w.flow[i];
            r = [cellToR(gx), cellToR(gy), best];
          }
        }
      }
      if (r) break;
    }
    if (!r) skip('river', 'no inland river cell (riverAmt >= 180)');
    else {
      const t = [r[0], surfAt(r[0], r[1]), r[1]];
      put('river', frame(t, 90, 26, downhillAz(r[0], r[1])), `largest inland river cell, flow ${r[2].toFixed(3)} (${biomeAt(r[0], r[1])})`);
    }
  }

  // 6. forest: the 32-unit square with the most trees.
  {
    const BIN = 32;
    const side = Math.ceil(R / BIN);
    const count = new Uint32Array(side * side);
    const sx = new Float64Array(side * side);
    const sz = new Float64Array(side * side);
    for (let k = 0; k < w.floraCount; k++) {
      if (!TREES.has(w.floraType[k])) continue;
      const x = w.floraX[k] * cellUnitToR;
      const z = w.floraZ[k] * cellUnitToR;
      const bx = Math.min(side - 1, Math.max(0, Math.floor((x + R / 2) / BIN)));
      const bz = Math.min(side - 1, Math.max(0, Math.floor((z + R / 2) / BIN)));
      const b = bz * side + bx;
      count[b]++;
      sx[b] += x;
      sz[b] += z;
    }
    // Inner 84% of the map only: a forest on the very edge frames the map border, not the trees.
    const inner = (b) => Math.abs(((b % side) + 0.5) * BIN - R / 2) < R * 0.42 && Math.abs((Math.floor(b / side) + 0.5) * BIN - R / 2) < R * 0.42;
    let bi = -1;
    for (let b = 0; b < count.length; b++) if (inner(b) && (bi < 0 || count[b] > count[bi])) bi = b;
    if (bi < 0 || count[bi] === 0) skip('forest', 'no trees in the flora set');
    else {
      const x = sx[bi] / count[bi];
      const z = sz[bi] / count[bi];
      const t = [x, surfAt(x, z) + 2, z];
      put('forest', frame(t, 70, 32, azTo(t, [0, 0, 0])), `densest trees: ${count[bi]} in a ${BIN}x${BIN} square (${biomeAt(x, z)})`);
    }
  }

  // 7. waterfall: the tallest drop that can be SEEN, from downstream, a little above its base.
  // The curtain hangs one unit downstream of the lip (`WorldWaterfalls`), and downstream is +yaw.
  // Waterfalls are tried tallest first; for each, cameras 10-52 units downstream (up to 60 degrees
  // off the flow line) at a third to one drop above the base, and the first with a clear line of
  // sight to the middle of the curtain, as drawn, wins. The old steep fallback is the last resort.
  if (!w.waterfallCount) skip('waterfall', 'world has no waterfalls');
  else {
    const order = Array.from({ length: w.waterfallCount }, (_, i) => i).sort((a, b) => w.waterfallDrop[b] - w.waterfallDrop[a]);
    // The drawn terrain is a coarser mesh than the data (hundreds of metres per vertex), and in a
    // gorge it sits well above the data floor: test against the higher of the two, read from the
    // terrain geometry itself (a regular (res+1)^2 grid; max of the four corners, conservative).
    const tGeom = terrain && terrain.geometry && terrain.geometry.attributes && terrain.geometry.attributes.position;
    const side = tGeom ? Math.round(Math.sqrt(tGeom.count)) : 0;
    const meshSurfAt = (x, z) => {
      if (!tGeom || side * side !== tGeom.count) return -Infinity;
      const fx = Math.min(side - 1, Math.max(0, (x / R + 0.5) * (side - 1)));
      const fz = Math.min(side - 1, Math.max(0, (z / R + 0.5) * (side - 1)));
      const x0 = Math.floor(fx);
      const z0 = Math.floor(fz);
      const x1 = Math.min(side - 1, x0 + 1);
      const z1 = Math.min(side - 1, z0 + 1);
      const a = tGeom.array;
      return Math.max(a[(z0 * side + x0) * 3 + 1], a[(z0 * side + x1) * 3 + 1], a[(z1 * side + x0) * 3 + 1], a[(z1 * side + x1) * 3 + 1]);
    };
    const groundAt = (x, z) => Math.max(surfAt(x, z), meshSurfAt(x, z));
    // Where each curtain is actually drawn (the renderer may seat it on the drawn surface): the
    // instance translation is the curtain's centre, its Y scale the drop + 0.6 (WorldWaterfalls).
    const curtains = scene.getObjectByName('waterfall-curtains');
    const drawn = (k) => {
      if (!curtains || !curtains.instanceMatrix || k >= curtains.count) return null;
      const e = curtains.instanceMatrix.array;
      const o = k * 16;
      const sy = Math.hypot(e[o + 4], e[o + 5], e[o + 6]);
      return { centre: [e[o + 12], e[o + 13], e[o + 14]], drop: Math.max(0.1, sy - 0.6) };
    };
    const losClear = (from, to) => {
      for (let s = 1; s < 48; s++) {
        const t = s / 48;
        if (t > 0.92) break;
        const x = from[0] + (to[0] - from[0]) * t;
        const y = from[1] + (to[1] - from[1]) * t;
        const z = from[2] + (to[2] - from[2]) * t;
        if (groundAt(x, z) > y - 0.15) return false;
      }
      return true;
    };
    let pick = null;
    for (const k of order) {
      if (pick) break;
      const yaw = w.waterfallYaw[k];
      const drop = Math.max(1.2, w.waterfallDrop[k] * H);
      const topY = w.waterfallTopE[k] * H;
      const lip = [w.waterfallX[k] * cellUnitToR, w.waterfallZ[k] * cellUnitToR];
      const d = drawn(k);
      const t = d ? d.centre : [lip[0] + Math.cos(yaw) * 1.0, topY - drop / 2, lip[1] + Math.sin(yaw) * 1.0];
      const baseY = d ? d.centre[1] - d.drop / 2 : topY - drop;
      // A curtain the drawn terrain swallows past its middle cannot be framed: next waterfall.
      if (meshSurfAt(t[0], t[2]) > t[1]) continue;
      for (const dist of [10, 16, 24, 36, 52]) {
        if (pick) break;
        for (const off of [0, 0.35, -0.35, 0.7, -0.7, 1.05, -1.05]) {
          if (pick) break;
          for (const up of [0.35, 0.7, 1.0]) {
            const az = yaw + off;
            const x = t[0] + dist * Math.cos(az);
            const z = t[2] + dist * Math.sin(az);
            if (!insideMap(x, z)) continue;
            const y = Math.max(baseY + drop * up, groundAt(x, z) + 1.0);
            if (y > baseY + drop * 1.6) continue; // the ground there is above the fall: not "downstream"
            const pos = [x, y, z];
            if (!losClear(pos, t)) continue;
            pick = { k, drop, pose: { position: pos, target: t, azimuthDeg: deg(azTo(pos, t)), pitchDeg: deg(Math.atan2(t[1] - y, dist)), lineOfSight: 'clear' }, dist, rank: order.indexOf(k) + 1 };
            break;
          }
        }
      }
    }
    if (pick) {
      put('waterfall', pick.pose, `waterfall #${pick.rank} by drop of ${w.waterfallCount} (drop ${pick.drop.toFixed(1)} units), from ${pick.dist} units downstream, a little above its base`);
    } else {
      const k = order[0];
      const x = w.waterfallX[k] * cellUnitToR;
      const z = w.waterfallZ[k] * cellUnitToR;
      const drop = Math.max(1.2, w.waterfallDrop[k] * H);
      const t = [x, w.waterfallTopE[k] * H - drop / 2, z];
      put('waterfall', frame(t, Math.max(15, drop * 6), 10, w.waterfallYaw[k]), `tallest of ${w.waterfallCount} waterfalls, drop ${drop.toFixed(1)} units (no clear downstream view found)`);
    }
  }

  // Kept on the page for the creature view, which is framed later (creatures move).
  window.__mapCaptureFrame = frame;
  window.__mapCaptureTreesNear = treesNear;
  return { renderSize: R, heightUnits: H, seaLevel: sea, worldSize: N, home, notes, views };
}

/**
 * Pick a creature to follow: an instance of the live-agents layer standing on land, nearest the
 * map centre. Generic on purpose (card B1 is replacing the creature meshes): any InstancedMesh with
 * instances under an object named like `live-agent*` / `creature*` qualifies.
 */
function pageFollowCreature(phase) {
  // phase 'approach': far creatures are drawn as LOD markers only, so first fly near one of them
  // (markers included); phase 'follow': pick a full-detail instance there and follow it.
  const approach = phase === 'approach';
  const st = window.__mapCapture;
  const scene = window.__worldScene;
  const w = window.__world;
  const N = w.size;
  const roots = [];
  scene.traverse((o) => {
    const n = (o.name || '').toLowerCase();
    if (n === 'live-agents' || n.startsWith('creature')) roots.push(o);
  });
  const meshes = [];
  for (const root of roots) {
    root.traverse((o) => {
      const n = (o.name || '').toLowerCase();
      if (!o.isInstancedMesh || !o.count || !o.visible) return;
      if (/shadow|selection|ring|snout/.test(n)) return;
      if (!approach && /marker/.test(n)) return;
      meshes.push(o);
    });
  }
  if (!meshes.length) {
    return { ok: false, reason: approach ? 'no creature instances in the scene (agents=0, or the creature layer changed shape)' : 'creatures did not switch to full detail near the camera' };
  }
  meshes.sort((a, b) => (b.name.includes('sphere') ? 1 : 0) - (a.name.includes('sphere') ? 1 : 0));
  const mesh = meshes[0];
  mesh.updateMatrixWorld(true);
  const R = window.__mapCaptureScale.renderSize;
  const a = mesh.instanceMatrix.array;
  const e = mesh.matrixWorld.elements;
  if (!mesh.geometry.boundingSphere) mesh.geometry.computeBoundingSphere();
  const cands = [];
  for (let i = 0; i < mesh.count; i++) {
    const lx = a[i * 16 + 12];
    const ly = a[i * 16 + 13];
    const lz = a[i * 16 + 14];
    const x = e[0] * lx + e[4] * ly + e[8] * lz + e[12];
    const y = e[1] * lx + e[5] * ly + e[9] * lz + e[13];
    const z = e[2] * lx + e[6] * ly + e[10] * lz + e[14];
    // Unused slots are parked with a zero scale: invisible, so not a subject.
    if (Math.hypot(a[i * 16], a[i * 16 + 1], a[i * 16 + 2]) < 1e-4) continue;
    const u = x / R + 0.5;
    const v = z / R + 0.5;
    if (u < 0.03 || u > 0.97 || v < 0.03 || v > 0.97) continue;
    const ci = Math.round(v * (N - 1)) * N + Math.round(u * (N - 1));
    if (w.elevation[ci] <= w.seaLevel + 0.005) continue; // in the sea
    const icy = w.biome[ci] === 12 || w.biome[ci] === 20 || w.biome[ci] === 11; // Snow, Glacier, Rock
    // Open ground first (fewest trees within 12 units), then nearest the map centre.
    cands.push({ i, pos: [x, y, z], trees: window.__mapCaptureTreesNear(x, z, 12) + (icy ? 1000 : 0), d: Math.hypot(x, z) });
  }
  if (!cands.length) return { ok: false, reason: `${mesh.count} creature instances, none on land` };
  cands.sort((p, q) => p.trees - q.trees || p.d - q.d);
  let pick = null;
  for (const c of cands.slice(0, 60)) {
    const s = Math.hypot(a[c.i * 16], a[c.i * 16 + 1], a[c.i * 16 + 2]) || 1;
    const radius = (mesh.geometry.boundingSphere ? mesh.geometry.boundingSphere.radius : 0.5) * s;
    const dist = Math.min(60, Math.max(8, radius * 18));
    const pose = window.__mapCaptureFrame(c.pos, dist, 22, Math.atan2(-c.pos[2], -c.pos[0]), { trees: true });
    if (!pick || (pose.lineOfSight === 'clear' && pick.pose.lineOfSight !== 'clear')) pick = { ...c, radius, dist, pose };
    if (pose.lineOfSight === 'clear') break;
  }
  if (approach) {
    st.pose = pick.pose;
    st.follow = null;
    return { ok: true, approach: true };
  }
  const offset = [0, 1, 2].map((k) => pick.pose.position[k] - pick.pos[k]);
  st.pose = { position: pick.pose.position, target: pick.pos };
  st.follow = { mesh, last: pick.pos, offset, maxStep: Math.max(3, pick.radius * 4) };
  const radius = pick.radius;
  const dist = pick.dist;
  return {
    ok: true,
    mesh: mesh.name || '(unnamed InstancedMesh)',
    index: pick.i,
    instances: mesh.count,
    radius: Math.round(radius * 100) / 100,
    distance: Math.round(dist * 10) / 10,
    treesNearby: pick.trees % 1000,
    lineOfSight: pick.pose.lineOfSight,
  };
}

// ------------------------------------------------------------------------------- console

const EXPECTED_NO_BACKEND = /tauri|__TAURI|ipc:\/\/|transformCallback|invoke\b|plugin:event/i;
const TOOLCHAIN_NOISE = ['[vite]', 'React DevTools', 'Download the React DevTools', 'GL Driver Message', 'WebGL-0x', 'THREE.Clock: This module has been deprecated'];

function classify(m) {
  if (EXPECTED_NO_BACKEND.test(m.text)) return 'expectedNoBackend';
  if (TOOLCHAIN_NOISE.some((n) => m.text.includes(n))) return 'toolchainNoise';
  if (m.type === 'pageerror') return 'pageErrors';
  return m.type === 'error' ? 'errors' : 'warnings';
}

function summariseConsole(messages) {
  const groups = { pageErrors: [], errors: [], warnings: [], expectedNoBackend: [], toolchainNoise: [] };
  const seen = new Map();
  for (const m of messages) {
    const key = `${m.type}|${m.text.slice(0, 300)}`;
    const hit = seen.get(key);
    if (hit) {
      hit.count++;
      continue;
    }
    const entry = { type: m.type, text: m.text.slice(0, 600), count: 1, firstSeen: m.phase };
    seen.set(key, entry);
    groups[classify(m)].push(entry);
  }
  return groups;
}

// ------------------------------------------------------------------------------- stats

function frameStats(times) {
  const dts = [];
  for (let i = 1; i < times.length; i++) dts.push(times[i] - times[i - 1]);
  if (dts.length < 2) return { frames: dts.length, medianFps: null, p5Fps: null, meanFps: null };
  const sorted = [...dts].sort((a, b) => a - b);
  const q = (p) => sorted[Math.min(sorted.length - 1, Math.floor(p * (sorted.length - 1)))];
  const r1 = (n) => Math.round(n * 10) / 10;
  return {
    frames: dts.length,
    medianFps: r1(1000 / q(0.5)),
    // 5th-percentile FPS = the 95th-percentile frame time: what the slow frames feel like.
    p5Fps: r1(1000 / q(0.95)),
    meanFps: r1((dts.length * 1000) / (times[times.length - 1] - times[0])),
    worstFrameMs: r1(sorted[sorted.length - 1]),
  };
}

function median(nums) {
  if (!nums.length) return null;
  const s = [...nums].sort((a, b) => a - b);
  return s[Math.floor(s.length / 2)];
}

// ------------------------------------------------------------------------------- contact sheet

const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
const fmt = (n) => (n === null || n === undefined ? '–' : typeof n === 'number' ? n.toLocaleString('en-US') : n);
/** `(+12.3)` coloured good/bad: FPS up is good, draw calls and triangles up are a cost. */
function delta(now, before, higherIsBetter = true) {
  if (typeof now !== 'number' || typeof before !== 'number') return '';
  const d = Math.round((now - before) * 10) / 10;
  const cls = d === 0 ? 'same' : (d > 0) === higherIsBetter ? 'good' : 'bad';
  const pct = before ? ` ${d > 0 ? '+' : ''}${Math.round((d / before) * 100)}%` : '';
  return ` <span class="d ${cls}">(${d > 0 ? '+' : ''}${d.toLocaleString('en-US')}${pct})</span>`;
}

function renderSheet(report) {
  const b = report.baseline;
  const warn = [];
  if (report.environment.softwareGl) warn.push(`Software WebGL renderer (${report.environment.gl?.renderer}). FPS figures are meaningless on this run.`);
  if (report.environment.headless) warn.push('Headless browser: FPS is indicative only.');
  if (!report.server.verified) warn.push('Reused a dev server whose checkout could not be verified.');
  if (b && !report.options.vsync) {
    warn.push(
      'Uncapped FPS is noisy: two runs of identical code on an M5 Mac differed by up to ~25% on a single view (2026-11-19). ' +
        'Read FPS deltas smaller than that as noise; draw calls and triangles are the stable numbers (creature views vary with the fixture).',
    );
  }
  const cards = report.views
    .map((v) => {
      const bv = b?.views?.[v.id];
      const img = (src, cap) =>
        src ? `<figure><a href="${esc(src)}" target="_blank"><img src="${esc(src)}" loading="lazy"></a><figcaption>${esc(cap)}</figcaption></figure>` : `<figure class="missing"><div>no image</div><figcaption>${esc(cap)}</figcaption></figure>`;
      const pics = b
        ? img(bv?.png ? `baseline/${bv.png}` : null, `baseline: ${b.label || b.commit || b.runId}`) + img(v.png, `this run: ${report.label || report.commit}`)
        : img(v.png, report.label || report.commit);
      const f = v.fps || {};
      const bf = bv?.fps || {};
      const c = v.counters?.total || {};
      const bc = bv?.counters?.total || {};
      const rows =
        v.status === 'ok'
          ? `<tr><th>FPS median</th><td>${fmt(f.medianFps)}${delta(f.medianFps, bf.medianFps)}</td><th>FPS p5</th><td>${fmt(f.p5Fps)}${delta(f.p5Fps, bf.p5Fps)}</td></tr>
             <tr><th>draw calls</th><td>${fmt(c.calls)}${delta(c.calls, bc.calls, false)}</td><th>triangles</th><td>${fmt(c.triangles)}${delta(c.triangles, bc.triangles, false)}</td></tr>`
          : `<tr><td colspan="4">skipped: ${esc(v.reason)}</td></tr>`;
      return `<section class="card"><h2>${esc(v.id)} <small>${esc(v.feature || '')}</small></h2>
        <div class="pics ${b ? 'two' : ''}">${pics}</div>
        <table>${rows}</table>
        ${v.pose ? `<p class="pose">camera ${esc(JSON.stringify(v.pose.position))} → ${esc(JSON.stringify(v.pose.target))}${v.framing ? ` · az ${v.framing.azimuthDeg}° pitch ${v.framing.pitchDeg}° · ${esc(v.framing.lineOfSight)}` : ''}</p>` : ''}
      </section>`;
    })
    .join('\n');
  const consoleBlock = (name, list) =>
    list.length
      ? `<details ${name === 'pageErrors' || name === 'errors' ? 'open' : ''}><summary>${esc(name)} (${list.length})${
          b?.newConsole?.[name] ? ` — <b>${b.newConsole[name]} new vs baseline</b>` : ''
        }</summary><ul>${list.map((m) => `<li><code>${m.count}× [${esc(m.firstSeen)}]</code> ${esc(m.text)}</li>`).join('')}</ul></details>`
      : `<p class="muted">${esc(name)}: none</p>`;
  const cons = report.console;
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Map captures ${esc(report.runId)}</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>
:root{--bg:#f6f7f9;--fg:#15171a;--card:#fff;--muted:#667;--line:#dde;--up:#0a7d32;--down:#b3261e;--warn:#fff4d6}
@media (prefers-color-scheme:dark){:root{--bg:#121417;--fg:#e8eaed;--card:#1c1f24;--muted:#9aa;--line:#333;--up:#6fdc8c;--down:#ff8a80;--warn:#3a3218}}
body{margin:0;padding:16px;background:var(--bg);color:var(--fg);font:14px/1.45 system-ui,sans-serif}
h1{font-size:20px;margin:0 0 4px}h2{font-size:16px;margin:0 0 8px}h2 small{font-weight:400;color:var(--muted)}
.meta{color:var(--muted);margin:0 0 12px}.warn{background:var(--warn);padding:8px 12px;border-radius:6px;margin:6px 0}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(560px,1fr));gap:14px}
.card{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:12px}
.pics{display:grid;gap:8px}.pics.two{grid-template-columns:1fr 1fr}
figure{margin:0}img{width:100%;display:block;border-radius:4px}figcaption{font-size:12px;color:var(--muted)}
.missing div{aspect-ratio:16/9;display:grid;place-items:center;background:var(--line);border-radius:4px}
table{width:100%;border-collapse:collapse;margin-top:8px}th{text-align:left;font-weight:500;color:var(--muted)}td,th{padding:2px 6px}
.d.good{color:var(--up)}.d.bad{color:var(--down)}.d.same{color:var(--muted)}.pose{font-size:11px;color:var(--muted);word-break:break-all}
.muted{color:var(--muted)}code{font-size:12px}li{margin:2px 0;word-break:break-word}
@media (max-width:620px){.grid{grid-template-columns:1fr}.pics.two{grid-template-columns:1fr}}
</style></head><body>
<h1>Map visual check ${esc(report.label ? `— ${report.label}` : '')} <small class="muted">(prototype)</small></h1>
<p class="meta">${esc(report.runId)} · commit ${esc(report.commit)}${report.dirty ? ' (uncommitted changes)' : ''} · ${esc(report.environment.browser)} ${esc(report.environment.browserVersion)} · ${esc(report.environment.gl?.renderer || 'GL unknown')} · viewport ${report.options.viewport.width}x${report.options.viewport.height} · quality ${esc(report.options.quality)} · world ready in ${fmt(report.timings.worldReadyMs)} ms${
    b ? ` · baseline: ${esc(b.label || b.runId)} (${esc(b.commit)})` : ''
  }</p>
<p class="meta">Creatures: ${report.options.agents} SYNTHETIC fixture agents (?replay=synthetic) — procedural test data, not simulation output. FPS is from requestAnimationFrame over ${report.options.seconds} s per view; it is this browser on this machine, not the Tauri app and not the Iris Xe target.</p>
${report.error ? `<p class="warn"><b>Run failed part-way:</b> <code>${esc(report.error.split('\n')[0])}</code></p>` : ''}
${warn.map((w) => `<p class="warn">${esc(w)}</p>`).join('')}
<div class="grid">${cards}</div>
<h2 style="margin-top:20px">Console</h2>
${consoleBlock('pageErrors', cons.pageErrors)}${consoleBlock('errors', cons.errors)}${consoleBlock('warnings', cons.warnings)}${consoleBlock('expectedNoBackend', cons.expectedNoBackend)}${consoleBlock('toolchainNoise', cons.toolchainNoise)}
</body></html>`;
}

// ------------------------------------------------------------------------------- baseline

function loadBaseline(dir, outDir) {
  const file = path.join(dir, 'report.json');
  if (!fs.existsSync(file)) throw new Error(`--baseline: no report.json in ${dir}`);
  const prev = JSON.parse(fs.readFileSync(file, 'utf8'));
  const bdir = path.join(outDir, 'baseline');
  fs.mkdirSync(bdir, { recursive: true });
  const views = {};
  for (const v of prev.views ?? []) {
    let png = null;
    if (v.png && fs.existsSync(path.join(dir, v.png))) {
      png = path.basename(v.png);
      fs.copyFileSync(path.join(dir, v.png), path.join(bdir, png));
    }
    views[v.id] = { png, fps: v.fps ?? null, counters: v.counters ?? null, status: v.status };
  }
  return { dir, runId: prev.runId, label: prev.label, commit: prev.commit, console: prev.console, views };
}

function consoleKeys(groups) {
  const out = {};
  for (const [k, list] of Object.entries(groups ?? {})) out[k] = new Set(list.map((m) => m.text.slice(0, 300)));
  return out;
}

// ------------------------------------------------------------------------------- main

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const runId = stamp();
  const outDir = opts.out ?? path.join(REPO_ROOT, 'artifacts', 'map-captures', runId);
  const t0 = Date.now();
  const log = (s) => process.stdout.write(`[capture:map +${((Date.now() - t0) / 1000).toFixed(1)}s] ${s}\n`);

  const server = await ensureDevServer(opts.port);
  fs.mkdirSync(outDir, { recursive: true });
  log(server.reused ? `reusing dev server ${server.base}${server.verified ? ' (serves this checkout)' : ' (checkout NOT verified)'}` : `started dev server ${server.base}`);
  const stopServer = () => {
    if (server.child && server.child.exitCode === null) server.child.kill();
  };
  process.on('SIGINT', () => {
    stopServer();
    process.exit(130);
  });

  let browser;
  const messages = [];
  let phase = 'load';
  const report = {
    tool: 'capture:map (prototype)',
    runId,
    label: opts.label,
    commit: git(['rev-parse', '--short', 'HEAD']),
    branch: git(['rev-parse', '--abbrev-ref', 'HEAD']),
    dirty: (git(['status', '--porcelain']) ?? '').length > 0,
    options: { ...opts, baseline: opts.baseline, out: undefined },
    server: { base: server.base, reused: server.reused, verified: server.verified },
    environment: {},
    timings: {},
    world: null,
    views: [],
    console: null,
    caveats: [
      'Not a gate: nothing here passes or fails on how the picture looks.',
      'landscape.html under Vite dev in Chrome, not the Tauri desktop app; FPS is not the Iris Xe target.',
      'Headless FPS is indicative only; check environment.gl.renderer for a software rasteriser.',
      'Creatures are the ?replay=synthetic fixture: procedural test data, not simulation output.',
      'The scene keeps animating (water, weather, creatures) while it is captured; two runs are not byte-identical.',
    ],
  };

  try {
    const launched = await launchBrowser(opts.headed, opts.vsync);
    browser = launched.browser;
    report.environment = {
      browser: launched.kind,
      browserVersion: browser.version(),
      headless: !opts.headed,
      fpsCap: opts.vsync ? 'display vsync' : 'uncapped (--disable-gpu-vsync --disable-frame-rate-limit)',
      platform: `${process.platform}-${process.arch}`,
    };
    log(`browser: ${launched.kind} ${browser.version()}${opts.headed ? ' (headed)' : ' (headless)'}`);

    const context = await browser.newContext({ viewport: opts.viewport, deviceScaleFactor: 1 });
    const page = await context.newPage();
    page.on('console', (m) => {
      const type = m.type();
      if (type !== 'error' && type !== 'warning') return;
      const where = m.location()?.url;
      // "Failed to load resource: 404" names no URL in its text; the location does.
      const text = /^Failed to load resource/.test(m.text()) && where ? `${m.text()} — ${where}` : m.text();
      messages.push({ type, text, phase });
    });
    page.on('pageerror', (e) => messages.push({ type: 'pageerror', text: String(e?.stack || e), phase }));

    const url = `${server.base}/landscape.html?replay=synthetic&agents=${opts.agents}&segments=${opts.segments}${opts.quality === 'auto' ? '' : `&quality=${opts.quality}`}${opts.worldRoot === 'world-v2' ? '' : `&worldV2Root=${opts.worldRoot}`}`;
    report.url = url;
    log(`opening ${url}`);
    await page.goto(url, { waitUntil: 'domcontentloaded' });

    // Readiness: the page publishes the generated world on `window.__world` and the scene graph
    // on `window.__worldScene`; the terrain mesh existing means the canvas has the world in it.
    // Until then the page shows "Generating a huge 2048² world…" (10 s+ on a fresh profile).
    const tWorld = Date.now();
    await page.waitForFunction(
      () => {
        const w = window.__world;
        const s = window.__worldScene;
        return !!(w && w.elevation && w.size && s && (s.getObjectByName('world-terrain') || s.getObjectByName('world-terrain-lod')));
      },
      null,
      { timeout: 300_000, polling: 500 },
    );
    report.timings.worldReadyMs = Date.now() - tWorld;
    log(`world ready after ${(report.timings.worldReadyMs / 1000).toFixed(1)} s`);
    // Let instanced flora and the first shadow pass upload, and the synthetic replay start.
    await page.evaluate(pageWaitFrames, { minFrames: 30, minMs: 1500, capMs: 30_000 });
    if (!(await page.evaluate(pageInstallPin))) throw new Error('window.__worldScene missing: cannot place the camera');
    if (opts.agents > 0) {
      const t = Date.now();
      await page
        .waitForFunction(
          () => {
            let n = 0;
            window.__worldScene.traverse((o) => {
              if (o.isInstancedMesh && /live-agent|creature/i.test(o.name || o.parent?.name || '')) n += o.count;
            });
            return n > 0;
          },
          null,
          { timeout: 30_000, polling: 250 },
        )
        .catch(() => log('no creature instances appeared within 30 s'));
      report.timings.creaturesReadyMs = Date.now() - t;
    }
    await page.evaluate(pageWaitFrames, { minFrames: 5, minMs: 100, capMs: 10_000 });
    report.environment.gl = await page.evaluate(pageGlInfo);
    report.environment.softwareGl = /swiftshader|llvmpipe|software|basic render/i.test(report.environment.gl?.renderer ?? '');
    log(`WebGL: ${report.environment.gl?.renderer ?? 'unknown'}${report.environment.softwareGl ? '  <-- SOFTWARE RASTERISER' : ''}`);

    // World extent / elevation span for the metre-based optional views, read from the World v2
    // manifest and the format constants (no hard-coded map size).
    await page.evaluate((m) => (window.__mapCaptureWorld = m), readWorldMetrics(opts.worldRoot));
    const analysis = await page.evaluate(pageAnalyseWorld);
    await page.evaluate((s) => (window.__mapCaptureScale = s), { renderSize: analysis.renderSize, heightUnits: analysis.heightUnits });
    report.world = { size: analysis.worldSize, renderSize: analysis.renderSize, heightUnits: Math.round(analysis.heightUnits * 10) / 10, seaLevel: analysis.seaLevel, notes: analysis.notes };

    if (!opts.hud) {
      // Everything but the WebGL canvas is HUD: hide it so the pictures show the world, not the app.
      // The minimap and compass draw on canvases of their own, so "keep what holds a canvas" is not
      // enough: keep exactly the ancestors of the renderer's own canvas and hide every sibling.
      await page.evaluate(pageHideHud);
    }

    for (const id of opts.views) {
      phase = id;
      const spec = id === 'creature' ? { status: 'ok' } : analysis.views[id];
      const entry = { id, status: spec?.status ?? 'skipped', reason: spec?.reason, feature: spec?.feature, pose: spec?.pose, framing: spec?.framing };
      report.views.push(entry);
      if (entry.status !== 'ok') {
        log(`${id}: skipped — ${entry.reason}`);
        continue;
      }
      await page.evaluate(pageInstallPin); // re-wrap if the page re-installed its own hook
      if (id === 'creature') {
        let f = await page.evaluate(pageFollowCreature, 'approach');
        if (f.ok) {
          await page.evaluate(pageWaitFrames, { minFrames: 30, minMs: 1000, capMs: 15_000 });
          f = await page.evaluate(pageFollowCreature, 'follow');
        }
        if (!f.ok) {
          entry.status = 'skipped';
          entry.reason = f.reason;
          log(`${id}: skipped — ${f.reason}`);
          continue;
        }
        entry.feature = `synthetic creature #${f.index} of ${f.instances} (${f.mesh}), radius ${f.radius}, camera ${f.distance} away, followed; ${f.treesNearby} trees within 12 units; line of sight ${f.lineOfSight}`;
      } else {
        await page.evaluate(pageSetPose, spec.pose);
      }
      // Settle: chunks, impostors and shadow cascades follow the camera; give them frames.
      await page.evaluate(pageWaitFrames, { minFrames: 40, minMs: 1500, capMs: 20_000 });
      const png = `${String(report.views.length).padStart(2, '0')}-${id}.png`;
      await page.screenshot({ path: path.join(outDir, png) });
      entry.png = png;
      const s = await page.evaluate(pageSampleFrames, opts.seconds * 1000);
      entry.fps = frameStats(s.times);
      if (s.counters) {
        entry.counters = {
          source: 'window.__animaFrameCounters',
          total: { calls: median(s.totals.map((t) => t[0])), triangles: median(s.totals.map((t) => t[1])) },
          lastFrame: s.counters,
        };
      } else if (s.rendererInfo) {
        entry.counters = { source: 'renderer.info (main pass only)', total: s.rendererInfo };
      }
      if (id === 'creature') {
        const followed = await page.evaluate(() => window.__mapCapture.followed);
        if (followed) entry.pose = { position: null, target: followed.map((n) => Math.round(n * 10) / 10) };
      }
      log(`${id}: ${entry.fps.medianFps ?? '–'} fps median, ${entry.fps.p5Fps ?? '–'} p5, ${entry.counters?.total?.calls ?? '–'} calls, ${fmt(entry.counters?.total?.triangles)} tris`);
    }
    phase = 'end';
    await page.evaluate(pageWaitFrames, { minFrames: 2, minMs: 200, capMs: 5000 });
    await context.close();
  } catch (e) {
    // Keep what was captured so far: a half run with its console log explains more than a stack.
    report.error = String(e?.stack || e);
  } finally {
    if (browser) await browser.close().catch(() => {});
    stopServer();
  }

  report.console = summariseConsole(messages);
  if (opts.baseline) {
    report.baseline = loadBaseline(opts.baseline, outDir);
    const before = consoleKeys(report.baseline.console);
    report.baseline.newConsole = {};
    for (const [k, list] of Object.entries(report.console)) {
      report.baseline.newConsole[k] = list.filter((m) => !(before[k]?.has(m.text.slice(0, 300)))).length;
    }
    delete report.baseline.console;
  }
  report.timings.totalMs = Date.now() - t0;
  fs.writeFileSync(path.join(outDir, 'report.json'), JSON.stringify(report, null, 2));
  fs.writeFileSync(path.join(outDir, 'index.html'), renderSheet(report));

  // ---- terminal summary
  const rows = report.views.map((v) => {
    const b = report.baseline?.views?.[v.id];
    const d = (a, c) => (typeof a === 'number' && typeof c === 'number' ? ` (${a - c >= 0 ? '+' : ''}${Math.round((a - c) * 10) / 10})` : '');
    return v.status === 'ok'
      ? `  ${v.id.padEnd(10)} ${String(v.fps?.medianFps ?? '–').padStart(6)}${d(v.fps?.medianFps, b?.fps?.medianFps).padEnd(9)} ${String(v.fps?.p5Fps ?? '–').padStart(6)}${d(v.fps?.p5Fps, b?.fps?.p5Fps).padEnd(9)} ${String(v.counters?.total?.calls ?? '–').padStart(6)} ${fmt(v.counters?.total?.triangles).padStart(12)}`
      : `  ${v.id.padEnd(10)} skipped: ${v.reason}`;
  });
  const c = report.console;
  process.stdout.write(
    `\n  view        FPS med           FPS p5        calls    triangles\n${rows.join('\n')}\n\n` +
      `  console: ${c.pageErrors.length} page errors, ${c.errors.length} errors, ${c.warnings.length} warnings; ` +
      `${c.expectedNoBackend.length} expected no-backend (Tauri) messages\n` +
      (report.environment.softwareGl ? '  WARNING: software WebGL — FPS figures are meaningless.\n' : '') +
      `\n  output: ${outDir}\n  open:   ${path.join(outDir, 'index.html')}\n`,
  );
  if (report.error) {
    process.stderr.write(`\n[capture:map] run failed part-way (partial report written):\n${report.error}\n`);
    process.exitCode = 1;
  }
}

main().catch((e) => {
  process.stderr.write(`[capture:map] ${e?.stack || e}\n`);
  process.exit(1);
});
