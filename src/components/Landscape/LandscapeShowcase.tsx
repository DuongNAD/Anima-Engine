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
import type { TerrainData } from './utils/terrainGenerator';
import type { WeatherKind } from './utils/weatherKind';
import { getMemoizedTerrain, loadOrGenerateTerrain, heightDataFromTerrain } from './utils/terrainCache';

// World footprint (cells). Larger = bigger, more varied world; generation is cached so the
// heavy cost is paid only on the first ever run for a given size/seed.
const WORLD_SIZE = 160;
const WORLD_SEED = 'seed';

// Patch THREE.Object3D to prevent R3F crash on data-* attributes in browser.
//
// r3f resolves a dashed prop by walking the object: `data-testid` becomes `object.data.testid`, and
// `Object3D` has no `data`, so the walk dereferences `undefined` and throws. The proxy gives the
// walk something to land on that keeps producing more of itself.
//
// `Record<string | symbol, unknown>` rather than `any` for the proxy target: every property really
// is unknown — that is the point of the thing — and the difference is that a reader now has to say
// so, where `any` let one be dereferenced further without a word.
type RecursiveProxy = Record<string | symbol, unknown>;

interface Object3DWithData {
  data?: RecursiveProxy;
  _r3fDataProxy?: RecursiveProxy;
}

if (typeof window !== 'undefined' && !(THREE.Object3D.prototype as Object3DWithData).data) {
  const createRecursiveProxy = (): RecursiveProxy => {
    return new Proxy({} as RecursiveProxy, {
      get(target: RecursiveProxy, prop: string | symbol) {
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
    get(this: Object3DWithData) {
      if (!this._r3fDataProxy) {
        this._r3fDataProxy = createRecursiveProxy();
      }
      return this._r3fDataProxy;
    },
    set(this: Object3DWithData, val: RecursiveProxy) {
      this._r3fDataProxy = val;
    },
    configurable: true,
  });
}

export const LandscapeShowcase: React.FC = () => {
  const [weather, setWeather] = useState<WeatherKind>('clear');
  const [speed, setSpeed] = useState<number>(1.0);
  const [volume, setVolume] = useState<number>(0.5);
  const [isMuted, setIsMuted] = useState<boolean>(false);
  const [cameraMode, setCameraMode] = useState<'orbit' | 'fly' | 'cinematic' | 'map'>('orbit');
  const [timeOfDay, setTimeOfDay] = useState<number>(12.0);

  // Load the world ONCE (shared by every component). With IndexedDB available we read the
  // cached world from a previous session (skipping heavy generation); without it (e.g. tests)
  // we generate synchronously so the scene is present on first render.
  const [terrain, setTerrain] = useState<TerrainData | null>(() =>
    typeof indexedDB === 'undefined' ? getMemoizedTerrain(WORLD_SIZE, WORLD_SIZE, WORLD_SEED) : null,
  );
  // Load once. `terrain` is in the dependency list rather than suppressed out of it: the guard on
  // the first line makes the re-run the loaded terrain triggers a no-op, so the honest list and the
  // empty one do the same thing — and the honest one keeps saying so if the guard ever moves.
  // (`WorldShowcase` has the identical shape for its world.)
  useEffect(() => {
    if (terrain) return;
    let alive = true;
    loadOrGenerateTerrain(WORLD_SIZE, WORLD_SIZE, WORLD_SEED).then((t) => {
      if (alive) setTerrain(t);
    });
    return () => {
      alive = false;
    };
  }, [terrain]);

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

  // From the cached terrain, not a second generation. `main` built its own height map here at
  // 1000×1000 (capped to 100 under vitest, because that size was too slow for tests); this branch
  // later moved to one shared `terrain` at WORLD_SIZE, generated once and passed down, which is why
  // that side is not carried over — regenerating per consumer is the cost the cache exists to
  // remove, and the vitest cap was a symptom of it.
  const heightMap = useMemo(
    () => (terrain ? heightDataFromTerrain(terrain) : new Float32Array(0)),
    [terrain],
  );

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

  if (!terrain) {
    return (
      <div
        data-testid="landscape-showcase"
        style={{
          width: '100%',
          height: '100%',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          background: '#0a0a0a',
          color: '#9aa',
          fontFamily: 'sans-serif',
        }}
      >
        Generating world…
      </div>
    );
  }

  return (
    <div style={{ width: '100%', height: '100%', position: 'relative', overflow: 'hidden' }} data-testid="landscape-showcase">
      <LandscapeControlsOverlay
        weather={weather}
        onWeatherChange={setWeather}
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

      <Minimap gridWidth={WORLD_SIZE} gridHeight={WORLD_SIZE} />

      <Canvas
        camera={{ position: [0, WORLD_SIZE * 0.42, WORLD_SIZE * 0.72], fov: 60 }}
        // `gl` hints from `main`: orthogonal to the terrain question, so they carry over.
        gl={{ powerPreference: 'high-performance', antialias: true }}
        style={{ width: '100%', height: '100%' }}
        onCreated={(state) => { state.scene.fog = new THREE.FogExp2('#87ceeb', 0.0035); }}
      >
        <Sky speed={speed} timeOfDay={timeOfDay} />
        <Terrain width={WORLD_SIZE} height={WORLD_SIZE} wetnessRatio={wetnessRatio} terrain={terrain} />
        {/* Seabed under the transparent water plane, from `main`. Sized to the grid rather than its
            fixed 2000, which was scaled for that side's 1000-wide world. */}
        <mesh rotation-x={-Math.PI / 2} position={[0, -5, 0]} receiveShadow name="seabed-mesh">
          <planeGeometry args={[WORLD_SIZE * 2, WORLD_SIZE * 2]} />
          <meshStandardMaterial color="#d2b48c" roughness={0.9} metalness={0.1} />
        </mesh>
        <Water width={WORLD_SIZE} height={WORLD_SIZE} windSpeed={windSpeed} reflectionColor={waterReflectionColor} depthTransparency={waterTransparency} timeOfDay={timeOfDay} terrain={terrain} />
        <Vegetation width={WORLD_SIZE} height={WORLD_SIZE} windSpeed={windSpeed} densityFactor={1.0} terrain={terrain} />
        <Weather weather={weather} precipitationRate={precipitationRate} />
        <PositionalAudio id="ambient-forest" position={[0, 2, 0]} volume={volume} isMuted={isMuted} />
        <PositionalAudio id="waterfall" position={[10, 1, 10]} volume={volume} isMuted={isMuted} />
        <CameraControls cameraMode={cameraMode} terrainHeightMap={heightMap} gridWidth={WORLD_SIZE} gridHeight={WORLD_SIZE} />
      </Canvas>
    </div>
  );
};

export default LandscapeShowcase;
