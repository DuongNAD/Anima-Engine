// Refuse to land unresolved Git merge-conflict markers in tracked files.
//
// Background: commit `4b6258d` accidentally committed `<<<<<<< ours` / `=======` / `>>>>>>> theirs`
// blocks into `scripts/check_test_targets.mjs` and `src-tauri/tests/common/allocator.rs`, keeping CI
// red until fixed in `286e45a8`. This gate scans all git-tracked and untracked non-ignored files
// (or a target directory via `--dir <path>`) for 2-way and diff3 conflict markers while avoiding
// false positives on binary files, Markdown Setext H1 headings, Markdown fenced code examples, and
// long `={8,}` decorative dividers.
//
// Usage:
//   node scripts/check_conflict_markers.mjs
//   node scripts/check_conflict_markers.mjs --dir <path>
//   node scripts/check_conflict_markers.mjs --self-test

import { execFileSync } from 'node:child_process';
import { lstatSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { extname, join, relative, resolve } from 'node:path';
import process from 'node:process';
import { pathToFileURL } from 'node:url';

const BINARY_PROBE_BYTES = 8192;
const MARKDOWN_EXTENSIONS = new Set(['.md', '.markdown', '.mdx']);

const RE_START = /^<{7}(?:[ \t].*)?$/;
const RE_BASE = /^\|{7}(?:[ \t].*)?$/;
const RE_END = /^>{7}(?:[ \t].*)?$/;
const RE_DIV = /^={7}[ \t]*$/;

const RE_BACKTICK_FENCE_OPEN = /^ {0,3}(`{3,})([^`]*)$/;
const RE_TILDE_FENCE_OPEN = /^ {0,3}(~{3,})(.*)$/;

/**
 * Normalize a path to forward-slash separators for deterministic reporting.
 * @param {string} p
 * @returns {string}
 */
function toPosixPath(p) {
  return String(p).replaceAll('\\', '/');
}

/**
 * Heuristic binary check matching Git's NUL-byte convention in the first 8 KiB.
 * @param {Buffer | Uint8Array} buf
 * @returns {boolean}
 */
export function isBinaryBuffer(buf) {
  if (!buf || buf.length === 0) return false;
  return buf.subarray(0, BINARY_PROBE_BYTES).includes(0);
}

/**
 * Scan decoded UTF-8 text for Git merge-conflict markers.
 * @param {string} text
 * @param {string} relPath
 * @returns {Array<{ file: string, line: number, kind: string, message: string }>}
 */
export function scanText(text, relPath) {
  if (!text || text.length === 0) return [];

  const file = toPosixPath(relPath);
  const isMarkdown = MARKDOWN_EXTENSIONS.has(extname(file).toLowerCase());
  const rawLines = text.split('\n');
  const violations = [];

  let inConflict = false;
  let prevLineIsPlainText = false;
  /** @type {{ char: string, length: number } | null} */
  let fence = null;

  for (let i = 0; i < rawLines.length; i += 1) {
    const raw = rawLines[i];
    const line = raw.endsWith('\r') ? raw.slice(0, -1) : raw;
    const lineNumber = i + 1;

    if (isMarkdown) {
      if (fence !== null) {
        const closeRe = new RegExp(`^ {0,3}${fence.char}{${fence.length},}[ \\t]*$`);
        if (closeRe.test(line)) {
          fence = null;
        }
        prevLineIsPlainText = false;
        continue;
      }

      const fenceMatch = line.match(RE_BACKTICK_FENCE_OPEN) || line.match(RE_TILDE_FENCE_OPEN);
      if (fenceMatch) {
        fence = { char: fenceMatch[1][0], length: fenceMatch[1].length };
        prevLineIsPlainText = false;
        continue;
      }
    }

    if (RE_START.test(line)) {
      violations.push({
        file,
        line: lineNumber,
        kind: 'conflict-start',
        message: 'unresolved conflict start marker',
      });
      inConflict = true;
      prevLineIsPlainText = false;
      continue;
    }

    if (RE_BASE.test(line)) {
      violations.push({
        file,
        line: lineNumber,
        kind: 'conflict-base',
        message: 'unresolved diff3 base marker',
      });
      prevLineIsPlainText = false;
      continue;
    }

    if (RE_END.test(line)) {
      violations.push({
        file,
        line: lineNumber,
        kind: 'conflict-end',
        message: 'unresolved conflict end marker',
      });
      inConflict = false;
      prevLineIsPlainText = false;
      continue;
    }

    if (RE_DIV.test(line)) {
      if (isMarkdown && !inConflict && prevLineIsPlainText) {
        // CommonMark Setext H1 heading underline (`Title\n=======`).
        prevLineIsPlainText = false;
        continue;
      }
      violations.push({
        file,
        line: lineNumber,
        kind: 'conflict-divider',
        message: 'unresolved conflict divider marker',
      });
      prevLineIsPlainText = false;
      continue;
    }

    prevLineIsPlainText = line.trim().length > 0;
  }

  return violations;
}

