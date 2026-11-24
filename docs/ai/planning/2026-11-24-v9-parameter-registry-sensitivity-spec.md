---
phase: planning
feature: v1-vertical-slice
title: Card V9 — Parameter Registry and Multi-Seed Sensitivity Analysis
description: Implementation spec for card V9 centralizing vertical-slice simulation constants with units/bounds/sources (S62) and running multi-seed reproducible sensitivity analysis (S63, S64)
status: proposed
owner: anima-dell
last_reviewed: 2026-11-24
---

# Card `V9` — Parameter Registry & Sensitivity Analysis (`M11.1`–`M11.3`)

> **Design spec only (`status: proposed`).** No Rust, TypeScript, or docs-index edits are made in D8 (`V-SPECS-2`).
> All trunk `file:line` anchors below were checked against `origin/feat/ecological-selection` at
> `8d52341549007a50ce033a6d272c5dd22a71b1b1`. Anything not on trunk yet is labeled **`to be created by V9`**.

## 1. Goal, Dependencies, `START WHEN` & Scope

Eliminate un-documented arbitrary magic constants (`"Khoa học giả do hằng số tùy ý"`, [`WORLD_SIMULATION_PLAN.md`](../../../WORLD_SIMULATION_PLAN.md) `:1122`) across the v1.0 vertical slice per `WORLD_SIMULATION_PLAN.md` `§7 M11.1–M11.3` (`:909-913`), `§8.6 S62–S64` (`:1003-1005`), and `2026-11-05-swarm-master-plan.md` Đợt 6 `V9`:

1. Build a machine-checked **`ParameterRegistry`** (`M11.1` / `S62`, `to be created by V9`) listing every governing vertical-slice constant with its canonical key, physical/model **unit**, default value, valid `[min, max]` domain, subsystem owner, **scientific citation / rationale**, and sweep compatibility (`SweepTarget`, `src-tauri/src/core/sweeps.rs:46`).
2. Run multi-seed **sensitivity analysis** (`M11.2`–`M11.3` / `S63`–`S64`, `to be created by V9`) over existing `sweeps.rs` (`SamplingStrategy` Grid/Sobol/LatinHypercube at `sweeps.rs:303`) + `anima-lab` / multi-seed ensemble pipelines, identifying which parameters dominate each vertical-slice KPI deterministically across seeds.

### Exact Dependency Order & `START WHEN`

- **Depends directly on:** **[Card `V8`](2026-11-24-v8-slice-acceptance-spec.md)** (and transitively `V0`–`V6` — the vertical-slice acceptance scenarios & observables must already be running).
- **`START WHEN`:** `V8` is merged on trunk and owner decisions `D-V9-1`–`D-V9-4` are resolved.
- **Unblocks:** Tuned/defensible parameter justification for `H11` release & methods paper deliverables (`M11.6`, `V11` doc cross-links).

### Explicit Non-Goals

- **No silent retuning of existing frozen benchmarks (`E2`, `E3`, `E3b`):** Frozen preregistration manifests and legacy default values remain unchanged unless an explicit new manifest/parameter key overrides them; registry defaults must match current trunk runtime behavior when overrides are absent (`src-tauri/src/core/ecology.rs:26-37, 81, 155-241`).
- **No mid-run parameter mutation:** Every sensitivity sample executes as an independent genesis manifest (`CLAUDE.md:145-147`).

## 2. Current Trunk Anchors (`8d523415`) & Design

### 2.1 Governing Constants & Registries Verified on Trunk Today

1. **Ecological & metabolic constants (`src-tauri/src/core/ecology.rs:26-241`):**
   - `BOLTZMANN_EV` (`8.617_333e-5`, `:26`), `REF_TEMP_K` (`293.15`, `:28`), `E_ANIMAL_EV` (`0.65`, `:30`), `E_PRODUCER_EV` (`0.30`, `:32`), `METABOLIC_NORM` (`0.06`, `:34`), `KLEIBER_EXP` (`0.75`, `:37`), `NPP_TO_CAPACITY` (`0.01`, `:81`), `LINDEMAN_EFFICIENCY` (`0.40`, `:155`), `REPRODUCTION_EFFICIENCY` (`0.60`, `:158`), `CAPTURE_ATTACK` (`0.02`, `:174`), `CAPTURE_HANDLING` (`0.02`, `:175`), `DEFAULT_RARITY_REFUGE` (`0.20`, `:178`), `MASS_REFERENCE` (`20.0`, `:205`), `SEASON_AMPLITUDE` (`0.5`, `:241`).
