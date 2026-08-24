import { describe, it, expect } from 'vitest';
import {
  SIM_BOUNDS_EXTENT,
  SIM_BOUNDS_HALF,
  isInsideSimBounds,
  renderToSim,
  simToRender,
} from '../../src/components/Landscape/utils/liveAgentTransform';

// The render size the landscape actually uses (`FLORA_RADIUS_REFERENCE_EXTENT`). Restated here on
// purpose: if the scene's constant moves, these expectations should be re-derived by a human rather
// than following it silently and continuing to pass.
const RENDER_SIZE = 1200;

describe('simulation ↔ render coordinates', () => {
  it('maps the simulation bounds onto the full render footprint', () => {
    // The map edge in world units lands on the map edge in render units. Getting this wrong draws a
    // population correctly *shaped* and wrongly *sized*, which reads as a physics bug rather than a
    // units bug — the same confusion that made every flora collider six times too fat.
    expect(simToRender(SIM_BOUNDS_HALF, RENDER_SIZE)).toBe(RENDER_SIZE / 2);
    expect(simToRender(-SIM_BOUNDS_HALF, RENDER_SIZE)).toBe(-RENDER_SIZE / 2);
    expect(simToRender(0, RENDER_SIZE)).toBe(0);
  });

  it('is the factor the two spaces actually differ by', () => {
    expect(simToRender(1, RENDER_SIZE)).toBe(RENDER_SIZE / SIM_BOUNDS_EXTENT);
    expect(simToRender(1, RENDER_SIZE)).toBe(6);
  });

  it('round-trips', () => {
    for (const world of [-100, -37.5, 0, 12.25, 99.9]) {
      expect(renderToSim(simToRender(world, RENDER_SIZE), RENDER_SIZE)).toBeCloseTo(world, 10);
    }
  });

  it('follows renderSize rather than hard-coding six', () => {
    // A future scene at a different extent must not need this module edited.
    expect(simToRender(SIM_BOUNDS_HALF, 600)).toBe(300);
    expect(simToRender(SIM_BOUNDS_HALF, 2400)).toBe(1200);
  });

  it('mirrors MapBounds::default(), which is the source of the number', () => {
    // `min = (-100, 0, -100)`, `max = (100, 10, 100)` in `src-tauri/src/core/resources.rs`.
    expect(SIM_BOUNDS_EXTENT).toBe(200);
    expect(SIM_BOUNDS_HALF).toBe(100);
  });
});

describe('bounds rejection', () => {
  it('accepts positions inside the simulated world, including its edge', () => {
    expect(isInsideSimBounds(0, 0)).toBe(true);
    expect(isInsideSimBounds(-100, 100)).toBe(true);
    expect(isInsideSimBounds(99.99, -12)).toBe(true);
  });

  it('rejects a position outside the bounds instead of clamping it to the edge', () => {
    // This is the legacy founding layout's failure mode: `x = i * 5.0` put founder 999 at 4995.
    // Drawing that at the map edge would show a tidy population and hide the disagreement.
    expect(isInsideSimBounds(4995, 0)).toBe(false);
    expect(isInsideSimBounds(0, -100.5)).toBe(false);
  });

  it('rejects non-finite coordinates', () => {
    expect(isInsideSimBounds(Number.NaN, 0)).toBe(false);
    expect(isInsideSimBounds(0, Number.POSITIVE_INFINITY)).toBe(false);
  });
});
