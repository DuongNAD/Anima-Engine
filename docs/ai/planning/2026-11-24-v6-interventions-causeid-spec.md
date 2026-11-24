---
phase: planning
feature: v1-vertical-slice
title: Card V6 — Four Vertical-Slice Interventions With CausalLedger Chains
description: Implementation spec for card V6 routing the four v1.0 vertical-slice interventions (30-day drought, remove 80% wolves, clear 40% headwaters forest, fertilize around lakes) through field/ecological mechanisms with multi-hop CauseId provenance
status: proposed
owner: anima-dell
last_reviewed: 2026-11-24
---

# Card `V6` — Four Vertical-Slice Interventions & `CauseId` Provenance (`M8.1`, `M8.5`)

> **Design spec only (`status: proposed`).** No Rust or TypeScript code is written in D7 (`V-SPECS`).
> All `file:line` anchors below are verified against `origin/feat/ecological-selection` at trunk
> commit `1a2d1d62de52334e675108852311a87c3c1bb10f`.

## 1. Goal, Dependencies & Scope

Wire the four v1.0 vertical-slice interventions (`WORLD_SIMULATION_PLAN.md` §2.2, §4.1–§4.2, §6, §9 `1048-1063`, `docs/planning/COMPLETION_ROADMAP_2026-10-16.md:304`, and `2026-11-05-swarm-master-plan.md` Đợt 6 `V6`) into the live engine strictly through physical/ecological field mechanisms (`"no magic effects"`) with verifiable multi-hop **`CausalLedger`** mechanism chains:

1. **30 simulated days of drought** (`InterventionKind::RainfallDelta`, `signed_negative: true`): propagates spatially through precipitation -> soil moisture (`V1`) -> grass NPP limitation & wilt (`V5`) -> rabbit/wolf starvation (`V3`) without directly culling animals or bypassing spatial fields.
2. **Remove 80% of wolves** (`InterventionKind::RemovePredators`, `intensity: 0.8`): deterministically culls `floor(0.8 * count)` target-region `SPECIES_WOLF` agents once upon trigger -> lowers predation -> rabbits surge -> grass overgrazing trophic cascade (`WORLD_SIMULATION_PLAN.md` §9).
3. **Clear 40% of headwaters forest** (`InterventionKind::Deforest`, `intensity: 0.4` over headwaters region): removes 40% of living `Tree` entities + understory biomass cleanly (`plants -> detritus`) -> `V5` reduces `canopy_cover` & `root_cover` -> `step_water` cuts infiltration & raises runoff -> `step_erosion` raises erosion & downstream `turbidity` (`WORLD_SIMULATION_PLAN.md` §4.2 `385-392`, §6 `679`).
4. **Fertilize nutrients around lakes** (`InterventionKind::AddNutrient` over lake riparian buffers): injects nitrogen strictly through `DynamicFields::step_soil` (`soil_nitrogen`, tracked in `cum_nutrient_added` / `cum_nutrient_leached`) -> `V5` grass growth surge -> herbivore growth **plus** runoff/leaching lake nutrient enrichment & simple eutrophication (`WORLD_SIMULATION_PLAN.md` §6 `683`, §9 `1060`), removing the legacy detritus-to-grass EU teleport (`live_experiment.rs:1221-1250`).

### Exact Dependency Order

- **Depends directly on:**
  - **[Card `V4`](2026-11-24-v4-carcasses-decomposition-spec.md)** (and transitively `V0`, **[`V2`](2026-11-24-v2-species-profile-spec.md)**, **[`V3`](2026-11-24-v3-lifecycle-spec.md)**) — species-tagged wolves, realistic demography/mortality under drought/enrichment, and closed-EU carcass nutrient buffering.
  - **[Card `V5`](2026-11-24-v5-fields-plants-feedback-spec.md)** (and transitively `V1`) — spatial water, soil nitrogen, grass biomass limitation, canopy/root feedback, erosion/turbidity.
