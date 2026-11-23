---
title: How to Add a New CI Gate
status: active
owner: maintainers
last_reviewed: 2026-11-23
review_cycle: per-release
---

# How to Add a New CI Gate

This guide walks through creating a new gate script, attaching mandatory **negative controls**, and wiring it into [`package.json`](../../package.json) and [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml).

> **Context & rationale behind existing gates:** see [`STATE_OF_THE_PROJECT.md` §2.1](../planning/STATE_OF_THE_PROJECT.md#21-vì-sao-bộ-gate-này-đáng-tin) and [`COMPLETION_ROADMAP_2026-10-16.md` §1.2](../planning/COMPLETION_ROADMAP_2026-10-16.md#12-sáu-lỗi-quy-trình-đã-dẫn-tới-đây--mỗi-lỗi-thành-một-gate-ở-ht04).

---

## Step 1 — Follow Exit-Code, Scan-Scope, and Bidirectional Allow-List Conventions

New gate scripts under `scripts/` should follow these conventions:

- **Exit codes (convention for new gates):**
  - `0`: all checks passed.
  - `1`: contract, budget, or policy violation detected in repository or artifact files.
  - `2`: convention for configuration errors, missing inputs, zero-match parser failures, or **inline negative control / `--self-test` failures**. *(Note: not every existing gate exits `2` on `--self-test` failures — for example, `verify_e3_artifacts.mjs --self-test`, `verify_esel1_artifacts.mjs --self-test`, and `verify_sel2p_artifacts.mjs --self-test` exit `1`.)*
- **Include untracked files during authoring:** text and documentation gates (like `check_text_hygiene.mjs` and `check_docs_links.mjs`) enumerate files with `git ls-files -z --cached --others --exclude-standard` so errors fail before commit.
- **Bidirectional allow-lists (when exemptions exist):** like [`scripts/test_target_policy.mjs`](../../scripts/test_target_policy.mjs) and [`scripts/root_allowlist.json`](../../scripts/root_allowlist.json), fail both on **unlisted entries** and on **stale entries that no longer apply**, with an explicit reason string for every exemption.
- **Guard against zero-match vacuity:** never exit `0` if a parser matches `0` items (for example `check_ipc_reference.mjs` exits `2` if zero handlers are parsed, and `check_test_targets.mjs` cross-checks `ignored` totals against `libtest` output).

---

## Step 2 — Attach Mandatory Negative Controls

A gate without negative controls can silently pass `0` forever when a regex or path drifts. Trunk uses two patterns depending on input setup cost:

### Pattern A — Inline `selfTest()` Before Scanning (Fast Parsers & Validators)

Used by in-memory gates such as [`scripts/check_docs_metadata.mjs`](../../scripts/check_docs_metadata.mjs), [`scripts/check_ipc_reference.mjs`](../../scripts/check_ipc_reference.mjs), [`scripts/check_root_hygiene.mjs`](../../scripts/check_root_hygiene.mjs), and [`scripts/check_npm_audit.mjs`](../../scripts/check_npm_audit.mjs):

- Author `selfTest()` covering both `mustFail` fault fixtures and `mustPass` valid fixtures.
- Call `selfTest()` **unconditionally before inspecting repo files**; if any fault fixture slips through or valid fixture fails, print an error and exit (`process.exit(2)`).

### Pattern B — Dedicated `--self-test` CLI Mode + Vitest Unit Tests (Artifact / File-Tree Gates)

Used by disk/artifact verifiers such as [`scripts/verify_e3_artifacts.mjs`](../../scripts/verify_e3_artifacts.mjs), [`scripts/verify_esel1_artifacts.mjs`](../../scripts/verify_esel1_artifacts.mjs), [`scripts/verify_sel2p_artifacts.mjs`](../../scripts/verify_sel2p_artifacts.mjs), and [`scripts/paper/compare_checksums.mjs`](../../scripts/paper/compare_checksums.mjs):

- Support a `--self-test` flag creating temporary directories with `fs.mkdtempSync`, injecting each fault class (corrupted SHA-256, missing file, edited preregistration threshold, etc.), and asserting failure.
- Guard the CLI entrypoint with `if (argv[1] && import.meta.url === pathToFileURL(argv[1]).href) main()` so pure validation functions can also be exercised directly in Vitest (as done by [`scripts/check_bundle_budget.mjs`](../../scripts/check_bundle_budget.mjs) + `tests/frontend/checkBundleBudget.test.ts`, even when a script has no standalone `--self-test` CLI flag) alongside subprocess `spawnSync` checks.

---

## Step 3 — Prove the Gate Turns RED on an Injected Fault

Before considering a gate finished, verify that injecting a real fault fails with non-zero status and names the offending input. The example below copies committed E2 smoke artifacts into gitignored `src-tauri/target/doc2-gate-neg/`, perturbs `per-seed-deltas.csv`, and confirms `verify_e2_artifacts.mjs` rejects it:

<!-- doc-test -->
```bash
rm -rf src-tauri/target/doc2-gate-neg
mkdir -p src-tauri/target/doc2-gate-neg
cp -R artifacts/experiments/e2-evolved-brain-default/smoke src-tauri/target/doc2-gate-neg/smoke
node -e "const fs = require('fs'); const p = 'src-tauri/target/doc2-gate-neg/smoke/per-seed-deltas.csv'; const lines = fs.readFileSync(p, 'utf8').split('\n'); lines[1] = lines[1].replace(/[^,]+$/, '999999.0'); fs.writeFileSync(p, lines.join('\n'));"
! node scripts/verify_e2_artifacts.mjs src-tauri/target/doc2-gate-neg
```

Observed output (`macOS arm64`, `2026-11-22`, commit `03b9d3d`; script exited `1`, negated with `!` to assert rejection):

```text
note  analysis: no ensemble has been run yet (no paired-report.json at the artifact root)
note  smoke: recomputed 22 per-seed deltas
note  analysis: no execution-lock.json — the duration rung was not locked from a smoke calibration

FAIL  smoke: per-seed-deltas.csv hashes to 1b3313e1..., but checksums.sha256 says a459a031...
FAIL  smoke: seed 999983 detritus: delta 999999.0 is not 1019.997442117276 - 1019.9974421173067 (= -3.069544618483633e-11)

2 failure(s).
```

---

## Step 4 — Wire Into `package.json` and `.github/workflows/ci.yml`

1. Add `"check:<name>": "node scripts/check_<name>.mjs"` under `"scripts"` in [`package.json`](../../package.json).
2. Open [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) and pick the right job:
   - **`rust`** (`windows-latest`, `working-directory: src-tauri`): for Cargo, `cargo tree`, test-target policy, or Windows-host license/compliance checks.
   - **`frontend`** (`ubuntu-latest`): for Node hygiene/docs/IPC/artifact checks, ESLint, TypeScript, Vitest, and Playwright.
3. Add `if: ${{ !cancelled() }}` on the step so earlier check failures still allow downstream gates to run and report all diagnostics in one CI pass.
4. If adding unit tests under `tests/frontend/`, run `npm run typecheck:tests` and `npm run test:frontend` (`tests/` installs its own `node_modules` via `npm ci --prefix tests`).

---

## Step 5 — Run Existing Hygiene, Docs, and `--self-test` Gates

Run the repository docs/hygiene gates and existing `--self-test` suites from the repo root:

<!-- doc-test -->
```bash
npm run check:docs
npm run check:text-hygiene
npm run check:root-hygiene
node scripts/verify_esel1_artifacts.mjs --self-test
node scripts/verify_e3_artifacts.mjs --self-test
node scripts/verify_sel2p_artifacts.mjs --self-test
```

Observed output (`macOS arm64`, `2026-11-22`, commit `03b9d3d`, trimmed; authoritative repo totals live in [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền)):

```text
docs link check: … relative links + … same-page anchors in … files, 0 broken
…
docs metadata check: … files, 0 errors, …; self-test 21 controls ok
ipc reference check: … commands, … events, 0 differences
text hygiene: … tracked text file(s), no raw control bytes
root hygiene: … root entries, all listed in scripts/root_allowlist.json; self-test 4 controls ok
self-test ok    mean, sd, se of deltas 1,2,3
… (verify_esel1_artifacts: 8/8 ok)
verify_e3_artifacts --self-test PASSED (20/20 ok)
… (verify_sel2p_artifacts: 34/34 ok)
```

---

## Next Steps

- Coding agent workflow and ratchets: [AGENT_WORKFLOW.md](AGENT_WORKFLOW.md).
- Rationale behind every gate family: [Explanation index](../explanation/README.md).
