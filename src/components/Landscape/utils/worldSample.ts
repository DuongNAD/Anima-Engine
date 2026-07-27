import type { World } from './worldGen';
import type { FloraSource } from './floraClearance';
import { buildFloraColliderIndex, floraOverlapAt, resolveFloraOverlap } from './floraClearance';

/**
 * The fields `biomeAt` reads.
 *
 * Declared narrow for the reason `FloraSource` is: `World` satisfies it structurally, and a caller
 * that only wants a biome lookup — a test with a hand-drawn 2x2 grid, most of all — can hand in the
 * two fields instead of manufacturing twenty typed arrays it will never read.
 *
 * `biome` is optional because the function guards for it. An older or partially-built world really
 * can arrive without one, which is why the guard is there, and an optional field is how a caller
 * gets to exercise that branch rather than assert its way around it.
 */
export interface BiomeSource {
  size: number;
  biome?: Uint8Array;
}

/**
 * The fields `findSpawn` reads: a biome grid, the terrain it scores, and the flora it must clear.
 *
 * Optionality mirrors the guards in the function body exactly — `biome`/`elevation` short-circuit to
 * the origin, `slope`/`shore` score as zero. Nothing here is optional that the code trusts.
 */
export interface SpawnSource extends FloraSource, BiomeSource {
  elevation?: Float32Array;
  slope?: Float32Array;
  shore?: Float32Array;
  seaLevel: number;
}

// ---------------------------------------------------------------------------------------
// Shared sampling helpers for the SoA world. Keeping these in one place guarantees the
// terrain mesh, the vegetation placement and the water all agree on where the ground is.
// ---------------------------------------------------------------------------------------

