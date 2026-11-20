#!/usr/bin/env node
// Verify a published SEL2-P artifact directory independently of `run_sel2p_experiment`.
//
//   node scripts/verify_sel2p_artifacts.mjs [artifact-dir]
//   node scripts/verify_sel2p_artifacts.mjs --self-test
//
// Defaults to `artifacts/experiments/sel2-p-food-scarcity`. Missing directory exits 0 ("no
// artifacts yet"); an existing directory that is empty or incomplete FAILS (exit 1).

import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import {
  cpSync,
  readFileSync,
  existsSync,
  readdirSync,
  mkdtempSync,
  mkdirSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const committedManifestDir = 'src-tauri/tests/fixtures/experiments_sel2p';
const PREREG = 'sel2p-preregistration.json';

const failures = [];
const notes = [];
const fail = (msg) => failures.push(msg);
const note = (msg) => notes.push(msg);

const sha256 = (buf) => createHash('sha256').update(buf).digest('hex');
const readJson = (p) => JSON.parse(readFileSync(p, 'utf8'));

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
  if (bytes === null) {
    // Fallback to working tree during `--self-test` if HEAD does not yet hold the fixtures.
    const wtPath = join(repoRoot, committedManifestDir, PREREG);
    if (existsSync(wtPath)) return JSON.parse(readFileSync(wtPath, 'utf8'));
    throw new Error(`HEAD has no ${committedManifestDir}/${PREREG}`);
  }
  return JSON.parse(bytes.toString('utf8'));
}

function committedBytes(name) {
  const bytes = readCommitted(name);
  if (bytes !== null) return bytes;
  const wtPath = join(repoRoot, committedManifestDir, name);
  if (existsSync(wtPath)) return readFileSync(wtPath);
  return null;
}

const T_975 = [
  12.706204736432095, 4.302652729696142, 3.1824463052842638, 2.7764451051977987,
  2.570581835636314, 2.4469118487916806, 2.3646242510102993, 2.306004135204166,
  2.2621571628540993, 2.2281388519649385, 2.200985160082949, 2.1788128296634177,
  2.1603686564610127, 2.1447866879169273, 2.131449545559323, 2.1199052992210112,
  2.109815577833181, 2.10092204024096, 2.093024054408263, 2.0859634472658364,
  2.079613844727662, 2.073873067904015, 2.068657610419041, 2.063898561628021,
  2.059538552753294, 2.055529438642871, 2.051830516480289, 2.048407141795244,
  2.045229642132703, 2.042272456301238,
];

function studentT95(df) {
  const lower = Math.floor(df);
  if (lower <= 30) return T_975[lower - 1];
  if (lower < 40) return T_975[29];
  if (lower < 60) return 2.0210753903062733;
  if (lower < 80) return 2.000297821058262;
  if (lower < 100) return 1.9900634210714;
  if (lower < 120) return 1.9839715184496334;
  return 1.979930405052777;
}

export function pairedStats(control, treatment) {
  const n = control.length;
  const deltas = control.map((c, i) => treatment[i] - c);
  const mean = (xs) => xs.reduce((a, b) => a + b, 0) / xs.length;
  const out = {
    n,
    controlMean: null,
    treatmentMean: null,
    delta: null,
    sd: null,
    se: null,
    lo: null,
    hi: null,
    dz: null,
  };
  if (n === 0) return out;
  out.controlMean = mean(control);
  out.treatmentMean = mean(treatment);
  out.delta = mean(deltas);
  if (n < 2) return out;
  out.sd = Math.sqrt(deltas.reduce((a, d) => a + (d - out.delta) ** 2, 0) / (n - 1));
  out.se = out.sd / Math.sqrt(n);
  const half = studentT95(n - 1) * out.se;
  out.lo = out.delta - half;
  out.hi = out.delta + half;
  if (out.sd > 0) out.dz = out.delta / out.sd;
  return out;
}

export function materiality(st, thresholds = committedPrereg().thresholds) {
  const minAbsDz = Number(thresholds?.min_abs_paired_dz);
  const minRelShift = Number(thresholds?.min_relative_shift_vs_control_mean);
  if (!Number.isFinite(minAbsDz) || !Number.isFinite(minRelShift)) {
    throw new Error('prereg thresholds lack min_abs_paired_dz / min_relative_shift_vs_control_mean');
  }
  const ci = st.lo !== null && st.hi !== null && (st.lo > 0 || st.hi < 0);
  const dz = st.dz !== null && Math.abs(st.dz) >= minAbsDz;
  const rel =
    st.delta !== null &&
    st.controlMean !== null &&
    Math.abs(st.delta) >= minRelShift * Math.abs(st.controlMean);
  return { ci, dz, rel, material: ci && dz && rel };
}

export function evaluatePooledBaseline(runs, minSurviving = committedPrereg().seeds.min_surviving_baseline_seeds) {
  const surviving = runs.filter(
    (r) =>
      r.completed &&
      (r.extinct_tick === null || r.extinct_tick === undefined) &&
      Number.isFinite(r.crop_second_half),
  );
  const sufficient = surviving.length >= minSurviving;
  const pooledCrop =
    sufficient && surviving.length > 0
      ? surviving.reduce((acc, r) => acc + r.crop_second_half, 0) / surviving.length
      : null;
  return {
    n_requested: runs.length,
    n_surviving: surviving.length,
    min_required: minSurviving,
    surviving_seeds: surviving.map((r) => r.seed),
    pooled_crop_second_half: pooledCrop,
    sufficient,
  };
}

export function evaluateSmokeGate(
  durationTicks,
  baseline,
  selected,
  scarceMut0,
  scarceDrift,
  smokeGate = committedPrereg().smoke_gate,
) {
  const byArm = { selected, scarce_mut0: scarceMut0, scarce_drift: scarceDrift };
  const c1MinAgents = Number(smokeGate.c1_min_agents);
  const [c2Lo, c2Hi] = smokeGate.c2_g_band.map(Number);
  const c3MinGens = Number(smokeGate.c3_min_generations);
  const c4MinShare = Number(smokeGate.c4_min_starvation_share);

  const c1Arms = smokeGate.c1_applies_to_arms ?? ['selected', 'scarce_mut0'];
  const c2Arms = smokeGate.c2_applies_to_arms ?? ['selected', 'scarce_mut0'];
  const c3Arms = smokeGate.c3_applies_to_arms ?? ['selected', 'scarce_mut0'];
  const c4Arms = smokeGate.c4_applies_to_arms ?? ['selected', 'scarce_mut0'];
  const c5Arms = smokeGate.c5_applies_to_arms ?? ['selected', 'scarce_mut0'];

  const survived = (x) =>
    Boolean(x) && x.completed && (x.extinct_tick === null || x.extinct_tick === undefined);
  const c1Arm = (x) => survived(x) && Number.isFinite(x.min_agents) && x.min_agents >= c1MinAgents;
  const c1 = c1Arms.every((arm) => c1Arm(byArm[arm]));

  const refCrop =
    baseline.sufficient &&
    Number.isFinite(baseline.pooled_crop_second_half) &&
    baseline.pooled_crop_second_half > 0
      ? baseline.pooled_crop_second_half
      : null;
  const gOf = (x) =>
    refCrop !== null && x && Number.isFinite(x.crop_second_half) ? x.crop_second_half / refCrop : null;
  const gSelected = gOf(selected);
  const gMut0 = gOf(scarceMut0);
  const gDrift = gOf(scarceDrift);
  const inBand = (g) => g !== null && g >= c2Lo && g <= c2Hi;
  const c2 = Boolean(baseline.sufficient) && c2Arms.every((arm) => inBand(gOf(byArm[arm])));

  const gensOf = (x) => {
    if (!x || !(x.total_deaths > 0) || !Number.isFinite(x.mean_lifespan) || x.mean_lifespan <= 0) {
      return null;
    }
    if (!Number.isFinite(x.mean_agents) || x.mean_agents <= 0) return null;
    return Math.min(durationTicks / x.mean_lifespan, x.total_births / Math.max(1.0, x.mean_agents));
  };
  const gensSel = gensOf(selected);
  const gensMut0 = gensOf(scarceMut0);
  const c3 = c3Arms.every((arm) => {
    const gens = gensOf(byArm[arm]);
    return gens !== null && gens >= c3MinGens;
  });

  const starvShare = (x) => {
    if (!x || !(x.total_deaths > 0) || x.cause_partition_ok !== true) return null;
    if (!Number.isFinite(x.total_starvation) || !Number.isFinite(x.total_dehydration)) return null;
    return x.total_starvation / x.total_deaths;
  };
  const shareSel = starvShare(selected);
  const shareMut0 = starvShare(scarceMut0);
  const c4 = c4Arms.every((arm) => {
    const sh = starvShare(byArm[arm]);
    return sh !== null && sh > c4MinShare;
  });

  const c5Arm = (x) => Boolean(x) && x.births_last_quarter > 0 && x.deaths_last_quarter > 0;
  const c5 = c5Arms.every((arm) => c5Arm(byArm[arm]));
  const driftValid = survived(scarceDrift);

  return {
    c1,
    c2,
    c3,
    c4,
    c5,
    drift_valid: driftValid,
    gate_pass: c1 && c2 && c3 && c4 && c5 && driftValid,
    g_selected: gSelected,
    g_scarce_mut0: gMut0,
    g_scarce_drift: gDrift,
    generations_selected: gensSel,
    generations_scarce_mut0: gensMut0,
    starvation_share_selected: shareSel,
    starvation_share_scarce_mut0: shareMut0,
  };
}

export function parseCsv(text) {
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
  return rows
    .filter((r) => r.length > 1 || (r.length === 1 && r[0] !== ''))
    .map((r) => Object.fromEntries(header.map((h, i) => [h, r[i] ?? ''])));
}

function strictFinite(val) {
  if (val === '' || val === null || val === undefined) return null;
  const num = Number(val);
  return Number.isFinite(num) ? num : null;
}

