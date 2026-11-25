---
phase: planning
feature: v1-vertical-slice
title: Card V4 — Carcasses & Temperature/Moisture Decomposition With Bit-Exact EU Ledger
description: Implementation spec for card V4 routing agent deaths into a zero-alloc CarcassPool with temperature/moisture-dependent decomposition and bit-exact closed-EU conservation
status: proposed
owner: anima-dell
last_reviewed: 2026-11-24
---

# Card `V4` — Carcasses & Decomposition With Bit-Exact EU Ledger (`M5.6`, `M6.3`)

> **Design spec only (`status: proposed`).** No Rust or TypeScript code is written in D7 (`V-SPECS`).
> All `file:line` anchors below are verified against `origin/feat/ecological-selection` at trunk
> commit `1a2d1d62de52334e675108852311a87c3c1bb10f`.

## 1. Goal, Dependencies & Scope

Introduce discrete carcasses and lagged temperature/moisture-dependent decomposition (`WORLD_SIMULATION_PLAN.md` §5.6, §7 `M5.6` / `M6.3`, §9; `2026-11-05-swarm-master-plan.md` Đợt 6) without introducing any energy leak or floating-point drift in the closed-EU ledger:

1. Replace instantaneous death `energy -> detritus` banking inside `ReclaimAndDespawnAgentCommand::apply` (`src-tauri/src/core/agent_systems.rs:141-191`) — when `CarcassPolicy` is enabled — with an atomic transfer of the corpse's exact `f64` measured reserve into a bounded, zero-alloc **`CarcassPool`** holding spatial carcass records at the death location.
2. Add a schedule-ordered `carcass_decomposition_system` that decays carcasses at a rate driven by local temperature and moisture (reading `DynamicFields` from `V1` when active, or static `TerrainMap` fallback), settling exact `f64` released EU into `EcosystemBiomass::detritus` (`EnergyFluxMatrix::mortality_to_detritus` at `src-tauri/crates/anima-domain/src/energy.rs:98`), optionally mineralized alongside soil organic matter when coupled.
3. Preserve **exact closed-EU conservation (`|residual| <= RESIDUAL_ABS_TOLERANCE_EU = 1e-3`, `src-tauri/crates/anima-domain/src/energy.rs:123`)** across death, partial decomposition, save/load, and full decay, with zero double-count windows (`ecosystem_census_system` at `src-tauri/src/core/environmental_systems.rs:178-229`) and **100% bit-identical** trunk behavior when `live.carcasses` / `ANIMA_CARCASSES` is off.

### Exact Dependency Order

- **Depends directly on:**
  - **[Card `V3`](2026-11-24-v3-lifecycle-spec.md)** (mortality from starvation, dehydration, and biological senescence drives natural carcass supply through `ReclaimAndDespawnAgentCommand`).
  - Optionally pairs with **`V1`** (`docs/ai/planning/2026-11-24-v1-dynamic-fields-schedule-spec.md`, PR `#235`) — `V4` reads `DynamicFields::{air_temperature, soil_moisture, soil_organic}` when present and falls back cleanly to `TerrainMap::{temperatures, moistures}` when `DynamicFields` is off, so `V4` compiles and tests independently of runtime field activation.
- **Unblocks:**
  - **[Card `V6`](2026-11-24-v6-interventions-causeid-spec.md)** (complete nutrient/detritus return loops under drought, deforestation, and predator removal; depends on `V4` and `V5`).
  - Future scavenger/detritivore feeding (`M6.2`) using `CarcassPool` spatial queries + `V2`'s `DietSpec::eats_carcass`.

### Explicit Non-Goals

