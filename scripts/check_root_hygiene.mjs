// Refuse a repository-root entry that nobody reviewed.
//
// # The problem this exists for
//
// The root is where an agent drops whatever it produced last. 42 files of an unrelated Python
// exercise (with .pyc) once landed there, and so did agent task prompts, screenshots and a
// "captured evidence" PNG generator (card G6). Each was committed without anyone deciding the root
// was the place for it, and each stayed because nothing failed.
//
// # The rule
//
// Every entry git has at the root, file or directory, must be listed in scripts/root_allowlist.json
// with a one-line reason. The check runs in both directions, like scripts/test_target_policy.mjs:
//
//   - an entry at the root that is not on the list fails, so a new root file needs a reviewed line;
//   - a listed entry that no longer exists fails, so the list cannot rot into a list of excuses.
//
// It reads the index (`git ls-files --cached`), so a file that is only staged already fails and the
// commit that introduces it can be stopped. Untracked files are left alone on purpose: a working
// tree has scratch output and worktree symlinks that are nobody's business until they are added.
//
// Before checking the repository it runs its own negative controls and exits 2 if one of them
// passes, the same convention as scripts/check_docs_metadata.mjs.
//
// Usage: node scripts/check_root_hygiene.mjs

import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = process.cwd();
const ALLOWLIST_PATH = 'scripts/root_allowlist.json';

/** The first path component of every path git has in its index. */
export function rootEntries(paths) {
  const entries = new Set();
  for (const p of paths) {
    if (p) entries.add(p.split('/')[0]);
  }
  return entries;
}

/**
 * Compare the root with the allow-list. Returns every problem found; an empty array means the root
 * is exactly the reviewed set.
 */
export function compare(entries, allowlist) {
  const problems = [];
  for (const [name, reason] of Object.entries(allowlist)) {
    if (typeof reason !== 'string' || reason.trim() === '') {
      problems.push(`${name}: listed without a reason`);
    }
    if (!entries.has(name)) {
      problems.push(`${name}: listed in ${ALLOWLIST_PATH} but not in the repository; remove its line`);
    }
  }
  for (const name of [...entries].sort()) {
    if (!Object.prototype.hasOwnProperty.call(allowlist, name)) {
      problems.push(`${name}: at the repository root but not in ${ALLOWLIST_PATH}`);
    }
  }
  return problems;
}

function selfTest() {
  const allow = { 'README.md': 'readme', src: 'source' };
  const controls = [
    ['an unlisted root file', rootEntries(['README.md', 'src/a.ts', 'scratch.md'])],
    ['a listed entry that is gone', rootEntries(['README.md'])],
    ['an entry without a reason', rootEntries(['README.md', 'src/a.ts']), { ...allow, src: ' ' }],
  ];
  let failed = 0;
  for (const [label, entries, list = allow] of controls) {
    if (compare(entries, list).length === 0) {
      console.error(`root hygiene self-test: the check accepted ${label}`);
      failed++;
    }
  }
  if (compare(rootEntries(['README.md', 'src/a.ts', 'src/b/c.ts']), allow).length !== 0) {
    console.error('root hygiene self-test: the check rejected a root that matches its list');
    failed++;
  }
  return failed;
}

function main() {
  if (selfTest() > 0) {
    process.exit(2);
  }

  const allowlist = JSON.parse(readFileSync(join(ROOT, ALLOWLIST_PATH), 'utf8')).entries;
  const paths = execFileSync('git', ['ls-files', '-z', '--cached'], {
    cwd: ROOT,
    encoding: 'utf8',
    maxBuffer: 1 << 28,
  }).split('\0');
  const entries = rootEntries(paths);
  const problems = compare(entries, allowlist);

  if (problems.length > 0) {
    console.error(`root hygiene: ${problems.length} problem(s)`);
    for (const p of problems) console.error(`  - ${p}`);
    console.error(
      `A root entry is a reviewed decision: add it to ${ALLOWLIST_PATH} with a one-line reason, ` +
        'or move the file somewhere it belongs.',
    );
    process.exit(1);
  }
  console.log(`root hygiene: ${entries.size} root entries, all listed in ${ALLOWLIST_PATH}; self-test 4 controls ok`);
}

main();
