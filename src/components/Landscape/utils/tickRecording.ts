import type { AgentType } from '../../../types/generated/AgentType';
import type { EnvironmentalElement } from '../../../types/generated/EnvironmentalElement';
import type { SegmentState } from '../../../types/generated/SegmentState';
import type { SimulationTickPayload } from '../../../types/generated/SimulationTickPayload';
import { isCaptureRequested } from './captureMode';
import {
  SYNTHETIC_REPLAY_PARAM,
  SYNTHETIC_SCREEN_LABEL,
  SYNTHETIC_TICK_INTERVAL_MS,
  createSyntheticTickGenerator,
  parseSyntheticReplaySearch,
  selectWorldV2LandAnchors,
  type SyntheticAnchorWorld,
  type SyntheticTicksOptions,
} from './syntheticTicks';

// ---------------------------------------------------------------------------------------
// Tick sources & 30-second real-tick ring recorder (RW0-1, RW0-3).
//
// # Why LiveAgents reads through TickSource
//
// `LiveAgents` previously hardcoded `@tauri-apps/api/event` `listen('simulation-tick')` inline,
// so any browser load without a running Rust backend drew nothing and could not replay recorded
// motion. Splitting tick delivery into `TickSource` lets the desktop app (`tauri`), recorded
// payloads (`recording`), and the explicit procedural fixture (`synthetic`) drive the exact
// same pose/render pipeline, while keeping `?capture=1` (with no `replay=`) returning `null`
// so canonical map captures stay byte-identical.
// ---------------------------------------------------------------------------------------

export const TICK_EMIT_INTERVAL_MS = 33;
export const DEFAULT_RECORDING_WINDOW_MS = 30_000;
export const MAX_RECORDING_FRAMES = 1200;

export interface RecordedTickFrame {
  offsetMs: number;
  payload: SimulationTickPayload;
}

export interface TickRecordingFile {
  schemaVersion: 1;
  kind: 'recording' | typeof SYNTHETIC_REPLAY_PARAM;
  recordedAtIso?: string;
  windowMs: number;
  frames: RecordedTickFrame[];
}

export type TickSourceKind = 'tauri' | typeof SYNTHETIC_REPLAY_PARAM | 'recording';
export type TickListener = (payload: SimulationTickPayload) => void;

export interface TickSource {
  readonly kind: TickSourceKind;
  readonly screenLabel: string | null;
  subscribe(listener: TickListener): () => void;
  mountOverlay?(host: HTMLElement): () => void;
}

export type TauriTickListenFn = (
  eventName: string,
  handler: (event: { payload: SimulationTickPayload }) => void,
) => Promise<() => void>;

