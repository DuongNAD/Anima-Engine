// ESLint warning ratchet.
//
// The baseline is zero. `npm run lint` reports no warnings and no errors, and this script's job is
// now to keep it that way: it fails when the total goes UP, which at a baseline of zero means it
// fails on the first new warning rather than after a slow drift.
//
// Errors are handled by `npm run lint` itself and are not this script's job.
//
// Run:  node scripts/eslint_ratchet.mjs
// Env:  ESLINT_WARNING_BASELINE  overrides the baseline below (CI does not set it).

import { ESLint } from 'eslint';
import process from 'node:process';

// Measured on feature-anima-completion, 2026-07-27, with 0 errors.
//
// History: 444 -> 440 when `src/hooks/useSimulation.ts` (dead, no importer) was deleted; 440 -> 491
// on the eslint 9 -> 10 upgrade, taken to clear a high-severity brace-expansion advisory reachable
// only through eslint's pinned minimatch. That upgrade brought eslint-plugin-react-hooks 7 and its
// React Compiler rules (immutability, purity, refs, set-state-in-effect), which are errors by
// default and fired on 53 pre-existing findings in the R3F and Pixi components. Setting them to
// `warn` in eslint.config.js meant the security fix was not gated behind an unrelated refactor.
//
// 491 -> 483 when `ecosystem.html` and `webgl-test.html` left the shipped surface (they load
// unpinned scripts from a CDN; see the plugin in vite.config.ts) — not a lint cleanup, so it was
// locked in here rather than spent. 483 -> 267 over two passes that typed the 3D scene and the Pixi
// viewport for real. 267 -> 0 by finishing the job: every remaining `any`, every React Compiler
// finding and the six `eslint-disable-next-line` directives are gone, resolved in the code rather
// than in configuration. Three of those findings were live defects, described in the commits that
// removed them.
//
// Only ever lower this number. There is nowhere left to lower it to, which is the point: a warning
// is now a thing that did not exist a moment ago, and the commit that introduced it is the one
// holding it.
const DEFAULT_BASELINE = 0;

const parsed = Number.parseInt(process.env.ESLINT_WARNING_BASELINE ?? '', 10);
const limit = Number.isFinite(parsed) ? parsed : DEFAULT_BASELINE;

// The Node API rather than a subprocess: the `eslint` and `npx` entry points are .cmd shims on
// Windows, which Node refuses to spawn without a shell.
const eslint = new ESLint();
const results = await eslint.lintFiles(['.']);

let warnings = 0;
let errors = 0;
for (const file of results) {
  warnings += file.warningCount;
  errors += file.errorCount;
}

console.log(`eslint: ${errors} errors, ${warnings} warnings (baseline ${limit})`);

if (warnings > limit) {
  console.error(
    `\nESLint warnings went up: ${warnings} > ${limit}.\n` +
      'Fix the new warnings rather than raising the baseline.',
  );
  process.exit(1);
}

if (warnings < limit) {
  console.log(
    `\nWarnings are down to ${warnings}. Lower DEFAULT_BASELINE in ` +
      'scripts/eslint_ratchet.mjs to lock the improvement in.',
  );
}
