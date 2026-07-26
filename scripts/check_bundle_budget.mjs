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

import { readdirSync, statSync, existsSync } from 'node:fs';
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
