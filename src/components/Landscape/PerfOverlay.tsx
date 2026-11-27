import React, { useCallback, useEffect, useRef, useState } from 'react';
import type { QualityTier } from './utils/qualityTiers';
import {
  PERF_RECORD_DURATION_MS,
  PerfRecorder,
  buildPerfReportJson,
  ensureFrameCountersInstalled,
  fetchLiveAgentCount,
  probeGpuTimers,
  readFrameCounters,
  readFrontendAppVersion,
  type GpuTimerState,
  type PerfSummary,
} from './utils/perfRecorder';

export interface PerfCanvasBridge {
  gl: unknown;
  scene: unknown;
  size: { width: number; height: number };
  onFrame: (() => void) | null;
}

interface PerfOverlaySnapshot {
  summary: PerfSummary;
  recording: boolean;
  elapsedMs: number;
  gpuTimers: GpuTimerState;
  reportJson: string | null;
  copyState: 'idle' | 'copied' | 'manual';
}

const PERF_BTN = (active: boolean): React.CSSProperties => ({
  padding: '4px 10px',
  borderRadius: 6,
  border: active ? '1px solid #7dd3fc' : '1px solid #334155',
  background: active ? 'rgba(56,189,248,0.25)' : 'rgba(15,23,42,0.6)',
  color: '#e2e8f0',
  cursor: 'pointer',
  fontSize: 12,
  textTransform: 'capitalize',
});

function setBridgeFrameHandler(bridge: PerfCanvasBridge, fn: (() => void) | null): void {
  bridge.onFrame = fn;
}

