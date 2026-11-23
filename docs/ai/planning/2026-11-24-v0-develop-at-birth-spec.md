---
phase: planning
feature: v1-vertical-slice
title: Card V0 — develop_at_birth and Serialized DevelopedPhenotype (Creature Morphogenesis P1–P2)
description: Implementation spec for card V0 splitting genotype development from ECS spawn, recounting all decode_genotype call sites at 1a2d1d62, and persisting DevelopedPhenotype across save, migration, wake, and rehydration
status: proposed
owner: anima-dell
last_reviewed: 2026-11-24
---

# Card `V0` — `develop_at_birth` & Serialized `DevelopedPhenotype` (Creature Morphogenesis P1–P2)

> **Design spec only (`status: proposed`).** No Rust or TypeScript code is written in D4 (`V-ADR`).
> All `file:line` anchors below are verified against `origin/feat/ecological-selection` at trunk
> commit `1a2d1d62de52334e675108852311a87c3c1bb10f`.

## 1. Goal & Scope

Implement **Phase P1 (Pure Foundation)** and **Phase P2 (Spawn Split & Persistence)** of [`CREATURE_MORPHOGENESIS_PLAN.md`](../../planning/CREATURE_MORPHOGENESIS_PLAN.md) (`CM-101`–`CM-106`, `CM-201`–`CM-209`) and [`CREATURE_DEVELOPMENT_CONTRACT.md`](../../reference/CREATURE_DEVELOPMENT_CONTRACT.md), unblocking card `V2` (`SpeciesProfile`) and `V3` (live lifecycle under `ADR-0013`):

1. Separate environment sampling and pure morphological development (`develop_at_birth`) from ECS entity instantiation (`spawn_developed`) inside a new `evolution/ecomorph.rs` module (`ADR-0001`).
2. Materialize a versioned, serializable `DevelopedPhenotype` once at genesis/birth/evolutionary replacement and persist it across snapshot save/restore, cross-shard migration, hibernation wake, and aggregate-LOD rehydration without ever re-developing an existing creature (`D01`, `D02`, `D09`).
3. Keep the legacy flag-off path (`live.morphogenesis` absent/`0`, `DevelopmentalProgram::default()` slope `0.0`) **bit-identical** to trunk `1a2d1d62`, consuming zero additional `SimRng` draws when disabled.

### Explicit Non-Goals (Deferred to Later Cards / P3–P5)

- **P3 genesis priors & habitat sampling** (`SimulationEngine::start` straight-line/grid founder layout changes, candidate cell search): separate card after P1–P2 (`CM-301`–`CM-308`).
- **P4 active reaction-norm plasticity, pigment inheritance & selection experiments** (`CM-401`–`CM-406`) and **P5 frontend/IPC `SegmentState` changes** (`CM-501`–`CM-507`): keep `SegmentState` (`src-tauri/src/core/simulation_state.rs:23`) and TypeScript types untouched in `V0`.
- **Thermal/hydration physiology (`P6`), reciprocal transplants (`P7`), `SpeciesProfile` (`V2`), and `LifecycleState` (`V3`)**: deferred to their own cards.

## 2. Recount of `decode_genotype` Definitions & Call Sites at `1a2d1d62`

[`CREATURE_DEVELOPMENT_CONTRACT.md`](../../reference/CREATURE_DEVELOPMENT_CONTRACT.md) (`:52-65`, checked `2026-11-17`) documented **7** non-test call sites and explicitly warned agents to re-count before starting (`:65`). Below is the complete, verified inventory at `1a2d1d62`:

### 2.1 Definitions Found Under `src-tauri/src/`

