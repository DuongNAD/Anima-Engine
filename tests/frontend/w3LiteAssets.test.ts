import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it, vi, afterEach } from 'vitest';
import * as THREE from 'three';
import { generateWorld, Biome } from '../../src/components/Landscape/utils/worldGen';
import { buildSplatTexture } from '../../src/components/Landscape/utils/worldTerrainTextures';
import {
  clearAssetCachesForTest,
  loadFloraAssetGeometries,
  loadTerrainPbrArrays,
} from '../../src/components/Landscape/assets/worldAssets';
import { createTerrainPbrMaterial } from '../../src/components/Landscape/materials/terrainPbrMaterial';
import { createFloraWindMaterial } from '../../src/components/Landscape/materials/floraWindMaterial';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '../..');
const WORLD_ASSETS_DIR = resolve(ROOT, 'public/assets/world');

const ALLOWED_SITES = new Set(['polyhaven.com', 'ambientcg.com', 'quaternius.com']);
const MAX_COMMITTED_BYTES = 20 * 1024 * 1024;

interface SourceEntry {
  file: string;
  sourceSite: string;
  assetId: string;
  assetName: string;
  sourceUrl: string;
  license: string;
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
      expect(ALLOWED_SITES.has(entry.sourceSite)).toBe(true);
      expect(entry.sourceUrl).toMatch(/^https:\/\//);
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

  it('falls back cleanly to null when CC0 textures or models fail to load', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response(null, { status: 404 }));

    const pbr = await loadTerrainPbrArrays('high');
    expect(pbr).toBeNull();

    const floraLow = await loadFloraAssetGeometries([0], 'low');
    expect(floraLow).toBeNull();

    const floraHigh = await loadFloraAssetGeometries([0], 'high');
    expect(floraHigh).toBeNull();
  });

  it('compiles terrain PBR and flora wind onBeforeCompile hooks cleanly', () => {
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
      uniforms: {} as Record<string, { value: unknown }>,
      vertexShader: 'void main() {\n#include <worldpos_vertex>\n}',
      fragmentShader: 'void main() {\n#include <map_fragment>\n#include <normal_fragment_maps>\n}',
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
      uniforms: {} as Record<string, { value: unknown }>,
      vertexShader: 'void main() {\n#include <begin_vertex>\n}',
      fragmentShader: 'void main() {}',
    };
    floraBundle.material.onBeforeCompile(
      fShader as unknown as THREE.WebGLProgramParametersWithUniforms,
      {} as THREE.WebGLRenderer,
    );
    expect(fShader.uniforms.uWindTime.value).toBeCloseTo(2.5, 5);
    expect(fShader.vertexShader).toContain('instanceMatrix');
    floraBundle.dispose();
  });
});
