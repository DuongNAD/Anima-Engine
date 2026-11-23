#!/usr/bin/env node
// Card P1: verify docs/paper/data/incidents.csv against git history and pinned source anchors.
//
// Usage:
//   node scripts/paper/incident_dates.mjs --check [--csv <path>] [--root <path>]
//   node scripts/paper/incident_dates.mjs --self-test

import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { isAbsolute, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const EXPECTED_HEADERS = [
  'id',
  'title',
  'class',
  'introduced_commit',
  'fixed_commit',
  'detected_how',
  'introduced_date',
  'detected_date',
  'fixed_date',
  'days_to_fix',
  'was_suite_green',
  'source',
];

export const ALLOWED_CLASSES = new Set([
  'vacuous-gate',
  'fabricated-evidence',
  'loosened-test',
  'narrowed-command',
  'silent-semantic-change',
  'nondeterminism',
  'contract-overwrite',
  'toolchain-drift',
  'other',
]);

export const ALLOWED_SUITE_GREEN = new Set(['yes', 'no', 'unknown']);

const COMMIT_HASH_RE = /^[0-9a-f]{7,40}$/i;
const SOURCE_ANCHOR_RE = /^([^@:\r\n]+)@([0-9a-f]{7,40}):(\d+)$/i;
const ISO_DAY_RE = /^(\d{4})-(\d{2})-(\d{2})$/;
const MS_PER_DAY = 86_400_000;
const GIT_LOG_CHUNK_SIZE = 200;

/**
 * Parse RFC 4180 CSV text into an array of string rows.
 * Throws an Error on malformed quoting or stray characters after closing quotes.
 */
export function parseCsv(rawText) {
  const text = rawText.charCodeAt(0) === 0xfeff ? rawText.slice(1) : rawText;
  const rows = [];
  let row = [];
  let field = '';
  let inQuotes = false;
  let justClosedQuote = false;

  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i];

    if (inQuotes) {
      if (ch === '"') {
        if (text[i + 1] === '"') {
          field += '"';
          i += 1;
        } else {
          inQuotes = false;
          justClosedQuote = true;
        }
      } else {
        field += ch;
      }
      continue;
    }

    if (justClosedQuote) {
      if (ch === ',') {
        row.push(field);
        field = '';
        justClosedQuote = false;
      } else if (ch === '\r') {
        if (text[i + 1] === '\n') i += 1;
        row.push(field);
        rows.push(row);
        row = [];
        field = '';
        justClosedQuote = false;
      } else if (ch === '\n') {
        row.push(field);
        rows.push(row);
        row = [];
        field = '';
        justClosedQuote = false;
      } else {
        throw new Error(`unexpected character ${JSON.stringify(ch)} after closing quote at row ${rows.length + 1}`);
      }
      continue;
    }

    if (ch === '"') {
      if (field.length > 0) {
        throw new Error(`unexpected quote inside unquoted field at row ${rows.length + 1}`);
      }
      inQuotes = true;
    } else if (ch === ',') {
      row.push(field);
      field = '';
    } else if (ch === '\r') {
      if (text[i + 1] === '\n') i += 1;
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
    } else if (ch === '\n') {
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
    } else {
      field += ch;
    }
  }

  if (inQuotes) {
    throw new Error(`unterminated quoted field starting at row ${rows.length + 1}`);
  }

  if (field.length > 0 || row.length > 0 || justClosedQuote) {
    row.push(field);
    rows.push(row);
  }

  return rows;
}

export function isValidIsoDay(dayStr) {
  const match = ISO_DAY_RE.exec(dayStr);
  if (!match) return false;
  const year = Number.parseInt(match[1], 10);
  const month = Number.parseInt(match[2], 10);
  const day = Number.parseInt(match[3], 10);
  const dt = new Date(Date.UTC(year, month - 1, day));
  return (
    dt.getUTCFullYear() === year &&
    dt.getUTCMonth() + 1 === month &&
    dt.getUTCDate() === day
  );
}

export function extractAnchorCommits(sourceText) {
  if (!sourceText || sourceText.trim() === '') return [];
  const commits = [];
  for (const segment of sourceText.split(';').map((p) => p.trim())) {
    const m = SOURCE_ANCHOR_RE.exec(segment);
    if (m) commits.push(m[2]);
  }
  return commits;
}

/**
 * Batch-verify commit hashes for object existence, type == "commit", ancestry of HEAD
 * (via `git rev-list HEAD`), and committer ISO timestamp `%cI` using minimal git subprocesses.
 */