- **Unblocks:** `V7` (UI causal timeline & control vs treatment charts), `V8` (v1.0 vertical-slice multi-seed acceptance scenarios reporting all 7 observations in `WORLD_SIMULATION_PLAN.md` §9), and `V9–V11`.

### Explicit Non-Goals

- **No UI timeline / charts (`V7`) or multi-seed scientific acceptance sweep (`V8`):** `V6` builds and tests the deterministic engine mechanisms, scenario builders, conservation invariants, and multi-hop `CausalLedger` provenance traces; `V8` runs the multi-seed ecological calibration.
- **No direct population-subtraction cheat:** drought and nutrient interventions MUST NEVER mutate animal counts or reserves directly (`WORLD_SIMULATION_PLAN.md` §2.2 `:131-132`, `docs/planning/COMPLETION_ROADMAP_2026-10-16.md:304`).

## 2. Current Code Anchors (`1a2d1d62`) & Required Replacements

Auditing `apply_live_interventions_system` (`src-tauri/src/core/live_experiment.rs:1084-1409`) against `V6` reveals five concrete trunk limitations when the vertical slice runs:

| Intervention | Trunk Behavior (`1a2d1d62`) | Trunk Gap for `V6` | Required `V6` Mechanism (When Coupled Mode Active) |
|---|---|---|---|
| **Drought (`RainfallDelta`)** | `live_experiment.rs:1119-1121`: computes a single scalar `fertility_multiplier` ignoring `cmd.region` (`Global`/`Rect`/`Radius`), applied uniformly in `resource_field_regrowth_system` (`environmental_systems.rs:55`); records flat `EffectRecord` (`parent: None` at `:1154-1163`) | Global grass-rate shortcut bypasses spatial hydrology & `soil_moisture` | When `live.v6_interventions` (and `DynamicFields` + `V5`) is enabled, keep `forcings.fertility_multiplier = 1.0` so `RainfallDelta` acts **exclusively** via `DynamicFields::step_climate` (`dynamic_fields.rs:284`) -> `step_water` (`soil_moisture`) -> `V5` `step_regrowth_coupled_strided` -> grazing/starvation |
| **Remove wolves (`RemovePredators`)** | `live_experiment.rs:1251-1280`: selects `With<Predator>` **without** checking `cmd.region` or `SpeciesId`, and repeats culling `floor(len * fraction)` **every `ECOLOGY_PERIOD` (`60` ticks)** for the entire `duration_ticks`! | Repeating every 60 ticks over `duration_ticks > 60` culls ~80% repeatedly down to zero; ignores `cmd.region` | Map agent `Position` via `Res<MapBounds>` + `sim_rules::world_xz_to_cell` (`sim_rules.rs:123`) / `ResourceField::cell_index` (`ecology.rs:498`) into grid `(col, row)` for `cmd.region.contains(col, row)`, filter by `SPECIES_WOLF` (when `V2` active, falling back to `Predator`), apply as a **single-shot trigger** inside the `ECOLOGY_PERIOD` band (`live_experiment.rs:1145`; windows must span at least one `60`-tick boundary per `V1 D-V1-1 = A` & `live_experiment_tests.rs:160-189`, deduplicated by `applied_once_ids` persisted in `SavedSimulationState`, OR explicitly placed above `:1145` for sub-second triggers), and use `leave_carcass` per `D-V4-3` |
| **Deforest headwaters (`Deforest`)** | `live_experiment.rs:1191-1220`: calls `strip_region(field, cmd, fraction)` (`:1413`) on grass `ResourceField` only; never despawns `Tree` entities or touches headwaters canopy/roots | Never cuts actual forest `Tree` entities; `root_cover` / infiltration / erosion unaffected | Map `Tree` `Position` to `(col, row)` inside `cmd.region` (inside `ECOLOGY_PERIOD` with single-shot `applied_once_ids` guard), deterministically sort matching `Tree` entities by spatial/entity key, mark them synchronously with `DeforestedTree` + queue `Commands::despawn` for `floor(count * intensity)` trees, and strip understory grass (`plants -> detritus` closed-EU exact); `V5` `sync_vegetation_feedback_system` (`Without<DeforestedTree>`) immediately sees the reduction on tick `T` without needing a new `apply_deferred` before tuple 2 -> runoff + erosion + turbidity |
| **Fertilize around lakes (`AddNutrient`)** | `live_experiment.rs:1221-1250`: drains `pool.detritus` into `ResourceField.r` (`fertilise_region` `:1435`), teleporting EU into grass while disconnected from `DynamicFields.soil_nitrogen`! | Violates causal mechanism ("teleports detritus into standing crop") & ignores lakes / eutrophication | Disable `fertilise_region` detritus->grass EU teleport when `V6` is enabled! Apply `AddNutrient` strictly in `DynamicFields::step_soil` (`dynamic_fields.rs:427-450`), tracking `cum_nutrient_added` & `cum_nutrient_leached`; `V5` turns `soil_nitrogen` into real grass NPP (`detritus -> plants` limited by moisture/light), while lake/excess leached nitrogen drives simple lake eutrophication (see `D-V6-2`) |
| **Causal provenance (`CausalLedger`)** | `live_experiment.rs:1154, 1209, 1239, 1270` always pass **`parent: None`**; `src-tauri/crates/anima-domain/src/causal.rs:77-155` (`CausalLedger::record` + `trace_to_root`) exists with parent-chain support, completely unused by live ECS systems! | Zero `parent: Some(...)` edges anywhere in `src-tauri/src/` live runs | Add persisted `CausalChainTracker` tracking root intervention `EffectId` (`CauseId` preserved!) -> intermediate abiotic field `EffectId` -> biotic/trophic `EffectId` (`parent: Some(...)`, chain depth `>= 3`) |

