import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { ResearchDashboard } from '../../src/components/ResearchDashboard/ResearchDashboard';

describe('Frontend Integration Test: Research Dashboard Suite (Milestone 4)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('initializes Research Dashboard with live IPC mock telemetry and switches all 5 panels cleanly', async () => {
    render(<ResearchDashboard />);

    // Verify initial telemetry load
    await waitFor(() => {
      expect(screen.getByTestId('research-dashboard-container')).toBeInTheDocument();
    });

    // Verify Tab 1: Telemetry & Phase Space
    expect(screen.getByTestId('telemetry-overview-panel')).toBeInTheDocument();
    expect(screen.getByTestId('biomass-timeseries-chart')).toBeInTheDocument();
    expect(screen.getByTestId('lotka-volterra-phase-space')).toBeInTheDocument();

    // Verify Tab 2: Trophic Dynamics
    fireEvent.click(screen.getByTestId('dashboard-tab-trophic'));
    await waitFor(() => {
      expect(screen.getByTestId('trophic-dynamics-panel')).toBeInTheDocument();
      expect(screen.getByTestId('energy-flux-sankey')).toBeInTheDocument();
    });

    // Verify Tab 3: Evolution & Lineage
    fireEvent.click(screen.getByTestId('dashboard-tab-evolution'));
    await waitFor(() => {
      expect(screen.getByTestId('evolution-lineage-panel')).toBeInTheDocument();
      expect(screen.getByTestId('phylogenetic-dag-container')).toBeInTheDocument();
      expect(screen.getByTestId('map-elites-qd-grid')).toBeInTheDocument();
    });

    // Verify Tab 4: Causal Interventions
    fireEvent.click(screen.getByTestId('dashboard-tab-interventions'));
    await waitFor(() => {
      expect(screen.getByTestId('causal-intervention-panel')).toBeInTheDocument();
      expect(screen.getByTestId('causal-spatial-canvas')).toBeInTheDocument();
    });

    // Verify Tab 5: Checkpoint Studio
    fireEvent.click(screen.getByTestId('dashboard-tab-checkpoints'));
    await waitFor(() => {
      expect(screen.getByTestId('checkpoint-studio-panel')).toBeInTheDocument();
      expect(screen.getByTestId('causal-branch-tree-svg')).toBeInTheDocument();
    });
  });

  it('performs end-to-end dataset export pipeline calling export_scientific_dataset IPC', async () => {
    render(<ResearchDashboard />);

    // Open export modal
    fireEvent.click(screen.getByTestId('open-export-dialog-btn'));
    await waitFor(() => {
      expect(screen.getByTestId('data-export-dialog-modal')).toBeInTheDocument();
    });

    // Trigger export
    fireEvent.click(screen.getByTestId('execute-export-dataset-btn'));

    await waitFor(() => {
      expect(screen.getByTestId('export-preview-content')).toBeInTheDocument();
    });

    expect(screen.getByTestId('export-preview-content')).toHaveTextContent(/schemaVersion/i);
  });
});
