#!/usr/bin/env node
// Verify a published E3 confirmation artifact directory independently of `run_e3_experiment`.
//
//   node scripts/verify_e3_artifacts.mjs [artifact-dir] [--prereg <path>]
//   node scripts/verify_e3_artifacts.mjs --self-test
//
// Defaults to `artifacts/experiments/e3-productivity-gradient-body-mass/confirmation`.
// If that default directory does not exist yet and no explicit argument was passed, exits 0 cleanly
// ("no E3 confirmation artifacts yet"); an explicit missing path or an empty/incomplete directory fails.

import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, isAbsolute, join, relative, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const committedManifestDir = 'src-tauri/tests/fixtures/experiments_e3';
const DEFAULT_CONFIRM_DIR = 'artifacts/experiments/e3-productivity-gradient-body-mass/confirmation';
const PREREG_FILE = 'e3-preregistration.json';

const ARM_ORDER = [
  'control_uniform',
  'control_uniform_scarcity',
  'treatment_gradient',
];

const CONTRAST_DEFS = [
  {
    name: 'treatment_gradient - control_uniform_scarcity',
    controlArm: 'control_uniform_scarcity',
    treatmentArm: 'treatment_gradient',
  },
  {
    name: 'treatment_gradient - control_uniform',
    controlArm: 'control_uniform',
    treatmentArm: 'treatment_gradient',
  },
  {
    name: 'control_uniform_scarcity - control_uniform',
    controlArm: 'control_uniform',
    treatmentArm: 'control_uniform_scarcity',
  },
];

// Exact NIST 0.975 Student-t quantiles matching `src-tauri/src/core/experiment_runner/statistics.rs`.
const T_975_DF_1_TO_30 = [
  12.706204736432095, 4.302652729696142, 3.182446305284264, 2.7764451051977987,
  2.570581835636314, 2.446911848791681, 2.3646242510102993, 2.306004135204166,
  2.2621571628540993, 2.2281388519649385, 2.200985160082949, 2.1788128296634177,
  2.1603686564610127, 2.1447866879169273, 2.131449545559323, 2.1199052992210112,
  2.109815577833181, 2.10092204024096, 2.093024054408263, 2.0859634472658364,
  2.079613844727662, 2.073873067904015, 2.068657610419041, 2.063898561628021,
  2.059538552753294, 2.055529438642871, 2.051830516480289, 2.048407141795244,
  2.045229642132703, 2.042272456301238,
];

export function studentT95(df) {
  if (!Number.isFinite(df) || df < 1) return NaN;
  const lower = Math.floor(df);
  if (lower <= 30) return T_975_DF_1_TO_30[lower - 1];
  if (lower < 40) return T_975_DF_1_TO_30[29];
  if (lower < 60) return 2.0210753903062733;
  if (lower < 80) return 2.000297821058262;
  if (lower < 100) return 1.9900634210714;
  if (lower < 120) return 1.9839715184496334;
  return 1.979930405052777;
}

const ZERO_SD_TOL = 1e-12;
const sha256 = (buf) => createHash('sha256').update(buf).digest('hex');
const sha256NormalizedLf = (buf) =>
  sha256(Buffer.from(buf.toString('utf8').replace(/\r\n/g, '\n'), 'utf8'));

const isSafeGitCommit = (ref) => typeof ref === 'string' && /^[0-9a-fA-F]{7,40}$/.test(ref);

function gitCommitHasBlob(ref, relPath) {
  if (!isSafeGitCommit(ref)) return false;
  try {
    execFileSync('git', ['cat-file', '-e', `${ref}:${relPath}`], {
      cwd: repoRoot,
      stdio: ['ignore', 'ignore', 'ignore'],
    });
    return true;
  } catch {
    return false;
  }
}

function readCommittedBytesAt(name, ref = 'HEAD') {
  if (ref !== 'HEAD' && !isSafeGitCommit(ref)) {
    return null;
  }
  try {
    return execFileSync('git', ['show', `${ref}:${committedManifestDir}/${name}`], {
      cwd: repoRoot,
      stdio: ['ignore', 'pipe', 'ignore'],
    });
  } catch {
    if (ref === 'HEAD') {
      const wt = join(repoRoot, committedManifestDir, name);
      return existsSync(wt) ? readFileSync(wt) : null;
    }
    return null;
  }
}

export function loadCommittedPrereg(preregOverridePath = null, gitRef = 'HEAD') {
  if (preregOverridePath) {
    return JSON.parse(readFileSync(preregOverridePath, 'utf8'));
  }
  const bytes = readCommittedBytesAt(PREREG_FILE, gitRef);
  if (!bytes) {
    throw new Error(`cannot load ${committedManifestDir}/${PREREG_FILE} at ${gitRef}`);
  }
  return JSON.parse(bytes.toString('utf8'));
}

export function isAllowedDirtyPath(entry) {
  if (typeof entry !== 'string' || entry.trim().length === 0) return false;
  const parts = entry.includes(' -> ') ? entry.split(' -> ') : [entry];
  for (const rawPart of parts) {
    let cleaned = rawPart.trim();
    if (cleaned.startsWith('"') && cleaned.endsWith('"') && cleaned.length >= 2) {
      cleaned = cleaned.slice(1, -1);
    }
    const posix = cleaned.replace(/\\/g, '/').replace(/\/+$/, '');
    if (!posix || isAbsolute(cleaned) || /^[A-Za-z]:/.test(posix)) return false;
    const segs = posix.split('/');
    if (segs[0] !== 'artifacts' || segs.length < 2) return false;
    if (segs.some((s) => s.length === 0 || s === '.' || s === '..')) return false;
  }
  return true;
}

export function parseCsv(text) {
  const lines = text.replace(/\r\n/g, '\n').split('\n').filter((l) => l.length > 0);
  if (lines.length === 0) return [];
  const headers = lines[0].split(',');
  return lines.slice(1).map((line) => {
    const vals = line.split(',');
    const row = {};
    for (let i = 0; i < headers.length; i += 1) {
      row[headers[i]] = vals[i] ?? '';
    }
    return row;
  });
}

export function median(xs) {
  if (xs.length === 0) return null;
  const sorted = [...xs].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1 ? sorted[mid] : 0.5 * (sorted[mid - 1] + sorted[mid]);
}

export function betweenSeedVariance(xs) {
  if (xs.length < 2) return null;
  const mean = xs.reduce((a, b) => a + b, 0) / xs.length;
  const ss = xs.reduce((acc, x) => acc + (x - mean) ** 2, 0);
  return ss / (xs.length - 1);
}

export function pairedStats(control, treatment, nRequested = control.length) {
  if (control.length !== treatment.length) {
    throw new Error('control and treatment vectors must have equal length');
  }
  const n = control.length;
  const deltas = control.map((c, i) => treatment[i] - c);
  const out = {
    n_requested: nRequested,
    n_complete_pairs: n,
    control_mean: null,
    treatment_mean: null,
    paired_mean_delta: null,
    paired_sd: null,
    paired_se: null,
    ci95_low: null,
    ci95_high: null,
    paired_dz: null,
    median_of_per_seed_deltas: median(deltas),
    between_seed_variance_of_deltas: betweenSeedVariance(deltas),
  };
  if (n === 0) return out;
  const mean = (xs) => xs.reduce((a, b) => a + b, 0) / xs.length;
  out.control_mean = mean(control);
  out.treatment_mean = mean(treatment);
  out.paired_mean_delta = mean(deltas);
  if (n < 2) return out;
  const sd = Math.sqrt(deltas.reduce((acc, d) => acc + (d - out.paired_mean_delta) ** 2, 0) / (n - 1));
  const sdTol = ZERO_SD_TOL * Math.max(1.0, Math.abs(out.paired_mean_delta));
  if (sd < sdTol) {
    out.paired_sd = 0;
    out.paired_se = 0;
    out.ci95_low = out.paired_mean_delta;
    out.ci95_high = out.paired_mean_delta;
    out.paired_dz = null;
    out.between_seed_variance_of_deltas = 0;
    return out;
  }
  const se = sd / Math.sqrt(n);
  const half = studentT95(n - 1) * se;
  out.paired_sd = sd;
  out.paired_se = se;
  out.ci95_low = out.paired_mean_delta - half;
  out.ci95_high = out.paired_mean_delta + half;
  out.paired_dz = sd > 0 ? out.paired_mean_delta / sd : null;
  return out;
}

export function classifyE3Decision(stats, thresholds, opts = {}) {
  const allowZeroFallback = Boolean(opts.allowZeroControlFallback);
  const minPairs = Number(thresholds?.min_valid_pairs_for_decision);
  const dzMin = Number(thresholds?.dz_min);
  const relMin = Number(thresholds?.relative_delta_min);
  const ciReq = thresholds?.ci_excludes_zero_required;
  const enumList = thresholds?.decision_enum;
  if (
    !Number.isFinite(minPairs) ||
    !Number.isFinite(dzMin) ||
    !Number.isFinite(relMin) ||
    typeof ciReq !== 'boolean' ||
    !Array.isArray(enumList)
  ) {
    throw new Error('preregistration thresholds missing required numeric/enum fields');
  }

  let label;
  let zeroControlFallbackUsed = false;
  if (stats.n_complete_pairs < minPairs) {
    label = 'insufficient_evidence';
  } else if (
    stats.ci95_low === null ||
    stats.ci95_high === null ||
    stats.paired_mean_delta === null ||
    stats.control_mean === null
  ) {
    label = 'null';
  } else {
    const ciOk = ciReq ? stats.ci95_low > 0.0 || stats.ci95_high < 0.0 : true;
    if (!ciOk) {
      label = 'null';
    } else {
      const dzOk = stats.paired_dz !== null && Math.abs(stats.paired_dz) >= dzMin;
      let relOk;
      if (stats.control_mean === 0.0 && allowZeroFallback) {
        zeroControlFallbackUsed = true;
        relOk = Math.abs(stats.paired_mean_delta) > 0.0;
      } else {
        relOk = Math.abs(stats.paired_mean_delta) >= relMin * Math.abs(stats.control_mean);
      }
      if (!dzOk || !relOk) {
        label = 'between_detectable_below_materiality';
      } else {
        label = stats.paired_mean_delta > 0.0 ? 'material_positive' : 'material_negative_opposite';
      }
    }
  }
  if (!enumList.includes(label)) {
    throw new Error(`decision '${label}' not declared in preregistration.thresholds.decision_enum`);
  }
  return { decision: label, zero_control_mean_fallback_used: zeroControlFallbackUsed };
}

