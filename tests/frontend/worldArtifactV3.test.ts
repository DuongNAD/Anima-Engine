import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { deflateSync } from 'node:zlib';
import { describe, expect, it } from 'vitest';
import {
  ANMF_FLAG_DECORATIVE,
  decodeAnmwV3Sim,
  decodeCaveNetworkBin,
  decodeFloraSectorAnmf,
  deriveSimSeedU32,
  detectWebGl2TileCapabilities,
  encodeAnmwV3Sim,
  encodeCaveNetworkBin,
  encodeFloraSectorAnmf,
  expandNormalRgToRgba8,
  packR16ToRg8Unorm,
  peekAnmwV3Seed,
  unpackRg8HeightM,
} from '../../src/components/Landscape/utils/worldArtifactV3';
import {
  KTX2_HEADER_LEN,
  KTX2_LEVEL_LEN,
  KTX2_MAGIC,
  KTX2_SUPERCOMPRESSION_NONE,
  KTX2_SUPERCOMPRESSION_ZLIB,
  VK_FORMAT_BC5_UNORM_BLOCK,
  VK_FORMAT_R16G16_UNORM,
  copyActiveTilePatch,
  decodeBc5Unorm,
  decodeKtx2Level0,
  decodeR16HeightM,
  decodeWaterR16G16,
} from '../../src/components/Landscape/worldV2/ktx2Decoder';
import { computeSha256Hex, verifyTileBytes } from '../../src/components/Landscape/worldV2/sha256';
import { TILE_ACTIVE_SIZE, TILE_TOTAL_SIZE, TileIntegrityError } from '../../src/components/Landscape/worldV2/types';

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(HERE, '../..');

function buildTestKtx2(
  vkFormat: number,
  width: number,
  height: number,
  payload: Uint8Array,
  compressed = false,
): Uint8Array {
  const levelData = compressed ? new Uint8Array(deflateSync(payload)) : payload;
  const headerLen = KTX2_HEADER_LEN + KTX2_LEVEL_LEN;
  const out = new Uint8Array(headerLen + levelData.byteLength);
  out.set(KTX2_MAGIC, 0);
  const dv = new DataView(out.buffer);
  dv.setUint32(12, vkFormat, true);
  dv.setUint32(16, 1, true);
  dv.setUint32(20, width, true);
  dv.setUint32(24, height, true);
  dv.setUint32(40, 1, true);
  dv.setUint32(44, compressed ? KTX2_SUPERCOMPRESSION_ZLIB : KTX2_SUPERCOMPRESSION_NONE, true);
  dv.setUint32(KTX2_HEADER_LEN, headerLen, true);
  dv.setUint32(KTX2_HEADER_LEN + 8, levelData.byteLength, true);
  dv.setUint32(KTX2_HEADER_LEN + 16, payload.byteLength, true);
  out.set(levelData, headerLen);
  return out;
}

