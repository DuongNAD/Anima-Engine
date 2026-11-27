import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach, beforeAll } from 'vitest';
import { render, cleanup, act, fireEvent, waitFor } from '@testing-library/react';
import type { FrameCallback } from '../mocks/r3f-frame-state';
import { installStubbedProperty, removeStubbedProperties } from '../mocks/r3f-dom-stubs';
import { warmSharedWorld, WARM_SHARED_WORLD_BUDGET_MS } from './helpers/warmSharedWorld';
import {
  DEFAULT_PERF_CAPACITY,
  PerfRecorder,
  buildPerfReportJson,
  ensureFrameCountersInstalled,
  percentileNearestRank,
  probeGpuTimers,
} from '../../src/components/Landscape/utils/perfRecorder';
import {
  CANONICAL_POSE_SETTER_GLOBAL,
  FRAME_COUNTERS_GLOBAL,
  diagnostics,
  type FrameRenderCounters,
} from '../../src/components/Landscape/utils/diagnosticsWindow';

function makeCounters(
  frame: number,
  calls: number,
  triangles: number,
  terrainCalls = 1,
  floraCalls = 4,
): FrameRenderCounters {
  return {
    frame,
    total: { calls, triangles },
    layers: {
      terrain: { calls: terrainCalls, triangles: Math.round(triangles * 0.4) },
      flora: { calls: floraCalls, triangles: Math.round(triangles * 0.3) },
      creatures: { calls: Math.max(0, calls - terrainCalls - floraCalls - 2), triangles: 1200 },
      water: { calls: 1, triangles: 800 },
    },
    other: { calls: 1, triangles: 400 },
  };
}

