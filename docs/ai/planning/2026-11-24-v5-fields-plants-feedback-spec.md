---
phase: planning
feature: v1-vertical-slice
title: Card V5 — Soil Water, Nutrients, and Grass Biomass Fields Coupled With Tree Feedback
description: Implementation spec for card V5 coupling DynamicFields climate/water/soil chemistry with ResourceField grass growth and Tree canopy/root feedback while keeping both EU and H2O/N budgets conserved and zero-alloc
status: proposed
owner: anima-dell
last_reviewed: 2026-11-24
---

# Card `V5` — Soil Water, Nutrients, Grass Biomass Fields & Tree Feedback (`M3.2`–`M3.3`, `M4.2`, `M4.4`)

> **Design spec only (`status: proposed`).** No Rust or TypeScript code is written in D7 (`V-SPECS`).
> All `file:line` anchors below are verified against `origin/feat/ecological-selection` at trunk
> commit `1a2d1d62de52334e675108852311a87c3c1bb10f`.

## 1. Goal, Dependencies & Scope

Close the biophysical loop between **`DynamicFields`** (abiotic climate, water, soil chemistry, erosion) and vegetation (**`ResourceField`** grass biomass + **`Tree`** entities) per `WORLD_SIMULATION_PLAN.md` §5.1–§5.4, §7 (`M3.2`–`M3.3`, `M4.2`, `M4.4`), §9, and `2026-11-05-swarm-master-plan.md` Đợt 6:

1. Couple `ResourceField` per-cell grass growth (`src-tauri/src/core/ecology.rs:331, 454`) and `Tree::fruit_growth_system` (`src-tauri/src/core/environmental_systems.rs:6`) to live local **`soil_moisture`**, **`soil_nitrogen`**, and **`air_temperature`** in `DynamicFields` (`src-tauri/src/core/dynamic_fields.rs:66`), debiting nutrient uptake against `DynamicFields::cum_nutrient_uptake` (`:106`) so the soil nutrient budget remains conserved (`nutrient_budget_residual` at `:461-466`) alongside closed EU (`pool.detritus <-> pool.plants`).
2. Compute dynamic per-cell **`canopy_cover`** and **`root_cover`** from living **`Tree`** entities (`src-tauri/src/core/components.rs:137`) + standing `ResourceField` grass cover (`S24` / `M4.4`) so vegetation actively feeds back into **infiltration**, **evaporation shading**, **erosion resistance**, and **downstream turbidity** inside `DynamicFields::step_water` (`src-tauri/src/core/dynamic_fields.rs:314`) and `step_erosion` (`dynamic_fields.rs:472`).
3. Keep all tick loops **zero-heap-allocation** (`TrackingAllocator` = `0`), keep `water_budget_residual`, `nutrient_budget_residual`, and `closed_eu_residual` simultaneously below tolerance, and keep flag-off (`live.field_plant_coupling` absent/`0`) **100% bit-identical** to `V1`.

### Exact Dependency Order

- **Depends directly on:** **`V1`** (`docs/ai/planning/2026-11-24-v1-dynamic-fields-schedule-spec.md`, PR `#235`, `VERIFIED`, pending lead merge) — requires `DynamicFields` to already be a schedule-integrated, persisted Bevy `Resource` in `build_tick_schedule` (`step_dynamic_fields_system`).
- **Unblocks:**
  - **[Card `V6`](2026-11-24-v6-interventions-causeid-spec.md)** — makes 30-day drought reduce soil water -> limit grass growth (`S44`), clearing 40% headwaters forest lower canopy/roots -> cut infiltration & spike erosion/turbidity, and fertilizing lakes raise soil nitrogen -> drive NPP boom & eutrophication without artificial teleports.

### Explicit Non-Goals

