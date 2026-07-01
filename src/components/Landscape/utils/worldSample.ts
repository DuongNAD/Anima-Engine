import type { World } from './worldGen';

// ---------------------------------------------------------------------------------------
// Shared sampling helpers for the SoA world. Keeping these in one place guarantees the
// terrain mesh, the vegetation placement and the water all agree on where the ground is.
// ---------------------------------------------------------------------------------------

/** Bilinear sample of any per-cell field at normalized (u, v) in [0, 1]. */
export function sampleField(arr: Float32Array, size: number, u: number, v: number): number {
  const fx = Math.min(size - 1, Math.max(0, u * (size - 1)));
  const fy = Math.min(size - 1, Math.max(0, v * (size - 1)));
  const x0 = Math.floor(fx);
  const y0 = Math.floor(fy);
  const x1 = Math.min(size - 1, x0 + 1);
  const y1 = Math.min(size - 1, y0 + 1);
  const tx = fx - x0;
  const ty = fy - y0;
  const a = arr[y0 * size + x0];
  const b = arr[y0 * size + x1];
  const c = arr[y1 * size + x0];
  const d = arr[y1 * size + x1];
  return a * (1 - tx) * (1 - ty) + b * tx * (1 - ty) + c * (1 - tx) * ty + d * tx * ty;
}

/** Bilinear elevation (full data resolution) at (u, v). */
export function sampleElevation(world: World, u: number, v: number): number {
  return sampleField(world.elevation, world.size, u, v);
}

/**
 * Height of the RENDER MESH surface at (u, v), normalized. The terrain mesh is built at a
 * coarser `res` than the data, so its visible surface is the bilinear blend of the mesh-grid
 * vertex heights — NOT the full-resolution elevation. Anything that must sit ON the terrain
 * (trees, rocks) has to sample this, otherwise it floats above / sinks into rough ground.
 */
export function sampleMeshHeight(world: World, u: number, v: number, res: number): number {
  const fx = Math.min(1, Math.max(0, u)) * res;
  const fy = Math.min(1, Math.max(0, v)) * res;
  const gx0 = Math.floor(fx);
  const gy0 = Math.floor(fy);
  const gx1 = Math.min(res, gx0 + 1);
  const gy1 = Math.min(res, gy0 + 1);
  const tx = fx - gx0;
  const ty = fy - gy0;
  const e00 = sampleElevation(world, gx0 / res, gy0 / res);
  const e10 = sampleElevation(world, gx1 / res, gy0 / res);
  const e01 = sampleElevation(world, gx0 / res, gy1 / res);
  const e11 = sampleElevation(world, gx1 / res, gy1 / res);
  return e00 * (1 - tx) * (1 - ty) + e10 * tx * (1 - ty) + e01 * (1 - tx) * ty + e11 * tx * ty;
}