### 2.1 Vertical-Slice Scenario Preset Helpers & Duration Scale

Provide pure scenario-builder constructors in `core::scenario` / `core::v6_interventions` (`validate_intervention` in `src-tauri/src/core/experiment/interventions.rs:16-165` already supports arbitrary `Region`, `Curve`, `start_tick`, `duration_ticks`, and `cause_id`):

1. **`headwaters_region(terrain: &TerrainMap, elevation_quantile: f32) -> Region` (or composite `Vec<InterventionCommand>`):** computes a deterministic high-elevation headwaters zone matching `TerrainMap` / `DynamicFields` downstream basins. Keep `Region` (`src-tauri/crates/anima-domain/src/intervention.rs:20-34`) cleanly as its 4 existing variants (`Global | Cell | Rect | Radius`) by deriving a deterministic upland `Rect`/`Radius` or multi-rect cluster (avoiding `Region` matcher churn in `exotic_energy/forcings.rs:97, 171` and `experiment/manifest.rs:560`); provide scenario helpers `drought_30d(...)`, `cull_wolves_80pct(...)`, `clear_headwaters_forest_40pct(...)`, and `fertilize_lake_ring(...)` for reproducible tests.
2. **`lake_buffer_region(...)`:** derives deterministic rings/rects around `Lake` positions (`simulation_loop.rs:1507`) or riparian lake cells (`TerrainMap` river/water adjacency).
3. **"30 simulated days" duration constant (`TICKS_PER_SIM_DAY`):** trunk uses `60 Hz` (`src-tauri/crates/anima-domain/src/sim_clock.rs:15`) and `SeasonClock::default()` defines one year = `100s` = `6,000` ticks (`ecology.rs:261-264`), while UI `src/components/Landscape/utils/sceneClock.ts:177` uses 10 real minutes (`36,000` ticks) for visual day-night. Define one authoritative simulation constant `TICKS_PER_SIM_DAY` via **`D-V6-1`** (`60` ticks/day -> `30 days = 1,800` ticks, well inside headless soak budgets and matching ~`30%` of a `SeasonClock` year!).