interface StoredFrame {
  receivedAtMs: number;
  payload: SimulationTickPayload;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function cloneSegment(seg: SegmentState): SegmentState {
  return {
    agent_id: seg.agent_id,
    segment_id: seg.segment_id,
    parent_segment_id: seg.parent_segment_id,
    length: seg.length,
    radius: seg.radius,
    x: seg.x,
    y: seg.y,
    z: seg.z,
    yaw: seg.yaw,
    pitch: seg.pitch,
    roll: seg.roll,
    joint_anchor_x: seg.joint_anchor_x,
    joint_anchor_y: seg.joint_anchor_y,
    joint_anchor_z: seg.joint_anchor_z,
    joint_axis_x: seg.joint_axis_x,
    joint_axis_y: seg.joint_axis_y,
    joint_axis_z: seg.joint_axis_z,
    energy: seg.energy,
    hydration: seg.hydration,
    head_direction: [seg.head_direction[0], seg.head_direction[1], seg.head_direction[2]],
    agent_type: seg.agent_type,
  };
}

function cloneTickPayload(payload: SimulationTickPayload): SimulationTickPayload {
  const headDirections: Record<number, [number, number, number]> = {};
  for (const [key, dir] of Object.entries(payload.head_directions ?? {})) {
    if (dir) {
      headDirections[Number(key)] = [dir[0], dir[1], dir[2]];
    }
  }
  const elements: EnvironmentalElement[] = (payload.environmental_state?.elements ?? []).map(
    (el) => ({
      type: el.type,
      x: el.x,
      y: el.y,
      radius: el.radius,
      resources: el.resources,
    }),
  );
  return {
    segments: payload.segments.map(cloneSegment),
    environmental_state: { elements },
    head_directions: headDirections,
  };
}

function validateAgentType(raw: unknown, label: string): AgentType | null {
  if (raw === null || raw === undefined) return null;
  if (raw === 'prey' || raw === 'predator') return raw;
  throw new Error(`tick recording: invalid agent_type at ${label}`);
}

function validateSegment(raw: unknown, label: string): SegmentState {
  if (!isRecord(raw)) {
    throw new Error(`tick recording: expected object at ${label}`);
  }
  const numericFields = [
    'agent_id',
    'segment_id',
    'length',
    'radius',
    'x',
    'y',
    'z',
    'yaw',
    'pitch',
    'roll',
    'joint_anchor_x',
    'joint_anchor_y',
    'joint_anchor_z',
    'joint_axis_x',
    'joint_axis_y',
    'joint_axis_z',
    'energy',
    'hydration',
  ] as const;
  for (const field of numericFields) {
    if (!isFiniteNumber(raw[field])) {
      throw new Error(`tick recording: invalid numeric field "${field}" at ${label}`);
    }
  }
  const parentId = raw.parent_segment_id;
  if (parentId !== null && parentId !== undefined && !isFiniteNumber(parentId)) {
    throw new Error(`tick recording: invalid parent_segment_id at ${label}`);
  }
  const hd = raw.head_direction;
  if (
    !Array.isArray(hd) ||
    hd.length !== 3 ||
    !isFiniteNumber(hd[0]) ||
    !isFiniteNumber(hd[1]) ||
    !isFiniteNumber(hd[2])
  ) {
    throw new Error(`tick recording: invalid head_direction at ${label}`);
  }

  return {
    agent_id: raw.agent_id as number,
    segment_id: raw.segment_id as number,
    parent_segment_id: parentId === undefined ? null : (parentId as number | null),
    length: raw.length as number,
    radius: raw.radius as number,
    x: raw.x as number,
    y: raw.y as number,
    z: raw.z as number,
    yaw: raw.yaw as number,
    pitch: raw.pitch as number,
    roll: raw.roll as number,
    joint_anchor_x: raw.joint_anchor_x as number,
    joint_anchor_y: raw.joint_anchor_y as number,
    joint_anchor_z: raw.joint_anchor_z as number,
    joint_axis_x: raw.joint_axis_x as number,
    joint_axis_y: raw.joint_axis_y as number,
    joint_axis_z: raw.joint_axis_z as number,
    energy: raw.energy as number,
    hydration: raw.hydration as number,
    head_direction: [hd[0], hd[1], hd[2]],
    agent_type: validateAgentType(raw.agent_type, label),
  };
}

function validatePayload(raw: unknown, label: string): SimulationTickPayload {
  if (!isRecord(raw) || !Array.isArray(raw.segments)) {
    throw new Error(`tick recording: expected payload with segments array at ${label}`);
  }
  const segments = raw.segments.map((seg, idx) => validateSegment(seg, `${label}.segments[${idx}]`));

  const elements: EnvironmentalElement[] = [];
  if (raw.environmental_state !== undefined) {
    if (!isRecord(raw.environmental_state) || !Array.isArray(raw.environmental_state.elements)) {
      throw new Error(`tick recording: invalid environmental_state at ${label}`);
    }
    for (let i = 0; i < raw.environmental_state.elements.length; i++) {
      const el = raw.environmental_state.elements[i];
      if (
        !isRecord(el) ||
        typeof el.type !== 'string' ||
        !isFiniteNumber(el.x) ||
        !isFiniteNumber(el.y) ||
        !isFiniteNumber(el.radius) ||
        !isFiniteNumber(el.resources)
      ) {
        throw new Error(`tick recording: invalid environmental element at ${label}[${i}]`);
      }
      elements.push({
        type: el.type,
        x: el.x,
        y: el.y,
        radius: el.radius,
        resources: el.resources,
      });
    }
  }

  const headDirections: Record<number, [number, number, number]> = {};
  if (raw.head_directions !== undefined) {
    if (!isRecord(raw.head_directions)) {
      throw new Error(`tick recording: invalid head_directions map at ${label}`);
    }
    for (const [k, v] of Object.entries(raw.head_directions)) {
      const numericKey = Number(k);
      if (
        !Number.isFinite(numericKey) ||
        !Array.isArray(v) ||
        v.length !== 3 ||
        !isFiniteNumber(v[0]) ||
        !isFiniteNumber(v[1]) ||
        !isFiniteNumber(v[2])
      ) {
        throw new Error(`tick recording: invalid head_directions entry "${k}" at ${label}`);
      }
      headDirections[numericKey] = [v[0], v[1], v[2]];
    }
  }

  return {
    segments,
    environmental_state: { elements },
    head_directions: headDirections,
  };
}

/**
 * Rolling 30-second ring buffer of incoming `simulation-tick` payloads (`RW0-3`).
 * Deep-clones payloads on push so reused emitters or mutable caller references never corrupt history.
 */
export class TickRecorder {
  readonly windowMs: number;
  private readonly nowMs: () => number;
  private readonly frames: StoredFrame[] = [];

