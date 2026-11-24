---
phase: planning
feature: v1-vertical-slice
title: Card V8 — Vertical-Slice Multi-Seed Acceptance Suite (WORLD_SIMULATION_PLAN §9)
description: Implementation spec for card V8 verifying all four vertical-slice interventions across multi-seed control/treatment ensembles and reporting all seven observations required by WORLD_SIMULATION_PLAN §9
status: proposed
owner: anima-dell
last_reviewed: 2026-11-24
---

# Card `V8` — Vertical-Slice Multi-Seed Acceptance (`WORLD_SIMULATION_PLAN` §9)

> **Design spec only (`status: proposed`).** No Rust, TypeScript, or docs-index edits are made in D8 (`V-SPECS-2`).
> All trunk `file:line` anchors below were checked against `origin/feat/ecological-selection` at
> `8d52341549007a50ce033a6d272c5dd22a71b1b1`. Anything not on trunk yet is labeled **`to be created by V8`**
> (or **`to be created by V5`** / **`V6`**).

## 1. Goal, Dependencies, `START WHEN` & Scope

Execute multi-seed paired control-vs-treatment acceptance scenarios covering all **4 interventions** and generating a machine-verifiable report covering all **7 required observations** of the `"lưu vực – đồng cỏ – thỏ – sói"` vertical slice in [`WORLD_SIMULATION_PLAN.md`](../../../WORLD_SIMULATION_PLAN.md) (`§9 :1029-1064`, `M8`/`M11.3`, `S44`, `S47`, `S48`, `S64`, `2026-11-05-swarm-master-plan.md` Đợt 6 `V8`):

| # | §9 Required Item (`WORLD_SIMULATION_PLAN.md:1048-1063`) | Type | Primary Verification Source |
|---|---|---|---|
| **I1** | 30 simulated days of drought (`RainfallDelta`, `:1050`) | Intervention | `V6` `drought_30d`, `CausalLedger` chain `precip -> soil_moisture -> grass_npp -> starvation` (`to be created by V6`) |
| **I2** | Remove 80% of wolves (`RemovePredators`, `:1051`) | Intervention | `V6` `cull_wolves_80pct`, single-shot deterministic cull + trophic cascade (`to be created by V6`) |
| **I3** | Clear 40% of headwaters forest (`Deforest`, `:1052`) | Intervention | `V6` `clear_headwaters_forest_40pct`, `canopy_cover` (`to be created by V5`) / `root_cover` -> runoff / erosion / turbidity (`to be created by V6`) |
| **I4** | Fertilize around lakes (`AddNutrient`, `:1053`) | Intervention | `V6` `fertilize_lake_ring`, `soil_nitrogen` -> NPP + `algal_bloom` / `dissolved_oxygen` (`to be created by V6`) |
| **O1** | Predator–prey cycles (`:1057`) | Observation | Paired/baseline time-series oscillation sign-changes & non-extinction telemetry |
| **O2** | Trophic cascade (`:1058`) | Observation | Wolf removal `treatment - control` paired deltas (`PairedEnsembleReport`: rabbit up, grass down) |
| **O3** | Water/soil/plant feedback (`:1059`) | Observation | Drought & forest-clear paired deltas (`soil_moisture`, `runoff`, `erosion`, `plants`) |
| **O4** | Simple eutrophication (`:1060`) | Observation | Lake fertilization paired deltas (`soil_nitrogen`, `algal_bloom`, `dissolved_oxygen`, `to be created by V6`) |
| **O5** | Causal timeline (`:1061`) | Observation | Non-empty `CausalLedger` per intervention with `trace_to_root` depth `>= 3` |
| **O6** | Control vs treatment (`:1062`) | Observation | Identical-seed `run_paired_ensemble_with_control` (`ensemble.rs:218`) with strict `FactorDiff` |
| **O7** | Save/replay deterministic (`:1063`) | Observation | Same-OS `run(N) == run(K) -> save -> load -> run(N - K)` + `world_record` / replay parity |

### Exact Dependency Order & `START WHEN`

