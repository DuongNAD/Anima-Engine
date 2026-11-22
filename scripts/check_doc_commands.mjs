#!/usr/bin/env node
// Extract and run `<!-- doc-test -->` fenced shell blocks in docs/tutorials and docs/how-to.
//
// Usage:
//   node scripts/check_doc_commands.mjs [--dir <path>]... [--cwd <path>] [--dry-run] [--require-blocks]
//   node scripts/check_doc_commands.mjs --self-test
//   node scripts/check_doc_commands.mjs --help

import { spawnSync } from 'node:child_process';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join, relative, resolve } from 'node:path';
import process from 'node:process';
import { pathToFileURL } from 'node:url';

const DEFAULT_SUBDIRS = ['docs/tutorials', 'docs/how-to'];

// Layer 1: Fence language allowlist (runDocCommands executes with `bash -eo pipefail -s`,
// so `zsh` is strictly rejected to eliminate bash-vs-zsh semantic mismatches).
const ALLOWED_SHELL_LANGS = new Set(['', 'bash', 'sh']);

// Layer 3: Strict environment variable prefix allowlist.
const SAFE_ENV_ALLOWLIST = {
  CARGO_BUILD_JOBS: /^[1-9][0-9]{0,2}$/,
  RUST_LOG: /^[A-Za-z0-9_,=:-]+$/,
  NO_COLOR: /^[01]$/,
};

// Layer 4: Closed allowlists.
const SAFE_NPM_SCRIPTS = new Set([
  'check:e2',
  'check:docs',
  'check:text-hygiene',
  'check:root-hygiene',
]);
const SAFE_CARGO_SUBCOMMANDS = new Set(['test', 'build', 'check']);
const SAFE_CARGO_PACKAGES = new Set(['anima-engine', 'anima-domain', 'anima-worldgen']);
const SAFE_ANIMA_LAB_SUBCOMMANDS = new Set(['ensemble', 'sweep', 'branch']);
const SAFE_GREP_FLAGS = new Set(['-q', '-E', '-F', '-n', '-i']);
const SAFE_RM_FLAGS = new Set(['-rf', '-fr', '-f', '-r']);
const SAFE_NODE_SCRIPT_FLAGS = new Set(['--self-test', '--check', '--dry-run']);
const ENV_ALLOWED_HEADS = new Set([
  'cargo',
  'node',
  'npm',
  './src-tauri/target/release/anima-lab',
  './src-tauri/target/release/examples/run_e2_brain_experiment',
]);

const UNQUOTED_WORD_CHAR_RE = /^[A-Za-z0-9._/:=-]$/;
const HEREDOC_IDENT_RE = /^[A-Za-z_][A-Za-z0-9_]*$/;
const ENV_ASSIGN_RE = /^[A-Za-z_][A-Za-z0-9_]*=.*$/;

const NODE_EVAL_FORBIDDEN_RE =
  /(?:child_process|worker_threads|inspector|createRequire|mainModule|getBuiltinModule|__proto__|WebAssembly|XMLHttpRequest|execSync|execFileSync|execFile|spawnSync|spawn|fork|rmSync|unlinkSync|rmdirSync|renameSync|truncateSync|chmodSync|chownSync|symlinkSync|linkSync|cpSync|copyFileSync|appendFileSync|openSync|createWriteStream)\b|\b(?:vm|net|http|https|dgram|tls|cluster|repl|dns|os|module|eval|Function|import|Reflect|Proxy|globalThis|global|window|constructor|prototype|fetch|setTimeout|setInterval|setImmediate|promises|binding|dlopen|exec|rm|unlink|rmdir|rename|truncate|chmod|chown|symlink|link)\b/;

function toPosixPath(p) {
  return p.replace(/\\/g, '/');
}

function compareAscii(a, b) {
  if (a < b) return -1;
  if (a > b) return 1;
  return 0;
}

function isExistingDir(dirPath) {
  if (!existsSync(dirPath)) return false;
  try {
    return statSync(dirPath).isDirectory();
  } catch {
    return false;
  }
}

function isDisallowedControlCode(code) {
  return code === 0 || code === 13 || (code < 32 && code !== 9 && code !== 10) || code === 127;
}

export function parseArgs(argv) {
  const opts = {
    dirs: [],
    cwd: null,
    dryRun: false,
    selfTest: false,
    requireBlocks: false,
    help: false,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--help' || arg === '-h') {
      opts.help = true;
    } else if (arg === '--dry-run') {
      opts.dryRun = true;
    } else if (arg === '--self-test') {
      opts.selfTest = true;
    } else if (arg === '--require-blocks') {
      opts.requireBlocks = true;
    } else if (arg === '--dir') {
      if (i + 1 >= argv.length) {
        throw new Error('--dir requires a directory path argument');
      }
      opts.dirs.push(argv[i + 1]);
      i += 1;
    } else if (arg.startsWith('--dir=')) {
      const val = arg.slice('--dir='.length);
      if (!val) throw new Error('--dir requires a non-empty directory path');
      opts.dirs.push(val);
    } else if (arg === '--cwd') {
      if (i + 1 >= argv.length) {
        throw new Error('--cwd requires a directory path argument');
      }
      opts.cwd = argv[i + 1];
      i += 1;
    } else if (arg.startsWith('--cwd=')) {
      const val = arg.slice('--cwd='.length);
      if (!val) throw new Error('--cwd requires a non-empty directory path');
      opts.cwd = val;
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  return opts;
}

function walkMarkdownFiles(rootDir, displayRoot, seenAbs, out) {
  const entries = readdirSync(rootDir, { withFileTypes: true }).sort((a, b) =>
    compareAscii(a.name, b.name),
  );
  const filesHere = [];
  for (const entry of entries) {
    const abs = join(rootDir, entry.name);
    if (entry.isDirectory()) {
      walkMarkdownFiles(abs, displayRoot, seenAbs, filesHere);
    } else if (entry.isFile() && entry.name.endsWith('.md')) {
      if (!seenAbs.has(abs)) {
        seenAbs.add(abs);
        filesHere.push({
          absPath: abs,
          relPath: toPosixPath(relative(displayRoot, abs)),
        });
      }
    }
  }
  filesHere.sort((a, b) => compareAscii(a.relPath, b.relPath));
  out.push(...filesHere);
}

/**
 * Resolve scan directories and return deterministically ordered markdown files.
 * Ordering: tutorials/ sorted ASCII POSIX relative path, then how-to/ sorted ASCII POSIX relative
 * path, or recursive ASCII POSIX relative path for arbitrary directories.
 */
export function collectMarkdownFiles(dirs, cwd) {
  const resolvedCwd = resolve(cwd);
  const seenAbs = new Set();
  const collected = [];

  if (!dirs || dirs.length === 0) {
    let foundDefault = false;
    for (const sub of DEFAULT_SUBDIRS) {
      const absSub = join(resolvedCwd, sub);
      if (isExistingDir(absSub)) {
        foundDefault = true;
        walkMarkdownFiles(absSub, resolvedCwd, seenAbs, collected);
      }
    }
    if (!foundDefault) {
      throw new Error(
        `none of the default doc directories (${DEFAULT_SUBDIRS.join(', ')}) exist under ${resolvedCwd}`,
      );
    }
    return collected;
  }

  for (const rawDir of dirs) {
    const absDir = resolve(rawDir);
    if (!isExistingDir(absDir)) {
      throw new Error(`--dir path does not exist or is not a directory: ${rawDir}`);
    }

    const directTutorials = join(absDir, 'tutorials');
    const directHowTo = join(absDir, 'how-to');
    const nestedTutorials = join(absDir, 'docs', 'tutorials');
    const nestedHowTo = join(absDir, 'docs', 'how-to');

    if (isExistingDir(directTutorials) || isExistingDir(directHowTo)) {
      if (isExistingDir(directTutorials)) {
        walkMarkdownFiles(directTutorials, absDir, seenAbs, collected);
      }
      if (isExistingDir(directHowTo)) {
        walkMarkdownFiles(directHowTo, absDir, seenAbs, collected);
      }
    } else if (isExistingDir(nestedTutorials) || isExistingDir(nestedHowTo)) {
      if (isExistingDir(nestedTutorials)) {
        walkMarkdownFiles(nestedTutorials, absDir, seenAbs, collected);
      }
      if (isExistingDir(nestedHowTo)) {
        walkMarkdownFiles(nestedHowTo, absDir, seenAbs, collected);
      }
    } else {
      walkMarkdownFiles(absDir, absDir, seenAbs, collected);
    }
  }

  return collected;
}

function parseFenceOpen(line) {
  const m = /^\s{0,3}(`{3,}|~{3,})(.*)$/.exec(line);
  if (!m) return null;
  const seq = m[1];
  const info = m[2];
  if (seq[0] === '`' && info.includes('`')) return null;
  const lang = (info.trim().split(/\s+/)[0] ?? '').toLowerCase();
  return { char: seq[0], length: seq.length, lang };
}

function isFenceClose(line, fenceChar, fenceLength) {
  const m = /^\s{0,3}(`{3,}|~{3,})\s*$/.exec(line);
  if (!m) return false;
  return m[1][0] === fenceChar && m[1].length >= fenceLength;
}

/**
 * Extract `<!-- doc-test -->` fenced code blocks from markdown text.
 * Returns `{ blocks, errors }` with 1-based line numbers.
 */
export function extractDocCommands(markdownText, filePath) {
  const lines = markdownText.replace(/\r\n/g, '\n').split('\n');
  const blocks = [];
  const errors = [];
  let activeFence = null;

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];

    if (activeFence !== null) {
      if (isFenceClose(line, activeFence.char, activeFence.length)) {
        activeFence = null;
      }
      continue;
    }

    if (line.trim() === '<!-- doc-test -->') {
      const markerLine = i + 1;
      let j = i + 1;
      while (j < lines.length && lines[j].trim() === '') {
        j += 1;
      }

      if (j >= lines.length) {
        errors.push({
          file: filePath,
          line: markerLine,
          message: `${filePath}:${markerLine}: dangling <!-- doc-test --> marker at end of file`,
        });
        break;
      }

      const open = parseFenceOpen(lines[j]);
      if (!open) {
        errors.push({
          file: filePath,
          line: markerLine,
          message: `${filePath}:${markerLine}: dangling <!-- doc-test --> marker not followed by a fenced code block (found at line ${j + 1})`,
        });
        i = j - 1;
        continue;
      }

      const fenceLine = j + 1;
      let k = j + 1;
      let closed = false;
      while (k < lines.length) {
        if (isFenceClose(lines[k], open.char, open.length)) {
          closed = true;
          break;
        }
        k += 1;
      }

      if (!closed) {
        errors.push({
          file: filePath,
          line: fenceLine,
          message: `${filePath}:${fenceLine}: unclosed fenced code block after <!-- doc-test --> (marker at line ${markerLine})`,
        });
        break;
      }

      if (!ALLOWED_SHELL_LANGS.has(open.lang)) {
        errors.push({
          file: filePath,
          line: fenceLine,
          message: `${filePath}:${fenceLine}: unsupported fence language '${open.lang}' after <!-- doc-test --> (expected bash, sh, or untagged)`,
        });
        i = k;
        continue;
      }

      const script = lines.slice(j + 1, k).join('\n');
      if (script.trim().length === 0) {
        errors.push({
          file: filePath,
          line: fenceLine,
          message: `${filePath}:${fenceLine}: empty <!-- doc-test --> fenced code block (marker at line ${markerLine})`,
        });
        i = k;
        continue;
      }

      blocks.push({
        file: filePath,
        markerLine,
        fenceLine,
        contentStartLine: j + 2,
        endLine: k + 1,
        lang: open.lang,
        script,
      });
      i = k;
      continue;
    }

    const open = parseFenceOpen(line);
    if (open) {
      activeFence = open;
    }
  }

  return { blocks, errors };
}

