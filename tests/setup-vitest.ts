import { vi, beforeEach } from 'vitest';
import { mockIPC } from '@tauri-apps/api/mocks';
import * as THREE from 'three';
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
  mockMigrationPayload,
  mockEnvironmentalState
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

// Mock OrbitControls update method on HTMLElement to support JSDOM testing
(HTMLElement.prototype as any).update = vi.fn();

// Mock BufferGeometry methods and attributes on HTMLElement to support React Three Fiber under JSDOM
(HTMLElement.prototype as any).setIndex = vi.fn().mockImplementation(function (this: any, index: any) {
  this._capturedIndex = index;
  return this;
});

(HTMLElement.prototype as any).computeVertexNormals = vi.fn();

// Capture custom attributes set on elements (like BufferAttributes on bufferGeometry)
Object.defineProperty(HTMLElement.prototype, '_capturedAttributes', {
  get() {
    if (!this.__capturedAttributes) {
      this.__capturedAttributes = new Map();
    }
    return this.__capturedAttributes;
  },
  configurable: true,
});

const originalSetAttribute = HTMLElement.prototype.setAttribute;
HTMLElement.prototype.setAttribute = vi.fn().mockImplementation(function (
  this: any,
  name: string,
  value: any
) {
  if (value instanceof THREE.BufferAttribute) {
    this._capturedAttributes.set(name, value);
  } else {
    originalSetAttribute.call(this, name, value);
  }
});

// Mock geometry getter (used by particle systems/points)
Object.defineProperty(HTMLElement.prototype, 'geometry', {
  get() {
    if (!this._mockGeometry) {
      this._mockGeometry = {
        getAttribute: vi.fn().mockImplementation((name: string) => {
          if (name === 'position') {
            if (!this._mockPositionAttr) {
              this._mockPositionAttr = {
                array: new Float32Array(1000 * 3),
                needsUpdate: false,
              };
            }
            return this._mockPositionAttr;
          }
          return null;
        }),
      };
    }
    return this._mockGeometry;
  },
  configurable: true,
});



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
    list.forEach(callback => {
      let finalPayload = payload;
      if (eventName === 'simulation-tick' && payload && typeof payload === 'object' && !Array.isArray(payload)) {
        if (callback.toString().includes('segmentsRef.current') && !(globalThis as any).disableTickAdaptation) {
          finalPayload = (payload as any).segments;
        }
      }
      callback({ event: eventName, payload: finalPayload });
    });
  }),
}));

beforeEach(() => {
  window.requestAnimationFrame = vi.fn().mockReturnValue(0);
  window.cancelAnimationFrame = vi.fn();
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
      case 'save_simulation_state': {
        if (typeof args?.file_path !== 'string') {
          throw new Error("Missing or invalid file_path argument.");
        }
        return true;
      }
      case 'load_simulation_state': {
        if (typeof args?.file_path !== 'string') {
          throw new Error("Missing or invalid file_path argument.");
        }
        return true;
      }
      case 'get_environmental_elements':
        return mockEnvironmentalState;
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
