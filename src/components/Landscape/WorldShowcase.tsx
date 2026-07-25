import React, { useEffect, useRef, useState } from 'react';
import { Canvas, useThree } from '@react-three/fiber';
import * as THREE from 'three';
import WorldTerrain from './WorldTerrain';
import WorldTerrainLod from './WorldTerrainLod';
import WorldVegetation from './WorldVegetation';
import WorldWater from './WorldWater';
import WorldWaterfalls from './WorldWaterfalls';
import WorldCaves from './WorldCaves';
import WorldBirds from './WorldBirds';
import WorldFish from './WorldFish';
import WorldWildlife from './WorldWildlife';
import WorldSky from './WorldSky';
import WorldWeather, { type WeatherKind } from './WorldWeather';
import WorldMinimap, { type CameraView } from './WorldMinimap';
import WorldCompass from './WorldCompass';
import WorldCameraRig, { type CameraMode } from './WorldCameraRig';
import type { World } from './utils/worldGen';
import { BIOME_NAMES_VI, BIOME_EMOJI } from './utils/worldGen';
import { getMemoizedWorld, loadOrGenerateWorld } from './utils/worldCache';
import { findSpawn } from './utils/worldSample';
import {
  focusFromLookAt,
  sendLodFocus,
  sendLodFocusNow,
  shouldSend,
  FOCUS_OFF,
  SAMPLE_INTERVAL_MS,
  type LodFocusPayload,
} from '../../utils/lodFocus';
import { sunDirectionForTime } from './utils/skyParams';
import { audioManager } from './utils/audioManager';

// Data resolution of the world (cells). Rendering is decoupled from this (see RENDER_SIZE).
// 2048^2 = ~4M cells: a huge, detailed continent generated once (off-thread) and cached.
const WORLD_SIZE = 2048;
const WORLD_SEED = 'seed';
const WORLD_SHAPE: 'island' | 'continent' = 'continent';

