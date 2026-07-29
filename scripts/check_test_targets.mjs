// Fails when a Rust integration-test target compiles but runs zero tests.
//
// The failure this exists to catch is silent by construction. `src-tauri/tests/` holds seven files
// carrying a crate-level `#![cfg(feature = "networking")]` or `#![cfg(feature = "ml-wgpu")]`. Under
// a default build every one of them compiles to an empty binary and reports `running 0 tests` — and
// `cargo test` exits 0, because zero passing tests is a pass. That was 1,877 lines of migration,
// cross-shard and GPU-fallback coverage, and the command the docs told you to run never executed a
// line of it.
//
// CI runs `--features desktop`, so it did run them; the gap was between CI and the documented local
// gate, which is the worst place for a gap to be. This turns "a target ran nothing" into an error
// for whatever feature set it is handed, so an accidental gate — or a file emptied by a bad merge —
// is loud instead of green.
//
// Usage:  cargo test --features desktop 2>&1 | tee out.txt  &&  node scripts/check_test_targets.mjs out.txt
// Env:    ALLOW_EMPTY_TEST_TARGETS  comma-separated target names that are legitimately empty.

import { readFileSync } from 'node:fs';
import process from 'node:process';

const file = process.argv[2];
if (!file) {
  console.error('usage: node scripts/check_test_targets.mjs <cargo-test-output.txt>');
  process.exit(2);
}

// `src/main.rs` is a binary with no tests by design — a `main.rs` that only calls `lib::run()` has
// nothing to assert. Doc-test targets legitimately report 0 when a crate has no doc examples.
const DEFAULT_ALLOWED = new Set(['src/main.rs']);
for (const name of (process.env.ALLOW_EMPTY_TEST_TARGETS ?? '').split(',')) {
  const trimmed = name.trim();
  if (trimmed) DEFAULT_ALLOWED.add(trimmed);
}

const text = readFileSync(file, 'utf8');
const lines = text.split(/\r?\n/);

// Cargo prints, per target:
//     Running unittests src/lib.rs (target/debug/deps/anima_engine_lib-<hash>.exe)
//   running 286 tests
// and for doc tests:
//    Doc-tests anima_domain
//
// The `(path/to/binary.exe)` part is deliberately NOT required to match. It used to be, and that
// made this gate skip targets in silence: PowerShell wraps a native command's redirected stderr at
// the console width, so `Running tests\foo.rs (target\...)` arrives as two lines and the `(` is on
// the second one. Measured 2026-07-29 on a local capture: 75 `Running` lines, 61 matched, and the
// script cheerfully reported `test targets: 61, empty: 0` and exited 0 — the exact silent-skip
// failure it exists to prevent, in the gate itself. CI pipes into `tee`, so cargo does not wrap
// there and CI never saw it.
const targets = [];
const unresolved = [];
let pending = null;
let runningLines = 0;

const finishPending = () => {
  if (pending) unresolved.push(pending.name);
  pending = null;
};

for (const line of lines) {
  const running = line.match(/^\s*Running\s+(?:unittests\s+)?(\S+)/);
  if (running) {
    // A second `Running` before the previous one reported a count means that target never printed
    // one. Do not overwrite it into oblivion — that is a target which loaded and died, which is
    // what a missing native DLL looks like (STATUS_ENTRYPOINT_NOT_FOUND, 0xc0000139).
    finishPending();
    runningLines += 1;
    // Normalize the Windows separators cargo emits so the allow-list is written one way.
    pending = { name: running[1].replace(/\\/g, '/'), kind: 'test' };
    continue;
  }
  if (/^\s*Doc-tests\s+/.test(line)) {
    finishPending();
    continue; // doc-test targets are exempt: an absent doc example is not a missing test
  }
  const count = line.match(/^running (\d+) tests?$/);
  if (count && pending) {
    targets.push({ ...pending, count: Number(count[1]) });
    pending = null;
  }
}
finishPending();

if (targets.length === 0) {
  console.error(`No test targets found in ${file}. Did the cargo output get captured?`);
  process.exit(2);
}

// The gate's own self-check. Every `Running` line must have produced a parsed target; if the two
// numbers disagree, this script did not look at everything it claims to have looked at, and
// "empty: 0" below would be a statement about a subset.
if (targets.length !== runningLines) {
  console.error(
    `\nParsed ${targets.length} target(s) from ${runningLines} "Running" line(s) in ${file}.\n` +
      (unresolved.length
        ? `These printed no "running N tests" line:\n${unresolved.map(n => `  - ${n}`).join('\n')}\n`
        : '') +
      '\nA target that starts and never reports has either crashed on load or had its output ' +
      'mangled by the capture. Re-run from PowerShell with `| Tee-Object`, or from CI with ' +
      '`2>&1 | tee`, so cargo does not wrap its status lines.',
  );
  process.exit(1);
}

const empty = targets.filter(t => t.count === 0 && !DEFAULT_ALLOWED.has(t.name));

console.log(`test targets: ${targets.length}, empty: ${empty.length}`);

if (empty.length > 0) {
  console.error(
    `\n${empty.length} test target(s) compiled but ran zero tests:\n` +
      empty.map(t => `  - ${t.name}`).join('\n') +
      '\n\nA target that runs nothing passes. Either the file is gated out of this feature set ' +
      '(check for a crate-level #![cfg(feature = "...")]), or it has no #[test] left in it.\n' +
      'If it is deliberately empty, add it to ALLOW_EMPTY_TEST_TARGETS.',
  );
  process.exit(1);
}
