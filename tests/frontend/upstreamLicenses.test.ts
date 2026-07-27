import { describe, it, expect } from 'vitest';
import { createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  MATERIAL_TYPES,
  MUTABLE_REFS,
  PROVENANCE_KINDS,
  isSafeStorePath,
  loadUpstreamStore,
  parseRawUrl,
  storeProvenancePath,
} from '../../scripts/lib/upstream_licenses.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const STORE = resolve(ROOT, 'licensing', 'upstream');

const sha256 = (buf: Buffer | string): string => createHash('sha256').update(buf).digest('hex');

// ---- the shape of the manifest, as the tests build and mutate it --------------------------------

interface SourceEntry {
  id: string;
  url: string;
  repository: string;
  commit: string;
  pathInRepo: string;
  filename: string;
  materialType: string;
  spdx: string | null;
  bytes: number;
  sha256: string;
  retrieved: string;
}
interface ProvenanceEntry {
  kind: string;
  repository: string;
  commit: string;
  tag: string | null;
  evidence: string[];
  componentRepository?: string;
  componentCommit?: string;
  componentTag?: string;
  justification?: string;
}
interface ComponentEntry {
  purl: string;
  name: string;
  version: string;
  ecosystem: string;
  declaredSpdx: string;
  provenance: ProvenanceEntry;
  sources: string[];
  material?: string;
}
interface BlockedEntry {
  purl: string;
  name: string;
  version: string;
  ecosystem: string;
  declaredSpdx: string;
  repository: string;
  commit: string;
  tag: string | null;
  investigated: string;
  evidence: string[];
}
interface Manifest {
  $comment: string;
  schemaVersion: number;
  sources: SourceEntry[];
  components: ComponentEntry[];
  blocked: BlockedEntry[];
}

const COMMIT_A = '1111111111111111111111111111111111111111';
const LICENCE_BYTES = 'MIT License\n\nCopyright (c) 2020 Acme\n';

/**
 * A minimal but genuinely valid store, so each test can break exactly one thing.
 *
 * Written to a temp directory rather than asserted against the real one: a test that can only be
 * expressed as "the committed store happens to be fine today" cannot show the checks fire, and a
 * validator that never rejects anything is indistinguishable from one that returns true.
 */
function baseManifest(): Manifest {
  const id = `github.com/acme/widget/${COMMIT_A}/LICENSE`;
  return {
    $comment: 'test fixture',
    schemaVersion: 1,
    sources: [
      {
        id,
        url: `https://raw.githubusercontent.com/acme/widget/${COMMIT_A}/LICENSE`,
        repository: 'https://github.com/acme/widget',
        commit: COMMIT_A,
        pathInRepo: 'LICENSE',
        filename: 'LICENSE',
        materialType: 'licence-text',
        spdx: 'MIT',
        bytes: Buffer.byteLength(LICENCE_BYTES),
        sha256: sha256(LICENCE_BYTES),
        retrieved: '2026-07-27',
      },
    ],
    components: [
      {
        purl: 'pkg:cargo/widget@1.2.3',
        name: 'widget',
        version: '1.2.3',
        ecosystem: 'cargo',
        declaredSpdx: 'MIT',
        provenance: {
          kind: 'release-tree',
          repository: 'https://github.com/acme/widget',
          commit: COMMIT_A,
          tag: 'v1.2.3',
          evidence: ['the published .crate names this commit in .cargo_vcs_info.json'],
        },
        sources: [id],
      },
    ],
    blocked: [],
  };
}

interface Fixture {
  root: string;
  tracked: Set<string>;
}

/**
 * Materialise a manifest into a temp store. `files` overrides what actually lands on disk, which is
 * how the tampering, traversal and missing-file cases are built without touching the manifest.
 */
