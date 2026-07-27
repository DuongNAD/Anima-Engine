// The vendored upstream licence store, and the rules that make trusting it safe.
//
// # Why a store exists at all
//
// 32 distributed components declared a licence and shipped no copy of its text. Nothing in the
// install tree could close that: the text is simply not in the published artifact. It *is* in the
// upstream repository, at the revision the release was cut from — so the store holds those bytes,
// and this module is the part that refuses to let them be trusted for the wrong reasons.
//
// The failure this guards against is not a missing file; a missing file is loud. It is a file that
// is present, plausible, and wrong: a text fetched from `main` months after the release, a text
// mapped to the wrong version, a text edited after it was vendored, a mapping left behind after the
// upstream started shipping its own licence. Each of those produces a compliance document that looks
// exactly like a correct one. So every check here is fail-closed, and a violation throws rather than
// warns — a licence bundle generated from a store that half-verified is worth less than none.
//
// # The layout is the provenance
//
//   licensing/upstream/github.com/<owner>/<repo>/<commit>/<path-in-repo>
//   https://raw.githubusercontent.com/<owner>/<repo>/<commit>/<path-in-repo>
//
// The store path is the tail of the raw URL, byte for byte. That is not a convention for
// tidiness — it means location and provenance cannot drift apart, because they are the same string.
// A manifest entry whose `url` and `id` disagree is rejected before its bytes are read, and the
// pinned ref segment must be a 40-hex commit, so `main`, `master`, `trunk` and `HEAD` cannot be
// spelled at all.
//
// # What is deliberately *not* here
//
// No network. This module reads the committed store and nothing else, so CI and the release gates
// are offline by construction. `scripts/verify_upstream_licenses.mjs` is the opt-in half that
// re-fetches from the pinned URLs and compares; it never writes back.

import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { lstatSync, readFileSync, realpathSync } from 'node:fs';
import { isAbsolute, join, relative, resolve, sep } from 'node:path';

export const STORE_REL = 'licensing/upstream';
export const MANIFEST_REL = 'licensing/upstream/sources.json';

/** The manifest shape this module understands. A newer store must not be read by an older reader. */
export const SUPPORTED_SCHEMA_VERSION = 1;

const SHA256_RE = /^[0-9a-f]{64}$/;
const COMMIT_RE = /^[0-9a-f]{40}$/;
const DATE_RE = /^\d{4}-\d{2}-\d{2}$/;
const RAW_PREFIX = 'https://raw.githubusercontent.com/';

/**
 * What a vendored file is being packaged *as*, kept explicit so the bundle cannot imply more than
 * the bytes support.
 *
 *   licence-text       a licence grant — the text the licence requires be reproduced
 *   notice             copyright/attribution material: COPYING, COPYRIGHT, NOTICE
 *   licence-statement  the project's own declaration of its licence, for a project that publishes
 *                      no licence file at all. Vendored verbatim rather than replaced by a
 *                      reconstructed text with a guessed copyright holder.
 */
export const MATERIAL_TYPES = new Set(['licence-text', 'notice', 'licence-statement']);

/**
 * How a commit was tied to a released version.
 *
 *   release-tree        the file is in the component's own repository at the commit the release was
 *                       published from. The only kind that needs no argument.
 *   project-repository  the component's repository publishes no copy, and the text comes from the
 *                       same project's primary repository. Requires a written justification and the
 *                       component's own repository/commit, both recorded and checked.
 */
export const PROVENANCE_KINDS = new Set(['release-tree', 'project-repository']);

/** Names that identify a moving target rather than a revision. None may appear as a pinned ref. */
export const MUTABLE_REFS = new Set(['main', 'master', 'trunk', 'HEAD', 'head', 'default', 'latest']);

export function sha256hex(buf) {
  return createHash('sha256').update(buf).digest('hex');
}

const byteCompare = (a, b) => (a < b ? -1 : a > b ? 1 : 0);

/**
 * Split a raw.githubusercontent.com URL, and refuse anything that is not one pinned to a commit.
 *
 * Returns `null` rather than throwing so callers can attribute the failure to a specific manifest
 * entry. The ref segment must be a 40-hex commit: that single rule is what makes a branch name
 * unspellable, and it is checked here rather than by a blocklist, because a blocklist of branch
 * names is a list someone will eventually get around.
 */
