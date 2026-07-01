import React, { useLayoutEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { FloraType } from './utils/worldGen';
import { sampleMeshHeight } from './utils/worldSample';

export interface WorldVegetationProps {
  world: World;
  renderSize?: number;
  heightRatio?: number;
  /** Mesh resolution the terrain is rendered at — flora snaps to THAT surface, not the data. */
  meshResolution?: number;
  /** Base size (world units) of a flora instance before its per-instance scale. */
  baseSize?: number;
}

// One low-poly geometry + colour per flora type.
const TYPE_DEFS: { type: FloraType; color: string; make: () => THREE.BufferGeometry }[] = [
  { type: FloraType.Pine, color: '#2f6b3d', make: () => new THREE.ConeGeometry(0.5, 1.8, 5) },
  { type: FloraType.Round, color: '#3f8a3a', make: () => new THREE.SphereGeometry(0.7, 6, 5) },
  { type: FloraType.Jungle, color: '#1d6b2e', make: () => new THREE.ConeGeometry(0.7, 1.4, 6) },
  { type: FloraType.Cactus, color: '#5f8a4a', make: () => new THREE.CylinderGeometry(0.25, 0.3, 1.2, 5) },
  { type: FloraType.Rock, color: '#8a847e', make: () => new THREE.DodecahedronGeometry(0.6, 0) },
];

const TypedInstances: React.FC<{
  world: World;
  type: FloraType;
  color: string;
  geometry: THREE.BufferGeometry;
  renderSize: number;
  heightRatio: number;
  meshResolution: number;
  baseSize: number;
}> = ({ world, type, color, geometry, renderSize, heightRatio, meshResolution, baseSize }) => {
  const ref = useRef<THREE.InstancedMesh>(null);

  // Indices of flora of this type.
  const indices = useMemo(() => {
    const out: number[] = [];
    for (let i = 0; i < world.floraCount; i++) {
      if (world.floraType[i] === type) out.push(i);
    }
    return out;
  }, [world, type]);

  // Vertical offset so the geometry's lowest point sits exactly on the ground (unit scale).
  const groundLift = useMemo(() => {
    geometry.computeBoundingBox();
    return geometry.boundingBox ? -geometry.boundingBox.min.y : 0;
  }, [geometry]);

  useLayoutEffect(() => {
    const inst = ref.current;
    if (!inst) return;
    const dummy = new THREE.Object3D();
    const { size } = world;
    const heightUnits = renderSize * heightRatio;
    for (let k = 0; k < indices.length; k++) {
      const i = indices[k];
      const u = (world.floraX[i] + size / 2) / size;
      const v = (world.floraZ[i] + size / 2) / size;
      const x = (u - 0.5) * renderSize;
      const z = (v - 0.5) * renderSize;
      // Snap to the RENDER MESH surface (coarser than the data) so nothing floats/sinks.
      const y = sampleMeshHeight(world, u, v, meshResolution) * heightUnits;
      const s = world.floraScale[i] * baseSize;
      dummy.position.set(x, y + groundLift * s, z);
      dummy.scale.set(s, s, s);
      dummy.rotation.set(0, (world.floraX[i] * 0.7 + world.floraZ[i] * 0.3) % (Math.PI * 2), 0);
      dummy.updateMatrix();
      if (typeof inst.setMatrixAt === 'function') inst.setMatrixAt(k, dummy.matrix);
    }
    if (inst.instanceMatrix) inst.instanceMatrix.needsUpdate = true;
  }, [indices, world, renderSize, heightRatio, meshResolution, baseSize, groundLift]);

  if (indices.length === 0) return null;
  return (
    <instancedMesh ref={ref} args={[geometry, undefined as any, indices.length]} name={`flora-${type}`}>
      <meshStandardMaterial color={color} roughness={0.9} metalness={0} flatShading />
    </instancedMesh>
  );
};

/** Renders the world's flora (SoA) as instanced low-poly meshes, one InstancedMesh per type. */
export const WorldVegetation: React.FC<WorldVegetationProps> = ({
  world,
  renderSize = 400,
  heightRatio = 0.13,
  meshResolution = 256,
  baseSize = 1.4,
}) => {
  const geoms = useMemo(() => TYPE_DEFS.map((d) => ({ ...d, geometry: d.make() })), []);
  return (
    <group name="world-vegetation">
      {geoms.map((d) => (
        <TypedInstances
          key={d.type}
          world={world}
          type={d.type}
          color={d.color}
          geometry={d.geometry}
          renderSize={renderSize}
          heightRatio={heightRatio}
          meshResolution={meshResolution}
          baseSize={baseSize}
        />
      ))}
    </group>
  );
};

export default WorldVegetation;
