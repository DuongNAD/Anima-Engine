// ---------------------------------------------------------------------------------------
// One definition of "a tree is in the way", for every consumer that needs it.
//
// # Why this module exists
//
// Walk mode spawned the camera inside a trunk. Not because any single rule was wrong, but
// because there were three of them, written independently, and only one of them ran at spawn
// time:
//
//   * `WorldCameraRig.tsx` owned an inline `TALL` set and an inline `r = 0.45 + floraScale*0.25`,
//     and applied them per frame while walking;
//   * `scripts/gen_map_manifest.ts` restated both — once in a header comment, once in code — to
//     tell the map-review MCP which entities are solid;
//   * `findSpawn` in `worldSample.ts` scored biome, slope, shoreline and distance-to-centre, and
//     did not know flora existed. So it could, and did, return a cell with a pine growing out of
//     it, and the runtime push-out then declined to fix it (see below).
//
// A fourth consumer — the canonical view capture harness — was about to restate them a fourth
// time. Three copies of a rule is a coincidence; four is a policy that belongs in one place.
//
// # The zero-distance hole
//
// The runtime push-out read `if (d2 < r*r && d2 > 1e-6)`. The `d2 > 1e-6` guard is there because
// the next line divides by `sqrt(d2)`, and dividing by ~0 throws the player to infinity. But the
// guard does not *avoid* the singular case, it *skips* it: a player at exactly the trunk centre —
// which is precisely what a spawn picked from a flora cell produces — is the most-overlapped
// position possible, and it was the one position never resolved.
//
// `resolveFloraOverlap` handles it by choosing a fixed direction (+X) when the offset is
// degenerate. Any constant direction is correct; naming one and testing it keeps deterministic
// captures reproducible.
// ---------------------------------------------------------------------------------------

import { FloraType } from './worldGen';

/**
 * The flora fields this policy reads. `World` satisfies it structurally; declaring the narrow
 * shape lets tests hand in a handful of hand-placed trunks instead of a whole generated world.
 */
export interface FloraSource {
  /** Data-grid resolution the flora coordinates are expressed in. */
  size: number;
  floraCount: number;
  floraX: Float32Array;
  floraZ: Float32Array;
  floraScale: Float32Array;
  floraType: Uint8Array;
}

/**
 * Flora that gets a solid trunk collider in walk mode.
 *
 * Ground cover (bush/reed/tuft/rock) and aquatic flora (coral/kelp/seagrass) are walked through:
 * they have no trunk, and colliding with them would make a grassland impassable.
 */
export const TALL_FLORA_TYPES: readonly FloraType[] = Object.freeze([
  FloraType.Pine,
  FloraType.Round,
  FloraType.Jungle,
  FloraType.Cactus,
  FloraType.Acacia,
  FloraType.Palm,
  FloraType.DeadTree,
]);

const TALL = new Set<number>(TALL_FLORA_TYPES);

/** Does this flora type have a solid trunk? */
export function isTallFlora(type: number): boolean {
  return TALL.has(type);
}

/** Trunk collider radius at `floraScale == 0`. */
export const FLORA_COLLIDER_BASE_RADIUS = 0.45;
/** How much of the instance's visual scale becomes collider radius. */
export const FLORA_COLLIDER_SCALE_COEFF = 0.25;

/**
 * World-space radius of a trunk collider. Verbatim the rule `WorldCameraRig` walked against
 * before this module existed, so moving the callers here changes no behaviour.
 */
export function floraColliderRadius(floraScale: number): number {
  return FLORA_COLLIDER_BASE_RADIUS + floraScale * FLORA_COLLIDER_SCALE_COEFF;
}

// ---- the collider is not the tree -------------------------------------------------------
//
// The trunk collider is deliberately narrow: it is what stops a walker passing *through* a tree,
// and widening it would make a forest feel like a maze of invisible pillars. It is not, and was
// never meant to be, the space the tree occupies.
//
// That distinction is the whole finding. At the shipped world identity (2048², seed "seed") the
// old `findSpawn` returned render (-128.7, -93.5) — the position reproduced in the browser — and
// that point is **outside every trunk collider**. It is nonetheless inside a canopy: walking eye
// height is 2.1 world units (`WorldCameraRig` EYE), an acacia's umbrella sits at y≈1.51..1.99 at
// unit flora scale and reaches 0.95 × scale × 1.4 horizontally, and a palm's fronds sit higher
// still. So the camera opened *within the foliage*, with the centre of frame occluded, while
// every collider check said "clear".
//
// Spawn and canonical-capture poses therefore test the CANOPY footprint. The runtime walk keeps
// testing the collider: a walker brushing leaves is correct, a walker standing in a trunk is not.

