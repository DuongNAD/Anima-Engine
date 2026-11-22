---
title: Incident catalogue — documented failures where a gate exited 0 while something was wrong
status: active
owner: maintainers
last_reviewed: 2026-11-23
review_cycle: per-release
---

# Incident Catalogue (Card P1)

Machine-verifiable data file: [`incidents.csv`](incidents.csv) (53 incidents, verified at trunk `origin/feat/ecological-selection` commit `6495c5c`).

## Scope and Sources

This dataset catalogues documented incidents in the repository where a test suite, gate, script, or build check exited `0` (or reported green/clean/valid) while a defect, falsification, blind agent sensor, stale/broken check, or nondeterminism was present. All rows trace to the five Card P1 source sets:

1. [`FINDINGS.md`](../../../FINDINGS.md)
2. [`COMPLETION_ROADMAP_2026-10-16.md` §1.2 and §4.1](../../planning/COMPLETION_ROADMAP_2026-10-16.md#12-sáu-lỗi-quy-trình-đã-dẫn-tới-đây--mỗi-lỗi-thành-một-gate-ở-ht04)
3. [`STATE_OF_THE_PROJECT.md` §1.e, §3.0, and §4](../../planning/STATE_OF_THE_PROJECT.md)
4. Original E2 report §7 preserved at [`superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md`](../../../artifacts/experiments/e2-evolved-brain-default/superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md) (superseding the Amendment 1 path [`RESULT.md`](../../../artifacts/experiments/e2-evolved-brain-default/RESULT.md))
5. Commit messages of `fa938c5` (#38) and `03b331f` (#39)

## Column and Verification Contract

Verification script: [`scripts/paper/incident_dates.mjs`](../../../scripts/paper/incident_dates.mjs) (covered by [`tests/frontend/incidentDates.test.ts`](../../../tests/frontend/incidentDates.test.ts)).

- `id`: Unique row identifier (`INC-01` through `INC-53`).
- `title`: Concise factual description of the failure.
- `class`: One of `vacuous-gate`, `fabricated-evidence`, `loosened-test`, `narrowed-command`, `silent-semantic-change`, `nondeterminism`, `contract-overwrite`, `toolchain-drift`, `other`.
- `introduced_commit` / `detected_commit`: Abbreviated git commit hash on `origin/feat/ecological-selection` (`git merge-base --is-ancestor` verified). At least one commit hash is non-empty on every row; `""` denotes an uncommitted or external origin without a trunk ancestor commit (`INC-21`, `INC-22`, `INC-25`). Depending on the incident, `detected_commit` is either the remediation commit or the finding-log commit that first recorded the detection in git (`INC-20` `3ad1107`, `INC-44` `9d86c65`, `INC-45` `177d851`).
- `detected_how`: Mechanism or check that surfaced the failure.
- `introduced_date` / `detected_date`: Exact committer timestamp from `git show -s --format=%cI <hash>` (empty `""` when the corresponding commit column is empty).
- `days_to_detection`: Non-negative integer elapsed 24-hour days computed as `Math.round((Date.parse(detected_date) - Date.parse(introduced_date)) / 86_400_000)` (empty `""` when either commit is empty).
- `was_suite_green`: `yes`, `no`, or `unknown`, recording whether the relevant suite/gate reported green while the defect was present.
- `source`: Exact `path:line` anchor relative to repository root, verified on disk with `1 <= line <= fileLines.length`.

Re-run verification:

```bash
node scripts/paper/incident_dates.mjs --check
node scripts/paper/incident_dates.mjs --self-test
```

## Summary Table

| ID | Title | Class | Introduced | Detected | Days | Suite Green | Source |
|---|---|---|---|---|---:|---|---|
| INC-01 | ScheduleGraph::systems empty after initialization made ambiguity gate vacuous | `vacuous-gate` | `993a587` | `ec94933` | 0 | `yes` | `artifacts/experiments/e2-evolved-brain-default/superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md:178` |
| INC-02 | ecosystem_census_system unordered against EU-moving systems (0.186 EU per-process drift) | `nondeterminism` | `6a42014` | `993a587` | 24 | `yes` | `artifacts/experiments/e2-evolved-brain-default/superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md:177` |
| INC-03 | Lifecycle readiness test slept blind 500 ms with 120-220 ms real headroom | `other` | `af58e74` | `5f0383f` | 41 | `yes` | `artifacts/experiments/e2-evolved-brain-default/superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md:180` |
| INC-04 | DEFAULT_GRID_DIM stayed 128 across 9 contract, generator, and test sites while world ran 256 | `contract-overwrite` | `73237fb` | `250e03d` | 2 | `yes` | `docs/planning/STATE_OF_THE_PROJECT.md:976` |
| INC-05 | Five wrong camelCase/snake_case IPC keys across 4 commands frozen by 6 mock tests | `vacuous-gate` | `af58e74` | `19e67fa` | 42 | `yes` | `docs/planning/STATE_OF_THE_PROJECT.md:390` |
| INC-06 | tests/ package never typechecked (86 hidden TypeScript errors and type-escape assertions) | `vacuous-gate` | `81871de` | `fb03692` | 45 | `yes` | `docs/planning/STATE_OF_THE_PROJECT.md:1587` |
| INC-07 | E2E global-setup.ts lacked Tauri mock transport and console watcher (8 TypeErrors ignored on exit 0) | `vacuous-gate` | `2752112` | `1cddeb5` | 2 | `yes` | `docs/planning/STATE_OF_THE_PROJECT.md:411` |
| INC-08 | capture.config.ts polled 127.0.0.1 while Vite bound ::1; capture:views dead while CI hashed committed PNGs | `vacuous-gate` | `fd3dc79` | `0f5b4d3` | 1 | `yes` | `docs/planning/STATE_OF_THE_PROJECT.md:512` |
| INC-09 | tests/vitest.config.ts hardcoded maxWorkers: 4 causing timeout failures on 4-vCPU CI runners | `other` | `3fb691d` | `72a6e34` | 1 | `yes` | `docs/planning/STATE_OF_THE_PROJECT.md:503` |
| INC-10 | check_test_targets.mjs required Running <path> ( on one line, silently skipping wrapped PowerShell targets | `vacuous-gate` | `2752112` | `4b6258d` | 14 | `yes` | `docs/planning/STATE_OF_THE_PROJECT.md:1606` |
| INC-11 | observer_trace_zero_alloc_tests used process-wide allocator counter despite single #[test], catching libtest main thread | `other` | `101d327` | `4b6258d` | 13 | `yes` | `docs/planning/STATE_OF_THE_PROJECT.md:1613` |
| INC-12 | eslint.config.js scanned .worktrees/** (4656 false local errors while clean CI stayed green) | `other` | `b2243cf` | `4b6258d` | 39 | `yes` | `docs/planning/STATE_OF_THE_PROJECT.md:1600` |
| INC-13 | Commit 4b6258d pushed conflict markers to main (local push exited 0; main CI red 69d git / 2.5 months doc) | `other` | `4b6258d` | `1b3e7ef` | 69 | `no` | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md:105` |
| INC-14 | scripts/check_test_targets.mjs retained 3 conflict-marker blocks on trunk (F-017, disabling target gate and ESLint) | `vacuous-gate` | `4b6258d` | `bef7768` | 84 | `no` | `FINDINGS.md:719` |
| INC-15 | scripts/verify.sh:41 (GUARD_DIRS) aborted exit 1 with zero output on clean or non-test diffs (F-010/F-024) | `other` | `ecdf5ba` | `b8d1fa7` | 98 | `no` | `FINDINGS.md:602` |
| INC-16 | justfile:151 passed nonexistent --count flag to verify_dataflow.py, silently printing fallback text (F-026) | `vacuous-gate` | `36401ee` | `b8d1fa7` | 98 | `yes` | `FINDINGS.md:493` |
| INC-17 | MapElitesArchive::select_parent iterated HashMap, making evolution non-reproducible across processes (F-001) | `nondeterminism` | `81871de` | `5f792ed` | 60 | `yes` | `FINDINGS.md:960` |
| INC-18 | is_valid_genotype had zero negative-control tests (return true mutant survived, F-008) | `vacuous-gate` | `81871de` | `f54f337` | 60 | `yes` | `FINDINGS.md:1091` |
| INC-19 | adversarial_coverage_tests used process-wide allocator counter racing sibling test threads (F-023) | `nondeterminism` | `81871de` | `445c9c8` | 75 | `yes` | `FINDINGS.md:580` |
| INC-20 | src-tauri/icons/ held 12-byte placeholder text files; Windows CI compiled while macOS/Linux panicked (F-028) | `other` | `81871de` | `3ad1107` | 141 | `yes` | `FINDINGS.md:452` |
| INC-21 | Audit recommendation SIM-01 blinded every agent sensor; raycast fixture moved x=99->90 to keep suite green | `loosened-test` | — | `ef9bc3a` | — | `yes` | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md:107` |
| INC-22 | Narrow commit title fix(sensors) altered 5 simulation semantics without re-measuring published E2 evidence | `silent-semantic-change` | — | `ef9bc3a` | — | `yes` | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md:114` |
| INC-23 | 970d963 appended live.dormant_eu to frozen E2 manifests while cargo test passed, breaking check:e2 until Amendment 1 | `contract-overwrite` | `970d963` | `30e70b7` | 83 | `yes` | `docs/planning/STATE_OF_THE_PROJECT.md:262` |
| INC-24 | PROJECT.md overwritten twice by agent spec/audit docs, erasing IPC contract pointer | `contract-overwrite` | `a43872c` | `491ec61` | 23 | `yes` | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md:112` |
| INC-25 | Unpinned CI rustc (1.98 new chunks_exact_to_as_chunks lint) and new rustls/npm advisories broke unchanged lockfiles | `toolchain-drift` | — | `a0c1b09` | — | `no` | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md:116` |
| INC-26 | WorkloadContext.ts hand-edited at b6a3dd7 (latent ts-rs binding parity failure masked while main CI was red) | `contract-overwrite` | `b6a3dd7` | `b021e44` | 79 | `no` | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md:121` |
| INC-27 | Map 2 verify_map2_evidence.ts wrote 1x1 PNG + HTML comment per view for distinct hashes and faked TEST_READY.md 100% pass | `fabricated-evidence` | `7d09779` | `d703c01` | 26 | `yes` | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md:123` |
| INC-28 | Telemetry and export hardcoded seed 42, 0.70/0.30 trophic split, 10000.0 biomass, and fallback floors (G3) | `fabricated-evidence` | `4e618b9` | `133a845` | 27 | `yes` | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md:124` |
| INC-29 | Research Dashboard used Math.sin curves, fake checksums, and mock export fallback for nonexistent IPC commands (G4) | `fabricated-evidence` | `cf1a631` | `0dd4d42` | 26 | `yes` | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md:125` |
| INC-30 | Exact count assertions weakened to >=, constant compared to itself, and 4 anima-lab tests deleted (G7) | `loosened-test` | `cfc6ef5` | `95fc481` | 25 | `yes` | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md:125` |
| INC-31 | tests/package.json narrowed test:e2e to e2e/tiers/, silently skipping 18 CI specs (G2) | `narrowed-command` | `b576309` | `b507bb4` | 25 | `yes` | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md:126` |
| INC-32 | Snapshot restore left SpatialHashGrid empty at tick K+1 until rebuild, blinding sensors 1 tick after 1d301c3 (#38) | `silent-semantic-change` | `1d301c3` | `fa938c5` | 10 | `yes` | `docs/planning/STATE_OF_THE_PROJECT.md:330` |
| INC-33 | 1d301c3 added 5th ResMut<SimRng> system unordered while schedule_ambiguity_tests checked EU systems only (F-029, #39) | `nondeterminism` | `1d301c3` | `03b331f` | 10 | `yes` | `FINDINGS.md:401` |
| INC-34 | autonomous_reproduction_system bypassed VitalStatistics, reporting live.births = 0 while agents bred (F-039) | `silent-semantic-change` | `1d301c3` | `b0a8c4d` | 26 | `yes` | `FINDINGS.md:166` |
| INC-35 | agent_mortality_system and mortality_system both scheduled: starve_ticks bypassed and live.deaths = 0 (F-037) | `silent-semantic-change` | `1d301c3` | `27e85bb` | 26 | `yes` | `FINDINGS.md:187` |
| INC-36 | crossover_genotypes exceeded MAX_MIGRATION_MORPHOLOGY_NODES (128), breaking restore and migration (F-038) | `other` | `5de4451` | `a47d534` | 110 | `yes` | `FINDINGS.md:176` |
| INC-37 | resolve_joints_system depended on ECS table order after deaths, diverging post-restore (F-040) | `nondeterminism` | `1d301c3` | `176a1d1` | 26 | `yes` | `FINDINGS.md:157` |
| INC-38 | spawn_food_system vs detect_food_collisions_system Commands flush ambiguity missed by EU/SimRng gates (F-041) | `nondeterminism` | `1d301c3` | `3d34c69` | 26 | `yes` | `FINDINGS.md:139` |
| INC-39 | live.food_cap overwritten to 50 every tick by apply_environmental_effects_system (F-043) | `silent-semantic-change` | `af58e74` | `544187d` | 154 | `yes` | `FINDINGS.md:116` |
| INC-40 | starve_ticks and dehydration_ticks shared one starving_ticks counter (F-044) | `silent-semantic-change` | `a1c3885` | `f3e9e47` | 84 | `yes` | `FINDINGS.md:103` |
| INC-41 | Checkpoint restore re-indexed post-rebuild offspring, merged AgentSex tables, and reset NextNodeId(3) (F-045) | `nondeterminism` | `1d301c3` | `21644fb` | 28 | `yes` | `FINDINGS.md:86` |
| INC-42 | pending_inference keyed solely by non-unique AgentLineageId diverged on sibling restore (F-047) | `nondeterminism` | `38fd867` | `5ee99b9` | 112 | `yes` | `FINDINGS.md:71` |
| INC-43 | prey_cap checked once before reproduction loop, overshooting cap on tick 1 (F-052) | `silent-semantic-change` | `15fc1f4` | `aff9f65` | 2 | `yes` | `FINDINGS.md:55` |
| INC-44 | live.mean_lifespan always 0.0 in final_observables after time-series sample drained VitalStatistics window (F-053) | `vacuous-gate` | `d8b39e5` | `9d86c65` | 87 | `yes` | `FINDINGS.md:43` |
| INC-45 | check_bundle_budget.mjs identified 3D chunk only by filename prefix react-three-fiber (F-054) | `vacuous-gate` | `766609e` | `177d851` | 116 | `yes` | `FINDINGS.md:33` |
| INC-46 | ScheduleGraph Commands-backed SimRng draws invisible to SIMRNG_SYSTEMS gate (F-030) | `vacuous-gate` | `1d301c3` | `03b331f` | 10 | `yes` | `FINDINGS.md:371` |
| INC-47 | E2 long-horizon energy metrics saturated to 0.0 floor because adapter lacked starvation death or replacement | `other` | `bb8248e` | `9c57184` | 0 | `yes` | `artifacts/experiments/e2-evolved-brain-default/superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md:183` |
| INC-48 | PathogenTransmission debited animal EU without removing from agents, creating closed EU on every trigger (G5) | `silent-semantic-change` | `add568e` | `a5b89f3` | 26 | `yes` | `docs/planning/COMPLETION_ROADMAP_2026-10-16.md:253` |
| INC-49 | thread_tracking_allocator_tests called Barrier::wait() inside window; Windows CI green while macOS failed 3 vs 1 (F-031) | `other` | `c6e8f42` | `c8afb3f` | 27 | `yes` | `FINDINGS.md:343` |
| INC-50 | anima-core failed to compile after merge 157f85d while main crate tests ran and 70% mutation score stayed stale (F-015) | `other` | `157f85d` | `1ec6ac5` | 0 | `yes` | `FINDINGS.md:771` |
| INC-51 | Full suite green and CPG oscillated while no system wrote target_velocity, leaving creatures unable to locomote (F-042) | `silent-semantic-change` | `af58e74` | `0dac4b8` | 154 | `yes` | `FINDINGS.md:127` |
| INC-52 | compact() filtered original lineage relations instead of plan.edges, silently turning pruned descendants into roots | `silent-semantic-change` | `d006f64` | `57a8246` | 0 | `yes` | `docs/planning/STATE_OF_THE_PROJECT.md:942` |
| INC-53 | challenger_meta_ai_tests ran 3 #[test] in one binary with a background thread racing process-wide ALLOCATOR | `nondeterminism` | `81871de` | `c6e8f42` | 47 | `yes` | `docs/planning/STATE_OF_THE_PROJECT.md:1647` |

## Provenance Notes and Doc-vs-Git Intervals

1. **`RESULT.md` §7 relocation (`INC-01`, `INC-02`, `INC-03`, `INC-47`):** On 2026-11-16 (PR #59), `artifacts/experiments/e2-evolved-brain-default/RESULT.md` was updated to Amendment 1 (§1–§6, 96 lines). The original 2026-07-27 report containing §7 (*"Defects found by this experiment, and what happened to them"*, lines 169–186) is preserved unedited at [`artifacts/experiments/e2-evolved-brain-default/superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md`](../../../artifacts/experiments/e2-evolved-brain-default/superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md). `incidents.csv` cites those preserved line numbers.
2. **Doc-stated vs. git `%cI` interval for `4b6258d` (`INC-13`, `INC-14`):**
   - `COMPLETION_ROADMAP_2026-10-16.md:105-106` states `main` was *"red for two and a half months"* (~75 days).
   - Git committer timestamps (`%cI`) yield `2026-08-08T12:49:25+07:00` (`4b6258d`) to `2026-10-16T20:42:03+07:00` (`1b3e7ef`, PR #33), which is **69 days** from `%cI` (79 days from `4b6258d` author timestamp `%aI = 2026-07-29T16:17:33+07:00`).
   - On `feat/ecological-selection`, the conflict markers in `scripts/check_test_targets.mjs` (`F-017`, `INC-14`) persisted until `bef7768` (`2026-10-31T13:29:22+07:00`), **84 days** from `%cI` (94 days from `%aI`).
3. **Author date (`%aI`) vs. committer date (`%cI`) on rebased commits (`4b6258d`, `c52b127`):**
   - `4b6258d` has `%aI = 2026-07-29T16:17:33+07:00` and `%cI = 2026-08-08T12:49:25+07:00`.
   - `c52b127` (`fix(contract): DEFAULT_GRID_DIM was 128 and nothing read it`) has `%aI = 2026-07-29T16:32:05+07:00` and `%cI = 2026-08-08T12:50:16+07:00`; `INC-04` uses `250e03d` (`%cI = 2026-07-27T03:04:48+07:00`), the first `DEFAULT_GRID_DIM` fix on `feat/ecological-selection`.
4. **Uncommitted `2026-08-16` WIP block vs. `2026-10-22` commits (`INC-24`, `INC-27`–`INC-31`, `INC-48`):** `COMPLETION_ROADMAP_2026-10-16.md:95,122-127,235-259` dates the uncommitted WIP working tree to `2026-08-16`; those clusters were committed to `main` on `2026-10-22` (`1d301c3`, `4e618b9`, `add568e`, `cf1a631`, `cfc6ef5`, `7d09779`, `b576309`, `a43872c`). `incidents.csv` uses the `2026-10-22` trunk commits for `%cI` reproducibility. Note also that `PROJECT.md` (`INC-24`) was overwritten once before `389de4b` and again in the `2026-08-16` WIP block / `29e02ba` / `a43872c` before restoration in `491ec61`.
5. **Off-trunk commits excluded from `incidents.csv` (`389de4b`, `82a7a604`):**
   - `389de4b` (`2026-08-15T10:05:57+07:00`) exists on `origin/feat/oss-071b-live-tracker` and `origin/wip/2026-08-16-map2-tier5`, not as an ancestor of `origin/feat/ecological-selection`. For `INC-21` (uncommitted SIM-01 sensor/fixture regression) and `INC-22` (narrow `fix(sensors)` commit title covering 5 semantic changes, recorded in `26e4104` on `2026-10-16`), `incidents.csv` leaves `introduced_commit` empty and cites trunk landing `ef9bc3a` (`2026-10-22T00:27:27+07:00`).
   - `82a7a604` (`2026-11-20T16:44:03+07:00`, referenced in `FINDINGS.md:39`) exists only on `origin/dell/m2-gate-controls`; `INC-45` (`F-054`) uses trunk commit `177d851` (`2026-11-20T17:23:27+07:00`).
6. **`OPEN` status drift in `FINDINGS.md` (`F-024`, `F-026`, `F-054`):**
   - `FINDINGS.md:512` (`F-026`), `:610` (`F-024`), and `:958` (`F-010`) still read `OPEN`, whereas trunk commit `b8d1fa7` (`2026-11-17T15:28:41+07:00`) fixed both `scripts/verify.sh:41` and `justfile:151`.
   - `FINDINGS.md:41` (`F-054`) still reads `OPEN`, whereas trunk commit `427a0e6` (`2026-11-21T21:06:29+07:00`) implemented content-based 3D chunk detection and static closure checking in `scripts/check_bundle_budget.mjs`.
7. **Additional context on `INC-20`, `INC-32`, and `INC-50`:**
   - `INC-20` (`F-028`): 12-byte `placeholder` icons existed from `81871de` (`2026-06-12`) and remained on `main` (`043930c`), while `fddbdee` (`2026-08-10`) accidentally committed real icons on `feat/ecological-selection` before `3ad1107` (`2026-10-31`) documented `F-028`.
   - `INC-32`: `bb8248e` (`2026-07-27`) originally omitted `SpatialHashGrid` population on snapshot restore, remaining invisible until `1d301c3` (`2026-10-22`) spawned tick-1 offspring; `incidents.csv` uses `1d301c3` -> `fa938c5` (10 days).
   - `INC-50` (`F-015`): after merge `157f85d`, main-crate `just test` continued running without compiling the standalone `anima-core` crate, leaving its `"70%"` mutation score stale until `1ec6ac5`.
8. **Excluded from `incidents.csv` by scope:**
   - `F-012` (`FINDINGS.md:893`, 11+ orphaned test suites after `157f85d`, resolved `770b5f3`): `just test` failed with exit `101` (suite was red, not exit `0`).
   - Open wall-clock timing flakes in `STATE_OF_THE_PROJECT.md:1661-1668` (the `sleep(16ms)` half of `challenger_meta_ai_tests` and `environmental_elements_stress_tests` 50 ms budget failing at 101.9 ms on PR #112): still open without a remediation commit.
   - Pure feature gaps (`F-018`–`F-021`), performance smells (`F-022`), and equivalent mutants (`F-009`, `F-011`, `F-016`) in `FINDINGS.md`.
