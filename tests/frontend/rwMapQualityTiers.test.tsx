import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach, beforeAll } from 'vitest';
import { warmSharedWorld, WARM_SHARED_WORLD_BUDGET_MS } from './helpers/warmSharedWorld';
import { render, act, fireEvent, screen, cleanup } from '@testing-library/react';

const { canvasRenders, setDprCalls, shadowState } = vi.hoisted(() => ({
  canvasRenders: [] as Array<{ dpr: unknown; shadows: unknown }>,
  setDprCalls: [] as number[],
  shadowState: { enabled: false, needsUpdate: false },
}));

vi.mock('@react-three/fiber', async () => {
  const THREE = await import('three');
  const mockScene = new THREE.Scene();
  const mockCamera = new THREE.PerspectiveCamera();
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
    useFrame: () => {},
    useThree: <T,>(selector?: (state: typeof rootState) => T) =>
      selector ? selector(rootState) : rootState,
    extend: () => {},
  };
});

import WorldShowcase from '../../src/components/Landscape/WorldShowcase';
import {
  QUALITY_SELECTIONS,
  QUALITY_SELECTION_LABELS,
  QUALITY_TIERS,
  QUALITY_TIER_CONFIGS,
  getCanvasDprProp,
  getQualityTierConfig,
  isQualitySelection,
  isQualityTier,
  legacyVegetationQuality,
  readInitialQualitySelection,
  resolveDeviceDpr,
  resolveEffectiveQualityTier,
} from '../../src/components/Landscape/utils/qualityTiers';
import {
  CAPTURE_DEFAULT_QUALITY,
  readCaptureRequest,
} from '../../src/components/Landscape/utils/captureMode';

