import { vi, describe, it, expect, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { App } from '../../src/App';

// Store invoked calls to inspect them in the test
const invokedCalls: { cmd: string; args: any }[] = [];

vi.mock('@tauri-apps/api/core', async (importOriginal) => {
  const original = await importOriginal<typeof import('@tauri-apps/api/core')>();
  return {
    ...original,
    invoke: vi.fn().mockImplementation((cmd, args) => {
      invokedCalls.push({ cmd, args });
      if (cmd === 'get_organism_presets') {
        return Promise.resolve([
          {
            name: "Default Prey",
            description: "Default prey organism",
            segment_count: 3,
            agent_class: "Prey",
            initial_energy: 100.0,
            mass: 1.0,
            length: 1.0,
            radius: 0.2,
            frequency: 1.0,
            amplitude: 0.5,
          },
          {
            name: "Default Predator",
            description: "Default predator organism",
            segment_count: 3,
            agent_class: "Predator",
            initial_energy: 120.0,
            mass: 1.2,
            length: 1.0,
            radius: 0.25,
            frequency: 1.2,
            amplitude: 0.6,
          },
          {
            name: "Giant Prey",
            description: "A large, slow-moving prey organism",
            segment_count: 5,
            agent_class: "Prey",
            initial_energy: 200.0,
            mass: 3.0,
            length: 1.5,
            radius: 0.4,
            frequency: 0.6,
            amplitude: 0.4,
          },
          {
            name: "Speedy Predator",
            description: "A fast, agile predator organism",
            segment_count: 2,
            agent_class: "Predator",
            initial_energy: 90.0,
            mass: 0.8,
            length: 0.8,
            radius: 0.15,
            frequency: 2.0,
            amplitude: 0.8,
          },
          {
            name: "Long Snake",
            description: "A long snake-like prey organism",
            segment_count: 8,
            agent_class: "Prey",
            initial_energy: 150.0,
            mass: 0.7,
            length: 0.8,
            radius: 0.12,
            frequency: 1.5,
            amplitude: 0.7,
          }
        ]);
      }
      if (cmd === 'get_simulation_status') {
        return Promise.resolve({
          running: false,
          tick_count: 120,
          avg_tick_time_ms: 1.45,
          fps: 60.2,
        });
      }
      if (cmd === 'get_map_elites_grid') {
        return Promise.resolve({
          grid: {},
          grid_resolution: 50
        });
      }
      if (cmd === 'get_pheromone_grid') {
        return Promise.resolve({
          grid: [],
          width: 128,
          height: 128
        });
      }
      if (cmd === 'get_active_raycasts') {
        return Promise.resolve([]);
      }
      if (cmd === 'get_lineage_graph') {
        return Promise.resolve({
          nodes: [],
          links: [],
          db_connected: false
        });
      }
      if (cmd === 'get_chronicle_history') {
        return Promise.resolve([]);
      }
      if (cmd === 'get_env_state') {
        return Promise.resolve({
          ambient_light: 1.0,
          elapsed_time: 0.0,
          clouds: []
        });
      }
      return Promise.resolve(true);
    }),
  };
});

describe('Organism Library Front-end Tab UI', () => {
  beforeEach(() => {
    invokedCalls.length = 0;
  });

  it('renders tab button and toggles tab to Organism Library', async () => {
    render(<App />);

    // Tab button should be visible (query by role to distinguish it from the h2 title)
    const tabButton = screen.getByRole('button', { name: 'Thư viện Sinh vật' });
    expect(tabButton).toBeDefined();

    // Click the tab button
    fireEvent.click(tabButton);

    // Grid panel should be displayed
    const panel = screen.getByTestId('organism-library-panel');
    expect(panel).toBeDefined();
    expect(panel.style.display).not.toBe('none');
  });

  it('fetches presets and displays cards with preset details', async () => {
    render(<App />);

    // Click the tab button first to make sure UI is visible
    const tabButton = screen.getByRole('button', { name: 'Thư viện Sinh vật' });
    fireEvent.click(tabButton);

    // Wait for presets to load and render using findByText
    expect(await screen.findByText('Default Prey')).toBeTruthy();
    expect(await screen.findByText('Default Predator')).toBeTruthy();
    expect(await screen.findByText('Giant Prey')).toBeTruthy();
    expect(await screen.findByText('Speedy Predator')).toBeTruthy();
    expect(await screen.findByText('Long Snake')).toBeTruthy();

    // Verify some values
    expect(screen.getAllByText('Số phân đoạn:')[0].parentElement?.textContent).toContain('3');
    expect(screen.getAllByText('Năng lượng ban đầu:')[0].parentElement?.textContent).toContain('100');
  });

  it('triggers spawn_preset_organism Tauri command when Spawn is clicked', async () => {
    render(<App />);

    // Click the tab button first to make sure UI is visible
    const tabButton = screen.getByRole('button', { name: 'Thư viện Sinh vật' });
    fireEvent.click(tabButton);

    // Wait for presets to load and render using findByText
    expect(await screen.findByText('Default Prey')).toBeTruthy();

    // Find the spawn button for "Default Prey"
    const spawnButtons = screen.getAllByRole('button', { name: 'Spawn' });
    expect(spawnButtons.length).toBe(5);

    // Click first spawn button (Default Prey)
    fireEvent.click(spawnButtons[0]);

    // Verify spawn_preset_organism was invoked
    const spawnCall = invokedCalls.find(call => call.cmd === 'spawn_preset_organism');
    expect(spawnCall).toBeDefined();
    expect(spawnCall?.args).toEqual({ presetName: 'Default Prey' });
  });
});
