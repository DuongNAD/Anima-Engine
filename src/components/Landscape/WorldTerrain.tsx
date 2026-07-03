import React, { useEffect, useMemo } from 'react';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { BIOME_RGB } from './utils/worldGen';
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
  const { size, biome } = world;
  const data = new Uint8Array(size * size * 4);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = y * size + x;
      const [r, g, b] = BIOME_RGB[biome[i]] ?? [255, 0, 255];
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
  const { size } = world;
  const heightUnits = renderSize * heightRatio;
  const cellW = renderSize / (size - 1);

  // Residual world-space height per cell (full detail minus the mesh's bilinear surface).
  const resid = new Float32Array(size * size);
  const inv = 1 / (size - 1);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const u = x * inv;
      const v = y * inv;
      resid[y * size + x] =
        (sampleElevation(world, u, v) - sampleMeshHeight(world, u, v, meshResolution)) * heightUnits;
    }
  }

  const data = new Uint8Array(size * size * 4);
  for (let y = 0; y < size; y++) {
    const y0 = Math.max(0, y - 1) * size;
    const y1 = Math.min(size - 1, y + 1) * size;
    for (let x = 0; x < size; x++) {
      const x0 = Math.max(0, x - 1);
      const x1 = Math.min(size - 1, x + 1);
      const dhdx = (resid[y * size + x1] - resid[y * size + x0]) / ((x1 - x0) * cellW || cellW);
      const dhdz = (resid[y1 + x] - resid[y0 + x]) / (((y1 - y0) / size) * cellW || cellW);
      // Tangent space of a +Y-up heightfield with uv aligned to world XZ.
      const nx = -dhdx;
      const nz = -dhdz;
      const len = Math.sqrt(nx * nx + nz * nz + 1);
      const o = (y * size + x) * 4;
      data[o] = ((nx / len) * 0.5 + 0.5) * 255;
      data[o + 1] = ((nz / len) * 0.5 + 0.5) * 255;
      data[o + 2] = ((1 / len) * 0.5 + 0.5) * 255;
      data[o + 3] = 255;
    }
  }
  const tex = new THREE.DataTexture(data, size, size, THREE.RGBAFormat, THREE.UnsignedByteType);
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

  useEffect(() => {
    return () => {
      colorMap.dispose();
      normalMap.dispose();
    };
  }, [colorMap, normalMap]);

  return (
    <mesh geometry={geometry} name="world-terrain" receiveShadow>
      <meshStandardMaterial
        map={colorMap}
        normalMap={normalMap}
        normalScale={new THREE.Vector2(1, 1)}
        roughness={0.95}
        metalness={0.0}
      />
    </mesh>
  );
};

export default WorldTerrain;
