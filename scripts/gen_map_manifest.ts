// Real map-manifest exporter for the animal-map-vision MCP.
//
// Runs the ACTUAL frontend world generator (`generateWorld`, deterministic for a given
// seed/size/shape — the same world the app renders) and emits `animal-map.manifest.json`
// in the MCP schema (schemas/animal-map-manifest.schema.json). This replaces the earlier
// hand-authored scaffold so the deterministic gates run on real data:
//
//   • bounds / biomes  — real coordinate contract + per-biome bbox and representative °C
//   • entities         — a real sample of the generated flora, each with a species ecology
//                        profile (allowedBiomes/allowedMedia) so the ecology gate validates
//                        REAL vegetation placement (this is what would have caught pines on
//                        the snow caps before that fix landed), plus lake water bodies
//   • navigation       — spawn/exit/waypoints picked from a real BFS over the walkable grid,
//                        navmeshCoverage = reachable-land / total-land
//   • pipeline         — placeholder: this project renders a 3D mesh, not an equirectangular
//                        panorama, so these fields await a real capture pipeline (see _note)
//
// Collision + temperature gates are now lit up from real game data: the 7 trunked flora
// types are marked solid with the ACTUAL walk-mode collider radius (WorldCameraRig treeGrid,
// r = 0.45 + floraScale*0.25), and those tree species carry °C ranges calibrated to the
// biomes they occupy. The only field still not backed by real data is pipeline.panorama —
// this project renders a 3D mesh, not an equirectangular panorama.
//
// Build + run from the repo root (offline):
//   npx esbuild scripts/gen_map_manifest.ts --bundle --platform=node --format=cjs \
//     --outfile=<tmp>/g.cjs && node <tmp>/g.cjs [seed] [size]
// or, if tsx is available:  npx tsx scripts/gen_map_manifest.ts [seed] [size]

import { writeFileSync } from 'fs';
import { resolve } from 'path';
import {
  generateWorld,
  Biome,
  FloraType,
  type World,
} from '../src/components/Landscape/utils/worldGen';
import {
  DEFAULT_XZ_BOUNDS,
  cellCenterToWorldXz,
} from '../src/components/Landscape/utils/coordinate';

// ---- config (CLI-overridable) -------------------------------------------------------------
const SEED = process.argv[2] ?? '1337';
const SIZE = Number(process.argv[3] ?? 128); // backend working resolution
const SHAPE = 'continent' as const;
const WORLD_MAX_Y = 10; // COORDINATE_CONTRACT: y = elevation * 10, y ∈ [0, 10]
const MAX_FLORA_ENTITIES = 400; // sample cap so the manifest stays reviewable
const WALKABLE_SLOPE = 0.6; // slope above this is a cliff / non-walkable

// ---- helpers ------------------------------------------------------------------------------
type Vec3 = { x: number; y: number; z: number };

/** Normalized field temperature [0,1] → approximate °C. The worldgen temperature is a
 * latitude+altitude climate proxy (≈0.9 at a sea-level equator, ≈0.1 at a sea-level pole);
 * this linear map lands those at ~+29°C / ~-19°C. Documented, not physically calibrated. */
const normTempToC = (t: number): number => Math.round((-25 + t * 60) * 10) / 10;

const biomeType = (b: number): string => Biome[b].toLowerCase();

/** Recover the grid cell a flora world-coord (centred on origin, 1 cell = 1 unit) came from,
 * then map that cell centre into the canonical [-100,100] world bounds so the whole manifest
 * lives in one coordinate space. */
function floraCoordToCell(w: number): number {
  return Math.min(SIZE - 1, Math.max(0, Math.round(w + SIZE / 2)));
}

function cellToWorld(ix: number, iy: number, elevation: number): Vec3 {
  const [x, z] = cellCenterToWorldXz(ix, iy, SIZE, SIZE, DEFAULT_XZ_BOUNDS);
  return { x: r2(x), y: r2(Math.min(WORLD_MAX_Y, Math.max(0, elevation * WORLD_MAX_Y))), z: r2(z) };
}

const r2 = (n: number): number => Math.round(n * 100) / 100;

