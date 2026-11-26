// Generate `artifacts/world-v2/world_v2_manifest.json` from committed World v2 artifacts (`WV2-6` / `WV2-BIG-4`).
//
// Never touches `artifacts/map_manifest.json`, `map_manifest.json`, `artifacts/world_256.anmw`, or any
// `WV2-4`/`WV2-5`/`WV2-BIG-3` committed artifact (`sim_256.anmw3`, `index.json`, `gates.json`, `preset_v2.toml`).
// Run via: `node scripts/run_ts.mjs scripts/gen_world_v2_manifest.ts`

import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import {
  WV2_ELEV_MAX_M,
  WV2_ELEV_MIN_M,
  WV2_ELEV_SPAN_M,
  decodeAnmwV3Sim,
  validateWorldV2Manifest,
  type AnmwV3Sim,
  type WorldV2CanonicalPose,
  type WorldV2CanonicalViewId,
  type WorldV2Manifest,
} from '../src/components/Landscape/utils/worldArtifactV3';
import { resolveWorldHeightRatio } from '../src/components/Landscape/worldV2/types';

function sha256Hex(bytes: Uint8Array): string {
  return createHash('sha256').update(bytes).digest('hex');
}

const BIOME_NAMES = [
  'Ocean', 'Beach', 'Desert', 'Savanna', 'Grassland', 'Shrubland', 'Forest', 'Jungle',
  'Taiga', 'Tundra', 'Swamp', 'Rock', 'Snow', 'River', 'Lake', 'Mangrove', 'Chaparral',
  'Steppe', 'Alpine', 'Badlands', 'Glacier', 'Bog',
] as const;

const FOREST_BIOMES = new Set([6, 7, 8]); // Forest, Jungle, Taiga
const WATERY_BIOMES = new Set([0, 13, 14]); // Ocean, River, Lake

const round2 = (v: number): number => Math.round(v * 100) / 100;
const roundTriple = (v: [number, number, number]): [number, number, number] => [
  round2(v[0]),
  round2(v[1]),
  round2(v[2]),
];

