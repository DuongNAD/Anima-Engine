// Enforces the test-target policy in `scripts/test_target_policy.mjs` against a captured
// `cargo test` run.
//
// Three failures are silent by construction, and this catches all three.
//
//  1. **A target that runs nothing passes.** `src-tauri/tests/` holds eight files carrying a
//     crate-level `#![cfg(feature = "networking")]` or `#![cfg(feature = "ml-wgpu")]`. Before they
//     carried `required-features`, a default build compiled every one of them into an empty binary
//     that reported `running 0 tests` and exited 0 — 2,040 lines of migration, cross-shard and
//     GPU-fallback coverage, and the command the docs told you to run never executed a line of it.
//  2. **An `#[ignore]` passes.** libtest reports an ignored test as neither a pass nor a failure, so
//     a test quietly parked with `#[ignore]` costs nothing and says nothing.
//  3. **A feature-gated target that stops being scheduled passes.** Delete a `required-features`
//     line and eight empty targets come back; mistype the feature name and the targets vanish from
//     *every* configuration without a word.
//
// The policy is an allow-list with a reason per entry, and it is checked in both directions: an item
// outside the list fails, and so does a list entry that no longer applies. That is what keeps it a
// decision record rather than a suppression file.
//
// Usage:
//   cmd /c "cargo test --features desktop --no-fail-fast > out.txt 2>&1"
//   node scripts/check_test_targets.mjs out.txt --profile desktop
//
// `--profile` is `default` or `desktop` (see PROFILES in the policy). Omitted, the feature-gated
// half of the check is skipped and the script says so, because guessing which features produced a
// capture is exactly the kind of assumption this gate exists to remove.
//
// Capture note: redirect at the OS level (`cmd /c "... > f 2>&1"`), not through a PowerShell
// pipeline. PowerShell buffers cargo's stdout and stderr separately, which reorders `Running` lines
// away from the `running N tests` line they belong to and makes this script under-count targets.

import { readFileSync } from 'node:fs';
import process from 'node:process';
import {
  EMPTY_TARGETS,
  FEATURE_GATED_TARGETS,
  IGNORED_TESTS,
  PROFILES,
} from './test_target_policy.mjs';

const args = process.argv.slice(2);
const file = args.find(a => !a.startsWith('--'));
const profileIdx = args.indexOf('--profile');
const profile = profileIdx >= 0 ? args[profileIdx + 1] : null;

if (!file) {
  console.error(
    'usage: node scripts/check_test_targets.mjs <cargo-test-output.txt> [--profile default|desktop]',
  );
  process.exit(2);
}
if (profile !== null && !Object.hasOwn(PROFILES, profile)) {
  console.error(`unknown --profile ${profile}; known: ${Object.keys(PROFILES).join(', ')}`);
  process.exit(2);
}

const failures = [];
const fail = msg => failures.push(msg);

// ---- policy sanity ---------------------------------------------------------------------------
// A reason is the whole point of the list; an entry without one is a suppression wearing a comment.
for (const entry of [...EMPTY_TARGETS, ...IGNORED_TESTS]) {
  if (!entry.reason || entry.reason.trim().length < 40) {
    fail(`policy entry "${entry.name}" has no usable reason. Every exemption must be explained.`);
  }
}

const text = readFileSync(file, 'utf8');
const lines = text.split(/\r?\n/);

// ---- parse the capture -----------------------------------------------------------------------
// Cargo prints, per target:
//     Running unittests src/lib.rs (target/debug/deps/anima_engine_lib-<hash>.exe)
//   running 286 tests
// and for doc tests:
//    Doc-tests anima_domain
<<<<<<< ours
//   running 0 tests
const targets = [];
const ignoredTests = [];
let pending = null;
let currentTarget = null;
let reportedIgnored = 0;
=======
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
>>>>>>> theirs

for (const line of lines) {
  const running = line.match(/^\s*Running\s+(?:unittests\s+)?(\S+)/);
  if (running) {
<<<<<<< ours
    // Normalize the Windows separators cargo emits so the policy is written one way.
    pending = { name: running[1].replace(/\\/g, '/') };
    continue;
  }
  const doc = line.match(/^\s*Doc-tests\s+(\S+)\s*$/);
  if (doc) {
    // Named rather than silently exempt: a doc-test target that runs nothing is still a decision.
    pending = { name: `doc-tests:${doc[1]}` };
    continue;
=======
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
>>>>>>> theirs
  }
  const count = line.match(/^running (\d+) tests?$/);
  if (count && pending) {
    currentTarget = { ...pending, count: Number(count[1]) };
    targets.push(currentTarget);
    pending = null;
    continue;
  }
  const ignored = line.match(/^test (\S+) \.\.\. ignored/);
  if (ignored) {
    ignoredTests.push({ name: ignored[1], target: currentTarget?.name ?? '<unknown target>' });
    continue;
  }
  const summary = line.match(/^test result: \w+\. \d+ passed; \d+ failed; (\d+) ignored/);
  if (summary) reportedIgnored += Number(summary[1]);
}
finishPending();

if (targets.length === 0) {
  console.error(`No test targets found in ${file}. Did the cargo output get captured?`);
  process.exit(2);
}

