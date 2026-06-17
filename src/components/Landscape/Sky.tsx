import React, { useRef, useMemo } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { getSkyParams } from './utils/skyParams';

interface SkyProps {
  speed?: number;
  timeOfDay?: number; // 0 to 24
}


interface CloudDescriptor {
  id: number;
  position: [number, number, number];
  scale: [number, number, number];
  speed: number;
}

export const Sky: React.FC<SkyProps> = ({ speed = 1.0, timeOfDay = 12 }) => {
  const skyRef = useRef<THREE.Mesh>(null);
  const cloudsGroupRef = useRef<THREE.Group>(null);

  // Orbit calculations
  const angle = ((timeOfDay - 6) / 24) * Math.PI * 2;
  const R = 300;
  const sunX = Math.cos(angle) * R;
  const sunY = Math.sin(angle) * R;
  const sunZ = Math.sin(angle) * 0.1 * R; // Slightly tilted orbit
  const sunPosition = [sunX, sunY, sunZ] as [number, number, number];

  const moonAngle = angle + Math.PI;
  const moonX = Math.cos(moonAngle) * R;
  const moonY = Math.sin(moonAngle) * R;
  const moonZ = Math.sin(moonAngle) * 0.1 * R;
  const moonPosition = [moonX, moonY, moonZ] as [number, number, number];

  const showMoon = moonY > 0;

  // Sky parameters
  const params = getSkyParams(timeOfDay);
  const showStars = params.starOpacity > 0;

  // Generate starfield vertices once
  const starPositions = useMemo(() => {
    const count = 500;
    const positions = new Float32Array(count * 3);
    for (let i = 0; i < count; i++) {
      // Uniform distribution on a hemisphere
      const u = Math.random();
      const v = Math.random();
      const theta = u * 2.0 * Math.PI;
      const phi = Math.acos(2.0 * v - 1.0);
      const r = 450;
      positions[i * 3] = r * Math.sin(phi) * Math.cos(theta);
      positions[i * 3 + 1] = Math.abs(r * Math.sin(phi) * Math.sin(theta)); // Keep above horizon
      positions[i * 3 + 2] = r * Math.cos(phi);
    }
    return positions;
  }, []);

  // Cloud descriptions
  const initialClouds = useMemo<CloudDescriptor[]>(() => {
    return [
      { id: 1, position: [-150, 60, -50], scale: [25, 6, 15], speed: 1.5 },
      { id: 2, position: [-80, 75, -120], scale: [35, 8, 20], speed: 1.0 },
      { id: 3, position: [20, 65, 80], scale: [20, 5, 12], speed: 2.0 },
      { id: 4, position: [90, 80, -20], scale: [40, 10, 25], speed: 0.8 },
      { id: 5, position: [-200, 70, 40], scale: [30, 7, 18], speed: 1.2 },
      { id: 6, position: [160, 85, -80], scale: [28, 6, 16], speed: 1.6 },
    ];
  }, []);

  useFrame((state, delta) => {
    const safeDelta = Math.min(delta, 0.1);
    
    // Rotate the sky dome slowly
    if (skyRef.current && skyRef.current.rotation) {
      skyRef.current.rotation.y = state.clock.getElapsedTime() * 0.005 * speed;
    }

    // Drift clouds
    if (cloudsGroupRef.current && cloudsGroupRef.current.children) {
      const children = cloudsGroupRef.current.children;
      for (let i = 0; i < children.length; i++) {
        const child = children[i] as any;
        if (child && child.position) {
          const cloudSpeed = initialClouds[i]?.speed || 1.0;
          child.position.x += safeDelta * cloudSpeed * 3.0 * speed;
          if (child.position.x > 250) {
            child.position.x = -250;
          }
        }
      }
    }
  });

  return (
    <group
      name="sky-group"
      userData={{ timeOfDay, lightIntensity: params.sunIntensity, skyColor: params.skyColor, speed }}
      data-speed={speed}
      data-time-of-day={timeOfDay}
      data-light-intensity={params.sunIntensity}
      data-sky-color={params.skyColor}
    >
      {/* Sky dome */}
      <mesh ref={skyRef} name="sky-mesh">
        <sphereGeometry args={[500, 32, 32]} />
        <meshBasicMaterial color={params.skyColor} side={THREE.BackSide} />
      </mesh>

      {/* Main directional sun light */}
      <directionalLight
        name="sky-light"
        position={sunPosition}
        color={params.sunColor}
        intensity={params.sunIntensity}
        castShadow
        shadow-mapSize-width={2048}
        shadow-mapSize-height={2048}
        shadow-camera-far={1000}
        shadow-camera-left={-150}
        shadow-camera-right={150}
        shadow-camera-top={150}
        shadow-camera-bottom={-150}
      />

      {/* Moonlight (opposite to sun, active when moon is up) */}
      {showMoon && (
        <directionalLight
          name="moon-light"
          position={moonPosition}
          color="#e0f2fe"
          intensity={0.2 * (moonY / R)}
          castShadow
          shadow-mapSize-width={1024}
          shadow-mapSize-height={1024}
        />
      )}

      {/* Hemisphere and Ambient lights */}
      <hemisphereLight
        name="hemi-light"
        color={params.hemiSkyColor}
        groundColor={params.hemiGroundColor}
        intensity={params.hemiIntensity}
      />
      <ambientLight name="ambient-light" color={params.ambientColor} intensity={params.ambientIntensity} />

      {/* Moon geometry */}
      {showMoon && (
        <mesh name="moon-mesh" position={moonPosition}>
          <sphereGeometry args={[8, 16, 16]} />
          <meshBasicMaterial color="#ffffd0" />
        </mesh>
      )}

      {/* Stars particle system */}
      {showStars && (
        <points name="stars-particles">
          <bufferGeometry>
            <bufferAttribute
              attach="attributes-position"
              count={starPositions.length / 3}
              array={starPositions}
              itemSize={3}
            />
          </bufferGeometry>
          <pointsMaterial
            size={1.5}
            color="#ffffff"
            transparent
            opacity={params.starOpacity}
            sizeAttenuation={true}
          />
        </points>
      )}

      {/* Drifting clouds */}
      <group ref={cloudsGroupRef} name="clouds-group">
        {initialClouds.map((cloud) => (
          <group key={cloud.id} position={cloud.position}>
            <mesh name="cloud-mesh">
              <boxGeometry args={cloud.scale} />
              <meshStandardMaterial color="#ffffff" transparent opacity={0.65} roughness={0.9} />
            </mesh>
            <mesh position={[-cloud.scale[0] * 0.35, -cloud.scale[1] * 0.15, 0]}>
              <boxGeometry args={[cloud.scale[0] * 0.55, cloud.scale[1] * 0.75, cloud.scale[2] * 0.75]} />
              <meshStandardMaterial color="#ffffff" transparent opacity={0.55} roughness={0.9} />
            </mesh>
            <mesh position={[cloud.scale[0] * 0.35, -cloud.scale[1] * 0.15, 0]}>
              <boxGeometry args={[cloud.scale[0] * 0.55, cloud.scale[1] * 0.75, cloud.scale[2] * 0.75]} />
              <meshStandardMaterial color="#ffffff" transparent opacity={0.55} roughness={0.9} />
            </mesh>
          </group>
        ))}
      </group>
    </group>
  );
};

export default Sky;
