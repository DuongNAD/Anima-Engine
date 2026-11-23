---
title: How to Add a New Intervention Kind
status: active
owner: maintainers
last_reviewed: 2026-11-23
review_cycle: monthly
---

# How to Add a New Intervention Kind

This guide describes how to add a new `InterventionKind` variant executed through `InterventionCommand` across both the headless live world (`LiveExperimentAdapter`) and the synthetic reference world (`ReferenceEcosystem`).

> **Exotic forcings live elsewhere:** `MU`-field interventions (`AddSource`, `RemoveSource`, `Pulse`) are defined in `ExoticInterventionKind` ([`src-tauri/src/core/exotic_energy/forcings.rs`](../../src-tauri/src/core/exotic_energy/forcings.rs)) and are rejected by `LiveExperimentAdapter`. Intervention contracts live in [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) *(proposed)* (`ER01`, `ER03`, `ER07`, `ER08`), [`ENERGY_LEDGER_CONTRACT.md`](../reference/ENERGY_LEDGER_CONTRACT.md) (§1–§6), and [`DETERMINISM_CONTRACT.md`](../reference/DETERMINISM_CONTRACT.md) (§2, §5).

---

## Step 1 — Add the Variant to `InterventionKind`

Open [`src-tauri/crates/anima-domain/src/intervention.rs`](../../src-tauri/crates/anima-domain/src/intervention.rs) (re-exported via `core::intervention`) and add the new variant to `InterventionKind`:

- Keep `anima-domain` pure with no Bevy dependencies, and preserve `Clone, Debug, Serialize, Deserialize, PartialEq`.
- Reuse the outer `InterventionCommand` fields (`id`, `cause_id`, `region: Region`, `start_tick`, `duration_ticks`, `intensity: f32`, `signed_negative: bool`, `curve: Curve`, `reversible: bool`) rather than duplicating timing or spatial fields inside the variant.

---

## Step 2 — Validate Manifest Inputs in `validate_intervention`

Open [`src-tauri/src/core/experiment/interventions.rs`](../../src-tauri/src/core/experiment/interventions.rs). `validate_intervention(cmd, run_ticks)` already checks:
- `cmd.intensity` is finite and `>= 0.0` (negative direction uses `signed_negative`);
- `Region::Rect` bounds are non-inverted and `Region::Radius` has finite center and `r > 0.0`;
- `start_tick + effective_duration()` does not overflow `u64` and the active window intersects `1..=run_ticks`.

If your variant introduces numeric payload parameters (like `center_x`, `radius`, `toxicity_level`, or `transmission_rate` in existing variants), add explicit `is_finite()` and range validation returning `ExperimentError`.

> **Manifest rule:** `ExperimentManifest::validate` rejects any `InterventionCommand` with `cause_id == CAUSE_OBSERVER` (reserved for runtime `ObserverPolicy::Inhabit` actions).

---

## Step 3 — Assign a Unique Canonical Fingerprint Tag

Open [`src-tauri/src/core/experiment/manifest.rs`](../../src-tauri/src/core/experiment/manifest.rs) and handle the variant in `canonicalize_intervention_kind`:

1. Pick an **unused** byte tag (`0x50..=0x57` are currently assigned, so the next available tag is **`0x58`**).
2. Serialize each payload field deterministically via canonical writers (`c.u32(...)`, `c.f32(...)`, `c.u64(...)`, length/presence tags) — never `Debug` formatting.
3. Add an instance to the `kinds` list in `intervention_kind_tags_are_distinct` inside [`src-tauri/src/core/experiment/tests.rs`](../../src-tauri/src/core/experiment/tests.rs) so distinctness is enforced by tests.

---

## Step 4 — Implement Execution in `apply_live_interventions_system`

Open [`src-tauri/src/core/live_experiment.rs`](../../src-tauri/src/core/live_experiment.rs) at `apply_live_interventions_system` (scheduled inside [`simulation_schedule::build_tick_schedule`](../../src-tauri/src/core/simulation_schedule.rs)):

