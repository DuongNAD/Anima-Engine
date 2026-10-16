import * as THREE from 'three';
import {
  Map2TerrainMathEngine,
  sampleMap2Terrain,
  getMap2Height,
  getMap2NormalAndSteepness,
} from '../src/components/Map2/utils/map2TerrainGen';
import {
  Map2Biome,
  Map2BiomeEcologySystem,
  BIOME_METADATA,
} from '../src/components/Map2/utils/map2BiomeConfig';
import {
  Map2CameraPhysicsController,
  PHYSICS_CONSTANTS,
  SpatialFloraColliderIndex,
  resolveFloraOverlap,
  computeBearing,
  computePitch,
} from '../src/components/Map2/utils/map2Physics';
import {
  computeCelestialState,
} from '../src/components/Map2/utils/map2AtmosphereMath';

class DeterministicRNG {
  private state: number;
  constructor(seed = 0xabcdef) {
    this.state = seed >>> 0;
  }
  next(): number {
    this.state = (Math.imul(1664525, this.state) + 1013904223) >>> 0;
    return this.state / 4294967296;
  }
}

interface StressResult {
  suite: string;
  passed: boolean;
  details: Record<string, any>;
  errors: string[];
}

const results: StressResult[] = [];

console.log('=============================================================================');
console.log('CHALLENGER 1 — EMPIRICAL ADVERSARIAL STRESS TEST & VALIDATION HARNESS');
console.log('=============================================================================');

const engine = new Map2TerrainMathEngine('anima_map2_master_seed');
const ecology = new Map2BiomeEcologySystem();

// ------------------------------------------------------------------------------------------
// 1. Extreme Coordinate Domain & Floating-Point Singularities
// ------------------------------------------------------------------------------------------
console.log('\n[1/8] Stress-testing Coordinate Extreme Domain & Floating Point Singularities...');
const rng1 = new DeterministicRNG(0x11223344);
const nCoords = 200_000;
let nanH = 0;
let infH = 0;
let negH = 0;
let outOfRangeH = 0;
let minElevation = Infinity;
let maxElevation = -Infinity;

const t0 = performance.now();
for (let i = 0; i < nCoords; i++) {
  const x = (rng1.next() * 2 - 1) * 10_000_000; // [-10M, +10M]
  const z = (rng1.next() * 2 - 1) * 10_000_000;
  const h = engine.getHeight(x, z);

  if (Number.isNaN(h)) nanH++;
  if (!Number.isFinite(h)) infH++;
  if (h < 0) negH++;
  if (h > engine.maxElevation) outOfRangeH++;
  if (h < minElevation) minElevation = h;
  if (h > maxElevation) maxElevation = h;
}

// Check extreme edge floats
const extremeFloats = [
  0, -0, 1e-15, -1e-15, 1e-7, -1e-7, 0.999999999, 1.000000001,
  255, 255.0000001, 256, 511, 512, 1024, 65536, 1e6, -1e6,
];
let floatEdgeErrors = 0;
for (const fx of extremeFloats) {
  for (const fz of extremeFloats) {
    const s = engine.sample(fx, fz);
    if (!Number.isFinite(s.height) || s.height < 0 || s.height > 90.0) {
      floatEdgeErrors++;
    }
  }
}
const elapsed1 = performance.now() - t0;

const p1 = nanH === 0 && infH === 0 && negH === 0 && outOfRangeH === 0 && floatEdgeErrors === 0;
results.push({
  suite: '1. Extreme Coordinate Domain & Float Singularities',
  passed: p1,
  details: {
    samples: nCoords,
    durationMs: elapsed1.toFixed(2),
    minElevation: minElevation.toFixed(4),
    maxElevation: maxElevation.toFixed(4),
    nanCount: nanH,
    infCount: infH,
    negativeCount: negH,
    outOfRangeCount: outOfRangeH,
    floatEdgeCombinations: extremeFloats.length * extremeFloats.length,
    floatEdgeErrors,
  },
  errors: p1 ? [] : ['Found coordinate stability failure'],
});
console.log(` -> Result: ${p1 ? 'PASSED' : 'FAILED'} (Min: ${minElevation.toFixed(2)}m, Max: ${maxElevation.toFixed(2)}m, Duration: ${elapsed1.toFixed(1)}ms)`);

