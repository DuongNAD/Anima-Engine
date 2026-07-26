#!/usr/bin/env node
// Gate: the declared flora footprint radii must equal the geometry the app actually builds.
//
// # Why this is a script and not a test
//
// `FLORA_FOOTPRINT_UNIT_RADIUS` in `src/components/Landscape/utils/floraClearance.ts` is what the
// spawn picker and the canonical-view capture keep the camera outside of. It has to agree with the
// meshes `floraGeometry.ts` builds, and the only honest way to check that is to build them and
// measure — reading the source for `0.95` and trusting it is what let the numbers drift apart in
// the first place.
//
// Measuring needs real three. Both Vitest configs alias `three` to `tests/mocks/three.ts`
// (see CLAUDE.md), so a Vitest test cannot do it: it would measure the mock. This script runs in
// plain Node against the installed three, which is the same package the browser bundle uses.
//
// Fails closed: a declared value that drifts, a solid flora type with no declared value, and a
// declared value for a type that is not solid are all errors.
//
//   node scripts/check_flora_footprint.mjs

import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');

// The measurement itself runs through `run_ts.mjs` so the TypeScript sources are bundled by the
// bundler this repo actually ships (rolldown — esbuild is not installed).
const probe = resolve(ROOT, 'scripts/_flora_footprint_probe.ts');
const res = spawnSync(process.execPath, [resolve(ROOT, 'scripts/run_ts.mjs'), probe], {
  cwd: ROOT,
  encoding: 'utf8',
});

if (res.status !== 0) {
  process.stderr.write(res.stdout ?? '');
  process.stderr.write(res.stderr ?? '');
  console.error('\ncheck:flora-footprint FAILED — the measurement probe did not run');
  process.exit(res.status ?? 1);
}

process.stdout.write(res.stdout);
process.exit(0);
