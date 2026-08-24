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
// # Why the camera is placed by a solver and not by an offset
//
// The first derivation placed every camera at `subject - (d, d)`: a fixed south-west offset, which
// for any subject in the north-east aims the lens at the two nearest edges of a finite square of
// terrain. Independent review rejected four of the eight images for exactly that — the hard world
// boundary along the right edge of `collision`, `water`, `biome_transition` and `ecosystem`.
//
// `utils/viewFraming.ts` replaces the offset with a constraint: project the image rectangle onto the
// ground and require every sample to land inside the world. The solver searches azimuths around the
// subject and takes the most inward-facing one that satisfies it, so a subject near an edge is shot
// from *outside* it looking in, with the continent behind. The same predicate is asserted of the
// committed literals by `viewFraming.test.ts`, because a solver that guarantees a property and a
// pasted number that has it are two different claims.
//
// # Why the result is pasted rather than computed at runtime
//
// A canonical view has to be *repeatable* — a before/after pair must differ by the change under
// review and nothing else. A pose recomputed from world state would move whenever the world moved,
// and two images framing different places cannot be compared. So this is a one-off derivation
// whose output is committed as literals, and re-running it is a deliberate act that invalidates
// the previous captures. It must be re-run after any change to `WORLD_GEN_VERSION`, the world
// identity, or the framing rules — the previous pass changed worldgen from 20 to 21 *after*
// deriving, and shipped poses describing a superseded world.
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
import { solveInwardFraming } from '../src/components/Landscape/utils/viewFraming';
import type { FramedPose } from '../src/components/Landscape/utils/viewFraming';
import { buildWalkGraph, labelComponents, nodeAt } from '../src/components/Landscape/utils/mapEvidence';
import {
  SHARED_WORLD_SEED,
  SHARED_WORLD_SHAPE,
  SHARED_WORLD_SIZE,
} from '../src/utils/sharedWorld';

const RENDER_SIZE = FLORA_RADIUS_REFERENCE_EXTENT; // WorldShowcase
const HEIGHT_RATIO = 0.14; // WorldShowcase
const MESH_RES = 384; // WorldShowcase
/** Uniform canonical -> render factor; see `canonicalCameraToRender`. */
const K = RENDER_SIZE / CANONICAL_XZ_EXTENT;
/** Half the world footprint in canonical units — the terrain mesh spans exactly the bounds. */
const FOOTPRINT_HALF = CANONICAL_XZ_EXTENT / 2;
/**
 * Downward angle from camera to subject for every solved view.
 *
 * The scene's vertical FOV is 55°, so the frame's top edge sits `pitch - 27.5°` below horizontal.
 * 42° leaves 14.5°, which is enough that the visible ground stops well short of the horizon while
 * the shot still reads as a three-quarter view rather than a plan.
 */
const PITCH_DEG = 42;

console.log(`generating ${SHARED_WORLD_SEED} ${SHARED_WORLD_SIZE}² ${SHARED_WORLD_SHAPE} ...`);
const world: World = generateWorld(SHARED_WORLD_SEED, {
  size: SHARED_WORLD_SIZE,
  shape: SHARED_WORLD_SHAPE,
});
const N = world.size;

/** Cell -> canonical X. */
const cx = (ix: number): number => (ix / (N - 1) - 0.5) * CANONICAL_XZ_EXTENT;
/** Cell -> canonical Z. */
const cz = (iy: number): number => (iy / (N - 1) - 0.5) * CANONICAL_XZ_EXTENT;
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
const render = (n: number): number => Math.round(n * K * 10) / 10;
const fmtPose = (p: FramedPose): string =>
  `{ position: [${fmt(p.position[0])}, ${fmt(p.position[1])}, ${fmt(p.position[2])}], ` +
  `target: [${fmt(p.target[0])}, ${fmt(p.target[1])}, ${fmt(p.target[2])}] }`;

const out: Record<string, string> = {};
const why: Record<string, string[]> = {};

/** A ranked evidence subject: where it is, why it qualifies, and how good a candidate it was. */
interface Candidate {
  /** Canonical `[x, groundY, z]`. */
  subject: [number, number, number];
  /** Human-readable justification, printed as the pose's comment. */
  reason: string;
}

