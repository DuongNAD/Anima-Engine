---
phase: planning
feature: v1-vertical-slice
title: Card V1 — Opt-In DynamicFields Integration into build_tick_schedule
description: Implementation spec for card V1 wiring DynamicFields into build_tick_schedule with opt-in gating, snapshot persistence, zero-heap-allocation tick updates, and deterministic replay
status: proposed
owner: anima-dell
last_reviewed: 2026-11-24
---

# Card `V1` — Opt-In `DynamicFields` Integration into `build_tick_schedule`

> **Design spec only (`status: proposed`).** No Rust or TypeScript code is written in D4 (`V-ADR`).
> All `file:line` anchors below are verified against `origin/feat/ecological-selection` at trunk
> commit `1a2d1d62de52334e675108852311a87c3c1bb10f`.

## 1. Goal & Scope

Promote M3's standalone `DynamicFields` (`src-tauri/src/core/dynamic_fields.rs:66`; tests `S15–S20`) into the live Bevy simulation schedule (`core::simulation_schedule::build_tick_schedule` at `src-tauri/src/core/simulation_schedule.rs:121`), gated opt-in so that:

1. Headless experiments (`live.dynamic_fields = 1.0`) and opt-in desktop sessions (`ANIMA_DYNAMIC_FIELDS`) run climate, conserved water budget, soil mineralization/nutrient budget, and erosion/turbidity in real ticks driven by declared `InterventionCommand`s (`LiveInterventions` at `src-tauri/src/core/live_experiment.rs:1011`).
2. Snapshot save/load (`SavedSimulationState` at `src-tauri/src/core/simulation_state.rs:459`) carries full dynamic-field buffers and conservation ledgers, preserving bit-exact `run(N) == run(K) -> save -> load -> run(N - K)` replay on the same OS.
3. The live tick system executes with **0 heap allocations** (`CLAUDE.md`, S17) and **0 ambiguous schedule pairs** (`schedule_ambiguity_tests.rs:201`).
4. Flag-off / absent `DynamicFields` runs remain **100% bit-identical** to trunk `1a2d1d62` (zero allocations, early return, untouched `world_checksum`).

### Explicit Non-Goals (Deferred to `V5` / `V6` / `V8`)

- **Coupling fields back into `ResourceField` grass regrowth, tree canopy/root feedback, or agent drinking/thermoregulation** (`V5`: "Soil water, nutrients, and grass biomass as fields; large trees feed back", [`2026-11-05-swarm-master-plan.md`](2026-11-05-swarm-master-plan.md) Đợt 6).
- **The four vertical-slice interventions with ecological target selection (`remove 80% wolves`, `clear 40% headwaters forest`, etc.)** (`V6`).
- **Map v2 4096² terrain wiring or R3F shader rendering** (`QD-MAPV2` / `ADR-0012`).

## 2. Current Code Anchors & Required Design Mechanics (`1a2d1d62`)

### 2.1 `DynamicFields` Resource & Zero-Allocation Intervention Borrowing (`core/dynamic_fields.rs`)

