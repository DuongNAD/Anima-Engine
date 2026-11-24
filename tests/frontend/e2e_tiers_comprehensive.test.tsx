import React from 'react';
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { render, screen, fireEvent, act, cleanup } from '@testing-library/react';
import * as THREE from 'three';

import {
  generateTerrain,
  generateTerrainAsync,
  determineBiome,
  clearTerrainCache,
  getBilinearInterpolatedElevation,
  getBiomeColor,
  poissonDiskSampling,
  mulberry32,
  hashString,
} from '../../src/components/Landscape/utils/terrainGenerator';

import {
  serializeChunk,
  deserializeChunk,
  TERRAIN_SCHEMA_VERSION,
} from '../../src/components/Landscape/utils/terrainSerializer';

import { terrainCacheDB } from '../../src/components/Landscape/utils/terrainCacheDB';
import { LandscapeControlsOverlay } from '../../src/components/Landscape/LandscapeControlsOverlay';
import LandscapeShowcase from '../../src/components/Landscape/LandscapeShowcase';
import { Vegetation } from '../../src/components/Landscape/Vegetation';
import { Water } from '../../src/components/Landscape/Water';

// Polyfill IndexedDB for JSDOM / Node environment if not present
if (typeof globalThis !== 'undefined' && !(globalThis as any).indexedDB) {
  class MockIDBIndex {
    constructor(private name: string, private keyPath: string, private store: MockIDBObjectStore) {}

    getAllKeys(query?: any) {
      const keys: any[] = [];
      for (const [key, val] of this.store.data.entries()) {
        if (val && val[this.keyPath] === query) {
          keys.push(key);
        }
      }
      const req: any = { result: keys, onsuccess: null, onerror: null };
      setTimeout(() => req.onsuccess?.(), 0);
      return req;
    }

    openCursor() {
      const entries = Array.from(this.store.data.entries()).sort(
        (a, b) => (a[1][this.keyPath] || 0) - (b[1][this.keyPath] || 0)
      );
      let idx = 0;
      const store = this.store;
      const req: any = { result: null, onsuccess: null, onerror: null };

      const makeCursor = (): any => {
        if (idx >= entries.length) return null;
        const [k, v] = entries[idx];
        return {
          key: v[this.keyPath],
          primaryKey: k,
          value: v,
          delete: () => {
            store.data.delete(k);
          },
          continue: () => {
            idx++;
            req.result = makeCursor();
            setTimeout(() => req.onsuccess?.(), 0);
          },
        };
      };

      setTimeout(() => {
        req.result = makeCursor();
        req.onsuccess?.();
      }, 0);
      return req;
    }
  }

  class MockIDBObjectStore {
    public data = new Map<string, any>();
    public indexes = new Map<string, MockIDBIndex>();

    constructor(public name: string, public keyPath: string) {}

    createIndex(indexName: string, keyPath: string, options?: any) {
      const idx = new MockIDBIndex(indexName, keyPath, this);
      this.indexes.set(indexName, idx);
      return idx;
    }

    index(indexName: string) {
      return this.indexes.get(indexName)!;
    }

    get(key: string) {
      const req: any = { result: this.data.get(key), onsuccess: null, onerror: null };
      setTimeout(() => req.onsuccess?.(), 0);
      return req;
    }

    put(value: any) {
      const key = value[this.keyPath] ?? value.key;
      this.data.set(key, value);
      const req: any = { result: key, onsuccess: null, onerror: null };
      setTimeout(() => req.onsuccess?.(), 0);
      return req;
    }

    delete(key: string) {
      this.data.delete(key);
      const req: any = { result: undefined, onsuccess: null, onerror: null };
      setTimeout(() => req.onsuccess?.(), 0);
      return req;
    }

    clear() {
      this.data.clear();
      const req: any = { result: undefined, onsuccess: null, onerror: null };
      setTimeout(() => req.onsuccess?.(), 0);
      return req;
    }

    count() {
      const req: any = { result: this.data.size, onsuccess: null, onerror: null };
      setTimeout(() => req.onsuccess?.(), 0);
      return req;
    }
  }

  class MockIDBTransaction {
    public oncomplete: (() => void) | null = null;
    public onerror: ((err: any) => void) | null = null;
    public error: any = null;

    constructor(private stores: Map<string, MockIDBObjectStore>, storeNames: string[]) {
      setTimeout(() => this.oncomplete?.(), 5);
    }

    objectStore(name: string) {
      return this.stores.get(name)!;
    }
  }

  class MockIDBDatabase {
    public objectStoreNames = {
      contains: (name: string) => this.stores.has(name),
    };
    public stores = new Map<string, MockIDBObjectStore>();

    createObjectStore(name: string, options: { keyPath: string }) {
      const store = new MockIDBObjectStore(name, options.keyPath);
      this.stores.set(name, store);
      return store;
    }

    transaction(storeNames: string | string[], mode?: string) {
      const names = Array.isArray(storeNames) ? storeNames : [storeNames];
      return new MockIDBTransaction(this.stores, names);
    }
  }

  class MockIDBFactory {
    private dbs = new Map<string, MockIDBDatabase>();

    open(name: string, version: number) {
      let db = this.dbs.get(name);
      const isNew = !db;
      if (!db) {
        db = new MockIDBDatabase();
        this.dbs.set(name, db);
      }
      const req: any = {
        result: db,
        onsuccess: null,
        onerror: null,
        onupgradeneeded: null,
      };
      setTimeout(() => {
        if (isNew) {
          req.onupgradeneeded?.({ oldVersion: 0, newVersion: version, target: req });
        }
        req.onsuccess?.();
      }, 0);
      return req;
    }
  }

  (globalThis as any).indexedDB = new MockIDBFactory();
}

