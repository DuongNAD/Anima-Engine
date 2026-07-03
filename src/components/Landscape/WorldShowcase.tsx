import React, { useEffect, useRef, useState } from 'react';
import { Canvas, useThree } from '@react-three/fiber';
import * as THREE from 'three';
import WorldTerrain from './WorldTerrain';
import WorldVegetation from './WorldVegetation';
import WorldWater from './WorldWater';
import WorldWaterfalls from './WorldWaterfalls';
import WorldCaves from './WorldCaves';
import WorldBirds from './WorldBirds';
import WorldSky from './WorldSky';
import WorldWeather, { type WeatherKind } from './WorldWeather';
import WorldMinimap, { type CameraView } from './WorldMinimap';
import WorldCameraRig, { type CameraMode } from './WorldCameraRig';
import type { World } from './utils/worldGen';
import { getMemoizedWorld, loadOrGenerateWorld } from './utils/worldCache';
import { sunDirectionForTime } from './utils/skyParams';

// Data resolution of the world (cells). Rendering is decoupled from this (see RENDER_SIZE).
// 2048^2 = ~4M cells: a huge, detailed continent generated once (off-thread) and cached.
const WORLD_SIZE = 2048;
const WORLD_SEED = 'seed';
const WORLD_SHAPE: 'island' | 'continent' = 'continent';

// World-space extent the terrain is drawn at (independent of WORLD_SIZE).
const RENDER_SIZE = 1200;
const HEIGHT_RATIO = 0.14;
const MESH_RES = 384;

type Quality = 'high' | 'low';

const CAM_MODES: Array<{ key: CameraMode; label: string }> = [
  { key: 'orbit', label: '🌀 Quay' },
  { key: 'fly', label: '🕊 Bay' },
  { key: 'walk', label: '🚶 Đi bộ' },
  { key: 'top', label: '🗺 Trên cao' },
  { key: 'cine', label: '🎬 Cine' },
];

/** Applies the quality preset LIVE (no GL-context remount): render scale + shadow pass. */
const QualityApplier: React.FC<{ quality: Quality }> = ({ quality }) => {
  const { gl, scene, setDpr } = useThree();
  useEffect(() => {
    setDpr(quality === 'high' ? Math.min(window.devicePixelRatio || 1, 1.5) : 1);
    gl.shadowMap.enabled = quality === 'high';
    // Shadow toggling only takes effect after materials recompile.
    scene.traverse((o: THREE.Object3D) => {
      const mesh = o as THREE.Mesh;
      const mat = mesh.material as THREE.Material | THREE.Material[] | undefined;
      if (!mat) return;
      if (Array.isArray(mat)) mat.forEach((m) => (m.needsUpdate = true));
      else mat.needsUpdate = true;
    });
  }, [quality, gl, scene, setDpr]);
  return null;
};

const WEATHERS: WeatherKind[] = ['clear', 'rain', 'snow', 'fog'];

