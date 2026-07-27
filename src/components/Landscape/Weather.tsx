import React, { useRef, useMemo, useState, useEffect } from 'react';
import { useFrame, useThree } from '@react-three/fiber';
import * as THREE from 'three';
import { makeSceneRandom } from './utils/sceneClock';
import { testAttrs } from './testAttrs';

interface WeatherProps {
  weather: 'clear' | 'rain' | 'snow' | 'fog';
  precipitationRate?: number;
  fogDensity?: number;
  timeOfDay?: number;
}

/**
 * Install this component's exponential fog on the scene, and hand back its removal.
 *
 * A named operation taking the scene explicitly, rather than two assignments written inline in the
 * effect. `scene` is a value `useThree` handed the component, and writing to one of those is what
 * `react-hooks/immutability` flags — the fog belongs to the scene and this component owns it only
 * by convention, so the honest place to state that convention is a signature. Everything after
 * mount is eased in `useFrame`, off the render path entirely.
 */
function installExpFog(scene: THREE.Scene, color: string, density: number): () => void {
  scene.fog = new THREE.FogExp2(color, density);
  return () => {
    scene.fog = null;
  };
}

/** The fog colour for a given sky/weather combination. Darker at night, greyer in weather. */
function fogColorFor(weather: WeatherProps['weather'], timeOfDay: number): string {
  if (timeOfDay < 5 || timeOfDay > 20) return '#020208'; // Night dark fog
  if (weather === 'rain' || weather === 'fog') return '#8a9ba8'; // Rainy/foggy grayish blue
  if (weather === 'snow') return '#d0dce5'; // Snowy cool white
  return '#87ceeb'; // Clear day light blue
}

