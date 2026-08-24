#!/usr/bin/env node
// Re-fetch every vendored licence text from its pinned URL and prove the committed bytes are those
// bytes. The only part of the licensing system that touches the network, and it is opt-in.
//
// # Why this is separate from the gates
//
// `scripts/lib/upstream_licenses.mjs` proves the store is internally consistent: the hashes match
// the files, the manifest matches the layout, every mapping is used. It cannot prove the files came
// from where they say they came from — that claim is only checkable against the upstream, and an
// upstream is a network dependency with an outage, a rate limit and a rewritten history.
//
// So the two are split. The offline half runs in CI and on every release gate. This half runs when
// someone wants the claim re-confirmed, and it *never* rewrites anything: a mismatch is reported and
// exits non-zero. A verifier that repairs what it finds is a verifier that turns a tampered store
// into a clean one and reports success.
//
// # What a failure means
//
// The URLs are pinned to 40-hex commits, so a fetch that returns different bytes than the store
// holds is not "upstream moved on". Either the store was edited after it was vendored, or the
// upstream rewrote history at that commit, or something is between this machine and GitHub. All
// three need a human, and none of them may be resolved by taking the new bytes.
//
// Usage: node scripts/verify_upstream_licenses.mjs [--keep]
//   --keep   leave the temporary download directory in place for inspection
//
// Exit codes: 0 every file matched · 1 a mismatch or a manifest problem · 2 the network was
// unusable, which is not a licensing failure and is reported as its own outcome.

import { createHash } from 'node:crypto';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import process from 'node:process';
import { loadUpstreamStore } from './lib/upstream_licenses.mjs';

const ROOT = process.cwd();
const KEEP = process.argv.includes('--keep');
const TIMEOUT_MS = 30_000;

const sha256 = (buf) => createHash('sha256').update(buf).digest('hex');

let store;
try {
  store = loadUpstreamStore(ROOT);
} catch (e) {
  console.error(`${e.message}\n\nThe store is not internally consistent; there is nothing to verify against.`);
  process.exit(1);
}

const sources = [...store.sources.values()].sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
if (sources.length === 0) {
  console.log('no vendored sources to verify');
  process.exit(0);
}

// A dedicated temp directory, created for this run and removed at the end. Downloads never land in
// the working tree: a fetched file inside `licensing/upstream/` would be indistinguishable from a
// vendored one on the next `git add -A`, which is how an unreviewed byte gets committed.
const scratch = mkdtempSync(join(tmpdir(), 'anima-upstream-verify-'));

const matched = [];
const mismatched = [];
const unreachable = [];

for (const source of sources) {
  let bytes;
  try {
    const res = await fetch(source.url, {
      signal: AbortSignal.timeout(TIMEOUT_MS),
      headers: { 'user-agent': 'anima-engine-licence-verifier' },
    });
    if (!res.ok) {
      unreachable.push(`${source.id}: HTTP ${res.status} ${res.statusText}`);
      continue;
    }
    bytes = Buffer.from(await res.arrayBuffer());
  } catch (e) {
    unreachable.push(`${source.id}: ${e.message}`);
    continue;
  }

  // Written out before comparing, so a mismatch leaves something a human can diff rather than only
  // two hashes that disagree.
  const scratchPath = join(scratch, `${sha256(Buffer.from(source.id, 'utf8')).slice(0, 16)}.fetched`);
  writeFileSync(scratchPath, bytes);

  const actual = sha256(bytes);
  if (actual === source.sha256 && bytes.length === source.bytes) {
    matched.push(source.id);
  } else {
    mismatched.push(
      `${source.id}\n` +
        `    url      : ${source.url}\n` +
        `    committed: sha256:${source.sha256}  ${source.bytes} bytes\n` +
        `    fetched  : sha256:${actual}  ${bytes.length} bytes\n` +
        `    fetched bytes written to ${scratchPath}`,
    );
  }
}

if (!KEEP && mismatched.length === 0) {
  rmSync(scratch, { recursive: true, force: true });
} else {
  console.log(`temporary downloads kept in ${scratch}`);
}

if (mismatched.length > 0) {
  console.error(
    `${mismatched.length} vendored licence file(s) do not match the bytes at their pinned URL:\n`,
  );
  for (const m of mismatched) console.error(`  x ${m}\n`);
  console.error(
    'These URLs are pinned to 40-hex commits, so this is not upstream moving on. Do not take the\n' +
      'fetched bytes: work out whether the store was edited or the upstream rewrote that commit,\n' +
      'and record the answer before changing anything.',
  );
  process.exit(1);
}

if (unreachable.length > 0) {
  console.error(`could not reach ${unreachable.length} of ${sources.length} pinned URL(s):`);
  for (const u of unreachable) console.error(`  ? ${u}`);
  console.error(
    `\n${matched.length} file(s) did match. A network that cannot be reached is not a licensing\n` +
      'failure and is not reported as one; run again with a working connection. The offline gates\n' +
      '(`npm run check:licenses`) are unaffected and remain the ones CI depends on.',
  );
  process.exit(2);
}

const repos = new Set(sources.map((s) => s.repository)).size;
const commits = new Set(sources.map((s) => s.commit)).size;
console.log(
  `upstream licence verification: ${matched.length} file(s) from ${commits} commit(s) across ` +
    `${repos} repositories match their pinned URLs byte for byte`,
);
