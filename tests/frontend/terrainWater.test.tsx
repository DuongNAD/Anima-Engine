import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, act } from '@testing-library/react';
import * as THREE from 'three';
import Terrain from '../../src/components/Landscape/Terrain';
import Water from '../../src/components/Landscape/Water';
import { generateTerrain, TERRAIN_HEIGHT_SCALE } from '../../src/components/Landscape/utils/terrainGenerator';
import { frameStateAt, type FrameCallback } from '../mocks/r3f-frame-state';
import {
  getByName,
  installStubbedProperty,
  removeStubbedProperties,
  type StubAttributeCapture,
  type StubGeometryHolder,
  type StubLod,
  type StubUniforms,
} from '../mocks/r3f-dom-stubs';

let originalSetAttribute: typeof HTMLElement.prototype.setAttribute | undefined;
let frameCallbacks: FrameCallback[] = [];

vi.mock('@react-three/fiber', async () => {
  return {
    Canvas: ({ children }: { children?: React.ReactNode }) => <div data-testid="mock-canvas">{children}</div>,
    useFrame: (cb: FrameCallback) => {
      frameCallbacks.push(cb);
    },
    useThree: () => ({
      scene: { fog: null, add: vi.fn(), remove: vi.fn() },
      camera: { position: { set: vi.fn() }, lookAt: vi.fn() },
      gl: { setSize: vi.fn(), domElement: document.createElement('canvas') },
    }),
    extend: vi.fn(),
  };
});

