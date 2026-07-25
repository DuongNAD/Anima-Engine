import { describe, it, expect } from 'vitest';
import {
  ImprovedNoise2D,
  determineBiome,
  generateTerrain,
  hashString,
} from '../../src/components/Landscape/utils/terrainGenerator';

describe('Terrain Generator - Seeded Noise', () => {
  it('should generate deterministic 2D noise with the same seed', () => {
    const seed = 'anima_test_seed';
    const noiseGen1 = new ImprovedNoise2D(seed);
    const noiseGen2 = new ImprovedNoise2D(seed);

    for (let i = 0; i < 50; i++) {
      const x = i * 0.13;
      const y = i * 0.29;
      expect(noiseGen1.noise(x, y)).toBeCloseTo(noiseGen2.noise(x, y), 6);
    }
  });

  it('should generate different noise values for different seeds', () => {
    const noiseGen1 = new ImprovedNoise2D('seed_a');
    const noiseGen2 = new ImprovedNoise2D('seed_b');

    let matches = 0;
    for (let i = 0; i < 20; i++) {
      const x = i * 0.57 + 0.12;
      const y = i * 0.83 + 0.24;
      if (Math.abs(noiseGen1.noise(x, y) - noiseGen2.noise(x, y)) < 1e-5) {
        matches++;
      }
    }
    // They should not match for almost all coordinates
    expect(matches).toBeLessThan(5);
  });
});

describe('Terrain Generator - Bounds & Biomes', () => {
  it('should generate elevation and moisture values within expected bounds', () => {
    const width = 30;
    const height = 30;
    const seed = 'bounds_check';
    const terrain = generateTerrain(width, height, seed);

    expect(terrain.width).toBe(width);
    expect(terrain.height).toBe(height);
    expect(terrain.grid.length).toBe(height);
    expect(terrain.grid[0].length).toBe(width);

    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const cell = terrain.grid[y][x];
        expect(cell.x).toBe(x);
        expect(cell.y).toBe(y);
        expect(cell.elevation).toBeGreaterThanOrEqual(0);
        expect(cell.elevation).toBeLessThanOrEqual(100);
        expect(cell.moisture).toBeGreaterThanOrEqual(0);
        expect(cell.moisture).toBeLessThanOrEqual(100);
      }
    }
  });

  it('should map elevation and moisture to the correct biomes', () => {
    // ocean: elevation < 3.0
    expect(determineBiome(2.5, 80)).toBe('ocean');
    expect(determineBiome(0, 10)).toBe('ocean');

    // beach: 3.0 <= elevation < 5.0
    expect(determineBiome(3.5, 10)).toBe('beach');
    expect(determineBiome(4.9, 90)).toBe('beach');

    // snow peaks: elevation >= 80
    expect(determineBiome(80, 0)).toBe('snow peaks');
    expect(determineBiome(95, 95)).toBe('snow peaks');

    // taiga: elevation >= 60, moisture >= 50
    expect(determineBiome(65, 55)).toBe('taiga');
    expect(determineBiome(79, 90)).toBe('taiga');

    // alpine rock: elevation >= 60, moisture < 50
    expect(determineBiome(65, 45)).toBe('alpine rock');
    expect(determineBiome(79, 10)).toBe('alpine rock');

    // forest: elevation < 60, moisture >= 45
    expect(determineBiome(30, 45)).toBe('forest');
    expect(determineBiome(55, 90)).toBe('forest');

    // grassland: elevation < 60, moisture < 45
    expect(determineBiome(30, 40)).toBe('grassland');
    expect(determineBiome(55, 10)).toBe('grassland');
  });
});

describe('Terrain Generator - Hydrological System', () => {
  it('should set river, lake, and waterfall flags appropriately', () => {
    const width = 60;
    const height = 60;
    // Use a seed that is known/likely to generate lakes and rivers
    const seed = 'hydro_testing_seed';
    const terrain = generateTerrain(width, height, seed);

    let hasRiver = false;
    let hasLake = false;
    let hasWaterfall = false;

    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const cell = terrain.grid[y][x];
        if (cell.isRiver) {
          hasRiver = true;
        }
        if (cell.isLake) {
          hasLake = true;
          expect(cell.waterY).toBeDefined();
          expect(cell.elevation).toBeLessThan(cell.waterY as number);
        }
        if (cell.isWaterfall) {
          hasWaterfall = true;
          // Waterfall cells must be river cells
          expect(cell.isRiver ? true : false).toBe(true);
        }
      }
    }

    // Verify system works and sets flags
    expect(hasRiver).toBe(true);
    expect(hasLake).toBe(true);
    expect(hasWaterfall).toBe(true);
  });
});

describe('Terrain Generator - Vegetation Placement', () => {
  it('should place flora within correct boundaries and respect biome criteria', () => {
    const width = 60;
    const height = 60;
    const seed = 'flora_testing_seed';
    const terrain = generateTerrain(width, height, seed);

    expect(terrain.flora.length).toBeGreaterThan(0);

    for (const p of terrain.flora) {
      // 1. Within bounds
      expect(p.x).toBeGreaterThanOrEqual(0);
      expect(p.x).toBeLessThan(width);
      expect(p.y).toBeGreaterThanOrEqual(0);
      expect(p.y).toBeLessThan(height);

      // 2. Scale constraints
      expect(p.scale).toBeGreaterThanOrEqual(0.6); // Scale in [0.6, 1.4] due to 0.6 + random() * 0.8
      expect(p.scale).toBeLessThanOrEqual(1.4);

      // 3. Biome/Terrain constraints
      const cx = Math.floor(p.x);
      const cy = Math.floor(p.y);
      const cell = terrain.grid[cy][cx];

      // No vegetation in water (ocean, lake, river, waterfall)
      expect(cell.elevation).toBeGreaterThanOrEqual(3.0);
      expect(cell.isLake).toBe(false);
      expect(cell.isRiver ? true : false).toBe(false);
      expect(cell.isWaterfall).toBe(false);

      // No vegetation on snow peaks
      expect(cell.biome).not.toBe('snow peaks');
      expect(cell.elevation).toBeLessThan(80);

      // Allowed types
      expect(['Oak', 'Pine', 'Bush', 'Rock', 'Palm', 'Cactus', 'Jungle', 'Birch', 'Flowers', 'Dead Trunk', 'Snow Pine', 'Ice Rock']).toContain(p.type);
    }
  });
});
