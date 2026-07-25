import React, { useMemo, useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils.js';
import type { World } from './utils/worldGen';
import { Biome } from './utils/worldGen';
import { sampleMeshHeight } from './utils/worldSample';

// ---------------------------------------------------------------------------------------
// WorldWildlife — ambient creatures, with a bias toward FRESH WATER life:
//   ducks   paddling slow circles on lakes and ponds
//   herons  standing statue-still at lake shores and river banks
//   butterflies fluttering over riverbanks and flower meadows
//   deer    grazing in small herds on open ground; white goats on the alpine slopes
// Five instanced meshes total. Placement is deterministic (hash probes over the world
// fields), so every visit finds the same animals in the same haunts.
// ---------------------------------------------------------------------------------------

export interface WorldWildlifeProps {
  world: World;
  renderSize?: number;
  heightRatio?: number;
  meshResolution?: number;
}

function hash01(n: number): number {
  let h = Math.imul(n + 1, 374761393);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

/** Fills a geometry's vertex colours with one flat colour (two-tone merged bodies). */
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

function merged(parts: THREE.BufferGeometry[]): THREE.BufferGeometry {
  const m = mergeGeometries(parts, false) ?? parts[0];
  parts.forEach((p) => p !== m && p.dispose());
  return m;
}

/** Mallard-ish duck floating low, pointing +X. */
function makeDuck(): THREE.BufferGeometry {
  return merged([
    paint(new THREE.SphereGeometry(0.22, 6, 5).scale(1.35, 0.75, 0.9).translate(0, 0.08, 0), '#8a6b4a'),
    paint(new THREE.SphereGeometry(0.1, 5, 4).translate(0.24, 0.26, 0), '#2e5d3a'),
    paint(new THREE.ConeGeometry(0.045, 0.14, 4).rotateZ(-Math.PI / 2).translate(0.36, 0.24, 0), '#d8a13c'),
  ]);
}

/** Grey heron standing tall, pointing +X. */
function makeHeron(): THREE.BufferGeometry {
  return merged([
    paint(new THREE.CylinderGeometry(0.018, 0.018, 0.45, 4).translate(0, 0.225, 0.05), '#4a4a42'),
    paint(new THREE.CylinderGeometry(0.018, 0.018, 0.45, 4).translate(0, 0.225, -0.05), '#4a4a42'),
    paint(new THREE.SphereGeometry(0.16, 6, 5).scale(1.4, 1.0, 0.85).translate(0, 0.58, 0), '#ccd2d5'),
    paint(new THREE.CylinderGeometry(0.032, 0.05, 0.42, 4).rotateZ(-0.32).translate(0.14, 0.86, 0), '#ccd2d5'),
    paint(new THREE.SphereGeometry(0.06, 5, 4).translate(0.26, 1.05, 0), '#b9c0c4'),
    paint(new THREE.ConeGeometry(0.025, 0.2, 4).rotateZ(-Math.PI / 2).translate(0.4, 1.04, 0), '#c9b458'),
  ]);
}

/** Butterfly: tiny shallow-V wings (flap = Y-squash), neutral base for bright tints. */
function makeButterfly(): THREE.BufferGeometry {
  const geom = new THREE.BufferGeometry();
  const verts = new Float32Array([
    0, 0, 0.09, 0, 0, -0.09, -0.28, 0.14, 0,
    0, 0, 0.09, 0.28, 0.14, 0, 0, 0, -0.09,
  ]);
  geom.setAttribute('position', new THREE.BufferAttribute(verts, 3));
  const colors = new Float32Array(6 * 3).fill(1);
  geom.setAttribute('color', new THREE.BufferAttribute(colors, 3));
  geom.computeVertexNormals();
  return geom;
}

/** Low-poly deer pointing +X (body, four legs, neck, head). */
function makeDeer(): THREE.BufferGeometry {
  const legs: THREE.BufferGeometry[] = [];
  for (const lx of [-0.28, 0.28]) {
    for (const lz of [-0.12, 0.12]) {
      legs.push(paint(new THREE.CylinderGeometry(0.045, 0.04, 0.5, 4).translate(lx, 0.25, lz), '#74553a'));
    }
  }
  return merged([
    paint(new THREE.BoxGeometry(0.85, 0.4, 0.36).translate(0, 0.66, 0), '#8a6844'),
    ...legs,
    paint(new THREE.CylinderGeometry(0.06, 0.09, 0.42, 4).rotateZ(-0.65).translate(0.5, 0.95, 0), '#8a6844'),
    paint(new THREE.BoxGeometry(0.24, 0.15, 0.13).translate(0.68, 1.12, 0), '#7c5c3c'),
  ]);
}

interface Spot {
  x: number;
  z: number;
  y: number;
  yaw: number;
  s: number;
  seed: number;
}

export const WorldWildlife: React.FC<WorldWildlifeProps> = ({
  world,
  renderSize = 400,
  heightRatio = 0.13,
  meshResolution = 384,
}) => {
  const heightUnits = renderSize * heightRatio;

  const duckGeom = useMemo(() => makeDuck(), []);
  const heronGeom = useMemo(() => makeHeron(), []);
  const butterflyGeom = useMemo(() => makeButterfly(), []);
  const deerGeom = useMemo(() => makeDeer(), []);

  // ---- deterministic placement over the world fields ----
  const spots = useMemo(() => {
    const { size, water, temperature, lakeBasins, riverAmt, shore, biome, slope, elevation, seaLevel } = world;
    const toWorld = (g: number) => (g / (size - 1) - 0.5) * renderSize;
    const groundY = (gx: number, gy: number) =>
      sampleMeshHeight(world, gx / (size - 1), gy / (size - 1), meshResolution) * heightUnits;

    // Ducks: a few per sizeable unfrozen basin, floating at the spill level.
    const ducks: Spot[] = [];
    for (let bi = 0; bi < (lakeBasins?.length ?? 0) && ducks.length < 48; bi++) {
      const b = lakeBasins[bi];
      if ((b.maxX - b.minX) * (b.maxY - b.minY) < 40) continue;
      const ci = Math.round((b.minY + b.maxY) / 2) * size + Math.round((b.minX + b.maxX) / 2);
      if ((temperature?.[ci] ?? 0.5) < 0.19) continue; // frozen
      const nDucks = 2 + Math.floor(hash01(bi * 7) * 3);
      for (let d = 0; d < nDucks; d++) {
        // Probe random bbox cells for genuinely flooded water.
        for (let attempt = 0; attempt < 16; attempt++) {
          const gx = b.minX + Math.floor(hash01(bi * 97 + d * 13 + attempt) * (b.maxX - b.minX + 1));
          const gy = b.minY + Math.floor(hash01(bi * 61 + d * 17 + attempt) * (b.maxY - b.minY + 1));
          const gi = gy * size + gx;
          if ((water?.[gi] ?? 0) <= 0) continue;
          ducks.push({
            x: toWorld(gx),
            z: toWorld(gy),
            y: b.level * heightUnits + 0.05,
            yaw: hash01(bi * 131 + d) * Math.PI * 2,
            s: 0.9 + hash01(bi * 61 + d) * 0.4,
            seed: bi * 100 + d,
          });
          break;
        }
      }
    }

    // Herons: statue-still at lake shores and river banks.
    const herons: Spot[] = [];
    for (let probe = 0; probe < 30000 && herons.length < 32; probe++) {
      const gx = 2 + Math.floor(hash01(probe * 3 + 11) * (size - 4));
      const gy = 2 + Math.floor(hash01(probe * 3 + 12) * (size - 4));
      const gi = gy * size + gx;
      if (elevation[gi] <= seaLevel || (water?.[gi] ?? 0) > 0 || (riverAmt?.[gi] ?? 0) > 40) continue;
      if ((temperature?.[gi] ?? 0.5) < 0.3 || (slope?.[gi] ?? 1) > 0.3) continue;
      const nearRiver =
        (riverAmt?.[gi - 2] ?? 0) > 150 ||
        (riverAmt?.[gi + 2] ?? 0) > 150 ||
        (riverAmt?.[gi - 2 * size] ?? 0) > 150 ||
        (riverAmt?.[gi + 2 * size] ?? 0) > 150;
      const nearStill = (shore?.[gi] ?? 0) > 0.85;
      if (!nearRiver && !nearStill) continue;
      herons.push({
        x: toWorld(gx),
        z: toWorld(gy),
        y: groundY(gx, gy),
        yaw: hash01(probe * 7) * Math.PI * 2,
        s: 1.1 + hash01(probe * 11) * 0.35,
        seed: probe,
      });
    }

    // Butterflies: over riverbanks and warm flower meadows.
    const butterflies: Spot[] = [];
    for (let probe = 0; probe < 30000 && butterflies.length < 52; probe++) {
      const gx = 2 + Math.floor(hash01(probe * 5 + 71) * (size - 4));
      const gy = 2 + Math.floor(hash01(probe * 5 + 72) * (size - 4));
      const gi = gy * size + gx;
      if (elevation[gi] <= seaLevel || (water?.[gi] ?? 0) > 0) continue;
      const bio = biome[gi];
      const meadow = bio === Biome.Grassland || bio === Biome.Forest || bio === Biome.Shrubland || bio === Biome.Swamp;
      if (!meadow || (temperature?.[gi] ?? 0) < 0.42 || (slope?.[gi] ?? 1) > 0.35) continue;
      const nearWater = (shore?.[gi] ?? 0) > 0.35 || (riverAmt?.[gi + 2] ?? 0) > 80 || (riverAmt?.[gi - 2] ?? 0) > 80;
      if (!nearWater && hash01(probe * 13) > 0.35) continue; // most flutter near water
      butterflies.push({
        x: toWorld(gx),
        z: toWorld(gy),
        y: groundY(gx, gy),
        yaw: 0,
        s: 0.8 + hash01(probe * 17) * 0.5,
        seed: probe,
      });
    }

    // Deer herds on open ground; goats up on the alpine slopes.
    const deer: Spot[] = [];
    for (let probe = 0; probe < 30000 && deer.length < 46; probe++) {
      const gx = 4 + Math.floor(hash01(probe * 9 + 313) * (size - 8));
      const gy = 4 + Math.floor(hash01(probe * 9 + 314) * (size - 8));
      const gi = gy * size + gx;
      if (elevation[gi] <= seaLevel || (water?.[gi] ?? 0) > 0 || (riverAmt?.[gi] ?? 0) > 0) continue;
      const bio = biome[gi];
      const graze = bio === Biome.Grassland || bio === Biome.Shrubland || bio === Biome.Savanna || bio === Biome.Forest;
      if (!graze || (slope?.[gi] ?? 1) > 0.25) continue;
      const herdN = 2 + Math.floor(hash01(probe * 11) * 3);
      for (let d = 0; d < herdN && deer.length < 46; d++) {
        const ox = (hash01(probe * 41 + d * 3) - 0.5) * 9;
        const oz = (hash01(probe * 43 + d * 5) - 0.5) * 9;
        const wx = toWorld(gx) + ox;
        const wz = toWorld(gy) + oz;
        // HABITAT CHECK on the member's own landing cell — the herd anchor being on grass
        // does not mean an offset 4 units away is: without this, deer spread straight into
        // the lake or the river channel and stand there half-drowned.
        const mgx = Math.min(size - 1, Math.max(0, Math.round((wx / renderSize + 0.5) * (size - 1))));
        const mgy = Math.min(size - 1, Math.max(0, Math.round((wz / renderSize + 0.5) * (size - 1))));
        const mi = mgy * size + mgx;
        if (elevation[mi] <= seaLevel || (water?.[mi] ?? 0) > 0 || (riverAmt?.[mi] ?? 0) > 0) continue;
        if ((slope?.[mi] ?? 1) > 0.3) continue;
        const u = wx / renderSize + 0.5;
        const v = wz / renderSize + 0.5;
        deer.push({
          x: wx,
          z: wz,
          y: sampleMeshHeight(world, u, v, meshResolution) * heightUnits,
          yaw: hash01(probe * 51 + d) * Math.PI * 2,
          s: 1.0 + hash01(probe * 53 + d) * 0.35,
          seed: probe * 10 + d,
        });
      }
      probe += 220; // spread herds apart
    }
    const goats: Spot[] = [];
    for (let probe = 0; probe < 30000 && goats.length < 16; probe++) {
      const gx = 4 + Math.floor(hash01(probe * 15 + 977) * (size - 8));
      const gy = 4 + Math.floor(hash01(probe * 15 + 978) * (size - 8));
      const gi = gy * size + gx;
      const bio = biome[gi];
      if (bio !== Biome.Alpine && bio !== Biome.Rock) continue;
      if ((slope?.[gi] ?? 1) > 0.6 || elevation[gi] <= seaLevel) continue;
      goats.push({
        x: toWorld(gx),
        z: toWorld(gy),
        y: groundY(gx, gy),
        yaw: hash01(probe * 19) * Math.PI * 2,
        s: 0.75 + hash01(probe * 23) * 0.2,
        seed: probe,
      });
    }

    return { ducks, herons, butterflies, deer, goats };
  }, [world, renderSize, heightUnits, meshResolution]);

  const duckRef = useRef<THREE.InstancedMesh>(null);
  const heronRef = useRef<THREE.InstancedMesh>(null);
  const butterflyRef = useRef<THREE.InstancedMesh>(null);
  const deerRef = useRef<THREE.InstancedMesh>(null);
  const goatRef = useRef<THREE.InstancedMesh>(null);

  useFrame((state) => {
    const t = state.clock.getElapsedTime();
    const dummy = new THREE.Object3D();

    // Ducks paddle a lazy little circle and bob on the swell.
    const ducks = duckRef.current;
    if (ducks && typeof ducks.setMatrixAt === 'function') {
      for (let i = 0; i < spots.ducks.length; i++) {
        const p = spots.ducks[i];
        const a = t * (0.12 + hash01(p.seed) * 0.1) + p.yaw;
        const r = 1.2 + hash01(p.seed * 3) * 1.6;
        dummy.position.set(p.x + Math.cos(a) * r, p.y + Math.sin(t * 1.1 + p.seed) * 0.06, p.z + Math.sin(a) * r);
        dummy.rotation.set(0, -a - Math.PI / 2, 0);
        dummy.scale.setScalar(p.s);
        dummy.updateMatrix();
        ducks.setMatrixAt(i, dummy.matrix);
      }
      if (ducks.instanceMatrix) ducks.instanceMatrix.needsUpdate = true;
    }

    // Butterflies flutter in a small wandering loop above the flowers.
    const flies = butterflyRef.current;
    if (flies && typeof flies.setMatrixAt === 'function') {
      const tint = new THREE.Color();
      for (let i = 0; i < spots.butterflies.length; i++) {
        const p = spots.butterflies[i];
        const wx = Math.sin(t * 0.5 + p.seed) * 2.2 + Math.sin(t * 1.3 + p.seed * 2) * 0.7;
        const wz = Math.cos(t * 0.4 + p.seed * 3) * 2.2;
        const wy = 1.2 + Math.sin(t * 0.9 + p.seed) * 0.7;
        dummy.position.set(p.x + wx, p.y + wy, p.z + wz);
        dummy.rotation.set(0, Math.atan2(-Math.cos(t * 0.5 + p.seed), Math.sin(t * 0.4 + p.seed * 3)), 0);
        const flap = 0.25 + Math.abs(Math.sin(t * 11 + p.seed)) * 1.0;
        dummy.scale.set(p.s, p.s * flap, p.s);
        dummy.updateMatrix();
        flies.setMatrixAt(i, dummy.matrix);
        if (typeof flies.setColorAt === 'function' && t < 0.5) {
          const h = hash01(p.seed * 7);
          if (h < 0.3) tint.setRGB(1.1, 1.05, 0.95);
          else if (h < 0.55) tint.setRGB(1.15, 0.85, 0.3);
          else if (h < 0.8) tint.setRGB(1.15, 0.6, 0.5);
          else tint.setRGB(0.6, 0.75, 1.15);
          flies.setColorAt(i, tint);
        }
      }
      if (flies.instanceMatrix) flies.instanceMatrix.needsUpdate = true;
      if (flies.instanceColor && t < 0.5) flies.instanceColor.needsUpdate = true;
    }

    // Herons stand still; deer and goats just breathe/graze with a tiny bob.
    const still = (ref: React.RefObject<THREE.InstancedMesh>, arr: Spot[], bob: number) => {
      const inst = ref.current;
      if (!inst || typeof inst.setMatrixAt !== 'function') return;
      for (let i = 0; i < arr.length; i++) {
        const p = arr[i];
        dummy.position.set(p.x, p.y + Math.abs(Math.sin(t * 0.7 + p.seed)) * bob, p.z);
        dummy.rotation.set(0, p.yaw, 0);
        dummy.scale.setScalar(p.s);
        dummy.updateMatrix();
        inst.setMatrixAt(i, dummy.matrix);
      }
      if (inst.instanceMatrix) inst.instanceMatrix.needsUpdate = true;
    };
    still(heronRef, spots.herons, 0);
    still(deerRef, spots.deer, 0.04);
    still(goatRef, spots.goats, 0.03);
  });

  return (
    <group name="world-wildlife">
      {spots.ducks.length > 0 && (
        <instancedMesh ref={duckRef} args={[duckGeom, undefined as any, spots.ducks.length]} name="wildlife-ducks">
          <meshStandardMaterial vertexColors roughness={0.9} metalness={0} flatShading />
        </instancedMesh>
      )}
      {spots.herons.length > 0 && (
        <instancedMesh ref={heronRef} args={[heronGeom, undefined as any, spots.herons.length]} name="wildlife-herons">
          <meshStandardMaterial vertexColors roughness={0.9} metalness={0} flatShading />
        </instancedMesh>
      )}
      {spots.butterflies.length > 0 && (
        <instancedMesh
          ref={butterflyRef}
          args={[butterflyGeom, undefined as any, spots.butterflies.length]}
          name="wildlife-butterflies"
          frustumCulled={false}
        >
          <meshBasicMaterial vertexColors side={THREE.DoubleSide} />
        </instancedMesh>
      )}
      {spots.deer.length > 0 && (
        <instancedMesh ref={deerRef} args={[deerGeom, undefined as any, spots.deer.length]} name="wildlife-deer" castShadow>
          <meshStandardMaterial vertexColors roughness={0.9} metalness={0} flatShading />
        </instancedMesh>
      )}
      {spots.goats.length > 0 && (
        <instancedMesh ref={goatRef} args={[deerGeom, undefined as any, spots.goats.length]} name="wildlife-goats" castShadow>
          <meshStandardMaterial color="#e8e4da" roughness={0.9} metalness={0} flatShading />
        </instancedMesh>
      )}
    </group>
  );
};

export default WorldWildlife;
