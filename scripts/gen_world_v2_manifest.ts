// Generate `artifacts/world-v2/world_v2_manifest.json` from committed World v2 artifacts (`WV2-6`, `§7.2`).
//
// Never touches `artifacts/map_manifest.json`, `map_manifest.json`, `artifacts/world_256.anmw`, or any
// `WV2-4`/`WV2-5` committed artifact (`sim_256.anmw3`, `index.json`, `gates.json`, `preset_v2.toml`).
// Run via: `node scripts/run_ts.mjs scripts/gen_world_v2_manifest.ts`

import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import {
  WV2_ELEV_MAX_M,
  WV2_ELEV_MIN_M,
  decodeAnmwV3Sim,
  validateWorldV2Manifest,
  type WorldV2Manifest,
} from '../src/components/Landscape/utils/worldArtifactV3';

function sha256Hex(bytes: Uint8Array): string {
  return createHash('sha256').update(bytes).digest('hex');
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
    `  tiles indexed:    ${index.tiles.length}`,
);
