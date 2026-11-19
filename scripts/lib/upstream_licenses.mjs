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
// # Elections: the one place the distributor chooses
//
// `elections` records a component offered under an SPDX disjunction (`MIT OR Apache-2.0`) whose
// published artifact *and* upstream carry no licence file and no notice at all, for which the
// distribution elects Apache-2.0. The reason is narrow: MIT's text is a template whose notice
// condition points at "the above copyright notice", and such a component has no holder's notice to
// reproduce, so an MIT text could only ship with no holder line or an invented one. Apache-2.0's text
// is complete as published, and its conditions (§4) are (a) a copy of the License, (b) change notices
// on modified files, which do not apply to a crate compiled unmodified, and (c)/(d) the notices and
// NOTICE file the work itself carries, of which there are none. So electing Apache-2.0 meets those
// conditions without writing a word.
//
// That argument holds only under narrow conditions, and each one is checked here or in
// `resolveElections`, fail-closed: a pure disjunction with Apache-2.0 as a whole operand; Apache-2.0
// as the only electable licence; one resolution per component; the installed crate directory,
// scanned recursively, holding no licence-like file or directory at any depth and no copyright notice
// in any file (installed text wins, exactly as for a vendored mapping); the elected bytes being the
// Apache-2.0 text the inventory already ships most widely, with the appendix placeholder line present
// and unfilled and no copyright line carrying a year, pinned by hash; and every election used.
//
// What no check can settle: an election presupposes that the crate's bare Cargo `license` field is
// the author's grant. Whether it is, with no licence text or notice anywhere, is a legal question.
// The project treats the published crate's declared licence as the grant, as the Rust ecosystem
// does. None of this is legal advice.
//
// # What is deliberately *not* here
//
// No network. This module reads the committed store and nothing else, so CI and the release gates
// are offline by construction. `scripts/verify_upstream_licenses.mjs` is the opt-in half that
// re-fetches from the pinned URLs and compares; it never writes back.

import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { lstatSync, readFileSync, readdirSync, realpathSync, statSync } from 'node:fs';
import { isAbsolute, join, relative, resolve, sep } from 'node:path';
import { SPDX_IDS } from './licensing.mjs';

export const STORE_REL = 'licensing/upstream';
export const MANIFEST_REL = 'licensing/upstream/sources.json';

/**
 * The manifest shape this module understands. A newer store must not be read by an older reader.
 *
 * 2 added the required `elections` array. A reader that knew only 1 would ignore it, and report the
 * elected component as unresolved — safe, but a store that means something its reader cannot see.
 */
export const SUPPORTED_SCHEMA_VERSION = 2;

/**
 * The licences an election may choose. Apache-2.0 alone, because it is the one whose conditions can
 * be met for a work that ships no notice: its text is the same for every licensee, and §4(c)/(d)
 * ask only for notices the work contains. Adding a licence here needs that argument made again.
 */
export const ELECTABLE_LICENCES = new Set(['Apache-2.0']);

/**
 * The appendix line of the canonical Apache-2.0 text, as published with its placeholders. A copy
 * with this line filled in is somebody's applied notice, and electing it would attribute that
 * somebody's copyright to a different work.
 */
export const APACHE_UNFILLED_COPYRIGHT = 'Copyright [yyyy] [name of copyright owner]';

/**
 * A file or directory name that carries licence or attribution material, matched as a prefix and
 * case-insensitively, at any depth of an elected crate. Deliberately wider than the bundle's
 * `isLicenseFileName`: `licenses/`, `LICENSE.rs` and `NOTICE-third-party.md` all stop an election,
 * because an election is only sound for a crate that speaks about its licence nowhere.
 */
const LICENCE_LIKE_NAME_RE = /^(licen[cs]e|copying|notice|copyright|unlicense)/i;

/** A copyright notice in any file: the word, `(c) <year>`, or the © sign. Case-insensitive. */
const COPYRIGHT_NOTICE_RE = /copyright|\(c\)\s*\d{4}|\u00a9/i;

/** A line in a licence text that applies a notice: it mentions copyright, (c) or © and a year. */
const DATED_NOTICE_RE = /(copyright|\(c\)|\u00a9).*\b(1[89]|20)\d{2}\b/i;