- **No fabrication of structural-mass EU on death:** `ReclaimAndDespawnAgentCommand` (`src-tauri/src/core/agent_systems.rs:135-191`) reclaims **strictly** `debit_reserve(&mut homeo.energy, homeo.energy)` (`:159-165`, `f64`), never adding synthetic EU derived from `Segment::mass` — unless genesis/maintenance is redesigned globally, creating mass EU on death would immediately break `closed_total_eu` (`src-tauri/crates/anima-domain/src/energy.rs:375`)!
- **No full scavenger/detritivore animal guild or soil nutrient coupling loop yet:** carcass scavenging hooks (`DietSpec::eats_carcass`) may consume from `CarcassPool` if `D-V4` / `M6.2` extends `V4`, while soil/grass plant feedback lives in [Card `V5`](2026-11-24-v5-fields-plants-feedback-spec.md).

## 2. Current Code Anchors (`1a2d1d62`) & Required Design Mechanics

### 2.1 Existing Death, EU Ledger & Census Anchors

- **Sole atomic death primitive:** `ReclaimAndDespawnAgentCommand::apply` (`src-tauri/src/core/agent_systems.rs:141-191`) zeroes `HomeostaticState.energy` via `debit_reserve` (`:159-165`, `:162`), adds `reclaimed` directly into `pool.detritus` (`:166-172`, `:170`), and despawns the root + segment tree (`:179-189`) at an `apply_deferred` sync point — explicitly eliminating any double-count window (`:130-134`). Other callers queuing it live at `agent_systems.rs:683, 1451` and `live_experiment.rs:1266`.
- **Closed-EU definition:** `closed_total_eu(plant_biomass, animal_energy, detritus, dormant)` (`src-tauri/crates/anima-domain/src/energy.rs:375-377`), measured inside `ecosystem_census_system` (`src-tauri/src/core/environmental_systems.rs:209-227`) using authoritative stores (`pool.plants`, summed `HomeostaticState.energy` + `DormantCohorts::total_energy()`, `pool.detritus`, `pool.dormant`). Notice the critical F1-1 precedent at `:194-196` and `:357-359`: `ecosystem_census_system` reads `DormantCohorts::total_energy()` directly so dormant EU is never double-counted or dropped!
- **Flux reporting:** `EnergyFluxMatrix` (`src-tauri/crates/anima-domain/src/energy.rs:88-105`) defines `mortality_to_detritus: f64` (`:98`), read by `energy_flux_aggregation_system` (`environmental_systems.rs:426-442`) -> `generate_energy_flux_report` (`src-tauri/src/core/telemetry_export.rs:331`). Note `pool.detritus += respired` / carcass deposits must maintain bit-exact complementarity (`source_before - source_after == dest_after - dest_before`).
- **Schedule sync points:** `build_tick_schedule` (`src-tauri/src/core/simulation_schedule.rs:154-195`) already flushes `mortality_system` commands at `apply_deferred` (`:185`), runs `autonomous_reproduction_system -> apply_deferred` (`:186-187`), and runs `ecosystem_census_system` at `:191`. Tuple 2 runs `resource_field_regrowth_system` (`:155`).

### 2.2 Zero-Alloc `CarcassPool` Resource (`core/carcass.rs`)

Storing every carcass as a Bevy ECS entity with dynamic `Commands::spawn` inside a tick loop risks archetype churn and violates the zero-allocation hot-loop rule (`src-tauri/tests/birth_death_zero_alloc_tests.rs:308`). Therefore:

1. **Data layout (`src-tauri/src/core/carcass.rs`, `pub mod carcass;` in `core/mod.rs`):**
   - `pub struct CarcassRecord`:
     - `pub id: u64` (monotonically assigned from `next_id: u64`, serialized in `CarcassPool` — deterministic across runs/replays)
     - `pub pos_x: f32`, `pub pos_z: f32` (death world coordinates, wrapped matching `MapBounds`)
     - `pub species_id: Option<SpeciesId>` (from `AgentSpecies` when `V2` active)
     - `pub initial_eu: f64` (exact `reclaimed` value at death)
     - `pub remaining_eu: f64` (exact undecayed EU remaining in the carcass; **strictly `f64`** so micro-decays never suffer `f32` rounding ratchet!)
     - `pub created_tick: u64`
   - `#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)] pub struct CarcassPool`:
     - `pub records: Vec<CarcassRecord>` (pre-allocated once via `Vec::with_capacity(max_carcasses)` at genesis/restore — never reallocated in ticks!)
     - `pub max_carcasses: usize` (e.g., default `1024`)
     - `pub next_id: u64`
     - `pub active_eu_sum: f64` (or exact recomputed sum over ordered `records` — strictly `records.len() <= 1024`, so iterating in fixed index order is `< 10 µs` and bit-exact with `total_eu()`!)
