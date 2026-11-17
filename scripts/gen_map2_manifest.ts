import { writeFileSync, readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';
import {
  generateMap2Manifest,
  validateMap2Manifest,
  validateMap2EcologicalRules,
} from '../src/components/Map2/utils/map2Manifest';

// Writes artifacts/maps/map2/map2_manifest.json from the Map 2 terrain, flora and navigation code.
//
// It writes nothing else. It renders no image and copies no world artifact: Map 2 has never had a
// captured canonical view, and it has no `.anmw` of its own. The script this replaces
// (verify_map2_evidence.ts) wrote a 1x1 PNG per view, copied Map 1's world_256.anmw in as Map 2's,
// and marked every view captured.
//
// Usage:
//   node scripts/run_ts.mjs scripts/gen_map2_manifest.ts           write the manifest
//   node scripts/run_ts.mjs scripts/gen_map2_manifest.ts --check   exit 1 if the committed file differs

const MANIFEST_PATH = resolve(process.cwd(), 'artifacts/maps/map2/map2_manifest.json');

const manifest = generateMap2Manifest({
  mapId: 'map2_open_world',
  mapName: 'Map 2 - Open World Ecosystem',
  seed: 'anima_map2_photorealistic_v1',
  seaLevel: 12.0,
  elevationRange: [0, 90],
});

const schema = validateMap2Manifest(manifest);
if (!schema.ok) {
  console.error(`map2 manifest: schema errors:\n  ${schema.errors.join('\n  ')}`);
  process.exit(1);
}
const ecology = validateMap2EcologicalRules(manifest);
if (!ecology.ok) {
  console.error(`map2 manifest: ecological violations:\n  ${ecology.violations.join('\n  ')}`);
  process.exit(1);
}

const json = JSON.stringify(manifest, null, 2) + '\n';

if (process.argv.includes('--check')) {
  const committed = existsSync(MANIFEST_PATH) ? readFileSync(MANIFEST_PATH, 'utf8') : '';
  if (committed !== json) {
    console.error(`map2 manifest: ${MANIFEST_PATH} is stale; run scripts/gen_map2_manifest.ts`);
    process.exit(1);
  }
  console.log('map2 manifest: up to date');
} else {
  writeFileSync(MANIFEST_PATH, json, 'utf8');
  console.log(`map2 manifest: wrote ${MANIFEST_PATH}`);
}
