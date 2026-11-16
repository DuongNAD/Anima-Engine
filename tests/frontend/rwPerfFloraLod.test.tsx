import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/react';
import * as THREE from 'three';
import WorldVegetation from '../../src/components/Landscape/WorldVegetation';
import {
  CELL_FAR,
  CELL_HIDDEN,
  CELL_MID,
  CELL_NEAR,
  FLORA_IMPOSTOR_TRIANGLES,
  collectCellRanges,
  floraCellOf,
  floraDensityCount,
  floraDensityKeep,
  frustumPlanesFromViewProjection,
  multiplyMat4,
  selectFloraCells,
} from '../../src/components/Landscape/utils/chunkLod';
import {
  buildFloraSources,
  makeImpostorGeometry,
} from '../../src/components/Landscape/utils/floraChunks';
import { makeFloraGeometry } from '../../src/components/Landscape/utils/floraGeometry';
import { FloraType, generateWorld } from '../../src/components/Landscape/utils/worldGen';
import { QUALITY_TIER_CONFIGS, QUALITY_TIERS } from '../../src/components/Landscape/utils/qualityTiers';

const RENDER = 1200;
const C = 16;

function flatInput(camX: number, camY: number, camZ: number, planes: ArrayLike<number> | null, near = 160, mid = 280) {
  return {
    cellsPerSide: C,
    renderSize: RENDER,
    cellYMin: new Float32Array(C * C).fill(0),
    cellYMax: new Float32Array(C * C).fill(10),
    camX,
    camY,
    camZ,
    planes,
    nearRadius: near,
    midRadius: mid,
    frustumPad: 0,
  };
}

function planesFor(pos: [number, number, number], target: [number, number, number]): Float64Array {
  const cam = new THREE.PerspectiveCamera(55, 4 / 3, 0.5, 6000);
  cam.position.set(...pos);
  cam.lookAt(...target);
  cam.updateMatrixWorld(true);
  return frustumPlanesFromViewProjection(multiplyMat4(cam.projectionMatrix.elements, cam.matrixWorldInverse.elements));
}

describe('RW4-2 chunk selection by distance and frustum', () => {
  it('classifies cells near / mid / far by 3D distance with no frustum', () => {
    const cls = selectFloraCells(flatInput(0, 5, 0, null));
    const at = (x: number, z: number) => cls[floraCellOf(x, z, RENDER, C)];
    expect(at(1, 1)).toBe(CELL_NEAR);
    expect(at(260, 0)).toBe(CELL_MID); // cell box spans 225..300 -> nearest point 225 > 160, <= 280
    expect(at(550, 550)).toBe(CELL_FAR);
  });

  it('hides cells behind the camera and keeps those in front', () => {
    const planes = planesFor([0, 20, 0], [0, 5, -300]);
    const cls = selectFloraCells(flatInput(0, 20, 0, planes, 400, 800));
    expect(cls[floraCellOf(0, -150, RENDER, C)]).toBe(CELL_NEAR);
    expect(cls[floraCellOf(0, 300, RENDER, C)]).toBe(CELL_HIDDEN);
    expect(cls[floraCellOf(500, 0, RENDER, C)]).toBe(CELL_HIDDEN); // far off to the side
  });

  it('keeps every tree-bearing cell as FAR (never hard-culled) when the camera is far above', () => {
    // Overview-like: ~950 units out, beyond every tier's floraMidRadius.
    const planes = planesFor([0, 700, 700], [0, 0, 0]);
    const cls = selectFloraCells(flatInput(0, 700, 700, planes, 360, 700));
    const farCells = Array.from(cls).filter((c) => c === CELL_FAR).length;
    expect(farCells).toBeGreaterThan(40);
    expect(Array.from(cls).filter((c) => c === CELL_NEAR).length).toBe(0);
  });

  it('marks empty cells hidden', () => {
    const input = flatInput(0, 5, 0, null);
    input.cellYMin = new Float32Array(C * C).fill(Infinity);
    input.cellYMax = new Float32Array(C * C).fill(-Infinity);
    expect(Array.from(selectFloraCells(input)).every((c) => c === CELL_HIDDEN)).toBe(true);
  });

  it('merges adjacent selected cells into contiguous ranges', () => {
    const cellStart = [0, 2, 4, 4, 7];
    expect(collectCellRanges(cellStart, [1, 1, 1, 0], (c) => c === 1)).toEqual([0, 4]);
    expect(collectCellRanges(cellStart, [1, 0, 1, 1], (c) => c === 1)).toEqual([0, 2, 4, 7]);
  });
});

