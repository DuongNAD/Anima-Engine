import React, { useEffect, useRef } from 'react';
import { extend, useFrame, useThree } from '@react-three/fiber';
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import type { World } from './utils/worldGen';
import { sampleMeshHeight } from './utils/worldSample';
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
//   fly    free spectator: WASD + Q/E for down/up, hold left mouse and drag to look,
//          Shift to sprint
//   walk   first-person on foot: WASD, camera glued to the terrain surface (wades at the
//          shore instead of sinking), drag to look, Shift to jog
//   top    straight-down map view: pan + zoom only
//   cine   hands-off cinematic: slow auto-orbit around the current point of interest
// Minimap teleports work in every mode. The rig reports camera/target world XZ into
// `viewRef` each frame for the minimap and the HUD readout.
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
  const lookRef = useRef({ yaw: 0, pitch: -0.4, dragging: false, lastX: 0, lastY: 0 });
  const cineRef = useRef({ angle: 0, tx: 0, tz: 0 });

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

  // Keyboard state (fly / walk). Codes, not chars, so layout doesn't matter.
  useEffect(() => {
    const down = (e: KeyboardEvent) => keysRef.current.add(e.code);
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

  // Drag-to-look (fly / walk).
  useEffect(() => {
    if (mode !== 'fly' && mode !== 'walk') return;
    const el = gl.domElement;
    const look = lookRef.current;
    const downH = (e: PointerEvent) => {
      if (e.button !== 0) return;
      look.dragging = true;
      look.lastX = e.clientX;
      look.lastY = e.clientY;
    };
    const moveH = (e: PointerEvent) => {
      if (!look.dragging) return;
      look.yaw -= (e.clientX - look.lastX) * 0.0035;
      look.pitch = Math.max(-1.35, Math.min(1.35, look.pitch - (e.clientY - look.lastY) * 0.0032));
      look.lastX = e.clientX;
      look.lastY = e.clientY;
    };
    const upH = () => {
      look.dragging = false;
    };
    el.addEventListener('pointerdown', downH);
    window.addEventListener('pointermove', moveH);
    window.addEventListener('pointerup', upH);
    return () => {
      el.removeEventListener('pointerdown', downH);
      window.removeEventListener('pointermove', moveH);
      window.removeEventListener('pointerup', upH);
    };
  }, [mode, gl]);

  // Mode entry: seed the new controller from wherever the camera currently is.
  useEffect(() => {
    const look = lookRef.current;
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
        camera.position.y = surfaceY(camera.position.x, camera.position.z) + EYE;
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode]);

  useFrame((_, delta) => {
    const dt = Math.min(delta, 0.1);
    const look = lookRef.current;
    const keys = keysRef.current;
    const v = viewRef.current;
    const tp = teleportRef.current;

    if (mode === 'orbit' || mode === 'top') {
      const controls = controlsRef.current;
      if (!controls) return;
      if (tp) {
        const dx = tp.x - controls.target.x;
        const dz = tp.z - controls.target.z;
        controls.target.x += dx;
        controls.target.z += dz;
        camera.position.x += dx;
        camera.position.z += dz;
        teleportRef.current = null;
      }
      controls.update();
      v.targetX = controls.target.x;
      v.targetZ = controls.target.z;
      v.camX = camera.position.x;
      v.camZ = camera.position.z;
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
      return;
    }

    // fly / walk
    if (tp) {
      camera.position.x = tp.x;
      camera.position.z = tp.z;
      if (mode === 'fly') camera.position.y = Math.max(camera.position.y, surfaceY(tp.x, tp.z) + 20);
      teleportRef.current = null;
    }

    camera.quaternion.setFromEuler(new THREE.Euler(look.pitch, look.yaw, 0, 'YXZ'));

    const sprint = keys.has('ShiftLeft') || keys.has('ShiftRight') ? 3.2 : 1;
    const base = mode === 'fly' ? renderSize * 0.1 : renderSize * 0.02;
    const speed = base * sprint * dt;

    const forward = new THREE.Vector3();
    camera.getWorldDirection(forward);
    if (mode === 'walk') {
      forward.y = 0;
      forward.normalize();
    }
    const right = new THREE.Vector3().crossVectors(forward, new THREE.Vector3(0, 1, 0)).normalize();

    if (keys.has('KeyW') || keys.has('ArrowUp')) camera.position.addScaledVector(forward, speed);
    if (keys.has('KeyS') || keys.has('ArrowDown')) camera.position.addScaledVector(forward, -speed);
    if (keys.has('KeyA') || keys.has('ArrowLeft')) camera.position.addScaledVector(right, -speed);
    if (keys.has('KeyD') || keys.has('ArrowRight')) camera.position.addScaledVector(right, speed);
    if (mode === 'fly') {
      if (keys.has('KeyE') || keys.has('Space')) camera.position.y += speed;
      if (keys.has('KeyQ')) camera.position.y -= speed;
    }

    // Keep the camera over (or near) the map.
    const lim = renderSize * 0.75;
    camera.position.x = Math.max(-lim, Math.min(lim, camera.position.x));
    camera.position.z = Math.max(-lim, Math.min(lim, camera.position.z));

    if (mode === 'walk') {
      camera.position.y = surfaceY(camera.position.x, camera.position.z) + EYE;
    } else {
      const floor = surfaceY(camera.position.x, camera.position.z) + 1.2;
      camera.position.y = Math.max(floor, Math.min(renderSize * 2.5, camera.position.y));
    }

    v.camX = camera.position.x;
    v.camZ = camera.position.z;
    v.targetX = camera.position.x + forward.x * 60;
    v.targetZ = camera.position.z + forward.z * 60;
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