export function summarizeSmokeSeries(
  seriesRows,
  durationTicks,
  reportRuns,
  onError = fail,
  expectedSamplePeriod = null,
) {
  const byRun = new Map();
  for (const r of reportRuns) {
    byRun.set(`${r.arm}|${r.seed}`, {
      arm: r.arm,
      seed: Number(r.seed),
      completed: r.status === 'completed' || r.status === 'Completed',
      rows: [],
    });
  }
  for (const row of seriesRows) {
    const key = `${row.arm}|${row.seed}`;
    if (!byRun.has(key)) {
      byRun.set(key, { arm: row.arm, seed: Number(row.seed), completed: true, rows: [] });
    }
    byRun.get(key).rows.push(row);
  }

  const summarizeOne = (entry) => {
    const isDrift = entry.arm === 'scarce_drift';
    if (expectedSamplePeriod && entry.completed) {
      const expectedCount = durationTicks / expectedSamplePeriod;
      if (entry.rows.length !== expectedCount) {
        onError(
          `smoke: ${entry.arm} seed ${entry.seed} has ${entry.rows.length} series samples, expected ${expectedCount}`,
        );
      }
      for (let i = 0; i < entry.rows.length; i += 1) {
        const expectedTick = (i + 1) * expectedSamplePeriod;
        const gotTick = strictFinite(entry.rows[i].tick);
        if (gotTick !== expectedTick) {
          onError(
            `smoke: ${entry.arm} seed ${entry.seed} sample[${i}] tick=${entry.rows[i].tick}, expected ${expectedTick}`,
          );
          break;
        }
      }
    }

    let minAgents = Infinity;
    let agentsSum = 0;
    let agentsN = 0;
    let extinctTick = null;
    let cropSum = 0;
    let cropN = 0;
    let totalBirths = 0;
    let totalDeaths = 0;
    let lifespanWeighted = 0;
    let totalStarv = 0;
    let totalDehyd = 0;
    let partOk = true;
    let birthsLq = 0;
    let deathsLq = 0;

    for (const r of entry.rows) {
      const tick = Number(r.tick);
      const n = strictFinite(r['live.agent_count']);
      if (n !== null) {
        minAgents = Math.min(minAgents, n);
        agentsSum += n;
        agentsN += 1;
        if (n === 0 && extinctTick === null) extinctTick = tick;
      } else if (entry.completed) {
        onError(`smoke: ${entry.arm} seed ${entry.seed} tick ${tick} missing live.agent_count`);
      }
      const crop = strictFinite(r['live.standing_crop']);
      if (tick * 2 > durationTicks) {
        if (crop !== null) {
          cropSum += crop;
          cropN += 1;
        } else if (entry.completed) {
          onError(`smoke: ${entry.arm} seed ${entry.seed} tick ${tick} missing live.standing_crop`);
        }
      }
      const b = strictFinite(r['live.births']);
      const d = strictFinite(r['live.deaths']);
      const ls = strictFinite(r['live.mean_lifespan']) ?? 0;
      if (b === null || d === null) {
        if (entry.completed) {
          onError(`smoke: ${entry.arm} seed ${entry.seed} tick ${tick} missing births/deaths`);
        }
        continue;
      }
      totalBirths += b;
      totalDeaths += d;
      lifespanWeighted += ls * d;
      if (tick * 4 > durationTicks * 3) {
        birthsLq += b;
        deathsLq += d;
      }
      if (isDrift) {
        if (r['live.deaths_starvation'] !== '' || r['live.deaths_dehydration'] !== '') {
          onError(`smoke: scarce_drift seed ${entry.seed} tick ${tick} emitted cause-split column`);
        }
      } else {
        const s = strictFinite(r['live.deaths_starvation']);
        const h = strictFinite(r['live.deaths_dehydration']);
        if (s === null || h === null) {
          onError(
            `smoke: ${entry.arm} seed ${entry.seed} tick ${tick} missing starvation/dehydration cell`,
          );
          partOk = false;
        } else {
          totalStarv += s;
          totalDehyd += h;
          if (Math.abs(s + h - d) > 1e-6) partOk = false;
        }
      }
    }

    return {
      arm: entry.arm,
      seed: entry.seed,
      completed: entry.completed,
      extinct_tick: extinctTick,
      min_agents: agentsN > 0 ? minAgents : null,
      mean_agents: agentsN > 0 ? agentsSum / agentsN : null,
      crop_second_half: cropN > 0 ? cropSum / cropN : null,
      total_births: totalBirths,
      total_deaths: totalDeaths,
      mean_lifespan: totalDeaths > 0 ? lifespanWeighted / totalDeaths : null,
      total_starvation: isDrift ? null : totalStarv,
      total_dehydration: isDrift ? null : totalDehyd,
      cause_partition_ok: isDrift
        ? null
        : partOk && Math.abs(totalStarv + totalDehyd - totalDeaths) <= 1e-6,
      births_last_quarter: birthsLq,
      deaths_last_quarter: deathsLq,
    };
  };

  const baseline = [];
  const scarce = {};
  for (const entry of byRun.values()) {
    const s = summarizeOne(entry);
    if (s.arm === 'baseline_ref') baseline.push(s);
    else scarce[s.arm] = s;
  }
  return { baseline, scarce };
}

function close(a, b) {
  if (a === null || a === undefined || b === null || b === undefined) {
    return (a ?? null) === (b ?? null);
  }
  return Math.abs(a - b) <= Math.max(1e-9 * Math.max(Math.abs(a), Math.abs(b)), 1e-12);
}

const ROOT_FILES = [
  'paired-report.json',
  'mut0-runs.json',
  'effects.json',
  'runs.csv',
  'per-seed-deltas.csv',
  'provenance.json',
  'summary.md',
  'checksums.sha256',
];

const SMOKE_FILES = [
  'runs.csv',
  'smoke-series.csv',
  'smoke-report.json',
  'provenance.json',
  'summary.md',
  'checksums.sha256',
];

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

function verifyChecksums(dir, label) {
  const file = join(dir, 'checksums.sha256');
  if (!existsSync(file)) {
    fail(`${label}: checksums.sha256 is missing`);
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
      fail(`${label}: ${name} is listed in checksums.sha256 but missing`);
      continue;
    }
    const actual = sha256(readFileSync(target));
    if (actual !== expected) {
      fail(`${label}: ${name} hashes to ${actual}, expected ${expected}`);
    }
    covered.add(name);
  }
  return covered;
}

function verifyNothingUncovered(dir, covered, label, allowDriverDecisions = false) {
  const skip = new Set(['checksums.sha256', 'RESULT.md']);
  if (allowDriverDecisions) skip.add('smoke-decisions.json');
  const walk = (rel) => {
    for (const entry of readdirSync(join(dir, rel), { withFileTypes: true })) {
      const child = rel ? `${rel}/${entry.name}` : entry.name;
      if (entry.isDirectory()) {
        if (label === 'analysis' && (child === 'smoke' || child === 'replay' || child === 'superseded')) {
          continue;
        }
        walk(child);
      } else if (!skip.has(child) && !covered.has(child)) {
        fail(`${label}: ${child} exists without checksum coverage`);
      }
    }
  };
  walk('');
}

function verifyManifestsAreCommitted(dir, label) {
  const copied = join(dir, 'manifests');
  if (!existsSync(copied)) {
    fail(`${label}: no manifests/ copy`);
    return;
  }
  for (const entry of readdirSync(copied)) {
    const committed = committedBytes(entry);
    if (committed === null) {
      fail(`${label}: manifests/${entry} has no counterpart in ${committedManifestDir}`);
      continue;
    }
    const a = sha256(readFileSync(join(copied, entry)));
    const b = sha256(committed);
    if (a !== b) {
      fail(`${label}: manifests/${entry} (${a}) != committed blob (${b})`);
    }
  }
}

function verifySmoke(dir, label, prereg) {
  if (!requireFiles(dir, label, SMOKE_FILES, [])) return null;
  const covered = verifyChecksums(dir, label);
  verifyNothingUncovered(dir, covered, label, false);

  for (const entry of readdirSync(dir)) {
    const text = readFileSync(join(dir, entry), 'utf8');
    if (text.includes('trait_mass')) {
      fail(`${label}: ${entry} names a body-mass observable; smoke must remain blind`);
    }
  }
  for (const banned of [
    'effects.json',
    'paired-report.json',
    'mut0-runs.json',
    'per-seed-deltas.csv',
    'manifests',
    'execution-lock.json',
    'smoke-decisions.json',
  ]) {
    if (existsSync(join(dir, banned))) fail(`${label}: ${banned} must not exist in smoke/`);
  }

  const prov = readJson(join(dir, 'provenance.json'));
  const report = readJson(join(dir, 'smoke-report.json'));
  if (report.contains_body_mass !== false || prov.contains_body_mass !== false) {
    fail(`${label}: smoke provenance/report must state contains_body_mass = false`);
  }
  if (
    report.baseline_computed_before_scarce !== true ||
    prov.baseline_computed_before_scarce !== true
  ) {
    fail(`${label}: smoke must state baseline_computed_before_scarce = true`);
  }
  if (prov.timing?.baseline_ended_utc && prov.timing?.scarce_started_utc) {
    if (prov.timing.baseline_ended_utc > prov.timing.scarce_started_utc) {
      fail(`${label}: baseline ended (${prov.timing.baseline_ended_utc}) after scarce started`);
    }
  }
  if (prov.build?.profile !== 'release') {
    fail(`${label}: built as "${prov.build?.profile}", expected release`);
  }

  if (report.mode !== 'smoke' && report.mode !== 'pipeline-check') {
    fail(`${label}: unsupported smoke report.mode ${JSON.stringify(report.mode)}`);
    return null;
  }
  if (prov.mode !== report.mode) {
    fail(`${label}: provenance.mode (${prov.mode}) != report.mode (${report.mode})`);
  }
  const isPipeline = report.mode === 'pipeline-check';
  const expectedDuration = isPipeline
    ? prereg.duration.pipeline_check_duration_ticks
    : prereg.duration.duration_ticks;
  const expectedPeriod = isPipeline
    ? prereg.duration.pipeline_check_smoke_sample_period
    : prereg.duration.smoke_sample_period;

  if (report.duration_ticks !== expectedDuration || report.sample_period !== expectedPeriod) {
    fail(
      `${label}: smoke report duration/period (${report.duration_ticks}, ${report.sample_period}) != prereg (${expectedDuration}, ${expectedPeriod})`,
    );
  }
  if (
    prov.duration?.duration_ticks_as_run !== expectedDuration ||
    prov.duration?.sample_period !== expectedPeriod
  ) {
    fail(
      `${label}: smoke provenance duration/period (${prov.duration?.duration_ticks_as_run}, ${prov.duration?.sample_period}) != prereg (${expectedDuration}, ${expectedPeriod})`,
    );
  }

  const expectedBase = isPipeline
    ? prereg.seeds.pipeline_check.baseline_seeds
    : prereg.seeds.baseline_seeds;
  const expectedSmokeSeed = isPipeline
    ? prereg.seeds.pipeline_check.smoke_seed
    : prereg.seeds.smoke_seed;
  if (JSON.stringify(prov.seeds?.baseline_seeds) !== JSON.stringify(expectedBase)) {
    fail(`${label}: baseline_seeds mismatch`);
  }
  if (prov.seeds?.scarce_smoke_seed !== expectedSmokeSeed) {
    fail(`${label}: scarce_smoke_seed ${prov.seeds?.scarce_smoke_seed} != ${expectedSmokeSeed}`);
  }

  const expectedRunKeys = new Set([
    ...expectedBase.map((s) => `baseline_ref|${s}`),
    `selected|${expectedSmokeSeed}`,
    `scarce_mut0|${expectedSmokeSeed}`,
    `scarce_drift|${expectedSmokeSeed}`,
  ]);
  const gotRunKeys = new Set((report.runs ?? []).map((r) => `${r.arm}|${r.seed}`));
  for (const k of expectedRunKeys) {
    if (!gotRunKeys.has(k)) fail(`${label}: smoke-report missing expected run ${k}`);
  }
  for (const k of gotRunKeys) {
    if (!expectedRunKeys.has(k)) fail(`${label}: smoke-report has unexpected run ${k}`);
  }

  const seriesText = readFileSync(join(dir, 'smoke-series.csv'), 'utf8');
  const rows = parseCsv(seriesText);
  const summarized = summarizeSmokeSeries(
    rows,
    expectedDuration,
    report.runs ?? [],
    fail,
    expectedPeriod,
  );
  const recomputedPool = evaluatePooledBaseline(
    summarized.baseline,
    prereg.seeds.min_surviving_baseline_seeds,
  );
  const recomputedGate = evaluateSmokeGate(
    expectedDuration,
    recomputedPool,
    summarized.scarce.selected,
    summarized.scarce.scarce_mut0,
    summarized.scarce.scarce_drift,
    prereg.smoke_gate,
  );

  for (const field of ['n_requested', 'n_surviving', 'min_required', 'sufficient']) {
    if (report.pooled_baseline?.[field] !== recomputedPool[field]) {
      fail(`${label}: pooled_baseline.${field} disagrees with smoke-series.csv recomputation`);
    }
  }
  if (!close(report.pooled_baseline?.pooled_crop_second_half, recomputedPool.pooled_crop_second_half)) {
    fail(
      `${label}: pooled_crop_second_half = ${report.pooled_baseline?.pooled_crop_second_half}, ` +
        `recomputed ${recomputedPool.pooled_crop_second_half}`,
    );
  }
  for (const [jsonKey, gateKey] of [
    ['c1_min_agents_ge_5', 'c1'],
    ['c2_g_in_0_4_to_0_6', 'c2'],
    ['c3_generations_ge_30', 'c3'],
    ['c4_starvation_share_gt_0_5_non_drift_only', 'c4'],
    ['c5_last_quarter_active', 'c5'],
    ['drift_valid', 'drift_valid'],
    ['gate_pass', 'gate_pass'],
  ]) {
    if (report.gate?.[jsonKey] !== recomputedGate[gateKey]) {
      fail(`${label}: gate.${jsonKey} (${report.gate?.[jsonKey]}) != recomputed ${recomputedGate[gateKey]}`);
    }
  }
  for (const numKey of [
    'g_selected',
    'g_scarce_mut0',
    'generations_selected',
    'generations_scarce_mut0',
    'starvation_share_selected',
    'starvation_share_scarce_mut0',
  ]) {
    if (!close(report.gate?.[numKey], recomputedGate[numKey])) {
      fail(`${label}: gate.${numKey} (${report.gate?.[numKey]}) != recomputed ${recomputedGate[numKey]}`);
    }
  }
  if (!close(report.gate?.g_scarce_drift_diagnostic, recomputedGate.g_scarce_drift)) {
    fail(
      `${label}: gate.g_scarce_drift_diagnostic (${report.gate?.g_scarce_drift_diagnostic}) ` +
        `!= recomputed ${recomputedGate.g_scarce_drift}`,
    );
  }
  note(
    `${label}: baseline surviving ${recomputedPool.n_surviving}/${recomputedPool.n_requested}; ` +
      `C1..C5=${recomputedGate.c1},${recomputedGate.c2},${recomputedGate.c3},${recomputedGate.c4},${recomputedGate.c5} ` +
      `drift=${recomputedGate.drift_valid}`,
  );
  return {
    mode: report.mode,
    durationTicks: expectedDuration,
    samplePeriod: expectedPeriod,
    smokeSeed: expectedSmokeSeed,
    recomputedPool,
    recomputedGate,
  };
}

