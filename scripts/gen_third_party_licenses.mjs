#!/usr/bin/env node
// Package the licence texts that MIT, BSD and Apache-2.0 require to travel with the distribution.
//
// # What was actually missing
//
// `NOTICE` is an inventory: it names 455 components and the SPDX expression each one declares. That
// is a prerequisite for compliance, not the discharge of it. MIT's operative sentence is "the above
// copyright notice and this permission notice shall be included in all copies or substantial
// portions of the Software", and a list of the string `"MIT"` is neither. `NOTICE` said so itself,
// in a section headed "What this file does NOT establish" — an honest admission of a
// release-blocking gap, left open.
//
// This closes it for every component whose installed artifact contains the text, and enumerates the
// ones where it does not. It never synthesises a licence: the canonical SPDX text of MIT carries no
// copyright holder, and MIT's requirement is precisely that the holder's notice be reproduced, so
// substituting a generic text would be fabricated compliance that reads as real. Those components
// go into `UNRESOLVED.md` with the exact reason and the upstream to fetch from.
//
// # The second source: vendored upstream text
//
// 32 distributed components published no licence file in their artifact at all, so no amount of
// reading `node_modules/` or the cargo registry could close them. Their text exists upstream, at the
// revision the release was cut from, and `licensing/upstream/` holds those bytes with a provenance
// manifest that `scripts/lib/upstream_licenses.mjs` validates fail-closed — hash, byte length,
// commit, ref, purl, version, containment, symlinks, duplicates, unused entries, the lot.
//
// Two rules keep that store from becoming a place where inconvenient components go to be marked
// resolved. **Installed text wins**: a vendored mapping for a component whose artifact *does* carry
// its licence is an error, not a fallback, so the day an upstream starts shipping its own text the
// stale mapping fails loudly instead of quietly shadowing it. And **every mapping must be used**: a
// mapping naming a component that is not in the graph, or is not distributed, or is not currently
// unresolved, stops the run.
//
// # Outputs
//
//   licensing/THIRD_PARTY_LICENSES.txt   the document that ships: identity block per component,
//                                        then each distinct licence text once
//   licensing/third-party-index.json     machine-readable provenance, including the SHA-256 of the
//                                        source bytes so any entry can be re-verified against the
//                                        installed package or re-fetched from its pinned URL
//   licensing/UNRESOLVED.md              components whose text could not be obtained, and why
//
// # Deduplication
//
// 722 licence files across 419 crates reduce to a few hundred distinct texts, because every crate
// that offers Apache-2.0 ships a byte-identical copy of the same 11 KiB. They are emitted once and
// referenced by a stable id. The id is assigned by sorting the distinct texts by their SHA-256 in
// byte order, so it depends only on the set of texts — never on which component happened to be
// visited first, which is what a naive counter would encode.
//
// Usage: node scripts/gen_third_party_licenses.mjs [--check] [--require-complete]
//   --check             exit 1 if any artifact is out of date instead of rewriting it (CI)
//   --require-complete  exit 1 while any component's licence text is unresolved (release gate)

import { mkdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import {
  byteCompare,
  cargoInventory,
  collectLicenseTexts,
  decodeLicenseText,
  npmInventory,
  provenancePath,
  sha256hex,
} from './lib/licensing.mjs';
import { loadUpstreamStore, storeProvenancePath } from './lib/upstream_licenses.mjs';

const ROOT = process.cwd();
const OUT_DIR = join(ROOT, 'licensing');
const CHECK = process.argv.includes('--check');
const REQUIRE_COMPLETE = process.argv.includes('--require-complete');

const cargo = cargoInventory(ROOT);
const npm = npmInventory(ROOT);
const components = [...cargo.components, ...npm.components].sort((a, b) => byteCompare(a.purl, b.purl));

// Loaded before anything is rendered, and it throws rather than warns. A store that half-verified is
// worse than no store: the artifacts would still be produced, still look complete, and be wrong in a
// way no reader could see. Nothing here reaches the network — the bytes are committed and hashed.
const upstream = loadUpstreamStore(ROOT);

// Two components with one identity would make every downstream reference ambiguous — which text
// belongs to which, which SPDX expression applies. There is no safe way to guess, so it stops here.
const identities = new Set();
for (const c of components) {
  if (identities.has(c.purl)) {
    throw new Error(`duplicate component identity ${c.purl}; the inventory is ambiguous`);
  }
  identities.add(c.purl);
}

// ---- gather -----------------------------------------------------------------------------------

/** Distinct licence text -> id. Keyed by the SHA-256 of the decoded text. */
const textsBySha = new Map();
const entries = [];

/**
 * Record one licence text against the component that carries it.
 *
 * Deduplication keys on the decoded bytes and nothing else, so a text vendored from upstream and the
 * same text found inside another package's artifact collapse into one entry — which is correct, and
 * which is why the *source* hash stays per-entry: the text is shared, the provenance is not.
 */
function addText(textSha256, text, component, filename) {
  if (!textsBySha.has(textSha256)) textsBySha.set(textSha256, { text, users: [] });
  textsBySha.get(textSha256).users.push({ component, filename });
}

/** A licence file read out of the artifact that was actually installed. */
function asInstalledText(component, file) {
  return {
    origin: 'installed',
    filename: file.filename,
    provenance: provenancePath(component, file.filename, ROOT),
    sourceSha256: file.sourceSha256,
    sourceBytes: file.sourceBytes,
    textSha256: file.textSha256,
  };
}

// One row per component, not per reason. `@oxc-project/runtime` fails two checks at once — it
// declares no licence AND is absent from the install tree — and counting that as two unresolved
// components would overstate the gap by exactly the amount that makes the number untrustworthy.
const unresolvedByPurl = new Map();
function blocked(component, reason) {
  const existing = unresolvedByPurl.get(component.purl);
  if (existing) {
    if (!existing.reasons.includes(reason)) existing.reasons.push(reason);
    return;
  }
  unresolvedByPurl.set(component.purl, {
    purl: component.purl,
    name: component.name,
    version: component.version,
    ecosystem: component.ecosystem,
    repository: component.repository,
    declared: component.declaredLicense,
    reasons: [reason],
  });
}

/** Purls the vendored store claims, so an unused or unknown mapping can be reported at the end. */
const usedMappings = new Set();

for (const component of components) {
  // Only what ships carries an obligation. `node_modules/` is not distributed, so an install-only
  // package's licence text is not required — it is listed in the index for completeness and marked
  // as not distributed, rather than silently dropped or misleadingly attributed.
  if (!component.distributed) {
    // A mapping for something that does not ship is not merely useless: it inflates the coverage
    // numbers with obligations nobody has. Caught here rather than left to the unused-mapping sweep,
    // because the message differs and the distinction is the point.
    if (upstream.byPurl.has(component.purl)) {
      throw new Error(
        `licensing/upstream/sources.json maps ${component.purl}, which is installed but not ` +
          `distributed. Only what ships carries an obligation; remove the mapping.`,
      );
    }
    entries.push({ component, texts: [] });
    continue;
  }

  const mapping = upstream.byPurl.get(component.purl);
  // The store may supply the SPDX expression for a component whose local manifest carries none.
  // `@oxc-project/runtime` is the case: rolldown compiles its helpers into the output and it is
  // never installed, so there is no manifest here to read a licence from — the registry publishes
  // one for that exact version, and the mapping records it with the evidence.
  const declared = component.declaredLicense ?? mapping?.declaredSpdx ?? null;
  const files = component.sourceDir ? collectLicenseTexts(component.sourceDir) : [];

  if (!declared && !component.declaredLicenseFile) {
    blocked(component, 'declares no licence in its own manifest');
  }

  const corrupt = files.filter((f) => !f.lossless);
  if (corrupt.length > 0) {
    blocked(
      component,
      `licence file is not valid UTF-8 and was not transcoded: ${corrupt.map((f) => f.filename).join(', ')}`,
    );
  }

  const installed = files.filter((f) => f.lossless);

  // Installed text wins, always. The artifact that was actually linked or bundled is the primary
  // evidence of its own terms; a vendored copy is a reconstruction of what should have been in it.
  // So a mapping here is a hard error rather than a silent no-op: the day an upstream starts
  // shipping its licence, this fires instead of letting a stale pin shadow the real thing.
  if (installed.length > 0) {
    if (mapping) {
      throw new Error(
        `licensing/upstream/sources.json maps ${component.purl}, but its installed artifact now ` +
          `contains ${installed.map((f) => f.filename).join(', ')}. Installed text is preferred; ` +
          `delete the mapping and its now-unused sources.`,
      );
    }
    for (const f of installed) addText(f.textSha256, f.text, component, f.filename);
    entries.push({ component, texts: installed.map((f) => asInstalledText(component, f)) });
    continue;
  }

  if (mapping) {
    usedMappings.add(component.purl);
    const texts = mapping.resolvedSources.map((source) => {
      const { text, lossless } = decodeLicenseText(source.raw);
      // A licence that cannot be decoded losslessly must not be published as one: replacement
      // characters in a copyright line corrupt exactly the name the licence requires be reproduced.
      if (!lossless) {
        throw new Error(
          `${storeProvenancePath(source.id)} is not valid UTF-8. A vendored licence is published ` +
            `verbatim or not at all; re-fetch it or leave ${component.purl} unresolved.`,
        );
      }
      addText(sha256hex(Buffer.from(text, 'utf8')), text, component, source.filename);
      return {
        origin: 'upstream',
        filename: source.filename,
        provenance: storeProvenancePath(source.id),
        sourceSha256: source.sha256,
        sourceBytes: source.bytes,
        textSha256: sha256hex(Buffer.from(text, 'utf8')),
        upstream: source,
        mapping,
      };
    });
    entries.push({ component, texts });
    continue;
  }

  blocked(
    component,
    component.sourceDir
      ? `declares ${declared ?? 'no licence'} but the installed artifact contains no licence file`
      : 'is compiled into the output by the toolchain and is not present in the install tree',
  );
  entries.push({ component, texts: [] });
}

// A mapping that matched no component in the graph describes a dependency this application does not
// have. Left unreported it is a claim of coverage for something that is not here — and the shape of
// a typo'd purl, which would otherwise look identical to a resolved row.
const unknownMappings = [...upstream.byPurl.keys()].filter((purl) => !identities.has(purl));
if (unknownMappings.length > 0) {
  throw new Error(
    `licensing/upstream/sources.json maps ${unknownMappings.length} component(s) that are not in ` +
      `the dependency graph:\n  ${unknownMappings.join('\n  ')}\n` +
      `A mapping for a component nothing depends on cannot be verified against anything.`,
  );
}

// The branches above are meant to account for every mapping — used, shadowed by installed text, not
// distributed, or not in the graph. This asserts that they do, so a future branch that forgets to
// classify one cannot leave it silently unaccounted.
const strandedMappings = [...upstream.byPurl.keys()].filter((purl) => !usedMappings.has(purl));
if (strandedMappings.length > 0) {
  throw new Error(
    `licensing/upstream/sources.json maps ${strandedMappings.length} component(s) that were ` +
      `neither used nor rejected:\n  ${strandedMappings.join('\n  ')}`,
  );
}

// Ids depend only on the set of texts, so the same graph yields the same ids regardless of the
// order components were visited in.
const textIds = new Map();
[...textsBySha.keys()].sort(byteCompare).forEach((sha, i) => {
  textIds.set(sha, `T${String(i + 1).padStart(4, '0')}`);
});

const unresolved = [...unresolvedByPurl.values()]
  .map((u) => ({ ...u, reasons: [...u.reasons].sort(byteCompare) }))
  .sort((a, b) => byteCompare(a.purl, b.purl));

// A `blocked` record asserts that a search happened and found nothing. Left standing after the
// component is resolved — or after it leaves the graph — it is a false statement about the current
// tree in a document whose only value is being true about it.
const staleBlocked = [...upstream.blockedByPurl.keys()].filter((purl) => !unresolvedByPurl.has(purl));
if (staleBlocked.length > 0) {
  throw new Error(
    `licensing/upstream/sources.json records ${staleBlocked.length} component(s) as blocked that ` +
      `are not unresolved:\n  ${staleBlocked.join('\n  ')}\n` +
      `Either the text was found, or the component left the graph. Remove the record.`,
  );
}

// ---- render -----------------------------------------------------------------------------------

/** Components whose text came out of the artifact that was installed, and those from the store. */
const vendoredEntries = entries.filter((e) => e.texts.some((t) => t.origin === 'upstream'));
const vendoredSourceIds = new Set(
  vendoredEntries.flatMap((e) => e.texts.filter((t) => t.origin === 'upstream').map((t) => t.upstream.id)),
);

const counts = {
  total: components.length,
  distributed: components.filter((c) => c.distributed).length,
  cargo: cargo.components.length,
  npmDistributed: npm.components.filter((c) => c.distributed).length,
  npmInstallOnly: npm.components.filter((c) => !c.distributed).length,
  texts: textsBySha.size,
  fromInstalledArtifact: entries.filter((e) => e.texts.some((t) => t.origin === 'installed')).length,
  fromVendoredUpstream: vendoredEntries.length,
  vendoredSources: vendoredSourceIds.size,
  vendoredRepositories: new Set([...vendoredSourceIds].map((id) => id.split('/').slice(0, 3).join('/'))).size,
  vendoredCommits: new Set([...vendoredSourceIds].map((id) => id.split('/')[3])).size,
  unresolved: unresolved.length,
};

const rule = '='.repeat(96);
const thin = '-'.repeat(96);

/**
 * The SPDX expression to publish for a component.
 *
 * Normally the one its own manifest declares. `@oxc-project/runtime` is the exception that makes
 * this a function: rolldown compiles its helpers into `dist/` and it is never installed, so there is
 * no local manifest to read, and the expression comes from the vendored mapping — where it is
 * recorded with the registry evidence rather than inferred.
 */
function spdxOf(component, texts) {
  if (component.declaredLicense) return component.declaredLicense;
  const fromStore = texts.find((t) => t.origin === 'upstream');
  return fromStore ? fromStore.mapping.declaredSpdx : null;
}

function renderBundle() {
  const out = [];
  out.push(rule);
  out.push('ANIMA-ENGINE - THIRD-PARTY LICENCES');
  out.push(rule);
  out.push('');
  out.push('Anima-Engine is dual-licensed under MIT OR Apache-2.0; see LICENSE-MIT and LICENSE-APACHE.');
  out.push('');
  out.push('This file reproduces the licence and copyright notices of the third-party components');
  out.push('distributed inside the application, because MIT and BSD require that text to accompany');
  out.push('the distribution and Apache-2.0 adds a NOTICE-propagation clause.');
  out.push('');
  out.push('GENERATED - DO NOT EDIT. Regenerate with `node scripts/gen_third_party_licenses.mjs`.');
  out.push('Machine-readable provenance, including the SHA-256 of every source file, is in');
  out.push('licensing/third-party-index.json. Components whose text could not be obtained are');
  out.push('enumerated in licensing/UNRESOLVED.md; this file does not paper over them.');
  out.push('');
  out.push(`Components covered: ${counts.distributed} distributed of ${counts.total} inventoried`);
  out.push(`  Rust crates linked into the desktop binary : ${counts.cargo}`);
  out.push(`  npm packages with bytes in the frontend    : ${counts.npmDistributed}`);
  out.push(`  npm packages installed but not distributed : ${counts.npmInstallOnly} (listed, no text required)`);
  out.push(`Distinct licence texts                       : ${counts.texts}`);
  out.push(`  read from the installed artifact           : ${counts.fromInstalledArtifact} component(s)`);
  out.push(`  vendored from pinned upstream revisions    : ${counts.fromVendoredUpstream} component(s)`);
  out.push(`Components with no obtainable text           : ${counts.unresolved}`);
  out.push('');
  out.push('The only transforms applied to a licence text are removal of a UTF-8 byte-order mark and');
  out.push('normalisation of line endings to LF. No text is paraphrased, summarised or substituted.');
  out.push('');
  out.push('Where a published artifact carries no licence file, the text is taken from the upstream');
  out.push('repository at the immutable commit that release was published from, stored verbatim under');
  out.push('licensing/upstream/ and hashed. Those are enumerated below with their exact revision. A');
  out.push('licence text read out of the installed artifact is always preferred to a vendored one.');
  out.push('');
  out.push(rule);
  out.push('VENDORED UPSTREAM SOURCES');
  out.push(rule);
  out.push('');
  out.push(`${counts.vendoredSources} file(s) from ${counts.vendoredCommits} commit(s) across`);
  out.push(`${counts.vendoredRepositories} repositories, covering ${counts.fromVendoredUpstream} component(s).`);
  out.push('Full provenance, including the evidence tying each commit to each released version, is in');
  out.push('licensing/upstream/sources.json.');
  out.push('');

  for (const { component, texts } of vendoredEntries) {
    const upstreamTexts = texts.filter((t) => t.origin === 'upstream');
    const { provenance } = upstreamTexts[0].mapping;
    out.push(`${component.purl}`);
    out.push(`    upstream   : ${provenance.repository}`);
    out.push(`    commit     : ${provenance.commit}`);
    out.push(`    release ref: ${provenance.tag ?? '(no upstream tag at this commit)'}`);
    out.push(`    provenance : ${provenance.kind}`);
    if (provenance.kind === 'project-repository') {
      out.push(`    component  : ${provenance.componentRepository} @ ${provenance.componentCommit}`);
    }
    for (const t of upstreamTexts) {
      out.push(`    file       : ${t.upstream.materialType}  ${t.upstream.pathInRepo}`);
      out.push(`                 sha256:${t.sourceSha256}  ${t.sourceBytes} bytes`);
    }
    out.push('');
  }

  out.push(rule);
  out.push('COMPONENT INDEX');
  out.push(rule);
  out.push('');

  for (const { component, texts } of entries) {
    const marker = component.distributed ? '' : '   [NOT DISTRIBUTED - installed only]';
    out.push(`${component.purl}${marker}`);
    out.push(`    name       : ${component.name} ${component.version}`);
    out.push(`    ecosystem  : ${component.ecosystem} (${component.origin})`);
    out.push(`    SPDX       : ${spdxOf(component, texts) ?? '(none declared)'}`);
    if (component.repository) out.push(`    repository : ${component.repository}`);
    if (texts.length > 0) {
      for (const t of texts) {
        out.push(`    text       : ${textIds.get(t.textSha256)}  ${t.provenance}`);
      }
    } else if (component.distributed) {
      out.push('    text       : UNRESOLVED - see licensing/UNRESOLVED.md');
    }
    out.push('');
  }

  out.push(rule);
  out.push('LICENCE TEXTS');
  out.push(rule);
  out.push('');

  for (const sha of [...textsBySha.keys()].sort(byteCompare)) {
    const id = textIds.get(sha);
    const { text, users } = textsBySha.get(sha);
    const sorted = [...users].sort(
      (a, b) => byteCompare(a.component.purl, b.component.purl) || byteCompare(a.filename, b.filename),
    );
    out.push(thin);
    out.push(`${id}   sha256:${sha}`);
    out.push(`applies to ${sorted.length} component file(s):`);
    for (const u of sorted) out.push(`  ${u.component.purl}  (${u.filename})`);
    out.push(thin);
    out.push('');
    out.push(text.replace(/\n+$/, ''));
    out.push('');
  }
  // Exactly one newline at EOF. The loop leaves a blank separator after the last text, and a file
  // ending `\n\n` is a real defect in this generator rather than something a licence asked for.
  return `${out.join('\n').replace(/\n+$/, '')}\n`;
}

function renderIndex() {
  return `${JSON.stringify(
    {
      $comment:
        'Generated by scripts/gen_third_party_licenses.mjs. Do not edit by hand. `sourceSha256` is ' +
        'the hash of the licence file as installed — or, where `origin` is `upstream`, of the file ' +
        'vendored under licensing/upstream/ — so any entry can be re-verified against the package ' +
        'it came from or re-fetched from `upstreamUrl`; `textSha256` is the hash after BOM removal ' +
        'and LF normalisation, and is what THIRD_PARTY_LICENSES.txt deduplicates on.',
      counts,
      components: entries.map(({ component, texts }) => ({
        purl: component.purl,
        name: component.name,
        version: component.version,
        ecosystem: component.ecosystem,
        origin: component.origin,
        distributed: component.distributed,
        spdx: spdxOf(component, texts),
        spdxSource: component.declaredLicense ? 'manifest' : texts.some((t) => t.origin === 'upstream') ? 'upstream-manifest' : null,
        declaredLicenseFile: component.declaredLicenseFile,
        repository: component.repository,
        texts: texts.map((t) => ({
          id: textIds.get(t.textSha256),
          filename: t.filename,
          origin: t.origin,
          provenance: t.provenance,
          sourceSha256: t.sourceSha256,
          sourceBytes: t.sourceBytes,
          textSha256: t.textSha256,
          // Present only for vendored text: everything a reader needs to re-fetch and re-hash the
          // exact bytes without trusting this file, plus the material it is being packaged as.
          ...(t.origin === 'upstream'
            ? {
                upstreamUrl: t.upstream.url,
                upstreamCommit: t.upstream.commit,
                upstreamRef: t.mapping.provenance.tag,
                provenanceKind: t.mapping.provenance.kind,
                materialType: t.upstream.materialType,
                retrieved: t.upstream.retrieved,
              }
            : {}),
        })),
      })),
      // The store, flattened for consumers that want the provenance without walking components.
      // Sorted by purl, and every field is a committed constant, so this section is reproducible.
      vendored: vendoredEntries.map(({ component, texts }) => {
        const upstreamTexts = texts.filter((t) => t.origin === 'upstream');
        const { provenance, declaredSpdx, material } = upstreamTexts[0].mapping;
        return {
          purl: component.purl,
          declaredSpdx,
          kind: provenance.kind,
          repository: provenance.repository,
          commit: provenance.commit,
          ref: provenance.tag,
          ...(provenance.kind === 'project-repository'
            ? {
                componentRepository: provenance.componentRepository,
                componentCommit: provenance.componentCommit,
                componentTag: provenance.componentTag ?? null,
                justification: provenance.justification,
              }
            : {}),
          evidence: provenance.evidence,
          ...(material ? { material } : {}),
          files: upstreamTexts.map((t) => ({
            path: t.provenance,
            url: t.upstream.url,
            materialType: t.upstream.materialType,
            spdx: t.upstream.spdx,
            bytes: t.sourceBytes,
            sha256: t.sourceSha256,
            retrieved: t.upstream.retrieved,
          })),
        };
      }),
      texts: [...textsBySha.keys()]
        .sort(byteCompare)
        .map((sha) => ({ id: textIds.get(sha), textSha256: sha, usedBy: textsBySha.get(sha).users.length })),
      // Each unresolved row carries the search that failed, where one was recorded, so a consumer
      // can tell an upstream gap from work nobody started.
      unresolved: unresolved.map((u) => {
        const b = upstream.blockedByPurl.get(u.purl);
        return b
          ? {
              ...u,
              investigated: {
                repository: b.repository,
                commit: b.commit,
                tag: b.tag,
                date: b.investigated,
                evidence: b.evidence,
              },
            }
          : u;
      }),
    },
    null,
    2,
  )}\n`;
}

function renderUnresolved() {
  const out = [];
  out.push('# Unresolved third-party licence texts');
  out.push('');
  out.push('**Generated — do not edit by hand.** Regenerate with');
  out.push('`node scripts/gen_third_party_licenses.mjs`.');
  out.push('');
  out.push('Each component below is **distributed** inside the application and declares a licence,');
  out.push('but the artifact that was installed contains no copy of that licence text. Engineering');
  out.push('cannot close these by generating text: the canonical SPDX text of MIT contains no');
  out.push('copyright holder, and reproducing the holder’s notice is exactly what MIT requires. A');
  out.push('substituted text would look like compliance and would not be it.');
  out.push('');
  out.push('Resolving one means obtaining the licence file from the upstream repository **at the');
  out.push('immutable commit the release was published from**, vendoring it under');
  out.push('[`upstream/`](upstream/) with its provenance in');
  out.push('[`upstream/sources.json`](upstream/sources.json), and re-running the generator. A row');
  out.push('survives here only when no such file exists upstream at all.');
  out.push('');
  out.push(
    `**${counts.fromVendoredUpstream} of the original 32 have been closed that way**, from ` +
      `${counts.vendoredSources} vendored file(s)`,
  );
  out.push(`across ${counts.vendoredCommits} commit(s) in ${counts.vendoredRepositories} repositories.`);
  out.push('');
  if (unresolved.length === 0) {
    out.push('_Every distributed component supplies its licence text. Nothing is unresolved._');
    return `${out.join('\n').replace(/\n+$/, '')}\n`;
  }
  out.push(`**${unresolved.length} component(s) unresolved.** Distribution of the affected components`);
  out.push('is blocked until each is closed.');
  out.push('');
  out.push('| Component | Version | Ecosystem | Declared | Reason | Upstream |');
  out.push('|---|---|---|---|---|---|');
  for (const u of unresolved) {
    out.push(
      `| \`${u.name}\` | ${u.version} | ${u.ecosystem} | ${u.declared ?? '_none_'} | ` +
        `${u.reasons.join('; ')} | ${u.repository ?? '_none declared_'} |`,
    );
  }

  // The row on its own says a text is missing; it cannot say whether anyone looked. These say what
  // was searched, at which revision, and what was found instead — so the gap is a finding rather
  // than an absence of work, and a reader can re-run the search from the same starting point.
  const withEvidence = unresolved.filter((u) => upstream.blockedByPurl.has(u.purl));
  if (withEvidence.length > 0) {
    out.push('');
    out.push('## What was searched');
    out.push('');
    out.push('Recorded in [`upstream/sources.json`](upstream/sources.json) under `blocked`, and');
    out.push('re-checked on every run: a component may not be listed there and resolved at the same');
    out.push('time.');
    for (const u of withEvidence) {
      const b = upstream.blockedByPurl.get(u.purl);
      out.push('');
      out.push(`### \`${b.name}\` ${b.version} — declares ${b.declaredSpdx}`);
      out.push('');
      out.push(`Searched at \`${b.repository}\` commit \`${b.commit}\``);
      out.push(`(${b.tag === null ? 'no release tag at that commit' : `tag \`${b.tag}\``}), ${b.investigated}.`);
      out.push('');
      for (const line of b.evidence) out.push(`- ${line}`);
    }
  }
  return `${out.join('\n').replace(/\n+$/, '')}\n`;
}

// ---- emit -------------------------------------------------------------------------------------

const artifacts = [
  ['THIRD_PARTY_LICENSES.txt', renderBundle()],
  ['third-party-index.json', renderIndex()],
  ['UNRESOLVED.md', renderUnresolved()],
];

if (CHECK) {
  const stale = [];
  for (const [name, body] of artifacts) {
    const path = join(OUT_DIR, name);
    const current = existsSync(path) ? readFileSync(path, 'utf8') : '';
    if (current !== body) stale.push(name);
  }
  if (stale.length > 0) {
    console.error(
      `licensing/ is out of date with the dependency graph: ${stale.join(', ')}.\n` +
        'Regenerate with `node scripts/gen_third_party_licenses.mjs` and commit the result.',
    );
    process.exit(1);
  }
  console.log(
    `third-party licences check: up to date (${counts.distributed} distributed components, ` +
      `${counts.texts} distinct texts, ${counts.unresolved} unresolved)`,
  );
} else {
  mkdirSync(OUT_DIR, { recursive: true });
  for (const [name, body] of artifacts) writeFileSync(join(OUT_DIR, name), body);
  console.log(
    `wrote licensing/ — ${counts.distributed} distributed of ${counts.total} components, ` +
      `${counts.texts} distinct licence texts (sha256 of bundle: ` +
      `${sha256hex(Buffer.from(artifacts[0][1], 'utf8')).slice(0, 16)}), ${counts.unresolved} unresolved`,
  );
}

if (REQUIRE_COMPLETE && unresolved.length > 0) {
  console.error(
    `\n${unresolved.length} distributed component(s) have no obtainable licence text. ` +
      'See licensing/UNRESOLVED.md.\nThis gate is what "ready to distribute" means; it is failing.',
  );
  process.exit(1);
}