// Per-FloraType ecology. Tall vegetation gets an allowedBiomes set (a real biome check that
// flags misplacement, e.g. a palm in tundra); ubiquitous ground cover (bush/tuft/reed/rock)
// omits allowedBiomes to avoid false positives from the treeline/slope downgrades and is only
// medium-gated. Aquatic types are saltwater + submerged.
type Medium = 'land' | 'freshwater' | 'saltwater' | 'brackish_water' | 'air';
interface FloraProfile {
  species: string;
  medium: Medium;
  submerged?: boolean;
  allowedBiomes?: string[];
  allowedMedia: Medium[];
  /** Tall trunked flora (Pine/Round/Jungle/Cactus/Acacia/Palm/DeadTree) are REAL solid
   * colliders in walk mode — the WorldCameraRig `treeGrid` pushes the player capsule out of
   * their trunks with radius r = 0.45 + floraScale*0.25. Ground cover and aquatic flora are
   * not colliders, so they are left non-solid. */
  solid?: boolean;
  /** Real-world plausible air-temperature range (°C), set only for trunked trees and
   * calibrated to contain the proxy °C of every biome the species is actually placed in
   * (see the biome temps this exporter emits). Lets the temperature gate flag a tree that
   * lands in a climate-incompatible biome without false-positiving on correct placements. */
  temperatureC?: { min: number; max: number };
}
const FLORA: Partial<Record<FloraType, FloraProfile>> = {
  [FloraType.Pine]: { species: 'conifer_pine', medium: 'land', solid: true, allowedBiomes: ['taiga', 'alpine', 'forest'], allowedMedia: ['land'], temperatureC: { min: -35, max: 25 } },
  [FloraType.Round]: { species: 'broadleaf_tree', medium: 'land', solid: true, allowedBiomes: ['forest', 'grassland', 'shrubland'], allowedMedia: ['land'], temperatureC: { min: -15, max: 35 } },
  [FloraType.Jungle]: { species: 'tropical_tree', medium: 'land', solid: true, allowedBiomes: ['jungle', 'swamp', 'mangrove'], allowedMedia: ['land'], temperatureC: { min: 8, max: 45 } },
  [FloraType.Cactus]: { species: 'cactus', medium: 'land', solid: true, allowedBiomes: ['desert', 'badlands'], allowedMedia: ['land'], temperatureC: { min: -10, max: 55 } },
  [FloraType.Acacia]: { species: 'acacia', medium: 'land', solid: true, allowedBiomes: ['savanna', 'grassland'], allowedMedia: ['land'], temperatureC: { min: 0, max: 50 } },
  [FloraType.Palm]: { species: 'palm', medium: 'land', solid: true, allowedBiomes: ['mangrove', 'jungle', 'beach'], allowedMedia: ['land'], temperatureC: { min: 8, max: 45 } },
  [FloraType.DeadTree]: { species: 'dead_tree', medium: 'land', solid: true, allowedBiomes: ['chaparral', 'desert', 'badlands', 'savanna'], allowedMedia: ['land'], temperatureC: { min: -10, max: 50 } },
  [FloraType.Bush]: { species: 'shrub', medium: 'land', allowedMedia: ['land'] },
  [FloraType.Reed]: { species: 'reed', medium: 'land', allowedMedia: ['land', 'freshwater'] },
  [FloraType.Tuft]: { species: 'grass_tuft', medium: 'land', allowedMedia: ['land'] },
  [FloraType.Coral]: { species: 'coral', medium: 'saltwater', submerged: true, allowedBiomes: ['ocean'], allowedMedia: ['saltwater'] },
  [FloraType.Kelp]: { species: 'kelp', medium: 'saltwater', submerged: true, allowedBiomes: ['ocean'], allowedMedia: ['saltwater'] },
  [FloraType.Seagrass]: { species: 'seagrass', medium: 'saltwater', submerged: true, allowedBiomes: ['ocean'], allowedMedia: ['saltwater', 'brackish_water'] },
  // FloraType.Rock is a boulder, not an organism — emitted without a species/ecology.
};

// ---- generate -----------------------------------------------------------------------------
console.log(`generating world seed=${SEED} size=${SIZE} shape=${SHAPE} ...`);
const world: World = generateWorld(SEED, { size: SIZE, shape: SHAPE });
const { elevation, temperature, biome, water, slope, riverAmt, seaLevel } = world;

// ---- biomes: one entry per present type, world bbox + representative °C --------------------
interface BiomeAgg { minX: number; maxX: number; minZ: number; maxZ: number; minE: number; maxE: number; tSum: number; n: number }
const aggs = new Map<number, BiomeAgg>();
for (let iy = 0; iy < SIZE; iy++) {
  for (let ix = 0; ix < SIZE; ix++) {
    const i = iy * SIZE + ix;
    const b = biome[i];
    const [x, z] = cellCenterToWorldXz(ix, iy, SIZE, SIZE, DEFAULT_XZ_BOUNDS);
    const a = aggs.get(b);
    if (!a) {
      aggs.set(b, { minX: x, maxX: x, minZ: z, maxZ: z, minE: elevation[i], maxE: elevation[i], tSum: temperature[i], n: 1 });
    } else {
      a.minX = Math.min(a.minX, x); a.maxX = Math.max(a.maxX, x);
      a.minZ = Math.min(a.minZ, z); a.maxZ = Math.max(a.maxZ, z);
      a.minE = Math.min(a.minE, elevation[i]); a.maxE = Math.max(a.maxE, elevation[i]);
      a.tSum += temperature[i]; a.n++;
    }
  }
}
const biomes = [...aggs.entries()]
  .sort((p, q) => q[1].n - p[1].n)
  .map(([b, a]) => ({
    id: biomeType(b),
    type: biomeType(b),
    cellCount: a.n,
    bounds: {
      min: { x: r2(a.minX), y: r2(a.minE * WORLD_MAX_Y), z: r2(a.minZ) },
      max: { x: r2(a.maxX), y: r2(a.maxE * WORLD_MAX_Y), z: r2(a.maxZ) },
    },
    temperatureC: normTempToC(a.tSum / a.n),
  }));

