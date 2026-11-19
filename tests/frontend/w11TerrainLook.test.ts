import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import React from 'react';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import * as THREE from 'three';
import { Biome, FloraType, generateWorld, type World } from '../../src/components/Landscape/utils/worldGen';
import {
  clearWorldCache,
  loadOrGenerateWorld,
} from '../../src/components/Landscape/utils/worldCache';
import {
  buildColorTexture,
  buildRiverMaskTexture,
  computeHorizonAoMap,
} from '../../src/components/Landscape/utils/worldTerrainTextures';
import { createTerrainPbrMaterial } from '../../src/components/Landscape/materials/terrainPbrMaterial';
import {
  clearAssetCachesForTest,
  loadFloraAssetGeometries,
} from '../../src/components/Landscape/assets/worldAssets';
import { makeFloraGeometry } from '../../src/components/Landscape/utils/floraGeometry';
import { CANONICAL_XZ_EXTENT } from '../../src/components/Landscape/utils/mapManifest';
import {
  LAND_ANCHORS,
  createSyntheticTickGenerator,
} from '../../src/components/Landscape/utils/syntheticTicks';
import { biomeAt, sampleElevation } from '../../src/components/Landscape/utils/worldSample';
import { SHARED_WORLD_OPTS, SHARED_WORLD_SEED } from '../../src/utils/sharedWorld';

const HERE = dirname(fileURLToPath(import.meta.url));
const MODELS_DIR = resolve(HERE, '../../public/assets/world/models');

function decodeFlowChannel(byte: number): number {
  return (byte - 128) / 127;
}

function makeFlatSyntheticWorld(size: number, elev = 0.55, seaLevel = 0.38): World {
  const world = generateWorld(`w11-flat-${size}`, { size, shape: 'island' });
  world.seaLevel = seaLevel;
  world.elevation.fill(elev);
  world.slope.fill(0);
  world.water.fill(0);
  world.riverAmt.fill(0);
  world.shore.fill(0);
  world.biome.fill(Biome.Grassland);
  return world;
}

