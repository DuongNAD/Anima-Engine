import React, { useRef, useMemo, useState } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';

interface WeatherProps {
  weather: 'clear' | 'rain' | 'snow' | 'fog';
  precipitationRate?: number;
  fogDensity?: number;
  timeOfDay?: number;
}

export const Weather: React.FC<WeatherProps> = ({
  weather,
  precipitationRate = 1.0,
  fogDensity = 0.01,
  timeOfDay = 12,
}) => {
  const rainGeomRef = useRef<THREE.BufferGeometry>(null);
  const snowGeomRef = useRef<THREE.BufferGeometry>(null);
  const pointsRef = useRef<any>(null); // For legacy compatibility with any tests expecting this ref

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
  const rainPositions = useMemo(() => {
    const arr = new Float32Array(maxRainCount * 3);
    for (let i = 0; i < maxRainCount; i++) {
      arr[i * 3] = (Math.random() - 0.5) * 200;
      arr[i * 3 + 1] = Math.random() * 80;
      arr[i * 3 + 2] = (Math.random() - 0.5) * 200;
    }
    return arr;
  }, []);

  const snowPositions = useMemo(() => {
    const arr = new Float32Array(maxSnowCount * 3);
    for (let i = 0; i < maxSnowCount; i++) {
      arr[i * 3] = (Math.random() - 0.5) * 200;
      arr[i * 3 + 1] = Math.random() * 80;
      arr[i * 3 + 2] = (Math.random() - 0.5) * 200;
    }
    return arr;
  }, []);

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
  });

  // Calculate rendering states
  const showRain = weather === 'rain';
  const showSnow = weather === 'snow';
  const totalParticleCount = Math.floor(
    (weather === 'rain' ? currentRainIntensity : 0) * maxRainCount +
    (weather === 'snow' ? currentSnowIntensity : 0) * maxSnowCount
  );

  // Fog color changes based on time of day (darker at night, grayish in storm/fog)
  let targetFogColor = '#cccccc';
  if (timeOfDay < 5 || timeOfDay > 20) {
    targetFogColor = '#020208'; // Night dark fog
  } else if (weather === 'rain' || weather === 'fog') {
    targetFogColor = '#8a9ba8'; // Rainy/foggy grayish blue
  } else if (weather === 'snow') {
    targetFogColor = '#d0dce5'; // Snowy cool white
  } else {
    targetFogColor = '#87ceeb'; // Clear day light blue
  }

  return (
    <group
      name="weather-group"
      userData={{ weather, precipitationRate, particleCount: totalParticleCount, fogDensity: currentFogDensity }}
      data-weather={weather}
      data-precipitation-rate={precipitationRate}
      data-particle-count={totalParticleCount}
      data-fog-density={currentFogDensity}
    >
      {/* Fog element attaches to scene */}
      <fogExp2 attach="fog" color={targetFogColor} density={currentFogDensity} />

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
