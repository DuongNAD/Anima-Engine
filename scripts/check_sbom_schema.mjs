#!/usr/bin/env node
// Validate sbom.cdx.json against the official CycloneDX 1.5 JSON schema.
//
// # Why a generator's own claim is not evidence
//
// `gen_sbom.mjs` writes `"specVersion": "1.5"`. Nothing read the specification, so the file was
// CycloneDX-*shaped* — which is the state in which a consumer's stricter parser rejects it and the
// SBOM turns out to have been unusable for the whole time it looked fine. The schema is the only
// thing that can turn that string into a checked fact.
//
// # Reproducibility
//
// The schemas are vendored in `schemas/cyclonedx/`, pinned to a **commit** of the CycloneDX
// specification repository rather than to the mutable `1.5` branch, with provenance and SHA-256 in
// `PROVENANCE.json`. Nothing is fetched here: CI must not depend on a network round-trip to decide
// whether a release artifact is valid, and a schema that can change without a commit in this
// repository is a gate whose meaning can change without review. The hashes are re-checked on every
// run, so an edited schema fails loudly instead of quietly widening what counts as valid.
//
// # Beyond the schema
//
// Schema validity is necessary and not sufficient. The properties below cannot be expressed in JSON
// Schema, and each is a way a structurally valid SBOM can still be wrong:
//
//   * `bom-ref` and `purl` uniqueness — duplicates make every dependency edge ambiguous
//   * dependency refs resolve, and every component has a record — a dangling or missing ref makes a
//     consumer guess whether a component was omitted or an edge was wrong
//   * the reproducibility choices hold — a deterministic `serialNumber`, and no `timestamp`
//   * every component either declares a licence or is a **known** unresolved one, named in
//     `licensing/third-party-index.json`. Silence is the failure mode being guarded: a component
//     with no licence recorded reads as fine and is not
//   * the SBOM and the licence index describe the same set of components, so the two artifacts
//     cannot drift apart while both look freshly generated
//
// Usage: node scripts/check_sbom_schema.mjs [path/to/sbom.json]

import Ajv from 'ajv';
import addFormats from 'ajv-formats';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { SCHEMA_DIR } from './lib/licensing.mjs';

const ROOT = process.cwd();
const TARGET = process.argv[2] ?? join(ROOT, 'sbom.cdx.json');
const INDEX = join(ROOT, 'licensing', 'third-party-index.json');

const failures = [];
const fail = (message) => failures.push(message);

// ---- the vendored schemas are the ones that were reviewed ---------------------------------------
const provenance = JSON.parse(readFileSync(join(SCHEMA_DIR, 'PROVENANCE.json'), 'utf8'));
const schemas = [];
for (const entry of provenance.files) {
  const bytes = readFileSync(join(SCHEMA_DIR, entry.file));
  const actual = createHash('sha256').update(bytes).digest('hex');
  if (actual !== entry.sha256) {
    fail(
      `${entry.file} does not match PROVENANCE.json (expected sha256:${entry.sha256}, got ` +
        `sha256:${actual}). Re-vendor it from ${entry.url} or correct the manifest — do not ` +
        `validate against a schema nobody reviewed.`,
    );
  }
  const parsed = JSON.parse(bytes.toString('utf8'));
  if (parsed.$id !== entry.$id) fail(`${entry.file} declares $id ${parsed.$id}, expected ${entry.$id}`);
  schemas.push(parsed);
}

if (failures.length > 0) {
  for (const f of failures) console.error(`  x ${f}`);
  process.exit(1);
}

// ---- schema validation --------------------------------------------------------------------------
// `strict: false` because the CycloneDX schemas use draft-07 constructs Ajv's strict mode flags as
// unknown (`meta:enum`, and `$comment` beside a sibling `$ref`). That is a property of the official
// schema, not of the document under test, and treating it as an error would mean the gate could
// never run at all.
const ajv = new Ajv({ strict: false, allErrors: true, allowUnionTypes: true });
addFormats(ajv);

// `ajv-formats` implements the draft-07 format set; CycloneDX uses two that postdate it. Left
// unregistered, Ajv prints "unknown format ... ignored" for each of ~30 occurrences and then does
// not check them — sixty lines of warning for a check that is silently not happening. Registering
// validators makes the constraint real and the output quiet.
//
// Both are deliberate approximations. RFC 3987 IRIs differ from URIs by permitting non-ASCII, which
// is what `new URL` already accepts, and supplying a base is what makes a *reference* — the relative
// form — valid. RFC 6531 addresses are `local@domain` with the same relaxation. Neither is a full
// grammar and neither needs to be: the failure they exist to catch is a field holding something that
// is not an address at all.
const CONTROL_OR_SPACE = (ch) => {
  const c = ch.codePointAt(0) ?? 0;
  return c <= 0x20 || c === 0x7f;
};
ajv.addFormat('iri-reference', (value) => {
  // Rejecting control characters and spaces, and nothing else. A character class written to also
  // exclude `-` would reject every `github.com/tauri-apps/...` reference in this document while
  // looking perfectly reasonable.
  if ([...value].some(CONTROL_OR_SPACE)) return false;
  try {
    void new URL(value, 'http://anima.invalid/');
    return true;
  } catch {
    return false;
  }
});
ajv.addFormat('idn-email', (value) => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value));

