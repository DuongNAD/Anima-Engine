#!/usr/bin/env node
// Drive the E-SEL1 smoke exactly as the frozen preregistration prescribes, with no judgement calls.
//
// The algorithm is `smoke_may_set_only.algorithm` in
// src-tauri/tests/fixtures/experiments_esel1/esel1-preregistration.json and section 6 of
// docs/ai/planning/2026-11-16-experiment-e-sel1-food-scarcity-body-mass.md. This script only
// sequences runner invocations and applies those rules; it consults nothing but the two
// allow-listed observables (live.agent_count, live.standing_crop) and the measured us/tick, read
// from each invocation's smoke-report.json. The runner itself never writes a body-mass value in
// --smoke mode; this script refuses to read any report whose text names one.
//
//   node scripts/esel1_smoke_driver.mjs --bin <release run_esel1_experiment> --work <dir> [--repo <checkout>]
//
// Each trial copies the committed fixtures into <work>/manifests/<key>/, sets live.founders and
// live.food_cap on the three smoke manifests (the only keys a smoke manifest may change), and runs
// `<bin> --smoke --manifest-dir <that dir> --out <work>/trials/<key>`. The runner is deterministic,
// so a trial whose (N0, F_abundant, F_scarce) was already run is read back, not re-run; this is
// the reuse step C prescribes for c = 1/2. Decisions go to <work>/smoke-decisions.json.
//
// Amendment 1 (2026-11-17) adds one mechanical precondition, a manipulation check: a trial whose
// abundant and scarce arms differ in food cap but end on the same final checksum stops the smoke.
// Smoke run 1 stopped at step C for exactly that reason (F-043, live.food_cap was inert).

import { spawnSync, execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync, existsSync, mkdirSync, readdirSync, copyFileSync } from 'node:fs';
import { join, resolve, dirname } from 'node:path';
import { loadavg } from 'node:os';
import { fileURLToPath } from 'node:url';

const PREREG = 'esel1-preregistration.json';

// Constants of the frozen algorithm (preregistration smoke_may_set_only.algorithm).
const STEP_A_N0 = 30;
const STEP_A_F_START = 50;
const STEP_A_F_STEP = 10;
const STEP_A_F_FLOOR = 20;
const STEP_A_MAX_AGENTS = 80;
const STEP_B_N0 = [30, 40, 50, 60, 70, 80];
const MIN_AGENTS = 5;
const STEP_C_FRACTIONS = [
  ['1/2', 1 / 2],
  ['1/3', 1 / 3],
  ['1/4', 1 / 4],
  ['1/8', 1 / 8],
];
const R_LO = 0.4;
const R_HI = 0.6;
const RUNS_IN_ENSEMBLE = 36;
const LADDER = [18000, 12000, 6000];
const BUDGET_MINUTES = 90;

const roundCap = (x) => Math.max(1, Math.floor(x + 0.5));

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--bin' || a === '--work' || a === '--repo') out[a.slice(2)] = resolve(argv[++i]);
    else throw new Error(`unknown argument ${a}`);
  }
  if (!out.bin || !out.work) throw new Error('usage: esel1_smoke_driver.mjs --bin <binary> --work <dir> [--repo <checkout>]');
  return out;
}

const args = parseArgs(process.argv.slice(2));
// --repo: the checkout whose committed fixtures and HEAD the smoke runs against (default: this
// script's own checkout). The runner records that checkout's commit and whether its tree is dirty,
// so keep --work outside it.
const repoRoot = args.repo ?? resolve(dirname(fileURLToPath(import.meta.url)), '..');
const FIXTURES = join(repoRoot, 'src-tauri/tests/fixtures/experiments_esel1');
const prereg = JSON.parse(readFileSync(join(FIXTURES, PREREG), 'utf8'));
const SMOKE_FILES = {
  control: prereg.manifests.smoke_control,
  treatment: prereg.manifests.smoke_treatment,
  drift: prereg.manifests.smoke_drift,
};
if (JSON.stringify(prereg.duration.duration_ticks_ladder) !== JSON.stringify(LADDER)) {
  throw new Error('the registered ladder differs from the one this driver applies');
}

const log = [];
const trials = new Map();
const loadBefore = new Map();
const loadAfter = new Map();

function setKey(manifest, key, value) {
  const row = manifest.initial_conditions.values.find(([k]) => k === key);
  if (!row) throw new Error(`${manifest.experiment_id} has no ${key}`);
  row[1] = value;
}