  constructor(
    windowMs: number = DEFAULT_RECORDING_WINDOW_MS,
    nowMs: () => number = () =>
      typeof performance !== 'undefined' ? performance.now() : Date.now(),
  ) {
    if (!Number.isFinite(windowMs) || windowMs < 0) {
      throw new Error(`TickRecorder: windowMs must be a non-negative number (got ${windowMs})`);
    }
    this.windowMs = windowMs;
    this.nowMs = nowMs;
  }

  push(payload: SimulationTickPayload, receivedAtMs: number = this.nowMs()): void {
    if (!Number.isFinite(receivedAtMs)) return;
    this.frames.push({
      receivedAtMs,
      payload: cloneTickPayload(payload),
    });
    while (
      this.frames.length > 1 &&
      receivedAtMs - this.frames[0].receivedAtMs > this.windowMs
    ) {
      this.frames.shift();
    }
    while (this.frames.length > MAX_RECORDING_FRAMES) {
      this.frames.shift();
    }
  }

  clear(): void {
    this.frames.length = 0;
  }

  frameCount(): number {
    return this.frames.length;
  }

  durationMs(): number {
    if (this.frames.length < 2) return 0;
    const first = this.frames[0].receivedAtMs;
    const last = this.frames[this.frames.length - 1].receivedAtMs;
    return Math.max(0, Math.round(last - first));
  }

  snapshot(recordedAtIso?: string): TickRecordingFile {
    const first = this.frames.length > 0 ? this.frames[0].receivedAtMs : 0;
    const frames: RecordedTickFrame[] = this.frames.map((entry) => ({
      offsetMs: Math.max(0, Math.round(entry.receivedAtMs - first)),
      payload: cloneTickPayload(entry.payload),
    }));
    const result: TickRecordingFile = {
      schemaVersion: 1,
      kind: 'recording',
      windowMs: this.windowMs,
      frames,
    };
    if (recordedAtIso !== undefined) {
      result.recordedAtIso = recordedAtIso;
    }
    return result;
  }

