import { describe, it, expect } from 'vitest';
import {
  makeChunkGrid,
  lodForDistance,
  resForLod,
  estimateCost,
  assignChunkLods,
  updateActiveChunks,
} from '../components/Landscape/utils/chunkLod';

describe('chunkLod grid', () => {
  it('splits the unit square into C*C chunks that tile with no gaps or overlaps', () => {
    const C = 6;
    const grid = makeChunkGrid(C);
    expect(grid).toHaveLength(C * C);
    // Every chunk's range is 1/C wide, centres are correct, and the union covers [0,1].
    let minU = 1, maxU = 0, minV = 1, maxV = 0;
    for (const c of grid) {
      expect(c.u1 - c.u0).toBeCloseTo(1 / C, 6);
      expect(c.v1 - c.v0).toBeCloseTo(1 / C, 6);
      expect(c.cu).toBeCloseTo((c.u0 + c.u1) / 2, 6);
      expect(c.cv).toBeCloseTo((c.v0 + c.v1) / 2, 6);
      minU = Math.min(minU, c.u0);
      maxU = Math.max(maxU, c.u1);
      minV = Math.min(minV, c.v0);
      maxV = Math.max(maxV, c.v1);
    }
    expect(minU).toBeCloseTo(0, 6);
    expect(maxU).toBeCloseTo(1, 6);
    expect(minV).toBeCloseTo(0, 6);
    expect(maxV).toBeCloseTo(1, 6);
  });

  it('makes adjacent chunks share an exact edge (seamless at equal LOD)', () => {
    const C = 4;
    const grid = makeChunkGrid(C);
    const at = (ix: number, iz: number) => grid.find((c) => c.ix === ix && c.iz === iz)!;
    // The right edge of (0,0) is the left edge of (1,0); the bottom of (0,0) is the top of (0,1).
    expect(at(0, 0).u1).toBeCloseTo(at(1, 0).u0, 9);
    expect(at(0, 0).v1).toBeCloseTo(at(0, 1).v0, 9);
  });
});

describe('chunkLod LOD selection', () => {
  it('picks a coarser LOD the farther a chunk is', () => {
    const d = [100, 300, 700];
    expect(lodForDistance(50, d)).toBe(0);
    expect(lodForDistance(150, d)).toBe(1);
    expect(lodForDistance(400, d)).toBe(2);
    expect(lodForDistance(1000, d)).toBe(3);
  });

  it('halves resolution per LOD and clamps at minRes', () => {
    // baseRes 384 over 6 chunks → 64 segments at full detail.
    expect(resForLod(384, 6, 0)).toBe(64);
    expect(resForLod(384, 6, 1)).toBe(32);
    expect(resForLod(384, 6, 2)).toBe(16);
    expect(resForLod(384, 6, 10, 4)).toBe(4); // clamped
  });
});

describe('chunkLod camera-dynamic LOD', () => {
  const renderSize = 1200;

  it('gives every chunk LOD 0 when no LOD distances are set', () => {
    const grid = makeChunkGrid(6);
    const lods = assignChunkLods(grid, 0, 0, renderSize, []);
    expect(lods).toHaveLength(grid.length);
    expect(lods.every((l) => l === 0)).toBe(true);
  });

  it('keeps the chunk under the camera at full detail and coarsens with distance', () => {
    const grid = makeChunkGrid(6);
    const lodDistances = [150, 350];
    // Camera over the top-left corner of the map.
    const camX = -renderSize / 2;
    const camZ = -renderSize / 2;
    const lods = assignChunkLods(grid, camX, camZ, renderSize, lodDistances);
    const at = (ix: number, iz: number) => lods[grid.findIndex((c) => c.ix === ix && c.iz === iz)];
    // The corner chunk (nearest the camera) is finest; the opposite corner is coarsest.
    expect(at(0, 0)).toBe(0);
    expect(at(5, 5)).toBeGreaterThan(at(0, 0));
  });

  it('re-centres detail when the camera moves to the opposite corner', () => {
    const grid = makeChunkGrid(6);
    const d = [150, 350];
    const near = assignChunkLods(grid, renderSize / 2, renderSize / 2, renderSize, d);
    const at = (lods: number[], ix: number, iz: number) =>
      lods[grid.findIndex((c) => c.ix === ix && c.iz === iz)];
    // Now the far (5,5) corner is under the camera → finest there, coarse at (0,0).
    expect(at(near, 5, 5)).toBe(0);
    expect(at(near, 0, 0)).toBeGreaterThan(0);
  });
});

