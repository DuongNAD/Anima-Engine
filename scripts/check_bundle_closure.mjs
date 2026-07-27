#!/usr/bin/env node
// Prove the committed npm distribution boundary still matches what the bundler produces.
//
// # What this gate is for
//
// `licensing/bundle-closure.json` decides which npm components the licence artifacts treat as
// distributed. It is written by a plugin in `vite.config.ts` during `npm run build`, from the real
// module graph — nothing else in the repository knows the answer. That makes it the one file whose
// staleness is invisible: `check:notice`, `check:sbom` and `check:licenses` would all keep passing
// against a boundary that stopped being true, because each only compares itself to itself.
//
// So the check is "does a fresh build still write the committed bytes". Run it **after**
// `npm run build`, which is what CI does. A dependency added, removed, or newly reachable from an
// entry point moves this file, and moving it is correct — committing the move is the point.
//
// Usage: node scripts/check_bundle_closure.mjs
//   Requires `npm run build` to have run first in this working tree.

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = process.cwd();
const REL = 'licensing/bundle-closure.json';
const PATH = join(ROOT, REL);

if (!existsSync(PATH)) {
  console.error(
    `${REL} does not exist. It is written by \`npm run build\`; without it the npm distribution ` +
      `boundary is unknown and the licence artifacts cannot be generated.`,
  );
  process.exit(1);
}

const closure = JSON.parse(readFileSync(PATH, 'utf8'));
const problems = [];

if (!Array.isArray(closure.packages) || closure.packages.length === 0) {
  problems.push('packages is empty; a build that bundles nothing is not a build of this app');
}

// Unattributed shipped code is the failure this whole package exists to make impossible. The plugin
// records rather than drops it, so that it can fail here rather than vanish.
for (const id of closure.unresolved ?? []) {
  problems.push(`shipped module could not be resolved to a component: ${id}`);
}

// Sorted, and keyed by name AND version. `scheduler` really is bundled at two versions (0.21.0 via
// react-reconciler, 0.23.2 via react-dom), so a name-only key would silently drop one of them.
//
// The order is compared field by field rather than by joining name and version into one string. A
// joined key needs a separator that sorts below every legal character, which in practice means a
// literal NUL typed into the source — and that is exactly the corruption
// `scripts/check_text_hygiene.mjs` exists to reject, in the file that argues for rejecting it.
const seen = new Set();
let previous = null;
for (const p of closure.packages) {
  const key = `${p.name}@${p.version}`;
  if (seen.has(key)) problems.push(`duplicate entry ${key}`);
  seen.add(key);
  if (previous !== null) {
    const ordered =
      previous.name < p.name || (previous.name === p.name && previous.version <= p.version);
    if (!ordered) problems.push(`${key} is out of byte order; the file would not be reproducible`);
  }
  previous = p;
  if (p.origin !== 'node_modules' && p.origin !== 'injected') {
    problems.push(`${key} has origin ${String(p.origin)}, expected node_modules or injected`);
  }
}

if (problems.length > 0) {
  console.error(`${REL} is not usable as a distribution boundary:`);
  for (const p of problems) console.error(`  x ${p}`);
  process.exit(1);
}

// An untracked file is not "unchanged" — it is absent from every comparison. `git diff` says
// nothing about a path git does not know, so without this the gate reports a match for a boundary
// that was never committed and would not survive a fresh clone. Checked first, because it is the
// failure mode that looks most like success.
try {
  execFileSync('git', ['ls-files', '--error-unmatch', '--', REL], { cwd: ROOT, stdio: 'pipe' });
} catch {
  console.error(
    `${REL} is not tracked by git. It is the committed record of what the frontend distributes; ` +
      `untracked, it would vanish on a fresh clone and every licence gate would pass against ` +
      `nothing. Run \`git add ${REL}\`.`,
  );
  process.exit(1);
}

// `git diff HEAD` rather than a plain `git diff`: the question is whether the file a build just
// wrote differs from the committed one, and a staged-but-uncommitted change is still a change.
try {
  execFileSync('git', ['diff', 'HEAD', '--exit-code', '--', REL], { cwd: ROOT, stdio: 'pipe' });
} catch {
  console.error(
    `${REL} differs from the committed copy after a build. The set of npm packages with bytes in\n` +
      `dist/ has changed, which changes what must be attributed.\n\n` +
      `Regenerate the licence artifacts and commit all of them together:\n` +
      `  npm run build && npm run gen:compliance\n`,
  );
  execFileSync('git', ['--no-pager', 'diff', 'HEAD', '--', REL], { cwd: ROOT, stdio: 'inherit' });
  process.exit(1);
}

const injected = closure.packages.filter((p) => p.origin === 'injected').length;
console.log(
  `bundle closure check: ${closure.packages.length} package(s) with bytes in dist/ ` +
    `(${injected} toolchain-injected), matching the committed boundary`,
);
