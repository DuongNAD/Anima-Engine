// Generate `artifacts/map_evidence.json` — the navigation and collision claims the canonical
// `navigation` and `collision` views draw and `mapEvidence.test.ts` re-verifies.
//
// # Why the record exists at all
//
// Independent review rejected the first accepted `navigation.png` and `collision.png` on content:
// the first is a landscape photograph with no route, endpoints or navmesh in it, and the second is a
// canopy from above with no collider behaviour visible. Both gates were being satisfied by pictures
// of the right *place*.
//
// Drawing an overlay from live rendering code would not fix that — it would show what the rendering
// code believes. So the claim is computed here from the rules the shipped rig applies, committed, and
// then injected into the capture page: the overlay draws exactly this polyline and exactly these
// collider circles, so an image and a record that disagree is not a state the pipeline can produce.
//
// Run from the repo root, offline, and re-run after any change to `WORLD_GEN_VERSION`, the world
// identity, or the canonical camera poses:
//   npm run gen:map-evidence

import { writeFileSync, mkdirSync } from 'fs';
import { resolve, dirname } from 'path';
import { generateWorld } from '../src/components/Landscape/utils/worldGen';
import { buildFloraColliderIndex, FLORA_RADIUS_REFERENCE_EXTENT } from '../src/components/Landscape/utils/floraClearance';
import {
  CANONICAL_VIEW_CAMERAS,
  CANONICAL_XZ_EXTENT,
  canonicalCameraToRender,
} from '../src/components/Landscape/utils/mapManifest';
import {
  buildWalkGraph,
  buildCollisionEvidence,
  centreOf,
  floodReachable,
  labelComponents,
  nodeAt,
  shortestRoute,
  verifyRoute,
  MARKER_DRAW_LIFT,
  ROUTE_DRAW_LIFT,
  ROUTE_SAMPLE_STEP,
  WALK_GRAPH_CELL,
  WALK_MAX_GRADIENT,
  roundRecord,
  type MapEvidenceRecord,
} from '../src/components/Landscape/utils/mapEvidence';
import { isPointInFrame } from '../src/components/Landscape/utils/viewFraming';
import {
  SHARED_WORLD_SEED,
  SHARED_WORLD_SHAPE,
  SHARED_WORLD_SIZE,
} from '../src/utils/sharedWorld';

const RENDER_SIZE = FLORA_RADIUS_REFERENCE_EXTENT; // WorldShowcase
const HEIGHT_RATIO = 0.14; // WorldShowcase
const MESH_RES = 384; // WorldShowcase
/** Half-extent of the region the `collision` overlay rings, render units. */
const COLLISION_REGION_HALF = 18;

console.log(`generating ${SHARED_WORLD_SEED} ${SHARED_WORLD_SIZE}² ${SHARED_WORLD_SHAPE} ...`);
const world = generateWorld(SHARED_WORLD_SEED, {
  size: SHARED_WORLD_SIZE,
  shape: SHARED_WORLD_SHAPE,
});

const flora = buildFloraColliderIndex(world, RENDER_SIZE);
const graph = buildWalkGraph(world, RENDER_SIZE, HEIGHT_RATIO, MESH_RES, flora);

// Both views' subjects come from the manifest's own camera targets, so the evidence is computed for
// the ground the image actually frames rather than for a second choice of place.
const navTarget = canonicalCameraToRender(CANONICAL_VIEW_CAMERAS.navigation, RENDER_SIZE).target;
const colTarget = canonicalCameraToRender(CANONICAL_VIEW_CAMERAS.collision, RENDER_SIZE).target;

const start = nodeAt(graph, navTarget[0], navTarget[2]);
if (start < 0 || !graph.walkable[start]) {
  throw new Error(
    `the navigation view's target (render ${navTarget[0].toFixed(1)}, ${navTarget[2].toFixed(1)}) is ` +
      `not walkable ground. Re-derive the poses, or fix the view: a navigation view has to look at ` +
      `somewhere a walker can be.`,
  );
}

