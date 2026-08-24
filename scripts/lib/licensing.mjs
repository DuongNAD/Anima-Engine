// The one inventory of what Anima-Engine distributes, and where each component's licence text lives.
//
// # Why this is a library and not three copies
//
// `gen_notice.mjs`, `gen_sbom.mjs` and `gen_third_party_licenses.mjs` all answer "what ships".
// They had two independent implementations of that question and were already drifting: the SBOM used
// a NUL byte to join name and version while NOTICE used a space, and neither could see that the two
// npm closures they were describing were not the same set. Three artifacts that disagree about the
// dependency graph are worse than one, because each looks authoritative on its own.
//
// # The three closures, which are genuinely different sets
//
//   cargo-desktop      Every crate reachable by `cargo tree --features desktop -e normal`. Linked
//                      into the binary, so distributed. 419 crates.
//   npm-bundled        Packages with rendered bytes in `dist/`, measured by the bundler itself and
//                      committed to `licensing/bundle-closure.json`. Distributed. 21 components.
//   npm-install-only   In the production install closure but with no bytes in `dist/`. `node_modules`
//                      is not shipped, so these are NOT distributed. 18 components.
//
// The previous artifacts used the install closure as if it were the ship closure. That was wrong in
// both directions at once: 18 packages attributed that ship nothing, and three that do ship —
// `vite`, `rolldown` and `@oxc-project/runtime`, whose code the bundler compiles into the output —
// missing entirely because they are not production dependencies.
//
// # Host-target caveat, stated rather than hidden
//
// `cargo tree` resolves for the **host target**. This repository's desktop target is Windows and CI
// runs the Rust job on `windows-latest`, so the committed artifacts describe a Windows build. A
// macOS or Linux build links a different set (`objc2-*` and `gtk` rather than `windows-*`), and
// regenerating there would produce a different, equally correct, inventory. That is a property of
// the product, not a defect in the generator; it is recorded so nobody reads these files as
// platform-neutral.

import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, readdirSync, realpathSync } from 'node:fs';
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * Filename shapes that carry licence or attribution text.
 *
 * Anchored at both ends, because an unanchored `licen[cs]e` also matches `relicense.md` and
 * `NOTICES.tsx`. The suffix group covers the real spread measured across 419 crates: `LICENSE-MIT`,
 * `LICENSE.txt`, `LICENSE_APACHE-2.0`, `license-apache-2.0`, `LICENSE.MIT`.
 */
const LICENSE_NAME_RE = /^(LICEN[CS]E|COPYING|COPYRIGHT|NOTICE|UNLICENSE)([-._][A-Za-z0-9.+-]*)?$/i;

/**
 * Anchoring is not enough on its own, because the suffix group has to allow a dot — `LICENSE.md`
 * and `LICENSE.txt` are both common — and that lets `LICENSE-check.py` through. Packages really do
 * ship such files, and the result would be a compliance document with someone's build script
 * printed in it under the heading of a licence grant.
 */
const SOURCE_EXTENSION_RE =
  /\.(py|js|mjs|cjs|jsx|ts|tsx|rs|sh|bash|ps1|bat|json|ya?ml|toml|lock|c|h|cc|cpp|hpp|go|rb|java|cs|php|pl|lua|html?|xml|svg)$/i;

/** The one place that decides whether a filename is licence text. */
export function isLicenseFileName(name) {
  return LICENSE_NAME_RE.test(name) && !SOURCE_EXTENSION_RE.test(name);
}

const CARGO_FEATURES = ['--features', 'desktop'];

function sh(cwd) {
  return {
    cwd,
    encoding: 'utf8',
    maxBuffer: 1 << 28,
    // `npm` is `npm.cmd` on Windows and `execFileSync` will not find a `.cmd` without a shell.
    shell: process.platform === 'win32',
  };
}

export function sha256hex(buf) {
  return createHash('sha256').update(buf).digest('hex');
}

/** Byte order, never `localeCompare`: locale collation orders punctuation differently from a byte
 *  comparison, so a locale-sorted file disagrees with every consumer that sorts the obvious way. */
export function byteCompare(a, b) {
  return a < b ? -1 : a > b ? 1 : 0;
}

