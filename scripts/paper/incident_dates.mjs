#!/usr/bin/env node
// Card P1: verify docs/paper/data/incidents.csv against git history and source anchors.
//
// Usage:
//   node scripts/paper/incident_dates.mjs --check [--csv <path>] [--root <path>]
//   node scripts/paper/incident_dates.mjs --self-test

import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { isAbsolute, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const EXPECTED_HEADERS = [
  'id',
  'title',
  'class',
  'introduced_commit',
  'detected_commit',
  'detected_how',
  'introduced_date',
  'detected_date',
  'days_to_detection',
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
const SOURCE_ANCHOR_RE = /^([^:\r\n]+):(\d+)$/;
const MS_PER_DAY = 86_400_000;

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

export function verifyCommitAndGetDate(hash, repoRoot) {
  if (!COMMIT_HASH_RE.test(hash)) {
    return { ok: false, error: `invalid commit hash syntax "${hash}" (expected 7-40 hex chars)` };
  }

  let objectType;
  try {
    objectType = execFileSync('git', ['cat-file', '-t', hash], {
      cwd: repoRoot,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    }).trim();
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    return { ok: false, error: `git cat-file -t ${hash} failed: ${msg.split(/\r?\n/)[0]}` };
  }

  if (objectType !== 'commit') {
    return { ok: false, error: `git cat-file -t ${hash} returned "${objectType}" instead of "commit"` };
  }

  try {
    const committerDate = execFileSync('git', ['show', '-s', '--format=%cI', hash], {
      cwd: repoRoot,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    }).trim();
    if (!committerDate || Number.isNaN(Date.parse(committerDate))) {
      return { ok: false, error: `git show -s --format=%cI ${hash} returned invalid date "${committerDate}"` };
    }
    return { ok: true, date: committerDate };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    return { ok: false, error: `git show -s --format=%cI ${hash} failed: ${msg.split(/\r?\n/)[0]}` };
  }
}

export function validateSourceAnchor(sourceText, repoRoot, lineCountCache = new Map()) {
  if (!sourceText || sourceText.trim() === '') {
    return { ok: false, error: 'source anchor is empty' };
  }

  const segments = sourceText.split(';').map((part) => part.trim());
  for (const segment of segments) {
    if (!segment) {
      return { ok: false, error: `empty anchor segment in "${sourceText}"` };
    }
    const match = SOURCE_ANCHOR_RE.exec(segment);
    if (!match) {
      return { ok: false, error: `invalid source anchor "${segment}" (expected path:line)` };
    }

    const relPath = match[1];
    const lineNum = Number.parseInt(match[2], 10);

    if (isAbsolute(relPath) || relPath.startsWith('/') || relPath.startsWith('\\') || /^[A-Za-z]:/.test(relPath)) {
      return { ok: false, error: `source path "${relPath}" must be relative to repo root` };
    }
    if (relPath.split(/[\\/]/).includes('..')) {
      return { ok: false, error: `source path "${relPath}" must not contain ".."` };
    }

    const resolvedRoot = resolve(repoRoot);
    const resolvedFile = resolve(resolvedRoot, relPath);
    const relFromRoot = relative(resolvedRoot, resolvedFile);
    if (relFromRoot.startsWith('..') || isAbsolute(relFromRoot)) {
      return { ok: false, error: `source path "${relPath}" escapes repo root` };
    }

    if (!existsSync(resolvedFile)) {
      return { ok: false, error: `source file "${relPath}" does not exist` };
    }

    const stats = statSync(resolvedFile);
    if (!stats.isFile()) {
      return { ok: false, error: `source path "${relPath}" is not a regular file` };
    }

    let lineCount = lineCountCache.get(resolvedFile);
    if (lineCount === undefined) {
      const content = readFileSync(resolvedFile, 'utf8');
      const normalized = content.replace(/\r?\n$/, '');
      lineCount = content.length === 0 ? 0 : normalized.split(/\r?\n/).length;
      lineCountCache.set(resolvedFile, lineCount);
    }

    if (!Number.isInteger(lineNum) || lineNum < 1 || lineNum > lineCount) {
      return {
        ok: false,
        error: `source line ${lineNum} in "${relPath}" is out of range (1..${lineCount})`,
      };
    }
  }

  return { ok: true };
}

export function computeDaysToDetection(introducedIso, detectedIso) {
  const introducedMs = Date.parse(introducedIso);
  const detectedMs = Date.parse(detectedIso);
  if (Number.isNaN(introducedMs) || Number.isNaN(detectedMs)) {
    return { ok: false, error: `unparseable ISO date(s): introduced="${introducedIso}", detected="${detectedIso}"` };
  }
  if (detectedMs < introducedMs) {
    return {
      ok: false,
      error: `detected_date (${detectedIso}) precedes introduced_date (${introducedIso})`,
    };
  }
  return { ok: true, days: Math.round((detectedMs - introducedMs) / MS_PER_DAY) };
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
    // Not a git repository: the per-row commit checks report it.
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

  const seenIds = new Set();
  const commitCache = new Map();
  const lineCountCache = new Map();

  const getCommitInfo = (hash) => {
    let cached = commitCache.get(hash);
    if (!cached) {
      cached = verifyCommitAndGetDate(hash, resolvedRoot);
      commitCache.set(hash, cached);
    }
    return cached;
  };

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
    const detHash = record.detected_commit;

    if (introHash === '' && detHash === '') {
      errors.push({
        id: fallbackId,
        field: 'introduced_commit,detected_commit',
        message: 'at least one of introduced_commit or detected_commit must be non-empty',
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
      const info = getCommitInfo(introHash);
      if (!info.ok) {
        errors.push({ id: fallbackId, field: 'introduced_commit', message: info.error });
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

    let verifiedDetDate = null;
    if (detHash === '') {
      if (record.detected_date !== '') {
        errors.push({
          id: fallbackId,
          field: 'detected_date',
          message: `expected "" when detected_commit is empty, got "${record.detected_date}"`,
        });
      }
    } else {
      const info = getCommitInfo(detHash);
      if (!info.ok) {
        errors.push({ id: fallbackId, field: 'detected_commit', message: info.error });
      } else {
        verifiedDetDate = info.date;
        if (record.detected_date !== info.date) {
          errors.push({
            id: fallbackId,
            field: 'detected_date',
            message: `expected "${info.date}" from git %cI of ${detHash}, got "${record.detected_date}"`,
          });
        }
      }
    }

    if (introHash === '' || detHash === '') {
      if (record.days_to_detection !== '') {
        errors.push({
          id: fallbackId,
          field: 'days_to_detection',
          message: `expected "" when either commit is empty, got "${record.days_to_detection}"`,
        });
      }
    } else if (verifiedIntroDate !== null && verifiedDetDate !== null) {
      const diff = computeDaysToDetection(verifiedIntroDate, verifiedDetDate);
      if (!diff.ok) {
        errors.push({ id: fallbackId, field: 'days_to_detection', message: diff.error });
      } else if (record.days_to_detection !== String(diff.days)) {
        errors.push({
          id: fallbackId,
          field: 'days_to_detection',
          message: `expected "${diff.days}" (Math.round((${verifiedDetDate} - ${verifiedIntroDate}) / 86400000)), got "${record.days_to_detection}"`,
        });
      }
    }

    const sourceCheck = validateSourceAnchor(record.source, resolvedRoot, lineCountCache);
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

    git(['init']);
    git(['config', 'user.name', 'Test User']);
    git(['config', 'user.email', 'test@example.com']);

    writeFileSync(join(dir, 'notes.md'), 'line 1\nline 2\nline 3\n', 'utf8');
    git(['add', 'notes.md']);
    git(['commit', '-m', 'init'], {
      GIT_AUTHOR_DATE: '2026-10-01T10:00:00+07:00',
      GIT_COMMITTER_DATE: '2026-10-01T10:00:00+07:00',
    });
    const c1 = git(['rev-parse', '--short=7', 'HEAD']);
    const d1 = git(['show', '-s', '--format=%cI', c1]);

    writeFileSync(join(dir, 'notes.md'), 'line 1\nline 2\nline 3\nline 4\n', 'utf8');
    git(['add', 'notes.md']);
    git(['commit', '-m', 'detect'], {
      GIT_AUTHOR_DATE: '2026-10-04T22:00:00+07:00',
      GIT_COMMITTER_DATE: '2026-10-04T22:00:00+07:00',
    });
    const c2 = git(['rev-parse', '--short=7', 'HEAD']);
    const d2 = git(['show', '-s', '--format=%cI', c2]);

    const csvPath = join(dir, 'incidents.csv');
    const validRows = [
      ['INC-T1', 'Title, with comma and "quotes"', 'vacuous-gate', c1, c2, 'manual audit', d1, d2, '4', 'yes', 'notes.md:2'],
      ['INC-T2', 'Missing intro commit', 'other', '', c2, 'detected at c2', '', d2, '', 'unknown', 'notes.md:4'],
    ];

    writeFileSync(csvPath, formatCsv(validRows), 'utf8');
    const pos = checkIncidentsCsv({ csvPath, repoRoot: dir });
    recordCheck('positive control passes with quoted fields and rounded day count', pos.ok && pos.rowCount === 2);

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
      'rejects tampered detected_date',
      (r) => {
        r[0][7] = '2026-10-05T22:00:00+07:00';
      },
      'INC-T1',
      'detected_date',
    );
    mutateAndExpectFail(
      'rejects tampered days_to_detection',
      (r) => {
        r[0][8] = '3';
      },
      'INC-T1',
      'days_to_detection',
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
        r[0][9] = 'maybe';
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
      'rejects out-of-range source line',
      (r) => {
        r[0][10] = 'notes.md:999';
      },
      'INC-T1',
      'source',
    );
    mutateAndExpectFail(
      'rejects missing source file',
      (r) => {
        r[0][10] = 'missing.md:1';
      },
      'INC-T1',
      'source',
    );
    mutateAndExpectFail(
      'rejects path traversal in source',
      (r) => {
        r[0][10] = '../notes.md:1';
      },
      'INC-T1',
      'source',
    );
    mutateAndExpectFail(
      'rejects both commits empty',
      (r) => {
        r[1][4] = '';
        r[1][7] = '';
      },
      'INC-T2',
      'introduced_commit',
    );
    mutateAndExpectFail(
      'rejects detected_date earlier than introduced_date',
      (r) => {
        r[0][3] = c2;
        r[0][4] = c1;
        r[0][6] = d2;
        r[0][7] = d1;
        r[0][8] = '-4';
      },
      'INC-T1',
      'days_to_detection',
    );
    mutateAndExpectFail(
      'rejects non-empty date when commit is empty',
      (r) => {
        r[1][6] = d1;
      },
      'INC-T2',
      'introduced_date',
    );

    // Ensure non-commit git object (blob) is rejected
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

    // Ensure trailing newline does not inflate physical lineCount by 1
    mutateAndExpectFail(
      'rejects trailing-newline off-by-1 source line (LF)',
      (r) => {
        r[0][10] = 'notes.md:5';
      },
      'INC-T1',
      'source',
    );

    writeFileSync(join(dir, 'crlf.md'), 'line 1\r\nline 2\r\n', 'utf8');
    recordCheck(
      'accepts final physical line of CRLF-terminated source file',
      validateSourceAnchor('crlf.md:2', dir).ok,
    );
    mutateAndExpectFail(
      'rejects trailing-newline off-by-1 source line (CRLF)',
      (r) => {
        r[0][10] = 'crlf.md:3';
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
