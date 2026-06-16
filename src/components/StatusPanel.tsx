import React from 'react';
import { SimulationStatus } from '../types';

interface StatusPanelProps {
  status: SimulationStatus;
}

export const StatusPanel: React.FC<StatusPanelProps> = ({ status }) => {
  const fpsColor = status.fps >= 55 ? '#f8fafc' : status.fps >= 30 ? '#cbd5e1' : '#64748b';
  const latencyColor = status.avg_tick_time_ms < 2.0 ? '#f8fafc' : status.avg_tick_time_ms < 5.0 ? '#cbd5e1' : '#64748b';

  return (
    <div
      style={{
        border: '1px solid rgba(255, 255, 255, 0.08)',
        padding: '18px',
        borderRadius: '12px',
        backgroundColor: 'rgba(15, 23, 42, 0.45)',
        backdropFilter: 'blur(16px)',
        WebkitBackdropFilter: 'blur(16px)',
        color: '#cbd5e1',
        boxShadow: '0 8px 32px 0 rgba(0, 0, 0, 0.25)',
        fontFamily: "'Inter', -apple-system, sans-serif",
      }}
    >
      <h2
        className="hud-title-teal"
        style={{
          margin: '0 0 15px 0',
          fontSize: '16px',
          borderBottom: '1px solid rgba(255, 255, 255, 0.08)',
          paddingBottom: '8px',
        }}
      >
        🛰️ ENGINE_STATUS_TELEMETRY (Trạng thái Mô phỏng)
      </h2>

      {/* Retro performance status bar */}
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          backgroundColor: 'rgba(255, 255, 255, 0.02)',
          border: '1px solid rgba(255, 255, 255, 0.06)',
          borderRadius: '8px',
          padding: '12px',
          marginBottom: '15px',
        }}
      >
        <div>
          <span style={{ color: '#94a3b8', fontSize: '12px' }}>TICK_LATENCY: </span>
          <span style={{ color: latencyColor, fontWeight: 'bold' }}>
            {status.avg_tick_time_ms.toFixed(2)} ms
          </span>
        </div>
        <div>
          <span style={{ color: '#94a3b8', fontSize: '12px' }}>BACKEND_FPS: </span>
          <span style={{ color: fpsColor, fontWeight: 'bold' }}>
            {status.fps.toFixed(1)}
          </span>
        </div>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', fontSize: '14px' }}>
        <p style={{ margin: 0, display: 'flex', justifyContent: 'space-between', borderBottom: '1px dashed rgba(255, 255, 255, 0.08)', paddingBottom: '4px' }}>
          <strong>Đang chạy:</strong> 
          <span style={{ color: status.running ? '#f8fafc' : '#64748b', fontWeight: 'bold' }}>
            {status.running ? 'Có [ONLINE]' : 'Không [OFFLINE]'}
          </span>
        </p>
        <p style={{ margin: 0, display: 'flex', justifyContent: 'space-between', borderBottom: '1px dashed rgba(255, 255, 255, 0.08)', paddingBottom: '4px' }}>
          <strong>Số Ticks: </strong> 
          <span className="neon-teal" style={{ fontWeight: 'bold' }}>{status.tick_count}</span>
        </p>
        <p style={{ margin: 0, display: 'flex', justifyContent: 'space-between', borderBottom: '1px dashed rgba(255, 255, 255, 0.08)', paddingBottom: '4px' }}>
          <strong>Độ trễ TB của Tick: </strong> 
          <span style={{ color: latencyColor, fontWeight: 'bold' }}>{status.avg_tick_time_ms.toFixed(2)} ms</span>
        </p>
        <p style={{ margin: 0, display: 'flex', justifyContent: 'space-between', paddingBottom: '2px' }}>
          <strong>Backend FPS: </strong> 
          <span style={{ color: fpsColor, fontWeight: 'bold' }}>{status.fps.toFixed(1)}</span>
        </p>
      </div>
    </div>
  );
};