const reachable = floodReachable(graph, start, flora);
const comps = labelComponents(graph, flora);
let walkableNodes = 0;
for (let i = 0; i < graph.walkable.length; i++) walkableNodes += graph.walkable[i];
let reachableNodes = 0;
for (let i = 0; i < reachable.length; i++) reachableNodes += reachable[i];
if (comps.label[start] !== comps.largest) {
  throw new Error(
    `the navigation view's target is in component ${comps.label[start]} (${comps.sizes[comps.label[start]]} ` +
      `nodes), not the largest (${comps.sizes[comps.largest]} nodes). Re-derive the poses: a navigation ` +
      `view aimed at an isolated pocket publishes a reachability number about a pocket.`,
  );
}

// The goal has to be somewhere the navigation photograph can show.
//
// It used to be `farthestWithin(graph, start, 200)`: the farthest reachable node inside a radius,
// chosen with no reference to where the camera was pointing. The published route consequently ran off
// the bottom-right corner of `navigation.png` and its goal pillar was never in the image — a reviewer
// could see a path and not where it ended, which is most of what a reachability claim is.
//
// So candidates are filtered through the navigation camera's own frustum first, and the route between
// start and goal is required to be in frame along its whole length. Taking the farthest such goal
// keeps the route as long as the picture can honestly carry.
const navPose = canonicalCameraToRender(CANONICAL_VIEW_CAMERAS.navigation, RENDER_SIZE);
const groundY = (x: number, z: number): number => graph.heightAt(x, z);
/** Is this graph node's drawn marker inside the navigation frame? */
const nodeInFrame = (n: number): boolean => {
  const c = centreOf(graph, n);
  // The height the overlay actually draws the route at, so the test is of the marker rather than of
  // the ground under it: a tube 1.2 above the terrain, pillars 4 above.
  return (
    isPointInFrame(navPose, [c.x, groundY(c.x, c.z) + ROUTE_DRAW_LIFT, c.z]) &&
    isPointInFrame(navPose, [c.x, groundY(c.x, c.z) + MARKER_DRAW_LIFT, c.z])
  );
};

if (!nodeInFrame(start)) {
  throw new Error(
    `the navigation view's target is not inside its own camera frame — the pose and the evidence ` +
      `disagree about what is being photographed. Re-derive the poses.`,
  );
}

const reachableInFrame: number[] = [];
for (let i = 0; i < reachable.length; i++) {
  if (reachable[i] && nodeInFrame(i)) reachableInFrame.push(i);
}
const from = centreOf(graph, start);
reachableInFrame.sort((a, b) => {
  const ca = centreOf(graph, a);
  const cb = centreOf(graph, b);
  return Math.hypot(cb.x - from.x, cb.z - from.z) - Math.hypot(ca.x - from.x, ca.z - from.z);
});

let path: number[] | null = null;
let goal = -1;
for (const candidate of reachableInFrame) {
  if (candidate === start) continue;
  const p = shortestRoute(graph, start, candidate, flora);
  // Every node, not only the endpoints: a shortest path between two in-frame points can still bow
  // out of shot, and half a route is not a route.
  if (!p || p.length < 2 || !p.every(nodeInFrame)) continue;
  path = p;
  goal = candidate;
  break;
}
if (!path || goal < 0) {
  throw new Error(
    `no route from the navigation view target stays inside the navigation frame ` +
      `(${reachableInFrame.length} reachable nodes are in shot). Widen the shot or re-derive the pose: ` +
      `a navigation view has to be able to show a whole route.`,
  );
}
const route: Array<[number, number]> = path.map((n) => {
  const c = centreOf(graph, n);
  return [roundRecord(c.x), roundRecord(c.z)];
});

