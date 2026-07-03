import React, { useEffect, useLayoutEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';
import type { World } from './utils/worldGen';

// ---------------------------------------------------------------------------------------
// WorldWaterfalls — foam curtains hung over the steep river drops the generator detected
// (World.waterfall*). Two instanced meshes total: the curtains and the splash pools at
// their feet, so even hundreds of falls cost two draw calls.
// ---------------------------------------------------------------------------------------

export interface WorldWaterfallsProps {
  world: World;
  renderSize?: number;
  heightRatio?: number;
}

/** Vertical 1x1 plane, vertex-coloured white at the lip fading to pale blue at the base. */
function makeCurtainGeometry(): THREE.BufferGeometry {
  const geom = new THREE.PlaneGeometry(1, 1, 1, 3);
  const pos = geom.attributes.position;
  const colors = new Float32Array(pos.count * 3);
  for (let i = 0; i < pos.count; i++) {
    const t = pos.getY(i) + 0.5; // 0 at the base, 1 at the lip
    colors[i * 3] = 0.78 + 0.22 * t;
    colors[i * 3 + 1] = 0.88 + 0.12 * t;
    colors[i * 3 + 2] = 0.94 + 0.06 * t;
  }
  geom.setAttribute('color', new THREE.BufferAttribute(colors, 3));
  return geom;
}

export const WorldWaterfalls: React.FC<WorldWaterfallsProps> = ({
  world,
  renderSize = 400,
  heightRatio = 0.13,
}) => {
  const curtainRef = useRef<THREE.InstancedMesh>(null);
  const foamRef = useRef<THREE.InstancedMesh>(null);

  const count = world.waterfallCount ?? 0;
  const heightUnits = renderSize * heightRatio;

  const curtainGeom = useMemo(() => makeCurtainGeometry(), []);
  const foamGeom = useMemo(() => new THREE.CircleGeometry(1, 10).rotateX(-Math.PI / 2), []);

  useLayoutEffect(() => {
    const curtain = curtainRef.current;
    const foam = foamRef.current;
    if (!curtain || !foam || count === 0) return;
    const dummy = new THREE.Object3D();
    const { size } = world;
    const toWorld = renderSize / size;
    for (let i = 0; i < count; i++) {
      const x = world.waterfallX[i] * toWorld;
      const z = world.waterfallZ[i] * toWorld;
      const topY = world.waterfallTopE[i] * heightUnits;
      const dropY = Math.max(1.2, world.waterfallDrop[i] * heightUnits);
      const yaw = world.waterfallYaw[i];
      const dirX = Math.cos(yaw);
      const dirZ = Math.sin(yaw);
      const width = Math.min(4, 1.2 + dropY * 0.3);

      // Curtain: hangs from the lip, pushed slightly proud of the rock face.
      dummy.position.set(x + dirX * 0.7, topY - dropY / 2, z + dirZ * 0.7);
      dummy.rotation.set(0, Math.PI / 2 - yaw, 0);
      dummy.scale.set(width, dropY + 0.6, 1);
      dummy.updateMatrix();
      if (typeof curtain.setMatrixAt === 'function') curtain.setMatrixAt(i, dummy.matrix);

      // Splash pool at the base.
      dummy.position.set(x + dirX * (0.9 + width * 0.2), topY - dropY + 0.18, z + dirZ * (0.9 + width * 0.2));
      dummy.rotation.set(0, 0, 0);
      dummy.scale.set(width * 0.75, 1, width * 0.5);
      dummy.updateMatrix();
      if (typeof foam.setMatrixAt === 'function') foam.setMatrixAt(i, dummy.matrix);
    }
    if (curtain.instanceMatrix) curtain.instanceMatrix.needsUpdate = true;
    if (foam.instanceMatrix) foam.instanceMatrix.needsUpdate = true;
  }, [world, count, renderSize, heightUnits]);

  useEffect(() => {
    return () => {
      curtainGeom.dispose();
      foamGeom.dispose();
    };
  }, [curtainGeom, foamGeom]);

  if (count === 0) return null;
  return (
    <group name="world-waterfalls">
      <instancedMesh ref={curtainRef} args={[curtainGeom, undefined as any, count]} name="waterfall-curtains">
        <meshBasicMaterial vertexColors transparent opacity={0.85} side={THREE.DoubleSide} />
      </instancedMesh>
      <instancedMesh ref={foamRef} args={[foamGeom, undefined as any, count]} name="waterfall-foam">
        <meshBasicMaterial color="#ffffff" transparent opacity={0.45} depthWrite={false} />
      </instancedMesh>
    </group>
  );
};

export default WorldWaterfalls;
