import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import {
  KTX2_HEADER_LEN,
  KTX2_LEVEL_LEN,
  KTX2_MAGIC,
  KTX2_SUPERCOMPRESSION_NONE,
  VK_FORMAT_BC5_UNORM_BLOCK,
  VK_FORMAT_R16G16_UNORM,
  VK_FORMAT_R16_UNORM,
  VK_FORMAT_R8G8B8A8_SRGB,
  VK_FORMAT_R8G8B8A8_UNORM,
  copyActiveTilePatch,
  decodeBc5Unorm,
  decodeKtx2Level0,
  decodeR16HeightM,
  decodeWaterR16G16,
} from '../../src/components/Landscape/worldV2Preview/ktx2Decoder';
import { computeSha256Hex, verifyTileBytes } from '../../src/components/Landscape/worldV2Preview/sha256';
import { loadWorldV2Preview } from '../../src/components/Landscape/worldV2Preview/tileLoader';
import {
  BANNER_TEXT,
  ELEV_MIN_M,
  TILE_ACTIVE_SIZE,
  TILE_TOTAL_SIZE,
  TileIntegrityError,
  WaterKind,
  type WorldV2IndexJson,
} from '../../src/components/Landscape/worldV2Preview/types';

function makeMinimalKtx2(
  vkFormat: number,
  typeSize: number,
  width: number,
  height: number,
  payload: Uint8Array,
): Uint8Array {
  const dataOffset = KTX2_HEADER_LEN + KTX2_LEVEL_LEN;
  const buf = new Uint8Array(dataOffset + payload.byteLength);
  buf.set(KTX2_MAGIC, 0);
  const view = new DataView(buf.buffer);
  view.setUint32(12, vkFormat, true);
  view.setUint32(16, typeSize, true);
  view.setUint32(20, width, true);
  view.setUint32(24, height, true);
  view.setUint32(40, 1, true); // levelCount
  view.setUint32(44, KTX2_SUPERCOMPRESSION_NONE, true);
  view.setUint32(KTX2_HEADER_LEN, dataOffset, true);
  view.setUint32(KTX2_HEADER_LEN + 8, payload.byteLength, true);
  view.setUint32(KTX2_HEADER_LEN + 16, payload.byteLength, true);
  buf.set(payload, dataOffset);
  return buf;
}