| Location | Status at `1a2d1d62` | What `V0` Does |
|---|---|---|
| `src-tauri/src/evolution/genotype.rs:114` | **Active canonical definition** (`pub fn decode_genotype`) | Refactor body into `spawn_developed` (`ecomorph.rs`); retain `decode_genotype` as a thin legacy-only identity-phenotype wrapper calling `from_genotype_identity` + `spawn_developed` (`CM-205`, deferring `#[deprecated]` to a follow-up test/bench migration card so `clippy --all-targets -- -D warnings` stays clean) |
| `src-tauri/src/core/spawn.rs:50` | **Uncompiled orphan duplicate** (`src-tauri/src/core/mod.rs:1-49` never declares `pub mod spawn;`; lacks `IndexedInGrid(false)`, `LastWalkablePosition`, and BFS reachability in `MorphologyGenotype::total_mass`) | **Do not modify or wire in `V0`** (`CM-205` targets `evolution/genotype.rs:114`). Flag in `V0` PR for a separate dead-file cleanup card |

### 2.2 All `decode_genotype(...)` Call Sites at `1a2d1d62`

Every occurrence across `src-tauri/` was audited (`45` call sites total: `9` inside `src-tauri/src/` — `7` direct production sites + `2` unit tests — plus `1` benchmark site in `src-tauri/benches/` and `35` call sites across `18` integration test files in `src-tauri/tests/`, in addition to `2` definitions in `src-tauri/src/evolution/genotype.rs:114` and orphan `src-tauri/src/core/spawn.rs:50`, and indirect restore via `WakeAgentCommand`):

