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

const FLORA_STYLE_MAP: Record<
  FloraType,
  { model: 'pine' | 'tree_broad' | 'shrub' | 'rock' | 'fern'; tint: string; keepNaturalColor: boolean }
> = {
  [FloraType.Pine]: { model: 'pine', tint: '#d8edd2', keepNaturalColor: true },
  [FloraType.Round]: { model: 'shrub', tint: '#5fa44e', keepNaturalColor: false },
  [FloraType.Jungle]: { model: 'shrub', tint: '#3f8a43', keepNaturalColor: false },
  [FloraType.Cactus]: { model: 'tree_broad', tint: '#5c8c4a', keepNaturalColor: false },
  [FloraType.Acacia]: { model: 'tree_broad', tint: '#8ca84a', keepNaturalColor: false },
  [FloraType.Palm]: { model: 'tree_broad', tint: '#54964f', keepNaturalColor: true },
  [FloraType.DeadTree]: { model: 'tree_broad', tint: '#7b6855', keepNaturalColor: false },
  [FloraType.Bush]: { model: 'shrub', tint: '#86a662', keepNaturalColor: true },
  [FloraType.Rock]: { model: 'rock', tint: '#b5b2ac', keepNaturalColor: true },
  [FloraType.Reed]: { model: 'fern', tint: '#9fb070', keepNaturalColor: true },
  [FloraType.Tuft]: { model: 'fern', tint: '#d4dfb2', keepNaturalColor: false },
  [FloraType.Coral]: { model: 'rock', tint: '#dcd3cc', keepNaturalColor: false },
  [FloraType.Kelp]: { model: 'fern', tint: '#597a42', keepNaturalColor: false },
  [FloraType.Seagrass]: { model: 'fern', tint: '#639a74', keepNaturalColor: false },
};

const pbrPromises = new Map<string, Promise<TerrainPbrArrays | null>>();
const floraPromises = new Map<string, Promise<FloraGeomEntry[] | null>>();

export function clearAssetCachesForTest(): void {
  pbrPromises.clear();
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
  const cacheKey = `${tileSize}:${tier}`;
  const existing = pbrPromises.get(cacheKey);
  if (existing) return existing;

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

      return {
        tileSize,
        albedoArray,
        normalArray,
        dispose: () => {
          albedoArray.dispose();
          normalArray.dispose();
        },
      };
    } catch {
      return null;
    }
  })();

  pbrPromises.set(cacheKey, task);
  return task;
}

interface TexturePixels {
  width: number;
  height: number;
  data: Uint8ClampedArray;
}

function readTexturePixels(texture: THREE.Texture | null | undefined): TexturePixels | null {
  if (!texture || typeof document === 'undefined') return null;
  const img = texture.image as CanvasImageSource & { width?: number; height?: number };
  if (!img || !img.width || !img.height) return null;
  const w = Math.min(128, Math.max(1, Math.trunc(img.width)));
  const h = Math.min(128, Math.max(1, Math.trunc(img.height)));
  const canvas = document.createElement('canvas');
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;
  ctx.drawImage(img, 0, 0, w, h);
  return { width: w, height: h, data: ctx.getImageData(0, 0, w, h).data };
}

function bakeBakedVertexColorGeometry(
  rawGeom: THREE.BufferGeometry,
  materialColor: THREE.Color,
  pixels: TexturePixels | null,
): THREE.BufferGeometry {
  const geom = rawGeom.index ? rawGeom.toNonIndexed() : rawGeom.clone();
  const posAttr = geom.getAttribute('position');
  const normAttr = geom.getAttribute('normal');
  const uvAttr = geom.getAttribute('uv');
  if (!posAttr) return geom;

  if (!normAttr) geom.computeVertexNormals();
  const finalNorm = geom.getAttribute('normal');
  const vCount = posAttr.count;
  const colors = new Float32Array(vCount * 3);
  const cTmp = new THREE.Color();

  for (let i = 0; i < vCount; i++) {
    let r = materialColor.r;
    let g = materialColor.g;
    let b = materialColor.b;
    if (pixels && uvAttr) {
      const u = ((uvAttr.getX(i) % 1) + 1) % 1;
      const v = ((1 - (uvAttr.getY(i) % 1)) + 1) % 1;
      const px = Math.min(pixels.width - 1, Math.max(0, Math.floor(u * pixels.width)));
      const py = Math.min(pixels.height - 1, Math.max(0, Math.floor(v * pixels.height)));
      const off = (py * pixels.width + px) * 4;
      cTmp.setRGB(
        pixels.data[off] / 255,
        pixels.data[off + 1] / 255,
        pixels.data[off + 2] / 255,
        THREE.SRGBColorSpace,
      );
      // Slight brightness lift avoids crushing dark green/bark albedo under foliage lighting.
      r *= Math.min(1.0, cTmp.r * 1.18 + 0.04);
      g *= Math.min(1.0, cTmp.g * 1.18 + 0.04);
      b *= Math.min(1.0, cTmp.b * 1.18 + 0.04);
    }
    colors[i * 3] = Math.min(1, r);
    colors[i * 3 + 1] = Math.min(1, g);
    colors[i * 3 + 2] = Math.min(1, b);
  }

  const out = new THREE.BufferGeometry();
  out.setAttribute('position', posAttr.clone());
  out.setAttribute('normal', (finalNorm ?? geom.getAttribute('normal')).clone());
  out.setAttribute('color', new THREE.BufferAttribute(colors, 3));
  geom.dispose();
  return out;
}

