import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, act, fireEvent, screen, cleanup } from '@testing-library/react';

const { canvasRenders, setDprCalls, shadowState, mockGlHolder } = vi.hoisted(() => ({
  canvasRenders: [] as Array<{ dpr: unknown; shadows: unknown }>,
  setDprCalls: [] as number[],
  shadowState: { enabled: false, needsUpdate: false },
  mockGlHolder: { shadowMap: null as null | { enabled: boolean; needsUpdate: boolean } },
}));

vi.mock('@react-three/fiber', async () => {
  const THREE = await import('three');
  const mockScene = new THREE.Scene();
  const mockCamera = new THREE.PerspectiveCamera();
  mockGlHolder.shadowMap = shadowState;
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
import WorldSky from '../../src/components/Landscape/WorldSky';
import {
  NIGHT_LIGHT_FLOOR,
  getSkyParams,
  getWorldLightRig,
} from '../../src/components/Landscape/utils/skyParams';
import {
  ALWAYS_DAY_HOUR,
  DEFAULT_DAY_LENGTH_SECONDS,
  stepTimeOfDay,
} from '../../src/components/Landscape/utils/sceneClock';

describe('RW-B1 — map quick wins', () => {
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
    vi.useRealTimers();
    if (originalDpr) {
      Object.defineProperty(window, 'devicePixelRatio', originalDpr);
    }
  });

  describe('RW1-1 — low quality locks DPR to 1 across parent clock re-renders', () => {
    it('passes dynamic dpr=1 and shadows=false at low quality and holds across timer re-renders', () => {
      vi.useFakeTimers();
      render(<WorldShowcase />);

      expect(canvasRenders.length).toBeGreaterThan(0);
      expect(canvasRenders[canvasRenders.length - 1]).toEqual({
        dpr: [1, 1.5],
        shadows: 'percentage',
      });
      expect(setDprCalls).toContain(1.5);
      expect(shadowState.enabled).toBe(true);

      const lowBtn = screen.getByRole('button', { name: 'Nhẹ' });
      act(() => {
        fireEvent.click(lowBtn);
      });

      const countAtSwitch = canvasRenders.length;
      expect(canvasRenders[countAtSwitch - 1]).toEqual({
        dpr: 1,
        shadows: false,
      });
      expect(setDprCalls[setDprCalls.length - 1]).toBe(1);
      expect(shadowState.enabled).toBe(false);

      // Simulate 1 second of clock (100 ms) and telemetry (250 ms) parent re-renders separately
      // so React commits each tick instead of batching all 10 intervals into one render.
      for (let tick = 0; tick < 10; tick += 1) {
        act(() => {
          vi.advanceTimersByTime(100);
        });
      }

      const afterSwitch = canvasRenders.slice(countAtSwitch - 1);
      expect(afterSwitch.length).toBeGreaterThan(5);
      for (const call of afterSwitch) {
        expect(call.dpr).toBe(1);
        expect(call.shadows).toBe(false);
      }
      expect(setDprCalls[setDprCalls.length - 1]).toBe(1);
      expect(shadowState.enabled).toBe(false);

      const highBtn = screen.getByRole('button', { name: 'Đẹp' });
      act(() => {
        fireEvent.click(highBtn);
      });
      expect(canvasRenders[canvasRenders.length - 1]).toEqual({
        dpr: [1, 1.5],
        shadows: 'percentage',
      });
      expect(setDprCalls[setDprCalls.length - 1]).toBe(1.5);
      expect(shadowState.enabled).toBe(true);
    });
  });

  describe('RW1-2 — night lighting floor keeps terrain readable while t=10 stays unchanged', () => {
    it('leaves canonical capture time t=10 parameters unchanged', () => {
      const p10 = getSkyParams(10);
      expect(p10).toEqual({
        skyColor: '#88c9f3',
        sunColor: '#fff8cf',
        sunIntensity: 1.05,
        ambientColor: '#ceecfe',
        ambientIntensity: 0.7,
        hemiSkyColor: '#88c9f3',
        hemiGroundColor: '#182c68',
        hemiIntensity: 0.7,
        starOpacity: 0,
      });
      const rig10 = getWorldLightRig(10);
      expect(rig10.showMoon).toBe(false);
      expect(rig10.moonIntensity).toBe(0);
    });

    it('keeps total light intensity at or above NIGHT_LIGHT_FLOOR for every t in [0, 24]', () => {
      expect(NIGHT_LIGHT_FLOOR).toBe(0.35);

      for (let step = 0; step < 481; step += 1) {
        const t = step * 0.05;
        const rig = getWorldLightRig(t);
        expect(rig.totalIntensity, `t=${t.toFixed(2)}`).toBeGreaterThanOrEqual(NIGHT_LIGHT_FLOOR);
      }

      for (const edge of [-6, 20.1, 24, 30, Number.NaN, Number.POSITIVE_INFINITY]) {
        const rig = getWorldLightRig(edge);
        expect(Number.isFinite(rig.totalIntensity)).toBe(true);
        expect(rig.totalIntensity).toBeGreaterThanOrEqual(NIGHT_LIGHT_FLOOR);
      }
    });

    it('renders directional moonlight and raised night ambient/hemi in WorldSky at t=0 only', () => {
      const { container: night } = render(<WorldSky timeOfDay={0} />);
      const moonLight = night.querySelector('[name="world-moon-light"]');
      const hemiLight = night.querySelector('[name="world-hemi-light"]');
      const ambientLight = night.querySelector('[name="world-ambient-light"]');
      expect(moonLight).not.toBeNull();
      expect(night.querySelector('[name="world-moon-mesh"]')).not.toBeNull();
      expect(night.querySelector('[name="world-stars"]')).not.toBeNull();

      const rig0 = getWorldLightRig(0);
      expect(Number(moonLight?.getAttribute('intensity'))).toBeCloseTo(rig0.moonIntensity, 5);
      expect(Number(hemiLight?.getAttribute('intensity'))).toBeCloseTo(rig0.hemiIntensity, 5);
      expect(Number(ambientLight?.getAttribute('intensity'))).toBeCloseTo(rig0.ambientIntensity, 5);

      cleanup();

      const { container: day10 } = render(<WorldSky timeOfDay={10} />);
      expect(day10.querySelector('[name="world-moon-light"]')).toBeNull();
      expect(day10.querySelector('[name="world-moon-mesh"]')).toBeNull();
      expect(day10.querySelector('[name="world-stars"]')).toBeNull();
    });
  });

  describe('RW1-3 — 10-minute default day length and always-day HUD toggle', () => {
    it('advances 24 hours over 600 seconds at 1x and pins to 11:00 when alwaysDay is true', () => {
      expect(DEFAULT_DAY_LENGTH_SECONDS).toBe(600);
      expect(ALWAYS_DAY_HOUR).toBe(11);

      expect(stepTimeOfDay(11, 1, 0.1, false)).toBeCloseTo(11.004, 8);
      expect(stepTimeOfDay(0, 1, 300, false)).toBeCloseTo(12, 8);
      expect(stepTimeOfDay(0, 1, 600, false)).toBeCloseTo(0, 8);
      expect(stepTimeOfDay(23.99, 1, 1, false)).toBeCloseTo(0.03, 8);
      expect(stepTimeOfDay(15.5, 0, 10, false)).toBeCloseTo(15.5, 8);
      expect(stepTimeOfDay(2, 4, 10, true)).toBe(ALWAYS_DAY_HOUR);
    });

    it('runs the HUD clock on the 10-minute cycle and pins at 11:00 with Luôn ban ngày', () => {
      vi.useFakeTimers();
      render(<WorldShowcase />);

      const alwaysDayBtn = screen.getByRole('button', { name: /Luôn ban ngày/i });
      expect(alwaysDayBtn.getAttribute('aria-pressed')).toBe('false');
      expect(screen.getByText(/11:00/)).toBeTruthy();

      // 30.1 seconds at 1x = +1.204 simulated hours (11:00 -> 12:12), vs +15.05 h on the old 48 s day.
      act(() => {
        vi.advanceTimersByTime(30_100);
      });
      expect(screen.getByText(/12:12/)).toBeTruthy();

      act(() => {
        fireEvent.click(alwaysDayBtn);
      });
      expect(alwaysDayBtn.getAttribute('aria-pressed')).toBe('true');
      expect(screen.getByText(/11:00/)).toBeTruthy();

      act(() => {
        vi.advanceTimersByTime(30_000);
      });
      expect(screen.getByText(/11:00/)).toBeTruthy();
    });
  });
});
