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
// # Outputs
//
//   licensing/THIRD_PARTY_LICENSES.txt   the document that ships: identity block per component,
//                                        then each distinct licence text once
//   licensing/third-party-index.json     machine-readable provenance, including the SHA-256 of the
//                                        source bytes so any entry can be re-verified against the
//                                        installed package
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
  npmInventory,
  provenancePath,
  sha256hex,
} from './lib/licensing.mjs';

const ROOT = process.cwd();
const OUT_DIR = join(ROOT, 'licensing');
const CHECK = process.argv.includes('--check');
const REQUIRE_COMPLETE = process.argv.includes('--require-complete');

const cargo = cargoInventory(ROOT);
const npm = npmInventory(ROOT);
const components = [...cargo.components, ...npm.components].sort((a, b) => byteCompare(a.purl, b.purl));

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

for (const component of components) {
  // Only what ships carries an obligation. `node_modules/` is not distributed, so an install-only
  // package's licence text is not required — it is listed in the index for completeness and marked
  // as not distributed, rather than silently dropped or misleadingly attributed.
  if (!component.distributed) {
    entries.push({ component, texts: [] });
    continue;
  }

  const declared = component.declaredLicense;
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

  const usable = files.filter((f) => f.lossless);
  if (usable.length === 0) {
    blocked(
      component,
      component.sourceDir
        ? `declares ${declared ?? 'no licence'} but the installed artifact contains no licence file`
        : 'is compiled into the output by the toolchain and is not present in the install tree',
    );
  }

  for (const f of usable) {
    if (!textsBySha.has(f.textSha256)) textsBySha.set(f.textSha256, { text: f.text, users: [] });
    textsBySha.get(f.textSha256).users.push({ component, filename: f.filename });
  }
  entries.push({ component, texts: usable });
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

// ---- render -----------------------------------------------------------------------------------

const counts = {
  total: components.length,
  distributed: components.filter((c) => c.distributed).length,
  cargo: cargo.components.length,
  npmDistributed: npm.components.filter((c) => c.distributed).length,
  npmInstallOnly: npm.components.filter((c) => !c.distributed).length,
  texts: textsBySha.size,
  unresolved: unresolved.length,
};

const rule = '='.repeat(96);
const thin = '-'.repeat(96);

function renderBundle() {
  const out = [];
  out.push(rule);
  out.push('ANIMA-ENGINE - THIRD-PARTY LICENCES');
  out.push(rule);
  out.push('');
  out.push('Anima-Engine itself is proprietary and this file grants no rights to it; see LICENSE.');
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
  out.push(`Components with no obtainable text           : ${counts.unresolved}`);
  out.push('');
  out.push('The only transforms applied to a licence text are removal of a UTF-8 byte-order mark and');
  out.push('normalisation of line endings to LF. No text is paraphrased, summarised or substituted.');
  out.push('');
  out.push(rule);
  out.push('COMPONENT INDEX');
  out.push(rule);
  out.push('');

  for (const { component, texts } of entries) {
    const marker = component.distributed ? '' : '   [NOT DISTRIBUTED - installed only]';
    out.push(`${component.purl}${marker}`);
    out.push(`    name       : ${component.name} ${component.version}`);
    out.push(`    ecosystem  : ${component.ecosystem} (${component.origin})`);
    out.push(`    SPDX       : ${component.declaredLicense ?? '(none declared)'}`);
    if (component.repository) out.push(`    repository : ${component.repository}`);
    if (texts.length > 0) {
      for (const t of texts) {
        out.push(`    text       : ${textIds.get(t.textSha256)}  ${provenancePath(component, t.filename, ROOT)}`);
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
        'the hash of the licence file as installed, so any entry can be re-verified against the ' +
        'package; `textSha256` is the hash after BOM removal and LF normalisation, and is what ' +
        'THIRD_PARTY_LICENSES.txt deduplicates on.',
      counts,
      components: entries.map(({ component, texts }) => ({
        purl: component.purl,
        name: component.name,
        version: component.version,
        ecosystem: component.ecosystem,
        origin: component.origin,
        distributed: component.distributed,
        spdx: component.declaredLicense,
        declaredLicenseFile: component.declaredLicenseFile,
        repository: component.repository,
        texts: texts.map((t) => ({
          id: textIds.get(t.textSha256),
          filename: t.filename,
          provenance: provenancePath(component, t.filename, ROOT),
          sourceSha256: t.sourceSha256,
          sourceBytes: t.sourceBytes,
          textSha256: t.textSha256,
        })),
      })),
      texts: [...textsBySha.keys()]
        .sort(byteCompare)
        .map((sha) => ({ id: textIds.get(sha), textSha256: sha, usedBy: textsBySha.get(sha).users.length })),
      unresolved,
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
  out.push('Resolving one means obtaining the licence file from the upstream repository **at the tag');
  out.push('matching the version below**, recording where it came from, and re-running the');
  out.push('generator. That is a human decision about a legal artifact; this file names the work,');
  out.push('it does not do it.');
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
