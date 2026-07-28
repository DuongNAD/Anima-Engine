#!/usr/bin/env node
// Refuse to ship source files containing raw control bytes.
//
// # The bug this exists for
//
// `scripts/gen_sbom.mjs` joined a package name to its version with a **literal NUL byte**, typed
// directly into the source rather than written as the escape `\0`. It worked: NUL is a fine
// separator, and no package name contains one. What it also did was make git classify the file as
// binary — `git diff` reported `Bin 7983 -> 7984 bytes` and showed nothing, so every change to the
// SBOM generator was unreviewable, and `grep` skipped the file. A licence and SBOM generator that
// cannot be code-reviewed is the last file in a repository that should be invisible to diff.
//
// It also cannot be seen by reading. Editors, terminals and file viewers render a control byte as a
// space or a box, so the corrupted line looks exactly like the correct one. The only way to find it
// is to look at the bytes, which is what this does — and which caught a second instance, introduced
// into `check_sbom_schema.mjs` while writing the fix for the first.
//
// # Scope
//
// Text formats only, and only files git tracks. TAB, LF and CR are ordinary text. Everything else
// below 0x20, plus DEL, is not — in these formats it is either a typo or a paste accident, and in
// both cases the author meant an escape sequence.
//
// Usage: node scripts/check_text_hygiene.mjs

import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { extname, join } from 'node:path';
import { argv } from 'node:process';
import { pathToFileURL } from 'node:url';
import { TEXT_EXTENSIONS, findControlByteOffsets } from './lib/text_hygiene.mjs';

const ROOT = process.cwd();

function main() {
  // `--others --exclude-standard` as well as `--cached`: a gate that only sees committed files
  // cannot stop the commit that introduces the problem, which is the only moment it is cheap to fix.
  // Ignored paths stay out, so `node_modules/` and `target/` are not walked.
  const tracked = execFileSync(
    'git',
    ['ls-files', '-z', '--cached', '--others', '--exclude-standard'],
    { cwd: ROOT, encoding: 'utf8', maxBuffer: 1 << 28 },
  )
    .split('\0')
    .filter(Boolean);

  const findings = [];
  let scanned = 0;
  for (const rel of tracked) {
    if (!TEXT_EXTENSIONS.has(extname(rel).toLowerCase())) continue;
    let bytes;
    try {
      bytes = readFileSync(join(ROOT, rel));
    } catch {
      continue; // deleted or unreadable in this working tree
    }
    scanned++;
    const offsets = findControlByteOffsets(bytes);
    if (offsets.length > 0) {
      // Report a line number, because a byte offset is not something a reviewer can act on.
      const lines = offsets.map((o) => bytes.subarray(0, o).toString('latin1').split('\n').length);
      findings.push(
        `${rel}: control byte(s) at line ${[...new Set(lines)].join(', ')} ` +
          `(byte offset ${offsets.join(', ')}${offsets.length >= 5 ? ', ...' : ''})`,
      );
    }
  }

  if (findings.length > 0) {
    console.error(
      `${findings.length} tracked text file(s) contain raw control bytes. Write the escape ` +
        `sequence (\\0, \\u001b) instead of the byte itself:`,
    );
    for (const f of findings) console.error(`  x ${f}`);
    process.exit(1);
  }

  console.log(`text hygiene: ${scanned} tracked text file(s), no raw control bytes`);
}

// Importable for tests, executable as a gate.
if (argv[1] && import.meta.url === pathToFileURL(argv[1]).href) main();
