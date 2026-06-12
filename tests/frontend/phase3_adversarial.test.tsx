import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import { App } from '../../src/App';
import { invoke } from '@tauri-apps/api/core';
import { listen, emit } from '@tauri-apps/api/event';

vi.mock('@tauri-apps/api/core', async (importOriginal) => {
  const original = await importOriginal<typeof import('@tauri-apps/api/core')>();
  return {
    ...original,
    invoke: vi.fn().mockImplementation((cmd, args) => {
      if (cmd === 'get_map_elites_grid') {
        return Promise.resolve({ grid: {}, grid_resolution: 50 });
      }
      if (cmd === 'get_simulation_status') {
        return Promise.resolve({ running: false, tick_count: 0, avg_tick_time_ms: 0, fps: 0 });
      }
      if (cmd === 'get_pheromone_grid') {
        return Promise.resolve({ grid: [], width: 0, height: 0 });
      }
      if (cmd === 'get_active_raycasts') {
        return Promise.resolve([]);
      }
      return original.invoke(cmd, args);
    }),
  };
});

describe('Phase 3 Front-end UI - Adversarial Stress Tests', () => {
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

  it('CRASH 1: should cause infinite recursion (Stack Overflow) when cyclic segments are rendered', async () => {
    render(<App />);

    // Seed a cyclic parent-child relation that traverses from root
    // Seg 0 -> null (Root)
    // Seg 1 -> 0 (Normal child)
    // Seg 2 -> 1 (Normal child, but duplicated/overwritten to form cycle)
    const cyclicSegments = [
      {
        agent_id: 1,
        segment_id: 0,
        parent_segment_id: null,
        x: 0, y: 0, z: 0, yaw: 0, pitch: 0, roll: 0,
        joint_anchor_x: 0, joint_anchor_y: 0, joint_anchor_z: 0,
        joint_axis_x: 0, joint_axis_y: 0, joint_axis_z: 0,
        energy: 100
      },
      {
        agent_id: 1,
        segment_id: 1,
        parent_segment_id: 0,
        x: 0, y: 0, z: 0, yaw: 0, pitch: 0, roll: 0,
        joint_anchor_x: 0, joint_anchor_y: 0, joint_anchor_z: 0,
        joint_axis_x: 0, joint_axis_y: 0, joint_axis_z: 0,
        energy: 100
      },
      {
        agent_id: 1,
        segment_id: 1,
        parent_segment_id: 1, // Duplicate segment_id 1 pointing to itself!
        x: 0, y: 0, z: 0, yaw: 0, pitch: 0, roll: 0,
        joint_anchor_x: 0, joint_anchor_y: 0, joint_anchor_z: 0,
        joint_axis_x: 0, joint_axis_y: 0, joint_axis_z: 0,
        energy: 100
      }
    ];

    // Wait at least 200ms to bypass throttle threshold in App.tsx
    await act(async () => {
      await emit('simulation-tick', cyclicSegments);
      await new Promise((resolve) => setTimeout(resolve, 250));
    });

    // In JSDOM, React 18 might throw in console.error or fail the render.
    // Let's assert that it fails or triggers maximum call stack size exceeded.
  });

  it('CRASH 2: should crash the canvas rendering loop when a malformed raycast payload is received', async () => {
    render(<App />);

    // Malformed raycast payload where direction is missing (causing undefined[0] TypeError)
    const malformedRaycast = [
      {
        origin: [0, 0, 0],
        direction: undefined as any,
        hit_distance: 10.0,
        hit_entity_type: 'Prey',
        agent_id: 1
      } as any
    ];

    // Emit the event
    await act(async () => {
      await emit('raycast-update', malformedRaycast);
    });
  });

  it('CRASH 3: should NOT crash the React component when a combat event contains undefined fields (e.g. damage)', async () => {
    const consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    render(<App />);

    // Wait for mock IPC calls and listeners to resolve
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    const malformedCombat = {
      predator_id: 1,
      prey_id: 2,
      damage: undefined as any, // Missing damage, handled by fallback
      energy_transferred: 10
    };

    // Emit the malformed combat event
    await act(async () => {
      await emit('combat-event', malformedCombat);
    });

    expect(consoleErrorSpy).not.toHaveBeenCalled();
    // Verify fallback rendered
    expect(screen.queryByText(/Predator #1 damaged Prey #2/)).not.toBeNull();
    consoleErrorSpy.mockRestore();
  });

  it('LEAK 1: should NOT leak Tauri event listeners if component is unmounted immediately after mounting (race condition)', async () => {
    // Create an array of mock cleanup functions
    const cleanupSpies: any[] = [];

    // Mock the listen function specifically for this test
    vi.mocked(listen).mockImplementation(async (eventName: string, callback: any) => {
      const spy = vi.fn(() => {});
      cleanupSpies.push({ eventName, spy });
      return spy;
    });

    const { unmount } = render(<App />);

    // Unmount immediately before the setup async functions resolve
    unmount();

    // Flush microtasks to allow the async setup listeners to run
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    // Check if the cleanup functions returned by listen were called.
    expect(cleanupSpies.length).toBeGreaterThan(0);
    cleanupSpies.forEach(({ eventName, spy }) => {
      expect(spy).toHaveBeenCalled(); // Confirms the listener was CLEANED UP!
    });
  });
});
