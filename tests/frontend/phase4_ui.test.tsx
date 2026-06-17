import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act, fireEvent } from '@testing-library/react';
import { App } from '../../src/App';
import { invoke } from '@tauri-apps/api/core';
import { listen, emit } from '@tauri-apps/api/event';
import {
  mockLineageGraph,
  mockChronicleHistory,
  mockChronicleEvent,
  mockMigrationPayload
} from '../mocks/mock_ipc_payloads';

vi.mock('@tauri-apps/api/core', async (importOriginal) => {
  const original = await importOriginal<typeof import('@tauri-apps/api/core')>();
  return {
    ...original,
    invoke: vi.fn().mockImplementation((cmd, args) => {
      if (cmd === 'get_lineage_graph') {
        return Promise.resolve(mockLineageGraph);
      }
      if (cmd === 'get_chronicle_history') {
        return Promise.resolve(mockChronicleHistory);
      }
      if (cmd === 'get_simulation_status') {
        return Promise.resolve({
          running: true,
          tick_count: 100,
          avg_tick_time_ms: 1.5,
          fps: 60
        });
      }
      if (cmd === 'get_pheromone_grid') {
        return Promise.resolve({
          grid: [],
          width: 128,
          height: 128
        });
      }
      if (cmd === 'get_active_raycasts') {
        return Promise.resolve([]);
      }
      if (cmd === 'get_map_elites_grid') {
        return Promise.resolve({
          grid: {},
          grid_resolution: 50
        });
      }
      return original.invoke(cmd, args);
    }),
  };
});

