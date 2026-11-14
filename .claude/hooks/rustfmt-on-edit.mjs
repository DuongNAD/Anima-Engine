// PostToolUse(Write|Edit) hook: format edited Rust files with rustfmt.
//
// Node port of rustfmt-on-edit.ps1, which called `powershell.exe` and so never ran on macOS or
// Linux -- CLAUDE.md promised "edited .rs files are auto-formatted" on every platform while the hook
// silently did nothing off Windows. Node is already a hard requirement of this repo, so one script
// serves all three.
//
// Reads the tool-call JSON from stdin, takes the edited path, and runs `rustfmt --edition 2021` on
// it when it is an existing .rs file. Always exits 0: a formatter must never block an edit.

import { spawnSync } from 'node:child_process';
import fs from 'node:fs';

let raw = '';
try {
  raw = fs.readFileSync(0, 'utf8');
} catch {
  process.exit(0);
}

let payload;
try {
  payload = JSON.parse(raw);
} catch {
  process.exit(0);
}

const file = payload?.tool_input?.file_path ?? payload?.tool_response?.filePath;
if (typeof file === 'string' && file.toLowerCase().endsWith('.rs') && fs.existsSync(file)) {
  spawnSync('rustfmt', ['--edition', '2021', file], { stdio: 'ignore', shell: process.platform === 'win32' });
}
process.exit(0);