const PerfOverlay: React.FC<{
  bridgeRef: React.MutableRefObject<PerfCanvasBridge>;
  qualityTier: QualityTier;
  renderSize: number;
  heightRatio: number;
  onClose: () => void;
}> = ({ bridgeRef, qualityTier, renderSize, heightRatio, onClose }) => {
  const [snapshot, setSnapshot] = useState<PerfOverlaySnapshot | null>(null);
  const recorderRef = useRef<PerfRecorder | null>(null);
  const qualityTierRef = useRef<QualityTier>(qualityTier);
  const gpuStateRef = useRef<GpuTimerState>('unavailable');
  const lastNowRef = useRef(0);
  const lastEmitRef = useRef(0);
  const recordingRef = useRef(false);
  const reportJsonRef = useRef<string | null>(null);
  const copyStateRef = useRef<'idle' | 'copied' | 'manual'>('idle');
  const agentCountRef = useRef<number | null>(null);
  const appVersionRef = useRef<string | null>(null);
  const mountedRef = useRef(true);

  useEffect(() => {
    qualityTierRef.current = qualityTier;
  }, [qualityTier]);

  useEffect(() => {
    mountedRef.current = true;
    const rec = new PerfRecorder();
    recorderRef.current = rec;
    const bridge = bridgeRef.current;
    const handle = ensureFrameCountersInstalled(bridge.gl, bridge.scene, renderSize, heightRatio);
    gpuStateRef.current = probeGpuTimers(bridge.gl).status;
    lastNowRef.current = 0;
    lastEmitRef.current = 0;
    recordingRef.current = false;
    reportJsonRef.current = null;
    copyStateRef.current = 'idle';

    setSnapshot({
      summary: rec.summarize(),
      recording: false,
      elapsedMs: 0,
      gpuTimers: gpuStateRef.current,
      reportJson: null,
      copyState: 'idle',
    });

    setBridgeFrameHandler(bridge, () => {
      const now = performance.now();
      const prev = lastNowRef.current;
      lastNowRef.current = now;
      if (prev <= 0) return;

      const dtMs = now - prev;
      rec.recordFrame(dtMs, readFrameCounters());

      if (recordingRef.current && rec.elapsedMs >= PERF_RECORD_DURATION_MS) {
        recordingRef.current = false;
        const summary = rec.summarize();
        const curSize = bridgeRef.current.size;
        const report = buildPerfReportJson(summary, {
          gpu_timers: gpuStateRef.current,
          quality_tier: qualityTierRef.current,
          agent_count: agentCountRef.current,
          viewport_width: curSize.width,
          viewport_height: curSize.height,
          device_pixel_ratio: typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1,
          user_agent: typeof navigator !== 'undefined' ? navigator.userAgent : '',
          app_version: appVersionRef.current,
        });
        const json = JSON.stringify(report, null, 2);
        reportJsonRef.current = json;
        lastEmitRef.current = now;
        if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
          copyStateRef.current = 'copied';
          void navigator.clipboard.writeText(json).catch(() => {
            copyStateRef.current = 'manual';
            if (mountedRef.current) {
              setSnapshot({
                summary,
                recording: false,
                elapsedMs: summary.duration_ms,
                gpuTimers: gpuStateRef.current,
                reportJson: json,
                copyState: 'manual',
              });
            }
          });
        } else {
          copyStateRef.current = 'manual';
        }
        setSnapshot({
          summary,
          recording: false,
          elapsedMs: summary.duration_ms,
          gpuTimers: gpuStateRef.current,
          reportJson: json,
          copyState: copyStateRef.current,
        });
        return;
      }

      if (now - lastEmitRef.current >= 250) {
        lastEmitRef.current = now;
        setSnapshot({
          summary: rec.summarize(),
          recording: recordingRef.current,
          elapsedMs: rec.elapsedMs,
          gpuTimers: gpuStateRef.current,
          reportJson: reportJsonRef.current,
          copyState: copyStateRef.current,
        });
      }
    });

    return () => {
      mountedRef.current = false;
      setBridgeFrameHandler(bridge, null);
      recorderRef.current = null;
      handle?.dispose();
    };
  }, [bridgeRef, renderSize, heightRatio]);

  const handleRecord60s = useCallback(() => {
    const rec = recorderRef.current;
    if (!rec || recordingRef.current) return;
    rec.reset();
    lastNowRef.current = 0;
    recordingRef.current = true;
    reportJsonRef.current = null;
    copyStateRef.current = 'idle';
    setSnapshot({
      summary: rec.summarize(),
      recording: true,
      elapsedMs: 0,
      gpuTimers: gpuStateRef.current,
      reportJson: null,
      copyState: 'idle',
    });
    void Promise.all([fetchLiveAgentCount(), readFrontendAppVersion()]).then(([agents, ver]) => {
      if (mountedRef.current) {
        agentCountRef.current = agents;
        appVersionRef.current = ver;
      }
    });
  }, []);

  const s = snapshot?.summary;
  const recording = snapshot?.recording ?? false;
  const elapsedSec = Math.min(60, Math.floor((snapshot?.elapsedMs ?? 0) / 1000));

  return (
    <div
      data-testid="perf-overlay"
      style={{
        position: 'absolute',
        top: 12,
        right: 12,
        zIndex: 110,
        width: 280,
        padding: 10,
        borderRadius: 10,
        background: 'rgba(2,6,23,0.82)',
        backdropFilter: 'blur(6px)',
        border: '1px solid #334155',
        color: '#e2e8f0',
        fontFamily: 'monospace',
        fontSize: 11,
        display: 'flex',
        flexDirection: 'column',
        gap: 6,
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <strong>Perf Telemetry (F3)</strong>
        <button type="button" style={PERF_BTN(false)} onClick={onClose}>
          Close
        </button>
      </div>
      <div>Median FPS: {s ? s.median_fps.toFixed(1) : '0.0'}</div>
      <div>p5 FPS: {s ? s.p5_fps.toFixed(1) : '0.0'}</div>
      <div>
        Draw calls: {s?.draw_calls ? `${s.draw_calls.current} cur / ${s.draw_calls.peak} peak` : 'unavailable'}
      </div>
      <div>GPU timers: {snapshot?.gpuTimers ?? 'unavailable'}</div>
      <button
        type="button"
        data-testid="perf-record-60s-btn"
        disabled={recording}
        style={PERF_BTN(recording)}
        onClick={handleRecord60s}
      >
        {recording ? `Recording ${elapsedSec}/60s…` : 'Record 60s'}
      </button>
      {snapshot?.reportJson ? (
        <>
          <div style={{ fontSize: 10, opacity: 0.8 }}>
            {snapshot.copyState === 'copied'
              ? 'Copied JSON to clipboard (read-only copy below):'
              : 'Clipboard unavailable — copy JSON below:'}
          </div>
          <textarea
            readOnly
            data-testid="perf-json-output"
            value={snapshot.reportJson}
            rows={7}
            style={{
              width: '100%',
              background: '#0f172a',
              color: '#cbd5e1',
              border: '1px solid #334155',
              borderRadius: 6,
              fontSize: 10,
              padding: 6,
              resize: 'vertical',
            }}
          />
        </>
      ) : null}
    </div>
  );
};

export default PerfOverlay;
