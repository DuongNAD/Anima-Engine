import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, act, fireEvent } from '@testing-library/react';
import React from 'react';
import * as PIXI from 'pixi.js';
import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';
import {
  mockEnvironmentalState,
  mockSimulationTickPayload
} from '../mocks/mock_ipc_payloads';

// Mock pixi.js exactly like phase 5 and 6 test so rendering works in jsdom
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

// Mock `@tauri-apps/api/core` to spy on `invoke` and provide custom implementations
vi.mock('@tauri-apps/api/core', async (importOriginal) => {
  const original = await importOriginal<typeof import('@tauri-apps/api/core')>();
  return {
    ...original,
    invoke: vi.fn(),
  };
});

import { App } from '../../src/App';
import PixiViewport from '../../src/PixiViewport';

describe('Phase 6 Front-end - Adversarial, Stress, and Edge Case Tests', () => {
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

  const runWithInterceptors = async (action: () => void) => {
    const originalUncaught = [...process.listeners('uncaughtException')];
    const originalUnhandled = [...process.listeners('unhandledRejection')];
    process.removeAllListeners('uncaughtException');
    process.removeAllListeners('unhandledRejection');

    let caughtError: Error | null = null;
    const onError = (err: any) => {
      caughtError = err;
    };
    process.on('uncaughtException', onError);
    process.on('unhandledRejection', onError);

    try {
      action();
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 50));
      });
    } finally {
      process.removeAllListeners('uncaughtException');
      process.removeAllListeners('unhandledRejection');
      originalUncaught.forEach(l => process.on('uncaughtException', l));
      originalUnhandled.forEach(l => process.on('unhandledRejection', l));
    }

    return caughtError;
  };

  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(performance, 'now').mockReturnValue(0);
    setupDefaultInvokeMock();
  });

  // --- ZOOM LIMITS & STRESS CASES ---
  it('Verify Zoom Lower Limit Clamping works and Zoom Out does not go below 0.1', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const zoomOutBtn = screen.getByTestId('zoom-out-button');
    expect(zoomOutBtn).toBeDefined();

    // Clear previous draw calls from initial rendering
    mockGraphicsMethods.drawCircle.mockClear();

    // Click Zoom Out 15 times to hit and stay at the minimum clamp (0.1)
    for (let i = 0; i < 15; i++) {
      await act(async () => {
        fireEvent.click(zoomOutBtn);
      });
    }

    // Zoom = 0.1. Verify environmental element coordinates are scaled to 0.1 zoom.
    // Concentric outer radius = (30 - 2.87677) * 0.1 = 2.7123.
    const zoomCalls = mockGraphicsMethods.drawCircle.mock.calls;
    const finalCalls = zoomCalls.slice(-8);
    const zoomedLake = finalCalls.find(c => Math.abs(c[2] - 2.71) < 0.1);
    expect(zoomedLake).toBeDefined();
    expect(zoomedLake[0]).toBeCloseTo(5);
    expect(zoomedLake[1]).toBeCloseTo(5);
  });

  it('Verify Zoom In is unbounded and can reach extremely high scale values', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const zoomInBtn = screen.getByTestId('zoom-in-button');
    expect(zoomInBtn).toBeDefined();

    // Clear previous draw calls from initial rendering
    mockGraphicsMethods.drawCircle.mockClear();

    // Click Zoom In 20 times. Zoom becomes 1.0 + 2.0 = 3.0.
    for (let i = 0; i < 20; i++) {
      await act(async () => {
        fireEvent.click(zoomInBtn);
      });
    }

    // Zoom = 3.0. Lake at (50, 50) radius 30 should be cx = 150, cy = 150, concentric outer radius = (30 - 2.87677) * 3 = 81.369.
    const zoomCalls = mockGraphicsMethods.drawCircle.mock.calls;
    const finalCalls = zoomCalls.slice(-8);
    const zoomedLake = finalCalls.find(c => Math.abs(c[2] - 81.37) < 0.1);
    expect(zoomedLake).toBeDefined();
    expect(zoomedLake[0]).toBeCloseTo(150);
    expect(zoomedLake[1]).toBeCloseTo(150);
  });

  // --- PAN LIMITS & STRESS CASES ---
  it('Verify Pan Coordinates can increase/decrease indefinitely (Unbounded Pan)', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const panRightBtn = screen.getByTestId('pan-right-button');
    const panDownBtn = screen.getByTestId('pan-down-button');

    // Clear previous draw calls from initial rendering
    mockGraphicsMethods.drawCircle.mockClear();

    // Pan right 50 times (pan.x becomes 500) and down 50 times (pan.y becomes 500)
    for (let i = 0; i < 50; i++) {
      await act(async () => {
        fireEvent.click(panRightBtn);
        fireEvent.click(panDownBtn);
      });
    }

    // Zoom = 1.0, Pan.x = 500, Pan.y = 500.
    // Concentric outer radius = 30 - 2.87677 = 27.123.
    const panCalls = mockGraphicsMethods.drawCircle.mock.calls;
    const finalCalls = panCalls.slice(-8);
    const pannedLake = finalCalls.find(c => Math.abs(c[2] - 27.12) < 0.1);
    expect(pannedLake).toBeDefined();
    expect(pannedLake[0]).toBeCloseTo(550);
    expect(pannedLake[1]).toBeCloseTo(550);
  });

  // --- TAURI COMMAND CALLING ERRORS ---
  it('Verify graceful handling of errors when save_simulation_state and load_simulation_state fail', async () => {
    // Override tauri command to throw error
    setupDefaultInvokeMock({
      save_simulation_state: new Error("DISK_WRITE_FAILURE"),
      load_simulation_state: new Error("FILE_NOT_FOUND")
    });

    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const filepathInput = screen.getByTestId('filepath-input');
    const saveButton = screen.getByTestId('save-state-button');
    const loadButton = screen.getByTestId('load-state-button');

    fireEvent.change(filepathInput, { target: { value: 'error_test.json' } });

    // Click Save State
    await act(async () => {
      fireEvent.click(saveButton);
    });
    // App should not crash. An error should be displayed in the UI.
    expect(screen.getByText(/DISK_WRITE_FAILURE/)).toBeDefined();

    // Click Load State
    await act(async () => {
      fireEvent.click(loadButton);
    });
    // Error should update to the load error
    expect(screen.getByText(/FILE_NOT_FOUND/)).toBeDefined();
  });

  it('Verify app does not crash if get_environmental_elements throws an error during mounting', async () => {
    setupDefaultInvokeMock({
      get_environmental_elements: new Error("DATABASE_OFFLINE")
    });

    let renderError: Error | null = null;
    try {
      render(<App />);
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 50));
      });
    } catch (e: any) {
      renderError = e;
    }

    // Assert render did not crash and handles the error silently
    expect(renderError).toBeNull();
    // Container should exist but display the "No environmental elements loaded" message
    expect(screen.getByText('No environmental elements loaded')).toBeDefined();
  });

  // --- CORRUPTED SIMULATION TICK PAYLOADS ---
  it('Verify that sending a null simulation-tick payload does not crash the App', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    let tickError: Error | null = null;
    try {
      await act(async () => {
        await emit('simulation-tick', null);
      });
    } catch (e: any) {
      tickError = e;
    }

    expect(tickError).toBeNull();
  });

  it('Verify that sending a non-array segments in simulation-tick payload does not crash the App', async () => {
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

  it('Verify that a missing direction array inside head_directions does not crash the telemetry rendering', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const corruptedPayload = {
      segments: [],
      environmental_state: { elements: [] },
      head_directions: [
        {
          agent_id: 1,
          direction: undefined // missing
        }
      ]
    };

    let error: any = null;
    try {
      await act(async () => {
        await emit('simulation-tick', corruptedPayload);
      });
    } catch (e) {
      error = e;
    }
    expect(error).toBeNull();
    expect(screen.getByTestId('head-direction-telemetry').textContent).toContain('Head Direction: N/A');
  });

  it('Verify that segment coordinates of type string do not crash the segment list rendering', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const corruptedPayload = {
      segments: [
        {
          agent_id: 1,
          segment_id: 0,
          parent_segment_id: null,
          x: 10.0,
          y: 1.5,
          z: -5.0,
          yaw: 0.1,
          pitch: 0.0,
          roll: 0.0,
          joint_anchor_x: "corrupted_string" as any, // Not a number
          joint_anchor_y: 0,
          joint_anchor_z: 0,
          joint_axis_x: 0,
          joint_axis_y: 0,
          joint_axis_z: 0,
          energy: 95.5,
          agent_type: 'predator'
        }
      ],
      environmental_state: { elements: [] },
      head_directions: []
    };

    let error: any = null;
    try {
      await act(async () => {
        await emit('simulation-tick', corruptedPayload);
      });
    } catch (e) {
      error = e;
    }
    expect(error).toBeNull();
  });

  it('Verify that a non-array response for get_active_raycasts does not crash the viewport rendering', async () => {
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    const error = await runWithInterceptors(() => {
      render(<PixiViewport raycasts={{} as any} />);
    });

    expect(error).toBeNull();

    consoleSpy.mockRestore();
  });

  it('Verify that emitting a Phase 6 simulation-tick event payload passed as segments prop does not crash PixiViewport rendering', async () => {
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    const error = await runWithInterceptors(() => {
      render(<PixiViewport segments={mockSimulationTickPayload as any} />);
    });

    expect(error).toBeNull();

    consoleSpy.mockRestore();
  });

  // --- ADDITIONAL STRESS & ADVERSARIAL TESTS FOR MILESTONE T12 ---

  it('Verify production coordinate synchronization between segments and environmental elements', async () => {
    const originalVitest = (globalThis as any).process?.env?.VITEST;
    (globalThis as any).process.env.VITEST = 'false';

    try {
      mockGraphicsMethods.drawCircle.mockClear();

      const testSegments = [
        {
          agent_id: 1,
          segment_id: 0,
          parent_segment_id: null,
          x: 10,
          y: 20,
          z: 0,
          energy: 100,
          agent_type: 'prey'
        }
      ];

      const testEnv = {
        elements: [
          {
            type: 'lake',
            x: 10,
            y: 20,
            radius: 15,
            resources: 100
          }
        ]
      };

      const { rerender } = render(
        <PixiViewport
          segments={testSegments}
          environmentalState={testEnv}
          zoom={1.0}
          pan={{ x: 0, y: 0 }}
        />
      );

      // Wait for async initPixi to complete and run initial draw
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 50));
      });

      // Clear calls from initial draw to isolate the final draw
      mockGraphicsMethods.drawCircle.mockClear();

      // Force a redraw by re-rendering with same props
      rerender(
        <PixiViewport
          segments={testSegments}
          environmentalState={testEnv}
          zoom={1.0}
          pan={{ x: 0, y: 0 }}
        />
      );

      const drawCalls = mockGraphicsMethods.drawCircle.mock.calls;
      
      const segmentCall = drawCalls.find(c => Math.abs(c[2] - 10) < 0.1);
      const lakeCall = drawCalls.find(c => Math.abs(c[2] - 4381.1) < 0.1);

      expect(segmentCall).toBeDefined();
      expect(lakeCall).toBeDefined();

      // Verify the coordinate synchronization: both segment and lake are mapped to centered/fit-to-screen coords
      expect(segmentCall[0]).toBeCloseTo(250);
      expect(segmentCall[1]).toBeCloseTo(175);
      expect(lakeCall[0]).toBeCloseTo(250);
      expect(lakeCall[1]).toBeCloseTo(175);

    } finally {
      (globalThis as any).process.env.VITEST = originalVitest;
    }
  });

  it('Verify app does not crash when environmentalState.elements is not an array', async () => {
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    const invalidEnvState = { elements: {} as any };

    const error = await runWithInterceptors(async () => {
      render(<PixiViewport environmentalState={invalidEnvState} />);
      // Wait for async initPixi to complete
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    expect(error).toBeNull();

    consoleSpy.mockRestore();
  });

  it('Verify production coordinate mapping with NaN coordinates does not crash but results in NaN screen coordinates', async () => {
    const originalVitest = (globalThis as any).process?.env?.VITEST;
    (globalThis as any).process.env.VITEST = 'false';

    try {
      const nanSegments = [
        {
          agent_id: 1,
          segment_id: 0,
          parent_segment_id: null,
          x: NaN,
          y: NaN,
          z: NaN,
          energy: 100,
          agent_type: 'prey'
        }
      ];

      mockGraphicsMethods.drawCircle.mockClear();

      const { rerender } = render(<PixiViewport segments={nanSegments} />);

      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 50));
      });

      mockGraphicsMethods.drawCircle.mockClear();

      rerender(<PixiViewport segments={nanSegments} />);

      const drawCalls = mockGraphicsMethods.drawCircle.mock.calls;
      const segmentCall = drawCalls.find(c => Math.abs(c[2] - 10) < 0.1);
      expect(segmentCall).toBeDefined();
      expect(isNaN(segmentCall[0]) || isNaN(segmentCall[1])).toBe(true);

    } finally {
      (globalThis as any).process.env.VITEST = originalVitest;
    }
  });

  it('Verify coordinate calculation under extreme zoom and pan values', async () => {
    const testEnv = {
      elements: [
        {
          type: 'lake',
          x: 50,
          y: 50,
          radius: 30,
          resources: 100
        }
      ]
    };

    const { rerender } = render(
      <PixiViewport
        environmentalState={testEnv}
        zoom={10.0}
        pan={{ x: 1000, y: -1000 }}
      />
    );

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    mockGraphicsMethods.drawCircle.mockClear();

    rerender(
      <PixiViewport
        environmentalState={testEnv}
        zoom={10.0}
        pan={{ x: 1000, y: -1000 }}
      />
    );

    const drawCalls = mockGraphicsMethods.drawCircle.mock.calls;
    const finalLake = drawCalls.find(c => Math.abs(c[2] - 271.2) < 0.1);
    expect(finalLake).toBeDefined();
    expect(finalLake[0]).toBeCloseTo(1500);
    expect(finalLake[1]).toBeCloseTo(-500);
  });
});
