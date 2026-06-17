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

// Mock pixi.js exactly like phase 5 test so rendering works in jsdom
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
    invoke: vi.fn().mockImplementation(async (cmd, args) => {
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
      return original.invoke(cmd, args);
    }),
  };
});

import { App } from '../../src/App';

describe('Phase 6 UI, Persistence, Viewport, and Telemetry Tests', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(performance, 'now').mockReturnValue(0);
  });

  it('renders Persistence UI Controls and invokes Tauri commands on click', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const filepathInput = screen.getByTestId('filepath-input');
    const saveButton = screen.getByTestId('save-state-button');
    const loadButton = screen.getByTestId('load-state-button');

    expect(filepathInput).toBeDefined();
    expect(saveButton).toBeDefined();
    expect(loadButton).toBeDefined();

    // Change value
    fireEvent.change(filepathInput, { target: { value: 'save_test.json' } });

    // Click Save State
    await act(async () => {
      fireEvent.click(saveButton);
    });
    expect(invoke).toHaveBeenCalledWith('save_simulation_state', { file_path: 'save_test.json' });

    // Click Load State
    await act(async () => {
      fireEvent.click(loadButton);
    });
    expect(invoke).toHaveBeenCalledWith('load_simulation_state', { file_path: 'save_test.json' });
  });

  it('renders camera zoom and pan controls and updates viewport state/rendering', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const zoomInBtn = screen.getByTestId('zoom-in-button');
    const zoomOutBtn = screen.getByTestId('zoom-out-button');
    const panLeftBtn = screen.getByTestId('pan-left-button');
    const panRightBtn = screen.getByTestId('pan-right-button');
    const panUpBtn = screen.getByTestId('pan-up-button');
    const panDownBtn = screen.getByTestId('pan-down-button');
    const panBtn = screen.getByTestId('pan-button');

    expect(zoomInBtn).toBeDefined();
    expect(zoomOutBtn).toBeDefined();
    expect(panLeftBtn).toBeDefined();
    expect(panRightBtn).toBeDefined();
    expect(panUpBtn).toBeDefined();
    expect(panDownBtn).toBeDefined();
    expect(panBtn).toBeDefined();

    // Verify initial drawing of environmental elements from mockEnvironmentalState
    // Elements:
    // 1. Lake at (50, 50), radius 30
    // 2. Tree at (-50, -50), radius 10
    // Initial draw (zoom: 1.0, pan: {x: 0, y: 0})
    const initialCalls = mockGraphicsMethods.drawCircle.mock.calls;
    // Lake concentric circles: outer radius = (30 - 2.8767) = 27.1233
    const initialLake = initialCalls.find(c => Math.abs(c[2] - 27.12) < 0.1);
    // Tree canopy leaves: radius = 16 * 0.8 = 12.8
    const initialTree = initialCalls.find(c => Math.abs(c[2] - 12.8) < 0.1);
    expect(initialLake).toBeDefined();
    expect(initialLake[0]).toBeCloseTo(50);
    expect(initialLake[1]).toBeCloseTo(50);
    expect(initialTree).toBeDefined();
    expect(initialTree[0]).toBeCloseTo(-50);
    expect(initialTree[1]).toBeCloseTo(-56.4);

    // Click Zoom In (zoom becomes 1.1)
    mockGraphicsMethods.drawCircle.mockClear();
    await act(async () => {
      fireEvent.click(zoomInBtn);
    });
    const zoomCalls = mockGraphicsMethods.drawCircle.mock.calls;
    // Lake radius = 27.1233 * 1.1 = 29.8356
    const zoomedLake = zoomCalls.find(c => Math.abs(c[2] - 29.83) < 0.1);
    // Tree canopy leaves = 12.8 * 1.1 = 14.08
    const zoomedTree = zoomCalls.find(c => Math.abs(c[2] - 14.08) < 0.1);
    expect(zoomedLake).toBeDefined();
    expect(zoomedLake[0]).toBeCloseTo(55);
    expect(zoomedLake[1]).toBeCloseTo(55);
    expect(zoomedTree).toBeDefined();
    expect(zoomedTree[0]).toBeCloseTo(-55);
    expect(zoomedTree[1]).toBeCloseTo(-62.04);

    // Click Pan Right (pan.x becomes 10)
    mockGraphicsMethods.drawCircle.mockClear();
    await act(async () => {
      fireEvent.click(panRightBtn);
    });
    const panCalls = mockGraphicsMethods.drawCircle.mock.calls;
    const pannedLake = panCalls.find(c => Math.abs(c[2] - 29.83) < 0.1);
    expect(pannedLake).toBeDefined();
    expect(pannedLake[0]).toBeCloseTo(65);
    expect(pannedLake[1]).toBeCloseTo(55);
  });

  it('renders environmental elements with appropriate colors', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    // Verify environmental elements container is rendered
    const container = screen.getByTestId('environmental-elements-container');
    expect(container).toBeDefined();
    expect(screen.getByText('• lake at (50, 50), radius 30')).toBeDefined();
    expect(screen.getByText('• tree at (-50, -50), radius 10')).toBeDefined();

    // Verify drawing colors (lake ripple/gray: 0xcccccc; tree canopy: 0x888888)
    expect(mockGraphicsMethods.beginFill).toHaveBeenCalledWith(0xcccccc, 0.1);
    expect(mockGraphicsMethods.beginFill).toHaveBeenCalledWith(0x888888, 0.85);
  });

  it('updates hydration and telemetry on simulation-tick event', async () => {
    render(<App />);

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    // Trigger simulation-tick event with Phase 6 payload
    await act(async () => {
      await emit('simulation-tick', mockSimulationTickPayload);
    });

    // Assert hydration and head direction telemetry display correctly
    const hydrationText = screen.getByTestId('hydration-telemetry');
    expect(hydrationText.textContent).toContain('75.0%');

    const headDirText = screen.getByTestId('head-direction-telemetry');
    expect(headDirText.textContent).toContain('[1.0, 0.0, 0.0]');
  });
});
