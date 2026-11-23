#!/usr/bin/env node
// Extract and verify `<!-- doc-test -->` fenced shell blocks in docs/tutorials and docs/how-to.
// Usage: node scripts/check_doc_commands.mjs [--dir <path>]... [--cwd <path>] [--static]

import { spawnSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { basename, join, relative, resolve } from 'node:path';
import process from 'node:process';
import { pathToFileURL } from 'node:url';

const DEFAULT_SUBDIRS = ['docs/tutorials', 'docs/how-to'];
const ALLOWED_LANGS = new Set(['', 'bash', 'sh']);
const cmpAscii = (a, b) => (a < b ? -1 : a > b ? 1 : 0);

export function parseArgs(argv) {
  const opts = { dirs: [], cwd: null, staticOnly: false, help: false };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--help' || a === '-h') opts.help = true;
    else if (a === '--static') opts.staticOnly = true;
    else if (a === '--dir' && i + 1 < argv.length) { opts.dirs.push(argv[i + 1]); i += 1; }
    else if (a.startsWith('--dir=')) opts.dirs.push(a.slice('--dir='.length));
    else if (a === '--cwd' && i + 1 < argv.length) { opts.cwd = argv[i + 1]; i += 1; }
    else if (a.startsWith('--cwd=')) opts.cwd = a.slice('--cwd='.length);
    else throw new Error(`Unknown or incomplete option: ${a}`);
  }
  return opts;
}

export function collectMarkdownFiles(dirs, cwd) {
  const targets = dirs.length > 0 ? dirs : DEFAULT_SUBDIRS;
  const files = [];
  const seen = new Set();
  const walk = (dir, out) => {
    for (const e of readdirSync(dir, { withFileTypes: true }).sort((a, b) => cmpAscii(a.name, b.name))) {
      const abs = join(dir, e.name);
      if (e.isDirectory()) walk(abs, out);
      else if (e.isFile() && e.name.endsWith('.md')) {
        out.push({ absPath: abs, relPath: relative(cwd, abs).replace(/\\/g, '/') });
      }
    }
  };
  for (const dir of targets) {
    const abs = resolve(cwd, dir);
    if (!existsSync(abs) || !statSync(abs).isDirectory()) throw new Error(`Doc directory not found: ${dir}`);
    const local = [];
    walk(abs, local);
    local.sort((a, b) => cmpAscii(a.relPath, b.relPath));
    for (const f of local) {
      if (!seen.has(f.absPath)) { seen.add(f.absPath); files.push(f); }
    }
  }
  return files;
}