type MergeGeometriesFn = (
  geometries: THREE.BufferGeometry[],
  useGroups?: boolean,
) => THREE.BufferGeometry | null;

function extractGlbBaseGeometry(
  scene: THREE.Object3D,
  slug: 'pine' | 'tree_broad' | 'shrub' | 'rock' | 'fern',
  mergeFn: MergeGeometriesFn,
): THREE.BufferGeometry | null {
  scene.updateMatrixWorld(true);
  const parts: THREE.BufferGeometry[] = [];
  const allMeshes: THREE.Mesh[] = [];
  scene.traverse((obj) => {
    if (obj instanceof THREE.Mesh && obj.geometry) {
      allMeshes.push(obj);
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
    const pixels = readTexturePixels(std?.map);
    const cloned = mesh.geometry.clone();
    cloned.applyMatrix4(mesh.matrixWorld);
    parts.push(bakeBakedVertexColorGeometry(cloned, color, pixels));
    cloned.dispose();
  }

  const merged = parts.length === 1 ? parts[0] : mergeFn(parts, false);
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
      if (keepNaturalColor) {
        col.setXYZ(i, col.getX(i) * tint.r, col.getY(i) * tint.g, col.getZ(i) * tint.b);
      } else {
        // Preserve grayscale light/bark shading while applying biome/species colour identity.
        const lum = 0.45 + 0.55 * (col.getX(i) * 0.3 + col.getY(i) * 0.59 + col.getZ(i) * 0.11);
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
  if (tier === 'low' || typeof fetch !== 'function' || typeof document === 'undefined') {
    return Promise.resolve(null);
  }
  const cacheKey = `${tier}:${types.join(',')}`;
  const existing = floraPromises.get(cacheKey);
  if (existing) return existing;

  const task = (async (): Promise<FloraGeomEntry[] | null> => {
    try {
      const [{ GLTFLoader }, { mergeGeometries }] = await Promise.all([
        import('three/examples/jsm/loaders/GLTFLoader.js'),
        import('three/examples/jsm/utils/BufferGeometryUtils.js'),
      ]);
      const loader = new GLTFLoader();
      const slugs: Array<'pine' | 'tree_broad' | 'shrub' | 'rock' | 'fern'> = [
        'pine',
        'tree_broad',
        'shrub',
        'rock',
        'fern',
      ];
      const baseBySlug = new Map<string, THREE.BufferGeometry>();
      await Promise.all(
        slugs.map(async (slug) => {
          const gltf = await loader.loadAsync(`/assets/world/models/${slug}.glb`);
          const extracted = extractGlbBaseGeometry(gltf.scene, slug, mergeGeometries);
          if (extracted) baseBySlug.set(slug, extracted);
        }),
      );

      if (baseBySlug.size === 0) return null;

      const out: FloraGeomEntry[] = types.map((type) => {
        const spec = FLORA_STYLE_MAP[type];
        const base = baseBySlug.get(spec.model);
        if (!base) {
          return { type, geometry: makeFloraGeometry(type) };
        }
        return {
          type,
          geometry: adaptModelToFloraType(base, type, spec.tint, spec.keepNaturalColor),
        };
      });

      baseBySlug.forEach((g) => g.dispose());
      return out;
    } catch {
      return null;
    }
  })();

  floraPromises.set(cacheKey, task);
  return task;
}
