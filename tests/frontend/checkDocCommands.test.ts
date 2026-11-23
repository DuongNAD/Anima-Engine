import { describe, expect, it } from 'vitest';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
// @ts-expect-error -- check_doc_commands.mjs has no .d.mts declaration
import { collectDocTestBlocks, collectMarkdownFiles, detectCargoZeroTests, extractDocCommands, parseArgs, runDocCommands, runStaticChecks } from '../../scripts/check_doc_commands.mjs';

interface ExtractedBlock {
  file: string;
  markerLine: number;
  fenceLine: number;
  lang: string;
  script: string;
}

interface ParseError {
  file: string;
  line: number;
  message: string;
}

interface ExtractResult {
  blocks: ExtractedBlock[];
  errors: ParseError[];
}

interface RunResult {
  ok: boolean;
  failedBlock?: ExtractedBlock;
  exitStatus?: number;
  message?: string;
  output?: string;
  executed: Array<{ file: string; fenceLine: number; status: number }>;
}

const REPO_ROOT = resolve(import.meta.dirname, '../..');
const SCRIPT_PATH = resolve(REPO_ROOT, 'scripts/check_doc_commands.mjs');

function makeTempDir(prefix = 'anima-doc8-test-'): { dir: string; cleanup: () => void } {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  return {
    dir,
    cleanup: () => rmSync(dir, { recursive: true, force: true }),
  };
}

