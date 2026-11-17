import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { EnergyFluxReport } from '../../types/generated/EnergyFluxReport';
import type { EcosystemState } from '../../types/generated/EcosystemState';
import { TrophicPyramid } from './common/TrophicPyramid';
import { SankeyDiagram } from './common/SankeyDiagram';

export interface TrophicDynamicsPanelProps {
  initialFluxReport?: EnergyFluxReport | null;
  isRunning?: boolean;
}

export const TrophicDynamicsPanel: React.FC<TrophicDynamicsPanelProps> = ({
  initialFluxReport,
  isRunning = true,
}) => {
  const [fluxReport, setFluxReport] = useState<EnergyFluxReport | null>(initialFluxReport || null);
  const [ecoState, setEcoState] = useState<EcosystemState | null>(null);

  useEffect(() => {
    let alive = true;

    const fetchFlux = async () => {
      try {
        const report = await invoke<EnergyFluxReport>('get_energy_flux_matrix');
        if (alive && report) {
          setFluxReport(report);
        }
      } catch {
        // Ignore fallback
      }

      try {
        const eco = await invoke<EcosystemState>('get_ecosystem_state');
        if (alive && eco) {
          setEcoState(eco);
        }
      } catch {
        // Ignore fallback
      }
    };

    fetchFlux();
    const interval = setInterval(() => {
      if (isRunning) fetchFlux();
    }, 1000);

    return () => {
      alive = false;
      clearInterval(interval);
    };
  }, [isRunning]);

  // Only values the backend reported. Before telemetry arrives every figure is "—"; this panel used
  // to fill the gap with invented numbers (detritus 100, plants 500, animals 300, respiration 125,
  // drift 0.002 %, body masses 6 and 9) and to split animals 75/25 into herbivores and carnivores.
  const detritus = ecoState?.detritus ?? null;
  const plants = ecoState?.plants ?? null;
  const animals = ecoState?.animals ?? null;

  const respirationLoss = fluxReport?.respirationLoss ?? null;
  const totalEnergy = fluxReport?.totalSystemEnergy ?? ecoState?.total ?? null;
  const driftPct = fluxReport?.energyDriftPercent ?? null;

  // Kleiber scaling (M^(3/4)) of the guild mean body masses the backend reports.
  const avgPreyMass = ecoState?.prey_mass ?? null;
  const avgPredMass = ecoState?.predator_mass ?? null;
  const preyKleiberScaling = avgPreyMass === null ? null : Math.pow(avgPreyMass, 0.75);
  const predKleiberScaling = avgPredMass === null ? null : Math.pow(avgPredMass, 0.75);

  const fmt = (v: number | null, digits: number): string =>
    v === null || !Number.isFinite(v) ? '—' : v.toFixed(digits);

  return (
    <div
      data-testid="trophic-dynamics-panel"
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: '16px',
        color: '#e2e8f0',
      }}
    >
      {/* Top Overview Cards */}
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
          gap: '12px',
        }}
      >
        <div
          style={{
            backgroundColor: 'rgba(15, 23, 42, 0.55)',
            border: '1px solid rgba(255, 255, 255, 0.08)',
            borderRadius: '8px',
            padding: '12px',
          }}
        >
          <div style={{ fontSize: '11px', color: '#94a3b8' }}>Total System Energy & Drift</div>
          <div style={{ fontSize: '16px', fontWeight: 700, color: '#f8fafc', margin: '4px 0' }}>
            {fmt(totalEnergy, 1)} EU
          </div>
          <div
            style={{
              fontSize: '11px',
              color: driftPct === null ? '#94a3b8' : Math.abs(driftPct) < 0.01 ? '#10b981' : '#ef4444',
            }}
          >
            Conservation Drift: {fmt(driftPct, 4)}% (&lt; 0.01% law)
          </div>
        </div>

        <div
          style={{
            backgroundColor: 'rgba(15, 23, 42, 0.55)',
            border: '1px solid rgba(255, 255, 255, 0.08)',
            borderRadius: '8px',
            padding: '12px',
          }}
        >
          <div style={{ fontSize: '11px', color: '#94a3b8' }}>Metabolic Respiration Sink</div>
          <div style={{ fontSize: '16px', fontWeight: 700, color: '#f59e0b', margin: '4px 0' }}>
            {fmt(respirationLoss, 1)} EU/tick
          </div>
          <div style={{ fontSize: '11px', color: '#cbd5e1' }}>
            MTE Kleiber Scaling: M^(3/4) allometry
          </div>
        </div>

        <div
          style={{
            backgroundColor: 'rgba(15, 23, 42, 0.55)',
            border: '1px solid rgba(255, 255, 255, 0.08)',
            borderRadius: '8px',
            padding: '12px',
          }}
        >
          <div style={{ fontSize: '11px', color: '#94a3b8' }}>Guild Body Masses</div>
          <div style={{ fontSize: '14px', fontWeight: 600, color: '#f8fafc', margin: '4px 0' }}>
            Prey: {fmt(avgPreyMass, 1)} MU (B ∝ {fmt(preyKleiberScaling, 2)})
          </div>
          <div style={{ fontSize: '14px', fontWeight: 600, color: '#f8fafc' }}>
            Predator: {fmt(avgPredMass, 1)} MU (B ∝ {fmt(predKleiberScaling, 2)})
          </div>
        </div>
      </div>

      {/* Main Visualizations: Biomass Pyramid + Energy Flux Sankey */}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1.6fr', gap: '16px', alignItems: 'start' }}>
        {/* Tiered Biomass Pyramid */}
        <TrophicPyramid
          data={{
            detritus,
            plants,
            herbivores: null,
            carnivores: null,
            consumers: animals,
          }}
          width={340}
          height={280}
          testid="trophic-pyramid-container"
        />

        {/* Interactive Energy Flux Sankey */}
        <SankeyDiagram
          report={fluxReport}
          width={580}
          height={300}
          testid="energy-flux-sankey"
        />
      </div>

      {/* MTE Kleiber & Arrhenius Scaling Explainer Bar */}
      <div
        style={{
          backgroundColor: 'rgba(15, 23, 42, 0.45)',
          border: '1px solid rgba(255, 255, 255, 0.06)',
          borderRadius: '8px',
          padding: '12px',
          fontSize: '12px',
          color: '#cbd5e1',
        }}
      >
        <div style={{ fontWeight: 600, color: '#38bdf8', marginBottom: '4px' }}>
          📐 Metabolic Theory of Ecology (MTE) Physical Invariants
        </div>
        <div>
          Basal metabolic burn adheres to Kleiber&apos;s allometric law <code>B(M) = B₀ · M^(3/4)</code> and Arrhenius thermal scaling <code>f(T) = exp(-E / kT)</code>. Herbivores and predators consume biomass via Holling Type II &amp; III functional responses, dissipating metabolic heat into the global entropy sink.
        </div>
      </div>
    </div>
  );
};