function verifySmokeDecisions(dir, smokeInfo) {
  const decisionsPath = join(dir, 'smoke-decisions.json');
  if (!existsSync(decisionsPath)) return;
  const raw = readFileSync(decisionsPath, 'utf8');
  if (raw.includes('trait_mass')) {
    fail('smoke-decisions: names a body-mass observable');
  }
  if (!smokeInfo) {
    fail('smoke-decisions: smoke-decisions.json exists without a valid smoke/ directory');
    return;
  }
  const d = JSON.parse(raw);
  if (d.contains_body_mass !== false || d.baseline_computed_before_scarce !== true) {
    fail('smoke-decisions: blindness/ordering flags must be false/true');
  }
  if (d.mode !== smokeInfo.mode || d.duration_ticks !== smokeInfo.durationTicks) {
    fail(`smoke-decisions: mode/duration (${d.mode}, ${d.duration_ticks}) != smoke (${smokeInfo.mode}, ${smokeInfo.durationTicks})`);
  }
  const expectedOutcome = smokeInfo.recomputedGate.gate_pass ? 'pass' : 'stopped';
  if (d.outcome !== expectedOutcome) {
    fail(`smoke-decisions: outcome "${d.outcome}" != expected "${expectedOutcome}"`);
  }
  for (const field of ['n_requested', 'n_surviving', 'min_required', 'sufficient']) {
    if (d.pooled_baseline?.[field] !== smokeInfo.recomputedPool[field]) {
      fail(`smoke-decisions: pooled_baseline.${field} mismatches recomputed smoke pool`);
    }
  }
  if (!close(d.pooled_baseline?.pooled_crop_second_half, smokeInfo.recomputedPool.pooled_crop_second_half)) {
    fail('smoke-decisions: pooled_baseline.pooled_crop_second_half mismatches recomputed smoke pool');
  }
  for (const [k, v] of Object.entries(smokeInfo.recomputedGate)) {
    const got = d.gate?.[k];
    if (typeof v === 'number' || v === null) {
      if (!close(got, v)) fail(`smoke-decisions: gate.${k} (${got}) != recomputed ${v}`);
    } else if (got !== v) {
      fail(`smoke-decisions: gate.${k} (${got}) != recomputed ${v}`);
    }
  }
}

function verifyConfirmLock(dir, prov, prereg, smokeInfo) {
  if (!smokeInfo) {
    fail('confirm: smoke/ is mandatory in confirm mode');
    return;
  }
  if (
    smokeInfo.mode !== 'smoke' ||
    smokeInfo.durationTicks !== prereg.duration.duration_ticks ||
    smokeInfo.smokeSeed !== prereg.seeds.smoke_seed
  ) {
    fail(
      `confirm: smoke/ must be a real smoke run (${prereg.duration.duration_ticks} ticks, seed ${prereg.seeds.smoke_seed}), ` +
        `got mode=${smokeInfo.mode} duration=${smokeInfo.durationTicks} seed=${smokeInfo.smokeSeed}`,
    );
  }
  if (smokeInfo.recomputedGate.gate_pass !== true || smokeInfo.recomputedPool.sufficient !== true) {
    fail('confirm: smoke gate_pass must recompute to true before confirmation is valid');
  }

  const lockPath = join(dir, 'execution-lock.json');
  if (!existsSync(lockPath)) {
    fail('confirm: execution-lock.json is missing');
    return;
  }
  const lockBytes = readFileSync(lockPath);
  const actualLockSha = sha256(lockBytes);
  if (prov.execution_lock_sha256 !== actualLockSha) {
    fail(
      `confirm: provenance.execution_lock_sha256 (${prov.execution_lock_sha256}) != ${actualLockSha}`,
    );
  }
  const lock = JSON.parse(lockBytes.toString('utf8'));
  if (lock.smoke_gate_passed !== true) fail('confirm: lock.smoke_gate_passed must be true');
  if (
    lock.duration_ticks !== prereg.duration.duration_ticks ||
    lock.warmup_ticks !== prereg.duration.warmup_ticks ||
    lock.sample_period !== prereg.duration.confirmation_sample_period
  ) {
    fail('confirm: lock duration/warmup/sample_period disagrees with preregistration');
  }

  const expectedManifestFiles = [PREREG, ...Object.values(prereg.manifests)].sort();
  const lockHashes = lock.manifest_sha256;
  if (!lockHashes || typeof lockHashes !== 'object' || Array.isArray(lockHashes)) {
    fail('confirm: execution-lock.json missing manifest_sha256 object');
  } else {
    const lockKeys = Object.keys(lockHashes).sort();
    if (JSON.stringify(lockKeys) !== JSON.stringify(expectedManifestFiles)) {
      fail(
        `confirm: lock.manifest_sha256 keys (${lockKeys.length}) do not cover exact 8 committed files`,
      );
    }
    for (const name of expectedManifestFiles) {
      const committed = committedBytes(name);
      if (committed === null) {
        fail(`confirm: committed ${name} missing from repo for lock hash check`);
        continue;
      }
      const expectedHash = sha256(committed);
      if (lockHashes[name] !== expectedHash) {
        fail(`confirm: lock.manifest_sha256[${name}] (${lockHashes[name]}) != ${expectedHash}`);
      }
    }
  }

  if (lock.baseline_surviving_count !== smokeInfo.recomputedPool.n_surviving) {
    fail(
      `confirm: lock.baseline_surviving_count (${lock.baseline_surviving_count}) != smoke n_surviving (${smokeInfo.recomputedPool.n_surviving})`,
    );
  }
  if (!close(lock.pooled_baseline_crop, smokeInfo.recomputedPool.pooled_crop_second_half)) {
    fail(
      `confirm: lock.pooled_baseline_crop (${lock.pooled_baseline_crop}) != smoke pooled_crop_second_half (${smokeInfo.recomputedPool.pooled_crop_second_half})`,
    );
  }
}

function seriesObs(sample, id) {
  return sample.observables.find(([k]) => k === id)?.[1];
}

