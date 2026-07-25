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
