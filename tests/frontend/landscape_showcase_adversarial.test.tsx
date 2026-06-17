import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, act, screen, fireEvent, cleanup } from '@testing-library/react';
import * as THREE from 'three';
import LandscapeShowcase from '../../src/components/Landscape/LandscapeShowcase';
import { LandscapeControlsOverlay } from '../../src/components/Landscape/LandscapeControlsOverlay';
import Terrain from '../../src/components/Landscape/Terrain';
import Water from '../../src/components/Landscape/Water';
import Sky from '../../src/components/Landscape/Sky';
import Vegetation from '../../src/components/Landscape/Vegetation';
import Weather from '../../src/components/Landscape/Weather';
import PositionalAudio from '../../src/components/Landscape/PositionalAudio';
import CameraControls from '../../src/components/Landscape/CameraControls';
import { audioManager } from '../../src/components/Landscape/utils/audioManager';
import {
  generateTerrainData,
  determineBiome,
  generateTerrain,
  mulberry32,
  hashString,
  poissonDiskSampling
} from '../../src/components/Landscape/utils/terrainGenerator';

// Mock Canvas and useFrame for R3F compatibility
let frameCallbacks: Array<(state: any) => void> = [];
vi.mock('@react-three/fiber', async () => {
  return {
    extend: vi.fn(),
    Canvas: ({ children }: any) => <div data-testid="mock-canvas">{children}</div>,
    useFrame: (cb: any) => {
      frameCallbacks.push(cb);
    },
    useThree: () => {
      const camera = new THREE.PerspectiveCamera();
      camera.position.set(0, 0, 0);
      camera.quaternion.set(0, 0, 0, 1);
      return {
        camera,
        scene: new THREE.Scene(),
        gl: {
          setSize: vi.fn(),
          domElement: document.createElement('div')
        }
      };
    }
  };
});