export function extractDocCommands(markdown, relPath) {
  const lines = String(markdown ?? '').split(/\r?\n/);
  const blocks = [];
  const errors = [];
  let outer = null;

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    if (outer) {
      if (new RegExp(`^\\s*${outer.char}{${outer.len},}\\s*$`).test(line)) outer = null;
      continue;
    }
    const open = /^\s*(`{3,}|~{3,})(.*)$/.exec(line);
    if (open) { outer = { char: open[1][0], len: open[1].length }; continue; }
    if (line.trim() !== '<!-- doc-test -->') continue;

    const markerLine = i + 1;
    let j = i + 1;
    while (j < lines.length && lines[j].trim() === '') j += 1;
    const inner = j < lines.length ? /^\s*(`{3,}|~{3,})\s*([^\s`~]*)\s*$/.exec(lines[j]) : null;
    if (!inner) {
      const msg = j >= lines.length ? '<!-- doc-test --> has no following code fence' : '<!-- doc-test --> must precede a code fence';
      errors.push({ file: relPath, line: markerLine, message: `${relPath}:${markerLine}: ${msg}` });
      if (j < lines.length) i = j - 1;
      continue;
    }

    const fenceLine = j + 1;
    const closeRe = new RegExp(`^\\s*${inner[1][0]}{${inner[1].length},}\\s*$`);
    const body = [];
    let k = j + 1;
    while (k < lines.length && !closeRe.test(lines[k])) body.push(lines[k++]);
    if (k >= lines.length) {
      errors.push({ file: relPath, line: fenceLine, message: `${relPath}:${fenceLine}: unclosed <!-- doc-test --> fence` });
      break;
    }
    i = k;
    const script = body.join('\n').trim();
    if (!ALLOWED_LANGS.has(inner[2])) {
      errors.push({ file: relPath, line: fenceLine, message: `${relPath}:${fenceLine}: unsupported fence language '${inner[2]}'` });
    } else if (!script) {
      errors.push({ file: relPath, line: fenceLine, message: `${relPath}:${fenceLine}: empty <!-- doc-test --> block` });
    } else {
      blocks.push({ file: relPath, markerLine, fenceLine, lang: inner[2], script });
    }
  }
  return { blocks, errors };
}

export function collectDocTestBlocks(dirs = [], cwd = process.cwd()) {
  const files = collectMarkdownFiles(dirs, cwd);
  const blocks = [];
  const errors = [];
  for (const f of files) {
    const res = extractDocCommands(readFileSync(f.absPath, 'utf8'), f.relPath);
    blocks.push(...res.blocks);
    errors.push(...res.errors);
  }
  return { files, blocks, errors };
}

export function detectCargoZeroTests(script, output) {
  const active = String(script ?? '').split(/\r?\n/).filter((l) => !/^\s*#/.test(l)).join('\n');
  if (!/\bcargo\s+test\b/.test(active)) return null;
  const split = String(output ?? '').split(/^\s*Finished\b/m);
  const sections = split.length > 1 ? split.slice(1) : [split[0]];
  for (const section of sections) {
    const counts = [...section.matchAll(/^\s*running\s+(\d+)\s+tests?\b/gm)].map((m) => Number(m[1]));
    if (counts.length === 0) return 'cargo test output missing `running N tests` summary line';
    if (counts.reduce((sum, n) => sum + n, 0) === 0) {
      return 'cargo test reported `running 0 tests` (stale filter or unscheduled target)';
    }
  }
  return null;
}

export function runStaticChecks(blocks, repoRoot) {
  const pkgPath = join(repoRoot, 'package.json');
  const pkgScripts = new Set(existsSync(pkgPath) ? Object.keys(JSON.parse(readFileSync(pkgPath, 'utf8')).scripts ?? {}) : []);
  const rustTargets = new Set();
  const addTestsDir = (dir) => {
    if (!existsSync(dir)) return;
    for (const e of readdirSync(dir, { withFileTypes: true })) {
      if (e.isFile() && e.name.endsWith('.rs')) rustTargets.add(basename(e.name, '.rs'));
      else if (e.isDirectory() && existsSync(join(dir, e.name, 'main.rs'))) rustTargets.add(e.name);
    }
  };
  addTestsDir(join(repoRoot, 'src-tauri', 'tests'));
  const cratesDir = join(repoRoot, 'src-tauri', 'crates');
  if (existsSync(cratesDir)) {
    for (const c of readdirSync(cratesDir, { withFileTypes: true })) if (c.isDirectory()) addTestsDir(join(cratesDir, c.name, 'tests'));
  }

  const errors = [];
  for (const b of blocks) {
    const active = b.script.split(/\r?\n/).filter((l) => !/^\s*#/.test(l)).join('\n');
    for (const m of active.matchAll(/\bnpm\s+run\s+([A-Za-z0-9:_-]+)/g)) {
      if (!pkgScripts.has(m[1])) errors.push(`${b.file}:${b.fenceLine}: unknown npm script '${m[1]}'`);
    }
    for (const m of active.matchAll(/\bnode\s+(?:["'])?(?:\.\.?\/)*(scripts\/[A-Za-z0-9._/-]+\.mjs)(?:["'])?/g)) {
      if (!existsSync(join(repoRoot, m[1]))) errors.push(`${b.file}:${b.fenceLine}: referenced script not found: ${m[1]}`);
    }
    for (const m of active.matchAll(/\bcargo\s+test\b[^\n]*?\s--test(?:=|\s+)([A-Za-z0-9_-]+)/g)) {
      if (!rustTargets.has(m[1])) errors.push(`${b.file}:${b.fenceLine}: unknown Rust test target '--test ${m[1]}'`);
    }
  }
  return errors;
}

export function runDocCommands(blocks, { cwd = process.cwd(), env = {} } = {}) {
  const executed = [];
  if (blocks.length === 0) return { ok: false, message: 'check_doc_commands: 0 <!-- doc-test --> blocks found; failing closed', executed };
  for (const b of blocks) {
    const res = spawnSync('bash', ['-eo', 'pipefail', '-c', `exec 2>&1\n${b.script}`], {
      cwd,
      env: { ...process.env, ...env },
      encoding: 'utf8',
      maxBuffer: 32 * 1024 * 1024,
    });
    const combined = `${res.stdout || ''}${res.stderr ? `\n${res.stderr}` : ''}`;
    const rawStatus = res.status ?? 1;
    const zeroMsg = rawStatus === 0 ? detectCargoZeroTests(b.script, combined) : null;
    const status = zeroMsg ? 1 : rawStatus;
    executed.push({ file: b.file, fenceLine: b.fenceLine, status });
    if (status !== 0) {
      return {
        ok: false,
        failedBlock: b,
        exitStatus: status,
        message: `${b.file}:${b.fenceLine}: ${zeroMsg ?? `exited with status ${status}`}`,
        output: combined.trim(),
        executed,
      };
    }
  }
  return { ok: true, executed };
}

function main() {
  let opts;
  try { opts = parseArgs(process.argv.slice(2)); } catch (err) { console.error(err.message); process.exit(2); }
  if (opts.help) {
    console.log('Usage: node scripts/check_doc_commands.mjs [--dir <path>]... [--cwd <path>] [--static]');
    return;
  }
  const cwd = resolve(opts.cwd ?? process.cwd());
  let scan;
  try {
    scan = collectDocTestBlocks(opts.dirs, cwd);
  } catch (err) {
    console.error(`check_doc_commands: ${err.message}`);
    process.exit(1);
  }
  if (scan.errors.length > 0) { scan.errors.forEach((e) => console.error(e.message)); process.exit(1); }
  if (opts.staticOnly) {
    if (scan.blocks.length === 0) {
      console.log('check_doc_commands: 0 blocks; nothing to check');
      return;
    }
    const errs = runStaticChecks(scan.blocks, cwd);
    if (errs.length > 0) { errs.forEach((e) => console.error(e)); process.exit(1); }
    console.log(`check_doc_commands: static ok (${scan.blocks.length} block(s) in ${scan.files.length} file(s))`);
    return;
  }
  const res = runDocCommands(scan.blocks, { cwd });
  if (!res.ok) { console.error(res.message); if (res.output) console.error(res.output); process.exit(res.exitStatus || 1); }
  console.log(`check_doc_commands: OK (${res.executed.length} block(s) in ${scan.files.length} file(s))`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) main();
