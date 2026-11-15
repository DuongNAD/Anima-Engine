import * as THREE from 'three';
import {
  Map2TerrainMathEngine,
} from '../src/components/Map2/utils/map2TerrainGen';
import {
  Map2BiomeEcologySystem,
  ClimateInput,
} from '../src/components/Map2/utils/map2BiomeConfig';
import {
  sampleMap2Water,
} from '../src/components/Map2/utils/map2WaterMath';
import {
  computeCelestialState,
} from '../src/components/Map2/utils/map2AtmosphereMath';
import {
  SpatialFloraColliderIndex,
  resolveFloraOverlap,
  resolveObstacleCollision,
  computeBearing,
  computePitch,
  calculateHeadBob,
} from '../src/components/Map2/utils/map2Physics';

// High-speed Deterministic PRNG (xoshiro128**)
class Xoshiro128StarStar {
  private s: Uint32Array;
  constructor(seed: number = 0x854321) {
    this.s = new Uint32Array(4);
    this.s[0] = seed >>> 0;
    this.s[1] = (seed ^ 0x6c078965) >>> 0;
    this.s[2] = (seed ^ 0x01234567) >>> 0;
    this.s[3] = (seed ^ 0x89abcdef) >>> 0;
    for (let i = 0; i < 16; i++) this.nextUint32();
  }

  nextUint32(): number {
    let s0 = this.s[0];
    let s1 = this.s[1];
    let s2 = this.s[2];
    let s3 = this.s[3];

    const result = Math.imul(this.rotl(Math.imul(s1, 5), 7), 9) >>> 0;
    const t = (s1 << 9) >>> 0;

    s2 ^= s0;
    s3 ^= s1;
    s1 ^= s2;
    s0 ^= s3;
    s2 ^= t;
    s3 = this.rotl(s3, 11);

    this.s[0] = s0;
    this.s[1] = s1;
    this.s[2] = s2;
    this.s[3] = s3;
    return result;
  }

  private rotl(x: number, k: number): number {
    return ((x << k) | (x >>> (32 - k))) >>> 0;
  }

  nextFloat(): number {
    return this.nextUint32() / 4294967296;
  }

  range(min: number, max: number): number {
    return min + this.nextFloat() * (max - min);
  }
}

interface FuzzResult {
  suite: string;
  totalQueries: number;
  passed: boolean;
  nanCount: number;
  infCount: number;
  invariantViolations: number;
  maxDeviation: number;
  durationMs: number;
  throughput: number;
  details: Record<string, unknown>;
  errors: string[];
}

const fuzzResults: FuzzResult[] = [];

console.log('========================================================================');
console.log('STARTING MAP 2 DEEP ADVERSARIAL MATHEMATICAL FUZZING HARNESS (100k+ Runs)');
console.log('========================================================================\n');

// -------------------------------------------------------------------------------------------------
// SUITE 1: TERRAIN ELEVATION & GRADIENT ADVERSARIAL FUZZING (100,000 Queries)
// -------------------------------------------------------------------------------------------------
console.log('>>> [Suite 1/6] Fuzzing Terrain Elevation & Analytical Normals (-10^6 to +10^6)...');
{
  const rng = new Xoshiro128StarStar(0x11223344);
  const engine = new Map2TerrainMathEngine('anima_map2_adversarial_seed');
  const count = 100_000;
  let nanCount = 0;
  let infCount = 0;
  let violations = 0;
  let maxDev = 0;
  let minH = Infinity;
  let maxH = -Infinity;
  let negativeNy = 0;
  let invalidSteepness = 0;
  const errors: string[] = [];

  const start = performance.now();
  for (let i = 0; i < count; i++) {
    const exponent = rng.range(0, 6);
    const signX = rng.nextFloat() > 0.5 ? 1 : -1;
    const signZ = rng.nextFloat() > 0.5 ? 1 : -1;
    const x = signX * Math.pow(10, exponent) * rng.nextFloat();
    const z = signZ * Math.pow(10, exponent) * rng.nextFloat();

    const sample = engine.sample(x, z);

    if (Number.isNaN(sample.height)) {
      nanCount++;
      violations++;
    } else if (!Number.isFinite(sample.height)) {
      infCount++;
      violations++;
    } else {
      if (sample.height < 0 || sample.height > engine.maxElevation + 1e-4) {
        violations++;
        errors.push(`Height out of bounds: ${sample.height} at (${x}, ${z})`);
      }
      if (sample.height < minH) minH = sample.height;
      if (sample.height > maxH) maxH = sample.height;
    }

    const nLen = sample.normal.length();
    const devN = Math.abs(nLen - 1.0);
    if (devN > maxDev) maxDev = devN;
    if (devN > 1e-4 || Number.isNaN(nLen) || !Number.isFinite(nLen)) {
      violations++;
      errors.push(`Invalid normal length: ${nLen} at (${x}, ${z})`);
    }

    if (sample.normal.y <= 0) {
      negativeNy++;
      violations++;
    }

    if (sample.steepness < 0 || sample.steepness > 1.0001 || !Number.isFinite(sample.steepness)) {
      invalidSteepness++;
      violations++;
    }
  }
  const duration = performance.now() - start;

  fuzzResults.push({
    suite: '1. Terrain Elevation & Gradient Invariants',
    totalQueries: count,
    passed: violations === 0,
    nanCount,
    infCount,
    invariantViolations: violations,
    maxDeviation: maxDev,
    durationMs: duration,
    throughput: Math.round((count / duration) * 1000),
    details: {
      elevationRange: `[${minH.toFixed(4)}, ${maxH.toFixed(4)}]`,
      maxElevationAllowed: engine.maxElevation,
      negativeNyCount: negativeNy,
      invalidSteepnessCount: invalidSteepness,
    },
    errors: errors.slice(0, 5),
  });
  console.log(`    Result: ${violations === 0 ? 'PASSED' : 'FAILED'} (Violations: ${violations}, NaNs: ${nanCount}, Max Normal Dev: ${maxDev.toExponential(3)}, Throughput: ${Math.round((count / duration) * 1000)} q/s)`);
}