- **Depends directly on:** **[Card `V6`](2026-11-24-v6-interventions-causeid-spec.md)** (and transitively `V0`–`V5` and accepted [`ADR-0013`](../../decisions/ADR-0013-live-creature-lifecycle.md) `(:3, 13)`, currently `status: proposed` on trunk pending owner acceptance).
- **`START WHEN`:** `V6` is merged on trunk, and owner decisions `D-V8-1`–`D-V8-4` are resolved.
- **Unblocks:** `V9` (parameter registry & sensitivity), `V10` (soak test), `V11` (example scenarios), and `H10` / `H11`.

### Explicit Non-Goals

- **No UI timeline/charts:** belongs to [Card `V7`](2026-11-24-v7-causal-timeline-ui-spec.md).
- **No multi-dimensional parameter sweep / sensitivity study:** belongs to `V9`.
- **No fabricated statistics or single-seed victory claims:** per `docs/ai/planning/2026-11-23-prereg-statistics-template.md:11-114` and `WORLD_SIMULATION_PLAN.md:1129`, all conclusions require a multi-seed paired ensemble reporting complete/incomplete pairs honestly (`ensemble.rs:135-188`).

## 2. Current Trunk Anchors (`8d523415`) & Acceptance Suite Design

### 2.1 Existing Runners & Statistics Verified on Trunk

- **Paired ensemble engine:** `src-tauri/src/core/experiment_runner/ensemble.rs:61-93` (`run_ensemble`), `:135-188` (`PairedEffect`, `PairedEnsembleReport` with `control_manifest_fingerprint`, `treatment_manifest_fingerprint`, `control_law_fingerprint`, `treatment_law_fingerprint`, `declared_factors`, `pairs: Vec<SeedPair>`, `complete_pairs()`, `incomplete_pairs()`), `:201-225` (`run_paired_ensemble`, `run_paired_ensemble_with_control` enforcing identical ordered seed list `control.seeds == treatment.seeds` at `:230`).
- **Statistical summaries:** `src-tauri/src/core/experiment_runner/statistics.rs:156-225` (`EffectSize`, `EnsembleComparison`, `compare_ensembles` — note lines `:207-214` warn not to confuse un-paired `compare_ensembles` with paired causal design).
- **Checkpoint/genesis forks:** `src-tauri/src/core/experiment_runner/fork.rs:10-99` (`ForkReport`, `genesis_fork`, `CheckpointForkReport`).
- **Headless live adapter & honesty contract:** `src-tauri/src/core/live_experiment.rs:25-43` (freezes shared `BrainModel`, pumps inference synchronously so trajectories stay reproducible; lines `:36-43` clarify numerical equivalence with `ReferenceEcosystem` is never claimed — only shared-law direction/meaning), `:280-320` (`LIVE_KEYS`, `LiveWorldConfig`).
- **World Record chain (`#236`):** `src-tauri/src/core/world_record/mod.rs:1-22` (provides opt-in zero-feedback end-of-tick digest chain for O7 verification without altering simulated trajectories).
- **Statistical preregistration checklist:** `docs/ai/planning/2026-11-23-prereg-statistics-template.md:11-114` (defines sample size `N`, exact paired/sign-test thresholds, missing-pair accounting).
- **Test-target completeness enforcement:** `scripts/check_test_targets.mjs:1-40` (blocks silent `#[ignore]` or skipped integration binaries).

### 2.2 Acceptance Runner & Suite (`to be created by V8`)

1. **Frozen scenario manifests (`src-tauri/tests/fixtures/experiments_v8/`, `to be created by V8`):**
   - `v8-control-watershed.json` + one paired treatment manifest per intervention (`v8-drought-30d.json`, `v8-remove-wolves-80pct.json`, `v8-deforest-headwaters-40pct.json`, `v8-fertilize-lakes.json`), all declaring identical ordered `seeds`, baseline `exotic_energy = null`, opt-in `V0–V6` live flags, and matched `observable_ids`.
2. **Acceptance binary / example (`src-tauri/examples/run_v8_slice_acceptance.rs`, `to be created by V8`):**
   - Supports `--smoke` (CI fast subset per `D-V8-1`) and `--full` (full confirmation seed set), invoking `run_paired_ensemble_with_control::<LiveExperimentAdapter>` (`ensemble.rs:218`) for all 4 intervention pairs plus a checkpoint save/load round-trip on the baseline & intervention trajectories.
   - Emits a schema-checked JSON report + Markdown scorecard mapping every row of **`I1–I4`** and **`O1–O7`** (`WORLD_SIMULATION_PLAN.md:1048-1063`) to concrete observed metrics, `n_complete_pairs / n_requested`, paired 95% CI, conservation residuals, causal trace depth, and `PASS` / `FAIL`.
