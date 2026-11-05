#!/usr/bin/env node
// Regenerates the command and event tables of docs/reference/IPC_CONTRACT.md from the code, and
// with --check fails when the committed tables no longer match it.
//
// Why this exists: the IPC contract used to live as hand-written prose in PROJECT.md, and that file
// was overwritten three times by other agents' project documents. Each time the contract silently
// disappeared while CLAUDE.md still pointed to it. A table generated from `generate_handler!`, the
// `#[tauri::command]` signatures and the `emit(...)` call sites cannot drift from the code without
// this gate going red.
//
// Run:   node scripts/gen_ipc_contract.mjs          (rewrite the generated block)
//        node scripts/gen_ipc_contract.mjs --check  (exit 1 if the block is stale)

import { readFileSync, writeFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const DOC = join(ROOT, 'docs/reference/IPC_CONTRACT.md');
const LIB = join(ROOT, 'src-tauri/src/lib.rs');
const SRC = join(ROOT, 'src-tauri/src');
const FRONTEND = join(ROOT, 'src');
const BEGIN = '<!-- BEGIN GENERATED: scripts/gen_ipc_contract.mjs -->';
const END = '<!-- END GENERATED -->';

function walk(dir, exts, out = []) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    const s = statSync(p);
    if (s.isDirectory()) {
      if (name === 'node_modules' || name === 'target' || name === 'generated') continue;
      walk(p, exts, out);
    } else if (exts.some((e) => name.endsWith(e))) out.push(p);
  }
  return out;
}

// 1. Commands registered in generate_handler![...]
const lib = readFileSync(LIB, 'utf8');
const handler = lib.match(/generate_handler!\[([\s\S]*?)\]/);
if (!handler) {
  console.error('gen_ipc_contract: generate_handler![...] not found in src-tauri/src/lib.rs');
  process.exit(1);
}
const registered = [...handler[1].matchAll(/([A-Za-z_][A-Za-z0-9_]*)\s*(?:,|$)/g)]
  .map((m) => m[1])
  .filter((n) => n !== 'commands');