/**
 * A directory entry that is safe to read as licence text.
 *
 * Exported so it can be tested against hand-built entries: creating a real symlink needs
 * privileges this project's Windows machine does not have, and a gate that can only be verified on
 * one platform is not a gate.
 *
 * `isFile()` is the symlink defence and it is not incidental — `readdirSync(…, {withFileTypes:true})`
 * uses `lstat` semantics, so a symlink reports `isSymbolicLink()`, never `isFile()`. A licence
 * "file" that is really a link to `/etc/shadow` or to a file outside the package therefore never
 * reaches a read. The name checks reject traversal attempts that a hostile archive could put in a
 * directory listing.
 */
export function isSafeLicenseEntry(entry) {
  if (!entry.isFile()) return false;
  const name = entry.name;
  if (name === '.' || name === '..') return false;
  if (name.includes('/') || name.includes('\\') || name.includes('\0')) return false;
  return isLicenseFileName(name);
}

function isInside(parentReal, childReal) {
  const rel = relative(parentReal, childReal);
  return rel !== '' && !rel.startsWith('..') && !isAbsolute(rel);
}

/**
 * Decode licence bytes for embedding, and say whether the decode was lossless.
 *
 * Two transforms, both recorded in the generated artifact: a UTF-8 BOM is dropped and CRLF/CR are
 * normalised to LF. Neither changes a word of the licence, and without them the committed bundle
 * would differ between a Windows checkout and a Linux one — the freshness gate would then be
 * reporting the checkout rather than the dependency graph. The **source** bytes are still hashed
 * unmodified, so the index remains verifiable against the installed package.
 *
 * A file that is not valid UTF-8 is reported rather than transcoded. Emitting replacement
 * characters into a licence would be a silent corruption of the exact text the licence requires be
 * reproduced.
 */
export function decodeLicenseText(buf) {
  const text = buf.toString('utf8');
  const lossless = Buffer.from(text, 'utf8').equals(buf);
  let out = text.charCodeAt(0) === 0xfeff ? text.slice(1) : text;
  out = out.replace(/\r\n?/g, '\n');
  if (out.length > 0 && !out.endsWith('\n')) out += '\n';
  return { text: out, lossless };
}

/**
 * Read every licence-bearing file in a package directory.
 *
 * Returns `[]` for a directory that does not exist — an unpacked-source directory can legitimately
 * be absent — and the caller decides whether that is a blocker. Results are sorted by filename in
 * byte order so the generated artifacts do not depend on filesystem enumeration order, which is not
 * stable across platforms.
 */
export function collectLicenseTexts(dir) {
  let entries;
  let realDir;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
    realDir = realpathSync(dir);
  } catch {
    return [];
  }

  const out = [];
  for (const entry of entries) {
    if (!isSafeLicenseEntry(entry)) continue;
    const full = join(dir, entry.name);
    let real;
    try {
      real = realpathSync(full);
    } catch {
      continue;
    }
    // A Windows directory junction inside the package would let a path that passed the name checks
    // still resolve outside it. Both sides are real paths, so this comparison is not defeated by
    // the package directory itself living under a junction — which, on this machine, it does.
    if (!isInside(realDir, real)) continue;

    const bytes = readFileSync(full);
    const { text, lossless } = decodeLicenseText(bytes);
    out.push({
      filename: entry.name,
      sourceSha256: sha256hex(bytes),
      sourceBytes: bytes.length,
      text,
      textSha256: sha256hex(Buffer.from(text, 'utf8')),
      lossless,
    });
  }
  return out.sort((a, b) => byteCompare(a.filename, b.filename));
}

// ---- cargo ------------------------------------------------------------------------------------

/**
 * The crates linked into the desktop binary, with their declared licence and a dependency graph.
 *
 * `--features desktop` is passed to **both** commands and that is load-bearing. Without it
 * `cargo metadata` resolves the default graph, where the optional `neo4j`/`networking` crates are
 * absent and shared dependencies can resolve to different versions — measured: `phf` is 0.13.1 in
 * the default graph and 0.11.3 under `desktop`. Every mismatch became an "UNKNOWN licence", which
 * is how 18 crates were once reported as unlicensed.
 */
