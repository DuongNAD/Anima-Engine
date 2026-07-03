import React, { useMemo, useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { sampleElevation } from './utils/worldSample';

// ---------------------------------------------------------------------------------------
// WorldFish — schools of fish circling in the sunlit shallows, seen through the transparent
// water from above. One InstancedMesh for every fish in every school (a single draw call);
// each school gets its own colour, depth, radius and speed, each fish its own phase and
// bob so the school shimmers instead of rotating as a rigid ring.
// ---------------------------------------------------------------------------------------

export interface WorldFishProps {
  world: World;
  renderSize?: number;
  heightRatio?: number;
  schools?: number;
  fishPerSchool?: number;
}

/** Flat side-view fish silhouette pointing +X (body triangle + tail fin), ~1 unit long. */
function makeFishGeometry(): THREE.BufferGeometry {
  const geom = new THREE.BufferGeometry();
  const verts = new Float32Array([
    // body
    0.42, 0, 0, -0.28, 0.14, 0, -0.28, -0.12, 0,
    // tail
    -0.26, 0, 0, -0.5, 0.15, 0, -0.5, -0.15, 0,
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

const SCHOOL_COLORS = ['#d8dee6', '#ffd257', '#6db4e8', '#ff9c54', '#a9e0d0', '#e88fb0'];

export const WorldFish: React.FC<WorldFishProps> = ({
  world,
  renderSize = 400,
  heightRatio = 0.13,
  schools = 12,
  fishPerSchool = 20,
}) => {
  const ref = useRef<THREE.InstancedMesh>(null);
  const geom = useMemo(() => makeFishGeometry(), []);
  const heightUnits = renderSize * heightRatio;
  const seaY = world.seaLevel * heightUnits;

  // Deterministic school sites: probe hashed cells for warm-ish shallow ocean floor.
  const flights = useMemo(() => {
    const out: Array<{
      cx: number;
      cz: number;
      y: number;
      r: number;
      speed: number;
      phase: number;
      color: THREE.Color;
    }> = [];
    for (let probe = 0; probe < 4000 && out.length < schools; probe++) {
      const u = hash01(probe * 2 + 1);
      const v = hash01(probe * 2 + 2);
      const e = sampleElevation(world, u, v);
      const depth = world.seaLevel - e;
      if (depth < 0.02 || depth > 0.07) continue; // sunlit shelf only
      const cx = (u - 0.5) * renderSize;
      const cz = (v - 0.5) * renderSize;
      const floorY = e * heightUnits;
      out.push({
        cx,
        cz,
        y: floorY + (seaY - floorY) * (0.35 + hash01(probe * 7) * 0.35), // mid-water column
        r: 4 + hash01(probe * 11) * 8,
        speed: (0.35 + hash01(probe * 13) * 0.4) * (hash01(probe * 17) > 0.5 ? 1 : -1),
        phase: hash01(probe * 19) * Math.PI * 2,
        color: new THREE.Color(SCHOOL_COLORS[out.length % SCHOOL_COLORS.length]),
      });
    }
    return out;
  }, [world, renderSize, heightUnits, seaY, schools]);

  const count = flights.length * fishPerSchool;

  useFrame((state) => {
    const inst = ref.current;
    if (!inst || typeof inst.setMatrixAt !== 'function' || count === 0) return;
    const t = state.clock.getElapsedTime();
    const dummy = new THREE.Object3D();
    let k = 0;
    for (let s = 0; s < flights.length; s++) {
      const f = flights[s];
      for (let i = 0; i < fishPerSchool; i++) {
        const off = (i / fishPerSchool) * Math.PI * 2 + hash01(s * 131 + i) * 0.5;
        const rr = f.r * (0.75 + hash01(s * 57 + i) * 0.5);
        const a = f.phase + t * f.speed + off;
        dummy.position.set(
          f.cx + Math.cos(a) * rr,
          f.y + Math.sin(t * 1.3 + i * 1.7) * 0.5,
          f.cz + Math.sin(a) * rr,
        );
        // Face the swim direction (tangent of the circle).
        dummy.rotation.set(0, -a - (f.speed > 0 ? Math.PI / 2 : -Math.PI / 2), 0);
        // Tail-beat wobble.
        const s0 = 0.9 + Math.sin(t * 9 + i * 2.3) * 0.12;
        dummy.scale.set(s0, 0.9, 0.9);
        dummy.updateMatrix();
        inst.setMatrixAt(k, dummy.matrix);
        if (typeof inst.setColorAt === 'function' && t < 0.5) inst.setColorAt(k, f.color);
        k++;
      }
    }
    if (inst.instanceMatrix) inst.instanceMatrix.needsUpdate = true;
    if (inst.instanceColor && t < 0.5) inst.instanceColor.needsUpdate = true;
  });

  if (count === 0) return null;
  return (
    <instancedMesh ref={ref} args={[geom, undefined as any, count]} name="world-fish" frustumCulled={false}>
      <meshBasicMaterial side={THREE.DoubleSide} transparent opacity={0.92} />
    </instancedMesh>
  );
};

export default WorldFish;