### 2.2 Multi-Hop `CausalLedger` Tracking & Zero-Alloc Discipline (`CausalChainTracker`)

At `1a2d1d62`, `CausalLedger::record` (`src-tauri/crates/anima-domain/src/causal.rs:86-113`) appends `EffectRecord` (`String` allocations inside `record` only when thresholds cross!). To satisfy `WORLD_SIMULATION_PLAN.md` §4.2 (`:394-400`) and keep tick hot loops allocation-free when idle:

- Store `#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)] pub struct CausalChainTracker`:
  - `pub applied_once_ids: Vec<u32>` (deduplicates one-shot interventions like `RemovePredators` & `Deforest` without re-triggering across ticks; persisted in `SavedSimulationState` so save/load never re-fires a completed cull!)
  - `pub active_root_effects: Vec<(CauseId, u32, EffectId)>` (maps active `(cause_id, cmd_id)` -> root `EffectId` recorded when intervention opens, and intermediate abiotic `EffectId`s for `soil_moisture`, `soil_nitrogen`, `canopy_cover`, `erosion`, `turbidity`, `algal_bloom`)
  - `pub prev_metrics: [f64; ...]` baseline snapshot per tracked aggregate metric so `CausalLedger::record` is emitted **only on `ECOLOGY_PERIOD` (60 ticks)** and **only when `|delta| >= SIGNIFICANCE_THRESHOLD`**, bounded by a configurable `MAX_LEDGER_RECORDS` cap (`D-V6-3`).
- **Four Verifiable Mechanism Chains (`parent: Some(...)`, Rooted via `ledger.trace_to_root(leaf)`):**
  1. **Drought (`cause_1`):** `precip` (`parent=None`) -> `soil_moisture` (`parent=e1`) -> `grass_npp` (`parent=e2`) -> `herbivore_population / starvation_deaths` (`parent=e3`) -> `predator_population` (`parent=e4`).
  2. **Remove 80% wolves (`cause_2`):** `wolf_population` (`parent=None`) -> `predation_flux` (`parent=e1`) -> `rabbit_population` (`parent=e2`) -> `grass_standing_crop` (`parent=e3`).
  3. **Clear 40% headwaters forest (`cause_3`):** `tree_canopy_cover` (`parent=None`) -> `root_cover / infiltration` (`parent=e1`) -> `surface_runoff` (`parent=e2`) -> `soil_erosion / river_turbidity` (`parent=e3`).
  4. **Fertilize around lakes (`cause_4`):** `soil_nitrogen` (`parent=None`) -> `grass_npp / littoral_plants` (`parent=e1`) -> `lake_eutrophication / dissolved_oxygen` (`parent=e2`) -> `herbivore_population` (`parent=e3`).
- **Closed-EU Compatibility:** Every `EffectRecord` is pure observation metadata; `CausalChainTracker` and `CausalLedger` sit outside `EcosystemBiomass` and never create or destroy EU.

### 2.3 Eutrophication State, Persistence & `world_checksum`

- **Lake eutrophication representation (per `D-V6-2`):** Adding lightweight pre-allocated lake/cell fields `algal_bloom: Vec<f32>` & `dissolved_oxygen: Vec<f32>` on `DynamicFields` (or tracked per `Lake` entity) responds smoothly to leached `soil_nitrogen` + `turbidity`, driving oxygen dip when nutrient excess spikes — all deterministic, finite `[0.0, 1.0]`, zero-alloc, and saved inside `SavedSimulationState`.
- **Snapshot Persistence & `world_checksum`:**
  - Persist `CausalChainTracker` (and `CausalLedger` if experiment adapter checkpointing saves ledger state — verify parity with `run(N) == run(K) -> save -> load -> run(N - K)`) via `#[serde(default, skip_serializing_if = "Option::is_none")]` in `SavedSimulationState` (`simulation_state.rs:459`).
  - Gate new causal/interventions tracking strictly behind **`LIVE_KEY_V6_INTERVENTIONS: &str = "live.v6_interventions"`** (`LiveWorldConfig::v6_interventions: bool`, `ANIMA_V6_INTERVENTIONS` desktop env; preflight-requires `dynamic_fields`, `species_profiles`, `lifecycle`, `carcasses`, `field_plant_coupling` or activates compatible subset loudly!) so all pre-`V6` E2/E3b/M2 tests remain **100% bit-identical**.
  - Include new fields (`applied_once_ids`, `algal_bloom`, `dissolved_oxygen`) in `world_checksum` **only when** `V6` policy is enabled.

