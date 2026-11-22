import { describe, expect, it } from 'vitest';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
// @ts-expect-error -- check_conflict_markers.mjs has no .d.mts declaration
import { isBinaryBuffer, runSelfTests, scanBuffer, scanDirectory, scanRepo, scanText } from '../../scripts/check_conflict_markers.mjs';
import { readFileSync } from 'node:fs';

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
}

interface SelfTestResult {
  ok: boolean;
  total: number;
  passed: number;
  failures: string[];
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

const CODE_EXTENSIONS = [
  { ext: 'rs', rel: 'src/lib.rs' },
  { ext: 'ts', rel: 'src/app.ts' },
  { ext: 'mjs', rel: 'scripts/tool.mjs' },
  { ext: 'json', rel: 'config/settings.json' },
  { ext: 'md', rel: 'docs/guide.md' },
] as const;

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

function withTempDir(fn: (dir: string) => void): void {
  const dir = mkdtempSync(join(tmpdir(), 'anima-conflict-markers-'));
  try {
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
  it('passes built-in --self-test CLI and unit helper', () => {
    const unit = runSelfTests() as SelfTestResult;
    expect(unit.ok).toBe(true);
    expect(unit.failures).toEqual([]);
    expect(unit.passed).toBeGreaterThanOrEqual(6);

    const cli = runCli(['--self-test']);
    expect(cli.status).toBe(0);
    expect(cli.stdout).toContain('self-test:');
    expect(cli.stderr).toBe('');
  });

  it('passes on the clean repository in default mode', () => {
    const unit = scanRepo(REPO_ROOT) as ScanResult;
    expect(unit.scannedFiles).toBeGreaterThan(100);
    expect(unit.violations).toEqual([]);

    const cli = runCli([]);
    expect(cli.status).toBe(0);
    expect(cli.stdout).toMatch(/conflict markers: \d+ file\(s\) scanned, 0 markers found/);
    expect(cli.stderr).toBe('');
  });

  it('passes on a clean --dir fixture via both --dir <path> and --dir=<path>', () => {
    withTempDir((dir) => {
      writeFixture(dir, 'src/main.rs', 'fn main() {\n    println!("ok");\n}\n');
      writeFixture(dir, 'src/index.ts', 'export const answer = 42;\n');
      writeFixture(dir, 'docs/readme.md', 'Project Title\n' + EQ7 + '\n\nClean text.\n');

      const unit = scanDirectory(dir) as ScanResult;
      expect(unit.scannedFiles).toBe(3);
      expect(unit.violations).toEqual([]);

      const cliSpace = runCli(['--dir', dir]);
      expect(cliSpace.status).toBe(0);
      expect(cliSpace.stdout).toContain('3 file(s) scanned, 0 markers found');

      const cliEq = runCli([`--dir=${dir}`]);
      expect(cliEq.status).toBe(0);
      expect(cliEq.stdout).toContain('3 file(s) scanned, 0 markers found');
    });
  });

  it('exits 2 on invalid CLI arguments or missing --dir target', () => {
    expect(runCli(['--unknown-flag']).status).toBe(2);
    expect(runCli(['--dir']).status).toBe(2);
    expect(runCli(['--dir=']).status).toBe(2);
    expect(runCli(['--self-test', '--dir', REPO_ROOT]).status).toBe(2);

    withTempDir((dir) => {
      const missing = join(dir, 'does-not-exist');
      const resMissing = runCli(['--dir', missing]);
      expect(resMissing.status).toBe(2);
      expect(resMissing.stderr).toContain('Error:');

      const filePath = join(dir, 'plain.txt');
      writeFileSync(filePath, 'hello\n', 'utf8');
      const resFile = runCli(['--dir', filePath]);
      expect(resFile.status).toBe(2);
      expect(resFile.stderr).toContain('Not a directory');
    });
  });

  it('catches full 2-way conflict hunks in .rs, .ts, .mjs, .json, and .md with exact file:line', () => {
    for (const target of CODE_EXTENSIONS) {
      withTempDir((dir) => {
        const body = [
          'line 1',
          'line 2',
          `${LT7} HEAD`,
          'ours',
          EQ7,
          'theirs',
          `${GT7} feat/branch`,
          'line 8',
        ].join('\n');
        writeFixture(dir, target.rel, body);

        const cli = runCli(['--dir', dir]);
        expect(cli.status).toBe(1);
        expect(cli.stderr).toContain(`${target.rel}:3:`);
        expect(cli.stderr).toContain(`${target.rel}:5:`);
        expect(cli.stderr).toContain(`${target.rel}:7:`);
      });
    }
  });

  it('catches diff3 conflict hunks (|||||||) with exact file:line', () => {
    for (const target of CODE_EXTENSIONS) {
      withTempDir((dir) => {
        const body = [
          'header',
          `${LT7} ours`,
          'alpha',
          `${BAR7} merged common ancestors`,
          'base',
          EQ7,
          'beta',
          `${GT7} theirs`,
        ].join('\n');
        writeFixture(dir, target.rel, body);

        const cli = runCli(['--dir', dir]);
        expect(cli.status).toBe(1);
        expect(cli.stderr).toContain(`${target.rel}:2:`);
        expect(cli.stderr).toContain(`${target.rel}:4:`);
        expect(cli.stderr).toContain(`${target.rel}:6:`);
        expect(cli.stderr).toContain(`${target.rel}:8:`);
      });
    }
  });

  it('catches stray start, end, base, and code divider markers across extensions', () => {
    for (const target of CODE_EXTENSIONS) {
      withTempDir((dir) => {
        writeFixture(dir, target.rel, `a\nb\nc\n${LT7} HEAD\nd\n`);
        const startRes = runCli(['--dir', dir]);
        expect(startRes.status).toBe(1);
        expect(startRes.stderr).toContain(`${target.rel}:4:`);
      });

      withTempDir((dir) => {
        writeFixture(dir, target.rel, `a\nb\nc\nd\n${GT7} origin/main\n`);
        const endRes = runCli(['--dir', dir]);
        expect(endRes.status).toBe(1);
        expect(endRes.stderr).toContain(`${target.rel}:5:`);
      });

      withTempDir((dir) => {
        writeFixture(dir, target.rel, `a\n${BAR7} base\n`);
        const baseRes = runCli(['--dir', dir]);
        expect(baseRes.status).toBe(1);
        expect(baseRes.stderr).toContain(`${target.rel}:2:`);
      });

      withTempDir((dir) => {
        // In Markdown, a blank line before ======= prevents Setext H1 interpretation.
        writeFixture(dir, target.rel, `\n${EQ7}\n`);
        const divRes = runCli(['--dir', dir]);
        expect(divRes.status).toBe(1);
        expect(divRes.stderr).toContain(`${target.rel}:2:`);
      });
    }
  });

  it('reports accurate 1-based line numbers for CRLF files', () => {
    withTempDir((dir) => {
      const crlf = `first\r\n${LT7} HEAD\r\nours\r\n${EQ7}\r\ntheirs\r\n${GT7} branch\r\n`;
      writeFixture(dir, 'src/windows.ts', crlf);

      const cli = runCli(['--dir', dir]);
      expect(cli.status).toBe(1);
      expect(cli.stderr).toContain('src/windows.ts:2:');
      expect(cli.stderr).toContain('src/windows.ts:4:');
      expect(cli.stderr).toContain('src/windows.ts:6:');
    });
  });

  it('does not false-positive on Markdown Setext H1 headings, banner dividers, fenced blocks, binary files, empty files, or inline mentions', () => {
    withTempDir((dir) => {
      // Setext H1 headings across .md, .markdown, .mdx
      writeFixture(
        dir,
        'docs/setext.md',
        `Title\n${EQ7}\n\nSubtitle\n${EQ7}   \n\n${'='.repeat(21)}\n${'='.repeat(96)}\n`,
      );
      writeFixture(dir, 'docs/setext.mdx', `MDX Title\n${EQ7}\n`);
      writeFixture(dir, 'docs/setext.markdown', `Markdown Title\n${EQ7}\n`);

      // Fenced code blocks (backticks, tildes, and 4-backtick outer fence containing 3 backticks)
      writeFixture(
        dir,
        'docs/examples.md',
        [
          '```diff',
          `${LT7} HEAD`,
          'ours',
          `${BAR7} base`,
          EQ7,
          'theirs',
          `${GT7} main`,
          '```',
          '',
          '~~~text',
          `${LT7} ours`,
          EQ7,
          `${GT7} theirs`,
          '~~~',
          '',
          '````markdown',
          '```',
          `${LT7} HEAD`,
          EQ7,
          `${GT7} main`,
          '````',
        ].join('\n'),
      );

      // Binary files with NUL bytes (both .png and extensionless)
      const binaryWithMarkers = Buffer.concat([
        Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x00, 0x0a]),
        Buffer.from(`${LT7} HEAD\n${EQ7}\n${GT7} branch\n`, 'utf8'),
      ]);
      writeFixture(dir, 'assets/icon.png', binaryWithMarkers);
      writeFixture(dir, 'assets/blob', Buffer.from(`\0${LT7} HEAD\n${EQ7}\n${GT7} branch\n`, 'utf8'));

      // Empty file
      writeFixture(dir, 'empty.txt', '');

      // Non-column-0 mentions and decorative banners
      writeFixture(
        dir,
        'src/comments.ts',
        [
          `// ${'='.repeat(40)}`,
          `const doc = 'example ${LT7} HEAD ${EQ7} ${GT7} branch';`,
          `  ${LT7} indented`,
          `${'='.repeat(8)}`,
        ].join('\n'),
      );

      // Ignored directories (.git, node_modules)
      writeFixture(dir, '.git/MERGE_MSG', `${LT7} HEAD\n${EQ7}\n${GT7} branch\n`);
      writeFixture(dir, 'node_modules/pkg/index.js', `${LT7} HEAD\n${EQ7}\n${GT7} branch\n`);

      const cli = runCli(['--dir', dir]);
      expect(cli.status).toBe(0);
      expect(cli.stderr).toBe('');
      expect(cli.stdout).toContain('0 markers found');
    });
  });

  it('catches real conflict markers immediately after a closing Markdown fence and inside open conflicts', () => {
    const mdAfterFence = [
      '```ts',
      `${LT7} HEAD`,
      '```',
      EQ7,
      `${LT7} HEAD`,
      'Heading',
      EQ7,
      `${GT7} branch`,
    ].join('\n');

    const violations = scanText(mdAfterFence, 'docs/mixed.md') as Violation[];
    expect(violations.map((v) => v.line)).toEqual([4, 5, 7, 8]);

    expect(isBinaryBuffer(Buffer.from([0x00, 0x01]))).toBe(true);
    expect(isBinaryBuffer(Buffer.from('plain text', 'utf8'))).toBe(false);
    expect(scanBuffer(Buffer.alloc(0), 'empty.rs')).toEqual([]);
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