function isTokenBoundaryChar(ch) {
  return (
    ch === undefined ||
    ch === ' ' ||
    ch === '\t' ||
    ch === '\n' ||
    ch === ';' ||
    ch === '&' ||
    ch === '|' ||
    ch === '>' ||
    ch === '<' ||
    ch === '#'
  );
}

/**
 * Layer 2: Strict byte and lexical allowlist tokenizer.
 */
function tokenizeAllowedShell(script) {
  const normalized = script.replace(/\r\n/g, '\n');
  const lines = normalized.split('\n');
  const tokens = [];

  const fail = (reason, lineIndex) => ({
    ok: false,
    reason,
    lineIndex,
    lineText: (lines[lineIndex] ?? '').trim(),
  });

  let i = 0;
  let lineIndex = 0;
  let currentWord = null;
  let hadSpaceBefore = true;
  let pendingHeredoc = null;

  const flushWord = () => {
    if (currentWord !== null) {
      tokens.push({
        kind: 'WORD',
        value: currentWord.value,
        quoted: null,
        lineIndex: currentWord.lineIndex,
      });
      currentWord = null;
      hadSpaceBefore = false;
    }
  };

  while (i < normalized.length) {
    if (pendingHeredoc !== null && lineIndex > pendingHeredoc.bodyStartLine - 1) {
      // Read HEREDOC_BODY line-by-line until exact line === pendingHeredoc.delim.
      const bodyLines = [];
      let closed = false;
      while (i <= normalized.length) {
        const nlIdx = normalized.indexOf('\n', i);
        const rawLine = nlIdx === -1 ? normalized.slice(i) : normalized.slice(i, nlIdx);
        for (let k = 0; k < rawLine.length; k += 1) {
          if (isDisallowedControlCode(rawLine.charCodeAt(k))) {
            return fail('disallowed control byte inside heredoc body', lineIndex);
          }
        }
        if (rawLine === pendingHeredoc.delim) {
          closed = true;
          pendingHeredoc.token.content = bodyLines.join('\n');
          pendingHeredoc = null;
          if (nlIdx === -1) {
            i = normalized.length;
          } else {
            i = nlIdx + 1;
            lineIndex += 1;
          }
          hadSpaceBefore = true;
          break;
        }
        bodyLines.push(rawLine);
        if (nlIdx === -1) {
          break;
        }
        i = nlIdx + 1;
        lineIndex += 1;
      }
      if (!closed) {
        return fail(`unterminated heredoc '${pendingHeredoc.delim}'`, pendingHeredoc.lineIndex);
      }
      continue;
    }

    const ch = normalized[i];
    const code = ch.charCodeAt(0);

    if (isDisallowedControlCode(code)) {
      return fail('disallowed control byte', lineIndex);
    }
    if (code > 0x7e) {
      return fail('disallowed non-ASCII byte', lineIndex);
    }

    if (ch === ' ' || ch === '\t') {
      flushWord();
      hadSpaceBefore = true;
      i += 1;
      continue;
    }

    if (ch === '\n') {
      flushWord();
      tokens.push({ kind: 'NEWLINE', lineIndex });
      if (pendingHeredoc !== null) {
        pendingHeredoc.bodyStartLine = lineIndex + 1;
      }
      lineIndex += 1;
      hadSpaceBefore = true;
      i += 1;
      continue;
    }

    if (ch === '#') {
      if (currentWord !== null) {
        return fail("unexpected '#' inside word", lineIndex);
      }
      while (i < normalized.length && normalized[i] !== '\n') {
        const cCode = normalized.charCodeAt(i);
        if (isDisallowedControlCode(cCode)) {
          return fail('disallowed control byte inside comment', lineIndex);
        }
        i += 1;
      }
      continue;
    }

    if (ch === '\\') {
      if (
        currentWord === null &&
        hadSpaceBefore &&
        normalized[i + 1] === '\n' &&
        pendingHeredoc === null
      ) {
        i += 2;
        lineIndex += 1;
        hadSpaceBefore = true;
        continue;
      }
      return fail('disallowed backslash or line continuation', lineIndex);
    }

    if (ch === "'") {
      if (currentWord !== null) {
        return fail('mid-word single quote splicing is disallowed', lineIndex);
      }
      const startLine = lineIndex;
      i += 1;
      let val = '';
      let closed = false;
      while (i < normalized.length) {
        const sc = normalized[i];
        const sCode = sc.charCodeAt(0);
        if (isDisallowedControlCode(sCode)) {
          return fail('disallowed control byte inside single-quoted string', startLine);
        }
        if (sc === '\n') {
          return fail('multi-line single-quoted string is disallowed', startLine);
        }
        if (sc === "'") {
          closed = true;
          i += 1;
          break;
        }
        val += sc;
        i += 1;
      }
      if (!closed) {
        return fail('unclosed single-quoted string', startLine);
      }
      if (!isTokenBoundaryChar(normalized[i])) {
        return fail('single-quoted string must be followed by token boundary', startLine);
      }
      tokens.push({ kind: 'WORD', value: val, quoted: 'single', lineIndex: startLine });
      hadSpaceBefore = false;
      continue;
    }

    if (ch === '"') {
      if (currentWord !== null) {
        return fail('mid-word double quote splicing is disallowed', lineIndex);
      }
      const startLine = lineIndex;
      i += 1;
      let val = '';
      let closed = false;
      while (i < normalized.length) {
        const dc = normalized[i];
        const dCode = dc.charCodeAt(0);
        if (isDisallowedControlCode(dCode)) {
          return fail('disallowed control byte inside double-quoted string', startLine);
        }
        if (dc === '\n') {
          return fail('multi-line double-quoted string is disallowed', startLine);
        }
        if (dc === '`') {
          return fail('backtick command substitution inside double quotes is disallowed', startLine);
        }
        if (dc === '\\') {
          const next = normalized[i + 1];
          if (
            next === undefined ||
            next === '\n' ||
            next === '"' ||
            next === '\\' ||
            next === '$' ||
            next === '`'
          ) {
            return fail('disallowed escape inside double-quoted string', startLine);
          }
          val += `\\${next}`;
          i += 2;
          continue;
        }
        if (dc === '$') {
          if (normalized[i + 1] === '/') {
            val += '$/';
            i += 2;
            continue;
          }
          return fail("disallowed '$' expansion inside double-quoted string", startLine);
        }
        if (dc === '"') {
          closed = true;
          i += 1;
          break;
        }
        val += dc;
        i += 1;
      }
      if (!closed) {
        return fail('unclosed double-quoted string', startLine);
      }
      if (!isTokenBoundaryChar(normalized[i])) {
        return fail('double-quoted string must be followed by token boundary', startLine);
      }
      tokens.push({ kind: 'WORD', value: val, quoted: 'double', lineIndex: startLine });
      hadSpaceBefore = false;
      continue;
    }

    if (ch === ';') {
      if (pendingHeredoc !== null) {
        return fail("disallowed ';' on heredoc-opening line", lineIndex);
      }
      const next = normalized[i + 1];
      if (next === ';' || next === '&' || next === '|') {
        return fail(`disallowed shell operator ';${next}'`, lineIndex);
      }
      flushWord();
      tokens.push({ kind: 'OP', value: ';', lineIndex });
      hadSpaceBefore = true;
      i += 1;
      continue;
    }

    if (ch === '&') {
      if (pendingHeredoc !== null) {
        return fail("disallowed '&' on heredoc-opening line", lineIndex);
      }
      if (normalized[i + 1] === '&' && normalized[i + 2] !== '&') {
        flushWord();
        tokens.push({ kind: 'OP', value: '&&', lineIndex });
        hadSpaceBefore = true;
        i += 2;
        continue;
      }
      return fail("disallowed '&' or '&>' operator", lineIndex);
    }

    if (ch === '|') {
      if (pendingHeredoc !== null) {
        return fail("disallowed '|' on heredoc-opening line", lineIndex);
      }
      const next = normalized[i + 1];
      if (next === '&') {
        return fail("disallowed '|&' operator", lineIndex);
      }
      if (next === '|') {
        if (normalized[i + 2] === '|') {
          return fail("disallowed '|||' operator", lineIndex);
        }
        flushWord();
        tokens.push({ kind: 'OP', value: '||', lineIndex });
        hadSpaceBefore = true;
        i += 2;
        continue;
      }
      flushWord();
      tokens.push({ kind: 'OP', value: '|', lineIndex });
      hadSpaceBefore = true;
      i += 1;
      continue;
    }

    if (ch === '!') {
      if (currentWord !== null || !hadSpaceBefore || (normalized[i + 1] !== ' ' && normalized[i + 1] !== '\t')) {
        return fail("disallowed '!' outside leading pipeline negation", lineIndex);
      }
      tokens.push({ kind: 'BANG', lineIndex });
      hadSpaceBefore = false;
      i += 1;
      continue;
    }

    if (ch === '>') {
      let fd = 1;
      if (currentWord !== null) {
        if (currentWord.value === '1' || currentWord.value === '2') {
          fd = Number(currentWord.value);
          currentWord = null;
        } else {
          return fail('redirection must be separated from preceding word by whitespace', lineIndex);
        }
      }
      const next = normalized[i + 1];
      if (next === '|' || next === '&') {
        return fail(`disallowed redirection '>${next}'`, lineIndex);
      }
      let op = '>';
      let advance = 1;
      if (next === '>') {
        if (normalized[i + 2] === '>' || normalized[i + 2] === '&' || normalized[i + 2] === '|') {
          return fail('disallowed redirection operator', lineIndex);
        }
        op = '>>';
        advance = 2;
      }
      const after = normalized[i + advance];
      if (after !== ' ' && after !== '\t') {
        return fail('output redirection must be followed by whitespace and target path', lineIndex);
      }
      tokens.push({ kind: 'REDIR', op, fd, lineIndex });
      hadSpaceBefore = true;
      i += advance;
      continue;
    }

    if (ch === '<') {
      if (currentWord !== null) {
        return fail('input redirection attached to word is disallowed', lineIndex);
      }
      if (normalized[i + 1] !== '<') {
        return fail("disallowed '<' input redirection", lineIndex);
      }
      const afterOp = normalized[i + 2];
      if (afterOp === '-' || afterOp === '<') {
        return fail("disallowed '<<-' or '<<<' redirection", lineIndex);
      }
      if (pendingHeredoc !== null) {
        return fail('multiple heredocs on the same line are disallowed', lineIndex);
      }
      let scan = i + 2;
      while (scan < normalized.length && (normalized[scan] === ' ' || normalized[scan] === '\t')) {
        scan += 1;
      }
      if (normalized[scan] !== "'") {
        return fail("only single-quoted <<'IDENT' heredocs are allowed", lineIndex);
      }
      scan += 1;
      const closeQ = normalized.indexOf("'", scan);
      if (closeQ === -1 || normalized.slice(scan, closeQ).includes('\n')) {
        return fail('unclosed heredoc delimiter quote', lineIndex);
      }
      const delim = normalized.slice(scan, closeQ);
      if (!HEREDOC_IDENT_RE.test(delim)) {
        return fail(`invalid heredoc delimiter '${delim}'`, lineIndex);
      }
      const postChar = normalized[closeQ + 1];
      if (
        postChar !== undefined &&
        postChar !== ' ' &&
        postChar !== '\t' &&
        postChar !== '\n' &&
        postChar !== '#'
      ) {
        return fail('heredoc delimiter must be followed by whitespace or newline', lineIndex);
      }
      const hdTok = { kind: 'HEREDOC', delim, content: '', lineIndex };
      tokens.push(hdTok);
      pendingHeredoc = { delim, lineIndex, bodyStartLine: lineIndex + 1, token: hdTok };
      hadSpaceBefore = false;
      i = closeQ + 1;
      continue;
    }

    if (!UNQUOTED_WORD_CHAR_RE.test(ch)) {
      return fail(`disallowed shell metacharacter '${ch}'`, lineIndex);
    }

    if (currentWord === null) {
      currentWord = { value: ch, lineIndex };
    } else {
      currentWord.value += ch;
    }
    i += 1;
  }

  flushWord();
  if (pendingHeredoc !== null) {
    return fail(`unterminated heredoc '${pendingHeredoc.delim}'`, pendingHeredoc.lineIndex);
  }

  return { ok: true, tokens, lines };
}

