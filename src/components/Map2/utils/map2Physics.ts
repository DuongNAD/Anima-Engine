import * as THREE from 'three';
import { Map2Biome } from './map2BiomeConfig';
import {
  Map2FloraSpecies,
  FLORA_TRUNK_COLLIDER_RADII,
  isTallFlora,
} from './map2FloraGeometry';

// -------------------------------------------------------------------------------------------------
// Map 2 Character & Camera Physics Engine
// Supports Walk / First-Person Exploration, Gravity, Jump, Slope Sliding, Head-Bob Kinematics,
// Spatial Flora Trunk Collision Push-Out, Terrain Safety Floors, and Multi-Mode Telemetry.
// -------------------------------------------------------------------------------------------------

/** Authoritative Physics Constants */
export const PHYSICS_CONSTANTS = {
  GRAVITY: 34.0, // m/s^2
  JUMP_VELOCITY: 12.5, // m/s -> ~2.3m apex, ~0.74s airtime
  EYE_HEIGHT: 2.1, // m above terrain (y = groundHeight + 2.1)
  HEADBOB_FREQ: 0.9, // Hz (cycles per meter or stride unit)
  HEADBOB_AMP: 0.13, // m (amplitude)
  SLOPE_LIMIT: 0.45, // steepness threshold (~43 degrees)
  PLAYER_RADIUS: 0.45, // m (player capsule horizontal radius)
  COLLIDER_EPSILON: 0.01,
  WORLD_EXTENT: 100.0, // bounds [-100, 100]
  SPECTATOR_EXTENT: 150.0, // bounds [-150, 150]
  ACCEL_LAMBDA: 11.0, // exponential smoothing acceleration rate
  FRICTION_LAMBDA: 14.0, // exponential smoothing friction rate
  AIR_LAMBDA: 2.5, // air control rate while falling
  MIN_SAFETY_FLOOR_ORBIT: 2.0, // m above terrain in orbit mode
  MIN_SAFETY_FLOOR_FLY: 1.2, // m above terrain in fly mode
} as const;

export type CameraMode = 'orbit' | 'fly' | 'walk' | 'cine';

export interface CameraTelemetry {
  position: THREE.Vector3;
  target: THREE.Vector3;
  mode: CameraMode;
  bearing: number; // radians in [0, 2*PI)
  pitch: number; // radians in [-PI/2, PI/2]
  speed: number;
  currentBiome: Map2Biome;
  isGrounded?: boolean;
  altitude?: number;
}

export interface TerrainSampleResult {
  height: number;
  normal: THREE.Vector3;
  steepness: number;
  isWater?: boolean;
}

export interface FloraCollider {
  x: number;
  z: number;
  radius: number;
  species?: Map2FloraSpecies;
}

/**
 * 8 Canonical Landmark Keyframes for Cinematic Catmull-Rom Tour.
 */
export const CANONICAL_CINE_KEYFRAMES: Array<{
  position: [number, number, number];
  target: [number, number, number];
  nameVi: string;
  nameEn: string;
  biome: Map2Biome;
}> = [
  {
    position: [0, 52, -68],
    target: [0, 36, -35],
    nameVi: 'Đỉnh Tuyết Vĩnh Cửu',
    nameEn: 'Glacial Snow Peaks',
    biome: Map2Biome.SNOW_PEAKS,
  },
  {
    position: [-62, 38, -42],
    target: [-42, 22, -22],
    nameVi: 'Vách Đá Dốc Đứng',
    nameEn: 'Jagged Rocky Cliffs',
    biome: Map2Biome.ROCKY_CLIFFS,
  },
  {
    position: [-45, 24, 25],
    target: [-25, 14, 12],
    nameVi: 'Thảo Nguyên Ôn Đới',
    nameEn: 'Temperate Plains',
    biome: Map2Biome.TEMPERATE_PLAINS,
  },
  {
    position: [55, 32, -48],
    target: [35, 18, -28],
    nameVi: 'Rừng Mưa Nhiệt Đới',
    nameEn: 'Tropical Rainforest',
    biome: Map2Biome.TROPICAL_RAINFOREST,
  },
  {
    position: [62, 26, 32],
    target: [42, 16, 16],
    nameVi: 'Thảo Nguyên Savanna',
    nameEn: 'Arid Savanna',
    biome: Map2Biome.ARID_SAVANNA,
  },
  {
    position: [-52, 24, 62],
    target: [-32, 15, 38],
    nameVi: 'Sa Mạc Cát Đỏ',
    nameEn: 'Desert Dunes',
    biome: Map2Biome.DESERT_DUNES,
  },
  {
    position: [12, 22, 52],
    target: [0, 12, 28],
    nameVi: 'Sông Hồ & Thác Nước',
    nameEn: 'Hydrology River & Lake',
    biome: Map2Biome.HYDROLOGY_RIVER_LAKE,
  },
  {
    position: [0, 18, -8],
    target: [0, 12, 12],
    nameVi: 'Hang Động Pha Lê',
    nameEn: 'Bioluminescent Caves',
    biome: Map2Biome.BIOLUMINESCENT_CAVES,
  },
];

