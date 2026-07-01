import React, { useEffect, useRef, useState } from 'react';
import { Canvas, extend, useFrame, useThree } from '@react-three/fiber';
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import WorldTerrain from './WorldTerrain';
import WorldVegetation from './WorldVegetation';
import WorldWater from './WorldWater';
import WorldSky from './WorldSky';
import WorldWeather, { type WeatherKind } from './WorldWeather';
import WorldMinimap, { type CameraView } from './WorldMinimap';
import type { World } from './utils/worldGen';
import { getMemoizedWorld, loadOrGenerateWorld } from './utils/worldCache';
import { sunDirectionForTime } from './utils/skyParams';

extend({ OrbitControls });

declare global {
  namespace JSX {
    interface IntrinsicElements {
      orbitControls: any;
    }
  }
}

const OrbitCam: React.FC<{
  viewRef: React.MutableRefObject<CameraView>;
  teleportRef: React.MutableRefObject<{ x: number; z: number } | null>;
}> = ({ viewRef, teleportRef }) => {
  const { camera, gl } = useThree();
  const ref = useRef<any>(null);
  useFrame(() => {
    const controls = ref.current;
    if (!controls) return;

    // Apply a pending minimap teleport: shift both target and camera so the framing is kept.
    const tp = teleportRef.current;
    if (tp) {
      const dx = tp.x - controls.target.x;
      const dz = tp.z - controls.target.z;
      controls.target.x += dx;
      controls.target.z += dz;
      camera.position.x += dx;
      camera.position.z += dz;
      teleportRef.current = null;
    }

    controls.update();

    // Report the live camera state to the HTML overlay (minimap / HUD).
    const v = viewRef.current;
    v.targetX = controls.target.x;
    v.targetZ = controls.target.z;
    v.camX = camera.position.x;
    v.camZ = camera.position.z;
  });
  return (
    <orbitControls
      ref={ref}
      args={[camera, gl.domElement]}
      enableDamping
      dampingFactor={0.08}
      maxPolarAngle={Math.PI / 2.05}
      minDistance={40}
      maxDistance={1200}
    />
  );
};

// Data resolution of the world (cells). Rendering is decoupled from this (see RENDER_SIZE).
const WORLD_SIZE = 1024;
const WORLD_SEED = 'seed';
const WORLD_SHAPE: 'island' | 'continent' = 'continent';

// World-space extent the terrain is drawn at (independent of WORLD_SIZE).
const RENDER_SIZE = 400;
const HEIGHT_RATIO = 0.13;
const MESH_RES = 256;

const WEATHERS: WeatherKind[] = ['clear', 'rain', 'snow', 'fog'];

/** Precipitation intensity (0..1) for each weather kind. */
function precipFor(weather: WeatherKind): number {
  if (weather === 'rain') return 0.85;
  if (weather === 'snow') return 0.6;
  return 0;
}