describe('W11 terrain look round 2 — C1 horizon AO, C2 river flow advection, and flora model guard', () => {
  let sharedWorld: World;

  beforeAll(() => {
    sharedWorld = generateWorld(SHARED_WORLD_SEED, SHARED_WORLD_OPTS);
  }, 60_000);

  afterEach(async () => {
    cleanup();
    await clearWorldCache();
    clearAssetCachesForTest();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('C1: leaves flat plains unoccluded (1.0), darkens V-valley floors, and halves occlusion under sea level', () => {
    const flat = makeFlatSyntheticWorld(128, 0.55, 0.38);
    const flatAo = computeHorizonAoMap(flat);
    expect(flatAo.size).toBe(128);
    for (let i = 0; i < flatAo.data.length; i++) {
      expect(flatAo.data[i]).toBe(1.0);
    }

    // Build a smooth synthetic V-valley centered along x = 64 so the valley floor sits at
    // elevation 0.40 and walls rise linearly to 0.90 at |x - 64| >= 25.
    const valley = makeFlatSyntheticWorld(128, 0.9, 0.38);
    for (let y = 0; y < 128; y++) {
      for (let x = 0; x < 128; x++) {
        const dist = Math.abs(x - 64);
        const h = dist >= 25 ? 0.9 : 0.4 + (dist / 25) * 0.5;
        valley.elevation[y * 128 + x] = h;
      }
    }
    const valleyAo = computeHorizonAoMap(valley);
    const floorAo = valleyAo.data[64 * 128 + 64];
    const rimAo = valleyAo.data[64 * 128 + 100];
    expect(rimAo).toBe(1.0);
    expect(floorAo).toBeLessThan(rimAo - 0.05);

    // Baked colour texture must also darken the V-valley floor relative to the flat plain / rim
    // using the exact same Grassland base biome (slope = 0 everywhere to isolate AO from grain).
    const tex = buildColorTexture(valley);
    const img = tex.image as { width: number; height: number; data: Uint8Array };
    const floorOff = (64 * img.width + 64) * 4;
    const rimOff = (64 * img.width + 100) * 4;
    const floorLuma =
      img.data[floorOff] * 0.2126 + img.data[floorOff + 1] * 0.7152 + img.data[floorOff + 2] * 0.0722;
    const rimLuma =
      img.data[rimOff] * 0.2126 + img.data[rimOff + 1] * 0.7152 + img.data[rimOff + 2] * 0.0722;
    expect(floorLuma).toBeLessThan(rimLuma * 0.92);
    tex.dispose();

    // Identical V-valley geometry submerged under seaLevel (= 0.95) must have half the horizon occlusion.
    const submerged = makeFlatSyntheticWorld(128, 0.9, 0.95);
    submerged.elevation.set(valley.elevation);
    const subAo = computeHorizonAoMap(submerged);
    const dryOcc = 1 - floorAo;
    const subOcc = 1 - subAo.data[64 * 128 + 64];
    expect(subOcc).toBeCloseTo(dryOcc * 0.5, 5);
  });

  it('C1: benchmarks horizon AO + buildColorTexture on 2048^2 shared world and caps Snow/Glacier <= 6% and land <= 5%', () => {
    const t0 = performance.now();
    const ao = computeHorizonAoMap(sharedWorld);
    const tAo = performance.now() - t0;

    const t1 = performance.now();
    const colorTex = buildColorTexture(sharedWorld);
    const tColor = performance.now() - t1;
    const noAoTex = buildColorTexture(sharedWorld, { disableAo: true });

    const aoData = (colorTex.image as { data: Uint8Array }).data;
    const baseData = (noAoTex.image as { data: Uint8Array }).data;
    const { size, biome, elevation, seaLevel } = sharedWorld;
    const n = size * size;
    const sLevel = seaLevel ?? 0.38;

    let snowBaseSum = 0;
    let snowAoSum = 0;
    let snowCount = 0;
    let glacierBaseSum = 0;
    let glacierAoSum = 0;
    let glacierCount = 0;
    let landBaseSum = 0;
    let landAoSum = 0;
    let landCount = 0;
    let oceanBaseSum = 0;
    let oceanAoSum = 0;
    let oceanCount = 0;

    for (let i = 0; i < n; i++) {
      const o = i * 4;
      const lBase = 0.2126 * baseData[o] + 0.7152 * baseData[o + 1] + 0.0722 * baseData[o + 2];
      const lAo = 0.2126 * aoData[o] + 0.7152 * aoData[o + 1] + 0.0722 * aoData[o + 2];
      const bio = biome[i];
      if (bio === Biome.Snow) {
        snowBaseSum += lBase;
        snowAoSum += lAo;
        snowCount++;
      } else if (bio === Biome.Glacier) {
        glacierBaseSum += lBase;
        glacierAoSum += lAo;
        glacierCount++;
      }
      if (elevation[i] >= sLevel) {
        landBaseSum += lBase;
        landAoSum += lAo;
        landCount++;
      } else {
        oceanBaseSum += lBase;
        oceanAoSum += lAo;
        oceanCount++;
      }
    }

    const snowRatio = snowAoSum / snowBaseSum;
    const glacierRatio = glacierAoSum / glacierBaseSum;
    const snowGlacierRatio = (snowAoSum + glacierAoSum) / (snowBaseSum + glacierBaseSum);
    const landRatio = landAoSum / landBaseSum;
    const oceanRatio = oceanAoSum / oceanBaseSum;

    colorTex.dispose();
    noAoTex.dispose();

    console.log(
      `\n[C1 BENCH @ ${sharedWorld.size}^2] computeHorizonAoMap(${ao.size}^2)=${tAo.toFixed(1)}ms, buildColorTexture=${tColor.toFixed(1)}ms | ` +
        `ratios(AO/noAO): Snow=${snowRatio.toFixed(4)} (${((1 - snowRatio) * 100).toFixed(2)}% dark, n=${snowCount}), ` +
        `Glacier=${glacierRatio.toFixed(4)} (${((1 - glacierRatio) * 100).toFixed(2)}% dark, n=${glacierCount}), ` +
        `Snow+Glacier=${snowGlacierRatio.toFixed(4)}, Land=${landRatio.toFixed(4)} (${((1 - landRatio) * 100).toFixed(2)}% dark, n=${landCount}), ` +
        `Ocean=${oceanRatio.toFixed(4)} (n=${oceanCount})`,
    );
    expect(ao.size).toBe(512);
    expect(tAo).toBeLessThan(400);
    expect(snowCount).toBeGreaterThan(100);
    expect(glacierCount).toBeGreaterThan(100);
    expect(landCount).toBeGreaterThan(1000);
    // R2: Snow and Glacier stay white (at most 6% darkening); average land darkening at most 5%.
    expect(snowRatio).toBeGreaterThanOrEqual(0.94);
    expect(glacierRatio).toBeGreaterThanOrEqual(0.94);
    expect(snowGlacierRatio).toBeGreaterThanOrEqual(0.94);
    expect(landRatio).toBeGreaterThanOrEqual(0.95);
    expect(oceanRatio).toBeGreaterThanOrEqual(0.98);
  });

  it('C2: encodes D8 downstream flow on synthetic sloped river, A=0 on flat lake junction, and compiles Vlachos GLSL', () => {
    const sloped = makeFlatSyntheticWorld(64, 0.5, 0.2);
    for (let y = 0; y < 64; y++) {
      for (let x = 0; x < 64; x++) {
        // Monotonic slope downwards toward (+x, +y): (x + 1, y + 1) is always steepest descent.
        sloped.elevation[y * 64 + x] = 0.9 - ((x + y) / 128) * 0.5;
      }
    }
    sloped.riverAmt[30 * 64 + 30] = 220;
    sloped.biome[30 * 64 + 30] = Biome.River;

    const maskTex = buildRiverMaskTexture(sloped);
    expect(maskTex.format).toBe(THREE.RGBAFormat);
    const img = maskTex.image as { width: number; height: number; data: Uint8Array };
    const off = (30 * 64 + 30) * 4;
    expect(img.data[off]).toBe(220);
    expect(img.data[off + 3]).toBe(255);
    const dx = decodeFlowChannel(img.data[off + 1]);
    const dy = decodeFlowChannel(img.data[off + 2]);
    expect(dx).toBeCloseTo(Math.SQRT1_2, 1);
    expect(dy).toBeCloseTo(Math.SQRT1_2, 1);

    // Dry cell holds neutral (128, 128, A=0) so bilinear sampling does not bias bank edges.
    const dryOff = (10 * 64 + 10) * 4;
    expect(img.data[dryOff]).toBe(0);
    expect(img.data[dryOff + 1]).toBe(128);
    expect(img.data[dryOff + 2]).toBe(128);
    expect(img.data[dryOff + 3]).toBe(0);

    // Flat lake junction with equal neighbours and uniform flow decodes A = 0, G = 128, B = 128.
    const flatRiver = makeFlatSyntheticWorld(32, 0.5, 0.2);
    flatRiver.flow.fill(0);
    flatRiver.riverAmt[16 * 32 + 16] = 200;
    const flatTex = buildRiverMaskTexture(flatRiver);
    const flatImg = flatTex.image as { data: Uint8Array };
    const fOff = (16 * 32 + 16) * 4;
    expect(flatImg.data[fOff]).toBe(200);
    expect(flatImg.data[fOff + 1]).toBe(128);
    expect(flatImg.data[fOff + 2]).toBe(128);
    expect(flatImg.data[fOff + 3]).toBe(0);
    flatTex.dispose();

    // Verify terrain PBR material shader hook and bumped program cache key.
    const dummy = new THREE.DataTexture(new Uint8Array([128, 128, 128, 255]), 1, 1, THREE.RGBAFormat);
    dummy.needsUpdate = true;
    const { material, handle } = createTerrainPbrMaterial({
      colorMap: dummy,
      normalMap: dummy,
      roughnessMap: dummy,
      detailMap: dummy,
      riverMaskMap: maskTex,
      splatMap: dummy,
      heightUnits: 156,
    });
    expect(material.customProgramCacheKey()).toBe('w3-lite-terrain-pbr-v3');
    const shader = {
      uniforms: THREE.UniformsUtils.clone(THREE.ShaderLib.standard.uniforms) as Record<
        string,
        { value: unknown }
      >,
      vertexShader: THREE.ShaderLib.standard.vertexShader,
      fragmentShader: THREE.ShaderLib.standard.fragmentShader,
    };
    material.onBeforeCompile(shader as unknown as THREE.WebGLProgramParametersWithUniforms, {} as THREE.WebGLRenderer);
    expect(shader.fragmentShader).toContain('vec4 rivMask = texture2D(uRiverMask, vMapUv)');
    expect(shader.fragmentShader).toContain('float phaseJitter = texture2D(uDetail');
    expect(shader.fragmentShader).toContain('mix(fallbackFlow, advected, validFlow)');
    handle.dispose();
    dummy.dispose();
    maskTex.dispose();
  });

  it('C2: verifies >= 97% of core river cells point to a strict D8 downstream neighbour and <= 1% point upstream', () => {
    const t0 = performance.now();
    const maskTex = buildRiverMaskTexture(sharedWorld);
    const elapsed = performance.now() - t0;
    const data = (maskTex.image as { data: Uint8Array }).data;
    const { size, biome, riverAmt, elevation, flow } = sharedWorld;

    let totalRiverCells = 0;
    let validDirectionCells = 0;
    let strictDownstreamD8 = 0;
    let hydroChecked = 0;
    let hydroAgree = 0;
    let hydroUpstream = 0;

    let totalMaskCells = 0;
    let maskHydroChecked = 0;
    let maskHydroAgree = 0;
    let maskHydroUpstream = 0;

    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        const i = y * size + x;
        const amt = riverAmt[i];
        if (amt === 0) continue;
        totalMaskCells++;

        const isCoreRiver = biome[i] === Biome.River;
        if (isCoreRiver) totalRiverCells++;

        const o = i * 4;
        if (data[o + 3] !== 255) continue;
        if (isCoreRiver) validDirectionCells++;

        const fx = decodeFlowChannel(data[o + 1]);
        const fy = decodeFlowChannel(data[o + 2]);
        const fLen = Math.hypot(fx, fy);
        if (fLen === 0) continue;

        // Strict immediate D8 neighbour step (chebyshev distance == 1).
        const maxAxis = Math.max(Math.abs(fx), Math.abs(fy));
        const dx = Math.round(fx / maxAxis);
        const dy = Math.round(fy / maxAxis);
        const tx = x + dx;
        const ty = y + dy;
        const tIdx = ty * size + tx;
        const inBounds = tx >= 0 && tx < size && ty >= 0 && ty < size;

        const isDownstreamD8 =
          inBounds && (flow[tIdx] > flow[i] + 1e-7 || elevation[tIdx] < elevation[i] - 1e-7);
        if (isCoreRiver && isDownstreamD8) {
          strictDownstreamD8++;
        }

        // Compare encoded vector against true hydrological downstream receiver (highest flow D8 neighbour).
        let bestFlow = flow[i] + 1e-7;
        let rx = 0;
        let ry = 0;
        let hasReceiver = false;
        for (let ndy = -1; ndy <= 1; ndy++) {
          for (let ndx = -1; ndx <= 1; ndx++) {
            if (ndx === 0 && ndy === 0) continue;
            const nx = x + ndx;
            const ny = y + ndy;
            if (nx < 0 || nx >= size || ny < 0 || ny >= size) continue;
            const nf = flow[ny * size + nx];
            if (nf > bestFlow) {
              bestFlow = nf;
              rx = ndx;
              ry = ndy;
              hasReceiver = true;
            }
          }
        }
        if (hasReceiver) {
          const cos = (fx * rx + fy * ry) / (fLen * Math.hypot(rx, ry));
          maskHydroChecked++;
          if (cos > 0.3) maskHydroAgree++;
          if (cos < -0.7) maskHydroUpstream++;
          if (isCoreRiver) {
            hydroChecked++;
            if (cos > 0.3) hydroAgree++;
            if (cos < -0.7) hydroUpstream++;
          }
        }
      }
    }

    const pctDownstream = totalRiverCells > 0 ? (strictDownstreamD8 / totalRiverCells) * 100 : 0;
    const pctHydroAgree = hydroChecked > 0 ? (hydroAgree / hydroChecked) * 100 : 0;
    const pctHydroUpstream = hydroChecked > 0 ? (hydroUpstream / hydroChecked) * 100 : 0;
    const pctMaskAgree = maskHydroChecked > 0 ? (maskHydroAgree / maskHydroChecked) * 100 : 0;
    const pctMaskUpstream = maskHydroChecked > 0 ? (maskHydroUpstream / maskHydroChecked) * 100 : 0;

    console.log(
      `\n[C2 STRICT D8 FLOW @ ${size}^2] build=${elapsed.toFixed(1)}ms, coreRiver=${totalRiverCells}, A=255=${validDirectionCells}, ` +
        `strictD8Downstream=${strictDownstreamD8} (${pctDownstream.toFixed(2)}%), hydroChecked=${hydroChecked}, ` +
        `hydroAgree=${hydroAgree} (${pctHydroAgree.toFixed(2)}%), hydroUpstream=${hydroUpstream} (${pctHydroUpstream.toFixed(2)}%) | ` +
        `allMask(${totalMaskCells}): hydroAgree=${pctMaskAgree.toFixed(2)}%, hydroUpstream=${pctMaskUpstream.toFixed(2)}%`,
    );
    maskTex.dispose();

    expect(totalRiverCells).toBeGreaterThan(1000);
    expect(pctDownstream).toBeGreaterThanOrEqual(97.0);
    expect(pctHydroAgree).toBeGreaterThanOrEqual(97.0);
    expect(pctHydroUpstream).toBeLessThanOrEqual(1.0);
  });

  it('Flora guard: non-tree FloraTypes never load pine/tree_broad GLBs and zero trees spawn on snow/glacier/rock', async () => {
    const requestedUrls: string[] = [];
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = typeof input === 'string' ? input : input instanceof URL ? input.toString() : input.url;
      requestedUrls.push(url);
      const m = /^\/assets\/world\/models\/([a-z0-9_]+\.glb)$/.exec(url);
      if (!m) return new Response(null, { status: 404 });
      const bytes = readFileSync(resolve(MODELS_DIR, m[1]));
      return new Response(new Uint8Array(bytes), {
        status: 200,
        headers: { 'Content-Type': 'model/gltf-binary' },
      });
    });

    // Check every mapped non-tree type individually so none can alias a conifer or broadleaf tree GLB.
    const expectedNonTreeGlbs: Array<[FloraType, string]> = [
      [FloraType.Bush, '/assets/world/models/shrub.glb'],
      [FloraType.Rock, '/assets/world/models/rock.glb'],
      [FloraType.Reed, '/assets/world/models/fern.glb'],
    ];
    for (const [floraType, expectedUrl] of expectedNonTreeGlbs) {
      clearAssetCachesForTest();
      requestedUrls.length = 0;
      const loaded = await loadFloraAssetGeometries([floraType], 'high');
      expect(loaded).not.toBeNull();
      expect(loaded).toHaveLength(1);
      expect(requestedUrls).toEqual([expectedUrl]);
      expect(requestedUrls).not.toContain('/assets/world/models/pine.glb');
      expect(requestedUrls).not.toContain('/assets/world/models/tree_broad.glb');
    }

    // Procedural-only non-tree types (Tuft, Coral, Kelp, Seagrass, Cactus) must stay procedural without fetching tree GLBs.
    clearAssetCachesForTest();
    requestedUrls.length = 0;
    const procTypes = [FloraType.Tuft, FloraType.Coral, FloraType.Kelp, FloraType.Seagrass, FloraType.Cactus];
    const procRes = await loadFloraAssetGeometries(procTypes, 'high');
    expect(procRes).toBeNull();
    expect(requestedUrls).toEqual([]);
    for (const ft of procTypes) {
      const geom = makeFloraGeometry(ft);
      expect(geom.getAttribute('position').count).toBeGreaterThan(0);
      geom.dispose();
    }

    // Verify zero tree FloraTypes land on Snow, Glacier, Alpine, or Rock in the 2048^2 shared world data.
    const treeTypes = new Set<number>([
      FloraType.Pine,
      FloraType.Round,
      FloraType.Jungle,
      FloraType.Acacia,
      FloraType.Palm,
      FloraType.DeadTree,
    ]);
    const alpineBareBiomes = new Set<number>([Biome.Snow, Biome.Glacier, Biome.Alpine, Biome.Rock]);
    const { size, biome, floraCount, floraX, floraZ, floraType } = sharedWorld;
    let treesOnSnowOrRock = 0;
    for (let k = 0; k < floraCount; k++) {
      if (!treeTypes.has(floraType[k])) continue;
      const gx = Math.min(size - 1, Math.max(0, Math.round(floraX[k] + size / 2)));
      const gz = Math.min(size - 1, Math.max(0, Math.round(floraZ[k] + size / 2)));
      if (alpineBareBiomes.has(biome[gz * size + gx])) {
        treesOnSnowOrRock++;
      }
    }
    expect(treesOnSnowOrRock).toBe(0);
  });

  it('Synthetic anchors: every LAND_ANCHORS point sits on lowland land of the 2048^2 shared world (never water/beach/snow/alpine)', () => {
    const disallowed = new Set<Biome>([
      Biome.Ocean,
      Biome.Lake,
      Biome.River,
      Biome.Beach,
      Biome.Snow,
      Biome.Glacier,
      Biome.Rock,
      Biome.Alpine,
    ]);
    expect(LAND_ANCHORS.length).toBeGreaterThan(0);

    for (const [x, z] of LAND_ANCHORS) {
      const b = biomeAt(sharedWorld, x, z, CANONICAL_XZ_EXTENT) as Biome;
      const u = (x + CANONICAL_XZ_EXTENT * 0.5) / CANONICAL_XZ_EXTENT;
      const v = (z + CANONICAL_XZ_EXTENT * 0.5) / CANONICAL_XZ_EXTENT;
      const elev = sampleElevation(sharedWorld, u, v);
      expect(disallowed.has(b), `LAND_ANCHORS [${x}, ${z}] landed on disallowed ${Biome[b]}`).toBe(false);
      expect(elev, `LAND_ANCHORS [${x}, ${z}] (${Biome[b]}) too high/submerged: ${elev}`).toBeGreaterThan(
        sharedWorld.seaLevel,
      );
    }

    // Also verify default synthetic fixture trees/lakes/food stay completely clear of Ocean/Snow/Glacier/Rock/Alpine.
    const highOrSea = new Set<Biome>([Biome.Ocean, Biome.Snow, Biome.Glacier, Biome.Rock, Biome.Alpine]);
    const elements = createSyntheticTickGenerator({ agents: 200 }).tickAt(0).environmental_state.elements;
    expect(elements.length).toBeGreaterThan(0);
    for (const el of elements) {
      const b = biomeAt(sharedWorld, el.x, el.y, CANONICAL_XZ_EXTENT) as Biome;
      expect(
        highOrSea.has(b),
        `synthetic ${el.type} (${el.x.toFixed(1)}, ${el.y.toFixed(1)}) landed on ${Biome[b]}`,
      ).toBe(false);
    }
  });

  it('R1: worker constructor throw and worker onerror both recover via fallback (never hang)', async () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    // 1. Mock Worker constructor throwing synchronously.
    vi.stubGlobal(
      'Worker',
      vi.fn(function () {
        throw new Error('Mock Worker ctor failed');
      }),
    );
    const wThrow = await loadOrGenerateWorld('r1-worker-throw', { size: 32, shape: 'island' });
    expect(wThrow.size).toBe(32);
    expect(wThrow.elevation.length).toBe(32 * 32);
    expect(errSpy).toHaveBeenCalled();

    // 2. Mock Worker instance posting an error event on postMessage.
    errSpy.mockClear();
    vi.stubGlobal(
      'Worker',
      vi.fn(function () {
        return {
          onmessage: null as ((e: MessageEvent<World>) => void) | null,
          onerror: null as ((ev: ErrorEvent) => void) | null,
          terminate: vi.fn(),
          postMessage() {
            this.onerror?.(new ErrorEvent('error', { message: 'Mock worker error event' }));
          },
        };
      }),
    );
    const wErr = await loadOrGenerateWorld('r1-worker-error', { size: 32, shape: 'island' });
    expect(wErr.size).toBe(32);
    expect(wErr.biome.length).toBe(32 * 32);
    expect(errSpy).toHaveBeenCalled();
  });

  it('R1: WorldShowcase and App surface visible error UI and never hang when shared-world load rejects', async () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.resetModules();

    // Stub indexedDB so WorldShowcase takes its async loadOrGenerateWorld path rather than sync memo.
    vi.stubGlobal('indexedDB', {
      open: vi.fn(),
      deleteDatabase: vi.fn(() => {
        const req: { onsuccess?: () => void } = {};
        queueMicrotask(() => req.onsuccess?.());
        return req;
      }),
    });

    const mockReject = vi.fn().mockRejectedValue(new Error('Simulated worker/world load failure'));
    vi.doMock('../../src/components/Landscape/utils/worldCache', () => ({
      loadOrGenerateWorld: mockReject,
      getMemoizedWorld: vi.fn().mockReturnValue(null),
      clearWorldCache: vi.fn().mockResolvedValue(undefined),
    }));
    vi.doMock('pixi.js', () => ({
      Application: vi.fn(function () {
        return {
          init: vi.fn().mockResolvedValue(undefined),
          canvas: document.createElement('canvas'),
          stage: { addChild: vi.fn(), removeChild: vi.fn() },
          renderer: {},
          destroy: vi.fn(),
        };
      }),
      Graphics: vi.fn(function () {
        return {
          clear: vi.fn().mockReturnThis(),
          beginFill: vi.fn().mockReturnThis(),
          drawCircle: vi.fn().mockReturnThis(),
          endFill: vi.fn().mockReturnThis(),
        };
      }),
    }));
    vi.doMock('@tauri-apps/api/core', async (importOriginal) => {
      const orig = await importOriginal<typeof import('@tauri-apps/api/core')>();
      return {
        ...orig,
        invoke: vi.fn(async (cmd: string) => {
          switch (cmd) {
            case 'get_simulation_status':
              return { running: false, tick_count: 0, avg_tick_time_ms: 0, fps: 0 };
            case 'get_map_elites_grid':
              return { grid: {}, grid_resolution: 50 };
            case 'get_pheromone_grid':
              return { grid: [], width: 128, height: 128 };
            case 'get_active_raycasts':
              return [];
            case 'get_environmental_elements':
              return { elements: [] };
            case 'get_lineage_graph':
              return { nodes: [], links: [], db_connected: false };
            case 'get_chronicle_history':
              return [];
            case 'get_ecosystem_state':
              return {
                detritus: 0,
                plants: 0,
                animals: 0,
                total: 0,
                prey_count: 0,
                predator_count: 0,
                shannon: 0,
                simpson: 0,
                prey_mass: 0,
                predator_mass: 0,
                niche_divergence: 0,
                archive_coverage: 0,
              };
            default:
              return null;
          }
        }),
      };
    });

    try {
      const { WorldShowcase } = await import('../../src/components/Landscape/WorldShowcase');
      render(React.createElement(WorldShowcase));
      const alert = await waitFor(() => screen.getByTestId('world-load-error'));
      expect(alert.textContent).toContain('Simulated worker/world load failure');
      expect(screen.queryByText(/Generating a huge/i)).toBeNull();
      cleanup();

      const { App } = await import('../../src/App');
      render(React.createElement(App));
      await waitFor(() => {
        const errBanner = screen.getByText(/Shared world failed to load/i);
        expect(errBanner.textContent).toContain('Simulated worker/world load failure');
      });
      const toggleBtn = screen.getByTestId('toggle-simulation-button') as HTMLButtonElement;
      expect(toggleBtn.disabled).toBe(false);
      expect(errSpy).toHaveBeenCalled();
    } finally {
      vi.doUnmock('../../src/components/Landscape/utils/worldCache');
      vi.doUnmock('pixi.js');
      vi.doUnmock('@tauri-apps/api/core');
      vi.resetModules();
    }
  });
});
