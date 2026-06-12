import { describe, it, expect } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { SimulationStatus } from '../mocks/mock_ipc_payloads';

describe('Tauri IPC Commands - Simulation Status', () => {
  it('F2: should successfully call get_simulation_status and retrieve status details', async () => {
    const status = await invoke<SimulationStatus>('get_simulation_status');

    expect(status).toBeDefined();
    expect(status.running).toBe(true);
    expect(status.tick_count).toBe(120);
    expect(status.avg_tick_time_ms).toBe(1.45);
    expect(status.fps).toBe(60.2);
  });

  it('F2_toggle: should toggle simulation running state', async () => {
    // Initial status should be running: true (reset by beforeEach)
    const initialStatus = await invoke<SimulationStatus>('get_simulation_status');
    expect(initialStatus.running).toBe(true);

    // Toggle once: should set to false and return false
    const toggleResult1 = await invoke<boolean>('toggle_simulation');
    expect(toggleResult1).toBe(false);

    // Get status again: should be running: false
    const toggledStatus = await invoke<SimulationStatus>('get_simulation_status');
    expect(toggledStatus.running).toBe(false);

    // Toggle again: should set to true and return true
    const toggleResult2 = await invoke<boolean>('toggle_simulation');
    expect(toggleResult2).toBe(true);

    // Get status again: should be running: true
    const doubleToggledStatus = await invoke<SimulationStatus>('get_simulation_status');
    expect(doubleToggledStatus.running).toBe(true);
  });

  it('F2_reset: should reset status running state to true in next test to avoid pollution', async () => {
    const status = await invoke<SimulationStatus>('get_simulation_status');
    expect(status.running).toBe(true);
  });
});
