#!/usr/bin/env node
// Verify a published E-SEL1 artifact directory, independently of the program that wrote it.
//
// Modelled on scripts/verify_e2_artifacts.mjs. The Rust runner (run_esel1_experiment) hashes its own
// output with `core::sha256`, a hand-written implementation. A hash that agrees with itself proves
// nothing, so this recomputes every digest with `node:crypto`. It also checks what a checksum
// cannot: that the manifests copied beside the results are byte-identical to the ones committed in
// src-tauri/tests/fixtures/experiments_esel1, that the seed order run is the preregistered one,
// that the smoke seed (999971) is absent from the analysis, that effects.json agrees with
// paired-report.json and runs.csv (including every extinction drop), that per-seed deltas are
// arithmetic, and that execution-lock.json was written against the committed manifests.
//
//   node scripts/verify_esel1_artifacts.mjs [artifact-dir]
//
// Defaults to `artifacts/experiments/e-sel1-food-scarcity`.
//
// NOT WIRED INTO CI OR package.json. No E-SEL1 artifact exists yet (the preregistration is frozen
// before any smoke or experimental run). When the artifact directory does not exist this prints
// "no artifacts yet" and exits 0; if it exists but is empty or incomplete that is a failure.
// Wire it into CI/package.json in the same commit that first commits artifacts.

import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const artifactDir = resolve(
  process.argv[2] ?? join(repoRoot, 'artifacts/experiments/e-sel1-food-scarcity'),
);
const committedManifestDir = 'src-tauri/tests/fixtures/experiments_esel1';
const PREREG = 'esel1-preregistration.json';

const failures = [];
const notes = [];
const fail = (msg) => failures.push(msg);
const note = (msg) => notes.push(msg);

const sha256 = (buf) => createHash('sha256').update(buf).digest('hex');
const readJson = (p) => JSON.parse(readFileSync(p, 'utf8'));

/** The bytes Git has committed, not the platform-dependent working-tree representation. */
function readCommitted(name) {
  try {
    return execFileSync('git', ['cat-file', 'blob', `HEAD:${committedManifestDir}/${name}`], {
      cwd: repoRoot,
      stdio: ['ignore', 'pipe', 'ignore'],
    });
  } catch {
    return null;
  }
}

function committedPrereg() {
  const bytes = readCommitted(PREREG);
  if (bytes === null) throw new Error(`HEAD has no ${committedManifestDir}/${PREREG}`);
  return JSON.parse(bytes.toString('utf8'));
}

function committedManifest(name) {
  const bytes = readCommitted(name);
  return bytes === null ? null : JSON.parse(bytes.toString('utf8'));
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

/** Every artifact must be covered by a checksum; hand-written files are checked on content. */
function verifyNothingUncovered(dir, covered, label) {
  const skip = new Set(['checksums.sha256', 'execution-lock.json', 'RESULT.md']);
  const walk = (rel) => {
    for (const entry of readdirSync(join(dir, rel), { withFileTypes: true })) {
      const child = rel ? `${rel}/${entry.name}` : entry.name;
      if (entry.isDirectory()) {
        // smoke/ and replay/ are verified (or checked) separately; superseded/ may never feed the
        // analysis and is neither re-verified nor allowed to fail this walk.
        if (child === 'smoke' || child === 'replay' || child === 'superseded') continue;
        walk(child);
      } else if (!skip.has(child) && !covered.has(child)) {
        fail(`${label}: ${child} exists but no checksum covers it`);
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
    const committed = readCommitted(entry);
    if (committed === null) {
      fail(`${label}: manifests/${entry} has no counterpart in ${committedManifestDir}`);
      continue;
    }
    const a = sha256(readFileSync(join(copied, entry)));
    const b = sha256(committed);
    if (a !== b) {
      fail(`${label}: manifests/${entry} (${a}) is not the committed file (${b}).`);
    }
  }
}

/** Parse runs.csv into row objects (fields the runner quotes are handled). */
function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = '';
  let quoted = false;
  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i];
    if (quoted) {
      if (ch === '"' && text[i + 1] === '"') {
        field += '"';
        i += 1;
      } else if (ch === '"') quoted = false;
      else field += ch;
    } else if (ch === '"') quoted = true;
    else if (ch === ',') {
      row.push(field);
      field = '';
    } else if (ch === '\n') {
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
    } else field += ch;
  }
  if (field !== '' || row.length) {
    row.push(field);
    rows.push(row);
  }
  const header = rows.shift() ?? [];
  return rows.map((r) => Object.fromEntries(header.map((h, i) => [h, r[i] ?? ''])));
}

