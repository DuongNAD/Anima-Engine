// scripts/bench_baseline.mjs
//
// M0.4 — reproducible benchmark BASELINE (Anima-Engine).
//
// The full Bevy/Tauri backend must NOT be launched here: running it
// (`npm run tauri dev` / `cargo run`) has CRASHED the dev machine, and even a
// GPU-device probe is off-limits (WORLD_SIMULATION_PLAN.md §10.2 note). So this
// script captures the *reproducibility envelope* — seed, config, hardware — and
// gathers timings from two sources:
//
//   1. REAL per-system measurements, read from Criterion's own output under
//      `src-tauri/target/criterion/**/new/estimates.json` (OSS-010). These come
//      from `cargo bench --bench tick_systems`, which drives one system at a time
//      headless — no Tauri, no window, no GPU device. See
//      docs/how-to/BENCHMARKING.md.
//   2. A cheap pure-CPU PROXY loop, kept as a harness self-test and as the anchor
//      for `proxyChecksum`. It is NOT the engine and never was.
//
// Ordering matters: run `cargo bench` FIRST, then this script. `target/` is
// gitignored, so a fresh clone has no Criterion data — and a proxy-only report
// overwriting a real one is a silent regression. The script refuses to do that
// unless ANIMA_BENCH_ALLOW_PROXY_ONLY=1 says it is intended.
//
// One consequence worth stating: real measurements vary run to run, so this file
// no longer diffs byte-stable. That stability was only ever available because
// nothing real was being measured. Values are rounded to keep the noise small.
//
// Dependencies: Node built-ins only (`node:os`, `node:fs`, `node:path`,
// `node:process`) plus the `URL` and `performance` globals. No third-party
// packages, no Date.now().
//
// Run:  cargo bench --bench tick_systems   (from src-tauri/, optional but expected)
//       node scripts/bench_baseline.mjs
// Out:  benchmark_report.json  (written at repo root, next to this script's parent)

import os from 'node:os';
import fs from 'node:fs';
import process from 'node:process';

// ---- knobs (overridable by env, defaults are the canonical M0 values) -------

const DEFAULT_SEED = 1337;
const parsedSeed = Number.parseInt(process.env.ANIMA_BENCH_SEED ?? '', 10);
const seed = Number.isFinite(parsedSeed) ? parsedSeed : DEFAULT_SEED;

// timestampNote is intentionally NOT Date.now(): the file must be byte-stable
// across re-runs so it diffs cleanly. Whoever captures a real run stamps it via
// ANIMA_BENCH_TIMESTAMP (e.g. an ISO date), otherwise it reads "set on capture".
const timestampNote = process.env.ANIMA_BENCH_TIMESTAMP ?? 'set on capture';

const GRID_DIM = 128; // DEFAULT_GRID_DIM (sim_rules.rs)
const TICK_HZ = 60; // TICK_HZ (sim_rules.rs)
const TICKS_PER_EPOCH = 1000; // TICKS_PER_EPOCH (sim_rules.rs)

// ---- the CHEAP proxy workload ----------------------------------------------

// A seeded integer hash → pseudo value in [-1, 1). Pure arithmetic, allocation
// free. Mirrors the *shape* of value-noise sampling, none of the real logic.
function hashUnit(x, y, s) {
  let h = (Math.imul(x, 374761393) ^ Math.imul(y, 668265263) ^ Math.imul(s, 2654435761)) | 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177) | 0;
  h ^= h >>> 16;
  return (h >>> 0) / 0x80000000 - 1; // → [-1, 1)
}

// fBm-like accumulation over a `dim`×`dim` grid, `octaves` deep. Returns a scalar
// checksum so the loop cannot be optimized away. This is the PROXY, not terrain.
function fbmProxy(dim, s, octaves) {
  let acc = 0;
  for (let y = 0; y < dim; y++) {
    for (let x = 0; x < dim; x++) {
      let amp = 1;
      let freq = 1;
      let v = 0;
      for (let o = 0; o < octaves; o++) {
        v += hashUnit(x * freq, y * freq, s + o) * amp;
        amp *= 0.5;
        freq *= 2;
      }
      acc += v;
    }
  }
  return acc;
}