/** Base size (world units) of a flora instance before its per-instance scale — `WorldVegetation`. */
export const VEGETATION_BASE_SIZE = 1.4;

/**
 * The scene extent every radius in this module is expressed in.
 *
 * A flora instance is `floraScale * VEGETATION_BASE_SIZE` scene units across, and the landscape
 * scene spans `WorldShowcase`'s `RENDER_SIZE`. So a tree is not "0.7 units" in the abstract — it
 * is 0.7 units *of a 1200-unit map*, i.e. a fixed fraction of the world.
 *
 * Anything working in a different span of the same world has to convert, or it describes trees of
 * the wrong size. `gen_map_manifest.ts` publishes positions in the canonical [-100, 100] bounds
 * (200 units for the same map) and was emitting `collider.radius` straight out of the render-space
 * formula — declaring every trunk six times too fat to the map-review gates.
 */
export const FLORA_RADIUS_REFERENCE_EXTENT = 1200;

/** Re-express a radius from {@link FLORA_RADIUS_REFERENCE_EXTENT} into a span of `extent`. */
export function convertFloraRadius(radius: number, extent: number): number {
  return (radius * extent) / FLORA_RADIUS_REFERENCE_EXTENT;
}

/**
 * Largest horizontal reach of each solid flora type at unit instance scale.
 *
 * **Measured, not estimated.** Every value is the max `hypot(x, z)` over the vertices of the
 * geometry `floraGeometry.ts` builds, and `npm run check:flora-footprint` rebuilds that geometry
 * with real three and fails if any entry drifts. This module cannot import three itself — it has
 * to stay usable from `worldSample.ts` and from offline Node scripts — so the number is declared
 * here and proven there.
 */
export const FLORA_FOOTPRINT_UNIT_RADIUS: Readonly<Record<number, number>> = Object.freeze({
  [FloraType.Pine]: 0.55,
  [FloraType.Round]: 0.7628,
  [FloraType.Jungle]: 0.7595,
  [FloraType.Cactus]: 0.5952,
  [FloraType.Acacia]: 0.95,
  [FloraType.Palm]: 0.8062,
  [FloraType.DeadTree]: 0.424,
});

/** How exactly a declared footprint radius must match the measured geometry. */
export const FLORA_FOOTPRINT_TOLERANCE = 1e-3;

/** World-space radius of the space a flora instance visually occupies. */
export function floraCanopyRadius(type: number, floraScale: number): number {
  const unit = FLORA_FOOTPRINT_UNIT_RADIUS[type] ?? 0;
  return unit * floraScale * VEGETATION_BASE_SIZE;
}

/**
 * Which footprint a clearance question is about.
 *
 * `'collider'` — the narrow trunk the walk rig pushes out of (runtime physics).
 * `'canopy'` — the space the instance visually occupies.
 * `'spawn'` — the canopy, widened until it stops filling the frame; see below.
 */
export type FloraFootprint = 'collider' | 'canopy' | 'spawn';

// ---- what "occluded" actually means ------------------------------------------------------
//
// The reported spawn is outside every collider *and* outside every canopy, so neither footprint
// on its own explains the screenshot. Measuring the neighbourhood explains it:
//
//   nearest solid flora to render (-128.7, -93.5) on the shipped world:
//     Round (broadleaf), scale 1.255 → canopy radius 1.340, at distance 1.896
//
// A sphere of radius R seen from distance D subtends a half-angle of asin(R/D). Here R/D = 0.707,
// so the canopy subtends **90 degrees** — it is not merely nearby, it is the entire centre of a
// walker's view. The camera was 0.56 units from the leaf surface at eye height 2.1.
//
// So the spawn rule is not "outside the canopy" — that was already true and it was not enough.
// It is "far enough that a canopy is a tree in the scene rather than the scene". Requiring
// D >= 2R caps the subtended angle at 60 degrees, which leaves the middle of the frame readable
// at any sane field of view. The absolute margin still applies underneath it so a small shrub
// cannot be legal at arm's length just because it is small.

/**
 * A spawn pose must stand at least this multiple of a canopy's radius from its centre, capping
 * the angle that canopy subtends at 2·asin(1/2) = 60°.
 */