| # | Call Site (`file:line` at `1a2d1d62`) | Flow / Symbol | Contract Rule (`D01`/`D06`) | Target Transformation in `V0` |
|---|---|---|---|---|
| 1 | `src-tauri/src/core/simulation_loop.rs:1676` | App genesis (`SimulationEngine::start`) | Genesis: develop **once**, then spawn (`D01`, `D06` `t=0` boundary) | Under opt-in flag (`ANIMA_MORPHOGENESIS`): `sample_environment` + `develop_at_birth` using a dedicated setup RNG (`derived_rng(..., sim_stream::LIVE_DEVELOPMENT)`) -> `spawn_developed`, insert `DevelopedPhenotype` component on root. When off: identity `DevelopedPhenotype::from_genotype_identity(&genotype, None)` (`version = 1`, `slope = 0.0`, zero RNG draws) -> `spawn_developed` |
| 2 | `src-tauri/src/core/live_experiment.rs:3108` | Headless experiment genesis (`live_experiment::genesis`) | Genesis: develop **once** (`D01`), zero `SimRng` shift when off | When `config.morphogenesis` is enabled: `sample_environment` -> `develop_at_birth(&genotype, env, &mut dev_rng)` with dedicated `sim_stream::LIVE_DEVELOPMENT` (isolated from `SimRng` and `LIVE_GENESIS_BRAINS`) -> `spawn_developed`. When off (`false`): identity `DevelopedPhenotype` (zero RNG draws, bit-identical to trunk) |
| 3 | `src-tauri/src/core/agent_systems.rs:392` | Evolutionary replacement (`SpawnGenotypeCommand::apply`) | Replacement: develop **once**, fund reserve from `Detritus` (`D06`, `:442-468`) | When morphogenesis enabled: sample env at `self.initial_pos` + `develop_at_birth` -> `spawn_developed`, preserving `EnergyEvent::Replacement` detritus debit (`:455-462`) and `BrainPolicy::new_brain` (`:477-488`). When off: identity `DevelopedPhenotype` -> `spawn_developed` |
| 4 | `src-tauri/src/core/agent_systems.rs:514` | Biological birth (`SpawnOffspringCommand::apply`) | Birth: develop **once**, reserve from parent debit (`D06`, `:1090-1123`) | **Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub phenotype: Option<DevelopedPhenotype>` on `SpawnOffspringCommand` (`agent_systems.rs:494`)**: in `execute_reproduction_birth`, `spawn_pos` (`:1140`) is snapped for `land_nav` at `:1145-1155`, so sample environment **after** `:1155`; when `morphogenesis` enabled, develop once via dedicated `sim_stream::LIVE_DEVELOPMENT` + deterministic birth counter (`D07`, never shifting `SimRng`) and pass `Some(...)` (when off, pass `None` — `0` extra `Vec` allocations against `src-tauri/tests/birth_death_zero_alloc_tests.rs:553`); call `spawn_developed`; preserve `LiveLineage::record_birth` (`:522-535`) |
| 5 | `src-tauri/src/core/simulation_state.rs:936` | Snapshot restore (`spawn_serialized_agent`, called by `SimulationLoop` `:1475`, `live_experiment::build_live_world` `:2795`, and `WakeAgentCommand::apply` `agent_systems.rs:348`) | **Restore / Wake: MUST NOT develop (`D01`, `D02`, `D09`)** | Read `agent.phenotype`: if `Some(p)`, validate via `p.validate()` and call `spawn_developed` verbatim. If `None` (legacy save `D09`), materialize identity `DevelopedPhenotype` once with `DevelopmentalProgram::default()` (`slope = 0.0`) from `agent.genotype` — never calling environment-dependent plasticity — then `spawn_developed`, restoring exact segment kinematics (`:1013-1037`, including `osc.frequency`, `amplitude`, `output` at `:1031-1037`) |
| 6 | `src-tauri/src/core/world_systems.rs:1003` | Cross-shard migration (`SpawnMigrationCommand::apply`) | **Migration: MUST NOT develop (`D01`, `D02`, `CM-S04`)** | Validate `self.data.phenotype` inside `AgentMigrationData::validate` (`components.rs:166`). Use saved phenotype (or identity fallback when `None` for wire backwards-compatibility `D09`) in `spawn_developed`. Destination environment may only be sampled to log habitat mismatch warnings, never to resize/recolor body |
| 7 | `src-tauri/src/core/aggregate_population.rs:1187` | Aggregate-LOD rehydration (`RehydrateCommand::apply`) | **Rehydrate: MUST NOT develop (`D01`, `D02`)** | Store `Option<DevelopedPhenotype>` on `ArchivedIndividual` (`aggregate_population.rs:224`) when `DehydrateAgentCommand::apply` (`aggregate_population.rs:1039`, literal at `:1122`; queued by `dehydrate_cold_agents_system` at `:1270`) archives a resident individual; pass stored phenotype (or legacy identity fallback when `None`) directly to `spawn_developed`; preserve cohort EU drain (`:1234-1261`) |
| 8 | `src-tauri/src/evolution/genotype.rs:362` | Unit test `total_mass_matches_the_body_decode_genotype_spawns` | Regression test locking BFS reachability mass == spawned mass | Keep `decode_genotype` call green via legacy wrapper (`CM-205`), and add twin test asserting `DevelopedPhenotype::total_mass == spawned_segment_mass` |
| 9 | `src-tauri/src/core/terrain_navigation.rs:400` | In-crate `#[cfg(test)]` `shoreline_reversion_moves_whole_body_and_respects_wrap_seam` | Unit test for shoreline wrap reversion | Keep green via legacy `decode_genotype` wrapper (`CM-205`) |
| 10 | `src-tauri/benches/live_schedule_scaling.rs:79` | Benchmark setup (`benches/live_schedule_scaling.rs`) | Benchmark harness fixture | Keep green via legacy `decode_genotype` wrapper (`CM-205`, no `#[deprecated]` warning under `--all-targets`) |
| 11 | `src-tauri/tests/*.rs` (`35` calls across all `18` files): `action_gates_tests.rs:371`, `adversarial_ecology_stress_tests.rs:241,266`, `aggregate_population_tests.rs:138`, `birth_death_zero_alloc_tests.rs:82,240,409`, `brain_cost_and_coupling_tests.rs:229,363,384`, `challenger_trophic_stress_tests.rs:231,297`, `determinism_gate_tests.rs:105`, `ecology_closed_ledger_integration_tests.rs:118`, `empirical_m2_challenger_tests.rs:143,156`, `energy_conservation_tests.rs:131`, `f047_restore_divergence_tests.rs:517`, `genotype_tests.rs:27,108,200,271,330,369`, `land_mask_tests.rs:815,893,994`, `multi_trophic_coexistence_tests.rs:241,312`, `post_194_hardening_audit_tests.rs:668`, `production_ecology_adversarial_stress_tests.rs:246,271`, `production_ecology_e2e_tests.rs:275,300`, `snapshot_checkpoint_tests.rs:95` | Integration & stress test harnesses | Zero assertion loosening; keep compiles & green | Keep all `18` `src-tauri/tests/` callers compiling via legacy `decode_genotype` wrapper (`CM-205`, `CREATURE_MORPHOGENESIS_PLAN.md:131`); add `ecomorph_tests.rs` specifically exercising `sample_environment`, `develop_at_birth`, `spawn_developed`, restore, and migration |

