import React, { useRef, useMemo, useEffect } from 'react';
import { useFrame, useThree } from '@react-three/fiber';
import * as THREE from 'three';
import { sceneElapsed, makeSceneRandom, sceneSmoothing, sceneDelta } from './utils/sceneClock';

// ---------------------------------------------------------------------------------------
// WorldWeather — precipitation + distance fog for the huge world (WorldShowcase).
//
// Scaled-up sibling of the legacy Weather.tsx: the precipitation volume spans the rendered
// world (±worldScale) and fog uses LINEAR THREE.Fog with near/far tuned to RENDER_SIZE (the
// legacy FogExp2 0.005 density was calibrated for a ~100-unit world and would white-out a
// 400-unit one). This component OWNS scene.fog; WorldSky owns scene.background.
// ---------------------------------------------------------------------------------------

// One definition of the four weathers, in `utils/weatherKind`. Re-exported because the showcase,
// the minimap HUD, the capture request and the tests all import the name from here.
export type { WeatherKind } from './utils/weatherKind';
import type { WeatherKind } from './utils/weatherKind';

export interface WorldWeatherProps {
  weather: WeatherKind;
  precipitationRate?: number;
  timeOfDay?: number;
  worldScale?: number;
}

interface FogProfile {
  near: number;
  far: number;
  color: string;
}

function fogProfile(weather: WeatherKind, timeOfDay: number, s: number): FogProfile {
  const night = timeOfDay < 5 || timeOfDay > 20;
  if (weather === 'fog') return { near: s * 0.4, far: s * 2.4, color: night ? '#0a0c14' : '#9fb0bd' };
  if (weather === 'rain') return { near: s * 0.9, far: s * 4.0, color: night ? '#070a12' : '#8a9ba8' };
  if (weather === 'snow') return { near: s * 1.0, far: s * 4.5, color: night ? '#0c1018' : '#d0dce5' };
  // clear
  return { near: s * 2.0, far: s * 8.0, color: night ? '#05060f' : '#bfe2f2' };
}

/**
 * Install this component's linear fog on the scene, and hand back its removal.
 *
 * Named, and taking the scene explicitly, because `scene` is a value `useThree` handed the
 * component and writing to one of those is what `react-hooks/immutability` flags. The fog is the
 * scene's own state; this component owns it by convention (see the header), and a signature is
 * where a convention like that can be read.
 */
function installLinearFog(scene: THREE.Scene, p: FogProfile): () => void {
  scene.fog = new THREE.Fog(p.color, p.near, p.far);
  return () => {
    scene.fog = null;
  };
}

/** Ease an installed linear fog one frame toward `target`. */
function easeLinearFog(fog: THREE.Fog, target: FogProfile, k: number): void {
  fog.near = THREE.MathUtils.lerp(fog.near, target.near, k);
  fog.far = THREE.MathUtils.lerp(fog.far, target.far, k);
  fog.color.lerp(new THREE.Color(target.color), k);
}