describe('World v2 codecs, fallbacks, and integrity (WV2-6)', () => {
  it('decodes committed sim_256.anmw3 cleanly and round-trips byte-identically', () => {
    const simPath = resolve(REPO_ROOT, 'artifacts/world-v2/sim_256.anmw3');
    const raw = new Uint8Array(readFileSync(simPath));
    const sim = decodeAnmwV3Sim(raw);

    expect(sim.width).toBe(256);
    expect(sim.height).toBe(256);
    expect(sim.generatorVersion).toBe(3);
    expect(sim.worldScale).toBe(200);
    expect(sim.caveEntrances.length).toBe(40);
    expect(peekAnmwV3Seed(raw)).toBe(deriveSimSeedU32(sim.seedU64));

    const reencoded = encodeAnmwV3Sim(sim);
    expect(reencoded).toEqual(raw);
  });

  it('round-trips ANMC cave network and ANMF flora sectors and rejects missing decorative bit', () => {
    const netBytes = encodeCaveNetworkBin({
      flags: 0,
      nodes: [
        { x_m: 1000, y_m: 420, z_m: 2000, radius_m: 3.5, light_decay: 0.7 },
        { x_m: 1040, y_m: 410, z_m: 2020, radius_m: 2.8, light_decay: 0.9 },
      ],
      edges: [{ from_idx: 0, to_idx: 1, passage_kind: 1 }],
      entrances: [
        {
          node_idx: 0,
          mouth_kind: 0,
          realm_status: 1,
          yaw_rad: 1.25,
          elevation_m: 420,
          clearance_m: 4.0,
        },
      ],
    });
    const decodedNet = decodeCaveNetworkBin(netBytes);
    expect(decodedNet.nodes).toHaveLength(2);
    expect(decodedNet.edges).toEqual([{ from_idx: 0, to_idx: 1, passage_kind: 1 }]);
    expect(decodedNet.entrances[0].clearance_m).toBe(4.0);

    const sectorBytes = encodeFloraSectorAnmf({
      sectorX: 3,
      sectorZ: 7,
      decorativeOnly: true,
      instances: [
        {
          x_local_u16: 12345,
          z_local_u16: 54321,
          species_id: 2,
          scale_u8: 180,
          yaw_u8: 64,
          flags_u8: ANMF_FLAG_DECORATIVE,
        },
      ],
    });
    const decodedSector = decodeFloraSectorAnmf(sectorBytes);
    expect(decodedSector.sectorX).toBe(3);
    expect(decodedSector.sectorZ).toBe(7);
    expect(decodedSector.instances).toHaveLength(1);

    const tampered = new Uint8Array(sectorBytes);
    tampered[tampered.byteLength - 1] = 0; // clear FLORA_FLAG_DECORATIVE
    expect(() => decodeFloraSectorAnmf(tampered)).toThrow(/FLORA_FLAG_DECORATIVE/);
  });

  it('implements WebGL2 capability detection, lossless RG8 height fallback, and BC5 RGBA8 fallback', () => {
    const fullCaps = detectWebGl2TileCapabilities({
      getExtension: ((ext: string) =>
        ext === 'EXT_texture_norm16' || ext === 'EXT_texture_compression_rgtc'
          ? {}
          : null) as unknown as WebGL2RenderingContext['getExtension'],
    });
    expect(fullCaps.heightEncoding).toBe('r16');
    expect(fullCaps.normalEncoding).toBe('bc5');

    const fallbackCaps = detectWebGl2TileCapabilities({
      getExtension: (() => null) as unknown as WebGL2RenderingContext['getExtension'],
    });
    expect(fallbackCaps.heightEncoding).toBe('rg8');
    expect(fallbackCaps.normalEncoding).toBe('rgba8');

    // 0-ULP round-trip on R16 -> RG8_UNORM height packing
    const rawR16 = new Uint8Array(8);
    const dv = new DataView(rawR16.buffer);
    [0, 13653, 32768, 65535].forEach((q, idx) => dv.setUint16(idx * 2, q, true));
    const rg8 = packR16ToRg8Unorm(rawR16, 2, 2);
    const directM = decodeR16HeightM(rawR16, 2, 2);
    for (let i = 0; i < 4; i += 1) {
      expect(Math.abs(unpackRg8HeightM(rg8, i) - directM[i])).toBeLessThan(1e-6);
    }

    const normalRg = new Uint8Array([128, 128, 255, 128]);
    const rgba8 = expandNormalRgToRgba8(normalRg, 2);
    expect(Array.from(rgba8.subarray(0, 4))).toEqual([128, 255, 128, 255]);
    expect(rgba8[4]).toBe(255);
  });

  it('decodes NONE & ZLIB KTX2 images, BC5 blocks, water R16G16, and trims gutter patches', () => {
    const rawWater = new Uint8Array(16);
    const dv = new DataView(rawWater.buffer);
    dv.setUint16(0, 13653, true); // ~0 m
    dv.setUint16(2, 2, true); // FreshwaterLake
    const zKtx = buildTestKtx2(VK_FORMAT_R16G16_UNORM, 2, 2, rawWater, true);
    const decodedZ = decodeKtx2Level0(zKtx);
    expect(decodedZ.vkFormat).toBe(VK_FORMAT_R16G16_UNORM);
    const water = decodeWaterR16G16(decodedZ.level0, 2, 2);
    expect(water.waterKind[0]).toBe(2);

    const bc5Block = new Uint8Array(16);
    bc5Block[0] = 200;
    bc5Block[1] = 200;
    bc5Block[8] = 60;
    bc5Block[9] = 60;
    const bc5Ktx = buildTestKtx2(VK_FORMAT_BC5_UNORM_BLOCK, 4, 4, bc5Block, false);
    const decodedBc5 = decodeBc5Unorm(decodeKtx2Level0(bc5Ktx).level0, 4, 4);
    expect(decodedBc5[0]).toBe(200);
    expect(decodedBc5[1]).toBe(60);

    const patch = new Float32Array(TILE_TOTAL_SIZE * TILE_TOTAL_SIZE);
    patch.fill(42.5);
    const dst = new Float32Array(TILE_ACTIVE_SIZE * TILE_ACTIVE_SIZE);
    copyActiveTilePatch(patch, dst, 1, TILE_ACTIVE_SIZE, 0, 0);
    expect(dst[0]).toBe(42.5);
    expect(dst[dst.length - 1]).toBe(42.5);
  });

  it('verifies tile SHA-256 and throws TileIntegrityError on 1-byte tamper (negative control)', async () => {
    const payload = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
    const sha256 = await computeSha256Hex(payload);
    const entry = { path: 'tiles/height/L2/t_0_0.ktx2', byte_len: payload.byteLength, sha256 };

    await expect(verifyTileBytes(entry, payload)).resolves.toBe(sha256);

    const tampered = new Uint8Array(payload);
    tampered[3] ^= 0x01; // 1-byte tamper
    await expect(verifyTileBytes(entry, tampered)).rejects.toBeInstanceOf(TileIntegrityError);
  });

  it('verifies prefetch helper invariants and fails loudly on tampered tile staging', () => {
    const scriptUrl = pathToFileURL(resolve(REPO_ROOT, 'scripts/fetch_world_v2_tiles.mjs')).href;
    const tmpArchive = mkdtempSync(join(tmpdir(), 'wv2-tamper-'));
    try {
      const sampleFile = join(tmpArchive, 'sample.tar.gz');
      const stagedDir = join(tmpArchive, 'staged');
      const data = Buffer.from('anima-world-v2-sample');
      writeFileSync(sampleFile, data);
      const digest = createHash('sha256').update(data).digest('hex');

      const out = execFileSync(
        process.execPath,
        [
          '--input-type=module',
          '-e',
          `
          import assert from 'node:assert/strict';
          import { createHash } from 'node:crypto';
          import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
          import { join } from 'node:path';
          const mod = await import(${JSON.stringify(scriptUrl)});
          const { index, indexBytes, simBytes } = mod.verifyIndexAndSim256(${JSON.stringify(REPO_ROOT)});
          assert.ok(index.tiles.length > 3000);
          assert.ok(indexBytes.byteLength > 1000);
          assert.equal(simBytes.byteLength, index.sim_256_byte_len);
          assert.equal(
            mod.resolveArchiveUrl(index.release_archive),
            mod.WORLD_V2_RELEASE_BASE_URL + '/' + index.release_archive,
          );
          const samplePath = ${JSON.stringify(sampleFile)};
          const expectedDigest = ${JSON.stringify(digest)};
          assert.equal(mod.verifyArchiveSha256(samplePath, expectedDigest).sha256, expectedDigest);
          const mutated = Buffer.from(readFileSync(samplePath));
          mutated[0] ^= 0xff;
          writeFileSync(samplePath, mutated);
          assert.throws(() => mod.verifyArchiveSha256(samplePath, expectedDigest), /SHA-256 mismatch/);

          const staged = ${JSON.stringify(stagedDir)};
          mkdirSync(staged, { recursive: true });
          const blob = Buffer.from([1, 2, 3, 4]);
          const blobSha = createHash('sha256').update(blob).digest('hex');
          const fakeIdx = { tiles: [{ path: 'blob.bin', byte_len: 4, sha256: blobSha }] };
          writeFileSync(join(staged, 'blob.bin'), blob);
          assert.equal(mod.verifyStagedTiles(fakeIdx, staged), 1);
          writeFileSync(join(staged, 'blob.bin'), Buffer.from([1, 2, 9, 4]));
          assert.throws(() => mod.verifyStagedTiles(fakeIdx, staged), /SHA-256 mismatch/);
          console.log('PREFETCH_HELPERS_OK');
          `,
        ],
        { encoding: 'utf8' },
      );
      expect(out).toContain('PREFETCH_HELPERS_OK');
    } finally {
      rmSync(tmpArchive, { recursive: true, force: true });
    }

    const stagedDir = resolve(REPO_ROOT, 'public/world-v2');
    if (existsSync(join(stagedDir, 'cave_network.bin'))) {
      const realCave = new Uint8Array(readFileSync(join(stagedDir, 'cave_network.bin')));
      const parsedCave = decodeCaveNetworkBin(realCave);
      expect(parsedCave.entrances).toHaveLength(40);
    }
  });
});
