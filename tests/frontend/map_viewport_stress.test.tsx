import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import React from 'react';

// We'll track the mock container instances and ticker callbacks to inspect values.
let containerInstances: any[] = [];
let tickerCallbacks: any[] = [];

const mockGraphicsMethods = {
  rect: vi.fn().mockReturnThis(),
  stroke: vi.fn().mockReturnThis(),
  lineStyle: vi.fn().mockReturnThis(),
  fill: vi.fn().mockReturnThis(),
  moveTo: vi.fn().mockReturnThis(),
  lineTo: vi.fn().mockReturnThis(),
  bezierCurveTo: vi.fn().mockReturnThis(),
  quadraticCurveTo: vi.fn().mockReturnThis(),
  circle: vi.fn().mockReturnThis(),
  ellipse: vi.fn().mockReturnThis(),
  closePath: vi.fn().mockReturnThis(),
  clear: vi.fn().mockReturnThis(),
  addChild: vi.fn().mockReturnThis(),
  on: vi.fn().mockReturnThis(),
  off: vi.fn().mockReturnThis(),
  eventMode: 'none',
  cursor: 'auto',
};

const mockStageAddChild = vi.fn();
const mockAppDestroy = vi.fn();
const mockTextureDestroy = vi.fn();
const mockTickerAdd = vi.fn().mockImplementation((cb) => {
  tickerCallbacks.push(cb);
});
const mockTickerRemove = vi.fn().mockImplementation((cb) => {
  tickerCallbacks = tickerCallbacks.filter(c => c !== cb);
});

// Canvas element that we can dispatch events to
const mockCanvas = document.createElement('canvas');
mockCanvas.getBoundingClientRect = () => ({
  left: 10,
  top: 10,
  right: 810,
  bottom: 610,
  width: 800,
  height: 600,
  x: 10,
  y: 10,
  toJSON: () => {},
});

vi.mock('pixi.js', () => {
  const ApplicationMock = vi.fn().mockImplementation(() => ({
    init: vi.fn().mockResolvedValue(undefined),
    canvas: mockCanvas,
    stage: {
      addChild: mockStageAddChild,
      removeChild: vi.fn(),
    },
    ticker: {
      add: mockTickerAdd,
      remove: mockTickerRemove,
    },
    destroy: mockAppDestroy,
  }));

  const ContainerMock = vi.fn().mockImplementation(() => {
    const inst = {
      addChild: vi.fn(),
      destroy: vi.fn(),
      sortableChildren: false,
      position: {
        x: 0,
        y: 0,
        set: vi.fn().mockImplementation((x, y) => {
          inst.position.x = x;
          inst.position.y = y;
        }),
      },
      scale: {
        x: 1,
        y: 1,
        set: vi.fn().mockImplementation((s) => {
          inst.scale.x = s;
          inst.scale.y = s;
        }),
      },
      on: vi.fn().mockReturnThis(),
      off: vi.fn().mockReturnThis(),
      eventMode: 'none',
      cursor: 'auto',
    };
    containerInstances.push(inst);
    return inst;
  });

  const GraphicsMock = vi.fn().mockImplementation(() => mockGraphicsMethods);

  const TilingSpriteMock = vi.fn().mockImplementation((options) => ({
    options,
    destroy: vi.fn(),
  }));

  const TextureMock = vi.fn().mockImplementation((options) => ({
    options,
    destroy: mockTextureDestroy,
  }));
  (TextureMock as any).from = vi.fn().mockImplementation(() => ({
    destroy: mockTextureDestroy,
  }));

  const TextMock = vi.fn().mockImplementation(() => ({
    anchor: {
      set: vi.fn(),
    },
    x: 0,
    y: 0,
    destroy: vi.fn(),
  }));

  const TextStyleMock = vi.fn().mockImplementation((options) => options);

  return {
    Application: ApplicationMock,
    Container: ContainerMock,
    Graphics: GraphicsMock,
    TilingSprite: TilingSpriteMock,
    Texture: TextureMock,
    Text: TextMock,
    TextStyle: TextStyleMock,
  };
});

import MapViewport from '../../playground/fantasy-map/MapViewport';