## 3. Architecture & Exact Integration Anchors (`1a2d1d62`)

### 3.1 Pure Morphogenesis & Spawn Split (`evolution/ecomorph.rs` & `evolution/mod.rs:1-11`)

Create `src-tauri/src/evolution/ecomorph.rs` and export `pub mod ecomorph;` in `src-tauri/src/evolution/mod.rs:1-11`:

1. **`EnvSample`, `EnvironmentSnapshot`, and `EcomorphError` (`CM-102`, `CM-103`):**
   - `sample_environment(terrain: &TerrainMap, resource: Option<&ResourceField>, bounds: &MapBounds, position: Vec3) -> Result<EnvSample, EcomorphError>`
   - Uses existing `TerrainMap` (`src-tauri/src/core/terrain.rs:129`), `ResourceField` (`src-tauri/src/core/ecology.rs:331`), and `MapBounds`.
   - Enforces Contract `D10`: positions outside `[bounds.min, bounds.max]` (or non-finite coordinates) return `Err(EcomorphError::OutOfBounds)` — never fabricates a default ocean cell; reads `resource_capacity` (`r_max`) and `standing_resource` (`r`) separately when `resource` is `Some`.
   - `EnvironmentSnapshot` captures the scalar snapshot embedded in `DevelopedPhenotype.birth_environment` (`CREATURE_DEVELOPMENT_CONTRACT.md:31`).
2. **Genotype Extensions (`src-tauri/src/evolution/genotype.rs:21-51`, `CM-104`):**
   - Define `DevelopmentalProgram { pub mass_temp_slope: f32, pub appendage_temp_slope: f32, pub pigment_moisture_slope: f32 }` with `Default` yielding all `0.0` (so `slope == 0.0` produces an identity transformation on old saves, `D09`), `LocomotionMedium` + `HabitatPreference` (MVP `Default` terrestrial + wide elevation bounds), and `Option<[f32; 3]> pigment` (`None` by default, `D09`).
   - Attach these cleanly via `#[serde(default, skip_serializing_if = "Option::is_none")]` (or explicit `skip_serializing_if` for zero defaults) on new `MorphologyGenotype` fields (`genotype.rs:21`) rather than plain `#[serde(default)]`, so legacy snapshot/genotype JSON bytes stay unchanged when morphogenesis is off; update `MorphologyGenotype::is_bit_identical` (`genotype.rs:31`) so mutation-reporting clones compare identically.
   - Ensure `mutate_genotype` / `mutate_genotype_reporting` (`src-tauri/src/evolution/mutation.rs:70`, `:87`) and `crossover_genotypes` (`src-tauri/src/evolution/crossover.rs:32`) already receive `&mut R` RNG (verified at `CM-106`); keep default mutation/crossover paths untouched when developmental slopes are zero/legacy unless bounded developmental mutation is enabled.
3. **`DevelopedPhenotype`, Validator, and `develop_at_birth` (`CM-201`–`CM-203`):**
   - `DevelopedPhenotype` (`#[derive(Component, Clone, Debug, Serialize, Deserialize)]`) stores `version: u16` (`1`), `birth_environment: EnvironmentSnapshot`, `nodes: Vec<DevelopedNode>`, `edges: Vec<DevelopedEdge>` (including developed segment `length`, `radius`, `mass`, and `joint_anchor` clamped/scaled consistently with parent segment dimensions per `D05`!), `total_mass: f32` (BFS-reachable sum matching spawned segments), `habitat: HabitatPreference`, and `display_color: Option<[f32; 3]>`.
   - Pure `develop_at_birth(genotype: &MorphologyGenotype, environment: EnvSample, rng: &mut impl rand::Rng) -> Result<DevelopedPhenotype, EcomorphError>` applies reaction norms at most **once** (`D03`), clamps segment geometry positively, and recomputes child `joint_anchor` offsets consistently with parent growth (`D05` / `CM-S06`).
   - Provide `DevelopedPhenotype::from_genotype_identity(genotype: &MorphologyGenotype, environment: Option<EnvironmentSnapshot>) -> Result<Self, EcomorphError>` (zero RNG draws, `0.0` slopes, bit-exact geometry parity with `decode_genotype`).
