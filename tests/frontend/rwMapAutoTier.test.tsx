import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach, beforeAll } from 'vitest';
import { warmSharedWorld, WARM_SHARED_WORLD_BUDGET_MS } from './helpers/warmSharedWorld';
import { render, act, fireEvent, screen, cleanup } from '@testing-library/react';
import type { FrameCallback } from '../mocks/r3f-frame-state';

const { canvasRenders, setDprCalls, shadowState, frameCallbacks } = vi.hoisted(() => ({
  canvasRenders: [] as Array<{ dpr: unknown; shadows: unknown }>,
  setDprCalls: [] as number[],
  shadowState: { enabled: false, needsUpdate: false },
  frameCallbacks: [] as FrameCallback[],
}));

vi.mock('@react-three/fiber', async () => {
  const THREE = await import('three');
  const mockScene = new THREE.Scene();
  const mockCamera = new THREE.PerspectiveCamera();
  const mockGl = {
    setSize: vi.fn(),
    domElement: document.createElement('canvas'),
    render: vi.fn(),
    getContext: () => ({ disable: vi.fn(), DITHER: 0 }),
    shadowMap: shadowState,
  };
  const rootState = {
    clock: { getElapsedTime: () => 0, elapsedTime: 0 },
    scene: mockScene,
    camera: mockCamera,
    gl: mockGl,
    size: { width: 1024, height: 768 },
    setFrameloop: vi.fn(),
    setDpr: vi.fn((val: number) => {
      setDprCalls.push(val);
    }),
  };
  return {
    Canvas: ({
      children,
      dpr,
      shadows,
    }: {
      children?: React.ReactNode;
      dpr?: unknown;
      shadows?: unknown;
    }) => {
      canvasRenders.push({ dpr, shadows });
      return (
        <div
          data-testid="mock-canvas"
          data-dpr={JSON.stringify(dpr)}
          data-shadows={String(shadows)}
        >
          {children}
        </div>
      );
    },
    useFrame: (cb: FrameCallback) => {
      // Only one R3F useFrame consumer is active at a time in this test (`AutoQualityMonitor`
      // or `CaptureReadySignal`), so keep the latest callback rather than re-pushing per render.
      frameCallbacks[0] = cb;
    },
    useThree: <T,>(selector?: (state: typeof rootState) => T) =>
      selector ? selector(rootState) : rootState,
    extend: () => {},
  };
});

vi.mock('../../src/components/Landscape/WorldSky', () => ({ default: () => null }));
vi.mock('../../src/components/Landscape/WorldWeather', () => ({ default: () => null }));
vi.mock('../../src/components/Landscape/WorldTerrain', () => ({ default: () => null }));
vi.mock('../../src/components/Landscape/WorldTerrainLod', () => ({ default: () => null }));
vi.mock('../../src/components/Landscape/WorldVegetation', () => ({ default: () => null }));
vi.mock('../../src/components/Landscape/WorldWater', () => ({ default: () => null }));
vi.mock('../../src/components/Landscape/WorldWaterfalls', () => ({ default: () => null }));
vi.mock('../../src/components/Landscape/WorldCaves', () => ({ default: () => null }));
vi.mock('../../src/components/Landscape/WorldBirds', () => ({ default: () => null }));
vi.mock('../../src/components/Landscape/WorldFish', () => ({ default: () => null }));
vi.mock('../../src/components/Landscape/WorldWildlife', () => ({ default: () => null }));
vi.mock('../../src/components/Landscape/LiveAgents', () => ({ default: () => null }));
vi.mock('../../src/components/Landscape/WorldCameraRig', () => ({ default: () => null }));

import WorldShowcase from '../../src/components/Landscape/WorldShowcase';
import {
  AUTO_QUALITY_DOWNGRADE_WINDOW_MS,
  AUTO_QUALITY_MAX_SAMPLE_FRAME_MS,
  AUTO_QUALITY_SETTLE_MS,
  AUTO_QUALITY_UPGRADE_WINDOW_MS,
  AUTO_QUALITY_WARMUP_FRAMES,
  createAutoQualityState,
  medianFrameMs,
  pickSettledQualityTier,
  resolveEffectiveQualityState,
  stepAutoQualityState,
  stepDownQuality,
  type AutoQualityState,
} from '../../src/components/Landscape/utils/qualityTiers';
import {
  CAPTURE_DEFAULT_QUALITY,
  isAutoQualityAllowed,
  readCaptureRequest,
} from '../../src/components/Landscape/utils/captureMode';
import { makeFrameState } from '../mocks/r3f-frame-state';

function advanceFrames(state: AutoQualityState, count: number, frameMs: number): AutoQualityState {
  let cur = state;
  for (let i = 0; i < count; i += 1) {
    cur = stepAutoQualityState(cur, frameMs);
  }
  return cur;
}