/**
 * Solve the best-ranked candidate that can actually be photographed.
 *
 * Why ranked candidates rather than one subject: the densest stand of trunks in this world sits at
 * canonical x = -94, six units from the terrain edge. There is no camera position that frames it
 * without the cut plane in shot — to look inward the camera must stand between the subject and the
 * edge, which is off the mesh and over abyssal water, and the edge then crosses the foreground.
 *
 * The response is not to photograph the boundary and label it collision evidence, and it is not to
 * pitch the camera down until the image is a plan view. It is that "the densest stand" was never the
 * requirement — "a dense stand, framed so a reviewer can judge the colliders" is. So each view
 * supplies its candidates in descending order of merit and takes the first one that is photographable,
 * printing what it skipped so the substitution is visible rather than silent.
 */
function solve(
  id: string,
  candidates: Candidate[],
  distance: number,
  aimAbove: number,
  opts: { requireFrameInside?: boolean; exemption?: string; pitchDeg?: number } = {},
): void {
  const requireFrameInside = opts.requireFrameInside ?? true;
  const pitchDeg = opts.pitchDeg ?? PITCH_DEG;
  for (let rank = 0; rank < candidates.length; rank++) {
    const c = candidates[rank];
    const sol = solveInwardFraming({
      subject: c.subject,
      distance,
      pitchDeg,
      aimAbove,
      footprintHalf: FOOTPRINT_HALF,
      requireFrameInside,
    });
    if (!sol) continue;
    out[id] = fmtPose(sol.pose);
    why[id] = [c.reason];
    if (rank > 0) {
      why[id].push(
        `candidate ${rank + 1} of ${candidates.length}: the ${rank} better-ranked subject(s) sit too ` +
          `close to the world edge to photograph without the cut plane in frame`,
      );
    }
    why[id].push(
      `framed inward (inwardness ${sol.inwardness.toFixed(2)}, azimuth ${((sol.azimuth * 180) / Math.PI).toFixed(0)}°), ` +
        `ground reach ${fmt(sol.groundReach)} canonical / ${render(sol.groundReach)} render units`,
    );
    if (opts.exemption) why[id].push(opts.exemption);
    return;
  }
  throw new Error(
    `${id}: none of the ${candidates.length} candidates can be framed at distance ${distance} and ` +
      `pitch ${pitchDeg}° without the world edge in shot. Supply more candidates, move closer, or ` +
      `document an exemption.`,
  );
}

// ---- overview: the whole continent ---------------------------------------------------------
//
// The one view exempt from the inward-framing constraint, and the exemption is the subject. Framing
// a 1200-unit map needs the camera outside it, and the world's boundary *is* what an overview shows:
// a continent surrounded by ocean, which is what this world is. Independent review accepted this
// image; the four it rejected were close-ups where the cut plane appeared behind the subject.
//
// Framing, not taste. The scene camera is 55° vertical FOV at 16:9, so horizontal half-FOV is
// atan(tan(27.5°) * 16/9) = 42.8°. Viewed from 45° elevation, a 1200-unit map projects to 1200
// wide and 1200*sin45 = 849 tall, so the binding constraint is vertical: d >= 424.5/tan(27.5°) =
// 815 render units. The old pose sat at 806 and clipped two edges.
{
  const dRender = 950; // 815 needed + margin
  const dCanon = dRender / K;
  const c = dCanon / Math.SQRT2;
  out.overview = fmtPose({ position: [0, c, c], target: [0, 0, 0] });
  why.overview = [
    `whole map from 45°, ${dRender} render units out (>=815 needed to frame 1200 at 55° FOV)`,
    'exempt from the inward-framing constraint: the subject IS the whole bounded world',
  ];
}