function hasTraversalOrWeirdSegments(p) {
  if (typeof p !== 'string' || p.length === 0) return true;
  if (p.includes('\\') || p.includes('//') || p.endsWith('/')) return true;
  const segs = p.split('/');
  return segs.some((s) => s === '.' || s === '..' || s.length === 0);
}

function isSafeTargetPath(p) {
  return (
    !hasTraversalOrWeirdSegments(p) &&
    /^src-tauri\/target\/[A-Za-z0-9._-]+(?:\/[A-Za-z0-9._-]+)*$/.test(p)
  );
}

function isSafeOutputPath(p) {
  return p === '/dev/null' || isSafeTargetPath(p);
}

function isSafeRepoReadPath(p) {
  return (
    !hasTraversalOrWeirdSegments(p) &&
    /^(?:src-tauri\/target|src-tauri\/tests\/fixtures|artifacts|docs)\/[A-Za-z0-9._-]+(?:\/[A-Za-z0-9._-]+)*$/.test(
      p,
    )
  );
}

function isSafeTempRmTarget(p) {
  return (
    !hasTraversalOrWeirdSegments(p.slice(1)) &&
    /^\/tmp\/anima-[A-Za-z0-9._-]+(?:\/[A-Za-z0-9._-]+)*$/.test(p)
  );
}

/**
 * Layer 3: Structural parser (simple pipelines + allowlisted env prefixes + safe redir targets).
 */
