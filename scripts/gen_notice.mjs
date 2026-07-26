#!/usr/bin/env node
// Generate NOTICE from the dependency graph that is actually distributed.
//
// # Why this is generated rather than written
//
// `LICENSE` is proprietary and grants nothing. That does not remove the obligation to attribute the
// permissive components shipped inside the binary — MIT and BSD require the copyright notice and
// licence text to travel with the distribution, and Apache-2.0 adds a NOTICE-propagation clause.
// There was no NOTICE at all, so every one of those obligations was unmet.
//
// # Why it reads the graph and not Cargo.toml
//
// Attribution follows what is *distributed*, which is the transitive closure, not the handful of
// direct dependencies a human would think to list. It also has to follow the shipped feature set:
// `--features desktop` is what `npm run tauri:build` passes, and it pulls in crates a default build
// does not. Reading `Cargo.toml` would produce a NOTICE that is confidently wrong in both
// directions.
//
// `cargo tree -e normal` is the source of truth for "what links into the binary": `-e normal`
// excludes dev-dependencies (test-only) and build-dependencies (not distributed). Licences then come
// from `cargo metadata`, which reads each crate's own manifest.
//
// npm production dependencies are included because they are bundled into `dist/` by Vite and ship
// inside the app. devDependencies are not.
//
// Usage: node scripts/gen_notice.mjs [--check]
//   --check  exit 1 if NOTICE is out of date instead of rewriting it (for CI)

import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = process.cwd();
const CHECK = process.argv.includes('--check');
const SRC_TAURI = join(ROOT, 'src-tauri');

function run(cmd, args, cwd) {
  // `shell: true` on Windows: `npm` is `npm.cmd` there, and `execFileSync` will not find a `.cmd`
  // without going through the shell (ENOENT with an empty stdout, which is indistinguishable from
  // a command that produced nothing).
  return execFileSync(cmd, args, {
    cwd,
    encoding: 'utf8',
    maxBuffer: 1 << 28,
    shell: process.platform === 'win32',
  });
}

// ---- Rust: what links into the desktop binary -------------------------------------------------
const treeOut = run(
  'cargo',
  ['tree', '--features', 'desktop', '-e', 'normal', '--prefix', 'none', '--no-dedupe'],
  SRC_TAURI,
);

// Lines look like `serde v1.0.200` or `serde v1.0.200 (proc-macro)`; workspace members appear too.
const shipped = new Set();
for (const line of treeOut.split(/\r?\n/)) {
  const m = /^([A-Za-z0-9_.-]+)\s+v([0-9][^\s]*)/.exec(line.trim());
  if (m) shipped.add(`${m[1]} ${m[2]}`);
}

// `--features desktop` must match the tree command above. Without it, `cargo metadata` resolves the
// DEFAULT graph: the optional `neo4j`/`networking` crates are absent entirely, and shared
// dependencies can resolve to different versions (measured: `phf` is 0.13.1 in the default graph and
// 0.11.3 under `desktop`). Both make the version key miss, and a miss silently becomes "UNKNOWN
// licence" — 18 crates were reported as unlicensed for exactly this reason, all of them in the
// feature-gated subtree.
const meta = JSON.parse(
  run('cargo', ['metadata', '--format-version', '1', '--features', 'desktop'], SRC_TAURI),
);
const licenseOf = new Map();
const repoOf = new Map();
for (const p of meta.packages) {
  licenseOf.set(`${p.name} ${p.version}`, p.license ?? (p.license_file ? `see ${p.license_file}` : null));
  repoOf.set(`${p.name} ${p.version}`, p.repository ?? null);
}

// Workspace members are first-party and are covered by LICENSE, not attributed to a third party.
const firstParty = new Set(
  meta.workspace_members.map((id) => {
    const p = meta.packages.find((q) => q.id === id);
    return p ? `${p.name} ${p.version}` : id;
  }),
);

const rustCrates = [...shipped]
  .filter((k) => !firstParty.has(k))
  .map((k) => ({ key: k, license: licenseOf.get(k) ?? 'UNKNOWN', repo: repoOf.get(k) ?? '' }))
  .sort((a, b) => a.key.localeCompare(b.key));

// ---- npm: what Vite bundles into dist/ --------------------------------------------------------
//
// The **production closure**, not the direct dependency list.
//
// This used to read `package.json.dependencies` and stop there — eight names. Vite bundles the
// transitive graph, so the components actually shipping inside `dist/` include everything those
// eight pull in: `scheduler` (React's cooordinator), `@pixi/*`, `earcut`, `eventemitter3`,
// `zustand`, `react-reconciler`, and so on. Every one of them carries the same attribution
// obligation as the package that named it, and none of them appeared.
//
// `npm ls --omit=dev --all --json` walks the installed tree from the production roots, which is
// the same closure the bundler traverses. Deduplicated on exact `name@version`, because npm's tree
// repeats a shared dependency under each dependent and a NOTICE listing `scheduler` eleven times
// is not more attributive.
const npmPackages = [];
{
  let tree;
  try {
    // `npm ls` exits non-zero on peer-dependency complaints while still emitting valid JSON, so the
    // output is used and the exit code is not.
    tree = JSON.parse(
      run('npm', ['ls', '--omit=dev', '--all', '--json', '--long'], ROOT),
    );
  } catch (e) {
    if (!e.stdout) throw e;
    tree = JSON.parse(e.stdout);
  }

  const seen = new Map();
  const walk = (node) => {
    for (const [name, dep] of Object.entries(node.dependencies ?? {})) {
      // A deduped node carries no version of its own; the resolved copy appears elsewhere in the
      // tree, so skipping it here loses nothing.
      if (dep.version) {
        const key = `${name} ${dep.version}`;
        if (!seen.has(key)) {
          seen.set(key, {
            key,
            license: normaliseLicense(dep),
            repo: normaliseRepo(dep),
          });
        }
      }
      walk(dep);
    }
  };
  walk(tree);
  npmPackages.push(...[...seen.values()].sort((a, b) => a.key.localeCompare(b.key)));
}

