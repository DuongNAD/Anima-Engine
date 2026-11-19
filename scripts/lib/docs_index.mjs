// Generated index blocks for the README indexes under docs/.
//
// Why this exists. Almost every pull request adds one result note under docs/ai/ and, until
// 2026-11-19, also appended a row for it to the same table in docs/ai/README.md. Two PRs open at the
// same time therefore inserted a line at the same place in the same file, GitHub marked the second
// one as conflicting, and the lead resolved it by hand -- `git log -- docs/ai/README.md` was mostly
// "Merge ... into lead/..." commits. `.gitattributes merge=union` does not help: GitHub's merge button
// does not consider repository merge drivers (docs/ai/design/2026-11-19-feature-generated-docs-index.md).
//
// The contract, shared by scripts/gen_docs_index.mjs (which writes) and
// scripts/check_docs_metadata.mjs (which checks), so the two cannot disagree:
//
//   * An index README opts in by holding exactly one block between a line starting with BLOCK_BEGIN
//     and a line starting with BLOCK_END. Inside is one table, TABLE_HEADER then TABLE_RULE, whose rows
//     read `| YYYY-MM-DD | [title](path) — summary | kind | `status` |`.
//   * The block's scope is every document whose nearest index README (walking up from its directory,
//     templates skipped) is that README. For docs/ai/<phase>/x.md that is docs/ai/README.md, because
//     docs/ai/<phase>/README.md is an upstream template, not an index.
//   * A document in scope that no index links to is *pending*. It is acceptable when its front matter
//     can produce a row (`title`, `status`, `description`) and its directory is linked from an index,
//     so GitHub's directory listing reaches it until the row exists.
//   * Regeneration keeps every existing row's text and order, refreshes only its status cell from the
//     linked file's front matter, drops a row whose file is gone, and appends one row per pending
//     document, ordered by (date, path). A row the lead has reworded stays reworded.

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import nodePath from 'node:path';
// git prints forward slashes on every OS; posix keeps Map lookups and joins consistent on Windows.
const path = nodePath.posix;

export const BLOCK_BEGIN = '<!-- docs-index:begin';
export const BLOCK_END = '<!-- docs-index:end';
export const TABLE_HEADER = '| Ngày | Tài liệu | Loại | Trạng thái |';
export const TABLE_RULE = '|---|---|---|---|';
/** Front matter keys a pending document needs for the generator to write its row. */
export const ROW_FIELDS = ['title', 'status', 'description'];

// ---------------------------------------------------------------------------------------------
// Front matter and links (moved here from check_docs_metadata.mjs, which re-exports them)
// ---------------------------------------------------------------------------------------------

/** Returns null when the file has no front matter block, else a flat key -> raw value map. */
export function parseFrontMatter(text) {
  const m = text.match(/^---\r?\n([\s\S]*?)\r?\n---(\r?\n|$)/);
  if (!m) return null;
  const fm = {};
  for (const line of m[1].split(/\r?\n/)) {
    const kv = line.match(/^([A-Za-z_][A-Za-z0-9_]*):\s*(.*)$/);
    if (kv) fm[kv[1]] = kv[2].trim();
  }
  return fm;
}

