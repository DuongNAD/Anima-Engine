// Measurement half of `check_flora_footprint.mjs`. Kept as TypeScript so it imports the same
// modules the app does, and run through `run_ts.mjs` so it is bundled by rolldown.
//
// Builds every flora geometry with real three, measures the largest horizontal reach of its
// vertices, and compares that to what `floraClearance.ts` declares. Exits non-zero on any drift,
// any solid type missing a declaration, or any declaration for a type that is not solid.

import { FloraType } from '../src/components/Landscape/utils/worldGen';
import { measureFloraFootprintRadius } from '../src/components/Landscape/utils/floraGeometry';
import {
  FLORA_FOOTPRINT_UNIT_RADIUS,
  FLORA_FOOTPRINT_TOLERANCE,
  TALL_FLORA_TYPES,
} from '../src/components/Landscape/utils/floraClearance';

const problems: string[] = [];
const rows: string[] = [];

for (const type of TALL_FLORA_TYPES) {
  const declared = FLORA_FOOTPRINT_UNIT_RADIUS[type];
  const measured = measureFloraFootprintRadius(type);
  const name = FloraType[type];

  if (declared === undefined) {
    problems.push(`${name} is solid but has no declared footprint (measured ${measured.toFixed(4)})`);
    continue;
  }
  const drift = Math.abs(declared - measured);
  rows.push(
    `  ${name.padEnd(9)} declared ${declared.toFixed(4)}  measured ${measured.toFixed(4)}  ` +
      `drift ${drift.toExponential(1)}`,
  );
  if (drift > FLORA_FOOTPRINT_TOLERANCE) {
    problems.push(
      `${name}: declared ${declared} but the geometry measures ${measured.toFixed(4)} ` +
        `(drift ${drift.toFixed(4)} > ${FLORA_FOOTPRINT_TOLERANCE})`,
    );
  }
}

const solid = new Set<number>(TALL_FLORA_TYPES);
for (const key of Object.keys(FLORA_FOOTPRINT_UNIT_RADIUS)) {
  const t = Number(key);
  if (!solid.has(t)) {
    problems.push(
      `${FloraType[t] ?? t} has a declared footprint but is not in TALL_FLORA_TYPES — ` +
        `nothing tests it against the geometry`,
    );
  }
}

console.log(`flora footprint: ${TALL_FLORA_TYPES.length} solid types measured against real three`);
console.log(rows.join('\n'));

if (problems.length > 0) {
  console.error(`\ncheck:flora-footprint FAILED — ${problems.length} problem(s):`);
  for (const p of problems) console.error(`  - ${p}`);
  console.error(
    '\nThe spawn picker and the canonical-view capture keep the camera outside these radii. ' +
      'Update FLORA_FOOTPRINT_UNIT_RADIUS in floraClearance.ts to the measured values.',
  );
  process.exit(1);
}

console.log('\ncheck:flora-footprint OK — every declared radius matches the geometry');