function parseAllowedPipelines(tokens, lines) {
  const commands = [];
  const fail = (reason, lineIndex) => ({
    ok: false,
    reason,
    lineIndex,
    lineText: (lines[lineIndex] ?? '').trim(),
  });

  let idx = 0;

  const isSep = (tok) =>
    tok.kind === 'NEWLINE' ||
    (tok.kind === 'OP' && (tok.value === ';' || tok.value === '&&' || tok.value === '||'));

  const skipNewlines = () => {
    while (idx < tokens.length && tokens[idx].kind === 'NEWLINE') {
      idx += 1;
    }
  };

  const parseSimpleCmd = () => {
    const envAssigns = [];
    while (
      idx < tokens.length &&
      tokens[idx].kind === 'WORD' &&
      tokens[idx].quoted === null &&
      ENV_ASSIGN_RE.test(tokens[idx].value)
    ) {
      const tok = tokens[idx];
      const eq = tok.value.indexOf('=');
      const name = tok.value.slice(0, eq);
      const val = tok.value.slice(eq + 1);
      const pattern = SAFE_ENV_ALLOWLIST[name];
      if (!pattern || !pattern.test(val)) {
        return fail(`disallowed environment assignment '${tok.value}'`, tok.lineIndex);
      }
      envAssigns.push({ name, value: val, lineIndex: tok.lineIndex });
      idx += 1;
    }

    if (idx >= tokens.length || isSep(tokens[idx]) || (tokens[idx].kind === 'OP' && tokens[idx].value === '|')) {
      const errLine =
        idx < tokens.length ? tokens[idx].lineIndex : envAssigns[0]?.lineIndex ?? 0;
      return fail('missing command after prefix or operator', errLine);
    }

    const headTok = tokens[idx];
    if (headTok.kind !== 'WORD' || headTok.quoted !== null || headTok.value.includes('=')) {
      return fail('expected unquoted command name', headTok.lineIndex);
    }
    idx += 1;

    const args = [];
    const redirs = [];
    const heredocs = [];

    while (idx < tokens.length && !isSep(tokens[idx]) && !(tokens[idx].kind === 'OP' && tokens[idx].value === '|')) {
      const tok = tokens[idx];
      if (tok.kind === 'BANG') {
        return fail("'!' is only allowed at the start of a pipeline", tok.lineIndex);
      }
      if (tok.kind === 'REDIR') {
        const targetTok = tokens[idx + 1];
        if (!targetTok || targetTok.kind !== 'WORD' || targetTok.quoted !== null) {
          return fail('redirection target must be an unquoted safe path', tok.lineIndex);
        }
        if (!isSafeOutputPath(targetTok.value)) {
          return fail(`disallowed redirection target '${targetTok.value}'`, targetTok.lineIndex);
        }
        redirs.push({
          fd: tok.fd,
          op: tok.op,
          target: targetTok.value,
          lineIndex: tok.lineIndex,
        });
        idx += 2;
        continue;
      }
      if (tok.kind === 'HEREDOC') {
        heredocs.push({ delim: tok.delim, content: tok.content, lineIndex: tok.lineIndex });
        idx += 1;
        continue;
      }
      if (tok.kind === 'WORD') {
        args.push(tok);
        idx += 1;
        continue;
      }
      return fail('unexpected token in command', tok.lineIndex);
    }

    return {
      ok: true,
      cmd: {
        head: headTok.value,
        headLineIndex: headTok.lineIndex,
        envAssigns,
        args,
        redirs,
        heredocs,
      },
    };
  };

  skipNewlines();
  while (idx < tokens.length) {
    const cur = tokens[idx];
    if (cur.kind === 'OP') {
      return fail(`unexpected leading or consecutive operator '${cur.value}'`, cur.lineIndex);
    }

    if (cur.kind === 'BANG') {
      idx += 1;
    }

    const firstRes = parseSimpleCmd();
    if (!firstRes.ok) return firstRes;
    commands.push(firstRes.cmd);

    while (idx < tokens.length && tokens[idx].kind === 'OP' && tokens[idx].value === '|') {
      const pipeTok = tokens[idx];
      idx += 1;
      skipNewlines();
      if (idx >= tokens.length || isSep(tokens[idx]) || (tokens[idx].kind === 'OP' && tokens[idx].value === '|')) {
        return fail("expected command after '|'", pipeTok.lineIndex);
      }
      const pipedRes = parseSimpleCmd();
      if (!pipedRes.ok) return pipedRes;
      commands.push(pipedRes.cmd);
    }

    if (idx < tokens.length) {
      const sep = tokens[idx];
      if (!isSep(sep)) {
        return fail('expected command separator', sep.lineIndex);
      }
      if (sep.kind === 'OP') {
        idx += 1;
        skipNewlines();
        if (idx >= tokens.length) {
          return fail(`trailing '${sep.value}' operator without following command`, sep.lineIndex);
        }
        if (tokens[idx].kind === 'OP') {
          return fail(`consecutive operators '${sep.value}' and '${tokens[idx].value}'`, tokens[idx].lineIndex);
        }
      } else {
        skipNewlines();
      }
    }
  }

  if (commands.length === 0) {
    return fail('command block contains no executable statements', 0);
  }

  return { ok: true, commands };
}

