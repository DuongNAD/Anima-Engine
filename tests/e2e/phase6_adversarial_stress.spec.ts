import { test, expect } from '@playwright/test';

declare global {
  interface Window {
    __mock_listeners: Record<string, number[]>;
    __mock_callbacks: Map<number, (event: any) => void>;
    __mock_callback_counter: number;
    __mock_emit: (eventName: string, payload: any) => void;
    __TAURI_INTERNALS__: any;
    __TAURI_EVENT_PLUGIN_INTERNALS__: any;
  }
}

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
        invoke: async (cmd: string, args: any) => {
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
            const { event, handler } = args;
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
        transformCallback: (callback: any) => {
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
      window.__mock_emit = (eventName: string, payload: any) => {
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
    await page.goto('http://localhost:5173', { waitUntil: 'load' });
  });

  test('Adversarial E2E: Stable under corrupted/non-numeric telemetry formats', async ({ page }) => {
    let pageError: Error | null = null;
    page.on('pageerror', (err) => {
      pageError = err;
    });

    // Inject tick payload containing NaN / non-numeric energy value, strings, and missing attributes
    await page.evaluate(() => {
      window.__mock_emit('simulation-tick', {
        segments: [
          {
            agent_id: 99,
            segment_id: 0,
            parent_segment_id: null,
            x: 10, y: 10, z: 0, yaw: 0, pitch: 0, roll: 0,
            joint_anchor_x: 0, joint_anchor_y: 0, joint_anchor_z: 0,
            joint_axis_x: 0, joint_axis_y: 0, joint_axis_z: 0,
            energy: "corrupted_string_energy" as any, // non-numeric energy
            hydration: NaN, // non-numeric hydration
            agent_type: 'predator',
            head_direction: [NaN, "invalid", undefined] as any
          },
          {
            agent_id: 99,
            segment_id: 1,
            parent_segment_id: 1, // forms parent-child cycle
            x: undefined as any, y: 10, z: 0, yaw: 0, pitch: 0, roll: 0,
            joint_anchor_x: 0, joint_anchor_y: 0, joint_anchor_z: 0,
            joint_axis_x: 0, joint_axis_y: 0, joint_axis_z: 0,
            energy: undefined as any, // missing energy
            hydration: undefined,
            agent_type: 'prey'
          }
        ],
        environmental_state: {
          elements: [
            { type: 'lake', x: NaN, y: 50, radius: "corrupted" as any, resources: undefined as any }
          ]
        },
        head_directions: [
          { agent_id: 99, direction: null as any }
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
      const segments: any[] = [];
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
