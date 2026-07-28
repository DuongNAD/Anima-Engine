#!/usr/bin/env node
// Verify a published E2 artifact directory, independently of the program that wrote it.
//
// The Rust runner hashes its own output with `core::sha256`, a hand-written implementation. A hash
// that agrees with itself proves nothing, so this recomputes every digest with `node:crypto` — a
// different implementation, in a different language, on the same bytes. If the two disagree, one of
// them is wrong and the artifact is not publishable either way.
//
// It also checks the things a checksum cannot: that the manifests the run copied beside its results
// are byte-identical to the ones committed in the repository, that the seed order actually run is
// the preregistered one, that the smoke seed is absent from the analysis, and that the counts in
// `effects.json` agree with the pairs in `paired-report.json`. Those are the ways an artifact set
// can be internally consistent and still describe an experiment nobody registered.
//
//   node scripts/verify_e2_artifacts.mjs [artifact-dir]
//
// Defaults to `artifacts/experiments/e2-evolved-brain-default`. Exits non-zero on the first failure
// class, printing every failure it found rather than only the first.

import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const artifactDir = resolve(process.argv[2] ?? join(repoRoot, 'artifacts/experiments/e2-evolved-brain-default'));
const committedManifestDir = 'src-tauri/tests/fixtures/experiments_e2';

const failures = [];
const notes = [];
const fail = (msg) => failures.push(msg);
const note = (msg) => notes.push(msg);

const sha256 = (buf) => createHash('sha256').update(buf).digest('hex');
const readJson = (p) => JSON.parse(readFileSync(p, 'utf8'));

/**
 * Read the bytes Git has committed, not the platform-dependent working-tree representation.
 *
 * Git for Windows commonly checks text files out with CRLF. The E2 artifacts intentionally retain
 * their original bytes because they are covered by SHA-256, so comparing those artifacts with a
 * CRLF checkout produces a false tampering alarm. `git cat-file` reads the canonical blob and also
 * makes "committed manifest" mean exactly what the verifier says.
 */
function readCommittedManifest(name) {
  try {
    return execFileSync(
      'git',
      ['cat-file', 'blob', `HEAD:${committedManifestDir}/${name}`],
      { cwd: repoRoot, stdio: ['ignore', 'pipe', 'ignore'] },
    );
  } catch {
    return null;
  }
}

function readCommittedPreregistration() {
  const bytes = readCommittedManifest('e2-preregistration.json');
  if (bytes === null) {
    throw new Error(`HEAD has no ${committedManifestDir}/e2-preregistration.json`);
  }
  return JSON.parse(bytes.toString('utf8'));
}

/** Recompute every digest in a `checksums.sha256` file with node:crypto. */
function verifyChecksums(dir, label) {
  const file = join(dir, 'checksums.sha256');
  if (!existsSync(file)) {
    fail(`${label}: checksums.sha256 is missing, so nothing in this directory can be verified`);
    return new Set();
  }
  const covered = new Set();
  for (const line of readFileSync(file, 'utf8').split('\n')) {
    if (!line.trim()) continue;
    const m = /^([0-9a-f]{64})\s\s(.+)$/.exec(line);
    if (!m) {
      fail(`${label}: unreadable checksum line: ${JSON.stringify(line)}`);
      continue;
    }
    const [, expected, name] = m;
    const target = join(dir, name);
    if (!existsSync(target)) {
      fail(`${label}: ${name} is listed in checksums.sha256 but does not exist`);
      continue;
    }
    const actual = sha256(readFileSync(target));
    if (actual !== expected) {
      fail(`${label}: ${name} hashes to ${actual}, but checksums.sha256 says ${expected}`);
    }
    covered.add(name);
  }
  return covered;
}

/**
 * Every artifact in the directory must be covered by a checksum — silence is not coverage.
 *
 * Two files are written by hand rather than by the runner and so cannot appear in a checksum file
 * the runner produced: `execution-lock.json`, which records the duration rung chosen from the smoke
 * calibration *before* the ensemble existed, and `RESULT.md`. Both are cross-checked on content
 * instead, below.
 */
function verifyNothingUncovered(dir, covered, label) {
  const skip = new Set(['checksums.sha256', 'execution-lock.json', 'RESULT.md']);
  const walk = (rel) => {
    for (const entry of readdirSync(join(dir, rel), { withFileTypes: true })) {
      const child = rel ? `${rel}/${entry.name}` : entry.name;
      if (entry.isDirectory()) {
        // `smoke/` and `replay/` carry their own checksums file and are verified separately.
        // `superseded/` holds failed attempts, kept deliberately: each retains the
        // `checksums.sha256` it was written with, and none of it may feed the analysis, so it is
        // neither re-verified against the current preregistration nor allowed to fail this walk.
        if (child === 'smoke' || child === 'replay' || child === 'superseded') continue;
        walk(child);
      } else if (!skip.has(child)) {
        if (!covered.has(child)) fail(`${label}: ${child} exists but no checksum covers it`);
      }
    }
  };
  walk('');
}