// 2. Signatures of #[tauri::command] functions
const rsFiles = walk(SRC, ['.rs']);
const sigs = new Map();
for (const f of rsFiles) {
  const text = readFileSync(f, 'utf8');
  const lines = text.split('\n');
  for (let i = 0; i < lines.length; i++) {
    if (!lines[i].includes('#[tauri::command')) continue;
    let j = i + 1;
    let sig = '';
    while (j < lines.length && !/\{\s*$/.test(lines[j]) && j < i + 40) sig += ' ' + lines[j++].trim();
    sig += ' ' + (lines[j] || '').trim();
    const m = sig.match(/pub\s+(?:async\s+)?fn\s+([a-z_0-9]+)\s*(?:<[^>]*>)?\s*\(([\s\S]*?)\)\s*(?:->\s*([\s\S]*?))?\s*(?:where[\s\S]*)?\{/);
    if (!m) continue;
    const [, name, rawParams, rawRet] = m;
    const params = rawParams
      .split(/,(?![^<]*>)/)
      .map((p) => p.trim())
      .filter(Boolean)
      .filter((p) => !/^(state|_state|app_handle|_app_handle|app|window)\s*:/.test(p))
      .map((p) => p.replace(/\s+/g, ' '));
    const ret = (rawRet || '()').replace(/\s+/g, ' ').trim();
    sigs.set(name, { file: relative(ROOT, f), line: j + 1 - (j - i), params, ret });
  }
}

// 3. Events emitted from Rust, with their call sites
const events = new Map();
for (const f of rsFiles) {
  const lines = readFileSync(f, 'utf8').split('\n');
  lines.forEach((l, idx) => {
    for (const m of l.matchAll(/\.emit(?:_all|_to)?\(\s*"([a-z0-9_-]+)"/g)) {
      const arr = events.get(m[1]) || [];
      arr.push(`${relative(ROOT, f)}:${idx + 1}`);
      events.set(m[1], arr);
    }
  });
}

// 4. Frontend use (literal invoke/listen in src/)
const tsFiles = walk(FRONTEND, ['.ts', '.tsx']).filter((f) => !f.includes('__tests__') && !f.includes('/generated/'));
const invoked = new Map();
const listened = new Map();
for (const f of tsFiles) {
  const lines = readFileSync(f, 'utf8').split('\n');
  lines.forEach((l, idx) => {
    for (const m of l.matchAll(/invoke(?:<[^>]*>)?\(\s*['"]([a-z_0-9]+)['"]/g)) {
      const arr = invoked.get(m[1]) || [];
      arr.push(`${relative(ROOT, f)}:${idx + 1}`);
      invoked.set(m[1], arr);
    }
    for (const m of l.matchAll(/listen(?:<[^>]*>)?\(\s*['"]([a-z0-9_-]+)['"]/g)) {
      const arr = listened.get(m[1]) || [];
      arr.push(`${relative(ROOT, f)}:${idx + 1}`);
      listened.set(m[1], arr);
    }
  });
}

const problems = [];
for (const name of registered) if (!sigs.has(name)) problems.push(`registered but no #[tauri::command] fn found: ${name}`);
for (const name of sigs.keys()) if (!registered.includes(name)) problems.push(`#[tauri::command] fn not registered in generate_handler!: ${name}`);

const esc = (s) => s.replace(/\|/g, '\\|');
let out = `${BEGIN}\n\n`;
out += `### Lệnh (${registered.length}, theo thứ tự trong \`generate_handler!\`)\n\n`;
out += '| Lệnh | Tham số (JS gửi camelCase) | Trả về | Định nghĩa | Gọi từ `src/` |\n|---|---|---|---|---|\n';
for (const name of registered) {
  const s = sigs.get(name);
  const where = s ? `\`${s.file}\`` : '—';
  const params = s && s.params.length ? s.params.map((p) => `\`${esc(p)}\``).join(', ') : '—';
  const ret = s ? `\`${esc(s.ret)}\`` : '—';
  const uses = invoked.get(name);
  out += `| \`${name}\` | ${params} | ${ret} | ${where} | ${uses ? uses.length + ' chỗ' : 'không có lời gọi literal'} |\n`;
}
const dead = [...invoked.keys()].filter((n) => !registered.includes(n)).sort();
out += `\n### Lời gọi từ frontend tới lệnh không tồn tại (${dead.length})\n\n`;
if (dead.length === 0) out += 'Không có.\n';
else {
  out += '| Lệnh | Gọi tại |\n|---|---|\n';
  for (const n of dead) out += `| \`${n}\` | ${invoked.get(n).map((u) => `\`${u}\``).join(', ')} |\n`;
}
out += `\n### Sự kiện Rust → webview (${events.size})\n\n`;
out += '| Sự kiện | Phát tại | `listen` literal trong `src/` |\n|---|---|---|\n';
for (const name of [...events.keys()].sort()) {
  const l = listened.get(name);
  out += `| \`${name}\` | ${events.get(name).map((u) => `\`${u.replace(/:\d+$/, '')}\``).filter((v, i, a) => a.indexOf(v) === i).join(', ')} | ${l ? l.length + ' chỗ' : 'không có'} |\n`;
}
out += `\n${END}`;

const doc = readFileSync(DOC, 'utf8');
const start = doc.indexOf(BEGIN);
const stop = doc.indexOf(END);
if (start < 0 || stop < 0) {
  console.error(`gen_ipc_contract: markers not found in ${relative(ROOT, DOC)}`);
  process.exit(1);
}
const next = doc.slice(0, start) + out + doc.slice(stop + END.length);

if (process.argv.includes('--check')) {
  if (problems.length) {
    for (const p of problems) console.error(`gen_ipc_contract: ${p}`);
    process.exit(1);
  }
  if (next !== doc) {
    console.error('gen_ipc_contract: docs/reference/IPC_CONTRACT.md is stale — run node scripts/gen_ipc_contract.mjs');
    process.exit(1);
  }
  console.log(`gen_ipc_contract: up to date — ${registered.length} commands, ${events.size} events, ${dead.length} frontend call(s) to missing commands`);
} else {
  writeFileSync(DOC, next);
  for (const p of problems) console.error(`gen_ipc_contract: WARNING ${p}`);
  console.log(`gen_ipc_contract: wrote ${registered.length} commands, ${events.size} events`);
}
