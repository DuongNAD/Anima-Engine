import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, cleanup, act, fireEvent, screen } from '@testing-library/react';
import * as THREE from 'three';
import type { EnvironmentalElement } from '../../src/types/generated/EnvironmentalElement';
import type { EnvironmentalState } from '../../src/types/generated/EnvironmentalState';
import type { SimulationTickPayload } from '../../src/types/generated/SimulationTickPayload';
import type { FrameCallback } from '../mocks/r3f-frame-state';
import { frameStateAt } from '../mocks/r3f-frame-state';
import { installStubbedProperty, removeStubbedProperties } from '../mocks/r3f-dom-stubs';
import { segment } from '../mocks/segment_fixtures';
import { generateWorld, type World } from '../../src/components/Landscape/utils/worldGen';
import { surfaceHeight } from '../../src/components/Landscape/utils/worldSample';
import { buildAgentPoses } from '../../src/components/Landscape/utils/agentPose';
import {
  DEFAULT_XZ_BOUNDS,
  renderXzToWorldXz,
} from '../../src/components/Landscape/utils/coordinate';
import { simToRender } from '../../src/components/Landscape/utils/liveAgentTransform';
import type { TickListener, TickSource } from '../../src/components/Landscape/utils/tickRecording';
import {
  DECORATION_LEGEND_TOOLTIP,
  LAKE_SURFACE_OFFSET,
  countsFromMapped,
  mapSimulationElements,
  normalizeSimEntityKind,
} from '../../src/components/Landscape/utils/simEnvironment';

const { mockTauriListen, mockTauriInvoke, shadowState, frameCallbacks } = vi.hoisted(() => ({
  mockTauriListen: vi.fn(),
  mockTauriInvoke: vi.fn(),
  shadowState: { enabled: false, needsUpdate: false },
  frameCallbacks: [] as FrameCallback[],
}));

vi.mock('@react-three/fiber', async () => {
  const three = await import('three');
  const mockScene = new three.Scene();
  const mockCamera = new three.PerspectiveCamera();
  const mockGl = {
    setSize: vi.fn(),
    domElement: document.createElement('canvas'),
    render: vi.fn(),
    getContext: () => {
      throw new Error('jsdom has no WebGL context');
    },
    shadowMap: shadowState,
  };
  const rootState = {
    clock: { getElapsedTime: () => 0, elapsedTime: 0 },
    scene: mockScene,
    camera: mockCamera,
    gl: mockGl,
    size: { width: 1024, height: 768 },
    setFrameloop: vi.fn(),
    setDpr: vi.fn(),
  };
  return {
    Canvas: ({ children }: { children?: React.ReactNode }) => (
      <div data-testid="mock-canvas">{children}</div>
    ),
    useFrame: (cb: FrameCallback) => {
      frameCallbacks.push(cb);
    },
    useThree: <T,>(selector?: (state: typeof rootState) => T) =>
      selector ? selector(rootState) : rootState,
    extend: () => {},
  };
});

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => mockTauriListen(...args),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockTauriInvoke(...args),
}));

const { LiveEnvironment } = await import('../../src/components/Landscape/LiveEnvironment');
const { default: WorldShowcase } = await import('../../src/components/Landscape/WorldShowcase');

interface RecordedEnvDraw {
  index: number;
  elements: number[];
  color?: [number, number, number];
}

interface MockInstancedElement extends HTMLElement {
  count?: number;
  visible?: boolean;
  instanceMatrix?: { needsUpdate: boolean };
  instanceColor?: { needsUpdate: boolean };
  _v1Draws?: RecordedEnvDraw[];
}