export const SPAWN_CANOPY_DISTANCE_RATIO = 2;

/**
 * Absolute breathing room required beyond a canopy edge, whatever its size. Half a metre of air
 * is the difference between "a plant is next to me" and "a plant is on my face".
 */
export const SPAWN_CLEARANCE_MARGIN = 0.5;

/** Distance a spawn or capture pose must keep from the centre of a canopy of radius `canopy`. */
export function spawnClearanceRadius(canopyRadius: number): number {
  return Math.max(canopyRadius * SPAWN_CANOPY_DISTANCE_RATIO, canopyRadius + SPAWN_CLEARANCE_MARGIN);
}

interface Bucketed {
  readonly cellSize: number;
  readonly stride: number;
  readonly origin: number;
  readonly buckets: Map<number, number[]>;
}

/** A spatial index over the solid trunks of a world, in RENDER space. */
export interface FloraColliderIndex extends Bucketed {
  /** How many solid trunks were indexed. Zero means nothing can collide. */
  readonly count: number;
  readonly x: Float64Array;
  readonly z: Float64Array;
  /** Trunk collider radius — what the walk rig resolves against. */
  readonly radius: Float64Array;
  /** Visual footprint radius — the space the instance occupies. */
  readonly canopy: Float64Array;
  /** Canopy widened by `spawnClearanceRadius` — what a spawn or capture pose stays outside of. */
  readonly spawn: Float64Array;
  /** Index into the source flora arrays, for callers that need the original instance. */
  readonly source: Int32Array;
}

/** A trunk overlapping a queried point. */
export interface FloraOverlap {
  /** Render-space centre of the trunk. */
  x: number;
  z: number;
  /** Radius of the footprint that was queried (collider or canopy). */
  radius: number;
  /** Distance from the queried point to the trunk centre. */
  distance: number;
  /** Index into the source flora arrays. */
  floraIndex: number;
}

/** Bucket edge length in render units. Matches the rig's original 8-unit grid. */
const BUCKET_SIZE = 8;

/**
 * Build the trunk index for a world at a given render size.
 *
 * Flora coordinates live in data-grid units centred on the origin; everything that collides with
 * them works in render units. The conversion (`renderSize / world.size`) is applied once, here,
 * rather than at each of the four call sites that used to do it themselves.
 */
export function buildFloraColliderIndex(world: FloraSource, renderSize: number): FloraColliderIndex {
  const toWorld = renderSize / world.size;

  let count = 0;
  for (let i = 0; i < world.floraCount; i++) if (isTallFlora(world.floraType[i])) count++;

  const x = new Float64Array(count);
  const z = new Float64Array(count);
  const radius = new Float64Array(count);
  const canopy = new Float64Array(count);
  const spawn = new Float64Array(count);
  const source = new Int32Array(count);

  // The rig keyed buckets as `cx * 2048 + cz`, which silently collides once a world spans more
  // than 2048 buckets on an axis. Deriving the stride from the extent removes the ceiling.
  const origin = renderSize;
  const stride = Math.ceil((2 * renderSize) / BUCKET_SIZE) + 3;
  const buckets = new Map<number, number[]>();

  let k = 0;
  for (let i = 0; i < world.floraCount; i++) {
    if (!isTallFlora(world.floraType[i])) continue;
    const wx = world.floraX[i] * toWorld;
    const wz = world.floraZ[i] * toWorld;
    x[k] = wx;
    z[k] = wz;
    radius[k] = floraColliderRadius(world.floraScale[i]);
    canopy[k] = floraCanopyRadius(world.floraType[i], world.floraScale[i]);
    spawn[k] = spawnClearanceRadius(canopy[k]);
    source[k] = i;

    const key = bucketKey(wx, wz, origin, stride);
    let arr = buckets.get(key);
    if (!arr) buckets.set(key, (arr = []));
    arr.push(k);
    k++;
  }

  return { count, x, z, radius, canopy, spawn, source, cellSize: BUCKET_SIZE, stride, origin, buckets };
}

function bucketKey(x: number, z: number, origin: number, stride: number): number {
  return Math.floor((x + origin) / BUCKET_SIZE) * stride + Math.floor((z + origin) / BUCKET_SIZE);
}

/**
 * The trunk a point is inside, or `null`. When several overlap, the *deepest* penetration wins so
 * a caller that resolves one overlap at a time converges.
 *
 * `margin` widens every collider — spawn and capture poses ask for `SPAWN_CLEARANCE_MARGIN`,
 * the runtime walk asks for `0` (touching bark while walking is normal; standing in the trunk
 * is not).
 */