/** One runner invocation at (N0, F_abundant, F_scarce); returns the consulted quantities per arm. */
function trial(n0, fAbundant, fScarce) {
  const key = `n0-${n0}_fa-${fAbundant}_fs-${fScarce}`;
  if (trials.has(key)) return trials.get(key);
  const manifestDir = join(args.work, 'manifests', key);
  const outDir = join(args.work, 'trials', key);
  const reportPath = join(outDir, 'smoke', 'smoke-report.json');
  if (!existsSync(reportPath)) {
    mkdirSync(manifestDir, { recursive: true });
    for (const f of readdirSync(FIXTURES)) copyFileSync(join(FIXTURES, f), join(manifestDir, f));
    for (const [arm, file] of Object.entries(SMOKE_FILES)) {
      const p = join(manifestDir, file);
      const m = JSON.parse(readFileSync(p, 'utf8'));
      setKey(m, 'live.founders', n0);
      setKey(m, 'live.food_cap', arm === 'control' ? fAbundant : fScarce);
      writeFileSync(p, `${JSON.stringify(m, null, 2)}\n`);
    }
    console.log(`smoke trial ${key} ...`);
    loadBefore.set(key, loadavg());
    const r = spawnSync(args.bin, ['--smoke', '--manifest-dir', manifestDir, '--out', outDir], {
      cwd: repoRoot,
      stdio: 'inherit',
    });
    if (r.status !== 0) throw new Error(`the runner exited ${r.status} on trial ${key}`);
    loadAfter.set(key, loadavg());
  }
  const text = readFileSync(reportPath, 'utf8');
  if (text.includes('trait_mass')) throw new Error(`${reportPath} names a body-mass observable`);
  const report = JSON.parse(text);
  if (report.contains_body_mass !== false) throw new Error(`${reportPath} does not state contains_body_mass = false`);
  const arms = {};
  for (const run of report.runs) {
    arms[run.arm] = {
      status: run.status,
      extinct_tick: run.extinct_tick,
      min_agents: run.min_live_agent_count_all_samples,
      max_agents_le_1200: run.max_live_agent_count_tick_le_1200,
      crop_tail_mean: run.mean_live_standing_crop_tick_gt_half_T,
      us_per_tick: run.us_per_tick,
      final_checksum: run.final_checksum,
    };
  }
  // Host load (1, 5, 15 min) around the invocation: context for the us/tick, never an input.
  const host_load = { before: loadBefore.get(key) ?? null, after: loadAfter.get(key) ?? null };
  const t = { key, n0, f_abundant: fAbundant, f_scarce: fScarce, T_smoke: report.T_smoke, arms, host_load };
  trials.set(key, t);
  // Manipulation check (amendment 1, F-043): an abundant and a scarce arm that differ in their
  // food cap yet end on the same final checksum ran the same world, so the declared factor did
  // nothing. That stops the smoke; it can never choose a value.
  if (fAbundant !== fScarce && arms.control.final_checksum === arms.treatment.final_checksum) {
    log.push({ step: 'manipulation_check', trial: key, final_checksum: arms.control.final_checksum, pass: false });
    stop(
      `manipulation check: trial ${key} ran food caps ${fAbundant} and ${fScarce} to the same final ` +
        `checksum ${arms.control.final_checksum}; live.food_cap had no effect`,
    );
  }
  return t;
}

function stop(finding) {
  log.push({ step: 'stop', finding });
  finish({ outcome: 'stopped', finding });
  console.error(`E-SEL1 smoke stopped: ${finding}`);
  process.exit(3);
}

function finish(result) {
  const git = (a) => {
    try {
      return execFileSync('git', a, { cwd: repoRoot, encoding: 'utf8' }).trim();
    } catch {
      return null;
    }
  };
  const doc = {
    kind: 'e-sel1-smoke-decisions-EXCLUDED-FROM-ANALYSIS',
    consulted_only: ['live.agent_count', 'live.standing_crop', 'us_per_tick'],
    git_commit: git(['rev-parse', 'HEAD']),
    algorithm_source: `${PREREG} smoke_may_set_only.algorithm`,
    trials: [...trials.values()],
    steps: log,
    result,
  };
  const text = `${JSON.stringify(doc, null, 2)}\n`;
  if (text.includes('trait_mass')) throw new Error('the decision record would name a body-mass observable');
  writeFileSync(join(args.work, 'smoke-decisions.json'), text);
}

