import { vi, beforeEach } from 'vitest';
import type { InvokeArgs } from '@tauri-apps/api/core';
import { mockIPC } from '@tauri-apps/api/mocks';
import * as THREE from 'three';
import {
  mockSimulationStatus as originalStatus,
  mockMapElitesGridState as originalMapElitesGridState,
  EvolutionSettings,
  mockPheromoneGridState,
  mockRaycastTelemetry,
  mockLineageGraph,
  mockChronicleHistory,
  mockEnvironmentalState
} from './mocks/mock_ipc_payloads';
import { tickPayloadIsAdapted } from './mocks/tick-adaptation';

// Global canvas context mock setup.
//
// jsdom implements no canvas context at all, so the HUD widgets that draw with Canvas 2D — the
// compass ribbon and the minimap — would throw on `getContext('2d')`. This is the subset of
// `CanvasRenderingContext2D` they call; a component reaching for anything else fails loudly here
// rather than silently drawing nothing.
type MockCanvasContext = Pick<
  CanvasRenderingContext2D,
  | 'canvas'
  | 'clearRect'
  | 'beginPath'
  | 'arc'
  | 'fill'
  | 'stroke'
  | 'moveTo'
  | 'lineTo'
  | 'fillText'
  | 'rect'
  | 'fillRect'
  | 'strokeRect'
  | 'closePath'
  | 'fillStyle'
  | 'strokeStyle'
  | 'lineWidth'
  | 'font'
  | 'textAlign'
  | 'textBaseline'
>;

const mockContexts = new Map<HTMLCanvasElement, MockCanvasContext>();

/**
 * The one `getContext` overload this file replaces.
 *
 * `HTMLCanvasElement` is assignable to the view below — its `'2d'` overload returns a
 * `CanvasRenderingContext2D`, and that is a `MockCanvasContext` — so the line after it is an
 * ordinary widening the compiler checks, not a cast. Writing the mock onto `prototype` directly
 * needed one, because the DOM's `getContext` also has `'webgl'` and `'webgpu'` overloads that this
 * mock cannot answer and that a cast said nothing about; naming the `'2d'` signature is what makes
 * the returned stub checked against the members it claims.
 */
type Get2DContext = (this: HTMLCanvasElement, contextId: '2d') => MockCanvasContext | null;
const canvasProto: { getContext: Get2DContext } = HTMLCanvasElement.prototype;

canvasProto.getContext = vi.fn(function (this: HTMLCanvasElement, contextId: string) {
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
});

// The r3f reconciler creates DOM elements for three objects under jsdom, so the methods a
// three object would have get called on an `HTMLElement`. These stubs put them there.
//
// `HTMLElement.prototype` is typed with exactly the DOM's own members, so writing new ones onto it
// is not an assignment the DOM types describe. The choice is between claiming the prototype is
// something it is not and using the API that exists for adding a property to an object — and
// `Object.defineProperty` is that API. It is already how the two accessors further down are
// installed. Its descriptor would take any value at all, so `ThreeObjectStubs` is what keeps the
// value checked: a stub with the wrong arity is a compile error here rather than a silent no-op in
// a suite.

/** The three-object members the r3f reconciler calls on the DOM elements it creates under jsdom. */
interface ThreeObjectStubs {
  /** `OrbitControls.update()`. */
  update(): void;
  /** `BufferGeometry.setIndex()`, which returns the geometry for chaining. */
  setIndex(index: unknown): unknown;
  computeVertexNormals(): void;
  computeBoundingSphere(): void;
}

function defineThreeStub<K extends keyof ThreeObjectStubs>(
  name: K,
  value: ThreeObjectStubs[K],
): void {
  Object.defineProperty(HTMLElement.prototype, name, {
    value,
    configurable: true,
    writable: true,
  });
}

/** A three geometry with the one field these stubs record on it. */
interface CapturingGeometry {
  _capturedIndex?: unknown;
}

// Mock OrbitControls update method on HTMLElement to support JSDOM testing
defineThreeStub('update', vi.fn());

// Mock BufferGeometry methods and attributes on HTMLElement to support React Three Fiber under JSDOM
defineThreeStub(
  'setIndex',
  vi.fn(function (this: CapturingGeometry, index: unknown) {
    this._capturedIndex = index;
    return this;
  }),
);

defineThreeStub('computeVertexNormals', vi.fn());
defineThreeStub('computeBoundingSphere', vi.fn());

// Capture custom attributes set on elements (like BufferAttributes on bufferGeometry)
/** An element that lazily grows the attribute map the vegetation tests read back. */
interface CapturingElement {
  __capturedAttributes?: Map<string, unknown>;
  _capturedAttributes: Map<string, unknown>;
}

Object.defineProperty(HTMLElement.prototype, '_capturedAttributes', {
  get(this: CapturingElement) {
    if (!this.__capturedAttributes) {
      this.__capturedAttributes = new Map();
    }
    return this.__capturedAttributes;
  },
  configurable: true,
});

const originalSetAttribute = HTMLElement.prototype.setAttribute;
HTMLElement.prototype.setAttribute = vi.fn(function (
  this: HTMLElement & CapturingElement,
  name: string,
  value: unknown
) {
  if (value instanceof THREE.BufferAttribute) {
    this._capturedAttributes.set(name, value);
  } else {
    // The real `setAttribute` takes a string and coerces anything else, which is what the DOM does
    // to every value r3f passes through here. Saying so is what lets this call be checked.
    originalSetAttribute.call(this, name, String(value));
  }
});

