---
phase: planning
feature: v1-vertical-slice
title: Card V2 — Data-Driven SpeciesProfile for Rabbit and Wolf
description: Implementation spec for card V2 introducing data-driven SpeciesProfile, diet/food-web edges, habitat suitability, migration/snapshot persistence, and opt-in wiring for rabbit and wolf in the v1.0 vertical slice
status: proposed
owner: anima-dell
last_reviewed: 2026-11-24
---

# Card `V2` — Data-Driven `SpeciesProfile` for Rabbit & Wolf (`M5.1`, `M6.1`)

> **Design spec only (`status: proposed`).** No Rust or TypeScript code is written in D7 (`V-SPECS`).
> All `file:line` anchors below are verified against `origin/feat/ecological-selection` at trunk
> commit `1a2d1d62de52334e675108852311a87c3c1bb10f`.

## 1. Goal, Dependencies & Scope

Replace hardcoded binary `Prey` / `Predator` parameters (`AgentClass`, fixed energy thresholds, flat grazing/predation constants) with a data-driven **`SpeciesProfile`** architecture (`WORLD_SIMULATION_PLAN.md` §7 `M5.1` / `M6.1` and §9, `2026-11-05-swarm-master-plan.md` Đợt 6) for the two vertical-slice species (**rabbit** and **wolf**):

1. Define pure domain data structs (`SpeciesProfile`, `TrophicGuild`, `DietSpec`, `HabitatSuitability`, `LifeHistorySpec`) keyed by `SpeciesId` (`src-tauri/crates/anima-domain/src/intervention.rs:88`) and stored in an immutable `SpeciesRegistry` resource loaded deterministically without heap allocations on tick paths.
2. Attach an additive `AgentSpecies(pub SpeciesId)` ECS component at genesis, biological birth, evolutionary replacement, snapshot restore, migration, hibernation wake, and aggregate-LOD rehydration (`D01`, `D02`, `D09`).
3. Route diet queries (`herbivore_grazing_system`, `detect_environmental_collisions_system`, `combat_system`), basal/stage cost coefficients (`metabolic_decay_system`), and default reproduction thresholds/caps (`autonomous_reproduction_system`) through `SpeciesProfile` when `live.species_profiles` (or desktop `ANIMA_SPECIES_PROFILES`) is enabled, while preserving **100% bit-identical** trunk `1a2d1d62` behavior when disabled/absent.

### Exact Dependency Order

- **Depends directly on:** **`V0`** (`docs/ai/planning/2026-11-24-v0-develop-at-birth-spec.md`, PR `#235`, `VERIFIED`, pending lead merge) — `V0`'s `develop_at_birth` / `spawn_developed` pipeline and `HabitatPreference` on `MorphologyGenotype` / `DevelopedPhenotype` establish the birth-time development boundary that `V2`'s species morphology/habitat suitability builds upon.
- **Unblocks:**
  - **[Card `V3`](2026-11-24-v3-lifecycle-spec.md)** (lifecycle stages, maturation age, reproductive cooldown, and senescence age ceiling read per-species parameters from `SpeciesProfile`).
  - Transitively **[Card `V4`](2026-11-24-v4-carcasses-decomposition-spec.md)** (species-tagged carcasses and scavenging food-web edges) and **[Card `V6`](2026-11-24-v6-interventions-causeid-spec.md)** (`RemovePredators` targeting `SPECIES_WOLF`).

### Explicit Non-Goals (Deferred to Later Cards)

- **Age, life stages (`Juvenile` / `Adult` / `Senescent`), cooldowns, and senescence mortality:** deferred to [Card `V3`](2026-11-24-v3-lifecycle-spec.md) (`ADR-0013`).
- **Scavenging, detritivores, and carcass nutrient pools:** deferred to [Card `V4`](2026-11-24-v4-carcasses-decomposition-spec.md) (`M5.6`, `M6.2`, `M6.3`).
- **Multi-species food webs beyond rabbit & wolf, omnivores, or plant species profiles (`M4.1`):** deferred beyond `V2` (data model must remain extensible to `N` species without changing core traits).
- **Wire changes to TypeScript `AgentType` (`ts-rs`):** keep `AgentType` (`src-tauri/src/core/components.rs:58-69`) untouched so frontend IPC contracts stay invariant (`check:ipc` `0` differences).

## 2. Current Code Anchors (`1a2d1d62`) & Required Design Mechanics

At `1a2d1d62`, binary species behavior is scattered across five hardcoded sites:

