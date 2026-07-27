// Pure, dependency-free validator for the Anima **map manifest** (M0.5 / test S05). A map manifest
// binds a World Artifact (ANMW, see worldArtifact.ts / src-tauri/src/core/world_artifact.rs) to the
// shared coordinate system, the 22/11 biome taxonomy, and the canonical camera/view list that the
// map-vision gate inspects (WORLD_SIMULATION_PLAN.md §13, pipeline: discover_map_artifacts ->
// validate_map_manifest -> prepare_team_review -> inspect_map_views).
//
// This is the local, hand-written counterpart of map_manifest.schema.json — no ajv, no runtime deps,
// no imports (keeps it usable from strict pure tests). It performs required-field presence + basic
// type checks and enforces the canonical view-id enum. The authoritative shape is the JSON Schema;
// keep the two in sync.

/** The eight canonical view ids the map-vision gate inspects when present (plan §13). */
export const CANONICAL_VIEW_IDS = [
  'overview',
  'navigation',
  'collision',
  'lighting',
  'spawn',
  'water',
  'biome_transition',
  'ecosystem',
] as const;

/** A canonical view identifier. */
export type CanonicalViewId = (typeof CANONICAL_VIEW_IDS)[number];

/** A world-space point, in the canonical coordinate bounds. */
export interface CanonicalPoint {
  position: [number, number, number];
  target: [number, number, number];
}

// ---- the canonical camera poses ----------------------------------------------------------
//
// One definition, two consumers: `scripts/gen_world_manifest.ts` publishes these as the manifest's
// `views[].camera`, and `tests/e2e/canonical_views.spec.ts` flies exactly them to produce
// `map-views/*.png`. They lived only in the generator while no capture existed, which meant the
// manifest could specify one shot and a harness take another and nothing would notice.
//
// Expressed in the CANONICAL bounds from COORDINATE_CONTRACT.md — x, z ∈ [-100, 100], y ∈ [0, 10]
// — because that is the space the manifest publishes and the map-review gates read. The landscape
// scene is a different span (1200 units wide, y exaggerated); `canonicalCameraToRender` is the
// only conversion, so a capture cannot quietly use a different one.

/** Span of the canonical XZ bounds (COORDINATE_CONTRACT.md §4: -100..100). */
export const CANONICAL_XZ_EXTENT = 200;
/** Top of the canonical Y range (elevation 1.0). */
export const CANONICAL_MAX_Y = 10;

/**
 * Where each canonical view looks from, and at what.
 *
 * Fixed poses, deliberately: a canonical view is a *repeatable* shot, so that a before/after pair
 * differs by the change under review and nothing else. A pose derived from world state at capture
 * time would move whenever the world moved, and two images that frame different places cannot be
 * compared.
 */
