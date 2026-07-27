import { describe, it, expect } from 'vitest';
import { readFileSync, existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { generateWorld, WORLD_GEN_VERSION } from '@/components/Landscape/utils/worldGen';
import {
  buildFloraColliderIndex,
  floraOverlapAt,
  resolveFloraOverlap,
  FLORA_RADIUS_REFERENCE_EXTENT,
} from '@/components/Landscape/utils/floraClearance';
import {
  buildWalkGraph,
  labelComponents,
  nodeAt,
  verifyRoute,
  MARKER_DRAW_LIFT,
  ROUTE_DRAW_LIFT,
  ROUTE_SAMPLE_STEP,
  WALK_GRAPH_CELL,
  WALK_MAX_GRADIENT,
  type MapEvidenceRecord,
} from '@/components/Landscape/utils/mapEvidence';
import { isPointInFrame } from '@/components/Landscape/utils/viewFraming';
import {
  CANONICAL_VIEW_CAMERAS,
  canonicalCameraToRender,
} from '@/components/Landscape/utils/mapManifest';
import {
  SHARED_WORLD_SEED,
  SHARED_WORLD_SHAPE,
  SHARED_WORLD_SIZE,
} from '@/utils/sharedWorld';

// Evidence gate for the committed navigation and collision record.
//
// # Why this exists
//
// Independent review rejected the first accepted `navigation.png` and `collision.png` on content: a
// landscape photograph does not show that ground is reachable, and a canopy from above does not show
// what a walker does when they meet a trunk. The repair adds an overlay — and an overlay is only worth
// anything if the thing it draws is checkable. So `artifacts/map_evidence.json` is committed, the
// capture harness injects it into the page, and this recomputes it.
//
// # What is checked how
//
// The route is the strong claim and is re-derived from the world here, at the walker's own step
// granularity, against the rig's own rules. The aggregate reachability numbers are recomputed too. The
// push-out cases are re-run through `resolveFloraOverlap`, the function the rig itself calls.
//
// This generates the 2048² world (~5 s) and walks a 400² graph, so it carries its own timeout.

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '../..');
const RECORD_PATH = resolve(ROOT, 'artifacts/map_evidence.json');
const RENDER_SIZE = FLORA_RADIUS_REFERENCE_EXTENT;
const HEIGHT_RATIO = 0.14;
const MESH_RES = 384;
const TIMEOUT = 180_000;

type Record_ = MapEvidenceRecord & { _generated: Record<string, unknown> };

/** The precision `gen_map_evidence.ts` writes at. */
const round3 = (n: number): number => Math.round(n * 1000) / 1000;

function loadRecord(): Record_ {
  expect(existsSync(RECORD_PATH), `artifacts/map_evidence.json must exist at ${RECORD_PATH}`).toBe(true);
  return JSON.parse(readFileSync(RECORD_PATH, 'utf8')) as Record_;
}

// One world for the whole file: generating it per test would be eight times the cost for one answer.
let cached: ReturnType<typeof buildScene> | null = null;
function buildScene() {
  const world = generateWorld(SHARED_WORLD_SEED, {
    size: SHARED_WORLD_SIZE,
    shape: SHARED_WORLD_SHAPE,
  });
  const flora = buildFloraColliderIndex(world, RENDER_SIZE);
  const graph = buildWalkGraph(world, RENDER_SIZE, HEIGHT_RATIO, MESH_RES, flora);
  return { world, flora, graph };
}
function scene(): NonNullable<typeof cached> {
  if (!cached) cached = buildScene();
  return cached;
}