describe('Terrain and Water Component Tests', () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    frameCallbacks = [];
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    // Elements are found by their `name` attribute — `getByName` in `tests/mocks/r3f-dom-stubs.ts`
    // says why, and what overwriting `screen.getByTestId` here used to cost.

    // Mock LOD properties & methods on HTMLElement
    Object.defineProperty(HTMLElement.prototype, 'levels', {
      get() {
        if (!this._mockLevels) {
          this._mockLevels = [];
        }
        return this._mockLevels;
      },
      set(val) {
        this._mockLevels = val;
      },
      configurable: true,
    });

    installStubbedProperty(
      HTMLElement.prototype,
      'addLevel',
      vi.fn(function (this: HTMLElement & StubLod, mesh: unknown, distance: number) {
        this.levels.push({ object: mesh, distance });
      })
    );

    // Mock BufferGeometry methods
    installStubbedProperty(HTMLElement.prototype, 'setIndex', vi.fn());
    installStubbedProperty(HTMLElement.prototype, 'computeVertexNormals', vi.fn());

    // Capture custom attributes set on elements
    Object.defineProperty(HTMLElement.prototype, '_capturedAttributes', {
      get() {
        if (!this.__capturedAttributes) {
          this.__capturedAttributes = new Map();
        }
        return this.__capturedAttributes;
      },
      configurable: true,
    });

    originalSetAttribute = HTMLElement.prototype.setAttribute;
    HTMLElement.prototype.setAttribute = vi.fn().mockImplementation(function (
      this: HTMLElement & StubAttributeCapture,
      name: string,
      value: string | THREE.BufferAttribute
    ) {
      if (value instanceof THREE.BufferAttribute) {
        this._capturedAttributes.set(name, value);
      } else {
        originalSetAttribute?.call(this, name, value);
      }
    });

    // Mock uniforms for ShaderMaterial
    Object.defineProperty(HTMLElement.prototype, 'uniforms', {
      get() {
        if (!this._mockUniforms) {
          this._mockUniforms = {
            time: { value: 0 },
            windSpeed: { value: 1.0 },
            reflectionColor: { value: new THREE.Color('#0055ff') },
            depthTransparency: { value: 0.8 },
          };
        }
        return this._mockUniforms;
      },
      configurable: true,
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
  });

  afterEach(() => {
    consoleErrorSpy.mockRestore();
    if (originalSetAttribute) {
      HTMLElement.prototype.setAttribute = originalSetAttribute;
    }
    removeStubbedProperties(
      HTMLElement.prototype,
      'levels',
      'addLevel',
      'setIndex',
      'computeVertexNormals',
      '_capturedAttributes',
      'uniforms',
      'geometry',
    );
  });

  describe('Terrain Component', () => {
    it('should render the LOD component and high/medium/low detail meshes', () => {
      const { container } = render(<Terrain width={64} height={64} />);

      // Find the LOD element
      const lodEl = container.querySelector('lod');
      expect(lodEl).not.toBeNull();

      // Verify LOD levels were registered
      expect((lodEl as Element & StubLod).levels.length).toBe(3);

      // Verify the detail meshes exist by name
      const highMesh = getByName('terrain-mesh');
      const medMesh = getByName('terrain-mesh-lod-1');
      const lowMesh = getByName('terrain-mesh-lod-2');

      expect(highMesh).toBeDefined();
      expect(medMesh).toBeDefined();
      expect(lowMesh).toBeDefined();
    });

    it('should initialize geometry with heights deformed correctly based on elevation data', () => {
      const width = 64;
      const height = 64;
      render(<Terrain width={width} height={height} />);

      // Retrieve high-detail bufferGeometry
      const highMesh = getByName('terrain-mesh');
      const geomEl = highMesh.querySelector('buffergeometry');
      expect(geomEl).not.toBeNull();

      const captured = (geomEl as Element & StubAttributeCapture)._capturedAttributes;
      const posAttr = captured.get('position');
      const colorAttr = captured.get('color');

      expect(posAttr).toBeDefined();
      expect(colorAttr).toBeDefined();

      // A guard, not an assertion: `Map.get` answers `undefined` for a key it does not hold, and the
      // rest of this test indexes into the array. Throwing here names what went missing.
      if (!posAttr) throw new Error('the terrain geometry published no `position` attribute');
      const positions = posAttr.array;

      // Height logic check: y coordinate is index i*3 + 1
      // Let's verify coordinates for (x=0, y=0) which is index 0
      const terrainData = generateTerrain(width, height, 'seed');
      const cell0 = terrainData.grid[0][0];
      const expectedZ0 = cell0.elevation * TERRAIN_HEIGHT_SCALE;

      expect(positions[1]).toBeCloseTo(expectedZ0, 4);

      // Check another coordinate, e.g. x=10, y=5
      const gx = 10;
      const gy = 5;
      const cellIndex = gy * width + gx;
      const cell = terrainData.grid[gy][gx];
      const expectedZ = cell.elevation * TERRAIN_HEIGHT_SCALE;

      expect(positions[cellIndex * 3 + 1]).toBeCloseTo(expectedZ, 4);
    });
  });

  describe('Water Component', () => {
    it('should render the water mesh with proper properties and custom geometry', () => {
      const reflectionColor = '#00ffaa';
      const windSpeed = 2.5;
      const depthTransparency = 0.6;

      render(
        <Water
          width={64}
          height={64}
          reflectionColor={reflectionColor}
          windSpeed={windSpeed}
          depthTransparency={depthTransparency}
        />
      );

      const waterMesh = getByName('water-mesh');
      expect(waterMesh).toBeDefined();

      // Check DOM properties mapping (using user-facing data-attributes)
      expect(waterMesh.getAttribute('data-wind-speed')).toBe(String(windSpeed));
      expect(waterMesh.getAttribute('data-reflection-color')).toBe(reflectionColor);
      expect(waterMesh.getAttribute('data-depth-transparency')).toBe(String(depthTransparency));
    });

    it('should update the shader material time uniform in the rendering frame loop', () => {
      render(<Water width={64} height={64} />);

      const shaderEl = document.querySelector('shadermaterial') as (Element & StubUniforms) | null;
      expect(shaderEl).not.toBeNull();
      expect(shaderEl?.uniforms).toBeDefined();

      act(() => {
        frameCallbacks.forEach((cb) => cb(frameStateAt(15.42), 0.016));
      });

      expect(shaderEl?.uniforms.time.value).toBe(15.42);
    });

    it('should update the positions of the waterfall particles over time', () => {
      // Use a seed/dimension that has waterfalls
      render(<Water width={200} height={200} />);

      // Verify waterfall-particles points exist
      const particlesEl = getByName('waterfall-particles') as HTMLElement & StubGeometryHolder;
      expect(particlesEl).toBeDefined();

      const posAttr = particlesEl.geometry.getAttribute('position');
      const posArray = posAttr?.array ?? new Float32Array();

      // Record initial particle Y coordinates (every 3rd element starting from index 1)
      const initialYs = Array.from(posArray.filter((_, idx) => idx % 3 === 1));

      // Advance frame
      act(() => {
        frameCallbacks.forEach((cb) => cb(frameStateAt(1.0), 0.05)); // 50ms delta
      });

      const updatedYs = Array.from(posArray.filter((_, idx) => idx % 3 === 1));

      // Check that at least some waterfall particle Y coordinates changed (fell down)
      let changedCount = 0;
      for (let i = 0; i < initialYs.length; i++) {
        if (Math.abs(updatedYs[i] - initialYs[i]) > 0.001) {
          changedCount++;
        }
      }

      expect(changedCount).toBeGreaterThan(0);
    });
  });
});
