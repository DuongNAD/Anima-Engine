import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import { App } from '../../src/App';
import { invoke } from '@tauri-apps/api/core';
import { listen, emit } from '@tauri-apps/api/event';
import {
  mockRaycastTelemetry,
  mockPheromoneGridState,
  mockCombatEvent,
  mockSegmentStates,
  RaycastTelemetry,
  PheromoneGridState,
  CombatEvent
} from '../mocks/mock_ipc_payloads';

// Mock pixi.js
const mockGraphicsMethods = {
  clear: vi.fn().mockReturnThis(),
  beginFill: vi.fn().mockReturnThis(),
  drawCircle: vi.fn().mockReturnThis(),
  drawPolygon: vi.fn().mockReturnThis(),
  endFill: vi.fn().mockReturnThis(),
  lineStyle: vi.fn().mockReturnThis(),
  moveTo: vi.fn().mockReturnThis(),
  lineTo: vi.fn().mockReturnThis(),
  drawRect: vi.fn().mockReturnThis(),
};

vi.mock('pixi.js', () => {
  return {
    Application: vi.fn().mockImplementation(() => ({
      init: vi.fn().mockResolvedValue(undefined),
      canvas: document.createElement('canvas'),
      stage: {
        addChild: vi.fn(),
        removeChild: vi.fn(),
      },
      renderer: {},
      ticker: {
        add: vi.fn(),
        remove: vi.fn(),
      },
      destroy: vi.fn(),
    })),
    Graphics: vi.fn().mockImplementation(() => mockGraphicsMethods),
    Container: vi.fn().mockImplementation(() => ({
      addChild: vi.fn(),
      removeChild: vi.fn(),
    })),
  };
});

vi.mock('@tauri-apps/api/core', async (importOriginal) => {
  const original = await importOriginal<typeof import('@tauri-apps/api/core')>();
  return {
    ...original,
    invoke: vi.fn().mockImplementation((cmd, args) => {
      if (cmd === 'get_map_elites_grid') {
        return Promise.resolve({
          grid: {},
          grid_resolution: 50
        });
      }
      if (cmd === 'get_simulation_status') {
        return Promise.resolve({
          running: false,
          tick_count: 0,
          avg_tick_time_ms: 0,
          fps: 0,
        });
      }
      if (cmd === 'get_pheromone_grid') {
        return Promise.resolve(mockPheromoneGridState);
      }
      if (cmd === 'get_active_raycasts') {
        return Promise.resolve(mockRaycastTelemetry);
      }
      return original.invoke(cmd, args);
    }),
  };
});

describe('Phase 3 Front-end UI & Canvas Rendering', () => {
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
      fillStyle: '',
      strokeStyle: '',
      lineWidth: 1,
      font: '',
      textAlign: '',
      textBaseline: '',
    };

    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(mockCtx as any);
  });

  it('should call get_pheromone_grid and get_active_raycasts on mount and display them', async () => {
    render(<App />);

    // Wait for mock IPC calls to resolve
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    expect(invoke).toHaveBeenCalledWith('get_pheromone_grid');
    expect(invoke).toHaveBeenCalledWith('get_active_raycasts');

    // Verify UI panel displays the state
    const panel = screen.getByTestId('phase3-panel');
    expect(panel).toBeDefined();
    expect(screen.getByText('Phase 3: Socialization & Emergent Behaviors')).toBeDefined();
    expect(screen.getByText('Grid Size: 128x128')).toBeDefined();
    expect(screen.getByText('Active Raycasts: 2')).toBeDefined();
  });

  it('should listen to pheromone-update, raycast-update, and combat-event Tauri events', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    // 1. Pheromone grid update
    const updatedGrid: PheromoneGridState = {
      grid: new Array(128 * 128).fill(0.0),
      width: 128,
      height: 128
    };
    updatedGrid.grid[10] = 0.9; // add a pheromone spot

    await act(async () => {
      await emit('pheromone-update', updatedGrid);
    });

    expect(screen.getByText('Active Pheromone Sites: 1')).toBeDefined();

    // 2. Raycast update
    const updatedRaycasts: RaycastTelemetry[] = [
      {
        origin: [0, 0, 0],
        direction: [1, 0, 0],
        hit_distance: 2.0,
        hit_entity_type: 'Predator',
        agent_id: 3
      }
    ];

    await act(async () => {
      await emit('raycast-update', updatedRaycasts);
    });

    expect(screen.getByText('Active Raycasts: 1')).toBeDefined();
    expect(screen.getByText('Agent #3 detected Predator at 2.0m')).toBeDefined();

    // 3. Combat event log
    const updatedCombat: CombatEvent = {
      predator_id: 5,
      prey_id: 6,
      damage: 25.5,
      energy_transferred: 20.0
    };

    await act(async () => {
      await emit('combat-event', updatedCombat);
    });

    expect(screen.getByText('Total Events: 1')).toBeDefined();
    expect(screen.getByText('Predator #5 damaged Prey #6 (-25.5 energy)')).toBeDefined();
  });

  it('should invoke canvas drawing methods for pheromone heatmap, sensor beams, and predator/prey geometries', async () => {
    render(<App />);

    // Wait for async initPixi to complete and register event listeners
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    // Seed canvas segments data via simulation-tick to trigger agent rendering
    await act(async () => {
      await emit('simulation-tick', mockSegmentStates);
    });

    // Wait for requestAnimationFrame loop to render
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 100));
    });

    // Verify pheromone drawing calls (which calls drawRect)
    expect(mockGraphicsMethods.drawRect).toHaveBeenCalled();

    // Verify sensor beam drawing (which uses moveTo/lineTo)
    expect(mockGraphicsMethods.moveTo).toHaveBeenCalled();
    expect(mockGraphicsMethods.lineTo).toHaveBeenCalled();

    // Verify predator drawing (which uses drawPolygon)
    expect(mockGraphicsMethods.drawPolygon).toHaveBeenCalled();
  });
});
