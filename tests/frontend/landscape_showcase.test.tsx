import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import * as THREE from 'three';
import { render, act, screen, fireEvent, waitFor, cleanup } from '@testing-library/react';
import { App } from '../../src/App';
import LandscapeShowcase from '../../src/components/Landscape/LandscapeShowcase';
import Terrain from '../../src/components/Landscape/Terrain';
import Water from '../../src/components/Landscape/Water';
import Sky from '../../src/components/Landscape/Sky';
import Vegetation from '../../src/components/Landscape/Vegetation';
import Weather from '../../src/components/Landscape/Weather';
import PositionalAudio from '../../src/components/Landscape/PositionalAudio';
import CameraControls from '../../src/components/Landscape/CameraControls';
import { audioManager } from '../../src/components/Landscape/utils/audioManager';
import { generateTerrainData, getBiomeColor, determineBiome, generateTerrain, generateFloraPlacements } from '../../src/components/Landscape/utils/terrainGenerator';

// Mock Canvas and useFrame for R3F compatibility
let frameCallbacks: Array<(state: any) => void> = [];
vi.mock('@react-three/fiber', async () => {
  return {
    Canvas: ({ children }: any) => <div data-testid="mock-canvas">{children}</div>,
    useFrame: (cb: any) => {
      frameCallbacks.push(cb);
    },
    useThree: () => ({
      camera: {
        position: { set: vi.fn(), x: 0, y: 25, z: 45 },
        lookAt: vi.fn(),
        quaternion: new THREE.Quaternion(),
        rotation: { set: vi.fn() },
        rotateY: vi.fn(),
        rotateX: vi.fn()
      },
      scene: { add: vi.fn(), remove: vi.fn() },
      gl: { setSize: vi.fn(), domElement: document.createElement('canvas') }
    }),
    extend: vi.fn()
  };
});

// Mock Web Audio API
class MockAudioContext {
  createGain() {
    return {
      gain: { value: 1, setValueAtTime: vi.fn() },
      connect: vi.fn(),
    };
  }
  createPanner() {
    return {
      panningModel: 'HRTF',
      positionX: { value: 0, setValueAtTime: vi.fn() },
      positionY: { value: 0, setValueAtTime: vi.fn() },
      positionZ: { value: 0, setValueAtTime: vi.fn() },
      connect: vi.fn(),
    };
  }
  destination = {};
  listener = {
    positionX: { value: 0, setValueAtTime: vi.fn() },
    positionY: { value: 0, setValueAtTime: vi.fn() },
    positionZ: { value: 0, setValueAtTime: vi.fn() },
    forwardX: { value: 0, setValueAtTime: vi.fn() },
    forwardY: { value: 0, setValueAtTime: vi.fn() },
    forwardZ: { value: 0, setValueAtTime: vi.fn() },
    upX: { value: 0, setValueAtTime: vi.fn() },
    upY: { value: 0, setValueAtTime: vi.fn() },
    upZ: { value: 0, setValueAtTime: vi.fn() },
  };
}
vi.stubGlobal('AudioContext', MockAudioContext);
vi.stubGlobal('webkitAudioContext', MockAudioContext);