const PROXY_NOTE =
  'HARNESS SELF-TEST, not an engine measurement — a cheap 128^2 fBm-like ' +
  'arithmetic loop. It is not the terrain generator and never was. Kept because it ' +
  'anchors proxyChecksum and proves the harness runs; the real per-system numbers ' +
  'are the criterion/* entries. See docs/how-to/BENCHMARKING.md.';

// ---- real measurements, read from Criterion ---------------------------------

// Criterion writes one estimates.json per benchmark under
// `target/criterion/<sanitised group>/<bench>[/<param>]/new/estimates.json`.
// Group names have `/` replaced by `_`, but a BenchmarkId parameter stays a
// directory — so the layout is walked rather than reconstructed, which keeps this
// working if Criterion changes how it sanitises a name.
const CRITERION_ROOT = new URL('../src-tauri/target/criterion/', import.meta.url);

function findEstimates(dirUrl, relative = []) {
  let entries;
  try {
    entries = fs.readdirSync(dirUrl, { withFileTypes: true });
  } catch {
    return []; // no criterion output at all
  }
  const found = [];
  for (const entry of entries) {
    if (!entry.isDirectory()) continue;
    const childUrl = new URL(`${encodeURIComponent(entry.name)}/`, dirUrl);
    if (entry.name === 'new') {
      const fileUrl = new URL('estimates.json', childUrl);
      if (fs.existsSync(fileUrl)) found.push({ name: relative.join('/'), fileUrl });
      continue;
    }
    // `base` holds the previous run and `report` holds rendered HTML; neither is a
    // measurement of this run.
    if (entry.name === 'base' || entry.name === 'report') continue;
    found.push(...findEstimates(childUrl, [...relative, entry.name]));
  }
  return found;
}

/// Criterion point estimates are in NANOSECONDS.
function readCriterionTimings() {
  const timings = {};
  for (const { name, fileUrl } of findEstimates(CRITERION_ROOT).sort((a, b) =>
    a.name.localeCompare(b.name),
  )) {
    let parsed;
    try {
      parsed = JSON.parse(fs.readFileSync(fileUrl, 'utf8'));
    } catch {
      continue; // a half-written file from an interrupted run is not a measurement
    }
    const medianNs = parsed?.median?.point_estimate;
    if (typeof medianNs !== 'number' || !Number.isFinite(medianNs)) continue;
    const meanNs = parsed?.mean?.point_estimate;
    const stdDevNs = parsed?.std_dev?.point_estimate;

    // `ms` is the PER-ITERATION median, so `iterations: 1` keeps the pair honest
    // against the schema's "ms over all iterations" wording. Sub-microsecond
    // benchmarks would round to 0.000 ms, so the nanosecond figures are carried
    // alongside and are the ones to read.
    timings[`criterion/${name}`] = {
      ms: Math.round((medianNs / 1e6) * 1e6) / 1e6,
      iterations: 1,
      note:
        'REAL measurement — Criterion median, release build, per iteration. ' +
        'Source: cargo bench --bench tick_systems. See docs/how-to/BENCHMARKING.md.',
      medianNs: Math.round(medianNs * 1e3) / 1e3,
      ...(typeof meanNs === 'number' ? { meanNs: Math.round(meanNs * 1e3) / 1e3 } : {}),
      ...(typeof stdDevNs === 'number' ? { stdDevNs: Math.round(stdDevNs * 1e3) / 1e3 } : {}),
    };
  }
  return timings;
}

function timeLoop(iterations, fn) {
  const t0 = performance.now();
  let checksum = 0;
  for (let i = 0; i < iterations; i++) checksum += fn(i);
  const t1 = performance.now();
  return { ms: Math.round((t1 - t0) * 1000) / 1000, iterations, checksum };
}

// Two independent proxy measurements so `timings` is non-trivial. Both cheap:
// together ~5M octave-ops, finishing far under a second on any modern CPU.
const terrain = timeLoop(50, (i) => fbmProxy(GRID_DIM, seed + i, 6));
const field = timeLoop(200, (i) => fbmProxy(GRID_DIM, seed ^ (i + 1), 3));