// World-space extent the terrain is drawn at (independent of WORLD_SIZE).
const RENDER_SIZE = 1200;
const HEIGHT_RATIO = 0.14;
const MESH_RES = 384;
// M3: opt-in chunked terrain (frustum-culls off-screen chunks; see WorldTerrainLod / chunkLod).
// OFF by default = the proven single-mesh WorldTerrain. Turn on for a chunked terrain; add
// TERRAIN_LOD_DISTANCES (e.g. [520, 900]) + TERRAIN_SKIRT (e.g. 6) to also drop distant detail,
// which then FOLLOWS THE CAMERA (TERRAIN_DYNAMIC_LOD). Uniform mode (empty distances) is
// geometry-identical to WorldTerrain; verify the LOD look on real hardware before defaulting it.
const TERRAIN_CHUNKED: boolean = false;
const TERRAIN_CHUNKS_PER_SIDE = 6;
const TERRAIN_LOD_DISTANCES: number[] = [];
const TERRAIN_SKIRT = 0;
const TERRAIN_DYNAMIC_LOD = true;
// Streaming: keep only chunks within this world-radius of the camera resident (0 = off = all
// chunks). Bounds GPU/mesh memory for worlds bigger than one in-memory mesh, but unloads distant
// terrain — suits ground-level (walk/fly, fogged horizon) views, not the whole-map overview.
const TERRAIN_LOAD_RADIUS = 0;

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
  const [muted, setMuted] = useState(false);
  const [camReadout, setCamReadout] = useState({ x: 0, z: 0, biome: 0, fps: 0, locked: false });

  // Camera <-> HTML-overlay bridge: the rig writes here each frame; the minimap/HUD read it.
  const viewRef = useRef<CameraView>({ targetX: 0, targetZ: 0, camX: 0, camZ: 0 });
  const teleportRef = useRef<{ x: number; z: number } | null>(null);

  // Diagnostics hook (like __worldScene): lets tooling inspect the generated world data.
  useEffect(() => {
    (window as unknown as { __world?: World | null }).__world = world;
  }, [world]);

  // Open the world on a scenic patch of LAND rather than the origin (usually open ocean).
  // Runs once the world is ready; the rig applies the teleport on its first frame.
  const spawnedRef = useRef(false);
  useEffect(() => {
    if (!world || !isWorldRenderable(world) || spawnedRef.current) return;
    spawnedRef.current = true;
    const s = findSpawn(world, RENDER_SIZE);
    viewRef.current.targetX = s.x;
    viewRef.current.targetZ = s.z;
    teleportRef.current = { x: s.x, z: s.z };
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

  // Throttled readout of the camera telemetry for the HUD (avoids per-frame React re-renders).
  useEffect(() => {
    const id = setInterval(() => {
      const v = viewRef.current;
      setCamReadout({
        x: Math.round(v.camX),
        z: Math.round(v.camZ),
        biome: v.biome ?? 0,
        fps: Math.round(v.fps ?? 0),
        locked: v.locked ?? false,
      });
    }, 250);
    return () => clearInterval(id);
  }, []);

  // Tell the simulation where the explorer is standing, so it can spend its per-tick brain
  // inference near them instead of uniformly (`core/simulation_lod.rs`). Sampled on a timer like
  // the HUD readout above rather than driven from the render loop: this is a hint about where
  // detail belongs, and the tiers cannot respond faster than the camera is sampled.
  //
  // Inert outside Tauri. See `utils/lodFocus` for why leaving sends an explicit *off* rather than
  // simply going quiet, and why the camera's height is dropped.
  useEffect(() => {
    let last: LodFocusPayload | null = null;
    let inFlight = false;
    const id = setInterval(() => {
      if (inFlight) return;
      const v = viewRef.current;
      // `target`, not `cam`: in orbit mode the camera sits off the terrain to frame the whole
      // continent, and mapping *that* would focus the simulation outside its own world.
      const next = focusFromLookAt(v.targetX, v.targetZ, RENDER_SIZE);
      if (!shouldSend(last, next)) return;
      inFlight = true;
      void sendLodFocus(next).then((ok) => {
        inFlight = false;
        // Recorded only once it lands, so a dropped message is retried rather than assumed applied.
        if (ok) last = next;
      });
    }, SAMPLE_INTERVAL_MS);
    // Handing detail back is part of leaving. Without it the simulation stays tiered around
    // wherever the explorer last stood, with distant agents frozen out of thinking because a page
    // closed — and the agent views on `index.html` would inherit that stale focus with nothing on
    // screen to explain it.
    //
    // React's cleanup alone does not cover this. It runs on unmount, and leaving `landscape.html`
    // is a document navigation: the JS context is torn down without React ever unmounting anything.
    // `pagehide` is the event that actually fires for a navigation, a tab close and a bfcache
    // eviction, so both paths are wired and the send is idempotent.
    const leave = () => {
      // Synchronous: an `await import(...)` does not resolve on a page being torn down.
      sendLodFocusNow(FOCUS_OFF);
    };
    window.addEventListener('pagehide', leave);
    return () => {
      clearInterval(id);
      window.removeEventListener('pagehide', leave);
      leave();
    };
  }, []);

  // Ambient wind bed. Browsers block audio until a user gesture, so we lazily start the
  // synthesizer on the first click/keypress, then keep it in sync with the weather.
  const audioStarted = useRef(false);
  useEffect(() => {
    const AMBIENT_VOL = 0.55;
    const start = () => {
      if (audioStarted.current) return;
      audioStarted.current = true;
      audioManager.initialize();
      audioManager.updateEnvironment(weather, speed, AMBIENT_VOL);
      if (muted) audioManager.mute();
    };
    window.addEventListener('pointerdown', start);
    window.addEventListener('keydown', start);
    return () => {
      window.removeEventListener('pointerdown', start);
      window.removeEventListener('keydown', start);
    };
  }, [weather, speed, muted]);

  // Keep the wind bed reacting to weather/day-length once it's running.
  useEffect(() => {
    if (audioStarted.current) audioManager.updateEnvironment(weather, speed, 0.55);
  }, [weather, speed]);

  useEffect(() => {
    if (!audioStarted.current) return;
    if (muted) audioManager.mute();
    else audioManager.unmute();
  }, [muted]);

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
          // near must be SHORT for first-person walking: with near=2, looking up a steep
          // slope clipped the ground inside 2 units and let the camera see under the world.
          near: 0.6,
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

        {TERRAIN_CHUNKED ? (
          <WorldTerrainLod
            world={world}
            renderSize={RENDER_SIZE}
            heightRatio={HEIGHT_RATIO}
            meshResolution={MESH_RES}
            chunksPerSide={TERRAIN_CHUNKS_PER_SIDE}
            lodDistances={TERRAIN_LOD_DISTANCES}
            skirtDepth={TERRAIN_SKIRT}
            dynamicLod={TERRAIN_DYNAMIC_LOD}
            loadRadius={TERRAIN_LOAD_RADIUS}
          />
        ) : (
          <WorldTerrain
            world={world}
            renderSize={RENDER_SIZE}
            heightRatio={HEIGHT_RATIO}
            meshResolution={MESH_RES}
          />
        )}

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
        <WorldCaves world={world} renderSize={RENDER_SIZE} heightRatio={HEIGHT_RATIO} meshResolution={MESH_RES} />
        <WorldBirds renderSize={RENDER_SIZE} />
        <WorldFish world={world} renderSize={RENDER_SIZE} heightRatio={HEIGHT_RATIO} />
        <WorldWildlife
          world={world}
          renderSize={RENDER_SIZE}
          heightRatio={HEIGHT_RATIO}
          meshResolution={MESH_RES}
        />

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

      {/* Compass heading ribbon (rAF-driven, no per-frame React re-render). */}
      <WorldCompass viewRef={viewRef} />

      {/* Location banner — the biome you're standing in / looking at. */}
      <div
        data-testid="biome-banner"
        style={{
          position: 'absolute',
          top: 50,
          left: '50%',
          transform: 'translateX(-50%)',
          zIndex: 100,
          display: 'flex',
          gap: 8,
          alignItems: 'center',
          padding: '6px 16px',
          borderRadius: 999,
          background: 'rgba(2,6,23,0.5)',
          backdropFilter: 'blur(6px)',
          color: '#f1f5f9',
          fontFamily: 'sans-serif',
          fontSize: 14,
          fontWeight: 600,
          letterSpacing: 0.2,
          userSelect: 'none',
          pointerEvents: 'none',
          boxShadow: '0 2px 12px rgba(0,0,0,0.35)',
        }}
      >
        <span style={{ fontSize: 17 }}>{BIOME_EMOJI[camReadout.biome] ?? '📍'}</span>
        <span>{BIOME_NAMES_VI[camReadout.biome] ?? 'Vùng đất lạ'}</span>
      </div>

      {/* First-person reticle + click-to-explore prompt (walk / fly only). */}
      {(camMode === 'walk' || camMode === 'fly') && (
        <>
          <div
            style={{
              position: 'absolute',
              top: '50%',
              left: '50%',
              transform: 'translate(-50%, -50%)',
              pointerEvents: 'none',
              zIndex: 90,
            }}
          >
            <div
              style={{
                width: 6,
                height: 6,
                borderRadius: '50%',
                background: 'rgba(255,255,255,0.9)',
                boxShadow: '0 0 0 1.5px rgba(0,0,0,0.55)',
              }}
            />
          </div>
          {!camReadout.locked && (
            <div
              style={{
                position: 'absolute',
                top: 'calc(50% + 30px)',
                left: '50%',
                transform: 'translateX(-50%)',
                zIndex: 95,
                padding: '6px 14px',
                borderRadius: 8,
                background: 'rgba(2,6,23,0.62)',
                color: '#e2e8f0',
                fontFamily: 'sans-serif',
                fontSize: 12,
                pointerEvents: 'none',
                whiteSpace: 'nowrap',
              }}
            >
              🖱 Nhấp để khám phá · Esc để thả chuột
            </div>
          )}
        </>
      )}

      <WorldHud
        timeOfDay={timeOfDay}
        speed={speed}
        weather={weather}
        camMode={camMode}
        quality={quality}
        coords={camReadout}
        fps={camReadout.fps}
        muted={muted}
        onSpeed={setSpeed}
        onWeather={setWeather}
        onCamMode={setCamMode}
        onQuality={setQuality}
        onMute={() => setMuted((m) => !m)}
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
  fps: number;
  muted: boolean;
  onSpeed: (s: number) => void;
  onWeather: (w: WeatherKind) => void;
  onCamMode: (m: CameraMode) => void;
  onQuality: (q: Quality) => void;
  onMute: () => void;
  onReset: () => void;
}> = ({ timeOfDay, speed, weather, camMode, quality, coords, fps, muted, onSpeed, onWeather, onCamMode, onQuality, onMute, onReset }) => {
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
          {camMode === 'walk'
            ? 'WASD di chuyển · chuột nhìn quanh · Shift chạy · Space nhảy'
            : 'WASD di chuyển · chuột nhìn quanh · Shift tăng tốc · E/Space lên · Q xuống'}
        </div>
      )}
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, opacity: 0.85 }}>
        <span style={{ fontVariantNumeric: 'tabular-nums' }}>📍 x {coords.x}, z {coords.z}</span>
        <span
          title="Frames per second"
          style={{ fontVariantNumeric: 'tabular-nums', color: fps >= 50 ? '#86efac' : fps >= 30 ? '#fde68a' : '#fca5a5' }}
        >
          {fps} FPS
        </span>
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, opacity: 0.85 }}>
        <button style={HUD_BTN(false)} onClick={onReset}>⟲ Reset</button>
        <button style={HUD_BTN(!muted)} onClick={onMute} title="Âm thanh môi trường">
          {muted ? '🔇' : '🔊'}
        </button>
        <span style={{ opacity: 0.7 }}>GPU</span>
        <button style={HUD_BTN(quality === 'high')} onClick={() => onQuality('high')}>Đẹp</button>
        <button style={HUD_BTN(quality === 'low')} onClick={() => onQuality('low')}>Nhẹ</button>
      </div>
    </div>
  );
};

export default WorldShowcase;
