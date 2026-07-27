import React, { useRef, useMemo } from 'react';
import { useFrame, useThree } from '@react-three/fiber';
import * as THREE from 'three';
import { getSkyParams } from './utils/skyParams';
import { makeSceneRandom, sceneDelta } from './utils/sceneClock';

// Gradient sky dome: zenith deepens, horizon pales into haze, and a soft halo wraps the sun.
// A flat single-colour dome is the single biggest "this is a tech demo" tell — the gradient
// alone makes every frame read as atmosphere.
const SKY_VERTEX = /* glsl */ `
  varying vec3 vDir;
  void main() {
    vDir = position;
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
  }
`;
const SKY_FRAGMENT = /* glsl */ `
  precision highp float;
  varying vec3 vDir;
  uniform vec3 uZenith;
  uniform vec3 uHorizon;
  uniform vec3 uSunDir;
  uniform vec3 uSunColor;
  uniform float uGlow;
  uniform vec3 uMoonDir;
  uniform float uMoonGlow;
  void main() {
    vec3 d = normalize(vDir);
    float h = clamp(d.y, 0.0, 1.0);
    vec3 col = mix(uHorizon, uZenith, pow(h, 0.62));
    float s = max(dot(d, normalize(uSunDir)), 0.0);
    col += uSunColor * (pow(s, 300.0) * 0.9 + pow(s, 10.0) * 0.16 * uGlow);
    // Cold, tighter halo around the moon at night.
    float m = max(dot(d, normalize(uMoonDir)), 0.0);
    col += vec3(0.58, 0.66, 0.8) * (pow(m, 700.0) * 0.55 + pow(m, 26.0) * 0.05) * uMoonGlow;
    gl_FragColor = vec4(col, 1.0);
  }
`;

// ---------------------------------------------------------------------------------------
// WorldSky — day/night sky + lighting for the huge world (WorldShowcase).
//
// Same keyframe model as the legacy Sky.tsx (driven by `getSkyParams(timeOfDay)`), but every
// distance is scaled up for the big world: the terrain spans ~RENDER_SIZE (400) units and the
// camera orbits out to ~1200, so the dome / sun-orbit / stars all sit well beyond that.
//
// Returns the current sun direction via `onSun` so the water shader's specular can track it.
// ---------------------------------------------------------------------------------------

export interface WorldSkyProps {
  timeOfDay?: number; // 0..24
  /** Spin speed of the slow dome rotation + cloud drift. */
  speed?: number;
  /** Overall scale; dome/orbit/star radii derive from this (≈ camera far budget). */
  worldScale?: number;
}

interface CloudDescriptor {
  id: number;
  position: [number, number, number];
  scale: [number, number, number];
  speed: number;
}

