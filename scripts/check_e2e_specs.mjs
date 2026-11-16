#!/usr/bin/env node
// Gate: `npm run test:e2e --prefix tests` must schedule exactly the specs committed in
// `scripts/e2e_spec_policy.mjs`. See that file for why.
//
// It asks Playwright what the script in `tests/package.json` would schedule (`--list`, which
// starts no server and runs no test) and compares that to the list in BOTH directions. Narrowing
// `test:e2e` to a directory, a file or a `--grep` therefore turns this red and names the specs.
//
// Usage: node scripts/check_e2e_specs.mjs
// Exit 0 = agreement, 1 = disagreement, 2 = a negative control passed or the listing failed.

import { spawnSync } from 'node:child_process';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { EXCLUDED_SPECS, SCHEDULED_SPECS } from './e2e_spec_policy.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const TESTS_DIR = path.join(ROOT, 'tests');

/**
 * Pure comparison. Returns a list of problems; empty means agreement.
 * @param {string[]} scheduled specs the run schedules
 * @param {string[]} onDisk every `*.spec.ts` under tests/e2e
 * @param {string[]} committed the committed list
 * @param {{name: string, reason: string}[]} excluded
 */
export function compare(scheduled, onDisk, committed, excluded) {
  const problems = [];
  const sched = new Set(scheduled);
  const list = new Set(committed);
  const excl = new Set(excluded.map((e) => e.name));
  for (const e of excluded) {
    if (!e.reason || !e.reason.trim()) problems.push(`EXCLUDED entry has no reason: ${e.name}`);
    if (list.has(e.name)) problems.push(`in both SCHEDULED_SPECS and EXCLUDED_SPECS: ${e.name}`);
  }
  for (const s of [...sched].sort()) {
    if (!list.has(s)) problems.push(`SPEC MISSING FROM LIST: run schedules ${s} but the committed list does not name it`);
  }
  for (const s of [...list].sort()) {
    if (!sched.has(s)) problems.push(`SPEC MISSING FROM RUN: committed list names ${s} but test:e2e does not schedule it`);
  }
  for (const s of [...excl].sort()) {
    if (!onDisk.includes(s)) problems.push(`STALE EXCLUSION: ${s} is in EXCLUDED_SPECS but no such file exists`);
    if (sched.has(s)) problems.push(`STALE EXCLUSION: ${s} is in EXCLUDED_SPECS but the run now schedules it`);
  }
  for (const s of [...onDisk].sort()) {
    if (!sched.has(s) && !excl.has(s)) problems.push(`ORPHAN SPEC: ${s} exists on disk but is neither scheduled nor excluded with a reason`);
  }
  return problems;
}

function walk(dir) {
  const out = [];
  for (const name of readdirSync(dir)) {
    if (name === 'node_modules') continue;
    const p = path.join(dir, name);
    if (statSync(p).isDirectory()) out.push(...walk(p));
    else if (name.endsWith('.spec.ts')) out.push(path.relative(ROOT, p).split(path.sep).join('/'));
  }
  return out;
}

function listScheduled() {
  const pkg = JSON.parse(readFileSync(path.join(TESTS_DIR, 'package.json'), 'utf8'));
  const script = pkg.scripts?.['test:e2e'];
  const prefix = 'playwright test';
  if (typeof script !== 'string' || !script.startsWith(prefix)) {
    console.error(`tests/package.json "test:e2e" must start with "${prefix}", got: ${JSON.stringify(script)}`);
    process.exit(2);
  }
  const args = script.slice(prefix.length).trim().split(/\s+/).filter(Boolean);
  const bin = path.join(TESTS_DIR, 'node_modules', '.bin', process.platform === 'win32' ? 'playwright.cmd' : 'playwright');
  const r = spawnSync(bin, ['test', ...args, '--list', '--reporter=json'], {
    cwd: TESTS_DIR,
    encoding: 'utf8',
    maxBuffer: 256 * 1024 * 1024,
    shell: process.platform === 'win32',
  });
  if (r.status !== 0) {
    console.error(`playwright --list failed (exit ${r.status})\n${r.stderr}`);
    process.exit(2);
  }
  const report = JSON.parse(r.stdout);
  const rootDir = report.config.rootDir;
  const files = new Set();
  const visit = (suite) => {
    if (suite.specs?.length && suite.file) {
      files.add(path.relative(ROOT, path.resolve(rootDir, suite.file)).split(path.sep).join('/'));
    }
    for (const child of suite.suites ?? []) visit(child);
  };
  for (const s of report.suites ?? []) visit(s);
  return [...files].sort();
}

function negativeControls() {
  const onDisk = [...SCHEDULED_SPECS, ...EXCLUDED_SPECS.map((e) => e.name)];
  const base = () => compare(SCHEDULED_SPECS, onDisk, SCHEDULED_SPECS, EXCLUDED_SPECS);
  const cases = [
    ['baseline agrees', base, (p) => p.length === 0],
    ['spec dropped from the run is caught', () => compare(SCHEDULED_SPECS.slice(1), onDisk, SCHEDULED_SPECS, EXCLUDED_SPECS), (p) => p.some((x) => x.includes(SCHEDULED_SPECS[0]) && x.startsWith('SPEC MISSING FROM RUN'))],
    ['spec dropped from the list is caught', () => compare(SCHEDULED_SPECS, onDisk, SCHEDULED_SPECS.slice(1), EXCLUDED_SPECS), (p) => p.some((x) => x.includes(SCHEDULED_SPECS[0]) && x.startsWith('SPEC MISSING FROM LIST'))],
    ['orphan spec on disk is caught', () => compare(SCHEDULED_SPECS, [...onDisk, 'tests/e2e/new.spec.ts'], SCHEDULED_SPECS, EXCLUDED_SPECS), (p) => p.some((x) => x.startsWith('ORPHAN SPEC'))],
    ['reasonless exclusion is caught', () => compare(SCHEDULED_SPECS, onDisk, SCHEDULED_SPECS, [{ name: EXCLUDED_SPECS[0].name, reason: ' ' }, ...EXCLUDED_SPECS.slice(1)]), (p) => p.some((x) => x.includes('no reason'))],
  ];
  for (const [name, run, ok] of cases) {
    if (!ok(run())) {
      console.error(`negative control FAILED to detect: ${name}`);
      process.exit(2);
    }
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  negativeControls();
  const scheduled = listScheduled();
  const onDisk = walk(path.join(TESTS_DIR, 'e2e'));
  const problems = compare(scheduled, onDisk, SCHEDULED_SPECS, EXCLUDED_SPECS);
  if (problems.length) {
    console.error(`E2E spec gate RED (${problems.length} problem(s)):`);
    for (const p of problems) console.error(`  - ${p}`);
    process.exit(1);
  }
  console.log(`E2E spec gate green: ${scheduled.length} scheduled spec file(s) match the committed list; ${EXCLUDED_SPECS.length} excluded with reasons.`);
}
