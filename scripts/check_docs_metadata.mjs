// Docs metadata and index check.
//
// docs/governance/DOCUMENTATION_POLICY.md has required front matter on every document since
// 2026-07-24 and an index entry for every document since 2026-07-26. Neither rule was enforced,
// so both drifted: by 2026-11-14 seven documents had no front matter at all, nine living documents
// lacked `review_cycle`, two used status values the policy does not define, four live contracts were
// missing from the reference index, and `docs/ai/` -- 45 documents -- had no index. A rule that
// only lives in prose is a wish; this turns the policy into a gate.
//
// What it checks, for every markdown file under docs/ (tracked or untracked-but-not-ignored, so
// a new document is checked before it is staged):
//
//   1. Front matter exists and carries the fields its kind requires (see `requiredFields`).
//   2. `status`, `review_cycle` and the dates use the values the policy defines.
//   3. Archive files say `superseded`/`deprecated` and name a replacement that exists.
//   4. Every directory under docs/ that holds a document has a README.md index.
//   5. Every document is linked from at least one index (docs/README.md or a docs/**/README.md).
//
// Overdue reviews are reported, never failed: the policy says a stale document "is not wrong by
// itself, but must show its owner and enter the review backlog". `--report` lists them.
//
// The gate measures itself first. Before touching the tree it runs a fixed set of documents that
// must be rejected (and two that must pass) through the same validator; if any negative control
// passes, the run fails. A checker that cannot go red is how three gates here passed for months
// without looking (FINDINGS F-024, F-031).
//
// Run:  node scripts/check_docs_metadata.mjs [--report]

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import nodePath from 'node:path';
// git prints forward slashes on every OS; posix keeps Map lookups and joins consistent on Windows.
const path = nodePath.posix;
import process from 'node:process';

// ---------------------------------------------------------------------------------------------
// Policy, as data. Change DOCUMENTATION_POLICY.md "Metadata tối thiểu" in the same commit.
// ---------------------------------------------------------------------------------------------

const STATUS = ['proposed', 'active', 'accepted', 'completed', 'deprecated', 'superseded'];
// `completed` is valid only for the docs/ai working set (policy, "completed và schema mở rộng").
const STATUS_AI_ONLY = ['completed'];
const RECORD_STATUS = ['proposed', 'active', 'completed'];
const ARCHIVE_STATUS = ['superseded', 'deprecated'];
const REVIEW_CYCLE_DAYS = { weekly: 7, monthly: 31, quarterly: 92, 'per-release': null };
// Session records: a plan or review as it stood on the day, not a living document. They carry
// `created` instead of `last_reviewed`, and their body is never updated to match the present.
const RECORD_KINDS = ['agent-goal', 'review-findings'];

// Templates are skeletons of a process, not project documents (policy rule 3 for docs/ai).
const isTemplate = (f) =>
  /^docs\/ai\/[^/]+\/README\.md$/.test(f) || f === 'docs/decisions/ADR-0000-template.md';

// Files that are deliberately without front matter. Each needs a reason, and an entry that no
// longer applies (file gone, or it gained front matter) fails the run, so this list cannot rot.
const NO_FRONT_MATTER = {
  'docs/ai/AI_Context.md':
    'Agent-owned context file from the Phase 0 run (2026-06-12), moved out of the repo root at c8ebec2. Kept byte-stable for the agent tooling that reads it.',
  'docs/ai/ORIGINAL_REQUEST.md':
    'Agent-owned request log, synced by the agent tooling ("docs: sync original request"). Front matter would be overwritten or conflict on the next sync.',
  'docs/ai/TEST_INFRA.md':
    'Agent-owned E2E design matrix moved with the two files above at c8ebec2; its banner already points measured status to STATE §1.',
  'docs/ai/TEST_READY.md':
    'Agent-owned E2E readiness note, same provenance and banner as TEST_INFRA.md.',
};

