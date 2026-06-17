import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, act, screen } from '@testing-library/react';
import * as THREE from 'three';
import Sky from '../../src/components/Landscape/Sky';
import Weather from '../../src/components/Landscape/Weather';

let originalSetAttribute: any;
let frameCallbacks: Array<(state: any, delta: number) => void> = [];

vi.mock('@react-three/fiber', async () => {
  return {
    Canvas: ({ children }: any) => <div data-testid="mock-canvas">{children}</div>,
    useFrame: (cb: any) => {
      frameCallbacks.push(cb);
    },
  };
});

describe('Sky and Weather Component Tests', () => {
  let consoleErrorSpy: any;

  beforeEach(() => {
    frameCallbacks = [];
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    // Custom screen.getByTestId to find elements in JSDOM mocks
    screen.getByTestId = (id: string) => {
      const el = document.querySelector(`[name="${id}"]`) || document.querySelector(`[data-testid="${id}"]`);
      if (!el) {
        throw new Error(`Unable to find element with testid/name="${id}"`);
      }
      return el as any;
    };

    // Mock position and rotation for JSDOM R3F elements
    Object.defineProperty(Element.prototype, 'position', {
      get() {
        if (!this._mockPosition) {
          this._mockPosition = { x: 0, y: 0, z: 0 };
        }
        return this._mockPosition;
      },
      configurable: true,
    });

    Object.defineProperty(Element.prototype, 'rotation', {
      get() {
        if (!this._mockRotation) {
          this._mockRotation = { x: 0, y: 0, z: 0 };
        }
        return this._mockRotation;
      },
      configurable: true,
    });

    // Mock geometry getter
    Object.defineProperty(HTMLElement.prototype, 'geometry', {
      get() {
        if (!this._mockGeometry) {
          this._mockGeometry = {
            getAttribute: vi.fn().mockImplementation((name: string) => {
              if (name === 'position') {
                if (!this._mockPositionAttr) {
                  this._mockPositionAttr = {
                    array: new Float32Array(1500 * 3),
                    needsUpdate: false,
                  };
                }
                return this._mockPositionAttr;
              }
              return null;
            }),
          };
        }
        return this._mockGeometry;
      },
      configurable: true,
    });

    originalSetAttribute = HTMLElement.prototype.setAttribute;
    HTMLElement.prototype.setAttribute = vi.fn().mockImplementation(function (
      this: any,
      name: string,
      value: any
    ) {
      if (value instanceof THREE.BufferAttribute) {
        if (!this._capturedAttributes) {
          this._capturedAttributes = new Map();
        }
        this._capturedAttributes.set(name, value);
      } else {
        originalSetAttribute.call(this, name, value);
      }
    });
  });

  afterEach(() => {
    consoleErrorSpy.mockRestore();
    if (originalSetAttribute) {
      HTMLElement.prototype.setAttribute = originalSetAttribute;
    }
    delete (Element.prototype as any).position;
    delete (Element.prototype as any).rotation;
    delete (HTMLElement.prototype as any).geometry;
    delete (HTMLElement.prototype as any)._capturedAttributes;
  });

  describe('Sky Component', () => {
    it('should render sky dome, clouds, and directional light at noon', () => {
      const { container } = render(<Sky timeOfDay={12.0} />);

      const skyGroup = container.querySelector('[name="sky-group"]');
      expect(skyGroup).toBeTruthy();
      expect(skyGroup?.getAttribute('data-time-of-day')).toBe('12');

      const skyMesh = container.querySelector('[name="sky-mesh"]');
      expect(skyMesh).toBeTruthy();

      const skyLight = container.querySelector('[name="sky-light"]');
      expect(skyLight).toBeTruthy();

      const cloudsGroup = container.querySelector('[name="clouds-group"]');
      expect(cloudsGroup).toBeTruthy();

      // At noon, stars should not be visible, moon should not be visible
      const moonMesh = container.querySelector('[name="moon-mesh"]');
      const starsParticles = container.querySelector('[name="stars-particles"]');
      expect(moonMesh).toBeNull();
      expect(starsParticles).toBeNull();
    });

    it('should render stars and moon at midnight', () => {
      const { container } = render(<Sky timeOfDay={0.0} />);

      const moonMesh = container.querySelector('[name="moon-mesh"]');
      const starsParticles = container.querySelector('[name="stars-particles"]');
      expect(moonMesh).toBeTruthy();
      expect(starsParticles).toBeTruthy();
    });

    it('should rotate sky and drift clouds in the rendering frame loop', () => {
      const { container } = render(<Sky speed={2.0} timeOfDay={12.0} />);
      const cloudsGroup = container.querySelector('[name="clouds-group"]') as any;
      expect(cloudsGroup).toBeTruthy();

      // Record initial cloud X positions
      const initialPositions = Array.from(cloudsGroup.children).map((child: any) => child.position.x);

      // Simulate frame updates
      const mockState = {
        clock: {
          getElapsedTime: () => 5.0,
        },
      };

      act(() => {
        // Run frameCallbacks multiple times
        frameCallbacks.forEach((cb) => cb(mockState, 0.05));
      });

      // Verify at least one cloud position changed
      const updatedPositions = Array.from(cloudsGroup.children).map((child: any) => child.position.x);
      let changed = false;
      for (let i = 0; i < initialPositions.length; i++) {
        if (Math.abs(updatedPositions[i] - initialPositions[i]) > 0.001) {
          changed = true;
          break;
        }
      }
      expect(changed).toBe(true);
    });
  });

  describe('Weather Component', () => {
    it('should render rain particles and fog matching rain status', () => {
      const { container } = render(<Weather weather="rain" precipitationRate={0.8} />);

      const weatherGroup = container.querySelector('[name="weather-group"]');
      expect(weatherGroup).toBeTruthy();
      expect(weatherGroup?.getAttribute('data-weather')).toBe('rain');
      expect(weatherGroup?.getAttribute('data-particle-count')).toBe(String(Math.floor(1000 * 0.8)));

      const particles = container.querySelector('[name="weather-particles"]');
      expect(particles).toBeTruthy();
    });

    it('should render snow particles matching snow status', () => {
      const { container } = render(<Weather weather="snow" precipitationRate={0.6} />);

      const weatherGroup = container.querySelector('[name="weather-group"]');
      expect(weatherGroup?.getAttribute('data-weather')).toBe('snow');
      expect(weatherGroup?.getAttribute('data-particle-count')).toBe(String(Math.floor(800 * 0.6)));

      const particles = container.querySelector('[name="weather-particles"]');
      expect(particles).toBeTruthy();
    });

    it('should render fog density and no particles for clear weather', () => {
      const { container } = render(<Weather weather="clear" />);

      const weatherGroup = container.querySelector('[name="weather-group"]');
      expect(weatherGroup?.getAttribute('data-fog-density')).toBe('0.005');

      const particles = container.querySelector('[name="weather-particles"]');
      expect(particles).toBeNull();
    });

    it('should smoothly transition between weather states in useFrame', () => {
      const { container, rerender } = render(<Weather weather="clear" />);

      let weatherGroup = container.querySelector('[name="weather-group"]');
      expect(weatherGroup?.getAttribute('data-particle-count')).toBe('0');
      expect(weatherGroup?.getAttribute('data-fog-density')).toBe('0.005');

      // Shift to rain
      rerender(<Weather weather="rain" precipitationRate={1.0} />);

      // Right after shift, before frames run, it should start lerp from clear values
      weatherGroup = container.querySelector('[name="weather-group"]');
      const intermediateCount = parseInt(weatherGroup?.getAttribute('data-particle-count') || '0', 10);
      const intermediateFog = parseFloat(weatherGroup?.getAttribute('data-fog-density') || '0');
      
      // Let's run a frame update to animate transition
      const mockState = {
        clock: {
          getElapsedTime: () => 1.0,
        },
      };

      act(() => {
        frameCallbacks.forEach((cb) => cb(mockState, 0.05)); // 50ms step
      });

      // The count and fog density should have progressed towards rain values
      weatherGroup = container.querySelector('[name="weather-group"]');
      const updatedCount = parseInt(weatherGroup?.getAttribute('data-particle-count') || '0', 10);
      const updatedFog = parseFloat(weatherGroup?.getAttribute('data-fog-density') || '0');

      expect(updatedCount).toBeGreaterThan(intermediateCount);
      expect(updatedFog).toBeGreaterThan(intermediateFog);
    });
  });
});