4. **`spawn_developed` & `decode_genotype` Wrapper (`CM-204`–`CM-205`):**
   - Preserve every invariant and component currently inserted by `evolution::genotype::decode_genotype` (`src-tauri/src/evolution/genotype.rs:114-274`):
     - Root-finding and BFS traversal order over reachable nodes (`:145-153`, `:201-271`) so `total_mass` matches `genotype.total_mass()` (`:64-96`).
     - Exact ECS component bundle parity on root & children, **critically** including `crate::physics::spatial::IndexedInGrid(false)` on both root (`:184`) and child (`:259`), `ActionGates::default()` (`:197`), and `LastWalkablePosition::new(initial_pos)` (`:198`).
     - Attach `DevelopedPhenotype` component onto the spawned root entity so `serialize_world_state` (`simulation_state.rs:1151`), `serialize_single_agent` (`:1047`), outbound migration (`world_systems.rs:728`, `:905`), dehydration (`DehydrateAgentCommand::apply` at `aggregate_population.rs:1039, 1122`, queued by `dehydrate_cold_agents_system` at `:1270`), and future `MAP-Elites`/renderer (`P4`–`P5`) read it directly without re-developing.
   - Refactor `evolution::genotype::decode_genotype` (`:114`) into a thin legacy-only wrapper calling `from_genotype_identity` + `spawn_developed` (keeping `CM-205` zero behavior change for tests/benches, while deferring `#[deprecated]` to a later migration card so `cargo clippy --all-targets -- -D warnings` passes cleanly).

### 3.2 Dedicated Seed Stream & Opt-In Gate (`resources.rs`, `live_experiment.rs`)

- **RNG stream isolation (`src-tauri/src/core/resources.rs:925-948`):** Currently `sim_stream` defines constants `1..=5` (`WORLD_INIT=1`, `EVOLUTION=2`, `LIVE_GENESIS_BRAINS=3`, `LIVE_DRIFT_SELECTION=4`, `LIVE_GENESIS_FOUNDERS=5`), and `DORMANCY_STREAM = 7` lives at `aggregate_population.rs:190`. Reserve **`pub const LIVE_DEVELOPMENT: u64 = 6;`** inside `sim_stream` (avoiding collision with `7`) so toggling development never shifts `SimRng`, brain weights, or founder scatter draws (`D07`, `ADR-0008`).
- **Opt-in configuration:** Add `LIVE_KEY_MORPHOGENESIS: &str = "live.morphogenesis"` to `LIVE_KEYS` (`src-tauri/src/core/live_experiment.rs:280`) and `#[serde(default)] pub morphogenesis: bool` to `LiveWorldConfig` (`:316`). Note that bumping `LIVE_KEYS` array length updates `[&str; 31]` -> `[&str; 32]` cleanly without changing any key parsing when absent (`false`). For desktop, gate behind `ANIMA_MORPHOGENESIS` (absent/`0`/`false` = off).

### 3.3 Serialization Across All Four Persistence Boundaries (`CM-206`–`CM-209`)

