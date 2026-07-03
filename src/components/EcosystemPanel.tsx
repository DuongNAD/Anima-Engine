import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

/** Live snapshot of the closed ecosystem (mirrors the backend `EcosystemState` DTO). */
export interface EcosystemState {
  detritus: number;
  plants: number;
  animals: number;
  total: number;
  prey_count: number;
  predator_count: number;
  shannon: number;
  simpson: number;
}

const COMPARTMENTS: Array<{ key: keyof EcosystemState; label: string; color: string }> = [
  { key: 'plants', label: 'Thực vật', color: '#38a169' },
  { key: 'animals', label: 'Động vật', color: '#dd6b20' },
  { key: 'detritus', label: 'Mùn hữu cơ', color: '#805ad5' },
];

// Population series colors (validated for CVD + contrast on the light card surface).
const PREY_COLOR = '#3182ce';
const PREDATOR_COLOR = '#e53e3e';

const HISTORY = 60; // rolling window of samples (~1 minute at 1 Hz)

// Text ink tokens — labels never wear the series colour (a colored dot beside them carries it).
const INK = '#4a5568';
const INK_MUTED = '#718096';

interface Series {
  label: string;
  color: string;
  values: number[];
}

/**
 * A compact multi-line sparkline over one shared Y axis (never dual-axis): every series is
 * normalized against the combined min/max of the chart, so magnitudes stay comparable. 2px
 * lines, a recessive baseline, and a single end-dot marking the latest value.
 */
const Spark: React.FC<{ series: Series[]; testid: string; height?: number }> = ({
  series,
  testid,
  height = 34,
}) => {
  const W = 240;
  const H = height;
  const pad = 3;
  const all = series.flatMap((s) => s.values);
  const lo = Math.min(...all, 0);
  const hi = Math.max(...all, 1);
  const span = hi - lo || 1;
  const n = Math.max(...series.map((s) => s.values.length), 1);
  const x = (i: number) => (n <= 1 ? 0 : (i / (n - 1)) * (W - pad * 2) + pad);
  const y = (v: number) => H - pad - ((v - lo) / span) * (H - pad * 2);

  return (
    <svg
      data-testid={testid}
      viewBox={`0 0 ${W} ${H}`}
      preserveAspectRatio="none"
      style={{ width: '100%', height, display: 'block' }}
      role="img"
    >
      {/* Recessive baseline. */}
      <line x1={0} y1={H - pad} x2={W} y2={H - pad} stroke="#edf2f7" strokeWidth={1} />
      {series.map((s) => {
        if (s.values.length < 2) return null;
        const pts = s.values.map((v, i) => `${x(i).toFixed(1)},${y(v).toFixed(1)}`).join(' ');
        const lastX = x(s.values.length - 1);
        const lastY = y(s.values[s.values.length - 1]);
        return (
          <g key={s.label}>
            <title>{s.label}</title>
            <polyline points={pts} fill="none" stroke={s.color} strokeWidth={2} strokeLinejoin="round" />
            <circle cx={lastX} cy={lastY} r={2.5} fill={s.color} />
          </g>
        );
      })}
    </svg>
  );
};

/** A legend row: colored dots + labels (identity is never color-alone). */
const Legend: React.FC<{ items: Array<{ label: string; color: string }> }> = ({ items }) => (
  <div style={{ display: 'flex', gap: '12px', flexWrap: 'wrap', fontSize: '12px', color: INK_MUTED, marginTop: '2px' }}>
    {items.map((it) => (
      <span key={it.label} style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
        <span style={{ width: '9px', height: '9px', borderRadius: '2px', backgroundColor: it.color }} />
        {it.label}
      </span>
    ))}
  </div>
);

/**
 * Live dashboard for the closed-energy ecosystem: the three compartments of the conserved
 * biomass ledger (their sum should stay ~constant → the anti-collapse conservation the ecology
 * model guarantees), the predator/prey split, the biodiversity indices, and — over a rolling
 * one-minute window — the population cycles and energy-flow trends. Polls the backend
 * `get_ecosystem_state` command once a second; shows a hint until the simulation is running.
 */