| Concern | Trunk Anchor (`1a2d1d62`) | Current Hardcoding | Target in `V2` (When Enabled) |
|---|---|---|---|
| Species identifier type | `src-tauri/crates/anima-domain/src/intervention.rs:88` | `pub type SpeciesId = u32;` already defined, unused by ECS agents | Canonical `AgentSpecies(pub SpeciesId)` component; `SPECIES_RABBIT: SpeciesId = 1`, `SPECIES_WOLF: SpeciesId = 2`, `SPECIES_LEGACY_UNSET: SpeciesId = 0` |
| Guild markers & UI enum | `src-tauri/src/core/components.rs:8-24, 58-69` | Marker components `Predator` (`:9`), `Prey` (`:12`); wire enums `AgentClass` (`:21`) & `AgentType` (`:66`) | Keep `Predator` / `Prey` / `AgentClass` / `AgentType` derived from `SpeciesProfile.trophic_guild` during `V2–V8` (see `D-V2-2`) |
| Grass grazing & fruit | `src-tauri/src/core/environmental_systems.rs:97-173`, `:351-385` | `herbivore_grazing_system` filters `(With<Agent>, With<Prey>)` (`:104`) and fixes `max_bite = 8.0 * dt` (`:116`); fruit loop checks `opt_prey.is_some()` (`:351`) | Query `Option<&AgentSpecies>` and resolve `DietSpec` (`grazes_grass`, `max_bite_per_sec`, `eats_fruit`) from `SpeciesRegistry` (`O(1)` slice index) |
| Predation & food-web edges | `src-tauri/src/core/world_systems.rs:366-588`, `src-tauri/src/core/ecology.rs:124, 155, 174-175, 192` | `combat_system` checks `(With<Agent>, With<Predator>)` against `Prey` (`:375, 379`) using global `CAPTURE_ATTACK` (`0.02`), `CAPTURE_HANDLING` (`0.02`), `LINDEMAN_EFFICIENCY` (`0.40`) via `predation_capture_with_refuge` (`:192`) | Call `ecology::holling_type3_with_refuge` (`:124`) directly inside `combat_system` with `predator_profile.diet.can_prey_on(prey_species_id)` + profile `capture_attack`, `capture_handling`, `assimilation_efficiency` (leaving legacy `predation_capture_with_refuge` intact; closing every `f64` residual into `pool.detritus` at `world_systems.rs:512-514`) |
| Basal metabolism & reproduction | `src-tauri/src/core/world_systems.rs:137`, `src-tauri/src/core/agent_systems.rs:1000-1002` | `k_base = if opt_predator.is_some() { 0.2 } else { 0.1 }`; reproduction uses `(0.95, 0.35, 0.65)` vs `(0.65, 0.30, 0.50)` | Read `profile.metabolism.k_base` and `profile.reproduction` (`threshold_fraction`, `safe_fraction`, `invest_fraction`, `efficiency`) plus `profile.life_history` (`V3` fields present with validated defaults) |

### 2.1 Pure Domain Module (`src-tauri/crates/anima-domain/src/species.rs`) & `SpeciesRegistry`

Create `src-tauri/crates/anima-domain/src/species.rs` (`pub mod species;` in `src-tauri/crates/anima-domain/src/lib.rs`, Bevy-free so scenario & unit tests run directly):

