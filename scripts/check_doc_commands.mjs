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
const WRAPPER_COMMANDS = new Set(['env', 'command', 'nohup', 'time', 'nice']);

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

function scanShellLineForCommentAndHeredoc(line) {
  let quote = null;
  let commentStart = line.length;
  let heredocDelim = null;

  for (let i = 0; i < line.length; i += 1) {
    const ch = line[i];

    if (quote === "'") {
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

    if (ch === "'" || ch === '"') {
      quote = ch;
      continue;
    }

    if (ch === '#' && (i === 0 || /\s|[;|&(]/.test(line[i - 1]))) {
      commentStart = i;
      break;
    }

    if (ch === '<' && line[i + 1] === '<' && line[i + 2] !== '<' && heredocDelim === null) {
      const rest = line.slice(i + 2);
      const m = /^-?\s*(['"]?)([A-Za-z_][A-Za-z0-9_]*)\1/.exec(rest);
      if (m) {
        heredocDelim = m[2];
      }
    }
  }

  return {
    cleaned: line.slice(0, commentStart),
    heredocDelim,
  };
}

/**
 * Strip heredoc bodies and `#` comments outside quotes, returning logical lines with start offsets.
 */
export function stripHeredocsAndComments(script) {
  const rawLines = script.replace(/\r\n/g, '\n').split('\n');
  const strippedLines = [];
  let heredocDelim = null;

  for (let i = 0; i < rawLines.length; i += 1) {
    const line = rawLines[i];
    if (heredocDelim !== null) {
      if (line.trim() === heredocDelim) {
        heredocDelim = null;
      }
      strippedLines.push('');
      continue;
    }

    const { cleaned, heredocDelim: nextDelim } = scanShellLineForCommentAndHeredoc(line);
    strippedLines.push(cleaned);
    if (nextDelim !== null) {
      heredocDelim = nextDelim;
    }
  }

  const logical = [];
  let current = '';
  let startIdx = 0;
  let continuing = false;

  for (let i = 0; i < strippedLines.length; i += 1) {
    const part = strippedLines[i];
    if (!continuing) {
      startIdx = i;
      current = part;
    } else {
      current += ` ${part.trimStart()}`;
    }

    const trimmedEnd = current.replace(/[ \t]+$/, '');
    let trailingBackslashes = 0;
    for (let k = trimmedEnd.length - 1; k >= 0 && trimmedEnd[k] === '\\'; k -= 1) {
      trailingBackslashes += 1;
    }
    if (trailingBackslashes % 2 === 1) {
      current = trimmedEnd.slice(0, -1);
      continuing = true;
    } else {
      logical.push({ lineIndex: startIdx, text: current });
      current = '';
      continuing = false;
    }
  }

  if (continuing && current.trim().length > 0) {
    logical.push({ lineIndex: startIdx, text: current });
  }

  return logical;
}

function splitShellSegments(commandLine) {
  const segments = [];
  let quote = null;
  let current = '';

  const flush = () => {
    const trimmed = current.trim();
    if (trimmed) segments.push(trimmed);
    current = '';
  };

  for (let i = 0; i < commandLine.length; i += 1) {
    const ch = commandLine[i];

    if (quote === "'") {
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
      current += ch;
      if (ch === '"') quote = null;
      continue;
    }

    if (ch === '\\' && i + 1 < commandLine.length) {
      current += ch + commandLine[i + 1];
      i += 1;
      continue;
    }

    if (ch === "'" || ch === '"') {
      quote = ch;
      current += ch;
      continue;
    }

    if (ch === '$' && commandLine[i + 1] === '(') {
      flush();
      i += 1;
      continue;
    }

    if (ch === '`' || ch === ';' || ch === '(' || ch === ')') {
      flush();
      continue;
    }

    if ((ch === '&' && commandLine[i + 1] === '&') || (ch === '|' && commandLine[i + 1] === '|')) {
      flush();
      i += 1;
      continue;
    }

    if (ch === '&' || ch === '|') {
      flush();
      continue;
    }

    current += ch;
  }

  flush();
  return segments;
}

function tokenizeSegment(segment) {
  const tokens = [];
  let current = '';
  let quote = null;
  let hadToken = false;

  const pushCurrent = () => {
    if (hadToken || current.length > 0) {
      tokens.push(current);
    }
    current = '';
    hadToken = false;
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
      continue;
    }

    if (ch === '\\' && i + 1 < segment.length) {
      current += segment[i + 1];
      i += 1;
      hadToken = true;
      continue;
    }

    if (ch === "'" || ch === '"') {
      quote = ch;
      hadToken = true;
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

function stripCommandPrefixes(tokens) {
  let idx = 0;
  while (idx < tokens.length) {
    const tok = tokens[idx];
    if (tok === '!' || /^[A-Za-z_][A-Za-z0-9_]*=/.test(tok)) {
      idx += 1;
      continue;
    }
    const base = tok.split('/').pop() ?? tok;
    if (WRAPPER_COMMANDS.has(base)) {
      idx += 1;
      while (idx < tokens.length && tokens[idx].startsWith('-')) {
        if (base === 'nice' && tokens[idx] === '-n' && idx + 1 < tokens.length) {
          idx += 2;
        } else {
          idx += 1;
        }
      }
      continue;
    }
    break;
  }
  return tokens.slice(idx);
}

function isDangerousRmTarget(rawTarget) {
  if (!rawTarget) return true;
  const trimmed = rawTarget.trim();
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

function checkSegmentSafety(segment) {
  const rawTokens = tokenizeSegment(segment);
  const tokens = stripCommandPrefixes(rawTokens);
  if (tokens.length === 0) return null;

  const cmd = tokens[0];
  const baseCmd = cmd.split('/').pop() ?? cmd;

  if (baseCmd === 'npm' && tokens[1] === 'run') {
    const scriptName = tokens[2] ?? '';
    if (FORBIDDEN_NPM_SCRIPTS.has(scriptName)) {
      return `forbidden desktop/dev script 'npm run ${scriptName}'`;
    }
  }

  if (baseCmd === 'npx' && tokens[1] === 'tauri' && (tokens[2] === 'dev' || tokens[2] === 'build')) {
    return `forbidden desktop command 'npx tauri ${tokens[2]}'`;
  }

  if (baseCmd === 'tauri' && (tokens[1] === 'dev' || tokens[1] === 'build')) {
    return `forbidden desktop command 'tauri ${tokens[1]}'`;
  }

  if (baseCmd === 'npx' && tokens[1] === 'vite' && (tokens.length === 2 || tokens[2] === 'preview')) {
    return `forbidden dev server command 'npx vite${tokens[2] ? ` ${tokens[2]}` : ''}'`;
  }

  if (baseCmd === 'vite' && (tokens.length === 1 || tokens[1] === 'preview')) {
    return `forbidden dev server command 'vite${tokens[1] ? ` ${tokens[1]}` : ''}'`;
  }

  if (baseCmd === 'anima-engine' || baseCmd === 'anima-engine.exe') {
    return `forbidden direct desktop binary invocation '${cmd}'`;
  }

  if (baseCmd === 'cargo') {
    const subIdx = tokens.findIndex((t, i) => i > 0 && !t.startsWith('-') && !t.startsWith('+'));
    const sub = subIdx >= 0 ? tokens[subIdx] : '';
    if (sub === 'run') {
      return "forbidden 'cargo run' invocation (build the release binary first and invoke it directly)";
    }
    const binFlagIdx = tokens.indexOf('--bin');
    if (binFlagIdx >= 0 && tokens[binFlagIdx + 1] === 'anima-engine') {
      return "forbidden '--bin anima-engine' desktop target";
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
      if (tok === '>' || tok === '>>' || tok === '2>' || tok === '1>') {
        i += 1;
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

/**
 * Validate a single doc-test block's script against safety rules.
 */
export function validateCommand(script) {
  if (typeof script !== 'string' || script.trim().length === 0) {
    return { ok: false, reason: 'empty command block', lineIndex: 0, lineText: '' };
  }

  const logicalLines = stripHeredocsAndComments(script);
  for (const entry of logicalLines) {
    if (!entry.text.trim()) continue;
    const segments = splitShellSegments(entry.text);
    for (const seg of segments) {
      const reason = checkSegmentSafety(seg);
      if (reason) {
        return {
          ok: false,
          reason,
          lineIndex: entry.lineIndex,
          lineText: seg,
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
    'cargo run --bin anima-lab',
    'npx tauri dev',
    'vite preview',
    './src-tauri/target/release/anima-engine',
    'rm -rf /',
    'rm -rf $HOME',
    'rm -rf .',
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
    '! node scripts/verify_e2_artifacts.mjs src-tauri/target/doc2-gate-neg',
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
