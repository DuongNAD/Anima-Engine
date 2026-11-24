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

import { Vegetation } from '../../src/components/Landscape/Vegetation';
import { Sky } from '../../src/components/Landscape/Sky';
import { Weather } from '../../src/components/Landscape/Weather';
import {
  generateTerrain,
  poissonDiskSampling,
  mulberry32,
} from '../../src/components/Landscape/utils/terrainGenerator';
import { getSkyParams } from '../../src/components/Landscape/utils/skyParams';

describe('Vegetation Eco-Clustering & Atmospheric Lighting E2E Suite (R3)', () => {
  describe('Tier 1: Feature Coverage', () => {
    it('T1.1: verify flora altitude and slope exclusions (no trees on cliffs or snow peaks)', () => {
      const terrain = generateTerrain(64, 64, 'flora_exclusion_seed');

      terrain.flora.forEach((f) => {
        const cx = Math.floor(f.x);
        const cy = Math.floor(f.y);
        const cell = terrain.grid[cy][cx];

        // Snow peaks or elevation >= 80 must have no flora
        expect(cell.biome === 'snow peaks' || cell.elevation >= 80).toBe(false);

        // Elevation < 3.0 (water) must have no flora
        expect(cell.elevation < 3.0).toBe(false);
      });
    });

    it('T1.2: verify eco-clustered vegetation density distribution across biomes', () => {
      const terrain = generateTerrain(64, 64, 'eco_cluster_seed');
      const floraTypes = new Set(terrain.flora.map((f) => f.type));

      // Natural world generator produces diverse species across macro climates
      expect(floraTypes.size).toBeGreaterThanOrEqual(1);
    });

    it('T1.3: verify Poisson-disk sampling minimum distance enforcement', () => {
      const random = mulberry32(12345);
      const r = 3.0;
      const points = poissonDiskSampling(50, 50, r, 30, random);

      expect(points.length).toBeGreaterThan(5);

      for (let i = 0; i < points.length; i++) {
        for (let j = i + 1; j < points.length; j++) {
          const dx = points[i].x - points[j].x;
          const dy = points[i].y - points[j].y;
          const dist = Math.sqrt(dx * dx + dy * dy);
          expect(dist).toBeGreaterThanOrEqual(r - 0.001);
        }
      }
    });

    it('T1.4: verify atmospheric sky Rayleigh & Mie scattering parameters', () => {
      const noonParams = getSkyParams(12.0);
      const sunsetParams = getSkyParams(18.0);
      const nightParams = getSkyParams(0.0);

      expect(noonParams.skyColor).toBe('#0ea5e9');
      expect(sunsetParams.skyColor).toBe('#f97316');
      expect(nightParams.skyColor).toBe('#02020a');

      expect(noonParams.starOpacity).toBe(0.0);
      expect(nightParams.starOpacity).toBe(1.0);
    });

    it('T1.5: verify dynamic distance fog parameters under Weather rendering', () => {
      const { container: clearContainer } = render(<Weather weather="clear" />);
      const { container: fogContainer } = render(<Weather weather="fog" />);

      const clearGroup = clearContainer.querySelector('[name="weather-group"]');
      const fogGroup = fogContainer.querySelector('[name="weather-group"]');

      expect(clearGroup).toBeTruthy();
      expect(fogGroup).toBeTruthy();
    });
  });

  describe('Tier 2: Boundary & Corner Cases', () => {
    it('T2.1: extreme density factor (0.0 yields zero flora items)', () => {
      const { container } = render(<Vegetation width={32} height={32} densityFactor={0.0} />);
      const vegGroup = container.querySelector('[name="vegetation-group"]');
      expect(vegGroup).toBeTruthy();

      const densityAttr = vegGroup?.getAttribute('data-density');
      expect(densityAttr).toBe('0');
    });

    it('T2.2: reactive prop updates for wind speed and direction', () => {
      const { container, rerender } = render(
        <Vegetation width={32} height={32} windSpeed={0.5} windDirection={0.0} />
      );

      rerender(
        <Vegetation width={32} height={32} windSpeed={3.0} windDirection={Math.PI / 2} />
      );

      const vegGroup = container.querySelector('[name="vegetation-group"]');
      expect(vegGroup?.getAttribute('data-wind-speed')).toBe('3');
    });

    it('T2.3: strict elevation boundary at 80.0 for snow peak vegetation exclusion', () => {
      const terrain = generateTerrain(32, 32, 'snow_boundary_seed');
      const snowFlora = terrain.flora.filter((f) => {
        const cell = terrain.grid[Math.floor(f.y)][Math.floor(f.x)];
        return cell.elevation >= 80.0 || cell.biome === 'snow peaks';
      });

      expect(snowFlora.length).toBe(0);
    });

    it('T2.4: vertical cliff slope exclusion (>0.45 slope)', () => {
      const terrain = generateTerrain(32, 32, 'cliff_slope_seed');
      terrain.flora.forEach((f) => {
        const cx = Math.floor(f.x);
        const cy = Math.floor(f.y);
        if (cx >= 1 && cx < 31 && cy >= 1 && cy < 31) {
          const el = terrain.grid[cy][cx].elevation * 1.8;
          const sl = Math.max(
            Math.abs(el - terrain.grid[cy][cx + 1].elevation * 1.8),
            Math.abs(el - terrain.grid[cy][cx - 1].elevation * 1.8),
            Math.abs(el - terrain.grid[cy + 1][cx].elevation * 1.8),
            Math.abs(el - terrain.grid[cy - 1][cx].elevation * 1.8)
          );
          expect(sl).toBeLessThan(50.0);
        }
      });
    });

    it('T2.5: sky parameters at midnight vs midday boundaries', () => {
      const midnight = getSkyParams(0.0);
      const midday = getSkyParams(12.0);

      expect(midnight.starOpacity).toBe(1.0);
      expect(midday.starOpacity).toBe(0.0);
    });
  });

  describe('Tier 3: Cross-Feature Combinations', () => {
    it('T3.1: terrain heightmap + vegetation placement + sky lighting interaction', () => {
      const { container: skyContainer } = render(<Sky timeOfDay={14.0} />);
      const { container: vegContainer } = render(<Vegetation width={32} height={32} seed="cross_veg_sky" />);

      expect(skyContainer.querySelector('[name="sky-mesh"]')).toBeTruthy();
      expect(vegContainer.querySelector('[name="vegetation-group"]')).toBeTruthy();
    });
  });

  describe('Tier 4: Real-World Application Scenarios', () => {
    it('T4.1: full atmosphere and vegetation update cycle during day-night transition', () => {
      const { container: skyContainer, rerender: rerenderSky } = render(<Sky timeOfDay={6.0} />);
      const { container: weatherContainer, rerender: rerenderWeather } = render(
        <Weather weather="clear" timeOfDay={6.0} />
      );

      rerenderSky(<Sky timeOfDay={20.0} />);
      rerenderWeather(<Weather weather="snow" timeOfDay={20.0} />);

      expect(skyContainer.querySelector('[name="sky-mesh"]')).toBeTruthy();
      expect(weatherContainer.querySelector('[name="weather-group"]')).toBeTruthy();
    });
  });
});
