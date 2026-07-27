// Machine-verifiable navigation and collision evidence for the canonical map views.
//
// # Why a screenshot was not enough
//
// Independent review looked at the first accepted capture set and rejected two of the images on
// content, not composition:
//
//   navigation.png   "a scenic aerial view but contains no route, reachable endpoints, navmesh, or
//                     other visible navigation evidence"
//   collision.png    "shows a dense canopy from above, not collider behaviour or trunk clearance"
//
// Both are fair. A photograph of walkable ground does not show that it is walkable, and a photograph
// of trees does not show what a walker does when they meet one. Adding an overlay alone would only
// move the problem: a route line drawn by rendering code is a picture of the rendering code's belief.
//
// So the evidence is computed here, published as a record, verified by a test against the rules the
// shipped rig actually applies, and *then* drawn. The image and the record are the same claim — the
// capture harness injects this record into the page and the overlay draws exactly the polyline it
// contains, so an image showing a route that the record does not contain is not representable.
//
// # What each claim is worth
//
// Precision about the strength of each number, because "reachable" is easy to overstate:
//
//   reachableFraction   Graph-granularity, and an upper bound. Cells are 3 render units across and a
//                       cell counts as passable when any of nine probes inside it is clear, so a cell
//                       whose only clear position is a pocket the walker cannot get *into* still
//                       counts. Edges are checked along their whole length, which removes the usual
//                       grid-graph error of routing through a trunk, but the bound stays a bound.
//   route               Verified at the walker's own granularity. Every 0.5-unit sample along the
//                       published polyline is over the mesh, above water, outside every trunk
//                       collider, and within the rig's own maximum climb gradient. This is the
//                       strong claim: *this* line is walkable by the rules the rig enforces.
//   pushOutCases        Exact. Each case runs `resolveFloraOverlap` — the function the rig calls — and
//                       records where it moved the walker to, including the degenerate case of
//                       standing exactly on a trunk's centre.

import type { World } from './worldGen';
import { Biome } from './worldGen';
import { surfaceHeight } from './worldSample';
import {
  buildFloraColliderIndex,
  floraOverlapAt,
  resolveFloraOverlap,
  type FloraColliderIndex,
} from './floraClearance';
import { isOverTerrain } from './cameraBounds';

/**
 * Maximum climb: rise over run a walker can take in one step.
 *
 * The rig's own `MAXG`. A step is rejected when `surfaceY(next) - surfaceY(now) > step * MAXG`, so
 * 0.95 is a ~43.5° slope limit. Restated here would be a second copy; it is exported from here and
 * the rig imports it, so there is one.
 */
export const WALK_MAX_GRADIENT = 0.95;

/**
 * Spacing at which a route is re-verified, in render units.
 *
 * The walk top speed is `renderSize * 0.02` = 24 units/s, so one 60 Hz frame moves 0.4 units. 0.5 is
 * that rounded up: sampling coarser than the rig steps would let a route pass a test the walker fails.
 */
export const ROUTE_SAMPLE_STEP = 0.5;

/** Graph node spacing in render units. See the note on `reachableFraction` above. */
export const WALK_GRAPH_CELL = 3;

// ---- how high the overlay draws, and why the numbers live here ---------------------------
//
// `CaptureEvidenceOverlay` draws the route as a tube lifted clear of the ground and its endpoints as
// pillars standing above that. `gen_map_evidence.ts` has to know both, because the goal it picks is
// the farthest one whose *drawn marker* is inside the navigation frame — testing the ground point
// instead would choose a goal whose pillar is half out of shot, which is the defect one step smaller.
//
// So they are declared once, here, and imported by both. A pair of literals in the overlay and a
// second pair in the generator would be two claims about one picture.

/** Height above the terrain at which the route tube is drawn, render units. */
export const ROUTE_DRAW_LIFT = 1.2;

/** Height above the terrain of the centre of a start/goal pillar, render units. */
export const MARKER_DRAW_LIFT = 4;

/** Height of a start/goal pillar, render units. Its top reaches `MARKER_DRAW_LIFT + HEIGHT / 2`. */
export const MARKER_HEIGHT = 8;