  toJson(recordedAtIso?: string): string {
    return JSON.stringify(this.snapshot(recordedAtIso));
  }
}

/**
 * Parse a JSON tick recording file (either a `TickRecordingFile`, a `SyntheticTickFixture`,
 * or an array of raw `SimulationTickPayload` objects). Preserves `kind: 'synthetic'` so
 * synthetic fixtures cannot be disguised as simulation output.
 */
export function parseTickRecordingJson(jsonText: string): TickRecordingFile {
  let parsed: unknown;
  let parseErrorMessage: string | null = null;
  try {
    parsed = JSON.parse(jsonText);
  } catch (err) {
    parseErrorMessage = err instanceof Error ? err.message : String(err);
  }
  if (parseErrorMessage !== null) {
    throw new Error(`tick recording: invalid JSON (${parseErrorMessage})`);
  }

  if (Array.isArray(parsed)) {
    const frames = parsed.map((item, idx) => ({
      offsetMs: idx * TICK_EMIT_INTERVAL_MS,
      payload: validatePayload(item, `frames[${idx}]`),
    }));
    return {
      schemaVersion: 1,
      kind: 'recording',
      windowMs: DEFAULT_RECORDING_WINDOW_MS,
      frames,
    };
  }

  if (!isRecord(parsed) || !Array.isArray(parsed.frames)) {
    throw new Error('tick recording: expected an object with a frames array or a payload array');
  }

  const isSynthetic =
    parsed.kind === SYNTHETIC_REPLAY_PARAM || parsed.synthetic === true;
  const kind: TickRecordingFile['kind'] = isSynthetic ? SYNTHETIC_REPLAY_PARAM : 'recording';

  const frames: RecordedTickFrame[] = parsed.frames.map((entry, idx) => {
    if (!isRecord(entry) || !isFiniteNumber(entry.offsetMs) || entry.offsetMs < 0) {
      throw new Error(`tick recording: invalid frame offsetMs at frames[${idx}]`);
    }
    return {
      offsetMs: entry.offsetMs,
      payload: validatePayload(entry.payload, `frames[${idx}].payload`),
    };
  });

  const windowMs = isFiniteNumber(parsed.windowMs) && parsed.windowMs >= 0
    ? parsed.windowMs
    : DEFAULT_RECORDING_WINDOW_MS;

  const result: TickRecordingFile = {
    schemaVersion: 1,
    kind,
    windowMs,
    frames,
  };
  if (typeof parsed.recordedAtIso === 'string') {
    result.recordedAtIso = parsed.recordedAtIso;
  }
  return result;
}

function defaultTauriListen(): TauriTickListenFn | null {
  if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) {
    return null;
  }
  return async (eventName, handler) => {
    const { listen } = await import('@tauri-apps/api/event');
    return listen<SimulationTickPayload>(eventName, handler);
  };
}

function triggerJsonDownload(json: string, filename: string): void {
  if (typeof document === 'undefined' || typeof URL === 'undefined') return;
  if (typeof URL.createObjectURL !== 'function') return;
  const blob = new Blob([json], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  link.style.display = 'none';
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  if (typeof URL.revokeObjectURL === 'function') {
    URL.revokeObjectURL(url);
  }
}

function mountRecorderOverlay(host: HTMLElement, recorder: TickRecorder): () => void {
  const doc = host.ownerDocument;
  const root = doc.createElement('div');
  root.setAttribute('data-testid', 'rw-tick-recorder-hud');
  Object.assign(root.style, {
    position: 'absolute',
    right: '14px',
    bottom: '14px',
    zIndex: '200',
    display: 'flex',
    alignItems: 'center',
    gap: '8px',
    padding: '8px 12px',
    borderRadius: '8px',
    background: 'rgba(15, 23, 42, 0.82)',
    color: '#f8fafc',
    fontFamily: 'sans-serif',
    fontSize: '12px',
    border: '1px solid rgba(248, 113, 113, 0.6)',
  });

  const status = doc.createElement('span');
  status.setAttribute('data-testid', 'rw-tick-recorder-status');

  const exportBtn = doc.createElement('button');
  exportBtn.type = 'button';
  exportBtn.setAttribute('data-testid', 'rw-tick-recorder-export');
  exportBtn.textContent = 'Export JSON';
  Object.assign(exportBtn.style, {
    padding: '4px 8px',
    borderRadius: '4px',
    border: '1px solid rgba(148, 163, 184, 0.5)',
    background: 'rgba(30, 41, 59, 0.9)',
    color: '#f8fafc',
    cursor: 'pointer',
    fontSize: '11px',
  });

  const copyBtn = doc.createElement('button');
  copyBtn.type = 'button';
  copyBtn.setAttribute('data-testid', 'rw-tick-recorder-copy');
  copyBtn.textContent = 'Copy JSON';
  Object.assign(copyBtn.style, {
    padding: '4px 8px',
    borderRadius: '4px',
    border: '1px solid rgba(148, 163, 184, 0.5)',
    background: 'rgba(30, 41, 59, 0.9)',
    color: '#f8fafc',
    cursor: 'pointer',
    fontSize: '11px',
  });

  const updateStatus = (note?: string) => {
    const secs = (recorder.durationMs() / 1000).toFixed(1);
    const base = `REC ${Math.round(recorder.windowMs / 1000)}s ring · ${recorder.frameCount()} ticks (${secs}s)`;
    status.textContent = note ? `${base} · ${note}` : base;
  };

  exportBtn.addEventListener('click', () => {
    const iso = new Date().toISOString();
    const stamp = iso.replace(/[:.]/g, '-').slice(0, 19);
    triggerJsonDownload(recorder.toJson(iso), `anima_tick_recording_${stamp}.json`);
    updateStatus('exported');
  });

  copyBtn.addEventListener('click', () => {
    const json = recorder.toJson(new Date().toISOString());
    if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
      void navigator.clipboard
        .writeText(json)
        .then(() => updateStatus('copied'))
        .catch(() => updateStatus('copy unavailable'));
    } else {
      updateStatus('clipboard unavailable');
    }
  });

  updateStatus();
  root.append(status, exportBtn, copyBtn);
  host.appendChild(root);

  const timer = setInterval(() => updateStatus(), 250);
  return () => {
    clearInterval(timer);
    root.remove();
  };
}

