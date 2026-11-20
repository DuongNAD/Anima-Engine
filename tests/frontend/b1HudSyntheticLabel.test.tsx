import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach, beforeAll } from 'vitest';
import { warmSharedWorld, WARM_SHARED_WORLD_BUDGET_MS } from './helpers/warmSharedWorld';
import { render, cleanup, act, waitFor } from '@testing-library/react';
import * as THREE from 'three';
import type { SimulationTickPayload } from '../../src/types/generated/SimulationTickPayload';
import type { FrameCallback } from '../mocks/r3f-frame-state';
import { installStubbedProperty, removeStubbedProperties } from '../mocks/r3f-dom-stubs';
import { segment } from '../mocks/segment_fixtures';

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

const { default: WorldShowcase } = await import('../../src/components/Landscape/WorldShowcase');

interface MockInstancedEl extends HTMLElement {
  count?: number;
  visible?: boolean;
  instanceMatrix?: { needsUpdate: boolean };
  instanceColor?: { needsUpdate: boolean };
}

describe('F1 — WorldShowcase simulated-entities HUD label distinguishes synthetic vs real ticks', () => {
  beforeAll(warmSharedWorld, WARM_SHARED_WORLD_BUDGET_MS);
  beforeEach(() => {
    frameCallbacks.length = 0;
    mockTauriListen.mockReset();
    mockTauriInvoke.mockReset();
    shadowState.enabled = false;
    shadowState.needsUpdate = false;
    window.history.replaceState(null, '', '/landscape.html');

    installStubbedProperty(HTMLElement.prototype, 'setMatrixAt', vi.fn());
    installStubbedProperty(HTMLElement.prototype, 'setColorAt', vi.fn());
    Object.defineProperty(HTMLElement.prototype, 'instanceMatrix', {
      get(this: HTMLElement & { _f1IM?: { needsUpdate: boolean } }) {
        if (!this._f1IM) this._f1IM = { needsUpdate: false };
        return this._f1IM;
      },
      configurable: true,
    });
    Object.defineProperty(HTMLElement.prototype, 'instanceColor', {
      get(this: HTMLElement & { _f1IC?: { needsUpdate: boolean } }) {
        if (!this._f1IC) this._f1IC = { needsUpdate: false };
        return this._f1IC;
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
    document.querySelectorAll('[data-testid^="rw-"]').forEach((node) => node.remove());
  });

  it('labels ?replay=synthetic env counts as Synthetic fixture (not simulation output) and real Tauri ticks as Simulated', async () => {
    // 1. Synthetic replay source: HUD must NEVER label fixture elements as "Simulated".
    window.history.replaceState(null, '', '/landscape.html?replay=synthetic&agents=4&segments=3');
    const synthView = render(<WorldShowcase />);

    await waitFor(() => {
      const el = synthView.getByTestId('sim-env-legend');
      expect(el.textContent).toBe(
        'Synthetic fixture (not simulation output): 3 lakes · 8 trees · 10 food',
      );
    });
    const synthLegend = synthView.getByTestId('sim-env-legend');
    expect(synthLegend.textContent).not.toMatch(/^Simulated:/);
    expect(synthLegend.getAttribute('title')).not.toContain('from the Rust simulation');
    expect(
      (
        synthView.container.querySelector(
          'instancedmesh[name="sim-env-lakes"]',
        ) as MockInstancedEl | null
      )?.count,
    ).toBe(3);

    synthView.unmount();

    // 2. Real Tauri tick source: HUD stays "Simulated: ...".
    window.history.replaceState(null, '', '/landscape.html');
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      value: {},
      configurable: true,
    });
    const handlers: Array<(event: { payload: SimulationTickPayload }) => void> = [];
    mockTauriInvoke.mockResolvedValue(null);
    mockTauriListen.mockImplementation(
      async (eventName: string, cb: (event: { payload: SimulationTickPayload }) => void) => {
        if (eventName === 'simulation-tick') handlers.push(cb);
        return () => {};
      },
    );

    const liveView = render(<WorldShowcase />);
    await waitFor(() => {
      expect(handlers.length).toBeGreaterThan(0);
    });

    await act(async () => {
      const payload: SimulationTickPayload = {
        segments: [segment({ agent_id: 1, x: 0, z: 0 })],
        environmental_state: {
          elements: [
            { type: 'lake', x: -10, y: 10, radius: 4, resources: 80 },
            { type: 'lake', x: 15, y: 12, radius: 3.5, resources: 60 },
            { type: 'tree', x: 5, y: -8, radius: 1.5, resources: 70 },
            { type: 'food', x: -6, y: -4, radius: 0.4, resources: 40 },
          ],
        },
        head_directions: {},
      };
      for (const h of handlers) h({ payload });
    });

    await waitFor(() => {
      const liveLegend = liveView.getByTestId('sim-env-legend');
      expect(liveLegend.textContent).toBe('Simulated: 2 lakes · 1 trees · 1 food');
    });
    const liveLegend = liveView.getByTestId('sim-env-legend');
    expect(liveLegend.textContent).not.toContain('Synthetic');
    expect(liveLegend.getAttribute('title')).toContain('from the Rust simulation');
    expect(new THREE.Color().getHex()).toBeTypeOf('number');
  });
});