/**
 * Probes per axis inside each graph cell when looking for somewhere to stand.
 *
 * One probe at the cell centre is what the first version did, and it was wrong in a way that showed
 * up immediately: in dense jungle most cell centres are inside a trunk, so a walker who can plainly
 * stroll between the trees came out as a set of isolated single nodes and the route generator refused
 * to publish anything longer than one hop. A cell is passable when *some* position in it is clear,
 * which is a question about the cell, not about its centre — so each cell probes a 3×3 lattice and
 * remembers the clear position it found. Routes are then built from positions a walker can occupy.
 */
const CELL_PROBES = 3;

/**
 * Decimal places the record is published at.
 *
 * Three places is 1 mm on a 1200-unit world — far below anything a reviewer can see and far above the
 * precision a JSON diff should churn on. Exported because a verifier has to round the same way to
 * compare, and a second copy of `1000` would be one refactor from disagreeing.
 */
export const RECORD_PRECISION = 3;

/** Round to the record's published precision. */
export function roundRecord(n: number): number {
  const f = 10 ** RECORD_PRECISION;
  return Math.round(n * f) / f;
}

/** Biomes a walker cannot stand on. Water, and the two frozen ones `findSpawn` also refuses. */
const UNWALKABLE_BIOMES = new Set<number>([
  Biome.Ocean,
  Biome.Lake,
  Biome.River,
  Biome.Glacier,
  Biome.Snow,
]);

/** A coarse walkability graph over the terrain footprint. */
export interface WalkGraph {
  /** Node spacing, render units. */
  cell: number;
  /** Nodes per side. */
  dim: number;
  /** Render-space extent the graph covers: `[-half, +half]` on both axes. */
  half: number;
  /** 1 where a walker may stand. */
  walkable: Uint8Array;
  /** Render-space X of the clear position found in each cell. Meaningless where `walkable` is 0. */
  standX: Float32Array;
  /** Render-space Z of the clear position found in each cell. */
  standZ: Float32Array;
  /** Surface height at that position, render units. */
  height: Float32Array;
  /**
   * Surface height anywhere, not only at a node.
   *
   * Carried on the graph so an edge test can check the ground *between* two nodes at the granularity
   * a walker moves at. Without it a 3-unit edge was accepted on its average gradient and `verifyRoute`
   * then rejected the route for a locally steeper half-metre inside that average — a graph and a
   * verifier disagreeing about the same rule.
   */
  heightAt: (x: number, z: number) => number;
}

/** Node containing render-space `(x, z)`, or `-1` when outside the graph. */
export function nodeAt(graph: WalkGraph, x: number, z: number): number {
  const gx = Math.floor((x + graph.half) / graph.cell);
  const gz = Math.floor((z + graph.half) / graph.cell);
  if (gx < 0 || gz < 0 || gx >= graph.dim || gz >= graph.dim) return -1;
  return gz * graph.dim + gx;
}

/**
 * The position a walker occupies at node `idx` — the clear probe the graph found, not the cell centre.
 */
export function centreOf(graph: WalkGraph, idx: number): { x: number; z: number } {
  return { x: graph.standX[idx], z: graph.standZ[idx] };
}

/**
 * Build the walkability graph.
 *
 * A cell is walkable when some position in it is over the mesh, in a biome a walker may stand on,
 * above sea level, and outside every trunk collider — the same four things the rig and `findSpawn`
 * between them require. The position that satisfied all four is remembered, so a route is a sequence
 * of places a walker can actually be rather than of grid coordinates that approximate them.
 */
