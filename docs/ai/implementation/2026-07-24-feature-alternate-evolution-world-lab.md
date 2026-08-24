---
phase: implementation
feature: alternate-evolution-world-lab
title: Implementation Notes — Alternate Evolution & World Lab
description: Implemented AE1–AE2.5 headless entry points, contracts, verification and deferred boundaries
status: active
owner: simulation-architecture
last_reviewed: 2026-07-25
plan: ../planning/2026-07-24-feature-alternate-evolution-world-lab.md
---

# Implementation Notes — Alternate Evolution & World Lab

> ## 📜 Historical package record — not current status
>
> This document belongs to **one dated work package**. Every count in it (test totals, warning
> counts, target counts, coverage numbers) is a **historical measurement**: true when the command
> ran during that package, and not a description of the tree today.
>
> **Current measured status lives in exactly one place:**
> [`docs/planning/STATE_OF_THE_PROJECT.md` §1](../../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).


**Status update (2026-07-25):** AE1–AE2.5 and the opt-in AE3 pathway/selection slice are implemented
for the **headless reference model**. AE4–AE7, live Bevy integration, persistence, species
diagnostics, UI parity, and map review remain open. Current task and validation status live in
[planning](../planning/2026-07-24-feature-alternate-evolution-world-lab.md) and
[testing](../testing/2026-07-24-feature-alternate-evolution-world-lab.md).

## Required reading

1. [Evolution Experiment Contract](../../reference/EVOLUTION_EXPERIMENT_CONTRACT.md)
2. [ADR-0002](../../decisions/ADR-0002-world-laws-and-exotic-energy.md)
3. [Feature design](../design/2026-07-24-feature-alternate-evolution-world-lab.md)
4. [Creature Development Contract](../../reference/CREATURE_DEVELOPMENT_CONTRACT.md) nếu chạm
   genotype/phenotype/spawn/save/migration.

## Current code anchors

| Concern | Symbol/file |
|---|---|
| Scenario/model/runner | `core/scenario.rs::{SimModel, Scenario, run_scenario, control_treatment}` |
| Intervention | `core/intervention.rs::{InterventionKind, InterventionCommand}` |
| Causality | `core/causal.rs::{EffectRecord, CausalLedger}` |
| Headless pathway/selection | `core/evolution_pathway.rs::{EnergyPathwayGenotype, PathwayCohort, ReferencePopulation}` |
| Clock | `core/sim_clock.rs::SimClock` |
| Units/conservation | `core/sim_rules.rs`, `SIMULATION_RULES.md` |
| EU/resource | `core/ecology.rs::{ResourceField, EcosystemBiomass}` |
| World identity | `core/world_artifact.rs::WorldIdentity` |
| Live world init | `core/ecs.rs::init_world` |
| Genotype/spawn | `evolution/genotype.rs::decode_genotype` |
| Persistence | `core/simulation_state.rs` |
| Migration | `core/components.rs`, `core/world_systems.rs` |
| Existing dashboard | `src/components/EcosystemPanel.tsx` |

## Hard constraints

- Bắt đầu AE1 schema/runner; không bắt đầu bằng UI hoặc chèn Mana vào genotype.
- `exotic_energy=None` là fast path/rollback và phải giữ AE-S01.
- MU không phải EU; không sửa `SIMULATION_RULES.md` mà không sửa machine contract/tests.
- Không đọc environment ngầm trong `decode_genotype`.
- Không dùng `thread_rng()` trên deterministic paths.
- Field buffers preallocate; hot loop allocation = 0.
- Renderer/UI đọc observable payload, không sample biome/field để tự suy phenotype.
- Không gọi result là species/adaptation trước gate.

## AE1–AE2 headless implementation (2026-07-25)

Implemented as four new, self-contained `core/` modules (no edits to `scenario.rs`, `ecology.rs`,
`sim_rules.rs`, genotype, save/migration, live Bevy, or any UI/map code). The only edit to an
existing file was registering the four modules in `core/mod.rs` (append-only, preserving the
pre-existing M0–M3 module list).

### Files added

| File | Milestone | Public symbols (anchors) |
|---|---|---|
| `src-tauri/src/core/exotic_energy.rs` | AE2 (AE-201/202/203/204/205) | `EnergySourceId`, `UnitId`, `EU_UNIT`/`MU_UNIT`, `ExoticSourceModel{Renewable}` (there is **no** `Disabled` variant — "disabled" is `WorldLawSet.exotic_energy = None`), `SourceTopology{Uniform,Patchy}`, `BoundaryMode{Closed,Open}`, `ExoticEnergyLaw` + `validate`, `ExoticEnergyBudget` + `balance_error`, `ExoticEnergyField` + `from_law`/`step`/`uptake`/`budget`, `spend_storage` |
| `src-tauri/src/core/experiment.rs` | AE1 (AE-101/102/103/109) | `ExperimentError`, `fnv1a_64`, `Canon`, `BaselineEnergyLaw`, `WorldLawSet` + `fingerprint`/`validate`, `InitialConditionSet`, `ObservableSpec`/`ObservableRegistry` + `fingerprint`/`validate`, `ExperimentManifest` + `fingerprint`/`validate`/`control_variant`, `FactorDiff` |
| `src-tauri/src/core/experiment_runner.rs` | AE1 (AE-104/105/107/108/110/111) | `ExperimentModel` trait (incl. `Snapshot`/`snapshot`/`from_snapshot`), `RunProvenance`, `RunStatus`, `RunResult`, `run_manifest_seed`, `ForkReport`, `genesis_fork`, `CheckpointForkReport`, `checkpoint_fork`, `MetricSummary`, `EnsembleSummary`, `run_ensemble` |
| `src-tauri/src/core/reference_world.rs` | AE2 (AE-104/106/206/207/208) | `ReferenceEvolutionWorld` (impl `ExperimentModel`), `CAUSE_EXOTIC_WORLD_LAW` |
| `src-tauri/tests/exotic_energy_zero_alloc_tests.rs` | AE-203 | zero-heap-alloc hot-loop test |

