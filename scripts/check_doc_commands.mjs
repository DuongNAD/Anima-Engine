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
const STANDALONE_REDIR_RE = /^(?:\d*)(?:>>|>\||>|<>|<<<|<<-|<<|<|&>>|&>|>&|<&)$/;
const ATTACHED_REDIR_RE = /^(?:\d*(?:>>|>\||>|<>|<<<|<<-|<<|<|>&|<&)|&>>|&>).+$/;
const INPUT_REDIR_OP_RE = /^(?:\d*)(?:<<<|<<-|<<|<&|<>|<)$/;
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
  if (idx >= rest.length) return null;

  let delim = '';
  let hasToken = false;
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
    if (q === '"') {
      if (c === '\\' && idx + 1 < rest.length) {
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
    if (c === "'" || c === '"') {
      q = c;
      hasToken = true;
      idx += 1;
      continue;
    }
    if (c === '\\' && idx + 1 < rest.length) {
      delim += rest[idx + 1];
      hasToken = true;
      idx += 2;
      continue;
    }
    delim += c;
    hasToken = true;
    idx += 1;
  }

  if (!hasToken || q !== null) return null;
  return { delim, stripTabs, consumed: idx };
}

function scanShellLineForCommentAndHeredoc(line, initialQuote = null) {
  let quote = initialQuote;
  let commentStart = line.length;
  const heredocSpecs = [];

  for (let i = 0; i < line.length; i += 1) {
    const ch = line[i];

    if (quote === "'") {
      if (ch === "'") quote = null;
      continue;
    }

    if (quote === "$'") {
      if (ch === '\\') {
        i += 1;
        continue;
      }
      if (ch === "'") quote = null;
      continue;
    }

    if (quote === '"') {
      if (ch === '\\') {
        i += 1;
        continue;
      }
      if (ch === '"') quote = null;
      continue;
    }

    if (ch === '\\') {
      i += 1;
      continue;
    }

    if (ch === '$' && line[i + 1] === "'") {
      quote = "$'";
      i += 1;
      continue;
    }

    if (ch === "'" || ch === '"') {
      quote = ch;
      continue;
    }

    if (ch === '#' && (i === 0 || /\s|[;|&(]/.test(line[i - 1]))) {
      commentStart = i;
      break;
    }

    if (ch === '<' && line[i + 1] === '<' && line[i + 2] !== '<') {
      const spec = parseHeredocSpec(line.slice(i + 2));
      if (spec) {
        heredocSpecs.push({ delim: spec.delim, stripTabs: spec.stripTabs });
        i += 1 + spec.consumed;
      }
    }
  }

  return {
    cleaned: line.slice(0, commentStart),
    heredocSpecs,
    endQuote: quote,
  };
}

/**
 * Strip heredoc bodies and `#` comments outside quotes, returning logical lines with start offsets.
 */
export function stripHeredocsAndComments(script) {
  const rawLines = script.replace(/\r\n/g, '\n').split('\n');
  const strippedLines = [];
  const heredocQueue = [];
  let quote = null;

  for (let i = 0; i < rawLines.length; i += 1) {
    const line = rawLines[i];
    if (heredocQueue.length > 0) {
      const active = heredocQueue[0];
      const candidate = active.stripTabs ? line.replace(/^\t+/, '') : line;
      if (candidate === active.delim) {
        heredocQueue.shift();
      }
      strippedLines.push({ text: '', endQuote: quote });
      continue;
    }

    const { cleaned, heredocSpecs, endQuote } = scanShellLineForCommentAndHeredoc(line, quote);
    quote = endQuote;
    strippedLines.push({ text: cleaned, endQuote });
    if (heredocSpecs.length > 0) {
      heredocQueue.push(...heredocSpecs);
    }
  }

  const logical = [];
  let current = '';
  let startIdx = 0;
  let continuing = false;
  let continueWithNewline = false;

  for (let i = 0; i < strippedLines.length; i += 1) {
    const { text: part, endQuote } = strippedLines[i];
    if (!continuing) {
      startIdx = i;
      current = part;
    } else if (continueWithNewline) {
      current += `\n${part}`;
    } else {
      current += part;
    }

    const trimmedEnd = current.replace(/[ \t]+$/, '');
    let trailingBackslashes = 0;
    for (let k = trimmedEnd.length - 1; k >= 0 && trimmedEnd[k] === '\\'; k -= 1) {
      trailingBackslashes += 1;
    }

    if (endQuote !== "'" && endQuote !== "$'" && trailingBackslashes % 2 === 1) {
      current = trimmedEnd.slice(0, -1);
      continuing = true;
      continueWithNewline = false;
    } else if (endQuote !== null) {
      continuing = true;
      continueWithNewline = true;
    } else {
      logical.push({ lineIndex: startIdx, text: current });
      current = '';
      continuing = false;
      continueWithNewline = false;
    }
  }

  if (continuing && current.trim().length > 0) {
    logical.push({ lineIndex: startIdx, text: current });
  }

  return logical;
}

function findClosingCommandSubParen(str, startIdx) {
  let depth = 1;
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
      if (ch === '"') q = null;
      continue;
    }
    if (ch === '\\' && i + 1 < str.length) {
      i += 1;
      continue;
    }
    if (ch === '$' && str[i + 1] === "'") {
      q = "$'";
      i += 1;
      continue;
    }
    if (ch === "'" || ch === '"') {
      q = ch;
      continue;
    }
    if (ch === '#' && (i === startIdx || /\s|[;|&(]/.test(str[i - 1]))) {
      const nl = str.indexOf('\n', i + 1);
      if (nl < 0) return -1;
      i = nl;
      continue;
    }
    if (ch === '$' && str[i + 1] === '(') {
      const innerEnd = findClosingCommandSubParen(str, i + 2);
      if (innerEnd < 0) return -1;
      i = innerEnd;
      continue;
    }
    if (ch === '(') {
      depth += 1;
      continue;
    }
    if (ch === ')') {
      depth -= 1;
      if (depth === 0) return i;
    }
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

function extractNestedCommandSubSegments(inner) {
  const nested = [];
  const logicalLines = stripHeredocsAndComments(inner);
  for (const entry of logicalLines) {
    if (!entry.text.trim()) continue;
    nested.push(...splitShellSegmentsWithMeta(entry.text));
  }
  return nested;
}

function splitShellSegmentsWithMeta(commandLine) {
  const segments = [];
  let quote = null;
  let current = '';
  let nextPipedIn = false;

  const flush = (setNextPipedIn = false, endedAtSub = false) => {
    const trimmed = current.trim();
    if (trimmed) {
      segments.push({ text: trimmed, pipedIn: nextPipedIn, endedAtSub });
      nextPipedIn = setNextPipedIn;
    } else if (setNextPipedIn) {
      nextPipedIn = true;
    } else if (endedAtSub) {
      nextPipedIn = false;
    }
    current = '';
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
          segments.push(...extractNestedCommandSubSegments(inner));
          current += commandLine.slice(i, endIdx + 1);
          i = endIdx;
          continue;
        }
        segments.push(...extractNestedCommandSubSegments(commandLine.slice(i + 2)));
      }
      if (ch === '`') {
        const bt = extractDoubleQuotedBacktick(commandLine, i + 1);
        if (bt) {
          segments.push(...extractNestedCommandSubSegments(bt.inner));
          current += commandLine.slice(i, bt.endIdx + 1);
          i = bt.endIdx;
          continue;
        }
        segments.push(...extractNestedCommandSubSegments(commandLine.slice(i + 1)));
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

    if (ch === "'" || ch === '"') {
      quote = ch;
      current += ch;
      continue;
    }

    if (ch === '$' && commandLine[i + 1] === '(') {
      flush(false, true);
      i += 1;
      continue;
    }

    if ((ch === '<' || ch === '>') && commandLine[i + 1] === '(') {
      const endIdx = findClosingCommandSubParen(commandLine, i + 2);
      if (endIdx >= 0) {
        const inner = commandLine.slice(i + 2, endIdx);
        segments.push(...extractNestedCommandSubSegments(inner));
        current += commandLine.slice(i, endIdx + 1);
        i = endIdx;
        continue;
      }
      segments.push(...extractNestedCommandSubSegments(commandLine.slice(i + 2)));
    }

    if (ch === '`') {
      flush(false, true);
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

  flush(false, false);
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

  const pushCurrent = () => {
    if (hadToken || current.length > 0) {
      tokens.push({ text: current, isRedirOp: false });
    }
    current = '';
    hadToken = false;
    currentHasQuoteOrEscape = false;
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
      current += segment[i + 1];
      i += 1;
      hadToken = true;
      currentHasQuoteOrEscape = true;
      continue;
    }

    if (ch === '$' && segment[i + 1] === "'") {
      quote = "$'";
      hadToken = true;
      currentHasQuoteOrEscape = true;
      i += 1;
      continue;
    }

    if (ch === "'" || ch === '"') {
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
      tokens.push({ text: op, isRedirOp: true });
      i += op.length - 1;
      continue;
    }

    if ((ch === '>' || ch === '<') && segment[i + 1] === '(') {
      const endIdx = findClosingCommandSubParen(segment, i + 2);
      if (endIdx >= 0) {
        current += segment.slice(i, endIdx + 1);
        hadToken = true;
        currentHasQuoteOrEscape = true;
        i = endIdx;
        continue;
      }
    }

    if ((ch === '>' || ch === '<') && segment[i + 1] !== '(') {
      let fdPrefix = '';
      if (!currentHasQuoteOrEscape && current.length > 0 && /^\d+$/.test(current)) {
        fdPrefix = current;
        current = '';
        hadToken = false;
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
      tokens.push({ text: `${fdPrefix}${op}`, isRedirOp: true });
      i += op.length - 1;
      continue;
    }

    if (/\s/.test(ch)) {
      pushCurrent();
      continue;
    }

    current += ch;
    hadToken = true;
  }

  pushCurrent();
  return tokens;
}

function tokenizeSegment(segment) {
  return tokenizeSegmentMeta(segment).map((t) => t.text);
}

function stripRedirections(metaTokens) {
  const tokens = [];
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
  }
  return { tokens, redirs };
}

function validateEnvSplitStringPayload(payload, depth, hasRemaining = false) {
  if (payload.trim().length === 0) return null;
  if (hasRemaining) {
    const logicalLines = stripHeredocsAndComments(payload);
    const segments = [];
    for (const entry of logicalLines) {
      if (!entry.text.trim()) continue;
      segments.push(...splitShellSegmentsWithMeta(entry.text));
    }
    if (segments.length <= 1) {
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

function stripCommandPrefixes(rawTokens, depth = 0, redirs = []) {
  if (depth > MAX_SHELL_RECURSION_DEPTH) {
    return {
      tokens: [],
      error: `nested shell command recursion depth exceeded (>${MAX_SHELL_RECURSION_DEPTH})`,
    };
  }

  const tokens = [...rawTokens];
  let idx = 0;
  while (idx < tokens.length) {
    const tok = tokens[idx];
    if (FORBIDDEN_STARTUP_ENV_ASSIGN_RE.test(tok)) {
      return {
        tokens: [],
        error: `forbidden shell startup file variable assignment '${tok}'`,
      };
    }
    if (tok === 'function' && idx + 1 < tokens.length) {
      idx += 2;
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
          error: "forbidden 'exec' with input redirection",
        };
      }
      idx += 1;
      const envSplitTokens = [];
      while (idx < tokens.length) {
        if (
          base === 'env' &&
          envSplitTokens.some((t) => !t.startsWith('-') && !/^[A-Za-z_][A-Za-z0-9_]*=/.test(t))
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
            if (err) return { tokens: [], error: err };
            envSplitTokens.push(...tokenizeSegment(payload));
            idx += 2;
            continue;
          }
          if (flag.startsWith('--split-string=')) {
            const payload = flag.slice('--split-string='.length);
            const err = validateEnvSplitStringPayload(payload, depth, idx + 1 < tokens.length);
            if (err) return { tokens: [], error: err };
            envSplitTokens.push(...tokenizeSegment(payload));
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
                if (err) return { tokens: [], error: err };
                envSplitTokens.push(...tokenizeSegment(payload));
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
      if (base === 'env' && envSplitTokens.length > 0) {
        const remaining = tokens.slice(idx);
        const firstNonAssign = envSplitTokens.find((t) => !/^[A-Za-z_][A-Za-z0-9_]*=/.test(t));
        const spliced =
          firstNonAssign && firstNonAssign.startsWith('-')
            ? ['env', ...envSplitTokens, ...remaining]
            : [...envSplitTokens, ...remaining];
        return stripCommandPrefixes(spliced, depth + 1, redirs);
      }
      continue;
    }
    break;
  }
  return { tokens: tokens.slice(idx), error: null };
}

function isDangerousRmTarget(rawTarget) {
  if (!rawTarget) return true;
  const trimmed = rawTarget.trim();
  if (hasDynamicShellExpansion(trimmed)) return true;
  if (trimmed === '/' || /^\/+\*?$/.test(trimmed)) return true;
  if (trimmed === '~' || trimmed.startsWith('~/')) return true;
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

function isKnownNpmBooleanOrShortFlag(tok) {
  if (NPM_KNOWN_BOOLEAN_FLAGS.has(tok)) return true;
  if (/^-[A-Za-z]+$/.test(tok)) {
    return [...tok.slice(1)].every((ch) => NPM_SHORT_BOOL_CHARS.has(ch));
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
  if (tok === '--color') {
    return next === 'always' || next === 'true' || next === 'false';
  }
  if (NPM_BOOLEAN_CONFIG_FLAGS.has(tok)) {
    return next === 'true' || next === 'false';
  }
  if (/^-[A-Za-z]{2,}$/.test(tok)) {
    const prefix = tok.slice(1, -1);
    const last = tok[tok.length - 1];
    const allPrefixBool = [...prefix].every(
      (ch) => NPM_SHORT_BOOL_CHARS.has(ch) && !valueFlags.has(`-${ch}`),
    );
    if (allPrefixBool && valueFlags.has(`-${last}`)) {
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
      if (/^(?:\d*)(?:<<<|<<-|<<|<&|<>|<)/.test(tok)) {
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

function checkDynamicSubcommand(subToks, placeholders, contextLabel, isFindContext = false) {
  if (subToks.length === 0) return null;
  const subBase = subToks[0].split('/').pop() ?? subToks[0];

  if (!isFindContext && containsAnyPlaceholder(subToks[0], placeholders)) {
    return `forbidden '${contextLabel}' invocation with placeholder command '${subToks[0]}'`;
  }

  if (subBase === 'cargo') {
    if (!hasExplicitSafeCargoSubcommand(subToks)) {
      return `forbidden '${contextLabel} cargo' invocation without an explicit safe subcommand`;
    }
    if (!isFindContext && placeholders.size > 0) {
      for (let i = 1; i < subToks.length; i += 1) {
        const ct = subToks[i];
        if (ct.startsWith('--bin=') && containsAnyPlaceholder(ct.slice('--bin='.length), placeholders)) {
          return `forbidden '${contextLabel} cargo' invocation with placeholder '--bin' target`;
        }
        if (ct === '--bin' && containsAnyPlaceholder(subToks[i + 1] ?? '', placeholders)) {
          return `forbidden '${contextLabel} cargo' invocation with placeholder '--bin' target`;
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

  if (subBase === 'tauri' && (subToks.length === 1 || containsAnyPlaceholder(subToks[1] ?? '', placeholders))) {
    return `forbidden '${contextLabel} tauri' without safe subcommand`;
  }

  if (subBase === 'npm') {
    let subIdx = -1;
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
      if (st.startsWith('-')) continue;
      subIdx = i;
      break;
    }
    const npmSub = subIdx >= 0 ? subToks[subIdx] : '';
    if (!npmSub || containsAnyPlaceholder(npmSub, placeholders)) {
      return `forbidden '${contextLabel} npm' without an explicit subcommand`;
    }
    if (NPM_RUN_SUBCOMMANDS.has(npmSub)) {
      let scriptName = '';
      for (let j = subIdx + 1; j < subToks.length; j += 1) {
        const st = subToks[j];
        if (st === '--') {
          scriptName = subToks[j + 1] ?? '';
          break;
        }
        if (consumesNpmFlagValue(subToks, j, NPM_VALUE_FLAGS)) {
          j += 1;
          continue;
        }
        if (st.startsWith('-')) continue;
        scriptName = st;
        break;
      }
      if (!scriptName || containsAnyPlaceholder(scriptName, placeholders)) {
        return `forbidden '${contextLabel} npm ${npmSub}' without an explicit script name`;
      }
    } else if (npmSub === 'exec' || npmSub === 'x') {
      let execCmd = '';
      let hasCallPayload = false;
      for (let j = subIdx + 1; j < subToks.length; j += 1) {
        const st = subToks[j];
        if (st === '--') {
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
        if (st.startsWith('-')) continue;
        execCmd = st;
        break;
      }
      if ((!execCmd && !hasCallPayload) || containsAnyPlaceholder(execCmd, placeholders)) {
        return `forbidden '${contextLabel} npm ${npmSub}' without an explicit command`;
      }
    }
  }

  if (subBase === 'npx') {
    let idx = 1;
    let hasCallPayload = false;
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
        idx += 1;
        continue;
      }
      break;
    }
    const npxTarget = subToks[idx] ?? '';
    const npxArg = subToks[idx + 1] ?? '';
    if (
      (!npxTarget && !hasCallPayload) ||
      containsAnyPlaceholder(npxTarget, placeholders) ||
      (npxTarget === 'tauri' && (idx + 1 >= subToks.length || containsAnyPlaceholder(npxArg, placeholders)))
    ) {
      return `forbidden '${contextLabel} npx' without an explicit safe command`;
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
        if (st === '--recursive' || (!st.startsWith('--') && /[rR]/.test(st.slice(1)))) {
          hasRecursive = true;
        }
        continue;
      }
      targets.push(st);
    }
    if (hasRecursive && (targets.length === 0 || targets.some((t) => containsAnyPlaceholder(t, placeholders)))) {
      return `forbidden '${contextLabel} rm -r' without explicit safe target paths`;
    }
  }

  return null;
}

function checkTokensSafety(rawTokens, depth = 0, pipedIn = false, endedAtSub = false, redirs = []) {
  if (depth > MAX_SHELL_RECURSION_DEPTH) {
    return `nested shell command recursion depth exceeded (>${MAX_SHELL_RECURSION_DEPTH})`;
  }
  const stripped = stripCommandPrefixes(rawTokens, depth, redirs);
  if (stripped.error) return stripped.error;
  const tokens = stripped.tokens;
  if (tokens.length === 0) return null;

  const cmd = tokens[0];
  if (hasDynamicShellExpansion(cmd)) {
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
        if (IMPLICIT_FOR_LOOP_RE.test(subScript)) {
          return `forbidden '${baseCmd} -c' with implicit positional 'for' loop in inline script`;
        }
        if (subScript.trim().length > 0) {
          const subRes = validateCommand(subScript, depth + 1);
          if (!subRes.ok) {
            return `forbidden nested '${baseCmd} -c' command: ${subRes.reason}`;
          }
        }
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
          if (/^(?:\d*)(?:<<<|<<-|<<|<&|<>|<)/.test(t)) {
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
        .some((t) => INPUT_REDIR_OP_RE.test(t) || /^(?:\d*)(?:<<<|<<-|<<|<&|<>|<).+/.test(t));
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
      const nestedReason = checkTokensSafety(subTokens, depth + 1, false, false);
      if (nestedReason) {
        return `forbidden nested 'xargs' command: ${nestedReason}`;
      }
      const strippedSub = stripCommandPrefixes(subTokens, depth + 1);
      if (strippedSub.error) {
        return `forbidden nested 'xargs' command: ${strippedSub.error}`;
      }
      if (strippedSub.tokens.length === 0) {
        return "forbidden 'xargs' wrapper invocation with no explicit target command";
      }
      const dynamicReason = checkDynamicSubcommand(strippedSub.tokens, xargsPlaceholders, 'xargs', false);
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
        if (subTokens.length > 0) {
          const nestedReason = checkTokensSafety(subTokens, depth + 1, false, false);
          if (nestedReason) {
            return `forbidden nested 'find ${tok}' command: ${nestedReason}`;
          }
          const strippedSub = stripCommandPrefixes(subTokens, depth + 1);
          if (!strippedSub.error && strippedSub.tokens.length > 0) {
            const dynamicReason = checkDynamicSubcommand(
              strippedSub.tokens,
              new Set(['{}']),
              `find ${tok}`,
              true,
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
        if (!tok.includes('=') && !isKnownNpmBooleanOrShortFlag(tok) && !tok.startsWith('--no-')) {
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
          if (!tok.includes('=') && !isKnownNpmBooleanOrShortFlag(tok) && !tok.startsWith('--no-')) {
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
      if (hasDynamicShellExpansion(scriptName)) {
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
            if (hasDynamicShellExpansion(afterDash)) {
              return `forbidden dynamic script name '${afterDash}' in 'npm ${subCmd}'`;
            }
            break;
          }
          if (tok.startsWith('-')) continue;
          if (FORBIDDEN_NPM_SCRIPTS.has(tok)) {
            return `forbidden desktop/dev script 'npm ${subCmd} ${tok}'`;
          }
          if (hasDynamicShellExpansion(tok)) {
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
          if (!tok.includes('=') && !isKnownNpmBooleanOrShortFlag(tok) && !tok.startsWith('--no-')) {
            sawUnrecognizedExecFlag = true;
          }
          continue;
        }
        cmdIdx = j;
        break;
      }

      if (cmdIdx >= 0 && cmdIdx < tokens.length) {
        const resolved = tokens.slice(cmdIdx);
        const nestedReason = checkTokensSafety(resolved, depth + 1);
        if (nestedReason) {
          return nestedReason;
        }
      }
      if (sawUnrecognizedExecFlag) {
        for (let k = subIdx + 1; k < tokens.length; k += 1) {
          const tok = tokens[k];
          if (tok === '--') {
            if (k + 1 < tokens.length) {
              const nestedReason = checkTokensSafety(tokens.slice(k + 1), depth + 1);
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
          }
        }
      }
    } else if (sawUnrecognizedPreFlag) {
      for (let k = 1; k < tokens.length; k += 1) {
        const tok = tokens[k];
        if (tok === '--') {
          if (k + 1 < tokens.length) {
            const fallbackReason = checkTokensSafety(['npm', ...tokens.slice(k + 1)], depth + 1);
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
          const fallbackReason = checkTokensSafety(['npm', ...tokens.slice(k)], depth + 1);
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
        if (!tok.includes('=') && !isKnownNpmBooleanOrShortFlag(tok) && !tok.startsWith('--no-')) {
          sawUnrecognizedNpxFlag = true;
        }
        cmdIdx += 1;
        continue;
      }
      break;
    }
    const npxCmd = tokens[cmdIdx] ?? '';
    const npxArg = tokens[cmdIdx + 1] ?? '';
    if (npxCmd === 'tauri' && (npxArg === 'dev' || npxArg === 'build' || hasDynamicShellExpansion(npxArg))) {
      return `forbidden desktop command 'npx tauri ${npxArg}'`;
    }
    if (
      npxCmd === 'vite' &&
      (cmdIdx + 1 >= tokens.length || npxArg === 'preview' || hasDynamicShellExpansion(npxArg))
    ) {
      return `forbidden dev server command 'npx vite${npxArg ? ` ${npxArg}` : ''}'`;
    }
    if (cmdIdx < tokens.length) {
      const nestedReason = checkTokensSafety(tokens.slice(cmdIdx), depth + 1);
      if (nestedReason) {
        return nestedReason;
      }
    }
    if (sawUnrecognizedNpxFlag) {
      for (let k = 1; k < tokens.length; k += 1) {
        const tok = tokens[k];
        if (tok === '--') {
          if (k + 1 < tokens.length) {
            const nestedReason = checkTokensSafety(['npx', ...tokens.slice(k)], depth + 1);
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
        }
      }
    }
  }

  if (
    baseCmd === 'tauri' &&
    (tokens[1] === 'dev' || tokens[1] === 'build' || hasDynamicShellExpansion(tokens[1] ?? ''))
  ) {
    return `forbidden desktop command 'tauri ${tokens[1]}'`;
  }

  if (
    baseCmd === 'vite' &&
    (tokens.length === 1 || tokens[1] === 'preview' || hasDynamicShellExpansion(tokens[1] ?? ''))
  ) {
    return `forbidden dev server command 'vite${tokens[1] ? ` ${tokens[1]}` : ''}'`;
  }

  if (baseCmd === 'anima-engine' || baseCmd === 'anima-engine.exe') {
    return `forbidden direct desktop binary invocation '${cmd}'`;
  }

  if (baseCmd === 'cargo') {
    let sub = '';
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
      break;
    }

    if (sub === 'run' || sub === 'r' || hasDynamicShellExpansion(sub)) {
      return "forbidden 'cargo run' invocation (build the release binary first and invoke it directly)";
    }

    for (let i = 1; i < tokens.length; i += 1) {
      const tok = tokens[i];
      if (tok === '--bin=anima-engine' || tok === '--bin=anima-engine.exe') {
        return "forbidden '--bin anima-engine' desktop target";
      }
      if (tok === '--bin' && (tokens[i + 1] === 'anima-engine' || tokens[i + 1] === 'anima-engine.exe')) {
        return "forbidden '--bin anima-engine' desktop target";
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
  const { tokens, redirs } = stripRedirections(metaTokens);
  return checkTokensSafety(tokens, depth, pipedIn, endedAtSub, redirs);
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
    if (!entry.text.trim()) continue;
    const segments = splitShellSegmentsWithMeta(entry.text);
    for (const seg of segments) {
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