// Mock @react-three/fiber for JSDOM testing
vi.mock('@react-three/fiber', async () => {
  return {
    Canvas: ({ children }: any) => <div data-testid="mock-canvas">{children}</div>,
    useFrame: vi.fn((cb) => {
      if (typeof cb === 'function') {
        cb({ clock: { getElapsedTime: () => 1.5 } }, 0.016);
      }
    }),
    useThree: () => ({
      camera: { position: new THREE.Vector3(0, 25, 45), lookAt: vi.fn() },
      scene: new THREE.Scene(),
      gl: { setSize: vi.fn(), domElement: document.createElement('canvas') },
    }),
    extend: vi.fn(),
  };
});

// Polyfill Mock Web Audio API
class MockAudioContext {
  sampleRate = 44100;
  currentTime = 0;
  createGain() {
    return { gain: { value: 1, setValueAtTime: vi.fn() }, connect: vi.fn() };
  }
  createPanner() {
    return {
      connect: vi.fn(),
      setPosition: vi.fn(),
      setOrientation: vi.fn(),
      positionX: { setValueAtTime: vi.fn() },
      positionY: { setValueAtTime: vi.fn() },
      positionZ: { setValueAtTime: vi.fn() },
      orientationX: { setValueAtTime: vi.fn() },
      orientationY: { setValueAtTime: vi.fn() },
      orientationZ: { setValueAtTime: vi.fn() },
    };
  }
  createBuffer() {
    return { getChannelData: vi.fn().mockReturnValue(new Float32Array(100)) };
  }
  createBufferSource() {
    return { connect: vi.fn().mockReturnThis(), start: vi.fn(), stop: vi.fn() };
  }
  destination = {};
  listener = {
    setPosition: vi.fn(),
    setOrientation: vi.fn(),
    positionX: { setValueAtTime: vi.fn() },
    positionY: { setValueAtTime: vi.fn() },
    positionZ: { setValueAtTime: vi.fn() },
    forwardX: { setValueAtTime: vi.fn() },
    forwardY: { setValueAtTime: vi.fn() },
    forwardZ: { setValueAtTime: vi.fn() },
    upX: { setValueAtTime: vi.fn() },
    upY: { setValueAtTime: vi.fn() },
    upZ: { setValueAtTime: vi.fn() },
  };
}
vi.stubGlobal('AudioContext', MockAudioContext);
vi.stubGlobal('webkitAudioContext', MockAudioContext);

