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
    let deepEnvS = 'echo hi';
    let deepNpmExec = 'echo hi';
    for (let i = 0; i < 6; i += 1) {
      deepNested = `bash -c ${JSON.stringify(deepNested)}`;
      deepEnvS = `env -S ${JSON.stringify(deepEnvS)}`;
      deepNpmExec = `npm exec -c ${JSON.stringify(deepNpmExec)}`;
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
      deepEnvS,
      deepNpmExec,
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
      // 7. Double-quoted command substitutions (single-line and multi-line)
      'echo "$(cargo run)"',
      'echo "`npm run tauri:dev`"',
      'echo "$(echo "$(cargo run)")"',
      'echo "$(\necho ok\ncargo run\n)"',
      'echo "`\necho ok\ncargo run\n`"',
      'echo "$(\necho ok\n# safe comment\ncargo run\n)"',
      // 8. env -S / --split-string payloads & combined short flags
      'env -S "cargo run"',
      'env -S "npm run tauri:dev"',
      'env --split-string "cargo run"',
      'env --split-string="cargo run"',
      'env -S "-u RUST_LOG cargo run"',
      'env -S "cargo" run',
      'env -iS "cargo run"',
      'env -iu RUST_LOG cargo run',
      // 9. bash/sh/zsh option-value flags (-co, -O, +O, --rcfile, --init-file)
      "bash -co errexit 'cargo run'",
      "bash -cO extglob 'cargo run'",
      "bash -O extglob -c 'cargo run'",
      "bash +O extglob -c 'cargo run'",
      "bash --rcfile /dev/null -c 'cargo run'",
      "bash --rcfile=/dev/null -c 'cargo run'",
      "bash --init-file /dev/null -c 'cargo run'",
      "bash --init-file=/dev/null -c 'cargo run'",
      "sh -co errexit 'cargo run'",
      "zsh -co errexit 'cargo run'",
      // 10. exec wrapper & flags
      'exec cargo run',
      'exec -a foo cargo run',
      'exec -c npm run tauri:dev',
      'exec -ca foo cargo run',
      // 11. npm exec / npm x / npx -c
      'npm exec -- tauri dev',
      'npm x -- tauri build',
      'npm exec -- vite preview',
      'npm exec -p @tauri-apps/cli -- tauri dev',
      'npm exec --package=@tauri-apps/cli tauri dev',
      "npm exec -c 'cargo run'",
      "npm exec --call='cargo run'",
      "npm x -c 'npm run tauri:dev'",
      "npm --call='cargo run' exec",
      "npx -c 'cargo run'",
      'npx --call="npm run tauri:dev"',
      // 12. Iteration 4 regressions: env -S operand termination, npm -p pre-subcmd, env -P, builtin
      'env -S "bash" -c "cargo run"',
      'env -S "FOO=1 bash" -c "cargo run"',
      'env -S "bash -c" "cargo run"',
      'env -P /Users/Shared/bin cargo run',
      'env -v -P /Users/Shared/bin cargo run',
      'env -P/Users/Shared/bin cargo run',
      'env -S "-P /Users/Shared/bin cargo run"',
      'npm -p run tauri:dev',
      'npm -p exec -- tauri dev',
      'npm -p x -- tauri build',
      'builtin exec cargo run',
      'builtin -- eval "cargo run"',
      'builtin exec npm run tauri:dev',
      // 13. Brace groups & compound shell keywords
      '{ cargo run; }',
      '{ npm run tauri:dev; }',
      'if true; then cargo run; fi',
      'if false; then echo ok; else cargo run; fi',
      'if cargo run; then echo ok; fi',
      'if false; then echo ok; elif cargo run; then echo ok; fi',
      'for i in 1; do cargo run; done',
      'while true; do cargo run; break; done',
      'until cargo run; do echo ok; done',
      // 14. Leading I/O redirections
      '>/dev/null cargo run',
      '2>&1 cargo run',
      'LOG=1 >/dev/null cargo run',
      '< /dev/null cargo run',
      '1>>out.log cargo run',
      '&>out.log npm run tauri:dev',
      // 15. Comment ')' inside double-quoted multi-line command substitution
      'echo "$(\n# step 1)\ncargo run\n)"',
      'echo "$(# )\ncargo run\n)"',
      'echo "$(echo a; # done)\ncargo run\n)"',
      // 16. Stdin / pipe / process-substitution / xargs / find -exec / dynamic eval
      "echo 'cargo run' | bash",
      "echo 'cargo run' | sh",
      "echo 'cargo run' | zsh",
      "echo 'cargo run' |& bash",
      "echo 'cargo run' | env bash",
      "bash <<< 'cargo run'",
      "bash <(echo 'cargo run')",
      "source <(echo 'cargo run')",
      ". <(echo 'cargo run')",
      'bash <<\'EOF\'\ncargo run\nEOF',
      'echo run | xargs cargo',
      'echo run | xargs -n1 cargo',
      'echo run | xargs -I{} cargo {}',
      "echo 'cargo run' | xargs bash -c",
      'echo tauri:dev | xargs npm run',
      'echo / | xargs rm -rf',
      'find . -maxdepth 0 -exec cargo run \\;',
      'find . -maxdepth 0 -execdir cargo run {} +',
      'find . -maxdepth 0 -ok bash -c "cargo run" \\;',
      'find . -maxdepth 0 -exec cargo {} \\;',
      'eval "$(echo \'cargo run\')"',
      'eval "`echo \'cargo run\'`"',
      "eval $(echo 'cargo run')",
      'builtin eval "$(echo \'cargo run\')"',
      // 17. Iteration 5 regressions: >| noclobber, bash -c -, npm/npx -c= & value flags & rum/urn, function name { ... }, xargs -S / -I / find -exec eval "$1" / BASH_ENV
      '>|out.txt cargo run',
      '>| out.txt cargo run',
      '1>| out.txt cargo run',
      '>| out.txt npm run tauri:dev',
      "bash -c - 'cargo run'",
      "sh -c - 'cargo run'",
      "zsh -c - 'cargo run'",
      'bash -ec - "cargo run"',
      'bash -o pipefail -c - "cargo run"',
      'env -S "bash" -c - "cargo run"',
      'npm exec -c="cargo run"',
      'npm x -c="cargo run"',
      'npm -c="cargo run" exec',
      'npx -c="cargo run"',
      'npm rum tauri:dev',
      'npm urn tauri:dev',
      'npm --color always run tauri:dev',
      'npm --color run tauri:dev',
      'npm --globalconfig /dev/null run tauri:dev',
      'npm --shell /bin/sh run tauri:dev',
      'npm --fetch-retries 1 run tauri:dev',
      'npm --unknown-future-opt val run tauri:dev',
      'function f { cargo run; }; f',
      'function f { npm run tauri:dev; }; f',
      'echo run | xargs -I {} -S 255 cargo {}',
      'echo run | xargs -I {} -S255 cargo {}',
      'echo "cargo run" | xargs -I{} sh -c "{}"',
      'echo "cargo run" | xargs -I CMD sh -c "CMD"',
      'printf "cargo run\\0" | xargs -0 -I{} sh -c "{}"',
      'echo "cargo run" | xargs --replace sh -c "{}"',
      'echo "cargo run" | xargs --replace=CMD sh -c "CMD"',
      'echo "tauri:dev" | xargs -I{} npm run {}',
      'echo "tauri:dev" | xargs -I{} npm rum {}',
      'echo "/" | xargs -I{} rm -rf {}',
      'printf \'"cargo run"\\n\' | xargs -n1 sh -c \'eval "$1"\' _',
      'find . -maxdepth 0 -exec sh -c \'eval "$1"\' _ "cargo run" \\;',
      'find . -maxdepth 0 -exec sh -c \'eval "$0"\' "cargo run" \\;',
      'find . -maxdepth 0 -exec sh -c \'$1\' _ "cargo run" \\;',
      'CMD="cargo run"; eval "$CMD"',
      "BASH_ENV=<(echo 'cargo run') bash -c 'echo ok'",
      'export BASH_ENV=/tmp/evil.sh',
      // 18. Iteration 6 regressions: attached/mid-cmd redirs, xargs empty wrapper, npm/npx gaps, positional/for-a/fd3, trap, $'...', $VAR & expand_aliases
      'cargo</dev/null run',
      'cargo<>out.txt run',
      'cargo>cargo_out.txt run',
      'cargo>|noclob.txt run',
      'npm</dev/null run tauri:dev',
      'npm>npm_out.txt run tauri:dev',
      'rm>rm_out.txt -rf /',
      "bash>bash_out.txt -c 'cargo run'",
      "eval>eval_out.txt 'cargo run'",
      'cargo 2>err.txt run',
      'cargo 2> err.txt run',
      'npm 2>err.txt run tauri:dev',
      'npm run 2>err.txt tauri:dev',
      'npm exec 2>err.txt -- tauri dev',
      'npx tauri 2>err.txt dev',
      'tauri 2>err.txt dev',
      'vite 2>err.txt preview',
      'echo "cargo run" | xargs env',
      'echo "cargo run" | xargs env -u FOO',
      'echo "cargo run" | xargs env -S',
      'echo "cargo run" | xargs -I { env -S "{"',
      'echo "cargo run" | xargs nice',
      'echo "cargo run" | xargs time',
      'echo "cargo run" | xargs command',
      'echo "cargo run" | xargs nohup',
      'npm -- run tauri:dev',
      'npm -- rum tauri:dev',
      'npm --workspaces false run tauri:dev',
      'npm --if-present false run tauri:dev',
      'npm --json false run tauri:dev',
      'npm --offline false run tauri:dev',
      'npm run --workspaces false tauri:dev',
      'npm run --if-present false tauri:dev',
      'npx -yc "cargo run"',
      'npm exec -yc "cargo run"',
      'npm exec -sc "cargo run"',
      'npm -sc "cargo run" exec',
      'npm -C. run tauri:dev',
      'npm run --init-version 1.0.0 tauri:dev',
      'npm run --allow-git all tauri:dev',
      'npm --init-author-name foo -c="cargo run" exec',
      'npm --init-author-name foo -c "cargo run" exec',
      'npm --init-author-name foo --call="cargo run" exec',
      'npm --init-author-email a@b.c -c="cargo run" exec',
      'npm --unknown-future-opt val -c="cargo run" exec',
      'npm run --unknown-future-opt val tauri:dev',
      'bash -c \'"$@"\' _ cargo run',
      "bash -c '$0 $1' cargo run",
      "bash -c '$1 $2' _ cargo run",
      'bash -c \'"$0" "$1"\' cargo run',
      'bash -c \'sh -c "$0"\' \'cargo run\'',
      'find . -maxdepth 0 -exec sh -c \'for a; do $a; done\' _ "cargo run" \\;',
      'find . -maxdepth 0 -execdir sh -c \'for a; do $a; done\' _ "cargo run" \\;',
      'echo "cargo run" | xargs -I{} sh -c \'for a; do $a; done\' _ {}',
      'echo \'"cargo run"\' | xargs sh -c \'for a; do sh -c "$a"; done\' _',
      "bash /dev/fd/3 3<<< 'cargo run'",
      "sh /dev/fd/3 3<<< 'cargo run'",
      "source /dev/fd/3 3<<< 'cargo run'",
      ". /dev/fd/3 3<<< 'cargo run'",
      "exec 3<<< 'cargo run'; bash /dev/fd/3",
      "trap 'cargo run' EXIT",
      "trap 'npm run tauri:dev' EXIT",
      "trap 'rm -rf /' EXIT",
      "$'cargo' run",
      "bash -c $'cargo\\x20run'",
      "npm run $'tauri\\x3adev'",
      "$'rm' -rf $'/'",
      'CMD="cargo run"; $CMD',
      'C=cargo; $C run',
      'IFS=:; CMD="cargo:run"; $CMD',
      'T=/; rm -rf "$T"',
      "shopt -s expand_aliases\nalias x='cargo run'\nx",
      // 19. Iteration 7 regressions: unquoted $(...) / `...` splitting, npm --no-* / -sy / -py / $VAR, <<$'EOF', xargs+find, tauri -v, vite --port, rm ~user, coproc
      '$(echo cargo) run',
      '`echo cargo` run',
      '$(echo npm) run tauri:dev',
      'cargo $(echo run)',
      'cargo `echo run`',
      'cargo --manifest-path $(echo src-tauri/Cargo.toml) run',
      'npm $(echo run) tauri:dev',
      'npm "$(echo run)" tauri:dev',
      'npm "`echo run`" tauri:dev',
      'npm run $(echo tauri:dev)',
      'npm --prefix $(echo .) run tauri:dev',
      'npm exec -- $(echo tauri) dev',
      'npx $(echo tauri) dev',
      'tauri $(echo dev)',
      'rm -rf $(echo /)',
      'rm $(echo -rf) /',
      'rm "$(echo -rf)" /',
      "IFS=; bash -c $(printf 'cargo\\x20run')",
      'bash -c $(echo tauri)',
      'S=run; npm $S tauri:dev',
      "bash -c 'npm $1 tauri:dev' _ run",
      'X=exec; npm $X -c "cargo run"',
      'X=x; npm $X -- tauri dev',
      'npm --no-color false run tauri:dev',
      'npm --no-color true run tauri:dev',
      'npm --no-color always run tauri:dev',
      'npm --no-if-present false run tauri:dev',
      'npm --no-audit false run tauri:dev',
      'npm --no-fund false run tauri:dev',
      'npm run --no-color false tauri:dev',
      "npm exec --no-color false -c 'cargo run'",
      "npx --no-color false -c 'cargo run'",
      'npx --no-color false tauri dev',
      'npm --no-silent false run tauri:dev',
      'npm -sy false run tauri:dev',
      'npm -sy true run tauri:dev',
      'npm -sf false run tauri:dev',
      'npm -sg false run tauri:dev',
      'npm -sp false run tauri:dev',
      'npm -sl false run tauri:dev',
      'npm -sj false run tauri:dev',
      'npm -sa false run tauri:dev',
      'npm run -sy false tauri:dev',
      'npm run -sy true tauri:dev',
      'npm exec -sy false -- tauri dev',
      'npx -sy false tauri dev',
      'npm -sy false exec -c "cargo run"',
      'npm -sy true exec -c "cargo run"',
      'npm exec -py pkg -c "cargo run"',
      'npx -py pkg -c "cargo run"',
      "cat <<$'EOF' > /dev/null\nhello\nEOF\ncargo run",
      "cat <<E$'O'F > /dev/null\nhi\nEOF\ncargo run",
      "cat <<-$'EOF' > /dev/null\n\thi\n\tEOF\ncargo run",
      'echo "cargo" | xargs -I{} find . -maxdepth 0 -exec {} run \\;',
      'echo "cargo" | xargs -I{} find . -maxdepth 0 -execdir {} run \\;',
      'echo "npm" | xargs -I{} find . -maxdepth 0 -exec {} run tauri:dev \\;',
      'echo "tauri:dev" | xargs -I{} find . -maxdepth 0 -exec npm run {} \\;',
      'echo "rm" | xargs -I{} find . -maxdepth 0 -exec {} -rf / \\;',
      'echo "/" | xargs -I{} find . -maxdepth 0 -exec rm -rf {} \\;',
      'tauri -v dev',
      'tauri --verbose dev',
      'npx tauri -v dev',
      'npm exec -- tauri -v dev',
      'vite --port 3000',
      'vite --port 4173 preview',
      'npx vite --port 3000',
      'npm exec -- vite --port 3000',
      'rm -rf ~root',
      'rm -rf ~+',
      'rm -rf ~-',
      'coproc cargo run',
      'coproc MYPROC { cargo run; }',
      // 20. Iteration 8 regressions: multi-line unquoted $(...)/`...`/$(cat <<EOF), backslash heredoc cont, $"" heredoc delim, case/backtick in $(...), brace/glob, rm dynamic $F/$1/$@, npm -v/-h/--no-* false, tauri android|ios
      '$(\necho cargo\n) run',
      '`\necho cargo\n` run',
      'cargo $(\necho run\n)',
      'npm $(\necho run\n) tauri:dev',
      'npm run $(\necho tauri:dev\n)',
      'rm -rf $(\necho /\n)',
      'tauri $(\necho dev\n)',
      '$(cat <<\'EOF\'\ncargo run\nEOF\n)',
      'cargo $(cat <<\'EOF\'\nrun\nEOF\n)',
      'npm run $(cat <<\'EOF\'\ntauri:dev\nEOF\n)',
      'rm -rf $(cat <<\'EOF\'\n/\nEOF\n)',
      'echo $(cargo run',
      '$(echo cargo',
      'echo `cargo run',
      '`echo cargo',
      'cat <<EOF \\\n&& cargo run\nhello\nEOF',
      'cat <<EOF \\\n; cargo run\nhello\nEOF',
      'cat <<EOF \\\n&& npm run tauri:dev\nhello\nEOF',
      "cat <<'EOF' \\\n&& rm -rf /\nhello\nEOF",
      'cat <<EOF \\\n\\\n&& cargo run\nhi\nEOF',
      'cat <<$"EOF" > /dev/null\nhi\nEOF\ncargo run',
      'cat <<-$"EOF" > /dev/null\n\thi\n\tEOF\ncargo run',
      'cat <<E$"O"F > /dev/null\nhi\nEOF\ncargo run',
      'cat <<$"" > /dev/null\nhi\n\ncargo run',
      'npm --prefix $(echo `case x in x) echo . ;; esac` >/dev/null; echo .) run tauri:dev',
      'cargo -C $(echo `case x in x) echo . ;; esac` >/dev/null; echo .) run',
      'echo $(case x in x) cargo run ;; esac)',
      'echo $(case a in a) cargo run;; esac)',
      'echo "$(case a in a) cargo run;; esac)"',
      'echo $(\ncase x in\nx) cargo run ;;\nesac\n)',
      'echo $(case x in (a) cargo run ;; esac)',
      'echo $(case x in a) case y in b) cargo run ;; esac ;; esac)',
      '{cargo,} run',
      'cargo {run,}',
      'cargo {run,check}',
      'cargo {r,check}',
      '{npm,} run tauri:dev',
      'npm {run,} tauri:dev',
      'npm run {tauri:dev,check:docs}',
      'rm -rf {/,/tmp/safe}',
      'rm {-rf,} /',
      '{bash,} -c "cargo run"',
      "bash -c '{cargo,} run'",
      '{eval,} "cargo run"',
      'eval {cargo,run}',
      'env {cargo,} run',
      'echo run | xargs {cargo,}',
      'tauri {dev,info}',
      'vite {build,preview}',
      "/bin/ba[s]h -c 'cargo run'",
      '/usr/bin/e[n]v cargo run',
      './node_modules/.bin/tau[r]i dev',
      './node_modules/.bin/vi[t]e --port 3000',
      '$PROBE/bin/car[g]o run',
      "bash -c '$PROBE/bin/car[g]o run'",
      'find . -maxdepth 0 -exec $PROBE/bin/car[g]o run \\;',
      'echo run | xargs $PROBE/bin/car[g]o',
      'F="-rf /"; rm $F',
      'F="-rf ~"; rm $F',
      "bash -c 'rm $1' _ '-rf /'",
      'set -- -rf /; rm "$@"',
      "bash -c 'rm $@' _ -rf /",
      'bash -c \'rm "$@"\' _ -rf /',
      'echo "-rf /" | xargs rm',
      'echo "-rf /" | xargs -I{} rm {}',
      'npm -v false run tauri:dev',
      'npm -h false run tauri:dev',
      'npm --version false run tauri:dev',
      'npm --help false run tauri:dev',
      'npm --usage false run tauri:dev',
      'npm --versions false run tauri:dev',
      'npm -sv false run tauri:dev',
      'npm -sh false run tauri:dev',
      'npm run -v false tauri:dev',
      'npm run -h false tauri:dev',
      'npm run --help false tauri:dev',
      'npm exec -v false -- tauri dev',
      'npm exec -h false -- tauri dev',
      'npx --no-unknown false tauri dev',
      'npx --no-unknown true tauri dev',
      'npx --no-silent false tauri dev',
      'npx -y --no-unknown false tauri dev',
      'npm exec --no-unknown false tauri dev',
      'npm exec --no-unknown true tauri dev',
      'npm exec --no-silent false tauri dev',
      'npx --unknown-opt val tauri dev',
      'npm exec --unknown-opt val tauri dev',
      'tauri android dev',
      'tauri android build',
      'tauri android run',
      'tauri ios dev',
      'tauri ios build',
      'tauri ios run',
      'tauri android -v dev',
      'npx tauri android dev',
      'npm exec -- tauri ios build',
      // 21. Iteration 9 regressions:
      // Finding 1: heredoc inside $(...) / "$(...)" double-stripping
      "echo $(\ncat <<'EOF'\nhello\nEOF\ncargo run\n)",
      "echo \"$(\ncat <<'EOF'\nhello\nEOF\nrm -rf /\n)\"",
      "echo $(\ncat <<'EOF'\nhello\nEOF\nnpm run tauri:dev\n)",
      "echo $(\ncat <<'EOF'\nhello\nEOF\ntauri dev\n)",
      "echo \"$(\ncat <<'EOF'\nhello\nEOF\ncargo run\n)\"",
      // Finding 2: unquoted heredoc expansion & backslash-newline continuation delimiters
      'cat <<EOF\n$(cargo run)\nEOF',
      "cat <<EOF\n'$(cargo run)'\nEOF",
      'cat <<EOF\n"$(cargo run)"\nEOF',
      'cat <<EOF\n`cargo run`\nEOF',
      'cat <<EOF\n$(npm run tauri:dev)\nEOF',
      'cat <<EOF\n$(rm -rf /)\nEOF',
      'cat <<-EOF\n\t`rm -rf /`\n\tEOF',
      'cat <<-EOF\n\t$(cargo run)\n\tEOF',
      'echo $(cat <<EOF\n$(cargo run)\nEOF\n)',
      'cat <<EOF\n$(echo ok\nEOF',
      'cat <<EOF\n$\\\n(cargo run)\nEOF',
      'cat <<\\\nEOF\nhi\nEOF\ncargo run',
      'cat <<E\\\nOF\nhi\nEOF\ncargo run',
      'cat <<-\\\nEOF\n\thi\n\tEOF\ncargo run',
      'cat <<\\\nEOF\n$(cargo run)\nEOF',
      // Finding 3: case inside $(...) prefixes & zsh ;|
      'echo $(if case x in x) cargo run ;; esac; then echo ok; fi)',
      'echo $(while case x in x) cargo run ;; esac; do break; done)',
      'echo $(until case x in x) cargo run ;; esac; do break; done)',
      'echo $(time case x in x) cargo run ;; esac)',
      'echo $(time -p case x in x) cargo run ;; esac)',
      'echo $(coproc case x in x) cargo run ;; esac; wait)',
      'echo $(foo() case x in x) cargo run ;; esac\nfoo)',
      'echo $(function foo case x in x) cargo run ;; esac\nfoo)',
      'echo $(>/dev/null case x in x) cargo run;; esac)',
      'echo $(2>/dev/null case x in x) cargo run;; esac)',
      'echo $({fd}>/dev/null case x in x) cargo run;; esac)',
      'echo $(case a in a) echo 1 ;| a) cargo run ;; esac)',
      // Finding 4: $"..." in tokenizeSegmentMeta & named FD redirection {fd}>...
      'rm -rf $"/"',
      'rm -rf $"."',
      'rm -rf /tmp/$".."',
      'npm run tauri$":dev"',
      'npm --prefix $"." run tauri$":dev"',
      'cargo build --manifest-path src-tauri/Cargo.toml --bin $"anima-engine"',
      'cargo build --manifest-path src-tauri/Cargo.toml --bin=$"anima-engine"',
      'cargo {fd}>/dev/null run',
      'npm {fd}>/dev/null run tauri:dev',
      'tauri {fd}>/dev/null dev',
      "bash {fd}>/dev/null -c 'cargo run'",
      'bash {fd}<<< "cargo run"',
      // Finding 5: xargs / xargs ... find -exec + npm exec / npm x tail validation
      'echo "dev" | xargs npm exec -- tauri',
      'echo "dev" | xargs -I{} npm exec -- tauri {}',
      'echo "dev" | xargs -I{} npm x -- tauri {}',
      'echo "dev" | xargs -I{} npm exec -- tauri android {}',
      'echo "run" | xargs -I{} npm exec -- cargo {}',
      'echo "-rf /" | xargs -I{} npm exec -- rm {}',
      'echo "-rf /" | xargs npm exec -- rm',
      'echo "run" | xargs -I{} find . -maxdepth 0 -exec npm exec -- cargo {} \\;',
      'echo "dev" | xargs npm exec --unknown-opt val tauri',
      // Finding 6: cargo --bin dynamic & brace expansion
      'cargo --bin={anima-engine,anima-lab} build',
      'cargo build --manifest-path src-tauri/Cargo.toml --bin $(echo anima-engine)',
      'cargo build --manifest-path src-tauri/Cargo.toml --bin=$(echo anima-engine)',
      'cargo build --manifest-path src-tauri/Cargo.toml --bin {anima-engine,anima-lab}',
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
      'npm exec -- eslint src',
      'npm x -- tsc --noEmit',
      "npm exec -c 'echo ok'",
      'npm -p run check:docs',
      'npm rum check:docs',
      'npm urn check:docs',
      'npm --color always run check:docs',
      'npm --color run check:docs',
      'npm --globalconfig /dev/null run check:docs',
      'env -u RUST_LOG CARGO_BUILD_JOBS=4 nice -n 10 cargo test --manifest-path src-tauri/Cargo.toml --test e2_seam_tests',
      'env -S "echo ok"',
      'env -S "bash" -c "echo ok"',
      'env --split-string="RUST_LOG=info" echo ok',
      'env -P /Users/Shared/bin echo ok',
      'exec echo ok',
      'exec -a custom-name echo ok',
      'builtin echo ok',
      'cargo build --manifest-path src-tauri/Cargo.toml \\\n  --release --bin anima-lab',
      "bash -c 'echo ok'",
      "bash -c - 'echo ok'",
      "bash -co errexit 'echo ok'",
      "bash --rcfile /dev/null -c 'echo ok'",
      'eval "echo ok"',
      'cat <<EOF-1 > out.txt\ncargo run\nEOF-1\necho ok',
      'cat <<-EOF > out2.txt\n\tcargo run\n\tEOF\necho ok',
      'cat <<\\EOF > out3.txt\ncargo run\nEOF\necho ok',
      'cat <<EOF > out4.txt\n  EOF\ncargo run\nEOF',
      'echo "\ncargo run\n"',
      "echo '$(cargo run)'",
      'echo "\\$(cargo run)"',
      'echo "$(echo ok)"',
      'echo "$(echo ok\necho done)"',
      'echo "$(\n# step 1)\necho ok\n)"',
      'echo "$(\n# cargo run in comment\necho ok\n)"',
      'echo "$(\ncat <<\'EOF\'\ncargo run\nEOF\n)"',
      '{ echo ok; echo done; }',
      'if true; then echo ok; fi',
      'function safe_fn { echo ok; }; safe_fn',
      'echo "${HOME:-/tmp}"',
      'ls file.{a,b}',
      '> /dev/null',
      'echo ok >| bash',
      '>|/tmp/safe_noclobber echo ok',
      'cat < in.txt',
      'echo ok | xargs echo',
      'echo hi | xargs -I{} echo {}',
      'echo "--test e2_seam_tests" | xargs cargo test --manifest-path src-tauri/Cargo.toml',
      "find . -name '*.md' -exec echo {} \\;",
      'cat <(echo ok)',
      'echo ok 2>&1 | tee log',
      'rm -rf src-tauri/target/doc2-gate-neg >| /tmp/rm.log',
      'echo a | xargs echo',
      'npm --workspaces run check:docs',
      "sh -c 'echo $1' _ ok",
      "trap 'rm -f /tmp/safe.lock' EXIT",
      'trap - EXIT',
      "echo $'hello\\nworld'",
      'RUST_LOG=info echo "$VAR"',
      'RUST_LOG=info cargo test --manifest-path src-tauri/Cargo.toml --test e2_seam_tests',
      'rm -rf /tmp/safe',
      'echo $(echo ok)',
      'echo $((1 + 2))',
      'npm --no-color run check:docs',
      'npm -s run check:docs',
      "cat <<'EOF'\ncargo run\nEOF\necho ok",
      'vite build',
      'vite optimize',
      'vite --help',
      'tauri info',
      'tauri --help',
      'tauri -v info',
      'rm -rf src-tauri/target/release/bundle',
      'rm -rf target/doc2',
      'echo "foo" | xargs -I{} find . -maxdepth 0 -exec echo {} \\;',
      'coproc MYPROC { echo ok; }',
      'echo $(\necho ok\n)',
      "echo $(cat <<'EOF'\ncargo run\nEOF\n)",
      "echo $(cat <<'EOF'\nhello )\nEOF\n)",
      'echo $(case x in x) echo ok ;; esac)',
      'echo $(case x in (x) echo ok ;; esac)',
      'echo $(echo case; echo ok)',
      'for f in docs/*.md; do echo "$f"; done',
      'npm exec --unknown-safe-flag eslint src',
      'npx --unknown-safe-flag tsc --noEmit',
      'tauri android --help',
      'tauri ios --help',
      'rm -rf /tmp/safe*',
      'if [ -f package.json ]; then echo ok; fi',
      'echo a[0]',
      "echo $(\ncat <<'EOF'\nhello )\nEOF\n)",
      "cat <<'EOF'\n$(cargo run)\nEOF",
      'cat <<"EOF"\n$(cargo run)\nEOF',
      'cat <<\\EOF\n$(cargo run)\nEOF',
      'cat <<EOF\n\\$(cargo run)\nEOF',
      'cat <<\\\n\'EOF\'\n$(cargo run)\nEOF',
      'cat <<EOF\n$(echo ok)\nEOF',
      'echo $(case a in a) echo 1 ;| a) echo 2 ;; esac)',
      'echo $(>/dev/null case x in x) echo ok ;; esac)',
      'echo $(foo() case x in x) echo ok ;; esac\nfoo)',
      'echo {fd}>/dev/null ok',
      'echo "src" | xargs npm exec -- eslint',
      'echo "--test e2_seam_tests" | xargs npm exec -- cargo test --manifest-path src-tauri/Cargo.toml',
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