## 3. Open Owner Decisions (`D-V6-1`–`D-V6-4`)

- **`D-V6-1` (Simulated day -> tick conversion `TICKS_PER_SIM_DAY`):**
  - **Option A (Recommended):** `TICKS_PER_SIM_DAY = 60` ticks (`1` sim day = `1` ecology cycle = `1` sim second at `60 Hz`, matching `SeasonClock` 1 year = `100` sim days = `6,000` ticks at `ecology.rs:261-264`), making `"30 simulated days"` = **`1,800` ticks** — clearly long enough to deplete soil moisture, depress grass, and trigger delayed rabbit/wolf response while finishing cleanly in integration tests and multi-seed `V8` sweeps.
  - **Option B:** `TICKS_PER_SIM_DAY = 600` ticks (`30 days = 18,000` ticks, ~3 seasonal years).
  - **Option C:** Reuse UI visual clock `src/components/Landscape/utils/sceneClock.ts:177` (`36,000` ticks/day -> `30 days = 1,080,000` ticks; impractical per scenario run).
- **`D-V6-2` (Simple eutrophication model for lake fertilization, §9 `1060`):**
  - **Option A (Recommended):** Per-cell `algal_bloom: Vec<f32>` & `dissolved_oxygen: Vec<f32>` on `DynamicFields` (active only on water/lake cells where nutrient runoff enters; excess leached/runoff nitrogen boosts `algal_bloom`, bloom decay draws down `dissolved_oxygen`, feeding aquatic mortality/stress signal cleanly without changing closed terrestrial EU).
  - **Option B:** Track scalar lake nutrient load + NPP/turbidity proxy without adding `algal_bloom` / `dissolved_oxygen` cell arrays. Less direct against §6 table (`683`: `nutrient, NPP, algae, oxygen, population cycles`).
- **`D-V6-3` (`CausalLedger` noise gating & memory bound):**
  - **Option A (Recommended):** Record causal mechanism updates on `ECOLOGY_PERIOD` (`60` ticks) when `|delta| >= threshold` **and** cap ledger growth cleanly via bounded capacity / ring compaction (`WORLD_SIMULATION_PLAN.md` §4.2 `:396-399`) so long `V10` soak runs cannot OOM.
  - **Option B:** Append unboundedly when threshold crosses (relying on `V10` run length staying modest).
- **`D-V6-4` (Default interaction with legacy tests calling `apply_live_interventions_system`):**
  - **Option A (Recommended):** Gate `V6` field-mediated intervention dispatch + single-shot `RemovePredators` + `Tree` removal behind opt-in `live.v6_interventions = 1.0` (`ANIMA_V6_INTERVENTIONS`) so existing legacy tests (like `src-tauri/tests/ledger_bypass_tests.rs:271` testing trunk `AddNutrient`/`Deforest`/`RemovePredators`) keep running untouched until explicitly migrated.
  - **Option B:** Switch automatically whenever `DynamicFields` is present (`live.dynamic_fields = 1.0`), adding explicit single-shot flag on `RemovePredators`.

## 4. Future `V6` Coding Card Boundaries

