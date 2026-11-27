import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { WorldShowcase } from '../../src/components/Landscape/WorldShowcase';
import { clearWorldV2Cache, resolveWorldSourceMode } from '../../src/components/Landscape/worldV2/tileLoader';
import * as worldCache from '../../src/components/Landscape/utils/worldCache';
import * as worldGen from '../../src/components/Landscape/utils/worldGen';
import * as worldTerrainTextures from '../../src/components/Landscape/utils/worldTerrainTextures';
import * as diagnosticsWindow from '../../src/components/Landscape/utils/diagnosticsWindow';
import * as THREE from 'three';
import WorldV2Flora from '../../src/components/Landscape/worldV2/WorldV2Flora';
import {
  buildWorldFromV2Bundle,
  WV2_SPECIES_TO_FLORA,
} from '../../src/components/Landscape/worldV2/worldV2Adapter';
import { decodeFloraSectorAnmf } from '../../src/components/Landscape/utils/worldArtifactV3';
import type { WorldV2LoadedBundle } from '../../src/components/Landscape/worldV2/types';
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
import {
  floraSectorsPerSideForGrid,
  resolveWorldHeightRatio,
  resolveWorldV2FloraStride,
  TILE_TOTAL_SIZE,
  WORLD_V2_FLORA_MAX_INSTANCES,
} from '../../src/components/Landscape/worldV2/types';

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

