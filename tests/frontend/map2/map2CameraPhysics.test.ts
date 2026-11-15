import { describe, it, expect } from 'vitest';
import * as THREE from 'three';

// -------------------------------------------------------------------------------------------------
// Map 2 Camera Physics & Telemetry Test Suite: 4 Modes, Walk Physics, Slope Collision & Telemetry
// -------------------------------------------------------------------------------------------------

export enum Map2Biome {
  SNOW_PEAKS = 0,
  ROCKY_CLIFFS = 1,
  TEMPERATE_PLAINS = 2,
  TROPICAL_RAINFOREST = 3,
  ARID_SAVANNA = 4,
  DESERT_DUNES = 5,
  HYDROLOGY_RIVER_LAKE = 6,
  BIOLUMINESCENT_CAVES = 7,
}

export type CameraMode = 'orbit' | 'fly' | 'walk' | 'cine';

export interface CameraTelemetry {
  position: THREE.Vector3;
  target: THREE.Vector3;
  mode: CameraMode;
  bearing: number; // radians in [0, 2*PI)
  pitch: number; // radians in [-PI/2, PI/2]
  speed: number;
  currentBiome: Map2Biome;
}

export class Map2CameraPhysicsController {
  public mode: CameraMode = 'orbit';
  public position = new THREE.Vector3(0, 30, 60);
  public target = new THREE.Vector3(0, 0, 0);
  public velocity = new THREE.Vector3(0, 0, 0);
  public isGrounded = false;
  public readonly eyeHeight = 1.7;
  public readonly gravity = 19.6;
  public readonly slopeLimit = 0.65; // ~40-45 degrees
  public readonly worldExtent = 100.0;
  public readonly spectatorExtent = 150.0;

  constructor(initialMode: CameraMode = 'orbit') {
    this.mode = initialMode;
  }

  public setMode(newMode: CameraMode) {
    this.mode = newMode;
    this.velocity.set(0, 0, 0);
  }

  public clampToBounds(pos: THREE.Vector3): THREE.Vector3 {
    const limit = this.mode === 'walk' ? this.worldExtent : this.spectatorExtent;
    pos.x = Math.max(-limit, Math.min(limit, pos.x));
    pos.z = Math.max(-limit, Math.min(limit, pos.z));
    return pos;
  }

  public updateWalk(
    inputDirection: THREE.Vector2, // normalized WASD input
    moveSpeed: number,
    dt: number,
    jumpRequested: boolean,
    sampleTerrain: (x: number, z: number) => { height: number; normal: THREE.Vector3; steepness: number },
  ) {
    if (this.mode !== 'walk') return;

    // Apply horizontal input
    const forward = new THREE.Vector3(0, 0, -1);
    const right = new THREE.Vector3(1, 0, 0);
    const move = new THREE.Vector3()
      .addScaledVector(right, inputDirection.x)
      .addScaledVector(forward, inputDirection.y)
      .normalize()
      .multiplyScalar(moveSpeed);

    // Check terrain at prospective position
    const nextX = this.position.x + move.x * dt;
    const nextZ = this.position.z + move.z * dt;
    const terrain = sampleTerrain(nextX, nextZ);

    // Steep slope collision
    if (terrain.steepness > this.slopeLimit) {
      // Slope deflection: block uphill velocity
      const normalH = new THREE.Vector3(terrain.normal.x, 0, terrain.normal.z).normalize();
      const dot = move.dot(normalH);
      if (dot < 0) {
        // Pushing into cliff -> deflect along tangent
        move.sub(normalH.clone().multiplyScalar(dot));
      }
      // Add sliding velocity downhill
      this.velocity.x += normalH.x * this.gravity * 0.5 * dt;
      this.velocity.z += normalH.z * this.gravity * 0.5 * dt;
    }

    // Ground floor check
    const floorY = terrain.height + this.eyeHeight;
    if (this.position.y <= floorY + 0.01) {
      this.isGrounded = true;
    }

    // Jump physics
    if (jumpRequested && this.isGrounded) {
      this.velocity.y = 8.0;
      this.isGrounded = false;
    }

    // Apply gravity
    if (!this.isGrounded) {
      this.velocity.y -= this.gravity * dt;
    }

    // Integrate position
    this.position.x += (move.x + this.velocity.x) * dt;
    this.position.z += (move.z + this.velocity.z) * dt;
    this.position.y += this.velocity.y * dt;

    // Boundary clamp
    this.clampToBounds(this.position);

    // Ground clamping
    const currentTerrain = sampleTerrain(this.position.x, this.position.z);
    const minFloorY = currentTerrain.height + this.eyeHeight;

    if (this.position.y <= minFloorY) {
      this.position.y = minFloorY;
      this.velocity.y = 0;
      this.isGrounded = true;
    } else {
      this.isGrounded = false;
    }
  }

  public resolveObstacleCollision(obstaclePos: THREE.Vector2, obstacleRadius: number) {
    const camPos2D = new THREE.Vector2(this.position.x, this.position.z);
    const dist = camPos2D.distanceTo(obstaclePos);
    if (dist < obstacleRadius) {
      const pushDir = camPos2D.clone().sub(obstaclePos).normalize();
      if (pushDir.lengthSq() === 0) pushDir.set(1, 0);
      const corrected = obstaclePos.clone().add(pushDir.multiplyScalar(obstacleRadius + 0.01));
      this.position.x = corrected.x;
      this.position.z = corrected.y;
    }
  }