<<<<<<< ours
// Cross-check the parse against libtest's own arithmetic, so a regex that stops matching cannot
// quietly turn this gate into a no-op.
if (ignoredTests.length !== reportedIgnored) {
  fail(
    `parsed ${ignoredTests.length} "... ignored" line(s) but the run summaries report ` +
      `${reportedIgnored} ignored test(s). The parse is wrong, so nothing below can be trusted.`,
  );
}

// ---- empty targets ---------------------------------------------------------------------------
const allowedEmpty = new Map(EMPTY_TARGETS.map(e => [e.name, e]));
const empty = targets.filter(t => t.count === 0);

for (const t of empty) {
  if (!allowedEmpty.has(t.name)) {
    fail(
      `test target "${t.name}" compiled but ran zero tests, and is not in EMPTY_TARGETS.\n` +
        '    A target that runs nothing exits 0. Either it is gated out of this feature set — in ' +
        'which case it needs `required-features` in Cargo.toml so Cargo does not schedule it at ' +
        'all — or it has no #[test] left in it. If it is empty by design, add it to ' +
        'scripts/test_target_policy.mjs with a reason.',
    );
  }
}
for (const entry of allowedEmpty.values()) {
  const seen = targets.find(t => t.name === entry.name);
  if (!seen) {
    fail(
      `EMPTY_TARGETS lists "${entry.name}", which this run did not produce at all. ` +
        'Remove the stale entry, or find out why the target disappeared.',
    );
  } else if (seen.count > 0) {
    fail(
      `EMPTY_TARGETS lists "${entry.name}" as intentionally empty, but it ran ${seen.count} ` +
        'test(s). Good news — delete the entry so the exemption stops covering real coverage.',
    );
  }
}

// ---- ignored tests ---------------------------------------------------------------------------
const allowedIgnored = new Map(IGNORED_TESTS.map(e => [e.name, e]));
for (const t of ignoredTests) {
  const entry = allowedIgnored.get(t.name);
  if (!entry) {
    fail(
      `test "${t.name}" (${t.target}) is #[ignore]d and is not in IGNORED_TESTS.\n` +
        '    An ignored test is neither a pass nor a failure — it is coverage that stopped ' +
        'running without anybody deciding so. Either make it run, or record in ' +
        'scripts/test_target_policy.mjs why it is an operational entry point rather than a test.',
    );
  } else if (entry.target && t.target !== '<unknown target>' && entry.target !== t.target) {
    fail(
      `IGNORED_TESTS says "${t.name}" lives in ${entry.target}, but it was reported by ` +
        `${t.target}. Update the policy so the record matches the tree.`,
    );
  }
}
for (const entry of allowedIgnored.values()) {
  if (!ignoredTests.some(t => t.name === entry.name)) {
    fail(
      `IGNORED_TESTS lists "${entry.name}", which this run did not report as ignored. ` +
        'Either it is no longer ignored — delete the entry — or the target holding it was not ' +
        'run, which makes this capture the wrong thing to check the policy against.',
    );
  }
}

// ---- feature-gated targets -------------------------------------------------------------------
if (profile === null) {
  console.log(
    'note: no --profile given, so feature-gated target scheduling was NOT checked. ' +
      'Pass --profile default or --profile desktop.',
  );
} else {
  const enabled = new Set(PROFILES[profile]);
  for (const gated of FEATURE_GATED_TARGETS) {
    const seen = targets.find(t => t.name === gated.name);
    if (enabled.has(gated.feature)) {
      if (!seen) {
        fail(
          `profile "${profile}" enables "${gated.feature}", so ${gated.name} must run — but it ` +
            'is absent from this capture. Check its `required-features` spelling in Cargo.toml: ' +
            'a feature name Cargo does not recognise makes a target vanish silently.',
        );
      } else if (seen.count === 0) {
        fail(
          `${gated.name} ran zero tests under profile "${profile}", which enables its required ` +
            `feature "${gated.feature}". The crate-level #![cfg] and the feature set disagree.`,
        );
      }
    } else if (seen) {
      fail(
        `${gated.name} was scheduled under profile "${profile}", which does not enable its ` +
          `required feature "${gated.feature}". It can only be an empty binary that passes — ` +
          'its `required-features` entry in Cargo.toml is missing or wrong.',
      );
    }
  }
}
=======
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
>>>>>>> theirs

// ---- report -----------------------------------------------------------------------------------
const gatedRunning = profile
  ? FEATURE_GATED_TARGETS.filter(g => PROFILES[profile].includes(g.feature)).length
  : 0;
console.log(
  `test targets: ${targets.length}, empty: ${empty.length} (allow-listed ${EMPTY_TARGETS.length}), ` +
    `ignored: ${ignoredTests.length} (allow-listed ${IGNORED_TESTS.length})` +
    (profile ? `, feature-gated running under "${profile}": ${gatedRunning}` : ''),
);

if (failures.length > 0) {
  console.error(`\n${failures.length} test-target policy violation(s):\n`);
  for (const f of failures) console.error(`  - ${f}\n`);
  console.error(
    'The policy lives in scripts/test_target_policy.mjs. It is an allow-list with a reason per ' +
      'entry, checked in both directions: an unlisted item fails, and so does a listed item that ' +
      'no longer applies.',
  );
  process.exit(1);
}
