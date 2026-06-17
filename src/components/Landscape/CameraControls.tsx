import React, { useEffect, useRef } from 'react';
import { extend, useFrame, useThree } from '@react-three/fiber';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import * as THREE from 'three';
import { audioManager } from './utils/audioManager';

extend({ OrbitControls });

declare global {
  namespace JSX {
    interface IntrinsicElements {
      orbitControls: any;
    }
  }
}

interface CameraControlsProps {
  cameraMode: 'orbit' | 'fly' | 'cinematic' | 'map';
  terrainHeightMap?: Float32Array;
  gridWidth?: number;
  gridHeight?: number;
}

export const CameraControls: React.FC<CameraControlsProps> = ({
  cameraMode,
  terrainHeightMap,
  gridWidth = 64,
  gridHeight = 64,
}) => {
  const { camera, gl } = useThree();
  const keysPressed = useRef<{ [key: string]: boolean }>({});
  const orbitControlsRef = useRef<any>(null);
  const rotation = useRef({ yaw: 0, pitch: 0 });

  // Persistent reference to the current look-at target across modes
  const activeTarget = useRef<THREE.Vector3>(new THREE.Vector3(0, 0, 0));

  // Map mode parameters
  const tdZoom = useRef<number>(150);
  const tdTarget = useRef<THREE.Vector3>(new THREE.Vector3(0, 0, 0));

  // Fly mode parameters
  const flyPos = useRef<THREE.Vector3>(new THREE.Vector3(0, 80, 200));

  const getTerrainHeight = (x: number, z: number): number => {
    if (!terrainHeightMap) return 5.5;
    const gx = x + gridWidth / 2;
    const gy = z + gridHeight / 2;
    if (gx >= 0 && gx < gridWidth && gy >= 0 && gy < gridHeight) {
      const x0 = Math.floor(gx);
      const y0 = Math.floor(gy);
      const x1 = Math.min(gridWidth - 1, x0 + 1);
      const y1 = Math.min(gridHeight - 1, y0 + 1);
      const tx = gx - x0;
      const ty = gy - y0;
      const h00 = terrainHeightMap[y0 * gridWidth + x0] ?? 0;
      const h10 = terrainHeightMap[y0 * gridWidth + x1] ?? 0;
      const h01 = terrainHeightMap[y1 * gridWidth + x0] ?? 0;
      const h11 = terrainHeightMap[y1 * gridWidth + x1] ?? 0;
      return (h00 * (1 - tx) * (1 - ty) + h10 * tx * (1 - ty) + h01 * (1 - tx) * ty + h11 * tx * ty) * 1.8;
    }
    return 5.5;
  };

  // Expose helper globally and register teleport handler
  useEffect(() => {
    (window as any).getTerrainHeight = getTerrainHeight;
    (window as any).globalTerrainHeightMap = terrainHeightMap;

    (window as any).teleportCameraTarget = (wx: number, wz: number) => {
      const th = getTerrainHeight(wx, wz);
      activeTarget.current.set(wx, th, wz);

      if (cameraMode === 'orbit') {
        if (orbitControlsRef.current && orbitControlsRef.current.target) {
          orbitControlsRef.current.target.copy(activeTarget.current);
          orbitControlsRef.current.update();
        }
      } else if (cameraMode === 'fly') {
        flyPos.current.set(wx, th + 15, wz);
        camera.position.copy(flyPos.current);
      } else if (cameraMode === 'map') {
        tdTarget.current.copy(activeTarget.current);
      }
    };
  }, [terrainHeightMap, gridWidth, gridHeight, cameraMode, camera]);

  // Keep track of the active camera reference for Minimap
  useEffect(() => {
    (window as any).activeCamera = camera;
  }, [camera]);

  // Handle keyboard events
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      keysPressed.current[e.key.toLowerCase()] = true;
    };
    const handleKeyUp = (e: KeyboardEvent) => {
      keysPressed.current[e.key.toLowerCase()] = false;
    };
    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
    };
  }, []);

  // Handle wheel events for zoom in Map mode
  useEffect(() => {
    const handleWheel = (e: WheelEvent) => {
      if (cameraMode === 'map') {
        // adjust map height (zoom)
        tdZoom.current = Math.max(30, Math.min(400, tdZoom.current + e.deltaY * 0.2));
      }
    };
    window.addEventListener('wheel', handleWheel, { passive: true });
    return () => {
      window.removeEventListener('wheel', handleWheel);
    };
  }, [cameraMode]);

  // Sync state between camera mode transitions
  const lastMode = useRef(cameraMode);
  useEffect(() => {
    if (cameraMode === lastMode.current) return;

    if (cameraMode === 'orbit') {
      if (orbitControlsRef.current && orbitControlsRef.current.target) {
        orbitControlsRef.current.target.copy(activeTarget.current);
        orbitControlsRef.current.update();
      }
    } else if (cameraMode === 'fly') {
      // Set fly position to current camera position
      flyPos.current.copy(camera.position);
      const euler = new THREE.Euler().setFromQuaternion(camera.quaternion, 'YXZ');
      rotation.current.yaw = euler.y;
      rotation.current.pitch = euler.x;
    } else if (cameraMode === 'map') {
      tdTarget.current.copy(activeTarget.current);
      tdZoom.current = Math.max(50, camera.position.y);
    }

    lastMode.current = cameraMode;
  }, [cameraMode, camera]);

  useEffect(() => {
    if (cameraMode === 'fly') {
      const euler = new THREE.Euler().setFromQuaternion(camera.quaternion, 'YXZ');
      rotation.current.yaw = euler.y;
      rotation.current.pitch = euler.x;
    }
  }, [cameraMode, camera]);

  useEffect(() => {
    if (cameraMode !== 'fly') {
      if (document.pointerLockElement === gl.domElement) {
        document.exitPointerLock();
      }
      return;
    }

    const handleMouseMove = (e: MouseEvent) => {
      if (document.pointerLockElement !== gl.domElement) return;
      rotation.current.yaw -= e.movementX * 0.002;
      rotation.current.pitch = Math.max(
        -Math.PI / 3,
        Math.min(Math.PI / 3, rotation.current.pitch - e.movementY * 0.002)
      );
    };

    const lockPointer = () => {
      if (document.pointerLockElement !== gl.domElement) {
        gl.domElement.requestPointerLock();
      }
    };

    gl.domElement.addEventListener('click', lockPointer);
    window.addEventListener('mousemove', handleMouseMove);

    return () => {
      gl.domElement.removeEventListener('click', lockPointer);
      window.removeEventListener('mousemove', handleMouseMove);
    };
  }, [cameraMode, gl]);

  useFrame((state) => {
    if (!camera) return;

    if (cameraMode === 'orbit') {
      if (orbitControlsRef.current && typeof orbitControlsRef.current.update === 'function') {
        orbitControlsRef.current.update();
        if (orbitControlsRef.current.target) {
          activeTarget.current.copy(orbitControlsRef.current.target);
        }
      }
      // Clamp Orbit mode camera height to prevent underground clipping
      const terrainHeight = getTerrainHeight(camera.position.x, camera.position.z);
      const minHeight = Math.max(5.5, terrainHeight) + 3.0;
      if (camera.position.y < minHeight) {
        camera.position.y = minHeight;
      }
    } else if (cameraMode === 'fly') {
      // Apply rotation from mouse look
      camera.rotation.set(0, 0, 0);
      camera.rotateY(rotation.current.yaw);
      camera.rotateX(rotation.current.pitch);

      let { x, y, z } = camera.position;
      const baseSpeed = 0.5;
      const speed = keysPressed.current['shift'] ? baseSpeed * 3 : baseSpeed;
      const forward = new THREE.Vector3(0, 0, -1).applyQuaternion(camera.quaternion);
      const right = new THREE.Vector3(1, 0, 0).applyQuaternion(camera.quaternion);

      const moveDir = new THREE.Vector3();
      if (keysPressed.current['w']) moveDir.add(forward);
      if (keysPressed.current['s']) moveDir.sub(forward);
      if (keysPressed.current['a']) moveDir.sub(right);
      if (keysPressed.current['d']) moveDir.add(right);

      if (moveDir.lengthSq() > 0) {
        moveDir.normalize().multiplyScalar(speed);
        x += moveDir.x;
        y += moveDir.y;
        z += moveDir.z;
      }

      if (keysPressed.current[' '] || keysPressed.current['space']) {
        y += speed;
      }
      if (keysPressed.current['e']) {
        y -= speed;
      }

      // Bound checks relative to map boundaries
      const halfWidth = gridWidth / 2;
      const halfHeight = gridHeight / 2;
      x = Math.max(-halfWidth, Math.min(halfWidth, x));
      z = Math.max(-halfHeight, Math.min(halfHeight, z));

      const terrainHeight = getTerrainHeight(x, z);
      const minHeight = Math.max(5.5, terrainHeight) + 3.0;
      if (y < minHeight) {
        y = minHeight;
      }

      camera.position.x = x;
      camera.position.y = y;
      camera.position.z = z;
      flyPos.current.copy(camera.position);

      // Keep active target in front of camera
      const gazeDir = new THREE.Vector3(0, 0, -1).applyQuaternion(camera.quaternion);
      activeTarget.current.copy(camera.position).addScaledVector(gazeDir, 15);
    } else if (cameraMode === 'cinematic') {
      const time = state.clock.getElapsedTime();
      const x = Math.sin(time * 0.2) * 50;
      const z = Math.cos(time * 0.2) * 50;
      const y = 15 + Math.sin(time * 0.5) * 5;
      camera.position.set(x, y, z);
      camera.lookAt(0, 0, 0);
      activeTarget.current.set(0, 0, 0);
    } else if (cameraMode === 'map') {
      // WASD pans map target on XZ plane
      const panSpeed = keysPressed.current['shift'] ? 2.5 : 1.0;
      if (keysPressed.current['w']) tdTarget.current.z -= panSpeed;
      if (keysPressed.current['s']) tdTarget.current.z += panSpeed;
      if (keysPressed.current['a']) tdTarget.current.x -= panSpeed;
      if (keysPressed.current['d']) tdTarget.current.x += panSpeed;

      // Bound checks relative to map boundaries
      const halfWidth = gridWidth / 2;
      const halfHeight = gridHeight / 2;
      tdTarget.current.x = Math.max(-halfWidth, Math.min(halfWidth, tdTarget.current.x));
      tdTarget.current.z = Math.max(-halfHeight, Math.min(halfHeight, tdTarget.current.z));

      // Align Y coordinate to terrain height
      tdTarget.current.y = getTerrainHeight(tdTarget.current.x, tdTarget.current.z);

      // Smooth tracking camera
      camera.position.x += (tdTarget.current.x - camera.position.x) * 0.08;
      camera.position.y += (tdZoom.current - camera.position.y) * 0.08;
      camera.position.z += (tdTarget.current.z - camera.position.z) * 0.08;
      camera.lookAt(tdTarget.current);

      activeTarget.current.copy(tdTarget.current);
    }

    // Update Web Audio Listener
    const forwardVec = new THREE.Vector3(0, 0, -1).applyQuaternion(camera.quaternion);
    const upVec = new THREE.Vector3(0, 1, 0).applyQuaternion(camera.quaternion);
    audioManager.updateListener(
      camera.position.x, camera.position.y, camera.position.z,
      forwardVec.x, forwardVec.y, forwardVec.z,
      upVec.x, upVec.y, upVec.z
    );
  });

  return (
    <>
      {cameraMode === 'orbit' && (
        <orbitControls ref={orbitControlsRef} args={[camera, gl.domElement]} enableDamping />
      )}
      <object3D name="camera-controls" userData={{ cameraMode }} />
    </>
  );
};

export default CameraControls;