2. **Bounded Overflow Invariant (Bit-Exact When Pool Is Full):**
   - When `ReclaimAndDespawnAgentCommand::apply` tries to push a new `CarcassRecord` (`reclaimed > 0.0`, excluding zero-reserve deaths) and `records.len() >= max_carcasses`: settle the oldest record `records[0]`'s `remaining_eu` into `pool.detritus` (and `EnergyFluxMatrix::mortality_to_detritus`) or settle the incoming `reclaimed` directly into `pool.detritus`, rather than growing `Vec` past pre-allocated capacity! Never allocate on the heap during live ticks.
   - Wait: verify ordering — popping index `0` shifts elements in place without reallocating capacity; even simpler and `O(1)`: settle the new `reclaimed` directly to `pool.detritus` when at `max_carcasses` (with an explicit `overflow_to_detritus_count: u64` counter for soak telemetry). Zero reallocation, zero shift, bit-exact closed EU!
3. **Atomic Handoff in `ReclaimAndDespawnAgentCommand::apply` (`src-tauri/src/core/agent_systems.rs:159-191`):**
   - Immediately after `let reclaimed = debit_reserve(&mut homeo.energy, amount)` (`:159-165`, `:162`), route through a shared `reclaim_and_despawn_inner(world, root, leave_carcass)`:
     - Keep `ReclaimAndDespawnAgentCommand { pub root: Entity }` 1-field (with `leave_carcass = true`, preserving all 6 existing call sites in `src-tauri/tests/birth_death_zero_alloc_tests.rs:209, 225, 242, 393, 411, 635`, both calls in `src-tauri/tests/htc_dual_mortality_starvation_tests.rs:185-186`, and `agent_systems.rs:683, 1451`), and expose a sibling `ReclaimAndDespawnNoCarcassCommand { pub root: Entity }` (`leave_carcass = false`) for `RemovePredators` culling (`live_experiment.rs:1266`, per `D-V4-3`). *(Alternative: add `pub leave_carcass: bool` directly to `ReclaimAndDespawnAgentCommand` and update those 8 test/production struct literals explicitly listed in §4.)*
     - When `CarcassPolicy` is present and `enabled` **and** `leave_carcass == true`: push `CarcassRecord { remaining_eu: reclaimed, ... }` into `CarcassPool` (routing to `pool.detritus` only if `reclaimed <= 0` guard or pool full). Do **not** credit `pool.detritus` for EU deposited in `CarcassPool`!
     - Otherwise (flag off, pool absent, or `leave_carcass == false`): execute existing `pool.detritus += reclaimed;` (`:170`) unchanged.
   - Because reserve debit and carcass push occur atomically at the `apply_deferred` sync point (`simulation_schedule.rs:185` / `:187` / `:190`), no observer system between ticks can ever see EU in both `Animals` and `Carcasses` or in neither!
4. **Closed-EU Accounting (See Open Decision `D-V4-1`):**
   - Every EU inside `CarcassPool::records` MUST be counted in `closed_total_eu` inside `ecosystem_census_system` (`environmental_systems.rs:210`) — **either** as a dedicated 5th `EcosystemBiomass::carcasses: f64` compartment (`D-V4-1 = A`), **or** rolled cleanly into `detritus`/auxiliary tracked field audited at `census` (`D-V4-1 = B`, preserving the 4-field `EcosystemBiomass` struct). Option A is cleaner scientifically because undecayed carcass biomass is **not yet mineralized** into plant-available detritus/regrowth budget (`ResourceField::step_regrowth_gated_strided` at `environmental_systems.rs:75` only reads `pool.detritus`, never undecayed `carcasses`!). Keep the existing 4-argument `closed_total_eu` (`src-tauri/crates/anima-domain/src/energy.rs:375`) untouched for legacy callers and add `closed_total_eu_with_carcasses(..., carcasses)` for `ecosystem_census_system` and `live_observables` (`live_experiment.rs:1864`).