describe('RW4-2 impostor', () => {
  it('has at most 8 triangles (6 by design)', () => {
    const g = makeImpostorGeometry();
    const tris = (g.index ? g.index.count : g.attributes.position.count) / 3;
    expect(tris).toBe(FLORA_IMPOSTOR_TRIANGLES);
    expect(tris).toBeLessThanOrEqual(8);
    g.dispose();
  });
});

describe('RW4-3 density per tier comes from the single tier table', () => {
  it('table ratios are 1/4, 1/2, 1', () => {
    expect(QUALITY_TIER_CONFIGS.low.vegetationDensity).toBe(0.25);
    expect(QUALITY_TIER_CONFIGS.medium.vegetationDensity).toBe(0.5);
    expect(QUALITY_TIER_CONFIGS.high.vegetationDensity).toBe(1);
  });

  it('floraDensityKeep keeps exactly floor(n * density) of n', () => {
    for (const d of [0.25, 0.5, 1]) {
      for (const n of [0, 1, 3, 4, 7, 100, 1001]) {
        let kept = 0;
        for (let s = 0; s < n; s++) if (floraDensityKeep(s, d)) kept++;
        expect(kept).toBe(floraDensityCount(n, d));
        expect(kept).toBe(Math.floor(n * d));
      }
    }
  });

  it('instance counts built per tier follow those ratios, type by type', () => {
    const world = generateWorld('rw-c3-density', { size: 128, shape: 'island' });
    expect(world.floraCount).toBeGreaterThan(200);
    const geoms = [
      FloraType.Pine,
      FloraType.Round,
      FloraType.Rock,
      FloraType.Bush,
      FloraType.Tuft,
    ].map((type) => ({ type, geometry: makeFloraGeometry(type) }));
    const totals = new Map<FloraType, number>();
    for (let i = 0; i < world.floraCount; i++) {
      const t = world.floraType[i] as FloraType;
      totals.set(t, (totals.get(t) ?? 0) + 1);
    }
    const sumByTier: Record<string, number> = {};
    for (const tier of QUALITY_TIERS) {
      const d = QUALITY_TIER_CONFIGS[tier].vegetationDensity;
      const src = buildFloraSources(world, geoms, {
        renderSize: RENDER,
        heightRatio: 0.14,
        meshResolution: 64,
        baseSize: 1.4,
        density: d,
      });
      let sum = 0;
      for (const ts of src.types) {
        expect(ts.count).toBe(Math.floor((totals.get(ts.type) ?? 0) * d));
        sum += ts.count;
      }
      sumByTier[tier] = sum;
      // Impostors exist only for trees and shrubs, at the same density.
      const treeShrub = src.types
        .filter((t) => [FloraType.Pine, FloraType.Round, FloraType.Rock, FloraType.Bush].includes(t.type))
        .reduce((a, t) => a + t.count, 0);
      expect(src.impostor.count).toBe(treeShrub);
      expect(src.impostor.slotStart[src.impostor.slotStart.length - 1]).toBe(src.impostor.count);
    }
    expect(sumByTier.low / sumByTier.high).toBeCloseTo(0.25, 1);
    expect(sumByTier.medium / sumByTier.high).toBeCloseTo(0.5, 1);
  });
});

describe('RW4-4 rule: capture mode keeps the legacy full drawing path', () => {
  const world = generateWorld('rw-c3-capture', { size: 32, shape: 'island' });

  afterEach(() => {
    cleanup();
    window.history.pushState({}, '', '/');
  });

  function names(container: HTMLElement): string[] {
    return Array.from(container.querySelectorAll('[name]')).map((e) => e.getAttribute('name') ?? '');
  }

  it('an ordinary visit with a tier mounts the chunked path (impostor mesh present)', () => {
    window.history.pushState({}, '', '/landscape.html');
    const { container } = render(<WorldVegetation world={world} renderSize={RENDER} meshResolution={64} tier="low" />);
    expect(names(container)).toContain('flora-impostor');
  });

  it('?capture=1 renders the legacy per-type meshes only, with no impostor, whatever the tier', () => {
    window.history.pushState({}, '', '/landscape.html?capture=1&view=overview&quality=high');
    const { container } = render(<WorldVegetation world={world} renderSize={RENDER} meshResolution={64} tier="high" quality="high" />);
    const n = names(container);
    expect(n).not.toContain('flora-impostor');
    expect(n.some((x) => x.startsWith('flora-'))).toBe(true);
  });

  it('without a tier prop the legacy path runs (no impostor)', () => {
    window.history.pushState({}, '', '/landscape.html');
    const { container } = render(<WorldVegetation world={world} renderSize={RENDER} meshResolution={64} />);
    expect(names(container)).not.toContain('flora-impostor');
  });
});
