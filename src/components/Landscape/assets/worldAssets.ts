import * as THREE from 'three';
import { makeFloraGeometry } from '../utils/floraGeometry';
import { FloraType } from '../utils/worldGen';
import type { QualityTier } from '../utils/qualityTiers';

// prototype, to be replaced — progressive 'self'-hosted CC0-1.0 asset loader for W3-lite.

export interface TerrainPbrArrays {
  readonly tileSize: number;
  readonly albedoArray: THREE.DataArrayTexture;
  readonly normalArray: THREE.DataArrayTexture;
  dispose(): void;
}

export interface FloraGeomEntry {
  type: FloraType;
  geometry: THREE.BufferGeometry;
}

const TERRAIN_LAYERS = ['grass', 'forest', 'rock', 'sand', 'snow', 'mud'] as const;

// Only map species whose silhouette truly matches our 5 Poly Haven CC0 low-poly models.
// Unmapped species (Cactus, Acacia, Palm, DeadTree, Tuft, Coral, Kelp, Seagrass) stay on
// their silhouette-faithful procedural geometry so palm fronds, cacti, grass tufts, and reefs
// keep accurate shapes and high-tier triangle counts stay comfortably inside §4a budgets.
const FLORA_STYLE_MAP: Partial<
  Record<
    FloraType,
    {
      model: 'pine' | 'tree_broad' | 'shrub' | 'rock' | 'fern';
      tint: string;
      keepNaturalColor: boolean;
      hasWoodyTrunk?: boolean;
    }
  >
> = {
  [FloraType.Pine]: { model: 'pine', tint: '#f3faef', keepNaturalColor: true },
  [FloraType.Round]: { model: 'tree_broad', tint: '#57a846', keepNaturalColor: false, hasWoodyTrunk: true },
  [FloraType.Jungle]: { model: 'tree_broad', tint: '#2d8c3c', keepNaturalColor: false, hasWoodyTrunk: true },
  [FloraType.Bush]: { model: 'shrub', tint: '#f2f7ec', keepNaturalColor: true },
  [FloraType.Rock]: { model: 'rock', tint: '#f2f0ec', keepNaturalColor: true },
  [FloraType.Reed]: { model: 'fern', tint: '#e4edd0', keepNaturalColor: true },
};

let activePbrTileSize: number | null = null;
let pbrLoadGeneration = 0;
let activeFloraTier: QualityTier | null = null;
const pbrPromises = new Map<number, Promise<TerrainPbrArrays | null>>();
const pbrResolved = new Map<number, TerrainPbrArrays>();
const floraPromises = new Map<string, Promise<FloraGeomEntry[] | null>>();
const floraResolved = new Map<string, FloraGeomEntry[]>();

function disposeTerrainPbrTileSize(tileSize: number): void {
  pbrPromises.delete(tileSize);
  const resolved = pbrResolved.get(tileSize);
  if (resolved) {
    pbrResolved.delete(tileSize);
    resolved.dispose();
  }
}

/** Dispose cached terrain PBR array textures for tiers other than `activeTier` (N5). */
export function disposeOtherTerrainPbrTiers(activeTier: QualityTier): void {
  const keepSize = activeTier === 'low' ? 512 : 1024;
  if (activePbrTileSize !== keepSize) {
    pbrLoadGeneration++;
  }
  activePbrTileSize = keepSize;
  for (const size of new Set([...pbrPromises.keys(), ...pbrResolved.keys()])) {
    if (size !== keepSize) disposeTerrainPbrTileSize(size);
  }
}

/** Dispose cached CC0 flora geometries (for example when Auto drops to `low`). */
export function disposeFloraAssetGeometries(): void {
  activeFloraTier = 'low';
  floraPromises.clear();
  for (const entries of floraResolved.values()) {
    for (const entry of entries) entry.geometry.dispose();
  }
  floraResolved.clear();
}

function isFloraTierLow(): boolean {
  return activeFloraTier === 'low';
}

export function clearAssetCachesForTest(): void {
  pbrLoadGeneration++;
  activePbrTileSize = null;
  activeFloraTier = null;
  for (const resolved of pbrResolved.values()) resolved.dispose();
  pbrResolved.clear();
  pbrPromises.clear();
  for (const entries of floraResolved.values()) {
    for (const entry of entries) entry.geometry.dispose();
  }
  floraResolved.clear();
  floraPromises.clear();
}

