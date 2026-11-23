---
title: How to Add a New Observable
status: active
owner: maintainers
last_reviewed: 2026-11-23
review_cycle: monthly
---

# How to Add a New Observable

This guide explains how to add a new scalar observable to the headless live world (`LiveExperimentAdapter`) or reference world (`ReferenceEvolutionWorld`) while preserving metadata contracts (`ER10`), closed-EU conservation, and frozen E2 manifests.

> **Related contracts (not redefined here):** [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) *(proposed)* (`ER07`, `ER10`, `ER11`), [`ENERGY_LEDGER_CONTRACT.md`](../reference/ENERGY_LEDGER_CONTRACT.md) (§1, §1.1, §5), [`DETERMINISM_CONTRACT.md`](../reference/DETERMINISM_CONTRACT.md), and the S2 observable implementation note in [`2026-11-19-s2-observables.md`](../ai/planning/2026-11-19-s2-observables.md).

---

## Before Starting: Frozen Boundaries and Unimplemented Features

1. **Never edit `LIVE_OBSERVABLE_IDS`** ([`src-tauri/src/core/live_experiment.rs`](../../src-tauri/src/core/live_experiment.rs)): those 12 IDs are pinned by frozen E2 manifests (`tests/e2_frozen_manifest_tests.rs`). Append new live observables to **`LIVE_EXTENDED_OBSERVABLE_IDS`** (currently 15 items).
2. **Never share an ID with `reference_default()`** unless it measures the exact same physical quantity, `unit`, `ConservationRole`, and `ObservableScope`. Currently only **2 IDs** are shared: `plants` and `detritus` (`tests/live_experiment_tests.rs`: `live_and_reference_agree_on_shared_ids` asserts `shared == 2`).
3. **Do not emit or assume unimplemented live features:**
   - `LiveExperimentAdapter` has **no** `exotic.*` (`MU`) fields and **no** AE3 `evolution.*` population (those belong to `ReferenceEvolutionWorld`).
   - `ObservableSpec` currently has **no** `causal_parents` field in Rust (`src-tauri/src/core/experiment/observables.rs`), even though target spec `ER10` mentions it.
   - `time-to-floor` is intentionally kept a **post-hoc** analysis metric rather than a per-tick emitted observable, because floor thresholds depend on experimental questions and non-collapsing runs are right-censored.
   - Never emit sentinel numbers such as `-1.0` or `NaN` when a cohort is absent or extinct — always emit an honest finite value (`0.0`), or enforce explicit preflight rejection such as `live.deaths_starvation` under `drift_control`.

---

## Step 1 — Register `ObservableSpec` in `ObservableRegistry`

Open [`src-tauri/src/core/experiment/observables.rs`](../../src-tauri/src/core/experiment/observables.rs) and add an `ObservableSpec` entry to `ObservableRegistry::live_default()` (or `reference_default()` for the reference model):

- Populate all 11 fields: `id`, `display_name`, `unit` (e.g., `EU`, `EU·tick`, `individuals`, `fraction`), `scope` (`ObservableScope::World`, etc.), `cadence_name`, `cadence_period > 0` (`PHYSICS_PERIOD = 1` or `ECOLOGY_PERIOD = 60`), `aggregation` (`Instant`, `Sum`, `Mean`, `Max`), `valid_min`, `valid_max`, `conservation` (`ClosedEu`, `ExoticMu`, `None`), and `source` (`"core::live_experiment::LiveExperimentAdapter"`).
- **Never use `f64::INFINITY` or `f64::NEG_INFINITY`:** `serde_json` serializes non-finite floats as `null`. Use `Self::OPEN_LOWER_BOUND` (`-1e300`) and `Self::OPEN_UPPER_BOUND` (`1e300`).
- `ObservableRegistry::validate()` asserts `OBSERVABLE_REGISTRY_VERSION == 2`, non-empty strings, `cadence_period > 0`, finite `valid_min <= valid_max`, and unique `id`s.

---

## Step 2 — Append the ID to `LIVE_EXTENDED_OBSERVABLE_IDS`

Open [`src-tauri/src/core/live_experiment.rs`](../../src-tauri/src/core/live_experiment.rs) and append the new ID to the **end** of `LIVE_EXTENDED_OBSERVABLE_IDS` (incrementing its array length). `all_live_observable_ids()` chains `LIVE_OBSERVABLE_IDS` followed by `LIVE_EXTENDED_OBSERVABLE_IDS` to preserve emission order.

---

