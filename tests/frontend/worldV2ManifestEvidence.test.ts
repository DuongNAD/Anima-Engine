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
import { resolveWorldHeightRatio } from '../../src/components/Landscape/worldV2/types';

const HERE = dirname(fileURLToPath(import.meta.url));
const WORLD_V2_DIR = resolve(HERE, '../../artifacts/world-v2');
const OLD_C01_SIM_WORLD_IDENTITY =
  'cbdd3ba9f57725ada233d1b43f2f5804fe8cb0fe0b098ca5ef8ae0aae5784c4d';
const OLD_C01_BUNDLE_IDENTITY =
  '2f002d1bf976131e3034f9d574a3b7089c291a0c6c0803236ee6817c0ab8cafe';

function sha256Hex(bytes: Buffer): string {
  return createHash('sha256').update(bytes).digest('hex');
}

function loadActualEvidence() {
  const simBytes = readFileSync(resolve(WORLD_V2_DIR, 'sim_256.anmw3'));
  const indexBytes = readFileSync(resolve(WORLD_V2_DIR, 'index.json'));
  const gatesBytes = readFileSync(resolve(WORLD_V2_DIR, 'gates.json'));
  const presetBytes = readFileSync(resolve(WORLD_V2_DIR, 'preset_v2.toml'));

  const index = JSON.parse(indexBytes.toString('utf8')) as {
    extent_m: number;
    release_archive_sha256: string;
    tiles: unknown[];
  };
  const decoded = decodeAnmwV3Sim(new Uint8Array(simBytes));
  const expectedHeightRatio = resolveWorldHeightRatio('v2', index.extent_m);

  return {
    index,
    decoded,
    actual: {
      sim256Sha256: sha256Hex(simBytes),
      sim256Bytes: simBytes.byteLength,
      indexSha256: sha256Hex(indexBytes),
      indexBytes: indexBytes.byteLength,
      gatesSha256: sha256Hex(gatesBytes),
      presetSha256: sha256Hex(presetBytes),
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
    },
  };
}

describe('world_v2_manifest.json committed evidence (WV2-BIG-4)', () => {
  it('matches committed BIG_4096_v2 artifacts and synced canonical poses', () => {
    const manifest = JSON.parse(
      readFileSync(resolve(WORLD_V2_DIR, 'world_v2_manifest.json'), 'utf8'),
    ) as WorldV2Manifest;
    const { index, decoded, actual } = loadActualEvidence();

    const result = validateWorldV2Manifest(manifest, actual);

    expect(result.errors).toEqual([]);
    expect(result.ok).toBe(true);
    expect(index.extent_m).toBe(81_920);
    expect(manifest.coordinateContract.extentM).toBe(81_920);
    expect(manifest.canonicalPoses.heightRatio).toBe(0.10);
    expect(manifest.canonicalPoses.views.lighting.feature).toContain('highest peak');
    expect(manifest.canonicalPoses.views.water.feature).toContain('waterfall');
    expect(manifest.canonicalPoses.views.navigation.feature).toContain('river');
    expect(manifest.canonicalPoses.views.spawn.feature).toContain('beach');
    expect(manifest.canonicalPoses.views.ecosystem.feature).toContain('forest');

    const canonH = 200 * manifest.canonicalPoses.heightRatio;
    for (const [id, pose] of Object.entries(manifest.canonicalPoses.views)) {
      if (id === 'overview') continue;
      const gx = Math.max(0, Math.min(decoded.width - 1, Math.round((pose.position[0] / 200 + 0.5) * (decoded.width - 1))));
      const gy = Math.max(0, Math.min(decoded.height - 1, Math.round((pose.position[2] / 200 + 0.5) * (decoded.height - 1))));
      const groundY = Math.max(decoded.elevation[gy * decoded.width + gx], decoded.seaLevelNorm) * canonH;
      expect(pose.position[1], `${id} camera must sit above local ground`).toBeGreaterThan(groundY);
    }

    expect(manifest.ownerGates.publicTileRelease).toBe('open');
    expect(manifest.ownerGates.irisXeBeforeAfterFps).toBe('open');
  });

  it('rejects a stale manifest whose sim_world_identity is the old C01 value (negative control)', () => {
    const manifest = JSON.parse(
      readFileSync(resolve(WORLD_V2_DIR, 'world_v2_manifest.json'), 'utf8'),
    ) as WorldV2Manifest;
    const { actual } = loadActualEvidence();

    const staleC01: WorldV2Manifest = {
      ...manifest,
      identities: {
        ...manifest.identities,
        simWorldIdentity: OLD_C01_SIM_WORLD_IDENTITY,
        bundleIdentity: OLD_C01_BUNDLE_IDENTITY,
      },
      coordinateContract: {
        ...manifest.coordinateContract,
        extentM: 40_960,
      },
    };

    const result = validateWorldV2Manifest(staleC01, actual);

    expect(result.ok).toBe(false);
    expect(result.errors.some((e) => e.includes('simWorldIdentity mismatch'))).toBe(true);
    expect(result.errors.some((e) => e.includes('bundleIdentity mismatch'))).toBe(true);
  });
});