export function batchVerifyCommits(hashes, repoRoot) {
  const resultMap = new Map();
  const uniqueHashes = [...new Set(hashes.filter(Boolean))];
  if (uniqueHashes.length === 0) return resultMap;

  const validSyntaxHashes = [];
  for (const h of uniqueHashes) {
    if (!COMMIT_HASH_RE.test(h)) {
      resultMap.set(h, {
        ok: false,
        error: `invalid commit hash syntax "${h}" (expected 7-40 hex chars)`,
      });
    } else {
      validSyntaxHashes.push(h);
    }
  }

  if (validSyntaxHashes.length === 0) return resultMap;

  let ancestors;
  try {
    const revOut = execFileSync('git', ['rev-list', 'HEAD'], {
      cwd: repoRoot,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
      maxBuffer: 1 << 24,
    });
    ancestors = new Set(revOut.split(/\r?\n/).filter(Boolean));
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    const firstLine = msg.split(/\r?\n/)[0];
    for (const h of validSyntaxHashes) {
      resultMap.set(h, { ok: false, error: `git rev-list HEAD failed: ${firstLine}` });
    }
    return resultMap;
  }

  let batchOut;
  try {
    batchOut = execFileSync('git', ['cat-file', '--batch-check=%(objectname)\t%(objecttype)'], {
      cwd: repoRoot,
      input: `${validSyntaxHashes.join('\n')}\n`,
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'pipe'],
      maxBuffer: 1 << 24,
    });
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    const firstLine = msg.split(/\r?\n/)[0];
    for (const h of validSyntaxHashes) {
      resultMap.set(h, { ok: false, error: `git cat-file --batch-check failed: ${firstLine}` });
    }
    return resultMap;
  }

  const batchLines = batchOut.replace(/\r?\n$/, '').split(/\r?\n/);
  const fullShaByInput = new Map();
  const uniqueAncestorFullShas = new Set();

  for (let i = 0; i < validSyntaxHashes.length; i += 1) {
    const h = validSyntaxHashes[i];
    const line = batchLines[i] ?? '';
    const tabIdx = line.indexOf('\t');
    if (tabIdx === -1) {
      resultMap.set(h, {
        ok: false,
        error: `git object lookup failed for "${h}": ${line || 'missing'}`,
      });
      continue;
    }

    const fullSha = line.slice(0, tabIdx);
    const objectType = line.slice(tabIdx + 1).trim();
    if (objectType !== 'commit') {
      resultMap.set(h, {
        ok: false,
        error: `git object ${h} has type "${objectType}" instead of "commit"`,
      });
      continue;
    }

    if (!ancestors.has(fullSha)) {
      resultMap.set(h, {
        ok: false,
        error: `commit ${h} (${fullSha}) is not an ancestor of HEAD`,
      });
      continue;
    }

    fullShaByInput.set(h, fullSha);
    uniqueAncestorFullShas.add(fullSha);
  }

  const ancestorList = [...uniqueAncestorFullShas];
  const dateByFullSha = new Map();
  for (let i = 0; i < ancestorList.length; i += GIT_LOG_CHUNK_SIZE) {
    const chunk = ancestorList.slice(i, i + GIT_LOG_CHUNK_SIZE);
    let logOut;
    try {
      logOut = execFileSync('git', ['log', '--no-walk', '--format=%H%x09%cI', ...chunk], {
        cwd: repoRoot,
        encoding: 'utf8',
        stdio: ['ignore', 'pipe', 'pipe'],
        maxBuffer: 1 << 24,
      });
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      const firstLine = msg.split(/\r?\n/)[0];
      for (const [h, fullSha] of fullShaByInput.entries()) {
        if (chunk.includes(fullSha)) {
          resultMap.set(h, { ok: false, error: `git log --no-walk failed: ${firstLine}` });
        }
      }
      continue;
    }

    for (const line of logOut.split(/\r?\n/)) {
      if (!line) continue;
      const tabIdx = line.indexOf('\t');
      if (tabIdx === -1) continue;
      const fullSha = line.slice(0, tabIdx);
      const committerDate = line.slice(tabIdx + 1).trim();
      dateByFullSha.set(fullSha, committerDate);
    }
  }

  for (const [h, fullSha] of fullShaByInput.entries()) {
    if (resultMap.has(h)) continue;
    const committerDate = dateByFullSha.get(fullSha);
    if (!committerDate || Number.isNaN(Date.parse(committerDate))) {
      resultMap.set(h, {
        ok: false,
        error: `git log returned invalid date "${committerDate ?? ''}" for ${h}`,
      });
    } else {
      resultMap.set(h, { ok: true, fullSha, date: committerDate });
    }
  }

  return resultMap;
}

