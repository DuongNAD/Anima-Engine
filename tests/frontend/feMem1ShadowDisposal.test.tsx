import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, fireEvent, act, cleanup, screen } from '@testing-library/react';
import * as THREE from 'three';

const {
  shadowState,
  r3fRef,
  mapDispose,
  depthDispose,
  mapPassDispose,
} = vi.hoisted(() => ({
  shadowState: { enabled: false, needsUpdate: false },
  r3fRef: {
    scene: null as THREE.Scene | null,
    sunLight: null as THREE.DirectionalLight | null,
  },
  mapDispose: vi.fn(),
  depthDispose: vi.fn(),
  mapPassDispose: vi.fn(),
}));

vi.mock('@react-three/fiber', async () => {
  const actualThree = await import('three');
  const scene = new actualThree.Scene();
  const camera = new actualThree.PerspectiveCamera();
  const sunLight = new actualThree.DirectionalLight(0xffffff, 1);
  sunLight.name = 'world-sun-light';
  scene.add(sunLight);
  r3fRef.scene = scene;
  r3fRef.sunLight = sunLight;

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
    scene,
    camera,
    gl: mockGl,
    size: { width: 1024, height: 768 },
    setFrameloop: vi.fn(),
    setDpr: vi.fn(),
  };
  return {
    Canvas: ({ children }: { children?: React.ReactNode }) => (
      <div data-testid="mock-canvas">{children}</div>
    ),
    useFrame: () => {},
    useThree: <T,>(selector?: (state: typeof rootState) => T) =>
      selector ? selector(rootState) : rootState,
    extend: () => {},
  };
});

import WorldShowcase from '../../src/components/Landscape/WorldShowcase';
import WorldSky from '../../src/components/Landscape/WorldSky';
import { warmSharedWorld, WARM_SHARED_WORLD_BUDGET_MS } from './helpers/warmSharedWorld';

interface MockShadowTarget {
  map: {
    dispose: ReturnType<typeof vi.fn>;
    depthTexture: { dispose: ReturnType<typeof vi.fn> } | null;
    width?: number;
    height?: number;
  } | null;
  mapPass: { dispose: ReturnType<typeof vi.fn> } | null;
}

describe('FE-MEM-1 shadow render target disposal', () => {
  beforeEach(async () => {
    await warmSharedWorld();
    mapDispose.mockClear();
    depthDispose.mockClear();
    mapPassDispose.mockClear();
    shadowState.enabled = false;

    const shadow = r3fRef.sunLight!.shadow as unknown as MockShadowTarget;
    shadow.map = {
      dispose: mapDispose,
      depthTexture: { dispose: depthDispose },
      width: 2048,
      height: 2048,
    };
    shadow.mapPass = { dispose: mapPassDispose };
  }, WARM_SHARED_WORLD_BUDGET_MS);

  afterEach(() => {
    cleanup();
  });

  it('disposes the 2048x2048 shadow render target and depthTexture when switching to Low', async () => {
    await act(async () => {
      render(<WorldShowcase />);
    });

    const lowBtn = screen.getByTestId('quality-btn-low');
    await act(async () => {
      fireEvent.click(lowBtn);
    });

    expect(shadowState.enabled).toBe(false);
    expect(depthDispose).toHaveBeenCalledTimes(1);
    expect(mapDispose).toHaveBeenCalledTimes(1);
    expect(mapPassDispose).toHaveBeenCalledTimes(1);

    const shadow = r3fRef.sunLight!.shadow as unknown as MockShadowTarget;
    expect(shadow.map).toBeNull();
    expect(shadow.mapPass).toBeNull();
  });

  it('disposes attached directional light shadow resources in WorldSky when castShadow disables or on unmount', () => {
    const consoleErr = vi.spyOn(console, 'error').mockImplementation(() => {});
    const { container, rerender, unmount } = render(
      <WorldSky timeOfDay={12} speed={0} worldScale={400} castShadow={true} shadowMapSize={2048} />,
    );

    const lightEl = container.querySelector('[name="world-sun-light"]') as unknown as
      | (Element & { shadow?: MockShadowTarget })
      | null;
    expect(lightEl).not.toBeNull();

    const localDepthDispose = vi.fn();
    const localMapDispose = vi.fn();
    const localMapPassDispose = vi.fn();
    if (lightEl) {
      lightEl.shadow = {
        map: {
          dispose: localMapDispose,
          depthTexture: { dispose: localDepthDispose },
          width: 2048,
          height: 2048,
        },
        mapPass: { dispose: localMapPassDispose },
      };
    }

    rerender(
      <WorldSky timeOfDay={12} speed={0} worldScale={400} castShadow={false} shadowMapSize={1024} />,
    );

    expect(localDepthDispose).toHaveBeenCalledTimes(1);
    expect(localMapDispose).toHaveBeenCalledTimes(1);
    expect(localMapPassDispose).toHaveBeenCalledTimes(1);
    expect(lightEl?.shadow?.map).toBeNull();
    expect(lightEl?.shadow?.mapPass).toBeNull();

    // Verify unmount cleanup also disposes a newly attached shadow map target.
    const unmountMapDispose = vi.fn();
    if (lightEl) {
      lightEl.shadow = {
        map: { dispose: unmountMapDispose, depthTexture: null },
        mapPass: null,
      };
    }
    unmount();
    expect(unmountMapDispose).toHaveBeenCalledTimes(1);
    consoleErr.mockRestore();
  });
});