function summarizeRunRules(runMeta, runSeries, finalRec, prereg) {
  const durationTicks = Number(prereg.duration.duration_ticks);
  const warmupTicks = Number(prereg.duration.warmup_ticks);
  const [tailLo, tailHi] = prereg.duration.tail_window_ticks.map(Number);
  const h1Spec = prereg.hypotheses.find((h) => h.id === 'H1');
  const minAgentsAtT = Number(h1Spec?.requires_min_agent_count_at_T ?? 2);

  const fin = finalRec?.final_observables ?? {};
  const finalN = Number(fin['live.agent_count'] ?? 0);

  let firstExtinctTick = null;
  for (const s of runSeries) {
    const n = Number(s['live.agent_count']);
    if (n === 0) {
      firstExtinctTick = Number(s.tick);
      break;
    }
  }
  if (firstExtinctTick === null && finalN === 0) {
    firstExtinctTick = durationTicks;
  }

  let minTailAgents = null;
  let minTailTick = tailHi;
  for (const s of runSeries) {
    const tk = Number(s.tick);
    if (tk >= tailLo && tk <= tailHi) {
      const n = Number(s['live.agent_count']);
      if (minTailAgents === null || n < minTailAgents) {
        minTailAgents = n;
        minTailTick = tk;
      }
    }
  }
  if (minTailAgents === null || finalN < minTailAgents) {
    minTailAgents = finalN;
    minTailTick = tailHi;
  }

  const mw = Number(fin['live.trait_mass_mean_west'] ?? 0);
  const me = Number(fin['live.trait_mass_mean_east'] ?? 0);
  let emptyHalf = null;
  if (mw === 0 && me === 0) emptyHalf = 'both';
  else if (mw === 0) emptyHalf = 'west';
  else if (me === 0) emptyHalf = 'east';

  const tailVarVals = runSeries
    .filter((s) => Number(s.tick) >= tailLo && Number(s.tick) <= tailHi)
    .map((s) => Number(s['live.trait_mass_var']));
  const postWarmupCropVals = runSeries
    .filter((s) => Number(s.tick) > warmupTicks)
    .map((s) => Number(s['live.standing_crop']));

  const varFinal = Number(fin['live.trait_mass_var'] ?? 0);
  return {
    seed: Number(runMeta.seed),
    arm: runMeta.arm,
    completed: runMeta.status === 'completed',
    failedTick: runMeta.failed_tick ? Number(runMeta.failed_tick) : null,
    failedReason: runMeta.failed_reason ?? '',
    extinctTick: firstExtinctTick,
    finalN,
    singletonAtT: firstExtinctTick === null && finalN < minAgentsAtT,
    singletonInTail: firstExtinctTick === null && minTailAgents < minAgentsAtT,
    minTailTick,
    minTailAgents,
    emptyHalf,
    fin,
    varFinal,
    besselVarFinal: firstExtinctTick === null && finalN >= minAgentsAtT ? (varFinal * finalN) / (finalN - 1) : null,
    tailVarMean: tailVarVals.length > 0 ? tailVarVals.reduce((a, b) => a + b, 0) / tailVarVals.length : null,
    iqrFinal: Number(fin['live.trait_mass_p90'] ?? 0) - Number(fin['live.trait_mass_p10'] ?? 0),
    s3Final: me - mw,
    birthsSum: runSeries.reduce((acc, s) => acc + Number(s['live.births'] ?? 0), 0),
    deathsSum: runSeries.reduce((acc, s) => acc + Number(s['live.deaths'] ?? 0), 0),
    cropPwMean: postWarmupCropVals.length > 0
      ? postWarmupCropVals.reduce((a, b) => a + b, 0) / postWarmupCropVals.length
      : null,
  };
}

export function evaluateE3Dataset({ runs, series, finalObservables, prereg }) {
  const executionOrder = prereg.seeds.execution_order.map(Number);
  const nRequested = executionOrder.length;
  const durationTicks = Number(prereg.duration.duration_ticks);
  const thresholds = prereg.thresholds;

  const seriesByKey = new Map();
  for (const s of series) {
    const key = `${Number(s.seed)}::${s.arm}`;
    if (!seriesByKey.has(key)) seriesByKey.set(key, []);
    seriesByKey.get(key).push(s);
  }
  for (const arr of seriesByKey.values()) {
    arr.sort((a, b) => Number(a.tick) - Number(b.tick));
  }

  const finalByKey = new Map();
  for (const f of finalObservables) {
    finalByKey.set(`${Number(f.seed)}::${f.arm}`, f);
  }

  const summaryByKey = new Map();
  for (const r of runs) {
    const key = `${Number(r.seed)}::${r.arm}`;
    summaryByKey.set(
      key,
      summarizeRunRules(r, seriesByKey.get(key) ?? [], finalByKey.get(key), prereg),
    );
  }

  const agentCountsByArm = {
    control_uniform: [],
    control_uniform_scarcity: [],
    treatment_gradient: [],
  };
  const extinctionTable = [];
  const singletonTable = [];
  const emptyHalfTable = [];
  const perArmEmptyHalfCounts = {
    control_uniform: 0,
    control_uniform_scarcity: 0,
    treatment_gradient: 0,
  };
  const failedRuns = [];

  for (const seed of executionOrder) {
    for (const arm of ARM_ORDER) {
      const sum = summaryByKey.get(`${seed}::${arm}`);
      if (!sum) throw new Error(`missing run summary for seed=${seed}, arm=${arm}`);
      if (!sum.completed) {
        failedRuns.push({
          seed,
          arm,
          tick: sum.failedTick ?? durationTicks,
          reason: sum.failedReason,
        });
      }
      agentCountsByArm[arm].push({ seed, agent_count: sum.finalN });
      if (sum.extinctTick !== null) {
        extinctionTable.push({ seed, arm, first_extinct_tick: sum.extinctTick });
      } else {
        if (sum.singletonAtT) {
          singletonTable.push({
            seed,
            arm,
            tick: durationTicks,
            agent_count: sum.finalN,
            scope: 'final_T',
          });
        } else if (sum.singletonInTail) {
          singletonTable.push({
            seed,
            arm,
            tick: sum.minTailTick,
            agent_count: sum.minTailAgents,
            scope: 'tail_window',
          });
        }
        if (sum.emptyHalf !== null) {
          emptyHalfTable.push({ seed, arm, tick: durationTicks, empty_half: sum.emptyHalf });
          perArmEmptyHalfCounts[arm] += 1;
        }
      }
    }
  }

  const buildReport = (rows, extDrop, singleDrop, ehDrop, decided, opts = {}) => {
    const cs = rows.map((r) => r.control);
    const ts = rows.map((r) => r.treatment);
    const st = pairedStats(cs, ts, nRequested);
    const dec = decided
      ? classifyE3Decision(st, thresholds, opts)
      : { decision: null, zero_control_mean_fallback_used: false };
    return {
      ...st,
      n_extinct_pairs_dropped: extDrop,
      n_singleton_pairs_dropped: singleDrop,
      n_empty_half_pairs_dropped_s3: ehDrop,
      decision: dec.decision,
      zero_control_mean_fallback_used: dec.zero_control_mean_fallback_used,
      per_seed_delta_table: rows,
    };
  };

  const contrasts = {};
  for (const c of CONTRAST_DEFS) {
    let extDrop = 0;
    let singleTDrop = 0;
    let singleTailDrop = 0;
    let emptyHalfDrop = 0;

    const rows = {
      h1: [],
      bessel: [],
      tail: [],
      s1: [],
      s2: [],
      s3: [],
      s4: [],
      s5: [],
      s6Life: [],
      s6Births: [],
      s6Deaths: [],
      i1: [],
      m1Final: [],
      m1Pw: [],
    };

    const pushRow = (arr, seed, cv, tv) => arr.push({ seed, control: cv, treatment: tv, delta: tv - cv });

    for (const seed of executionOrder) {
      const ctrl = summaryByKey.get(`${seed}::${c.controlArm}`);
      const treat = summaryByKey.get(`${seed}::${c.treatmentArm}`);
      if (!ctrl.completed || !treat.completed) continue;

      pushRow(rows.s4, seed, Number(ctrl.fin['live.shannon'] ?? 0), Number(treat.fin['live.shannon'] ?? 0));
      pushRow(rows.s5, seed, Number(ctrl.fin['live.archive_coverage'] ?? 0), Number(treat.fin['live.archive_coverage'] ?? 0));
      pushRow(rows.s6Life, seed, Number(ctrl.fin['live.mean_lifespan'] ?? 0), Number(treat.fin['live.mean_lifespan'] ?? 0));
      pushRow(rows.s6Births, seed, ctrl.birthsSum, treat.birthsSum);
      pushRow(rows.s6Deaths, seed, ctrl.deathsSum, treat.deathsSum);
      pushRow(rows.i1, seed, Number(ctrl.fin['live.closed_eu_total'] ?? 0), Number(treat.fin['live.closed_eu_total'] ?? 0));
      pushRow(rows.m1Final, seed, Number(ctrl.fin['live.standing_crop'] ?? 0), Number(treat.fin['live.standing_crop'] ?? 0));
      pushRow(rows.m1Pw, seed, ctrl.cropPwMean ?? 0, treat.cropPwMean ?? 0);

      if (ctrl.extinctTick !== null || treat.extinctTick !== null) {
        extDrop += 1;
        continue;
      }

      pushRow(rows.s2, seed, Number(ctrl.fin['live.trait_mass_mean'] ?? 0), Number(treat.fin['live.trait_mass_mean'] ?? 0));

      if (ctrl.singletonAtT || treat.singletonAtT) {
        singleTDrop += 1;
        singleTailDrop += 1;
      } else {
        pushRow(rows.h1, seed, ctrl.varFinal, treat.varFinal);
        pushRow(rows.bessel, seed, ctrl.besselVarFinal, treat.besselVarFinal);
        pushRow(rows.s1, seed, ctrl.iqrFinal, treat.iqrFinal);
        if (ctrl.singletonInTail || treat.singletonInTail) {
          singleTailDrop += 1;
        } else {
          pushRow(rows.tail, seed, ctrl.tailVarMean, treat.tailVarMean);
        }
      }

      if (ctrl.emptyHalf !== null || treat.emptyHalf !== null) {
        emptyHalfDrop += 1;
      } else {
        pushRow(rows.s3, seed, ctrl.s3Final, treat.s3Final);
      }
    }

    contrasts[c.name] = {
      H1: buildReport(rows.h1, extDrop, singleTDrop, 0, true),
      H1_bessel_sensitivity: buildReport(rows.bessel, extDrop, singleTDrop, 0, true),
      H1_tail_window: buildReport(rows.tail, extDrop, singleTailDrop, 0, true),
      S1: buildReport(rows.s1, extDrop, singleTDrop, 0, true),
      S2: buildReport(rows.s2, extDrop, 0, 0, true),
      S3: buildReport(rows.s3, extDrop, 0, emptyHalfDrop, true, { allowZeroControlFallback: true }),
      S4_structural_zero: buildReport(rows.s4, 0, 0, 0, false),
      S5_structural_zero: buildReport(rows.s5, 0, 0, 0, false),
      S6: {
        mean_lifespan: buildReport(rows.s6Life, 0, 0, 0, true),
        births_sum: buildReport(rows.s6Births, 0, 0, 0, true),
        deaths_sum: buildReport(rows.s6Deaths, 0, 0, 0, true),
      },
      I1_closed_eu_total: buildReport(rows.i1, 0, 0, 0, false),
      M1_standing_crop_final: buildReport(rows.m1Final, 0, 0, 0, false),
      M1_standing_crop_post_warmup: buildReport(rows.m1Pw, 0, 0, 0, false),
    };
  }

  const additive = [];
  for (const seed of executionOrder) {
    const u = summaryByKey.get(`${seed}::control_uniform`);
    const s = summaryByKey.get(`${seed}::control_uniform_scarcity`);
    const t = summaryByKey.get(`${seed}::treatment_gradient`);
    if (
      [u, s, t].every(
        (arm) => arm.completed && arm.extinctTick === null && !arm.singletonAtT,
      )
    ) {
      const dTU = t.varFinal - u.varFinal;
      const dTS = t.varFinal - s.varFinal;
      const dSU = s.varFinal - u.varFinal;
      additive.push({
        seed,
        control_uniform: u.varFinal,
        control_uniform_scarcity: s.varFinal,
        treatment_gradient: t.varFinal,
        delta_t_minus_u: dTU,
        delta_t_minus_s: dTS,
        delta_s_minus_u: dSU,
        residual: dTU - (dTS + dSU),
      });
    }
  }

  const primaryName = 'treatment_gradient - control_uniform_scarcity';
  return {
    primary_decision: contrasts[primaryName].H1.decision,
    contrasts,
    agent_count_at_T_by_arm: agentCountsByArm,
    additive_decomposition_table_on_all_three_valid_seeds: additive,
    extinction_table_by_arm: extinctionTable,
    singleton_table_by_arm: singletonTable,
    empty_half_table_by_arm: emptyHalfTable,
    per_arm_empty_half_counts: perArmEmptyHalfCounts,
    failed_runs: failedRuns,
  };
}