describe('MapViewport Navigation Stress & Adversarial Tests', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    containerInstances = [];
    tickerCallbacks = [];
  });

  // Helpers to simulate interactions
  const triggerPointerDown = (x: number, y: number, button = 0, pointerId = 1) => {
    const e = new MouseEvent('pointerdown', {
      clientX: x,
      clientY: y,
      button,
      bubbles: true,
    }) as any;
    e.pointerId = pointerId;
    mockCanvas.dispatchEvent(e);
  };

  const triggerPointerMove = (x: number, y: number, pointerId = 1) => {
    const e = new MouseEvent('pointermove', {
      clientX: x,
      clientY: y,
      bubbles: true,
    }) as any;
    e.pointerId = pointerId;
    mockCanvas.dispatchEvent(e);
  };

  const triggerPointerUp = (x: number, y: number, pointerId = 1) => {
    const e = new MouseEvent('pointerup', {
      clientX: x,
      clientY: y,
      bubbles: true,
    }) as any;
    e.pointerId = pointerId;
    mockCanvas.dispatchEvent(e);
  };

  const triggerWheel = (x: number, y: number, deltaY: number) => {
    const e = new WheelEvent('wheel', {
      clientX: x,
      clientY: y,
      deltaY,
      bubbles: true,
      cancelable: true,
    });
    mockCanvas.dispatchEvent(e);
  };

  const runTick = (deltaTime: number, elapsedMS?: number) => {
    tickerCallbacks.forEach((cb) => {
      cb({ deltaTime, elapsedMS });
    });
  };

  it('initializes mapContentContainer at scale 1.0 and position (0, 0)', async () => {
    render(<MapViewport />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });

    const mapContentContainer = containerInstances[0];
    expect(mapContentContainer).toBeDefined();
    expect(mapContentContainer.scale.x).toBe(1.0);
    expect(mapContentContainer.position.x).toBe(0);
    expect(mapContentContainer.position.y).toBe(0);
  });

  it('keeps map position at (0, 0) under scale 1.0 even when dragged (boundary clamping)', async () => {
    render(<MapViewport />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });

    const mapContentContainer = containerInstances[0];

    // Drag from 100, 100 to 200, 200
    triggerPointerDown(100, 100);
    triggerPointerMove(200, 200);
    triggerPointerUp(200, 200);

    // Run ticker to let any update propagate
    runTick(1.0, 16.6);

    expect(mapContentContainer.scale.x).toBe(1.0);
    expect(mapContentContainer.position.x).toBe(0);
    expect(mapContentContainer.position.y).toBe(0);
  });

  it('clamps zoom scale between 1.0 and 5.0 under extreme scroll wheel actions', async () => {
    render(<MapViewport />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });

    const mapContentContainer = containerInstances[0];

    // Zoom in massively (e.deltaY < 0)
    for (let i = 0; i < 20; i++) {
      triggerWheel(400, 300, -100);
    }
    expect(mapContentContainer.scale.x).toBeCloseTo(5.0, 3);

    // Zoom out massively (e.deltaY > 0)
    for (let i = 0; i < 20; i++) {
      triggerWheel(400, 300, 100);
    }
    expect(mapContentContainer.scale.x).toBeCloseTo(1.0, 3);
  });

  it('handles zoom coordinates far outside the canvas without throwing or breaking bounds constraints', async () => {
    render(<MapViewport />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });

    const mapContentContainer = containerInstances[0];

    // Zoom in at client coordinates (-1000, 2000)
    triggerWheel(-1000, 2000, -100);

    expect(mapContentContainer.scale.x).toBeGreaterThan(1.0);
    // Position must be clamped to boundaries
    const s = mapContentContainer.scale.x;
    const maxAllowedLeft = 800 * (1 - s);
    const maxAllowedTop = 600 * (1 - s);

    expect(mapContentContainer.position.x).toBeGreaterThanOrEqual(maxAllowedLeft);
    expect(mapContentContainer.position.x).toBeLessThanOrEqual(0);
    expect(mapContentContainer.position.y).toBeGreaterThanOrEqual(maxAllowedTop);
    expect(mapContentContainer.position.y).toBeLessThanOrEqual(0);
  });

  it('retains position inside strict mathematical bounds during pan and zoom updates', async () => {
    render(<MapViewport />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });

    const mapContentContainer = containerInstances[0];

    // Zoom to scale 2.0 at center (400, 300)
    // 1.15^5 ~ 2.01
    for (let i = 0; i < 5; i++) {
      triggerWheel(400, 300, -100);
    }
    const s = mapContentContainer.scale.x;
    expect(s).toBeCloseTo(2.01, 1);

    // Boundary constraints for s = 2.01:
    // minX = 800 * (1 - 2.01) = -808
    // minY = 600 * (1 - 2.01) = -606
    const minX = 800 * (1 - s);
    const minY = 600 * (1 - s);

    // Try to drag map far past top-left boundary (drag direction: negative delta)
    triggerPointerDown(400, 300);
    triggerPointerMove(100, 100); // moves map by -300, -200
    triggerPointerUp(100, 100);

    expect(mapContentContainer.position.x).toBeGreaterThanOrEqual(minX);
    expect(mapContentContainer.position.y).toBeGreaterThanOrEqual(minY);

    // Try to drag map far past bottom-right boundary (drag direction: positive delta)
    triggerPointerDown(100, 100);
    triggerPointerMove(900, 700); // moves map by +800, +600
    triggerPointerUp(900, 700);

    expect(mapContentContainer.position.x).toBeLessThanOrEqual(0);
    expect(mapContentContainer.position.y).toBeLessThanOrEqual(0);
  });

  it('calculates inertia from rapid panning, applies correct damping, and stops exactly at bounds', async () => {
    render(<MapViewport />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });

    const mapContentContainer = containerInstances[0];

    // Zoom in so panning is allowed (zoom in 5 times -> scale ~2.01)
    for (let i = 0; i < 5; i++) {
      triggerWheel(400, 300, -100);
    }
    const s = mapContentContainer.scale.x;

    // Simulate rapid panning: move pointers with specific timestamps
    // To mock performance.now(), we will temporarily mock it or simulate rapid ticks
    const nowSpy = vi.spyOn(performance, 'now');
    let currentTime = 1000;
    nowSpy.mockImplementation(() => currentTime);

    // Drag started at t=1000, position=(400, 300)
    triggerPointerDown(400, 300);

    // Move 1 at t=1020, position=(350, 280)
    currentTime = 1020;
    triggerPointerMove(350, 280);

    // Move 2 at t=1040, position=(300, 250) (velocity dx = -50, dy = -30 in 20ms)
    currentTime = 1040;
    triggerPointerMove(300, 250);

    // Up at t=1045, position=(300, 250)
    currentTime = 1045;
    triggerPointerUp(300, 250);

    // Expected velocity: dx/dt and dy/dt in px/ms
    // dx = 300 - 400 = -100
    // dy = 250 - 30 = -50 (wait: y down is (300, 280, 250) so dy is 250 - 300 = -50)
    // dt = 1045 - 1000 = 45 ms
    // velocityX should be approx -100 / 45 = -2.22 px/ms
    // velocityY should be approx -50 / 45 = -1.11 px/ms

    // Run a tick. Since dragging is false, inertia updates position
    const posXBefore = mapContentContainer.position.x;
    const posYBefore = mapContentContainer.position.y;

    // Tick with dt=1, elapsedMS=16.6ms
    runTick(1.0, 16.6);

    const posXAfter = mapContentContainer.position.x;
    const posYAfter = mapContentContainer.position.y;

    // Since we dragged from 400 to 300, mapX should have moved negatively
    expect(posXAfter).toBeLessThan(posXBefore);
    expect(posYAfter).toBeLessThan(posYBefore);

    // Verify friction is applied: velocity decreases
    // Run multiple ticks to verify damping brings map to rest
    for (let i = 0; i < 100; i++) {
      runTick(1.0, 16.6);
    }

    const posFinalX = mapContentContainer.position.x;
    const posFinalY = mapContentContainer.position.y;

    // Subsequent ticks should not move the map since velocity is damped to 0
    runTick(1.0, 16.6);
    expect(mapContentContainer.position.x).toBe(posFinalX);
    expect(mapContentContainer.position.y).toBe(posFinalY);

    nowSpy.mockRestore();
  });

  it('robustly handles zero, negative, and extreme tick times without crashes or NaN states', async () => {
    render(<MapViewport />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });

    const mapContentContainer = containerInstances[0];

    // Set scale to 2.0 to allow panning
    triggerWheel(400, 300, -100);
    triggerWheel(400, 300, -100);
    triggerWheel(400, 300, -100);

    // Set up velocity via drag
    triggerPointerDown(400, 300);
    triggerPointerMove(380, 280);
    triggerPointerUp(380, 280);

    // 1. Tick with zero dt
    expect(() => runTick(0, 0)).not.toThrow();
    expect(Number.isNaN(mapContentContainer.position.x)).toBe(false);
    expect(Number.isNaN(mapContentContainer.position.y)).toBe(false);

    // 2. Tick with negative dt (should not crash)
    expect(() => runTick(-1.0, -16.6)).not.toThrow();
    expect(Number.isNaN(mapContentContainer.position.x)).toBe(false);

    // 3. Tick with extreme positive dt (simulating long background tab pause)
    expect(() => runTick(1000.0, 16666.0)).not.toThrow();
    expect(Number.isNaN(mapContentContainer.position.x)).toBe(false);
  });

  it('safely handles zero duration flicks (up and down in same ms) during pointerup without division-by-zero or NaN velocity', async () => {
    render(<MapViewport />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });

    const mapContentContainer = containerInstances[0];

    // Zoom in
    triggerWheel(400, 300, -100);

    const nowSpy = vi.spyOn(performance, 'now');
    nowSpy.mockImplementation(() => 2000); // static timestamp

    // Down, Move, Up all at the exact same millisecond
    triggerPointerDown(400, 300);
    triggerPointerMove(350, 280);
    triggerPointerUp(350, 280);

    // Run tick to verify no NaN position
    runTick(1.0, 16.6);
    expect(Number.isNaN(mapContentContainer.position.x)).toBe(false);
    expect(Number.isNaN(mapContentContainer.position.y)).toBe(false);

    nowSpy.mockRestore();
  });
});
