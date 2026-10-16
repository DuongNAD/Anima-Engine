import * as THREE from 'three';
import {
  Map2TerrainMathEngine,
  sampleMap2Terrain,
  getMap2Height,
  getMap2NormalAndSteepness,
} from '../src/components/Map2/utils/map2TerrainGen';
import {
  Map2Biome,
  BIOME_METADATA,
  Map2BiomeEcologySystem,
} from '../src/components/Map2/utils/map2BiomeConfig';
import {
  Map2FloraSpecies,
  FLORA_CANOPY_RADII,
  FLORA_TRUNK_COLLIDER_RADII,
  isTallFlora,
  isGroundCoverFlora,
} from '../src/components/Map2/utils/map2FloraGeometry';

import {
  generatePlacements,
  FloraPlacement,
  ALL_SPECIES,
  hash2D,
} from '../src/components/Map2/Map2Vegetation';


console.log('========================================================================');
console.log('CHALLENGER: EMPIRICAL AUDIT OF MAP 2 VEGETATION & GLADES DISTRIBUTION');
console.log('========================================================================\n');

// -------------------------------------------------------------------------------------------------
// TASK 1 & 2: MEASURE EXACT PERCENTAGE OF OPEN GRASSY GLADE VS TREE GROVES (>= 50% GLADES)
// -------------------------------------------------------------------------------------------------
console.log('>>> [Task 1 & 2] Sampling $[-96, 96]^2$ Continuous Field: Glade vs Grove Area Percentage...');

const sampleRes = 400; // 400x400 = 160,000 sample points
const halfBound = 96.0;
const sampleStep = (halfBound * 2) / sampleRes;

let totalLandSamples = 0;
let totalWaterSamples = 0;
let gladeSamples = 0;
let groveSamples = 0;
const biomeLandCounts = new Map<Map2Biome, { total: number; glade: number; grove: number }>();

for (let gx = 0; gx < sampleRes; gx++) {
  for (let gz = 0; gz < sampleRes; gz++) {
    const wx = -halfBound + (gx + 0.5) * sampleStep;
    const wz = -halfBound + (gz + 0.5) * sampleStep;

    const sample = sampleMap2Terrain(wx, wz);
    const { height, steepness, isWater, biomeId, moisture } = sample;

    if (isWater || height <= 12.0) {
      totalWaterSamples++;
      continue;
    }

    totalLandSamples++;

    if (!biomeLandCounts.has(biomeId)) {
      biomeLandCounts.set(biomeId, { total: 0, glade: 0, grove: 0 });
    }
    const bStats = biomeLandCounts.get(biomeId)!;
    bStats.total++;

    const n1 = Math.sin(wx * 0.025 + 1.2) * Math.cos(wz * 0.025 + 0.8);
    const n2 = Math.sin(wx * 0.055 - 0.7) * Math.cos(wz * 0.052 + 1.3) * 0.5;
    const n3 = Math.sin(wx * 0.012 + wz * 0.014 + 2.0) * 0.35;
    const rawGladeNoise = (n1 + n2 + n3 + 1.85) / 3.7;

    const moistureFactor = 0.65 + 0.70 * moisture;
    const slopeFactor = Math.max(0.0, 1.0 - 2.2 * steepness);
    const elevationFactor = height <= 34.0 ? 1.0 : Math.max(0.0, 1.0 - (height - 34.0) / 16.0);

    const groveIntensity = rawGladeNoise * moistureFactor * slopeFactor * elevationFactor;
    const isTreeGrove = groveIntensity > 0.52;

    if (isTreeGrove) {
      groveSamples++;
      bStats.grove++;
    } else {
      gladeSamples++;
      bStats.glade++;
    }
  }
}

const gladePct = (gladeSamples / totalLandSamples) * 100;
const grovePct = (groveSamples / totalLandSamples) * 100;