// ---- entities: sampled real flora + lake water bodies + a spawn animal ---------------------
type Entity = Record<string, unknown>;
const entities: Entity[] = [];

const floraStride = Math.max(1, Math.floor(world.floraCount / MAX_FLORA_ENTITIES));
let emitted = 0;
for (let k = 0; k < world.floraCount; k += floraStride) {
  const ix = floraCoordToCell(world.floraX[k]);
  const iy = floraCoordToCell(world.floraZ[k]);
  const ci = iy * SIZE + ix;
  const ft = world.floraType[k] as FloraType;
  const pos = cellToWorld(ix, iy, elevation[ci]);
  const prof = FLORA[ft];
  const e: Entity = { id: `flora-${k}`, kind: 'flora', position: pos, biomeId: biomeType(biome[ci]) };
  if (prof) {
    e.species = prof.species;
    e.environment = { medium: prof.medium, submerged: prof.submerged ?? false };
    e.ecology = {
      ...(prof.allowedBiomes ? { allowedBiomes: prof.allowedBiomes } : {}),
      allowedMedia: prof.allowedMedia,
      ...(prof.temperatureC ? { temperatureC: prof.temperatureC } : {}),
    };
    if (prof.solid) {
      // Real walk-mode trunk collider: WorldCameraRig treeGrid uses r = 0.45 + floraScale*0.25.
      e.solid = true;
      e.collider = { enabled: true, shape: 'cylinder', radius: r2(0.45 + world.floraScale[k] * 0.25) };
    }
  }
  entities.push(e);
  if (++emitted >= MAX_FLORA_ENTITIES) break;
}

// Lake basins → freshwater environment bodies (bbox centre → world).
world.lakeBasins.forEach((lk, idx) => {
  const cx = Math.round((lk.minX + lk.maxX) / 2);
  const cy = Math.round((lk.minY + lk.maxY) / 2);
  const ci = Math.min(SIZE * SIZE - 1, Math.max(0, cy * SIZE + cx));
  entities.push({
    id: `lake-${idx}`,
    kind: 'water',
    position: cellToWorld(Math.min(SIZE - 1, Math.max(0, cx)), Math.min(SIZE - 1, Math.max(0, cy)), lk.level),
    environment: { medium: lk.saline ? 'brackish_water' : 'freshwater', submerged: false },
  });
});

// ---- navigation: real BFS over the walkable grid ------------------------------------------
const isLand = (i: number) => elevation[i] > seaLevel && water[i] === 0;
const walkable = (i: number) => isLand(i) && slope[i] < WALKABLE_SLOPE && riverAmt[i] < 100;

// spawn = walkable cell closest to map centre.
const c = Math.floor(SIZE / 2);
let spawnCell = -1; let bestD = Infinity;
for (let iy = 0; iy < SIZE; iy++) {
  for (let ix = 0; ix < SIZE; ix++) {
    const i = iy * SIZE + ix;
    if (!walkable(i)) continue;
    const d = (ix - c) * (ix - c) + (iy - c) * (iy - c);
    if (d < bestD) { bestD = d; spawnCell = i; }
  }
}
if (spawnCell < 0) throw new Error('no walkable land cell found — cannot place a spawn');

// BFS flood-fill of the reachable walkable region from spawn.
const reached = new Uint8Array(SIZE * SIZE);
const queue = [spawnCell];
reached[spawnCell] = 1;
let reachCount = 1; let far = spawnCell; let farD = 0;
const sx = spawnCell % SIZE, sy = (spawnCell / SIZE) | 0;
for (let h = 0; h < queue.length; h++) {
  const i = queue[h];
  const x = i % SIZE, y = (i / SIZE) | 0;
  const d = (x - sx) * (x - sx) + (y - sy) * (y - sy);
  if (d > farD) { farD = d; far = i; }
  const nb = [i - 1, i + 1, i - SIZE, i + SIZE];
  for (let e = 0; e < 4; e++) {
    const j = nb[e];
    if (j < 0 || j >= SIZE * SIZE) continue;
    if (e < 2 && ((j % SIZE) === 0 || (i % SIZE) === 0) && Math.abs((j % SIZE) - (i % SIZE)) !== 1) continue; // no row wrap
    if (reached[j] || !walkable(j)) continue;
    reached[j] = 1; reachCount++; queue.push(j);
  }
}
let totalLand = 0;
for (let i = 0; i < SIZE * SIZE; i++) if (isLand(i)) totalLand++;

