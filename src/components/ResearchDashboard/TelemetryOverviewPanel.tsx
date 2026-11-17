import React, { useEffect, useState, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { EcosystemState } from '../../types/generated/EcosystemState';
import type { DemographicCensusState } from '../../types/generated/DemographicCensusState';
import { TimeSeriesChart } from './common/TimeSeriesChart';
import { PhaseSpaceCanvas } from './common/PhaseSpaceCanvas';

export interface TelemetryOverviewPanelProps {
  initialEcosystemState?: EcosystemState | null;
  initialDemographics?: DemographicCensusState | null;
  isRunning?: boolean;
}

export const TelemetryOverviewPanel: React.FC<TelemetryOverviewPanelProps> = ({
  initialEcosystemState,
  initialDemographics,
  isRunning = true,
}) => {
  const [ecoState, setEcoState] = useState<EcosystemState | null>(initialEcosystemState || null);
  const [demographics, setDemographics] = useState<DemographicCensusState | null>(initialDemographics || null);
  const [timeWindow, setTimeWindow] = useState<number>(500);

  // Rolling time-series buffers
  const [plantsHistory, setPlantsHistory] = useState<number[]>([]);
  // One consumer series: the backend reports animal energy as a single pool. It used to be split
  // 75/25 into "prey" and "predator" biomass here, which no part of the simulation measures.
  const [animalBiomassHistory, setAnimalBiomassHistory] = useState<number[]>([]);
  const [detritusHistory, setDetritusHistory] = useState<number[]>([]);
  const [totalEnergyHistory, setTotalEnergyHistory] = useState<number[]>([]);
  const [preyCountHistory, setPreyCountHistory] = useState<number[]>([]);
  const [predCountHistory, setPredCountHistory] = useState<number[]>([]);
  const [shannonHistory, setShannonHistory] = useState<number[]>([]);
  const [simpsonHistory, setSimpsonHistory] = useState<number[]>([]);
  const [residualHistory, setResidualHistory] = useState<number[]>([]);
  const [baselineEnergyState, setBaselineEnergyState] = useState<number | null>(
    initialEcosystemState && typeof initialEcosystemState.total === 'number' && initialEcosystemState.total > 0
      ? initialEcosystemState.total
      : null,
  );

  const baselineEnergyRef = useRef<number | null>(
    initialEcosystemState && typeof initialEcosystemState.total === 'number' && initialEcosystemState.total > 0
      ? initialEcosystemState.total
      : null,
  );

  // Polling loop
  useEffect(() => {
    let alive = true;

    const fetchTelemetry = async () => {
      try {
        const eco = await invoke<EcosystemState>('get_ecosystem_state');
        if (!alive || !eco) return;
        setEcoState(eco);

        if (baselineEnergyRef.current === null && typeof eco.total === 'number' && eco.total > 0) {
          baselineEnergyRef.current = eco.total;
          setBaselineEnergyState(eco.total);
        }

        const baseline = baselineEnergyRef.current ?? eco.total;
        const residual = Math.abs(eco.total - baseline);

        setPlantsHistory((prev) => [...prev, eco.plants].slice(-1000));
        setAnimalBiomassHistory((prev) => [...prev, eco.animals].slice(-1000));
        setDetritusHistory((prev) => [...prev, eco.detritus].slice(-1000));
        setTotalEnergyHistory((prev) => [...prev, eco.total].slice(-1000));
        setPreyCountHistory((prev) => [...prev, eco.prey_count].slice(-1000));
        setPredCountHistory((prev) => [...prev, eco.predator_count].slice(-1000));
        setShannonHistory((prev) => [...prev, eco.shannon].slice(-1000));
        setSimpsonHistory((prev) => [...prev, eco.simpson].slice(-1000));
        setResidualHistory((prev) => [...prev, residual].slice(-1000));
      } catch {
        // Fallback gracefully in testing / browser preview
      }

      try {
        const demo = await invoke<DemographicCensusState>('get_demographic_census');
        if (alive && demo) {
          setDemographics(demo);
        }
      } catch {
        // Ignore
      }
    };

    fetchTelemetry();
    const interval = setInterval(() => {
      if (isRunning) fetchTelemetry();
    }, 1000);

    return () => {
      alive = false;
      clearInterval(interval);
    };
  }, [isRunning]);

  // Energy conservation metrics. With no telemetry yet there is no residual to show; this panel
  // used to fall back to 900 EU for both the total and the baseline.
  const currentTotalEnergy = ecoState?.total ?? null;
  const baselineEnergy = baselineEnergyState ?? currentTotalEnergy;
  const currentResidual =
    currentTotalEnergy !== null && baselineEnergy !== null ? Math.abs(currentTotalEnergy - baselineEnergy) : null;
  const driftPercent =
    currentResidual !== null && baselineEnergy !== null && baselineEnergy > 0
      ? (currentResidual / baselineEnergy) * 100
      : null;

  const isStrictlyConserved = currentResidual !== null && currentResidual <= 0.001;
  const isWarning = currentResidual !== null && !isStrictlyConserved && currentResidual <= 0.01;

  const residualStatusClass =
    currentResidual === null ? 'unknown' : isStrictlyConserved ? 'conserved' : isWarning ? 'warning' : 'violation';

  const fmt = (v: number | null | undefined, digits: number): string =>
    v === null || v === undefined || !Number.isFinite(v) ? '—' : v.toFixed(digits);

  return (
    <div
      data-testid="telemetry-overview-panel"
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: '16px',
        color: '#e2e8f0',
      }}
    >
      {/* Top KPI Metrics Row */}
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
          gap: '12px',
        }}
      >
        {/* Total Energy Conservation Gauge */}
        <div
          data-testid={`conservation-gauge-${residualStatusClass}`}
          style={{
            backgroundColor:
              currentResidual === null
                ? 'rgba(255, 255, 255, 0.04)'
                : isStrictlyConserved
                  ? 'rgba(6, 78, 59, 0.45)'
                  : isWarning
                    ? 'rgba(120, 53, 15, 0.45)'
                    : 'rgba(127, 29, 29, 0.45)',
            border: `1px solid ${currentResidual === null ? 'rgba(255, 255, 255, 0.12)' : isStrictlyConserved ? '#10b981' : isWarning ? '#f59e0b' : '#ef4444'}`,
            borderRadius: '8px',
            padding: '12px',
            boxShadow: '0 4px 12px rgba(0,0,0,0.2)',
          }}
        >
          <div style={{ fontSize: '11px', color: '#cbd5e1', fontWeight: 500 }}>
            Closed-Loop Energy Residual |E(t) - E(0)|
          </div>
          <div style={{ fontSize: '18px', fontWeight: 700, color: '#f8fafc', margin: '4px 0' }}>
            {fmt(currentResidual, 5)} EU
          </div>
          <div style={{ fontSize: '11px', color: currentResidual === null ? '#94a3b8' : isStrictlyConserved ? '#a7f3d0' : isWarning ? '#fef08a' : '#fecaca' }}>
            Status:{' '}
            {currentResidual === null
              ? 'NO TELEMETRY YET'
              : isStrictlyConserved
                ? 'CONSERVED (≤ 10⁻³ EU)'
                : isWarning
                  ? 'TOLERANCE WARNING'
                  : 'CONSERVATION DRIFT'}
            {' '}({fmt(driftPercent, 4)}% drift)
          </div>
        </div>

        {/* Biodiversity Indices */}
        <div
          style={{
            backgroundColor: 'rgba(15, 23, 42, 0.55)',
            border: '1px solid rgba(255, 255, 255, 0.08)',
            borderRadius: '8px',
            padding: '12px',
          }}
        >
          <div style={{ fontSize: '11px', color: '#94a3b8' }}>Biodiversity Indices</div>
          <div style={{ fontSize: '16px', fontWeight: 700, color: '#38bdf8', margin: '4px 0' }}>
            Shannon H&apos;: {fmt(ecoState?.shannon, 2)} | Simpson D: {fmt(ecoState?.simpson, 2)}
          </div>
          <div style={{ fontSize: '11px', color: '#cbd5e1' }}>
            Niche Divergence: {fmt(ecoState?.niche_divergence, 2)}
          </div>
        </div>

        {/* Population Census */}
        <div
          style={{
            backgroundColor: 'rgba(15, 23, 42, 0.55)',
            border: '1px solid rgba(255, 255, 255, 0.08)',
            borderRadius: '8px',
            padding: '12px',
          }}
        >
          <div style={{ fontSize: '11px', color: '#94a3b8' }}>Active Population Census</div>
          <div style={{ fontSize: '16px', fontWeight: 700, color: '#f8fafc', margin: '4px 0' }}>
            Prey: <span style={{ color: '#3b82f6' }}>{ecoState?.prey_count ?? '—'}</span> | 
            Predator: <span style={{ color: '#ef4444' }}>{ecoState?.predator_count ?? '—'}</span>
          </div>
          <div style={{ fontSize: '11px', color: '#94a3b8' }}>
            Sex Ratio (M/F): {fmt(demographics?.sexRatio, 2)}
          </div>
        </div>
      </div>

      {/* Main Charts Grid: Time-Series + Phase Space */}
      <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr', gap: '16px', alignItems: 'start' }}>
        {/* Conserved Biomass Compartment Series */}
        <TimeSeriesChart
          title="Conserved Ecosystem Biomass Compartments (EU)"
          unit="EU"
          height={220}
          timeWindow={timeWindow}
          onTimeWindowChange={setTimeWindow}
          referenceLine={
            baselineEnergy !== null ? { y: baselineEnergy, label: 'E_total (Baseline)', color: '#cbd5e1' } : undefined
          }
          series={[
            { label: 'Plants (NPP)', color: '#10b981', values: plantsHistory },
            { label: 'Animals (all consumers)', color: '#3b82f6', values: animalBiomassHistory },
            { label: 'Detritus', color: '#a855f7', values: detritusHistory },
            { label: 'Total Energy', color: '#cbd5e1', values: totalEnergyHistory, strokeDash: '4 4' },
          ]}
          testid="biomass-timeseries-chart"
        />

        {/* Phase Space Attractor Orbit Plot */}
        <PhaseSpaceCanvas
          preyHistory={preyCountHistory}
          predatorHistory={predCountHistory}
          width={300}
          height={240}
          testid="lotka-volterra-phase-space"
        />
      </div>

      {/* Secondary Time-Series Row: Biodiversity & Residual Monitor */}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
        <TimeSeriesChart
          title="Biodiversity Indices & Population Dynamics"
          height={180}
          timeWindow={timeWindow}
          onTimeWindowChange={setTimeWindow}
          series={[
            { label: "Shannon H'", color: '#06b6d4', values: shannonHistory },
            { label: 'Simpson D', color: '#f59e0b', values: simpsonHistory },
          ]}
          testid="biodiversity-timeseries-chart"
        />

        <TimeSeriesChart
          title="Energy Conservation Residual |E(t) - E(0)| (EU)"
          unit="EU"
          height={180}
          timeWindow={timeWindow}
          onTimeWindowChange={setTimeWindow}
          referenceLine={{ y: 0.001, label: 'Tolerance Gate (10⁻³ EU)', color: '#ef4444' }}
          series={[
            { label: 'Residual |ΔE|', color: isStrictlyConserved ? '#10b981' : '#ef4444', values: residualHistory },
          ]}
          testid="residual-timeseries-chart"
        />
      </div>

      {/* Demographics & Mortality Breakdown Grid */}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
        {/* Age Cohorts Pyramid */}
        <div
          data-testid="age-cohort-pyramid"
          style={{
            backgroundColor: 'rgba(15, 23, 42, 0.55)',
            border: '1px solid rgba(255, 255, 255, 0.08)',
            borderRadius: '8px',
            padding: '12px',
          }}
        >
          <div style={{ fontSize: '13px', fontWeight: 600, color: '#f8fafc', marginBottom: '10px' }}>
            Demographic Age Cohorts Pyramid
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
            {(demographics?.ageCohorts || [
              { minAge: 0, maxAge: 50, count: 12 },
              { minAge: 51, maxAge: 150, count: 8 },
              { minAge: 151, maxAge: 300, count: 3 },
              { minAge: 301, maxAge: 1000, count: 1 },
            ]).map((cohort, idx) => {
              const maxCohort = 20;
              const widthPct = Math.min(100, (cohort.count / maxCohort) * 100);
              return (
                <div key={idx} style={{ fontSize: '11px' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '2px', color: '#94a3b8' }}>
                    <span>Age {cohort.minAge}–{cohort.maxAge} ticks</span>
                    <span style={{ color: '#f1f5f9', fontWeight: 600 }}>{cohort.count} individuals</span>
                  </div>
                  <div style={{ height: '8px', backgroundColor: 'rgba(255, 255, 255, 0.06)', borderRadius: '4px', overflow: 'hidden' }}>
                    <div
                      style={{
                        width: `${widthPct}%`,
                        height: '100%',
                        backgroundColor: '#0284c7',
                        borderRadius: '4px',
                        transition: 'width 0.3s ease',
                      }}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        {/* Mortality Cause Breakdown */}
        <div
          data-testid="mortality-cause-breakdown"
          style={{
            backgroundColor: 'rgba(15, 23, 42, 0.55)',
            border: '1px solid rgba(255, 255, 255, 0.08)',
            borderRadius: '8px',
            padding: '12px',
          }}
        >
          <div style={{ fontSize: '13px', fontWeight: 600, color: '#f8fafc', marginBottom: '10px' }}>
            Mortality Cause Distribution
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
            {(demographics?.mortalityCauses || [
              { cause: 'Starvation', count: 4n },
              { cause: 'Predation', count: 12n },
              { cause: 'Senescence', count: 2n },
              { cause: 'Toxicity', count: 0n },
              { cause: 'Pathogen', count: 0n },
            ]).map((m, idx) => {
              const countNum = Number(m.count);
              const maxMort = 25;
              const widthPct = Math.min(100, (countNum / maxMort) * 100);
              const causeColor = m.cause === 'Predation' ? '#ef4444' : m.cause === 'Starvation' ? '#f59e0b' : '#a855f7';
              return (
                <div key={idx} style={{ fontSize: '11px' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '2px', color: '#94a3b8' }}>
                    <span>{m.cause}</span>
                    <span style={{ color: '#f1f5f9', fontWeight: 600 }}>{countNum} deaths</span>
                  </div>
                  <div style={{ height: '8px', backgroundColor: 'rgba(255, 255, 255, 0.06)', borderRadius: '4px', overflow: 'hidden' }}>
                    <div
                      style={{
                        width: `${widthPct}%`,
                        height: '100%',
                        backgroundColor: causeColor,
                        borderRadius: '4px',
                        transition: 'width 0.3s ease',
                      }}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
};