/**
 * Tick source backed by Tauri's `simulation-tick` event (`RW0-1`).
 * Optionally feeds incoming payloads into a `TickRecorder` (`RW0-3`) when `?record=1`.
 */
export function createTauriTickSource(options?: {
  recorder?: TickRecorder | null;
  listenImpl?: TauriTickListenFn | null;
}): TickSource {
  const recorder = options?.recorder ?? null;

  return {
    kind: 'tauri',
    screenLabel: null,
    subscribe(listener: TickListener): () => void {
      const listenFn =
        options?.listenImpl !== undefined ? options.listenImpl : defaultTauriListen();
      if (!listenFn) return () => {};

      let unlisten: (() => void) | undefined;
      let cancelled = false;

      void listenFn('simulation-tick', (event) => {
        if (cancelled) return;
        recorder?.push(event.payload);
        listener(event.payload);
      })
        .then((stop) => {
          if (cancelled) stop();
          else unlisten = stop;
        })
        .catch(() => {
          // A transport that is present but refuses to subscribe leaves the scene as it was: empty.
          // Nothing is logged, because `console_hygiene.spec.ts` holds this scene to zero
          // Anima-owned console output and an unrunning backend is not a defect to report.
        });

      return () => {
        cancelled = true;
        unlisten?.();
      };
    },
    mountOverlay: recorder
      ? (host: HTMLElement) => mountRecorderOverlay(host, recorder)
      : undefined,
  };
}

function normalizeReplayFrames(
  input: readonly RecordedTickFrame[] | readonly SimulationTickPayload[],
  intervalMs: number,
): RecordedTickFrame[] {
  if (input.length === 0) return [];
  const first = input[0] as RecordedTickFrame | SimulationTickPayload;
  if ('payload' in first && 'offsetMs' in first) {
    return (input as readonly RecordedTickFrame[]).map((f) => ({
      offsetMs: f.offsetMs,
      payload: f.payload,
    }));
  }
  return (input as readonly SimulationTickPayload[]).map((payload, idx) => ({
    offsetMs: idx * intervalMs,
    payload,
  }));
}

