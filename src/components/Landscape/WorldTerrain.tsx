import React, { useMemo } from 'react';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { BIOME_RGB } from './utils/worldGen';

export interface WorldTerrainProps {
  world: World;
  /** World-space width the terrain is rendered at, regardless of data resolution. */
  renderSize?: number;
  /** Peak height as a fraction of renderSize. */
  heightRatio?: number;
  /** Mesh resolution (segments per side). Decoupled from the data resolution. */
  meshResolution?: number;
}

/** Bilinear elevation sample at normalized (u, v) in [0, 1]. */
function sampleElevation(world: World, u: number, v: number): number {
  const { size, elevation } = world;
  const fx = Math.min(size - 1, Math.max(0, u * (size - 1)));
  const fy = Math.min(size - 1, Math.max(0, v * (size - 1)));
  const x0 = Math.floor(fx);
  const y0 = Math.floor(fy);
  const x1 = Math.min(size - 1, x0 + 1);
  const y1 = Math.min(size - 1, y0 + 1);
  const tx = fx - x0;
  const ty = fy - y0;
  const e00 = elevation[y0 * size + x0];
  const e10 = elevation[y0 * size + x1];
  const e01 = elevation[y1 * size + x0];
  const e11 = elevation[y1 * size + x1];
  return (
    e00 * (1 - tx) * (1 - ty) + e10 * tx * (1 - ty) + e01 * (1 - tx) * ty + e11 * tx * ty
  );
}

function sampleBiome(world: World, u: number, v: number): number {
  const { size, biome } = world;
  const x = Math.min(size - 1, Math.max(0, Math.round(u * (size - 1))));
  const y = Math.min(size - 1, Math.max(0, Math.round(v * (size - 1))));
  return biome[y * size + x];
}

/**
 * Renders the huge SoA world as a single biome-coloured, height-displaced mesh. The mesh is
 * built at `meshResolution` (e.g. 256) while sampling the full-resolution heightmap, so a
 * 1024^2 world renders with rich relief without a million-vertex mesh.
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
    const colors = new Float32Array(verts * 3);
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

        const [r, g, b] = BIOME_RGB[sampleBiome(world, u, v)];
        colors[i * 3] = r / 255;
        colors[i * 3 + 1] = g / 255;
        colors[i * 3 + 2] = b / 255;
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
    geom.setAttribute('color', new THREE.BufferAttribute(colors, 3));
    geom.setIndex(new THREE.BufferAttribute(indices, 1));
    geom.computeVertexNormals();
    return geom;
  }, [world, renderSize, heightRatio, meshResolution]);

  return (
    <mesh geometry={geometry} name="world-terrain" receiveShadow>
      <meshStandardMaterial vertexColors roughness={0.95} metalness={0.0} />
    </mesh>
  );
};

export default WorldTerrain;
