import { describe, it, expect } from 'vitest';
import * as fs from 'fs';
import * as path from 'path';
import { runEmpiricalBenchmark } from './gpu_instancing_lod_benchmark';

describe('Empirical Benchmark Exporter', () => {
  it('runs benchmark and exports metrics to workspace JSON', () => {
    const metrics = runEmpiricalBenchmark();
    const outputPath = path.resolve('e:/project/Anima-Engine/.agents/challenger_2_m3/benchmark_results.json');
    fs.writeFileSync(outputPath, JSON.stringify(metrics, null, 2), 'utf-8');
    expect(fs.existsSync(outputPath)).toBe(true);
    expect(metrics.ecoClusteringValidation.passed).toBe(true);
    expect(metrics.frameTickLatency.passed).toBe(true);
  });
});
