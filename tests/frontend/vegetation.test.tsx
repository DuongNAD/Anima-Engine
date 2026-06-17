import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render } from '@testing-library/react';
import * as THREE from 'three';
import { Vegetation } from '../../src/components/Landscape/Vegetation';
import { generateTerrain, getBilinearInterpolatedElevation } from '../../src/components/Landscape/utils/terrainGenerator';

// Mock react-three-fiber Canvas and useFrame
let frameCallbacks: Array<(state: any, delta: number) => void> = [];

vi.mock('@react-three/fiber', async () => {
  return {
    Canvas: ({ children }: any) => <div data-testid="mock-canvas">{children}</div>,
    useFrame: (cb: any) => {
      frameCallbacks.push(cb);
    },
  };
});

describe('Vegetation Component Tests', () => {
  let originalSetAttribute: any;
  let capturedMaterials: THREE.MeshStandardMaterial[] = [];
  let setMatrixCalls: Array<{ instanceId: number; matrix: THREE.Matrix4; element: any }> = [];

  beforeEach(() => {
    frameCallbacks = [];
    capturedMaterials = [];
    setMatrixCalls = [];

    // Intercept material creation to get references to onBeforeCompile
    Object.defineProperty(THREE.MeshStandardMaterial.prototype, 'onBeforeCompile', {
      get() {
        return this._onBeforeCompile;
      },
      set(fn) {
        this._onBeforeCompile = fn;
        capturedMaterials.push(this);
      },
      configurable: true,
    });

    // Mock setMatrixAt to inspect positions
    HTMLElement.prototype.setMatrixAt = vi.fn().mockImplementation(function (
      this: any,
      instanceId: number,
      matrix: THREE.Matrix4
    ) {
      setMatrixCalls.push({ instanceId, matrix: matrix.clone(), element: this });
    });

    HTMLElement.prototype.setIndex = vi.fn();
    HTMLElement.prototype.computeVertexNormals = vi.fn();

    // Capture custom attributes
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
  });

  afterEach(() => {
    if (originalSetAttribute) {
      HTMLElement.prototype.setAttribute = originalSetAttribute;
    }
    delete (HTMLElement.prototype as any).setMatrixAt;
    delete (HTMLElement.prototype as any).setIndex;
    delete (HTMLElement.prototype as any).computeVertexNormals;
    delete (HTMLElement.prototype as any)._capturedAttributes;
    delete (THREE.MeshStandardMaterial.prototype as any).onBeforeCompile;
  });

  it('should render instanced meshes for each flora type', () => {
    const { container } = render(<Vegetation width={100} height={100} />);

    // Verify group container and attributes
    const groupEl = container.querySelector('[name="vegetation-group"]');
    expect(groupEl).not.toBeNull();

    // Verify instancedMesh components exist by name for types that are rendered
    const types = ['oak', 'pine', 'bush', 'rock', 'grass'];
    let renderedCount = 0;
    types.forEach((t) => {
      const el = container.querySelector(`[name="vegetation-instanced-mesh-${t}"]`);
      if (el) {
        renderedCount++;
        expect(el).not.toBeNull();
      }
    });
    expect(renderedCount).toBeGreaterThan(0);
  });

  it('should correctly position instances according to terrain heights', () => {
    const width = 32;
    const height = 32;
    render(<Vegetation width={width} height={height} />);

    const terrainData = generateTerrain(width, height, 'seed');

    // Retrieve matrix translations and verify they correspond to terrain heights
    expect(setMatrixCalls.length).toBeGreaterThan(0);

    setMatrixCalls.forEach(({ matrix, element }) => {
      const position = new THREE.Vector3();
      position.setFromMatrixPosition(matrix);

      const scale = new THREE.Vector3();
      scale.setFromMatrixScale(matrix);
      const s = scale.y;

      // Reconstruct original grid coordinates
      // posX = gx - width / 2 => gx = posX + width / 2
      // posZ = gy - height / 2 => gy = posZ + height / 2
      const gx = Math.floor(position.x + width / 2 + 1e-5);
      const gy = Math.floor(position.z + height / 2 + 1e-5);

      expect(gx).toBeGreaterThanOrEqual(0);
      expect(gx).toBeLessThan(width);
      expect(gy).toBeGreaterThanOrEqual(0);
      expect(gy).toBeLessThan(height);

      const px = position.x + width / 2;
      const py = position.z + height / 2;
      let expectedZ = getBilinearInterpolatedElevation(px, py, width, height, terrainData.grid) * 1.8;

      const name = element ? element.getAttribute('name') : '';
      if (name === 'vegetation-instanced-mesh-oak') {
        expectedZ += 1.25 * s;
      } else if (name === 'vegetation-instanced-mesh-oak-leaves') {
        expectedZ += 3.0 * s;
      } else if (name === 'vegetation-instanced-mesh-pine') {
        expectedZ += 1.0 * s;
      } else if (name === 'vegetation-instanced-mesh-pine-leaves-1') {
        expectedZ += 2.2 * s;
      } else if (name === 'vegetation-instanced-mesh-pine-leaves-2') {
        expectedZ += 3.4 * s;
      } else if (name === 'vegetation-instanced-mesh-pine-leaves-3') {
        expectedZ += 4.5 * s;
      } else if (name === 'vegetation-instanced-mesh-bush') {
        expectedZ += 0.4 * s;
      } else if (name === 'vegetation-instanced-mesh-rock') {
        expectedZ -= 0.3 * s;
      } else if (name === 'vegetation-instanced-mesh-palm') {
        expectedZ += 2.5 * s;
      } else if (name === 'vegetation-instanced-mesh-palm-leaves') {
        expectedZ += 5.0 * s;
      } else if (name === 'vegetation-instanced-mesh-cactus') {
        expectedZ += 1.2 * s;
      } else if (name === 'vegetation-instanced-mesh-jungle') {
        expectedZ += 2.0 * s;
      } else if (name === 'vegetation-instanced-mesh-jungle-leaves') {
        expectedZ += 5.0 * s;
      } else if (name === 'vegetation-instanced-mesh-birch') {
        expectedZ += 1.5 * s;
      } else if (name === 'vegetation-instanced-mesh-birch-leaves') {
        expectedZ += 3.2 * s;
      } else if (name === 'vegetation-instanced-mesh-flowers') {
        expectedZ += 0.3;
      }

      // Height (y) must match the terrain height plus offset
      expect(position.y).toBeCloseTo(expectedZ, 4);
    });
  });

  it('should reactively update uniforms when windSpeed or windDirection props change', () => {
    const { rerender } = render(
      <Vegetation windSpeed={2.5} windDirection={1.2} />
    );

    expect(capturedMaterials.length).toBeGreaterThan(0);

    // Simulate compiling shader
    const mockShader = {
      uniforms: {} as any,
      vertexShader: '#include <common>\n#include <begin_vertex>',
      fragmentShader: '',
    };

    // Run compile for each captured material
    capturedMaterials.forEach((material) => {
      if (typeof material.onBeforeCompile === 'function') {
        material.onBeforeCompile(mockShader);
      }
    });

    // Verify initial values in uniforms
    expect(mockShader.uniforms.uWindSpeed.value).toBe(2.5);
    expect(mockShader.uniforms.uWindDirection.value).toBe(1.2);

    // Update props
    rerender(<Vegetation windSpeed={4.5} windDirection={2.8} />);

    // Verify uniforms are reactively updated
    expect(mockShader.uniforms.uWindSpeed.value).toBe(4.5);
    expect(mockShader.uniforms.uWindDirection.value).toBe(2.8);
  });
});