function summarizeRawAnalysisRun(arm, seed, rawRun, duration, warmup, samplePeriod, label) {
  const completed = rawRun.status === 'Completed' || rawRun.status === 'completed';
  const series = Array.isArray(rawRun.series) ? rawRun.series : [];
  if (completed) {
    const expectedLen = duration / samplePeriod;
    if (series.length !== expectedLen) {
      fail(`${label}: ${arm} seed ${seed} series length ${series.length} != ${expectedLen}`);
    }
    for (let i = 0; i < series.length; i += 1) {
      const expectedTick = (i + 1) * samplePeriod;
      if (series[i].tick !== expectedTick) {
        fail(`${label}: ${arm} seed ${seed} series[${i}].tick=${series[i].tick} != ${expectedTick}`);
        break;
      }
    }
  }

  let extinctTick = null;
  let minAgents = Infinity;
  let maxAgents = -Infinity;
  let birthsTotal = 0;
  let euLo = Infinity;
  let euHi = -Infinity;
  let euN = 0;
  let mut0MassVarMax = -Infinity;
  let mut0VarN = 0;

  const postMass = [];
  const tailMass = [];

  for (const s of series) {
    const n = seriesObs(s, 'live.agent_count');
    if (n !== undefined) {
      minAgents = Math.min(minAgents, n);
      maxAgents = Math.max(maxAgents, n);
      if (n === 0 && extinctTick === null) extinctTick = s.tick;
    }
    const b = seriesObs(s, 'live.births');
    if (b !== undefined) birthsTotal += b;
    const eu = seriesObs(s, 'live.closed_eu_total');
    if (eu !== undefined) {
      euLo = Math.min(euLo, eu);
      euHi = Math.max(euHi, eu);
      euN += 1;
    }
    const mv = seriesObs(s, 'live.trait_mass_var');
    if (mv !== undefined) {
      mut0MassVarMax = Math.max(mut0MassVarMax, mv);
      mut0VarN += 1;
    }
    const m = seriesObs(s, 'live.trait_mass_mean');
    if (m !== undefined) {
      if (s.tick > warmup) postMass.push(m);
      if (s.tick * 5 > duration * 4) tailMass.push(m);
    }
  }

  if (extinctTick === null) {
    const finalCount = rawRun.final_observables?.find(([k]) => k === 'live.agent_count')?.[1];
    if (finalCount === 0) extinctTick = duration;
  }

  const iWarm = rawRun.series?.find((s) => s.tick === warmup);
  const iEnd = rawRun.series?.find((s) => s.tick === duration);
  const iwVal = iWarm ? seriesObs(iWarm, 'live.mean_energy_integral') : undefined;
  const ieVal = iEnd ? seriesObs(iEnd, 'live.mean_energy_integral') : undefined;
  const postEnergy =
    iwVal !== undefined && ieVal !== undefined && duration > warmup
      ? (ieVal - iwVal) / (duration - warmup)
      : null;

  const closedEuSpread = euN > 0 ? (euHi - euLo) / Math.max(Math.abs(euHi), 1.0) : null;
  const mean = (xs) => (xs.length > 0 ? xs.reduce((a, b) => a + b, 0) / xs.length : null);

  return {
    arm,
    seed,
    completed,
    usable: completed && extinctTick === null,
    extinctTick,
    postWarmupMass: mean(postMass),
    tail20Mass: mean(tailMass),
    postWarmupEnergy: postEnergy,
    closedEuSpread,
    birthsTotal,
    minAgents: minAgents === Infinity ? null : minAgents,
    maxAgents: maxAgents === -Infinity ? null : maxAgents,
    mut0MassVarMax: mut0VarN > 0 ? mut0MassVarMax : null,
  };
}

function verifyAnalysisRecompute(dir, label, prereg, smokeInfo) {
  const prov = readJson(join(dir, 'provenance.json'));
  const report = readJson(join(dir, 'paired-report.json'));
  const mut0Report = readJson(join(dir, 'mut0-runs.json'));
  const effects = readJson(join(dir, 'effects.json'));
  const runs = parseCsv(readFileSync(join(dir, 'runs.csv'), 'utf8'));
  const deltasCsv = parseCsv(readFileSync(join(dir, 'per-seed-deltas.csv'), 'utf8'));

  if (prov.mode !== 'confirm' && prov.mode !== 'pipeline-check') {
    fail(`${label}: unsupported provenance.mode ${JSON.stringify(prov.mode)}`);
    return;
  }
  const isConfirm = prov.mode === 'confirm';
  if (effects.mode !== prov.mode) {
    fail(`${label}: effects.mode (${effects.mode}) != provenance.mode (${prov.mode})`);
  }
  const expectedSeeds = isConfirm ? prereg.seeds.execution_order : prereg.seeds.pipeline_check.pair_seeds;
  if (JSON.stringify(report.seed_order) !== JSON.stringify(expectedSeeds)) {
    fail(`${label}: seed_order ${JSON.stringify(report.seed_order)} != ${JSON.stringify(expectedSeeds)}`);
  }
  if (JSON.stringify(mut0Report.seed_order) !== JSON.stringify(expectedSeeds)) {
    fail(`${label}: mut0-runs.json seed_order mismatch`);
  }
  if (isConfirm && !['ACCEPTED', 'FROZEN'].includes(prereg.status)) {
    fail(`${label}: confirm mode run exists while preregistration status is ${prereg.status}`);
  }
  if (prov.build?.profile !== 'release') {
    fail(`${label}: built as "${prov.build?.profile}", expected release`);
  }
  if (JSON.stringify(report.declared_factors) !== JSON.stringify(['initial_conditions'])) {
    fail(`${label}: declared_factors ${JSON.stringify(report.declared_factors)} != ["initial_conditions"]`);
  }
  if (isConfirm) {
    verifyConfirmLock(dir, prov, prereg, smokeInfo);
  }

  const expectedDuration = isConfirm
    ? prereg.duration.duration_ticks
    : prereg.duration.pipeline_check_duration_ticks;
  const expectedWarmup = isConfirm
    ? prereg.duration.warmup_ticks
    : prereg.duration.pipeline_check_warmup_ticks;
  const expectedPeriod = isConfirm
    ? prereg.duration.confirmation_sample_period
    : prereg.duration.pipeline_check_pair_sample_period;

  if (
    prov.duration?.duration_ticks_as_run !== expectedDuration ||
    prov.duration?.warmup_ticks_as_run !== expectedWarmup ||
    prov.duration?.sample_period !== expectedPeriod
  ) {
    fail(
      `${label}: analysis provenance duration/warmup/period (${prov.duration?.duration_ticks_as_run}, ` +
        `${prov.duration?.warmup_ticks_as_run}, ${prov.duration?.sample_period}) != prereg ` +
        `(${expectedDuration}, ${expectedWarmup}, ${expectedPeriod})`,
    );
  }

  if (report.pairs.length !== expectedSeeds.length || mut0Report.runs.length !== expectedSeeds.length) {
    fail(`${label}: pair / mut0 run count mismatch vs expectedSeeds (${expectedSeeds.length})`);
    return;
  }

  // Recompute per-seed summaries from paired-report.json + mut0-runs.json and cross-check runs.csv.
  const runRowByKey = new Map(runs.map((r) => [`${r.arm}|${r.seed}`, r]));
  const rawSummaryByKey = new Map();
  const recomputedExtinct = [];
  const recomputedIntegrity = [];
  const windowDeltaBySeed = new Map();

  for (let i = 0; i < expectedSeeds.length; i += 1) {
    const seed = expectedSeeds[i];
    const pair = report.pairs[i];
    if (pair.seed !== seed) {
      fail(`${label}: paired-report.json pairs[${i}].seed=${pair.seed}, expected ${seed}`);
    }
    const entries = [
      ['selected', pair.treatment],
      ['scarce_drift', pair.control],
      ['scarce_mut0', mut0Report.runs[i]],
    ];
    for (const [arm, rawRun] of entries) {
      const sum = summarizeRawAnalysisRun(
        arm,
        seed,
        rawRun,
        expectedDuration,
        expectedWarmup,
        expectedPeriod,
        label,
      );
      rawSummaryByKey.set(`${arm}|${seed}`, sum);

      if (sum.closedEuSpread !== null && sum.closedEuSpread > 1e-6) {
        recomputedIntegrity.push(`${arm} seed ${seed}: live.closed_eu_total spread ${sum.closedEuSpread}`);
      }
      if (sum.birthsTotal === 0 && sum.minAgents !== null && sum.maxAgents > sum.minAgents) {
        recomputedIntegrity.push(`${arm} seed ${seed}: population grew without live.births > 0`);
      }
      if (arm === 'scarce_mut0' && sum.mut0MassVarMax !== null && sum.mut0MassVarMax > 1e-12) {
        recomputedIntegrity.push(
          `scarce_mut0 seed ${seed}: live.trait_mass_var max ${sum.mut0MassVarMax} != 0`,
        );
      }
      if (sum.completed && sum.extinctTick !== null) {
        recomputedExtinct.push({ seed, arm, tick: sum.extinctTick });
      }

      const row = runRowByKey.get(`${arm}|${seed}`);
      if (!row) {
        fail(`${label}: runs.csv lacks ${arm} seed ${seed}`);
        continue;
      }
      const rowCompleted = row.status === 'completed' || row.status === 'Completed';
      if (rowCompleted !== sum.completed) {
        fail(`${label}: runs.csv status=${row.status} disagrees with raw status for ${arm} seed ${seed}`);
      }
      if (String(sum.extinctTick !== null) !== row.extinct) {
        fail(`${label}: runs.csv extinct=${row.extinct} mismatches recomputed ${sum.extinctTick !== null} for ${arm} seed ${seed}`);
      }
      if (sum.extinctTick !== null && String(sum.extinctTick) !== row.extinct_tick) {
        fail(`${label}: runs.csv extinct_tick=${row.extinct_tick} mismatches ${sum.extinctTick} for ${arm} seed ${seed}`);
      }

      if (sum.completed) {
        for (const [col, val] of [
          ['post_warmup_trait_mass_mean', sum.postWarmupMass],
          ['post_warmup_mean_energy_integral', sum.postWarmupEnergy],
          ['tail20_trait_mass_mean', sum.tail20Mass],
        ]) {
          const got = strictFinite(row[col]);
          if (got === null || val === null || !close(got, val)) {
            fail(`${label}: ${arm} seed ${seed} ${col} (${row[col]}) != recomputed ${val}`);
          }
        }
        if (arm === 'selected') {
          const r2Val = strictFinite(row.window_net_selection_trait_mass);
          if (r2Val === null) {
            fail(`${label}: selected seed ${seed} missing window_net_selection_trait_mass in runs.csv`);
          } else {
            windowDeltaBySeed.set(seed, r2Val);
          }
        }
      }
    }
  }

  if (effects.integrity_findings?.length || recomputedIntegrity.length) {
    fail(
      `${label}: integrity findings present (effects=${JSON.stringify(effects.integrity_findings)}, ` +
        `recomputed=${JSON.stringify(recomputedIntegrity)})`,
    );
  }

  // Recompute extinction list across all 3 arms vs effects.json.
  const listedExtinct = effects.extinction?.extinct_runs ?? [];
  const extKey = (x) => `${x.seed}|${x.arm}|${x.tick}`;
  const recomputedExtSet = new Set(recomputedExtinct.map(extKey));
  const effExtSet = new Set(listedExtinct.map(extKey));
  for (const k of recomputedExtSet) {
    if (!effExtSet.has(k)) fail(`${label}: recomputed extinct ${k} missing from effects.json`);
  }
  for (const k of effExtSet) {
    if (!recomputedExtSet.has(k)) fail(`${label}: effects.json extinct ${k} not present in raw series`);
  }
  if (effects.extinction?.n_extinct_runs_total !== recomputedExtinct.length) {
    fail(`${label}: extinction.n_extinct_runs_total (${effects.extinction?.n_extinct_runs_total}) != ${recomputedExtinct.length}`);
  }

  // Recompute per-contrast usable seed lists, verify per-seed-deltas.csv ties directly to
  // verified endpoints, and recompute pairedStats + materiality from those verified endpoints.
  const h1UsableSeeds = expectedSeeds.filter(
    (s) =>
      rawSummaryByKey.get(`selected|${s}`)?.usable &&
      rawSummaryByKey.get(`scarce_drift|${s}`)?.usable,
  );
  if (h1UsableSeeds.length !== effects.h1?.n_complete_pairs || h1UsableSeeds.length !== effects.n_complete_pairs_used_by_h1) {
    fail(
      `${label}: H1 complete pairs mismatch (recomputed=${h1UsableSeeds.length}, ` +
        `effects.h1=${effects.h1?.n_complete_pairs}, n_used=${effects.n_complete_pairs_used_by_h1})`,
    );
  }
  const expectedSufficient = h1UsableSeeds.length >= prereg.seeds.min_complete_pairs_for_a_decision;
  if (effects.sufficient_pairs !== expectedSufficient) {
    fail(`${label}: effects.sufficient_pairs (${effects.sufficient_pairs}) != ${expectedSufficient}`);
  }

  const contrastDefs = [
    {
      name: 'H1_selected_vs_scarce_drift',
      block: effects.h1,
      checkMaterial: true,
      ctrlArm: 'scarce_drift',
      trtArm: 'selected',
      ctrlVal: (s) => rawSummaryByKey.get(`scarce_drift|${s}`)?.postWarmupMass ?? null,
      trtVal: (s) => rawSummaryByKey.get(`selected|${s}`)?.postWarmupMass ?? null,
    },
    {
      name: 'H2a_selected_vs_scarce_mut0',
      block: effects.h2a_selected_vs_scarce_mut0,
      checkMaterial: false,
      ctrlArm: 'scarce_mut0',
      trtArm: 'selected',
      ctrlVal: (s) => rawSummaryByKey.get(`scarce_mut0|${s}`)?.postWarmupMass ?? null,
      trtVal: (s) => rawSummaryByKey.get(`selected|${s}`)?.postWarmupMass ?? null,
    },
    {
      name: 'H2b_scarce_drift_vs_scarce_mut0',
      block: effects.h2b_scarce_drift_vs_scarce_mut0,
      checkMaterial: false,
      ctrlArm: 'scarce_mut0',
      trtArm: 'scarce_drift',
      ctrlVal: (s) => rawSummaryByKey.get(`scarce_mut0|${s}`)?.postWarmupMass ?? null,
      trtVal: (s) => rawSummaryByKey.get(`scarce_drift|${s}`)?.postWarmupMass ?? null,
    },
    {
      name: 'R1_tail20_selected_vs_scarce_drift',
      block: effects.r1_tail20,
      checkMaterial: false,
      ctrlArm: 'scarce_drift',
      trtArm: 'selected',
      ctrlVal: (s) => rawSummaryByKey.get(`scarce_drift|${s}`)?.tail20Mass ?? null,
      trtVal: (s) => rawSummaryByKey.get(`selected|${s}`)?.tail20Mass ?? null,
    },
    {
      name: 'R2_window_net_selected_vs_scarce_drift',
      block: effects.r2_window_net_selection,
      checkMaterial: false,
      ctrlArm: 'scarce_drift',
      trtArm: 'selected',
      ctrlVal: () => 0,
      trtVal: (s) => windowDeltaBySeed.get(s) ?? null,
    },
    {
      name: 'S2_energy_selected_vs_scarce_drift',
      block: effects.s2_energy_post_warmup,
      checkMaterial: false,
      ctrlArm: 'scarce_drift',
      trtArm: 'selected',
      ctrlVal: (s) => rawSummaryByKey.get(`scarce_drift|${s}`)?.postWarmupEnergy ?? null,
      trtVal: (s) => rawSummaryByKey.get(`selected|${s}`)?.postWarmupEnergy ?? null,
    },
  ];

  let h1MaterialRecomputed = null;
  for (const def of contrastDefs) {
    const expectedContrastSeeds = expectedSeeds.filter(
      (s) =>
        rawSummaryByKey.get(`${def.ctrlArm}|${s}`)?.usable &&
        rawSummaryByKey.get(`${def.trtArm}|${s}`)?.usable,
    );
    const sub = deltasCsv.filter((r) => r.contrast === def.name);
    const gotSeeds = sub.map((r) => strictFinite(r.seed));
    if (JSON.stringify(gotSeeds) !== JSON.stringify(expectedContrastSeeds)) {
      fail(
        `${label}: ${def.name} per-seed-deltas seeds ${JSON.stringify(gotSeeds)} != expected ${JSON.stringify(expectedContrastSeeds)}`,
      );
    }

    const rowBySeed = new Map(sub.map((r) => [strictFinite(r.seed), r]));
    const ctrlVec = [];
    const trtVec = [];
    for (const s of expectedContrastSeeds) {
      const expC = def.ctrlVal(s);
      const expT = def.trtVal(s);
      if (expC === null || expT === null) {
        fail(`${label}: ${def.name} seed ${s} missing verified endpoint (${expC}, ${expT})`);
        continue;
      }
      ctrlVec.push(expC);
      trtVec.push(expT);

      const csvRow = rowBySeed.get(s);
      if (csvRow) {
        const c = strictFinite(csvRow.control_value);
        const t = strictFinite(csvRow.treatment_value);
        const d = strictFinite(csvRow.delta);
        if (c === null || t === null || d === null) {
          fail(`${label}: ${def.name} seed ${s} has empty/non-finite cell in per-seed-deltas.csv`);
        } else {
          if (!close(c, expC)) {
            fail(`${label}: ${def.name} seed ${s} control_value (${c}) != verified endpoint (${expC})`);
          }
          if (!close(t, expT)) {
            fail(`${label}: ${def.name} seed ${s} treatment_value (${t}) != verified endpoint (${expT})`);
          }
          if (!close(t - c, d)) {
            fail(`${label}: ${def.name} seed ${s}: ${t} - ${c} != ${d}`);
          }
        }
      }
    }

    const st = pairedStats(ctrlVec, trtVec);
    const block = def.block;
    if (!block) {
      fail(`${label}: effects.json missing contrast block for ${def.name}`);
      continue;
    }
    if (block.n_complete_pairs !== st.n) {
      fail(`${label}: ${def.name} n_complete_pairs ${block.n_complete_pairs} != ${st.n}`);
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
      if (!close(block[field], got)) {
        fail(`${label}: ${def.name}.${field} (${block[field]}) != recomputed ${got}`);
      }
    }
    if (def.checkMaterial) {
      const m = materiality(st, prereg.thresholds);
      h1MaterialRecomputed = m;
      const claimed = block.materiality ?? {};
      for (const [f, got] of [
        ['ci95_excludes_zero', m.ci],
        ['abs_dz_at_least_0_66', m.dz],
        ['abs_delta_at_least_5pc_of_control_mean', m.rel],
        ['material', m.material],
      ]) {
        if (claimed[f] !== got) {
          fail(`${label}: H1 materiality.${f} (${claimed[f]}) != recomputed ${got}`);
        }
      }
    }
  }

  // D2: independently recompute headline decision.
  let expectedDecision;
  if (!isConfirm) {
    expectedDecision = 'pipeline_check_only';
  } else if (!expectedSufficient) {
    expectedDecision = 'insufficient_evidence';
  } else if (
    h1MaterialRecomputed?.material &&
    recomputedIntegrity.length === 0 &&
    (effects.integrity_findings?.length ?? 0) === 0
  ) {
    expectedDecision = 'shift_in_model';
  } else {
    expectedDecision = 'null';
  }
  if (effects.decision !== expectedDecision) {
    fail(`${label}: effects.decision "${effects.decision}" != recomputed "${expectedDecision}"`);
  }

  note(`${label}: mode=${prov.mode}, H1 pairs=${effects.h1?.n_complete_pairs}, decision=${effects.decision}`);
}

