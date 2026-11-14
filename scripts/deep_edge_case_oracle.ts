import * as THREE from 'three';
import {
  Map2TerrainMathEngine,
} from '../src/components/Map2/utils/map2TerrainGen';
import {
  Map2Biome,
  Map2BiomeEcologySystem,
} from '../src/components/Map2/utils/map2BiomeConfig';
import {
  sampleMap2Water,
  GerstnerWaveDefinition,
} from '../src/components/Map2/utils/map2WaterMath';
import {
  buildMap2StarfieldGeometry,
} from '../src/components/Map2/utils/map2AtmosphereMath';

console.log('=== RUNNING DEEP EDGE CASE & SINGULARITY ORACLE ===');

let totalEdgeTests = 0;
let passedEdgeTests = 0;
const failures: string[] = [];

function assert(condition: boolean, msg: string) {
  totalEdgeTests++;
  if (condition) {
    passedEdgeTests++;
  } else {
    failures.push(msg);
    console.error(`FAIL: ${msg}`);
  }
}

// 1. Starfield geometry integrity
{
  const starGeo = buildMap2StarfieldGeometry(2000, 1150);
  assert(starGeo.getAttribute('position') !== undefined, 'Starfield has position attribute');
  assert(starGeo.getAttribute('position').count === 2000, 'Starfield position count == 2000');
  assert(starGeo.getAttribute('aSize').count === 2000, 'Starfield aSize count == 2000');
  assert(starGeo.getAttribute('aPhase').count === 2000, 'Starfield aPhase count == 2000');
  assert(starGeo.getAttribute('aTwinkleSpeed').count === 2000, 'Starfield aTwinkleSpeed count == 2000');
  assert(starGeo.getAttribute('aStarColor').count === 2000, 'Starfield aStarColor count == 2000');

  const posArr = starGeo.getAttribute('position').array as Float32Array;
  let hasNan = false;
  for (let i = 0; i < posArr.length; i++) {
    if (Number.isNaN(posArr[i]) || !Number.isFinite(posArr[i])) hasNan = true;
  }
  assert(!hasNan, 'Starfield attributes contain zero NaNs/Infs');
}

// 2. Wave math with degenerate/zero waves
{
  const emptyWaves: GerstnerWaveDefinition[] = [];
  const resEmpty = sampleMap2Water(0, 0, 0, 12.0, emptyWaves);
  assert(resEmpty.height === 12.0, 'Empty wave list returns exact sea level');
  assert(resEmpty.displacement.lengthSq() === 0, 'Empty wave displacement is zero vector');
  assert(Math.abs(resEmpty.normal.y - 1.0) < 1e-6, 'Empty wave normal is flat up (0, 1, 0)');

  const zeroAmpWave: GerstnerWaveDefinition[] = [
    { direction: [1, 0], amplitude: 0, wavelength: 10, speed: 1, steepness: 0.5 },
  ];
  const resZeroAmp = sampleMap2Water(50, 50, 10, 12.0, zeroAmpWave);
  assert(resZeroAmp.height === 12.0, 'Zero amplitude wave returns exact sea level');
  assert(Math.abs(resZeroAmp.normal.y - 1.0) < 1e-6, 'Zero amplitude wave normal is (0, 1, 0)');
}

// 3. Ecology boundary rules
{
  const eco = new Map2BiomeEcologySystem();

  // Exactly on waterLevel (12.0)
  assert(!eco.canPlaceFlora(Map2Biome.TEMPERATE_PLAINS, 12.0, 0.1), 'Flora disallowed at exact waterLevel 12.0');
  assert(eco.canPlaceFlora(Map2Biome.TEMPERATE_PLAINS, 12.01, 0.1), 'Flora allowed above waterLevel 12.01');

  // Exactly on snowLine (48.0)
  assert(!eco.canPlaceFlora(Map2Biome.SNOW_PEAKS, 48.0, 0.1), 'Flora disallowed at exact snowLine 48.0');
  assert(!eco.canPlaceFlora(Map2Biome.SNOW_PEAKS, 30.0, 0.1), 'Flora disallowed in SNOW_PEAKS even at low elevation');

  // Exactly on cliffThreshold (0.55)
  assert(!eco.canPlaceFlora(Map2Biome.ROCKY_CLIFFS, 25.0, 0.55), 'Flora disallowed at cliffThreshold 0.55');
  assert(eco.canPlaceFlora(Map2Biome.TEMPERATE_PLAINS, 25.0, 0.44), 'Flora allowed below cliffThreshold');

  // Validate fauna medium
  assert(eco.validateFaunaMedium('fish', new THREE.Vector3(0, 10, 0), 8.0, 0.1), 'Fish submerged valid');
  assert(!eco.validateFaunaMedium('fish', new THREE.Vector3(0, 14, 0), 8.0, 0.1), 'Fish above water level invalid');
  assert(eco.validateFaunaMedium('bird', new THREE.Vector3(0, 30, 0), 10.0, 0.1), 'Bird aloft valid');
  assert(!eco.validateFaunaMedium('bird', new THREE.Vector3(0, 12, 0), 10.0, 0.1), 'Bird too close to ground invalid');
  assert(eco.validateFaunaMedium('terrestrial', new THREE.Vector3(0, 15, 0), 15.0, 0.2), 'Terrestrial on dry gentle land valid');
  assert(!eco.validateFaunaMedium('terrestrial', new THREE.Vector3(0, 10, 0), 10.0, 0.2), 'Terrestrial in water invalid');
  assert(!eco.validateFaunaMedium('terrestrial', new THREE.Vector3(0, 25, 0), 25.0, 0.6), 'Terrestrial on sheer cliff invalid');
}

// 4. Subnormal and Extreme Floating Point Terrain Input
{
  const engine = new Map2TerrainMathEngine('anima_map2_adversarial_seed');
  const extremeCoords = [
    [0.0, 0.0],
    [-0.0, -0.0],
    [Number.MIN_VALUE, Number.MIN_VALUE],
    [-Number.MIN_VALUE, -Number.MIN_VALUE],
    [Number.EPSILON, Number.EPSILON],
    [1e-12, -1e-12],
    [1e6, 1e6],
    [-1e6, -1e6],
    [1e6, -1e6],
    [-1e6, 1e6],
  ];

  for (const [x, z] of extremeCoords) {
    const s = engine.sample(x, z);
    assert(Number.isFinite(s.height), `Height is finite at (${x}, ${z})`);
    assert(s.height >= 0 && s.height <= 90.0, `Height bounded at (${x}, ${z})`);
    assert(Math.abs(s.normal.length() - 1.0) < 1e-4, `Normal length == 1 at (${x}, ${z})`);
    assert(s.steepness >= 0 && s.steepness <= 1.0, `Steepness in [0, 1] at (${x}, ${z})`);
    assert(s.temperature >= 0 && s.temperature <= 1.0, `Temperature in [0, 1] at (${x}, ${z})`);
    assert(s.moisture >= 0 && s.moisture <= 1.0, `Moisture in [0, 1] at (${x}, ${z})`);
  }
}

console.log(`\n=============================================`);
console.log(`ORACLE RESULTS: ${passedEdgeTests} / ${totalEdgeTests} passed (${failures.length} failures)`);
if (failures.length === 0) {
  console.log('ALL EDGE CASES & INVARIANTS VERIFIED SUCCESSFULLY!');
}
console.log(`=============================================`);
