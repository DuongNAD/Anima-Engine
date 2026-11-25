#!/usr/bin/env node
// Fail the build when a JavaScript chunk, or the bundle as a whole, grows past its budget.
//
// # Why a budget and not just a warning
//
// Vite already prints "(!) Some chunks are larger than 500 kB after minification" and has printed it
// for a long time. A warning that is always on is a warning nobody reads; it cannot distinguish
// "still 856 kB" from "now 1.4 MB". A budget with a number in it can, and it turns a gradual
// regression into a failing command with a diff.
//
// # How the numbers were chosen
//
// Measured, then given deliberate headroom — not aspirational. Anything below the current size would
// fail on the first run and be raised by the next person, which is how budgets become decoration.
// The right way to *lower* these is to land the split first and then tighten the number in the same
// commit, so the file always records what the build actually does.
//
// The 3D chunk is the interesting one. `react-three-fiber.esm` is ~840 KiB because three.js is
// bundled into it, and it is *already* a separate chunk: it does not load until something renders 3D.
// So the cost is real but deferred, which is why its budget is separate from — and much larger than
// — the entry budget. What matters for startup is `main`, and that is budgeted tightly.
//
// Usage: node scripts/check_bundle_budget.mjs [distDir]

import { readdirSync, statSync, existsSync, readFileSync } from 'node:fs';
import { join, relative, basename, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const ROOT = process.cwd();
const KIB = 1024;

/**
 * Per-chunk budgets in KiB, matched by filename prefix (Vite appends a content hash).
 *
 * `default` applies to any chunk without a specific entry, which is what catches a *new* fat chunk
 * appearing — the failure mode a total-only budget misses entirely.
 */
const CHUNK_BUDGET_KIB = {
  // three.js + the R3F reconciler. Lazy: not fetched until a 3D view mounts.
  'react-three-fiber.esm': 900,
  // The main entry. This is the one that gates time-to-interactive, so it is the tight one.
  //
  // Re-measured 2026-11-14 across Vite 8.1.5 (03b331f, rolldown 1.1.5) vs Vite 8.3.5 (78b2e85,
  // rolldown 1.2.16): rolldown 1.2.16 stopped hoisting `react-dom/client` (`client`, 130.38 KiB),
  // `CanvasRenderer` (84.16 KiB), one `init` chunk (24.15 KiB) and `jsx-runtime` (6.95 KiB) into
  // separate chunks, folding them into `main` (160.08 -> 267.68 KiB) and the pre-existing
  // `defineProperty` chunk (0.69 -> 137.88 KiB). Measured static eager-preload totals actually
  // decreased slightly from 03b331f to 78b2e85 (`index.html`: 632,413 B / 617.59 KiB across 16
  // chunks -> 631,399 B / 616.60 KiB across 12 chunks; `landscape.html`: 1106.40 -> 1104.33 KiB;
  // `map2.html`: 1105.37 -> 1103.04 KiB; all-JS total: 1849.90 -> 1846.55 KiB). Raised from 200
  // to 320 (~20% headroom above 267.68 KiB) and removed the vanished `client` and `CanvasRenderer`
  // keys; `init` remains emitted (`15.15 KiB`) under `default: 80`.
  main: 320,
  // Shared rolldown/oxc runtime helper chunk into which rolldown 1.2.16 folded shared Pixi/app code
  // formerly in `CanvasRenderer` (`0.69 KiB` in 03b331f -> `137.88 KiB` in 78b2e85). Budget 170
  // gives ~23% headroom, matching the former `client: 160` entry.
  defineProperty: 170,
  // Deterministic world generation, shared by the app and the landscape entry.
  sharedWorld: 200,
  landscape: 120,
  Geometry: 140,
  RenderTargetSystem: 110,
  LandscapeShowcase: 90,
  // The shared-world scene. It became a chunk of the *dashboard's* graph when the Landscape button
  // stopped opening the 160-cell `LandscapeShowcase` and started opening the 2048² world the
  // simulation actually runs in — one world on screen instead of two, and the live population drawn
  // on it.
  //
  // Measured at 106.2 KiB when that landed; 130 gives the same kind of headroom the entries above
  // carry. It is lazy behind the same button, so it costs nothing until someone opens the map, and
  // it is not additive: `LandscapeShowcase` left the dashboard's graph in the same change.
  // Raised 130 -> 145 KiB (2026-11-19, reason: 'prototype' — V1 real-world simulation entities
  // `LiveEnvironment` + `simEnvironment` + decoration legend/toggle grew WorldShowcase from
  // ~127.5 KiB to ~132.8 KiB; authorized under PROTOTYPE-FIRST MSG_20261119_040227),
  // 145 -> 155 KiB (2026-11-19, reason: 'prototype' — B1 continuous SkinnedMesh bodies +
  // 3-tier LOD `creatureSkinMesh.ts` + `creatureLod.ts`; combined V1 + B1 measured at 154.8 KiB),
  // and 155 -> 175 KiB (2026-11-19, reason: 'prototype' — W3-lite CC0 PBR terrain splatting &
  // instanced flora wind `terrainPbrMaterial.ts`, `floraWindMaterial.ts`, `worldAssets.ts`;
  // lazy-loaded only on 3D routes).
  // Raised 175 -> 225 KiB (2026-11-26, reason: 'prototype, WV2-6' — measured 206.7 KiB after
  // moving `loadWorldV2Cached` out of `worldCache.ts` into `worldV2/tileLoader.ts`, which
  // keeps `sharedWorld` at trunk size 28.3 KiB and puts all SHA-256/KTX2/ANMC/ANMF viewer
  // code exclusively behind the lazy 3D route).
  WorldShowcase: 225,
  Map2Showcase: 130,
  map2: 120,
  // The interactive research dashboard suite (Features 14-19 & DataExportDialog).
  // Measured at 88.6 KiB (21.5 KiB gzipped); 120 gives standard headroom.
  ResearchDashboard: 120,
  default: 80,
};

/**
 * Total shipped JS, all chunks. Guards against death by a thousand small chunks.
 * Raised 2000 -> 2006 KiB (W11 Fix Round 1, R1, measured 2004.2 KiB): restores main-thread
 * fallback generation when the world worker fails in production (`worldCache.ts`), plus `.catch`
 * error UI in `WorldShowcase.tsx` and `App.tsx`. Note: `generateWorld` remains in `sharedWorld`
 * (`28.8 KiB`, vs trunk `28.93 kB`) rather than a separate lazy chunk because `WorldMinimap` and
 * sibling components statically import `Biome`/`FloraType`/`WORLD_GEN_VERSION` from `worldGen.ts`.
 * Raised 2006 -> 2060 KiB (2026-11-26, reason: 'prototype, WV2-6' — measured 2039.3 KiB; adds
 * lazy-loaded World v2 viewer codecs/SHA-256 loader & `fflate` zlib decompression in 3D routes;
 * `main` startup bundle stays unchanged at 273.7 KiB <= 320 KiB).
 */
const TOTAL_BUDGET_KIB = 2060;

export const ENTRY_HTML = {
  'index.html': '2D dashboard',
  'landscape.html': 'landscape scene',
  'map2.html': 'map 2 scene',
};
/** Chunks that must never be reachable from the dashboard entry's static preload graph. */
export const THREE_D_CHUNK = /^react-three-fiber/;
/**
 * Signature tokens emitted by `three` (`window.__THREE__ = REVISION` and unmangled `THREE.*`
 * identifiers/warnings), avoiding false positives on PixiJS's `WebGLRenderer`/`Matrix4` or Vite's
 * lazy-preload filename strings.
 */
export const THREE_D_CONTENT = /__THREE__\b|\bTHREE\.[A-Z][A-Za-z0-9_]+\b/;

const STATIC_ESM_IMPORT_RE =
  /(?:^|[;}\n])\s*import\s*(?:[^('"`;]+?\s*from\s*)?["']\.\/([^"']+\.js)["']/g;

export function hasThreeJsSignature(jsSource) {
  return THREE_D_CONTENT.test(jsSource);
}

export function extractHtmlJsRefs(html) {
  return [...html.matchAll(/(?:src|href)="\/?assets\/([^"]+\.js)"/g)].map((m) => m[1]);
}

export function collectStaticChunkClosure(entryChunks, assetsDir) {
  const visited = new Set();
  const queue = [...entryChunks];
  const chunks = [];

  while (queue.length > 0) {
    const file = queue.shift();
    if (!file || visited.has(file)) continue;
    visited.add(file);

    const fullPath = join(assetsDir, file);
    if (!existsSync(fullPath)) {
      chunks.push({ file, content: '', missing: true });
      continue;
    }

    const content = readFileSync(fullPath, 'utf8');
    chunks.push({ file, content, missing: false });

    STATIC_ESM_IMPORT_RE.lastIndex = 0;
    let match;
    while ((match = STATIC_ESM_IMPORT_RE.exec(content)) !== null) {
      const dep = match[1];
      if (!visited.has(dep)) {
        queue.push(dep);
      }
    }
  }

  return chunks;
}

export function checkBundleBudget(distDir = join(ROOT, 'dist')) {
  if (!existsSync(distDir)) {
    return {
      ok: false,
      exitCode: 2,
      error: `missing ${relative(ROOT, distDir)} — run \`npm run build\` first.`,
      files: [],
      totalKib: 0,
      entryReports: [],
      failures: [],
    };
  }

  const assets = join(distDir, 'assets');
  if (!existsSync(assets)) {
    return {
      ok: false,
      exitCode: 2,
      error: `missing ${relative(ROOT, assets)} — the build output looks wrong.`,
      files: [],
      totalKib: 0,
      entryReports: [],
      failures: [],
    };
  }

  const files = readdirSync(assets)
    .filter((f) => f.endsWith('.js'))
    .map((f) => {
      const full = join(assets, f);
      const kib = statSync(full).size / KIB;
      // Strip Vite's `-<hash>.js` suffix to get the stable chunk name.
      const name = basename(f).replace(/-[A-Za-z0-9_-]{6,}\.js$/, '');
      return { file: f, name, kib };
    })
    .sort((a, b) => b.kib - a.kib);

  if (files.length === 0) {
    return {
      ok: false,
      exitCode: 2,
      error: `no .js in ${relative(ROOT, assets)} — the build output looks wrong.`,
      files: [],
      totalKib: 0,
      entryReports: [],
      failures: [],
    };
  }

  const failures = [];
  let totalKib = 0;

  for (const f of files) {
    totalKib += f.kib;
    const budget = CHUNK_BUDGET_KIB[f.name] ?? CHUNK_BUDGET_KIB.default;
    const explicit = f.name in CHUNK_BUDGET_KIB;
    if (f.kib > budget) {
      failures.push(
        `${f.name} is ${f.kib.toFixed(1)} KiB, budget ${budget} KiB` +
          (explicit ? '' : ' (the `default` budget — a new chunk this large needs its own entry)'),
      );
    }
  }

  if (totalKib > TOTAL_BUDGET_KIB) {
    failures.push(`total shipped JS is ${totalKib.toFixed(1)} KiB, budget ${TOTAL_BUDGET_KIB} KiB`);
  }

  // ---- the split, checked as behaviour rather than as a number ----------------------------------
  //
  // A byte ceiling around the 836 KiB three.js/react-three-fiber chunk is regression protection and
  // nothing more: it would still pass on the day someone adds `import * as THREE from 'three'` to
  // `App.tsx` and doubles what the 2D dashboard downloads, because the chunk's own size would not
  // change. The property that matters is *which route pays for it*.
  //
  // Measured against the built output, 2026-07-27, with `vite preview` and a real browser:
  //
  //   /                 17 JS files fetched — react-three-fiber chunk: NOT fetched
  //   /landscape.html    9 JS files fetched — react-three-fiber chunk: fetched
  //
  // So the chunk is already a route boundary, which is the correct architectural answer here: it is
  // the three.js runtime, every consumer of it needs all of it, and the only page that renders 3D is
  // the only page that loads it. Splitting three.js internally is tree-shaking, which rolldown
  // already does; splitting r3f from three would produce two chunks that are always fetched together.
  //
  // The remaining 836 KiB is three.js itself, and it is honestly still 836 KiB — that debt is scored,
  // not resolved. What this gate adds is that the debt cannot silently spread to the dashboard.
  const entryReports = [];

  for (const [file, label] of Object.entries(ENTRY_HTML)) {
    const p = join(distDir, file);
    if (!existsSync(p)) {
      failures.push(`${file} is missing from dist/ — was the build run?`);
      entryReports.push({
        file,
        label,
        referenced: [],
        reachable: [],
        loadsThreeD: false,
        missing: true,
      });
      continue;
    }
    const html = readFileSync(p, 'utf8');
    const referenced = extractHtmlJsRefs(html);
    const reachable = collectStaticChunkClosure(referenced, assets);
    const loadsThreeD = reachable.some(
      (chunk) => THREE_D_CHUNK.test(chunk.file) || hasThreeJsSignature(chunk.content),
    );

    if (file === 'index.html' && loadsThreeD) {
      failures.push(
        `the ${label} (${file}) statically loads the three.js chunk. It renders 2D only; something ` +
          `now imports three (or react-three-fiber) at module scope from the dashboard entry. Import ` +
          `it lazily, or the dashboard pays 836 KiB for a renderer it does not use.`,
      );
    }
    if ((file === 'landscape.html' || file === 'map2.html') && !loadsThreeD) {
      // The other direction, so the check above cannot pass because the chunk stopped existing.
      failures.push(
        `the ${label} (${file}) does not reference the three.js chunk. Either the 3D scene stopped ` +
          `loading its renderer, or the chunk was renamed and this gate is now vacuous.`,
      );
    }
    entryReports.push({
      file,
      label,
      referenced,
      reachable,
      loadsThreeD,
      missing: false,
    });
  }

  return {
    ok: failures.length === 0,
    exitCode: failures.length > 0 ? 1 : 0,
    files,
    totalKib,
    entryReports,
    failures,
  };
}

function main() {
  const distDir = process.argv[2] ?? join(ROOT, 'dist');
  const result = checkBundleBudget(distDir);

  if (result.exitCode === 2) {
    console.error(result.error);
    process.exit(2);
  }

  for (const r of result.entryReports) {
    if (r.missing) continue;
    console.log(
      `  entry ${r.file.padEnd(16)} ${String(r.referenced.length).padStart(3)} static chunk(s), ` +
        `3D renderer: ${r.loadsThreeD ? 'yes' : 'no'}`,
    );
  }

  const width = Math.max(...result.files.map((f) => f.name.length));
  console.log(`bundle budget — ${result.files.length} chunk(s), ${result.totalKib.toFixed(1)} KiB total\n`);
  for (const f of result.files.slice(0, 12)) {
    const budget = CHUNK_BUDGET_KIB[f.name] ?? CHUNK_BUDGET_KIB.default;
    const pct = ((f.kib / budget) * 100).toFixed(0);
    const flag = f.kib > budget ? 'OVER' : '    ';
    console.log(`  ${flag} ${f.name.padEnd(width)}  ${f.kib.toFixed(1).padStart(8)} KiB  ${pct.padStart(4)}% of ${budget}`);
  }

  if (result.failures.length > 0) {
    console.error(`\nbundle budget FAILED:\n`);
    for (const f of result.failures) console.error(`  - ${f}`);
    console.error(
      `\nRaising a budget is a decision, not a fix. If the growth is intended, raise the number in ` +
        `scripts/check_bundle_budget.mjs in the same commit that causes it, and say why.`,
    );
    process.exit(1);
  }

  console.log(`\nOK — every chunk within budget, total ${result.totalKib.toFixed(1)}/${TOTAL_BUDGET_KIB} KiB`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main();
}
