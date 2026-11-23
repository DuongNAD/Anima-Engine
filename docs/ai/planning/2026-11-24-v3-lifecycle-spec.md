---
phase: planning
feature: v1-vertical-slice
title: Card V3 — Live Creature Lifecycle Implementation Spec (Per ADR-0013)
description: Implementation spec for card V3 adding chronological age, life stages, maturation, costly reproduction cooldowns, senescence mortality, DemographicTracker fix, and closed-EU persistence under ADR-0013
status: proposed
owner: anima-dell
last_reviewed: 2026-11-24
---

# Card `V3` — Live Creature Lifecycle (`M5.2`–`M5.3`, Per `ADR-0013`)

> **Design spec only (`status: proposed`).** Do **not** implement `V3` until `ADR-0013` is accepted with
> owner decisions `QD-LIFECYCLE` (`D1–D5`) and prerequisites `V0` + `V2` are merged on trunk.
> All `file:line` anchors below are verified against `origin/feat/ecological-selection` at trunk
> commit `1a2d1d62de52334e675108852311a87c3c1bb10f`.

## 1. Goal, Dependencies & Scope

Implement the concrete engine mechanics specified in `docs/decisions/ADR-0013-live-creature-lifecycle.md` `(PR #235, VERIFIED, pending lead merge)` (`Option A` baseline, `WORLD_SIMULATION_PLAN.md` §5.5, §5.7, §7 `M5.2`–`M5.3`, and `2026-11-05-swarm-master-plan.md` Đợt 6):

1. Separate chronological biological age (`LifecycleState.age_ticks`) from `AgentEvaluation::survival_ticks` (`src-tauri/src/core/agent_systems.rs:76, 1273-1284`) so age advances monotonically even during zero-energy grace ticks and survives all restore boundaries verbatim.
2. Enforce developmental stages (`Juvenile -> Adult -> Senescent`), maturation surcharge and senescence frailty surcharge inside `metabolic_decay_system`'s `total_cost` (`src-tauri/src/core/world_systems.rs:157`), maturity gating + per-species cooldowns in `autonomous_reproduction_system` (`agent_systems.rs:704`), and `DeathCause::Senescence` in `mortality_system` (`src-tauri/src/core/mortality.rs:32, 187-242`).
3. Update `VitalStatistics` (`src-tauri/src/core/resources.rs:474-570`) and fix `DemographicTracker` (`src-tauri/src/core/telemetry_export.rs:81`, `mortality.rs:173-180`) + `demographic_census_system` (`src-tauri/src/core/environmental_systems.rs:391-422`) when lifecycle is active, while keeping all flag-off trunk runs **100% bit-identical**.

### Exact Dependency Order

- **Depends directly on:**
  1. **Accepted `ADR-0013` (`QD-LIFECYCLE`, `D1–D5`)** (`docs/decisions/ADR-0013-live-creature-lifecycle.md`, PR `#235`, `VERIFIED`, pending lead merge).
  2. **`V0`** (`docs/ai/planning/2026-11-24-v0-develop-at-birth-spec.md`, PR `#235`) — ensures offspring born in `SpawnOffspringCommand` (`agent_systems.rs:514`) develop once at birth rather than via legacy `decode_genotype`.
  3. **[Card `V2`](2026-11-24-v2-species-profile-spec.md)** (`SpeciesProfile`) — supplies per-species `maturity_age_ticks`, `senescence_age_ticks`, `max_age_ticks`, `reproduction_cooldown_ticks`, `litter_size`, `maturation_surcharge`, and `senescence_surcharge`.
- **Unblocks:**
  - **[Card `V4`](2026-11-24-v4-carcasses-decomposition-spec.md)** (mortality from starvation, dehydration, and natural senescence feeds the carcass pool).
  - Transitively `V8` (rabbit fast-maturing vs wolf slow-maturing predator–prey dynamics).

### Explicit Non-Goals

- **No physical collider/segment scaling at maturation:** body dimensions stay fixed from birth (`ADR-0013` `D3 = A` & `ADR-0001` `D05`).
- **No carcass delay pool:** `DeathCause::Senescence` routes through `ReclaimAndDespawnAgentCommand` (`agent_systems.rs:168`); delayed decomposition is implemented in [Card `V4`](2026-11-24-v4-carcasses-decomposition-spec.md).
- **No bi-parental mating unless `ADR-0013` `D1 = B` is selected by the owner.**

