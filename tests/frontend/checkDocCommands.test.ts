import { describe, expect, it } from 'vitest';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
// @ts-expect-error -- check_doc_commands.mjs has no .d.mts declaration
import { collectMarkdownFiles, extractDocCommands, runDocCommands, scanDocDirectories, validateBlocks, validateCommand } from '../../scripts/check_doc_commands.mjs';

interface ExtractedBlock {
  file: string;
  markerLine: number;
  fenceLine: number;
  contentStartLine: number;
  endLine: number;
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

interface ValidateCommandResult {
  ok: boolean;
  reason?: string;
  lineIndex?: number;
  lineText?: string;
}

interface BlockSafetyError {
  file: string;
  line: number;
  commandLine: number;
  reason: string;
  lineText: string;
  message: string;
}

interface MarkdownFileEntry {
  absPath: string;
  relPath: string;
}

interface ScanResult {
  files: MarkdownFileEntry[];
  blocks: ExtractedBlock[];
  parseErrors: ParseError[];
  safetyErrors: BlockSafetyError[];
}

interface RunResult {
  ok: boolean;
  failedBlock?: ExtractedBlock;
  exitStatus?: number;
  message?: string;
  executed: Array<{ file: string; fenceLine: number; status: number; dryRun: boolean }>;
}

const REPO_ROOT = resolve(import.meta.dirname, '../..');
const SCRIPT_PATH = resolve(REPO_ROOT, 'scripts/check_doc_commands.mjs');
const WORKFLOW_PATH = resolve(REPO_ROOT, '.github/workflows/docs-commands.yml');

function makeTempDir(prefix = 'anima-doc-commands-'): { dir: string; cleanup: () => void } {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  return {
    dir,
    cleanup: () => rmSync(dir, { recursive: true, force: true }),
  };
}

describe('check_doc_commands — extraction, ordering, safety, and fail-fast gate', () => {
  it('extracts bash, sh, zsh, bare, and tilde fences preceded by <!-- doc-test --> and ignores unmarked/nested ones', () => {
    const md = [
      '# Sample Doc',
      '',
      'Prose mentioning `<!-- doc-test -->` inline must be ignored.',
      '',
      '```bash',
      'echo "unmarked"',
      '```',
      '',
      '````markdown',
      '<!-- doc-test -->',
      '```bash',
      'echo "inside-existing-fence"',
      '```',
      '````',
      '',
      '<!-- doc-test -->',
      '```bash',
      'echo "first"',
      '```',
      '',
      '<!-- doc-test -->',
      '',
      '```sh',
      'echo "second"',
      '```',
      '',
      '<!-- doc-test -->',
      '```zsh',
      'echo "third"',
      '```',
      '',
      '<!-- doc-test -->',
      '```',
      'echo "fourth"',
      '```',
      '',
      '<!-- doc-test -->',
      '~~~bash',
      'echo "fifth"',
      '~~~',
    ].join('\n');

    const res = extractDocCommands(md, 'docs/tutorials/sample.md') as ExtractResult;
    expect(res.errors).toEqual([]);
    expect(res.blocks).toHaveLength(5);
    expect(res.blocks.map((b) => [b.lang, b.script, b.markerLine, b.fenceLine])).toEqual([
      ['bash', 'echo "first"', 16, 17],
      ['sh', 'echo "second"', 21, 23],
      ['zsh', 'echo "third"', 27, 28],
      ['', 'echo "fourth"', 32, 33],
      ['bash', 'echo "fifth"', 37, 38],
    ]);
  });

  it('rejects dangling, prose-followed, non-shell, unclosed, and empty <!-- doc-test --> blocks with exact doc:line', () => {
    const md = [
      '<!-- doc-test -->',
      'Not a fence line',
      '',
      '<!-- doc-test -->',
      '```powershell',
      'Write-Host "ps"',
      '```',
      '',
      '<!-- doc-test -->',
      '```json',
      '{"k": 1}',
      '```',
      '',
      '<!-- doc-test -->',
      '```bash',
      '   ',
      '```',
      '',
      '<!-- doc-test -->',
      '```bash',
      'echo "never closed"',
    ].join('\n');

    const res = extractDocCommands(md, 'docs/how-to/broken.md') as ExtractResult;
    expect(res.blocks).toEqual([]);
    expect(res.errors).toHaveLength(5);
    expect(res.errors.map((e) => `${e.file}:${e.line}`)).toEqual([
      'docs/how-to/broken.md:1',
      'docs/how-to/broken.md:5',
      'docs/how-to/broken.md:10',
      'docs/how-to/broken.md:15',
      'docs/how-to/broken.md:20',
    ]);

    const eofDangling = extractDocCommands('<!-- doc-test -->\n', 'docs/how-to/eof.md') as ExtractResult;
    expect(eofDangling.errors).toHaveLength(1);
    expect(eofDangling.errors[0]?.message).toContain('docs/how-to/eof.md:1');
  });

  it('orders files deterministically: tutorials/ ASCII-sorted, then how-to/ ASCII-sorted, then blocks top-to-bottom', () => {
    const fixture = makeTempDir();
    try {
      mkdirSync(join(fixture.dir, 'tutorials'), { recursive: true });
      mkdirSync(join(fixture.dir, 'how-to'), { recursive: true });
      writeFileSync(join(fixture.dir, 'how-to', 'B_SECOND.md'), '<!-- doc-test -->\n```bash\necho b\n```\n', 'utf8');
      writeFileSync(join(fixture.dir, 'how-to', 'A_FIRST.md'), '<!-- doc-test -->\n```bash\necho a\n```\n', 'utf8');
      writeFileSync(
        join(fixture.dir, 'tutorials', '02-next.md'),
        '<!-- doc-test -->\n```bash\necho t2\n```\n',
        'utf8',
      );
      writeFileSync(
        join(fixture.dir, 'tutorials', '01-intro.md'),
        '<!-- doc-test -->\n```bash\necho t1a\n```\n<!-- doc-test -->\n```bash\necho t1b\n```\n',
        'utf8',
      );

      const files = collectMarkdownFiles([fixture.dir], REPO_ROOT) as MarkdownFileEntry[];
      expect(files.map((f) => f.relPath)).toEqual([
        'tutorials/01-intro.md',
        'tutorials/02-next.md',
        'how-to/A_FIRST.md',
        'how-to/B_SECOND.md',
      ]);

      const scan = scanDocDirectories([fixture.dir], fixture.dir) as ScanResult;
      expect(scan.parseErrors).toEqual([]);
      expect(scan.safetyErrors).toEqual([]);
      expect(scan.blocks.map((b) => `${b.file}:${b.script}`)).toEqual([
        'tutorials/01-intro.md:echo t1a',
        'tutorials/01-intro.md:echo t1b',
        'tutorials/02-next.md:echo t2',
        'how-to/A_FIRST.md:echo a',
        'how-to/B_SECOND.md:echo b',
      ]);
    } finally {
      fixture.cleanup();
    }
  });

  it('rejects forbidden/unsafe commands while allowing heredocs, comments, ! checks, and target/tmp cleanup', () => {
    const forbidden = [
      'npm run tauri:dev',
      'npm run tauri:build',
      'npm run tauri',
      'npm run dev',
      'npm run preview',
      'npx tauri dev',
      'tauri build',
      'vite',
      'vite preview',
      './src-tauri/target/release/anima-engine',
      'cargo run --bin anima-lab',
      'CARGO_BUILD_JOBS=4 nice -n 10 cargo run --example run_e2_brain_experiment',
      'rm -rf /',
      'rm -rf ~',
      'rm -rf .',
      'rm -rf ..',
      'rm -rf $HOME',
      'rm -rf "$HOME"',
    ];

    for (const cmd of forbidden) {
      const res = validateCommand(cmd) as ValidateCommandResult;
      expect(res.ok, `expected rejection for: ${cmd}`).toBe(false);
    }

    const allowed = [
      'rm -rf src-tauri/target/doc2-gate-neg\nmkdir -p src-tauri/target/doc2-gate-neg',
      'rm -rf /tmp/anima-doc-test-clean',
      "cat <<'EOF' > manifest.json\n{\n  \"note\": \"never run cargo run or npm run tauri:dev\"\n}\nEOF",
      '# cargo run in a comment should not fail\nnode -e "console.log(\'cargo run\')"',
      '! node scripts/verify_e2_artifacts.mjs src-tauri/target/doc2-gate-neg',
    ];

    for (const cmd of allowed) {
      const res = validateCommand(cmd) as ValidateCommandResult;
      expect(res.ok, `unexpected rejection for safe command: ${res.reason ?? ''}`).toBe(true);
    }

    const blockErrs = validateBlocks([
      {
        file: 'how-to/BAD.md',
        markerLine: 9,
        fenceLine: 10,
        contentStartLine: 11,
        endLine: 13,
        lang: 'bash',
        script: 'echo ok\ncargo run --bin anima-lab',
      },
    ]) as BlockSafetyError[];
    expect(blockErrs).toHaveLength(1);
    expect(blockErrs[0]?.message).toContain('how-to/BAD.md:10');
  });

  it('rejects cargo/npm/npx/wrapper/subshell/heredoc/continuation/command-sub bypasses without false positives', () => {
    let deepNested = 'echo hi';
    for (let i = 0; i < 6; i += 1) {
      deepNested = `bash -c ${JSON.stringify(deepNested)}`;
    }

    const bypassAttempts = [
      // 1. cargo global value-taking flags, r alias, and --bin=anima-engine
      'cargo --manifest-path src-tauri/Cargo.toml run --bin anima-lab',
      'cargo -C src-tauri run',
      'cargo --target-dir /tmp/tgt --color never r --bin anima-lab',
      'cargo +nightly --config build.jobs=4 run',
      'cargo --lockfile-path src-tauri/Cargo.lock --message-format short run',
      'cargo build --manifest-path src-tauri/Cargo.toml --bin=anima-engine',
      // 2. bash/sh/zsh -c (including combined short flags) & eval
      "bash -c 'cargo run'",
      'sh -c "cargo run"',
      'zsh -c "npm run tauri:dev"',
      "bash -ec 'cargo run'",
      "bash -lc 'rm -rf /'",
      'bash -euoc "npm run tauri:dev"',
      'bash -o pipefail -c "cargo run"',
      'eval "cargo run"',
      "eval -- 'npm run tauri:dev'",
      deepNested,
      // 3. npm / npx flags & -- separator
      'npm --silent run tauri:dev',
      'npm --prefix . run tauri:dev',
      'npm run -- tauri:dev',
      'npm --workspace web -s run --if-present dev',
      'npx --yes tauri dev',
      'npx -p @tauri-apps/cli -- tauri build',
      'npx -q vite preview',
      // 4. Wrapper flags (env, nice, command, timeout)
      'env -u RUST_LOG cargo run',
      'env -i --unset=RUST_LOG -C /tmp cargo run',
      'nice -10 cargo run',
      'nice --adjustment=5 cargo run',
      'command -p cargo run',
      'timeout -k 5s -s TERM 10s cargo run',
      'timeout 30 npm run tauri:dev',
      // 5. Backslash line continuation splitting tokens
      'car\\\ngo run --bin anima-lab',
      'npm \\\nrun tauri:dev',
      // 6. Multi-line quotes & heredoc delimiter parsing (<< vs <<-, EOF-1, \EOF)
      'echo "hello\n<<EOF"\ncargo run',
      "echo 'hello\n<<EOF'\ncargo run",
      'cat <<EOF-1\nhi\nEOF-1\ncargo run',
      'cat <<\\EOF\nhi\nEOF\ncargo run',
      'cat <<-EOF\n\t\thi\n\tEOF\ncargo run',
      // 7. Double-quoted command substitutions
      'echo "$(cargo run)"',
      'echo "`npm run tauri:dev`"',
      'echo "$(echo "$(cargo run)")"',
    ];

    for (const cmd of bypassAttempts) {
      const res = validateCommand(cmd) as ValidateCommandResult;
      expect(res.ok, `expected safety rejection for bypass: ${cmd}`).toBe(false);
    }

    const safeForms = [
      'cargo build --manifest-path src-tauri/Cargo.toml --release --bin anima-lab',
      'cargo test --manifest-path src-tauri/Cargo.toml --test e2_seam_tests',
      'npm --silent run check:docs',
      'npm run -- check:e2',
      'env -u RUST_LOG CARGO_BUILD_JOBS=4 nice -n 10 cargo test --manifest-path src-tauri/Cargo.toml --test e2_seam_tests',
      'cargo build --manifest-path src-tauri/Cargo.toml \\\n  --release --bin anima-lab',
      "bash -c 'echo ok'",
      'eval "echo ok"',
      'cat <<EOF-1 > out.txt\ncargo run\nEOF-1\necho ok',
      'cat <<-EOF > out2.txt\n\tcargo run\n\tEOF\necho ok',
      'cat <<\\EOF > out3.txt\ncargo run\nEOF\necho ok',
      'cat <<EOF > out4.txt\n  EOF\ncargo run\nEOF',
      'echo "\ncargo run\n"',
      "echo '$(cargo run)'",
      'echo "\\$(cargo run)"',
      'echo "$(echo ok)"',
    ];

    for (const cmd of safeForms) {
      const res = validateCommand(cmd) as ValidateCommandResult;
      expect(res.ok, `unexpected rejection for safe command (${cmd}): ${res.reason ?? ''}`).toBe(true);
    }
  });

  it('executes blocks in order, supports heredocs and ! negation, and fails fast naming doc:line without running later blocks', () => {
    const fixture = makeTempDir();
    try {
      const docsDir = join(fixture.dir, 'docs');
      const workDir = join(fixture.dir, 'work');
      mkdirSync(join(docsDir, 'tutorials'), { recursive: true });
      mkdirSync(join(docsDir, 'how-to'), { recursive: true });
      mkdirSync(workDir, { recursive: true });

      writeFileSync(
        join(docsDir, 'tutorials', '01-first.md'),
        [
          '<!-- doc-test -->',
          '```bash',
          'echo "step1" > trace.txt',
          "cat <<'EOF' > payload.json",
          '{"cargo_run_note": "ok inside heredoc"}',
          'EOF',
          '! false',
          '```',
        ].join('\n'),
        'utf8',
      );
      writeFileSync(
        join(docsDir, 'how-to', '02-fail.md'),
        [
          '# Second doc',
          '',
          '<!-- doc-test -->',
          '```bash',
          'echo "step2" >> trace.txt',
          'node -e "process.exit(3)"',
          '```',
          '',
          '<!-- doc-test -->',
          '```bash',
          'touch must-not-run.flag',
          '```',
        ].join('\n'),
        'utf8',
      );

      const scan = scanDocDirectories([docsDir], workDir) as ScanResult;
      expect(scan.parseErrors).toEqual([]);
      expect(scan.safetyErrors).toEqual([]);
      expect(scan.blocks).toHaveLength(3);

      const run = runDocCommands(scan.blocks, {
        cwd: workDir,
        stdio: ['pipe', 'pipe', 'pipe'],
        log: () => {},
      }) as RunResult;

      expect(run.ok).toBe(false);
      expect(run.exitStatus).toBe(3);
      expect(run.message).toContain('how-to/02-fail.md:4');
      expect(readFileSync(join(workDir, 'trace.txt'), 'utf8').trim().split('\n')).toEqual([
        'step1',
        'step2',
      ]);
      expect(existsSync(join(workDir, 'payload.json'))).toBe(true);
      expect(existsSync(join(workDir, 'must-not-run.flag'))).toBe(false);
    } finally {
      fixture.cleanup();
    }
  });

  it('preserves CLI contract for --self-test, --dry-run, --require-blocks, missing --dir, and fail-fast negative control', () => {
    const fixture = makeTempDir();
    try {
      const docsDir = join(fixture.dir, 'docs');
      const workDir = join(fixture.dir, 'work');
      const emptyDir = join(fixture.dir, 'empty');
      mkdirSync(join(docsDir, 'tutorials'), { recursive: true });
      mkdirSync(workDir, { recursive: true });
      mkdirSync(emptyDir, { recursive: true });

      writeFileSync(
        join(docsDir, 'tutorials', '01-cli.md'),
        [
          '<!-- doc-test -->',
          '```bash',
          'node -e "require(\'node:fs\').writeFileSync(\'ran.txt\', \'1\')"',
          '```',
          '<!-- doc-test -->',
          '```bash',
          'node -e "process.exit(5)"',
          '```',
          '<!-- doc-test -->',
          '```bash',
          'node -e "require(\'node:fs\').writeFileSync(\'after-fail.txt\', \'1\')"',
          '```',
        ].join('\n'),
        'utf8',
      );
      writeFileSync(join(emptyDir, 'readme.md'), '# No markers here\n', 'utf8');

      const selfTestRun = spawnSync(process.execPath, [SCRIPT_PATH, '--self-test'], {
        cwd: REPO_ROOT,
        encoding: 'utf8',
      });
      expect(selfTestRun.status).toBe(0);
      expect(selfTestRun.stdout).toContain('self-test: all positive and negative controls passed');

      const dryRun = spawnSync(
        process.execPath,
        [SCRIPT_PATH, '--dir', fixture.dir, '--cwd', workDir, '--dry-run'],
        { cwd: REPO_ROOT, encoding: 'utf8' },
      );
      expect(dryRun.status).toBe(0);
      expect(dryRun.stdout).toContain('docs/tutorials/01-cli.md:2');
      expect(existsSync(join(workDir, 'ran.txt'))).toBe(false);

      const negRun = spawnSync(
        process.execPath,
        [SCRIPT_PATH, '--dir', fixture.dir, '--cwd', workDir],
        { cwd: REPO_ROOT, encoding: 'utf8' },
      );
      expect(negRun.status).toBe(1);
      expect(negRun.stderr).toContain('FAIL docs/tutorials/01-cli.md:6');
      expect(existsSync(join(workDir, 'ran.txt'))).toBe(true);
      expect(existsSync(join(workDir, 'after-fail.txt'))).toBe(false);

      const emptyDefault = spawnSync(process.execPath, [SCRIPT_PATH, '--dir', emptyDir], {
        cwd: REPO_ROOT,
        encoding: 'utf8',
      });
      expect(emptyDefault.status).toBe(0);
      expect(emptyDefault.stdout).toContain('0 <!-- doc-test --> block(s) found');

      const emptyStrict = spawnSync(
        process.execPath,
        [SCRIPT_PATH, '--dir', emptyDir, '--require-blocks'],
        { cwd: REPO_ROOT, encoding: 'utf8' },
      );
      expect(emptyStrict.status).toBe(1);
      expect(emptyStrict.stderr).toContain('--require-blocks');

      const missingDir = spawnSync(
        process.execPath,
        [SCRIPT_PATH, '--dir', join(fixture.dir, 'does-not-exist')],
        { cwd: REPO_ROOT, encoding: 'utf8' },
      );
      expect(missingDir.status).toBe(1);
    } finally {
      fixture.cleanup();
    }
  });

  it('wires weekly schedule + workflow_dispatch in .github/workflows/docs-commands.yml without PR triggers', () => {
    const yaml = readFileSync(WORKFLOW_PATH, 'utf8');
    expect(yaml).toMatch(/^on\s*:/m);
    expect(yaml).toMatch(/^\s+schedule\s*:/m);
    expect(yaml).toMatch(/^\s+-\s*cron\s*:/m);
    expect(yaml).toMatch(/^\s+workflow_dispatch\s*:/m);
    expect(yaml).not.toMatch(/^\s*pull_request\s*:/m);
    expect(yaml).not.toMatch(/^\s*push\s*:/m);
    expect(yaml).toMatch(/^permissions\s*:\s*(?:\r?\n)+\s+contents\s*:\s*read\b/m);
    expect(yaml).toMatch(/^\s+runs-on\s*:\s*ubuntu-latest\b/m);
    expect(yaml).toMatch(/uses\s*:\s*Swatinem\/rust-cache@v2\b/);
    expect(yaml).toMatch(/run\s*:\s*node\s+scripts\/check_doc_commands\.mjs\s+--self-test\b/);
    expect(yaml).toMatch(/run\s*:\s*npm\s+run\s+check:doc-commands\b/);
  });
});
