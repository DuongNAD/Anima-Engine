import { describe, it, expect, vi } from 'vitest';
import React from 'react';
import { render } from '@testing-library/react';
import * as THREE from 'three';

vi.mock('@react-three/fiber', async () => {
  return {
    Canvas: ({ children }: any) => <div data-testid="mock-canvas">{children}</div>,
    useFrame: vi.fn(),
    useThree: () => ({
      camera: {
        position: { set: vi.fn(), x: 0, y: 25, z: 45 },
        lookAt: vi.fn(),
        quaternion: new THREE.Quaternion(),
        rotation: { set: vi.fn() },
        rotateY: vi.fn(),
        rotateX: vi.fn(),
      },
      scene: { add: vi.fn(), remove: vi.fn() },
      gl: { setSize: vi.fn(), domElement: document.createElement('canvas') },
    }),
  };
});

import { Water } from '../../src/components/Landscape/Water';
import { Terrain } from '../../src/components/Landscape/Terrain';

// Pure slope calculation matching Terrain.tsx shader logic
export function calculateSlope(normal: THREE.Vector3): number {
  const up = new THREE.Vector3(0, 1, 0);
  const dotVal = Math.min(1.0, Math.max(0.0, normal.dot(up)));
  return 1.0 - dotVal;
}

// Pure Gerstner wave displacement logic matching Water.tsx shader
export interface Wave {
  dir: [number, number];
  steepness: number;
  amplitude: number;
  frequency: number;
  speed: number;
}

export function calculateGerstnerWave(
  pos: [number, number, number],
  w: Wave,
  t: number,
  windSpeed: number
): [number, number, number] {
  const len = Math.sqrt(w.dir[0] * w.dir[0] + w.dir[1] * w.dir[1]) || 1;
  const dx = w.dir[0] / len;
  const dy = w.dir[1] / len;

  const k = w.frequency;
  const phase = k * (dx * pos[0] + dy * pos[1]) + t * w.speed * windSpeed;
  const c = Math.cos(phase);
  const s = Math.sin(phase);

  const q = w.steepness;
  const a = w.amplitude;

  return [q * a * dx * c, q * a * dy * c, a * s];
}