### Allowed Paths
- `src-tauri/crates/anima-domain/src/intervention.rs` (keep 4 `Region` variants; add `exotic_energy/forcings.rs:97, 171` & `experiment/manifest.rs:560` ONLY if extending `Region`, plus any strictly `#[serde(default)]` backwards-compatible `InterventionCommand` metadata)
- `src-tauri/crates/anima-domain/src/causal.rs` (only if bounded ring-cap / serialization helpers added without breaking `s12_chain_traces_to_root_cause`)
- `src-tauri/src/core/experiment/interventions.rs` (`validate_intervention` for any new region/duration validation)
- `src-tauri/src/core/live_experiment.rs` (`apply_live_interventions_system` `V6` branch, `LIVE_KEY_V6_INTERVENTIONS`, `LIVE_KEYS`, `LiveWorldConfig`, `CausalChainTracker` wiring, preflight checks, optional opt-in causal/eutrophication observables excluded from `all_live_observable_ids()`)
- `src-tauri/src/core/dynamic_fields.rs` (`algal_bloom` / `dissolved_oxygen` if `D-V6-2 = A`, watershed/headwaters helper)
- `src-tauri/src/core/scenario.rs` (deterministic constructors for the 4 vertical-slice scenarios + `headwaters_region` / `lake_buffer_region` helpers)
- `src-tauri/src/core/environmental_systems.rs` (disable global `fertility_multiplier` shortcut when `V6` routes drought through `DynamicFields` + emit biotic chain links via `CausalChainTracker`)
- `src-tauri/src/core/agent_systems.rs` & `mortality.rs` (emit wolf/rabbit demographic/mortality causal child links with `parent: Some(...)` when `CausalChainTracker` active)
- `src-tauri/src/core/simulation_schedule.rs` (any `step_causal_chains_system` in tuple 4 right before `ecosystem_census_system`, 0 `Commands`)
- `src-tauri/src/core/simulation_state.rs` (`SavedSimulationState::causal_chain_tracker`, `empty_saved_state_for_tests`, `serialize_world_state`)
- `src-tauri/src/core/snapshot.rs` (`validate_state`, conditional `world_checksum`)
- `src-tauri/src/core/simulation_loop.rs` (`ANIMA_V6_INTERVENTIONS` opt-in init/restore)
- `src-tauri/src/core/experiment/observables.rs` (opt-in causal/eutrophication observable metadata if added)
- `src-tauri/tests/schedule_ambiguity_tests.rs` (assert declared schedule order)
- `src-tauri/tests/v6_interventions_causeid_tests.rs` (new: S12/S44/S47/S48 vertical-slice tests for all 4 interventions verifying intermediate field effects, `trace_to_root`, closed EU + water + nutrient conservation, single-shot wolf cull, no direct drought mortality magic, zero-alloc idle ticks, checkpoint replay, flag-off bit-identity)
- `LIVE_KEYS.len()` pinning tests (`+1` count) + prereg manifests (`e2_seam_tests.rs`, `eco_card_b_tests.rs`, `founder_genotype_tests.rs`, `founder_layout_tests.rs`, `grass_steering_tests.rs`, `lake_layout_tests.rs`, `land_mask_tests.rs`, `strict_prey_cap_tests.rs`, `prereg_e3_manifest_tests.rs` + fixture JSON, `prereg_e3b_manifest_tests.rs` + fixture JSON per freeze policy)
- `SavedSimulationState::causal_chain_tracker: None` is supplied via `empty_saved_state_for_tests()` (`src-tauri/src/core/simulation_state.rs:608`, spread with `..empty_saved_state_for_tests()` by `environmental_elements_stress_tests.rs`, `environmental_elements_tests.rs`, `persistence_stress_tests.rs`, `persistence_tests.rs`)
- `docs/ai/planning/` (card `V6` implementation report note)

## 5. `DONE WHEN`, Negative Controls, Conservation & Determinism Checks