- **No 5-strategy `PlantSpeciesProfile` / full tree-competition forest simulator (`M4.1`, `M4.3`, `M4.5`):** keep `ResourceField` for grass/understory + existing `Tree` entities for large trees; only extend `Tree` / `DynamicFields` with the vertical-slice canopy/root/growth couplings required by §9.
- **No direct intervention-routing rewrites (`RemovePredators`, `CauseId` chains):** intervention propagation & multi-hop `CausalLedger` chains belong to [Card `V6`](2026-11-24-v6-interventions-causeid-spec.md).

## 2. Current Code Anchors (`1a2d1d62`) & Disconnected Loops to Wire

At `1a2d1d62`, the two layers are completely decoupled:
- `DynamicFields::from_terrain` (`dynamic_fields.rs:137-241`) initializes `root_cover: Vec<f32>` once from static `TerrainMap.biomes` (`:118-132, :179-188`), uses fixed `INFIL_RATE` (`0.15`, `:35`) and unshaded `EVAP_RATE` (`0.03`, `:37`) in `step_water` (`:321-397`), and leaves `cum_nutrient_uptake: f64` (`:106`) untouched at `0.0` (only subtracted conceptually in `nutrient_budget_residual()` at `:465`).
- `resource_field_regrowth_system` (`environmental_systems.rs:35-89`) calls `ResourceField::step_regrowth_gated_strided` (`ecology.rs:454-494`) using solely global `SeasonClock * LiveForcings.fertility_multiplier`, ignoring local `soil_moisture` and `soil_nitrogen`.
- `fruit_growth_system` (`environmental_systems.rs:6-28`) reads only static `field.r_max[i]`.

### 2.1 Dynamic Canopy & Root Cover (`DynamicFields` Extensions)

Extend `DynamicFields` (`src-tauri/src/core/dynamic_fields.rs:66-114`) with pre-allocated SoA buffers (omitted / zero-touched when coupling flag is off, or persisted via `SavedSimulationState::dynamic_fields`):
- `#[serde(default)] pub base_root_cover: Vec<f32>` (captures terrain substrate anchor if trees/grass vanish; length `n = width * height`)
- `#[serde(default)] pub canopy_cover: Vec<f32>` (`[0.0, 1.0]` per cell; fraction shaded by overlapping `Tree` crowns)
- Optional pre-allocated `#[serde(default)] transport_scratch: Vec<f32>` (`len = n`, if `D-V5-2 = A` enables 1-hop downstream advection of turbidity/leached nitrogen).

**Feedback System (`sync_vegetation_feedback_system`):**
1. Insert in **tuple 1** of `build_tick_schedule` (`simulation_schedule.rs:135-138`) immediately **before** `V1`'s `step_dynamic_fields_system`:
   `apply_live_interventions_system -> sync_vegetation_feedback_system (V5) -> step_dynamic_fields_system (V1) -> sensory_system`.
   *(Critical flush compatibility with [Card `V6`](2026-11-24-v6-interventions-causeid-spec.md): `apply_live_interventions_system` at `:136` runs before the tuple-2 `apply_deferred` at `:154`. To avoid delaying `V6` `Deforest` canopy/root updates until the next `ECOLOGY_PERIOD` fire tick, `V6` marks cut `Tree` entities synchronously with an inline marker component `DeforestedTree` alongside queueing `Commands::despawn`, and `sync_vegetation_feedback_system` filters `Without<DeforestedTree>` — keeping tuple 1 free of extra `apply_deferred` sync points!)*
   (Early return if `FieldPlantCouplingPolicy` is absent or `!policy.enabled`.)
