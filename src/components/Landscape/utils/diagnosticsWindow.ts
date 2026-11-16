// The window properties the landscape scene publishes for tooling, and for its own HTML overlays.
//
// # Why they exist
//
// The 3D scene lives inside a `<Canvas>` and the minimap, the Playwright harnesses and the
// landscape tests all live outside it. Rather than thread a camera reference back out through React,
// `CameraControls` writes the live objects onto `window` each frame and everyone else reads them.
//
// # Why they are declared centrally
//
// `CameraControls` declared this shape privately and `Minimap` read the same properties off an
// untyped view of `window`, which is the arrangement where a rename on the writing side is
// discovered by a user. The members now live in the one global declaration this app keeps for
// window properties, `window-globals.d.ts`, so both sides compile against the same contract — and
// `unknown` on the two object-valued entries keeps a reader honest about narrowing what it got.
//
// The function below is what everything calls; it exists so the intent reads at the call site
// (`diagnostics().activeCamera`, not `window.activeCamera`) and so the surface has one doc comment.

import type * as THREE from 'three';
import {
  CANONICAL_VIEW_CAMERAS,
  CANONICAL_VIEW_IDS,
  canonicalCameraToRender,
  type CanonicalPoint,
  type CanonicalViewId,
} from './mapManifest';

/** Terrain-height sampler, in the legacy landscape's grid coordinates. */
export type TerrainHeightProbe = (x: number, z: number) => number;

/** Move the active camera's look-at target to a world XZ position. */
export type TeleportCameraTarget = (worldX: number, worldZ: number) => void;

/** Global property name publishing the latest completed WebGL frame counters. */
export const FRAME_COUNTERS_GLOBAL = '__animaFrameCounters' as const;

/** Global property name publishing the canonical camera pose setter for measurements. */
export const CANONICAL_POSE_SETTER_GLOBAL = '__animaSetCanonicalPose' as const;

/** Ordered layer names reported by the per-frame WebGL diagnostics counters. */
export const FRAME_COUNTER_LAYERS = ['terrain', 'flora', 'creatures', 'water'] as const;

/** A primary scene layer tracked by the WebGL frame counters. */
export type FrameCounterLayer = (typeof FRAME_COUNTER_LAYERS)[number];

/** Scene layer classification including uncategorized scenery/atmosphere (`other`). */
export type ClassifiedLayer = FrameCounterLayer | 'other';

/** Draw-call and triangle totals for one frame or one scene layer. */
export interface LayerRenderCounts {
  calls: number;
  triangles: number;
}

/** Per-frame WebGL draw-call and triangle breakdown published on `window`. */
export interface FrameRenderCounters {
  frame: number;
  total: LayerRenderCounts;
  layers: Record<FrameCounterLayer, LayerRenderCounts>;
  other: LayerRenderCounts;
}

/** Imperative camera-pose pinning callback for reproducible per-pose measurements. */
export type CanonicalPoseSetter = (view: CanonicalViewId | null) => void;

/** Scene extent inputs used when converting canonical view poses to render space. */
export interface FrameCountersOptions {
  renderSize?: number;
  heightRatio?: number;
}

/** Lifecycle handle returned by `installFrameCounters`. */
export interface FrameCountersHandle {
  applyPinnedPose: (camera: unknown) => void;
  dispose: () => void;
}

/** The scene's published diagnostics surface. Every member is absent until the scene mounts. */
export function diagnostics(): Window {
  return window;
}

/** The minimum a reader needs off `activeCamera`: where it is. */
export interface CameraPositionReadout {
  position: { x: number; y: number; z: number };
}

/**
 * The live camera's position, or `null` when no scene has published one.
 *
 * A checked read rather than a cast: `activeCamera` is written by a component that may not be
 * mounted, and the minimap draws every animation frame whether it is or not.
 */
export function activeCameraPosition(): CameraPositionReadout['position'] | null {
  const camera = diagnostics().activeCamera;
  if (!camera || typeof camera !== 'object') return null;
  const { position } = camera as Partial<CameraPositionReadout>;
  if (!position || typeof position.x !== 'number' || typeof position.z !== 'number') return null;
  return position;
}

function isValidLayerTag(value: unknown): value is ClassifiedLayer {
  return (
    value === 'terrain' ||
    value === 'flora' ||
    value === 'creatures' ||
    value === 'water' ||
    value === 'other'
  );
}

