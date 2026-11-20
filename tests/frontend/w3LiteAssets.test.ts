import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import React from 'react';
import { describe, expect, it, vi, afterEach } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import * as THREE from 'three';
import { BIOME_COUNT, Biome, FloraType, generateWorld } from '../../src/components/Landscape/utils/worldGen';
import {
  BIOME_DISPLAY_RGB,
  buildRoughnessTexture,
  buildSplatTexture,
} from '../../src/components/Landscape/utils/worldTerrainTextures';
import { buildCavesGeometry } from '../../src/components/Landscape/utils/caveGeometry';
import WorldMinimap, { type CameraView } from '../../src/components/Landscape/WorldMinimap';
import {
  clearAssetCachesForTest,
  disposeOtherTerrainPbrTiers,
  loadFloraAssetGeometries,
  loadTerrainPbrArrays,
} from '../../src/components/Landscape/assets/worldAssets';
import { createTerrainPbrMaterial } from '../../src/components/Landscape/materials/terrainPbrMaterial';
import { createFloraWindMaterial } from '../../src/components/Landscape/materials/floraWindMaterial';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '../..');
const WORLD_ASSETS_DIR = resolve(ROOT, 'public/assets/world');

const ALLOWED_SITES = new Set(['polyhaven.com', 'ambientcg.com']);
const MAX_COMMITTED_BYTES = 20 * 1024 * 1024;

function hostnameMatchesSite(rawUrl: string, site: string): boolean {
  const host = new URL(rawUrl).hostname.toLowerCase();
  return host === site || host.endsWith(`.${site}`);
}

interface SourceEntry {
  file: string;
  sourceSite: string;
  assetId: string;
  assetName: string;
  sourceUrl: string;
  license: string;
  licence_url: string;
  downloadDate: string;
  originalResolution: string;
  bytes: number;
  processing: string;
}

function listFilesRecursive(dir: string): string[] {
  const out: string[] = [];
  for (const name of readdirSync(dir)) {
    const full = resolve(dir, name);
    const st = statSync(full);
    if (st.isDirectory()) {
      out.push(...listFilesRecursive(full));
    } else if (st.isFile()) {
      out.push(relative(WORLD_ASSETS_DIR, full).replace(/\\/g, '/'));
    }
  }
  return out;
}