describe('map evidence — the committed record, recomputed', () => {
  it('describes the world the app renders', () => {
    const rec = loadRecord();
    expect(rec._generated.seed).toBe(SHARED_WORLD_SEED);
    expect(rec._generated.shape).toBe(SHARED_WORLD_SHAPE);
    expect(rec._generated.sourceSize).toBe(SHARED_WORLD_SIZE);
    // The trap this catches is the one that caught the camera poses: worldgen moved from 20 to 21 and
    // the committed evidence went on describing 20.
    expect(rec._generated.worldGenVersion).toBe(WORLD_GEN_VERSION);
    expect(rec._generated.renderSize).toBe(RENDER_SIZE);
    expect(rec._generated.heightRatio).toBe(HEIGHT_RATIO);
    expect(rec._generated.meshResolution).toBe(MESH_RES);
  });

  it('states the rules it was computed under', () => {
    const rec = loadRecord();
    expect(rec.navigation.graphCell).toBe(WALK_GRAPH_CELL);
    expect(rec.navigation.maxGradient).toBe(WALK_MAX_GRADIENT);
    expect(rec.navigation.routeSampleStep).toBe(ROUTE_SAMPLE_STEP);
  });

  it('publishes a route that starts at the navigation view target', { timeout: TIMEOUT }, () => {
    const rec = loadRecord();
    const { graph } = scene();
    const target = canonicalCameraToRender(CANONICAL_VIEW_CAMERAS.navigation, RENDER_SIZE).target;
    const node = nodeAt(graph, target[0], target[2]);
    expect(node, 'the navigation target must be inside the graph').toBeGreaterThanOrEqual(0);
    expect(graph.walkable[node], 'the navigation target must be walkable ground').toBe(1);

    // The route's first point is the standable position the graph found in that cell — not the cell
    // centre, which in dense flora is often inside a trunk.
    const [rx, rz] = rec.navigation.route[0];
    expect(rx).toBeCloseTo(graph.standX[node], 3);
    expect(rz).toBeCloseTo(graph.standZ[node], 3);
  });

  it('publishes a route a walker can actually walk', { timeout: TIMEOUT }, () => {
    // The claim, re-derived. Every 0.5-unit sample must be over the mesh, above water, outside every
    // trunk collider, and within the rig's climb limit of its predecessor.
    const rec = loadRecord();
    const { world, flora } = scene();
    const verdict = verifyRoute(
      world,
      RENDER_SIZE,
      HEIGHT_RATIO,
      MESH_RES,
      rec.navigation.route,
      flora,
    );
    expect(verdict.failures.slice(0, 5)).toEqual([]);
    expect(verdict.ok).toBe(true);
    expect(verdict.samples).toBe(rec.navigation.routeSamples);
    expect(verdict.lengthRenderUnits).toBeCloseTo(rec.navigation.routeLengthRenderUnits, 2);
  });

  it('publishes a route long enough to be evidence of travel', () => {
    const rec = loadRecord();
    // A one-hop route is a picture of two adjacent cells. The first attempt produced exactly that,
    // because the graph had disconnected the subject from everything.
    expect(rec.navigation.route.length).toBeGreaterThan(20);
    expect(rec.navigation.routeLengthRenderUnits).toBeGreaterThan(100);
  });

  it('publishes a route the navigation photograph can actually show', { timeout: TIMEOUT }, () => {
    // The defect this is the regression for: the goal was picked as the farthest reachable node
    // inside a 200-unit radius, with no reference to where the camera was pointing. The route ran off
    // the bottom-right corner of `navigation.png` and the goal pillar fell outside the image, so a
    // reviewer could see a path and not where it ended — which is most of what a reachability claim
    // is. Independent visual review caught it; nothing in this file would have.
    //
    // Every node is checked, not only the endpoints: a shortest path between two in-frame points can
    // still bow out of shot, and half a route is not a route.
    const rec = loadRecord();
    const pose = canonicalCameraToRender(CANONICAL_VIEW_CAMERAS.navigation, RENDER_SIZE);
    const { world, flora } = scene();
    const graph = buildWalkGraph(world, RENDER_SIZE, HEIGHT_RATIO, MESH_RES, flora);

    const outOfFrame = rec.navigation.route.filter(([x, z]) => {
      const y = graph.heightAt(x, z);
      return (
        !isPointInFrame(pose, [x, y + ROUTE_DRAW_LIFT, z]) ||
        !isPointInFrame(pose, [x, y + MARKER_DRAW_LIFT, z])
      );
    });
    expect(
      outOfFrame,
      'these route nodes are drawn outside the navigation view — the image would show a route ' +
        'leaving the frame, and the record would be claiming something the photograph does not',
    ).toEqual([]);

    // And the gate has power: a point well outside the frame is rejected.
    const [fx, , fz] = pose.target;
    expect(isPointInFrame(pose, [fx + RENDER_SIZE, graph.heightAt(fx, fz), fz])).toBe(false);
  });

  it('rejects a route that cuts through a trunk — the negative control', { timeout: TIMEOUT }, () => {
    // Without this, `verifyRoute` returning `ok` proves only that it returns `ok`. A straight chord
    // from the route's first point to its last ignores every obstacle the real route went around.
    const rec = loadRecord();
    const { world, flora } = scene();
    const chord = [rec.navigation.route[0], rec.navigation.route[rec.navigation.route.length - 1]];
    const verdict = verifyRoute(world, RENDER_SIZE, HEIGHT_RATIO, MESH_RES, chord, flora);
    expect(verdict.ok, 'a straight line across 200 units of jungle cannot be walkable').toBe(false);
  });

  it('recomputes the same reachability numbers', { timeout: TIMEOUT }, () => {
    const rec = loadRecord();
    const { graph, flora } = scene();

    let walkable = 0;
    for (let i = 0; i < graph.walkable.length; i++) walkable += graph.walkable[i];
    expect(walkable).toBe(rec.navigation.walkableNodes);

    const comps = labelComponents(graph, flora);
    expect(comps.sizes.length).toBe(rec.navigation.componentCount);
    expect(comps.sizes[comps.largest]).toBe(rec.navigation.largestComponentNodes);

    // The navigation view must look at the walkable world, not an island of it. The second derivation
    // attempt aimed at a swamp clearing with seven reachable nodes out of fifty thousand walkable.
    const target = canonicalCameraToRender(CANONICAL_VIEW_CAMERAS.navigation, RENDER_SIZE).target;
    expect(comps.label[nodeAt(graph, target[0], target[2])]).toBe(comps.largest);
    expect(rec.navigation.reachableNodes).toBe(rec.navigation.largestComponentNodes);
  });

  it('does not overstate reachability', () => {
    const rec = loadRecord();
    expect(rec.navigation.reachableFraction).toBeGreaterThan(0);
    expect(rec.navigation.reachableFraction).toBeLessThanOrEqual(1);
    expect(rec.navigation.reachableFraction).toBeCloseTo(
      rec.navigation.reachableNodes / rec.navigation.walkableNodes,
      5,
    );
  });
});

