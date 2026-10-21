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

interface TestSummary {
  name: string;
  passed: boolean;
  metrics: Record<string, any>;
  errors: string[];
}

const summaries: TestSummary[] = [];

// Seeded LCG
class DeterministicRNG {
  private state: number;
  constructor(seed = 0x12345678) {
    this.state = seed >>> 0;
  }
  next(): number {
    this.state = (Math.imul(1664525, this.state) + 1013904223) >>> 0;
    return this.state / 4294967296;
  }
}

// -------------------------------------------------------------------------------------------------
// 1. COORDINATE EXTREME BOUNDS (-1,000,000 to +1,000,000)
// -------------------------------------------------------------------------------------------------
console.log('=== [1/5] RUNNING ADVERSARIAL TEST 1: COORDINATE EXTREME BOUNDS ===');
const rng1 = new DeterministicRNG(0xdeadbeef);
const engine = new Map2TerrainMathEngine('anima_map2_master_seed');
const boundsSamples = 100_000;
let minH = Infinity;
let maxH = -Infinity;
let nanCount = 0;
let infCount = 0;
let negCount = 0;
const startT1 = performance.now();

for (let i = 0; i < boundsSamples; i++) {
  const x = (rng1.next() * 2 - 1) * 1_000_000;
  const z = (rng1.next() * 2 - 1) * 1_000_000;
  const h = engine.getHeight(x, z);

  if (Number.isNaN(h)) nanCount++;
  if (!Number.isFinite(h)) infCount++;
  if (h < 0) negCount++;
  if (h < minH) minH = h;
  if (h > maxH) maxH = h;
}
const elapsedT1 = performance.now() - startT1;

// Singular point checks
const singularPoints = [
  [0, 0],
  [1_000_000, 1_000_000],
  [-1_000_000, -1_000_000],
  [1_000_000, -1_000_000],
  [-1_000_000, 1_000_000],
  [255, 255],
  [256, 256],
  [-255, -255],
  [-256, -256],
  [511, 511],
  [512, 512],
  [-511, -511],
  [-512, -512],
  [1e-7, -1e-7],
];
let singularErrors = 0;
for (const [x, z] of singularPoints) {
  const s = engine.sample(x, z);
  if (!Number.isFinite(s.height) || s.height < 0 || s.height > engine.maxElevation) {
    singularErrors++;
  }
}

const t1Passed = nanCount === 0 && infCount === 0 && negCount === 0 && singularErrors === 0;
summaries.push({
  name: 'Test 1: Coordinate Extreme Bounds (-1M to +1M)',
  passed: t1Passed,
  metrics: {
    samples: boundsSamples,
    durationMs: elapsedT1.toFixed(2),
    throughputQueriesPerSec: ((boundsSamples / elapsedT1) * 1000).toFixed(0),
    minElevation: minH.toFixed(4),
    maxElevation: maxH.toFixed(4),
    maxElevationAllowed: engine.maxElevation,
    nanCount,
    infCount,
    negativeCount: negCount,
    singularTestCases: singularPoints.length,
    singularErrors,
  },
  errors: t1Passed ? [] : [`Found ${nanCount} NaNs, ${infCount} Infs, ${negCount} negatives`],
});
console.log(`Test 1 Result: ${t1Passed ? 'PASSED' : 'FAILED'} (Min H: ${minH.toFixed(2)}, Max H: ${maxH.toFixed(2)}, Elapsed: ${elapsedT1.toFixed(1)}ms)`);

// -------------------------------------------------------------------------------------------------
// 2. PARTITION OF UNITY ACROSS 50,000 RANDOM COORDINATES
// -------------------------------------------------------------------------------------------------
console.log('\n=== [2/5] RUNNING ADVERSARIAL TEST 2: PARTITION OF UNITY ===');
const rng2 = new DeterministicRNG(0x12345678);
const ecology = new Map2BiomeEcologySystem();
const pouSamples = 50_000;
let maxDeviation = 0;
let sumDeviation = 0;
let invalidPouCount = 0;
let negativeWeightCount = 0;
const biomeCounts: number[] = new Array(8).fill(0);
const startT2 = performance.now();