1. **Core Value Types (JSON/bincode `Serialize, Deserialize`, `PartialEq`, zero `HashMap` for determinism):**
   - `pub struct SpeciesProfile`:
     - `pub id: SpeciesId`, `pub slug: String` (`"rabbit"`, `"wolf"`), `pub display_name: String`
     - `pub legacy_class: SpeciesClassMirror` (`Prey` | `Predator` — drives `AgentClass` and backward-compatible `Prey`/`Predator` markers when `D-V2-2 = A`)
     - `pub trophic_guild: TrophicGuild` (`Herbivore`, `Carnivore`, `Omnivore`, `Scavenger`)
     - `pub diet: DietSpec`: `grazes_grass: bool`, `eats_fruit: bool`, `eats_carcass: bool`, `max_bite_per_sec: f32`, `prey_targets: Vec<SpeciesId>` (empty for rabbit, `[SPECIES_RABBIT]` for wolf), `capture_attack: f32`, `capture_handling: f32`, `assimilation_efficiency: f32` (used instead of flat `LINDEMAN_EFFICIENCY` when active, preserving bit-exact `debit_reserve` / `credit_reserve` + detritus remainder accounting in `combat_system`)
     - `pub habitat: HabitatSuitability`: `allowed_biomes: [bool; 11]` (`O(1)` lookup indexed by `BiomeType as usize` matching `src-tauri/src/core/terrain.rs:114-126` indices `0..=10`), `min_elevation: f32`, `max_elevation: f32`, `min_temp_norm: f32`, `max_temp_norm: f32`, `is_aquatic: bool`, `unsuitable_stress_cost: f32`
     - `pub metabolism: SpeciesMetabolicSpec`: `k_base: f32`, `reference_mass: f32`, `target_energy: f32`, `target_hydration: f32`
     - `pub life_history: SpeciesLifeHistorySpec`: `repro_threshold_fraction: f32`, `safe_reserve_fraction: f32`, `repro_invest_fraction: f32`, `maturity_age_ticks: u64`, `senescence_age_ticks: u64`, `max_age_ticks: u64`, `reproduction_cooldown_ticks: u32`, `litter_size: u8`, `maturation_surcharge: f32`, `senescence_surcharge: f32`
       *(In `V2`, only `repro_*` fractions and `k_base` affect dynamics when enabled; stage/age/cooldown fields are validated and stored ready for `V3`.)*
2. **Built-in Vertical-Slice Defaults (`M5.1` + `WORLD_SIMULATION_PLAN.md` §9):**
   - `SPECIES_RABBIT` (`id = 1`, `slug = "rabbit"`): `TrophicGuild::Herbivore`, `grazes_grass = true`, `eats_fruit = true`, `prey_targets = []`, suitable terrestrial grassland/temperate/boreal biomes (non-ocean, non-snow-peak), fast life-history preset (`maturity_age_ticks < wolf`, shorter `reproduction_cooldown_ticks`, lower `k_base`).
   - `SPECIES_WOLF` (`id = 2`, `slug = "wolf"`): `TrophicGuild::Carnivore`, `grazes_grass = false`, `eats_fruit = false`, `prey_targets = [SPECIES_RABBIT]`, broad terrestrial forest/grassland suitability, slow life-history preset (higher `maturity_age_ticks`, longer `reproduction_cooldown_ticks`, higher `repro_threshold_fraction` & `k_base`).
3. **Validation & `O(1)` Lookup (`SpeciesRegistry`):**
   - Validate all probabilities/fractions in `[0.0, 1.0]` (finite), `k_base >= 0.0`, `maturity_age_ticks <= senescence_age_ticks <= max_age_ticks`, `litter_size >= 1`, unique non-zero `SpeciesId` & unique non-empty `slug`, and every `prey_targets` ID resolves to a registered species.
   - Maintain a dense lookup table / sorted slice (`profiles: Vec<SpeciesProfile>`) with `get(id: SpeciesId) -> Option<&SpeciesProfile>` performing `0` heap allocations on lookup during tick execution (`can_prey_on` scans a tiny `[SpeciesId]` slice of length `<= 8`, zero allocations).

### 2.2 ECS Attachment, Boundary Persistence & `world_checksum`

1. **Component & Resource:** Add `#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)] pub struct AgentSpecies(pub SpeciesId);` in `core/components.rs` and `#[derive(Resource, ...)]` `SpeciesRegistry` in `core/species.rs`. Keep `SpeciesPolicy { pub enabled: bool }` opt-in so absent `SpeciesPolicy` / `!policy.enabled` executes legacy `Prey`/`Predator` branches untouched.
2. **Opt-in Triggers:**
   - Headless (`src-tauri/src/core/live_experiment.rs:280, 316`): add `pub const LIVE_KEY_SPECIES_PROFILES: &str = "live.species_profiles";` to `LIVE_KEYS` (bump `[&str; N]` -> `[&str; N+1]`) and `#[serde(default)] pub species_profiles: bool` on `LiveWorldConfig`. When `species_profiles` is `true`, founders assigned `Prey` receive `AgentSpecies(SPECIES_RABBIT)` and founders assigned `Predator` receive `AgentSpecies(SPECIES_WOLF)` in `live_experiment::genesis` (`:3022`).
   - Desktop (`src-tauri/src/core/simulation_loop.rs:1390`): gate behind `ANIMA_SPECIES_PROFILES` env flag (`false`/absent by default).