export function cargoInventory(root) {
  const srcTauri = join(root, 'src-tauri');
  const treeOut = execFileSync(
    'cargo',
    ['tree', ...CARGO_FEATURES, '-e', 'normal', '--prefix', 'none', '--no-dedupe'],
    sh(srcTauri),
  );

  // Lines look like `serde v1.0.200` or `serde v1.0.200 (proc-macro)`.
  const shipped = new Set();
  for (const line of treeOut.split(/\r?\n/)) {
    const m = /^([A-Za-z0-9_.-]+)\s+v([0-9][^\s]*)/.exec(line.trim());
    if (m) shipped.add(`${m[1]}@${m[2]}`);
  }

  const meta = JSON.parse(
    execFileSync('cargo', ['metadata', '--format-version', '1', ...CARGO_FEATURES], sh(srcTauri)),
  );

  const idOf = new Map();
  const byKey = new Map();
  for (const p of meta.packages) {
    const key = `${p.name}@${p.version}`;
    byKey.set(key, p);
    idOf.set(p.id, key);
  }
  // Workspace members are first-party — the subject of the bill, not entries in it.
  const firstParty = new Set(meta.workspace_members.map((id) => idOf.get(id) ?? id));

  const components = [];
  for (const key of [...shipped].sort(byteCompare)) {
    if (firstParty.has(key)) continue;
    const p = byKey.get(key);
    if (!p) {
      throw new Error(
        `cargo tree lists ${key} but cargo metadata does not resolve it. The two commands ` +
          `disagree about the graph; do not publish an inventory from this state.`,
      );
    }
    components.push({
      ecosystem: 'cargo',
      name: p.name,
      version: p.version,
      purl: `pkg:cargo/${p.name}@${p.version}`,
      declaredLicense: p.license ?? null,
      declaredLicenseFile: p.license_file ?? null,
      description: p.description ?? null,
      repository: p.repository ?? null,
      distributed: true,
      origin: 'cargo-registry',
      // Absolute, machine-specific, and deliberately never emitted into an artifact.
      sourceDir: dirname(p.manifest_path),
    });
  }

  // `resolve` carries every target's dependencies, while `cargo tree` is filtered to the host
  // target. Edges to crates outside the shipped set are dropped rather than pulling non-shipped
  // components in through the back door.
  const present = new Set(components.map((c) => `${c.name}@${c.version}`));
  const normalDeps = (node) => {
    const to = [];
    for (const dep of node.deps ?? []) {
      // `kind: null` is a normal dependency. `dev` is test-only and `build` never links.
      if (!dep.dep_kinds?.some((k) => k.kind === null || k.kind === undefined)) continue;
      const target = idOf.get(dep.pkg);
      if (target && present.has(target)) to.push(target);
    }
    return [...new Set(to)].sort(byteCompare);
  };

  const edges = new Map();
  const rootDeps = new Set();
  for (const node of meta.resolve?.nodes ?? []) {
    const from = idOf.get(node.id);
    if (!from) continue;
    if (firstParty.has(from)) {
      // A workspace member's dependencies are the application's own direct dependencies.
      for (const d of normalDeps(node)) rootDeps.add(d);
      continue;
    }
    if (!present.has(from)) continue;
    edges.set(from, normalDeps(node).filter((d) => d !== from));
  }
  return { components, edges, rootDeps: [...rootDeps].sort(byteCompare) };
}

// ---- npm --------------------------------------------------------------------------------------

function readJson(file) {
  return JSON.parse(readFileSync(file, 'utf8'));
}

/**
 * Node's own resolution, walked on disk: `node_modules/<name>` from `fromDir` upwards.
 *
 * Used instead of trusting `npm ls`'s deduped placeholders. A deduped node in that tree carries no
 * version, and resolving it by name alone is exactly the bug this avoids — `scheduler` is installed
 * at **two** versions in this project (0.21.0 under react-reconciler, 0.23.2 under react-dom) and
 * both ship, so a name-keyed lookup would attribute one of them to the other's dependents.
 */