export function buildWalkGraph(
  world: World,
  renderSize: number,
  heightRatio: number,
  meshResolution: number,
  flora: FloraColliderIndex = buildFloraColliderIndex(world, renderSize),
): WalkGraph {
  const half = renderSize / 2;
  const dim = Math.floor(renderSize / WALK_GRAPH_CELL);
  const walkable = new Uint8Array(dim * dim);
  const standX = new Float32Array(dim * dim);
  const standZ = new Float32Array(dim * dim);
  const height = new Float32Array(dim * dim);
  const seaY = world.seaLevel * renderSize * heightRatio;
  const size = world.size;
  // Probe offsets inside a cell, as fractions of the cell: 1/6, 1/2, 5/6 for a 3×3 lattice.
  const frac: number[] = [];
  for (let p = 0; p < CELL_PROBES; p++) frac.push((p + 0.5) / CELL_PROBES);

  for (let gz = 0; gz < dim; gz++) {
    const z0 = -half + gz * WALK_GRAPH_CELL;
    for (let gx = 0; gx < dim; gx++) {
      const x0 = -half + gx * WALK_GRAPH_CELL;
      const idx = gz * dim + gx;
      // Default to the cell centre so an unwalkable node still has a defined position.
      standX[idx] = x0 + WALK_GRAPH_CELL * 0.5;
      standZ[idx] = z0 + WALK_GRAPH_CELL * 0.5;
      height[idx] = surfaceHeight(world, standX[idx], standZ[idx], renderSize, heightRatio, meshResolution);

      let found = false;
      for (let pz = 0; pz < CELL_PROBES && !found; pz++) {
        for (let px = 0; px < CELL_PROBES && !found; px++) {
          const x = x0 + frac[px] * WALK_GRAPH_CELL;
          const z = z0 + frac[pz] * WALK_GRAPH_CELL;
          if (!isOverTerrain(renderSize, x, z)) continue;
          const y = surfaceHeight(world, x, z, renderSize, heightRatio, meshResolution);
          if (y <= seaY + 1e-6) continue;
          const cx = Math.min(size - 1, Math.max(0, Math.round((x / renderSize + 0.5) * (size - 1))));
          const cz = Math.min(size - 1, Math.max(0, Math.round((z / renderSize + 0.5) * (size - 1))));
          if (UNWALKABLE_BIOMES.has(world.biome[cz * size + cx])) continue;
          if (floraOverlapAt(flora, x, z, 0, 'collider') !== null) continue;
          walkable[idx] = 1;
          standX[idx] = x;
          standZ[idx] = z;
          height[idx] = y;
          found = true;
        }
      }
    }
  }
  return {
    cell: WALK_GRAPH_CELL,
    dim,
    half,
    walkable,
    standX,
    standZ,
    height,
    heightAt: (x, z) => surfaceHeight(world, x, z, renderSize, heightRatio, meshResolution),
  };
}

/** 8-neighbour offsets, in a fixed order so a BFS tie breaks the same way every run. */
const NEIGHBOURS: ReadonlyArray<readonly [number, number]> = [
  [1, 0],
  [0, 1],
  [-1, 0],
  [0, -1],
  [1, 1],
  [1, -1],
  [-1, 1],
  [-1, -1],
];

/**
 * Can a walker cross from node `a` to node `b`?
 *
 * The whole segment between the two positions is tested, at the granularity the walker moves at, for
 * both the things that stop a step: a trunk in the way and ground too steep to climb. A grid graph
 * that tests only its endpoints says two clear cells with a tree between them are connected, and says
 * a 3-unit edge over a lip is climbable because it averages out. Both produced routes that read as
 * valid and that `verifyRoute` then rejected — the graph and the verifier applying the same rule at
 * two different resolutions.
 */
function connected(graph: WalkGraph, a: number, b: number, flora: FloraColliderIndex): boolean {
  if (!graph.walkable[b]) return false;
  const ax = graph.standX[a];
  const az = graph.standZ[a];
  const bx = graph.standX[b];
  const bz = graph.standZ[b];
  const run = Math.hypot(bx - ax, bz - az);
  if (run < 1e-9) return true;
  const steps = Math.max(1, Math.ceil(run / ROUTE_SAMPLE_STEP));
  const stride = run / steps;
  let prevY = graph.height[a];
  for (let s = 1; s <= steps; s++) {
    const t = s / steps;
    const x = ax + (bx - ax) * t;
    const z = az + (bz - az) * t;
    if (s < steps && floraOverlapAt(flora, x, z, 0, 'collider') !== null) return false;
    const y = s === steps ? graph.height[b] : graph.heightAt(x, z);
    if (Math.abs(y - prevY) > stride * WALK_MAX_GRADIENT) return false;
    prevY = y;
  }
  return true;
}

