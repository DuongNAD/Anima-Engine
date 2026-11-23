import { describe, expect, it } from 'vitest';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
// @ts-expect-error -- check_conflict_markers.mjs has no .d.mts declaration
import { MIN_SCANNED_FILES, isBinaryBuffer, runNegativeControl, scanRepo, scanText } from '../../scripts/check_conflict_markers.mjs';

interface Violation {
  file: string;
  line: number;
  kind: string;
  message: string;
}

interface ScanResult {
  root: string;
  scannedFiles: number;
  violations: Violation[];
  minFiles: number;
  underFloor: boolean;
}

interface CliOutput {
  status: number | null;
  stdout: string;
  stderr: string;
}

const TEST_DIR = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(TEST_DIR, '../..');
const SCRIPT_PATH = resolve(REPO_ROOT, 'scripts/check_conflict_markers.mjs');

const LT7 = '<'.repeat(7);
const EQ7 = '='.repeat(7);
const GT7 = '>'.repeat(7);
const BAR7 = '|'.repeat(7);

function runCli(args: string[], cwd = REPO_ROOT): CliOutput {
  const res = spawnSync(process.execPath, [SCRIPT_PATH, ...args], {
    cwd,
    encoding: 'utf8',
  });
  expect(res.error).toBeUndefined();
  return {
    status: res.status,
    stdout: res.stdout ?? '',
    stderr: res.stderr ?? '',
  };
}

