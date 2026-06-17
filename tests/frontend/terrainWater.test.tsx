import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, act, screen } from '@testing-library/react';
import * as THREE from 'three';
import Terrain from '../../src/components/Landscape/Terrain';
import Water from '../../src/components/Landscape/Water';
import { generateTerrain } from '../../src/components/Landscape/utils/terrainGenerator';

let originalSetAttribute: any;
let frameCallbacks: Array<(state: any, delta: number) => void> = [];

vi.mock('@react-three/fiber', async () => {
  return {
    Canvas: ({ children }: any) => <div data-testid="mock-canvas">{children}</div>,
    useFrame: (cb: any) => {
      frameCallbacks.push(cb);
    },
  };
});

describe('Terrain and Water Component Tests', () => {
  let consoleErrorSpy: any;

  beforeEach(() => {
    frameCallbacks = [];
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    // Override screen.getByTestId to look for elements with [name="..."] attribute in JSDOM
    screen.getByTestId = (id: string) => {
      const el = document.querySelector(`[name="${id}"]`);
      if (!el) {
        throw new Error(`Unable to find element with name="${id}"`);
      }
      return el as any;
    };

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

    HTMLElement.prototype.addLevel = vi.fn().mockImplementation(function (
      this: any,
      mesh: any,
      distance: number
    ) {
      this.levels.push({ object: mesh, distance });
    });

    // Mock BufferGeometry methods
    HTMLElement.prototype.setIndex = vi.fn();
    HTMLElement.prototype.computeVertexNormals = vi.fn();

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
    delete (HTMLElement.prototype as any).levels;
    delete (HTMLElement.prototype as any).addLevel;
    delete (HTMLElement.prototype as any).setIndex;
    delete (HTMLElement.prototype as any).computeVertexNormals;
    delete (HTMLElement.prototype as any)._capturedAttributes;
    delete (HTMLElement.prototype as any).uniforms;
    delete (HTMLElement.prototype as any).geometry;
  });

  describe('Terrain Component', () => {
    it('should render the LOD component and high/medium/low detail meshes', () => {
      const { container } = render(<Terrain width={64} height={64} />);

      // Find the LOD element
      const lodEl = container.querySelector('lod');
      expect(lodEl).not.toBeNull();

      // Verify LOD levels were registered
      expect((lodEl as any).levels.length).toBe(3);

      // Verify the detail meshes exist by name
      const highMesh = screen.getByTestId('terrain-mesh');
      const medMesh = screen.getByTestId('terrain-mesh-lod-1');
      const lowMesh = screen.getByTestId('terrain-mesh-lod-2');

      expect(highMesh).toBeDefined();
      expect(medMesh).toBeDefined();
      expect(lowMesh).toBeDefined();
    });

    it('should initialize geometry with heights deformed correctly based on elevation data', () => {
      const width = 64;
      const height = 64;
      const { container } = render(<Terrain width={width} height={height} />);

      // Retrieve high-detail bufferGeometry
      const highMesh = screen.getByTestId('terrain-mesh');
      const geomEl = highMesh.querySelector('buffergeometry');
      expect(geomEl).not.toBeNull();

      const posAttr = (geomEl as any)._capturedAttributes.get('position');
      const colorAttr = (geomEl as any)._capturedAttributes.get('color');

      expect(posAttr).toBeDefined();
      expect(colorAttr).toBeDefined();

      const positions = posAttr.array;
      const colors = colorAttr.array;

      // Height logic check: y coordinate is index i*3 + 1
      // Let's verify coordinates for (x=0, y=0) which is index 0
      const terrainData = generateTerrain(width, height, 'seed');
      const cell0 = terrainData.grid[0][0];
      const expectedZ0 = cell0.elevation * 1.8;

      expect(positions[1]).toBeCloseTo(expectedZ0, 4);

      // Check another coordinate, e.g. x=10, y=5
      const gx = 10;
      const gy = 5;
      const cellIndex = gy * width + gx;
      const cell = terrainData.grid[gy][gx];
      const expectedZ = cell.elevation * 1.8;

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

      const waterMesh = screen.getByTestId('water-mesh') as any;
      expect(waterMesh).toBeDefined();

      // Check DOM properties mapping (using user-facing data-attributes)
      expect(waterMesh.getAttribute('data-wind-speed')).toBe(String(windSpeed));
      expect(waterMesh.getAttribute('data-reflection-color')).toBe(reflectionColor);
      expect(waterMesh.getAttribute('data-depth-transparency')).toBe(String(depthTransparency));
    });

    it('should update the shader material time uniform in the rendering frame loop', () => {
      render(<Water width={64} height={64} />);

      const shaderEl = document.querySelector('shadermaterial') as any;
      expect(shaderEl).not.toBeNull();
      expect(shaderEl.uniforms).toBeDefined();

      // Simulate clock advancement
      const mockState = {
        clock: {
          getElapsedTime: () => 15.42,
        },
      };

      act(() => {
        frameCallbacks.forEach((cb) => cb(mockState, 0.016));
      });

      expect(shaderEl.uniforms.time.value).toBe(15.42);
    });

    it('should update the positions of the waterfall particles over time', () => {
      // Use a seed/dimension that has waterfalls
      render(<Water width={200} height={200} />);

      // Verify waterfall-particles points exist
      const particlesEl = screen.getByTestId('waterfall-particles') as any;
      expect(particlesEl).toBeDefined();

      const geom = particlesEl.geometry;
      const posAttr = geom.getAttribute('position');
      const posArray = posAttr.array;

      // Record initial particle Y coordinates (every 3rd element starting from index 1)
      const initialYs = Array.from(posArray.filter((_, idx) => idx % 3 === 1));

      // Advance frame
      const mockState = { clock: { getElapsedTime: () => 1.0 } };
      act(() => {
        frameCallbacks.forEach((cb) => cb(mockState, 0.05)); // 50ms delta
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