3. **Birth & Four Persistence Boundaries (No Silent Reset, D01/D02/D09):**
   - Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub species_id: Option<SpeciesId>` to the serialized types:
     - `SerializedAgent` (`src-tauri/src/core/simulation_state.rs:105`, populated in `serialize_single_agent` at `:1047` and `serialize_world_state` at `:1151`, validated in `SerializedAgent::validate` at `:196` & restored verbatim in `spawn_serialized_agent` at `:922`).
     - `AgentMigrationData` (`src-tauri/src/core/components.rs:166`, populated at `world_systems.rs:728` and `:905`, validated at `components.rs:421`, restored in `SpawnMigrationCommand` at `world_systems.rs:980`).
     - `ArchivedIndividual` (`src-tauri/src/core/aggregate_population.rs:224`, captured at `DehydrateAgentCommand` `:1122`, validated at `:380`, restored in `RehydrateCommand` `:1164`).
   - Add plain field `pub species_id: Option<SpeciesId>` (no `#[serde(...)]` since it has no serde derives) to `SpawnOffspringCommand` (`src-tauri/src/core/agent_systems.rs:494`) inheriting parent's `species_id`; `DormantAgentRecord` (`:205`) inherits via `SerializedAgent`.
   - Store active `SpeciesRegistry` override table cleanly in `SavedSimulationState` (`#[serde(default, skip_serializing_if = "Option::is_none")] pub species_registry: Option<SpeciesRegistry>`) if customized (`D-V2-1`), omitted when default/disabled so legacy save bytes remain identical.
   - Validate in `snapshot::validate_state` (`src-tauri/src/core/snapshot.rs:458`) that any `Some(species_id)` is non-zero and registered in the active/saved `SpeciesRegistry`.
4. **Conditional `world_checksum` (`src-tauri/src/core/snapshot.rs:789-822`):**
   - Only when `SpeciesPolicy.enabled` is true, include `AgentSpecies.0` **inside** the per-agent sorted tuple in `world_checksum` (`snapshot.rs:798-816`, extending the tuple entry cleanly when enabled rather than hashing unsorted queries or altering flag-off bytes).
5. **Additive Diagnostic Observables (Per `D-V2` Policy / E3b Isolation):**
   - Do **not** alter `LIVE_OBSERVABLE_IDS` (`12`) or `LIVE_EXTENDED_OBSERVABLE_IDS` (`16`). Any per-species counts (`live.rabbit_count`, `live.wolf_count`) go into an opt-in slice (`LIVE_SPECIES_OBSERVABLE_IDS`) registered in `ObservableRegistry::live_default` (`src-tauri/src/core/experiment/observables.rs:306`), gated via `ensure_observables_supported` (`live_experiment.rs:849`), and **excluded** from `all_live_observable_ids()` (`live_experiment.rs:1652`).

## 3. Open Owner Decisions (`D-V2-1`–`D-V2-3`)

- **`D-V2-1` (Source of `SpeciesProfile` values):** — **DECIDED (2026-11-24, QD-OPENQ): Option A** (compiled-in defaults + optional validated JSON override).
  - **Option A (Recommended):** Ship deterministic compiled-in `SpeciesRegistry::default_vertical_slice()` (`rabbit` & `wolf` presets matching existing calibration baseline) **plus** optional validated JSON override passed via experiment manifest / scenario config and saved in `SavedSimulationState::species_registry`.
  - **Option B:** Require an external config file at startup (blocks standalone unit tests and risks config-path drift).
- **`D-V2-2` (Transition strategy from `Prey`/`Predator` marker components):** — **DECIDED (2026-11-24, QD-OPENQ): Option A** (keep deriving `Prey`/`Predator` markers alongside `AgentSpecies` through `V2–V8`).
  - **Option A (Recommended):** Keep deriving `Prey` / `Predator` ECS markers from `SpeciesProfile::legacy_class` alongside `AgentSpecies(SpeciesId)` throughout `V2–V8` so existing sensors, `combat_system`, `prey_cap` checks, and `AgentType` IPC serialization remain interoperable while food-web edges become data-driven. Cleanly deprecate hardcoded marker branching after `V8`.
  - **Option B:** Remove `Prey` and `Predator` components immediately and rewrite all `30+` system/test queries (`With<Prey>`, `With<Predator>`). Higher churn; unnecessary before multi-predator food webs (`M6.2`).