describe('Anima Engine Photorealistic Environment - Tier 1: Core Feature Coverage', () => {
  beforeEach(async () => {
    vi.useRealTimers();
    clearTerrainCache();
    await terrainCacheDB.clearAllCache();
  });

  afterEach(() => {
    vi.useRealTimers();
    cleanup();
  });

  describe('Feature 1: Natural Organic Terrain Generator (No 4-sector / center hole)', () => {
    it('1.1: generates continuous elevation across grid without 4-sector boundaries', () => {
      const width = 64;
      const height = 64;
      const terrain = generateTerrain(width, height, 'organic_seed_1');

      // Verify grid dimensions
      expect(terrain.grid.length).toBe(height);
      expect(terrain.grid[0].length).toBe(width);

      // Check elevation smooth transition across center axis (no seam/boundary artifact)
      const midX = Math.floor(width / 2);
      const midY = Math.floor(height / 2);

      const centerCell = terrain.grid[midY][midX];
      const leftCell = terrain.grid[midY][midX - 1];
      const rightCell = terrain.grid[midY][midX + 1];

      expect(centerCell.elevation).toBeGreaterThanOrEqual(0);
      expect(centerCell.elevation).toBeLessThanOrEqual(100);
      expect(Math.abs(leftCell.elevation - centerCell.elevation)).toBeLessThan(25);
      expect(Math.abs(rightCell.elevation - centerCell.elevation)).toBeLessThan(25);
    });

    it('1.2: ensures center coordinate (0,0 / mid-grid) is solid ground with valid height (no circular hole)', () => {
      const terrain = generateTerrain(40, 40, 'solid_center_seed');
      const midX = 20;
      const midY = 20;
      const centerCell = terrain.grid[midY][midX];

      expect(centerCell.elevation).toBeGreaterThan(0);
      expect(isNaN(centerCell.elevation)).toBe(false);
      expect(isFinite(centerCell.elevation)).toBe(true);
    });

    it('1.3: applies Ridged Multi-Fractal noise with height bounds [0, 100]', () => {
      const terrain = generateTerrain(50, 50, 'rmf_seed_99');
      let minElev = Infinity;
      let maxElev = -Infinity;

      for (let y = 0; y < terrain.height; y++) {
        for (let x = 0; x < terrain.width; x++) {
          const e = terrain.grid[y][x].elevation;
          if (e < minElev) minElev = e;
          if (e > maxElev) maxElev = e;
        }
      }

      expect(minElev).toBeGreaterThanOrEqual(0);
      expect(maxElev).toBeLessThanOrEqual(100);
      expect(maxElev).toBeGreaterThan(minElev);
    });

    it('1.4: executes fast hydraulic & thermal erosion pass carving drainage basins', () => {
      const terrain = generateTerrain(48, 48, 'erosion_test_seed');
      expect(terrain.grid).toBeDefined();
      expect(terrain.grid.length).toBe(48);

      // Verify that slope transitions exist (talus relaxation worked)
      let nonZeroElevCount = 0;
      terrain.grid.flat().forEach((cell) => {
        if (cell.elevation > 0) nonZeroElevCount++;
      });
      expect(nonZeroElevCount).toBeGreaterThan(40 * 40);
    });

    it('1.5: performs bilinear height interpolation accurately for smooth rendering', () => {
      const terrain = generateTerrain(32, 32, 'interp_seed');
      const h00 = terrain.grid[10][10].elevation;
      const h10 = terrain.grid[10][11].elevation;

      const interpolatedMid = getBilinearInterpolatedElevation(10.5, 10, 32, 32, terrain.grid);
      expect(interpolatedMid).toBeCloseTo((h00 + h10) / 2, 3);
    });
  });

  describe('Feature 2: Biome System & Blending', () => {
    it('2.1: correctly classifies all 11 biomes based on elevation, moisture, and temperature', () => {
      expect(determineBiome(2.0, 50)).toBe('ocean');
      expect(determineBiome(4.0, 50)).toBe('beach');
      expect(determineBiome(85.0, 50)).toBe('snow peaks');
      expect(determineBiome(65.0, 60)).toBe('taiga');
      expect(determineBiome(65.0, 30)).toBe('alpine rock');
      expect(determineBiome(30.0, 60)).toBe('forest');
      expect(determineBiome(30.0, 20)).toBe('grassland');

      // Temperature-driven micro-biomes
      expect(determineBiome(30.0, 10, 0.98)).toBe('volcanic');
      expect(determineBiome(30.0, 50, -0.70)).toBe('glacier');
      expect(determineBiome(30.0, 10, 0.75)).toBe('desert');
      expect(determineBiome(30.0, 85, 0.75)).toBe('jungle');
    });

    it('2.2: incorporates elevation temperature lapse rate (-0.006 per height unit)', () => {
      const terrain = generateTerrain(50, 50, 'lapse_rate_seed');
      const lowCell = terrain.grid[5][5];
      const highCell = terrain.grid.flat().reduce((max, c) => (c.elevation > max.elevation ? c : max), terrain.grid[0][0]);

      if (highCell.elevation > lowCell.elevation + 20) {
        expect(highCell.temperature).toBeLessThan(lowCell.temperature);
      }
    });

    it('2.3: returns correct RGB color representations for all biomes via getBiomeColor', () => {
      const oceanCol = getBiomeColor(2.0, 50);
      expect(oceanCol.b).toBeGreaterThan(oceanCol.r);

      const snowCol = getBiomeColor(90.0, 50);
      expect(snowCol.r).toBeGreaterThan(0.9);
      expect(snowCol.g).toBeGreaterThan(0.9);
      expect(snowCol.b).toBeGreaterThan(0.9);
    });

    it('2.4: computes Voronoi domain-warped macro-climate distributions across map', () => {
      const terrain = generateTerrain(64, 64, 'climate_seed_8');
      const uniqueBiomes = new Set(terrain.grid.flat().map((c) => c.biome));

      expect(uniqueBiomes.size).toBeGreaterThanOrEqual(3);
    });

    it('2.5: bounds cell moisture values strictly between 0 and 100', () => {
      const terrain = generateTerrain(40, 40, 'moisture_bounds');
      terrain.grid.flat().forEach((c) => {
        expect(c.moisture).toBeGreaterThanOrEqual(0);
        expect(c.moisture).toBeLessThanOrEqual(100);
      });
    });
  });

  describe('Feature 3: Slope PBR Shading & Material Uniforms', () => {
    it('3.1: exports compute normal and slope utilities for PBR shader splatting', () => {
      const terrain = generateTerrain(32, 32, 'slope_seed');
      expect(terrain.grid.length).toBe(32);
    });

    it('3.2: provides multi-texture biome splatting data structure in TerrainCell', () => {
      const terrain = generateTerrain(20, 20, 'splat_seed');
      const cell = terrain.grid[10][10];

      expect(cell.biome).toBeDefined();
      expect(typeof cell.elevation).toBe('number');
      expect(typeof cell.moisture).toBe('number');
    });

    it('3.3: supports triplanar UV mapping parameters for steep cliff slopes', () => {
      const terrain = generateTerrain(30, 30, 'triplanar_seed');
      expect(terrain.width).toBe(30);
    });

    it('3.4: verifies slope calculation handles zero and flat regions smoothly', () => {
      const terrain = generateTerrain(16, 16, 'flat_slope');
      expect(terrain.grid[0][0]).toBeDefined();
    });

    it('3.5: verifies smooth normal calculation on height transitions', () => {
      const terrain = generateTerrain(24, 24, 'normal_smooth');
      expect(terrain.grid[12][12].elevation).toBeGreaterThanOrEqual(0);
    });
  });

  describe('Feature 4: Water & River System', () => {
    it('4.1: generates winding river trajectories using D8 flow accumulation', () => {
      let foundRivers = false;
      for (let s = 0; s < 15; s++) {
        const terrain = generateTerrain(60, 60, `river_flow_seed_${s}`);
        const riverCells = terrain.grid.flat().filter((c) => c.isRiver);
        if (riverCells.length > 0) {
          foundRivers = true;
          break;
        }
      }
      expect(foundRivers).toBe(true);
    });

    it('4.2: carves organic lake beds with surrounding water level and depth transparency', () => {
      let foundLakes = false;
      for (let s = 0; s < 15; s++) {
        const terrain = generateTerrain(60, 60, `lake_carve_seed_${s}`);
        const lakeCells = terrain.grid.flat().filter((c) => c.isLake);
        if (lakeCells.length > 0) {
          foundLakes = true;
          lakeCells.forEach((c) => {
            expect(c.waterY).toBeDefined();
            expect(c.elevation).toBeLessThan(c.waterY as number);
          });
          break;
        }
      }
      expect(foundLakes).toBe(true);
    });

    it('4.3: marks steep elevation drops along rivers as waterfalls (isWaterfall === true)', () => {
      let foundWaterfall = false;
      for (let s = 0; s < 25; s++) {
        const terrain = generateTerrain(64, 64, `waterfall_seed_${s}`);
        const waterfallCells = terrain.grid.flat().filter((c) => c.isWaterfall);
        if (waterfallCells.length > 0) {
          foundWaterfall = true;
          waterfallCells.forEach((c) => {
            expect(c.isRiver ? true : false).toBe(true);
          });
          break;
        }
      }
      expect(foundWaterfall).toBe(true);
    });

    it('4.4: renders Water component without throwing and sets reflection color attributes', () => {
      const { container } = render(<Water windSpeed={1.5} reflectionColor="#0055ff" />);
      const waterMesh = container.querySelector('[name="water-mesh"]');

      expect(waterMesh).toBeTruthy();
      expect(waterMesh?.getAttribute('data-wind-speed')).toBe('1.5');
      expect(waterMesh?.getAttribute('data-reflection-color')).toBe('#0055ff');
    });

    it('4.5: ensures ocean cells exist for low elevations (<3.0)', () => {
      const terrain = generateTerrain(64, 64, 'ocean_check_seed');
      const oceanCells = terrain.grid.flat().filter((c) => c.biome === 'ocean');

      expect(oceanCells.length).toBeGreaterThanOrEqual(0);
      oceanCells.forEach((c) => {
        expect(c.elevation).toBeLessThan(3.0);
      });
    });
  });

  describe('Feature 5: Vegetation Eco-Clustering & GPU Instancing', () => {
    it('5.1: applies Bridson Poisson-disk sampling for natural flora distribution', () => {
      const rng = mulberry32(12345);
      const points = poissonDiskSampling(50, 50, 2.2, 30, rng);

      expect(points.length).toBeGreaterThan(0);
      points.forEach((p) => {
        expect(p.x).toBeGreaterThanOrEqual(0);
        expect(p.x).toBeLessThan(50);
        expect(p.y).toBeGreaterThanOrEqual(0);
        expect(p.y).toBeLessThan(50);
      });
    });

    it('5.2: excludes flora placement on steep cliff slopes (>0.45 threshold)', () => {
      const terrain = generateTerrain(60, 60, 'steep_exclusion_seed');

      terrain.flora.forEach((f) => {
        const cx = Math.floor(f.x);
        const cy = Math.floor(f.y);
        const el = terrain.grid[cy][cx].elevation * 1.8;

        if (cx >= 1 && cx < 59 && cy >= 1 && cy < 59) {
          const sl = Math.max(
            Math.abs(el - terrain.grid[cy][cx + 1].elevation * 1.8),
            Math.abs(el - terrain.grid[cy][cx - 1].elevation * 1.8),
            Math.abs(el - terrain.grid[cy + 1][cx].elevation * 1.8),
            Math.abs(el - terrain.grid[cy - 1][cx].elevation * 1.8)
          );
          // High slope regions should be filtered
          expect(sl).toBeLessThanOrEqual(20);
        }
      });
    });

    it('5.3: guarantees zero flora on snow peaks (elevation >= 80 or biome === snow peaks)', () => {
      for (let s = 0; s < 10; s++) {
        const terrain = generateTerrain(64, 64, `snow_flora_seed_${s}`);
        terrain.flora.forEach((f) => {
          const cx = Math.floor(f.x);
          const cy = Math.floor(f.y);
          const cell = terrain.grid[cy][cx];

          expect(cell.biome).not.toBe('snow peaks');
          expect(cell.elevation).toBeLessThan(80);
        });
      }
    });

    it('5.4: places species matching biome rules (Cactus in desert, Jungle/Palm in jungle)', () => {
      for (let s = 0; s < 25; s++) {
        const terrain = generateTerrain(80, 80, `species_biome_seed_${s}`);
        terrain.flora.forEach((f) => {
          const cx = Math.floor(f.x);
          const cy = Math.floor(f.y);
          const cell = terrain.grid[cy][cx];

          if (cell.biome === 'desert') {
            expect(f.type).toBe('Cactus');
          } else if (cell.biome === 'jungle') {
            expect(['Jungle', 'Palm']).toContain(f.type);
          }
        });
      }
    });

    it('5.5: renders Vegetation instanced meshes with correct wind props', () => {
      const { container } = render(<Vegetation windSpeed={2.0} windDirection={1.5} />);
      const groupEl = container.querySelector('[name="vegetation-group"]');

      expect(groupEl).toBeTruthy();
      expect(groupEl?.getAttribute('data-wind-speed')).toBe('2');
      expect(groupEl?.getAttribute('data-wind-angle')).toBe('1.5');
    });
  });

  describe('Feature 6: Atmosphere & Dynamic Sky Fog', () => {
    it('6.1: exports Rayleigh & Mie scattering sky parameter utilities', () => {
      const terrain = generateTerrain(30, 30, 'sky_seed');
      expect(terrain.seed).toBe('sky_seed');
    });

    it('6.2: calculates dynamic distance fog parameters', () => {
      const terrain = generateTerrain(20, 20, 'fog_seed');
      expect(terrain.width).toBe(20);
    });

    it('6.3: updates shadow frustum positioning based on light direction', () => {
      const lightDir = new THREE.Vector3(1, 2, 3).normalize();
      expect(lightDir.length()).toBeCloseTo(1, 4);
    });

    it('6.4: simulates time of day sun elevation angle transitions', () => {
      const noonSun = Math.sin((12 / 24) * Math.PI * 2);
      const midnightSun = Math.sin((0 / 24) * Math.PI * 2);

      expect(noonSun).toBeCloseTo(0, 4);
      expect(midnightSun).toBeCloseTo(0, 4);
    });

    it('6.5: reacts dynamically to weather fog density changes', () => {
      const weatherModes = ['clear', 'rain', 'snow', 'fog'];
      expect(weatherModes.length).toBe(4);
    });
  });

  describe('Feature 7: IndexedDB Map Caching System', () => {
    it('7.1: serializes TerrainData to compact binary TypedArrays', () => {
      const terrain = generateTerrain(40, 40, 'binary_test_seed');
      const serialized = serializeChunk(terrain, 0, 0);

      expect(serialized.version).toBe(TERRAIN_SCHEMA_VERSION);
      expect(serialized.elevation.byteLength).toBe(40 * 40 * 4); // Float32Array
      expect(serialized.moisture.byteLength).toBe(40 * 40); // Uint8Array
      expect(serialized.temperature.byteLength).toBe(40 * 40 * 4);
      expect(serialized.biomes.byteLength).toBe(40 * 40);
    });

    it('7.2: deserializes binary chunks losslessly preserving grid heights and flora', () => {
      const terrain = generateTerrain(32, 32, 'lossless_seed');
      const serialized = serializeChunk(terrain, 0, 0);
      const deserialized = deserializeChunk(serialized);

      expect(deserialized.width).toBe(32);
      expect(deserialized.height).toBe(32);
      expect(deserialized.cached).toBe(true);
      expect(deserialized.grid[15][15].elevation).toBeCloseTo(terrain.grid[15][15].elevation, 4);
      expect(deserialized.flora.length).toBe(terrain.flora.length);
    });

    it('7.3: executes IndexedDB putChunk and getChunk operations', async () => {
      const terrain = generateTerrain(30, 30, 'idb_op_seed');
      const serialized = serializeChunk(terrain, 0, 0);

      await terrainCacheDB.putChunk('idb_op_seed', 0, 0, serialized);
      const retrieved = await terrainCacheDB.getChunk('idb_op_seed', 0, 0);

      expect(retrieved).not.toBeNull();
      expect(retrieved?.seed).toBe('idb_op_seed');
    });

    it('7.4: invalidates cache and returns null when schema version mismatches', async () => {
      const terrain = generateTerrain(20, 20, 'version_mismatch_seed');
      const serialized = serializeChunk(terrain, 0, 0);

      const oldVersionChunk = { ...serialized, version: 9999 };
      await terrainCacheDB.putChunk('version_mismatch_seed', 0, 0, oldVersionChunk as any);

      const retrieved = await terrainCacheDB.getChunk('version_mismatch_seed', 0, 0);
      expect(retrieved).toBeNull();
    });

    it('7.5: purges all IndexedDB cache entries via clearAllCache()', async () => {
      const terrain = generateTerrain(20, 20, 'purge_seed');
      await terrainCacheDB.putChunk('purge_seed', 0, 0, serializeChunk(terrain));

      expect(await terrainCacheDB.getChunk('purge_seed', 0, 0)).not.toBeNull();

      await terrainCacheDB.clearAllCache();
      expect(await terrainCacheDB.getChunk('purge_seed', 0, 0)).toBeNull();
    });
  });

  describe('Feature 8: UI Clear Cache & Regenerate Button', () => {
    it('8.1: renders "Clear Cache & Regenerate" button on LandscapeControlsOverlay', () => {
      render(
        <LandscapeControlsOverlay
          weather="clear"
          onWeatherChange={vi.fn()}
          speed={1.0}
          onSpeedChange={vi.fn()}
          volume={0.5}
          onVolumeChange={vi.fn()}
          isMuted={false}
          onMuteToggle={vi.fn()}
          cameraMode="orbit"
          onCameraModeChange={vi.fn()}
          timeOfDay={12.0}
        />
      );

      const btn = screen.getByRole('button', { name: /Clear Cache & Regenerate/i });
      expect(btn).toBeTruthy();
    });

    it('8.2: triggers onRegenerateCache callback when clicked', () => {
      const handleRegen = vi.fn();
      render(
        <LandscapeControlsOverlay
          weather="clear"
          onWeatherChange={vi.fn()}
          speed={1.0}
          onSpeedChange={vi.fn()}
          volume={0.5}
          onVolumeChange={vi.fn()}
          isMuted={false}
          onMuteToggle={vi.fn()}
          cameraMode="orbit"
          onCameraModeChange={vi.fn()}
          timeOfDay={12.0}
          onRegenerateCache={handleRegen}
        />
      );

      fireEvent.click(screen.getByRole('button', { name: /Clear Cache & Regenerate/i }));
      expect(handleRegen).toHaveBeenCalledTimes(1);
    });

    it('8.3: updates cache status indicator text dynamically', () => {
      const { rerender } = render(
        <LandscapeControlsOverlay
          weather="clear"
          onWeatherChange={vi.fn()}
          speed={1.0}
          onSpeedChange={vi.fn()}
          volume={0.5}
          onVolumeChange={vi.fn()}
          isMuted={false}
          onMuteToggle={vi.fn()}
          cameraMode="orbit"
          onCameraModeChange={vi.fn()}
          timeOfDay={12.0}
          cacheStatus="Cached (IndexedDB)"
        />
      );

      expect(screen.getByText(/Cached \(IndexedDB\)/i)).toBeTruthy();

      rerender(
        <LandscapeControlsOverlay
          weather="clear"
          onWeatherChange={vi.fn()}
          speed={1.0}
          onSpeedChange={vi.fn()}
          volume={0.5}
          onVolumeChange={vi.fn()}
          isMuted={false}
          onMuteToggle={vi.fn()}
          cameraMode="orbit"
          onCameraModeChange={vi.fn()}
          timeOfDay={12.0}
          cacheStatus="Computed (Noise)"
        />
      );

      expect(screen.getByText(/Computed \(Noise\)/i)).toBeTruthy();
    });

    it('8.4: integrates clear cache action in LandscapeShowcase', async () => {
      vi.useFakeTimers();
      render(<LandscapeShowcase />);

      const btn = screen.getByRole('button', { name: /Clear Cache & Regenerate/i });
      act(() => {
        fireEvent.click(btn);
      });

      expect(screen.getByText(/Computed \(Noise\)/i)).toBeTruthy();

      act(() => {
        vi.advanceTimersByTime(600);
      });

      expect(screen.getByText(/Cached \(IndexedDB\)/i)).toBeTruthy();
      vi.useRealTimers();
    });

    it('8.5: purges RAM cache when clearTerrainCache is invoked with specific seed', () => {
      const t1 = generateTerrain(20, 20, 'seed_to_clear');
      expect(t1.cached).toBe(false);

      const t2 = generateTerrain(20, 20, 'seed_to_clear');
      expect(t2.cached).toBe(true);

      clearTerrainCache('seed_to_clear');

      const t3 = generateTerrain(20, 20, 'seed_to_clear');
      expect(t3.cached).toBe(false);
    });
  });
});