2. Gate `sync_vegetation_feedback_system` on the **exact same tick counter and cadence** (`DynamicFields.field_tick` and `ECOLOGY_PERIOD = 60` under `V1` `D-V1-1 = A`) as `step_dynamic_fields_system` (never conflating it with `ResourceField::REGROWTH_STRIDE = 4`, which is a 4-phase spatial sweep stepped every `60 Hz` base tick reading the latest `DynamicFields` buffers). On each firing tick, **zero `canopy_cover` in-place** (`fill(0.0)`), then:
   - Read standing grass density `grass_cover[i] = (field.r[i] / field.r_max[i].max(1e-6)).clamp(0.0, 1.0)`.
   - Map each living `(Position, SpatialCollider, &Tree)` (`Without<DeforestedTree>`) onto overlapped grid cells `(col, row)` via `Res<MapBounds>` + `sim_rules::world_xz_to_cell` (`src-tauri/src/core/sim_rules.rs:123`, matching `ResourceField::cell_index` at `ecology.rs:498`, bounded clamping, zero allocations) adding normalized tree canopy density `tree_canopy_weight` and clamping `canopy_cover[i]` to `[0.0, 1.0]`.
   - Update `fields.root_cover[i] = (GRASS_ROOT_WEIGHT * grass_cover[i] + TREE_ROOT_WEIGHT * canopy_cover[i] + SUBSTRATE_ROOT_RESIDUAL * base_root_cover[i]).clamp(0.0, 1.0)`.
   - **Hydrology feedback inside `step_water` (when coupled):**
     - **Infiltration (`dynamic_fields.rs:324-335`):** boost cell infiltration capacity with roots (`effective_infil = INFIL_RATE * (0.5 + 0.8 * root_cover[i])`, clamped so infiltrated amount `<= min(surface_water[i], room)` and measured from actual `surface_water` delta -> `groundwater` retains bit-exact mass transfer!).
     - **Evaporation shading (`dynamic_fields.rs:337-351`):** canopy reduces soil/surface evaporation (`shade_factor = 1.0 - CANOPY_SHADE_EVAP * canopy_cover[i]`, floored `>= 0.2`), measuring exact removed surface + ground deltas into `cum_evap` so relative **`|water_budget_residual()| / scale < 1e-4`** holds (`dynamic_fields.rs:570`)!
   - **Erosion feedback inside `step_erosion` (`:472-480`):** already divides by `(1.0 + ROOT_RESIST * self.root_cover[i])`! Once `sync_vegetation_feedback_system` updates `root_cover[i]` from living grass + trees, losing trees/grass immediately elevates `erosion[i]` and downstream `turbidity[i]` via real biophysical coupling!

### 2.2 Biophysical Limitation on Grass (`ResourceField`) & Nutrient Conservation

Add `step_regrowth_coupled_strided` on `ResourceField` (`src-tauri/src/core/ecology.rs`, keeping existing `step_regrowth_gated_strided` at `:454` untouched when coupling is off!):

1. **Four Limiting Factors (`M4.2` / `S22`) for each visited cell `i`:**
   - **Water factor `f_w(i)`:** monotone saturating curve of `dynamic.soil_moisture[i]` (zero or severe depression below `WILTING_MOISTURE`, approaching `1.0` around optimal moisture `>= OPTIMAL_MOISTURE`).
   - **Nutrient factor `f_n(i)`:** saturating Michaelis–Menten curve `N / (N + K_N)` over `dynamic.soil_nitrogen[i]`.
   - **Temperature factor `f_t(i)`:** unimodal thermal curve peaking at intermediate normalized `dynamic.air_temperature[i]` (falling off in alpine cold / extreme heat).
   - **Light / Shading factor `f_light(i)`:** `1.0 - CANOPY_GRASS_SHADE * dynamic.canopy_cover[i]` (`>= MIN_UNDERSTORY_LIGHT`) so dense closed canopy limits grass beneath trees (`M4.4` competition!).
   - Combine per **`D-V5-1`** (`Liebig min(f_w, f_n) * f_t * f_light` or multiplicative) -> `cell_fertility = global_season_fertility * limiting_factor`.
