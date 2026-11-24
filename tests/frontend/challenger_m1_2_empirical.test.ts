import { describe, it, expect } from 'vitest';
import {
  generateTerrain,
  getBilinearInterpolatedElevation,
  determineBiome,
  getContinuousBiomeWeights,
  clearTerrainCache,
  TerrainCell,
  FloraPlacement
} from '../../src/components/Landscape/utils/terrainGenerator';

describe('Challenger M1.2 - Empirical Verification Suite', () => {

  describe('1. Hydrology & River Flow Verification', () => {
    it('verifies river origins, continuous flow without uphill steps or broken paths across seeds', () => {
      const seeds = ['seed_alpha', 'seed_beta', 'seed_gamma', 'hydro_test_1', 'hydro_test_2', 'mountain_seed_99'];
      const mapSizes = [60, 100];

      let totalRiversTested = 0;
      let totalUphillSteps = 0;
      let totalBrokenPaths = 0;

      for (const size of mapSizes) {
        for (const seed of seeds) {
          const terrain = generateTerrain(size, size, seed);
          const grid = terrain.grid;

          // Identify all river cells
          const riverCells: TerrainCell[] = [];
          for (let y = 0; y < size; y++) {
            for (let x = 0; x < size; x++) {
              if (grid[y][x].isRiver) {
                riverCells.push(grid[y][x]);
              }
            }
          }

          if (riverCells.length === 0) continue;
          totalRiversTested++;

          // Trace river connectivity and flow direction
          // Find river source candidates (river cells with highest local elevation among neighbors)
          for (const cell of riverCells) {
            const neighbors: TerrainCell[] = [];
            const directions = [
              [-1, 0], [1, 0], [0, -1], [0, 1],
              [-1, -1], [-1, 1], [1, -1], [1, 1]
            ];

            for (const [dx, dy] of directions) {
              const nx = cell.x + dx;
              const ny = cell.y + dy;
              if (nx >= 0 && nx < size && ny >= 0 && ny < size && grid[ny][nx].isRiver) {
                neighbors.push(grid[ny][nx]);
              }
            }

            // Check if any river neighbor flows UPHILL from this cell
            for (const n of neighbors) {
              // If neighbor is an downstream candidate (lower or equal), ok.
              // But if all neighbor paths lead to higher elevation, that's uphill flow.
              if (n.elevation > cell.elevation + 0.001) {
                // Potential uphill flow step along river
                totalUphillSteps++;
              }
            }

            // Check if river cell is isolated (broken path without ocean/lake connection)
            const connectsToWater = cell.elevation < 3.0 || cell.isLake || neighbors.length > 0;
            if (!connectsToWater) {
              totalBrokenPaths++;
            }
          }
        }
      }

      console.log(`[Hydrology Test] Tested ${totalRiversTested} river grids. Uphill step instances: ${totalUphillSteps}, Broken path instances: ${totalBrokenPaths}`);

      // Assertions for Hydrology
      expect(totalUphillSteps, `Found ${totalUphillSteps} uphill flow steps in river paths`).toBe(0);
      expect(totalBrokenPaths, `Found ${totalBrokenPaths} broken/isolated river cells`).toBe(0);
    });
  });

  describe('2. Flora Placement Exclusions Verification', () => {
    it('asserts 100% exclusion of vegetation on cliffs (slope > 0.6) and snowy peaks (elevation >= 80)', () => {
      const seeds = ['flora_seed_1', 'flora_seed_2', 'flora_seed_3', 'seed_test_88', 'biome_seed_500'];
      const mapSizes = [60, 100];

      let totalFloraChecked = 0;
      let floraOnSnowPeaks = 0;
      let floraOnCliffsGrad = 0;
      let floraOnCliffsNormal = 0;

      for (const size of mapSizes) {
        for (const seed of seeds) {
          const terrain = generateTerrain(size, size, seed);
          const grid = terrain.grid;

          for (const item of terrain.flora) {
            totalFloraChecked++;
            const cx = Math.floor(item.x);
            const cy = Math.floor(item.y);

            const cell = grid[cy][cx];
            const interpElev = getBilinearInterpolatedElevation(item.x, item.y, size, size, grid);

            // 1. Snowy peaks check (elevation >= 80 or biome === 'snow peaks')
            if (cell.elevation >= 80 || interpElev >= 80 || cell.biome === 'snow peaks') {
              floraOnSnowPeaks++;
            }

            // 2. Slope checks
            // Calculate height gradient delta in grid
            const hC = cell.elevation;
            const hR = cx < size - 1 ? grid[cy][cx + 1].elevation : hC;
            const hL = cx > 0 ? grid[cy][cx - 1].elevation : hC;
            const hD = cy < size - 1 ? grid[cy + 1][cx].elevation : hC;
            const hU = cy > 0 ? grid[cy - 1][cx].elevation : hC;

            const dhdx = (hR - hL) / 2;
            const dhdy = (hD - hU) / 2;

            // Gradient magnitude slope: |grad| = sqrt(dhdx^2 + dhdy^2)
            const gradSlope = Math.sqrt(dhdx * dhdx + dhdy * dhdy);

            // Max directional delta slope: max(|hC - neighbor|)
            const maxDeltaSlope = Math.max(
              Math.abs(hC - hR),
              Math.abs(hC - hL),
              Math.abs(hC - hD),
              Math.abs(hC - hU)
            );

            // Surface normal slope: 1 - Ny = 1 - 1 / sqrt(1 + dhdx^2 + dhdy^2)
            const normalSlope = 1.0 - (1.0 / Math.sqrt(1.0 + dhdx * dhdx + dhdy * dhdy));

            if (gradSlope > 0.6 || maxDeltaSlope > 0.6) {
              floraOnCliffsGrad++;
            }

            if (normalSlope > 0.6) {
              floraOnCliffsNormal++;
            }
          }
        }
      }

      console.log(`[Flora Test] Total flora checked: ${totalFloraChecked}. On snow peaks (el >= 80): ${floraOnSnowPeaks}. On cliffs (gradient > 0.6): ${floraOnCliffsGrad}. On cliffs (normal > 0.6): ${floraOnCliffsNormal}`);

      // Assertions
      expect(floraOnSnowPeaks, `Found ${floraOnSnowPeaks} vegetation items on snowy peaks (el >= 80)`).toBe(0);
      expect(floraOnCliffsGrad, `Found ${floraOnCliffsGrad} vegetation items on cliff slopes (gradient > 0.6)`).toBe(0);
    });
  });

  describe('3. Performance Benchmarking & Vertex Count Limits', () => {
    it('benchmarks terrain generation execution time and vertex count limits across grid sizes', () => {
      const benchmarkSizes = [
        { width: 32, height: 32, maxTimeMs: 100 },
        { width: 64, height: 64, maxTimeMs: 250 },
        { width: 100, height: 100, maxTimeMs: 500 },
        { width: 128, height: 128, maxTimeMs: 800 },
        { width: 256, height: 256, maxTimeMs: 3000 },
      ];

      const benchmarkResults: {
        gridSize: string;
        vertexCount: number;
        triangleCount: number;
        executionTimeMs: number;
        cellsPerMs: number;
      }[] = [];

      for (const { width, height, maxTimeMs } of benchmarkSizes) {
        clearTerrainCache(); // Ensure cold generation benchmark

        const startTime = performance.now();
        const terrain = generateTerrain(width, height, `benchmark_seed_${width}x${height}`);
        const endTime = performance.now();
        const duration = endTime - startTime;

        const vertexCount = width * height;
        const triangleCount = (width - 1) * (height - 1) * 2;
        const cellsPerMs = vertexCount / (duration || 1);

        benchmarkResults.push({
          gridSize: `${width}x${height}`,
          vertexCount,
          triangleCount,
          executionTimeMs: duration,
          cellsPerMs,
        });

        // Verify terrain data structure validity
        expect(terrain.grid.length).toBe(height);
        expect(terrain.grid[0].length).toBe(width);
        expect(terrain.cached).toBeFalsy();

        // Performance limit assertion
        expect(duration, `Grid ${width}x${height} execution time (${duration.toFixed(2)}ms) exceeded budget (${maxTimeMs}ms)`).toBeLessThan(maxTimeMs);
      }

      console.table(benchmarkResults);
    });

    it('verifies in-memory and fast-path cache speedup over cold generation', async () => {
      const width = 100;
      const height = 100;
      const seed = 'cache_perf_test_seed';

      clearTerrainCache();

      // Cold generation timing
      const t0 = performance.now();
      const coldTerrain = generateTerrain(width, height, seed);
      const coldTime = performance.now() - t0;
      expect(coldTerrain.cached).toBeFalsy();

      // Warm generation timing (in-memory hit)
      const t1 = performance.now();
      const warmTerrain = generateTerrain(width, height, seed);
      const warmTime = performance.now() - t1;
      expect(warmTerrain.cached).toBe(true);

      console.log(`[Cache Perf] Cold generation: ${coldTime.toFixed(2)}ms | In-memory cache hit: ${warmTime.toFixed(2)}ms`);

      // In-memory hit should be at least 10x faster than cold generation
      expect(warmTime).toBeLessThan(coldTime * 0.1);
      expect(warmTime).toBeLessThan(10.0); // < 10ms
    });
  });

  describe('4. Biome Classification under Extreme Height, Moisture, and Slope Inputs', () => {
    it('handles extreme elevation inputs without NaN, invalid biomes, or crashes', () => {
      const extremeElevations = [-1000.0, -100.0, -1.0, 0.0, 2.9, 3.0, 4.9, 5.0, 59.9, 60.0, 79.9, 80.0, 100.0, 500.0, 10000.0];

      for (const el of extremeElevations) {
        const biome = determineBiome(el, 50, 0.0);
        expect(biome).toBeDefined();
        expect(['ocean', 'beach', 'grassland', 'forest', 'taiga', 'alpine rock', 'snow peaks', 'desert', 'jungle', 'volcanic', 'glacier']).toContain(biome);

        if (el < 3.0) expect(biome).toBe('ocean');
        else if (el < 5.0) expect(biome).toBe('beach');
        else if (el >= 80.0) expect(biome).toBe('snow peaks');
      }
    });

    it('handles extreme moisture inputs (-500 to 500) and temperature extremes gracefully', () => {
      const moistureInputs = [-500.0, -100.0, 0.0, 10.0, 20.0, 45.0, 50.0, 70.0, 100.0, 500.0];
      const temperatures = [-2.0, -0.6, -0.51, 0.0, 0.61, 0.96, 2.0];

      for (const m of moistureInputs) {
        for (const t of temperatures) {
          const biome = determineBiome(25.0, m, t);
          expect(biome).toBeDefined();
          
          if (t > 0.95) {
            expect(biome).toBe('volcanic');
          } else if (t < -0.5) {
            expect(biome).toBe('glacier');
          } else if (t > 0.6) {
            if (m < 20) expect(biome).toBe('desert');
            if (m >= 70) expect(biome).toBe('jungle');
          }
        }
      }
    });

    it('verifies getContinuousBiomeWeights normalizes sum to 1.0 across extreme slope, elevation, and moisture inputs', () => {
      const elevations = [-50, 0, 4.0, 30, 70, 95, 200];
      const moistures = [-50, 0, 25, 65, 100, 200];
      const temperatures = [-1.5, -0.5, 0.2, 0.75, 1.2];
      const slopes = [0.0, 0.05, 0.15, 0.35, 0.6, 1.0, 10.0, 100.0];

      for (const el of elevations) {
        for (const moist of moistures) {
          for (const temp of temperatures) {
            for (const slope of slopes) {
              const weights = getContinuousBiomeWeights(el, moist, temp, slope);

              let sum = 0;
              for (const [bKey, wVal] of Object.entries(weights)) {
                expect(Number.isNaN(wVal)).toBe(false);
                expect(Number.isFinite(wVal)).toBe(true);
                expect(wVal).toBeGreaterThanOrEqual(0.0);
                expect(wVal).toBeLessThanOrEqual(1.0);
                sum += wVal;
              }

              // Normalization assertion: sum must equal 1.0
              expect(sum).toBeCloseTo(1.0, 4);
            }
          }
        }
      }
    });
  });

});