/** True only when a loaded world has every field the scene reads (guards against partial data). */
function isWorldRenderable(w: World): boolean {
  const n = w.size * w.size;
  return (
    w.elevation?.length === n &&
    w.flow?.length === n &&
    w.slope?.length === n &&
    w.water?.length === n &&
    w.riverAmt?.length === n &&
    w.shore?.length === n &&
    w.biome?.length === n &&
    Array.isArray(w.lakeBasins)
  );
}

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
  const [camMode, setCamMode] = useState<CameraMode>('orbit');
  const [quality, setQuality] = useState<Quality>('high');
  const [camReadout, setCamReadout] = useState({ x: 0, z: 0 });

  // Camera <-> HTML-overlay bridge: the rig writes here each frame; the minimap/HUD read it.
  const viewRef = useRef<CameraView>({ targetX: 0, targetZ: 0, camX: 0, camZ: 0 });
  const teleportRef = useRef<{ x: number; z: number } | null>(null);

  // Diagnostics hook (like __worldScene): lets tooling inspect the generated world data.
  useEffect(() => {
    (window as unknown as { __world?: World | null }).__world = world;
  }, [world]);

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

  if (!world || !isWorldRenderable(world)) {
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
        Generating a huge 2048² world… (first run only — cached afterwards)
      </div>
    );
  }

  const sunDir = sunDirectionForTime(timeOfDay);

  return (
    <div data-testid="world-showcase" style={{ width: '100%', height: '100%', position: 'relative' }}>
      <Canvas
        shadows
        dpr={[1, 1.5]}
        gl={{ powerPreference: 'high-performance' }}
        camera={{
          position: [0, RENDER_SIZE * 0.5, RENDER_SIZE * 0.8],
          near: 2,
          far: RENDER_SIZE * 11, // must exceed the sky dome (worldScale * 6.5)
          fov: 55,
        }}
        style={{ width: '100%', height: '100%' }}
        onCreated={(state) => {
          state.scene.background = new THREE.Color('#9fd0e8');
          // Debug/diagnostics hook (harmless in prod): lets tooling inspect the scene graph.
          (window as unknown as { __worldScene?: THREE.Scene }).__worldScene = state.scene;
        }}
      >
        <QualityApplier quality={quality} />

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

        <WorldVegetation
          world={world}
          renderSize={RENDER_SIZE}
          heightRatio={HEIGHT_RATIO}
          meshResolution={MESH_RES}
          quality={quality}
        />

        {/* Shader-based ocean/lakes (depth colour + swell + foam), waterfall curtains on the
            steep river drops, and cave mouths on the cliff faces. */}
        <WorldWater
          world={world}
          renderSize={RENDER_SIZE}
          heightRatio={HEIGHT_RATIO}
          sunDir={sunDir}
        />
        <WorldWaterfalls world={world} renderSize={RENDER_SIZE} heightRatio={HEIGHT_RATIO} />
        <WorldCaves world={world} renderSize={RENDER_SIZE} heightRatio={HEIGHT_RATIO} />
        <WorldBirds renderSize={RENDER_SIZE} />

        <WorldCameraRig
          mode={camMode}
          world={world}
          renderSize={RENDER_SIZE}
          heightRatio={HEIGHT_RATIO}
          meshResolution={MESH_RES}
          viewRef={viewRef}
          teleportRef={teleportRef}
        />
      </Canvas>

      <WorldHud
        timeOfDay={timeOfDay}
        speed={speed}
        weather={weather}
        camMode={camMode}
        quality={quality}
        coords={camReadout}
        onSpeed={setSpeed}
        onWeather={setWeather}
        onCamMode={setCamMode}
        onQuality={setQuality}
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
  camMode: CameraMode;
  quality: Quality;
  coords: { x: number; z: number };
  onSpeed: (s: number) => void;
  onWeather: (w: WeatherKind) => void;
  onCamMode: (m: CameraMode) => void;
  onQuality: (q: Quality) => void;
  onReset: () => void;
}> = ({ timeOfDay, speed, weather, camMode, quality, coords, onSpeed, onWeather, onCamMode, onQuality, onReset }) => {
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
      <div style={{ display: 'flex', alignItems: 'center', gap: 6, flexWrap: 'wrap' }}>
        {CAM_MODES.map((m) => (
          <button key={m.key} style={HUD_BTN(camMode === m.key)} onClick={() => onCamMode(m.key)}>
            {m.label}
          </button>
        ))}
      </div>
      {(camMode === 'fly' || camMode === 'walk') && (
        <div style={{ opacity: 0.75, fontSize: 11 }}>
          WASD di chuyển · giữ chuột trái kéo để nhìn · Shift tăng tốc{camMode === 'fly' ? ' · E/Q lên/xuống' : ''}
        </div>
      )}
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, opacity: 0.85 }}>
        <span style={{ fontVariantNumeric: 'tabular-nums' }}>📍 x {coords.x}, z {coords.z}</span>
        <button style={HUD_BTN(false)} onClick={onReset}>⟲ Reset</button>
        <span style={{ opacity: 0.7 }}>GPU</span>
        <button style={HUD_BTN(quality === 'high')} onClick={() => onQuality('high')}>Đẹp</button>
        <button style={HUD_BTN(quality === 'low')} onClick={() => onQuality('low')}>Nhẹ</button>
      </div>
    </div>
  );
};

export default WorldShowcase;