5. **Decomposition Law & Schedule Placement (`carcass_decomposition_system`):**
   - Place `carcass_decomposition_system` in `build_tick_schedule` **tuple 2** immediately before `resource_field_regrowth_system` (`simulation_schedule.rs:155`): zero `Commands`, zero new `apply_deferred`, runs on `PLANT_PERIOD = 300` (`src-tauri/crates/anima-domain/src/sim_clock.rs:21`, or `ECOLOGY_PERIOD` per `D-V4-2`), returning mineralized EU into `pool.detritus` right before plant regrowth can consume it (`WORLD_SIMULATION_PLAN.md` §4.4 step 4/5!).
   - Also keep `ecosystem_census_system` (`:191`) after all deaths (`:184-185`) so first-tick death -> carcass deposit is captured in the exact same tick's census.
   - **Temperature & Moisture Rate Function (`M6.3` / `S35`):**
     - Map `(record.pos_x, record.pos_z)` via `Res<MapBounds>` + `sim_rules::world_xz_to_cell` (`src-tauri/src/core/sim_rules.rs:123`, matching `ResourceField::cell_index` at `src-tauri/src/core/ecology.rs:498`) into grid cell index (since `DynamicFields` stores `width`/`height` only), and sample normalized local `(temp, moisture)`:
       - From `DynamicFields::{air_temperature, soil_moisture}` when present (`V1`), or `TerrainMap::{temperatures, moistures}` fallback when `DynamicFields` is absent.
       - Q10 / logistic moisture curve: `effective_rate = base_decay_rate * q10_temp_factor(temp) * moisture_factor(moisture) * step_dt`.
       - Warm + moist habitats (e.g., Rainforest / summer) decompose carcasses markedly faster than cold/dry habitats (Desert / Snow / winter)!
       - Exact EU transfer: `released = (record.remaining_eu * (1.0 - (-effective_rate).exp())).min(record.remaining_eu)` (snap `record.remaining_eu <= EPS` cleanly to consume exact remainder into `released` so tiny `1e-12` zombie records vanish cleanly!). Deposit `released` into `pool.detritus` and record into `EnergyFluxMatrix::mortality_to_detritus`.
     - **Compaction order invariance:** Compact fully decayed records (`remaining_eu == 0.0`) via `pool.records.retain(|r| r.remaining_eu > 0.0)`, which shrinks length in-place with **0 heap allocations** while strictly preserving ascending `id` order across every run and checkpoint!
6. **Optional Soil-Organic Coupling (`DynamicFields` Present):**
   - When `DynamicFields` is active (`V1` / `V5`), simultaneously credit a declared `CARCASS_EU_TO_ORGANIC_NM * released` nutrient flux into `DynamicFields.soil_organic[cell]` while recording the exact added `f32` diff (`after - before`) inside `DynamicFields.cum_nutrient_added` (`dynamic_fields.rs:104, 463`) — keeping **`DynamicFields::nutrient_budget_residual()` conserved** alongside `EnergyLedger`!