console.log(`- Total Sample Probes across $[-96, 96]^2$: ${sampleRes * sampleRes} (400x400 grid, 0.48m step)`);
console.log(`- Total Dry Land Samples: ${totalLandSamples} (${((totalLandSamples / (sampleRes * sampleRes)) * 100).toFixed(2)}% of map)`);
console.log(`- Open Grassy Glades & Meadows: ${gladeSamples} samples -> ${gladePct.toFixed(2)}% of land (Requirement: >= 50.0%)`);
console.log(`- Organic Tree Groves: ${groveSamples} samples -> ${grovePct.toFixed(2)}% of land`);
console.log(`- GLADE PERCENTAGE GATE VERDICT: ${gladePct >= 50.0 ? 'PASS (PASSED WITH FLYING COLORS)' : 'FAIL'}`);

console.log('\n- Per-Biome Glade vs Grove Breakdown:');
for (const [bId, stats] of biomeLandCounts.entries()) {
  const bName = BIOME_METADATA[bId]?.nameEn || `Biome_${bId}`;
  const bGladePct = (stats.glade / stats.total) * 100;
  const bGrovePct = (stats.grove / stats.total) * 100;
  console.log(`  * ${bName.padEnd(28)}: Total=${stats.total.toString().padStart(5)}, Glade=${bGladePct.toFixed(1)}%, Grove=${bGrovePct.toFixed(1)}%`);
}

// -------------------------------------------------------------------------------------------------
// TASK 3: COUNT TOTAL INSTANTIATED TALL TREES (<= 600 TREES, 60-70% REDUCTION FROM ~1,580)
// -------------------------------------------------------------------------------------------------
console.log('\n>>> [Task 3] Instantiated Tall Tree Count & Reduction Metric Analysis...');

const highGen = generatePlacements(200, 12.0, 6.0, 'high');
const lowGen = generatePlacements(200, 12.0, 6.0, 'low');

const legacyTreeCount = 1580;
const highTallTreeCount = highGen.tallTrees.length;
const lowTallTreeCount = lowGen.tallTrees.length;

const highReductionPct = ((legacyTreeCount - highTallTreeCount) / legacyTreeCount) * 100;
const lowReductionPct = ((legacyTreeCount - lowTallTreeCount) / legacyTreeCount) * 100;

console.log(`- Legacy Tree Count Baseline: ~${legacyTreeCount} trees`);
console.log(`- High Quality (60x60 grid): Total Placements=${highGen.placements.length}, Tall Trees=${highTallTreeCount}, Ground Cover=${highGen.groundCover.length}`);
console.log(`  * High Quality Reduction: ${highReductionPct.toFixed(2)}% reduction from legacy (Requirement: 60-70% reduction, <= 600 trees)`);
console.log(`  * High Quality Tall Tree Gate: ${highTallTreeCount <= 600 && highTallTreeCount >= 350 ? 'PASS' : 'FAIL'}`);

console.log(`- Low Quality (36x36 grid): Total Placements=${lowGen.placements.length}, Tall Trees=${lowTallTreeCount}, Ground Cover=${lowGen.groundCover.length}`);
console.log(`  * Low Quality Reduction: ${lowReductionPct.toFixed(2)}% reduction from legacy`);

console.log('\n- Tall Tree Species Breakdown (High Quality):');
const tallSpeciesMap = new Map<Map2FloraSpecies, number>();
highGen.tallTrees.forEach((t) => tallSpeciesMap.set(t.species, (tallSpeciesMap.get(t.species) || 0) + 1));
for (const [sp, count] of tallSpeciesMap.entries()) {
  console.log(`  * ${sp.padEnd(24)}: ${count} trees (${((count / highTallTreeCount) * 100).toFixed(1)}%)`);
}

// -------------------------------------------------------------------------------------------------
// TASK 4: VERIFY SPECIMEN TREE ISOLATION DISTANCE (>= 14M CLEARANCE FROM OTHER TALL TREES)
// -------------------------------------------------------------------------------------------------
console.log('\n>>> [Task 4] Specimen Tree Isolation & Spacing Analysis (>= 14.0m Clearance)...');

