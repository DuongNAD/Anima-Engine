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

import { writeFileSync, mkdirSync, existsSync, readFileSync } from 'fs';
import { createHash } from 'crypto';
import { resolve, dirname } from 'path';
import {
  CANONICAL_WORLD_SCALE,
  WORLD_ARTIFACT_VERSION,
  worldToArtifact,
} from '../src/components/Landscape/utils/worldArtifact';
import { generateWorld } from '../src/components/Landscape/utils/worldGen';
import {
  CANONICAL_VIEW_CAMERAS,
  CANONICAL_VIEW_IDS,
} from '../src/components/Landscape/utils/mapManifest';
import {
  SHARED_WORLD_SEED,
  SHARED_WORLD_SHAPE,
  SHARED_WORLD_SIZE,
} from '../src/utils/sharedWorld';

// ---- config ---------------------------------------------------------------------------------
//
// # Which world this describes, and why that is not a free choice
//
// The app has exactly one world identity, declared in `src/utils/sharedWorld.ts`: seed "seed",
// 2048², continent. `WorldShowcase` renders it, `worldCache.loadOrGenerateWorld` hands it to the
// backend, and `worldToArtifact(world, 256)` is the downsample that becomes the simulation's
// working grid. Two callers that disagree about any of the three do not merely render differently
// — they generate two worlds and the simulation lives on whichever page loaded last. That is why
// those constants are an identity rather than settings, and it is stated at the top of the file
// that owns them.
//
// This script used to generate `generateWorld("1337", { size: 256 })` and encode it directly. The
// resulting checksum was real — real bytes, real SHA-256, nothing fabricated — and it identified a
// world the app never renders. Note that "generate at 2048 then downsample to 256" and "generate
// at 256" are not two routes to one result: the generator samples noise at the grid it is given,
// so the second is a different world, not a coarser view of the first.
//
// So: import the identity, generate the authoritative world, downsample exactly as the shipped
// path does.
const SEED = process.argv[2] ?? SHARED_WORLD_SEED;
const SOURCE_SIZE = Number(process.argv[3] ?? SHARED_WORLD_SIZE);
// The BACKEND working resolution — `DEFAULT_GRID_DIM` in `src-tauri/src/core/sim_rules.rs`, which
// is bound by test `s03_default_grid_dim_tracks_map_settings_default` to `MapSettings::default()`,
// and by `worldCache.SIM_ARTIFACT_SIZE` on the shipped path.
const DIM = Number(process.argv[4] ?? 256);
const SHAPE = SHARED_WORLD_SHAPE;

// Fixed by MapBounds::default / SIMULATION_RULES.md. Restated as literals rather than imported so a
// silent change on either side shows up as a manifest diff.
const WORLD_MIN_XZ = -100;
const WORLD_MAX_XZ = 100;
const WORLD_MIN_Y = 0;
const WORLD_MAX_Y = 10;
const CANONICAL_BIOME_COUNT = 22;
const LEGACY_BIOME_COUNT = 11;

/**
 * Camera specifications for the canonical views — imported, not restated.
 *
 * `CANONICAL_VIEW_CAMERAS` is the single definition the deterministic capture harness
 * (`tests/e2e/canonical_views.spec.ts`) actually flies. Declaring the poses here as well would
 * mean the manifest could describe a shot nobody took.
 */
const VIEW_CAMERAS = CANONICAL_VIEW_CAMERAS;

// ---- generate -------------------------------------------------------------------------------

console.log(
  `generating the authoritative world seed=${SEED} size=${SOURCE_SIZE} shape=${SHAPE}, ` +
    `downsampling to ${DIM}² ...`,
);
const world = generateWorld(SEED, { size: SOURCE_SIZE, shape: SHAPE });

// Exactly what `worldCache.persistWorldArtifact` does before `save_world_artifact`.
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
    // The identity of the world these bytes came from. `mapManifestEvidence.test.ts` binds each of
    // these to `src/utils/sharedWorld.ts`, so a manifest generated for some other world fails the
    // gate instead of looking like evidence.
    seed: SEED,
    shape: SHAPE,
    sourceSize: SOURCE_SIZE,
    worldGenVersion: world.version,
    note:
      'Regenerate with `npm run gen:world-manifest`. The artifact is a build output and is ' +
      'gitignored; this manifest is tracked because the coordinate contract and test S05 read it. ' +
      'The identity above is the app identity from src/utils/sharedWorld.ts — it is not a knob.',
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
  views: CANONICAL_VIEW_IDS.map((id) => {
    const imagePath = `map-views/${id}.png`;
    const abs = resolve(process.cwd(), imagePath);
    // `captured` reports what is on disk when this runs. It is never asserted: if the capture
    // harness has not run, this stays false and the evidence gate is satisfied by the honest
    // absence rather than by a claim.
    if (!existsSync(abs)) {
      return { id, imagePath, camera: VIEW_CAMERAS[id], captured: false };
    }
    const png = readFileSync(abs);
    return {
      id,
      imagePath,
      camera: VIEW_CAMERAS[id],
      captured: true,
      bytes: png.length,
      checksum: `sha256:${createHash('sha256').update(png).digest('hex')}`,
    };
  }),
};

const out = resolve(process.cwd(), 'map_manifest.json');
writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`);

const capturedCount = manifest.views.filter((v) => v.captured).length;
console.log(
  `wrote ${artifactRel} (${bytes.length} bytes)\n` +
    `wrote map_manifest.json\n` +
    `  identity: seed=${SEED} source=${SOURCE_SIZE}² shape=${SHAPE} genVersion=${world.version}\n` +
    `  gridDim=${DIM} unitsPerCell=${(WORLD_MAX_XZ - WORLD_MIN_XZ) / DIM} seaLevel=${manifest.worldArtifact.seaLevel}\n` +
    `  ${checksum}\n` +
    `  views: ${CANONICAL_VIEW_IDS.length} specified, ${capturedCount} captured`,
);
