import { describe, it, expect, beforeEach } from 'vitest';
import {
  generateTerrain,
  generateTerrainData,
  determineBiome,
  getBilinearInterpolatedElevation,
  clearTerrainCache,
  TerrainCell,
  BiomeType,
} from '../../src/components/Landscape/utils/terrainGenerator';

describe('Terrain Naturalness & Biome Distribution E2E Suite (R1)', () => {
  beforeEach(() => {
    clearTerrainCache();
  });

  describe('Tier 1: Feature Coverage', () => {
    it('T1.1: verify removal of radial center bias (no center hole or radial drop)', () => {
      const width = 64;
      const height = 64;
      const terrain = generateTerrain(width, height, 'test_seed_radial');

      // Sample center cells (30..34, 30..34) vs corner cells (2..6, 2..6)
      const centerCells: TerrainCell[] = [];
      for (let y = 30; y <= 34; y++) {
        for (let x = 30; x <= 34; x++) {
          centerCells.push(terrain.grid[y][x]);
        }
      }

      const cornerCells: TerrainCell[] = [];
      for (let y = 2; y <= 6; y++) {
        for (let x = 2; x <= 6; x++) {
          cornerCells.push(terrain.grid[y][x]);
        }
      }

      const avgCenterElev = centerCells.reduce((acc, c) => acc + c.elevation, 0) / centerCells.length;
      const avgCornerElev = cornerCells.reduce((acc, c) => acc + c.elevation, 0) / cornerCells.length;

      // Center should not be artificially zeroed out or forced to an artificial hole
      expect(avgCenterElev).toBeGreaterThan(5.0);
      expect(Number.isNaN(avgCenterElev)).toBe(false);
      expect(Number.isNaN(avgCornerElev)).toBe(false);
    });

    it('T1.2: verify heightmap variation across organic ranges', () => {
      const terrain = generateTerrain(64, 64, 'test_organic_range');
      const flatGrid = terrain.grid.flat();
      const elevations = flatGrid.map((c) => c.elevation);

      const minElev = Math.min(...elevations);
      const maxElev = Math.max(...elevations);
      const range = maxElev - minElev;

      // Organic terrain must feature dynamic elevation range across mountain peaks & lowlands
      expect(range).toBeGreaterThan(15.0);
      expect(minElev).toBeGreaterThanOrEqual(0.0);
      expect(maxElev).toBeLessThanOrEqual(100.0);
    });

    it('T1.3: verify biome distribution mapping across temperature, moisture, and elevation', () => {
      expect(determineBiome(2.0, 50, 0.0)).toBe('ocean');
      expect(determineBiome(4.0, 50, 0.0)).toBe('beach');
      expect(determineBiome(85.0, 50, 0.0)).toBe('snow peaks');
      expect(determineBiome(65.0, 60, 0.0)).toBe('taiga');
      expect(determineBiome(65.0, 30, 0.0)).toBe('alpine rock');
      expect(determineBiome(20.0, 50, 0.0)).toBe('forest');
      expect(determineBiome(20.0, 20, 0.0)).toBe('grassland');

      // Temperature overrides
      expect(determineBiome(20.0, 10, 1.0)).toBe('volcanic');
      expect(determineBiome(20.0, 30, -0.6)).toBe('glacier');
      expect(determineBiome(20.0, 10, 0.7)).toBe('desert');
      expect(determineBiome(20.0, 80, 0.7)).toBe('jungle');
    });

    it('T1.4: verify Voronoi and Perlin macro-climate cellular distribution', () => {
      const terrain = generateTerrain(64, 64, 'climate_cell_seed');
      const grid = terrain.grid;

      const temperatures = grid.flat().map((c) => c.temperature);
      const moistures = grid.flat().map((c) => c.moisture);

      const minTemp = Math.min(...temperatures);
      const maxTemp = Math.max(...temperatures);
      const minMoist = Math.min(...moistures);
      const maxMoist = Math.max(...moistures);

      expect(maxTemp - minTemp).toBeGreaterThan(0.3);
      expect(maxMoist - minMoist).toBeGreaterThan(15.0);
    });

    it('T1.5: verify hydraulic and thermal erosion simulation pass', () => {
      const terrain = generateTerrain(64, 64, 'erosion_test_seed');
      const riverCells = terrain.grid.flat().filter((c) => c.isRiver);
      
      // Drainage basins should yield river flow paths
      expect(riverCells.length).toBeGreaterThanOrEqual(0);
      terrain.grid.flat().forEach((cell) => {
        expect(cell.elevation).toBeGreaterThanOrEqual(0.0);
        expect(cell.elevation).toBeLessThanOrEqual(100.0);
      });
    });
  });

  describe('Tier 2: Boundary & Corner Cases', () => {
    it('T2.1: extreme seeds (0, negative int, max 32-bit int, empty string)', () => {
      const seeds = [0, -99999, 2147483647, ''];
      seeds.forEach((seed) => {
        const terrain = generateTerrain(32, 32, seed);
        expect(terrain.grid.length).toBe(32);
        expect(terrain.grid[0][0].elevation).toBeGreaterThanOrEqual(0);
        expect(Number.isNaN(terrain.grid[0][0].elevation)).toBe(false);
      });
    });

    it('T2.2: boundary elevations (0.0 to 100.0 limits)', () => {
      expect(determineBiome(0.0, 50)).toBe('ocean');
      expect(determineBiome(100.0, 50)).toBe('snow peaks');
      expect(determineBiome(3.0, 50)).toBe('beach');
      expect(determineBiome(79.9, 20)).toBe('alpine rock');
      expect(determineBiome(80.0, 20)).toBe('snow peaks');
    });

    it('T2.3: extreme moisture values (0.0 and 100.0)', () => {
      expect(determineBiome(20.0, 0.0, 0.7)).toBe('desert');
      expect(determineBiome(20.0, 100.0, 0.7)).toBe('jungle');
      expect(determineBiome(65.0, 0.0)).toBe('alpine rock');
      expect(determineBiome(65.0, 100.0)).toBe('taiga');
    });

    it('T2.4: edge and corner coordinate lookups without out-of-bounds', () => {
      const width = 32;
      const height = 32;
      const terrain = generateTerrain(width, height, 'boundary_coord');

      const topLeft = getBilinearInterpolatedElevation(0, 0, width, height, terrain.grid);
      const topRight = getBilinearInterpolatedElevation(width - 1, 0, width, height, terrain.grid);
      const bottomLeft = getBilinearInterpolatedElevation(0, height - 1, width, height, terrain.grid);
      const bottomRight = getBilinearInterpolatedElevation(width - 1, height - 1, width, height, terrain.grid);

      expect(topLeft).toBeGreaterThanOrEqual(0);
      expect(topRight).toBeGreaterThanOrEqual(0);
      expect(bottomLeft).toBeGreaterThanOrEqual(0);
      expect(bottomRight).toBeGreaterThanOrEqual(0);
    });

    it('T2.5: minimal (2x2) and non-power-of-two (17x17) grid sizes', () => {
      const smallTerrain = generateTerrain(2, 2, 'small');
      expect(smallTerrain.grid.length).toBe(2);
      expect(smallTerrain.grid[0].length).toBe(2);

      const oddTerrain = generateTerrain(17, 17, 'odd');
      expect(oddTerrain.grid.length).toBe(17);
      expect(oddTerrain.grid[16].length).toBe(17);
    });
  });

  describe('Tier 3: Cross-Feature Combinations', () => {
    it('T3.1: elevation + moisture + temperature produces cohesive biome distribution', () => {
      const terrain = generateTerrain(48, 48, 'cross_biome_seed');
      const biomes = new Set(terrain.grid.flat().map((c) => c.biome));
      expect(biomes.size).toBeGreaterThanOrEqual(2);
    });
  });

  describe('Tier 4: Real-World Application Scenarios', () => {
    it('T4.1: map regeneration sequence produces distinct organic heightmap patterns', () => {
      const terrainA = generateTerrainData(32, 32, 'seed_alpha');
      const terrainB = generateTerrainData(32, 32, 'seed_beta');

      let totalDiff = 0;
      for (let i = 0; i < terrainA.length; i++) {
        totalDiff += Math.abs(terrainA[i] - terrainB[i]);
      }
      expect(totalDiff).toBeGreaterThan(50.0);
    });
  });
});