export const EcosystemPanel: React.FC<{ pollMs?: number }> = ({ pollMs = 1000 }) => {
  const [state, setState] = useState<EcosystemState | null>(null);
  const [history, setHistory] = useState<EcosystemState[]>([]);
  const lastTotal = useRef<number | null>(null);

  useEffect(() => {
    let alive = true;
    const poll = async () => {
      try {
        const s = await invoke<EcosystemState>('get_ecosystem_state');
        if (!alive || !s || typeof s.total !== 'number') return;
        setState(s);
        // Only accumulate history while the sim is actually producing new frames.
        if (s.total !== lastTotal.current || s.prey_count > 0 || s.predator_count > 0) {
          lastTotal.current = s.total;
          setHistory((h) => {
            const next = [...h, s];
            return next.length > HISTORY ? next.slice(next.length - HISTORY) : next;
          });
        }
      } catch {
        /* simulation not running yet — keep the placeholder */
      }
    };
    poll();
    const id = setInterval(poll, pollMs);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, [pollMs]);

  const total = state?.total ?? 0;
  const pct = (v: number) => (total > 0 ? (v / total) * 100 : 0);

  const popSeries: Series[] = [
    { label: 'Con mồi', color: PREY_COLOR, values: history.map((h) => h.prey_count) },
    { label: 'Săn mồi', color: PREDATOR_COLOR, values: history.map((h) => h.predator_count) },
  ];
  const bioSeries: Series[] = COMPARTMENTS.map((c) => ({
    label: c.label,
    color: c.color,
    values: history.map((h) => h[c.key] as number),
  }));
  const showTrends = history.length >= 2;

  return (
    <div
      data-testid="ecosystem-panel"
      style={{
        border: '1px solid #e2e8f0',
        padding: '15px',
        borderRadius: '6px',
        backgroundColor: 'white',
        boxShadow: '0 1px 3px rgba(0,0,0,0.1)',
      }}
    >
      <h2
        style={{
          margin: '0 0 10px 0',
          fontSize: '18px',
          borderBottom: '2px solid #edf2f7',
          paddingBottom: '5px',
        }}
      >
        🌍 Hệ Sinh Thái (Năng lượng khép kín)
      </h2>

      {!state ? (
        <p style={{ color: INK_MUTED, margin: 0 }} data-testid="ecosystem-empty">
          Chưa có dữ liệu — khởi động mô phỏng để xem dòng năng lượng.
        </p>
      ) : (
        <>
          {/* Stacked biomass bar: plants | animals | detritus (widths = share of total). A 2px
              surface gap separates the fills so adjacent compartments stay distinct. */}
          <div
            style={{
              display: 'flex',
              gap: '2px',
              height: '22px',
              borderRadius: '4px',
              overflow: 'hidden',
              marginBottom: '10px',
            }}
          >
            {COMPARTMENTS.map((c) => (
              <div
                key={c.key}
                title={`${c.label}: ${(state[c.key] as number).toFixed(1)}`}
                style={{ width: `${pct(state[c.key] as number)}%`, backgroundColor: c.color }}
              />
            ))}
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: '5px', fontSize: '14px', color: INK }}>
            {COMPARTMENTS.map((c) => (
              <p key={c.key} style={{ margin: 0, display: 'flex', justifyContent: 'space-between' }}>
                <span>
                  <span
                    style={{
                      display: 'inline-block',
                      width: '10px',
                      height: '10px',
                      borderRadius: '2px',
                      backgroundColor: c.color,
                      marginRight: '6px',
                    }}
                  />
                  {c.label}
                </span>
                <strong data-testid={`ecosystem-${c.key}`}>{(state[c.key] as number).toFixed(1)}</strong>
              </p>
            ))}
            <p style={{ margin: '4px 0 0 0', display: 'flex', justifyContent: 'space-between', borderTop: '1px dashed #e2e8f0', paddingTop: '5px' }}>
              <span>Con mồi / Săn mồi</span>
              <strong data-testid="ecosystem-populations">
                🐇 {state.prey_count} / 🐺 {state.predator_count}
              </strong>
            </p>
            <p style={{ margin: 0, display: 'flex', justifyContent: 'space-between' }}>
              <span>Đa dạng (Shannon / Simpson)</span>
              <strong data-testid="ecosystem-diversity">
                {state.shannon.toFixed(2)} / {state.simpson.toFixed(2)}
              </strong>
            </p>
          </div>

          {/* Time-series: population cycles and energy-flow trends over a rolling window. */}
          {showTrends && (
            <div style={{ marginTop: '12px', display: 'flex', flexDirection: 'column', gap: '10px' }}>
              <div>
                <div style={{ fontSize: '12px', color: INK_MUTED, marginBottom: '2px' }}>
                  Chu kỳ quần thể (theo thời gian)
                </div>
                <Spark series={popSeries} testid="ecosystem-spark-population" />
                <Legend items={popSeries.map((s) => ({ label: s.label, color: s.color }))} />
              </div>
              <div>
                <div style={{ fontSize: '12px', color: INK_MUTED, marginBottom: '2px' }}>
                  Dòng năng lượng (theo thời gian)
                </div>
                <Spark series={bioSeries} testid="ecosystem-spark-biomass" />
                <Legend items={bioSeries.map((s) => ({ label: s.label, color: s.color }))} />
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
};

export default EcosystemPanel;