2. **Spatial & conservation constants:**
   - `DEFAULT_GRID_DIM` (`256`, `src-tauri/src/core/sim_rules.rs:47`), `WORLD_MIN_XZ`/`MAX_XZ` (`-100.0`/`100.0`, `:51-53`), `WORLD_MIN_Y`/`MAX_Y` (`0.0`/`10.0`, `:56-58`), `STATE_VARIABLES` (`sim_rules.rs:179`).
   - `GROUND_CAPACITY` (`2.0`, `src-tauri/src/core/dynamic_fields.rs:33`), `NITROGEN_CAP` (`5.0`, `:47`).
   - `RESIDUAL_ABS_TOLERANCE_EU` (`1e-3`, `src-tauri/crates/anima-domain/src/energy.rs:123`).
3. **Manifest knobs & sweep infrastructure:**
   - `LIVE_KEYS` (`[&str; 31]` at `src-tauri/src/core/live_experiment.rs:280`, feeding `LiveWorldConfig`).
   - `src-tauri/src/core/sweeps.rs:19-80, 303, 325, 759, 904` (`ParameterDim`, `SweepTarget` `{ InitialCondition | ExoticLawField | InterventionIntensity | InterventionStartTick | InterventionDuration }`, `DimensionSpec`, `SamplingStrategy` Grid/Sobol/LHS, `SweepManifest`, `SweepResult`, `run_sweep`).
   - `src-tauri/src/core/experiment/laws.rs:11-80, 142-160` (`BaselineEnergyLaw`, `WorldLawSet::fingerprint`, `InitialConditionSet`).
   - `src-tauri/src/bin/anima-lab.rs:95-108, 210, 427-445` (`Command::Ensemble`, `Command::Sweep`, `Command::Branch`) and CLI usage in [`RUN_EXPERIMENTS.md`](../../how-to/RUN_EXPERIMENTS.md) `:31-55` and [`WRITE_A_MANIFEST.md`](../../how-to/WRITE_A_MANIFEST.md) `:17-55`.

### 2.2 Registry & Sensitivity Pipeline (`to be created by V9`)

1. **`ParameterRegistry` & `ParameterEntry` (`src-tauri/src/core/parameter_registry.rs`, `to be created by V9`):**
   - Pure metadata + validation catalog indexed by unique static `id: &'static str` (matching `STATE_VARIABLES` style at `sim_rules.rs:179`): `{ id, unit, default_value, min, max, source_citation, rationale, subsystem, sweep_target }`.
   - `ParameterRegistry::validate()` rejects empty `unit`, empty `source_citation`, non-finite bounds, `min > max`, `default_value` outside `[min, max]`, duplicate IDs, or any `SweepTarget` referencing an unregistered parameter key when strict mode is requested (`S62`).
   - Add an optional `SweepTarget::RegistryParameter(String)` variant (`to be created by V9`, keeping JSON backwards compatibility in `sweeps.rs:46`) or route vertical-slice parameters through validated `InitialConditionSet` keys so `V9` sweeps perturb real parameters without editing code constants.
2. **Multi-seed sensitivity engine (`src-tauri/src/core/sensitivity.rs` + `run_v9_sensitivity.rs` / `anima-lab sensitivity`, `to be created by V9`):**
   - Wraps deterministic `SamplingStrategy` (`sweeps.rs:303`) + multi-seed `ExperimentManifest.seeds` (`S64`: every design point runs across the same ordered seed set `S` rather than a single seed).
   - Computes elementary-effect / standardized regression / variance-based dominance indices (per `D-V9-1`) across target KPIs (`WORLD_SIMULATION_PLAN.md §10.1`: grass biomass, herbivore/predator persistence, peak cascade magnitude, eutrophication level, water/EU residual) and outputs a ranked `SensitivityReport` (`to be created by V9`, with same-OS deterministic tie-breaking).

