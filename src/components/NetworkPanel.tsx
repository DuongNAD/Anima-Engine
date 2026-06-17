import React from 'react';
import { MigrationPayload } from '../types';

interface NetworkPanelProps {
  running: boolean;
  targetPort: number;
  setTargetPort: (port: number) => void;
  migrationEvents: MigrationPayload[];
  handleTriggerMigration: () => void;
}

export const NetworkPanel: React.FC<NetworkPanelProps> = ({
  running,
  targetPort,
  setTargetPort,
  migrationEvents,
  handleTriggerMigration,
}) => {
  return (
    <div
      data-testid="migration-panel"
      className="hud-panel"
      style={{
        padding: '18px',
        borderRadius: '15px',
      }}
    >
      <h3
        className="hud-title-teal"
        style={{
          margin: '0 0 15px 0',
          fontSize: '18px',
          borderBottom: '1px solid rgba(255, 255, 255, 0.15)',
          paddingBottom: '8px',
        }}
      >
        Distributed Socket Migration
      </h3>
      <div style={{ marginBottom: '15px' }}>
        <div style={{ display: 'flex', gap: '8px', marginBottom: '12px', alignItems: 'center' }}>
          <label htmlFor="target-port-input" style={{ fontSize: '12px', fontWeight: 'bold', color: 'rgba(255, 255, 255, 0.7)' }}>
            Port:
          </label>
          <input
            id="target-port-input"
            type="number"
            value={targetPort}
            onChange={(e) => setTargetPort(parseInt(e.target.value) || 8081)}
            style={{
              width: '80px',
              padding: '6px',
              backgroundColor: 'rgba(255, 255, 255, 0.03)',
              border: '1px solid rgba(255, 255, 255, 0.1)',
              borderRadius: '6px',
              color: '#f4f4f5',
              fontFamily: "'Inter', sans-serif",
              outline: 'none',
            }}
          />
        </div>
        <button
          data-testid="migration-trigger-button"
          disabled={!running}
          className="hud-btn"
          style={{
            padding: '8px 16px',
            borderColor: running ? 'rgba(255, 255, 255, 0.4)' : 'rgba(255, 255, 255, 0.15)',
            color: running ? '#f4f4f5' : '#71717a',
            boxShadow: 'none',
            width: '100%',
            cursor: running ? 'pointer' : 'not-allowed',
            background: running ? 'rgba(255, 255, 255, 0.05)' : 'rgba(255, 255, 255, 0.01)',
          }}
          onClick={handleTriggerMigration}
        >
          Trigger Migration
        </button>
      </div>
      <div style={{ maxHeight: '150px', overflowY: 'auto', fontSize: '12px', fontFamily: "'Inter', sans-serif" }}>
        {migrationEvents.length === 0 ? (
          <p style={{ color: '#a1a1aa', fontStyle: 'italic' }}>No migration events</p>
        ) : (
          migrationEvents.map((mig, idx) => (
            <div
              key={idx}
              style={{
                padding: '6px 8px',
                borderBottom: '1px dashed rgba(255, 255, 255, 0.1)',
                color: '#cbd5e1',
              }}
            >
              Agent #{mig.agent_id} {mig.direction} ({mig.source_port} ➔ {mig.target_port}) - <span style={{ color: mig.status === 'Success' ? '#a1a1aa' : '#ffffff', fontWeight: 'bold' }}>{mig.status}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
};
export default NetworkPanel;
