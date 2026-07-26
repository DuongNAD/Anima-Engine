// Generate `map_manifest.json` and the World Artifact it points at, from the REAL world generator.
//
// # Why this exists
//
// `map_manifest.json` used to be hand-authored, and it had drifted into a document that asserted
// things that were not true: it named `artifacts/world_128.anmw` (no such file, no such directory),
// carried `sha256:` followed by 64 zeroes, and listed eight `map-views/*.png` that did not exist.
// The test that should have caught this built an inline copy of the manifest and validated *that*,
// so the committed file could rot arbitrarily far without turning anything red.
//
// A manifest is evidence or it is noise. This makes it evidence: every number below is computed
// from bytes that exist on disk when the script finishes.
//
// # What it does
//
//   1. runs the real frontend `generateWorld` (deterministic for a seed/size/shape);
//   2. encodes it with the real `worldToArtifact` — the same encoder whose byte-equality with the
//      Rust decoder is already gated by `decodes_frontend_generated_fixture`;
//   3. writes `artifacts/world_<dim>.anmw` and takes the SHA-256 of exactly those bytes;
//   4. emits the manifest with that path, that checksum, that width/height.
//
// # Two manifests, and why they are not the same document
//
// This is NOT `animal-map.manifest.json`. That one is the `animal-map-vision` MCP's schema
// (`schemaVersion` is a *string*) and is produced by `scripts/gen_map_manifest.ts`. This one is the
// project-local schema (`map_manifest.schema.json`, `schemaVersion` is a *number*) consumed by
// `src/components/Landscape/utils/mapManifest.ts` and test S05. Pointing the MCP's validator at this
// file returns `critical / invalid-manifest — schemaVersion: expected string, received number`,
// which is the validator being right about the wrong document. Do not "fix" that by changing the
// type here; it would break the local validator to satisfy a schema this file was never written to.
//
// Build + run from the repo root (offline):
//   npm run gen:world-manifest
// or with an explicit seed/size:
//   npm run gen:world-manifest -- 1337 256

import { writeFileSync, mkdirSync } from 'fs';
import { createHash } from 'crypto';
import { resolve, dirname } from 'path';
import {
  CANONICAL_WORLD_SCALE,
  WORLD_ARTIFACT_VERSION,
  worldToArtifact,
} from '../src/components/Landscape/utils/worldArtifact';
import { generateWorld } from '../src/components/Landscape/utils/worldGen';
import { CANONICAL_VIEW_IDS } from '../src/components/Landscape/utils/mapManifest';

// ---- config ---------------------------------------------------------------------------------

const SEED = process.argv[2] ?? '1337';
// The BACKEND working resolution — `DEFAULT_GRID_DIM` in `src-tauri/src/core/sim_rules.rs`, which
// is bound by test `s03_default_grid_dim_tracks_map_settings_default` to `MapSettings::default()`.
// It was 128 here for as long as that constant was stale; the world has run 256² since the working
// map was matched to the artifact.
const DIM = Number(process.argv[3] ?? 256);
const SHAPE = 'continent' as const;

// Fixed by MapBounds::default / SIMULATION_RULES.md. Restated as literals rather than imported so a
// silent change on either side shows up as a manifest diff.
const WORLD_MIN_XZ = -100;
const WORLD_MAX_XZ = 100;
const WORLD_MIN_Y = 0;
const WORLD_MAX_Y = 10;
const CANONICAL_BIOME_COUNT = 22;
const LEGACY_BIOME_COUNT = 11;

/**
 * Camera specifications for the canonical views.
 *
 * These are a *contract for what a capture must shoot*, not a record of captures that happened.
 * There is no capture pipeline in this repository — the project renders a 3D mesh and
 * `gen_map_manifest.ts` says as much about `pipeline.panorama` — and CLAUDE.md forbids running the
 * full backend on the development machine.
 *
 * So every view is emitted with `captured: false` and no image is claimed. The gate in
 * `mapManifestEvidence.test.ts` enforces the invariant that actually matters: anything claiming
 * `captured: true` must have a file on disk. The day a capture harness lands, it flips these flags
 * and the gate is already watching. Inventing eight PNGs to make a check go green is the exact
 * failure this whole file exists to remove.
 */
