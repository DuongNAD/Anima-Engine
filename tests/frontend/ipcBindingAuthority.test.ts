import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const GENERATED_DIR = resolve(ROOT, 'src/types/generated');

// ---------------------------------------------------------------------------------------
// The generated bindings are the authority for IPC shapes.
//
// # What this defends
//
// `src/types/generated/` is written by ts-rs from the Rust structs, and CI runs
// `cargo test --lib export_bindings` followed by `git diff --exit-code` over that directory — so a
// Rust field rename that nobody mirrored fails the build.
//
// A hand-written copy of the same struct sits outside that gate by construction. `App.tsx` carried
// nine of them, and they were all *correct*, which is exactly the problem: a correct copy and a
// generated one are indistinguishable until the day the Rust side changes, and then the frontend
// keeps compiling against a shape the backend no longer sends. The failure surfaces as an
// `undefined` in a render, far from the cause.
//
// Switching to the generated types found one such divergence immediately: `ChronicleEvent`
// declared `parameter_delta: Record<string, number>` where the Rust `HashMap<String, f64>`
// actually produces `{ [k: string]?: number }`. `undefined >= 0` is false, so a missing delta had
// been rendering as `rate: undefined` with no sign.
//
// # Why a source scan here, when a source scan was the wrong tool for `BrainModel`
//
// Different question. There, the property was "this invariant holds", and encapsulation could
// enforce it structurally — so scanning was a weaker substitute for something better available.
// Here the property is "nobody re-declared a type that already exists", which is a statement about
// *the absence of a declaration*. A type that is never imported has no runtime or type-level
// footprint to assert on; there is nothing else to look at but the source.
// ---------------------------------------------------------------------------------------

/** Every type ts-rs currently exports, by file name. */
function generatedTypeNames(): string[] {
  return readdirSync(GENERATED_DIR)
    .filter((f) => f.endsWith('.ts'))
    .map((f) => f.replace(/\.ts$/, ''))
    .sort();
}

/** Source files that talk to the backend and could plausibly restate a payload shape. */
const IPC_CONSUMERS = ['src/App.tsx', 'src/PixiViewport.tsx', 'src/types/index.ts'];

describe('generated ts-rs bindings are the authority for IPC shapes', () => {
  it('has bindings to be authoritative with', () => {
    const names = generatedTypeNames();
    expect(names.length, 'src/types/generated is empty — did export_bindings run?').toBeGreaterThan(
      10,
    );
    // A spot-check that these are the IPC payloads and not some unrelated directory.
    expect(names).toContain('SimulationStatus');
    expect(names).toContain('MapElitesGridState');
  });

  it('no consumer re-declares a type that ts-rs already generates', () => {
    const generated = new Set(generatedTypeNames());
    const offences: string[] = [];

    for (const rel of IPC_CONSUMERS) {
      let src: string;
      try {
        src = readFileSync(resolve(ROOT, rel), 'utf8');
      } catch {
        continue; // a consumer that no longer exists is not an offence
      }
      // `export interface Foo {` / `export type Foo = {` — a declaration, not a re-export.
      for (const m of src.matchAll(/^export\s+(?:interface|type)\s+([A-Za-z0-9_]+)\s*[={]/gm)) {
        const name = m[1];
        if (generated.has(name)) {
          offences.push(
            `${rel} declares "${name}", which ts-rs already generates in src/types/generated/. ` +
              `Import it from there instead — a hand-written copy is outside the drift gate.`,
          );
        }
      }
    }

    expect(offences, offences.join('\n')).toEqual([]);
  });

  it('App.tsx imports its IPC payload types from the generated directory', () => {
    // The positive control. Without it, the test above passes for a file that stopped using these
    // types altogether, or that imports them from a third hand-written module.
    const src = readFileSync(resolve(ROOT, 'src/App.tsx'), 'utf8');
    for (const name of [
      'SimulationStatus',
      'MapElitesGridState',
      'PheromoneGridState',
      'RaycastTelemetry',
      'CombatEvent',
      'ChronicleEvent',
      'EvolutionSettings',
      'EliteIndividualState',
    ]) {
      expect(src, `App.tsx should import ${name} from the generated bindings`).toContain(
        `import type { ${name} } from './types/generated/${name}'`,
      );
    }
  });

  it('counts the IPC payloads that still have no ts-rs source', () => {
    // The honest remainder, asserted so it can only shrink.
    //
    // `LineageGraphState` (and its `LineageNode`/`LineageLink`) and `MigrationPayload` cross the
    // same bridge as everything above with none of the same protection: their Rust definitions do
    // not derive `TS`, so no binding is generated and no drift gate watches them. Deriving `TS`
    // there is the fix. Until then this number is the size of the gap, and lowering it is the only
    // way to change this assertion.
    const src = readFileSync(resolve(ROOT, 'src/App.tsx'), 'utf8');
    const generated = new Set(generatedTypeNames());
    const handWritten = [...src.matchAll(/^export\s+interface\s+([A-Za-z0-9_]+)\s*{/gm)]
      .map((m) => m[1])
      .filter((n) => !generated.has(n));

    expect(handWritten.sort()).toEqual([
      'LineageGraphState',
      'LineageLink',
      'LineageNode',
      'MigrationPayload',
    ]);
  });
});