3. **Integration tests (`src-tauri/tests/v8_slice_acceptance_tests.rs`, `to be created by V8`):**
   - Un-ignored headless tests running a deterministic smoke slice (`--features desktop` or default profile as decided in `D-V8-4`) and asserting all 7 §9 observations (`O1–O7`), FactorDiff validation, and conservation bounds pass without `#[ignore]`.

## 3. Applicable [`CLAUDE.md`](../../../CLAUDE.md) Traps & Invariants

- **World-law immutability & baseline compatibility (`CLAUDE.md:145-153, 173-185`):** `WorldLawSet` is immutable within every `V8` run (`exotic_energy = null` for the v1.0 baseline slice); control and treatment differ strictly in declared `InterventionCommand` entries validated via `FactorDiff`; all headless runs execute strictly via `build_tick_schedule` (`CLAUDE.md:183-185`).
- **Checksums never pinned across platforms/models (`CLAUDE.md:41, 176-182`, `docs/decisions/ADR-0008-world-past-and-time.md:175, 283, 304`, `src-tauri/tests/cross_platform_checksum_probe.rs:235`):** Never hardcode a magic `world_checksum` hex string across Windows vs macOS or across `LIVE_MODEL_VERSION` changes! Assert `O7` exclusively via **same-process / same-OS** equality: `run(N) == run(K) -> save -> load -> run(N - K)` and same-OS 3-process replay equality (`world_checksum` and `world_record` chain match within that OS/build).
- **Zero-heap-allocation hot loop & closed EU (`CLAUDE.md:50, 105-107`, `energy.rs:123`):** All 4 `V8` paired ensembles check closed EU residual (`<= RESIDUAL_ABS_TOLERANCE_EU` = `1e-3`, `src-tauri/crates/anima-domain/src/energy.rs:123`) plus `water_budget_residual` and `nutrient_budget_residual` `< 1e-4` (`dynamic_fields.rs:570, 762`) at sampled checkpoints; any energy charges stay routed through `total_cost`.
- **No Lamarck (`CLAUDE.md:97-98`):** `AgentBrain.learned` remains runtime-only across multi-generation predator–prey cycles (`O1`).
- **Do not overclaim desktop parity (`CLAUDE.md:178-182`, `live_experiment.rs:36-43`):** Report `V8` results explicitly as **headless `LiveExperimentAdapter` verified**.

## 4. Open Owner Decisions (`D-V8-1`–`D-V8-4`)

| Slot | Question | Options | Recommendation |
|---|---|---|---|
| **`D-V8-1`** | Full `V8` confirmation ensemble size `N` seeds vs CI smoke size? | **(A)** `N = 20` seeds for confirmation bundle (per `2026-11-23-prereg-statistics-template.md` power guidance), `N = 3` seeds in `--smoke` integration tests.<br>**(B)** `N = 12` confirmation seeds / `2` smoke seeds.<br>**(C)** `N = 30` confirmation seeds. | **(A)** `20` paired seeds for release evidence (`N = 3` in fast CI smoke), rejecting any run with incomplete pairs above threshold. |
| **`D-V8-2`** | Simulation tick horizon (`O1` baseline cycle window + pre/post-intervention duration): | **(A)** Warmup to cycle phase `t0` + `1,800` ticks (30 sim days at `60` ticks/day per `V6 D-V6-1`) + `1,800` recovery ticks (`3,600`–`5,400` total ticks).<br>**(B)** Fixed `18,000` ticks per seed (`~3` seasonal years). | **(A)** ~`3,600–5,400` ticks if oscillations (`O1`) & cascades (`O2`–`O4`) express cleanly; validate empirically during `V8` preflight before locking in frozen `v8` manifests. |
| **`D-V8-3`** | Acceptance criteria for each of `O1–O7`: | **(A)** Paired sign-direction + non-zero 95% CI / minimum effect threshold per observation, plus mandatory zero incomplete-pair silent drops and `trace_to_root` depth `>= 3` on every intervention.<br>**(B)** Raw direction-of-mean only. | **(A)** Paired direction + CI/minimum-pair bar (matching E3b rigor). |
| **`D-V8-4`** | Baseline world input for the vertical-slice watershed: | **(A)** Deterministic `TerrainMap::generate` watershed configured via `V1`–`V6` manifest.<br>**(B)** Offline `ANMW` v3 world generated by `anima-worldgen` (`WV2-3`). | **(A)** Start on deterministic `TerrainMap` if `WV2-3` is still settling; support optional `ANMW` world artifact if `WV2-3` has landed first. |

