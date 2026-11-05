import React, { useCallback, useEffect, useRef } from 'react';
import { extend, useFrame, useThree } from '@react-three/fiber';
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { sampleMap2Terrain, getMap2Height } from './utils/map2TerrainGen';
import { Map2Biome } from './utils/map2BiomeConfig';
import {
  CameraMode,
  PHYSICS_CONSTANTS,
  calculateHeadBob,
  clampCameraSafetyFloor,
  clampWorldBounds,
  SpatialFloraColliderIndex,
  resolveFloraOverlap,
  computeBearing,
  computePitch,
  CANONICAL_CINE_KEYFRAMES,
} from './utils/map2Physics';

extend({ OrbitControls });

// -------------------------------------------------------------------------------------------------
// Map 2 Multi-Mode Camera Rig
// 4 Distinct Modes: Orbit, Fly, Walk/FPS with collision/gravity/head-bob, and Cine Catmull-Rom tour.
// -------------------------------------------------------------------------------------------------

export interface Map2CameraView {
  targetX: number;
  targetZ: number;
  camX: number;
  camY: number;
  camZ: number;
  bearing: number; // degrees [0, 360)
  pitch: number; // degrees [-90, 90]
  biome: Map2Biome;
  fps: number;
  locked: boolean;
  mode: CameraMode;
  speed: number;
  isGrounded?: boolean;
}

export interface Map2CameraRigProps {
  mode: CameraMode;
  onModeChange?: (mode: CameraMode) => void;
  viewRef: React.MutableRefObject<Map2CameraView>;
  teleportRef: React.MutableRefObject<{ x: number; z: number } | null>;
  worldSize?: number;
  colliderIndex?: SpatialFloraColliderIndex;
  capturePose?: { position: [number, number, number]; target: [number, number, number] } | null;
}

const scratchEuler = new THREE.Euler(0, 0, 0, 'YXZ');
const scratchForward = new THREE.Vector3();
const scratchRight = new THREE.Vector3();
const scratchWish = new THREE.Vector3();
const UP_VECTOR = new THREE.Vector3(0, 1, 0);