function deriveCanonicalPoses(
  sim: AnmwV3Sim,
  extentM: number,
  heightRatio: number,
): WorldV2Manifest['canonicalPoses'] {
  const N = sim.width;
  const sea = sim.seaLevelNorm;
  const canonH = 200 * heightRatio;

  const cellToX = (gx: number): number => (gx / (N - 1) - 0.5) * 200;
  const cellToZ = (gy: number): number => (gy / (N - 1) - 0.5) * 200;
  const mToCanon = (m: number): number => (m / extentM - 0.5) * 200;
  const idxAt = (x: number, z: number): number => {
    const gx = Math.max(0, Math.min(N - 1, Math.round((x / 200 + 0.5) * (N - 1))));
    const gy = Math.max(0, Math.min(N - 1, Math.round((z / 200 + 0.5) * (N - 1))));
    return gy * N + gx;
  };
  const surfCanon = (x: number, z: number): number => {
    const i = idxAt(x, z);
    return Math.max(sim.elevation[i], sea) * canonH;
  };
  const insideBounds = (x: number, z: number): boolean => Math.abs(x) <= 96 && Math.abs(z) <= 96;

  const frameTarget = (
    target: [number, number, number],
    dist: number,
    pitchDeg: number,
    prefAzRad: number,
  ): { position: [number, number, number]; target: [number, number, number] } => {
    const steps = 16;
    const order = [0];
    for (let k = 1; k <= steps / 2; k += 1) order.push(k, -k);
    for (const pDeg of [pitchDeg, pitchDeg + 12, pitchDeg + 25, 65]) {
      const pr = (pDeg * Math.PI) / 180;
      for (const k of order) {
        const az = prefAzRad + (k * 2 * Math.PI) / steps;
        const pos: [number, number, number] = [
          target[0] + dist * Math.cos(pr) * Math.cos(az),
          target[1] + dist * Math.sin(pr),
          target[2] + dist * Math.cos(pr) * Math.sin(az),
        ];
        if (!insideBounds(pos[0], pos[2])) continue;
        const clearance = Math.max(1.5, dist * 0.08);
        if (pos[1] < surfCanon(pos[0], pos[2]) + clearance) continue;
        let blocked = false;
        for (let s = 1; s < 24 && !blocked; s += 1) {
          const t = s / 24;
          const x = pos[0] + (target[0] - pos[0]) * t;
          const y = pos[1] + (target[1] - pos[1]) * t;
          const z = pos[2] + (target[2] - pos[2]) * t;
          if (surfCanon(x, z) > y + 0.15) blocked = true;
        }
        if (!blocked) return { position: roundTriple(pos), target: roundTriple(target) };
      }
    }
    const px = Math.max(-95, Math.min(95, target[0] + dist * 0.4 * Math.cos(prefAzRad)));
    const pz = Math.max(-95, Math.min(95, target[2] + dist * 0.4 * Math.sin(prefAzRad)));
    const py = Math.max(target[1] + dist * 0.85, surfCanon(px, pz) + 4);
    return { position: roundTriple([px, py, pz]), target: roundTriple(target) };
  };

  // 1. Highest peak -> lighting
  let peakIdx = 0;
  let maxElev = -1;
  for (let gy = 4; gy < N - 4; gy += 1) {
    for (let gx = 4; gx < N - 4; gx += 1) {
      const i = gy * N + gx;
      if (sim.elevation[i] > maxElev) {
        maxElev = sim.elevation[i];
        peakIdx = i;
      }
    }
  }
  const peakX = cellToX(peakIdx % N);
  const peakZ = cellToZ(Math.floor(peakIdx / N));
  const peakTarget: [number, number, number] = [peakX, maxElev * canonH, peakZ];
  const peakPose = frameTarget(peakTarget, 30, 18, Math.atan2(-peakZ, -peakX));

  // 2. Tallest waterfall -> water (fallback to largest inland lake if none)
  let waterPoseData: WorldV2CanonicalPose;
  if (sim.waterfalls.length > 0) {
    let bestWf = sim.waterfalls[0];
    for (const wf of sim.waterfalls) {
      if (wf.drop_m > bestWf.drop_m) bestWf = wf;
    }
    const wx = mToCanon(bestWf.x_m);
    const wz = mToCanon(bestWf.z_m);
    const topNorm = (bestWf.lip_elev_m - WV2_ELEV_MIN_M) / WV2_ELEV_SPAN_M;
    const dropCanon = (bestWf.drop_m / WV2_ELEV_SPAN_M) * canonH;
    const wy = topNorm * canonH - dropCanon * 0.5;
    const framed = frameTarget([wx, wy, wz], 18, 16, bestWf.yaw_rad);
    const wi = idxAt(wx, wz);
    waterPoseData = {
      ...framed,
      feature: `tallest waterfall of ${sim.waterfalls.length} (drop=${bestWf.drop_m.toFixed(1)} m, q=${bestWf.discharge_q_m3_s.toFixed(1)} m3/s)`,
      biome: BIOME_NAMES[sim.biome22[wi]] ?? `Biome(${sim.biome22[wi]})`,
    };
  } else {
    const framed = frameTarget([0, sea * canonH, 0], 45, 25, 0.5);
    waterPoseData = { ...framed, feature: 'sea level basin fallback', biome: 'Ocean' };
  }

  // 3. Largest inland river valley -> navigation
  let riverIdx = Math.floor((N * N) / 2);
  let bestFlow = -1;
  for (let gy = 8; gy < N - 8; gy += 1) {
    for (let gx = 8; gx < N - 8; gx += 1) {
      const i = gy * N + gx;
      if (sim.elevation[i] <= sea + 0.01) continue;
      if (sim.waterKind[i] !== 4 && sim.biome22[i] !== 13) continue; // River
      if (sim.flow[i] > bestFlow) {
        bestFlow = sim.flow[i];
        riverIdx = i;
      }
    }
  }
  const rivX = cellToX(riverIdx % N);
  const rivZ = cellToZ(Math.floor(riverIdx / N));
  const rivTarget: [number, number, number] = [rivX, sim.elevation[riverIdx] * canonH, rivZ];
  const rivPose = frameTarget(rivTarget, 24, 26, Math.atan2(-rivZ, -rivX));

  // 4. Gentle coast / beach -> spawn
  let coastIdx = Math.floor((N * N) / 2);
  let bestCoastScore = -Infinity;
  for (let gy = 10; gy < N - 10; gy += 1) {
    for (let gx = 10; gx < N - 10; gx += 1) {
      const i = gy * N + gx;
      if (sim.biome22[i] !== 1 && (sim.elevation[i] <= sea || sim.elevation[i] >= sea + 0.02)) continue;
      if (sim.slope[i] > 0.25) continue;
      const cx = cellToX(gx);
      const cz = cellToZ(gy);
      const score = (sim.biome22[i] === 1 ? 10 : 0) - Math.hypot(cx, cz) * 0.05;
      if (score > bestCoastScore) {
        bestCoastScore = score;
        coastIdx = i;
      }
    }
  }
  const coastX = cellToX(coastIdx % N);
  const coastZ = cellToZ(Math.floor(coastIdx / N));
  const coastTarget: [number, number, number] = [coastX, surfCanon(coastX, coastZ), coastZ];
  let oceanDx = 0;
  let oceanDz = 0;
  for (let dy = -4; dy <= 4; dy += 1) {
    for (let dx = -4; dx <= 4; dx += 1) {
      const nx = Math.max(0, Math.min(N - 1, (coastIdx % N) + dx));
      const ny = Math.max(0, Math.min(N - 1, Math.floor(coastIdx / N) + dy));
      if (sim.biome22[ny * N + nx] === 0) {
        oceanDx += dx;
        oceanDz += dy;
      }
    }
  }
  const coastAz = Math.hypot(oceanDx, oceanDz) > 0 ? Math.atan2(oceanDz, oceanDx) : Math.atan2(-coastZ, -coastX);
  const coastPose = frameTarget(coastTarget, 18, 20, coastAz);

  // 5. Densest inland forest/jungle cluster -> ecosystem
  let forestIdx = Math.floor((N * N) / 2);
  let bestForestCount = -1;
  for (let gy = 16; gy < N - 16; gy += 2) {
    for (let gx = 16; gx < N - 16; gx += 2) {
      let c = 0;
      for (let dy = -3; dy <= 3; dy += 1) {
        for (let dx = -3; dx <= 3; dx += 1) {
          const j = (gy + dy) * N + (gx + dx);
          if (FOREST_BIOMES.has(sim.biome22[j])) c += 1;
        }
      }
      const center = gy * N + gx;
      if (FOREST_BIOMES.has(sim.biome22[center]) && c > bestForestCount) {
        bestForestCount = c;
        forestIdx = center;
      }
    }
  }
  const forX = cellToX(forestIdx % N);
  const forZ = cellToZ(Math.floor(forestIdx / N));
  const forTarget: [number, number, number] = [forX, sim.elevation[forestIdx] * canonH, forZ];
  const forPose = frameTarget(forTarget, 22, 28, Math.atan2(-forZ, -forX));

  // 6. Richest biome transition window -> biome_transition
  let btIdx = Math.floor((N * N) / 2);
  let bestDiversity = -1;
  for (let gy = 16; gy < N - 16; gy += 2) {
    for (let gx = 16; gx < N - 16; gx += 2) {
      const center = gy * N + gx;
      if (WATERY_BIOMES.has(sim.biome22[center])) continue;
      const seen = new Set<number>();
      for (let dy = -4; dy <= 4; dy += 1) {
        for (let dx = -4; dx <= 4; dx += 1) {
          seen.add(sim.biome22[(gy + dy) * N + (gx + dx)]);
        }
      }
      if (seen.size > bestDiversity) {
        bestDiversity = seen.size;
        btIdx = center;
      }
    }
  }
  const btX = cellToX(btIdx % N);
  const btZ = cellToZ(Math.floor(btIdx / N));
  const btTarget: [number, number, number] = [btX, sim.elevation[btIdx] * canonH, btZ];
  const btPose = frameTarget(btTarget, 26, 28, Math.atan2(-btZ, -btX));

  // 7. Prominent cave / cliff edge -> collision
  let colPose: WorldV2CanonicalPose;
  if (sim.caveEntrances.length > 0) {
    let bestCave = sim.caveEntrances[0];
    let bestScore = -Infinity;
    for (const c of sim.caveEntrances) {
      const cx = mToCanon(c.x_m);
      const cz = mToCanon(c.z_m);
      if (Math.abs(cx) > 85 || Math.abs(cz) > 85) continue;
      const s = c.clearance_m - Math.hypot(cx, cz) * 0.01;
      if (s > bestScore) {
        bestScore = s;
        bestCave = c;
      }
    }
    const cx = mToCanon(bestCave.x_m);
    const cz = mToCanon(bestCave.z_m);
    const cy = ((bestCave.elevation_m - WV2_ELEV_MIN_M) / WV2_ELEV_SPAN_M) * canonH;
    const framed = frameTarget([cx, cy, cz], 16, 20, bestCave.yaw_rad);
    const ci = idxAt(cx, cz);
    colPose = {
      ...framed,
      feature: `karst cave entrance (clearance=${bestCave.clearance_m.toFixed(1)} m, elev=${bestCave.elevation_m.toFixed(1)} m)`,
      biome: BIOME_NAMES[sim.biome22[ci]] ?? `Biome(${sim.biome22[ci]})`,
    };
  } else {
    colPose = { ...forPose, feature: 'inland forest corridor', biome: BIOME_NAMES[sim.biome22[forestIdx]] ?? 'Forest' };
  }

  const views: Record<WorldV2CanonicalViewId, WorldV2CanonicalPose> = {
    overview: {
      position: [0, 112, 112],
      target: [0, 0, 0],
      feature: `whole ${Math.round(extentM / 1000)} km world from south overview`,
      biome: 'All',
    },
    navigation: {
      ...rivPose,
      feature: `river valley max-flow reach (flow=${bestFlow.toFixed(3)})`,
      biome: BIOME_NAMES[sim.biome22[riverIdx]] ?? `Biome(${sim.biome22[riverIdx]})`,
    },
    collision: colPose,
    lighting: {
      ...peakPose,
      feature: `highest peak (elev_norm=${maxElev.toFixed(4)})`,
      biome: BIOME_NAMES[sim.biome22[peakIdx]] ?? `Biome(${sim.biome22[peakIdx]})`,
    },
    spawn: {
      ...coastPose,
      feature: 'gentle coastal beach near landmass',
      biome: BIOME_NAMES[sim.biome22[coastIdx]] ?? `Biome(${sim.biome22[coastIdx]})`,
    },
    water: waterPoseData,
    biome_transition: {
      ...btPose,
      feature: `biome transition zone (${bestDiversity} distinct biomes in 9x9 window)`,
      biome: BIOME_NAMES[sim.biome22[btIdx]] ?? `Biome(${sim.biome22[btIdx]})`,
    },
    ecosystem: {
      ...forPose,
      feature: `dense tropical forest cluster (${bestForestCount}/49 canopy cells)`,
      biome: BIOME_NAMES[sim.biome22[forestIdx]] ?? `Biome(${sim.biome22[forestIdx]})`,
    },
  };

  return { heightRatio, views };
}

