import type { EcosystemState } from '../../../types/generated/EcosystemState';
import {
  CANONICAL_POSE_SETTER_GLOBAL,
  FRAME_COUNTER_LAYERS,
  diagnostics,
  installFrameCounters,
  readFrameCounters,
  type ClassifiedLayer,
  type FrameCountersHandle,
  type FrameRenderCounters,
  type LayerRenderCounts,
} from './diagnosticsWindow';
import type { QualityTier } from './qualityTiers';

export { readFrameCounters };

export const DEFAULT_PERF_CAPACITY = 16_384;
export const PERF_RECORD_DURATION_MS = 60_000;

export type GpuTimerState = 'available' | 'unavailable';

export interface GpuTimerProbe {
  status: GpuTimerState;
  reason: string;
}

export interface PerfSummary {
  frames: number;
  duration_ms: number;
  median_fps: number;
  p5_fps: number;
  p1_fps: number;
  p50_frame_ms: number;
  p95_frame_ms: number;
  p99_frame_ms: number;
  worst_frame_ms: number;
  draw_calls: { peak: number; median: number; current: number } | null;
  triangles: { peak: number; current: number } | null;
  layer_peaks: Record<ClassifiedLayer, LayerRenderCounts> | null;
}

export interface PerfReportContext {
  gpu_timers: GpuTimerState;
  quality_tier: QualityTier;
  agent_count: number | null;
  viewport_width: number;
  viewport_height: number;
  device_pixel_ratio: number;
  user_agent: string;
  app_version: string | null;
  timestamp_iso?: string;
}

export interface PerfReportJson {
  schema_version: 1;
  timestamp_iso: string;
  frames: number;
  duration_ms: number;
  median_fps: number;
  p5_fps: number;
  p1_fps: number;
  p50_frame_ms: number;
  p95_frame_ms: number;
  p99_frame_ms: number;
  worst_frame_ms: number;
  draw_calls: { peak: number; median: number; current: number } | null;
  triangles: { peak: number; current: number } | null;
  layer_peaks: Record<ClassifiedLayer, LayerRenderCounts> | null;
  gpu_timers: GpuTimerState;
  per_system_ms:
    | Record<'terrain' | 'vegetation' | 'water' | 'sky_atmosphere' | 'creatures' | 'post_upscale', 'unavailable'>
    | 'unavailable';
  quality_tier: QualityTier;
  agent_count: number | null;
  viewport: { width: number; height: number; device_pixel_ratio: number };
  user_agent: string;
  app_version: string | null;
}

/**
 * Nearest-rank percentile matching `tick_capture.rs::percentile_nearest_rank`:
 * returns the element at index `ceil(p / 100 * n) - 1` (never interpolates).
 */
export function percentileNearestRank(
  sorted: ArrayLike<number>,
  length: number,
  p: number,
): number {
  if (length <= 0) return 0;
  const rank = Math.ceil((p / 100) * length);
  const idx = rank < 1 ? 0 : Math.min(rank - 1, length - 1);
  return sorted[idx];
}

function emptyLayerPeaks(): Record<ClassifiedLayer, LayerRenderCounts> {
  return {
    terrain: { calls: 0, triangles: 0 },
    flora: { calls: 0, triangles: 0 },
    creatures: { calls: 0, triangles: 0 },
    water: { calls: 0, triangles: 0 },
    other: { calls: 0, triangles: 0 },
  };
}

/**
 * Pre-allocated fixed-capacity ring buffer for live rendering telemetry.
 * Zero heap allocations occur in `recordFrame` after construction.
 */
export class PerfRecorder {
  readonly capacity: number;
  private readonly deltasMs: Float64Array;
  private readonly callsBuf: Uint32Array;
  private readonly hasCallsBuf: Uint8Array;
  private readonly scratchF64: Float64Array;
  private readonly scratchU32: Uint32Array;

  private writeIdx = 0;
  private sampleCount = 0;
  private totalDurationMs = 0;
  private totalFrames = 0;

