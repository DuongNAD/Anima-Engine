import React from 'react';
import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, cleanup, act } from '@testing-library/react';
import * as THREE from 'three';
import type { SimulationTickPayload } from '../../src/types/generated/SimulationTickPayload';
import type { FrameCallback } from '../mocks/r3f-frame-state';
import { makeFrameState } from '../mocks/r3f-frame-state';
import { installStubbedProperty, removeStubbedProperties } from '../mocks/r3f-dom-stubs';
import { segments } from '../mocks/segment_fixtures';
import { generateWorld, type World } from '../../src/components/Landscape/utils/worldGen';
import {
  CREATURE_BODY_SCALE,
  buildAgentPoses,
  computeCreatureColor,
  computeFarMarkerScale,
  eulerYXZToQuaternion,
  quaternionToEulerYXZ,
} from '../../src/components/Landscape/utils/agentPose';
import {
  createReplayTickSource,
  createSyntheticTickSource,
} from '../../src/components/Landscape/utils/tickRecording';

const __dir = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dir, '../..');

const { frameCallbacks, mockGraphicsMethods } = vi.hoisted(() => ({
  frameCallbacks: [] as FrameCallback[],
  mockGraphicsMethods: {
    clear: vi.fn().mockReturnThis(),
    beginFill: vi.fn().mockReturnThis(),
    drawCircle: vi.fn().mockReturnThis(),
    drawPolygon: vi.fn().mockReturnThis(),
    endFill: vi.fn().mockReturnThis(),
    lineStyle: vi.fn().mockReturnThis(),
    moveTo: vi.fn().mockReturnThis(),
    lineTo: vi.fn().mockReturnThis(),
    drawRect: vi.fn().mockReturnThis(),
  },
}));

vi.mock('@react-three/fiber', async () => {
  return {
    Canvas: ({ children }: { children?: React.ReactNode }) => (
      <div data-testid="mock-canvas">{children}</div>
    ),
    useFrame: (cb: FrameCallback) => {
      frameCallbacks.push(cb);
    },
  };
});

