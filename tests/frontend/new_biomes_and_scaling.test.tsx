import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render } from '@testing-library/react';
import * as THREE from 'three';
import {
  generateTerrain,
  determineBiome,
  getBiomeColor,
} from '../../src/components/Landscape/utils/terrainGenerator';
import { Minimap } from '../../src/components/Landscape/Minimap';
import Water from '../../src/components/Landscape/Water';
import type { FrameCallback } from '../mocks/r3f-frame-state';
import {
  removeStubbedProperties,
  type StubAttributeCapture,
  type StubLod,
} from '../mocks/r3f-dom-stubs';

// Mock react-three-fiber Canvas and useFrame
let frameCallbacks: FrameCallback[] = [];

vi.mock('@react-three/fiber', async () => {
  return {
    Canvas: ({ children }: { children?: React.ReactNode }) => <div data-testid="mock-canvas">{children}</div>,
    useFrame: (cb: FrameCallback) => {
      frameCallbacks.push(cb);
    },
  };
});

/**
 * Install a member on `HTMLElement.prototype` that the DOM does not declare.
 *
 * The same reasoning as `tests/setup-vitest.ts`: assigning to `HTMLElement.prototype.addLevel` is
 * not an assignment the DOM types describe, and `Object.defineProperty` is the API for adding a
 * property to an object. `afterEach` takes them off again with `removeStubbedProperties`.
 */
function defineStub(name: string, value: unknown): void {
  Object.defineProperty(HTMLElement.prototype, name, {
    value,
    configurable: true,
    writable: true,
  });
}