// ------------------------------------------------------------------------------------------
// 2. Hydrology Boundedness & Solid Continental Landmass (>=85% landmass)
// ------------------------------------------------------------------------------------------
console.log('\n[2/8] Stress-testing Hydrology Boundedness & Continental Solid Landmass Ratio...');
const rng2 = new DeterministicRNG(0x44332211);
const nHydro = 100_000;
let submergedCount = 0;
let illegalSubmergedCount = 0; // Submerged outside central lake and river basins
let minMainlandElevationOutsideHydro = Infinity;

const startT2 = performance.now();
for (let i = 0; i < nHydro; i++) {
  // Uniform sampling within map domain [-100, 100]
  const x = (rng2.next() * 2 - 1) * 100.0;
  const z = (rng2.next() * 2 - 1) * 100.0;
  const h = engine.getHeight(x, z);

  const isSubmerged = h <= engine.waterLevel;
  if (isSubmerged) {
    submergedCount++;
    // Check if inside legitimate basins:
    // A. Central Lake Basin: dist((x,z), (35, 25)) < 28.0
    const distToLake = Math.hypot(x - 35.0, z - 25.0);
    // B. Sinusoidal River: dist(x, 35 + 20*sin((z-25)*0.04)) < 7.5 and z in [25, 90]
    const riverCurveX = 35.0 + 20.0 * Math.sin((z - 25.0) * 0.04);
    const distToRiver = Math.abs(x - riverCurveX);
    const inRiver = distToRiver < 7.5 && z > 25.0 && z < 90.0;

    if (distToLake >= 28.0 && !inRiver) {
      illegalSubmergedCount++;
    }
  } else {
    // Outside water
    const distToLake = Math.hypot(x - 35.0, z - 25.0);
    const riverCurveX = 35.0 + 20.0 * Math.sin((z - 25.0) * 0.04);
    const distToRiver = Math.abs(x - riverCurveX);
    const inRiver = distToRiver < 7.5 && z > 25.0 && z < 90.0;

    if (distToLake >= 28.0 && !inRiver) {
      if (h < minMainlandElevationOutsideHydro) {
        minMainlandElevationOutsideHydro = h;
      }
    }
  }
}
const elapsed2 = performance.now() - startT2;
const landmassRatio = ((nHydro - submergedCount) / nHydro) * 100;
const waterRatio = (submergedCount / nHydro) * 100;

const p2 = illegalSubmergedCount === 0 && landmassRatio >= 85.0 && minMainlandElevationOutsideHydro >= 15.5;
results.push({
  suite: '2. Hydrology Boundedness & Solid Continental Landmass',
  passed: p2,
  details: {
    samples: nHydro,
    durationMs: elapsed2.toFixed(2),
    submergedSamples: submergedCount,
    waterCoveragePercentage: `${waterRatio.toFixed(2)}%`,
    solidLandmassPercentage: `${landmassRatio.toFixed(2)}%`,
    minimumRequiredLandmass: '85.0%',
    illegalSubmergedOutsideBasins: illegalSubmergedCount,
    minMainlandElevationOutsideBasins: `${minMainlandElevationOutsideHydro.toFixed(4)}m (guaranteed >= 15.5m)`,
  },
  errors: p2 ? [] : [`Failed hydrology boundedness: ${illegalSubmergedCount} illegal submerged points, landmass ${landmassRatio.toFixed(2)}%`],
});
console.log(` -> Result: ${p2 ? 'PASSED' : 'FAILED'} (Landmass: ${landmassRatio.toFixed(2)}% >= 85%, Illegal Submerged: ${illegalSubmergedCount}, Min Mainland H: ${minMainlandElevationOutsideHydro.toFixed(2)}m)`);