/** Every node reachable from `start` by repeated legal steps. */
export function floodReachable(
  graph: WalkGraph,
  start: number,
  flora: FloraColliderIndex,
): Uint8Array {
  const seen = new Uint8Array(graph.dim * graph.dim);
  if (start < 0 || !graph.walkable[start]) return seen;
  const queue = [start];
  seen[start] = 1;
  for (let head = 0; head < queue.length; head++) {
    const cur = queue[head];
    const gx = cur % graph.dim;
    const gz = Math.floor(cur / graph.dim);
    for (const [dx, dz] of NEIGHBOURS) {
      const nx = gx + dx;
      const nz = gz + dz;
      if (nx < 0 || nz < 0 || nx >= graph.dim || nz >= graph.dim) continue;
      const next = nz * graph.dim + nx;
      if (seen[next] || !connected(graph, cur, next, flora)) continue;
      seen[next] = 1;
      queue.push(next);
    }
  }
  return seen;
}

/** Connected walkable regions of the graph, largest first. */
export interface ComponentLabels {
  /** Component id per node, or `-1` for unwalkable. Ids are assigned in scan order. */
  label: Int32Array;
  /** Node count per component id. */
  sizes: number[];
  /** Id of the largest component, or `-1` when nothing is walkable. */
  largest: number;
}

/**
 * Label every connected walkable region.
 *
 * Needed because "reachable from the view's subject" is only a useful number once the subject is
 * somewhere worth standing. The first attempt picked the most *open* neighbourhood and found seven
 * reachable nodes out of fifty thousand walkable ones: the subject was a pocket in a swamp, enclosed
 * by trunks at this graph's granularity. Openness counts neighbours; it says nothing about whether
 * they join up. Labelling the components first lets the view be aimed at ground that is actually part
 * of the walkable world.
 */
export function labelComponents(graph: WalkGraph, flora: FloraColliderIndex): ComponentLabels {
  const n = graph.dim * graph.dim;
  const label = new Int32Array(n).fill(-1);
  const sizes: number[] = [];
  const queue = new Int32Array(n);

  for (let seed = 0; seed < n; seed++) {
    if (!graph.walkable[seed] || label[seed] !== -1) continue;
    const id = sizes.length;
    let head = 0;
    let tail = 0;
    queue[tail++] = seed;
    label[seed] = id;
    let size = 0;
    while (head < tail) {
      const cur = queue[head++];
      size++;
      const gx = cur % graph.dim;
      const gz = Math.floor(cur / graph.dim);
      for (const [dx, dz] of NEIGHBOURS) {
        const nx = gx + dx;
        const nz = gz + dz;
        if (nx < 0 || nz < 0 || nx >= graph.dim || nz >= graph.dim) continue;
        const next = nz * graph.dim + nx;
        if (label[next] !== -1 || !connected(graph, cur, next, flora)) continue;
        label[next] = id;
        queue[tail++] = next;
      }
    }
    sizes.push(size);
  }

  let largest = -1;
  for (let id = 0; id < sizes.length; id++) {
    if (largest === -1 || sizes[id] > sizes[largest]) largest = id;
  }
  return { label, sizes, largest };
}

/** Fewest-hop route from `start` to `goal`, or `null` when unreachable. */
export function shortestRoute(
  graph: WalkGraph,
  start: number,
  goal: number,
  flora: FloraColliderIndex,
): number[] | null {
  if (start < 0 || goal < 0 || !graph.walkable[start] || !graph.walkable[goal]) return null;
  const prev = new Int32Array(graph.dim * graph.dim).fill(-1);
  const seen = new Uint8Array(graph.dim * graph.dim);
  const queue = [start];
  seen[start] = 1;
  for (let head = 0; head < queue.length; head++) {
    const cur = queue[head];
    if (cur === goal) break;
    const gx = cur % graph.dim;
    const gz = Math.floor(cur / graph.dim);
    for (const [dx, dz] of NEIGHBOURS) {
      const nx = gx + dx;
      const nz = gz + dz;
      if (nx < 0 || nz < 0 || nx >= graph.dim || nz >= graph.dim) continue;
      const next = nz * graph.dim + nx;
      if (seen[next] || !connected(graph, cur, next, flora)) continue;
      seen[next] = 1;
      prev[next] = cur;
      queue.push(next);
    }
  }
  if (!seen[goal]) return null;
  const path: number[] = [];
  for (let n = goal; n !== -1; n = prev[n]) path.push(n);
  return path.reverse();
}