for (let i = 0; i < pouSamples; i++) {
  const height = rng2.next() * 95.0 - 5.0; // [-5, 90]
  const steepness = rng2.next();
  const temperature = rng2.next();
  const moisture = rng2.next();
  const caveIntensity = rng2.next();

  const climate = { height, steepness, temperature, moisture, caveIntensity };
  const primary = ecology.classifyPrimaryBiome(climate);
  biomeCounts[primary]++;

  const weights = ecology.computeBiomeWeights(climate);
  let sum = 0;
  for (let b = 0; b < 8; b++) {
    const w = weights[b];
    if (w < 0) negativeWeightCount++;
    sum += w;
  }
  const dev = Math.abs(sum - 1.0);
  sumDeviation += dev;
  if (dev > maxDeviation) maxDeviation = dev;
  if (dev > 1e-5) invalidPouCount++;
}
const elapsedT2 = performance.now() - startT2;
const meanDeviation = sumDeviation / pouSamples;

const t2Passed = invalidPouCount === 0 && negativeWeightCount === 0 && maxDeviation <= 1e-5;
summaries.push({
  name: 'Test 2: Partition of Unity (Biome Weights Sum == 1.0)',
  passed: t2Passed,
  metrics: {
    samples: pouSamples,
    durationMs: elapsedT2.toFixed(2),
    maxAbsoluteDeviation: maxDeviation.toExponential(4),
    meanAbsoluteDeviation: meanDeviation.toExponential(4),
    tolerance: '1e-5',
    negativeWeights: negativeWeightCount,
    violations: invalidPouCount,
    biomeDistribution: {
      SNOW_PEAKS: biomeCounts[0],
      ROCKY_CLIFFS: biomeCounts[1],
      TEMPERATE_PLAINS: biomeCounts[2],
      TROPICAL_RAINFOREST: biomeCounts[3],
      ARID_SAVANNA: biomeCounts[4],
      DESERT_DUNES: biomeCounts[5],
      HYDROLOGY_RIVER_LAKE: biomeCounts[6],
      BIOLUMINESCENT_CAVES: biomeCounts[7],
    },
  },
  errors: t2Passed ? [] : [`Max deviation ${maxDeviation} exceeded 1e-5 tolerance`],
});
console.log(`Test 2 Result: ${t2Passed ? 'PASSED' : 'FAILED'} (Max Dev: ${maxDeviation.toExponential(3)}, Mean Dev: ${meanDeviation.toExponential(3)})`);

// -------------------------------------------------------------------------------------------------
// 3. ANALYTICAL NORMAL UNIT LENGTH (||N|| == 1.0 +- 1e-4)
// -------------------------------------------------------------------------------------------------
console.log('\n=== [3/5] RUNNING ADVERSARIAL TEST 3: ANALYTICAL NORMAL UNIT LENGTH ===');
const rng3 = new DeterministicRNG(0xfeedbeef);
const normalSamples = 50_000;
let minNormalLen = Infinity;
let maxNormalLen = -Infinity;
let maxNormalDev = 0;
let sumNormalDev = 0;
let invalidNormalCount = 0;
let negativeNyCount = 0;
let minSteepness = Infinity;
let maxSteepness = -Infinity;
const startT3 = performance.now();

for (let i = 0; i < normalSamples; i++) {
  const x = (rng3.next() * 2 - 1) * 500.0;
  const z = (rng3.next() * 2 - 1) * 500.0;
  const { normal, steepness } = getMap2NormalAndSteepness(x, z);

  const len = normal.length();
  if (len < minNormalLen) minNormalLen = len;
  if (len > maxNormalLen) maxNormalLen = len;

  const dev = Math.abs(len - 1.0);
  sumNormalDev += dev;
  if (dev > maxNormalDev) maxNormalDev = dev;
  if (dev > 1e-4) invalidNormalCount++;
  if (normal.y <= 0) negativeNyCount++;

  if (steepness < minSteepness) minSteepness = steepness;
  if (steepness > maxSteepness) maxSteepness = steepness;
}
const elapsedT3 = performance.now() - startT3;
const meanNormalDev = sumNormalDev / normalSamples;