const ROOT = process.cwd();
const DIR = resolve(ROOT, 'artifacts', 'world-v2');

const simBytes = readFileSync(resolve(DIR, 'sim_256.anmw3'));
const indexBytes = readFileSync(resolve(DIR, 'index.json'));
const gatesBytes = readFileSync(resolve(DIR, 'gates.json'));
const presetBytes = readFileSync(resolve(DIR, 'preset_v2.toml'));

const index = JSON.parse(indexBytes.toString('utf8')) as {
  extent_m: number;
  format_version: number;
  generator_version: number;
  grid_size: number;
  release_archive: string;
  release_archive_sha256: string;
  schema_tag: string;
  seed: number;
  sim_256_byte_len: number;
  sim_256_file: string;
  sim_world_identity: string;
  tiles: Array<{ layer: string; mip: number }>;
};

const simWorldIdentity = sha256Hex(simBytes);
const bundleIdentity = sha256Hex(indexBytes);
const gatesSha256 = sha256Hex(gatesBytes);
const presetSha256 = sha256Hex(presetBytes);

if (simWorldIdentity !== index.sim_world_identity.toLowerCase()) {
  throw new Error(
    `sim_256.anmw3 SHA-256 (${simWorldIdentity}) != index.json sim_world_identity (${index.sim_world_identity})`,
  );
}
if (simBytes.byteLength !== index.sim_256_byte_len) {
  throw new Error(`sim_256.anmw3 length (${simBytes.byteLength}) != index.json (${index.sim_256_byte_len})`);
}