function writeStore(
  dir: string,
  manifest: Manifest,
  options: { files?: Record<string, string>; tracked?: string[]; escapeVia?: string } = {},
): Fixture {
  const store = join(dir, 'licensing', 'upstream');
  mkdirSync(store, { recursive: true });

  const files = options.files ?? { [manifest.sources[0].id]: LICENCE_BYTES };
  for (const [id, body] of Object.entries(files)) {
    const full = join(store, ...id.split('/'));
    mkdirSync(dirname(full), { recursive: true });
    writeFileSync(full, body);
  }
  if (options.escapeVia) {
    const outside = join(dir, 'outside');
    mkdirSync(outside, { recursive: true });
    writeFileSync(join(outside, 'LICENSE'), LICENCE_BYTES);
    // A directory junction on Windows, a symlink elsewhere. Either way the file below it passes
    // `lstat().isFile()` and only `realpath` containment can catch that it is not in the store.
    const link = join(store, ...options.escapeVia.split('/'));
    mkdirSync(dirname(link), { recursive: true });
    symlinkSync(outside, link, 'junction');
  }
  writeFileSync(join(store, 'sources.json'), `${JSON.stringify(manifest, null, 2)}\n`);
  return {
    root: dir,
    tracked: new Set(
      (options.tracked ?? Object.keys(files)).map((id) => `licensing/upstream/${id}`),
    ),
  };
}

