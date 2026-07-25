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

// Measured on chore/init-and-frontend-test-fixes, 2026-07-25, with 0 errors.
// Only ever lower this number.
const DEFAULT_BASELINE = 445;

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