- **`D-V2-3` (Habitat suitability enforcement mode):** — **DECIDED (2026-11-24, QD-OPENQ): Option A** (habitat suitability filter + `unsuitable_stress_cost` in `total_cost`).
  - **Option A (Recommended):** Filter candidate genesis/seed habitat placement by `HabitatSuitability::is_suitable(biome, elevation, temp)` (`V2` S27 spawn suitability check) **and** apply an additive `unsuitable_stress_cost` inside `metabolic_decay_system`'s `total_cost` when a creature wanders outside its biome/temperature bounds (flowing to `respired -> pool.detritus` so EU stays conserved).
  - **Option B:** Spawn/habitat suitability filter + diagnostic mismatch metric only; no metabolic penalty when stepping into unsuitable biomes.

## 4. Future `V2` Coding Card Boundaries

### Allowed Paths
- `src-tauri/crates/anima-domain/src/lib.rs`, `src-tauri/crates/anima-domain/src/species.rs` (new)
- `src-tauri/src/core/mod.rs`, `src-tauri/src/core/species.rs` (new), `src-tauri/src/core/components.rs` (`AgentSpecies`, `AgentMigrationData::species_id`)
- `src-tauri/src/core/agent_systems.rs` (`SpawnGenotypeCommand`, `SpawnOffspringCommand`, `autonomous_reproduction_system`, `execute_reproduction_birth`)
- `src-tauri/src/core/environmental_systems.rs` (`herbivore_grazing_system`, `detect_environmental_collisions_system`)
- `src-tauri/src/core/world_systems.rs` (`metabolic_decay_system`, `combat_system`, `check_migration_boundaries_system`, `manual_migration_system`, `SpawnMigrationCommand`)
- `src-tauri/src/core/aggregate_population.rs` (`ArchivedIndividual`, `SavedDormantCohorts::validate`, `DehydrateAgentCommand`, `RehydrateCommand`)
- `src-tauri/src/core/simulation_state.rs` (`SerializedAgent`, `SavedSimulationState::species_registry`, `empty_saved_state_for_tests`, `serialize_single_agent`, `serialize_world_state`, `spawn_serialized_agent`)
- `src-tauri/src/core/snapshot.rs` (`validate_state`, conditional sorted-tuple `world_checksum`, test fixtures `:967, 1022`)
- `src-tauri/src/core/simulation_loop.rs` (`ANIMA_SPECIES_PROFILES` init/genesis/restore)
- `src-tauri/src/core/live_experiment.rs` (`LIVE_KEY_SPECIES_PROFILES`, `LIVE_KEYS`, `LiveWorldConfig`, `genesis`, `build_live_world`, optional `LIVE_SPECIES_OBSERVABLE_IDS` + `ensure_observables_supported` while leaving `all_live_observable_ids` untouched)
- `src-tauri/src/core/experiment/observables.rs` (`ObservableRegistry::live_default` entries if opt-in species observables are exposed)
- `src-tauri/tests/species_profile_tests.rs` (new: S27/S33 unit + integration suite, zero-alloc, conservation, checkpoint replay, flag-off bit-identity)
- `LIVE_KEYS.len()` pinning tests (increment count by `+1` relative to trunk at `V2` start; `31` on `1a2d1d62`):
  - `src-tauri/tests/e2_seam_tests.rs`, `src-tauri/tests/eco_card_b_tests.rs`, `src-tauri/tests/founder_genotype_tests.rs`, `src-tauri/tests/founder_layout_tests.rs`, `src-tauri/tests/grass_steering_tests.rs`, `src-tauri/tests/lake_layout_tests.rs`, `src-tauri/tests/land_mask_tests.rs`, `src-tauri/tests/strict_prey_cap_tests.rs`
- Preregistration manifest suites (`LIVE_KEYS` partition):
  - `src-tauri/tests/prereg_e3_manifest_tests.rs` + `src-tauri/tests/fixtures/experiments_e3/e3-preregistration.json`
  - `src-tauri/tests/prereg_e3b_manifest_tests.rs` (+ `src-tauri/tests/fixtures/experiments_e3b/e3b-preregistration.json` only if E3b freeze allows zero-default addition per `scripts/verify_e3b_artifacts.mjs:1100`, otherwise freeze-key scoping per `D-V0-2`)
