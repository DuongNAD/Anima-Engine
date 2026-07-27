import React, { useMemo, useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { sampleElevation } from './utils/worldSample';
import { sceneElapsed } from './utils/sceneClock';

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
      // Schools keep their distance from each other (no interlocking rings).
      let crowded = false;
      for (const o of out) {
        const ddx = o.cx - cx;
        const ddz = o.cz - cz;
        if (ddx * ddx + ddz * ddz < 40 * 40) {
          crowded = true;
          break;
        }
      }
      if (crowded) continue;
      const floorY = e * heightUnits;
      out.push({
        cx,
        cz,
        // Firmly UNDER the surface (≥1.5 below) and off the bed — submerged shadows, not
        // sprites standing on the water.
        y: Math.max(floorY + 0.5, Math.min(seaY - 1.5, floorY + (seaY - floorY) * (0.3 + hash01(probe * 7) * 0.3))),
        r: 4 + hash01(probe * 11) * 8,
        speed: (0.35 + hash01(probe * 13) * 0.4) * (hash01(probe * 17) > 0.5 ? 1 : -1),
        phase: hash01(probe * 19) * Math.PI * 2,
        color: new THREE.Color(SCHOOL_COLORS[out.length % SCHOOL_COLORS.length]).lerp(
          new THREE.Color('#16323e'),
          0.35, // submerged tint: darker and blue-shifted under the water column
        ),
      });
    }
    // FRESHWATER schools: one per sizeable unfrozen lake basin, circling under the surface.
    const FRESH_COLORS = ['#c9cdd3', '#c8a25a', '#9fb08a'];
    const { size, temperature, lakeBasins } = world;
    let fresh = 0;
    for (let bi = 0; bi < (lakeBasins?.length ?? 0) && fresh < 10; bi++) {
      const b = lakeBasins[bi];
      const w = b.maxX - b.minX;
      const h = b.maxY - b.minY;
      if (w * h < 90) continue; // ponds too small for a school
      const ccx = Math.round((b.minX + b.maxX) / 2);
      const ccy = Math.round((b.minY + b.maxY) / 2);
      if ((temperature?.[ccy * size + ccx] ?? 0.5) < 0.19) continue; // frozen over
      const u = ccx / (size - 1);
      const v = ccy / (size - 1);
      const surfaceY = b.level * heightUnits;
      const bedY = sampleElevation(world, u, v) * heightUnits;
      out.push({
        cx: (u - 0.5) * renderSize,
        cz: (v - 0.5) * renderSize,
        y: Math.min(surfaceY - 1.2, bedY + Math.max(0.6, (surfaceY - bedY) * 0.5)),
        r: Math.max(2.5, Math.min(7, (Math.min(w, h) * renderSize) / size / 3)),
        speed: (0.3 + hash01(bi * 23) * 0.3) * (hash01(bi * 29) > 0.5 ? 1 : -1),
        phase: hash01(bi * 31) * Math.PI * 2,
        color: new THREE.Color(FRESH_COLORS[fresh % FRESH_COLORS.length]).lerp(new THREE.Color('#16323e'), 0.3),
      });
      fresh++;
    }
    return out;
  }, [world, renderSize, heightUnits, seaY, schools]);

  const count = flights.length * fishPerSchool;

  useFrame((state) => {
    const inst = ref.current;
    if (!inst || typeof inst.setMatrixAt !== 'function' || count === 0) return;
    const t = sceneElapsed(state.clock);
    const dummy = new THREE.Object3D();
    let k = 0;
    for (let s = 0; s < flights.length; s++) {
      const f = flights[s];
      for (let i = 0; i < fishPerSchool; i++) {
        const off = (i / fishPerSchool) * Math.PI * 2 + hash01(s * 131 + i) * 0.9;
        // A loose SHOAL, not a carousel: every fish breathes its own radius and drifts its
        // own angular speed, so members constantly trade places without overlapping.
        const rr =
          f.r * (0.7 + hash01(s * 57 + i) * 0.55) * (1 + 0.16 * Math.sin(t * 0.6 + i * 2.1 + f.phase));
        const a = f.phase + t * f.speed * (1 + (hash01(s * 77 + i) - 0.5) * 0.3) + off;
        dummy.position.set(
          f.cx + Math.cos(a) * rr,
          f.y + Math.sin(t * 1.3 + i * 1.7) * 0.4,
          f.cz + Math.sin(a) * rr,
        );
        // Face the swim direction (tangent of the circle).
        dummy.rotation.set(0, -a - (f.speed > 0 ? Math.PI / 2 : -Math.PI / 2), 0);
        // Tail-beat wobble.
        const s0 = 0.75 + Math.sin(t * 9 + i * 2.3) * 0.1;
        dummy.scale.set(s0, 0.75, 0.75);
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
    <instancedMesh ref={ref} args={[geom, null, count]} name="world-fish" frustumCulled={false}>
      <meshBasicMaterial side={THREE.DoubleSide} transparent opacity={0.6} />
    </instancedMesh>
  );
};

export default WorldFish;