2. **Dual Conservation Law (Closed EU **AND** Soil Nutrient Balance):**
   - **EU side (`pool.detritus <-> pool.plants`):** identical two-pass strided gating against `pool.detritus` as `step_regrowth_gated_strided` (`ecology.rs:468-493`) — Pass 1 computes per-cell `desired_growth` (additionally bounded by available local `dynamic.soil_nitrogen[i] / N_PER_EU_UPTAKE` so regrowth cannot demand more biomass than local nitrogen can stoichiometry-wise sustain!), Pass 2 applies `scale = (budget / desired).min(1.0)`, reads exact `(field.r[i] - before) as f64` (`grown`), and updates `pool.detritus -= grown; pool.plants += grown;`.
   - **Nitrogen side (`dynamic.soil_nitrogen` -> `dynamic.cum_nutrient_uptake`):** for every cell `i` that grew by actual measured `cell_grown`, debit `n_wanted = (cell_grown * N_PER_EU_UPTAKE) as f32` from `dynamic.soil_nitrogen[i]`:
     - Measure actual stored delta: `let n_before = dynamic.soil_nitrogen[i]; dynamic.soil_nitrogen[i] = (n_before - n_wanted).max(0.0); let n_taken = (n_before as f64) - (dynamic.soil_nitrogen[i] as f64);`
     - Accumulate `dynamic.cum_nutrient_uptake += n_taken;` (`dynamic_fields.rs:106`).
     - Notice why measuring stored delta in `f64` guarantees **`DynamicFields::nutrient_budget_residual()` (`:461-466`) stays exact**: `total_nutrient()` drops by exact `n_taken`, while `-cum_nutrient_uptake` subtracts exact `n_taken`!
3. **Tree Fruit & Litter Return (`environmental_systems.rs:6-28`):**
   - When coupled, `fruit_growth_system` scales `npp_factor` with `dynamic.soil_moisture[cell] * f_nutrient(soil_nitrogen[cell])` (zero heap allocations). (If `D-V5-3 = A`, severe prolonged drought also induces `plants -> detritus` wilting flux tracked through `pool.plants -= wilted; pool.detritus += wilted` without deleting EU!)

### 2.3 Opt-In Gate, Persistence Validation & Conditional `world_checksum`

- **Config:** Add `LIVE_KEY_FIELD_PLANT_COUPLING: &str = "live.field_plant_coupling"` to `LIVE_KEYS` (`live_experiment.rs:280`), `#[serde(default)] pub field_plant_coupling: bool` to `LiveWorldConfig` (`:316`), and `ANIMA_FIELD_PLANT_COUPLING` in `simulation_loop.rs`.
- **Preflight invariant:** Reject (`Err(ExperimentError)`) any manifest setting `live.field_plant_coupling = 1.0` without `live.dynamic_fields = 1.0` (`"field_plant_coupling requires live.dynamic_fields = 1.0"`).
- **Persistence:** Validate new `base_root_cover`, `canopy_cover`, and `transport_scratch` vector lengths (`== width * height`, all finite, `[0.0, 1.0]`) inside `snapshot::validate_state` (`snapshot.rs:458`) when `DynamicFields` is present (`#[serde(default)]` preserves backwards compatibility with `V1`-era fixtures if any).
- **Conditional `world_checksum` (`snapshot.rs:789`):** Hash `canopy_cover` (and `base_root_cover` / `root_cover`, which is now dynamic rather than static from terrain!) **only when** `FieldPlantCouplingPolicy.enabled`, preserving flag-off checksums bit-for-bit.

## 3. Open Owner Decisions (`D-V5-1`–`D-V5-3`)

