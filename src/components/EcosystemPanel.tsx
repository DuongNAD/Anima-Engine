import { useEffect, useState } from 'react';
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

/**
 * Live dashboard for the closed-energy ecosystem: the three compartments of the conserved
 * biomass ledger (their sum should stay ~constant → the anti-collapse conservation the
 * ecology model guarantees), the predator/prey split, and the biodiversity indices. Polls the
 * backend `get_ecosystem_state` command once a second; renders nothing but a hint until the
 * simulation is running.
 */
export const EcosystemPanel: React.FC<{ pollMs?: number }> = ({ pollMs = 1000 }) => {
  const [state, setState] = useState<EcosystemState | null>(null);

  useEffect(() => {
    let alive = true;
    const poll = async () => {
      try {
        const s = await invoke<EcosystemState>('get_ecosystem_state');
        if (alive && s && typeof s.total === 'number') setState(s);
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
        <p style={{ color: '#718096', margin: 0 }} data-testid="ecosystem-empty">
          Chưa có dữ liệu — khởi động mô phỏng để xem dòng năng lượng.
        </p>
      ) : (
        <>
          {/* Stacked biomass bar: plants | animals | detritus (widths = share of total). */}
          <div
            style={{
              display: 'flex',
              height: '22px',
              borderRadius: '4px',
              overflow: 'hidden',
              marginBottom: '10px',
              border: '1px solid #edf2f7',
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

          <div style={{ display: 'flex', flexDirection: 'column', gap: '5px', fontSize: '14px' }}>
            {COMPARTMENTS.map((c) => (
              <p
                key={c.key}
                style={{ margin: 0, display: 'flex', justifyContent: 'space-between' }}
              >
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
        </>
      )}
    </div>
  );
};

export default EcosystemPanel;