describe('collision evidence — the rig\'s own resolver, recorded', () => {
  it('rings the region the collision view frames', () => {
    const rec = loadRecord();
    const target = canonicalCameraToRender(CANONICAL_VIEW_CAMERAS.collision, RENDER_SIZE).target;
    expect(rec.collision.regionCentre[0]).toBeCloseTo(target[0], 2);
    expect(rec.collision.regionCentre[1]).toBeCloseTo(target[2], 2);
  });

  it('states its cap rather than truncating silently', () => {
    const rec = loadRecord();
    expect(rec.collision.collidersDrawn).toBe(
      Math.min(rec.collision.collidersInRegion, rec.collision.colliderCap),
    );
    expect(rec.collision.colliders.length).toBe(rec.collision.collidersDrawn);
  });

  it('lists the trunks that are really there, at the radii the rig uses', { timeout: TIMEOUT }, () => {
    const rec = loadRecord();
    const { flora } = scene();
    const [cx, cz] = rec.collision.regionCentre;
    const half = rec.collision.regionHalfExtent;

    let inRegion = 0;
    for (let i = 0; i < flora.count; i++) {
      if (Math.abs(flora.x[i] - cx) <= half && Math.abs(flora.z[i] - cz) <= half) inRegion++;
    }
    expect(inRegion).toBe(rec.collision.collidersInRegion);
    expect(flora.count).toBe(rec.collision.colliderCount);

    // Every published circle is a real trunk at its real collider radius. A ring drawn at the canopy
    // radius instead would be a picture of the wrong footprint — and that exact unit confusion (render
    // units vs canonical) already shipped once in the MCP manifest.
    for (const c of rec.collision.colliders) {
      const hit = floraOverlapAt(flora, c.x, c.z, 0, 'collider');
      expect(hit, `no trunk at (${c.x}, ${c.z})`).not.toBeNull();
      expect(hit!.radius).toBeCloseTo(c.radius, 3);
    }
  });

  it('reproduces every push-out case exactly', { timeout: TIMEOUT }, () => {
    const rec = loadRecord();
    const { flora } = scene();
    for (const c of rec.collision.pushOutCases) {
      const resolved = resolveFloraOverlap(flora, c.from.x, c.from.z);
      // Exact, against the record's own precision. `toBeCloseTo(v, 3)` would be the wrong comparison:
      // it allows less error than rounding to three places introduces, so a correctly recorded value
      // can fail it. Rounding the recomputed value the same way the generator did and demanding
      // equality is both stricter and right.
      expect(round3(resolved.x), `${c.label}: x`).toBe(c.to.x);
      expect(round3(resolved.z), `${c.label}: z`).toBe(c.to.z);
    }
  });

  it('covers the three behaviours worth showing', { timeout: TIMEOUT }, () => {
    const rec = loadRecord();
    const { flora } = scene();
    const byLabel = new Map(rec.collision.pushOutCases.map((c) => [c.label, c]));

    // The degenerate case: exactly on a trunk centre. The rig's old inline push-out guarded on
    // `d2 > 1e-6`, so the one position that most needed resolving was the only one never resolved.
    const centre = byLabel.get('exactly on a trunk centre');
    expect(centre).toBeDefined();
    expect(centre!.from.x).toBeCloseTo(centre!.trunk.x, 3);
    expect(centre!.from.z).toBeCloseTo(centre!.trunk.z, 3);
    expect(centre!.resolvedDistance).toBeGreaterThanOrEqual(centre!.trunk.radius - 1e-6);
    expect(floraOverlapAt(flora, centre!.to.x, centre!.to.z, 0, 'collider')).toBeNull();

    const inside = byLabel.get('inside a trunk, off centre');
    expect(inside).toBeDefined();
    expect(floraOverlapAt(flora, inside!.from.x, inside!.from.z, 0, 'collider')).not.toBeNull();
    expect(floraOverlapAt(flora, inside!.to.x, inside!.to.z, 0, 'collider')).toBeNull();

    // And the control: a clear position must not move. A resolver that always nudged would satisfy
    // both cases above and be wrong.
    const clear = byLabel.get('already clear — must not move');
    expect(clear).toBeDefined();
    expect(clear!.to.x).toBeCloseTo(clear!.from.x, 6);
    expect(clear!.to.z).toBeCloseTo(clear!.from.z, 6);
  });
});
