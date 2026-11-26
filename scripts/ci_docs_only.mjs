// Docs-only fast path for `Rust (src-tauri)` (ci.yml) and `determinism` (hardening.yml) (QD-FOCUS FOCUS-5, owner 2026-11-26).
//
// The Rust CI job costs one to two hours on a Windows runner and hardening's `determinism` job takes
// ~35 minutes on Ubuntu. Until 2026-11-26, a pull request that only edited documentation paid those
// costs too, and most of the pull requests of the night of 2026-11-25/26 were documentation. This
// script decides whether a pull request's diff is "docs-only". Both the `scope` job in
// .github/workflows/ci.yml and the `scope` job in .github/workflows/hardening.yml run it, sharing one
// definition and one drift guard; `Rust (src-tauri)` and `determinism` are skipped when the answer
// is yes. GitHub reports a job skipped by its `if:` as a success, so required checks keep their
// names and branch protection is unchanged.
//
// The `Frontend` job always runs in full, docs-only or not. It holds every docs gate: link check,
// metadata, IPC reference, doc commands, text hygiene and conflict markers.
//
// What counts as docs-only. Every changed path must pass, and with --no-renames both sides of a
// rename are counted. A path passes when it is:
//   - a markdown file under docs/, except the paths in READ_BY_RUST; or
//   - the root files TODO.md or FINDINGS.md.
// Anything else forces the full run. That covers non-markdown files under docs/ (.txt, .csv, .json),
// licensing/*.md (read by the licence check), README.md and CLAUDE.md. An empty diff, a push or a
// workflow_dispatch also run in full.
//
// How READ_BY_RUST was measured (2026-11-26, trunk 5e6927bb):
//   - Every string literal in src-tauri/**/*.rs that names a docs/ path was listed. Exactly one is
//     read: tests/prereg_e3_manifest_tests.rs joins
//     docs/ai/planning/2026-08-26-experiment-e3-productivity-gradient-body-mass.md.
//   - The other hits are #[ignore = "..."] reasons and a message string; neither reads a file.
//   - The node and python steps of the Rust job do not read docs/, TODO.md or FINDINGS.md. Those
//     steps are check_test_targets, the licence, SBOM, NOTICE and flora-footprint checks, and
//     verify_dataflow.py.
// The drift guard below repeats that scan on every run. A new "docs/...md" literal under src-tauri/
// that is missing from READ_BY_RUST fails the job, and so does a listed path that no literal names any
// more. The guard cannot see a path built piece by piece (join("docs").join(...)); review has to.
//
// The gate measures itself first. --self-test, also run at the start of every real run, puts fixed
// path lists through the classifier: code, mixed, empty, a READ_BY_RUST file, non-markdown docs,
// licensing markdown and README.md must all come out "full run"; two pure-docs lists must come out
// docs-only. If any control answers wrong, the script exits 2.
//
// Run:  node scripts/ci_docs_only.mjs --self-test
//       node scripts/ci_docs_only.mjs --event pull_request --base <sha> --head <sha>
//       (writes docs_only=true|false to $GITHUB_OUTPUT when that variable is set)

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import nodePath from 'node:path';
import { fileURLToPath } from 'node:url';

export const READ_BY_RUST = Object.freeze([
  'docs/ai/planning/2026-08-26-experiment-e3-productivity-gradient-body-mass.md',
]);

const ROOT_DOCS = new Set(['TODO.md', 'FINDINGS.md']);

export function isDocsPath(path) {
  if (READ_BY_RUST.includes(path)) return false;
  if (ROOT_DOCS.has(path)) return true;
  return path.startsWith('docs/') && path.endsWith('.md');
}

export function classify(paths) {
  const offending = paths.filter((p) => !isDocsPath(p));
  return { docsOnly: paths.length > 0 && offending.length === 0, offending };
}

const CONTROLS = [
  { name: 'rust source', paths: ['src-tauri/src/lib.rs'], want: false },
  { name: 'docs plus code', paths: ['docs/how-to/AGENT_WORKFLOW.md', 'src/App.tsx'], want: false },
  { name: 'empty diff', paths: [], want: false },
  { name: 'doc read by a Rust test', paths: [READ_BY_RUST[0]], want: false },
  { name: 'non-markdown under docs', paths: ['docs/paper/data/incidents.csv'], want: false },
  { name: 'licensing markdown', paths: ['licensing/UNRESOLVED.md'], want: false },
  { name: 'root README', paths: ['README.md'], want: false },
  { name: 'workflow file', paths: ['.github/workflows/ci.yml'], want: false },
  { name: 'docs-ish but not docs/', paths: ['documents/x.md'], want: false },
  { name: 'pure docs', paths: ['docs/decisions/ADR-0015-evolving-plants.md'], want: true },
  { name: 'state sync', paths: ['docs/planning/STATE_OF_THE_PROJECT.md', 'TODO.md'], want: true },
];