// Derived by `scripts/derive_view_cameras.ts` against the shipped world identity at worldgen v21.
//
// # Two things went wrong the first time, and both are structural
//
// **Stale.** The poses were derived at `WORLD_GEN_VERSION` 20 and then worldgen changed to 21 — a
// flora-species fix that moved every instance's biome. The committed `spawn` comment still claimed
// `findSpawn` returned render (-100.5, -86.5) when v21 returns (-128.7, -93.5), and the `ecosystem`
// subject had moved by 53 canonical units. Camera evidence describing a superseded world version is
// worse than none, because it looks current. Re-derive after **any** change to worldgen, the world
// identity, or these poses; re-run the capture; regenerate both manifests.
//
// **Outward-facing.** Every pose was `subject - (d, d)`: a fixed south-west offset that aims the
// lens north-east, which for a subject in the north-east is straight at the two nearest edges of a
// finite square of terrain. Independent review rejected `collision`, `water`, `biome_transition` and
// `ecosystem` for exactly that. The offset is now a constraint solver (`utils/viewFraming.ts`) that
// projects the image rectangle onto the ground and requires it to land inside the world;
// `viewFraming.test.ts` asserts that property of these literals, not of the solver.
export const CANONICAL_VIEW_CAMERAS: Record<CanonicalViewId, CanonicalPoint> = {
  // Whole map from 45°. 950 render units out; 815 is the minimum that frames a 1200-unit map at
  // the scene's 55° vertical FOV and 16:9, and the previous pose sat at 806 and clipped two edges.
  // Exempt from inward framing: the subject IS the whole bounded world.
  overview: { position: [0, 112, 112], target: [0, 0, 0] },
  // Open grassland inside the largest connected walkable component, 9/16 open samples. The route
  // overlay starts at this target — see `utils/mapEvidence.ts`. Two filters had to be added to get
  // here: the openness score counts open cells *around* a centre and never asks whether the centre is
  // inside a trunk (the first pick was), and being walkable does not mean being connected (the second
  // pick was a swamp clearing with seven reachable nodes out of fifty thousand).
  //
  // Further out and much steeper than every other view (68 units at 64° against the usual 26 at 42°),
  // because its subject is a *route* and the frame has to hold the whole of one. At the standard pose
  // the farthest goal whose path stayed in shot was a 60-unit stub along a shoreline, half of it
  // occluded by the ridge it crossed and its goal pillar out of frame. See `derive_view_cameras.ts`.
  navigation: { position: [7.9, 72.9, -38.1], target: [2.1, 11.8, -8.8] },
  // 302 solid trunks in one 24-unit bucket, jungle. The third-densest bucket, and that substitution
  // is the point: the densest two sit within six canonical units of the terrain edge, where no camera
  // frames them without the cut plane in shot. Framed inward (inwardness 1.00, azimuth 174°).
  collision: { position: [-62.1, 16.9, 6.8], target: [-54, 11.1, 6] },
  // Highest relief in the world (elevation 0.88, slope 1.00 — glacier), lit from the side so the
  // shadow pass has something to cast against. Horizon in frame by design: a lighting view has to
  // reach far enough for the sun's direction to read, and at 55° FOV a frame that reaches that far
  // spreads wider than this world. What it shows past the terrain is a mountain against sky.
  lighting: { position: [-95.6, 59.5, -32.2], target: [-59.9, 30.7, -17.4] },
  // The position `findSpawn` actually returns at the shipped identity: render (-128.7, -93.5),
  // grassland. The pose it replaced targeted canonical (10, 1, 10) — the middle of the map, which
  // on this world is open ocean, so the "spawn" view was a photograph of the sea.
  spawn: { position: [-29.5, 23.1, -21], target: [-21.4, 15.4, -15.6] },
  // Largest lake basin, 344 cells across. Shot from its shore at 34 canonical units rather than
  // framed whole: a distance that fits a 34-unit basin in frame spreads wider than the world.
  water: { position: [-85.2, 34.9, 81.9], target: [-67.3, 13.1, 64] },
  // Five distinct land biomes within ten cells, centred on alpine. The sharpest boundary in the world
  // has six, and the five sharpest are all too close to the edge to photograph.
  biome_transition: { position: [-64.8, 38.2, -34.5], target: [-45.1, 18.1, -24] },
  // Densest flora of any kind: 251 instances in one 20-unit bucket, swamp.
  ecosystem: { position: [-52.5, 22.6, -1.7], target: [-38.3, 10.9, -1.7] },
};

/**
 * Convert a canonical-bounds camera pose into the landscape scene's own span.
 *
 * **Uniform scale, including Y.** The tempting alternative is to scale `y` by the terrain's own
 * factor — `renderSize * heightRatio / CANONICAL_MAX_Y` — since `CANONICAL_MAX_Y` is 10 and the
 * render column is `renderSize * heightRatio` tall. That is wrong, and the first capture run shows
 * why: `overview` is authored at `[0, 95, 95]`, a 45° look at the origin, and the terrain factor
 * (16.8 at the shipped settings) lifted it to y=1596 over a 1200-wide map. The result was a
 * near-vertical shot of a small distant square.
 *
 * `CANONICAL_MAX_Y` describes how high *terrain* goes, not where a camera may sit. A pose is a
 * point in space and an angle; only a uniform scale preserves the angle, which is the whole
 * content of a camera specification.
 *
 * `heightRatio` is therefore unused, and kept in the signature deliberately: it is the number a
 * future reader will reach for, and its absence from the body is where they should find out why.
 */
export function canonicalCameraToRender(
  cam: CanonicalPoint,
  renderSize: number,
  _heightRatio?: number,
): CanonicalPoint {
  const k = renderSize / CANONICAL_XZ_EXTENT;
  const map = (p: [number, number, number]): [number, number, number] => [p[0] * k, p[1] * k, p[2] * k];
  return { position: map(cam.position), target: map(cam.target) };
}

/** Result of validating a manifest: `ok` is true only when `errors` is empty. */
export interface MapManifestValidationResult {
  ok: boolean;
  errors: string[];
}

/** Narrow an unknown value to a plain (non-array, non-null) object. */
function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

/** True for a finite number (rejects NaN/Infinity). */
function isNumber(v: unknown): v is number {
  return typeof v === 'number' && Number.isFinite(v);
}

/** True for a string. */
function isString(v: unknown): v is string {
  return typeof v === 'string';
}

/** True for a `[x, y, z]` triple of finite numbers. */
function isVec3(v: unknown): v is [number, number, number] {
  return Array.isArray(v) && v.length === 3 && v.every(isNumber);
}

/** Validate the `worldArtifact` reference block, appending any problems to `errors`. */
function validateWorldArtifact(wa: unknown, errors: string[]): void {
  if (wa === undefined) {
    errors.push('missing required field: worldArtifact');
    return;
  }
  if (!isRecord(wa)) {
    errors.push('worldArtifact must be an object');
    return;
  }
  if (!isString(wa.path)) errors.push('worldArtifact.path must be a string');
  if (wa.magic !== 'ANMW') errors.push('worldArtifact.magic must be the literal "ANMW"');
  if (!isNumber(wa.version)) errors.push('worldArtifact.version must be a number');
  if (!isNumber(wa.width)) errors.push('worldArtifact.width must be a number');
  if (!isNumber(wa.height)) errors.push('worldArtifact.height must be a number');
  if (!isNumber(wa.seaLevel)) errors.push('worldArtifact.seaLevel must be a number');
  if ('checksum' in wa && !isString(wa.checksum)) {
    errors.push('worldArtifact.checksum, when present, must be a string');
  }
}