export function isLicenceLikeName(name) {
  return typeof name === 'string' && LICENCE_LIKE_NAME_RE.test(name);
}

/** 1-based line numbers of every line in `text` that carries a copyright notice. */
export function copyrightNoticeLines(text) {
  return String(text)
    .split('\n')
    .flatMap((line, i) => (COPYRIGHT_NOTICE_RE.test(line) ? [i + 1] : []));
}

/**
 * Everything in an installed crate directory that would make an election unsound, found by walking
 * it to the bottom. Returns `{ exists, licenceFiles, notices, unscanned }`, every path relative to
 * `dir` with `/` separators, sorted.
 *
 * A directory that is not there reports `exists: false` rather than an empty scan: "found nothing"
 * and "looked nowhere" must not read the same. Entries are `lstat`ed and links are not followed, so a
 * symlink is reported under `unscanned` instead of being taken on trust. Files are read as Latin-1,
 * which never fails to decode, so a non-UTF-8 file is still searched.
 */
export function scanElectedArtifact(dir) {
  const result = { exists: false, licenceFiles: [], notices: [], unscanned: [] };
  if (typeof dir !== 'string' || dir === '') return result;
  try {
    if (!statSync(dir).isDirectory()) return result;
  } catch {
    return result;
  }
  result.exists = true;
  const walk = (abs, rel) => {
    for (const entry of readdirSync(abs).sort(byteCompare)) {
      const childAbs = join(abs, entry);
      const childRel = rel === '' ? entry : `${rel}/${entry}`;
      const st = lstatSync(childAbs);
      if (isLicenceLikeName(entry)) result.licenceFiles.push(st.isDirectory() ? `${childRel}/` : childRel);
      if (st.isSymbolicLink()) {
        result.unscanned.push(childRel);
      } else if (st.isDirectory()) {
        walk(childAbs, childRel);
      } else if (st.isFile()) {
        for (const line of copyrightNoticeLines(readFileSync(childAbs).toString('latin1'))) {
          result.notices.push(`${childRel}:${line}`);
        }
      } else {
        result.unscanned.push(childRel);
      }
    }
  };
  walk(dir, '');
  for (const list of [result.licenceFiles, result.notices, result.unscanned]) list.sort(byteCompare);
  return result;
}

const ELECTION_KEYS = new Set([
  'purl',
  'name',
  'version',
  'ecosystem',
  'declaredSpdx',
  'chosen',
  'textSha256',
  'attribution',
  'commit',
  'tag',
  'investigated',
  'evidence',
]);
const ATTRIBUTION_KEYS = new Set(['name', 'version', 'authors', 'repository']);

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
 * The operands of an SPDX expression that is a pure disjunction, or `null` for anything else.
 *
 * Pure means `A OR B [OR C …]` and nothing more: every operand a whole identifier from the SPDX list,
 * the operator spelled `OR` exactly, single spaces, no repeats. `AND`, `WITH`, a `+` suffix and any
 * parenthesis all return `null`, including the harmless-looking `(MIT OR Apache-2.0)`, because the
 * point of this check is that "Apache-2.0 is on offer by itself" can be read off the string without
 * an expression parser — and a parser is where a precedence mistake would hide.
 */
export function disjunctionOperands(expr) {
  if (typeof expr !== 'string' || expr === '') return null;
  const tokens = expr.split(' ');
  if (tokens.length < 3 || tokens.length % 2 === 0) return null;
  const operands = [];
  for (let i = 0; i < tokens.length; i += 1) {
    const token = tokens[i];
    if (i % 2 === 1) {
      if (token !== 'OR') return null;
      continue;
    }
    if (!/^[A-Za-z0-9][A-Za-z0-9.-]*$/.test(token) || !SPDX_IDS.has(token)) return null;
    operands.push(token);
  }
  return new Set(operands).size === operands.length ? operands : null;
}

/**
 * Whether a decoded licence text is the Apache License 2.0: its first two non-blank lines are the
 * licence's own title lines, and it reaches the end of the terms. Content, not filename, because
 * crates name the same file `LICENSE`, `LICENSE-APACHE`, `LICENSE-APACHE.md` and more.
 */
