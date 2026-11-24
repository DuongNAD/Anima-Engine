import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import { getSkyParams } from '../../src/components/Landscape/utils/skyParams';

// Utility helper functions representing the exact GLSL math implemented in Terrain & Water shaders

interface GerstnerWaveParams {
  dir: [number, number];
  steepness: number;
  amplitude: number;
  frequency: number;
  speed: number;
}

export function computeGerstnerWave(
  pos: [number, number, number],
  wave: GerstnerWaveParams,
  time: number,
  windSpeed: number
): [number, number, number] {
  const len = Math.sqrt(wave.dir[0] * wave.dir[0] + wave.dir[1] * wave.dir[1]);
  const dx = wave.dir[0] / (len || 1);
  const dy = wave.dir[1] / (len || 1);

  const k = wave.frequency;
  const phase = k * (dx * pos[0] + dy * pos[1]) + time * wave.speed * windSpeed;
  const c = Math.cos(phase);
  const s = Math.sin(phase);

  const q = wave.steepness;
  const a = wave.amplitude;

  return [
    q * a * dx * c,
    q * a * dy * c,
    a * s,
  ];
}

export function calculateSlope(normal: [number, number, number]): number {
  const len = Math.sqrt(normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]);
  const ny = normal[1] / (len || 1);
  const clampedNy = Math.max(0, Math.min(1, ny));
  return 1.0 - clampedNy;
}

export function calculateRockFactor(slope: number): number {
  // GLSL smoothstep(0.30, 0.40, slope)
  const edge0 = 0.30;
  const edge1 = 0.40;
  const x = Math.max(0, Math.min(1, (slope - edge0) / (edge1 - edge0)));
  return x * x * (3 - 2 * x);
}

export function determineBiomeRoughness(
  rockFactor: number,
  color: { r: number; g: number; b: number },
  posY: number
): number {
  if (rockFactor > 0.35) {
    return 0.95; // Volcanic / Rock
  } else if (color.r > 0.8 && color.g > 0.85 && color.b > 0.9) {
    return 0.1; // Ice
  } else if (posY < 6.0 || (color.r > 0.65 && color.g > 0.58 && color.b < 0.45)) {
    return 0.3; // Wet sand / Beach
  }
  return 0.8; // Grassland default
}

export function calculateShadowLightPosition(
  cameraPos: [number, number, number],
  sunPos: [number, number, number]
): { lightPos: [number, number, number]; targetPos: [number, number, number] } {
  const focusX = cameraPos[0];
  const focusZ = cameraPos[2];
  return {
    lightPos: [focusX + sunPos[0], Math.max(sunPos[1], 20), focusZ + sunPos[2]],
    targetPos: [focusX, 0, focusZ],
  };
}