export function verifyCommitAndGetDate(hash, repoRoot) {
  return batchVerifyCommits([hash], repoRoot).get(hash);
}

export function validateSourceAnchor(sourceText, repoRoot, commitMap, blobCache = new Map()) {
  if (!sourceText || sourceText.trim() === '') {
    return { ok: false, error: 'source anchor is empty' };
  }

  const resolvedRoot = resolve(repoRoot);
  const resolvedCommits =
    commitMap ?? batchVerifyCommits(extractAnchorCommits(sourceText), resolvedRoot);

  const segments = sourceText.split(';').map((part) => part.trim());
  for (const segment of segments) {
    if (!segment) {
      return { ok: false, error: `empty anchor segment in "${sourceText}"` };
    }
    const match = SOURCE_ANCHOR_RE.exec(segment);
    if (!match) {
      return {
        ok: false,
        error: `invalid source anchor "${segment}" (expected pinned <path>@<sha>:<line>)`,
      };
    }

    const relPath = match[1];
    const anchorSha = match[2];
    const lineNum = Number.parseInt(match[3], 10);

    if (
      isAbsolute(relPath) ||
      relPath.startsWith('/') ||
      relPath.startsWith('\\') ||
      /^[A-Za-z]:/.test(relPath)
    ) {
      return { ok: false, error: `source path "${relPath}" must be relative to repo root` };
    }
    if (relPath.split(/[\\/]/).includes('..')) {
      return { ok: false, error: `source path "${relPath}" must not contain ".."` };
    }

    const resolvedFile = resolve(resolvedRoot, relPath);
    const relFromRoot = relative(resolvedRoot, resolvedFile);
    if (relFromRoot.startsWith('..') || isAbsolute(relFromRoot)) {
      return { ok: false, error: `source path "${relPath}" escapes repo root` };
    }

    const commitInfo = resolvedCommits.get(anchorSha);
    if (!commitInfo || !commitInfo.ok) {
      return {
        ok: false,
        error: `source anchor commit "${anchorSha}": ${commitInfo?.error ?? 'unverified'}`,
      };
    }

    const gitRelPath = relPath.replaceAll('\\', '/');
    const cacheKey = `${commitInfo.fullSha}:${gitRelPath}`;
    let lines = blobCache.get(cacheKey);
    if (lines === undefined) {
      let content;
      try {
        content = execFileSync('git', ['show', `${commitInfo.fullSha}:${gitRelPath}`], {
          cwd: resolvedRoot,
          encoding: 'utf8',
          stdio: ['ignore', 'pipe', 'pipe'],
          maxBuffer: 1 << 24,
        });
      } catch {
        return {
          ok: false,
          error: `source file "${gitRelPath}" does not exist at commit ${anchorSha}`,
        };
      }
      const normalized = content.replace(/\r?\n$/, '');
      lines = content.length === 0 ? [] : normalized.split(/\r?\n/);
      blobCache.set(cacheKey, lines);
    }

    if (!Number.isInteger(lineNum) || lineNum < 1 || lineNum > lines.length) {
      return {
        ok: false,
        error: `source line ${lineNum} in "${gitRelPath}@${anchorSha}" is out of range (1..${lines.length})`,
      };
    }

    if (lines[lineNum - 1].trim().length === 0) {
      return {
        ok: false,
        error: `source line ${lineNum} in "${gitRelPath}@${anchorSha}" is blank`,
      };
    }
  }

  return { ok: true };
}

export function computeDaysToFix(introducedIso, fixedIso) {
  const introducedMs = Date.parse(introducedIso);
  const fixedMs = Date.parse(fixedIso);
  if (Number.isNaN(introducedMs) || Number.isNaN(fixedMs)) {
    return {
      ok: false,
      error: `unparseable ISO date(s): introduced="${introducedIso}", fixed="${fixedIso}"`,
    };
  }
  if (fixedMs < introducedMs) {
    return {
      ok: false,
      error: `fixed_date (${fixedIso}) precedes introduced_date (${introducedIso})`,
    };
  }
  return { ok: true, days: Math.round((fixedMs - introducedMs) / MS_PER_DAY) };
}

