// W2-lite: prototype, to be replaced.
import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface BundledManifestEntry {
  id: string;
  label: string;
  manifest: Record<string, unknown>;
}

const SEL2P_SELECTED_MANIFEST: Record<string, unknown> = {
  schema_version: 1,
  experiment_id: 'sel2-p-food-scarcity-body-mass::selected',
  name: 'SEL2-P selected - 16 spread lakes, scale 0.63, mutation 0.05',
  world_identity: {
    seed: 0,
    generator_version: 0,
    width: 0,
    height: 0,
    checksum: 0,
  },
  laws: {
    schema_version: 1,
    baseline_energy: {
      eu_unit: 'EU',
    },
    exotic_energy: null,
  },
  initial_conditions: {
    schema_version: 1,
    values: [
      ['live.founders', 100],
      ['live.predator_fraction', 0],
      ['live.trees', 8],
      ['live.lakes', 16],
      ['live.lake_layout', 1],
      ['live.resource_capacity_scale', 0.63],
      ['live.plant_reserve', 0],
      ['live.prey_cap', 320],
      ['live.dehydration_ticks', 960],
      ['live.mortality', 1],
      ['live.reproduction', 1],
      ['live.locomotion', 1],
      ['live.drive_floor', 0.5],
      ['live.thirst_steering', 1],
      ['live.evolved_brains', 0],
      ['live.mutation_rate', 0.05],
      ['live.drift_control', 0],
      ['live.drift_reset_ticks', 0],
    ],
  },
  interventions: [],
  seeds: [
    780001, 781001, 782001, 783001, 784001, 785001, 786001, 787001, 788001, 789001, 790001,
    791001, 792001, 793001, 794001, 795001, 796001, 797001, 798001, 799001,
  ],
  duration_ticks: 200000,
  sample_period: 500,
  observable_ids: [
    'plants',
    'detritus',
    'live.animals_eu',
    'live.dormant_eu',
    'live.closed_eu_total',
    'live.agent_count',
    'live.herbivore_count',
    'live.predator_count',
    'live.food_items',
    'live.standing_crop',
    'live.mean_agent_energy',
    'live.season_phase',
    'live.trait_mass_mean',
    'live.trait_mass_var',
    'live.trait_mass_p10',
    'live.trait_mass_p90',
    'live.shannon',
    'live.simpson',
    'live.births',
    'live.deaths',
    'live.mean_lifespan',
    'live.archive_coverage',
    'live.mean_energy_integral',
    'live.trait_mass_mean_west',
    'live.trait_mass_mean_east',
  ],
  exotic_interventions: [],
  observer: {
    mode: 'absent',
  },
};

const BUNDLED_MANIFESTS: readonly BundledManifestEntry[] = [
  {
    id: 'sel2p-selected',
    label: 'SEL2-P selected (16 lakes, scale 0.63, 100 founders)',
    manifest: SEL2P_SELECTED_MANIFEST,
  },
];

export interface ExperimentWorldControlProps {
  /** Whether the engine is currently reported as running by the parent status state. */
  running?: boolean;
  /** Called after `start_from_manifest` succeeds with the started manifest name. */
  onStarted?: (manifestName: string) => void;
}

export const ExperimentWorldControl: React.FC<ExperimentWorldControlProps> = ({
  running,
  onStarted,
}) => {
  const [selectedId, setSelectedId] = useState<string>(BUNDLED_MANIFESTS[0].id);
  const [activeManifestName, setActiveManifestName] = useState<string | null>(null);
  const [busy, setBusy] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  const selectedEntry =
    BUNDLED_MANIFESTS.find((entry) => entry.id === selectedId) ?? BUNDLED_MANIFESTS[0];
  const showingActiveManifest = activeManifestName !== null && running !== false;

  const handleOpen = async () => {
    setBusy(true);
    setError(null);
    try {
      const manifestJson = JSON.stringify(selectedEntry.manifest);
      const startedName = await invoke<string>('start_from_manifest', { manifestJson });
      setActiveManifestName(startedName);
      onStarted?.(startedName);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      data-testid="experiment-world-control"
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: '6px',
        border: '1px solid #cbd5e0',
        borderRadius: '4px',
        padding: '6px 10px',
        backgroundColor: 'white',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
        <label
          htmlFor="experiment-manifest-select"
          style={{ fontSize: '13px', fontWeight: 'bold', color: '#2d3748' }}
        >
          Experiment world:
        </label>
        <select
          id="experiment-manifest-select"
          data-testid="experiment-manifest-select"
          value={selectedEntry.id}
          onChange={(e) => setSelectedId(e.target.value)}
          disabled={busy}
          style={{ padding: '4px 8px', border: '1px solid #cbd5e0', borderRadius: '4px' }}
        >
          {BUNDLED_MANIFESTS.map((entry) => (
            <option key={entry.id} value={entry.id}>
              {entry.label}
            </option>
          ))}
        </select>
        <button
          type="button"
          data-testid="open-experiment-world-button"
          onClick={handleOpen}
          disabled={busy}
          style={{
            padding: '6px 12px',
            fontSize: '13px',
            fontWeight: 'bold',
            backgroundColor: busy ? '#a0aec0' : '#2b6cb0',
            color: 'white',
            border: 'none',
            borderRadius: '4px',
            cursor: busy ? 'wait' : 'pointer',
          }}
        >
          {busy ? 'Opening…' : 'Open experiment world'}
        </button>
      </div>

      {showingActiveManifest && (
        <div
          data-testid="active-experiment-manifest-banner"
          style={{ fontSize: '12px', color: '#2b6cb0' }}
        >
          <strong data-testid="active-experiment-manifest">{activeManifestName}</strong>{' '}
          <span data-testid="experiment-viewer-notice" style={{ color: '#4a5568' }}>
            (Viewer only — background app threads run, so trajectories may differ from a headless
            experiment and do not reproduce an experiment.)
          </span>
        </div>
      )}

      {error && (
        <div
          role="alert"
          data-testid="experiment-manifest-error"
          style={{ fontSize: '12px', color: '#c53030' }}
        >
          {error}
        </div>
      )}
    </div>
  );
};