function approxEq(a, b, tol = 1e-9) {
  if (a === null && b === null) return true;
  if (typeof a !== 'number' || typeof b !== 'number') return false;
  if (Number.isNaN(a) && Number.isNaN(b)) return true;
  return Math.abs(a - b) <= tol * Math.max(1, Math.abs(a), Math.abs(b));
}

function compareReport(label, observed, expected, fail) {
  const exactFields = [
    'n_requested',
    'n_complete_pairs',
    'n_extinct_pairs_dropped',
    'n_singleton_pairs_dropped',
    'n_empty_half_pairs_dropped_s3',
    'decision',
    'zero_control_mean_fallback_used',
  ];
  for (const k of exactFields) {
    if ((observed?.[k] ?? null) !== (expected?.[k] ?? null)) {
      fail(`${label}.${k}: expected ${JSON.stringify(expected[k])}, observed ${JSON.stringify(observed?.[k])}`);
    }
  }
  const floatFields = [
    'control_mean',
    'treatment_mean',
    'paired_mean_delta',
    'paired_sd',
    'paired_se',
    'ci95_low',
    'ci95_high',
    'paired_dz',
    'median_of_per_seed_deltas',
    'between_seed_variance_of_deltas',
  ];
  for (const k of floatFields) {
    if (!approxEq(observed?.[k] ?? null, expected?.[k] ?? null)) {
      fail(`${label}.${k}: expected ${expected[k]}, observed ${observed?.[k]}`);
    }
  }
  const obsRows = observed?.per_seed_delta_table ?? [];
  const expRows = expected.per_seed_delta_table;
  if (obsRows.length !== expRows.length) {
    fail(`${label}.per_seed_delta_table length mismatch: ${obsRows.length} vs ${expRows.length}`);
  } else {
    for (let i = 0; i < expRows.length; i += 1) {
      if (
        Number(obsRows[i].seed) !== Number(expRows[i].seed) ||
        !approxEq(obsRows[i].control, expRows[i].control) ||
        !approxEq(obsRows[i].treatment, expRows[i].treatment) ||
        !approxEq(obsRows[i].delta, expRows[i].delta)
      ) {
        fail(`${label}.per_seed_delta_table[${i}] mismatch`);
      }
    }
  }
}

function resolveManifestGitRef(recordedCommit, notes) {
  const preregRel = `${committedManifestDir}/${PREREG_FILE}`;
  if (recordedCommit && gitCommitHasBlob(recordedCommit, preregRel)) {
    return recordedCommit;
  }
  notes.push(
    `manifest comparison fell back to recorded provenance.manifests.sha256_as_committed because recorded commit '${recordedCommit ?? '<missing>'}' is not available locally with committed E3 manifests`,
  );
  return null;
}

function resolveSmokeDirOnDisk(smokeDirField) {
  if (typeof smokeDirField !== 'string' || smokeDirField.trim().length === 0) {
    return null;
  }
  const basePath = isAbsolute(smokeDirField)
    ? resolve(smokeDirField)
    : resolve(repoRoot, smokeDirField);
  for (const cand of [basePath, join(basePath, 'smoke')]) {
    if (existsSync(join(cand, 'provenance.json')) && existsSync(join(cand, 'smoke-report.json'))) {
      return cand;
    }
  }
  return null;
}

function readSmokeShaInfo(smokeDirField) {
  const resolved = resolveSmokeDirOnDisk(smokeDirField);
  if (!resolved) {
    throw new Error(`cannot resolve host smoke directory: ${smokeDirField}`);
  }
  const provBytes = readFileSync(join(resolved, 'provenance.json'));
  const reportBytes = readFileSync(join(resolved, 'smoke-report.json'));
  const smokeProv = JSON.parse(provBytes.toString('utf8'));
  return {
    resolvedDir: resolved,
    provenance_sha256: sha256(provBytes),
    smoke_report_sha256: sha256(reportBytes),
    git_commit: smokeProv.build?.git_commit ?? null,
    host_os: smokeProv.host?.os ?? null,
    host_arch: smokeProv.host?.arch ?? null,
    profile: smokeProv.build?.profile ?? null,
    world_identity_observed: smokeProv.world_identity_observed ?? null,
  };
}

function verifyChecksumManifest(dir, requiredEntries, fail, labelPrefix) {
  const checksumPath = join(dir, 'checksums.sha256');
  const covered = new Map();
  if (!existsSync(checksumPath)) {
    fail(`${labelPrefix}: missing checksums.sha256`);
    return covered;
  }
  const lines = readFileSync(checksumPath, 'utf8')
    .replace(/\r\n/g, '\n')
    .split('\n')
    .filter((l) => l.length > 0);
  if (lines.length === 0) {
    fail(`${labelPrefix}: checksums.sha256 is empty`);
    return covered;
  }
  for (const line of lines) {
    const m = /^([0-9a-f]{64}) {2}(.+)$/.exec(line);
    if (!m) {
      fail(`${labelPrefix}: malformed checksums.sha256 line: '${line}'`);
      continue;
    }
    const [, expectedHex, rel] = m;
    covered.set(rel, expectedHex);
    const target = join(dir, rel);
    if (!existsSync(target)) {
      fail(`${labelPrefix}: checksums.sha256 references missing file: ${rel}`);
    } else {
      const actualHex = sha256(readFileSync(target));
      if (actualHex !== expectedHex) {
        fail(`${labelPrefix}: SHA-256 mismatch for ${rel}: expected ${expectedHex}, got ${actualHex}`);
      }
    }
  }
  for (const req of requiredEntries) {
    if (!covered.has(req)) {
      fail(`${labelPrefix}: checksums.sha256 does not cover required file: ${req}`);
    }
  }
  return covered;
}