### Key implementation decisions

- **Canonical identity, not serde/debug.** `Canon` builds a fixed-order, domain-separated byte
  stream (tags per enum/section, length-prefixed strings, floats by IEEE-754 bits) hashed with
  `fnv1a_64`. Set-like collections (seeds, observable ids, interventions, initial-condition entries)
  are sorted before hashing, so reordered non-semantic input yields the same fingerprint (AE-S02)
  while any material law change flips it (AE-S03). This reuses the engine's existing FNV-1a family
  (`world_artifact::fnv1a_32`) rather than inventing a new hash.
- **MU ≠ EU enforced at the type + validation boundary.** `UnitId::is_eu()` rejects an exotic law
  that claims the `EU` unit (`WorldLawSet::validate` → `ExperimentError::InvalidLaw`). The MU ledger
  (`ExoticEnergyBudget`) is entirely separate from the closed-EU `EcosystemBiomass`.
- **Exotic field decoupled from EU + RNG.** `ReferenceEvolutionWorld::step` runs the field on the
  ecology band with pure arithmetic (no RNG draw) and never writes back into the trophic pools, so a
  control/treatment genesis fork produces a **byte-identical EU trajectory** — the intended AE2
  result (measurable MU field + closed MU ledger, biomass unaffected). This is what makes AE-S05
  provable as an exact-equality (`delta.abs() < 1e-12`) rather than a tolerance.
- **Baseline is bit-identical, not merely close.** With `exotic_energy = None`,
  `ReferenceEvolutionWorld::checksum()` returns the inner `ReferenceEcosystem::checksum()` bytes
  unchanged, and the runner mirrors the legacy `run_scenario` loop (same `StdRng::seed_from_u64`,
  `SimClock`, `InterventionQueue`), so a baseline manifest reproduces the legacy scenario checksum
  exactly (AE-S01). An extra RNG draw would have diverged the checksum, so this equality doubles as
  the "no hidden RNG draw" proof (AE-208).
- **Determinism from declared seeds only.** Patchy hotspot placement derives its `StdRng` from
  `seed ^ fnv1a_64(source_id)` at construction; the hot loop draws no RNG. No `thread_rng()` anywhere.
- **Zero-alloc hot loop.** `ExoticEnergyField` is SoA with a preallocated `next` scratch and
  `source_mask`; `step` uses `copy_from_slice` and iterator sums only. Proven by
  `test_exotic_field_hotloop_zero_heap_allocations` (1000 ticks, 0 allocations).
- **Failure preservation.** `RunResult`/`RunStatus` keep failed runs; `run_ensemble` never drops a
  failed seed. The later AE2.5 paired gate adds same-seed `SeedPair` records, paired effects/intervals,
  and preserves one-sided failures. The historical pre-AE2.5 partial status below is superseded.
- **Causal chain (AE-207).** The exotic field's per-tick change is recorded as a chain rooted at the
  reserved `CAUSE_EXOTIC_WORLD_LAW` cause id, giving a world-law → field trace (AE-S12 partial).

### Deliberate scope boundaries at the AE2 checkpoint (historical; AE3 extends the reference model)

- At the AE2 checkpoint there was no organism pathway/genotype: `storage`/`spent_dissipated` on
  `ReferenceEvolutionWorld` were the
  AE-205 organism-storage **test double** and stay at zero; the transaction seams are exercised only
  by `ExoticEnergyField`'s own unit tests. The opt-in AE3 reference population below supersedes this
  limitation for pathway transactions and selection, but not for live ECS entities or species.
- `Finite`/`Pulsed` source models and the `FieldArtifact` topology are intentionally **absent** from
  the enums (deferred behind a future ADR) so no unsupported variant can be silently mishandled.

## Supervisor-audit follow-up fixes (2026-07-25, second pass)

A review found six in-scope gaps in the first checkpoint. All are now fixed and verified with fresh
command output. **Process note (honest):** these fixes were implemented code-first and then covered by
new tests in the same pass — they were *not* written test-first (no test was observed failing before
its fix). The evidence below is therefore end-state verification, not TDD process evidence.

