import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Canvas, useFrame, useThree } from '@react-three/fiber';
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
import LiveAgents from './LiveAgents';
import WorldFauna from './WorldFauna';
import WorldSky from './WorldSky';
import WorldWeather, { type WeatherKind } from './WorldWeather';
import WorldMinimap, { type CameraView } from './WorldMinimap';
import WorldCompass from './WorldCompass';
import WorldCameraRig, { type CameraMode } from './WorldCameraRig';
import type { World } from './utils/worldGen';
import { BIOME_NAMES_VI, BIOME_EMOJI } from './utils/worldGen';
import { getMemoizedWorld, loadOrGenerateWorld } from './utils/worldCache';
import { findSpawn } from './utils/worldSample';
import { FLORA_RADIUS_REFERENCE_EXTENT } from './utils/floraClearance';
import { canonicalCameraToRender } from './utils/mapManifest';
import { CAPTURE_READY_FLAG, CAPTURE_SETTLE_FRAMES, readCaptureRequest } from './utils/captureMode';
import type { CaptureRequest } from './utils/captureMode';
import CaptureEvidenceOverlay from './CaptureEvidenceOverlay';
import { readInjectedEvidence } from './utils/mapEvidence';
import {
  SHARED_WORLD_SEED as WORLD_SEED,
  SHARED_WORLD_SIZE as WORLD_SIZE,
  SHARED_WORLD_SHAPE as WORLD_SHAPE,
} from '../../utils/sharedWorld';
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

// The world's identity (seed/size/shape) now lives in `utils/sharedWorld`, imported above and
// aliased to the names this file already used. It moved because the main app pushes the *same*
// world to the simulation: a second copy of those three values would not fail, it would make the
// world the agents live on depend on which page loaded last.

// World-space extent the terrain is drawn at (independent of WORLD_SIZE).
//
// Imported rather than written as a literal because the flora collider and canopy radii in
// `floraClearance.ts` are calibrated for this span — a tree is 1.4 units across *of a 1200-unit
// map*. Changing the scene extent without changing those together silently resizes every tree
// relative to the world.
const RENDER_SIZE = FLORA_RADIUS_REFERENCE_EXTENT;
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

/**
 * Turn the renderer's shadow pass on or off, and mark every material in the scene for recompile.
 *
 * A named operation taking the two objects explicitly, because both come from `useThree` and
 * writing to a value React handed the component is what `react-hooks/immutability` flags. The
 * parameter type is the one field this touches rather than the whole renderer, so the signature
 * says what it does to what.
 */
function applyShadowPass(
  gl: { shadowMap: Pick<THREE.WebGLShadowMap, 'enabled'> },
  scene: THREE.Scene,
  enabled: boolean,
): void {
  gl.shadowMap.enabled = enabled;
  // Shadow toggling only takes effect after materials recompile.
  //
  // Checked rather than asserted at each step. `traverse` visits every `Object3D` — groups, lights
  // and the camera rig's helpers included — and only some of those carry a material at all, so
  // calling each one a `Mesh` and reading `.material` off it was a claim that was false for most of
  // the scene and merely happened to yield `undefined`.
  scene.traverse((o: THREE.Object3D) => {
    if (!('material' in o)) return;
    const mat: unknown = o.material;
    if (Array.isArray(mat)) {
      for (const m of mat) if (m instanceof THREE.Material) m.needsUpdate = true;
    } else if (mat instanceof THREE.Material) {
      mat.needsUpdate = true;
    }
  });
}