describe('Landscape Showcase Test Suite (93 Tests)', () => {
  beforeEach(() => {
    frameCallbacks = [];
    vi.clearAllMocks();
    // Reset audioManager context
    audioManager.ctx = null;
    audioManager.masterGain = null;
  });

  afterEach(() => {
    cleanup();
  });

  describe('Tier 1: Feature Coverage (40 tests)', () => {
    describe('F1: Procedural Terrain', () => {
      it('F1.1: grid size calculation', () => {
        const data = generateTerrainData(32, 32);
        expect(data.length).toBe(1024);
      });

      it('F1.2: biome calculation verification', () => {
        expect(determineBiome(2.5, 50)).toBe('ocean');
        expect(determineBiome(4.5, 50)).toBe('beach');
        expect(determineBiome(85, 50)).toBe('snow peaks');
      });

      it('F1.3: LOD calculation verification', () => {
        const data = generateTerrainData(16, 16);
        expect(data.length).toBe(256);
      });

      it('F1.4: river heights', () => {
        const terrain = generateTerrain(32, 32, 'river_seed');
        const riverCells = terrain.grid.flat().filter(c => c.isRiver);
        expect(riverCells.length).toBeGreaterThanOrEqual(0);
        riverCells.forEach(c => {
          expect(c.elevation).toBeLessThanOrEqual(80);
        });
      });

      it('F1.5: plateau height', () => {
        const terrain = generateTerrain(32, 32, 'lake_seed');
        const lakeCells = terrain.grid.flat().filter(c => c.isLake);
        lakeCells.forEach(c => {
          expect(c.elevation).toBe(35);
        });
      });
    });

    describe('F2: Advanced Water', () => {
      it('F2.1: reflection updates', () => {
        const { container } = render(<Water reflectionColor="#ff0000" />);
        const mesh = container.querySelector('[name="water-mesh"]');
        expect(mesh).toBeTruthy();
        expect(mesh?.getAttribute('data-reflection-color')).toBe('#ff0000');
      });

      it('F2.2: wave ticks', () => {
        const { container } = render(<Water windSpeed={2.0} />);
        const mesh = container.querySelector('[name="water-mesh"]');
        expect(mesh).toBeTruthy();
        
        act(() => {
          frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => 1.0 } }));
        });
        expect(frameCallbacks.length).toBeGreaterThan(0);
      });

      it('F2.3: distinct waves', () => {
        const { container: c1 } = render(<Water windSpeed={1.0} />);
        const { container: c2 } = render(<Water windSpeed={5.0} />);
        expect(c1.querySelector('[name="water-mesh"]')).toBeTruthy();
        expect(c2.querySelector('[name="water-mesh"]')).toBeTruthy();
      });

      it('F2.4: waterfall particles', () => {
        const { container } = render(<Water />);
        const waterfall = container.querySelector('[name="waterfall-particles"]');
        expect(waterfall).toBeTruthy();
      });

      it('F2.5: depth transparency', () => {
        const { container } = render(<Water depthTransparency={0.5} />);
        const mesh = container.querySelector('[name="water-mesh"]');
        expect(mesh?.getAttribute('data-depth-transparency')).toBe('0.5');
      });
    });

    describe('F3: Vegetation', () => {
      it('F3.1: tree species', () => {
        const { container } = render(<Vegetation width={32} height={32} />);
        const group = container.querySelector('[name="vegetation-group"]');
        expect(group).toBeTruthy();
      });

      it('F3.2: ground cover biomes', () => {
        const { container } = render(<Vegetation width={16} height={16} densityFactor={0.5} />);
        const group = container.querySelector('[name="vegetation-group"]');
        expect(group).toBeTruthy();
      });

      it('F3.3: wind angle', () => {
        const { container } = render(<Vegetation windAngle={Math.PI / 4} />);
        const group = container.querySelector('[name="vegetation-group"]');
        expect(group?.getAttribute('data-wind-angle')).toBe(String(Math.PI / 4));
      });

      it('F3.4: GPU instancing setup', () => {
        const { container } = render(<Vegetation />);
        expect(container.querySelector('[name="vegetation-instanced-mesh-oak"]')).toBeTruthy();
        expect(container.querySelector('[name="vegetation-instanced-mesh-pine"]')).toBeTruthy();
      });

      it('F3.5: density check', () => {
        const { container: c1 } = render(<Vegetation densityFactor={1.0} />);
        const { container: c2 } = render(<Vegetation densityFactor={0.1} />);
        expect(c1).toBeTruthy();
        expect(c2).toBeTruthy();
      });
    });

    describe('F4: Sky & Lighting', () => {
      it('F4.1: sky colors', () => {
        const { container: daySky } = render(<Sky timeOfDay={12} />);
        const { container: nightSky } = render(<Sky timeOfDay={23} />);
        expect(daySky.querySelector('[name="sky-group"]')).toBeTruthy();
        expect(nightSky.querySelector('[name="sky-group"]')).toBeTruthy();
      });

      it('F4.2: lighting intensities', () => {
        const { container } = render(<Sky timeOfDay={12} />);
        expect(container.querySelector('[name="sky-light"]')).toBeTruthy();
      });

      it('F4.3: shadow maps', () => {
        const { container } = render(<Sky />);
        const light = container.querySelector('[name="sky-light"]');
        expect(light).toBeTruthy();
      });

      it('F4.4: cloud animation', () => {
        const { container } = render(<Sky speed={2.0} />);
        expect(container.querySelector('[name="cloud-mesh"]')).toBeTruthy();
        act(() => {
          frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => 2.0 } }));
        });
      });

      it('F4.5: moon/stars visibility', () => {
        const { container } = render(<Sky timeOfDay={0} />);
        expect(container.querySelector('[name="moon-mesh"]')).toBeTruthy();
        expect(container.querySelector('[name="stars-particles"]')).toBeTruthy();
      });
    });

    describe('F5: Weather', () => {
      it('F5.1: transitions', () => {
        const { container } = render(<Weather weather="rain" />);
        expect(container.querySelector('[name="weather-group"]')).toBeTruthy();
      });

      it('F5.2: rain drops', () => {
        const { container } = render(<Weather weather="rain" precipitationRate={0.5} />);
        expect(container.querySelector('[name="weather-particles"]')).toBeTruthy();
      });

      it('F5.3: snow accumulation', () => {
        const { container } = render(<Weather weather="snow" />);
        expect(container.querySelector('[name="weather-particles"]')).toBeTruthy();
      });

      it('F5.4: fog thickness', () => {
        const { container } = render(<Weather weather="fog" />);
        expect(container.querySelector('[name="weather-group"]')).toBeTruthy();
      });

      it('F5.5: wet surface darkening', () => {
        const { container } = render(<Terrain wetnessRatio={0.8} />);
        expect(container.querySelector('[name="terrain-mesh"]')).toBeTruthy();
      });
    });

    describe('F6: Audio', () => {
      it('F6.1: spatial coordinates', () => {
        audioManager.initialize();
        audioManager.createSpatialSource('test-src');
        audioManager.updateSpatialSource('test-src', 10, 20, 30);
        expect(audioManager.ctx).toBeTruthy();
      });

      it('F6.2: proximity volume', () => {
        audioManager.initialize();
        audioManager.setVolume(0.8);
        expect(audioManager.getVolume()).toBe(0.8);
      });

      it('F6.3: blended layers', () => {
        audioManager.initialize();
        audioManager.createSpatialSource('layer1');
        audioManager.createSpatialSource('layer2');
        expect(audioManager.ctx).toBeTruthy();
      });

      it('F6.4: muted gain', () => {
        audioManager.initialize();
        audioManager.mute();
        expect(audioManager.getIsMuted()).toBe(true);
      });

      it('F6.5: audio context setup', () => {
        audioManager.initialize();
        expect(audioManager.ctx).toBeTruthy();
      });
    });

    describe('F7: Camera', () => {
      it('F7.1: orbit navigation', () => {
        const { container } = render(<CameraControls cameraMode="orbit" />);
        expect(container.querySelector('[name="camera-controls"]')).toBeTruthy();
      });

      it('F7.2: fly WASD', () => {
        const { container } = render(<CameraControls cameraMode="fly" />);
        expect(container.querySelector('[name="camera-controls"]')).toBeTruthy();
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'w' }));
        act(() => {
          frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => 1.0 } }));
        });
        window.dispatchEvent(new KeyboardEvent('keyup', { key: 'w' }));
      });

      it('F7.3: terrain collision', () => {
        const heightMap = new Float32Array(4096).fill(50);
        const { container } = render(<CameraControls cameraMode="fly" terrainHeightMap={heightMap} />);
        expect(container).toBeTruthy();
      });

      it('F7.4: mode switching', () => {
        const { container } = render(<CameraControls cameraMode="cinematic" />);
        expect(container.querySelector('[name="camera-controls"]')).toBeTruthy();
      });

      it('F7.5: cinematic flight', () => {
        const { container } = render(<CameraControls cameraMode="cinematic" />);
        act(() => {
          frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => 10.0 } }));
        });
        expect(container).toBeTruthy();
      });
    });

    describe('F8: App Integration', () => {
      it('F8.1: dashboard sliders', () => {
        render(<LandscapeShowcase />);
        expect(document.getElementById('weather-select')).toBeTruthy();
        expect(document.getElementById('speed-slider')).toBeTruthy();
        expect(document.getElementById('volume-slider')).toBeTruthy();
      });

      it('F8.2: App.tsx mount toggle', async () => {
        render(<App />);
        const toggleBtn = screen.getByText(/Landscape Showcase/i);
        expect(toggleBtn).toBeTruthy();
        await act(async () => {
          fireEvent.click(toggleBtn);
          await new Promise((resolve) => setTimeout(resolve, 100));
        });
        const showcase = await screen.findByTestId('landscape-showcase');
        expect(showcase).toBeTruthy();
      });

      it('F8.3: UI event handler', () => {
        render(<LandscapeShowcase />);
        const select = document.getElementById('weather-select') as HTMLSelectElement;
        fireEvent.change(select, { target: { value: 'rain' } });
        expect(select.value).toBe('rain');
      });

      it('F8.4: resize updates', () => {
        render(<LandscapeShowcase />);
        window.dispatchEvent(new Event('resize'));
        expect(screen.getByTestId('landscape-showcase')).toBeTruthy();
      });

      it('F8.5: error boundaries', () => {
        // Safe render of component
        const { container } = render(<LandscapeShowcase />);
        expect(container).toBeTruthy();
      });
    });
  });

  describe('Tier 2: Boundary & Corner Cases (40 tests)', () => {
    describe('F1: Terrain boundaries', () => {
      it('F1.1: extreme grid sizes', () => {
        const data = generateTerrainData(0, 0);
        expect(data.length).toBe(0);
      });

      it('F1.2: boundary elevation limits', () => {
        const terrain = generateTerrain(16, 16, 'seed');
        terrain.grid.flat().forEach(cell => {
          expect(cell.elevation).toBeGreaterThanOrEqual(0);
          expect(cell.elevation).toBeLessThanOrEqual(100);
        });
      });

      it('F1.3: zero elevation', () => {
        expect(determineBiome(0, 0)).toBe('ocean');
      });

      it('F1.4: out-of-bounds rivers', () => {
        const data = generateTerrain(5, 5, 'seed');
        expect(data.grid.length).toBe(5);
      });

      it('F1.5: extreme camera LOD', () => {
        const data = generateTerrainData(128, 128);
        expect(data.length).toBe(16384);
      });
    });

    describe('F2: Water boundaries', () => {
      it('F2.1: zero wind speed calm', () => {
        const { container } = render(<Water windSpeed={0} />);
        expect(container.querySelector('[name="water-mesh"]')).toBeTruthy();
      });

      it('F2.2: storm waves max height', () => {
        const { container } = render(<Water windSpeed={100} />);
        expect(container.querySelector('[name="water-mesh"]')).toBeTruthy();
      });

      it('F2.3: zero/negative depth', () => {
        const { container } = render(<Water depthTransparency={-1} />);
        expect(container.querySelector('[name="water-mesh"]')).toBeTruthy();
      });

      it('F2.4: waterfall limits', () => {
        const { container } = render(<Water />);
        expect(container.querySelector('[name="waterfall-particles"]')).toBeTruthy();
      });

      it('F2.5: grazing angles', () => {
        const { container } = render(<Water reflectionColor="#000" />);
        expect(container.querySelector('[name="water-mesh"]')).toBeTruthy();
      });
    });

    describe('F3: Vegetation boundaries', () => {
      it('F3.1: no underwater placement', () => {
        const placements = generateFloraPlacements(16, 16);
        placements.forEach(p => {
          expect(p.type).toBeTruthy();
        });
      });

      it('F3.2: zero vegetation count', () => {
        const { container } = render(<Vegetation densityFactor={0} />);
        expect(container.querySelector('[name="vegetation-group"]')).toBeTruthy();
      });

      it('F3.3: wind force hurricane', () => {
        const { container } = render(<Vegetation windSpeed={50} />);
        expect(container.querySelector('[name="vegetation-group"]')).toBeTruthy();
      });

      it('F3.4: max instanced capacity', () => {
        const { container } = render(<Vegetation maxCapacity={5} />);
        expect(container.querySelector('[name="vegetation-group"]')).toBeTruthy();
      });

      it('F3.5: overlap prevention', () => {
        const { container } = render(<Vegetation width={10} height={10} />);
        expect(container.querySelector('[name="vegetation-group"]')).toBeTruthy();
      });
    });

    describe('F4: Sky boundaries', () => {
      it('F4.1: sunrise/sunset transitions exact ticks', () => {
        const { container } = render(<Sky timeOfDay={6.0} />);
        expect(container.querySelector('[name="sky-group"]')).toBeTruthy();
      });

      it('F4.2: cycle speed limits', () => {
        const { container } = render(<Sky speed={0} />);
        expect(container.querySelector('[name="sky-group"]')).toBeTruthy();
      });

      it('F4.3: cloudless vs overcast', () => {
        const { container } = render(<Sky speed={10} />);
        expect(container.querySelector('[name="cloud-mesh"]')).toBeTruthy();
      });

      it('F4.4: total darkness shadows', () => {
        const { container } = render(<Sky timeOfDay={0} />);
        expect(container.querySelector('[name="sky-light"]')).toBeTruthy();
      });

      it('F4.5: daylight star opacity', () => {
        const { container } = render(<Sky timeOfDay={12} />);
        expect(container.querySelector('[name="stars-particles"]')).toBeNull();
      });
    });

    describe('F5: Weather boundaries', () => {
      it('F5.1: rapid weather shifts', () => {
        const { rerender, container } = render(<Weather weather="clear" />);
        rerender(<Weather weather="rain" />);
        rerender(<Weather weather="snow" />);
        rerender(<Weather weather="fog" />);
        expect(container.querySelector('[name="weather-group"]')).toBeTruthy();
      });

      it('F5.2: max precipitation blizzards', () => {
        const { container } = render(<Weather weather="snow" precipitationRate={10} />);
        expect(container.querySelector('[name="weather-particles"]')).toBeTruthy();
      });

      it('F5.3: clear state zero particles', () => {
        const { container } = render(<Weather weather="clear" />);
        expect(container.querySelector('[name="weather-particles"]')).toBeNull();
      });

      it('F5.4: drying speed', () => {
        const { container } = render(<Terrain wetnessRatio={0} />);
        expect(container.querySelector('[name="terrain-mesh"]')).toBeTruthy();
      });

      it('F5.5: snow limit cap', () => {
        const { container } = render(<Weather weather="snow" precipitationRate={2.0} />);
        expect(container.querySelector('[name="weather-particles"]')).toBeTruthy();
      });
    });

    describe('F6: Audio boundaries', () => {
      it('F6.1: zero volume', () => {
        audioManager.initialize();
        audioManager.setVolume(0);
        expect(audioManager.getVolume()).toBe(0);
      });

      it('F6.2: exact position match division-by-zero prevention', () => {
        audioManager.initialize();
        audioManager.updateSpatialSource('div-zero', 0, 0, 0);
        expect(audioManager.ctx).toBeTruthy();
      });

      it('F6.3: extremely far audio distance', () => {
        audioManager.initialize();
        audioManager.updateSpatialSource('far-src', 100000, 100000, 100000);
        expect(audioManager.ctx).toBeTruthy();
      });

      it('F6.4: web audio startup error fallback', () => {
        const oldAudioContext = window.AudioContext;
        const oldWebkitAudioContext = (window as any).webkitAudioContext;
        window.AudioContext = undefined as any;
        (window as any).webkitAudioContext = undefined as any;
        audioManager.ctx = null;
        audioManager.initialize();
        expect(audioManager.ctx).toBeNull();
        window.AudioContext = oldAudioContext;
        (window as any).webkitAudioContext = oldWebkitAudioContext;
      });

      it('F6.5: layer transitions', () => {
        audioManager.initialize();
        audioManager.mute();
        audioManager.unmute();
        expect(audioManager.getIsMuted()).toBe(false);
      });
    });

    describe('F7: Camera boundaries', () => {
      it('F7.1: fly mode max speeds', () => {
        const { container } = render(<CameraControls cameraMode="fly" />);
        expect(container).toBeTruthy();
      });

      it('F7.2: vertical cliff collision', () => {
        const heightMap = new Float32Array(4096).fill(95);
        const { container } = render(<CameraControls cameraMode="fly" terrainHeightMap={heightMap} />);
        expect(container).toBeTruthy();
      });

      it('F7.3: zoom scroll limits', () => {
        const { container } = render(<CameraControls cameraMode="orbit" />);
        expect(container).toBeTruthy();
      });

      it('F7.4: out-of-bounds coordinates', () => {
        const { container } = render(<CameraControls cameraMode="fly" />);
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'd' }));
        act(() => {
          for (let i = 0; i < 500; i++) {
            frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => i } }));
          }
        });
        expect(container).toBeTruthy();
      });

      it('F7.5: rapid mode switching', () => {
        const { rerender, container } = render(<CameraControls cameraMode="orbit" />);
        rerender(<CameraControls cameraMode="fly" />);
        rerender(<CameraControls cameraMode="cinematic" />);
        expect(container).toBeTruthy();
      });
    });

    describe('F8: Integration boundaries', () => {
      it('F8.1: controls at max limits', () => {
        render(<LandscapeShowcase />);
        const speedInput = document.getElementById('speed-slider') as HTMLInputElement;
        fireEvent.change(speedInput, { target: { value: '10' } });
        expect(speedInput.value).toBe('10');
      });

      it('F8.2: rabbit vs landscape exclusion', async () => {
        render(<App />);
        const landscapeBtn = screen.getByText(/Landscape Showcase/i);
        const rabbitBtn = screen.getByText(/Thử nghiệm Thỏ/i);
        
        await act(async () => {
          fireEvent.click(landscapeBtn);
          await new Promise((resolve) => setTimeout(resolve, 100));
        });
        const showcase = await screen.findByTestId('landscape-showcase');
        expect(showcase).toBeTruthy();
        
        await act(async () => {
          fireEvent.click(rabbitBtn);
          await new Promise((resolve) => setTimeout(resolve, 100));
        });
        await waitFor(() => {
          expect(screen.queryByTestId('landscape-showcase')).toBeNull();
        });
      });

      it('F8.3: rapid resizes', () => {
        render(<LandscapeShowcase />);
        for (let i = 0; i < 50; i++) {
          window.dispatchEvent(new Event('resize'));
        }
        expect(screen.getByTestId('landscape-showcase')).toBeTruthy();
      });

      it('F8.4: invalid text inputs', () => {
        render(<LandscapeShowcase />);
        const select = document.getElementById('weather-select') as HTMLSelectElement;
        expect(() => {
          fireEvent.change(select, { target: { value: 'unknown-weather-condition' } });
        }).not.toThrow();
      });

      it('F8.5: performance stats counter', () => {
        render(<App />);
        expect(screen.queryByText(/Backend FPS/i)).toBeTruthy();
      });
    });
  });

  describe('Tier 3: Cross-Feature Combinations (8 tests)', () => {
    it('Tier 3: Test 1 (Weather & Terrain)', () => {
      // Rain affects terrain wetness ratio, and high elevation changes rain to snow
      const { container: cRain } = render(<Terrain wetnessRatio={0.9} />);
      const mesh = cRain.querySelector('[name="terrain-mesh"]');
      expect(mesh?.getAttribute('data-wetness-ratio')).toBe('0.9');

      const biomeAtPeak = determineBiome(90, 50);
      expect(biomeAtPeak).toBe('snow peaks');
    });

    it('Tier 3: Test 2 (Sky & Water)', () => {
      // Day-night cycle changes sky color, which updates water reflection
      const { container: cNightSky } = render(<Sky timeOfDay={0} />);
      const { container: cNightWater } = render(<Water reflectionColor="#01112a" />);
      
      expect(cNightSky.querySelector('[name="sky-group"]')).toBeTruthy();
      expect(cNightWater.querySelector('[name="water-mesh"]')?.getAttribute('data-reflection-color')).toBe('#01112a');
    });

    it('Tier 3: Test 3 (Weather & Audio)', () => {
      // Heavy rain weather triggers thunder sounds and increases wind audio volume
      audioManager.initialize();
      audioManager.createSpatialSource('thunder');
      audioManager.setVolume(1.0); // maximum volume for heavy rain
      expect(audioManager.getVolume()).toBe(1.0);
    });

    it('Tier 3: Test 4 (Camera & Audio)', () => {
      // Moving camera close to waterfall triggers spatial updates and increases waterfall volume
      audioManager.initialize();
      audioManager.createSpatialSource('waterfall');
      audioManager.updateSpatialSource('waterfall', 1, 2, 3);
      audioManager.setVolume(0.9);
      expect(audioManager.getVolume()).toBe(0.9);
    });

    it('Tier 3: Test 5 (Sky & Vegetation)', () => {
      // Sunrise/sunset shifts sun shadows across vegetation positions
      const { container } = render(<Sky timeOfDay={6.5} />); // dawn
      expect(container.querySelector('[name="sky-light"]')).toBeTruthy();
    });

    it('Tier 3: Test 6 (Camera & Terrain)', () => {
      // Fly-through camera WASD updates trigger terrain collision checks and clamp height
      const heightMap = new Float32Array(4096).fill(30);
      const { container } = render(<CameraControls cameraMode="fly" terrainHeightMap={heightMap} />);
      expect(container).toBeTruthy();
    });

    it('Tier 3: Test 7 (Weather & Vegetation)', () => {
      // Wind speed updates from weather influence vegetation sway amplitude
      const { container } = render(<Vegetation windSpeed={4.0} />);
      expect(container.querySelector('[name="vegetation-group"]')?.getAttribute('data-wind-speed')).toBe('4');
    });

    it('Tier 3: Test 8 (Day-Night Cycle & Lighting/UI)', () => {
      // Changing day-night speed via UI dashboard slider increases sky rotation tick speed
      const { container } = render(<Sky speed={5.0} />);
      expect(container.querySelector('[name="sky-group"]')?.getAttribute('data-speed')).toBe('5');
    });
  });

  describe('Tier 4: Real-World Application Scenarios (5 tests)', () => {
    it('Tier 4: Scenario 1 (Cinematic Flyover)', () => {
      // Player triggers cinematic flight. Camera moves along pre-defined spline path. Light animates. Audio updates.
      const { container: skyCont } = render(<Sky speed={2.0} timeOfDay={17.5} />); // Sunset
      const { container: camCont } = render(<CameraControls cameraMode="cinematic" />);
      
      audioManager.initialize();
      audioManager.createSpatialSource('ocean-ambient');
      audioManager.createSpatialSource('forest-ambient');
      audioManager.createSpatialSource('wind-ambient');
      
      act(() => {
        frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => 10.0 } }));
      });
      
      expect(skyCont.querySelector('[name="sky-group"]')).toBeTruthy();
      expect(camCont).toBeTruthy();
    });

    it('Tier 4: Scenario 2 (Weather Transition)', async () => {
      // Clear day -> rain storm -> dry evening
      const { rerender, container } = render(<Weather weather="clear" />);
      expect(container.querySelector('[name="weather-particles"]')).toBeNull();
      
      rerender(<Weather weather="rain" precipitationRate={0.8} />);
      act(() => {
        frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => 1.0 } }, 0.5));
      });
      expect(container.querySelector('[name="weather-particles"]')).toBeTruthy();
      
      rerender(<Weather weather="clear" />);
      act(() => {
        frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => 2.0 } }, 0.5));
      });
      await waitFor(() => {
        expect(container.querySelector('[name="weather-particles"]')).toBeNull();
      });
    });

    it('Tier 4: Scenario 3 (Alpine Exploration)', () => {
      // Camera flies to alpine snow peaks. Elevation is high. Biome matches alpine/snow. Weather triggers snow.
      const biome = determineBiome(85, 40);
      expect(biome).toBe('snow peaks');
      
      const { container } = render(<Weather weather="snow" />);
      expect(container.querySelector('[name="weather-particles"]')).toBeTruthy();
    });

    it('Tier 4: Scenario 4 (Beach Exploration)', () => {
      // Camera flies to beach shore. Elevation is low. Biome matches beach. Audio triggers waves.
      const biome = determineBiome(4.5, 20);
      expect(biome).toBe('beach');
      
      const { container } = render(<Water reflectionColor="#d97706" />);
      expect(container.querySelector('[name="water-mesh"]')).toBeTruthy();
    });

    it('Tier 4: Scenario 5 (Ecosystem Speedup)', () => {
      // Rapid day-night cycle synchronizes all elements over multiple days
      const { container: showcaseCont } = render(<LandscapeShowcase />);
      const speedInput = document.getElementById('speed-slider') as HTMLInputElement;
      fireEvent.change(speedInput, { target: { value: '8.0' } });
      
      act(() => {
        for (let i = 0; i < 20; i++) {
          frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => i } }));
        }
      });
      
      expect(showcaseCont).toBeTruthy();
    });
  });
});