1. **AE-107 / AE-S09 checkpoint fork — now IMPLEMENTED (headless, no persistence/live Bevy).** Added a
   `type Snapshot: Clone` + `snapshot()` + `from_snapshot()` seam to `ExperimentModel`, and
   `experiment_runner::checkpoint_fork`. It runs the shared prefix over ticks `1..=fork_tick`
   **once**, captures a model snapshot **and clones the live `StdRng`** *after* `fork_tick` has been
   processed, and continues both branches over ticks `fork_tick+1 ..= duration_ticks` from that
   checkpoint — never re-simulating the prefix. `from_snapshot` restores full state but starts a fresh
   post-fork causal chain. Completeness proof: a control branch continued with identical inputs equals
   an uninterrupted run **bit-for-bit** (`ae_s09_checkpoint_continuation_equals_uninterrupted_run`) —
   the RNG-clone is what makes this exact. Children carry `parent_run_id = prefix` and `fork_tick`.
2. **Silent MU loss at field construction — fixed.** `ExoticEnergyField::from_law` now returns
   `Result`: an `initial_amount` that cannot be placed without clamping (topology/`max_density`/grid
   capacity) is **rejected** with a structured reason rather than silently shrinking the declared
   initial condition. Propagated through `ExperimentError::FieldConstruction`. Tests:
   `over_capacity_initial_amount_is_rejected_not_silently_clamped`,
   `over_capacity_law_fails_construction_with_structured_error`.
3. **Ledger-exact uptake across f32 field / f64 storage — fixed.** `uptake` now performs the
   withdrawal in f32 and credits storage the **actual field decrease**, not the pre-rounding f64
   request, so `field_delta == storage_gain` bit-exactly. Test:
   `fractional_and_repeated_uptake_is_ledger_exact` (documented f32-granularity tolerance). The
   `budget_error` observable and `RunResult.exotic_budget` now read one authoritative
   `ReferenceEvolutionWorld::current_budget()` (spend sink folded in consistently) so they can never
   disagree.
4. **Disabled semantics unambiguous — fixed.** The redundant `ExoticSourceModel::Disabled` variant was
   **removed**. `exotic_energy = None` is the sole baseline (no field, no field-RNG, no cost); a
   `Some(law)` is always a live source and can never masquerade as the baseline. Test:
   `exotic_none_is_the_only_baseline_path`.
5. **Runner entry points hardened.** `run_manifest_seed`/`run_ensemble` now validate the registry and
   manifest up front; an invalid config yields a **preserved `RunStatus::Failed`** (structured reason),
   never silent execution, and every emitted observable gets `ObservableSpec` metadata (self-describing
   for the full emitted set, not just requested ids — no silent `filter_map` drop). Tests:
   `run_manifest_seed_fails_on_unknown_observable_not_silently`,
   `run_manifest_seed_fails_on_invalid_manifest_and_registry`, and (renamed in the third pass)
   `ensemble_rejects_invalid_and_empty_inputs_at_ensemble_level`,
   `result_is_self_describing_for_every_emitted_observable`.

## Supervisor edge-case pass (2026-07-25, third pass)

A final edge-case review found five more in-scope defects; all fixed and covered by new tests.
**Process note (honest):** as in the second pass, these were implemented code-first with tests added
in the same pass — not test-first. What is proven below is the verified end state, not TDD process.

1. **Undeclared seed rejected before model/RNG.** `run_manifest_seed` now returns a preserved
   `RunStatus::Failed` (structured `ExperimentError::SeedNotInManifest`) when the requested seed is not
   in `manifest.seeds`, before constructing any model or RNG; `checkpoint_fork` returns the same error
   as a structured `Err`. Tests: `run_manifest_seed_rejects_seed_not_in_manifest`,
   `checkpoint_fork_rejects_seed_not_in_manifest`.
2. **`run_ensemble` now returns `Result` and validates once.** An invalid registry/manifest — including
   an **empty seed set** — is a structured ensemble-level `Err` (`EmptySeeds` etc.), not a misleading
   zero-run summary; per-seed *runtime* failures are still preserved inside the `Ok` summary. No
   fabricated seed. **API change**: callers must handle `Result`. Test:
   `ensemble_rejects_invalid_and_empty_inputs_at_ensemble_level`.
3. **`RunResult` metadata/warnings cover the union of all emitted observables.** `assemble_result`
   now builds the deterministic union of every `StateSample.observables` name plus `final_observables`
   (first-appearance order), so a **transient series-only** observable is described and any missing
   spec is warned — not just the final set. Stale "requested observables" comments corrected. Test:
   `result_describes_transient_series_only_observable_with_missing_spec_warning`.
4. **Checkpoint tick semantics made exact + guarded.** Documented that the prefix processes
   `1..=fork_tick`, the snapshot is taken **after** `fork_tick`, and branches process
   `fork_tick+1 ..= duration_ticks`. `checkpoint_fork` now rejects a `treatment_extra` intervention
   whose `start_tick <= fork_tick` (belongs to the shared prefix) or `> duration_ticks` (never
   applied) with `ExperimentError::InapplicableIntervention`, so no unapplied factor is declared; and
   a prefix that diverges before the checkpoint returns `ExperimentError::CheckpointPrefixFailed`
   instead of continuing from partial state with a wrong `remaining`. Tests:
   `checkpoint_rejects_treatment_extra_outside_post_fork_window` (boundaries `fork_tick`,
   `fork_tick+1`, `duration_ticks`, `duration_ticks+1`),
   `checkpoint_fork_fails_structurally_when_prefix_diverges`.