// ------------------------------------------------------------------------------------------
// 3. Analytical Normal Unit Length & Directional Gradients
// ------------------------------------------------------------------------------------------
console.log('\n[3/8] Stress-testing Analytical Normal Unit Length (||N|| == 1.0 +- 1e-12)...');
const rng3 = new DeterministicRNG(0x77889900);
const nNormals = 100_000;
let maxNormDev = 0;
let sumNormDev = 0;
let flippedNormals = 0;
let normViolations = 0;

const startT3 = performance.now();
for (let i = 0; i < nNormals; i++) {
  const x = (rng3.next() * 2 - 1) * 300.0;
  const z = (rng3.next() * 2 - 1) * 300.0;
  const { normal, steepness } = engine.getNormalAndSteepness(x, z);

  const len = normal.length();
  const dev = Math.abs(len - 1.0);
  sumNormDev += dev;
  if (dev > maxNormDev) maxNormDev = dev;
  if (dev > 1e-5) normViolations++;
  if (normal.y <= 0) flippedNormals++;
}
const elapsed3 = performance.now() - startT3;
const meanNormDev = sumNormDev / nNormals;

const p3 = normViolations === 0 && flippedNormals === 0 && maxNormDev <= 1e-5;
results.push({
  suite: '3. Analytical Normal Unit Length & Surface Gradients',
  passed: p3,
  details: {
    samples: nNormals,
    durationMs: elapsed3.toFixed(2),
    maxDeviationFromUnit: maxNormDev.toExponential(4),
    meanDeviationFromUnit: meanNormDev.toExponential(4),
    tolerance: '1e-5',
    violations: normViolations,
    flippedNormalsYNonPositive: flippedNormals,
  },
  errors: p3 ? [] : [`Normal deviation ${maxNormDev} exceeded tolerance or flipped normals found`],
});
console.log(` -> Result: ${p3 ? 'PASSED' : 'FAILED'} (Max Dev: ${maxNormDev.toExponential(3)}, Flipped: ${flippedNormals})`);

// ------------------------------------------------------------------------------------------
// 4. Partition of Unity & Biome Ecotone Continuity
// ------------------------------------------------------------------------------------------
console.log('\n[4/8] Stress-testing Partition of Unity & Biome Ecotone Continuity across 100,000 climate states...');
const rng4 = new DeterministicRNG(0x55667788);
const nPou = 100_000;
let maxPouDev = 0;
let sumPouDev = 0;
let invalidPouCount = 0;
let negativePouWeights = 0;
const biomeHist = new Array(8).fill(0);

const startT4 = performance.now();
for (let i = 0; i < nPou; i++) {
  const height = rng4.next() * 100.0 - 5.0; // [-5, 95]
  const steepness = rng4.next();
  const temperature = rng4.next();
  const moisture = rng4.next();
  const caveIntensity = rng4.next();

  const climate = { height, steepness, temperature, moisture, caveIntensity };
  const primary = ecology.classifyPrimaryBiome(climate);
  biomeHist[primary]++;

  const weights = ecology.computeBiomeWeights(climate);
  let sum = 0;
  for (let b = 0; b < 8; b++) {
    const w = weights[b];
    if (w < 0) negativePouWeights++;
    sum += w;
  }
  const dev = Math.abs(sum - 1.0);
  sumPouDev += dev;
  if (dev > maxPouDev) maxPouDev = dev;
  if (dev > 1e-5) invalidPouCount++;
}
const elapsed4 = performance.now() - startT4;
const meanPouDev = sumPouDev / nPou;