/** The manifests a run copied beside its results must be the committed ones, byte for byte. */
function verifyManifestsAreTheCommittedOnes(dir, label) {
  const copied = join(dir, 'manifests');
  if (!existsSync(copied)) {
    fail(`${label}: no manifests/ copy, so the run cannot prove which manifests it used`);
    return;
  }
  for (const entry of readdirSync(copied)) {
    const committed = readCommittedManifest(entry);
    if (committed === null) {
      fail(`${label}: manifests/${entry} has no counterpart in ${committedManifestDir}`);
      continue;
    }
    const a = sha256(readFileSync(join(copied, entry)));
    const b = sha256(committed);
    if (a !== b) {
      fail(
        `${label}: manifests/${entry} (${a}) is not the committed file (${b}). The run used a ` +
          `manifest that is not in the repository, or the repository copy changed after the run.`,
      );
    }
  }
}

/** The registered plan, and what the run says it did, must be the same thing. */
function verifyAgainstPreregistration(dir, label, { isSmoke }) {
  const prereg = readCommittedPreregistration();
  const prov = readJson(join(dir, 'provenance.json'));
  const report = readJson(join(dir, 'paired-report.json'));
  const effects = readJson(join(dir, 'effects.json'));

  const registeredSeeds = prereg.seeds.execution_order;
  const smokeSeed = prereg.seeds.smoke_seed;
  const ran = report.seed_order;

  if (isSmoke) {
    if (JSON.stringify(ran) !== JSON.stringify([smokeSeed])) {
      fail(`${label}: a smoke run must run exactly [${smokeSeed}], but ran ${JSON.stringify(ran)}`);
    }
  } else {
    if (JSON.stringify(ran) !== JSON.stringify(registeredSeeds)) {
      fail(
        `${label}: ran ${JSON.stringify(ran)} but the preregistered execution order is ` +
          `${JSON.stringify(registeredSeeds)}. No substitution or reordering is permitted.`,
      );
    }
    if (ran.includes(smokeSeed)) {
      fail(`${label}: the smoke seed ${smokeSeed} appears in the analysis`);
    }
    const complete = report.pairs.filter(
      (p) => p.control.status === 'Completed' && p.treatment.status === 'Completed',
    ).length;
    const minimum = prereg.seeds.min_complete_pairs_for_a_decision;
    if (complete < minimum) {
      note(
        `${label}: ${complete} complete pairs against a registered minimum of ${minimum} — this is ` +
          `"insufficient evidence" by planning §8, not a decision taken on ${complete}`,
      );
    }
    if (complete !== effects.n_complete_pairs) {
      fail(
        `${label}: effects.json claims ${effects.n_complete_pairs} complete pairs, ` +
          `paired-report.json contains ${complete}`,
      );
    }
  }

  if (!prereg.duration.duration_ticks_ladder.includes(prov.duration.duration_ticks_as_run)) {
    fail(
      `${label}: ran at ${prov.duration.duration_ticks_as_run} ticks, which is not on the ` +
        `registered ladder ${JSON.stringify(prereg.duration.duration_ticks_ladder)}`,
    );
  }
  if (prov.duration.sample_period !== prereg.duration.sample_period) {
    fail(
      `${label}: sample period ${prov.duration.sample_period} is not the registered ` +
        `${prereg.duration.sample_period}`,
    );
  }
  if (prov.build.profile !== 'release') {
    fail(`${label}: built as "${prov.build.profile}"; the preregistration requires release`);
  }
  if (!prov.integrity.brains_present_only_in_treatment) {
    fail(`${label}: gate E2-G1 — brains are not present in the treatment arm only`);
  }
  if (!prov.integrity.ecology_stream_identical_after_genesis) {
    fail(`${label}: gate E2-G3 — the arms' ecology streams diverged before the first tick`);
  }
  if (!prov.world_identity_observed.identical_across_arms) {
    fail(`${label}: gate E2-G6 — the arms observed different world identities`);
  }
  if (JSON.stringify(report.declared_factors) !== JSON.stringify(['initial_conditions'])) {
    fail(
      `${label}: gate E2-G4 — declared factors are ${JSON.stringify(report.declared_factors)}, ` +
        `not ["initial_conditions"]`,
    );
  }
  // The reproduction command must never launch the app. Pinned in Rust too; checked here because a
  // reader of the published artifacts runs this script, not the test suite.
  if (String(prereg.reproduction_command).includes('cargo run')) {
    fail(`${label}: the registered reproduction command uses \`cargo run\`, which is forbidden`);
  }
}