// -------------------------------------------------------------------------------------------------
// SUITE 2: SOFTMAX BIOME PARTITION OF UNITY UNDER ADVERSARIAL CLIMATES (100,000 Queries)
// -------------------------------------------------------------------------------------------------
console.log('\n>>> [Suite 2/6] Fuzzing Biome Partition of Unity (\\sum w_i == 1.0) under Extreme Climates...');
{
  const rng = new Xoshiro128StarStar(0x55667788);
  const ecology = new Map2BiomeEcologySystem();
  const count = 100_000;
  let nanCount = 0;
  let infCount = 0;
  let violations = 0;
  let maxDev = 0;
  let negativeWeights = 0;
  const errors: string[] = [];

  const start = performance.now();
  for (let i = 0; i < count; i++) {
    const height = rng.range(-50.0, 150.0);
    const steepness = rng.range(-0.5, 2.0);
    const temperature = rng.range(-1.0, 2.0);
    const moisture = rng.range(-1.0, 2.0);
    const caveIntensity = rng.range(-0.5, 1.5);

    const climate: ClimateInput = { height, steepness, temperature, moisture, caveIntensity };
    const weights = ecology.computeBiomeWeights(climate);

    let sum = 0;
    for (let b = 0; b < 8; b++) {
      const w = weights[b];
      if (Number.isNaN(w)) nanCount++;
      if (!Number.isFinite(w)) infCount++;
      if (w < 0) negativeWeights++;
      sum += w;
    }

    const dev = Math.abs(sum - 1.0);
    if (dev > maxDev) maxDev = dev;
    if (dev > 1e-5 || Number.isNaN(sum) || !Number.isFinite(sum)) {
      violations++;
      if (errors.length < 5) {
        errors.push(`POU violation sum=${sum} (dev=${dev}) at climate: ${JSON.stringify(climate)}`);
      }
    }
  }
  const duration = performance.now() - start;

  fuzzResults.push({
    suite: '2. Softmax Biome Partition of Unity',
    totalQueries: count,
    passed: violations === 0 && negativeWeights === 0 && nanCount === 0,
    nanCount,
    infCount,
    invariantViolations: violations,
    maxDeviation: maxDev,
    durationMs: duration,
    throughput: Math.round((count / duration) * 1000),
    details: {
      negativeWeights,
      tolerance: '1e-5',
    },
    errors: errors.slice(0, 5),
  });
  console.log(`    Result: ${violations === 0 ? 'PASSED' : 'FAILED'} (Max POU Dev: ${maxDev.toExponential(3)}, Violations: ${violations}, NaNs: ${nanCount}, Throughput: ${Math.round((count / duration) * 1000)} q/s)`);
}