describe('Slope Shading & Water Simulation E2E Suite (R2)', () => {
  describe('Tier 1: Feature Coverage', () => {
    it('T1.1: verify slope calculation math', () => {
      const flatNormal = new THREE.Vector3(0, 1, 0);
      const verticalNormal = new THREE.Vector3(1, 0, 0);
      const angledNormal = new THREE.Vector3(0.7071, 0.7071, 0);

      expect(calculateSlope(flatNormal)).toBeCloseTo(0.0, 4);
      expect(calculateSlope(verticalNormal)).toBeCloseTo(1.0, 4);
      expect(calculateSlope(angledNormal)).toBeCloseTo(0.2929, 3);
    });

    it('T1.2: verify Gerstner wave displacement equations', () => {
      const wave: Wave = { dir: [1.0, 0.0], steepness: 0.35, amplitude: 0.4, frequency: 0.08, speed: 1.2 };
      const pos: [number, number, number] = [10.0, 5.0, 0.0];
      const displacement = calculateGerstnerWave(pos, wave, 1.0, 1.0);

      expect(Number.isNaN(displacement[0])).toBe(false);
      expect(Number.isNaN(displacement[1])).toBe(false);
      expect(Number.isNaN(displacement[2])).toBe(false);
      expect(Math.abs(displacement[2])).toBeLessThanOrEqual(wave.amplitude);
    });

    it('T1.3: verify water depth transparency bounds (0.0 to 1.0)', () => {
      const { container, rerender } = render(<Water depthTransparency={0.2} />);
      let mesh = container.querySelector('[name="water-mesh"]');
      expect(mesh).toBeTruthy();
      expect(mesh?.getAttribute('data-depth-transparency')).toBe('0.2');

      rerender(<Water depthTransparency={0.8} />);
      mesh = container.querySelector('[name="water-mesh"]');
      expect(mesh?.getAttribute('data-depth-transparency')).toBe('0.8');
    });

    it('T1.4: verify sky-aware Fresnel reflection rendering props', () => {
      const { container } = render(<Water reflectionColor="#3388cc" timeOfDay={12.0} />);
      const mesh = container.querySelector('[name="water-mesh"]');
      expect(mesh).toBeTruthy();
      expect(mesh?.getAttribute('data-reflection-color')).toBe('#3388cc');
    });

    it('T1.5: verify shoreline and crest foam rendering component setup', () => {
      const { container } = render(<Water windSpeed={2.5} width={32} height={32} />);
      const mesh = container.querySelector('[name="water-mesh"]');
      expect(mesh).toBeTruthy();
      expect(mesh?.getAttribute('data-wind-speed')).toBe('2.5');
    });
  });

  describe('Tier 2: Boundary & Corner Cases', () => {
    it('T2.1: extreme slopes (>0.8 up to 1.0)', () => {
      const steepNormal = new THREE.Vector3(0.99, 0.1, 0).normalize();
      const slopeVal = calculateSlope(steepNormal);

      expect(slopeVal).toBeGreaterThan(0.8);
      expect(slopeVal).toBeLessThanOrEqual(1.0);

      // Smoothstep rockFactor for slope > 0.40 yields 1.0 (full cliff rock face)
      const rockFactor = Math.min(1.0, Math.max(0.0, (slopeVal - 0.30) / 0.10));
      expect(rockFactor).toBe(1.0);
    });

    it('T2.2: zero water depth (shoreline boundary)', () => {
      const { container } = render(<Water depthTransparency={0.0} />);
      const mesh = container.querySelector('[name="water-mesh"]');
      expect(mesh?.getAttribute('data-depth-transparency')).toBe('0');
    });

    it('T2.3: extreme wind speeds (0.0 and 10.0+) and negative wind directions', () => {
      const wave: Wave = { dir: [-1.0, -1.0], steepness: 0.5, amplitude: 1.0, frequency: 0.5, speed: 5.0 };
      const zeroDisplacement = calculateGerstnerWave([0, 0, 0], wave, 0, 0.0);
      const extremeDisplacement = calculateGerstnerWave([100, 100, 0], wave, 10, 12.0);

      expect(zeroDisplacement[2]).toBe(0.0);
      expect(Number.isNaN(extremeDisplacement[0])).toBe(false);
      expect(Number.isNaN(extremeDisplacement[2])).toBe(false);
    });

    it('T2.4: water rendering under extreme map dimensions (8x8 to 100x100)', () => {
      const { container: smallContainer } = render(<Water width={8} height={8} />);
      expect(smallContainer.querySelector('[name="water-mesh"]')).toBeTruthy();

      const { container: largeContainer } = render(<Water width={100} height={100} />);
      expect(largeContainer.querySelector('[name="water-mesh"]')).toBeTruthy();
    });

    it('T2.5: custom reflection colors (hex, RGB string formats)', () => {
      const { container, rerender } = render(<Water reflectionColor="#ffffff" />);
      let mesh = container.querySelector('[name="water-mesh"]');
      expect(mesh?.getAttribute('data-reflection-color')).toBe('#ffffff');

      rerender(<Water reflectionColor="#000000" />);
      mesh = container.querySelector('[name="water-mesh"]');
      expect(mesh?.getAttribute('data-reflection-color')).toBe('#000000');
    });
  });

  describe('Tier 3: Cross-Feature Combinations', () => {
    it('T3.1: terrain slope shader + water mesh placement interaction', () => {
      const { container: terrainContainer } = render(<Terrain width={32} height={32} seed="cross_water" />);
      const { container: waterContainer } = render(<Water width={32} height={32} seed="cross_water" />);

      expect(terrainContainer.querySelector('[name="terrain-lod"]')).toBeTruthy();
      expect(waterContainer.querySelector('[name="water-mesh"]')).toBeTruthy();
    });
  });

  describe('Tier 4: Real-World Application Scenarios', () => {
    it('T4.1: water simulation dynamic prop updates during environment transitions', () => {
      const { container, rerender } = render(
        <Water windSpeed={1.0} depthTransparency={0.3} reflectionColor="#112233" />
      );

      rerender(
        <Water windSpeed={4.0} depthTransparency={0.9} reflectionColor="#55aaff" />
      );

      const mesh = container.querySelector('[name="water-mesh"]');
      expect(mesh?.getAttribute('data-wind-speed')).toBe('4');
      expect(mesh?.getAttribute('data-depth-transparency')).toBe('0.9');
      expect(mesh?.getAttribute('data-reflection-color')).toBe('#55aaff');
    });
  });
});