function mountReplayBanner(
  host: HTMLElement,
  kind: typeof SYNTHETIC_REPLAY_PARAM | 'recording',
  text: string,
): () => void {
  const doc = host.ownerDocument;
  const banner = doc.createElement('div');
  banner.setAttribute(
    'data-testid',
    kind === SYNTHETIC_REPLAY_PARAM ? 'rw-synthetic-banner' : 'rw-tick-replay-hud',
  );
  Object.assign(banner.style, {
    position: 'absolute',
    left: '50%',
    bottom: '14px',
    transform: 'translateX(-50%)',
    zIndex: '200',
    padding: '6px 12px',
    borderRadius: '8px',
    background: 'rgba(15, 23, 42, 0.82)',
    color: kind === SYNTHETIC_REPLAY_PARAM ? '#fde68a' : '#bae6fd',
    border:
      kind === SYNTHETIC_REPLAY_PARAM
        ? '1px solid rgba(251, 191, 36, 0.6)'
        : '1px solid rgba(56, 189, 248, 0.5)',
    fontFamily: 'sans-serif',
    fontSize: '12px',
    pointerEvents: 'none',
    userSelect: 'none',
  });
  banner.textContent = text;
  host.appendChild(banner);
  return () => banner.remove();
}

/**
 * Tick source backed by a pre-recorded or fixed sequence of payloads (`RW0-1`).
 * Emits frame 0 immediately upon subscribe and advances through subsequent frames by offset.
 */
export function createReplayTickSource(
  input: readonly RecordedTickFrame[] | readonly SimulationTickPayload[],
  options?: {
    kind?: typeof SYNTHETIC_REPLAY_PARAM | 'recording';
    screenLabel?: string | null;
    intervalMs?: number;
    loop?: boolean;
  },
): TickSource {
  const intervalMs = options?.intervalMs ?? TICK_EMIT_INTERVAL_MS;
  const loop = options?.loop ?? true;
  const kind = options?.kind ?? 'recording';
  const rawLabel = options?.screenLabel;
  const screenLabel =
    kind === SYNTHETIC_REPLAY_PARAM
      ? rawLabel && rawLabel.toLowerCase().includes(SYNTHETIC_REPLAY_PARAM)
        ? rawLabel
        : SYNTHETIC_SCREEN_LABEL
      : rawLabel ?? null;
  const frames = normalizeReplayFrames(input, intervalMs);

  return {
    kind,
    screenLabel,
    subscribe(listener: TickListener): () => void {
      if (frames.length === 0) return () => {};

      let stopped = false;
      let index = 0;
      let timer: ReturnType<typeof setTimeout> | undefined;

      const scheduleNext = () => {
        if (stopped || frames.length <= 1) return;
        let nextIndex = index + 1;
        let delay: number;
        if (nextIndex >= frames.length) {
          if (!loop) return;
          nextIndex = 0;
          delay = intervalMs;
        } else {
          const delta = frames[nextIndex].offsetMs - frames[index].offsetMs;
          delay = delta > 0 ? Math.max(1, delta) : 1;
        }
        timer = setTimeout(() => {
          if (stopped) return;
          index = nextIndex;
          listener(frames[index].payload);
          scheduleNext();
        }, delay);
      };

      listener(frames[0].payload);
      scheduleNext();

      return () => {
        stopped = true;
        if (timer !== undefined) clearTimeout(timer);
      };
    },
    mountOverlay:
      screenLabel !== null
        ? (host: HTMLElement) => mountReplayBanner(host, kind, screenLabel)
        : undefined,
  };
}

/**
 * Deterministic SYNTHETIC creature tick source (`RW0-2`).
 * Always displays `SYNTHETIC` on screen so it is never confused with simulation output.
 */
export function createSyntheticTickSource(opts: SyntheticTicksOptions = {}): TickSource {
  const gen = createSyntheticTickGenerator(opts);

  return {
    kind: SYNTHETIC_REPLAY_PARAM,
    screenLabel: gen.screenLabel,
    subscribe(listener: TickListener): () => void {
      let stopped = false;
      let tickIndex = 0;

      listener(gen.tickAt(tickIndex));

      const timer = setInterval(() => {
        if (stopped) return;
        tickIndex = (tickIndex + 1) % gen.options.ticks;
        listener(gen.tickAt(tickIndex));
      }, SYNTHETIC_TICK_INTERVAL_MS);

      return () => {
        stopped = true;
        clearInterval(timer);
      };
    },
    mountOverlay: (host: HTMLElement) =>
      mountReplayBanner(host, SYNTHETIC_REPLAY_PARAM, gen.screenLabel),
  };
}