// ---- spawn: where the app actually opens ----------------------------------------------------
{
  const s = findSpawn(world, RENDER_SIZE);
  const ix = Math.round((s.x / RENDER_SIZE + 0.5) * (N - 1));
  const iy = Math.round((s.z / RENDER_SIZE + 0.5) * (N - 1));
  const groundY = cy(at(ix, iy));
  // One candidate only, and deliberately: `spawn` must show where the app opens. If this stopped
  // being photographable the answer would be to fix `findSpawn`, not to photograph somewhere else.
  solve(
    'spawn',
    [
      {
        subject: [s.x / K, groundY, s.z / K],
        reason: `findSpawn = render (${fmt(s.x)}, ${fmt(s.z)}), biome ${Biome[world.biome[at(ix, iy)]]}`,
      },
    ],
    13,
    1,
  );
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
  const ranked = [...counts.entries()]
    // Descending density, then a stable key order so a tie cannot move the pose between runs.
    .sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1))
    .slice(0, 40)
    .map(([k, n]): Candidate => {
      const [bx, bz] = k.split(',').map(Number);
      const rx = (bx + 0.5) * CELL;
      const rz = (bz + 0.5) * CELL;
      const ix = Math.round((rx / RENDER_SIZE + 0.5) * (N - 1));
      const iy = Math.round((rz / RENDER_SIZE + 0.5) * (N - 1));
      return {
        subject: [rx / K, cy(at(ix, iy)), rz / K],
        reason: `${n} solid trunks in one ${CELL}-unit bucket at render (${fmt(rx)}, ${fmt(rz)}), biome ${Biome[world.biome[at(ix, iy)]]}`,
      };
    });
  solve('collision', ranked, 11, 1.5);
}

// ---- lighting: the highest relief, lit from the side ----------------------------------------
{
  const scored: Array<{ i: number; score: number }> = [];
  const step = Math.max(1, Math.floor(N / 400));
  for (let iy = step; iy < N - step; iy += step) {
    for (let ix = step; ix < N - step; ix += step) {
      const i = at(ix, iy);
      if (world.elevation[i] <= world.seaLevel) continue;
      scored.push({ i, score: world.elevation[i] + world.slope[i] * 1.5 });
    }
  }
  scored.sort((a, b) => b.score - a.score || a.i - b.i);
  const ranked = scored.slice(0, 40).map(({ i }): Candidate => ({
    subject: [cx(i % N), cy(i), cz((i / N) | 0)],
    reason: `highest relief: elevation ${world.elevation[i].toFixed(2)} slope ${world.slope[i].toFixed(2)}, biome ${Biome[world.biome[i]]}`,
  }));
  // Horizon in frame by design. A lighting view has to show a lit face against an unlit one at a
  // scale where the sun's direction reads, and at 55° FOV a shot that reaches far enough for that
  // spreads laterally by 0.93× its ground distance — wider than this world — so the frame cannot be
  // contained. What it shows past the terrain is a mountain silhouette against sky, which is not the
  // cut-plane artifact the constraint exists to prevent. Independent review accepted this view.
  solve('lighting', ranked, 52, 6, {
    requireFrameInside: false,
    exemption: 'horizon in frame by design — see the note in scripts/derive_view_cameras.ts',
  });
}

// ---- water: the largest lake ----------------------------------------------------------------
{
  const lakes = [...world.lakeBasins].sort(
    (a, b) => (b.maxX - b.minX) * (b.maxY - b.minY) - (a.maxX - a.minX) * (a.maxY - a.minY),
  );
  if (lakes.length === 0) throw new Error('no lake basins — pick a coastline instead');
  const ranked = lakes.slice(0, 20).map((lk): Candidate => {
    const mx = (lk.minX + lk.maxX) / 2;
    const my = (lk.minY + lk.maxY) / 2;
    const spanCells = Math.max(lk.maxX - lk.minX, lk.maxY - lk.minY);
    return {
      subject: [cx(mx), (lk.level * RENDER_SIZE * HEIGHT_RATIO) / K, cz(my)],
      reason: `lake basin ${spanCells} cells across at cell (${mx | 0}, ${my | 0}), level ${lk.level.toFixed(3)}`,
    };
  });
  // A fixed 34-unit shot rather than one scaled to the basin: the largest basin here is 344 cells
  // (≈34 canonical units) across, and a distance scaled to frame it whole spreads wider than the
  // world. Water reads as water from a shore, not from a plan view of an entire lake.
  solve('water', ranked, 34, 1);
}

// ---- biome_transition: the sharpest land/land boundary --------------------------------------
{
  const WET = new Set<number>([Biome.Ocean, Biome.Lake, Biome.River]);
  const scored: Array<{ i: number; variety: number }> = [];
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
      scored.push({ i, variety: seen.size });
    }
  }
  scored.sort((a, b) => b.variety - a.variety || a.i - b.i);
  const ranked = scored.slice(0, 60).map(({ i, variety }): Candidate => ({
    subject: [cx(i % N), cy(i), cz((i / N) | 0)],
    reason: `${variety} distinct land biomes within ${R} cells, centre biome ${Biome[world.biome[i]]}`,
  }));
  solve('biome_transition', ranked, 30, 0);
}

