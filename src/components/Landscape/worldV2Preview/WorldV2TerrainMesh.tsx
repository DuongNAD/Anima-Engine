import React, { useEffect, useMemo } from 'react';
import * as THREE from 'three';
import { BIOME_SRGB_PALETTE, RENDER_SIZE, type WorldV2PreviewGrid } from './types';

interface WorldV2TerrainMeshProps {
  grid: WorldV2PreviewGrid;
}

const MESH_SEGMENTS = 511; // 512x512 vertices (~522k tris), smooth on L2 1024x1024

function sampleBilinearHeight(
  heightM: Float32Array,
  size: number,
  u: number,
  v: number,
): number {
  const gx = Math.max(0, Math.min(size - 1, u * (size - 1)));
  const gz = Math.max(0, Math.min(size - 1, v * (size - 1)));
  const x0 = Math.floor(gx);
  const z0 = Math.floor(gz);
  const x1 = Math.min(size - 1, x0 + 1);
  const z1 = Math.min(size - 1, z0 + 1);
  const tx = gx - x0;
  const tz = gz - z0;
  const h00 = heightM[z0 * size + x0];
  const h10 = heightM[z0 * size + x1];
  const h01 = heightM[z1 * size + x0];
  const h11 = heightM[z1 * size + x1];
  return (h00 * (1 - tx) + h10 * tx) * (1 - tz) + (h01 * (1 - tx) + h11 * tx) * tz;
}

function sampleNormal(
  normalRg: Uint8Array,
  size: number,
  u: number,
  v: number,
): [number, number, number] {
  const x = Math.max(0, Math.min(size - 1, Math.round(u * (size - 1))));
  const z = Math.max(0, Math.min(size - 1, Math.round(v * (size - 1))));
  const idx = (z * size + x) * 2;
  const nx = (normalRg[idx] / 255) * 2 - 1;
  const nz = (normalRg[idx + 1] / 255) * 2 - 1;
  const nySq = Math.max(0.01, 1 - nx * nx - nz * nz);
  const ny = Math.sqrt(nySq);
  const invLen = 1 / Math.hypot(nx, ny, nz);
  return [nx * invLen, ny * invLen, nz * invLen];
}

function buildAlbedoAoTexture(grid: WorldV2PreviewGrid): THREE.DataTexture {
  const { size, macroAlbedoRgba, biomeIdsRgba, biomeWeightsRgba, lightingRgba } = grid;
  const texBytes = new Uint8Array(size * size * 4);

  for (let r = 0; r < size; r += 1) {
    // PlaneGeometry rotated -PI/2 maps North (r=0) to uv.y=1; with flipY=false that is texRow=size-1.
    const texRow = size - 1 - r;
    for (let c = 0; c < size; c += 1) {
      const srcIdx = r * size + c;
      const src4 = srcIdx * 4;
      const dst4 = (texRow * size + c) * 4;

      const macR = macroAlbedoRgba[src4];
      const macG = macroAlbedoRgba[src4 + 1];
      const macB = macroAlbedoRgba[src4 + 2];

      let bioR = 0;
      let bioG = 0;
      let bioB = 0;
      let wSum = 0;
      for (let k = 0; k < 4; k += 1) {
        const w = biomeWeightsRgba[src4 + k];
        if (w === 0) continue;
        const bId = biomeIdsRgba[src4 + k];
        const pal = BIOME_SRGB_PALETTE[bId] ?? [112, 112, 112];
        bioR += pal[0] * w;
        bioG += pal[1] * w;
        bioB += pal[2] * w;
        wSum += w;
      }
      if (wSum > 0) {
        bioR /= wSum;
        bioG /= wSum;
        bioB /= wSum;
      } else {
        bioR = macR;
        bioG = macG;
        bioB = macB;
      }

      const ao = lightingRgba[src4] / 255;
      const shade = 0.62 + 0.38 * ao;

      texBytes[dst4] = Math.min(255, Math.max(0, Math.round((macR * 0.78 + bioR * 0.22) * shade)));
      texBytes[dst4 + 1] = Math.min(
        255,
        Math.max(0, Math.round((macG * 0.78 + bioG * 0.22) * shade)),
      );
      texBytes[dst4 + 2] = Math.min(
        255,
        Math.max(0, Math.round((macB * 0.78 + bioB * 0.22) * shade)),
      );
      texBytes[dst4 + 3] = 255;
    }
  }

  const texture = new THREE.DataTexture(texBytes, size, size, THREE.RGBAFormat);
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.magFilter = THREE.LinearFilter;
  texture.minFilter = THREE.LinearMipmapLinearFilter;
  texture.generateMipmaps = true;
  texture.wrapS = THREE.ClampToEdgeWrapping;
  texture.wrapT = THREE.ClampToEdgeWrapping;
  texture.flipY = false;
  texture.needsUpdate = true;
  return texture;
}

function buildTerrainGeometry(grid: WorldV2PreviewGrid): THREE.BufferGeometry {
  const segs = Math.min(MESH_SEGMENTS, grid.size - 1);
  const geom = new THREE.PlaneGeometry(RENDER_SIZE, RENDER_SIZE, segs, segs);
  geom.rotateX(-Math.PI / 2);

  const posAttr = geom.getAttribute('position') as THREE.BufferAttribute;
  const normAttr = geom.getAttribute('normal') as THREE.BufferAttribute;
  const heightScale = RENDER_SIZE / grid.extentM;
  const stride = segs + 1;

  for (let iz = 0; iz <= segs; iz += 1) {
    const v = iz / segs;
    for (let ix = 0; ix <= segs; ix += 1) {
      const u = ix / segs;
      const idx = iz * stride + ix;
      const elevM = sampleBilinearHeight(grid.heightM, grid.size, u, v);
      posAttr.setY(idx, elevM * heightScale);

      const [nx, ny, nz] = sampleNormal(grid.normalRg, grid.size, u, v);
      normAttr.setXYZ(idx, nx, ny, nz);
    }
  }

  posAttr.needsUpdate = true;
  normAttr.needsUpdate = true;
  geom.computeBoundingSphere();
  geom.computeBoundingBox();
  return geom;
}

export const WorldV2TerrainMesh: React.FC<WorldV2TerrainMeshProps> = ({ grid }) => {
  const { geometry, texture } = useMemo(() => {
    return {
      geometry: buildTerrainGeometry(grid),
      texture: buildAlbedoAoTexture(grid),
    };
  }, [grid]);

  useEffect(() => {
    return () => {
      geometry.dispose();
      texture.dispose();
    };
  }, [geometry, texture]);

  return (
    <mesh name="world-terrain" geometry={geometry} receiveShadow castShadow>
      <meshStandardMaterial
        map={texture}
        roughness={0.88}
        metalness={0.04}
      />
    </mesh>
  );
};