/**
 * Scan a raw file buffer, skipping empty and binary buffers.
 * @param {Buffer} buf
 * @param {string} relPath
 * @returns {Array<{ file: string, line: number, kind: string, message: string }>}
 */
export function scanBuffer(buf, relPath) {
  if (!buf || buf.length === 0) return [];
  if (isBinaryBuffer(buf)) return [];
  return scanText(buf.toString('utf8'), relPath);
}

/**
 * Recursively scan a directory, skipping `.git`, `node_modules`, and symlinks.
 * @param {string} dirPath
 * @returns {{ root: string, scannedFiles: number, violations: Array<{ file: string, line: number, kind: string, message: string }> }}
 */
export function scanDirectory(dirPath) {
  const resolvedRoot = resolve(dirPath);
  const rootStat = statSync(resolvedRoot);
  if (!rootStat.isDirectory()) {
    throw new Error(`Not a directory: ${dirPath}`);
  }

  const violations = [];
  let scannedFiles = 0;

  /**
   * @param {string} currentDir
   */
  function walk(currentDir) {
    const entries = readdirSync(currentDir, { withFileTypes: true });
    entries.sort((a, b) => a.name.localeCompare(b.name));

    for (const entry of entries) {
      if (entry.isSymbolicLink()) continue;
      const fullPath = join(currentDir, entry.name);

      if (entry.isDirectory()) {
        if (entry.name === '.git' || entry.name === 'node_modules') continue;
        walk(fullPath);
        continue;
      }

      if (!entry.isFile()) continue;

      let buf;
      try {
        buf = readFileSync(fullPath);
      } catch {
        continue;
      }

      scannedFiles += 1;
      const rel = toPosixPath(relative(resolvedRoot, fullPath));
      violations.push(...scanBuffer(buf, rel));
    }
  }

  walk(resolvedRoot);
  return { root: resolvedRoot, scannedFiles, violations };
}

/**
 * Scan all git-tracked and untracked non-ignored files in a repository.
 * @param {string} [rootDir]
 * @returns {{ root: string, scannedFiles: number, violations: Array<{ file: string, line: number, kind: string, message: string }> }}
 */
export function scanRepo(rootDir = process.cwd()) {
  const resolvedRoot = resolve(rootDir);
  const output = execFileSync(
    'git',
    ['ls-files', '-z', '--cached', '--others', '--exclude-standard'],
    { cwd: resolvedRoot, encoding: 'utf8', maxBuffer: 1 << 28 },
  );

  const files = output.split('\0').filter(Boolean);
  const violations = [];
  let scannedFiles = 0;

  for (const rel of files) {
    const absPath = join(resolvedRoot, rel);
    let st;
    try {
      st = lstatSync(absPath);
    } catch {
      continue; // Deleted in working tree.
    }

    if (st.isSymbolicLink() || !st.isFile()) continue;

    let buf;
    try {
      buf = readFileSync(absPath);
    } catch {
      continue;
    }

    scannedFiles += 1;
    violations.push(...scanBuffer(buf, rel));
  }

  return { root: resolvedRoot, scannedFiles, violations };
}

/**
 * Run deterministic positive and negative self-tests.
 * Marker strings are built dynamically via `.repeat(7)` so this file never triggers itself.
 * @returns {{ ok: boolean, total: number, passed: number, failures: string[] }}
 */
