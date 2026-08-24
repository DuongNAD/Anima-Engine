#!/usr/bin/env node
// Cross-check every frontend `invoke(...)` argument key against the Rust `#[tauri::command]` it calls.
//
// # The bug class this exists for
//
// `tauri-macros` sets `argument_case: ArgumentCase::Camel` as its default and applies
// `key.to_lower_camel_case()` to every command parameter name. So a command declared
//
//     #[tauri::command]
//     pub fn save_simulation_state(app_handle: tauri::AppHandle, file_path: String) -> ...
//
// expects the JS key `filePath`. Passing `file_path` does not "mostly work" — the argument fails to
// deserialize and the command errors. For an `Option<T>` parameter it is worse: the key simply reads
// as `None` and the command silently takes its default path.
//
// Nothing in the existing test suite can see this. Every frontend and E2E test mocks `invoke`, so the
// mock happily accepts whatever key the caller passes — and several tests had in fact frozen the
// *wrong* key into an assertion, which made the suite green *because* the bug was there.
//
// A type checker cannot see it either: `invoke` takes `Record<string, unknown>`, so both spellings
// type-check. This script is the only thing standing between that and a release.
//
//   node scripts/check_ipc_arg_case.mjs
//
// Exits non-zero listing every mismatch, with the file and line of the call site.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, resolve, dirname, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const RUST_DIRS = [join(repoRoot, 'src-tauri/src/commands')];
const TS_DIRS = [join(repoRoot, 'src')];

/** Parameters the framework injects; they never appear in an `invoke` payload. */
const INJECTED_TYPE = /\b(State\s*<|AppHandle|WebviewWindow|Window\b|Runtime\b|Emitter\b)/;

const toCamel = (s) => s.replace(/_([a-z0-9])/g, (_, c) => c.toUpperCase());

function walk(dir, exts, out = []) {
  for (const entry of readdirSync(dir)) {
    const p = join(dir, entry);
    if (statSync(p).isDirectory()) walk(p, exts, out);
    else if (exts.some((e) => p.endsWith(e))) out.push(p);
  }
  return out;
}

/** Every `#[tauri::command]` and the argument keys it expects from JS. */
function rustCommands() {
  const commands = new Map();
  for (const file of RUST_DIRS.flatMap((d) => walk(d, ['.rs']))) {
    const src = readFileSync(file, 'utf8');
    const re = /#\[tauri::command(?:\(([^)]*)\))?\]\s*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)\s*\(/g;
    let m;
    while ((m = re.exec(src))) {
      const [, attrs = '', name] = m;
      const snake = /rename_all\s*=\s*"snake_case"/.test(attrs);
      // Take the parameter list by balancing parentheses from the opening one.
      let depth = 1;
      let i = re.lastIndex;
      while (i < src.length && depth > 0) {
        if (src[i] === '(') depth++;
        else if (src[i] === ')') depth--;
        i++;
      }
      const params = src.slice(re.lastIndex, i - 1);
      // Split on top-level commas only (generics and tuples contain their own).
      const parts = [];
      let buf = '';
      let d = 0;
      for (const ch of params) {
        if ('<([{'.includes(ch)) d++;
        else if ('>)]}'.includes(ch)) d--;
        if (ch === ',' && d === 0) {
          parts.push(buf);
          buf = '';
        } else buf += ch;
      }
      if (buf.trim()) parts.push(buf);

      const expected = new Map();
      for (const raw of parts) {
        const p = raw.replace(/\/\/.*$/gm, '').trim();
        if (!p) continue;
        const c = p.indexOf(':');
        if (c < 0) continue;
        const pname = p.slice(0, c).trim();
        const ptype = p.slice(c + 1).trim();
        if (!/^_?[a-z][a-z0-9_]*$/.test(pname)) continue;
        if (INJECTED_TYPE.test(ptype)) continue;
        if (pname.startsWith('_')) continue;
        expected.set(pname, snake ? pname : toCamel(pname));
      }
      commands.set(name, { file: relative(repoRoot, file), expected, snake });
    }
  }
  return commands;
}

/** Every `invoke('name', { ...keys })` call site in the frontend. */
function invokeCallSites() {
  const sites = [];
  for (const file of TS_DIRS.flatMap((d) => walk(d, ['.ts', '.tsx']))) {
    const src = readFileSync(file, 'utf8');
    const re = /invoke\s*(?:<[^>]*>)?\s*\(\s*['"]([\w]+)['"]\s*(,)?/g;
    let m;
    while ((m = re.exec(src))) {
      const [, command, hasArgs] = m;
      const line = src.slice(0, m.index).split('\n').length;
      let keys = [];
      if (hasArgs) {
        // Find the object literal that follows and take its TOP-LEVEL keys.
        let i = re.lastIndex;
        while (i < src.length && /\s/.test(src[i])) i++;
        if (src[i] === '{') {
          let depth = 0;
          const start = i;
          while (i < src.length) {
            if (src[i] === '{') depth++;
            else if (src[i] === '}') {
              depth--;
              if (depth === 0) break;
            }
            i++;
          }
          const body = src.slice(start + 1, i);
          let d = 0;
          let buf = '';
          const parts = [];
          for (const ch of body) {
            if ('([{'.includes(ch)) d++;
            else if (')]}'.includes(ch)) d--;
            if (ch === ',' && d === 0) {
              parts.push(buf);
              buf = '';
            } else buf += ch;
          }
          if (buf.trim()) parts.push(buf);
          keys = parts
            .map((p) => p.trim())
            .filter(Boolean)
            .map((p) => {
              const k = p.split(':')[0].trim();
              return /^[A-Za-z_$][\w$]*$/.test(k) ? k : null;
            })
            .filter(Boolean);
        }
      }
      sites.push({ command, keys, file: relative(repoRoot, file), line });
    }
  }
  return sites;
}

const commands = rustCommands();
const sites = invokeCallSites();
const failures = [];
let checked = 0;

for (const site of sites) {
  const cmd = commands.get(site.command);
  if (!cmd) continue; // plugin commands and anything outside src-tauri/src/commands
  for (const key of site.keys) {
    checked++;
    const expectedKeys = [...cmd.expected.values()];
    if (expectedKeys.includes(key)) continue;
    // Is it the snake_case spelling of a real parameter? That is the bug, precisely.
    const rustName = [...cmd.expected.keys()].find((n) => n === key || toCamel(n) === toCamel(key));
    if (rustName) {
      failures.push(
        `${site.file}:${site.line} invoke('${site.command}', { ${key}: ... }) — the command declares ` +
          `\`${rustName}\` and #[tauri::command] defaults to camelCase, so it expects ` +
          `\`${cmd.expected.get(rustName)}\`. As written the argument never arrives.`,
      );
    } else {
      failures.push(
        `${site.file}:${site.line} invoke('${site.command}', { ${key}: ... }) — the command declares ` +
          `no such parameter. It accepts: ${expectedKeys.length ? expectedKeys.join(', ') : '(none)'}.`,
      );
    }
  }
}

console.log(
  `ipc arg case: ${commands.size} tauri commands, ${sites.length} invoke call sites, ${checked} argument keys checked`,
);
if (failures.length === 0) {
  console.log('OK — every invoke argument key matches the command signature it calls.');
  process.exit(0);
}
console.error('');
for (const f of failures) console.error(`FAIL  ${f}`);
console.error(`\n${failures.length} mismatch(es). These fail at runtime in the real app and no ` + `mocked test can see them.`);
process.exit(1);