const decoded = decodeAnmwV3Sim(new Uint8Array(simBytes));
const expectedHeightRatio = resolveWorldHeightRatio('v2', index.extent_m);
const canonicalPoses = deriveCanonicalPoses(decoded, index.extent_m, expectedHeightRatio);

const manifest: WorldV2Manifest = {
  schemaVersion: 1,
  _generated: {
    by: 'scripts/gen_world_v2_manifest.ts',
    seed: index.seed,
    schemaTag: index.schema_tag,
    gridSize: index.grid_size,
    extentM: index.extent_m,
    generatorVersion: index.generator_version,
  },
  identities: {
    simWorldIdentity,
    bundleIdentity,
    releaseArchive: index.release_archive,
    releaseArchiveSha256: index.release_archive_sha256,
    gatesSha256,
    presetSha256,
  },
  simBundle: {
    path: 'artifacts/world-v2/sim_256.anmw3',
    bytes: simBytes.byteLength,
    width: decoded.width,
    height: decoded.height,
    seaLevelNorm: Math.round(decoded.seaLevelNorm * 1e8) / 1e8,
    simSeedU32: decoded.simSeedU32,
    waterfallCount: decoded.waterfalls.length,
    caveEntranceCount: decoded.caveEntrances.length,
  },
  indexSummary: {
    path: 'artifacts/world-v2/index.json',
    bytes: indexBytes.byteLength,
    tileEntryCount: index.tiles.length,
  },
  coordinateContract: {
    extentM: index.extent_m,
    elevMinM: WV2_ELEV_MIN_M,
    elevMaxM: WV2_ELEV_MAX_M,
    worldMinXZ: -100,
    worldMaxXZ: 100,
    worldMinY: 0,
    worldMaxY: 10,
    simGridDim: 256,
  },
  canonicalPoses,
  ownerGates: {
    publicTileRelease: 'open',
    irisXeBeforeAfterFps: 'open',
  },
};