/**
 * Computes bearing angle in radians [0, 2*PI) from camera to target or forward look direction.
 * 0 radians = South (+Z look) or North (-Z), clockwise towards East (+X).
 */
export function computeBearing(dir: THREE.Vector3): number {
  const lenSq = dir.x * dir.x + dir.z * dir.z;
  if (lenSq < 1e-9) return 0;
  return (Math.atan2(dir.x, dir.z) + Math.PI * 2) % (Math.PI * 2);
}

/**
 * Computes pitch angle in radians [-PI/2, PI/2].
 */
export function computePitch(dir: THREE.Vector3): number {
  const len = dir.length();
  if (len < 1e-6) return 0;
  return Math.asin(Math.max(-1, Math.min(1, dir.y / len)));
}

/**
 * Calculates head-bob vertical offset and lateral sway from accumulated stride distance.
 */
export function calculateHeadBob(
  strideDistance: number,
  frequency: number = PHYSICS_CONSTANTS.HEADBOB_FREQ,
  amplitude: number = PHYSICS_CONSTANTS.HEADBOB_AMP,
): { offsetY: number; swayX: number } {
  const angle = strideDistance * frequency * Math.PI * 2;
  const offsetY = Math.sin(angle) * amplitude;
  const swayX = Math.cos(angle * 0.5) * (amplitude * 0.4);
  return { offsetY, swayX };
}

/**
 * Clamps camera position to stay above single-sided terrain mesh.
 */
export function clampCameraSafetyFloor(
  cameraPos: THREE.Vector3,
  groundHeight: number,
  safetyMargin: number = 2.0,
): void {
  const minY = groundHeight + safetyMargin;
  if (cameraPos.y < minY) {
    cameraPos.y = minY;
  }
}

/**
 * Clamps position to world extent.
 */
export function clampWorldBounds(
  pos: THREE.Vector3,
  extent: number = PHYSICS_CONSTANTS.WORLD_EXTENT,
): THREE.Vector3 {
  pos.x = Math.max(-extent, Math.min(extent, pos.x));
  pos.z = Math.max(-extent, Math.min(extent, pos.z));
  return pos;
}

/**
 * Spatial Grid bucket index for fast O(1) flora trunk collision push-out.
 */
export class SpatialFloraColliderIndex {
  private cellSize: number;
  private grid = new Map<string, FloraCollider[]>();

  constructor(cellSize = 8.0) {
    this.cellSize = cellSize;
  }

  private key(cellX: number, cellZ: number): string {
    return `${cellX},${cellZ}`;
  }

  public insert(collider: FloraCollider): void {
    const cx = Math.floor(collider.x / this.cellSize);
    const cz = Math.floor(collider.z / this.cellSize);
    const k = this.key(cx, cz);
    let list = this.grid.get(k);
    if (!list) {
      list = [];
      this.grid.set(k, list);
    }
    list.push(collider);
  }

  public queryNearby(x: number, z: number, searchRadius: number = 2.0): FloraCollider[] {
    const minCX = Math.floor((x - searchRadius) / this.cellSize);
    const maxCX = Math.floor((x + searchRadius) / this.cellSize);
    const minCZ = Math.floor((z - searchRadius) / this.cellSize);
    const maxCZ = Math.floor((z + searchRadius) / this.cellSize);

    const result: FloraCollider[] = [];
    for (let cx = minCX; cx <= maxCX; cx++) {
      for (let cz = minCZ; cz <= maxCZ; cz++) {
        const list = this.grid.get(this.key(cx, cz));
        if (list) {
          for (let i = 0; i < list.length; i++) {
            result.push(list[i]);
          }
        }
      }
    }
    return result;
  }

  public clear(): void {
    this.grid.clear();
  }
}

/**
 * Builds a spatial collider index from flora placement records.
 */