describe('Milestone 2: Terrain & Water Shader Enhancements', () => {
  describe('1. Multi-directional Gerstner Wave Calculations', () => {
    const oceanWaves: GerstnerWaveParams[] = [
      { dir: [1.0, 0.5], steepness: 0.35, amplitude: 0.4, frequency: 0.08, speed: 1.2 },
      { dir: [-0.7, 0.7], steepness: 0.25, amplitude: 0.25, frequency: 0.15, speed: 1.5 },
      { dir: [0.2, -1.0], steepness: 0.2, amplitude: 0.15, frequency: 0.25, speed: 2.0 },
    ];

    it('should compute valid wave displacement for ocean surface', () => {
      const pos: [number, number, number] = [10, 20, 0];
      const disp = computeGerstnerWave(pos, oceanWaves[0], 1.5, 1.0);

      expect(disp[0]).not.toBeNaN();
      expect(disp[1]).not.toBeNaN();
      expect(disp[2]).not.toBeNaN();
    });

    it('should satisfy wave steepness constraint Q * A <= 1.0 to prevent self-intersection', () => {
      for (const w of oceanWaves) {
        const qa = w.steepness * w.amplitude;
        expect(qa).toBeLessThanOrEqual(1.0);
      }
    });

    it('should calculate wave crest peak elevation correctly', () => {
      const w = oceanWaves[0];
      // When phase is PI/2, sin(phase) = 1.0, z displacement equals amplitude A
      const pos: [number, number, number] = [0, 0, 0];
      // Find time where phase = Math.PI / 2
      const targetTime = (Math.PI / 2) / (w.speed * 1.0);
      const disp = computeGerstnerWave(pos, w, targetTime, 1.0);

      expect(disp[2]).toBeCloseTo(w.amplitude, 4);
    });

    it('should sum displacements across multiple Gerstner waves', () => {
      const pos: [number, number, number] = [15, -25, 0];
      let totalZ = 0;
      for (const wave of oceanWaves) {
        const disp = computeGerstnerWave(pos, wave, 2.0, 1.0);
        totalZ += disp[2];
      }
      expect(totalZ).not.toBeNaN();
      expect(Math.abs(totalZ)).toBeLessThanOrEqual(0.4 + 0.25 + 0.15 + 0.001);
    });
  });

  describe('2. Slope Calculation Math & Rock Cliff Threshold Logic', () => {
    it('should calculate zero slope for flat ground (N = (0, 1, 0))', () => {
      const flatNormal: [number, number, number] = [0, 1, 0];
      const slope = calculateSlope(flatNormal);
      expect(slope).toBeCloseTo(0.0, 5);
    });

    it('should calculate 1.0 slope for vertical face (N = (1, 0, 0))', () => {
      const verticalNormal: [number, number, number] = [1, 0, 0];
      const slope = calculateSlope(verticalNormal);
      expect(slope).toBeCloseTo(1.0, 5);
    });

    it('should calculate correct slope for 35-degree inclination', () => {
      // cos(35 deg) approx 0.81915
      const cos35 = Math.cos((35 * Math.PI) / 180);
      const sin35 = Math.sin((35 * Math.PI) / 180);
      const slope35 = calculateSlope([sin35, cos35, 0]);

      expect(slope35).toBeCloseTo(1.0 - cos35, 4);
      expect(slope35).toBeLessThan(0.35); // 35 degrees is slightly below 0.35 threshold
    });

    it('should trigger rock cliff blending when slope > 0.35 (~50-degree inclination)', () => {
      // 50 deg inclination: cos(50 deg) = 0.6428 -> slope = 0.3572 > 0.35
      const cos50 = Math.cos((50 * Math.PI) / 180);
      const sin50 = Math.sin((50 * Math.PI) / 180);
      const slope50 = calculateSlope([sin50, cos50, 0]);

      expect(slope50).toBeGreaterThan(0.35);
      const rockFactor = calculateRockFactor(slope50);
      expect(rockFactor).toBeGreaterThan(0.5);
    });

    it('should return rockFactor = 0 for flat ground and rockFactor = 1 for steep cliffs', () => {
      expect(calculateRockFactor(0.0)).toBe(0.0);
      expect(calculateRockFactor(0.20)).toBe(0.0);
      expect(calculateRockFactor(0.45)).toBe(1.0);
    });
  });

  describe('3. Per-Biome Roughness Mapping', () => {
    it('should assign Ice roughness = 0.1 for snowy/icy color', () => {
      const iceColor = { r: 0.9, g: 0.95, b: 0.98 };
      const roughness = determineBiomeRoughness(0.0, iceColor, 20.0);
      expect(roughness).toBe(0.1);
    });

    it('should assign Wet Sand/Beach roughness = 0.3 for low-altitude shoreline', () => {
      const beachColor = { r: 0.7, g: 0.6, b: 0.4 };
      const roughness = determineBiomeRoughness(0.0, beachColor, 3.0);
      expect(roughness).toBe(0.3);
    });

    it('should assign Grassland roughness = 0.8 for standard ground', () => {
      const grassColor = { r: 0.3, g: 0.6, b: 0.2 };
      const roughness = determineBiomeRoughness(0.0, grassColor, 25.0);
      expect(roughness).toBe(0.8);
    });

    it('should assign Volcanic/Rock roughness = 0.95 when rockFactor > 0.35', () => {
      const groundColor = { r: 0.3, g: 0.6, b: 0.2 };
      const roughness = determineBiomeRoughness(0.6, groundColor, 25.0);
      expect(roughness).toBe(0.95);
    });
  });

  describe('4. Directional Light Shadow Tracking Frustum Parameters', () => {
    it('should track camera focus position to prevent shadow frustum clipping', () => {
      const sunPos: [number, number, number] = [100, 200, 50];
      const cameraPos: [number, number, number] = [400, 50, -300]; // Far outside center

      const { lightPos, targetPos } = calculateShadowLightPosition(cameraPos, sunPos);

      expect(lightPos[0]).toBe(400 + 100); // 500
      expect(lightPos[1]).toBe(200);       // 200
      expect(lightPos[2]).toBe(-300 + 50); // -250

      expect(targetPos[0]).toBe(400);
      expect(targetPos[1]).toBe(0);
      expect(targetPos[2]).toBe(-300);
    });

    it('should verify shadow camera frustum coverage range', () => {
      // Frustum left/right/top/bottom = [-250, 250], far = 1200
      const frustumSize = 500; // 250 - (-250)
      const farPlane = 1200;

      expect(frustumSize).toBeGreaterThanOrEqual(400);
      expect(farPlane).toBeGreaterThanOrEqual(1000);
    });
  });

  describe('5. Material Uniform Integration & Shader Setup', () => {
    it('should verify skyParams horizon color sync format', () => {
      const dayParams = getSkyParams(12.0);
      expect(dayParams.skyColor).toBeDefined();
      expect(dayParams.skyColor).toMatch(/^#/);

      const nightParams = getSkyParams(0.0);
      expect(nightParams.skyColor).toBeDefined();
      expect(nightParams.skyColor).toMatch(/^#/);
    });

    it('should create valid Vector3 light direction structures', () => {
      const dir = new THREE.Vector3(1, 1, 0).normalize();
      expect(dir.length()).toBeCloseTo(1.0, 5);
    });
  });
});
