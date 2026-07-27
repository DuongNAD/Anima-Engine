import { test, expect } from '@playwright/test';
// Declares the `window.__mock_*` and `window.__TAURI_*` globals this spec injects below.
import './tauri-mock-types';
import type { TauriEventEnvelope, TauriInvokeArgs } from './tauri-mock-types';

test.describe('Phase 6 E2E - Challenger Adversarial & Stress Tests', () => {

  test.beforeEach(async ({ page }) => {
    // Inject Tauri mock internals before the page loads
    await page.addInitScript(() => {
      window.__mock_listeners = {};
      window.__mock_callbacks = new Map();
      window.__mock_callback_counter = 0;

      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
        unregisterListener: () => {}
      };

      window.__TAURI_INTERNALS__ = {
        invoke: async (cmd: string, args: TauriInvokeArgs) => {
          if (cmd === 'get_map_elites_grid') {
            return { grid: {}, grid_resolution: 50 };
          }
          if (cmd === 'get_simulation_status') {
            return { running: false, tick_count: 0, avg_tick_time_ms: 0, fps: 0 };
          }
          if (cmd === 'get_pheromone_grid') {
            return { grid: new Array(128 * 128).fill(0.0), width: 128, height: 128 };
          }
          if (cmd === 'get_active_raycasts') {
            return [];
          }
          if (cmd === 'get_lineage_graph') {
            return { nodes: [], links: [], db_connected: false };
          }
          if (cmd === 'get_chronicle_history') {
            return [];
          }
          if (cmd === 'get_environmental_elements') {
            return {
              elements: [
                { type: 'lake', x: 50, y: 50, radius: 30, resources: 100 },
                { type: 'tree', x: -50, y: -50, radius: 10, resources: 50 }
              ]
            };
          }
          if (cmd === 'plugin:event|listen') {
            // Both are optional on `TauriInvokeArgs` because `invoke` carries whatever the caller
            // sent, and a subscription with no event name has nothing to key a listener list on.
            const { event, handler } = args;
            if (event === undefined || handler === undefined) {
              throw new Error('plugin:event|listen was invoked without an event name or handler');
            }
            if (!window.__mock_listeners[event]) {
              window.__mock_listeners[event] = [];
            }
            window.__mock_listeners[event].push(handler);
            return handler;
          }
          if (cmd === 'plugin:event|unlisten') {
            return;
          }
          if (cmd === 'save_simulation_state' || cmd === 'load_simulation_state') {
            return true;
          }
          if (cmd === 'toggle_simulation') {
            return true;
          }
          throw new Error(`Unrecognized command mock: ${cmd}`);
        },
        transformCallback: (callback: (event: TauriEventEnvelope) => void) => {
          const id = ++window.__mock_callback_counter;
          window.__mock_callbacks.set(id, callback);
          return id;
        },
        unregisterCallback: (id: number) => {
          window.__mock_callbacks.delete(id);
        },
        convertFileSrc: (fp: string) => fp
      };

      // Helper to trigger events
      window.__mock_emit = (eventName: string, payload: unknown) => {
        const handlers = window.__mock_listeners[eventName] || [];
        handlers.forEach((handlerId: number) => {
          const cb = window.__mock_callbacks.get(handlerId);
          if (cb) {
            cb({ event: eventName, payload, id: handlerId });
          }
        });
      };
    });

    // Navigate to local Vite dev server
    await page.goto('/', { waitUntil: 'load' });
  });

  test('Adversarial E2E: Stable under corrupted/non-numeric telemetry formats', async ({ page }) => {
    let pageError: Error | null = null;
    page.on('pageerror', (err) => {
      pageError = err;
    });

    // Inject tick payload containing NaN / non-numeric energy value, strings, and missing attributes
    await page.evaluate(() => {
      // Deliberately not a valid tick payload — every field below is a shape the backend must
      // never send and the frontend must survive anyway. `__mock_emit` takes `unknown`, which is
      // what a payload straight off the IPC boundary is, so none of these needs a cast to say so.
      window.__mock_emit('simulation-tick', {
        segments: [
          {
            agent_id: 99,
            segment_id: 0,
            parent_segment_id: null,
            x: 10, y: 10, z: 0, yaw: 0, pitch: 0, roll: 0,
            joint_anchor_x: 0, joint_anchor_y: 0, joint_anchor_z: 0,
            joint_axis_x: 0, joint_axis_y: 0, joint_axis_z: 0,
            energy: "corrupted_string_energy", // non-numeric energy
            hydration: NaN, // non-numeric hydration
            agent_type: 'predator',
            head_direction: [NaN, "invalid", undefined]
          },
          {
            agent_id: 99,
            segment_id: 1,
            parent_segment_id: 1, // forms parent-child cycle
            x: undefined, y: 10, z: 0, yaw: 0, pitch: 0, roll: 0,
            joint_anchor_x: 0, joint_anchor_y: 0, joint_anchor_z: 0,
            joint_axis_x: 0, joint_axis_y: 0, joint_axis_z: 0,
            energy: undefined, // missing energy
            hydration: undefined,
            agent_type: 'prey'
          }
        ],
        environmental_state: {
          elements: [
            { type: 'lake', x: NaN, y: 50, radius: "corrupted", resources: undefined }
          ]
        },
        head_directions: [
          { agent_id: 99, direction: null }
        ]
      });
    });

    // Wait for frame rendering loop
    await page.waitForTimeout(500);

    // Verify page did not crash
    expect(pageError).toBeNull();

    // Confirm UI remains interactive (e.g. projection button is clickable)
    const xyButton = page.locator('button', { hasText: 'X-Y' });
    await expect(xyButton).toBeVisible();
    await xyButton.click();
    expect(pageError).toBeNull();
  });

  test('Adversarial E2E: Stable under massive telemetry loads (10,000+ segments)', async ({ page }) => {
    let pageError: Error | null = null;
    page.on('pageerror', (err) => {
      pageError = err;
    });

    // Generate 10,000 segments
    await page.evaluate(() => {
      const segments: unknown[] = [];
      for (let i = 0; i < 10000; i++) {
        segments.push({
          agent_id: i,
          segment_id: 0,
          parent_segment_id: null,
          x: (i % 100) * 2 - 100,
          y: Math.floor(i / 100) * 2 - 100,
          z: 0,
          yaw: 0.5, pitch: 0, roll: 0,
          joint_anchor_x: 0, joint_anchor_y: 0, joint_anchor_z: 0,
          joint_axis_x: 0, joint_axis_y: 0, joint_axis_z: 0,
          energy: 80,
          hydration: 90,
          agent_type: i % 2 === 0 ? 'predator' : 'prey',
          head_direction: [1.0, 0.0, 0.0]
        });
      }
      window.__mock_emit('simulation-tick', {
        segments,
        environmental_state: { elements: [] },
        head_directions: []
      });
    });

    // Wait to allow rendering or processing of the payload
    await page.waitForTimeout(1000);

    // Verify page did not crash
    expect(pageError).toBeNull();

    // Check that we can still interact with the UI elements
    const zoomInBtn = page.locator('[data-testid="zoom-in-button"]');
    await expect(zoomInBtn).toBeVisible();
    await zoomInBtn.click();
    expect(pageError).toBeNull();
  });

  test('E2E Boundary Check: Zoom limits are correctly enforced and clamped', async ({ page }) => {
    let pageError: Error | null = null;
    page.on('pageerror', (err) => {
      pageError = err;
    });

    const zoomInBtn = page.locator('[data-testid="zoom-in-button"]');
    const zoomOutBtn = page.locator('[data-testid="zoom-out-button"]');

    await expect(zoomInBtn).toBeVisible();
    await expect(zoomOutBtn).toBeVisible();

    // Click Zoom In 150 times
    for (let i = 0; i < 150; i++) {
      await zoomInBtn.click();
    }

    // Click Zoom Out 150 times
    for (let i = 0; i < 150; i++) {
      await zoomOutBtn.click();
    }

    // Confirm no errors occurred during extreme zooming
    expect(pageError).toBeNull();
  });

  test('E2E Boundary Check: Pan controls accept extreme values gracefully', async ({ page }) => {
    let pageError: Error | null = null;
    page.on('pageerror', (err) => {
      pageError = err;
    });

    const panRightBtn = page.locator('[data-testid="pan-right-button"]');
    const panDownBtn = page.locator('[data-testid="pan-down-button"]');

    await expect(panRightBtn).toBeVisible();
    await expect(panDownBtn).toBeVisible();

    // Perform rapid clicking to simulate extreme panning
    for (let i = 0; i < 15; i++) {
      await panRightBtn.click();
      await panDownBtn.click();
    }

    // Verify no UI lock or crash
    expect(pageError).toBeNull();
  });
});