function resolvePackageDir(fromDir, name, stopAt) {
  let dir = fromDir;
  for (;;) {
    const candidate = join(dir, 'node_modules', ...name.split('/'));
    if (existsSync(join(candidate, 'package.json'))) return candidate;
    const parent = dirname(dir);
    if (parent === dir || !isInside(stopAt, dir)) return null;
    dir = parent;
  }
}

/** npm's licence field has three historical shapes and all three still occur in a real tree. */
function normaliseNpmLicense(manifest) {
  if (typeof manifest.license === 'string') return manifest.license;
  if (Array.isArray(manifest.licenses)) {
    const parts = manifest.licenses.map((l) => (typeof l === 'string' ? l : l?.type)).filter(Boolean);
    if (parts.length) return parts.join(' OR ');
  }
  if (typeof manifest.license?.type === 'string') return manifest.license.type;
  return null;
}

function normaliseNpmRepo(manifest) {
  if (typeof manifest.repository === 'string') return manifest.repository;
  if (typeof manifest.repository?.url === 'string') return manifest.repository.url;
  return typeof manifest.homepage === 'string' ? manifest.homepage : null;
}

function npmComponent(name, version, dir, extra) {
  const manifest = dir && existsSync(join(dir, 'package.json')) ? readJson(join(dir, 'package.json')) : {};
  return {
    ecosystem: 'npm',
    name,
    version,
    // purl keeps the scope as a namespace segment: `@react-three/fiber` is `%40react-three/fiber`.
    purl: `pkg:npm/${name.replace('@', '%40')}@${version}`,
    declaredLicense: normaliseNpmLicense(manifest),
    declaredLicenseFile: null,
    description: typeof manifest.description === 'string' ? manifest.description : null,
    repository: normaliseNpmRepo(manifest),
    sourceDir: dir,
    ...extra,
  };
}

/**
 * The npm side of the bill: the production install closure, and the subset of it with bytes in
 * `dist/`, kept as distinct facts rather than merged into one number.
 *
 * `licensing/bundle-closure.json` is read from disk rather than recomputed. It is written by the
 * bundler during `npm run build` and committed; `npm run check:bundle-closure` is what proves it
 * still matches a fresh build. Recomputing it here would mean guessing at the module graph, which
 * is the guess that produced the wrong closure in the first place.
 */