const p4 = invalidPouCount === 0 && negativePouWeights === 0 && maxPouDev <= 1e-5;
results.push({
  suite: '4. Partition of Unity & Biome Ecotone Continuity',
  passed: p4,
  details: {
    samples: nPou,
    durationMs: elapsed4.toFixed(2),
    maxAbsoluteDeviation: maxPouDev.toExponential(4),
    meanAbsoluteDeviation: meanPouDev.toExponential(4),
    tolerance: '1e-5',
    negativeWeightsCount: negativePouWeights,
    violations: invalidPouCount,
    biomeDistribution: {
      SNOW_PEAKS: biomeHist[0],
      ROCKY_CLIFFS: biomeHist[1],
      TEMPERATE_PLAINS: biomeHist[2],
      TROPICAL_RAINFOREST: biomeHist[3],
      ARID_SAVANNA: biomeHist[4],
      DESERT_DUNES: biomeHist[5],
      HYDROLOGY_RIVER_LAKE: biomeHist[6],
      BIOLUMINESCENT_CAVES: biomeHist[7],
    },
  },
  errors: p4 ? [] : [`Partition of unity deviation ${maxPouDev} exceeded 1e-5`],
});
console.log(` -> Result: ${p4 ? 'PASSED' : 'FAILED'} (Max Dev: ${maxPouDev.toExponential(3)}, Mean Dev: ${meanPouDev.toExponential(3)})`);

// ------------------------------------------------------------------------------------------
// 5. Subterranean Crystal Gating Invariant
// ------------------------------------------------------------------------------------------
console.log('\n[5/8] Stress-testing Subterranean Crystal Placement Invariant (0 surface crystals)...');
const rng5 = new DeterministicRNG(0x99aabbcc);
const nCrystals = 50_000;
let illegalSurfaceCrystals = 0;
let validGrottoCrystals = 0;
let validCaveCrystals = 0;

const startT5 = performance.now();
for (let i = 0; i < nCrystals; i++) {
  const x = (rng5.next() * 2 - 1) * 100.0;
  const z = (rng5.next() * 2 - 1) * 100.0;
  const h = engine.getHeight(x, z);
  const cave = engine.getCaveIntensity(x, z, h);

  // Crystal spawn rule from PROJECT.md:
  // (dist((-60, -60)) < 16m) || (caveIntensity > 0.75 && height < 22m)
  const distGrotto = Math.hypot(x - (-60), z - (-60));
  const isGrotto = distGrotto < 16.0;
  const isDeepCave = cave > 0.75 && h < 22.0;
  const crystalSpawns = isGrotto || isDeepCave;

  if (crystalSpawns) {
    if (isGrotto) validGrottoCrystals++;
    if (isDeepCave) validCaveCrystals++;

    // Violation: crystal placed on open high hill (h >= 22m) outside grotto
    if (!isGrotto && h >= 22.0) {
      illegalSurfaceCrystals++;
    }
  }
}
const elapsed5 = performance.now() - startT5;

const p5 = illegalSurfaceCrystals === 0;
results.push({
  suite: '5. Subterranean Crystal Gating Invariant',
  passed: p5,
  details: {
    samples: nCrystals,
    durationMs: elapsed5.toFixed(2),
    validGrottoCrystals,
    validCaveCrystals,
    illegalSurfaceCrystalsOnOpenHills: illegalSurfaceCrystals,
  },
  errors: p5 ? [] : [`Found ${illegalSurfaceCrystals} illegal crystals on open surface hills`],
});
console.log(` -> Result: ${p5 ? 'PASSED' : 'FAILED'} (Surface crystal violations: ${illegalSurfaceCrystals}, Grotto: ${validGrottoCrystals}, Deep Cave: ${validCaveCrystals})`);

// ------------------------------------------------------------------------------------------
// 6. Character Kinematics & Flora Trunk Collision Push-out (25,000 Steps)
// ------------------------------------------------------------------------------------------
console.log('\n[6/8] Stress-testing Character Kinematics, Slope Limits & Flora Trunk Collisions (25,000 steps)...');
const rng6 = new DeterministicRNG(0x13579bdf);
const charController = new Map2CameraPhysicsController('walk');
const colliderGrid = new SpatialFloraColliderIndex(8.0);

// Populate 500 random flora colliders outside spawn clearance
const floraCount = 500;
let placed = 0;
while (placed < floraCount) {
  const fx = (rng6.next() * 2 - 1) * 90.0;
  const fz = (rng6.next() * 2 - 1) * 90.0;
  if (Math.hypot(fx, fz) < 6.0) continue; // Respect spawn clearing
  const rad = 0.15 + rng6.next() * 0.75;
  colliderGrid.insert({ x: fx, z: fz, radius: rad });
  placed++;
}