export function parseRawUrl(url) {
  if (typeof url !== 'string' || !url.startsWith(RAW_PREFIX)) return null;
  const rest = url.slice(RAW_PREFIX.length);
  const parts = rest.split('/');
  if (parts.length < 4) return null;
  const [owner, repo, commit, ...pathParts] = parts;
  if (!owner || !repo || !COMMIT_RE.test(commit)) return null;
  const pathInRepo = pathParts.join('/');
  if (pathInRepo === '') return null;
  return { owner, repo, commit, pathInRepo };
}

/**
 * Whether a store-relative id is safe to join onto the store root.
 *
 * Rejects absolute paths, drive letters, UNC prefixes, backslashes, NUL, empty segments and any `.`
 * or `..` segment. This runs before the path touches the filesystem, so a traversal attempt is
 * refused rather than resolved-and-then-noticed; `realpathSync` containment is the second layer, for
 * the cases a name check cannot see, such as a directory symlink or a Windows junction.
 */
export function isSafeStorePath(id) {
  if (typeof id !== 'string' || id === '') return false;
  if (id.includes('\0') || id.includes('\\')) return false;
  if (id.startsWith('/') || /^[A-Za-z]:/.test(id)) return false;
  const segments = id.split('/');
  return segments.every((s) => s !== '' && s !== '.' && s !== '..');
}

function fail(problems, message) {
  problems.push(message);
}

/**
 * The set of store files git tracks.
 *
 * An untracked vendored licence is not "present" in any sense that matters: it vanishes on a fresh
 * clone, and every gate that reads the store would then pass against a file nobody can obtain. Same
 * reasoning as `scripts/check_bundle_closure.mjs`, for the same failure mode — the one that looks
 * most like success.
 */
export function trackedStoreFiles(root) {
  const out = execFileSync('git', ['ls-files', '-z', '--', STORE_REL], {
    cwd: root,
    encoding: 'utf8',
    maxBuffer: 1 << 28,
  });
  return new Set(out.split('\0').filter(Boolean));
}

function checkSource(entry, index, root, storeReal, tracked, problems, seenIds) {
  const where = `sources[${index}]`;
  const id = entry?.id;
  if (!isSafeStorePath(id)) {
    fail(problems, `${where}: id ${JSON.stringify(id)} is not a safe relative store path`);
    return null;
  }
  if (seenIds.has(id)) {
    fail(problems, `${where}: duplicate source id ${id}`);
    return null;
  }
  seenIds.add(id);

  const parsed = parseRawUrl(entry.url);
  if (!parsed) {
    fail(
      problems,
      `${where} (${id}): url must be a raw.githubusercontent.com URL pinned to a 40-hex commit, got ` +
        `${JSON.stringify(entry.url)}`,
    );
    return null;
  }
  const expectedId = `github.com/${parsed.owner}/${parsed.repo}/${parsed.commit}/${parsed.pathInRepo}`;
  if (id !== expectedId) {
    fail(problems, `${where} (${id}): store path does not match its url, which tails ${expectedId}`);
  }
  if (entry.commit !== parsed.commit) {
    fail(problems, `${where} (${id}): commit ${entry.commit} disagrees with the url's ${parsed.commit}`);
  }
  if (entry.pathInRepo !== parsed.pathInRepo) {
    fail(problems, `${where} (${id}): pathInRepo disagrees with the url`);
  }
  if (entry.repository !== `https://github.com/${parsed.owner}/${parsed.repo}`) {
    fail(problems, `${where} (${id}): repository disagrees with the url`);
  }
  if (entry.filename !== parsed.pathInRepo.split('/').pop()) {
    fail(problems, `${where} (${id}): filename is not the last segment of pathInRepo`);
  }
  if (!MATERIAL_TYPES.has(entry.materialType)) {
    fail(problems, `${where} (${id}): unknown materialType ${JSON.stringify(entry.materialType)}`);
  }
  if (entry.spdx !== null && (typeof entry.spdx !== 'string' || entry.spdx === '')) {
    fail(problems, `${where} (${id}): spdx must be a non-empty string or null`);
  }
  if (!Number.isInteger(entry.bytes) || entry.bytes <= 0) {
    fail(problems, `${where} (${id}): bytes must be a positive integer`);
  }
  if (!SHA256_RE.test(entry.sha256 ?? '')) {
    fail(problems, `${where} (${id}): sha256 must be 64 lowercase hex characters`);
  }
  if (!DATE_RE.test(entry.retrieved ?? '')) {
    fail(problems, `${where} (${id}): retrieved must be an ISO date`);
  }
  if (!tracked.has(`${STORE_REL}/${id}`)) {
    fail(problems, `${where} (${id}): not tracked by git; it would not survive a fresh clone`);
  }

  const full = join(root, STORE_REL, ...id.split('/'));
  let stat;
  try {
    stat = lstatSync(full);
  } catch {
    fail(problems, `${where} (${id}): no such file in the store`);
    return null;
  }
  // `lstat`, so a symlink reports as a symlink and never reaches a read. A licence "file" that is
  // really a link out of the store would otherwise land verbatim in a document we publish.
  if (!stat.isFile()) {
    fail(problems, `${where} (${id}): is not a regular file (a symlink or directory is not licence text)`);
    return null;
  }
  let real;
  try {
    real = realpathSync(full);
  } catch {
    fail(problems, `${where} (${id}): could not be resolved`);
    return null;
  }
  const rel = relative(storeReal, real);
  if (rel === '' || rel.startsWith(`..${sep}`) || rel === '..' || isAbsolute(rel)) {
    fail(problems, `${where} (${id}): resolves outside the store`);
    return null;
  }

  const bytes = readFileSync(full);
  if (bytes.length !== entry.bytes) {
    fail(problems, `${where} (${id}): is ${bytes.length} bytes, manifest says ${entry.bytes}`);
  }
  const actual = sha256hex(bytes);
  if (actual !== entry.sha256) {
    fail(problems, `${where} (${id}): sha256 is ${actual}, manifest says ${entry.sha256}`);
  }
  return { ...entry, bytes: entry.bytes, raw: bytes };
}