describe('Phase 4 Front-end UI & Feature Verification', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // TIER 1: Assert rendering of components
  it('should render Lineage Graph SVG container/nodes, Chronicle Timeline Panel, and Migration Panel on mount', async () => {
    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    expect(screen.getByTestId('lineage-svg-container')).toBeDefined();
    expect(screen.getByTestId('chronicle-timeline-panel')).toBeDefined();
    expect(screen.getByTestId('migration-panel')).toBeDefined();
    
    // Check nodes rendering
    expect(screen.getByTestId('lineage-node-A-0')).toBeDefined();
    expect(screen.getByTestId('lineage-node-A-3')).toBeDefined();
    
    // Check titles
    expect(screen.getByText('Mother Nature Chronicle')).toBeDefined();
    expect(screen.getByText('Distributed Socket Migration')).toBeDefined();
  });

  // TIER 2: Assert rendering of empty states and Neo4j offline warning
  it('should render empty states and Neo4j offline warning banner', async () => {
    vi.mocked(invoke).mockImplementation((cmd, args) => {
      if (cmd === 'get_lineage_graph') {
        return Promise.resolve({
          nodes: [],
          links: [],
          db_connected: false
        });
      }
      if (cmd === 'get_chronicle_history') {
        return Promise.resolve([]);
      }
      if (cmd === 'get_simulation_status') {
        return Promise.resolve({
          running: false,
          tick_count: 0,
          avg_tick_time_ms: 0,
          fps: 0
        });
      }
      if (cmd === 'get_map_elites_grid') {
        return Promise.resolve({ grid: {}, grid_resolution: 50 });
      }
      if (cmd === 'get_pheromone_grid') {
        return Promise.resolve({ grid: [], width: 128, height: 128 });
      }
      if (cmd === 'get_active_raycasts') {
        return Promise.resolve([]);
      }
      return Promise.resolve({});
    });

    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    // Neo4j offline banner
    expect(screen.getByTestId('neo4j-offline-banner')).toBeDefined();
    expect(screen.getByText(/Neo4j Offline - Fallback In-Memory Tracker Active/i)).toBeDefined();

    // Empty state messages
    expect(screen.getByText('No lineage data available')).toBeDefined();
    expect(screen.getByText('No chronicle events recorded')).toBeDefined();
  });

  // TIER 3: Verify interactive controls enable/disable based on simulation state
  it('should disable migration trigger button when simulation is not running', async () => {
    // 1. Simulation not running -> button disabled
    vi.mocked(invoke).mockImplementation((cmd, args) => {
      if (cmd === 'get_simulation_status') {
        return Promise.resolve({
          running: false,
          tick_count: 0,
          avg_tick_time_ms: 0,
          fps: 0
        });
      }
      if (cmd === 'get_lineage_graph') {
        return Promise.resolve(mockLineageGraph);
      }
      if (cmd === 'get_chronicle_history') {
        return Promise.resolve(mockChronicleHistory);
      }
      if (cmd === 'get_map_elites_grid') {
        return Promise.resolve({ grid: {}, grid_resolution: 50 });
      }
      if (cmd === 'get_pheromone_grid') {
        return Promise.resolve({ grid: [], width: 128, height: 128 });
      }
      if (cmd === 'get_active_raycasts') {
        return Promise.resolve([]);
      }
      return Promise.resolve({});
    });

    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const button = screen.getByTestId('migration-trigger-button') as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  it('should enable migration trigger button when simulation is running', async () => {
    // 2. Simulation running -> button enabled
    vi.mocked(invoke).mockImplementation((cmd, args) => {
      if (cmd === 'get_simulation_status') {
        return Promise.resolve({
          running: true,
          tick_count: 100,
          avg_tick_time_ms: 1.5,
          fps: 60
        });
      }
      if (cmd === 'get_lineage_graph') {
        return Promise.resolve(mockLineageGraph);
      }
      if (cmd === 'get_chronicle_history') {
        return Promise.resolve(mockChronicleHistory);
      }
      if (cmd === 'get_map_elites_grid') {
        return Promise.resolve({ grid: {}, grid_resolution: 50 });
      }
      if (cmd === 'get_pheromone_grid') {
        return Promise.resolve({ grid: [], width: 128, height: 128 });
      }
      if (cmd === 'get_active_raycasts') {
        return Promise.resolve([]);
      }
      return Promise.resolve({});
    });

    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const buttonEnabled = screen.getByTestId('migration-trigger-button') as HTMLButtonElement;
    expect(buttonEnabled.disabled).toBe(false);
  });

  // TIER 4: Workflow simulation by emitting Tauri event streams
  it('should handle workflow simulation events for chronicles and migration', async () => {
    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    // Verify initial values
    expect(screen.getByText('Meta-AI: Temperature Spike detected')).toBeDefined();

    // Emit chronicle event
    await act(async () => {
      await emit('chronicle-event', mockChronicleEvent);
    });

    expect(screen.getByText(mockChronicleEvent.title)).toBeDefined();
    expect(screen.getByText(mockChronicleEvent.description)).toBeDefined();

    // Emit migration event
    await act(async () => {
      await emit('migration-event', mockMigrationPayload);
    });

    expect(screen.getByText(/Agent #42 outgoing/i)).toBeDefined();
    expect(screen.getByText(/8080 ➔ 8081/i)).toBeDefined();
  });

  it('should support configuring target port and pass it to trigger_migration IPC invoke call', async () => {
    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const portInput = screen.getByLabelText('Port:') as HTMLInputElement;
    expect(portInput).toBeDefined();
    expect(portInput.value).toBe('8081');

    fireEvent.change(portInput, { target: { value: '9000' } });
    expect(portInput.value).toBe('9000');

    const button = screen.getByTestId('migration-trigger-button') as HTMLButtonElement;
    expect(button.disabled).toBe(false);

    await act(async () => {
      fireEvent.click(button);
    });

    expect(invoke).toHaveBeenCalledWith('trigger_migration', { target_port: 9000 });
  });

  it('should color-code chronicle events based on event type and show the formatted time', async () => {
    vi.mocked(invoke).mockImplementation((cmd, args) => {
      if (cmd === 'get_chronicle_history') {
        return Promise.resolve([
          {
            id: 'evt-1',
            event_type: 'Drought',
            timestamp: 1625097600000,
            title: 'Severe Drought',
            description: 'Water sources are drying up',
            parameter_delta: {}
          },
          {
            id: 'evt-2',
            event_type: 'Abundance',
            timestamp: 1625097600000,
            title: 'Fruitful Season',
            description: 'Abundance of food',
            parameter_delta: {}
          }
        ]);
      }
      if (cmd === 'get_lineage_graph') {
        return Promise.resolve(mockLineageGraph);
      }
      if (cmd === 'get_simulation_status') {
        return Promise.resolve({ running: true, tick_count: 100, avg_tick_time_ms: 1.5, fps: 60 });
      }
      if (cmd === 'get_map_elites_grid') {
        return Promise.resolve({ grid: {}, grid_resolution: 50 });
      }
      if (cmd === 'get_pheromone_grid') {
        return Promise.resolve({ grid: [], width: 128, height: 128 });
      }
      if (cmd === 'get_active_raycasts') {
        return Promise.resolve([]);
      }
      return Promise.resolve({});
    });

    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const droughtTitle = screen.getByText('Severe Drought');
    const abundanceTitle = screen.getByText('Fruitful Season');

    const droughtContainer = droughtTitle.parentElement;
    const abundanceContainer = abundanceTitle.parentElement;

    expect(droughtContainer).toBeDefined();
    expect(abundanceContainer).toBeDefined();

    expect(droughtContainer?.style.backgroundColor).toBe('rgba(255, 255, 255, 0.02)');
    expect(droughtContainer?.style.borderLeft).toBe('3px solid rgb(255, 255, 255)');

    expect(abundanceContainer?.style.backgroundColor).toBe('rgba(255, 255, 255, 0.02)');
    expect(abundanceContainer?.style.borderLeft).toBe('3px solid rgba(255, 255, 255, 0.3)');

    const expectedTime = new Date(1625097600000).toLocaleTimeString();
    expect(screen.getAllByText(expectedTime).length).toBeGreaterThanOrEqual(1);
  });
});
