import { describe, it, expect } from 'vitest';
import { readFileSync, existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const NOTICE = resolve(ROOT, 'NOTICE');
const SBOM = resolve(ROOT, 'sbom.cdx.json');
const INDEX = resolve(ROOT, 'licensing', 'third-party-index.json');

function notice(): string {
  expect(existsSync(NOTICE), 'NOTICE must exist — run `node scripts/gen_notice.mjs`').toBe(true);
  return readFileSync(NOTICE, 'utf8');
}

interface LicenseIndex {
  counts: {
    cargo: number;
    npmDistributed: number;
    npmInstallOnly: number;
    fromVendoredUpstream: number;
  };
  components: { purl: string; name: string; version: string; distributed: boolean; spdx: string | null }[];
  unresolved: { purl: string; name: string }[];
}

function index(): LicenseIndex {
  expect(existsSync(INDEX), 'run `npm run gen:licenses`').toBe(true);
  return JSON.parse(readFileSync(INDEX, 'utf8')) as LicenseIndex;
}

describe('NOTICE — the attribution inventory of what actually ships', () => {
  it('attributes the transitive graph, not just the direct dependencies', () => {
    // The regression this test exists for.
    //
    // The generator read `package.json.dependencies` and stopped: eight names. The bundler pulls in
    // the transitive graph, so what ships in `dist/` includes everything those eight pull in, each
    // carrying the same MIT/BSD/Apache obligation as the package that named it.
    //
    // These four are chosen because none of them appears in `package.json`, so a generator that
    // regressed to a direct-only list would fail here rather than merely get smaller. They are
    // load-bearing runtime code, not incidental tooling: `scheduler` is React's cooperative
    // scheduler, `earcut` triangulates every Pixi polygon, `eventemitter3` is Pixi's event bus,
    // and `zustand` is react-three-fiber's store.
    const text = notice();
    const pkg = JSON.parse(readFileSync(resolve(ROOT, 'package.json'), 'utf8')) as {
      dependencies?: Record<string, string>;
    };
    const direct = new Set(Object.keys(pkg.dependencies ?? {}));

    for (const transitive of ['scheduler', 'earcut', 'eventemitter3', 'zustand']) {
      expect(direct.has(transitive), `${transitive} should be transitive, not direct`).toBe(false);
      expect(text, `NOTICE omits the transitive component ${transitive}`).toContain(transitive);
    }
  });

  it('names every component with a version', () => {
    const text = notice();
    const npmSection = text.slice(text.indexOf('### Packages with bytes in dist/'));
    const entries = [...npmSection.matchAll(/^- (\S+) (\d+\.\d+\.\d+[^\s—]*)/gm)];
    expect(entries.length, 'the npm section should list versioned components').toBeGreaterThan(15);
  });

  it('separates what is distributed from what is merely installed', () => {
    // `node_modules` is not shipped; Tauri packages `dist/`. Listing the install closure as though
    // it were the product over-attributes — 18 components, including type-only packages that
    // contain no runtime code at all. The two sets are now distinct headings, so a reader can see
    // which obligation attaches to which.
    const text = notice();
    expect(text).toContain('## JavaScript components — distributed');
    expect(text).toContain('## JavaScript components — installed, not distributed');
    expect(text.indexOf('## JavaScript components — distributed')).toBeLessThan(
      text.indexOf('## JavaScript components — installed, not distributed'),
    );

    const idx = index();
    expect(text).toContain(`**npm, distributed — ${idx.counts.npmDistributed} components.**`);
    expect(text).toContain(
      `**npm, installed but not distributed — ${idx.counts.npmInstallOnly} components.**`,
    );
  });

  it('leaves nothing without a licence decision recorded', () => {
    // An unattributed component is the failure mode this file guards; "UNKNOWN" in a generated
    // inventory is the same problem wearing a label. A component that genuinely declares nothing is
    // allowed — but only if it is named in the Gaps section, where it cannot be overlooked.
    const text = notice();
    expect(text).not.toMatch(/^- .*UNKNOWN/m);
    expect(text).not.toContain('NOT INSTALLED');

    const gaps = text.slice(text.indexOf('## Gaps'), text.indexOf('## What this file does NOT'));
    const undeclared = index().components.filter((c) => c.distributed && c.spdx === null);
    for (const c of undeclared) {
      expect(gaps, `${c.name} declares no licence and must be named in Gaps`).toContain(
        `${c.name} ${c.version}`,
      );
    }
  });

  it('points at the licence texts, and states the residual gap with the real count', () => {
    // This assertion replaces one that required NOTICE to keep saying "Licence **texts** are not
    // reproduced … the packaging step must copy each component's licence file, and it currently
    // does not". That sentence was true and load-bearing when it was written: it kept an honest
    // admission of a release blocker from being quietly dropped while the gap remained.
    //
    // The gap no longer remains in that form. `licensing/THIRD_PARTY_LICENSES.txt` packages the text
    // of all but a handful of the distributed components, so keeping the old sentence would have
    // made NOTICE false in the opposite direction. What still must not be droppable is the
    // **residual** gap, so the guard moves rather than disappears — and it is now stronger, because
    // every number in the prose has to equal the number in the machine-readable artifact behind it
    // rather than merely being a sentence someone remembered to leave in.
    const text = notice();
    const idx = index();

    expect(text).toContain('licensing/THIRD_PARTY_LICENSES.txt');
    expect(text).toContain('licensing/third-party-index.json');
    expect(text).toContain(`The remaining **${idx.unresolved.length}** are enumerated`);
    expect(text).toContain('licensing/UNRESOLVED.md');
    expect(text).toContain('No text is invented to cover for that.');

    // Where a text was not in the artifact it must say so, with the count the store actually holds.
    // The claim being guarded here is the one a reader would most reasonably misread: that every
    // packaged text came out of the thing that was installed. Some did not, and NOTICE says which.
    expect(text).toContain(`**${idx.counts.fromVendoredUpstream}** of those publish no licence file`);
    expect(text).toContain('licensing/upstream/sources.json');
    expect(text).toContain('the immutable commit the release');
    expect(text).toContain('Text read out of the installed\nartifact is always preferred to a vendored copy.');

    // And it still refuses to claim what it has not done.
    expect(text).toContain('It is an **inventory**, not a legal review.');
    expect(text).toContain('nothing here constitutes legal sign-off');
    expect(text).not.toContain('legal sign-off has been');
  });

  it('reports counts that match the inventory rather than a remembered number', () => {
    // Both CI and the status doc carried hand-written counts that had gone stale — "419 crates and
    // 45 npm packages" in one, "419 crate + 8 gói npm" in another, against a measured 419 + 36.
    // Everything countable in NOTICE is now derived, so this asserts the derivation held.
    const text = notice();
    const idx = index();
    expect(text).toContain(`**Rust — ${idx.counts.cargo} crates.**`);
    expect(text).toContain(`### Crates linked into the desktop binary — ${idx.counts.cargo} component(s)`);
    expect(text).toContain(
      `### Packages with bytes in dist/ — ${idx.counts.npmDistributed} component(s)`,
    );
  });
});

describe('SBOM — a machine-readable bill of materials', () => {
  it('exists and is CycloneDX', () => {
    expect(existsSync(SBOM), 'sbom.cdx.json must exist — run `node scripts/gen_sbom.mjs`').toBe(true);
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
    expect(npm.length, 'the npm graph should be in the SBOM').toBeGreaterThan(20);
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