// -------------------------------------------------------------------------------------------------
// SUITE 3: GERSTNER WAVE DISPERSION BOUNDS & ORTHOGONALITY (100,000 Queries)
// -------------------------------------------------------------------------------------------------
console.log('\n>>> [Suite 3/6] Fuzzing Gerstner Waves & Differential Orthogonality...');
{
  const rng = new Xoshiro128StarStar(0x99aabbcc);
  const count = 100_000;
  let nanCount = 0;
  let infCount = 0;
  let violations = 0;
  let maxNormalDev = 0;
  let maxTangentDev = 0;
  let maxBinormalDev = 0;
  let minHeight = Infinity;
  let maxHeight = -Infinity;
  const errors: string[] = [];

  const start = performance.now();
  for (let i = 0; i < count; i++) {
    const x = rng.range(-1_000_000, 1_000_000);
    const z = rng.range(-1_000_000, 1_000_000);
    const time = rng.range(-1_000_000, 1_000_000);

    const water = sampleMap2Water(x, z, time, 12.0);

    if (Number.isNaN(water.height)) nanCount++;
    if (!Number.isFinite(water.height)) infCount++;
    if (water.height < minHeight) minHeight = water.height;
    if (water.height > maxHeight) maxHeight = water.height;

    if (water.height < 11.2 || water.height > 12.8) {
      violations++;
      if (errors.length < 5) errors.push(`Water height out of theoretical range: ${water.height} at (${x}, ${z}, t=${time})`);
    }

    const nLen = water.normal.length();
    const devN = Math.abs(nLen - 1.0);
    if (devN > maxNormalDev) maxNormalDev = devN;
    if (devN > 1e-4) violations++;

    const tLen = water.tangent.length();
    const bLen = water.binormal.length();
    if (Math.abs(tLen - 1.0) > maxTangentDev) maxTangentDev = Math.abs(tLen - 1.0);
    if (Math.abs(bLen - 1.0) > maxBinormalDev) maxBinormalDev = Math.abs(bLen - 1.0);

    const reconstructedNormal = new THREE.Vector3().crossVectors(water.binormal, water.tangent).normalize();
    const alignment = Math.abs(reconstructedNormal.dot(water.normal) - 1.0);
    if (alignment > 1e-4) {
      violations++;
      if (errors.length < 5) errors.push(`Normal-Tangent-Binormal frame misalignment: ${alignment}`);
    }
  }
  const duration = performance.now() - start;

  fuzzResults.push({
    suite: '3. Gerstner Waves & Differential Orthogonality',
    totalQueries: count,
    passed: violations === 0 && nanCount === 0 && infCount === 0,
    nanCount,
    infCount,
    invariantViolations: violations,
    maxDeviation: maxNormalDev,
    durationMs: duration,
    throughput: Math.round((count / duration) * 1000),
    details: {
      waterHeightBounds: `[${minHeight.toFixed(4)}, ${maxHeight.toFixed(4)}]`,
      maxNormalDev: maxNormalDev.toExponential(3),
      maxTangentDev: maxTangentDev.toExponential(3),
      maxBinormalDev: maxBinormalDev.toExponential(3),
    },
    errors: errors.slice(0, 5),
  });
  console.log(`    Result: ${violations === 0 ? 'PASSED' : 'FAILED'} (Violations: ${violations}, NaNs: ${nanCount}, Height Range: [${minHeight.toFixed(2)}, ${maxHeight.toFixed(2)}], Throughput: ${Math.round((count / duration) * 1000)} q/s)`);
}

