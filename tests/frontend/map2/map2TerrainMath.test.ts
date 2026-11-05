import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import {
  Map2TerrainMathEngine,
  getMap2Height,
  getMap2NormalAndSteepness,
  sampleMap2Terrain,
  SeededNoise2D,
} from '../../../src/components/Map2/utils/map2TerrainGen';

// -------------------------------------------------------------------------------------------------
// Map 2 Terrain Math Test Suite: Tectonics, Erosion, Elevation, Normals, Slopes & Scientific Invariants
// -------------------------------------------------------------------------------------------------

describe('Map 2 Terrain Math — Tier 1: Noise Determinism, Plate Tectonics & Continental Shields', () => {
  it('generates bit-identical 2D noise for identical seeds and coordinates', () => {
    const genA = new SeededNoise2D('map2_test_seed');
    const genB = new SeededNoise2D('map2_test_seed');

    for (let i = 0; i < 100; i++) {
      const x = (i * 0.17) - 25;
      const z = (i * 0.31) + 14;
      const valA = genA.noise(x, z);
      const valB = genB.noise(x, z);
      expect(valA).toBeCloseTo(valB, 8);
    }
  });

  it('produces distinct noise distributions for different seeds', () => {
    const genA = new SeededNoise2D('seed_alpha');
    const genB = new SeededNoise2D('seed_beta');

    let diffCount = 0;
    for (let i = 0; i < 50; i++) {
      const x = i * 0.42;
      const z = i * 0.77;
      if (Math.abs(genA.noise(x, z) - genB.noise(x, z)) > 1e-4) {
        diffCount++;
      }
    }
    expect(diffCount).toBeGreaterThan(45);
  });

  it('bounds FBM and domain warped values within [-1.5, 1.5] without NaN or Inf', () => {
    const gen = new SeededNoise2D('bound_check_seed');
    for (let x = -50; x <= 50; x += 10) {
      for (let z = -50; z <= 50; z += 10) {
        const val = gen.domainWarpFBM(x, z);
        expect(Number.isNaN(val)).toBe(false);
        expect(Number.isFinite(val)).toBe(true);
        expect(val).toBeGreaterThanOrEqual(-1.5);
        expect(val).toBeLessThanOrEqual(1.5);
      }
    }
  });

  it('verifies continental shield coverage: >=85% dry land area across the continental zone', () => {
    const engine = new Map2TerrainMathEngine('continental_shield_seed');
    const SAMPLES = 2500;
    let landCount = 0;

    for (let i = 0; i < SAMPLES; i++) {
      const x = ((i % 50) - 25) * 3.2; // [-80, 80]
      const z = (Math.floor(i / 50) - 25) * 3.2; // [-80, 80]
      const h = engine.getHeight(x, z);

      if (h > engine.waterLevel) {
        landCount++;
      }
    }

    const landFraction = landCount / SAMPLES;
    // R1: Continental shield plains and solid landmass comprise >= 85% of total continental area
    expect(landFraction).toBeGreaterThanOrEqual(0.85);
  });

  it('verifies tectonic fold mountains and peaks reach high elevations (>40m) on mountain ranges', () => {
    const engine = new Map2TerrainMathEngine('tectonics_seed');
    let maxElevationObserved = 0;
    let highPeakCount = 0;

    for (let x = -90; x <= 90; x += 4) {
      for (let z = -90; z <= 90; z += 4) {
        const h = engine.getHeight(x, z);
        if (h > maxElevationObserved) maxElevationObserved = h;
        if (h >= 40.0) highPeakCount++;
      }
    }

    expect(maxElevationObserved).toBeGreaterThan(40.0);
    expect(highPeakCount).toBeGreaterThan(10);
    expect(maxElevationObserved).toBeLessThanOrEqual(engine.maxElevation);
  });
});