export function selfTest() {
  const bad = CONTROLS.filter((c) => classify(c.paths).docsOnly !== c.want);
  for (const c of bad) console.error(`self-test: control "${c.name}" answered ${!c.want}, expected ${c.want}`);
  return bad.length === 0;
}

// Drift guard: compare READ_BY_RUST with the "docs/...md" string literals under src-tauri/.
const LITERAL = /"(?:\.\.\/)*(docs\/[^"\s]*\.md)"/g;

function listRustFiles(dir, out = []) {
  for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
    if (ent.name === 'target' || ent.name.startsWith('target_') || ent.name === 'gen') continue;
    const full = nodePath.join(dir, ent.name);
    if (ent.isDirectory()) listRustFiles(full, out);
    else if (ent.name.endsWith('.rs')) out.push(full);
  }
  return out;
}

export function scanRustDocLiterals(sourceText) {
  const found = new Set();
  for (const line of sourceText.split('\n')) {
    const trimmed = line.trim();
    if (trimmed.startsWith('//')) continue;
    for (const m of line.matchAll(LITERAL)) found.add(m[1]);
  }
  return found;
}

function driftGuard(repoRoot) {
  const found = new Set();
  for (const file of listRustFiles(nodePath.join(repoRoot, 'src-tauri'))) {
    for (const p of scanRustDocLiterals(fs.readFileSync(file, 'utf8'))) found.add(p);
  }
  const missing = [...found].filter((p) => !READ_BY_RUST.includes(p));
  const stale = READ_BY_RUST.filter((p) => !found.has(p));
  for (const p of missing) console.error(`drift: src-tauri names ${p}, which is not in READ_BY_RUST`);
  for (const p of stale) console.error(`drift: READ_BY_RUST lists ${p}, which no src-tauri literal names`);
  return missing.length === 0 && stale.length === 0;
}

function scannerSelfTest() {
  const sample = [
    '        .join("docs/ai/planning/x.md");',
    '// let p = "docs/commented.md";',
    '#[ignore = "MAP-D3: see docs/ai/planning/y.md"]',
    '    panic!("see docs/how-to/BENCHMARKING.md.");',
    '    let q = "../docs/z.md";',
  ].join('\n');
  const got = [...scanRustDocLiterals(sample)].sort().join(',');
  const want = 'docs/ai/planning/x.md,docs/z.md';
  if (got !== want) console.error(`self-test: scanner found [${got}], expected [${want}]`);
  return got === want;
}

function changedPaths(base, head) {
  const out = execFileSync('git', ['diff', '--name-only', '--no-renames', `${base}...${head}`], {
    encoding: 'utf8',
  });
  return out.split('\n').map((s) => s.trim()).filter(Boolean);
}

function writeOutput(docsOnly) {
  const file = process.env.GITHUB_OUTPUT;
  if (file) fs.appendFileSync(file, `docs_only=${docsOnly}\n`);
}

function main(argv) {
  const arg = (name) => {
    const i = argv.indexOf(name);
    return i >= 0 ? argv[i + 1] : undefined;
  };
  if (!selfTest() || !scannerSelfTest()) process.exit(2);
  if (argv.includes('--self-test')) {
    console.log(`ci_docs_only self-test: ${CONTROLS.length} classifier controls and the scanner control ok`);
    return;
  }
  const repoRoot = process.cwd();
  if (!driftGuard(repoRoot)) process.exit(1);

  const event = arg('--event') ?? '';
  const base = arg('--base') ?? '';
  const head = arg('--head') ?? '';
  const sha = /^[0-9a-f]{7,40}$/;
  if (event !== 'pull_request' || !sha.test(base) || !sha.test(head)) {
    console.log(`docs_only=false (event "${event}": only a pull request with both SHAs can take the fast path)`);
    writeOutput(false);
    return;
  }
  const paths = changedPaths(base, head);
  const { docsOnly, offending } = classify(paths);
  console.log(`changed paths: ${paths.length}`);
  if (!docsOnly) {
    const shown = offending.slice(0, 10).join(', ');
    console.log(`docs_only=false (${paths.length === 0 ? 'empty diff' : `full run needed for: ${shown}`})`);
  } else {
    console.log('docs_only=true (every changed path is documentation the Rust job does not read)');
  }
  writeOutput(docsOnly);
}

// fileURLToPath, not URL.pathname: on Windows the pathname is "/D:/...", which never equals argv[1], so the script
// silently did nothing there (anima-case's review of #315, verdict CI-FASTPATH_e1161c85).
const invokedDirectly =
  process.argv[1] && nodePath.resolve(process.argv[1]) === nodePath.resolve(fileURLToPath(import.meta.url));
if (invokedDirectly) main(process.argv.slice(2));
