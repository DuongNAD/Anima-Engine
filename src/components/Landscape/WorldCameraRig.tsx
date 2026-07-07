import React, { useEffect, useMemo, useRef } from 'react';
import { extend, useFrame, useThree } from '@react-three/fiber';
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import type { World } from './utils/worldGen';
import { FloraType } from './utils/worldGen';
import { sampleMeshHeight, biomeAt } from './utils/worldSample';
import type { CameraView } from './WorldMinimap';

extend({ OrbitControls });

declare global {
  namespace JSX {
    interface IntrinsicElements {
      orbitControls: any;
    }
  }
}

// ---------------------------------------------------------------------------------------
// WorldCameraRig — five ways to see the world:
//   orbit  classic inspect camera (drag to orbit, wheel to zoom, right-drag to pan)
//   fly    free spectator: WASD + Q/E (or Space) for down/up, Shift to sprint
//   walk   first-person on foot: WASD with real momentum, gravity + Space to jump, head-bob,
//          camera glued to the terrain surface (wades at the shore instead of sinking)
//   top    straight-down map view: pan + zoom only
//   cine   hands-off cinematic: slow auto-orbit around the current point of interest
//
// In fly/walk, clicking the viewport LOCKS the pointer for free first-person mouse-look
// (Esc releases it); where pointer-lock is unavailable it falls back to click-drag look.
// Minimap teleports work in every mode. The rig reports camera/target world XZ — plus the
// biome underfoot, smoothed FPS and pointer-lock state — into `viewRef` for the HUD/minimap.
// ---------------------------------------------------------------------------------------

export type CameraMode = 'orbit' | 'fly' | 'walk' | 'top' | 'cine';

export interface WorldCameraRigProps {
  mode: CameraMode;
  world: World;
  renderSize: number;
  heightRatio: number;
  meshResolution: number;
  viewRef: React.MutableRefObject<CameraView>;
  teleportRef: React.MutableRefObject<{ x: number; z: number } | null>;
}

