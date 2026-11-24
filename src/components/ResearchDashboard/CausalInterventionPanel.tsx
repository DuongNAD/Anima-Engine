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

const GENETIC_INJECTION_REASON = 'not implemented in the live world';
const GENETIC_INJECTION_FINDING = 'F-071';
const UNIMPLEMENTED_REASONS: Partial<Record<InterventionKindName, string>> = {
  GeneticInjection: GENETIC_INJECTION_REASON,
};

/**
 * Interventions cannot be dispatched from the desktop app: the backend has no
 * `dispatch_causal_intervention` command. The panel keeps what is real (the region picker over the
 * live spatial density raster, and the curve preview) and says plainly that dispatch is not
 * available. It used to start with a pre-filled "Active" RainfallDelta row, and when the missing
 * command threw it reported the intervention as "registered in active ledger".
 */
export const CausalInterventionPanel: React.FC = () => {
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
              onChange={(e) => {
                const next = e.target.value as InterventionKindName;
                if (next in UNIMPLEMENTED_REASONS) return;
                setKind(next);
              }}
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
              <option value="GeneticInjection" disabled title={GENETIC_INJECTION_REASON}>
                🧬 Genetic Injection — disabled: {GENETIC_INJECTION_REASON} ({GENETIC_INJECTION_FINDING})
              </option>
              <option value="ToxicityPlume">☣️ Toxicity Plume (Chemical pollutant)</option>
              <option value="PathogenTransmission">🦠 Pathogen Outbreak (Epidemic)</option>
            </select>
            <div data-testid="unimplemented-interventions-notice" style={{ color: '#cbd5e1', marginTop: 2 }}>
              Disabled: <code>GeneticInjection</code> — {GENETIC_INJECTION_REASON} (
              <a
                data-testid="unimplemented-intervention-finding-link"
                href="FINDINGS.md#f-071"
                style={{ color: '#fbbf24', textDecoration: 'underline' }}
              >
                FINDINGS.md {GENETIC_INJECTION_FINDING}
              </a>
              ).
            </div>
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
              disabled
              title="Not available in this build"
              className="hud-btn"
              style={{
                padding: '6px 16px',
                backgroundColor: 'rgba(255, 255, 255, 0.06)',
                color: '#94a3b8',
                borderRadius: '6px',
                border: '1px solid rgba(255, 255, 255, 0.1)',
                fontWeight: 600,
                fontSize: '12px',
                cursor: 'not-allowed',
              }}
            >
              ⚡ Dispatch Causal Intervention
            </button>
          </div>

          <div data-testid="intervention-dispatch-unavailable" style={{ fontSize: '11px', color: '#94a3b8', marginTop: '2px' }}>
            Dispatch is not available in this build: the desktop backend has no command that applies an
            intervention. Headless runs declare interventions in an experiment manifest for <code>anima-lab</code>.
          </div>
        </div>
      </div>
    </div>
  );
};