const check = validateWorldV2Manifest(manifest, {
  sim256Sha256: simWorldIdentity,
  sim256Bytes: simBytes.byteLength,
  indexSha256: bundleIdentity,
  indexBytes: indexBytes.byteLength,
  gatesSha256,
  presetSha256,
  releaseArchiveSha256: index.release_archive_sha256,
  tileEntryCount: index.tiles.length,
  expectedHeightRatio,
  decodedSim: {
    width: decoded.width,
    height: decoded.height,
    seaLevelNorm: decoded.seaLevelNorm,
    simSeedU32: decoded.simSeedU32,
    generatorVersion: decoded.generatorVersion,
    waterfallCount: decoded.waterfalls.length,
    caveEntranceCount: decoded.caveEntrances.length,
  },
});

if (!check.ok) {
  throw new Error(`Manifest self-validation failed: ${check.errors.join('; ')}`);
}

const outPath = resolve(DIR, 'world_v2_manifest.json');
writeFileSync(outPath, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(
  `wrote artifacts/world-v2/world_v2_manifest.json\n` +
    `  simWorldIdentity: ${simWorldIdentity}\n` +
    `  bundleIdentity:   ${bundleIdentity}\n` +
    `  sim_256:          ${decoded.width}x${decoded.height}, waterfalls=${decoded.waterfalls.length}, caves=${decoded.caveEntrances.length}\n` +
    `  heightRatio:      ${expectedHeightRatio}\n` +
    `  tiles indexed:    ${index.tiles.length}`,
);