function getDrawnEnvInstances(el: MockInstancedElement | null): {
  count: number;
  visible: boolean;
  draws: RecordedEnvDraw[];
} {
  if (!el) return { count: 0, visible: false, draws: [] };
  const count = el.count ?? 0;
  const visible = el.visible ?? false;
  const raw = el._v1Draws ?? [];
  return {
    count,
    visible,
    draws: raw.slice(0, count).map((entry) => ({
      index: entry.index,
      elements: entry.elements.slice(),
      color: entry.color ? [entry.color[0], entry.color[1], entry.color[2]] : undefined,
    })),
  };
}

function advanceFrames(): void {
  const state = frameStateAt(0.5);
  for (const cb of frameCallbacks) {
    cb(state, 1 / 60);
  }
}

function createManualTickSource(): {
  source: TickSource;
  emit: (payload: SimulationTickPayload) => void;
} {
  const listeners = new Set<TickListener>();
  return {
    source: {
      kind: 'recording',
      screenLabel: null,
      subscribe(listener) {
        listeners.add(listener);
        return () => {
          listeners.delete(listener);
        };
      },
    },
    emit(payload) {
      listeners.forEach((listener) => {
        listener(payload);
      });
    },
  };
}

describe('V1 — 3D map renders simulation lakes, trees, food and labels decoration', () => {
  const renderSize = 1200;
  const heightRatio = 0.14;
  const meshResolution = 64;
  let world: World;

  beforeEach(() => {
    frameCallbacks.length = 0;
    mockTauriListen.mockReset();
    mockTauriInvoke.mockReset();
    shadowState.enabled = false;
    shadowState.needsUpdate = false;
    window.history.replaceState(null, '', '/landscape.html');
    world = generateWorld('v1-test-world', { size: 32, shape: 'island' });

    installStubbedProperty(
      HTMLElement.prototype,
      'setMatrixAt',
      vi.fn(function (this: MockInstancedElement, index: number, matrix: THREE.Matrix4) {
        if (!this._v1Draws) this._v1Draws = [];
        const existing = this._v1Draws[index];
        this._v1Draws[index] = {
          index,
          elements: Array.from(matrix.elements),
          color: existing?.color,
        };
      }),
    );

    installStubbedProperty(
      HTMLElement.prototype,
      'setColorAt',
      vi.fn(function (this: MockInstancedElement, index: number, color: THREE.Color) {
        if (!this._v1Draws) this._v1Draws = [];
        const existing = this._v1Draws[index] ?? { index, elements: [] };
        existing.color = [color.r, color.g, color.b];
        this._v1Draws[index] = existing;
      }),
    );

    Object.defineProperty(HTMLElement.prototype, 'instanceMatrix', {
      get(this: HTMLElement & { _v1IM?: { needsUpdate: boolean } }) {
        if (!this._v1IM) this._v1IM = { needsUpdate: false };
        return this._v1IM;
      },
      configurable: true,
    });

    Object.defineProperty(HTMLElement.prototype, 'instanceColor', {
      get(this: HTMLElement & { _v1IC?: { needsUpdate: boolean } }) {
        if (!this._v1IC) this._v1IC = { needsUpdate: false };
        return this._v1IC;
      },
      configurable: true,
    });
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    window.history.replaceState(null, '', '/');
    Reflect.deleteProperty(window, '__TAURI_INTERNALS__');
    removeStubbedProperties(
      HTMLElement.prototype,
      'setMatrixAt',
      'setColorAt',
      'instanceMatrix',
      'instanceColor',
    );
  });

  it('maps simulation coordinates through liveAgentTransform/coordinate and aligns with creature grounding', () => {
    expect(normalizeSimEntityKind(' lake ')).toBe('lake');
    expect(normalizeSimEntityKind('TREE')).toBe('tree');
    expect(normalizeSimEntityKind('food')).toBe('food');
    expect(normalizeSimEntityKind('rock')).toBeNull();

    const sampleGround = (rx: number, rz: number): number =>
      surfaceHeight(world, rx, rz, renderSize, heightRatio, meshResolution);

    const elements: EnvironmentalElement[] = [
      { type: 'lake', x: -24, y: 18, radius: 5.5, resources: 95 },
      { type: 'tree', x: 12.5, y: -18.25, radius: 1.8, resources: 80 },
      { type: 'food', x: 12.5, y: -18.25, radius: 0.4, resources: 30 },
      // Invalid / out-of-bounds entries must be ignored cleanly rather than clamped onto the map edge.
      { type: 'lake', x: 120, y: 0, radius: 3, resources: 50 },
      { type: 'tree', x: 0, y: Number.NaN, radius: 1, resources: 10 },
      { type: 'unknown', x: 0, y: 0, radius: 1, resources: 10 },
    ];

    const mapped = mapSimulationElements(elements, renderSize, sampleGround);
    expect(mapped.skipped).toBe(3);
    expect(countsFromMapped(mapped)).toEqual({ lakes: 1, trees: 1, food: 1, total: 3 });

    const lake = mapped.lakes[0];
    expect(lake.renderX).toBeCloseTo(simToRender(-24, renderSize), 6);
    expect(lake.renderZ).toBeCloseTo(simToRender(18, renderSize), 6);
    expect(renderXzToWorldXz(lake.renderX, lake.renderZ, renderSize, DEFAULT_XZ_BOUNDS)).toEqual([
      -24,
      18,
    ]);
    expect(lake.renderY).toBeCloseTo(lake.groundY + LAKE_SURFACE_OFFSET, 6);

    const tree = mapped.trees[0];
    const food = mapped.food[0];
    const [creaturePose] = buildAgentPoses(
      [
        segment({
          agent_id: 7,
          segment_id: 0,
          parent_segment_id: null,
          x: 12.5,
          y: 2.0,
          z: -18.25,
          radius: 0.25,
          length: 1.0,
        }),
      ],
      renderSize,
      sampleGround,
    );

    expect(creaturePose).toBeDefined();
    expect(tree.renderX).toBeCloseTo(creaturePose.rootPosition[0], 6);
    expect(tree.renderZ).toBeCloseTo(creaturePose.rootPosition[2], 6);
    expect(tree.groundY).toBeCloseTo(creaturePose.groundY, 6);
    expect(tree.renderY).toBeCloseTo(creaturePose.groundY, 6);
    expect(food.renderX).toBeCloseTo(creaturePose.rootPosition[0], 6);
    expect(food.renderZ).toBeCloseTo(creaturePose.rootPosition[2], 6);
    expect(food.groundY).toBeCloseTo(creaturePose.groundY, 6);
  });

  it('renders one instanced mesh element per lake, tree, and food in a synthetic tick at its mapped position', async () => {
    const manual = createManualTickSource();
    const countsUpdates: Array<{ lakes: number; trees: number; food: number; total: number }> = [];

    const { container } = render(
      <LiveEnvironment
        world={world}
        renderSize={renderSize}
        heightRatio={heightRatio}
        meshResolution={meshResolution}
        tickSource={manual.source}
        fetchInitialElements={null}
        onCountsChange={(counts) => {
          countsUpdates.push(counts);
        }}
      />,
    );

    expect(container.querySelector('group[name="sim-environment"]')).toBeNull();

    const syntheticEnv: EnvironmentalElement[] = [
      { type: 'lake', x: -30, y: -12, radius: 4.5, resources: 100 },
      { type: 'lake', x: 22, y: 34, radius: 3.2, resources: 60 },
      { type: 'tree', x: -8, y: 14, radius: 1.4, resources: 85 },
      { type: 'tree', x: 16, y: -24, radius: 1.6, resources: 40 },
      { type: 'tree', x: 42, y: 9, radius: 1.1, resources: 10 },
      { type: 'food', x: -4, y: -4, radius: 0.35, resources: 25 },
      { type: 'food', x: 31, y: -19, radius: 0.5, resources: 45 },
    ];

    await act(async () => {
      manual.emit({
        segments: [],
        environmental_state: { elements: syntheticEnv },
        head_directions: {},
      });
    });

    act(() => {
      advanceFrames();
    });

    const lakeMesh = container.querySelector(
      'group[name="sim-environment"] instancedmesh[name="sim-env-lakes"]',
    ) as MockInstancedElement | null;
    const treeMesh = container.querySelector(
      'group[name="sim-environment"] instancedmesh[name="sim-env-trees"]',
    ) as MockInstancedElement | null;
    const foodMesh = container.querySelector(
      'group[name="sim-environment"] instancedmesh[name="sim-env-food"]',
    ) as MockInstancedElement | null;

    expect(lakeMesh).not.toBeNull();
    expect(treeMesh).not.toBeNull();
    expect(foodMesh).not.toBeNull();

    const lakesDrawn = getDrawnEnvInstances(lakeMesh);
    const treesDrawn = getDrawnEnvInstances(treeMesh);
    const foodDrawn = getDrawnEnvInstances(foodMesh);

    expect(lakesDrawn.count).toBe(2);
    expect(lakesDrawn.visible).toBe(true);
    expect(treesDrawn.count).toBe(3);
    expect(treesDrawn.visible).toBe(true);
    expect(foodDrawn.count).toBe(2);
    expect(foodDrawn.visible).toBe(true);
    expect(countsUpdates[countsUpdates.length - 1]).toEqual({
      lakes: 2,
      trees: 3,
      food: 2,
      total: 7,
    });

    const expected = mapSimulationElements(syntheticEnv, renderSize, (rx, rz) =>
      surfaceHeight(world, rx, rz, renderSize, heightRatio, meshResolution),
    );

    expected.lakes.forEach((item, idx) => {
      const m = lakesDrawn.draws[idx].elements;
      expect(m[12]).toBeCloseTo(item.renderX, 5);
      expect(m[13]).toBeCloseTo(item.renderY, 5);
      expect(m[14]).toBeCloseTo(item.renderZ, 5);
      expect(m[0]).toBeCloseTo(item.renderRadius, 5);
    });
    expected.trees.forEach((item, idx) => {
      const m = treesDrawn.draws[idx].elements;
      expect(m[12]).toBeCloseTo(item.renderX, 5);
      expect(m[13]).toBeCloseTo(item.renderY, 5);
      expect(m[14]).toBeCloseTo(item.renderZ, 5);
      expect(m[0]).toBeCloseTo(item.renderRadius, 5);
      expect(m[5]).toBeCloseTo(item.heightScale, 5);
    });
    expected.food.forEach((item, idx) => {
      const m = foodDrawn.draws[idx].elements;
      expect(m[12]).toBeCloseTo(item.renderX, 5);
      expect(m[13]).toBeCloseTo(item.renderY, 5);
      expect(m[14]).toBeCloseTo(item.renderZ, 5);
      expect(m[0]).toBeCloseTo(item.renderRadius, 5);
    });

    // Subsequent tick with 1 food consumed updates instance count immediately.
    await act(async () => {
      manual.emit({
        segments: [],
        environmental_state: { elements: syntheticEnv.slice(0, 6) },
        head_directions: {},
      });
    });
    act(() => {
      advanceFrames();
    });

    expect(getDrawnEnvInstances(foodMesh).count).toBe(1);
    expect(countsUpdates[countsUpdates.length - 1]).toEqual({
      lakes: 2,
      trees: 3,
      food: 1,
      total: 6,
    });
  });

  it('fetches initial elements from get_environmental_elements in Tauri and skips IPC during capture mode', async () => {
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      value: {},
      configurable: true,
    });

    const initialState: EnvironmentalState = {
      elements: [
        { type: 'lake', x: -10, y: 10, radius: 4, resources: 75 },
        { type: 'tree', x: 15, y: -5, radius: 1.5, resources: 90 },
      ],
    };
    mockTauriInvoke.mockResolvedValueOnce(initialState);
    mockTauriListen.mockResolvedValue(async () => {});

    const liveView = render(
      <LiveEnvironment
        world={world}
        renderSize={renderSize}
        heightRatio={heightRatio}
        meshResolution={meshResolution}
      />,
    );

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });
    act(() => {
      advanceFrames();
    });

    expect(mockTauriInvoke).toHaveBeenCalledWith('get_environmental_elements');
    expect(
      (
        liveView.container.querySelector(
          'instancedmesh[name="sim-env-lakes"]',
        ) as MockInstancedElement | null
      )?.count,
    ).toBe(1);
    expect(
      (
        liveView.container.querySelector(
          'instancedmesh[name="sim-env-trees"]',
        ) as MockInstancedElement | null
      )?.count,
    ).toBe(1);

    liveView.unmount();
    mockTauriInvoke.mockReset();
    mockTauriListen.mockReset();

    // Canonical capture mode skips both IPC and simulation-environment meshes.
    window.history.replaceState(null, '', '/landscape.html?capture=1&view=overview');
    const captureView = render(
      <LiveEnvironment
        world={world}
        renderSize={renderSize}
        heightRatio={heightRatio}
        meshResolution={meshResolution}
      />,
    );

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mockTauriInvoke).not.toHaveBeenCalled();
    expect(mockTauriListen).not.toHaveBeenCalled();
    expect(captureView.container.querySelector('group[name="sim-environment"]')).toBeNull();
  });

  it('labels WorldFish/WorldBirds/WorldWildlife/WorldCaves as decoration without removing them, with a user toggle', async () => {
    const view = render(<WorldShowcase />);

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    const legend = screen.getByTestId('decoration-legend');
    expect(legend.textContent?.toLowerCase()).toContain('decoration, not simulated');
    expect(legend.textContent?.toLowerCase()).toContain('fish');
    expect(legend.textContent?.toLowerCase()).toContain('birds');
    expect(legend.textContent?.toLowerCase()).toContain('wildlife');
    expect(legend.textContent?.toLowerCase()).toContain('caves');
    expect(legend.getAttribute('title')).toBe(DECORATION_LEGEND_TOOLTIP);

    expect(screen.getByTestId('sim-env-legend')).toBeTruthy();

    expect(view.container.querySelector('[name="world-birds"]')).not.toBeNull();
    expect(view.container.querySelector('[name="world-fish"]')).not.toBeNull();
    expect(view.container.querySelector('[name="world-wildlife"]')).not.toBeNull();
    expect(view.container.querySelector('[name="world-caves"]')).not.toBeNull();

    const toggleBtn = screen.getByTestId('toggle-decoration-btn');
    expect(toggleBtn.getAttribute('aria-pressed')).toBe('true');

    act(() => {
      fireEvent.click(toggleBtn);
    });

    expect(toggleBtn.getAttribute('aria-pressed')).toBe('false');
    expect(view.container.querySelector('[name="world-birds"]')).toBeNull();
    expect(view.container.querySelector('[name="world-fish"]')).toBeNull();
    expect(view.container.querySelector('[name="world-wildlife"]')).toBeNull();
    expect(view.container.querySelector('[name="world-caves"]')).toBeNull();

    act(() => {
      fireEvent.click(toggleBtn);
    });

    expect(toggleBtn.getAttribute('aria-pressed')).toBe('true');
    expect(view.container.querySelector('[name="world-birds"]')).not.toBeNull();

    // Capture mode hides HUD overlays while keeping decoration layers intact.
    view.unmount();
    window.history.replaceState(null, '', '/landscape.html?capture=1&view=overview');
    const captureView = render(<WorldShowcase />);
    expect(screen.queryByTestId('decoration-legend')).toBeNull();
    expect(captureView.container.querySelector('[name="world-birds"]')).not.toBeNull();
    expect(captureView.container.querySelector('[name="world-wildlife"]')).not.toBeNull();
  });
});