1. **`SerializedAgent` (`src-tauri/src/core/simulation_state.rs:105`, `:196`, `:922`, `:1047`, `:1280` & `snapshot.rs:512`):**
   - Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub phenotype: Option<DevelopedPhenotype>` (note: omitted when `None` so existing byte fixtures and schema-6/7/8 tests stay untouched).
   - Validate in `SerializedAgent::validate` (`:196`) when `Some` (positive finite dimensions, reachable node/edge parity with `self.genotype`, valid `version <= CURRENT_PHENOTYPE_VERSION`, `MAX_MIGRATION_MORPHOLOGY_NODES` bound at `components.rs:192`). Also validate `state.dormant_agents` (`DormantAgentRecord.agent.validate()`) alongside `state.agents` inside `snapshot::validate_state` (`src-tauri/src/core/snapshot.rs:512`), since `DormantAgents` restores `SerializedAgent` records (`simulation_loop.rs:1491`, `simulation_state.rs:1484`).
   - Populate in **both** `serialize_single_agent` (`:1047`, used by `HibernateAgentCommand` -> `DormantAgentRecord` -> `WakeAgentCommand`) and `serialize_world_state` (`:1280`, full world snapshot).
   - Read and restore in `spawn_serialized_agent` (`:922`) — **never** calling `develop_at_birth` (`D01`); when `None`, construct legacy identity phenotype with slope `0.0` (`D09`).
2. **`AgentMigrationData` (`src-tauri/src/core/components.rs:166`, `world_systems.rs:728, :905, :980`):**
   - Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub phenotype: Option<DevelopedPhenotype>`.
   - Populate at **both** outbound migration sites (`check_migration_boundaries_system` at `world_systems.rs:728` and `manual_migration_system` at `world_systems.rs:905`) from the migrating entity's `DevelopedPhenotype` component.
   - Validate inside `AgentMigrationData::validate` (`components.rs:419`) and consume verbatim inside `SpawnMigrationCommand::apply` (`world_systems.rs:980`) — ignoring destination climate/biome for morphology/pigment (`CM-S04`).
3. **`ArchivedIndividual` (`src-tauri/src/core/aggregate_population.rs:224`, `:431`, `:1039`, `:1122`, `:1164`, `:1270`):**
   - Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub phenotype: Option<DevelopedPhenotype>`, captured in `DehydrateAgentCommand::apply` (`:1039`, literal at `:1122`; queued by `dehydrate_cold_agents_system` at `:1270`), validated inside `SavedDormantCohorts::validate()` (`:431`), and restored verbatim in `RehydrateCommand::apply` (`:1164`).
4. **Snapshot Schema Compatibility (`src-tauri/src/core/snapshot.rs:58, :68`):**
   - Keep `MIN_SUPPORTED_SCHEMA = SCHEMA_VERSION - 2` working against schema 6–8 fixtures. See **Open Decision `D-V0-1`** below regarding whether to bump `SCHEMA_VERSION` from `8` to `9` or land purely additively under schema `8` (following the `live_lineage` pattern at `simulation_state.rs:589-594`).

## 4. Open Owner Decisions

- **`D-V0-1` (Snapshot schema bump vs additive under schema 8):**
  - **Option A (Recommended):** Keep `SCHEMA_VERSION = 8` (`src-tauri/src/core/snapshot.rs:58`) during P1–P2 and carry `#[serde(default, skip_serializing_if = "Option::is_none")] pub phenotype: Option<DevelopedPhenotype>` additively (exact precedent: `live_lineage` at `simulation_state.rs:589-594` and `live_id` at `:143-144`). Zero breakage of existing schema 6/7/8 tests (`MIN_SUPPORTED_SCHEMA = 6`), while unknown future `phenotype.version` fails validation loudly.
  - **Option B:** Bump `SCHEMA_VERSION` from `8` to `9` (`MIN_SUPPORTED_SCHEMA` rises to `7`, dropping schema-6 backward support). Only choose if policy mandates a schema bump whenever any field is added to `SerializedAgent`.
- **`D-V0-2` (Frozen E3b `LIVE_KEYS` partition handling when `LIVE_KEYS.len()` becomes `32`):**
  - **Option A (Recommended):** Add `"live.morphogenesis": 0` to `default_keys_left_absent` in `src-tauri/tests/fixtures/experiments_e3b/e3b-preregistration.json` (explicitly permitted by `scripts/verify_e3b_artifacts.mjs:1104-1118` `diffPreregAgainstHeadAllowingAddedDefaultZeros`) and update `src-tauri/tests/prereg_e3b_manifest_tests.rs:417-426`.
  - **Option B:** Narrow `src-tauri/tests/prereg_e3b_manifest_tests.rs:212-217, 417-426` to assert the `31` freeze-time keys so frozen `experiments_e3b/` stays byte-untouched while E3b runs.