function checkComponent(entry, index, sourcesById, problems, seenPurls) {
  const where = `components[${index}]`;
  const { purl, name, version, ecosystem } = entry ?? {};
  if (typeof purl !== 'string' || purl === '') {
    fail(problems, `${where}: purl is required`);
    return null;
  }
  if (seenPurls.has(purl)) {
    fail(problems, `${where}: duplicate mapping for ${purl}`);
    return null;
  }
  seenPurls.add(purl);

  // The purl is the join key every downstream artifact uses, so it is recomputed from the fields
  // rather than trusted. A mapping whose purl says one version and whose `version` says another
  // would attach the wrong licence to the right-looking row.
  const expectedPurl =
    ecosystem === 'cargo'
      ? `pkg:cargo/${name}@${version}`
      : ecosystem === 'npm'
        ? `pkg:npm/${String(name).replace('@', '%40')}@${version}`
        : null;
  if (expectedPurl === null) {
    fail(problems, `${where} (${purl}): unknown ecosystem ${JSON.stringify(ecosystem)}`);
  } else if (expectedPurl !== purl) {
    fail(problems, `${where} (${purl}): purl does not match ${ecosystem} ${name}@${version}`);
  }

  const p = entry.provenance;
  if (!p || typeof p !== 'object') {
    fail(problems, `${where} (${purl}): provenance is required`);
    return null;
  }
  if (!PROVENANCE_KINDS.has(p.kind)) {
    fail(problems, `${where} (${purl}): unknown provenance kind ${JSON.stringify(p.kind)}`);
  }
  if (!COMMIT_RE.test(p.commit ?? '')) {
    fail(problems, `${where} (${purl}): provenance.commit must be a 40-hex commit`);
  }
  if (p.tag !== null && (typeof p.tag !== 'string' || p.tag === '')) {
    fail(problems, `${where} (${purl}): provenance.tag must be a non-empty string or null`);
  }
  if (typeof p.tag === 'string' && MUTABLE_REFS.has(p.tag)) {
    fail(problems, `${where} (${purl}): provenance.tag ${p.tag} names a moving ref, not a release`);
  }
  if (!Array.isArray(p.evidence) || p.evidence.length === 0 || p.evidence.some((e) => typeof e !== 'string' || e.trim() === '')) {
    fail(problems, `${where} (${purl}): provenance.evidence must be a non-empty list of statements`);
  }
  if (typeof entry.declaredSpdx !== 'string' || entry.declaredSpdx === '') {
    fail(problems, `${where} (${purl}): declaredSpdx is required`);
  }

  if (!Array.isArray(entry.sources) || entry.sources.length === 0) {
    fail(problems, `${where} (${purl}): sources must name at least one vendored file`);
    return null;
  }
  if (new Set(entry.sources).size !== entry.sources.length) {
    fail(problems, `${where} (${purl}): repeats a source id`);
  }
  const resolved = [];
  for (const id of entry.sources) {
    const source = sourcesById.get(id);
    if (!source) {
      fail(problems, `${where} (${purl}): names unknown source ${id}`);
      continue;
    }
    // The mapping and the file must agree about which revision this is. Without this a component
    // could point at a correctly-hashed file vendored from a different release entirely.
    if (source.commit !== p.commit) {
      fail(
        problems,
        `${where} (${purl}): provenance.commit ${p.commit} but source ${id} is from ${source.commit}`,
      );
    }
    if (p.kind === 'release-tree' && source.repository !== p.repository) {
      fail(problems, `${where} (${purl}): release-tree provenance names ${p.repository} but source ${id} is from ${source.repository}`);
    }
    resolved.push(source);
  }

  if (p.kind === 'project-repository') {
    // The escape hatch, and the only one. It must carry its own argument, in writing, plus the
    // component's real repository and commit — so a reviewer can check the claim rather than the
    // category. Silence here is the failure mode this whole module exists to prevent.
    if (typeof p.justification !== 'string' || p.justification.trim().length < 80) {
      fail(problems, `${where} (${purl}): project-repository provenance needs a written justification`);
    }
    if (typeof p.componentRepository !== 'string' || !p.componentRepository.startsWith('https://github.com/')) {
      fail(problems, `${where} (${purl}): project-repository provenance must name componentRepository`);
    }
    if (!COMMIT_RE.test(p.componentCommit ?? '')) {
      fail(problems, `${where} (${purl}): project-repository provenance must pin componentCommit`);
    }
    if (p.componentRepository === p.repository) {
      fail(problems, `${where} (${purl}): componentRepository equals the source repository; use release-tree`);
    }
  } else {
    for (const extra of ['justification', 'componentRepository', 'componentCommit', 'componentTag']) {
      if (p[extra] !== undefined) {
        fail(problems, `${where} (${purl}): ${extra} is only meaningful for project-repository provenance`);
      }
    }
  }

  return { ...entry, resolvedSources: resolved };
}