7. **Persistence & Conditional `world_checksum`:**
   - Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub carcass_pool: Option<CarcassPool>` to `SavedSimulationState` (`simulation_state.rs:459`).
   - Validate in `snapshot::validate_state` (`snapshot.rs:458`): `records.len() <= max_carcasses <= MAX_CARCASSES`, strictly increasing `id < next_id`, finite coordinates & `remaining_eu > 0.0`, no `NaN`.
   - In `world_checksum` (`snapshot.rs:789`): **only when** `CarcassPolicy.enabled` (`CarcassPool` resource exists), sort `(id, pos_x.to_bits(), pos_z.to_bits(), remaining_eu.to_bits())` or hash ordered `records` + `next_id` so checkpoint save/load verifies exact carcass state without affecting flag-off checksums.

## 3. Open Owner Decisions (`D-V4-1`–`D-V4-3`)

- **`D-V4-1` (`EcosystemBiomass` compartment extension vs census folding):** — **DECIDED (2026-11-24, QD-OPENQ): Option A** (`EcosystemBiomass::carcasses` + `Compartment::Carcass`).
  - **Option A (Recommended):** Add `#[serde(default)] pub carcasses: f64` on `EcosystemBiomass` (`src-tauri/crates/anima-domain/src/energy.rs:19`) and `Compartment::Carcass` (`:49`), add a sibling `closed_total_eu_with_carcasses(..., carcasses)` helper (keeping 4-argument `closed_total_eu` at `:375` unchanged for legacy callers), handle `carcasses` cleanly in `generate_energy_flux_report` (`src-tauri/src/core/telemetry_export.rs:331-404`, preserving 3 nodes when `carcasses == 0.0` so legacy report assertions hold), and update `e3_smoke.rs:768` + the `15` `src-tauri/tests/` `EcosystemBiomass { ... }` literals cleanly with `carcasses: 0.0`. Keeps carcass biomass clearly separated from mineralized `detritus` available for plant regrowth (preventing instant dead-body -> grass conversion!).
  - **Option B:** Keep `EcosystemBiomass` 4-field unchanged (`15` test files + `e3_smoke.rs` untouched) and fold `CarcassPool::total_eu()` into a separate ledger observation (`observe_carcasses`) or temporary `pool.detritus` offset inside `ecosystem_census_system`. Less clear in telemetry exports (`EnergyFluxReport`) and mixes un-decomposed corpses into mineralized detritus capacity.
