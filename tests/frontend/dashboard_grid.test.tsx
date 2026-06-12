import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { App } from '../../src/App';
import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';

vi.mock('@tauri-apps/api/core', async (importOriginal) => {
  const original = await importOriginal<typeof import('@tauri-apps/api/core')>();
  return {
    ...original,
    invoke: vi.fn().mockImplementation((cmd, args) => {
      if (cmd === 'get_map_elites_grid') {
        return Promise.resolve({
          grid: {
            "10,20": { fitness: 0.85, features: [0.2, 0.4] },
            "30,40": { fitness: 0.92, features: [0.6, 0.8] }
          },
          grid_resolution: 50
        });
      }
      return original.invoke(cmd, args);
    }),
  };
});

describe('MAP-Elites Grid Dashboard UI', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should render the evolution controls and the grid', async () => {
    render(<App />);

    // Wait for initial get_map_elites_grid to resolve and state to update
    const title = await screen.findByText('MAP-Elites Evolutionary Archive');
    expect(title).toBeDefined();

    // Check sliders are rendered
    const mutationSlider = screen.getByTestId('mutation-rate-slider');
    const selectionSlider = screen.getByTestId('selection-bias-slider');
    expect(mutationSlider).toBeDefined();
    expect(selectionSlider).toBeDefined();

    // Check toggle evolution button
    const toggleBtn = screen.getByTestId('toggle-evolution-button');
    expect(toggleBtn).toBeDefined();
    expect(toggleBtn.textContent).toBe('Start Evolution');

    // Check the grid container is rendered
    const gridContainer = screen.getByTestId('map-elites-grid');
    expect(gridContainer).toBeDefined();

    // Check that occupied cells are styled correctly
    const cell1 = screen.getByTestId('grid-cell-10,20');
    expect(cell1.style.backgroundColor).toContain('rgba(236, 72, 153, 0.85)');

    const cell2 = screen.getByTestId('grid-cell-30,40');
    expect(cell2.style.backgroundColor).toContain('rgba(236, 72, 153, 0.92)');
  });

  it('should dispatch Tauri update_evolution_settings when mutation rate slider is changed', async () => {
    render(<App />);

    const mutationSlider = await screen.findByTestId('mutation-rate-slider');
    
    // Simulate slider change
    await act(async () => {
      fireEvent.change(mutationSlider, { target: { value: '0.35' } });
    });

    // Verify it invoked update_evolution_settings
    expect(invoke).toHaveBeenCalledWith('update_evolution_settings', {
      settings: {
        mutation_rate: 0.35,
        selection_bias: 1.5,
        grid_resolution: 50,
      }
    });
  });

  it('should dispatch Tauri update_evolution_settings when selection bias slider is changed', async () => {
    render(<App />);

    const selectionSlider = await screen.findByTestId('selection-bias-slider');

    // Simulate slider change
    await act(async () => {
      fireEvent.change(selectionSlider, { target: { value: '2.5' } });
    });

    // Verify it invoked update_evolution_settings
    expect(invoke).toHaveBeenCalledWith('update_evolution_settings', {
      settings: {
        mutation_rate: 0.15,
        selection_bias: 2.5,
        grid_resolution: 50,
      }
    });
  });

  it('should dispatch toggle_evolution when evolution toggle button is clicked', async () => {
    render(<App />);

    const toggleBtn = await screen.findByTestId('toggle-evolution-button');
    expect(toggleBtn.textContent).toBe('Start Evolution');

    // Click button
    await act(async () => {
      fireEvent.click(toggleBtn);
    });

    // Verify it invoked toggle_evolution
    expect(invoke).toHaveBeenCalledWith('toggle_evolution');

    // Wait for UI to update
    await screen.findByText('Stop Evolution');
  });

  it('should dynamically update grid when map-elites-update event is received', async () => {
    render(<App />);

    // Wait for initial render
    await screen.findByTestId('map-elites-grid');

    // Wait a brief moment for the event listener to be fully registered in App.tsx
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    // Emit event with new grid
    const updatedGrid = {
      grid: {
        "10,20": { fitness: 0.85, features: [0.2, 0.4] },
        "30,40": { fitness: 0.92, features: [0.6, 0.8] },
        "0,0": { fitness: 0.75, features: [0.0, 0.0] }
      },
      grid_resolution: 50
    };

    await act(async () => {
      await emit('map-elites-update', updatedGrid);
    });

    // Verify new cell is rendered with pink fitness color
    const cellNew = await screen.findByTitle('Fitness: 0.75');
    expect(cellNew).toBeDefined();
    expect(cellNew.style.backgroundColor).toContain('rgba(236, 72, 153, 0.75)');
  });
});
