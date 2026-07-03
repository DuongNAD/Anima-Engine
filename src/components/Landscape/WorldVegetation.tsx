import React, { useLayoutEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils.js';
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
  /** 'low' keeps every other instance and skips the shadow pass (light-GPU mode). */
  quality?: 'high' | 'low';
}

/** Fills a geometry's vertex colours with one flat colour (so merged parts keep two tones). */
function paint(geom: THREE.BufferGeometry, hex: string): THREE.BufferGeometry {
  const c = new THREE.Color(hex);
  const count = geom.attributes.position.count;
  const colors = new Float32Array(count * 3);
  for (let i = 0; i < count; i++) {
    colors[i * 3] = c.r;
    colors[i * 3 + 1] = c.g;
    colors[i * 3 + 2] = c.b;
  }
  geom.setAttribute('color', new THREE.BufferAttribute(colors, 3));
  return geom;
}

const TRUNK = '#6b4a2f';

/**
 * One merged low-poly geometry per flora type, base sitting on y=0. Trunks and canopies are
 * separate vertex-colour tones inside a single geometry, so each type still renders as ONE
 * InstancedMesh (two-tone without extra draw calls).
 */
function makeGeometry(type: FloraType): THREE.BufferGeometry {
  const parts: THREE.BufferGeometry[] = [];
  switch (type) {
    case FloraType.Pine: {
      parts.push(paint(new THREE.CylinderGeometry(0.09, 0.13, 0.5, 5).translate(0, 0.25, 0), TRUNK));
      parts.push(paint(new THREE.ConeGeometry(0.55, 1.1, 6).translate(0, 0.95, 0), '#2f6b3d'));
      parts.push(paint(new THREE.ConeGeometry(0.38, 0.8, 6).translate(0, 1.6, 0), '#3a7c49'));
      break;
    }
    case FloraType.Round: {
      parts.push(paint(new THREE.CylinderGeometry(0.1, 0.15, 0.6, 5).translate(0, 0.3, 0), TRUNK));
      parts.push(paint(new THREE.SphereGeometry(0.62, 6, 5).translate(0, 1.05, 0), '#3f8a3a'));
      parts.push(paint(new THREE.SphereGeometry(0.38, 5, 4).translate(0.34, 1.32, 0.18), '#4d9c46'));
      break;
    }
    case FloraType.Jungle: {
      parts.push(paint(new THREE.CylinderGeometry(0.09, 0.13, 1.15, 5).translate(0, 0.575, 0), '#5d4028'));
      parts.push(
        paint(new THREE.SphereGeometry(0.78, 7, 5).scale(1, 0.55, 1).translate(0, 1.35, 0), '#1d6b2e'),
      );
      parts.push(
        paint(new THREE.SphereGeometry(0.5, 6, 4).scale(1, 0.55, 1).translate(0.25, 1.66, 0.12), '#2a7d3c'),
      );
      break;
    }
    case FloraType.Cactus: {
      parts.push(paint(new THREE.CylinderGeometry(0.2, 0.24, 1.2, 6).translate(0, 0.6, 0), '#5f8a4a'));
      parts.push(
        paint(
          new THREE.CylinderGeometry(0.09, 0.11, 0.55, 5).rotateZ(Math.PI / 2.6).translate(0.3, 0.78, 0),
          '#547e42',
        ),
      );
      break;
    }
    case FloraType.Acacia: {
      // Umbrella-canopy savanna tree: tall thin trunk, flat spreading top.
      parts.push(paint(new THREE.CylinderGeometry(0.07, 0.11, 1.05, 5).translate(0, 0.52, 0), TRUNK));
      parts.push(paint(new THREE.CylinderGeometry(0.95, 0.5, 0.34, 7).translate(0, 1.25, 0), '#7ca03e'));
      break;
    }
    case FloraType.Palm: {
      parts.push(paint(new THREE.CylinderGeometry(0.08, 0.12, 1.0, 5).translate(0, 0.5, 0), '#7d6243'));
      parts.push(
        paint(new THREE.CylinderGeometry(0.06, 0.08, 0.75, 5).rotateZ(-0.16).translate(0.1, 1.32, 0), '#7d6243'),
      );
      // Fronds: flattened cones splayed outward from the crown.
      for (let k = 0; k < 5; k++) {
        const a = (k / 5) * Math.PI * 2;
        parts.push(
          paint(
            new THREE.ConeGeometry(0.1, 0.85, 4)
              .rotateX(Math.PI / 2.15)
              .rotateY(a)
              .translate(0.16 + Math.cos(a) * 0.3, 1.72, Math.sin(a) * 0.3),
            k % 2 ? '#2e7d3f' : '#35914a',
          ),
        );
      }
      break;
    }
    case FloraType.DeadTree: {
      parts.push(paint(new THREE.CylinderGeometry(0.06, 0.11, 1.15, 5).translate(0, 0.57, 0), '#6e5b48'));
      parts.push(
        paint(new THREE.CylinderGeometry(0.03, 0.05, 0.55, 4).rotateZ(0.75).translate(0.2, 0.98, 0), '#665441'),
      );
      parts.push(
        paint(
          new THREE.CylinderGeometry(0.025, 0.045, 0.45, 4).rotateZ(-0.9).rotateY(1.2).translate(-0.16, 0.8, 0.05),
          '#665441',
        ),
      );
      break;
    }
    case FloraType.Bush: {
      parts.push(paint(new THREE.SphereGeometry(0.4, 5, 4).translate(0, 0.34, 0), '#5d7c3a'));
      parts.push(paint(new THREE.SphereGeometry(0.27, 5, 4).translate(0.26, 0.28, 0.1), '#6b8a42'));
      break;
    }
    case FloraType.Reed: {
      parts.push(paint(new THREE.CylinderGeometry(0.02, 0.035, 0.95, 4).translate(0, 0.47, 0), '#9aa86a'));
      parts.push(
        paint(new THREE.CylinderGeometry(0.018, 0.03, 0.8, 4).rotateZ(0.12).translate(0.09, 0.4, 0.04), '#8c9c5e'),
      );
      parts.push(
        paint(new THREE.CylinderGeometry(0.018, 0.03, 0.7, 4).rotateZ(-0.14).translate(-0.08, 0.35, -0.03), '#a5b273'),
      );
      break;
    }
    case FloraType.Tuft: {
      parts.push(paint(new THREE.ConeGeometry(0.17, 0.38, 5).translate(0, 0.19, 0), '#7fae4a'));
      break;
    }
    case FloraType.Rock:
    default: {
      parts.push(paint(new THREE.DodecahedronGeometry(0.6, 0), '#8a847e'));
      break;
    }
  }
  const merged = mergeGeometries(parts, false) ?? parts[0];
  parts.forEach((p) => p !== merged && p.dispose());
  return merged;
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
        const lum = 0.82 + h * 0.36; // ±18% brightness
        const warm = 0.96 + hash01(world.floraZ[i], world.floraX[i]) * 0.08; // slight hue drift
        tint.setRGB(lum * warm, lum, lum * (2 - warm));
        inst.setColorAt(k, tint);
      }
    }
    if (inst.instanceMatrix) inst.instanceMatrix.needsUpdate = true;
    if (inst.instanceColor) inst.instanceColor.needsUpdate = true;
  }, [indices, world, renderSize, heightRatio, meshResolution, baseSize, groundLift]);

  if (indices.length === 0) return null;
  return (
    <instancedMesh
      ref={ref}
      args={[geometry, undefined as any, indices.length]}
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
  baseSize = 1.4,
  quality = 'high',
}) => {
  const geoms = useMemo(() => TYPES.map((type) => ({ type, geometry: makeGeometry(type) })), []);
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
