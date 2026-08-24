#!/usr/bin/env node
// Check the built frontend against the production Content-Security-Policy declared in
// `src-tauri/tauri.conf.json`.
//
// # Why this gate exists
//
// `security.csp` was `null` — no policy at all — and setting one is only half the work. A CSP that
// the shipped assets violate gets reverted by the next person who hits a blank window, and the
// surface goes back to open. The policy therefore needs something that fails at build time rather
// than at run time in a webview nobody can attach a debugger to.
//
// It is also how the concrete finding surfaced: `public/ecosystem.html` loads three.js and
// simplex-noise from `cdnjs.cloudflare.com`, unpinned, and Vite copies `public/` verbatim — so every
// desktop build was packaging a page that fetches and executes remote code. `default-src 'self'`
// blocks it at runtime, but the page should not ship at all, which is what the
// `anima-exclude-dev-only-public-pages` plugin in vite.config.ts now handles. This script is what
// keeps that true.
//
// # What it can and cannot prove
//
// It checks the *shipped artifacts* against the *declared policy*. It does NOT prove the app runs
// under that policy — that needs the Tauri webview, and CLAUDE.md forbids running the full backend
// on this machine. Treat a green run here as "nothing in dist/ obviously violates the CSP", not as
// "the app works". The live check is a human running `npm run tauri:dev`.
//
// Usage: node scripts/check_csp_compat.mjs [distDir]

import { readFileSync, existsSync, readdirSync, statSync } from 'node:fs';
import { join, relative, extname } from 'node:path';

const ROOT = process.cwd();
const DIST = process.argv[2] ?? join(ROOT, 'dist');
const CONF = join(ROOT, 'src-tauri', 'tauri.conf.json');

/** Collect every file under `dir` recursively. */
function walk(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) out.push(...walk(full));
    else out.push(full);
  }
  return out;
}

const problems = [];
const note = (file, msg) => problems.push(`${relative(ROOT, file)}: ${msg}`);

// ---- the declared policy --------------------------------------------------------------------
if (!existsSync(CONF)) {
  console.error(`missing ${relative(ROOT, CONF)}`);
  process.exit(2);
}
const conf = JSON.parse(readFileSync(CONF, 'utf8'));
const csp = conf?.app?.security?.csp;

if (csp === null || csp === undefined) {
  console.error(
    'app.security.csp is null. That is the finding this gate exists for: the webview runs with no ' +
      'content-security policy at all.',
  );
  process.exit(1);
}

// Directives that carry the actual hardening. Losing any of these silently is the regression that
// matters, so they are asserted by name rather than inferred.
const REQUIRED = {
  'default-src': "'self'",
  'object-src': "'none'",
  'base-uri': "'self'",
  'frame-ancestors': "'none'",
};
for (const [directive, expected] of Object.entries(REQUIRED)) {
  const actual = typeof csp === 'string' ? csp : csp[directive];
  if (!actual || !String(actual).includes(expected)) {
    problems.push(`tauri.conf.json: csp.${directive} should contain ${expected} (got ${actual ?? 'nothing'})`);
  }
}
// `script-src` must not be blanket-inline in production; that is what makes an injected string
// executable, and it is the single directive most likely to be loosened "just to ship".
const scriptSrc = String((typeof csp === 'string' ? csp : csp['script-src']) ?? '');
if (scriptSrc.includes("'unsafe-inline'")) {
  problems.push("tauri.conf.json: production csp.script-src must not allow 'unsafe-inline'");
}
if (scriptSrc.includes("'unsafe-eval'")) {
  problems.push("tauri.conf.json: production csp.script-src must not allow 'unsafe-eval'");
}

// ---- the shipped artifacts ------------------------------------------------------------------
if (!existsSync(DIST)) {
  console.error(
    `missing ${relative(ROOT, DIST)} — run \`npm run build\` first; this gate checks build output.`,
  );
  process.exit(2);
}

const files = walk(DIST);
const html = files.filter((f) => extname(f) === '.html');
if (html.length === 0) {
  console.error(`no HTML in ${relative(ROOT, DIST)} — the build output looks wrong`);
  process.exit(2);
}

const EXTERNAL = /(?:src|href)\s*=\s*["'](https?:)?\/\/[^"']+["']/gi;
// An inline <script> with a body. `<script src=...></script>` is fine; `<script>code</script>` is
// not, under a script-src without 'unsafe-inline'.
const INLINE_SCRIPT = /<script(?![^>]*\bsrc\s*=)[^>]*>([\s\S]*?)<\/script>/gi;

for (const file of html) {
  const src = readFileSync(file, 'utf8');

  for (const m of src.matchAll(EXTERNAL)) {
    note(file, `loads an external origin, which default-src 'self' blocks: ${m[0].slice(0, 120)}`);
  }
  for (const m of src.matchAll(INLINE_SCRIPT)) {
    if (m[1].trim().length > 0) {
      note(
        file,
        `has an inline <script> body (${m[1].trim().length} chars), which script-src without ` +
          `'unsafe-inline' blocks`,
      );
    }
  }
}

// ---- report ----------------------------------------------------------------------------------
if (problems.length > 0) {
  console.error('CSP compatibility check FAILED:\n');
  for (const p of problems) console.error(`  - ${p}`);
  console.error(
    `\n${problems.length} problem(s). Either the asset should not ship, or the policy is wrong — ` +
      `do not loosen the policy to make an unused debug page load.`,
  );
  process.exit(1);
}

console.log(
  `csp check: ${html.length} shipped HTML file(s) in ${relative(ROOT, DIST) || 'dist'}, ` +
    `0 external origins, 0 inline script bodies; required directives present`,
);