function hasToneMappedDisabled(object: THREE.Object3D): boolean {
  const material = (object as { material?: unknown }).material;
  if (!material) return false;
  if (Array.isArray(material)) {
    return material.some(
      (entry) => typeof entry === 'object' && entry !== null && (entry as { toneMapped?: boolean }).toneMapped === false,
    );
  }
  return typeof material === 'object' && (material as { toneMapped?: boolean }).toneMapped === false;
}

function classifyByName(name: string): ClassifiedLayer | null {
  if (name === 'world-terrain' || name === 'world-terrain-lod' || name.startsWith('terrain-')) {
    return 'terrain';
  }
  if (name === 'world-vegetation' || name.startsWith('flora-') || name.startsWith('vegetation-')) {
    return 'flora';
  }
  if (
    name === 'world-water' ||
    name === 'world-waterfalls' ||
    name.startsWith('world-ocean') ||
    name.startsWith('world-lake') ||
    name.startsWith('waterfall-')
  ) {
    return 'water';
  }
  if (
    name === 'live-agents' ||
    name.startsWith('live-agent') ||
    name.startsWith('creature') ||
    name.startsWith('agent-')
  ) {
    return 'creatures';
  }
  if (
    name === 'world-sky-group' ||
    name.startsWith('world-sky') ||
    name.startsWith('world-cloud') ||
    name.startsWith('world-sun') ||
    name.startsWith('world-moon') ||
    name.startsWith('world-star') ||
    name === 'world-weather-group' ||
    name.startsWith('world-rain') ||
    name.startsWith('world-snow') ||
    name === 'world-caves' ||
    name === 'world-birds' ||
    name === 'world-fish' ||
    name === 'world-wildlife' ||
    name.startsWith('wildlife-') ||
    name.startsWith('capture-evidence')
  ) {
    return 'other';
  }
  return null;
}

/**
 * Attribute a Three.js scene object to one of the four tracked layers or `other`.
 *
 * Walks up to the enclosing `Scene`, preferring explicit `userData.rwLayer` overrides and known
 * component names, with a narrow fallback for `LiveAgents`' unnamed top-level `InstancedMesh`.
 */
export function classifySceneObjectLayer(object: THREE.Object3D): ClassifiedLayer {
  let cur: THREE.Object3D | null = object;
  while (cur && !(cur as { isScene?: boolean }).isScene) {
    const tagged = cur.userData?.rwLayer;
    if (isValidLayerTag(tagged)) return tagged;
    const named = cur.name ? classifyByName(cur.name.toLowerCase()) : null;
    if (named !== null) return named;
    cur = cur.parent;
  }

  // `LiveAgents` on trunk currently mounts an unnamed `<instancedMesh>` directly under `Scene`
  // with `MeshStandardMaterial` (`toneMapped: true`), whereas `CaptureEvidenceOverlay` wraps a
  // `toneMapped: false` `InstancedMesh` inside a `<group>`.
  const isTopLevelInstanced =
    (object as { isInstancedMesh?: boolean }).isInstancedMesh === true &&
    (object.parent as { isScene?: boolean } | null)?.isScene === true;
  if (isTopLevelInstanced && !hasToneMappedDisabled(object)) {
    return 'creatures';
  }
  return 'other';
}

interface RendererInfoLike {
  autoReset: boolean;
  reset: () => void;
  render: {
    frame: number;
    calls: number;
    triangles: number;
  };
}

interface RendererWithInfo {
  info: RendererInfoLike;
}

function asRendererWithInfo(gl: unknown): RendererWithInfo | null {
  if (!gl || typeof gl !== 'object') return null;
  const info = (gl as { info?: unknown }).info;
  if (!info || typeof info !== 'object') return null;
  const candidate = info as Partial<RendererInfoLike>;
  if (
    typeof candidate.autoReset !== 'boolean' ||
    typeof candidate.reset !== 'function' ||
    !candidate.render ||
    typeof candidate.render !== 'object' ||
    typeof candidate.render.frame !== 'number' ||
    typeof candidate.render.calls !== 'number' ||
    typeof candidate.render.triangles !== 'number'
  ) {
    return null;
  }
  return { info: candidate as RendererInfoLike };
}

function asScene(scene: unknown): THREE.Scene | null {
  if (!scene || typeof scene !== 'object') return null;
  const candidate = scene as Partial<THREE.Scene>;
  if (candidate.isScene !== true || typeof candidate.traverse !== 'function') return null;
  return candidate as THREE.Scene;
}