export function npmInventory(root) {
  const closurePath = join(root, 'licensing', 'bundle-closure.json');
  if (!existsSync(closurePath)) {
    throw new Error(
      `licensing/bundle-closure.json is missing. It is written by \`npm run build\`; without it ` +
        `the npm distribution boundary is unknown and no attribution artifact may be generated.`,
    );
  }
  const closure = readJson(closurePath);
  if (Array.isArray(closure.unresolved) && closure.unresolved.length > 0) {
    throw new Error(
      `The last build shipped modules that could not be resolved to a component:\n  ` +
        `${closure.unresolved.join('\n  ')}\nUnattributed shipped code cannot pass a licensing gate.`,
    );
  }

  const rootReal = realpathSync(root);
  const installed = new Map();
  const edges = new Map();

  /**
   * Walk the production closure on disk, from each package's own manifest.
   *
   * This used to shell out to `npm ls --omit=dev --all --json`, and that tree cannot be walked
   * correctly. npm renders a shared dependency **once** and leaves a placeholder everywhere else:
   * a deduped node is the empty object `{}`, with no version, no path and no `dependencies`. Recurse
   * into the placeholder and the subtree under it vanishes — measured, `js-tokens@4.0.0` disappeared
   * from the closure because `loose-envify` happened to be rendered as a placeholder under the first
   * dependent that reached it.
   *
   * A package's own `package.json` has no such ambiguity, and resolving each name the way Node does
   * — `node_modules/<name>` from the dependent's directory upwards — picks the same copy the bundler
   * picked. That matters here: `scheduler` is installed at **two** versions (0.21.0 under
   * react-reconciler, 0.23.2 under react-dom) and both ship, so a name-keyed lookup would attribute
   * one to the other's dependents.
   *
   * `optionalDependencies` that are absent are skipped — code that is not on disk cannot be in the
   * product. A missing **required** dependency is not skipped: the tree is incomplete and the
   * inventory would under-report, so it stops the run.
   *
   * `peerDependencies` are not followed. A peer is satisfied from the consumer's own tree, and every
   * non-optional peer in this graph (`react`, `react-dom`, `three`) is a root dependency already.
   * Anything a peer contributes to the product shows up in the bundle closure regardless, which is
   * the set that decides what is distributed.
   */
  const rootKey = (() => {
    const m = readJson(join(root, 'package.json'));
    return `${m.name}@${m.version}`;
  })();

  const walk = (dir, manifest) => {
    const self = `${manifest.name}@${manifest.version}`;
    const outgoing = [];
    const required = Object.keys(manifest.dependencies ?? {});
    const optional = new Set(Object.keys(manifest.optionalDependencies ?? {}));
    for (const name of [...new Set([...required, ...optional])].sort(byteCompare)) {
      const depDir = resolvePackageDir(dir, name, rootReal);
      if (!depDir) {
        if (optional.has(name)) continue;
        throw new Error(
          `${name} is a dependency of ${self} but is not installed anywhere Node would look for ` +
            `it. Run \`npm ci\`; an incomplete tree cannot be inventoried.`,
        );
      }
      const childManifest = readJson(join(depDir, 'package.json'));
      const key = `${name}@${childManifest.version}`;
      outgoing.push(key);
      if (!installed.has(key)) {
        installed.set(key, npmComponent(name, childManifest.version, depDir, {}));
        walk(depDir, childManifest);
      }
    }
    edges.set(self, [...new Set(outgoing)].sort(byteCompare));
  };
  walk(rootReal, readJson(join(root, 'package.json')));

  // Bundled packages that the install closure cannot see: toolchain code the bundler compiles in.
  const components = [];
  const bundled = new Map();
  for (const pkg of closure.packages) {
    const key = `${pkg.name}@${pkg.version}`;
    bundled.set(key, pkg);
    if (!installed.has(key)) {
      const dir = join(root, 'node_modules', ...pkg.name.split('/'));
      installed.set(key, npmComponent(pkg.name, pkg.version, existsSync(dir) ? dir : null, {}));
    }
  }

  for (const [key, component] of installed) {
    const pkg = bundled.get(key);
    components.push({
      ...component,
      distributed: pkg !== undefined,
      origin: pkg?.origin === 'injected' ? 'toolchain-injected' : 'node_modules',
    });
  }
  components.sort((a, b) => byteCompare(a.name, b.name) || byteCompare(a.version, b.version));
  // Toolchain-injected components have no dependent in the install graph — the bundler put them
  // there — so they hang off the application directly, like the direct dependencies they behave as.
  const rootDeps = [
    ...new Set([
      ...(edges.get(rootKey) ?? []),
      ...components.filter((c) => c.origin === 'toolchain-injected').map((c) => `${c.name}@${c.version}`),
    ]),
  ].sort(byteCompare);
  edges.delete(rootKey);
  return { components, edges, rootDeps };
}

// ---- SPDX -------------------------------------------------------------------------------------

/**
 * The official SPDX licence list, read from the CycloneDX schema vendored beside it.
 *
 * Located relative to this file rather than to `process.cwd()`: the generators are run from the
 * repository root, but a test importing this module is not, and a cwd-relative read would make the
 * SPDX set silently empty there — every licence would classify as free text and every assertion
 * about classification would pass for the wrong reason.
 */
export const SCHEMA_DIR = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'schemas', 'cyclonedx');
export const SPDX_IDS = new Set(
  JSON.parse(readFileSync(join(SCHEMA_DIR, 'spdx.schema.json'), 'utf8')).enum ?? [],
);

/**
 * A deliberately conservative SPDX expression check: every operand must be a known SPDX id, or an
 * id with the `+` "or later" suffix, or the operand of a `WITH` (which names an exception from a
 * list this schema does not carry, so it is accepted unchecked). Anything else is not an expression.
 */