export const WorldSky: React.FC<WorldSkyProps> = ({
  timeOfDay = 12,
  speed = 1.0,
  worldScale = 400,
}) => {
  const { scene } = useThree();
  const cloudsRef = useRef<THREE.Group>(null);
  const tmpColor = useMemo(() => new THREE.Color(), []);
  const HAZE = useMemo(() => new THREE.Color('#eef6fb'), []);
  const skyUniforms = useMemo(
    () => ({
      uZenith: { value: new THREE.Color('#3f7fd0') },
      uHorizon: { value: new THREE.Color('#cfe6f2') },
      uSunDir: { value: new THREE.Vector3(0, 1, 0) },
      uSunColor: { value: new THREE.Color('#fff2cc') },
      uGlow: { value: 1 },
      uMoonDir: { value: new THREE.Vector3(0, -1, 0) },
      uMoonGlow: { value: 0 },
    }),
    [],
  );

  const DOME_R = worldScale * 6.5; // 2600 @400 — beyond camera maxDistance (1200)
  const ORBIT_R = worldScale * 4.5; // 1800
  const STAR_R = worldScale * 6.0; // 2400
  const CLOUD_Y = worldScale * 0.8; // 320
  const CLOUD_SPREAD = worldScale * 2.4; // 960

  // Sun / moon orbit (same tilt convention as legacy Sky).
  const angle = ((timeOfDay - 6) / 24) * Math.PI * 2;
  const sunDirUnit = useMemo(
    () => new THREE.Vector3(Math.cos(angle), Math.sin(angle), Math.sin(angle) * 0.1).normalize(),
    [angle],
  );
  const sunPosition: [number, number, number] = [
    sunDirUnit.x * ORBIT_R,
    sunDirUnit.y * ORBIT_R,
    sunDirUnit.z * ORBIT_R,
  ];
  const moonAngle = angle + Math.PI;
  const moonDirY = Math.sin(moonAngle);
  const moonPosition: [number, number, number] = [
    Math.cos(moonAngle) * ORBIT_R,
    moonDirY * ORBIT_R,
    Math.sin(moonAngle) * 0.1 * ORBIT_R,
  ];
  const showMoon = moonDirY > 0;

  const params = getSkyParams(timeOfDay);
  const showStars = params.starOpacity > 0;

  const starPositions = useMemo(() => {
    const count = 700;
    const positions = new Float32Array(count * 3);
    // Seeded under capture, `Math.random` otherwise. 700 stars scattered afresh on every page load
    // are 700 pixels that differ between two shots of the same view — invisible to a reviewer and
    // fatal to a byte-comparison reproducibility gate. See `sceneClock.ts`.
    const rand = makeSceneRandom('sky.stars');
    for (let i = 0; i < count; i++) {
      const u = rand();
      const v = rand();
      const theta = u * 2.0 * Math.PI;
      const phi = Math.acos(2.0 * v - 1.0);
      positions[i * 3] = STAR_R * Math.sin(phi) * Math.cos(theta);
      positions[i * 3 + 1] = Math.abs(STAR_R * Math.sin(phi) * Math.sin(theta)); // keep above horizon
      positions[i * 3 + 2] = STAR_R * Math.cos(phi);
    }
    return positions;
  }, [STAR_R]);

  const clouds = useMemo<CloudDescriptor[]>(() => {
    const s = worldScale / 400;
    const base: CloudDescriptor[] = [
      { id: 1, position: [-150, 60, -50], scale: [25, 6, 15], speed: 1.5 },
      { id: 2, position: [-80, 75, -120], scale: [35, 8, 20], speed: 1.0 },
      { id: 3, position: [20, 65, 80], scale: [20, 5, 12], speed: 2.0 },
      { id: 4, position: [90, 80, -20], scale: [40, 10, 25], speed: 0.8 },
      { id: 5, position: [-200, 70, 40], scale: [30, 7, 18], speed: 1.2 },
      { id: 6, position: [160, 85, -80], scale: [28, 6, 16], speed: 1.6 },
      { id: 7, position: [-300, 90, 180], scale: [44, 9, 26], speed: 1.1 },
      { id: 8, position: [260, 78, 140], scale: [32, 7, 20], speed: 1.4 },
      { id: 9, position: [40, 68, 200], scale: [26, 6, 15], speed: 1.3 },
      { id: 10, position: [-120, 82, 260], scale: [38, 9, 22], speed: 0.9 },
      { id: 11, position: [220, 72, 60], scale: [22, 5, 13], speed: 1.7 },
      { id: 12, position: [-260, 66, -180], scale: [30, 7, 17], speed: 1.2 },
      { id: 13, position: [120, 88, -220], scale: [46, 10, 26], speed: 0.7 },
      { id: 14, position: [-40, 76, -280], scale: [24, 6, 14], speed: 1.5 },
    ];
    // Modest cumulus puffs high above the terrain — NOT map-sized slabs: at worldScale 1000
    // a x4 multiplier made 400-unit blobs that smeared into streaks across the whole sky.
    return base.map((c) => ({
      ...c,
      position: [c.position[0] * 4.5 * s, CLOUD_Y * 1.35 + c.position[1] * s, c.position[2] * 4.5 * s],
      scale: [c.scale[0] * 1.8 * s, c.scale[1] * 1.4 * s, c.scale[2] * 1.8 * s],
    }));
  }, [worldScale, CLOUD_Y]);

  useFrame((_state, rawDelta) => {
    // Fixed under capture. Cloud drift below integrates this, and `speed = 0` happens to zero the
    // product today — a coincidence of the canonical defaults, not a property worth relying on.
    const delta = sceneDelta(rawDelta);
    const safeDelta = Math.min(delta, 0.1);
    // Background follows the sky colour so night actually goes dark beyond the dome.
    scene.background = new THREE.Color(params.skyColor);

    // Drive the gradient + halo from the day/night palette. The horizon pales toward haze
    // by day but stays dark at night (scaled by the sky's own luminance).
    tmpColor.set(params.skyColor);
    const lum = (tmpColor.r + tmpColor.g + tmpColor.b) / 3;
    skyUniforms.uZenith.value.copy(tmpColor).multiplyScalar(0.62);
    skyUniforms.uHorizon.value.copy(tmpColor).lerp(HAZE, 0.15 + 0.4 * Math.min(1, lum * 2.2));
    skyUniforms.uSunDir.value.set(sunPosition[0], sunPosition[1], sunPosition[2]).normalize();
    skyUniforms.uSunColor.value.set(params.sunColor);
    skyUniforms.uGlow.value = params.sunIntensity;
    skyUniforms.uMoonDir.value.set(moonPosition[0], moonPosition[1], moonPosition[2]).normalize();
    skyUniforms.uMoonGlow.value = showMoon ? moonDirY : 0;

    if (cloudsRef.current) {
      const children = cloudsRef.current.children;
      for (let i = 0; i < children.length; i++) {
        const child = children[i] as THREE.Object3D;
        child.position.x += safeDelta * (clouds[i]?.speed ?? 1) * 8.0 * speed;
        if (child.position.x > CLOUD_SPREAD) child.position.x = -CLOUD_SPREAD;
      }
    }
  });

  return (
    <group name="world-sky-group">
      {/* Sky dome — gradient shader (zenith -> horizon haze) with a soft sun halo. */}
      <mesh name="world-sky-dome">
        <sphereGeometry args={[DOME_R, 32, 24]} />
        <shaderMaterial
          vertexShader={SKY_VERTEX}
          fragmentShader={SKY_FRAGMENT}
          uniforms={skyUniforms}
          side={THREE.BackSide}
          fog={false}
        />
      </mesh>

      <directionalLight
        name="world-sun-light"
        position={sunPosition}
        color={params.sunColor}
        intensity={params.sunIntensity * 0.8}
        castShadow
        shadow-mapSize-width={2048}
        shadow-mapSize-height={2048}
        shadow-camera-far={ORBIT_R * 2}
        shadow-camera-left={-worldScale * 0.7}
        shadow-camera-right={worldScale * 0.7}
        shadow-camera-top={worldScale * 0.7}
        shadow-camera-bottom={-worldScale * 0.7}
      />

      {showMoon && (
        <directionalLight
          name="world-moon-light"
          position={moonPosition}
          color="#dbeafe"
          intensity={0.25 * moonDirY}
        />
      )}

      {/* Ambient + hemisphere are kept low so the summed irradiance stays near 1.0 — otherwise
          the light biome colours (sand, rock, snow) blow out to white and stop matching the
          minimap. The sun provides most of the directional shading. */}
      <hemisphereLight
        name="world-hemi-light"
        color={params.hemiSkyColor}
        groundColor={params.hemiGroundColor}
        intensity={params.hemiIntensity * 0.35}
      />
      <ambientLight name="world-ambient-light" color={params.ambientColor} intensity={params.ambientIntensity * 0.4} />

      {showMoon && (
        <mesh name="world-moon-mesh" position={moonPosition}>
          <sphereGeometry args={[worldScale * 0.09, 16, 16]} />
          <meshBasicMaterial color="#fffdf0" fog={false} />
        </mesh>
      )}

      {/* Sun disc */}
      {params.sunIntensity > 0.05 && (
        <mesh name="world-sun-mesh" position={sunPosition}>
          <sphereGeometry args={[worldScale * 0.11, 16, 16]} />
          <meshBasicMaterial color={params.sunColor} fog={false} />
        </mesh>
      )}

      {showStars && (
        <points name="world-stars">
          <bufferGeometry>
            <bufferAttribute
              attach="attributes-position"
              count={starPositions.length / 3}
              array={starPositions}
              itemSize={3}
            />
          </bufferGeometry>
          <pointsMaterial size={worldScale * 0.006} color="#ffffff" transparent opacity={params.starOpacity} sizeAttenuation fog={false} />
        </points>
      )}

      {/* Puffy low-poly clouds: overlapping flat-shaded ellipsoids read as soft cumulus from
          any angle (the old axis-aligned boxes looked like slabs at the horizon). */}
      <group ref={cloudsRef} name="world-clouds-group">
        {clouds.map((cloud) => (
          <group key={cloud.id} position={cloud.position}>
            {/* The core puff casts a real shadow — drifting cloud shadows sweep the land. */}
            <mesh castShadow scale={[cloud.scale[0] * 0.55, cloud.scale[1] * 0.8, cloud.scale[2] * 0.6]}>
              <sphereGeometry args={[1, 7, 6]} />
              <meshStandardMaterial color="#ffffff" transparent opacity={0.5} roughness={1} fog={false} flatShading />
            </mesh>
            <mesh
              position={[-cloud.scale[0] * 0.38, -cloud.scale[1] * 0.15, cloud.scale[2] * 0.12]}
              scale={[cloud.scale[0] * 0.38, cloud.scale[1] * 0.55, cloud.scale[2] * 0.42]}
            >
              <sphereGeometry args={[1, 7, 6]} />
              <meshStandardMaterial color="#f4f8ff" transparent opacity={0.42} roughness={1} fog={false} flatShading />
            </mesh>
            <mesh
              position={[cloud.scale[0] * 0.36, -cloud.scale[1] * 0.1, -cloud.scale[2] * 0.1]}
              scale={[cloud.scale[0] * 0.35, cloud.scale[1] * 0.5, cloud.scale[2] * 0.4]}
            >
              <sphereGeometry args={[1, 7, 6]} />
              <meshStandardMaterial color="#f8fbff" transparent opacity={0.46} roughness={1} fog={false} flatShading />
            </mesh>
          </group>
        ))}
      </group>
    </group>
  );
};

export default WorldSky;