export function runSelfTests() {
  const LT7 = '<'.repeat(7);
  const EQ7 = '='.repeat(7);
  const GT7 = '>'.repeat(7);
  const BAR7 = '|'.repeat(7);

  /** @type {Array<{ name: string, run: () => void }>} */
  const cases = [
    {
      name: 'clean TypeScript source returns zero violations',
      run: () => {
        const res = scanText('export const ok = 1;\n// divider ========\n', 'src/ok.ts');
        if (res.length !== 0) {
          throw new Error(`expected 0 violations, got ${res.length}`);
        }
      },
    },
    {
      name: 'empty buffer and binary buffer with NUL bytes return zero violations',
      run: () => {
        if (scanBuffer(Buffer.alloc(0), 'empty.txt').length !== 0) {
          throw new Error('expected empty buffer to return 0 violations');
        }
        const bin = Buffer.concat([
          Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x00, 0x0a]),
          Buffer.from(`${LT7} HEAD\n${EQ7}\n${GT7} branch\n`, 'utf8'),
        ]);
        if (!isBinaryBuffer(bin)) {
          throw new Error('expected isBinaryBuffer to return true');
        }
        if (scanBuffer(bin, 'image.png').length !== 0) {
          throw new Error('expected binary buffer to return 0 violations');
        }
      },
    },
    {
      name: '2-way conflict hunk reports exact lines in source file',
      run: () => {
        const src = ['fn main() {', `${LT7} HEAD`, '    let x = 1;', EQ7, '    let x = 2;', `${GT7} feat/x`, '}'].join('\n');
        const res = scanText(src, 'src/main.rs');
        const lines = res.map((v) => v.line);
        if (lines.join(',') !== '2,4,6') {
          throw new Error(`expected lines 2,4,6, got ${lines.join(',')}`);
        }
      },
    },
    {
      name: 'diff3 conflict hunk detects base marker',
      run: () => {
        const src = [`${LT7} HEAD`, 'ours', `${BAR7} base`, 'ancestor', EQ7, 'theirs', `${GT7} main`].join('\n');
        const res = scanText(src, 'src/mod.ts');
        const kinds = res.map((v) => `${v.line}:${v.kind}`);
        if (kinds.join(',') !== '1:conflict-start,3:conflict-base,5:conflict-divider,7:conflict-end') {
          throw new Error(`unexpected diff3 result: ${kinds.join(',')}`);
        }
      },
    },
    {
      name: 'stray single markers are caught in code files',
      run: () => {
        if (scanText(`const a = 1;\n${LT7} HEAD\n`, 'a.ts').map((v) => v.line).join(',') !== '2') {
          throw new Error('missed stray start marker');
        }
        if (scanText(`const a = 1;\n${GT7} branch\n`, 'b.ts').map((v) => v.line).join(',') !== '2') {
          throw new Error('missed stray end marker');
        }
        if (scanText(`const a = 1;\n${BAR7} base\n`, 'c.ts').map((v) => v.line).join(',') !== '2') {
          throw new Error('missed stray base marker');
        }
        if (scanText(`const a = 1;\n${EQ7}\n`, 'd.ts').map((v) => v.line).join(',') !== '2') {
          throw new Error('missed stray divider marker in .ts');
        }
      },
    },
    {
      name: 'CRLF line endings report accurate 1-based line numbers',
      run: () => {
        const crlf = `line1\r\n${LT7} HEAD\r\na\r\n${EQ7}\r\nb\r\n${GT7} main\r\n`;
        const lines = scanText(crlf, 'crlf.ts').map((v) => v.line);
        if (lines.join(',') !== '2,4,6') {
          throw new Error(`expected CRLF lines 2,4,6, got ${lines.join(',')}`);
        }
      },
    },
    {
      name: 'Markdown Setext H1 heading and fenced code examples do not false-positive',
      run: () => {
        const md = [
          'Document Title',
          EQ7,
          '',
          'Another Title',
          `${EQ7}   `,
          '',
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
          `${LT7} HEAD`,
          EQ7,
          `${GT7} main`,
          '~~~',
          '',
          `${'='.repeat(79)}`,
        ].join('\n');
        const res = scanText(md, 'docs/example.md');
        if (res.length !== 0) {
          throw new Error(`expected 0 violations in valid markdown, got ${JSON.stringify(res)}`);
        }
      },
    },
    {
      name: 'Markdown catches divider after blank line, inside open conflict, and after fence close',
      run: () => {
        const md = [
          '```',
          `${LT7} HEAD`,
          '```',
          EQ7,
          '',
          `${LT7} HEAD`,
          'Heading',
          EQ7,
          `${GT7} branch`,
        ].join('\n');
        const res = scanText(md, 'docs/broken.md');
        const lines = res.map((v) => v.line);
        if (lines.join(',') !== '4,6,8,9') {
          throw new Error(`expected lines 4,6,8,9, got ${lines.join(',')}`);
        }
      },
    },
  ];

  const failures = [];
  let passed = 0;

  for (const c of cases) {
    try {
      c.run();
      passed += 1;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      failures.push(`${c.name}: ${msg}`);
    }
  }

  return {
    ok: failures.length === 0,
    total: cases.length,
    passed,
    failures,
  };
}