### `DONE WHEN`
1. **Drought (`S44`, 30 sim days, `RainfallDelta`):** Only target-region `precipitation` and `soil_moisture` fall; **zero** instant creature deaths on drought onset; grass NPP declines through `V5` limitation before rabbit population declines; `CausalLedger::trace_to_root` traces herbivore decline back to `cause_1` through `precip -> soil_moisture -> grass_npp` (`depth >= 3`).
2. **Remove 80% wolves (`S48`, `RemovePredators`):** Exactly `floor(0.8 * wolves_in_region)` are reclaimed once (not repeatedly every 60 ticks and not outside `cmd.region`); subsequent rabbit abundance rises and standing grass drops (`trophic cascade`) relative to unperturbed control seed; `trace_to_root` traces rabbit surge to `cause_2`.
3. **Clear 40% headwaters forest (`Deforest`):** `floor(0.4 * trees_in_region)` trees are marked `DeforestedTree` + despawned and stripped understory EU moves conservatively (`plants -> detritus`); `canopy_cover` and `root_cover` fall on the same tick, reducing infiltration while spiking `erosion` and downstream `turbidity` (`trace_to_root` -> `cause_3`).
4. **Fertilize around lakes (`S47`, `AddNutrient`):** `soil_nitrogen` rises via `DynamicFields::step_soil` (zero direct `pool.detritus -> ResourceField.r` EU teleport!); `V5` lifts local NPP, excess leached/runoff nitrogen raises `algal_bloom` and depresses `dissolved_oxygen` (under `D-V6-2 = A`), with relative water/nutrient residuals `< 1e-4` (`dynamic_fields.rs:570, 762`) and closed EU within `RESIDUAL_ABS_TOLERANCE_EU` (`1e-3`, `src-tauri/crates/anima-domain/src/energy.rs:123`).
5. **No direct-population magic & workspace gates:** Control vs treatment paired runs differ **strictly** downstream of `start_tick` through field/trophic cascades; flag-off trajectories stay bit-exact with trunk; `cargo fmt --check`, `cargo clippy --features desktop --all-targets -- -D warnings`, `cargo test --features desktop` + `check_test_targets.mjs --profile desktop`, `schedule_ambiguity_tests.rs`, and `npm run check:docs` all exit `0`.

### Negative Controls (Must Fail Red Under Fault Injection, A3 #2)
1. **Broken parent links:** Temporarily pass `parent: None` on downstream causal records instead of linking intermediate `EffectId`s -> `trace_to_root` depth `>= 3` test fails red.
2. **Drought bypass (`"magic effects"`):** Re-enable global `forcings.fertility_multiplier` shortcut in `resource_field_regrowth_system` while bypassing `soil_moisture` -> spatial drought confinement & soil-moisture mediation tests fail red.
3. **Repeated wolf culling bug:** Remove `applied_once_ids` guard on `RemovePredators` over a multi-`ECOLOGY_PERIOD` duration -> 80% wolf fraction test fails red (`> 80%` culled across second firing).
4. **Legacy nutrient EU teleport:** Re-enable `fertilise_region` detritus->grass shortcut alongside `step_soil` -> uncoupled fertilizer-without-soil test or `V6` mechanism check fails red; omitting `algal_bloom` / `applied_once_ids` from `SavedSimulationState` fails checkpoint `run(N) == run(K) -> save -> load -> run(N-K)` red.

### Determinism Checks
- **Same-OS 3-process replay:** All four vertical-slice intervention runs produce bit-identical `world_checksum`, `CausalLedger` records (`EffectId`, `cause_id`, `parent`, `quantity`, `delta`), and observables across 3 independent processes on the same OS.
- **Checkpoint fork equivalence:** Saving mid-intervention (e.g., tick `K` inside the 30-day drought or immediately after 80% wolf removal) and resuming `N - K` ticks never re-fires single-shot interventions and matches uninterrupted `run(N)` bit-for-bit on `world_checksum`.
