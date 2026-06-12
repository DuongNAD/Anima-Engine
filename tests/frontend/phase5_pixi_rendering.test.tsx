import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import React, { useEffect, useRef } from 'react';
import * as PIXI from 'pixi.js';

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
    const { container } = render(
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
    expect(mockGraphicsMethods.drawPolygon).toHaveBeenCalledWith([100, 88, 88, 112, 112, 112]);
    // Check prey rendering (drawn as circle via drawCircle)
    expect(mockGraphicsMethods.drawCircle).toHaveBeenCalledWith(200, 200, 10);
    // Check active raycast rendering (drawn as line via moveTo & lineTo)
    expect(mockGraphicsMethods.moveTo).toHaveBeenCalledWith(100, 100);
    expect(mockGraphicsMethods.lineTo).toHaveBeenCalledWith(150, 100);
    // Check pheromone grid heatmap tile rendering (drawn as rectangle via drawRect)
    expect(mockGraphicsMethods.drawRect).toHaveBeenCalledWith(4, 0, 4, 4);
  });
});