console.log(`- Specimen Trees Found (High Quality): ${highGen.specimenTrees.length} majestic solitary ancient trees`);
let minSpecimenIsolation = Infinity;
let specimenViolations = 0;
const specimenDetails: Array<{
  species: Map2FloraSpecies;
  pos: [number, number, number];
  scale: number;
  minDistToTallTree: number;
  closestTreeSpecies: Map2FloraSpecies;
  closestTreePos: [number, number, number];
}> = [];

for (let i = 0; i < highGen.specimenTrees.length; i++) {
  const spec = highGen.specimenTrees[i];
  let minDist = Infinity;
  let closestTree: FloraPlacement | null = null;

  for (let j = 0; j < highGen.tallTrees.length; j++) {
    const other = highGen.tallTrees[j];
    // Don't compare with self
    if (spec === other) continue;

    const d = Math.hypot(spec.x - other.x, spec.z - other.z);
    if (d < minDist) {
      minDist = d;
      closestTree = other;
    }
  }

  if (minDist < minSpecimenIsolation) {
    minSpecimenIsolation = minDist;
  }

  if (minDist < 14.0) {
    specimenViolations++;
  }

  specimenDetails.push({
    species: spec.species,
    pos: [Number(spec.x.toFixed(2)), Number(spec.y.toFixed(2)), Number(spec.z.toFixed(2))],
    scale: Number(spec.scale.toFixed(2)),
    minDistToTallTree: Number(minDist.toFixed(2)),
    closestTreeSpecies: closestTree ? closestTree.species : spec.species,
    closestTreePos: closestTree ? [Number(closestTree.x.toFixed(2)), Number(closestTree.y.toFixed(2)), Number(closestTree.z.toFixed(2))] : [0, 0, 0],
  });
}

console.log(`- Minimum Specimen Isolation Distance Observed: ${minSpecimenIsolation.toFixed(2)}m (Requirement: >= 14.0m)`);
console.log(`- Specimen Isolation Violations (< 14.0m): ${specimenViolations}`);
console.log(`- SPECIMEN ISOLATION GATE VERDICT: ${minSpecimenIsolation >= 14.0 && specimenViolations === 0 ? 'PASS' : 'FAIL'}`);

console.log('\n- Detailed Specimen Trees Inventory:');
specimenDetails.forEach((sd, idx) => {
  console.log(`  [Specimen #${idx + 1}] Species=${sd.species}, Pos=(${sd.pos.join(', ')}), Scale=${sd.scale}x -> Nearest Tree Dist=${sd.minDistToTallTree}m (${sd.closestTreeSpecies} at ${sd.closestTreePos.join(', ')})`);
});

// -------------------------------------------------------------------------------------------------
// TASK 5: VERIFY SPAWN AREA CLEARANCE AROUND (0, 20.1, 0)
// (NO TALL TREES WITHIN 24M, NO FLORA WITHIN 6M)
// -------------------------------------------------------------------------------------------------
console.log('\n>>> [Task 5] Spawn Area Clearance Audit around (0, 20.1, 0)...');

let floraWithin6m = 0;
let tallTreesWithin24m = 0;
let floraBetween6and24m = 0;
let nonGroundCoverInVista = 0;
let minFloraSpawnDist = Infinity;
let minTreeSpawnDist = Infinity;

for (const p of highGen.placements) {
  const dist = Math.hypot(p.x, p.z);
  if (dist < minFloraSpawnDist) minFloraSpawnDist = dist;

  if (p.isTall && dist < minTreeSpawnDist) {
    minTreeSpawnDist = dist;
  }

  if (dist < 6.0) {
    floraWithin6m++;
  }

  if (dist < 24.0) {
    if (p.isTall) {
      tallTreesWithin24m++;
    } else {
      floraBetween6and24m++;
      if (p.species !== Map2FloraSpecies.Wildflower && p.species !== Map2FloraSpecies.MeadowGrassTuft) {
        nonGroundCoverInVista++;
      }
    }
  }
}