const VIEW_CAMERAS: Record<string, { position: [number, number, number]; target: [number, number, number] }> = {
  overview: { position: [0, 95, 95], target: [0, 0, 0] },
  navigation: { position: [-60, 45, -60], target: [-40, 0, -40] },
  collision: { position: [50, 30, 50], target: [30, 2, 30] },
  lighting: { position: [0, 80, -90], target: [0, 5, 0] },
  spawn: { position: [20, 25, 20], target: [10, 1, 10] },
  water: { position: [-80, 22, 12], target: [-95, 0, 0] },
  biome_transition: { position: [40, 32, -40], target: [20, 2, -20] },
  ecosystem: { position: [-30, 35, 60], target: [-10, 1, 40] },
};

// ---- generate -------------------------------------------------------------------------------

console.log(`generating world seed=${SEED} size=${DIM} shape=${SHAPE} ...`);
const world = generateWorld(SEED, { size: DIM, shape: SHAPE });

const buf = worldToArtifact(world, DIM);
const bytes = Buffer.from(buf);

const artifactRel = `artifacts/world_${DIM}.anmw`;
const artifactAbs = resolve(process.cwd(), artifactRel);
mkdirSync(dirname(artifactAbs), { recursive: true });
writeFileSync(artifactAbs, bytes);

// The real thing, over the bytes just written. Not a placeholder.
const checksum = `sha256:${createHash('sha256').update(bytes).digest('hex')}`;

const manifest = {
  $schema: './map_manifest.schema.json',
  schemaVersion: 1,
  _generated: {
    by: 'scripts/gen_world_manifest.ts',
    seed: SEED,
    shape: SHAPE,
    worldGenVersion: world.version,
    note:
      'Regenerate with `npm run gen:world-manifest`. The artifact is a build output and is ' +
      'gitignored; this manifest is tracked because the coordinate contract and test S05 read it.',
  },
  worldArtifact: {
    path: artifactRel,
    magic: 'ANMW',
    version: WORLD_ARTIFACT_VERSION,
    width: DIM,
    height: DIM,
    seaLevel: Math.round(world.seaLevel * 1e6) / 1e6,
    checksum,
    bytes: bytes.length,
  },
  coordinateSystem: {
    worldMinXZ: WORLD_MIN_XZ,
    worldMaxXZ: WORLD_MAX_XZ,
    worldMinY: WORLD_MIN_Y,
    worldMaxY: WORLD_MAX_Y,
    gridDim: DIM,
    unitsPerCell: (WORLD_MAX_XZ - WORLD_MIN_XZ) / DIM,
    worldScale: CANONICAL_WORLD_SCALE,
  },
  biomeTaxonomy: {
    canonicalCount: CANONICAL_BIOME_COUNT,
    legacyCount: LEGACY_BIOME_COUNT,
  },
  views: CANONICAL_VIEW_IDS.map((id) => ({
    id,
    imagePath: `map-views/${id}.png`,
    camera: VIEW_CAMERAS[id],
    // See the note on VIEW_CAMERAS. No capture pipeline exists, so nothing is claimed.
    captured: false,
  })),
};

const out = resolve(process.cwd(), 'map_manifest.json');
writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`);

console.log(
  `wrote ${artifactRel} (${bytes.length} bytes)\n` +
    `wrote map_manifest.json\n` +
    `  gridDim=${DIM} unitsPerCell=${(WORLD_MAX_XZ - WORLD_MIN_XZ) / DIM} seaLevel=${manifest.worldArtifact.seaLevel}\n` +
    `  ${checksum}\n` +
    `  views: ${CANONICAL_VIEW_IDS.length} specified, 0 captured`,
);
