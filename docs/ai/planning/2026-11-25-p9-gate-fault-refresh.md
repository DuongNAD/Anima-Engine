---
phase: planning
feature: gate-controls
title: P9 GATE-FAULT-REFRESH — Pin scripts/paper/gate_faults/*.patch to LF in .gitattributes
description: Root-cause fix and full Windows NTFS verification for gate_faults patch checkout under core.autocrlf=true, correcting the WV2-5 "ten rotted patches" artifact.
status: proposed
owner: anima-case
last_reviewed: 2026-11-25
---

# `P9` — `GATE-FAULT-REFRESH` (`MSG_20261125_025100`)

## Root Cause & Correction of `WV2-5` Note

This card corrects the non-blocking note in `verdicts/WV2-5_e4cae414.md` that described `10` patches under `scripts/paper/gate_faults/*.patch` as having drifted from trunk ("ten rotted patches"). **None of the `55` patches on trunk had drifted or rotted.**

On trunk (`a146d2f2` and `9a53bad6`), piping every committed pure-LF patch blob directly to `git apply --check --cached` (`git show HEAD:scripts/paper/gate_faults/*.patch`) gives:

- **`PURE_LF_INDEX_CHECK: TOTAL=55 OK=55 FAIL=0`** — all `55/55` patches already apply cleanly against trunk as LF.

All Windows failures came from missing LF normalization in `.gitattributes`:
- Git on Windows defaults to `core.autocrlf=true`. Because `.gitattributes` pinned `NOTICE`, `sbom.cdx.json`, `licensing/**`, `animal-map.manifest.json`, and `artifacts/experiments/**` to LF (`i/lf w/lf`) while omitting `scripts/paper/gate_faults/*.patch`, Windows checked out all `55` `.patch` files as `i/lf w/crlf`.
- Direct `git apply --check` failed on the `10` LF-pinned targets (plus binary-diff `check_text_hygiene.patch`) due to CR vs LF context mismatch, and `scripts/paper/gate_controls.mjs` (`withTempWorktree`, `--self-test`) creates detached worktrees with `-c core.autocrlf=false` and pipes raw host `.patch` bytes via `readFileSync`, failing all `55/55` patches.

## Authorized Change (`MSG_20261125_025100`)

- **Only code/config change:** appended **exactly one line** to `.gitattributes` (`+1/-0`):
  ```gitattributes
  scripts/paper/gate_faults/*.patch text eol=lf
  ```
- **`0` `.patch` edits** and **`0` changes to `scripts/paper/gate_controls.mjs`** or any gate.
- After re-checking out `scripts/paper/gate_faults/` in `win/p9-gate-fault-refresh`, `git ls-files --eol scripts/paper/gate_faults/*.patch` reports `i/lf w/lf attr/text eol=lf` on all **`55/55`** files and `git status --porcelain` is clean aside from `.gitattributes` and this note.
- On a fresh Windows `core.autocrlf=true` NTFS clone (`%TEMP%\anima-p9-ntfs`), all **`55/55`** patches check out automatically as `i/lf w/lf attr/text eol=lf`.

> [!NOTE]
> Drive `D:\` on Windows 1 is `exFAT`, where cross-volume temp worktrees in `C:\Users\Admin\AppData\Local\Temp` trigger git `dubious ownership` and `exFAT` does not support NTFS directory junctions inside `gate_controls.mjs:332` (`linkNodeModules()`). Full 62-gate `node scripts/paper/gate_controls.mjs` verification was therefore executed in a fresh Windows `core.autocrlf=true` NTFS clone (`P9_NTFS_FULL_EXIT=0`), while `--self-test` and `frontend/gateControls.test.ts` were verified in both the `D:\` worktree and the NTFS clone.

## Verification Summary (Windows 1)

| Check | Before (`core.autocrlf=true`, no `.gitattributes` pin) | After (`+1` line in `.gitattributes`) |
|---|---|---|
| `git ls-files --eol scripts/paper/gate_faults/*.patch` | `55/55` `i/lf w/crlf` | `55/55` `i/lf w/lf attr/text eol=lf` |
| `node scripts/paper/gate_controls.mjs --self-test` | **FAIL (`EXIT 1`)** — `55/55` patches fail `git apply --check` | **PASS (`EXIT 0`)** — `63 controls ok (62 gates registered)` |
| `npx vitest run --config vitest.config.ts --root tests frontend/gateControls.test.ts` | **FAIL (`EXIT 1`)** | **PASS (`EXIT 0`, `16/16` tests)** |
| `node scripts/paper/gate_controls.mjs` (Windows NTFS clone) | Blocked on every patch apply | **`52 CATCHES`, `1 VACUOUS`, `2 UNMATCHED_FAILURE`, `3` static `NOT RUN`, `4 NOT RUN (baseline red)`** |

## Before / After Table Across All 62 Registered Gates

Notes on non-`CATCHES` verdicts observed (all `55` patches applied cleanly without error):
- `ci_licenses_complete` -> `VACUOUS` (`exit=0`) by design: `ci.yml` omits `--require-complete` while `check_licenses_complete` passes it and reports `CATCHES`.
- `check_licenses` -> `UNMATCHED_FAILURE` (`exit=1`): fault patch applied and `gen_third_party_licenses.mjs --check` caught it and exited `1` (`"licensing/ is out of date with the dependency graph: third-party-index.json."`), while `gate_controls.mjs:1541` expects regex `/third-party-index\.json is out of date/`.
- `check_ipc_arg_case` -> `UNMATCHED_FAILURE` (`exit=1`): fault patch applied and `check_ipc_reference.mjs` caught the mismatch and exited `1`, printing Windows backslash path `src\components\Landscape\utils\worldCache.ts:241` whereas `gate_controls.mjs:1006` expects forward slashes.
- `conflict_marker_scan` (`NOT RUN (no automated gate)`), `cargo_test_desktop_full` (`NOT RUN (time)`), `hardening_stability_5x` (`NOT RUN (time)`): static M2 entries.
- `ci_ipc_arg_case`, `ci_e3`, `vitest_tests`, `check_e2e_specs`: `NOT RUN (baseline red)` on Windows due strictly to pre-existing Windows-only failures unrelated to LF checkout (see below) and missing Playwright install in temp worktree.

| Gate ID | Patch File | Before (Windows CRLF checkout) | After (`.gitattributes` LF pin) | `gate_controls.mjs` Verdict (Windows NTFS) |
|---|---|---|---|---|
| `ci_licenses_complete` | `check_licenses_complete.patch` | FAIL (CRLF `--self-test` & direct `git apply` CR mismatch) | OK (`i/lf w/lf`) | `VACUOUS` (`exit=0`, 12.5s) |
| `check_licenses` | `check_licenses.patch` | FAIL (CRLF `--self-test` & direct `git apply` CR mismatch) | OK (`i/lf w/lf`) | `UNMATCHED_FAILURE` (`exit=1`, 3.76s) |
| `check_ipc_arg_case` | `check_ipc_arg_case.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `UNMATCHED_FAILURE` (`exit=1`, 2.6s) |
| `ci_ipc_arg_case` | `check_ipc_arg_case.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `NOT RUN (baseline red)` (`exit=1`, 200.56s) |
| `ci_e3` | `check_e3.patch` | FAIL (CRLF `--self-test` & direct `git apply` CR mismatch) | OK (`i/lf w/lf`) | `NOT RUN (baseline red)` (`exit=1`, 169.32s) |
| `conflict_marker_scan` | — (static) | N/A (static) | N/A (static) | `NOT RUN (no automated gate)` (`exit=—`, 0s) |
| `cargo_test_desktop_full` | — (static) | N/A (static) | N/A (static) | `NOT RUN (time)` (`exit=—`, 0s) |
| `hardening_stability_5x` | — (static) | N/A (static) | N/A (static) | `NOT RUN (time)` (`exit=—`, 0s) |
| `vitest_tests` | `vitest_tests.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `NOT RUN (baseline red)` (`exit=1`, 157.91s) |
| `check_e2e_specs` | `check_e2e_specs.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `NOT RUN (baseline red)` (`exit=2`, 1.04s) |
| `thread_rng_scan` | `thread_rng_scan.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 2.1s) |
| `check_test_targets_unscheduled` | `check_test_targets_unscheduled.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 662.2s) |
| `check_licenses_complete` | `check_licenses_complete.patch` | FAIL (CRLF `--self-test` & direct `git apply` CR mismatch) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 9.64s) |
| `thread_rng_scan_crates` | `thread_rng_scan_crates.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 1.28s) |
| `cargo_fmt` | `cargo_fmt.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 47.24s) |
| `cargo_audit` | `cargo_audit.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 11.4s) |
| `cargo_deny` | `cargo_deny.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=4`, 15.72s) |
| `cargo_clippy_desktop` | `cargo_clippy_desktop.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=101`, 89.58s) |
| `cargo_clippy_worldgen` | `cargo_clippy_worldgen.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=101`, 18.45s) |
| `feature_separation` | `feature_separation.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 48.92s) |
| `feature_separation_engine` | `feature_separation_engine.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 4.14s) |
| `feature_separation_worldgen` | `feature_separation_worldgen.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 3.8s) |
| `ts_rs_parity` | `ts_rs_parity.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 78.07s) |
| `check_test_targets_empty` | `check_test_targets_empty.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 108.84s) |
| `check_test_targets_ignored` | `check_test_targets_ignored.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 111.42s) |
| `check_test_targets_unscheduled_profile` | `check_test_targets_unscheduled.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 1.15s) |
| `cargo_test_worldgen` | `cargo_test_worldgen.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=101`, 232.34s) |
| `check_test_targets_worldgen_unscheduled` | `check_test_targets_worldgen_unscheduled.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 209.67s) |
| `check_test_targets_worldgen_empty` | `check_test_targets_worldgen_empty.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 204.32s) |
| `feature_gated_targets` | `feature_gated_targets.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=101`, 96.66s) |
| `schedule_ambiguity_simrng` | `schedule_ambiguity_simrng.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=101`, 93.5s) |
| `schedule_ambiguity_energy` | `schedule_ambiguity_energy.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=101`, 48.13s) |
| `zero_alloc` | `zero_alloc.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=101`, 95.15s) |
| `closed_eu` | `closed_eu.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=101`, 93.83s) |
| `live_experiment_fork` | `live_experiment_fork.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=101`, 100.91s) |
| `live_experiment_saveload` | `live_experiment_saveload.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=101`, 50.7s) |
| `determinism_golden_hash` | `determinism_golden_hash.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 133.19s) |
| `check_sbom` | `check_sbom.patch` | FAIL (CRLF `--self-test` & direct `git apply` CR mismatch) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 5.84s) |
| `check_sbom_schema` | `check_sbom_schema.patch` | FAIL (CRLF `--self-test` & direct `git apply` CR mismatch) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 3.13s) |
| `check_notice` | `check_notice.patch` | FAIL (CRLF `--self-test` & direct `git apply` CR mismatch) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 6.03s) |
| `check_flora_footprint` | `check_flora_footprint.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 2.99s) |
| `verify_dataflow` | `verify_dataflow.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 69.86s) |
| `check_npm_audit` | `check_npm_audit.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 12.93s) |
| `check_text_hygiene` | `check_text_hygiene.patch` | FAIL (CRLF `--self-test` & direct `git apply` CR mismatch) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 3.61s) |
| `check_root_hygiene` | `check_root_hygiene.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 2.68s) |
| `check_e2` | `check_e2.patch` | FAIL (CRLF `--self-test` & direct `git apply` CR mismatch) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 2.97s) |
| `check_e3` | `check_e3.patch` | FAIL (CRLF `--self-test` & direct `git apply` CR mismatch) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 2.9s) |
| `npm_lint` | `npm_lint.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 24.77s) |
| `eslint_ratchet` | `eslint_ratchet.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 23.91s) |
| `typecheck_tests` | `typecheck_tests.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=2`, 13.43s) |
| `vitest_src` | `vitest_src.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 22.22s) |
| `npm_build` | `npm_build.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=2`, 11.16s) |
| `check_bundle_closure` | `check_bundle_closure.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 13.03s) |
| `check_csp` | `check_csp.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 12.77s) |
| `check_bundle` | `check_bundle.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 12.32s) |
| `playwright_e2e` | `playwright_e2e.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 152.41s) |
| `check_docs_links` | `check_docs_links.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 2.72s) |
| `check_docs_metadata` | `check_docs_metadata.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 2.33s) |
| `check_ipc_reference` | `check_ipc_reference.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 2.2s) |
| `check_map_vision_manifest` | `check_map_vision_manifest.patch` | FAIL (CRLF `--self-test` & direct `git apply` CR mismatch) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 12.35s) |
| `verify_upstream_licenses` | `verify_upstream_licenses.patch` | FAIL (CRLF `--self-test` & direct `git apply` CR mismatch) | OK (`i/lf w/lf`) | `CATCHES` (`exit=1`, 14.49s) |
| `compare_checksums_self_test` | `compare_checksums_self_test.patch` | FAIL (CRLF `--self-test`) | OK (`i/lf w/lf`) | `CATCHES` (`exit=2`, 1.93s) |

## Read-Only Report: Other Two Windows-Only Frontend Test Failures (Not Fixed in `P9`)

Per `MSG_20261125_025100`, both remaining pre-existing Windows `npm run test:frontend` failures were diagnosed read-only without edits:

1. **`tests/frontend/checkDocCommands.test.ts` (`2` failed / `7` passed):** **Does NOT share the CRLF cause.**
   - `executes blocks in order with bash -eo pipefail ...` (`tests/frontend/checkDocCommands.test.ts:216`): exited `1` on block `1` (`docs/01-pass-then-fail.md:2`) rather than block `2` (`:7`) because `scripts/check_doc_commands.mjs:169` spawns bare `'bash'` on Windows (picking up WSL `C:\Windows\system32\bash.exe` without a configured distro rather than Git Bash).
   - `fails when any cargo test block prints running 0 tests ...` (`tests/frontend/checkDocCommands.test.ts:309`): failed temp cleanup with Windows `Error: EBUSY: resource busy or locked, rmdir ...` (`rmSync` at line `44` lacks retries; line `281` also joins `PATH` with POSIX `':'` instead of `path.delimiter`).
2. **`tests/frontend/incidentDates.test.ts` (`1` failed / `12` passed):** **Does NOT share the CRLF cause.**
   - `executes main() when invoked via a symlink entrypoint` (`tests/frontend/incidentDates.test.ts:411`): threw `Error: EPERM: operation not permitted, symlink` because Windows `symlinkSync` requires Administrator / Developer Mode privilege. Notice the test explicitly exercising LF vs CRLF files (`rejects physicalLines + 1 on LF and CRLF committed files at pinned anchors`, line `449`) **passed**.
