// Docs link check.
//
// The contracts in docs/ are only binding if the pointers to them resolve. CLAUDE.md names a
// required-reading order by path, ADRs cite contracts, and planning docs cite ADRs -- a renamed or
// deleted file turns that chain into a dead end silently. This walks every tracked markdown file and
// fails on a relative link whose target does not exist.
//
// External links (http/https/mailto) are not fetched. Pure `#anchor` links are not resolved.
//
// Run:  node scripts/check_docs_links.mjs

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const files = execFileSync('git', ['ls-files', '*.md'], { encoding: 'utf8' })
  .split('\n')
  .map((l) => l.trim())
  .filter(Boolean)
  // Superseded drafts are kept verbatim as a record of past decisions; their links are historical.
  .filter((f) => !f.startsWith('docs/archive/'))
  // Vendored upstream licence and notice text, stored byte-for-byte as fetched from a pinned
  // commit. Their links are the upstream project's, not this repository's, and several are broken
  // upstream — neo4rs's README points at LICENSE-APACHE and LICENSE-MIT files that project has
  // never had, which is precisely the fact `licensing/upstream/sources.json` records. Editing a
  // vendored licence to satisfy a link check would falsify the bytes every gate hashes.
  .filter((f) => !f.startsWith('licensing/upstream/'));

// [text](target) -- skip images (![...]) by requiring the char before '[' to not be '!'.
const LINK = /(^|[^!])\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g;
// Fenced code blocks hold example paths that are not links to check.
const FENCE = /^\s*(```|~~~)/;

let broken = 0;
let checked = 0;

for (const file of files) {
  const dir = path.dirname(file);
  let inFence = false;

  const lines = fs.readFileSync(file, 'utf8').split(/\r?\n/);
  lines.forEach((line, i) => {
    if (FENCE.test(line)) {
      inFence = !inFence;
      return;
    }
    if (inFence) return;

    for (const m of line.matchAll(LINK)) {
      const raw = m[2];
      if (/^(https?:|mailto:|#)/.test(raw)) continue;

      const target = decodeURIComponent(raw.split('#')[0]);
      if (!target) continue;

      const resolved = path.resolve(dir, target);
      checked += 1;
      if (!fs.existsSync(resolved)) {
        broken += 1;
        console.error(`${file}:${i + 1}: broken link -> ${raw}`);
      }
    }
  });
}

console.log(`docs link check: ${checked} relative links in ${files.length} files, ${broken} broken`);
process.exit(broken > 0 ? 1 : 0);
