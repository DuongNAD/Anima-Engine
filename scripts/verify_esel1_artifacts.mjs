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
//   node scripts/verify_esel1_artifacts.mjs --self-test
//
// Defaults to `artifacts/experiments/e-sel1-food-scarcity`.
//
// NOT WIRED INTO CI OR package.json. No E-SEL1 artifact exists yet (the preregistration is frozen
// before any smoke or experimental run). ONLY when the artifact directory does not exist this
// prints "no artifacts yet" and exits 0. An existing directory that is empty, or that lacks any
// required file (paired-report.json, effects.json, runs.csv, per-seed-deltas.csv, provenance.json,
// checksums.sha256, execution-lock.json and the three experimental manifests under manifests/) is a
// FAILURE, never a pass. Beyond the byte-level checks it recomputes the H1 statistics (mean delta,
// SD, SE, 95% CI, d_z) from per-seed-deltas.csv, re-derives the materiality verdict, and checks the
// execution lock's sha256 against provenance.json. `--self-test` runs its negative controls.
// Wire it into CI/package.json in the same commit that first commits artifacts.

import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync, existsSync, readdirSync, mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const SELF_TEST = process.argv[2] === '--self-test';
const artifactDir = resolve(
  (SELF_TEST ? undefined : process.argv[2]) ??
    join(repoRoot, 'artifacts/experiments/e-sel1-food-scarcity'),
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

/** Two-sided 95% Student-t critical values, df 1..30, as the runner's conservative table holds them. */
const T_975 = [
  12.706204736432095, 4.302652729696142, 3.1824463052842638, 2.7764451051977987, 2.570581835636314,
  2.4469118487916806, 2.3646242510102993, 2.306004135204166, 2.2621571628540993, 2.2281388519649385,
  2.200985160082949, 2.1788128296634177, 2.1603686564610127, 2.1447866879169273, 2.131449545559323,
  2.1199052992210112, 2.109815577833181, 2.10092204024096, 2.093024054408263, 2.0859634472658364,
  2.079613844727662, 2.073873067904015, 2.068657610419041, 2.063898561628021, 2.059538552753294,
  2.055529438642871, 2.051830516480289, 2.048407141795244, 2.045229642132703, 2.042272456301238,
];

/** Paired statistics of a list of per-seed deltas, recomputed independently of the runner. */
export function pairedStats(control, treatment) {
  const n = control.length;
  const deltas = control.map((c, i) => treatment[i] - c);
  const mean = (xs) => xs.reduce((a, b) => a + b, 0) / xs.length;
  const out = { n, controlMean: null, treatmentMean: null, delta: null, sd: null, se: null, lo: null, hi: null, dz: null };
  if (n === 0) return out;
  out.controlMean = mean(control);
  out.treatmentMean = mean(treatment);
  out.delta = mean(deltas);
  if (n < 2) return out;
  out.sd = Math.sqrt(deltas.reduce((a, d) => a + (d - out.delta) ** 2, 0) / (n - 1));
  out.se = out.sd / Math.sqrt(n);
  if (n - 1 <= 30) {
    const half = T_975[n - 2] * out.se;
    out.lo = out.delta - half;
    out.hi = out.delta + half;
  }
  if (out.sd > 0) out.dz = out.delta / out.sd;
  return out;
}

/** The preregistered materiality verdict: all_of(ci excludes 0, |dz| >= 0.8, |delta| >= 5% |control|). */
export function materiality(st) {
  const ci = st.lo !== null && st.hi !== null && (st.lo > 0 || st.hi < 0);
  const dz = st.dz !== null && Math.abs(st.dz) >= 0.8;
  const rel = st.delta !== null && st.controlMean !== null && Math.abs(st.delta) >= 0.05 * Math.abs(st.controlMean);
  return { ci, dz, rel, material: ci && dz && rel };
}

/** Relative tolerance 1e-9 (absolute 1e-12 near zero); null must match null. */
function close(a, b) {
  if (a === null || a === undefined || b === null || b === undefined) {
    return (a ?? null) === (b ?? null);
  }
  return Math.abs(a - b) <= Math.max(1e-9 * Math.max(Math.abs(a), Math.abs(b)), 1e-12);
}

const ROOT_FILES = [
  'paired-report.json',
  'effects.json',
  'runs.csv',
  'per-seed-deltas.csv',
  'provenance.json',
  'checksums.sha256',
  'execution-lock.json',
];
// The smoke writes allow-listed observables only (no body mass, no effects, no manifest copies).
const SMOKE_FILES = [
  'runs.csv',
  'smoke-series.csv',
  'smoke-report.json',
  'provenance.json',
  'summary.md',
  'checksums.sha256',
];

/** Missing required files are failures. Returns true when every one is present. */
function requireFiles(dir, label, files, manifestNames) {
  let ok = true;
  for (const f of files) {
    if (!existsSync(join(dir, f))) {
      fail(`${label}: required file ${f} is missing`);
      ok = false;
    }
  }
  for (const m of manifestNames) {
    if (!existsSync(join(dir, 'manifests', m))) {
      fail(`${label}: required manifests/${m} is missing`);
      ok = false;
    }
  }
  return ok;
}

/** H1 recomputed from per-seed-deltas.csv alone, compared with effects.json (tolerance 1e-9). */
function verifyH1Recompute(dir, label) {
  const lines = readFileSync(join(dir, 'per-seed-deltas.csv'), 'utf8').trim().split('\n');
  lines.shift();
  const control = [];
  const treatment = [];
  for (const row of lines) {
    const [, contrast, , c, t] = row.split(',');
    if (!contrast.startsWith('H1')) continue;
    control.push(Number(c));
    treatment.push(Number(t));
  }
  const effects = readJson(join(dir, 'effects.json'));
  const h1 = effects.h1;
  if (!h1) {
    fail(`${label}: effects.json has no h1 block`);
    return;
  }
  const st = pairedStats(control, treatment);
  if (h1.n_complete_pairs !== st.n) {
    fail(`${label}: effects.h1.n_complete_pairs ${h1.n_complete_pairs} != ${st.n} rows of per-seed-deltas.csv`);
  }
  for (const [field, got] of [
    ['control_mean', st.controlMean],
    ['treatment_mean', st.treatmentMean],
    ['paired_mean_delta', st.delta],
    ['paired_sd', st.sd],
    ['paired_se', st.se],
    ['ci95_low', st.lo],
    ['ci95_high', st.hi],
    ['paired_dz', st.dz],
  ]) {
    if (!close(h1[field], got)) {
      fail(`${label}: effects.h1.${field} = ${h1[field]} but per-seed-deltas.csv recomputes ${got}`);
    }
  }
  const m = materiality(st);
  const claimed = h1.materiality ?? {};
  for (const [field, got] of [
    ['ci95_excludes_zero', m.ci],
    ['abs_dz_at_least_0_8', m.dz],
    ['abs_delta_at_least_5pc_of_control_mean', m.rel],
    ['material', m.material],
  ]) {
    if (claimed[field] !== got) {
      fail(`${label}: effects.h1.materiality.${field} = ${claimed[field]} but the verdict recomputes ${got}`);
    }
  }
  note(`${label}: H1 recomputed from ${st.n} per-seed deltas: material = ${m.material}`);
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
  const skip = new Set(['checksums.sha256', 'RESULT.md']);
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
  for (const field of ['duration_ticks', 'N0', 'F_hi', 'F_lo']) {
    if (typeof lock[field] !== 'number') fail(`lock: execution-lock.json lacks ${field}`);
  }
  if (ensembleExists) {
    const prov = readJson(join(dir, 'provenance.json'));
    const actual = sha256(readFileSync(lockPath));
    if (prov.execution_lock_sha256 !== actual) {
      fail(`lock: execution-lock.json hashes to ${actual}, but provenance.json records ${prov.execution_lock_sha256}`);
    }
  }
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
      if (lock[field] !== expected) {
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

/** The smoke directory: allow-listed outputs only, and provably free of any body-mass observable. */
function verifySmoke(dir, label, prereg) {
  if (!requireFiles(dir, label, SMOKE_FILES, [])) return;
  const covered = verifyChecksums(dir, label);
  verifyNothingUncovered(dir, covered, label);
  for (const entry of readdirSync(dir)) {
    const text = readFileSync(join(dir, entry), 'utf8');
    if (text.includes('trait_mass')) {
      fail(`${label}: ${entry} names a body-mass observable; the smoke may not consult or write one`);
    }
  }
  for (const banned of ['effects.json', 'paired-report.json', 'per-seed-deltas.csv', 'manifests']) {
    if (existsSync(join(dir, banned))) fail(`${label}: ${banned} must not exist in a smoke directory`);
  }
  const prov = readJson(join(dir, 'provenance.json'));
  const report = readJson(join(dir, 'smoke-report.json'));
  if (report.contains_body_mass !== false) fail(`${label}: smoke-report.json does not state contains_body_mass = false`);
  const ran = [...new Set(prov.seeds.executed)];
  if (JSON.stringify(ran) !== JSON.stringify([prereg.seeds.smoke_seed])) {
    fail(`${label}: a smoke run must run exactly [${prereg.seeds.smoke_seed}], but ran ${JSON.stringify(ran)}`);
  }
  if (prov.build.profile !== 'release') {
    fail(`${label}: built as "${prov.build.profile}"; timing measured outside release is not usable for the rung`);
  }
  note(`${label}: ${report.runs.length} smoke run(s), max ${report.max_us_per_tick?.toFixed?.(1)} us/tick`);
}

function verifyDirectory(dir, label, opts) {
  if (!existsSync(dir)) {
    fail(`${label}: ${dir} does not exist`);
    return;
  }
  const prereg = committedPrereg();
  const exp = [
    prereg.manifests.control,
    prereg.manifests.treatment,
    prereg.manifests.drift_control_optional_requires_P4,
  ].filter(Boolean);
  if (opts.isSmoke) {
    verifySmoke(dir, label, prereg);
    return;
  }
  const present = requireFiles(dir, label, ROOT_FILES, exp);
  if (!present) return; // nothing further can be read
  const covered = verifyChecksums(dir, label);
  if (!opts.isSmoke && !covered.has('execution-lock.json')) {
    fail(`${label}: checksums.sha256 does not cover execution-lock.json`);
  }
  verifyNothingUncovered(dir, covered, label);
  verifyManifestsAreTheCommittedOnes(dir, label);
  verifyAgainstPreregistration(dir, label, opts);
  verifyDeltasAreArithmetic(dir, label);
  if (!opts.isSmoke) verifyH1Recompute(dir, label);
}

function verifyAll(dir) {
  verifyDirectory(dir, 'analysis', { isSmoke: false });
  const smokeDir = join(dir, 'smoke');
  if (existsSync(smokeDir)) verifyDirectory(smokeDir, 'smoke', { isSmoke: true });
  else note('smoke: no smoke/ directory present');
  if (existsSync(join(dir, 'execution-lock.json'))) {
    verifyLock(dir, existsSync(join(dir, 'provenance.json')));
  }
}

function report() {
  for (const line of notes) console.log(`note  ${line}`);
  if (failures.length === 0) return 0;
  console.error('');
  for (const line of failures) console.error(`FAIL  ${line}`);
  console.error(`\n${failures.length} failure(s).`);
  return 1;
}

// ---- Self-test: negative controls --------------------------------------------------------------

function selfTest() {
  let bad = 0;
  const expect = (cond, what) => {
    if (!cond) {
      bad += 1;
      console.error(`SELF-TEST FAIL  ${what}`);
    } else console.log(`self-test ok    ${what}`);
  };
  // 1. The statistics reproduce a hand-computed case: deltas 1,2,3 -> mean 2, sd 1, se 1/sqrt3.
  const st = pairedStats([0, 0, 0], [1, 2, 3]);
  expect(close(st.delta, 2) && close(st.sd, 1) && close(st.se, 1 / Math.sqrt(3)), 'mean, sd, se of deltas 1,2,3');
  expect(close(st.dz, 2) && close(st.hi - st.delta, 4.302652729696142 / Math.sqrt(3)), 'dz and t-interval of deltas 1,2,3 (df = 2)');
  expect(materiality(st).material === false, 'three pairs with a CI of [-2.4, 6.4] are not material');
  expect(materiality(pairedStats([10, 10, 10, 10], [11, 11.2, 10.9, 11.1])).material === true, 'a consistent 10% shift is material');
  // 2. The empty directory case: an existing, empty directory must FAIL (exit 1), not pass.
  const root = mkdtempSync(join(tmpdir(), 'esel1-verify-selftest-'));
  const run = (dir) => {
    try {
      execFileSync(process.execPath, [fileURLToPath(import.meta.url), dir], { stdio: ['ignore', 'pipe', 'pipe'] });
      return { code: 0 };
    } catch (e) {
      return { code: e.status, err: String(e.stderr) };
    }
  };
  try {
    const empty = join(root, 'empty');
    mkdirpSync(empty);
    const r = run(empty);
    expect(r.code === 1 && /required file .* is missing/.test(r.err), 'an existing EMPTY artifact directory fails');
    // 3. An incomplete directory (one file only) fails too.
    const partial = join(root, 'partial');
    mkdirpSync(partial);
    writeFileSync(join(partial, 'runs.csv'), 'seed\n');
    const p = run(partial);
    expect(p.code === 1 && /paired-report\.json is missing/.test(p.err), 'a directory lacking required files fails');
    // 3b. A smoke directory with a body-mass name in any file fails verifySmoke.
    const smoke = join(root, 'smoke');
    mkdirpSync(smoke);
    const files = {
      'runs.csv': 'seed,arm\n',
      'smoke-series.csv': 'seed,arm,tick,live.trait_mass_mean\n',
      'smoke-report.json': '{"contains_body_mass":false,"runs":[]}',
      'provenance.json': '{"seeds":{"executed":[999971]},"build":{"profile":"release"}}',
      'summary.md': '# smoke\n',
    };
    const sums = [];
    for (const [n, t] of Object.entries(files)) {
      writeFileSync(join(smoke, n), t);
      sums.push(`${sha256(Buffer.from(t))}  ${n}`);
    }
    writeFileSync(join(smoke, 'checksums.sha256'), sums.join('\n') + '\n');
    failures.length = 0;
    verifySmoke(smoke, 'smoke', committedPrereg());
    expect(failures.some((f) => /body-mass observable/.test(f)), 'a smoke file naming a body-mass observable fails');
    failures.length = 0;
    // 4. A directory that does not exist is the only "no artifacts yet" pass.
    const absent = run(join(root, 'does-not-exist'));
    expect(absent.code === 0, 'a missing directory is "no artifacts yet" (exit 0)');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
  return bad === 0 ? 0 : 1;
}

function mkdirpSync(p) {
  mkdirSync(p, { recursive: true });
}

// ---- Run ---------------------------------------------------------------------------------------

if (SELF_TEST) {
  process.exit(selfTest());
}

if (!existsSync(artifactDir)) {
  console.log(`no artifacts yet: ${artifactDir} does not exist (E-SEL1 is frozen, not yet run).`);
  process.exit(0);
}

try {
  verifyAll(artifactDir);
} catch (e) {
  fail(`verification aborted: ${e instanceof Error ? e.message : String(e)}`);
}
const code = report();
if (code === 0) {
  console.log(`\nOK - every E-SEL1 artifact in ${artifactDir} verifies against node:crypto and the preregistration.`);
}
process.exit(code);