## 2. Current Code Anchors (`1a2d1d62`) & Implementation Mechanics

### 2.1 Components & Policy Resource (`core/components.rs`, `core/resources.rs`)

- **`LifeStage` (`enum`):** `Juvenile`, `Adult`, `Senescent` (`Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize`).
- **`LifecycleState` (`Component`):**
  - `pub age_ticks: u64`
  - `pub stage: LifeStage`
  - `pub reproduction_cooldown: u32`
  - `pub offspring_count: u32`
- **`LifecyclePolicy` (`Resource`, opt-in, default absent/`enabled: false`):**
  - Mirrors `MortalityPolicy` / `ReproductionPolicy` (`src-tauri/src/core/resources.rs:450, 614`): `pub enabled: bool`, fallback life-history defaults when an agent lacks `AgentSpecies` (`V2`).
  - Opt-in via `LIVE_KEY_LIFECYCLE: &str = "live.lifecycle"` (`LIVE_KEYS` in `live_experiment.rs:280`, `LiveWorldConfig::lifecycle: bool` at `:316`) and `ANIMA_LIFECYCLE` in `simulation_loop.rs:1390`.

### 2.2 Schedule Slot Inside `build_tick_schedule` (`src-tauri/src/core/simulation_schedule.rs:121-202`)

Place `lifecycle_age_system` in **tuple 4** (`simulation_schedule.rs:177-185`) immediately **before** `metabolic_decay_system`:

```text
seed_dropping_system (:177)
  -> combat_system (:178)
  -> lifecycle_age_system (NEW V3 slot — mutates &mut LifecycleState only, 0 Commands)
  -> metabolic_decay_system (:179)
  -> hrrl_learning_system (:183)
  -> crate::core::mortality::mortality_system (:184)
  -> apply_deferred (:185)
```