async function decodeImageToRgba(url: string, targetSize: number): Promise<Uint8Array | null> {
  if (typeof fetch !== 'function' || typeof document === 'undefined') return null;
  const resp = await fetch(url);
  if (!resp.ok) return null;
  const blob = await resp.blob();
  const canvas = document.createElement('canvas');
  canvas.width = targetSize;
  canvas.height = targetSize;
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;

  if (typeof createImageBitmap === 'function') {
    const bmp = await createImageBitmap(blob);
    try {
      ctx.drawImage(bmp, 0, 0, targetSize, targetSize);
    } finally {
      bmp.close();
    }
  } else {
    const imgUrl = URL.createObjectURL(blob);
    try {
      const img = await new Promise<HTMLImageElement>((resolve, reject) => {
        const el = new Image();
        el.onload = () => resolve(el);
        el.onerror = () => reject(new Error(`Failed to decode ${url}`));
        el.src = imgUrl;
      });
      ctx.drawImage(img, 0, 0, targetSize, targetSize);
    } finally {
      URL.revokeObjectURL(imgUrl);
    }
  }

  return new Uint8Array(ctx.getImageData(0, 0, targetSize, targetSize).data.buffer);
}

/**
 * Load 6 CC0 terrain PBR layers into GPU DataArrayTextures (`512` on Low, `1024` on Medium/High).
 * Returns `null` on missing assets or non-browser environments so procedural terrain remains intact.
 */
export function loadTerrainPbrArrays(tier: QualityTier): Promise<TerrainPbrArrays | null> {
  const tileSize = tier === 'low' ? 512 : 1024;
  // Dispose any other tier's resident/in-flight array set before loading this tier (N5).
  disposeOtherTerrainPbrTiers(tier);

  const existing = pbrPromises.get(tileSize);
  if (existing) return existing;

  const myGeneration = ++pbrLoadGeneration;
  const task = (async (): Promise<TerrainPbrArrays | null> => {
    try {
      const layerCount = TERRAIN_LAYERS.length;
      const albedoBuf = new Uint8Array(tileSize * tileSize * 4 * layerCount);
      const normalBuf = new Uint8Array(tileSize * tileSize * 4 * layerCount);
      const layerBytes = tileSize * tileSize * 4;

      const jobs = TERRAIN_LAYERS.map(async (slug, idx) => {
        const [diff, nor] = await Promise.all([
          decodeImageToRgba(`/assets/world/textures/${slug}_diff.webp`, tileSize),
          decodeImageToRgba(`/assets/world/textures/${slug}_nor.webp`, tileSize),
        ]);
        if (!diff || !nor) throw new Error(`Terrain texture missing for ${slug}`);
        albedoBuf.set(diff, idx * layerBytes);
        normalBuf.set(nor, idx * layerBytes);
      });
      await Promise.all(jobs);

      // Keep `RGBA8` (`NoColorSpace`) on the DataArrayTexture so WebGL2 `gl.generateMipmap(TEXTURE_2D_ARRAY)`
      // is valid without `SRGB8_ALPHA8` completeness errors; the shader applies sRGB -> linear decode.
      const albedoArray = new THREE.DataArrayTexture(albedoBuf, tileSize, tileSize, layerCount);
      albedoArray.format = THREE.RGBAFormat;
      albedoArray.type = THREE.UnsignedByteType;
      albedoArray.colorSpace = THREE.NoColorSpace;
      albedoArray.wrapS = THREE.RepeatWrapping;
      albedoArray.wrapT = THREE.RepeatWrapping;
      albedoArray.minFilter = THREE.LinearMipmapLinearFilter;
      albedoArray.magFilter = THREE.LinearFilter;
      albedoArray.generateMipmaps = true;
      albedoArray.anisotropy = tier === 'low' ? 1 : 4;
      albedoArray.needsUpdate = true;

      const normalArray = new THREE.DataArrayTexture(normalBuf, tileSize, tileSize, layerCount);
      normalArray.format = THREE.RGBAFormat;
      normalArray.type = THREE.UnsignedByteType;
      normalArray.colorSpace = THREE.NoColorSpace;
      normalArray.wrapS = THREE.RepeatWrapping;
      normalArray.wrapT = THREE.RepeatWrapping;
      normalArray.minFilter = THREE.LinearMipmapLinearFilter;
      normalArray.magFilter = THREE.LinearFilter;
      normalArray.generateMipmaps = true;
      normalArray.anisotropy = tier === 'low' ? 1 : 4;
      normalArray.needsUpdate = true;

      const arrays: TerrainPbrArrays = {
        tileSize,
        albedoArray,
        normalArray,
        dispose: () => {
          albedoArray.dispose();
          normalArray.dispose();
        },
      };

      // If quality changed or a newer load wave superseded this request while WebP decodes were in flight,
      // dispose only this request's own arrays and leave the newer cached entry untouched (NR3).
      if (myGeneration !== pbrLoadGeneration || activePbrTileSize !== tileSize) {
        arrays.dispose();
        if (myGeneration === pbrLoadGeneration) {
          pbrPromises.delete(tileSize);
        }
        return null;
      }
      const prev = pbrResolved.get(tileSize);
      if (prev && prev !== arrays) {
        prev.dispose();
      }
      pbrResolved.set(tileSize, arrays);
      return arrays;
    } catch {
      if (myGeneration === pbrLoadGeneration) {
        pbrPromises.delete(tileSize);
      }
      return null;
    }
  })();

  pbrPromises.set(tileSize, task);
  return task;
}