function verifySmokeReferenceOnDisk(prov, hostDiffers, commitDiffers, sameWid, fail) {
  const sr = prov.smoke_reference;
  if (!sr || typeof sr !== 'object') {
    if (hostDiffers || commitDiffers) {
      const hostTag = `${prov.host?.os}/${prov.host?.arch}`;
      fail(`provenance.smoke_reference is required when host (${hostTag}) or commit differs from baseline smoke`);
    }
    return;
  }
  if (sr.wp2_passed !== true) fail('smoke_reference.wp2_passed is not true');
  if (sr.profile !== 'release') fail(`smoke_reference.profile must be release, got ${sr.profile}`);
  if (hostDiffers || commitDiffers) {
    if (sr.host_os !== prov.host?.os || sr.host_arch !== prov.host?.arch) {
      fail('smoke_reference host disagrees with confirmation provenance.host');
    }
    if (sr.git_commit !== prov.build?.git_commit) {
      fail('smoke_reference.git_commit disagrees with confirmation provenance.build.git_commit');
    }
  }

  const smokeDir = resolveSmokeDirOnDisk(sr.smoke_dir);
  if (!smokeDir) {
    fail(`smoke_reference.smoke_dir does not contain provenance.json + smoke-report.json: ${sr.smoke_dir}`);
    return;
  }
  const smokeCovered = verifyChecksumManifest(
    smokeDir,
    ['provenance.json', 'smoke-report.json'],
    fail,
    'smoke_reference',
  );
  const actualProvSha = sha256(readFileSync(join(smokeDir, 'provenance.json')));
  const actualReportSha = sha256(readFileSync(join(smokeDir, 'smoke-report.json')));
  if (sr.provenance_sha256 !== actualProvSha) {
    fail(
      `smoke_reference.provenance_sha256 mismatch: expected ${actualProvSha}, got ${sr.provenance_sha256}`,
    );
  }
  if (sr.smoke_report_sha256 !== actualReportSha) {
    fail(
      `smoke_reference.smoke_report_sha256 mismatch: expected ${actualReportSha}, got ${sr.smoke_report_sha256}`,
    );
  }
  if (smokeCovered.has('provenance.json') && sr.provenance_sha256 !== smokeCovered.get('provenance.json')) {
    fail('smoke_reference.provenance_sha256 disagrees with host smoke checksums.sha256');
  }
  if (smokeCovered.has('smoke-report.json') && sr.smoke_report_sha256 !== smokeCovered.get('smoke-report.json')) {
    fail('smoke_reference.smoke_report_sha256 disagrees with host smoke checksums.sha256');
  }

  const smokeProv = JSON.parse(readFileSync(join(smokeDir, 'provenance.json'), 'utf8'));
  const smokeReport = JSON.parse(readFileSync(join(smokeDir, 'smoke-report.json'), 'utf8'));
  if (smokeProv.build?.profile !== 'release') {
    fail(`host smoke provenance.build.profile must be 'release', got ${smokeProv.build?.profile}`);
  }
  if (smokeProv.host?.os !== sr.host_os || smokeProv.host?.arch !== sr.host_arch) {
    fail('host smoke provenance.host disagrees with smoke_reference host');
  }
  if (smokeProv.build?.git_commit !== sr.git_commit) {
    fail('host smoke provenance.build.git_commit disagrees with smoke_reference.git_commit');
  }
  if (!sameWid(smokeProv.world_identity_observed, sr.world_identity_observed)) {
    fail('host smoke world_identity_observed disagrees with smoke_reference.world_identity_observed');
  }
  if (prov.world_identity && !sameWid(prov.world_identity, smokeProv.world_identity_observed)) {
    fail('host smoke world_identity_observed disagrees with confirmation provenance.world_identity');
  }

  const wp2Checks = [
    ['wp2_a_resource_field.passed', smokeReport.wp2_a_resource_field?.passed === true],
    ['wp2_b_standing_crop_intervention.passed', smokeReport.wp2_b_standing_crop_intervention?.passed === true],
    ['wp2_c_detritus_before_3000.passed', smokeReport.wp2_c_detritus_before_3000?.passed === true],
    ['all_observables_emitted_and_finite', smokeReport.all_observables_emitted_and_finite === true],
    ['integrity.world_identity_identical', smokeReport.integrity?.world_identity_identical === true],
    ['wp3_schedule_lock.within_budget', smokeReport.wp3_schedule_lock?.within_budget === true],
  ];
  for (const [gateName, passed] of wp2Checks) {
    if (!passed) {
      fail(`host smoke smoke-report.json gate failed: ${gateName}`);
    }
  }
}

function verifyArrayTable(label, observed, expected, compareRow, fail) {
  if (!Array.isArray(observed)) {
    fail(`${label} must be an array`);
    return;
  }
  if (observed.length !== expected.length) {
    fail(`${label} length ${observed.length} != ${expected.length}`);
    return;
  }
  for (let i = 0; i < expected.length; i += 1) {
    if (!compareRow(observed[i], expected[i])) {
      fail(`${label}[${i}] mismatch: expected ${JSON.stringify(expected[i])}, observed ${JSON.stringify(observed[i])}`);
    }
  }
}

function hasExactArmKeys(obj) {
  if (!obj || typeof obj !== 'object' || Array.isArray(obj)) return false;
  const keys = Object.keys(obj).sort();
  const expected = [...ARM_ORDER].sort();
  return keys.length === expected.length && keys.every((k, i) => k === expected[i]);
}

