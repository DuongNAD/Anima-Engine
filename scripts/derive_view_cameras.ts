// Derive the eight canonical camera poses from the world the app actually renders.
//
// # Why this exists
//
// The poses were authored before any capture harness existed, so nobody had ever looked through
// them. The first real capture showed what that costs: `spawn` framed open ocean (its target,
// canonical (10, 1, 10), is the middle of the map, and the middle of this map is sea), and
// `overview` cropped the continent at two edges. A camera specification that has never been
// rendered is a guess, and eight guesses were about to be published as canonical evidence.
//
// This derives each pose from the world data so that a view called `collision` points at the
// densest stand of solid trunks, `water` at real water, and `spawn` at the position `findSpawn`
// actually returns. Run it, read the output, and paste the poses into `CANONICAL_VIEW_CAMERAS`.
//
// # Why the result is pasted rather than computed at runtime
//
// A canonical view has to be *repeatable* — a before/after pair must differ by the change under
// review and nothing else. A pose recomputed from world state would move whenever the world moved,
// and two images framing different places cannot be compared. So this is a one-off derivation
// whose output is committed as literals, and re-running it is a deliberate act that invalidates
// the previous captures.
//
//   node scripts/run_ts.mjs scripts/derive_view_cameras.ts

import { generateWorld, Biome } from '../src/components/Landscape/utils/worldGen';
import type { World } from '../src/components/Landscape/utils/worldGen';
import { findSpawn } from '../src/components/Landscape/utils/worldSample';
import {
  buildFloraColliderIndex,
  FLORA_RADIUS_REFERENCE_EXTENT,
} from '../src/components/Landscape/utils/floraClearance';
import { CANONICAL_XZ_EXTENT } from '../src/components/Landscape/utils/mapManifest';
import {
  SHARED_WORLD_SEED,
  SHARED_WORLD_SHAPE,
  SHARED_WORLD_SIZE,
} from '../src/utils/sharedWorld';

const RENDER_SIZE = FLORA_RADIUS_REFERENCE_EXTENT; // WorldShowcase
const HEIGHT_RATIO = 0.14; // WorldShowcase
/** Uniform canonical -> render factor; see `canonicalCameraToRender`. */
const K = RENDER_SIZE / CANONICAL_XZ_EXTENT;

console.log(`generating ${SHARED_WORLD_SEED} ${SHARED_WORLD_SIZE}² ${SHARED_WORLD_SHAPE} ...`);
const world: World = generateWorld(SHARED_WORLD_SEED, {
  size: SHARED_WORLD_SIZE,
  shape: SHARED_WORLD_SHAPE,
});
const N = world.size;

/** Cell -> canonical XZ. */
const cx = (ix: number): number => (ix / (N - 1) - 0.5) * CANONICAL_XZ_EXTENT;
/**
 * Cell -> canonical Y of the terrain surface.
 *
 * Not `elevation * 10`. The scene draws a full-elevation column at `renderSize * heightRatio`
 * (168 units) and camera poses convert by the uniform factor K, so the canonical height that lands
 * a camera on the render terrain is `elevation * 168 / K` — the render's vertical exaggeration
 * expressed in canonical units.
 */
const cy = (i: number): number => (world.elevation[i] * RENDER_SIZE * HEIGHT_RATIO) / K;

const at = (ix: number, iy: number): number => iy * N + ix;
const fmt = (n: number): number => Math.round(n * 10) / 10;
const pose = (
  p: [number, number, number],
  t: [number, number, number],
): string => `{ position: [${fmt(p[0])}, ${fmt(p[1])}, ${fmt(p[2])}], target: [${fmt(t[0])}, ${fmt(t[1])}, ${fmt(t[2])}] }`;

const out: Record<string, string> = {};
const why: Record<string, string> = {};

// ---- overview: the whole continent ---------------------------------------------------------
//
// Framing, not taste. The scene camera is 55° vertical FOV at 16:9, so horizontal half-FOV is
// atan(tan(27.5°) * 16/9) = 42.8°. Viewed from 45° elevation, a 1200-unit map projects to 1200
// wide and 1200*sin45 = 849 tall, so the binding constraint is vertical: d >= 424.5/tan(27.5°) =
// 815 render units. The old pose sat at 806 and clipped two edges.
{
  const dRender = 950; // 815 needed + margin
  const dCanon = dRender / K;
  const c = dCanon / Math.SQRT2;
  out.overview = pose([0, c, c], [0, 0, 0]);
  why.overview = `whole map from 45°, ${dRender} render units out (>=815 needed to frame 1200 at 55° FOV)`;
}