/**
 * A component that was investigated and could not be closed, with what was checked.
 *
 * This exists so "unresolved" can mean *searched for and not found* rather than *not looked at*. The
 * distinction is the whole value of the row: without it a reader cannot tell a real upstream gap
 * from a task nobody got to. Validated the same way as a mapping — a stale entry claiming a
 * component is blocked when it is not is a false statement in a compliance artifact, so the
 * generator rejects it.
 */
function checkBlocked(entry, index, problems, seenPurls) {
  const where = `blocked[${index}]`;
  const { purl, name, version, ecosystem } = entry ?? {};
  if (typeof purl !== 'string' || purl === '') {
    fail(problems, `${where}: purl is required`);
    return null;
  }
  if (seenPurls.has(purl)) {
    fail(problems, `${where}: ${purl} is both mapped and recorded as blocked`);
    return null;
  }
  seenPurls.add(purl);
  const expectedPurl =
    ecosystem === 'cargo'
      ? `pkg:cargo/${name}@${version}`
      : ecosystem === 'npm'
        ? `pkg:npm/${String(name).replace('@', '%40')}@${version}`
        : null;
  if (expectedPurl !== purl) {
    fail(problems, `${where} (${purl}): purl does not match ${ecosystem} ${name}@${version}`);
  }
  if (typeof entry.declaredSpdx !== 'string' || entry.declaredSpdx === '') {
    fail(problems, `${where} (${purl}): declaredSpdx is required`);
  }
  if (!COMMIT_RE.test(entry.commit ?? '')) {
    fail(problems, `${where} (${purl}): commit must be the 40-hex revision that was searched`);
  }
  if (entry.tag !== null && (typeof entry.tag !== 'string' || entry.tag === '')) {
    fail(problems, `${where} (${purl}): tag must be a non-empty string or null`);
  }
  if (typeof entry.tag === 'string' && MUTABLE_REFS.has(entry.tag)) {
    fail(problems, `${where} (${purl}): tag ${entry.tag} names a moving ref, not a release`);
  }
  if (!DATE_RE.test(entry.investigated ?? '')) {
    fail(problems, `${where} (${purl}): investigated must be an ISO date`);
  }
  if (!Array.isArray(entry.evidence) || entry.evidence.length === 0 || entry.evidence.some((e) => typeof e !== 'string' || e.trim() === '')) {
    fail(problems, `${where} (${purl}): evidence must be a non-empty list of statements`);
  }
  return entry;
}

