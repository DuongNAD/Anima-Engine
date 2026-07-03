import React, { useEffect, useMemo } from 'react';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { Biome, BIOME_RGB } from './utils/worldGen';
import { sampleElevation, sampleMeshHeight } from './utils/worldSample';

export interface WorldTerrainProps {
  world: World;
  /** World-space width the terrain is rendered at, regardless of data resolution. */
  renderSize?: number;
  /** Peak height as a fraction of renderSize. */
  heightRatio?: number;
  /** Mesh resolution (segments per side). Decoupled from the data resolution. */
  meshResolution?: number;
}

/** Deterministic per-cell hash in [0, 1) — cheap brightness jitter, stable across runs. */
function hash01(x: number, y: number): number {
  let h = Math.imul(x, 374761393) + Math.imul(y, 668265263);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

/**
 * Bakes the world's biome colours into a FULL data-resolution sRGB texture. The mesh only has
 * ~384^2 vertices, but the texture keeps all 2048^2 cells, so biome borders, rivers and beaches
 * stay crisp no matter how close the camera gets — and the GPU's bilinear filter blends the
 * border texels into organic transitions for free. A subtle per-cell brightness jitter breaks
 * up the dead-flat look of wide single-biome fields. Palette = BIOME_RGB, same as the minimap.
 */
function buildColorTexture(world: World): THREE.DataTexture {
  const { size, biome, riverAmt, shore, elevation, slope, water, temperature } = world;
  const data = new Uint8Array(size * size * 4);
  const RIVER_RGB = BIOME_RGB[Biome.River];
  // Curvature AO ring radii (world-space constant regardless of data resolution).
  const R1 = Math.max(2, Math.round(size / 680));
  const R2 = Math.max(6, Math.round(size / 228));
  for (let y = 0; y < size; y++) {
    const y1a = Math.max(0, y - R1) * size;
    const y1b = Math.min(size - 1, y + R1) * size;
    const y2a = Math.max(0, y - R2) * size;
    const y2b = Math.min(size - 1, y + R2) * size;
    for (let x = 0; x < size; x++) {
      const i = y * size + x;
      const bio = biome[i];
      let [r, g, b] = BIOME_RGB[bio] ?? [255, 0, 255];

      // Frozen basins: the lakebed under (and peeking around) an ice sheet is pale ice,
      // not liquid-lake navy — matches the opaque ice mesh WorldWater lays on top.
      if ((water?.[i] ?? 0) > 0 && (temperature?.[i] ?? 0.5) < 0.19) {
        r = 213;
        g = 231;
        b = 241;
      }

      // Rivers & streams: the generator's ribbon mask (widens downstream, soft-feathered
      // banks). The gradient tint gives smooth tapering threads instead of jagged 1-cell
      // zigzags, and the centre darkens a touch so big rivers read deeper.
      const amt = riverAmt?.[i] ?? 0;
      if (amt > 0 && bio !== Biome.Ocean && bio !== Biome.Lake && bio !== Biome.Glacier && bio !== Biome.Snow) {
        const t = Math.min(1, (amt / 255) * 1.08);
        r = r * (1 - t) + RIVER_RGB[0] * t;
        g = g * (1 - t) + RIVER_RGB[1] * t;
        b = b * (1 - t) + RIVER_RGB[2] * t;
        const deep = 1 - 0.1 * t;
        r *= deep;
        g *= deep;
        b *= deep;
      }

      // Wet sand: darken the beach right at the waterline (the swash zone).
      if (bio === Biome.Beach && (shore?.[i] ?? 0) > 0.72) {
        const t = 1 - 0.14 * ((shore[i] - 0.72) / 0.28);
        r *= t;
        g *= t;
        b *= t;
      }

      // Sedimentary strata: faint horizontal banding across steep bare-rock faces, keyed to
      // absolute elevation so the layers line up across a whole cliff wall.
      const e = elevation[i];
      if ((bio === Biome.Rock || bio === Biome.Badlands) && (slope?.[i] ?? 0) > 0.45) {
        const band = Math.sin(e * 320.0) * (bio === Biome.Badlands ? 0.075 : 0.05);
        r *= 1 + band;
        g *= 1 + band;
        b *= 1 + band;
      }

      // Curvature ambient occlusion, baked: ravines/gorges/basins shade, crests lift a touch.
      // Sun-independent (like real sky occlusion), so it layers cleanly under the dynamic
      // lighting and gives the relief its depth even at noon.
      {
        const xa1 = Math.max(0, x - R1);
        const xb1 = Math.min(size - 1, x + R1);
        const xa2 = Math.max(0, x - R2);
        const xb2 = Math.min(size - 1, x + R2);
        const ring1 = (elevation[y * size + xa1] + elevation[y * size + xb1] + elevation[y1a + x] + elevation[y1b + x]) * 0.25;
        const ring2 = (elevation[y * size + xa2] + elevation[y * size + xb2] + elevation[y2a + x] + elevation[y2b + x]) * 0.25;
        // Gentle: strong enough to seat ravines and river beds, not enough to dim whole
        // lowland basins (a broad valley is open to the sky, not occluded).
        let occ = Math.min(0.22, Math.max(0, ring1 - e) * 22 + Math.max(0, ring2 - e) * 10);
        if (e < (world.seaLevel ?? 0.38)) occ *= 0.5; // keep the sunny turquoise shallows
        const lift = Math.min(0.08, Math.max(0, e - ring2) * 8);
        const shade = 1 - occ + lift;
        r *= shade;
        g *= shade;
        b *= shade;
      }

      const f = 0.95 + hash01(x, y) * 0.09; // ±~4.5% brightness variation
      const o = i * 4;
      data[o] = Math.min(255, r * f);
      data[o + 1] = Math.min(255, g * f);
      data[o + 2] = Math.min(255, b * f);
      data[o + 3] = 255;
    }
  }
  const tex = new THREE.DataTexture(data, size, size, THREE.RGBAFormat, THREE.UnsignedByteType);
  tex.colorSpace = THREE.SRGBColorSpace;
  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.magFilter = THREE.LinearFilter;
  tex.minFilter = THREE.LinearMipmapLinearFilter;
  tex.generateMipmaps = true; // 2048 is power-of-two; stops distant shimmer
  tex.anisotropy = 8; // terrain is mostly seen at grazing angles
  tex.needsUpdate = true;
  return tex;
}

/**
 * Bakes a tangent-space normal map from the RESIDUAL relief — the full-resolution elevation
 * minus what the coarse render mesh already shows. The vertex normals light the broad
 * mountainsides; this map adds back the sub-mesh detail (erosion grooves, ridgelines, rough
 * badlands) without double-counting the large shapes. Follows the same uv mapping as the
 * colour texture / water heightmap: texel (x, y) = data cell (x, y).
 */
function buildNormalTexture(
  world: World,
  renderSize: number,
  heightRatio: number,
  meshResolution: number,
): THREE.DataTexture {
  // Half the data resolution is plenty for a DETAIL map (it still carries ~2.7x more relief
  // than the render mesh), quarters the GPU memory and makes the bake ~4x faster.
  const res = Math.max(512, world.size >> 1);
  const heightUnits = renderSize * heightRatio;
  const cellW = renderSize / (res - 1);

  // Residual world-space height (full detail minus the mesh's bilinear surface).
  const resid = new Float32Array(res * res);
  const inv = 1 / (res - 1);
  for (let y = 0; y < res; y++) {
    for (let x = 0; x < res; x++) {
      const u = x * inv;
      const v = y * inv;
      resid[y * res + x] =
        (sampleElevation(world, u, v) - sampleMeshHeight(world, u, v, meshResolution)) * heightUnits;
    }
  }

  const data = new Uint8Array(res * res * 4);
  for (let y = 0; y < res; y++) {
    const y0 = Math.max(0, y - 1) * res;
    const y1 = Math.min(res - 1, y + 1) * res;
    for (let x = 0; x < res; x++) {
      const x0 = Math.max(0, x - 1);
      const x1 = Math.min(res - 1, x + 1);
      const dhdx = (resid[y * res + x1] - resid[y * res + x0]) / ((x1 - x0) * cellW || cellW);
      const dhdz = (resid[y1 + x] - resid[y0 + x]) / (((y1 - y0) / res) * cellW || cellW);
      // Tangent space of a +Y-up heightfield with uv aligned to world XZ.
      const nx = -dhdx;
      const nz = -dhdz;
      const len = Math.sqrt(nx * nx + nz * nz + 1);
      const o = (y * res + x) * 4;
      data[o] = ((nx / len) * 0.5 + 0.5) * 255;
      data[o + 1] = ((nz / len) * 0.5 + 0.5) * 255;
      data[o + 2] = ((1 / len) * 0.5 + 0.5) * 255;
      data[o + 3] = 255;
    }
  }
  const tex = new THREE.DataTexture(data, res, res, THREE.RGBAFormat, THREE.UnsignedByteType);
  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.magFilter = THREE.LinearFilter;
  tex.minFilter = THREE.LinearMipmapLinearFilter;
  tex.generateMipmaps = true;
  tex.anisotropy = 8;
  tex.needsUpdate = true;
  return tex;
}

/**
 * Roughness map (read from the GREEN channel): water is glossy, land is matte. This is what
 * makes rivers and lakes catch the sun and glint as it moves — the single biggest "this is
 * real water" cue — for the cost of one small texture.
 */
function buildRoughnessTexture(world: World): THREE.DataTexture {
  const { size, biome, water, riverAmt, shore, elevation, seaLevel } = world;
  const res = Math.max(512, size >> 1);
  const scale = size / res;
  const data = new Uint8Array(res * res * 4);
  for (let y = 0; y < res; y++) {
    for (let x = 0; x < res; x++) {
      const i = Math.min(size - 1, Math.round(y * scale)) * size + Math.min(size - 1, Math.round(x * scale));
      let rough = 242; // matte land (0.95)
      const bio = biome[i];
      if (bio === Biome.Lake || bio === Biome.River || (water?.[i] ?? 0) > 0) {
        rough = 84; // open inland water (0.33)
      } else if ((riverAmt?.[i] ?? 0) > 0 && bio !== Biome.Glacier && bio !== Biome.Snow) {
        rough = 242 - ((riverAmt[i] / 255) * 158) | 0; // feathered banks blend matte->glossy
      } else if (elevation[i] < seaLevel) {
        rough = 128; // submerged shelf under the transparent shallows
      } else if (bio === Biome.Beach && (shore?.[i] ?? 0) > 0.72) {
        rough = 178; // wet swash-zone sand (0.7)
      }
      const o = (y * res + x) * 4;
      data[o] = 255;
      data[o + 1] = rough;
      data[o + 2] = 255;
      data[o + 3] = 255;
    }
  }
  const tex = new THREE.DataTexture(data, res, res, THREE.RGBAFormat, THREE.UnsignedByteType);
  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.magFilter = THREE.LinearFilter;
  tex.minFilter = THREE.LinearFilter;
  tex.needsUpdate = true;
  return tex;
}

/**
 * Renders the huge SoA world as a single height-displaced mesh. The mesh is built at
 * `meshResolution` (e.g. 384) while COLOUR comes from a full data-resolution texture and the
 * fine relief from a residual normal map — so a 2048^2 world reads at full detail without a
 * 4M-vertex mesh. The texture palette is BIOME_RGB, the exact colours the minimap paints, so
 * the 3D world and the minimap always match.
 */
export const WorldTerrain: React.FC<WorldTerrainProps> = ({
  world,
  renderSize = 400,
  heightRatio = 0.13,
  meshResolution = 256,
}) => {
  const geometry = useMemo(() => {
    const res = meshResolution;
    const verts = (res + 1) * (res + 1);
    const positions = new Float32Array(verts * 3);
    const uvs = new Float32Array(verts * 2);
    const heightUnits = renderSize * heightRatio;

    for (let gy = 0; gy <= res; gy++) {
      for (let gx = 0; gx <= res; gx++) {
        const u = gx / res;
        const v = gy / res;
        const e = sampleElevation(world, u, v);
        const i = gy * (res + 1) + gx;

        positions[i * 3] = (u - 0.5) * renderSize; // X
        positions[i * 3 + 1] = e * heightUnits; // Y (up)
        positions[i * 3 + 2] = (v - 0.5) * renderSize; // Z
        uvs[i * 2] = u;
        uvs[i * 2 + 1] = v;
      }
    }

    const indices = new Uint32Array(res * res * 6);
    let o = 0;
    for (let gy = 0; gy < res; gy++) {
      for (let gx = 0; gx < res; gx++) {
        const a = gy * (res + 1) + gx;
        const b = a + 1;
        const c = a + (res + 1);
        const d = c + 1;
        // CCW from above so computeVertexNormals() yields +Y normals (no back-face culling).
        indices[o++] = a;
        indices[o++] = c;
        indices[o++] = b;
        indices[o++] = b;
        indices[o++] = c;
        indices[o++] = d;
      }
    }

    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    geom.setAttribute('uv', new THREE.BufferAttribute(uvs, 2));
    geom.setIndex(new THREE.BufferAttribute(indices, 1));
    geom.computeVertexNormals();
    return geom;
  }, [world, renderSize, heightRatio, meshResolution]);

  const colorMap = useMemo(() => buildColorTexture(world), [world]);
  const normalMap = useMemo(
    () => buildNormalTexture(world, renderSize, heightRatio, meshResolution),
    [world, renderSize, heightRatio, meshResolution],
  );
  const roughnessMap = useMemo(() => buildRoughnessTexture(world), [world]);

  useEffect(() => {
    return () => {
      colorMap.dispose();
      normalMap.dispose();
      roughnessMap.dispose();
    };
  }, [colorMap, normalMap, roughnessMap]);

  return (
    <mesh geometry={geometry} name="world-terrain" receiveShadow>
      <meshStandardMaterial
        map={colorMap}
        normalMap={normalMap}
        normalScale={new THREE.Vector2(1, 1)}
        roughnessMap={roughnessMap}
        roughness={1.0}
        metalness={0.0}
      />
    </mesh>
  );
};

export default WorldTerrain;