// ---- spawn: where the app actually opens ----------------------------------------------------
{
  const s = findSpawn(world, RENDER_SIZE);
  const sx = s.x / K;
  const sz = s.z / K;
  const ix = Math.round((s.x / RENDER_SIZE + 0.5) * (N - 1));
  const iy = Math.round((s.z / RENDER_SIZE + 0.5) * (N - 1));
  const groundY = cy(at(ix, iy));
  // Behind and above, looking slightly down at the spawn — the shot a player sees on arrival.
  out.spawn = pose([sx - 9, groundY + 6, sz - 9], [sx, groundY + 1, sz]);
  why.spawn = `findSpawn = render (${fmt(s.x)}, ${fmt(s.z)}), biome ${Biome[world.biome[at(ix, iy)]]}`;
}

// ---- collision: the densest stand of solid trunks -------------------------------------------
{
  const flora = buildFloraColliderIndex(world, RENDER_SIZE);
  // Coarse 24-unit histogram over render space, then the fullest bucket.
  const CELL = 24;
  const counts = new Map<string, number>();
  for (let i = 0; i < flora.count; i++) {
    const k = `${Math.floor(flora.x[i] / CELL)},${Math.floor(flora.z[i] / CELL)}`;
    counts.set(k, (counts.get(k) ?? 0) + 1);
  }
  let bestK = '0,0';
  let bestN = -1;
  for (const [k, n] of counts) if (n > bestN) { bestN = n; bestK = k; }
  const [bx, bz] = bestK.split(',').map(Number);
  const rx = (bx + 0.5) * CELL;
  const rz = (bz + 0.5) * CELL;
  const ix = Math.round((rx / RENDER_SIZE + 0.5) * (N - 1));
  const iy = Math.round((rz / RENDER_SIZE + 0.5) * (N - 1));
  const g = cy(at(ix, iy));
  out.collision = pose([rx / K - 7, g + 5, rz / K - 7], [rx / K, g + 1.5, rz / K]);
  why.collision = `densest ${CELL}-unit bucket: ${bestN} solid trunks at render (${fmt(rx)}, ${fmt(rz)})`;
}

// ---- lighting: the highest relief, lit from the side ----------------------------------------
{
  let best = -1;
  let bi = 0;
  const step = Math.max(1, Math.floor(N / 400));
  for (let iy = step; iy < N - step; iy += step) {
    for (let ix = step; ix < N - step; ix += step) {
      const i = at(ix, iy);
      if (world.elevation[i] <= world.seaLevel) continue;
      const score = world.elevation[i] + world.slope[i] * 1.5;
      if (score > best) { best = score; bi = i; }
    }
  }
  const ix = bi % N;
  const iy = (bi / N) | 0;
  const g = cy(bi);
  out.lighting = pose([cx(ix) - 34, g + 22, cz(iy) + 34], [cx(ix), g * 0.6, cz(iy)]);
  why.lighting = `highest relief: elevation ${world.elevation[bi].toFixed(2)} slope ${world.slope[bi].toFixed(2)}, biome ${Biome[world.biome[bi]]}`;
}

function cz(iy: number): number {
  return (iy / (N - 1) - 0.5) * CANONICAL_XZ_EXTENT;
}

// ---- water: the largest lake ----------------------------------------------------------------
{
  const lakes = [...world.lakeBasins].sort(
    (a, b) => (b.maxX - b.minX) * (b.maxY - b.minY) - (a.maxX - a.minX) * (a.maxY - a.minY),
  );
  const lk = lakes[0];
  if (!lk) throw new Error('no lake basins — pick a coastline instead');
  const mx = (lk.minX + lk.maxX) / 2;
  const my = (lk.minY + lk.maxY) / 2;
  const spanCells = Math.max(lk.maxX - lk.minX, lk.maxY - lk.minY);
  const spanCanon = (spanCells / N) * CANONICAL_XZ_EXTENT;
  const d = Math.max(18, spanCanon * 1.6);
  const g = (lk.level * RENDER_SIZE * HEIGHT_RATIO) / K;
  out.water = pose([cx(mx) - d, g + d * 0.6, cz(my) - d], [cx(mx), g, cz(my)]);
  why.water = `largest lake basin: ${spanCells} cells across at cell (${mx | 0}, ${my | 0}), level ${lk.level.toFixed(3)}`;
}

