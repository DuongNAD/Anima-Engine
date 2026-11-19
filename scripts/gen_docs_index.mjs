// Regenerates the generated-index blocks of the README indexes under docs/ (today: docs/ai/README.md).
//
// A pull request adds a note under docs/ai/ with front matter and no README edit; the checker
// (scripts/check_docs_metadata.mjs) accepts it as *pending*. The lead runs this in a docs PR to turn
// the pending notes into rows. Rules, shared with the checker: scripts/lib/docs_index.mjs.
//
//   * Existing rows keep their text and order; only the status cell is refreshed from front matter.
//   * A row whose file is gone is dropped.
//   * Each pending note gets one row, appended in (date, path) order.
//   * Line endings are preserved, so a Windows checkout with core.autocrlf=true stays clean.
//
// Run:  npm run gen:docs-index
//       node scripts/gen_docs_index.mjs --check   # print what would change, write nothing, exit 1 if
//                                                 # anything would. Not a CI gate, on purpose: a
//                                                 # pending row is the normal state between lead PRs.

import fs from 'node:fs';
import process from 'node:process';
import { isIndex, parseBlock, readDocsTree, regenerate } from './lib/docs_index.mjs';

const check = process.argv.includes('--check');
const docs = readDocsTree();
const exists = (p) => fs.existsSync(p);

let changed = 0;
let failed = false;
for (const [readme, text] of docs) {
  if (!isIndex(readme)) continue;
  const block = parseBlock(text);
  if (!block) continue;
  let r;
  try {
    r = regenerate(readme, docs, exists);
  } catch (e) {
    console.error(e.message);
    failed = true;
    continue;
  }
  for (const f of r.added) console.log(`${readme}: + ${f}`);
  for (const x of r.refreshed) console.log(`${readme}: ~ ${x.file} (${x.from} -> ${x.to})`);
  for (const f of r.dropped) console.log(`${readme}: - ${f} (file is gone)`);
  console.log(
    `${readme}: ${r.kept} rows kept, ${r.added.length} added, ${r.refreshed.length} status refreshed, ` +
      `${r.dropped.length} dropped`,
  );
  if (r.text !== text) {
    changed += 1;
    if (!check) fs.writeFileSync(readme, r.text);
  }
}

if (failed) {
  console.error('gen:docs-index: nothing written for the README above; fix the front matter it names first');
  process.exit(1);
}
if (check) {
  console.log(changed ? `gen:docs-index --check: ${changed} index(es) would change` : 'gen:docs-index --check: up to date');
  process.exit(changed ? 1 : 0);
}
console.log(changed ? `gen:docs-index: rewrote ${changed} index(es)` : 'gen:docs-index: nothing to do');
