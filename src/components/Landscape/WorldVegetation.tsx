import React, { useLayoutEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { FloraType } from './utils/worldGen';
import { makeFloraGeometry } from './utils/floraGeometry';
import { VEGETATION_BASE_SIZE } from './utils/floraClearance';
import { sampleMeshHeight } from './utils/worldSample';

export interface WorldVegetationProps {
  world: World;
  renderSize?: number;
  heightRatio?: number;
  /** Mesh resolution the terrain is rendered at — flora snaps to THAT surface, not the data. */
  meshResolution?: number;
  /** Base size (world units) of a flora instance before its per-instance scale. */
  baseSize?: number;
  /** 'low' keeps every other instance and skips the shadow pass (light-GPU mode). */
  quality?: 'high' | 'low';
}

const TYPES: FloraType[] = [
  FloraType.Pine,
  FloraType.Round,
  FloraType.Jungle,
  FloraType.Cactus,
  FloraType.Rock,
  FloraType.Acacia,
  FloraType.Palm,
  FloraType.DeadTree,
  FloraType.Bush,
  FloraType.Reed,
  FloraType.Tuft,
  FloraType.Coral,
  FloraType.Kelp,
  FloraType.Seagrass,
];

/** Tall types cast sun shadows; ground cover (bushes, reeds, tufts, boulders) skips the
 *  shadow pass — it reads fine without and roughly halves the shadow-map vertex load. */
const CASTS_SHADOW = new Set<FloraType>([
  FloraType.Pine,
  FloraType.Round,
  FloraType.Jungle,
  FloraType.Cactus,
  FloraType.Acacia,
  FloraType.Palm,
  FloraType.DeadTree,
]);

/** Deterministic per-instance hash in [0, 1) from the flora's world position. */
function hash01(a: number, b: number): number {
  let h = Math.imul((a * 1024) | 0, 374761393) + Math.imul((b * 1024) | 0, 668265263);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

const TypedInstances: React.FC<{
  world: World;
  type: FloraType;
  geometry: THREE.BufferGeometry;
  renderSize: number;
  heightRatio: number;
  meshResolution: number;
  baseSize: number;
  lowQuality: boolean;
}> = ({ world, type, geometry, renderSize, heightRatio, meshResolution, baseSize, lowQuality }) => {
  const ref = useRef<THREE.InstancedMesh>(null);

  // Indices of flora of this type (low quality keeps every other instance).
  const indices = useMemo(() => {
    const out: number[] = [];
    let seen = 0;
    for (let i = 0; i < world.floraCount; i++) {
      if (world.floraType[i] !== type) continue;
      if (!lowQuality || seen % 2 === 0) out.push(i);
      seen++;
    }
    return out;
  }, [world, type, lowQuality]);

  // Vertical offset so the geometry's lowest point sits exactly on the ground (unit scale).
  const groundLift = useMemo(() => {
    geometry.computeBoundingBox();
    return geometry.boundingBox ? -geometry.boundingBox.min.y : 0;
  }, [geometry]);

  useLayoutEffect(() => {
    const inst = ref.current;
    if (!inst) return;
    const dummy = new THREE.Object3D();
    const tint = new THREE.Color();
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
      // Subtle per-instance tint (vertex colours carry the two-tone; this multiplies on top)
      // so a forest reads as thousands of slightly different trees, not one repeated prop.
      if (typeof inst.setColorAt === 'function') {
        const h = hash01(world.floraX[i], world.floraZ[i]);
        const h2 = hash01(world.floraZ[i], world.floraX[i]);
        if (type === FloraType.Tuft) {
          // Meadow mix over the pale neutral base: mostly grass greens, a scattering of
          // pink / yellow / white wildflowers.
          if (h < 0.055) tint.setRGB(1.05, 0.6, 0.78); // pink
          else if (h < 0.1) tint.setRGB(1.12, 1.02, 0.42); // yellow
          else if (h < 0.13) tint.setRGB(1.12, 1.1, 1.05); // white
          else {
            const g = 0.5 + h2 * 0.18;
            tint.setRGB(g * 0.78, g * 1.05, g * 0.55); // grass green range
          }
        } else if (type === FloraType.Coral) {
          // Reef palette over the pale base: pink / orange / purple / red / cream heads.
          if (h < 0.3) tint.setRGB(1.25, 0.55, 0.72);
          else if (h < 0.55) tint.setRGB(1.3, 0.75, 0.4);
          else if (h < 0.75) tint.setRGB(0.85, 0.55, 1.15);
          else if (h < 0.9) tint.setRGB(1.3, 0.45, 0.45);
          else tint.setRGB(1.1, 1.05, 0.88);
        } else {
          const lum = 0.82 + h * 0.36; // ±18% brightness
          const warm = 0.96 + h2 * 0.08; // slight hue drift
          tint.setRGB(lum * warm, lum, lum * (2 - warm));
        }
        inst.setColorAt(k, tint);
      }
    }
    if (inst.instanceMatrix) inst.instanceMatrix.needsUpdate = true;
    if (inst.instanceColor) inst.instanceColor.needsUpdate = true;
    // `type` chooses the tint palette a few lines up, so it belongs here: leaving it out meant a
    // meadow re-rendered as coral kept its greens until something else invalidated the effect.
  }, [indices, world, renderSize, heightRatio, meshResolution, baseSize, groundLift, type]);

  if (indices.length === 0) return null;
  return (
    <instancedMesh
      ref={ref}
      args={[geometry, null, indices.length]}
      name={`flora-${type}`}
      castShadow={!lowQuality && CASTS_SHADOW.has(type)}
    >
      <meshStandardMaterial vertexColors roughness={0.9} metalness={0} flatShading />
    </instancedMesh>
  );
};

/** Renders the world's flora (SoA) as instanced low-poly meshes, one InstancedMesh per type. */
export const WorldVegetation: React.FC<WorldVegetationProps> = ({
  world,
  renderSize = 400,
  heightRatio = 0.13,
  meshResolution = 256,
  baseSize = VEGETATION_BASE_SIZE,
  quality = 'high',
}) => {
  const geoms = useMemo(() => TYPES.map((type) => ({ type, geometry: makeFloraGeometry(type) })), []);
  return (
    <group name="world-vegetation">
      {geoms.map((d) => (
        <TypedInstances
          key={d.type}
          world={world}
          type={d.type}
          geometry={d.geometry}
          renderSize={renderSize}
          heightRatio={heightRatio}
          meshResolution={meshResolution}
          baseSize={baseSize}
          lowQuality={quality === 'low'}
        />
      ))}
    </group>
  );
};

export default WorldVegetation;
