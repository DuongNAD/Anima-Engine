import React, { useEffect, useMemo } from 'react';
import * as THREE from 'three';
import { RENDER_SIZE, WaterKind, type WorldV2PreviewGrid } from './types';

interface WorldV2WaterMeshProps {
  grid: WorldV2PreviewGrid;
}

const INLAND_COLOR_BY_KIND: Record<number, readonly [number, number, number]> = {
  [WaterKind.FreshwaterLake]: [0.11, 0.44, 0.67],
  [WaterKind.EndorheicLake]: [0.14, 0.48, 0.61],
  [WaterKind.River]: [0.16, 0.56, 0.79],
  [WaterKind.Waterfall]: [0.55, 0.85, 0.98],
  [WaterKind.DeltaTurbid]: [0.21, 0.52, 0.58],
};

function buildInlandWaterGeometry(grid: WorldV2PreviewGrid): THREE.BufferGeometry | null {
  const { size, waterKind, waterHeightM, extentM } = grid;
  const heightScale = RENDER_SIZE / extentM;
  const cellSpan = RENDER_SIZE / size;
  const half = RENDER_SIZE / 2;
  const pad = cellSpan * 0.58; // Slight overlap avoids pinholes between diagonal river cells.

  let wetCount = 0;
  for (let i = 0; i < waterKind.length; i += 1) {
    if (waterKind[i] >= WaterKind.FreshwaterLake) {
      wetCount += 1;
    }
  }
  if (wetCount === 0) return null;

  const positions = new Float32Array(wetCount * 4 * 3);
  const normals = new Float32Array(wetCount * 4 * 3);
  const colors = new Float32Array(wetCount * 4 * 3);
  const indices = new Uint32Array(wetCount * 6);

  let q = 0;
  for (let r = 0; r < size; r += 1) {
    const cz = -half + (r + 0.5) * cellSpan;
    for (let c = 0; c < size; c += 1) {
      const idx = r * size + c;
      const wk = waterKind[idx];
      if (wk < WaterKind.FreshwaterLake) continue;

      const cx = -half + (c + 0.5) * cellSpan;
      const y = waterHeightM[idx] * heightScale + 0.22;
      const rgb = INLAND_COLOR_BY_KIND[wk] ?? [0.14, 0.5, 0.72];

      const vBase = q * 4;
      const pBase = vBase * 3;
      // Top-facing quad (CCW from +Y)
      positions[pBase] = cx - pad;
      positions[pBase + 1] = y;
      positions[pBase + 2] = cz - pad;

      positions[pBase + 3] = cx - pad;
      positions[pBase + 4] = y;
      positions[pBase + 5] = cz + pad;

      positions[pBase + 6] = cx + pad;
      positions[pBase + 7] = y;
      positions[pBase + 8] = cz + pad;

      positions[pBase + 9] = cx + pad;
      positions[pBase + 10] = y;
      positions[pBase + 11] = cz - pad;

      for (let v = 0; v < 4; v += 1) {
        const nOff = pBase + v * 3;
        normals[nOff] = 0;
        normals[nOff + 1] = 1;
        normals[nOff + 2] = 0;
        colors[nOff] = rgb[0];
        colors[nOff + 1] = rgb[1];
        colors[nOff + 2] = rgb[2];
      }

      const iBase = q * 6;
      indices[iBase] = vBase;
      indices[iBase + 1] = vBase + 1;
      indices[iBase + 2] = vBase + 2;
      indices[iBase + 3] = vBase;
      indices[iBase + 4] = vBase + 2;
      indices[iBase + 5] = vBase + 3;
      q += 1;
    }
  }

  const geom = new THREE.BufferGeometry();
  geom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
  geom.setAttribute('normal', new THREE.BufferAttribute(normals, 3));
  geom.setAttribute('color', new THREE.BufferAttribute(colors, 3));
  geom.setIndex(new THREE.BufferAttribute(indices, 1));
  geom.computeBoundingSphere();
  return geom;
}

export const WorldV2WaterMesh: React.FC<WorldV2WaterMeshProps> = ({ grid }) => {
  const inlandGeometry = useMemo(() => buildInlandWaterGeometry(grid), [grid]);

  useEffect(() => {
    return () => {
      inlandGeometry?.dispose();
    };
  }, [inlandGeometry]);

  return (
    <group name="world-v2-water">
      <mesh name="world-v2-ocean" rotation={[-Math.PI / 2, 0, 0]} position={[0, 0.05, 0]} receiveShadow>
        <planeGeometry args={[RENDER_SIZE, RENDER_SIZE, 1, 1]} />
        <meshStandardMaterial
          color="#134e78"
          roughness={0.18}
          metalness={0.18}
          transparent
          opacity={0.84}
        />
      </mesh>
      {inlandGeometry ? (
        <mesh name="world-v2-inland-water" geometry={inlandGeometry} receiveShadow>
          <meshStandardMaterial
            vertexColors
            roughness={0.14}
            metalness={0.15}
            transparent
            opacity={0.88}
          />
        </mesh>
      ) : null}
    </group>
  );
};
