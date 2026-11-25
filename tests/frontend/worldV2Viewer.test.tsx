import { createHash } from 'node:crypto';
import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { WorldShowcase } from '../../src/components/Landscape/WorldShowcase';
import { resolveWorldSourceMode } from '../../src/components/Landscape/worldV2/tileLoader';
import * as worldCache from '../../src/components/Landscape/utils/worldCache';
import * as worldGen from '../../src/components/Landscape/utils/worldGen';
import * as worldTerrainTextures from '../../src/components/Landscape/utils/worldTerrainTextures';
import {
  ANMF_FLAG_DECORATIVE,
  encodeAnmwV3Sim,
  encodeCaveNetworkBin,
  encodeFloraSectorAnmf,
  type AnmwV3Sim,
} from '../../src/components/Landscape/utils/worldArtifactV3';
import {
  KTX2_HEADER_LEN,
  KTX2_LEVEL_LEN,
  KTX2_MAGIC,
  KTX2_SUPERCOMPRESSION_NONE,
  VK_FORMAT_BC5_UNORM_BLOCK,
  VK_FORMAT_R16_UNORM,
  VK_FORMAT_R8G8B8A8_SRGB,
  VK_FORMAT_R8G8B8A8_UNORM,
  VK_FORMAT_R8G8_UNORM,
} from '../../src/components/Landscape/worldV2/ktx2Decoder';
import { TILE_TOTAL_SIZE } from '../../src/components/Landscape/worldV2/types';

vi.setConfig({ testTimeout: 30_000 });

function sha256(bytes: Uint8Array): string {
  return createHash('sha256').update(bytes).digest('hex');
}

function makeSyntheticKtx2(vkFormat: number, payload: Uint8Array): Uint8Array {
  const headerLen = KTX2_HEADER_LEN + KTX2_LEVEL_LEN;
  const out = new Uint8Array(headerLen + payload.byteLength);
  out.set(KTX2_MAGIC, 0);
  const dv = new DataView(out.buffer);
  dv.setUint32(12, vkFormat, true);
  dv.setUint32(16, 1, true);
  dv.setUint32(20, TILE_TOTAL_SIZE, true);
  dv.setUint32(24, TILE_TOTAL_SIZE, true);
  dv.setUint32(40, 1, true);
  dv.setUint32(44, KTX2_SUPERCOMPRESSION_NONE, true);
  dv.setUint32(KTX2_HEADER_LEN, headerLen, true);
  dv.setUint32(KTX2_HEADER_LEN + 8, payload.byteLength, true);
  dv.setUint32(KTX2_HEADER_LEN + 16, payload.byteLength, true);
  out.set(payload, headerLen);
  return out;
}