/** The position attribute the precipitation systems write into each frame. */
interface MockPositionAttribute {
  array: Float32Array;
  needsUpdate: boolean;
}

/** The slice of `BufferGeometry` a particle system reaches for. */
interface MockGeometry {
  getAttribute(name: string): MockPositionAttribute | null;
}

/** An element standing in for a `Points`, lazily growing the geometry it is asked for. */
interface GeometryHolder {
  _mockGeometry?: MockGeometry;
  _mockPositionAttr?: MockPositionAttribute;
}

// Mock geometry getter (used by particle systems/points)
Object.defineProperty(HTMLElement.prototype, 'geometry', {
  get(this: GeometryHolder) {
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

/** A Tauri event as `@tauri-apps/api`'s `listen` delivers it. */
interface MockTauriEvent {
  event: string;
  payload: unknown;
}

/**
 * One named argument from an `invoke` call.
 *
 * `InvokeArgs` is a union — a record of named arguments, but also `number[]`, `ArrayBuffer` and
 * `Uint8Array` for the raw-payload form — so `args?.file_path` is not a property that exists on it.
 * Every command mocked below uses the record form; this is where that is established rather than
 * assumed.
 */
function invokeArg(args: InvokeArgs | undefined, name: string): unknown {
  if (!args || Array.isArray(args) || args instanceof ArrayBuffer || ArrayBuffer.isView(args)) {
    return undefined;
  }
  return args[name];
}

/** `value` seen as what a parsed IPC argument is: an object of unchecked fields. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

/**
 * `value` as evolution settings, or `null`.
 *
 * Checked rather than asserted, because the validation below is the whole behaviour this command
 * mocks: a caller that sends the wrong shape must reach "missing settings", not read `undefined`
 * out of a value the types promised had numbers in it.
 */
function asEvolutionSettings(value: unknown): EvolutionSettings | null {
  if (!isRecord(value)) return null;
  const { mutation_rate, selection_bias, grid_resolution } = value;
  if (
    typeof mutation_rate !== 'number' ||
    typeof selection_bias !== 'number' ||
    typeof grid_resolution !== 'number'
  ) {
    return null;
  }
  return { mutation_rate, selection_bias, grid_resolution };
}

// Global event bus listeners for testing IPC events
const listeners = new Map<string, Array<(event: MockTauriEvent) => void>>();

let mockSimulationStatus = { ...originalStatus };
let mockMapElitesGridState = JSON.parse(JSON.stringify(originalMapElitesGridState));
let mockEvolutionRunning = false;
let mockLineageState = { ...mockLineageGraph };
let mockChronicleState = [...mockChronicleHistory];

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (eventName: string, callback: (event: MockTauriEvent) => void) => {
    if (!listeners.has(eventName)) {
      listeners.set(eventName, []);
    }
    listeners.get(eventName)!.push(callback);

    return () => {
      const current = listeners.get(eventName) || [];
      listeners.set(eventName, current.filter(cb => cb !== callback));
    };
  }),
  emit: vi.fn(async (eventName: string, payload: unknown) => {
    const list = listeners.get(eventName) || [];
    list.forEach(callback => {
      let finalPayload = payload;
      if (isRecord(payload) && eventName === 'simulation-tick' && !Array.isArray(payload)) {
        // Some consumers subscribe to the whole tick payload and some to just its `segments`. The
        // callback's source is the only thing that distinguishes them here, which is ugly and is
        // what the tests were written against; `tick-adaptation.ts` is the switch for a test whose
        // subject is the whole-object shape.
        if (tickPayloadIsAdapted() && callback.toString().includes('segmentsRef.current')) {
          finalPayload = payload.segments;
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
        const settings = asEvolutionSettings(invokeArg(args, 'settings'));
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
        // The validated settings are deliberately not stored: no command in this mock reads them
        // back, so a variable holding them was written and never observed. The behaviour under
        // test is the validation above, which still throws.
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
        if (typeof invokeArg(args, 'file_path') !== 'string') {
          throw new Error("Missing or invalid file_path argument.");
        }
        return true;
      }
      case 'load_simulation_state': {
        if (typeof invokeArg(args, 'file_path') !== 'string') {
          throw new Error("Missing or invalid file_path argument.");
        }
        return true;
      }
      case 'get_environmental_elements':
        return mockEnvironmentalState;
      case 'get_terrain_map':
        return {
          width: 128,
          height: 128,
          elevations: new Array(128 * 128).fill(0.5),
          moistures: new Array(128 * 128).fill(0.5),
          biomes: new Array(128 * 128).fill(4), // Grassland = 4
          flows: new Array(128 * 128).fill(0.0),
        };
      case 'get_ecosystem_state':
        return {
          detritus: 100,
          plants: 500,
          animals: 300,
          total: 900,
          prey_count: 12,
          predator_count: 3,
          shannon: 0.5,
          simpson: 0.4,
          prey_mass: 6.0,
          predator_mass: 9.0,
          niche_divergence: 0.15,
          archive_coverage: 42,
        };
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
