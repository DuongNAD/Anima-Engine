import React, { useMemo, useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';

// ---------------------------------------------------------------------------------------
// WorldBirds — a handful of birds gliding in slow circles over the world. One InstancedMesh
// of a 2-triangle silhouette; wing flap is faked by squashing the instance's Y scale. It is
// the cheapest possible "this world is alive" signal (one draw call, ~20 matrices/frame).
// ---------------------------------------------------------------------------------------

export interface WorldBirdsProps {
  renderSize?: number;
  count?: number;
}

/** Shallow-V bird silhouette (two triangles), ~2 units wingspan before instance scale. */
function makeBirdGeometry(): THREE.BufferGeometry {
  const geom = new THREE.BufferGeometry();
  // front, back, left tip / front, right tip, back
  const verts = new Float32Array([
    0, 0, 0.38, 0, 0, -0.22, -1, 0.34, -0.06,
    0, 0, 0.38, 1, 0.34, -0.06, 0, 0, -0.22,
  ]);
  geom.setAttribute('position', new THREE.BufferAttribute(verts, 3));
  geom.computeVertexNormals();
  return geom;
}

function hash01(n: number): number {
  let h = Math.imul(n + 1, 374761393);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

export const WorldBirds: React.FC<WorldBirdsProps> = ({ renderSize = 400, count = 18 }) => {
  const ref = useRef<THREE.InstancedMesh>(null);
  const geom = useMemo(() => makeBirdGeometry(), []);

  // Per-bird flight parameters (deterministic): circle centre, radius, height, speed, phase.
  const flights = useMemo(
    () =>
      Array.from({ length: count }, (_, i) => ({
        cx: (hash01(i * 5 + 1) - 0.5) * renderSize * 0.9,
        cz: (hash01(i * 5 + 2) - 0.5) * renderSize * 0.9,
        r: renderSize * (0.03 + hash01(i * 5 + 3) * 0.06),
        y: renderSize * (0.09 + hash01(i * 5 + 4) * 0.08),
        speed: (0.12 + hash01(i * 5 + 5) * 0.2) * (hash01(i * 7) > 0.5 ? 1 : -1),
        phase: hash01(i * 11) * Math.PI * 2,
        flap: 7 + hash01(i * 13) * 5,
      })),
    [count, renderSize],
  );

  useFrame((state) => {
    const inst = ref.current;
    if (!inst || typeof inst.setMatrixAt !== 'function') return;
    const t = state.clock.getElapsedTime();
    const dummy = new THREE.Object3D();
    for (let i = 0; i < flights.length; i++) {
      const f = flights[i];
      const a = f.phase + t * f.speed;
      dummy.position.set(f.cx + Math.cos(a) * f.r, f.y + Math.sin(t * 0.7 + f.phase) * 3, f.cz + Math.sin(a) * f.r);
      // Face along the flight direction; bank slightly into the turn.
      dummy.rotation.set(0, -a - (f.speed > 0 ? Math.PI / 2 : -Math.PI / 2), f.speed > 0 ? -0.18 : 0.18);
      // Wing flap: squash/stretch the V vertically.
      const flap = 0.55 + Math.abs(Math.sin(t * f.flap + f.phase)) * 0.9;
      dummy.scale.set(1.6, 1.6 * flap, 1.6);
      dummy.updateMatrix();
      inst.setMatrixAt(i, dummy.matrix);
    }
    if (inst.instanceMatrix) inst.instanceMatrix.needsUpdate = true;
  });

  return (
    <instancedMesh ref={ref} args={[geom, undefined as any, count]} name="world-birds" frustumCulled={false}>
      <meshBasicMaterial color="#1d232b" side={THREE.DoubleSide} />
    </instancedMesh>
  );
};

export default WorldBirds;