/**
 * The reachable node furthest from `start` but within `maxRadius` render units of it.
 *
 * A route needs two endpoints and "the far corner of the world" makes a polyline nothing in a single
 * frame can show. This picks a goal that is far enough to be a journey and close enough to be in shot.
 */
export function farthestWithin(
  graph: WalkGraph,
  start: number,
  maxRadius: number,
  flora: FloraColliderIndex,
): number {
  const reach = floodReachable(graph, start, flora);
  const from = centreOf(graph, start);
  let best = start;
  let bestD = -1;
  for (let i = 0; i < reach.length; i++) {
    if (!reach[i]) continue;
    const c = centreOf(graph, i);
    const d = Math.hypot(c.x - from.x, c.z - from.z);
    if (d <= maxRadius && d > bestD) {
      bestD = d;
      best = i;
    }
  }
  return best;
}

/** Why a route sample was rejected. */
export interface RouteFailure {
  x: number;
  z: number;
  reason: 'off-mesh' | 'submerged' | 'inside-trunk' | 'too-steep';
}

/**
 * Re-verify a polyline at the walker's own step granularity.
 *
 * This is what makes the published route a claim about the rig rather than about the graph: it
 * resamples at `ROUTE_SAMPLE_STEP` and applies the rig's four rules to every sample and the climb
 * limit to every consecutive pair.
 */
export function verifyRoute(
  world: World,
  renderSize: number,
  heightRatio: number,
  meshResolution: number,
  points: ReadonlyArray<readonly [number, number]>,
  flora: FloraColliderIndex = buildFloraColliderIndex(world, renderSize),
): { ok: boolean; samples: number; failures: RouteFailure[]; lengthRenderUnits: number } {
  const failures: RouteFailure[] = [];
  const seaY = world.seaLevel * renderSize * heightRatio;
  let samples = 0;
  let length = 0;
  let prevY: number | null = null;

  for (let i = 0; i + 1 < points.length; i++) {
    const [ax, az] = points[i];
    const [bx, bz] = points[i + 1];
    const span = Math.hypot(bx - ax, bz - az);
    length += span;
    const steps = Math.max(1, Math.ceil(span / ROUTE_SAMPLE_STEP));
    for (let s = i === 0 ? 0 : 1; s <= steps; s++) {
      const t = s / steps;
      const x = ax + (bx - ax) * t;
      const z = az + (bz - az) * t;
      samples++;
      const y = surfaceHeight(world, x, z, renderSize, heightRatio, meshResolution);
      if (!isOverTerrain(renderSize, x, z)) failures.push({ x, z, reason: 'off-mesh' });
      else if (y <= seaY + 1e-6) failures.push({ x, z, reason: 'submerged' });
      else if (floraOverlapAt(flora, x, z, 0, 'collider') !== null)
        failures.push({ x, z, reason: 'inside-trunk' });
      else if (prevY !== null && Math.abs(y - prevY) > (span / steps) * WALK_MAX_GRADIENT)
        failures.push({ x, z, reason: 'too-steep' });
      prevY = y;
    }
  }
  return { ok: failures.length === 0, samples, failures, lengthRenderUnits: length };
}

/** A trunk drawn in the collision overlay. */
export interface ColliderCircle {
  x: number;
  z: number;
  radius: number;
}

/** One demonstration of the rig's push-out, recorded exactly as the rig computes it. */
export interface PushOutCase {
  label: string;
  from: { x: number; z: number };
  to: { x: number; z: number };
  trunk: ColliderCircle;
  /** Distance from the resolved position to the trunk centre. Must be >= the trunk radius. */
  resolvedDistance: number;
}

