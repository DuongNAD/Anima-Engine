#!/usr/bin/env node
// Generate a CycloneDX 1.5 SBOM for everything Anima-Engine distributes.
//
// # Why this exists alongside NOTICE
//
// They are not the same artifact and one does not substitute for the other.
//
// `NOTICE` discharges a **legal** obligation: MIT and BSD require a copyright notice to travel with
// the distribution, Apache-2.0 adds a NOTICE-propagation clause. It is prose, grouped by licence,
// written to be read by a person. `licensing/THIRD_PARTY_LICENSES.txt` carries the texts themselves.
//
// An SBOM answers a **machine's** question: given a CVE published this morning, is this product
// affected? That needs a stable identifier per component — a package URL — and a dependency graph
// saying how it got in, not a bullet list grouped by licence string.
//
// # What changed, and why the old file was misleading
//
// The scope came from `npm ls --omit=dev --all`, which is the **install** closure. `node_modules` is
// never shipped; Tauri packages `dist/`. Measured, that was wrong in both directions at once: 18
// packages listed as components leave no bytes in the product (`@types/react`, `csstype`,
// `js-tokens`…), while three that do ship — `vite`, `rolldown` and `@oxc-project/runtime`, whose
// code the bundler compiles into the output — were absent because they are not production
// dependencies. Both errors are invisible from `package.json`; only the bundler knows, and
// `licensing/bundle-closure.json` is what it reports.
//
// Install-only components are still listed, with `scope: "excluded"` — CycloneDX's term for
// "declared, not distributed". A scanner that wants the attack surface of the shipped product reads
// `scope`; one that wants the build environment reads the whole list. Dropping them would answer
// only the first question.
//
// # Determinism
//
// No timestamp, components sorted by purl in byte order, and a `serialNumber` derived from the
// document's own content rather than randomly. An SBOM that differs on every run cannot be diffed,
// and the `--check` gate would then fail for reasons unrelated to the dependency graph.
//
// The file is validated against the official CycloneDX 1.5 JSON schema by
// `scripts/check_sbom_schema.mjs`. Emitting `specVersion: "1.5"` is a claim; that script is what
// makes it a checked one.
//
// Usage: node scripts/gen_sbom.mjs [--check]
//   --check  exit 1 if sbom.cdx.json is out of date instead of rewriting it (for CI)

import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import {
  byteCompare,
  cargoInventory,
  deterministicSerialNumber,
  licenseNode,
  npmInventory,
} from './lib/licensing.mjs';

const ROOT = process.cwd();
const CHECK = process.argv.includes('--check');
const OUT = join(ROOT, 'sbom.cdx.json');

// The SPDX expression the licence index settled on, keyed by purl.
//
// Almost always the component's own `declaredLicense`; the exception is a component the toolchain
// compiles into the output and never installs, which has no local manifest to read one from.
// `@oxc-project/runtime` is that case, and its expression comes from the registry via
// `licensing/upstream/sources.json`. Read back from the index rather than recomputed, so the SBOM
// and the licence bundle cannot disagree about the same component — three artifacts that describe
// the graph differently is the failure `scripts/lib/licensing.mjs` exists to prevent.
const INDEX = join(ROOT, 'licensing', 'third-party-index.json');
if (!existsSync(INDEX)) {
  throw new Error(
    'licensing/third-party-index.json is missing. Run `node scripts/gen_third_party_licenses.mjs` ' +
      'first: the SBOM reports the licence expressions that artifact resolved.',
  );
}
const spdxByPurl = new Map(
  JSON.parse(readFileSync(INDEX, 'utf8')).components.map((c) => [c.purl, c.spdx]),
);

function toComponent(c, closure) {
  const licenses = licenseNode(spdxByPurl.has(c.purl) ? spdxByPurl.get(c.purl) : c.declaredLicense);
  return {
    type: 'library',
    'bom-ref': c.purl,
    name: c.name,
    version: c.version,
    ...(c.description ? { description: c.description } : {}),
    // `required` — distributed with the product. `excluded` — installed but no bytes in the output.
    scope: c.distributed ? 'required' : 'excluded',
    ...(licenses ? { licenses } : {}),
    purl: c.purl,
    ...(c.repository ? { externalReferences: [{ type: 'vcs', url: c.repository }] } : {}),
    properties: [
      { name: 'anima:closure', value: closure },
      { name: 'anima:origin', value: c.origin },
    ],
  };
}

const cargo = cargoInventory(ROOT);
const npm = npmInventory(ROOT);

const components = [
  ...cargo.components.map((c) => toComponent(c, 'cargo-desktop')),
  ...npm.components.map((c) => toComponent(c, c.distributed ? 'npm-bundle' : 'npm-install-only')),
].sort((a, b) => byteCompare(a.purl, b.purl));