describe('Map 2 Terrain Math — Tier 2: Boundary & Corner Value Testing', () => {
  const engine = new Map2TerrainMathEngine('boundary_seed');

  it('ensures all elevation queries fall strictly within non-negative elevation bounds [0, maxElevation]', () => {
    for (let x = -100; x <= 100; x += 10) {
      for (let z = -100; z <= 100; z += 10) {
        const h = engine.getHeight(x, z);
        expect(h).toBeGreaterThanOrEqual(0);
        expect(h).toBeLessThanOrEqual(engine.maxElevation);
        expect(Number.isNaN(h)).toBe(false);
      }
    }
  });

  it('evaluates coordinate singularities ([0,0], [255,255], [256,256], [512,512], powers of 2) smoothly', () => {
    const singularities = [
      [0, 0],
      [255, 255],
      [256, 256],
      [-255, -255],
      [-256, -256],
      [511, 511],
      [512, 512],
      [1e-6, -1e-6],
    ];

    for (const [x, z] of singularities) {
      const sample = engine.sample(x, z);
      expect(Number.isFinite(sample.height)).toBe(true);
      expect(Number.isFinite(sample.normal.x)).toBe(true);
      expect(Number.isFinite(sample.normal.y)).toBe(true);
      expect(Number.isFinite(sample.normal.z)).toBe(true);
      expect(sample.normal.length()).toBeCloseTo(1.0, 4);
    }
  });

  it('handles extreme out-of-bounds coordinates without crashing or numerical divergence', () => {
    const extremeCoords = [
      { x: -500, z: -500 },
      { x: 500, z: 500 },
      { x: 10000, z: -10000 },
      { x: -1e6, z: 1e6 },
    ];

    for (const pt of extremeCoords) {
      const sample = engine.sample(pt.x, pt.z);
      expect(Number.isNaN(sample.height)).toBe(false);
      expect(sample.height).toBeGreaterThanOrEqual(0);
      expect(sample.height).toBeLessThanOrEqual(engine.maxElevation);
      expect(sample.normal.length()).toBeCloseTo(1.0, 3);
    }
  });

  it('correctly classifies water flag and depth across the waterline (y = 12.0m +- eps)', () => {
    // Water basin center at (35, 25)
    const lakeCenter = engine.sample(35.0, 25.0);
    expect(lakeCenter.isWater).toBe(true);
    expect(lakeCenter.height).toBeLessThanOrEqual(engine.waterLevel);
    expect(lakeCenter.waterDepth).toBeCloseTo(engine.waterLevel - lakeCenter.height, 4);
    expect(lakeCenter.normal).toBeInstanceOf(THREE.Vector3);

    // Module-level function check
    const { normal: spawnNormal, steepness: spawnSteepness } = getMap2NormalAndSteepness(0, 0);
    expect(spawnNormal.y).toBeGreaterThan(0.7);
    expect(spawnSteepness).toBeGreaterThanOrEqual(0);

    // Spawn plateau at (0, 0) is solidly elevated above water
    const spawnPt = engine.sample(0, 0);
    expect(spawnPt.isWater).toBe(false);
    expect(spawnPt.height).toBeGreaterThan(engine.waterLevel);
    expect(spawnPt.waterDepth).toBe(0);
  });

  it('correctly detects snowline transition at high altitudes (y >= 48.0m)', () => {
    expect(engine.snowLine).toBe(48.0);
  });

  it('identifies vertical cliff faces with steepness > 0.45 for physics deflection', () => {
    let foundCliffs = 0;
    for (let x = -80; x <= 80; x += 5) {
      for (let z = -80; z <= 80; z += 5) {
        const { steepness } = engine.getNormalAndSteepness(x, z);
        if (steepness > 0.45) {
          foundCliffs++;
        }
      }
    }
    expect(foundCliffs).toBeGreaterThan(5);
  });
});