function verifyAll(dir, preregOverride = null) {
  const prereg = preregOverride ?? committedPrereg();
  const provPath = join(dir, 'provenance.json');
  const hasAnalysis = existsSync(provPath);
  const smokeDir = join(dir, 'smoke');
  const hasSmoke = existsSync(smokeDir);

  if (!hasAnalysis && !hasSmoke) {
    requireFiles(dir, 'analysis', ROOT_FILES, Object.values(prereg.manifests));
    return;
  }

  let smokeInfo = null;
  if (hasSmoke) {
    smokeInfo = verifySmoke(smokeDir, 'smoke', prereg);
  }
  verifySmokeDecisions(dir, smokeInfo);

  if (hasAnalysis) {
    const prov = readJson(provPath);
    const req = prov.mode === 'confirm' ? [...ROOT_FILES, 'execution-lock.json'] : ROOT_FILES;
    if (requireFiles(dir, 'analysis', req, Object.values(prereg.manifests))) {
      const covered = verifyChecksums(dir, 'analysis');
      if (prov.mode === 'confirm' && !covered.has('execution-lock.json')) {
        fail('analysis: checksums.sha256 must cover execution-lock.json in confirm mode');
      }
      verifyNothingUncovered(dir, covered, 'analysis', true);
      verifyManifestsAreCommitted(dir, 'analysis');
      verifyAnalysisRecompute(dir, 'analysis', prereg, smokeInfo);
    }
  }
}

function rehashDir(dir) {
  const names = [];
  const walk = (rel) => {
    for (const entry of readdirSync(join(dir, rel), { withFileTypes: true })) {
      const child = rel ? `${rel}/${entry.name}` : entry.name;
      if (entry.isDirectory()) {
        if (child === 'smoke' || child === 'replay' || child === 'superseded') continue;
        walk(child);
      } else if (
        child !== 'checksums.sha256' &&
        child !== 'RESULT.md' &&
        child !== 'smoke-decisions.json'
      ) {
        names.push(child);
      }
    }
  };
  walk('');
  names.sort();
  const lines = names.map((n) => `${sha256(readFileSync(join(dir, n)))}  ${n}\n`);
  writeFileSync(join(dir, 'checksums.sha256'), lines.join(''));
}

