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
      case 'get_organism_presets':
        return [
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
        ];
      case 'spawn_preset_organism':
        return true;
      default:
        throw new Error(`Command ${cmd} is not supported in the mock environment.`);
    }
  });
});
