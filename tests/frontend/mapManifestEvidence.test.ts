import { describe, it, expect } from 'vitest';
import { readFileSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { validateMapManifest, CANONICAL_VIEW_IDS } from '@/components/Landscape/utils/mapManifest';

// Evidence gate for the COMMITTED map manifest.
//
// # Why this file lives in tests/ and not src/__tests__
//
// It reads the filesystem, and the root tsconfig type-checks `src/**` for the production build
// against browser libs only -- no @types/node, correctly, since the frontend does not run in node.
// Putting an fs test there broke `npm run build` with TS2307 on `node:fs`. The `tests/` package has
// @types/node, so this is where a test that touches disk belongs.
//
// # Why this file exists separately from mapManifest.test.ts
//
// The two tests have different jobs and must not be merged. `mapManifest.test.ts` proves the
// *validator* rejects malformed input; it builds manifests inline and stays free of `fs` so it can
// run as a pure test. That is correct for what it does.
//
// It is also exactly why the real defect went unnoticed for so long. The committed
// `map_manifest.json` named `artifacts/world_128.anmw` (no such file, and no `artifacts/`
// directory at all), carried `sha256:` followed by 64 zeroes, and listed eight `map-views/*.png`
// that did not exist. Every one of those is structurally valid, so a schema check on a copy of the
// document passes while the document describes nothing.
//
// This file checks the physical claims: the file on disk, the bytes it points at, and the hash of
// those bytes. Regenerate with `npm run gen:world-manifest`.

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '../..');
const MANIFEST_PATH = resolve(ROOT, 'map_manifest.json');

function loadManifest(): Record<string, unknown> {
  expect(existsSync(MANIFEST_PATH), `map_manifest.json must exist at ${MANIFEST_PATH}`).toBe(true);
  return JSON.parse(readFileSync(MANIFEST_PATH, 'utf8')) as Record<string, unknown>;
}

describe('map manifest — the committed file, not a copy of it', () => {
  it('passes its own validator', () => {
    const res = validateMapManifest(loadManifest());
    expect(res.errors).toEqual([]);
    expect(res.ok).toBe(true);
  });

  it('points at a World Artifact that exists', () => {
    const m = loadManifest();
    const wa = m.worldArtifact as Record<string, unknown>;
    const artifactPath = resolve(ROOT, String(wa.path));
    expect(
      existsSync(artifactPath),
      `worldArtifact.path "${wa.path}" does not exist. This is the exact defect this gate was ` +
        `written for. Run \`npm run gen:world-manifest\`.`,
    ).toBe(true);
  });

  it('declares the real SHA-256 of those bytes', () => {
    const m = loadManifest();
    const wa = m.worldArtifact as Record<string, unknown>;
    const bytes = readFileSync(resolve(ROOT, String(wa.path)));

    const declared = String(wa.checksum ?? '');
    // The original value was 64 zeroes — a well-formed string that hashes nothing. Reject the shape
    // explicitly so a placeholder can never satisfy the "checksum is a string" schema rule again.
    expect(declared).toMatch(/^sha256:[0-9a-f]{64}$/);
    expect(declared, 'an all-zero checksum is a placeholder, not a checksum').not.toBe(
      `sha256:${'0'.repeat(64)}`,
    );

    const actual = `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
    expect(actual).toBe(declared);
    expect(bytes.length).toBe(wa.bytes);
  });

  it('describes the artifact it actually points at', () => {
    const m = loadManifest();
    const wa = m.worldArtifact as Record<string, unknown>;
    const bytes = readFileSync(resolve(ROOT, String(wa.path)));

    // ANMW header (worldArtifact.ts): magic, u32 version, u32 width, u32 height, ...
    expect(bytes.subarray(0, 4).toString('ascii')).toBe('ANMW');
    const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    expect(dv.getUint32(4, true)).toBe(wa.version);
    expect(dv.getUint32(8, true)).toBe(wa.width);
    expect(dv.getUint32(12, true)).toBe(wa.height);
  });

  it('agrees with the backend coordinate contract', () => {
    const m = loadManifest();
    const cs = m.coordinateSystem as Record<string, number>;
    const wa = m.worldArtifact as Record<string, unknown>;

    // DEFAULT_GRID_DIM in src-tauri/src/core/sim_rules.rs, bound by
    // `s03_default_grid_dim_tracks_map_settings_default` to MapSettings::default(). The manifest
    // said 128 while the world ran 256, and published a units-per-cell figure wrong by 2x.
    expect(cs.gridDim).toBe(256);
    expect(cs.worldMinXZ).toBe(-100);
    expect(cs.worldMaxXZ).toBe(100);
    expect(cs.worldMinY).toBe(0);
    expect(cs.worldMaxY).toBe(10);
    expect(cs.unitsPerCell).toBeCloseTo((cs.worldMaxXZ - cs.worldMinXZ) / cs.gridDim, 10);
    expect(cs.unitsPerCell).toBe(0.78125);

    // The artifact must be the grid the coordinate system describes, or the two blocks describe
    // different worlds while each looks internally consistent.
    expect(wa.width).toBe(cs.gridDim);
    expect(wa.height).toBe(cs.gridDim);
  });

  it('lists every canonical view exactly once', () => {
    const m = loadManifest();
    const views = m.views as Array<Record<string, unknown>>;
    const ids = views.map((v) => String(v.id));
    expect([...ids].sort()).toEqual([...CANONICAL_VIEW_IDS].sort());
    expect(new Set(ids).size).toBe(ids.length);
  });

  it('only claims a captured image when the image is on disk', () => {
    // The invariant that matters, and the one that survives the capture pipeline landing later.
    //
    // There is no capture harness in this repository — the app renders a 3D mesh, and CLAUDE.md
    // forbids running the full backend on the development machine — so every view is currently
    // `captured: false` and no PNG is claimed. Asserting "eight files exist" would have forced
    // eight fabricated screenshots, which is worse than an honest absence. Asserting the
    // implication means the day a view flips to `captured: true` without a file, this fails.
    const m = loadManifest();
    const views = m.views as Array<Record<string, unknown>>;

    for (const v of views) {
      if (v.captured === true) {
        const img = resolve(ROOT, String(v.imagePath));
        expect(existsSync(img), `view "${v.id}" claims captured:true but ${v.imagePath} is missing`).toBe(
          true,
        );
      }
    }

    // Record the current honest state, so flipping a flag without capturing is a deliberate act
    // that updates this number rather than something that slips through.
    const capturedCount = views.filter((v) => v.captured === true).length;
    expect(capturedCount).toBe(0);
  });
});
