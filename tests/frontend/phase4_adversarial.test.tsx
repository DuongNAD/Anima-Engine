import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act, fireEvent } from '@testing-library/react';
import { App } from '../../src/App';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', async (importOriginal) => {
  const original = await importOriginal<typeof import('@tauri-apps/api/core')>();
  return {
    ...original,
    invoke: vi.fn().mockImplementation((cmd) => {
      if (cmd === 'get_lineage_graph') {
        return Promise.resolve({ nodes: [], links: [], db_connected: false });
      }
      if (cmd === 'get_chronicle_history') {
        return Promise.resolve([]);
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
        return Promise.resolve({ grid: [], width: 128, height: 128 });
      }
      if (cmd === 'get_active_raycasts') {
        return Promise.resolve([]);
      }
      if (cmd === 'get_map_elites_grid') {
        return Promise.resolve({ grid: {}, grid_resolution: 50 });
      }
      return Promise.resolve({});
    }),
  };
});

describe('Phase 4 Front-end UI - Adversarial Stress Tests', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const makeSafeMock = (overrides: Record<string, any>) => {
    vi.mocked(invoke).mockImplementation((cmd) => {
      if (overrides[cmd] !== undefined) {
        if (overrides[cmd] instanceof Error) {
          return Promise.reject(overrides[cmd]);
        }
        return Promise.resolve(overrides[cmd]);
      }
      if (cmd === 'get_lineage_graph') {
        return Promise.resolve({ nodes: [], links: [], db_connected: false });
      }
      if (cmd === 'get_chronicle_history') {
        return Promise.resolve([]);
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
        return Promise.resolve({ grid: [], width: 128, height: 128 });
      }
      if (cmd === 'get_active_raycasts') {
        return Promise.resolve([]);
      }
      if (cmd === 'get_map_elites_grid') {
        return Promise.resolve({ grid: {}, grid_resolution: 50 });
      }
      return Promise.resolve({});
    });
  };

  it('STRESS 1: Handles extremely large lineage graphs without crashing', async () => {
    const massiveNodes = Array.from({ length: 500 }, (_, i) => ({
      id: `A-${i}`,
      generation: Math.floor(i / 10),
      parent_id: i > 0 ? `A-${i - 1}` : null,
      fitness: Math.random(),
      mutations_count: Math.floor(Math.random() * 5),
    }));

    const massiveLinks = Array.from({ length: 499 }, (_, i) => ({
      source: `A-${i}`,
      target: `A-${i + 1}`,
    }));

    makeSafeMock({
      get_lineage_graph: {
        nodes: massiveNodes,
        links: massiveLinks,
        db_connected: true,
      }
    });

    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const container = screen.getByTestId('lineage-svg-container');
    expect(container).toBeDefined();

    expect(screen.getByTestId('lineage-node-A-0')).toBeDefined();
    expect(screen.getByTestId('lineage-node-A-499')).toBeDefined();
  });

  it('STRESS 2: Handles extremely long chronicle history lists', async () => {
    const massiveChronicles = Array.from({ length: 2000 }, (_, i) => ({
      id: `evt-${i}`,
      event_type: i % 2 === 0 ? ('Drought' as const) : ('Abundance' as const),
      timestamp: 1625097600000 + i * 1000,
      title: `Event ${i}`,
      description: `Description of event ${i}`,
      parameter_delta: {},
    }));

    makeSafeMock({
      get_chronicle_history: massiveChronicles
    });

    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    expect(screen.getByText('Event 0')).toBeDefined();
    expect(screen.getByText('Event 1999')).toBeDefined();
  });

  it('ERROR HANDLING 1: Handles IPC initial fetch rejection and missing properties gracefully without crashing', async () => {
    const consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    // Case A: Missing properties (lacks nodes or links arrays)
    makeSafeMock({
      get_lineage_graph: { db_connected: false }, // missing nodes and links
      get_chronicle_history: new Error('Chronicle file read timeout'),
    });

    const { unmount } = render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    expect(screen.getByText('No lineage data available')).toBeDefined();
    expect(screen.getByText('No chronicle events recorded')).toBeDefined();

    unmount();

    // Case B: IPC initial fetch rejection (throws/rejects Error)
    makeSafeMock({
      get_lineage_graph: new Error('Database connection failed'),
      get_chronicle_history: new Error('Chronicle file read timeout'),
    });

    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    // Check that we logged the errors to console.error
    expect(consoleErrorSpy).toHaveBeenCalledWith(
      'Failed to load lineage graph:',
      expect.any(Error)
    );
    expect(consoleErrorSpy).toHaveBeenCalledWith(
      'Failed to load chronicle history:',
      expect.any(Error)
    );

    // Verify UI fallback empty state messages are rendered and App did not crash
    expect(screen.getByText('No lineage data available')).toBeDefined();
    expect(screen.getByText('No chronicle events recorded')).toBeDefined();

    consoleErrorSpy.mockRestore();
  });

  it('INPUT HANDLING 1: Allows input of extreme ports and passes them to trigger_migration', async () => {
    makeSafeMock({});

    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const portInput = screen.getByLabelText('Port:') as HTMLInputElement;
    const triggerButton = screen.getByTestId('migration-trigger-button') as HTMLButtonElement;

    fireEvent.change(portInput, { target: { value: '999999' } });
    await act(async () => {
      fireEvent.click(triggerButton);
    });
    expect(invoke).toHaveBeenLastCalledWith('trigger_migration', { target_port: 999999 });

    fireEvent.change(portInput, { target: { value: '-80' } });
    await act(async () => {
      fireEvent.click(triggerButton);
    });
    expect(invoke).toHaveBeenLastCalledWith('trigger_migration', { target_port: -80 });

    fireEvent.change(portInput, { target: { value: 'invalid_port' } });
    await act(async () => {
      fireEvent.click(triggerButton);
    });
    expect(invoke).toHaveBeenLastCalledWith('trigger_migration', { target_port: 8081 });
  });
});
