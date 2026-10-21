import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import {
  PHYSICS_CONSTANTS,
  calculateHeadBob,
  clampCameraSafetyFloor,
  clampWorldBounds,
  SpatialFloraColliderIndex,
  resolveFloraOverlap,
  computeBearing,
  computePitch,
  CANONICAL_CINE_KEYFRAMES,
  Map2CameraPhysicsController,
} from '../../../src/components/Map2/utils/map2Physics';
import { Map2Biome } from '../../../src/components/Map2/utils/map2BiomeConfig';
import { Map2FloraSpecies } from '../../../src/components/Map2/utils/map2FloraGeometry';

// -------------------------------------------------------------------------------------------------
// Map 2 Milestone 5 Test Suite: Physics Engine, Kinematics, Collision Index & HUD Telemetry
// -------------------------------------------------------------------------------------------------

describe('Map 2 Physics Module — Constants & Kinematic Formulas', () => {
  it('exports authoritative physical constants matching Milestone 5 specifications', () => {
    expect(PHYSICS_CONSTANTS.GRAVITY).toBe(34.0);
    expect(PHYSICS_CONSTANTS.JUMP_VELOCITY).toBe(12.5);
    expect(PHYSICS_CONSTANTS.EYE_HEIGHT).toBe(2.1);
    expect(PHYSICS_CONSTANTS.HEADBOB_FREQ).toBe(0.9);
    expect(PHYSICS_CONSTANTS.HEADBOB_AMP).toBe(0.13);
    expect(PHYSICS_CONSTANTS.SLOPE_LIMIT).toBe(0.45);
    expect(PHYSICS_CONSTANTS.PLAYER_RADIUS).toBe(0.45);
    expect(PHYSICS_CONSTANTS.WORLD_EXTENT).toBe(100.0);
    expect(PHYSICS_CONSTANTS.SPECTATOR_EXTENT).toBe(150.0);
  });

  it('calculates head-bob vertical offset and lateral sway predictably', () => {
    // Zero stride distance -> zero vertical offset
    const zeroBob = calculateHeadBob(0);
    expect(zeroBob.offsetY).toBeCloseTo(0, 5);

    // Stride at quarter period
    const quarterBob = calculateHeadBob(1 / (0.9 * 4));
    expect(quarterBob.offsetY).toBeCloseTo(0.13, 3);

    // Stride at three-quarters period
    const threeQuarterBob = calculateHeadBob(3 / (0.9 * 4));
    expect(threeQuarterBob.offsetY).toBeCloseTo(-0.13, 3);
  });

  it('computes accurate bearing [0, 2PI) and pitch [-PI/2, PI/2] from direction vectors', () => {
    // Direction along +Z
    const dirZ = new THREE.Vector3(0, 0, 1);
    const bearingZ = computeBearing(dirZ);
    expect(bearingZ).toBeCloseTo(0, 4);

    // Direction along +X (East)
    const dirX = new THREE.Vector3(1, 0, 0);
    const bearingX = computeBearing(dirX);
    expect(bearingX).toBeCloseTo(Math.PI / 2, 4);

    // Direction pitched up 45 degrees
    const dirUp = new THREE.Vector3(0, 1, 1).normalize();
    const pitchUp = computePitch(dirUp);
    expect(pitchUp).toBeCloseTo(Math.PI / 4, 3);
  });
});

describe('Map 2 Spatial Flora Collider Index & Overlap Resolution', () => {
  it('indexes solid tree trunks into spatial hash grid and queries local neighborhood in O(1)', () => {
    const index = new SpatialFloraColliderIndex(8.0);
    index.insert({ x: 10, z: 10, radius: 0.5, species: Map2FloraSpecies.ConiferPine });
    index.insert({ x: 50, z: 50, radius: 0.5, species: Map2FloraSpecies.LushJungleTree });

    // Query near (10, 10)
    const nearbyClose = index.queryNearby(10.5, 10.2, 2.0);
    expect(nearbyClose.length).toBe(1);
    expect(nearbyClose[0].species).toBe(Map2FloraSpecies.ConiferPine);

    // Query far at (0, 0)
    const nearbyFar = index.queryNearby(0, 0, 2.0);
    expect(nearbyFar.length).toBe(0);
  });

  it('resolves flora trunk collision overlap with player capsule', () => {
    const playerPos = new THREE.Vector3(10.2, 5.0, 10.0);
    const colliders = [{ x: 10.0, z: 10.0, radius: 0.5, species: Map2FloraSpecies.ConiferPine }];

    const collided = resolveFloraOverlap(playerPos, colliders, 0.45);
    expect(collided).toBe(true);

    const dist = new THREE.Vector2(playerPos.x, playerPos.z).distanceTo(new THREE.Vector2(10.0, 10.0));
    expect(dist).toBeGreaterThanOrEqual(0.5 + 0.45);
  });

  it('handles degenerate zero-distance player-trunk overlap by displacing along +X axis', () => {
    const playerPos = new THREE.Vector3(10.0, 5.0, 10.0); // Exact center of trunk
    const colliders = [{ x: 10.0, z: 10.0, radius: 0.5, species: Map2FloraSpecies.BroadleafDeciduous }];

    const collided = resolveFloraOverlap(playerPos, colliders, 0.45);
    expect(collided).toBe(true);
    expect(playerPos.x).toBeGreaterThan(10.0);
    expect(playerPos.z).toBeCloseTo(10.0, 4);
    expect(playerPos.x).toBeCloseTo(10.0 + 0.5 + 0.45 + 0.01, 3);
  });
});