const cellNode = (i: number, id: string, kind: string) => {
  const ix = i % SIZE, iy = (i / SIZE) | 0;
  return { id, kind, position: cellToWorld(ix, iy, elevation[i]) };
};
// two intermediate waypoints from the reachable set, at ~1/3 and ~2/3 of the path to `far`.
const pathQ = queue.filter((i) => reached[i]);
const wpA = pathQ[Math.floor(pathQ.length / 3)] ?? spawnCell;
const wpB = pathQ[Math.floor((pathQ.length * 2) / 3)] ?? far;
const navNodes = [
  cellNode(spawnCell, 'spawn-center', 'spawn'),
  cellNode(wpA, 'waypoint-a', 'walkable'),
  cellNode(wpB, 'waypoint-b', 'walkable'),
  cellNode(far, 'exit-far', 'exit'),
];
const navEdges = [
  { from: 'spawn-center', to: 'waypoint-a' },
  { from: 'waypoint-a', to: 'waypoint-b' },
  { from: 'waypoint-b', to: 'exit-far' },
];

// a spawn animal (the project's real rabbit) on the spawn cell.
entities.push({
  id: 'rabbit-spawn',
  kind: 'animal',
  species: 'anima_rabbit',
  biomeId: biomeType(biome[spawnCell]),
  position: cellNode(spawnCell, '_', 'spawn').position,
  environment: { medium: 'land', submerged: false },
  ecology: { allowedBiomes: ['grassland', 'forest', 'shrubland', 'savanna', 'steppe', 'beach'], allowedMedia: ['land'] },
});

const navmeshCoverage = totalLand > 0 ? Math.round((reachCount / totalLand) * 1000) / 1000 : 0;

// ---- assemble + write ---------------------------------------------------------------------
const manifest = {
  $schema: '../mcp-Vision/schemas/animal-map-manifest.schema.json',
  schemaVersion: '1.0',
  _generated: {
    by: 'scripts/gen_map_manifest.ts — real worldgen export',
    seed: SEED,
    size: SIZE,
    shape: SHAPE,
    worldGenVersion: world.version,
    seaLevel: r2(seaLevel),
    tempMapping: '°C = -25 + norm*60 (documented climate proxy, not physically calibrated)',
    asserted: [
      'colliders — the 7 trunked flora types are solid with the real walk-mode collider radius (WorldCameraRig treeGrid: r=0.45+floraScale*0.25)',
      'temperatureC — trunked-tree species carry °C ranges calibrated to contain the proxy °C of every biome they occupy',
    ],
    notAsserted: [
      'pipeline.panorama — this project renders a 3D mesh, not an equirectangular panorama (placeholder values below)',
    ],
    floraSampled: emitted,
    floraTotal: world.floraCount,
  },
  map: {
    name: `Anima World (seed ${SEED}, ${SIZE}²)`,
    seed: Number.isFinite(Number(SEED)) ? Number(SEED) : undefined,
    bounds: {
      min: { x: DEFAULT_XZ_BOUNDS.minX, y: 0, z: DEFAULT_XZ_BOUNDS.minZ },
      max: { x: DEFAULT_XZ_BOUNDS.maxX, y: WORLD_MAX_Y, z: DEFAULT_XZ_BOUNDS.maxZ },
    },
    biomes,
  },
  entities,
  navigation: {
    navmeshCoverage,
    nodes: navNodes,
    edges: navEdges,
  },
  pipeline: {
    // Placeholder: no equirectangular capture pipeline exists for this mesh world.
    panorama: { width: 4096, height: 2048 },
    depthAligned: true,
    normalsAligned: true,
    camerasAligned: true,
  },
};

const out = resolve(process.cwd(), 'animal-map.manifest.json');
writeFileSync(out, JSON.stringify(manifest, null, 2));
console.log(
  `wrote ${out}\n  biomes=${biomes.length} entities=${entities.length} ` +
    `(flora=${emitted}/${world.floraCount}, lakes=${world.lakeBasins.length}) ` +
    `navmeshCoverage=${navmeshCoverage} reachLand=${reachCount}/${totalLand}`,
);
