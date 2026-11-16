import { describe, it, expect, vi, afterEach } from 'vitest';
import * as THREE from 'three';
import {
  CANONICAL_POSE_SETTER_GLOBAL,
  FRAME_COUNTERS_GLOBAL,
  classifySceneObjectLayer,
  installFrameCounters,
  readFrameCounters,
  setCanonicalPose,
  type FrameCountersHandle,
} from '../../src/components/Landscape/utils/diagnosticsWindow';
import {
  CANONICAL_VIEW_CAMERAS,
  canonicalCameraToRender,
} from '../../src/components/Landscape/utils/mapManifest';

interface SimulatedRenderable {
  object: THREE.Object3D;
  mainCalls: number;
  mainTris: number;
  shadowCalls?: number;
  shadowTris?: number;
}

/**
 * Simulate `THREE.WebGLRenderer.render` (`three@0.184.0`):
 * 1. `scene.onBeforeRender`
 * 2. `shadowMap.render` (`onBeforeShadow` / draw / `onAfterShadow`)
 * 3. `if (info.autoReset) info.reset()`
 * 4. `renderScene` (`onBeforeRender` / draw / `onAfterRender`)
 * 5. `scene.onAfterRender`
 */
function simulateThreeFrame(
  gl: {
    info: {
      autoReset: boolean;
      reset: () => void;
      render: { frame: number; calls: number; triangles: number };
    };
  },
  scene: THREE.Scene,
  camera: THREE.Camera,
  renderables: SimulatedRenderable[],
): { rawWebGlCalls: number; rawWebGlTris: number } {
  const geom = new THREE.BufferGeometry();
  const mat = new THREE.MeshBasicMaterial();
  const dummyGroup = new THREE.Group();
  let rawWebGlCalls = 0;
  let rawWebGlTris = 0;

  const rendererArg = gl as unknown as THREE.WebGLRenderer;
  scene.onBeforeRender(rendererArg, scene, camera, geom, mat, dummyGroup);

  gl.info.render.frame += 1;

  for (const item of renderables) {
    if (!item.shadowCalls || !item.shadowTris) continue;
    item.object.onBeforeShadow(rendererArg, scene, camera, camera, geom, mat, dummyGroup);
    gl.info.render.calls += item.shadowCalls;
    gl.info.render.triangles += item.shadowTris;
    rawWebGlCalls += item.shadowCalls;
    rawWebGlTris += item.shadowTris;
    item.object.onAfterShadow(rendererArg, scene, camera, camera, geom, mat, dummyGroup);
  }

  if (gl.info.autoReset) {
    gl.info.reset();
  }

  for (const item of renderables) {
    item.object.onBeforeRender(rendererArg, scene, camera, geom, mat, dummyGroup);
    gl.info.render.calls += item.mainCalls;
    gl.info.render.triangles += item.mainTris;
    rawWebGlCalls += item.mainCalls;
    rawWebGlTris += item.mainTris;
    item.object.onAfterRender(rendererArg, scene, camera, geom, mat, dummyGroup);
  }

  scene.onAfterRender(rendererArg, scene, camera, geom, mat, dummyGroup);
  geom.dispose();
  mat.dispose();
  return { rawWebGlCalls, rawWebGlTris };
}