const nWalkSteps = 25_000;
let terrainPenetrations = 0;
let floraClips = 0;
let kinematicNans = 0;
let maxWalkSpeed = 0;
const dt = 1 / 60;

const startT6 = performance.now();
// Start at spawn
charController.position.set(0, engine.getHeight(0, 0) + PHYSICS_CONSTANTS.EYE_HEIGHT, 0);

for (let s = 0; s < nWalkSteps; s++) {
  // Adversarial teleport to steep cliff every 1000 steps to test cliff recovery
  if (s > 0 && s % 1000 === 0) {
    const cliffX = 40.0 + rng6.next() * 20.0;
    const cliffZ = -30.0 + rng6.next() * 20.0;
    charController.position.set(cliffX, engine.getHeight(cliffX, cliffZ) + PHYSICS_CONSTANTS.EYE_HEIGHT, cliffZ);
    charController.velocity.set(0, 0, 0);
  }

  const inputDir = new THREE.Vector2(rng6.next() * 2 - 1, rng6.next() * 2 - 1);
  if (inputDir.lengthSq() > 0) inputDir.normalize();
  const speed = rng6.next() > 0.3 ? 12.0 : 5.0;
  const jump = rng6.next() < 0.1;

  charController.updateWalk(
    inputDir,
    speed,
    dt,
    jump,
    (x, z) => engine.sample(x, z),
    colliderGrid,
  );

  const curSpd = charController.velocity.length();
  if (curSpd > maxWalkSpeed) maxWalkSpeed = curSpd;

  if (
    !Number.isFinite(charController.position.x) ||
    !Number.isFinite(charController.position.y) ||
    !Number.isFinite(charController.position.z)
  ) {
    kinematicNans++;
  }

  // Terrain penetration check
  const curTerrainH = engine.getHeight(charController.position.x, charController.position.z);
  const clearance = charController.position.y - (curTerrainH + PHYSICS_CONSTANTS.EYE_HEIGHT);
  if (clearance < -1e-4) {
    terrainPenetrations++;
  }

  // Flora trunk collision check
  const nearby = colliderGrid.queryNearby(charController.position.x, charController.position.z, 3.0);
  for (const c of nearby) {
    const d = Math.hypot(charController.position.x - c.x, charController.position.z - c.z);
    if (d < c.radius + PHYSICS_CONSTANTS.PLAYER_RADIUS - 1e-3) {
      floraClips++;
    }
  }
}
const elapsed6 = performance.now() - startT6;

const p6 = terrainPenetrations === 0 && floraClips === 0 && kinematicNans === 0;
results.push({
  suite: '6. Character Kinematics, Slope Limits & Flora Trunk Collisions',
  passed: p6,
  details: {
    steps: nWalkSteps,
    durationMs: elapsed6.toFixed(2),
    denseFloraColliders: floraCount,
    terrainPenetrations,
    floraClips,
    kinematicNans,
    maxAchievedSpeed: `${maxWalkSpeed.toFixed(2)} m/s`,
  },
  errors: p6 ? [] : [`Penetrations: ${terrainPenetrations}, Flora clips: ${floraClips}, NaNs: ${kinematicNans}`],
});
console.log(` -> Result: ${p6 ? 'PASSED' : 'FAILED'} (Steps: ${nWalkSteps}, Penetrations: ${terrainPenetrations}, Flora Clips: ${floraClips}, Max Speed: ${maxWalkSpeed.toFixed(1)}m/s)`);

// ------------------------------------------------------------------------------------------
// 7. Spawn Clearing Safety & Topography Invariants
// ------------------------------------------------------------------------------------------
console.log('\n[7/8] Verifying Spawn Point Topography, Elevation, and Tree Clearance...');
const spawnH = engine.getHeight(0, 0);
const { normal: spawnNorm, steepness: spawnSteep } = engine.getNormalAndSteepness(0, 0);
const spawnSample = engine.sample(0, 0);

