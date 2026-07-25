// Shared coordinate contract — the FRONTEND side of the cell↔uv↔world transforms that the backend
// pins in `src-tauri/src/core/sim_rules.rs`. These are the exact same pure functions, so a cell
// centre computed here buckets back to the same cell the Rust sim would pick (property S03), and a
// world position handed across the IPC boundary lands in the identical grid cell on both sides.
//
// Four spaces (see COORDINATE_CONTRACT.md at the repo root for the full prose):
//   • cell   — integer (ix, iy), 0 ≤ ix < width, 0 ≤ iy < height; flat index i = iy*width + ix.
//   • uv     — normalized [0,1]²; a cell's *centre* is uv = ((ix+0.5)/width, (iy+0.5)/height).
//   • world  — backend sim space; x,z ∈ [minX,maxX]×[minZ,maxZ] (default MapBounds = ±100).
//   • render — the 3D scene; a pure scale of `world`, applied frontend-side (no rotation/shear).
//
// There are TWO distinct world→grid rules and this module implements the *cell-bucket* one
// (`TerrainMap::get_map_indices`, terrain.rs:563): floor(coord·dim) clamped to the last cell, used
// for "which cell owns this point". The separate *node-interpolation* rule
// (`get_elevation_at_pos`, terrain.rs:579) samples a (W-1)×(H-1) node grid with bilinear blending
// and is only for reading a smooth field value at a position — it is intentionally NOT reproduced
// here. Keep this file dependency-free (no imports) so it is trivially shareable and testable.

/** Axis-aligned horizontal world bounds. Mirrors the x/z components of the backend `MapBounds`
 * (default `min.xz = -100`, `max.xz = +100`). `world_scale` is implicit: there is no scale field in
 * the World Artifact v1 header, so the mapping is defined entirely by these bounds. */
export interface XZBounds {
  minX: number;
  maxX: number;
  minZ: number;
  maxZ: number;
}

/** The backend `MapBounds::default()` projected onto the horizontal plane: a 200×200 world-unit
 * square centred on the origin. */
export const DEFAULT_XZ_BOUNDS: XZBounds = {
  minX: -100,
  maxX: 100,
  minZ: -100,
  maxZ: 100,
};

/**
 * Flat row-major index of cell `(ix, iy)` in a `width`-wide grid.
 * Mirror of `cell_index` (sim_rules.rs).
 */
export function cellIndex(ix: number, iy: number, width: number): number {
  return iy * width + ix;
}

/**
 * Normalized centre `(u, v)` of cell `(ix, iy)` in a `width × height` grid.
 * Mirror of `cell_center_uv` (sim_rules.rs).
 */
export function cellCenterUv(
  ix: number,
  iy: number,
  width: number,
  height: number,
): [number, number] {
  return [(ix + 0.5) / width, (iy + 0.5) / height];
}

/**
 * Bucket a normalized `(u, v)` to its cell `(ix, iy)` using the same `floor(coord · dim)` rule as
 * `TerrainMap::get_map_indices`, clamped to the last cell. Mirror of `uv_to_cell` (sim_rules.rs).
 * The low clamp to 0 reproduces Rust's saturating `as usize` cast for slightly-out-of-range inputs.
 */
export function uvToCell(
  u: number,
  v: number,
  width: number,
  height: number,
): [number, number] {
  const ix = Math.min(Math.max(Math.floor(u * width), 0), width - 1);
  const iy = Math.min(Math.max(Math.floor(v * height), 0), height - 1);
  return [ix, iy];
}

/**
 * World-space `(x, z)` of the centre of cell `(ix, iy)` for a grid covering `bounds`.
 * Mirror of `cell_center_to_world_xz` (sim_rules.rs).
 */
export function cellCenterToWorldXz(
  ix: number,
  iy: number,
  width: number,
  height: number,
  bounds: XZBounds,
): [number, number] {
  const [u, v] = cellCenterUv(ix, iy, width, height);
  return [
    bounds.minX + u * (bounds.maxX - bounds.minX),
    bounds.minZ + v * (bounds.maxZ - bounds.minZ),
  ];
}

/**
 * Bucket a world-space `(x, z)` to its cell, or `null` if the point lies outside `bounds`.
 * This is the canonical inverse of {@link cellCenterToWorldXz} and matches
 * `TerrainMap::get_map_indices` / `world_xz_to_cell` (terrain.rs:563 / sim_rules.rs): the `[0,1]`
 * membership test is inclusive at both ends, so the max corner resolves to the extreme cell.
 */
export function worldXzToCell(
  x: number,
  z: number,
  width: number,
  height: number,
  bounds: XZBounds,
): [number, number] | null {
  const xRange = bounds.maxX - bounds.minX;
  const zRange = bounds.maxZ - bounds.minZ;
  if (xRange <= 0 || zRange <= 0) {
    return null;
  }
  const u = (x - bounds.minX) / xRange;
  const v = (z - bounds.minZ) / zRange;
  if (u < 0 || u > 1 || v < 0 || v > 1) {
    return null;
  }
  return uvToCell(u, v, width, height);
}
