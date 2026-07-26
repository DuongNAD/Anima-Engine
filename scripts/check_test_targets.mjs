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
const targets = [];
let pending = null;

for (const line of lines) {
  const running = line.match(/^\s*Running\s+(?:unittests\s+)?(\S+)\s+\(/);
  if (running) {
    // Normalize the Windows separators cargo emits so the allow-list is written one way.
    pending = { name: running[1].replace(/\\/g, '/'), kind: 'test' };
    continue;
  }
  if (/^\s*Doc-tests\s+/.test(line)) {
    pending = null; // doc-test targets are exempt: an absent doc example is not a missing test
    continue;
  }
  const count = line.match(/^running (\d+) tests?$/);
  if (count && pending) {
    targets.push({ ...pending, count: Number(count[1]) });
    pending = null;
  }
}

if (targets.length === 0) {
  console.error(`No test targets found in ${file}. Did the cargo output get captured?`);
  process.exit(2);
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
