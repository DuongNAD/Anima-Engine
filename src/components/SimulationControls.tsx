import React from 'react';

interface SimulationControlsProps {
  running: boolean;
  projection: 'xy' | 'xz';
  handleToggle: () => void;
  setProjection: (proj: 'xy' | 'xz') => void;
}

export const SimulationControls: React.FC<SimulationControlsProps> = ({
  running,
  projection,
  handleToggle,
  setProjection,
}) => {
  return (
    <div style={{ display: 'flex', gap: '15px', marginBottom: '20px', fontFamily: "'Inter', -apple-system, sans-serif" }}>
      <button
        onClick={handleToggle}
        className={running ? 'hud-btn' : 'hud-btn'}
        style={{
          padding: '10px 20px',
          fontSize: '14px',
          backgroundColor: running ? 'rgba(239, 68, 68, 0.15)' : 'rgba(255, 255, 255, 0.05)',
          borderColor: running ? 'rgba(239, 68, 68, 0.4)' : 'rgba(255, 255, 255, 0.1)',
          color: running ? '#fca5a5' : '#f1f5f9',
          boxShadow: 'none',
        }}
      >
        {running ? 'Dừng mô phỏng' : 'Bắt đầu mô phỏng'}
      </button>

      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '8px',
          border: '1px solid rgba(255, 255, 255, 0.08)',
          borderRadius: '12px',
          padding: '4px 12px',
          backgroundColor: 'rgba(15, 23, 42, 0.45)',
          backdropFilter: 'blur(16px)',
          WebkitBackdropFilter: 'blur(16px)',
          boxShadow: '0 4px 16px rgba(0, 0, 0, 0.15)',
        }}
      >
        <span style={{ fontSize: '13px', fontWeight: 'bold', color: '#cbd5e1' }}>Mặt phẳng chiếu:</span>
        <button
          onClick={() => setProjection('xy')}
          className="hud-btn"
          style={{
            padding: '6px 12px',
            backgroundColor: projection === 'xy' ? 'rgba(255, 255, 255, 0.12)' : 'transparent',
            color: '#f1f5f9',
            borderColor: projection === 'xy' ? 'rgba(255, 255, 255, 0.2)' : 'transparent',
            boxShadow: 'none',
            fontSize: '12px',
          }}
        >
          X-Y (Mặt trước/bên)
        </button>
        <button
          onClick={() => setProjection('xz')}
          className="hud-btn"
          style={{
            padding: '6px 12px',
            backgroundColor: projection === 'xz' ? 'rgba(255, 255, 255, 0.12)' : 'transparent',
            color: '#f1f5f9',
            borderColor: projection === 'xz' ? 'rgba(255, 255, 255, 0.2)' : 'transparent',
            boxShadow: 'none',
            fontSize: '12px',
          }}
        >
          X-Z (Mặt trên)
        </button>
      </div>
    </div>
  );
};