// ---- biome_transition: the sharpest land/land boundary --------------------------------------
{
  const WET = new Set<number>([Biome.Ocean, Biome.Lake, Biome.River]);
  let bestI = 0;
  let bestVariety = -1;
  const R = Math.max(2, Math.floor(N / 200));
  const step = Math.max(1, Math.floor(N / 300));
  for (let iy = R; iy < N - R; iy += step) {
    for (let ix = R; ix < N - R; ix += step) {
      const i = at(ix, iy);
      if (world.elevation[i] <= world.seaLevel || WET.has(world.biome[i])) continue;
      const seen = new Set<number>();
      for (let dy = -R; dy <= R; dy += R) {
        for (let dx = -R; dx <= R; dx += R) {
          const j = at(ix + dx, iy + dy);
          if (!WET.has(world.biome[j])) seen.add(world.biome[j]);
        }
      }
      if (seen.size > bestVariety) { bestVariety = seen.size; bestI = i; }
    }
  }
  const ix = bestI % N;
  const iy = (bestI / N) | 0;
  const g = cy(bestI);
  out.biome_transition = pose([cx(ix) - 20, g + 14, cz(iy) - 20], [cx(ix), g, cz(iy)]);
  why.biome_transition = `${bestVariety} distinct land biomes within ${R} cells, centre biome ${Biome[world.biome[bestI]]}`;
}

// ---- navigation: the most open walkable ground ----------------------------------------------
{
  const WET = new Set<number>([Biome.Ocean, Biome.Lake, Biome.River]);
  let bestI = 0;
  let bestOpen = -1;
  const R = Math.max(3, Math.floor(N / 150));
  const step = Math.max(1, Math.floor(N / 250));
  for (let iy = R; iy < N - R; iy += step) {
    for (let ix = R; ix < N - R; ix += step) {
      const i = at(ix, iy);
      if (world.elevation[i] <= world.seaLevel || WET.has(world.biome[i])) continue;
      let open = 0;
      for (let dy = -R; dy <= R; dy += R) {
        for (let dx = -R; dx <= R; dx += R) {
          const j = at(ix + dx, iy + dy);
          if (world.elevation[j] > world.seaLevel && !WET.has(world.biome[j]) && world.slope[j] < 0.35) open++;
        }
      }
      const score = open - Math.hypot(ix / N - 0.5, iy / N - 0.5) * 4;
      if (score > bestOpen) { bestOpen = score; bestI = i; }
    }
  }
  const ix = bestI % N;
  const iy = (bestI / N) | 0;
  const g = cy(bestI);
  out.navigation = pose([cx(ix) - 26, g + 20, cz(iy) - 26], [cx(ix), g, cz(iy)]);
  why.navigation = `most open walkable neighbourhood, biome ${Biome[world.biome[bestI]]}`;
}

// ---- ecosystem: densest flora of ANY kind, i.e. the most alive-looking ground ----------------
{
  const CELL = 20;
  const toWorld = RENDER_SIZE / N;
  const counts = new Map<string, number>();
  for (let i = 0; i < world.floraCount; i++) {
    const k = `${Math.floor((world.floraX[i] * toWorld) / CELL)},${Math.floor((world.floraZ[i] * toWorld) / CELL)}`;
    counts.set(k, (counts.get(k) ?? 0) + 1);
  }
  let bestK = '0,0';
  let bestN = -1;
  for (const [k, n] of counts) if (n > bestN) { bestN = n; bestK = k; }
  const [bx, bz] = bestK.split(',').map(Number);
  const rx = (bx + 0.5) * CELL;
  const rz = (bz + 0.5) * CELL;
  const ix = Math.round((rx / RENDER_SIZE + 0.5) * (N - 1));
  const iy = Math.round((rz / RENDER_SIZE + 0.5) * (N - 1));
  const g = cy(at(ix, iy));
  out.ecosystem = pose([rx / K - 13, g + 9, rz / K - 13], [rx / K, g + 1, rz / K]);
  why.ecosystem = `densest ${CELL}-unit flora bucket: ${bestN} instances, biome ${Biome[world.biome[at(ix, iy)]]}`;
}

console.log('\n// Derived by scripts/derive_view_cameras.ts against the shipped world identity.');
for (const id of ['overview', 'navigation', 'collision', 'lighting', 'spawn', 'water', 'biome_transition', 'ecosystem']) {
  console.log(`  // ${why[id]}`);
  console.log(`  ${id}: ${out[id]},`);
}