export function verifyArtifactDir(dir, opts = {}) {
  const failures = [];
  const notes = [];
  const fail = (m) => failures.push(m);

  if (!existsSync(dir)) {
    fail(`artifact directory does not exist: ${dir}`);
    return { ok: false, failures, notes };
  }
  const entries = readdirSync(dir);
  if (entries.length === 0) {
    fail(`artifact directory is empty: ${dir}`);
    return { ok: false, failures, notes };
  }

  const requiredFiles = [
    'checksums.sha256',
    'provenance.json',
    'runs.csv',
    'series.csv',
    'final-observables.json',
    'effects.json',
    'summary.md',
  ];
  for (const f of requiredFiles) {
    if (!existsSync(join(dir, f))) {
      fail(`missing required file: ${f}`);
    }
  }
  const checksumPath = join(dir, 'checksums.sha256');
  const covered = new Set();
  if (existsSync(checksumPath)) {
    const lines = readFileSync(checksumPath, 'utf8')
      .replace(/\r\n/g, '\n')
      .split('\n')
      .filter((l) => l.length > 0);
    if (lines.length === 0) fail('checksums.sha256 is empty');
    for (const line of lines) {
      const m = /^([0-9a-f]{64}) {2}(.+)$/.exec(line);
      if (!m) {
        fail(`malformed checksums.sha256 line: '${line}'`);
        continue;
      }
      const [, expectedHex, rel] = m;
      covered.add(rel);
      const target = join(dir, rel);
      if (!existsSync(target)) {
        fail(`checksums.sha256 references missing file: ${rel}`);
      } else {
        const actualHex = sha256(readFileSync(target));
        if (actualHex !== expectedHex) {
          fail(`SHA-256 mismatch for ${rel}: expected ${expectedHex}, got ${actualHex}`);
        }
      }
    }
  }

  // Check coverage over all deterministic top-level and manifest files.
  const ignoredUncovered = new Set(['checksums.sha256', 'timing.json', 'manifests']);
  for (const name of readdirSync(dir)) {
    if (!ignoredUncovered.has(name) && !covered.has(name)) {
      fail(`file uncovered by checksums.sha256: ${name}`);
    }
  }

  let prov = null;
  if (existsSync(join(dir, 'provenance.json'))) {
    try {
      prov = JSON.parse(readFileSync(join(dir, 'provenance.json'), 'utf8'));
    } catch (e) {
      fail(`invalid provenance.json: ${e.message}`);
    }
  }
  const manifestDir = join(dir, 'manifests');
  const manifestGitRef = opts.preregOverride || opts.preregPath
    ? 'HEAD'
    : resolveManifestGitRef(prov?.build?.git_commit, notes);
  const useRecordedShaFallback = !(opts.preregOverride || opts.preregPath) && manifestGitRef === null;
  let prereg = opts.preregOverride ?? null;
  if (!prereg) {
    if (opts.preregPath) {
      prereg = loadCommittedPrereg(opts.preregPath, 'HEAD');
    } else if (!useRecordedShaFallback) {
      prereg = loadCommittedPrereg(null, manifestGitRef);
    } else {
      const artifactPreregPath = join(manifestDir, PREREG_FILE);
      if (!existsSync(artifactPreregPath)) {
        fail(`missing manifest copy: manifests/${PREREG_FILE}`);
        return { ok: false, failures, notes };
      }
      try {
        prereg = JSON.parse(readFileSync(artifactPreregPath, 'utf8'));
      } catch (e) {
        fail(`invalid manifests/${PREREG_FILE}: ${e.message}`);
        return { ok: false, failures, notes };
      }
    }
  }

  if (!existsSync(manifestDir)) {
    fail('missing manifests/ directory');
  } else {
    const expectedManifests = [
      PREREG_FILE,
      prereg.manifests.control,
      prereg.manifests.control_uniform_scarcity,
      prereg.manifests.treatment,
    ];
    for (const mf of expectedManifests) {
      const copyPath = join(manifestDir, mf);
      if (!existsSync(copyPath)) {
        fail(`missing manifest copy: manifests/${mf}`);
        continue;
      }
      if (!covered.has(`manifests/${mf}`)) {
        fail(`manifest copy not covered by checksums.sha256: manifests/${mf}`);
      }
      if (!opts.skipCommittedManifestCheck) {
        if (useRecordedShaFallback) {
          const recordedSha = prov?.manifests?.sha256_as_committed?.[mf];
          if (typeof recordedSha !== 'string' || !/^[0-9a-f]{64}$/.test(recordedSha)) {
            fail(
              `provenance.manifests.sha256_as_committed missing valid SHA-256 for ${mf} while recorded commit '${prov?.build?.git_commit ?? '<missing>'}' is unavailable locally`,
            );
          } else {
            const actualNormSha = sha256NormalizedLf(readFileSync(copyPath));
            if (actualNormSha !== recordedSha) {
              fail(
                `manifests/${mf} SHA-256 (${actualNormSha}) differs from provenance.manifests.sha256_as_committed (${recordedSha})`,
              );
            }
          }
        } else {
          const committed = readCommittedBytesAt(mf, manifestGitRef);
          if (!committed) {
            fail(`cannot read committed manifest ${mf} at ${manifestGitRef}`);
          } else if (sha256(readFileSync(copyPath)) !== sha256(committed)) {
            fail(`manifests/${mf} differs from committed fixture in repo at ${manifestGitRef}`);
          }
        }
      }
    }
  }

  if (failures.length > 0 && (!prov || !existsSync(join(dir, 'effects.json')))) {
    return { ok: false, failures, notes };
  }

  const runs = parseCsv(readFileSync(join(dir, 'runs.csv'), 'utf8'));
  const series = parseCsv(readFileSync(join(dir, 'series.csv'), 'utf8'));
  const finals = JSON.parse(readFileSync(join(dir, 'final-observables.json'), 'utf8'));
  const effects = JSON.parse(readFileSync(join(dir, 'effects.json'), 'utf8'));

  // Provenance integrity checks.
  if (prov.stopped_early !== false) fail(`provenance.stopped_early must be false, got ${prov.stopped_early}`);
  if (prov.build?.profile !== 'release') fail(`provenance.build.profile must be 'release', got ${prov.build?.profile}`);
  if (!Array.isArray(prov.build?.git_dirty_paths)) {
    fail('provenance.build.git_dirty_paths must be an array of dirty paths (not missing/bare boolean)');
  } else {
    for (const dirtyPath of prov.build.git_dirty_paths) {
      if (!isAllowedDirtyPath(dirtyPath)) {
        fail(`disallowed dirty working tree path in provenance.build.git_dirty_paths: ${JSON.stringify(dirtyPath)}`);
      }
    }
  }
  if (!prov.build?.git_commit) fail('provenance.build.git_commit is missing');
  if (!prov.build?.rustc) fail('provenance.build.rustc is missing');
  if (!prov.build?.binary_sha256) fail('provenance.build.binary_sha256 is missing');
  if (!prov.host?.os || !prov.host?.arch) fail('provenance.host.{os,arch} is missing');
  if (prov.terrain_source !== 'generated') {
    fail(`provenance.terrain_source must be 'generated', got ${prov.terrain_source}`);
  }
  if (prov.world_artifact_prerun_check?.anima_world_artifact_unset !== true ||
      prov.world_artifact_prerun_check?.temp_artifact_absent !== true) {
    fail('provenance.world_artifact_prerun_check did not confirm unset env + absent temp artifact');
  }

  // World identity uniformity across provenance, smoke reference, runs.csv, and final-observables.json.
  const wid = prov.world_identity;
  const sameWid = (a, b) =>
    a && b &&
    Number(a.seed) === Number(b.seed) &&
    Number(a.generator_version) === Number(b.generator_version) &&
    Number(a.width) === Number(b.width) &&
    Number(a.height) === Number(b.height) &&
    Number(a.checksum) === Number(b.checksum);
  if (!wid || typeof wid.checksum !== 'number') {
    fail('provenance.world_identity is missing');
  } else {
    if (prov.smoke_reference?.world_identity_observed && !sameWid(wid, prov.smoke_reference.world_identity_observed)) {
      fail('smoke_reference.world_identity_observed disagrees with provenance.world_identity');
    }
    const perRun = prov.per_run_world_provenance ?? [];
    for (let i = 0; i < perRun.length; i += 1) {
      if (perRun[i].terrain_source !== 'generated' || !sameWid(wid, perRun[i].world_identity)) {
        fail(`per_run_world_provenance[${i}] mismatches provenance.world_identity`);
      }
    }
  }

  const hostTag = `${prov.host?.os}/${prov.host?.arch}`;
  const commitDiffers = prov.build?.git_commit !== prereg.wp2_smoke_calibration?.parent_commit;
  const hostDiffers = hostTag !== prereg.execution_host_rule?.smoke_host_observed;
  verifySmokeReferenceOnDisk(prov, hostDiffers, commitDiffers, sameWid, fail);

  // Verify run coverage (36 runs, exact execution_order, no smoke_seed).
  const expectedOrder = prereg.seeds.execution_order.map(Number);
  const smokeSeed = Number(prereg.seeds.smoke_seed);
  if (runs.length !== expectedOrder.length * ARM_ORDER.length) {
    fail(`runs.csv has ${runs.length} rows, expected ${expectedOrder.length * ARM_ORDER.length}`);
  }
  const seenKeys = new Set();
  for (const r of runs) {
    const seed = Number(r.seed);
    if (seed === smokeSeed) fail(`smoke_seed ${smokeSeed} leaked into runs.csv`);
    if (r.terrain_source !== 'generated') fail(`runs.csv seed=${seed} arm=${r.arm} terrain_source != generated`);
    if (wid && !sameWid(wid, {
      seed: r.world_seed,
      generator_version: r.world_gen_version,
      width: r.world_width,
      height: r.world_height,
      checksum: r.world_checksum,
    })) {
      fail(`runs.csv world_identity mismatch at seed=${seed} arm=${r.arm}`);
    }
    seenKeys.add(`${seed}::${r.arm}`);
  }
  for (const seed of expectedOrder) {
    for (const arm of ARM_ORDER) {
      if (!seenKeys.has(`${seed}::${arm}`)) {
        fail(`missing run row for seed=${seed} arm=${arm}`);
      }
    }
  }
  if (finals.length !== expectedOrder.length * ARM_ORDER.length) {
    fail(`final-observables.json has ${finals.length} entries, expected ${expectedOrder.length * ARM_ORDER.length}`);
  }
  for (const f of finals) {
    if (Number(f.seed) === smokeSeed) fail(`smoke_seed ${smokeSeed} leaked into final-observables.json`);
    if (f.terrain_source !== 'generated' || (wid && !sameWid(wid, f.world_identity))) {
      fail(`final-observables.json world provenance mismatch at seed=${f.seed} arm=${f.arm}`);
    }
  }

  // Recompute all statistics from raw artifacts and verify effects.json line by line.
  if (failures.length === 0 || opts.alwaysCheckAnalysis) {
    const recomputed = evaluateE3Dataset({ runs, series, finalObservables: finals, prereg });
    if (effects.primary_decision !== recomputed.primary_decision) {
      fail(`effects.primary_decision expected '${recomputed.primary_decision}', observed '${effects.primary_decision}'`);
    }
    for (const c of CONTRAST_DEFS) {
      const obsC = effects.contrasts?.[c.name];
      const expC = recomputed.contrasts[c.name];
      if (!obsC) {
        fail(`effects.json missing contrast '${c.name}'`);
        continue;
      }
      compareReport(`${c.name}.H1`, obsC.H1, expC.H1, fail);
      compareReport(`${c.name}.H1_bessel_sensitivity`, obsC.H1_bessel_sensitivity, expC.H1_bessel_sensitivity, fail);
      compareReport(`${c.name}.H1_tail_window`, obsC.H1_tail_window, expC.H1_tail_window, fail);
      compareReport(`${c.name}.S1`, obsC.S1, expC.S1, fail);
      compareReport(`${c.name}.S2`, obsC.S2, expC.S2, fail);
      compareReport(`${c.name}.S3`, obsC.S3, expC.S3, fail);
      compareReport(`${c.name}.S4`, obsC.S4_structural_zero, expC.S4_structural_zero, fail);
      compareReport(`${c.name}.S5`, obsC.S5_structural_zero, expC.S5_structural_zero, fail);
      compareReport(`${c.name}.S6.mean_lifespan`, obsC.S6?.mean_lifespan, expC.S6.mean_lifespan, fail);
      compareReport(`${c.name}.S6.births_sum`, obsC.S6?.births_sum, expC.S6.births_sum, fail);
      compareReport(`${c.name}.S6.deaths_sum`, obsC.S6?.deaths_sum, expC.S6.deaths_sum, fail);
      compareReport(`${c.name}.I1`, obsC.I1_closed_eu_total, expC.I1_closed_eu_total, fail);
      compareReport(`${c.name}.M1_final`, obsC.M1_standing_crop_final, expC.M1_standing_crop_final, fail);
      compareReport(`${c.name}.M1_post_warmup`, obsC.M1_standing_crop_post_warmup, expC.M1_standing_crop_post_warmup, fail);
    }

    if (!hasExactArmKeys(effects.agent_count_at_T_by_arm)) {
      fail('effects.agent_count_at_T_by_arm missing or has unexpected arm keys');
    } else {
      for (const arm of ARM_ORDER) {
        verifyArrayTable(
          `agent_count_at_T_by_arm.${arm}`,
          effects.agent_count_at_T_by_arm[arm],
          recomputed.agent_count_at_T_by_arm[arm],
          (o, e) => o && Number(o.seed) === e.seed && typeof o.agent_count === 'number' && approxEq(o.agent_count, e.agent_count),
          fail,
        );
      }
    }

    if (!hasExactArmKeys(effects.per_arm_empty_half_counts)) {
      fail('effects.per_arm_empty_half_counts missing or has unexpected arm keys');
    } else {
      for (const arm of ARM_ORDER) {
        const obsVal = effects.per_arm_empty_half_counts[arm];
        const expVal = recomputed.per_arm_empty_half_counts[arm];
        if (typeof obsVal !== 'number' || obsVal !== expVal) {
          fail(`per_arm_empty_half_counts.${arm}: expected ${expVal}, observed ${JSON.stringify(obsVal)}`);
        }
      }
    }

    verifyArrayTable(
      'additive_decomposition_table_on_all_three_valid_seeds',
      effects.additive_decomposition_table_on_all_three_valid_seeds,
      recomputed.additive_decomposition_table_on_all_three_valid_seeds,
      (o, e) =>
        o &&
        Number(o.seed) === e.seed &&
        approxEq(o.control_uniform, e.control_uniform) &&
        approxEq(o.control_uniform_scarcity, e.control_uniform_scarcity) &&
        approxEq(o.treatment_gradient, e.treatment_gradient) &&
        approxEq(o.delta_t_minus_u, e.delta_t_minus_u) &&
        approxEq(o.delta_t_minus_s, e.delta_t_minus_s) &&
        approxEq(o.delta_s_minus_u, e.delta_s_minus_u) &&
        approxEq(o.residual, e.residual),
      fail,
    );

    verifyArrayTable(
      'extinction_table_by_arm',
      effects.extinction_table_by_arm,
      recomputed.extinction_table_by_arm,
      (o, e) =>
        o &&
        Number(o.seed) === e.seed &&
        o.arm === e.arm &&
        typeof o.first_extinct_tick === 'number' &&
        o.first_extinct_tick === e.first_extinct_tick,
      fail,
    );

    verifyArrayTable(
      'singleton_table_by_arm',
      effects.singleton_table_by_arm,
      recomputed.singleton_table_by_arm,
      (o, e) =>
        o &&
        Number(o.seed) === e.seed &&
        o.arm === e.arm &&
        typeof o.tick === 'number' &&
        o.tick === e.tick &&
        typeof o.agent_count === 'number' &&
        approxEq(o.agent_count, e.agent_count) &&
        o.scope === e.scope,
      fail,
    );

    verifyArrayTable(
      'empty_half_table_by_arm',
      effects.empty_half_table_by_arm,
      recomputed.empty_half_table_by_arm,
      (o, e) =>
        o &&
        Number(o.seed) === e.seed &&
        o.arm === e.arm &&
        typeof o.tick === 'number' &&
        o.tick === e.tick &&
        o.empty_half === e.empty_half,
      fail,
    );

    verifyArrayTable(
      'failed_runs',
      effects.failed_runs,
      recomputed.failed_runs,
      (o, e) =>
        o &&
        Number(o.seed) === e.seed &&
        o.arm === e.arm &&
        typeof o.tick === 'number' &&
        o.tick === e.tick &&
        (o.reason ?? '') === (e.reason ?? ''),
      fail,
    );
  }

  return { ok: failures.length === 0, failures, notes };
}