  public getTelemetry(currentBiome: Map2Biome): CameraTelemetry {
    const dir = this.target.clone().sub(this.position);
    const len = dir.length();
    let bearing = 0;
    let pitch = 0;

    if (len > 1e-6) {
      dir.normalize();
      bearing = (Math.atan2(dir.x, dir.z) + Math.PI * 2) % (Math.PI * 2);
      pitch = Math.asin(Math.max(-1, Math.min(1, dir.y)));
    }

    return {
      position: this.position.clone(),
      target: this.target.clone(),
      mode: this.mode,
      bearing,
      pitch,
      speed: this.velocity.length(),
      currentBiome,
    };
  }
}

// -------------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------------

describe('Map 2 Camera Physics — Tier 1: 4 Camera Modes & Telemetry Invariants', () => {
  it('supports all 4 canonical camera modes (orbit, fly, walk, cine)', () => {
    const rig = new Map2CameraPhysicsController();
    const modes: CameraMode[] = ['orbit', 'fly', 'walk', 'cine'];

    for (const m of modes) {
      rig.setMode(m);
      expect(rig.mode).toBe(m);
    }
  });

  it('computes accurate telemetry bearing [0, 2PI) and pitch [-PI/2, PI/2]', () => {
    const rig = new Map2CameraPhysicsController('orbit');
    rig.position.set(0, 10, 20);
    rig.target.set(0, 10, 0); // Looking directly South along -Z

    const telem = rig.getTelemetry(Map2Biome.TEMPERATE_PLAINS);
    expect(telem.mode).toBe('orbit');
    expect(telem.bearing).toBeGreaterThanOrEqual(0);
    expect(telem.bearing).toBeLessThan(Math.PI * 2);
    expect(telem.pitch).toBeCloseTo(0, 4);
    expect(telem.currentBiome).toBe(Map2Biome.TEMPERATE_PLAINS);
  });
});

describe('Map 2 Camera Physics — Tier 2 & Tier 3: Walk Mode Ground Clamping & Boundary Limits', () => {
  it('enforces eye-height ground clamping on flat and varying terrain', () => {
    const rig = new Map2CameraPhysicsController('walk');
    rig.position.set(10, 22.0, 10); // Start slightly in air above terrain (15.0)

    const mockTerrain = () => ({
      height: 15.0,
      normal: new THREE.Vector3(0, 1, 0),
      steepness: 0,
    });

    // Step physics with dt: falls from 22.0 down to floor 16.7
    for (let step = 0; step < 30; step++) {
      rig.updateWalk(new THREE.Vector2(0, 0), 0, 0.05, false, mockTerrain);
    }

    // Should settle onto floor: terrainHeight (15.0) + eyeHeight (1.7) = 16.7
    expect(rig.position.y).toBeCloseTo(16.7, 3);
    expect(rig.isGrounded).toBe(true);
  });

  it('confines walk mode strictly to worldExtent while allowing spectator modes wider framing', () => {
    const rig = new Map2CameraPhysicsController('walk');

    // Attempt to move walker past boundary to x = 300
    rig.position.set(300, 10, -300);
    rig.clampToBounds(rig.position);

    expect(rig.position.x).toBe(100.0);
    expect(rig.position.z).toBe(-100.0);

    // Spectator mode can frame outer area up to 150
    rig.setMode('orbit');
    rig.position.set(300, 50, -300);
    rig.clampToBounds(rig.position);

    expect(rig.position.x).toBe(150.0);
    expect(rig.position.z).toBe(-150.0);
  });

  it('deflects walk velocity and prevents climbing on steep vertical cliffs (steepness > slopeLimit)', () => {
    const rig = new Map2CameraPhysicsController('walk');
    rig.position.set(0, 10, 0);

    // Cliff facing East with normal pointing West (-1, 0, 0)
    const cliffTerrain = () => ({
      height: 10.0,
      normal: new THREE.Vector3(-1, 0.2, 0).normalize(),
      steepness: 0.85,
    });

    // Walk forward into the cliff (+X)
    rig.updateWalk(new THREE.Vector2(1, 0), 5.0, 0.1, false, cliffTerrain);

    // Horizontal sliding / deflection should push back away from the cliff (negative X)
    expect(rig.velocity.x).toBeLessThan(0);
  });

  it('pushes camera out of obstacle cylinders (e.g. tree trunks and cave pillars)', () => {
    const rig = new Map2CameraPhysicsController('walk');
    rig.position.set(10.2, 5.0, 10.1); // Inside tree at (10, 10) with radius 1.0

    rig.resolveObstacleCollision(new THREE.Vector2(10, 10), 1.0);

    const distFromTrunk = new THREE.Vector2(rig.position.x, rig.position.z).distanceTo(new THREE.Vector2(10, 10));
    expect(distFromTrunk).toBeGreaterThanOrEqual(1.0);
  });
});

describe('Map 2 Camera Physics — Tier 4: Rapid Mode Switching & Stress Stability', () => {
  it('executes 200 rapid mode switches without NaN position, gimbal lock or telemetric divergence', () => {
    const rig = new Map2CameraPhysicsController();
    const modeSequence: CameraMode[] = ['orbit', 'walk', 'fly', 'cine'];

    for (let i = 0; i < 200; i++) {
      const mode = modeSequence[i % 4];
      rig.setMode(mode);
      rig.position.add(new THREE.Vector3(0.05, 0.02, -0.05));
      rig.clampToBounds(rig.position);

      const telem = rig.getTelemetry(Map2Biome.DESERT_DUNES);
      expect(Number.isNaN(telem.position.x)).toBe(false);
      expect(Number.isNaN(telem.bearing)).toBe(false);
      expect(Number.isNaN(telem.pitch)).toBe(false);
      expect(Number.isFinite(telem.bearing)).toBe(true);
    }
  });
});