// A duplicate identity makes every reference in `dependencies` ambiguous, and a scanner resolving
// one would silently pick whichever it saw first.
const refs = new Set();
for (const c of components) {
  if (refs.has(c['bom-ref'])) {
    throw new Error(`duplicate bom-ref ${c['bom-ref']}; the SBOM would be ambiguous`);
  }
  refs.add(c['bom-ref']);
}

// ---- dependency graph -------------------------------------------------------------------------
//
// Two graphs joined under one root. Refs are purls, so a cargo edge and an npm edge cannot collide.
const APP_REF = 'anima-engine';
const purlOf = new Map();
for (const c of cargo.components) purlOf.set(`${c.name}@${c.version}`, c.purl);
for (const c of npm.components) purlOf.set(`npm:${c.name}@${c.version}`, c.purl);

const resolveCargo = (keys) => keys.map((k) => purlOf.get(k)).filter((p) => p !== undefined);
const resolveNpm = (keys) => keys.map((k) => purlOf.get(`npm:${k}`)).filter((p) => p !== undefined);

const dependencies = [
  {
    ref: APP_REF,
    dependsOn: [...resolveCargo(cargo.rootDeps), ...resolveNpm(npm.rootDeps)].sort(byteCompare),
  },
];
for (const c of cargo.components) {
  dependencies.push({
    ref: c.purl,
    dependsOn: resolveCargo(cargo.edges.get(`${c.name}@${c.version}`) ?? []).sort(byteCompare),
  });
}
for (const c of npm.components) {
  dependencies.push({
    ref: c.purl,
    dependsOn: resolveNpm(npm.edges.get(`${c.name}@${c.version}`) ?? []).sort(byteCompare),
  });
}
dependencies.sort((a, b) => byteCompare(a.ref, b.ref));

// Every `dependsOn` must name something the document defines, or a consumer walking the graph hits
// a dangling reference and has to guess whether the component was omitted or the edge was wrong.
const known = new Set([APP_REF, ...refs]);
for (const d of dependencies) {
  for (const target of d.dependsOn) {
    if (!known.has(target)) throw new Error(`dependency ${d.ref} -> ${target} names no component`);
  }
}

// ---- assemble ---------------------------------------------------------------------------------

const rootPkg = JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8'));
const body = {
  bomFormat: 'CycloneDX',
  specVersion: '1.5',
  version: 1,
  metadata: {
    // Deliberately no `timestamp` — see the determinism note at the top of this file.
    component: {
      type: 'application',
      'bom-ref': APP_REF,
      name: 'anima-engine',
      version: rootPkg.version ?? '0.0.0',
      description:
        'Real-time GPU-accelerated Artificial Life & Evolution simulator (Tauri v2 desktop app).',
      licenses: [{ expression: 'MIT OR Apache-2.0' }],
    },
    properties: [
      { name: 'anima:cargo:features', value: 'desktop' },
      { name: 'anima:cargo:edges', value: 'normal (excludes dev- and build-dependencies)' },
      {
        name: 'anima:cargo:target',
        value: `host target of the generating machine (${process.platform}); a build for another OS links a different set`,
      },
      {
        name: 'anima:npm:bundle',
        value: 'packages with rendered bytes in dist/, measured by the bundler (licensing/bundle-closure.json)',
      },
      {
        name: 'anima:npm:install-only',
        value: 'installed for production but with no bytes in dist/; scope=excluded, not distributed',
      },
      {
        name: 'anima:scope:note',
        value:
          'scope=required means the component is distributed inside the desktop binary. Workspace ' +
          'members are the subject of this BOM (metadata.component), not entries in it.',
      },
    ],
  },
  components,
  dependencies,
};

// The serial identifies the document, so it is hashed over the document — with the field itself
// absent, since it cannot be an input to its own value.
const serialNumber = deterministicSerialNumber(JSON.stringify(body));
const rendered = `${JSON.stringify({ ...body, serialNumber }, null, 2)}\n`;

const distributed = components.filter((c) => c.scope === 'required').length;
if (CHECK) {
  const current = existsSync(OUT) ? readFileSync(OUT, 'utf8') : '';
  if (current !== rendered) {
    console.error(
      'sbom.cdx.json is out of date. Regenerate with `node scripts/gen_sbom.mjs` and commit it.',
    );
    process.exit(1);
  }
  console.log(
    `SBOM check: up to date (${components.length} components, ${distributed} distributed, ` +
      `${dependencies.length} dependency records)`,
  );
} else {
  writeFileSync(OUT, rendered);
  const cargoCount = cargo.components.length;
  console.log(
    `wrote sbom.cdx.json — ${components.length} components (${cargoCount} cargo, ` +
      `${components.length - cargoCount} npm), ${distributed} distributed, ` +
      `${dependencies.length} dependency records`,
  );
}
