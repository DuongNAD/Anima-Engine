import React, { useEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import {
  createMap2SkyMaterial,
  createMap2StarfieldMaterial,
  Map2SkyShaderUniforms,
  Map2StarfieldShaderUniforms,
} from './utils/map2Shaders';
import {
  CelestialState,
  computeCelestialState,
  buildMap2StarfieldGeometry,
} from './utils/map2AtmosphereMath';

export type { CelestialState };

// -------------------------------------------------------------------------------------------------
// React Three Fiber Component: Map2Atmosphere
// -------------------------------------------------------------------------------------------------

export interface Map2AtmosphereProps {
  timeOfDay?: number; // 0.0 to 24.0 hours (default: 11.5)
  timeSpeed?: number; // hour progression rate per second (default: 0.0 = static / manual)
  enableShadows?: boolean;
  shadowMapSize?: number;
  skyRadius?: number;
  starCount?: number;
  windSpeed?: number;
  windDirection?: [number, number];
  onCelestialUpdate?: (state: CelestialState) => void;
}

/**
 * Dynamic Atmospheric Sky Dome, 24-Hour Celestial Cycle, Lighting & Distance Fog for Map 2.
 */
export const Map2Atmosphere: React.FC<Map2AtmosphereProps> = ({
  timeOfDay = 11.5,
  timeSpeed = 0.0,
  enableShadows = true,
  shadowMapSize = 2048,
  skyRadius = 1200,
  starCount = 2000,
  windSpeed = 2.5,
  windDirection = [1.0, 0.5],
  onCelestialUpdate,
}) => {
  const currentTimeRef = useRef<number>(timeOfDay);
  const skyMeshRef = useRef<THREE.Mesh>(null);
  const starPointsRef = useRef<THREE.Points>(null);
  const sunLightRef = useRef<THREE.DirectionalLight>(null);
  const moonLightRef = useRef<THREE.DirectionalLight>(null);
  const ambientLightRef = useRef<THREE.AmbientLight>(null);
  const hemiLightRef = useRef<THREE.HemisphereLight>(null);
  const fogRef = useRef<THREE.FogExp2>(null);

  // Sync prop changes to ref
  useEffect(() => {
    currentTimeRef.current = timeOfDay;
  }, [timeOfDay]);

  // 1. Create Physical Sky Dome Geometry & Shader Material
  const skyGeometry = useMemo(() => {
    return new THREE.SphereGeometry(skyRadius, 32, 32);
  }, [skyRadius]);

  const skyMaterial = useMemo(() => {
    return createMap2SkyMaterial();
  }, []);

  // 2. Create Nocturnal Procedural Starfield
  const starGeometry = useMemo(() => {
    return buildMap2StarfieldGeometry(starCount, skyRadius * 0.95);
  }, [starCount, skyRadius]);

  const starMaterial = useMemo(() => {
    return createMap2StarfieldMaterial();
  }, []);

  // 3. Initial Celestial State
  const initialCelestial = useMemo(() => {
    return computeCelestialState(timeOfDay, windSpeed, windDirection);
  }, [timeOfDay, windSpeed, windDirection]);

  // 4. Render Frame Animation Loop
  useFrame(({ clock }, delta) => {
    if (timeSpeed > 0) {
      currentTimeRef.current = (currentTimeRef.current + delta * timeSpeed) % 24.0;
    }

    const state = computeCelestialState(currentTimeRef.current, windSpeed, windDirection);
    const elapsedTime = clock.getElapsedTime();

    // Update Sky Dome Uniforms
    if (skyMeshRef.current && skyMeshRef.current.material) {
      const mat = skyMeshRef.current.material as THREE.ShaderMaterial;
      const u = mat.uniforms as unknown as Map2SkyShaderUniforms;
      if (u) {
        u.uTime.value = elapsedTime;
        u.uSunDir.value.copy(state.sunDirection);
        u.uSunColor.value.copy(state.sunColor);
        u.uMoonDir.value.copy(state.moonDirection);
        u.uMoonColor.value.copy(state.moonColor);
        u.uZenithColor.value.copy(state.skyZenithColor);
        u.uHorizonColor.value.copy(state.skyHorizonColor);
        u.uGroundColor.value.copy(state.skyGroundColor);
        u.uSunIntensity.value = state.sunIntensity;
        u.uMoonIntensity.value = state.moonIntensity;
      }
    }

    // Update Starfield Uniforms
    if (starPointsRef.current && starPointsRef.current.material) {
      const mat = starPointsRef.current.material as THREE.ShaderMaterial;
      const u = mat.uniforms as unknown as Map2StarfieldShaderUniforms;
      if (u) {
        u.uTime.value = elapsedTime;
        u.uOpacity.value = state.starOpacity;
      }
    }

    // Update Directional Sun Light
    if (sunLightRef.current) {
      sunLightRef.current.position.copy(state.sunPosition);
      sunLightRef.current.color.copy(state.sunColor);
      sunLightRef.current.intensity = Math.max(0.01, state.sunIntensity);
    }

    // Update Directional Moon Light
    if (moonLightRef.current) {
      moonLightRef.current.position.copy(state.moonPosition);
      moonLightRef.current.color.copy(state.moonColor);
      moonLightRef.current.intensity = state.moonIntensity;
    }

    // Update Ambient & Hemisphere Lights
    if (ambientLightRef.current) {
      ambientLightRef.current.color.copy(state.ambientColor);
      ambientLightRef.current.intensity = state.ambientIntensity;
    }

    if (hemiLightRef.current) {
      hemiLightRef.current.color.copy(state.hemiSkyColor);
      hemiLightRef.current.groundColor.copy(state.hemiGroundColor);
      hemiLightRef.current.intensity = state.hemiIntensity;
    }

    // Synchronize Scene Fog via Ref
    if (fogRef.current) {
      fogRef.current.color.copy(state.fogColor);
      fogRef.current.density = state.fogDensity;
    }

    if (onCelestialUpdate) {
      onCelestialUpdate(state);
    }
  });

  // 5. Explicit Resource Cleanup on Unmount
  useEffect(() => {
    return () => {
      skyGeometry.dispose();
      skyMaterial.dispose();
      starGeometry.dispose();
      starMaterial.dispose();
    };
  }, [skyGeometry, skyMaterial, starGeometry, starMaterial]);

  return (
    <group name="map2-atmosphere-system">
      {/* Attached Exponential Distance Fog */}
      <fogExp2
        ref={fogRef}
        attach="fog"
        args={[initialCelestial.fogColor, initialCelestial.fogDensity]}
      />

      {/* Physical Rayleigh & Mie Sky Dome */}
      <mesh
        ref={skyMeshRef}
        name="map2-sky-dome"
        geometry={skyGeometry}
        material={skyMaterial}
      />

      {/* Procedural Twinkling Starfield */}
      <points
        ref={starPointsRef}
        name="map2-starfield"
        geometry={starGeometry}
        material={starMaterial}
      />

      {/* Directional Sun Light with High-Resolution Shadow Map */}
      <directionalLight
        ref={sunLightRef}
        name="map2-sun-light"
        position={[initialCelestial.sunPosition.x, initialCelestial.sunPosition.y, initialCelestial.sunPosition.z]}
        color={initialCelestial.sunColor}
        intensity={initialCelestial.sunIntensity}
        castShadow={enableShadows}
        shadow-mapSize-width={shadowMapSize}
        shadow-mapSize-height={shadowMapSize}
        shadow-camera-near={0.5}
        shadow-camera-far={1200}
        shadow-camera-left={-130}
        shadow-camera-right={130}
        shadow-camera-top={130}
        shadow-camera-bottom={-130}
        shadow-bias={-0.00015}
      />

      {/* Directional Moon Light for Night Illumination */}
      <directionalLight
        ref={moonLightRef}
        name="map2-moon-light"
        position={[initialCelestial.moonPosition.x, initialCelestial.moonPosition.y, initialCelestial.moonPosition.z]}
        color={initialCelestial.moonColor}
        intensity={initialCelestial.moonIntensity}
        castShadow={false}
      />

      {/* Ambient Fill Light */}
      <ambientLight
        ref={ambientLightRef}
        name="map2-ambient-light"
        color={initialCelestial.ambientColor}
        intensity={initialCelestial.ambientIntensity}
      />

      {/* Hemisphere Sky / Ground Contrast Light */}
      <hemisphereLight
        ref={hemiLightRef}
        name="map2-hemisphere-light"
        args={[initialCelestial.hemiSkyColor, initialCelestial.hemiGroundColor, initialCelestial.hemiIntensity]}
      />
    </group>
  );
};

export default Map2Atmosphere;
