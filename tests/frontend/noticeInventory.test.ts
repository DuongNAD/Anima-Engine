import { describe, it, expect } from 'vitest';
import { readFileSync, existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const NOTICE = resolve(ROOT, 'NOTICE');
const SBOM = resolve(ROOT, 'sbom.cdx.json');

function notice(): string {
  expect(existsSync(NOTICE), 'NOTICE must exist — run `node scripts/gen_notice.mjs`').toBe(true);
  return readFileSync(NOTICE, 'utf8');
}

describe('NOTICE — the attribution inventory of what actually ships', () => {
  it('attributes the npm PRODUCTION CLOSURE, not just the direct dependencies', () => {
    // The regression this test exists for.
    //
    // The generator read `package.json.dependencies` and stopped: eight names. Vite bundles the
    // transitive graph, so what ships in `dist/` includes everything those eight pull in, each
    // carrying the same MIT/BSD/Apache obligation as the package that named it.
    //
    // These four are chosen because none of them appears in `package.json`, so a generator that
    // regressed to a direct-only list would fail here rather than merely get smaller. They are
    // load-bearing runtime code, not incidental tooling: `scheduler` is React's cooperative
    // scheduler, `earcut` triangulates every Pixi polygon, `eventemitter3` is Pixi's event bus,
    // and `zustand` is react-three-fiber's store.
    const text = notice();
    const pkg = JSON.parse(readFileSync(resolve(ROOT, 'package.json'), 'utf8'));
    const direct = new Set(Object.keys(pkg.dependencies ?? {}));

    for (const transitive of ['scheduler', 'earcut', 'eventemitter3', 'zustand']) {
      expect(direct.has(transitive), `${transitive} should be transitive, not direct`).toBe(false);
      expect(text, `NOTICE omits the transitive component ${transitive}`).toContain(transitive);
    }
  });

  it('names every component with a version', () => {
    const text = notice();
    const npmSection = text.slice(text.indexOf('### Packages bundled into dist/'));
    const entries = [...npmSection.matchAll(/^- (\S+) (\d+\.\d+\.\d+[^\s—]*)/gm)];
    expect(entries.length, 'the npm section should list versioned components').toBeGreaterThan(20);
  });

  it('leaves nothing without a declared licence', () => {
    // An unattributed component is the failure mode this file guards; "UNKNOWN" in a generated
    // inventory is the same problem wearing a label.
    const text = notice();
    expect(text).not.toMatch(/^- .*UNKNOWN/m);
    expect(text).not.toContain('NOT INSTALLED');
  });

  it('still says the licence texts themselves are not packaged', () => {
    // The obligation MIT and BSD impose is that the *text* travels with the distribution, and it
    // does not yet. The inventory is a prerequisite, not the discharge. This assertion exists so
    // that the honest statement cannot be quietly dropped from the generated file while the gap
    // remains — closing it is a release-blocking task with a named owner, recorded in the
    // planning doc, not something to let fade.
    const text = notice();
    expect(text).toContain('Licence **texts** are not reproduced');
    expect(text).toContain('it currently does not');
  });
});

describe('SBOM — a machine-readable bill of materials', () => {
  it('exists and is CycloneDX', () => {
    expect(
      existsSync(SBOM),
      'sbom.cdx.json must exist — run `node scripts/gen_sbom.mjs`',
    ).toBe(true);
    const doc = JSON.parse(readFileSync(SBOM, 'utf8'));
    expect(doc.bomFormat).toBe('CycloneDX');
    expect(doc.specVersion).toBe('1.5');
    expect(Array.isArray(doc.components)).toBe(true);
  });

  it('covers both ecosystems and carries purls', () => {
    // An inventory grouped by licence string is a document for humans. An SBOM is a document for
    // tools — vulnerability scanners, license-policy engines, procurement — and what makes it one
    // is that every component has a stable machine identifier.
    const doc = JSON.parse(readFileSync(SBOM, 'utf8'));
    const cargo = doc.components.filter((c: { purl?: string }) => c.purl?.startsWith('pkg:cargo/'));
    const npm = doc.components.filter((c: { purl?: string }) => c.purl?.startsWith('pkg:npm/'));

    expect(cargo.length, 'the Rust graph should be in the SBOM').toBeGreaterThan(100);
    expect(npm.length, 'the npm production closure should be in the SBOM').toBeGreaterThan(20);
    for (const c of doc.components) {
      expect(c.purl, `${c.name} has no purl`).toBeTruthy();
      expect(c.version, `${c.name} has no version`).toBeTruthy();
    }
  });

  it('is deterministic — components are sorted, and no timestamp churns the file', () => {
    // A bill of materials that differs on every run cannot be diffed, and a `--check` gate over it
    // would fail for reasons that are not about the dependency graph. So the component list is
    // sorted and the metadata carries no wall-clock timestamp.
    const doc = JSON.parse(readFileSync(SBOM, 'utf8'));
    const keys = doc.components.map((c: { purl: string }) => c.purl);
    expect(keys).toEqual([...keys].sort());
    expect(JSON.stringify(doc.metadata ?? {})).not.toMatch(/"timestamp"/);
  });
});