export function checkIncidentsCsv({ csvPath, repoRoot }) {
  const resolvedRoot = resolve(repoRoot);
  const resolvedCsv = resolve(resolvedRoot, csvPath);
  const errors = [];

  if (!existsSync(resolvedCsv)) {
    return {
      ok: false,
      rowCount: 0,
      errors: [{ id: '<file>', field: 'csv', message: `CSV file does not exist: ${resolvedCsv}` }],
    };
  }

  // A shallow clone (CI's default depth-1 checkout) has none of the catalogued commits. Say that
  // once instead of reporting every row's commits as missing.
  let isShallow = 'false';
  try {
    isShallow = execFileSync('git', ['rev-parse', '--is-shallow-repository'], {
      cwd: resolvedRoot,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    }).trim();
  } catch {
    // Not a git repository: the commit checks below report it.
  }
  if (isShallow === 'true') {
    return {
      ok: false,
      rowCount: 0,
      errors: [
        {
          id: '<repo>',
          field: 'git',
          message: `${resolvedRoot} is a shallow clone; run \`git fetch --unshallow\` (CI: actions/checkout with fetch-depth: 0)`,
        },
      ],
    };
  }

  let parsedRows;
  try {
    parsedRows = parseCsv(readFileSync(resolvedCsv, 'utf8'));
  } catch (err) {
    return {
      ok: false,
      rowCount: 0,
      errors: [{ id: '<file>', field: 'csv', message: err instanceof Error ? err.message : String(err) }],
    };
  }

  if (parsedRows.length === 0) {
    return {
      ok: false,
      rowCount: 0,
      errors: [{ id: '<header>', field: 'header', message: 'CSV file is empty' }],
    };
  }

  const header = parsedRows[0];
  if (
    header.length !== EXPECTED_HEADERS.length ||
    header.some((col, idx) => col !== EXPECTED_HEADERS[idx])
  ) {
    errors.push({
      id: '<header>',
      field: 'header',
      message: `expected header ${EXPECTED_HEADERS.join(',')} but got ${header.join(',')}`,
    });
    return { ok: false, rowCount: Math.max(0, parsedRows.length - 1), errors };
  }

  const dataRows = parsedRows.slice(1);
  if (dataRows.length === 0) {
    errors.push({
      id: '<file>',
      field: 'rows',
      message: 'CSV contains header only and zero incident rows',
    });
    return { ok: false, rowCount: 0, errors };
  }

  // Batch all commit lookups (introduced_commit, fixed_commit, source @sha) across all rows.
  const idxIntro = EXPECTED_HEADERS.indexOf('introduced_commit');
  const idxFix = EXPECTED_HEADERS.indexOf('fixed_commit');
  const idxSource = EXPECTED_HEADERS.indexOf('source');
  const allHashes = [];
  for (const cols of dataRows) {
    if (cols.length !== EXPECTED_HEADERS.length) continue;
    if (cols[idxIntro]) allHashes.push(cols[idxIntro]);
    if (cols[idxFix]) allHashes.push(cols[idxFix]);
    allHashes.push(...extractAnchorCommits(cols[idxSource]));
  }
  const commitMap = batchVerifyCommits(allHashes, resolvedRoot);
  const blobCache = new Map();
  const seenIds = new Set();

  for (let rowIndex = 0; rowIndex < dataRows.length; rowIndex += 1) {
    const cols = dataRows[rowIndex];
    const fallbackId = (cols[0] ?? '').trim() || `<row ${rowIndex + 2}>`;

    if (cols.length !== EXPECTED_HEADERS.length) {
      errors.push({
        id: fallbackId,
        field: 'row',
        message: `expected ${EXPECTED_HEADERS.length} columns but found ${cols.length}`,
      });
      continue;
    }

    const record = Object.fromEntries(EXPECTED_HEADERS.map((key, idx) => [key, cols[idx]]));
    const id = record.id;

    if (!id || id.trim() !== id) {
      errors.push({ id: fallbackId, field: 'id', message: 'id must be non-empty without leading/trailing whitespace' });
    } else if (seenIds.has(id)) {
      errors.push({ id, field: 'id', message: `duplicate id "${id}"` });
    } else {
      seenIds.add(id);
    }

    if (!record.title || record.title.trim() === '') {
      errors.push({ id: fallbackId, field: 'title', message: 'title must be non-empty' });
    }

    if (!ALLOWED_CLASSES.has(record.class)) {
      errors.push({
        id: fallbackId,
        field: 'class',
        message: `invalid class "${record.class}" (allowed: ${[...ALLOWED_CLASSES].join(', ')})`,
      });
    }

    if (!record.detected_how || record.detected_how.trim() === '') {
      errors.push({ id: fallbackId, field: 'detected_how', message: 'detected_how must be non-empty' });
    }

    if (!ALLOWED_SUITE_GREEN.has(record.was_suite_green)) {
      errors.push({
        id: fallbackId,
        field: 'was_suite_green',
        message: `invalid was_suite_green "${record.was_suite_green}" (expected yes|no|unknown)`,
      });
    }

    const introHash = record.introduced_commit;
    const fixHash = record.fixed_commit;

    if (introHash === '' && fixHash === '') {
      errors.push({
        id: fallbackId,
        field: 'introduced_commit,fixed_commit',
        message: 'at least one of introduced_commit or fixed_commit must be non-empty',
      });
    }

    let verifiedIntroDate = null;
    if (introHash === '') {
      if (record.introduced_date !== '') {
        errors.push({
          id: fallbackId,
          field: 'introduced_date',
          message: `expected "" when introduced_commit is empty, got "${record.introduced_date}"`,
        });
      }
    } else {
      const info = commitMap.get(introHash);
      if (!info || !info.ok) {
        errors.push({
          id: fallbackId,
          field: 'introduced_commit',
          message: info?.error ?? `unverified commit ${introHash}`,
        });
      } else {
        verifiedIntroDate = info.date;
        if (record.introduced_date !== info.date) {
          errors.push({
            id: fallbackId,
            field: 'introduced_date',
            message: `expected "${info.date}" from git %cI of ${introHash}, got "${record.introduced_date}"`,
          });
        }
      }
    }

    let verifiedFixedDate = null;
    if (fixHash === '') {
      if (record.fixed_date !== '') {
        errors.push({
          id: fallbackId,
          field: 'fixed_date',
          message: `expected "" when fixed_commit is empty, got "${record.fixed_date}"`,
        });
      }
    } else {
      const info = commitMap.get(fixHash);
      if (!info || !info.ok) {
        errors.push({
          id: fallbackId,
          field: 'fixed_commit',
          message: info?.error ?? `unverified commit ${fixHash}`,
        });
      } else {
        verifiedFixedDate = info.date;
        if (record.fixed_date !== info.date) {
          errors.push({
            id: fallbackId,
            field: 'fixed_date',
            message: `expected "${info.date}" from git %cI of ${fixHash}, got "${record.fixed_date}"`,
          });
        }
      }
    }

    if (!isValidIsoDay(record.detected_date)) {
      errors.push({
        id: fallbackId,
        field: 'detected_date',
        message: `detected_date "${record.detected_date}" must be a valid YYYY-MM-DD calendar date`,
      });
    } else {
      if (verifiedIntroDate !== null && record.detected_date < verifiedIntroDate.slice(0, 10)) {
        errors.push({
          id: fallbackId,
          field: 'detected_date',
          message: `detected_date (${record.detected_date}) precedes introduced_date (${verifiedIntroDate.slice(0, 10)})`,
        });
      }
      if (verifiedFixedDate !== null && record.detected_date > verifiedFixedDate.slice(0, 10)) {
        errors.push({
          id: fallbackId,
          field: 'detected_date',
          message: `detected_date (${record.detected_date}) follows fixed_date (${verifiedFixedDate.slice(0, 10)})`,
        });
      }
    }

    if (introHash === '' || fixHash === '') {
      if (record.days_to_fix !== '') {
        errors.push({
          id: fallbackId,
          field: 'days_to_fix',
          message: `expected "" when either commit is empty, got "${record.days_to_fix}"`,
        });
      }
    } else if (verifiedIntroDate !== null && verifiedFixedDate !== null) {
      const diff = computeDaysToFix(verifiedIntroDate, verifiedFixedDate);
      if (!diff.ok) {
        errors.push({ id: fallbackId, field: 'days_to_fix', message: diff.error });
      } else if (record.days_to_fix !== String(diff.days)) {
        errors.push({
          id: fallbackId,
          field: 'days_to_fix',
          message: `expected "${diff.days}" (Math.round((${verifiedFixedDate} - ${verifiedIntroDate}) / 86400000)), got "${record.days_to_fix}"`,
        });
      }
    }

    const sourceCheck = validateSourceAnchor(record.source, resolvedRoot, commitMap, blobCache);
    if (!sourceCheck.ok) {
      errors.push({ id: fallbackId, field: 'source', message: sourceCheck.error });
    }
  }

  return { ok: errors.length === 0, rowCount: dataRows.length, errors };
}