function validateSafeNodeEval(payload) {
  if (typeof payload !== 'string' || payload.trim().length === 0) {
    return 'empty node -e payload';
  }
  if (payload.includes('..')) {
    return "node -e payload may not contain '..'";
  }
  if (/\\(?:x|u|[0-7])/i.test(payload)) {
    return 'node -e payload may not contain hex, unicode, or octal escapes';
  }
  if (payload.includes('${')) {
    return "node -e payload may not contain '${'";
  }
  if (NODE_EVAL_FORBIDDEN_RE.test(payload)) {
    return 'node -e payload uses disallowed module, eval, process, or destructive fs API';
  }
  if (/(?:require|fs|process|global|globalThis)\s*\[/.test(payload)) {
    return 'node -e payload uses disallowed computed property access';
  }

  for (const match of payload.matchAll(/\brequire\b/g)) {
    const rest = payload.slice(match.index);
    if (!/^require\s*\(\s*(?:'(?:fs|node:fs)'|"(?:fs|node:fs)")\s*\)/.test(rest)) {
      return "node -e require() is strictly limited to 'fs' or 'node:fs'";
    }
  }

  for (const match of payload.matchAll(/\bprocess\b/g)) {
    const rest = payload.slice(match.index);
    if (!/^process\.exit\(\s*[0-9]+\s*\)/.test(rest)) {
      return 'node -e process access is strictly limited to process.exit(<int>)';
    }
  }

  if (/\bwriteFileSync\b/.test(payload)) {
    const hasSafeTarget =
      payload.includes("'src-tauri/target/") || payload.includes('"src-tauri/target/');
    const hasUnsafeAbs = /['"]\/(?:etc|tmp|var|usr|bin|sbin|Users|home)/.test(payload);
    if (!hasSafeTarget || hasUnsafeAbs) {
      return 'node -e writeFileSync must target src-tauri/target/...';
    }
  }

  return null;
}

function validateCargoCommand(args) {
  if (args.length === 0) return 'cargo requires a subcommand (test, build, or check)';
  for (const a of args) {
    if (a.quoted !== null) return 'cargo arguments must be unquoted';
  }
  const sub = args[0].value;
  if (!SAFE_CARGO_SUBCOMMANDS.has(sub)) {
    return `disallowed cargo subcommand '${sub}' (only test, build, check allowed)`;
  }

  let hasManifestPath = false;
  let sawDashDash = false;
  let positionalCount = 0;

  for (let i = 1; i < args.length; i += 1) {
    const tok = args[i].value;
    if (sawDashDash) {
      if (!/^[A-Za-z0-9_:.-]+$/.test(tok) || tok.startsWith('-')) {
        return `disallowed cargo test filter '${tok}'`;
      }
      continue;
    }

    if (tok === '--') {
      if (sub !== 'test') return "'--' filter separator is only allowed for cargo test";
      sawDashDash = true;
      continue;
    }

    if (tok === '--manifest-path') {
      const val = args[i + 1]?.value;
      if (
        !val ||
        hasTraversalOrWeirdSegments(val) ||
        (val !== 'src-tauri/Cargo.toml' && !/^crates\/[A-Za-z0-9_-]+\/Cargo\.toml$/.test(val))
      ) {
        return `disallowed cargo --manifest-path '${val ?? ''}'`;
      }
      hasManifestPath = true;
      i += 1;
      continue;
    }

    if (tok === '--release' || tok === '--locked' || tok === '--offline') {
      continue;
    }

    if (tok === '--lib') {
      if (sub !== 'test' && sub !== 'check') return "'--lib' is only allowed with cargo test or check";
      continue;
    }

    if (tok === '--features') {
      const val = args[i + 1]?.value;
      if (val !== 'desktop') {
        return `disallowed cargo --features '${val ?? ''}' (only 'desktop' allowed)`;
      }
      i += 1;
      continue;
    }

    if (tok === '-p' || tok === '--package') {
      const val = args[i + 1]?.value;
      if (!val || !SAFE_CARGO_PACKAGES.has(val)) {
        return `disallowed cargo package '${val ?? ''}'`;
      }
      i += 1;
      continue;
    }

    if (tok === '--test') {
      if (sub !== 'test') return "'--test' is only allowed with cargo test";
      const val = args[i + 1]?.value;
      if (!val || !/^[A-Za-z0-9_-]+$/.test(val)) {
        return `invalid cargo --test target '${val ?? ''}'`;
      }
      i += 1;
      continue;
    }

    if (tok === '--example') {
      if (sub !== 'build' && sub !== 'check') return "'--example' is only allowed with cargo build or check";
      const val = args[i + 1]?.value;
      if (val !== 'run_e2_brain_experiment') {
        return `disallowed cargo --example '${val ?? ''}'`;
      }
      i += 1;
      continue;
    }

    if (tok === '--bin') {
      if (sub !== 'build' && sub !== 'check') return "'--bin' is only allowed with cargo build or check";
      const val = args[i + 1]?.value;
      if (val !== 'anima-lab') {
        return `disallowed cargo --bin '${val ?? ''}' (only 'anima-lab' allowed)`;
      }
      i += 1;
      continue;
    }

    if (!tok.startsWith('-')) {
      if (sub !== 'test' || positionalCount >= 1 || !/^[A-Za-z0-9_:.-]+$/.test(tok)) {
        return `disallowed cargo positional argument '${tok}'`;
      }
      positionalCount += 1;
      continue;
    }

    return `disallowed cargo flag '${tok}'`;
  }

  if (!hasManifestPath) {
    return 'cargo command must specify --manifest-path';
  }
  return null;
}

function validateNpmCommand(args) {
  if (args.length === 0) return 'npm requires a subcommand';
  for (const a of args) {
    if (a.quoted !== null) return 'npm arguments must be unquoted';
  }
  const sub = args[0].value;
  if (sub === 'ci' || sub === 'install') {
    return args.length === 1 ? null : `npm ${sub} accepts no additional flags or arguments`;
  }
  if (sub === 'run') {
    if (args.length < 2) return 'npm run requires an allowlisted script name';
    const scriptName = args[1].value;
    if (!SAFE_NPM_SCRIPTS.has(scriptName)) {
      return `disallowed npm script '${scriptName}'`;
    }
    if (args.length === 2) return null;
    if (args[2].value !== '--' || args.length < 4) {
      return "extra npm run arguments must follow '-- <src-tauri/target/...>'";
    }
    for (const extra of args.slice(3)) {
      if (!isSafeTargetPath(extra.value)) {
        return `disallowed npm run argument path '${extra.value}'`;
      }
    }
    return null;
  }
  return `disallowed npm subcommand or flag '${sub}'`;
}

function validateRunE2ExampleCommand(args) {
  for (const a of args) {
    if (a.quoted !== null) return 'run_e2_brain_experiment arguments must be unquoted';
  }
  if (args.length === 0) return 'run_e2_brain_experiment requires explicit flags';
  for (let i = 0; i < args.length; i += 1) {
    const tok = args[i].value;
    if (tok === '--help' || tok === '-h' || tok === '--smoke' || tok === '--ensemble' || tok === '--replay') {
      continue;
    }
    if (tok === '--manifest-dir') {
      const val = args[i + 1]?.value;
      if (!val || !isSafeRepoReadPath(val)) {
        return `disallowed --manifest-dir '${val ?? ''}'`;
      }
      i += 1;
      continue;
    }
    if (tok === '--out') {
      const val = args[i + 1]?.value;
      if (!val || !isSafeTargetPath(val)) {
        return `disallowed --out '${val ?? ''}'`;
      }
      i += 1;
      continue;
    }
    if (tok === '--duration-rung') {
      const val = args[i + 1]?.value;
      if (val !== '6000' && val !== '12000' && val !== '18000') {
        return `disallowed --duration-rung '${val ?? ''}'`;
      }
      i += 1;
      continue;
    }
    if (tok === '--replay-seed') {
      const val = args[i + 1]?.value;
      if (!val || !/^[0-9]+$/.test(val)) {
        return `invalid --replay-seed '${val ?? ''}'`;
      }
      i += 1;
      continue;
    }
    if (tok === '--replay-arm') {
      const val = args[i + 1]?.value;
      if (val !== 'control' && val !== 'treatment') {
        return `disallowed --replay-arm '${val ?? ''}'`;
      }
      i += 1;
      continue;
    }
    return `disallowed run_e2_brain_experiment flag or argument '${tok}'`;
  }
  return null;
}

function validateAnimaLabCommand(args) {
  for (const a of args) {
    if (a.quoted !== null) return 'anima-lab arguments must be unquoted';
  }
  if (args.length === 0) return 'anima-lab requires --help or a manifest path';
  if (args.length === 1 && (args[0].value === '--help' || args[0].value === '-h')) {
    return null;
  }

  let idx = 0;
  let sub = null;
  if (SAFE_ANIMA_LAB_SUBCOMMANDS.has(args[0].value)) {
    sub = args[0].value;
    idx = 1;
  }

  let manifestCount = 0;
  let sawHelp = false;
  for (; idx < args.length; idx += 1) {
    const tok = args[idx].value;
    if (tok === '--help' || tok === '-h') {
      sawHelp = true;
      continue;
    }
    if (tok === '--model') {
      const val = args[idx + 1]?.value;
      if (val !== 'reference' && val !== 'live') {
        return `disallowed anima-lab --model '${val ?? ''}'`;
      }
      idx += 1;
      continue;
    }
    if (tok === '--format') {
      if (sub !== 'sweep') return "'--format' is only allowed with anima-lab sweep";
      const val = args[idx + 1]?.value;
      if (val !== 'json' && val !== 'csv') {
        return `disallowed anima-lab --format '${val ?? ''}'`;
      }
      idx += 1;
      continue;
    }
    if (tok === '--threads') {
      if (sub !== 'sweep') return "'--threads' is only allowed with anima-lab sweep";
      const val = args[idx + 1]?.value;
      if (!val || !/^[1-9][0-9]*$/.test(val)) {
        return `invalid anima-lab --threads '${val ?? ''}'`;
      }
      idx += 1;
      continue;
    }
    if (!tok.startsWith('-')) {
      if (manifestCount >= 1 || !isSafeRepoReadPath(tok) || !tok.endsWith('.json')) {
        return `disallowed anima-lab manifest path '${tok}'`;
      }
      manifestCount += 1;
      continue;
    }
    return `disallowed anima-lab flag '${tok}'`;
  }

  if (!sawHelp && manifestCount !== 1) {
    return 'anima-lab requires exactly 1 safe .json manifest path';
  }
  return null;
}

/**
 * Layer 4: Closed command/subcommand/flag/path allowlist validator.
 */
function validateSimpleCommand(rawCmd) {
  let cmd = rawCmd;
  if (cmd.head === 'nice') {
    if (cmd.heredocs.length > 0) return 'nice does not accept heredocs';
    if (
      cmd.args.length < 3 ||
      cmd.args[0].quoted !== null ||
      cmd.args[0].value !== '-n' ||
      cmd.args[1].quoted !== null ||
      !/^(?:[1-9]|1[0-9]|20)$/.test(cmd.args[1].value) ||
      cmd.args[2].quoted !== null ||
      cmd.args[2].value !== 'cargo'
    ) {
      return "nice is only allowed as 'nice -n <1..20> cargo ...'";
    }
    cmd = {
      head: 'cargo',
      headLineIndex: cmd.args[2].lineIndex,
      envAssigns: cmd.envAssigns,
      args: cmd.args.slice(3),
      redirs: cmd.redirs,
      heredocs: cmd.heredocs,
    };
  }

  if (cmd.envAssigns.length > 0 && !ENV_ALLOWED_HEADS.has(cmd.head)) {
    return `environment prefixes are not allowed on '${cmd.head}'`;
  }

  if (cmd.head !== 'cat' && cmd.heredocs.length > 0) {
    return `heredocs are not allowed on '${cmd.head}'`;
  }

  switch (cmd.head) {
    case 'npm':
      return validateNpmCommand(cmd.args);
    case 'cargo':
      return validateCargoCommand(cmd.args);
    case './src-tauri/target/release/examples/run_e2_brain_experiment':
      return validateRunE2ExampleCommand(cmd.args);
    case './src-tauri/target/release/anima-lab':
      return validateAnimaLabCommand(cmd.args);
    case 'node': {
      if (cmd.args.length === 2 && cmd.args[0].quoted === null && cmd.args[0].value === '-e') {
        return validateSafeNodeEval(cmd.args[1].value);
      }
      if (
        cmd.args.length >= 1 &&
        cmd.args[0].quoted === null &&
        /^scripts\/(?:verify|check)_[A-Za-z0-9_]+\.mjs$/.test(cmd.args[0].value) &&
        cmd.args[0].value !== 'scripts/check_doc_commands.mjs'
      ) {
        for (const extra of cmd.args.slice(1)) {
          if (
            extra.quoted !== null ||
            (!SAFE_NODE_SCRIPT_FLAGS.has(extra.value) && !isSafeTargetPath(extra.value))
          ) {
            return `disallowed node script argument '${extra.value}'`;
          }
        }
        return null;
      }
      return 'node is only allowed with -e <safe_js> or scripts/(verify|check)_*.mjs';
    }
    case 'mkdir': {
      if (cmd.args.length < 2 || cmd.args[0].quoted !== null || cmd.args[0].value !== '-p') {
        return "mkdir must be 'mkdir -p <src-tauri/target/...>'";
      }
      for (const target of cmd.args.slice(1)) {
        if (target.quoted !== null || !isSafeTargetPath(target.value)) {
          return `disallowed mkdir target '${target.value}'`;
        }
      }
      return null;
    }
    case 'cp': {
      if (
        cmd.args.length !== 3 ||
        cmd.args[0].quoted !== null ||
        cmd.args[0].value !== '-R' ||
        cmd.args[1].quoted !== null ||
        cmd.args[2].quoted !== null ||
        !isSafeRepoReadPath(cmd.args[1].value) ||
        !isSafeTargetPath(cmd.args[2].value)
      ) {
        return "cp must be 'cp -R <safe_repo_read_path> <src-tauri/target/...>'";
      }
      return null;
    }
    case 'rm': {
      if (
        cmd.args.length !== 2 ||
        cmd.args[0].quoted !== null ||
        !SAFE_RM_FLAGS.has(cmd.args[0].value) ||
        cmd.args[1].quoted !== null
      ) {
        return "rm must be 'rm (-rf|-fr|-f|-r) <safe_target>'";
      }
      const target = cmd.args[1].value;
      if (!isSafeTargetPath(target) && !isSafeTempRmTarget(target)) {
        return `disallowed rm target '${target}'`;
      }
      return null;
    }
    case 'cat': {
      if (cmd.heredocs.length === 1 && cmd.args.length === 0) {
        if (cmd.redirs.length === 0) return null;
        if (
          cmd.redirs.length === 1 &&
          cmd.redirs[0].fd === 1 &&
          cmd.redirs[0].op === '>' &&
          isSafeTargetPath(cmd.redirs[0].target)
        ) {
          return null;
        }
        return "cat <<'IDENT' may only redirect stdout '>' to src-tauri/target/...";
      }
      if (
        cmd.heredocs.length === 0 &&
        cmd.args.length === 1 &&
        cmd.args[0].quoted === null &&
        isSafeRepoReadPath(cmd.args[0].value)
      ) {
        return null;
      }
      return "cat must read a safe repo path or write via <<'IDENT' > src-tauri/target/...";
    }
    case 'head': {
      if (
        cmd.args.length !== 3 ||
        cmd.args[0].quoted !== null ||
        cmd.args[0].value !== '-n' ||
        cmd.args[1].quoted !== null ||
        !/^[1-9][0-9]*$/.test(cmd.args[1].value) ||
        cmd.args[2].quoted !== null ||
        !isSafeRepoReadPath(cmd.args[2].value)
      ) {
        return "head must be 'head -n <positive_int> <safe_repo_read_path>'";
      }
      return null;
    }
    case 'grep': {
      let i = 0;
      while (i < cmd.args.length && cmd.args[i].quoted === null && cmd.args[i].value.startsWith('-')) {
        if (!SAFE_GREP_FLAGS.has(cmd.args[i].value)) {
          return `disallowed grep flag '${cmd.args[i].value}'`;
        }
        i += 1;
      }
      if (cmd.args.length - i !== 2) {
        return 'grep must specify 1 pattern and 1 safe repo read path';
      }
      const fileTok = cmd.args[i + 1];
      if (fileTok.quoted !== null || !isSafeRepoReadPath(fileTok.value)) {
        return `disallowed grep file path '${fileTok.value}'`;
      }
      return null;
    }
    case 'touch': {
      if (cmd.args.length !== 1 || cmd.args[0].quoted !== null || !isSafeTargetPath(cmd.args[0].value)) {
        return "touch must target 'src-tauri/target/...'";
      }
      return null;
    }
    case 'echo':
      return null;
    case 'true':
    case 'false':
      return cmd.args.length === 0 ? null : `${cmd.head} takes no arguments`;
    default:
      return `command '${cmd.head}' is not in the strict doc-test allowlist`;
  }
}

/**
 * Validate a single doc-test block's script against the 4-layer strict fail-closed allowlist.
 */
export function validateCommand(script) {
  if (typeof script !== 'string' || script.trim().length === 0) {
    return { ok: false, reason: 'empty command block', lineIndex: 0, lineText: '' };
  }

  const lexed = tokenizeAllowedShell(script);
  if (!lexed.ok) {
    return {
      ok: false,
      reason: lexed.reason,
      lineIndex: lexed.lineIndex,
      lineText: lexed.lineText,
    };
  }

  const parsed = parseAllowedPipelines(lexed.tokens, lexed.lines);
  if (!parsed.ok) {
    return {
      ok: false,
      reason: parsed.reason,
      lineIndex: parsed.lineIndex,
      lineText: parsed.lineText,
    };
  }

  for (const cmd of parsed.commands) {
    const errReason = validateSimpleCommand(cmd);
    if (errReason) {
      return {
        ok: false,
        reason: errReason,
        lineIndex: cmd.headLineIndex,
        lineText: (lexed.lines[cmd.headLineIndex] ?? '').trim(),
      };
    }
  }

  return { ok: true };
}

/**
 * Validate all extracted blocks before any execution starts.
 */
export function validateBlocks(blocks) {
  const errors = [];
  for (const block of blocks) {
    const res = validateCommand(block.script);
    if (!res.ok) {
      const cmdLine = block.contentStartLine + res.lineIndex;
      errors.push({
        file: block.file,
        line: block.fenceLine,
        commandLine: cmdLine,
        reason: res.reason,
        lineText: res.lineText,
        message: `${block.file}:${block.fenceLine} (line ${cmdLine}): ${res.reason} [${res.lineText}]`,
      });
    }
  }
  return errors;
}

export function previewCommand(script) {
  const lines = script
    .replace(/\r\n/g, '\n')
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l.length > 0 && !l.startsWith('#'));
  const first = lines[0] ?? script.trim().split('\n')[0] ?? '';
  return first.length > 100 ? `${first.slice(0, 97)}...` : first;
}

export function runDocCommands(blocks, opts = {}) {
  const cwd = resolve(opts.cwd ?? process.cwd());
  const dryRun = Boolean(opts.dryRun);
  const stdio = opts.stdio ?? ['pipe', 'inherit', 'inherit'];
  const log = opts.log ?? ((msg) => console.log(msg));

  const executed = [];

  for (let idx = 0; idx < blocks.length; idx += 1) {
    const block = blocks[idx];
    const preview = previewCommand(block.script);
    log(`[${idx + 1}/${blocks.length}] ${block.file}:${block.fenceLine} — ${preview}`);

    if (dryRun) {
      executed.push({ file: block.file, fenceLine: block.fenceLine, status: 0, dryRun: true });
      continue;
    }

    const res = spawnSync('bash', ['-eo', 'pipefail', '-s'], {
      cwd,
      input: block.script,
      stdio,
      encoding: 'utf8',
      env: process.env,
    });

    if (res.error || res.signal || res.status !== 0) {
      const exitDesc = res.error
        ? `spawn error: ${res.error.message}`
        : res.signal
          ? `terminated by signal ${res.signal}`
          : `exit ${res.status}`;
      return {
        ok: false,
        failedBlock: block,
        exitStatus: res.status ?? 1,
        message: `FAIL ${block.file}:${block.fenceLine} (marker line ${block.markerLine}, ${exitDesc}): ${preview}`,
        executed,
        stdout: typeof res.stdout === 'string' ? res.stdout : '',
        stderr: typeof res.stderr === 'string' ? res.stderr : '',
      };
    }

    executed.push({ file: block.file, fenceLine: block.fenceLine, status: 0, dryRun: false });
  }

  return { ok: true, executed };
}

export function scanDocDirectories(dirs, cwd) {
  const files = collectMarkdownFiles(dirs, cwd);
  const blocks = [];
  const parseErrors = [];

  for (const f of files) {
    const content = readFileSync(f.absPath, 'utf8');
    const parsed = extractDocCommands(content, f.relPath);
    blocks.push(...parsed.blocks);
    parseErrors.push(...parsed.errors);
  }

  const safetyErrors = validateBlocks(blocks);
  return { files, blocks, parseErrors, safetyErrors };
}

/**
 * Fast in-process parser + strict allowlist self-checks run unconditionally on startup (exits 2 if broken)
 * so the gate cannot pass vacuously.
 */
export function runStartupSelfChecks() {
  const failures = [];

  const sampleMd = [
    '````markdown',
    '<!-- doc-test -->',
    '```bash',
    'echo ignored-inside-fence',
    '```',
    '````',
    '',
    '<!-- doc-test -->',
    '',
    '```bash',
    'echo ok',
    '```',
  ].join('\n');

  const parsed = extractDocCommands(sampleMd, 'inline.md');
  if (parsed.errors.length !== 0 || parsed.blocks.length !== 1 || parsed.blocks[0].script !== 'echo ok') {
    failures.push('parser failed positive control with nested fence and blank line');
  }

  const malformed = [
    '<!-- doc-test -->',
    'prose instead of fence',
    '<!-- doc-test -->',
    '```json',
    '{"a":1}',
    '```',
    '<!-- doc-test -->',
    '```zsh',
    'echo zsh-rejected',
    '```',
    '<!-- doc-test -->',
    '```bash',
    '   ',
    '```',
    '<!-- doc-test -->',
    '```bash',
    'echo unclosed',
  ].join('\n');

  const badParsed = extractDocCommands(malformed, 'bad.md');
  if (badParsed.errors.length !== 5 || badParsed.blocks.length !== 0) {
    failures.push(`parser expected 5 malformed errors, got ${badParsed.errors.length}`);
  }

  const unsafeCmds = [
    'npm run tauri:dev',
    'npm --silent run tauri:dev',
    'npm --prefix . run tauri:dev',
    'npm run -- tauri:dev',
    'npm --tmp /tmp run tauri:dev',
    'npm --tmp=/tmp run tauri:dev',
    'npm run --tmp /tmp tauri:dev',
    'npm exec -- tauri dev',
    'npm x -- tauri build',
    'npm exec -p @tauri-apps/cli -- tauri dev',
    'npm -p run tauri:dev',
    'npm -p exec -- tauri dev',
    'npm -p x -- tauri build',
    "npm exec -c 'cargo run'",
    'npm rum check:docs',
    'npm urn check:docs',
    'npm --silent run check:docs',
    'npm --workspaces run check:docs',
    'npx --yes tauri dev',
    'npx --tmp /tmp tauri dev',
    'cargo run --bin anima-lab',
    'cargo --manifest-path src-tauri/Cargo.toml run --bin anima-lab',
    'cargo -C src-tauri run',
    'cargo build --manifest-path src-tauri/Cargo.toml --bin=anima-engine',
    'cargo build --manifest-path src-tauri/Cargo.toml --bin anima-engine',
    "bash -c 'cargo run'",
    "bash -c 'echo ok'",
    "sh -c 'echo $1' _ ok",
    "zsh -c 'echo ok'",
    'eval "cargo run"',
    'eval "echo ok"',
    'env -u RUST_LOG cargo run',
    'env -S "echo ok"',
    'exec echo ok',
    'builtin echo ok',
    'noglob cargo run',
    'nocorrect cargo run',
    '- cargo run',
    'repeat 1 npm run tauri:dev',
    '() cargo run',
    'for x (x) cargo run',
    'foreach x (x)\ncargo run\nend',
    'foo bar() cargo run\nfoo',
    'case x { (x) cargo run }',
    '{ echo ok; echo done; }',
    'if true; then echo ok; fi',
    'function safe_fn { echo ok; }; safe_fn',
    'for f in docs/*.md; do echo "$f"; done',
    'coproc MYPROC { echo ok; }',
    'echo $(( 1 << 2 ))\ncargo run',
    '(( 1 << 2 ))\ncargo run',
    'a=$(( 1 << 0 ))\ncargo run',
    '(( x <<= 1 ))\ncargo run',
    'for (( i=0; i<1; i++ )); do cargo run; done',
    '< $(echo ok) case x in x) cargo run ;; esac',
    '</dev/null</dev/null case x in x) cargo run ;; esac',
    "(\necho $(cat <<'EOF'\nhello\nEOF)\ncargo run --bin anima-lab\n)",
    "echo $(cat <<'EOF'\nhello\nEOF); cargo run --bin anima-lab",
    'cat <<EOF\n`echo "; cargo run; echo "`\nEOF',
    "cat <<EOF\n$(cat <<'INNER'\n)\nINNER\ncargo run\n)\nEOF",
    'cat <<EOF-1 > src-tauri/target/n.txt\ncargo run\nEOF-1\necho ok',
    'cat <<-EOF > src-tauri/target/n2.txt\n\tcargo run\n\tEOF\necho ok',
    'cat <<"EOF"\n$(cargo run)\nEOF',
    'cat <<\\EOF\n$(cargo run)\nEOF',
    'echo "$(echo ok)"',
    'echo $(echo ok)',
    'echo "${HOME:-/tmp}"',
    'ls file.{a,b}',
    '> /dev/null',
    'cat < in.txt',
    'echo ok >| src-tauri/target/x.txt',
    'echo ok 2>&1',
    'echo hi | xargs echo',
    "find . -name '*.md' -exec echo {} \\;",
    "trap 'rm -f /tmp/safe.lock' EXIT",
    "echo $'hello\\nworld'",
    'RUST_LOG=info echo "$VAR"',
    'vite build',
    'tauri info',
    './src-tauri/target/release/anima-engine',
    'rm -rf /',
    'rm -rf $HOME',
    'rm -rf .',
    'rm -rf /tmp/safe',
    'rm -rf /tmp/safe*',
    'car\\\ngo run --bin anima-lab',
    'car"go" run',
    'echo "\ncargo run\n"',
    'cat <<\'EOF\' > package.json\n{}\nEOF',
    'node -e "require(\'child_process\').execSync(\'cargo run\')"',
    'node -e "require(\'fs\').rmSync(\'src-tauri/src\', { recursive: true })"',
    'node -e "process[\'mainModule\'].require(\'child_process\')"',
    'node -e "require(\'fs\').writeFileSync(\'package.json\', \'{}\')"',
    'node -e "eval(\'1+1\')"',
  ];
  for (const cmd of unsafeCmds) {
    if (validateCommand(cmd).ok) {
      failures.push(`safety check failed to reject '${cmd}'`);
    }
  }

  const safeCmds = [
    'npm ci',
    'npm run check:docs',
    'npm run check:e2 -- src-tauri/target/tutorial-e2-smoke',
    'cargo test --manifest-path src-tauri/Cargo.toml --test e2_seam_tests',
    'cargo test --manifest-path src-tauri/Cargo.toml --lib core::experiment::tests::observable',
    'cargo test --manifest-path src-tauri/Cargo.toml -p anima-domain',
    'cargo build --manifest-path src-tauri/Cargo.toml --release --bin anima-lab',
    'cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop --example run_e2_brain_experiment',
    'CARGO_BUILD_JOBS=4 nice -n 10 cargo test --manifest-path src-tauri/Cargo.toml --test e2_seam_tests',
    'RUST_LOG=info cargo test --manifest-path src-tauri/Cargo.toml --test e2_seam_tests',
    './src-tauri/target/release/examples/run_e2_brain_experiment --smoke \\\n  --manifest-dir src-tauri/tests/fixtures/experiments_e2 \\\n  --out src-tauri/target/tutorial-e2-smoke',
    './src-tauri/target/release/anima-lab --help',
    './src-tauri/target/release/anima-lab ensemble --model reference \\\n  src-tauri/tests/fixtures/experiments/mana-patchy-renewable.json \\\n  > src-tauri/target/doc2-runs/mana.json',
    './src-tauri/target/release/anima-lab sweep --format csv --threads 2 \\\n  src-tauri/target/doc2-runs/sweep-sobol.json \\\n  > src-tauri/target/doc2-runs/sweep.csv',
    '! ./src-tauri/target/release/anima-lab \\\n  src-tauri/tests/fixtures/experiments/invalid-negative-source.json \\\n  > src-tauri/target/doc2-runs/err-out.txt \\\n  2> src-tauri/target/doc2-runs/err.txt',
    'rm -rf src-tauri/target/doc2-gate-neg\nmkdir -p src-tauri/target/doc2-gate-neg',
    'rm -rf /tmp/anima-doc-commands-safe-dir',
    'cp -R artifacts/experiments/e2-evolved-brain-default/smoke src-tauri/target/doc2-gate-neg/smoke',
    "cat <<'EOF' > src-tauri/target/m.json\n{\"note\": \"do not cargo run or npm run tauri:dev\"}\nEOF\n# cargo run in comment",
    "cat <<'EOF'\ncargo run\nEOF\necho ok",
    "cat <<'EOF'\n$(cargo run)\nEOF",
    'cat src-tauri/target/tutorial-e2-smoke/smoke/summary.md',
    'head -n 5 src-tauri/target/doc2-runs/sweep.csv',
    'grep -q "exotic energy source_rate must be >= 0" src-tauri/target/doc2-runs/err.txt',
    '! node scripts/verify_e2_artifacts.mjs src-tauri/target/doc2-gate-neg',
    'node scripts/verify_esel1_artifacts.mjs --self-test',
    'node -e "const fs = require(\'fs\'); const p = \'src-tauri/target/doc2-gate-neg/smoke/per-seed-deltas.csv\'; const lines = fs.readFileSync(p, \'utf8\').split(\'\\n\'); lines[1] = lines[1].replace(/[^,]+$/, \'999999.0\'); fs.writeFileSync(p, lines.join(\'\\n\'));"',
  ];
  for (const cmd of safeCmds) {
    const res = validateCommand(cmd);
    if (!res.ok) {
      failures.push(`safety check falsely rejected safe command (${cmd}): ${res.reason}`);
    }
  }

  return failures;
}

/**
 * Comprehensive built-in positive & negative controls in an isolated temp dir.
 * Exits 0 on pass, 2 on control failure.
 */
export function selfTest(opts = {}) {
  const log = opts.log ?? ((msg) => console.log(msg));
  const startupFailures = runStartupSelfChecks();
  if (startupFailures.length > 0) {
    return { ok: false, failures: startupFailures };
  }

  const failures = [];
  const tempRoot = mkdtempSync(join(tmpdir(), 'anima-doc-commands-selftest-'));

  try {
    const tutDir = join(tempRoot, 'tutorials');
    const howToDir = join(tempRoot, 'how-to');
    const workDir = join(tempRoot, 'work');
    mkdirSync(tutDir, { recursive: true });
    mkdirSync(howToDir, { recursive: true });
    mkdirSync(join(workDir, 'src-tauri', 'target'), { recursive: true });

    writeFileSync(
      join(tutDir, '01-start.md'),
      [
        '# Tutorial',
        '',
        '```bash',
        'echo unmarked-ignored >> src-tauri/target/order.log',
        '```',
        '',
        '<!-- doc-test -->',
        '```bash',
        'echo "tut-1" > src-tauri/target/order.log',
        '```',
        '',
        '<!-- doc-test -->',
        '',
        '```sh',
        'echo "tut-2" >> src-tauri/target/order.log',
        '```',
      ].join('\n'),
      'utf8',
    );

    writeFileSync(
      join(howToDir, '01-a.md'),
      [
        '# How-to A',
        '<!-- doc-test -->',
        '```bash',
        'echo "howto-a" >> src-tauri/target/order.log',
        '```',
      ].join('\n'),
      'utf8',
    );

    writeFileSync(
      join(howToDir, '02-b.md'),
      [
        '# How-to B',
        '<!-- doc-test -->',
        '```',
        "cat <<'EOF' > src-tauri/target/payload.json",
        '{"forbidden_text": "cargo run && npm run tauri:dev"}',
        'EOF',
        'rm -rf /tmp/anima-selftest-nonexistent-subdir',
        'echo "howto-b" >> src-tauri/target/order.log',
        '```',
      ].join('\n'),
      'utf8',
    );

    const scan = scanDocDirectories([tempRoot], workDir);
    if (scan.parseErrors.length !== 0) {
      failures.push(`unexpected parse errors: ${scan.parseErrors.map((e) => e.message).join('; ')}`);
    } else if (scan.safetyErrors.length !== 0) {
      failures.push(`unexpected safety errors: ${scan.safetyErrors.map((e) => e.message).join('; ')}`);
    } else if (scan.blocks.length !== 4) {
      failures.push(`expected 4 extracted blocks, got ${scan.blocks.length}`);
    } else {
      const dryRes = runDocCommands(scan.blocks, {
        cwd: workDir,
        dryRun: true,
        log: () => {},
      });
      if (!dryRes.ok || existsSync(join(workDir, 'src-tauri', 'target', 'order.log'))) {
        failures.push('dry-run executed commands or failed');
      }

      const runRes = runDocCommands(scan.blocks, {
        cwd: workDir,
        stdio: ['pipe', 'pipe', 'pipe'],
        log: () => {},
      });
      if (!runRes.ok) {
        failures.push(`positive execution control failed: ${runRes.message}`);
      } else {
        const orderLines = readFileSync(join(workDir, 'src-tauri', 'target', 'order.log'), 'utf8')
          .trim()
          .split('\n');
        const expected = ['tut-1', 'tut-2', 'howto-a', 'howto-b'];
        if (JSON.stringify(orderLines) !== JSON.stringify(expected)) {
          failures.push(`ordering mismatch: expected ${JSON.stringify(expected)}, got ${JSON.stringify(orderLines)}`);
        }
      }
    }

    // Negative control: fail-fast on block 2 must report exact doc:line and NOT execute block 3
    const negDir = join(tempRoot, 'neg-exec');
    const negWork = join(tempRoot, 'neg-work');
    mkdirSync(negDir, { recursive: true });
    mkdirSync(join(negWork, 'src-tauri', 'target'), { recursive: true });

    writeFileSync(
      join(negDir, 'fail.md'),
      [
        '# Fail fast test',
        '<!-- doc-test -->',
        '```bash',
        'touch src-tauri/target/step1.ok',
        '```',
        '<!-- doc-test -->',
        '```bash',
        'node -e "process.exit(7)"',
        '```',
        '<!-- doc-test -->',
        '```bash',
        'touch src-tauri/target/step3.should_not_exist',
        '```',
      ].join('\n'),
      'utf8',
    );

    const negScan = scanDocDirectories([negDir], negWork);
    if (negScan.blocks.length !== 3) {
      failures.push(`negative control expected 3 blocks, got ${negScan.blocks.length}`);
    } else if (negScan.safetyErrors.length !== 0) {
      failures.push(`negative control unexpected safety error: ${negScan.safetyErrors.map((e) => e.message).join('; ')}`);
    } else {
      const negRes = runDocCommands(negScan.blocks, {
        cwd: negWork,
        stdio: ['pipe', 'pipe', 'pipe'],
        log: () => {},
      });
      if (negRes.ok) {
        failures.push('fail-fast negative control unexpectedly succeeded');
      } else {
        if (!negRes.message.includes('fail.md:7')) {
          failures.push(`fail-fast message missing 'fail.md:7': ${negRes.message}`);
        }
        if (!existsSync(join(negWork, 'src-tauri', 'target', 'step1.ok'))) {
          failures.push('fail-fast control did not run block 1');
        }
        if (existsSync(join(negWork, 'src-tauri', 'target', 'step3.should_not_exist'))) {
          failures.push('fail-fast control executed block 3 after block 2 failed');
        }
      }
    }

    // Negative control: dangling/unclosed/non-shell/zsh/empty markers
    const badDir = join(tempRoot, 'neg-parse');
    mkdirSync(badDir, { recursive: true });
    writeFileSync(join(badDir, 'dangling.md'), '<!-- doc-test -->\n', 'utf8');
    writeFileSync(join(badDir, 'nonshell.md'), '<!-- doc-test -->\n```text\nhi\n```\n', 'utf8');
    writeFileSync(join(badDir, 'zsh.md'), '<!-- doc-test -->\n```zsh\necho hi\n```\n', 'utf8');
    writeFileSync(join(badDir, 'empty.md'), '<!-- doc-test -->\n```bash\n\n```\n', 'utf8');
    writeFileSync(join(badDir, 'unclosed.md'), '<!-- doc-test -->\n```bash\necho hi\n', 'utf8');
    writeFileSync(join(badDir, 'unsafe.md'), '<!-- doc-test -->\n```bash\ncargo run\n```\n', 'utf8');

    const badScan = scanDocDirectories([badDir], negWork);
    if (badScan.parseErrors.length !== 5) {
      failures.push(`parse negative control expected 5 errors, got ${badScan.parseErrors.length}`);
    }
    if (badScan.safetyErrors.length !== 1 || !badScan.safetyErrors[0].message.includes('unsafe.md:2')) {
      failures.push(`safety negative control expected unsafe.md:2 error, got ${badScan.safetyErrors.length}`);
    }
  } finally {
    rmSync(tempRoot, { recursive: true, force: true });
  }

  if (failures.length === 0) {
    log('doc-commands self-test: all positive and negative controls passed');
  }
  return { ok: failures.length === 0, failures };
}

function printHelp() {
  console.log(
    [
      'Usage: node scripts/check_doc_commands.mjs [options]',
      '',
      'Options:',
      '  --dir <path>       Scan directory (repeatable; resolves tutorials/ + how-to/,',
      '                     docs/tutorials/ + docs/how-to/, or recursive *.md).',
      '                     Defaults to docs/tutorials and docs/how-to under --cwd.',
      '  --cwd <path>       Working directory for executing commands (default: cwd).',
      '  --dry-run          Parse and validate commands without executing them.',
      '  --require-blocks   Fail (exit 1) if zero <!-- doc-test --> blocks are found.',
      '  --self-test        Run built-in positive and negative controls (exit 0 or 2).',
      '  -h, --help         Show this help text.',
    ].join('\n'),
  );
}

export function main(argv = process.argv.slice(2)) {
  let args;
  try {
    args = parseArgs(argv);
  } catch (err) {
    console.error(`doc-commands: ${err instanceof Error ? err.message : String(err)}`);
    process.exit(1);
  }

  if (args.help) {
    printHelp();
    process.exit(0);
  }

  if (args.selfTest) {
    const st = selfTest();
    if (!st.ok) {
      for (const f of st.failures) {
        console.error(`doc-commands self-test FAIL: ${f}`);
      }
      process.exit(2);
    }
    process.exit(0);
  }

  const startupFailures = runStartupSelfChecks();
  if (startupFailures.length > 0) {
    console.error('doc-commands: internal self-check failed; refusing to trust gate');
    for (const f of startupFailures) console.error(`  - ${f}`);
    process.exit(2);
  }

  const cwd = args.cwd ? resolve(args.cwd) : process.cwd();
  if (!isExistingDir(cwd)) {
    console.error(`doc-commands: --cwd path does not exist or is not a directory: ${cwd}`);
    process.exit(1);
  }

  let scan;
  try {
    scan = scanDocDirectories(args.dirs, cwd);
  } catch (err) {
    console.error(`doc-commands: ${err instanceof Error ? err.message : String(err)}`);
    process.exit(1);
  }

  if (scan.parseErrors.length > 0) {
    for (const err of scan.parseErrors) {
      console.error(`FAIL ${err.message}`);
    }
    process.exit(1);
  }

  if (scan.safetyErrors.length > 0) {
    for (const err of scan.safetyErrors) {
      console.error(`FAIL unsafe command at ${err.message}`);
    }
    process.exit(1);
  }

  if (scan.blocks.length === 0) {
    const msg = `doc-commands: 0 <!-- doc-test --> block(s) found across ${scan.files.length} markdown file(s)`;
    if (args.requireBlocks) {
      console.error(`FAIL ${msg} (--require-blocks is set)`);
      process.exit(1);
    }
    console.log(`note  ${msg}; nothing to execute (pass --require-blocks to fail when empty)`);
    process.exit(0);
  }

  const runResult = runDocCommands(scan.blocks, {
    cwd,
    dryRun: args.dryRun,
  });

  if (!runResult.ok) {
    console.error(runResult.message);
    process.exit(1);
  }

  const mode = args.dryRun ? 'validated (dry-run)' : 'passed';
  console.log(
    `doc-commands: ${scan.blocks.length} block(s) across ${scan.files.length} markdown file(s) ${mode}`,
  );
  process.exit(0);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main();
}