// ---- Synthetic confirmation bundle helper for `--self-test` and tests ------------------------

export function writeSyntheticSmokeDir(dir, opts = {}) {
  mkdirSync(dir, { recursive: true });
  const commit = opts.commit ?? 'deadbeef';
  const hostOs = opts.hostOs ?? 'windows';
  const hostArch = opts.hostArch ?? 'x86_64';
  const wid = opts.wid ?? {
    seed: 1337,
    generator_version: 1,
    width: 256,
    height: 256,
    checksum: 3152323152,
  };
  const wp2 = opts.wp2Passed !== false;
  const provDoc = {
    kind: 'e3-smoke-provenance-EXCLUDED-FROM-ANALYSIS',
    build: { git_commit: commit, profile: opts.profile ?? 'release' },
    host: { os: hostOs, arch: hostArch },
    world_identity_observed: wid,
  };
  const reportDoc = {
    kind: 'e3-smoke-report-EXCLUDED-FROM-ANALYSIS',
    all_observables_emitted_and_finite: wp2,
    wp2_a_resource_field: { passed: wp2 },
    wp2_b_standing_crop_intervention: { passed: wp2 },
    wp2_c_detritus_before_3000: { passed: wp2 },
    integrity: { world_identity_identical: true },
    wp3_schedule_lock: { within_budget: true },
  };
  writeFileSync(join(dir, 'provenance.json'), `${JSON.stringify(provDoc, null, 2)}\n`);
  writeFileSync(join(dir, 'smoke-report.json'), `${JSON.stringify(reportDoc, null, 2)}\n`);
  const lines = ['provenance.json', 'smoke-report.json']
    .map((n) => `${sha256(readFileSync(join(dir, n)))}  ${n}\n`)
    .join('');
  writeFileSync(join(dir, 'checksums.sha256'), lines);
  return dir;
}

export function writeRecomputedBundleFiles(dir, prereg, { runs, series, finals, prov }, opts = {}) {
  mkdirSync(join(dir, 'manifests'), { recursive: true });
  const mFiles = [
    PREREG_FILE,
    prereg.manifests.control,
    prereg.manifests.control_uniform_scarcity,
    prereg.manifests.treatment,
  ];
  const manifestGitRef = opts.manifestGitRef ?? 'HEAD';
  const sha256AsCommitted = {};
  for (const mf of mFiles) {
    const bytes = readCommittedBytesAt(mf, manifestGitRef);
    if (!bytes) {
      throw new Error(`cannot read committed manifest ${mf} at ${manifestGitRef}`);
    }
    writeFileSync(join(dir, 'manifests', mf), bytes);
    sha256AsCommitted[mf] = sha256NormalizedLf(bytes);
  }
  const provOut = structuredClone(prov);
  if (!provOut.manifests?.sha256_as_committed) {
    provOut.manifests = {
      ...(provOut.manifests ?? {}),
      sha256_as_committed: sha256AsCommitted,
    };
  }

  const runHeaders = [
    'seed', 'arm', 'experiment_id', 'status', 'failed_tick', 'failed_reason',
    'final_checksum', 'duration_ticks', 'sample_period', 'sample_count',
    'terrain_source', 'world_seed', 'world_gen_version', 'world_width',
    'world_height', 'world_checksum', 'extinct_tick', 'final_agent_count', 'empty_half',
  ];
  const runsText = [
    runHeaders.join(','),
    ...runs.map((r) => runHeaders.map((h) => r[h] ?? '').join(',')),
  ].join('\n') + '\n';

  const seriesHeaders = [
    'seed', 'arm', 'tick', 'live.agent_count', 'live.trait_mass_var',
    'live.standing_crop', 'live.births', 'live.deaths', 'live.closed_eu_total',
    'live.shannon', 'live.archive_coverage',
  ];
  const seriesText = [
    seriesHeaders.join(','),
    ...series.map((r) => seriesHeaders.map((h) => r[h] ?? '').join(',')),
  ].join('\n') + '\n';

  const recomputed = evaluateE3Dataset({ runs, series, finalObservables: finals, prereg });
  const effectsDoc = {
    kind: 'e3-effects-and-decision',
    preregistration_id: prereg.preregistration_id,
    primary_contrast: 'treatment_gradient - control_uniform_scarcity',
    primary_decision: recomputed.primary_decision,
    thresholds_applied: {
      min_valid_pairs_for_decision: prereg.thresholds.min_valid_pairs_for_decision,
      dz_min: prereg.thresholds.dz_min,
      relative_delta_min: prereg.thresholds.relative_delta_min,
      ci_excludes_zero_required: prereg.thresholds.ci_excludes_zero_required,
      decision_enum: prereg.thresholds.decision_enum,
    },
    ...recomputed,
  };

  writeFileSync(join(dir, 'runs.csv'), runsText);
  writeFileSync(join(dir, 'series.csv'), seriesText);
  writeFileSync(join(dir, 'final-observables.json'), `${JSON.stringify(finals, null, 2)}\n`);
  writeFileSync(join(dir, 'effects.json'), `${JSON.stringify(effectsDoc, null, 2)}\n`);
  writeFileSync(join(dir, 'provenance.json'), `${JSON.stringify(provOut, null, 2)}\n`);
  writeFileSync(join(dir, 'summary.md'), '# Synthetic E3 Bundle\n');
  writeFileSync(join(dir, 'timing.json'), '{"wall_clock_seconds":1.0}\n');

  rehashBundle(dir, mFiles);
  return effectsDoc;
}

export function rehashBundle(dir, mFiles) {
  const names = [
    'effects.json',
    'final-observables.json',
    'provenance.json',
    'runs.csv',
    'series.csv',
    'summary.md',
    ...mFiles.map((m) => `manifests/${m}`),
  ].sort();
  const lines = names.map((n) => `${sha256(readFileSync(join(dir, n)))}  ${n}\n`).join('');
  writeFileSync(join(dir, 'checksums.sha256'), lines);
}