- Exhaustive struct-literal updates adding `species_id: None` across `10` distinct test files (zero assertion changes):
  - `SerializedAgent` (`3` files): `src-tauri/tests/brain_persistence_tests.rs` (`:40`), `src-tauri/tests/f047_restore_divergence_tests.rs` (`:839`), `src-tauri/tests/living_world_dormancy_tests.rs` (`:58`)
  - `AgentMigrationData` (`6` files, `brain_persistence_tests.rs` shared): `src-tauri/tests/adversarial_challenger_tests.rs` (`:160, 227, 246`), `src-tauri/tests/brain_persistence_tests.rs` (`:87`), `src-tauri/tests/migration_enqueue_atomicity_tests.rs` (`:170`), `src-tauri/tests/migration_high_throughput_tests.rs` (`:74`), `src-tauri/tests/migration_stress_tests.rs` (`:101, 178, 252`), `src-tauri/tests/migration_tests.rs` (`:44, 483, 608, 799, 902`)
  - `ArchivedIndividual` (`1` file): `src-tauri/tests/plant_reserve_tests.rs` (`:491`)
  - `SpawnOffspringCommand` (`1` file): `src-tauri/tests/birth_death_zero_alloc_tests.rs` (`:176, 359, 482`)
- `SavedSimulationState::species_registry: None` is supplied via `empty_saved_state_for_tests()` (`src-tauri/src/core/simulation_state.rs:608`, spread with `..empty_saved_state_for_tests()` by `environmental_elements_stress_tests.rs`, `environmental_elements_tests.rs`, `persistence_stress_tests.rs`, `persistence_tests.rs`)
- `docs/ai/planning/` (card `V2` implementation report note)

## 5. `DONE WHEN`, Negative Controls, Conservation & Determinism Checks

### `DONE WHEN`
1. **S27 (`M5.1`) Species & habitat suitability:** Rabbit and wolf profiles load cleanly in `SpeciesRegistry`; unsuitable-biome placement is rejected by validation/spawn helper, and invalid configs (unknown `prey_targets`, out-of-range fractions, inverted age bounds) fail `SpeciesRegistry::validate()` with explicit errors.
2. **S33 (`M6.1`) Data-driven food-web edges:** Changing `DietSpec::prey_targets` or `grazes_grass` in a custom test registry alters grazing and predation behavior in `herbivore_grazing_system` and `combat_system` without hardcoding `Predator`/`Prey` rules; wolf cannot graze grass, rabbit cannot prey on wolf.
3. **Four-boundary persistence parity:** Save -> restore (`spawn_serialized_agent`), cross-shard migration (`SpawnMigrationCommand`), hibernate -> wake (`WakeAgentCommand`), and dehydrate -> rehydrate (`RehydrateCommand`) preserve `AgentSpecies` and custom `SpeciesRegistry`; legacy fixtures (`species_id: None`) load cleanly.
4. **Closed-EU conservation & zero allocations:** Varying `k_base`, `unsuitable_stress_cost` (if `D-V2-3 = A`), `repro_invest_fraction`, and `assimilation_efficiency` keeps `|closed_eu_residual| <= RESIDUAL_ABS_TOLERANCE_EU` (`1e-3`, `src-tauri/crates/anima-domain/src/energy.rs:123`) over multi-thousand-tick live runs, with `0` heap allocations per tick under `TrackingAllocator`.
5. **Flag-off bit-identity & workspace gates:** With `live.species_profiles` absent/`0`, E2/E3b control seeds produce **bit-exact** `world_checksum` and observables matching pre-`V2` trunk; `cargo fmt --check`, `cargo clippy --features desktop --all-targets -- -D warnings`, `cargo test --features desktop` + `check_test_targets.mjs --profile desktop`, and `npm run check:docs` all exit `0`.

### Negative Controls (Must Fail Red Under Fault Injection, A3 #2)
1. **Prey-list bypass:** Temporarily ignore `DietSpec::prey_targets` inside `combat_system` -> S33 non-target immunity test fails red.
2. **Boundary strip:** Temporarily omit `species_id` copy inside `SpawnMigrationCommand::apply` -> cross-shard species preservation test fails red.
3. **Metabolic leak:** Temporarily deduct `k_base` / `unsuitable_stress_cost` separately from `homeo.energy` outside `total_cost` -> closed-EU conservation test exceeds `RESIDUAL_ABS_TOLERANCE_EU` and fails red.
4. **Default leakage:** Temporarily insert `SpeciesPolicy { enabled: true }` by default -> flag-off bit-identity test fails red.

### Determinism Checks
- **Three-process equality (`F-041`):** With `live.species_profiles = 1.0`, 3 independent processes on the same OS yield identical `world_checksum` across `N` ticks (`SpeciesRegistry` uses sorted slice/vector iteration, zero `HashMap`).
- **Save/load & fork equivalence:** `run(N) == run(K) -> save -> load -> run(N - K)` matches bit-for-bit on `world_checksum`, reserves, and species assignments on the same OS.