/**
 * Parse CLI arguments.
 * @param {string[]} args
 * @returns {{ mode: 'repo' | 'dir' | 'self-test', dir?: string } | { error: string }}
 */
function parseArgs(args) {
  let mode = /** @type {'repo' | 'dir' | 'self-test' | null} */ (null);
  let dir;

  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === '--self-test') {
      if (mode !== null) {
        return { error: `Unexpected flag '${arg}' alongside '--${mode}'` };
      }
      mode = 'self-test';
    } else if (arg === '--dir') {
      if (mode !== null) {
        return { error: `Unexpected flag '${arg}' alongside '--${mode}'` };
      }
      const value = args[i + 1];
      if (!value || value.startsWith('-')) {
        return { error: 'Missing path value for --dir' };
      }
      mode = 'dir';
      dir = value;
      i += 1;
    } else if (arg.startsWith('--dir=')) {
      if (mode !== null) {
        return { error: `Unexpected flag '${arg}' alongside '--${mode}'` };
      }
      const value = arg.slice('--dir='.length);
      if (!value) {
        return { error: 'Missing path value for --dir=' };
      }
      mode = 'dir';
      dir = value;
    } else {
      return { error: `Unknown argument: ${arg}` };
    }
  }

  if (mode === null) return { mode: 'repo' };
  if (mode === 'dir') return { mode: 'dir', dir };
  return { mode: 'self-test' };
}

function printUsage() {
  console.error(
    'Usage: node scripts/check_conflict_markers.mjs [--dir <path> | --self-test]',
  );
}

function main() {
  const parsed = parseArgs(process.argv.slice(2));
  if ('error' in parsed) {
    console.error(`Error: ${parsed.error}`);
    printUsage();
    process.exit(2);
  }

  if (parsed.mode === 'self-test') {
    const result = runSelfTests();
    if (!result.ok) {
      for (const failure of result.failures) {
        console.error(`FAIL ${failure}`);
      }
      console.error(`self-test: ${result.passed}/${result.total} passed`);
      process.exit(1);
    }
    console.log(`self-test: ${result.passed}/${result.total} passed`);
    process.exit(0);
  }

  let scanResult;
  try {
    scanResult =
      parsed.mode === 'dir' && parsed.dir !== undefined
        ? scanDirectory(parsed.dir)
        : scanRepo(process.cwd());
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error(`Error: ${msg}`);
    process.exit(2);
  }

  if (scanResult.violations.length > 0) {
    for (const v of scanResult.violations) {
      console.error(`${v.file}:${v.line}: ${v.message}`);
    }
    console.error(
      `conflict markers: ${scanResult.violations.length} marker(s) found across ${scanResult.scannedFiles} file(s)`,
    );
    process.exit(1);
  }

  console.log(
    `conflict markers: ${scanResult.scannedFiles} file(s) scanned, 0 markers found`,
  );
  process.exit(0);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main();
}
