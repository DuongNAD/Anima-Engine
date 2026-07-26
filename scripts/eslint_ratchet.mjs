// ESLint warning ratchet.
//
// `npm run lint` currently reports a large number of warnings -- legacy `any` on the Tauri IPC
// payloads and unused vars. Rewriting them in one pass is not worth the churn and would bury real
// review, so instead the count is frozen: this script fails when the total goes UP, and tells you to
// lower the baseline when it goes down.
//
// Errors are handled by `npm run lint` itself and are not this script's job.
//
// Run:  node scripts/eslint_ratchet.mjs
// Env:  ESLINT_WARNING_BASELINE  overrides the baseline below (CI does not set it).

import { ESLint } from 'eslint';
import process from 'node:process';

// Measured on chore/audit-remediation, 2026-07-26, with 0 errors.
//
// History: 444 -> 440 when `src/hooks/useSimulation.ts` (dead, no importer) was deleted; 440 -> 491
// on the eslint 9 -> 10 upgrade, which was taken to clear a high-severity brace-expansion advisory
// reachable only through eslint's pinned minimatch. That upgrade brings eslint-plugin-react-hooks 7
// and its React Compiler rules (immutability, purity, refs, set-state-in-effect), which are errors
// by default and fire on 53 pre-existing findings in the R3F and Pixi components. They are set to
// `warn` in eslint.config.js so the security fix was not gated behind an unrelated refactor; the
// note there lists the three that look like real defects and should go first.
//
// Only ever lower this number.
//
// 2026-07-27: 491 -> 483. Not from a lint cleanup — the eight came off when `ecosystem.html` and
// `webgl-test.html` stopped being part of the shipped surface (they load unpinned scripts from a
// CDN; see the plugin in vite.config.ts). Locked in here so the reduction cannot silently be spent.
const DEFAULT_BASELINE = 483;

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
