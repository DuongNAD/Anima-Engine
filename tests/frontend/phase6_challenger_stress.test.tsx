import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act, fireEvent } from '@testing-library/react';
import React from 'react';
import * as PIXI from 'pixi.js';
import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';
import {
  mockEnvironmentalState,
  mockSimulationTickPayload
} from '../mocks/mock_ipc_payloads';

// Mock pixi.js exactly like phase 6 test so rendering works in jsdom
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

// Mock `@tauri-apps/api/core`
vi.mock('@tauri-apps/api/core', async (importOriginal) => {
  const original = await importOriginal<typeof import('@tauri-apps/api/core')>();
  return {
    ...original,
    invoke: vi.fn(),
  };
});

import { App } from '../../src/App';

describe('Phase 6 Challenger Stress and Edge Case Tests', () => {
  const setupDefaultInvokeMock = (overrides: Record<string, any> = {}) => {
    vi.mocked(invoke).mockImplementation(async (cmd, args) => {
      if (overrides[cmd] !== undefined) {
        if (overrides[cmd] instanceof Error) {
          throw overrides[cmd];
        }
        return overrides[cmd];
      }
      if (cmd === 'get_environmental_elements') {
        return mockEnvironmentalState;
      }
      if (cmd === 'save_simulation_state' || cmd === 'load_simulation_state') {
        return true;
      }
      if (cmd === 'get_simulation_status') {
        return {
          running: false,
          tick_count: 0,
          avg_tick_time_ms: 0,
          fps: 0,
        };
      }
      if (cmd === 'get_map_elites_grid') {
        return {
          grid: {},
          grid_resolution: 50,
        };
      }
      if (cmd === 'get_active_raycasts') {
        return [];
      }
      if (cmd === 'get_pheromone_grid') {
        return {
          grid: [],
          width: 0,
          height: 0
        };
      }
      return {};
    });
  };

  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(performance, 'now').mockReturnValue(0);
    setupDefaultInvokeMock();
  });

  // --- ZOOM UPPER LIMIT VERIFICATION ---
  it('Verify Zoom Upper Limit Clamping at 10.0 (Zoom In does NOT scale infinitely)', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const zoomInBtn = screen.getByTestId('zoom-in-button');
    expect(zoomInBtn).toBeDefined();

    mockGraphicsMethods.drawCircle.mockClear();

    // Click Zoom In 150 times.
    // If zoom is capped at 10.0, the final zoom must be 10.0, not 1.0 + 15.0 = 16.0.
    for (let i = 0; i < 150; i++) {
      await act(async () => {
        fireEvent.click(zoomInBtn);
      });
    }

    // Zoom = 10.0. Concentric outer radius = (30 - 2.87677) * 10 = 271.232.
    const zoomCalls = mockGraphicsMethods.drawCircle.mock.calls;
    const finalCalls = zoomCalls.slice(-8);
    const zoomedLake = finalCalls.find(c => Math.abs(c[2] - 271.2) < 0.1);
    expect(zoomedLake).toBeDefined();
    expect(zoomedLake[0]).toBeCloseTo(500);
    expect(zoomedLake[1]).toBeCloseTo(500);
  }, 60000);

  // --- ZOOM LOWER LIMIT VERIFICATION ---
  it('Verify Zoom Lower Limit Clamping at 0.1', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const zoomOutBtn = screen.getByTestId('zoom-out-button');
    expect(zoomOutBtn).toBeDefined();

    mockGraphicsMethods.drawCircle.mockClear();

    // Click Zoom Out 150 times.
    // If zoom is capped at 0.1, the final zoom must be 0.1, not negative or 0.
    for (let i = 0; i < 150; i++) {
      await act(async () => {
        fireEvent.click(zoomOutBtn);
      });
    }

    // Zoom = 0.1. Concentric outer radius = (30 - 2.87677) * 0.1 = 2.7123.
    const zoomCalls = mockGraphicsMethods.drawCircle.mock.calls;
    const finalCalls = zoomCalls.slice(-8);
    const zoomedLake = finalCalls.find(c => Math.abs(c[2] - 2.71) < 0.1);
    expect(zoomedLake).toBeDefined();
    expect(zoomedLake[0]).toBeCloseTo(5);
    expect(zoomedLake[1]).toBeCloseTo(5);
  }, 60000);

  // --- PAN LIMITS AND COORDINATE TRANSFORMATION STRESS ---
  it('Verify Pan Coordinates can accept extreme inputs without crashing', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const panRightBtn = screen.getByTestId('pan-right-button');
    const panDownBtn = screen.getByTestId('pan-down-button');

    mockGraphicsMethods.drawCircle.mockClear();

    // Click Pan Right and Pan Down 1000 times (total pan = 10000)
    for (let i = 0; i < 1000; i++) {
      // We don't act/await each click to prevent timing out the test, we just call the click handler directly
      // fireEvent.click triggers onClick synchronously
      fireEvent.click(panRightBtn);
      fireEvent.click(panDownBtn);
    }

    // Force re-render/wait
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });

    // Zoom = 1.0, Pan.x = 10000, Pan.y = 10000.
    // Concentric outer radius = 30 - 2.87677 = 27.123.
    const panCalls = mockGraphicsMethods.drawCircle.mock.calls;
    const finalCalls = panCalls.slice(-8);
    const pannedLake = finalCalls.find(c => Math.abs(c[2] - 27.12) < 0.1);
    expect(pannedLake).toBeDefined();
    expect(pannedLake[0]).toBeCloseTo(10050);
    expect(pannedLake[1]).toBeCloseTo(10050);
  }, 60000);

  // --- PAYLOAD PARSING CRASHES: OBJ SEGMENTS ---
  it('Verify gracefulness on non-array segments object in simulation-tick', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    let tickError: Error | null = null;
    try {
      await act(async () => {
        await emit('simulation-tick', { segments: {} });
      });
    } catch (e: any) {
      tickError = e;
    }
    expect(tickError).toBeNull();
  });

  // --- PAYLOAD PARSING CRASHES: NULL IN HEAD_DIRECTIONS ---
  it('Verify gracefulness on null item inside head_directions array in simulation-tick', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const corruptedPayload = {
      segments: [],
      environmental_state: { elements: [] },
      head_directions: [
        null // null item instead of object
      ]
    };

    let tickError: Error | null = null;
    try {
      await act(async () => {
        await emit('simulation-tick', corruptedPayload);
      });
    } catch (e: any) {
      tickError = e;
    }
    expect(tickError).toBeNull();
  });

  // --- PAYLOAD PARSING CRASHES: STRING SEGMENTS ---
  it('Verify gracefulness on segments being a string in simulation-tick', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    let tickError: Error | null = null;
    try {
      await act(async () => {
        await emit('simulation-tick', { segments: "corrupted_string" });
      });
    } catch (e: any) {
      tickError = e;
    }
    expect(tickError).toBeNull();
  });
});