## 3. Applicable [`CLAUDE.md`](../../../CLAUDE.md) Traps & Invariants

- **`WorldLawSet` immutability (`CLAUDE.md:145-147`):** Never mutate constants or world laws mid-run during sensitivity loops; each parameter sample creates a fresh manifest/seed run whose per-sample `manifest_fingerprint` & `law_fingerprint` (`laws.rs:79`) are recorded by `sensitivity.rs` (`to be created by V9`; trunk `SweepResult` at `sweeps.rs:759-774` records `base_manifest_fingerprint` only). `exotic_energy = None` stays the baseline compatibility path (`CLAUDE.md:153`).
- **`total_cost` & EU/water/soil conservation (`CLAUDE.md:105-107`, `energy.rs:123`):** Any swept metabolic/feeding/reproduction parameter **must** preserve exact EU accounting through `total_cost` in `metabolic_decay_system` and never break `RESIDUAL_ABS_TOLERANCE_EU` or `dynamic_fields` conservation bounds.
- **Zero-heap-allocation hot loop (`CLAUDE.md:50`):** `ParameterRegistry` is constructed/validated at preflight/boot time and read via plain copy (`f32`/`f64`) fields inside `LiveWorldConfig` / resources — zero `HashMap` lookups or `String` allocations inside simulation tick loops.
- **No Lamarck (`CLAUDE.md:97-98`):** Parameter overrides never alter `AgentBrain.learned` -> `.genotype` separation.
- **Checksums never pinned across platforms (`CLAUDE.md:41, 176-182`, `docs/decisions/ADR-0008-world-past-and-time.md:175, 283`):** `S63`/`S64` determinism tests assert identical **parameter sensitivity rankings and finite metrics** across repeated same-OS runs — never hardcoded cross-OS `world_checksum`s.
- **Single-source live schedule (`CLAUDE.md:183-185`):** Sensitivity runs using `LiveExperimentAdapter` must call `build_tick_schedule`.

## 4. Open Owner Decisions (`D-V9-1`–`D-V9-4`)

| Slot | Question | Options | Recommendation |
|---|---|---|---|
| **`D-V9-1`** | Primary sensitivity analysis method (`S63`): | **(A)** Morris Elementary Effects (`r` trajectories × `(p + 1)` points) + Sobol/Spearman rank screening across a multi-seed ensemble (`S64`).<br>**(B)** Full Saltelli–Sobol variance decomposition (higher runtime cost `N_samples * (2p + 2)` per seed).<br>**(C)** One-at-a-time (OAT) ±20% local perturbation only (misses nonlinear trophic interactions). | **(A)** Morris screening across seeds first, with optional Sobol refinement for top-ranked parameters — captures nonlinear interactions at bounded compute cost. |
| **`D-V9-2`** | Scope of constants covered by `V9`'s `ParameterRegistry` (`S62`): | **(A)** All governing vertical-slice constants in `ecology.rs`, `dynamic_fields.rs`, `V2` `SpeciesProfile`, `V3` lifecycle, `V4` decomposition, `V5` field-plant coupling, and `V6` interventions.<br>**(B)** Every numeric literal across the entire `src-tauri/` workspace (including UI timing and rendering constants). | **(A)** Every vertical-slice biophysical & demographic governing constant first (UI rendering constants live outside ecology sensitivity). |
| **`D-V9-3`** | Multi-seed sensitivity sweep budget (`S63` × `S64`): | **(A)** Configurable CI smoke design (`>= 3` seeds, small `r`) + separate release/confirmation sweep (`>= 10` seeds).<br>**(B)** Single fixed large sweep baked into default `cargo test`. | **(A)** Fast CI `S63`/`S64` test (`<= 30 s`) + heavyweight release/confirmation sweep via `anima-lab` / example runner. |
| **`D-V9-4`** | Generate a checked Markdown table from `ParameterRegistry` (like `scripts/check_ipc_reference.mjs:1-19`)? | **(A)** Yes — add a doc/code parity check or `DOC4` hook later so `docs/reference/` never drifts from `ParameterRegistry`.<br>**(B)** Keep `ParameterRegistry` purely in Rust JSON/CLI output in `V9` and wire doc generation in `DOC4`. | **(B)** Emit clean JSON/CSV + `ParameterRegistry` unit checks in `V9`; wire generated reference tables under `DOC4` to keep `V9` focused. |