// ---- Step A: F_hi ------------------------------------------------------------------------------
let fHi = null;
for (let f = STEP_A_F_START; f >= STEP_A_F_FLOOR; f -= STEP_A_F_STEP) {
  const t = trial(STEP_A_N0, f, roundCap(f / 2));
  const m = t.arms.control.max_agents_le_1200;
  const pass = m !== null && m !== undefined && m <= STEP_A_MAX_AGENTS;
  log.push({ step: 'A', trial: t.key, F: f, max_agent_count_tick_le_1200: m, pass });
  if (pass) {
    fHi = f;
    break;
  }
}
if (fHi === null) stop(`step A: even F = ${STEP_A_F_FLOOR} gives max live.agent_count > ${STEP_A_MAX_AGENTS} at tick <= 1200`);

// ---- Step B: N0 --------------------------------------------------------------------------------
let n0 = null;
let abundantRef = null;
for (const n of STEP_B_N0) {
  const t = trial(n, fHi, roundCap(fHi / 2));
  const a = t.arms.control.min_agents;
  const s = t.arms.treatment.min_agents;
  const pass = a !== null && s !== null && a >= MIN_AGENTS && s >= MIN_AGENTS;
  log.push({ step: 'B', trial: t.key, N0: n, min_agents_abundant: a, min_agents_scarce: s, pass });
  if (pass) {
    n0 = n;
    abundantRef = t.arms.control;
    break;
  }
}
if (n0 === null) stop(`step B: no N0 in ${STEP_B_N0.join(', ')} keeps min live.agent_count >= ${MIN_AGENTS} in both runs`);

// ---- Step C: F_lo ------------------------------------------------------------------------------
let fLo = null;
let cAccepted = null;
const atN0 = [trial(n0, fHi, roundCap(fHi / 2))];
for (const [label, c] of STEP_C_FRACTIONS) {
  const fs = roundCap(c * fHi);
  const t = trial(n0, fHi, fs);
  if (!atN0.includes(t)) atN0.push(t);
  const crop = t.arms.treatment.crop_tail_mean;
  const ref = abundantRef.crop_tail_mean;
  const r = crop !== null && ref !== null && ref > 0 ? crop / ref : null;
  const minS = t.arms.treatment.min_agents;
  const pass = r !== null && r >= R_LO && r <= R_HI && minS !== null && minS >= MIN_AGENTS;
  log.push({
    step: 'C',
    trial: t.key,
    c: label,
    F_scarce: fs,
    standing_crop_tail_mean_scarce: crop,
    standing_crop_tail_mean_abundant: ref,
    r,
    min_agents_scarce: minS,
    pass,
  });
  if (pass) {
    fLo = fs;
    cAccepted = label;
    break;
  }
}
if (fLo === null) stop(`step C: no c in 1/2, 1/3, 1/4, 1/8 gives ${R_LO} <= r <= ${R_HI} with min live.agent_count >= ${MIN_AGENTS}`);

// ---- Duration rung -----------------------------------------------------------------------------
// max us/tick over all smoke runs at the accepted N0 (steps B and C), no parallelism credit.
const maxUs = Math.max(...atN0.flatMap((t) => Object.values(t.arms).map((a) => a.us_per_tick)));
let rung = null;
const estimates = LADDER.map((T) => {
  const minutes = (RUNS_IN_ENSEMBLE * T * maxUs) / 60e6;
  return { T, estimated_total_minutes: minutes, within_budget: minutes <= BUDGET_MINUTES };
});
for (const e of estimates) {
  if (e.within_budget) {
    rung = e.T;
    break;
  }
}
log.push({ step: 'rung', max_us_per_tick_at_accepted_N0: maxUs, trials_at_accepted_N0: atN0.map((t) => t.key), estimates });
if (rung === null) stop(`rung: even T = 6000 exceeds ${BUDGET_MINUTES} minutes at ${maxUs.toFixed(1)} us/tick`);

const finalTrial = trial(n0, fHi, fLo).key;
const result = { outcome: 'chosen', N0: n0, F_hi: fHi, F_lo: fLo, c: cAccepted, duration_ticks: rung, final_trial: finalTrial };
finish(result);
console.log(JSON.stringify(result));