/** Applies the quality preset LIVE (no GL-context remount): render scale + shadow pass. */
const QualityApplier: React.FC<{ quality: Quality }> = ({ quality }) => {
  const { gl, scene, setDpr } = useThree();
  useEffect(() => {
    setDpr(quality === 'high' ? Math.min(window.devicePixelRatio || 1, 1.5) : 1);
    applyShadowPass(gl, scene, quality === 'high');
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

/**
 * Settles the scene, stops it, renders exactly one final frame, and only then says so.
 *
 * Rendered only in capture mode. Three separate jobs, in an order that matters:
 *
 * **Settle.** "Loaded" is not "settled": R3F suspends on textures and geometry, `WorldTerrain` builds
 * its mesh on first frame, and instanced flora uploads its matrices in a layout effect. A screenshot
 * on the first frame after `load` catches some of that mid-flight, and which part varies with machine
 * speed. Counting real rendered frames is the cheap, honest wait.
 *
 * **Stop.** With the loop still running, the buffer the harness reads is whatever frame happened to
 * be current when `readPixels` reached the GPU — a race between the harness and the render loop.
 * Every frame is *supposed* to be identical by then, which is exactly the assumption a byte-identity
 * gate exists to test rather than to rely on. `setFrameloop('never')` ends the race.
 *
 * **Render once.** After the loop stops, one explicit `gl.render`. The buffer the harness reads is
 * then the product of a known number of renders — frame 91, every time — instead of "however many
 * fitted in before the read". Combined with `preserveDrawingBuffer`, that content is still there when
 * `readPixels` runs, however long afterwards.
 *
 * The flag is set last, so a harness that polls it can never observe a scene that is still moving.
 */
const CaptureReadySignal: React.FC<{ frames: number }> = ({ frames }) => {
  const seen = useRef(0);
  const gl = useThree((s) => s.gl);
  const scene = useThree((s) => s.scene);
  const camera = useThree((s) => s.camera);
  const setFrameloop = useThree((s) => s.setFrameloop);
  useFrame(() => {
    seen.current += 1;
    if (seen.current !== frames) return;
    setFrameloop('never');
    gl.render(scene, camera);
    window[CAPTURE_READY_FLAG] = true;
  });
  return null;
};

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
  // Deterministic capture of the canonical map views. `null` on every ordinary visit — see
  // `captureMode.ts` for why this is a query parameter and nothing else.
  //
  // A lazy `useState` initialiser rather than a ref filled during render: both read once, and only
  // one of them is a pure render. The request is fixed for the lifetime of the page, so re-reading it
  // per render would additionally let a history change move the camera mid-capture.
  const [capture] = useState<CaptureRequest | null>(() =>
    readCaptureRequest(typeof window === 'undefined' ? '' : window.location.search),
  );
  // The navigation/collision evidence the capture harness injects, or `null`. See
  // `CaptureEvidenceOverlay.tsx` for why the overlay draws a committed record and computes nothing.
  const [evidence] = useState(() => readInjectedEvidence());

  const [timeOfDay, setTimeOfDay] = useState(capture?.timeOfDay ?? 11.0);
  const [speed, setSpeed] = useState(capture ? 0 : 1.0); // 0 = paused
  const [weather, setWeather] = useState<WeatherKind>(capture?.weather ?? 'clear');
  const [camMode, setCamMode] = useState<CameraMode>('orbit');
  const [quality, setQuality] = useState<Quality>(capture?.quality ?? 'high');
  const [muted, setMuted] = useState(false);
  const [camReadout, setCamReadout] = useState({ x: 0, z: 0, biome: 0, fps: 0, locked: false });

  // Camera <-> HTML-overlay bridge: the rig writes here each frame; the minimap/HUD read it.
  const viewRef = useRef<CameraView>({ targetX: 0, targetZ: 0, camX: 0, camZ: 0 });
  const teleportRef = useRef<{ x: number; z: number } | null>(null);

  // Diagnostics hook (like __worldScene): lets tooling inspect the generated world data.
  useEffect(() => {
    window.__world = world;
  }, [world]);

  // The scenic patch of LAND the world opens on, rather than the origin (usually open ocean).
  //
  // Derived from the world, not remembered from an effect. It used to be computed in the effect
  // below and stashed in a ref, and Reset read `homeRef.current ?? { x: 0, z: 0 }`. That fallback is
  // the ocean origin this file warns about a few lines up: between the first render with a
  // renderable world and the effect that follows it, Reset had exactly the destination it is
  // documented never to use. Deriving `home` closes the window instead of narrowing it — it is
  // available on the same render that first draws the Reset button — and `null` now means "no world
  // yet", a state in which no button exists to press.
  const home = useMemo(
    () => (world && isWorldRenderable(world) ? findSpawn(world, RENDER_SIZE) : null),
    [world],
  );

  // Apply it once, on arrival; the rig consumes the teleport on its first frame.
  const spawnedRef = useRef(false);
  useEffect(() => {
    if (!home || spawnedRef.current) return;
    spawnedRef.current = true;
    viewRef.current.targetX = home.x;
    viewRef.current.targetZ = home.z;
    teleportRef.current = { x: home.x, z: home.z };
  }, [home]);

  // Load once. `world` is in the dependency list rather than suppressed out of it: the guard on the
  // first line makes the re-run the loaded world triggers a no-op, so the honest list and the empty
  // one do exactly the same thing — and the honest one keeps saying so if the guard ever moves.
  useEffect(() => {
    if (world) return;
    let alive = true;
    loadOrGenerateWorld(WORLD_SEED, { size: WORLD_SIZE, shape: WORLD_SHAPE }).then((w) => {
      if (alive) setWorld(w);
    });
    return () => {
      alive = false;
    };
  }, [world]);

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
  // Canonical poses are published in the canonical [-100, 100] bounds; the scene is a different
  // span with its own vertical exaggeration. One conversion, in `mapManifest.ts`.
  const capturePose = capture
    ? canonicalCameraToRender(capture.camera, RENDER_SIZE, HEIGHT_RATIO)
    : null;

  return (
    <div data-testid="world-showcase" style={{ width: '100%', height: '100%', position: 'relative' }}>
      <Canvas
        // `shadows` (bare) asks react-three-fiber for PCFSoftShadowMap, which three 0.184
        // deprecated: `WebGLShadowMap` warns and silently uses PCFShadowMap instead
        // (three.module.js:9135). The warning fires on every shadow-map rebuild, so a running
        // scene emitted it continuously — and it was telling the truth: the soft filter had not
        // been in use for some time. Naming the filter three actually applies removes the noise
        // and changes no pixels.
        shadows="percentage"
        dpr={[1, 1.5]}
        gl={{
          powerPreference: 'high-performance',
          // ---- three capture-only context flags, each closing one source of frame variance -----
          //
          // All three are `capture !== null` / `capture === null`, so an ordinary visit gets exactly
          // the context it got before: multisampled, alpha-composited, buffer discarded after
          // compositing. Nobody browsing the world is comparing their frames byte for byte, and two
          // of these cost real quality or performance.
          //
          // **preserveDrawingBuffer.** WebGL's default is that the drawing buffer's contents are
          // *undefined* once the frame has been composited, and reading it afterwards is a read of
          // undefined memory. It costs an extra copy of the frame and blocks compositor fast paths.
          preserveDrawingBuffer: capture !== null,
          // **antialias.** MSAA resolves samples in an implementation-defined order, and that is the
          // documented suspect for the last-bit differences an earlier pass tried to absorb with a
          // tolerance. Off, there is no resolve. The canonical views are aliased along high-contrast
          // silhouettes as a result — a visible cost, paid deliberately, because an evidence image
          // that cannot be reproduced is not evidence. See `captureMode.ts`.
          antialias: capture === null,
          // **alpha.** A context without an alpha channel cannot be composited against the page, and
          // `readPixels` on one returns 1.0 for alpha by specification. With alpha on, three's
          // blending writes `a = as² + ad(1 − as)` through transparent geometry, so water and
          // precipitation would save as partly transparent pixels in an image nobody would think to
          // check for transparency.
          alpha: capture === null,
        }}
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
          window.__worldScene = state.scene;
          if (capture) {
            // Dithering is on by default and the spec leaves the pattern to the implementation. It
            // exists to hide banding when a higher-precision colour is written to a lower-precision
            // buffer, which is a thing to want when a human is looking and a thing to remove when the
            // question is whether two frames are the same bytes. The three flags above are set at
            // context creation; this one is state, so it is set here — once, before any frame.
            const ctx = state.gl.getContext();
            ctx.disable(ctx.DITHER);
          }
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
        {/* Camels, musk oxen, boar, rabbits, turtles and frogs — the biomes `WorldWildlife` leaves
            empty. Table-driven, one instanced mesh per species. */}
        <WorldFauna
          world={world}
          renderSize={RENDER_SIZE}
          heightRatio={HEIGHT_RATIO}
          meshResolution={MESH_RES}
        />

        {/* The running simulation's population, on the world it is actually simulating in. Draws
            nothing without a Tauri transport, which is what keeps the canonical capture — driven by
            an ordinary Chromium — byte-identical. */}
        <LiveAgents
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
          capturePose={capturePose}
        />
        {/* Navigation route / collider rings, drawn from the committed evidence record the harness
            injects. Nothing renders on an ordinary visit: no capture request, no injected record. */}
        {capture && evidence ? (
          <CaptureEvidenceOverlay
            view={capture.view}
            evidence={evidence}
            world={world}
            renderSize={RENDER_SIZE}
            heightRatio={HEIGHT_RATIO}
            meshResolution={MESH_RES}
          />
        ) : null}
        {capture ? <CaptureReadySignal frames={CAPTURE_SETTLE_FRAMES} /> : null}
      </Canvas>

      {/* Every HTML overlay below is HUD. In capture mode none of it renders.

          Playwright's element screenshot captures the PAGE REGION the element occupies, not the
          element's own pixel buffer, so anything painted on top of the canvas composites into the
          image. The first capture run produced eight 'canonical map views' with the control panel,
          the compass ribbon, the biome banner and the minimap burned into them — a picture of the
          application, where the manifest promised a picture of the world. */}
      {capture ? null : (
        <>
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
        // Reset returns to the validated spawn, or does not exist. There is no third option and
        // specifically no origin fallback: `{ x: 0, z: 0 }` is the middle of the map, the middle of
        // this map is open ocean, and a browser reproduction of the old code put the readout biome
        // at "Đại dương" with the camera in the sea.
        onReset={
          home
            ? () => {
                teleportRef.current = { x: home.x, z: home.z };
              }
            : null
        }
      />

      <WorldMinimap
        world={world}
        renderSize={RENDER_SIZE}
        viewRef={viewRef}
        onTeleport={(x, z) => {
          teleportRef.current = { x, z };
        }}
      />
        </>
      )}
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
  /** `null` when no validated home position exists yet — Reset is then disabled, never guessed. */
  onReset: (() => void) | null;
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
        <button
          style={{ ...HUD_BTN(false), opacity: onReset ? 1 : 0.45, cursor: onReset ? 'pointer' : 'default' }}
          disabled={onReset === null}
          title={onReset ? 'Về điểm khởi đầu' : 'Chưa có điểm khởi đầu hợp lệ'}
          onClick={onReset ?? undefined}
        >
          ⟲ Reset
        </button>
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