// review_cycle is required where a document is expected to stay true. docs/ai is a per-feature
// working set and docs/decisions holds immutable ADRs: both are records, reviewed by supersession.
const needsReviewCycle = (f) =>
  !f.startsWith('docs/ai/') && !f.startsWith('docs/decisions/') && !f.startsWith('docs/archive/');

// ---------------------------------------------------------------------------------------------
// Parsing
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
const scalar = (v) =>
  v === undefined ? undefined : v.replace(/\s+#.*$/, '').replace(/^(['"])(.*)\1$/, '$2').trim();

const ISO_DATE = /^(\d{4})-(\d{2})-(\d{2})$/;
function dateError(field, raw, today) {
  const v = scalar(raw);
  const m = v.match(ISO_DATE);
  if (!m) return `${field} "${v}" is not YYYY-MM-DD`;
  const d = new Date(`${v}T00:00:00Z`);
  if (Number.isNaN(d.getTime()) || d.toISOString().slice(0, 10) !== v) {
    return `${field} "${v}" is not a real date`;
  }
  // One day of slack for time zones: the repo is edited at UTC+7.
  if (d.getTime() > today.getTime() + 86_400_000) return `${field} ${v} is in the future`;
  return null;
}

// ---------------------------------------------------------------------------------------------
// Validation of one document
// ---------------------------------------------------------------------------------------------

/** All metadata errors for one file. `exists` resolves a repo-relative path (for superseded_by). */
export function validateDoc(file, text, { today, exists }) {
  if (isTemplate(file)) return [];
  const errors = [];
  const fm = parseFrontMatter(text);

  if (Object.hasOwn(NO_FRONT_MATTER, file)) {
    if (fm) errors.push('listed in NO_FRONT_MATTER but now has front matter -- remove the entry');
    return errors;
  }
  if (!fm) return ['no front matter (policy: "Metadata tối thiểu")'];

  const kind = scalar(fm.kind);
  const status = scalar(fm.status);
  const isArchive = file.startsWith('docs/archive/') && path.basename(file) !== 'README.md';
  const isRecord = RECORD_KINDS.includes(kind);

  const required = isRecord
    ? ['title', 'status', 'owner', 'created']
    : ['title', 'status', 'owner', 'last_reviewed'];
  if (!isRecord && needsReviewCycle(file)) required.push('review_cycle');
  if (isArchive) required.push('superseded_by');
  for (const k of required) if (!fm[k]) errors.push(`missing \`${k}\``);

  if (status) {
    const allowed = isArchive ? ARCHIVE_STATUS : isRecord ? RECORD_STATUS : STATUS;
    if (!allowed.includes(status)) {
      errors.push(`status "${status}" not in {${allowed.join(', ')}}`);
    } else if (STATUS_AI_ONLY.includes(status) && !file.startsWith('docs/ai/')) {
      errors.push(`status "${status}" is only valid under docs/ai/`);
    }
  }

  for (const k of ['last_reviewed', 'created', 'completed']) {
    if (fm[k]) {
      const e = dateError(k, fm[k], today);
      if (e) errors.push(e);
    }
  }
  if (kind === 'agent-goal' && status === 'completed' && !fm.completed) {
    errors.push('agent-goal with status completed needs `completed: YYYY-MM-DD`');
  }

  if (fm.review_cycle) {
    const c = scalar(fm.review_cycle);
    if (!Object.hasOwn(REVIEW_CYCLE_DAYS, c)) {
      errors.push(`review_cycle "${c}" not in {${Object.keys(REVIEW_CYCLE_DAYS).join(', ')}}`);
    }
  }

  if (isArchive && fm.superseded_by) {
    const target = scalar(fm.superseded_by);
    const resolved = path.normalize(path.join(path.dirname(file), target));
    if (!exists(resolved)) errors.push(`superseded_by -> ${target} does not exist`);
  }
  return errors;
}

// ---------------------------------------------------------------------------------------------
// Index coverage across the tree
// ---------------------------------------------------------------------------------------------

const LINK = /(^|[^!])\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g;
const FENCE = /^\s*(```|~~~)/;

/** Repo-relative targets of every relative markdown link in `text`, outside code fences. */
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
      if (target) out.add(path.normalize(path.join(path.dirname(file), target)));
    }
  }
  return out;
}

const isIndex = (f) => path.basename(f) === 'README.md' && !isTemplate(f);

/** `docs` is a Map(file -> text) of every docs/**.md file. Returns [file, message] pairs. */
export function checkIndexes(docs) {
  const errors = [];

  const dirs = new Set([...docs.keys()].map((f) => path.dirname(f)));
  for (const d of dirs) {
    if (!docs.has(path.join(d, 'README.md'))) {
      errors.push([d + '/', 'directory has documents but no README.md index']);
    }
  }

  const indexed = new Set();
  for (const [f, text] of docs) {
    if (!isIndex(f)) continue;
    for (const t of linkTargets(f, text)) if (t !== f) indexed.add(t);
  }
  for (const f of docs.keys()) {
    if (f === 'docs/README.md' || isTemplate(f)) continue;
    if (!indexed.has(f)) errors.push([f, 'not linked from docs/README.md or any docs/**/README.md']);
  }
  return errors;
}

// ---------------------------------------------------------------------------------------------
// Self-test: negative controls that must be rejected, positive controls that must pass
// ---------------------------------------------------------------------------------------------

function selfTest() {
  const today = new Date('2026-11-14T00:00:00Z');
  const exists = (p) => p === 'docs/planning/REAL.md';
  const fm = (lines) => `---\n${lines.join('\n')}\n---\n\n# Body\n`;
  const ok = ['title: T', 'status: active', 'owner: o', 'last_reviewed: 2026-11-01'];

  const mustFail = [
    ['docs/planning/A.md', '# no front matter\n'],
    ['docs/planning/A.md', fm(ok)], // review_cycle missing outside docs/ai
    ['docs/ai/design/A.md', fm(['title: T', 'status: draft', 'owner: o', 'last_reviewed: 2026-11-01'])],
    ['docs/ai/design/A.md', fm(['title: T', 'status: active', 'owner: o', 'last_reviewed: 2026-13-01'])],
    ['docs/ai/design/A.md', fm(['title: T', 'status: active', 'owner: o', 'last_reviewed: 2027-01-01'])],
    ['docs/ai/design/A.md', fm(['title: T', 'status: active', 'last_reviewed: 2026-11-01'])],
    ['docs/planning/A.md', fm([...ok, 'review_cycle: sau mỗi chặng'])],
    ['docs/planning/A.md', fm(['title: T', 'status: completed', 'owner: o', 'last_reviewed: 2026-11-01', 'review_cycle: monthly'])],
    ['docs/ai/planning/G.md', fm(['kind: agent-goal', 'title: T', 'status: active', 'owner: o'])],
    ['docs/ai/planning/G.md', fm(['kind: agent-goal', 'title: T', 'status: completed', 'owner: o', 'created: 2026-07-24'])],
    ['docs/ai/planning/R.md', fm(['kind: review-findings', 'title: T', 'status: historical', 'owner: o', 'created: 2026-07-27'])],
    ['docs/archive/X.md', fm(['title: T', 'status: active', 'owner: o', 'last_reviewed: 2026-07-24', 'superseded_by: ../planning/REAL.md'])],
    ['docs/archive/X.md', fm(['title: T', 'status: superseded', 'owner: o', 'last_reviewed: 2026-07-24', 'superseded_by: ../planning/GONE.md'])],
    ['docs/ai/AI_Context.md', fm(ok)], // allow-listed file that gained front matter
  ];
  const mustPass = [
    ['docs/planning/A.md', fm([...ok, 'review_cycle: monthly  # và sau mỗi chặng'])],
    ['docs/ai/planning/G.md', fm(['kind: agent-goal', 'title: T', 'status: completed', 'owner: o', 'created: 2026-07-24', 'completed: 2026-07-25'])],
    ['docs/archive/X.md', fm(['title: T', 'status: superseded', 'owner: o', 'last_reviewed: 2026-07-24', 'superseded_by: ../planning/REAL.md'])],
    ['docs/ai/design/README.md', '# template, exempt\n'],
  ];

  const failures = [];
  mustFail.forEach(([f, t], i) => {
    if (validateDoc(f, t, { today, exists }).length === 0) failures.push(`negative control #${i} (${f}) was accepted`);
  });
  mustPass.forEach(([f, t], i) => {
    const e = validateDoc(f, t, { today, exists });
    if (e.length) failures.push(`positive control #${i} (${f}) was rejected: ${e.join('; ')}`);
  });

  const tree = new Map([
    ['docs/README.md', '[p](planning/README.md)'],
    ['docs/planning/README.md', '[a](A.md)\n```\n[b](B.md)\n```'],
    ['docs/planning/A.md', ''],
    ['docs/planning/B.md', ''], // only linked inside a code fence: must be reported
    ['docs/orphan/C.md', '[a](../planning/A.md)'], // directory without README, and unindexed
  ]);
  const idx = checkIndexes(tree).map(([f]) => f).sort();
  const want = ['docs/orphan/', 'docs/orphan/C.md', 'docs/planning/B.md'];
  if (JSON.stringify(idx) !== JSON.stringify(want)) {
    failures.push(`index control: expected ${JSON.stringify(want)}, got ${JSON.stringify(idx)}`);
  }
  return { failures, controls: mustFail.length + mustPass.length + 1 };
}

// ---------------------------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------------------------

function main() {
  const report = process.argv.includes('--report');

  const st = selfTest();
  if (st.failures.length) {
    for (const f of st.failures) console.error(`self-test: ${f}`);
    console.error('docs metadata check: the checker itself is broken; fix it before trusting a green run');
    process.exit(2);
  }

  const files = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', 'docs/*.md'], {
    encoding: 'utf8',
  })
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean)
    .filter((f) => fs.existsSync(f)); // a tracked file deleted in the working tree

  const docs = new Map(files.map((f) => [f, fs.readFileSync(f, 'utf8')]));
  const today = new Date(new Date().toISOString().slice(0, 10) + 'T00:00:00Z');
  const exists = (p) => fs.existsSync(p);

  let errors = 0;
  for (const [f, text] of docs) {
    for (const e of validateDoc(f, text, { today, exists })) {
      errors += 1;
      console.error(`${f}: ${e}`);
    }
  }
  for (const [f, e] of checkIndexes(docs)) {
    errors += 1;
    console.error(`${f}: ${e}`);
  }
  for (const f of Object.keys(NO_FRONT_MATTER)) {
    if (!docs.has(f)) {
      errors += 1;
      console.error(`${f}: listed in NO_FRONT_MATTER but does not exist -- remove the entry`);
    }
  }

  // Overdue reviews: reported, not failed.
  const overdue = [];
  for (const [f, text] of docs) {
    const fm = parseFrontMatter(text);
    if (!fm || !fm.last_reviewed || !fm.review_cycle) continue;
    const days = REVIEW_CYCLE_DAYS[scalar(fm.review_cycle)];
    if (!days) continue;
    const age = Math.floor((today - new Date(`${scalar(fm.last_reviewed)}T00:00:00Z`)) / 86_400_000);
    if (age > days) overdue.push([age - days, f, scalar(fm.review_cycle), scalar(fm.last_reviewed), scalar(fm.owner)]);
  }
  overdue.sort((a, b) => b[0] - a[0]);
  if (report) {
    console.log('Overdue reviews (not a failure -- policy: show the owner, put it in the review backlog):');
    for (const [late, f, cycle, last, owner] of overdue) {
      console.log(`  ${String(late).padStart(4)}d late  ${cycle.padEnd(11)} last ${last}  owner ${owner}  ${f}`);
    }
  }

  console.log(
    `docs metadata check: ${docs.size} files, ${errors} errors, ${overdue.length} overdue reviews` +
      (report ? '' : ' (--report lists them)') +
      `; self-test ${st.controls} controls ok`,
  );
  process.exit(errors > 0 ? 1 : 0);
}

main();
