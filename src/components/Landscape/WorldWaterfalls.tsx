import React, { useEffect, useLayoutEffect, useMemo, useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { driveTimeUniform } from './utils/sceneClock';

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
  varying vec3 vWorldPos;
  void main() {
    vUv = uv;
    vec4 world = modelMatrix * instanceMatrix * vec4(position, 1.0);
    vWorldPos = world.xyz;
    gl_Position = projectionMatrix * viewMatrix * world;
  }
`;
const CURTAIN_FRAGMENT = /* glsl */ `
  precision highp float;
  varying vec2 vUv;
  varying vec3 vWorldPos;
  uniform float uTime;
  uniform float uTerrainSize;
  uniform vec3 uFogColor;
  float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123); }
  float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
               mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
  }
  void main() {
    // Soft continuous spray sheet (no quantized column stripes) + smooth distance fade before overview.
    float f1 = noise(vec2(vUv.x * 4.5, vUv.y * 2.5 + uTime * 1.4));
    float f2 = noise(vec2(vUv.x * 9.0 + 3.1, vUv.y * 5.0 + uTime * 2.2));
    float mist = smoothstep(0.28, 0.85, f1 * 0.62 + f2 * 0.38);
    float side = smoothstep(0.0, 0.25, vUv.x) * smoothstep(1.0, 0.75, vUv.x);
    float vert = smoothstep(0.0, 0.16, vUv.y) * smoothstep(1.0, 0.86, vUv.y);
    float dist = distance(cameraPosition, vWorldPos);
    float distFade = 1.0 - smoothstep(uTerrainSize * 0.14, uTerrainSize * 0.40, dist);
    vec3 col = mix(vec3(0.68, 0.83, 0.91), vec3(0.95, 0.98, 1.0), mist * (0.4 + 0.6 * (1.0 - vUv.y)));
    col = mix(col, uFogColor, smoothstep(uTerrainSize * 0.5, uTerrainSize * 2.2, dist));
    float alpha = (0.18 + 0.30 * mist) * side * vert * distFade;
    gl_FragColor = vec4(col, alpha);
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
        uniforms: {
          uTime: { value: 0 },
          uTerrainSize: { value: renderSize },
          uFogColor: { value: new THREE.Color('#c4dfef') },
        },
        transparent: true,
        depthWrite: false,
        side: THREE.DoubleSide,
        fog: false,
      }),
    [renderSize],
  );

  const foamMat = useMemo(
    () =>
      new THREE.MeshBasicMaterial({
        color: '#ffffff',
        transparent: true,
        opacity: 0.24,
        depthWrite: false,
      }),
    [],
  );

  useFrame((state) => {
    driveTimeUniform(curtainMat, state.clock);
    const fog = state.scene.fog as THREE.Fog | null;
    if (fog && fog.color) {
      (curtainMat.uniforms.uFogColor.value as THREE.Color).copy(fog.color);
    }
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
      foamMat.dispose();
    };
  }, [curtainGeom, foamGeom, curtainMat, foamMat]);

  if (count === 0) return null;
  // Both materials go through `args`, not a `material=` prop or a JSX child. `InstancedMesh`'s
  // constructor takes (geometry, material, count) with none of them optional, so the old cast on an
  // `undefined` material was not a loose type on a value that could be absent — it was a lie about
  // a slot three requires. Passing the real material there is shorter *and* honest, and it gave the
  // foam a material this component owns and therefore disposes.
  return (
    <group name="world-waterfalls">
      <instancedMesh
        ref={curtainRef}
        args={[curtainGeom, curtainMat, count]}
        name="waterfall-curtains"
      />
      <instancedMesh ref={foamRef} args={[foamGeom, foamMat, count]} name="waterfall-foam" />
    </group>
  );
};

export default WorldWaterfalls;
