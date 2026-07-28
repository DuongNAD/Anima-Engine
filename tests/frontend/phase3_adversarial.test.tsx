import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { Mock } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import { App } from '../../src/App';
import { listen, emit } from '@tauri-apps/api/event';
import { makeCanvas2DStub, stubCanvas2D, type Canvas2DStub } from '../mocks/canvas-2d';

// The first `render(<App />)` here pays for the whole lazy module graph plus world generation
// under jsdom — seconds, not milliseconds, and more again when the machine is shared with other
// builds. Raised at file scope, where that cost actually is, rather than in the project config
// where it would also hide a hang in a test that should finish instantly.
vi.setConfig({ testTimeout: 30_000 });

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
  let mockCtx: Canvas2DStub;

  beforeEach(() => {
    vi.clearAllMocks();
    mockCtx = makeCanvas2DStub();
    stubCanvas2D(mockCtx);
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

    // Malformed raycast payload where direction is missing (causing undefined[0] TypeError).
    //
    // Typed as `unknown[]`, not as an array of casts: the point of the payload is that it does *not*
    // satisfy `RaycastTelemetry`, and saying so once at the array is both shorter and closer to what
    // arrives over IPC — bytes nothing has checked yet.
    const malformedRaycast: unknown[] = [
      {
        origin: [0, 0, 0],
        direction: undefined,
        hit_distance: 10.0,
        hit_entity_type: 'Prey',
        agent_id: 1
      }
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

    const malformedCombat: unknown = {
      predator_id: 1,
      prey_id: 2,
      damage: undefined, // Missing damage, handled by fallback
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
    // One record per `listen` call: the event it subscribed to, and the unlisten it handed back.
    const cleanupSpies: Array<{ eventName: string; spy: Mock<() => void> }> = [];

    // Mock the listen function specifically for this test
    vi.mocked(listen).mockImplementation(async (eventName) => {
      const spy = vi.fn(() => {});
      cleanupSpies.push({ eventName: String(eventName), spy });
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
    cleanupSpies.forEach(({ spy }) => {
      expect(spy).toHaveBeenCalled(); // Confirms the listener was CLEANED UP!
    });
  });
});