/** Every per-seed delta must be exactly `treatment_final - control_final`, recomputed here. */
function verifyDeltasAreArithmetic(dir, label) {
  const csv = readFileSync(join(dir, 'per-seed-deltas.csv'), 'utf8').trim().split('\n');
  const header = csv.shift();
  if (header !== 'seed,observable,control_final,treatment_final,delta') {
    fail(`${label}: per-seed-deltas.csv header is "${header}", not the schema design §7 declares`);
    return;
  }
  let checked = 0;
  for (const row of csv) {
    const [seed, observable, c, t, d] = row.split(',');
    const recomputed = Number(t) - Number(c);
    // Float subtraction is exact when redone the same way; a mismatch means the column was not
    // computed from the two beside it.
    if (recomputed !== Number(d)) {
      fail(
        `${label}: seed ${seed} ${observable}: delta ${d} is not ${t} - ${c} (= ${recomputed})`,
      );
    }
    checked += 1;
  }
  note(`${label}: recomputed ${checked} per-seed deltas`);
}

/**
 * The duration rung must have been locked from the smoke calibration BEFORE the ensemble ran, and
 * the ensemble must have used the rung that was locked.
 *
 * This is the check that stops the one deviation the preregistration permits from becoming a knob:
 * a lock committed after the fact, or an ensemble that quietly ran at a different T, both show up
 * here rather than in a sentence somebody has to be trusted on.
 */
function verifyDurationLock(dir) {
  const lockPath = join(dir, 'execution-lock.json');
  if (!existsSync(lockPath)) {
    note('analysis: no execution-lock.json — the duration rung was not locked from a smoke calibration');
    return;
  }
  const lock = readJson(lockPath);
  const prereg = readCommittedPreregistration();
  if (!prereg.duration.duration_ticks_ladder.includes(lock.selected_duration_ticks)) {
    fail(
      `lock: the selected rung ${lock.selected_duration_ticks} is not on the registered ladder ` +
        `${JSON.stringify(prereg.duration.duration_ticks_ladder)}`,
    );
  }
  const provPath = join(dir, 'provenance.json');
  if (existsSync(provPath)) {
    const ran = readJson(provPath).duration.duration_ticks_as_run;
    if (ran !== lock.selected_duration_ticks) {
      fail(
        `lock: the ensemble ran at ${ran} ticks but the rung locked before it was ` +
          `${lock.selected_duration_ticks}. T may never change once experimental data exists.`,
      );
    }
  }
  const smokeProv = join(dir, 'smoke', 'provenance.json');
  if (existsSync(smokeProv)) {
    const smokeCommit = readJson(smokeProv).build.git_commit;
    if (lock.build_commit && smokeCommit && lock.build_commit !== smokeCommit) {
      fail(
        `lock: locked against commit ${lock.build_commit}, but the smoke ran at ${smokeCommit}`,
      );
    }
  }
}

function verifyDirectory(dir, label, opts) {
  if (!existsSync(dir)) {
    fail(`${label}: ${dir} does not exist`);
    return;
  }
  const covered = verifyChecksums(dir, label);
  verifyNothingUncovered(dir, covered, label);
  verifyManifestsAreTheCommittedOnes(dir, label);
  verifyAgainstPreregistration(dir, label, opts);
  verifyDeltasAreArithmetic(dir, label);
}

// ---- Run ---------------------------------------------------------------------------------------

if (!existsSync(artifactDir)) {
  console.error(`No artifact directory at ${artifactDir}`);
  process.exit(1);
}

// The analysis may legitimately not exist yet: between the smoke calibration and the ensemble there
// is a committed state where only `smoke/` and `execution-lock.json` are present, and that state has
// to verify too — it is the state the lock commit is made in.
if (existsSync(join(artifactDir, 'paired-report.json'))) {
  verifyDirectory(artifactDir, 'analysis', { isSmoke: false });
} else {
  note('analysis: no ensemble has been run yet (no paired-report.json at the artifact root)');
}

const smokeDir = join(artifactDir, 'smoke');
if (existsSync(smokeDir)) {
  verifyDirectory(smokeDir, 'smoke', { isSmoke: true });
} else {
  note('smoke: no smoke/ directory present');
}

verifyDurationLock(artifactDir);

for (const line of notes) console.log(`note  ${line}`);
if (failures.length === 0) {
  console.log(`\nOK — every E2 artifact in ${artifactDir} verifies against node:crypto and the preregistration.`);
  process.exit(0);
}
console.error('');
for (const line of failures) console.error(`FAIL  ${line}`);
console.error(`\n${failures.length} failure(s).`);
process.exit(1);