  private hasAnyCounters = false;
  private peakCalls = 0;
  private peakTriangles = 0;
  private currentCalls = 0;
  private currentTriangles = 0;
  private readonly layerPeaks: Record<ClassifiedLayer, LayerRenderCounts> = emptyLayerPeaks();

  constructor(capacity = DEFAULT_PERF_CAPACITY) {
    if (!Number.isInteger(capacity) || capacity <= 0) {
      throw new Error(`Invalid PerfRecorder capacity: ${String(capacity)}`);
    }
    this.capacity = capacity;
    this.deltasMs = new Float64Array(capacity);
    this.callsBuf = new Uint32Array(capacity);
    this.hasCallsBuf = new Uint8Array(capacity);
    this.scratchF64 = new Float64Array(capacity);
    this.scratchU32 = new Uint32Array(capacity);
  }

  get frameCount(): number {
    return this.totalFrames;
  }

  get bufferedCount(): number {
    return this.sampleCount;
  }

  get elapsedMs(): number {
    return this.totalDurationMs;
  }

  /** Direct typed-array handles for zero-allocation buffer assertions in unit tests. */
  getBackingBuffers(): {
    deltasMs: Float64Array;
    callsBuf: Uint32Array;
    hasCallsBuf: Uint8Array;
  } {
    return {
      deltasMs: this.deltasMs,
      callsBuf: this.callsBuf,
      hasCallsBuf: this.hasCallsBuf,
    };
  }

  reset(): void {
    this.writeIdx = 0;
    this.sampleCount = 0;
    this.totalDurationMs = 0;
    this.totalFrames = 0;
    this.hasAnyCounters = false;
    this.peakCalls = 0;
    this.peakTriangles = 0;
    this.currentCalls = 0;
    this.currentTriangles = 0;
    for (const k of FRAME_COUNTER_LAYERS) {
      this.layerPeaks[k].calls = 0;
      this.layerPeaks[k].triangles = 0;
    }
    this.layerPeaks.other.calls = 0;
    this.layerPeaks.other.triangles = 0;
  }

  recordFrame(deltaMs: number, counters: FrameRenderCounters | null): void {
    if (!Number.isFinite(deltaMs) || deltaMs <= 0) return;

    const i = this.writeIdx;
    if (this.sampleCount === this.capacity) {
      this.totalDurationMs -= this.deltasMs[i];
    } else {
      this.sampleCount += 1;
    }

    this.deltasMs[i] = deltaMs;
    this.totalDurationMs += deltaMs;
    this.totalFrames += 1;

    if (counters !== null) {
      this.hasAnyCounters = true;
      this.hasCallsBuf[i] = 1;
      const calls = counters.total.calls;
      const tris = counters.total.triangles;
      this.callsBuf[i] = calls;
      this.currentCalls = calls;
      this.currentTriangles = tris;
      if (calls > this.peakCalls) this.peakCalls = calls;
      if (tris > this.peakTriangles) this.peakTriangles = tris;

      for (let l = 0; l < FRAME_COUNTER_LAYERS.length; l += 1) {
        const layer = FRAME_COUNTER_LAYERS[l];
        const src = counters.layers[layer];
        const dst = this.layerPeaks[layer];
        if (src.calls > dst.calls) dst.calls = src.calls;
        if (src.triangles > dst.triangles) dst.triangles = src.triangles;
      }
      if (counters.other.calls > this.layerPeaks.other.calls) {
        this.layerPeaks.other.calls = counters.other.calls;
      }
      if (counters.other.triangles > this.layerPeaks.other.triangles) {
        this.layerPeaks.other.triangles = counters.other.triangles;
      }
    } else {
      this.hasCallsBuf[i] = 0;
      this.callsBuf[i] = 0;
    }

    this.writeIdx = i + 1 === this.capacity ? 0 : i + 1;
  }

