import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import * as THREE from 'three';
import { render, screen, fireEvent, act, cleanup } from '@testing-library/react';
import { LandscapeControlsOverlay } from '../../src/components/Landscape/LandscapeControlsOverlay';
import LandscapeShowcase from '../../src/components/Landscape/LandscapeShowcase';
import { clearTerrainCache } from '../../src/components/Landscape/utils/terrainGenerator';

// Mock Canvas and R3F components for test environment
vi.mock('@react-three/fiber', async () => {
  return {
    Canvas: ({ children }: any) => <div data-testid="mock-canvas">{children}</div>,
    useFrame: vi.fn(),
    useThree: () => ({
      camera: { position: { set: vi.fn(), x: 0, y: 25, z: 45 }, lookAt: vi.fn() },
      scene: { add: vi.fn(), remove: vi.fn() },
      gl: { setSize: vi.fn(), domElement: document.createElement('canvas') },
    }),
    extend: vi.fn(),
  };
});

// Mock Web Audio API
class MockAudioContext {
  sampleRate = 44100;
  currentTime = 0;
  createGain() {
    return { gain: { value: 1, setValueAtTime: vi.fn() }, connect: vi.fn() };
  }
  createPanner() {
    return { connect: vi.fn() };
  }
  createBuffer() {
    return { getChannelData: vi.fn().mockReturnValue(new Float32Array(100)) };
  }
  createBufferSource() {
    return { connect: vi.fn().mockReturnThis(), start: vi.fn(), stop: vi.fn() };
  }
  destination = {};
  listener = {};
}
vi.stubGlobal('AudioContext', MockAudioContext);
vi.stubGlobal('webkitAudioContext', MockAudioContext);

describe('Terrain UI & Cache Controls Test Suite', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    clearTerrainCache();
  });

  afterEach(() => {
    cleanup();
  });

  describe('LandscapeControlsOverlay UI Unit Tests', () => {
    it('renders "Clear Cache & Regenerate" button and default cache status', () => {
      render(
        <LandscapeControlsOverlay
          weather="clear"
          onWeatherChange={vi.fn()}
          speed={1.0}
          onSpeedChange={vi.fn()}
          volume={0.5}
          onVolumeChange={vi.fn()}
          isMuted={false}
          onMuteToggle={vi.fn()}
          cameraMode="orbit"
          onCameraModeChange={vi.fn()}
          timeOfDay={12.0}
        />
      );

      // Verify button rendering
      const regenBtn = screen.getByRole('button', { name: /Clear Cache & Regenerate/i });
      expect(regenBtn).toBeTruthy();
      expect(regenBtn.textContent).toContain('♻️ Clear Cache & Regenerate');

      // Verify default cache status display
      expect(screen.getByText(/Cached \(IndexedDB\)/i)).toBeTruthy();
    });

    it('renders custom cacheStatus when provided', () => {
      render(
        <LandscapeControlsOverlay
          weather="clear"
          onWeatherChange={vi.fn()}
          speed={1.0}
          onSpeedChange={vi.fn()}
          volume={0.5}
          onVolumeChange={vi.fn()}
          isMuted={false}
          onMuteToggle={vi.fn()}
          cameraMode="orbit"
          onCameraModeChange={vi.fn()}
          timeOfDay={12.0}
          cacheStatus="Computed (Noise)"
        />
      );

      expect(screen.getByText(/Computed \(Noise\)/i)).toBeTruthy();
    });

    it('invokes onRegenerateCache callback when button is clicked', () => {
      const handleRegenerate = vi.fn();
      render(
        <LandscapeControlsOverlay
          weather="clear"
          onWeatherChange={vi.fn()}
          speed={1.0}
          onSpeedChange={vi.fn()}
          volume={0.5}
          onVolumeChange={vi.fn()}
          isMuted={false}
          onMuteToggle={vi.fn()}
          cameraMode="orbit"
          onCameraModeChange={vi.fn()}
          timeOfDay={12.0}
          onRegenerateCache={handleRegenerate}
        />
      );

      const regenBtn = screen.getByRole('button', { name: /Clear Cache & Regenerate/i });
      fireEvent.click(regenBtn);

      expect(handleRegenerate).toHaveBeenCalledTimes(1);
    });
  });

  describe('LandscapeShowcase Cache Control Integration Tests', () => {
    it('renders LandscapeShowcase with cache controls present', () => {
      render(<LandscapeShowcase />);

      expect(screen.getByTestId('landscape-showcase')).toBeTruthy();
      expect(screen.getByRole('button', { name: /Clear Cache & Regenerate/i })).toBeTruthy();
      expect(screen.getByText(/Cached \(IndexedDB\)/i)).toBeTruthy();
    });

    it('clears cache and updates cache status when Clear Cache button is clicked in LandscapeShowcase', async () => {
      vi.useFakeTimers();
      render(<LandscapeShowcase />);

      const regenBtn = screen.getByRole('button', { name: /Clear Cache & Regenerate/i });

      act(() => {
        fireEvent.click(regenBtn);
      });

      // Instantly status changes to Computed (Noise)
      expect(screen.getByText(/Computed \(Noise\)/i)).toBeTruthy();

      // Fast-forward timers
      act(() => {
        vi.advanceTimersByTime(600);
      });

      // Status resets to Cached (IndexedDB)
      expect(screen.getByText(/Cached \(IndexedDB\)/i)).toBeTruthy();

      vi.useRealTimers();
    });
  });
});
