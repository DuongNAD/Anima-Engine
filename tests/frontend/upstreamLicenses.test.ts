import { describe, it, expect } from 'vitest';
import { createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  APACHE_UNFILLED_COPYRIGHT,
  ELECTABLE_LICENCES,
  MATERIAL_TYPES,
  MUTABLE_REFS,
  PROVENANCE_KINDS,
  disjunctionOperands,
  electedText,
  electionProblems,
  isApache2Text,
  isSafeStorePath,
  loadUpstreamStore,
  parseRawUrl,
  resolveElections,
  storeProvenancePath,
  type Election,
  type ElectionComponent,
  type PooledText,
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
interface ElectionEntry {
  purl: string;
  name: string;
  version: string;
  ecosystem: string;
  declaredSpdx: string;
  chosen: string;
  textSha256: string;
  attribution: { name: string; version: string; authors: string[]; repository: string };
  commit: string;
  tag: string | null;
  investigated: string;
  evidence: string[];
  [extra: string]: unknown;
}
interface Manifest {
  $comment: string;
  schemaVersion: number;
  sources: SourceEntry[];
  components: ComponentEntry[];
  blocked: BlockedEntry[];
  elections: ElectionEntry[];
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
    schemaVersion: 2,
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
    elections: [],
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
    // 1 is the store before elections existed; 3 is one this reader has never seen. Both refused.
    for (const schemaVersion of [1, 3]) {
      const message = rejection((manifest) => ({ manifest: { ...manifest, schemaVersion } }));
      expect(message, String(schemaVersion)).toContain('this reader understands 2');
    }
  });

  it('refuses a manifest that has no elections array, rather than reading it as empty', () => {
    const message = rejection((manifest) => {
      const withoutElections: Partial<Manifest> = { ...manifest };
      delete withoutElections.elections;
      return { manifest: withoutElections as Manifest };
    });
    expect(message).toContain('sources, components, blocked and elections arrays');
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

// ---- elections: the one place a licence is chosen on a component's behalf ------------------------

/**
 * A stand-in for the canonical Apache-2.0 text: the title lines, the end of the terms, and the
 * appendix with its placeholder unfilled — the three things `isApache2Text` and `electedText` read.
 */
const APACHE_TEXT = [
  '                              Apache License',
  '                        Version 2.0, January 2004',
  '                     http://www.apache.org/licenses/',
  '',
  'TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION',
  '',
  '   (the terms, elided in this fixture)',
  '',
  'END OF TERMS AND CONDITIONS',
  '',
  'APPENDIX: How to apply the Apache License to your work.',
  '',
  `   ${APACHE_UNFILLED_COPYRIGHT}`,
  '',
].join('\n');
/** The same licence, applied by someone: the appendix names a holder. */
const APACHE_FILLED = APACHE_TEXT.replace(APACHE_UNFILLED_COPYRIGHT, 'Copyright 2020 Acme Corp');
/** The same licence without its appendix, as several crates ship it. */
const APACHE_NO_APPENDIX = APACHE_TEXT.slice(0, APACHE_TEXT.indexOf('APPENDIX'));

const ELECTED_PURL = 'pkg:cargo/nolicence@0.1.0';

function electionEntry(overrides: Partial<ElectionEntry> = {}): ElectionEntry {
  return {
    purl: ELECTED_PURL,
    name: 'nolicence',
    version: '0.1.0',
    ecosystem: 'cargo',
    declaredSpdx: 'MIT OR Apache-2.0',
    chosen: 'Apache-2.0',
    textSha256: sha256(APACHE_TEXT),
    attribution: {
      name: 'nolicence',
      version: '0.1.0',
      authors: ['Someone <someone@example.com>'],
      repository: 'https://github.com/acme/nolicence',
    },
    commit: COMMIT_A,
    tag: null,
    investigated: '2026-11-19',
    evidence: ['the published crate and its release commit carry no licence file and no notice'],
    ...overrides,
  };
}

/** The validated form, for the inventory-side functions that take what the loader returns. */
const asElection = (e: ElectionEntry): Election => e as unknown as Election;

function electedComponent(overrides: Partial<ElectionComponent> = {}): ElectionComponent {
  return {
    purl: ELECTED_PURL,
    declaredLicense: 'MIT OR Apache-2.0',
    declaredLicenseFile: null,
    repository: 'https://github.com/acme/nolicence',
    authors: ['Someone <someone@example.com>'],
    distributed: true,
    ...overrides,
  };
}

function pooled(text: string, carriers: number, prefix = 'c'): PooledText {
  return {
    textSha256: sha256(text),
    text,
    purls: new Set(Array.from({ length: carriers }, (_, i) => `pkg:cargo/${prefix}${i}@1.0.0`)),
  };
}

/** Load a store carrying the given elections and return the rejection, or `null` if it loaded. */
function electionRejection(elections: ElectionEntry[], extra: Partial<Manifest> = {}): string | null {
  return rejection((manifest) => ({ manifest: { ...manifest, ...extra, elections } }));
}

describe('a declared expression is a pure disjunction or it is nothing', () => {
  it('reads the operands of `A OR B [OR C]` made of whole SPDX identifiers', () => {
    expect(disjunctionOperands('MIT OR Apache-2.0')).toEqual(['MIT', 'Apache-2.0']);
    expect(disjunctionOperands('Apache-2.0 OR MIT')).toEqual(['Apache-2.0', 'MIT']);
    expect(disjunctionOperands('MIT OR Apache-2.0 OR Zlib')).toEqual(['MIT', 'Apache-2.0', 'Zlib']);
  });

  it('refuses AND, WITH, parentheses, an or-later suffix and every malformed spelling', () => {
    for (const expr of [
      'MIT AND Apache-2.0',
      'Apache-2.0 WITH LLVM-exception OR MIT',
      'MIT OR Apache-2.0 WITH LLVM-exception',
      '(MIT OR Apache-2.0)',
      'MIT OR (Apache-2.0 AND Zlib)',
      '(MIT AND Zlib) OR Apache-2.0',
      'MIT OR Apache-2.0+',
      'MIT or Apache-2.0',
      'MIT/Apache-2.0',
      'MIT  OR Apache-2.0',
      ' MIT OR Apache-2.0',
      'MIT OR MIT',
      'MIT OR NotARealLicence',
      'Apache-2.0',
      'MIT OR',
      '',
    ]) {
      expect(disjunctionOperands(expr), JSON.stringify(expr)).toBeNull();
    }
  });

  it('recognises the Apache-2.0 text by content, not by filename', () => {
    expect(isApache2Text(APACHE_TEXT)).toBe(true);
    expect(isApache2Text(APACHE_NO_APPENDIX)).toBe(true);
    expect(isApache2Text(LICENCE_BYTES)).toBe(false);
    expect(isApache2Text('Apache License\nVersion 2.0, January 2004\n(truncated)\n')).toBe(false);
    expect([...ELECTABLE_LICENCES]).toEqual(['Apache-2.0']);
  });
});

describe('an election loads only when its manifest entry is narrow', () => {
  it('accepts a well-formed election', () => {
    withStore(
      (manifest) => ({ manifest: { ...manifest, elections: [electionEntry()] } }),
      (fixture) => {
        const store = loadUpstreamStore(fixture.root, { tracked: fixture.tracked });
        expect(store.electionsByPurl.get(ELECTED_PURL)?.chosen).toBe('Apache-2.0');
      },
    );
  });

  it('rejects a conjunction, because AND means both licences apply and neither can be chosen', () => {
    expect(electionRejection([electionEntry({ declaredSpdx: 'MIT AND Apache-2.0' })])).toContain(
      'is not a pure disjunction',
    );
  });

  it('rejects a WITH exception, which changes the terms of the operand it attaches to', () => {
    expect(
      electionRejection([electionEntry({ declaredSpdx: 'Apache-2.0 WITH LLVM-exception OR MIT' })]),
    ).toContain('is not a pure disjunction');
  });

  it('rejects parentheses, even ones that change nothing', () => {
    for (const declaredSpdx of ['(MIT OR Apache-2.0)', 'MIT OR (Apache-2.0 AND Zlib)']) {
      expect(electionRejection([electionEntry({ declaredSpdx })]), declaredSpdx).toContain(
        'is not a pure disjunction',
      );
    }
  });

  it('rejects a disjunction that does not offer Apache-2.0 as a whole operand', () => {
    const message = electionRejection([electionEntry({ declaredSpdx: 'MIT OR Zlib' })]);
    expect(message).toContain('does not offer Apache-2.0 as a whole operand');
    expect(message).toContain('is not one of the operands');
  });

  it('rejects electing anything but Apache-2.0, even an operand that is on offer', () => {
    expect(electionRejection([electionEntry({ chosen: 'MIT' })])).toContain(
      'only Apache-2.0 is supported',
    );
  });

  it('rejects an election for a component that is already mapped to vendored text', () => {
    const message = electionRejection([
      electionEntry({
        purl: 'pkg:cargo/widget@1.2.3',
        name: 'widget',
        version: '1.2.3',
        attribution: { ...electionEntry().attribution, name: 'widget', version: '1.2.3' },
      }),
    ]);
    expect(message).toContain('is already mapped or recorded as blocked');
  });

  it('rejects an election for a component also recorded as blocked', () => {
    const blocked: BlockedEntry = {
      purl: ELECTED_PURL,
      name: 'nolicence',
      version: '0.1.0',
      ecosystem: 'cargo',
      declaredSpdx: 'MIT OR Apache-2.0',
      repository: 'https://github.com/acme/nolicence',
      commit: COMMIT_A,
      tag: null,
      investigated: '2026-11-18',
      evidence: ['searched'],
    };
    expect(electionRejection([electionEntry()], { blocked: [blocked] })).toContain(
      'is already mapped or recorded as blocked',
    );
  });

  it('rejects an unknown key instead of ignoring it', () => {
    expect(electionRejection([electionEntry({ choosen: 'Apache-2.0' })])).toContain('unknown key "choosen"');
  });

  it('rejects an npm election, whose attribution nothing yet checks', () => {
    expect(
      electionRejection([electionEntry({ purl: 'pkg:npm/nolicence@0.1.0', ecosystem: 'npm' })]),
    ).toContain('only cargo components can be elected');
  });

  it('rejects an attribution that names another component, or no author', () => {
    const base = electionEntry();
    expect(
      electionRejection([electionEntry({ attribution: { ...base.attribution, version: '9.9.9' } })]),
    ).toContain('attribution must name the component it attributes');
    expect(
      electionRejection([electionEntry({ attribution: { ...base.attribution, authors: [] } })]),
    ).toContain('attribution.authors must be');
  });

  it('rejects a text hash that is not a SHA-256, and an election with no evidence', () => {
    const message = electionRejection([electionEntry({ textSha256: 'apache', evidence: [] })]);
    expect(message).toContain('textSha256 must be 64 lowercase hex characters');
    expect(message).toContain('evidence must be a non-empty list');
  });

  it('rejects elections out of byte order', () => {
    const later = electionEntry({
      purl: 'pkg:cargo/zz@0.1.0',
      name: 'zz',
      attribution: { ...electionEntry().attribution, name: 'zz' },
    });
    expect(electionRejection([later, electionEntry()])).toContain('elections: not in byte order');
  });
});

describe('an election applies only to the component as it is actually installed', () => {
  const election = asElection(electionEntry());

  it('applies to a distributed component that declares the expression and ships no licence file', () => {
    expect(electionProblems(election, electedComponent(), [])).toEqual([]);
  });

  it('refuses the moment the installed artifact carries a licence file: installed text wins', () => {
    for (const filename of ['LICENSE-MIT', 'COPYRIGHT', 'NOTICE']) {
      expect(electionProblems(election, electedComponent(), [{ filename }]).join('\n'), filename).toContain(
        'Installed text wins',
      );
    }
  });

  it('refuses a component that does not ship', () => {
    expect(electionProblems(election, electedComponent({ distributed: false }), []).join('\n')).toContain(
      'installed but not distributed',
    );
  });

  it('refuses when the component declares a different expression than the election recorded', () => {
    expect(
      electionProblems(election, electedComponent({ declaredLicense: 'MIT OR Apache-2.0 OR Zlib' }), []).join('\n'),
    ).toContain('the component declares');
  });

  it('refuses a component that points at a licence file of its own', () => {
    expect(
      electionProblems(election, electedComponent({ declaredLicenseFile: 'TERMS.txt' }), []).join('\n'),
    ).toContain('declares a licence file');
  });

  it('refuses an attribution that is not what the manifest declares', () => {
    expect(
      electionProblems(election, electedComponent({ authors: ['Someone Else'] }), []).join('\n'),
    ).toContain('is not what the manifest declares');
    expect(
      electionProblems(election, electedComponent({ repository: 'https://github.com/other/fork' }), []).join('\n'),
    ).toContain("is not the manifest's");
  });
});

describe('the elected bytes are the Apache-2.0 text the inventory already ships most widely', () => {
  const election = asElection(electionEntry());

  it('picks the most widely shipped Apache-2.0 text, ignoring a more common text that is not Apache', () => {
    const picked = electedText(election, [
      pooled(LICENCE_BYTES, 50, 'mit'),
      pooled(APACHE_TEXT, 20, 'apache'),
      pooled(APACHE_NO_APPENDIX, 5, 'bare'),
    ]);
    expect(picked.problems).toEqual([]);
    expect(picked.text).toBe(APACHE_TEXT);
  });

  it('refuses a pin that is not the most widely shipped copy, so a less common variant cannot be cherry-picked', () => {
    const picked = electedText(asElection(electionEntry({ textSha256: sha256(APACHE_NO_APPENDIX) })), [
      pooled(APACHE_TEXT, 20, 'apache'),
      pooled(APACHE_NO_APPENDIX, 5, 'bare'),
    ]);
    expect(picked.text).toBeUndefined();
    expect(picked.problems.join('\n')).toContain('re-pin');
  });

  it('refuses a copy whose appendix names a holder, which would attribute that holder to this component', () => {
    const picked = electedText(asElection(electionEntry({ textSha256: sha256(APACHE_FILLED) })), [
      pooled(APACHE_FILLED, 20, 'filled'),
    ]);
    expect(picked.problems.join('\n')).toContain('appendix filled in');
  });

  it('refuses when two Apache-2.0 texts are equally common, because "the" text is then ambiguous', () => {
    const picked = electedText(election, [pooled(APACHE_TEXT, 7, 'a'), pooled(APACHE_NO_APPENDIX, 7, 'b')]);
    expect(picked.problems.join('\n')).toContain('equally common');
  });

  it('refuses when nothing in the inventory ships an Apache-2.0 text to elect', () => {
    expect(electedText(election, [pooled(LICENCE_BYTES, 9)]).problems.join('\n')).toContain(
      'ships an Apache-2.0 text',
    );
  });

  it('applies a whole set of elections, and refuses one the graph does not use', () => {
    const elections = new Map([[ELECTED_PURL, election]]);
    const pool = [pooled(APACHE_TEXT, 3)];
    const applied = resolveElections(elections, [electedComponent()], new Map(), pool);
    expect(applied.get(ELECTED_PURL)?.text).toBe(APACHE_TEXT);

    expect(() => resolveElections(elections, [], new Map(), pool)).toThrow('the election is unused');
    expect(() =>
      resolveElections(elections, [electedComponent()], new Map([[ELECTED_PURL, [{ filename: 'LICENSE' }]]]), pool),
    ).toThrow('Installed text wins');
  });
});

// ---- the committed store ------------------------------------------------------------------------

interface IndexTextEntry {
  id: string;
  filename: string;
  origin: 'installed' | 'upstream' | 'elected';
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
  electedFrom?: string;
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
    fromElection: number;
    unresolved: number;
  };
  components: IndexComponentEntry[];
  vendored: VendoredEntry[];
  elected: {
    purl: string;
    declaredSpdx: string;
    chosen: string;
    textId: string;
    textSha256: string;
    sameTextCarriers: number;
    attribution: { name: string; version: string; authors: string[]; repository: string };
    evidence: string[];
  }[];
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
    const elected = idx.components.filter((c) => c.texts.some((t) => t.origin === 'elected')).length;
    expect(idx.counts.fromInstalledArtifact).toBe(installed);
    expect(idx.counts.fromVendoredUpstream).toBe(vendored);
    expect(idx.counts.fromElection).toBe(elected);
    expect(installed + vendored + elected + idx.counts.unresolved).toBe(idx.counts.distributed);

    const files = new Set(idx.vendored.flatMap((v) => v.files.map((f) => f.path)));
    expect(idx.counts.vendoredSources).toBe(files.size);
    expect(idx.counts.vendoredCommits).toBe(new Set(idx.vendored.map((v) => v.commit)).size);
  });

  it('applies every committed election, and says plainly in the bundle what was chosen from what', () => {
    const store = loadUpstreamStore(ROOT);
    const idx = index();
    const bundle = readFileSync(resolve(ROOT, 'licensing', 'THIRD_PARTY_LICENSES.txt'), 'utf8');
    expect(idx.elected.map((e) => e.purl)).toEqual([...store.electionsByPurl.keys()]);
    const unresolved = new Set(idx.unresolved.map((u) => u.purl));
    for (const e of idx.elected) {
      const c = idx.components.find((x) => x.purl === e.purl);
      expect(c?.distributed, e.purl).toBe(true);
      // The declared expression is still what is published as SPDX; the election sits beside it.
      expect(c?.spdx, e.purl).toBe(e.declaredSpdx);
      expect(c?.texts.map((t) => t.origin), e.purl).toEqual(['elected']);
      expect(unresolved.has(e.purl), `${e.purl} is both elected and unresolved`).toBe(false);
      // Byte-identical to a copy other components ship from their own installed artifacts.
      const carriers = idx.components.filter((x) =>
        x.texts.some((t) => t.origin === 'installed' && t.textSha256 === e.textSha256),
      );
      expect(carriers.length, e.purl).toBe(e.sameTextCarriers);
      expect(carriers.length, e.purl).toBeGreaterThan(10);
      expect(bundle).toContain(
        `licence    : ${e.chosen}, elected from ${e.declaredSpdx}; the published crate ships no licence file`,
      );
      expect(bundle).toContain(`attribution: ${e.attribution.name} ${e.attribution.version}, by ${e.attribution.authors.join(', ')}`);
      expect(e.evidence.join(' ')).toContain('not legal advice');
    }
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