/** An enum-like value: drop a trailing YAML comment and surrounding quotes. */
export const scalar = (v) =>
  v === undefined ? undefined : v.replace(/\s+#.*$/, '').replace(/^(['"])(.*)\1$/, '$2').trim();

/** A prose value (title, description): surrounding quotes only. `#144` in prose is not a comment. */
export function prose(v) {
  if (v === undefined) return undefined;
  const t = v.trim();
  if (/^".*"$/.test(t)) return t.slice(1, -1).replace(/\\(["\\])/g, '$1');
  if (/^'.*'$/.test(t)) return t.slice(1, -1).replace(/''/g, "'");
  return t;
}

// Templates are skeletons of a process, not project documents (policy rule 3 for docs/ai).
export const isTemplate = (f) =>
  /^docs\/ai\/[^/]+\/README\.md$/.test(f) || f === 'docs/decisions/ADR-0000-template.md';

export const isIndex = (f) => path.basename(f) === 'README.md' && !isTemplate(f);

const LINK = /(^|[^!])\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g;
const FENCE = /^\s*(```|~~~)/;

/**
 * Repo-relative targets of every relative markdown link in `text`, outside code fences. A link to
 * a directory (`planning/`) comes back without its trailing slash.
 */
export function linkTargets(file, text) {
  const out = new Set();
  let inFence = false;
  for (const line of text.split(/\r?\n/)) {
    if (FENCE.test(line)) {
      inFence = !inFence;
      continue;
    }
    if (inFence) continue;
    for (const m of line.matchAll(LINK)) {
      const raw = m[2];
      if (/^(https?:|mailto:|#)/.test(raw)) continue;
      let target;
      try {
        target = decodeURIComponent(raw.split('#')[0]);
      } catch {
        continue;
      }
      if (target) out.add(path.normalize(path.join(path.dirname(file), target)).replace(/\/+$/, ''));
    }
  }
  return out;
}

/** Every target linked from any index README, files and directories alike. */
export function indexedTargets(docs) {
  const out = new Set();
  for (const [f, text] of docs) {
    if (!isIndex(f)) continue;
    for (const t of linkTargets(f, text)) if (t !== f) out.add(t);
  }
  return out;
}

/** The README that indexes `file`: the nearest one up the tree, templates skipped. */
export function nearestIndex(file, has) {
  let dir = path.dirname(file);
  for (;;) {
    const r = path.join(dir, 'README.md');
    if (r !== file && has(r) && isIndex(r)) return r;
    if (!dir.includes('/')) return null;
    dir = path.dirname(dir);
  }
}

// ---------------------------------------------------------------------------------------------
// The block
// ---------------------------------------------------------------------------------------------

const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;

function splitCells(line) {
  const t = line.trim();
  if (!t.startsWith('|') || !t.endsWith('|') || t.endsWith('\\|')) return null;
  return t
    .slice(1, -1)
    .split(/(?<!\\)\|/)
    .map((c) => c.trim());
}

/** One table row, or null when the line is not a row the generator can read and rewrite. */
export function parseRow(line) {
  const cells = splitCells(line);
  if (!cells || cells.length !== 4) return null;
  const [date, doc, kind, status] = cells;
  const link = doc.match(/^\[(.+?)\]\(([^)\s]+)\)/);
  const st = status.match(/^`([a-z-]+)`$/);
  if (!ISO_DATE.test(date) || !link || !kind || !st) return null;
  return { date, title: link[1], target: link[2], kind, status: st[1] };
}

/**
 * null when `text` has no block; `{ error }` when it has a malformed one; otherwise
 * `{ lines, rowStart, rowEnd, rows: [{ index, row }] }` with `lines[rowStart..rowEnd)` the rows.
 */
export function parseBlock(text) {
  const lines = text.split(/\r?\n/);
  const begins = [];
  const ends = [];
  lines.forEach((l, i) => {
    if (l.trimStart().startsWith(BLOCK_BEGIN)) begins.push(i);
    if (l.trimStart().startsWith(BLOCK_END)) ends.push(i);
  });
  if (begins.length === 0 && ends.length === 0) return null;
  if (begins.length !== 1 || ends.length !== 1 || ends[0] < begins[0]) {
    return { error: `needs exactly one \`${BLOCK_BEGIN} ... -->\` line, then one \`${BLOCK_END} ... -->\` line` };
  }
  const end = ends[0];
  let i = begins[0] + 1;
  while (i < end && lines[i].trim() === '') i += 1;
  if (lines[i]?.trim() !== TABLE_HEADER || lines[i + 1]?.trim() !== TABLE_RULE) {
    return { error: `line ${i + 1}: the block must open with \`${TABLE_HEADER}\` and \`${TABLE_RULE}\`` };
  }
  const rowStart = i + 2;
  const rows = [];
  let j = rowStart;
  for (; j < end && lines[j].trim() !== ''; j += 1) {
    const row = parseRow(lines[j]);
    if (!row) {
      return {
        error: `line ${j + 1}: not a row the generator can read; keep \`| YYYY-MM-DD | [title](path) — summary | kind | \`status\` |\``,
      };
    }
    rows.push({ index: j, row });
  }
  const rowEnd = j;
  for (; j < end; j += 1) {
    if (lines[j].trim() !== '') return { error: `line ${j + 1}: text after the table, inside the block` };
  }
  return { lines, rowStart, rowEnd, rows };
}

// ---------------------------------------------------------------------------------------------
// Pending documents and their rows
// ---------------------------------------------------------------------------------------------

/** Why `fm` cannot produce a row; empty when it can. */
export function rowProblems(fm) {
  if (!fm) return ['no front matter'];
  const problems = [];
  for (const k of ROW_FIELDS) {
    if (!(k === 'status' ? scalar(fm[k]) : prose(fm[k]))) problems.push(`missing \`${k}\` (its row is built from it)`);
  }
  for (const k of ['title', 'description']) {
    if (fm[k] && /\]\(/.test(fm[k])) problems.push(`\`${k}\` holds a markdown link, which would not resolve from the index`);
  }
  return problems;
}

/** The row date: the first YYYY-MM-DD in the file name, else `created`, else `last_reviewed`. */
export function rowDate(file, fm) {
  return path.basename(file).match(/\d{4}-\d{2}-\d{2}/)?.[0] ?? scalar(fm.created) ?? scalar(fm.last_reviewed);
}

/** The row the generator writes for a pending document. */
export function rowFor(file, fm, readme) {
  const rel = path.relative(path.dirname(readme), file);
  const cell = (s) => s.replace(/\|/g, '\\|');
  const href = rel.replace(/ /g, '%20').replace(/\(/g, '%28').replace(/\)/g, '%29');
  // The Loại column: `kind` when the note declares one (agent-goal, review-findings, or a free label
  // such as `thẻ` or `đo`); else the phase directory it lives in.
  const kind = scalar(fm.kind) || (rel.includes('/') ? rel.split('/')[0] : 'ghi chú');
  return (
    `| ${rowDate(file, fm)} | [${cell(prose(fm.title))}](${href}) — ${cell(prose(fm.description))} ` +
    `| ${cell(kind)} | \`${scalar(fm.status)}\` |`
  );
}

/**
 * Documents in `readme`'s scope that no index links to: `[{ file, problems }]`. `problems` holds
 * every reason the document cannot wait for its generated row; empty means it is acceptable pending.
 */
export function pendingFor(readme, docs, indexed = indexedTargets(docs)) {
  const has = (p) => docs.has(p);
  const out = [];
  for (const [f, text] of docs) {
    if (f === readme || isTemplate(f) || indexed.has(f)) continue;
    if (nearestIndex(f, has) !== readme) continue;
    const problems = rowProblems(parseFrontMatter(text));
    const dir = path.dirname(f);
    if (dir !== path.dirname(readme) && !indexed.has(dir)) {
      problems.push(`its directory ${dir}/ is not linked from any index, so nothing reaches it before its row exists`);
    }
    out.push({ file: f, problems });
  }
  return out;
}

/**
 * The README text after regeneration, plus what changed. Throws when the README has no usable block,
 * or -- unless `lenient` -- when a pending document cannot produce a row: the generator must not
 * write half an index. The checker regenerates leniently, because it reports those documents itself.
 */
export function regenerate(readme, docs, exists = (p) => docs.has(p), { lenient = false } = {}) {
  const text = docs.get(readme);
  const block = parseBlock(text);
  if (!block) throw new Error(`${readme}: no docs-index block`);
  if (block.error) throw new Error(`${readme}: docs-index block ${block.error}`);
  const eol = text.includes('\r\n') ? '\r\n' : '\n';

  const kept = [];
  const dropped = [];
  const refreshed = [];
  for (const { index, row } of block.rows) {
    let line = block.lines[index];
    let target;
    try {
      target = path.normalize(path.join(path.dirname(readme), decodeURIComponent(row.target.split('#')[0])));
    } catch {
      target = null;
    }
    if (target && !exists(target)) {
      dropped.push(target);
      continue;
    }
    const fm = target && docs.has(target) ? parseFrontMatter(docs.get(target)) : null;
    const status = fm && scalar(fm.status);
    if (status && status !== row.status) {
      line = line.replace(/`[a-z-]+`(\s*\|\s*)$/, `\`${status}\`$1`);
      refreshed.push({ file: target, from: row.status, to: status });
    }
    kept.push(line);
  }

  const pending = pendingFor(readme, docs);
  const bad = pending.filter((p) => p.problems.length);
  if (bad.length && !lenient) {
    throw new Error(bad.map((p) => `${p.file}: cannot generate its row: ${p.problems.join('; ')}`).join('\n'));
  }
  const added = pending
    .filter((p) => p.problems.length === 0)
    .map(({ file }) => {
      const fm = parseFrontMatter(docs.get(file));
      return { file, date: rowDate(file, fm), line: rowFor(file, fm, readme) };
    })
    .sort((a, b) => (a.date === b.date ? (a.file < b.file ? -1 : 1) : a.date < b.date ? -1 : 1));

  const lines = [
    ...block.lines.slice(0, block.rowStart),
    ...kept,
    ...added.map((a) => a.line),
    ...block.lines.slice(block.rowEnd),
  ];
  return {
    text: lines.join(eol),
    kept: kept.length,
    added: added.map((a) => a.file),
    refreshed,
    dropped,
  };
}

// ---------------------------------------------------------------------------------------------
// The tree
// ---------------------------------------------------------------------------------------------

/**
 * Map(file -> text) of every markdown file under docs/ that git tracks or would track. Untracked but
 * not ignored files are included so a new document is checked, and indexed, before it is staged.
 */
export function readDocsTree() {
  const files = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', 'docs/*.md'], {
    encoding: 'utf8',
  })
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean)
    .filter((f) => fs.existsSync(f)); // a tracked file deleted in the working tree
  return new Map(files.map((f) => [f, fs.readFileSync(f, 'utf8')]));
}
