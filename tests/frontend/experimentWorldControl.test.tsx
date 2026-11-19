// W2-lite: prototype, to be replaced.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ExperimentWorldControl } from '@/components/ExperimentWorldControl';

const mockInvoke = vi.hoisted(() => vi.fn());

vi.mock('pixi.js', () => {
  const graphicsMethods = {
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
  return {
    Application: vi.fn(function () {
      return {
        init: vi.fn().mockResolvedValue(undefined),
        canvas: document.createElement('canvas'),
        stage: { addChild: vi.fn(), removeChild: vi.fn() },
        renderer: {},
        ticker: { add: vi.fn(), remove: vi.fn() },
        destroy: vi.fn(),
      };
    }),
    Graphics: vi.fn(function () {
      return graphicsMethods;
    }),
    Container: vi.fn(function () {
      return { addChild: vi.fn(), removeChild: vi.fn() };
    }),
  };
});

vi.mock('@/components/Landscape/utils/worldCache', () => ({
  loadOrGenerateWorld: vi.fn().mockResolvedValue({
    size: 256,
    shape: 'continent',
    elevation: new Float32Array(256 * 256),
    moisture: new Float32Array(256 * 256),
    temperature: new Float32Array(256 * 256),
    flow: new Float32Array(256 * 256),
    biomes: new Uint8Array(256 * 256),
  }),
  getMemoizedWorld: vi.fn().mockReturnValue(null),
}));

vi.mock('@tauri-apps/api/core', async (importOriginal) => {
  const original = await importOriginal<typeof import('@tauri-apps/api/core')>();
  return {
    ...original,
    invoke: mockInvoke,
  };
});

import { App } from '@/App';

function setupDefaultAppInvoke(
  startHandler?: (args: Record<string, unknown> | undefined) => Promise<string>,
): void {
  mockInvoke.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
    switch (cmd) {
      case 'get_simulation_status':
        return { running: false, tick_count: 0, avg_tick_time_ms: 0, fps: 0 };
      case 'toggle_simulation':
        return true;
      case 'get_map_elites_grid':
        return { grid: {}, grid_resolution: 50 };
      case 'get_pheromone_grid':
        return { grid: [], width: 128, height: 128 };
      case 'get_active_raycasts':
        return [];
      case 'get_environmental_elements':
        return { elements: [] };
      case 'get_lineage_graph':
        return { nodes: [], links: [], db_connected: false };
      case 'get_chronicle_history':
        return [];
      case 'get_ecosystem_state':
        return {
          detritus: 0,
          plants: 0,
          animals: 0,
          total: 0,
          prey_count: 0,
          predator_count: 0,
          shannon: 0,
          simpson: 0,
          prey_mass: 0,
          predator_mass: 0,
          niche_divergence: 0,
          archive_coverage: 0,
        };
      case 'start_from_manifest':
        return startHandler
          ? startHandler(args)
          : 'SEL2-P selected - 16 spread lakes, scale 0.63, mutation 0.05';
      default:
        return null;
    }
  });
}

describe('ExperimentWorldControl (W2-lite)', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it('mounts inside App simulation controls row and invokes start_from_manifest with camelCase manifestJson', async () => {
    let capturedArgs: Record<string, unknown> | undefined;
    setupDefaultAppInvoke(async (args) => {
      capturedArgs = args;
      const parsed = JSON.parse(String(args?.manifestJson ?? '{}')) as { name?: string };
      return parsed.name ?? 'missing-name';
    });

    render(<App />);

    const toggleBtn = screen.getByTestId('toggle-simulation-button');
    const control = screen.getByTestId('experiment-world-control');
    expect(control).not.toBeNull();
    // Mounted in the same simulation controls row beside the toggle button.
    expect(toggleBtn.parentElement).toBe(control.parentElement);

    // Does not call start_from_manifest on mount.
    expect(
      mockInvoke.mock.calls.some(([cmd]) => cmd === 'start_from_manifest'),
    ).toBe(false);

    const select = screen.getByTestId('experiment-manifest-select') as HTMLSelectElement;
    expect(select.value).toBe('sel2p-selected');

    fireEvent.click(screen.getByTestId('open-experiment-world-button'));

    await waitFor(() =>
      expect(
        mockInvoke.mock.calls.some(([cmd]) => cmd === 'start_from_manifest'),
      ).toBe(true),
    );

    expect(Object.keys(capturedArgs ?? {})).toEqual(['manifestJson']);
    const manifestObj = JSON.parse(String(capturedArgs?.manifestJson)) as {
      name: string;
      initial_conditions: { values: [string, number][] };
    };
    const ic = new Map(manifestObj.initial_conditions.values);
    expect(ic.get('live.founders')).toBe(100);
    expect(ic.get('live.lakes')).toBe(16);
    expect(ic.get('live.lake_layout')).toBe(1);
    expect(ic.get('live.resource_capacity_scale')).toBe(0.63);
    expect(ic.get('live.prey_cap')).toBe(320);
    expect(ic.get('live.dehydration_ticks')).toBe(960);

    await waitFor(() =>
      expect(screen.getByTestId('active-experiment-manifest').textContent).toBe(
        'SEL2-P selected - 16 spread lakes, scale 0.63, mutation 0.05',
      ),
    );
    expect(screen.getByTestId('experiment-viewer-notice').textContent).toContain('Viewer only');
    // App syncs its running state so the toggle reflects that the simulation is active.
    expect(toggleBtn.textContent).toBe('Dừng mô phỏng');
  });

  it('hides the running banner when parent running transitions to false', async () => {
    mockInvoke.mockResolvedValue('SEL2-P selected - 16 spread lakes, scale 0.63, mutation 0.05');
    const { rerender } = render(<ExperimentWorldControl running={true} />);

    fireEvent.click(screen.getByTestId('open-experiment-world-button'));
    await waitFor(() =>
      expect(screen.queryByTestId('active-experiment-manifest-banner')).not.toBeNull(),
    );

    rerender(<ExperimentWorldControl running={false} />);
    expect(screen.queryByTestId('active-experiment-manifest-banner')).toBeNull();
  });

  it('surfaces backend preflight refusals verbatim and re-enables the button', async () => {
    mockInvoke.mockRejectedValue(
      'the live Bevy world has no exotic-energy field: laws.exotic_energy refused',
    );
    render(<ExperimentWorldControl running={false} />);

    const btn = screen.getByTestId('open-experiment-world-button') as HTMLButtonElement;
    fireEvent.click(btn);

    const alert = await screen.findByTestId('experiment-manifest-error');
    expect(alert.getAttribute('role')).toBe('alert');
    expect(alert.textContent).toContain('no exotic-energy field');
    expect(btn.disabled).toBe(false);
    expect(screen.queryByTestId('active-experiment-manifest-banner')).toBeNull();
  });
});