const NAV_KEYS = new Set(['Space', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight']);
const clampPitch = (p: number) => Math.max(-1.42, Math.min(1.42, p));

export const Map2CameraRig: React.FC<Map2CameraRigProps> = ({
  mode,
  onModeChange: _onModeChange,
  viewRef,
  teleportRef,
  worldSize = 200,
  colliderIndex,
  capturePose = null,
}) => {
  const { camera, gl } = useThree();
  const controlsRef = useRef<OrbitControls | null>(null);

  // Input & Kinematics state
  const keysRef = useRef<Set<string>>(new Set());
  const lookRef = useRef({ yaw: 0, pitch: -0.25, dragging: false, locked: false, lastX: 0, lastY: 0 });
  const velRef = useRef({ x: 0, z: 0 });
  const physRef = useRef({
    baseY: 0,
    vy: 0,
    grounded: true,
    strideDist: 0,
  });

  // Cinematic Spline state
  const cineSplineRef = useRef<{
    posCurve: THREE.CatmullRomCurve3;
    targetCurve: THREE.CatmullRomCurve3;
    progress: number;
  } | null>(null);

  // Build Catmull-Rom spline curves for Cinematic Mode
  useEffect(() => {
    const posPoints = CANONICAL_CINE_KEYFRAMES.map((kf) => new THREE.Vector3(...kf.position));
    const targetPoints = CANONICAL_CINE_KEYFRAMES.map((kf) => new THREE.Vector3(...kf.target));

    const posCurve = new THREE.CatmullRomCurve3(posPoints, true, 'catmullrom', 0.5);
    const targetCurve = new THREE.CatmullRomCurve3(targetPoints, true, 'catmullrom', 0.5);

    cineSplineRef.current = {
      posCurve,
      targetCurve,
      progress: 0,
    };
  }, []);

  const queryTerrain = useCallback((x: number, z: number) => {
    return sampleMap2Terrain(x, z);
  }, []);

  // Keyboard Event Listeners for WASD + Q/E + Space + Shift
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      keysRef.current.add(e.code);
      if (NAV_KEYS.has(e.code)) e.preventDefault();
    };
    const onKeyUp = (e: KeyboardEvent) => {
      keysRef.current.delete(e.code);
    };
    const onBlur = () => {
      keysRef.current.clear();
    };

    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('keyup', onKeyUp);
    window.addEventListener('blur', onBlur);

    return () => {
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('keyup', onKeyUp);
      window.removeEventListener('blur', onBlur);
    };
  }, []);

  // Pointer Lock & Mouse Look for Fly and Walk Modes
  useEffect(() => {
    if (mode !== 'fly' && mode !== 'walk') return;
    const el = gl.domElement;
    const look = lookRef.current;
    const view = viewRef.current;
    const SENS_LOCK = 0.0022;
    const SENS_DRAG = 0.0035;

    const onPointerDown = (e: PointerEvent) => {
      if (e.button !== 0) return;
      if (document.pointerLockElement !== el) {
        el.requestPointerLock?.();
      }
      look.dragging = true;
      look.lastX = e.clientX;
      look.lastY = e.clientY;
    };

    const onPointerMove = (e: PointerEvent) => {
      if (look.locked) {
        look.yaw -= (e.movementX || 0) * SENS_LOCK;
        look.pitch = clampPitch(look.pitch - (e.movementY || 0) * SENS_LOCK);
      } else if (look.dragging) {
        look.yaw -= (e.clientX - look.lastX) * SENS_DRAG;
        look.pitch = clampPitch(look.pitch - (e.clientY - look.lastY) * SENS_DRAG);
        look.lastX = e.clientX;
        look.lastY = e.clientY;
      }
    };

    const onPointerUp = () => {
      look.dragging = false;
    };

    const onLockChange = () => {
      look.locked = document.pointerLockElement === el;
      view.locked = look.locked;
    };

    el.addEventListener('pointerdown', onPointerDown);
    window.addEventListener('pointermove', onPointerMove);
    window.addEventListener('pointerup', onPointerUp);
    document.addEventListener('pointerlockchange', onLockChange);

    return () => {
      el.removeEventListener('pointerdown', onPointerDown);
      window.removeEventListener('pointermove', onPointerMove);
      window.removeEventListener('pointerup', onPointerUp);
      document.removeEventListener('pointerlockchange', onLockChange);
      look.locked = false;
      view.locked = false;
      if (document.pointerLockElement === el) {
        document.exitPointerLock?.();
      }
    };
  }, [mode, gl, viewRef]);

  // Mode Transition & Seeding
  const lastModeRef = useRef<CameraMode | null>(null);
  useEffect(() => {
    if (lastModeRef.current === mode) return;
    lastModeRef.current = mode;

    const look = lookRef.current;
    velRef.current.x = 0;
    velRef.current.z = 0;

    if (mode === 'orbit') {
      const controls = controlsRef.current;
      if (controls) {
        controls.enabled = true;
      }
    } else {
      const controls = controlsRef.current;
      if (controls) {
        controls.enabled = false;
      }
    }

    if (mode === 'fly' || mode === 'walk') {
      const dir = new THREE.Vector3();
      camera.getWorldDirection(dir);
      look.yaw = Math.atan2(-dir.x, -dir.z);
      look.pitch = Math.asin(Math.max(-1, Math.min(1, dir.y)));

      if (mode === 'walk') {
        const v = viewRef.current;
        const groundH = getMap2Height(v.targetX, v.targetZ);
        const gy = groundH + PHYSICS_CONSTANTS.EYE_HEIGHT;
        camera.position.set(v.targetX, gy, v.targetZ);
        physRef.current.baseY = gy;
        physRef.current.vy = 0;
        physRef.current.grounded = true;
        physRef.current.strideDist = 0;
        look.pitch = Math.max(-0.35, look.pitch);
      }
      (document.activeElement as HTMLElement | null)?.blur?.();
    }
  }, [mode, camera, viewRef]);

  // Per-Frame Physics & Camera Controller Loop
  useFrame((state, rawDelta) => {
    const { camera } = state;
    const dt = Math.min(rawDelta, 0.1);
    const look = lookRef.current;
    const keys = keysRef.current;
    const v = viewRef.current;
    const tp = teleportRef.current;

    // Smoothed FPS
    const currentFps = dt > 0 ? 1 / dt : 60;
    v.fps = v.fps ? v.fps * 0.9 + currentFps * 0.1 : currentFps;
    v.mode = mode;

    // Handle Canonical Capture Pose Override
    if (capturePose) {
      camera.position.set(capturePose.position[0], capturePose.position[1], capturePose.position[2]);
      camera.lookAt(capturePose.target[0], capturePose.target[1], capturePose.target[2]);
      v.targetX = capturePose.target[0];
      v.targetZ = capturePose.target[2];
      v.camX = camera.position.x;
      v.camY = camera.position.y;
      v.camZ = camera.position.z;
      const sample = queryTerrain(v.targetX, v.targetZ);
      v.biome = sample.biomeId;
      v.bearing = 0;
      v.pitch = 0;
      v.speed = 0;
      return;
    }

    // ---------------------------------------------------------------------------------------------
    // MODE 1: ORBIT MODE
    // ---------------------------------------------------------------------------------------------
    if (mode === 'orbit') {
      const controls = controlsRef.current;
      if (!controls) return;

      if (tp) {
        const groundH = getMap2Height(tp.x, tp.z);
        const dx = tp.x - controls.target.x;
        const dz = tp.z - controls.target.z;
        const dy = groundH + 2 - controls.target.y;

        controls.target.x += dx;
        controls.target.z += dz;
        controls.target.y += dy;
        camera.position.x += dx;
        camera.position.z += dz;
        camera.position.y += dy;
        teleportRef.current = null;
      }

      controls.update();

      // Enforce Terrain Safety Floor
      const groundH = getMap2Height(camera.position.x, camera.position.z);
      clampCameraSafetyFloor(camera.position, groundH, PHYSICS_CONSTANTS.MIN_SAFETY_FLOOR_ORBIT);

      // Spectator boundary clamp
      clampWorldBounds(camera.position, PHYSICS_CONSTANTS.SPECTATOR_EXTENT);

      v.targetX = controls.target.x;
      v.targetZ = controls.target.z;
      v.camX = camera.position.x;
      v.camY = camera.position.y;
      v.camZ = camera.position.z;

      const sample = queryTerrain(controls.target.x, controls.target.z);
      v.biome = sample.biomeId;

      const dir = controls.target.clone().sub(camera.position);
      v.bearing = ((computeBearing(dir) * 180) / Math.PI) % 360;
      v.pitch = (computePitch(dir) * 180) / Math.PI;
      v.speed = 0;
      v.isGrounded = false;
      return;
    }

    // ---------------------------------------------------------------------------------------------
    // MODE 2: CINEMATIC MODE (8-Biome Catmull-Rom Spline Tour)
    // ---------------------------------------------------------------------------------------------
    if (mode === 'cine') {
      const cine = cineSplineRef.current;
      if (cine) {
        cine.progress = (cine.progress + dt * 0.02) % 1.0;
        const pos = cine.posCurve.getPointAt(cine.progress);
        const lookTarget = cine.targetCurve.getPointAt(cine.progress);

        camera.position.copy(pos);
        camera.lookAt(lookTarget);

        v.targetX = lookTarget.x;
        v.targetZ = lookTarget.z;
        v.camX = camera.position.x;
        v.camY = camera.position.y;
        v.camZ = camera.position.z;

        const sample = queryTerrain(lookTarget.x, lookTarget.z);
        v.biome = sample.biomeId;

        const dir = lookTarget.clone().sub(camera.position);
        v.bearing = ((computeBearing(dir) * 180) / Math.PI) % 360;
        v.pitch = (computePitch(dir) * 180) / Math.PI;
        v.speed = 4.5;
        v.isGrounded = false;
      }
      return;
    }

    // ---------------------------------------------------------------------------------------------
    // MODES 3 & 4: FLY & WALK MODES
    // ---------------------------------------------------------------------------------------------
    const phys = physRef.current;
    const vel = velRef.current;

    // Handle Teleportation
    if (tp) {
      camera.position.x = tp.x;
      camera.position.z = tp.z;
      const groundH = getMap2Height(tp.x, tp.z);

      if (mode === 'fly') {
        camera.position.y = Math.max(camera.position.y, groundH + 20);
      } else {
        const gy = groundH + PHYSICS_CONSTANTS.EYE_HEIGHT;
        camera.position.y = gy;
        phys.baseY = gy;
        phys.vy = 0;
        phys.grounded = true;
      }
      vel.x = 0;
      vel.z = 0;
      teleportRef.current = null;
    }

    // Apply Orientation
    scratchEuler.set(look.pitch, look.yaw, 0, 'YXZ');
    camera.quaternion.setFromEuler(scratchEuler);

    const sprinting = keys.has('ShiftLeft') || keys.has('ShiftRight');

    // Direction calculation
    scratchForward.set(0, 0, 0);
    camera.getWorldDirection(scratchForward);
    if (mode === 'walk') {
      scratchForward.y = 0;
      scratchForward.normalize();
    }
    scratchRight.crossVectors(scratchForward, UP_VECTOR).normalize();

    scratchWish.set(0, 0, 0);
    if (keys.has('KeyW') || keys.has('ArrowUp')) scratchWish.add(scratchForward);
    if (keys.has('KeyS') || keys.has('ArrowDown')) scratchWish.sub(scratchForward);
    if (keys.has('KeyA') || keys.has('ArrowLeft')) scratchWish.sub(scratchRight);
    if (keys.has('KeyD') || keys.has('ArrowRight')) scratchWish.add(scratchRight);

    const isMoving = scratchWish.lengthSq() > 1e-6;
    if (isMoving) scratchWish.normalize();

    // Max Speeds
    const maxSpeed =
      mode === 'fly'
        ? (worldSize * 0.15) * (sprinting ? 3.2 : 1.0)
        : (worldSize * 0.04) * (sprinting ? 2.2 : 1.0);

    const targetVx = isMoving ? scratchWish.x * maxSpeed : 0;
    const targetVz = isMoving ? scratchWish.z * maxSpeed : 0;

    const lambda =
      mode === 'walk' && !phys.grounded
        ? PHYSICS_CONSTANTS.AIR_LAMBDA
        : isMoving
        ? PHYSICS_CONSTANTS.ACCEL_LAMBDA
        : PHYSICS_CONSTANTS.FRICTION_LAMBDA;

    const k = 1 - Math.exp(-lambda * dt);
    vel.x += (targetVx - vel.x) * k;
    vel.z += (targetVz - vel.z) * k;

    // --- FLY MODE PHYSICS ---
    if (mode === 'fly') {
      camera.position.x += vel.x * dt;
      camera.position.z += vel.z * dt;

      // Vertical Q/E / Space / C movement
      let flyVy = 0;
      if (keys.has('KeyE') || keys.has('Space')) flyVy += maxSpeed * 0.75;
      if (keys.has('KeyQ') || keys.has('KeyC')) flyVy -= maxSpeed * 0.75;
      camera.position.y += flyVy * dt;

      // Soft Floor and Spectator Bounds
      const groundH = getMap2Height(camera.position.x, camera.position.z);
      clampCameraSafetyFloor(camera.position, groundH, PHYSICS_CONSTANTS.MIN_SAFETY_FLOOR_FLY);
      clampWorldBounds(camera.position, PHYSICS_CONSTANTS.SPECTATOR_EXTENT);

      v.camX = camera.position.x;
      v.camY = camera.position.y;
      v.camZ = camera.position.z;
      v.targetX = camera.position.x + scratchForward.x * 10;
      v.targetZ = camera.position.z + scratchForward.z * 10;

      const sample = queryTerrain(camera.position.x, camera.position.z);
      v.biome = sample.biomeId;
      v.bearing = ((look.yaw * 180) / Math.PI + 360) % 360;
      v.pitch = (look.pitch * 180) / Math.PI;
      v.speed = Math.sqrt(vel.x * vel.x + vel.z * vel.z + flyVy * flyVy);
      v.isGrounded = false;
      return;
    }

    // --- WALK MODE KINEMATICS & OBSTACLE COLLISION ---
    if (mode === 'walk') {
      const prospectiveX = camera.position.x + vel.x * dt;
      const prospectiveZ = camera.position.z + vel.z * dt;
      const prospectiveSample = queryTerrain(prospectiveX, prospectiveZ);

      // Slope Sliding: Deflect uphill velocity downhill on steep terrain (s > 0.45)
      if (prospectiveSample.steepness > PHYSICS_CONSTANTS.SLOPE_LIMIT) {
        const normalH = new THREE.Vector3(prospectiveSample.normal.x, 0, prospectiveSample.normal.z);
        if (normalH.lengthSq() > 1e-6) {
          normalH.normalize();
          const dot = vel.x * normalH.x + vel.z * normalH.z;
          if (dot < 0) {
            // Deflect velocity along cliff tangent
            vel.x -= normalH.x * dot;
            vel.z -= normalH.z * dot;
          }
          // Downhill gravity push
          vel.x += normalH.x * PHYSICS_CONSTANTS.GRAVITY * 0.5 * dt;
          vel.z += normalH.z * PHYSICS_CONSTANTS.GRAVITY * 0.5 * dt;
        }
      }

      // Jump
      const jumpKey = keys.has('Space');
      if (jumpKey && phys.grounded) {
        phys.vy = PHYSICS_CONSTANTS.JUMP_VELOCITY;
        phys.grounded = false;
      }

      // Gravity integration
      if (!phys.grounded) {
        phys.vy -= PHYSICS_CONSTANTS.GRAVITY * dt;
        phys.baseY += phys.vy * dt;
      }

      // Position update
      camera.position.x += vel.x * dt;
      camera.position.z += vel.z * dt;

      // Tree trunk collider push-out
      if (colliderIndex) {
        const nearby = colliderIndex.queryNearby(camera.position.x, camera.position.z, 3.0);
        if (nearby.length > 0) {
          resolveFloraOverlap(camera.position, nearby, PHYSICS_CONSTANTS.PLAYER_RADIUS);
        }
      }

      // World boundary clamp
      clampWorldBounds(camera.position, PHYSICS_CONSTANTS.WORLD_EXTENT);

      // Ground clamping & Head-bob
      const currentSample = queryTerrain(camera.position.x, camera.position.z);
      const groundY = currentSample.height + PHYSICS_CONSTANTS.EYE_HEIGHT;

      if (phys.baseY <= groundY) {
        phys.baseY = groundY;
        phys.vy = 0;
        phys.grounded = true;
      } else {
        phys.grounded = false;
      }

      // Head-bob calculation while walking on ground
      let bobY = 0;
      if (phys.grounded && isMoving) {
        const horizontalSpeed = Math.sqrt(vel.x * vel.x + vel.z * vel.z);
        phys.strideDist += horizontalSpeed * dt;
        const bob = calculateHeadBob(phys.strideDist);
        bobY = bob.offsetY;
      }

      camera.position.y = phys.baseY + bobY;

      v.camX = camera.position.x;
      v.camY = camera.position.y;
      v.camZ = camera.position.z;
      v.targetX = camera.position.x + scratchForward.x * 10;
      v.targetZ = camera.position.z + scratchForward.z * 10;
      v.biome = currentSample.biomeId;
      v.bearing = ((look.yaw * 180) / Math.PI + 360) % 360;
      v.pitch = (look.pitch * 180) / Math.PI;
      v.speed = Math.sqrt(vel.x * vel.x + vel.z * vel.z);
      v.isGrounded = phys.grounded;
    }
  });

  return (
    <orbitControls
      ref={controlsRef}
      args={[camera, gl.domElement]}
      enableDamping={true}
      dampingFactor={0.08}
      minDistance={2}
      maxDistance={350}
      maxPolarAngle={Math.PI / 2.05}
    />
  );
};

export default Map2CameraRig;
