import React, { useMemo, useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { Biome } from './utils/worldGen';
import { sampleMeshHeight } from './utils/worldSample';
import { sceneElapsed } from './utils/sceneClock';
import { grazePosition, grazeYaw } from './utils/grazing';
import { hash01, merged, paint } from './utils/lowPoly';

// ---------------------------------------------------------------------------------------
// WorldFauna — the biomes `WorldWildlife` leaves empty.
//
// That component covers water's edge and open pasture: ducks, herons, butterflies, deer, goats. It
// grew one hand-written placement loop per species, which is fine for five and unmaintainable at
// twelve — so this one is a table. Adding a species is a row plus a geometry, and every row gets the
// same habitat probe, the same grazing path and the same one-draw-call instancing for free.
//
// Placement is deterministic over the world fields, like everything else in the scene: the same
// world always produces the same animals in the same haunts, and a frozen capture clock freezes
// them where they stand.
// ---------------------------------------------------------------------------------------

export interface WorldFaunaProps {
  world: World;
  renderSize?: number;
  heightRatio?: number;
  meshResolution?: number;
}

/** Camel: humped desert body on long legs, pointing +X. */
function makeCamel(): THREE.BufferGeometry {
  const legs: THREE.BufferGeometry[] = [];
  for (const lx of [-0.3, 0.3]) {
    for (const lz of [-0.14, 0.14]) {
      legs.push(paint(new THREE.CylinderGeometry(0.05, 0.045, 0.62, 4).translate(lx, 0.31, lz), '#b8925c'));
    }
  }
  return merged([
    paint(new THREE.BoxGeometry(0.92, 0.4, 0.4).translate(0, 0.8, 0), '#caa269'),
    paint(new THREE.SphereGeometry(0.22, 5, 4).scale(1, 0.9, 0.85).translate(-0.05, 1.06, 0), '#caa269'),
    ...legs,
    paint(new THREE.CylinderGeometry(0.07, 0.1, 0.5, 4).rotateZ(-0.5).translate(0.52, 1.1, 0), '#caa269'),
    paint(new THREE.BoxGeometry(0.26, 0.16, 0.14).translate(0.74, 1.32, 0), '#b8925c'),
  ]);
}

/** Musk ox: low shaggy block for tundra and taiga. */
function makeMuskox(): THREE.BufferGeometry {
  const legs: THREE.BufferGeometry[] = [];
  for (const lx of [-0.26, 0.26]) {
    for (const lz of [-0.16, 0.16]) {
      legs.push(paint(new THREE.CylinderGeometry(0.06, 0.055, 0.3, 4).translate(lx, 0.15, lz), '#3b322c'));
    }
  }
  return merged([
    paint(new THREE.BoxGeometry(0.9, 0.46, 0.5).translate(0, 0.53, 0), '#4a3f36'),
    ...legs,
    paint(new THREE.BoxGeometry(0.3, 0.28, 0.34).translate(0.55, 0.5, 0), '#2f2822'),
    paint(new THREE.CylinderGeometry(0.05, 0.02, 0.24, 4).rotateZ(1.2).translate(0.62, 0.66, 0.17), '#c9bda4'),
    paint(new THREE.CylinderGeometry(0.05, 0.02, 0.24, 4).rotateZ(1.2).translate(0.62, 0.66, -0.17), '#c9bda4'),
  ]);
}

/** Wild boar: forest body, snout forward. */
function makeBoar(): THREE.BufferGeometry {
  const legs: THREE.BufferGeometry[] = [];
  for (const lx of [-0.22, 0.22]) {
    for (const lz of [-0.11, 0.11]) {
      legs.push(paint(new THREE.CylinderGeometry(0.035, 0.03, 0.26, 4).translate(lx, 0.13, lz), '#2e2620'));
    }
  }
  return merged([
    paint(new THREE.BoxGeometry(0.66, 0.32, 0.3).translate(0, 0.42, 0), '#4b3b2f'),
    ...legs,
    paint(new THREE.ConeGeometry(0.16, 0.34, 5).rotateZ(-Math.PI / 2).translate(0.44, 0.44, 0), '#3f3128'),
    paint(new THREE.ConeGeometry(0.03, 0.12, 4).rotateZ(-1.9).translate(0.56, 0.42, 0.07), '#e0d6bd'),
  ]);
}

/** Rabbit: small crouched body with two upright ears. */
function makeRabbit(): THREE.BufferGeometry {
  return merged([
    paint(new THREE.SphereGeometry(0.14, 5, 4).scale(1.4, 0.9, 0.95).translate(0, 0.14, 0), '#9c8d7a'),
    paint(new THREE.SphereGeometry(0.09, 5, 4).translate(0.16, 0.24, 0), '#a89a86'),
    paint(new THREE.BoxGeometry(0.03, 0.16, 0.05).translate(0.15, 0.38, 0.05), '#8d7f6d'),
    paint(new THREE.BoxGeometry(0.03, 0.16, 0.05).translate(0.15, 0.38, -0.05), '#8d7f6d'),
    paint(new THREE.SphereGeometry(0.05, 4, 3).translate(-0.19, 0.16, 0), '#efeae0'),
  ]);
}

/** Sea turtle hauled out on the sand: domed shell, four flippers. */
function makeTurtle(): THREE.BufferGeometry {
  return merged([
    paint(new THREE.SphereGeometry(0.26, 7, 5).scale(1.15, 0.5, 1).translate(0, 0.12, 0), '#4e6b43'),
    paint(new THREE.SphereGeometry(0.09, 5, 4).translate(0.3, 0.1, 0), '#6b7f5c'),
    paint(new THREE.BoxGeometry(0.2, 0.04, 0.1).rotateY(0.5).translate(0.16, 0.06, 0.22), '#5d7350'),
    paint(new THREE.BoxGeometry(0.2, 0.04, 0.1).rotateY(-0.5).translate(0.16, 0.06, -0.22), '#5d7350'),
    paint(new THREE.BoxGeometry(0.18, 0.04, 0.09).rotateY(-0.5).translate(-0.16, 0.06, 0.2), '#5d7350'),
    paint(new THREE.BoxGeometry(0.18, 0.04, 0.09).rotateY(0.5).translate(-0.16, 0.06, -0.2), '#5d7350'),
  ]);
}

/** Frog: wetland squat with folded hind legs. */
function makeFrog(): THREE.BufferGeometry {
  return merged([
    paint(new THREE.SphereGeometry(0.11, 5, 4).scale(1.2, 0.8, 1).translate(0, 0.08, 0), '#4c7a3a'),
    paint(new THREE.SphereGeometry(0.035, 4, 3).translate(0.06, 0.16, 0.05), '#e8e2c0'),
    paint(new THREE.SphereGeometry(0.035, 4, 3).translate(0.06, 0.16, -0.05), '#e8e2c0'),
    paint(new THREE.BoxGeometry(0.09, 0.04, 0.04).rotateY(0.6).translate(-0.08, 0.05, 0.09), '#3f6832'),
    paint(new THREE.BoxGeometry(0.09, 0.04, 0.04).rotateY(-0.6).translate(-0.08, 0.05, -0.09), '#3f6832'),
  ]);
}

/**
 * One kind of animal: what it looks like, where it lives, and how it moves.
 *
 * `speed` and `wander` are what make a herd of musk oxen read differently from a warren of rabbits
 * without either needing its own frame-loop branch.
 */
interface Species {
  id: string;
  geometry: () => THREE.BufferGeometry;
  /** Biomes it will settle in. */
  biomes: readonly Biome[];
  /** Upper bound on how many the world gets. */
  cap: number;
  /** Base model scale, jittered per individual. */
  scale: number;
  /** Steepest ground it will stand on. */
  maxSlope: number;
  /** How far it may roam from where it settled, in render units. */
  wander: number;
  /** Radians per second along its grazing path. */
  speed: number;
  /** Vertical breathing amplitude. */
  bob: number;
}

const SPECIES: readonly Species[] = [
  { id: 'camel', geometry: makeCamel, biomes: [Biome.Desert, Biome.Badlands], cap: 40, scale: 1.15, maxSlope: 0.28, wander: 9, speed: 0.07, bob: 0.05 },
  { id: 'muskox', geometry: makeMuskox, biomes: [Biome.Tundra, Biome.Taiga, Biome.Snow], cap: 46, scale: 1.05, maxSlope: 0.3, wander: 6, speed: 0.06, bob: 0.04 },
  { id: 'boar', geometry: makeBoar, biomes: [Biome.Forest, Biome.Jungle], cap: 60, scale: 1.0, maxSlope: 0.34, wander: 5, speed: 0.14, bob: 0.03 },
  { id: 'rabbit', geometry: makeRabbit, biomes: [Biome.Grassland, Biome.Shrubland, Biome.Steppe, Biome.Chaparral], cap: 90, scale: 0.9, maxSlope: 0.3, wander: 4, speed: 0.26, bob: 0.09 },
  { id: 'turtle', geometry: makeTurtle, biomes: [Biome.Beach], cap: 34, scale: 1.0, maxSlope: 0.2, wander: 2.5, speed: 0.04, bob: 0.01 },
  { id: 'frog', geometry: makeFrog, biomes: [Biome.Swamp, Biome.Bog, Biome.Mangrove], cap: 70, scale: 1.0, maxSlope: 0.35, wander: 2, speed: 0.3, bob: 0.07 },
];

interface Placed {
  x: number;
  z: number;
  yaw: number;
  s: number;
  seed: number;
  r: number;
}

export const WorldFauna: React.FC<WorldFaunaProps> = ({
  world,
  renderSize = 400,
  heightRatio = 0.13,
  meshResolution = 384,
}) => {
  const heightUnits = renderSize * heightRatio;
  const geometries = useMemo(() => SPECIES.map((s) => s.geometry()), []);
  const refs = useRef<(THREE.InstancedMesh | null)[]>([]);

  const placements = useMemo(() => {
    const { size, water, riverAmt, biome, slope, elevation, seaLevel } = world;
    const toWorld = (g: number) => (g / (size - 1) - 0.5) * renderSize;

    const standable = (wx: number, wz: number, maxSlope: number) => {
      const u = wx / renderSize + 0.5;
      const v = wz / renderSize + 0.5;
      if (u < 0 || u > 1 || v < 0 || v > 1) return false;
      const gx = Math.min(size - 1, Math.max(0, Math.round(u * (size - 1))));
      const gy = Math.min(size - 1, Math.max(0, Math.round(v * (size - 1))));
      const gi = gy * size + gx;
      if (elevation[gi] <= seaLevel) return false;
      if ((water?.[gi] ?? 0) > 0 || (riverAmt?.[gi] ?? 0) > 0) return false;
      return (slope?.[gi] ?? 1) <= maxSlope;
    };

    // Same contract as `WorldWildlife`'s probe: the whole circle has to be standable, not just the
    // destination, or a path cuts across the water between two dry points.
    const roamRadius = (wx: number, wz: number, max: number, maxSlope: number) => {
      for (let r = max; r >= 1; r -= 1) {
        let ok = true;
        for (let k = 0; k < 8 && ok; k++) {
          const a = (k / 8) * Math.PI * 2;
          ok = standable(wx + Math.cos(a) * r, wz + Math.sin(a) * r, maxSlope);
        }
        if (ok) return r;
      }
      return 0;
    };

    return SPECIES.map((sp, si) => {
      const out: Placed[] = [];
      const wanted = new Set<number>(sp.biomes);
      // A different probe stride per species, so two species that share a biome do not land on the
      // same cells and stand inside one another.
      const salt = 977 + si * 131;
      for (let probe = 0; probe < 40000 && out.length < sp.cap; probe++) {
        const gx = 3 + Math.floor(hash01(probe * 13 + salt) * (size - 6));
        const gy = 3 + Math.floor(hash01(probe * 13 + salt + 1) * (size - 6));
        const gi = gy * size + gx;
        if (!wanted.has(biome[gi] as Biome)) continue;
        const wx = toWorld(gx);
        const wz = toWorld(gy);
        if (!standable(wx, wz, sp.maxSlope)) continue;
        out.push({
          x: wx,
          z: wz,
          yaw: hash01(probe * 31 + salt) * Math.PI * 2,
          s: sp.scale * (0.85 + hash01(probe * 37 + salt) * 0.35),
          seed: probe * 7 + si,
          r: roamRadius(wx, wz, sp.wander, sp.maxSlope),
        });
        probe += 40; // spread individuals out rather than clustering on one patch
      }
      return out;
    });
    // Placement reads the world's grids and the render extent, and nothing else: the terrain height
    // is sampled per frame instead, so `heightUnits` and `meshResolution` are deliberately not here.
  }, [world, renderSize]);

  useFrame((state) => {
    const t = sceneElapsed(state.clock);
    const dummy = new THREE.Object3D();
    for (let si = 0; si < SPECIES.length; si++) {
      const inst = refs.current[si];
      const arr = placements[si];
      if (!inst || typeof inst.setMatrixAt !== 'function' || arr.length === 0) continue;
      const sp = SPECIES[si];
      for (let i = 0; i < arr.length; i++) {
        const p = arr[i];
        const { x, z } = grazePosition(p, p.r, p.seed, t, sp.speed);
        const yaw = grazeYaw(p, p.r, p.seed, t, sp.speed, p.yaw);
        const u = x / renderSize + 0.5;
        const v = z / renderSize + 0.5;
        const y = sampleMeshHeight(world, u, v, meshResolution) * heightUnits;
        dummy.position.set(x, y + Math.abs(Math.sin(t * 1.6 + p.seed)) * sp.bob, z);
        dummy.rotation.set(0, yaw, 0);
        dummy.scale.setScalar(p.s);
        dummy.updateMatrix();
        inst.setMatrixAt(i, dummy.matrix);
      }
      if (inst.instanceMatrix) inst.instanceMatrix.needsUpdate = true;
    }
  });

  return (
    <group name="world-fauna">
      {SPECIES.map((sp, si) =>
        placements[si].length > 0 ? (
          <instancedMesh
            key={sp.id}
            ref={(m) => {
              refs.current[si] = m;
            }}
            args={[geometries[si], null, placements[si].length]}
            name={`fauna-${sp.id}`}
            castShadow
            receiveShadow
          >
            <meshStandardMaterial vertexColors roughness={0.9} metalness={0} flatShading />
          </instancedMesh>
        ) : null,
      )}
    </group>
  );
};

export default WorldFauna;