export const WorldShowcase: React.FC = () => {
  // Load the world ONCE. With IndexedDB we read the cached world (skipping generation);
  // without it (tests/SSR) we generate synchronously so a frame is available immediately.
  const [world, setWorld] = useState<World | null>(() =>
    typeof indexedDB === 'undefined'
      ? getMemoizedWorld(WORLD_SEED, { size: WORLD_SIZE, shape: WORLD_SHAPE })
      : null,
  );
  const [timeOfDay, setTimeOfDay] = useState(11.0);
  const [speed, setSpeed] = useState(1.0); // 0 = paused
  const [weather, setWeather] = useState<WeatherKind>('clear');
  const [camReadout, setCamReadout] = useState({ x: 0, z: 0 });

  // Camera <-> HTML-overlay bridge: OrbitCam writes here each frame; the minimap/HUD read it.
  const viewRef = useRef<CameraView>({ targetX: 0, targetZ: 0, camX: 0, camZ: 0 });
  const teleportRef = useRef<{ x: number; z: number } | null>(null);

  useEffect(() => {
    if (world) return;
    let alive = true;
    loadOrGenerateWorld(WORLD_SEED, { size: WORLD_SIZE, shape: WORLD_SHAPE }).then((w) => {
      if (alive) setWorld(w);
    });
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Advance the day/night clock (24h) at `speed`. Paused when speed is 0.
  useEffect(() => {
    if (speed === 0) return;
    const id = setInterval(() => setTimeOfDay((t) => (t + 0.05 * speed) % 24), 100);
    return () => clearInterval(id);
  }, [speed]);

  // Throttled readout of the camera target for the HUD (avoids per-frame React re-renders).
  useEffect(() => {
    const id = setInterval(() => {
      const v = viewRef.current;
      setCamReadout({ x: Math.round(v.targetX), z: Math.round(v.targetZ) });
    }, 300);
    return () => clearInterval(id);
  }, []);

  if (!world) {
    return (
      <div
        data-testid="world-showcase"
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
        Generating world… (first run only)
      </div>
    );
  }

  const sunDir = sunDirectionForTime(timeOfDay);

  return (
    <div data-testid="world-showcase" style={{ width: '100%', height: '100%', position: 'relative' }}>
      <Canvas
        shadows
        camera={{ position: [0, RENDER_SIZE * 0.55, RENDER_SIZE * 0.85], far: 4000, fov: 55 }}
        style={{ width: '100%', height: '100%' }}
        onCreated={(state) => {
          state.scene.background = new THREE.Color('#9fd0e8');
        }}
      >
        {/* Sky owns scene.background + lighting; weather owns scene.fog + precipitation. */}
        <WorldSky timeOfDay={timeOfDay} speed={speed} worldScale={RENDER_SIZE} />
        <WorldWeather
          weather={weather}
          precipitationRate={precipFor(weather)}
          timeOfDay={timeOfDay}
          worldScale={RENDER_SIZE}
        />

        <WorldTerrain
          world={world}
          renderSize={RENDER_SIZE}
          heightRatio={HEIGHT_RATIO}
          meshResolution={MESH_RES}
        />

        <WorldVegetation world={world} renderSize={RENDER_SIZE} heightRatio={HEIGHT_RATIO} />

        {/* Shader-based ocean (depth colour + swell + foam) and flowing rivers. */}
        <WorldWater
          world={world}
          renderSize={RENDER_SIZE}
          heightRatio={HEIGHT_RATIO}
          sunDir={sunDir}
        />

        <OrbitCam viewRef={viewRef} teleportRef={teleportRef} />
      </Canvas>

      <WorldHud
        timeOfDay={timeOfDay}
        speed={speed}
        weather={weather}
        coords={camReadout}
        onSpeed={setSpeed}
        onWeather={setWeather}
        onReset={() => {
          teleportRef.current = { x: 0, z: 0 };
        }}
      />

      <WorldMinimap
        world={world}
        renderSize={RENDER_SIZE}
        viewRef={viewRef}
        onTeleport={(x, z) => {
          teleportRef.current = { x, z };
        }}
      />
    </div>
  );
};

const HUD_BTN = (active: boolean): React.CSSProperties => ({
  padding: '4px 10px',
  borderRadius: 6,
  border: active ? '1px solid #7dd3fc' : '1px solid #334155',
  background: active ? 'rgba(56,189,248,0.25)' : 'rgba(15,23,42,0.6)',
  color: '#e2e8f0',
  cursor: 'pointer',
  fontSize: 12,
  textTransform: 'capitalize',
});

const WorldHud: React.FC<{
  timeOfDay: number;
  speed: number;
  weather: WeatherKind;
  coords: { x: number; z: number };
  onSpeed: (s: number) => void;
  onWeather: (w: WeatherKind) => void;
  onReset: () => void;
}> = ({ timeOfDay, speed, weather, coords, onSpeed, onWeather, onReset }) => {
  const hh = Math.floor(timeOfDay);
  const mm = Math.floor((timeOfDay - hh) * 60);
  const clock = `${String(hh).padStart(2, '0')}:${String(mm).padStart(2, '0')}`;
  const phase = timeOfDay < 5 || timeOfDay > 20 ? '🌙 Night' : timeOfDay < 8 ? '🌅 Dawn' : timeOfDay > 17 ? '🌇 Dusk' : '☀ Day';
  return (
    <div
      style={{
        position: 'absolute',
        top: 12,
        left: 12,
        display: 'flex',
        flexDirection: 'column',
        gap: 8,
        padding: 12,
        borderRadius: 10,
        background: 'rgba(2,6,23,0.55)',
        backdropFilter: 'blur(6px)',
        color: '#e2e8f0',
        fontFamily: 'sans-serif',
        fontSize: 12,
        userSelect: 'none',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <span style={{ fontSize: 16, fontVariantNumeric: 'tabular-nums' }}>🕑 {clock}</span>
        <span style={{ opacity: 0.85 }}>{phase}</span>
        <button style={HUD_BTN(false)} onClick={() => onSpeed(speed === 0 ? 1 : 0)}>
          {speed === 0 ? '▶ Play' : '⏸ Pause'}
        </button>
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
        <span style={{ opacity: 0.7 }}>Speed</span>
        {[0.5, 1, 2, 4].map((s) => (
          <button key={s} style={HUD_BTN(speed === s)} onClick={() => onSpeed(s)}>
            {s}×
          </button>
        ))}
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
        {WEATHERS.map((w) => (
          <button key={w} style={HUD_BTN(weather === w)} onClick={() => onWeather(w)}>
            {w}
          </button>
        ))}
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, opacity: 0.85 }}>
        <span style={{ fontVariantNumeric: 'tabular-nums' }}>📍 x {coords.x}, z {coords.z}</span>
        <button style={HUD_BTN(false)} onClick={onReset}>⟲ Reset</button>
      </div>
    </div>
  );
};

export default WorldShowcase;
