import { describe, it, expect } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { listen, emit } from '@tauri-apps/api/event';
import {
  EvolutionSettings,
  MapElitesGridState,
  mockMapElitesGridState
} from '../mocks/mock_ipc_payloads';

describe('Tauri IPC Commands & Events - Phase 2', () => {
  it('should call get_map_elites_grid and return the expected grid state structure', async () => {
    const gridState = await invoke<MapElitesGridState>('get_map_elites_grid');

    expect(gridState).toBeDefined();
    expect(gridState.grid_resolution).toBe(50);
    expect(gridState.grid).toBeDefined();
    for (const [key, elite] of Object.entries(gridState.grid)) {
      expect(key).toMatch(/^\d+,\d+$/);
      expect(typeof elite.fitness).toBe('number');
      expect(elite.fitness).toBeGreaterThanOrEqual(0.0);
      expect(elite.fitness).toBeLessThanOrEqual(1.0);
      expect(elite.features.length).toBe(2);
    }
  });

  it('should call update_evolution_settings with valid settings and return true', async () => {
    const validSettings: EvolutionSettings = {
      mutation_rate: 0.25,
      selection_bias: 1.8,
      grid_resolution: 40
    };

    const result = await invoke<boolean>('update_evolution_settings', { settings: validSettings });
    expect(result).toBe(true);
  });

  it('should call update_evolution_settings with invalid settings and throw an error', async () => {
    const invalidSettings1: EvolutionSettings = {
      mutation_rate: -0.1, // negative mutation rate
      selection_bias: 1.5,
      grid_resolution: 50
    };

    await expect(
      invoke('update_evolution_settings', { settings: invalidSettings1 })
    ).rejects.toThrow();

    const invalidSettings2: EvolutionSettings = {
      mutation_rate: 1.5, // mutation rate > 1.0
      selection_bias: 1.5,
      grid_resolution: 50
    };

    await expect(
      invoke('update_evolution_settings', { settings: invalidSettings2 })
    ).rejects.toThrow();

    const invalidSettings3: EvolutionSettings = {
      mutation_rate: 0.2,
      selection_bias: -0.5, // negative selection bias
      grid_resolution: 50
    };

    await expect(
      invoke('update_evolution_settings', { settings: invalidSettings3 })
    ).rejects.toThrow();
  });

  it('should call toggle_evolution and return the toggled running state', async () => {
    // Initial evolution running state should be false
    const toggle1 = await invoke<boolean>('toggle_evolution');
    expect(toggle1).toBe(true);

    const toggle2 = await invoke<boolean>('toggle_evolution');
    expect(toggle2).toBe(false);
  });

  it('should subscribe to map-elites-update event stream and trigger it via emit', async () => {
    let receivedPayload: MapElitesGridState | null = null;

    const unlisten = await listen<MapElitesGridState>('map-elites-update', (event: any) => {
      receivedPayload = event.payload;
    });

    await emit('map-elites-update', mockMapElitesGridState);

    expect(receivedPayload).not.toBeNull();
    expect(receivedPayload!.grid_resolution).toBe(50);
    expect(receivedPayload!.grid['10,20'].fitness).toBe(0.85);
    expect(receivedPayload!.grid['30,40'].features).toEqual([0.6, 0.8]);

    unlisten();
  });
});