/** Validate the `coordinateSystem` block (presence + numeric type of every required field). */
function validateCoordinateSystem(cs: unknown, errors: string[]): void {
  if (cs === undefined) {
    errors.push('missing required field: coordinateSystem');
    return;
  }
  if (!isRecord(cs)) {
    errors.push('coordinateSystem must be an object');
    return;
  }
  const keys = ['worldMinXZ', 'worldMaxXZ', 'worldMinY', 'worldMaxY', 'gridDim'] as const;
  for (const key of keys) {
    if (!isNumber(cs[key])) {
      errors.push(`coordinateSystem.${key} must be a number`);
    }
  }
}

/** Validate the `biomeTaxonomy` block (canonical/legacy counts present and numeric). */
function validateBiomeTaxonomy(bt: unknown, errors: string[]): void {
  if (bt === undefined) {
    errors.push('missing required field: biomeTaxonomy');
    return;
  }
  if (!isRecord(bt)) {
    errors.push('biomeTaxonomy must be an object');
    return;
  }
  if (!isNumber(bt.canonicalCount)) errors.push('biomeTaxonomy.canonicalCount must be a number');
  if (!isNumber(bt.legacyCount)) errors.push('biomeTaxonomy.legacyCount must be a number');
}

/** Validate a single view's `camera` (position + target must be world-space triples). */
function validateCamera(camera: unknown, label: string, errors: string[]): void {
  if (camera === undefined) {
    errors.push(`${label}.camera is required`);
    return;
  }
  if (!isRecord(camera)) {
    errors.push(`${label}.camera must be an object`);
    return;
  }
  if (!isVec3(camera.position)) {
    errors.push(`${label}.camera.position must be a [x, y, z] number triple`);
  }
  if (!isVec3(camera.target)) {
    errors.push(`${label}.camera.target must be a [x, y, z] number triple`);
  }
}

/** Validate a single entry of the `views` array. */
function validateView(view: unknown, index: number, errors: string[]): void {
  const label = `views[${index}]`;
  if (!isRecord(view)) {
    errors.push(`${label} must be an object`);
    return;
  }
  if (!isString(view.id)) {
    errors.push(`${label}.id must be a string`);
  } else if (!(CANONICAL_VIEW_IDS as readonly string[]).includes(view.id)) {
    errors.push(
      `${label}.id "${view.id}" is not a canonical view id (expected one of: ${CANONICAL_VIEW_IDS.join(', ')})`,
    );
  }
  if (!isString(view.imagePath)) {
    errors.push(`${label}.imagePath must be a string`);
  }
  validateCamera(view.camera, label, errors);
}

/** Validate the `views` array: must be a non-empty array of well-formed views. */
function validateViews(views: unknown, errors: string[]): void {
  if (views === undefined) {
    errors.push('missing required field: views');
    return;
  }
  if (!Array.isArray(views)) {
    errors.push('views must be an array');
    return;
  }
  if (views.length < 1) {
    errors.push('views must contain at least one view');
    return;
  }
  views.forEach((view, i) => validateView(view, i, errors));
}

/**
 * Validate an arbitrary value against the map-manifest contract (map_manifest.schema.json).
 *
 * Checks every required field (`schemaVersion`, `worldArtifact`, `coordinateSystem`,
 * `biomeTaxonomy`, `views`) and, for each view, the canonical id enum plus the camera triples.
 * Removing any required field — or using an unknown view id — yields `ok: false` with a descriptive
 * error. Unknown extra properties are tolerated (forward-compatible).
 *
 * @param obj - The parsed manifest (e.g. `JSON.parse(...)`); typed `unknown` so callers need no cast.
 * @returns `{ ok, errors }` — `ok` is true only when `errors` is empty.
 */
export function validateMapManifest(obj: unknown): MapManifestValidationResult {
  const errors: string[] = [];

  if (!isRecord(obj)) {
    return { ok: false, errors: ['manifest must be a non-null object'] };
  }

  if (!('schemaVersion' in obj)) {
    errors.push('missing required field: schemaVersion');
  } else if (!isNumber(obj.schemaVersion)) {
    errors.push('schemaVersion must be a number');
  }

  validateWorldArtifact(obj.worldArtifact, errors);
  validateCoordinateSystem(obj.coordinateSystem, errors);
  validateBiomeTaxonomy(obj.biomeTaxonomy, errors);
  validateViews(obj.views, errors);

  return { ok: errors.length === 0, errors };
}