describe('WV2-6-PREVIEW tile verification and KTX2 decoding', () => {
  it('uses the required WV2-7-absent banner text', () => {
    expect(BANNER_TEXT).toBe('World v2 Preview — not the simulated world');
  });

  it('verifies tile SHA-256 via WebCrypto and rejects a 1-byte tamper loudly', async () => {
    const payload = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
    const expectedSha = await computeSha256Hex(payload);
    const entry = {
      path: 'tiles/height/L2/t_0_0.ktx2',
      byte_len: payload.byteLength,
      sha256: expectedSha,
    };

    await expect(verifyTileBytes(entry, payload)).resolves.toBe(expectedSha);

    // Negative control (a): flip 1 byte -> TileIntegrityError
    const tampered = new Uint8Array(payload);
    tampered[3] ^= 0x01;
    await expect(verifyTileBytes(entry, tampered)).rejects.toThrow(TileIntegrityError);
  });

  it('decodes KTX2 R16_UNORM height, R16G16_UNORM water, BC5 normal_xy, and strips 2-texel gutters', () => {
    // Height: 260x260 tile with interior active region set to max elevation (65535 -> +5700m)
    const hRaw = new Uint8Array(TILE_TOTAL_SIZE * TILE_TOTAL_SIZE * 2);
    const hView = new DataView(hRaw.buffer);
    for (let r = 0; r < TILE_TOTAL_SIZE; r += 1) {
      for (let c = 0; c < TILE_TOTAL_SIZE; c += 1) {
        const isGutter = r < 2 || r >= 258 || c < 2 || c >= 258;
        hView.setUint16((r * TILE_TOTAL_SIZE + c) * 2, isGutter ? 0 : 65535, true);
      }
    }
    const hKtx = makeMinimalKtx2(VK_FORMAT_R16_UNORM, 2, TILE_TOTAL_SIZE, TILE_TOTAL_SIZE, hRaw);
    const parsedH = decodeKtx2Level0(hKtx);
    const decodedH = decodeR16HeightM(parsedH.level0, parsedH.width, parsedH.height);
    const stitchedH = new Float32Array(TILE_ACTIVE_SIZE * TILE_ACTIVE_SIZE);
    copyActiveTilePatch(decodedH, stitchedH, 1, TILE_ACTIVE_SIZE, 0, 0);
    expect(stitchedH[0]).toBeCloseTo(5700, 1);
    expect(decodedH[0]).toBeCloseTo(ELEV_MIN_M, 1);

    // Water: R16G16_UNORM
    const wRaw = new Uint8Array(4 * 4 * 4);
    const wView = new DataView(wRaw.buffer);
    wView.setUint16(0, 13653, true); // ~0 m
    wView.setUint16(2, WaterKind.FreshwaterLake, true);
    const wKtx = makeMinimalKtx2(VK_FORMAT_R16G16_UNORM, 2, 4, 4, wRaw);
    const parsedW = decodeKtx2Level0(wKtx);
    const decodedW = decodeWaterR16G16(parsedW.level0, 4, 4);
    expect(decodedW.waterKind[0]).toBe(WaterKind.FreshwaterLake);
    expect(Math.abs(decodedW.waterHeightM[0])).toBeLessThan(1.0);

    // BC5: single 4x4 block with constant (r=128, g=200)
    const bc5Block = new Uint8Array(16);
    bc5Block[0] = 128;
    bc5Block[1] = 128;
    bc5Block[8] = 200;
    bc5Block[9] = 200;
    const nKtx = makeMinimalKtx2(VK_FORMAT_BC5_UNORM_BLOCK, 1, 4, 4, bc5Block);
    const parsedN = decodeKtx2Level0(nKtx);
    const decodedN = decodeBc5Unorm(parsedN.level0, 4, 4);
    expect(decodedN[0]).toBe(128);
    expect(decodedN[1]).toBe(200);
    expect(decodedN[30]).toBe(128);
    expect(decodedN[31]).toBe(200);
  });

  it('validates committed artifacts/world-v2/index.json L2 coverage and refuses corrupt tile without v1 fallback', async () => {
    const indexPath = path.resolve(__dirname, '../../artifacts/world-v2/index.json');
    const rawJson = fs.readFileSync(indexPath, 'utf8');
    const index = JSON.parse(rawJson) as WorldV2IndexJson;
    expect(index.sim_world_identity).toMatch(/^cbdd3ba9/);
    expect(index.extent_m).toBe(40960);
    expect(index.grid_size).toBe(4096);

    const l2Height = index.tiles.filter((t) => t.layer === 'height' && t.mip === 2);
    expect(l2Height).toHaveLength(16);

    // Synthetic minimal 256x256 (1-tile) manifest to test full loadWorldV2Preview refusal on tamper
    const zerosR16 = new Uint8Array(TILE_TOTAL_SIZE * TILE_TOTAL_SIZE * 2);
    const zerosRgba = new Uint8Array(TILE_TOTAL_SIZE * TILE_TOTAL_SIZE * 4);
    const zerosBc5 = new Uint8Array((TILE_TOTAL_SIZE / 4) * (TILE_TOTAL_SIZE / 4) * 16);

    const hTile = makeMinimalKtx2(VK_FORMAT_R16_UNORM, 2, TILE_TOTAL_SIZE, TILE_TOTAL_SIZE, zerosR16);
    const nTile = makeMinimalKtx2(VK_FORMAT_BC5_UNORM_BLOCK, 1, TILE_TOTAL_SIZE, TILE_TOTAL_SIZE, zerosBc5);
    const mTile = makeMinimalKtx2(VK_FORMAT_R8G8B8A8_SRGB, 1, TILE_TOTAL_SIZE, TILE_TOTAL_SIZE, zerosRgba);
    const uTile = makeMinimalKtx2(VK_FORMAT_R8G8B8A8_UNORM, 1, TILE_TOTAL_SIZE, TILE_TOTAL_SIZE, zerosRgba);

    const tileBlobs = new Map<string, Uint8Array>([
      ['tiles/height/L0/t_0_0.ktx2', hTile],
      ['tiles/normal_xy/L0/t_0_0.ktx2', nTile],
      ['tiles/macro_albedo/L0/t_0_0.ktx2', mTile],
      ['tiles/biome_ids/L0/t_0_0.ktx2', uTile],
      ['tiles/biome_weights/L0/t_0_0.ktx2', uTile],
      ['tiles/terrain_lighting/L0/t_0_0.ktx2', uTile],
    ]);

    const manifestTiles = [];
    for (const [rel, blob] of tileBlobs.entries()) {
      const layer = rel.split('/')[1];
      manifestTiles.push({
        layer,
        mip: 0,
        tx: 0,
        ty: 0,
        path: rel,
        byte_len: blob.byteLength,
        sha256: await computeSha256Hex(blob),
      });
    }
    const syntheticIndex: WorldV2IndexJson = {
      ...index,
      grid_size: 256,
      tiles: manifestTiles,
    };

    const mockFetch = (async (input: string | URL | Request): Promise<Response> => {
      const url = String(input);
      if (url.endsWith('index.json')) {
        return new Response(JSON.stringify(syntheticIndex), { status: 200 });
      }
      const match = Array.from(tileBlobs.entries()).find(([rel]) => url.endsWith(rel));
      if (!match) return new Response(null, { status: 404 });
      return new Response(new Uint8Array(match[1]), { status: 200 });
    }) as typeof fetch;

    const okGrid = await loadWorldV2Preview({
      indexUrl: '/artifacts/world-v2/index.json',
      tilesBaseUrl: '/world-v2-tiles-local',
      mip: 0,
      fetcher: mockFetch,
    });
    expect(okGrid.size).toBe(256);
    expect(okGrid.tileCountVerified).toBe(6);

    // Corrupt 1 byte of height tile -> must reject loudly with TileIntegrityError
    const badH = new Uint8Array(hTile);
    badH[badH.byteLength - 1] ^= 0xff;
    tileBlobs.set('tiles/height/L0/t_0_0.ktx2', badH);
    await expect(
      loadWorldV2Preview({
        indexUrl: '/artifacts/world-v2/index.json',
        tilesBaseUrl: '/world-v2-tiles-local',
        mip: 0,
        fetcher: mockFetch,
      }),
    ).rejects.toThrow(TileIntegrityError);
  });
});