interface PinnableCamera {
  position: { set: (x: number, y: number, z: number) => unknown };
  lookAt: (x: number, y: number, z: number) => unknown;
  updateMatrixWorld: (force?: boolean) => unknown;
}

function asPinnableCamera(camera: unknown): PinnableCamera | null {
  if (!camera || typeof camera !== 'object') return null;
  const cam = camera as Partial<PinnableCamera>;
  if (
    !cam.position ||
    typeof cam.position.set !== 'function' ||
    typeof cam.lookAt !== 'function' ||
    typeof cam.updateMatrixWorld !== 'function'
  ) {
    return null;
  }
  return cam as PinnableCamera;
}

type SceneRenderHook = THREE.Object3D['onBeforeRender'];
type ShadowRenderHook = THREE.Object3D['onBeforeShadow'];

interface SavedObjectHooks {
  hasOwnBeforeRender: boolean;
  beforeRender: SceneRenderHook;
  hasOwnAfterRender: boolean;
  afterRender: SceneRenderHook;
  hasOwnBeforeShadow: boolean;
  beforeShadow: ShadowRenderHook;
  hasOwnAfterShadow: boolean;
  afterShadow: ShadowRenderHook;
}

function isRenderableObject(object: THREE.Object3D): boolean {
  const obj = object as { isMesh?: boolean; isLine?: boolean; isPoints?: boolean };
  return obj.isMesh === true || obj.isLine === true || obj.isPoints === true;
}

function restoreHook<K extends keyof THREE.Object3D>(
  target: THREE.Object3D,
  key: K,
  hadOwn: boolean,
  original: THREE.Object3D[K],
): void {
  if (hadOwn) {
    target[key] = original;
  } else {
    delete (target as Partial<THREE.Object3D>)[key];
  }
}

const NOOP_FRAME_COUNTERS_HANDLE: FrameCountersHandle = {
  applyPinnedPose: () => {},
  dispose: () => {},
};

/**
 * Instrument a Three.js `WebGLRenderer` and `Scene` to publish per-layer draw calls and triangles
 * (including shadow-map passes) to `window.__animaFrameCounters` after each frame.
 *
 * Returns a no-op handle in test environments whose renderer/scene mocks omit `gl.info`.
 */
