import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  decodeAnmwV3Sim,
  validateWorldV2Manifest,
  type WorldV2Manifest,
} from '../../src/components/Landscape/utils/worldArtifactV3';

const HERE = dirname(fileURLToPath(import.meta.url));
const WORLD_V2_DIR = resolve(HERE, '../../artifacts/world-v2');

function sha256Hex(bytes: Buffer): string {
  return createHash('sha256').update(bytes).digest('hex');
}

describe('world_v2_manifest.json committed evidence (WV2-6)', () => {
  it('matches committed sim_256.anmw3, index.json, gates.json, and preset_v2.toml', () => {
    const manifest = JSON.parse(
      readFileSync(resolve(WORLD_V2_DIR, 'world_v2_manifest.json'), 'utf8'),
    ) as WorldV2Manifest;
    const simBytes = readFileSync(resolve(WORLD_V2_DIR, 'sim_256.anmw3'));
    const indexBytes = readFileSync(resolve(WORLD_V2_DIR, 'index.json'));
    const gatesBytes = readFileSync(resolve(WORLD_V2_DIR, 'gates.json'));
    const presetBytes = readFileSync(resolve(WORLD_V2_DIR, 'preset_v2.toml'));

    const index = JSON.parse(indexBytes.toString('utf8')) as {
      release_archive_sha256: string;
      tiles: unknown[];
    };
    const decoded = decodeAnmwV3Sim(new Uint8Array(simBytes));

    const result = validateWorldV2Manifest(manifest, {
      sim256Sha256: sha256Hex(simBytes),
      sim256Bytes: simBytes.byteLength,
      indexSha256: sha256Hex(indexBytes),
      indexBytes: indexBytes.byteLength,
      gatesSha256: sha256Hex(gatesBytes),
      presetSha256: sha256Hex(presetBytes),
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

    expect(result.errors).toEqual([]);
    expect(result.ok).toBe(true);
    expect(manifest.ownerGates.publicTileRelease).toBe('open');
    expect(manifest.ownerGates.irisXeBeforeAfterFps).toBe('open');
  });

  it('rejects a stale manifest identity or drifted tile count (negative control)', () => {
    const manifest = JSON.parse(
      readFileSync(resolve(WORLD_V2_DIR, 'world_v2_manifest.json'), 'utf8'),
    ) as WorldV2Manifest;
    const simBytes = readFileSync(resolve(WORLD_V2_DIR, 'sim_256.anmw3'));
    const indexBytes = readFileSync(resolve(WORLD_V2_DIR, 'index.json'));
    const gatesBytes = readFileSync(resolve(WORLD_V2_DIR, 'gates.json'));
    const presetBytes = readFileSync(resolve(WORLD_V2_DIR, 'preset_v2.toml'));
    const index = JSON.parse(indexBytes.toString('utf8')) as {
      release_archive_sha256: string;
      tiles: unknown[];
    };
    const decoded = decodeAnmwV3Sim(new Uint8Array(simBytes));

    const stale: WorldV2Manifest = {
      ...manifest,
      identities: {
        ...manifest.identities,
        simWorldIdentity: '0'.repeat(64),
      },
      indexSummary: {
        ...manifest.indexSummary,
        tileEntryCount: manifest.indexSummary.tileEntryCount - 1,
      },
    };

    const result = validateWorldV2Manifest(stale, {
      sim256Sha256: sha256Hex(simBytes),
      sim256Bytes: simBytes.byteLength,
      indexSha256: sha256Hex(indexBytes),
      indexBytes: indexBytes.byteLength,
      gatesSha256: sha256Hex(gatesBytes),
      presetSha256: sha256Hex(presetBytes),
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

    expect(result.ok).toBe(false);
    expect(result.errors.length).toBeGreaterThanOrEqual(2);
  });
});