## Contract-hardening pass (2026-07-25, fourth pass)

Four further defects fixed. **These were genuinely test-first**: eight focused tests were written and
observed failing (including a JSON round-trip that failed with a real `null`-float data loss) before
any fix was applied.

- **A — registry ranges are JSON-safe; validation is strict.** `ObservableSpec` bounds in
  `ObservableRegistry::reference_default` no longer use `f64::INFINITY`; they use the finite
  `ObservableRegistry::{OPEN_UPPER_BOUND, OPEN_LOWER_BOUND}` (`±1e300`). `serde_json` renders a
  non-finite float as `null`, which **cannot be parsed back into an `f64`** — so an infinite bound
  silently destroyed the range metadata (and broke deserialization) of the supposedly self-describing
  `RunResult`. `validate()` now also rejects non-finite/NaN bounds, `cadence_period == 0`, and empty
  `display_name`/`cadence_name`/`source` (id/unit checks unchanged). No new schema was introduced.
  Tests: `defect_a_reference_registry_is_json_safe_and_finite`,
  `defect_a_registry_validation_rejects_malformed_specs`,
  `defect_a_run_result_json_round_trips_for_baseline_and_treatment` (completed baseline **and**
  treatment runs).
  **Honest limitation recorded by that test:** JSON export preserves float *payload* values to
  serde_json's documented ±1 ULP (the `float_roundtrip` feature is not enabled — the same caveat the
  existing `dynamic_fields` save/load test records). Structural data (checksum, provenance, specs,
  observable names/count) round-trips exactly.
- **B — manifest-path intervention validation.** New reusable
  `experiment::validate_intervention(cmd, run_ticks)` rejects non-finite/negative `intensity`,
  invalid `Radius` (non-finite centre/radius, non-positive radius), inverted `Rect` bounds, a
  `start_tick + effective_duration` that overflows `u64`, and a schedule whose active window never
  intersects run ticks `1..=duration_ticks`. `ExperimentManifest::validate` applies it; legacy
  `core/intervention.rs` is **unchanged**. `checkpoint_fork` applies the same helper to
  `treatment_extra` and additionally enforces the combined `MAX_INTERVENTIONS` ceiling and unique ids
  both *within* the extras and *against* the base interventions — all before any model/RNG. Fork-window
  checks run first so the specific `InapplicableIntervention` is not masked by the generic error. No
  per-kind upper limits were invented. Tests:
  `defect_b_manifest_rejects_invalid_intervention_values`,
  `defect_b_checkpoint_validates_treatment_extra_values_and_ids`,
  `defect_b_checkpoint_enforces_combined_intervention_limit`.
- **C — exotic law/field defensive validation.** `ExoticEnergyLaw::validate` rejects an empty
  `display_name`; the public `ExoticEnergyField::from_law` now re-validates the law defensively and
  **rejects** zero or overflowing grid dimensions instead of silently coercing `0 → 1` (which would
  quietly change the declared world size). Tests: `defect_c_law_rejects_empty_display_name`,
  `defect_c_from_law_validates_defensively_and_rejects_bad_grids`.
- **D — documentation honesty.** Stale `ExoticSourceModel::Disabled` claims removed from the current
  status/implementation sections (see the AE-S14 and AE-202/208/209 corrections in the planning doc).

### AE-S14 was PARTIAL before AE2.5 (historical; superseded)

At this fourth-pass checkpoint, `EnsembleSummary` reported N, completed/requested counts,
per-observable mean/std/min/max and a 95%
CI, and preserves every failed run. It does **not** provide a control–treatment **effect-size** API,
which AE-S14 also requires ("Summary chứa N/**effect**/interval/failures"). Comparative effect size
was deliberately **not** implemented in this pass. Every AE-S14 claim in this slice is therefore
recorded as **PARTIAL**. The authoritative AE2.5 audit section below supersedes this state with
`run_paired_ensemble` / `run_paired_ensemble_with_control`.

## Provenance & transaction audit (2026-07-25, fifth pass)

Four defects, all **test-first** (7 focused tests written and observed failing before any fix; the P3
failures were material — a `NaN` uptake request moved **6.25 MU** and a `NaN` spend drained the entire
**10.0** balance, because `f64::min` returns the non-NaN operand and so slipped past the `<= 0.0`
guards).

### P1 — effective-treatment provenance contract (material for replay)

`checkpoint_fork` previously stamped **both** branches with the *base* manifest fingerprint and a
base-derived run id, even when `treatment_extra` changed the treatment's effective input — so a
treatment run was not independently addressable or replayable, and `declared_factors` (a lossy
`kind@start` string) was the only record of the difference.

The contract is now explicit:

- An **effective treatment manifest** is built by cloning the base manifest and appending
  `treatment_extra`, and is **validated in its own right** (the combined set must itself be a legal
  manifest).