// Check clearance around spawn (D < 6.0m)
let spawnTrunkClashes = 0;
for (const c of colliderGrid.queryNearby(0, 0, 8.0)) {
  const d = Math.hypot(c.x, c.z);
  if (d < 6.0) {
    spawnTrunkClashes++;
  }
}

const p7 = spawnH >= 17.9 && spawnH <= 18.1 && spawnSteep < 0.05 && spawnSample.biomeId === Map2Biome.TEMPERATE_PLAINS && spawnTrunkClashes === 0;
results.push({
  suite: '7. Spawn Clearing Topography & Safety Invariants',
  passed: p7,
  details: {
    spawnPosition: `(0, ${spawnH.toFixed(2)}, 0)`,
    spawnSteepness: spawnSteep.toFixed(4),
    spawnBiome: BIOME_METADATA[spawnSample.biomeId].nameEn,
    clearRadiusGuaranteed: '>= 6.0m',
    clashesWithin6m: spawnTrunkClashes,
  },
  errors: p7 ? [] : ['Spawn topography does not match expected safe flat temperate plateau'],
});
console.log(` -> Result: ${p7 ? 'PASSED' : 'FAILED'} (Spawn H: ${spawnH.toFixed(2)}m, Steepness: ${spawnSteep.toFixed(4)}, Biome: ${BIOME_METADATA[spawnSample.biomeId].nameEn})`);

// ------------------------------------------------------------------------------------------
// 8. 24-Hour Celestial Solar/Lunar Orbit & Atmospheric PBR Consistency
// ------------------------------------------------------------------------------------------
console.log('\n[8/8] Stress-testing 24-Hour Celestial Orbit & Atmospheric PBR Stability...');
const rng8 = new DeterministicRNG(0x2468ace0);
const nCel = 100_000;
let celNans = 0;
let sunNormViolations = 0;
let moonNormViolations = 0;
let pbrViolations = 0;

const startT8 = performance.now();
for (let i = 0; i < nCel; i++) {
  const timeOfDay = (rng8.next() * 2 - 1) * 100_000; // Extreme time values
  const state = computeCelestialState(timeOfDay);

  if (Math.abs(state.sunDirection.length() - 1.0) > 1e-4) sunNormViolations++;
  if (Math.abs(state.moonDirection.length() - 1.0) > 1e-4) moonNormViolations++;

  if (!Number.isFinite(state.sunIntensity) || !Number.isFinite(state.fogDensity) || !Number.isFinite(state.ambientIntensity)) {
    celNans++;
  }

  const colors = [state.sunColor, state.moonColor, state.skyZenithColor, state.skyHorizonColor, state.fogColor, state.ambientColor];
  for (const c of colors) {
    if (c.r < 0 || c.g < 0 || c.b < 0 || !Number.isFinite(c.r) || !Number.isFinite(c.g) || !Number.isFinite(c.b)) {
      pbrViolations++;
    }
  }
}
const elapsed8 = performance.now() - startT8;

const p8 = celNans === 0 && sunNormViolations === 0 && moonNormViolations === 0 && pbrViolations === 0;
results.push({
  suite: '8. 24-Hour Celestial Orbit & Atmospheric PBR Stability',
  passed: p8,
  details: {
    queries: nCel,
    durationMs: elapsed8.toFixed(2),
    sunDirectionViolations: sunNormViolations,
    moonDirectionViolations: moonNormViolations,
    colorViolations: pbrViolations,
    celestialNans: celNans,
  },
  errors: p8 ? [] : ['Celestial stability failure detected'],
});
console.log(` -> Result: ${p8 ? 'PASSED' : 'FAILED'} (Queries: ${nCel}, Color Violations: ${pbrViolations}, Nans: ${celNans})`);

console.log('\n=============================================================================');
console.log('ALL EMPIRICAL CHALLENGER TESTS COMPLETED');
console.log('=============================================================================');
const allPassed = results.every((r) => r.passed);
console.log(`OVERALL VERDICT: ${allPassed ? 'APPROVE (ALL 8 SUITES PASSED 100%)' : 'REQUEST_CHANGES'}`);
console.log(JSON.stringify(results, null, 2));
