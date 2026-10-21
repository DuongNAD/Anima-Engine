import * as THREE from 'three';
import { sampleMap2Terrain } from '../src/components/Map2/utils/map2TerrainGen';
import { Map2Biome, BIOME_METADATA } from '../src/components/Map2/utils/map2BiomeConfig';
import {
  Map2FloraSpecies,
  isTallFlora,
  FLORA_CANOPY_RADII,
} from '../src/components/Map2/utils/map2FloraGeometry';
import { generatePlacements, FloraPlacement } from '../src/components/Map2/Map2Vegetation';

console.log('========================================================================');
console.log('ADVERSARIAL STRESS TEST: Map 2 Vegetation Exhaustive Invariants Check');
console.log('========================================================================\n');

interface TestConfig {
  name: string;
  size: number;
  waterLevel: number;
  spawnClearanceRadius: number;
  quality: 'high' | 'low';
}

const configs: TestConfig[] = [
  { name: 'High Quality (Default)', size: 200, waterLevel: 12.0, spawnClearanceRadius: 6.0, quality: 'high' },
  { name: 'Low Quality (Performance Mode)', size: 200, waterLevel: 12.0, spawnClearanceRadius: 6.0, quality: 'low' },
  { name: 'Higher Water Level (15m flood)', size: 200, waterLevel: 15.0, spawnClearanceRadius: 6.0, quality: 'high' },
  { name: 'Low Water Level (8m drought)', size: 200, waterLevel: 8.0, spawnClearanceRadius: 6.0, quality: 'high' },
  { name: 'Wider World (250m)', size: 250, waterLevel: 12.0, spawnClearanceRadius: 6.0, quality: 'high' },
  { name: 'Compact World (150m)', size: 150, waterLevel: 12.0, spawnClearanceRadius: 6.0, quality: 'high' },
];

let globalFailures = 0;

for (const cfg of configs) {
  console.log(`\n--- Testing Config: ${cfg.name} (size=${cfg.size}, water=${cfg.waterLevel}, spawnR=${cfg.spawnClearanceRadius}, quality=${cfg.quality}) ---`);
  
  const result = generatePlacements(cfg.size, cfg.waterLevel, cfg.spawnClearanceRadius, cfg.quality);
  const { placements, tallTrees, specimenTrees, groundCover } = result;

  console.log(`  * Total Placements: ${placements.length}`);
  console.log(`  * Tall Trees: ${tallTrees.length}`);
  console.log(`  * Specimen Trees: ${specimenTrees.length}`);
  console.log(`  * Ground Cover: ${groundCover.length}`);

  // Test 1: Specimen Tree Isolation
  let specViolations = 0;
  let minSpecDist = Infinity;
  for (const st of specimenTrees) {
    for (const tt of tallTrees) {
      if (st === tt) continue;
      const d = Math.hypot(st.x - tt.x, st.z - tt.z);
      if (d < minSpecDist) minSpecDist = d;
      if (d < 14.0) {
        specViolations++;
        console.error(`  [FAIL] Specimen at (${st.x.toFixed(2)}, ${st.z.toFixed(2)}) has neighbor (${tt.x.toFixed(2)}, ${tt.z.toFixed(2)}) at distance ${d.toFixed(2)}m < 14.0m!`);
      }
    }
  }
  if (specimenTrees.length > 0) {
    console.log(`  [Pass/Fail] Specimen Isolation: Min Dist = ${minSpecDist.toFixed(2)}m (Violations: ${specViolations}) -> ${specViolations === 0 ? 'PASS' : 'FAIL'}`);
    if (specViolations > 0) globalFailures++;
  } else {
    console.log(`  [Pass/Fail] Specimen Isolation: No specimens generated (PASS)`);
  }

  // Test 2: Slope and Elevation Invariants for Tall Trees
  let tallOnCliff = 0;
  let tallOnSnowPeak = 0;
  let tallInWater = 0;
  for (const tt of tallTrees) {
    const s = sampleMap2Terrain(tt.x, tt.z);
    if (s.steepness >= 0.38) {
      tallOnCliff++;
      console.error(`  [FAIL] Tall tree at (${tt.x.toFixed(2)}, ${tt.z.toFixed(2)}) on cliff: steepness=${s.steepness.toFixed(3)} >= 0.38`);
    }
    if (tt.y >= 46.0) {
      tallOnSnowPeak++;
      console.error(`  [FAIL] Tall tree at (${tt.x.toFixed(2)}, ${tt.z.toFixed(2)}) on snow peak: y=${tt.y.toFixed(2)} >= 46.0`);
    }
    if (tt.y <= cfg.waterLevel || s.isWater) {
      tallInWater++;
      console.error(`  [FAIL] Tall tree at (${tt.x.toFixed(2)}, ${tt.z.toFixed(2)}) in water: y=${tt.y.toFixed(2)} <= ${cfg.waterLevel} or isWater=${s.isWater}`);
    }
  }
  console.log(`  [Pass/Fail] Ecological Constraints (Tall Trees): Cliffs=${tallOnCliff}, SnowPeaks=${tallOnSnowPeak}, Water=${tallInWater} -> ${tallOnCliff === 0 && tallOnSnowPeak === 0 && tallInWater === 0 ? 'PASS' : 'FAIL'}`);
  if (tallOnCliff > 0 || tallOnSnowPeak > 0 || tallInWater > 0) globalFailures++;

  // Test 3: Spawn Clearance
  let floraInSpawn = 0;
  let tallInSpawn = 0;
  for (const p of placements) {
    const dist = Math.hypot(p.x, p.z);
    if (dist < 6.0) {
      floraInSpawn++;
    }
    if (p.isTall && dist < 24.0) {
      tallInSpawn++;
    }
  }
  console.log(`  [Pass/Fail] Spawn Clearance: Flora < 6.0m: ${floraInSpawn}, Tall Trees < 24m: ${tallInSpawn} -> ${floraInSpawn === 0 && tallInSpawn === 0 ? 'PASS' : 'FAIL'}`);
  if (floraInSpawn > 0 || tallInSpawn > 0) globalFailures++;
}

console.log('\n========================================================================');
console.log(`ADVERSARIAL STRESS TEST SUMMARY: ${globalFailures === 0 ? 'ALL PASSED (0 FAILURES)' : `${globalFailures} CONFIG FAILURES`}`);
console.log('========================================================================');

if (globalFailures > 0) {
  process.exit(1);
}