function csvCell(value) {
  if (/[",\r\n]/.test(value)) {
    return `"${value.replace(/"/g, '""')}"`;
  }
  return value;
}

function formatCsv(rows) {
  return [EXPECTED_HEADERS, ...rows].map((r) => r.map(csvCell).join(',')).join('\n') + '\n';
}

export function runSelfTest() {
  const dir = mkdtempSync(join(tmpdir(), 'anima-incident-dates-selftest-'));
  const checks = [];
  const recordCheck = (name, ok, detail = '') => {
    checks.push({ name, ok, detail });
  };

  try {
    const git = (args, extraEnv = {}) =>
      execFileSync('git', args, {
        cwd: dir,
        encoding: 'utf8',
        env: { ...process.env, ...extraEnv },
        stdio: ['ignore', 'pipe', 'pipe'],
      }).trim();

    const gitIdentityArgs = [
      '-c',
      'user.name=Test User',
      '-c',
      'user.email=test@example.com',
      '-c',
      'commit.gpgsign=false',
    ];

    git(['init']);

    writeFileSync(join(dir, 'notes.md'), 'line 1\n\nline 3\n', 'utf8');
    writeFileSync(join(dir, 'crlf.md'), 'crlf 1\r\ncrlf 2\r\n', 'utf8');
    git(['add', 'notes.md', 'crlf.md']);
    git([...gitIdentityArgs, 'commit', '-m', 'init'], {
      GIT_AUTHOR_DATE: '2026-10-01T10:00:00+07:00',
      GIT_COMMITTER_DATE: '2026-10-01T10:00:00+07:00',
    });
    const c1 = git(['rev-parse', '--short=7', 'HEAD']);
    const d1 = git(['show', '-s', '--format=%cI', c1]);

    // Off-trunk branch commit: valid git object, NOT an ancestor of trunk HEAD.
    git(['checkout', '-b', 'off-trunk']);
    writeFileSync(join(dir, 'off.md'), 'off trunk line 1\n', 'utf8');
    git(['add', 'off.md']);
    git([...gitIdentityArgs, 'commit', '-m', 'off-trunk'], {
      GIT_AUTHOR_DATE: '2026-10-02T12:00:00+07:00',
      GIT_COMMITTER_DATE: '2026-10-02T12:00:00+07:00',
    });
    const cOff = git(['rev-parse', '--short=7', 'HEAD']);
    const dOff = git(['show', '-s', '--format=%cI', cOff]);
    git(['checkout', '-']);

    writeFileSync(join(dir, 'notes.md'), 'line 1\n\nline 3\nline 4\n', 'utf8');
    git(['add', 'notes.md']);
    git([...gitIdentityArgs, 'commit', '-m', 'fix'], {
      GIT_AUTHOR_DATE: '2026-10-04T22:00:00+07:00',
      GIT_COMMITTER_DATE: '2026-10-04T22:00:00+07:00',
    });
    const c2 = git(['rev-parse', '--short=7', 'HEAD']);
    const d2 = git(['show', '-s', '--format=%cI', c2]);

    const csvPath = join(dir, 'incidents.csv');
    const validRows = [
      [
        'INC-T1',
        'Title, with comma and "quotes"',
        'vacuous-gate',
        c1,
        c2,
        'manual audit',
        d1,
        '2026-10-03',
        d2,
        '4',
        'yes',
        `notes.md@${c2}:1`,
      ],
      [
        'INC-T2',
        'Missing intro commit',
        'other',
        '',
        c2,
        'detected before fix',
        '',
        '2026-10-02',
        d2,
        '',
        'yes',
        `notes.md@${c2}:4`,
      ],
      [
        'INC-T3',
        'Open unfixed incident',
        'vacuous-gate',
        c1,
        '',
        'detected before fix',
        d1,
        '2026-10-02',
        '',
        '',
        'yes',
        `notes.md@${c1}:3`,
      ],
    ];

    writeFileSync(csvPath, formatCsv(validRows), 'utf8');
    const pos = checkIncidentsCsv({ csvPath, repoRoot: dir });
    recordCheck(
      'positive control passes with 12-column schema and pinned anchors',
      pos.ok && pos.rowCount === 3,
      pos.ok ? '' : JSON.stringify(pos.errors),
    );

    // Inserting 5 lines into working-tree notes.md must NOT break pinned anchors.
    const originalNotes = readFileSync(join(dir, 'notes.md'), 'utf8');
    writeFileSync(join(dir, 'notes.md'), `\n\n\n\n\n${originalNotes}`, 'utf8');
    const shiftedRes = checkIncidentsCsv({ csvPath, repoRoot: dir });
    recordCheck(
      'pinned anchor survives prepending 5 working-tree lines',
      shiftedRes.ok,
      shiftedRes.ok ? '' : JSON.stringify(shiftedRes.errors),
    );
    writeFileSync(join(dir, 'notes.md'), originalNotes, 'utf8');

    const mutateAndExpectFail = (label, mutator, expectedId, expectedField) => {
      const cloned = validRows.map((r) => [...r]);
      mutator(cloned);
      writeFileSync(csvPath, formatCsv(cloned), 'utf8');
      const res = checkIncidentsCsv({ csvPath, repoRoot: dir });
      const matched =
        !res.ok && res.errors.some((e) => e.id === expectedId && e.field.includes(expectedField));
      recordCheck(label, matched, matched ? '' : JSON.stringify(res.errors));
    };

    mutateAndExpectFail(
      'rejects tampered introduced_date',
      (r) => {
        r[0][6] = '2026-10-02T10:00:00+07:00';
      },
      'INC-T1',
      'introduced_date',
    );
    mutateAndExpectFail(
      'rejects tampered fixed_date',
      (r) => {
        r[0][8] = '2026-10-05T22:00:00+07:00';
      },
      'INC-T1',
      'fixed_date',
    );
    mutateAndExpectFail(
      'rejects invalid calendar detected_date',
      (r) => {
        r[0][7] = '2026-02-30';
      },
      'INC-T1',
      'detected_date',
    );
    mutateAndExpectFail(
      'rejects detected_date after fixed_date',
      (r) => {
        r[0][7] = '2026-10-05';
      },
      'INC-T1',
      'detected_date',
    );
    mutateAndExpectFail(
      'rejects detected_date before introduced_date',
      (r) => {
        r[0][7] = '2026-09-30';
      },
      'INC-T1',
      'detected_date',
    );
    mutateAndExpectFail(
      'rejects tampered days_to_fix',
      (r) => {
        r[0][9] = '3';
      },
      'INC-T1',
      'days_to_fix',
    );
    mutateAndExpectFail(
      'rejects invalid class enum',
      (r) => {
        r[0][2] = 'invented-class';
      },
      'INC-T1',
      'class',
    );
    mutateAndExpectFail(
      'rejects invalid was_suite_green enum',
      (r) => {
        r[0][10] = 'maybe';
      },
      'INC-T1',
      'was_suite_green',
    );
    mutateAndExpectFail(
      'rejects nonexistent commit hash',
      (r) => {
        r[0][3] = 'deadbeef';
      },
      'INC-T1',
      'introduced_commit',
    );
    mutateAndExpectFail(
      'rejects off-trunk commit not ancestor of HEAD',
      (r) => {
        r[1][3] = cOff;
        r[1][6] = dOff;
        r[1][9] = '3';
      },
      'INC-T2',
      'introduced_commit',
    );
    mutateAndExpectFail(
      'rejects unpinned source anchor without @sha',
      (r) => {
        r[0][11] = 'notes.md:1';
      },
      'INC-T1',
      'source',
    );
    mutateAndExpectFail(
      'rejects blank line at pinned source anchor',
      (r) => {
        r[0][11] = `notes.md@${c2}:2`;
      },
      'INC-T1',
      'source',
    );
    mutateAndExpectFail(
      'rejects out-of-range source line',
      (r) => {
        r[0][11] = `notes.md@${c2}:999`;
      },
      'INC-T1',
      'source',
    );
    mutateAndExpectFail(
      'rejects missing source file at commit',
      (r) => {
        r[0][11] = `missing.md@${c2}:1`;
      },
      'INC-T1',
      'source',
    );
    mutateAndExpectFail(
      'rejects path traversal in source',
      (r) => {
        r[0][11] = `../notes.md@${c2}:1`;
      },
      'INC-T1',
      'source',
    );
    mutateAndExpectFail(
      'rejects both commits empty',
      (r) => {
        r[1][4] = '';
        r[1][8] = '';
      },
      'INC-T2',
      'introduced_commit',
    );
    mutateAndExpectFail(
      'rejects fixed_date earlier than introduced_date',
      (r) => {
        r[0][3] = c2;
        r[0][4] = c1;
        r[0][6] = d2;
        r[0][7] = '2026-10-04';
        r[0][8] = d1;
        r[0][9] = '-4';
      },
      'INC-T1',
      'days_to_fix',
    );
    mutateAndExpectFail(
      'rejects non-empty date when commit is empty',
      (r) => {
        r[1][6] = d1;
      },
      'INC-T2',
      'introduced_date',
    );

    mkdirSync(join(dir, 'sub'), { recursive: true });
    writeFileSync(join(dir, 'sub', 'blob.txt'), 'hello\n', 'utf8');
    const blobSha = git(['hash-object', '-w', 'sub/blob.txt']).slice(0, 7);
    mutateAndExpectFail(
      'rejects non-commit git object hash (blob)',
      (r) => {
        r[0][3] = blobSha;
      },
      'INC-T1',
      'introduced_commit',
    );

    mutateAndExpectFail(
      'rejects trailing-newline off-by-1 source line (LF)',
      (r) => {
        r[0][11] = `notes.md@${c2}:5`;
      },
      'INC-T1',
      'source',
    );

    recordCheck(
      'accepts final physical line of CRLF-terminated file at pinned commit',
      validateSourceAnchor(`crlf.md@${c1}:2`, dir).ok,
    );
    mutateAndExpectFail(
      'rejects trailing-newline off-by-1 source line (CRLF)',
      (r) => {
        r[0][11] = `crlf.md@${c1}:3`;
      },
      'INC-T1',
      'source',
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }

  return {
    ok: checks.every((c) => c.ok),
    checks,
  };
}

function parseCliArgs(argv) {
  let check = false;
  let selfTest = false;
  let csvPath = 'docs/paper/data/incidents.csv';
  let repoRoot = resolve(import.meta.dirname, '../..');

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--check') {
      check = true;
    } else if (arg === '--self-test') {
      selfTest = true;
    } else if (arg === '--csv') {
      const next = argv[i + 1];
      if (!next) throw new Error('missing value for --csv');
      csvPath = next;
      i += 1;
    } else if (arg === '--root') {
      const next = argv[i + 1];
      if (!next) throw new Error('missing value for --root');
      repoRoot = next;
      i += 1;
    } else {
      throw new Error(`unknown argument "${arg}"`);
    }
  }

  return { check, selfTest, csvPath, repoRoot };
}

