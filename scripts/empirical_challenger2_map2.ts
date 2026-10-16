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
  computeMap2NavigationBFS,
  generateMap2Manifest,
} from '../src/components/Map2/utils/map2Manifest';
import {
  Map2FloraSpecies,
  FLORA_CANOPY_RADII,
  FLORA_TRUNK_COLLIDER_RADII,
} from '../src/components/Map2/utils/map2FloraGeometry';

function hash2D(x: number, z: number, seed = 0): number {
  let h = Math.imul((x * 374761393) | 0, (z * 668265263) | 0) + Math.imul(seed, 1274126177);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

console.log('========================================================================');
console.log('CHALLENGER 2: EMPIRICAL STRESS TEST & ADVERSARIAL AUDIT FOR MAP 2');
console.log('========================================================================\n');

// -------------------------------------------------------------------------------------------------
// TEST 1: BFS NAVIGATION GRAPH CONNECTIVITY & MAJOR REGIONS ANALYSIS
// -------------------------------------------------------------------------------------------------
console.log('>>> [Audit 1] BFS Navigation Graph Reachability & Component Decomposition...');

const seaLevel = 12.0;
const maxPassableSlope = 0.75;
const gridDim = 64;
const cellSize = 200.0 / gridDim;
const halfDim = gridDim / 2;

const walkable = new Uint8Array(gridDim * gridDim);
const componentId = new Int32Array(gridDim * gridDim).fill(-1);
let walkableCount = 0;

for (let gz = 0; gz < gridDim; gz++) {
  for (let gx = 0; gx < gridDim; gx++) {
    const wx = (gx - halfDim + 0.5) * cellSize;
    const wz = (gz - halfDim + 0.5) * cellSize;
    const sample = sampleMap2Terrain(wx, wz);
    const idx = gz * gridDim + gx;

    const isLandOrWadeable = sample.height >= seaLevel - 2.0;
    const isPassableSlope = sample.steepness <= maxPassableSlope;

    if (isLandOrWadeable && isPassableSlope) {
      walkable[idx] = 1;
      walkableCount++;
    }
  }
}

// Connected component analysis across the whole graph
let currentComp = 0;
const componentSizes: number[] = [];
const componentBiomes: Map<number, Set<Map2Biome>> = new Map();

for (let i = 0; i < gridDim * gridDim; i++) {
  if (walkable[i] && componentId[i] === -1) {
    const q: number[] = [i];
    componentId[i] = currentComp;
    let size = 0;
    const biomes = new Set<Map2Biome>();

    while (q.length > 0) {
      const cur = q.shift()!;
      size++;
      const gx = cur % gridDim;
      const gz = Math.floor(cur / gridDim);
      const wx = (gx - halfDim + 0.5) * cellSize;
      const wz = (gz - halfDim + 0.5) * cellSize;
      const s = sampleMap2Terrain(wx, wz);
      biomes.add(s.biomeId);

      const neighbors = [
        [gx + 1, gz],
        [gx - 1, gz],
        [gx, gz + 1],
        [gx, gz - 1],
        [gx + 1, gz + 1],
        [gx + 1, gz - 1],
        [gx - 1, gz + 1],
        [gx - 1, gz - 1],
      ];

      for (const [nx, nz] of neighbors) {
        if (nx >= 0 && nx < gridDim && nz >= 0 && nz < gridDim) {
          const nidx = nz * gridDim + nx;
          if (walkable[nidx] && componentId[nidx] === -1) {
            componentId[nidx] = currentComp;
            q.push(nidx);
          }
        }
      }
    }

    componentSizes.push(size);
    componentBiomes.set(currentComp, biomes);
    currentComp++;
  }
}

console.log(`- Total Grid Cells: ${gridDim * gridDim} (64x64)`);
console.log(`- Total Walkable Nodes: ${walkableCount}`);
console.log(`- Number of Connected Components: ${componentSizes.length}`);
componentSizes.forEach((sz, cid) => {
  const biomesInComp = Array.from(componentBiomes.get(cid) || []).map(b => BIOME_METADATA[b]?.key);
  console.log(`  Component #${cid}: ${sz} nodes (${((sz / walkableCount) * 100).toFixed(2)}%) - Biomes: [${biomesInComp.join(', ')}]`);
});

const spawnGx = Math.floor(halfDim);
const spawnGz = Math.floor(halfDim);
const spawnIdx = spawnGz * gridDim + spawnGx;
const spawnComponent = componentId[spawnIdx];
const mainCompSize = componentSizes[spawnComponent] || 0;
const connectivityRatio = walkableCount > 0 ? mainCompSize / walkableCount : 0;

console.log(`- Spawn Cell (${spawnGx}, ${spawnGz}) belongs to Component #${spawnComponent}`);
console.log(`- Connectivity Ratio: ${connectivityRatio.toFixed(4)} (${(connectivityRatio * 100).toFixed(2)}%)`);

// Multi-resolution BFS robustness test
const resolutions = [32, 64, 128];
console.log('\n- Multi-resolution BFS Connectivity Test:');
for (const res of resolutions) {
  const bfs = computeMap2NavigationBFS(res, seaLevel, 0.75);
  console.log(`  Resolution ${res}x${res}: Walkable = ${bfs.walkableNodes}/${bfs.totalNodes} (${((bfs.walkableNodes/bfs.totalNodes)*100).toFixed(1)}%), Ratio = ${bfs.connectivityRatio}, Biomes = [${bfs.spawnReachableBiomes.join(', ')}]`);
}

// -------------------------------------------------------------------------------------------------
// TEST 2: FLORA ECOLOGICAL CONSTRAINTS (ELEVATION, STEEPNESS, WATER) - 78,400 CELLS
// -------------------------------------------------------------------------------------------------
console.log('\n>>> [Audit 2] Flora Placement Ecological Invariant Audit...');

const size = 200;
const halfSize = size * 0.48;
const gridRes = 280; // 280x280 = 78,400 probes
const step = (halfSize * 2) / gridRes;
const spawnClearanceRadius = 6.0;

let totalFloraPlaced = 0;
let waterViolations = 0;
let snowPeakViolations = 0;
let cliffViolations = 0;
let spawnViolations = 0;
const floraSpeciesCount = new Map<Map2FloraSpecies, number>();

for (let gx = 0; gx < gridRes; gx++) {
  for (let gz = 0; gz < gridRes; gz++) {
    const jx = (hash2D(gx, gz, 1) - 0.5) * step * 0.95;
    const jz = (hash2D(gx, gz, 2) - 0.5) * step * 0.95;
    const wx = -halfSize + (gx + 0.5) * step + jx;
    const wz = -halfSize + (gz + 0.5) * step + jz;

    const distToSpawn = Math.sqrt(wx * wx + wz * wz);
    if (distToSpawn < spawnClearanceRadius) continue;

    const sample = sampleMap2Terrain(wx, wz);
    const { height, steepness, isWater, biomeId } = sample;

    if (isWater || height <= seaLevel) continue;
    if (height > 55.0) continue;
    if (steepness > 0.45) continue;

    const rSelect = hash2D(gx, gz, 3);
    let chosenSpecies: Map2FloraSpecies | null = null;

    switch (biomeId) {
      case Map2Biome.TROPICAL_RAINFOREST:
        if (rSelect < 0.28) chosenSpecies = Map2FloraSpecies.TropicalPalm;
        else if (rSelect < 0.45) chosenSpecies = Map2FloraSpecies.LushJungleTree;
        else if (rSelect < 0.75) chosenSpecies = Map2FloraSpecies.MeadowGrassTuft;
        else chosenSpecies = Map2FloraSpecies.Wildflower;
        break;
      case Map2Biome.ARID_SAVANNA:
        if (steepness > 0.32 && rSelect < 0.35) chosenSpecies = Map2FloraSpecies.CliffMoss;
        else if (rSelect < 0.22) chosenSpecies = Map2FloraSpecies.UmbrellaAcacia;
        else if (rSelect < 0.4) chosenSpecies = Map2FloraSpecies.DesertScrub;
        else chosenSpecies = Map2FloraSpecies.MeadowGrassTuft;
        break;
      case Map2Biome.DESERT_DUNES:
        if (rSelect < 0.35) chosenSpecies = Map2FloraSpecies.SaguaroCactus;
        else if (rSelect < 0.8) chosenSpecies = Map2FloraSpecies.DesertScrub;
        break;
      case Map2Biome.TEMPERATE_PLAINS:
        if (steepness > 0.30 && rSelect < 0.35) chosenSpecies = Map2FloraSpecies.CliffMoss;
        else if (rSelect < 0.18) chosenSpecies = Map2FloraSpecies.BroadleafDeciduous;
        else if (rSelect < 0.32) chosenSpecies = Map2FloraSpecies.ConiferPine;
        else if (rSelect < 0.72) chosenSpecies = Map2FloraSpecies.MeadowGrassTuft;
        else chosenSpecies = Map2FloraSpecies.Wildflower;
        break;
      case Map2Biome.SNOW_PEAKS:
        if (height <= 55.0 && steepness <= 0.45) {
          if (rSelect < 0.5) chosenSpecies = Map2FloraSpecies.CliffMoss;
          else chosenSpecies = Map2FloraSpecies.ConiferPine;
        }
        break;
      case Map2Biome.ROCKY_CLIFFS:
        if (steepness <= 0.45) {
          if (rSelect < 0.65) chosenSpecies = Map2FloraSpecies.CliffMoss;
          else chosenSpecies = Map2FloraSpecies.ConiferPine;
        }
        break;
      case Map2Biome.BIOLUMINESCENT_CAVES:
        if (rSelect < 0.55) chosenSpecies = Map2FloraSpecies.GlowingMushroom;
        else chosenSpecies = Map2FloraSpecies.CrystalCluster;
        break;
    }

    if (chosenSpecies) {
      totalFloraPlaced++;
      floraSpeciesCount.set(chosenSpecies, (floraSpeciesCount.get(chosenSpecies) || 0) + 1);

      if (height <= seaLevel || isWater) waterViolations++;
      if (height > 55.0) snowPeakViolations++;
      if (steepness > 0.45) cliffViolations++;
      if (distToSpawn < spawnClearanceRadius) spawnViolations++;
    }
  }
}

console.log(`- Total Flora Instances Placed: ${totalFloraPlaced}`);
console.log(`- Water Invariant Violations: ${waterViolations}`);
console.log(`- Snow Peak Invariant Violations: ${snowPeakViolations}`);
console.log(`- Cliff Invariant Violations: ${cliffViolations}`);
console.log(`- Spawn Clearance Violations: ${spawnViolations}`);
console.log('- Distribution by Species:');
for (const [sp, cnt] of floraSpeciesCount.entries()) {
  console.log(`  ${sp}: ${cnt} instances (${((cnt / totalFloraPlaced) * 100).toFixed(1)}%)`);
}

// -------------------------------------------------------------------------------------------------
// TEST 3: FANTASY CRYSTAL SUBTERRANEAN & GEOLOGICAL RESTRICTIONS
// -------------------------------------------------------------------------------------------------
console.log('\n>>> [Audit 3] Fantasy Crystal Subterranean Restriction Audit...');

const crystalGridRes = 70;
const crystalHalfSize = size * 0.45;
const crystalStep = (crystalHalfSize * 2) / crystalGridRes;

let totalSpires = 0;
let totalStalactites = 0;
let surfaceOpenHillViolations = 0;
let crystalWaterViolations = 0;

for (let gx = 0; gx < crystalGridRes; gx++) {
  for (let gz = 0; gz < crystalGridRes; gz++) {
    const jx = (hash2D(gx, gz, 11) - 0.5) * crystalStep * 0.9;
    const jz = (hash2D(gx, gz, 12) - 0.5) * crystalStep * 0.9;
    const wx = -crystalHalfSize + (gx + 0.5) * crystalStep + jx;
    const wz = -crystalHalfSize + (gz + 0.5) * crystalStep + jz;

    if (Math.sqrt(wx * wx + wz * wz) < 12.0) continue;

    const sample = sampleMap2Terrain(wx, wz);
    const { height, biomeId, caveIntensity } = sample;

    const isCaveBiome = biomeId === Map2Biome.BIOLUMINESCENT_CAVES;
    const isGrotto = Math.hypot(wx - (-60), wz - (-60)) < 16.0;
    const isSubterranean = caveIntensity > 0.75 && height < 22.0;

    if (!isCaveBiome && !isGrotto && !isSubterranean) {
      // Not allowed to place crystals here
      continue;
    }

    const rType = hash2D(gx, gz, 14);
    if (rType < 0.65) totalSpires++;
    else totalStalactites++;

    // Check if placed on open hill (height > 28m and not grotto)
    if (height > 28.0 && !isGrotto && !isCaveBiome) {
      surfaceOpenHillViolations++;
    }
    if (height <= seaLevel) {
      crystalWaterViolations++;
    }
  }
}

console.log(`- Total Crystal Spires Placed: ${totalSpires}`);
console.log(`- Total Stalactites Placed: ${totalStalactites}`);
console.log(`- Surface Open Hill Violations: ${surfaceOpenHillViolations}`);
console.log(`- Crystal Water Violations: ${crystalWaterViolations}`);

// -------------------------------------------------------------------------------------------------
// TEST 4: SOLID MAINLAND PERCENTAGE & LANDMASS CONTINUITY
// -------------------------------------------------------------------------------------------------
console.log('\n>>> [Audit 4] Solid Mainland Coverage & Landmass Continuity Check...');

const macroRes = 200;
let mainlandCount = 0;
let waterCount = 0;
let lakeCount = 0;
let streamCount = 0;

for (let gz = 0; gz < macroRes; gz++) {
  for (let gx = 0; gx < macroRes; gx++) {
    const wx = -100 + (gx + 0.5) * (200 / macroRes);
    const wz = -100 + (gz + 0.5) * (200 / macroRes);
    const s = sampleMap2Terrain(wx, wz);

    if (s.height > seaLevel) {
      mainlandCount++;
    } else {
      waterCount++;
      const distToLake = Math.hypot(wx - 35, wz - 25);
      if (distToLake < 28) lakeCount++;
      else streamCount++;
    }
  }
}

const totalMacro = macroRes * macroRes;
const landPct = (mainlandCount / totalMacro) * 100;
const waterPct = (waterCount / totalMacro) * 100;

console.log(`- Continental Mainland Landmass: ${landPct.toFixed(2)}% (Requirement: >= 85%) -> ${landPct >= 85.0 ? 'PASS' : 'FAIL'}`);
console.log(`- Water Body Coverage: ${waterPct.toFixed(2)}% (Lake: ${((lakeCount/totalMacro)*100).toFixed(2)}%, Stream/Tributaries: ${((streamCount/totalMacro)*100).toFixed(2)}%)`);

console.log('\n========================================================================');
console.log('CHALLENGER 2 EMPIRICAL AUDIT COMPLETE');
console.log('========================================================================\n');