function verifyAgainstPreregistration(dir, label, { isSmoke }) {
  const prereg = committedPrereg();
  const prov = readJson(join(dir, 'provenance.json'));
  const report = readJson(join(dir, 'paired-report.json'));
  const effects = readJson(join(dir, 'effects.json'));
  const runs = parseCsv(readFileSync(join(dir, 'runs.csv'), 'utf8'));

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
    if (ran.includes(smokeSeed)) fail(`${label}: the smoke seed ${smokeSeed} appears in the analysis`);
    if (runs.some((r) => Number(r.seed) === smokeSeed)) {
      fail(`${label}: runs.csv lists the smoke seed ${smokeSeed}`);
    }
  }

  // Extinction and failure drops, recomputed from runs.csv rather than trusted from effects.json.
  const extinctRuns = runs.filter((r) => r.extinct === 'true');
  const failedRuns = runs.filter((r) => r.status !== 'Completed' && r.status !== 'completed');
  const droppedSeeds = new Set([...extinctRuns, ...failedRuns].map((r) => Number(r.seed)));
  const listed = effects.extinction?.extinct_runs ?? [];
  const key = (x) => `${x.seed}|${x.arm}|${x.tick}`;
  const fromCsv = new Set(extinctRuns.map((r) => `${r.seed}|${r.arm}|${r.extinct_tick}`));
  const fromEffects = new Set(listed.map(key));
  for (const k of fromCsv) {
    if (!fromEffects.has(k)) fail(`${label}: runs.csv has extinct run ${k} that effects.json does not list`);
  }
  for (const k of fromEffects) {
    if (!fromCsv.has(k)) fail(`${label}: effects.json lists extinct run ${k} that runs.csv does not show`);
  }
  if (effects.extinction?.n_extinct_runs_total !== listed.length) {
    fail(`${label}: effects.json extinction count ${effects.extinction?.n_extinct_runs_total} != ${listed.length} listed`);
  }

  // Complete pairs: both arms Completed and neither extinct, recomputed from the paired report.
  const complete = report.pairs.filter(
    (p) =>
      p.control.status === 'Completed' &&
      p.treatment.status === 'Completed' &&
      !droppedSeeds.has(p.seed),
  ).length;
  if (complete !== effects.n_complete_pairs_used_by_h1) {
    fail(
      `${label}: effects.json claims ${effects.n_complete_pairs_used_by_h1} complete pairs used by H1, ` +
        `paired-report.json and runs.csv yield ${complete}`,
    );
  }
  if (effects.h1 && effects.h1.n_complete_pairs !== complete) {
    fail(`${label}: effects.h1.n_complete_pairs ${effects.h1.n_complete_pairs} != ${complete}`);
  }
  const minimum = prereg.seeds.min_complete_pairs_for_a_decision;
  if (!isSmoke && complete < minimum) {
    note(
      `${label}: ${complete} complete pairs against a registered minimum of ${minimum} - ` +
        `"insufficient evidence" (plan section 5.4), not a decision taken on ${complete}`,
    );
  }
  if (!isSmoke && effects.sufficient_pairs !== complete >= minimum) {
    fail(`${label}: effects.sufficient_pairs disagrees with ${complete} >= ${minimum}`);
  }
  if (extinctRuns.length) {
    note(`${label}: ${extinctRuns.length} extinct run(s): ${[...fromCsv].join('; ')}`);
  }
  if (failedRuns.length) note(`${label}: ${failedRuns.length} failed run(s) listed in runs.csv`);

  if (!prereg.duration.duration_ticks_ladder.includes(prov.duration.duration_ticks_as_run)) {
    fail(
      `${label}: ran at ${prov.duration.duration_ticks_as_run} ticks, not on the registered ladder ` +
        `${JSON.stringify(prereg.duration.duration_ticks_ladder)}`,
    );
  }
  if (prov.duration.sample_period !== prereg.duration.sample_period) {
    fail(`${label}: sample period ${prov.duration.sample_period} is not ${prereg.duration.sample_period}`);
  }
  if (prov.build.profile !== 'release') {
    fail(`${label}: built as "${prov.build.profile}"; the preregistration requires release`);
  }
  if (JSON.stringify(report.declared_factors) !== JSON.stringify(['initial_conditions'])) {
    fail(`${label}: declared factors are ${JSON.stringify(report.declared_factors)}, not ["initial_conditions"]`);
  }
  if (effects.integrity_findings?.length) {
    fail(`${label}: open harness-integrity finding(s): ${JSON.stringify(effects.integrity_findings)}`);
  }
  if (String(prereg.reproduction_command).includes('cargo run')) {
    fail(`${label}: the registered reproduction command uses \`cargo run\`, which is forbidden`);
  }
}