export function installFrameCounters(
  gl: unknown,
  scene: unknown,
  options: FrameCountersOptions = {},
): FrameCountersHandle {
  const renderer = asRendererWithInfo(gl);
  const validScene = asScene(scene);
  if (!renderer || !validScene) {
    return NOOP_FRAME_COUNTERS_HANDLE;
  }

  const info = renderer.info;
  const renderSize = options.renderSize ?? 1200;
  const heightRatio = options.heightRatio ?? 0.14;

  let pinnedPose: CanonicalPoint | null = null;
  const applyPoseToCamera = (camera: unknown): void => {
    if (!pinnedPose) return;
    const cam = asPinnableCamera(camera);
    if (!cam) return;
    cam.position.set(pinnedPose.position[0], pinnedPose.position[1], pinnedPose.position[2]);
    cam.lookAt(pinnedPose.target[0], pinnedPose.target[1], pinnedPose.target[2]);
    cam.updateMatrixWorld(true);
  };

  // Three.js resets `gl.info` between `shadowMap.render` and `renderScene` when `autoReset` is
  // true (`WebGLRenderer.render`), which would drop all shadow-pass draw calls and triangles.
  // Managing reset once at the start of `scene.onBeforeRender` preserves the full-frame counts.
  const prevAutoReset = info.autoReset;
  info.autoReset = false;

  const hasOwnSceneBefore = Object.prototype.hasOwnProperty.call(validScene, 'onBeforeRender');
  const prevSceneBefore = validScene.onBeforeRender;
  const hasOwnSceneAfter = Object.prototype.hasOwnProperty.call(validScene, 'onAfterRender');
  const prevSceneAfter = validScene.onAfterRender;

  let activeDepth = 0;
  let snapCalls = 0;
  let snapTris = 0;
  let terrainCalls = 0;
  let terrainTris = 0;
  let floraCalls = 0;
  let floraTris = 0;
  let creaturesCalls = 0;
  let creaturesTris = 0;
  let waterCalls = 0;
  let waterTris = 0;
  let otherCalls = 0;
  let otherTris = 0;

  const hooked = new Map<THREE.Object3D, SavedObjectHooks>();

  const resetAccumulators = (): void => {
    terrainCalls = 0;
    terrainTris = 0;
    floraCalls = 0;
    floraTris = 0;
    creaturesCalls = 0;
    creaturesTris = 0;
    waterCalls = 0;
    waterTris = 0;
    otherCalls = 0;
    otherTris = 0;
  };

  const addDelta = (layer: ClassifiedLayer, calls: number, triangles: number): void => {
    if (calls === 0 && triangles === 0) return;
    switch (layer) {
      case 'terrain':
        terrainCalls += calls;
        terrainTris += triangles;
        break;
      case 'flora':
        floraCalls += calls;
        floraTris += triangles;
        break;
      case 'creatures':
        creaturesCalls += calls;
        creaturesTris += triangles;
        break;
      case 'water':
        waterCalls += calls;
        waterTris += triangles;
        break;
      case 'other':
        otherCalls += calls;
        otherTris += triangles;
        break;
    }
  };

  const hookObject = (obj: THREE.Object3D): void => {
    if (!isRenderableObject(obj) || hooked.has(obj)) return;

    const saved: SavedObjectHooks = {
      hasOwnBeforeRender: Object.prototype.hasOwnProperty.call(obj, 'onBeforeRender'),
      beforeRender: obj.onBeforeRender,
      hasOwnAfterRender: Object.prototype.hasOwnProperty.call(obj, 'onAfterRender'),
      afterRender: obj.onAfterRender,
      hasOwnBeforeShadow: Object.prototype.hasOwnProperty.call(obj, 'onBeforeShadow'),
      beforeShadow: obj.onBeforeShadow,
      hasOwnAfterShadow: Object.prototype.hasOwnProperty.call(obj, 'onAfterShadow'),
      afterShadow: obj.onAfterShadow,
    };
    hooked.set(obj, saved);

    obj.onBeforeRender = (...args) => {
      saved.beforeRender.apply(obj, args);
      snapCalls = info.render.calls;
      snapTris = info.render.triangles;
    };

    obj.onAfterRender = (...args) => {
      addDelta(
        classifySceneObjectLayer(obj),
        info.render.calls - snapCalls,
        info.render.triangles - snapTris,
      );
      saved.afterRender.apply(obj, args);
    };

    obj.onBeforeShadow = (...args) => {
      saved.beforeShadow.apply(obj, args);
      snapCalls = info.render.calls;
      snapTris = info.render.triangles;
    };

    obj.onAfterShadow = (...args) => {
      addDelta(
        classifySceneObjectLayer(obj),
        info.render.calls - snapCalls,
        info.render.triangles - snapTris,
      );
      saved.afterShadow.apply(obj, args);
    };
  };

  validScene.onBeforeRender = (...args) => {
    prevSceneBefore.apply(validScene, args);
    applyPoseToCamera(args[2]);
    if (activeDepth++ === 0) {
      info.reset();
      resetAccumulators();
      validScene.traverse(hookObject);
    }
  };

  validScene.onAfterRender = (...args) => {
    if (activeDepth > 0 && --activeDepth === 0) {
      const totalCalls = info.render.calls;
      const totalTris = Math.round(info.render.triangles);
      const roundedTerrainTris = Math.round(terrainTris);
      const roundedFloraTris = Math.round(floraTris);
      const roundedCreaturesTris = Math.round(creaturesTris);
      const roundedWaterTris = Math.round(waterTris);
      const namedCalls = terrainCalls + floraCalls + creaturesCalls + waterCalls;
      const namedTris =
        roundedTerrainTris + roundedFloraTris + roundedCreaturesTris + roundedWaterTris;

      diagnostics()[FRAME_COUNTERS_GLOBAL] = {
        frame: info.render.frame,
        total: { calls: totalCalls, triangles: totalTris },
        layers: {
          terrain: { calls: terrainCalls, triangles: roundedTerrainTris },
          flora: { calls: floraCalls, triangles: roundedFloraTris },
          creatures: { calls: creaturesCalls, triangles: roundedCreaturesTris },
          water: { calls: waterCalls, triangles: roundedWaterTris },
        },
        other: {
          calls: Math.max(otherCalls, totalCalls - namedCalls),
          triangles: Math.max(Math.round(otherTris), totalTris - namedTris),
        },
      };
    }
    prevSceneAfter.apply(validScene, args);
  };

  diagnostics()[CANONICAL_POSE_SETTER_GLOBAL] = (view: CanonicalViewId | null) => {
    if (view === null) {
      pinnedPose = null;
      return;
    }
    if (!(CANONICAL_VIEW_IDS as readonly string[]).includes(view)) {
      throw new Error(`diagnostics: unknown canonical view "${String(view)}"`);
    }
    pinnedPose = canonicalCameraToRender(CANONICAL_VIEW_CAMERAS[view], renderSize, heightRatio);
  };

  return {
    applyPinnedPose: applyPoseToCamera,
    dispose: () => {
      info.autoReset = prevAutoReset;
      restoreHook(validScene, 'onBeforeRender', hasOwnSceneBefore, prevSceneBefore);
      restoreHook(validScene, 'onAfterRender', hasOwnSceneAfter, prevSceneAfter);
      hooked.forEach((saved, obj) => {
        restoreHook(obj, 'onBeforeRender', saved.hasOwnBeforeRender, saved.beforeRender);
        restoreHook(obj, 'onAfterRender', saved.hasOwnAfterRender, saved.afterRender);
        restoreHook(obj, 'onBeforeShadow', saved.hasOwnBeforeShadow, saved.beforeShadow);
        restoreHook(obj, 'onAfterShadow', saved.hasOwnAfterShadow, saved.afterShadow);
      });
      hooked.clear();
      delete diagnostics()[FRAME_COUNTERS_GLOBAL];
      delete diagnostics()[CANONICAL_POSE_SETTER_GLOBAL];
    },
  };
}

