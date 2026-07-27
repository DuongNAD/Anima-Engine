#!/usr/bin/env node
// Bundle a TypeScript script to CJS and run it, using the bundler this repo actually ships.
//
// # Why this exists
//
// `gen:manifest` and `gen:world-manifest` both invoked `esbuild`, and **esbuild is not installed**.
// Vite 8 bundles with rolldown/oxc and no longer depends on it — the same fact CLAUDE.md already
// records for `build.minify`, where naming "esbuild" explicitly fails with
// `Cannot find package 'esbuild'`. So `npm run gen:manifest` had been failing with
// `'esbuild' is not recognized as an internal or external command`, which means the "real map
// manifest exporter" could not be run by anyone following the documented instructions.
//
// Routing both scripts through rolldown (already a dependency, `node_modules/.bin/rolldown`) makes
// them runnable again without adding a dependency.
//
// Usage: node scripts/run_ts.mjs <entry.ts> [args...]

import { rolldown } from 'rolldown';
import { pathToFileURL } from 'node:url';
import { resolve, basename } from 'node:path';
import { mkdirSync, writeFileSync } from 'node:fs';

const [entry, ...rest] = process.argv.slice(2);
if (!entry) {
  console.error('usage: node scripts/run_ts.mjs <entry.ts> [args...]');
  process.exit(2);
}

const outDir = resolve(process.cwd(), 'node_modules/.cache');
mkdirSync(outDir, { recursive: true });
const outFile = resolve(outDir, `${basename(entry).replace(/\.ts$/, '')}.mjs`);

const bundle = await rolldown({
  input: resolve(process.cwd(), entry),
  platform: 'node',
  // Everything these scripts import is first-party TS under src/; nothing needs externalising.
  treeshake: true,
});
const { output } = await bundle.generate({ format: 'esm', codeSplitting: false });
await bundle.close();

writeFileSync(outFile, output[0].code);

// The bundled script reads its own CLI args, so hand them over as if it had been invoked directly.
process.argv = [process.argv[0], outFile, ...rest];
await import(pathToFileURL(outFile).href);