describe('chunkLod streaming (updateActiveChunks)', () => {
  const renderSize = 1200;

  it('keeps the resident chunk count bounded regardless of world size', () => {
    // A big 16×16 world; a load radius that only covers a local neighbourhood.
    const grid = makeChunkGrid(16);
    const chunkSpan = renderSize / 16; // ~75 units per chunk
    const load = chunkSpan * 2.5;
    const unload = chunkSpan * 3.5;
    let active = new Set<number>();
    let maxResident = 0;
    // Sweep the camera across the whole map; the resident set must never blow up.
    for (let x = -renderSize / 2; x <= renderSize / 2; x += chunkSpan) {
      const r = updateActiveChunks(grid, x, 0, renderSize, load, unload, active);
      active = r.active;
      maxResident = Math.max(maxResident, active.size);
    }
    expect(maxResident).toBeGreaterThan(0);
    expect(maxResident).toBeLessThan(grid.length); // never the whole 256-chunk world
    expect(maxResident).toBeLessThan(40); // a bounded local window
  });

  it('hysteresis: a chunk between load and unload radius keeps its previous state', () => {
    const grid = makeChunkGrid(6);
    // Chunk (0,0) centre is at world (-500,-500). Put the camera so its distance sits in the band.
    const c00 = grid.findIndex((c) => c.ix === 0 && c.iz === 0);
    const cx = (grid[c00].cu - 0.5) * renderSize;
    const cz = (grid[c00].cv - 0.5) * renderSize;
    const load = 100;
    const unload = 400;
    const camX = cx + 250; // 250 units away → between load(100) and unload(400)
    // If it was loaded, it stays loaded.
    let r = updateActiveChunks(grid, camX, cz, renderSize, load, unload, new Set([c00]));
    expect(r.active.has(c00)).toBe(true);
    // If it was unloaded, it stays unloaded.
    r = updateActiveChunks(grid, camX, cz, renderSize, load, unload, new Set());
    expect(r.active.has(c00)).toBe(false);
  });

  it('reports changed only when the resident set actually changes', () => {
    const grid = makeChunkGrid(6);
    const first = updateActiveChunks(grid, 0, 0, renderSize, 300, 500, new Set());
    expect(first.changed).toBe(true);
    const again = updateActiveChunks(grid, 0, 0, renderSize, 300, 500, first.active);
    expect(again.changed).toBe(false);
  });
});

describe('chunkLod cost estimate', () => {
  const baseRes = 384;
  const renderSize = 1200;

  it('a full-detail, un-culled grid costs exactly the single uniform mesh', () => {
    const grid = makeChunkGrid(6);
    const r = estimateCost(grid, {
      renderSize,
      baseRes,
      camX: 0,
      camZ: 0,
      cullDistance: 1e9,
      lodDistances: [], // no LOD drop anywhere
    });
    expect(r.triangles).toBe(r.uniformTriangles);
    expect(r.ratio).toBeCloseTo(1, 6);
    expect(r.culled).toBe(0);
  });

  it('culls off-view chunks and drops far-chunk detail, cutting the triangle budget', () => {
    const grid = makeChunkGrid(6);
    const r = estimateCost(grid, {
      renderSize,
      baseRes,
      camX: 0, // near the map centre, looking around locally (walk/fly)
      camZ: 0,
      cullDistance: 500, // can't see the far corners
      lodDistances: [150, 350],
    });
    expect(r.culled).toBeGreaterThan(0);
    expect(r.drawn).toBeLessThan(grid.length);
    expect(r.triangles).toBeLessThan(r.uniformTriangles);
    expect(r.ratio).toBeLessThan(1);
  });
});
