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
const ALLOWED_SHELL_LANGS = new Set(['', 'bash', 'sh', 'zsh']);
const FORBIDDEN_NPM_SCRIPTS = new Set(['tauri:dev', 'tauri:build', 'tauri', 'dev', 'preview']);
const WRAPPER_COMMANDS = new Set([
  'env',
  'command',
  'exec',
  'builtin',
  'nohup',
  'time',
  'nice',
  'timeout',
]);
const MAX_SHELL_RECURSION_DEPTH = 5;
const ENV_VALUE_FLAGS = new Set(['-u', '--unset', '-C', '--chdir', '-P', '--argv0']);
const NICE_VALUE_FLAGS = new Set(['-n', '--adjustment']);
const TIMEOUT_VALUE_FLAGS = new Set(['-k', '--kill-after', '-s', '--signal']);
const SHELL_COMPOUND_PREFIXES = new Set([
  '{',
  '}',
  'then',
  'else',
  'elif',
  'do',
  'if',
  'while',
  'until',
  'fi',
  'done',
]);
const FD_PREFIX_PATTERN = '(?:\\d+|\\{[A-Za-z_][A-Za-z0-9_]*\\})';
const STANDALONE_REDIR_RE = new RegExp(
  `^(?:${FD_PREFIX_PATTERN})?(?:>>|>\\||>|<>|<<<|<<-|<<|<|&>>|&>|>&|<&)$`,
);
const ATTACHED_REDIR_RE = new RegExp(
  `^(?:(?:${FD_PREFIX_PATTERN})?(?:>>|>\\||>|<>|<<<|<<-|<<|<|>&|<&)|&>>|&>).+$`,
);
const INPUT_REDIR_OP_RE = new RegExp(`^(?:${FD_PREFIX_PATTERN})?(?:<<<|<<-|<<|<&|<>|<)$`);
const INPUT_REDIR_ATTACHED_RE = new RegExp(`^(?:${FD_PREFIX_PATTERN})?(?:<<<|<<-|<<|<&|<>|<)`);
const FD_SCRIPT_PATH_RE = /^(?:\/dev\/fd\/|\/proc\/(?:self|\d+)\/fd\/|\/dev\/stdin$|[<>]\()/;
const IMPLICIT_FOR_LOOP_RE = /\bfor\s+[A-Za-z_][A-Za-z0-9_]*(?:\s*;|\s*\n)\s*do\b/;
const SAFE_CARGO_SUBCOMMANDS = new Set([
  'build',
  'b',
  'test',
  't',
  'check',
  'c',
  'clippy',
  'fmt',
  'metadata',
  'version',
  'doc',
  'bench',
  'tree',
  'verify-project',
]);
const XARGS_VALUE_FLAGS = new Set([
  '-a',
  '--arg-file',
  '-d',
  '--delimiter',
  '-E',
  '--eof',
  '-I',
  '-J',
  '-L',
  '--max-lines',
  '-n',
  '--max-args',
  '-P',
  '--max-procs',
  '-R',
  '-s',
  '-S',
  '--max-chars',
  '--process-slot-var',
]);
const XARGS_SHORT_VALUE_CHARS = new Set(['a', 'd', 'E', 'I', 'J', 'L', 'n', 'P', 'R', 's', 'S']);
const SHELL_POSITIONAL_OR_VAR_RE = /\$(?:[0-9@*?#!-]|[A-Za-z_][A-Za-z0-9_]*|\{)/;
const SHELL_POSITIONAL_PARAM_RE = /\$(?:[0-9@*]|\{[0-9@*])/;
const FORBIDDEN_STARTUP_ENV_ASSIGN_RE = /^(?:BASH_ENV|ENV)=.+$/;
const NPM_RUN_SUBCOMMANDS = new Set(['run', 'run-script', 'rum', 'urn']);
const NPM_VALUE_FLAGS = new Set([
  '--prefix',
  '-C',
  '--loglevel',
  '--workspace',
  '-w',
  '--userconfig',
  '--globalconfig',
  '--cache',
  '--cache-max',
  '--cache-min',
  '--registry',
  '--scope',
  '--scopes',
  '--tag',
  '--tag-version-prefix',
  '--script-shell',
  '--shell',
  '--fetch-retries',
  '--fetch-retry-factor',
  '--fetch-retry-maxtimeout',
  '--fetch-retry-mintimeout',
  '--fetch-timeout',
  '--editor',
  '--viewer',
  '--tmp',
  '--location',
  '-L',
  '--local-address',
  '--ca',
  '--cafile',
  '--cert',
  '--key',
  '--proxy',
  '--https-proxy',
  '--noproxy',
  '--otp',
  '--access',
  '--allow-directory',
  '--allow-file',
  '--allow-git',
  '--allow-remote',
  '--allow-scripts',
  '--also',
  '--audit-level',
  '--auth-type',
  '--before',
  '--browser',
  '--cidr',
  '--cpu',
  '--depth',
  '--diff',
  '--diff-dst-prefix',
  '--diff-src-prefix',
  '--diff-unified',
  '--expect-result-count',
  '--expires',
  '--git',
  '--heading',
  '--include',
  '--init-author-email',
  '--init-author-name',
  '--init-author-url',
  '--init-license',
  '--init-module',
  '--init-type',
  '--init-version',
  '--init.author.email',
  '--init.author.name',
  '--init.author.url',
  '--init.license',
  '--init.module',
  '--init.version',
  '--install-strategy',
  '--libc',
  '--lockfile-version',
  '--logs-dir',
  '--logs-max',
  '--maxsockets',
  '--message',
  '-m',
  '--min-release-age',
  '--min-release-age-exclude',
  '--name',
  '--node-gyp',
  '--node-options',
  '--omit',
  '--only',
  '--orgs',
  '--orgs-permission',
  '--os',
  '--pack-destination',
  '--package',
  '--packages',
  '--packages-and-scopes-permission',
  '--preid',
  '--provenance-file',
  '--replace-registry-host',
  '--save-prefix',
  '--sbom-format',
  '--sbom-type',
  '--searchexclude',
  '--searchlimit',
  '--searchopts',
  '--searchstaleness',
  '--token-description',
  '--umask',
  '--user-agent',
  '--which',
  '--_auth',
  '--password',
]);
const NPM_PRE_SUBCMD_VALUE_FLAGS = new Set([
  ...NPM_VALUE_FLAGS,
  '--package',
  '-c',
  '--call',
  '--allow-scripts',
]);
const NPM_EXEC_VALUE_FLAGS = new Set([
  ...NPM_VALUE_FLAGS,
  '--package',
  '-p',
  '-c',
  '--call',
  '--allow-scripts',
]);
const NPX_VALUE_FLAGS = new Set([
  ...NPM_VALUE_FLAGS,
  '--package',
  '-p',
  '-c',
  '--call',
  '--allow-scripts',
]);
const CARGO_GLOBAL_VALUE_FLAGS = new Set([
  '--manifest-path',
  '-C',
  '--config',
  '--target-dir',
  '--lockfile-path',
  '--color',
  '--message-format',
  '-Z',
  '--explain',
]);

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
          message: `${filePath}:${fenceLine}: unsupported fence language '${open.lang}' after <!-- doc-test --> (expected bash, sh, zsh, or untagged)`,
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

function parseHeredocSpec(rest) {
  let idx = 0;
  let stripTabs = false;
  if (rest[idx] === '-') {
    stripTabs = true;
    idx += 1;
  }
  while (idx < rest.length && (rest[idx] === ' ' || rest[idx] === '\t')) {
    idx += 1;
  }
  if (idx >= rest.length) return { spec: null, incomplete: false };

  let delim = '';
  let hasToken = false;
  let quoted = false;
  let q = null;

  while (idx < rest.length) {
    const c = rest[idx];
    if (q === "'") {
      if (c === "'") {
        q = null;
      } else {
        delim += c;
      }
      hasToken = true;
      idx += 1;
      continue;
    }
    if (q === "$'") {
      if (c === '\\' && idx + 1 < rest.length) {
        const decoded = decodeAnsiCEscapeAt(rest, idx);
        delim += decoded.value;
        hasToken = true;
        idx += decoded.advance + 1;
        continue;
      }
      if (c === "'") {
        q = null;
      } else {
        delim += c;
      }
      hasToken = true;
      idx += 1;
      continue;
    }
    if (q === '"') {
      if (c === '\\') {
        if (idx + 1 >= rest.length) {
          return { spec: null, incomplete: true };
        }
        const next = rest[idx + 1];
        if (next === '"' || next === '\\' || next === '$' || next === '`') {
          delim += next;
        } else {
          delim += `\\${next}`;
        }
        hasToken = true;
        idx += 2;
        continue;
      }
      if (c === '"') {
        q = null;
      } else {
        delim += c;
      }
      hasToken = true;
      idx += 1;
      continue;
    }

    if (/[\s;|&<>()]/.test(c)) {
      break;
    }
    if (c === '$' && (rest[idx + 1] === "'" || rest[idx + 1] === '"')) {
      q = rest[idx + 1] === "'" ? "$'" : '"';
      quoted = true;
      hasToken = true;
      idx += 2;
      continue;
    }
    if (c === "'" || c === '"') {
      q = c;
      quoted = true;
      hasToken = true;
      idx += 1;
      continue;
    }
    if (c === '\\') {
      if (idx + 1 >= rest.length) {
        return { spec: null, incomplete: true };
      }
      delim += rest[idx + 1];
      quoted = true;
      hasToken = true;
      idx += 2;
      continue;
    }
    delim += c;
    hasToken = true;
    idx += 1;
  }

  if (!hasToken || q !== null) return { spec: null, incomplete: false };
  return { spec: { delim, stripTabs, quoted, consumed: idx }, incomplete: false };
}

function extractUnquotedHeredocSubstitutions(bodyText, lineStartOffsets, baseLineIndex) {
  const entries = [];
  const resolveLineIndex = (offset) => {
    let resolved = baseLineIndex;
    for (const item of lineStartOffsets) {
      if (item.offset <= offset) {
        resolved = item.lineIndex;
      } else {
        break;
      }
    }
    return resolved;
  };

  for (let i = 0; i < bodyText.length; i += 1) {
    const ch = bodyText[i];
    if (ch === '\\' && i + 1 < bodyText.length) {
      i += 1;
      continue;
    }
    if (ch === '$' && bodyText[i + 1] === '(') {
      const lineIndex = resolveLineIndex(i);
      const endIdx = findClosingCommandSubParen(bodyText, i + 2);
      if (endIdx < 0) {
        entries.push({ lineIndex, text: bodyText.slice(i + 2), unclosed: true });
        break;
      }
      entries.push({
        lineIndex,
        text: bodyText.slice(i + 2, endIdx),
        isHeredocSubScript: true,
      });
      i = endIdx;
      continue;
    }
    if (ch === '`') {
      const lineIndex = resolveLineIndex(i);
      const bt = extractDoubleQuotedBacktick(bodyText, i + 1);
      if (!bt) {
        entries.push({ lineIndex, text: bodyText.slice(i + 1), unclosed: true });
        break;
      }
      entries.push({
        lineIndex,
        text: bt.inner,
        isHeredocSubScript: true,
      });
      i = bt.endIdx;
    }
  }

  return entries;
}

const REDIR_FD_PREFIX_RE = /^(?:\d+|\{[A-Za-z_][A-Za-z0-9_]*\})$/;

function createCmdSubFrame(savedQuote = null) {
  return {
    savedQuote,
    parenDepth: 1,
    caseStack: [],
    atCommandStart: true,
    word: '',
    wordPureUnquoted: true,
    afterTimeKeyword: false,
    afterCoprocKeyword: false,
    expectFuncName: false,
    canOpenFuncParen: false,
    pendingEmptyFuncParenDepth: -1,
    pendingRedirTarget: false,
  };
}

function flushCmdSubWord(frame) {
  if (frame.pendingRedirTarget) {
    if (!frame.word && frame.wordPureUnquoted) {
      return;
    }
    frame.pendingRedirTarget = false;
    frame.word = '';
    frame.wordPureUnquoted = true;
    frame.atCommandStart = true;
    frame.afterTimeKeyword = false;
    frame.afterCoprocKeyword = false;
    frame.expectFuncName = false;
    frame.canOpenFuncParen = false;
    return;
  }

  if (!frame.word) {
    frame.wordPureUnquoted = true;
    return;
  }

  const top = frame.caseStack[frame.caseStack.length - 1];
  const wasAfterTime = frame.afterTimeKeyword;
  const wasAfterCoproc = frame.afterCoprocKeyword;
  const wasExpectFuncName = frame.expectFuncName;
  frame.afterTimeKeyword = false;
  frame.afterCoprocKeyword = false;
  frame.expectFuncName = false;
  frame.canOpenFuncParen = false;

  if (frame.wordPureUnquoted) {
    if (frame.atCommandStart && frame.word === 'case') {
      frame.caseStack.push({
        state: 'WAIT_IN',
        baseDepth: frame.parenDepth,
        patternParen: false,
        atPatternStart: false,
      });
      frame.atCommandStart = false;
    } else if (
      top &&
      top.state === 'WAIT_IN' &&
      frame.parenDepth === top.baseDepth &&
      frame.word === 'in'
    ) {
      top.state = 'PATTERN';
      top.patternParen = false;
      top.atPatternStart = true;
      frame.atCommandStart = false;
    } else if (
      top &&
      frame.parenDepth === top.baseDepth &&
      frame.word === 'esac' &&
      (top.state === 'PATTERN' || (top.state === 'BODY' && frame.atCommandStart))
    ) {
      frame.caseStack.pop();
      frame.atCommandStart = false;
    } else if (wasExpectFuncName) {
      frame.atCommandStart = true;
      frame.canOpenFuncParen = true;
    } else if (frame.atCommandStart && frame.word === 'function') {
      frame.expectFuncName = true;
      frame.atCommandStart = false;
    } else if (frame.atCommandStart && frame.word === 'time') {
      frame.afterTimeKeyword = true;
      frame.atCommandStart = true;
    } else if (wasAfterTime && frame.word === '-p') {
      frame.atCommandStart = true;
    } else if (frame.atCommandStart && frame.word === 'coproc') {
      frame.afterCoprocKeyword = true;
      frame.atCommandStart = true;
    } else if (
      wasAfterCoproc &&
      frame.word !== 'if' &&
      frame.word !== 'while' &&
      frame.word !== 'until' &&
      frame.word !== 'for' &&
      frame.word !== 'select' &&
      frame.word !== '{' &&
      /^[A-Za-z_][A-Za-z0-9_]*$/.test(frame.word)
    ) {
      frame.atCommandStart = true;
    } else if (
      frame.atCommandStart &&
      (frame.word === 'then' ||
        frame.word === 'do' ||
        frame.word === 'else' ||
        frame.word === 'elif' ||
        frame.word === 'if' ||
        frame.word === 'while' ||
        frame.word === 'until' ||
        frame.word === '{' ||
        frame.word === '!')
    ) {
      frame.atCommandStart = true;
    } else {
      if (frame.atCommandStart) {
        frame.canOpenFuncParen = true;
      }
      frame.atCommandStart = false;
    }
  } else {
    frame.atCommandStart = false;
  }
  frame.word = '';
  frame.wordPureUnquoted = true;
}

function stepCmdSubChar(frame, str, idx) {
  const ch = str[idx];

  if (ch === '(') {
    flushCmdSubWord(frame);
    const canBeFuncParen = frame.canOpenFuncParen;
    frame.canOpenFuncParen = false;
    frame.pendingEmptyFuncParenDepth = -1;
    frame.pendingRedirTarget = false;
    const top = frame.caseStack[frame.caseStack.length - 1];
    if (top && top.state === 'PATTERN' && frame.parenDepth === top.baseDepth && top.atPatternStart) {
      top.patternParen = true;
      top.atPatternStart = false;
      frame.parenDepth += 1;
    } else {
      if (top && top.state === 'PATTERN') {
        top.atPatternStart = false;
      }
      frame.parenDepth += 1;
      if (canBeFuncParen) {
        frame.pendingEmptyFuncParenDepth = frame.parenDepth;
      }
      frame.atCommandStart = true;
    }
    return { handled: true, closed: false, nextIdx: idx };
  }

  if (ch === ')') {
    flushCmdSubWord(frame);
    frame.canOpenFuncParen = false;
    frame.pendingRedirTarget = false;
    const top = frame.caseStack[frame.caseStack.length - 1];
    if (top && top.state === 'PATTERN') {
      if (top.patternParen && frame.parenDepth === top.baseDepth + 1) {
        frame.parenDepth -= 1;
        top.patternParen = false;
        top.state = 'BODY';
        frame.atCommandStart = true;
        return { handled: true, closed: false, nextIdx: idx };
      }
      if (!top.patternParen && frame.parenDepth === top.baseDepth) {
        top.state = 'BODY';
        frame.atCommandStart = true;
        return { handled: true, closed: false, nextIdx: idx };
      }
    }
    const isEmptyFuncParen = frame.parenDepth === frame.pendingEmptyFuncParenDepth;
    frame.pendingEmptyFuncParenDepth = -1;
    frame.parenDepth -= 1;
    if (frame.parenDepth === 0) {
      return { handled: true, closed: true, nextIdx: idx };
    }
    frame.atCommandStart = isEmptyFuncParen;
    return { handled: true, closed: false, nextIdx: idx };
  }

  if (ch === ';' || ch === '&' || ch === '|' || ch === '\n') {
    if (ch === '&' && str[idx + 1] === '>') {
      flushCmdSubWord(frame);
      frame.canOpenFuncParen = false;
      frame.pendingEmptyFuncParenDepth = -1;
      if (frame.atCommandStart) {
        frame.pendingRedirTarget = true;
      }
      const nextIdx = str[idx + 2] === '>' ? idx + 2 : idx + 1;
      return { handled: true, closed: false, nextIdx };
    }
    flushCmdSubWord(frame);
    frame.canOpenFuncParen = false;
    frame.pendingEmptyFuncParenDepth = -1;
    frame.pendingRedirTarget = false;
    frame.afterTimeKeyword = false;
    frame.afterCoprocKeyword = false;
    frame.expectFuncName = false;
    const top = frame.caseStack[frame.caseStack.length - 1];
    if (ch === ';' && (str[idx + 1] === ';' || str[idx + 1] === '&' || str[idx + 1] === '|')) {
      const nextIdx = str[idx + 1] === ';' && str[idx + 2] === '&' ? idx + 2 : idx + 1;
      if (top && top.state === 'BODY' && frame.parenDepth === top.baseDepth) {
        top.state = 'PATTERN';
        top.patternParen = false;
        top.atPatternStart = true;
      }
      frame.atCommandStart = false;
      return { handled: true, closed: false, nextIdx };
    }
    if (ch === '|' && top && top.state === 'PATTERN' && frame.parenDepth === top.baseDepth) {
      top.atPatternStart = false;
      return { handled: true, closed: false, nextIdx: idx };
    }
    let nextIdx = idx;
    if (
      (ch === '&' && str[idx + 1] === '&') ||
      (ch === '|' && (str[idx + 1] === '|' || str[idx + 1] === '&'))
    ) {
      nextIdx = idx + 1;
    }
    frame.atCommandStart = true;
    return { handled: true, closed: false, nextIdx };
  }

  if (/\s/.test(ch)) {
    flushCmdSubWord(frame);
    return { handled: true, closed: false, nextIdx: idx };
  }

  if (ch === '<' || ch === '>') {
    frame.pendingEmptyFuncParenDepth = -1;
    const isLeadingRedir =
      frame.atCommandStart &&
      (frame.word === '' || (frame.wordPureUnquoted && REDIR_FD_PREFIX_RE.test(frame.word)));
    if (isLeadingRedir) {
      frame.word = '';
      frame.wordPureUnquoted = true;
      frame.pendingRedirTarget = true;
      frame.canOpenFuncParen = false;
    } else {
      flushCmdSubWord(frame);
    }
    let nextIdx = idx;
    const next = str[idx + 1];
    if (ch === '>') {
      if (next === '>' || next === '|' || next === '&') {
        nextIdx = idx + 1;
      }
    } else {
      const three = str.slice(idx, idx + 3);
      if (three === '<<<' || three === '<<-') {
        nextIdx = idx + 2;
      } else if (next === '<' || next === '>' || next === '&') {
        nextIdx = idx + 1;
      }
    }
    return { handled: true, closed: false, nextIdx };
  }

  frame.canOpenFuncParen = false;
  frame.pendingEmptyFuncParenDepth = -1;
  const top = frame.caseStack[frame.caseStack.length - 1];
  if (top && top.state === 'PATTERN' && frame.parenDepth === top.baseDepth) {
    top.atPatternStart = false;
  }
  frame.word += ch;
  return { handled: true, closed: false, nextIdx: idx };
}

function scanShellLineForCommentAndHeredoc(
  line,
  state,
  prevLineContinuedNoSpace = false,
  skipHeredocs = false,
) {
  let commentStart = line.length;
  const heredocSpecs = [];
  let startIdx = 0;

  if (!skipHeredocs && state.pendingHeredocText !== null) {
    const prevLen = state.pendingHeredocText.length;
    const combined = `${state.pendingHeredocText}${line}`;
    const parsed = parseHeredocSpec(combined);
    if (parsed.incomplete) {
      state.pendingHeredocText = combined.slice(0, -1);
      return {
        cleaned: line,
        hasComment: false,
        heredocSpecs,
      };
    }
    state.pendingHeredocText = null;
    if (parsed.spec) {
      heredocSpecs.push({
        delim: parsed.spec.delim,
        stripTabs: parsed.spec.stripTabs,
        quoted: parsed.spec.quoted,
      });
      startIdx = Math.max(0, parsed.spec.consumed - prevLen);
    }
  }

  for (let i = startIdx; i < line.length; i += 1) {
    const ch = line[i];

    if (state.inBacktick !== false) {
      if (ch === '\\' && i + 1 < line.length) {
        i += 1;
        continue;
      }
      if (ch === '`') {
        state.quote = state.inBacktick.savedQuote;
        state.inBacktick = false;
        const activeFrame = state.cmdSubStack[state.cmdSubStack.length - 1];
        if (activeFrame) {
          activeFrame.wordPureUnquoted = false;
          activeFrame.canOpenFuncParen = false;
          activeFrame.pendingEmptyFuncParenDepth = -1;
          activeFrame.atCommandStart = false;
        }
      }
      continue;
    }

    if (state.quote === "'") {
      if (ch === "'") state.quote = null;
      continue;
    }

    if (state.quote === "$'") {
      if (ch === '\\') {
        i += 1;
        continue;
      }
      if (ch === "'") state.quote = null;
      continue;
    }

    if (state.quote === '"') {
      if (ch === '\\') {
        i += 1;
        continue;
      }
      if (ch === '$' && line[i + 1] === '(') {
        state.cmdSubStack.push(createCmdSubFrame('"'));
        state.quote = null;
        i += 1;
        continue;
      }
      if (ch === '`') {
        state.inBacktick = { savedQuote: '"' };
        state.quote = null;
        continue;
      }
      if (ch === '"') state.quote = null;
      continue;
    }

    const activeFrame = state.cmdSubStack[state.cmdSubStack.length - 1] ?? null;

    if (ch === '\\') {
      if (activeFrame) {
        activeFrame.wordPureUnquoted = false;
        activeFrame.canOpenFuncParen = false;
        activeFrame.pendingEmptyFuncParenDepth = -1;
      }
      i += 1;
      continue;
    }

    if (ch === '$' && line[i + 1] === "'") {
      if (activeFrame) {
        activeFrame.wordPureUnquoted = false;
        activeFrame.canOpenFuncParen = false;
        activeFrame.pendingEmptyFuncParenDepth = -1;
      }
      state.quote = "$'";
      i += 1;
      continue;
    }

    if (ch === '$' && line[i + 1] === '"') {
      if (activeFrame) {
        activeFrame.wordPureUnquoted = false;
        activeFrame.canOpenFuncParen = false;
        activeFrame.pendingEmptyFuncParenDepth = -1;
      }
      state.quote = '"';
      i += 1;
      continue;
    }

    if (ch === "'" || ch === '"') {
      if (activeFrame) {
        activeFrame.wordPureUnquoted = false;
        activeFrame.canOpenFuncParen = false;
        activeFrame.pendingEmptyFuncParenDepth = -1;
      }
      state.quote = ch;
      continue;
    }

    if (
      ch === '#' &&
      ((i === 0 && !prevLineContinuedNoSpace) || (i > 0 && /\s|[;|&(]/.test(line[i - 1])))
    ) {
      if (activeFrame) flushCmdSubWord(activeFrame);
      commentStart = i;
      break;
    }

    if (
      (ch === '$' || ch === '<' || ch === '>') &&
      line[i + 1] === '('
    ) {
      if (activeFrame) {
        flushCmdSubWord(activeFrame);
        activeFrame.canOpenFuncParen = false;
        activeFrame.pendingEmptyFuncParenDepth = -1;
        activeFrame.atCommandStart = false;
      }
      state.cmdSubStack.push(createCmdSubFrame(null));
      i += 1;
      continue;
    }

    if (ch === '`') {
      if (activeFrame) {
        flushCmdSubWord(activeFrame);
        activeFrame.canOpenFuncParen = false;
        activeFrame.pendingEmptyFuncParenDepth = -1;
        activeFrame.atCommandStart = false;
      }
      state.inBacktick = { savedQuote: null };
      continue;
    }

    if (!skipHeredocs && ch === '<' && line[i + 1] === '<' && line[i + 2] !== '<') {
      if (activeFrame) {
        flushCmdSubWord(activeFrame);
        activeFrame.pendingEmptyFuncParenDepth = -1;
        activeFrame.canOpenFuncParen = false;
      }
      const rest = line.slice(i + 2);
      const parsed = parseHeredocSpec(rest);
      if (parsed.incomplete) {
        state.pendingHeredocText = rest.slice(0, -1);
        break;
      }
      if (parsed.spec) {
        heredocSpecs.push({
          delim: parsed.spec.delim,
          stripTabs: parsed.spec.stripTabs,
          quoted: parsed.spec.quoted,
        });
        i += 1 + parsed.spec.consumed;
        continue;
      }
    }

    if (activeFrame) {
      const step = stepCmdSubChar(activeFrame, line, i);
      i = step.nextIdx;
      if (step.closed) {
        const popped = state.cmdSubStack.pop();
        state.quote = popped.savedQuote;
        const parentFrame = state.cmdSubStack[state.cmdSubStack.length - 1];
        if (parentFrame) {
          parentFrame.atCommandStart = false;
        }
      }
    }
  }

  return {
    cleaned: line.slice(0, commentStart),
    hasComment: commentStart < line.length,
    heredocSpecs,
  };
}

/**
 * Strip heredoc bodies and `#` comments outside quotes, returning logical lines with start offsets.
 */
export function stripHeredocsAndComments(script, opts = {}) {
  const skipHeredocs = Boolean(opts?.skipHeredocs);
  const rawLines = script.replace(/\r\n/g, '\n').split('\n');
  const heredocQueue = [];
  const pendingHeredocs = [];
  const state = {
    quote: null,
    cmdSubStack: [],
    inBacktick: false,
    pendingHeredocText: null,
  };

  const logical = [];
  let current = '';
  let startIdx = 0;
  let continuing = false;
  let continueWithNewline = false;
  let prevLineContinuedNoSpace = false;

  const flushUnquotedHeredoc = (active) => {
    if (active.quoted || !active.bodyText) return;
    const subEntries = extractUnquotedHeredocSubstitutions(
      active.bodyText,
      active.lineStartOffsets,
      active.bodyStartLine,
    );
    logical.push(...subEntries);
  };

  for (let i = 0; i < rawLines.length; i += 1) {
    const line = rawLines[i];
    if (heredocQueue.length > 0) {
      const active = heredocQueue[0];
      const candidate = active.stripTabs ? line.replace(/^\t+/, '') : line;
      if (!active.bodyContinued && candidate === active.delim) {
        heredocQueue.shift();
        flushUnquotedHeredoc(active);
      } else if (!active.quoted) {
        if (active.bodyStartLine < 0) {
          active.bodyStartLine = i;
        }
        if (!active.bodyContinued) {
          active.lineStartOffsets.push({ offset: active.bodyText.length, lineIndex: i });
        }
        let trailingBs = 0;
        for (let k = candidate.length - 1; k >= 0 && candidate[k] === '\\'; k -= 1) {
          trailingBs += 1;
        }
        const continued = trailingBs % 2 === 1;
        if (continued) {
          active.bodyText += candidate.slice(0, -1);
          active.bodyContinued = true;
        } else {
          active.bodyText += `${candidate}\n`;
          active.bodyContinued = false;
        }
      }
      if (continuing && continueWithNewline) {
        current += '\n';
      }
      prevLineContinuedNoSpace = false;
      continue;
    }

    const { cleaned, hasComment, heredocSpecs } = scanShellLineForCommentAndHeredoc(
      line,
      state,
      prevLineContinuedNoSpace,
      skipHeredocs,
    );
    if (heredocSpecs.length > 0) {
      pendingHeredocs.push(
        ...heredocSpecs.map((spec) => ({
          ...spec,
          bodyText: '',
          bodyStartLine: -1,
          lineStartOffsets: [],
          bodyContinued: false,
        })),
      );
    }

    let trailingBackslashes = 0;
    if (!hasComment && state.quote !== "'" && state.quote !== "$'") {
      for (let k = cleaned.length - 1; k >= 0 && cleaned[k] === '\\'; k -= 1) {
        trailingBackslashes += 1;
      }
    }
    const continuedByBackslash = trailingBackslashes % 2 === 1;
    const part = continuedByBackslash ? cleaned.slice(0, -1) : cleaned;

    if (!continuing) {
      startIdx = i;
      current = part;
    } else if (continueWithNewline) {
      current += `\n${part}`;
    } else {
      current += part;
    }

    if (continuedByBackslash) {
      continuing = true;
      continueWithNewline = false;
      prevLineContinuedNoSpace = part.length > 0 && !/\s$/.test(part);
      continue;
    }

    prevLineContinuedNoSpace = false;
    const topFrame = state.cmdSubStack[state.cmdSubStack.length - 1];
    if (topFrame && state.quote === null && !state.inBacktick) {
      flushCmdSubWord(topFrame);
      topFrame.atCommandStart = true;
    }

    if (state.quote === null && !state.inBacktick && pendingHeredocs.length > 0) {
      heredocQueue.push(...pendingHeredocs);
      pendingHeredocs.length = 0;
    }

    if (state.quote === null && !state.inBacktick && state.cmdSubStack.length === 0) {
      logical.push({ lineIndex: startIdx, text: current });
      current = '';
      continuing = false;
      continueWithNewline = false;
    } else {
      continuing = true;
      continueWithNewline = true;
    }
  }

  for (const active of heredocQueue) {
    flushUnquotedHeredoc(active);
  }

  const isUnclosed = state.quote !== null || state.inBacktick !== false || state.cmdSubStack.length > 0;
  if (continuing && (current.trim().length > 0 || isUnclosed)) {
    logical.push({ lineIndex: startIdx, text: current, ...(isUnclosed ? { unclosed: true } : {}) });
  } else if (isUnclosed) {
    logical.push({
      lineIndex: Math.max(0, rawLines.length - 1),
      text: '',
      unclosed: true,
    });
  }

  return logical;
}

function findClosingCommandSubParen(str, startIdx) {
  const frame = createCmdSubFrame(null);
  let q = null;
  for (let i = startIdx; i < str.length; i += 1) {
    const ch = str[i];
    if (q === "'") {
      if (ch === "'") q = null;
      continue;
    }
    if (q === "$'") {
      if (ch === '\\' && i + 1 < str.length) {
        i += 1;
        continue;
      }
      if (ch === "'") q = null;
      continue;
    }
    if (q === '"') {
      if (ch === '\\' && i + 1 < str.length) {
        i += 1;
        continue;
      }
      if (ch === '$' && str[i + 1] === '(') {
        const innerEnd = findClosingCommandSubParen(str, i + 2);
        if (innerEnd < 0) return -1;
        i = innerEnd;
        continue;
      }
      if (ch === '`') {
        const bt = extractDoubleQuotedBacktick(str, i + 1);
        if (!bt) return -1;
        i = bt.endIdx;
        continue;
      }
      if (ch === '"') q = null;
      continue;
    }
    if (ch === '\\' && i + 1 < str.length) {
      frame.wordPureUnquoted = false;
      frame.canOpenFuncParen = false;
      frame.pendingEmptyFuncParenDepth = -1;
      i += 1;
      continue;
    }
    if (ch === '$' && str[i + 1] === "'") {
      frame.wordPureUnquoted = false;
      frame.canOpenFuncParen = false;
      frame.pendingEmptyFuncParenDepth = -1;
      q = "$'";
      i += 1;
      continue;
    }
    if (ch === '$' && str[i + 1] === '"') {
      frame.wordPureUnquoted = false;
      frame.canOpenFuncParen = false;
      frame.pendingEmptyFuncParenDepth = -1;
      q = '"';
      i += 1;
      continue;
    }
    if (ch === "'" || ch === '"') {
      frame.wordPureUnquoted = false;
      frame.canOpenFuncParen = false;
      frame.pendingEmptyFuncParenDepth = -1;
      q = ch;
      continue;
    }
    if (ch === '#' && (i === startIdx || /\s|[;|&(]/.test(str[i - 1]))) {
      flushCmdSubWord(frame);
      const nl = str.indexOf('\n', i + 1);
      if (nl < 0) return -1;
      frame.atCommandStart = true;
      i = nl;
      continue;
    }
    if ((ch === '$' || ch === '<' || ch === '>') && str[i + 1] === '(') {
      flushCmdSubWord(frame);
      frame.canOpenFuncParen = false;
      frame.pendingEmptyFuncParenDepth = -1;
      frame.atCommandStart = false;
      const innerEnd = findClosingCommandSubParen(str, i + 2);
      if (innerEnd < 0) return -1;
      i = innerEnd;
      continue;
    }
    if (ch === '`') {
      flushCmdSubWord(frame);
      frame.canOpenFuncParen = false;
      frame.pendingEmptyFuncParenDepth = -1;
      frame.atCommandStart = false;
      const bt = extractUnquotedBacktick(str, i + 1);
      if (!bt) return -1;
      i = bt.endIdx;
      continue;
    }
    const step = stepCmdSubChar(frame, str, i);
    i = step.nextIdx;
    if (step.closed) return i;
  }
  return -1;
}

function extractDoubleQuotedBacktick(str, startIdx) {
  let inner = '';
  for (let i = startIdx; i < str.length; i += 1) {
    const ch = str[i];
    if (ch === '\\' && i + 1 < str.length) {
      const next = str[i + 1];
      if (next === '`' || next === '\\' || next === '$' || next === '"') {
        inner += next;
      } else {
        inner += ch + next;
      }
      i += 1;
      continue;
    }
    if (ch === '`') {
      return { inner, endIdx: i };
    }
    inner += ch;
  }
  return null;
}

function extractUnquotedBacktick(str, startIdx) {
  let inner = '';
  for (let i = startIdx; i < str.length; i += 1) {
    const ch = str[i];
    if (ch === '\\' && i + 1 < str.length) {
      const next = str[i + 1];
      if (next === '`' || next === '\\' || next === '$') {
        inner += next;
      } else {
        inner += ch + next;
      }
      i += 1;
      continue;
    }
    if (ch === '`') {
      return { inner, endIdx: i };
    }
    inner += ch;
  }
  return null;
}

function extractNestedCommandSubSegments(inner, opts = {}) {
  const nested = [];
  const logicalLines = stripHeredocsAndComments(inner, opts);
  for (const entry of logicalLines) {
    if (entry.unclosed) {
      nested.push({ text: entry.text || '', pipedIn: false, endedAtSub: true, unclosedSub: true });
      continue;
    }
    if (!entry.text.trim()) continue;
    if (entry.isHeredocSubScript) {
      nested.push(...extractNestedCommandSubSegments(entry.text, { skipHeredocs: false }));
    } else {
      nested.push(...splitShellSegmentsWithMeta(entry.text));
    }
  }
  return nested;
}

function splitShellSegmentsWithMeta(commandLine) {
  const segments = [];
  let quote = null;
  let current = '';
  let nextPipedIn = false;
  let hasUnclosedSubInCurrent = false;

  const flush = (setNextPipedIn = false, endedAtSub = false, unclosedSub = false) => {
    const isUnclosed = unclosedSub || hasUnclosedSubInCurrent;
    const trimmed = current.trim();
    if (trimmed || isUnclosed) {
      segments.push({
        text: trimmed,
        pipedIn: nextPipedIn,
        endedAtSub: endedAtSub || isUnclosed,
        unclosedSub: isUnclosed,
      });
      nextPipedIn = setNextPipedIn;
    } else if (setNextPipedIn) {
      nextPipedIn = true;
    } else if (endedAtSub) {
      nextPipedIn = false;
    }
    current = '';
    hasUnclosedSubInCurrent = false;
  };

  for (let i = 0; i < commandLine.length; i += 1) {
    const ch = commandLine[i];

    if (quote === "'") {
      current += ch;
      if (ch === "'") quote = null;
      continue;
    }

    if (quote === "$'") {
      if (ch === '\\' && i + 1 < commandLine.length) {
        current += ch + commandLine[i + 1];
        i += 1;
        continue;
      }
      current += ch;
      if (ch === "'") quote = null;
      continue;
    }

    if (quote === '"') {
      if (ch === '\\' && i + 1 < commandLine.length) {
        current += ch + commandLine[i + 1];
        i += 1;
        continue;
      }
      if (ch === '$' && commandLine[i + 1] === '(') {
        const endIdx = findClosingCommandSubParen(commandLine, i + 2);
        if (endIdx >= 0) {
          const inner = commandLine.slice(i + 2, endIdx);
          segments.push(...extractNestedCommandSubSegments(inner, { skipHeredocs: true }));
          current += commandLine.slice(i, endIdx + 1);
          i = endIdx;
          continue;
        }
        hasUnclosedSubInCurrent = true;
        segments.push(
          ...extractNestedCommandSubSegments(commandLine.slice(i + 2), { skipHeredocs: true }),
        );
      }
      if (ch === '`') {
        const bt = extractDoubleQuotedBacktick(commandLine, i + 1);
        if (bt) {
          segments.push(...extractNestedCommandSubSegments(bt.inner, { skipHeredocs: false }));
          current += commandLine.slice(i, bt.endIdx + 1);
          i = bt.endIdx;
          continue;
        }
        hasUnclosedSubInCurrent = true;
        segments.push(
          ...extractNestedCommandSubSegments(commandLine.slice(i + 1), { skipHeredocs: false }),
        );
      }
      current += ch;
      if (ch === '"') quote = null;
      continue;
    }

    if (ch === '\\' && i + 1 < commandLine.length) {
      current += ch + commandLine[i + 1];
      i += 1;
      continue;
    }

    if (ch === '$' && commandLine[i + 1] === "'") {
      quote = "$'";
      current += "$'";
      i += 1;
      continue;
    }

    if (ch === '$' && commandLine[i + 1] === '"') {
      quote = '"';
      current += '$"';
      i += 1;
      continue;
    }

    if (ch === "'" || ch === '"') {
      quote = ch;
      current += ch;
      continue;
    }

    if (ch === '$' && commandLine[i + 1] === '(') {
      const endIdx = findClosingCommandSubParen(commandLine, i + 2);
      if (endIdx >= 0) {
        const inner = commandLine.slice(i + 2, endIdx);
        segments.push(...extractNestedCommandSubSegments(inner, { skipHeredocs: true }));
        current += commandLine.slice(i, endIdx + 1);
        i = endIdx;
        continue;
      }
      segments.push(
        ...extractNestedCommandSubSegments(commandLine.slice(i + 2), { skipHeredocs: true }),
      );
      flush(false, true, true);
      i += 1;
      continue;
    }

    if ((ch === '<' || ch === '>') && commandLine[i + 1] === '(') {
      const endIdx = findClosingCommandSubParen(commandLine, i + 2);
      if (endIdx >= 0) {
        const inner = commandLine.slice(i + 2, endIdx);
        segments.push(...extractNestedCommandSubSegments(inner, { skipHeredocs: true }));
        current += commandLine.slice(i, endIdx + 1);
        i = endIdx;
        continue;
      }
      segments.push(
        ...extractNestedCommandSubSegments(commandLine.slice(i + 2), { skipHeredocs: true }),
      );
      flush(false, true, true);
      i += 1;
      continue;
    }

    if (ch === '`') {
      const bt = extractUnquotedBacktick(commandLine, i + 1);
      if (bt) {
        segments.push(...extractNestedCommandSubSegments(bt.inner, { skipHeredocs: false }));
        current += commandLine.slice(i, bt.endIdx + 1);
        i = bt.endIdx;
        continue;
      }
      segments.push(
        ...extractNestedCommandSubSegments(commandLine.slice(i + 1), { skipHeredocs: false }),
      );
      flush(false, true, true);
      continue;
    }

    if (ch === '\n' || ch === ';' || ch === '(' || ch === ')') {
      flush(false, false);
      continue;
    }

    if ((ch === '&' && commandLine[i + 1] === '&') || (ch === '|' && commandLine[i + 1] === '|')) {
      flush(false, false);
      i += 1;
      continue;
    }

    if (ch === '|' && commandLine[i + 1] === '&') {
      flush(true, false);
      i += 1;
      continue;
    }

    if (ch === '|') {
      if (i > 0 && commandLine[i - 1] === '>') {
        current += ch;
        continue;
      }
      flush(true, false);
      continue;
    }

    if (ch === '&') {
      const isRedirAmp =
        commandLine[i + 1] === '>' ||
        (i > 0 && (commandLine[i - 1] === '>' || commandLine[i - 1] === '<'));
      if (isRedirAmp) {
        current += ch;
        continue;
      }
      flush(false, false);
      continue;
    }

    current += ch;
  }

  flush(false, false, quote !== null);
  return segments;
}

function decodeAnsiCEscapeAt(str, backslashIdx) {
  const next = str[backslashIdx + 1];
  switch (next) {
    case 'a':
      return { value: '\x07', advance: 1 };
    case 'b':
      return { value: '\b', advance: 1 };
    case 'e':
    case 'E':
      return { value: '\x1b', advance: 1 };
    case 'f':
      return { value: '\f', advance: 1 };
    case 'n':
      return { value: '\n', advance: 1 };
    case 'r':
      return { value: '\r', advance: 1 };
    case 't':
      return { value: '\t', advance: 1 };
    case 'v':
      return { value: '\v', advance: 1 };
    case '\\':
      return { value: '\\', advance: 1 };
    case "'":
      return { value: "'", advance: 1 };
    case '"':
      return { value: '"', advance: 1 };
    case '?':
      return { value: '?', advance: 1 };
    case 'x': {
      const m = /^[0-9a-fA-F]{1,2}/.exec(str.slice(backslashIdx + 2));
      if (m) {
        return {
          value: String.fromCharCode(parseInt(m[0], 16)),
          advance: 1 + m[0].length,
        };
      }
      return { value: '\\x', advance: 1 };
    }
    case 'u': {
      const m = /^[0-9a-fA-F]{1,4}/.exec(str.slice(backslashIdx + 2));
      if (m) {
        return {
          value: String.fromCodePoint(parseInt(m[0], 16)),
          advance: 1 + m[0].length,
        };
      }
      return { value: '\\u', advance: 1 };
    }
    case 'U': {
      const m = /^[0-9a-fA-F]{1,8}/.exec(str.slice(backslashIdx + 2));
      if (m) {
        const cp = parseInt(m[0], 16);
        if (cp <= 0x10ffff) {
          return {
            value: String.fromCodePoint(cp),
            advance: 1 + m[0].length,
          };
        }
        return { value: `\\U${m[0]}`, advance: 1 + m[0].length };
      }
      return { value: '\\U', advance: 1 };
    }
    case 'c': {
      if (backslashIdx + 2 < str.length) {
        const ctrlCode = str.charCodeAt(backslashIdx + 2) & 0x1f;
        return { value: String.fromCharCode(ctrlCode), advance: 2 };
      }
      return { value: '\\c', advance: 1 };
    }
    default: {
      if (next >= '0' && next <= '7') {
        const m = /^[0-7]{1,3}/.exec(str.slice(backslashIdx + 1));
        if (m) {
          return {
            value: String.fromCharCode(parseInt(m[0], 8) & 0xff),
            advance: m[0].length,
          };
        }
      }
      return { value: `\\${next}`, advance: 1 };
    }
  }
}

function tokenizeSegmentMeta(segment) {
  const tokens = [];
  let current = '';
  let quote = null;
  let hadToken = false;
  let currentHasQuoteOrEscape = false;
  let currentHasUnquotedBrace = false;
  let currentHasUnquotedGlob = false;
  let braceStack = [];
  let inBracketClass = false;
  let bracketContentLen = 0;
  let justSawUnquotedDollar = false;

  const pushCurrent = () => {
    if (hadToken || current.length > 0) {
      tokens.push({
        text: current,
        isRedirOp: false,
        hasUnquotedBrace: currentHasUnquotedBrace,
        hasUnquotedGlob: currentHasUnquotedGlob,
      });
    }
    current = '';
    hadToken = false;
    currentHasQuoteOrEscape = false;
    currentHasUnquotedBrace = false;
    currentHasUnquotedGlob = false;
    braceStack = [];
    inBracketClass = false;
    bracketContentLen = 0;
    justSawUnquotedDollar = false;
  };

  for (let i = 0; i < segment.length; i += 1) {
    const ch = segment[i];
    if (quote === "'") {
      if (ch === "'") {
        quote = null;
      } else {
        current += ch;
      }
      hadToken = true;
      currentHasQuoteOrEscape = true;
      continue;
    }

    if (quote === "$'") {
      if (ch === '\\' && i + 1 < segment.length) {
        const decoded = decodeAnsiCEscapeAt(segment, i);
        current += decoded.value;
        i += decoded.advance;
      } else if (ch === "'") {
        quote = null;
      } else {
        current += ch;
      }
      hadToken = true;
      currentHasQuoteOrEscape = true;
      continue;
    }

    if (quote === '"') {
      if (ch === '\\' && i + 1 < segment.length) {
        current += segment[i + 1];
        i += 1;
      } else if (ch === '$' && segment[i + 1] === '(') {
        const endIdx = findClosingCommandSubParen(segment, i + 2);
        if (endIdx >= 0) {
          current += segment.slice(i, endIdx + 1);
          i = endIdx;
        } else {
          current += ch;
        }
      } else if (ch === '`') {
        const bt = extractDoubleQuotedBacktick(segment, i + 1);
        if (bt) {
          current += segment.slice(i, bt.endIdx + 1);
          i = bt.endIdx;
        } else {
          current += ch;
        }
      } else if (ch === '"') {
        quote = null;
      } else {
        current += ch;
      }
      hadToken = true;
      currentHasQuoteOrEscape = true;
      continue;
    }

    if (ch === '\\' && i + 1 < segment.length) {
      justSawUnquotedDollar = false;
      if (inBracketClass) bracketContentLen += 1;
      current += segment[i + 1];
      i += 1;
      hadToken = true;
      currentHasQuoteOrEscape = true;
      continue;
    }

    if (ch === '$' && segment[i + 1] === "'") {
      justSawUnquotedDollar = false;
      if (inBracketClass) bracketContentLen += 1;
      quote = "$'";
      hadToken = true;
      currentHasQuoteOrEscape = true;
      i += 1;
      continue;
    }

    if (ch === '$' && segment[i + 1] === '"') {
      justSawUnquotedDollar = false;
      if (inBracketClass) bracketContentLen += 1;
      quote = '"';
      hadToken = true;
      currentHasQuoteOrEscape = true;
      i += 1;
      continue;
    }

    if (ch === "'" || ch === '"') {
      justSawUnquotedDollar = false;
      if (inBracketClass) bracketContentLen += 1;
      quote = ch;
      hadToken = true;
      currentHasQuoteOrEscape = true;
      continue;
    }

    if (ch === '&' && segment[i + 1] === '>') {
      if (hadToken || current.length > 0) {
        pushCurrent();
      }
      const op = segment[i + 2] === '>' ? '&>>' : '&>';
      tokens.push({ text: op, isRedirOp: true, hasUnquotedBrace: false, hasUnquotedGlob: false });
      i += op.length - 1;
      continue;
    }

    if ((ch === '>' || ch === '<' || ch === '$') && segment[i + 1] === '(') {
      justSawUnquotedDollar = false;
      if (inBracketClass) bracketContentLen += 1;
      const endIdx = findClosingCommandSubParen(segment, i + 2);
      if (endIdx >= 0) {
        current += segment.slice(i, endIdx + 1);
        hadToken = true;
        currentHasQuoteOrEscape = true;
        i = endIdx;
        continue;
      }
    }

    if (ch === '`') {
      justSawUnquotedDollar = false;
      if (inBracketClass) bracketContentLen += 1;
      const bt = extractUnquotedBacktick(segment, i + 1);
      if (bt) {
        current += segment.slice(i, bt.endIdx + 1);
        hadToken = true;
        currentHasQuoteOrEscape = true;
        i = bt.endIdx;
        continue;
      }
    }

    if ((ch === '>' || ch === '<') && segment[i + 1] !== '(') {
      let fdPrefix = '';
      if (!currentHasQuoteOrEscape && current.length > 0 && REDIR_FD_PREFIX_RE.test(current)) {
        fdPrefix = current;
        current = '';
        hadToken = false;
        currentHasUnquotedBrace = false;
        braceStack = [];
      } else if (hadToken || current.length > 0) {
        pushCurrent();
      }

      let op = ch;
      if (ch === '>') {
        const next = segment[i + 1];
        if (next === '>' || next === '|' || next === '&') {
          op = `>${next}`;
        }
      } else {
        const three = segment.slice(i, i + 3);
        const next = segment[i + 1];
        if (three === '<<<' || three === '<<-') {
          op = three;
        } else if (next === '<' || next === '>' || next === '&') {
          op = `<${next}`;
        }
      }
      tokens.push({
        text: `${fdPrefix}${op}`,
        isRedirOp: true,
        hasUnquotedBrace: false,
        hasUnquotedGlob: false,
      });
      i += op.length - 1;
      continue;
    }

    if (/\s/.test(ch)) {
      pushCurrent();
      continue;
    }

    if (ch === '{') {
      if (!justSawUnquotedDollar) {
        braceStack.push({ hasSep: false });
      }
      justSawUnquotedDollar = false;
      if (inBracketClass) bracketContentLen += 1;
    } else if (ch === ',' && braceStack.length > 0) {
      braceStack[braceStack.length - 1].hasSep = true;
      justSawUnquotedDollar = false;
      if (inBracketClass) bracketContentLen += 1;
    } else if (ch === '.' && segment[i + 1] === '.' && braceStack.length > 0) {
      braceStack[braceStack.length - 1].hasSep = true;
      justSawUnquotedDollar = false;
      if (inBracketClass) bracketContentLen += 1;
    } else if (ch === '}' && braceStack.length > 0) {
      const frame = braceStack.pop();
      if (frame.hasSep) {
        currentHasUnquotedBrace = true;
      }
      justSawUnquotedDollar = false;
      if (inBracketClass) bracketContentLen += 1;
    } else {
      justSawUnquotedDollar = ch === '$';
      if (ch === '*' || ch === '?') {
        currentHasUnquotedGlob = true;
      }
      if (ch === '[' && !inBracketClass) {
        inBracketClass = true;
        bracketContentLen = 0;
      } else if (inBracketClass) {
        if (ch === ']') {
          if (bracketContentLen > 0) {
            currentHasUnquotedGlob = true;
            inBracketClass = false;
            bracketContentLen = 0;
          } else {
            bracketContentLen += 1;
          }
        } else {
          bracketContentLen += 1;
        }
      }
    }

    current += ch;
    hadToken = true;
  }

  pushCurrent();
  return tokens;
}

function stripRedirections(metaTokens) {
  const tokens = [];
  const metas = [];
  const redirs = [];
  for (let i = 0; i < metaTokens.length; i += 1) {
    const item = metaTokens[i];
    if (item.isRedirOp) {
      const next = metaTokens[i + 1];
      if (next && !next.isRedirOp) {
        redirs.push({ op: item.text, target: next.text });
        i += 1;
      } else {
        redirs.push({ op: item.text, target: '' });
      }
      continue;
    }
    tokens.push(item.text);
    metas.push({
      hasUnquotedBrace: Boolean(item.hasUnquotedBrace),
      hasUnquotedGlob: Boolean(item.hasUnquotedGlob),
    });
  }
  return { tokens, metas, redirs };
}

function validateEnvSplitStringPayload(payload, depth, hasRemaining = false) {
  if (payload.trim().length === 0) return null;
  if (hasRemaining) {
    const logicalLines = stripHeredocsAndComments(payload);
    const segments = [];
    for (const entry of logicalLines) {
      if (entry.unclosed) {
        return `forbidden nested 'env -S' command: unclosed quote or command substitution`;
      }
      if (!entry.text.trim()) continue;
      if (entry.isHeredocSubScript) {
        segments.push(...extractNestedCommandSubSegments(entry.text, { skipHeredocs: false }));
      } else {
        segments.push(...splitShellSegmentsWithMeta(entry.text));
      }
    }
    if (segments.length <= 1 && !segments.some((s) => s.unclosedSub)) {
      return null;
    }
  }
  const subRes = validateCommand(payload, depth + 1);
  if (!subRes.ok) {
    return `forbidden nested 'env -S' command: ${subRes.reason}`;
  }
  if (payload.trimStart().startsWith('-')) {
    const envPrefixedRes = validateCommand(`env ${payload}`, depth + 1);
    if (!envPrefixedRes.ok) {
      return `forbidden nested 'env -S' command: ${envPrefixedRes.reason}`;
    }
  }
  return null;
}

function hasDynamicShellExpansion(str) {
  return Boolean(str) && (SHELL_POSITIONAL_OR_VAR_RE.test(str) || /\$\(|`/.test(str));
}

function hasBraceOrGlobMeta(metas, idx) {
  const m = metas?.[idx];
  return Boolean(m && (m.hasUnquotedBrace || m.hasUnquotedGlob));
}

function hasDynamicExpansionAt(tokens, metas, idx) {
  if (idx < 0 || idx >= tokens.length) return false;
  return hasDynamicShellExpansion(tokens[idx]) || hasBraceOrGlobMeta(metas, idx);
}

function stripCommandPrefixes(rawTokens, depth = 0, redirs = [], rawMetas = null) {
  if (depth > MAX_SHELL_RECURSION_DEPTH) {
    return {
      tokens: [],
      metas: [],
      error: `nested shell command recursion depth exceeded (>${MAX_SHELL_RECURSION_DEPTH})`,
    };
  }

  const tokens = [...rawTokens];
  const metas = rawMetas
    ? [...rawMetas]
    : tokens.map(() => ({ hasUnquotedBrace: false, hasUnquotedGlob: false }));
  let idx = 0;
  while (idx < tokens.length) {
    const tok = tokens[idx];
    if (FORBIDDEN_STARTUP_ENV_ASSIGN_RE.test(tok)) {
      return {
        tokens: [],
        metas: [],
        error: `forbidden shell startup file variable assignment '${tok}'`,
      };
    }
    if (tok === 'function' && idx + 1 < tokens.length) {
      idx += 2;
      continue;
    }
    if (tok === 'coproc') {
      if (
        idx + 2 < tokens.length &&
        /^[A-Za-z_][A-Za-z0-9_]*$/.test(tokens[idx + 1]) &&
        (tokens[idx + 2] === '{' || tokens[idx + 2] === '(')
      ) {
        idx += 2;
      } else {
        idx += 1;
      }
      continue;
    }
    if (SHELL_COMPOUND_PREFIXES.has(tok) || tok === '!' || /^[A-Za-z_][A-Za-z0-9_]*=/.test(tok)) {
      idx += 1;
      continue;
    }
    if (STANDALONE_REDIR_RE.test(tok)) {
      idx += 2;
      continue;
    }
    if (ATTACHED_REDIR_RE.test(tok)) {
      idx += 1;
      continue;
    }
    const base = tok.split('/').pop() ?? tok;
    if (WRAPPER_COMMANDS.has(base)) {
      if (
        base === 'exec' &&
        redirs.some((r) => INPUT_REDIR_OP_RE.test(r.op) && !(r.op === '<' && r.target === '/dev/null'))
      ) {
        return {
          tokens: [],
          metas: [],
          error: "forbidden 'exec' with input redirection",
        };
      }
      idx += 1;
      const envSplitItems = [];
      while (idx < tokens.length) {
        if (
          base === 'env' &&
          envSplitItems.some((item) => !item.text.startsWith('-') && !/^[A-Za-z_][A-Za-z0-9_]*=/.test(item.text))
        ) {
          break;
        }
        const flag = tokens[idx];
        if (flag === '--') {
          idx += 1;
          break;
        }
        if (!flag.startsWith('-') || flag === '-') {
          break;
        }
        if (base === 'env') {
          if (flag === '-S' || flag === '--split-string') {
            const payload = tokens[idx + 1] ?? '';
            const err = validateEnvSplitStringPayload(payload, depth, idx + 2 < tokens.length);
            if (err) return { tokens: [], metas: [], error: err };
            envSplitItems.push(...tokenizeSegmentMeta(payload).filter((m) => !m.isRedirOp));
            idx += 2;
            continue;
          }
          if (flag.startsWith('--split-string=')) {
            const payload = flag.slice('--split-string='.length);
            const err = validateEnvSplitStringPayload(payload, depth, idx + 1 < tokens.length);
            if (err) return { tokens: [], metas: [], error: err };
            envSplitItems.push(...tokenizeSegmentMeta(payload).filter((m) => !m.isRedirOp));
            idx += 1;
            continue;
          }
          if (ENV_VALUE_FLAGS.has(flag)) {
            idx += 2;
            continue;
          }
          if (!flag.startsWith('--') && !flag.includes('=') && flag.length > 1) {
            let consumedNext = false;
            for (let j = 1; j < flag.length; j += 1) {
              const ch = flag[j];
              if (ch === 'S') {
                let payload;
                if (j + 1 < flag.length) {
                  payload = flag.slice(j + 1);
                } else {
                  payload = tokens[idx + 1] ?? '';
                  consumedNext = true;
                }
                const err = validateEnvSplitStringPayload(
                  payload,
                  depth,
                  idx + (consumedNext ? 2 : 1) < tokens.length,
                );
                if (err) return { tokens: [], metas: [], error: err };
                envSplitItems.push(...tokenizeSegmentMeta(payload).filter((m) => !m.isRedirOp));
                break;
              }
              if (ch === 'u' || ch === 'C' || ch === 'P') {
                if (j + 1 >= flag.length) {
                  consumedNext = true;
                }
                break;
              }
            }
            idx += consumedNext ? 2 : 1;
            continue;
          }
          idx += 1;
          continue;
        }
        if (base === 'exec') {
          if (flag === '-a') {
            idx += 2;
          } else if (!flag.startsWith('--') && flag.includes('a')) {
            idx += flag.endsWith('a') ? 2 : 1;
          } else {
            idx += 1;
          }
          continue;
        }
        if (
          (base === 'nice' && NICE_VALUE_FLAGS.has(flag)) ||
          (base === 'timeout' && TIMEOUT_VALUE_FLAGS.has(flag))
        ) {
          idx += 2;
        } else {
          idx += 1;
        }
      }
      if (base === 'timeout' && idx < tokens.length) {
        idx += 1;
      }
      if (base === 'env' && envSplitItems.length > 0) {
        const remainingTokens = tokens.slice(idx);
        const remainingMetas = metas.slice(idx);
        const firstNonAssign = envSplitItems.find((item) => !/^[A-Za-z_][A-Za-z0-9_]*=/.test(item.text));
        const prefixEnv = firstNonAssign && firstNonAssign.text.startsWith('-');
        const splicedTokens = [
          ...(prefixEnv ? ['env'] : []),
          ...envSplitItems.map((m) => m.text),
          ...remainingTokens,
        ];
        const splicedMetas = [
          ...(prefixEnv ? [{ hasUnquotedBrace: false, hasUnquotedGlob: false }] : []),
          ...envSplitItems.map((m) => ({
            hasUnquotedBrace: Boolean(m.hasUnquotedBrace),
            hasUnquotedGlob: Boolean(m.hasUnquotedGlob),
          })),
          ...remainingMetas,
        ];
        return stripCommandPrefixes(splicedTokens, depth + 1, redirs, splicedMetas);
      }
      continue;
    }
    break;
  }
  return { tokens: tokens.slice(idx), metas: metas.slice(idx), error: null };
}

function isDangerousRmTarget(rawTarget) {
  if (!rawTarget) return true;
  const trimmed = rawTarget.trim();
  if (hasDynamicShellExpansion(trimmed)) return true;
  if (trimmed === '/' || /^\/+\*?$/.test(trimmed)) return true;
  if (/^~[^/]*(?:\/|$)/.test(trimmed)) return true;
  if (/^\$\{?HOME\}?(?:\/|$)/.test(trimmed)) return true;

  const withoutLeadingDotSlash = trimmed.replace(/^(?:\.\/)+/, '');
  const normalized = (withoutLeadingDotSlash || '.').replace(/\/+$/, '') || '/';

  if (normalized === '.' || normalized === '..' || normalized === '*') return true;
  const parts = normalized.split('/');
  if (parts.includes('..')) return true;

  if (normalized.startsWith('/')) {
    const isSafeTmp =
      (normalized.startsWith('/tmp/') && normalized.length > '/tmp/'.length && normalized !== '/tmp/*') ||
      (normalized.startsWith('/private/tmp/') &&
        normalized.length > '/private/tmp/'.length &&
        normalized !== '/private/tmp/*');
    return !isSafeTmp;
  }

  return false;
}

function hasExplicitSafeCargoSubcommand(cargoTokens) {
  let cargoSub = '';
  let hasInfoFlag = false;
  for (let i = 1; i < cargoTokens.length; i += 1) {
    const ct = cargoTokens[i];
    if (CARGO_GLOBAL_VALUE_FLAGS.has(ct)) {
      i += 1;
      continue;
    }
    if (ct === '-V' || ct === '--version' || ct === '-h' || ct === '--help' || ct === '--list') {
      hasInfoFlag = true;
    }
    if (ct.startsWith('-') || ct.startsWith('+')) {
      continue;
    }
    cargoSub = ct;
    break;
  }
  return hasInfoFlag || SAFE_CARGO_SUBCOMMANDS.has(cargoSub);
}

const NPM_BOOLEAN_CONFIG_FLAGS = new Set([
  '-g',
  '--global',
  '-p',
  '--parseable',
  '-l',
  '--long',
  '-j',
  '--json',
  '-y',
  '--yes',
  '-f',
  '--force',
  '-a',
  '--all',
  '--verbose',
  '--ignore-scripts',
  '--offline',
  '--prefer-offline',
  '--prefer-online',
  '--workspaces',
  '--ws',
  '--include-workspace-root',
  '--if-present',
  '--dry-run',
  '--allow-same-version',
  '--allow-scripts-pending',
  '--allow-scripts-pin',
  '--audit',
  '--bin-links',
  '--bypass-2fa',
  '--commit-hooks',
  '--dangerously-allow-all-scripts',
  '--description',
  '--dev',
  '--diff-ignore-all-space',
  '--diff-name-only',
  '--diff-no-prefix',
  '--diff-text',
  '--engine-strict',
  '--expect-results',
  '--foreground-scripts',
  '--format-package-lock',
  '--fund',
  '--git-tag-version',
  '--global-style',
  '--include-attestations',
  '--include-staged',
  '--init-private',
  '--install-links',
  '--legacy-bundling',
  '--legacy-peer-deps',
  '--link',
  '--omit-lockfile-registry-resolved',
  '--optional',
  '--package-lock',
  '--package-lock-only',
  '--packages-all',
  '--prefer-dedupe',
  '--production',
  '--progress',
  '--provenance',
  '--read-only',
  '--rebuild-bundle',
  '--save',
  '--save-bundle',
  '--save-dev',
  '--save-exact',
  '--save-optional',
  '--save-peer',
  '--save-prod',
  '--shrinkwrap',
  '--sign-git-commit',
  '--sign-git-tag',
  '--strict-allow-scripts',
  '--strict-peer-deps',
  '--strict-ssl',
  '--timing',
  '--unicode',
  '--update-notifier',
  '--workspaces-update',
]);

const NPM_KNOWN_BOOLEAN_FLAGS = new Set([
  '-s',
  '--silent',
  '-q',
  '--quiet',
  '-d',
  '-dd',
  '-ddd',
  '-v',
  '--version',
  '-h',
  '--help',
  '--usage',
  '--versions',
  '--no-audit',
  '--no-fund',
  '--no-save',
  '--no-package-lock',
  '--color',
  '--no-color',
  ...NPM_BOOLEAN_CONFIG_FLAGS,
]);

const NPM_SHORT_BOOL_CHARS = new Set(['s', 'q', 'd', 'g', 'p', 'l', 'j', 'y', 'f', 'v', 'h', 'a']);
const NPM_INFO_BOOL_FLAGS = new Set([
  '-v',
  '--version',
  '-h',
  '--help',
  '--usage',
  '--versions',
]);
const NPM_INFO_BOOL_SHORT_CHARS = new Set(['v', 'h']);
const SAFE_VITE_SUBCOMMANDS = new Set(['build', 'optimize']);
const VITE_VALUE_FLAGS = new Set([
  '--host',
  '--port',
  '--open',
  '-c',
  '--config',
  '--base',
  '-l',
  '--logLevel',
  '--configLoader',
  '--profile',
  '-d',
  '--debug',
  '-f',
  '--filter',
  '-m',
  '--mode',
]);

function resolveTauriSubcommand(tauriTokens) {
  let hasInfoFlag = false;
  let sub = '';
  let subIdx = -1;
  let secondarySub = '';
  let secondarySubIdx = -1;
  let afterDashDash = false;
  for (let i = 1; i < tauriTokens.length; i += 1) {
    const tok = tauriTokens[i];
    if (!afterDashDash && tok === '--') {
      afterDashDash = true;
      continue;
    }
    if (!afterDashDash && (tok === '-h' || tok === '--help' || tok === '-V' || tok === '--version')) {
      hasInfoFlag = true;
      continue;
    }
    if (!afterDashDash && tok.startsWith('-')) {
      continue;
    }
    if (subIdx < 0) {
      sub = tok;
      subIdx = i;
      if (sub !== 'android' && sub !== 'ios') {
        break;
      }
      continue;
    }
    secondarySub = tok;
    secondarySubIdx = i;
    break;
  }
  return { sub, subIdx, secondarySub, secondarySubIdx, hasInfoFlag };
}

function isForbiddenTauriSubcommand(tauriInfo, tokens, metas = null, placeholders = null) {
  if (
    tauriInfo.sub === 'dev' ||
    tauriInfo.sub === 'build' ||
    hasDynamicExpansionAt(tokens, metas, tauriInfo.subIdx) ||
    containsAnyPlaceholder(tauriInfo.sub, placeholders)
  ) {
    return true;
  }
  if (tauriInfo.sub === 'android' || tauriInfo.sub === 'ios') {
    if (
      tauriInfo.secondarySub === 'dev' ||
      tauriInfo.secondarySub === 'build' ||
      tauriInfo.secondarySub === 'run' ||
      hasDynamicExpansionAt(tokens, metas, tauriInfo.secondarySubIdx) ||
      containsAnyPlaceholder(tauriInfo.secondarySub, placeholders)
    ) {
      return true;
    }
  }
  return false;
}

function isSafeViteCommand(viteTokens, viteMetas = null) {
  let hasInfoFlag = false;
  let sub = '';
  for (let i = 1; i < viteTokens.length; i += 1) {
    const tok = viteTokens[i];
    if (hasDynamicExpansionAt(viteTokens, viteMetas, i)) {
      return false;
    }
    if (tok === '--') {
      if (i + 1 < viteTokens.length) {
        const after = viteTokens[i + 1];
        if (hasDynamicExpansionAt(viteTokens, viteMetas, i + 1)) return false;
        if (!sub) sub = after;
      }
      break;
    }
    if (tok === '-h' || tok === '--help' || tok === '-v' || tok === '--version') {
      hasInfoFlag = true;
      continue;
    }
    if (VITE_VALUE_FLAGS.has(tok)) {
      if (i + 1 < viteTokens.length && hasDynamicExpansionAt(viteTokens, viteMetas, i + 1)) {
        return false;
      }
      i += 1;
      continue;
    }
    if (tok.startsWith('-')) {
      continue;
    }
    if (!sub) {
      sub = tok;
    }
  }
  if (hasInfoFlag) return true;
  return SAFE_VITE_SUBCOMMANDS.has(sub);
}

function isKnownNpmBooleanOrShortFlag(tok, valueFlags = NPM_VALUE_FLAGS) {
  if (NPM_KNOWN_BOOLEAN_FLAGS.has(tok)) return true;
  if (tok.startsWith('--no-')) {
    const positive = `--${tok.slice('--no-'.length)}`;
    return NPM_BOOLEAN_CONFIG_FLAGS.has(positive);
  }
  if (/^-[A-Za-z]+$/.test(tok)) {
    return [...tok.slice(1)].every(
      (ch) => NPM_SHORT_BOOL_CHARS.has(ch) && !valueFlags.has(`-${ch}`),
    );
  }
  return false;
}

function parseNpmCallFlag(tokens, idx) {
  const tok = tokens[idx];
  if (tok === '-c' || tok === '--call' || /^-[A-Za-z]*c$/.test(tok)) {
    return { matched: true, payload: tokens[idx + 1] ?? '', step: 2 };
  }
  if (tok.startsWith('--call=') || /^-[A-Za-z]*c=.*$/.test(tok)) {
    return { matched: true, payload: tok.slice(tok.indexOf('=') + 1), step: 1 };
  }
  return { matched: false, payload: '', step: 1 };
}

function consumesNpmFlagValue(tokens, idx, valueFlags) {
  const tok = tokens[idx];
  const next = tokens[idx + 1];
  if (valueFlags.has(tok)) {
    return true;
  }
  if (tok === '--color' || tok === '--no-color') {
    return next === 'always' || next === 'true' || next === 'false';
  }
  if (tok.startsWith('--no-')) {
    return next === 'true' || next === 'false';
  }
  if (NPM_BOOLEAN_CONFIG_FLAGS.has(tok) || NPM_INFO_BOOL_FLAGS.has(tok)) {
    return next === 'true' || next === 'false';
  }
  if (/^-[A-Za-z]{2,}$/.test(tok)) {
    const chars = [...tok.slice(1)];
    const prefix = chars.slice(0, -1);
    const last = chars[chars.length - 1];
    const allPrefixBool = prefix.every(
      (ch) => NPM_SHORT_BOOL_CHARS.has(ch) && !valueFlags.has(`-${ch}`),
    );
    if (allPrefixBool && valueFlags.has(`-${last}`)) {
      return true;
    }
    if (
      (next === 'true' || next === 'false') &&
      chars.some(
        (ch) =>
          !valueFlags.has(`-${ch}`) &&
          (NPM_BOOLEAN_CONFIG_FLAGS.has(`-${ch}`) || NPM_INFO_BOOL_SHORT_CHARS.has(ch)),
      )
    ) {
      return true;
    }
  }
  return false;
}

function containsAnyPlaceholder(str, placeholders) {
  if (!placeholders || placeholders.size === 0 || !str) return false;
  for (const ph of placeholders) {
    if (ph.length > 0 && str.includes(ph)) {
      return true;
    }
  }
  return false;
}

function parseShellInvocationFlags(tokens) {
  let hasCFlag = false;
  let hasSFlag = false;
  let hasInfoFlag = false;
  let hasStdinRedir = false;
  let hasExpandAliasesOpt = false;
  let argIdx = 1;

  while (argIdx < tokens.length) {
    const tok = tokens[argIdx];
    if (tok === '--' || tok === '-') {
      argIdx += 1;
      break;
    }
    if (STANDALONE_REDIR_RE.test(tok)) {
      if (INPUT_REDIR_OP_RE.test(tok)) {
        hasStdinRedir = true;
      }
      argIdx += 2;
      continue;
    }
    if (ATTACHED_REDIR_RE.test(tok)) {
      if (INPUT_REDIR_ATTACHED_RE.test(tok)) {
        hasStdinRedir = true;
      }
      argIdx += 1;
      continue;
    }
    if (tok === '--rcfile' || tok === '--init-file') {
      argIdx += 2;
      continue;
    }
    if (tok === '--version' || tok === '--help') {
      hasInfoFlag = true;
      argIdx += 1;
      continue;
    }
    if (tok.startsWith('--')) {
      argIdx += 1;
      continue;
    }
    if (tok.startsWith('+') && tok.length > 1) {
      let consumeArgs = 0;
      for (const ch of tok.slice(1)) {
        if (ch === 'o' || ch === 'O') {
          consumeArgs += 1;
        }
      }
      argIdx += 1 + consumeArgs;
      continue;
    }
    if (tok.startsWith('-') && tok.length > 1) {
      let consumeArgs = 0;
      let hasBigO = false;
      for (const ch of tok.slice(1)) {
        if (ch === 'c') {
          hasCFlag = true;
        } else if (ch === 's') {
          hasSFlag = true;
        } else if (ch === 'o' || ch === 'O') {
          consumeArgs += 1;
          if (ch === 'O') hasBigO = true;
        }
      }
      if (hasBigO) {
        for (let k = 1; k <= consumeArgs; k += 1) {
          if (tokens[argIdx + k] === 'expand_aliases') {
            hasExpandAliasesOpt = true;
          }
        }
      }
      argIdx += 1 + consumeArgs;
      continue;
    }
    break;
  }

  return { hasCFlag, hasSFlag, hasInfoFlag, hasStdinRedir, hasExpandAliasesOpt, argIdx };
}

function checkDynamicSubcommand(
  subToks,
  placeholders,
  contextLabel,
  isFindContext = false,
  subMetas = null,
) {
  if (subToks.length === 0) return null;
  const subBase = subToks[0].split('/').pop() ?? subToks[0];

  if (!isFindContext && containsAnyPlaceholder(subToks[0], placeholders)) {
    return `forbidden '${contextLabel}' invocation with placeholder command '${subToks[0]}'`;
  }

  if (subBase === 'cargo') {
    if (!hasExplicitSafeCargoSubcommand(subToks)) {
      return `forbidden '${contextLabel} cargo' invocation without an explicit safe subcommand`;
    }
    if (!isFindContext) {
      for (let i = 1; i < subToks.length; i += 1) {
        const ct = subToks[i];
        if (ct === '--') break;
        if (ct.startsWith('--bin=')) {
          const binVal = ct.slice('--bin='.length);
          if (
            !binVal ||
            binVal === 'anima-engine' ||
            binVal === 'anima-engine.exe' ||
            containsAnyPlaceholder(binVal, placeholders) ||
            hasDynamicExpansionAt(subToks, subMetas, i)
          ) {
            return `forbidden '${contextLabel} cargo' invocation with dynamic or disallowed '--bin' target`;
          }
        }
        if (ct === '--bin') {
          const binVal = subToks[i + 1] ?? '';
          if (
            !binVal ||
            binVal === 'anima-engine' ||
            binVal === 'anima-engine.exe' ||
            containsAnyPlaceholder(binVal, placeholders) ||
            hasDynamicExpansionAt(subToks, subMetas, i + 1)
          ) {
            return `forbidden '${contextLabel} cargo' invocation with dynamic or disallowed '--bin' target`;
          }
        }
      }
    }
  }

  if (subBase === 'bash' || subBase === 'sh' || subBase === 'zsh') {
    const parsed = parseShellInvocationFlags(subToks);
    if (parsed.hasCFlag) {
      if (parsed.argIdx >= subToks.length) {
        return `forbidden '${contextLabel} ${subBase} -c' without an explicit inline script`;
      }
      const subScript = subToks[parsed.argIdx];
      if (containsAnyPlaceholder(subScript, placeholders)) {
        return `forbidden '${contextLabel} ${subBase} -c' with dynamic placeholder in inline script`;
      }
      if (SHELL_POSITIONAL_PARAM_RE.test(subScript)) {
        return `forbidden '${contextLabel} ${subBase} -c' with positional parameter expansion in inline script`;
      }
      if (IMPLICIT_FOR_LOOP_RE.test(subScript)) {
        return `forbidden '${contextLabel} ${subBase} -c' with implicit positional 'for' loop in inline script`;
      }
    } else if (!isFindContext && placeholders.size > 0) {
      const rest = subToks.slice(parsed.argIdx);
      if (rest.some((arg) => containsAnyPlaceholder(arg, placeholders))) {
        return `forbidden '${contextLabel} ${subBase}' with dynamic placeholder script argument`;
      }
    }
  }

  if (subBase === 'eval') {
    let idx = 1;
    if (subToks[idx] === '--') idx += 1;
    if (idx >= subToks.length) {
      return `forbidden '${contextLabel} eval' without an explicit static script`;
    }
    if (subToks.slice(idx).some((t) => containsAnyPlaceholder(t, placeholders))) {
      return `forbidden '${contextLabel} eval' with dynamic placeholder`;
    }
  }

  if (isFindContext) {
    return null;
  }

  if (subBase === 'find' && placeholders.size > 0) {
    for (let i = 1; i < subToks.length; i += 1) {
      const ft = subToks[i];
      if (ft === '-exec' || ft === '-execdir' || ft === '-ok' || ft === '-okdir') {
        let j = i + 1;
        while (j < subToks.length && subToks[j] !== ';' && subToks[j] !== '+') {
          j += 1;
        }
        const execSubTokens = subToks.slice(i + 1, j);
        const execSubMetas = subMetas ? subMetas.slice(i + 1, j) : null;
        if (execSubTokens.length > 0) {
          const strippedExec = stripCommandPrefixes(execSubTokens, 0, [], execSubMetas);
          if (strippedExec.error) {
            return `forbidden '${contextLabel} find ${ft}' command: ${strippedExec.error}`;
          }
          if (strippedExec.tokens.length > 0) {
            const dynamicReason = checkDynamicSubcommand(
              strippedExec.tokens,
              placeholders,
              `${contextLabel} find ${ft}`,
              false,
              strippedExec.metas,
            );
            if (dynamicReason) {
              return dynamicReason;
            }
          }
        }
        i = j;
      }
    }
  }

  if (subBase === 'tauri') {
    const tauriInfo = resolveTauriSubcommand(subToks);
    if (
      (!tauriInfo.hasInfoFlag && !tauriInfo.sub) ||
      ((tauriInfo.sub === 'android' || tauriInfo.sub === 'ios') &&
        !tauriInfo.hasInfoFlag &&
        !tauriInfo.secondarySub) ||
      isForbiddenTauriSubcommand(tauriInfo, subToks, subMetas, placeholders)
    ) {
      return `forbidden '${contextLabel} tauri' without safe subcommand`;
    }
  }

  if (subBase === 'vite') {
    if (!isSafeViteCommand(subToks, subMetas) || subToks.some((t) => containsAnyPlaceholder(t, placeholders))) {
      return `forbidden '${contextLabel} vite' dev server command`;
    }
  }

  if (subBase === 'npm') {
    let subIdx = -1;
    let sawUnrecognizedPreFlag = false;
    for (let i = 1; i < subToks.length; i += 1) {
      const st = subToks[i];
      if (st === '--') {
        if (i + 1 < subToks.length) {
          subIdx = i + 1;
        }
        break;
      }
      const callFlag = parseNpmCallFlag(subToks, i);
      if (callFlag.matched) {
        if (containsAnyPlaceholder(callFlag.payload, placeholders)) {
          return `forbidden '${contextLabel} npm' with placeholder '-c' script`;
        }
        i += callFlag.step - 1;
        continue;
      }
      if (consumesNpmFlagValue(subToks, i, NPM_PRE_SUBCMD_VALUE_FLAGS)) {
        i += 1;
        continue;
      }
      if (st.startsWith('-')) {
        if (!st.includes('=') && !isKnownNpmBooleanOrShortFlag(st, NPM_PRE_SUBCMD_VALUE_FLAGS)) {
          sawUnrecognizedPreFlag = true;
        }
        continue;
      }
      subIdx = i;
      break;
    }
    const npmSub = subIdx >= 0 ? subToks[subIdx] : '';
    if (
      !npmSub ||
      containsAnyPlaceholder(npmSub, placeholders) ||
      hasDynamicExpansionAt(subToks, subMetas, subIdx)
    ) {
      return `forbidden '${contextLabel} npm' without an explicit subcommand`;
    }
    if (NPM_RUN_SUBCOMMANDS.has(npmSub)) {
      let scriptName = '';
      let scriptIdx = -1;
      for (let j = subIdx + 1; j < subToks.length; j += 1) {
        const st = subToks[j];
        if (st === '--') {
          scriptIdx = j + 1;
          scriptName = subToks[j + 1] ?? '';
          break;
        }
        if (consumesNpmFlagValue(subToks, j, NPM_VALUE_FLAGS)) {
          j += 1;
          continue;
        }
        if (st.startsWith('-')) continue;
        scriptIdx = j;
        scriptName = st;
        break;
      }
      if (
        !scriptName ||
        containsAnyPlaceholder(scriptName, placeholders) ||
        hasDynamicExpansionAt(subToks, subMetas, scriptIdx)
      ) {
        return `forbidden '${contextLabel} npm ${npmSub}' without an explicit script name`;
      }
    } else if (npmSub === 'exec' || npmSub === 'x') {
      let execCmd = '';
      let execIdx = -1;
      let hasCallPayload = false;
      let sawUnrecognizedExecFlag = false;
      for (let j = subIdx + 1; j < subToks.length; j += 1) {
        const st = subToks[j];
        if (st === '--') {
          execIdx = j + 1;
          execCmd = subToks[j + 1] ?? '';
          break;
        }
        const callFlag = parseNpmCallFlag(subToks, j);
        if (callFlag.matched) {
          hasCallPayload = true;
          if (containsAnyPlaceholder(callFlag.payload, placeholders)) {
            return `forbidden '${contextLabel} npm ${npmSub}' with placeholder '-c' script`;
          }
          j += callFlag.step - 1;
          continue;
        }
        if (consumesNpmFlagValue(subToks, j, NPM_EXEC_VALUE_FLAGS)) {
          j += 1;
          continue;
        }
        if (st.startsWith('-')) {
          if (!st.includes('=') && !isKnownNpmBooleanOrShortFlag(st, NPM_EXEC_VALUE_FLAGS)) {
            sawUnrecognizedExecFlag = true;
          }
          continue;
        }
        execIdx = j;
        execCmd = st;
        break;
      }
      if (
        (!execCmd && !hasCallPayload) ||
        containsAnyPlaceholder(execCmd, placeholders) ||
        hasDynamicExpansionAt(subToks, subMetas, execIdx)
      ) {
        return `forbidden '${contextLabel} npm ${npmSub}' without an explicit command`;
      }
      if (execIdx >= 0 && execIdx < subToks.length) {
        const execTail = subToks.slice(execIdx);
        const execTailMetas = subMetas ? subMetas.slice(execIdx) : null;
        const strippedExec = stripCommandPrefixes(execTail, 0, [], execTailMetas);
        if (strippedExec.error) {
          return `forbidden '${contextLabel} npm ${npmSub}' command: ${strippedExec.error}`;
        }
        if (strippedExec.tokens.length === 0) {
          return `forbidden '${contextLabel} npm ${npmSub}' without an explicit command`;
        }
        const dynamicReason = checkDynamicSubcommand(
          strippedExec.tokens,
          placeholders,
          `${contextLabel} npm ${npmSub}`,
          false,
          strippedExec.metas,
        );
        if (dynamicReason) {
          return dynamicReason;
        }
      }
      if (sawUnrecognizedExecFlag) {
        for (let k = subIdx + 1; k < subToks.length; k += 1) {
          const st = subToks[k];
          if (st === '--') {
            if (k + 1 < subToks.length) {
              const tail = subToks.slice(k + 1);
              const tailMetas = subMetas ? subMetas.slice(k + 1) : null;
              const strippedExec = stripCommandPrefixes(tail, 0, [], tailMetas);
              if (strippedExec.error) {
                return `forbidden '${contextLabel} npm ${npmSub}' command: ${strippedExec.error}`;
              }
              if (strippedExec.tokens.length > 0) {
                const dynamicReason = checkDynamicSubcommand(
                  strippedExec.tokens,
                  placeholders,
                  `${contextLabel} npm ${npmSub}`,
                  false,
                  strippedExec.metas,
                );
                if (dynamicReason) return dynamicReason;
              }
            }
            break;
          }
          const callFlag = parseNpmCallFlag(subToks, k);
          if (callFlag.matched) {
            if (containsAnyPlaceholder(callFlag.payload, placeholders)) {
              return `forbidden '${contextLabel} npm ${npmSub}' with placeholder '-c' script`;
            }
            k += callFlag.step - 1;
            continue;
          }
          if (!st.startsWith('-')) {
            const tail = subToks.slice(k);
            const tailMetas = subMetas ? subMetas.slice(k) : null;
            const strippedExec = stripCommandPrefixes(tail, 0, [], tailMetas);
            if (strippedExec.error) {
              return `forbidden '${contextLabel} npm ${npmSub}' command: ${strippedExec.error}`;
            }
            if (strippedExec.tokens.length > 0) {
              const dynamicReason = checkDynamicSubcommand(
                strippedExec.tokens,
                placeholders,
                `${contextLabel} npm ${npmSub}`,
                false,
                strippedExec.metas,
              );
              if (dynamicReason) return dynamicReason;
            }
          }
        }
      }
    } else if (sawUnrecognizedPreFlag) {
      for (let k = 1; k < subToks.length; k += 1) {
        const st = subToks[k];
        if (st === '--') {
          if (k + 1 < subToks.length) {
            const fallbackReason = checkDynamicSubcommand(
              ['npm', ...subToks.slice(k + 1)],
              placeholders,
              contextLabel,
              false,
              subMetas ? [{ hasUnquotedBrace: false, hasUnquotedGlob: false }, ...subMetas.slice(k + 1)] : null,
            );
            if (fallbackReason) return fallbackReason;
          }
          break;
        }
        const callFlag = parseNpmCallFlag(subToks, k);
        if (callFlag.matched) {
          if (containsAnyPlaceholder(callFlag.payload, placeholders)) {
            return `forbidden '${contextLabel} npm' with placeholder '-c' script`;
          }
          k += callFlag.step - 1;
          continue;
        }
        if (NPM_RUN_SUBCOMMANDS.has(st) || st === 'exec' || st === 'x') {
          const fallbackReason = checkDynamicSubcommand(
            ['npm', ...subToks.slice(k)],
            placeholders,
            contextLabel,
            false,
            subMetas ? [{ hasUnquotedBrace: false, hasUnquotedGlob: false }, ...subMetas.slice(k)] : null,
          );
          if (fallbackReason) return fallbackReason;
        }
      }
    }
  }

  if (subBase === 'npx') {
    let idx = 1;
    let hasCallPayload = false;
    let sawUnrecognizedNpxFlag = false;
    while (idx < subToks.length) {
      const st = subToks[idx];
      if (st === '--') {
        idx += 1;
        break;
      }
      const callFlag = parseNpmCallFlag(subToks, idx);
      if (callFlag.matched) {
        hasCallPayload = true;
        if (containsAnyPlaceholder(callFlag.payload, placeholders)) {
          return `forbidden '${contextLabel} npx' with placeholder '-c' script`;
        }
        idx += callFlag.step;
        continue;
      }
      if (consumesNpmFlagValue(subToks, idx, NPX_VALUE_FLAGS)) {
        idx += 2;
        continue;
      }
      if (st.startsWith('-')) {
        if (!st.includes('=') && !isKnownNpmBooleanOrShortFlag(st, NPX_VALUE_FLAGS)) {
          sawUnrecognizedNpxFlag = true;
        }
        idx += 1;
        continue;
      }
      break;
    }
    const npxTarget = subToks[idx] ?? '';
    const npxTail = subToks.slice(idx);
    const npxTailMetas = subMetas ? subMetas.slice(idx) : null;
    const tauriCheck =
      npxTarget === 'tauri' ? resolveTauriSubcommand(npxTail) : null;
    if (
      (!npxTarget && !hasCallPayload) ||
      containsAnyPlaceholder(npxTarget, placeholders) ||
      hasDynamicExpansionAt(subToks, subMetas, idx) ||
      (tauriCheck &&
        ((!tauriCheck.hasInfoFlag && !tauriCheck.sub) ||
          ((tauriCheck.sub === 'android' || tauriCheck.sub === 'ios') &&
            !tauriCheck.hasInfoFlag &&
            !tauriCheck.secondarySub) ||
          isForbiddenTauriSubcommand(tauriCheck, npxTail, npxTailMetas, placeholders))) ||
      (npxTarget === 'vite' &&
        (!isSafeViteCommand(npxTail, npxTailMetas) ||
          npxTail.some((t) => containsAnyPlaceholder(t, placeholders))))
    ) {
      return `forbidden '${contextLabel} npx' without an explicit safe command`;
    }
    if (idx < subToks.length) {
      const strippedNpx = stripCommandPrefixes(npxTail, 0, [], npxTailMetas);
      if (strippedNpx.error) {
        return `forbidden '${contextLabel} npx' command: ${strippedNpx.error}`;
      }
      if (strippedNpx.tokens.length > 0) {
        const dynamicReason = checkDynamicSubcommand(
          strippedNpx.tokens,
          placeholders,
          `${contextLabel} npx`,
          false,
          strippedNpx.metas,
        );
        if (dynamicReason) return dynamicReason;
      }
    }
    if (sawUnrecognizedNpxFlag) {
      for (let k = 1; k < subToks.length; k += 1) {
        const st = subToks[k];
        if (st === '--') {
          if (k + 1 < subToks.length) {
            const tail = subToks.slice(k + 1);
            const tailMetas = subMetas ? subMetas.slice(k + 1) : null;
            const strippedNpx = stripCommandPrefixes(tail, 0, [], tailMetas);
            if (strippedNpx.error) {
              return `forbidden '${contextLabel} npx' command: ${strippedNpx.error}`;
            }
            if (strippedNpx.tokens.length > 0) {
              const dynamicReason = checkDynamicSubcommand(
                strippedNpx.tokens,
                placeholders,
                `${contextLabel} npx`,
                false,
                strippedNpx.metas,
              );
              if (dynamicReason) return dynamicReason;
            }
          }
          break;
        }
        const callFlag = parseNpmCallFlag(subToks, k);
        if (callFlag.matched) {
          if (containsAnyPlaceholder(callFlag.payload, placeholders)) {
            return `forbidden '${contextLabel} npx' with placeholder '-c' script`;
          }
          k += callFlag.step - 1;
          continue;
        }
        if (!st.startsWith('-')) {
          const tail = subToks.slice(k);
          const tailMetas = subMetas ? subMetas.slice(k) : null;
          const strippedNpx = stripCommandPrefixes(tail, 0, [], tailMetas);
          if (strippedNpx.error) {
            return `forbidden '${contextLabel} npx' command: ${strippedNpx.error}`;
          }
          if (strippedNpx.tokens.length > 0) {
            const dynamicReason = checkDynamicSubcommand(
              strippedNpx.tokens,
              placeholders,
              `${contextLabel} npx`,
              false,
              strippedNpx.metas,
            );
            if (dynamicReason) return dynamicReason;
          }
        }
      }
    }
  }

  if (subBase === 'rm') {
    let hasRecursive = false;
    let endOfFlags = false;
    const targets = [];
    for (let i = 1; i < subToks.length; i += 1) {
      const st = subToks[i];
      if (!endOfFlags && st === '--') {
        endOfFlags = true;
        continue;
      }
      if (!endOfFlags && st.startsWith('-') && st.length > 1) {
        if (hasDynamicExpansionAt(subToks, subMetas, i) || containsAnyPlaceholder(st, placeholders)) {
          return `forbidden '${contextLabel} rm' with dynamic flag '${st}'`;
        }
        if (st === '--recursive' || (!st.startsWith('--') && /[rR]/.test(st.slice(1)))) {
          hasRecursive = true;
        }
        continue;
      }
      if (
        containsAnyPlaceholder(st, placeholders) ||
        hasDynamicShellExpansion(st) ||
        subMetas?.[i]?.hasUnquotedBrace
      ) {
        return `forbidden '${contextLabel} rm' with dynamic target '${st}'`;
      }
      targets.push(st);
    }
    if (targets.length === 0 || (hasRecursive && targets.some((t) => isDangerousRmTarget(t)))) {
      return `forbidden '${contextLabel} rm' without explicit safe target paths`;
    }
  }

  return null;
}

function checkTokensSafety(
  rawTokens,
  depth = 0,
  pipedIn = false,
  endedAtSub = false,
  redirs = [],
  rawMetas = null,
) {
  if (depth > MAX_SHELL_RECURSION_DEPTH) {
    return `nested shell command recursion depth exceeded (>${MAX_SHELL_RECURSION_DEPTH})`;
  }
  const stripped = stripCommandPrefixes(rawTokens, depth, redirs, rawMetas);
  if (stripped.error) return stripped.error;
  const tokens = stripped.tokens;
  const metas = stripped.metas;
  if (tokens.length === 0) return null;

  const cmd = tokens[0];
  if (hasDynamicExpansionAt(tokens, metas, 0)) {
    return `forbidden dynamic command word '${cmd}'`;
  }

  const baseCmd = cmd.split('/').pop() ?? cmd;

  if (baseCmd === 'export' || baseCmd === 'declare' || baseCmd === 'local' || baseCmd === 'typeset') {
    for (const arg of tokens.slice(1)) {
      if (FORBIDDEN_STARTUP_ENV_ASSIGN_RE.test(arg)) {
        return `forbidden shell startup file variable assignment '${arg}'`;
      }
    }
  }

  if (baseCmd === 'shopt') {
    const rest = tokens.slice(1);
    const hasEnableFlag = rest.some((t) => t === '-s' || /^-[A-Za-z]*s[A-Za-z]*$/.test(t));
    if (hasEnableFlag && rest.includes('expand_aliases')) {
      return "forbidden 'shopt -s expand_aliases' (aliases can obscure executed commands)";
    }
  }

  if (baseCmd === 'alias') {
    for (const arg of tokens.slice(1)) {
      if (arg === '-p' || arg === '--') continue;
      const eqIdx = arg.indexOf('=');
      if (eqIdx > 0) {
        const aliasPayload = arg.slice(eqIdx + 1);
        if (aliasPayload.trim().length > 0) {
          const subRes = validateCommand(aliasPayload, depth + 1);
          if (!subRes.ok) {
            return `forbidden 'alias' command: ${subRes.reason}`;
          }
        }
      }
    }
  }

  if (baseCmd === 'trap') {
    let isPrintOrList = false;
    let idx = 1;
    while (idx < tokens.length) {
      const tok = tokens[idx];
      if (tok === '--') {
        idx += 1;
        break;
      }
      if (tok === '-l' || tok === '-p' || /^-[lp]+$/.test(tok)) {
        isPrintOrList = true;
        idx += 1;
        continue;
      }
      if (tok.startsWith('-') && tok.length > 1) {
        idx += 1;
        continue;
      }
      break;
    }
    const trapArgs = tokens.slice(idx);
    if (!isPrintOrList && trapArgs.length > 0) {
      const action = trapArgs[0];
      const isSignalResetOnly =
        trapArgs.length === 1 && /^(?:[0-9]+|(?:SIG)?[A-Z0-9_+]+)$/.test(action);
      if (action !== '-' && action.trim().length > 0 && !isSignalResetOnly) {
        const subRes = validateCommand(action, depth + 1);
        if (!subRes.ok) {
          return `forbidden 'trap' handler command: ${subRes.reason}`;
        }
      }
    }
  }

  if (baseCmd === 'bash' || baseCmd === 'sh' || baseCmd === 'zsh') {
    const parsed = parseShellInvocationFlags(tokens);
    const { hasCFlag, hasSFlag, hasInfoFlag, hasExpandAliasesOpt, argIdx } = parsed;
    let { hasStdinRedir } = parsed;

    if (hasExpandAliasesOpt) {
      return `forbidden '${baseCmd} -O expand_aliases' option`;
    }

    if (hasCFlag) {
      if (
        redirs.some((r) => INPUT_REDIR_OP_RE.test(r.op) && !(r.op === '<' && r.target === '/dev/null'))
      ) {
        return `forbidden un-inspected '${baseCmd} -c' input redirection`;
      }
      if (argIdx < tokens.length || tokens.length > 2) {
        const subScript = argIdx < tokens.length ? tokens[argIdx] : tokens[tokens.length - 1];
        if (endedAtSub && !subScript.trim()) {
          return `forbidden un-inspected '${baseCmd} -c' with dynamic command substitution`;
        }
        if (IMPLICIT_FOR_LOOP_RE.test(subScript)) {
          return `forbidden '${baseCmd} -c' with implicit positional 'for' loop in inline script`;
        }
        if (subScript.trim().length > 0) {
          const subRes = validateCommand(subScript, depth + 1);
          if (!subRes.ok) {
            return `forbidden nested '${baseCmd} -c' command: ${subRes.reason}`;
          }
        }
      } else if (endedAtSub) {
        return `forbidden un-inspected '${baseCmd} -c' with dynamic command substitution`;
      }
    } else if (!hasInfoFlag) {
      if (redirs.some((r) => INPUT_REDIR_OP_RE.test(r.op))) {
        hasStdinRedir = true;
      }
      const rest = tokens.slice(argIdx);
      const positionalArgs = [];
      for (let i = 0; i < rest.length; i += 1) {
        const t = rest[i];
        if (STANDALONE_REDIR_RE.test(t)) {
          if (INPUT_REDIR_OP_RE.test(t)) {
            hasStdinRedir = true;
          }
          i += 1;
          continue;
        }
        if (ATTACHED_REDIR_RE.test(t)) {
          if (INPUT_REDIR_ATTACHED_RE.test(t)) {
            hasStdinRedir = true;
          }
          continue;
        }
        positionalArgs.push(t);
      }

      if (
        pipedIn ||
        hasSFlag ||
        hasStdinRedir ||
        positionalArgs.length === 0 ||
        positionalArgs.some((arg) => FD_SCRIPT_PATH_RE.test(arg))
      ) {
        return `forbidden un-inspected '${baseCmd}' stdin/pipe/fd/process-substitution execution`;
      }
    }
  }

  if (baseCmd === 'source' || baseCmd === '.') {
    const hasInputRedir =
      redirs.some((r) => INPUT_REDIR_OP_RE.test(r.op)) ||
      tokens
        .slice(1)
        .some((t) => INPUT_REDIR_OP_RE.test(t) || INPUT_REDIR_ATTACHED_RE.test(t));
    const hasFdPath = tokens.slice(1).some((t) => FD_SCRIPT_PATH_RE.test(t));
    if (pipedIn || hasInputRedir || hasFdPath || tokens.length <= 1) {
      return `forbidden un-inspected '${baseCmd}' input`;
    }
  }

  if (baseCmd === 'eval') {
    let argIdx = 1;
    if (tokens[argIdx] === '--') {
      argIdx += 1;
    }
    const evalScript = tokens.slice(argIdx).join(' ');
    if (endedAtSub || /\$\(|`/.test(evalScript)) {
      return "forbidden dynamic 'eval' with command substitution";
    }
    if (SHELL_POSITIONAL_OR_VAR_RE.test(evalScript)) {
      return "forbidden dynamic 'eval' with variable or positional-parameter expansion";
    }
    if (evalScript.trim().length > 0) {
      const subRes = validateCommand(evalScript, depth + 1);
      if (!subRes.ok) {
        return `forbidden nested 'eval' command: ${subRes.reason}`;
      }
    }
  }

  if (baseCmd === 'xargs') {
    const xargsPlaceholders = new Set();
    let cmdIdx = 1;
    while (cmdIdx < tokens.length) {
      const tok = tokens[cmdIdx];
      if (tok === '--') {
        cmdIdx += 1;
        break;
      }
      if (STANDALONE_REDIR_RE.test(tok)) {
        cmdIdx += 2;
        continue;
      }
      if (ATTACHED_REDIR_RE.test(tok)) {
        cmdIdx += 1;
        continue;
      }
      if (tok === '-I' || tok === '-J') {
        const repl = tokens[cmdIdx + 1] ?? '';
        if (repl) xargsPlaceholders.add(repl);
        cmdIdx += 2;
        continue;
      }
      if (tok === '--replace' || tok === '-i') {
        xargsPlaceholders.add('{}');
        cmdIdx += 1;
        continue;
      }
      if (tok.startsWith('--replace=')) {
        const repl = tok.slice('--replace='.length) || '{}';
        xargsPlaceholders.add(repl);
        cmdIdx += 1;
        continue;
      }
      if (XARGS_VALUE_FLAGS.has(tok)) {
        cmdIdx += 2;
        continue;
      }
      if (tok.startsWith('--')) {
        cmdIdx += 1;
        continue;
      }
      if (tok.startsWith('-') && tok.length > 1) {
        let consumedNext = false;
        for (let j = 1; j < tok.length; j += 1) {
          const ch = tok[j];
          if (ch === 'I' || ch === 'J') {
            if (j + 1 < tok.length) {
              const repl = tok.slice(j + 1);
              if (repl) xargsPlaceholders.add(repl);
            } else {
              const repl = tokens[cmdIdx + 1] ?? '';
              if (repl) xargsPlaceholders.add(repl);
              consumedNext = true;
            }
            break;
          }
          if (ch === 'i') {
            const repl = j + 1 < tok.length ? tok.slice(j + 1) : '{}';
            if (repl) xargsPlaceholders.add(repl);
            break;
          }
          if (XARGS_SHORT_VALUE_CHARS.has(ch)) {
            if (j + 1 >= tok.length) {
              consumedNext = true;
            }
            break;
          }
        }
        cmdIdx += consumedNext ? 2 : 1;
        continue;
      }
      break;
    }

    if (cmdIdx < tokens.length) {
      const subTokens = tokens.slice(cmdIdx);
      const subMetas = metas.slice(cmdIdx);
      const nestedReason = checkTokensSafety(subTokens, depth + 1, false, false, [], subMetas);
      if (nestedReason) {
        return `forbidden nested 'xargs' command: ${nestedReason}`;
      }
      const strippedSub = stripCommandPrefixes(subTokens, depth + 1, [], subMetas);
      if (strippedSub.error) {
        return `forbidden nested 'xargs' command: ${strippedSub.error}`;
      }
      if (strippedSub.tokens.length === 0) {
        return "forbidden 'xargs' wrapper invocation with no explicit target command";
      }
      const dynamicReason = checkDynamicSubcommand(
        strippedSub.tokens,
        xargsPlaceholders,
        'xargs',
        false,
        strippedSub.metas,
      );
      if (dynamicReason) {
        return dynamicReason;
      }
    }
  }

  if (baseCmd === 'find') {
    for (let i = 1; i < tokens.length; i += 1) {
      const tok = tokens[i];
      if (tok === '-exec' || tok === '-execdir' || tok === '-ok' || tok === '-okdir') {
        let j = i + 1;
        while (j < tokens.length && tokens[j] !== ';' && tokens[j] !== '+') {
          j += 1;
        }
        const subTokens = tokens.slice(i + 1, j);
        const subMetas = metas.slice(i + 1, j);
        if (subTokens.length > 0) {
          const nestedReason = checkTokensSafety(subTokens, depth + 1, false, false, [], subMetas);
          if (nestedReason) {
            return `forbidden nested 'find ${tok}' command: ${nestedReason}`;
          }
          const strippedSub = stripCommandPrefixes(subTokens, depth + 1, [], subMetas);
          if (!strippedSub.error && strippedSub.tokens.length > 0) {
            const dynamicReason = checkDynamicSubcommand(
              strippedSub.tokens,
              new Set(['{}']),
              `find ${tok}`,
              true,
              strippedSub.metas,
            );
            if (dynamicReason) {
              return dynamicReason;
            }
          }
        }
        i = j;
      }
    }
  }

  if (baseCmd === 'npm') {
    let subIdx = -1;
    let sawUnrecognizedPreFlag = false;
    const preCallPayloads = [];
    for (let i = 1; i < tokens.length; i += 1) {
      const tok = tokens[i];
      if (tok === '--') {
        if (i + 1 < tokens.length) {
          subIdx = i + 1;
        }
        break;
      }
      const callFlag = parseNpmCallFlag(tokens, i);
      if (callFlag.matched) {
        preCallPayloads.push(callFlag.payload);
        i += callFlag.step - 1;
        continue;
      }
      if (consumesNpmFlagValue(tokens, i, NPM_PRE_SUBCMD_VALUE_FLAGS)) {
        i += 1;
        continue;
      }
      if (tok.startsWith('-')) {
        if (!tok.includes('=') && !isKnownNpmBooleanOrShortFlag(tok, NPM_PRE_SUBCMD_VALUE_FLAGS)) {
          sawUnrecognizedPreFlag = true;
        }
        continue;
      }
      subIdx = i;
      break;
    }

    for (const callPayload of preCallPayloads) {
      if (callPayload.trim().length > 0) {
        const subRes = validateCommand(callPayload, depth + 1);
        if (!subRes.ok) {
          return `forbidden nested 'npm -c' command: ${subRes.reason}`;
        }
      }
    }

    const subCmd = subIdx >= 0 ? tokens[subIdx] : '';
    if (hasDynamicExpansionAt(tokens, metas, subIdx)) {
      return `forbidden dynamic 'npm' subcommand '${subCmd}'`;
    }

    if (NPM_RUN_SUBCOMMANDS.has(subCmd)) {
      let scriptName = '';
      let scriptIdx = -1;
      let sawUnrecognizedRunFlag = false;
      for (let j = subIdx + 1; j < tokens.length; j += 1) {
        const tok = tokens[j];
        if (tok === '--') {
          scriptIdx = j + 1;
          scriptName = tokens[j + 1] ?? '';
          break;
        }
        if (consumesNpmFlagValue(tokens, j, NPM_VALUE_FLAGS)) {
          j += 1;
          continue;
        }
        if (tok.startsWith('-')) {
          if (!tok.includes('=') && !isKnownNpmBooleanOrShortFlag(tok, NPM_VALUE_FLAGS)) {
            sawUnrecognizedRunFlag = true;
          }
          continue;
        }
        scriptIdx = j;
        scriptName = tok;
        break;
      }
      if (FORBIDDEN_NPM_SCRIPTS.has(scriptName)) {
        return `forbidden desktop/dev script 'npm ${subCmd} ${scriptName}'`;
      }
      if (hasDynamicExpansionAt(tokens, metas, scriptIdx)) {
        return `forbidden dynamic script name '${scriptName}' in 'npm ${subCmd}'`;
      }
      if (sawUnrecognizedRunFlag && scriptIdx >= 0) {
        for (let k = scriptIdx + 1; k < tokens.length; k += 1) {
          const tok = tokens[k];
          if (tok === '--') {
            const afterDash = tokens[k + 1] ?? '';
            if (FORBIDDEN_NPM_SCRIPTS.has(afterDash)) {
              return `forbidden desktop/dev script 'npm ${subCmd} ${afterDash}'`;
            }
            if (hasDynamicExpansionAt(tokens, metas, k + 1)) {
              return `forbidden dynamic script name '${afterDash}' in 'npm ${subCmd}'`;
            }
            break;
          }
          if (tok.startsWith('-')) continue;
          if (FORBIDDEN_NPM_SCRIPTS.has(tok)) {
            return `forbidden desktop/dev script 'npm ${subCmd} ${tok}'`;
          }
          if (hasDynamicExpansionAt(tokens, metas, k)) {
            return `forbidden dynamic script name '${tok}' in 'npm ${subCmd}'`;
          }
        }
      }
    } else if (subCmd === 'exec' || subCmd === 'x') {
      let cmdIdx = -1;
      let sawUnrecognizedExecFlag = false;
      for (let j = subIdx + 1; j < tokens.length; j += 1) {
        const tok = tokens[j];
        if (tok === '--') {
          cmdIdx = j + 1;
          break;
        }
        const callFlag = parseNpmCallFlag(tokens, j);
        if (callFlag.matched) {
          if (callFlag.payload.trim().length > 0) {
            const subRes = validateCommand(callFlag.payload, depth + 1);
            if (!subRes.ok) {
              return `forbidden nested 'npm ${subCmd} -c' command: ${subRes.reason}`;
            }
          }
          j += callFlag.step - 1;
          continue;
        }
        if (consumesNpmFlagValue(tokens, j, NPM_EXEC_VALUE_FLAGS)) {
          j += 1;
          continue;
        }
        if (tok.startsWith('-')) {
          if (!tok.includes('=') && !isKnownNpmBooleanOrShortFlag(tok, NPM_EXEC_VALUE_FLAGS)) {
            sawUnrecognizedExecFlag = true;
          }
          continue;
        }
        cmdIdx = j;
        break;
      }

      if (cmdIdx >= 0 && cmdIdx < tokens.length) {
        const resolved = tokens.slice(cmdIdx);
        const resolvedMetas = metas.slice(cmdIdx);
        const nestedReason = checkTokensSafety(resolved, depth + 1, false, false, [], resolvedMetas);
        if (nestedReason) {
          return nestedReason;
        }
      }
      if (sawUnrecognizedExecFlag) {
        for (let k = subIdx + 1; k < tokens.length; k += 1) {
          const tok = tokens[k];
          if (tok === '--') {
            if (k + 1 < tokens.length) {
              const nestedReason = checkTokensSafety(
                tokens.slice(k + 1),
                depth + 1,
                false,
                false,
                [],
                metas.slice(k + 1),
              );
              if (nestedReason) return nestedReason;
            }
            break;
          }
          const callFlag = parseNpmCallFlag(tokens, k);
          if (callFlag.matched) {
            if (callFlag.payload.trim().length > 0) {
              const subRes = validateCommand(callFlag.payload, depth + 1);
              if (!subRes.ok) {
                return `forbidden nested 'npm ${subCmd} -c' command: ${subRes.reason}`;
              }
            }
            k += callFlag.step - 1;
            continue;
          }
          if (!tok.startsWith('-')) {
            const nestedReason = checkTokensSafety(
              tokens.slice(k),
              depth + 1,
              false,
              false,
              [],
              metas.slice(k),
            );
            if (nestedReason) return nestedReason;
          }
        }
      }
    } else if (sawUnrecognizedPreFlag) {
      for (let k = 1; k < tokens.length; k += 1) {
        const tok = tokens[k];
        if (tok === '--') {
          if (k + 1 < tokens.length) {
            const fallbackReason = checkTokensSafety(
              ['npm', ...tokens.slice(k + 1)],
              depth + 1,
              false,
              false,
              [],
              [{ hasUnquotedBrace: false, hasUnquotedGlob: false }, ...metas.slice(k + 1)],
            );
            if (fallbackReason) {
              return fallbackReason;
            }
          }
          break;
        }
        const callFlag = parseNpmCallFlag(tokens, k);
        if (callFlag.matched) {
          if (callFlag.payload.trim().length > 0) {
            const subRes = validateCommand(callFlag.payload, depth + 1);
            if (!subRes.ok) {
              return `forbidden nested 'npm -c' command: ${subRes.reason}`;
            }
          }
          k += callFlag.step - 1;
          continue;
        }
        if (NPM_RUN_SUBCOMMANDS.has(tok) || tok === 'exec' || tok === 'x') {
          const fallbackReason = checkTokensSafety(
            ['npm', ...tokens.slice(k)],
            depth + 1,
            false,
            false,
            [],
            [{ hasUnquotedBrace: false, hasUnquotedGlob: false }, ...metas.slice(k)],
          );
          if (fallbackReason) {
            return fallbackReason;
          }
        }
      }
    }
  }

  if (baseCmd === 'npx') {
    let cmdIdx = 1;
    let sawUnrecognizedNpxFlag = false;
    while (cmdIdx < tokens.length) {
      const tok = tokens[cmdIdx];
      if (tok === '--') {
        cmdIdx += 1;
        break;
      }
      const callFlag = parseNpmCallFlag(tokens, cmdIdx);
      if (callFlag.matched) {
        if (callFlag.payload.trim().length > 0) {
          const subRes = validateCommand(callFlag.payload, depth + 1);
          if (!subRes.ok) {
            return `forbidden nested 'npx -c' command: ${subRes.reason}`;
          }
        }
        cmdIdx += callFlag.step;
        continue;
      }
      if (consumesNpmFlagValue(tokens, cmdIdx, NPX_VALUE_FLAGS)) {
        cmdIdx += 2;
        continue;
      }
      if (tok.startsWith('-')) {
        if (!tok.includes('=') && !isKnownNpmBooleanOrShortFlag(tok, NPX_VALUE_FLAGS)) {
          sawUnrecognizedNpxFlag = true;
        }
        cmdIdx += 1;
        continue;
      }
      break;
    }
    const npxCmd = tokens[cmdIdx] ?? '';
    const npxTail = tokens.slice(cmdIdx);
    const npxTailMetas = metas.slice(cmdIdx);
    if (npxCmd === 'tauri') {
      const tauriInfo = resolveTauriSubcommand(npxTail);
      if (isForbiddenTauriSubcommand(tauriInfo, npxTail, npxTailMetas)) {
        const shown = tauriInfo.secondarySub ? `${tauriInfo.sub} ${tauriInfo.secondarySub}` : tauriInfo.sub;
        return `forbidden desktop command 'npx tauri ${shown}'`;
      }
    }
    if (npxCmd === 'vite' && !isSafeViteCommand(npxTail, npxTailMetas)) {
      const npxArg = tokens[cmdIdx + 1] ?? '';
      return `forbidden dev server command 'npx vite${npxArg ? ` ${npxArg}` : ''}'`;
    }
    if (cmdIdx < tokens.length) {
      const nestedReason = checkTokensSafety(npxTail, depth + 1, false, false, [], npxTailMetas);
      if (nestedReason) {
        return nestedReason;
      }
    }
    if (sawUnrecognizedNpxFlag) {
      for (let k = 1; k < tokens.length; k += 1) {
        const tok = tokens[k];
        if (tok === '--') {
          if (k + 1 < tokens.length) {
            const nestedReason = checkTokensSafety(
              ['npx', ...tokens.slice(k)],
              depth + 1,
              false,
              false,
              [],
              [{ hasUnquotedBrace: false, hasUnquotedGlob: false }, ...metas.slice(k)],
            );
            if (nestedReason) return nestedReason;
          }
          break;
        }
        const callFlag = parseNpmCallFlag(tokens, k);
        if (callFlag.matched) {
          if (callFlag.payload.trim().length > 0) {
            const subRes = validateCommand(callFlag.payload, depth + 1);
            if (!subRes.ok) {
              return `forbidden nested 'npx -c' command: ${subRes.reason}`;
            }
          }
          k += callFlag.step - 1;
          continue;
        }
        if (!tok.startsWith('-')) {
          const nestedReason = checkTokensSafety(
            tokens.slice(k),
            depth + 1,
            false,
            false,
            [],
            metas.slice(k),
          );
          if (nestedReason) return nestedReason;
        }
      }
    }
  }

  if (baseCmd === 'tauri') {
    const tauriInfo = resolveTauriSubcommand(tokens);
    if (isForbiddenTauriSubcommand(tauriInfo, tokens, metas)) {
      const shown = tauriInfo.secondarySub ? `${tauriInfo.sub} ${tauriInfo.secondarySub}` : tauriInfo.sub;
      return `forbidden desktop command 'tauri ${shown}'`;
    }
  }

  if (baseCmd === 'vite' && !isSafeViteCommand(tokens, metas)) {
    return `forbidden dev server command 'vite${tokens[1] ? ` ${tokens[1]}` : ''}'`;
  }

  if (baseCmd === 'anima-engine' || baseCmd === 'anima-engine.exe') {
    return `forbidden direct desktop binary invocation '${cmd}'`;
  }

  if (baseCmd === 'cargo') {
    let sub = '';
    let subIdx = -1;
    for (let i = 1; i < tokens.length; i += 1) {
      const tok = tokens[i];
      if (CARGO_GLOBAL_VALUE_FLAGS.has(tok)) {
        i += 1;
        continue;
      }
      if (tok.startsWith('-') || tok.startsWith('+')) {
        continue;
      }
      sub = tok;
      subIdx = i;
      break;
    }

    if (sub === 'run' || sub === 'r' || hasDynamicExpansionAt(tokens, metas, subIdx)) {
      return "forbidden 'cargo run' invocation (build the release binary first and invoke it directly)";
    }

    for (let i = 1; i < tokens.length; i += 1) {
      const tok = tokens[i];
      if (tok === '--') break;
      if (tok.startsWith('--bin=')) {
        const binVal = tok.slice('--bin='.length);
        if (
          binVal === 'anima-engine' ||
          binVal === 'anima-engine.exe' ||
          hasDynamicExpansionAt(tokens, metas, i)
        ) {
          return "forbidden '--bin anima-engine' or dynamic '--bin' desktop target";
        }
      }
      if (tok === '--bin') {
        const binVal = tokens[i + 1] ?? '';
        if (
          binVal === 'anima-engine' ||
          binVal === 'anima-engine.exe' ||
          hasDynamicExpansionAt(tokens, metas, i + 1)
        ) {
          return "forbidden '--bin anima-engine' or dynamic '--bin' desktop target";
        }
      }
    }
  }

  if (baseCmd === 'rm') {
    let hasRecursive = false;
    let endOfFlags = false;
    const targets = [];

    for (let i = 1; i < tokens.length; i += 1) {
      const tok = tokens[i];
      if (!endOfFlags && tok === '--') {
        endOfFlags = true;
        continue;
      }
      if (!endOfFlags && tok.startsWith('-') && tok.length > 1) {
        if (hasDynamicExpansionAt(tokens, metas, i)) {
          return `dangerous 'rm' dynamic flag '${tok}'`;
        }
        if (tok === '--recursive' || (!tok.startsWith('--') && /[rR]/.test(tok.slice(1)))) {
          hasRecursive = true;
        }
        continue;
      }
      if (STANDALONE_REDIR_RE.test(tok)) {
        i += 1;
        continue;
      }
      if (ATTACHED_REDIR_RE.test(tok)) {
        continue;
      }
      if (hasDynamicShellExpansion(tok) || metas[i]?.hasUnquotedBrace) {
        return `dangerous 'rm' dynamic argument '${tok}'`;
      }
      targets.push(tok);
    }

    if (hasRecursive) {
      for (const target of targets) {
        if (isDangerousRmTarget(target)) {
          return `dangerous 'rm' target '${target}' (only explicit subpaths like src-tauri/target/... or /tmp/... are allowed)`;
        }
      }
    }
  }

  return null;
}

function checkSegmentSafety(segment, depth = 0, pipedIn = false, endedAtSub = false) {
  const metaTokens = tokenizeSegmentMeta(segment);
  const { tokens, metas, redirs } = stripRedirections(metaTokens);
  return checkTokensSafety(tokens, depth, pipedIn, endedAtSub, redirs, metas);
}

/**
 * Validate a single doc-test block's script against safety rules.
 */
export function validateCommand(script, depth = 0) {
  if (depth > MAX_SHELL_RECURSION_DEPTH) {
    return {
      ok: false,
      reason: `nested shell command recursion depth exceeded (>${MAX_SHELL_RECURSION_DEPTH})`,
      lineIndex: 0,
      lineText: typeof script === 'string' ? script : '',
    };
  }
  if (typeof script !== 'string' || script.trim().length === 0) {
    return { ok: false, reason: 'empty command block', lineIndex: 0, lineText: '' };
  }

  const logicalLines = stripHeredocsAndComments(script);
  for (const entry of logicalLines) {
    if (entry.unclosed) {
      return {
        ok: false,
        reason: 'unclosed quote or command substitution',
        lineIndex: entry.lineIndex,
        lineText: entry.text,
      };
    }
    if (!entry.text.trim()) continue;
    const segments = entry.isHeredocSubScript
      ? extractNestedCommandSubSegments(entry.text, { skipHeredocs: false })
      : splitShellSegmentsWithMeta(entry.text);
    for (const seg of segments) {
      if (seg.unclosedSub || seg.endedAtSub) {
        return {
          ok: false,
          reason: 'unclosed command substitution',
          lineIndex: entry.lineIndex,
          lineText: seg.text,
        };
      }
      const reason = checkSegmentSafety(seg.text, depth, seg.pipedIn, seg.endedAtSub);
      if (reason) {
        return {
          ok: false,
          reason,
          lineIndex: entry.lineIndex,
          lineText: seg.text,
        };
      }
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
 * Fast in-process parser + safety self-checks run unconditionally on startup (exits 2 if broken)
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
    '```bash',
    '   ',
    '```',
    '<!-- doc-test -->',
    '```bash',
    'echo unclosed',
  ].join('\n');

  const badParsed = extractDocCommands(malformed, 'bad.md');
  if (badParsed.errors.length !== 4 || badParsed.blocks.length !== 0) {
    failures.push(`parser expected 4 malformed errors, got ${badParsed.errors.length}`);
  }

  const unsafeCmds = [
    'npm run tauri:dev',
    'npm --silent run tauri:dev',
    'npm --prefix . run tauri:dev',
    'npm run -- tauri:dev',
    'npm exec -- tauri dev',
    'npm x -- tauri build',
    'npm exec -p @tauri-apps/cli -- tauri dev',
    'npm -p run tauri:dev',
    'npm -p exec -- tauri dev',
    'npm -p x -- tauri build',
    "npm exec -c 'cargo run'",
    'npx --yes tauri dev',
    'cargo run --bin anima-lab',
    'cargo --manifest-path src-tauri/Cargo.toml run --bin anima-lab',
    'cargo -C src-tauri run',
    'cargo build --manifest-path src-tauri/Cargo.toml --bin=anima-engine',
    "bash -c 'cargo run'",
    "bash -co errexit 'cargo run'",
    "bash -O extglob -c 'cargo run'",
    "bash --rcfile /dev/null -c 'cargo run'",
    "bash --init-file /dev/null -c 'cargo run'",
    'eval "cargo run"',
    'eval "$(echo \'cargo run\')"',
    'env -u RUST_LOG cargo run',
    'env -P /Users/Shared/bin cargo run',
    'env -P/Users/Shared/bin cargo run',
    'env -S "cargo run"',
    'env -S "bash" -c "cargo run"',
    'env --split-string="cargo run"',
    'exec cargo run',
    'exec -a foo cargo run',
    'builtin exec cargo run',
    'builtin eval "cargo run"',
    '{ cargo run; }',
    'if true; then cargo run; fi',
    'if false; then echo ok; else cargo run; fi',
    'for i in 1; do cargo run; done',
    '>/dev/null cargo run',
    '2>&1 cargo run',
    '< /dev/null cargo run',
    'echo "$(\n# step 1)\ncargo run\n)"',
    'echo \'cargo run\' | bash',
    'bash <<< \'cargo run\'',
    'bash <(echo \'cargo run\')',
    'echo run | xargs cargo',
    'find . -maxdepth 0 -exec cargo run \\;',
    'car\\\ngo run --bin anima-lab',
    'echo "hello\n<<EOF"\ncargo run',
    'cat <<EOF-1\nhi\nEOF-1\ncargo run',
    'echo "$(cargo run)"',
    'echo "$(\necho ok\ncargo run\n)"',
    'echo "`\necho ok\ncargo run\n`"',
    'npx tauri dev',
    'vite preview',
    './src-tauri/target/release/anima-engine',
    'rm -rf /',
    'rm -rf $HOME',
    'rm -rf .',
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
    'function f { cargo run; }; f',
    'function f { npm run tauri:dev; }; f',
    'echo run | xargs -I {} -S 255 cargo {}',
    'echo "cargo run" | xargs -I{} sh -c "{}"',
    'echo "cargo run" | xargs -I CMD sh -c "CMD"',
    'echo "tauri:dev" | xargs -I{} npm run {}',
    'echo "/" | xargs -I{} rm -rf {}',
    'printf \'"cargo run"\\n\' | xargs -n1 sh -c \'eval "$1"\' _',
    'find . -maxdepth 0 -exec sh -c \'eval "$1"\' _ "cargo run" \\;',
    'find . -maxdepth 0 -exec sh -c \'eval "$0"\' "cargo run" \\;',
    'CMD="cargo run"; eval "$CMD"',
    "BASH_ENV=<(echo 'cargo run') bash -c 'echo ok'",
    'cargo</dev/null run',
    'cargo 2>err.txt run',
    'npm</dev/null run tauri:dev',
    'npm run 2>err.txt tauri:dev',
    'rm>rm_out.txt -rf /',
    'echo "cargo run" | xargs env',
    'echo "cargo run" | xargs -I { env -S "{"',
    'npm -- run tauri:dev',
    'npm --workspaces false run tauri:dev',
    'npm run --workspaces false tauri:dev',
    'npx -yc "cargo run"',
    'npm exec -yc "cargo run"',
    'npm --init-author-name foo -c="cargo run" exec',
    'npm run --init-version 1.0.0 tauri:dev',
    'bash -c \'"$@"\' _ cargo run',
    "bash -c '$0 $1' cargo run",
    'find . -maxdepth 0 -exec sh -c \'for a; do $a; done\' _ "cargo run" \\;',
    "bash /dev/fd/3 3<<< 'cargo run'",
    "exec 3<<< 'cargo run'; bash /dev/fd/3",
    "trap 'cargo run' EXIT",
    "$'cargo' run",
    "bash -c $'cargo\\x20run'",
    "npm run $'tauri\\x3adev'",
    'CMD="cargo run"; $CMD',
    'T=/; rm -rf "$T"',
    "shopt -s expand_aliases\nalias x='cargo run'\nx",
    '$(echo cargo) run',
    '`echo cargo` run',
    'cargo $(echo run)',
    'cargo `echo run`',
    'npm $(echo run) tauri:dev',
    'npm "$(echo run)" tauri:dev',
    'npm "`echo run`" tauri:dev',
    'npm run $(echo tauri:dev)',
    'npm exec -- $(echo tauri) dev',
    'npx $(echo tauri) dev',
    'tauri $(echo dev)',
    'rm -rf $(echo /)',
    'rm $(echo -rf) /',
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
    'npm -sy false run tauri:dev',
    'npm -sf false run tauri:dev',
    'npm run -sy false tauri:dev',
    'npm exec -sy false -- tauri dev',
    'npx -sy false tauri dev',
    'npm exec -py pkg -c "cargo run"',
    'npx -py pkg -c "cargo run"',
    "cat <<$'EOF' > /dev/null\nhello\nEOF\ncargo run",
    "cat <<E$'O'F > /dev/null\nhi\nEOF\ncargo run",
    'echo "cargo" | xargs -I{} find . -maxdepth 0 -exec {} run \\;',
    'echo "cargo" | xargs -I{} find . -maxdepth 0 -execdir {} run \\;',
    'echo "tauri:dev" | xargs -I{} find . -maxdepth 0 -exec npm run {} \\;',
    'echo "rm" | xargs -I{} find . -maxdepth 0 -exec {} -rf / \\;',
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
    '$(\necho cargo\n) run',
    '`\necho cargo\n` run',
    'cargo $(\necho run\n)',
    'npm run $(\necho tauri:dev\n)',
    '$(cat <<\'EOF\'\ncargo run\nEOF\n)',
    'cargo $(cat <<\'EOF\'\nrun\nEOF\n)',
    'npm run $(cat <<\'EOF\'\ntauri:dev\nEOF\n)',
    'rm -rf $(cat <<\'EOF\'\n/\nEOF\n)',
    'tauri $(\necho dev\n)',
    'echo $(cargo run',
    'cat <<EOF \\\n&& cargo run\nhello\nEOF',
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
    'echo "$(case a in a) cargo run;; esac)"',
    '{cargo,} run',
    'cargo {run,}',
    'cargo {run,check}',
    '{npm,} run tauri:dev',
    'npm {run,} tauri:dev',
    'npm run {tauri:dev,check:docs}',
    'rm -rf {/,/tmp/safe}',
    'rm {-rf,} /',
    "bash -c '{cargo,} run'",
    'eval {cargo,run}',
    'env {cargo,} run',
    'echo run | xargs {cargo,}',
    'tauri {dev,info}',
    "/bin/ba[s]h -c 'cargo run'",
    '/usr/bin/e[n]v cargo run',
    './node_modules/.bin/tau[r]i dev',
    './node_modules/.bin/vi[t]e --port 3000',
    '$PROBE/bin/car[g]o run',
    'find . -maxdepth 0 -exec $PROBE/bin/car[g]o run \\;',
    'echo run | xargs $PROBE/bin/car[g]o',
    'F="-rf /"; rm $F',
    "bash -c 'rm $1' _ '-rf /'",
    'set -- -rf /; rm "$@"',
    "bash -c 'rm $@' _ -rf /",
    'npm -v false run tauri:dev',
    'npm --help false run tauri:dev',
    'npm -sv false run tauri:dev',
    'npm exec -v false -- tauri dev',
    'npx --no-unknown false tauri dev',
    'npx --no-silent false tauri dev',
    'npm exec --no-unknown false tauri dev',
    'tauri android dev',
    'tauri ios dev',
    'tauri android build',
    'tauri ios run',
    // Iteration 9 regressions:
    // 1. Heredoc inside $(...) / "$(...)" double-stripping
    "echo $(\ncat <<'EOF'\nhello\nEOF\ncargo run\n)",
    "echo \"$(\ncat <<'EOF'\nhello\nEOF\nrm -rf /\n)\"",
    "echo $(\ncat <<'EOF'\nhello\nEOF\nnpm run tauri:dev\n)",
    "echo $(\ncat <<'EOF'\nhello\nEOF\ntauri dev\n)",
    // 2. Unquoted heredoc expansion & backslash-newline continuation delimiters
    'cat <<EOF\n$(cargo run)\nEOF',
    "cat <<EOF\n'$(cargo run)'\nEOF",
    'cat <<EOF\n`cargo run`\nEOF',
    'cat <<EOF\n$(npm run tauri:dev)\nEOF',
    'cat <<EOF\n$(rm -rf /)\nEOF',
    'cat <<-EOF\n\t`rm -rf /`\n\tEOF',
    'echo $(cat <<EOF\n$(cargo run)\nEOF\n)',
    'cat <<\\\nEOF\nhi\nEOF\ncargo run',
    'cat <<E\\\nOF\nhi\nEOF\ncargo run',
    'cat <<-\\\nEOF\n\thi\n\tEOF\ncargo run',
    // 3. case inside $(...) prefixes & zsh ;|
    'echo $(if case x in x) cargo run ;; esac; then echo ok; fi)',
    'echo $(while case x in x) cargo run ;; esac; do break; done)',
    'echo $(until case x in x) cargo run ;; esac; do break; done)',
    'echo $(time case x in x) cargo run ;; esac)',
    'echo $(time -p case x in x) cargo run ;; esac)',
    'echo $(coproc case x in x) cargo run ;; esac; wait)',
    'echo $(foo() case x in x) cargo run ;; esac\nfoo)',
    'echo $(>/dev/null case x in x) cargo run;; esac)',
    'echo $(case a in a) echo 1 ;| a) cargo run ;; esac)',
    // 4. $"..." in tokenizeSegmentMeta & named FD redirection {fd}>...
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
    // 5. xargs / xargs ... find -exec + npm exec / npm x tail validation
    'echo "dev" | xargs npm exec -- tauri',
    'echo "dev" | xargs -I{} npm exec -- tauri {}',
    'echo "dev" | xargs -I{} npm x -- tauri {}',
    'echo "dev" | xargs -I{} npm exec -- tauri android {}',
    'echo "run" | xargs -I{} npm exec -- cargo {}',
    'echo "-rf /" | xargs -I{} npm exec -- rm {}',
    'echo "-rf /" | xargs npm exec -- rm',
    'echo "run" | xargs -I{} find . -maxdepth 0 -exec npm exec -- cargo {} \\;',
    // 6. cargo --bin dynamic & brace expansion
    'cargo --bin={anima-engine,anima-lab} build',
    'cargo build --manifest-path src-tauri/Cargo.toml --bin $(echo anima-engine)',
  ];
  for (const cmd of unsafeCmds) {
    if (validateCommand(cmd).ok) {
      failures.push(`safety check failed to reject '${cmd}'`);
    }
  }

  const safeCmds = [
    'rm -rf src-tauri/target/doc2-gate-neg\nmkdir -p src-tauri/target/doc2-gate-neg',
    'rm -rf /tmp/doc-commands-safe-dir',
    "cat <<'EOF' > m.json\n{\"note\": \"do not cargo run or npm run tauri:dev\"}\nEOF\n# cargo run in comment",
    'cat <<EOF-1 > n.txt\ncargo run\nEOF-1\necho ok',
    'cat <<-EOF > n2.txt\n\tcargo run\n\tEOF\necho ok',
    'cargo build --manifest-path src-tauri/Cargo.toml --release --bin anima-lab',
    '! node scripts/verify_e2_artifacts.mjs src-tauri/target/doc2-gate-neg',
    'echo "$(echo ok\necho done)"',
    'echo "$(\n# step 1)\necho ok\n)"',
    'env -S "echo ok"',
    'env -S "bash" -c "echo ok"',
    'npm exec -- eslint src',
    'npm -p run check:docs',
    'npm rum check:docs',
    'npm --color always run check:docs',
    'npm --workspaces run check:docs',
    '{ echo ok; echo done; }',
    'if true; then echo ok; fi',
    '> /dev/null',
    'cat < in.txt',
    'echo ok >| bash',
    '>|/tmp/safe_noclobber echo ok',
    'echo ok 2>&1 | tee log',
    'rm -rf src-tauri/target/doc2-gate-neg >| /tmp/rm.log',
    "bash -c - 'echo ok'",
    "sh -c 'echo $1' _ ok",
    'function safe_fn { echo ok; }; safe_fn',
    'echo hi | xargs -I{} echo {}',
    'echo "--test e2_seam_tests" | xargs cargo test --manifest-path src-tauri/Cargo.toml',
    "find . -name '*.md' -exec echo {} \\;",
    "trap 'rm -f /tmp/safe.lock' EXIT",
    'trap - EXIT',
    "echo $'hello\\nworld'",
    'RUST_LOG=info echo "$VAR"',
    'echo $(echo ok)',
    'npm --no-color run check:docs',
    'npm -s run check:docs',
    "cat <<'EOF'\ncargo run\nEOF\necho ok",
    'vite build',
    'vite --help',
    'tauri info',
    'tauri --help',
    'tauri -v info',
    'echo $(\necho ok\n)',
    "echo $(cat <<'EOF'\ncargo run\nEOF\n)",
    "echo $(\ncat <<'EOF'\nhello )\nEOF\n)",
    "cat <<'EOF'\n$(cargo run)\nEOF",
    'cat <<"EOF"\n$(cargo run)\nEOF',
    'cat <<\\EOF\n$(cargo run)\nEOF',
    'echo $(case x in x) echo ok ;; esac)',
    'echo $(case a in a) echo 1 ;| a) echo 2 ;; esac)',
    'echo {fd}>/dev/null ok',
    'echo "src" | xargs npm exec -- eslint',
    'ls file.{a,b}',
    'echo "${HOME:-/tmp}"',
    'for f in docs/*.md; do echo "$f"; done',
    'tauri android --help',
    'tauri ios --help',
  ];
  for (const cmd of safeCmds) {
    const res = validateCommand(cmd);
    if (!res.ok) {
      failures.push(`safety check falsely rejected safe command: ${res.reason}`);
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
    mkdirSync(workDir, { recursive: true });

    writeFileSync(
      join(tutDir, '01-start.md'),
      [
        '# Tutorial',
        '',
        '```bash',
        'echo unmarked-ignored >> order.log',
        '```',
        '',
        '<!-- doc-test -->',
        '```bash',
        'echo "tut-1" > order.log',
        '```',
        '',
        '<!-- doc-test -->',
        '',
        '```sh',
        'echo "tut-2" >> order.log',
        '```',
      ].join('\n'),
      'utf8',
    );

    writeFileSync(
      join(howToDir, '01-a.md'),
      [
        '# How-to A',
        '<!-- doc-test -->',
        '```zsh',
        'echo "howto-a" >> order.log',
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
        "cat <<'EOF' > payload.json",
        '{"forbidden_text": "cargo run && npm run tauri:dev"}',
        'EOF',
        'rm -rf /tmp/anima-selftest-nonexistent-subdir',
        'echo "howto-b" >> order.log',
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
      if (!dryRes.ok || existsSync(join(workDir, 'order.log'))) {
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
        const orderLines = readFileSync(join(workDir, 'order.log'), 'utf8').trim().split('\n');
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
    mkdirSync(negWork, { recursive: true });

    writeFileSync(
      join(negDir, 'fail.md'),
      [
        '# Fail fast test',
        '<!-- doc-test -->',
        '```bash',
        'touch step1.ok',
        '```',
        '<!-- doc-test -->',
        '```bash',
        'echo "boom" >&2',
        'exit 7',
        '```',
        '<!-- doc-test -->',
        '```bash',
        'touch step3.should_not_exist',
        '```',
      ].join('\n'),
      'utf8',
    );

    const negScan = scanDocDirectories([negDir], negWork);
    if (negScan.blocks.length !== 3) {
      failures.push(`negative control expected 3 blocks, got ${negScan.blocks.length}`);
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
        if (!existsSync(join(negWork, 'step1.ok'))) {
          failures.push('fail-fast control did not run block 1');
        }
        if (existsSync(join(negWork, 'step3.should_not_exist'))) {
          failures.push('fail-fast control executed block 3 after block 2 failed');
        }
      }
    }

    // Negative control: dangling/unclosed/non-shell/empty markers
    const badDir = join(tempRoot, 'neg-parse');
    mkdirSync(badDir, { recursive: true });
    writeFileSync(join(badDir, 'dangling.md'), '<!-- doc-test -->\n', 'utf8');
    writeFileSync(join(badDir, 'nonshell.md'), '<!-- doc-test -->\n```text\nhi\n```\n', 'utf8');
    writeFileSync(join(badDir, 'empty.md'), '<!-- doc-test -->\n```bash\n\n```\n', 'utf8');
    writeFileSync(join(badDir, 'unclosed.md'), '<!-- doc-test -->\n```bash\necho hi\n', 'utf8');
    writeFileSync(join(badDir, 'unsafe.md'), '<!-- doc-test -->\n```bash\ncargo run\n```\n', 'utf8');

    const badScan = scanDocDirectories([badDir], negWork);
    if (badScan.parseErrors.length !== 4) {
      failures.push(`parse negative control expected 4 errors, got ${badScan.parseErrors.length}`);
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
