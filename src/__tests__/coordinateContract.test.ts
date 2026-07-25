// Frontend proof of property S03 — the cell↔uv↔world coordinate round-trip that the backend pins in
// `src-tauri/src/core/sim_rules.rs` (test `s03_cell_center_round_trips_through_world`). Keeping this
// self-contained (it imports ONLY ./coordinate — no worldGen.ts, three, or R3F) guarantees the two
// language sides agree on which grid cell owns a point without dragging in the render stack.
import { describe, it, expect } from 'vitest';
import {
  cellIndex,
  cellCenterUv,
  uvToCell,
  cellCenterToWorldXz,
  worldXzToCell,
  DEFAULT_XZ_BOUNDS,
  type XZBounds,
} from '../components/Landscape/utils/coordinate';

// Same resolutions the Rust S03 test sweeps, plus the backend default 128² and a couple of tiny
// grids so the whole space is enumerated cheaply.
const GRIDS: Array<[number, number]> = [
  [1, 1],
  [4, 4],
  [16, 16],
  [128, 128],
  [200, 137], // non-square, matches the Rust sweep
];

const bounds: XZBounds = DEFAULT_XZ_BOUNDS;

describe('S03 — coordinate contract round-trips (frontend mirror of sim_rules.rs)', () => {
  it('every cell centre buckets back to the same cell via uv', () => {
    for (const [w, h] of GRIDS) {
      for (let iy = 0; iy < h; iy++) {
        for (let ix = 0; ix < w; ix++) {
          const [u, v] = cellCenterUv(ix, iy, w, h);
          expect(uvToCell(u, v, w, h)).toEqual([ix, iy]);
        }
      }
    }
  });

  it('every cell centre round-trips through world space back to the same cell', () => {
    for (const [w, h] of GRIDS) {
      for (let iy = 0; iy < h; iy++) {
        for (let ix = 0; ix < w; ix++) {
          const [x, z] = cellCenterToWorldXz(ix, iy, w, h, bounds);
          const back = worldXzToCell(x, z, w, h, bounds);
          expect(back).not.toBeNull();
          expect(back).toEqual([ix, iy]);
        }
      }
    }
  });

  it('world → cell → world-centre → cell is a fixed point (idempotent bucketing)', () => {
    const w = 128;
    const h = 128;
    // Arbitrary in-bounds world samples, including both inclusive corners.
    const samples: Array<[number, number]> = [
      [bounds.minX, bounds.minZ],
      [bounds.maxX, bounds.maxZ],
      [0, 0],
      [-42.5, 87.3],
      [99.999, -100],
      [12.34, -56.78],
    ];
    for (const [x, z] of samples) {
      const cell = worldXzToCell(x, z, w, h, bounds);
      expect(cell).not.toBeNull();
      const [ix, iy] = cell as [number, number];
      const [cx, cz] = cellCenterToWorldXz(ix, iy, w, h, bounds);
      expect(worldXzToCell(cx, cz, w, h, bounds)).toEqual([ix, iy]);
    }
  });

  it('the inclusive corners resolve to the extreme cells (matches get_map_indices)', () => {
    const w = 128;
    const h = 128;
    expect(worldXzToCell(bounds.minX, bounds.minZ, w, h, bounds)).toEqual([0, 0]);
    expect(worldXzToCell(bounds.maxX, bounds.maxZ, w, h, bounds)).toEqual([w - 1, h - 1]);
  });

  it('out-of-bounds world points have no cell (null)', () => {
    const w = 128;
    const h = 128;
    expect(worldXzToCell(bounds.minX - 1, 0, w, h, bounds)).toBeNull();
    expect(worldXzToCell(0, bounds.maxZ + 1, w, h, bounds)).toBeNull();
    expect(worldXzToCell(bounds.minX - 0.001, bounds.minZ, w, h, bounds)).toBeNull();
    // A degenerate (zero-area) bounds yields no cell either.
    expect(
      worldXzToCell(0, 0, w, h, { minX: 5, maxX: 5, minZ: -1, maxZ: 1 }),
    ).toBeNull();
  });

  it('flat index is row-major (i = iy*width + ix)', () => {
    expect(cellIndex(0, 0, 128)).toBe(0);
    expect(cellIndex(3, 0, 128)).toBe(3);
    expect(cellIndex(0, 1, 128)).toBe(128);
    expect(cellIndex(5, 2, 16)).toBe(2 * 16 + 5);
  });
});