function bakeBakedVertexColorGeometry(
  rawGeom: THREE.BufferGeometry,
  materialColor: THREE.Color,
): THREE.BufferGeometry {
  const geom = rawGeom.index ? rawGeom.toNonIndexed() : rawGeom.clone();
  const posAttr = geom.getAttribute('position');
  const normAttr = geom.getAttribute('normal');
  const colAttr = geom.getAttribute('color');
  if (!posAttr) return geom;

  if (!normAttr) geom.computeVertexNormals();
  const finalNorm = geom.getAttribute('normal');
  const vCount = posAttr.count;
  const colors = new Float32Array(vCount * 3);

  for (let i = 0; i < vCount; i++) {
    const r = colAttr ? colAttr.getX(i) : materialColor.r;
    const g = colAttr ? colAttr.getY(i) : materialColor.g;
    const b = colAttr ? colAttr.getZ(i) : materialColor.b;
    colors[i * 3] = Math.max(0, Math.min(1, r));
    colors[i * 3 + 1] = Math.max(0, Math.min(1, g));
    colors[i * 3 + 2] = Math.max(0, Math.min(1, b));
  }

  const out = new THREE.BufferGeometry();
  out.setAttribute('position', posAttr.clone());
  out.setAttribute('normal', (finalNorm ?? geom.getAttribute('normal')).clone());
  out.setAttribute('color', new THREE.BufferAttribute(colors, 3));
  geom.dispose();
  return out;
}

function concatNonIndexedGeometries(parts: THREE.BufferGeometry[]): THREE.BufferGeometry | null {
  if (parts.length === 0) return null;
  if (parts.length === 1) return parts[0];
  let totalVerts = 0;
  for (const p of parts) {
    const pos = p.getAttribute('position');
    if (pos) totalVerts += pos.count;
  }
  if (totalVerts === 0) return null;

  const posOut = new Float32Array(totalVerts * 3);
  const nrmOut = new Float32Array(totalVerts * 3);
  const colOut = new Float32Array(totalVerts * 3);
  let cursor = 0;
  for (const p of parts) {
    const pos = p.getAttribute('position');
    if (!pos) continue;
    const nrm = p.getAttribute('normal');
    const col = p.getAttribute('color');
    const n = pos.count;
    for (let i = 0; i < n; i++) {
      const o = (cursor + i) * 3;
      posOut[o] = pos.getX(i);
      posOut[o + 1] = pos.getY(i);
      posOut[o + 2] = pos.getZ(i);
      if (nrm) {
        nrmOut[o] = nrm.getX(i);
        nrmOut[o + 1] = nrm.getY(i);
        nrmOut[o + 2] = nrm.getZ(i);
      } else {
        nrmOut[o + 1] = 1;
      }
      if (col) {
        colOut[o] = col.getX(i);
        colOut[o + 1] = col.getY(i);
        colOut[o + 2] = col.getZ(i);
      } else {
        colOut[o] = 0.5;
        colOut[o + 1] = 0.5;
        colOut[o + 2] = 0.5;
      }
    }
    cursor += n;
  }

  const merged = new THREE.BufferGeometry();
  merged.setAttribute('position', new THREE.BufferAttribute(posOut, 3));
  merged.setAttribute('normal', new THREE.BufferAttribute(nrmOut, 3));
  merged.setAttribute('color', new THREE.BufferAttribute(colOut, 3));
  return merged;
}