// -------------------------------------------------------------------------------------------------
// SUITE 4: CELESTIAL EPHEMERIS CONTINUITY & COLOR TEMPERATURE BOUNDS (100,000 Queries)
// -------------------------------------------------------------------------------------------------
console.log('\n>>> [Suite 4/6] Fuzzing Celestial Ephemeris Continuity & Color Bounds (-10^6 to +10^6 Hours)...');
{
  const rng = new Xoshiro128StarStar(0xddeeff00);
  const count = 100_000;
  let nanCount = 0;
  const infCount = 0;
  let violations = 0;
  let sunDirDev = 0;
  let moonDirDev = 0;
  let antipodalMaxDev = 0;
  const errors: string[] = [];

  const start = performance.now();
  for (let i = 0; i < count; i++) {
    const timeOfDay = rng.range(-1_000_000, 1_000_000);
    const windSpeed = rng.range(0, 50);
    const windX = rng.range(-10, 10);
    const windZ = rng.range(-10, 10);

    const cel = computeCelestialState(timeOfDay, windSpeed, [windX, windZ]);

    if (cel.timeOfDay < 0 || cel.timeOfDay >= 24.0) {
      violations++;
      errors.push(`Invalid normalized timeOfDay: ${cel.timeOfDay}`);
    }

    const sLen = cel.sunDirection.length();
    const mLen = cel.moonDirection.length();
    if (Math.abs(sLen - 1.0) > sunDirDev) sunDirDev = Math.abs(sLen - 1.0);
    if (Math.abs(mLen - 1.0) > moonDirDev) moonDirDev = Math.abs(mLen - 1.0);
    if (Math.abs(sLen - 1.0) > 1e-4 || Math.abs(mLen - 1.0) > 1e-4) violations++;

    const antipodalSum = cel.sunDirection.clone().add(cel.moonDirection).length();
    if (antipodalSum > antipodalMaxDev) antipodalMaxDev = antipodalSum;
    if (antipodalSum > 1e-4) violations++;

    const sunR = cel.sunPosition.length();
    if (Math.abs(sunR - 400.0) > 1e-3) violations++;

    const colorList = [
      cel.sunColor,
      cel.moonColor,
      cel.skyZenithColor,
      cel.skyHorizonColor,
      cel.skyGroundColor,
      cel.fogColor,
      cel.ambientColor,
      cel.hemiSkyColor,
      cel.hemiGroundColor,
    ];
    for (const c of colorList) {
      if (c.r < 0 || c.g < 0 || c.b < 0 || !Number.isFinite(c.r) || !Number.isFinite(c.g) || !Number.isFinite(c.b)) {
        violations++;
        nanCount++;
      }
    }

    if (!Number.isFinite(cel.sunIntensity) || !Number.isFinite(cel.moonIntensity) || !Number.isFinite(cel.fogDensity)) {
      nanCount++;
      violations++;
    }
  }
  const duration = performance.now() - start;

  fuzzResults.push({
    suite: '4. Celestial Ephemeris Continuity & Color Bounds',
    totalQueries: count,
    passed: violations === 0 && nanCount === 0,
    nanCount,
    infCount,
    invariantViolations: violations,
    maxDeviation: Math.max(sunDirDev, moonDirDev, antipodalMaxDev),
    durationMs: duration,
    throughput: Math.round((count / duration) * 1000),
    details: {
      maxSunDirDev: sunDirDev.toExponential(3),
      maxMoonDirDev: moonDirDev.toExponential(3),
      maxAntipodalDev: antipodalMaxDev.toExponential(3),
    },
    errors: errors.slice(0, 5),
  });
  console.log(`    Result: ${violations === 0 ? 'PASSED' : 'FAILED'} (Violations: ${violations}, NaNs: ${nanCount}, Antipodal Dev: ${antipodalMaxDev.toExponential(3)}, Throughput: ${Math.round((count / duration) * 1000)} q/s)`);
}

// -------------------------------------------------------------------------------------------------
// SUITE 5: VECTOR KINEMATICS, SINGULAR SPHERICAL ANGLES & COLLISION OVERLAP (100,000 Queries)
// -------------------------------------------------------------------------------------------------
console.log('\n>>> [Suite 5/6] Fuzzing Vector Angles, Head-Bob Kinematics & Degenerate Collisions...');
{
  const rng = new Xoshiro128StarStar(0x12344321);
  const count = 100_000;
  let nanCount = 0;
  const infCount = 0;
  let violations = 0;
  const errors: string[] = [];

  const start = performance.now();

  for (let i = 0; i < count; i++) {
    let dir: THREE.Vector3;
    if (i === 0) dir = new THREE.Vector3(0, 0, 0);
    else if (i === 1) dir = new THREE.Vector3(0, 1, 0);
    else if (i === 2) dir = new THREE.Vector3(0, -1, 0);
    else if (i === 3) dir = new THREE.Vector3(1e-15, 0, 1e-15);
    else {
      dir = new THREE.Vector3(
        rng.range(-1000, 1000),
        rng.range(-1000, 1000),
        rng.range(-1000, 1000),
      );
    }

    const bearing = computeBearing(dir);
    const pitch = computePitch(dir);

    if (Number.isNaN(bearing) || !Number.isFinite(bearing) || bearing < 0 || bearing >= Math.PI * 2 + 1e-4) {
      violations++;
      nanCount++;
    }
    if (Number.isNaN(pitch) || !Number.isFinite(pitch) || pitch < -Math.PI / 2 - 1e-4 || pitch > Math.PI / 2 + 1e-4) {
      violations++;
      nanCount++;
    }

    const dist = rng.range(-1000, 1000);
    const bob = calculateHeadBob(dist);
    if (!Number.isFinite(bob.offsetY) || !Number.isFinite(bob.swayX)) {
      violations++;
      nanCount++;
    }

    const pos = new THREE.Vector3(10, 0, 10);
    resolveObstacleCollision(pos, new THREE.Vector2(10, 10), 1.0, 0.5);
    if (!Number.isFinite(pos.x) || !Number.isFinite(pos.z) || Math.hypot(pos.x - 10, pos.z - 10) < 1.5) {
      violations++;
    }
  }
  const duration = performance.now() - start;

  fuzzResults.push({
    suite: '5. Vector Kinematics & Singular Collisions',
    totalQueries: count,
    passed: violations === 0 && nanCount === 0,
    nanCount,
    infCount,
    invariantViolations: violations,
    maxDeviation: 0,
    durationMs: duration,
    throughput: Math.round((count / duration) * 1000),
    details: {
      singularitiesTested: ['(0,0,0)', '(0,1,0)', '(0,-1,0)', 'subnormals', 'exact_overlap'],
    },
    errors: errors.slice(0, 5),
  });
  console.log(`    Result: ${violations === 0 ? 'PASSED' : 'FAILED'} (Violations: ${violations}, NaNs: ${nanCount}, Throughput: ${Math.round((count / duration) * 1000)} q/s)`);
}