function isLayerCounts(v: unknown): v is LayerRenderCounts {
  if (!v || typeof v !== 'object') return false;
  const c = v as Partial<LayerRenderCounts>;
  return typeof c.calls === 'number' && typeof c.triangles === 'number';
}

/** Read the latest completed WebGL frame counters, or `null` when unavailable. */
export function readFrameCounters(): FrameRenderCounters | null {
  const raw = diagnostics()[FRAME_COUNTERS_GLOBAL];
  if (!raw || typeof raw !== 'object') return null;
  const candidate = raw as Partial<FrameRenderCounters>;
  if (
    typeof candidate.frame !== 'number' ||
    !isLayerCounts(candidate.total) ||
    !isLayerCounts(candidate.other) ||
    !candidate.layers ||
    typeof candidate.layers !== 'object'
  ) {
    return null;
  }
  const layers = candidate.layers as Partial<Record<FrameCounterLayer, unknown>>;
  if (
    !isLayerCounts(layers.terrain) ||
    !isLayerCounts(layers.flora) ||
    !isLayerCounts(layers.creatures) ||
    !isLayerCounts(layers.water)
  ) {
    return null;
  }
  return candidate as FrameRenderCounters;
}

/** Pin or clear the canonical camera pose via the published diagnostics setter. */
export function setCanonicalPose(view: CanonicalViewId | null): boolean {
  const setter = diagnostics()[CANONICAL_POSE_SETTER_GLOBAL];
  if (typeof setter !== 'function') return false;
  setter(view);
  return true;
}

/** URL parameter of the budget gate's negative control (`RW-C4`): multiplies flora density. */
export const FLORA_DENSITY_SCALE_PARAM = 'floraDensityScale' as const;

/** Largest multiplier the debug switch accepts; keeps a typo from allocating the whole world. */
export const FLORA_DENSITY_SCALE_MAX = 8;

/**
 * Test-only multiplier on the tier's flora density, read from `?floraDensityScale=N`.
 *
 * It exists so `tests/e2e/rw_budget.spec.ts` can prove its own negative control: a gate that has
 * never been seen to go red proves nothing. It is inert in a production build, where
 * `import.meta.env.DEV` is not `true` and the whole body is dead code, and it returns `1` (no change)
 * for a missing, non-numeric, non-positive or out-of-range value. `dev`/`test` builds only.
 */
export function debugFloraDensityScale(search: string = typeof window === 'undefined' ? '' : window.location.search): number {
  // No `vite/client` types in this tsconfig, hence the narrow cast. Absent or false means inert.
  if ((import.meta as { env?: { DEV?: boolean } }).env?.DEV !== true) return 1;
  const raw = new URLSearchParams(search).get(FLORA_DENSITY_SCALE_PARAM);
  if (raw === null) return 1;
  const n = Number(raw);
  return Number.isFinite(n) && n > 0 && n <= FLORA_DENSITY_SCALE_MAX ? n : 1;
}