/** Bilinear sample of any per-cell field at normalized (u, v) in [0, 1]. */
export function sampleField(arr: Float32Array | undefined, size: number, u: number, v: number): number {
  // Guard: a missing or wrong-sized field (e.g. an older/partial world) samples as 0 instead
  // of throwing on an out-of-range / undefined index.
  if (!arr || arr.length < size * size) return 0;
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

// Biome enum values used by the spawn heuristic (kept local so worldSample stays dependency-light).
const B_OCEAN = 0;
const B_BEACH = 1;
const B_SAVANNA = 3;
const B_GRASSLAND = 4;
const B_SHRUBLAND = 5;
const B_FOREST = 6;
const B_JUNGLE = 7;
const B_TAIGA = 8;
const B_RIVER = 13;
const B_LAKE = 14;
const B_STEPPE = 17;
const B_ALPINE = 18;
const B_SNOW = 12;
const B_GLACIER = 20;

/** How inviting each biome is as a starting spot (0 = avoid, 1 = ideal). */
function spawnBiomeScore(b: number): number {
  switch (b) {
    case B_GRASSLAND:
    case B_FOREST:
      return 1.0;
    case B_SAVANNA:
    case B_JUNGLE:
      return 0.9;
    case B_SHRUBLAND:
      return 0.85;
    case B_STEPPE:
    case B_TAIGA:
      return 0.75;
    case B_ALPINE:
      return 0.6;
    case B_BEACH:
      return 0.5;
    default:
      return 0.3;
  }
}

/**
 * Pick a pleasant on-land starting position (world XZ) so the world opens on interesting
 * terrain instead of the origin (which is often open ocean). Scans a coarse sample of cells
 * and scores each by biome appeal, flatness, a little water nearby for scenery, and closeness
 * to the map centre. Deterministic. Falls back to the origin if the world is all water.
 *
 * **Flora clearance is part of "pleasant".** A cell can be flat, grassy, near the shore and still
 * open the view into a canopy — and at the shipped world identity it did: this returned render
 * (-128.7, -93.5), where a broadleaf of canopy radius 1.34 stands 1.90 units away and therefore
 * subtends ninety degrees of the frame. A candidate is only eligible if it clears the `'spawn'`
 * footprint of every solid flora, which is the shared policy in `floraClearance.ts` — the canopy
 * widened until it stops being the whole picture.
 *
 * If no scored cell is clear at all — a fully forested world — the best cell is taken and stepped
 * out of the canopy deterministically rather than returned as-is.
 */
export function findSpawn(world: SpawnSource, renderSize: number): { x: number; z: number } {
  const { size, biome, elevation, slope, shore, seaLevel } = world;
  if (!biome || biome.length < size * size || !elevation) return { x: 0, z: 0 };

  const flora = buildFloraColliderIndex(world, renderSize);
  const toRenderX = (gx: number): number => (gx / (size - 1) - 0.5) * renderSize;

  const step = Math.max(1, Math.floor(size / 160));
  // Two winners are tracked: the best cell that is genuinely clear (what we want to return) and
  // the best cell overall (what we fall back to and push out of the trees).
  let bestClear = -Infinity;
  let clearX: number | null = null;
  let clearZ = 0;
  let best = -Infinity;
  let bx = Math.floor(size / 2);
  let bz = Math.floor(size / 2);
  for (let gy = 0; gy < size; gy += step) {
    for (let gx = 0; gx < size; gx += step) {
      const idx = gy * size + gx;
      const b = biome[idx];
      const e = elevation[idx];
      // Must be dry land (above sea) and not water/ice/high-snow.
      if (
        e <= seaLevel + 0.005 ||
        b === B_OCEAN ||
        b === B_LAKE ||
        b === B_RIVER ||
        b === B_GLACIER ||
        b === B_SNOW
      )
        continue;
      const nu = gx / (size - 1) - 0.5;
      const nv = gy / (size - 1) - 0.5;
      const distC = Math.hypot(nu, nv); // 0 centre .. ~0.71 corner
      const sl = slope ? slope[idx] : 0;
      const sh = shore ? shore[idx] : 0;
      const score =
        spawnBiomeScore(b) -
        sl * 1.6 + // prefer walkable, flat-ish ground
        sh * 0.7 + // a shoreline/water view is scenic
        Math.min(0.3, (e - seaLevel) * 2) - // a little relief is nice
        distC * 0.7; // stay near the middle of the map
      if (score > best) {
        best = score;
        bx = gx;
        bz = gy;
      }
      if (score > bestClear) {
        const rx = toRenderX(gx);
        const rz = toRenderX(gy);
        if (floraOverlapAt(flora, rx, rz, 0, 'spawn') === null) {
          bestClear = score;
          clearX = rx;
          clearZ = rz;
        }
      }
    }
  }

  if (clearX !== null) return { x: clearX, z: clearZ };
  // Nothing scored was clear. Take the best cell and step out of the canopy rather than handing
  // back a position known to be inside one.
  return resolveFloraOverlap(flora, toRenderX(bx), toRenderX(bz), 0, 'spawn');
}

/**
 * Height of the surface a walker stands on at render-space `(x, z)`.
 *
 * Terrain, or still water where water is higher: a walker wades at the shore and stands on a lake's
 * surface rather than its drowned bed. Off the mesh there is no terrain data, so the answer is sea
 * level — which is exactly why walk mode is confined to the mesh footprint (`cameraBounds.ts`): off
 * it, this function keeps returning a plausible height for ground that is not drawn.
 *
 * Extracted from `WorldCameraRig`'s closure because the navigation-evidence graph has to ask the same
 * question. Two implementations of "what is the ground here" would let a published reachability
 * record describe terrain the rig does not agree exists.
 */
export function surfaceHeight(
  world: World,
  x: number,
  z: number,
  renderSize: number,
  heightRatio: number,
  meshResolution: number,
): number {
  const heightUnits = renderSize * heightRatio;
  const seaY = world.seaLevel * heightUnits;
  const u = x / renderSize + 0.5;
  const v = z / renderSize + 0.5;
  if (u < 0 || u > 1 || v < 0 || v > 1) return seaY;
  const groundY = sampleMeshHeight(world, u, v, meshResolution) * heightUnits;
  const size = world.size;
  const cx = Math.min(size - 1, Math.max(0, Math.round(u * (size - 1))));
  const cz = Math.min(size - 1, Math.max(0, Math.round(v * (size - 1))));
  const lake = world.water ? world.water[cz * size + cx] : 0;
  return Math.max(groundY, seaY, lake > 0 ? lake * heightUnits : 0);
}

/**
 * Biome index at a world-space (x, z) position, where the terrain spans
 * [-renderSize/2, renderSize/2] on both axes. Returns Ocean (0) when off-map or when the
 * world has no biome field. Nearest-cell lookup (biomes are categorical — no interpolation).
 */
export function biomeAt(world: BiomeSource, x: number, z: number, renderSize: number): number {
  if (!world.biome || world.biome.length < world.size * world.size) return 0;
  const u = x / renderSize + 0.5;
  const v = z / renderSize + 0.5;
  if (u < 0 || u > 1 || v < 0 || v > 1) return 0;
  const size = world.size;
  const cx = Math.min(size - 1, Math.max(0, Math.round(u * (size - 1))));
  const cz = Math.min(size - 1, Math.max(0, Math.round(v * (size - 1))));
  return world.biome[cz * size + cx];
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
