import React, { useEffect, useLayoutEffect, useMemo, useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import type { World } from './utils/worldGen';

// ---------------------------------------------------------------------------------------
// WorldWaterfalls — foam curtains hung over the steep river drops the generator detected
// (World.waterfall*). Two instanced meshes total: the curtains and the splash pools at
// their feet, so even hundreds of falls cost two draw calls. The curtain shader scrolls
// bright streaks downward so the water visibly FALLS instead of hanging as a static sheet.
// ---------------------------------------------------------------------------------------

export interface WorldWaterfallsProps {
  world: World;
  renderSize?: number;
  heightRatio?: number;
}

// ShaderMaterial on an InstancedMesh: three auto-defines USE_INSTANCING and declares the
// instanceMatrix attribute for us — we only have to APPLY it in the vertex transform.
const CURTAIN_VERTEX = /* glsl */ `
  varying vec2 vUv;
  void main() {
    vUv = uv;
    gl_Position = projectionMatrix * modelViewMatrix * instanceMatrix * vec4(position, 1.0);
  }
`;
const CURTAIN_FRAGMENT = /* glsl */ `
  precision highp float;
  varying vec2 vUv;
  uniform float uTime;
  float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123); }
  void main() {
    // Streaks race down the curtain (two speeds for parallax), broken up per-column.
    float col = floor(vUv.x * 7.0);
    float s1 = fract(vUv.y * 3.0 + uTime * 1.5 + hash(vec2(col, 1.0)));
    float s2 = fract(vUv.y * 6.0 + uTime * 2.6 + hash(vec2(col, 7.0)) * 3.0);
    float streak = smoothstep(0.55, 1.0, s1) * 0.5 + smoothstep(0.65, 1.0, s2) * 0.5;
    // Paler + more transparent at the lip, frothy at the base.
    float body = mix(0.72, 1.0, streak);
    vec3 color = mix(vec3(0.62, 0.78, 0.88), vec3(1.0), body * (0.55 + 0.45 * (1.0 - vUv.y)));
    // Soft side edges so the rectangle reads as a spray curtain, not a billboard.
    float side = smoothstep(0.0, 0.18, vUv.x) * smoothstep(1.0, 0.82, vUv.x);
    float alpha = (0.5 + 0.42 * streak) * side;
    gl_FragColor = vec4(color, alpha);
  }
`;

export const WorldWaterfalls: React.FC<WorldWaterfallsProps> = ({
  world,
  renderSize = 400,
  heightRatio = 0.13,
}) => {
  const curtainRef = useRef<THREE.InstancedMesh>(null);
  const foamRef = useRef<THREE.InstancedMesh>(null);

  const count = world.waterfallCount ?? 0;
  const heightUnits = renderSize * heightRatio;

  const curtainGeom = useMemo(() => new THREE.PlaneGeometry(1, 1, 1, 1), []);
  const foamGeom = useMemo(() => new THREE.CircleGeometry(1, 10).rotateX(-Math.PI / 2), []);
  const curtainMat = useMemo(
    () =>
      new THREE.ShaderMaterial({
        vertexShader: CURTAIN_VERTEX,
        fragmentShader: CURTAIN_FRAGMENT,
        uniforms: { uTime: { value: 0 } },
        transparent: true,
        depthWrite: false,
        side: THREE.DoubleSide,
        fog: false,
      }),
    [],
  );

  useFrame((state) => {
    curtainMat.uniforms.uTime.value = state.clock.getElapsedTime();
  });

  useLayoutEffect(() => {
    const curtain = curtainRef.current;
    const foam = foamRef.current;
    if (!curtain || !foam || count === 0) return;
    const dummy = new THREE.Object3D();
    const { size } = world;
    const toWorld = renderSize / size;
    for (let i = 0; i < count; i++) {
      const x = world.waterfallX[i] * toWorld;
      const z = world.waterfallZ[i] * toWorld;
      const topY = world.waterfallTopE[i] * heightUnits;
      const dropY = Math.max(1.2, world.waterfallDrop[i] * heightUnits);
      const yaw = world.waterfallYaw[i];
      const dirX = Math.cos(yaw);
      const dirZ = Math.sin(yaw);
      const width = Math.min(4, 1.2 + dropY * 0.3);

      // Curtain: hangs from the lip, pushed proud of the rock face so the top edge doesn't
      // knife back into the slope.
      dummy.position.set(x + dirX * 1.0, topY - dropY / 2, z + dirZ * 1.0);
      dummy.rotation.set(0, Math.PI / 2 - yaw, 0);
      dummy.scale.set(width, dropY + 0.6, 1);
      dummy.updateMatrix();
      if (typeof curtain.setMatrixAt === 'function') curtain.setMatrixAt(i, dummy.matrix);

      // Splash pool at the base — small and lifted so it never Z-fights the riverbed.
      dummy.position.set(x + dirX * (1.0 + width * 0.2), topY - dropY + 0.3, z + dirZ * (1.0 + width * 0.2));
      dummy.rotation.set(0, 0, 0);
      dummy.scale.set(width * 0.55, 1, width * 0.4);
      dummy.updateMatrix();
      if (typeof foam.setMatrixAt === 'function') foam.setMatrixAt(i, dummy.matrix);
    }
    if (curtain.instanceMatrix) curtain.instanceMatrix.needsUpdate = true;
    if (foam.instanceMatrix) foam.instanceMatrix.needsUpdate = true;
  }, [world, count, renderSize, heightUnits]);

  useEffect(() => {
    return () => {
      curtainGeom.dispose();
      foamGeom.dispose();
      curtainMat.dispose();
    };
  }, [curtainGeom, foamGeom, curtainMat]);

  if (count === 0) return null;
  return (
    <group name="world-waterfalls">
      <instancedMesh
        ref={curtainRef}
        args={[curtainGeom, undefined as any, count]}
        material={curtainMat}
        name="waterfall-curtains"
      />
      <instancedMesh ref={foamRef} args={[foamGeom, undefined as any, count]} name="waterfall-foam">
        <meshBasicMaterial color="#ffffff" transparent opacity={0.38} depthWrite={false} />
      </instancedMesh>
    </group>
  );
};

export default WorldWaterfalls;
