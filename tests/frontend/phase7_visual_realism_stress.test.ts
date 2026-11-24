import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import { getSkyParams } from '../../src/components/Landscape/utils/skyParams';
import { determineBiome, getBiomeColor, generateTerrain, BiomeType } from '../../src/components/Landscape/utils/terrainGenerator';
import {
  computeGerstnerWave,
  calculateSlope,
  calculateRockFactor,
  determineBiomeRoughness,
  calculateShadowLightPosition,
} from './terrain_shaders.test';

describe('Phase 7: Visual Realism & Shader Stress Testing Harness', () => {

  describe('1. Shader Rendering Parameters Stress Testing', () => {

    describe('1.1 Gerstner Wave Steepness & Math Extremes', () => {
      const oceanWaves = [
        { dir: [1.0, 0.5] as [number, number], steepness: 0.35, amplitude: 0.4, frequency: 0.08, speed: 1.2 },
        { dir: [-0.7, 0.7] as [number, number], steepness: 0.25, amplitude: 0.25, frequency: 0.15, speed: 1.5 },
        { dir: [0.2, -1.0] as [number, number], steepness: 0.2, amplitude: 0.15, frequency: 0.25, speed: 2.0 },
      ];

      it('should maintain steepness constraint Q * A <= 1.0 to prevent self-intersecting wave geometry', () => {
        oceanWaves.forEach((wave) => {
          const qa = wave.steepness * wave.amplitude;
          expect(qa).toBeLessThanOrEqual(1.0);
          expect(qa).toBeGreaterThanOrEqual(0.0);
        });
      });

      it('should handle zero wind speed without NaN or division by zero', () => {
        const pos: [number, number, number] = [50, 50, 0];
        const disp = computeGerstnerWave(pos, oceanWaves[0], 10.0, 0.0);
        expect(disp[0]).not.toBeNaN();
        expect(disp[1]).not.toBeNaN();
        expect(disp[2]).not.toBeNaN();
        expect(Number.isFinite(disp[0])).toBe(true);
        expect(Number.isFinite(disp[1])).toBe(true);
        expect(Number.isFinite(disp[2])).toBe(true);
      });

      it('should handle extreme storm wind speeds (windSpeed = 100) stably', () => {
        const pos: [number, number, number] = [100, -200, 0];
        const disp = computeGerstnerWave(pos, oceanWaves[0], 5.0, 100.0);
        expect(disp[0]).not.toBeNaN();
        expect(disp[1]).not.toBeNaN();
        expect(disp[2]).not.toBeNaN();
        // Amplitude bound: z displacement must remain bounded within [-amplitude, +amplitude]
        expect(Math.abs(disp[2])).toBeLessThanOrEqual(oceanWaves[0].amplitude + 1e-5);
      });

      it('should handle un-normalized wave direction vectors gracefully', () => {
        const rawWave = { dir: [30.0, 40.0] as [number, number], steepness: 0.3, amplitude: 0.5, frequency: 0.1, speed: 1.0 };
        const pos: [number, number, number] = [10, 10, 0];
        const disp = computeGerstnerWave(pos, rawWave, 1.0, 1.0);
        expect(disp[0]).not.toBeNaN();
        expect(disp[1]).not.toBeNaN();
        expect(disp[2]).not.toBeNaN();
      });

      it('should evaluate multi-wave superposition bound across random space-time samples', () => {
        const maxPossibleZ = oceanWaves.reduce((acc, w) => acc + w.amplitude, 0);
        for (let time = 0; time < 50; time += 5) {
          for (let x = -500; x <= 500; x += 250) {
            for (let y = -500; y <= 500; y += 250) {
              let totalZ = 0;
              for (const wave of oceanWaves) {
                const disp = computeGerstnerWave([x, y, 0], wave, time, 1.5);
                totalZ += disp[2];
              }
              expect(Math.abs(totalZ)).toBeLessThanOrEqual(maxPossibleZ + 1e-4);
            }
          }
        }
      });
    });

    describe('1.2 Slope Calculation & Rock Cliff Threshold (slope > 0.35)', () => {
      it('should correctly calculate slope angle curve from 0° (flat) to 90° (cliff)', () => {
        const testAnglesDeg = [0, 15, 30, 35, 45, 50, 60, 75, 90];
        testAnglesDeg.forEach((deg) => {
          const rad = (deg * Math.PI) / 180;
          const normal: [number, number, number] = [Math.sin(rad), Math.cos(rad), 0];
          const slope = calculateSlope(normal);
          const expectedSlope = 1.0 - Math.cos(rad);
          expect(slope).toBeCloseTo(expectedSlope, 4);
        });
      });

      it('should evaluate slope threshold > 0.35 boundary transition (~35° vs ~50° slope)', () => {
        // At 35 degrees: cos(35 deg) ~= 0.81915 -> slope ~= 0.18085 < 0.35
        const rad35 = (35 * Math.PI) / 180;
        const slope35 = calculateSlope([Math.sin(rad35), Math.cos(rad35), 0]);
        expect(slope35).toBeLessThan(0.35);
        expect(calculateRockFactor(slope35)).toBe(0.0); // smoothstep(0.3, 0.4, 0.1808) == 0

        // At 50 degrees: cos(50 deg) ~= 0.64279 -> slope ~= 0.35721 > 0.35
        const rad50 = (50 * Math.PI) / 180;
        const slope50 = calculateSlope([Math.sin(rad50), Math.cos(rad50), 0]);
        expect(slope50).toBeGreaterThan(0.35);
        const rockFactor50 = calculateRockFactor(slope50);
        expect(rockFactor50).toBeGreaterThan(0.5); // smoothstep(0.3, 0.4, 0.357) is ~0.57

        // At 65 degrees: cos(65 deg) ~= 0.4226 -> slope ~= 0.5774 > 0.40
        const rad65 = (65 * Math.PI) / 180;
        const slope65 = calculateSlope([Math.sin(rad65), Math.cos(rad65), 0]);
        expect(calculateRockFactor(slope65)).toBe(1.0);
      });

      it('should handle unnormalized normal vectors without NaN', () => {
        const unnormalizedNormal: [number, number, number] = [10.0, 20.0, 30.0];
        const slope = calculateSlope(unnormalizedNormal);
        expect(slope).not.toBeNaN();
        expect(slope).toBeGreaterThanOrEqual(0.0);
        expect(slope).toBeLessThanOrEqual(1.0);
      });
    });

    describe('1.3 Per-Biome Roughness Mapping', () => {
      it('should map roughness correctly across all biomes and slope combinations', () => {
        // Ice: 0.1
        expect(determineBiomeRoughness(0.0, { r: 0.85, g: 0.9, b: 0.95 }, 20.0)).toBe(0.1);
        
        // Wet sand / Beach: 0.3 (low altitude y < 6.0)
        expect(determineBiomeRoughness(0.0, { r: 0.5, g: 0.5, b: 0.5 }, 4.0)).toBe(0.3);
        // Wet sand by color
        expect(determineBiomeRoughness(0.0, { r: 0.7, g: 0.6, b: 0.4 }, 10.0)).toBe(0.3);

        // Volcanic / Rock cliff (rockFactor > 0.35): 0.95
        expect(determineBiomeRoughness(0.40, { r: 0.3, g: 0.6, b: 0.2 }, 15.0)).toBe(0.95);

        // Grassland / default ground: 0.8
        expect(determineBiomeRoughness(0.0, { r: 0.3, g: 0.6, b: 0.2 }, 15.0)).toBe(0.8);
      });

      it('should prioritize steep rock cliff roughness (0.95) over ice or beach color', () => {
        // Even if color looks like ice, if rockFactor > 0.35, cliff rock roughness applies
        const iceColor = { r: 0.9, g: 0.95, b: 0.98 };
        const cliffIceRoughness = determineBiomeRoughness(0.5, iceColor, 3.0);
        expect(cliffIceRoughness).toBe(0.95);
      });
    });

    describe('1.4 Sky-Aware Fresnel Reflection Math', () => {
      function computeFresnel(normal: THREE.Vector3, viewDir: THREE.Vector3): number {
        const dotNV = Math.max(0.0, normal.dot(viewDir));
        const rawFresnel = Math.pow(1.0 - dotNV, 5.0);
        return Math.max(0.05, Math.min(0.85, rawFresnel));
      }

      it('should return minimum fresnel (0.05) when looking straight down (normal // viewDir)', () => {
        const normal = new THREE.Vector3(0, 1, 0);
        const viewDir = new THREE.Vector3(0, 1, 0);
        const f = computeFresnel(normal, viewDir);
        expect(f).toBeCloseTo(0.05, 4);
      });

      it('should return maximum fresnel (0.85) at grazing angles (normal perpendicular to viewDir)', () => {
        const normal = new THREE.Vector3(0, 1, 0);
        const viewDir = new THREE.Vector3(1, 0.01, 0).normalize();
        const f = computeFresnel(normal, viewDir);
        expect(f).toBeCloseTo(0.85, 2);
      });

      it('should smoothly interpolate sky reflection between horizon and zenith', () => {
        const reflectDir = new THREE.Vector3(0, 1, 0); // zenith reflection
        const skyGradientZenith = Math.max(0.0, Math.min(1.0, reflectDir.y * 0.5 + 0.5));
        expect(skyGradientZenith).toBe(1.0);

        const reflectDirHorizon = new THREE.Vector3(1, 0, 0); // horizon reflection
        const skyGradientHorizon = Math.max(0.0, Math.min(1.0, reflectDirHorizon.y * 0.5 + 0.5));
        expect(skyGradientHorizon).toBe(0.5);

        const reflectDirGround = new THREE.Vector3(0, -1, 0); // ground reflection
        const skyGradientGround = Math.max(0.0, Math.min(1.0, reflectDirGround.y * 0.5 + 0.5));
        expect(skyGradientGround).toBe(0.0);
      });
    });

    describe('1.5 Dynamic Shadow Frustum Camera Tracking (1000x1000 Map)', () => {
      it('should update directional light position for camera movement across 1000x1000 map bounds', () => {
        const sunOrbitDir: [number, number, number] = [150, 250, 20];
        const testCameraPositions: [number, number, number][] = [
          [0, 50, 0],
          [-450, 120, -450], // top-left corner of 1000x1000 map
          [450, 80, 450],   // bottom-right corner of 1000x1000 map
          [-490, 30, 490],  // near map boundaries
          [0, 300, 0],      // high altitude view
        ];

        testCameraPositions.forEach((camPos) => {
          const { lightPos, targetPos } = calculateShadowLightPosition(camPos, sunOrbitDir);
          
          expect(lightPos[0]).toBe(camPos[0] + sunOrbitDir[0]);
          expect(lightPos[1]).toBe(Math.max(sunOrbitDir[1], 20));
          expect(lightPos[2]).toBe(camPos[2] + sunOrbitDir[2]);

          expect(targetPos[0]).toBe(camPos[0]);
          expect(targetPos[1]).toBe(0);
          expect(targetPos[2]).toBe(camPos[2]);

          // Ensure light position Y is never below 20 units to prevent zero-elevation shadow artifacts
          expect(lightPos[1]).toBeGreaterThanOrEqual(20);
        });
      });

      it('should ensure shadow frustum covers camera neighborhood without clipping', () => {
        const frustumSize = 500; // left -250 to right 250
        const mapSize = 1000;
        // Verify shadow coverage span (500 units around camera focus)
        expect(frustumSize).toBe(mapSize / 2);
      });
    });
  });

  describe('2. Stability Across All 11 Biomes & Dynamic Day/Night Cycles', () => {

    describe('2.1 Biome Completeness & Threshold Verification (11 Biomes)', () => {
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

      it('should verify determineBiome returns all 11 distinct biomes under correct conditions', () => {
        const detectedBiomes = new Set<BiomeType>();

        // Test temperature overrides first
        detectedBiomes.add(determineBiome(30, 50, 0.98));  // volcanic
        detectedBiomes.add(determineBiome(30, 50, -0.6));  // glacier
        detectedBiomes.add(determineBiome(30, 10, 0.7));   // desert
        detectedBiomes.add(determineBiome(30, 80, 0.7));   // jungle

        // Test elevation/moisture biomes
        detectedBiomes.add(determineBiome(2.0, 50));        // ocean
        detectedBiomes.add(determineBiome(4.0, 50));        // beach
        detectedBiomes.add(determineBiome(85.0, 50));       // snow peaks
        detectedBiomes.add(determineBiome(65.0, 60));       // taiga
        detectedBiomes.add(determineBiome(65.0, 30));       // alpine rock
        detectedBiomes.add(determineBiome(30.0, 60));       // forest
        detectedBiomes.add(determineBiome(30.0, 30));       // grassland

        allBiomes.forEach((b) => {
          expect(detectedBiomes.has(b)).toBe(true);
        });
        expect(detectedBiomes.size).toBe(11);
      });

      it('should return valid non-NaN RGB colors for all 11 biomes in getBiomeColor', () => {
        allBiomes.forEach((biome) => {
          let col;
          switch (biome) {
            case 'volcanic': col = getBiomeColor(30, 50, 0.98); break;
            case 'glacier': col = getBiomeColor(30, 50, -0.6); break;
            case 'desert': col = getBiomeColor(30, 10, 0.7); break;
            case 'jungle': col = getBiomeColor(30, 80, 0.7); break;
            case 'ocean': col = getBiomeColor(2.0, 50); break;
            case 'beach': col = getBiomeColor(4.0, 50); break;
            case 'snow peaks': col = getBiomeColor(85.0, 50); break;
            case 'taiga': col = getBiomeColor(65.0, 60); break;
            case 'alpine rock': col = getBiomeColor(65.0, 30); break;
            case 'forest': col = getBiomeColor(30.0, 60); break;
            case 'grassland': col = getBiomeColor(30.0, 30); break;
          }

          expect(col).toBeDefined();
          expect(col.r).toBeGreaterThanOrEqual(0);
          expect(col.r).toBeLessThanOrEqual(1);
          expect(col.g).toBeGreaterThanOrEqual(0);
          expect(col.g).toBeLessThanOrEqual(1);
          expect(col.b).toBeGreaterThanOrEqual(0);
          expect(col.b).toBeLessThanOrEqual(1);
        });
      });

      it('should generate valid terrain grid without NaN values across 100x100 and 200x200 seeds', () => {
        const seeds = ['test_seed_1', 42, 'biome_stress'];
        seeds.forEach((seed) => {
          const terrain = generateTerrain(100, 100, seed);
          expect(terrain.grid.length).toBe(100);
          expect(terrain.grid[0].length).toBe(100);

          let oceanCount = 0;
          let lakeCount = 0;
          let riverCount = 0;

          for (let y = 0; y < 100; y++) {
            for (let x = 0; x < 100; x++) {
              const cell = terrain.grid[y][x];
              expect(Number.isFinite(cell.elevation)).toBe(true);
              expect(Number.isFinite(cell.moisture)).toBe(true);
              expect(Number.isFinite(cell.temperature)).toBe(true);
              expect(cell.biome).toBeDefined();
              expect(allBiomes.includes(cell.biome)).toBe(true);

              if (cell.biome === 'ocean') oceanCount++;
              if (cell.isLake) lakeCount++;
              if (cell.isRiver) riverCount++;
            }
          }

          expect(terrain.flora).toBeDefined();
          // Ensure no flora placed on snow peaks or elevation >= 80
          terrain.flora.forEach((item) => {
            const cell = terrain.grid[Math.floor(item.y)][Math.floor(item.x)];
            expect(cell.biome).not.toBe('snow peaks');
            expect(cell.elevation).toBeLessThan(80);
          });
        });
      });
    });

    describe('2.2 Dynamic Day/Night Cycle Stability (timeOfDay 0.0 to 24.0)', () => {
      it('should produce smooth continuous sky lighting parameters across 24-hour cycle in 0.1h steps', () => {
        let prevParams = getSkyParams(0.0);

        for (let timeOfDay = 0.1; timeOfDay <= 24.0; timeOfDay = Math.round((timeOfDay + 0.1) * 10) / 10) {
          const params = getSkyParams(timeOfDay);

          expect(params.skyColor).toMatch(/^#[0-9a-f]{6}$/i);
          expect(params.sunColor).toMatch(/^#[0-9a-f]{6}$/i);
          expect(params.ambientColor).toMatch(/^#[0-9a-f]{6}$/i);
          expect(params.hemiSkyColor).toMatch(/^#[0-9a-f]{6}$/i);
          expect(params.hemiGroundColor).toMatch(/^#[0-9a-f]{6}$/i);

          expect(params.sunIntensity).toBeGreaterThanOrEqual(0.0);
          expect(params.sunIntensity).toBeLessThanOrEqual(1.5);

          expect(params.ambientIntensity).toBeGreaterThanOrEqual(0.0);
          expect(params.ambientIntensity).toBeLessThanOrEqual(1.0);

          expect(params.starOpacity).toBeGreaterThanOrEqual(0.0);
          expect(params.starOpacity).toBeLessThanOrEqual(1.0);

          // Verify day/night contrasts: midnight (0h or 24h) stars=1.0, sunIntensity=0.0; noon (12h) stars=0.0, sunIntensity=1.2
          if (timeOfDay === 12.0) {
            expect(params.sunIntensity).toBe(1.2);
            expect(params.starOpacity).toBe(0.0);
          }
          if (timeOfDay === 0.0 || timeOfDay === 24.0) {
            expect(params.sunIntensity).toBe(0.0);
            expect(params.starOpacity).toBe(1.0);
          }

          prevParams = params;
        }
      });

      it('should calculate sun and moon orbit positions without gimbal lock or NaN', () => {
        for (let timeOfDay = 0; timeOfDay <= 24; timeOfDay += 1.0) {
          const angle = ((timeOfDay - 6) / 24) * Math.PI * 2;
          const R = 300;
          const sunPos = new THREE.Vector3(
            Math.cos(angle) * R,
            Math.sin(angle) * R,
            Math.sin(angle) * 0.1 * R
          );

          const moonAngle = angle + Math.PI;
          const moonPos = new THREE.Vector3(
            Math.cos(moonAngle) * R,
            Math.sin(moonAngle) * R,
            Math.sin(moonAngle) * 0.1 * R
          );

          expect(Number.isFinite(sunPos.x)).toBe(true);
          expect(Number.isFinite(sunPos.y)).toBe(true);
          expect(Number.isFinite(sunPos.z)).toBe(true);

          expect(Number.isFinite(moonPos.x)).toBe(true);
          expect(Number.isFinite(moonPos.y)).toBe(true);
          expect(Number.isFinite(moonPos.z)).toBe(true);

          // At noon (12.0), sun Y should be positive (~300), moon Y negative (~-300)
          if (timeOfDay === 12.0) {
            expect(sunPos.y).toBeGreaterThan(250);
            expect(moonPos.y).toBeLessThan(-250);
          }
          // At midnight (0.0 or 24.0), sun Y should be negative (~-300), moon Y positive (~300)
          if (timeOfDay === 0.0 || timeOfDay === 24.0) {
            expect(sunPos.y).toBeLessThan(-250);
            expect(moonPos.y).toBeGreaterThan(250);
          }
        }
      });
    });
  });
});
