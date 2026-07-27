import { describe, it, expect } from 'vitest';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { readFileSync, existsSync } from 'node:fs';
import { resolve, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { TEXT_EXTENSIONS, findControlByteOffsets } from '../../scripts/check_text_hygiene.mjs';
import {
  isLicenseFileName,
  byteCompare,
  collectLicenseTexts,
  decodeLicenseText,
  deterministicSerialNumber,
  isSafeLicenseEntry,
  isSpdxExpression,
  licenseNode,
  provenancePath,
  type LicenseChoice,
} from '../../scripts/lib/licensing.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const LICENSING = resolve(ROOT, 'licensing');

interface TextEntry {
  id: string;
  filename: string;
  provenance: string;
  sourceSha256: string;
  sourceBytes: number;
  textSha256: string;
}
interface IndexComponent {
  purl: string;
  name: string;
  version: string;
  ecosystem: 'cargo' | 'npm';
  origin: string;
  distributed: boolean;
  spdx: string | null;
  repository: string | null;
  texts: TextEntry[];
}
interface LicenseIndex {
  counts: {
    total: number;
    distributed: number;
    cargo: number;
    npmDistributed: number;
    npmInstallOnly: number;
    texts: number;
    unresolved: number;
  };
  components: IndexComponent[];
  texts: { id: string; textSha256: string; usedBy: number }[];
  unresolved: { purl: string; name: string; version: string; reasons: string[] }[];
}
interface BundleClosure {
  packages: { name: string; version: string; origin: 'node_modules' | 'injected' }[];
  unresolved: string[];
}

function readIndex(): LicenseIndex {
  expect(
    existsSync(resolve(LICENSING, 'third-party-index.json')),
    'licensing/third-party-index.json must exist — run `npm run gen:licenses`',
  ).toBe(true);
  return JSON.parse(readFileSync(resolve(LICENSING, 'third-party-index.json'), 'utf8')) as LicenseIndex;
}

function readClosure(): BundleClosure {
  return JSON.parse(readFileSync(resolve(LICENSING, 'bundle-closure.json'), 'utf8')) as BundleClosure;
}

function withTempDir(run: (dir: string) => void): void {
  const dir = mkdtempSync(join(tmpdir(), 'anima-licence-'));
  try {
    run(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

describe('licence file selection — what counts as a licence, and what must never be read', () => {
  it('matches the filename spellings that actually occur, and rejects source files that merely contain the word', () => {
    // Measured across 419 crates: LICENSE-MIT (402), LICENSE-APACHE (376), LICENSE (200),
    // license-apache-2.0 (58), COPYRIGHT (35), LICENSE.md, COPYING, UNLICENSE, LICENSE_MIT,
    // LICENSE.MIT, LICENSE.txt.
    for (const name of [
      'LICENSE', 'LICENCE', 'LICENSE-MIT', 'LICENSE-APACHE', 'license-apache-2.0', 'LICENSE.md',
      'LICENSE.txt', 'LICENSE_APACHE-2.0', 'LICENSE.MIT', 'COPYING', 'COPYRIGHT', 'NOTICE',
      'UNLICENSE',
    ]) {
      expect(isLicenseFileName(name), `${name} should be treated as licence text`).toBe(true);
    }
    // The suffix group has to allow a dot for `LICENSE.md` and `LICENSE.txt`, which is exactly what
    // lets `LICENSE-check.py` through — a real filename in real packages, and one whose contents
    // would then be published as a licence grant. Anchoring alone does not catch it.
    for (const name of [
      'LICENSE-check.py', 'license_generator.rs', 'licenses.json', 'relicense.md', 'NOTICES.tsx',
      'LICENSE.sh', 'COPYING.yml', 'README.md', 'Cargo.toml', 'not-a-LICENSE',
    ]) {
      expect(isLicenseFileName(name), `${name} is not a licence grant`).toBe(false);
    }
  });

  it('refuses symlinks, directories and traversal names', () => {
    // A symlink is the one that matters: `readdirSync(…, {withFileTypes: true})` uses lstat
    // semantics, so a link reports isSymbolicLink() and never isFile(). Without that check a
    // package could ship `LICENSE -> /etc/shadow` — or, less exotically, a link out of the package
    // whose target then lands verbatim in a file we publish.
    //
    // Asserted against hand-built entries rather than real links on purpose: creating a symlink on
    // Windows needs a privilege this project's machine does not have, and a check that can only run
    // on one platform is not a check.
    const file = (name: string) => ({ name, isFile: () => true });
    const notFile = (name: string) => ({ name, isFile: () => false });

    expect(isSafeLicenseEntry(file('LICENSE'))).toBe(true);
    expect(isSafeLicenseEntry(notFile('LICENSE')), 'a symlink or directory is not a file').toBe(false);
    expect(isSafeLicenseEntry(file('../LICENSE')), 'parent traversal').toBe(false);
    expect(isSafeLicenseEntry(file('..')), 'the parent itself').toBe(false);
    expect(isSafeLicenseEntry(file('.')), 'the directory itself').toBe(false);
    expect(isSafeLicenseEntry(file('sub/LICENSE')), 'posix separator').toBe(false);
    expect(isSafeLicenseEntry(file('sub\\LICENSE')), 'windows separator').toBe(false);
  });

  it('reads only licence files from a directory, in a stable order', () => {
    withTempDir((dir) => {
      writeFileSync(join(dir, 'LICENSE-MIT'), 'MIT text\n');
      writeFileSync(join(dir, 'LICENSE-APACHE'), 'Apache text\n');
      writeFileSync(join(dir, 'README.md'), 'not a licence\n');
      mkdirSync(join(dir, 'LICENSES'));

      const found = collectLicenseTexts(dir);
      expect(found.map((f) => f.filename)).toEqual(['LICENSE-APACHE', 'LICENSE-MIT']);
      expect(found[0].text).toBe('Apache text\n');
    });
  });

  it('returns nothing for a directory that is not there, rather than throwing', () => {
    // Three crates in the graph resolve to a source directory that a pruned registry may not have
    // unpacked. That is a component to report as unresolved, not a crash that stops the whole
    // inventory and leaves no artifact at all.
    withTempDir((dir) => {
      expect(collectLicenseTexts(join(dir, 'does-not-exist'))).toEqual([]);
    });
  });
});

describe('licence text decoding — verbatim, except where verbatim is not reproducible', () => {
  it('normalises CRLF and strips a BOM, so the bundle is byte-identical on Windows and Linux', () => {
    const crlf = decodeLicenseText(Buffer.from('MIT License\r\n\r\nCopyright (c) X\r\n', 'utf8'));
    expect(crlf.text).toBe('MIT License\n\nCopyright (c) X\n');
    expect(crlf.lossless).toBe(true);

    const bom = decodeLicenseText(Buffer.from('\uFEFFMIT License\n', 'utf8'));
    expect(bom.text).toBe('MIT License\n');

    // A lone CR is a Mac-classic line ending and still occurs in old licence files.
    expect(decodeLicenseText(Buffer.from('a\rb\r', 'utf8')).text).toBe('a\nb\n');
  });

  it('reports a file that is not valid UTF-8 instead of transcoding it into mojibake', () => {
    // Copyright lines carry names, and a latin-1 file decoded as UTF-8 turns "Müller" into
    // replacement characters. Publishing that as the copyright notice a licence requires be
    // reproduced is a corruption that reads as compliance, so it becomes an unresolved component.
    const latin1 = Buffer.from([0x4d, 0xfc, 0x6c, 0x6c, 0x65, 0x72, 0x0a]);
    const decoded = decodeLicenseText(latin1);
    expect(decoded.lossless).toBe(false);

    expect(decodeLicenseText(Buffer.from('Müller\n', 'utf8')).lossless).toBe(true);
  });

  it('always ends a non-empty text with a newline, so concatenation cannot fuse two licences', () => {
    expect(decodeLicenseText(Buffer.from('no trailing newline', 'utf8')).text).toBe(
      'no trailing newline\n',
    );
    expect(decodeLicenseText(Buffer.from('', 'utf8')).text).toBe('');
  });
});

describe('SPDX classification — a malformed expression must not be laundered into a valid one', () => {
  it('uses license.id for a bare SPDX identifier', () => {
    expect(licenseNode('MIT')).toEqual([{ license: { id: 'MIT' } }]);
    expect(licenseNode('Apache-2.0')).toEqual([{ license: { id: 'Apache-2.0' } }]);
    expect(licenseNode('0BSD')).toEqual([{ license: { id: '0BSD' } }]);
  });

  it('uses expression for a real compound expression', () => {
    for (const expr of ['MIT OR Apache-2.0', '0BSD OR MIT OR Apache-2.0', 'MIT AND BSD-3-Clause']) {
      expect(licenseNode(expr), expr).toEqual([{ expression: expr }]);
      expect(isSpdxExpression(expr), expr).toBe(true);
    }
    expect(isSpdxExpression('Apache-2.0 WITH LLVM-exception')).toBe(true);
  });

  it('falls back to license.name for strings SPDX has no grammar for', () => {
    // Five `unic-*` crates in the shipped graph declare `MIT/Apache-2.0`. SPDX has never had `/` as
    // a separator, and the previous implementation classified on a character class, so this string
    // — and every other malformed one made of licence-ish characters — was emitted as an
    // `expression`, asserting a conformance it does not have.
    for (const raw of ['MIT/Apache-2.0', 'Apache-2.0 / MIT', 'MIT OR NotARealLicence', 'see LICENSE']) {
      expect(isSpdxExpression(raw), raw).toBe(false);
      expect(licenseNode(raw), raw).toEqual([{ license: { name: raw } }]);
    }
  });

  it('treats absent and UNKNOWN as no decision, never as a licence named "UNKNOWN"', () => {
    for (const raw of [null, undefined, '', '   ', 'UNKNOWN']) {
      expect(licenseNode(raw), String(raw)).toBeUndefined();
    }
  });

  it('rejects dangling operators rather than accepting a truncated expression', () => {
    for (const bad of ['MIT OR', 'OR MIT', 'MIT WITH', 'AND', 'MIT OR OR Apache-2.0']) {
      expect(isSpdxExpression(bad), bad).toBe(false);
    }
  });
});

describe('determinism', () => {
  it('derives the SBOM serial number from content, so identical graphs give identical bytes', () => {
    const a = deterministicSerialNumber('{"components":[]}');
    expect(a).toBe(deterministicSerialNumber('{"components":[]}'));
    expect(a).not.toBe(deterministicSerialNumber('{"components":[1]}'));
    // A well-formed v5 UUID, which is what the CycloneDX schema's pattern requires.
    expect(a).toMatch(/^urn:uuid:[0-9a-f]{8}-[0-9a-f]{4}-5[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
  });

  it('sorts by bytes, not by locale', () => {
    // `localeCompare` orders punctuation by collation rules that disagree with a byte comparison, so
    // a locale-sorted artifact disagrees with every consumer that sorts the obvious way.
    const input = ['@scope/a', 'Zed', 'a-b', 'a_b', 'ab'];
    const byBytes = [...input].sort(byteCompare);
    expect(byBytes).toEqual([...input].sort((x, y) => (x < y ? -1 : x > y ? 1 : 0)));
    expect(byBytes[0]).toBe('@scope/a');
  });

  it('emits provenance that names the package, never an absolute machine path', () => {
    // The cargo registry directory carries a per-machine hash and the repository root differs on
    // every checkout. Either one in a committed artifact makes the freshness gate report the
    // machine rather than the dependency graph.
    const cargo = provenancePath(
      { ecosystem: 'cargo', name: 'serde', version: '1.0.200', sourceDir: 'C:\\Users\\x\\.cargo\\registry\\src\\index.crates.io-1949cf8c\\serde-1.0.200' },
      'LICENSE-MIT',
      ROOT,
    );
    expect(cargo).toBe('cargo-registry:serde-1.0.200/LICENSE-MIT');
    expect(cargo).not.toContain('index.crates.io');

    const npm = provenancePath(
      { ecosystem: 'npm', name: 'react', version: '18.3.1', sourceDir: join(ROOT, 'node_modules', 'react') },
      'LICENSE',
      ROOT,
    );
    expect(npm).toBe('node_modules/react/LICENSE');
    expect(npm).not.toContain('\\');
  });
});

describe('the generated licence artifacts describe what actually ships', () => {
  it('covers every distributed component with a text or a named blocker — never silence', () => {
    const index = readIndex();
    const distributed = index.components.filter((c) => c.distributed);
    const unresolved = new Set(index.unresolved.map((u) => u.purl));

    expect(distributed.length).toBeGreaterThan(400);
    for (const c of distributed) {
      const accounted = c.texts.length > 0 || unresolved.has(c.purl);
      expect(accounted, `${c.purl} ships with neither a licence text nor a recorded reason`).toBe(true);
    }
    // The inverse: a blocker must name a component that is actually in the bill.
    const purls = new Set(index.components.map((c) => c.purl));
    for (const u of index.unresolved) {
      expect(purls.has(u.purl), `${u.purl} is recorded as unresolved but is not a component`).toBe(true);
      expect(u.reasons.length, `${u.purl} is unresolved with no reason`).toBeGreaterThan(0);
    }
  });

  it('separates the install closure from the ship closure', () => {
    // The regression this exists for. `npm ls --omit=dev --all` is what npm installs for
    // production; `dist/` is what Tauri packages. The old artifacts used the first as if it were
    // the second, over-attributing packages that leave no bytes in the product.
    const index = readIndex();
    const installOnly = index.components.filter((c) => c.ecosystem === 'npm' && !c.distributed);
    expect(installOnly.length).toBeGreaterThan(0);

    // Type-only packages are the clearest case: they contain no runtime code at all, so a bundler
    // cannot put them in the output, and attributing them as distributed is simply false.
    const names = new Set(installOnly.map((c) => c.name));
    for (const typeOnly of ['@types/react', 'csstype']) {
      expect(names.has(typeOnly), `${typeOnly} contains no runtime code and cannot be distributed`).toBe(true);
    }
    // …and no install-only component carries licence text, because none of them carries an
    // obligation.
    for (const c of installOnly) expect(c.texts).toEqual([]);
  });

  it('attributes toolchain code that ships even though it is not a production dependency', () => {
    // Measured from the real module graph: the bundler compiles `vite/preload-helper.js`,
    // `rolldown/runtime.js` and four `@oxc-project/runtime` helpers into the shipped chunks. None
    // is in `dependencies`, so no reading of package.json can find them, and every artifact
    // generated from `npm ls --omit=dev` missed all three.
    const index = readIndex();
    const injected = index.components.filter((c) => c.origin === 'toolchain-injected');
    expect(injected.map((c) => c.name).sort()).toEqual(['@oxc-project/runtime', 'rolldown', 'vite']);
    for (const c of injected) {
      expect(c.distributed, `${c.name} is compiled into dist/ and is therefore distributed`).toBe(true);
    }
  });

  it('keeps two versions of one package as two components', () => {
    // `scheduler` is installed at 0.21.0 (under react-reconciler) and 0.23.2 (under react-dom), and
    // both have bytes in `dist/`. A name-keyed inventory silently keeps whichever it saw first and
    // attributes the other's copyright to it.
    const closure = readClosure();
    const schedulers = closure.packages.filter((p) => p.name === 'scheduler');
    expect(schedulers.map((p) => p.version).sort()).toEqual(['0.21.0', '0.23.2']);

    const index = readIndex();
    const inIndex = index.components.filter((c) => c.name === 'scheduler');
    expect(inIndex.length).toBe(2);
    expect(new Set(inIndex.map((c) => c.purl)).size, 'each version needs its own identity').toBe(2);
  });

  it('includes transitive dependencies, not just the ones a human would list', () => {
    const index = readIndex();
    const pkg = JSON.parse(readFileSync(resolve(ROOT, 'package.json'), 'utf8')) as {
      dependencies?: Record<string, string>;
    };
    const direct = new Set(Object.keys(pkg.dependencies ?? {}));
    const distributed = new Set(
      index.components.filter((c) => c.ecosystem === 'npm' && c.distributed).map((c) => c.name),
    );
    for (const transitive of ['scheduler', 'earcut', 'eventemitter3', 'zustand', 'react-reconciler']) {
      expect(direct.has(transitive), `${transitive} should be transitive, not direct`).toBe(false);
      expect(distributed.has(transitive), `${transitive} ships and must be attributed`).toBe(true);
    }
  });

  it('gives every text a stable content-addressed identity and deduplicates only through it', () => {
    const index = readIndex();
    const ids = new Set<string>();
    const shas = new Set<string>();
    for (const t of index.texts) {
      expect(ids.has(t.id), `duplicate text id ${t.id}`).toBe(false);
      expect(shas.has(t.textSha256), `duplicate text sha ${t.textSha256}`).toBe(false);
      ids.add(t.id);
      shas.add(t.textSha256);
      expect(t.id).toMatch(/^T\d{4}$/);
      expect(t.usedBy).toBeGreaterThan(0);
    }
    // Ids are assigned by sorting the distinct texts, so they depend on the set and not on which
    // component happened to be visited first.
    expect(index.texts.map((t) => t.textSha256)).toEqual([...index.texts.map((t) => t.textSha256)].sort());

    // Every reference from a component resolves to a declared text.
    for (const c of index.components) {
      for (const t of c.texts) {
        expect(ids.has(t.id), `${c.purl} references unknown text ${t.id}`).toBe(true);
        expect(t.sourceSha256).toMatch(/^[0-9a-f]{64}$/);
        expect(t.sourceBytes).toBeGreaterThan(0);
      }
    }
    // Deduplication is real, not incidental. Measured: 722 licence files across the shipped graph
    // collapse to 247 distinct texts, and the most-shared one — the Apache-2.0 boilerplate — is
    // referenced by 97 components. It is not shared by *every* Apache-2.0 crate, because several
    // fill in the appendix differently, which is precisely why dedup keys on content and not on the
    // SPDX identifier.
    const references = index.texts.reduce((sum, t) => sum + t.usedBy, 0);
    expect(references).toBeGreaterThan(index.texts.length * 2);
    expect(Math.max(...index.texts.map((t) => t.usedBy))).toBeGreaterThan(50);
  });

  it('records provenance that is machine-independent for every text', () => {
    const index = readIndex();
    for (const c of index.components) {
      for (const t of c.texts) {
        expect(t.provenance, `${c.purl} leaks a Windows path`).not.toMatch(/^[A-Za-z]:/);
        expect(t.provenance, `${c.purl} leaks a backslash`).not.toContain('\\');
        expect(t.provenance, `${c.purl} leaks the registry hash`).not.toContain('index.crates.io');
        expect(t.provenance.endsWith(`/${t.filename}`), `${c.purl} provenance must name the file`).toBe(true);
      }
    }
  });

  it('reproduces the texts it claims to, byte for byte', () => {
    // The bundle is what ships. If it and the index disagree, the index is a description of a file
    // that does not exist.
    const index = readIndex();
    const bundle = readFileSync(resolve(LICENSING, 'THIRD_PARTY_LICENSES.txt'), 'utf8');
    for (const t of index.texts) {
      expect(bundle, `${t.id} is indexed but not in the bundle`).toContain(`${t.id}   sha256:${t.textSha256}`);
    }
    expect(bundle).toContain(`Distinct licence texts                       : ${index.texts.length}`);
    expect(bundle).toContain(`Components with no obtainable text           : ${index.unresolved.length}`);
  });

  it('states the residual gap in NOTICE with the count the index actually holds', () => {
    // The honest-statement guard, re-pointed. NOTICE used to promise that licence texts were not
    // packaged at all; that is no longer true, and leaving the sentence would be its own false
    // claim. What must not be droppable is the *residual* gap, so this asserts the number in the
    // prose equals the number in the machine-readable artifact behind it.
    const index = readIndex();
    const notice = readFileSync(resolve(ROOT, 'NOTICE'), 'utf8');
    expect(notice).toContain('licensing/THIRD_PARTY_LICENSES.txt');
    expect(notice).toContain(`The remaining **${index.unresolved.length}** are enumerated`);
    expect(notice).toContain('licensing/UNRESOLVED.md');

    const unresolvedDoc = readFileSync(resolve(LICENSING, 'UNRESOLVED.md'), 'utf8');
    expect(unresolvedDoc).toContain(`**${index.unresolved.length} component(s) unresolved.**`);
    for (const u of index.unresolved) {
      expect(unresolvedDoc, `${u.name} is unresolved but not named in UNRESOLVED.md`).toContain(
        `\`${u.name}\``,
      );
    }
  });

  it('never claims a licence text it did not read', () => {
    const index = readIndex();
    const unresolved = new Set(index.unresolved.map((u) => u.purl));
    for (const c of index.components) {
      if (unresolved.has(c.purl) && c.texts.length === 0) continue;
      if (c.distributed) {
        expect(c.texts.length > 0 || unresolved.has(c.purl)).toBe(true);
      }
    }
    // Counts in the artifact are derived, so they cannot be edited to look better than the data.
    expect(index.counts.unresolved).toBe(index.unresolved.length);
    expect(index.counts.texts).toBe(index.texts.length);
    expect(index.counts.total).toBe(index.components.length);
    expect(index.counts.distributed).toBe(index.components.filter((c) => c.distributed).length);
  });
});

describe('the bundle closure is the boundary, and is usable as one', () => {
  it('records no unattributed shipped module', () => {
    expect(readClosure().unresolved).toEqual([]);
  });

  it('agrees with the licence index about what the frontend distributes', () => {
    const closure = readClosure();
    const index = readIndex();
    const fromClosure = new Set(closure.packages.map((p) => `${p.name}@${p.version}`));
    const fromIndex = new Set(
      index.components.filter((c) => c.ecosystem === 'npm' && c.distributed).map((c) => `${c.name}@${c.version}`),
    );
    expect([...fromIndex].sort()).toEqual([...fromClosure].sort());
  });

  it('carries no absolute path, so it is identical on every checkout', () => {
    const raw = readFileSync(resolve(LICENSING, 'bundle-closure.json'), 'utf8');
    expect(raw).not.toMatch(/[A-Za-z]:\\/);
    expect(raw).not.toContain('/home/');
    expect(raw).not.toContain('node_modules/');
  });
});

describe('the SBOM is a checked CycloneDX document, not a CycloneDX-shaped one', () => {
  const sbom = () =>
    JSON.parse(readFileSync(resolve(ROOT, 'sbom.cdx.json'), 'utf8')) as {
      specVersion: string;
      serialNumber: string;
      metadata: { timestamp?: string; component: { 'bom-ref': string } };
      components: { 'bom-ref': string; purl: string; scope: string; licenses?: LicenseChoice[] }[];
      dependencies: { ref: string; dependsOn: string[] }[];
    };

  it('validates against the vendored official schema, which is pinned by checksum', () => {
    // The schema bytes are the gate. If they can change without a commit here, so can the meaning
    // of "valid", and `scripts/check_sbom_schema.mjs` re-hashes them on every run for that reason.
    const provenance = JSON.parse(
      readFileSync(resolve(ROOT, 'schemas/cyclonedx/PROVENANCE.json'), 'utf8'),
    ) as { source: { commit: string }; files: { file: string; sha256: string }[] };
    expect(provenance.source.commit).toMatch(/^[0-9a-f]{40}$/);
    for (const f of provenance.files) {
      expect(f.sha256).toMatch(/^[0-9a-f]{64}$/);
      expect(existsSync(resolve(ROOT, 'schemas/cyclonedx', f.file)), `${f.file} is vendored`).toBe(true);
    }
  });

  it('gives every component a unique bom-ref, a purl and a distribution scope', () => {
    const doc = sbom();
    const refs = new Set<string>();
    for (const c of doc.components) {
      expect(refs.has(c['bom-ref']), `duplicate bom-ref ${c['bom-ref']}`).toBe(false);
      refs.add(c['bom-ref']);
      expect(c.purl).toBeTruthy();
      expect(['required', 'excluded']).toContain(c.scope);
    }
    expect(doc.components.filter((c) => c.scope === 'excluded').length).toBeGreaterThan(0);
  });

  it('has a dependency graph in which every reference resolves', () => {
    const doc = sbom();
    const known = new Set([doc.metadata.component['bom-ref'], ...doc.components.map((c) => c['bom-ref'])]);
    const declared = new Set<string>();
    for (const d of doc.dependencies) {
      expect(known.has(d.ref), `dependency record ${d.ref} names no component`).toBe(true);
      declared.add(d.ref);
      for (const target of d.dependsOn) {
        expect(known.has(target), `${d.ref} -> ${target} names no component`).toBe(true);
      }
    }
    expect(declared.size, 'every component needs a dependency record').toBe(known.size);
    // The graph is a graph, not a flat list: the root must actually depend on things, and so must
    // some interior node.
    const root = doc.dependencies.find((d) => d.ref === doc.metadata.component['bom-ref']);
    expect(root?.dependsOn.length).toBeGreaterThan(5);
    expect(doc.dependencies.some((d) => d.ref !== root?.ref && d.dependsOn.length > 0)).toBe(true);
  });

  it('is reproducible: sorted, timestamp-free, and serialised from its own content', () => {
    const doc = sbom();
    const purls = doc.components.map((c) => c.purl);
    expect(purls).toEqual([...purls].sort());
    expect(doc.metadata.timestamp).toBeUndefined();
    expect(doc.serialNumber).toMatch(
      /^urn:uuid:[0-9a-f]{8}-[0-9a-f]{4}-5[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
    );
    expect(doc.specVersion).toBe('1.5');
  });
});

describe('the validator rejects invalid documents — a gate that only ever passes is not a gate', () => {
  // Each case mutates a copy of the real SBOM and runs the real gate against it. Asserting the
  // negative matters more than asserting the positive here: a validator with a typo'd schema path,
  // a swallowed exception or an inverted condition prints "valid" for everything, and the committed
  // SBOM would keep passing while the check meant nothing.
  const runValidator = (doc: unknown): { status: number; output: string } => {
    const dir = mkdtempSync(join(tmpdir(), 'anima-sbom-'));
    try {
      const file = join(dir, 'mutated.cdx.json');
      writeFileSync(file, `${JSON.stringify(doc, null, 2)}\n`);
      const result = spawnSync(process.execPath, [resolve(ROOT, 'scripts/check_sbom_schema.mjs'), file], {
        cwd: ROOT,
        encoding: 'utf8',
      });
      return { status: result.status ?? -1, output: `${result.stdout}${result.stderr}` };
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  };

  const fresh = (): Record<string, unknown> =>
    JSON.parse(readFileSync(resolve(ROOT, 'sbom.cdx.json'), 'utf8')) as Record<string, unknown>;

  it('accepts the committed SBOM', () => {
    const result = runValidator(fresh());
    expect(result.output).toContain('valid CycloneDX 1.5');
    expect(result.status).toBe(0);
  }, 60_000);

  it('rejects a document the official schema forbids', () => {
    // `version` is required by the schema and must be an integer. Removing it is a violation the
    // extra assertions in the checker do not look at, so only the schema can catch it — which is
    // the point of validating against the schema at all.
    const doc = fresh();
    delete doc.version;
    const result = runValidator(doc);
    expect(result.status).toBe(1);
    expect(result.output).toContain('schema:');
  }, 60_000);

  it('rejects a wrong specVersion', () => {
    const doc = fresh();
    doc.specVersion = '1.4';
    const result = runValidator(doc);
    expect(result.status).toBe(1);
  }, 60_000);

  it('rejects duplicate component identities', () => {
    const doc = fresh();
    const components = doc.components as Record<string, unknown>[];
    doc.components = [...components, { ...components[0] }];
    const result = runValidator(doc);
    expect(result.status).toBe(1);
    expect(result.output).toMatch(/duplicate (bom-ref|purl)/);
  }, 60_000);

  it('rejects a dependency edge that names no component', () => {
    const doc = fresh();
    const dependencies = doc.dependencies as { ref: string; dependsOn: string[] }[];
    doc.dependencies = [
      { ...dependencies[0], dependsOn: [...dependencies[0].dependsOn, 'pkg:cargo/not-a-crate@9.9.9'] },
      ...dependencies.slice(1),
    ];
    const result = runValidator(doc);
    expect(result.status).toBe(1);
    expect(result.output).toContain('names no component');
  }, 60_000);

  it('rejects a wall-clock timestamp, which would break reproducibility', () => {
    const doc = fresh();
    (doc.metadata as Record<string, unknown>).timestamp = '2026-07-27T00:00:00Z';
    const result = runValidator(doc);
    expect(result.status).toBe(1);
    expect(result.output).toContain('reproducible');
  }, 60_000);

  it('rejects a component that ships with no licence and no recorded blocker', () => {
    const doc = fresh();
    const components = doc.components as Record<string, unknown>[];
    // A component that HAS a licence today, stripped of it. It is not in UNRESOLVED.md, so silence
    // about it is exactly the gap the checker exists to refuse.
    const victim = components.find((c) => Array.isArray(c.licenses) && c.licenses.length > 0);
    expect(victim, 'the SBOM should contain at least one licensed component').toBeDefined();
    doc.components = components.map((c) => (c === victim ? { ...c, licenses: undefined } : c));
    const result = runValidator(doc);
    expect(result.status).toBe(1);
    expect(result.output).toContain('declares no licence');
  }, 60_000);

  it('rejects an SBOM that has drifted from the licence index', () => {
    const doc = fresh();
    const components = doc.components as Record<string, unknown>[];
    doc.components = components.slice(0, components.length - 1);
    const result = runValidator(doc);
    expect(result.status).toBe(1);
    expect(result.output).toContain('is in the licence index but not in the SBOM');
  }, 60_000);
});

describe('line endings are pinned for the files whose bytes are the contract', () => {
  // `core.autocrlf=true` is the Git-for-Windows installer default and the default on GitHub's
  // `windows-latest` runners — which is the runner the licensing gates run on. Without an explicit
  // attribute, git rewrites LF to CRLF on checkout for anything it detects as text, and every
  // byte-comparing gate here then fails on a *fresh clone* of a correct repository.
  //
  // Measured before the fix, on a simulated checkout: NOTICE gained 657 CRLF pairs,
  // `sbom.cdx.json` 17,452, `THIRD_PARTY_LICENSES.txt` 22,478, and the vendored CycloneDX schema's
  // SHA-256 moved from 067f7824… to e03d2164… — meaning `check:sbom-schema` would have reported an
  // untouched official schema as tampered with.
  //
  // This asks git for the *effective* attribute rather than grepping `.gitattributes`, so it tests
  // the behaviour and not the spelling.
  const attr = (name: string, path: string): string => {
    const result = spawnSync('git', ['check-attr', name, '--', path], { cwd: ROOT, encoding: 'utf8' });
    return result.stdout.trim().split(': ').pop() ?? '';
  };

  it('forces LF for every generated compliance artifact', () => {
    for (const path of [
      'NOTICE',
      'sbom.cdx.json',
      'licensing/THIRD_PARTY_LICENSES.txt',
      'licensing/third-party-index.json',
      'licensing/UNRESOLVED.md',
      'licensing/bundle-closure.json',
    ]) {
      expect(attr('text', path), `${path} must be declared text`).toBe('set');
      expect(attr('eol', path), `${path} must be pinned to LF`).toBe('lf');
    }
  });

  it('forbids any conversion of the checksummed vendored schemas', () => {
    // `-text` rather than `eol=lf`: these are hashed against PROVENANCE.json and must round-trip
    // exactly as vendored, so git is told not to transform them in either direction.
    for (const file of ['bom-1.5.schema.json', 'spdx.schema.json', 'jsf-0.82.schema.json']) {
      expect(attr('text', `schemas/cyclonedx/${file}`), `${file} must not be eol-converted`).toBe('unset');
    }
  });

  it('has no CRLF in any artifact as it stands', () => {
    for (const path of ['NOTICE', 'sbom.cdx.json', 'licensing/THIRD_PARTY_LICENSES.txt']) {
      const raw = readFileSync(resolve(ROOT, path), 'utf8');
      expect(raw.includes('\r\n'), `${path} contains CRLF`).toBe(false);
    }
  });
});

describe('control bytes in source are found, not rendered away', () => {
  it('detects the exact corruption that made scripts/gen_sbom.mjs a binary file to git', () => {
    // The generator joined a package name to its version with a literal NUL typed into the source.
    // It worked, and it made `git diff` report `Bin 7983 -> 7984 bytes` — so every change to the
    // SBOM generator was unreviewable and `grep` skipped the file. It is also invisible on screen:
    // editors and terminals render a control byte as a space, so the corrupt line and the correct
    // one look identical.
    const corrupt = Buffer.from('const key = `${name}\0${version}`;\n', 'utf8');
    expect(findControlByteOffsets(corrupt)).toEqual([20]);

    // Escape sequences in source are text; the whole point is to write `\0`, not the byte.
    const clean = Buffer.from('const key = `${name}\\0${version}`;\n', 'utf8');
    expect(findControlByteOffsets(clean)).toEqual([]);
  });

  it('treats tab, newline and carriage return as text, and DEL and ESC as not', () => {
    expect(findControlByteOffsets(Buffer.from('a\tb\r\nc\n', 'utf8'))).toEqual([]);
    expect(findControlByteOffsets(Buffer.from([0x61, 0x7f, 0x62]))).toEqual([1]);
    expect(findControlByteOffsets(Buffer.from([0x61, 0x1b, 0x5b, 0x30, 0x6d]))).toEqual([1]);
  });

  it('caps its report rather than listing every byte of a genuinely binary file', () => {
    const many = Buffer.alloc(100); // 100 NULs
    expect(findControlByteOffsets(many)).toHaveLength(5);
    expect(findControlByteOffsets(many, 2)).toHaveLength(2);
  });

  it('covers the file types the generators are written in', () => {
    // A gate that does not scan `.mjs` would not have caught the bug it exists for.
    for (const ext of ['.mjs', '.ts', '.tsx', '.rs', '.json', '.md', '.yml']) {
      expect(TEXT_EXTENSIONS.has(ext), `${ext} must be scanned`).toBe(true);
    }
  });
});