export function isSpdxExpression(value) {
  const tokens = value.replace(/[()]/g, ' ').split(/\s+/).filter(Boolean);
  if (tokens.length < 2) return false;
  let expectException = false;
  let sawOperand = false;
  for (const token of tokens) {
    if (expectException) {
      expectException = false;
      sawOperand = true;
      continue;
    }
    if (token === 'AND' || token === 'OR') {
      if (!sawOperand) return false;
      sawOperand = false;
      continue;
    }
    if (token === 'WITH') {
      if (!sawOperand) return false;
      expectException = true;
      continue;
    }
    const bare = token.endsWith('+') ? token.slice(0, -1) : token;
    if (!SPDX_IDS.has(bare)) return false;
    sawOperand = true;
  }
  return sawOperand && !expectException;
}

/**
 * CycloneDX distinguishes three ways to say what a licence is, and picking the wrong one loses
 * information that policy engines act on.
 *
 *   `license.id`   a single identifier from the SPDX list — the strongest form, machine-comparable
 *   `expression`   a compound SPDX expression such as `MIT OR Apache-2.0`
 *   `license.name` free text, for anything that is not valid SPDX
 *
 * The third case is real and must not be laundered into the second: five `unic-*` crates declare
 * `MIT/Apache-2.0`, a separator SPDX has never had. Recording that as an expression asserts a
 * conformance the string does not have; recording it as a name says exactly what was found. The
 * previous implementation classified on a character class, so `MIT/Apache-2.0` — and every other
 * malformed string made only of licence-ish characters — became an `expression`.
 */
export function licenseNode(raw) {
  if (!raw) return undefined;
  const value = raw.trim();
  if (value === '' || value === 'UNKNOWN') return undefined;
  if (SPDX_IDS.has(value)) return [{ license: { id: value } }];
  return isSpdxExpression(value) ? [{ expression: value }] : [{ license: { name: value } }];
}

// ---- provenance -------------------------------------------------------------------------------

/**
 * A machine-independent description of where a licence file was read from.
 *
 * Absolute paths cannot go into a committed artifact: the cargo registry directory carries a
 * per-machine hash (`index.crates.io-1949cf8c6b5b557f`), and the repository root differs on every
 * checkout. Both are stripped to the part that identifies the package, so the same graph produces
 * the same bytes on a Windows dev machine and a CI runner. Separators are normalised to `/` for
 * the same reason.
 */
export function provenancePath(component, filename, root) {
  const posix = (p) => p.split(sep).join('/');
  if (component.ecosystem === 'cargo') {
    return `cargo-registry:${component.name}-${component.version}/${filename}`;
  }
  if (!component.sourceDir) return `npm:${component.name}@${component.version}/${filename}`;
  // Resolved through symlinks on both sides. `npmInventory` walks from `realpathSync(root)`, so a
  // checkout reached through a Windows junction — which is how this machine's drives are laid out —
  // would otherwise compare a real path against a virtual one, every relative path would escape
  // upwards, and every npm entry would silently fall back to the coarse form on that machine only.
  let base = resolve(root);
  try {
    base = realpathSync(base);
  } catch {
    /* a root that cannot be resolved is the caller's problem, not this function's */
  }
  const rel = relative(base, component.sourceDir);
  return rel.startsWith('..') || isAbsolute(rel)
    ? `npm:${component.name}@${component.version}/${filename}`
    : `${posix(rel)}/${filename}`;
}

/**
 * A UUID derived from the BOM's own content, so the same graph always yields the same serial.
 *
 * CycloneDX wants a `serialNumber` that identifies the document, and every generator reaches for a
 * random v4 UUID. A random serial makes the file differ on every run, which breaks the `--check`
 * freshness gate for reasons that have nothing to do with dependencies — the same reason
 * `metadata.timestamp` is omitted. Hashing the content instead keeps the field's meaning (two
 * documents with the same serial *are* the same bill) and keeps the bytes reproducible. Version and
 * variant bits are set so the result is a well-formed UUID, which the schema's pattern requires.
 */
export function deterministicSerialNumber(canonicalContent) {
  const h = createHash('sha256').update(canonicalContent).digest();
  const b = Buffer.from(h.subarray(0, 16));
  b[6] = (b[6] & 0x0f) | 0x50; // version 5 — name-based, which is what a content hash is
  b[8] = (b[8] & 0x3f) | 0x80; // RFC 4122 variant
  const hex = b.toString('hex');
  return `urn:uuid:${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}