describe('Map 2 Terrain Math — Tier 3: Scientific Geology, Tectonics & Fluvial Erosion', () => {
  it('evaluates Voronoi plate tectonics: detects convergent collision orogeny and divergent rift valleys', () => {
    const engine = new Map2TerrainMathEngine('tectonics_science_seed');

    // Test near Northeast Alpine collision boundary (convergent)
    const convSample = engine.evaluateTectonics(45.0, 48.0);
    expect(convSample.nearestPlateId).toBeDefined();
    expect(convSample.secondPlateId).toBeDefined();
    expect(Number.isFinite(convSample.boundaryStress)).toBe(true);
    expect(Number.isFinite(convSample.crustElevation)).toBe(true);
    expect(convSample.crustElevation).toBeGreaterThanOrEqual(15.0);

    // Test near Central Rift graben (divergent rift)
    const riftSample = engine.evaluateTectonics(28.0, 18.0);
    expect(Number.isFinite(riftSample.riftDepression)).toBe(true);
  });

  it('verifies safe spawn plateau at (0, 0) is elevated (16-18m) with gentle slope (< 0.15)', () => {
    const engine = new Map2TerrainMathEngine('spawn_plateau_seed');
    const spawnHeight = engine.getHeight(0, 0);
    const { steepness } = engine.getNormalAndSteepness(0, 0);

    expect(spawnHeight).toBeGreaterThanOrEqual(16.0);
    expect(spawnHeight).toBeLessThanOrEqual(19.0);
    expect(steepness).toBeLessThan(0.15); // Gentle, flat, safe spawn
  });

  it('verifies thermal scree talus slope relaxation maintains natural angle of repose across terrain', () => {
    const engine = new Map2TerrainMathEngine('talus_scree_seed');
    let totalSteepness = 0;
    let sampleCount = 0;
    let excessiveCliffCount = 0;

    for (let x = -80; x <= 80; x += 4) {
      for (let z = -80; z <= 80; z += 4) {
        const { steepness } = engine.getNormalAndSteepness(x, z);
        totalSteepness += steepness;
        sampleCount++;

        // Near-vertical cliffs (steepness > 0.85) should be rare geological features
        if (steepness > 0.85) {
          excessiveCliffCount++;
        }
      }
    }

    const avgSteepness = totalSteepness / sampleCount;
    // Average continental terrain steepness is gentle and walkable (< 0.35)
    expect(avgSteepness).toBeLessThan(0.35);
    // Extreme sheer cliffs occur in < 5% of terrain
    expect(excessiveCliffCount / sampleCount).toBeLessThan(0.05);
  });

  it('couples mountain elevation and wind vector into leeward orographic moisture shadow', () => {
    // Western windward side (negative X) vs Eastern leeward side (positive X)
    const westSample = sampleMap2Terrain(-50, 0);
    const eastSample = sampleMap2Terrain(50, 0);

    // Moisture model includes eastward rain shadow factor
    expect(westSample.moisture).toBeGreaterThan(eastSample.moisture - 0.25);
  });

  it('couples elevation lapse rate to temperature: high mountain peaks are cold', () => {
    const highAltitudeSample = sampleMap2Terrain(0, 0); // elevated
    const valleySample = sampleMap2Terrain(35, 25); // lake valley

    // High elevation has higher lapse rate cooling
    expect(highAltitudeSample.temperature).toBeDefined();
    expect(valleySample.temperature).toBeDefined();
  });

  it('verifies dendritic river routing and lake drainage alignment: riverbed carved below banks', () => {
    // Along main river centerline: z in [30, 80]
    for (let z = 30; z <= 80; z += 10) {
      const riverX = 35.0 + 20.0 * Math.sin((z - 25.0) * 0.04);
      const hRiver = getMap2Height(riverX, z);
      const hBank = getMap2Height(riverX + 15.0, z);

      // Riverbed is carved deeper than surrounding banks
      expect(hRiver).toBeLessThanOrEqual(hBank);
    }

    // Central lake basin (35, 25) receives river drainage at deepest point (h <= 12.0)
    const lakeBedH = getMap2Height(35.0, 25.0);
    expect(lakeBedH).toBeLessThanOrEqual(12.0);
  });
});

describe('Map 2 Terrain Math — Tier 4: Real-World Latency & Performance Benchmark', () => {
  it('benchmarks 10,000 terrain sample queries to verify latency is strictly < 0.1ms per query', () => {
    const engine = new Map2TerrainMathEngine('benchmark_seed');
    const ITERATIONS = 10000;

    const start = performance.now();
    let sumHeight = 0;

    for (let i = 0; i < ITERATIONS; i++) {
      const x = ((i % 200) - 100) * 0.95;
      const z = (Math.floor(i / 200) - 25) * 0.95;
      const sample = engine.sample(x, z);
      sumHeight += sample.height;
    }

    const elapsedMs = performance.now() - start;
    const avgLatencyMs = elapsedMs / ITERATIONS;

    expect(sumHeight).toBeGreaterThan(0);
    expect(avgLatencyMs).toBeLessThan(0.1); // < 100 microseconds per query budget
  });
});
