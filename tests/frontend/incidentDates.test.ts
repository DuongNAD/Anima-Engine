import { describe, expect, it } from 'vitest';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
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
  it('verifies the committed docs/paper/data/incidents.csv cleanly via CLI --check', () => {
    const run = runCli(['--check']);
    expect(run.status).toBe(0);
    expect(run.stderr).toBe('');
    expect(run.stdout).toMatch(/^incident_dates: OK \(\d+ rows verified\)\r?\n$/);

    const apiResult = (
      checkIncidentsCsv as (opts: { csvPath: string; repoRoot: string }) => CheckResult
    )({
      csvPath: 'docs/paper/data/incidents.csv',
      repoRoot: REPO_ROOT,
    });
    expect(apiResult.ok).toBe(true);
    expect(apiResult.errors).toEqual([]);
    expect(apiResult.rowCount).toBeGreaterThanOrEqual(46);
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
    expect(EXPECTED_HEADERS).toHaveLength(11);
    expect(() => parser('"unterminated,field')).toThrow(/unterminated/);
    expect(() => parser('"bad"tail,ok')).toThrow(/unexpected character/);
  });

  it('exits 1 naming row and field when introduced_date or detected_date is tampered', () => {
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
        rows[1][7] = '2026-12-31T00:00:00+07:00';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-01] detected_date:');
      },
    );
  });

  it('exits 1 naming row and days_to_detection when day count is tampered', () => {
    withMutatedCsv(
      (rows) => {
        rows[2][8] = '999';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-02] days_to_detection:');
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
        rows[1][9] = 'partial';
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
        expect(run.stderr).toContain('[INC-01] detected_commit:');
      },
    );

    withMutatedCsv(
      (rows) => {
        rows[1][3] = '';
        rows[1][4] = '';
        rows[1][6] = '';
        rows[1][7] = '';
        rows[1][8] = '';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-01] introduced_commit,detected_commit:');
      },
    );
  });

  it('exits 1 on missing source file, out-of-bounds source line, path traversal, or bad header', () => {
    withMutatedCsv(
      (rows) => {
        rows[1][10] = 'does/not/exist.md:1';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-01] source:');
      },
    );

    withMutatedCsv(
      (rows) => {
        rows[1][10] = 'FINDINGS.md:999999';
      },
      (tmpCsv) => {
        const run = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(run.status).toBe(1);
        expect(run.stderr).toContain('[INC-01] source:');
      },
    );

    withMutatedCsv(
      (rows) => {
        rows[1][10] = '../FINDINGS.md:1';
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
      expect(okRun.stdout).toMatch(/^incident_dates: OK \(\d+ rows verified\)\r?\n$/);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it('rejects physicalLines + 1 on LF and CRLF trailing-newline files', () => {
    const findingsRaw = readFileSync(resolve(REPO_ROOT, 'FINDINGS.md'), 'utf8');
    const findingsPhysicalLines = findingsRaw.replace(/\r?\n$/, '').split(/\r?\n/).length;

    withMutatedCsv(
      (rows) => {
        rows[1][10] = `FINDINGS.md:${findingsPhysicalLines}`;
      },
      (tmpCsv) => {
        const okRun = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(okRun.status).toBe(0);
      },
    );

    withMutatedCsv(
      (rows) => {
        rows[1][10] = `FINDINGS.md:${findingsPhysicalLines + 1}`;
      },
      (tmpCsv) => {
        const failRun = runCli(['--check', '--csv', tmpCsv, '--root', REPO_ROOT]);
        expect(failRun.status).toBe(1);
        expect(failRun.stderr).toContain('[INC-01] source:');
      },
    );

    const anchorCheck = validateSourceAnchor as (
      sourceText: string,
      repoRoot: string,
    ) => { ok: boolean; error?: string };
    const dir = mkdtempSync(join(tmpdir(), 'anima-incident-dates-eol-'));
    try {
      writeFileSync(join(dir, 'lf.md'), 'alpha\nbeta\n', 'utf8');
      writeFileSync(join(dir, 'crlf.md'), 'alpha\r\nbeta\r\n', 'utf8');

      expect(anchorCheck('lf.md:2', dir).ok).toBe(true);
      const lfOffByOne = anchorCheck('lf.md:3', dir);
      expect(lfOffByOne.ok).toBe(false);
      expect(lfOffByOne.error).toContain('out of range (1..2)');

      expect(anchorCheck('crlf.md:2', dir).ok).toBe(true);
      const crlfOffByOne = anchorCheck('crlf.md:3', dir);
      expect(crlfOffByOne.ok).toBe(false);
      expect(crlfOffByOne.error).toContain('out of range (1..2)');
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
