import React from 'react';
import { ChronicleEvent } from '../types';

interface ChroniclePanelProps {
  chronicleHistory: ChronicleEvent[];
}

export const ChroniclePanel: React.FC<ChroniclePanelProps> = ({ chronicleHistory }) => {
  const history = Array.isArray(chronicleHistory) ? chronicleHistory : [];

  return (
    <div
      data-testid="chronicle-timeline-panel"
      className="hud-panel"
      style={{
        padding: '18px',
        borderRadius: '15px',
      }}
    >
      <h2
        className="hud-title-teal"
        style={{
          margin: '0 0 15px 0',
          fontSize: '18px',
          borderBottom: '1px solid rgba(255, 255, 255, 0.15)',
          paddingBottom: '8px',
        }}
      >
        Mother Nature Chronicle
      </h2>
      <div style={{ maxHeight: '200px', overflowY: 'auto' }}>
        {history.length === 0 ? (
          <p style={{ color: '#a1a1aa', fontStyle: 'italic', fontFamily: "'Inter', sans-serif" }}>No chronicle events recorded</p>
        ) : (
          history.map((evt, idx) => {
            const isAlert = ['Drought', 'TemperatureSpike', 'PredatorWave'].includes(evt.event_type);
            return (
              <div
                key={evt.id || idx}
                style={{
                  padding: '10px',
                  marginBottom: '8px',
                  backgroundColor: 'rgba(255, 255, 255, 0.02)',
                  border: '1px solid rgba(255, 255, 255, 0.08)',
                  borderLeft: '3px solid ' + (isAlert ? '#ffffff' : 'rgba(255, 255, 255, 0.3)'),
                  borderRadius: '8px',
                  fontFamily: "'Inter', sans-serif",
                }}
              >
                <div
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    fontSize: '10px',
                    color: '#94a3b8',
                    marginBottom: '2px',
                  }}
                >
                  <span style={{ fontWeight: 'bold', color: isAlert ? '#ffffff' : '#a1a1aa' }}>{evt.event_type}</span>
                  <span>{new Date(evt.timestamp).toLocaleTimeString()}</span>
                </div>
                <strong style={{ color: '#f8fafc' }}>{evt.title}</strong>
                <p style={{ margin: '4px 0 0 0', fontSize: '12px', color: '#cbd5e1' }}>{evt.description}</p>
                {evt.parameter_delta && Object.keys(evt.parameter_delta).length > 0 && (
                  <div
                    style={{ marginTop: '6px', fontSize: '11px', color: '#cbd5e1', fontWeight: 'bold' }}
                    data-testid="parameter-delta-warning"
                  >
                    ⚠️ Parameter Deltas:{' '}
                    {Object.entries(evt.parameter_delta)
                      .map(([k, v]) => `${k}: ${v >= 0 ? '+' : ''}${v}`)
                      .join(', ')}
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>
    </div>
  );
};
export default ChroniclePanel;
