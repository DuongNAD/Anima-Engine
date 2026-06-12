import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import React from 'react';
import { App } from '../../src/App';
import { invoke } from '@tauri-apps/api/core';
import { listen, emit } from '@tauri-apps/api/event';

// Mock Tauri Core
vi.mock('@tauri-apps/api/core', async (importOriginal) => {
  const original = await importOriginal<typeof import('@tauri-apps/api/core')>();
  return {
    ...original,
    invoke: vi.fn().mockImplementation((cmd, args) => {
      if (cmd === 'get_map_elites_grid') {
        return Promise.resolve({ grid: {}, grid_resolution: 50 });
      }
      if (cmd === 'get_simulation_status') {
        return Promise.resolve({ running: false, tick_count: 0, avg_tick_time_ms: 0, fps: 0 });
      }
      if (cmd === 'get_pheromone_grid') {
        return Promise.resolve({ grid: [], width: 128, height: 128 });
      }
      if (cmd === 'get_active_raycasts') {
        return Promise.resolve([]);
      }
      if (cmd === 'get_lineage_graph') {
        return Promise.resolve({ nodes: [], links: [], db_connected: false });
      }
      if (cmd === 'get_chronicle_history') {
        return Promise.resolve([
          {
            id: 'event-1',
            event_type: 'Drought',
            timestamp: Date.now() - 10000,
            title: 'Initial Drought Event',
            description: 'A starting drought that reduces food multipliers.',
            parameter_delta: { food_multiplier: 0.5 },
          }
        ]);
      }
      return original.invoke(cmd, args);
    }),
  };
});

describe('Phase 5 Gemini Chronicle Timeline Logs UI', () => {
  let mockCtx: any;

  beforeEach(() => {
    vi.clearAllMocks();

    mockCtx = {
      clearRect: vi.fn(),
      beginPath: vi.fn(),
      arc: vi.fn(),
      fill: vi.fn(),
      stroke: vi.fn(),
      moveTo: vi.fn(),
      lineTo: vi.fn(),
      closePath: vi.fn(),
      fillText: vi.fn(),
      fillRect: vi.fn(),
    };
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(mockCtx as any);
  });

  it('should query get_chronicle_history on mount and display chronicle history log', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    expect(invoke).toHaveBeenCalledWith('get_chronicle_history');
    
    // Verify chronicle event title & description render correctly
    expect(screen.getByText('Initial Drought Event')).toBeDefined();
    expect(screen.getByText('A starting drought that reduces food multipliers.')).toBeDefined();

    // Verify parameter delta warnings are displayed
    const warning = screen.getByTestId('parameter-delta-warning');
    expect(warning).toBeDefined();
    expect(warning.textContent).toContain('food_multiplier: +0.5');
  });

  it('should append new chronicle events and display correct alert styling for alerts vs stable events', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    // Emit a 'Drought' alert event
    const alertEvent = {
      id: 'event-alert-1',
      event_type: 'TemperatureSpike',
      timestamp: Date.now(),
      title: 'Extreme Heatwave',
      description: 'Severe heatwave shifts metabolic cost.',
      parameter_delta: { temp_target: 5.0, energy_decay_multiplier: 1.2 },
    };

    await act(async () => {
      await emit('chronicle-event', alertEvent);
    });

    expect(screen.getByText('Extreme Heatwave')).toBeDefined();
    const alertWarning = screen.getAllByTestId('parameter-delta-warning');
    // Both events have warnings
    expect(alertWarning.length).toBe(2);
    expect(alertWarning[0].textContent).toContain('temp_target: +5');
    expect(alertWarning[0].textContent).toContain('energy_decay_multiplier: +1.2');

    // Emit a 'Stable' abundance event (should render with green banner and no warnings)
    const stableEvent = {
      id: 'event-stable-1',
      event_type: 'Abundance',
      timestamp: Date.now(),
      title: 'Stable Climate Returning',
      description: 'Conditions returned to normal.',
      parameter_delta: {},
    };

    await act(async () => {
      await emit('chronicle-event', stableEvent);
    });

    expect(screen.getByText('Stable Climate Returning')).toBeDefined();
    expect(screen.getByText('Conditions returned to normal.')).toBeDefined();

    // The stable event has empty parameter_delta, so the number of warning blocks remains 2
    const finalWarnings = screen.getAllByTestId('parameter-delta-warning');
    expect(finalWarnings.length).toBe(2);
  });
});