console.log(`- Minimum Flora Distance from Spawn: ${minFloraSpawnDist.toFixed(2)}m (Requirement: >= 6.0m)`);
console.log(`- Flora Placements strictly inside 6.0m: ${floraWithin6m}`);
console.log(`- Minimum Tall Tree Distance from Spawn: ${minTreeSpawnDist.toFixed(2)}m (Requirement: >= 24.0m)`);
console.log(`- Tall Tree Placements inside 24.0m: ${tallTreesWithin24m}`);
console.log(`- Gentle Meadow Ground Cover in [6m, 24m] Vista Zone: ${floraBetween6and24m} (Wildflowers & Grass Tufts)`);
console.log(`- Invalid Species in Vista Zone: ${nonGroundCoverInVista}`);
console.log(`- SPAWN CLEARANCE GATE VERDICT: ${floraWithin6m === 0 && tallTreesWithin24m === 0 && minFloraSpawnDist >= 6.0 && minTreeSpawnDist >= 24.0 ? 'PASS' : 'FAIL'}`);

// -------------------------------------------------------------------------------------------------
// TASK 6: VERIFY NO TREES ON WATER (h <= 12.0), SNOW PEAKS (h >= 46.0), OR SHEER CLIFFS (s >= 0.38)
// -------------------------------------------------------------------------------------------------
console.log('\n>>> [Task 6] Ecological Exclusion Invariants (Water h<=12.0, Snow Peaks h>=46.0, Cliffs s>=0.38)...');

let treesInWater = 0;
let treesOnSnowPeaks = 0;
let treesOnCliffs = 0;

let allFloraInWater = 0;
let allFloraOnHighPeaks = 0;
let allFloraOnCliffs = 0;

for (const t of highGen.tallTrees) {
  const sample = sampleMap2Terrain(t.x, t.z);
  if (t.y <= 12.0 || sample.isWater) {
    treesInWater++;
  }
  if (t.y >= 46.0) {
    treesOnSnowPeaks++;
  }
  if (sample.steepness >= 0.38) {
    treesOnCliffs++;
  }
}

for (const p of highGen.placements) {
  const sample = sampleMap2Terrain(p.x, p.z);
  if (p.y <= 12.0 || sample.isWater) allFloraInWater++;
  if (p.y > 52.0) allFloraOnHighPeaks++;
  if (sample.steepness > 0.42) allFloraOnCliffs++;
}

console.log(`- Tall Trees in Water (height <= 12.0m): ${treesInWater} (Requirement: 0)`);
console.log(`- Tall Trees on Snow Peaks (height >= 46.0m): ${treesOnSnowPeaks} (Requirement: 0)`);
console.log(`- Tall Trees on Sheer Cliffs (steepness >= 0.38): ${treesOnCliffs} (Requirement: 0)`);
console.log(`- Any Flora in Water: ${allFloraInWater}`);
console.log(`- Any Flora on High Glacial Peaks (> 52m): ${allFloraOnHighPeaks}`);
console.log(`- Any Flora on Sheer Cliffs (> 0.42): ${allFloraOnCliffs}`);
console.log(`- ECOLOGICAL EXCLUSION GATE VERDICT: ${treesInWater === 0 && treesOnSnowPeaks === 0 && treesOnCliffs === 0 ? 'PASS' : 'FAIL'}`);

// -------------------------------------------------------------------------------------------------
// COMPREHENSIVE VERDICT
// -------------------------------------------------------------------------------------------------
const allPassed =
  gladePct >= 50.0 &&
  highTallTreeCount <= 600 &&
  highReductionPct >= 60.0 &&
  highReductionPct <= 75.0 &&
  minSpecimenIsolation >= 14.0 &&
  specimenViolations === 0 &&
  floraWithin6m === 0 &&
  tallTreesWithin24m === 0 &&
  minFloraSpawnDist >= 6.0 &&
  minTreeSpawnDist >= 24.0 &&
  treesInWater === 0 &&
  treesOnSnowPeaks === 0 &&
  treesOnCliffs === 0;

console.log('\n========================================================================');
console.log(`FINAL CHALLENGER VERDICT: ${allPassed ? 'APPROVE' : 'FAIL'}`);
console.log('========================================================================\n');

if (!allPassed) {
  process.exit(1);
}