describe('perfRecorder — nearest-rank percentiles, ring buffer, GPU timers, and report schema', () => {
  afterEach(() => {
    delete diagnostics()[FRAME_COUNTERS_GLOBAL];
    delete diagnostics()[CANONICAL_POSE_SETTER_GLOBAL];
  });

  it('matches tick_capture.rs percentile_nearest_rank edge and 1..=100 cases', () => {
    expect(percentileNearestRank([], 0, 50)).toBe(0);
    const single = [7];
    for (const p of [0, 50, 95, 99, 100]) {
      expect(percentileNearestRank(single, 1, p)).toBe(7);
    }
    const pair = [1, 2];
    expect(percentileNearestRank(pair, 2, 50)).toBe(1);
    expect(percentileNearestRank(pair, 2, 51)).toBe(2);
    expect(percentileNearestRank(pair, 2, 100)).toBe(2);

    const hundred = Array.from({ length: 100 }, (_, i) => i + 1);
    expect(percentileNearestRank(hundred, 100, 50)).toBe(50);
    expect(percentileNearestRank(hundred, 100, 95)).toBe(95);
    expect(percentileNearestRank(hundred, 100, 99)).toBe(99);
    expect(percentileNearestRank(hundred, 100, 100)).toBe(100);
  });

  it('computes median_fps, p5_fps, p1_fps, worst_frame_ms, and draw call stats accurately', () => {
    const rec = new PerfRecorder(256);
    // 20 frames: 18 at 10 ms (100 FPS), 1 at 20 ms (50 FPS, P95), 1 at 40 ms (25 FPS, P99/worst)
    for (let i = 0; i < 18; i += 1) {
      rec.recordFrame(10, makeCounters(i + 1, 20 + (i % 5), 100_000 + i * 1_000, 2, 8));
    }
    rec.recordFrame(20, makeCounters(19, 55, 250_000, 3, 10));
    rec.recordFrame(40, makeCounters(20, 42, 190_000, 2, 12));

    const summary = rec.summarize();
    expect(summary.frames).toBe(20);
    expect(summary.duration_ms).toBe(240);
    expect(summary.p50_frame_ms).toBe(10);
    expect(summary.p95_frame_ms).toBe(20);
    expect(summary.p99_frame_ms).toBe(40);
    expect(summary.worst_frame_ms).toBe(40);
    expect(summary.median_fps).toBe(100);
    expect(summary.p5_fps).toBe(50);
    expect(summary.p1_fps).toBe(25);
    expect(summary.draw_calls).toEqual({ peak: 55, median: 22, current: 42 });
    expect(summary.triangles).toEqual({ peak: 250_000, current: 190_000 });
    expect(summary.layer_peaks?.terrain.calls).toBe(3);
    expect(summary.layer_peaks?.flora.calls).toBe(12);
    expect(summary.layer_peaks?.creatures.calls).toBe(40);
  });

  it('keeps fixed-capacity pre-allocated typed arrays across wraps with zero array growth', () => {
    const rec = new PerfRecorder(64);
    expect(DEFAULT_PERF_CAPACITY).toBe(16_384);
    const bufsBefore = rec.getBackingBuffers();
    const deltaBacking = bufsBefore.deltasMs.buffer;
    const callsBacking = bufsBefore.callsBuf.buffer;
    const flagsBacking = bufsBefore.hasCallsBuf.buffer;

    for (let i = 0; i < 64 * 3; i += 1) {
      rec.recordFrame(16.6 + (i % 4), null);
    }

    const bufsAfter = rec.getBackingBuffers();
    expect(bufsAfter.deltasMs.buffer).toBe(deltaBacking);
    expect(bufsAfter.callsBuf.buffer).toBe(callsBacking);
    expect(bufsAfter.hasCallsBuf.buffer).toBe(flagsBacking);
    expect(bufsAfter.deltasMs.length).toBe(64);
    expect(rec.bufferedCount).toBe(64);
    expect(rec.frameCount).toBe(192);
    const s = rec.summarize();
    expect(s.draw_calls).toBeNull();
    expect(s.triangles).toBeNull();
    expect(s.layer_peaks).toBeNull();
  });

  it('probes EXT_disjoint_timer_query_webgl2 honestly and never estimates GPU ms', () => {
    expect(probeGpuTimers(null)).toEqual({ status: 'unavailable', reason: 'no_webgl_context' });
    expect(probeGpuTimers({ getContext: () => null })).toEqual({
      status: 'unavailable',
      reason: 'no_webgl_context',
    });
    expect(
      probeGpuTimers({
        getExtension: () => null,
        getParameter: () => 0,
      }),
    ).toEqual({ status: 'unavailable', reason: 'webgl2_queries_unsupported' });

    const baseWebgl2 = {
      createQuery: vi.fn(),
      beginQuery: vi.fn(),
      endQuery: vi.fn(),
      getQueryParameter: vi.fn(),
      deleteQuery: vi.fn(),
      getQuery: () => 32,
      getParameter: () => false,
    };

    expect(probeGpuTimers({ ...baseWebgl2, getExtension: () => null })).toEqual({
      status: 'unavailable',
      reason: 'extension_absent',
    });

    const mockExt = {
      TIME_ELAPSED_EXT: 0x88bf,
      QUERY_COUNTER_BITS_EXT: 0x8864,
      GPU_DISJOINT_EXT: 0x8fbb,
    };

    expect(
      probeGpuTimers({
        ...baseWebgl2,
        getExtension: () => mockExt,
        getQuery: () => 0,
      }),
    ).toEqual({ status: 'unavailable', reason: 'zero_timer_bits' });

    expect(
      probeGpuTimers({
        ...baseWebgl2,
        getExtension: () => mockExt,
        getParameter: () => true,
      }),
    ).toEqual({ status: 'unavailable', reason: 'gpu_disjoint' });

    expect(
      probeGpuTimers({
        ...baseWebgl2,
        getExtension: () => mockExt,
      }),
    ).toEqual({ status: 'available', reason: 'ok' });
  });

  it('emits §6 JSON fields with no PII and never fabricates per-system timings', () => {
    const rec = new PerfRecorder(32);
    rec.recordFrame(25, makeCounters(1, 31, 400_000));
    const report = buildPerfReportJson(rec.summarize(), {
      gpu_timers: 'unavailable',
      quality_tier: 'low',
      agent_count: 1000,
      viewport_width: 1280,
      viewport_height: 720,
      device_pixel_ratio: 1,
      user_agent: 'Mozilla/5.0',
      app_version: '0.1.0',
      timestamp_iso: '2026-11-27T05:00:00.000Z',
    });

    expect(report.frames).toBe(1);
    expect(report.duration_ms).toBe(25);
    expect(report.median_fps).toBe(40);
    expect(report.p5_fps).toBe(40);
    expect(report.p1_fps).toBe(40);
    expect(report.worst_frame_ms).toBe(25);
    expect(report.draw_calls).toEqual({ peak: 31, median: 31, current: 31 });
    expect(report.triangles).toEqual({ peak: 400_000, current: 400_000 });
    expect(report.layer_peaks?.terrain.calls).toBe(1);
    expect(report.gpu_timers).toBe('unavailable');
    expect(report.per_system_ms).toBe('unavailable');
    expect(report.quality_tier).toBe('low');
    expect(report.agent_count).toBe(1000);
    expect(report.viewport).toEqual({ width: 1280, height: 720, device_pixel_ratio: 1 });
    expect(report.user_agent).toBe('Mozilla/5.0');
    expect(report.app_version).toBe('0.1.0');
    expect(report.timestamp_iso).toBe('2026-11-27T05:00:00.000Z');

    const serialized = JSON.stringify(report);
    expect(serialized).not.toMatch(/Users\/|C:\\|duongnad/i);
  });

  it('installs frame counters lazily only when not already mounted', () => {
    expect(ensureFrameCountersInstalled({}, {}, 1200, 0.14)).not.toBeNull();
    diagnostics()[CANONICAL_POSE_SETTER_GLOBAL] = () => {};
    expect(ensureFrameCountersInstalled({}, {}, 1200, 0.14)).toBeNull();
  });
});