function writeSyntheticSmoke(smokeDir, prereg, mode) {
  mkdirSync(smokeDir, { recursive: true });
  const isPipeline = mode === 'pipeline-check';
  const duration = isPipeline
    ? prereg.duration.pipeline_check_duration_ticks
    : prereg.duration.duration_ticks;
  const period = isPipeline
    ? prereg.duration.pipeline_check_smoke_sample_period
    : prereg.duration.smoke_sample_period;
  const baseSeeds = isPipeline
    ? prereg.seeds.pipeline_check.baseline_seeds
    : prereg.seeds.baseline_seeds;
  const smokeSeed = isPipeline
    ? prereg.seeds.pipeline_check.smoke_seed
    : prereg.seeds.smoke_seed;

  const seriesLines = [
    'seed,arm,tick,live.closed_eu_total,live.agent_count,live.standing_crop,live.births,live.deaths,live.mean_lifespan,live.deaths_starvation,live.deaths_dehydration',
  ];
  const runs = [];
  const nSamples = duration / period;
  const birthsPerTick = isPipeline ? 2 : 200;
  const starvPerTick = isPipeline ? 1 : 150;
  const dehydPerTick = isPipeline ? 1 : 50;

  for (const s of baseSeeds) {
    for (let k = 1; k <= nSamples; k += 1) {
      seriesLines.push(`${s},baseline_ref,${k * period},1000,100,1000,1,1,1000,1,0`);
    }
    runs.push({ arm: 'baseline_ref', seed: s, status: 'completed' });
  }
  for (const arm of ['selected', 'scarce_mut0', 'scarce_drift']) {
    const crop = isPipeline ? 600 : 500;
    for (let k = 1; k <= nSamples; k += 1) {
      const tail = arm === 'scarce_drift' ? ',' : `,${starvPerTick},${dehydPerTick}`;
      seriesLines.push(
        `${smokeSeed},${arm},${k * period},1000,50,${crop},${birthsPerTick},${starvPerTick + dehydPerTick},1000${tail}`,
      );
    }
    runs.push({ arm, seed: smokeSeed, status: 'completed' });
  }

  const seriesCsv = `${seriesLines.join('\n')}\n`;
  const rows = parseCsv(seriesCsv);
  const sum = summarizeSmokeSeries(rows, duration, runs, () => {}, period);
  const pool = evaluatePooledBaseline(sum.baseline, prereg.seeds.min_surviving_baseline_seeds);
  const gate = evaluateSmokeGate(
    duration,
    pool,
    sum.scarce.selected,
    sum.scarce.scarce_mut0,
    sum.scarce.scarce_drift,
    prereg.smoke_gate,
  );

  const report = {
    mode,
    contains_body_mass: false,
    baseline_computed_before_scarce: true,
    duration_ticks: duration,
    sample_period: period,
    pooled_baseline: pool,
    gate: {
      c1_min_agents_ge_5: gate.c1,
      c2_g_in_0_4_to_0_6: gate.c2,
      c3_generations_ge_30: gate.c3,
      c4_starvation_share_gt_0_5_non_drift_only: gate.c4,
      c5_last_quarter_active: gate.c5,
      drift_valid: gate.drift_valid,
      gate_pass: gate.gate_pass,
      g_selected: gate.g_selected,
      g_scarce_mut0: gate.g_scarce_mut0,
      g_scarce_drift_diagnostic: gate.g_scarce_drift,
      generations_selected: gate.generations_selected,
      generations_scarce_mut0: gate.generations_scarce_mut0,
      starvation_share_selected: gate.starvation_share_selected,
      starvation_share_scarce_mut0: gate.starvation_share_scarce_mut0,
    },
    runs,
  };
  const prov = {
    mode,
    contains_body_mass: false,
    baseline_computed_before_scarce: true,
    build: { profile: 'release' },
    timing: {
      baseline_ended_utc: '2026-11-19T00:00:00Z',
      scarce_started_utc: '2026-11-19T00:00:01Z',
    },
    duration: { duration_ticks_as_run: duration, sample_period: period },
    seeds: { baseline_seeds: baseSeeds, scarce_smoke_seed: smokeSeed },
  };
  writeFileSync(join(smokeDir, 'smoke-series.csv'), seriesCsv);
  writeFileSync(join(smokeDir, 'smoke-report.json'), `${JSON.stringify(report, null, 2)}\n`);
  writeFileSync(join(smokeDir, 'provenance.json'), `${JSON.stringify(prov, null, 2)}\n`);
  writeFileSync(join(smokeDir, 'runs.csv'), 'seed,arm,status\n');
  writeFileSync(join(smokeDir, 'summary.md'), '# synthetic smoke\n');
  rehashDir(smokeDir);
  return { report, pool, gate };
}

function writeSyntheticFixture(rootDir, prereg, mode) {
  mkdirSync(rootDir, { recursive: true });
  const isConfirm = mode === 'confirm';
  const duration = isConfirm
    ? prereg.duration.duration_ticks
    : prereg.duration.pipeline_check_duration_ticks;
  const warmup = isConfirm
    ? prereg.duration.warmup_ticks
    : prereg.duration.pipeline_check_warmup_ticks;
  const period = isConfirm
    ? prereg.duration.confirmation_sample_period
    : prereg.duration.pipeline_check_pair_sample_period;
  const seeds = isConfirm ? prereg.seeds.execution_order : prereg.seeds.pipeline_check.pair_seeds;

  const smokeInfo = writeSyntheticSmoke(join(rootDir, 'smoke'), prereg, isConfirm ? 'smoke' : 'pipeline-check');

  const manifestNames = Object.values(prereg.manifests);
  mkdirSync(join(rootDir, 'manifests'), { recursive: true });
  for (const m of manifestNames) {
    writeFileSync(join(rootDir, 'manifests', m), committedBytes(m));
  }

  const mkSeries = (massBase, energyRate) => {
    const n = duration / period;
    const out = [];
    for (let k = 1; k <= n; k += 1) {
      const tick = k * period;
      out.push({
        tick,
        observables: [
          ['live.closed_eu_total', 1000],
          ['live.agent_count', 50],
          ['live.births', 2],
          ['live.trait_mass_mean', massBase],
          ['live.trait_mass_var', 0],
          ['live.mean_energy_integral', tick * energyRate],
        ],
      });
    }
    return out;
  };

  const pairs = [];
  const mut0Runs = [];
  const runsLines = [
    'seed,arm,status,failed_tick,failure_reason,extinct,extinct_tick,post_warmup_trait_mass_mean,post_warmup_mean_energy_integral,tail20_trait_mass_mean,window_net_selection_trait_mass,final_checksum,warnings',
  ];
  const deltaLines = ['seed,contrast,statistic,control_value,treatment_value,delta'];

  const perArm = new Map();
  for (let i = 0; i < seeds.length; i += 1) {
    const seed = seeds[i];
    const selMass = 4.0 + i * 0.01;
    const driftMass = 3.3;
    const mut0Mass = 3.2;
    const selWin = 0.05;

    const treatment = {
      status: 'Completed',
      series: mkSeries(selMass, 2.0),
      final_observables: [['live.agent_count', 50]],
    };
    const control = {
      status: 'Completed',
      series: mkSeries(driftMass, 1.0),
      final_observables: [['live.agent_count', 50]],
    };
    const mut0 = {
      status: 'Completed',
      series: mkSeries(mut0Mass, 1.5),
      final_observables: [['live.agent_count', 50]],
    };
    pairs.push({ seed, control, treatment });
    mut0Runs.push(mut0);

    for (const [arm, mass, energy, win] of [
      ['selected', selMass, 2.0, String(selWin)],
      ['scarce_drift', driftMass, 1.0, ''],
      ['scarce_mut0', mut0Mass, 1.5, ''],
    ]) {
      perArm.set(`${arm}|${seed}`, { mass, energy, win: selWin });
      runsLines.push(`${seed},${arm},completed,,,false,,${mass},${energy},${mass},${win},1,`);
    }
  }

  const contrastSpecs = [
    ['H1_selected_vs_scarce_drift', 'h1', 'live.trait_mass_mean@post_warmup_int', (s) => [perArm.get(`scarce_drift|${s}`).mass, perArm.get(`selected|${s}`).mass]],
    ['H2a_selected_vs_scarce_mut0', 'h2a_selected_vs_scarce_mut0', 'live.trait_mass_mean@post_warmup_int', (s) => [perArm.get(`scarce_mut0|${s}`).mass, perArm.get(`selected|${s}`).mass]],
    ['H2b_scarce_drift_vs_scarce_mut0', 'h2b_scarce_drift_vs_scarce_mut0', 'live.trait_mass_mean@post_warmup_int', (s) => [perArm.get(`scarce_mut0|${s}`).mass, perArm.get(`scarce_drift|${s}`).mass]],
    ['R1_tail20_selected_vs_scarce_drift', 'r1_tail20', 'live.trait_mass_mean@tail20', (s) => [perArm.get(`scarce_drift|${s}`).mass, perArm.get(`selected|${s}`).mass]],
    ['R2_window_net_selected_vs_scarce_drift', 'r2_window_net_selection', 'window_net_selection_deltas', (s) => [0, perArm.get(`selected|${s}`).win]],
    ['S2_energy_selected_vs_scarce_drift', 's2_energy_post_warmup', 'live.mean_energy_integral@post_warmup_mean', (s) => [perArm.get(`scarce_drift|${s}`).energy, perArm.get(`selected|${s}`).energy]],
  ];

  const effects = {
    mode,
    decision: isConfirm ? 'shift_in_model' : 'pipeline_check_only',
    n_complete_pairs_used_by_h1: seeds.length,
    sufficient_pairs: seeds.length >= prereg.seeds.min_complete_pairs_for_a_decision,
    extinction: { n_extinct_runs_total: 0, extinct_runs: [] },
    integrity_findings: [],
  };
  for (const [contrast, key, stat, fn] of contrastSpecs) {
    const cv = [];
    const tv = [];
    for (const s of seeds) {
      const [c, t] = fn(s);
      cv.push(c);
      tv.push(t);
      deltaLines.push(`${s},${contrast},${stat},${c},${t},${t - c}`);
    }
    const st = pairedStats(cv, tv);
    effects[key] = {
      n_complete_pairs: st.n,
      control_mean: st.controlMean,
      treatment_mean: st.treatmentMean,
      paired_mean_delta: st.delta,
      paired_sd: st.sd,
      paired_se: st.se,
      ci95_low: st.lo,
      ci95_high: st.hi,
      paired_dz: st.dz,
    };
    if (key === 'h1') {
      const m = materiality(st, prereg.thresholds);
      effects[key].materiality = {
        ci95_excludes_zero: m.ci,
        abs_dz_at_least_0_66: m.dz,
        abs_delta_at_least_5pc_of_control_mean: m.rel,
        material: m.material,
      };
    }
  }

  const prov = {
    mode,
    build: { profile: 'release' },
    duration: { duration_ticks_as_run: duration, warmup_ticks_as_run: warmup, sample_period: period },
  };
  if (isConfirm) {
    const all8 = [PREREG, ...manifestNames].sort();
    const manifestSha = Object.fromEntries(all8.map((n) => [n, sha256(committedBytes(n))]));
    const lock = {
      smoke_gate_passed: true,
      duration_ticks: prereg.duration.duration_ticks,
      warmup_ticks: prereg.duration.warmup_ticks,
      sample_period: prereg.duration.confirmation_sample_period,
      baseline_surviving_count: smokeInfo.pool.n_surviving,
      pooled_baseline_crop: smokeInfo.pool.pooled_crop_second_half,
      manifest_sha256: manifestSha,
    };
    const lockText = `${JSON.stringify(lock, null, 2)}\n`;
    writeFileSync(join(rootDir, 'execution-lock.json'), lockText);
    prov.execution_lock_sha256 = sha256(Buffer.from(lockText));
  }

  writeFileSync(
    join(rootDir, 'paired-report.json'),
    `${JSON.stringify({ seed_order: seeds, declared_factors: ['initial_conditions'], pairs }, null, 2)}\n`,
  );
  writeFileSync(
    join(rootDir, 'mut0-runs.json'),
    `${JSON.stringify({ kind: 'sel2p-scarce-mut0-runs', mode, seed_order: seeds, runs: mut0Runs }, null, 2)}\n`,
  );
  writeFileSync(join(rootDir, 'effects.json'), `${JSON.stringify(effects, null, 2)}\n`);
  writeFileSync(join(rootDir, 'runs.csv'), `${runsLines.join('\n')}\n`);
  writeFileSync(join(rootDir, 'per-seed-deltas.csv'), `${deltaLines.join('\n')}\n`);
  writeFileSync(join(rootDir, 'provenance.json'), `${JSON.stringify(prov, null, 2)}\n`);
  writeFileSync(join(rootDir, 'summary.md'), '# synthetic analysis\n');
  rehashDir(rootDir);

  const decisions = {
    kind: 'sel2p-smoke-decisions-EXCLUDED-FROM-CONFIRMATION',
    mode: isConfirm ? 'smoke' : 'pipeline-check',
    contains_body_mass: false,
    baseline_computed_before_scarce: true,
    duration_ticks: smokeInfo.report.duration_ticks,
    pooled_baseline: smokeInfo.pool,
    gate: smokeInfo.gate,
    outcome: smokeInfo.gate.gate_pass ? 'pass' : 'stopped',
  };
  writeFileSync(join(rootDir, 'smoke-decisions.json'), `${JSON.stringify(decisions, null, 2)}\n`);
}

