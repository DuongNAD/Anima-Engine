// scripts/bench_baseline.mjs
//
// M0.4 — reproducible benchmark BASELINE SCAFFOLD (Anima-Engine).
//
// The full Bevy/Tauri backend must NOT be launched here: running it
// (`npm run tauri dev` / `cargo run`) has CRASHED the dev machine, and even a
// GPU-device probe is off-limits (WORLD_SIMULATION_PLAN.md §10.2 note). So this
// script captures the *reproducibility envelope* — seed, config, hardware — and
// times a deliberately CHEAP, pure-CPU PROXY loop instead of a real tick.
//
// The proxy is an fBm-like arithmetic loop over a 128² grid. It is NOT the real
// terrain generator and its numbers are NOT the engine's numbers; it exists only
// to give the report a non-empty, reproducible `timings` block and to prove the
// harness works. Replace it, on the target hardware, with a real
// `cargo test --release` terrain-gen timing or an in-app tick capture (see
// BENCHMARK_BASELINE.md). Perf numbers stay UNLOCKED until then (plan §10.2).
//
// Dependencies: Node built-ins only (`node:os`, `node:fs`, `node:process`) plus
// the `URL` and `performance` globals. No third-party packages, no Date.now().
//
// Run:  node scripts/bench_baseline.mjs
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
  'PROXY ONLY — a cheap 128^2 fBm-like arithmetic loop on the CAPTURE machine, ' +
  'NOT the real terrain generator and NOT the target hardware. Replace with a ' +
  '`cargo test --release` terrain-gen timing or an in-app tick capture on the ' +
  'Dell Vostro 3530 (i7-1355U). See BENCHMARK_BASELINE.md; numbers are not locked ' +
  '(WORLD_SIMULATION_PLAN.md §10.2).';

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
    'Timings above are PROXY measurements from the machine that ran this script, ' +
    'not the engine and not the target hardware. Baseline perf numbers are NOT ' +
    'locked until captured on the Vostro 3530 (WORLD_SIMULATION_PLAN.md §10.2).',
};

// Write at repo root (parent of scripts/), resolved off this file — no path module
// needed, and fs accepts a file: URL directly on every platform.
const outUrl = new URL('../benchmark_report.json', import.meta.url);
fs.writeFileSync(outUrl, `${JSON.stringify(report, null, 2)}\n`);

process.stdout.write(
  `benchmark_report.json written (seed=${seed}, ` +
    `terrain_fbm_proxy=${terrain.ms}ms/${terrain.iterations} iters, ` +
    `field_fbm_proxy=${field.ms}ms/${field.iterations} iters). PROXY numbers — ` +
    `capture real timings on the target hardware before treating them as a baseline.\n`,
);