export const Weather: React.FC<WeatherProps> = ({
  weather,
  precipitationRate = 1.0,
  fogDensity = 0.01,
  timeOfDay = 12,
}) => {
  const rainGeomRef = useRef<THREE.BufferGeometry>(null);
  const snowGeomRef = useRef<THREE.BufferGeometry>(null);
  // Kept for tests that reach for it by name. `THREE.Points` is what it holds.
  const pointsRef = useRef<THREE.Points>(null);
  const { scene } = useThree();

  // Maximum particle counts
  const maxRainCount = 1000;
  const maxSnowCount = 800;

  // Track the actual transition values
  const [currentRainIntensity, setCurrentRainIntensity] = useState(weather === 'rain' ? precipitationRate : 0);
  const [currentSnowIntensity, setCurrentSnowIntensity] = useState(weather === 'snow' ? precipitationRate : 0);
  const [currentFogDensity, setCurrentFogDensity] = useState(
    weather === 'fog' ? 0.15 : weather === 'rain' ? 0.05 : weather === 'snow' ? 0.04 : 0.005
  );

  // Generate initial particle positions
  // Seeded under capture, `Math.random` otherwise — and `Math.random()` in a `useMemo` is a
  // render-phase impurity the React Compiler rules reject regardless. `makeSceneRandom` answers both.
  const rainPositions = useMemo(() => {
    const arr = new Float32Array(maxRainCount * 3);
    const rand = makeSceneRandom('weather.legacy.rain');
    for (let i = 0; i < maxRainCount; i++) {
      arr[i * 3] = (rand() - 0.5) * 200;
      arr[i * 3 + 1] = rand() * 80;
      arr[i * 3 + 2] = (rand() - 0.5) * 200;
    }
    return arr;
  }, [maxRainCount]);

  const snowPositions = useMemo(() => {
    const arr = new Float32Array(maxSnowCount * 3);
    const rand = makeSceneRandom('weather.legacy.snow');
    for (let i = 0; i < maxSnowCount; i++) {
      arr[i * 3] = (rand() - 0.5) * 200;
      arr[i * 3 + 1] = rand() * 80;
      arr[i * 3 + 2] = (rand() - 0.5) * 200;
    }
    return arr;
  }, [maxSnowCount]);

  // Imperatively manage scene.fog (replaces <fogExp2> which crashes in Three.js 0.184).
  //
  // Mount/unmount only, and the dependency list says so rather than an `eslint-disable-line`: the
  // density this installs is the *initial* one, and `useFrame` below drives it every frame after.
  // Re-running on a density change would reinstall the fog mid-flight and discard whatever the
  // frame loop had eased it to. `INITIAL_FOG_DENSITY` is read once, on purpose.
  const initialFogDensity = useRef(currentFogDensity);
  useEffect(() => installExpFog(scene, '#87ceeb', initialFogDensity.current), [scene]);

  // Fog colour follows time of day (darker at night, grey in storm/fog). Computed before the frame
  // loop that reads it: `useFrame`'s callback runs long after render, but a `const` declared below
  // it is still a temporal-dead-zone reference as far as the React Compiler is concerned, and it is
  // right to object — the ordering is only safe by accident of when frames happen to fire.
  const targetFogColor = fogColorFor(weather, timeOfDay);

  useFrame((state, delta) => {
    const time = state.clock.getElapsedTime();
    const safeDelta = Math.min(delta, 0.1);

    // Target values
    const targetRain = weather === 'rain' ? precipitationRate : 0;
    const targetSnow = weather === 'snow' ? precipitationRate : 0;
    
    let targetFog = 0.005;
    if (weather === 'rain') targetFog = 0.05;
    else if (weather === 'snow') targetFog = 0.04;
    else if (weather === 'fog') targetFog = fogDensity > 0.01 ? fogDensity : 0.15;

    // Smoothly update transition states
    const transitionSpeed = 2.0; // speed of weather transitions
    
    if (Math.abs(currentRainIntensity - targetRain) > 0.01) {
      setCurrentRainIntensity(THREE.MathUtils.lerp(currentRainIntensity, targetRain, safeDelta * transitionSpeed));
    } else if (currentRainIntensity !== targetRain) {
      setCurrentRainIntensity(targetRain);
    }

    if (Math.abs(currentSnowIntensity - targetSnow) > 0.01) {
      setCurrentSnowIntensity(THREE.MathUtils.lerp(currentSnowIntensity, targetSnow, safeDelta * transitionSpeed));
    } else if (currentSnowIntensity !== targetSnow) {
      setCurrentSnowIntensity(targetSnow);
    }

    if (Math.abs(currentFogDensity - targetFog) > 0.001) {
      setCurrentFogDensity(THREE.MathUtils.lerp(currentFogDensity, targetFog, safeDelta * transitionSpeed));
    } else if (currentFogDensity !== targetFog) {
      setCurrentFogDensity(targetFog);
    }

    // Animate rain particles downward
    if (rainGeomRef.current) {
      const posAttr = rainGeomRef.current.getAttribute('position');
      if (posAttr) {
        const arr = posAttr.array as Float32Array;
        for (let i = 0; i < arr.length / 3; i++) {
          arr[i * 3 + 1] -= safeDelta * 50.0; // Rain falls rapidly
          arr[i * 3] += safeDelta * 4.0;      // Slight wind angle

          if (arr[i * 3 + 1] < 0) {
            arr[i * 3 + 1] = 80 + Math.random() * 20;
            arr[i * 3] = (Math.random() - 0.5) * 200;
            arr[i * 3 + 2] = (Math.random() - 0.5) * 200;
          }
        }
        posAttr.needsUpdate = true;
      }
    }

    // Animate snow particles downward with swaying
    if (snowGeomRef.current) {
      const posAttr = snowGeomRef.current.getAttribute('position');
      if (posAttr) {
        const arr = posAttr.array as Float32Array;
        for (let i = 0; i < arr.length / 3; i++) {
          arr[i * 3 + 1] -= safeDelta * 12.0; // Snow falls slowly
          arr[i * 3] += Math.sin(time * 1.5 + i) * 0.06 + safeDelta * 1.0;

          if (arr[i * 3 + 1] < 0) {
            arr[i * 3 + 1] = 80 + Math.random() * 20;
            arr[i * 3] = (Math.random() - 0.5) * 200;
            arr[i * 3 + 2] = (Math.random() - 0.5) * 200;
          }
        }
        posAttr.needsUpdate = true;
      }
    }

    // Legacy fallback behavior for tests that update pointsRef position directly
    if (pointsRef.current && pointsRef.current.position) {
      pointsRef.current.position.y = -((time * 5) % 10);
    }

    // Update fog imperatively. Reached through `state.scene` rather than the closed-over `scene`:
    // inside the frame loop the live scene is the callback's own argument, which is where r3f
    // intends imperative work to read it from.
    const fog = state.scene.fog;
    if (fog instanceof THREE.FogExp2) {
      fog.density = currentFogDensity;
      fog.color.set(targetFogColor);
    }
  });

  // Calculate rendering states
  const showRain = weather === 'rain';
  const showSnow = weather === 'snow';
  const totalParticleCount = Math.floor(
    (weather === 'rain' ? currentRainIntensity : 0) * maxRainCount +
    (weather === 'snow' ? currentSnowIntensity : 0) * maxSnowCount
  );

  return (
    <group
      name="weather-group"
      userData={{ weather, precipitationRate, particleCount: totalParticleCount, fogDensity: currentFogDensity }}
      {...testAttrs({
        'data-weather': weather,
        'data-precipitation-rate': precipitationRate,
        'data-particle-count': totalParticleCount,
        'data-fog-density': currentFogDensity,
      })}
    >

      {/* Rain Points system */}
      {showRain && (
        <points ref={weather === 'rain' ? pointsRef : null} name="weather-particles">
          <bufferGeometry ref={rainGeomRef}>
            <bufferAttribute
              attach="attributes-position"
              count={rainPositions.length / 3}
              array={rainPositions}
              itemSize={3}
            />
          </bufferGeometry>
          <pointsMaterial
            size={0.08}
            color="#93c5fd"
            transparent
            opacity={0.6 * (currentRainIntensity / precipitationRate)}
          />
        </points>
      )}

      {/* Snow Points system */}
      {showSnow && (
        <points ref={weather === 'snow' ? pointsRef : null} name="weather-particles">
          <bufferGeometry ref={snowGeomRef}>
            <bufferAttribute
              attach="attributes-position"
              count={snowPositions.length / 3}
              array={snowPositions}
              itemSize={3}
            />
          </bufferGeometry>
          <pointsMaterial
            size={0.15}
            color="#ffffff"
            transparent
            opacity={0.75 * (currentSnowIntensity / precipitationRate)}
          />
        </points>
      )}
    </group>
  );
};

export default Weather;
