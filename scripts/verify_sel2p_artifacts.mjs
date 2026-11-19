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

export function materiality(st) {
  const ci = st.lo !== null && st.hi !== null && (st.lo > 0 || st.hi < 0);
  const dz = st.dz !== null && Math.abs(st.dz) >= 0.66;
  const rel =
    st.delta !== null &&
    st.controlMean !== null &&
    Math.abs(st.delta) >= 0.05 * Math.abs(st.controlMean);
  return { ci, dz, rel, material: ci && dz && rel };
}

export function evaluatePooledBaseline(runs, minSurviving = 3) {
  const surviving = runs.filter(
    (r) => r.completed && (r.extinct_tick === null || r.extinct_tick === undefined) && Number.isFinite(r.crop_second_half),
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

export function evaluateSmokeGate(durationTicks, baseline, selected, scarceMut0, scarceDrift) {
  const survived = (x) => Boolean(x) && x.completed && (x.extinct_tick === null || x.extinct_tick === undefined);
  const c1Arm = (x) => survived(x) && Number.isFinite(x.min_agents) && x.min_agents >= 5.0;
  const c1 = c1Arm(selected) && c1Arm(scarceMut0);

  const refCrop =
    baseline.sufficient && Number.isFinite(baseline.pooled_crop_second_half) && baseline.pooled_crop_second_half > 0
      ? baseline.pooled_crop_second_half
      : null;
  const gOf = (x) =>
    refCrop !== null && x && Number.isFinite(x.crop_second_half) ? x.crop_second_half / refCrop : null;
  const gSelected = gOf(selected);
  const gMut0 = gOf(scarceMut0);
  const gDrift = gOf(scarceDrift);
  const inBand = (g) => g !== null && g >= 0.4 && g <= 0.6;
  const c2 = Boolean(baseline.sufficient) && inBand(gSelected) && inBand(gMut0);

  const gensOf = (x) => {
    if (!x || !(x.total_deaths > 0) || !Number.isFinite(x.mean_lifespan) || x.mean_lifespan <= 0) {
      return null;
    }
    if (!Number.isFinite(x.mean_agents) || x.mean_agents <= 0) return null;
    return Math.min(durationTicks / x.mean_lifespan, x.total_births / Math.max(1.0, x.mean_agents));
  };
  const gensSel = gensOf(selected);
  const gensMut0 = gensOf(scarceMut0);
  const c3 = gensSel !== null && gensSel >= 30.0 && gensMut0 !== null && gensMut0 >= 30.0;

  const starvShare = (x) => {
    if (!x || !(x.total_deaths > 0) || x.cause_partition_ok !== true) return null;
    if (!Number.isFinite(x.total_starvation) || !Number.isFinite(x.total_dehydration)) return null;
    return x.total_starvation / x.total_deaths;
  };
  const shareSel = starvShare(selected);
  const shareMut0 = starvShare(scarceMut0);
  const c4 = shareSel !== null && shareSel > 0.5 && shareMut0 !== null && shareMut0 > 0.5;

  const c5Arm = (x) => Boolean(x) && x.births_last_quarter > 0 && x.deaths_last_quarter > 0;
  const c5 = c5Arm(selected) && c5Arm(scarceMut0);
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

export function summarizeSmokeSeries(seriesRows, durationTicks, reportRuns) {
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
      const n = Number(r['live.agent_count']);
      if (Number.isFinite(n)) {
        minAgents = Math.min(minAgents, n);
        agentsSum += n;
        agentsN += 1;
        if (n === 0 && extinctTick === null) extinctTick = tick;
      }
      const crop = Number(r['live.standing_crop']);
      if (tick * 2 > durationTicks && Number.isFinite(crop)) {
        cropSum += crop;
        cropN += 1;
      }
      const b = Number(r['live.births'] || 0);
      const d = Number(r['live.deaths'] || 0);
      const ls = Number(r['live.mean_lifespan'] || 0);
      totalBirths += b;
      totalDeaths += d;
      lifespanWeighted += ls * d;
      if (tick * 4 > durationTicks * 3) {
        birthsLq += b;
        deathsLq += d;
      }
      if (isDrift) {
        if (r['live.deaths_starvation'] !== '' || r['live.deaths_dehydration'] !== '') {
          fail(`smoke: scarce_drift seed ${entry.seed} tick ${tick} emitted cause-split column`);
        }
      } else {
        const s = Number(r['live.deaths_starvation'] || 0);
        const h = Number(r['live.deaths_dehydration'] || 0);
        totalStarv += s;
        totalDehyd += h;
        if (Math.abs(s + h - d) > 1e-6) partOk = false;
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

function verifyNothingUncovered(dir, covered, label) {
  const skip = new Set(['checksums.sha256', 'RESULT.md']);
  const walk = (rel) => {
    for (const entry of readdirSync(join(dir, rel), { withFileTypes: true })) {
      const child = rel ? `${rel}/${entry.name}` : entry.name;
      if (entry.isDirectory()) {
        if (child === 'smoke' || child === 'replay' || child === 'superseded') continue;
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
    const committed = readCommitted(entry);
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
  if (!requireFiles(dir, label, SMOKE_FILES, [])) return;
  const covered = verifyChecksums(dir, label);
  verifyNothingUncovered(dir, covered, label);

  for (const entry of readdirSync(dir)) {
    const text = readFileSync(join(dir, entry), 'utf8');
    if (text.includes('trait_mass')) {
      fail(`${label}: ${entry} names a body-mass observable; smoke must remain blind`);
    }
  }
  for (const banned of [
    'effects.json',
    'paired-report.json',
    'per-seed-deltas.csv',
    'manifests',
    'execution-lock.json',
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

  const isPipeline = report.mode === 'pipeline-check';
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

  const seriesText = readFileSync(join(dir, 'smoke-series.csv'), 'utf8');
  const rows = parseCsv(seriesText);
  const summarized = summarizeSmokeSeries(rows, report.duration_ticks, report.runs ?? []);
  const recomputedPool = evaluatePooledBaseline(
    summarized.baseline,
    prereg.seeds.min_surviving_baseline_seeds,
  );
  const recomputedGate = evaluateSmokeGate(
    report.duration_ticks,
    recomputedPool,
    summarized.scarce.selected,
    summarized.scarce.scarce_mut0,
    summarized.scarce.scarce_drift,
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
}

function verifyAnalysisRecompute(dir, label, prereg) {
  const prov = readJson(join(dir, 'provenance.json'));
  const report = readJson(join(dir, 'paired-report.json'));
  const effects = readJson(join(dir, 'effects.json'));
  const runs = parseCsv(readFileSync(join(dir, 'runs.csv'), 'utf8'));
  const deltasCsv = parseCsv(readFileSync(join(dir, 'per-seed-deltas.csv'), 'utf8'));

  const isConfirm = prov.mode === 'confirm';
  const expectedSeeds = isConfirm ? prereg.seeds.execution_order : prereg.seeds.pipeline_check.pair_seeds;
  if (JSON.stringify(report.seed_order) !== JSON.stringify(expectedSeeds)) {
    fail(`${label}: seed_order ${JSON.stringify(report.seed_order)} != ${JSON.stringify(expectedSeeds)}`);
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
  if (effects.integrity_findings?.length) {
    fail(`${label}: integrity findings present: ${JSON.stringify(effects.integrity_findings)}`);
  }

  // Recompute extinction / failure drops and complete-pair counts against effects.json.
  const extinctRuns = runs.filter((r) => r.extinct === 'true');
  const failedRuns = runs.filter((r) => r.status !== 'completed' && r.status !== 'Completed');
  const droppedH1Seeds = new Set(
    [...extinctRuns, ...failedRuns]
      .filter((r) => r.arm === 'selected' || r.arm === 'scarce_drift')
      .map((r) => Number(r.seed)),
  );
  const listedExtinct = effects.extinction?.extinct_runs ?? [];
  const extKey = (x) => `${x.seed}|${x.arm}|${x.tick}`;
  const csvExtSet = new Set(extinctRuns.map((r) => `${r.seed}|${r.arm}|${r.extinct_tick}`));
  const effExtSet = new Set(listedExtinct.map(extKey));
  for (const k of csvExtSet) {
    if (!effExtSet.has(k)) fail(`${label}: runs.csv extinct ${k} missing from effects.json`);
  }
  for (const k of effExtSet) {
    if (!csvExtSet.has(k)) fail(`${label}: effects.json extinct ${k} missing from runs.csv`);
  }
  if (effects.extinction?.n_extinct_runs_total !== listedExtinct.length) {
    fail(`${label}: extinction.n_extinct_runs_total mismatch`);
  }
  const completeH1Pairs = report.pairs.filter(
    (p) =>
      p.control.status === 'Completed' &&
      p.treatment.status === 'Completed' &&
      !droppedH1Seeds.has(p.seed),
  ).length;
  if (completeH1Pairs !== effects.n_complete_pairs_used_by_h1) {
    fail(
      `${label}: effects.n_complete_pairs_used_by_h1 (${effects.n_complete_pairs_used_by_h1}) != ${completeH1Pairs}`,
    );
  }
  if (isConfirm && effects.sufficient_pairs !== (completeH1Pairs >= prereg.seeds.min_complete_pairs_for_a_decision)) {
    fail(`${label}: effects.sufficient_pairs mismatch`);
  }

  // Recompute per-seed endpoints from paired-report.json series (selected & scarce_drift).
  const warmup = prov.duration?.warmup_ticks_as_run;
  const duration = prov.duration?.duration_ticks_as_run;
  const runRowByKey = new Map(runs.map((r) => [`${r.arm}|${r.seed}`, r]));
  for (const pair of report.pairs) {
    for (const [arm, rawRun] of [
      ['selected', pair.treatment],
      ['scarce_drift', pair.control],
    ]) {
      const row = runRowByKey.get(`${arm}|${pair.seed}`);
      if (!row) {
        fail(`${label}: runs.csv lacks ${arm} seed ${pair.seed}`);
        continue;
      }
      const massSamples = rawRun.series
        .filter((s) => s.tick > warmup)
        .map((s) => s.observables.find(([k]) => k === 'live.trait_mass_mean')?.[1])
        .filter((v) => v !== undefined);
      if (massSamples.length > 0 && row.post_warmup_trait_mass_mean !== '') {
        const pwMass = massSamples.reduce((a, b) => a + b, 0) / massSamples.length;
        if (!close(Number(row.post_warmup_trait_mass_mean), pwMass)) {
          fail(`${label}: ${arm} seed ${pair.seed} post_warmup_trait_mass_mean mismatch`);
        }
      }
      const tailSamples = rawRun.series
        .filter((s) => s.tick * 5 > duration * 4)
        .map((s) => s.observables.find(([k]) => k === 'live.trait_mass_mean')?.[1])
        .filter((v) => v !== undefined);
      if (tailSamples.length > 0 && row.tail20_trait_mass_mean !== '') {
        const tailMass = tailSamples.reduce((a, b) => a + b, 0) / tailSamples.length;
        if (!close(Number(row.tail20_trait_mass_mean), tailMass)) {
          fail(`${label}: ${arm} seed ${pair.seed} tail20_trait_mass_mean mismatch`);
        }
      }
      const iWarm = rawRun.series
        .find((s) => s.tick === warmup)
        ?.observables.find(([k]) => k === 'live.mean_energy_integral')?.[1];
      const iEnd = rawRun.series
        .find((s) => s.tick === duration)
        ?.observables.find(([k]) => k === 'live.mean_energy_integral')?.[1];
      if (iWarm !== undefined && iEnd !== undefined && row.post_warmup_mean_energy_integral !== '') {
        const pwEnergy = (iEnd - iWarm) / (duration - warmup);
        if (!close(Number(row.post_warmup_mean_energy_integral), pwEnergy)) {
          fail(`${label}: ${arm} seed ${pair.seed} post_warmup_mean_energy_integral mismatch`);
        }
      }
    }
  }

  // Recompute per-seed-deltas.csv arithmetic and pairedStats for all contrasts.
  for (const row of deltasCsv) {
    const c = Number(row.control_value);
    const t = Number(row.treatment_value);
    const d = Number(row.delta);
    if (!close(t - c, d)) {
      fail(`${label}: ${row.contrast} seed ${row.seed}: ${t} - ${c} != ${d}`);
    }
  }
  const contrasts = [
    ['H1_selected_vs_scarce_drift', effects.h1, true],
    ['H2a_selected_vs_scarce_mut0', effects.h2a_selected_vs_scarce_mut0, false],
    ['H2b_scarce_drift_vs_scarce_mut0', effects.h2b_scarce_drift_vs_scarce_mut0, false],
    ['R1_tail20_selected_vs_scarce_drift', effects.r1_tail20, false],
    ['R2_window_net_selected_vs_scarce_drift', effects.r2_window_net_selection, false],
    ['S2_energy_selected_vs_scarce_drift', effects.s2_energy_post_warmup, false],
  ];
  for (const [contrastName, block, checkMaterial] of contrasts) {
    const sub = deltasCsv.filter((r) => r.contrast === contrastName);
    const st = pairedStats(
      sub.map((r) => Number(r.control_value)),
      sub.map((r) => Number(r.treatment_value)),
    );
    if (!block) {
      fail(`${label}: effects.json missing contrast block for ${contrastName}`);
      continue;
    }
    if (block.n_complete_pairs !== st.n) {
      fail(`${label}: ${contrastName} n_complete_pairs ${block.n_complete_pairs} != ${st.n}`);
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
        fail(`${label}: ${contrastName}.${field} (${block[field]}) != recomputed ${got}`);
      }
    }
    if (checkMaterial) {
      const m = materiality(st);
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
  note(`${label}: mode=${prov.mode}, H1 pairs=${effects.h1?.n_complete_pairs}, decision=${effects.decision}`);
}

function verifyAll(dir) {
  const prereg = committedPrereg();
  const provPath = join(dir, 'provenance.json');
  const hasAnalysis = existsSync(provPath);
  const smokeDir = join(dir, 'smoke');
  const hasSmoke = existsSync(smokeDir);

  if (!hasAnalysis && !hasSmoke) {
    requireFiles(dir, 'analysis', ROOT_FILES, Object.values(prereg.manifests));
    return;
  }

  if (hasAnalysis) {
    const prov = readJson(provPath);
    const req = prov.mode === 'confirm' ? [...ROOT_FILES, 'execution-lock.json'] : ROOT_FILES;
    if (requireFiles(dir, 'analysis', req, Object.values(prereg.manifests))) {
      const covered = verifyChecksums(dir, 'analysis');
      if (prov.mode === 'confirm' && !covered.has('execution-lock.json')) {
        fail('analysis: checksums.sha256 must cover execution-lock.json in confirm mode');
      }
      verifyNothingUncovered(dir, covered, 'analysis');
      verifyManifestsAreCommitted(dir, 'analysis');
      verifyAnalysisRecompute(dir, 'analysis', prereg);
    }
  }
  if (hasSmoke) {
    verifySmoke(smokeDir, 'smoke', prereg);
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

  // 1. Paired statistics and 0.66 dz materiality threshold.
  const st = pairedStats([0, 0, 0], [1, 2, 3]);
  expect(close(st.delta, 2) && close(st.sd, 1) && close(st.se, 1 / Math.sqrt(3)), 'mean/sd/se of 1,2,3');
  expect(materiality(st).material === false, 'df=2 wide CI crossing 0 is not material');
  expect(
    materiality(pairedStats([10, 10, 10, 10], [11, 11.2, 10.9, 11.1])).material === true,
    'consistent 10% shift with |dz| >= 0.66 is material',
  );
  // |dz| = 0.5 < 0.66 even when CI excludes zero? Check threshold enforcement directly:
  expect(
    materiality({ controlMean: 10, delta: 1, lo: 0.1, hi: 1.9, dz: 0.65 }).material === false,
    '|dz| = 0.65 fails 0.66 materiality gate',
  );
  expect(
    materiality({ controlMean: 10, delta: 1, lo: 0.1, hi: 1.9, dz: 0.66 }).material === true,
    '|dz| = 0.66 passes 0.66 materiality gate when CI and 5% pass',
  );

  // 2. Pooled baseline and C1..C5 gate arithmetic (QD-SEL2g).
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
  const passGate = evaluateSmokeGate(200000, pool4, mkArm('selected'), mkArm('scarce_mut0'), driftArm);
  expect(passGate.gate_pass === true && close(passGate.g_selected, 0.5), 'hand-built S15 gate passes C1..C5 even without drift cause columns');

  const pool2 = evaluatePooledBaseline(s15Baseline.slice(0, 3), 3);
  const failPool = evaluateSmokeGate(200000, pool2, mkArm('selected'), mkArm('scarce_mut0'), driftArm);
  expect(pool2.sufficient === false && failPool.c2 === false && failPool.gate_pass === false, '< 3 surviving baseline fails C2');

  const failC3 = evaluateSmokeGate(
    2000,
    pool4,
    mkArm('selected', { total_births: 40, total_deaths: 40, mean_lifespan: 1000 }),
    mkArm('scarce_mut0', { total_births: 40, total_deaths: 40, mean_lifespan: 1000 }),
    driftArm,
  );
  expect(failC3.c3 === false && failC3.gate_pass === false, '2,000-tick run fails C3 (< 30 generations)');

  const failC4 = evaluateSmokeGate(
    200000,
    pool4,
    mkArm('selected', { total_starvation: 1750, total_dehydration: 1750 }),
    mkArm('scarce_mut0'),
    driftArm,
  );
  expect(failC4.c4 === false, 'starvation share == 0.50 fails strict > 0.50 C4');

  // 3. Filesystem negative controls.
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

    const smoke = join(root, 'smoke');
    mkdirSync(smoke, { recursive: true });
    const files = {
      'runs.csv': 'seed,arm\n',
      'smoke-series.csv': 'seed,arm,tick,live.trait_mass_mean\n',
      'smoke-report.json': '{"contains_body_mass":false,"baseline_computed_before_scarce":true,"runs":[]}\n',
      'provenance.json': '{"contains_body_mass":false,"baseline_computed_before_scarce":true,"build":{"profile":"release"}}\n',
      'summary.md': '# smoke\n',
    };
    const sums = [];
    for (const [n, t] of Object.entries(files)) {
      writeFileSync(join(smoke, n), t);
      sums.push(`${sha256(Buffer.from(t))}  ${n}`);
    }
    writeFileSync(join(smoke, 'checksums.sha256'), `${sums.join('\n')}\n`);
    failures.length = 0;
    verifySmoke(smoke, 'smoke', committedPrereg());
    expect(failures.some((f) => /body-mass observable/.test(f)), 'smoke naming trait_mass fails blindness check');
    failures.length = 0;

    const absent = runCli(join(root, 'does-not-exist'));
    expect(absent.code === 0, 'non-existent directory exits 0 (no artifacts yet)');
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
