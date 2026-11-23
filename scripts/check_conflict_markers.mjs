// Refuse to land unresolved Git merge-conflict markers in tracked or untracked non-ignored files.
// Usage: node scripts/check_conflict_markers.mjs [--self-test]

import { execFileSync } from 'node:child_process';
import { lstatSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

export const MIN_SCANNED_FILES = 100;
const MARKER_RE = /^(<{7}|\|{7}|>{7})(?:[ \t].*)?$/;
const KIND_BY_CHAR = {
  '<': ['conflict-start', 'unresolved conflict start marker'],
  '|': ['conflict-base', 'unresolved diff3 base marker'],
  '>': ['conflict-end', 'unresolved conflict end marker'],
};

export function isBinaryBuffer(buf) {
  return Boolean(buf?.length && buf.subarray(0, 8192).includes(0));
}

export function scanText(text, relPath) {
  if (!text) return [];
  const normalized = text.startsWith('\uFEFF') ? text.slice(1) : text;
  if (!normalized) return [];
  const file = String(relPath).replaceAll('\\', '/');
  const lines = normalized.split(/\r\n|\r|\n/);
  const violations = [];
  for (let i = 0; i < lines.length; i += 1) {
    if (!MARKER_RE.test(lines[i])) continue;
    const [kind, message] = KIND_BY_CHAR[lines[i][0]];
    violations.push({ file, line: i + 1, kind, message });
  }
  return violations;
}

export function scanRepo(rootDir = process.cwd(), { minFiles = MIN_SCANNED_FILES } = {}) {
  const root = resolve(rootDir);
  const out = execFileSync('git', ['ls-files', '-z', '--cached', '--others', '--exclude-standard'], {
    cwd: root,
    encoding: 'utf8',
    maxBuffer: 1 << 28,
  });
  const violations = [];
  let scannedFiles = 0;
  for (const rel of new Set(out.split('\0').filter(Boolean))) {
    const absPath = join(root, rel);
    try {
      const st = lstatSync(absPath);
      if (st.isSymbolicLink() || !st.isFile()) continue;
      const buf = readFileSync(absPath);
      if (isBinaryBuffer(buf)) continue;
      scannedFiles += 1;
      violations.push(...scanText(buf.toString('utf8'), rel));
    } catch { /* deleted or unreadable */ }
  }
  return { root, scannedFiles, violations, minFiles, underFloor: scannedFiles < minFiles };
}

export function runNegativeControl() {
  const [lt7, bar7, eq7, gt7] = ['<', '|', '=', '>'].map((c) => c.repeat(7));
  const dirty = ['```diff', `${lt7} HEAD`, `${bar7} base`, eq7, `${gt7} main`, '```'].join('\n');
  const got = scanText(dirty, 'docs/README.md').map((v) => `${v.line}:${v.kind}`).join(',');
  if (got !== '2:conflict-start,3:conflict-base,5:conflict-end') {
    return { ok: false, error: `unexpected conflict scan: ${got}` };
  }
  const clean = scanText(`Heading\n${eq7}\n\n// ${'='.repeat(20)}\n`, 'docs/clean.md');
  return clean.length === 0 ? { ok: true, error: '' } : { ok: false, error: JSON.stringify(clean) };
}

function main() {
  const args = process.argv.slice(2);
  if (args.length === 1 && args[0] === '--self-test') {
    const ctrl = runNegativeControl();
    if (!ctrl.ok) { console.error(`self-test FAIL: ${ctrl.error}`); process.exit(1); }
    console.log('conflict markers self-test: ok');
    process.exit(0);
  }
  if (args.length > 0) {
    console.error('Usage: node scripts/check_conflict_markers.mjs [--self-test]');
    process.exit(2);
  }
  const res = scanRepo(process.cwd());
  if (res.violations.length > 0) {
    for (const v of res.violations) console.error(`${v.file}:${v.line}: ${v.message}`);
    console.error(`conflict markers: ${res.violations.length} marker(s) found across ${res.scannedFiles} file(s)`);
    process.exit(1);
  }
  if (res.underFloor) {
    console.error(`conflict markers: scanned only ${res.scannedFiles} file(s) (minimum ${res.minFiles})`);
    process.exit(1);
  }
  console.log(`conflict markers: ${res.scannedFiles} file(s) scanned, 0 markers found`);
}

if (process.argv[1] && resolve(fileURLToPath(import.meta.url)) === resolve(process.argv[1])) main();
