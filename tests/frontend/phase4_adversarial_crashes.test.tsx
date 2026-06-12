import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import { App } from '../../src/App';
import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';

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

describe('Phase 4 Front-end - Adversarial Crash & Vulnerability Tests', () => {
  const makeCustomMock = (overrides: Record<string, any>) => {
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

  beforeEach(() => {
    vi.clearAllMocks();
    makeCustomMock({}); // Ensure each test starts with clean safe mocks
  });

  it('CRASH SCENARIO 1: Null item inside lineage nodes array does not crash SVG rendering', async () => {
    makeCustomMock({
      get_lineage_graph: {
        nodes: [null],
        links: [],
        db_connected: true
      }
    });

    let error: Error | null = null;
    try {
      render(<App />);
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 50));
      });
    } catch (e: any) {
      error = e;
    }

    expect(error).toBeNull();
    expect(screen.getByTestId("phase4-panel")).toBeDefined();
  });

  it('CRASH SCENARIO 2: Null item inside lineage links array does not crash SVG rendering', async () => {
    makeCustomMock({
      get_lineage_graph: {
        nodes: [{ id: "A-0", generation: 0, parent_id: null, fitness: 0.5, mutations_count: 0 }],
        links: [null],
        db_connected: true
      }
    });

    let error: Error | null = null;
    try {
      render(<App />);
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 50));
      });
    } catch (e: any) {
      error = e;
    }

    expect(error).toBeNull();
    expect(screen.getByTestId("phase4-panel")).toBeDefined();
  });

  it('CRASH SCENARIO 3: Invalid simulation-tick event payload does not cause unhandled exception', async () => {
    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    let error: Error | null = null;
    try {
      await act(async () => {
        await emit('simulation-tick', [null]);
      });
    } catch (e: any) {
      error = e;
    }

    expect(error).toBeNull();
    expect(screen.getByTestId("phase4-panel")).toBeDefined();
  });

  it('CRASH SCENARIO 4: Null combat event payload does not crash log rendering', async () => {
    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    let error: Error | null = null;
    try {
      await act(async () => {
        await emit('combat-event', null);
      });
    } catch (e: any) {
      error = e;
    }

    expect(error).toBeNull();
    expect(screen.getByTestId("phase3-panel")).toBeDefined();
  });

  it('CRASH SCENARIO 5: Null migration event payload does not crash migration log rendering', async () => {
    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    let error: Error | null = null;
    try {
      await act(async () => {
        await emit('migration-event', null);
      });
    } catch (e: any) {
      error = e;
    }

    expect(error).toBeNull();
    expect(screen.getByTestId("migration-panel")).toBeDefined();
  });

  it('CRASH SCENARIO 6: Pheromone update event payload missing grid array does not crash rendering', async () => {
    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    let error: Error | null = null;
    try {
      await act(async () => {
        await emit('pheromone-update', { width: 10, height: 10 } as any);
      });
    } catch (e: any) {
      error = e;
    }

    expect(error).toBeNull();
    expect(screen.getByTestId("phase3-panel")).toBeDefined();
  });

  it('CRASH SCENARIO 7: Raycast update event payload with null item does not crash rendering', async () => {
    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    let error: Error | null = null;
    try {
      await act(async () => {
        await emit('raycast-update', [null]);
      });
    } catch (e: any) {
      error = e;
    }

    expect(error).toBeNull();
    expect(screen.getByTestId("phase3-panel")).toBeDefined();
  });
});