/** Every per-seed delta must be exactly `treatment - control`, recomputed here. */
function verifyDeltasAreArithmetic(dir, label) {
  const csv = readFileSync(join(dir, 'per-seed-deltas.csv'), 'utf8').trim().split('\n');
  const header = csv.shift();
  if (header !== 'seed,contrast,observable,control_final,treatment_final,delta') {
    fail(`${label}: per-seed-deltas.csv header is "${header}", not the runner's schema`);
    return;
  }
  let checked = 0;
  for (const row of csv) {
    const [seed, contrast, observable, c, t, d] = row.split(',');
    if (Number(t) - Number(c) !== Number(d)) {
      fail(`${label}: seed ${seed} ${contrast} ${observable}: delta ${d} is not ${t} - ${c} (= ${Number(t) - Number(c)})`);
    }
    checked += 1;
  }
  note(`${label}: recomputed ${checked} per-seed deltas`);
}

/**
 * The execution lock is written by hand after the smoke and before the ensemble (plan section 7).
 * It must state the runner's key `duration_ticks`, which must be on the ladder and equal to the
 * duration the ensemble ran; its recorded manifest hashes (if any) must be the committed ones; its
 * N0 / F_hi / F_lo must be what the committed experimental manifests hold; and none of those
 * committed values may still be a negative placeholder sentinel.
 */
function verifyLock(dir, ensembleExists) {
  const lockPath = join(dir, 'execution-lock.json');
  if (!existsSync(lockPath)) {
    if (ensembleExists) fail('lock: an ensemble exists but execution-lock.json does not');
    else note('lock: no execution-lock.json yet');
    return;
  }
  const lock = readJson(lockPath);
  const prereg = committedPrereg();
  if (!prereg.duration.duration_ticks_ladder.includes(lock.duration_ticks)) {
    fail(`lock: duration_ticks ${lock.duration_ticks} is not on the registered ladder`);
  }
  if (ensembleExists) {
    const ran = readJson(join(dir, 'provenance.json')).duration.duration_ticks_as_run;
    if (ran !== lock.duration_ticks) {
      fail(`lock: the ensemble ran at ${ran} ticks but the rung locked before it was ${lock.duration_ticks}`);
    }
  }
  const ctrl = committedManifest(prereg.manifests.control);
  const trt = committedManifest(prereg.manifests.treatment);
  const drift = committedManifest(prereg.manifests.drift_control_optional_requires_P4);
  for (const [name, m] of [['control', ctrl], ['treatment', trt], ['drift', drift]]) {
    if (!m) {
      fail(`lock: committed ${name} manifest is missing`);
      continue;
    }
    for (const [k, v] of m.initial_conditions.values) {
      if (v < 0) fail(`lock: committed ${name} manifest still holds the placeholder sentinel ${k} = ${v}`);
    }
  }
  const val = (m, k) => m?.initial_conditions.values.find(([key]) => key === k)?.[1];
  if (ctrl && trt) {
    for (const [field, expected] of [
      ['N0', val(ctrl, 'live.founders')],
      ['F_hi', val(ctrl, 'live.food_cap')],
      ['F_lo', val(trt, 'live.food_cap')],
    ]) {
      if (lock[field] !== undefined && lock[field] !== expected) {
        fail(`lock: ${field} = ${lock[field]} but the committed manifests hold ${expected}`);
      }
    }
  }
  for (const [name, digest] of Object.entries(lock.manifest_sha256 ?? {})) {
    const committed = readCommitted(name);
    if (committed === null) fail(`lock: manifest_sha256 names ${name}, which is not committed`);
    else if (sha256(committed) !== digest) fail(`lock: ${name} was locked at ${digest}, but the committed file is ${sha256(committed)}`);
  }
  const smokeProv = join(dir, 'smoke', 'provenance.json');
  if (existsSync(smokeProv)) {
    const smokeCommit = readJson(smokeProv).build.git_commit;
    if (lock.build_commit && smokeCommit && lock.build_commit !== smokeCommit) {
      fail(`lock: locked against commit ${lock.build_commit}, but the smoke ran at ${smokeCommit}`);
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
  console.log(`no artifacts yet: ${artifactDir} does not exist (E-SEL1 is frozen, not yet run).`);
  process.exit(0);
}

const ensembleExists = existsSync(join(artifactDir, 'paired-report.json'));
if (ensembleExists) verifyDirectory(artifactDir, 'analysis', { isSmoke: false });
else note('analysis: no ensemble has been run yet (no paired-report.json at the artifact root)');

const smokeDir = join(artifactDir, 'smoke');
if (existsSync(smokeDir)) verifyDirectory(smokeDir, 'smoke', { isSmoke: true });
else note('smoke: no smoke/ directory present');

verifyLock(artifactDir, ensembleExists);

for (const line of notes) console.log(`note  ${line}`);
if (failures.length === 0) {
  console.log(`\nOK - every E-SEL1 artifact in ${artifactDir} verifies against node:crypto and the preregistration.`);
  process.exit(0);
}
console.error('');
for (const line of failures) console.error(`FAIL  ${line}`);
console.error(`\n${failures.length} failure(s).`);
process.exit(1);
