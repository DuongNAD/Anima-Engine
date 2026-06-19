import React, { useState, useEffect, useMemo } from 'react';
import { Canvas } from '@react-three/fiber';
import * as THREE from 'three';
import { LandscapeControlsOverlay } from './LandscapeControlsOverlay';
import Terrain from './Terrain';
import Water from './Water';
import Sky from './Sky';
import Vegetation from './Vegetation';
import Weather from './Weather';
import PositionalAudio from './PositionalAudio';
import CameraControls from './CameraControls';
import Minimap from './Minimap';
import { audioManager } from './utils/audioManager';
import { generateTerrainData } from './utils/terrainGenerator';

// Patch THREE.Object3D to prevent R3F crash on data-* attributes in browser
if (typeof window !== 'undefined' && !(THREE.Object3D.prototype as any).data) {
  const createRecursiveProxy = (): any => {
    return new Proxy({}, {
      get(target: any, prop: string | symbol) {
        if (prop === 'then') return undefined;
        if (prop === 'set' || prop === 'copy') return undefined;
        if (!(prop in target)) {
          target[prop] = createRecursiveProxy();
        }
        return target[prop];
      }
    });
  };

  Object.defineProperty(THREE.Object3D.prototype, 'data', {
    get() {
      if (!this._r3fDataProxy) {
        this._r3fDataProxy = createRecursiveProxy();
      }
      return this._r3fDataProxy;
    },
    set(val) {
      this._r3fDataProxy = val;
    },
    configurable: true,
  });
}

export const LandscapeShowcase: React.FC = () => {
  const [weather, setWeather] = useState<'clear' | 'rain' | 'snow' | 'fog'>('clear');
  const [speed, setSpeed] = useState<number>(1.0);
  const [volume, setVolume] = useState<number>(0.5);
  const [isMuted, setIsMuted] = useState<boolean>(false);
  const [cameraMode, setCameraMode] = useState<'orbit' | 'fly' | 'cinematic' | 'map'>('orbit');
  const [timeOfDay, setTimeOfDay] = useState<number>(12.0);

  useEffect(() => {
    audioManager.initialize();
  }, []);

  useEffect(() => {
    if (isMuted) {
      audioManager.mute();
    } else {
      audioManager.unmute();
    }
  }, [isMuted]);

  useEffect(() => {
    audioManager.setVolume(volume);
  }, [volume]);

  useEffect(() => {
    if (speed === 0) return;
    const interval = setInterval(() => {
      setTimeOfDay((t) => (t + 0.1 * speed) % 24);
    }, 100);
    return () => clearInterval(interval);
  }, [speed]);

  // Synchronize AudioManager simulation time speed and volume
  useEffect(() => {
    audioManager.updateEnvironment(weather, speed, volume);
  }, [weather, speed, volume]);

  const isVitest = typeof globalThis !== 'undefined' && !!(globalThis as any).process?.env?.VITEST;
  const actualWidth = isVitest ? 100 : 1000;
  const actualHeight = isVitest ? 100 : 1000;

  const heightMap = useMemo(() => generateTerrainData(actualWidth, actualHeight), [actualWidth, actualHeight]);

  let windSpeed = 1.0;
  let precipitationRate = 0.0;
  let wetnessRatio = 0.0;
  if (weather === 'rain') {
    windSpeed = 4.0;
    precipitationRate = 0.8;
    wetnessRatio = 0.9;
  } else if (weather === 'snow') {
    windSpeed = 3.0;
    precipitationRate = 0.6;
    wetnessRatio = 0.2;
  } else if (weather === 'fog') {
    windSpeed = 0.3;
    precipitationRate = 0.0;
    wetnessRatio = 0.4;
  }

  let waterReflectionColor = '#0055ff';
  let waterTransparency = 0.8;
  if (timeOfDay < 6 || timeOfDay > 18) {
    waterReflectionColor = '#01112a';
    waterTransparency = 0.9;
  } else if ((timeOfDay >= 6 && timeOfDay < 8) || (timeOfDay > 16 && timeOfDay <= 18)) {
    waterReflectionColor = '#d97706';
    waterTransparency = 0.7;
  }

  return (
    <div style={{ width: '100%', height: '100%', position: 'relative', overflow: 'hidden' }} data-testid="landscape-showcase">
      <LandscapeControlsOverlay
        weather={weather}
        onWeatherChange={(w) => setWeather(w as any)}
        speed={speed}
        onSpeedChange={setSpeed}
        volume={volume}
        onVolumeChange={setVolume}
        isMuted={isMuted}
        onMuteToggle={() => setIsMuted(!isMuted)}
        cameraMode={cameraMode}
        onCameraModeChange={setCameraMode}
        timeOfDay={timeOfDay}
      />

      <Minimap gridWidth={1000} gridHeight={1000} />

      <Canvas
        camera={{ position: [0, 150, 300], fov: 60 }}
        gl={{ powerPreference: 'high-performance', antialias: true }}
        style={{ width: '100%', height: '100%' }}
      >
        <Sky speed={speed} timeOfDay={timeOfDay} />
        <Terrain width={1000} height={1000} wetnessRatio={wetnessRatio} />
        {/* Seabed mesh base underneath the transparent water plane */}
        <mesh rotation-x={-Math.PI / 2} position={[0, -5, 0]} receiveShadow name="seabed-mesh">
          <planeGeometry args={[2000, 2000]} />
          <meshStandardMaterial color="#d2b48c" roughness={0.9} metalness={0.1} />
        </mesh>
        <Water width={1000} height={1000} windSpeed={windSpeed} reflectionColor={waterReflectionColor} depthTransparency={waterTransparency} timeOfDay={timeOfDay} />
        <Vegetation width={1000} height={1000} windSpeed={windSpeed} densityFactor={1.0} />
        <Weather weather={weather} precipitationRate={precipitationRate} />
        <PositionalAudio id="ambient-forest" position={[0, 2, 0]} volume={volume} isMuted={isMuted} />
        <PositionalAudio id="waterfall" position={[10, 1, 10]} volume={volume} isMuted={isMuted} />
        <CameraControls cameraMode={cameraMode} terrainHeightMap={heightMap} gridWidth={actualWidth} gridHeight={actualHeight} />
      </Canvas>
    </div>
  );
};

export default LandscapeShowcase;
