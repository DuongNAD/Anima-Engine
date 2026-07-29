// Docs link check.
//
// The contracts in docs/ are only binding if the pointers to them resolve. CLAUDE.md names a
// required-reading order by path, ADRs cite contracts, and planning docs cite ADRs -- a renamed or
// deleted file turns that chain into a dead end silently. This walks every tracked markdown file and
// fails on a relative link whose target does not exist.
//
// External links (http/https/mailto) are not fetched.
//
// Same-page `#anchor` links ARE resolved, against the headings of the file they appear in. That was
// not always true, and the reason it is now is a defect this check would have caught: a section
// added to STATE_OF_THE_PROJECT.md on 2026-07-29 was linked from four places, and one of them was
// written before the heading's final wording and pointed at a slug that never existed. The link
// rendered normally and did nothing — which is the same silent dead end this script exists to
// prevent, one level down. A living document whose own table of contents lies is worse than one
// with a missing file, because nothing looks wrong.
//
// Cross-file `other.md#anchor` links are checked for the FILE only. Resolving those needs the
// target's headings, which is a different pass; the same-page case is where a heading gets reworded
// and its links do not.
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
const HEADING = /^#{1,6}\s+(.+?)\s*$/;

// GitHub's heading-to-fragment rule: lower-case, drop everything that is not a letter, digit,
// space or hyphen, then spaces to hyphens. `\p{L}`/`\p{N}` rather than `\w` because these documents
// are written in Vietnamese, and `\w` would strip every diacritic and silently agree with itself.
const slug = (text) =>
  text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s-]/gu, '')
    .trim()
    .replace(/\s/g, '-');

let broken = 0;
let checked = 0;
let anchorsChecked = 0;

for (const file of files) {
  const dir = path.dirname(file);
  let inFence = false;

  const lines = fs.readFileSync(file, 'utf8').split(/\r?\n/);

  // Headings first: a link may point at a section further down the page than itself.
  const headings = new Set();
  {
    let fenced = false;
    for (const line of lines) {
      if (FENCE.test(line)) {
        fenced = !fenced;
        continue;
      }
      if (fenced) continue;
      const h = line.match(HEADING);
      if (h) headings.add(slug(h[1]));
    }
  }

  lines.forEach((line, i) => {
    if (FENCE.test(line)) {
      inFence = !inFence;
      return;
    }
    if (inFence) return;

    for (const m of line.matchAll(LINK)) {
      const raw = m[2];
      if (/^(https?:|mailto:)/.test(raw)) continue;

      if (raw.startsWith('#')) {
        anchorsChecked += 1;
        const anchor = decodeURIComponent(raw.slice(1));
        if (!headings.has(anchor)) {
          broken += 1;
          console.error(
            `${file}:${i + 1}: broken anchor -> ${raw} (no heading on this page slugs to it)`,
          );
        }
        continue;
      }

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

console.log(
  `docs link check: ${checked} relative links + ${anchorsChecked} same-page anchors in ` +
    `${files.length} files, ${broken} broken`,
);
process.exit(broken > 0 ? 1 : 0);