- **Prefix and control** provenance keep the **base** manifest fingerprint and base-derived run id
  (the prefix is shared history; the control continues the base input unchanged).
- **Treatment** provenance carries the **effective treatment manifest's** fingerprint and a run id
  derived from it, so the two branches are independently addressable.
- With **no extras** the effective manifest is byte-identical to the base, so all three fingerprints
  coincide — the no-treatment fork remains a pure replay of the base input.
- `law_fingerprint` is shared across branches by design: a checkpoint fork never changes a world law
  (that would be a genesis fork).
- `CheckpointForkReport` now carries **`treatment_extra: Vec<InterventionCommand>`** — the fully
  structured, lossless declaration of what differs. `declared_factors` remains **display-only and must
  never be parsed**; that is now stated on the field itself.

Tests: `p1_treatment_provenance_uses_effective_manifest_fingerprint` (treatment fingerprint equals an
independently reconstructed effective-manifest fingerprint and differs from control; run ids differ;
prefix stays base), `p1_no_extras_keeps_control_and_treatment_fingerprints_equal`,
`p1_report_carries_structured_extras_that_survive_json_roundtrip`,
`p1_fork_remains_deterministic_and_tick_exact` (determinism and exact fork-tick semantics preserved).

### P2 — `genesis_fork` registry preflight

`genesis_fork` now calls `registry.validate()` **before** any manifest validation, model construction
or RNG work, so a malformed catalogue cannot silently shape both runs' result metadata. Test:
`p2_genesis_fork_validates_registry_before_any_model_work` (unsupported version **and** duplicate-id
registry both fail preflight; no model is stepped).

### P3 — AE-205 transaction numeric hardening (scope: transaction helpers only)

`ExoticEnergyField::uptake` and `spend_storage` now reject non-finite or negative `requested`/`amount`,
`capacity`, and **the current ledger slots themselves** (`storage`, `dissipated`, and the cell's own
density) *before* touching any state: an invalid transaction returns `0.0` and leaves field, storage
and sink bit-unchanged. No physiology/genotype behaviour was added. Tests:
`p3_uptake_rejects_non_finite_and_negative_inputs_without_mutating`,
`p3_spend_storage_rejects_non_finite_and_negative_inputs_without_mutating`.

### P4 — clippy

The guarded tick modulo in `drive` now uses `tick.is_multiple_of(sample_period)` (stable since Rust
1.87; toolchain is 1.95). **The new AE modules now produce zero clippy warnings.** The pre-existing
warnings in `scenario.rs`/`sim_clock.rs`/`dynamic_fields.rs` were deliberately not touched.

## AE2.5 — runtime source forcings, effect size, fixtures (2026-07-25)

Goal document: [2026-07-25-claude-overnight-goal-ae25.md](../planning/2026-07-25-claude-overnight-goal-ae25.md).
Closes the three items AE1–AE2 left open (AE-209, AE-210, AE-S14 effect size). **All five milestones
were written test-first**: each focused test was run and observed failing (unresolved type, missing
method, missing manifest field, missing function, missing fixture) before the fix landed.

### Files changed

| File | What |
|---|---|
| `core/exotic_energy.rs` | `ExoticInterventionKind{AddSource,RemoveSource,Pulse}`, `ExoticIntervention`, `ExoticInterventionQueue`, `ExoticEnergyField::apply_forcing` |
| `core/experiment.rs` | `ExperimentManifest.exotic_interventions` (serde-defaulted), `ExperimentError::InvalidExoticIntervention`, canonical fingerprint contribution, `FactorDiff` path + allowlist, `control_variant` stripping, fixture tests + regenerator |
| `core/experiment_runner.rs` | `ExperimentModel::from_manifest` now takes `forcings` + `run_ticks`; `EffectSize`, `EnsembleComparison`, `compare_ensembles` |
| `core/reference_world.rs` | Forcing queue + per-tick application with causal records |
| `tests/fixtures/experiments/*.json` | Three committed manifest fixtures |

### Design decisions

- **A forcing is a state effect, never a law edit.** `ExoticIntervention` acts on the *field*;
  `WorldLawSet` stays immutable for the whole run (ER01). This is machine-checked: the world-law
  fingerprint is asserted identical between a forced and an unforced manifest, while the *manifest*
  fingerprint (the declared input) is asserted different.
- **Separate type, not a widened legacy enum.** `InterventionCommand`'s five kinds are matched
  exhaustively by `core/scenario.rs`, which must stay untouched (AE-111), so exotic forcings are a
  distinct type that *reuses* `Region`/`Curve`. Legacy `intervention.rs` is unchanged.
- **Every moved MU is booked; prevented MU is not invented.** `apply_forcing` credits
  `cum_sourced` for the actual bounded `AddSource`/`Pulse` increase. `RemoveSource` never moves
  stored MU: it suppresses only the renewable contribution that could enter the remaining cell
  headroom, lowers `cum_sourced`, and records the positive counterfactual separately in
  `cum_source_suppressed`.
- **Own `CauseId`.** Forcing effects are recorded on target `exotic.forcing` under the command's own
  cause id, distinct from `CAUSE_EXOTIC_WORLD_LAW`, so a trace separates "the law established a
  field" from "an intervention changed it" (AE-S12 partial).