function buildSyntheticBundleMap(): Map<string, Uint8Array> {
  const n256 = 256 * 256;
  const elev = new Float32Array(n256).fill(0.45);
  const simObj: AnmwV3Sim = {
    width: 256,
    height: 256,
    seaLevelNorm: 1500 / 7200,
    simSeedU32: 42,
    seedU64: 42n,
    generatorVersion: 3,
    worldScale: 200,
    flags: 0,
    elevation: elev,
    moisture: new Float32Array(n256).fill(0.5),
    temperature: new Float32Array(n256).fill(0.55),
    flow: new Float32Array(n256),
    slope: new Float32Array(n256).fill(0.05),
    biome22: new Uint8Array(n256).fill(4),
    waterKind: new Uint8Array(n256),
    waterfalls: [],
    caveEntrances: [
      {
        x_m: 12000,
        z_m: 14000,
        elevation_m: 420,
        yaw_rad: 0.5,
        clearance_m: 4.0,
        mouth_kind: 0,
        realm_status: 1,
      },
    ],
  };
  const simBytes = encodeAnmwV3Sim(simObj);

  const cells = TILE_TOTAL_SIZE * TILE_TOTAL_SIZE;
  const r16Payload = new Uint8Array(cells * 2);
  const r16Dv = new DataView(r16Payload.buffer);
  for (let i = 0; i < cells; i += 1) r16Dv.setUint16(i * 2, 22000, true); // well above sea level

  const bc5Payload = new Uint8Array((TILE_TOTAL_SIZE / 4) * (TILE_TOTAL_SIZE / 4) * 16);
  for (let b = 0; b < bc5Payload.length; b += 16) {
    bc5Payload[b] = 128;
    bc5Payload[b + 1] = 128;
    bc5Payload[b + 8] = 128;
    bc5Payload[b + 9] = 128;
  }

  const rgbaPayload = new Uint8Array(cells * 4);
  for (let i = 0; i < cells; i += 1) {
    rgbaPayload[i * 4] = 92;
    rgbaPayload[i * 4 + 1] = 134;
    rgbaPayload[i * 4 + 2] = 68;
    rgbaPayload[i * 4 + 3] = 255;
  }
  const biomeIdsPayload = new Uint8Array(cells * 4);
  for (let i = 0; i < cells; i += 1) biomeIdsPayload[i * 4] = 4; // Grassland
  const lightingPayload = new Uint8Array(cells * 4).fill(240);
  const horizonPayload = new Uint8Array(cells * 4).fill(20);
  const hydroPayload = new Uint8Array(cells * 4);
  const shorePayload = new Uint8Array(cells * 2).fill(180);

  const layerBuffers: Array<[string, number, string, Uint8Array]> = [
    ['height', 2, 'tiles/height/L2/t_0_0.ktx2', makeSyntheticKtx2(VK_FORMAT_R16_UNORM, r16Payload)],
    ['normal_xy', 2, 'tiles/normal_xy/L2/t_0_0.ktx2', makeSyntheticKtx2(VK_FORMAT_BC5_UNORM_BLOCK, bc5Payload)],
    ['macro_albedo', 2, 'tiles/macro_albedo/L2/t_0_0.ktx2', makeSyntheticKtx2(VK_FORMAT_R8G8B8A8_SRGB, rgbaPayload)],
    ['biome_ids', 2, 'tiles/biome_ids/L2/t_0_0.ktx2', makeSyntheticKtx2(VK_FORMAT_R8G8B8A8_UNORM, biomeIdsPayload)],
    ['biome_weights', 2, 'tiles/biome_weights/L2/t_0_0.ktx2', makeSyntheticKtx2(VK_FORMAT_R8G8B8A8_UNORM, lightingPayload)],
    ['terrain_lighting', 2, 'tiles/terrain_lighting/L2/t_0_0.ktx2', makeSyntheticKtx2(VK_FORMAT_R8G8B8A8_UNORM, lightingPayload)],
    ['terrain_horizon', 2, 'tiles/terrain_horizon/L2/t_0_0.ktx2', makeSyntheticKtx2(VK_FORMAT_R8G8B8A8_UNORM, horizonPayload)],
    ['hydro_flow', 2, 'tiles/hydro_flow/L2/t_0_0.ktx2', makeSyntheticKtx2(VK_FORMAT_R8G8B8A8_UNORM, hydroPayload)],
    ['shore_sdf', 2, 'tiles/shore_sdf/L2/t_0_0.ktx2', makeSyntheticKtx2(VK_FORMAT_R8G8_UNORM, shorePayload)],
  ];

  const caveBin = encodeCaveNetworkBin({
    flags: 0,
    nodes: [
      { x_m: 12000, y_m: 420, z_m: 14000, radius_m: 3.0, light_decay: 0.8 },
      { x_m: 12080, y_m: 410, z_m: 14040, radius_m: 2.5, light_decay: 0.9 },
    ],
    edges: [{ from_idx: 0, to_idx: 1, passage_kind: 0 }],
    entrances: [
      {
        node_idx: 0,
        mouth_kind: 0,
        realm_status: 1,
        yaw_rad: 0.5,
        elevation_m: 420,
        clearance_m: 4.0,
      },
    ],
  });

  const floraBin = encodeFloraSectorAnmf({
    sectorX: 8,
    sectorZ: 8,
    decorativeOnly: true,
    instances: [
      {
        x_local_u16: 32768,
        z_local_u16: 32768,
        species_id: 1,
        scale_u8: 150,
        yaw_u8: 40,
        flags_u8: ANMF_FLAG_DECORATIVE,
      },
    ],
  });

  const files = new Map<string, Uint8Array>();
  files.set('/world-v2/sim_256.anmw3', simBytes);
  const tilesMeta: Array<Record<string, unknown>> = [];

  for (const [layer, mip, path, bytes] of layerBuffers) {
    files.set(`/world-v2/${path}`, bytes);
    tilesMeta.push({ layer, mip, tx: 0, ty: 0, path, byte_len: bytes.byteLength, sha256: sha256(bytes) });
  }
  files.set('/world-v2/cave_network.bin', caveBin);
  tilesMeta.push({
    layer: 'cave_network',
    mip: 0,
    tx: 0,
    ty: 0,
    path: 'cave_network.bin',
    byte_len: caveBin.byteLength,
    sha256: sha256(caveBin),
  });
  files.set('/world-v2/flora/s_8_8.anmf', floraBin);
  tilesMeta.push({
    layer: 'flora_tiles',
    mip: 0,
    tx: 8,
    ty: 8,
    path: 'flora/s_8_8.anmf',
    byte_len: floraBin.byteLength,
    sha256: sha256(floraBin),
  });

  const indexJson = {
    extent_m: 40960,
    format_version: 1,
    generator_version: 3,
    grid_size: 1024,
    release_archive: 'world-v2/test.tar.gz',
    release_archive_sha256: 'a'.repeat(64),
    schema_tag: 'anima-world-v2-c01-v1',
    seed: 42,
    sim_256_byte_len: simBytes.byteLength,
    sim_256_file: 'sim_256.anmw3',
    sim_world_identity: sha256(simBytes),
    tiles: tilesMeta,
  };
  files.set('/world-v2/index.json', new TextEncoder().encode(JSON.stringify(indexJson)));
  return files;
}