function extractGlbBaseGeometry(
  scene: THREE.Object3D,
  slug: 'pine' | 'tree_broad' | 'shrub' | 'rock' | 'fern',
): THREE.BufferGeometry | null {
  scene.updateMatrixWorld(true);
  const parts: THREE.BufferGeometry[] = [];
  const allMeshes: THREE.Mesh[] = [];
  scene.traverse((obj) => {
    const maybeMesh = obj as THREE.Mesh;
    if (maybeMesh.isMesh && maybeMesh.geometry) {
      allMeshes.push(maybeMesh);
    }
  });
  if (allMeshes.length === 0) return null;

  // Poly Haven variant packs (`pine_sapling_small`, `fern_02`) bundle multiple side-by-side variants
  // in one scene; keep the primary centered variant so instances stay centered at origin.
  const selectedMeshes =
    slug === 'pine' || slug === 'fern' ? [allMeshes[0]] : allMeshes;

  for (const mesh of selectedMeshes) {
    const mat = Array.isArray(mesh.material) ? mesh.material[0] : mesh.material;
    const std = mat as THREE.MeshStandardMaterial | undefined;
    const color = std?.color ? std.color.clone() : new THREE.Color(1, 1, 1);
    const cloned = mesh.geometry.clone();
    cloned.applyMatrix4(mesh.matrixWorld);
    parts.push(bakeBakedVertexColorGeometry(cloned, color));
    cloned.dispose();
  }

  const merged = concatNonIndexedGeometries(parts);
  if (!merged) {
    parts.forEach((p) => p.dispose());
    return null;
  }
  if (parts.length > 1) parts.forEach((p) => p.dispose());

  // Normalize bottom y = 0 and center XZ around 0.
  merged.computeBoundingBox();
  const bb = merged.boundingBox;
  if (bb) {
    const cx = (bb.min.x + bb.max.x) * 0.5;
    const cz = (bb.min.z + bb.max.z) * 0.5;
    merged.translate(-cx, -bb.min.y, -cz);
  }
  merged.computeBoundingBox();
  return merged;
}

function adaptModelToFloraType(
  baseGeom: THREE.BufferGeometry,
  type: FloraType,
  tintHex: string,
  keepNaturalColor: boolean,
  hasWoodyTrunk = false,
): THREE.BufferGeometry {
  const ref = makeFloraGeometry(type);
  ref.computeBoundingBox();
  const refBox = ref.boundingBox;
  const refH = refBox ? Math.max(0.2, refBox.max.y - refBox.min.y) : 1;
  const refW = refBox
    ? Math.max(0.2, refBox.max.x - refBox.min.x, refBox.max.z - refBox.min.z)
    : 1;
  ref.dispose();

  baseGeom.computeBoundingBox();
  const srcBox = baseGeom.boundingBox;
  const srcH = srcBox ? Math.max(0.05, srcBox.max.y - srcBox.min.y) : 1;
  const srcW = srcBox
    ? Math.max(0.05, srcBox.max.x - srcBox.min.x, srcBox.max.z - srcBox.min.z)
    : 1;

  const sy = refH / srcH;
  const sxz = Math.min(refW / srcW, sy * 1.45);
  const geom = baseGeom.clone();
  geom.scale(sxz, sy, sxz);
  geom.computeBoundingBox();
  if (geom.boundingBox) {
    geom.translate(0, -geom.boundingBox.min.y, 0);
  }

  const tint = new THREE.Color(tintHex);
  const col = geom.getAttribute('color') as THREE.BufferAttribute | undefined;
  if (col) {
    for (let i = 0; i < col.count; i++) {
      const rx = col.getX(i);
      const gx = col.getY(i);
      const bx = col.getZ(i);
      const isBark = hasWoodyTrunk && rx >= gx * 1.05 && rx > 0.08;
      if (isBark) {
        col.setXYZ(i, Math.min(1, rx * 1.12), Math.min(1, gx * 1.06), Math.min(1, bx * 1.02));
      } else if (keepNaturalColor) {
        col.setXYZ(i, Math.min(1, rx * tint.r), Math.min(1, gx * tint.g), Math.min(1, bx * tint.b));
      } else {
        // Preserve sunlit light/canopy shading while applying species colour identity.
        const origLum = rx * 0.3 + gx * 0.59 + bx * 0.11;
        const lum = 0.58 + 0.42 * origLum;
        col.setXYZ(
          i,
          Math.min(1, tint.r * lum),
          Math.min(1, tint.g * lum),
          Math.min(1, tint.b * lum),
        );
      }
    }
    col.needsUpdate = true;
  }

  geom.computeBoundingBox();
  geom.computeBoundingSphere();
  return geom;
}