- **`D-V4-2` (Decomposition schedule cadence):** — **DECIDED (2026-11-24, QD-OPENQ): Option A** (`PLANT_PERIOD = 300`).
  - **Option A (Recommended):** Step decomposition on `PLANT_PERIOD = 300` (`0.2 Hz`, matching `src-tauri/crates/anima-domain/src/sim_clock.rs:21`'s explicit comment `"// 0.2 Hz — plant growth, decomposition"`!).
  - **Option B:** Step on `ECOLOGY_PERIOD = 60` (`1 Hz`) for finer-grained carcass decay curves over short `< 5s` test runs.
- **`D-V4-3` (Does `RemovePredators` culling intervention leave carcasses?):** — **DECIDED (2026-11-24, QD-OPENQ): Option A** (`RemovePredators` culling leaves no carcasses; natural mortality leaves carcasses).
  - **Option A (Recommended):** `RemovePredators` culling (`V6`) removes targeted predators **without** leaving decaying carcasses in the world (`leave_carcass = false`, via `ReclaimAndDespawnNoCarcassCommand` at `live_experiment.rs:1266` returning debited reserve directly to `detritus`) so the V6 wolf-removal experiment tests trophic release cleanly without a confounding carcass fertilization spike at the moment of culling; natural `mortality_system` deaths use `ReclaimAndDespawnAgentCommand` (`leave_carcass = true`).
  - **Option B:** All `ReclaimAndDespawnAgentCommand` calls leave carcasses indiscriminately whenever `CarcassPolicy.enabled`.

## 4. Future `V4` Coding Card Boundaries

### Allowed Paths
- `src-tauri/crates/anima-domain/src/energy.rs` (if `D-V4-1 = A`: `EcosystemBiomass::carcasses`, `Compartment::Carcass`, `closed_total_eu_with_carcasses`)
- `src-tauri/src/core/mod.rs`, `src-tauri/src/core/carcass.rs` (new: `CarcassRecord`, `CarcassPool`, `CarcassPolicy`, decomposition math & `carcass_decomposition_system`)
- `src-tauri/src/core/ecs.rs` (`EcosystemBiomass` literal at `:117`), `src-tauri/src/core/energy_ledger.rs` (`:61`, `Compartment::Carcass` handling), `src-tauri/src/core/sim_rules.rs` (`:323`)
- `src-tauri/src/core/ecology.rs` (unit test `EcosystemBiomass` literals `:841, 874` if `D-V4-1 = A`)
- `src-tauri/src/core/telemetry_export.rs` (`generate_energy_flux_report` at `:331-404` if `D-V4-1 = A`)
- `src-tauri/src/core/agent_systems.rs` (`ReclaimAndDespawnAgentCommand`, `ReclaimAndDespawnNoCarcassCommand`, `:683, 1451` if struct modified)
- `src-tauri/src/core/mortality.rs` (`mortality_system` death call with `leave_carcass = true`)
- `src-tauri/src/core/environmental_systems.rs` (`ecosystem_census_system` summing `CarcassPool` + `closed_total_eu_with_carcasses` + `energy_flux_aggregation_system`)
- `src-tauri/src/core/simulation_schedule.rs` (`build_tick_schedule` tuple 2 `carcass_decomposition_system`)
- `src-tauri/src/core/simulation_state.rs` (`SavedSimulationState::carcass_pool`, `empty_saved_state_for_tests`, `serialize_world_state`)
- `src-tauri/src/core/snapshot.rs` (`validate_state`, conditional `world_checksum`)
- `src-tauri/src/core/simulation_loop.rs` (`ANIMA_CARCASSES` init & restore)
- `src-tauri/src/core/live_experiment.rs` (`LIVE_KEY_CARCASSES`, `LIVE_KEYS`, `LiveWorldConfig`, `build_live_world`, `RemovePredators` culling at `:1266`, `live_observables` at `:1864`, optional `LIVE_CARCASS_OBSERVABLE_IDS`)
- `src-tauri/src/core/experiment/observables.rs` (optional carcass observables excluded from `all_live_observable_ids()`)
- `src-tauri/tests/schedule_ambiguity_tests.rs` (assert `carcass_decomposition_system` ordered before `resource_field_regrowth_system`)
- `src-tauri/tests/carcass_decomposition_tests.rs` (new: S32/S35 rates, closed-EU exactness across partial + full decay, bounded pool overflow, save/load, zero-alloc `TrackingAllocator`)
- `LIVE_KEYS.len()` pinning tests (`+1` count) + prereg manifests (`e2_seam_tests.rs`, `eco_card_b_tests.rs`, `founder_genotype_tests.rs`, `founder_layout_tests.rs`, `grass_steering_tests.rs`, `lake_layout_tests.rs`, `land_mask_tests.rs`, `strict_prey_cap_tests.rs`, `prereg_e3_manifest_tests.rs` + fixture JSON, `prereg_e3b_manifest_tests.rs` + fixture JSON per freeze policy)
- If `D-V4-1 = A`, exhaustive `EcosystemBiomass {` struct-literal updates (strictly adding `carcasses: 0.0`, zero assertion changes) across `src-tauri/examples/e3_smoke.rs:768` and all `15` test files at `1a2d1d62`:
  `src-tauri/tests/adversarial_demographics_energy_flux_tests.rs` (`:88, 405`), `src-tauri/tests/adversarial_ecology_stress_tests.rs` (`:514, 550`), `src-tauri/tests/aggregate_population_tests.rs` (`:127`), `src-tauri/tests/brain_cost_and_coupling_tests.rs` (`:43`), `src-tauri/tests/dual_mortality_grace_tests.rs` (`:92`), `src-tauri/tests/e2e_scientific_suite.rs` (`:71`), `src-tauri/tests/ecology_closed_ledger_integration_tests.rs` (`:206`), `src-tauri/tests/ecology_field_stride_tests.rs` (`:57`), `src-tauri/tests/empirical_m2_challenger_tests.rs` (`:654, 839, 922`), `src-tauri/tests/f044_shared_bottom_counter_tests.rs` (`:77`), `src-tauri/tests/htc_dual_mortality_starvation_tests.rs` (`:78`), `src-tauri/tests/mte_and_functional_response_tests.rs` (`:215`), `src-tauri/tests/production_ecology_e2e_tests.rs` (`:852, 895`), `src-tauri/tests/telemetry_export_tests.rs` (`:143, 202, 229`), `src-tauri/tests/tier5_backend_adversarial_tests.rs` (`:300, 331, 425, 456`)
- If `ReclaimAndDespawnAgentCommand` adds a `leave_carcass` field directly (instead of sibling `ReclaimAndDespawnNoCarcassCommand`): `src-tauri/tests/birth_death_zero_alloc_tests.rs` (`:209, 225, 242, 393, 411, 635`) and `src-tauri/tests/htc_dual_mortality_starvation_tests.rs` (`:185, 186`)
- `SavedSimulationState::carcass_pool: None` is supplied via `empty_saved_state_for_tests()` (`src-tauri/src/core/simulation_state.rs:608`, spread by `environmental_elements_stress_tests.rs`, `environmental_elements_tests.rs`, `persistence_stress_tests.rs`, `persistence_tests.rs`)
- `docs/ai/planning/` (card `V4` implementation report note)

## 5. `DONE WHEN`, Negative Controls, Conservation & Determinism Checks

### `DONE WHEN`
1. **`S32` (`M5.6`) Bit-exact death -> carcass -> detritus closed EU:** Dying with reserve `E` moves exact `E` into `CarcassPool` with `0` EU entering `detritus` until decomposition fires, `closed_total_eu` stays invariant on the death tick (`no double-count window`), and `|closed_eu_residual| <= 1e-9` across birth, feeding, metabolism, death, partial carcass decay, full carcass exhaustion, and pool overflow.
2. **`S35` (`M6.3`) Temperature & moisture delay:** Identical carcass cohorts placed in warm/moist cells decay statistically and monotonically faster (`remaining_eu(t)` smaller at fixed `t`) than cohorts placed in cold/dry cells, returning mineralized EU to `detritus` with lag rather than instantaneously.
3. **Zero heap allocations (`CLAUDE.md`):** Process-global `TrackingAllocator` across active decomposition ticks including death creation, decay updates, and record eviction/compaction measures `0` heap allocations per tick.
4. **Save/load & flag-off parity:** Checkpoint `run(N) == run(K) -> save -> load -> run(N - K)` preserves `CarcassPool` (`records`, `next_id`, `remaining_eu`) and matches `world_checksum` bit-for-bit; flag-off (`live.carcasses` absent/`0`) matches pre-`V4` trunk bit-for-bit.
5. **Workspace gates green under `CARGO_BUILD_JOBS=2`:** `cargo fmt --check`, `cargo clippy --features desktop --all-targets -- -D warnings`, `cargo test --features desktop` + `check_test_targets.mjs --profile desktop`, `schedule_ambiguity_tests.rs`, and `npm run check:docs` all exit `0`.

### Negative Controls (Must Fail Red Under Fault Injection, A3 #2)
1. **Double-count leak:** Temporarily keep `pool.detritus += reclaimed;` alongside pushing `reclaimed` into `CarcassPool` inside `ReclaimAndDespawnAgentCommand::apply` -> closed-EU conservation test fails red immediately (`residual == reclaimed`).
2. **Zombie-carcass leak:** Temporarily skip adding the remainder when snapping tiny `remaining_eu <= EPS` to `0.0` during compaction -> closed-EU test detects negative residual and fails red.
3. **Climate insensitivity:** Temporarily ignore `temp` and `moisture` in `effective_rate` -> `S35` warm/moist vs cold/dry divergence test fails red.
4. **Checksum omission:** Omit `carcass_pool` from `world_checksum` when `CarcassPolicy.enabled` -> mutated-carcass checkpoint test fails red.

### Determinism Checks
- **Same-OS three-process determinism:** With `live.carcasses = 1.0`, 3 independent processes yield bit-identical `world_checksum` and `CarcassPool` totals across deaths and decays.
- **Archetype-order immunity:** `ReclaimAndDespawnAgentCommand` flushes in deterministic sorted order (`mortality_system` entity order from `apply_deferred`), and `CarcassPool.records` iterates in strict ascending `id` order.
