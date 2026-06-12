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

test.describe('Phase 3 E2E - Adversarial Stress & Payload Tests', () => {

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
          if (cmd === 'get_sharding_config') {
            return { is_enabled: false, current_shard: 0, total_shards: 1 };
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
          if (cmd === 'toggle_simulation') {
            return true;
          }
          if (cmd === 'toggle_evolution') {
            return true;
          }
          if (cmd === 'update_evolution_settings') {
            return true;
          }
          if (cmd === 'trigger_migration') {
            return;
          }
          if (cmd === 'set_sharding_config') {
            return;
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

  test('Adversarial E2E: Stable under rapid projection switching', async ({ page }) => {
    // Locate the projection buttons
    const xyButton = page.locator('button', { hasText: 'X-Y' });
    const xzButton = page.locator('button', { hasText: 'X-Z' });

    // Ensure buttons are loaded
    await expect(xyButton).toBeVisible();
    await expect(xzButton).toBeVisible();

    let pageError: Error | null = null;
    page.on('pageerror', (err) => {
      pageError = err;
    });

    // Perform rapid projection switching in a tight loop to stress test rendering loop transitions
    for (let i = 0; i < 30; i++) {
      await xzButton.click();
      await xyButton.click();
    }

    // Verify no unhandled page crash/error occurred
    expect(pageError).toBeNull();
  });

  test('Adversarial E2E: Capture canvas rendering crash on malformed direction payload', async ({ page }) => {
    let pageError: Error | null = null;
    page.on('pageerror', (err) => {
      pageError = err;
    });

    // Emit a valid simulation tick first to ensure the canvas enters the segment rendering loop
    await page.evaluate(() => {
      window.__mock_emit('simulation-tick', [
        {
          agent_id: 12,
          segment_id: 0,
          parent_segment_id: null,
          x: 0, y: 0, z: 0, yaw: 0, pitch: 0, roll: 0,
          joint_anchor_x: 0, joint_anchor_y: 0, joint_anchor_z: 0,
          joint_axis_x: 0, joint_axis_y: 0, joint_axis_z: 0,
          energy: 100,
          agent_type: 'predator'
        }
      ]);
    });

    // Inject malformed raycast payload where direction is missing
    await page.evaluate(() => {
      window.__mock_emit('raycast-update', [
        {
          origin: [0, 0, 0],
          direction: undefined, // Missing direction field
          hit_distance: 5.0,
          hit_entity_type: 'Prey',
          agent_id: 12
        }
      ]);
    });

    // Allow time for requestAnimationFrame loop to execute
    await page.waitForTimeout(500);

    // Verify that NO error was thrown inside the window context
    expect(pageError).toBeNull();
  });

  test('Adversarial E2E: Capture combat event log crash on undefined damage field', async ({ page }) => {
    let pageError: Error | null = null;
    page.on('pageerror', (err) => {
      pageError = err;
    });

    // Inject malformed combat event where damage is undefined
    await page.evaluate(() => {
      window.__mock_emit('combat-event', {
        predator_id: 1,
        prey_id: 2,
        damage: undefined, // Causes e.damage.toFixed(1) crash if not handled
        energy_transferred: 10
      });
    });

    await page.waitForTimeout(200);

    // Verify that NO error was thrown inside the window context
    expect(pageError).toBeNull();
  });

  test('Adversarial E2E: Gracefully handles empty pheromone grid without crashing', async ({ page }) => {
    let pageError: Error | null = null;
    page.on('pageerror', (err) => {
      pageError = err;
    });

    // Emit empty pheromone grid
    await page.evaluate(() => {
      window.__mock_emit('pheromone-update', {
        grid: [],
        width: 0,
        height: 0
      });
    });

    await page.waitForTimeout(200);

    // Verify it doesn't crash on length 0 grid
    expect(pageError).toBeNull();
  });

  test('Adversarial E2E: Truncates combat event log and manages UI load under massive combat spam', async ({ page }) => {
    let pageError: Error | null = null;
    page.on('pageerror', (err) => {
      pageError = err;
    });

    // Spam 100 combat events in a loop
    await page.evaluate(() => {
      for (let i = 0; i < 100; i++) {
        window.__mock_emit('combat-event', {
          predator_id: i,
          prey_id: i + 1,
          damage: 10.5,
          energy_transferred: 8.0
        });
      }
    });

    // Verify the list contains only 50 elements (as per slice(0, 50))
    const combatEntries = page.locator('[data-testid="combat-log"] > div');
    await expect(combatEntries).toHaveCount(50);

    // Verify no crash occurred
    expect(pageError).toBeNull();
  });
});