const t3Passed = invalidNormalCount === 0 && negativeNyCount === 0 && maxNormalDev <= 1e-4;
summaries.push({
  name: 'Test 3: Analytical Normal Unit Length (||N|| == 1.0 +- 1e-4)',
  passed: t3Passed,
  metrics: {
    samples: normalSamples,
    durationMs: elapsedT3.toFixed(2),
    minNormalLength: minNormalLen.toFixed(8),
    maxNormalLength: maxNormalLen.toFixed(8),
    maxAbsoluteDeviation: maxNormalDev.toExponential(4),
    meanAbsoluteDeviation: meanNormalDev.toExponential(4),
    tolerance: '1e-4',
    negativeNyCount,
    violations: invalidNormalCount,
    steepnessRange: `[${minSteepness.toFixed(4)}, ${maxSteepness.toFixed(4)}]`,
  },
  errors: t3Passed ? [] : [`Max normal deviation ${maxNormalDev} exceeded 1e-4 tolerance`],
});
console.log(`Test 3 Result: ${t3Passed ? 'PASSED' : 'FAILED'} (Max Dev: ${maxNormalDev.toExponential(3)}, Len range: [${minNormalLen.toFixed(6)}, ${maxNormalLen.toFixed(6)}])`);

// -------------------------------------------------------------------------------------------------
// 4. CHARACTER PHYSICS KINEMATICS (10,000 STEPS ON STEEP CLIFFS + TRUNK COLLIDERS)
// -------------------------------------------------------------------------------------------------
console.log('\n=== [4/5] RUNNING ADVERSARIAL TEST 4: CHARACTER PHYSICS KINEMATICS ===');
const rng4 = new DeterministicRNG(0x99887766);
const controller = new Map2CameraPhysicsController('walk');
const colliderIndex = new SpatialFloraColliderIndex(8.0);

// Populate 300 dense tree colliders
const colliderCount = 300;
for (let i = 0; i < colliderCount; i++) {
  const cx = (rng4.next() * 2 - 1) * 80.0;
  const cz = (rng4.next() * 2 - 1) * 80.0;
  const radius = 0.2 + rng4.next() * 0.8;
  colliderIndex.insert({ x: cx, z: cz, radius });
}

// Find steep cliff locations (>45 deg)
const steepSpots: Array<[number, number]> = [];
for (let x = -80; x <= 80; x += 4) {
  for (let z = -80; z <= 80; z += 4) {
    const s = sampleMap2Terrain(x, z);
    if (s.steepness > 0.45) {
      steepSpots.push([x, z]);
    }
  }
}

const walkSteps = 10_000;
let terrainPenetrations = 0;
const treeCollisionsDetected = 0;
let treeCollisionsClipped = 0;
let maxSlideSpeed = 0;
let minClearance = Infinity;
let physicsNans = 0;
const dt = 1 / 60;
const startT4 = performance.now();

// Start at first steep cliff
controller.position.set(
  steepSpots[0][0],
  getMap2Height(steepSpots[0][0], steepSpots[0][1]) + PHYSICS_CONSTANTS.EYE_HEIGHT,
  steepSpots[0][1],
);