/** npm licence fields come in three historical shapes; all three still occur in a real tree. */
function normaliseLicense(m) {
  if (typeof m.license === 'string') return m.license;
  if (Array.isArray(m.licenses)) return m.licenses.map((l) => l.type ?? l).join(' OR ');
  if (m.license?.type) return m.license.type;
  return 'UNKNOWN';
}

function normaliseRepo(m) {
  if (typeof m.repository === 'string') return m.repository;
  return m.repository?.url ?? m.homepage ?? '';
}

// ---- group ------------------------------------------------------------------------------------
function group(items) {
  const byLicense = new Map();
  for (const it of items) {
    if (!byLicense.has(it.license)) byLicense.set(it.license, []);
    byLicense.get(it.license).push(it);
  }
  return [...byLicense.entries()].sort((a, b) => b[1].length - a[1].length);
}

function section(title, items) {
  if (items.length === 0) return `### ${title}\n\n_none_\n`;
  let out = `### ${title} — ${items.length} component(s)\n`;
  for (const [license, list] of group(items)) {
    out += `\n#### ${license} (${list.length})\n\n`;
    for (const it of list) out += `- ${it.key}${it.repo ? ` — ${it.repo}` : ''}\n`;
  }
  return out;
}

const unknownRust = rustCrates.filter((c) => c.license === 'UNKNOWN' || c.license === null);
const unknownNpm = npmPackages.filter((c) => c.license === 'UNKNOWN' || c.license === 'NOT INSTALLED');

const body = `# NOTICE

Anima-Engine itself is proprietary; see [LICENSE](LICENSE), which grants no rights and defines the
scope of what is and is not covered.

This file is the separate obligation. Anima-Engine is **distributed** with third-party components
under permissive licences, and MIT/BSD require their copyright notice and licence text to travel
with the distribution while Apache-2.0 adds a NOTICE-propagation clause. Being proprietary does not
remove that; it is why a proprietary product still ships a NOTICE.

**This file is generated — do not edit by hand.** Regenerate with \`node scripts/gen_notice.mjs\`,
and verify in CI with \`node scripts/gen_notice.mjs --check\`.

## What is counted, and why

- **Rust:** every crate reachable by \`cargo tree --features desktop -e normal\`. \`desktop\` is the
  feature set \`npm run tauri:build\` passes, so it is what ships. \`-e normal\` excludes
  dev-dependencies (test-only, not distributed) and build-dependencies (run at build time, not
  linked). The full lockfile is a superset and would over-attribute; \`Cargo.toml\`'s direct
  dependencies are a subset and would under-attribute.
- **npm:** the **production closure** — \`npm ls --omit=dev --all\` from the \`dependencies\` roots,
  deduplicated on exact name+version. Vite bundles the transitive graph into \`dist/\` and \`dist/\`
  is packaged into the desktop binary, so a component's obligation does not depend on whether a
  human happened to name it in \`package.json\`. Listing only the direct dependencies attributed 8
  packages and omitted everything they pull in. \`devDependencies\` are not distributed.
- **Excluded:** workspace members (\`anima-engine\`, \`anima-domain\`) — first-party, covered by
  \`LICENSE\`.

Version numbers are the resolved ones from the lockfiles at generation time. Licence strings are the
SPDX expressions each component declares in its own manifest; where a component declares a dual
licence (commonly \`MIT OR Apache-2.0\` in the Rust ecosystem) the choice is the distributor's.

## Rust components

${section('Crates linked into the desktop binary', rustCrates)}

## JavaScript components

${section('Packages bundled into dist/', npmPackages)}

## Gaps

${
  unknownRust.length + unknownNpm.length === 0
    ? 'Every component above declares a licence. No gaps.'
    : `The following declare no machine-readable licence and need a manual determination before any\ndistribution:\n\n${[...unknownRust, ...unknownNpm].map((c) => `- ${c.key}`).join('\n')}`
}

## What this file does NOT establish

- It is an **inventory**, not a legal review. Compatibility of each licence with proprietary
  distribution has not been assessed here.
- Licence **texts** are not reproduced. MIT and BSD require the text to accompany the distribution;
  the packaging step must copy each component's licence file, and it currently does not. That is an
  open item, recorded rather than implied.
- Copyright holders are not listed individually, only the components and their declared licences.
`;

const NOTICE = join(ROOT, 'NOTICE');
if (CHECK) {
  const current = existsSync(NOTICE) ? readFileSync(NOTICE, 'utf8') : '';
  if (current !== body) {
    console.error(
      'NOTICE is out of date with the dependency graph. Run `node scripts/gen_notice.mjs`.',
    );
    process.exit(1);
  }
  console.log(
    `NOTICE check: up to date (${rustCrates.length} crates, ${npmPackages.length} npm packages)`,
  );
} else {
  writeFileSync(NOTICE, body);
  console.log(
    `wrote NOTICE — ${rustCrates.length} Rust crates, ${npmPackages.length} npm packages, ` +
      `${unknownRust.length + unknownNpm.length} without a declared licence`,
  );
}
