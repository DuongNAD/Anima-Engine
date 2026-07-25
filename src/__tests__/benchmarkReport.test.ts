// S04 — benchmark baseline report validator (M0.4).
//
// Self-contained and pure: no worldGen.ts / three / @react-three/fiber imports,
// no ajv, no filesystem walk. It reads the committed benchmark_report.json (via a
// resolveJsonModule import, which keeps strict tsc happy without @types/node) and
// checks the required fields exist with the right shapes. It also proves the
// validator is real by feeding it inline objects that are missing `seed` or
// `hardware` and asserting those FAIL — so the test cannot silently pass on a
// broken report.
//
// Regenerate the report with:  node scripts/bench_baseline.mjs

import { describe, it, expect } from 'vitest';
import report from '../../benchmark_report.json';

/**
 * Tiny inline validator (no external schema lib). Returns the list of problems;
 * an empty list means the object satisfies the M0.4 baseline contract.
 */
function validateBenchmarkReport(obj: unknown): string[] {
  const errors: string[] = [];
  if (typeof obj !== 'object' || obj === null) {
    errors.push('report must be an object');
    return errors;
  }
  const r = obj as Record<string, unknown>;

  if (typeof r.seed !== 'number') {
    errors.push('seed must be a number');
  }

  const config = r.config;
  if (typeof config !== 'object' || config === null) {
    errors.push('config must be an object');
  } else {
    const c = config as Record<string, unknown>;
    if (typeof c.gridDim !== 'number') errors.push('config.gridDim must be a number');
    if (typeof c.tickHz !== 'number') errors.push('config.tickHz must be a number');
  }

  const hardware = r.hardware;
  if (typeof hardware !== 'object' || hardware === null) {
    errors.push('hardware must be an object');
  } else {
    const h = hardware as Record<string, unknown>;
    if (typeof h.cpuModel !== 'string') errors.push('hardware.cpuModel must be a string');
  }

  const timings = r.timings;
  if (typeof timings !== 'object' || timings === null) {
    errors.push('timings must be an object');
  } else if (Object.keys(timings as Record<string, unknown>).length === 0) {
    errors.push('timings must be non-empty');
  }

  return errors;
}

describe('S04 — benchmark_report.json baseline validator', () => {
  it('accepts the committed report and exposes the required fields', () => {
    expect(validateBenchmarkReport(report)).toEqual([]);

    // Spot-check the specific fields the contract names.
    const r = report as unknown as Record<string, unknown>;
    expect(typeof r.seed).toBe('number');

    const config = r.config as Record<string, unknown>;
    expect(typeof config.gridDim).toBe('number');
    expect(typeof config.tickHz).toBe('number');

    const hardware = r.hardware as Record<string, unknown>;
    expect(typeof hardware.cpuModel).toBe('string');

    const timings = r.timings as Record<string, unknown>;
    expect(Object.keys(timings).length).toBeGreaterThan(0);
  });

  it('rejects a report missing `seed`', () => {
    const invalid = {
      config: { gridDim: 128, tickHz: 60, ticksPerEpoch: 1000 },
      hardware: { cpuModel: 'Test CPU' },
      timings: { probe: { ms: 1, iterations: 1, note: 'x' } },
      // seed intentionally omitted
    };
    const errors = validateBenchmarkReport(invalid);
    expect(errors).toContain('seed must be a number');
    expect(errors.length).toBeGreaterThan(0);
  });

  it('rejects a report missing `hardware`', () => {
    const invalid = {
      seed: 1337,
      config: { gridDim: 128, tickHz: 60, ticksPerEpoch: 1000 },
      timings: { probe: { ms: 1, iterations: 1, note: 'x' } },
      // hardware intentionally omitted
    };
    const errors = validateBenchmarkReport(invalid);
    expect(errors).toContain('hardware must be an object');
    expect(errors.length).toBeGreaterThan(0);
  });

  it('rejects an empty timings block', () => {
    const invalid = {
      seed: 1337,
      config: { gridDim: 128, tickHz: 60, ticksPerEpoch: 1000 },
      hardware: { cpuModel: 'Test CPU' },
      timings: {},
    };
    expect(validateBenchmarkReport(invalid)).toContain('timings must be non-empty');
  });
});