- **Make `DynamicFields` a Bevy `Resource`:** Add `#[derive(Resource)]` on `pub struct DynamicFields` (`src-tauri/src/core/dynamic_fields.rs:65-66`), matching `ResourceField` (`src-tauri/src/core/ecology.rs:330`). Validate on deserialization/restore (`snapshot.rs`) that `width > 0`, `height > 0`, `n = width * height` matches all vectors (`elevation`, `slope`, `root_cover`, `base_moisture`, `downstream`, `dam`, `air_temperature`, `precipitation`, `surface_water`, `groundwater`, `soil_moisture`, `soil_nitrogen`, `soil_organic`, `erosion`, `turbidity`, and private scratch `inflow: Vec<f32>` at `:97` — if `inflow` is serialized/restored with wrong length, `self.inflow.fill(0.0)` / `self.inflow[dest]` in `step_water` at `:354,:378` would panic or corrupt!).
- **Fix `&[&InterventionCommand]` signature allocation trap:**
  - `step_climate` (`dynamic_fields.rs:256`) and `step_soil` (`:413`) currently accept `active: &[&InterventionCommand]`.
  - In the live world, `LiveInterventions` (`src-tauri/src/core/live_experiment.rs:1011-1016`) stores `pub active: Vec<InterventionCommand>` (owned structs pre-sized by `with_capacity`, `:1020`, never reallocated).
  - Collecting `&active.active` into a fresh `Vec<&InterventionCommand>` inside a tick system would **violate the `allocs == 0` hot-loop rule**. Storing a persistent `&'static` reference in a `Resource` is invalid, and `Local<Vec<...>>` is brittle across schedules.
  - **Clean fix:** Generalize `step_climate` (`:256`) and `step_soil` (`:413`) to `pub fn step_climate<C: std::borrow::Borrow<InterventionCommand>>(&mut self, tick: u64, season_phase: f32, active: &[C])` (and same for `step_soil`), OR add zero-alloc slice helpers taking `&[InterventionCommand]` while forwarding from `&[&InterventionCommand]`. Both existing tests (`s15_*`, `s17_*`, `dynamic_fields_zero_alloc_tests.rs:56`) and the live schedule `&active.active` compile and run with **zero heap allocations**.

### 2.2 Exact Slot Inside `build_tick_schedule` (`core/simulation_schedule.rs`)

