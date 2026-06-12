import { vi, beforeEach } from 'vitest';
import { mockIPC } from '@tauri-apps/api/mocks';
import {
  mockSimulationStatus as originalStatus,
  mockEvolutionSettings as originalEvolutionSettings,
  mockMapElitesGridState as originalMapElitesGridState,
  EvolutionSettings,
  mockPheromoneGridState,
  mockRaycastTelemetry,
  mockLineageGraph,
  mockChronicleHistory,
  mockChronicleEvent,
  mockMigrationPayload
} from './mocks/mock_ipc_payloads';

// Global canvas context mock setup
const mockContexts = new Map<HTMLCanvasElement, any>();

HTMLCanvasElement.prototype.getContext = vi.fn().mockImplementation(function (this: HTMLCanvasElement, contextId: string) {
  if (contextId === '2d') {
    let ctx = mockContexts.get(this);
    if (!ctx) {
      ctx = {
        canvas: this,
        clearRect: vi.fn(),
        beginPath: vi.fn(),
        arc: vi.fn(),
        fill: vi.fn(),
        stroke: vi.fn(),
        moveTo: vi.fn(),
        lineTo: vi.fn(),
        fillText: vi.fn(),
        rect: vi.fn(),
        fillRect: vi.fn(),
        strokeRect: vi.fn(),
        closePath: vi.fn(),
        fillStyle: '',
        strokeStyle: '',
        lineWidth: 1,
        font: '',
        textAlign: 'left',
        textBaseline: 'alphabetic',
      };
      mockContexts.set(this, ctx);
    }
    return ctx;
  }
  return null;
}) as any;

// Global event bus listeners for testing IPC events
const listeners = new Map<string, Array<(event: any) => void>>();

let mockSimulationStatus = { ...originalStatus };
let mockEvolutionSettings = { ...originalEvolutionSettings };
let mockMapElitesGridState = JSON.parse(JSON.stringify(originalMapElitesGridState));
let mockEvolutionRunning = false;
let mockLineageState = { ...mockLineageGraph };
let mockChronicleState = [...mockChronicleHistory];

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (eventName: string, callback: (event: any) => void) => {
    if (!listeners.has(eventName)) {
      listeners.set(eventName, []);
    }
    listeners.get(eventName)!.push(callback);

    return () => {
      const current = listeners.get(eventName) || [];
      listeners.set(eventName, current.filter(cb => cb !== callback));
    };
  }),
  emit: vi.fn(async (eventName: string, payload: any) => {
    const list = listeners.get(eventName) || [];
    list.forEach(callback => callback({ event: eventName, payload }));
  }),
}));

beforeEach(() => {
  vi.clearAllMocks();
  listeners.clear();
  mockSimulationStatus = { ...originalStatus };
  mockEvolutionSettings = { ...originalEvolutionSettings };
  mockMapElitesGridState = JSON.parse(JSON.stringify(originalMapElitesGridState));
  mockEvolutionRunning = false;
  mockLineageState = { ...mockLineageGraph };
  mockChronicleState = [...mockChronicleHistory];

  mockIPC((cmd, args) => {
    switch (cmd) {
      case 'get_simulation_status':
        return mockSimulationStatus;
      case 'toggle_simulation':
        mockSimulationStatus.running = !mockSimulationStatus.running;
        return mockSimulationStatus.running;
      case 'get_map_elites_grid':
        return mockMapElitesGridState;
      case 'update_evolution_settings': {
        const settings = args?.settings as EvolutionSettings | undefined;
        if (!settings) {
          throw new Error("Missing settings argument.");
        }
        if (
          settings.mutation_rate < 0.0 ||
          settings.mutation_rate > 1.0 ||
          settings.selection_bias <= 0.0
        ) {
          throw new Error("Invalid settings: mutation_rate must be in [0.0, 1.0] and selection_bias must be positive.");
        }
        mockEvolutionSettings = { ...settings };
        return true;
      }
      case 'toggle_evolution':
        mockEvolutionRunning = !mockEvolutionRunning;
        return mockEvolutionRunning;
      case 'get_pheromone_grid':
        return mockPheromoneGridState;
      case 'get_active_raycasts':
        return mockRaycastTelemetry;
      case 'get_lineage_graph':
        return mockLineageState;
      case 'get_chronicle_history':
        return mockChronicleState;
      case 'plugin:event|listen':
        return 0;
      case 'plugin:event|emit':
        return;
      case 'plugin:event|unlisten':
        return;
      default:
        throw new Error(`Command ${cmd} is not supported in the mock environment.`);
    }
  });
});