function withTempGitRepo(fn: (dir: string) => void): void {
  const dir = mkdtempSync(join(tmpdir(), 'anima-conflict-markers-'));
  try {
    execFileSync('git', ['init'], { cwd: dir, stdio: 'ignore' });
    fn(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

function writeFixture(rootDir: string, relPath: string, content: string | Buffer): void {
  const fullPath = join(rootDir, relPath);
  mkdirSync(dirname(fullPath), { recursive: true });
  writeFileSync(fullPath, content);
}

describe('check_conflict_markers gate', () => {
  it('passes built-in --self-test negative control and rejects unexpected CLI flags', () => {
    const unit = runNegativeControl() as { ok: boolean; error: string };
    expect(unit).toEqual({ ok: true, error: '' });

    const selfTest = runCli(['--self-test']);
    expect(selfTest.status).toBe(0);
    expect(selfTest.stdout.trim()).toBe('conflict markers self-test: ok');
    expect(selfTest.stderr).toBe('');

    expect(runCli(['--dir', REPO_ROOT]).status).toBe(2);
    expect(runCli(['--unknown-flag']).status).toBe(2);
  });

  it('passes on the clean repository above the minimum file floor', () => {
    const unit = scanRepo(REPO_ROOT) as ScanResult;
    expect(unit.scannedFiles).toBeGreaterThanOrEqual(MIN_SCANNED_FILES);
    expect(unit.underFloor).toBe(false);
    expect(unit.violations).toEqual([]);

    const cli = runCli([]);
    expect(cli.status).toBe(0);
    expect(cli.stdout).toMatch(/conflict markers: \d+ file\(s\) scanned, 0 markers found/);
    expect(cli.stderr).toBe('');
  });

  it('catches real conflicts inside Markdown code fences and after unclosed fences', () => {
    const fencedMd = [
      '# Docs',
      '```diff',
      `${LT7} HEAD`,
      'ours',
      `${BAR7} base`,
      EQ7,
      'theirs',
      `${GT7} main`,
      '```',
    ].join('\n');

    const unit = scanText(fencedMd, 'docs/README.md') as Violation[];
    expect(unit.map((v) => ({ line: v.line, kind: v.kind }))).toEqual([
      { line: 3, kind: 'conflict-start' },
      { line: 5, kind: 'conflict-base' },
      { line: 8, kind: 'conflict-end' },
    ]);

    withTempGitRepo((dir) => {
      writeFixture(dir, 'docs/README.md', fencedMd);
      const cli = runCli([], dir);
      expect(cli.status).toBe(1);
      expect(cli.stderr).toContain('docs/README.md:3:');
      expect(cli.stderr).toContain('docs/README.md:5:');
      expect(cli.stderr).toContain('docs/README.md:8:');
    });
  });

  it('catches stray start, end, and base markers across code and doc extensions', () => {
    for (const rel of ['src/lib.rs', 'src/app.ts', 'scripts/tool.mjs', 'config/settings.json', 'docs/guide.md']) {
      withTempGitRepo((dir) => {
        writeFixture(dir, rel, `a\n${LT7} HEAD\n${BAR7} base\n${GT7} branch\n`);
        const cli = runCli([], dir);
        expect(cli.status).toBe(1);
        expect(cli.stderr).toContain(`${rel}:2:`);
        expect(cli.stderr).toContain(`${rel}:3:`);
        expect(cli.stderr).toContain(`${rel}:4:`);
      });
    }
  });

  it('handles UTF-8 BOM, CRLF, and CR-only line endings with exact 1-based line numbers', () => {
    const UTF8_BOM = Buffer.from([0xef, 0xbb, 0xbf]);
    withTempGitRepo((dir) => {
      writeFixture(
        dir,
        'src/bom.ts',
        Buffer.concat([UTF8_BOM, Buffer.from(`${LT7} HEAD\nclean\n${GT7} main\n`, 'utf8')]),
      );
      writeFixture(dir, 'src/crlf.ts', `a\r\n${LT7} HEAD\r\nb\r\n${GT7} main\r\n`);
      writeFixture(dir, 'src/cr.ts', `a\r${LT7} HEAD\rb\r${GT7} main\r`);

      const cli = runCli([], dir);
      expect(cli.status).toBe(1);
      expect(cli.stderr).toContain('src/bom.ts:1:');
      expect(cli.stderr).toContain('src/bom.ts:3:');
      expect(cli.stderr).toContain('src/crlf.ts:2:');
      expect(cli.stderr).toContain('src/crlf.ts:4:');
      expect(cli.stderr).toContain('src/cr.ts:2:');
      expect(cli.stderr).toContain('src/cr.ts:4:');
    });
  });

  it('skips binary files and does not false-positive on Setext headings, dividers, or indented markers', () => {
    expect(isBinaryBuffer(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x00, 0x0a]))).toBe(true);
    expect(isBinaryBuffer(Buffer.from('plain text', 'utf8'))).toBe(false);

    withTempGitRepo((dir) => {
      writeFixture(dir, 'docs/setext.md', `Heading\n${EQ7}\n\n${'='.repeat(40)}\n`);
      writeFixture(
        dir,
        'src/comments.ts',
        `// ${'='.repeat(30)}\nconst s = 'example ${LT7} HEAD ${GT7}';\n  ${LT7} indented\n${EQ7}\n`,
      );
      writeFixture(
        dir,
        'assets/blob.bin',
        Buffer.from(`\0${LT7} HEAD\n${EQ7}\n${GT7} main\n`, 'utf8'),
      );
      writeFixture(dir, 'empty.txt', '');
      for (let i = 0; i < MIN_SCANNED_FILES; i += 1) {
        writeFixture(dir, `pad/f${i}.txt`, 'clean\n');
      }

      const cli = runCli([], dir);
      expect(cli.status).toBe(0);
      expect(cli.stderr).toBe('');
      expect(cli.stdout).toContain('0 markers found');
    });
  });

  it('fails if scannedFiles is below MIN_SCANNED_FILES', () => {
    withTempGitRepo((dir) => {
      writeFixture(dir, 'clean.txt', 'hello\n');
      const unit = scanRepo(dir) as ScanResult;
      expect(unit.scannedFiles).toBe(1);
      expect(unit.underFloor).toBe(true);
      expect(unit.violations).toEqual([]);

      const cli = runCli([], dir);
      expect(cli.status).toBe(1);
      expect(cli.stderr).toContain(`scanned only 1 file(s) (minimum ${MIN_SCANNED_FILES})`);
    });
  });

  it('deduplicates unmerged Git index entries (stages 1-3) in scanRepo and CLI', () => {
    withTempGitRepo((dir) => {
      const gitEnv = {
        ...process.env,
        GIT_TERMINAL_PROMPT: '0',
      };
      const gitCommitArgs = [
        '-c',
        'user.name=Test',
        '-c',
        'user.email=test@example.com',
        '-c',
        'commit.gpgsign=false',
      ];

      writeFixture(dir, 'src/merged.ts', 'export const v = 0;\n');
      execFileSync('git', ['add', 'src/merged.ts'], { cwd: dir, env: gitEnv });
      execFileSync('git', [...gitCommitArgs, 'commit', '-m', 'base'], {
        cwd: dir,
        env: gitEnv,
      });

      execFileSync('git', ['checkout', '-b', 'branch-ours'], { cwd: dir, env: gitEnv });
      writeFixture(dir, 'src/merged.ts', 'export const v = 1;\n');
      execFileSync('git', [...gitCommitArgs, 'commit', '-am', 'ours'], {
        cwd: dir,
        env: gitEnv,
      });

      execFileSync('git', ['checkout', '-b', 'branch-theirs', 'HEAD~1'], {
        cwd: dir,
        env: gitEnv,
      });
      writeFixture(dir, 'src/merged.ts', 'export const v = 2;\n');
      execFileSync('git', [...gitCommitArgs, 'commit', '-am', 'theirs'], {
        cwd: dir,
        env: gitEnv,
      });

      const mergeRes = spawnSync(
        'git',
        [...gitCommitArgs, '-c', 'merge.conflictstyle=merge', 'merge', 'branch-ours'],
        { cwd: dir, env: gitEnv, encoding: 'utf8' },
      );
      expect(mergeRes.status).toBe(1);

      const rawLsFiles = execFileSync(
        'git',
        ['ls-files', '-z', '--cached', '--others', '--exclude-standard'],
        { cwd: dir, env: gitEnv, encoding: 'utf8' },
      )
        .split('\0')
        .filter(Boolean);
      expect(rawLsFiles.length).toBeGreaterThan(1);

      const unit = scanRepo(dir, { minFiles: 1 }) as ScanResult;
      expect(unit.scannedFiles).toBe(1);
      expect(unit.violations.map((v) => v.kind)).toEqual([
        'conflict-start',
        'conflict-end',
      ]);
      expect(unit.violations.every((v) => v.file === 'src/merged.ts')).toBe(true);

      const cli = runCli([], dir);
      expect(cli.status).toBe(1);
      expect(cli.stderr).toContain('2 marker(s) found across 1 file(s)');
    });
  });

  it('is wired into package.json and .github/workflows/ci.yml frontend job', () => {
    const pkg = JSON.parse(readFileSync(resolve(REPO_ROOT, 'package.json'), 'utf8')) as {
      scripts?: Record<string, string>;
    };
    expect(pkg.scripts?.['check:conflict-markers']).toBe(
      'node scripts/check_conflict_markers.mjs',
    );

    const ciYml = readFileSync(resolve(REPO_ROOT, '.github/workflows/ci.yml'), 'utf8');
    expect(ciYml).toContain('run: npm run check:conflict-markers');
    expect(ciYml.indexOf('npm run check:root-hygiene')).toBeLessThan(
      ciYml.indexOf('npm run check:conflict-markers'),
    );
  });
});
