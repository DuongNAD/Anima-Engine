import { describe, it, expect, beforeEach } from 'vitest';
import {
  generateTerrain,
  determineBiome,
  getContinuousBiomeWeights,
  clearTerrainCache,
  BiomeType,
} from '../../src/components/Landscape/utils/terrainGenerator';
import { serializeChunk, deserializeChunk } from '../../src/components/Landscape/utils/terrainSerializer';

describe('Challenger M1.1 Empirical Verification & Stress Test Suite', () => {
  beforeEach(() => {
    clearTerrainCache();
  });

  describe('Empirical Test 1: Statistical Heightmap Peak Elevation Variation Across 50 Seeds', () => {
    it('should verify peak elevation is distributed across the entire map, NOT clustered artificially at (0,0)', () => {
      const NUM_SEEDS = 50;
      const WIDTH = 64;
      const HEIGHT = 64;

      const peakCoords: { x: number; y: number; seed: string; peakElev: number }[] = [];
      const quadrantCounts = { topLeft: 0, topRight: 0, bottomLeft: 0, bottomRight: 0 };
      let totalPeakElevSum = 0;

      for (let i = 0; i < NUM_SEEDS; i++) {
        const seed = `empirical_stat_seed_${i}_${i * 31 + 17}`;
        const terrain = generateTerrain(WIDTH, HEIGHT, seed);

        let maxElev = -Infinity;
        let maxX = 0;
        let maxY = 0;

        for (let y = 0; y < HEIGHT; y++) {
          for (let x = 0; x < WIDTH; x++) {
            const el = terrain.grid[y][x].elevation;
            expect(Number.isNaN(el)).toBe(false);
            expect(el).toBeGreaterThanOrEqual(0.0);
            expect(el).toBeLessThanOrEqual(100.0);

            if (el > maxElev) {
              maxElev = el;
              maxX = x;
              maxY = y;
            }
          }
        }

        peakCoords.push({ x: maxX, y: maxY, seed, peakElev: maxElev });
        totalPeakElevSum += maxElev;

        // Classify quadrant
        const isLeft = maxX < WIDTH / 2;
        const isTop = maxY < HEIGHT / 2;

        if (isTop && isLeft) quadrantCounts.topLeft++;
        else if (isTop && !isLeft) quadrantCounts.topRight++;
        else if (!isTop && isLeft) quadrantCounts.bottomLeft++;
        else quadrantCounts.bottomRight++;
      }

      // Calculate centroid and standard deviation of peak positions
      const meanX = peakCoords.reduce((sum, p) => sum + p.x, 0) / NUM_SEEDS;
      const meanY = peakCoords.reduce((sum, p) => sum + p.y, 0) / NUM_SEEDS;

      const stdX = Math.sqrt(
        peakCoords.reduce((sum, p) => sum + Math.pow(p.x - meanX, 2), 0) / NUM_SEEDS
      );
      const stdY = Math.sqrt(
        peakCoords.reduce((sum, p) => sum + Math.pow(p.y - meanY, 2), 0) / NUM_SEEDS
      );

      // Check distance of peaks from (0,0) origin
      const originClusteredPeaks = peakCoords.filter(
        (p) => p.x < WIDTH * 0.15 && p.y < HEIGHT * 0.15
      );

      // Assertions
      // 1. Peak elevation must not be clustered at (0,0)
      expect(originClusteredPeaks.length).toBeLessThan(NUM_SEEDS * 0.25);

      // 2. Peaks must be distributed across multiple quadrants (at least 3 quadrants populated)
      const populatedQuadrants = Object.values(quadrantCounts).filter((c) => c > 0).length;
      expect(populatedQuadrants).toBeGreaterThanOrEqual(3);

      // 3. No single quadrant should hold > 70% of peak locations
      for (const count of Object.values(quadrantCounts)) {
        expect(count).toBeLessThanOrEqual(NUM_SEEDS * 0.7);
      }

      // 4. Centroid of peak X and Y should be well spread out away from (0,0) edge
      expect(meanX).toBeGreaterThan(WIDTH * 0.15);
      expect(meanX).toBeLessThan(WIDTH * 0.85);
      expect(meanY).toBeGreaterThan(HEIGHT * 0.15);
      expect(meanY).toBeLessThan(HEIGHT * 0.85);

      // 5. Standard deviation of peak coordinates must reflect spatial dispersion across map
      expect(stdX).toBeGreaterThan(8.0);
      expect(stdY).toBeGreaterThan(8.0);

      // 6. Mean peak elevation across seeds must be substantial (> 20.0)
      const meanPeakElev = totalPeakElevSum / NUM_SEEDS;
      expect(meanPeakElev).toBeGreaterThan(20.0);
    });
  });

  describe('Empirical Test 2: Smoothness & Continuity across Grid Boundaries', () => {
    it('should maintain continuous elevation gradients without unnatural step spikes', () => {
      const WIDTH = 64;
      const HEIGHT = 64;
      const terrain = generateTerrain(WIDTH, HEIGHT, 'smoothness_test_seed');

      let maxGradientX = 0;
      let maxGradientY = 0;
      let totalGradientX = 0;
      let totalGradientY = 0;
      let sampleCount = 0;

      for (let y = 0; y < HEIGHT; y++) {
        for (let x = 0; x < WIDTH; x++) {
          const current = terrain.grid[y][x].elevation;

          if (x + 1 < WIDTH) {
            const nextX = terrain.grid[y][x + 1].elevation;
            const diffX = Math.abs(nextX - current);
            maxGradientX = Math.max(maxGradientX, diffX);
            totalGradientX += diffX;
          }

          if (y + 1 < HEIGHT) {
            const nextY = terrain.grid[y + 1][x].elevation;
            const diffY = Math.abs(nextY - current);
            maxGradientY = Math.max(maxGradientY, diffY);
            totalGradientY += diffY;
          }

          sampleCount++;
        }
      }

      const avgGradientX = totalGradientX / sampleCount;
      const avgGradientY = totalGradientY / sampleCount;

      // Average gradient per cell should be smooth (<= 3.5 height units per cell)
      expect(avgGradientX).toBeLessThan(3.5);
      expect(avgGradientY).toBeLessThan(3.5);

      // Max single-step gradient should be bounded (<= 30.0 height units)
      expect(maxGradientX).toBeLessThan(30.0);
      expect(maxGradientY).toBeLessThan(30.0);
    });

    it('should assert continuous biome transition weights across elevation, moisture, temperature, and slope sweeps', () => {
      // 1. Elevation sweep (0 to 100)
      for (let el = 0; el <= 100; el += 0.5) {
        const weights = getContinuousBiomeWeights(el, 50, 0.2, 0.1);
        let sum = 0;

        for (const biome of Object.keys(weights) as BiomeType[]) {
          const w = weights[biome];
          expect(Number.isNaN(w)).toBe(false);
          expect(w).toBeGreaterThanOrEqual(0.0);
          expect(w).toBeLessThanOrEqual(1.0);
          sum += w;
        }

        expect(sum).toBeCloseTo(1.0, 4);
      }

      // 2. Moisture sweep (0 to 100)
      for (let moist = 0; moist <= 100; moist += 0.5) {
        const weights = getContinuousBiomeWeights(40, moist, 0.2, 0.1);
        let sum = 0;

        for (const biome of Object.keys(weights) as BiomeType[]) {
          const w = weights[biome];
          expect(Number.isNaN(w)).toBe(false);
          expect(w).toBeGreaterThanOrEqual(0.0);
          expect(w).toBeLessThanOrEqual(1.0);
          sum += w;
        }

        expect(sum).toBeCloseTo(1.0, 4);
      }

      // 3. Temperature sweep (-1.0 to 1.5)
      for (let temp = -1.0; temp <= 1.5; temp += 0.05) {
        const weights = getContinuousBiomeWeights(40, 50, temp, 0.1);
        let sum = 0;

        for (const biome of Object.keys(weights) as BiomeType[]) {
          const w = weights[biome];
          expect(Number.isNaN(w)).toBe(false);
          expect(w).toBeGreaterThanOrEqual(0.0);
          expect(w).toBeLessThanOrEqual(1.0);
          sum += w;
        }

        expect(sum).toBeCloseTo(1.0, 4);
      }

      // 4. Smoothness of transition weight delta: step change in elevation of 0.5 should not cause sudden jump > 0.2 in weight
      let prevWeights = getContinuousBiomeWeights(0, 50, 0.2, 0.1);
      for (let el = 0.5; el <= 100; el += 0.5) {
        const currWeights = getContinuousBiomeWeights(el, 50, 0.2, 0.1);

        for (const biome of Object.keys(currWeights) as BiomeType[]) {
          const delta = Math.abs(currWeights[biome] - prevWeights[biome]);
          expect(delta).toBeLessThan(0.2);
        }

        prevWeights = currWeights;
      }
    });

    it('should assert chunk boundary serialization and elevation preservation', () => {
      const terrain = generateTerrain(32, 32, 'chunk_boundary_seed');
      const serialized = serializeChunk(terrain, 0, 0);
      const deserialized = deserializeChunk(serialized);

      expect(deserialized.width).toBe(32);
      expect(deserialized.height).toBe(32);

      for (let y = 0; y < 32; y++) {
        for (let x = 0; x < 32; x++) {
          const orig = terrain.grid[y][x];
          const restored = deserialized.grid[y][x];
          expect(restored.elevation).toBeCloseTo(orig.elevation, 4);
          expect(Math.abs(restored.moisture - orig.moisture)).toBeLessThanOrEqual(1.0);
          expect(restored.biome).toBe(orig.biome);
        }
      }
    });
  });

  describe('Empirical Test 3: Continuous Mountain Ranges & Connected Component Analysis Across 30 Seeds', () => {
    it('should verify mountain cells form connected continuous ranges (not fragmented single isolated spikes)', () => {
      const NUM_SEEDS = 30;
      const WIDTH = 64;
      const HEIGHT = 64;
      let validMountainMaps = 0;
      let totalMaxComponentSize = 0;
      let totalIsolatedMountainCells = 0;
      let totalMountainCells = 0;

      for (let s = 0; s < NUM_SEEDS; s++) {
        const seed = `mountain_continuity_seed_${s * 101 + 43}`;
        const terrain = generateTerrain(WIDTH, HEIGHT, seed);

        // Find 80th percentile elevation threshold for mountain classification
        const elevations: number[] = [];
        for (let y = 0; y < HEIGHT; y++) {
          for (let x = 0; x < WIDTH; x++) {
            elevations.push(terrain.grid[y][x].elevation);
          }
        }
        elevations.sort((a, b) => a - b);
        const threshold = elevations[Math.floor(elevations.length * 0.70)]; // Top 30% elevation

        // Create binary mountain mask
        const isMountain: boolean[][] = Array.from({ length: HEIGHT }, (_, y) =>
          Array.from({ length: WIDTH }, (_, x) => terrain.grid[y][x].elevation >= threshold)
        );

        // Connected Component Analysis (8-connectivity BFS)
        const visited: boolean[][] = Array.from({ length: HEIGHT }, () =>
          Array(WIDTH).fill(false)
        );
        const componentSizes: number[] = [];
        let seedIsolatedCount = 0;
        let seedMountainCount = 0;

        for (let y = 0; y < HEIGHT; y++) {
          for (let x = 0; x < WIDTH; x++) {
            if (isMountain[y][x] && !visited[y][x]) {
              // BFS to find connected component
              let compSize = 0;
              const queue: [number, number][] = [[x, y]];
              visited[y][x] = true;

              while (queue.length > 0) {
                const [cx, cy] = queue.shift()!;
                compSize++;

                for (let dy = -1; dy <= 1; dy++) {
                  for (let dx = -1; dx <= 1; dx++) {
                    if (dx === 0 && dy === 0) continue;
                    const nx = cx + dx;
                    const ny = cy + dy;
                    if (
                      nx >= 0 &&
                      nx < WIDTH &&
                      ny >= 0 &&
                      ny < HEIGHT &&
                      isMountain[ny][nx] &&
                      !visited[ny][nx]
                    ) {
                      visited[ny][nx] = true;
                      queue.push([nx, ny]);
                    }
                  }
                }
              }

              componentSizes.push(compSize);
              if (compSize === 1) seedIsolatedCount++;
              seedMountainCount += compSize;
            }
          }
        }

        if (componentSizes.length > 0) {
          validMountainMaps++;
          const maxComp = Math.max(...componentSizes);
          totalMaxComponentSize += maxComp;
          totalIsolatedMountainCells += seedIsolatedCount;
          totalMountainCells += seedMountainCount;

          // The largest mountain range in a map must span at least 8 connected cells
          expect(maxComp).toBeGreaterThanOrEqual(8);
        }
      }

      expect(validMountainMaps).toBe(NUM_SEEDS);

      // Average size of largest mountain component across seeds should be substantial
      const avgMaxComp = totalMaxComponentSize / NUM_SEEDS;
      expect(avgMaxComp).toBeGreaterThanOrEqual(15);

      // Isolated mountain cells (single 1x1 pixels with no mountain neighbors) must be very rare (< 12%)
      const isolatedRatio = totalIsolatedMountainCells / totalMountainCells;
      expect(isolatedRatio).toBeLessThan(0.12);
    });
  });

  describe('Empirical Test 4: Absence of 4-Sector Radial Artifacts & Seams Across 30 Seeds', () => {
    it('should verify heightmaps do not exhibit 4-fold radial harmonic energy or center axis discontinuities', () => {
      const NUM_SEEDS = 30;
      const WIDTH = 64;
      const HEIGHT = 64;
      const centerX = (WIDTH - 1) / 2;
      const centerY = (HEIGHT - 1) / 2;

      let totalF4EnergyRatio = 0;
      let maxAxisJumpRatio = 0;

      for (let s = 0; s < NUM_SEEDS; s++) {
        const seed = `radial_artifact_test_${s * 257 + 13}`;
        const terrain = generateTerrain(WIDTH, HEIGHT, seed);

        // 1. Cross-axis seam check: center line height jump vs average adjacent height jump
        let centerAxisJumpSum = 0;
        let generalJumpSum = 0;
        let centerSampleCount = 0;
        let generalSampleCount = 0;

        for (let y = 0; y < HEIGHT; y++) {
          for (let x = 0; x < WIDTH - 1; x++) {
            const diff = Math.abs(terrain.grid[y][x + 1].elevation - terrain.grid[y][x].elevation);
            if (x === Math.floor(WIDTH / 2) - 1) {
              centerAxisJumpSum += diff;
              centerSampleCount++;
            } else {
              generalJumpSum += diff;
              generalSampleCount++;
            }
          }
        }

        const avgCenterJump = centerAxisJumpSum / centerSampleCount;
        const avgGeneralJump = generalJumpSum / generalSampleCount;
        const axisJumpRatio = avgCenterJump / Math.max(0.001, avgGeneralJump);
        maxAxisJumpRatio = Math.max(maxAxisJumpRatio, axisJumpRatio);

        // Center axis jump should not exceed 2.5x general adjacent jump (no artificial seam along x=W/2)
        expect(axisJumpRatio).toBeLessThan(2.5);

        // 2. 4-fold angular harmonic energy test at radius r = 16 cells from center
        const r = 16;
        const numSamples = 64;
        let cos4Sum = 0;
        let sin4Sum = 0;
        let totalValSum = 0;

        for (let k = 0; k < numSamples; k++) {
          const theta = (2 * Math.PI * k) / numSamples;
          const sampleX = Math.round(centerX + r * Math.cos(theta));
          const sampleY = Math.round(centerY + r * Math.sin(theta));

          if (sampleX >= 0 && sampleX < WIDTH && sampleY >= 0 && sampleY < HEIGHT) {
            const val = terrain.grid[sampleY][sampleX].elevation;
            cos4Sum += val * Math.cos(4 * theta);
            sin4Sum += val * Math.sin(4 * theta);
            totalValSum += val;
          }
        }

        const f4Amplitude = Math.sqrt(cos4Sum * cos4Sum + sin4Sum * sin4Sum);
        const f4Ratio = f4Amplitude / Math.max(1.0, totalValSum);
        totalF4EnergyRatio += f4Ratio;

        // 4th harmonic (4-sector periodicity) ratio should be very low (< 0.20)
        expect(f4Ratio).toBeLessThan(0.20);
      }

      const meanF4EnergyRatio = totalF4EnergyRatio / NUM_SEEDS;
      expect(meanF4EnergyRatio).toBeLessThan(0.12);
    });
  });

  describe('Empirical Test 5: Absence of Center Round Holes & Artificial Center Sinkholes', () => {
    it('should verify the map center is not artificially carved down into a crater or round hole', () => {
      const NUM_SEEDS = 30;
      const WIDTH = 64;
      const HEIGHT = 64;
      const cx = (WIDTH - 1) / 2;
      const cy = (HEIGHT - 1) / 2;

      let validCenterElevCount = 0;
      let totalCenterMean = 0;
      let totalAnnulusMean = 0;

      for (let s = 0; s < NUM_SEEDS; s++) {
        const seed = `center_hole_test_${s * 389 + 71}`;
        const terrain = generateTerrain(WIDTH, HEIGHT, seed);

        let centerSum = 0;
        let centerCount = 0;
        let annulusSum = 0;
        let annulusCount = 0;

        for (let y = 0; y < HEIGHT; y++) {
          for (let x = 0; x < WIDTH; x++) {
            const dx = x - cx;
            const dy = y - cy;
            const dist = Math.sqrt(dx * dx + dy * dy);
            const el = terrain.grid[y][x].elevation;

            if (dist <= 6.0) {
              // Inner center core (r <= 6)
              centerSum += el;
              centerCount++;
            } else if (dist > 8.0 && dist <= 20.0) {
              // Annulus ring surrounding center (8 < r <= 20)
              annulusSum += el;
              annulusCount++;
            }
          }
        }

        const centerMean = centerSum / centerCount;
        const annulusMean = annulusSum / annulusCount;

        totalCenterMean += centerMean;
        totalAnnulusMean += annulusMean;

        // Center mean elevation should be positive and valid
        expect(Number.isNaN(centerMean)).toBe(false);
        expect(centerMean).toBeGreaterThanOrEqual(0.0);

        // Center mean elevation across seeds should not be artificially pinned to near-zero (hole)
        if (centerMean > 3.0) {
          validCenterElevCount++;
        }
      }

      // Across 30 seeds, the map center must have varied natural elevation (> 3.0) in the vast majority of seeds
      expect(validCenterElevCount).toBeGreaterThanOrEqual(NUM_SEEDS * 0.70);

      // Mean center elevation across all 30 seeds must be natural and non-zero (> 10.0)
      const avgCenterMean = totalCenterMean / NUM_SEEDS;
      expect(avgCenterMean).toBeGreaterThan(10.0);
    });
  });

  describe('Empirical Test 6: Biome Spatial Distribution, Diversity & Multidimensional Transition Smoothness', () => {
    it('should verify all 11 biomes are representable across varied environmental parameters', () => {
      const allBiomes: BiomeType[] = [
        'ocean',
        'beach',
        'grassland',
        'forest',
        'taiga',
        'alpine rock',
        'snow peaks',
        'desert',
        'jungle',
        'volcanic',
        'glacier',
      ];

      const observedBiomes = new Set<BiomeType>();

      // Sweep elevation, moisture, and temperature combinations
      const testCases: { el: number; moist: number; temp?: number }[] = [
        { el: 1.5, moist: 50, temp: 0.0 },     // ocean
        { el: 4.0, moist: 50, temp: 0.0 },     // beach
        { el: 30.0, moist: 25, temp: 0.2 },    // grassland
        { el: 30.0, moist: 60, temp: 0.2 },    // forest
        { el: 70.0, moist: 60, temp: 0.2 },    // taiga
        { el: 70.0, moist: 20, temp: 0.2 },    // alpine rock
        { el: 85.0, moist: 50, temp: 0.2 },    // snow peaks
        { el: 30.0, moist: 10, temp: 0.8 },    // desert
        { el: 30.0, moist: 80, temp: 0.8 },    // jungle
        { el: 50.0, moist: 30, temp: 1.05 },   // volcanic
        { el: 50.0, moist: 50, temp: -0.8 },   // glacier
      ];

      for (const tc of testCases) {
        const b = determineBiome(tc.el, tc.moist, tc.temp);
        observedBiomes.add(b);
      }

      // Assert all 11 biomes are produced by determineBiome
      for (const expectedBiome of allBiomes) {
        expect(observedBiomes.has(expectedBiome)).toBe(true);
      }
    });

    it('should verify spatial grid cell adjacent biomes transition logically without invalid extreme jumps', () => {
      const terrain = generateTerrain(64, 64, 'spatial_biome_transition_seed');

      let totalTransitions = 0;
      let abruptJumps = 0;

      for (let y = 0; y < 64; y++) {
        for (let x = 0; x < 63; x++) {
          const b1 = terrain.grid[y][x].biome;
          const b2 = terrain.grid[y][x + 1].biome;

          if (b1 !== b2) {
            totalTransitions++;
            // Check for direct extreme physical implausible adjacent pairs e.g. ocean <-> snow peaks or desert <-> glacier
            if (
              (b1 === 'ocean' && b2 === 'snow peaks') ||
              (b1 === 'snow peaks' && b2 === 'ocean') ||
              (b1 === 'desert' && b2 === 'glacier') ||
              (b1 === 'glacier' && b2 === 'desert')
            ) {
              abruptJumps++;
            }
          }
        }
      }

      expect(totalTransitions).toBeGreaterThan(0);
      expect(abruptJumps).toBe(0);
    });
  });
});