describe('W3-lite CC0 assets, provenance, terrain PBR splat, and flora fallback', () => {
  afterEach(() => {
    cleanup();
    clearAssetCachesForTest();
    vi.restoreAllMocks();
  });

  it('tracks every committed file under public/assets/world/ in SOURCES.json as CC0-1.0 within 20 MB', () => {
    const sourcesPath = resolve(WORLD_ASSETS_DIR, 'SOURCES.json');
    expect(existsSync(sourcesPath)).toBe(true);
    const parsed = JSON.parse(readFileSync(sourcesPath, 'utf8')) as {
      schemaVersion: number;
      assets: SourceEntry[];
    };
    expect(Array.isArray(parsed.assets)).toBe(true);
    expect(parsed.assets.length).toBeGreaterThan(0);

    const diskFiles = listFilesRecursive(WORLD_ASSETS_DIR).filter((f) => f !== 'SOURCES.json');
    const diskSet = new Set(diskFiles);
    const listedSet = new Set<string>();
    let totalBytes = statSync(sourcesPath).size;

    for (const entry of parsed.assets) {
      expect(typeof entry.file).toBe('string');
      expect(entry.file.length).toBeGreaterThan(0);
      expect(listedSet.has(entry.file)).toBe(false);
      listedSet.add(entry.file);

      const full = resolve(WORLD_ASSETS_DIR, entry.file);
      expect(existsSync(full)).toBe(true);
      const actualSize = statSync(full).size;
      expect(entry.bytes).toBe(actualSize);
      totalBytes += actualSize;

      expect(entry.license).toBe('CC0-1.0');
      expect(typeof entry.licence_url).toBe('string');
      expect(entry.licence_url).toMatch(/^https:\/\//);
      expect(ALLOWED_SITES.has(entry.sourceSite)).toBe(true);
      expect(entry.sourceUrl).toMatch(/^https:\/\//);
      expect(hostnameMatchesSite(entry.sourceUrl, entry.sourceSite)).toBe(true);
      expect(hostnameMatchesSite(entry.licence_url, entry.sourceSite)).toBe(true);
      expect(entry.assetName.trim().length).toBeGreaterThan(0);
      expect(entry.assetId.trim().length).toBeGreaterThan(0);
      expect(entry.downloadDate).toMatch(/^\d{4}-\d{2}-\d{2}$/);
      expect(entry.originalResolution.trim().length).toBeGreaterThan(0);
      expect(entry.processing.trim().length).toBeGreaterThan(0);
    }

    for (const f of diskFiles) {
      expect(listedSet.has(f)).toBe(true);
    }
    expect(listedSet.size).toBe(diskSet.size);
    expect(totalBytes).toBeLessThanOrEqual(MAX_COMMITTED_BYTES);
  });

  it('builds valid RGBA splat weights for Forest, Beach, Swamp, and Snow biomes', () => {
    const world = generateWorld('w3-splat-test', { size: 32, shape: 'island' });
    // Force known biomes in four distinct quadrants to verify exact RGBA channels deterministically.
    const n = world.size;
    world.biome[0] = Biome.Forest; // (x=0, y=0) -> strong R
    world.biome[n - 1] = Biome.Beach; // (x=n-1, y=0) -> strong G
    world.biome[(n - 1) * n] = Biome.Swamp; // (x=0, y=n-1) -> strong B
    world.biome[(n - 1) * n + (n - 1)] = Biome.Snow; // (x=n-1, y=n-1) -> strong A

    const splat = buildSplatTexture(world);
    const img = splat.image as { width: number; height: number; data: Uint8Array };
    expect(img.width).toBeGreaterThanOrEqual(256);
    expect(img.data.length).toBe(img.width * img.height * 4);

    const w = img.width;
    const h = img.height;
    const forestPx = 0;
    const beachPx = (w - 1) * 4;
    const swampPx = (h - 1) * w * 4;
    const snowPx = ((h - 1) * w + (w - 1)) * 4;

    expect(img.data[forestPx]).toBeGreaterThan(180); // R = forest
    expect(img.data[beachPx + 1]).toBeGreaterThan(200); // G = sand
    expect(img.data[swampPx + 2]).toBeGreaterThan(200); // B = mud
    expect(img.data[snowPx + 3]).toBeGreaterThan(240); // A = snow
    splat.dispose();
  });

  it('loads real CC0 GLBs via the fetch seam and falls back cleanly to null on 404 (N2)', async () => {
    const requestedUrls: string[] = [];
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = typeof input === 'string' ? input : input instanceof URL ? input.toString() : input.url;
      requestedUrls.push(url);
      const match = /^\/assets\/world\/models\/([a-z0-9_]+\.glb)$/.exec(url);
      if (!match) return new Response(null, { status: 404 });
      const bytes = readFileSync(resolve(WORLD_ASSETS_DIR, 'models', match[1]));
      return new Response(new Uint8Array(bytes), {
        status: 200,
        headers: { 'Content-Type': 'model/gltf-binary' },
      });
    });

    const sampleTypes = [
      FloraType.Pine,
      FloraType.Round,
      FloraType.Bush,
      FloraType.Rock,
      FloraType.Reed,
      FloraType.Tuft,
    ];
    const positive = await loadFloraAssetGeometries(sampleTypes, 'high');
    expect(positive).not.toBeNull();
    expect(positive).toHaveLength(sampleTypes.length);
    for (const slug of ['pine', 'tree_broad', 'shrub', 'rock', 'fern']) {
      expect(requestedUrls).toContain(`/assets/world/models/${slug}.glb`);
    }
    for (const entry of positive!) {
      expect(entry.geometry.getAttribute('position').count).toBeGreaterThan(0);
    }

    // 404 fallback must actually hit `fetch` through the same seam rather than failing pre-fetch.
    clearAssetCachesForTest();
    requestedUrls.length = 0;
    fetchSpy.mockImplementation(async (input) => {
      const url = typeof input === 'string' ? input : input instanceof URL ? input.toString() : input.url;
      requestedUrls.push(url);
      return new Response(null, { status: 404 });
    });

    const pbr = await loadTerrainPbrArrays('high');
    expect(pbr).toBeNull();

    requestedUrls.length = 0;
    const floraLow = await loadFloraAssetGeometries([FloraType.Pine], 'low');
    expect(floraLow).toBeNull();
    expect(requestedUrls).toHaveLength(0);

    const floraHigh = await loadFloraAssetGeometries([FloraType.Pine], 'high');
    expect(floraHigh).toBeNull();
    expect(requestedUrls).toContain('/assets/world/models/pine.glb');
  });

  it('disposes High 1024^2 terrain PBR textures when dropping to Low (N5)', async () => {
    const stubCanvas = () =>
      vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
        drawImage: vi.fn(),
        getImageData: (_x: number, _y: number, w: number, h: number) => ({
          data: new Uint8ClampedArray(w * h * 4),
        }),
      } as never);

    vi.stubGlobal(
      'createImageBitmap',
      vi.fn(async () => ({ close: vi.fn() })),
    );
    stubCanvas();
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockImplementation(async () => new Response(new Uint8Array([1, 2, 3]), { status: 200 }));

    try {
      const high = await loadTerrainPbrArrays('high');
      expect(high).not.toBeNull();
      expect(high!.tileSize).toBe(1024);

      const albedoDispose = vi.spyOn(high!.albedoArray, 'dispose');
      const normalDispose = vi.spyOn(high!.normalArray, 'dispose');

      const low = await loadTerrainPbrArrays('low');
      expect(low).not.toBeNull();
      expect(low!.tileSize).toBe(512);
      expect(albedoDispose).toHaveBeenCalledTimes(1);
      expect(normalDispose).toHaveBeenCalledTimes(1);

      // In-flight High load must dispose itself and return null if Auto drops to Low mid-load.
      clearAssetCachesForTest();
      let releaseHigh!: () => void;
      const highGate = new Promise<void>((resolve) => {
        releaseHigh = resolve;
      });
      let firstWave = true;
      fetchSpy.mockImplementation(async () => {
        if (firstWave) {
          await highGate;
        }
        return new Response(new Uint8Array([1, 2, 3]), { status: 200 });
      });

      const pendingHigh = loadTerrainPbrArrays('high');
      firstWave = false;
      disposeOtherTerrainPbrTiers('low');
      const raceLow = await loadTerrainPbrArrays('low');
      releaseHigh();
      const finishedHigh = await pendingHigh;
      expect(raceLow?.tileSize).toBe(512);
      expect(finishedHigh).toBeNull();

      // Rapid High -> Low -> High oscillation while Wave 1 is in flight must discard Wave 1 on
      // late completion (disposing ONLY Wave 1's newly created arrays, returning null) and keep
      // the active Wave 2 textures alive in `pbrResolved(1024)` without disposing them (NR3).
      clearAssetCachesForTest();
      let releaseWave1!: () => void;
      const wave1Gate = new Promise<void>((resolve) => {
        releaseWave1 = resolve;
      });
      let wave = 0;
      fetchSpy.mockImplementation(async () => {
        const myWave = wave++;
        if (myWave < 12) {
          await wave1Gate;
        }
        return new Response(new Uint8Array([1, 2, 3]), { status: 200 });
      });

      const highWave1Promise = loadTerrainPbrArrays('high');
      disposeOtherTerrainPbrTiers('low');
      disposeOtherTerrainPbrTiers('high');
      const highWave2 = await loadTerrainPbrArrays('high');
      expect(highWave2).not.toBeNull();
      const w2AlbedoDispose = vi.spyOn(highWave2!.albedoArray, 'dispose');
      const w2NormalDispose = vi.spyOn(highWave2!.normalArray, 'dispose');
      const protoDisposeSpy = vi.spyOn(THREE.DataArrayTexture.prototype, 'dispose');

      releaseWave1();
      const highWave1 = await highWave1Promise;
      // Superseded Wave 1 must return null and dispose only its own 2 DataArrayTextures (albedo + normal).
      expect(highWave1).toBeNull();
      expect(protoDisposeSpy).toHaveBeenCalledTimes(2);
      // The currently bound Wave 2 textures must remain alive (0 dispose calls) and still cached.
      expect(w2AlbedoDispose).toHaveBeenCalledTimes(0);
      expect(w2NormalDispose).toHaveBeenCalledTimes(0);
      const cachedActive = await loadTerrainPbrArrays('high');
      expect(cachedActive).toBe(highWave2);
      expect(w2AlbedoDispose).toHaveBeenCalledTimes(0);
      expect(w2NormalDispose).toHaveBeenCalledTimes(0);

      clearAssetCachesForTest();
      expect(w2AlbedoDispose).toHaveBeenCalledTimes(1);
      expect(w2NormalDispose).toHaveBeenCalledTimes(1);
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('compiles terrain PBR and flora wind onBeforeCompile hooks cleanly against ShaderLib.standard', () => {
    const tex = new THREE.DataTexture(new Uint8Array([128, 128, 128, 255]), 1, 1, THREE.RGBAFormat);
    tex.needsUpdate = true;
    const { material: terrainMat, handle } = createTerrainPbrMaterial({
      colorMap: tex,
      normalMap: tex,
      roughnessMap: tex,
      detailMap: tex,
      riverMaskMap: tex,
      splatMap: tex,
      heightUnits: 168,
    });

    const shader = {
      uniforms: THREE.UniformsUtils.clone(THREE.ShaderLib.standard.uniforms) as Record<
        string,
        { value: unknown }
      >,
      vertexShader: THREE.ShaderLib.standard.vertexShader,
      fragmentShader: THREE.ShaderLib.standard.fragmentShader,
    };
    terrainMat.onBeforeCompile(shader as unknown as THREE.WebGLProgramParametersWithUniforms, {} as THREE.WebGLRenderer);
    expect(shader.uniforms.uPbrEnabled.value).toBe(0);
    expect(shader.vertexShader).toContain('vW3WorldPos');
    expect(shader.fragmentShader).toContain('sampler2DArray uPbrAlbedo');
    expect(shader.fragmentShader).toContain('w3SampleTriplanarRock');

    handle.updateTime(4.25);
    expect(shader.uniforms.uTime.value).toBeCloseTo(4.25, 5);
    handle.dispose();
    tex.dispose();

    const floraBundle = createFloraWindMaterial({
      enableWind: true,
      smoothShading: true,
    });
    floraBundle.updateTime(2.5);
    const fShader = {
      uniforms: THREE.UniformsUtils.clone(THREE.ShaderLib.standard.uniforms) as Record<
        string,
        { value: unknown }
      >,
      vertexShader: THREE.ShaderLib.standard.vertexShader,
      fragmentShader: THREE.ShaderLib.standard.fragmentShader,
    };
    floraBundle.material.onBeforeCompile(
      fShader as unknown as THREE.WebGLProgramParametersWithUniforms,
      {} as THREE.WebGLRenderer,
    );
    expect(fShader.uniforms.uWindTime.value).toBeCloseTo(2.5, 5);
    expect(fShader.vertexShader).toContain('instanceMatrix');
    floraBundle.dispose();
  });

  it('gives all 22 biomes distinct display colours, distinct roughness tiers, and complete splat coverage', () => {
    expect(BIOME_DISPLAY_RGB.length).toBe(BIOME_COUNT);
    const uniqueRgb = new Set(BIOME_DISPLAY_RGB.map((c) => c.join(',')));
    expect(uniqueRgb.size).toBe(BIOME_COUNT);

    // Construct a 512x512 world so buildRoughnessTexture (res = size >> 1 = 256) and
    // buildSplatTexture (res = 512) map grid indices cleanly in four corners.
    const world = generateWorld('w3-biome-rough-test', { size: 512, shape: 'island' });
    const n = world.size;
    world.riverAmt.fill(0);
    world.water.fill(0);
    world.shore.fill(0);
    world.elevation.fill(0.55);

    const tl = 0;
    const tr = n - 1;
    const bl = (n - 1) * n;
    const br = n * n - 1;
    world.biome[tl] = Biome.Glacier;
    world.biome[tr] = Biome.Bog;
    world.biome[bl] = Biome.Badlands;
    world.biome[br] = Biome.Desert;

    const rough = buildRoughnessTexture(world);
    const rImg = rough.image as { width: number; height: number; data: Uint8Array };
    const rw = rImg.width;
    const rh = rImg.height;
    const gGlacier = rImg.data[1];
    const gBog = rImg.data[(rw - 1) * 4 + 1];
    const gBadlands = rImg.data[((rh - 1) * rw) * 4 + 1];
    const gDesert = rImg.data[((rh - 1) * rw + (rw - 1)) * 4 + 1];
    expect(gGlacier).toBeLessThan(gBog);
    expect(gBog).toBeLessThan(gBadlands);
    expect(gDesert).toBeGreaterThanOrEqual(245);
    rough.dispose();

    const splat = buildSplatTexture(world);
    const sImg = splat.image as { width: number; height: number; data: Uint8Array };
    const sw = sImg.width;
    const sh = sImg.height;
    const pxGlacierA = sImg.data[3];
    const pxBogB = sImg.data[(sw - 1) * 4 + 2];
    const pxBadlandsG = sImg.data[((sh - 1) * sw) * 4 + 1];
    const pxDesertG = sImg.data[((sh - 1) * sw + (sw - 1)) * 4 + 1];
    expect(pxGlacierA).toBeGreaterThan(240);
    expect(pxBogB).toBeGreaterThan(220);
    expect(pxBadlandsG).toBeGreaterThan(150);
    expect(pxDesertG).toBeGreaterThan(230);
    splat.dispose();
  });

  it('builds visible high-contrast rocky cave mouths seated at centred world UVs', () => {
    const world = generateWorld('w3-cave-test', { size: 64, shape: 'island' });
    // Put high cliff plateau ONLY in the negative-X quadrant where the old uncentred UV bug clamped to u=0.
    world.elevation.fill(world.seaLevel * 0.5);
    for (let y = 8; y < 24; y++) {
      for (let x = 8; x < 24; x++) {
        world.elevation[y * world.size + x] = 0.8;
      }
    }
    world.caveX = new Float32Array([-16]); // cell x = 16 -> u = (16-32+32)/64 = 0.25 (inside high plateau!)
    world.caveZ = new Float32Array([-16]);
    world.caveE = new Float32Array([0.8]);
    world.caveYaw = new Float32Array([0]);
    world.caveCount = 1;

    const geom = buildCavesGeometry(world, 1200, 168, 64);
    expect(geom).not.toBeNull();
    if (!geom) return;
    geom.computeBoundingBox();
    const bb = geom.boundingBox!;
    const heightUnits = 168;
    // Cave must sit up at plateau height (~0.8 * 168 = 134.4), not clamped down near sea level.
    expect(bb.min.y).toBeGreaterThan(0.6 * heightUnits);
    expect(bb.max.x - bb.min.x).toBeGreaterThanOrEqual(3.2);
    expect(bb.max.y - bb.min.y).toBeGreaterThanOrEqual(2.8);

    const colAttr = geom.getAttribute('color');
    let minLuma = 1;
    let maxLuma = 0;
    for (let i = 0; i < colAttr.count; i++) {
      const lum = colAttr.getX(i) * 0.3 + colAttr.getY(i) * 0.59 + colAttr.getZ(i) * 0.11;
      minLuma = Math.min(minLuma, lum);
      maxLuma = Math.max(maxLuma, lum);
    }
    expect(minLuma).toBeLessThan(0.03);
    expect(maxLuma).toBeGreaterThan(0.18);
    geom.dispose();
  });

  it('renders WorldMinimap legend by default showing only biomes present in the world', () => {
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);
    const world = generateWorld('w3-minimap-legend', { size: 32, shape: 'island' });
    world.biome.fill(Biome.Desert);
    world.biome[0] = Biome.Ocean;
    world.biome[1] = Biome.Badlands;
    world.biome[2] = Biome.Glacier;
    world.riverAmt.fill(0);
    world.water.fill(0);

    const viewRef = {
      current: { targetX: 0, targetZ: 0, camX: 10, camZ: 10 } satisfies CameraView,
    };

    render(
      React.createElement(WorldMinimap, {
        world,
        renderSize: 1200,
        viewRef,
        onTeleport: () => {},
      }),
    );

    const legend = screen.getByTestId('minimap-biome-legend');
    expect(legend.textContent).toContain('Desert');
    expect(legend.textContent).toContain('Badlands');
    expect(legend.textContent).toContain('Glacier');
    expect(legend.textContent).toContain('Ocean');
    // Biomes absent from world must NOT appear in the compact minimap legend.
    expect(legend.textContent).not.toContain('Mangrove');
    expect(legend.textContent).not.toContain('Jungle');

    const toggle = screen.getByTestId('minimap-legend-toggle');
    fireEvent.click(toggle);
    expect(screen.queryByTestId('minimap-biome-legend')).toBeNull();
  });
});