vi.mock('pixi.js', () => {
  return {
    Application: vi.fn(function () {
      return {
        init: vi.fn().mockResolvedValue(undefined),
        canvas: document.createElement('canvas'),
        stage: { addChild: vi.fn(), removeChild: vi.fn() },
        renderer: {},
        ticker: { add: vi.fn(), remove: vi.fn() },
        destroy: vi.fn(),
      };
    }),
    Graphics: vi.fn(function () {
      return mockGraphicsMethods;
    }),
    Container: vi.fn(function () {
      return { addChild: vi.fn(), removeChild: vi.fn() };
    }),
  };
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(null),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const { LiveAgents } = await import('../../src/components/Landscape/LiveAgents');
const { default: PixiViewport } = await import('../../src/PixiViewport');

interface RecordedInstance {
  elements: number[];
  color?: [number, number, number];
}

interface MockInstancedEl extends HTMLElement {
  count?: number;
  visible?: boolean;
  instanceMatrix?: { needsUpdate: boolean };
  instanceColor?: { needsUpdate: boolean };
  _rwDraws?: RecordedInstance[];
}

function advanceThreeFrame(overrides: Parameters<typeof makeFrameState>[0] = {}): void {
  const state = makeFrameState(overrides);
  for (const cb of frameCallbacks) {
    cb(state, 1 / 60);
  }
}

/**
 * Position camera in mid-LOD band (`160 < d <= 360` on medium tier) looking at origin so B1's
 * near SkinnedMesh tier (`<= 160` units) does not empty `live-agents-segments` in instanced checks.
 */
function makeMidLodCamera(): THREE.PerspectiveCamera {
  const cam = new THREE.PerspectiveCamera(90, 16 / 9, 0.5, 2000);
  cam.position.set(0, 120, 240);
  cam.lookAt(0, 60, 0);
  cam.updateMatrixWorld();
  cam.updateProjectionMatrix();
  return cam;
}

function activeInstancedMeshes(container: HTMLElement): MockInstancedEl[] {
  return Array.from(container.querySelectorAll('instancedmesh')).filter((el) => {
    const mock = el as MockInstancedEl;
    return mock.visible !== false && (mock.count ?? 0) > 0;
  }) as MockInstancedEl[];
}

describe('RW-A2 articulated creatures, 2D species/size parity, shadows, markers, and selection', () => {
  let world: World;

  beforeEach(() => {
    frameCallbacks.length = 0;
    vi.clearAllMocks();
    world = generateWorld('rw-a2-test-world', { size: 32, shape: 'island' });

    installStubbedProperty(
      HTMLElement.prototype,
      'setMatrixAt',
      vi.fn(function (this: MockInstancedEl, index: number, matrix: THREE.Matrix4) {
        if (!this._rwDraws) this._rwDraws = [];
        const existing = this._rwDraws[index];
        this._rwDraws[index] = {
          elements: Array.from(matrix.elements),
          color: existing?.color,
        };
      }),
    );

    installStubbedProperty(
      HTMLElement.prototype,
      'setColorAt',
      vi.fn(function (this: MockInstancedEl, index: number, color: THREE.Color) {
        if (!this._rwDraws) this._rwDraws = [];
        const existing = this._rwDraws[index] ?? { elements: [] };
        existing.color = [color.r, color.g, color.b];
        this._rwDraws[index] = existing;
      }),
    );

    Object.defineProperty(HTMLElement.prototype, 'instanceMatrix', {
      get(this: HTMLElement & { _rwIM?: { needsUpdate: boolean } }) {
        if (!this._rwIM) this._rwIM = { needsUpdate: false };
        return this._rwIM;
      },
      configurable: true,
    });

    Object.defineProperty(HTMLElement.prototype, 'instanceColor', {
      get(this: HTMLElement & { _rwIC?: { needsUpdate: boolean } }) {
        if (!this._rwIC) this._rwIC = { needsUpdate: false };
        return this._rwIC;
      },
      configurable: true,
    });
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
    window.history.replaceState(null, '', '/');
    removeStubbedProperties(
      HTMLElement.prototype,
      'setMatrixAt',
      'setColorAt',
      'instanceMatrix',
      'instanceColor',
    );
    document.querySelectorAll('[data-testid^="rw-"]').forEach((node) => node.remove());
  });

  it('RW1-4: PixiViewport draws N-1 body strokes, 1 species head, and 1 water bar per creature', async () => {
    // 4-segment prey + 3-segment predator
    const testSegs = segments(
      { agent_id: 10, segment_id: 0, parent_segment_id: null, x: -20, z: 0, energy: 80, hydration: 75, head_direction: [1, 0, 0], agent_type: 'prey' },
      { agent_id: 10, segment_id: 1, parent_segment_id: 0, x: -21, z: 0, energy: 80, hydration: 75, agent_type: 'prey' },
      { agent_id: 10, segment_id: 2, parent_segment_id: 1, x: -22, z: 0, energy: 80, hydration: 75, agent_type: 'prey' },
      { agent_id: 10, segment_id: 3, parent_segment_id: 2, x: -23, z: 0, energy: 80, hydration: 75, agent_type: 'prey' },
      { agent_id: 20, segment_id: 0, parent_segment_id: null, x: 20, z: 10, energy: 90, hydration: 60, head_direction: [0, 0, 1], agent_type: 'predator' },
      { agent_id: 20, segment_id: 1, parent_segment_id: 0, x: 20, z: 9, energy: 90, hydration: 60, agent_type: 'predator' },
      { agent_id: 20, segment_id: 2, parent_segment_id: 1, x: 20, z: 8, energy: 90, hydration: 60, agent_type: 'predator' },
    );

    const { rerender } = render(
      <PixiViewport segments={testSegs} raycasts={[]} pheromoneGrid={null} projection="xz" />,
    );
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 40));
    });

    // Clear initial mount/grid calls and trigger an isolated redraw.
    Object.values(mockGraphicsMethods).forEach((fn) => fn.mockClear());
    rerender(
      <PixiViewport
        segments={[...testSegs]}
        raycasts={[]}
        pheromoneGrid={null}
        projection="xz"
      />,
    );

    // 1 prey head circle (minimap root circles are smaller radius === 2.5)
    const preyHeadCircles = mockGraphicsMethods.drawCircle.mock.calls.filter(
      (call) => Math.abs((call[2] as number) - 10) < 0.01,
    );
    expect(preyHeadCircles).toHaveLength(1);

    // 1 predator head polygon
    expect(mockGraphicsMethods.drawPolygon).toHaveBeenCalledTimes(1);

    // Prey (0x5ad67d) and predator (0xe2554f) use distinct colors AND shapes.
    const fillColors = mockGraphicsMethods.beginFill.mock.calls.map((c) => c[0]);
    expect(fillColors).toContain(0x5ad67d);
    expect(fillColors).toContain(0xe2554f);
    expect(0x5ad67d).not.toBe(0xe2554f);

    // Body segment strokes: (4 - 1) prey + (3 - 1) predator = 5 green/red body strokes
    const preyBodyStrokes = mockGraphicsMethods.lineStyle.mock.calls.filter(
      (call) => call[1] === 0x43b566,
    );
    const predatorBodyStrokes = mockGraphicsMethods.lineStyle.mock.calls.filter(
      (call) => call[1] === 0xc8433d,
    );
    expect(preyBodyStrokes).toHaveLength(3);
    expect(predatorBodyStrokes).toHaveLength(2);

    // Hydration bars: 1 backing + 1 fill = 2 rects per creature -> 4 creature water bar rects
    const waterFills = mockGraphicsMethods.beginFill.mock.calls.filter((c) => c[0] === 0x38bdf8);
    expect(waterFills).toHaveLength(2);
  });

  it('RW2-8 (A1-5): PixiViewport scales 2D head and body segment geometry from length and radius', async () => {
    const testSegs = segments(
      { agent_id: 1, segment_id: 0, parent_segment_id: null, length: 1.0, radius: 0.25, x: -30, z: 0, agent_type: 'prey' },
      { agent_id: 1, segment_id: 1, parent_segment_id: 0, length: 1.0, radius: 0.25, x: -31, z: 0, agent_type: 'prey' },
      { agent_id: 2, segment_id: 0, parent_segment_id: null, length: 2.5, radius: 0.5, x: 30, z: 0, agent_type: 'prey' },
      { agent_id: 2, segment_id: 1, parent_segment_id: 0, length: 2.5, radius: 0.5, x: 27.5, z: 0, agent_type: 'prey' },
    );

    const { rerender } = render(
      <PixiViewport segments={testSegs} raycasts={[]} pheromoneGrid={null} projection="xz" />,
    );
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 40));
    });

    Object.values(mockGraphicsMethods).forEach((fn) => fn.mockClear());
    rerender(
      <PixiViewport
        segments={[...testSegs]}
        raycasts={[]}
        pheromoneGrid={null}
        projection="xz"
      />,
    );

    const headRadii = mockGraphicsMethods.drawCircle.mock.calls
      .map((c) => c[2] as number)
      .filter((r) => r > 5)
      .sort((a, b) => a - b);
    expect(headRadii).toEqual([10, 20]);

    const bodyWidths = mockGraphicsMethods.lineStyle.mock.calls
      .filter((c) => c[1] === 0x43b566)
      .map((c) => c[0] as number)
      .sort((a, b) => a - b);
    expect(bodyWidths).toEqual([3.5, 7.0]);
  });

  it('RW2-1: round-trips YXZ Euler angles across 3 triples and places 3 collinear segments spaced by exact length', () => {
    const triples: Array<[number, number, number]> = [
      [0.65, -0.32, 0.41],
      [-2.15, 0.48, -0.73],
      [1.28, 0.0, 1.05],
    ];
    for (const [yaw, pitch, roll] of triples) {
      const q = eulerYXZToQuaternion(yaw, pitch, roll);
      const out = quaternionToEulerYXZ(q);
      expect(out.yaw).toBeCloseTo(yaw, 6);
      expect(out.pitch).toBeCloseTo(pitch, 6);
      expect(out.roll).toBeCloseTo(roll, 6);
    }

    const L = 1.4;
    const R = 0.28;
    const collinear = segments(
      { agent_id: 7, segment_id: 0, parent_segment_id: null, length: L, radius: R, x: 0, y: 2.0, z: 0 },
      { agent_id: 7, segment_id: 1, parent_segment_id: 0, length: L, radius: R, x: 0, y: 2.3, z: -L },
      { agent_id: 7, segment_id: 2, parent_segment_id: 1, length: L, radius: R, x: 0, y: 2.6, z: -2 * L },
    );

    // At renderSize = 1200, simScale = 6, so spacing along Z is exact L * 6 = 8.4 and deltaY = 0.3 * 6 = 1.8.
    const poses = buildAgentPoses(collinear, 1200, () => 12.0);
    expect(poses).toHaveLength(1);
    const segPoses = poses[0].segments;
    expect(segPoses).toHaveLength(3);

    const p0 = new THREE.Vector3(
      segPoses[0].matrixElements[12],
      segPoses[0].matrixElements[13],
      segPoses[0].matrixElements[14],
    );
    const p1 = new THREE.Vector3(
      segPoses[1].matrixElements[12],
      segPoses[1].matrixElements[13],
      segPoses[1].matrixElements[14],
    );
    const p2 = new THREE.Vector3(
      segPoses[2].matrixElements[12],
      segPoses[2].matrixElements[13],
      segPoses[2].matrixElements[14],
    );

    expect(p1.z - p0.z).toBeCloseTo(-L * 6, 6);
    expect(p2.z - p1.z).toBeCloseTo(-L * 6, 6);
    expect(p1.y - p0.y).toBeCloseTo(0.3 * 6, 6);
    expect(p2.y - p1.y).toBeCloseTo(0.3 * 6, 6);

    // At renderSize = 200, simScale = 1, spacing equals raw L literally.
    const unitPoses = buildAgentPoses(collinear, 200, () => 0);
    expect(unitPoses[0].segments[0].matrixElements[14] - unitPoses[0].segments[1].matrixElements[14]).toBeCloseTo(L, 6);
  });

  it('RW2-2 (A1-4): removes SEGMENT_RADIUS, varies instance matrices by radius, and keeps draw calls up to 5 for 10 and 1000 creatures', () => {
    const liveAgentsSource = readFileSync(resolve(ROOT, 'src/components/Landscape/LiveAgents.tsx'), 'utf8');
    expect(liveAgentsSource).not.toContain('SEGMENT_RADIUS');

    const payloadDifferentRadius: SimulationTickPayload = {
      segments: segments(
        { agent_id: 1, segment_id: 0, parent_segment_id: null, length: 1.0, radius: 0.2, x: -5, z: 0, agent_type: 'prey', energy: 80 },
        { agent_id: 2, segment_id: 0, parent_segment_id: null, length: 1.0, radius: 0.45, x: 5, z: 0, agent_type: 'prey', energy: 80 },
      ),
      environmental_state: { elements: [] },
      head_directions: {},
    };
    const radiusSource = createReplayTickSource([payloadDifferentRadius], { loop: false });
    const radiusView = render(
      <LiveAgents world={world} renderSize={1200} heightRatio={0.14} meshResolution={64} tickSource={radiusSource} />,
    );
    advanceThreeFrame({ camera: makeMidLodCamera() });

    const segMesh = radiusView.container.querySelector('instancedmesh[name="live-agents-segments"]') as MockInstancedEl;
    expect(segMesh.count).toBe(2);
    const mat0 = segMesh._rwDraws![0].elements;
    const mat1 = segMesh._rwDraws![1].elements;
    // Scale X / Y encode renderRadius calibrated by CREATURE_BODY_SCALE (0.2/0.45 * 6 * CREATURE_BODY_SCALE).
    expect(mat0[0]).toBeCloseTo(1.2 * CREATURE_BODY_SCALE, 5);
    expect(mat1[0]).toBeCloseTo(2.7 * CREATURE_BODY_SCALE, 5);
    expect(mat0).not.toEqual(mat1);
    radiusView.unmount();

    // 10 synthetic creatures
    frameCallbacks.length = 0;
    const source10 = createSyntheticTickSource({ agents: 10, ticks: 2, segmentsPerAgent: 3 });
    const view10 = render(
      <LiveAgents world={world} renderSize={1200} heightRatio={0.14} meshResolution={64} tickSource={source10} />,
    );
    advanceThreeFrame();
    const active10 = activeInstancedMeshes(view10.container);
    expect(active10.length > 0 && active10.length < 6).toBe(true);
    view10.unmount();

    // 1000 synthetic creatures (first render stays within initial 4096 capacity when segments = 3000)
    frameCallbacks.length = 0;
    const source1000 = createSyntheticTickSource({ agents: 1000, ticks: 2, segmentsPerAgent: 3 });
    const view1000 = render(
      <LiveAgents world={world} renderSize={1200} heightRatio={0.14} meshResolution={64} tickSource={source1000} />,
    );
    advanceThreeFrame();
    const active1000 = activeInstancedMeshes(view1000.container);
    expect(active1000.length > 0 && active1000.length < 6).toBe(true);
    view1000.unmount();
  });

  it('RW2-3: gives predators a pointed snout cone + 2 eyes and prey a rounded head sphere + 2 eyes aligned with head_direction', () => {
    const payload: SimulationTickPayload = {
      segments: segments(
        { agent_id: 1, segment_id: 0, parent_segment_id: null, length: 1.0, radius: 0.25, x: 0, z: 0, head_direction: [1, 0, 0], agent_type: 'predator', energy: 90 },
        { agent_id: 2, segment_id: 0, parent_segment_id: null, length: 1.0, radius: 0.25, x: 10, z: 0, head_direction: [0, 0, 1], agent_type: 'prey', energy: 90 },
      ),
      environmental_state: { elements: [] },
      head_directions: { 1: [1, 0, 0], 2: [0, 0, 1] },
    };
    const source = createReplayTickSource([payload], { loop: false });
    const { container } = render(
      <LiveAgents world={world} renderSize={1200} heightRatio={0.14} meshResolution={64} tickSource={source} />,
    );
    advanceThreeFrame({ camera: makeMidLodCamera() });

    const snoutMesh = container.querySelector('instancedmesh[name="live-agents-snouts"]') as MockInstancedEl;
    const sphereMesh = container.querySelector('instancedmesh[name="live-agents-spheres"]') as MockInstancedEl;

    // 1 predator -> 1 snout cone + 2 eyes; 1 prey -> 0 snouts + 1 head cap + 2 eyes => 5 spheres total.
    expect(snoutMesh.count).toBe(1);
    expect(sphereMesh.count).toBe(5);

    // Predator snout center projects along +X (`x > 0`, `z ≈ 0`), scaled by CREATURE_BODY_SCALE.
    const snoutMat = snoutMesh._rwDraws![0].elements;
    expect(snoutMat[12] > 2 * CREATURE_BODY_SCALE).toBe(true);
    expect(Math.abs(snoutMat[14]) < 0.01).toBe(true);

    // Predator's two eyes sit symmetrically in Z (`z = ±eyeSide`) looking along +X.
    const eye0Z = sphereMesh._rwDraws![0].elements[14];
    const eye1Z = sphereMesh._rwDraws![1].elements[14];
    expect(eye0Z + eye1Z).toBeCloseTo(0, 5);
    expect(Math.abs(eye0Z) > 0.5 * CREATURE_BODY_SCALE).toBe(true);
  });

  it('RW2-4: hashes agent_id into non-colliding predator/prey colors and preserves energy dimming', () => {
    const seenPrey = new Set<string>();
    const seenPred = new Set<string>();
    for (let id = 1; id < 250; id++) {
      const preyA = computeCreatureColor('prey', id, 80);
      const preyB = computeCreatureColor('prey', id, 80);
      expect(preyA.getHex()).toBe(preyB.getHex());

      const pred = computeCreatureColor('predator', id, 80);
      seenPrey.add(`${preyA.r.toFixed(2)},${preyA.g.toFixed(2)},${preyA.b.toFixed(2)}`);
      seenPred.add(`${pred.r.toFixed(2)},${pred.g.toFixed(2)},${pred.b.toFixed(2)}`);
      // Prey green channel exceeds red; predator red channel exceeds green.
      expect(preyA.g > preyA.r).toBe(true);
      expect(pred.r > pred.g).toBe(true);
    }
    expect(seenPrey.size > 5).toBe(true);
    expect(seenPred.size > 5).toBe(true);
    for (const k of seenPrey) {
      expect(seenPred.has(k)).toBe(false);
    }

    const fullEnergy = computeCreatureColor('prey', 42, 100);
    const zeroEnergy = computeCreatureColor('prey', 42, 0);
    expect(zeroEnergy.r / fullEnergy.r).toBeCloseTo(0.45, 5);
  });

  it('RW2-5: draws fake ground-contact shadow discs only when real shadowMap is disabled', () => {
    const payload: SimulationTickPayload = {
      segments: segments(
        { agent_id: 1, segment_id: 0, parent_segment_id: null, x: -10, z: -10, agent_type: 'prey', energy: 90 },
        { agent_id: 2, segment_id: 0, parent_segment_id: null, x: 10, z: 10, agent_type: 'predator', energy: 90 },
      ),
      environmental_state: { elements: [] },
      head_directions: {},
    };
    const source = createReplayTickSource([payload], { loop: false });
    const { container } = render(
      <LiveAgents world={world} renderSize={1200} heightRatio={0.14} meshResolution={64} tickSource={source} />,
    );

    const shadowMesh = container.querySelector('instancedmesh[name="live-agents-shadows"]') as MockInstancedEl;

    // Low quality (shadowMap disabled) -> 2 contact shadows, visible=true
    advanceThreeFrame();
    expect(shadowMesh.count).toBe(2);
    expect(shadowMesh.visible).toBe(true);

    // High quality (shadowMap enabled) -> 0 fake shadows, visible=false (0 extra draw calls)
    const highState = makeFrameState();
    highState.gl.shadowMap.enabled = true;
    for (const cb of frameCallbacks) cb(highState, 1 / 60);
    expect(shadowMesh.count).toBe(0);
    expect(shadowMesh.visible).toBe(false);
  });

  it('RW2-6: renders all far overview creatures as flat at-least-6px species-coded markers', () => {
    const source = createSyntheticTickSource({ agents: 24, ticks: 2, segmentsPerAgent: 3 });
    const { container } = render(
      <LiveAgents world={world} renderSize={1200} heightRatio={0.14} meshResolution={64} tickSource={source} />,
    );

    const overviewCam = new THREE.PerspectiveCamera(55, 16 / 9, 0.1, 3000);
    overviewCam.position.set(0, 672, 672);
    overviewCam.lookAt(0, 0, 0);
    overviewCam.updateMatrixWorld();

    advanceThreeFrame({ camera: overviewCam, size: { width: 1024, height: 768 } });

    const segMesh = container.querySelector('instancedmesh[name="live-agents-segments"]') as MockInstancedEl;
    const markerMesh = container.querySelector('instancedmesh[name="live-agents-markers"]') as MockInstancedEl;
    expect(segMesh.count).toBe(0);
    expect(segMesh.visible).toBe(false);
    expect(markerMesh.count).toBe(24);
    expect(markerMesh.visible).toBe(true);

    const markerWorldScale = computeFarMarkerScale(950, 768, 55, 7.5);
    const pxOnScreen = (markerWorldScale * 768) / (2 * 950 * Math.tan((55 * Math.PI) / 360));
    expect(pxOnScreen > 6.0).toBe(true);
  });

  it('RW2-7: clicking an instance selects the creature, shows ring + payload-only HUD, and close clears it', () => {
    const payload: SimulationTickPayload = {
      segments: segments(
        { agent_id: 77, segment_id: 0, parent_segment_id: null, x: 5, z: -5, energy: 84.5, hydration: 61.2, agent_type: 'predator' },
        { agent_id: 77, segment_id: 1, parent_segment_id: 0, x: 4, z: -5, energy: 84.5, hydration: 61.2, agent_type: 'predator' },
        { agent_id: 77, segment_id: 2, parent_segment_id: 1, x: 3, z: -5, energy: 84.5, hydration: 61.2, agent_type: 'predator' },
      ),
      environmental_state: { elements: [] },
      head_directions: { 77: [1, 0, 0] },
    };
    const source = createReplayTickSource([payload], { loop: false });
    const { container } = render(
      <LiveAgents world={world} renderSize={1200} heightRatio={0.14} meshResolution={64} tickSource={source} />,
    );
    advanceThreeFrame({ camera: makeMidLodCamera() });

    const segMesh = container.querySelector('instancedmesh[name="live-agents-segments"]') as MockInstancedEl;
    expect(segMesh.count).toBe(3);

    // Click instance 1 (segment 1 belonging to agent 77) via React's R3F-style onClick handler.
    const reactPropsEntry = Object.entries(segMesh).find(([k]) => k.startsWith('__reactProps$'));
    const clickHandler = (
      reactPropsEntry?.[1] as { onClick?: (e: { instanceId: number; stopPropagation: () => void }) => void } | undefined
    )?.onClick;
    expect(clickHandler).toBeDefined();
    const stopPropagation = vi.fn();
    act(() => {
      clickHandler?.({ instanceId: 1, stopPropagation });
    });
    expect(stopPropagation).toHaveBeenCalledTimes(1);

    const hud = document.querySelector('[data-testid="rw-selected-agent-hud"]');
    expect(hud).not.toBeNull();
    expect(document.querySelector('[data-testid="rw-selected-agent-id"]')?.textContent).toBe('77');
    expect(document.querySelector('[data-testid="rw-selected-agent-type"]')?.textContent).toBe('predator');
    expect(document.querySelector('[data-testid="rw-selected-agent-energy"]')?.textContent).toBe('84.5');
    expect(document.querySelector('[data-testid="rw-selected-agent-hydration"]')?.textContent).toBe('61.2');
    expect(document.querySelector('[data-testid="rw-selected-agent-segments"]')?.textContent).toBe('3');
    expect(container.querySelector('mesh[name="live-agents-selection-ring"]')).not.toBeNull();

    const closeBtn = document.querySelector('[data-testid="rw-selected-agent-close"]') as HTMLButtonElement;
    act(() => {
      closeBtn.click();
    });
    expect(document.querySelector('[data-testid="rw-selected-agent-hud"]')).toBeNull();
    expect(container.querySelector('mesh[name="live-agents-selection-ring"]')).toBeNull();
  });
});
