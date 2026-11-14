// IPC reference check.
//
// docs/reference/IPC_CONTRACT.md lists every Tauri command and event. Before it existed, CLAUDE.md
// pointed at a PROJECT.md section that held no commands at all, and its own list named 7 of the 29
// registered commands and 5 of the 7 events -- so an agent changing the IPC surface was reading a
// partial map and could not tell. This keeps the reference and the code equal in both directions.
//
//   commands: `commands::<name>` entries of `tauri::generate_handler![...]` in src-tauri/src/lib.rs
//   events:   string literals passed to `.emit(` / `.emit_all(` / `.emit_to(` under src-tauri/src
//   doc:      the backticked first cell of every table row under "## Lệnh" / "## Event"
//
// Like check_docs_metadata.mjs it runs negative controls on its own parser first, so a regex that
// stopped matching cannot report a green "0 differences".
//
// Run:  node scripts/check_ipc_reference.mjs

import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const DOC = 'docs/reference/IPC_CONTRACT.md';
const LIB = 'src-tauri/src/lib.rs';
const SRC = 'src-tauri/src';

export function registeredCommands(libText) {
  const m = libText.match(/generate_handler!\s*\[([\s\S]*?)\]/);
  if (!m) return null;
  return new Set([...m[1].matchAll(/(?:[a-z_][a-z0-9_]*::)*([a-z_][a-z0-9_]*)\s*(?:,|$)/g)].map((x) => x[1]));
}

export function emittedEvents(rsText) {
  return [...rsText.matchAll(/\.emit(?:_all|_to)?\(\s*(?:[^,()]+,\s*)?"([a-z0-9][a-z0-9-]*)"/g)].map((x) => x[1]);
}

/** Backticked first cells of table rows, per "## " section whose title starts with `prefix`. */
export function documented(docText, prefix) {
  const out = new Set();
  let inSection = false;
  for (const line of docText.split(/\r?\n/)) {
    if (/^## /.test(line)) inSection = line.startsWith(`## ${prefix}`);
    if (!inSection) continue;
    const m = line.match(/^\|\s*`([a-z0-9_-]+)`\s*\|/);
    if (m) out.add(m[1]);
  }
  return out;
}

function selfTest() {
  const failures = [];
  const lib = 'x.invoke_handler(tauri::generate_handler![\n  commands::a_cmd,\n  commands::b_cmd\n])';
  const got = [...(registeredCommands(lib) ?? [])].sort().join(',');
  if (got !== 'a_cmd,b_cmd') failures.push(`registeredCommands parsed "${got}"`);
  if (registeredCommands('no handler here') !== null) failures.push('registeredCommands accepted a file with no handler');
  const ev = emittedEvents('h.emit("tick-a", &p); w.emit_to("main", "tick-b", x); h.emit_all("tick-c", y);').sort().join(',');
  if (ev !== 'tick-a,tick-b,tick-c') failures.push(`emittedEvents parsed "${ev}"`);
  const doc = '## Lệnh (2)\n| `a_cmd` | x |\n| Lệnh | h |\n## Event (1)\n| `tick-a` | y |\n## Khác\n| `nope` | z |';
  if ([...documented(doc, 'Lệnh')].join(',') !== 'a_cmd') failures.push('documented() misread the command table');
  if ([...documented(doc, 'Event')].join(',') !== 'tick-a') failures.push('documented() misread the event table');
  return failures;
}

function walk(dir, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, out);
    else if (e.name.endsWith('.rs')) out.push(p);
  }
  return out;
}

function diff(label, code, doc) {
  let n = 0;
  for (const x of [...code].sort()) if (!doc.has(x)) { n++; console.error(`${DOC}: ${label} \`${x}\` is in the code but not documented`); }
  for (const x of [...doc].sort()) if (!code.has(x)) { n++; console.error(`${DOC}: ${label} \`${x}\` is documented but not in the code`); }
  return n;
}

function main() {
  const st = selfTest();
  if (st.length) {
    for (const f of st) console.error(`self-test: ${f}`);
    console.error('ipc reference check: the checker itself is broken');
    process.exit(2);
  }
  const cmds = registeredCommands(fs.readFileSync(LIB, 'utf8'));
  if (!cmds || cmds.size === 0) {
    console.error(`${LIB}: no generate_handler![...] found -- the parser no longer matches the code`);
    process.exit(2);
  }
  const events = new Set(walk(SRC).flatMap((f) => emittedEvents(fs.readFileSync(f, 'utf8'))));
  const docText = fs.readFileSync(DOC, 'utf8');
  const errors = diff('command', cmds, documented(docText, 'Lệnh')) + diff('event', events, documented(docText, 'Event'));
  console.log(`ipc reference check: ${cmds.size} commands, ${events.size} events, ${errors} differences`);
  process.exit(errors ? 1 : 0);
}

main();