// The published polyline, re-checked at the walker's own step granularity against the rig's rules.
// This is the claim; the graph only proposed it.
const verdict = verifyRoute(world, RENDER_SIZE, HEIGHT_RATIO, MESH_RES, route, flora);
if (!verdict.ok) {
  throw new Error(
    `the proposed route fails the rig's own rules at ${verdict.failures.length} of ` +
      `${verdict.samples} samples (first: ${verdict.failures[0].reason} at ` +
      `${verdict.failures[0].x.toFixed(1)}, ${verdict.failures[0].z.toFixed(1)}). Publishing it would ` +
      `be a reachability claim the walker cannot honour.`,
  );
}

const collision = buildCollisionEvidence(
  world,
  RENDER_SIZE,
  [roundRecord(colTarget[0]), roundRecord(colTarget[2])],
  COLLISION_REGION_HALF,
  600,
  flora,
);
for (const c of collision.colliders) {
  c.x = roundRecord(c.x);
  c.z = roundRecord(c.z);
  c.radius = roundRecord(c.radius);
}
for (const p of collision.pushOutCases) {
  p.from.x = roundRecord(p.from.x);
  p.from.z = roundRecord(p.from.z);
  p.to.x = roundRecord(p.to.x);
  p.to.z = roundRecord(p.to.z);
  p.trunk.x = roundRecord(p.trunk.x);
  p.trunk.z = roundRecord(p.trunk.z);
  p.trunk.radius = roundRecord(p.trunk.radius);
  p.resolvedDistance = roundRecord(p.resolvedDistance);
}

const record: MapEvidenceRecord & { _generated: Record<string, unknown> } = {
  _generated: {
    by: 'scripts/gen_map_evidence.ts',
    seed: SHARED_WORLD_SEED,
    shape: SHARED_WORLD_SHAPE,
    sourceSize: SHARED_WORLD_SIZE,
    worldGenVersion: world.version,
    renderSize: RENDER_SIZE,
    heightRatio: HEIGHT_RATIO,
    meshResolution: MESH_RES,
    canonicalXzExtent: CANONICAL_XZ_EXTENT,
    note:
      'Regenerate with `npm run gen:map-evidence` after any change to WORLD_GEN_VERSION, the world ' +
      'identity in src/utils/sharedWorld.ts, or CANONICAL_VIEW_CAMERAS. Tracked, not a build output: ' +
      'tests/frontend/mapEvidence.test.ts recomputes it and the capture harness injects it into the ' +
      'page so the overlay draws exactly these claims.',
  },
  navigation: {
    graphCell: WALK_GRAPH_CELL,
    maxGradient: WALK_MAX_GRADIENT,
    walkableNodes,
    reachableNodes,
    componentCount: comps.sizes.length,
    largestComponentNodes: comps.sizes[comps.largest],
    reachableFraction: reachableNodes / walkableNodes,
    routeSampleStep: ROUTE_SAMPLE_STEP,
    routeSamples: verdict.samples,
    routeLengthRenderUnits: roundRecord(verdict.lengthRenderUnits),
    route,
  },
  collision,
};

const out = resolve(process.cwd(), 'artifacts/map_evidence.json');
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, `${JSON.stringify(record, null, 2)}\n`);

console.log(
  `wrote artifacts/map_evidence.json\n` +
    `  navigation: ${walkableNodes} walkable nodes, ${reachableNodes} reachable from the view target ` +
    `(${(record.navigation.reachableFraction * 100).toFixed(1)}% at ${WALK_GRAPH_CELL}-unit granularity)\n` +
    `  route: ${route.length} nodes, ${record.navigation.routeLengthRenderUnits} render units, ` +
    `${verdict.samples} samples verified at ${ROUTE_SAMPLE_STEP}-unit steps, 0 failures\n` +
    `  collision: ${collision.collidersInRegion} trunks within ${COLLISION_REGION_HALF} units of ` +
    `(${collision.regionCentre[0]}, ${collision.regionCentre[1]}), ${collision.collidersDrawn} drawn, ` +
    `${collision.pushOutCases.length} push-out cases`,
);
