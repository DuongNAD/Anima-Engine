import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import React, {} from 'react';

// 1. Mock pixi.js
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
    // `function`, not an arrow: PixiViewport calls these with `new`, and under @vitest/spy 4 a
    // mock constructed with `new` reaches its implementation through `Reflect.construct`. An arrow
    // has no [[Construct]] slot, so it throws "() => ({...}) is not a constructor" from inside the
    // spy. Vitest 1 called the implementation plainly, so arrows worked there by accident.
    Application: vi.fn(function () {
      return {
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
      };
    }),
    Graphics: vi.fn(function () {
      return mockGraphicsMethods;
    }),
    Container: vi.fn(function () {
      return {
        addChild: vi.fn(),
        removeChild: vi.fn(),
      };
    }),
  };
});

// Mock Tauri APIs
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue({}),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

import PixiViewport from '../../src/PixiViewport';

describe('Phase 5 PixiJS Rendering Canvas Container', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should mount successfully and handle empty/null agent telemetry payloads without errors', async () => {
    render(
      <PixiViewport segments={null} raycasts={null} pheromoneGrid={null} />
    );

    // Wait for the async PIXI init to finish
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const viewportContainer = screen.getByTestId('pixi-canvas-container');
    expect(viewportContainer).toBeDefined();
    expect(viewportContainer.children.length).toBe(1); // canvas appended successfully
  });

  it('should render overlay geometries correctly on non-empty telemetry payload updates', async () => {
    const mockSegments = [
      { agent_id: 1, segment_id: 0, x: 100, y: 100, agent_type: 'predator' },
      { agent_id: 2, segment_id: 0, x: 200, y: 200, agent_type: 'prey' },
    ];
    const mockRaycasts = [
      { origin: [100, 100, 0], direction: [1, 0, 0], hit_distance: 50, hit_entity_type: 'Prey', agent_id: 1 },
    ];
    const mockPheromones = {
      grid: [0.0, 0.8, 0.0, 0.0],
      width: 2,
      height: 2,
    };

    render(
      <PixiViewport segments={mockSegments} raycasts={mockRaycasts} pheromoneGrid={mockPheromones} />
    );

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    // Check predator rendering (drawn as triangle via drawPolygon)
    const predatorPolygonCall = mockGraphicsMethods.drawPolygon.mock.calls.find(call => {
      const coords = call[0];
      return coords && Math.abs(coords[0] - 140) < 0.1 && Math.abs(coords[1] - 300) < 0.1;
    });
    expect(predatorPolygonCall).toBeDefined();
    const coords = predatorPolygonCall[0];
    expect(coords[2]).toBeCloseTo(118.0);
    expect(coords[3]).toBeCloseTo(307.83);
    expect(coords[4]).toBeCloseTo(118.0);
    expect(coords[5]).toBeCloseTo(292.17);

    // Verify predator fill color and opacity
    expect(mockGraphicsMethods.beginFill).toHaveBeenCalledWith(0xffffff, 0.3);

    // Verify predator direction line
    expect(mockGraphicsMethods.moveTo).toHaveBeenCalledWith(125, 300);
    expect(mockGraphicsMethods.lineTo).toHaveBeenCalledWith(146, 300);

    // Check prey rendering (drawn as circle via drawCircle)
    expect(mockGraphicsMethods.drawCircle).toHaveBeenCalledWith(375, 50, 10);
    expect(mockGraphicsMethods.beginFill).toHaveBeenCalledWith(0x777777, 0.3);

    // Verify prey tail indicator line
    expect(mockGraphicsMethods.moveTo).toHaveBeenCalledWith(375, 50);
    expect(mockGraphicsMethods.lineTo).toHaveBeenCalledWith(359, 50);

    // Check active raycast rendering (drawn as line via moveTo & lineTo)
    expect(mockGraphicsMethods.moveTo).toHaveBeenCalledWith(125, 300);
    expect(mockGraphicsMethods.lineTo).toHaveBeenCalledWith(250, 300);

    // Check pheromone grid heatmap tile rendering (drawn as rectangle via drawRect)
    expect(mockGraphicsMethods.drawRect).toHaveBeenCalledWith(250, 50, 125, 125);
  });
});
