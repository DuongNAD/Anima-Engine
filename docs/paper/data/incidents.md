---
title: Incident catalogue — documented failures where a gate exited 0 while something was wrong
status: active
owner: maintainers
last_reviewed: 2026-11-23
review_cycle: per-release
---

# Incident Catalogue (Card P1)

Machine-verifiable data file: [`incidents.csv`](incidents.csv) (47 incidents, all with `was_suite_green = yes`, verified at trunk `origin/feat/ecological-selection` commit `6495c5c`).

## Scope and Sources

This dataset catalogues documented incidents in the repository where the relevant test suite, gate, script, or build check exited `0` (reported green/clean/valid) while a defect, falsification, blind agent sensor, stale/broken check, or nondeterminism was present (`was_suite_green = yes`). Rows where the suite or CI was already red (`was_suite_green = no`) are excluded from [`incidents.csv`](incidents.csv) and listed below under [Excluded by Scope](#excluded-by-scope). All rows trace to the five Card P1 source sets:

1. [`FINDINGS.md`](../../../FINDINGS.md)
2. [`COMPLETION_ROADMAP_2026-10-16.md` §1.2 and §4.1](../../planning/COMPLETION_ROADMAP_2026-10-16.md#12-sáu-lỗi-quy-trình-đã-dẫn-tới-đây--mỗi-lỗi-thành-một-gate-ở-ht04)
3. [`STATE_OF_THE_PROJECT.md` §1.e, §3.0, and §4](../../planning/STATE_OF_THE_PROJECT.md)
4. Original E2 report §7 preserved at [`superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md`](../../../artifacts/experiments/e2-evolved-brain-default/superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md) (superseding the Amendment 1 path [`RESULT.md`](../../../artifacts/experiments/e2-evolved-brain-default/RESULT.md))
5. Commit messages of `fa938c5` (#38) and `03b331f` (#39)

## Column and Verification Contract

Verification script: [`scripts/paper/incident_dates.mjs`](../../../scripts/paper/incident_dates.mjs) (covered by [`tests/frontend/incidentDates.test.ts`](../../../tests/frontend/incidentDates.test.ts)).

- `id`: Unique row identifier (`INC-01` through `INC-53`, excluding the 6 `was_suite_green = no` entries listed below).
- `title`: Concise factual description of the failure.
- `class`: One of `vacuous-gate`, `fabricated-evidence`, `loosened-test`, `narrowed-command`, `silent-semantic-change`, `nondeterminism`, `contract-overwrite`, `toolchain-drift`, `other`.
- `introduced_commit` / `fixed_commit`: Abbreviated git commit hash on `origin/feat/ecological-selection` verified in batch via `git rev-list HEAD` ancestry and `git cat-file --batch-check`. At least one commit hash is non-empty on every row; `""` denotes an unknown/unattributed origin (`INC-21`, `INC-22`, `INC-24`, `INC-27`–`INC-31`, `INC-39`, `INC-48`) or an unfixed finding at `6495c5c` (`INC-44`).
- `detected_how`: Mechanism or check that surfaced the failure.
- `introduced_date` / `fixed_date`: Exact committer timestamp from `git log --no-walk --format=%H%x09%cI` (empty `""` when the corresponding commit column is empty).
- `detected_date`: Calendar date (`YYYY-MM-DD`) when the incident was first detected or logged in its source text, verified to lie within `[introduced_date, fixed_date]` by day when those endpoints exist.
- `days_to_fix`: Non-negative integer elapsed 24-hour days between introduction and remediation computed as `Math.round((Date.parse(fixed_date) - Date.parse(introduced_date)) / 86_400_000)` (empty `""` when either commit is empty).
- `was_suite_green`: `yes`, `no`, or `unknown` (`yes` on every row retained in `incidents.csv`).
- `source`: Pinned `<path>@<sha>:<line>` anchor (verified via `git show <sha>:<path>` against an ancestor commit of `HEAD`, non-blank line within physical line bounds).

Re-run verification:

```bash
node scripts/paper/incident_dates.mjs --check
node scripts/paper/incident_dates.mjs --self-test
```

## Summary Table

| ID | Title | Class | Introduced | Fixed | Detected Date | Days to Fix | Source |
|---|---|---|---|---|---|---:|---|
| INC-01 | ScheduleGraph::systems empty after initialization made ambiguity gate vacuous | `vacuous-gate` | `993a587` | `ec94933` | 2026-07-27 | 0 | `artifacts/experiments/e2-evolved-brain-default/superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md@6495c5c:178` |
| INC-02 | ecosystem_census_system unordered against EU-moving systems (0.186 EU per-process drift) | `nondeterminism` | `6a42014` | `993a587` | 2026-07-27 | 24 | `artifacts/experiments/e2-evolved-brain-default/superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md@6495c5c:177` |
| INC-03 | Lifecycle readiness test slept blind 500 ms with 120-220 ms real headroom | `other` | `af58e74` | `5f0383f` | 2026-07-27 | 41 | `artifacts/experiments/e2-evolved-brain-default/superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md@6495c5c:180` |
| INC-04 | DEFAULT_GRID_DIM stayed 128 across 9 contract, generator, and test sites while world ran 256 | `contract-overwrite` | `73237fb` | `250e03d` | 2026-07-27 | 2 | `docs/planning/STATE_OF_THE_PROJECT.md@6495c5c:976` |
| INC-05 | Five wrong camelCase/snake_case IPC keys across 4 commands frozen by 6 mock tests | `vacuous-gate` | `af58e74` | `19e67fa` | 2026-07-27 | 42 | `docs/planning/STATE_OF_THE_PROJECT.md@6495c5c:390` |
| INC-06 | tests/ package never typechecked (86 hidden TypeScript errors and type-escape assertions) | `vacuous-gate` | `81871de` | `fb03692` | 2026-07-27 | 45 | `docs/planning/STATE_OF_THE_PROJECT.md@6495c5c:1587` |
| INC-07 | E2E global-setup.ts lacked Tauri mock transport and console watcher (8 TypeErrors ignored on exit 0) | `vacuous-gate` | `2752112` | `1cddeb5` | 2026-07-27 | 2 | `docs/planning/STATE_OF_THE_PROJECT.md@6495c5c:411` |
| INC-08 | capture.config.ts polled 127.0.0.1 while Vite bound ::1; capture:views dead while CI hashed committed PNGs | `vacuous-gate` | `fd3dc79` | `0f5b4d3` | 2026-07-27 | 1 | `docs/planning/STATE_OF_THE_PROJECT.md@6495c5c:512` |
| INC-09 | tests/vitest.config.ts hardcoded maxWorkers: 4 causing timeout failures on 4-vCPU CI runners | `other` | `3fb691d` | `72a6e34` | 2026-07-27 | 1 | `docs/planning/STATE_OF_THE_PROJECT.md@6495c5c:503` |
| INC-10 | check_test_targets.mjs required Running <path> ( on one line, silently skipping wrapped PowerShell targets | `vacuous-gate` | `2752112` | `4b6258d` | 2026-07-29 | 14 | `docs/planning/STATE_OF_THE_PROJECT.md@6495c5c:1606` |
| INC-11 | observer_trace_zero_alloc_tests used process-wide allocator counter despite single #[test], catching libtest main thread | `other` | `101d327` | `4b6258d` | 2026-07-29 | 13 | `docs/planning/STATE_OF_THE_PROJECT.md@6495c5c:1613` |
| INC-12 | eslint.config.js scanned .worktrees/** (4656 false local errors while clean CI stayed green) | `other` | `b2243cf` | `4b6258d` | 2026-07-29 | 39 | `docs/planning/STATE_OF_THE_PROJECT.md@6495c5c:1600` |
| INC-16 | justfile:151 passed nonexistent --count flag to verify_dataflow.py, silently printing fallback text (F-026) | `vacuous-gate` | `36401ee` | `b8d1fa7` | 2026-10-31 | 98 | `FINDINGS.md@6495c5c:493` |
| INC-17 | MapElitesArchive::select_parent iterated HashMap, making evolution non-reproducible across processes (F-001) | `nondeterminism` | `81871de` | `5f792ed` | 2026-08-11 | 60 | `FINDINGS.md@6495c5c:960` |
| INC-18 | is_valid_genotype had zero negative-control tests (return true mutant survived, F-008) | `vacuous-gate` | `81871de` | `f54f337` | 2026-08-11 | 60 | `FINDINGS.md@6495c5c:1091` |
| INC-19 | adversarial_coverage_tests used process-wide allocator counter racing sibling test threads (F-023) | `nondeterminism` | `81871de` | `445c9c8` | 2026-08-26 | 75 | `FINDINGS.md@6495c5c:580` |
| INC-20 | src-tauri/icons/ held 12-byte placeholder text files; Windows CI compiled while macOS/Linux panicked (F-028) | `other` | `81871de` | `843ee74` | 2026-10-31 | 155 | `FINDINGS.md@6495c5c:452` |
| INC-21 | Audit recommendation SIM-01 blinded every agent sensor; raycast fixture moved x=99->90 to keep suite green | `loosened-test` | — | `ef9bc3a` | 2026-10-16 | — | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md@6495c5c:107` |
| INC-22 | Narrow commit title fix(sensors) altered 5 simulation semantics without re-measuring published E2 evidence | `silent-semantic-change` | — | `ef9bc3a` | 2026-10-16 | — | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md@6495c5c:114` |
| INC-23 | 970d963 appended live.dormant_eu to frozen E2 manifests while cargo test passed, breaking check:e2 until Amendment 1 | `contract-overwrite` | `970d963` | `30e70b7` | 2026-11-16 | 83 | `docs/planning/STATE_OF_THE_PROJECT.md@6495c5c:262` |
| INC-24 | PROJECT.md overwritten twice by agent spec/audit docs, erasing IPC contract pointer | `contract-overwrite` | — | `491ec61` | 2026-10-16 | — | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md@6495c5c:112` |
| INC-27 | Map 2 verify_map2_evidence.ts wrote 1x1 PNG + HTML comment per view for distinct hashes and faked TEST_READY.md 100% pass | `fabricated-evidence` | — | `d703c01` | 2026-10-16 | — | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md@6495c5c:123` |
| INC-28 | Telemetry and export hardcoded seed 42, 0.70/0.30 trophic split, 10000.0 biomass, and fallback floors (G3) | `fabricated-evidence` | — | `133a845` | 2026-10-16 | — | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md@6495c5c:124` |
| INC-29 | Research Dashboard used Math.sin curves, fake checksums, and mock export fallback for nonexistent IPC commands (G4) | `fabricated-evidence` | — | `0dd4d42` | 2026-10-16 | — | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md@6495c5c:125` |
| INC-30 | Exact count assertions weakened to >=, constant compared to itself, and 4 anima-lab tests deleted (G7) | `loosened-test` | — | `95fc481` | 2026-10-16 | — | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md@6495c5c:125` |
| INC-31 | tests/package.json narrowed test:e2e to e2e/tiers/, silently skipping 18 CI specs (G2) | `narrowed-command` | — | `b507bb4` | 2026-10-16 | — | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md@6495c5c:126` |
| INC-33 | 1d301c3 added 5th ResMut<SimRng> system unordered while schedule_ambiguity_tests checked EU systems only (F-029, #39) | `nondeterminism` | `1d301c3` | `03b331f` | 2026-10-31 | 10 | `FINDINGS.md@6495c5c:401` |
| INC-34 | autonomous_reproduction_system bypassed VitalStatistics, reporting live.births = 0 while agents bred (F-039) | `silent-semantic-change` | `1d301c3` | `b0a8c4d` | 2026-11-16 | 26 | `FINDINGS.md@6495c5c:166` |
| INC-35 | agent_mortality_system and mortality_system both scheduled: starve_ticks bypassed and live.deaths = 0 (F-037) | `silent-semantic-change` | `1d301c3` | `27e85bb` | 2026-11-16 | 26 | `FINDINGS.md@6495c5c:187` |
| INC-36 | crossover_genotypes exceeded MAX_MIGRATION_MORPHOLOGY_NODES (128), breaking restore and migration (F-038) | `other` | `5de4451` | `a47d534` | 2026-11-16 | 110 | `FINDINGS.md@6495c5c:176` |
| INC-37 | resolve_joints_system depended on ECS table order after deaths, diverging post-restore (F-040) | `nondeterminism` | `1d301c3` | `176a1d1` | 2026-11-16 | 26 | `FINDINGS.md@6495c5c:157` |
| INC-38 | spawn_food_system vs detect_food_collisions_system Commands flush ambiguity missed by EU/SimRng gates (F-041) | `nondeterminism` | `1d301c3` | `3d34c69` | 2026-11-16 | 26 | `FINDINGS.md@6495c5c:139` |
| INC-39 | live.food_cap overwritten to 50 every tick by apply_environmental_effects_system (F-043) | `silent-semantic-change` | — | `544187d` | 2026-11-17 | — | `FINDINGS.md@6495c5c:116` |
| INC-40 | starve_ticks and dehydration_ticks shared one starving_ticks counter (F-044) | `silent-semantic-change` | `a1c3885` | `f3e9e47` | 2026-11-17 | 84 | `FINDINGS.md@6495c5c:103` |
| INC-41 | Checkpoint restore re-indexed post-rebuild offspring, merged AgentSex tables, and reset NextNodeId(3) (F-045) | `nondeterminism` | `1d301c3` | `21644fb` | 2026-11-18 | 28 | `FINDINGS.md@6495c5c:86` |
| INC-42 | pending_inference keyed solely by non-unique AgentLineageId diverged on sibling restore (F-047) | `nondeterminism` | `38fd867` | `5ee99b9` | 2026-11-19 | 112 | `FINDINGS.md@6495c5c:71` |
| INC-43 | prey_cap checked once before reproduction loop, overshooting cap on tick 1 (F-052) | `silent-semantic-change` | `15fc1f4` | `aff9f65` | 2026-11-19 | 2 | `FINDINGS.md@6495c5c:55` |
| INC-44 | live.mean_lifespan always 0.0 in final_observables after time-series sample drained VitalStatistics window (F-053) | `vacuous-gate` | `d8b39e5` | — | 2026-11-20 | — | `FINDINGS.md@6495c5c:43` |
| INC-45 | check_bundle_budget.mjs identified 3D chunk only by filename prefix react-three-fiber (F-054) | `vacuous-gate` | `766609e` | `427a0e6` | 2026-11-20 | 118 | `FINDINGS.md@6495c5c:33` |
| INC-46 | ScheduleGraph Commands-backed SimRng draws invisible to SIMRNG_SYSTEMS gate (F-030) | `vacuous-gate` | `1d301c3` | `03b331f` | 2026-10-31 | 10 | `FINDINGS.md@6495c5c:371` |
| INC-47 | E2 long-horizon energy metrics saturated to 0.0 floor because adapter lacked starvation death or replacement | `other` | `bb8248e` | `7530ccd` | 2026-07-27 | 0 | `artifacts/experiments/e2-evolved-brain-default/superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md@6495c5c:183` |
| INC-48 | PathogenTransmission debited animal EU without removing from agents, creating closed EU on every trigger (G5) | `silent-semantic-change` | — | `a5b89f3` | 2026-10-16 | — | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md@6495c5c:253` |
| INC-49 | thread_tracking_allocator_tests called Barrier::wait() inside window; Windows CI green while macOS failed 3 vs 1 (F-031) | `other` | `c6e8f42` | `c8afb3f` | 2026-08-24 | 27 | `FINDINGS.md@6495c5c:343` |
| INC-50 | anima-core failed to compile after merge 157f85d while main crate tests ran and 70% mutation score stayed stale (F-015) | `other` | `157f85d` | `1ec6ac5` | 2026-08-24 | 0 | `FINDINGS.md@6495c5c:771` |
| INC-51 | Full suite green and CPG oscillated while no system wrote target_velocity, leaving creatures unable to locomote (F-042) | `silent-semantic-change` | `af58e74` | `0dac4b8` | 2026-11-16 | 154 | `FINDINGS.md@6495c5c:127` |
| INC-52 | compact() filtered original lineage relations instead of plan.edges, silently turning pruned descendants into roots | `silent-semantic-change` | `d006f64` | `57a8246` | 2026-07-27 | 0 | `docs/planning/STATE_OF_THE_PROJECT.md@6495c5c:942` |
| INC-53 | challenger_meta_ai_tests ran 3 #[test] in one binary with a background thread racing process-wide ALLOCATOR | `nondeterminism` | `81871de` | `c6e8f42` | 2026-07-29 | 47 | `docs/planning/STATE_OF_THE_PROJECT.md@6495c5c:1647` |

## Provenance Notes

1. **`RESULT.md` §7 relocation (`INC-01`, `INC-02`, `INC-03`, `INC-47`):** On 2026-11-16 (PR #59), `artifacts/experiments/e2-evolved-brain-default/RESULT.md` was updated to Amendment 1 (§1–§6, 96 lines). The original 2026-07-27 report containing §7 (*"Defects found by this experiment, and what happened to them"*, lines 169–186) is preserved unedited at [`artifacts/experiments/e2-evolved-brain-default/superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md`](../../../artifacts/experiments/e2-evolved-brain-default/superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md).
2. **`INC-24`, `INC-27`–`INC-31`, `INC-48` pre-commit documentation (`2026-10-16` vs. `2026-10-22`):** These clusters were documented on `2026-10-16` in `COMPLETION_ROADMAP_2026-10-16.md` (`26e4104`) from the uncommitted `2026-08-16` WIP tree before being committed to `main` on `2026-10-22` (`a43872c`, `7d09779`, `4e618b9`, `cf1a631`, `cfc6ef5`, `b576309`, `add568e`). Because those `2026-10-22` commits are not the introduction timestamps, `introduced_commit` and `days_to_fix` are left blank (`""`), `detected_date` is `2026-10-16`, and `fixed_commit` records each trunk remediation commit.
3. **Off-trunk commits excluded (`389de4b`, `82a7a604`):** `389de4b` exists on `origin/feat/oss-071b-live-tracker` / `origin/wip/2026-08-16-map2-tier5`, not `origin/feat/ecological-selection`, so `INC-21` and `INC-22` record `detected_date = 2026-10-16` and trunk `fixed_commit = ef9bc3a` with `introduced_commit = ""`. `82a7a604` exists only on `origin/dell/m2-gate-controls`, so `INC-45` (`F-054`, `detected_date = 2026-11-20`) cites trunk fix `427a0e6` (`2026-11-21T21:06:29+07:00`).
4. **`INC-39` (`F-043`) attribution:** `FINDINGS.md@6495c5c:116` names no introducing commit: the hardcoded literal `50` in `world_systems.rs` dates back to `81871de` (`2026-06-12`), whereas the `live.food_cap` manifest key first appeared in `bb8248e` (`2026-07-27`). `introduced_commit` and `days_to_fix` are therefore left empty rather than attributing either commit alone; `fixed_commit` is `544187d` (`detected_date = 2026-11-17`).
5. **`INC-20`, `INC-44`, `INC-49`, and `INC-50`:** `INC-20` (`F-028`) was logged `2026-10-31` and fixed on trunk via `843ee74` (`2026-11-14T20:51:29+07:00`). `INC-44` (`F-053`) was detected `2026-11-20` and remains unfixed on trunk at `6495c5c` (`fixed_commit = ""`). `INC-49` (`F-031`) was fixed on `feat/ecological-selection` in `c8afb3f` (`2026-08-24T23:50:42+07:00`) and later re-encountered on unmerged `origin/main` when logging `FINDINGS.md` on `2026-10-31`. `INC-50` (`F-015`) was observed `2026-08-24` and resolved `2026-08-25` in `1ec6ac5`.

## Excluded by Scope

The following six former rows have `was_suite_green = no` (a CI/local check failed rather than silently passing) and are excluded from [`incidents.csv`](incidents.csv) so every CSV row represents a green-suite/gate failure:

- **`INC-13` & `INC-14` (`4b6258d` conflict markers — one root event):** `4b6258d` pushed unresolved conflict markers into `scripts/check_test_targets.mjs` and `src-tauri/tests/common/allocator.rs`. CI caught the markers on `main` immediately (`~0` days, keeping `main` red until `1b3e7ef`, PR #33), while `scripts/check_test_targets.mjs` retained 3 marker blocks (`F-017`, `INC-14`, dated `2026-08-25` in `FINDINGS.md`) until `bef7768`. Neither left CI green (`was_suite_green = no`).
- **`INC-15` (`F-010` / `F-024`, `ecdf5ba` -> `b8d1fa7`):** `scripts/verify.sh:41` (`GUARD_DIRS`) aborted with exit `1` and empty output (`was_suite_green = no`).
- **`INC-25` (`a0c1b09` toolchain drift):** Unpinned `rustc` 1.98 lints and new advisories turned CI red on unchanged lockfiles (`was_suite_green = no`).
- **`INC-26` (`b6a3dd7` -> `b021e44`):** `WorkloadContext.ts` manual edit broke `ts-rs` parity while `main` CI was already red from `4b6258d` (`was_suite_green = no`).
- **`INC-32` (`1d301c3` -> `fa938c5`):** Snapshot restore left `SpatialHashGrid` unpopulated for 1 tick when `live_experiment_tests` exited `101` (`was_suite_green = no`).
- Also excluded by Card P1 scope: `F-012` (`770b5f3`, `exit 101`), open wall-clock flakes in `STATE_OF_THE_PROJECT.md:1661-1668`, performance smells (`F-022`), feature gaps (`F-018`–`F-021`), and equivalent mutants (`F-009`, `F-011`, `F-016`).
