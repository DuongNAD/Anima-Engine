import React from 'react';
import { MapElitesGridState } from '../types';

interface EvolutionPanelProps {
  mutationRate: number;
  selectionBias: number;
  evolutionRunning: boolean;
  mapElitesGrid: MapElitesGridState;
  handleMutationRateChange: (rate: number) => void;
  handleSelectionBiasChange: (bias: number) => void;
  handleToggleEvolution: () => void;
}

export const EvolutionPanel: React.FC<EvolutionPanelProps> = ({
  mutationRate,
  selectionBias,
  evolutionRunning,
  mapElitesGrid,
  handleMutationRateChange,
  handleSelectionBiasChange,
  handleToggleEvolution,
}) => {
  const renderGrid = () => {
    const size = 10;
    const cells = [];
    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        const key = `${x * 5},${y * 5}`;
        const elite = mapElitesGrid.grid[key];
        const color = elite ? `rgba(255, 255, 255, ${0.1 + elite.fitness * 0.5})` : 'rgba(255, 255, 255, 0.03)';
        cells.push(
          <div
            key={key}
            data-testid={`grid-cell-${key}`}
            style={{
              width: '20px',
              height: '20px',
              backgroundColor: color,
              border: elite ? '1px solid rgba(255, 255, 255, 0.3)' : '1px solid rgba(255, 255, 255, 0.08)',
              display: 'inline-block',
            }}
            title={elite ? `Fitness: ${elite.fitness.toFixed(2)}` : 'Empty'}
          />
        );
      }
    }
    return (
      <div
        style={{ display: 'grid', gridTemplateColumns: `repeat(${size}, 22px)`, gap: '2px' }}
        data-testid="map-elites-grid"
      >
        {cells}
      </div>
    );
  };

  return (
    <div
      className="hud-panel"
      style={{
        padding: '18px',
        marginTop: '20px',
      }}
    >
      <h2
        className="hud-title-teal"
        style={{
          margin: '0 0 15px 0',
          fontSize: '18px',
          borderBottom: '1px solid rgba(255, 255, 255, 0.15)',
          paddingBottom: '8px',
        }}
      >
        MAP-Elites Evolutionary Archive
      </h2>
      <div style={{ display: 'flex', gap: '20px', flexWrap: 'wrap' }}>
        <div>
          <h3 style={{ fontSize: '16px', margin: '0 0 12px 0', color: 'rgba(255, 255, 255, 0.9)' }}>Evolution Controls</h3>
          <div style={{ marginBottom: '10px' }}>
            <label style={{ fontWeight: '500', color: 'rgba(255, 255, 255, 0.7)' }}>Mutation Rate: <span style={{ color: 'rgba(255, 255, 255, 0.95)', fontWeight: 'bold' }}>{mutationRate.toFixed(2)}</span></label>
            <br />
            <input
              type="range"
              min="0"
              max="1"
              step="0.01"
              value={mutationRate}
              onChange={(e) => handleMutationRateChange(parseFloat(e.target.value))}
              data-testid="mutation-rate-slider"
              style={{ accentColor: 'rgba(255, 255, 255, 0.8)', width: '150px' }}
            />
          </div>
          <div style={{ marginBottom: '15px' }}>
            <label style={{ fontWeight: '500', color: 'rgba(255, 255, 255, 0.7)' }}>Selection Bias: <span style={{ color: 'rgba(255, 255, 255, 0.95)', fontWeight: 'bold' }}>{selectionBias.toFixed(1)}</span></label>
            <br />
            <input
              type="range"
              min="0.1"
              max="5"
              step="0.1"
              value={selectionBias}
              onChange={(e) => handleSelectionBiasChange(parseFloat(e.target.value))}
              data-testid="selection-bias-slider"
              style={{ accentColor: 'rgba(255, 255, 255, 0.8)', width: '150px' }}
            />
          </div>
          <button
            onClick={handleToggleEvolution}
            data-testid="toggle-evolution-button"
            className="hud-btn"
            style={{
              padding: '8px 16px',
              borderColor: evolutionRunning ? 'rgba(255, 255, 255, 0.4)' : 'rgba(255, 255, 255, 0.15)',
              color: evolutionRunning ? '#f4f4f5' : '#a1a1aa',
              backgroundColor: evolutionRunning ? 'rgba(255, 255, 255, 0.08)' : 'rgba(255, 255, 255, 0.03)',
              boxShadow: 'none'
            }}
          >
            {evolutionRunning ? 'Stop Evolution' : 'Start Evolution'}
          </button>
        </div>
        <div>
          <h3 style={{ fontSize: '16px', margin: '0 0 12px 0', color: 'rgba(255, 255, 255, 0.9)' }}>Archive Grid (10x10 representation)</h3>
          {renderGrid()}
        </div>
      </div>
    </div>
  );
};