## 5. Future `V8` Card Boundaries (**Proposed** `ALLOWED PATHS`)

> Marked **`proposed`** for owner authorization when scheduling card `V8`.

- `src-tauri/examples/run_v8_slice_acceptance.rs` (`to be created by V8`)
- `src-tauri/tests/v8_slice_acceptance_tests.rs` (`to be created by V8`)
- `src-tauri/tests/fixtures/experiments_v8/` (frozen control + 4 intervention manifests, `to be created by V8`)
- `src-tauri/src/core/scenario.rs:1-20` (vertical-slice helper constructors only if extensions beyond `V6` are needed)
- `docs/ai/planning/` (frozen `V8` preregistration & acceptance results note; never modifying `docs/ai/README.md` unless requested)
- `artifacts/experiments/v8-vertical-slice/` (`--smoke` / confirmation report JSON/provenance, if committed under artifact policy)

## 6. `DONE WHEN`, Verification Commands & Negative Controls (`A3 #2`)

### `DONE WHEN`
1. All 4 intervention pairs (`I1`–`I4`) and all 7 §9 observations (`O1`–`O7`) emit concrete `PASS` evidence in the `V8` acceptance JSON + summary report without `#[ignore]`d acceptance tests or hand-picked seeds.
2. All 4 manifests pass `ExperimentManifest::validate` and strict `FactorDiff` checking (`run_paired_ensemble_with_control`).
3. Conservation residuals (`energy.rs:123` `<= 1e-3`, water/nutrient `dynamic_fields.rs:570, 762` `< 1e-4`), multi-hop `CausalLedger::trace_to_root` depth `>= 3`, and save/load replay identity (`run(N) == run(K) -> save -> load -> run(N - K)`) pass on every tested seed.
4. Concrete Verification Commands (after final edit, on host meeting `>= 5.00 GiB` RAM / CI):
   - `cargo fmt --check`
   - `cargo clippy --all-targets --features desktop -- -D warnings`
   - `cargo run --release --example run_v8_slice_acceptance -- --smoke` (when Cargo build budget holds)
   - `cmd /c "cargo test --features desktop --test v8_slice_acceptance_tests > ..\v8-test.txt 2>&1"` & `node scripts/check_test_targets.mjs cargo-test-output.txt --profile desktop` (on full `cargo test --features desktop` output)
   - `npm run check:docs`, `npm run check:text-hygiene`, `npm run check:root-hygiene`, `npm run check:conflict-markers`

### Negative Controls (Must Fail Red Under Fault Injection, `A3 #2`)
1. **Truncate seed parity:** Mutate `control.seeds` order vs `treatment.seeds` -> `run_paired_ensemble_with_control` (`ensemble.rs:230`) rejects with structured error before running a tick.
2. **Break `O7` replay persistence:** Omit one state field from `SavedSimulationState` -> checkpoint `run(N) == run(K) -> save -> load -> run(N - K)` assertion in `v8_slice_acceptance_tests.rs` fails red.
3. **Bypass `O5` causal chain:** Unlink `parent` in `CausalChainTracker` (`to be created by V6`) -> `O5` depth-`>= 3` acceptance gate fails red.
4. **Add `#[ignore]` to failing acceptance test:** `node scripts/check_test_targets.mjs` fails red against `test_target_policy.mjs`.

### Determinism Checks
- Re-running `run_v8_slice_acceptance --smoke` twice on the same machine produces identical per-seed `final_observables`, `CausalLedger` length/checksums, and identical `world_checksum` sequences (no timestamps embedded inside hashed state).
