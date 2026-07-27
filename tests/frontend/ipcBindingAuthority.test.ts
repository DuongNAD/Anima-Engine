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

  it('leaves no IPC payload without a ts-rs source', () => {
    // This used to assert a remainder of four — `LineageGraphState`, `LineageNode`, `LineageLink`
    // and `MigrationPayload` — on the grounds that their Rust definitions carried no `TS` derive.
    // That is the defect the gate exists to remove, not an exemption from it, so the derives were
    // added and the list is now empty.
    //
    // `MigrationPayload` is worth remembering. Its Rust fields were `direction: String` and
    // `status: String` with the permitted values written in a trailing comment, while `App.tsx`
    // declared `'incoming' | 'outgoing'` and `'Success' | 'Failed'` as real unions and
    // `src/types/index.ts` declared a third variant with `status: string`. Three mirrors, two of
    // them disagreeing, none compared to the source. They are now `MigrationDirection` and
    // `MigrationStatus`, generated from Rust enums whose serde renames reproduce the exact strings
    // the wire has always carried.
    const src = readFileSync(resolve(ROOT, 'src/App.tsx'), 'utf8');
    const generated = new Set(generatedTypeNames());
    const handWritten = [...src.matchAll(/^export\s+interface\s+([A-Za-z0-9_]+)\s*{/gm)]
      .map((m) => m[1])
      .filter((n) => !generated.has(n));

    expect(
      handWritten.sort(),
      'every IPC payload type must be generated by ts-rs, so the regeneration/diff gate covers it',
    ).toEqual([]);
  });

  it('generated the migration enums, not just a pair of strings', () => {
    // The specific improvement, asserted so a future revert to `String` is visible. A binding that
    // says `direction: string` would satisfy every other test in this file and would have lost the
    // only thing the hand-written mirrors got right.
    const generated = new Set(generatedTypeNames());
    expect(generated).toContain('MigrationDirection');
    expect(generated).toContain('MigrationStatus');

    const direction = readFileSync(resolve(GENERATED_DIR, 'MigrationDirection.ts'), 'utf8');
    const status = readFileSync(resolve(GENERATED_DIR, 'MigrationStatus.ts'), 'utf8');
    // The exact strings the wire has always carried, including the inconsistent capitalisation.
    expect(direction).toContain('"incoming"');
    expect(direction).toContain('"outgoing"');
    expect(status).toContain('"Success"');
    expect(status).toContain('"Failed"');

    const payload = readFileSync(resolve(GENERATED_DIR, 'MigrationPayload.ts'), 'utf8');
    expect(payload).toContain('MigrationDirection');
    expect(payload).toContain('MigrationStatus');
  });
});