// Fold both checksums into one value that survives serialization → guarantees the
// JIT keeps the loops. It is NOT a metric; it just anchors reproducibility.
const proxyChecksum = Math.round((terrain.checksum + field.checksum) * 1e6) / 1e6;

// ---- assemble the report ----------------------------------------------------

const cpus = os.cpus();
const criterionTimings = readCriterionTimings();
const criterionCount = Object.keys(criterionTimings).length;

// Write at repo root (parent of scripts/), resolved off this file — no path module
// needed, and fs accepts a file: URL directly on every platform.
const outUrl = new URL('../benchmark_report.json', import.meta.url);

// Guard against the silent regression: `target/` is gitignored, so running this on a
// fresh clone finds no Criterion data. Without this check the script would cheerfully
// replace a committed report full of real measurements with proxies only, and the
// result would still validate, still look like a baseline, and be worthless. Opt in
// explicitly if proxy-only really is what you want.
if (criterionCount === 0 && process.env.ANIMA_BENCH_ALLOW_PROXY_ONLY !== '1') {
  const existingIsReal = (() => {
    try {
      const existing = JSON.parse(fs.readFileSync(outUrl, 'utf8'));
      return Object.keys(existing?.timings ?? {}).some((k) => k.startsWith('criterion/'));
    } catch {
      return false; // no report yet, or unreadable — there is nothing to protect
    }
  })();
  if (existingIsReal) {
    process.stderr.write(
      'refusing to overwrite real Criterion timings with a proxy-only report.\n' +
        'Run `cargo bench --bench tick_systems` from src-tauri/ first, or set\n' +
        'ANIMA_BENCH_ALLOW_PROXY_ONLY=1 if a proxy-only report is intended.\n',
    );
    process.exit(1);
  }
}

const report = {
  schemaVersion: 1,
  timestampNote,
  seed,
  config: {
    gridDim: GRID_DIM,
    tickHz: TICK_HZ,
    ticksPerEpoch: TICKS_PER_EPOCH,
  },
  hardware: {
    platform: os.platform(),
    release: os.release(),
    arch: os.arch(),
    cpuModel: (cpus[0]?.model ?? 'unknown').trim(),
    cpuCount: cpus.length,
    totalMemMB: Math.round(os.totalmem() / (1024 * 1024)),
  },
  timings: {
    ...criterionTimings,
    terrain_fbm_proxy: {
      ms: terrain.ms,
      iterations: terrain.iterations,
      note: PROXY_NOTE,
    },
    field_fbm_proxy: {
      ms: field.ms,
      iterations: field.iterations,
      note: PROXY_NOTE,
    },
  },
  // Reproducibility anchor (keeps the proxy loops alive; not a performance metric).
  proxyChecksum,
  notes:
    criterionCount > 0
      ? `The criterion/* entries are REAL per-system measurements on the target ` +
        `hardware (${(cpus[0]?.model ?? 'unknown').trim()}), from ` +
        `cargo bench --bench tick_systems. They are a LOWER BOUND on a tick, not a ` +
        `frame: brain inference, ECS scheduling, emit, collision and metabolism are ` +
        `not among them. The *_fbm_proxy entries are a harness self-test, not the ` +
        `engine. See docs/how-to/BENCHMARKING.md.`
      : 'PROXY ONLY — no Criterion output was found under src-tauri/target/criterion. ' +
        'Run `cargo bench --bench tick_systems` from src-tauri/ and re-run this ' +
        'script to capture real per-system numbers.',
};

fs.writeFileSync(outUrl, `${JSON.stringify(report, null, 2)}\n`);

process.stdout.write(
  `benchmark_report.json written (seed=${seed}, ` +
    `${criterionCount} real Criterion timings, ` +
    `terrain_fbm_proxy=${terrain.ms}ms/${terrain.iterations} iters, ` +
    `field_fbm_proxy=${field.ms}ms/${field.iterations} iters)` +
    (criterionCount === 0
      ? '. PROXY ONLY — run `cargo bench --bench tick_systems` first.\n'
      : '.\n'),
);
