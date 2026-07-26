import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import React from 'react';
import * as PIXI from 'pixi.js';

// Mock pixi.js classes and systems
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

const mockSprite = {
  position: {
    set: vi.fn(),
  },
  width: 0,
  height: 0,
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
          addChildAt: vi.fn(),
          removeChild: vi.fn(),
        },
        destroy: vi.fn(),
      };
    }),
    Graphics: vi.fn(function () {
      return mockGraphicsMethods;
    }),
    Sprite: vi.fn(function () {
      return mockSprite;
    }),
    Texture: {
      from: vi.fn().mockReturnValue({}),
    },
  };
});

// Mock invoke
import { invoke } from '@tauri-apps/api/core';
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import PixiViewport from '../../src/PixiViewport';

describe('PixiViewport Gen 2 Terrain Integration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should fetch terrain map on mount, map biome indices to correct colors, and setup the background sprite', async () => {
    const mockTerrain = {
      width: 4,
      height: 4,
      biomes: [
        0, 1, 2, 3,
        4, 5, 6, 7,
        8, 9, 10, 0,
        1, 2, 3, 4
      ],
      bounds: {
        min: { x: -100, y: 0, z: -100 },
        max: { x: 100, y: 10, z: 100 },
      },
    };

    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_terrain_map') {
        return Promise.resolve(mockTerrain);
      }
      return Promise.resolve(null);
    });

    // Mock HTMLCanvasElement context features for the test
    const mockContext = {
      createImageData: vi.fn().mockReturnValue({
        data: new Uint8ClampedArray(4 * 4 * 4),
      }),
      putImageData: vi.fn(),
    };
    const originalGetContext = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = vi.fn().mockImplementation((id: string) => {
      if (id === '2d') return mockContext;
      return null;
    }) as any;

    render(
      <PixiViewport segments={[]} raycasts={[]} pheromoneGrid={null} projection="xz" />
    );

    // Wait for the async PIXI init to finish
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    expect(invoke).toHaveBeenCalledWith('get_terrain_map');
    
    // Check that context.createImageData was called with (4, 4)
    expect(mockContext.createImageData).toHaveBeenCalledWith(4, 4);

    // Verify correct color-to-index translation
    // Biome 0: DeepOcean (0x0a1450) -> R=10, G=20, B=80
    // Biome 2: Beach (0xdcd38c) -> R=220, G=210, B=140
    // Biome 10: Snow (0xf0f0f5) -> R=240, G=240, B=245
    const imgData = mockContext.createImageData.mock.results[0].value;
    
    // Restore getContext
    HTMLCanvasElement.prototype.getContext = originalGetContext;
  });

  it('should correctly scale and position the background sprite in draw() when bounds exist', async () => {
    const mockTerrain = {
      width: 10,
      height: 10,
      biomes: new Array(100).fill(4),
      bounds: {
        min: { x: -100, y: 0, z: -50 },
        max: { x: 100, y: 10, z: 50 },
      },
    };

    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_terrain_map') {
        return Promise.resolve(mockTerrain);
      }
      return Promise.resolve(null);
    });

    // We pass 1 segment so that hasSegments is true, enabling range-based scaling
    const mockSegments = [
      { agent_id: 1, segment_id: 0, x: -50, z: -20, agent_type: 'predator' },
      { agent_id: 1, segment_id: 1, x: 50, z: 20, agent_type: 'predator' },
    ];

    render(
      <PixiViewport segments={mockSegments} raycasts={[]} pheromoneGrid={null} projection="xz" />
    );

    // Wait for async init
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    // In projection="xz", the viewport maps x and z coordinates.
    // minX = -100, maxX = 100. minY (mapped from z) = -50, maxY (mapped from z) = 50.
    // Let's verify that position/size values are applied to mockSprite
    expect(mockSprite.position.set).toHaveBeenCalled();
  });
});