  summarize(): PerfSummary {
    const n = this.sampleCount;
    if (n === 0) {
      return {
        frames: 0,
        duration_ms: 0,
        median_fps: 0,
        p5_fps: 0,
        p1_fps: 0,
        p50_frame_ms: 0,
        p95_frame_ms: 0,
        p99_frame_ms: 0,
        worst_frame_ms: 0,
        draw_calls: null,
        triangles: null,
        layer_peaks: null,
      };
    }

    let cLen = 0;
    for (let k = 0; k < n; k += 1) {
      this.scratchF64[k] = this.deltasMs[k];
      if (this.hasCallsBuf[k] === 1) {
        this.scratchU32[cLen] = this.callsBuf[k];
        cLen += 1;
      }
    }
    const sortedDeltas = this.scratchF64.subarray(0, n);
    sortedDeltas.sort();

    const p50 = percentileNearestRank(sortedDeltas, n, 50);
    const p95 = percentileNearestRank(sortedDeltas, n, 95);
    const p99 = percentileNearestRank(sortedDeltas, n, 99);
    const worst = sortedDeltas[n - 1];

    let medianCalls = 0;
    if (cLen > 0) {
      const sortedCalls = this.scratchU32.subarray(0, cLen);
      sortedCalls.sort();
      medianCalls = percentileNearestRank(sortedCalls, cLen, 50);
    }

    return {
      frames: n,
      duration_ms: Math.round(this.totalDurationMs * 100) / 100,
      median_fps: p50 > 0 ? round2(1000 / p50) : 0,
      p5_fps: p95 > 0 ? round2(1000 / p95) : 0,
      p1_fps: p99 > 0 ? round2(1000 / p99) : 0,
      p50_frame_ms: round2(p50),
      p95_frame_ms: round2(p95),
      p99_frame_ms: round2(p99),
      worst_frame_ms: round2(worst),
      draw_calls: this.hasAnyCounters
        ? { peak: this.peakCalls, median: medianCalls, current: this.currentCalls }
        : null,
      triangles: this.hasAnyCounters
        ? { peak: this.peakTriangles, current: this.currentTriangles }
        : null,
      layer_peaks: this.hasAnyCounters ? cloneLayerPeaks(this.layerPeaks) : null,
    };
  }
}

function round2(v: number): number {
  return Math.round(v * 100) / 100;
}

function cloneLayerPeaks(
  src: Record<ClassifiedLayer, LayerRenderCounts>,
): Record<ClassifiedLayer, LayerRenderCounts> {
  return {
    terrain: { calls: src.terrain.calls, triangles: src.terrain.triangles },
    flora: { calls: src.flora.calls, triangles: src.flora.triangles },
    creatures: { calls: src.creatures.calls, triangles: src.creatures.triangles },
    water: { calls: src.water.calls, triangles: src.water.triangles },
    other: { calls: src.other.calls, triangles: src.other.triangles },
  };
}

interface DisjointTimerExt {
  TIME_ELAPSED_EXT?: number;
  QUERY_COUNTER_BITS_EXT?: number;
  GPU_DISJOINT_EXT?: number;
}

interface WebGL2Like {
  getExtension?: (name: string) => unknown;
  getParameter?: (pname: number) => unknown;
  getQuery?: (target: number, pname: number) => unknown;
  createQuery?: () => unknown;
  beginQuery?: (target: number, query: unknown) => void;
  endQuery?: (target: number) => void;
  getQueryParameter?: (query: unknown, pname: number) => unknown;
  deleteQuery?: (query: unknown) => void;
}

function unwrapWebGLContext(gl: unknown): WebGL2Like | null {
  if (!gl || typeof gl !== 'object') return null;
  const maybe = gl as { getContext?: () => unknown };
  const ctx = typeof maybe.getContext === 'function' ? maybe.getContext() : gl;
  if (!ctx || typeof ctx !== 'object') return null;
  return ctx as WebGL2Like;
}

/**
 * Probe `EXT_disjoint_timer_query_webgl2` honestly. Reports `'unavailable'` whenever the
 * extension is absent, disjoint, has zero timer bits, or runs outside WebGL2.
 */