const EYE = 2.1; // walking eye height (world units)
const BASE_FOV = 55;
const GRAVITY = 34; // world units / s^2
const JUMP_V = 12.5; // jump take-off velocity (units/s) -> ~2.3u high, ~0.75s airtime
const BOB_FREQ = 0.9; // head-bob cycles per world unit travelled
const BOB_AMP = 0.13; // head-bob amplitude (world units)
// Exponential-smoothing rates (per second) for velocity — frame-rate independent.
const ACCEL_LAMBDA = 11; // spin-up while a direction is held
const FRICTION_LAMBDA = 14; // spin-down when keys released (snappier stop)
const AIR_LAMBDA = 2.5; // reduced air control while airborne (walk)
// Keys we swallow the browser default for (Space/arrows scroll the page otherwise).
const NAV_KEYS = new Set(['Space', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight']);

const clampPitch = (p: number) => Math.max(-1.35, Math.min(1.35, p));

export const WorldCameraRig: React.FC<WorldCameraRigProps> = ({
  mode,
  world,
  renderSize,
  heightRatio,
  meshResolution,
  viewRef,
  teleportRef,
}) => {
  const { camera, gl } = useThree();
  const controlsRef = useRef<any>(null);

  const keysRef = useRef<Set<string>>(new Set());
  const lookRef = useRef({ yaw: 0, pitch: -0.4, dragging: false, locked: false, lastX: 0, lastY: 0 });
  const cineRef = useRef({ angle: 0, tx: 0, tz: 0 });
  // Horizontal velocity (units/s) for momentum-based fly/walk movement.
  const velRef = useRef({ x: 0, z: 0 });
  // Walk vertical physics: eye base height, vertical velocity, grounded flag, jump edge-detect
  // and accumulated stride distance driving the head-bob.
  const physRef = useRef({ baseY: 0, vy: 0, grounded: true, jumpHeld: false, bobDist: 0 });

  const heightUnits = renderSize * heightRatio;
  const seaY = world.seaLevel * heightUnits;

  /** Terrain-or-water surface height under world (x, z) — what a walker stands on. */
  const surfaceY = (x: number, z: number): number => {
    const u = x / renderSize + 0.5;
    const v = z / renderSize + 0.5;
    if (u < 0 || u > 1 || v < 0 || v > 1) return seaY;
    const groundY = sampleMeshHeight(world, u, v, meshResolution) * heightUnits;
    // Lakes: stand on the still-water surface rather than the drowned lakebed.
    const size = world.size;
    const cx = Math.min(size - 1, Math.max(0, Math.round(u * (size - 1))));
    const cz = Math.min(size - 1, Math.max(0, Math.round(v * (size - 1))));
    const lake = world.water[cz * size + cx];
    return Math.max(groundY, seaY, lake > 0 ? lake * heightUnits : 0);
  };

  // Static tree colliders for walk mode: trunked flora bucketed into an 8-unit grid, so the
  // player capsule can be pushed out of trunks with a 9-cell lookup instead of a 100k scan.
  const treeGrid = useMemo(() => {
    const TALL = new Set<number>([
      FloraType.Pine,
      FloraType.Round,
      FloraType.Jungle,
      FloraType.Cactus,
      FloraType.Acacia,
      FloraType.Palm,
      FloraType.DeadTree,
    ]);
    const grid = new Map<number, number[]>();
    const toWorld = renderSize / world.size;
    for (let i = 0; i < world.floraCount; i++) {
      if (!TALL.has(world.floraType[i])) continue;
      const x = world.floraX[i] * toWorld;
      const z = world.floraZ[i] * toWorld;
      const key = Math.floor((x + renderSize) / 8) * 2048 + Math.floor((z + renderSize) / 8);
      let arr = grid.get(key);
      if (!arr) grid.set(key, (arr = []));
      arr.push(i);
    }
    return grid;
  }, [world, renderSize]);

  // Keyboard state (fly / walk). Codes, not chars, so layout doesn't matter.
  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      keysRef.current.add(e.code);
      if (NAV_KEYS.has(e.code)) e.preventDefault();
    };
    const up = (e: KeyboardEvent) => keysRef.current.delete(e.code);
    const blur = () => keysRef.current.clear();
    window.addEventListener('keydown', down);
    window.addEventListener('keyup', up);
    window.addEventListener('blur', blur);
    return () => {
      window.removeEventListener('keydown', down);
      window.removeEventListener('keyup', up);
      window.removeEventListener('blur', blur);
    };
  }, []);

  // Look controls (fly / walk): pointer-lock first-person mouse-look, with click-drag fallback
  // when lock is unavailable (headless/permission-denied).
  useEffect(() => {
    if (mode !== 'fly' && mode !== 'walk') return;
    const el = gl.domElement;
    const look = lookRef.current;
    const SENS_LOCK = 0.0022;

    const onDown = (e: PointerEvent) => {
      if (e.button !== 0) return;
      // Prefer a true pointer lock; if it isn't available, remember the drag origin.
      if (document.pointerLockElement !== el) el.requestPointerLock?.();
      look.dragging = true;
      look.lastX = e.clientX;
      look.lastY = e.clientY;
    };
    const onMove = (e: PointerEvent) => {
      if (look.locked) {
        look.yaw -= (e.movementX || 0) * SENS_LOCK;
        look.pitch = clampPitch(look.pitch - (e.movementY || 0) * SENS_LOCK);
      } else if (look.dragging) {
        look.yaw -= (e.clientX - look.lastX) * 0.0035;
        look.pitch = clampPitch(look.pitch - (e.clientY - look.lastY) * 0.0032);
        look.lastX = e.clientX;
        look.lastY = e.clientY;
      }
    };
    const onUp = () => {
      look.dragging = false;
    };
    const onLockChange = () => {
      look.locked = document.pointerLockElement === el;
      viewRef.current.locked = look.locked;
    };
    el.addEventListener('pointerdown', onDown);
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    document.addEventListener('pointerlockchange', onLockChange);
    return () => {
      el.removeEventListener('pointerdown', onDown);
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      document.removeEventListener('pointerlockchange', onLockChange);
      look.locked = false;
      viewRef.current.locked = false;
      if (document.pointerLockElement === el) document.exitPointerLock?.();
    };
  }, [mode, gl, viewRef]);

  // Mode entry: seed the new controller from wherever the camera currently is.
  useEffect(() => {
    const look = lookRef.current;
    velRef.current.x = 0;
    velRef.current.z = 0;
    if (mode === 'fly' || mode === 'walk') {
      const dir = new THREE.Vector3();
      camera.getWorldDirection(dir);
      look.yaw = Math.atan2(-dir.x, -dir.z);
      look.pitch = Math.asin(Math.max(-1, Math.min(1, dir.y)));
      if (mode === 'walk') {
        // Walking starts AT the point being looked at (orbit target / minimap teleport),
        // not at the orbit camera's pulled-back position — which often floats off-shore.
        const v = viewRef.current;
        camera.position.x = v.targetX;
        camera.position.z = v.targetZ;
        const gy = surfaceY(camera.position.x, camera.position.z) + EYE;
        camera.position.y = gy;
        physRef.current.baseY = gy;
        physRef.current.vy = 0;
        physRef.current.grounded = true;
        physRef.current.bobDist = 0;
        look.pitch = Math.max(-0.35, look.pitch);
      }
      (document.activeElement as HTMLElement | null)?.blur?.();
    } else if (mode === 'top') {
      const v = viewRef.current;
      camera.position.set(v.targetX, renderSize * 1.35, v.targetZ + 0.01);
    } else if (mode === 'cine') {
      const v = viewRef.current;
      cineRef.current.tx = v.targetX;
      cineRef.current.tz = v.targetZ;
      cineRef.current.angle = Math.atan2(camera.position.z - v.targetZ, camera.position.x - v.targetX);
    }
    // Non-explorer modes always use the base field of view.
    if (mode !== 'fly' && mode !== 'walk') {
      const cam = camera as THREE.PerspectiveCamera;
      if (cam.isPerspectiveCamera && cam.fov !== BASE_FOV) {
        cam.fov = BASE_FOV;
        cam.updateProjectionMatrix();
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode]);

  useFrame((_, delta) => {
    const dt = Math.min(delta, 0.1);
    const look = lookRef.current;
    const keys = keysRef.current;
    const v = viewRef.current;
    const tp = teleportRef.current;

    // Smoothed FPS readout (uses the raw frame delta, not the movement-clamped dt).
    const rawFps = delta > 0 ? 1 / delta : 0;
    v.fps = v.fps ? v.fps * 0.9 + rawFps * 0.1 : rawFps;

    if (mode === 'orbit' || mode === 'top') {
      const controls = controlsRef.current;
      if (!controls) return;
      if (tp) {
        // Shift the whole rig, INCLUDING height: the orbit target must sit on the terrain
        // surface at the destination, or zooming in dives the camera under a mountain.
        const dx = tp.x - controls.target.x;
        const dz = tp.z - controls.target.z;
        const dy = surfaceY(tp.x, tp.z) + 2 - controls.target.y;
        controls.target.x += dx;
        controls.target.z += dz;
        controls.target.y += dy;
        camera.position.x += dx;
        camera.position.z += dz;
        camera.position.y += dy;
        teleportRef.current = null;
      }
      controls.update();
      // Hard floor: the camera never sinks below the terrain (single-sided mesh -> the
      // world would vanish into sky colour).
      const minY = surfaceY(camera.position.x, camera.position.z) + 2;
      if (camera.position.y < minY) camera.position.y = minY;
      v.targetX = controls.target.x;
      v.targetZ = controls.target.z;
      v.camX = camera.position.x;
      v.camZ = camera.position.z;
      v.biome = biomeAt(world, controls.target.x, controls.target.z, renderSize);
      return;
    }

    if (mode === 'cine') {
      const cine = cineRef.current;
      if (tp) {
        cine.tx = tp.x;
        cine.tz = tp.z;
        teleportRef.current = null;
      }
      cine.angle += dt * 0.1;
      const r = renderSize * 0.42;
      const groundY = surfaceY(cine.tx, cine.tz);
      camera.position.set(
        cine.tx + Math.cos(cine.angle) * r,
        groundY + renderSize * 0.2,
        cine.tz + Math.sin(cine.angle) * r,
      );
      camera.lookAt(cine.tx, groundY + heightUnits * 0.1, cine.tz);
      v.targetX = cine.tx;
      v.targetZ = cine.tz;
      v.camX = camera.position.x;
      v.camZ = camera.position.z;
      v.biome = biomeAt(world, cine.tx, cine.tz, renderSize);
      return;
    }

    // fly / walk
    const phys = physRef.current;
    const vel = velRef.current;
    if (tp) {
      camera.position.x = tp.x;
      camera.position.z = tp.z;
      if (mode === 'fly') {
        camera.position.y = Math.max(camera.position.y, surfaceY(tp.x, tp.z) + 20);
      } else {
        const gy = surfaceY(tp.x, tp.z) + EYE;
        camera.position.y = gy;
        phys.baseY = gy;
        phys.vy = 0;
        phys.grounded = true;
      }
      vel.x = 0;
      vel.z = 0;
      teleportRef.current = null;
    }

    camera.quaternion.setFromEuler(new THREE.Euler(look.pitch, look.yaw, 0, 'YXZ'));

    const sprinting = keys.has('ShiftLeft') || keys.has('ShiftRight');

    // Direction the player wants to go (screen-forward projected flat for walking).
    const forward = new THREE.Vector3();
    camera.getWorldDirection(forward);
    if (mode === 'walk') {
      forward.y = 0;
      forward.normalize();
    }
    const right = new THREE.Vector3().crossVectors(forward, new THREE.Vector3(0, 1, 0)).normalize();

    const wish = new THREE.Vector3();
    if (keys.has('KeyW') || keys.has('ArrowUp')) wish.add(forward);
    if (keys.has('KeyS') || keys.has('ArrowDown')) wish.sub(forward);
    if (keys.has('KeyA') || keys.has('ArrowLeft')) wish.sub(right);
    if (keys.has('KeyD') || keys.has('ArrowRight')) wish.add(right);
    const moving = wish.lengthSq() > 1e-9;
    if (moving) wish.normalize();

    // Top speed (units/s): same magnitudes as before, now reached via smoothed acceleration.
    const maxSpeed =
      mode === 'fly'
        ? renderSize * 0.1 * (sprinting ? 3.2 : 1)
        : renderSize * 0.02 * (sprinting ? 2.2 : 1);
    const targetVx = moving ? wish.x * maxSpeed : 0;
    const targetVz = moving ? wish.z * maxSpeed : 0;
    const lambda =
      mode === 'walk' && !phys.grounded ? AIR_LAMBDA : moving ? ACCEL_LAMBDA : FRICTION_LAMBDA;
    const k = 1 - Math.exp(-lambda * dt);
    vel.x += (targetVx - vel.x) * k;
    vel.z += (targetVz - vel.z) * k;

    if (mode === 'fly') {
      camera.position.x += vel.x * dt;
      camera.position.z += vel.z * dt;
      let vy = 0;
      if (keys.has('KeyE') || keys.has('Space')) vy += 1;
      if (keys.has('KeyQ')) vy -= 1;
      camera.position.y += vy * maxSpeed * dt;
    } else {
      // WALK: a real character step — max-slope limit with axis sliding, then trunk colliders.
      const cx0 = camera.position.x;
      const cz0 = camera.position.z;
      let nx = cx0 + vel.x * dt;
      let nz = cz0 + vel.z * dt;
      const step = Math.hypot(vel.x * dt, vel.z * dt);
      if (step > 1e-6) {
        // Slopes steeper than ~43° can't be climbed: try sliding along one axis, else stop.
        const MAXG = 0.95;
        const g0 = surfaceY(cx0, cz0);
        if (surfaceY(nx, nz) - g0 > step * MAXG) {
          if (surfaceY(cx0 + vel.x * dt, cz0) - g0 <= Math.abs(vel.x * dt) * MAXG + 1e-4) nz = cz0;
          else if (surfaceY(cx0, cz0 + vel.z * dt) - g0 <= Math.abs(vel.z * dt) * MAXG + 1e-4) nx = cx0;
          else {
            nx = cx0;
            nz = cz0;
          }
        }
        // Push the player capsule out of tree trunks (9 surrounding grid cells).
        const toWorld = renderSize / world.size;
        const gcx = Math.floor((nx + renderSize) / 8);
        const gcz = Math.floor((nz + renderSize) / 8);
        for (let dxc = -1; dxc <= 1; dxc++) {
          for (let dzc = -1; dzc <= 1; dzc++) {
            const arr = treeGrid.get((gcx + dxc) * 2048 + (gcz + dzc));
            if (!arr) continue;
            for (let kk = 0; kk < arr.length; kk++) {
              const ti = arr[kk];
              const tx = world.floraX[ti] * toWorld;
              const tz = world.floraZ[ti] * toWorld;
              const r = 0.45 + world.floraScale[ti] * 0.25;
              const ddx = nx - tx;
              const ddz = nz - tz;
              const d2 = ddx * ddx + ddz * ddz;
              if (d2 < r * r && d2 > 1e-6) {
                const d = Math.sqrt(d2);
                nx = tx + (ddx / d) * r;
                nz = tz + (ddz / d) * r;
              }
            }
          }
        }
      }
      // Reconcile velocity with what actually happened (a wall/slope zeroes that component,
      // so momentum doesn't build up against it and the head-bob doesn't fake a stride).
      vel.x = (nx - cx0) / dt;
      vel.z = (nz - cz0) / dt;
      camera.position.x = nx;
      camera.position.z = nz;
    }

    // Keep the camera over (or near) the map.
    const lim = renderSize * 0.75;
    camera.position.x = Math.max(-lim, Math.min(lim, camera.position.x));
    camera.position.z = Math.max(-lim, Math.min(lim, camera.position.z));

    if (mode === 'walk') {
      // Gravity + jump on the eye BASE height; head-bob is a cosmetic offset on top so it
      // never fights the grounded test.
      const groundY = surfaceY(camera.position.x, camera.position.z) + EYE;
      phys.vy -= GRAVITY * dt;
      phys.baseY += phys.vy * dt;
      if (phys.baseY <= groundY) {
        phys.baseY = groundY;
        phys.vy = 0;
        if (keys.has('Space') && !phys.jumpHeld) {
          phys.vy = JUMP_V;
          phys.grounded = false;
        } else {
          phys.grounded = true;
        }
      } else {
        phys.grounded = false;
      }
      phys.jumpHeld = keys.has('Space');

      const actualSpeed = Math.hypot(vel.x, vel.z);
      let bob = 0;
      if (phys.grounded && actualSpeed > 0.5) {
        phys.bobDist += actualSpeed * dt;
        const f = Math.min(1, actualSpeed / (renderSize * 0.02));
        bob = Math.sin(phys.bobDist * BOB_FREQ) * BOB_AMP * f;
      }
      camera.position.y = phys.baseY + bob;
    } else {
      const floor = surfaceY(camera.position.x, camera.position.z) + 1.2;
      camera.position.y = Math.max(floor, Math.min(renderSize * 2.5, camera.position.y));
    }

    // Sprint field-of-view kick — a small speed rush while actually moving fast.
    const cam = camera as THREE.PerspectiveCamera;
    if (cam.isPerspectiveCamera) {
      const targetFov = BASE_FOV + (sprinting && moving ? 7 : 0);
      cam.fov += (targetFov - cam.fov) * (1 - Math.exp(-6 * dt));
      cam.updateProjectionMatrix();
    }

    v.camX = camera.position.x;
    v.camZ = camera.position.z;
    v.targetX = camera.position.x + forward.x * 60;
    v.targetZ = camera.position.z + forward.z * 60;
    v.biome = biomeAt(world, camera.position.x, camera.position.z, renderSize);
  });

  if (mode !== 'orbit' && mode !== 'top') return null;
  return (
    <orbitControls
      ref={controlsRef}
      args={[camera, gl.domElement]}
      enableDamping
      dampingFactor={0.08}
      enableRotate={mode === 'orbit'}
      screenSpacePanning={false}
      maxPolarAngle={Math.PI / 2.05}
      minDistance={renderSize * (mode === 'top' ? 0.15 : 0.06)}
      maxDistance={renderSize * (mode === 'top' ? 2.4 : 3.2)}
    />
  );
};

export default WorldCameraRig;