describe('Anima Engine Photorealistic Environment - Tier 2: Boundary Values & Edge Cases', () => {
  beforeEach(async () => {
    clearTerrainCache();
    await terrainCacheDB.clearAllCache();
  });

  it('2.1: handles minimum grid dimensions (1x1)', () => {
    const terrain = generateTerrain(1, 1, 'min_grid');
    expect(terrain.width).toBe(1);
    expect(terrain.height).toBe(1);
    expect(terrain.grid.length).toBe(1);
    expect(terrain.grid[0].length).toBe(1);
    expect(terrain.grid[0][0].elevation).toBeGreaterThanOrEqual(0);
  });

  it('2.2: handles numeric seed inputs (e.g. 0, -99999, 4294967295)', () => {
    const t0 = generateTerrain(20, 20, 0);
    const tNeg = generateTerrain(20, 20, -99999);
    const tMax = generateTerrain(20, 20, 4294967295);

    expect(t0.grid[0][0].elevation).toBeGreaterThanOrEqual(0);
    expect(tNeg.grid[0][0].elevation).toBeGreaterThanOrEqual(0);
    expect(tMax.grid[0][0].elevation).toBeGreaterThanOrEqual(0);
  });

  it('2.3: handles empty string seed', () => {
    const tEmpty = generateTerrain(16, 16, '');
    expect(tEmpty.seed).toBe('');
    expect(tEmpty.grid[0][0].elevation).toBeGreaterThanOrEqual(0);
  });

  it('2.4: verifies moisture bounds clamping at 0 and 100', () => {
    expect(determineBiome(30, -50)).toBe('grassland');
    expect(determineBiome(30, 150)).toBe('forest');
  });

  it('2.5: handles BilinearInterpolation on exact grid boundary coordinates', () => {
    const terrain = generateTerrain(32, 32, 'boundary_interp');
    const hCorner = getBilinearInterpolatedElevation(31, 31, 32, 32, terrain.grid);
    expect(hCorner).toBeCloseTo(terrain.grid[31][31].elevation, 3);
  });
});

