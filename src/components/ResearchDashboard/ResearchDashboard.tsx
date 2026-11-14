import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { SimulationStatus } from '../../types/generated/SimulationStatus';
import type { EcosystemState } from '../../types/generated/EcosystemState';
import { TelemetryOverviewPanel } from './TelemetryOverviewPanel';
import { TrophicDynamicsPanel } from './TrophicDynamicsPanel';
import { EvolutionLineagePanel } from './EvolutionLineagePanel';
import { CausalInterventionPanel } from './CausalInterventionPanel';
import { CheckpointStudioPanel } from './CheckpointStudioPanel';
import { DataExportDialog } from './DataExportDialog';

export type DashboardTab = 'telemetry' | 'trophic' | 'evolution' | 'interventions' | 'checkpoints';

export interface ResearchDashboardProps {
  onClose?: () => void;
}

export const ResearchDashboard: React.FC<ResearchDashboardProps> = ({ onClose }) => {
  const [activeTab, setActiveTab] = useState<DashboardTab>('telemetry');
  const [status, setStatus] = useState<SimulationStatus>({
    running: false,
    tick_count: 0,
    avg_tick_time_ms: 0,
    fps: 0,
  });
  const [ecoState, setEcoState] = useState<EcosystemState | null>(null);
  const [showExportModal, setShowExportModal] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  // Fetch status and ecosystem telemetry with async polling
  useEffect(() => {
    let isCancelled = false;

    const pollStatus = async () => {
      try {
        const s = await invoke<SimulationStatus>('get_simulation_status');
        if (!isCancelled) {
          setStatus(s);
        }
      } catch {
        // Fallback
      }

      try {
        const eco = await invoke<EcosystemState>('get_ecosystem_state');
        if (!isCancelled && eco) {
          setEcoState(eco);
        }
      } catch {
        // Fallback
      }
    };

    void pollStatus();
    const interval = setInterval(() => {
      void pollStatus();
    }, 1000);

    return () => {
      isCancelled = true;
      clearInterval(interval);
    };
  }, []);

  const handleToggleSimulation = async () => {
    try {
      const running = await invoke<boolean>('toggle_simulation');
      setStatus((prev) => ({ ...prev, running }));
    } catch (e) {
      setError(String(e));
    }
  };

  // Keyboard shortcut listener: 1-5 for tabs, Space for toggle, Ctrl+E for export
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Don't intercept when focused on an input or textarea
      if (
        document.activeElement?.tagName === 'INPUT' ||
        document.activeElement?.tagName === 'TEXTAREA' ||
        document.activeElement?.tagName === 'SELECT'
      ) {
        return;
      }

      if (e.key === '1') setActiveTab('telemetry');
      else if (e.key === '2') setActiveTab('trophic');
      else if (e.key === '3') setActiveTab('evolution');
      else if (e.key === '4') setActiveTab('interventions');
      else if (e.key === '5') setActiveTab('checkpoints');
      else if (e.key === ' ' || e.code === 'Space') {
        e.preventDefault();
        handleToggleSimulation();
      } else if ((e.ctrlKey || e.metaKey) && (e.key === 'e' || e.key === 'E')) {
        e.preventDefault();
        setShowExportModal(true);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  const totalEnergy = ecoState?.total ?? 900;
  const baselineEnergy = 900;
  const energyResidual = Math.abs(totalEnergy - baselineEnergy);
  const driftPercent = (energyResidual / baselineEnergy) * 100;
  const isConserved = energyResidual <= 0.001 && driftPercent < 0.01;

  const tabs: Array<{ id: DashboardTab; label: string; icon: string }> = [
    { id: 'telemetry', label: 'Telemetry & Phase Space', icon: '📊' },
    { id: 'trophic', label: 'Trophic Dynamics & Flux', icon: '🌿' },
    { id: 'evolution', label: 'Lineage & MAP-Elites', icon: '🧬' },
    { id: 'interventions', label: 'Causal Interventions', icon: '⚡' },
    { id: 'checkpoints', label: 'Checkpoint Studio', icon: '⏱️' },
  ];

  return (
    <div
      data-testid="research-dashboard-container"
      className="hud-panel"
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: '14px',
        padding: '16px 20px',
        backgroundColor: 'rgba(9, 9, 11, 0.95)',
        minHeight: '85vh',
        color: '#e2e8f0',
      }}
    >
      {/* Top Header Bar: Title, Conservation Gate Pill, Clock/FPS, and Actions */}
      <header
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          flexWrap: 'wrap',
          gap: '12px',
          borderBottom: '1px solid rgba(255, 255, 255, 0.08)',
          paddingBottom: '12px',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <div>
            <h1
              data-testid="dashboard-title"
              style={{
                margin: 0,
                fontSize: '18px',
                color: '#f8fafc',
                display: 'flex',
                alignItems: 'center',
                gap: '8px',
              }}
            >
              🔬 Anima Engine Research &amp; Telemetry Suite
            </h1>
            <div style={{ fontSize: '11px', color: '#94a3b8', marginTop: '2px' }}>
              Deterministic Artificial Life &amp; Scientific Experimentation Platform
            </div>
          </div>

          {/* Conservation Status Badge Pill */}
          <div
            data-testid="conservation-status-badge"
            style={{
              padding: '4px 10px',
              borderRadius: '9999px',
              fontSize: '11px',
              fontWeight: 600,
              display: 'flex',
              alignItems: 'center',
              gap: '6px',
              backgroundColor: isConserved ? 'rgba(6, 78, 59, 0.5)' : 'rgba(127, 29, 29, 0.5)',
              border: `1px solid ${isConserved ? '#10b981' : '#ef4444'}`,
              color: isConserved ? '#a7f3d0' : '#fecaca',
            }}
          >
            <span
              style={{
                width: '6px',
                height: '6px',
                borderRadius: '50%',
                backgroundColor: isConserved ? '#10b981' : '#ef4444',
              }}
            />
            {isConserved ? 'CONSERVED (ΔE ≤ 10⁻³ EU)' : 'DRIFT WARNING (ΔE > 10⁻³ EU)'}
          </div>
        </div>

        {/* Live Simulation Clock & Provenance Ribbon */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px', flexWrap: 'wrap' }}>
          <div
            data-testid="clock-fps-badge"
            style={{
              fontSize: '12px',
              backgroundColor: 'rgba(255, 255, 255, 0.05)',
              padding: '4px 10px',
              borderRadius: '6px',
              border: '1px solid rgba(255, 255, 255, 0.08)',
              color: '#cbd5e1',
            }}
          >
            Tick: <strong style={{ color: '#f8fafc' }}>{status.tick_count}</strong> | 
            FPS: <strong style={{ color: '#38bdf8' }}>{status.fps.toFixed(1)}</strong> | 
            Lat: <strong style={{ color: '#f59e0b' }}>{status.avg_tick_time_ms.toFixed(1)}ms</strong>
          </div>

          <div
            data-testid="run-provenance-ribbon"
            style={{
              fontSize: '11px',
              color: '#94a3b8',
              backgroundColor: 'rgba(255, 255, 255, 0.03)',
              padding: '4px 8px',
              borderRadius: '4px',
              border: '1px solid rgba(255, 255, 255, 0.06)',
            }}
          >
            commit: <code style={{ color: '#cbd5e1' }}>c320fc0</code> | seed: <code>42</code>
          </div>

          {/* Global Simulation Controls & Export Trigger */}
          <div style={{ display: 'flex', gap: '8px' }}>
            <button
              data-testid="dashboard-toggle-sim-btn"
              onClick={handleToggleSimulation}
              className="hud-btn"
              style={{
                padding: '6px 14px',
                backgroundColor: status.running ? '#dc2626' : '#16a34a',
                color: '#ffffff',
                border: 'none',
                borderRadius: '6px',
                fontWeight: 600,
                fontSize: '12px',
                cursor: 'pointer',
              }}
            >
              {status.running ? '⏸️ Pause Sim' : '▶️ Resume Sim'}
            </button>

            <button
              data-testid="open-export-dialog-btn"
              onClick={() => setShowExportModal(true)}
              className="hud-btn"
              style={{
                padding: '6px 14px',
                backgroundColor: 'rgba(2, 132, 199, 0.8)',
                color: '#ffffff',
                border: 'none',
                borderRadius: '6px',
                fontWeight: 600,
                fontSize: '12px',
                cursor: 'pointer',
              }}
            >
              📥 Export Dataset
            </button>

            {onClose && (
              <button
                data-testid="close-dashboard-btn"
                onClick={onClose}
                style={{
                  padding: '6px 10px',
                  backgroundColor: 'rgba(255, 255, 255, 0.05)',
                  border: '1px solid rgba(255, 255, 255, 0.1)',
                  color: '#cbd5e1',
                  borderRadius: '6px',
                  cursor: 'pointer',
                  fontSize: '12px',
                }}
              >
                ⬅️ Exit
              </button>
            )}
          </div>
        </div>
      </header>

      {error && (
        <div
          data-testid="dashboard-error-banner"
          style={{
            backgroundColor: '#7f1d1d',
            color: '#fecaca',
            padding: '8px 12px',
            borderRadius: '6px',
            fontSize: '12px',
          }}
        >
          Error: {error}
        </div>
      )}

      {/* Tab Navigation Ribbon */}
      <nav
        data-testid="dashboard-tab-navigation"
        style={{
          display: 'flex',
          gap: '6px',
          borderBottom: '1px solid rgba(255, 255, 255, 0.08)',
          paddingBottom: '8px',
          overflowX: 'auto',
        }}
      >
        {tabs.map((t, idx) => {
          const isActive = activeTab === t.id;
          return (
            <button
              key={t.id}
              data-testid={`dashboard-tab-${t.id}`}
              onClick={() => setActiveTab(t.id)}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '6px',
                padding: '8px 14px',
                borderRadius: '6px',
                border: `1px solid ${isActive ? '#0284c7' : 'rgba(255, 255, 255, 0.08)'}`,
                backgroundColor: isActive ? 'rgba(2, 132, 199, 0.25)' : 'rgba(255, 255, 255, 0.03)',
                color: isActive ? '#f8fafc' : '#94a3b8',
                fontWeight: isActive ? 600 : 500,
                fontSize: '12px',
                cursor: 'pointer',
                transition: 'all 0.15s ease',
              }}
            >
              <span>{t.icon}</span>
              <span>{t.label}</span>
              <span style={{ fontSize: '10px', color: '#64748b' }}>[{idx + 1}]</span>
            </button>
          );
        })}
      </nav>

      {/* Tab Content Panel Body */}
      <main data-testid="dashboard-tab-body" style={{ flex: 1 }}>
        {activeTab === 'telemetry' && (
          <TelemetryOverviewPanel isRunning={status.running} />
        )}
        {activeTab === 'trophic' && (
          <TrophicDynamicsPanel isRunning={status.running} />
        )}
        {activeTab === 'evolution' && (
          <EvolutionLineagePanel isRunning={status.running} />
        )}
        {activeTab === 'interventions' && (
          <CausalInterventionPanel currentTick={status.tick_count} isRunning={status.running} />
        )}
        {activeTab === 'checkpoints' && (
          <CheckpointStudioPanel currentTick={status.tick_count} />
        )}
      </main>

      {/* Scientific Dataset Export Modal */}
      {showExportModal && (
        <DataExportDialog onClose={() => setShowExportModal(false)} />
      )}
    </div>
  );
};
