import React, { useEffect, useMemo } from 'react';
import * as THREE from 'three';
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils.js';
import type { World } from './utils/worldGen';
import { buildCavesGeometry } from './utils/caveGeometry';
import { ELEV_MIN_M, ELEV_SPAN_M, type WorldV2LoadedBundle } from './worldV2/types';

export interface WorldCavesProps {
  world: World;
  worldV2?: WorldV2LoadedBundle | null;
  renderSize?: number;
  heightRatio?: number;
  meshResolution?: number;
}

function buildCaveNetworkGeometry(
  worldV2: WorldV2LoadedBundle | null | undefined,
  renderSize: number,
  heightUnits: number,
): THREE.BufferGeometry | null {
  const net = worldV2?.caveNetwork;
  if (!net || net.edges.length === 0 || net.nodes.length === 0) return null;
  const extentM = Math.max(1, worldV2.extentM);
  const parts: THREE.BufferGeometry[] = [];
  const up = new THREE.Vector3(0, 1, 0);

  for (const edge of net.edges) {
    const a = net.nodes[edge.from_idx];
    const b = net.nodes[edge.to_idx];
    if (!a || !b) continue;
    const p0 = new THREE.Vector3(
      (a.x_m / extentM - 0.5) * renderSize,
      ((a.y_m - ELEV_MIN_M) / ELEV_SPAN_M) * heightUnits,
      (a.z_m / extentM - 0.5) * renderSize,
    );
    const p1 = new THREE.Vector3(
      (b.x_m / extentM - 0.5) * renderSize,
      ((b.y_m - ELEV_MIN_M) / ELEV_SPAN_M) * heightUnits,
      (b.z_m / extentM - 0.5) * renderSize,
    );
    const delta = new THREE.Vector3().subVectors(p1, p0);
    const len = delta.length();
    if (len < 0.05) continue;
    const r0 = Math.max(0.3, (a.radius_m / extentM) * renderSize);
    const r1 = Math.max(0.3, (b.radius_m / extentM) * renderSize);
    const seg = new THREE.CylinderGeometry(r1, r0, len, 6, 1, true);
    const mid = new THREE.Vector3().addVectors(p0, p1).multiplyScalar(0.5);
    const quat = new THREE.Quaternion().setFromUnitVectors(up, delta.normalize());
    seg.applyQuaternion(quat);
    seg.translate(mid.x, mid.y, mid.z);
    parts.push(seg);
  }
  if (parts.length === 0) return null;
  const merged = mergeGeometries(parts, false);
  for (const p of parts) p.dispose();
  return merged;
}

export const WorldCaves: React.FC<WorldCavesProps> = ({
  world,
  worldV2 = null,
  renderSize = 400,
  heightRatio = 0.13,
  meshResolution = 384,
}) => {
  const heightUnits = renderSize * heightRatio;

  const geom = useMemo(
    () => buildCavesGeometry(world, renderSize, heightUnits, meshResolution),
    [world, renderSize, heightUnits, meshResolution],
  );

  const networkGeom = useMemo(
    () => buildCaveNetworkGeometry(worldV2, renderSize, heightUnits),
    [worldV2, renderSize, heightUnits],
  );

  // Dispose geometries when the world changes or component unmounts.
  useEffect(() => {
    return () => {
      if (geom && typeof geom.dispose === 'function') geom.dispose();
      if (networkGeom && typeof networkGeom.dispose === 'function') networkGeom.dispose();
    };
  }, [geom, networkGeom]);

  if (!geom && !networkGeom) return null;
  return (
    <group name="world-caves-group" userData={{ decorative: true }}>
      {geom ? (
        <mesh geometry={geom} name="world-caves" receiveShadow frustumCulled={false}>
          <meshStandardMaterial
            vertexColors
            // WV2-LOOK prototype: the World v2 scene runs at a higher exposure; keep the cave
            // mouths' rock as dark as the surrounding cliff instead of pale grey boxes.
            color={worldV2 ? '#5c5650' : '#ffffff'}
            roughness={0.94}
            metalness={0}
            flatShading
            side={THREE.DoubleSide}
          />
        </mesh>
      ) : null}
      {networkGeom ? (
        <mesh geometry={networkGeom} name="world-cave-network" frustumCulled={false}>
          <meshStandardMaterial
            color="#0b0c10"
            roughness={0.96}
            metalness={0}
            side={THREE.DoubleSide}
          />
        </mesh>
      ) : null}
    </group>
  );
};

export default WorldCaves;