function isAllowedReplayUrl(fileParam: string): boolean {
  if (!fileParam || fileParam.startsWith('//')) return false;
  if (typeof window === 'undefined' || !window.location?.origin) {
    return !/^[a-zA-Z][a-zA-Z\d+\-.]*:/.test(fileParam);
  }
  try {
    const parsed = new URL(fileParam, window.location.origin);
    return parsed.origin === window.location.origin;
  } catch {
    return false;
  }
}

/**
 * Tick source for `?replay=<file>` (`RW0-3`): fetches a same-origin JSON recording URL
 * or accepts a local JSON file via the overlay file input (e.g. `owner_rw_recording_*.json`).
 */
export function createRecordingFileTickSource(
  fileParam: string,
  fetchImpl?: typeof fetch,
): TickSource {
  let activeListener: TickListener | null = null;
  let stopInnerReplay: (() => void) | undefined;
  let bannerEl: HTMLElement | null = null;
  let statusEl: HTMLElement | null = null;
  let currentKind: typeof SYNTHETIC_REPLAY_PARAM | 'recording' = 'recording';
  let currentStatus = `Replay: ${fileParam}`;

  const syncHud = () => {
    if (bannerEl) {
      bannerEl.setAttribute(
        'data-testid',
        currentKind === SYNTHETIC_REPLAY_PARAM ? 'rw-synthetic-banner' : 'rw-tick-replay-hud',
      );
      bannerEl.style.color = currentKind === SYNTHETIC_REPLAY_PARAM ? '#fde68a' : '#bae6fd';
      bannerEl.style.borderColor =
        currentKind === SYNTHETIC_REPLAY_PARAM
          ? 'rgba(251, 191, 36, 0.6)'
          : 'rgba(56, 189, 248, 0.5)';
    }
    if (statusEl) {
      statusEl.textContent = currentStatus;
    }
  };

  const applyRecordingText = (text: string, sourceName: string) => {
    const recording = parseTickRecordingJson(text);
    stopInnerReplay?.();
    stopInnerReplay = undefined;

    currentKind = recording.kind;
    const durationSec =
      recording.frames.length > 1
        ? (
            (recording.frames[recording.frames.length - 1].offsetMs -
              recording.frames[0].offsetMs) /
            1000
          ).toFixed(1)
        : '0.0';

    currentStatus =
      recording.kind === SYNTHETIC_REPLAY_PARAM
        ? `${SYNTHETIC_SCREEN_LABEL} · ${sourceName} (${recording.frames.length} ticks)`
        : `Recording replay · ${sourceName} (${recording.frames.length} ticks, ${durationSec}s)`;
    syncHud();

    if (activeListener) {
      const inner = createReplayTickSource(recording.frames, {
        kind: recording.kind,
        loop: true,
      });
      stopInnerReplay = inner.subscribe(activeListener);
    }
  };

  return {
    kind: 'recording',
    screenLabel: currentStatus,
    subscribe(listener: TickListener): () => void {
      let cancelled = false;
      activeListener = listener;

      const doFetch = fetchImpl ?? (typeof fetch === 'function' ? fetch : undefined);
      if (doFetch && fileParam !== 'picker' && isAllowedReplayUrl(fileParam)) {
        void doFetch(fileParam)
          .then(async (res) => {
            if (!res.ok) {
              throw new Error(`HTTP ${res.status}`);
            }
            return res.text();
          })
          .then((text) => {
            if (!cancelled) applyRecordingText(text, fileParam);
          })
          .catch((err) => {
            if (cancelled) return;
            currentStatus = `Replay not loaded (${err instanceof Error ? err.message : String(err)}) — choose JSON`;
            syncHud();
          });
      } else if (!isAllowedReplayUrl(fileParam) && fileParam !== 'picker') {
        currentStatus = 'Replay URL must be same-origin — choose local JSON';
        syncHud();
      }

      return () => {
        cancelled = true;
        activeListener = null;
        stopInnerReplay?.();
        stopInnerReplay = undefined;
      };
    },
    mountOverlay(host: HTMLElement): () => void {
      const doc = host.ownerDocument;
      const root = doc.createElement('div');
      Object.assign(root.style, {
        position: 'absolute',
        left: '50%',
        bottom: '14px',
        transform: 'translateX(-50%)',
        zIndex: '200',
        display: 'flex',
        alignItems: 'center',
        gap: '10px',
        padding: '6px 12px',
        borderRadius: '8px',
        background: 'rgba(15, 23, 42, 0.82)',
        color: '#bae6fd',
        border: '1px solid rgba(56, 189, 248, 0.5)',
        fontFamily: 'sans-serif',
        fontSize: '12px',
      });

      const status = doc.createElement('span');
      status.setAttribute('data-testid', 'rw-tick-replay-status');

      const label = doc.createElement('label');
      label.textContent = 'Load JSON';
      Object.assign(label.style, {
        padding: '3px 8px',
        borderRadius: '4px',
        border: '1px solid rgba(148, 163, 184, 0.5)',
        background: 'rgba(30, 41, 59, 0.9)',
        color: '#f8fafc',
        cursor: 'pointer',
        fontSize: '11px',
      });

      const input = doc.createElement('input');
      input.type = 'file';
      input.accept = '.json,application/json';
      input.setAttribute('data-testid', 'rw-tick-replay-file-input');
      input.style.display = 'none';
      input.addEventListener('change', () => {
        const file = input.files?.[0];
        if (!file) return;
        void file
          .text()
          .then((text) => applyRecordingText(text, file.name))
          .catch((err) => {
            currentStatus = `Invalid recording (${err instanceof Error ? err.message : String(err)})`;
            syncHud();
          });
      });

      label.appendChild(input);
      root.append(status, label);
      host.appendChild(root);

      bannerEl = root;
      statusEl = status;
      syncHud();

      return () => {
        bannerEl = null;
        statusEl = null;
        root.remove();
      };
    },
  };
}

