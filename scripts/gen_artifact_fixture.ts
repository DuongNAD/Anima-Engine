// Regenerate the cross-language World Artifact fixture consumed by the Rust test
//   src-tauri/src/core/world_artifact.rs :: decodes_frontend_generated_fixture
//
// It encodes a tiny 4x4 world (values are all f32-exact) with the REAL frontend encoder in
// worldArtifact.ts, so the fixture proves the TypeScript encoder emits bytes the Rust decoder reads
// byte-for-byte. Build + run from the repo root:
//
//   npx esbuild scripts/gen_artifact_fixture.ts --bundle --platform=node --format=cjs --outfile=<tmp>/g.cjs && node <tmp>/g.cjs
import { writeFileSync, mkdirSync } from 'fs';
import { dirname } from 'path';
import {
  CANONICAL_WORLD_SCALE,
  encodeWorldArtifact,
  worldToArtifact,
} from '../src/components/Landscape/utils/worldArtifact';
import { generateWorld } from '../src/components/Landscape/utils/worldGen';

const n = 16; // 4x4
const elevation = new Float32Array(n);
const moisture = new Float32Array(n);
const temperature = new Float32Array(n);
const flow = new Float32Array(n);
const biome = new Uint8Array(n);
for (let i = 0; i < n; i++) {
  elevation[i] = i * 0.5;
  moisture[i] = i * 0.25;
  temperature[i] = i * 0.125;
  flow[i] = i;
  biome[i] = i;
}

// Metadata mirrors the Rust `tiny_artifact()` EXACTLY so the cross-language byte-equality test holds
// (seed 1337, generator_version 20, world_scale 200 — all f32/u32-exact).
const buf = encodeWorldArtifact({
  width: 4,
  height: 4,
  seaLevel: 0.5,
  seed: 1337,
  generatorVersion: 20,
  worldScale: CANONICAL_WORLD_SCALE,
  elevation,
  moisture,
  temperature,
  flow,
  biome,
});

const out = 'src-tauri/tests/fixtures/world_4x4.anmw';
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, Buffer.from(buf));
console.log(`wrote ${out} (${buf.byteLength} bytes)`);

// A REAL world from the frontend generator at the backend's working resolution, encoded by the
// frontend's own worldToArtifact — proves worldGen's actual output is consumable by the backend.
const real = generateWorld('anima-fixture', { size: 128, shape: 'continent' });
const realBuf = worldToArtifact(real, 128);
const realOut = 'src-tauri/tests/fixtures/world_real_128.anmw';
writeFileSync(realOut, Buffer.from(realBuf));
console.log(`wrote ${realOut} (${realBuf.byteLength} bytes)`);
