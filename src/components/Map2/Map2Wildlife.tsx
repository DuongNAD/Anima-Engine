import React, { useEffect, useMemo, useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { sampleMap2Terrain, getMap2Height, getMap2NormalAndSteepness } from './utils/map2TerrainGen';
import { Map2Biome } from './utils/map2BiomeConfig';
import {
  Map2FloraSpecies,
  FLORA_TRUNK_COLLIDER_RADII,
} from './utils/map2FloraGeometry';
import {
  SpatialFloraColliderIndex,
  resolveFloraOverlap,
} from './utils/map2Physics';
import {
  makeBirdGeometry,
  makeFishGeometry,
  makeDeerGeometry,
  makeGoatGeometry,
  makeDuckGeometry,
  makeButterflyGeometry,
} from './utils/map2WildlifeGeometry';

// -------------------------------------------------------------------------------------------------
// Map 2 Autonomous Wildlife Simulation Engine (Boids Birds, Shoaling Fish & Terrestrial Herds)
// Features: Thermal Updraft Soaring, Gliding Aerodynamics, Centripetal Banking,
// 4-State Terrestrial Herd Behavioral Machine, Slope-Normal Quaternion Alignment,
// Obstacle Avoidance Feelers, Lake Bathymetry Bounding, and Zero-Jitter Ground Clamping.
// -------------------------------------------------------------------------------------------------

export interface Map2WildlifeProps {
  size?: number;
  waterLevel?: number;
  quality?: 'high' | 'low';
}

function hash01(n: number): number {
  let h = Math.imul((n + 1) | 0, 374761393);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

function hash2D(x: number, z: number, seed = 0): number {
  let h = Math.imul((x * 374761393) | 0, (z * 668265263) | 0) + Math.imul(seed, 1274126177);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

// -------------------------------------------------------------------------------------------------
// Thermal Updraft Vortices for Aerial Soaring
// -------------------------------------------------------------------------------------------------

interface ThermalUpdraft {
  cx: number;
  cz: number;
  radius: number;
  liftVelocity: number;
}

const MAP2_THERMAL_UPDRAFTS: ThermalUpdraft[] = [
  { cx: -45.0, cz: -35.0, radius: 28.0, liftVelocity: 5.0 }, // Jagged Rocky Cliffs
  { cx: 40.0, cz: -20.0, radius: 24.0, liftVelocity: 4.2 },  // Southern Ridge / Highlands
  { cx: -10.0, cz: -55.0, radius: 22.0, liftVelocity: 4.5 }, // Alpine Glacial Crags
];

function evaluateThermalUpdraft(x: number, z: number): {
  lift: number;
  steerX: number;
  steerZ: number;
  inThermal: boolean;
} {
  let totalLift = 0;
  let steerX = 0;
  let steerZ = 0;
  let inThermal = false;

  for (let k = 0; k < MAP2_THERMAL_UPDRAFTS.length; k++) {
    const th = MAP2_THERMAL_UPDRAFTS[k];
    const dx = th.cx - x;
    const dz = th.cz - z;
    const distSq = dx * dx + dz * dz;
    const rSq = th.radius * th.radius;

    if (distSq < rSq * 2.5) {
      const factor = Math.exp(-distSq / (2 * rSq));
      const lift = th.liftVelocity * factor;
      totalLift += lift;

      if (lift > 1.2) {
        inThermal = true;
        const dist = Math.sqrt(distSq) || 1.0;
        // Tangential circling vortex torque + gentle core attraction
        const tangentX = -dz / dist;
        const tangentZ = dx / dist;
        steerX += tangentX * 3.2 + (dx / dist) * 1.0;
        steerZ += tangentZ * 3.2 + (dz / dist) * 1.0;
      }
    }
  }

  return { lift: totalLift, steerX, steerZ, inThermal };
}

// -------------------------------------------------------------------------------------------------
// Simulation Data Interfaces
// -------------------------------------------------------------------------------------------------

interface BirdBoid {
  position: THREE.Vector3;
  velocity: THREE.Vector3;
  prevVelocity: THREE.Vector3;
  flockId: number;
  wingPhase: number;
  wingSpeed: number;
  scale: number;
  roll: number;
  isGliding: boolean;
}

const FISH_COLORS: readonly string[] = [
  '#38bdf8', // Sky blue trout
  '#fbbf24', // Golden carp
  '#f87171', // Salmon pink
  '#34d399', // Emerald minnow
  '#c084fc', // Violet bass
  '#e2e8f0', // Silver shad
] as const;

interface FishShoalMember {
  cx: number;
  cz: number;
  baseY: number;
  r: number;
  speed: number;
  phase: number;
  color: THREE.Color;
  scale: number;
  shoalId: number;
}

type TerrestrialFaunaState = 'GRAZING' | 'WANDERING' | 'RESTING' | 'FLEEING';

interface TerrestrialFaunaMember {
  species: 'deer' | 'goat';
  position: THREE.Vector3;
  velocity: THREE.Vector3;
  facing: THREE.Vector2; // Normalized horizontal forward look direction
  quaternion: THREE.Quaternion;
  state: TerrestrialFaunaState;
  stateTimer: number;
  scale: number;
  seed: number;
  herdHome: THREE.Vector2;
  stepPhase: number;
}

interface WaterDuckSpot {
  x: number;
  z: number;
  y: number;
  yaw: number;
  scale: number;
  seed: number;
}

interface ButterflySpot {
  x: number;
  z: number;
  y: number;
  scale: number;
  seed: number;
}

// -------------------------------------------------------------------------------------------------
// Fast Flora Trunk Spatial Collider Index Builder
// -------------------------------------------------------------------------------------------------

function buildFloraTrunkSpatialIndex(size = 200, waterLevel = 12.0): SpatialFloraColliderIndex {
  const index = new SpatialFloraColliderIndex(8.0);
  const halfSize = size * 0.48;
  const gridRes = 90;
  const step = (halfSize * 2) / gridRes;

  for (let gx = 0; gx < gridRes; gx++) {
    for (let gz = 0; gz < gridRes; gz++) {
      const jx = (hash2D(gx, gz, 1) - 0.5) * step * 0.92;
      const jz = (hash2D(gx, gz, 2) - 0.5) * step * 0.92;
      const wx = -halfSize + (gx + 0.5) * step + jx;
      const wz = -halfSize + (gz + 0.5) * step + jz;

      const distToSpawn = Math.sqrt(wx * wx + wz * wz);
      if (distToSpawn < 22.0) continue;

      const sample = sampleMap2Terrain(wx, wz);
      const { height, steepness, isWater, biomeId } = sample;
      if (isWater || height <= waterLevel || height > 52.0 || steepness > 0.42) continue;

      const gladeNoise = (Math.sin(wx * 0.038 + 1.2) * Math.cos(wz * 0.038 + 0.8) + 1.0) * 0.5;
      const isTreeGrove = gladeNoise > 0.50;
      const rSelect = hash2D(gx, gz, 7);
      const rScale = hash2D(gx, gz, 8);

      let sp: Map2FloraSpecies | null = null;
      let scale = 1.0;

      if (biomeId === Map2Biome.TEMPERATE_PLAINS) {
        if (isTreeGrove) {
          if (rSelect < 0.32) {
            sp = Map2FloraSpecies.BroadleafDeciduous;
            scale = 2.2 + rScale * 0.9;
          } else if (rSelect < 0.55) {
            sp = Map2FloraSpecies.ConiferPine;
            scale = 2.0 + rScale * 0.8;
          }
        } else if (rSelect >= 0.45 && rSelect < 0.49) {
          sp = Map2FloraSpecies.BroadleafDeciduous;
          scale = 2.8 + rScale * 0.8;
        }
      } else if (biomeId === Map2Biome.ARID_SAVANNA) {
        if (isTreeGrove && rSelect < 0.35) {
          sp = Map2FloraSpecies.UmbrellaAcacia;
          scale = 2.2 + rScale * 0.9;
        } else if (!isTreeGrove && rSelect >= 0.38 && rSelect < 0.42) {
          sp = Map2FloraSpecies.UmbrellaAcacia;
          scale = 2.5 + rScale * 0.8;
        }
      } else if (biomeId === Map2Biome.TROPICAL_RAINFOREST) {
        if (isTreeGrove) {
          if (rSelect < 0.32) {
            sp = Map2FloraSpecies.TropicalPalm;
            scale = 2.2 + rScale * 0.9;
          } else if (rSelect < 0.58) {
            sp = Map2FloraSpecies.LushJungleTree;
            scale = 2.4 + rScale * 1.0;
          }
        } else if (rSelect >= 0.55 && rSelect < 0.60) {
          sp = Map2FloraSpecies.TropicalPalm;
          scale = 2.4 + rScale * 0.8;
        }
      } else if (biomeId === Map2Biome.DESERT_DUNES && rSelect < 0.18) {
        sp = Map2FloraSpecies.SaguaroCactus;
        scale = 1.8 + rScale * 0.8;
      } else if (biomeId === Map2Biome.SNOW_PEAKS && height <= 48.0 && steepness <= 0.38 && rSelect < 0.25) {
        sp = Map2FloraSpecies.ConiferPine;
        scale = 1.6 + rScale * 0.7;
      } else if (biomeId === Map2Biome.ROCKY_CLIFFS && steepness <= 0.40 && rSelect >= 0.45 && rSelect < 0.60) {
        sp = Map2FloraSpecies.ConiferPine;
        scale = 1.5 + rScale * 0.6;
      }

      if (sp) {
        const baseRadius = FLORA_TRUNK_COLLIDER_RADII[sp] ?? 0.0;
        if (baseRadius > 0) {
          index.insert({
            x: wx,
            z: wz,
            radius: baseRadius * scale,
            species: sp,
          });
        }
      }
    }
  }

  return index;
}

// -------------------------------------------------------------------------------------------------
// Main Wildlife Component
// -------------------------------------------------------------------------------------------------

export const Map2Wildlife: React.FC<Map2WildlifeProps> = ({
  size = 200,
  waterLevel = 12.0,
  quality = 'high',
}) => {
  // Mesh refs
  const birdMeshRef = useRef<THREE.InstancedMesh>(null);
  const fishMeshRef = useRef<THREE.InstancedMesh>(null);
  const deerMeshRef = useRef<THREE.InstancedMesh>(null);
  const goatMeshRef = useRef<THREE.InstancedMesh>(null);
  const duckMeshRef = useRef<THREE.InstancedMesh>(null);
  const butterflyMeshRef = useRef<THREE.InstancedMesh>(null);

  // Scratch objects for zero-allocation 60 FPS rendering loops
  const scratchDummy = useMemo(() => new THREE.Object3D(), []);
  const scratchTint = useMemo(() => new THREE.Color(), []);
  const scratchRotMatrix = useMemo(() => new THREE.Matrix4(), []);
  const scratchTargetQuat = useMemo(() => new THREE.Quaternion(), []);
  const scratchHeadQuat = useMemo(() => new THREE.Quaternion(), []);

  // Geometries
  const birdGeom = useMemo(() => makeBirdGeometry(), []);
  const fishGeom = useMemo(() => makeFishGeometry(), []);
  const deerGeom = useMemo(() => makeDeerGeometry(), []);
  const goatGeom = useMemo(() => makeGoatGeometry(), []);
  const duckGeom = useMemo(() => makeDuckGeometry(), []);
  const butterflyGeom = useMemo(() => makeButterflyGeometry(), []);

  // Spatial index of solid tree colliders
  const floraIndex = useMemo(() => buildFloraTrunkSpatialIndex(size, waterLevel), [size, waterLevel]);

  // Dispose all geometries on unmount
  useEffect(() => {
    return () => {
      birdGeom.dispose();
      fishGeom.dispose();
      deerGeom.dispose();
      goatGeom.dispose();
      duckGeom.dispose();
      butterflyGeom.dispose();
    };
  }, [birdGeom, fishGeom, deerGeom, goatGeom, duckGeom, butterflyGeom]);

  // 1. Initialize Birds: Flock of 15 (High) / 9 (Low) birds soaring in thermal updrafts
  const birdCount = quality === 'high' ? 15 : 9;
  const birdsRef = useRef<BirdBoid[]>([]);

  useEffect(() => {
    const list: BirdBoid[] = [];
    const flockCenterX = 15.0;
    const flockCenterZ = -10.0;

    for (let i = 0; i < birdCount; i++) {
      const offsetR = 6 + hash01(i * 7) * 16;
      const offsetAngle = hash01(i * 11) * Math.PI * 2;
      const px = flockCenterX + Math.cos(offsetAngle) * offsetR;
      const pz = flockCenterZ + Math.sin(offsetAngle) * offsetR;
      const terrainH = getMap2Height(px, pz);
      const py = Math.max(terrainH + 20.0, 44.0 + hash01(i * 13) * 16.0);

      const heading = offsetAngle + Math.PI / 2;
      const speed = 7.5 + hash01(i * 17) * 2.5;
      const vx = Math.cos(heading) * speed;
      const vz = Math.sin(heading) * speed;

      list.push({
        position: new THREE.Vector3(px, py, pz),
        velocity: new THREE.Vector3(vx, 0, vz),
        prevVelocity: new THREE.Vector3(vx, 0, vz),
        flockId: 0,
        wingPhase: hash01(i * 19) * Math.PI * 2,
        wingSpeed: 7.5 + hash01(i * 23) * 2.5,
        scale: 1.3 + hash01(i * 29) * 0.3,
        roll: 0,
        isGliding: true,
      });
    }
    birdsRef.current = list;
  }, [birdCount, size]);

  // 2. Initialize Fish Shoals (1 cohesive shoal of 8 fish in Central Mountain Lake)
  const numShoals = 1;
  const fishPerShoal = quality === 'high' ? 8 : 5;
  const fishList = useMemo<FishShoalMember[]>(() => {
    const list: FishShoalMember[] = [];
    const cx = 35.0;
    const cz = 25.0; // Central mountain lake basin
    const shoalSpeed = 0.24;

    for (let s = 0; s < numShoals; s++) {
      for (let i = 0; i < fishPerShoal; i++) {
        const r = 4.5 + hash01(i * 3 + s * 17) * 7.5;
        const phase = (i / fishPerShoal) * Math.PI * 2 + hash01(i * 5) * 0.6;
        const bedH = getMap2Height(cx, cz);
        const submergedY = Math.max(bedH + 0.6, Math.min(waterLevel - 1.5, waterLevel - 2.2));
        const colorHex = FISH_COLORS[(i + s * 3) % FISH_COLORS.length];
        const shoalColor = new THREE.Color(colorHex);

        list.push({
          cx,
          cz,
          baseY: submergedY,
          r,
          speed: shoalSpeed + (hash01(i * 9) - 0.5) * 0.04,
          phase,
          color: shoalColor,
          scale: 0.95 + hash01(i) * 0.3,
          shoalId: s,
        });
      }
    }
    return list;
  }, [numShoals, fishPerShoal, waterLevel]);

  // 3. Initialize Terrestrial Fauna (Small natural herds: 5 Deer in meadow, 4 Goats on crags)
  const deerCount = 5;
  const goatCount = 4;
  const deerRef = useRef<TerrestrialFaunaMember[]>([]);
  const goatsRef = useRef<TerrestrialFaunaMember[]>([]);

  useEffect(() => {
    // 5 Woodland Deer centered at Temperate Plains meadow (-35, 20)
    const deerHerdHome = new THREE.Vector2(-35.0, 20.0);
    const deerList: TerrestrialFaunaMember[] = [];

    for (let i = 0; i < deerCount; i++) {
      const angle = (i / deerCount) * Math.PI * 2 + hash01(i * 7) * 0.5;
      const dist = 2.5 + hash01(i * 13) * 6.0;
      const px = deerHerdHome.x + Math.cos(angle) * dist;
      const pz = deerHerdHome.y + Math.sin(angle) * dist;
      const py = getMap2Height(px, pz);

      const lookAngle = hash01(i * 17) * Math.PI * 2;
      const facing = new THREE.Vector2(Math.cos(lookAngle), Math.sin(lookAngle));
      const quat = new THREE.Quaternion();

      // Initial slope-normal orientation
      const { normal } = getMap2NormalAndSteepness(px, pz);
      const fwd3D = new THREE.Vector3(facing.x, 0, facing.y);
      const dotN = fwd3D.dot(normal);
      const tFwd = fwd3D.clone().sub(normal.clone().multiplyScalar(dotN)).normalize();
      const tSide = new THREE.Vector3().crossVectors(tFwd, normal).normalize();
      const mat = new THREE.Matrix4().makeBasis(tFwd, normal, tSide);
      quat.setFromRotationMatrix(mat);

      deerList.push({
        species: 'deer',
        position: new THREE.Vector3(px, py, pz),
        velocity: new THREE.Vector3(0, 0, 0),
        facing,
        quaternion: quat,
        state: i % 2 === 0 ? 'GRAZING' : 'RESTING',
        stateTimer: 4.0 + hash01(i * 23) * 6.0,
        scale: 1.1 + hash01(i * 29) * 0.2,
        seed: i * 31,
        herdHome: deerHerdHome,
        stepPhase: hash01(i * 37) * Math.PI * 2,
      });
    }

    // 4 Alpine Goats centered at Rocky Cliffs crags (-52, -38)
    const goatHerdHome = new THREE.Vector2(-52.0, -38.0);
    const goatList: TerrestrialFaunaMember[] = [];

    for (let i = 0; i < goatCount; i++) {
      const angle = (i / goatCount) * Math.PI * 2 + hash01(i * 41) * 0.5;
      const dist = 2.0 + hash01(i * 43) * 5.0;
      const px = goatHerdHome.x + Math.cos(angle) * dist;
      const pz = goatHerdHome.y + Math.sin(angle) * dist;
      const py = getMap2Height(px, pz);

      const lookAngle = hash01(i * 47) * Math.PI * 2;
      const facing = new THREE.Vector2(Math.cos(lookAngle), Math.sin(lookAngle));
      const quat = new THREE.Quaternion();

      const { normal } = getMap2NormalAndSteepness(px, pz);
      const fwd3D = new THREE.Vector3(facing.x, 0, facing.y);
      const dotN = fwd3D.dot(normal);
      const tFwd = fwd3D.clone().sub(normal.clone().multiplyScalar(dotN)).normalize();
      const tSide = new THREE.Vector3().crossVectors(tFwd, normal).normalize();
      const mat = new THREE.Matrix4().makeBasis(tFwd, normal, tSide);
      quat.setFromRotationMatrix(mat);

      goatList.push({
        species: 'goat',
        position: new THREE.Vector3(px, py, pz),
        velocity: new THREE.Vector3(0, 0, 0),
        facing,
        quaternion: quat,
        state: i === 0 ? 'WANDERING' : 'GRAZING',
        stateTimer: 4.0 + hash01(i * 53) * 5.0,
        scale: 0.95 + hash01(i * 59) * 0.2,
        seed: i * 61 + 100,
        herdHome: goatHerdHome,
        stepPhase: hash01(i * 67) * Math.PI * 2,
      });
    }

    deerRef.current = deerList;
    goatsRef.current = goatList;
  }, [deerCount, goatCount]);

  // 4. Initialize Aquatic Ducks & Meadow Butterflies
  const ambientFauna = useMemo(() => {
    const duckList: WaterDuckSpot[] = [];
    const butterflyList: ButterflySpot[] = [];

    const halfSize = size * 0.45;
    const probeCount = 1000;

    for (let probe = 0; probe < probeCount; probe++) {
      const px = (hash01(probe * 7 + 1) - 0.5) * halfSize * 2;
      const pz = (hash01(probe * 7 + 2) - 0.5) * halfSize * 2;

      // Spawn clearance
      if (Math.sqrt(px * px + pz * pz) < 18.0) continue;

      const sample = sampleMap2Terrain(px, pz);
      const { height, steepness, isWater, biomeId } = sample;

      // Ducks on mountain lake
      if (isWater || height <= waterLevel) {
        if (duckList.length < 4 && Math.hypot(px - 35.0, pz - 25.0) < 22.0) {
          duckList.push({
            x: px,
            z: pz,
            y: waterLevel + 0.05,
            yaw: hash01(probe * 13) * Math.PI * 2,
            scale: 0.9 + hash01(probe * 17) * 0.2,
            seed: probe,
          });
        }
        continue;
      }

      // Butterflies in flower glades
      if (
        butterflyList.length < 12 &&
        steepness < 0.25 &&
        (biomeId === Map2Biome.TEMPERATE_PLAINS || biomeId === Map2Biome.TROPICAL_RAINFOREST)
      ) {
        if (hash01(probe * 43) > 0.6) {
          butterflyList.push({
            x: px,
            z: pz,
            y: height,
            scale: 0.8 + hash01(probe * 47) * 0.3,
            seed: probe,
          });
        }
      }
    }

    return { ducks: duckList, butterflies: butterflyList };
  }, [size, waterLevel]);

  // Per-Frame Simulation Loop (60 FPS WebGL2 Zero-Allocation Optimized)
  useFrame((state, delta) => {
    const t = state.clock.getElapsedTime();
    const dt = Math.min(delta, 0.05);
    const dummy = scratchDummy;
    const camPos = state.camera.position;

    // =========================================================================
    // SECTION A. Boids Birds Simulation with Thermal Updrafts & Aerodynamics
    // =========================================================================
    const birdMesh = birdMeshRef.current;
    const birds = birdsRef.current;
    if (birdMesh && birds.length > 0) {
      const halfBound = size * 0.48;

      for (let i = 0; i < birds.length; i++) {
        const b = birds[i];

        // 1. Reynolds 2D Boids steering forces (Separation, Cohesion, Alignment)
        let sepX = 0;
        let sepZ = 0;
        let cohX = 0;
        let cohZ = 0;
        let alignX = 0;
        let alignZ = 0;
        let neighborCount = 0;

        for (let j = 0; j < birds.length; j++) {
          if (i === j) continue;
          const other = birds[j];
          const dx = b.position.x - other.position.x;
          const dz = b.position.z - other.position.z;
          const distSq = dx * dx + dz * dz;

          if (distSq < 18.0 * 18.0 && distSq > 0.001) {
            const dist = Math.sqrt(distSq);
            // Separation (< 4.5m)
            if (dist < 4.5) {
              const rep = (4.5 - dist) * 3.5;
              sepX += (dx / dist) * rep;
              sepZ += (dz / dist) * rep;
            }
            // Cohesion & Alignment
            cohX += other.position.x;
            cohZ += other.position.z;
            alignX += other.velocity.x;
            alignZ += other.velocity.z;
            neighborCount++;
          }
        }

        if (neighborCount > 0) {
          cohX = (cohX / neighborCount - b.position.x) * 0.18;
          cohZ = (cohZ / neighborCount - b.position.z) * 0.18;
          alignX = (alignX / neighborCount - b.velocity.x) * 0.22;
          alignZ = (alignZ / neighborCount - b.velocity.z) * 0.22;

          b.velocity.x += (sepX + cohX + alignX) * dt;
          b.velocity.z += (sepZ + cohZ + alignZ) * dt;
        }

        // 2. Thermal Updraft Soaring Forces
        const { lift: thermalLift, steerX: thermalSteerX, steerZ: thermalSteerZ, inThermal } =
          evaluateThermalUpdraft(b.position.x, b.position.z);

        if (inThermal) {
          b.velocity.x += thermalSteerX * 0.45 * dt;
          b.velocity.z += thermalSteerZ * 0.45 * dt;
        }

        // 3. Boundary containment soft turnaround
        if (Math.abs(b.position.x) > halfBound) {
          b.velocity.x -= Math.sign(b.position.x) * 18.0 * dt;
        }
        if (Math.abs(b.position.z) > halfBound) {
          b.velocity.z -= Math.sign(b.position.z) * 18.0 * dt;
        }

        // 4. Horizontal speed limiting (6.5 m/s - 14.5 m/s)
        const curHorizSpeed = Math.hypot(b.velocity.x, b.velocity.z);
        if (curHorizSpeed > 14.5) {
          b.velocity.x = (b.velocity.x / curHorizSpeed) * 14.5;
          b.velocity.z = (b.velocity.z / curHorizSpeed) * 14.5;
        } else if (curHorizSpeed < 6.5) {
          const s = curHorizSpeed || 1.0;
          b.velocity.x = (b.velocity.x / s) * 6.5;
          b.velocity.z = (b.velocity.z / s) * 6.5;
        }

        // 5. Vertical Aerodynamics (Thermal lift, gliding vs flapping states)
        const terrainH = getMap2Height(b.position.x, b.position.z);
        const minAlt = terrainH + 12.0;
        const maxCeiling = 85.0;

        if (b.position.y < minAlt + 4.0 || b.velocity.y < -1.0) {
          // Low altitude or rapid descent -> FLAPPING climb
          b.isGliding = false;
          b.velocity.y += (3.6 - b.velocity.y) * 2.8 * dt;
        } else if (inThermal) {
          // Inside thermal column -> GLIDING soaring ascent
          b.isGliding = true;
          const targetAscent = Math.min(thermalLift, 4.8);
          b.velocity.y += (targetAscent - b.velocity.y) * 2.0 * dt;
          if (b.position.y > maxCeiling - 8.0) {
            b.velocity.y *= 0.92; // Damp ascent near ceiling
          }
        } else {
          // Outside thermal in open air -> Gentle unpowered GLIDE descent (-0.7 m/s)
          b.isGliding = true;
          b.velocity.y += (-0.75 - b.velocity.y) * 1.5 * dt;
        }

        // 6. Integrate position
        b.position.x += b.velocity.x * dt;
        b.position.y += b.velocity.y * dt;
        b.position.z += b.velocity.z * dt;

        // Hard altitude safety clamps
        if (b.position.y < minAlt) {
          b.position.y += (minAlt - b.position.y) * 5.0 * dt;
          b.velocity.y = Math.max(0, b.velocity.y);
        } else if (b.position.y > maxCeiling) {
          b.position.y -= (b.position.y - maxCeiling) * 3.0 * dt;
          b.velocity.y = Math.min(0, b.velocity.y);
        }

        // 7. Aerodynamic Kinematics: Centripetal Roll Banking & Smooth Pitch
        const ax = (b.velocity.x - b.prevVelocity.x) / Math.max(0.001, dt);
        const az = (b.velocity.z - b.prevVelocity.z) / Math.max(0.001, dt);
        b.prevVelocity.copy(b.velocity);

        const totalSpeed = b.velocity.length() || 1.0;
        // theta_roll = -arctan((v_x a_z - v_z a_x) / (g ||v||))
        const centripetalNumerator = b.velocity.x * az - b.velocity.z * ax;
        const centripetalDenominator = 9.81 * totalSpeed;
        const targetRoll = Math.max(-0.65, Math.min(0.65, -Math.atan(centripetalNumerator / centripetalDenominator)));
        b.roll = THREE.MathUtils.damp(b.roll, targetRoll, 6.0, dt);

        // Pitch & Yaw angles (Bird geometry local +Z is forward, +Y is up, +X is right)
        const pitch = -Math.asin(Math.max(-0.85, Math.min(0.85, b.velocity.y / totalSpeed)));
        const yaw = Math.atan2(b.velocity.x, b.velocity.z);

        // Wing flapping / gliding dihedral animation
        const flapScale = b.isGliding
          ? 0.95 + 0.1 * Math.sin(t * 1.2 + b.wingPhase)
          : 0.45 + Math.abs(Math.sin(t * b.wingSpeed + b.wingPhase)) * 0.95;

        dummy.position.copy(b.position);
        dummy.rotation.set(pitch, yaw, b.roll, 'YXZ');
        dummy.scale.set(b.scale, b.scale * flapScale, b.scale);
        dummy.updateMatrix();

        birdMesh.setMatrixAt(i, dummy.matrix);
      }
      if (birdMesh.instanceMatrix) birdMesh.instanceMatrix.needsUpdate = true;
    }

    // =========================================================================
    // SECTION B. Aquatic Shoaling Fish Simulation (Lake Bathymetry Bounded)
    // =========================================================================
    const fishMesh = fishMeshRef.current;
    if (fishMesh && fishList.length > 0) {
      for (let i = 0; i < fishList.length; i++) {
        const f = fishList[i];
        const rCurrent = f.r * (1.0 + 0.18 * Math.sin(t * 0.6 + f.phase));
        const angle = f.phase + t * f.speed;

        const fx = f.cx + Math.cos(angle) * rCurrent;
        const fz = f.cz + Math.sin(angle) * rCurrent;

        // Dynamic lake bed sampling: ensure deep water (> 1.5m below surface 12.0m, > 0.6m above bed)
        const bedH = getMap2Height(fx, fz);
        const maxFishY = waterLevel - 1.5;
        const minFishY = Math.min(maxFishY - 0.2, bedH + 0.6);
        const targetY = Math.min(maxFishY, Math.max(minFishY, f.baseY + Math.sin(t * 1.8 + i) * 0.25));

        // Tangent swimming heading (Fish geometry front along +X)
        const tangentAngle = angle + Math.PI / 2;
        const tailWobble = 0.85 + 0.15 * Math.sin(t * 7.5 + i * 2.0);

        dummy.position.set(fx, targetY, fz);
        dummy.rotation.set(0, -tangentAngle, 0);
        dummy.scale.set(f.scale * tailWobble, f.scale * 0.8, f.scale);
        dummy.updateMatrix();

        fishMesh.setMatrixAt(i, dummy.matrix);
        if (t < 0.5) {
          fishMesh.setColorAt(i, f.color);
        }
      }
      if (fishMesh.instanceMatrix) fishMesh.instanceMatrix.needsUpdate = true;
      if (fishMesh.instanceColor && t < 0.5) fishMesh.instanceColor.needsUpdate = true;
    }

    // =========================================================================
    // SECTION C. Terrestrial Fauna Simulation (4-State Herds, Quaternion Normal Alignment)
    // =========================================================================
    const deer = deerRef.current;
    const goats = goatsRef.current;

    const updateTerrestrialHerd = (
      mesh: THREE.InstancedMesh | null,
      herd: TerrestrialFaunaMember[],
      isGoat: boolean,
    ) => {
      if (!mesh || herd.length === 0) return;

      // 1. Compute herd centroid for cohesion
      let herdCenterX = 0;
      let herdCenterZ = 0;
      for (let i = 0; i < herd.length; i++) {
        herdCenterX += herd[i].position.x;
        herdCenterZ += herd[i].position.z;
      }
      herdCenterX /= herd.length;
      herdCenterZ /= herd.length;

      for (let i = 0; i < herd.length; i++) {
        const p = herd[i];

        // 2. Alert & Proximity Threat Detection (FLEEING from player camera)
        const dCam = Math.hypot(p.position.x - camPos.x, p.position.z - camPos.z);
        if (dCam < 8.5 && p.state !== 'FLEEING') {
          p.state = 'FLEEING';
          p.stateTimer = 4.5 + hash01(p.seed) * 2.5;
        }

        // 3. State Machine Transitions
        p.stateTimer -= dt;
        if (p.stateTimer <= 0) {
          if (p.state === 'GRAZING') {
            p.state = hash01(p.seed + t) < 0.65 ? 'WANDERING' : 'RESTING';
            p.stateTimer = 5.0 + hash01(p.seed + t * 2) * 5.0;
          } else if (p.state === 'WANDERING') {
            p.state = hash01(p.seed + t) < 0.55 ? 'GRAZING' : 'RESTING';
            p.stateTimer = 6.0 + hash01(p.seed + t * 2) * 6.0;
          } else if (p.state === 'RESTING') {
            p.state = hash01(p.seed + t) < 0.60 ? 'GRAZING' : 'WANDERING';
            p.stateTimer = 4.0 + hash01(p.seed + t * 2) * 4.0;
          } else if (p.state === 'FLEEING') {
            if (dCam > 18.0) {
              p.state = 'RESTING';
              p.stateTimer = 4.0 + hash01(p.seed) * 3.0;
            } else {
              p.stateTimer = 2.0; // Stay alert if threat is still nearby
            }
          }
        }

        // 4. Locomotion & Steering Forces
        const targetSpeed =
          p.state === 'FLEEING' ? (isGoat ? 4.8 : 5.4) :
          p.state === 'WANDERING' ? (isGoat ? 0.9 : 1.15) : 0.0;

        if (targetSpeed > 0) {
          let steerX = 0;
          let steerZ = 0;

          if (p.state === 'FLEEING') {
            // Run directly away from camera
            const fleeDx = p.position.x - camPos.x;
            const fleeDz = p.position.z - camPos.z;
            const fleeDist = Math.hypot(fleeDx, fleeDz) || 1.0;
            steerX += (fleeDx / fleeDist) * 14.0;
            steerZ += (fleeDz / fleeDist) * 14.0;
          } else if (p.state === 'WANDERING') {
            // A. Wander circle jitter
            const wanderAngle = t * 0.45 + p.seed * 17.0;
            steerX += Math.cos(wanderAngle) * 2.2;
            steerZ += Math.sin(wanderAngle) * 2.2;

            // B. Herd Cohesion (towards herd centroid)
            const toHerdX = herdCenterX - p.position.x;
            const toHerdZ = herdCenterZ - p.position.z;
            steerX += toHerdX * 0.35;
            steerZ += toHerdZ * 0.35;

            // C. Herd Territory Leash (keep herd in meadow/crags habitat)
            const toHomeX = p.herdHome.x - p.position.x;
            const toHomeZ = p.herdHome.y - p.position.z;
            const distHome = Math.hypot(toHomeX, toHomeZ);
            if (distHome > 15.0) {
              const pull = (distHome - 15.0) * 0.75;
              steerX += (toHomeX / distHome) * pull;
              steerZ += (toHomeZ / distHome) * pull;
            }
          }

          // D. Social Neighbor Separation (maintain 2.5m - 3.5m spacing)
          for (let j = 0; j < herd.length; j++) {
            if (i === j) continue;
            const other = herd[j];
            const sepDx = p.position.x - other.position.x;
            const sepDz = p.position.z - other.position.z;
            const sepDistSq = sepDx * sepDx + sepDz * sepDz;
            if (sepDistSq < 3.2 * 3.2 && sepDistSq > 0.001) {
              const sepDist = Math.sqrt(sepDistSq);
              steerX += (sepDx / sepDist) * (3.2 - sepDist) * 3.8;
              steerZ += (sepDz / sepDist) * (3.2 - sepDist) * 3.8;
            }
          }

          // E. Obstacle Avoidance Feelers (query 2.5m forward feeler ray against tree trunks)
          const feelerDist = 2.5;
          const feelerX = p.position.x + p.facing.x * feelerDist;
          const feelerZ = p.position.z + p.facing.y * feelerDist;
          const nearbyFlora = floraIndex.queryNearby(feelerX, feelerZ, 3.5);
          for (let k = 0; k < nearbyFlora.length; k++) {
            const col = nearbyFlora[k];
            const fdx = feelerX - col.x;
            const fdz = feelerZ - col.z;
            const fdist = Math.hypot(fdx, fdz);
            const minSafeDist = col.radius + 0.85;
            if (fdist < minSafeDist) {
              const avoidScale = (minSafeDist - fdist) * 9.0;
              steerX += (fdx / (fdist || 1)) * avoidScale;
              steerZ += (fdz / (fdist || 1)) * avoidScale;
            }
          }

          // F. Terrain Slope & Water Avoidance
          const nextX = p.position.x + (p.velocity.x + steerX * dt) * dt;
          const nextZ = p.position.z + (p.velocity.z + steerZ * dt) * dt;
          const nextTerrain = sampleMap2Terrain(nextX, nextZ);

          if (nextTerrain.isWater || nextTerrain.height <= waterLevel + 0.8) {
            // Strongly repel away from water towards dry land
            steerX -= nextTerrain.normal.x * 25.0;
            steerZ -= nextTerrain.normal.z * 25.0;
          }

          const maxSlope = isGoat ? 0.52 : 0.22;
          if (nextTerrain.steepness > maxSlope) {
            // Deflect downhill away from impassable cliff
            steerX += nextTerrain.normal.x * 20.0;
            steerZ += nextTerrain.normal.z * 20.0;
          }

          // Accelerate velocity
          p.velocity.x += steerX * dt * 4.0;
          p.velocity.z += steerZ * dt * 4.0;

          // Clamp horizontal speed
          const curSpeed = Math.hypot(p.velocity.x, p.velocity.z);
          if (curSpeed > targetSpeed) {
            p.velocity.x = (p.velocity.x / curSpeed) * targetSpeed;
            p.velocity.z = (p.velocity.z / curSpeed) * targetSpeed;
          }
        } else {
          // Natural friction deceleration to 0
          const friction = Math.exp(-8.0 * dt);
          p.velocity.x *= friction;
          p.velocity.z *= friction;
        }

        // Integrate position
        p.position.x += p.velocity.x * dt;
        p.position.z += p.velocity.z * dt;

        // Hard obstacle collision push-out against nearby flora trunks
        const nearbyColliders = floraIndex.queryNearby(p.position.x, p.position.z, 2.5);
        resolveFloraOverlap(p.position, nearbyColliders, 0.45);

        // Update facing vector and leg step phase
        const curHorizSpeed = Math.hypot(p.velocity.x, p.velocity.z);
        if (curHorizSpeed > 0.08) {
          p.facing.x = p.velocity.x / curHorizSpeed;
          p.facing.y = p.velocity.z / curHorizSpeed;
          p.stepPhase += curHorizSpeed * dt * (p.state === 'FLEEING' ? 4.5 : 2.5);
        }

        // Ground Clamping: Strictly clamp to exact terrain height with smooth step bob
        const groundH = getMap2Height(p.position.x, p.position.z);
        const stepBob = curHorizSpeed > 0.08
          ? Math.abs(Math.sin(p.stepPhase * Math.PI)) * (p.state === 'FLEEING' ? 0.07 : 0.04)
          : Math.sin(t * 1.2 + p.seed) * 0.015; // Gentle breathing bob
        p.position.y = groundH + stepBob;

        // Quaternion Slope-Normal Alignment
        // For Deer/Goat geometries: Local +X is Forward (Head), Local +Y is Up, Local +Z is Side
        const { normal: terrainNormal } = getMap2NormalAndSteepness(p.position.x, p.position.z);
        const fwd3D = new THREE.Vector3(p.facing.x, 0, p.facing.y);
        const dotN = fwd3D.dot(terrainNormal);
        const tFwd = fwd3D.clone().sub(terrainNormal.clone().multiplyScalar(dotN)).normalize();
        const tSide = new THREE.Vector3().crossVectors(tFwd, terrainNormal).normalize();

        scratchRotMatrix.makeBasis(tFwd, terrainNormal, tSide);
        scratchTargetQuat.setFromRotationMatrix(scratchRotMatrix);

        // Head pitch down for grazing state
        if (p.state === 'GRAZING') {
          scratchHeadQuat.setFromAxisAngle(tSide, -0.24); // ~14 degrees head pitch down
          scratchTargetQuat.multiply(scratchHeadQuat);
        }

        // Smooth slerp interpolation (10.0 s^-1 damping)
        p.quaternion.slerp(scratchTargetQuat, 1.0 - Math.exp(-10.0 * dt));

        // Compose instance matrix
        dummy.position.copy(p.position);
        dummy.quaternion.copy(p.quaternion);
        dummy.scale.setScalar(p.scale);
        dummy.updateMatrix();

        mesh.setMatrixAt(i, dummy.matrix);
      }

      if (mesh.instanceMatrix) mesh.instanceMatrix.needsUpdate = true;
    };

    updateTerrestrialHerd(deerMeshRef.current, deer, false);
    updateTerrestrialHerd(goatMeshRef.current, goats, true);

    // =========================================================================
    // SECTION D. Water Ducks & Meadow Butterflies
    // =========================================================================
    // 1. Ducks bobbing and paddling on water
    const duckMesh = duckMeshRef.current;
    if (duckMesh && ambientFauna.ducks.length > 0) {
      for (let i = 0; i < ambientFauna.ducks.length; i++) {
        const d = ambientFauna.ducks[i];
        const a = t * (0.15 + hash01(d.seed) * 0.1) + d.yaw;
        const r = 1.5 + hash01(d.seed * 3) * 1.8;
        const dx = d.x + Math.cos(a) * r;
        const dz = d.z + Math.sin(a) * r;
        const dy = d.y + Math.sin(t * 1.8 + d.seed) * 0.06;

        dummy.position.set(dx, dy, dz);
        dummy.rotation.set(0, -a - Math.PI / 2, 0);
        dummy.scale.setScalar(d.scale);
        dummy.updateMatrix();

        duckMesh.setMatrixAt(i, dummy.matrix);
      }
      if (duckMesh.instanceMatrix) duckMesh.instanceMatrix.needsUpdate = true;
    }

    // 2. Butterflies fluttering in harmonic loops
    const flyMesh = butterflyMeshRef.current;
    if (flyMesh && ambientFauna.butterflies.length > 0) {
      for (let i = 0; i < ambientFauna.butterflies.length; i++) {
        const p = ambientFauna.butterflies[i];
        const wx = Math.sin(t * 0.6 + p.seed) * 2.4 + Math.sin(t * 1.4 + p.seed * 2) * 0.8;
        const wz = Math.cos(t * 0.5 + p.seed * 3) * 2.4;
        const wy = 1.2 + Math.sin(t * 1.1 + p.seed) * 0.6;

        dummy.position.set(p.x + wx, p.y + wy, p.z + wz);
        dummy.rotation.set(0, Math.atan2(-Math.cos(t * 0.6 + p.seed), Math.sin(t * 0.5 + p.seed * 3)), 0);
        const flap = 0.2 + Math.abs(Math.sin(t * 12.0 + p.seed)) * 1.1;
        dummy.scale.set(p.scale, p.scale * flap, p.scale);
        dummy.updateMatrix();

        flyMesh.setMatrixAt(i, dummy.matrix);

        if (t < 0.5) {
          const h = hash01(p.seed * 7);
          if (h < 0.25) scratchTint.setRGB(1.2, 0.9, 0.2); // Yellow swallowtail
          else if (h < 0.5) scratchTint.setRGB(0.2, 0.8, 1.3); // Blue morpho
          else if (h < 0.75) scratchTint.setRGB(1.3, 0.4, 0.6); // Monarch orange-red
          else scratchTint.setRGB(1.1, 0.5, 1.2); // Purple emperor
          flyMesh.setColorAt(i, scratchTint);
        }
      }
      if (flyMesh.instanceMatrix) flyMesh.instanceMatrix.needsUpdate = true;
      if (flyMesh.instanceColor && t < 0.5) flyMesh.instanceColor.needsUpdate = true;
    }
  });

  return (
    <group name="map2-wildlife-root">
      {/* Aerial Boids Birds */}
      <instancedMesh
        ref={birdMeshRef}
        args={[birdGeom, null, birdCount]}
        name="map2-wildlife-birds"
        frustumCulled={false}
      >
        <meshBasicMaterial color="#1e293b" side={THREE.DoubleSide} />
      </instancedMesh>

      {/* Aquatic Shoaling Fish */}
      {fishList.length > 0 && (
        <instancedMesh
          ref={fishMeshRef}
          args={[fishGeom, null, fishList.length]}
          name="map2-wildlife-fish"
          frustumCulled={false}
        >
          <meshBasicMaterial side={THREE.DoubleSide} transparent opacity={0.7} />
        </instancedMesh>
      )}

      {/* Terrestrial Deer Herds */}
      <instancedMesh
        ref={deerMeshRef}
        args={[deerGeom, null, deerCount]}
        name="map2-wildlife-deer"
        castShadow
        receiveShadow
      >
        <meshStandardMaterial vertexColors roughness={0.88} metalness={0.05} flatShading />
      </instancedMesh>

      {/* Terrestrial Alpine Goats */}
      <instancedMesh
        ref={goatMeshRef}
        args={[goatGeom, null, goatCount]}
        name="map2-wildlife-goats"
        castShadow
        receiveShadow
      >
        <meshStandardMaterial vertexColors roughness={0.88} metalness={0.05} flatShading />
      </instancedMesh>

      {/* Aquatic Ducks */}
      {ambientFauna.ducks.length > 0 && (
        <instancedMesh
          ref={duckMeshRef}
          args={[duckGeom, null, ambientFauna.ducks.length]}
          name="map2-wildlife-ducks"
          castShadow
        >
          <meshStandardMaterial vertexColors roughness={0.9} metalness={0.05} flatShading />
        </instancedMesh>
      )}

      {/* Butterflies */}
      {ambientFauna.butterflies.length > 0 && (
        <instancedMesh
          ref={butterflyMeshRef}
          args={[butterflyGeom, null, ambientFauna.butterflies.length]}
          name="map2-wildlife-butterflies"
          frustumCulled={false}
        >
          <meshBasicMaterial vertexColors side={THREE.DoubleSide} />
        </instancedMesh>
      )}
    </group>
  );
};

export default Map2Wildlife;