export function buildFloraColliderIndex(
  placements: Array<{ species: Map2FloraSpecies; x: number; z: number; scale: number }>,
  cellSize = 8.0,
): SpatialFloraColliderIndex {
  const index = new SpatialFloraColliderIndex(cellSize);
  for (let i = 0; i < placements.length; i++) {
    const p = placements[i];
    const baseRadius = FLORA_TRUNK_COLLIDER_RADII[p.species] ?? (isTallFlora(p.species) ? 0.2 : 0.0);
    if (baseRadius > 0) {
      const radius = baseRadius * p.scale;
      index.insert({
        x: p.x,
        z: p.z,
        radius,
        species: p.species,
      });
    }
  }
  return index;
}

/**
 * Resolves player overlap against a list of solid flora colliders.
 * Handles the zero-distance singular case deterministically by displacing along +X axis.
 */
export function resolveFloraOverlap(
  pos: THREE.Vector3,
  colliders: FloraCollider[],
  playerRadius: number = PHYSICS_CONSTANTS.PLAYER_RADIUS,
): boolean {
  let collided = false;

  for (let i = 0; i < colliders.length; i++) {
    const col = colliders[i];
    const dx = pos.x - col.x;
    const dz = pos.z - col.z;
    const minDist = col.radius + playerRadius;
    const distSq = dx * dx + dz * dz;

    if (distSq < minDist * minDist) {
      collided = true;
      if (distSq < 1e-8) {
        // Deterministic degenerate resolution along +X
        pos.x = col.x + minDist + PHYSICS_CONSTANTS.COLLIDER_EPSILON;
        pos.z = col.z;
      } else {
        const dist = Math.sqrt(distSq);
        const overlap = minDist - dist + PHYSICS_CONSTANTS.COLLIDER_EPSILON;
        pos.x += (dx / dist) * overlap;
        pos.z += (dz / dist) * overlap;
      }
    }
  }

  return collided;
}

/**
 * Resolves collision against a single circular 2D obstacle.
 */
export function resolveObstacleCollision(
  pos: THREE.Vector3,
  obstaclePos: THREE.Vector2,
  obstacleRadius: number,
  playerRadius: number = 0.0,
): void {
  const dx = pos.x - obstaclePos.x;
  const dz = pos.z - obstaclePos.y;
  const targetDist = obstacleRadius + playerRadius;
  const distSq = dx * dx + dz * dz;

  if (distSq < targetDist * targetDist) {
    if (distSq < 1e-8) {
      pos.x = obstaclePos.x + targetDist + 0.01;
      pos.z = obstaclePos.y;
    } else {
      const dist = Math.sqrt(distSq);
      const pushFactor = (targetDist + 0.01) / dist;
      pos.x = obstaclePos.x + dx * pushFactor;
      pos.z = obstaclePos.y + dz * pushFactor;
    }
  }
}

/**
 * Full stateful Physics Controller for Map 2 exploration and camera rigs.
 */
export class Map2CameraPhysicsController {
  public mode: CameraMode = 'orbit';
  public position: THREE.Vector3 = new THREE.Vector3(0, 30, 60);
  public target: THREE.Vector3 = new THREE.Vector3(0, 0, 0);
  public velocity: THREE.Vector3 = new THREE.Vector3(0, 0, 0);
  public isGrounded: boolean = false;
  public strideDistance: number = 0;

  // Configurable physics constants with defaults
  public eyeHeight: number = PHYSICS_CONSTANTS.EYE_HEIGHT;
  public gravity: number = PHYSICS_CONSTANTS.GRAVITY;
  public jumpVelocity: number = PHYSICS_CONSTANTS.JUMP_VELOCITY;
  public slopeLimit: number = PHYSICS_CONSTANTS.SLOPE_LIMIT;
  public worldExtent: number = PHYSICS_CONSTANTS.WORLD_EXTENT;
  public spectatorExtent: number = PHYSICS_CONSTANTS.SPECTATOR_EXTENT;
  public playerRadius: number = PHYSICS_CONSTANTS.PLAYER_RADIUS;

  constructor(initialMode: CameraMode = 'orbit', options?: Partial<{
    eyeHeight: number;
    gravity: number;
    jumpVelocity: number;
    slopeLimit: number;
    worldExtent: number;
    spectatorExtent: number;
  }>) {
    this.mode = initialMode;
    if (options?.eyeHeight !== undefined) this.eyeHeight = options.eyeHeight;
    if (options?.gravity !== undefined) this.gravity = options.gravity;
    if (options?.jumpVelocity !== undefined) this.jumpVelocity = options.jumpVelocity;
    if (options?.slopeLimit !== undefined) this.slopeLimit = options.slopeLimit;
    if (options?.worldExtent !== undefined) this.worldExtent = options.worldExtent;
    if (options?.spectatorExtent !== undefined) this.spectatorExtent = options.spectatorExtent;
  }