describe('diagnosticsWindow per-layer WebGL frame counters (RW-C1 / RW0-4)', () => {
  let activeHandle: FrameCountersHandle | null = null;

  afterEach(() => {
    activeHandle?.dispose();
    activeHandle = null;
  });

  it('classifies terrain, flora, water, creatures, and other objects accurately', () => {
    const scene = new THREE.Scene();

    const terrain = new THREE.Mesh();
    terrain.name = 'world-terrain';
    scene.add(terrain);

    const terrainLodGroup = new THREE.Group();
    terrainLodGroup.name = 'world-terrain-lod';
    const terrainChunk = new THREE.Mesh();
    terrainLodGroup.add(terrainChunk);
    scene.add(terrainLodGroup);

    const vegGroup = new THREE.Group();
    vegGroup.name = 'world-vegetation';
    const floraMesh = new THREE.InstancedMesh(new THREE.BoxGeometry(1, 1, 1), new THREE.MeshBasicMaterial(), 4);
    floraMesh.name = 'flora-0';
    vegGroup.add(floraMesh);
    scene.add(vegGroup);

    const waterGroup = new THREE.Group();
    waterGroup.name = 'world-water';
    const oceanMesh = new THREE.Mesh();
    oceanMesh.name = 'world-ocean';
    waterGroup.add(oceanMesh);
    scene.add(waterGroup);

    const waterfallGroup = new THREE.Group();
    waterfallGroup.name = 'world-waterfalls';
    const waterfallMesh = new THREE.InstancedMesh(new THREE.PlaneGeometry(1, 1), new THREE.MeshBasicMaterial(), 2);
    waterfallMesh.name = 'waterfall-curtains';
    waterfallGroup.add(waterfallMesh);
    scene.add(waterfallGroup);

    const namedCreaturesGroup = new THREE.Group();
    namedCreaturesGroup.name = 'live-agents';
    const creaturePart = new THREE.Mesh();
    namedCreaturesGroup.add(creaturePart);
    scene.add(namedCreaturesGroup);

    // Trunk `LiveAgents.tsx` currently renders an unnamed top-level InstancedMesh with MeshStandardMaterial.
    const unnamedLiveAgents = new THREE.InstancedMesh(
      new THREE.SphereGeometry(1, 6, 4),
      new THREE.MeshStandardMaterial(),
      8,
    );
    scene.add(unnamedLiveAgents);

    // `CaptureEvidenceOverlay` wraps a `toneMapped: false` InstancedMesh inside a group.
    const evidenceGroup = new THREE.Group();
    const evidenceRings = new THREE.InstancedMesh(
      new THREE.TorusGeometry(1, 0.1, 4, 8),
      new THREE.MeshBasicMaterial({ toneMapped: false }),
      3,
    );
    evidenceGroup.add(evidenceRings);
    scene.add(evidenceGroup);

    const skyGroup = new THREE.Group();
    skyGroup.name = 'world-sky-group';
    const cloudMesh = new THREE.Mesh();
    skyGroup.add(cloudMesh);
    scene.add(skyGroup);

    const wildlifeGroup = new THREE.Group();
    wildlifeGroup.name = 'world-wildlife';
    const deerMesh = new THREE.InstancedMesh(new THREE.BoxGeometry(1, 1, 1), new THREE.MeshBasicMaterial(), 5);
    deerMesh.name = 'wildlife-deer';
    wildlifeGroup.add(deerMesh);
    scene.add(wildlifeGroup);

    const customTagged = new THREE.Mesh();
    customTagged.userData = { rwLayer: 'creatures' };
    scene.add(customTagged);

    expect(classifySceneObjectLayer(terrain)).toBe('terrain');
    expect(classifySceneObjectLayer(terrainChunk)).toBe('terrain');
    expect(classifySceneObjectLayer(floraMesh)).toBe('flora');
    expect(classifySceneObjectLayer(oceanMesh)).toBe('water');
    expect(classifySceneObjectLayer(waterfallMesh)).toBe('water');
    expect(classifySceneObjectLayer(creaturePart)).toBe('creatures');
    expect(classifySceneObjectLayer(unnamedLiveAgents)).toBe('creatures');
    expect(classifySceneObjectLayer(customTagged)).toBe('creatures');
    expect(classifySceneObjectLayer(evidenceRings)).toBe('other');
    expect(classifySceneObjectLayer(cloudMesh)).toBe('other');
    expect(classifySceneObjectLayer(deerMesh)).toBe('other');
  });

  it('tracks shadow + main pass calls and triangles per layer without losing shadow counts to autoReset', () => {
    expect(readFrameCounters()).toBeNull();

    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(55, 1024 / 768, 0.6, 13200);

    const terrain = new THREE.Mesh();
    terrain.name = 'world-terrain';
    scene.add(terrain);

    const vegGroup = new THREE.Group();
    vegGroup.name = 'world-vegetation';
    const flora = new THREE.Mesh();
    flora.name = 'flora-0';
    const existingFloraHook = vi.fn();
    flora.onBeforeRender = existingFloraHook;
    vegGroup.add(flora);
    scene.add(vegGroup);

    const creatures = new THREE.InstancedMesh(
      new THREE.SphereGeometry(1, 6, 4),
      new THREE.MeshStandardMaterial(),
      10,
    );
    scene.add(creatures);

    const waterGroup = new THREE.Group();
    waterGroup.name = 'world-water';
    const ocean = new THREE.Mesh();
    ocean.name = 'world-ocean';
    waterGroup.add(ocean);
    scene.add(waterGroup);

    const skyGroup = new THREE.Group();
    skyGroup.name = 'world-sky-group';
    const cloud = new THREE.Mesh();
    skyGroup.add(cloud);
    scene.add(skyGroup);

    const renderInfo = {
      autoReset: true,
      render: { frame: 0, calls: 0, triangles: 0 },
      reset: () => {
        renderInfo.render.calls = 0;
        renderInfo.render.triangles = 0;
      },
    };
    const gl = { info: renderInfo };

    activeHandle = installFrameCounters(gl, scene, { renderSize: 1200, heightRatio: 0.14 });
    expect(renderInfo.autoReset).toBe(false);

    const raw = simulateThreeFrame(gl, scene, camera, [
      { object: terrain, mainCalls: 1, mainTris: 294_912 },
      { object: flora, mainCalls: 1, mainTris: 800_000, shadowCalls: 1, shadowTris: 800_000 },
      { object: creatures, mainCalls: 1, mainTris: 2_400, shadowCalls: 1, shadowTris: 2_400 },
      { object: ocean, mainCalls: 1, mainTris: 32_768 },
      { object: cloud, mainCalls: 1, mainTris: 60, shadowCalls: 1, shadowTris: 60 },
    ]);

    expect(existingFloraHook).toHaveBeenCalledTimes(1);

    const counters = readFrameCounters();
    expect(counters).not.toBeNull();
    expect(counters?.frame).toBe(1);
    expect(counters?.layers.terrain).toEqual({ calls: 1, triangles: 294_912 });
    expect(counters?.layers.flora).toEqual({ calls: 2, triangles: 1_600_000 });
    expect(counters?.layers.creatures).toEqual({ calls: 2, triangles: 4_800 });
    expect(counters?.layers.water).toEqual({ calls: 1, triangles: 32_768 });
    expect(counters?.other).toEqual({ calls: 2, triangles: 120 });
    expect(counters?.total).toEqual({ calls: raw.rawWebGlCalls, triangles: raw.rawWebGlTris });

    const sumCalls =
      (counters?.layers.terrain.calls ?? 0) +
      (counters?.layers.flora.calls ?? 0) +
      (counters?.layers.creatures.calls ?? 0) +
      (counters?.layers.water.calls ?? 0) +
      (counters?.other.calls ?? 0);
    const sumTris =
      (counters?.layers.terrain.triangles ?? 0) +
      (counters?.layers.flora.triangles ?? 0) +
      (counters?.layers.creatures.triangles ?? 0) +
      (counters?.layers.water.triangles ?? 0) +
      (counters?.other.triangles ?? 0);

    expect(sumCalls).toBe(counters?.total.calls);
    expect(sumTris).toBe(counters?.total.triangles);

    activeHandle.dispose();
    activeHandle = null;

    expect(renderInfo.autoReset).toBe(true);
    expect(flora.onBeforeRender).toBe(existingFloraHook);
    expect(readFrameCounters()).toBeNull();
    expect(window[FRAME_COUNTERS_GLOBAL]).toBeUndefined();
    expect(window[CANONICAL_POSE_SETTER_GLOBAL]).toBeUndefined();
  });

  it('pins and clears canonical camera poses (overview, ecosystem, spawn)', () => {
    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(55, 1024 / 768, 0.6, 13200);
    const renderInfo = {
      autoReset: true,
      render: { frame: 0, calls: 0, triangles: 0 },
      reset: () => {
        renderInfo.render.calls = 0;
        renderInfo.render.triangles = 0;
      },
    };
    const gl = { info: renderInfo };

    activeHandle = installFrameCounters(gl, scene, { renderSize: 1200, heightRatio: 0.14 });

    for (const viewId of ['overview', 'ecosystem', 'spawn'] as const) {
      expect(setCanonicalPose(viewId)).toBe(true);
      simulateThreeFrame(gl, scene, camera, []);
      const expected = canonicalCameraToRender(CANONICAL_VIEW_CAMERAS[viewId], 1200, 0.14);
      expect(camera.position.x).toBeCloseTo(expected.position[0], 5);
      expect(camera.position.y).toBeCloseTo(expected.position[1], 5);
      expect(camera.position.z).toBeCloseTo(expected.position[2], 5);
    }

    expect(setCanonicalPose(null)).toBe(true);
    camera.position.set(12, 34, 56);
    activeHandle.applyPinnedPose(camera);
    expect(camera.position.toArray()).toEqual([12, 34, 56]);

    expect(() => {
      window[CANONICAL_POSE_SETTER_GLOBAL]?.('invalid' as unknown as 'overview');
    }).toThrow(/unknown canonical view/);
  });

  it('returns a safe no-op handle when gl.info or a Scene is absent (jsdom)', () => {
    const handle = installFrameCounters({ render: () => {} }, { traverse: () => {} });
    expect(() => handle.applyPinnedPose({})).not.toThrow();
    expect(() => handle.dispose()).not.toThrow();
    expect(readFrameCounters()).toBeNull();
    expect(setCanonicalPose('overview')).toBe(false);
  });
});