const { frameCallbacks, shadowState, mockTauriInvoke, mockTauriListen } = vi.hoisted(() => ({
  frameCallbacks: [] as FrameCallback[],
  shadowState: { enabled: false, needsUpdate: false },
  mockTauriInvoke: vi.fn(),
  mockTauriListen: vi.fn(),
}));

vi.mock('@react-three/fiber', async () => {
  const three = await import('three');
  const mockScene = new three.Scene();
  const mockCamera = new three.PerspectiveCamera();
  const mockGl = {
    setSize: vi.fn(),
    domElement: document.createElement('canvas'),
    render: vi.fn(),
    getContext: () => ({
      disable: vi.fn(),
      DITHER: 0x0bd0,
      getExtension: () => null,
    }),
    shadowMap: shadowState,
  };
  const rootState = {
    clock: { getElapsedTime: () => 0, elapsedTime: 0 },
    scene: mockScene,
    camera: mockCamera,
    gl: mockGl,
    size: { width: 1280, height: 720 },
    setFrameloop: vi.fn(),
    setDpr: vi.fn(),
  };
  return {
    Canvas: ({ children }: { children?: React.ReactNode }) =>
      React.createElement('div', { 'data-testid': 'mock-canvas' }, children),
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

describe('WV2-6-INSTR WorldShowcase overlay integration & zero-cost idle gating', () => {
  beforeAll(warmSharedWorld, WARM_SHARED_WORLD_BUDGET_MS);

  beforeEach(() => {
    frameCallbacks.length = 0;
    mockTauriInvoke.mockReset();
    mockTauriListen.mockReset();
    shadowState.enabled = false;
    shadowState.needsUpdate = false;
    window.history.replaceState(null, '', '/landscape.html');
    installStubbedProperty(HTMLElement.prototype, 'setMatrixAt', vi.fn());
    installStubbedProperty(HTMLElement.prototype, 'setColorAt', vi.fn());
    Object.defineProperty(HTMLElement.prototype, 'instanceMatrix', {
      get(this: HTMLElement & { _piIM?: { needsUpdate: boolean } }) {
        if (!this._piIM) this._piIM = { needsUpdate: false };
        return this._piIM;
      },
      configurable: true,
    });
    Object.defineProperty(HTMLElement.prototype, 'instanceColor', {
      get(this: HTMLElement & { _piIC?: { needsUpdate: boolean } }) {
        if (!this._piIC) this._piIC = { needsUpdate: false };
        return this._piIC;
      },
      configurable: true,
    });
  });

  afterEach(() => {
    cleanup();
    removeStubbedProperties(
      HTMLElement.prototype,
      'setMatrixAt',
      'setColorAt',
      'instanceMatrix',
      'instanceColor',
    );
    delete diagnostics()[FRAME_COUNTERS_GLOBAL];
    delete diagnostics()[CANONICAL_POSE_SETTER_GLOBAL];
  });

  it('is hidden by default, mounts zero PerfRecorder callbacks until toggled via Perf or F3, and stays completely inert in capture mode', async () => {
    const view = render(React.createElement(WorldShowcase));
    const toggleBtn = await view.findByTestId('perf-toggle-btn');
    expect(view.queryByTestId('perf-overlay')).toBeNull();
    const idleFrameHookCount = frameCallbacks.length;

    await act(async () => {
      fireEvent.click(toggleBtn);
    });
    const overlay = await view.findByTestId('perf-overlay');
    expect(overlay.textContent).toContain('Median FPS');
    expect(overlay.textContent).toContain('p5 FPS');
    expect(overlay.textContent).toContain('Draw calls');
    expect(overlay.textContent).toContain('GPU timers: unavailable');
    await view.findByTestId('perf-record-60s-btn');
    await waitFor(() => {
      expect(frameCallbacks.length).toBeGreaterThan(idleFrameHookCount);
    });

    await act(async () => {
      fireEvent.keyDown(window, { key: 'F3' });
    });
    expect(view.queryByTestId('perf-overlay')).toBeNull();

    cleanup();
    window.history.replaceState(null, '', '/landscape.html?capture=1&view=overview');
    const captureView = render(React.createElement(WorldShowcase));
    expect(captureView.queryByTestId('perf-toggle-btn')).toBeNull();
    expect(captureView.queryByTestId('perf-overlay')).toBeNull();
    await act(async () => {
      fireEvent.keyDown(window, { key: 'F3' });
    });
    expect(captureView.queryByTestId('perf-overlay')).toBeNull();
  });
});