  public setMode(newMode: CameraMode): void {
    this.mode = newMode;
    this.velocity.set(0, 0, 0);
  }

  public clampToBounds(pos: THREE.Vector3): THREE.Vector3 {
    const limit = this.mode === 'walk' ? this.worldExtent : this.spectatorExtent;
    pos.x = Math.max(-limit, Math.min(limit, pos.x));
    pos.z = Math.max(-limit, Math.min(limit, pos.z));
    return pos;
  }

  public resolveObstacleCollision(obstaclePos: THREE.Vector2, obstacleRadius: number): void {
    resolveObstacleCollision(this.position, obstaclePos, obstacleRadius, 0);
  }

  public updateWalk(
    inputDirection: THREE.Vector2, // normalized WASD input (x: right, y: forward)
    moveSpeed: number,
    dt: number,
    jumpRequested: boolean,
    sampleTerrain: (x: number, z: number) => TerrainSampleResult,
    colliderIndex?: SpatialFloraColliderIndex,
  ): void {
    if (this.mode !== 'walk') return;

    // Movement direction vectors relative to world or look heading
    const forward = new THREE.Vector3(0, 0, -1);
    const right = new THREE.Vector3(1, 0, 0);
    const move = new THREE.Vector3()
      .addScaledVector(right, inputDirection.x)
      .addScaledVector(forward, inputDirection.y);

    const isMoving = move.lengthSq() > 1e-6;
    if (isMoving) {
      move.normalize().multiplyScalar(moveSpeed);
    }

    // Check prospective terrain
    const nextX = this.position.x + (move.x + this.velocity.x) * dt;
    const nextZ = this.position.z + (move.z + this.velocity.z) * dt;
    const terrain = sampleTerrain(nextX, nextZ);

    // Steep slope sliding and deflection
    if (terrain.steepness > this.slopeLimit) {
      const normalH = new THREE.Vector3(terrain.normal.x, 0, terrain.normal.z);
      if (normalH.lengthSq() > 1e-6) {
        normalH.normalize();
        const dot = move.dot(normalH);
        if (dot < 0) {
          // Deflect movement along tangent
          move.sub(normalH.clone().multiplyScalar(dot));
        }
        // Downhill slide acceleration
        this.velocity.x += normalH.x * this.gravity * 0.5 * dt;
        this.velocity.z += normalH.z * this.gravity * 0.5 * dt;
      }
    } else {
      // Natural friction on flat ground
      const frictionRate = 1 - Math.exp(-PHYSICS_CONSTANTS.FRICTION_LAMBDA * dt);
      this.velocity.x *= (1 - frictionRate);
      this.velocity.z *= (1 - frictionRate);
    }

    // Ground floor check
    const floorY = terrain.height + this.eyeHeight;
    if (this.position.y <= floorY + 0.01) {
      this.isGrounded = true;
    }

    // Jump physics
    if (jumpRequested && this.isGrounded) {
      this.velocity.y = this.jumpVelocity;
      this.isGrounded = false;
    }

    // Gravity integration
    if (!this.isGrounded) {
      this.velocity.y -= this.gravity * dt;
    }

    // Horizontal position integration
    const stepX = (move.x + this.velocity.x) * dt;
    const stepZ = (move.z + this.velocity.z) * dt;
    this.position.x += stepX;
    this.position.z += stepZ;
    this.position.y += this.velocity.y * dt;

    if (this.isGrounded && isMoving) {
      const stepDist = Math.sqrt(stepX * stepX + stepZ * stepZ);
      this.strideDistance += stepDist;
    }

    // Flora trunk collision push-out
    if (colliderIndex) {
      const nearby = colliderIndex.queryNearby(this.position.x, this.position.z, 3.0);
      if (nearby.length > 0) {
        resolveFloraOverlap(this.position, nearby, this.playerRadius);
      }
    }

    // Boundary clamp
    this.clampToBounds(this.position);

    // Ground clamping & floor safety
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

  public getTelemetry(currentBiome: Map2Biome): CameraTelemetry {
    const dir = this.target.clone().sub(this.position);
    const bearing = computeBearing(dir);
    const pitch = computePitch(dir);

    return {
      position: this.position.clone(),
      target: this.target.clone(),
      mode: this.mode,
      bearing,
      pitch,
      speed: this.velocity.length(),
      currentBiome,
      isGrounded: this.isGrounded,
      altitude: this.position.y,
    };
  }
}