*(Consistency note: aligns with `ADR-0013` Option A §1 placing `lifecycle_age_system` immediately before `metabolic_decay_system` so `metabolic_decay_system` reads the agent's current-tick `LifeStage` on the same tick transitions occur.)*
Update `src-tauri/tests/schedule_ambiguity_tests.rs:241-307` (`assert_in_order(&order, &["combat_system", "lifecycle_age_system", "metabolic_decay_system", "mortality_system"])`) and verify `the_tick_schedule_has_no_ambiguous_pair_at_all` (`:201`) stays `0` ambiguous pairs.

**Behavior of `lifecycle_age_system`:**
1. Early return without mutating any entity when `LifecyclePolicy` is absent or `!policy.enabled`.
2. Advance `age_ticks = age_ticks.saturating_add(1)`, `reproduction_cooldown = reproduction_cooldown.saturating_sub(1)`, and compute `stage` deterministically against the agent's `SpeciesProfile.life_history` (`age >= senescence_age_ticks => Senescent`, `age >= maturity_age_ticks => Adult`, else `Juvenile`). Zero heap allocations, zero `SimRng` draws.

### 2.3 Spawn Initialization vs Restore/Migration/Wake/Rehydrate

| Lifecycle Event | Trunk Anchor (`1a2d1d62`) | Required `LifecycleState` Initialization (When Enabled) |
|---|---|---|
| Desktop / Headless genesis founders | `simulation_loop.rs:1676`, `live_experiment.rs:3108` | `LifeStage::Adult`, `age_ticks = profile.life_history.maturity_age_ticks`, `reproduction_cooldown = 0`, `offspring_count = 0` |
| Biological birth | `agent_systems.rs:514` (`SpawnOffspringCommand`), `:1059` (`execute_reproduction_birth`) | `LifeStage::Juvenile`, `age_ticks = 0`, `reproduction_cooldown = 0`, `offspring_count = 0`; increment parent's `offspring_count += 1` and set parent's `reproduction_cooldown = profile.life_history.reproduction_cooldown_ticks` **only after** `execute_reproduction_birth` returns `true` (`:1016-1228`) |
| Evolutionary replacement | `agent_systems.rs:392` (`SpawnGenotypeCommand`) | New individual matching genesis mature baseline (`Adult` at `maturity_age_ticks`, `offspring_count = 0`), clearly separated from biological birth (`D06`) |
| Snapshot restore | `simulation_state.rs:922` (`spawn_serialized_agent`) | Restore saved `Option<LifecycleState>` **verbatim** (`D01`/`D02`); if `None` in a lifecycle-enabled world, apply explicit deterministic legacy migration (`Adult` at `maturity_age_ticks`), never resetting to `Juvenile` age `0` |
| Cross-shard migration | `components.rs:166`, `world_systems.rs:728, 905, 980` | Populate in both outbound systems, validate (`validate`: valid stage/sane age vs policy), restore verbatim in `SpawnMigrationCommand` |
| Hibernation wake | `agent_systems.rs:205, 309` (`DormantAgentRecord` -> `WakeAgentCommand` -> `spawn_serialized_agent`) | Carried via `SerializedAgent` + `serialize_single_agent` (`simulation_state.rs:1047`) verbatim |
| Aggregate-LOD dehydrate / rehydrate | `aggregate_population.rs:224, 1122, 1164` (`ArchivedIndividual`, `DehydrateAgentCommand`, `RehydrateCommand`) | Save `Option<LifecycleState>` on `ArchivedIndividual`, validate in `SavedDormantCohorts::validate` (`:380`), restore verbatim in `RehydrateCommand` (fixing current `survival_ticks: 0` reset at `:1201`) |

### 2.4 Metabolic Coupling Inside `total_cost` (`world_systems.rs:85-225`)

In `metabolic_decay_system` (`world_systems.rs:130-157`):
- Query `Option<&LifecycleState>`, `Option<&AgentSpecies>`, `Option<Res<LifecyclePolicy>>`, `Option<Res<SpeciesRegistry>>`.
- Compute `lifecycle_cost` **only** when `policy.enabled`:
  - `Juvenile` -> `profile.life_history.maturation_surcharge`
  - `Adult` -> `0.0`
  - `Senescent` -> `profile.life_history.senescence_surcharge`
- Sum inside `total_cost`:
  `let total_cost = k_base + maintenance + activity_cost + cognition_cost + lifecycle_cost;` (`:157`, using branch when `!policy.enabled` so trunk `f32` rounding stays unaltered).
- Because `decay = total_cost * dt` (`:174`) and `respired += (before - homeo.energy) as f64` (`:177`) settle into `pool.detritus` (`:222-224`), all maturation and senescence metabolic charges remain closed-EU exact by construction.

### 2.5 Costly Reproduction Gate (`agent_systems.rs:704-1229`)

When `LifecyclePolicy` is enabled:
- In `autonomous_reproduction_system` (`:989-1020`): skip any candidate without `LifecycleState` or where `lifecycle.stage != LifeStage::Adult` or `lifecycle.reproduction_cooldown > 0` (`Juvenile` and post-reproductive `Senescent` cannot breed).
- Ensure candidate sorting and running `prey_cap` / `pred_count` accounting (`strict_cap` at `:744`) filter out immature/cooldown agents **deterministically before** choosing who breeds, so cooldown/juveniles never waste cap slots or silently change reproduction order.
- Note SEL2-D neutral-drift replay compatibility (`drift_control::Replay` inside `autonomous_reproduction_system` and `src-tauri/src/core/mortality.rs:127-137`, which currently also applies `lifespan >= 300 => "Senescence"`): keep neutral-drift replay strictly decoupled from `live.lifecycle` (`V3` tests assert flag-off parity, and `drift_control` keeps legacy tracker behavior unless explicitly opt-in combined).

### 2.6 Senescence Mortality, `VitalStatistics` & `DemographicTracker` (`mortality.rs`, `resources.rs`, `telemetry_export.rs`, `environmental_systems.rs`)

1. **Extend `DeathCause` (`src-tauri/src/core/mortality.rs:187-192`):**
   - Add `DeathCause::Senescence` (`/// Chronological age reached or exceeded species max_age_ticks.`).
   - Update `lethal_cause(homeo: &HomeostaticState, lifecycle: Option<&LifecycleState>, max_age_ticks: Option<u64>, policy: &MortalityPolicy) -> Option<DeathVerdict>` (or add a dedicated wrapper preserving `lethal_cause` signature for legacy tests):
     - Starvation lethal: `homeo.energy <= 0.0 && homeo.starving_ticks >= policy.starve_ticks`
     - Dehydration lethal: `homeo.hydration <= 0.0 && thirst >= policy.dehydration_ticks`
     - Senescence lethal (only when lifecycle active & `max_age_ticks` `Some(max)`): `lifecycle.age_ticks >= max`
     - Deterministic first-lethal overshoot tie-breaking across all 3 (`overshoot = streak - grace`): strictly pick the condition furthest past threshold, breaking exact ties in order **`Starvation > Dehydration > Senescence`** (preserving `mortality.rs:210` semantics).
   - Keep `mortality_system` (`mortality.rs:53-58`) gated on `MortalityPolicy.enabled` OR define explicit interaction if `LifecyclePolicy.enabled && !MortalityPolicy.enabled` (senescence still requires death execution via `ReclaimAndDespawnAgentCommand` — specify clean rule: `mortality_system` runs senescence checks whenever `LifecyclePolicy.enabled`, even if starvation/dehydration grace is disabled!).
2. **Fix `VitalStatistics`, `DemographicTracker` & `demographic_census_system`:**
   - Extend `VitalStatistics` (`src-tauri/src/core/resources.rs:474-570`) with `pub total_deaths_senescence: u64` and `sampled_deaths_senescence: u64`, updating `record_death_by` (`:512`), `sample_window` (`:535`), and adding `peek_cause_window_with_senescence(&self) -> (u64, u64, u64)` so `starvation + dehydration + senescence == total_deaths` holds identically across every window.
   - Fix `mortality.rs:173-180` (`DemographicTracker`): when `LifecyclePolicy.enabled`, map `DeathCause::Senescence` -> `"Senescence"` and `Starvation`/`Dehydration` -> `"Starvation"` (matching `src-tauri/src/core/telemetry_export.rs:103` `DemographicTracker::record_death` rather than mislabeling `lifespan >= 300` as `"Senescence"`!). Keep legacy fallback only when lifecycle is off.
   - Update `demographic_census_system` (`environmental_systems.rs:391-422`) to prefer `LifecycleState.age_ticks` (capped to `u32::MAX`) over `eval.survival_ticks` when present, and record parent's `LifecycleState.age_ticks` in `trk.record_birth(...)` (`agent_systems.rs:1201-1203`) when lifecycle is active.
3. **`world_checksum` (`src-tauri/src/core/snapshot.rs:798-816`):**
   - Only when `LifecyclePolicy.enabled`, hash `age_ticks`, `stage as u8`, `reproduction_cooldown`, and `offspring_count` **inside** `world_checksum`'s sorted per-agent tuple vector (extending the tuple so agents matching on reserve/hydration/temp/pos are deterministically ordered and archetype-order-independent!). Zero bytes hashed when disabled.

## 3. Open Owner Decisions (Inherited `ADR-0013` `D1–D5` + `V3`-Specific `D-V3-1`–`D-V3-2`)

**Gating prerequisite:** Owner accepts `ADR-0013` via `QD-LIFECYCLE`, resolving **`D1`** (asexual single-parent vs bi-parental mating), **`D2`** (deterministic age ceiling vs stochastic Gompertz hazard), **`D3`** (fixed birth body vs multi-stage collider growth), **`D4`** (`litter_size = 1` vs multi-offspring), and **`D5`** (opt-in vs desktop default).

Additional `V3`-specific implementation decisions:
- **`D-V3-1` (Opt-in `V3` observables excluded from `all_live_observable_ids()`):**
  - **Option A (Recommended):** Expose `LIVE_LIFECYCLE_OBSERVABLE_IDS` (`live.juvenile_count`, `live.adult_count`, `live.senescent_count`, `live.mean_age_ticks`, `live.deaths_senescence`) in an opt-in constant **excluded** from `LIVE_OBSERVABLE_IDS` (`12`) and `LIVE_EXTENDED_OBSERVABLE_IDS` (`16`) at `live_experiment.rs:1589, 1632` (matching AE3 / `V1` `D-V1-2` so frozen manifests stay intact), enforcing `ensure_observables_supported` (`:849`) when `live.lifecycle = 0`.
  - **Option B:** Expose lifecycle telemetry exclusively via existing `VitalStatistics` / `DemographicCensusState` without adding experiment observable IDs yet.
- **`D-V3-2` (Aging while dormant in `DormantAgents` / `DormantCohorts`):**
  - **Option A (Recommended):** Freeze chronological aging inside `DormantAgents` hibernation (cryptobiosis) **and** inside `DormantCohorts` (`aggregate_population.rs:1334`), carrying exact saved `LifecycleState` across `HibernateAgentCommand`/`WakeAgentCommand` and `DehydrateAgentCommand`/`RehydrateCommand`.
  - **Option B:** Advance `ArchivedIndividual.lifecycle.age_ticks` per tick inside `dormant_cohort_ecology_system` (`:1334`), triggering senescence culling inside dormant chunks.

## 4. Future `V3` Coding Card Boundaries

### Allowed Paths
- `src-tauri/src/core/components.rs` (`LifeStage`, `LifecycleState`, `AgentMigrationData::lifecycle`)
- `src-tauri/src/core/resources.rs` (`LifecyclePolicy`, `VitalStatistics::total_deaths_senescence`)
- `src-tauri/src/core/simulation_schedule.rs` (`build_tick_schedule`, `lifecycle_age_system`)
- `src-tauri/src/core/agent_systems.rs` (`lifecycle_age_system`, `SpawnGenotypeCommand`, `SpawnOffspringCommand` initializes fixed `Juvenile`/`age_ticks = 0` defaults without needing a command field, `autonomous_reproduction_system`, `execute_reproduction_birth`)
- `src-tauri/src/core/world_systems.rs` (`metabolic_decay_system` `total_cost`, migration serialization/restore)
- `src-tauri/src/core/mortality.rs` (`mortality_system`, `DeathCause::Senescence`, `lethal_cause`, `DemographicTracker` fix)
- `src-tauri/src/core/environmental_systems.rs` (`demographic_census_system`)
- `src-tauri/src/core/aggregate_population.rs` (`ArchivedIndividual::lifecycle`, `SavedDormantCohorts::validate`, `DehydrateAgentCommand`, `RehydrateCommand`)
- `src-tauri/src/core/simulation_state.rs` (`SerializedAgent::lifecycle`, `validate`, `serialize_single_agent`, `serialize_world_state`, `spawn_serialized_agent`)
- `src-tauri/src/core/snapshot.rs` (`validate_state`, conditional sorted-tuple `world_checksum`, test fixtures `:967, 1022`)
- `src-tauri/src/core/simulation_loop.rs` (`ANIMA_LIFECYCLE`, genesis & restore)
- `src-tauri/src/core/live_experiment.rs` (`LIVE_KEY_LIFECYCLE`, `LIVE_KEYS`, `LiveWorldConfig`, `genesis`, `build_live_world`, optional `LIVE_LIFECYCLE_OBSERVABLE_IDS` + `ensure_observables_supported`)
- `src-tauri/src/core/experiment/observables.rs` (`ObservableRegistry::live_default` entries if `D-V3-1 = A`)
- `src-tauri/tests/schedule_ambiguity_tests.rs` (assert `combat_system -> lifecycle_age_system -> metabolic_decay_system -> mortality_system`)
- `src-tauri/tests/lifecycle_tests.rs` (new: gates `LC-S01`–`LC-S08`, matching `ADR-0013` Proposed Verification Gates)
- `LIVE_KEYS.len()` pinning tests (`+1` count) + prereg manifests (`e2_seam_tests.rs`, `eco_card_b_tests.rs`, `founder_genotype_tests.rs`, `founder_layout_tests.rs`, `grass_steering_tests.rs`, `lake_layout_tests.rs`, `land_mask_tests.rs`, `strict_prey_cap_tests.rs`, `prereg_e3_manifest_tests.rs` + fixture JSON, `prereg_e3b_manifest_tests.rs` + fixture JSON per freeze policy)
- Exhaustive struct-literal updates adding `lifecycle: None` across `9` distinct files (`SerializedAgent` in `3` files, `AgentMigrationData` in `6` files, `ArchivedIndividual` in `1` file — exact same `9`-file set listed in [Card `V2`](2026-11-24-v2-species-profile-spec.md) §4)
- `docs/ai/planning/` (card `V3` implementation report note)

## 5. `DONE WHEN`, Negative Controls, Conservation & Determinism Checks

### `DONE WHEN` (`LC-S01`–`LC-S08` From `ADR-0013`)
1. **`LC-S01` Default-off bit-identity:** With `live.lifecycle` absent/`0`, E2/E3b seeds produce **bit-exact** `world_checksum` and observables vs pre-`V3` trunk.
2. **`LC-S02` Maturation & cooldown gates:** `Juvenile` agents with `energy >= threshold` do not breed until `age_ticks >= maturity_age_ticks`; adults cannot breed twice inside `reproduction_cooldown_ticks`; genesis founders initialize `Adult`.
3. **`LC-S03` Senescence & partition exactness:** Agents reaching `max_age_ticks` die under `DeathCause::Senescence` through `ReclaimAndDespawnAgentCommand`, tie-breaks follow `Starvation > Dehydration > Senescence`, `DemographicTracker.deaths_senescence` matches real senescence deaths, and `starv + dehyd + senesc == total_deaths` holds at every sample.
4. **`LC-S04` Closed-EU conservation:** Nonzero maturation and senescence surcharges routed inside `total_cost`, cooldown breeding, and senescence deaths keep `|closed_eu_residual| <= RESIDUAL_ABS_TOLERANCE_EU` (`1e-3`).
5. **`LC-S05` Non-Lamarck invariant:** Parent `LifecycleState` (`age_ticks`, `stage`, `offspring_count`) never mutates `MorphologyGenotype` or `BrainGenotype`; clone newborns start as `Juvenile` (`age_ticks = 0`).
6. **`LC-S06` Four-boundary persistence & legacy saves:** Save/restore, migration, wake, and rehydration carry exact `LifecycleState` (`D01`/`D02`); `run(N) == run(K) -> save -> load -> run(N - K)` matches bit-for-bit; legacy saves with `lifecycle: None` load cleanly.
7. **`LC-S07` Zero-alloc hot loop:** Process-global `TrackingAllocator` across lifecycle aging + metabolism + mortality + reproduction ticks measures `0` heap allocations per tick.
8. **`LC-S08` Schedule total order & workspace gates:** `schedule_ambiguity_tests.rs` passes with `0` ambiguous pairs; `cargo fmt --check`, `cargo clippy --features desktop --all-targets -- -D warnings`, `cargo test --features desktop` + `check_test_targets.mjs --profile desktop`, and `npm run check:docs` all exit `0`.

### Negative Controls (Must Fail Red Under Fault Injection, A3 #2)
1. **Un-gated juvenile birth (`LC-S02`):** Temporarily bypass `stage == LifeStage::Adult` check in `autonomous_reproduction_system` -> juvenile-breeding block test fails red.
2. **Surcharge EU leak (`LC-S04`):** Temporarily subtract `maturation_surcharge * dt` outside `total_cost` -> closed-EU test exceeds `1e-3` and fails red.
3. **Cooldown checksum omission (`LC-S06`):** Omit `LifecycleState` fields from `world_checksum`'s sorted tuple when `LifecyclePolicy.enabled` -> mutated-cooldown checkpoint test fails red.
4. **Rehydrate age reset (`LC-S06`):** Reset `age_ticks = 0` inside `RehydrateCommand::apply` -> rehydrate preservation test fails red.

### Determinism Checks
- **Three-process equality:** With `live.lifecycle = 1.0`, 3 separate processes on the same OS produce identical `world_checksum` and vital death splits across `N` ticks (zero extra `SimRng` draws).
- **Save/load & fork determinism:** Identical `world_checksum` after checkpoint restore across stage transitions and cooldown windows on the same OS.