- **Forcings fire before the field's own dynamics** in the same ecology tick, so an add/remove at
  tick T is visible to that tick's diffusion/decay.
- **A forcing with no field is rejected**, not ignored: declaring one while `exotic_energy = None`
  is a structured error, because silently dropping a declared input would misstate the experiment.
- **`control_variant` strips forcings with the law** — the exotic regime (law + its forcings) is one
  declared factor, so `FactorDiff::genesis_exotic` allows both paths and nothing else.

### ⚠️ Superseded by the AE2.5 audit pass

The AE-S14 subsection immediately below described the **first** AE2.5 pass and was **wrong**:
`compare_ensembles` is an *independent-sample* comparison and never satisfied the paired, same-seed
causal design AE-S14 requires. It is retained for history; the authoritative statement is
"AE2.5 audit pass" further down.

### AE-S14 status after AE2.5 — honest scope (FIRST PASS — SUPERSEDED)

`compare_ensembles` reports, per observable emitted by **both** sides: mean difference (in the
observable's unit), **Hedges' *g***, a 95% normal-approximation interval on the mean difference, and
per-side completed-run N; failures from both sides are preserved, and one-sided observables are listed
rather than compared against a fabricated zero. Degenerate (zero-variance) comparisons yield `g = 0`,
never `NaN`/`±inf`.

That satisfies the gate's "N / effect / interval / failures" contract **for the headless reference
ensemble**. It is a *descriptive* statistic, not a significance test. AE-S14's remaining release scope
(AE-703 seed tiers, AE-610 UI surfacing) is still open, and because AE1–AE2.5 contain **no pathway,
reproduction or selection**, an effect size here must not be read as adaptation, ecotype or species
evidence.

## AE2.5 audit pass — paired/causal/checkpoint contract (2026-07-25, authoritative)

A supervisor inspection found four contract defects in the first AE2.5 pass plus one missing API. All
were fixed **test-first** (each test observed failing on the old code first).

### D1 — AE-S14 is now genuinely paired

`compare_ensembles` compared two independently-run ensembles with a pooled, independent-sample
Hedges' *g*. It never checked that both sides used the same seeds in the same order, nor that the
factor diff was validated — so it was **not** a same-seed causal control/treatment design. It is kept
as an explicitly-documented **descriptive helper that is not the gate**.

New gate API:
- `run_paired_ensemble<M>(treatment, registry, allowed) -> PairedEnsembleReport` — derives and
  validates the control variant (which clones the treatment's seeds, so the identical ordered seed set
  holds *by construction*), validates registry/manifests/factor-diff **before** any model or RNG work.
- `run_paired_ensemble_with_control<M>(control, treatment, …)` — for factors `control_variant` cannot
  express (it only strips the exotic regime); rejects a seed-order mismatch structurally.
- `SeedPair` keeps **both** `RunResult`s per seed, including one-sided failures — a half-pair is never
  dropped; `PairedEnsembleReport::{complete_pairs, incomplete_pairs}`.
- `PairedEffect` reports `n_requested`, `n_complete_pairs`, control/treatment means, **paired mean
  delta** (primary effect), paired SD, SE, 95% CI and Cohen's *d_z*, all as `Option<f64>` under a
  documented defined-ness contract: `n=0` ⇒ everything `None`; `n=1` ⇒ means/delta defined, spread
  `None`; zero paired variance ⇒ SD/SE/CI defined (zero-width) but **`paired_dz = None`**. Nothing is
  fabricated and no `NaN`/`inf` can reach JSON.

### D2 — `RemoveSource` suppresses the source; it does not drain the field

Old behaviour debited stored MU and credited `cum_dissipated` — a sink smuggled under a "remove
source" name. Now: `RemoveSource` reduces the base renewable source contribution in its region/window,
**capped at that contribution**, and **never removes MU already present**. It therefore lowers
`cum_sourced`; the counterfactual is recorded in the new diagnostic `cum_source_suppressed` (not a
ledger slot — suppressed MU never existed, so the balance equation is unchanged). Overlapping
suppressions are jointly capped and credited in deterministic queue order.
`ExoticEnergyField::apply_forcing` now **ignores** `RemoveSource` so the drain cannot reappear. A real
drain would require a separate `DrainField` kind — deliberately **not** added.

### D3 — cadence and spatial validation

Exotic dynamics fire only on `ECOLOGY_PERIOD`, so `amount` is documented as **MU per ecology firing,
per affected cell**, and `ExoticIntervention::validate` rejects any window containing no ecology
firing within `1..=duration_ticks` (boundary-exact: `[59,61)` ✓, `[61,120)` ✗, `[61,121)` ✓, last
firing `6000` ✓). `ExoticEnergyField::validate_region_applicable` — enforced at model construction and
on checkpoint reconfiguration — requires `Cell` and the entire inclusive `Rect` to be contained by
the grid. `Radius` requires a finite positive radius with an **in-grid centre**; a disc overhanging
an edge is **clipped**, not rejected (documented).

### D4 — no causal double attribution

The world-law delta was computed as `total − last_field_total` *after* forcings had already moved MU,
so an injection was attributed twice. Measured on the old code: world-law delta **506.16** absorbed a
**512.0** injection whose unforced baseline was **−0.72**. The field total is now re-baselined
immediately after forcings, so `exotic.density_total` under `CAUSE_EXOTIC_WORLD_LAW` describes only
source/decay/diffusion. `RemoveSource` records a positive counterfactual on `exotic.source_suppressed`
under its own `CauseId` and emits **no** movement record, because no MU moved.

### AE-209 checkpoint exotic channel

`checkpoint_fork_with_exotic<M>(…, treatment_extra, treatment_extra_exotic)`; `checkpoint_fork`
delegates to it with no exotic extras, so existing callers are untouched. Every exotic extra is
validated **before** snapshot/model/RNG: structural validity, grid applicability, uniqueness within
the extras and against base forcings, exotic-field presence, combined `MAX_INTERVENTIONS`, and an
**ecology firing strictly after `fork_tick`** within `duration_ticks`. The effective treatment manifest
(base + extras) drives treatment provenance/fingerprint/run-id while prefix and control keep base
identity; the treatment branch is reconfigured from the shared snapshot via the new
`ExperimentModel::reconfigure_forcings` seam (default impl errors on non-empty forcings, so an
unsupporting model fails loudly). Control continuation stays **bit-for-bit** equal to the uninterrupted
run; the prefix is never re-simulated. `CheckpointForkReport.treatment_extra_exotic` carries the
structured commands.

### AE-S14 / AE-209 final status

At the AE2.5 checkpoint, both were **complete for the then-current headless reference slice**:
AE-S14 via the paired runner, AE-209 via the
schema + budgeted suppression/injection + manifest and checkpoint channels. Still open and unclaimed:
AE-703 seed tiers, AE-610 UI surfacing, pathway/selection/speciation, live Bevy, persistence and
the map gate. Because this slice has no pathway or reproduction, **no paired effect here is evidence of
adaptation or speciation**. The AE3 section below supersedes only the headless pathway/selection part.

The independent closure refinements (full-Rect containment, pure checkpoint grid preflight,
realizable-headroom suppression, cumulative causal attribution, and three-kind checkpoint timing
evidence) are recorded in the
[AE2.5 goal handoff](../planning/2026-07-25-claude-overnight-goal-ae25.md#independent-codex-closure-pass-2026-07-25)
and the testing document. They change no world law, schema version, live Bevy/UI/persistence path, or
legacy checkpoint signature.

## AE3 — energy pathway and selection reference slice (2026-07-25)

Goal document: [2026-07-25-claude-overnight-goal-ae3.md](../planning/2026-07-25-claude-overnight-goal-ae3.md).
Closes AE-301…AE-310 **for the headless reference model only**. This is the slice where an adaptation
claim first becomes testable — and where the ER03 "no magic effects" rule first has something real to
constrain.

### Files changed

| File | What |
|---|---|
| `core/evolution_pathway.rs` (new) | Serde-round-trippable `EnergyPathwayGenotype` (+`legacy`/`new`/`mutate`/`crossover`/`is_bounded`), `DevelopedEnergyPathway::develop`, `ExoticEnergyState`, `PathwayCohort` (+`uptake`/`metabolize`/`update_performance`), `ReferencePopulationConfig` (+`validate`/`from_initial_conditions`), `ReferencePopulation` (+`step_physiology`/`reproduce`), `ReproductionOutcome`, the `AE3_KEY_*` seam and `AE3_OBSERVABLE_IDS` |
| `core/mod.rs` | `pub mod evolution_pathway;` (append-only) |
| `core/experiment.rs` | 10 AE3 `ObservableSpec`s, `ExperimentError::InvalidPopulation`, AE3 initial-condition + observable preflight in `ExperimentManifest::validate` |
| `core/reference_world.rs` | `population` field, `step_population`, checksum/observables/budget/snapshot integration |
| `tests/exotic_energy_zero_alloc_tests.rs` | AE3 physiology hot-path zero-allocation test |

`experiment_runner.rs` was **not** touched — the generic runner needed no contract fix.

### Design decisions

- **Performance is a mechanism result, never a fitness field.** `PathwayCohort::update_performance`
  reads `state.last_spent_mu`, **not** `expressed`. A pathway therefore gains nothing until an atomic
  field→storage uptake *and* a storage→dissipated spend have both completed, while its
  maintenance/opportunity cost is paid on every ecology firing regardless. That asymmetry is what
  makes AE-S06 and AE-S07 provable rather than asserted. It is enforced by mutation testing, not just
  by reading the code: injecting a flat bonus keyed on `expressed` fails 6 tests.
- **Frequency moves only at reproduction.** Uptake, spend, cost and performance accounting are
  structurally forbidden from writing `count` or `genotype`; only `ReferencePopulation::reproduce`
  may, and the recorded delta is *derived from* the resolved offspring counts
  (`pathway_offspring / births`), never written independently (AE-S10).
- **A separate deterministic RNG stream.** The population draws from `StdRng::seed_from_u64(seed ^
  RNG_DOMAIN)`, never the ecology `rng` handed to `step`. This is exactly why the AE-S01 baseline
  stays bit-identical — an extra draw on the shared stream would have diverged the legacy checksum.
  The reproduction jitter is drawn *unconditionally*, so a treatment never changes draw order (ER07).
- **Opt-in through the version-1 initial-condition seam, not a new schema.** 13 documented `ae3.`
  scalar keys; absence of all of them disables the population and restores the AE1–AE2.5 path exactly.
  No schema version moved and no existing fixture needed migration. An **unknown `ae3.` key is
  rejected**, so a typo cannot silently become a no-op input.
- **Disabled means silent, not zero.** AE3 observables are emitted only when the population exists,
  and a manifest requesting one without enabling a population fails preflight with
  `InvalidPopulation` rather than returning a fabricated `0.0`.
- **Observable metadata describes emitted values.** `evolution.births` is a cumulative counter, so
  its registry aggregation is `Instant`; marking cumulative snapshots as `Sum` would count the same
  births repeatedly inside a cadence window.
- **MU accounting is unified.** `exotic.stored` and the budget's `organism_storage` both read
  `organism_storage_total()`, so they cannot disagree; parental storage is released into `dissipated`
  at each generation boundary, so replacement cannot leak or mint MU.
- **Causal attribution stays earned.** A performance effect roots at `CAUSE_EXOTIC_WORLD_LAW` **only**
  by descending from a real `exotic.uptake` effect; with no uptake it roots at `CAUSE_BACKGROUND`.
  An absent-source world therefore records selection under background dynamics and **no fabricated
  Mana cause** — machine-checked by
  `ae309_absent_source_cost_roots_at_background_not_a_fabricated_mana_cause`. A second test proves
  the full chain roots at an AddSource forcing when the field was empty, the law has zero renewable
  source rate, and exactly one forcing supplied MU. Mixed-origin fields conservatively keep the
  existing world-law/field parent because the MVP ledger supports only one immediate parent.
- **Storage is a real buffer.** Metabolic demand is a fraction of the *reserve*, not of the uptake
  surface. The first implementation scaled demand to uptake surface, which drained storage every tick
  and made `storage_capacity` a dead trait; a failing test caught it.

### Honest scope limits

- `crossover` is a tested public API but is **not** wired into reproduction — the two-strategy cohort
  model inherits clonally within a strategy with bounded mutation, because recombining a legacy and a
  pathway genotype would make pathway frequency undefined. The API returns `None` for two expressed
  parents with different `EnergySourceId`s rather than creating a source/trait chimera.
- Generational death is represented as full parental cohort turnover and MU-storage release; the
  reference slice does not expose a separate per-individual mortality process or death observable.
- `reproduce` allocates (genotype `String` clones); it is per-generation, not a hot path, and is
  excluded from the zero-alloc test explicitly rather than counted as zero.
- The population is a **two-cohort aggregate**, not live ECS entities. No species, live Bevy,
  persistence, UI or map claim follows from it. ADR-0002 stays `proposed` and the `1e-4` MU tolerance
  remains a test tolerance, not product policy.

## Historical verification status at AE2.5 (superseded by the AE3 evidence in the testing document)

Fresh results recorded in the [testing doc](../testing/2026-07-24-feature-alternate-evolution-world-lab.md#planned-verification-commands).
Summary (2026-07-25, **AE2.5**, `cargo` on Windows, debug profile):

- `cargo test --manifest-path src-tauri/Cargo.toml --lib` → **173 passed, 0 failed, 1 ignored**
  (73 pre-feature baseline + 100 AE tests; the ignored test is the fixture regenerator, run
  explicitly). The audit pass added 22 tests and deleted 2 that encoded the old drain semantics.
- Focused (audit): `--lib d1_` → 8; `--lib d2_` → 6; `--lib d3_` → 2; `--lib d4_` → 2; `--lib ck_` →
  4 (+1 pre-existing match); `--lib ae210_m5` → 3. All 0 failed.
- `cargo test --test exotic_energy_zero_alloc_tests` → **2 passed** (field hot loop **and** the final
  forcing hot path: suppression reset + accumulation + injection + step).
- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` → clean (exit 0).
- `cargo clippy --lib` → **4 warnings total, and ZERO in the new AE modules**
  (`experiment.rs`, `experiment_runner.rs`, `exotic_energy.rs`, `reference_world.rs`). All 4 are
  pre-existing and untouched: `dynamic_fields.rs:180` (needless range+index), `scenario.rs:409`
  (`manual_is_multiple_of`), `sim_clock.rs:66` (`manual_is_multiple_of`), `sim_clock.rs:77`
  (`manual checked division`).
- `git diff --check` → clean, exit 0 (only benign LF/CRLF advisory warnings).

At that checkpoint this was a verified in-scope foundation, not a feature-completion claim. AE3 now
extends the headless model as documented above; species, live Bevy, persistence/save (AE-S15), UI
parity (AE-S13), and the map gate remain deferred. ADR-0002 stays `proposed`, and the MU tolerance
policy (AE-006) remains user-owned (tests use a local `1e-4` relative tolerance only).
