import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import {
  Map2TerrainMathEngine,
  sampleMap2Terrain,
  getMap2Height,
  getMap2NormalAndSteepness,
} from '../../../src/components/Map2/utils/map2TerrainGen';
import {
  Map2Biome,
  Map2BiomeEcologySystem,
} from '../../../src/components/Map2/utils/map2BiomeConfig';
import {
  Map2CameraPhysicsController,
  PHYSICS_CONSTANTS,
  SpatialFloraColliderIndex,
  resolveFloraOverlap,
} from '../../../src/components/Map2/utils/map2Physics';
import {
  computeCelestialState,
} from '../../../src/components/Map2/utils/map2AtmosphereMath';

describe('Map 2 Adversarial Stress Testing & Empirical Verification Suite', () => {
  // -----------------------------------------------------------------------------------------------
  // 1. COORDINATE EXTREME BOUNDS (-1,000,000 to +1,000,000)
  // -----------------------------------------------------------------------------------------------
  describe('Adversarial Test 1: Coordinate Extreme Bounds', () => {
    it('evaluates 100,000 random coordinates in [-1,000,000, 1,000,000] without NaN, Inf, or negative heights', () => {
      const engine = new Map2TerrainMathEngine('adversarial_test_seed');
      const iterations = 100_000;
      let minHeight = Infinity;
      let maxHeight = -Infinity;
      let nanCount = 0;
      let infCount = 0;
      let negativeCount = 0;

      // Seeded LCG for deterministic test reproduction
      let lcgState = 0xdeadbeef;
      const lcg = () => {
        lcgState = (Math.imul(1664525, lcgState) + 1013904223) >>> 0;
        return lcgState / 4294967296;
      };

      for (let i = 0; i < iterations; i++) {
        const x = (lcg() * 2 - 1) * 1_000_000;
        const z = (lcg() * 2 - 1) * 1_000_000;

        const h = engine.getHeight(x, z);

        if (Number.isNaN(h)) nanCount++;
        if (!Number.isFinite(h)) infCount++;
        if (h < 0) negativeCount++;

        if (h < minHeight) minHeight = h;
        if (h > maxHeight) maxHeight = h;
      }

      expect(nanCount).toBe(0);
      expect(infCount).toBe(0);
      expect(negativeCount).toBe(0);
      expect(minHeight).toBeGreaterThanOrEqual(0);
      expect(maxHeight).toBeLessThanOrEqual(engine.maxElevation);
      expect(maxHeight).toBeGreaterThan(0);
    });

    it('evaluates singular and boundary coordinates without breakdown', () => {
      const engine = new Map2TerrainMathEngine('adversarial_test_seed');
      const singularPoints = [
        [0, 0],
        [1_000_000, 1_000_000],
        [-1_000_000, -1_000_000],
        [1_000_000, -1_000_000],
        [-1_000_000, 1_000_000],
        [255, 255],
        [256, 256],
        [-255, -255],
        [-256, -256],
        [511, 511],
        [512, 512],
        [-511, -511],
        [-512, -512],
        [1e-7, -1e-7],
        [Number.MAX_SAFE_INTEGER % 1_000_000, -Number.MAX_SAFE_INTEGER % 1_000_000],
      ];

      for (const [x, z] of singularPoints) {
        const sample = engine.sample(x, z);
        expect(Number.isFinite(sample.height)).toBe(true);
        expect(sample.height).toBeGreaterThanOrEqual(0);
        expect(sample.height).toBeLessThanOrEqual(engine.maxElevation);
        expect(sample.temperature).toBeGreaterThanOrEqual(0);
        expect(sample.temperature).toBeLessThanOrEqual(1);
        expect(sample.moisture).toBeGreaterThanOrEqual(0);
        expect(sample.moisture).toBeLessThanOrEqual(1);
        expect(sample.steepness).toBeGreaterThanOrEqual(0);
        expect(sample.steepness).toBeLessThanOrEqual(1);
      }
    });
  });

  // -----------------------------------------------------------------------------------------------
  // 2. PARTITION OF UNITY ACROSS 50,000 RANDOM COORDINATES
  // -----------------------------------------------------------------------------------------------
  describe('Adversarial Test 2: Partition of Unity for Biome Weights', () => {
    it('verifies sum of biome weights == 1.0 +- 1e-5 across 50,000 random coordinates', () => {
      const ecology = new Map2BiomeEcologySystem();
      const iterations = 50_000;
      let maxAbsDeviation = 0;
      let invalidSumCount = 0;
      let negativeWeightCount = 0;

      let lcgState = 0x12345678;
      const lcg = () => {
        lcgState = (Math.imul(1664525, lcgState) + 1013904223) >>> 0;
        return lcgState / 4294967296;
      };

      for (let i = 0; i < iterations; i++) {
        // Sample across full range of climates, slopes, heights, and caves
        const height = lcg() * 95.0 - 5.0; // [-5, 90]
        const steepness = lcg(); // [0, 1]
        const temperature = lcg(); // [0, 1]
        const moisture = lcg(); // [0, 1]
        const caveIntensity = lcg(); // [0, 1]

        const weights = ecology.computeBiomeWeights({
          height,
          steepness,
          temperature,
          moisture,
          caveIntensity,
        });

        expect(weights.length).toBe(8);

        let sum = 0;
        for (let b = 0; b < 8; b++) {
          const w = weights[b];
          if (w < 0) negativeWeightCount++;
          if (w > 1.00001) invalidSumCount++;
          sum += w;
        }

        const deviation = Math.abs(sum - 1.0);
        if (deviation > maxAbsDeviation) maxAbsDeviation = deviation;
        if (deviation > 1e-5) invalidSumCount++;
      }

      expect(negativeWeightCount).toBe(0);
      expect(invalidSumCount).toBe(0);
      expect(maxAbsDeviation).toBeLessThanOrEqual(1e-5);
    });

    it('verifies partition of unity when querying sampleMap2Terrain across 50,000 world coordinates', () => {
      const iterations = 50_000;
      let maxAbsDeviation = 0;
      let invalidCount = 0;

      let lcgState = 0x87654321;
      const lcg = () => {
        lcgState = (Math.imul(1664525, lcgState) + 1013904223) >>> 0;
        return lcgState / 4294967296;
      };

      for (let i = 0; i < iterations; i++) {
        const x = (lcg() * 2 - 1) * 150.0;
        const z = (lcg() * 2 - 1) * 150.0;

        const sample = sampleMap2Terrain(x, z);
        let sum = 0;
        for (let b = 0; b < 8; b++) {
          sum += sample.biomeWeights[b];
        }

        const dev = Math.abs(sum - 1.0);
        if (dev > maxAbsDeviation) maxAbsDeviation = dev;
        if (dev > 1e-5) invalidCount++;
      }

      expect(invalidCount).toBe(0);
      expect(maxAbsDeviation).toBeLessThanOrEqual(1e-5);
    });
  });

  // -----------------------------------------------------------------------------------------------
  // 3. ANALYTICAL NORMAL UNIT LENGTH (||N|| == 1.0 +- 1e-4)
  // -----------------------------------------------------------------------------------------------
  describe('Adversarial Test 3: Analytical Normal Unit Length', () => {
    it('verifies ||N|| == 1.0 +- 1e-4 across 50,000 random points', () => {
      const iterations = 50_000;
      let minLen = Infinity;
      let maxLen = -Infinity;
      let invalidCount = 0;
      let downwardCount = 0;

      let lcgState = 0xfeedbeef;
      const lcg = () => {
        lcgState = (Math.imul(1664525, lcgState) + 1013904223) >>> 0;
        return lcgState / 4294967296;
      };

      for (let i = 0; i < iterations; i++) {
        const x = (lcg() * 2 - 1) * 500.0;
        const z = (lcg() * 2 - 1) * 500.0;

        const { normal, steepness } = getMap2NormalAndSteepness(x, z);
        const len = normal.length();

        if (len < minLen) minLen = len;
        if (len > maxLen) maxLen = len;

        const dev = Math.abs(len - 1.0);
        if (dev > 1e-4) invalidCount++;
        if (normal.y <= 0) downwardCount++;
        if (steepness < 0 || steepness > 1) invalidCount++;
      }

      expect(invalidCount).toBe(0);
      expect(downwardCount).toBe(0);
      expect(minLen).toBeGreaterThanOrEqual(1.0 - 1e-4);
      expect(maxLen).toBeLessThanOrEqual(1.0 + 1e-4);
    });
  });

  // -----------------------------------------------------------------------------------------------
  // 4. CHARACTER PHYSICS KINEMATICS (10,000 Walk Steps on Steep Cliffs + Trunk Colliders)
  // -----------------------------------------------------------------------------------------------
  describe('Adversarial Test 4: Character Physics Kinematics on Steep Cliffs & Obstacle Collisions', () => {
    it('simulates 10,000 walk steps on steep cliffs (>45 deg) with tree trunk colliders without penetration or clipping', () => {
      const controller = new Map2CameraPhysicsController('walk');
      const colliderIndex = new SpatialFloraColliderIndex(8.0);

      // Create 300 tree colliders distributed around the map
      let lcgState = 0x99887766;
      const lcg = () => {
        lcgState = (Math.imul(1664525, lcgState) + 1013904223) >>> 0;
        return lcgState / 4294967296;
      };

      const collidersList: Array<{ x: number; z: number; radius: number }> = [];
      for (let i = 0; i < 300; i++) {
        const cx = (lcg() * 2 - 1) * 80.0;
        const cz = (lcg() * 2 - 1) * 80.0;
        const radius = 0.2 + lcg() * 0.8; // 0.2m to 1.0m radius
        const col = { x: cx, z: cz, radius };
        colliderIndex.insert(col);
        collidersList.push(col);
      }

      // Find steep cliff locations (>45 deg -> steepness > 0.45)
      const steepSpots: Array<[number, number]> = [];
      for (let x = -80; x <= 80; x += 5) {
        for (let z = -80; z <= 80; z += 5) {
          const sample = sampleMap2Terrain(x, z);
          if (sample.steepness > 0.45) {
            steepSpots.push([x, z]);
          }
        }
      }

      expect(steepSpots.length).toBeGreaterThan(10);

      // Spawn player on a steep cliff
      const spawnCliff = steepSpots[0];
      controller.position.set(
        spawnCliff[0],
        getMap2Height(spawnCliff[0], spawnCliff[1]) + PHYSICS_CONSTANTS.EYE_HEIGHT,
        spawnCliff[1],
      );

      let penetrationCount = 0;
      let colliderClipCount = 0;
      let nanCount = 0;
      const dt = 1 / 60; // 60 FPS timestep

      const totalSteps = 10_000;
      for (let step = 0; step < totalSteps; step++) {
        // Every 500 steps, teleport to another steep cliff or collider center
        if (step % 500 === 0) {
          const cliff = steepSpots[(step / 500) % steepSpots.length];
          controller.position.set(
            cliff[0],
            getMap2Height(cliff[0], cliff[1]) + PHYSICS_CONSTANTS.EYE_HEIGHT,
            cliff[1],
          );
          controller.velocity.set(0, 0, 0);
        }

        // Random adversarial input: sprint uphill, jump, erratic WASD
        const inputDir = new THREE.Vector2(lcg() * 2 - 1, lcg() * 2 - 1);
        if (inputDir.lengthSq() > 0) inputDir.normalize();
        const moveSpeed = lcg() > 0.5 ? 12.0 : 6.0;
        const jumpRequested = lcg() < 0.1; // 10% chance of jump request

        controller.updateWalk(
          inputDir,
          moveSpeed,
          dt,
          jumpRequested,
          (x, z) => sampleMap2Terrain(x, z),
          colliderIndex,
        );

        // Check for NaN / Inf
        if (
          !Number.isFinite(controller.position.x) ||
          !Number.isFinite(controller.position.y) ||
          !Number.isFinite(controller.position.z) ||
          !Number.isFinite(controller.velocity.x) ||
          !Number.isFinite(controller.velocity.y) ||
          !Number.isFinite(controller.velocity.z)
        ) {
          nanCount++;
        }

        // Verify no terrain penetration: player eye position must be >= terrain height + eyeHeight - 1e-4
        const groundH = getMap2Height(controller.position.x, controller.position.z);
        const minAllowedEyeY = groundH + PHYSICS_CONSTANTS.EYE_HEIGHT - 1e-4;
        if (controller.position.y < minAllowedEyeY) {
          penetrationCount++;
        }

        // Verify tree trunk collision push-out: player capsule must not be inside any nearby collider
        const nearbyColliders = colliderIndex.queryNearby(
          controller.position.x,
          controller.position.z,
          3.0,
        );
        for (const col of nearbyColliders) {
          const dx = controller.position.x - col.x;
          const dz = controller.position.z - col.z;
          const dist = Math.hypot(dx, dz);
          const minSafeDist = col.radius + PHYSICS_CONSTANTS.PLAYER_RADIUS - 1e-3;
          if (dist < minSafeDist) {
            colliderClipCount++;
          }
        }
      }

      expect(nanCount).toBe(0);
      expect(penetrationCount).toBe(0);
      expect(colliderClipCount).toBe(0);
    });

    it('handles degenerate zero-distance collision against tree colliders deterministically', () => {
      const pos = new THREE.Vector3(10.0, 15.0, 10.0);
      const colliders = [{ x: 10.0, z: 10.0, radius: 0.5 }];
      const resolved = resolveFloraOverlap(pos, colliders, 0.45);

      expect(resolved).toBe(true);
      expect(pos.x).toBeGreaterThan(10.0 + 0.5 + 0.45);
      expect(pos.z).toBe(10.0);
      expect(Number.isFinite(pos.x)).toBe(true);
    });
  });

  // -----------------------------------------------------------------------------------------------
  // 5. RAPID DAY-NIGHT TIME SCRUBBING & CAMERA MODE SWITCHING
  // -----------------------------------------------------------------------------------------------
  describe('Adversarial Test 5: Rapid Day-Night Time Scrubbing & Camera Mode Switching', () => {
    it('performs 100,000 rapid time-of-day queries across [-1,000,000, 1,000,000] without numerical instability', () => {
      const iterations = 100_000;
      let nanCount = 0;
      let unitVectorInvalidCount = 0;
      let invalidColorCount = 0;

      let lcgState = 0xabcdef12;
      const lcg = () => {
        lcgState = (Math.imul(1664525, lcgState) + 1013904223) >>> 0;
        return lcgState / 4294967296;
      };

      for (let i = 0; i < iterations; i++) {
        // Test wild range of times: negative, extreme, sub-millisecond deltas
        const timeOfDay = (lcg() * 2 - 1) * 1_000_000;
        const celestial = computeCelestialState(timeOfDay);

        // Check sun/moon direction unit vector lengths
        const sunDirLen = celestial.sunDirection.length();
        const moonDirLen = celestial.moonDirection.length();

        if (Math.abs(sunDirLen - 1.0) > 1e-4) unitVectorInvalidCount++;
        if (Math.abs(moonDirLen - 1.0) > 1e-4) unitVectorInvalidCount++;

        // Check sun and moon positions are at radius 400
        const sunPosLen = celestial.sunPosition.length();
        const moonPosLen = celestial.moonPosition.length();
        if (Math.abs(sunPosLen - 400.0) > 1e-2) unitVectorInvalidCount++;
        if (Math.abs(moonPosLen - 400.0) > 1e-2) unitVectorInvalidCount++;

        // Check Colors
        const colors = [
          celestial.sunColor,
          celestial.moonColor,
          celestial.skyZenithColor,
          celestial.skyHorizonColor,
          celestial.skyGroundColor,
          celestial.fogColor,
          celestial.ambientColor,
          celestial.hemiSkyColor,
          celestial.hemiGroundColor,
        ];

        for (const c of colors) {
          if (
            !Number.isFinite(c.r) ||
            !Number.isFinite(c.g) ||
            !Number.isFinite(c.b) ||
            c.r < 0 ||
            c.g < 0 ||
            c.b < 0
          ) {
            invalidColorCount++;
          }
        }

        // Check Intensities
        if (
          !Number.isFinite(celestial.sunIntensity) ||
          !Number.isFinite(celestial.moonIntensity) ||
          !Number.isFinite(celestial.fogDensity) ||
          !Number.isFinite(celestial.ambientIntensity) ||
          !Number.isFinite(celestial.hemiIntensity) ||
          !Number.isFinite(celestial.starOpacity) ||
          celestial.sunIntensity < 0 ||
          celestial.moonIntensity < 0 ||
          celestial.fogDensity < 0 ||
          celestial.starOpacity < 0 ||
          celestial.starOpacity > 1.0001
        ) {
          nanCount++;
        }
      }

      expect(nanCount).toBe(0);
      expect(unitVectorInvalidCount).toBe(0);
      expect(invalidColorCount).toBe(0);
    });

    it('handles 10,000 rapid camera mode switches without numerical explosion or NaN state', () => {
      const controller = new Map2CameraPhysicsController('orbit');
      const modes: Array<'orbit' | 'fly' | 'walk' | 'cine'> = ['orbit', 'fly', 'walk', 'cine'];
      let nanCount = 0;
      let outOfBoundsCount = 0;

      let lcgState = 0x55443322;
      const lcg = () => {
        lcgState = (Math.imul(1664525, lcgState) + 1013904223) >>> 0;
        return lcgState / 4294967296;
      };

      for (let i = 0; i < 10_000; i++) {
        const nextMode = modes[Math.floor(lcg() * modes.length)];
        controller.setMode(nextMode);

        // Teleport or move
        if (lcg() < 0.2) {
          const tx = (lcg() * 2 - 1) * 120.0;
          const tz = (lcg() * 2 - 1) * 120.0;
          controller.position.set(tx, getMap2Height(tx, tz) + 20, tz);
        }

        if (nextMode === 'walk') {
          controller.updateWalk(
            new THREE.Vector2(lcg() * 2 - 1, lcg() * 2 - 1),
            8.0,
            1 / 60,
            lcg() < 0.1,
            (x, z) => sampleMap2Terrain(x, z),
          );
        }

        const telemetry = controller.getTelemetry(Map2Biome.TEMPERATE_PLAINS);

        if (
          !Number.isFinite(telemetry.position.x) ||
          !Number.isFinite(telemetry.position.y) ||
          !Number.isFinite(telemetry.position.z) ||
          !Number.isFinite(telemetry.bearing) ||
          !Number.isFinite(telemetry.pitch) ||
          !Number.isFinite(telemetry.speed)
        ) {
          nanCount++;
        }

        if (
          telemetry.bearing < 0 ||
          telemetry.bearing >= Math.PI * 2 + 1e-4 ||
          telemetry.pitch < -Math.PI / 2 - 1e-4 ||
          telemetry.pitch > Math.PI / 2 + 1e-4
        ) {
          outOfBoundsCount++;
        }
      }

      expect(nanCount).toBe(0);
      expect(outOfBoundsCount).toBe(0);
    });
  });
});