export function buildSyntheticE3Data(prereg, opts = {}) {
  const wid = {
    seed: 1337,
    generator_version: 1,
    width: 256,
    height: 256,
    checksum: 3152323152,
  };
  const extinctControlUniformSeeds = new Set(opts.extinctControlUniformSeeds ?? []);
  const executionOrder = prereg.seeds.execution_order.map(Number);
  const samplePeriod = Number(prereg.duration.sample_period);
  const durationTicks = Number(prereg.duration.duration_ticks);
  const tailLo = Number(prereg.duration.tail_window_ticks[0]);

  const runs = [];
  const series = [];
  const finals = [];
  const perRunWorld = [];

  executionOrder.forEach((seed, idx) => {
    for (const arm of ARM_ORDER) {
      const isExtinctU = arm === 'control_uniform' && extinctControlUniformSeeds.has(seed);
      const baseVar = arm === 'control_uniform' ? 3.0 : arm === 'control_uniform_scarcity' ? 4.0 + idx * 0.03 : 5.5 + idx * 0.08;
      const meanMass = arm === 'control_uniform' ? 9.5 : arm === 'control_uniform_scarcity' ? 10.0 : 10.5;
      const westMean = arm === 'treatment_gradient' ? 9.5 : meanMass;
      const eastMean = arm === 'treatment_gradient' ? 11.0 : meanMass;
      const crop = arm === 'control_uniform' ? 130000 : 64000;
      const agentCount = isExtinctU ? 0 : arm === 'treatment_gradient' ? 50 : 12;

      for (let tick = samplePeriod; tick <= durationTicks; tick += samplePeriod) {
        const sampleAgents = isExtinctU && tick >= tailLo ? 0 : agentCount;
        const sampleVar = sampleAgents === 0 ? 0 : baseVar;
        series.push({
          seed,
          arm,
          tick,
          'live.agent_count': sampleAgents,
          'live.trait_mass_var': sampleVar,
          'live.standing_crop': crop,
          'live.births': 10,
          'live.deaths': 9,
          'live.closed_eu_total': 165587.25,
          'live.shannon': 0,
          'live.archive_coverage': 0,
        });
      }

      runs.push({
        seed,
        arm,
        experiment_id: `e3::${arm}`,
        status: 'completed',
        failed_tick: '',
        failed_reason: '',
        final_checksum: 1000 + idx,
        duration_ticks: durationTicks,
        sample_period: samplePeriod,
        sample_count: Math.floor(durationTicks / samplePeriod),
        terrain_source: 'generated',
        world_seed: wid.seed,
        world_gen_version: wid.generator_version,
        world_width: wid.width,
        world_height: wid.height,
        world_checksum: wid.checksum,
        extinct_tick: isExtinctU ? tailLo : '',
        final_agent_count: agentCount,
        empty_half: '',
      });

      finals.push({
        seed,
        arm,
        status: 'completed',
        final_checksum: 1000 + idx,
        terrain_source: 'generated',
        world_identity: wid,
        final_observables: {
          'live.agent_count': agentCount,
          'live.trait_mass_var': isExtinctU ? 0 : baseVar,
          'live.trait_mass_mean': isExtinctU ? 0 : meanMass,
          'live.trait_mass_p10': isExtinctU ? 0 : meanMass - 1.0,
          'live.trait_mass_p90': isExtinctU ? 0 : meanMass + 1.0,
          'live.trait_mass_mean_west': isExtinctU ? 0 : westMean,
          'live.trait_mass_mean_east': isExtinctU ? 0 : eastMean,
          'live.mean_lifespan': 1200,
          'live.standing_crop': crop,
          'live.closed_eu_total': 165587.25,
          'live.shannon': 0,
          'live.archive_coverage': 0,
        },
      });

      perRunWorld.push({
        run_index: perRunWorld.length,
        seed,
        arm,
        terrain_source: 'generated',
        world_identity: wid,
      });
    }
  });

  const parentCommit = opts.gitCommit ?? prereg.wp2_smoke_calibration.parent_commit;
  const smokeDirArg = opts.smokeDir ?? 'artifacts/experiments/e3-productivity-gradient-body-mass/smoke';
  const smokeInfo = readSmokeShaInfo(smokeDirArg);
  const relSmoke = isAbsolute(smokeDirArg) ? relative(repoRoot, smokeInfo.resolvedDir) : smokeDirArg;
  const portableSmokeDir = !relSmoke.startsWith('..') && !isAbsolute(relSmoke)
    ? relSmoke.replace(/\\/g, '/')
    : smokeInfo.resolvedDir.replace(/\\/g, '/');

  const prov = {
    kind: 'e3-confirmation-provenance',
    mode: 'ensemble',
    stopped_early: false,
    stop_reason: null,
    terrain_source: 'generated',
    world_identity: wid,
    world_identity_all_runs_identical: true,
    per_run_world_provenance: perRunWorld,
    world_artifact_prerun_check: {
      anima_world_artifact_unset: true,
      temp_artifact_absent: true,
    },
    smoke_reference: {
      smoke_dir: portableSmokeDir,
      provenance_sha256: smokeInfo.provenance_sha256,
      smoke_report_sha256: smokeInfo.smoke_report_sha256,
      git_commit: smokeInfo.git_commit ?? parentCommit,
      host_os: smokeInfo.host_os ?? 'windows',
      host_arch: smokeInfo.host_arch ?? 'x86_64',
      profile: smokeInfo.profile ?? 'release',
      world_identity_observed: smokeInfo.world_identity_observed ?? wid,
      wp2_passed: true,
    },
    build: {
      git_commit: parentCommit,
      git_dirty_paths: opts.gitDirtyPaths ?? [],
      git_tree_dirty: (opts.gitDirtyPaths?.length ?? 0) > 0,
      profile: 'release',
      rustc: 'rustc 1.98.1',
      binary_sha256: '0'.repeat(64),
    },
    host: { os: smokeInfo.host_os ?? 'windows', arch: smokeInfo.host_arch ?? 'x86_64' },
  };
  return { runs, series, finals, prov };
}