function withStore<T>(
  build: (manifest: Manifest) => { manifest: Manifest; options?: Parameters<typeof writeStore>[2] },
  run: (fixture: Fixture) => T,
): T {
  const dir = mkdtempSync(join(tmpdir(), 'anima-upstream-'));
  try {
    const { manifest, options } = build(baseManifest());
    return run(writeStore(dir, manifest, options));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/** Load a fixture and return the thrown message, or `null` when it loaded cleanly. */
function rejection(
  build: (manifest: Manifest) => { manifest: Manifest; options?: Parameters<typeof writeStore>[2] },
): string | null {
  return withStore(build, (fixture) => {
    try {
      loadUpstreamStore(fixture.root, { tracked: fixture.tracked });
      return null;
    } catch (e) {
      return e instanceof Error ? e.message : String(e);
    }
  });
}

// ---- pure helpers ------------------------------------------------------------------------------

describe('a pinned URL is one that cannot move', () => {
  it('accepts a raw URL pinned to a 40-hex commit and takes it apart', () => {
    const parsed = parseRawUrl(`https://raw.githubusercontent.com/gfx-rs/wgpu/${'a'.repeat(40)}/LICENSE.MIT`);
    expect(parsed).toEqual({ owner: 'gfx-rs', repo: 'wgpu', commit: 'a'.repeat(40), pathInRepo: 'LICENSE.MIT' });

    // A path with directories in it is normal — gpu-alloc keeps its texts in `license/`.
    expect(parseRawUrl(`https://raw.githubusercontent.com/z/gpu-alloc/${'b'.repeat(40)}/license/MIT`)?.pathInRepo)
      .toBe('license/MIT');
  });

  it('rejects every mutable ref, because a branch is a promise nobody made', () => {
    // The rule is "the ref segment is a 40-hex commit", not a blocklist of branch names — a
    // blocklist is a list someone eventually gets around by naming a branch `release`.
    for (const ref of [...MUTABLE_REFS, 'release', 'v1.2.3', 'refs/heads/main', 'a'.repeat(39), 'A'.repeat(40)]) {
      expect(parseRawUrl(`https://raw.githubusercontent.com/o/r/${ref}/LICENSE`), ref).toBeNull();
    }
  });

  it('rejects hosts that are not the raw content host, and URLs with no file', () => {
    for (const url of [
      `https://github.com/o/r/blob/${'a'.repeat(40)}/LICENSE`,
      `http://raw.githubusercontent.com/o/r/${'a'.repeat(40)}/LICENSE`,
      `https://raw.githubusercontent.example.com/o/r/${'a'.repeat(40)}/LICENSE`,
      `https://raw.githubusercontent.com/o/r/${'a'.repeat(40)}/`,
      `https://raw.githubusercontent.com/o/r/${'a'.repeat(40)}`,
    ]) {
      expect(parseRawUrl(url), url).toBeNull();
    }
  });
});

describe('store paths are checked before they touch the filesystem', () => {
  it('accepts the shape the layout produces', () => {
    expect(isSafeStorePath(`github.com/acme/widget/${COMMIT_A}/LICENSE`)).toBe(true);
    expect(isSafeStorePath(`github.com/z/gpu-alloc/${COMMIT_A}/license/MIT`)).toBe(true);
  });

  it('refuses traversal, absolute paths and separators that only Windows honours', () => {
    for (const bad of [
      '../../../etc/passwd',
      'github.com/acme/widget/../../../etc/passwd',
      '/etc/passwd',
      'C:/Windows/System32/drivers/etc/hosts',
      'c:/x',
      'github.com\\acme\\widget\\LICENSE',
      'github.com/acme//LICENSE',
      'github.com/./LICENSE',
      'github.com/../LICENSE',
      '',
      'github.com/acme/LICENSE\u0000.txt',
    ]) {
      expect(isSafeStorePath(bad), JSON.stringify(bad)).toBe(false);
    }
  });
});

// ---- the validator, exercised by breaking one thing at a time -----------------------------------

describe('the store loads only when every claim about it holds', () => {
  it('accepts a store whose manifest is true', () => {
    withStore(
      (manifest) => ({ manifest }),
      (fixture) => {
        const store = loadUpstreamStore(fixture.root, { tracked: fixture.tracked });
        expect(store.sources.size).toBe(1);
        expect(store.byPurl.get('pkg:cargo/widget@1.2.3')?.resolvedSources[0].filename).toBe('LICENSE');
        expect(store.blockedByPurl.size).toBe(0);
      },
    );
  });

  it('rejects bytes that were edited after they were vendored', () => {
    // The failure this module exists for. A tampered licence is present, plausible and wrong, and
    // nothing downstream would notice: the bundle would reproduce it under the right component name.
    const message = rejection((manifest) => ({
      manifest,
      options: { files: { [manifest.sources[0].id]: `${LICENCE_BYTES}Copyright (c) 2020 Someone Else\n` } },
    }));
    expect(message).toContain('sha256 is');
    expect(message).toContain('manifest says');
  });

  it('rejects a byte length that disagrees with the file, even when the hash field was updated too', () => {
    const body = 'MIT License\n';
    const message = rejection((manifest) => ({
      manifest: {
        ...manifest,
        sources: [{ ...manifest.sources[0], sha256: sha256(body) }],
      },
      options: { files: { [manifest.sources[0].id]: body } },
    }));
    expect(message).toContain('bytes, manifest says');
  });

  it('rejects a source file that is not there', () => {
    const message = rejection((manifest) => ({ manifest, options: { files: {}, tracked: [manifest.sources[0].id] } }));
    expect(message).toContain('no such file in the store');
  });

  it('rejects a source that is not tracked by git', () => {
    // Untracked is not "present": it vanishes on a fresh clone, and every gate that reads the store
    // would then pass against a file nobody can obtain.
    const message = rejection((manifest) => ({ manifest, options: { tracked: [] } }));
    expect(message).toContain('not tracked by git');
  });

  it('rejects a directory standing where a licence file should be', () => {
    // Same branch a symlink takes: `lstat().isFile()` is false for both, so a link out of the store
    // never reaches a read.
    const message = withStore(
      (manifest) => ({ manifest, options: { files: {}, tracked: [manifest.sources[0].id] } }),
      (fixture) => {
        mkdirSync(join(fixture.root, 'licensing', 'upstream', 'github.com', 'acme', 'widget', COMMIT_A, 'LICENSE'), {
          recursive: true,
        });
        try {
          loadUpstreamStore(fixture.root, { tracked: fixture.tracked });
          return null;
        } catch (e) {
          return e instanceof Error ? e.message : String(e);
        }
      },
    );
    expect(message).toContain('is not a regular file');
  });

  it('rejects a file reached through a link that escapes the store', () => {
    // A junction (Windows) or symlink (elsewhere) *above* the file: the file itself passes every
    // name check and `lstat().isFile()`, and only `realpath` containment sees that its bytes live
    // outside the store entirely. Creating a directory junction needs no privilege on Windows,
    // which is why the escape is built here rather than asserted against a hand-made entry.
    const message = rejection((manifest) => ({
      manifest,
      options: { files: {}, tracked: [manifest.sources[0].id], escapeVia: `github.com/acme/widget/${COMMIT_A}` },
    }));
    expect(message).toContain('resolves outside the store');
  });

  it('rejects a store path that does not match its own url', () => {
    const message = rejection((manifest) => ({
      manifest: {
        ...manifest,
        sources: [{ ...manifest.sources[0], url: `https://raw.githubusercontent.com/acme/other/${COMMIT_A}/LICENSE` }],
      },
    }));
    expect(message).toContain('store path does not match its url');
  });

  it('rejects a url pinned to a branch rather than a commit', () => {
    const message = rejection((manifest) => ({
      manifest: {
        ...manifest,
        sources: [{ ...manifest.sources[0], url: 'https://raw.githubusercontent.com/acme/widget/main/LICENSE' }],
      },
    }));
    expect(message).toContain('pinned to a 40-hex commit');
  });

  it('rejects a tag that names a moving ref', () => {
    for (const tag of ['main', 'master', 'trunk', 'HEAD']) {
      const message = rejection((manifest) => ({
        manifest: {
          ...manifest,
          components: [{ ...manifest.components[0], provenance: { ...manifest.components[0].provenance, tag } }],
        },
      }));
      expect(message, tag).toContain('names a moving ref');
    }
  });

  it('rejects a traversing store id before it is joined onto anything', () => {
    const message = rejection((manifest) => ({
      manifest: { ...manifest, sources: [{ ...manifest.sources[0], id: '../../../etc/passwd' }] },
      options: { files: {}, tracked: [] },
    }));
    expect(message).toContain('is not a safe relative store path');
  });

  it('rejects two mappings for one component', () => {
    const message = rejection((manifest) => ({
      manifest: { ...manifest, components: [manifest.components[0], { ...manifest.components[0] }] },
    }));
    expect(message).toContain('duplicate mapping for pkg:cargo/widget@1.2.3');
  });

  it('rejects two sources with one id', () => {
    const message = rejection((manifest) => ({
      manifest: { ...manifest, sources: [manifest.sources[0], { ...manifest.sources[0] }] },
    }));
    expect(message).toContain('duplicate source id');
  });

  it('rejects a purl that disagrees with the name and version beside it', () => {
    // The purl is the join key every downstream artifact uses. A mapping whose purl says one
    // version and whose fields say another attaches the wrong licence to a right-looking row.
    const message = rejection((manifest) => ({
      manifest: { ...manifest, components: [{ ...manifest.components[0], version: '9.9.9' }] },
    }));
    expect(message).toContain('purl does not match cargo widget@9.9.9');
  });

  it('rejects a scoped npm purl that is not encoded the way the inventory encodes it', () => {
    const message = rejection((manifest) => ({
      manifest: {
        ...manifest,
        components: [
          {
            ...manifest.components[0],
            purl: 'pkg:npm/@scope/thing@1.0.0',
            name: '@scope/thing',
            version: '1.0.0',
            ecosystem: 'npm',
          },
        ],
      },
    }));
    expect(message).toContain('purl does not match npm @scope/thing@1.0.0');
  });

  it('rejects a mapping whose commit disagrees with the source it names', () => {
    const message = rejection((manifest) => ({
      manifest: {
        ...manifest,
        components: [
          {
            ...manifest.components[0],
            provenance: { ...manifest.components[0].provenance, commit: '2'.repeat(40) },
          },
        ],
      },
    }));
    expect(message).toContain('but source');
    expect(message).toContain('is from');
  });

  it('rejects a release-tree mapping that reaches into another repository', () => {
    const message = rejection((manifest) => ({
      manifest: {
        ...manifest,
        components: [
          {
            ...manifest.components[0],
            provenance: { ...manifest.components[0].provenance, repository: 'https://github.com/acme/elsewhere' },
          },
        ],
      },
    }));
    expect(message).toContain('release-tree provenance names');
  });

  it('rejects a mapping that names a source the store does not have', () => {
    const message = rejection((manifest) => ({
      manifest: {
        ...manifest,
        components: [{ ...manifest.components[0], sources: [`github.com/acme/widget/${COMMIT_A}/NOTICE`] }],
      },
    }));
    expect(message).toContain('names unknown source');
  });

  it('rejects a vendored file no component maps', () => {
    // Not harmless: either a component left the graph and its text stayed behind, or a file was
    // added that no review ever tied to anything. Both are states where the store stops describing
    // what ships.
    const message = rejection((manifest) => ({
      manifest: { ...manifest, components: [] },
    }));
    expect(message).toContain('is vendored but no component maps it');
  });

  it('rejects an out-of-order manifest, so an insertion diffs as an insertion', () => {
    const second = `github.com/acme/widget/${COMMIT_A}/NOTICE`;
    const message = rejection((manifest) => ({
      manifest: {
        ...manifest,
        sources: [
          {
            ...manifest.sources[0],
            id: second,
            url: `https://raw.githubusercontent.com/acme/widget/${COMMIT_A}/NOTICE`,
            pathInRepo: 'NOTICE',
            filename: 'NOTICE',
            materialType: 'notice',
            spdx: null,
          },
          manifest.sources[0],
        ],
        components: [{ ...manifest.components[0], sources: [manifest.sources[0].id, second] }],
      },
      options: { files: { [manifest.sources[0].id]: LICENCE_BYTES, [second]: LICENCE_BYTES } },
    }));
    expect(message).toContain('not in byte order');
  });

  it('requires a written justification and the component\u2019s own revision for the escape hatch', () => {
    const message = rejection((manifest) => ({
      manifest: {
        ...manifest,
        components: [
          {
            ...manifest.components[0],
            provenance: { ...manifest.components[0].provenance, kind: 'project-repository' },
          },
        ],
      },
    }));
    expect(message).toContain('needs a written justification');
    expect(message).toContain('must name componentRepository');
    expect(message).toContain('must pin componentCommit');
  });

  it('refuses project-repository fields on an ordinary mapping', () => {
    // Otherwise the fields drift onto rows that never needed them and the escape hatch stops being
    // visible in review, which is the only thing keeping it narrow.
    const message = rejection((manifest) => ({
      manifest: {
        ...manifest,
        components: [
          {
            ...manifest.components[0],
            provenance: { ...manifest.components[0].provenance, justification: 'x'.repeat(100) },
          },
        ],
      },
    }));
    expect(message).toContain('only meaningful for project-repository provenance');
  });

  it('rejects a component recorded as both mapped and blocked', () => {
    const message = rejection((manifest) => ({
      manifest: {
        ...manifest,
        blocked: [
          {
            purl: 'pkg:cargo/widget@1.2.3',
            name: 'widget',
            version: '1.2.3',
            ecosystem: 'cargo',
            declaredSpdx: 'MIT',
            repository: 'https://github.com/acme/widget',
            commit: COMMIT_A,
            tag: null,
            investigated: '2026-07-27',
            evidence: ['searched'],
          },
        ],
      },
    }));
    expect(message).toContain('is both mapped and recorded as blocked');
  });

  it('rejects a blocked record with no evidence, so "unresolved" cannot mean "unexamined"', () => {
    const message = rejection((manifest) => ({
      manifest: {
        ...manifest,
        blocked: [
          {
            purl: 'pkg:cargo/other@1.0.0',
            name: 'other',
            version: '1.0.0',
            ecosystem: 'cargo',
            declaredSpdx: 'MIT',
            repository: 'https://github.com/acme/other',
            commit: COMMIT_A,
            tag: null,
            investigated: '2026-07-27',
            evidence: [],
          },
        ],
      },
    }));
    expect(message).toContain('evidence must be a non-empty list');
  });

  it('refuses a manifest whose schema version it does not understand', () => {
    const message = rejection((manifest) => ({ manifest: { ...manifest, schemaVersion: 2 } }));
    expect(message).toContain('this reader understands 1');
  });

  it('reports every problem at once rather than the first', () => {
    const message = rejection((manifest) => ({
      manifest: {
        ...manifest,
        sources: [{ ...manifest.sources[0], sha256: '0'.repeat(64), retrieved: 'yesterday' }],
      },
    }));
    expect(message).toContain('problem(s)');
    expect(message).toContain('retrieved must be an ISO date');
    expect(message).toContain('sha256 is');
  });
});

// ---- the committed store ------------------------------------------------------------------------

interface IndexTextEntry {
  id: string;
  filename: string;
  origin: 'installed' | 'upstream';
  provenance: string;
  sourceSha256: string;
  sourceBytes: number;
  textSha256: string;
  upstreamUrl?: string;
  upstreamCommit?: string;
  upstreamRef?: string | null;
  provenanceKind?: string;
  materialType?: string;
  retrieved?: string;
}
interface IndexComponentEntry {
  purl: string;
  name: string;
  version: string;
  distributed: boolean;
  spdx: string | null;
  spdxSource: string | null;
  texts: IndexTextEntry[];
}
interface VendoredEntry {
  purl: string;
  declaredSpdx: string;
  kind: string;
  repository: string;
  commit: string;
  ref: string | null;
  evidence: string[];
  files: { path: string; url: string; sha256: string; bytes: number }[];
}
interface LicenceIndex {
  counts: {
    distributed: number;
    texts: number;
    fromInstalledArtifact: number;
    fromVendoredUpstream: number;
    vendoredSources: number;
    vendoredRepositories: number;
    vendoredCommits: number;
    unresolved: number;
  };
  components: IndexComponentEntry[];
  vendored: VendoredEntry[];
  unresolved: { purl: string; name: string; investigated?: { evidence: string[] } }[];
}

const index = (): LicenceIndex =>
  JSON.parse(readFileSync(resolve(ROOT, 'licensing', 'third-party-index.json'), 'utf8')) as LicenceIndex;

describe('the committed store is what the committed artifacts describe', () => {
  it('loads clean, with git as the authority on what is tracked', () => {
    const store = loadUpstreamStore(ROOT);
    expect(store.sources.size).toBeGreaterThan(0);
    expect(store.byPurl.size).toBeGreaterThan(0);
    for (const source of store.sources.values()) {
      expect(source.raw.length, source.id).toBe(source.bytes);
      expect(sha256(source.raw), source.id).toBe(source.sha256);
      expect(MATERIAL_TYPES.has(source.materialType), source.materialType).toBe(true);
    }
    for (const component of store.byPurl.values()) {
      expect(PROVENANCE_KINDS.has(component.provenance.kind)).toBe(true);
      expect(component.provenance.evidence.length).toBeGreaterThan(0);
    }
  });

  it('keeps the escape hatch to the cases that argued for it', () => {
    // One today: `selectors`, whose own repository publishes no copy of the MPL. If a second
    // appears, that is a decision someone must make deliberately rather than notice later.
    const store = loadUpstreamStore(ROOT);
    const escaped = [...store.byPurl.values()].filter((c) => c.provenance.kind === 'project-repository');
    expect(escaped.map((c) => c.purl)).toEqual(['pkg:cargo/selectors@0.36.1']);
    for (const c of escaped) {
      expect(c.provenance.justification?.length ?? 0).toBeGreaterThan(200);
      expect(c.provenance.componentRepository).toBe('https://github.com/servo/stylo');
      expect(c.provenance.componentCommit).toMatch(/^[0-9a-f]{40}$/);
    }
  });

  it('resolves every component the index says it resolved, and only those', () => {
    const store = loadUpstreamStore(ROOT);
    const idx = index();
    const fromIndex = new Set(
      idx.components.filter((c) => c.texts.some((t) => t.origin === 'upstream')).map((c) => c.purl),
    );
    expect([...fromIndex].sort()).toEqual([...store.byPurl.keys()].sort());
    expect(idx.counts.fromVendoredUpstream).toBe(store.byPurl.size);
    expect(idx.vendored.map((v) => v.purl).sort()).toEqual([...fromIndex].sort());
  });

  it('never lets a vendored text shadow one the artifact itself carries', () => {
    // The rule that keeps the store from becoming a place inconvenient components go to be marked
    // resolved: a component with installed text must have no vendored text at all.
    for (const c of index().components) {
      const origins = new Set(c.texts.map((t) => t.origin));
      expect(origins.size <= 1, `${c.purl} mixes installed and vendored text`).toBe(true);
    }
  });

  it('gives every vendored text a re-fetchable, machine-independent provenance', () => {
    const idx = index();
    for (const c of idx.components) {
      for (const t of c.texts) {
        if (t.origin !== 'upstream') continue;
        expect(t.provenance.startsWith('licensing/upstream/'), `${c.purl} ${t.provenance}`).toBe(true);
        expect(t.provenance).not.toMatch(/^[A-Za-z]:/);
        expect(t.provenance).not.toContain('\\');
        // The store path is the tail of the URL, so the two cannot drift apart.
        expect(t.upstreamUrl).toBe(`https://raw.githubusercontent.com/${t.provenance.replace('licensing/upstream/github.com/', '')}`);
        expect(t.upstreamCommit).toMatch(/^[0-9a-f]{40}$/);
        expect(t.upstreamRef === null || (t.upstreamRef ?? '').length > 0).toBe(true);
        expect(t.sourceSha256).toMatch(/^[0-9a-f]{64}$/);
      }
    }
  });

  it('reproduces the vendored bytes it claims, from the files on disk', () => {
    for (const v of index().vendored) {
      for (const f of v.files) {
        const bytes = readFileSync(resolve(ROOT, f.path));
        expect(bytes.length, f.path).toBe(f.bytes);
        expect(sha256(bytes), f.path).toBe(f.sha256);
      }
    }
  });

  it('supplies the SPDX expression for the component that has no manifest to declare one', () => {
    // `@oxc-project/runtime` is compiled into `dist/` by rolldown and never installed, so nothing
    // local can declare its licence. Before the store it was the one component in the bill with no
    // SPDX at all.
    const oxc = index().components.find((c) => c.name === '@oxc-project/runtime');
    expect(oxc?.spdx).toBe('MIT');
    expect(oxc?.spdxSource).toBe('upstream-manifest');
    expect(oxc?.texts[0]?.origin).toBe('upstream');
  });

  it('states an evidence chain for every vendored component, not a category', () => {
    for (const v of index().vendored) {
      expect(v.evidence.length, v.purl).toBeGreaterThan(0);
      expect(v.evidence.join(' ').length, v.purl).toBeGreaterThan(60);
      expect(v.commit).toMatch(/^[0-9a-f]{40}$/);
    }
  });

  it('is byte-ordered everywhere it is listed, so regeneration is reproducible', () => {
    const idx = index();
    const purls = idx.vendored.map((v) => v.purl);
    expect(purls).toEqual([...purls].sort());

    const manifest = JSON.parse(readFileSync(resolve(STORE, 'sources.json'), 'utf8')) as Manifest;
    expect(manifest.sources.map((s) => s.id)).toEqual([...manifest.sources.map((s) => s.id)].sort());
    expect(manifest.components.map((c) => c.purl)).toEqual([...manifest.components.map((c) => c.purl)].sort());
    // Loading twice must produce the same thing in the same order: the generated bundle's ordering
    // is derived from this, and a Map that enumerated differently would move bytes in a committed
    // artifact for no reason a reviewer could see.
    const shape = (): string =>
      JSON.stringify([...loadUpstreamStore(ROOT).byPurl].map(([purl, c]) => [purl, c.sources]));
    expect(shape()).toBe(shape());
  });

  it('carries no wall-clock time into the shipping document', () => {
    // `retrieved` is a committed constant and appears in the index; the bundle that ships must not
    // carry a date at all, or the freshness gate would report the calendar rather than the graph.
    const bundle = readFileSync(resolve(ROOT, 'licensing', 'THIRD_PARTY_LICENSES.txt'), 'utf8');
    const header = bundle.slice(0, bundle.indexOf('COMPONENT INDEX'));
    expect(header).not.toMatch(/\b20\d\d-\d\d-\d\dT/);
    expect(header).toContain('VENDORED UPSTREAM SOURCES');
  });

  it('records what was searched for every row it still cannot close', () => {
    const idx = index();
    const store = loadUpstreamStore(ROOT);
    for (const u of idx.unresolved) {
      const blocked = store.blockedByPurl.get(u.purl);
      expect(blocked, `${u.purl} is unresolved with no record of a search`).toBeDefined();
      expect(u.investigated?.evidence.length ?? 0).toBeGreaterThan(2);
    }
    // And nothing is recorded as blocked that is not actually unresolved.
    const unresolvedPurls = new Set(idx.unresolved.map((u) => u.purl));
    for (const purl of store.blockedByPurl.keys()) {
      expect(unresolvedPurls.has(purl), `${purl} is recorded as blocked but is not unresolved`).toBe(true);
    }
  });

  it('agrees with itself about how many components got a text, and from where', () => {
    const idx = index();
    const installed = idx.components.filter((c) => c.texts.some((t) => t.origin === 'installed')).length;
    const vendored = idx.components.filter((c) => c.texts.some((t) => t.origin === 'upstream')).length;
    expect(idx.counts.fromInstalledArtifact).toBe(installed);
    expect(idx.counts.fromVendoredUpstream).toBe(vendored);
    expect(installed + vendored + idx.counts.unresolved).toBe(idx.counts.distributed);

    const files = new Set(idx.vendored.flatMap((v) => v.files.map((f) => f.path)));
    expect(idx.counts.vendoredSources).toBe(files.size);
    expect(idx.counts.vendoredCommits).toBe(new Set(idx.vendored.map((v) => v.commit)).size);
  });

  it('names the store in the paths the bundle publishes', () => {
    const bundle = readFileSync(resolve(ROOT, 'licensing', 'THIRD_PARTY_LICENSES.txt'), 'utf8');
    for (const v of index().vendored.slice(0, 5)) {
      expect(bundle, v.purl).toContain(v.commit);
      for (const f of v.files) expect(bundle, f.path).toContain(f.sha256);
    }
    expect(storeProvenancePath('github.com/a/b/c/LICENSE')).toBe('licensing/upstream/github.com/a/b/c/LICENSE');
  });
});