describe('RW-B3 (RW4-4 & RW4-5) — auto quality state machine and capture bypass', () => {
  beforeAll(warmSharedWorld, WARM_SHARED_WORLD_BUDGET_MS);
  const originalDpr = Object.getOwnPropertyDescriptor(window, 'devicePixelRatio');

  beforeEach(() => {
    canvasRenders.length = 0;
    setDprCalls.length = 0;
    frameCallbacks.length = 0;
    shadowState.enabled = false;
    shadowState.needsUpdate = false;
    vi.clearAllMocks();
    window.history.replaceState({}, '', '/landscape.html');
    Object.defineProperty(window, 'devicePixelRatio', {
      value: 1.5,
      configurable: true,
    });
  });

  afterEach(() => {
    cleanup();
    if (originalDpr) {
      Object.defineProperty(window, 'devicePixelRatio', originalDpr);
    }
  });

  it('RW4-5 settle picks high, medium, or low once warmup and settle duration complete', () => {
    expect(medianFrameMs([])).toBe(0);
    expect(medianFrameMs([40, 16, 20])).toBe(20);
    expect(medianFrameMs([10, 20, 30, 40])).toBe(25);
    expect(pickSettledQualityTier(16)).toBe('high');
    expect(pickSettledQualityTier(26)).toBe('medium');
    expect(pickSettledQualityTier(42)).toBe('low');

    let fast = createAutoQualityState({ devicePixelRatio: 1.5 });
    fast = advanceFrames(fast, AUTO_QUALITY_WARMUP_FRAMES - 1, 16);
    expect(fast.phase).toBe('warmup');
    fast = stepAutoQualityState(fast, 16);
    expect(fast.phase).toBe('settling');
    fast = advanceFrames(fast, 63, 16);
    expect(fast.phase).toBe('running');
    expect(fast.tier).toBe('high');

    let mid = advanceFrames(createAutoQualityState(), AUTO_QUALITY_WARMUP_FRAMES, 25);
    mid = advanceFrames(mid, Math.ceil(AUTO_QUALITY_SETTLE_MS / 25), 25);
    expect(mid.phase).toBe('running');
    expect(mid.tier).toBe('medium');

    let slow = advanceFrames(createAutoQualityState(), AUTO_QUALITY_WARMUP_FRAMES, 42);
    slow = advanceFrames(slow, Math.ceil(AUTO_QUALITY_SETTLE_MS / 42), 42);
    expect(slow.phase).toBe('running');
    expect(slow.tier).toBe('low');
  });

  it('RW4-5 lowers DPR first, then lowers tier over 2s over-budget windows, and ignores tab pauses', () => {
    expect(stepDownQuality('high', undefined, 1.5)).toEqual({ tier: 'high', dprOverride: 1 });
    expect(stepDownQuality('high', 1, 1.5)).toEqual({ tier: 'medium', dprOverride: undefined });
    expect(stepDownQuality('medium', undefined, 1.5)).toEqual({ tier: 'low', dprOverride: undefined });
    expect(stepDownQuality('low', undefined, 1.5)).toEqual({ tier: 'low', dprOverride: 0.75 });
    expect(stepDownQuality('low', 0.75, 1.5)).toEqual({ tier: 'low', dprOverride: 0.75 });

    // Settle onto 'high'
    let state = advanceFrames(createAutoQualityState({ devicePixelRatio: 1.5 }), AUTO_QUALITY_WARMUP_FRAMES, 16);
    state = advanceFrames(state, 63, 16);
    expect(state.tier).toBe('high');
    expect(state.dprOverride).toBeUndefined();

    // Tab pause frames above max sample ceiling are ignored verbatim
    const beforePause = state;
    expect(stepAutoQualityState(beforePause, AUTO_QUALITY_MAX_SAMPLE_FRAME_MS + 100)).toBe(beforePause);

    // 83 frames at 24 ms (1992 ms) is still inside the 2000 ms downgrade window
    state = advanceFrames(state, Math.floor(AUTO_QUALITY_DOWNGRADE_WINDOW_MS / 24), 24);
    expect(state.tier).toBe('high');
    expect(state.dprOverride).toBeUndefined();

    // Crossing 2000 ms drops DPR from 1.5 to 1 before leaving 'high'
    state = stepAutoQualityState(state, 24);
    expect(state.tier).toBe('high');
    expect(state.dprOverride).toBe(1);

    // Another 2 s over budget drops from high@DPR1 to medium
    state = advanceFrames(state, 84, 24);
    expect(state.tier).toBe('medium');
    expect(state.dprOverride).toBeUndefined();

    // Another 2 s at 40 ms drops medium -> low@DPR1
    state = advanceFrames(state, 50, 40);
    expect(state.tier).toBe('low');
    expect(state.dprOverride).toBeUndefined();

    // Another 2 s at 45 ms (above 30 FPS low budget) drops DPR at low to 0.75 floor
    state = advanceFrames(state, 45, 45);
    expect(state.tier).toBe('low');
    expect(state.dprOverride).toBe(0.75);
  });

  it('RW4-5 allows at most one upgrade after 10s sustained headroom with zero oscillation', () => {
    let state = advanceFrames(createAutoQualityState({ devicePixelRatio: 1.5 }), AUTO_QUALITY_WARMUP_FRAMES, 25);
    state = advanceFrames(state, 40, 25);
    expect(state.tier).toBe('medium');
    expect(state.upgradedOnce).toBe(false);

    // Just under 10s at 12 ms does not upgrade yet
    const framesBeforeUpgrade = Math.floor(AUTO_QUALITY_UPGRADE_WINDOW_MS / 12);
    state = advanceFrames(state, framesBeforeUpgrade, 12);
    expect(state.tier).toBe('medium');
    expect(state.upgradedOnce).toBe(false);

    // Crossing 10s upgrades once medium -> high
    state = stepAutoQualityState(state, 12);
    expect(state.tier).toBe('high');
    expect(state.upgradedOnce).toBe(true);

    // Subsequent sustained slow load steps down high -> high@DPR1 -> medium
    state = advanceFrames(state, 84, 24);
    expect(state.tier).toBe('high');
    expect(state.dprOverride).toBe(1);

    state = advanceFrames(state, 84, 24);
    expect(state.tier).toBe('medium');
    expect(state.dprOverride).toBeUndefined();

    // 30s of sustained headroom never triggers a second upgrade (no oscillation)
    state = advanceFrames(state, 2500, 12);
    expect(state.tier).toBe('medium');
    expect(state.upgradedOnce).toBe(true);
  });

  it('RW4-4 & RW4-5 manual selection wins and capture mode bypasses AutoQualityMonitor', () => {
    expect(resolveEffectiveQualityState('low', { tier: 'high', dprOverride: 1 })).toEqual({
      tier: 'low',
      dprOverride: undefined,
    });
    expect(resolveEffectiveQualityState('auto', { tier: 'low', dprOverride: 0.75 }, 'high')).toEqual({
      tier: 'high',
      dprOverride: undefined,
    });
    expect(isAutoQualityAllowed(null)).toBe(true);
    expect(isAutoQualityAllowed(readCaptureRequest('?capture=1&view=overview'))).toBe(false);

    render(<WorldShowcase />);
    const r3fState = makeFrameState();

    // Drive AutoQualityMonitor: 30 warmup + 63 settle (16 ms) + 84 over-budget (24 ms) -> high@DPR1
    act(() => {
      for (let i = 0; i < AUTO_QUALITY_WARMUP_FRAMES + 63; i += 1) {
        for (const cb of frameCallbacks) cb(r3fState, 0.016);
      }
    });
    act(() => {
      for (let i = 0; i < 84; i += 1) {
        for (const cb of frameCallbacks) cb(r3fState, 0.024);
      }
    });
    expect(canvasRenders[canvasRenders.length - 1]).toEqual({ dpr: 1, shadows: 'percentage' });
    expect(setDprCalls[setDprCalls.length - 1]).toBe(1);

    // Manual override immediately wins over auto DPR=1
    act(() => {
      fireEvent.click(screen.getByTestId('quality-btn-high'));
    });
    expect(canvasRenders[canvasRenders.length - 1]).toEqual({
      dpr: [1, 1.5],
      shadows: 'percentage',
    });
    expect(setDprCalls[setDprCalls.length - 1]).toBe(1.5);

    cleanup();
    canvasRenders.length = 0;
    setDprCalls.length = 0;
    frameCallbacks.length = 0;

    // Capture mode mounts CaptureReadySignal without AutoQualityMonitor or HUD
    window.history.replaceState({}, '', '/landscape.html?capture=1&view=overview');
    render(<WorldShowcase />);
    expect(screen.queryByTestId('quality-btn-auto')).toBeNull();
    expect(canvasRenders[canvasRenders.length - 1]).toEqual({
      dpr: [1, 1.5],
      shadows: CAPTURE_DEFAULT_QUALITY === 'high' ? 'percentage' : false,
    });
    expect(frameCallbacks).toHaveLength(1);
    // Advancing slow frames in capture mode (CaptureReadySignal only) never mutates quality/DPR
    act(() => {
      for (let i = 0; i < 89; i += 1) {
        for (const cb of frameCallbacks) cb(r3fState, 0.05);
      }
    });
    expect(canvasRenders[canvasRenders.length - 1]).toEqual({
      dpr: [1, 1.5],
      shadows: 'percentage',
    });
  });
});
