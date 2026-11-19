#!/usr/bin/env node
// Drive the SEL2-P smoke / pipeline-check through `run_sel2p_experiment` and enforce body-mass
// blindness + QD-SEL2g hard-stop gating on C1..C5.
//
//   node scripts/sel2p_smoke_driver.mjs --bin <release run_sel2p_experiment> --work <dir> [--repo <checkout>] [--mode smoke|pipeline-check]

import { spawnSync, execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync, existsSync, mkdirSync, readdirSync } from 'node:fs';
import { join, resolve, dirname } from 'node:path';
import { loadavg } from 'node:os';
import { fileURLToPath } from 'node:url';
import {
  evaluatePooledBaseline,
  evaluateSmokeGate,
  parseCsv,
  summarizeSmokeSeries,
} from './verify_sel2p_artifacts.mjs';

const PREREG = 'sel2p-preregistration.json';

function parseArgs(argv) {
  const out = { mode: 'smoke' };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--bin' || a === '--work' || a === '--repo') {
      out[a.slice(2)] = resolve(argv[++i]);
    } else if (a === '--mode') {
      out.mode = argv[++i];
    } else {
      throw new Error(`unknown argument ${a}`);
    }
  }
  if (!out.bin || !out.work || !['smoke', 'pipeline-check'].includes(out.mode)) {
    throw new Error(
      'usage: sel2p_smoke_driver.mjs --bin <binary> --work <dir> [--repo <checkout>] [--mode smoke|pipeline-check]',
    );
  }
  return out;
}

const args = parseArgs(process.argv.slice(2));
const repoRoot = args.repo ?? resolve(dirname(fileURLToPath(import.meta.url)), '..');
const FIXTURES = join(repoRoot, 'src-tauri/tests/fixtures/experiments_sel2p');
const prereg = JSON.parse(readFileSync(join(FIXTURES, PREREG), 'utf8'));

// Blindness pre-check: none of the 4 smoke/baseline manifests may request any trait_mass_* id.
for (const key of ['baseline_ref', 'smoke_selected', 'smoke_scarce_mut0', 'smoke_scarce_drift']) {
  const file = prereg.manifests[key];
  const raw = readFileSync(join(FIXTURES, file), 'utf8');
  if (raw.includes('trait_mass')) {
    throw new Error(`smoke/baseline manifest ${file} names a trait_mass observable`);
  }
}

mkdirSync(args.work, { recursive: true });
const flag = args.mode === 'smoke' ? '--smoke' : '--pipeline-check';
const beforeLoad = loadavg();
const run = spawnSync(args.bin, [flag, '--manifest-dir', FIXTURES, '--out', args.work], {
  cwd: repoRoot,
  stdio: 'inherit',
});
const afterLoad = loadavg();
if (run.status !== 0) {
  throw new Error(`run_sel2p_experiment ${flag} exited ${run.status}`);
}

const smokeDir = join(args.work, 'smoke');
for (const name of readdirSync(smokeDir)) {
  const text = readFileSync(join(smokeDir, name), 'utf8');
  if (text.includes('trait_mass')) {
    throw new Error(`blindness violation: ${join(smokeDir, name)} contains trait_mass`);
  }
}

const reportPath = join(smokeDir, 'smoke-report.json');
const seriesPath = join(smokeDir, 'smoke-series.csv');
if (!existsSync(reportPath) || !existsSync(seriesPath)) {
  throw new Error(`missing smoke outputs in ${smokeDir}`);
}
const report = JSON.parse(readFileSync(reportPath, 'utf8'));
if (report.contains_body_mass !== false) {
  throw new Error(`${reportPath} does not state contains_body_mass === false`);
}
if (report.baseline_computed_before_scarce !== true) {
  throw new Error(`${reportPath} does not state baseline_computed_before_scarce === true`);
}

// Recompute pooled baseline and C1..C5 independently from smoke-series.csv.
const seriesRows = parseCsv(readFileSync(seriesPath, 'utf8'));
const durationTicks = report.duration_ticks;
const summarized = summarizeSmokeSeries(
  seriesRows,
  durationTicks,
  report.runs,
  (msg) => {
    throw new Error(msg);
  },
  report.sample_period,
);
const baseline = evaluatePooledBaseline(
  summarized.baseline,
  prereg.seeds.min_surviving_baseline_seeds,
);
const gate = evaluateSmokeGate(
  durationTicks,
  baseline,
  summarized.scarce.selected,
  summarized.scarce.scarce_mut0,
  summarized.scarce.scarce_drift,
  prereg.smoke_gate,
);

const gitCommit = (() => {
  try {
    return execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repoRoot, encoding: 'utf8' }).trim();
  } catch {
    return null;
  }
})();

const decisions = {
  kind: 'sel2p-smoke-decisions-EXCLUDED-FROM-CONFIRMATION',
  mode: args.mode,
  consulted_only: [
    'live.closed_eu_total',
    'live.agent_count',
    'live.standing_crop',
    'live.births',
    'live.deaths',
    'live.mean_lifespan',
    'live.deaths_starvation',
    'live.deaths_dehydration',
  ],
  contains_body_mass: false,
  baseline_computed_before_scarce: true,
  git_commit: gitCommit,
  host_load: { before: beforeLoad, after: afterLoad },
  duration_ticks: durationTicks,
  pooled_baseline: baseline,
  gate,
  outcome: gate.gate_pass ? 'pass' : 'stopped',
};
const decisionsText = `${JSON.stringify(decisions, null, 2)}\n`;
if (decisionsText.includes('trait_mass')) {
  throw new Error('smoke-decisions.json would name a body-mass observable');
}
writeFileSync(join(args.work, 'smoke-decisions.json'), decisionsText);
console.log(JSON.stringify({ outcome: decisions.outcome, gate }));

if (!gate.gate_pass) {
  console.error(
    `SEL2-P ${args.mode} smoke hard-stop (0 confirmation seeds permitted): ` +
      `C1=${gate.c1} C2=${gate.c2} C3=${gate.c3} C4=${gate.c4} C5=${gate.c5} drift_valid=${gate.drift_valid}`,
  );
  process.exit(3);
}