// -------------------------------------------------------------------------------------------------
// SUITE 6: SPATIAL COLLIDER INDEX & OVERLAP SOLVER (100,000 Queries)
// -------------------------------------------------------------------------------------------------
console.log('\n>>> [Suite 6/6] Fuzzing Spatial Flora Collider Grid & Overlap Solver...');
{
  const rng = new Xoshiro128StarStar(0x77889900);
  const count = 100_000;
  let nanCount = 0;
  const infCount = 0;
  let violations = 0;
  const colliderIndex = new SpatialFloraColliderIndex(8.0);

  // Populate well-spaced colliders (min distance 2.0m to prevent intersecting colliders)
  for (let i = 0; i < 500; i++) {
    colliderIndex.insert({
      x: (i % 25) * 4.0 - 50.0 + rng.range(-0.5, 0.5),
      z: Math.floor(i / 25) * 4.0 - 50.0 + rng.range(-0.5, 0.5),
      radius: rng.range(0.2, 0.8),
    });
  }

  const start = performance.now();
  for (let i = 0; i < count; i++) {
    const qx = rng.range(-55, 55);
    const qz = rng.range(-55, 55);
    const searchR = rng.range(0.5, 4.0);

    const nearby = colliderIndex.queryNearby(qx, qz, searchR);

    const playerPos = new THREE.Vector3(qx, 10, qz);
    resolveFloraOverlap(playerPos, nearby, 0.45);

    if (!Number.isFinite(playerPos.x) || !Number.isFinite(playerPos.z)) {
      nanCount++;
      violations++;
    }

    for (const col of nearby) {
      const dist = Math.hypot(playerPos.x - col.x, playerPos.z - col.z);
      const minSafe = col.radius + 0.45;
      if (dist < minSafe - 1e-3) {
        violations++;
      }
    }
  }
  const duration = performance.now() - start;

  fuzzResults.push({
    suite: '6. Spatial Collider Index & Overlap Solver',
    totalQueries: count,
    passed: violations === 0 && nanCount === 0,
    nanCount,
    infCount,
    invariantViolations: violations,
    maxDeviation: 0,
    durationMs: duration,
    throughput: Math.round((count / duration) * 1000),
    details: {
      collidersInGrid: 500,
      gridCellSize: 8.0,
    },
    errors: [],
  });
  console.log(`    Result: ${violations === 0 ? 'PASSED' : 'FAILED'} (Violations: ${violations}, NaNs: ${nanCount}, Throughput: ${Math.round((count / duration) * 1000)} q/s)`);
}

// -------------------------------------------------------------------------------------------------
// FINAL VERDICT & SUMMARY
// -------------------------------------------------------------------------------------------------
console.log('\n========================================================================');
console.log('FINAL SUMMARY OF MAP 2 DEEP ADVERSARIAL MATHEMATICAL FUZZING (600,000 Total Queries):');
console.log(JSON.stringify(fuzzResults, null, 2));
console.log('========================================================================\n');

const allPassed = fuzzResults.every(r => r.passed);
console.log(`OVERALL MATHEMATICAL & INVARIANT ADVERSARIAL VERDICT: ${allPassed ? 'ALL PASS (APPROVE)' : 'FAILURES DETECTED (REQUEST_CHANGES)'}`);