describe('check_doc_commands — extraction, ordering, 0-block & 0-test failure, static pass', () => {
  it('extracts bash/sh/bare/tilde <!-- doc-test --> blocks and ignores unmarked or nested fences', () => {
    const md = [
      '# Sample',
      '',
      '```bash',
      'echo unmarked',
      '```',
      '',
      '````markdown',
      '<!-- doc-test -->',
      '```bash',
      'echo nested',
      '```',
      '````',
      '',
      '<!-- doc-test -->',
      '```bash',
      'echo first',
      '```',
      '',
      '<!-- doc-test -->',
      '',
      '```sh',
      'echo second',
      '```',
      '',
      '<!-- doc-test -->',
      '```',
      'echo third',
      '```',
      '',
      '<!-- doc-test -->',
      '~~~bash',
      'echo fourth',
      '~~~',
    ].join('\n');

    const res = extractDocCommands(md, 'docs/tutorials/sample.md') as ExtractResult;
    expect(res.errors).toEqual([]);
    expect(res.blocks).toEqual([
      { file: 'docs/tutorials/sample.md', markerLine: 14, fenceLine: 15, lang: 'bash', script: 'echo first' },
      { file: 'docs/tutorials/sample.md', markerLine: 19, fenceLine: 21, lang: 'sh', script: 'echo second' },
      { file: 'docs/tutorials/sample.md', markerLine: 25, fenceLine: 26, lang: '', script: 'echo third' },
      { file: 'docs/tutorials/sample.md', markerLine: 30, fenceLine: 31, lang: 'bash', script: 'echo fourth' },
    ]);
  });

  it('reports doc:line on orphan, non-fence, unsupported-language, empty, and unclosed blocks', () => {
    const md = [
      '<!-- doc-test -->',
      'plain prose',
      '',
      '<!-- doc-test -->',
      '```powershell',
      'Write-Host hi',
      '```',
      '',
      '<!-- doc-test -->',
      '```bash',
      '   ',
      '```',
      '',
      '<!-- doc-test -->',
      '```bash',
      'echo unclosed',
    ].join('\n');

    const res = extractDocCommands(md, 'docs/how-to/bad.md') as ExtractResult;
    expect(res.blocks).toEqual([]);
    expect(res.errors.map((e) => `${e.file}:${e.line}`)).toEqual([
      'docs/how-to/bad.md:1',
      'docs/how-to/bad.md:5',
      'docs/how-to/bad.md:10',
      'docs/how-to/bad.md:15',
    ]);
  });

  it('orders markdown files ASCII-sorted per directory and fails closed when 0 blocks exist', () => {
    const tmp = makeTempDir();
    try {
      mkdirSync(join(tmp.dir, 'tutorials'), { recursive: true });
      mkdirSync(join(tmp.dir, 'how-to'), { recursive: true });
      writeFileSync(join(tmp.dir, 'tutorials', '02.md'), 'No markers here\n', 'utf8');
      writeFileSync(join(tmp.dir, 'tutorials', '01.md'), '# Intro\n', 'utf8');
      writeFileSync(join(tmp.dir, 'how-to', 'B.md'), '# B\n', 'utf8');
      writeFileSync(join(tmp.dir, 'how-to', 'A.md'), '# A\n', 'utf8');

      const files = collectMarkdownFiles(['tutorials', 'how-to'], tmp.dir) as Array<{ relPath: string }>;
      expect(files.map((f) => f.relPath)).toEqual([
        'tutorials/01.md',
        'tutorials/02.md',
        'how-to/A.md',
        'how-to/B.md',
      ]);

      const cliRes = spawnSync(process.execPath, [SCRIPT_PATH, '--dir', 'tutorials', '--dir', 'how-to', '--cwd', tmp.dir], {
        encoding: 'utf8',
      });
      expect(cliRes.status).toBe(1);
      expect(cliRes.stderr).toContain('0 <!-- doc-test --> blocks found');

      const staticRes = spawnSync(
        process.execPath,
        [SCRIPT_PATH, '--dir', 'tutorials', '--dir', 'how-to', '--cwd', tmp.dir, '--static'],
        { encoding: 'utf8' },
      );
      expect(staticRes.status).toBe(1);
      expect(staticRes.stderr).toContain('0 <!-- doc-test --> blocks found');

      const scanZero = collectDocTestBlocks(['tutorials', 'how-to'], tmp.dir) as {
        blocks: ExtractedBlock[];
        errors: ParseError[];
      };
      expect(scanZero.blocks).toEqual([]);
      expect(scanZero.errors.map((e) => e.message)).toEqual([
        'check_doc_commands: 0 <!-- doc-test --> blocks found; failing closed',
      ]);

      // Missing default directory exits cleanly with status 1 and no uncaught stack trace.
      const emptyCwd = makeTempDir('anima-doc8-empty-cwd-');
      try {
        const missingDirRes = spawnSync(process.execPath, [SCRIPT_PATH, '--cwd', emptyCwd.dir], { encoding: 'utf8' });
        expect(missingDirRes.status).toBe(1);
        expect(missingDirRes.stderr).toContain('check_doc_commands: Doc directory not found:');
        expect(missingDirRes.stderr).not.toContain('at collectMarkdownFiles');
      } finally {
        emptyCwd.cleanup();
      }
    } finally {
      tmp.cleanup();
    }
  });

  it('executes blocks in order with bash -eo pipefail and stops at the first failure naming doc:line', () => {
    const tmp = makeTempDir();
    try {
      const markerFile = join(tmp.dir, 'later.txt');
      const docsDir = join(tmp.dir, 'docs');
      mkdirSync(docsDir, { recursive: true });
      writeFileSync(
        join(docsDir, '01-pass-then-fail.md'),
        [
          '<!-- doc-test -->',
          '```bash',
          'echo step1 > step1.txt',
          '```',
          '',
          '<!-- doc-test -->',
          '```bash',
          'false | true',
          '```',
          '',
          '<!-- doc-test -->',
          '```bash',
          `touch "${markerFile}"`,
          '```',
        ].join('\n'),
        'utf8',
      );

      const scan = collectDocTestBlocks(['docs'], tmp.dir) as { blocks: ExtractedBlock[]; errors: ParseError[] };
      expect(scan.errors).toEqual([]);
      const res = runDocCommands(scan.blocks, { cwd: tmp.dir }) as RunResult;

      expect(res.ok).toBe(false);
      expect(res.message).toBe('docs/01-pass-then-fail.md:7: exited with status 1');
      expect(res.executed).toHaveLength(2);
      expect(existsSync(join(tmp.dir, 'step1.txt'))).toBe(true);
      expect(existsSync(markerFile)).toBe(false);
    } finally {
      tmp.cleanup();
    }
  });

  it('fails when any cargo test block prints running 0 tests while allowing multi-crate >0 runs', () => {
    expect(
      detectCargoZeroTests(
        'cd src-tauri && cargo test --test nonexistent_filter',
        'Finished `test` profile\nRunning tests/foo.rs\nrunning 0 tests\ntest result: ok. 0 passed\n',
      ),
    ).toContain('running 0 tests');

    // Multi-crate `--lib <filter>` prints 0 tests for anima_domain and 4 tests for anima_engine_lib.
    expect(
      detectCargoZeroTests(
        'cd src-tauri && cargo test --lib core::sweeps::tests',
        [
          'Finished `test` profile [unoptimized + debuginfo] target(s) in 1.2s',
          'Running unittests src/lib.rs (target/debug/deps/anima_domain)',
          'running 0 tests',
          'Running unittests src/lib.rs (target/debug/deps/anima_engine_lib)',
          'running 4 tests',
          'test result: ok. 4 passed; 0 failed',
        ].join('\n'),
      ),
    ).toBeNull();

    // Two cargo test invocations in one block where the second runs 0 tests must fail.
    expect(
      detectCargoZeroTests(
        'cargo test --lib core::sweeps::tests && cargo test --test missing_filter',
        [
          'Finished `test` profile in 0.5s',
          'running 4 tests',
          'Finished `test` profile in 0.1s',
          'running 0 tests',
        ].join('\n'),
      ),
    ).toContain('running 0 tests');

    // End-to-end with realistic split-stream `cargo` stubs on PATH (Cargo metadata on stderr, test counts on stdout).
    const tmp = makeTempDir();
    try {
      const passBin = join(tmp.dir, 'bin-pass');
      mkdirSync(passBin, { recursive: true });
      writeFileSync(
        join(passBin, 'cargo'),
        [
          '#!/usr/bin/env bash',
          'echo "   Compiling anima-engine v0.1.0" >&2',
          'echo "    Finished \\`test\\` profile [unoptimized + debuginfo] target(s) in 0.42s" >&2',
          'echo "     Running tests/sample.rs (target/debug/deps/sample)" >&2',
          'echo "running 5 tests"',
          'echo "test result: ok. 5 passed; 0 failed"',
        ].join('\n'),
        { encoding: 'utf8', mode: 0o755 },
      );

      const passRes = runDocCommands(
        [{ file: 'docs/how-to/PASS.md', markerLine: 4, fenceLine: 5, lang: 'bash', script: 'cargo test --test sample' }],
        { cwd: tmp.dir, env: { PATH: `${passBin}:${process.env.PATH ?? ''}` } },
      ) as RunResult;
      expect(passRes.ok).toBe(true);
      expect(passRes.executed).toEqual([{ file: 'docs/how-to/PASS.md', fenceLine: 5, status: 0 }]);

      const zeroBin = join(tmp.dir, 'bin-zero');
      mkdirSync(zeroBin, { recursive: true });
      writeFileSync(
        join(zeroBin, 'cargo'),
        [
          '#!/usr/bin/env bash',
          'echo "   Compiling anima-engine v0.1.0" >&2',
          'echo "    Finished \\`test\\` profile [unoptimized + debuginfo] target(s) in 0.12s" >&2',
          'echo "     Running tests/sample.rs (target/debug/deps/sample)" >&2',
          'echo "running 0 tests"',
          'echo "test result: ok. 0 passed; 0 failed"',
        ].join('\n'),
        { encoding: 'utf8', mode: 0o755 },
      );

      const res = runDocCommands(
        [{ file: 'docs/how-to/STALE.md', markerLine: 4, fenceLine: 5, lang: 'bash', script: 'cargo test --test missing' }],
        { cwd: tmp.dir, env: { PATH: `${zeroBin}:${process.env.PATH ?? ''}` } },
      ) as RunResult;
      expect(res.ok).toBe(false);
      expect(res.message).toContain('docs/how-to/STALE.md:5');
      expect(res.message).toContain('running 0 tests');
    } finally {
      tmp.cleanup();
    }
  });

  it('validates npm scripts, node scripts/*.mjs paths (including ./scripts), and cargo --test targets via --static', () => {
    const okErrors = runStaticChecks(
      [
        {
          file: 'docs/tutorials/ok.md',
          markerLine: 1,
          fenceLine: 2,
          lang: 'bash',
          script: [
            'npm run check:docs',
            'node scripts/verify_e2_artifacts.mjs',
            'node ./scripts/verify_e2_artifacts.mjs',
            'cd src-tauri && cargo test --features desktop --test observables_tests',
          ].join('\n'),
        },
      ],
      REPO_ROOT,
    ) as string[];
    expect(okErrors).toEqual([]);

    const badErrors = runStaticChecks(
      [
        {
          file: 'docs/how-to/broken.md',
          markerLine: 9,
          fenceLine: 10,
          lang: 'bash',
          script: [
            'npm run nonexistent:script',
            'node scripts/does_not_exist.mjs',
            'node ./scripts/does_not_exist.mjs',
            'cd src-tauri && cargo test --test nonexistent_rust_target',
          ].join('\n'),
        },
      ],
      REPO_ROOT,
    ) as string[];
    expect(badErrors).toEqual([
      "docs/how-to/broken.md:10: unknown npm script 'nonexistent:script'",
      'docs/how-to/broken.md:10: referenced script not found: scripts/does_not_exist.mjs',
      'docs/how-to/broken.md:10: referenced script not found: scripts/does_not_exist.mjs',
      "docs/how-to/broken.md:10: unknown Rust test target '--test nonexistent_rust_target'",
    ]);
  });

  it('parses CLI options and wires check:doc-commands in package.json, docs-commands.yml, and ci.yml', () => {
    expect(parseArgs(['--dir', 'docs/a', '--static'])).toEqual({
      dirs: ['docs/a'],
      cwd: null,
      staticOnly: true,
      help: false,
    });
    expect(() => parseArgs(['--dry-run'])).toThrow(/Unknown or incomplete option/);
    expect(() => parseArgs(['--unknown'])).toThrow(/Unknown or incomplete option/);

    const pkg = JSON.parse(readFileSync(join(REPO_ROOT, 'package.json'), 'utf8')) as {
      scripts?: Record<string, string>;
    };
    expect(pkg.scripts?.['check:doc-commands']).toBe('node scripts/check_doc_commands.mjs');

    const workflow = readFileSync(join(REPO_ROOT, '.github/workflows/docs-commands.yml'), 'utf8');
    expect(workflow).toContain('npm run check:doc-commands');
    expect(workflow).not.toContain('--static');
    expect(workflow).not.toContain('--self-test');

    const ci = readFileSync(join(REPO_ROOT, '.github/workflows/ci.yml'), 'utf8');
    expect(ci).toContain('node scripts/check_doc_commands.mjs --static');
  });
});