export function probeGpuTimers(gl: unknown): GpuTimerProbe {
  const ctx = unwrapWebGLContext(gl);
  if (!ctx || typeof ctx.getExtension !== 'function') {
    return { status: 'unavailable', reason: 'no_webgl_context' };
  }
  if (
    typeof ctx.createQuery !== 'function' ||
    typeof ctx.beginQuery !== 'function' ||
    typeof ctx.endQuery !== 'function' ||
    typeof ctx.getQueryParameter !== 'function' ||
    typeof ctx.deleteQuery !== 'function'
  ) {
    return { status: 'unavailable', reason: 'webgl2_queries_unsupported' };
  }
  const ext = ctx.getExtension('EXT_disjoint_timer_query_webgl2') as DisjointTimerExt | null;
  if (!ext || typeof ext !== 'object' || typeof ext.TIME_ELAPSED_EXT !== 'number') {
    return { status: 'unavailable', reason: 'extension_absent' };
  }
  if (typeof ext.QUERY_COUNTER_BITS_EXT === 'number' && typeof ctx.getQuery === 'function') {
    const bits = ctx.getQuery(ext.TIME_ELAPSED_EXT, ext.QUERY_COUNTER_BITS_EXT);
    if (typeof bits === 'number' && bits <= 0) {
      return { status: 'unavailable', reason: 'zero_timer_bits' };
    }
  }
  if (typeof ext.GPU_DISJOINT_EXT === 'number' && typeof ctx.getParameter === 'function') {
    if (ctx.getParameter(ext.GPU_DISJOINT_EXT)) {
      return { status: 'unavailable', reason: 'gpu_disjoint' };
    }
  }
  return { status: 'available', reason: 'ok' };
}

/** Ensure `__animaFrameCounters` is active while the overlay is open (production safety fallback). */
export function ensureFrameCountersInstalled(
  gl: unknown,
  scene: unknown,
  renderSize: number,
  heightRatio: number,
): FrameCountersHandle | null {
  if (typeof diagnostics()[CANONICAL_POSE_SETTER_GLOBAL] === 'function') {
    return null;
  }
  return installFrameCounters(gl, scene, { renderSize, heightRatio });
}

/** Read total live creatures from Tauri backend when running in the desktop app, else `null`. */
export async function fetchLiveAgentCount(): Promise<number | null> {
  if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) {
    return null;
  }
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const eco = await invoke<EcosystemState>('get_ecosystem_state');
    if (eco && typeof eco.prey_count === 'number' && typeof eco.predator_count === 'number') {
      return eco.prey_count + eco.predator_count;
    }
    return null;
  } catch {
    return null;
  }
}

/** Read build/app version without fabricating unconfigured values or PII. */
export async function readFrontendAppVersion(): Promise<string | null> {
  const env = (import.meta as { env?: Record<string, unknown> }).env;
  const envVer = env?.VITE_APP_VERSION ?? env?.VITE_GIT_SHA;
  if (typeof envVer === 'string' && envVer.trim().length > 0) {
    return envVer.trim();
  }
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
    try {
      const { getVersion } = await import('@tauri-apps/api/app');
      const v = await getVersion();
      return typeof v === 'string' && v.length > 0 ? v : null;
    } catch {
      return null;
    }
  }
  return null;
}

export function buildPerfReportJson(
  summary: PerfSummary,
  context: PerfReportContext,
): PerfReportJson {
  return {
    schema_version: 1,
    timestamp_iso: context.timestamp_iso ?? new Date().toISOString(),
    frames: summary.frames,
    duration_ms: summary.duration_ms,
    median_fps: summary.median_fps,
    p5_fps: summary.p5_fps,
    p1_fps: summary.p1_fps,
    p50_frame_ms: summary.p50_frame_ms,
    p95_frame_ms: summary.p95_frame_ms,
    p99_frame_ms: summary.p99_frame_ms,
    worst_frame_ms: summary.worst_frame_ms,
    draw_calls: summary.draw_calls,
    triangles: summary.triangles,
    layer_peaks: summary.layer_peaks,
    gpu_timers: context.gpu_timers,
    per_system_ms: 'unavailable',
    quality_tier: context.quality_tier,
    agent_count: context.agent_count,
    viewport: {
      width: context.viewport_width,
      height: context.viewport_height,
      device_pixel_ratio: context.device_pixel_ratio,
    },
    user_agent: context.user_agent,
    app_version: context.app_version,
  };
}