## 5. Future `V9` Card Boundaries (**Proposed** `ALLOWED PATHS`)

> Marked **`proposed`** for owner authorization when scheduling card `V9`.

- `src-tauri/src/core/parameter_registry.rs` (`to be created by V9`)
- `src-tauri/src/core/sensitivity.rs` (`to be created by V9`)
- `src-tauri/src/core/mod.rs:1-20` (export `parameter_registry`, `sensitivity`)
- `src-tauri/src/core/sweeps.rs:46` (only if extending `SweepTarget` cleanly while preserving existing JSON schema backwards-compatibility)
- `src-tauri/src/bin/anima-lab.rs:95-108` (optional `sensitivity` subcommand/flags if chosen; else `src-tauri/examples/run_v9_sensitivity.rs`, `to be created by V9`)
- `src-tauri/src/core/live_experiment.rs:280-320` (read-only binding of opt-in vertical-slice parameter overrides in `LiveWorldConfig` / `LIVE_KEYS`, keeping defaults unchanged when keys absent)
- `src-tauri/tests/v9_parameter_registry_tests.rs` & `src-tauri/tests/v9_sensitivity_tests.rs` (`S62`, `S63`, `S64`, `to be created by V9`)
- `LIVE_KEYS.len()` pinning tests only if `LIVE_KEYS` changes (`+N` explicit count)

## 6. `DONE WHEN`, Verification Commands & Negative Controls (`A3 #2`)

### `DONE WHEN`
1. **`S62` (`M11.1`):** `ParameterRegistry::default()` covers all governing vertical-slice constants resolved under `D-V9-2` with valid non-empty units, finite `[min, max]` bounds, and cited physical/ecological rationale; all default values match trunk runtime behavior when overrides are unset.
2. **`S63` / `S64` (`M11.2`–`M11.3`):** Reproducible multi-seed `SensitivityReport` (`to be created by V9`) identifies dominant governing parameters (`effect_index` & rank, `to be created by V9`) deterministically across same-OS replays while preserving conservation residuals (`<= 1e-3` EU, `< 1e-4` H2O/N) across valid parameter samples.
3. **Concrete Verification Commands (all must exit `0` after final edit; Cargo runs only on host meeting `>= 5.00 GiB` RAM / CI):**
   - `cargo fmt --check`
   - `cargo clippy --all-targets --features desktop -- -D warnings`
   - `cmd /c "cargo test --features desktop > ..\cargo-test-output.txt 2>&1"` + `node scripts/check_test_targets.mjs cargo-test-output.txt --profile desktop`
   - Dedicated `v9_parameter_registry_tests` & `v9_sensitivity_tests` passing with zero `#[ignore]`
   - `npm run check:docs`, `npm run check:text-hygiene`, `npm run check:root-hygiene`, `npm run check:conflict-markers`

### Negative Controls (Must Fail Red Under Fault Injection, `A3 #2`)
1. **Missing unit or citation in `ParameterRegistry`:** Blank a `unit` or `source_citation` field (or set `default_value > max`) -> `ParameterRegistry::validate()` / `S62` test fails red.
2. **Unregistered sweep parameter:** Reference an unknown parameter key in a sweep manifest -> preflight fails red instead of silently ignoring the dimension (`WORLD_SIMULATION_PLAN.md:1122`).
3. **Single-seed / unseeded shortcut:** Pass duplicate seeds or non-finite bounds to sensitivity runner -> preflight validation fails red (`ensemble.rs:59`).

### Determinism Checks
- Running the `V9` sensitivity pipeline twice with identical `SweepManifest` and seed set on the same OS yields bit-identical parameter sample points, identical per-seed observables, and identical sensitivity-ranking scores (`std::cmp::Ordering` invariant).