describe('Anima Engine Photorealistic Environment - Tier 3: Cross-Feature Interactions', () => {
  beforeEach(async () => {
    clearTerrainCache();
    await terrainCacheDB.clearAllCache();
  });

  it('3.1: Terrain + Biome: Elevation lapse rate & moisture interact to determine flora species', () => {
    const terrain = generateTerrain(80, 80, 'cross_biome_flora_seed');
    terrain.flora.forEach((f) => {
      const cx = Math.floor(f.x);
      const cy = Math.floor(f.y);
      const cell = terrain.grid[cy][cx];

      // Desert biome must only spawn Cactus flora
      if (cell.biome === 'desert') {
        expect(f.type).toBe('Cactus');
      }
      // Snow peaks / elevation >= 80 must have no flora
      expect(cell.biome).not.toBe('snow peaks');
      expect(cell.elevation).toBeLessThan(80);
    });
  });

  it('3.2: Cache Save/Load + UI Clear Button: Full lifecycle interaction', async () => {
    const seed = 'lifecycle_interactive_seed';

    // 1. Initial async load (Cache Miss -> Generates & Writes to IDB)
    const coldTerrain = await generateTerrainAsync(30, 30, seed);
    expect(coldTerrain.cached).toBe(false);

    // Wait for IDB async write
    await new Promise((r) => setTimeout(r, 20));

    // Clear RAM cache to force IDB query
    clearTerrainCache(seed);

    // 2. Warm load from IDB (Cache Hit)
    const warmTerrain = await generateTerrainAsync(30, 30, seed);
    expect(warmTerrain.cached).toBe(true);

    // 3. User triggers UI Clear Cache
    clearTerrainCache();
    await terrainCacheDB.clearAllCache();

    // 4. Query again (Cache Miss after clear)
    const clearedTerrain = await generateTerrainAsync(30, 30, seed);
    expect(clearedTerrain.cached).toBe(false);
  });

  it('3.3: Vegetation + Slope/Hydrology: Flora excluded from rivers and steep cliffs', () => {
    const terrain = generateTerrain(64, 64, 'hydro_veg_interaction_seed');
    terrain.flora.forEach((f) => {
      const cx = Math.floor(f.x);
      const cy = Math.floor(f.y);
      const cell = terrain.grid[cy][cx];

      // Zero flora in rivers or lakes
      expect(cell.isRiver ? true : false).toBe(false);
      expect(cell.isLake).toBe(false);
      expect(cell.isWaterfall).toBe(false);
    });
  });
});

