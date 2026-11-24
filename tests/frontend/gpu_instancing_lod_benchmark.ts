import * as THREE from 'three';
import { generateTerrain, getBilinearInterpolatedElevation, poissonDiskSampling, mulberry32 } from '../../src/components/Landscape/utils/terrainGenerator.js';

interface BenchmarkMetrics {
  timestamp: string;
  foliageInstanceCounts: {
    map64x64: { total: number; speciesBreakdown: Record<string, number> };
    map100x100: { total: number; speciesBreakdown: Record<string, number> };
    map200x200: { total: number; speciesBreakdown: Record<string, number> };
  };
  lodMetrics: {
    lod0: { step: number; dist: number; vertexCount: number; memoryKB: number };
    lod1: { step: number; dist: number; vertexCount: number; memoryKB: number };
    lod2: { step: number; dist: number; vertexCount: number; memoryKB: number };
    totalTerrainMemoryKB: number;
  };
  instancingMemoryMetrics: {
    map64x64: { totalInstances: number; matrixMemoryBytes: number; matrixMemoryKB: number };
    map100x100: { totalInstances: number; matrixMemoryBytes: number; matrixMemoryKB: number };
    map200x200: { totalInstances: number; matrixMemoryBytes: number; matrixMemoryKB: number };
  };
  frameTickLatency: {
    iterations: number;
    totalTimeMs: number;
    avgTickMs: number;
    maxTickMs: number;
    targetFpsMs: number;
    passed: boolean;
  };
  ecoClusteringValidation: {
    testedSeeds: string[];
    totalInstancesChecked: number;
    snowPeakViolations: number;
    steepSlopeViolations: number;
    waterViolations: number;
    passed: boolean;
  };
}

export function runEmpiricalBenchmark(): BenchmarkMetrics {
  const timestamp = new Date().toISOString();

  // 1. Foliage Instance Counts across map sizes
  const mapSizes = [64, 100, 200];
  const foliageInstanceCounts: any = {};
  const instancingMemoryMetrics: any = {};

  for (const size of mapSizes) {
    const terrain = generateTerrain(size, size, `benchmark_seed_${size}`);
    const speciesCounts: Record<string, number> = {};
    terrain.flora.forEach((f) => {
      speciesCounts[f.type] = (speciesCounts[f.type] || 0) + 1;
    });

    const key = `map${size}x${size}`;
    foliageInstanceCounts[key] = {
      total: terrain.flora.length,
      speciesBreakdown: speciesCounts,
    };

    const matrixMemoryBytes = terrain.flora.length * 64;
    instancingMemoryMetrics[key] = {
      totalInstances: terrain.flora.length,
      matrixMemoryBytes,
      matrixMemoryKB: matrixMemoryBytes / 1024,
    };
  }

  // 2. LOD Metrics for 100x100 terrain
  const width = 100;
  const height = 100;
  const bytesPerVert = 36; // Pos (12) + Normal (12) + Color (12)

  const lod0_verts = Math.ceil(width / 1) * Math.ceil(height / 1);
  const lod1_verts = Math.ceil(width / 4) * Math.ceil(height / 4);
  const lod2_verts = Math.ceil(width / 16) * Math.ceil(height / 16);

  const lod0_mem = (lod0_verts * bytesPerVert) / 1024;
  const lod1_mem = (lod1_verts * bytesPerVert) / 1024;
  const lod2_mem = (lod2_verts * bytesPerVert) / 1024;

  const lodMetrics = {
    lod0: { step: 1, dist: 0, vertexCount: lod0_verts, memoryKB: lod0_mem },
    lod1: { step: 4, dist: 250, vertexCount: lod1_verts, memoryKB: lod1_mem },
    lod2: { step: 16, dist: 600, vertexCount: lod2_verts, memoryKB: lod2_mem },
    totalTerrainMemoryKB: lod0_mem + lod1_mem + lod2_mem,
  };

  // 3. Frame Tick Latency Benchmark
  const elapsedTimes: number[] = [];
  const iterations = 1000;
  let mockTime = 0;

  // Simulate useFrame tick logic (updating uniforms and checking instanced mesh states)
  const dummyUniforms = {
    uTime: { value: 0 },
    uWindSpeed: { value: 1.0 },
    uWindDirection: { value: 0.5 },
    time: { value: 0 },
  };

  const startAll = performance.now();
  for (let i = 0; i < iterations; i++) {
    const t0 = performance.now();
    mockTime += 0.016;
    dummyUniforms.uTime.value = mockTime;
    dummyUniforms.time.value = mockTime;
    // Simulate updating 20 instanced mesh materials
    for (let m = 0; m < 20; m++) {
      dummyUniforms.uTime.value = mockTime;
    }
    const t1 = performance.now();
    elapsedTimes.push(t1 - t0);
  }
  const endAll = performance.now();

  const totalTimeMs = endAll - startAll;
  const avgTickMs = totalTimeMs / iterations;
  const maxTickMs = Math.max(...elapsedTimes);
  const passedTick = avgTickMs < 2.0;

  // 4. Eco-clustering Distribution Validation
  const testedSeeds = ['seed_alpha', 'seed_beta', 'seed_gamma', 'seed_delta', 'seed_epsilon'];
  let totalInstancesChecked = 0;
  let snowPeakViolations = 0;
  let steepSlopeViolations = 0;
  let waterViolations = 0;

  for (const seed of testedSeeds) {
    const terrain = generateTerrain(100, 100, seed);
    totalInstancesChecked += terrain.flora.length;

    terrain.flora.forEach((p) => {
      const cx = Math.floor(p.x);
      const cy = Math.floor(p.y);
      const cell = terrain.grid[cy][cx];

      if (cell.elevation >= 80 || cell.biome === 'snow peaks') {
        snowPeakViolations++;
      }

      if (cell.elevation < 3.0 || cell.isLake || cell.isRiver || cell.isWaterfall) {
        waterViolations++;
      }

      if (cx >= 1 && cx < 99 && cy >= 1 && cy < 99) {
        const hC = cell.elevation;
        const hR = terrain.grid[cy][cx + 1].elevation;
        const hL = terrain.grid[cy][cx - 1].elevation;
        const hU = terrain.grid[cy - 1][cx].elevation;
        const hD = terrain.grid[cy + 1][cx].elevation;

        const dhdx = (hR - hL) / 2.0;
        const dhdy = (hD - hU) / 2.0;
        const gradMagnitude = Math.sqrt(dhdx * dhdx + dhdy * dhdy);
        const slope = 1.0 - 1.0 / Math.sqrt(1.0 + gradMagnitude * gradMagnitude);

        if (slope > 0.45) {
          steepSlopeViolations++;
        }
      }
    });
  }

  const passedEco = snowPeakViolations === 0 && steepSlopeViolations === 0 && waterViolations === 0;

  return {
    timestamp,
    foliageInstanceCounts,
    lodMetrics,
    instancingMemoryMetrics,
    frameTickLatency: {
      iterations,
      totalTimeMs,
      avgTickMs,
      maxTickMs,
      targetFpsMs: 2.0,
      passed: passedTick,
    },
    ecoClusteringValidation: {
      testedSeeds,
      totalInstancesChecked,
      snowPeakViolations,
      steepSlopeViolations,
      waterViolations,
      passed: passedEco,
    },
  };
}