describe('RW-B2 (RW4-1) — unified quality tiers & 4-choice HUD', () => {
  beforeAll(warmSharedWorld, WARM_SHARED_WORLD_BUDGET_MS);
  const originalDpr = Object.getOwnPropertyDescriptor(window, 'devicePixelRatio');

  beforeEach(() => {
    canvasRenders.length = 0;
    setDprCalls.length = 0;
    shadowState.enabled = false;
    shadowState.needsUpdate = false;
    vi.clearAllMocks();
    window.history.replaceState({}, '', '/landscape.html');
    Object.defineProperty(window, 'devicePixelRatio', {
      value: 2,
      configurable: true,
    });
  });

  afterEach(() => {
    cleanup();
    if (originalDpr) {
      Object.defineProperty(window, 'devicePixelRatio', originalDpr);
    }
  });

  it('defines low, medium, and high tier configs aligned with performance budgets', () => {
    expect(QUALITY_TIERS).toEqual(['low', 'medium', 'high']);
    expect(QUALITY_SELECTIONS).toEqual(['auto', 'low', 'medium', 'high']);
    expect(QUALITY_SELECTION_LABELS).toEqual({
      auto: 'Tự động',
      low: 'Thấp',
      medium: 'Vừa',
      high: 'Cao',
    });

    const low = getQualityTierConfig('low');
    expect(low.dprRange).toEqual([0.75, 1]);
    expect(low.defaultDpr).toBe(1);
    expect(low.shadows).toBe(false);
    expect(low.contactShadows).toBe(true);
    expect(low.vegetationDensity).toBe(0.25);
    expect(low.maxTriangles).toBe(1_200_000);
    expect(low.maxCreatureTriangles).toBe(300_000);
    expect(low.maxDrawCalls).toBe(60);
    expect(low.targetFps).toBe(30);

    const medium = getQualityTierConfig('medium');
    expect(medium.dprRange).toEqual([1, 1]);
    expect(medium.defaultDpr).toBe(1);
    expect(medium.shadows).toBe(false);
    expect(medium.contactShadows).toBe(true);
    expect(medium.vegetationDensity).toBe(0.5);
    expect(medium.maxTriangles).toBe(2_500_000);
    expect(medium.maxCreatureTriangles).toBe(400_000);
    expect(medium.maxDrawCalls).toBe(60);
    expect(medium.targetFps).toBe(60);

    const high = getQualityTierConfig('high');
    expect(high.dprRange).toEqual([1, 1.5]);
    expect(high.defaultDpr).toBe(1.5);
    expect(high.shadows).toBe(true);
    expect(high.contactShadows).toBe(false);
    expect(high.vegetationDensity).toBe(1);
    expect(high.maxTriangles).toBe(6_000_000);
    expect(high.maxCreatureTriangles).toBe(600_000);
    expect(high.maxDrawCalls).toBe(70);
    expect(high.targetFps).toBe(60);

    expect(QUALITY_TIER_CONFIGS.low).toBe(low);
    expect(legacyVegetationQuality('low')).toBe('low');
    expect(legacyVegetationQuality('medium')).toBe('low');
    expect(legacyVegetationQuality('high')).toBe('high');
  });

  it('validates type guards, DPR resolution, and capture mode integration', () => {
    expect(isQualityTier('low')).toBe(true);
    expect(isQualityTier('medium')).toBe(true);
    expect(isQualityTier('high')).toBe(true);
    expect(isQualityTier('auto')).toBe(false);
    expect(isQualityTier('ultra')).toBe(false);

    expect(isQualitySelection('auto')).toBe(true);
    expect(isQualitySelection('medium')).toBe(true);
    expect(isQualitySelection('ultra')).toBe(false);

    expect(resolveEffectiveQualityTier('auto')).toBe('high');
    expect(resolveEffectiveQualityTier('auto', 'medium')).toBe('medium');
    expect(resolveEffectiveQualityTier('low', 'high')).toBe('low');

    expect(resolveDeviceDpr('high', 2)).toBe(1.5);
    expect(resolveDeviceDpr('high', 0.5)).toBe(1);
    expect(resolveDeviceDpr('low', 2)).toBe(1);
    expect(resolveDeviceDpr('low', 2, 0.75)).toBe(0.75);
    expect(resolveDeviceDpr('low', 2, 0.5)).toBe(0.75);
    expect(resolveDeviceDpr('medium', 2)).toBe(1);

    expect(getCanvasDprProp('high')).toEqual([1, 1.5]);
    expect(getCanvasDprProp('medium')).toBe(1);
    expect(getCanvasDprProp('low')).toBe(1);
    expect(getCanvasDprProp('low', 0.8)).toBe(0.8);

    expect(readInitialQualitySelection('')).toBe('auto');
    expect(readInitialQualitySelection('?quality=medium')).toBe('medium');
    expect(readInitialQualitySelection('?quality=bogus')).toBe('auto');
    expect(readInitialQualitySelection('?quality=low', 'high')).toBe('high');

    expect(CAPTURE_DEFAULT_QUALITY).toBe('high');
    expect(readCaptureRequest('?capture=1&view=overview')?.quality).toBe('high');
    expect(readCaptureRequest('?capture=1&view=overview&quality=medium')?.quality).toBe('medium');
    expect(readCaptureRequest('?capture=1&view=overview&quality=low')?.quality).toBe('low');
    expect(() => readCaptureRequest('?capture=1&view=overview&quality=auto')).toThrow();
  });

  it('renders 4 HUD quality buttons and updates Canvas DPR and shadows across every selection', () => {
    render(<WorldShowcase />);

    const autoBtn = screen.getByTestId('quality-btn-auto');
    const lowBtn = screen.getByTestId('quality-btn-low');
    const mediumBtn = screen.getByTestId('quality-btn-medium');
    const highBtn = screen.getByTestId('quality-btn-high');

    expect(autoBtn.textContent).toBe('Tự động');
    expect(lowBtn.textContent).toBe('Thấp');
    expect(mediumBtn.textContent).toBe('Vừa');
    expect(highBtn.textContent).toBe('Cao');

    expect(autoBtn.getAttribute('aria-pressed')).toBe('true');
    expect(canvasRenders[canvasRenders.length - 1]).toEqual({
      dpr: [1, 1.5],
      shadows: 'percentage',
    });
    expect(setDprCalls[setDprCalls.length - 1]).toBe(1.5);
    expect(shadowState.enabled).toBe(true);

    act(() => {
      fireEvent.click(mediumBtn);
    });
    expect(mediumBtn.getAttribute('aria-pressed')).toBe('true');
    expect(autoBtn.getAttribute('aria-pressed')).toBe('false');
    expect(canvasRenders[canvasRenders.length - 1]).toEqual({ dpr: 1, shadows: false });
    expect(setDprCalls[setDprCalls.length - 1]).toBe(1);
    expect(shadowState.enabled).toBe(false);

    act(() => {
      fireEvent.click(lowBtn);
    });
    expect(lowBtn.getAttribute('aria-pressed')).toBe('true');
    expect(canvasRenders[canvasRenders.length - 1]).toEqual({ dpr: 1, shadows: false });
    expect(shadowState.enabled).toBe(false);

    act(() => {
      fireEvent.click(highBtn);
    });
    expect(highBtn.getAttribute('aria-pressed')).toBe('true');
    expect(canvasRenders[canvasRenders.length - 1]).toEqual({
      dpr: [1, 1.5],
      shadows: 'percentage',
    });
    expect(setDprCalls[setDprCalls.length - 1]).toBe(1.5);
    expect(shadowState.enabled).toBe(true);
  });

  it('honors initial ?quality=medium URL selection on mount', () => {
    window.history.replaceState({}, '', '/landscape.html?quality=medium');
    render(<WorldShowcase />);

    expect(screen.getByTestId('quality-btn-medium').getAttribute('aria-pressed')).toBe('true');
    expect(canvasRenders[canvasRenders.length - 1]).toEqual({ dpr: 1, shadows: false });
    expect(setDprCalls[setDprCalls.length - 1]).toBe(1);
    expect(shadowState.enabled).toBe(false);
  });
});