[`core::simulation_schedule::build_tick_schedule`](../../../src-tauri/src/core/simulation_schedule.rs#L121) (`src-tauri/src/core/simulation_schedule.rs:121-202`) is one total-order chain across 4 chained tuples (`F-041`) with `auto_insert_apply_deferred: false` (`:127`) and explicit `apply_deferred` sync points (`:154, :159, :176, :185, :187, :190`).

Place the new `step_dynamic_fields_system` in **tuple 1** (`:132-148`) immediately **after** `apply_live_interventions_system` (`:136`) and **before** `sensory_system` (`:137`):

```text
drain_observer_actions_system (:135)
  -> apply_live_interventions_system (:136)
  -> step_dynamic_fields_system (NEW V1 slot)
  -> sensory_system (:137)
  -> ...
```

**Why this exact slot:**
- `apply_live_interventions_system` runs directly upstream so any active `TemperatureDelta`, `RainfallDelta`, or `AddNutrient` for the current tick is visible on the same tick (`schedule_ambiguity_tests.rs:282-289`), while leaving `apply_live_interventions_system` and `LiveInterventions` (`live_experiment.rs:1011`) ready to be read immutably (`Option<Res<LiveInterventions>>`).
- `step_dynamic_fields_system` uses zero `Commands` and touches only `DynamicFields` (plus read-only `Option<Res<LiveInterventions>>` and `Option<Res<SeasonClock>>`), so it introduces **zero new `apply_deferred` ordering shifts**, **zero new ambiguous pairs** (`schedule_ambiguity_tests.rs:201`), and zero interference with the energy pipeline or `SimRng` draw order (`:244-306`).
- Update `schedule_ambiguity_tests.rs:241-307` (`declared_order_keeps_the_causal_sequences`) with `assert_in_order(&order, &["apply_live_interventions_system", "step_dynamic_fields_system", "sensory_system"])`.

### 2.3 Opt-In Activation, Tick Counter, and Default-Off Bit-Identity

- **Opt-in gating (ADR-0004 / ADR-0012 / F-043 pattern):**
  - `step_dynamic_fields_system` takes `fields: Option<ResMut<DynamicFields>>`, immediately returning if `fields` is `None` (`let Some(mut fields) = fields else { return; };`), mirroring `DormantCohorts` (`aggregate_population.rs:148`) and `LiveInterventions` (`live_experiment.rs:1008`).
  - **Experiment manifests (`src-tauri/src/core/live_experiment.rs`):** Add `pub const LIVE_KEY_DYNAMIC_FIELDS: &str = "live.dynamic_fields";` to `LIVE_KEYS` (`:280`, updating `[&str; 31]` -> `[&str; 32]`) and `#[serde(default)] pub dynamic_fields: bool` to `LiveWorldConfig` (`:316`). When `true`, `build_live_world` (`:2746`) either restores `state.dynamic_fields` from `SavedSimulationState` (`Some(state)`) or constructs `DynamicFields::from_terrain(&terrain_map)` (`None`) after `init_world_with` (`:2751`, where `TerrainMap` and `SeasonClock` already exist per `ecs.rs:86, :128`). Fail loudly (`ExperimentError`) if `config.dynamic_fields` is true during restore of a snapshot whose `state.dynamic_fields` is `None` rather than silently resetting fields from terrain!
  - **Desktop opt-in (`src-tauri/src/core/simulation_loop.rs`):** Check `ANIMA_DYNAMIC_FIELDS` (unset/`0`/`false` = off -> resource not inserted) at world construction (`~1390`, alongside `MortalityPolicy::from_env()` and `FitnessRuleSetting::from_env()`) and restore saved `state.dynamic_fields` when `Some` at restore (`~1487`, mirroring `DormantAgents` at `:1491`).
- **Tick counter & `SeasonClock` — NO `Local<u64>`:**
  - Never track step tick inside a Bevy `Local<u64>`: `ResourceField::regrowth_phase` (`src-tauri/src/core/ecology.rs:347-357`) proved that system-owned `Local<T>` resets silently to `0` on save/load and corrupts resumed trajectories!
  - Instead, store `pub field_tick: u64` (`#[serde(default)]`) directly on `DynamicFields` unconditionally — do **not** fall back to `LiveInterventions.tick`, because `LiveExperimentAdapter::run_schedule_once()` (`src-tauri/src/core/live_experiment.rs:2077-2085`) leaves `LiveInterventions.tick` at `0` (only `step()` at `:2556` updates it). Explicitly define priming-tick behavior in `SimulationEngine::start` (`src-tauri/src/core/simulation_loop.rs:1924-1925`, where `schedule.run(&mut world)` executes once on fresh genesis when `state_to_load.is_none()`): either skip advancing `field_tick` on priming (or align it with `ResourceField`'s initial pass so `field_tick` matches `tick_count`). Read `SeasonClock.phase` (`src-tauri/src/core/ecology.rs:254`, already serialized in `SavedSimulationState` at `simulation_state.rs:530` and restored in `restore_energy_state` / `SimulationEngine::start`) for `step_climate`'s seasonal term (`dynamic_fields.rs:257`).

### 2.4 Snapshot Persistence, Ledger Restoration, & Conditional `world_checksum`

- **Additive field on `SavedSimulationState` (`src-tauri/src/core/simulation_state.rs:459`):**
  ```rust
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub dynamic_fields: Option<crate::core::dynamic_fields::DynamicFields>,
  ```
  - Update `empty_saved_state_for_tests` (`:608`, set `dynamic_fields: None`) and `serialize_world_state` (`:1151`, clone `world.get_resource::<DynamicFields>().cloned()`).
  - Because `skip_serializing_if = "Option::is_none"` omits the key whenever `DynamicFields` is absent, existing schema 6/7/8 snapshots and byte payloads are unchanged.
- **Snapshot validation (`src-tauri/src/core/snapshot.rs:458`, `validate_state`):**
  - When `state.dynamic_fields` is `Some(df)`, validate: `df.width > 0`, `df.height > 0`, `df.width * df.height <= 4096 * 4096` (bounded against `MAX_SNAPSHOT_BYTES` at `:74`), all 16 vector fields (`elevation..turbidity` + `inflow`) have exact length `width * height`, `downstream` entries are either `< width * height`, `OUTLET`, or `RETAIN` (`dynamic_fields.rs:57-59`), and every float field + ledger scalar is finite (`is_finite()`).
- **`world_checksum` (`src-tauri/src/core/snapshot.rs:789`):**
  - Append `DynamicFields` cell state to `world_checksum` **only if** `world.get_resource::<DynamicFields>()` is `Some(df)` (so all flag-off runs retain bit-identical checksums).
  - Hash **all authoritative state and conservation ledgers** via `.to_bits().to_le_bytes()`: `width`, `height`, `field_tick`, `dam`, `air_temperature`, `precipitation`, `surface_water`, `groundwater`, `soil_moisture`, `soil_nitrogen`, `soil_organic`, `erosion`, `turbidity`, **and** the `f64` conservation ledgers (`initial_water`, `cum_precip`, `cum_evap`, `cum_discharge`, `cum_nutrient_added`, `cum_nutrient_leached`). Note that `src-tauri/Cargo.toml:43` already enables `serde_json`'s `float_roundtrip` feature and `src-tauri/src/core/snapshot.rs:152-154` confirms bit-exact `f64` JSON round-trips (while `snapshot.rs:845-847` already hashes `pool.detritus: f64`), so hashing authoritative `f64` ledgers detects save/load ledger corruption without breaking round-trip replay.
- **Dam / `AddNutrient` / conservation caveats (`dynamic_fields.rs`):**
  - `AddNutrient` in `LiveInterventions` flows to `DynamicFields::step_soil` when `DynamicFields` is active. Note `apply_live_interventions_system` (`live_experiment.rs:1084`) ALSO moves detritus into `ResourceField` on `ECOLOGY_PERIOD` ticks; in `V1`, `DynamicFields` tracks soil nitrogen/organic conservation independently in `total_nutrient()` / `nutrient_budget_residual()`, while `V5` connects soil nitrogen to grass growth. Document clearly that both budgets remain independently conserved in `V1` without breaking `EnergyLedger`.

## 3. Open Owner Decisions

- **`D-V1-1` (Schedule cadence inside `step_dynamic_fields_system`):**
  - **Option A (Recommended):** Gate full `DynamicFields` step (`step_climate` + `step_water` + `step_soil` + `step_erosion`) on `ECOLOGY_PERIOD` (`60` ticks = 1 Hz, `SimClock::fires(tick, ECOLOGY_PERIOD)` at `src-tauri/crates/anima-domain/src/sim_clock.rs:20, :65`). Avoids scanning 65,536 cells (`256²`) four times every single 60 Hz frame when desktop opt-in runs. *(Explicit caveat: under `ECOLOGY_PERIOD = 60`, short interventions with `duration_ticks < 60` or windows falling strictly between multiples of `60` per `InterventionCommand::active_at` at `src-tauri/crates/anima-domain/src/intervention.rs:170-173` will not fire, whereas original unit tests `s15`–`s20` at `dynamic_fields.rs:521-578` stepped consecutive ticks `1..=50` / `1..=200`. Schedule tests under Option A must align intervention windows to firing ticks; choose Option B if every-tick intervention resolution is required.)*
  - **Option B:** Step `DynamicFields` every base tick (`PHYSICS_PERIOD = 1`, matching `s16_water_budget_is_conserved` at `dynamic_fields.rs:563`), accepting 60× higher CPU cost on `256²` grids.
  - **Option C:** Multi-rate bands (`step_climate` on `CLIMATE_PERIOD = 600`, `step_soil` on `PLANT_PERIOD = 300`, `step_water`/`step_erosion` on `ECOLOGY_PERIOD = 60`). Note: if `step_climate` skips ticks, regional `TemperatureDelta`/`RainfallDelta` applied between climate ticks must be consistent with intervention start/end ticks.
- **`D-V1-2` (Opt-in `V1` observables excluded from `all_live_observable_ids()`):**
  - **Option A (Recommended):** Expose 4 opt-in observables (`live.water_budget_residual`, `live.nutrient_budget_residual`, `live.mean_soil_moisture`, `live.mean_surface_water`) in a separate constant `pub const LIVE_DYNAMIC_FIELDS_OBSERVABLE_IDS: [&str; 4]` **excluded from `LIVE_EXTENDED_OBSERVABLE_IDS` (`:1632`) and `all_live_observable_ids()` (`:1652`)** — matching AE3's opt-in observable pattern in `CLAUDE.md:163` so all existing 28-observable suites and frozen manifests stay intact. Register their metadata in `ObservableRegistry::live_default` (`src-tauri/src/core/experiment/observables.rs:345-410`), and enforce refusal when `live.dynamic_fields == false` via `LiveWorldConfig::ensure_observables_supported` (`live_experiment.rs:849`, called at `commands/simulation.rs:943`, `live_experiment.rs:3890`, `examples/e3_smoke.rs:650`, `examples/run_sel2p_experiment.rs:2153`) — plus optionally wire `ensure_observables_supported` into `experiment_runner::run_manifest_seed_inner` (`src-tauri/src/core/experiment_runner.rs:665-692`), since `LiveExperimentAdapter::from_manifest` (`live_experiment.rs:2465-2492`) does not receive `observable_ids`.
  - **Option B:** Defer `DynamicFields` observables to `V5` and verify `V1` via `DynamicFields` resource assertions in tests.

## 4. Future `V1` Coding Card Boundaries

### Allowed Paths
- `src-tauri/src/core/dynamic_fields.rs` (`Resource` derive, `field_tick`, `<C: Borrow<InterventionCommand>>` signature)
- `src-tauri/src/core/simulation_schedule.rs` (`build_tick_schedule` tuple 1 slot + `step_dynamic_fields_system`)
- `src-tauri/src/core/simulation_state.rs` (`SavedSimulationState::dynamic_fields`, `empty_saved_state_for_tests`, `serialize_world_state`)
- `src-tauri/src/core/snapshot.rs` (`validate_state`, conditional `world_checksum`)
- `src-tauri/src/core/live_experiment.rs` (`LIVE_KEY_DYNAMIC_FIELDS`, `LIVE_KEYS`, `LiveWorldConfig`, `build_live_world`, `LIVE_DYNAMIC_FIELDS_OBSERVABLE_IDS` + `ensure_observables_supported` per `D-V1-2`)
- `src-tauri/src/core/experiment/observables.rs` (`ObservableRegistry::live_default` entries when `D-V1-2 = A`)
- `src-tauri/src/core/experiment_runner.rs` (only if universal `run_manifest_seed_inner` preflight is wired for `D-V1-2 = A`)
- `src-tauri/src/core/simulation_loop.rs` (`ANIMA_DYNAMIC_FIELDS` init & restore)
- `src-tauri/tests/schedule_ambiguity_tests.rs` (assert declared position between `apply_live_interventions_system` and `sensory_system`)
- `src-tauri/tests/dynamic_fields_schedule_tests.rs` (new: S15–S20 driven through real `LiveExperimentAdapter` + `build_tick_schedule`, tracking-allocator zero-alloc test, save/load replay, flag-off bit-identity)
- `LIVE_KEYS.len()` pinning tests (`31 -> 32` count update only): `src-tauri/tests/e2_seam_tests.rs`, `eco_card_b_tests.rs`, `founder_genotype_tests.rs`, `founder_layout_tests.rs`, `grass_steering_tests.rs`, `lake_layout_tests.rs`, `land_mask_tests.rs`, `strict_prey_cap_tests.rs`
- Preregistration manifest suites (`LIVE_KEYS` partition): `src-tauri/tests/prereg_e3_manifest_tests.rs` + `src-tauri/tests/fixtures/experiments_e3/e3-preregistration.json` (`default_keys_left_absent` zero-default amendment), and `src-tauri/tests/prereg_e3b_manifest_tests.rs` (+ `src-tauri/tests/fixtures/experiments_e3b/e3b-preregistration.json` `default_keys_left_absent` zero-default amendment if E3b freeze allows, or freeze-at-31 key scoping matching `V0`'s `D-V0-2`)
- `docs/ai/planning/` (implementation result note)

## 5. DONE WHEN, Negative Controls & Determinism Checks

### DONE WHEN
1. **Default-off bit-identity:** Existing E2/E3b control seeds with `live.dynamic_fields` absent/`0.0` yield **bit-exact** `world_checksum`, closed EU, and observables vs trunk `1a2d1d62` on the same OS.
2. **Schedule integration (`F-041`):** `step_dynamic_fields_system` executes inside `core::simulation_schedule::build_tick_schedule`; `schedule_ambiguity_tests.rs` (`the_tick_schedule_has_no_ambiguous_pair_at_all` & `declared_order_keeps_the_causal_sequences`) and `schedule_total_order_tests.rs` pass with `0` ambiguous pairs.
3. **S15–S20 inside real schedule (`dynamic_fields_schedule_tests.rs`):** Running `LiveExperimentAdapter` with `live.dynamic_fields = 1.0` verifies `TemperatureDelta` / `RainfallDelta` confinement (`S15`), water-budget conservation (`S16`, `|residual| / scale < 1e-4`), `AddNutrient` saturation/leaching conservation (`S17`), erosion/turbidity update (`S18`), and checkpoint `run(N) == run(K) -> save -> load -> run(N - K)` (`S20`) preserving exact water/nutrient budgets and `field_tick`.
4. **Zero-alloc hot loop (`S17`, `CLAUDE.md`):** Process-global `TrackingAllocator` across **active firing ticks** of `build_tick_schedule` (e.g., `tick % ECOLOGY_PERIOD == 0` under `D-V1-1 = A` — not solely early-return ticks) **with active `LiveInterventions`** (`TemperatureDelta`, `RainfallDelta`, `AddNutrient`) records `0` heap allocations per firing tick.
5. **Preflight safety:** Restoring a snapshot lacking `dynamic_fields` (`None`) when `live.dynamic_fields = 1.0` fails loudly (`Err`) rather than regenerating fresh fields; corrupted field vector lengths or out-of-range `downstream` indices fail `snapshot::from_bytes` validation cleanly.
6. **Workspace gates green under `CARGO_BUILD_JOBS=2`:** `cargo fmt --check`, `cargo clippy --features desktop --all-targets -- -D warnings`, full `cargo test --features desktop` + `node scripts/check_test_targets.mjs <capture> --profile desktop` exit `0`, plus `npm run check:docs` green.

### Negative Controls (Must Fail Red Under Fault Injection, A3 #2)
1. **Missing schedule registration:** Temporarily remove `step_dynamic_fields_system` from `build_tick_schedule` -> live water/soil conservation & S15 confinement tests fail red (cells remain unperturbed).
2. **Vec allocation in system:** Temporarily collect `&active.active` into a fresh `Vec<&InterventionCommand>` inside `step_dynamic_fields_system` -> `TrackingAllocator` test fails red (`alloc_count > 0`).
3. **Dropped tick state (`Local<u64>`):** Temporarily track climate tick in `Local<u64>` instead of serialized `field_tick` and save/resume across a ramped/seasonal intervention -> `run(N) == run(K) -> save -> load -> run(N - K)` diverges and fails red.
4. **Default leakage:** Temporarily insert `DynamicFields` when `live.dynamic_fields` is absent (`false`) -> flag-off bit-identity checksum test fails red.
5. **Checksum omission:** Omit `DynamicFields` arrays from `world_checksum` and perturb `surface_water[0]` after save -> checkpoint divergence test fails red.

### Determinism Checks
- **Same-OS cross-process determinism:** Note `dynamic_fields.rs:17-19` documents that `f32::sin` in `step_climate` is platform-`libm`-dependent across OS families; therefore tests assert **same-OS multi-process bit-exact checksum equality** across 3 independent processes with `live.dynamic_fields = 1.0`, never hardcoded cross-OS checksum literals (`AGENT_RULES` / Wave Rule A2).
- **Save/load & fork determinism:** `run(N) == run(K) -> save -> load -> run(N - K)` matches bit-for-bit on `world_checksum` and every `f32` cell buffer on the same OS.
