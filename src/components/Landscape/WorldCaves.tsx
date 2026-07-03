import React, { useEffect, useLayoutEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';
import type { World } from './utils/worldGen';

// ---------------------------------------------------------------------------------------
// WorldCaves — cave mouths on steep bare-rock faces (World.cave*). Each mouth is a
// near-black unlit ellipse set against the cliff (an unlit surface on a lit mountainside
// reads as a hole), all instances in one draw call.
// ---------------------------------------------------------------------------------------

export interface WorldCavesProps {
  world: World;
  renderSize?: number;
  heightRatio?: number;
}

export const WorldCaves: React.FC<WorldCavesProps> = ({ world, renderSize = 400, heightRatio = 0.13 }) => {
  const ref = useRef<THREE.InstancedMesh>(null);
  const count = world.caveCount ?? 0;
  const heightUnits = renderSize * heightRatio;

  const geom = useMemo(() => new THREE.CircleGeometry(1, 10), []);

  useLayoutEffect(() => {
    const inst = ref.current;
    if (!inst || count === 0) return;
    const dummy = new THREE.Object3D();
    const toWorld = renderSize / world.size;
    for (let i = 0; i < count; i++) {
      const yaw = world.caveYaw[i];
      const dirX = Math.cos(yaw);
      const dirZ = Math.sin(yaw);
      // Proud of the face along the outward (downhill) bearing so it doesn't z-fight the mesh.
      dummy.position.set(
        world.caveX[i] * toWorld + dirX * 0.4,
        world.caveE[i] * heightUnits + 0.9,
        world.caveZ[i] * toWorld + dirZ * 0.4,
      );
      dummy.rotation.set(0, Math.PI / 2 - yaw, 0);
      dummy.scale.set(1.5, 1.1, 1);
      dummy.updateMatrix();
      if (typeof inst.setMatrixAt === 'function') inst.setMatrixAt(i, dummy.matrix);
    }
    if (inst.instanceMatrix) inst.instanceMatrix.needsUpdate = true;
  }, [world, count, renderSize, heightUnits]);

  useEffect(() => {
    return () => geom.dispose();
  }, [geom]);

  if (count === 0) return null;
  return (
    <instancedMesh ref={ref} args={[geom, undefined as any, count]} name="world-caves">
      <meshBasicMaterial color="#0b0906" side={THREE.DoubleSide} />
    </instancedMesh>
  );
};

export default WorldCaves;
