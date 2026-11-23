import { describe, expect, it } from 'vitest';
import { execFileSync, spawnSync } from 'node:child_process';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
// @ts-expect-error -- check_conflict_markers.mjs has no .d.mts declaration
import { isBinaryBuffer, isCliEntrypoint, runSelfTests, scanBuffer, scanDirectory, scanRepo, scanText } from '../../scripts/check_conflict_markers.mjs';

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

function runCli(args: string[], cwd = REPO_ROOT, scriptPath = SCRIPT_PATH): CliOutput {
  const res = spawnSync(process.execPath, [scriptPath, ...args], {
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
    expect(unit.passed).toBe(unit.total);
    expect(unit.passed).toBeGreaterThanOrEqual(11);

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

  it('executes CLI normally when invoked through a symlinked script path and validates isCliEntrypoint', () => {
    expect(isCliEntrypoint(undefined)).toBe(false);
    expect(isCliEntrypoint('/nonexistent/other.mjs', pathToFileURL(SCRIPT_PATH).href)).toBe(false);
    expect(isCliEntrypoint(SCRIPT_PATH, 'not-a-valid-url')).toBe(false);
    expect(isCliEntrypoint(SCRIPT_PATH, pathToFileURL(SCRIPT_PATH).href)).toBe(true);

    withTempDir((dir) => {
      const fixtureDir = join(dir, 'fixture');
      writeFixture(
        fixtureDir,
        'src/conflict.ts',
        `${LT7} HEAD\nconst x = 1;\n${EQ7}\nconst x = 2;\n${GT7} feat/branch\n`,
      );

      let symlinkScript: string;
      let linkToCleanup: string;
      if (process.platform === 'win32') {
        // Directory junctions work on Windows without Developer Mode or elevation.
        const scriptsLink = join(dir, 'scripts-link');
        symlinkSync(dirname(SCRIPT_PATH), scriptsLink, 'junction');
        linkToCleanup = scriptsLink;
        symlinkScript = join(scriptsLink, 'check_conflict_markers.mjs');
      } else {
        const fileLink = join(dir, 'check-conflict-markers-link.mjs');
        symlinkSync(SCRIPT_PATH, fileLink);
        linkToCleanup = fileLink;
        symlinkScript = fileLink;
      }

      try {
        expect(isCliEntrypoint(symlinkScript, pathToFileURL(SCRIPT_PATH).href)).toBe(true);

        const conflictRun = runCli(['--dir', fixtureDir], REPO_ROOT, symlinkScript);
        expect(conflictRun.status).toBe(1);
        expect(conflictRun.stderr).toContain('src/conflict.ts:1:');
        expect(conflictRun.stderr).toContain('src/conflict.ts:3:');
        expect(conflictRun.stderr).toContain('src/conflict.ts:5:');

        const selfTestRun = runCli(['--self-test'], REPO_ROOT, symlinkScript);
        expect(selfTestRun.status).toBe(0);
        expect(selfTestRun.stdout.trim().length).toBeGreaterThan(0);
        expect(selfTestRun.stdout).toContain('self-test:');
        expect(selfTestRun.stderr).toBe('');

        const bogusRun = runCli(['--bogus'], REPO_ROOT, symlinkScript);
        expect(bogusRun.status).toBe(2);
        expect(bogusRun.stderr).toContain('Error:');

        if (SCRIPT_PATH.startsWith('/private/tmp/')) {
          const tmpSymlinkScript = SCRIPT_PATH.replace(/^\/private/, '');
          if (existsSync(tmpSymlinkScript)) {
            const tmpConflictRun = runCli(['--dir', fixtureDir], REPO_ROOT, tmpSymlinkScript);
            expect(tmpConflictRun.status).toBe(1);
            expect(tmpConflictRun.stderr).toContain('src/conflict.ts:1:');
          }
        }
      } finally {
        rmSync(linkToCleanup, { recursive: true, force: true });
      }
    });
  });

  it('scans directory entries in deterministic UTF-16 code-unit order', () => {
    withTempDir((dir) => {
      // Distinct base names avoid macOS case-insensitive APFS collisions while testing
      // code-unit ordering: '-' (45) < 'Z' (90) < '_' (95) < 'a' (97).
      for (const name of ['a.ts', 'Z.ts', '_b.ts', '-c.ts']) {
        writeFixture(dir, name, `${LT7} HEAD\n`);
      }

      const unit = scanDirectory(dir) as ScanResult;
      expect(unit.scannedFiles).toBe(4);
      expect(unit.violations.map((v) => v.file)).toEqual(['-c.ts', 'Z.ts', '_b.ts', 'a.ts']);
    });
  });

  it('catches line-1 conflict markers after a UTF-8 BOM in .ts and .md without false-positives on BOM Setext headings', () => {
    const UTF8_BOM = Buffer.from([0xef, 0xbb, 0xbf]);

    withTempDir((dir) => {
      writeFixture(
        dir,
        'src/bom-stray.ts',
        Buffer.concat([UTF8_BOM, Buffer.from(`${LT7} HEAD\nexport const x = 1;\n`, 'utf8')]),
      );
      writeFixture(
        dir,
        'docs/bom-stray.md',
        Buffer.concat([UTF8_BOM, Buffer.from(`${LT7} HEAD\nParagraph.\n`, 'utf8')]),
      );
      writeFixture(
        dir,
        'src/bom-hunk.ts',
        Buffer.concat([
          UTF8_BOM,
          Buffer.from(`${LT7} HEAD\nconst a = 1;\n${EQ7}\nconst a = 2;\n${GT7} branch\n`, 'utf8'),
        ]),
      );
      writeFixture(
        dir,
        'docs/bom-hunk.md',
        Buffer.concat([
          UTF8_BOM,
          Buffer.from(`${LT7} HEAD\nours\n${EQ7}\ntheirs\n${GT7} branch\n`, 'utf8'),
        ]),
      );

      const cli = runCli(['--dir', dir]);
      expect(cli.status).toBe(1);
      expect(cli.stderr).toContain('src/bom-stray.ts:1:');
      expect(cli.stderr).toContain('docs/bom-stray.md:1:');
      expect(cli.stderr).toContain('src/bom-hunk.ts:1:');
      expect(cli.stderr).toContain('src/bom-hunk.ts:3:');
      expect(cli.stderr).toContain('src/bom-hunk.ts:5:');
      expect(cli.stderr).toContain('docs/bom-hunk.md:1:');
      expect(cli.stderr).toContain('docs/bom-hunk.md:3:');
      expect(cli.stderr).toContain('docs/bom-hunk.md:5:');
      expect(cli.stderr).toContain('8 marker(s) found across 4 file(s)');
    });

    withTempDir((dir) => {
      writeFixture(
        dir,
        'src/bom-clean.ts',
        Buffer.concat([UTF8_BOM, Buffer.from('export const clean = true;\n', 'utf8')]),
      );
      writeFixture(
        dir,
        'docs/bom-setext.md',
        Buffer.concat([UTF8_BOM, Buffer.from(`BOM Heading\n${EQ7}\n\nBody text.\n`, 'utf8')]),
      );

      const cli = runCli(['--dir', dir]);
      expect(cli.status).toBe(0);
      expect(cli.stderr).toBe('');
      expect(cli.stdout).toContain('2 file(s) scanned, 0 markers found');
    });
  });

  it('catches conflict markers and reports accurate 1-based line numbers for CR-only (\\r) line endings', () => {
    withTempDir((dir) => {
      writeFixture(
        dir,
        'src/classic-mac.ts',
        `first\r${LT7} HEAD\rours\r${EQ7}\rtheirs\r${GT7} branch\r`,
      );
      writeFixture(
        dir,
        'docs/classic-mac.md',
        `intro\r${LT7} HEAD\rleft\r${EQ7}\rright\r${GT7} main\r`,
      );

      const cli = runCli(['--dir', dir]);
      expect(cli.status).toBe(1);
      expect(cli.stderr).toContain('src/classic-mac.ts:2:');
      expect(cli.stderr).toContain('src/classic-mac.ts:4:');
      expect(cli.stderr).toContain('src/classic-mac.ts:6:');
      expect(cli.stderr).toContain('docs/classic-mac.md:2:');
      expect(cli.stderr).toContain('docs/classic-mac.md:4:');
      expect(cli.stderr).toContain('docs/classic-mac.md:6:');
      expect(cli.stderr).toContain('6 marker(s) found across 2 file(s)');
    });

    expect(scanText(`Title\r${EQ7}\r\rBody\r`, 'docs/cr-clean.md')).toEqual([]);
  });

  it('catches ======= and >>>>>>> in a Markdown conflict hunk containing an unclosed fenced block', () => {
    const mdUnclosedFence = [
      'Intro paragraph.',
      `${LT7} HEAD`,
      '```ts',
      'const ours = 1;',
      EQ7,
      'const theirs = 2;',
      `${GT7} feat/branch`,
      '',
      '```diff',
      `${LT7} HEAD`,
      EQ7,
      `${GT7} example-only`,
      '```',
    ].join('\n');

    const violations = scanText(mdUnclosedFence, 'docs/unclosed-fence.md') as Violation[];
    expect(violations.map((v) => ({ line: v.line, kind: v.kind }))).toEqual([
      { line: 2, kind: 'conflict-start' },
      { line: 5, kind: 'conflict-divider' },
      { line: 7, kind: 'conflict-end' },
    ]);

    withTempDir((dir) => {
      const mdDiff3Unclosed = [
        `${LT7} ours`,
        '~~~rust',
        'fn ours() {}',
        `${BAR7} base`,
        'fn base() {}',
        EQ7,
        'fn theirs() {}',
        '~~~',
        `${GT7} theirs`,
      ].join('\n');
      writeFixture(dir, 'docs/diff3-unclosed.md', mdDiff3Unclosed);

      const cli = runCli(['--dir', dir]);
      expect(cli.status).toBe(1);
      expect(cli.stderr).toContain('docs/diff3-unclosed.md:1:');
      expect(cli.stderr).toContain('docs/diff3-unclosed.md:4:');
      expect(cli.stderr).toContain('docs/diff3-unclosed.md:6:');
      expect(cli.stderr).toContain('docs/diff3-unclosed.md:9:');
      expect(cli.stderr).toContain('4 marker(s) found across 1 file(s)');
    });
  });

  it('deduplicates unmerged Git index entries (stages 1-3) in scanRepo and default CLI mode', () => {
    withTempDir((dir) => {
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

      execFileSync('git', ['init'], { cwd: dir, env: gitEnv });
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
        // `git merge` needs a committer identity even when it stops on a conflict. Without the
        // -c identity a runner with no global user.email (GitHub's ubuntu image) exits 128.
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

      const unit = scanRepo(dir) as ScanResult;
      expect(unit.scannedFiles).toBe(1);
      expect(unit.violations.map((v) => v.kind)).toEqual([
        'conflict-start',
        'conflict-divider',
        'conflict-end',
      ]);
      expect(unit.violations.every((v) => v.file === 'src/merged.ts')).toBe(true);

      const cli = runCli([], dir);
      expect(cli.status).toBe(1);
      expect(cli.stderr).toContain('3 marker(s) found across 1 file(s)');
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