- **`D-V5-1` (Multi-factor plant growth limitation rule, `M4.2` / `S22`):** — **DECIDED (2026-11-24, QD-OPENQ): Option A** (Liebig's Law of the Minimum).
  - **Option A (Recommended):** **Liebig's Law of the Minimum** (`limiting = min(f_water, f_nutrient) * f_temp * f_light`), directly identifying and attributing *which single factor* (`"water"`, `"nitrogen"`, `"temperature"`, or `"light"`) limited cell NPP for causal explainability (`M4.2` "explainable limiting factor").
  - **Option B:** Full multiplicative combination (`f_water * f_nutrient * f_temp * f_light`). Smoother curve, harder single-factor attribution.
- **`D-V5-2` (Downstream hydrologic transport of turbidity and leached nitrogen):** — **DECIDED (2026-11-24, QD-OPENQ): Option A** (pre-allocated `transport_scratch` downstream advection).
  - **Option A (Recommended):** Add one pre-allocated `transport_scratch: Vec<f32>` pass along `DynamicFields::downstream[i]` inside `step_erosion` / `step_soil` so headwaters deforestation turbidity and upslope excess leached nitrogen visibly advect into downstream river/lake cells (using strictly measured `moved = before - after` transfers so conservation holds!).
  - **Option B:** Keep `erosion`/`turbidity` and `soil_nitrogen` strictly cell-local (`V1` `step_erosion` & `step_soil` equations unchanged). Simpler, but lake cells only eutrophy if fertilizer lands directly inside or adjacent to lake cells.
- **`D-V5-3` (Drought plant wilt feedback vs growth-pause only):** — **DECIDED (2026-11-24, QD-OPENQ): Option A** (drought dieback below `WILTING_MOISTURE` conserving EU).
  - **Option A (Recommended):** Above wilting point only growth slows/stops; below `WILTING_MOISTURE`, standing grass `ResourceField.r[i]` suffers mild drought dieback (`wilt_rate * delta_moisture`), transferring exact measured `f64` biomass `plants -> pool.detritus` (and optionally `CARCASS_EU_TO_ORGANIC_NM` litter back into `soil_organic` tracked in `cum_nutrient_added`). Ensures severe 30-day drought visibly thins dense preexisting grassland rather than merely freezing regrowth if no herbivores are nearby.
  - **Option B:** Regrowth pauses cleanly (`f_water = 0.0`) without non-grazing plant dieback.

## 4. Future `V5` Coding Card Boundaries

### Allowed Paths
- `src-tauri/src/core/dynamic_fields.rs` (`canopy_cover`, `base_root_cover`, optional `transport_scratch`, root-modulated infiltration & canopy-shaded evaporation in `step_water`, coupled nutrient/advection helpers)
- `src-tauri/src/core/ecology.rs` (`ResourceField::step_regrowth_coupled_strided`, Liebig/multiplicative limiting factor helpers)
- `src-tauri/src/core/environmental_systems.rs` (`sync_vegetation_feedback_system`, coupled `resource_field_regrowth_system` & `fruit_growth_system`)
- `src-tauri/src/core/simulation_schedule.rs` (`build_tick_schedule` tuple 1 slot before `step_dynamic_fields_system`)
- `src-tauri/src/core/snapshot.rs` (`validate_state` buffer checks, conditional `world_checksum`)
- `src-tauri/src/core/simulation_loop.rs` (`ANIMA_FIELD_PLANT_COUPLING`)
- `src-tauri/src/core/live_experiment.rs` (`LIVE_KEY_FIELD_PLANT_COUPLING`, `LIVE_KEYS`, `LiveWorldConfig`, preflight check requiring `dynamic_fields`, optional opt-in observables `live.mean_canopy_cover` / `live.mean_turbidity` / `live.mean_soil_nitrogen` excluded from `all_live_observable_ids()`)
- `src-tauri/src/core/experiment/observables.rs` (opt-in observable metadata if exposed)
- `src-tauri/tests/schedule_ambiguity_tests.rs` (assert `apply_live_interventions_system -> sync_vegetation_feedback_system -> step_dynamic_fields_system -> sensory_system`)
- `src-tauri/tests/fields_plants_coupling_tests.rs` (new: S22 limiting factors, S24 tree canopy/root -> infiltration/erosion/grass shading feedback, dual water+nutrient+closed-EU budget conservation, `TrackingAllocator` zero-alloc on active coupled sweep, checkpoint replay, flag-off bit-identity)
- `LIVE_KEYS.len()` pinning tests (`+1` count) + prereg manifests (`e2_seam_tests.rs`, `eco_card_b_tests.rs`, `founder_genotype_tests.rs`, `founder_layout_tests.rs`, `grass_steering_tests.rs`, `lake_layout_tests.rs`, `land_mask_tests.rs`, `strict_prey_cap_tests.rs`, `prereg_e3_manifest_tests.rs` + fixture JSON, `prereg_e3b_manifest_tests.rs` + fixture JSON per freeze policy)
- `docs/ai/planning/` (card `V5` implementation report note)

## 5. `DONE WHEN`, Negative Controls, Conservation & Determinism Checks

### `DONE WHEN`
1. **`S22` (`M4.2`) Field-limited plant growth:** Lowering `soil_moisture` or `soil_nitrogen` cleanly suppresses `ResourceField` grass regrowth in those specific cells while well-watered/fertilized cells regrow normally; under dense `Tree` canopy, understory grass saturates lower than open sunlit cells.
2. **`S24` (`M4.4`) Vegetation -> soil/hydrology feedback:** Adding `Tree` entities raises local `canopy_cover` and `root_cover`, reducing local evaporation and erosion while raising infiltration; removing trees reverses all three responses dynamically.
3. **Triple conservation invariant:** Over multi-thousand-tick runs with active rain, evaporation, runoff, grazing, and fertilizer:
   - `|DynamicFields::water_budget_residual()| / water_scale < 1e-4` (`water_scale = (initial_water + cum_precip).max(1.0)`, matching `dynamic_fields.rs:570`)
   - `|DynamicFields::nutrient_budget_residual()| / nutrient_scale < 1e-4` (`nutrient_scale = (initial_nutrient + cum_nutrient_added).max(1.0)`, matching `dynamic_fields.rs:762`)
   - `|closed_eu_residual| <= RESIDUAL_ABS_TOLERANCE_EU` (`1e-3`, `src-tauri/crates/anima-domain/src/energy.rs:123`)
4. **Zero-alloc hot loop (`CLAUDE.md`):** Process-global `TrackingAllocator` across active coupled regrowth + `sync_vegetation_feedback_system` + `step_dynamic_fields_system` ticks records `0` heap allocations per tick.
5. **Flag-off parity & workspace gates:** With `live.field_plant_coupling = 0`/absent (`dynamic_fields = 1` or `0`), `world_checksum` and every buffer stay **bit-identical** to `V1`; enabling `field_plant_coupling` without `dynamic_fields` fails preflight loudly; `cargo fmt --check`, `cargo clippy --features desktop --all-targets -- -D warnings`, `cargo test --features desktop` + `check_test_targets.mjs --profile desktop`, and `npm run check:docs` all exit `0`.

### Negative Controls (Must Fail Red Under Fault Injection, A3 #2)
1. **Unrecorded nitrogen uptake:** Debit `soil_nitrogen[i]` in `step_regrowth_coupled_strided` without adding actual `n_taken` to `cum_nutrient_uptake` -> relative `|nutrient_budget_residual()| / nutrient_scale` exceeds `1e-4` and fails red.
2. **EU regrowth/wilt leak:** Skip subtracting `grown` from `pool.detritus` (or adding `wilted` if `D-V5-3 = A`) -> closed-EU conservation test fails red.
3. **Static `root_cover` bug:** Temporarily comment out the `root_cover` update inside `sync_vegetation_feedback_system` -> removing trees fails to raise `erosion` / reduce `infiltration` (`S24` test fails red).
4. **Preflight silence:** Allow `field_plant_coupling = 1` when `dynamic_fields = 0` without returning `Err` -> manifest validation test fails red.

### Determinism Checks
- **Same-OS multi-process determinism:** Across 3 independent processes with `live.dynamic_fields = 1.0, live.field_plant_coupling = 1.0` and trees + grazers present, `world_checksum`, `ResourceField.r`, `DynamicFields` arrays, and ledgers match bit-for-bit on the same OS.
- **Save/load equivalence:** `run(N) == run(K) -> save -> load -> run(N - K)` produces bit-exact `world_checksum` and identical water/nutrient/EU residuals.