/** The committed navigation + collision record. Serialisable, and the overlay's only input. */
export interface MapEvidenceRecord {
  navigation: {
    graphCell: number;
    maxGradient: number;
    walkableNodes: number;
    /** How many disjoint walkable regions the graph has. */
    componentCount: number;
    /** Nodes in the largest of them — the walkable world. */
    largestComponentNodes: number;
    reachableNodes: number;
    /** Graph-granularity upper bound; see the header note. */
    reachableFraction: number;
    routeSampleStep: number;
    routeSamples: number;
    routeLengthRenderUnits: number;
    /** Render-space polyline, the exact line the overlay draws. */
    route: Array<[number, number]>;
  };
  collision: {
    colliderCount: number;
    /** Centre of the region the `collision` view frames, render space. */
    regionCentre: [number, number];
    regionHalfExtent: number;
    /** How many trunks are in the region. */
    collidersInRegion: number;
    /**
     * How many the overlay draws. Lower than `collidersInRegion` when the cap bites — stated rather
     * than silently truncated, because "680 rings" and "the 600 nearest of 680 rings" are different
     * pictures and only one of them is what the image shows.
     */
    collidersDrawn: number;
    colliderCap: number;
    colliders: ColliderCircle[];
    pushOutCases: PushOutCase[];
  };
}

/**
 * Compute the collision half of the record for the region the `collision` view frames.
 *
 * The push-out cases are chosen to cover the three behaviours worth showing: an ordinary overlap, a
 * walker exactly on a trunk centre (the case the rig's old `d2 > 1e-6` guard never resolved), and a
 * position already clear, which must not move.
 */
export function buildCollisionEvidence(
  world: World,
  renderSize: number,
  regionCentre: [number, number],
  regionHalfExtent: number,
  colliderCap = 600,
  flora: FloraColliderIndex = buildFloraColliderIndex(world, renderSize),
): MapEvidenceRecord['collision'] {
  const inRegion: ColliderCircle[] = [];
  for (let i = 0; i < flora.count; i++) {
    if (
      Math.abs(flora.x[i] - regionCentre[0]) <= regionHalfExtent &&
      Math.abs(flora.z[i] - regionCentre[1]) <= regionHalfExtent
    ) {
      inRegion.push({ x: flora.x[i], z: flora.z[i], radius: flora.radius[i] });
    }
  }
  // Nearest the region centre first, so the cap keeps the trunks closest to what the camera frames;
  // then a stable coordinate sort, because the overlay draws them and a reordering changes the image.
  const d2 = (c: ColliderCircle): number =>
    (c.x - regionCentre[0]) ** 2 + (c.z - regionCentre[1]) ** 2;
  const colliders = inRegion
    .slice()
    .sort((a, b) => d2(a) - d2(b) || a.x - b.x || a.z - b.z)
    .slice(0, colliderCap)
    .sort((a, b) => a.x - b.x || a.z - b.z);

  const cases: PushOutCase[] = [];
  const record = (label: string, rawX: number, rawZ: number): void => {
    // Round the *input* before resolving, not the output afterwards.
    //
    // The record is published at three decimal places, and a reader — the test, or anyone auditing it
    // — only has those. Resolving from a full-precision probe and then rounding produces a `to` that
    // cannot be reproduced from the recorded `from`: the first version of this was off by one in the
    // third place, which looks like a floating-point shrug and is actually the record describing a
    // computation nobody else can run.
    const fx = roundRecord(rawX);
    const fz = roundRecord(rawZ);
    const hit = floraOverlapAt(flora, fx, fz, 0, 'collider');
    const to = resolveFloraOverlap(flora, fx, fz);
    const trunk = hit
      ? { x: hit.x, z: hit.z, radius: hit.radius }
      : { x: fx, z: fz, radius: 0 };
    cases.push({
      label,
      from: { x: fx, z: fz },
      to,
      trunk,
      resolvedDistance: Math.hypot(to.x - trunk.x, to.z - trunk.z),
    });
  };

  const nearest = colliders[0];
  if (nearest) {
    record('exactly on a trunk centre', nearest.x, nearest.z);
    record('inside a trunk, off centre', nearest.x + nearest.radius * 0.4, nearest.z);
  }
  record('already clear — must not move', regionCentre[0] + regionHalfExtent * 4, regionCentre[1]);

  return {
    colliderCount: flora.count,
    regionCentre,
    regionHalfExtent,
    collidersInRegion: inRegion.length,
    collidersDrawn: colliders.length,
    colliderCap,
    colliders,
    pushOutCases: cases,
  };
}