/**
 * Read and validate the store. Throws with every problem listed, rather than the first.
 *
 * `tracked` is injectable so the adversarial tests can build a store in a temp directory without a
 * git repository, and so the untracked-file rejection can itself be tested. The generator always
 * passes the real answer from `trackedStoreFiles`.
 */
export function loadUpstreamStore(root, { tracked } = {}) {
  const manifestPath = join(root, MANIFEST_REL);
  let doc;
  try {
    doc = JSON.parse(readFileSync(manifestPath, 'utf8'));
  } catch (e) {
    throw new Error(`${MANIFEST_REL} is missing or is not valid JSON: ${e.message}`, { cause: e });
  }
  if (doc?.schemaVersion !== SUPPORTED_SCHEMA_VERSION) {
    throw new Error(
      `${MANIFEST_REL} declares schemaVersion ${JSON.stringify(doc?.schemaVersion)}; this reader ` +
        `understands ${SUPPORTED_SCHEMA_VERSION}. Refusing to interpret a store it may not understand.`,
    );
  }
  if (!Array.isArray(doc.sources) || !Array.isArray(doc.components) || !Array.isArray(doc.blocked)) {
    throw new Error(`${MANIFEST_REL} must carry sources, components and blocked arrays`);
  }

  const storeDir = join(root, STORE_REL);
  let storeReal;
  try {
    storeReal = realpathSync(storeDir);
  } catch {
    throw new Error(`${STORE_REL} does not exist; the manifest describes a store that is not there`);
  }
  const trackedFiles = tracked ?? trackedStoreFiles(root);

  const problems = [];
  const sourcesById = new Map();
  const seenIds = new Set();
  doc.sources.forEach((entry, i) => {
    const checked = checkSource(entry, i, root, storeReal, trackedFiles, problems, seenIds);
    if (checked) sourcesById.set(checked.id, checked);
  });

  const byPurl = new Map();
  const seenPurls = new Set();
  doc.components.forEach((entry, i) => {
    const checked = checkComponent(entry, i, sourcesById, problems, seenPurls);
    if (checked) byPurl.set(checked.purl, checked);
  });

  // Shares `seenPurls` with the mappings above, so a component cannot be claimed as both resolved
  // and blocked — a contradiction the generated artifacts would report as whichever it read last.
  const blockedByPurl = new Map();
  doc.blocked.forEach((entry, i) => {
    const checked = checkBlocked(entry, i, problems, seenPurls);
    if (checked) blockedByPurl.set(checked.purl, checked);
  });

  // A source nobody maps is not harmless. It is either a component that was dropped from the graph
  // and left its text behind, or a file added to the store that no review ever tied to a component;
  // both are states where the store stops describing what ships.
  const used = new Set([...byPurl.values()].flatMap((c) => c.sources));
  for (const id of sourcesById.keys()) {
    if (!used.has(id)) fail(problems, `sources: ${id} is vendored but no component maps it`);
  }

  // Ordering is part of the contract: the manifest is reviewed as a diff, and an unsorted file makes
  // an insertion look like a rewrite.
  const ids = [...sourcesById.keys()];
  if (ids.some((id, i) => i > 0 && byteCompare(ids[i - 1], id) > 0)) {
    fail(problems, 'sources: not in byte order; the manifest would not diff cleanly');
  }
  for (const [label, keys] of [
    ['components', [...byPurl.keys()]],
    ['blocked', [...blockedByPurl.keys()]],
  ]) {
    if (keys.some((purl, i) => i > 0 && byteCompare(keys[i - 1], purl) > 0)) {
      fail(problems, `${label}: not in byte order; the manifest would not diff cleanly`);
    }
  }

  if (problems.length > 0) {
    throw new Error(
      `${MANIFEST_REL} cannot be trusted (${problems.length} problem(s)):\n` +
        problems.map((p) => `  x ${p}`).join('\n'),
    );
  }
  return { sources: sourcesById, byPurl, blockedByPurl };
}

/** Where a vendored file lives, as a repository-relative POSIX path for a committed artifact. */
export function storeProvenancePath(id) {
  return `${STORE_REL}/${id}`;
}

/** Absolute path of a store entry, for callers that need to read it directly. */
export function storeFilePath(root, id) {
  return resolve(root, STORE_REL, ...id.split('/'));
}
