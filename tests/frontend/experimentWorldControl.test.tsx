// W2-lite: prototype, to be replaced.
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import {
  ExperimentWorldControl,
  SEL2P_SELECTED_MANIFEST_JSON,
} from '@/components/ExperimentWorldControl';

const { mockInvoke, mockLoadOrGenerateWorld } = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
  mockLoadOrGenerateWorld: vi.fn(),
}));

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

const DEFAULT_MOCK_WORLD = {
  size: 256,
  shape: 'continent',
  elevation: new Float32Array(256 * 256),
  moisture: new Float32Array(256 * 256),
  temperature: new Float32Array(256 * 256),
  flow: new Float32Array(256 * 256),
  biomes: new Uint8Array(256 * 256),
};

vi.mock('@/components/Landscape/utils/worldCache', () => ({
  loadOrGenerateWorld: mockLoadOrGenerateWorld,
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
    mockLoadOrGenerateWorld.mockReset();
    mockLoadOrGenerateWorld.mockResolvedValue(DEFAULT_MOCK_WORLD);
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
    const openBtn = screen.getByTestId('open-experiment-world-button') as HTMLButtonElement;
    expect(control).not.toBeNull();
    // Mounted in the same simulation controls row beside the toggle button.
    expect(toggleBtn.parentElement).toBe(control.parentElement);

    // Wait for shared world readiness (N3).
    await waitFor(() => expect(openBtn.disabled).toBe(false));

    // Does not call start_from_manifest on mount.
    expect(
      mockInvoke.mock.calls.some(([cmd]) => cmd === 'start_from_manifest'),
    ).toBe(false);

    const select = screen.getByTestId('experiment-manifest-select') as HTMLSelectElement;
    expect(select.value).toBe('sel2p-selected');

    fireEvent.click(openBtn);

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
    expect(screen.getByTestId('active-experiment-seed').textContent).toContain('780001');
    expect(screen.getByTestId('experiment-viewer-notice').textContent).toContain('Viewer only');
    // App syncs its running state so the toggle reflects that the simulation is active.
    expect(toggleBtn.textContent).toBe('Dừng mô phỏng');
  });

  it('clears the active manifest banner on stop so a subsequent normal start does not show it (N1)', async () => {
    mockInvoke.mockResolvedValue('SEL2-P selected - 16 spread lakes, scale 0.63, mutation 0.05');
    const { rerender } = render(<ExperimentWorldControl running={true} />);

    fireEvent.click(screen.getByTestId('open-experiment-world-button'));
    await waitFor(() =>
      expect(screen.queryByTestId('active-experiment-manifest-banner')).not.toBeNull(),
    );

    rerender(<ExperimentWorldControl running={false} />);
    expect(screen.queryByTestId('active-experiment-manifest-banner')).toBeNull();

    // Starting normally again must NOT bring back the stale manifest banner.
    rerender(<ExperimentWorldControl running={true} />);
    expect(screen.queryByTestId('active-experiment-manifest-banner')).toBeNull();
  });

  it('clears the SEL2-P banner in App after Stop followed by a normal Start (N1)', async () => {
    let simRunning = false;
    mockInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case 'get_simulation_status':
          return { running: simRunning, tick_count: 0, avg_tick_time_ms: 0, fps: 0 };
        case 'start_from_manifest':
          simRunning = true;
          return 'SEL2-P selected - 16 spread lakes, scale 0.63, mutation 0.05';
        case 'toggle_simulation':
          simRunning = !simRunning;
          return simRunning;
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
        default:
          return null;
      }
    });

    render(<App />);

    const toggleBtn = screen.getByTestId('toggle-simulation-button') as HTMLButtonElement;
    const openBtn = screen.getByTestId('open-experiment-world-button') as HTMLButtonElement;
    await waitFor(() => expect(openBtn.disabled).toBe(false));

    // 1. Start from manifest -> banner visible, running = true.
    fireEvent.click(openBtn);
    await waitFor(() =>
      expect(screen.queryByTestId('active-experiment-manifest-banner')).not.toBeNull(),
    );
    expect(toggleBtn.textContent).toBe('Dừng mô phỏng');

    // 2. Stop simulation via toggle -> banner clears and running = false.
    fireEvent.click(toggleBtn);
    await waitFor(() => expect(toggleBtn.textContent).toBe('Bắt đầu mô phỏng'));
    expect(screen.queryByTestId('active-experiment-manifest-banner')).toBeNull();

    // 3. Normal Start via toggle -> running = true, manifest banner stays absent.
    fireEvent.click(toggleBtn);
    await waitFor(() => expect(toggleBtn.textContent).toBe('Dừng mô phỏng'));
    expect(screen.queryByTestId('active-experiment-manifest-banner')).toBeNull();
  });

  it('disables Open experiment world until worldReady resolves just like toggle-simulation-button (N3)', async () => {
    let resolveWorld: (w: typeof DEFAULT_MOCK_WORLD) => void = () => {};
    mockLoadOrGenerateWorld.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveWorld = resolve;
        }),
    );
    setupDefaultAppInvoke();

    render(<App />);

    const toggleBtn = screen.getByTestId('toggle-simulation-button') as HTMLButtonElement;
    const openBtn = screen.getByTestId('open-experiment-world-button') as HTMLButtonElement;
    expect(toggleBtn.disabled).toBe(true);
    expect(openBtn.disabled).toBe(true);

    // Clicking while disabled must not invoke start_from_manifest.
    fireEvent.click(openBtn);
    expect(
      mockInvoke.mock.calls.some(([cmd]) => cmd === 'start_from_manifest'),
    ).toBe(false);

    resolveWorld(DEFAULT_MOCK_WORLD);
    await waitFor(() => expect(openBtn.disabled).toBe(false));
    expect(toggleBtn.disabled).toBe(false);
  });

  it('bundles the exact byte-for-byte SEL2-P selected fixture JSON (N4)', () => {
    const sha256 = createHash('sha256')
      .update(SEL2P_SELECTED_MANIFEST_JSON, 'utf8')
      .digest('hex');
    expect(sha256).toBe('36ba3500d4df6774d96e766bb16fdde9201fd6a0c61597ff29621cf7c8b6ae3d');

    const fixturePath = path.resolve(
      process.cwd(),
      'src-tauri/tests/fixtures/experiments_sel2p/sel2p-selected.json',
    );
    if (fs.existsSync(fixturePath)) {
      expect(SEL2P_SELECTED_MANIFEST_JSON).toBe(fs.readFileSync(fixturePath, 'utf8'));
    }
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