for (let step = 0; step < walkSteps; step++) {
  if (step % 500 === 0) {
    const spot = steepSpots[(step / 500) % steepSpots.length];
    controller.position.set(
      spot[0],
      getMap2Height(spot[0], spot[1]) + PHYSICS_CONSTANTS.EYE_HEIGHT,
      spot[1],
    );
    controller.velocity.set(0, 0, 0);
  }

  // Adversarial movement
  const inputDir = new THREE.Vector2(rng4.next() * 2 - 1, rng4.next() * 2 - 1);
  if (inputDir.lengthSq() > 0) inputDir.normalize();
  const speed = rng4.next() > 0.4 ? 14.0 : 6.0;
  const jump = rng4.next() < 0.12;

  controller.updateWalk(
    inputDir,
    speed,
    dt,
    jump,
    (x, z) => sampleMap2Terrain(x, z),
    colliderIndex,
  );

  const curSpeed = controller.velocity.length();
  if (curSpeed > maxSlideSpeed) maxSlideSpeed = curSpeed;

  if (
    !Number.isFinite(controller.position.x) ||
    !Number.isFinite(controller.position.y) ||
    !Number.isFinite(controller.position.z)
  ) {
    physicsNans++;
  }

  // Check terrain clearance
  const gh = getMap2Height(controller.position.x, controller.position.z);
  const clearance = controller.position.y - (gh + PHYSICS_CONSTANTS.EYE_HEIGHT);
  if (clearance < minClearance) minClearance = clearance;
  if (clearance < -1e-4) {
    terrainPenetrations++;
  }

  // Check flora collisions
  const nearby = colliderIndex.queryNearby(controller.position.x, controller.position.z, 3.0);
  for (const col of nearby) {
    const dx = controller.position.x - col.x;
    const dz = controller.position.z - col.z;
    const dist = Math.hypot(dx, dz);
    const minSafeDist = col.radius + PHYSICS_CONSTANTS.PLAYER_RADIUS;
    if (dist < minSafeDist - 1e-3) {
      treeCollisionsClipped++;
    }
  }
}
const elapsedT4 = performance.now() - startT4;

const t4Passed = terrainPenetrations === 0 && treeCollisionsClipped === 0 && physicsNans === 0;
summaries.push({
  name: 'Test 4: Character Physics Kinematics (Steep Cliffs + Flora Colliders)',
  passed: t4Passed,
  metrics: {
    steps: walkSteps,
    durationMs: elapsedT4.toFixed(2),
    steepCliffSpotsIdentified: steepSpots.length,
    denseFloraColliders: colliderCount,
    minTerrainClearance: minClearance.toFixed(6),
    terrainPenetrations,
    treeCollisionsClipped,
    maxSlideSpeedAchieved: maxSlideSpeed.toFixed(2),
    physicsNans,
  },
  errors: t4Passed ? [] : [`Penetrations: ${terrainPenetrations}, Tree clips: ${treeCollisionsClipped}`],
});
console.log(`Test 4 Result: ${t4Passed ? 'PASSED' : 'FAILED'} (Min Clearance: ${minClearance.toFixed(6)}, Penetrations: ${terrainPenetrations}, Clips: ${treeCollisionsClipped})`);

// -------------------------------------------------------------------------------------------------
// 5. RAPID DAY-NIGHT TIME SCRUBBING & CAMERA MODE SWITCHING
// -------------------------------------------------------------------------------------------------
console.log('\n=== [5/5] RUNNING ADVERSARIAL TEST 5: DAY-NIGHT SCRUBBING & CAMERA SWITCHING ===');
const rng5 = new DeterministicRNG(0xabcdef12);
const scrubbingSamples = 100_000;
let celestialNans = 0;
let sunVectorInvalid = 0;
let moonVectorInvalid = 0;
let colorOutOfBounds = 0;
let minSunPosRadius = Infinity;
let maxSunPosRadius = -Infinity;
const startT5 = performance.now();

