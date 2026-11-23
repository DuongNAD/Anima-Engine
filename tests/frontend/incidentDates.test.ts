import { describe, expect, it } from 'vitest';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
// @ts-expect-error -- incident_dates.mjs has no .d.mts declaration
import { EXPECTED_HEADERS, checkIncidentsCsv, parseCsv, runSelfTest, validateSourceAnchor } from '../../scripts/paper/incident_dates.mjs';

interface CheckError {
  id: string;
  field: string;
  message: string;
}

interface CheckResult {
  ok: boolean;
  rowCount: number;
  errors: CheckError[];
}

interface SelfTestResult {
  ok: boolean;
  checks: Array<{ name: string; ok: boolean; detail: string }>;
}

const REPO_ROOT = resolve(import.meta.dirname, '../..');
const SCRIPT_PATH = resolve(REPO_ROOT, 'scripts/paper/incident_dates.mjs');
const CANONICAL_CSV_PATH = resolve(REPO_ROOT, 'docs/paper/data/incidents.csv');

function csvCell(value: string): string {
  if (/[",\r\n]/.test(value)) {
    return `"${value.replace(/"/g, '""')}"`;
  }
  return value;
}

function serializeCsv(rows: string[][]): string {
  return rows.map((row) => row.map(csvCell).join(',')).join('\n') + '\n';
}

function withMutatedCsv(
  mutator: (rows: string[][]) => void,
  fn: (tmpCsvPath: string) => void,
): void {
  const dir = mkdtempSync(join(tmpdir(), 'anima-incident-dates-test-'));
  try {
    const raw = readFileSync(CANONICAL_CSV_PATH, 'utf8');
    const rows = (parseCsv as (text: string) => string[][])(raw).map((r) => [...r]);
    mutator(rows);
    const tmpCsv = join(dir, 'incidents.csv');
    writeFileSync(tmpCsv, serializeCsv(rows), 'utf8');
    fn(tmpCsv);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

function runCli(args: string[]): { status: number | null; stdout: string; stderr: string } {
  const res = spawnSync(process.execPath, [SCRIPT_PATH, ...args], {
    cwd: REPO_ROOT,
    encoding: 'utf8',
  });
  return {
    status: res.status,
    stdout: res.stdout,
    stderr: res.stderr,
  };
}

describe('scripts/paper/incident_dates.mjs (Card P1)', () => {
  it('reports a shallow clone once instead of every catalogued commit as missing', () => {
    const dir = mkdtempSync(join(tmpdir(), 'anima-incident-dates-shallow-'));
    try {
      const origin = join(dir, 'origin');
      const clone = join(dir, 'clone');
      const git = (cwd: string, args: string[]) =>
        execFileSync(
          'git',
          ['-c', 'user.name=Test', '-c', 'user.email=test@example.com', '-c', 'commit.gpgsign=false', ...args],
          { cwd, stdio: ['ignore', 'pipe', 'pipe'] },
        );
      execFileSync('git', ['init', origin], { stdio: 'ignore' });
      for (const n of ['1', '2']) {
        writeFileSync(join(origin, 'f.txt'), n, 'utf8');
        git(origin, ['add', 'f.txt']);
        git(origin, ['commit', '-m', n]);
      }
      execFileSync('git', ['clone', '--depth', '1', pathToFileURL(origin).href, clone], { stdio: 'ignore' });
      writeFileSync(join(clone, 'incidents.csv'), readFileSync(CANONICAL_CSV_PATH, 'utf8'), 'utf8');

      const result = (checkIncidentsCsv as (o: { csvPath: string; repoRoot: string }) => CheckResult)({
        csvPath: 'incidents.csv',
        repoRoot: clone,
      });
      expect(result.ok).toBe(false);
      expect(result.errors).toHaveLength(1);
      expect(result.errors[0].field).toBe('git');
      expect(result.errors[0].message).toContain('shallow clone');
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it('verifies the committed 47-row docs/paper/data/incidents.csv cleanly with was_suite_green=yes only', () => {
    const run = runCli(['--check']);
    expect(run.status).toBe(0);
    expect(run.stderr).toBe('');
    expect(run.stdout).toMatch(/^incident_dates: OK \(47 rows verified\)\r?\n$/);

    const apiResult = (
      checkIncidentsCsv as (opts: { csvPath: string; repoRoot: string }) => CheckResult
    )({
      csvPath: 'docs/paper/data/incidents.csv',
      repoRoot: REPO_ROOT,
    });
    expect(apiResult.ok).toBe(true);
    expect(apiResult.errors).toEqual([]);
    expect(apiResult.rowCount).toBe(47);

    const parsed = (parseCsv as (text: string) => string[][])(readFileSync(CANONICAL_CSV_PATH, 'utf8'));
    expect(parsed[0]).toEqual(EXPECTED_HEADERS);
    for (const row of parsed.slice(1)) {
      expect(row[10]).toBe('yes');
    }
  });

  it('passes --self-test positive and negative controls', () => {
    const run = runCli(['--self-test']);
    expect(run.status).toBe(0);
    expect(run.stderr).toBe('');
    expect(run.stdout).toMatch(/incident_dates self-test: \d+ controls ok/);

    const apiResult = (runSelfTest as () => SelfTestResult)();
    expect(apiResult.ok).toBe(true);
    expect(apiResult.checks.every((c) => c.ok)).toBe(true);
  });

  it('parses RFC 4180 CSV edge cases and rejects malformed quotes', () => {
    const parser = parseCsv as (text: string) => string[][];
    const parsed = parser(
      'a,b,c\r\n"hello, world","say ""hi""",plain\n,,""\n',
    );
    expect(parsed).toEqual([
      ['a', 'b', 'c'],
      ['hello, world', 'say "hi"', 'plain'],
      ['', '', ''],
    ]);
    expect(EXPECTED_HEADERS).toHaveLength(12);
    expect(() => parser('"unterminated,field')).toThrow(/unterminated/);
    expect(() => parser('"bad"tail,ok')).toThrow(/unexpected character/);
  });

  it('exits 1 naming row and field when introduced_date, detected_date, or fixed_date is tampered', () => {
    withMutatedCsv(
      (rows) => {
        rows[1][6] = '2026-01-01T00:00:00+07:00';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-01] introduced_date:');
      },
    );

    withMutatedCsv(
      (rows) => {
        rows[1][8] = '2026-12-31T00:00:00+07:00';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-01] fixed_date:');
      },
    );

    withMutatedCsv(
      (rows) => {
        rows[1][7] = '2026-02-30';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-01] detected_date:');
      },
    );

    withMutatedCsv(
      (rows) => {
        rows[2][7] = '2026-07-02'; // precedes INC-02 introduced_date 2026-07-03
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-02] detected_date:');
      },
    );

    withMutatedCsv(
      (rows) => {
        rows[2][7] = '2026-07-28'; // follows INC-02 fixed_date 2026-07-27
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-02] detected_date:');
      },
    );
  });

  it('exits 1 naming row and days_to_fix when day count is tampered', () => {
    withMutatedCsv(
      (rows) => {
        rows[2][9] = '999';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-02] days_to_fix:');
      },
    );
  });

  it('exits 1 on invalid class or was_suite_green enum values', () => {
    withMutatedCsv(
      (rows) => {
        rows[1][2] = 'bogus-class';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-01] class:');
      },
    );

    withMutatedCsv(
      (rows) => {
        rows[1][10] = 'partial';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-01] was_suite_green:');
      },
    );
  });

  it('exits 1 on nonexistent commit hash, git-option-like hash, or both commits empty', () => {
    withMutatedCsv(
      (rows) => {
        rows[1][3] = '0000000deadbeef';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-01] introduced_commit:');
      },
    );

    withMutatedCsv(
      (rows) => {
        rows[1][4] = '--help';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-01] fixed_commit:');
      },
    );

    withMutatedCsv(
      (rows) => {
        rows[1][3] = '';
        rows[1][4] = '';
        rows[1][6] = '';
        rows[1][8] = '';
        rows[1][9] = '';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-01] introduced_commit,fixed_commit:');
      },
    );
  });

  it('rejects off-trunk commits that are not ancestors of HEAD and keeps pinned anchors stable on working-tree line shifts', () => {
    const dir = mkdtempSync(join(tmpdir(), 'anima-incident-dates-ancestry-'));
    try {
      const git = (args: string[], extraEnv: Record<string, string> = {}) =>
        execFileSync(
          'git',
          ['-c', 'user.name=Test', '-c', 'user.email=test@example.com', '-c', 'commit.gpgsign=false', ...args],
          { cwd: dir, env: { ...process.env, ...extraEnv }, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] },
        ).trim();

      git(['init']);
      writeFileSync(join(dir, 'doc.md'), 'header\n\nline three\n', 'utf8');
      git(['add', 'doc.md']);
      git(['commit', '-m', 'base'], {
        GIT_AUTHOR_DATE: '2026-10-01T10:00:00+07:00',
        GIT_COMMITTER_DATE: '2026-10-01T10:00:00+07:00',
      });
      const baseSha = git(['rev-parse', '--short=7', 'HEAD']);
      const baseIso = git(['show', '-s', '--format=%cI', baseSha]);

      // Create an unmerged side branch commit.
      git(['checkout', '-b', 'side-only']);
      writeFileSync(join(dir, 'side.txt'), 'side\n', 'utf8');
      git(['add', 'side.txt']);
      git(['commit', '-m', 'side commit'], {
        GIT_AUTHOR_DATE: '2026-10-02T10:00:00+07:00',
        GIT_COMMITTER_DATE: '2026-10-02T10:00:00+07:00',
      });
      const sideSha = git(['rev-parse', '--short=7', 'HEAD']);
      const sideIso = git(['show', '-s', '--format=%cI', sideSha]);
      git(['checkout', '-']);

      const okCsv = serializeCsv([
        EXPECTED_HEADERS,
        ['INC-1', 'Pinned check', 'other', baseSha, '', 'audit', baseIso, '2026-10-01', '', '', 'yes', `doc.md@${baseSha}:3`],
      ]);
      writeFileSync(join(dir, 'incidents.csv'), okCsv, 'utf8');

      // Working-tree prepend of 5 lines must not disturb pinned @sha:3.
      writeFileSync(join(dir, 'doc.md'), '\n\n\n\n\nheader\n\nline three\n', 'utf8');
      const shiftedOk = (checkIncidentsCsv as (o: { csvPath: string; repoRoot: string }) => CheckResult)({
        csvPath: 'incidents.csv',
        repoRoot: dir,
      });
      expect(shiftedOk.ok).toBe(true);

      // Off-trunk commit must be rejected as not an ancestor of HEAD.
      const offTrunkCsv = serializeCsv([
        EXPECTED_HEADERS,
        ['INC-1', 'Off-trunk check', 'other', sideSha, '', 'audit', sideIso, '2026-10-02', '', '', 'yes', `doc.md@${baseSha}:3`],
      ]);
      writeFileSync(join(dir, 'incidents.csv'), offTrunkCsv, 'utf8');
      const offTrunkRes = (checkIncidentsCsv as (o: { csvPath: string; repoRoot: string }) => CheckResult)({
        csvPath: 'incidents.csv',
        repoRoot: dir,
      });
      expect(offTrunkRes.ok).toBe(false);
      expect(offTrunkRes.errors.some((e) => e.field === 'introduced_commit' && e.message.includes('ancestor'))).toBe(true);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it('exits 1 on unpinned anchor, missing source file, out-of-bounds line, path traversal, or bad header', () => {
    withMutatedCsv(
      (rows) => {
        rows[1][11] = 'FINDINGS.md:33';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-01] source:');
      },
    );

    withMutatedCsv(
      (rows) => {
        rows[1][11] = 'does/not/exist.md@6495c5c:1';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-01] source:');
      },
    );

    withMutatedCsv(
      (rows) => {
        rows[1][11] = 'FINDINGS.md@6495c5c:999999';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-01] source:');
      },
    );

    withMutatedCsv(
      (rows) => {
        rows[1][11] = '../FINDINGS.md@6495c5c:1';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-01] source:');
      },
    );

    withMutatedCsv(
      (rows) => {
        rows[0][0] = 'incident_id';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[<header>] header:');
      },
    );
  });

  it('executes main() when invoked via a symlink entrypoint (no vacuous exit 0)', () => {
    const dir = mkdtempSync(join(tmpdir(), 'anima-incident-dates-symlink-'));
    try {
      const symlinkScript = join(dir, 'incident_dates_link.mjs');
      symlinkSync(SCRIPT_PATH, symlinkScript);

      const bogusRun = spawnSync(process.execPath, [symlinkScript, '--bogus-flag'], {
        cwd: REPO_ROOT,
        encoding: 'utf8',
      });
      expect(bogusRun.status).toBe(2);
      expect(bogusRun.stderr).toContain('unknown argument "--bogus-flag"');

      withMutatedCsv(
        (rows) => {
          rows[1][6] = '2026-01-01T00:00:00+07:00';
        },
        (tmpCsv) => {
          const failRun = spawnSync(
            process.execPath,
            [symlinkScript, '--check', '--csv', tmpCsv, '--root', REPO_ROOT],
            {
              cwd: REPO_ROOT,
              encoding: 'utf8',
            },
          );
          expect(failRun.status).toBe(1);
          expect(failRun.stderr).toContain('[INC-01] introduced_date:');
        },
      );

      const okRun = spawnSync(process.execPath, [symlinkScript, '--check'], {
        cwd: REPO_ROOT,
        encoding: 'utf8',
      });
      expect(okRun.status).toBe(0);
      expect(okRun.stdout).toMatch(/^incident_dates: OK \(47 rows verified\)\r?\n$/);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it('rejects physicalLines + 1 on LF and CRLF committed files at pinned anchors', () => {
    const dir = mkdtempSync(join(tmpdir(), 'anima-incident-dates-eol-'));
    try {
      const git = (args: string[]) =>
        execFileSync(
          'git',
          ['-c', 'user.name=Test', '-c', 'user.email=test@example.com', '-c', 'commit.gpgsign=false', ...args],
          { cwd: dir, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] },
        ).trim();

      git(['init']);
      writeFileSync(join(dir, 'lf.md'), 'alpha\nbeta\n', 'utf8');
      writeFileSync(join(dir, 'crlf.md'), 'alpha\r\nbeta\r\n', 'utf8');
      git(['add', 'lf.md', 'crlf.md']);
      git(['commit', '-m', 'eol fixtures']);
      const sha = git(['rev-parse', '--short=7', 'HEAD']);

      const anchorCheck = validateSourceAnchor as (
        sourceText: string,
        repoRoot: string,
      ) => { ok: boolean; error?: string };

      expect(anchorCheck(`lf.md@${sha}:2`, dir).ok).toBe(true);
      const lfOffByOne = anchorCheck(`lf.md@${sha}:3`, dir);
      expect(lfOffByOne.ok).toBe(false);
      expect(lfOffByOne.error).toContain('out of range (1..2)');

      expect(anchorCheck(`crlf.md@${sha}:2`, dir).ok).toBe(true);
      const crlfOffByOne = anchorCheck(`crlf.md@${sha}:3`, dir);
      expect(crlfOffByOne.ok).toBe(false);
      expect(crlfOffByOne.error).toContain('out of range (1..2)');
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
