import React, { useMemo } from 'react';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { BIOME_RGB } from './utils/worldGen';
import { sampleElevation } from './utils/worldSample';

export interface WorldTerrainProps {
  world: World;
  /** World-space width the terrain is rendered at, regardless of data resolution. */
  renderSize?: number;
  /** Peak height as a fraction of renderSize. */
  heightRatio?: number;
  /** Mesh resolution (segments per side). Decoupled from the data resolution. */
  meshResolution?: number;
}

/** Nearest biome index at (u, v) — the EXACT same lookup the minimap uses. */
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
 *
 * Vertex colour = BIOME_RGB[biome] — the SAME colour map the minimap draws, so the 3D terrain
 * matches the minimap exactly (rivers, beaches, lakes, snow, etc. are already distinct biomes).
 * Relief shading comes from the scene lighting, not from tinting the colours.
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

        const [br, bg, bb] = BIOME_RGB[sampleBiome(world, u, v)];
        colors[i * 3] = br / 255;
        colors[i * 3 + 1] = bg / 255;
        colors[i * 3 + 2] = bb / 255;
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
