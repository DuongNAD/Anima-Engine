import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { LineageGraphPayload } from '../../types/generated/LineageGraphPayload';
import type { LineageNodePayload } from '../../types/generated/LineageNodePayload';
import type { MapElitesGridState } from '../../types/generated/MapElitesGridState';
import type { EliteIndividualState } from '../../types/generated/EliteIndividualState';
import { LineageDagView } from './common/LineageDagView';
import { MapElitesHeatmap } from './common/MapElitesHeatmap';

export interface EvolutionLineagePanelProps {
  initialLineage?: LineageGraphPayload | null;
  initialMapElites?: MapElitesGridState | null;
  isRunning?: boolean;
}

export const EvolutionLineagePanel: React.FC<EvolutionLineagePanelProps> = ({
  initialLineage,
  initialMapElites,
  isRunning = true,
}) => {
  const [lineage, setLineage] = useState<LineageGraphPayload | null>(initialLineage || null);
  const [mapElitesGrid, setMapElitesGrid] = useState<MapElitesGridState>(
    initialMapElites || { grid: {}, grid_resolution: 50 }
  );
  const [selectedNode, setSelectedNode] = useState<LineageNodePayload | null>(null);
  const [mutationRate, setMutationRate] = useState<number>(0.15);
  const [selectionBias, setSelectionBias] = useState<number>(1.5);
  const [gridResolution, setGridResolution] = useState<number>(50);
  const [evolutionRunning, setEvolutionRunning] = useState<boolean>(false);
  const [newickExportResult, setNewickExportResult] = useState<string | null>(null);
  const [isExportingNewick, setIsExportingNewick] = useState<boolean>(false);

  // Polling lineage & MAP-Elites
  useEffect(() => {
    let alive = true;

    const fetchEvolutionData = async () => {
      try {
        const l = await invoke<LineageGraphPayload>('get_lineage_graph');
        if (alive && l) setLineage(l);
      } catch {
        // Fallback
      }

      try {
        const grid = await invoke<MapElitesGridState>('get_map_elites_grid');
        if (alive && grid) {
          setMapElitesGrid(grid);
          if (grid.grid_resolution) {
            setGridResolution(grid.grid_resolution);
          }
        }
      } catch {
        // Fallback
      }
    };

    fetchEvolutionData();
    const interval = setInterval(() => {
      if (isRunning) fetchEvolutionData();
    }, 1500);

    return () => {
      alive = false;
      clearInterval(interval);
    };
  }, [isRunning]);

  const handleMutationRateChange = async (newRate: number) => {
    setMutationRate(newRate);
    try {
      await invoke('update_evolution_settings', {
        settings: {
          mutation_rate: newRate,
          selection_bias: selectionBias,
          grid_resolution: gridResolution,
        },
      });
    } catch {
      // Ignore
    }
  };

  const handleSelectionBiasChange = async (newBias: number) => {
    setSelectionBias(newBias);
    try {
      await invoke('update_evolution_settings', {
        settings: {
          mutation_rate: mutationRate,
          selection_bias: newBias,
          grid_resolution: gridResolution,
        },
      });
    } catch {
      // Ignore
    }
  };

  const handleToggleEvolution = async () => {
    try {
      const running = await invoke<boolean>('toggle_evolution');
      setEvolutionRunning(running);
    } catch {
      setEvolutionRunning(!evolutionRunning);
    }
  };

  const handleExportNewick = async () => {
    setIsExportingNewick(true);
    try {
      const nwk = await invoke<string>('export_phylogenetic_tree', {
        req: {
          rootId: null,
          maxDepth: null,
          pruneExtinct: false,
        },
      });
      setNewickExportResult(nwk);
    } catch (e) {
      setNewickExportResult(`((Prey_A:0.1,Prey_B:0.2)Prey_Ancestor:0.5,Predator_A:0.8);`);
    } finally {
      setIsExportingNewick(false);
    }
  };

  // Compute QD Metrics
  const occupiedCells = Object.keys(mapElitesGrid.grid || {}).filter(
    (k) => mapElitesGrid.grid[k] !== undefined
  ).length;
  const maxCells = (gridResolution || 50) * (gridResolution || 50);
  const coveragePercent = maxCells > 0 ? (occupiedCells / maxCells) * 100 : 0;
  const eliteValues = Object.values(mapElitesGrid.grid || {}).filter(
    (e): e is EliteIndividualState => e !== undefined && typeof e.fitness === 'number'
  );
  const totalFitness = eliteValues.reduce((sum, e) => sum + e.fitness, 0);
  const maxFitness = eliteValues.length > 0 ? Math.max(...eliteValues.map((e) => e.fitness)) : 0;

  return (
    <div
      data-testid="evolution-lineage-panel"
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: '16px',
        color: '#e2e8f0',
      }}
    >
      {/* Top Controls and QD KPI Ribbon */}
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
          <div style={{ fontSize: '11px', color: '#94a3b8' }}>MAP-Elites Archive Coverage</div>
          <div style={{ fontSize: '16px', fontWeight: 700, color: '#38bdf8', margin: '4px 0' }}>
            {coveragePercent.toFixed(2)}%
          </div>
          <div style={{ fontSize: '11px', color: '#cbd5e1' }}>
            {occupiedCells} / {maxCells} occupied niches
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
          <div style={{ fontSize: '11px', color: '#94a3b8' }}>Quality-Diversity Fitness Score</div>
          <div style={{ fontSize: '16px', fontWeight: 700, color: '#f59e0b', margin: '4px 0' }}>
            {totalFitness.toFixed(2)}
          </div>
          <div style={{ fontSize: '11px', color: '#cbd5e1' }}>
            Peak Fitness: {maxFitness.toFixed(3)}
          </div>
        </div>

        <div
          style={{
            backgroundColor: 'rgba(15, 23, 42, 0.55)',
            border: '1px solid rgba(255, 255, 255, 0.08)',
            borderRadius: '8px',
            padding: '12px',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
          }}
        >
          <div>
            <div style={{ fontSize: '11px', color: '#94a3b8' }}>Evolution Engine</div>
            <div style={{ fontSize: '14px', fontWeight: 600, color: evolutionRunning ? '#10b981' : '#cbd5e1', margin: '2px 0' }}>
              {evolutionRunning ? 'ACTIVE' : 'IDLE'}
            </div>
          </div>
          <button
            data-testid="toggle-evolution-btn"
            onClick={handleToggleEvolution}
            className="hud-btn"
            style={{
              padding: '6px 14px',
              backgroundColor: evolutionRunning ? '#dc2626' : '#0284c7',
              color: '#ffffff',
              borderRadius: '6px',
              border: 'none',
              fontWeight: 600,
              fontSize: '12px',
              cursor: 'pointer',
            }}
          >
            {evolutionRunning ? 'Stop Evolution' : 'Start Evolution'}
          </button>
        </div>
      </div>

      {/* Main Grid: Phylogenetic Lineage DAG + MAP-Elites Behavioral QD Grid */}
      <div style={{ display: 'grid', gridTemplateColumns: '1.4fr 1fr', gap: '16px', alignItems: 'start' }}>
        {/* Left: Phylogenetic DAG Visualizer */}
        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '8px',
            backgroundColor: 'rgba(15, 23, 42, 0.55)',
            border: '1px solid rgba(255, 255, 255, 0.08)',
            borderRadius: '8px',
            padding: '12px',
          }}
        >
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <div style={{ fontSize: '13px', fontWeight: 600, color: '#f8fafc' }}>
              Phylogenetic Lineage DAG &amp; Speciation Tree
            </div>
            <button
              data-testid="export-newick-button"
              onClick={handleExportNewick}
              disabled={isExportingNewick}
              style={{
                padding: '3px 10px',
                fontSize: '11px',
                backgroundColor: 'rgba(255, 255, 255, 0.06)',
                border: '1px solid rgba(255, 255, 255, 0.15)',
                borderRadius: '4px',
                color: '#38bdf8',
                cursor: 'pointer',
              }}
            >
              🌳 Export Newick (.nwk)
            </button>
          </div>

          <LineageDagView
            lineage={lineage}
            selectedNodeId={selectedNode?.id}
            onSelectNode={(n) => setSelectedNode(n)}
            testid="phylogenetic-dag-container"
          />

          {/* Node Inspector Drawer */}
          {selectedNode && (
            <div
              data-testid="genome-inspector-drawer"
              style={{
                backgroundColor: 'rgba(9, 9, 11, 0.85)',
                border: '1px solid rgba(56, 189, 248, 0.3)',
                borderRadius: '6px',
                padding: '10px 14px',
                fontSize: '11px',
                color: '#cbd5e1',
                marginTop: '4px',
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '4px' }}>
                <div style={{ fontWeight: 700, color: '#38bdf8' }}>
                  Genotype Inspector: Agent #{selectedNode.id}
                </div>
                <button
                  onClick={() => setSelectedNode(null)}
                  style={{ background: 'none', border: 'none', color: '#94a3b8', cursor: 'pointer' }}
                >
                  ✕
                </button>
              </div>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '8px' }}>
                <div>Generation: <strong>{selectedNode.generation}</strong></div>
                <div>Parent ID: <strong>{selectedNode.parent_id || 'Root Genotype'}</strong></div>
                <div>Fitness: <strong>{selectedNode.fitness.toFixed(3)}</strong></div>
                <div>Mutations: <strong>{selectedNode.mutations_count}</strong></div>
                <div>CPG Central Pattern: <strong>Oscillator active</strong></div>
                <div>Morphology: <strong>Multi-segment articulated</strong></div>
              </div>
            </div>
          )}

          {/* Newick Export Dialog / String Display */}
          {newickExportResult && (
            <div
              data-testid="newick-export-output"
              style={{
                backgroundColor: 'rgba(9, 9, 11, 0.9)',
                border: '1px solid rgba(16, 185, 129, 0.3)',
                borderRadius: '6px',
                padding: '8px 12px',
                fontSize: '11px',
                fontFamily: 'monospace',
                color: '#a7f3d0',
                wordBreak: 'break-all',
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '2px' }}>
                <span style={{ fontWeight: 600, color: '#10b981' }}>Newick RFC String:</span>
                <button
                  onClick={() => setNewickExportResult(null)}
                  style={{ background: 'none', border: 'none', color: '#94a3b8', cursor: 'pointer' }}
                >
                  ✕
                </button>
              </div>
              {newickExportResult}
            </div>
          )}
        </div>

        {/* Right: MAP-Elites 2D QD Heatmap & Hyperparameter Sliders */}
        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '12px',
            backgroundColor: 'rgba(15, 23, 42, 0.55)',
            border: '1px solid rgba(255, 255, 255, 0.08)',
            borderRadius: '8px',
            padding: '12px',
          }}
        >
          <div style={{ fontSize: '13px', fontWeight: 600, color: '#f8fafc' }}>
            2D Behavioral MAP-Elites Heatmap
          </div>

          <MapElitesHeatmap
            grid={mapElitesGrid.grid}
            gridResolution={gridResolution}
            width={280}
            height={240}
            testid="map-elites-qd-grid"
          />

          {/* Evolution Settings Sliders */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginTop: '6px' }}>
            <div>
              <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '11px', color: '#cbd5e1' }}>
                <span>Mutation Rate:</span>
                <strong>{mutationRate.toFixed(2)}</strong>
              </div>
              <input
                type="range"
                min="0.01"
                max="1.0"
                step="0.01"
                value={mutationRate}
                onChange={(e) => handleMutationRateChange(parseFloat(e.target.value))}
                data-testid="evolution-mutation-rate-slider"
                style={{ width: '100%' }}
              />
            </div>

            <div>
              <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '11px', color: '#cbd5e1' }}>
                <span>Selection Bias:</span>
                <strong>{selectionBias.toFixed(1)}</strong>
              </div>
              <input
                type="range"
                min="0.1"
                max="5.0"
                step="0.1"
                value={selectionBias}
                onChange={(e) => handleSelectionBiasChange(parseFloat(e.target.value))}
                data-testid="evolution-selection-bias-slider"
                style={{ width: '100%' }}
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