function runSelfTest() {
  let bad = 0;
  const expect = (cond, label) => {
    if (!cond) {
      bad += 1;
      console.error(`SELF-TEST FAIL  ${label}`);
    } else {
      console.log(`self-test ok    ${label}`);
    }
  };

  const prereg = loadCommittedPrereg();
  const root = mkdtempSync(join(tmpdir(), 'e3-verify-selftest-'));
  const mFiles = [
    PREREG_FILE,
    prereg.manifests.control,
    prereg.manifests.control_uniform_scarcity,
    prereg.manifests.treatment,
  ];
  const editJsonAndRehash = (dir, filename, mutator) => {
    const target = join(dir, filename);
    const doc = JSON.parse(readFileSync(target, 'utf8'));
    mutator(doc);
    writeFileSync(target, `${JSON.stringify(doc, null, 2)}\n`);
    rehashBundle(dir, mFiles);
  };

  try {
    // 1. Positive control: valid 12-seed x 3-arm bundle passes.
    const validDir = join(root, 'valid');
    const validData = buildSyntheticE3Data(prereg);
    writeRecomputedBundleFiles(validDir, prereg, validData);
    const validRes = verifyArtifactDir(validDir);
    expect(validRes.ok && validRes.failures.length === 0, 'valid 12-seed x 3-arm bundle passes');

    // 2a. Negative control: raw tampered observable value fails checksum verification.
    const rawTamperDir = join(root, 'tamper_raw');
    const dTamperRaw = buildSyntheticE3Data(prereg);
    writeRecomputedBundleFiles(rawTamperDir, prereg, dTamperRaw);
    const rawFinals = JSON.parse(readFileSync(join(rawTamperDir, 'final-observables.json'), 'utf8'));
    rawFinals[0].final_observables['live.trait_mass_var'] += 5.0;
    writeFileSync(join(rawTamperDir, 'final-observables.json'), `${JSON.stringify(rawFinals, null, 2)}\n`);
    const resRawTamper = verifyArtifactDir(rawTamperDir);
    expect(!resRawTamper.ok && resRawTamper.failures.some((m) => m.includes('SHA-256 mismatch')), 'tampered value caught by SHA-256');

    // 2b. Negative control: re-hashed tampered value caught by independent stats recomputation.
    rehashBundle(rawTamperDir, mFiles);
    const resRehashTamper = verifyArtifactDir(rawTamperDir);
    expect(!resRehashTamper.ok && resRehashTamper.failures.some((m) => m.includes('.H1.')), 're-hashed tampered value caught by independent recomputation');

    // 3. Negative control: missing run (deleted row from runs.csv, re-hashed).
    const missingDir = join(root, 'missing_run');
    writeRecomputedBundleFiles(missingDir, prereg, buildSyntheticE3Data(prereg));
    const runLines = readFileSync(join(missingDir, 'runs.csv'), 'utf8').trimEnd().split('\n');
    writeFileSync(join(missingDir, 'runs.csv'), `${runLines.slice(0, -1).join('\n')}\n`);
    rehashBundle(missingDir, mFiles);
    const resMissing = verifyArtifactDir(missingDir);
    expect(!resMissing.ok && resMissing.failures.some((m) => m.includes('runs.csv has')), 'missing run detected');

    // 4. Negative control: world_identity mismatch on one run (re-hashed).
    const widDir = join(root, 'wid_mismatch');
    writeRecomputedBundleFiles(widDir, prereg, buildSyntheticE3Data(prereg));
    const runsBadWid = readFileSync(join(widDir, 'runs.csv'), 'utf8').replace(',3152323152,', ',999999999,');
    writeFileSync(join(widDir, 'runs.csv'), runsBadWid);
    rehashBundle(widDir, mFiles);
    const resWid = verifyArtifactDir(widDir);
    expect(!resWid.ok && resWid.failures.some((m) => m.includes('world_identity mismatch')), 'world_identity mismatch rejected');

    // 5. Negative control: threshold edited in place (dz_min raised in prereg -> decision mismatch).
    const editedPrereg = structuredClone(prereg);
    editedPrereg.thresholds.dz_min = 999.0;
    const resThresh = verifyArtifactDir(validDir, { preregOverride: editedPrereg });
    expect(!resThresh.ok && resThresh.failures.some((m) => m.includes('decision')), 'mutated threshold in prereg JSON catches stale decision');

    // 6. Negative control: extinction in control_uniform (3 of 12 seeds) excludes ONLY U contrasts.
    const extSeeds = prereg.seeds.execution_order.slice(0, 3);
    const extDir = join(root, 'extinct_u');
    const extData = buildSyntheticE3Data(prereg, { extinctControlUniformSeeds: extSeeds });
    const extEffects = writeRecomputedBundleFiles(extDir, prereg, extData);
    const resExt = verifyArtifactDir(extDir);
    const cTS = extEffects.contrasts['treatment_gradient - control_uniform_scarcity'].H1;
    const cTU = extEffects.contrasts['treatment_gradient - control_uniform'].H1;
    const cSU = extEffects.contrasts['control_uniform_scarcity - control_uniform'].H1;
    expect(
      resExt.ok &&
        cTS.n_complete_pairs === 12 &&
        cTS.decision === 'material_positive' &&
        cTU.n_complete_pairs === 9 &&
        cTU.n_extinct_pairs_dropped === 3 &&
        cTU.decision === 'insufficient_evidence' &&
        cSU.n_complete_pairs === 9 &&
        cSU.decision === 'insufficient_evidence' &&
        extEffects.additive_decomposition_table_on_all_three_valid_seeds.length === 9,
      'extinction in control_uniform excludes only contrasts containing U while keeping H1 (T-S) valid',
    );

    // 7a. R1 negative control: re-hashed tampered agent_count_at_T_by_arm, per_arm_empty_half_counts, failed_runs.
    const r1MediatorDir = join(root, 'r1_mediator');
    writeRecomputedBundleFiles(r1MediatorDir, prereg, buildSyntheticE3Data(prereg));
    editJsonAndRehash(r1MediatorDir, 'effects.json', (j) => {
      j.agent_count_at_T_by_arm.control_uniform_scarcity[0].agent_count = 999;
      j.per_arm_empty_half_counts.treatment_gradient = 5;
      j.failed_runs = [{ seed: 1, arm: 'control_uniform', tick: 1, reason: 'injected' }];
    });
    const resR1Med = verifyArtifactDir(r1MediatorDir);
    expect(
      !resR1Med.ok &&
        resR1Med.failures.some((m) => m.includes('agent_count_at_T_by_arm.control_uniform_scarcity')) &&
        resR1Med.failures.some((m) => m.includes('per_arm_empty_half_counts.treatment_gradient')) &&
        resR1Med.failures.some((m) => m.includes('failed_runs')),
      'R1: tampered agent_count_at_T_by_arm, per_arm_empty_half_counts, and failed_runs rejected',
    );

    // 7b. R1 negative control: re-hashed same-length rewritten exclusion tables (extinction, singleton, empty_half).
    const r1ExclDir = join(root, 'r1_excl_tables');
    const r1Data = buildSyntheticE3Data(prereg, { extinctControlUniformSeeds: extSeeds });
    const tailSeed = Number(prereg.seeds.execution_order[4]);
    const ehSeed = Number(prereg.seeds.execution_order[5]);
    for (const s of r1Data.series) {
      if (s.seed === tailSeed && s.arm === 'treatment_gradient' && s.tick === 15000) {
        s['live.agent_count'] = 1;
      }
    }
    const fEh = r1Data.finals.find((f) => f.seed === ehSeed && f.arm === 'treatment_gradient');
    fEh.final_observables['live.trait_mass_mean_west'] = 0;
    writeRecomputedBundleFiles(r1ExclDir, prereg, r1Data);
    editJsonAndRehash(r1ExclDir, 'effects.json', (j) => {
      j.extinction_table_by_arm[0] = { seed: 1, arm: 'control_uniform', first_extinct_tick: 1 };
      j.singleton_table_by_arm[0] = {
        seed: 1,
        arm: 'treatment_gradient',
        tick: 1,
        agent_count: 1,
        scope: 'final_T',
      };
      j.empty_half_table_by_arm[0] = {
        seed: 1,
        arm: 'treatment_gradient',
        tick: 1,
        empty_half: 'east',
      };
    });
    const resR1Excl = verifyArtifactDir(r1ExclDir);
    expect(
      !resR1Excl.ok &&
        resR1Excl.failures.some((m) => m.includes('extinction_table_by_arm[0]')) &&
        resR1Excl.failures.some((m) => m.includes('singleton_table_by_arm[0]')) &&
        resR1Excl.failures.some((m) => m.includes('empty_half_table_by_arm[0]')),
      'R1: same-length rewritten extinction, singleton, and empty_half tables rejected',
    );

    // 8. N1 control: identical non-integer deltas clamp tiny float noise to sd = 0 / dz = null.
    const identicalStats = pairedStats(Array(12).fill(3.0), Array(12).fill(1.1), 12);
    const identicalDecision = classifyE3Decision(identicalStats, prereg.thresholds);
    expect(
      identicalStats.paired_sd === 0 &&
        identicalStats.paired_se === 0 &&
        identicalStats.paired_dz === null &&
        identicalStats.between_seed_variance_of_deltas === 0 &&
        identicalDecision.decision === 'between_detectable_below_materiality',
      'N1: identical non-integer deltas (3.0 -> 1.1) clamp float noise to sd=0 and dz=null',
    );

    // 9. N2 negative control: mismatched smoke hashes or failed WP-2 gate in host smoke rejected.
    const smokeHashDir = join(root, 'n2_smoke_hash');
    writeRecomputedBundleFiles(smokeHashDir, prereg, buildSyntheticE3Data(prereg));
    editJsonAndRehash(smokeHashDir, 'provenance.json', (j) => {
      j.smoke_reference.provenance_sha256 = 'f'.repeat(64);
    });
    const resSmokeHash = verifyArtifactDir(smokeHashDir);
    expect(
      !resSmokeHash.ok && resSmokeHash.failures.some((m) => m.includes('smoke_reference.provenance_sha256')),
      'N2: tampered smoke_reference SHA-256 rejected',
    );

    const badHostSmoke = writeSyntheticSmokeDir(join(root, 'host_smoke_fail'), {
      commit: prereg.wp2_smoke_calibration.parent_commit,
      wp2Passed: false,
    });
    const smokeWp2Dir = join(root, 'n2_smoke_wp2');
    writeRecomputedBundleFiles(smokeWp2Dir, prereg, buildSyntheticE3Data(prereg, { smokeDir: badHostSmoke }));
    const resSmokeWp2 = verifyArtifactDir(smokeWp2Dir);
    expect(
      !resSmokeWp2.ok && resSmokeWp2.failures.some((m) => m.includes('smoke-report.json gate failed')),
      'N2: failed WP-2 gate inside host smoke directory rejected',
    );

    // 10. N3 control: dirty paths under artifacts/ pass while src/ or traversal paths fail.
    const dirtyOkDir = join(root, 'n3_dirty_ok');
    writeRecomputedBundleFiles(
      dirtyOkDir,
      prereg,
      buildSyntheticE3Data(prereg, {
        gitDirtyPaths: ['artifacts/experiments/e3-productivity-gradient-body-mass/host-smoke/'],
      }),
    );
    expect(verifyArtifactDir(dirtyOkDir).ok, 'N3: untracked path under artifacts/ allowed');

    const dirtyBadDir = join(root, 'n3_dirty_bad');
    writeRecomputedBundleFiles(
      dirtyBadDir,
      prereg,
      buildSyntheticE3Data(prereg, {
        gitDirtyPaths: ['src-tauri/src/core/live_experiment.rs'],
      }),
    );
    const resDirtyBad = verifyArtifactDir(dirtyBadDir);
    expect(
      !resDirtyBad.ok && resDirtyBad.failures.some((m) => m.includes('disallowed dirty working tree path')),
      'N3: dirty path outside artifacts/ rejected',
    );

    // 11. N4 control: fallback to recorded sha256_as_committed when commit unavailable vs exact commit match at HEAD.
    expect(
      validRes.notes.some((n) => n.includes('fell back to recorded provenance.manifests.sha256_as_committed')),
      'N4: unavailable recorded commit falls back to provenance.manifests.sha256_as_committed with explicit note',
    );
    const priorAmendmentDir = join(root, 'n4_prior_amendment');
    writeRecomputedBundleFiles(priorAmendmentDir, prereg, buildSyntheticE3Data(prereg));
    const priorPreregPath = join(priorAmendmentDir, 'manifests', PREREG_FILE);
    const priorPreregDoc = JSON.parse(readFileSync(priorPreregPath, 'utf8'));
    delete priorPreregDoc.default_keys_left_absent['live.founder_layout'];
    writeFileSync(priorPreregPath, `${JSON.stringify(priorPreregDoc, null, 2)}\n`);
    const priorPreregSha = sha256NormalizedLf(readFileSync(priorPreregPath));
    editJsonAndRehash(priorAmendmentDir, 'provenance.json', (j) => {
      j.manifests.sha256_as_committed[PREREG_FILE] = priorPreregSha;
    });
    const priorAmendmentRes = verifyArtifactDir(priorAmendmentDir);
    expect(
      priorAmendmentRes.ok &&
        priorAmendmentRes.notes.some((n) =>
          n.includes('fell back to recorded provenance.manifests.sha256_as_committed'),
        ),
      'N4: pre-amendment prereg copy differing from HEAD passes via recorded sha256_as_committed',
    );
    const tamperedCopyDir = join(root, 'n4_tampered_copy');
    writeRecomputedBundleFiles(tamperedCopyDir, prereg, buildSyntheticE3Data(prereg));
    editJsonAndRehash(tamperedCopyDir, `manifests/${PREREG_FILE}`, (j) => {
      j.default_keys_left_absent['live.founder_layout'] = 1;
    });
    const tamperedCopyRes = verifyArtifactDir(tamperedCopyDir);
    expect(
      !tamperedCopyRes.ok &&
        tamperedCopyRes.failures.some((m) => m.includes('sha256_as_committed')),
      'N4: manifest copy mismatching recorded sha256_as_committed rejected',
    );
    const missingShaDir = join(root, 'n4_missing_sha');
    writeRecomputedBundleFiles(missingShaDir, prereg, buildSyntheticE3Data(prereg));
    editJsonAndRehash(missingShaDir, 'provenance.json', (j) => {
      delete j.manifests.sha256_as_committed;
    });
    const missingShaRes = verifyArtifactDir(missingShaDir);
    expect(
      !missingShaRes.ok &&
        missingShaRes.failures.some((m) => m.includes('sha256_as_committed')),
      'N4: unavailable commit without provenance.manifests.sha256_as_committed rejected',
    );
    const headCommit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repoRoot }).toString('utf8').trim();
    const headSmokeDir = writeSyntheticSmokeDir(join(root, 'head_smoke'), {
      commit: headCommit,
      hostOs: 'windows',
      hostArch: 'x86_64',
    });
    const headBundleDir = join(root, 'n4_head');
    writeRecomputedBundleFiles(
      headBundleDir,
      prereg,
      buildSyntheticE3Data(prereg, { gitCommit: headCommit, smokeDir: headSmokeDir }),
      { manifestGitRef: headCommit },
    );
    const headRes = verifyArtifactDir(headBundleDir);
    expect(headRes.ok && headRes.notes.length === 0, 'N4: available recorded HEAD commit verifies without fallback note');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }

  if (bad > 0) {
    console.error(`verify_e3_artifacts --self-test FAILED (${bad} failure(s))`);
    process.exit(1);
  }
  console.log('verify_e3_artifacts --self-test PASSED');
}

function main() {
  const argv = process.argv.slice(2);
  if (argv.includes('--self-test')) {
    runSelfTest();
    return;
  }

  let dirArg = null;
  let preregPath = null;
  let skipCommittedManifestCheck = false;
  for (let i = 0; i < argv.length; i += 1) {
    if (argv[i] === '--prereg') {
      preregPath = resolve(argv[++i]);
    } else if (argv[i] === '--skip-committed-manifests') {
      skipCommittedManifestCheck = true;
    } else if (!argv[i].startsWith('--')) {
      dirArg = resolve(argv[i]);
    }
  }

  const targetDir = dirArg ?? join(repoRoot, DEFAULT_CONFIRM_DIR);
  if (!dirArg && !existsSync(targetDir)) {
    console.log(`check:e3 — no confirmation artifacts at ${DEFAULT_CONFIRM_DIR} yet (pre-E3-X-run ok)`);
    return;
  }

  const res = verifyArtifactDir(targetDir, { preregPath, skipCommittedManifestCheck });
  for (const n of res.notes) {
    console.warn(`verify_e3_artifacts NOTE: ${n}`);
  }
  if (!res.ok) {
    console.error(`verify_e3_artifacts FAILED (${res.failures.length} failure(s)):`);
    for (const f of res.failures) console.error(`  - ${f}`);
    process.exit(1);
  }
  console.log(`verify_e3_artifacts OK (${targetDir})`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main();
}