export function floraOverlapAt(
  index: FloraColliderIndex,
  px: number,
  pz: number,
  margin = 0,
  footprint: FloraFootprint = 'collider',
): FloraOverlap | null {
  if (index.count === 0) return null;

  const radii =
    footprint === 'canopy' ? index.canopy : footprint === 'spawn' ? index.spawn : index.radius;
  // A canopy can be wider than one bucket, so the 3×3 neighbourhood the collider used is not
  // guaranteed to contain it. Widen the search by however many buckets the largest footprint
  // spans; for the collider (max ~1.05 units against an 8-unit bucket) this stays 3×3.
  const reach = Math.max(1, Math.ceil((maxRadius(radii) + margin) / index.cellSize));

  const gcx = Math.floor((px + index.origin) / index.cellSize);
  const gcz = Math.floor((pz + index.origin) / index.cellSize);

  let best: FloraOverlap | null = null;
  let deepest = 0;

  for (let dxc = -reach; dxc <= reach; dxc++) {
    for (let dzc = -reach; dzc <= reach; dzc++) {
      const arr = index.buckets.get((gcx + dxc) * index.stride + (gcz + dzc));
      if (!arr) continue;
      for (let n = 0; n < arr.length; n++) {
        const j = arr[n];
        const r = radii[j] + margin;
        const dx = px - index.x[j];
        const dz = pz - index.z[j];
        const d2 = dx * dx + dz * dz;
        if (d2 >= r * r) continue;
        const d = Math.sqrt(d2);
        const penetration = r - d;
        if (penetration > deepest) {
          deepest = penetration;
          best = {
            x: index.x[j],
            z: index.z[j],
            radius: radii[j],
            distance: d,
            floraIndex: index.source[j],
          };
        }
      }
    }
  }
  return best;
}

// Memoised per radii array: the index is immutable once built, and `floraOverlapAt` is called
// tens of thousands of times by the spawn scan.
const maxRadiusCache = new WeakMap<Float64Array, number>();
function maxRadius(radii: Float64Array): number {
  const hit = maxRadiusCache.get(radii);
  if (hit !== undefined) return hit;
  let m = 0;
  for (let i = 0; i < radii.length; i++) if (radii[i] > m) m = radii[i];
  maxRadiusCache.set(radii, m);
  return m;
}

/** Is this point outside every solid trunk / canopy (plus `margin`)? */
export function isFloraClear(
  index: FloraColliderIndex,
  px: number,
  pz: number,
  margin = SPAWN_CLEARANCE_MARGIN,
  footprint: FloraFootprint = 'collider',
): boolean {
  return floraOverlapAt(index, px, pz, margin, footprint) === null;
}

/** Direction taken when a point is exactly on a trunk centre and there is no offset to normalise. */
const DEGENERATE_DIR = { x: 1, z: 0 } as const;
/** Below this squared distance the offset carries no usable direction. Matches the rig's guard. */
const DEGENERATE_D2 = 1e-6;

/**
 * Move a point to the nearest position outside every solid trunk.
 *
 * Deterministic, idempotent, and defined at the trunk centre — the three properties the inline
 * runtime version lacked. Iterates because pushing out of one trunk can push into its neighbour;
 * bounded so a pathological clump cannot spin.
 */
export function resolveFloraOverlap(
  index: FloraColliderIndex,
  px: number,
  pz: number,
  margin = 0,
  footprint: FloraFootprint = 'collider',
): { x: number; z: number } {
  let x = px;
  let z = pz;

  // 8 is comfortably above what a real canopy produces (measured max 2 on the shipped world) and
  // still terminates on a hand-built worst case.
  for (let iter = 0; iter < 8; iter++) {
    const hit = floraOverlapAt(index, x, z, margin, footprint);
    if (!hit) break;
    const r = hit.radius + margin;
    const dx = x - hit.x;
    const dz = z - hit.z;
    const d2 = dx * dx + dz * dz;
    if (d2 > DEGENERATE_D2) {
      const d = Math.sqrt(d2);
      x = hit.x + (dx / d) * r;
      z = hit.z + (dz / d) * r;
    } else {
      // Exactly on the centre: no direction to preserve, so take the documented one.
      x = hit.x + DEGENERATE_DIR.x * r;
      z = hit.z + DEGENERATE_DIR.z * r;
    }
  }
  return { x, z };
}