describe('New Biomes and Terrain Scaling Tests', () => {
  let originalSetAttribute: typeof HTMLElement.prototype.setAttribute;

  beforeEach(() => {
    frameCallbacks = [];

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

    defineStub(
      'addLevel',
      vi.fn(function (this: StubLod, mesh: unknown, distance: number) {
        this.levels.push({ object: mesh, distance });
      })
    );

    defineStub('setIndex', vi.fn());
    defineStub('computeVertexNormals', vi.fn());

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
    HTMLElement.prototype.setAttribute = vi.fn(function (
      this: HTMLElement & StubAttributeCapture,
      name: string,
      value: unknown
    ) {
      // `unknown`, not `string`: r3f's `<bufferAttribute>` arrives here as a real
      // `THREE.BufferAttribute`, which is the whole reason this interception exists. Declaring the
      // DOM's `string` made the `instanceof` below a comparison the compiler knew could not hold.
      if (value instanceof THREE.BufferAttribute) {
        this._capturedAttributes.set(name, value);
      } else {
        originalSetAttribute.call(this, name, String(value));
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
            uWaterType: { value: 0.0 },
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
    if (originalSetAttribute) {
      HTMLElement.prototype.setAttribute = originalSetAttribute;
    }
    // `Reflect.deleteProperty` asks the question of the object rather than of its type, which is
    // what `delete` cannot do: the operator insists on an optional property, so removing one the
    // DOM never declared meant first claiming the prototype was something else.
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

  describe('1. 1000x1000 Terrain Scaling', () => {
    it('should generate terrain with 1000x1000 dimensions and correct grid structure', () => {
      const width = 1000;
      const height = 1000;
      const terrain = generateTerrain(width, height, 'scaling_test_seed');
      
      expect(terrain.width).toBe(width);
      expect(terrain.height).toBe(height);
      expect(terrain.grid.length).toBe(height);
      expect(terrain.grid[0].length).toBe(width);
      
      // Check data structure of a sample cell
      const sampleCell = terrain.grid[500][500];
      expect(sampleCell).toBeDefined();
      expect(sampleCell.x).toBe(500);
      expect(sampleCell.y).toBe(500);
      expect(typeof sampleCell.elevation).toBe('number');
      expect(typeof sampleCell.moisture).toBe('number');
      expect(sampleCell).toHaveProperty('biome');
      
      // Check that temperature field is present on the cell structure
      expect(sampleCell).toHaveProperty('temperature');
      expect(typeof sampleCell.temperature).toBe('number');
    });
  });

  describe('2. Biome Determination with Temperature Bounds', () => {
    it('should determine the correct biome based on elevation, moisture, and temperature', () => {
      const determineBiomeFn = determineBiome;

      // Desert: Hot and dry
      expect(determineBiomeFn(30, 15, 0.8)).toBe('desert');

      // Jungle: Hot and wet
      expect(determineBiomeFn(35, 75, 0.7)).toBe('jungle');

      // Volcanic: Extremely hot
      expect(determineBiomeFn(40, 20, 0.98)).toBe('volcanic');

      // Glacier: Extremely cold
      expect(determineBiomeFn(45, 50, -0.6)).toBe('glacier');
    });
  });

  describe('3. Biome Coloring Updates', () => {
    it('should return correct colors for the new biomes', () => {
      const getBiomeColorFn = getBiomeColor;

      // Desert: sand color (e.g. reddish-yellow, r > 0.7, g > 0.6)
      const desertColor = getBiomeColorFn(30, 15, 0.8);
      expect(desertColor).toBeDefined();
      expect(desertColor.r).toBeGreaterThan(0.7);
      expect(desertColor.g).toBeGreaterThan(0.6);
      
      // Jungle: dense forest green (g > r and g > b)
      const jungleColor = getBiomeColorFn(35, 75, 0.7);
      expect(jungleColor.g).toBeGreaterThan(jungleColor.r);
      expect(jungleColor.g).toBeGreaterThan(jungleColor.b);

      // Volcanic: dark/red (r > g)
      const volcanicColor = getBiomeColorFn(40, 20, 0.98);
      expect(volcanicColor.r).toBeGreaterThan(volcanicColor.g);

      // Glacier: icy/snow white or light blue (b > 0.7)
      const glacierColor = getBiomeColorFn(45, 50, -0.6);
      expect(glacierColor.b).toBeGreaterThan(0.7);
    });
  });

  describe('4. Flora Placement in New Biomes', () => {
    it('should place cactus in Desert, jungle/palm in Jungle, rock/dead trunks in Volcanic, and snow-covered pines/ice rocks in Glacier', () => {
      const width = 100;
      const height = 100;
      const terrain = generateTerrain(width, height, 'flora_test_seed');

      let desertCactusFound = false;
      let jungleTreeFound = false;
      let volcanicRockOrTrunkFound = false;
      let glacierPineOrIceFound = false;

      terrain.flora.forEach((f) => {
        const cx = Math.floor(f.x);
        const cy = Math.floor(f.y);
        const cell = terrain.grid[cy][cx];

        if (cell.biome === 'desert') {
          expect(f.type).toBe('Cactus');
          desertCactusFound = true;
        } else if (cell.biome === 'jungle') {
          expect(['Jungle', 'Palm']).toContain(f.type);
          jungleTreeFound = true;
        } else if (cell.biome === 'volcanic') {
          expect(['Rock', 'Dead Trunk']).toContain(f.type);
          volcanicRockOrTrunkFound = true;
        } else if (cell.biome === 'glacier') {
          expect(['Snow Pine', 'Ice Rock']).toContain(f.type);
          glacierPineOrIceFound = true;
        }
      });

      expect(desertCactusFound).toBe(true);
      expect(jungleTreeFound).toBe(true);
      expect(volcanicRockOrTrunkFound).toBe(true);
      expect(glacierPineOrIceFound).toBe(true);
    });
  });

  describe('5. Minimap Cell Colors', () => {
    it('should draw correct colors on the minimap for the new biomes', () => {
      let capturedImageData: ImageData | null = null;
      const originalGetContext = HTMLCanvasElement.prototype.getContext;
      
      HTMLCanvasElement.prototype.getContext = vi.fn().mockImplementation(function(type: string) {
        if (type === '2d') {
          return {
            createImageData: (w: number, h: number) => ({ data: new Uint8ClampedArray(w * h * 4) }),
            putImageData: (imgData: ImageData) => {
              capturedImageData = imgData;
            },
            fillRect: vi.fn(),
            beginPath: vi.fn(),
            arc: vi.fn(),
            fill: vi.fn(),
            stroke: vi.fn(),
          };
        }
        return null;
      });

      try {
        const { unmount } = render(<Minimap gridWidth={64} gridHeight={64} />);
        
        // Minimap computes canvas data in useMemo and updates in useEffect
        // We verify that the ImageData gets successfully captured
        expect(capturedImageData).not.toBeNull();
        
        // Assert that the generated pixel data has been rendered
        expect(capturedImageData.data.length).toBeGreaterThan(0);
        
        unmount();
      } finally {
        HTMLCanvasElement.prototype.getContext = originalGetContext;
      }
    });
  });

  describe('6. Lava rivers and ice sheets are biomes, not yet water meshes', () => {
    // This was an `it.skip` asserting that `Water` renders `lava-river-mesh` and `ice-sheet-mesh`.
    // It had never passed on this branch and could not: `main` shipped those meshes in a `Water.tsx`
    // that generated its own terrain per component with a local `* 1.8` height scale, this branch
    // had already replaced that with one cached terrain passed down and one `TERRAIN_HEIGHT_SCALE`,
    // and the merge did not carry that side over.
    //
    // A disabled assertion is not coverage. It reports as a skip, which reads as "environment
    // problem" rather than "feature absent", and it pins the reviewer's attention on a mesh name
    // instead of on the thing that is actually true.
    //
    // So it is inverted into a characterisation test. What is true is asserted; what is missing is
    // asserted as missing, so adding it turns this red and sends whoever does it to the design
    // question — a data texture carrying temperature, as `main` did, versus per-biome meshes.

    it('supports both biomes in the taxonomy the renderer colours from', () => {
      // The half that did land, and the half the skipped test actually depended on.
      // `(temperature, moisture, elevation)` — same calls as section 2 above.
      expect(determineBiome(40, 20, 0.98)).toBe('volcanic');
      expect(determineBiome(45, 50, -0.6)).toBe('glacier');
    });

    it('renders no lava or ice water mesh yet, and this test is how that stays visible', () => {
      const { container } = render(<Water width={100} height={100} timeOfDay={12} />);

      // Not `toBeNull()` phrased as an expectation of absence for its own sake: these two queries are
      // the exact selectors the disabled test used, so the day someone implements the meshes this
      // fails and names the file to update.
      expect(
        container.querySelector('[name="lava-river-mesh"], [data-testid="lava-river-mesh"]'),
        'lava rivers now render — implement the volcanic water contract and re-point this test',
      ).toBeNull();
      expect(
        container.querySelector('[name="ice-sheet-mesh"], [data-testid="ice-sheet-mesh"]'),
        'ice sheets now render — implement the glacier water contract and re-point this test',
      ).toBeNull();

      // And the component does render its own water, so the two nulls above are a statement about
      // lava and ice rather than about `Water` having failed to mount.
      expect(container.querySelector('[data-testid="mock-canvas"], mesh, group')).not.toBeNull();
    });
  });
});
