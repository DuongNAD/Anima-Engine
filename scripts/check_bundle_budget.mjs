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
import { join, relative, basename } from 'node:path';

const ROOT = process.cwd();
const DIST = process.argv[2] ?? join(ROOT, 'dist');
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
  main: 200,
  // Deterministic world generation, shared by the app and the landscape entry.
  sharedWorld: 200,
  landscape: 120,
  Geometry: 140,
  CanvasRenderer: 120,
  RenderTargetSystem: 110,
  LandscapeShowcase: 90,
  default: 80,
};

/** Total shipped JS, all chunks. Guards against death by a thousand small chunks. */
const TOTAL_BUDGET_KIB = 2000;

if (!existsSync(DIST)) {
  console.error(`missing ${relative(ROOT, DIST)} — run \`npm run build\` first.`);
  process.exit(2);
}

const assets = join(DIST, 'assets');
if (!existsSync(assets)) {
  console.error(`missing ${relative(ROOT, assets)} — the build output looks wrong.`);
  process.exit(2);
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
  console.error(`no .js in ${relative(ROOT, assets)} — the build output looks wrong.`);
  process.exit(2);
}

const failures = [];
let total = 0;

for (const f of files) {
  total += f.kib;
  const budget = CHUNK_BUDGET_KIB[f.name] ?? CHUNK_BUDGET_KIB.default;
  const explicit = f.name in CHUNK_BUDGET_KIB;
  if (f.kib > budget) {
    failures.push(
      `${f.name} is ${f.kib.toFixed(1)} KiB, budget ${budget} KiB` +
        (explicit ? '' : ' (the `default` budget — a new chunk this large needs its own entry)'),
    );
  }
}

if (total > TOTAL_BUDGET_KIB) {
  failures.push(`total shipped JS is ${total.toFixed(1)} KiB, budget ${TOTAL_BUDGET_KIB} KiB`);
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
const ENTRY_HTML = { 'index.html': '2D dashboard', 'landscape.html': 'landscape scene' };
/** Chunks that must never be reachable from the dashboard entry's static preload graph. */
const THREE_D_CHUNK = /^react-three-fiber/;

for (const [file, label] of Object.entries(ENTRY_HTML)) {
  const p = join(DIST, file);
  if (!existsSync(p)) {
    failures.push(`${file} is missing from dist/ — was the build run?`);
    continue;
  }
  const html = readFileSync(p, 'utf8');
  const referenced = [...html.matchAll(/(?:src|href)="\/assets\/([^"]+\.js)"/g)].map((m) => m[1]);
  const loadsThreeD = referenced.some((n) => THREE_D_CHUNK.test(n));

  if (file === 'index.html' && loadsThreeD) {
    failures.push(
      `the ${label} (${file}) statically loads the three.js chunk. It renders 2D only; something ` +
        `now imports three (or react-three-fiber) at module scope from the dashboard entry. Import ` +
        `it lazily, or the dashboard pays 836 KiB for a renderer it does not use.`,
    );
  }
  if (file === 'landscape.html' && !loadsThreeD) {
    // The other direction, so the check above cannot pass because the chunk stopped existing.
    failures.push(
      `the ${label} (${file}) does not reference the three.js chunk. Either the 3D scene stopped ` +
        `loading its renderer, or the chunk was renamed and this gate is now vacuous.`,
    );
  }
  console.log(
    `  entry ${file.padEnd(16)} ${String(referenced.length).padStart(3)} static chunk(s), ` +
      `3D renderer: ${loadsThreeD ? 'yes' : 'no'}`,
  );
}

const width = Math.max(...files.map((f) => f.name.length));
console.log(`bundle budget — ${files.length} chunk(s), ${total.toFixed(1)} KiB total\n`);
for (const f of files.slice(0, 12)) {
  const budget = CHUNK_BUDGET_KIB[f.name] ?? CHUNK_BUDGET_KIB.default;
  const pct = ((f.kib / budget) * 100).toFixed(0);
  const flag = f.kib > budget ? 'OVER' : '    ';
  console.log(`  ${flag} ${f.name.padEnd(width)}  ${f.kib.toFixed(1).padStart(8)} KiB  ${pct.padStart(4)}% of ${budget}`);
}

if (failures.length > 0) {
  console.error(`\nbundle budget FAILED:\n`);
  for (const f of failures) console.error(`  - ${f}`);
  console.error(
    `\nRaising a budget is a decision, not a fix. If the growth is intended, raise the number in ` +
      `scripts/check_bundle_budget.mjs in the same commit that causes it, and say why.`,
  );
  process.exit(1);
}

console.log(`\nOK — every chunk within budget, total ${total.toFixed(1)}/${TOTAL_BUDGET_KIB} KiB`);