/**
 * Progressively load & normalize CC0 3D flora models for all 14 existing FloraTypes.
 * Keeps `low` tier on the lightweight procedural flora geometries (`null`), falling back cleanly
 * on any load/parse error so flora is never missing.
 */
export function loadFloraAssetGeometries(
  types: readonly FloraType[],
  tier: QualityTier,
): Promise<FloraGeomEntry[] | null> {
  if (tier === 'low' || typeof fetch !== 'function') {
    return Promise.resolve(null);
  }
  activeFloraTier = tier;
  const cacheKey = `${tier}:${types.join(',')}`;
  const existing = floraPromises.get(cacheKey);
  if (existing) return existing;

  const task = (async (): Promise<FloraGeomEntry[] | null> => {
    try {
      const { GLTFLoader } = await import('three/examples/jsm/loaders/GLTFLoader.js');
      const loader = new GLTFLoader();
      const neededSlugs = Array.from(
        new Set(
          types
            .map((t) => FLORA_STYLE_MAP[t]?.model)
            .filter((m): m is 'pine' | 'tree_broad' | 'shrub' | 'rock' | 'fern' => m !== undefined),
        ),
      );
      if (neededSlugs.length === 0) return null;

      const baseBySlug = new Map<string, THREE.BufferGeometry>();
      await Promise.all(
        neededSlugs.map(async (slug) => {
          // Route through global `fetch` + `GLTFLoader.parseAsync` so missing/404 GLBs use the
          // same HTTP path in both browsers and unit tests without DOM dependencies.
          const resp = await fetch(`/assets/world/models/${slug}.glb`);
          if (!resp.ok) throw new Error(`Flora GLB missing: ${slug} (${resp.status})`);
          const buf = await resp.arrayBuffer();
          const gltf = await loader.parseAsync(buf, '');
          const extracted = extractGlbBaseGeometry(gltf.scene, slug);
          if (extracted) baseBySlug.set(slug, extracted);
        }),
      );

      if (baseBySlug.size === 0) return null;

      const out: FloraGeomEntry[] = types.map((type) => {
        const spec = FLORA_STYLE_MAP[type];
        if (!spec) {
          return { type, geometry: makeFloraGeometry(type) };
        }
        const base = baseBySlug.get(spec.model);
        if (!base) {
          return { type, geometry: makeFloraGeometry(type) };
        }
        return {
          type,
          geometry: adaptModelToFloraType(
            base,
            type,
            spec.tint,
            spec.keepNaturalColor,
            Boolean(spec.hasWoodyTrunk),
          ),
        };
      });

      baseBySlug.forEach((g) => g.dispose());
      if (isFloraTierLow()) {
        out.forEach((entry) => entry.geometry.dispose());
        floraPromises.delete(cacheKey);
        return null;
      }
      floraResolved.set(cacheKey, out);
      return out;
    } catch {
      floraPromises.delete(cacheKey);
      return null;
    }
  })();

  floraPromises.set(cacheKey, task);
  return task;
}