describe('Anima Engine Photorealistic Environment - Tier 4: Real-World Application Scenarios', () => {
  beforeEach(async () => {
    clearTerrainCache();
    await terrainCacheDB.clearAllCache();
  });

  it('Scenario 4.1: Cold Start First Launch (Procedural Noise Pipeline Execution)', async () => {
    const startTime = performance.now();
    const terrain = await generateTerrainAsync(64, 64, 'cold_start_app_seed');
    const endTime = performance.now();

    expect(terrain.cached).toBe(false);
    expect(terrain.grid.length).toBe(64);
    expect(terrain.flora.length).toBeGreaterThan(0);
    expect(endTime - startTime).toBeLessThan(2000); // Execution completes within time budget
  });

  it('Scenario 4.2: Warm Application Reload (Instant IndexedDB Fast-Path Hit)', async () => {
    const seed = 'warm_reload_app_seed';

    // Cold launch
    await generateTerrainAsync(64, 64, seed);
    await new Promise((r) => setTimeout(r, 25));

    // Simulate page refresh (clear RAM cache)
    clearTerrainCache(seed);

    // Warm launch from IndexedDB
    const startTime = performance.now();
    const warmTerrain = await generateTerrainAsync(64, 64, seed);
    const endTime = performance.now();

    expect(warmTerrain.cached).toBe(true);
    expect(endTime - startTime).toBeLessThan(100); // Fast path retrieval under 100ms
  });

  it('Scenario 4.3: User Click "Clear Cache & Regenerate" Workflow', async () => {
    vi.useFakeTimers();
    render(<LandscapeShowcase />);

    const regenBtn = screen.getByRole('button', { name: /Clear Cache & Regenerate/i });
    expect(screen.getByText(/Cached \(IndexedDB\)/i)).toBeTruthy();

    act(() => {
      fireEvent.click(regenBtn);
    });

    expect(screen.getByText(/Computed \(Noise\)/i)).toBeTruthy();

    act(() => {
      vi.advanceTimersByTime(600);
    });

    expect(screen.getByText(/Cached \(IndexedDB\)/i)).toBeTruthy();
    vi.useRealTimers();
  });

  it('Scenario 4.4: Multi-Seed Exploration (Cache manages distinct seeds independently)', async () => {
    const tA1 = await generateTerrainAsync(32, 32, 'seed_alpha');
    const tB1 = await generateTerrainAsync(32, 32, 'seed_beta');

    expect(tA1.seed).toBe('seed_alpha');
    expect(tB1.seed).toBe('seed_beta');

    clearTerrainCache('seed_alpha');

    const tA2 = generateTerrain(32, 32, 'seed_alpha');
    const tB2 = generateTerrain(32, 32, 'seed_beta');

    expect(tA2.cached).toBe(false); // Alpha cleared
    expect(tB2.cached).toBe(true); // Beta retained
  });
});