export const WorldWeather: React.FC<WorldWeatherProps> = ({
  weather,
  precipitationRate = 0.8,
  timeOfDay = 12,
  worldScale = 400,
}) => {
  const { scene } = useThree();
  const rainGeomRef = useRef<THREE.BufferGeometry>(null);
  const snowGeomRef = useRef<THREE.BufferGeometry>(null);

  const SPREAD = worldScale * 2.0; // particle box half-extent in X/Z
  const TOP = worldScale * 0.75; // spawn height
  const maxRain = 1400;
  const maxSnow = 1000;

  // Seeded under capture — see `sceneClock.ts`. The canonical views are all shot in clear weather,
  // so no precipitation is drawn; the streams are seeded anyway because "the code path that would
  // have made this irreproducible is unreachable at the current defaults" is not a property worth
  // depending on.
  const rainPositions = useMemo(() => {
    const arr = new Float32Array(maxRain * 3);
    const rand = makeSceneRandom('weather.rain');
    for (let i = 0; i < maxRain; i++) {
      arr[i * 3] = (rand() - 0.5) * SPREAD * 2;
      arr[i * 3 + 1] = rand() * TOP;
      arr[i * 3 + 2] = (rand() - 0.5) * SPREAD * 2;
    }
    return arr;
  }, [SPREAD, TOP]);

  // One respawn stream per precipitation kind, created once per mount so a frozen capture and a
  // live scene both draw from a single sequence rather than restarting it every frame.
  const respawn = useMemo(
    () => ({ rain: makeSceneRandom('weather.rain.respawn'), snow: makeSceneRandom('weather.snow.respawn') }),
    [],
  );

  const snowPositions = useMemo(() => {
    const arr = new Float32Array(maxSnow * 3);
    const rand = makeSceneRandom('weather.snow');
    for (let i = 0; i < maxSnow; i++) {
      arr[i * 3] = (rand() - 0.5) * SPREAD * 2;
      arr[i * 3 + 1] = rand() * TOP;
      arr[i * 3 + 2] = (rand() - 0.5) * SPREAD * 2;
    }
    return arr;
  }, [SPREAD, TOP]);

  // Own scene.fog with a linear fog; transitions are eased toward the target in useFrame.
  //
  // The profile installed here is the one the *first* render asked for, captured at mount. That is
  // what the old empty dependency list meant, and it needed a lint suppression to say it: with
  // `weather`/`timeOfDay`/`worldScale` in the list the effect reinstalls the fog mid-flight on any
  // change and throws away whatever the frame loop had eased it to. Held in a ref, the effect's one
  // real dependency is the scene, and the list can be honest.
  const initialFogProfile = useRef(fogProfile(weather, timeOfDay, worldScale));
  useEffect(() => installLinearFog(scene, initialFogProfile.current), [scene]);

  useFrame((state, rawDelta) => {
    const time = sceneElapsed(state.clock);
    // Fixed under capture. Precipitation drifts by `safeDelta * worldScale`, which is unreachable at
    // the canonical clear weather and one parameter away from being reachable.
    const delta = sceneDelta(rawDelta);
    const safeDelta = Math.min(delta, 0.1);

    // Ease fog toward the current weather/time profile. Reached through `state.scene`: inside the
    // frame loop the live scene is the callback's own argument, which is where r3f intends
    // imperative work to read it from.
    const target = fogProfile(weather, timeOfDay, worldScale);
    const fog = state.scene.fog;
    if (fog instanceof THREE.Fog) {
      // Snapped rather than eased under capture: see `sceneSmoothing`. Easing integrates frame
      // deltas, so a fixed frame count converges by a machine-speed-dependent amount.
      easeLinearFog(fog, target, sceneSmoothing(Math.min(1, safeDelta * 2.0)));
    }

    if (weather === 'rain' && rainGeomRef.current) {
      const posAttr = rainGeomRef.current.getAttribute('position');
      if (posAttr) {
        const arr = posAttr.array as Float32Array;
        const fall = safeDelta * worldScale * 0.55;
        for (let i = 0; i < arr.length / 3; i++) {
          arr[i * 3 + 1] -= fall;
          arr[i * 3] += safeDelta * worldScale * 0.04;
          if (arr[i * 3 + 1] < 0) {
            arr[i * 3 + 1] = TOP + respawn.rain() * worldScale * 0.2;
            arr[i * 3] = (respawn.rain() - 0.5) * SPREAD * 2;
            arr[i * 3 + 2] = (respawn.rain() - 0.5) * SPREAD * 2;
          }
        }
        posAttr.needsUpdate = true;
      }
    }

    if (weather === 'snow' && snowGeomRef.current) {
      const posAttr = snowGeomRef.current.getAttribute('position');
      if (posAttr) {
        const arr = posAttr.array as Float32Array;
        const fall = safeDelta * worldScale * 0.13;
        for (let i = 0; i < arr.length / 3; i++) {
          arr[i * 3 + 1] -= fall;
          arr[i * 3] += Math.sin(time * 1.5 + i) * 0.3 + safeDelta * worldScale * 0.01;
          if (arr[i * 3 + 1] < 0) {
            arr[i * 3 + 1] = TOP + respawn.snow() * worldScale * 0.2;
            arr[i * 3] = (respawn.snow() - 0.5) * SPREAD * 2;
            arr[i * 3 + 2] = (respawn.snow() - 0.5) * SPREAD * 2;
          }
        }
        posAttr.needsUpdate = true;
      }
    }
  });

  const rainCount = Math.floor(maxRain * Math.min(1, precipitationRate));
  const snowCount = Math.floor(maxSnow * Math.min(1, precipitationRate));

  return (
    <group name="world-weather-group">
      {weather === 'rain' && (
        <points name="world-rain">
          <bufferGeometry ref={rainGeomRef}>
            <bufferAttribute attach="attributes-position" count={rainCount} array={rainPositions} itemSize={3} />
          </bufferGeometry>
          <pointsMaterial size={worldScale * 0.004} color="#9ec5ff" transparent opacity={0.55} fog={false} />
        </points>
      )}

      {weather === 'snow' && (
        <points name="world-snow">
          <bufferGeometry ref={snowGeomRef}>
            <bufferAttribute attach="attributes-position" count={snowCount} array={snowPositions} itemSize={3} />
          </bufferGeometry>
          <pointsMaterial size={worldScale * 0.01} color="#ffffff" transparent opacity={0.85} fog={false} />
        </points>
      )}
    </group>
  );
};

export default WorldWeather;