function main(argv) {
  let opts;
  try {
    opts = parseCliArgs(argv);
  } catch (err) {
    console.error(`incident_dates: ${err instanceof Error ? err.message : String(err)}`);
    console.error('usage: incident_dates.mjs --check [--csv <path>] [--root <path>] | --self-test');
    process.exit(2);
  }

  if (!opts.check && !opts.selfTest) {
    console.error('usage: incident_dates.mjs --check [--csv <path>] [--root <path>] | --self-test');
    process.exit(2);
  }

  if (opts.selfTest) {
    const result = runSelfTest();
    for (const c of result.checks) {
      console.log(`${c.ok ? 'ok  ' : 'FAIL'} ${c.name}${c.detail ? ` (${c.detail})` : ''}`);
    }
    const failed = result.checks.filter((c) => !c.ok).length;
    if (failed > 0) {
      console.error(`incident_dates self-test: ${failed} control(s) failed`);
      process.exit(1);
    }
    console.log(`incident_dates self-test: ${result.checks.length} controls ok`);
  }

  if (opts.check) {
    const result = checkIncidentsCsv({ csvPath: opts.csvPath, repoRoot: opts.repoRoot });
    if (!result.ok) {
      for (const err of result.errors) {
        console.error(`[${err.id}] ${err.field}: ${err.message}`);
      }
      process.exit(1);
    }
    console.log(`incident_dates: OK (${result.rowCount} rows verified)`);
  }
}

function canonicalizePath(filePath) {
  const resolved = resolve(filePath);
  try {
    return realpathSync(resolved);
  } catch {
    return resolved;
  }
}

if (
  process.argv[1] &&
  canonicalizePath(fileURLToPath(import.meta.url)) === canonicalizePath(process.argv[1])
) {
  main(process.argv.slice(2));
}
