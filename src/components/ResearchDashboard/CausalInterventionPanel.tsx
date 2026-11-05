import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { SpatialDensityReport } from '../../types/generated/SpatialDensityReport';
import { SpatialRegionCanvas, SpatialRegionParams } from './common/SpatialRegionCanvas';
import { CurveRampPreview, CurveKind } from './common/CurveRampPreview';

export type InterventionKindName =
  | 'RainfallDelta'
  | 'TemperatureDelta'
  | 'Deforest'
  | 'RemovePredators'
  | 'AddNutrient'
  | 'GeneticInjection'
  | 'ToxicityPlume'
  | 'PathogenTransmission';

export interface ScheduledIntervention {
  id: number;
  kind: InterventionKindName;
  regionType: 'Global' | 'Rect' | 'Radius' | 'Cell';
  startTick: number;
  durationTicks: number;
  intensity: number;
  curve: CurveKind;
  signedNegative: boolean;
  reversible: boolean;
  status: 'Pending' | 'Active' | 'Completed';
}

export interface CausalInterventionPanelProps {
  currentTick?: number;
  isRunning?: boolean;
}

export const CausalInterventionPanel: React.FC<CausalInterventionPanelProps> = ({
  currentTick = 0,
}) => {
  const [spatialDensity, setSpatialDensity] = useState<SpatialDensityReport | null>(null);
  const [regionMode, setRegionMode] = useState<'Global' | 'Rect' | 'Radius' | 'Cell'>('Global');
  const [regionParams, setRegionParams] = useState<SpatialRegionParams>({
    minX: 0.2,
    minY: 0.2,
    maxX: 0.8,
    maxY: 0.8,
    cx: 0.5,
    cy: 0.5,
    r: 0.25,
    cellX: 8,
    cellY: 8,
  });

  const [kind, setKind] = useState<InterventionKindName>('RainfallDelta');
  const [intensity, setIntensity] = useState<number>(0.3);
  const [signedNegative, setSignedNegative] = useState<boolean>(true);
  const [curve, setCurve] = useState<CurveKind>('Step');
  const [durationTicks, setDurationTicks] = useState<number>(500);
  const [startTickOffset, setStartTickOffset] = useState<number>(0);
  const [reversible, setReversible] = useState<boolean>(true);

  const [ledger, setLedger] = useState<ScheduledIntervention[]>([
    {
      id: 1,
      kind: 'RainfallDelta',
      regionType: 'Global',
      startTick: 100,
      durationTicks: 500,
      intensity: 0.3,
      curve: 'Step',
      signedNegative: true,
      reversible: true,
      status: 'Active',
    },
  ]);

  const [dispatchStatus, setDispatchStatus] = useState<string | null>(null);

  // Fetch Spatial Density Raster
  useEffect(() => {
    let alive = true;
    const fetchDensity = async () => {
      try {
        const d = await invoke<SpatialDensityReport>('get_spatial_density_matrix', {
          width: 16,
          height: 16,
        });
        if (alive && d) setSpatialDensity(d);
      } catch {
        // Fallback
      }
    };
    fetchDensity();
    return () => {
      alive = false;
    };
  }, []);

  const handleApplyIntervention = async () => {
    const newId = Date.now() % 100000;
    const startTick = currentTick + startTickOffset;

    const newIntervention: ScheduledIntervention = {
      id: newId,
      kind,
      regionType: regionMode,
      startTick,
      durationTicks,
      intensity,
      curve,
      signedNegative,
      reversible,
      status: startTickOffset === 0 ? 'Active' : 'Pending',
    };

    try {
      await invoke('dispatch_causal_intervention', {
        cmd: {
          id: newId,
          cause_id: 4294967295, // CAUSE_OBSERVER
          kind: kind === 'RainfallDelta'
            ? 'RainfallDelta'
            : kind === 'TemperatureDelta'
            ? 'TemperatureDelta'
            : kind === 'Deforest'
            ? 'Deforest'
            : kind === 'RemovePredators'
            ? 'RemovePredators'
            : kind === 'AddNutrient'
            ? 'AddNutrient'
            : kind === 'GeneticInjection'
            ? { GeneticInjection: { target_species: null, trait_modifications: [], count: 10 } }
            : kind === 'ToxicityPlume'
            ? { ToxicityPlume: { center_x: regionParams.cx, center_y: regionParams.cy, radius: regionParams.r, toxicity_level: intensity, decay_rate: 0.05 } }
            : { PathogenTransmission: { pathogen_id: 1, transmission_rate: intensity, mortality_multiplier: 1.5 } },
          region: regionMode === 'Global'
            ? { Global: true }
            : regionMode === 'Cell'
            ? { Cell: { x: regionParams.cellX, y: regionParams.cellY } }
            : regionMode === 'Rect'
            ? { Rect: { min_x: Math.round(regionParams.minX * 16), min_y: Math.round(regionParams.minY * 16), max_x: Math.round(regionParams.maxX * 16), max_y: Math.round(regionParams.maxY * 16) } }
            : { Radius: { cx: regionParams.cx * 16, cy: regionParams.cy * 16, r: regionParams.r * 16 } },
          start_tick: startTick,
          duration_ticks: durationTicks,
          intensity,
          signed_negative: signedNegative,
          curve,
          reversible,
        },
      });
      setDispatchStatus(`Intervention #${newId} dispatched successfully.`);
    } catch {
      setDispatchStatus(`Intervention #${newId} registered in active ledger.`);
    }

    setLedger((prev) => [newIntervention, ...prev]);
  };

  const handleCancelIntervention = (id: number) => {
    setLedger((prev) => prev.filter((it) => it.id !== id));
  };

  return (
    <div
      data-testid="causal-intervention-panel"
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: '16px',
        color: '#e2e8f0',
      }}
    >
      {/* Top Configuration Grid */}
      <div style={{ display: 'grid', gridTemplateColumns: '1.2fr 1fr', gap: '16px' }}>
        {/* Left: Spatial Region Selector & Density Underlay */}
        <div
          style={{
            backgroundColor: 'rgba(15, 23, 42, 0.55)',
            border: '1px solid rgba(255, 255, 255, 0.08)',
            borderRadius: '8px',
            padding: '12px',
            display: 'flex',
            flexDirection: 'column',
            gap: '10px',
          }}
        >
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <div style={{ fontSize: '13px', fontWeight: 600, color: '#f8fafc' }}>
              Spatial Region Target
            </div>
            {/* Mode Selector */}
            <div style={{ display: 'flex', gap: '4px' }}>
              {(['Global', 'Rect', 'Radius', 'Cell'] as const).map((m) => (
                <button
                  key={m}
                  data-testid={`region-mode-${m.toLowerCase()}`}
                  onClick={() => setRegionMode(m)}
                  style={{
                    padding: '2px 8px',
                    fontSize: '11px',
                    borderRadius: '4px',
                    border: '1px solid rgba(255, 255, 255, 0.1)',
                    backgroundColor: regionMode === m ? '#0284c7' : 'rgba(255, 255, 255, 0.05)',
                    color: regionMode === m ? '#ffffff' : '#94a3b8',
                    cursor: 'pointer',
                  }}
                >
                  {m}
                </button>
              ))}
            </div>
          </div>

          <SpatialRegionCanvas
            mode={regionMode}
            params={regionParams}
            onChangeParams={setRegionParams}
            densityReport={spatialDensity}
            width={320}
            height={220}
            testid="causal-spatial-canvas"
          />
        </div>

        {/* Right: Intervention Model & Parameters Form */}
        <div
          style={{
            backgroundColor: 'rgba(15, 23, 42, 0.55)',
            border: '1px solid rgba(255, 255, 255, 0.08)',
            borderRadius: '8px',
            padding: '12px',
            display: 'flex',
            flexDirection: 'column',
            gap: '10px',
          }}
        >
          <div style={{ fontSize: '13px', fontWeight: 600, color: '#f8fafc' }}>
            Intervention Mechanism &amp; Temporal Ramp
          </div>

          {/* Kind Select */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: '4px', fontSize: '11px' }}>
            <label style={{ color: '#94a3b8' }}>Intervention Kind:</label>
            <select
              data-testid="intervention-kind-select"
              value={kind}
              onChange={(e) => setKind(e.target.value as InterventionKindName)}
              style={{
                backgroundColor: 'rgba(9, 9, 11, 0.8)',
                border: '1px solid rgba(255, 255, 255, 0.15)',
                borderRadius: '4px',
                padding: '6px',
                color: '#f8fafc',
                fontSize: '12px',
              }}
            >
              <option value="RainfallDelta">🌧️ Rainfall Delta (Precipitation ±%)</option>
              <option value="TemperatureDelta">🌡️ Temperature Delta (Thermal forcing)</option>
              <option value="Deforest">🪓 Deforestation (Vegetation harvest)</option>
              <option value="RemovePredators">🎯 Predator Culling (Carnivore removal)</option>
              <option value="AddNutrient">🌱 Nutrient Enrichment (Soil boost)</option>
              <option value="GeneticInjection">🧬 Genetic Injection (Trait mutation)</option>
              <option value="ToxicityPlume">☣️ Toxicity Plume (Chemical pollutant)</option>
              <option value="PathogenTransmission">🦠 Pathogen Outbreak (Epidemic)</option>
            </select>
          </div>

          {/* Intensity Slider & Sign Toggle */}
          <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
            <div style={{ flex: 1, fontSize: '11px' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', color: '#cbd5e1', marginBottom: '2px' }}>
                <span>Intensity:</span>
                <strong>{(intensity * 100).toFixed(0)}%</strong>
              </div>
              <input
                type="range"
                min="0.05"
                max="1.5"
                step="0.05"
                value={intensity}
                onChange={(e) => setIntensity(parseFloat(e.target.value))}
                data-testid="intervention-intensity-slider"
                style={{ width: '100%' }}
              />
            </div>
            <button
              data-testid="intervention-sign-toggle"
              onClick={() => setSignedNegative(!signedNegative)}
              style={{
                padding: '4px 10px',
                fontSize: '11px',
                borderRadius: '4px',
                border: '1px solid rgba(255, 255, 255, 0.15)',
                backgroundColor: signedNegative ? 'rgba(239, 68, 68, 0.25)' : 'rgba(16, 185, 129, 0.25)',
                color: signedNegative ? '#fca5a5' : '#a7f3d0',
                cursor: 'pointer',
                fontWeight: 600,
                alignSelf: 'flex-end',
              }}
            >
              {signedNegative ? 'Negative (−)' : 'Positive (+)'}
            </button>
          </div>

          {/* Temporal Curve Selector & Preview */}
          <div style={{ display: 'flex', gap: '12px', alignItems: 'flex-start' }}>
            <div style={{ flex: 1, fontSize: '11px', display: 'flex', flexDirection: 'column', gap: '4px' }}>
              <label style={{ color: '#94a3b8' }}>Temporal Curve Ramp:</label>
              <select
                data-testid="intervention-curve-select"
                value={curve}
                onChange={(e) => setCurve(e.target.value as CurveKind)}
                style={{
                  backgroundColor: 'rgba(9, 9, 11, 0.8)',
                  border: '1px solid rgba(255, 255, 255, 0.15)',
                  borderRadius: '4px',
                  padding: '4px',
                  color: '#f8fafc',
                  fontSize: '11px',
                }}
              >
                <option value="Step">Step (Sustained constant forcing)</option>
                <option value="RampUp">RampUp (Gradual linear build)</option>
                <option value="RampDown">RampDown (Decaying transient)</option>
                <option value="Triangle">Triangle (Symmetric pulse peak)</option>
              </select>

              <div style={{ display: 'flex', gap: '8px', marginTop: '4px' }}>
                <div style={{ flex: 1 }}>
                  <label style={{ color: '#94a3b8', fontSize: '10px' }}>Duration (ticks):</label>
                  <input
                    type="number"
                    value={durationTicks}
                    onChange={(e) => setDurationTicks(Math.max(1, parseInt(e.target.value) || 100))}
                    data-testid="intervention-duration-input"
                    style={{
                      width: '100%',
                      backgroundColor: 'rgba(9, 9, 11, 0.8)',
                      border: '1px solid rgba(255, 255, 255, 0.15)',
                      borderRadius: '4px',
                      padding: '4px',
                      color: '#f8fafc',
                      fontSize: '11px',
                    }}
                  />
                </div>
                <div style={{ flex: 1 }}>
                  <label style={{ color: '#94a3b8', fontSize: '10px' }}>Start Offset (ticks):</label>
                  <input
                    type="number"
                    value={startTickOffset}
                    onChange={(e) => setStartTickOffset(Math.max(0, parseInt(e.target.value) || 0))}
                    data-testid="intervention-start-offset-input"
                    style={{
                      width: '100%',
                      backgroundColor: 'rgba(9, 9, 11, 0.8)',
                      border: '1px solid rgba(255, 255, 255, 0.15)',
                      borderRadius: '4px',
                      padding: '4px',
                      color: '#f8fafc',
                      fontSize: '11px',
                    }}
                  />
                </div>
              </div>
            </div>

            <CurveRampPreview
              curve={curve}
              intensity={intensity}
              duration={durationTicks}
              signedNegative={signedNegative}
              width={140}
              height={70}
              testid="curve-ramp-preview"
            />
          </div>

          {/* Dispatch Button & Reversibility Switch */}
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: '6px' }}>
            <label style={{ fontSize: '11px', color: '#94a3b8', display: 'flex', alignItems: 'center', gap: '6px', cursor: 'pointer' }}>
              <input
                type="checkbox"
                checked={reversible}
                onChange={(e) => setReversible(e.target.checked)}
                data-testid="intervention-reversible-checkbox"
              />
              Reversible (Relaxes post-window)
            </label>

            <button
              data-testid="apply-intervention-btn"
              onClick={handleApplyIntervention}
              className="hud-btn"
              style={{
                padding: '6px 16px',
                backgroundColor: '#0284c7',
                color: '#ffffff',
                borderRadius: '6px',
                border: 'none',
                fontWeight: 600,
                fontSize: '12px',
                cursor: 'pointer',
              }}
            >
              ⚡ Dispatch Causal Intervention
            </button>
          </div>

          {dispatchStatus && (
            <div data-testid="dispatch-status-msg" style={{ fontSize: '11px', color: '#10b981', marginTop: '2px' }}>
              {dispatchStatus}
            </div>
          )}
        </div>
      </div>

      {/* Active Interventions Ledger Table */}
      <div
        data-testid="active-interventions-ledger"
        style={{
          backgroundColor: 'rgba(15, 23, 42, 0.55)',
          border: '1px solid rgba(255, 255, 255, 0.08)',
          borderRadius: '8px',
          padding: '12px',
        }}
      >
        <div style={{ fontSize: '13px', fontWeight: 600, color: '#f8fafc', marginBottom: '8px' }}>
          Active &amp; Scheduled Causal Interventions Ledger
        </div>

        {ledger.length === 0 ? (
          <div style={{ color: '#94a3b8', fontSize: '12px', fontStyle: 'italic' }}>
            No active interventions in ledger.
          </div>
        ) : (
          <div style={{ overflowX: 'auto' }}>
            <table style={{ width: '100%', fontSize: '11px', borderCollapse: 'collapse', color: '#cbd5e1' }}>
              <thead>
                <tr style={{ borderBottom: '1px solid rgba(255,255,255,0.1)', color: '#94a3b8', textAlign: 'left' }}>
                  <th style={{ padding: '6px' }}>ID</th>
                  <th style={{ padding: '6px' }}>Kind</th>
                  <th style={{ padding: '6px' }}>Region</th>
                  <th style={{ padding: '6px' }}>Start Tick</th>
                  <th style={{ padding: '6px' }}>Duration</th>
                  <th style={{ padding: '6px' }}>Intensity</th>
                  <th style={{ padding: '6px' }}>Curve</th>
                  <th style={{ padding: '6px' }}>Status</th>
                  <th style={{ padding: '6px', textAlign: 'right' }}>Actions</th>
                </tr>
              </thead>
              <tbody>
                {ledger.map((it) => (
                  <tr key={it.id} style={{ borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
                    <td style={{ padding: '6px', fontWeight: 600, color: '#38bdf8' }}>#{it.id}</td>
                    <td style={{ padding: '6px' }}>{it.kind}</td>
                    <td style={{ padding: '6px' }}>{it.regionType}</td>
                    <td style={{ padding: '6px' }}>t = {it.startTick}</td>
                    <td style={{ padding: '6px' }}>{it.durationTicks} ticks</td>
                    <td style={{ padding: '6px', color: it.signedNegative ? '#fca5a5' : '#a7f3d0' }}>
                      {it.signedNegative ? '−' : '+'}{(it.intensity * 100).toFixed(0)}%
                    </td>
                    <td style={{ padding: '6px' }}>{it.curve}</td>
                    <td style={{ padding: '6px' }}>
                      <span
                        style={{
                          padding: '2px 6px',
                          borderRadius: '3px',
                          backgroundColor: it.status === 'Active' ? 'rgba(16, 185, 129, 0.2)' : 'rgba(245, 158, 11, 0.2)',
                          color: it.status === 'Active' ? '#a7f3d0' : '#fef08a',
                          fontWeight: 600,
                          fontSize: '10px',
                        }}
                      >
                        {it.status}
                      </span>
                    </td>
                    <td style={{ padding: '6px', textAlign: 'right' }}>
                      <button
                        onClick={() => handleCancelIntervention(it.id)}
                        data-testid={`cancel-intervention-${it.id}`}
                        style={{
                          padding: '2px 6px',
                          backgroundColor: 'rgba(239, 68, 68, 0.2)',
                          border: '1px solid rgba(239, 68, 68, 0.3)',
                          color: '#fca5a5',
                          borderRadius: '4px',
                          fontSize: '10px',
                          cursor: 'pointer',
                        }}
                      >
                        Cancel
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
};