describe('Map 2 Terrain Safety Floor & Boundary Clamping', () => {
  it('enforces terrain safety floor across camera positions to prevent sinking below mesh', () => {
    const camPos = new THREE.Vector3(20, 10, 20);
    const groundHeight = 15.0;

    clampCameraSafetyFloor(camPos, groundHeight, 2.0);
    expect(camPos.y).toBe(17.0); // 15.0 + 2.0
  });

  it('clamps coordinates strictly within specified world extent', () => {
    const pos = new THREE.Vector3(250, 10, -320);
    clampWorldBounds(pos, 100);

    expect(pos.x).toBe(100);
    expect(pos.z).toBe(-100);
  });
});

describe('Map 2 Cinematic Spline Tour Keyframes', () => {
  it('defines 8 canonical keyframes covering all 8 Map 2 biomes', () => {
    expect(CANONICAL_CINE_KEYFRAMES.length).toBe(8);

    const biomeSet = new Set(CANONICAL_CINE_KEYFRAMES.map((kf) => kf.biome));
    expect(biomeSet.size).toBe(8);

    expect(biomeSet.has(Map2Biome.SNOW_PEAKS)).toBe(true);
    expect(biomeSet.has(Map2Biome.ROCKY_CLIFFS)).toBe(true);
    expect(biomeSet.has(Map2Biome.TEMPERATE_PLAINS)).toBe(true);
    expect(biomeSet.has(Map2Biome.TROPICAL_RAINFOREST)).toBe(true);
    expect(biomeSet.has(Map2Biome.ARID_SAVANNA)).toBe(true);
    expect(biomeSet.has(Map2Biome.DESERT_DUNES)).toBe(true);
    expect(biomeSet.has(Map2Biome.HYDROLOGY_RIVER_LAKE)).toBe(true);
    expect(biomeSet.has(Map2Biome.BIOLUMINESCENT_CAVES)).toBe(true);

    CANONICAL_CINE_KEYFRAMES.forEach((kf) => {
      expect(kf.position).toHaveLength(3);
      expect(kf.target).toHaveLength(3);
      expect(kf.nameVi.length).toBeGreaterThan(0);
      expect(kf.nameEn.length).toBeGreaterThan(0);
    });
  });
});

describe('Map 2 Controller Walk Mode Kinematics (g=34, jump=12.5, eye=2.1)', () => {
  it('settles walking player onto terrain floor with eye height 2.1m', () => {
    const controller = new Map2CameraPhysicsController('walk');
    controller.position.set(0, 30.0, 0);

    const sampleTerrain = () => ({
      height: 10.0,
      normal: new THREE.Vector3(0, 1, 0),
      steepness: 0,
    });

    for (let i = 0; i < 40; i++) {
      controller.updateWalk(new THREE.Vector2(0, 0), 0, 0.03, false, sampleTerrain);
    }

    expect(controller.position.y).toBeCloseTo(12.1, 3); // 10.0 + 2.1
    expect(controller.isGrounded).toBe(true);
  });

  it('initiates jump with vertical velocity 12.5m/s', () => {
    const controller = new Map2CameraPhysicsController('walk');
    controller.position.set(0, 12.1, 0);
    controller.isGrounded = true;

    const sampleTerrain = () => ({
      height: 10.0,
      normal: new THREE.Vector3(0, 1, 0),
      steepness: 0,
    });

    controller.updateWalk(new THREE.Vector2(0, 0), 0, 0.016, true, sampleTerrain);

    expect(controller.velocity.y).toBeCloseTo(12.5 - 34.0 * 0.016, 2);
    expect(controller.isGrounded).toBe(false);
  });
});