/**
 * Matches `resolveWorldSourceMode(search, envMode) === 'v2'` without importing the World v2
 * tile loader/codec graph into `tickRecording`.
 */
export function isWorldV2Search(
  search: string,
  envMode = (import.meta as { env?: { MODE?: string } }).env?.MODE,
): boolean {
  const params = new URLSearchParams(search);
  const explicit = params.get('world');
  if (explicit === 'v2') return true;
  if (explicit === 'v1') return false;
  if (params.get('capture') === '1') return false;
  if (envMode === 'test') return false;
  return true;
}

/**
 * Resolve the active `TickSource` from a URL query string.
 * - `?replay=synthetic&agents=N` -> deterministic synthetic fixture (`RW0-2`, using deterministic
 *   World v2 land anchors when running on World v2)
 * - `?replay=<file>` -> recorded JSON replay (`RW0-3`)
 * - `?capture=1` (with no `replay`) -> `null` so canonical map captures stay creature-free
 * - default (or `?record=1`) -> Tauri `simulation-tick` source (`RW0-1`, optionally recording)
 */
export function resolveTickSourceFromSearch(
  search: string,
  world?: SyntheticAnchorWorld | null,
): TickSource | null {
  const syntheticOpts = parseSyntheticReplaySearch(search);
  if (syntheticOpts !== null) {
    if (isWorldV2Search(search)) {
      const candidateWorld =
        world !== undefined
          ? world
          : typeof window !== 'undefined'
            ? (window.__world ?? null)
            : null;
      const anchors = selectWorldV2LandAnchors(candidateWorld);
      return createSyntheticTickSource({ ...syntheticOpts, anchors });
    }
    return createSyntheticTickSource(syntheticOpts);
  }

  const params = new URLSearchParams(search);
  const replay = params.get('replay');
  if (replay !== null && replay.trim() !== '') {
    return createRecordingFileTickSource(replay.trim());
  }

  if (isCaptureRequested(search)) {
    return null;
  }

  const recorder = params.get('record') === '1' ? new TickRecorder() : null;
  return createTauriTickSource({ recorder });
}