1. **Rate forcing vs stock/entity forcing:**
   - **Rate forcings** (such as `RainfallDelta`, `TemperatureDelta`): recompute from neutral defaults (`1.0` / `0.0`) on **every tick** into `LiveForcings` so effects stop immediately when the window ends, and record `state.ledger.record(...)` only when `SimClock::fires(tick, ECOLOGY_PERIOD)`.
   - **Stock / entity forcings** (such as `Deforest`, `AddNutrient`, `RemovePredators`, `ToxicityPlume`, `PathogenTransmission`): guard inside `if !SimClock::fires(tick, ECOLOGY_PERIOD) { return; }` (`ECOLOGY_PERIOD = 60`).
2. **Guarantee determinism:**
   - Collect ECS query entities into a buffer and `sort_unstable_by_key(|e| (e.index(), e.generation()))` before iterating — never rely on Bevy archetype iteration order.
   - Grid updates (e.g., `fertilise_region`) must be order-independent across cells.
3. **Preserve Closed-EU via `EnergyLedger`:**
   - **Never** mutate `EcosystemBiomass::plants` or `EcosystemBiomass::animals` directly (they are display/census caches!).
   - Mutate authoritative pools (`ResourceField::r`, `HomeostaticState::energy`, `EcosystemBiomass::detritus`), measure actual realized deltas in `f64` (`f32 -> f64`), and route transfers through `EnergyLedger::transfer(..., EnergyEvent::Intervention)`, `energy_ledger::debit_reserve` + `settle_reserve_outflow_to_detritus`, or `ReclaimAndDespawnAgentCommand`, keeping residual error within `RESIDUAL_ABS_TOLERANCE_EU = 1e-3`.
4. **Record causal provenance:** emit `state.ledger.record(cmd.cause_id, None, tick, ...)` and increment `state.applied_interventions += 1`.

> **Truthfulness note on `GeneticInjection`:** on trunk today, `apply_live_interventions_system` records `live.genetic_injection` in `state.ledger` and increments `applied_interventions` without directly mutating living agents' genotypes (see rule `ER03` in `EVOLUTION_EXPERIMENT_CONTRACT.md`).

---

## Step 5 — Implement the Matching Branch in `ReferenceEcosystem::step`

Open [`src-tauri/src/core/scenario.rs`](../../src-tauri/src/core/scenario.rs) and add the corresponding match arm inside `ReferenceEcosystem::step` (running on `ECOLOGY_PERIOD` for commands where `cmd.affects(0, 0)` holds). Keep directional effects consistent with the live world and log to `CausalLedger` under `cmd.cause_id`.

---

## Step 6 — Run Verification Tests

Extend tests covering:
- Invalid input rejection (`core/experiment/tests.rs`);
- Canonical tag distinctness (`intervention_kind_tags_are_distinct`);
- Closed-EU conservation (`src-tauri/tests/ledger_bypass_tests.rs`);
- Reference scenario behavior (`core::scenario::tests`) and checkpoint forks (`core/experiment_runner/fork.rs`).

Run the verification suite from the repository root:

<!-- doc-test -->
```bash
cargo test --manifest-path src-tauri/Cargo.toml -p anima-domain
cargo test --manifest-path src-tauri/Cargo.toml --lib core::experiment::tests::defect_b
cargo test --manifest-path src-tauri/Cargo.toml --lib core::experiment::tests::intervention_kind_tags_are_distinct
cargo test --manifest-path src-tauri/Cargo.toml --lib core::scenario::tests
cargo test --manifest-path src-tauri/Cargo.toml --test ledger_bypass_tests
```

Summarized test results — *not raw verbatim stdout* — on `macOS arm64` (`2026-11-22`, commit `03b9d3d`; authoritative repo totals live in [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền)):

```text
[Summary across the 5 commands — not verbatim stdout]
anima-domain: 23 passed, 0 failed, 0 ignored
core::experiment::tests::defect_b_manifest_rejects_invalid_intervention_values: 1 passed, 0 failed
core::experiment::tests::intervention_kind_tags_are_distinct: 1 passed, 0 failed
core::scenario::tests: 11 passed, 0 failed, 0 ignored
ledger_bypass_tests: 5 passed, 0 failed, 0 ignored
```

---

## Next Steps

- Declare `InterventionCommand` in JSON manifests: [WRITE_A_MANIFEST.md](WRITE_A_MANIFEST.md).
- Add a scalar observable: [ADD_AN_OBSERVABLE.md](ADD_AN_OBSERVABLE.md).