## Step 3 — Compute and Emit the Value in `read_live_observables`

In `read_live_observables(world: &mut World, close_window: bool)` (`src-tauri/src/core/live_experiment.rs`):

1. **Instantaneous metrics:** read ECS resources/components read-only; never mutate `World` or draw from `SimRng`.
2. **Windowed counters:** `experiment_runner::drive` calls `observables_unsampled()` (`live_observables_peek`, `close_window = false`) on **every tick** to check finiteness and key uniqueness, and calls `observables()` (`live_observables`, `close_window = true`) only on sampling ticks. Only reset/drain windowed counters (`VitalStatistics::sample_window()`) when `close_window == true`; use non-destructive reads (`peek_window()`) when `close_window == false`.
3. **Multi-tick accumulators (S2 pattern like `live.mean_energy_integral`):** if summing/integrating across ticks, accumulate once per tick inside `LiveExperimentAdapter` (`step` and `run_schedule_once`), persist `#[serde(default)]` state on `LiveExperimentState` across disk checkpoints (`snapshot`/`from_snapshot`) without altering `SavedSimulationState` or `world_checksum`, and read passively inside `read_live_observables`.
4. **Empty population & `drift_control`:** return a finite `f64` (e.g., `0.0`, as `mass_statistics(&[])[0]`), and explicitly specify whether the metric is supported or rejected under `drift_control`.
5. **Exact order and naming:** every ID in `all_live_observable_ids()` must appear once with an exact key match to `ObservableSpec.id`, otherwise `specs_for_emitted` warns and registry tests fail.

---

## Step 4 — Reject Invalid Manifests Early in `ensure_observables_supported`

If the observable depends on an opt-in world law (`live.mortality`, `live.reproduction`, etc.), add a preflight guard in `LiveWorldConfig::ensure_observables_supported` (`src-tauri/src/core/live_experiment.rs`) returning `ExperimentError::InvalidObservableRequest` instead of emitting misleading zeros when the law is disabled.

---

## Step 5 — Add and Run Verification Tests

Extend [`src-tauri/tests/observables_tests.rs`](../../src-tauri/tests/observables_tests.rs) (registry completeness, emission ordering, preflight rejection) and/or [`src-tauri/tests/s2_observables_tests.rs`](../../src-tauri/tests/s2_observables_tests.rs) (non-interference between unsampled peek and windowed reads, checkpoint resume, empty populations). If creating a new `tests/*.rs` target, ensure it contains non-ignored tests to comply with [`scripts/test_target_policy.mjs`](../../scripts/test_target_policy.mjs).

Run all six verification targets from the repository root:

<!-- doc-test -->
```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib core::experiment::tests::observable
cargo test --manifest-path src-tauri/Cargo.toml --lib core::experiment::tests::defect_a
cargo test --manifest-path src-tauri/Cargo.toml --test observables_tests
cargo test --manifest-path src-tauri/Cargo.toml --test s2_observables_tests
cargo test --manifest-path src-tauri/Cargo.toml --test live_windowed_observable_sampling_tests
cargo test --manifest-path src-tauri/Cargo.toml --test e2_frozen_manifest_tests
```

Observed results on `macOS arm64` (`2026-11-22`, commit `03b9d3d`; first two blocks are trimmed `cargo test` stdout, followed by summarized pass counts — *not raw verbatim stdout* — for the 4 integration targets; authoritative repo totals live in [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền)):

```text
running 2 tests
test core::experiment::tests::observable_registry_rejects_duplicate_ids ... ok
test core::experiment::tests::observable_registry_validates_and_is_unique ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 559 filtered out; finished in 0.00s

running 2 tests
test core::experiment::tests::defect_a_registry_validation_rejects_malformed_specs ... ok
test core::experiment::tests::defect_a_reference_registry_is_json_safe_and_finite ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 559 filtered out; finished in 0.00s

[Summary of 4 integration targets — not verbatim stdout]
observables_tests: 7 passed, 0 failed, 0 ignored
s2_observables_tests: 5 passed, 0 failed, 0 ignored
live_windowed_observable_sampling_tests: 2 passed, 0 failed, 0 ignored
e2_frozen_manifest_tests: 2 passed, 0 failed, 0 ignored
```

---

## Next Steps

- Reference a new observable in manifests: [WRITE_A_MANIFEST.md](WRITE_A_MANIFEST.md).
- Add a new causal intervention: [ADD_AN_INTERVENTION.md](ADD_AN_INTERVENTION.md).
