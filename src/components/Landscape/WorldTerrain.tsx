import React, { useMemo } from 'react';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { BIOME_RGB } from './utils/worldGen';
import { sampleElevation, sampleField } from './utils/worldSample';

export interface WorldTerrainProps {
  world: World;
  /** World-space width the terrain is rendered at, regardless of data resolution. */
  renderSize?: number;
  /** Peak height as a fraction of renderSize. */
  heightRatio?: number;
  /** Mesh resolution (segments per side). Decoupled from the data resolution. */
  meshResolution?: number;
}

function sampleBiome(world: World, u: number, v: number): number {
  const { size, biome } = world;
  const x = Math.min(size - 1, Math.max(0, Math.round(u * (size - 1))));
  const y = Math.min(size - 1, Math.max(0, Math.round(v * (size - 1))));
  return biome[y * size + x];
}

// Colours blended into the terrain (0..1) for features baked into the mesh itself.
const RIVER_RGB = [0.16, 0.42, 0.62]; // river/stream water blended by flow
const SAND_RGB = [0.72, 0.66, 0.5]; // damp sand along shorelines
const ROCK_RGB = [0.5, 0.48, 0.45]; // exposed rock on steep faces

function smoothstep(e0: number, e1: number, x: number): number {
  const t = Math.max(0, Math.min(1, (x - e0) / (e1 - e0)));
  return t * t * (3 - 2 * t);
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
    const { size, flow, shore, slope, seaLevel } = world;

    for (let gy = 0; gy <= res; gy++) {
      for (let gx = 0; gx <= res; gx++) {
        const u = gx / res;
        const v = gy / res;
        const e = sampleElevation(world, u, v);
        const i = gy * (res + 1) + gx;

        // River amount: smooth blue where flow accumulates on land (baked into the mesh, so
        // streams read as a continuous ribbon following the ground — no floating quads).
        const f = sampleField(flow, size, u, v);
        const riverAmt = e >= seaLevel ? smoothstep(0.5, 0.82, f) : 0;

        positions[i * 3] = (u - 0.5) * renderSize; // X
        // Carve a shallow groove along strong flow so the river sits in a channel.
        positions[i * 3 + 1] = (e - riverAmt * 0.02) * heightUnits; // Y (up)
        positions[i * 3 + 2] = (v - 0.5) * renderSize; // Z

        const [br, bg, bb] = BIOME_RGB[sampleBiome(world, u, v)];
        let r = br / 255;
        let g = bg / 255;
        let b = bb / 255;

        // Damp-sand shoreline: blend towards sand near oceans & lakes.
        const sh = e >= seaLevel ? sampleField(shore, size, u, v) : 0;
        if (sh > 0) {
          const t = Math.min(1, sh) * 0.85;
          r += (SAND_RGB[0] - r) * t;
          g += (SAND_RGB[1] - g) * t;
          b += (SAND_RGB[2] - b) * t;
        }

        // Cliff shading: steep faces expose bare rock, breaking up the greenery on mountains.
        const sl = e >= seaLevel ? sampleField(slope, size, u, v) : 0;
        if (sl > 0) {
          const t = smoothstep(0.55, 1.0, sl) * 0.7;
          r += (ROCK_RGB[0] - r) * t;
          g += (ROCK_RGB[1] - g) * t;
          b += (ROCK_RGB[2] - b) * t;
        }

        // River water tint blended on top (rivers sit in flat valley bottoms).
        if (riverAmt > 0) {
          const t = riverAmt * 0.85;
          r += (RIVER_RGB[0] - r) * t;
          g += (RIVER_RGB[1] - g) * t;
          b += (RIVER_RGB[2] - b) * t;
        }

        colors[i * 3] = r;
        colors[i * 3 + 1] = g;
        colors[i * 3 + 2] = b;
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