function buildSyntheticBundleMap(opts: { extentM?: number; gridSize?: number; schemaTag?: string } = {}): Map<string, Uint8Array> {
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
    extent_m: opts.extentM ?? 40960,
    format_version: 1,
    generator_version: 3,
    grid_size: opts.gridSize ?? 1024,
    release_archive: 'world-v2/test.tar.gz',
    release_archive_sha256: 'a'.repeat(64),
    schema_tag: opts.schemaTag ?? 'ANMW3-BIG-v1',
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
    clearWorldV2Cache();
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
    const committedIndex = JSON.parse(
      readFileSync(resolve(__dirname, '../../artifacts/world-v2/index.json'), 'utf8'),
    ) as { schema_tag: string; extent_m: number };
    const files = buildSyntheticBundleMap({
      schemaTag: committedIndex.schema_tag,
      extentM: committedIndex.extent_m,
    });
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

    const identityText = screen.getByTestId('world-v2-identity').textContent ?? '';
    expect(identityText).toContain(`World v2 ${committedIndex.schema_tag}`);
    expect(identityText).not.toContain('World v2 C01');
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

  it('installs RW-C4 frame counters in v2 mode and keeps them active across decoration toggle', async () => {
    const files = buildSyntheticBundleMap();
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = typeof input === 'string' ? input : input instanceof URL ? input.pathname : input.url;
      const hit = files.get(url);
      if (!hit) return new Response(null, { status: 404 });
      return new Response(new Uint8Array(hit), { status: 200 });
    });
    const handles: Array<{ applyPinnedPose: ReturnType<typeof vi.fn>; dispose: ReturnType<typeof vi.fn> }> = [];
    const countersSpy = vi.spyOn(diagnosticsWindow, 'installFrameCounters').mockImplementation(() => {
      const h = { applyPinnedPose: vi.fn(), dispose: vi.fn() };
      handles.push(h);
      return h;
    });

    render(React.createElement(WorldShowcase));

    await waitFor(() => {
      expect(screen.getByTestId('world-v2-identity')).toBeTruthy();
    });

    expect(countersSpy).toHaveBeenCalled();
    const lastIdx = countersSpy.mock.calls.length - 1;
    expect(countersSpy.mock.calls[lastIdx][2]).toEqual({ renderSize: 1200, heightRatio: 0.14 });
    expect(handles.filter((h) => h.dispose.mock.calls.length === 0).length).toBe(1);

    fireEvent.click(screen.getByTestId('toggle-decoration-btn'));
    expect(handles.filter((h) => h.dispose.mock.calls.length === 0).length).toBe(1);
  });

  it('tags WorldV2Flora as rwLayer="flora" and honours ?floraDensityScale at low tier', () => {
    const floraInstances = Array.from({ length: 16 }, (_, idx) => ({
      x_local_u16: 4096 + idx * 1024,
      z_local_u16: 4096 + idx * 1024,
      species_id: 1,
      scale_u8: 150,
      yaw_u8: 40,
      flags_u8: ANMF_FLAG_DECORATIVE,
    }));
    const floraBin = encodeFloraSectorAnmf({
      sectorX: 8,
      sectorZ: 8,
      decorativeOnly: true,
      instances: floraInstances,
    });
    const sector = decodeFloraSectorAnmf(floraBin);
    const n256 = 256 * 256;
    const sim: AnmwV3Sim = {
      width: 256,
      height: 256,
      seaLevelNorm: 1500 / 7200,
      simSeedU32: 42,
      seedU64: 42n,
      generatorVersion: 3,
      worldScale: 200,
      flags: 0,
      elevation: new Float32Array(n256).fill(0.45),
      moisture: new Float32Array(n256).fill(0.5),
      temperature: new Float32Array(n256).fill(0.55),
      flow: new Float32Array(n256),
      slope: new Float32Array(n256).fill(0.05),
      biome22: new Uint8Array(n256).fill(4),
      waterKind: new Uint8Array(n256),
      waterfalls: [],
      caveEntrances: [],
    };
    const cells = TILE_TOTAL_SIZE * TILE_TOTAL_SIZE;
    const bundle: WorldV2LoadedBundle = {
      simWorldIdentity: 'a'.repeat(64),
      bundleIdentity: 'b'.repeat(64),
      schemaTag: 'anima-world-v2-c01-v1',
      seed: 42,
      extentM: 40960,
      gridSize: 4096,
      floraSectorsPerSide: 16,
      mip: 2,
      size: 1024,
      macroAlbedoSize: TILE_TOTAL_SIZE,
      tileCountVerified: 1,
      sim,
      heightM: new Float32Array(cells).fill(420),
      normalRg: new Uint8Array(cells * 2).fill(128),
      macroAlbedoRgba: new Uint8Array(cells * 4).fill(128),
      biomeIdsRgba: new Uint8Array(cells * 4),
      biomeWeightsRgba: new Uint8Array(cells * 4),
      lightingRgba: new Uint8Array(cells * 4),
      horizonRgba: new Uint8Array(cells * 4),
      hydroFlowRgba: new Uint8Array(cells * 4),
      shoreSdfRg: new Uint8Array(cells),
      waterHeightM: new Float32Array(cells),
      waterKind: new Uint8Array(cells),
      caveMask: new Uint8Array(cells),
      caveNetwork: null,
      floraSectors: [sector],
    };
    const world = buildWorldFromV2Bundle(bundle);

    window.history.replaceState({}, '', '?world=v2&quality=low');
    const baseRender = render(React.createElement(WorldV2Flora, { bundle, world, tier: 'low' }));
    const groupEl = baseRender.container.querySelector('[name="world-v2-flora"]');
    expect(groupEl).toBeTruthy();
    const baseMesh = baseRender.container.querySelector('instancedmesh');
    const baseArgs = baseMesh?.getAttribute('args') ?? '';
    expect(baseArgs.endsWith(',2')).toBe(true); // 16 instances / stride 8 = 2

    // Verify diagnostics classifier sees userData.rwLayer = 'flora' on a child mesh inside the group.
    const grp = new THREE.Group();
    grp.name = 'world-v2-flora';
    grp.userData = { decorative: true, rwLayer: 'flora' };
    const child = new THREE.Mesh();
    child.name = 'world-v2-flora-s1';
    grp.add(child);
    expect(diagnosticsWindow.classifySceneObjectLayer(child)).toBe('flora');
    baseRender.unmount();

    // Verify tier stride ordering on the 16-instance fixture: high (8) > medium (6) > low (2).
    window.history.replaceState({}, '', '?world=v2&quality=medium');
    const medRender = render(React.createElement(WorldV2Flora, { bundle, world, tier: 'medium' }));
    const medArgs = medRender.container.querySelector('instancedmesh')?.getAttribute('args') ?? '';
    expect(medArgs.endsWith(',6')).toBe(true); // ceil(16 / 3) = 6
    medRender.unmount();

    window.history.replaceState({}, '', '?world=v2&quality=high');
    const highRender = render(React.createElement(WorldV2Flora, { bundle, world, tier: 'high' }));
    const highArgs = highRender.container.querySelector('instancedmesh')?.getAttribute('args') ?? '';
    expect(highArgs.endsWith(',8')).toBe(true); // 16 / 2 = 8
    highRender.unmount();

    // Negative-control multiplier (?floraDensityScale=4) scales emitted instances 4x at low tier.
    window.history.replaceState({}, '', '?world=v2&quality=low&floraDensityScale=4');
    const scaledRender = render(React.createElement(WorldV2Flora, { bundle, world, tier: 'low' }));
    const scaledMesh = scaledRender.container.querySelector('instancedmesh');
    const scaledArgs = scaledMesh?.getAttribute('args') ?? '';
    expect(scaledArgs.endsWith(',8')).toBe(true); // 16 * (4 / 8) = 8
  });

  it('maps FloraSpeciesId 0..7 to ecologically accurate FloraType meshes', () => {
    const expected: ReadonlyArray<worldGen.FloraType> = [
      worldGen.FloraType.Jungle, // 0 RainforestCanopy
      worldGen.FloraType.Round, // 1 MontaneBroadleaf
      worldGen.FloraType.Pine, // 2 ElfinConifer
      worldGen.FloraType.Cactus, // 3 DryThornScrub
      worldGen.FloraType.Palm, // 4 MangroveProp
      worldGen.FloraType.Acacia, // 5 SavannaAcacia
      worldGen.FloraType.Bush, // 6 ParamoEspeletia
      worldGen.FloraType.Round, // 7 RiparianWillow
    ];
    expect(WV2_SPECIES_TO_FLORA).toEqual(expected);

    const floraBin = encodeFloraSectorAnmf({
      sectorX: 8,
      sectorZ: 8,
      decorativeOnly: true,
      instances: expected.map((_, speciesId) => ({
        x_local_u16: ((speciesId + 1) * 4000) & 0xffff,
        z_local_u16: ((speciesId + 1) * 4000) & 0xffff,
        scale_u8: 120,
        yaw_u8: 64,
        species_id: speciesId,
        flags_u8: ANMF_FLAG_DECORATIVE,
      })),
    });
    const sector = decodeFloraSectorAnmf(floraBin);
    const n256 = 256 * 256;
    const cells = TILE_TOTAL_SIZE * TILE_TOTAL_SIZE;
    const bundle: WorldV2LoadedBundle = {
      simWorldIdentity: 'a'.repeat(64),
      bundleIdentity: 'b'.repeat(64),
      schemaTag: 'anima-world-v2-c01-v1',
      seed: 42,
      extentM: 40960,
      gridSize: 4096,
      floraSectorsPerSide: 16,
      mip: 2,
      size: 1024,
      macroAlbedoSize: TILE_TOTAL_SIZE,
      tileCountVerified: 1,
      sim: {
        width: 256,
        height: 256,
        seaLevelNorm: 1500 / 7200,
        simSeedU32: 42,
        seedU64: 42n,
        generatorVersion: 3,
        worldScale: 200,
        flags: 0,
        elevation: new Float32Array(n256).fill(0.45),
        moisture: new Float32Array(n256).fill(0.5),
        temperature: new Float32Array(n256).fill(0.55),
        flow: new Float32Array(n256),
        slope: new Float32Array(n256).fill(0.05),
        biome22: new Uint8Array(n256).fill(4),
        waterKind: new Uint8Array(n256),
        waterfalls: [],
        caveEntrances: [],
      },
      heightM: new Float32Array(cells).fill(420),
      normalRg: new Uint8Array(cells * 2).fill(128),
      macroAlbedoRgba: new Uint8Array(cells * 4).fill(128),
      biomeIdsRgba: new Uint8Array(cells * 4),
      biomeWeightsRgba: new Uint8Array(cells * 4),
      lightingRgba: new Uint8Array(cells * 4),
      horizonRgba: new Uint8Array(cells * 4),
      hydroFlowRgba: new Uint8Array(cells * 4),
      shoreSdfRg: new Uint8Array(cells),
      waterHeightM: new Float32Array(cells),
      waterKind: new Uint8Array(cells),
      caveMask: new Uint8Array(cells),
      caveNetwork: null,
      floraSectors: [sector],
    };
    const world = buildWorldFromV2Bundle(bundle);
    expect(world.floraCount).toBe(expected.length);
    expect(Array.from(world.floraType)).toEqual(expected);
  });

  it('derives flora sectors per side from index grid_size for 4096 and 2048', () => {
    expect(floraSectorsPerSideForGrid(4096)).toBe(16);
    expect(floraSectorsPerSideForGrid(2048)).toBe(8);
    expect(() => floraSectorsPerSideForGrid(0)).toThrow();
    expect(() => floraSectorsPerSideForGrid(300)).toThrow();

    const makeFloraBundle = (gridSize: number): WorldV2LoadedBundle => {
      const n256 = 256 * 256;
      const cells = TILE_TOTAL_SIZE * TILE_TOTAL_SIZE;
      const floraBin = encodeFloraSectorAnmf({
        sectorX: 4,
        sectorZ: 4,
        decorativeOnly: true,
        instances: [
          {
            x_local_u16: 0,
            z_local_u16: 0,
            species_id: 0,
            scale_u8: 128,
            yaw_u8: 0,
            flags_u8: ANMF_FLAG_DECORATIVE,
          },
        ],
      });
      return {
        simWorldIdentity: 'a'.repeat(64),
        bundleIdentity: 'b'.repeat(64),
        schemaTag: 'ANMW3-BIG-v1',
        seed: 42,
        extentM: 81920,
        gridSize,
        floraSectorsPerSide: floraSectorsPerSideForGrid(gridSize),
        mip: 2,
        size: 256,
        macroAlbedoSize: TILE_TOTAL_SIZE,
        tileCountVerified: 1,
        sim: {
          width: 256,
          height: 256,
          seaLevelNorm: 1500 / 7200,
          simSeedU32: 42,
          seedU64: 42n,
          generatorVersion: 3,
          worldScale: 200,
          flags: 0,
          elevation: new Float32Array(n256).fill(0.45),
          moisture: new Float32Array(n256).fill(0.5),
          temperature: new Float32Array(n256).fill(0.55),
          flow: new Float32Array(n256),
          slope: new Float32Array(n256).fill(0.05),
          biome22: new Uint8Array(n256).fill(4),
          waterKind: new Uint8Array(n256),
          waterfalls: [],
          caveEntrances: [],
        },
        heightM: new Float32Array(cells).fill(420),
        normalRg: new Uint8Array(cells * 2).fill(128),
        macroAlbedoRgba: new Uint8Array(cells * 4).fill(128),
        biomeIdsRgba: new Uint8Array(cells * 4),
        biomeWeightsRgba: new Uint8Array(cells * 4),
        lightingRgba: new Uint8Array(cells * 4),
        horizonRgba: new Uint8Array(cells * 4),
        hydroFlowRgba: new Uint8Array(cells * 4),
        shoreSdfRg: new Uint8Array(cells),
        waterHeightM: new Float32Array(cells),
        waterKind: new Uint8Array(cells),
        caveMask: new Uint8Array(cells),
        caveNetwork: null,
        floraSectors: [decodeFloraSectorAnmf(floraBin)],
      };
    };

    const w4096 = buildWorldFromV2Bundle(makeFloraBundle(4096)); // u = 4 / 16 = 0.25 -> (0.25 - 0.5) * 256 = -64
    const w2048 = buildWorldFromV2Bundle(makeFloraBundle(2048)); // u = 4 / 8 = 0.50 -> (0.50 - 0.5) * 256 = 0
    expect(w4096.floraX[0]).toBe(-64);
    expect(w2048.floraX[0]).toBe(0);
  });

  it('uses WB-D5 heightRatio 0.10 for World v2 at 81,920 m and 0.14 on all other paths', async () => {
    expect(resolveWorldHeightRatio('v2', 81_920)).toBe(0.10);
    expect(resolveWorldHeightRatio('v2', 40_960)).toBe(0.14);
    expect(resolveWorldHeightRatio('v2', null)).toBe(0.14);
    expect(resolveWorldHeightRatio('v1', 81_920)).toBe(0.14);
    expect(resolveWorldHeightRatio('v1', 40_960)).toBe(0.14);
    expect(resolveWorldHeightRatio('v1')).toBe(0.14);

    const files = buildSyntheticBundleMap({ extentM: 81_920, schemaTag: 'ANMW3-BIG-v1' });
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = typeof input === 'string' ? input : input instanceof URL ? input.pathname : input.url;
      const hit = files.get(url);
      if (!hit) return new Response(null, { status: 404 });
      return new Response(new Uint8Array(hit), { status: 200 });
    });
    const countersSpy = vi.spyOn(diagnosticsWindow, 'installFrameCounters').mockImplementation(() => ({
      applyPinnedPose: vi.fn(),
      dispose: vi.fn(),
    }));

    render(React.createElement(WorldShowcase));
    await waitFor(() => {
      expect(screen.getByTestId('world-v2-identity')).toBeTruthy();
    });

    const lastIdx = countersSpy.mock.calls.length - 1;
    expect(countersSpy.mock.calls[lastIdx][2]).toEqual({ renderSize: 1200, heightRatio: 0.10 });
  });

  it('caps WorldV2Flora emitted instances at BIG_4096_v2 scale while preserving baseStride on small packs', () => {
    // Small packs keep baseStride = 8 / 3 / 2 unchanged.
    expect(resolveWorldV2FloraStride('low', 16)).toBe(8);
    expect(resolveWorldV2FloraStride('medium', 16)).toBe(3);
    expect(resolveWorldV2FloraStride('high', 16)).toBe(2);

    // BIG_4096_v2 scale (536,381 flora instances): stride scales up so baseline stays inside
    // §4a budgets while 4x low negative control cleanly exceeds 530,000 flora triangles.
    const bigCount = 536_381;
    const lowStride = resolveWorldV2FloraStride('low', bigCount);
    const medStride = resolveWorldV2FloraStride('medium', bigCount);
    const highStride = resolveWorldV2FloraStride('high', bigCount);
    expect(lowStride).toBe(135);
    expect(medStride).toBe(40);
    expect(highStride).toBe(27);

    const lowEmitted = Math.ceil(bigCount / lowStride);
    const medEmitted = Math.ceil(bigCount / medStride);
    const highEmitted = Math.ceil(bigCount / highStride);
    expect(lowEmitted).toBeLessThanOrEqual(WORLD_V2_FLORA_MAX_INSTANCES.low);
    expect(medEmitted).toBeLessThanOrEqual(WORLD_V2_FLORA_MAX_INSTANCES.medium);
    expect(highEmitted).toBeLessThanOrEqual(WORLD_V2_FLORA_MAX_INSTANCES.high);

    const low4xEmitted = Math.ceil((bigCount * 4) / lowStride);
    expect(low4xEmitted * 44).toBeGreaterThan(530_000);
  });
});