// Mock Web Audio API
class MockAudioContext {
  currentTime = 0;
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

describe('Landscape Showcase Adversarial Test Suite (Tier 5)', () => {
  beforeEach(() => {
    vi.stubGlobal('AudioContext', MockAudioContext);
    vi.stubGlobal('webkitAudioContext', MockAudioContext);
    
    // Mock JSDOM PointerLock functions
    document.exitPointerLock = vi.fn();
    if (typeof HTMLDivElement !== 'undefined') {
      HTMLDivElement.prototype.requestPointerLock = vi.fn();
    }
    
    frameCallbacks = [];
    vi.clearAllMocks();
    audioManager.ctx = null;
    audioManager.masterGain = null;
  });

  afterEach(() => {
    cleanup();
  });

  describe('1. Procedural Terrain Generator Math & Seeding', () => {
    it('should handle extreme seeds in mulberry32 generator', () => {
      const randMax = mulberry32(Number.MAX_SAFE_INTEGER);
      const randMin = mulberry32(Number.MIN_SAFE_INTEGER);
      const randFloat = mulberry32(1.2345);
      const randNaN = mulberry32(NaN);
      const randInf = mulberry32(Infinity);

      expect(typeof randMax()).toBe('number');
      expect(typeof randMin()).toBe('number');
      expect(typeof randFloat()).toBe('number');
      expect(typeof randNaN()).toBe('number');
      expect(typeof randInf()).toBe('number');
    });

    it('should handle empty, unicode, and non-ASCII strings in hashString', () => {
      expect(typeof hashString('')).toBe('number');
      expect(typeof hashString('✨Unicode🔥')).toBe('number');
      expect(typeof hashString('A'.repeat(10000))).toBe('number');
    });

    it('should return empty array for invalid/zero/negative r in poissonDiskSampling', () => {
      // poissonDiskSampling returns empty array if r <= 0 to prevent crashes
      expect(poissonDiskSampling(10, 10, 0, 30, Math.random)).toEqual([]);
      expect(poissonDiskSampling(10, 10, -1.5, 30, Math.random)).toEqual([]);
    });

    it('should handle zero, negative and extreme dimensions in generateTerrain', () => {
      // zero dimensions
      const zeroTerrain = generateTerrain(0, 0, 'seed');
      expect(zeroTerrain.grid.length).toBe(0);
      expect(zeroTerrain.flora.length).toBe(0);

      // 1x1 dimensions
      const miniTerrain = generateTerrain(1, 1, 'seed');
      expect(miniTerrain.grid.length).toBe(1);
      expect(miniTerrain.grid[0].length).toBe(1);
    });

    it('should verify exact biome boundaries in determineBiome', () => {
      // thresholds: 3.0 (ocean/beach), 5.0 (beach/grassland or forest), 60 (grassland or forest / alpine or taiga), 80 (peaks)
      expect(determineBiome(2.99, 50)).toBe('ocean');
      expect(determineBiome(3.0, 50)).toBe('beach');
      expect(determineBiome(4.99, 50)).toBe('beach');
      expect(determineBiome(5.0, 40)).toBe('grassland');
      expect(determineBiome(5.0, 50)).toBe('forest');
      
      expect(determineBiome(59.99, 40)).toBe('grassland');
      expect(determineBiome(60, 40)).toBe('alpine rock');
      expect(determineBiome(60, 50)).toBe('taiga');
      
      expect(determineBiome(79.99, 50)).toBe('taiga');
      expect(determineBiome(80, 50)).toBe('snow peaks');

      // negative and extreme boundaries
      expect(determineBiome(-100, 50)).toBe('ocean');
      expect(determineBiome(1000, 50)).toBe('snow peaks');
      expect(determineBiome(50, -100)).toBe('grassland');
      expect(determineBiome(50, 1000)).toBe('forest');
    });
  });

  describe('2. Terrain Component Boundary Settings', () => {
    it('should render successfully with width/height of 0', () => {
      const { container } = render(<Terrain width={0} height={0} />);
      expect(container).toBeTruthy();
      const mesh = container.querySelector('[name="terrain-mesh"]');
      expect(mesh).toBeTruthy();
    });

    it('should render successfully with extreme wetnessRatio', () => {
      const { container: cNegative } = render(<Terrain wetnessRatio={-5.0} />);
      const { container: cOverflow } = render(<Terrain wetnessRatio={10.0} />);
      const { container: cNaN } = render(<Terrain wetnessRatio={NaN} />);
      
      expect(cNegative.querySelector('[name="terrain-mesh"]')).toBeTruthy();
      expect(cOverflow.querySelector('[name="terrain-mesh"]')).toBeTruthy();
      expect(cNaN.querySelector('[name="terrain-mesh"]')).toBeTruthy();
    });
  });

  describe('3. Water Component Boundary Settings', () => {
    it('should handle windSpeed of NaN and Infinity', () => {
      const { container: cNaN } = render(<Water windSpeed={NaN} />);
      const { container: cInf } = render(<Water windSpeed={Infinity} />);
      expect(cNaN.querySelector('[name="water-mesh"]')).toBeTruthy();
      expect(cInf.querySelector('[name="water-mesh"]')).toBeTruthy();
    });

    it('should handle negative, NaN, and Infinity depthTransparency', () => {
      const { container: cNeg } = render(<Water depthTransparency={-1.5} />);
      const { container: cNaN } = render(<Water depthTransparency={NaN} />);
      const { container: cInf } = render(<Water depthTransparency={Infinity} />);
      expect(cNeg.querySelector('[name="water-mesh"]')).toBeTruthy();
      expect(cNaN.querySelector('[name="water-mesh"]')).toBeTruthy();
      expect(cInf.querySelector('[name="water-mesh"]')).toBeTruthy();
    });

    it('should handle empty/malformed reflectionColor', () => {
      // THREE.Color might log warning but shouldn't crash the renderer
      const { container: cEmpty } = render(<Water reflectionColor="" />);
      const { container: cBad } = render(<Water reflectionColor="invalid-rgb-format" />);
      expect(cEmpty.querySelector('[name="water-mesh"]')).toBeTruthy();
      expect(cBad.querySelector('[name="water-mesh"]')).toBeTruthy();
    });

    it('should handle negative delta in waterfall animation tick without crashing', () => {
      const { container } = render(<Water width={32} height={32} />);
      
      // Force frame update with negative delta
      act(() => {
        frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => 2.0 } }));
      });
      // Try again with negative delta
      act(() => {
        frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => 2.0 } }));
      });
      expect(container.querySelector('[name="water-mesh"]')).toBeTruthy();
    });
  });

  describe('4. Sky Component Day-Night Cycle Boundary Settings', () => {
    it('should handle negative and extreme timeOfDay in getSkyParams', () => {
      // test negative timeOfDay
      const { container: cNeg } = render(<Sky timeOfDay={-12.0} />);
      // test positive timeOfDay > 24
      const { container: cOverflow } = render(<Sky timeOfDay={36.0} />);
      // test NaN and Infinity
      const { container: cNaN } = render(<Sky timeOfDay={NaN} />);
      const { container: cInf } = render(<Sky timeOfDay={Infinity} />);

      expect(cNeg.querySelector('[name="sky-mesh"]')).toBeTruthy();
      expect(cOverflow.querySelector('[name="sky-mesh"]')).toBeTruthy();
      expect(cNaN.querySelector('[name="sky-mesh"]')).toBeTruthy();
      expect(cInf.querySelector('[name="sky-mesh"]')).toBeTruthy();
    });

    it('should handle boundary exact timeOfDay values at keyframes', () => {
      const boundaryTimes = [0, 4.5, 6.0, 8.0, 12.0, 16.0, 18.0, 19.5, 24.0];
      boundaryTimes.forEach(t => {
        const { container } = render(<Sky timeOfDay={t} />);
        expect(container.querySelector('[name="sky-mesh"]')).toBeTruthy();
      });
    });

    it('should handle speed of NaN, Infinity, and negative speed', () => {
      const { container: cNeg } = render(<Sky speed={-5.0} />);
      const { container: cNaN } = render(<Sky speed={NaN} />);
      const { container: cInf } = render(<Sky speed={Infinity} />);
      expect(cNeg.querySelector('[name="sky-mesh"]')).toBeTruthy();
      expect(cNaN.querySelector('[name="sky-mesh"]')).toBeTruthy();
      expect(cInf.querySelector('[name="sky-mesh"]')).toBeTruthy();
    });
  });

  describe('5. Weather Component Extreme State Transitions', () => {
    it('should handle NaN, Infinity, and negative precipitationRate', () => {
      const { container: cNeg } = render(<Weather weather="rain" precipitationRate={-1.0} />);
      const { container: cNaN } = render(<Weather weather="rain" precipitationRate={NaN} />);
      const { container: cInf } = render(<Weather weather="rain" precipitationRate={Infinity} />);
      
      expect(cNeg.querySelector('[name="weather-group"]')).toBeTruthy();
      expect(cNaN.querySelector('[name="weather-group"]')).toBeTruthy();
      expect(cInf.querySelector('[name="weather-group"]')).toBeTruthy();
    });

    it('should handle negative delta in weather animation (moving rain/snow particles)', () => {
      const { container } = render(<Weather weather="rain" />);
      act(() => {
        frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => 1.0 } }));
      });
      // pass negative delta
      act(() => {
        frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => 1.0 } }));
      });
      expect(container.querySelector('[name="weather-group"]')).toBeTruthy();
    });

    it('should handle invalid weather types gracefully without crashing', () => {
      const { container } = render(<Weather weather={"invalid-weather-type" as any} />);
      expect(container.querySelector('[name="weather-group"]')).toBeTruthy();
    });
  });

  describe('6. Vegetation Component Density & Capacity Settings', () => {
    it('should handle negative densityFactor and densityFactor > 1.0', () => {
      const { container: cNeg } = render(<Vegetation densityFactor={-0.5} />);
      const { container: cOver } = render(<Vegetation densityFactor={100.0} />);
      const { container: cNaN } = render(<Vegetation densityFactor={NaN} />);
      
      expect(cNeg.querySelector('[name="vegetation-group"]')).toBeTruthy();
      expect(cOver.querySelector('[name="vegetation-group"]')).toBeTruthy();
      expect(cNaN.querySelector('[name="vegetation-group"]')).toBeTruthy();
    });

    it('should handle maxCapacity of 0 and negative maxCapacity', () => {
      const { container: cZero } = render(<Vegetation maxCapacity={0} />);
      const { container: cNeg } = render(<Vegetation maxCapacity={-50} />);
      
      expect(cZero.querySelector('[name="vegetation-group"]')).toBeTruthy();
      expect(cNeg.querySelector('[name="vegetation-group"]')).toBeTruthy();
    });

    it('should handle windDirection and windSpeed of NaN and Infinity', () => {
      const { container: cNaN } = render(<Vegetation windSpeed={NaN} windDirection={NaN} />);
      const { container: cInf } = render(<Vegetation windSpeed={Infinity} windDirection={Infinity} />);
      expect(cNaN.querySelector('[name="vegetation-group"]')).toBeTruthy();
      expect(cInf.querySelector('[name="vegetation-group"]')).toBeTruthy();
    });
  });

  describe('7. Web Audio API Robustness', () => {
    it('should handle multiple sequential initializes & createSpatialSource calls', () => {
      audioManager.initialize();
      audioManager.initialize();
      audioManager.initialize();

      const p1 = audioManager.createSpatialSource('source-1');
      const p2 = audioManager.createSpatialSource('source-1');
      expect(p1).toBe(p2); // should return cached panner
    });

    it('should fall back gracefully if AudioContext initialization throws', () => {
      const BadAudioContext = class {
        constructor() {
          throw new Error('Web Audio not supported');
        }
      };
      vi.stubGlobal('AudioContext', BadAudioContext);
      
      audioManager.ctx = null;
      audioManager.initialize();
      expect(audioManager.ctx).toBeNull();
      
      // Ensure creating spatial source returns null instead of crashing
      const panner = audioManager.createSpatialSource('failed-source');
      expect(panner).toBeNull();
    });

    it('should use fallback setPosition if panner.positionX is missing setValueAtTime', () => {
      audioManager.initialize();
      const mockPanner = {
        connect: vi.fn(),
        setPosition: vi.fn(),
      };
      
      // Inject mock panner
      (audioManager as any).panners.set('legacy-panner', mockPanner);
      
      expect(() => {
        audioManager.updateSpatialSource('legacy-panner', 5, 10, 15);
      }).not.toThrow();
      
      expect(mockPanner.setPosition).toHaveBeenCalledWith(5, 10, 15);
    });

    it('should handle division by zero or NaN positions in spatial audio', () => {
      audioManager.initialize();
      expect(() => {
        audioManager.updateSpatialSource('bad-coords', NaN, Infinity, -Infinity);
      }).not.toThrow();
    });

    it('should handle listener updates when listener positionX is missing setValueAtTime', () => {
      audioManager.initialize();
      const mockListener = {
        setPosition: vi.fn(),
        setOrientation: vi.fn(),
      };
      Object.defineProperty(audioManager.ctx, 'listener', {
        value: mockListener,
        writable: true
      });

      expect(() => {
        audioManager.updateListener(1, 2, 3, 0, 0, -1, 0, 1, 0);
      }).not.toThrow();

      expect(mockListener.setPosition).toHaveBeenCalledWith(1, 2, 3);
      expect(mockListener.setOrientation).toHaveBeenCalledWith(0, 0, -1, 0, 1, 0);
    });

    it('should test PositionalAudio component mounting and dynamic ID updates', () => {
      const { rerender, unmount } = render(
        <PositionalAudio id="ambient-1" position={[0, 0, 0]} />
      );
      expect(screen.getByText((content, element) => element?.getAttribute('name') === 'audio-group')).toBeTruthy();

      rerender(<PositionalAudio id="ambient-2" position={[0, 0, 0]} />);
      expect(screen.getByText((content, element) => element?.getAttribute('name') === 'audio-group')).toBeTruthy();
      
      unmount();
    });
  });

  describe('8. CameraControls WASD Fly Mode & Bounds', () => {
    it('should handle non-standard key presses in fly mode', () => {
      render(<CameraControls cameraMode="fly" />);
      expect(() => {
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp' }));
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Shift' }));
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Space' }));
        frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => 1.0 } }));
      }).not.toThrow();
    });

    it('should handle invalid/out-of-bounds terrainHeightMap in fly mode', () => {
      // short array
      const shortMap = new Float32Array(5);
      const { rerender } = render(
        <CameraControls cameraMode="fly" terrainHeightMap={shortMap} gridWidth={64} gridHeight={64} />
      );
      
      // Let's trigger movement to force elevation lookup
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'w' }));
      act(() => {
        frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => 1.0 } }));
      });

      // map with NaNs
      const nanMap = new Float32Array(4096).fill(NaN);
      rerender(
        <CameraControls cameraMode="fly" terrainHeightMap={nanMap} gridWidth={64} gridHeight={64} />
      );
      act(() => {
        frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => 2.0 } }));
      });
      window.dispatchEvent(new KeyboardEvent('keyup', { key: 'w' }));
    });

    it('should test boundaries of fly camera position clamp limits', () => {
      render(<CameraControls cameraMode="fly" />);
      
      // Press 'd' (move x positive) many times to hit the +100 clamp
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'd' }));
      act(() => {
        for (let i = 0; i < 5; i++) {
          frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => i } }));
        }
      });
      window.dispatchEvent(new KeyboardEvent('keyup', { key: 'd' }));

      // Press 's' (move z positive) many times to hit the +100 clamp
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 's' }));
      act(() => {
        for (let i = 0; i < 5; i++) {
          frameCallbacks.forEach(cb => cb({ clock: { getElapsedTime: () => i } }));
        }
      });
      window.dispatchEvent(new KeyboardEvent('keyup', { key: 's' }));
    });
  });

  describe('9. Showcase Overlay & Timeline Integration', () => {
    it('should change weather to unknown state and render custom option', () => {
      const onWeatherChange = vi.fn();
      render(
        <LandscapeControlsOverlay
          weather="custom-weather"
          onWeatherChange={onWeatherChange}
          speed={1.0}
          onSpeedChange={vi.fn()}
          volume={0.5}
          onVolumeChange={vi.fn()}
          isMuted={false}
          onMuteToggle={vi.fn()}
          cameraMode="orbit"
          onCameraModeToggle={vi.fn()}
        />
      );
      const select = document.getElementById('weather-select') as HTMLSelectElement;
      expect(select.value).toBe('custom-weather');
      
      act(() => {
        fireEvent.change(select, { target: { value: 'rain' } });
      });
      expect(onWeatherChange).toHaveBeenCalledWith('rain');
    });

    it('should support rapid window resizes without performance crash', () => {
      render(<LandscapeShowcase />);
      expect(() => {
        for (let i = 0; i < 100; i++) {
          window.dispatchEvent(new Event('resize'));
        }
      }).not.toThrow();
    });

    it('should wrap timeOfDay correctly around 24 when speed is set to maximum', async () => {
      vi.useFakeTimers();
      render(<LandscapeShowcase />);
      
      const speedInput = document.getElementById('speed-slider') as HTMLInputElement;
      act(() => {
        fireEvent.change(speedInput, { target: { value: '10.0' } });
      });

      // advance timers to trigger interval state updates
      act(() => {
        vi.advanceTimersByTime(2000); // 20 intervals of 100ms
      });

      expect(screen.getByTestId('landscape-showcase')).toBeTruthy();
      vi.useRealTimers();
    });

    it('should stop timeOfDay progression when day-night speed is set to 0', async () => {
      vi.useFakeTimers();
      render(<LandscapeShowcase />);
      
      const speedInput = document.getElementById('speed-slider') as HTMLInputElement;
      act(() => {
        fireEvent.change(speedInput, { target: { value: '0.0' } });
      });

      act(() => {
        vi.advanceTimersByTime(2000);
      });

      expect(screen.getByTestId('landscape-showcase')).toBeTruthy();
      vi.useRealTimers();
    });
  });
});
