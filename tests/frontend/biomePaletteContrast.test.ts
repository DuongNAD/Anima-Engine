import React from 'react';
import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import {
  BIOME_COUNT,
  Biome,
  generateWorld,
  type World,
} from '../../src/components/Landscape/utils/worldGen';
import { BIOME_DISPLAY_RGB } from '../../src/components/Landscape/utils/worldTerrainTextures';
import WorldMinimap, { type CameraView } from '../../src/components/Landscape/WorldMinimap';
import { SHARED_WORLD_OPTS, SHARED_WORLD_SEED } from '../../src/utils/sharedWorld';

/** Minimum 4-connected border edge count at 2048^2 required to include a pair in the contrast gate. */
const MIN_BORDER_EDGES = 100;

/**
 * Calibrated from well-separated adjacent pairs on the 2048^2 shared world
 * (e.g. Shrubland <-> Steppe dE76 = 16.7, Savanna <-> Shrubland dE76 = 20.0 / dL* = 7.1,
 * Shrubland <-> Chaparral dE76 = 19.3 / dL* = 7.8):
 * - Every qualifying non-(River<->Lake) pair must maintain at least MIN_DELTA_E76 = 16.0.
 * - Any pair without strong chroma separation (dC_ab < CHROMA_OVERRIDE_THRESHOLD) must also maintain
 *   at least MIN_VALUE_DELTA_L = 4.5 so adjacent biomes never merge in luminance / grayscale.
 */
const MIN_DELTA_E76 = 16.0;
const CHROMA_OVERRIDE_THRESHOLD = 25.0;
const MIN_VALUE_DELTA_L = 4.5;

type RgbTuple = readonly [number, number, number];
type LabTuple = readonly [number, number, number];

interface AdjacentPairMetric {
  a: Biome;
  b: Biome;
  nameA: string;
  nameB: string;
  edges: number;
  lA: number;
  lB: number;
  deltaL: number;
  deltaChroma: number;
  deltaE76: number;
}