for (let i = 0; i < scrubbingSamples; i++) {
  const timeOfDay = (rng5.next() * 2 - 1) * 1_000_000;
  const cel = computeCelestialState(timeOfDay);

  const sunLen = cel.sunDirection.length();
  const moonLen = cel.moonDirection.length();
  if (Math.abs(sunLen - 1.0) > 1e-4) sunVectorInvalid++;
  if (Math.abs(moonLen - 1.0) > 1e-4) moonVectorInvalid++;

  const sunPosR = cel.sunPosition.length();
  if (sunPosR < minSunPosRadius) minSunPosRadius = sunPosR;
  if (sunPosR > maxSunPosRadius) maxSunPosRadius = sunPosR;

  const colors = [
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
  for (const c of colors) {
    if (c.r < 0 || c.g < 0 || c.b < 0 || !Number.isFinite(c.r) || !Number.isFinite(c.g) || !Number.isFinite(c.b)) {
      colorOutOfBounds++;
    }
  }

  if (!Number.isFinite(cel.sunIntensity) || !Number.isFinite(cel.fogDensity) || !Number.isFinite(cel.ambientIntensity)) {
    celestialNans++;
  }
}
const elapsedT5Scrubbing = performance.now() - startT5;

// Camera mode switching
const switchRng = new DeterministicRNG(0x55443322);
const switchController = new Map2CameraPhysicsController('orbit');
const modes: Array<'orbit' | 'fly' | 'walk' | 'cine'> = ['orbit', 'fly', 'walk', 'cine'];
const modeSwitches = 10_000;
let cameraNans = 0;
let angleOutOfBounds = 0;
const startT5Switch = performance.now();

for (let i = 0; i < modeSwitches; i++) {
  const m = modes[Math.floor(switchRng.next() * modes.length)];
  switchController.setMode(m);

  if (switchRng.next() < 0.25) {
    const tx = (switchRng.next() * 2 - 1) * 100.0;
    const tz = (switchRng.next() * 2 - 1) * 100.0;
    switchController.position.set(tx, getMap2Height(tx, tz) + 15, tz);
  }

  if (m === 'walk') {
    switchController.updateWalk(
      new THREE.Vector2(switchRng.next() * 2 - 1, switchRng.next() * 2 - 1),
      10.0,
      1 / 60,
      switchRng.next() < 0.1,
      (x, z) => sampleMap2Terrain(x, z),
    );
  }

  const tel = switchController.getTelemetry(Map2Biome.TEMPERATE_PLAINS);
  if (!Number.isFinite(tel.position.x) || !Number.isFinite(tel.bearing) || !Number.isFinite(tel.pitch)) {
    cameraNans++;
  }
  if (tel.bearing < 0 || tel.bearing >= Math.PI * 2 + 1e-4 || tel.pitch < -Math.PI / 2 - 1e-4 || tel.pitch > Math.PI / 2 + 1e-4) {
    angleOutOfBounds++;
  }
}
const elapsedT5Switch = performance.now() - startT5Switch;

const t5Passed =
  celestialNans === 0 &&
  sunVectorInvalid === 0 &&
  moonVectorInvalid === 0 &&
  colorOutOfBounds === 0 &&
  cameraNans === 0 &&
  angleOutOfBounds === 0;

summaries.push({
  name: 'Test 5: Rapid Day-Night Scrubbing & Camera Mode Switching',
  passed: t5Passed,
  metrics: {
    timeScrubbingQueries: scrubbingSamples,
    timeScrubbingDurationMs: elapsedT5Scrubbing.toFixed(2),
    sunDirectionViolations: sunVectorInvalid,
    moonDirectionViolations: moonVectorInvalid,
    sunOrbitRadiusRange: `[${minSunPosRadius.toFixed(2)}, ${maxSunPosRadius.toFixed(2)}]`,
    colorComponentViolations: colorOutOfBounds,
    celestialNans,
    modeSwitchCycles: modeSwitches,
    modeSwitchDurationMs: elapsedT5Switch.toFixed(2),
    cameraNans,
    cameraAngleViolations: angleOutOfBounds,
  },
  errors: t5Passed ? [] : ['Celestial or camera state instability detected'],
});
console.log(`Test 5 Result: ${t5Passed ? 'PASSED' : 'FAILED'} (Scrubbing: ${elapsedT5Scrubbing.toFixed(1)}ms, Switches: ${elapsedT5Switch.toFixed(1)}ms)`);

console.log('\n=================================================================');
console.log('FINAL SUMMARY OF ADVERSARIAL STRESS TESTING:');
console.log(JSON.stringify(summaries, null, 2));
console.log('=================================================================');