describe('WorldShowcase World v2 integration (WV2-6)', () => {
  beforeEach(() => {
    worldCache.clearWorldV2Cache();
    window.history.replaceState({}, '', '?world=v2&wv2Mip=2');
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    window.history.replaceState({}, '', '/');
  });

  it('defaults to v2 outside tests and switches cleanly for test/capture/v1 overrides', () => {
    expect(resolveWorldSourceMode('', 'production')).toBe('v2');
    expect(resolveWorldSourceMode('', 'development')).toBe('v2');
    expect(resolveWorldSourceMode('?world=v1', 'production')).toBe('v1');
    expect(resolveWorldSourceMode('?capture=1&view=overview', 'production')).toBe('v1');
    expect(resolveWorldSourceMode('', 'test')).toBe('v1');
    expect(resolveWorldSourceMode('?world=v2', 'test')).toBe('v2');
  });

  it('mounts verified World v2 bundle with 0 Worker, 0 generateWorld, 0 computeHorizonAoMap, and §6 decorative toggle', async () => {
    const files = buildSyntheticBundleMap();
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = typeof input === 'string' ? input : input instanceof URL ? input.pathname : input.url;
      const hit = files.get(url);
      if (!hit) return new Response(null, { status: 404 });
      return new Response(new Uint8Array(hit), { status: 200 });
    });

    const genSpy = vi.spyOn(worldGen, 'generateWorld');
    const v1LoadSpy = vi.spyOn(worldCache, 'loadOrGenerateWorld');
    const horizonAoSpy = vi.spyOn(worldTerrainTextures, 'computeHorizonAoMap');
    const workerSpy = vi.fn();
    vi.stubGlobal('Worker', workerSpy as unknown as typeof Worker);

    const { container } = render(React.createElement(WorldShowcase));

    await waitFor(() => {
      expect(screen.getByTestId('world-v2-identity')).toBeTruthy();
    });

    expect(screen.getByTestId('world-v2-identity').textContent).toContain('World v2 C01');
    expect(fetchSpy).toHaveBeenCalled();
    expect(workerSpy).not.toHaveBeenCalled();
    expect(genSpy).not.toHaveBeenCalled();
    expect(v1LoadSpy).not.toHaveBeenCalled();
    expect(horizonAoSpy).not.toHaveBeenCalled();

    // Decorative groups visible by default (`showDecoration = true`), hidden when toggled off.
    expect(container.querySelector('[name="world-v2-flora"]')).toBeTruthy();
    expect(container.querySelector('[name="world-caves-group"]')).toBeTruthy();

    fireEvent.click(screen.getByTestId('toggle-decoration-btn'));
    await waitFor(() => {
      expect(container.querySelector('[name="world-v2-flora"]')).toBeNull();
      expect(container.querySelector('[name="world-caves-group"]')).toBeNull();
    });
  });

  it('fails loudly via WorldV2ErrorPanel on 1-byte tampered tile with zero v1 fallback (negative control)', async () => {
    const files = buildSyntheticBundleMap();
    const hBytes = files.get('/world-v2/tiles/height/L2/t_0_0.ktx2')!;
    const tampered = new Uint8Array(hBytes);
    tampered[tampered.byteLength - 1] ^= 0x01;
    files.set('/world-v2/tiles/height/L2/t_0_0.ktx2', tampered);

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = typeof input === 'string' ? input : input instanceof URL ? input.pathname : input.url;
      const hit = files.get(url);
      if (!hit) return new Response(null, { status: 404 });
      return new Response(new Uint8Array(hit), { status: 200 });
    });

    const genSpy = vi.spyOn(worldGen, 'generateWorld');
    const v1LoadSpy = vi.spyOn(worldCache, 'loadOrGenerateWorld');

    render(React.createElement(WorldShowcase));

    await waitFor(() => {
      expect(screen.getByTestId('world-v2-load-error')).toBeTruthy();
    });

    expect(screen.getByTestId('world-v2-load-error').textContent).toContain('SHA-256 mismatch');
    expect(genSpy).not.toHaveBeenCalled();
    expect(v1LoadSpy).not.toHaveBeenCalled();
  });
});