function runTamperTest(srcDir, preregOverride, mutateFn) {
  const work = mkdtempSync(join(tmpdir(), 'sel2p-tamper-'));
  try {
    cpSync(srcDir, work, { recursive: true });
    mutateFn(work);
    failures.length = 0;
    notes.length = 0;
    verifyAll(work, preregOverride);
    return [...failures];
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}

function runSelfTest() {
  let bad = 0;
  const expect = (cond, what) => {
    if (!cond) {
      bad += 1;
      console.error(`SELF-TEST FAIL  ${what}`);
    } else {
      console.log(`self-test ok    ${what}`);
    }
  };

  const prereg = committedPrereg();

  // 1. Paired statistics & D5 prereg threshold parameterization.
  const st = pairedStats([0, 0, 0], [1, 2, 3]);
  expect(close(st.delta, 2) && close(st.sd, 1) && close(st.se, 1 / Math.sqrt(3)), 'mean/sd/se of 1,2,3');
  expect(materiality(st, prereg.thresholds).material === false, 'df=2 wide CI crossing 0 is not material');
  expect(
    materiality(pairedStats([10, 10, 10, 10], [11, 11.2, 10.9, 11.1]), prereg.thresholds).material === true,
    'consistent 10% shift with |dz| >= 0.66 is material',
  );
  expect(
    materiality({ controlMean: 10, delta: 1, lo: 0.1, hi: 1.9, dz: 0.65 }, prereg.thresholds).material === false,
    '|dz| = 0.65 fails prereg 0.66 materiality gate',
  );
  expect(
    materiality(
      { controlMean: 10, delta: 1, lo: 0.1, hi: 1.9, dz: 0.65 },
      { ...prereg.thresholds, min_abs_paired_dz: 0.60 },
    ).material === true,
    'D5: mutating prereg.thresholds.min_abs_paired_dz changes materiality outcome',
  );

  // 2. Pooled baseline and C1..C5 gate arithmetic (QD-SEL2g) + D5 threshold reading.
  const s15Baseline = [
    { seed: 799501, completed: true, extinct_tick: 8545, crop_second_half: 153586.9 },
    { seed: 799502, completed: true, extinct_tick: null, crop_second_half: 113036.9 },
    { seed: 799503, completed: true, extinct_tick: 20787, crop_second_half: 153586.9 },
    { seed: 799504, completed: true, extinct_tick: null, crop_second_half: 113400.0 },
    { seed: 799505, completed: true, extinct_tick: null, crop_second_half: 113800.0 },
    { seed: 799506, completed: true, extinct_tick: null, crop_second_half: 114042.0 },
  ];
  const pool4 = evaluatePooledBaseline(s15Baseline, 3);
  expect(pool4.n_surviving === 4 && pool4.sufficient === true, '4/6 surviving baseline passes >= 3');
  const expectedPoolCrop = (113036.9 + 113400.0 + 113800.0 + 114042.0) / 4;
  expect(close(pool4.pooled_crop_second_half, expectedPoolCrop), 'extinct baselines excluded from pool');

  const mkArm = (arm, overrides = {}) => ({
    arm,
    completed: true,
    extinct_tick: null,
    min_agents: 15,
    mean_agents: 100,
    crop_second_half: expectedPoolCrop * 0.5,
    total_births: 3500,
    total_deaths: 3500,
    mean_lifespan: 5000,
    total_starvation: 2500,
    total_dehydration: 1000,
    cause_partition_ok: true,
    births_last_quarter: 800,
    deaths_last_quarter: 800,
    ...overrides,
  });
  const driftArm = mkArm('scarce_drift', {
    total_starvation: null,
    total_dehydration: null,
    cause_partition_ok: null,
  });
  const passGate = evaluateSmokeGate(200000, pool4, mkArm('selected'), mkArm('scarce_mut0'), driftArm, prereg.smoke_gate);
  expect(passGate.gate_pass === true && close(passGate.g_selected, 0.5), 'hand-built S15 gate passes C1..C5');
  const widenedGate = evaluateSmokeGate(
    200000,
    pool4,
    mkArm('selected', { crop_second_half: expectedPoolCrop * 0.62 }),
    mkArm('scarce_mut0'),
    driftArm,
    { ...prereg.smoke_gate, c2_g_band: [0.35, 0.65] },
  );
  expect(widenedGate.c2 === true, 'D5: mutating prereg.smoke_gate.c2_g_band alters C2');

  const pool2 = evaluatePooledBaseline(s15Baseline.slice(0, 3), 3);
  const failPool = evaluateSmokeGate(200000, pool2, mkArm('selected'), mkArm('scarce_mut0'), driftArm, prereg.smoke_gate);
  expect(pool2.sufficient === false && failPool.c2 === false && failPool.gate_pass === false, '< 3 surviving baseline fails C2');

  const failC3 = evaluateSmokeGate(
    2000,
    pool4,
    mkArm('selected', { total_births: 40, total_deaths: 40, mean_lifespan: 1000 }),
    mkArm('scarce_mut0', { total_births: 40, total_deaths: 40, mean_lifespan: 1000 }),
    driftArm,
    prereg.smoke_gate,
  );
  expect(failC3.c3 === false && failC3.gate_pass === false, '2,000-tick run fails C3 (< 30 generations)');

  const failC4 = evaluateSmokeGate(
    200000,
    pool4,
    mkArm('selected', { total_starvation: 1750, total_dehydration: 1750 }),
    mkArm('scarce_mut0'),
    driftArm,
    prereg.smoke_gate,
  );
  expect(failC4.c4 === false, 'starvation share == 0.50 fails strict > 0.50 C4');

  // 3. Filesystem + D1..D7 tamper controls on synthetic pipeline-check and confirm bundles.
  const root = mkdtempSync(join(tmpdir(), 'sel2p-verify-selftest-'));
  const runCli = (dir) => {
    try {
      execFileSync(process.execPath, [fileURLToPath(import.meta.url), dir], {
        stdio: ['ignore', 'pipe', 'pipe'],
      });
      return { code: 0, err: '' };
    } catch (e) {
      return { code: e.status ?? 1, err: String(e.stderr) };
    }
  };
  try {
    const empty = join(root, 'empty');
    mkdirSync(empty, { recursive: true });
    const r = runCli(empty);
    expect(r.code === 1 && /required file .* is missing/.test(r.err), 'existing empty directory fails');

    const partial = join(root, 'partial');
    mkdirSync(partial, { recursive: true });
    writeFileSync(join(partial, 'provenance.json'), '{"mode":"pipeline-check"}\n');
    const p = runCli(partial);
    expect(p.code === 1 && /is missing/.test(p.err), 'partial directory fails');

    const smokeBlind = join(root, 'smoke-blind');
    mkdirSync(smokeBlind, { recursive: true });
    const files = {
      'runs.csv': 'seed,arm\n',
      'smoke-series.csv': 'seed,arm,tick,live.trait_mass_mean\n',
      'smoke-report.json': '{"mode":"pipeline-check","contains_body_mass":false,"baseline_computed_before_scarce":true,"runs":[]}\n',
      'provenance.json': '{"mode":"pipeline-check","contains_body_mass":false,"baseline_computed_before_scarce":true,"build":{"profile":"release"}}\n',
      'summary.md': '# smoke\n',
    };
    const sums = [];
    for (const [n, t] of Object.entries(files)) {
      writeFileSync(join(smokeBlind, n), t);
      sums.push(`${sha256(Buffer.from(t))}  ${n}`);
    }
    writeFileSync(join(smokeBlind, 'checksums.sha256'), `${sums.join('\n')}\n`);
    failures.length = 0;
    verifySmoke(smokeBlind, 'smoke', prereg);
    expect(failures.some((f) => /body-mass observable/.test(f)), 'smoke naming trait_mass fails blindness check');
    failures.length = 0;

    const absent = runCli(join(root, 'does-not-exist'));
    expect(absent.code === 0, 'non-existent directory exits 0 (no artifacts yet)');

    // Build complete valid synthetic pipeline-check and FROZEN confirm directories.
    const pcDir = join(root, 'valid-pipeline');
    writeSyntheticFixture(pcDir, prereg, 'pipeline-check');
    expect(runCli(pcDir).code === 0, 'D7a: valid pipeline-check bundle with smoke-decisions.json passes');

    const frozenPrereg = { ...prereg, status: 'FROZEN' };
    const cfgDir = join(root, 'valid-confirm');
    writeSyntheticFixture(cfgDir, frozenPrereg, 'confirm');
    expect(runTamperTest(cfgDir, frozenPrereg, () => {}).length === 0, 'valid synthetic confirm bundle passes');

    // Also confirm smoke-only + smoke-decisions passes when outcome matches gate.
    const smokeOnlyDir = join(root, 'smoke-only');
    mkdirSync(smokeOnlyDir, { recursive: true });
    cpSync(join(cfgDir, 'smoke'), join(smokeOnlyDir, 'smoke'), { recursive: true });
    cpSync(join(cfgDir, 'smoke-decisions.json'), join(smokeOnlyDir, 'smoke-decisions.json'));
    failures.length = 0;
    verifyAll(smokeOnlyDir, prereg);
    expect(failures.length === 0, 'D7a: valid smoke-only driver directory with smoke-decisions.json passes');

    // D1a: swap H1 control and treatment in per-seed-deltas.csv, make effects.json consistent, rehash.
    const d1a = runTamperTest(pcDir, prereg, (d) => {
      const rows = readFileSync(join(d, 'per-seed-deltas.csv'), 'utf8').split('\n');
      const swapped = rows.map((line) => {
        if (!line.includes(',H1_selected_vs_scarce_drift,')) return line;
        const parts = line.split(',');
        const [seed, contrast, stat, c, t] = parts;
        return `${seed},${contrast},${stat},${t},${c},${Number(c) - Number(t)}`;
      });
      writeFileSync(join(d, 'per-seed-deltas.csv'), swapped.join('\n'));
      const eff = readJson(join(d, 'effects.json'));
      const cm = eff.h1.control_mean;
      eff.h1.control_mean = eff.h1.treatment_mean;
      eff.h1.treatment_mean = cm;
      eff.h1.paired_mean_delta = -eff.h1.paired_mean_delta;
      eff.h1.paired_dz = -eff.h1.paired_dz;
      const hi = eff.h1.ci95_high;
      eff.h1.ci95_high = -eff.h1.ci95_low;
      eff.h1.ci95_low = -hi;
      writeFileSync(join(d, 'effects.json'), `${JSON.stringify(eff, null, 2)}\n`);
      rehashDir(d);
    });
    expect(d1a.some((f) => /control_value .* != verified endpoint/.test(f)), 'D1a tamper: swapped H1 control/treatment caught');

    // D1b: blank cell in runs.csv must fail, never skip.
    const d1b = runTamperTest(pcDir, prereg, (d) => {
      const csv = readFileSync(join(d, 'runs.csv'), 'utf8');
      writeFileSync(join(d, 'runs.csv'), csv.replace(',4,2,4,', ',,2,4,'));
      rehashDir(d);
    });
    expect(d1b.some((f) => /post_warmup_trait_mass_mean/.test(f)), 'D1b tamper: empty runs.csv cell fails (never skips)');

    // D1c: mutate scarce_mut0 endpoint across runs.csv + per-seed-deltas.csv + effects.json while leaving mut0-runs.json intact.
    const d1c = runTamperTest(pcDir, prereg, (d) => {
      writeFileSync(join(d, 'runs.csv'), readFileSync(join(d, 'runs.csv'), 'utf8').replaceAll(',3.2,1.5,3.2,', ',3.9,1.5,3.2,'));
      const rows = readFileSync(join(d, 'per-seed-deltas.csv'), 'utf8').split('\n').map((line) => {
        if (!line.includes('scarce_mut0')) return line;
        const p = line.split(',');
        p[3] = '3.9';
        p[5] = String(Number(p[4]) - 3.9);
        return p.join(',');
      });
      writeFileSync(join(d, 'per-seed-deltas.csv'), rows.join('\n'));
      rehashDir(d);
    });
    expect(d1c.some((f) => /scarce_mut0 .* post_warmup_trait_mass_mean/.test(f)), 'D1c tamper: forged scarce_mut0 endpoint caught against mut0-runs.json');

    // D1d: inject non-zero trait_mass_var into mut0-runs.json with empty integrity_findings.
    const d1d = runTamperTest(pcDir, prereg, (d) => {
      const mut0 = readJson(join(d, 'mut0-runs.json'));
      const idx = mut0.runs[0].series[0].observables.findIndex(([k]) => k === 'live.trait_mass_var');
      mut0.runs[0].series[0].observables[idx][1] = 0.25;
      writeFileSync(join(d, 'mut0-runs.json'), `${JSON.stringify(mut0, null, 2)}\n`);
      rehashDir(d);
    });
    expect(d1d.some((f) => /integrity findings/.test(f)), 'D1d tamper: non-zero scarce_mut0 trait_mass_var caught');

    // D1e: drop a surviving seed from H1 in per-seed-deltas.csv and update effects.json consistently.
    const d1e = runTamperTest(pcDir, prereg, (d) => {
      const filtered = readFileSync(join(d, 'per-seed-deltas.csv'), 'utf8')
        .split('\n')
        .filter((l) => !l.startsWith(`${prereg.seeds.pipeline_check.pair_seeds[0]},H1_selected_vs_scarce_drift,`))
        .join('\n');
      writeFileSync(join(d, 'per-seed-deltas.csv'), filtered);
      const eff = readJson(join(d, 'effects.json'));
      eff.h1.n_complete_pairs -= 1;
      eff.n_complete_pairs_used_by_h1 -= 1;
      writeFileSync(join(d, 'effects.json'), `${JSON.stringify(eff, null, 2)}\n`);
      rehashDir(d);
    });
    expect(d1e.some((f) => /H1 complete pairs mismatch|H1_selected_vs_scarce_drift per-seed-deltas seeds/.test(f)), 'D1e tamper: dropped surviving H1 pair caught');

    // D2a & D2b: forged headline decision caught in pipeline-check and confirm.
    const d2a = runTamperTest(pcDir, prereg, (d) => {
      const eff = readJson(join(d, 'effects.json'));
      eff.decision = 'shift_in_model';
      writeFileSync(join(d, 'effects.json'), `${JSON.stringify(eff, null, 2)}\n`);
      rehashDir(d);
    });
    expect(d2a.some((f) => /effects\.decision "shift_in_model" != recomputed "pipeline_check_only"/.test(f)), 'D2a tamper: forged shift_in_model in pipeline-check caught');

    const d2b = runTamperTest(cfgDir, frozenPrereg, (d) => {
      const eff = readJson(join(d, 'effects.json'));
      eff.decision = 'null';
      writeFileSync(join(d, 'effects.json'), `${JSON.stringify(eff, null, 2)}\n`);
      rehashDir(d);
    });
    expect(d2b.some((f) => /effects\.decision "null" != recomputed "shift_in_model"/.test(f)), 'D2b tamper: forged null on material confirm run caught');

    // D3a..D3e: confirm-mode hard stop and execution-lock tampering.
    const d3a = runTamperTest(cfgDir, frozenPrereg, (d) => {
      rmSync(join(d, 'smoke'), { recursive: true, force: true });
    });
    expect(d3a.some((f) => /smoke\/ is mandatory/.test(f)), 'D3a tamper: confirm without smoke/ fails');

    const d3b = runTamperTest(cfgDir, frozenPrereg, (d) => {
      const rep = readJson(join(d, 'smoke/smoke-report.json'));
      rep.gate.gate_pass = false;
      writeFileSync(join(d, 'smoke/smoke-report.json'), `${JSON.stringify(rep, null, 2)}\n`);
      rehashDir(join(d, 'smoke'));
    });
    expect(d3b.some((f) => /gate\.c4|gate_pass/.test(f)), 'D3b tamper: confirm with failing smoke gate_pass fails');

    const d3c = runTamperTest(cfgDir, frozenPrereg, (d) => {
      const lock = readJson(join(d, 'execution-lock.json'));
      delete lock.manifest_sha256[PREREG];
      const txt = `${JSON.stringify(lock, null, 2)}\n`;
      writeFileSync(join(d, 'execution-lock.json'), txt);
      const pJson = readJson(join(d, 'provenance.json'));
      pJson.execution_lock_sha256 = sha256(Buffer.from(txt));
      writeFileSync(join(d, 'provenance.json'), `${JSON.stringify(pJson, null, 2)}\n`);
      rehashDir(d);
    });
    expect(d3c.some((f) => /manifest_sha256 keys/.test(f)), 'D3c tamper: execution-lock missing manifest hash fails');

    const d3d = runTamperTest(cfgDir, frozenPrereg, (d) => {
      const lock = readJson(join(d, 'execution-lock.json'));
      lock.pooled_baseline_crop += 10;
      const txt = `${JSON.stringify(lock, null, 2)}\n`;
      writeFileSync(join(d, 'execution-lock.json'), txt);
      const pJson = readJson(join(d, 'provenance.json'));
      pJson.execution_lock_sha256 = sha256(Buffer.from(txt));
      writeFileSync(join(d, 'provenance.json'), `${JSON.stringify(pJson, null, 2)}\n`);
      rehashDir(d);
    });
    expect(d3d.some((f) => /pooled_baseline_crop/.test(f)), 'D3d tamper: lock pooled crop disagreeing with smoke fails');

    const d3e = runTamperTest(cfgDir, frozenPrereg, (d) => {
      const pJson = readJson(join(d, 'provenance.json'));
      pJson.execution_lock_sha256 = '0'.repeat(64);
      writeFileSync(join(d, 'provenance.json'), `${JSON.stringify(pJson, null, 2)}\n`);
      rehashDir(d);
    });
    expect(d3e.some((f) => /execution_lock_sha256/.test(f)), 'D3e tamper: mismatched provenance.execution_lock_sha256 fails');

    // D4a & D4b: wrong duration/period and gapped sample ticks caught.
    const d4a = runTamperTest(pcDir, prereg, (d) => {
      const pJson = readJson(join(d, 'provenance.json'));
      pJson.duration.duration_ticks_as_run = 1000;
      writeFileSync(join(d, 'provenance.json'), `${JSON.stringify(pJson, null, 2)}\n`);
      rehashDir(d);
    });
    expect(d4a.some((f) => /provenance duration\/warmup\/period/.test(f)), 'D4a tamper: wrong analysis duration vs prereg fails');

    const d4b = runTamperTest(pcDir, prereg, (d) => {
      const rep = readJson(join(d, 'paired-report.json'));
      rep.pairs[0].treatment.series.shift();
      writeFileSync(join(d, 'paired-report.json'), `${JSON.stringify(rep, null, 2)}\n`);
      rehashDir(d);
    });
    expect(d4b.some((f) => /series length/.test(f)), 'D4b tamper: dropped sample in analysis series fails');

    // D7b: tampered smoke-decisions.json caught.
    const d7b = runTamperTest(smokeOnlyDir, prereg, (d) => {
      const dec = readJson(join(d, 'smoke-decisions.json'));
      dec.outcome = 'stopped';
      writeFileSync(join(d, 'smoke-decisions.json'), `${JSON.stringify(dec, null, 2)}\n`);
    });
    expect(d7b.some((f) => /smoke-decisions: outcome/.test(f)), 'D7b tamper: forged smoke-decisions.json outcome fails');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
  return bad === 0 ? 0 : 1;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const selfTest = process.argv[2] === '--self-test';
  if (selfTest) {
    process.exit(runSelfTest());
  }
  const artifactDir = resolve(
    process.argv[2] ?? join(repoRoot, 'artifacts/experiments/sel2-p-food-scarcity'),
  );
  if (!existsSync(artifactDir)) {
    console.log(`no artifacts yet: ${artifactDir} does not exist.`);
    process.exit(0);
  }
  try {
    verifyAll(artifactDir);
  } catch (e) {
    fail(`verification aborted: ${e instanceof Error ? e.message : String(e)}`);
  }
  for (const line of notes) console.log(`note  ${line}`);
  if (failures.length > 0) {
    console.error('');
    for (const line of failures) console.error(`FAIL  ${line}`);
    console.error(`\n${failures.length} failure(s).`);
    process.exit(1);
  }
  console.log(`\nOK - every SEL2-P artifact in ${artifactDir} verifies against node:crypto and sel2p-preregistration.json.`);
}