for (const schema of schemas) ajv.addSchema(schema);

const validate = ajv.getSchema(provenance.files[0].$id);
if (!validate) {
  console.error(`  x could not compile ${provenance.files[0].$id}`);
  process.exit(1);
}

const bom = JSON.parse(readFileSync(TARGET, 'utf8'));

if (!validate(bom)) {
  for (const err of validate.errors ?? []) {
    fail(`schema: ${err.instancePath || '(root)'} ${err.message ?? ''} ${JSON.stringify(err.params)}`);
  }
}

// ---- properties the schema cannot express -------------------------------------------------------
if (bom.specVersion !== '1.5') fail(`specVersion is ${bom.specVersion}, expected 1.5`);
if (bom.bomFormat !== 'CycloneDX') fail(`bomFormat is ${bom.bomFormat}, expected CycloneDX`);

if (typeof bom.serialNumber !== 'string') {
  fail('serialNumber is absent; a BOM with no identity cannot be referenced by one that includes it');
} else if (
  !/^urn:uuid:[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(bom.serialNumber)
) {
  fail(`serialNumber ${bom.serialNumber} is not a lowercase urn:uuid`);
}

// The determinism contract: a wall-clock timestamp would make the `--check` freshness gate fail for
// reasons that have nothing to do with the dependency graph.
if (bom.metadata?.timestamp !== undefined) {
  fail('metadata.timestamp is present; the SBOM is generated without one so that it is reproducible');
}

// A component with no licence must be a component someone has already looked at and recorded.
const index = JSON.parse(readFileSync(INDEX, 'utf8'));
const knownUnresolved = new Set(index.unresolved.map((u) => u.purl));
const indexedPurls = new Set(index.components.map((c) => c.purl));

const seenRef = new Set();
const seenPurl = new Set();
for (const c of bom.components ?? []) {
  const ref = c['bom-ref'];
  if (seenRef.has(ref)) fail(`duplicate bom-ref: ${ref}`);
  seenRef.add(ref);
  if (typeof c.purl !== 'string' || c.purl === '') {
    fail(`${c.name}@${c.version} has no purl`);
  } else {
    if (seenPurl.has(c.purl)) fail(`duplicate purl: ${c.purl}`);
    seenPurl.add(c.purl);
  }
  if (!c.version) fail(`${c.name} has no version`);
  if (c.scope !== 'required' && c.scope !== 'excluded') {
    fail(`${c.purl} has scope ${c.scope ?? '(absent)'}; every component must state whether it ships`);
  }
  if ((!Array.isArray(c.licenses) || c.licenses.length === 0) && !knownUnresolved.has(c.purl)) {
    fail(
      `${c.purl} declares no licence and is not recorded in licensing/UNRESOLVED.md. An ` +
        `unattributed component must be resolved or documented, never merely absent.`,
    );
  }
}

// Two generated artifacts, one graph. If they disagree, at least one is stale and neither can be
// trusted — and both `--check` gates would still pass, because each only compares itself to itself.
for (const purl of indexedPurls) {
  if (!seenPurl.has(purl)) fail(`${purl} is in the licence index but not in the SBOM`);
}
for (const purl of seenPurl) {
  if (!indexedPurls.has(purl)) fail(`${purl} is in the SBOM but not in the licence index`);
}

const appRef = bom.metadata?.component?.['bom-ref'];
if (typeof appRef !== 'string') fail('metadata.component has no bom-ref');
const known = new Set([appRef, ...seenRef]);
const declared = new Set();
for (const d of bom.dependencies ?? []) {
  if (declared.has(d.ref)) fail(`duplicate dependency record for ${d.ref}`);
  declared.add(d.ref);
  if (!known.has(d.ref)) fail(`dependency record ${d.ref} names no component`);
  for (const target of d.dependsOn ?? []) {
    if (!known.has(target)) fail(`dependency ${d.ref} -> ${target} names no component`);
  }
}
for (const ref of known) {
  if (!declared.has(ref)) fail(`${ref} has no dependency record; the graph is incomplete`);
}

if (failures.length > 0) {
  console.error(`${TARGET} failed CycloneDX 1.5 validation — ${failures.length} problem(s):`);
  for (const f of failures.slice(0, 40)) console.error(`  x ${f}`);
  if (failures.length > 40) console.error(`  ... and ${failures.length - 40} more`);
  process.exit(1);
}

console.log(
  `SBOM schema check: valid CycloneDX ${bom.specVersion} — ${bom.components.length} components, ` +
    `${bom.dependencies.length} dependency records, ${seenPurl.size} unique purls, ` +
    `${knownUnresolved.size} documented licence gaps, ` +
    `schemas pinned at ${provenance.source.commit.slice(0, 12)}`,
);