// ---- navigation: the most open walkable ground ----------------------------------------------
{
  const WET = new Set<number>([Biome.Ocean, Biome.Lake, Biome.River]);
  const scored: Array<{ i: number; score: number; open: number }> = [];
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
      scored.push({ i, open, score: open - Math.hypot(ix / N - 0.5, iy / N - 0.5) * 4 });
    }
  }
  scored.sort((a, b) => b.score - a.score || a.i - b.i);
  // A navigation view has to look at ground a walker can *stand on*, and the neighbourhood score does
  // not establish that: it counts open cells around the centre and never asks whether the centre
  // itself is inside a trunk collider. The first derivation picked such a cell, and
  // `gen_map_evidence.ts` refused to publish a route from it. So the candidates are filtered by the
  // same walk graph the evidence record uses.
  // Walkable is not enough either. The score counts *open* neighbours, and the most open cell in this
  // world is a pocket in a swamp with seven reachable nodes out of fifty thousand walkable ones — a
  // clearing with no way out at this granularity. So candidates must sit in the graph's largest
  // connected component: the walkable world, rather than an island of it.
  const flora = buildFloraColliderIndex(world, RENDER_SIZE);
  const graph = buildWalkGraph(world, RENDER_SIZE, HEIGHT_RATIO, MESH_RES, flora);
  const comps = labelComponents(graph, flora);
  const ranked = scored
    .filter(({ i }) => {
      const node = nodeAt(graph, cx(i % N) * K, cz(((i / N) | 0)) * K);
      return node >= 0 && comps.label[node] === comps.largest;
    })
    .slice(0, 60)
    .map(({ i, open }): Candidate => ({
      subject: [cx(i % N), cy(i), cz((i / N) | 0)],
      reason: `open ground in the largest connected walkable component (${open}/16 open samples), biome ${Biome[world.biome[i]]}; the route overlay starts here`,
    }));
  // Further out and much steeper than the other views, and both are forced by what this view has to
  // show. Its subject is not a place, it is a *route* — a polyline with two ends — so the frame has
  // to contain the whole of one, and `gen_map_evidence.ts` picks the farthest goal whose entire path
  // stays in shot. At the standard 26 units and 42° the answer was a 60-unit stub hugging a
  // shoreline, half of it hidden behind the ridge it crossed and its goal pillar out of frame: a
  // shallow oblique frame covers a long thin trapezoid, and almost none of the reachable ground is
  // inside it.
  //
  // 68 units at 64° covers a broad, near-even patch instead, and a steep view is also the one that
  // cannot hide a route behind terrain — the occlusion that made the first attempt's polyline read as
  // dashes. It stops short of a plan view (`viewFraming` requires the pitch to clear the 27.5°
  // vertical half-FOV, and relief still reads at 64°), which is the other failure mode: a top-down
  // raster of the navmesh would be a picture of the graph, not of the world the walker crosses.
  solve('navigation', ranked, 68, 0, { pitchDeg: 64 });
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
  const ranked = [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1))
    .slice(0, 40)
    .map(([k, n]): Candidate => {
      const [bx, bz] = k.split(',').map(Number);
      const rx = (bx + 0.5) * CELL;
      const rz = (bz + 0.5) * CELL;
      const ix = Math.round((rx / RENDER_SIZE + 0.5) * (N - 1));
      const iy = Math.round((rz / RENDER_SIZE + 0.5) * (N - 1));
      return {
        subject: [rx / K, cy(at(ix, iy)), rz / K],
        reason: `${n} flora instances in one ${CELL}-unit bucket, biome ${Biome[world.biome[at(ix, iy)]]}`,
      };
    });
  solve('ecosystem', ranked, 19, 1);
}

console.log(
  `\n// Derived by scripts/derive_view_cameras.ts against the shipped world identity ` +
    `(worldgen v${world.version}).`,
);
for (const id of ['overview', 'navigation', 'collision', 'lighting', 'spawn', 'water', 'biome_transition', 'ecosystem']) {
  for (const line of why[id]) console.log(`  // ${line}`);
  console.log(`  ${id}: ${out[id]},`);
}