function srgbToLab([r8, g8, b8]: RgbTuple): LabTuple {
  const toLinear = (c: number) => {
    const s = c / 255;
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  const r = toLinear(r8);
  const g = toLinear(g8);
  const b = toLinear(b8);

  // sRGB D65 -> CIE XYZ normalized to D65 reference white.
  const x = (0.4124564 * r + 0.3575761 * g + 0.1804375 * b) / 0.95047;
  const y = (0.2126729 * r + 0.7151522 * g + 0.072175 * b) / 1.0;
  const z = (0.0193339 * r + 0.119192 * g + 0.9503041 * b) / 1.08883;

  const f = (t: number) => (t > 0.008856 ? Math.cbrt(t) : 7.787 * t + 16 / 116);
  const fx = f(x);
  const fy = f(y);
  const fz = f(z);
  return [116 * fy - 16, 500 * (fx - fy), 200 * (fy - fz)];
}

function computeAdjacentBiomeMetrics(
  world: World,
  palette: ReadonlyArray<RgbTuple>,
): AdjacentPairMetric[] {
  const { size, biome } = world;
  const edgeCounts = new Uint32Array(BIOME_COUNT * BIOME_COUNT);

  for (let y = 0; y < size; y++) {
    const row = y * size;
    for (let x = 0; x < size; x++) {
      const a = biome[row + x];
      if (x + 1 < size) {
        const b = biome[row + x + 1];
        if (a !== b) {
          const lo = a < b ? a : b;
          const hi = a < b ? b : a;
          edgeCounts[lo * BIOME_COUNT + hi]++;
        }
      }
      if (y + 1 < size) {
        const b = biome[(y + 1) * size + x];
        if (a !== b) {
          const lo = a < b ? a : b;
          const hi = a < b ? b : a;
          edgeCounts[lo * BIOME_COUNT + hi]++;
        }
      }
    }
  }

  const labs = palette.map(srgbToLab);
  const out: AdjacentPairMetric[] = [];
  for (let a = 0; a < BIOME_COUNT; a++) {
    for (let b = a + 1; b < BIOME_COUNT; b++) {
      const edges = edgeCounts[a * BIOME_COUNT + b];
      if (edges < MIN_BORDER_EDGES) continue;
      const [lA, aA, bA] = labs[a];
      const [lB, aB, bB] = labs[b];
      const deltaL = Math.abs(lA - lB);
      const deltaChroma = Math.hypot(aA - aB, bA - bB);
      const deltaE76 = Math.hypot(deltaL, deltaChroma);
      out.push({
        a: a as Biome,
        b: b as Biome,
        nameA: Biome[a] ?? `Biome(${a})`,
        nameB: Biome[b] ?? `Biome(${b})`,
        edges,
        lA,
        lB,
        deltaL,
        deltaChroma,
        deltaE76,
      });
    }
  }
  out.sort((x, y) => x.deltaE76 - y.deltaE76);
  return out;
}

describe('C3 — Biome display palette CIELAB contrast & minimap legend gate', () => {
  let world: World;

  beforeAll(() => {
    world = generateWorld(SHARED_WORLD_SEED, SHARED_WORLD_OPTS);
  }, 60_000);

  afterEach(() => {
    cleanup();
  });

  it('prints adjacent biome CIELAB table on the shared world and passes contrast + value thresholds', () => {
    expect(BIOME_DISPLAY_RGB.length).toBe(BIOME_COUNT);
    const metrics = computeAdjacentBiomeMetrics(world, BIOME_DISPLAY_RGB);
    expect(metrics.length).toBeGreaterThan(20);

    const lines = metrics.map(
      (m) =>
        `${`${m.nameA} <-> ${m.nameB}`.padEnd(24)} edges=${String(m.edges).padStart(6)}  ` +
        `L*=${m.lA.toFixed(1).padStart(4)}/${m.lB.toFixed(1).padStart(4)}  ` +
        `dL*=${m.deltaL.toFixed(1).padStart(4)}  dC=${m.deltaChroma.toFixed(1).padStart(4)}  ` +
        `dE76=${m.deltaE76.toFixed(1).padStart(5)}`,
    );
    console.log(
      `\n[C3 BIOME CONTRAST TABLE @ ${world.size}x${world.size}, edges >= ${MIN_BORDER_EDGES}]\n` +
        lines.join('\n'),
    );

    const failures: string[] = [];
    for (const m of metrics) {
      const isWaterToWater = m.a === Biome.River && m.b === Biome.Lake;
      if (isWaterToWater) continue;

      if (m.deltaE76 < MIN_DELTA_E76) {
        failures.push(
          `${m.nameA} <-> ${m.nameB} (edges=${m.edges}): dE76=${m.deltaE76.toFixed(2)} < ${MIN_DELTA_E76}`,
        );
      }
      if (m.deltaChroma < CHROMA_OVERRIDE_THRESHOLD && m.deltaL < MIN_VALUE_DELTA_L) {
        failures.push(
          `${m.nameA} <-> ${m.nameB} (edges=${m.edges}): dL*=${m.deltaL.toFixed(2)} < ${MIN_VALUE_DELTA_L} (dC=${m.deltaChroma.toFixed(1)})`,
        );
      }
    }

    // Explicit wetland/dark-green grayscale check (Taiga <-> Bog also separates in L* value).
    const taigaBog = metrics.find(
      (m) =>
        (m.a === Biome.Taiga && m.b === Biome.Bog) ||
        (m.a === Biome.Bog && m.b === Biome.Taiga),
    );
    if (taigaBog && taigaBog.deltaL < MIN_VALUE_DELTA_L) {
      failures.push(
        `Taiga <-> Bog (edges=${taigaBog.edges}): dL*=${taigaBog.deltaL.toFixed(2)} < ${MIN_VALUE_DELTA_L}`,
      );
    }

    expect(failures, `CIELAB contrast failures:\n${failures.join('\n')}`).toEqual([]);
  });

  it('keeps all 22 BIOME_DISPLAY_RGB entries distinct and synced with WorldMinimap legend swatches', () => {
    const seen = new Set(BIOME_DISPLAY_RGB.map((c) => c.join(',')));
    expect(seen.size).toBe(BIOME_COUNT);

    // Ensure every biome appears at least once in a minimal synthetic minimap sample or check
    // WorldMinimap legend directly against BIOME_DISPLAY_RGB using a tiny test world containing all 22.
    const mini = generateWorld('c3-legend-sync', { size: 32, shape: 'island' });
    for (let b = 0; b < BIOME_COUNT; b++) {
      mini.biome[b] = b;
    }
    const viewRef = {
      current: { targetX: 0, targetZ: 0, camX: 0, camZ: 20 } satisfies CameraView,
    };
    render(
      React.createElement(WorldMinimap, {
        world: mini,
        renderSize: 400,
        viewRef,
        onTeleport: () => undefined,
      }),
    );

    const legend = screen.getByTestId('minimap-biome-legend');
    const entries = legend.querySelectorAll('[data-biome]');
    expect(entries.length).toBe(BIOME_COUNT);
    for (const row of entries) {
      const bio = Number(row.getAttribute('data-biome'));
      const swatch = row.querySelector('span') as HTMLElement | null;
      expect(swatch).not.toBeNull();
      const [r, g, b] = BIOME_DISPLAY_RGB[bio];
      expect(swatch!.style.background).toBe(`rgb(${r}, ${g}, ${b})`);
    }
  });
});