## 5. Future `V0` Coding Card Boundaries

### Allowed Paths
- `src-tauri/src/evolution/ecomorph.rs` (new)
- `src-tauri/src/evolution/mod.rs`
- `src-tauri/src/evolution/genotype.rs`
- `src-tauri/src/evolution/mutation.rs`, `src-tauri/src/evolution/crossover.rs` (only if `morphogenesis` metadata passthrough requires it)
- `src-tauri/src/core/resources.rs` (`sim_stream::LIVE_DEVELOPMENT`, policy resource)
- `src-tauri/src/core/components.rs` (`AgentMigrationData`, validation)
- `src-tauri/src/core/simulation_state.rs` (`SerializedAgent`, `serialize_single_agent`, `serialize_world_state`, `spawn_serialized_agent`)
- `src-tauri/src/core/agent_systems.rs` (`SpawnGenotypeCommand`, `SpawnOffspringCommand`, `execute_reproduction_birth`)
- `src-tauri/src/core/aggregate_population.rs` (`ArchivedIndividual`, `SavedDormantCohorts::validate`, `DehydrateAgentCommand::apply`, `RehydrateCommand`)
- `src-tauri/src/core/world_systems.rs` (`check_migration_boundaries_system`, `manual_migration_system`, `SpawnMigrationCommand`)
- `src-tauri/src/core/simulation_loop.rs` (`SimulationEngine::start`)
- `src-tauri/src/core/live_experiment.rs` (`LIVE_KEYS`, `LiveWorldConfig`, `live_experiment::genesis`)
- `src-tauri/src/core/snapshot.rs` (`validate_state` for `agents` + `dormant_agents` `phenotype` + conditional `world_checksum` if enabled; only touch `SCHEMA_VERSION` if `D-V0-1 = B`)
- `src-tauri/tests/ecomorph_tests.rs` (new) + contract table row update in `docs/reference/CREATURE_DEVELOPMENT_CONTRACT.md` (`:81-91`) + planning result note under `docs/ai/planning/`
- `LIVE_KEYS.len()` pinning tests (`31 -> 32` count update only): `src-tauri/tests/e2_seam_tests.rs` (`:158`), `eco_card_b_tests.rs` (`:452`), `founder_genotype_tests.rs` (`:75`), `founder_layout_tests.rs` (`:174`), `grass_steering_tests.rs` (`:100`), `lake_layout_tests.rs` (`:71`), `land_mask_tests.rs` (`:150`), `strict_prey_cap_tests.rs` (`:73`)
- Preregistration manifest suites: `src-tauri/tests/prereg_e3_manifest_tests.rs` (`:143-150`) + `src-tauri/tests/fixtures/experiments_e3/e3-preregistration.json` (`default_keys_left_absent` zero-default amendment), and `src-tauri/tests/prereg_e3b_manifest_tests.rs` (`:212-217, 417-426`) (+ `src-tauri/tests/fixtures/experiments_e3b/e3b-preregistration.json` only if `D-V0-2 = A`)
- `15` distinct test files with exhaustive struct literals (`16` per-struct bullets below because `src-tauri/tests/brain_persistence_tests.rs` constructs both `SerializedAgent` and `AgentMigrationData`; note `adversarial_challenger_tests.rs:227,246` and `migration_stress_tests.rs:252` use `..agent.clone()`; strictly adding `phenotype: None` / default fields, zero assertion changes):
  - `SerializedAgent`: `src-tauri/tests/brain_persistence_tests.rs` (`:40`), `f047_restore_divergence_tests.rs` (`:839`), `living_world_dormancy_tests.rs` (`:58`)
  - `AgentMigrationData`: `src-tauri/tests/adversarial_challenger_tests.rs` (`:160, 227, 246`), `brain_persistence_tests.rs` (`:87`), `migration_enqueue_atomicity_tests.rs` (`:170`), `migration_high_throughput_tests.rs` (`:74`), `migration_stress_tests.rs` (`:101, 178, 252`), `migration_tests.rs` (`:44, 483, 608, 799, 902`)
  - `ArchivedIndividual`: `src-tauri/tests/plant_reserve_tests.rs` (`:491`)
  - `MorphologyGenotype`: `src-tauri/tests/lineage_compaction_tests.rs` (`:24`), `lineage_mrca_tests.rs` (`:51`), `lineage_mutation_count_tests.rs` (`:32`), `lineage_simplify_tests.rs` (`:48`), `newick_export_tests.rs` (`:51`)
  - `SpawnOffspringCommand`: `src-tauri/tests/birth_death_zero_alloc_tests.rs` (`:176, 359, 482`)