export function isApache2Text(text) {
  if (typeof text !== 'string') return false;
  const lines = text.split('\n').map((l) => l.trim()).filter((l) => l !== '');
  return (
    lines[0] === 'Apache License' &&
    lines[1] === 'Version 2.0, January 2004' &&
    lines.includes('END OF TERMS AND CONDITIONS')
  );
}

function purlFor(ecosystem, name, version) {
  if (ecosystem === 'cargo') return `pkg:cargo/${name}@${version}`;
  if (ecosystem === 'npm') return `pkg:npm/${String(name).replace('@', '%40')}@${version}`;
  return null;
}

/**
 * A licence the distribution elects from a disjunction the component offers. The static half of the
 * rules — everything that can be checked from the manifest alone. `resolveElections` checks the
 * rest against the inventory: that the component is in the graph, ships, declares exactly this
 * expression, still carries no licence file, and that the pinned text is the one the inventory
 * already ships most widely.
 */
function checkElection(entry, index, problems, seenPurls) {
  const where = `elections[${index}]`;
  const { purl, name, version, ecosystem } = entry ?? {};
  if (typeof purl !== 'string' || purl === '') {
    fail(problems, `${where}: purl is required`);
    return null;
  }
  // Shared with mappings and blocked records: a component has exactly one resolution. An election
  // beside a mapping would let the vendored text and the elected one disagree about the same row.
  if (seenPurls.has(purl)) {
    fail(problems, `${where}: ${purl} is already mapped or recorded as blocked; a component has one resolution`);
    return null;
  }
  seenPurls.add(purl);

  // Unknown keys are refused rather than ignored. A misspelt `choosen` would otherwise be silently
  // dropped, and the entry validated against whatever the reader assumed instead.
  for (const key of Object.keys(entry)) {
    if (!ELECTION_KEYS.has(key)) fail(problems, `${where} (${purl}): unknown key ${JSON.stringify(key)}`);
  }
  if (ecosystem !== 'cargo') {
    // The attribution is checked against `cargo metadata`'s `authors`; npm has no equivalent wired
    // in, so an npm election could not be held to the same standard. Refused until it can be.
    fail(problems, `${where} (${purl}): only cargo components can be elected; got ${JSON.stringify(ecosystem)}`);
  }
  if (purlFor(ecosystem, name, version) !== purl) {
    fail(problems, `${where} (${purl}): purl does not match ${ecosystem} ${name}@${version}`);
  }

  const operands = disjunctionOperands(entry.declaredSpdx);
  if (operands === null) {
    fail(
      problems,
      `${where} (${purl}): declaredSpdx ${JSON.stringify(entry.declaredSpdx)} is not a pure disjunction ` +
        '(`A OR B [OR C]` of whole SPDX identifiers, with no AND, WITH, + or parentheses)',
    );
  } else if (!operands.includes('Apache-2.0')) {
    fail(problems, `${where} (${purl}): declaredSpdx does not offer Apache-2.0 as a whole operand`);
  }
  if (!ELECTABLE_LICENCES.has(entry.chosen)) {
    fail(
      problems,
      `${where} (${purl}): chosen ${JSON.stringify(entry.chosen)} cannot be elected; only ` +
        `${[...ELECTABLE_LICENCES].join(', ')} is supported`,
    );
  }
  if (operands !== null && !operands.includes(entry.chosen)) {
    fail(problems, `${where} (${purl}): chosen ${JSON.stringify(entry.chosen)} is not one of the operands of ${entry.declaredSpdx}`);
  }
  if (!SHA256_RE.test(entry.textSha256 ?? '')) {
    fail(problems, `${where} (${purl}): textSha256 must be 64 lowercase hex characters`);
  }

  const a = entry.attribution;
  if (!a || typeof a !== 'object' || Array.isArray(a)) {
    fail(problems, `${where} (${purl}): attribution is required`);
  } else {
    for (const key of Object.keys(a)) {
      if (!ATTRIBUTION_KEYS.has(key)) fail(problems, `${where} (${purl}): attribution has unknown key ${JSON.stringify(key)}`);
    }
    if (a.name !== name || a.version !== version) {
      fail(problems, `${where} (${purl}): attribution must name the component it attributes`);
    }
    if (!Array.isArray(a.authors) || a.authors.length === 0 || a.authors.some((s) => typeof s !== 'string' || s.trim() === '')) {
      fail(problems, `${where} (${purl}): attribution.authors must be the non-empty author list the manifest declares`);
    }
    if (typeof a.repository !== 'string' || !a.repository.startsWith('https://')) {
      fail(problems, `${where} (${purl}): attribution.repository must be an https URL`);
    }
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
 * The inventory-dependent rules for one election against the component as installed. Returns the
 * problems found, empty when the election applies.
 *
 * `scan` is what `scanElectedArtifact` found in the installed crate directory. A licence-like file
 * or directory at any depth means the component speaks for itself, and "installed text wins"
 * applies to an election exactly as it does to a vendored mapping; a copyright notice anywhere is a
 * notice Apache-2.0 §4(c) would require be retained, which an election does not carry; a missing
 * directory, or a link the scan did not follow, means the absence was never shown. A published
 * version never changes, so in practice this fires when an election is carried over to a newer
 * release that ships its texts (the old one is then unused, which `resolveElections` refuses too).
 */
export function electionProblems(election, component, scan) {
  const problems = [];
  const at = `election for ${election.purl}`;
  if (!component.distributed) {
    problems.push(`${at}: the component is installed but not distributed; only what ships carries an obligation`);
  }
  if (component.declaredLicense !== election.declaredSpdx) {
    problems.push(
      `${at}: the component declares ${JSON.stringify(component.declaredLicense)}, the election ` +
        `records ${JSON.stringify(election.declaredSpdx)}`,
    );
  }
  if (component.declaredLicenseFile) {
    problems.push(`${at}: the component declares a licence file (${component.declaredLicenseFile}); its terms are not a plain SPDX choice`);
  }
  if (!scan?.exists) {
    problems.push(
      `${at}: the installed crate directory is missing, so nothing shows it carries no licence file; ` +
        'an election is never granted on an empty look',
    );
  } else {
    if (scan.licenceFiles.length > 0) {
      problems.push(
        `${at}: the installed artifact contains ${scan.licenceFiles.join(', ')}. Installed text wins; ` +
          'delete the election.',
      );
    }
    if (scan.notices.length > 0) {
      problems.push(
        `${at}: the installed artifact carries a copyright notice (${scan.notices.join(', ')}), which ` +
          'Apache-2.0 §4(c) would require be retained. An election is only for a crate with none.',
      );
    }
    if (scan.unscanned.length > 0) {
      problems.push(`${at}: the installed artifact holds entries the scan does not follow (${scan.unscanned.join(', ')})`);
    }
  }
  const authors = Array.isArray(component.authors) ? component.authors : [];
  const attributed = election.attribution?.authors ?? [];
  if (authors.length !== attributed.length || authors.some((s, i) => s !== attributed[i])) {
    problems.push(
      `${at}: attribution.authors ${JSON.stringify(attributed)} is not what the manifest declares, ` +
        `${JSON.stringify(authors)}`,
    );
  }
  if ((component.repository ?? null) !== (election.attribution?.repository ?? null)) {
    problems.push(
      `${at}: attribution.repository ${JSON.stringify(election.attribution?.repository)} is not the ` +
        `manifest's ${JSON.stringify(component.repository)}`,
    );
  }
  return problems;
}

/**
 * The text an election publishes, chosen from texts the inventory already ships. Returns
 * `{ problems, text }`; `text` is set only when there are no problems.
 *
 * `pool` holds each distinct decoded text read out of an installed artifact, with the set of
 * components that carry it. Downloading the Apache-2.0 text, or keeping a private copy, would make
 * the elected bytes something nobody else ships; taking the most widely shipped Apache-2.0 text in
 * this very inventory means the elected text is byte-identical to one already published beside it,
 * pinned by hash so a dependency update that changes which copy is most common fails loudly instead
 * of silently changing what is attributed.
 */
export function electedText(election, pool) {
  const at = `election for ${election.purl}`;
  const apache = pool
    .filter((t) => isApache2Text(t.text))
    .map((t) => ({ ...t, carriers: t.purls.size }))
    .sort((a, b) => b.carriers - a.carriers || byteCompare(a.textSha256, b.textSha256));
  if (apache.length === 0) {
    return { problems: [`${at}: no installed artifact in the inventory ships an Apache-2.0 text to elect`] };
  }
  const [top, second] = apache;
  const problems = [];
  if (second && second.carriers === top.carriers) {
    problems.push(
      `${at}: two Apache-2.0 texts are equally common (${top.carriers} components each: ` +
        `${top.textSha256}, ${second.textSha256}); which one is "the" text is ambiguous`,
    );
  }
  if (top.textSha256 !== election.textSha256) {
    problems.push(
      `${at}: textSha256 pins ${election.textSha256}, but the Apache-2.0 text the inventory ships most ` +
        `widely is ${top.textSha256} (${top.carriers} components). Review that text and re-pin, or ` +
        'leave the component unresolved.',
    );
  }
  if (sha256hex(Buffer.from(top.text, 'utf8')) !== top.textSha256) {
    problems.push(`${at}: the pooled text does not hash to ${top.textSha256}`);
  }
  const lines = top.text.split('\n');
  if (!lines.some((l) => l.trim() === APACHE_UNFILLED_COPYRIGHT)) {
    problems.push(
      lines.some((l) => l.trim().startsWith('APPENDIX'))
        ? `${at}: the most widely shipped Apache-2.0 text has its appendix filled in or altered (the ` +
            `unfilled line "${APACHE_UNFILLED_COPYRIGHT}" is missing); electing it could attribute ` +
            "another project's copyright notice to this component"
        : `${at}: the most widely shipped Apache-2.0 text has no appendix, so the unfilled ` +
            `"${APACHE_UNFILLED_COPYRIGHT}" line this rule requires is absent; review and re-pin`,
    );
  }
  const dated = lines.flatMap((l, i) => (DATED_NOTICE_RE.test(l) ? [i + 1] : []));
  if (dated.length > 0) {
    problems.push(
      `${at}: the most widely shipped Apache-2.0 text carries a dated copyright line (line ` +
        `${dated.join(', ')}); electing it would attribute that notice to this component`,
    );
  }
  return problems.length > 0 ? { problems } : { problems, text: top.text };
}

/**
 * Apply every election to the inventory. Returns a Map from purl to `{ election, text, textSha256 }`
 * and throws with every problem listed when any election does not hold.
 *
 * `components` is the whole inventory, distributed or not; `pool` is described at `electedText`.
 * Each elected component's installed directory is scanned **here**, from its own `sourceDir`, rather
 * than handed in by the caller: there is then no hand-off a caller could drop or blank, and a
 * component whose directory is missing fails instead of passing on an empty list. An election that
 * names a component the graph does not have is an error, for the same reason an unused mapping is:
 * it is a claim of coverage for something that is not here.
 */
export function resolveElections(electionsByPurl, components, pool) {
  const problems = [];
  const resolved = new Map();
  const byPurl = new Map(components.map((c) => [c.purl, c]));
  for (const [purl, election] of electionsByPurl) {
    const component = byPurl.get(purl);
    if (!component) {
      problems.push(`election for ${purl}: no such component in the dependency graph; the election is unused`);
      continue;
    }
    const local = electionProblems(election, component, scanElectedArtifact(component.sourceDir));
    if (local.length > 0) {
      problems.push(...local);
      continue;
    }
    const picked = electedText(election, pool);
    if (picked.problems.length > 0) {
      problems.push(...picked.problems);
      continue;
    }
    resolved.set(purl, { election, text: picked.text, textSha256: election.textSha256 });
  }
  if (problems.length > 0) {
    throw new Error(
      `${MANIFEST_REL} elections cannot be applied (${problems.length} problem(s)):\n` +
        problems.map((p) => `  x ${p}`).join('\n'),
    );
  }
  return resolved;
}

/**
 * Negative controls for the election rules, run by the generator before it trusts any election —
 * the same discipline as `scripts/check_docs_metadata.mjs`. A validator that never rejects anything
 * is indistinguishable from one that returns true, and an election is the one place this system
 * chooses a licence, so the generator exits 2 rather than publish one through rules it has not just
 * watched fire. `tests/frontend/upstreamLicenses.test.ts` breaks the same rules one at a time in CI;
 * this is the copy that travels with every run of the generator itself.
 */
export function electionSelfTest() {
  const failures = [];
  const apache = [
    'Apache License',
    'Version 2.0, January 2004',
    'TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION',
    'END OF TERMS AND CONDITIONS',
    'APPENDIX: How to apply the Apache License to your work.',
    `   ${APACHE_UNFILLED_COPYRIGHT}`,
    '',
  ].join('\n');
  const filled = apache.replace(APACHE_UNFILLED_COPYRIGHT, 'Copyright 2020 Somebody');
  const bare = apache.slice(0, apache.indexOf('APPENDIX'));
  const purl = 'pkg:cargo/control@0.1.0';
  const good = {
    purl,
    name: 'control',
    version: '0.1.0',
    ecosystem: 'cargo',
    declaredSpdx: 'MIT OR Apache-2.0',
    chosen: 'Apache-2.0',
    textSha256: sha256hex(Buffer.from(apache, 'utf8')),
    attribution: { name: 'control', version: '0.1.0', authors: ['A Person'], repository: 'https://example.com/control' },
    commit: '0'.repeat(40),
    tag: null,
    investigated: '2026-01-01',
    evidence: ['control'],
  };
  const component = {
    purl,
    declaredLicense: 'MIT OR Apache-2.0',
    declaredLicenseFile: null,
    repository: 'https://example.com/control',
    authors: ['A Person'],
    distributed: true,
  };
  const pool = (...texts) =>
    texts.map(([text, n], i) => ({
      textSha256: sha256hex(Buffer.from(text, 'utf8')),
      text,
      purls: new Set(Array.from({ length: n }, (_, k) => `pkg:cargo/t${i}-${k}@1.0.0`)),
    }));
  const entryProblems = (entry, seen = []) => {
    const problems = [];
    checkElection(entry, 0, problems, new Set(seen));
    return problems;
  };
  let rejects = 0;
  let accepts = 0;
  /** A must-reject control: `refused()` returns true when the rule fired. */
  const mustReject = (label, refused) => {
    rejects += 1;
    if (!refused()) failures.push(`negative control "${label}" was accepted`);
  };
  /** A must-accept control: `accepted()` returns true when the good input passed. */
  const mustAccept = (label, accepted) => {
    accepts += 1;
    if (!accepted()) failures.push(`positive control "${label}" was rejected`);
  };
  const throws = (fn) => {
    try {
      fn();
      return false;
    } catch {
      return true;
    }
  };

  // The manifest entry.
  for (const [label, entry] of [
    ['AND', { ...good, declaredSpdx: 'MIT AND Apache-2.0' }],
    ['WITH', { ...good, declaredSpdx: 'Apache-2.0 WITH LLVM-exception OR MIT' }],
    ['parentheses', { ...good, declaredSpdx: '(MIT OR Apache-2.0)' }],
    ['Apache-2.0 not offered', { ...good, declaredSpdx: 'MIT OR Zlib' }],
    ['MIT chosen', { ...good, chosen: 'MIT' }],
    ['unknown key', { ...good, choosen: 'Apache-2.0' }],
    ['npm', { ...good, ecosystem: 'npm', purl: 'pkg:npm/control@0.1.0' }],
    ['bad hash', { ...good, textSha256: 'apache' }],
    ['no authors', { ...good, attribution: { ...good.attribution, authors: [] } }],
  ]) {
    mustReject(label, () => entryProblems(entry).length > 0);
  }
  mustReject('already mapped or blocked', () => entryProblems(good, [purl]).length > 0);
  mustAccept('well-formed entry', () => entryProblems(good).length === 0);

  // The names and notices the artifact scan looks for.
  for (const name of ['LICENSE-MIT', 'licence.txt', 'COPYING', 'NOTICE.md', 'COPYRIGHT', 'UNLICENSE', 'licenses']) {
    mustReject(`licence-like name ${name}`, () => isLicenceLikeName(name));
  }
  mustAccept('ordinary names', () => !['lib.rs', 'README.md', 'Cargo.toml', '.cargo_vcs_info.json'].some(isLicenceLikeName));
  for (const line of ['// Copyright 2020 Someone', '(c) 2019 Someone', '\u00a9 Someone', 'THE COPYRIGHT HOLDERS']) {
    mustReject(`notice ${JSON.stringify(line)}`, () => copyrightNoticeLines(line).length === 1);
  }
  mustAccept('lines without a notice', () => copyrightNoticeLines('fn main() {}\n(c) You must retain\n').length === 0);

  // The installed component, as scanned.
  const clean = { exists: true, licenceFiles: [], notices: [], unscanned: [] };
  for (const [label, comp, scan] of [
    ['top-level licence file', component, { ...clean, licenceFiles: ['LICENSE'] }],
    ['nested licence file', component, { ...clean, licenceFiles: ['licenses/LICENSE-MIT'] }],
    ['copyright notice in a source file', component, { ...clean, notices: ['src/lib.rs:1'] }],
    ['missing crate directory', component, { ...clean, exists: false }],
    ['link the scan did not follow', component, { ...clean, unscanned: ['src/link'] }],
    ['not distributed', { ...component, distributed: false }, clean],
    ['declared expression differs', { ...component, declaredLicense: 'MIT' }, clean],
  ]) {
    mustReject(label, () => electionProblems(good, comp, scan).length > 0);
  }
  mustAccept('clean installed component', () => electionProblems(good, component, clean).length === 0);

  // The elected text.
  const dated = `${apache}Copyright 2021 Hidden Holder\n`;
  for (const [label, election, texts] of [
    ['pin is not the most common copy', { ...good, textSha256: sha256hex(Buffer.from(bare, 'utf8')) }, pool([apache, 5], [bare, 2])],
    ['appendix filled in', { ...good, textSha256: sha256hex(Buffer.from(filled, 'utf8')) }, pool([filled, 5])],
    ['no appendix', { ...good, textSha256: sha256hex(Buffer.from(bare, 'utf8')) }, pool([bare, 5])],
    ['placeholder kept, dated holder line added', { ...good, textSha256: sha256hex(Buffer.from(dated, 'utf8')) }, pool([dated, 5])],
    ['two copies equally common', good, pool([apache, 3], [bare, 3])],
    ['no Apache-2.0 text', good, pool(['MIT License\n', 9])],
  ]) {
    mustReject(label, () => electedText(election, texts).problems.length > 0);
  }
  mustAccept('most common Apache-2.0 copy', () => electedText(good, pool(['MIT License\n', 50], [apache, 5])).text === apache);

  // The whole set, including the scan `resolveElections` performs itself.
  mustReject('unused election', () => throws(() => resolveElections(new Map([[purl, good]]), [], pool([apache, 5]))));
  mustReject('elected crate directory not on disk', () =>
    throws(() =>
      resolveElections(
        new Map([[purl, good]]),
        [{ ...component, sourceDir: join(STORE_REL, 'no-such-crate-directory-for-a-control') }],
        pool([apache, 5]),
      ),
    ),
  );

  return { failures, controls: rejects + accepts, rejects, accepts };
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
  if (
    !Array.isArray(doc.sources) ||
    !Array.isArray(doc.components) ||
    !Array.isArray(doc.blocked) ||
    !Array.isArray(doc.elections)
  ) {
    throw new Error(`${MANIFEST_REL} must carry sources, components, blocked and elections arrays`);
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

  // Last, and on the same `seenPurls`: an election may only stand where nothing else resolves the
  // component — not a vendored mapping, and not a blocked record claiming the search found nothing.
  const electionsByPurl = new Map();
  doc.elections.forEach((entry, i) => {
    const checked = checkElection(entry, i, problems, seenPurls);
    if (checked) electionsByPurl.set(checked.purl, checked);
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
    ['elections', [...electionsByPurl.keys()]],
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
  return { sources: sourcesById, byPurl, blockedByPurl, electionsByPurl };
}

/** Where a vendored file lives, as a repository-relative POSIX path for a committed artifact. */
export function storeProvenancePath(id) {
  return `${STORE_REL}/${id}`;
}

/** Absolute path of a store entry, for callers that need to read it directly. */
export function storeFilePath(root, id) {
  return resolve(root, STORE_REL, ...id.split('/'));
}