## 6. DONE WHEN, Negative Controls & Determinism Checks

### DONE WHEN
1. `CM-S01`: `sample_environment` returns accurate terrain/resource cell fields (`resource_capacity` distinct from `standing_resource`) and `Err(EcomorphError::OutOfBounds)` outside `MapBounds`.
2. `CM-S02`: Identical `(genotype, env, seed)` inputs produce bit-identical `DevelopedPhenotype`; same genotype developed in cold vs warm `EnvSample` (with non-zero slope) diverges deterministically while never mutating `genotype` (`CM-S05`).
3. `CM-S03`: Save -> restore (`spawn_serialized_agent`) and hibernate -> wake (`HibernateAgentCommand` / `WakeAgentCommand`) and dehydrate -> rehydrate (`RehydrateCommand`) preserve `DevelopedPhenotype` (mass, node/edge geometry, `birth_environment`, `display_color`, `version`) even when saved into a modified terrain/climate world; legacy saves with `phenotype: None` load identically to trunk.
4. `CM-S04`: Cross-shard migration (`SpawnMigrationCommand`) into a biome/temperature opposite from birth retains exact origin `DevelopedPhenotype`.
5. `CM-S06`: Geometry validator rejects non-positive/non-finite dimensions, cyclic/disconnected topologies, and verifies `Segment.length`/`radius`/`RigidBody.mass`/`SpatialCollider.radius`/`JointConstraint.anchor_offset`/child placement remain mutually coherent.
6. `CM-S08`: Genesis, birth, evolutionary replacement, restore, wake, rehydrate, and migration keep closed-EU residual `<= 1e-9`.
7. Workspace gates green under `CARGO_BUILD_JOBS=2`: `cargo fmt --check`, `cargo clippy --features desktop --all-targets -- -D warnings`, full `cargo test --features desktop` + `node scripts/check_test_targets.mjs <capture> --profile desktop` exit `0`, plus `npm run check:docs` green.

### Negative Controls (Must Fail Red Under Fault Injection, A3 #2)
1. **Out-of-bounds silence fault:** Change `sample_environment` out-of-bounds branch to return a synthetic default ocean `EnvSample` -> `CM-S01` fails red.
2. **Double-development on restore fault:** Temporarily make `spawn_serialized_agent` re-run `develop_at_birth` using target world's terrain instead of saved `agent.phenotype` -> `CM-S03` fails red.
3. **Migration biome-overwriting fault:** Temporarily re-develop inside `SpawnMigrationCommand::apply` using destination temperature -> `CM-S04` fails red.
4. **Anchor-desync fault (`D05`):** Mutate segment length in `develop_at_birth` without updating `JointConstraint.anchor_offset` -> `CM-S06` fails red.
5. **`IndexedInGrid` parity fault:** Omit `IndexedInGrid(false)` when rewriting `decode_genotype` -> `spawn_developed` parity test fails red.

### Determinism Checks
- **Flag-off parity vs trunk `1a2d1d62`:** Run E2/E3b control seeds with `live.morphogenesis` absent -> **bit-exact** `world_checksum` and zero `SimRng` stream position shift vs pre-`V0` baseline on the same OS.
- **Flag-on multi-process & save/load determinism:** With `live.morphogenesis = 1.0`, run 3 separate processes at fixed seeds -> bit-identical `world_checksum`; verify `run(N) == run(K) -> save -> load -> run(N - K)` matches bit-for-bit on the same OS.
