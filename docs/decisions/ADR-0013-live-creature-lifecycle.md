---
title: ADR-0013 — Live Creature Lifecycle (Age, Life Stages, Maturation, Costly Reproduction, and Senescence Mortality) for V3
status: accepted
owner: simulation-architecture
last_reviewed: 2026-11-24
decision_date: 2026-11-24
supersedes: none
superseded_by: none
---

# ADR-0013 — Live Creature Lifecycle (Age, Life Stages, Maturation, Costly Reproduction, and Senescence Mortality) for V3

> **Status: `accepted` (`2026-11-24` — owner decision `QD-OPENQ` Group B, `"Duyệt A và B"`, incorporating `QD-PLAYRULES` #2, `QD-LIFE`, and `QD-BRAINBODY`; `D1 = B`, `D2 = A`, `D3 = B`, `D4 = A`, `D5 = A`).**
> Note: The original `Option A` text and `D1–D5` comparison table below remain preserved as the historical baseline; owner acceptance amends `D1` to sexual reproduction via homologous crossover (`B`) and `D3` to physical growth of scale gated behind `ADR-0019` (`QD-BRAINBODY`) per the folded `2026-11-24` `## Physical growth` and `## Biological Sexual Reproduction` sections (`D13 GROWTH-AMEND` folded into `D22 ADR-STATUS`). Sub-gates `D-GROWTH-1..4` and `D-REPRO-1..4` remain open for card-level implementation sign-off, and `V3` / `V3-GROWTH` / `V3-SEX` code changes follow separate implementation cards (`2026-11-24-v3-lifecycle-spec.md`).
> Baseline code anchors below (`§Context`–`§Owner Decision Block`) were verified on `origin/feat/ecological-selection` at `1a2d1d62de52334e675108852311a87c3c1bb10f`; the folded `2026-11-24` `## Physical growth` and `## Biological Sexual Reproduction` anchors below were verified at trunk `902e505d58dbe034a1771bcea352742c296985d9`.

## Context

The v1.0 vertical slice ("watershed – grassland – rabbit – wolf", [`WORLD_SIMULATION_PLAN.md`](../../WORLD_SIMULATION_PLAN.md) §9; card table `Đợt 6` in [`2026-11-05-swarm-master-plan.md`](../ai/planning/2026-11-05-swarm-master-plan.md)) requires an organism lifecycle (`birth/hatch -> juvenile -> mature -> reproduction -> aging -> death`, §5.5) with explicit maturity age, reproduction cost, death selection, and starvation/dehydration/senescence turnover (§5.7).

[`ADR-0005`](ADR-0005-live-population-turnover.md) explicitly left that scope open: it enabled continuous intra-tick energy-gated reproduction (`live.reproduction`) on top of `autonomous_reproduction_system` inside the live experiment adapter, while `mortality_system` (`live.mortality`) handles starvation and dehydration. Five concrete gaps on trunk `1a2d1d62` prevent card `V3` from relying on [`ADR-0005`](ADR-0005-live-population-turnover.md) as-is:

1. **No chronological age component.** `AgentEvaluation::survival_ticks` (`src-tauri/src/core/agent_systems.rs:76`, incremented in `update_agent_evaluation_system` at `:1273-1284`) skips incrementing whenever `homeo.energy <= 0.0 || homeo.hydration <= 0.0` (`:1274-1276`) and resets to `0` on aggregate-LOD `RehydrateCommand::apply` (`src-tauri/src/core/aggregate_population.rs:1201`). It measures non-depleted evaluation time, not chronological age.
2. **No life stages, juvenile maturation gate, or cooldown.** `autonomous_reproduction_system` (`src-tauri/src/core/agent_systems.rs:704`) gates reproduction solely on species/guild counts (`prey_cap`, `pred_count >= 8 || prey_count < pred_count * 5`, `:989-998`) and fixed reserve fractions (`0.65` prey / `0.95` predator, `:1000-1002`). Newborns can breed as soon as energy crosses threshold, and predators at `t ~ 170` immediately bred before their first hunt, draining founder reserves and starving (`ADR-0007`, Exploration Notes at `234-239`).
3. **No maturation cost vs adult reproduction cost separation.** `execute_reproduction_birth` (`src-tauri/src/core/agent_systems.rs:1059-1229`) transfers `invest_nominal` via `debit_reserve` (`:1089-1123`, `:1090`), endows `debited * REPRODUCTION_EFFICIENCY` (`0.60`, `src-tauri/src/core/ecology.rs:158`), and settles `overhead_waste` to detritus (`:1112-1117`). There is no juvenile growth/maturation metabolic charge in `metabolic_decay_system` (`src-tauri/src/core/world_systems.rs:85`) and no senescence metabolic frailty surcharge or per-species gestation/recovery cooldown.
4. **No biological senescence in `mortality_system` or `VitalStatistics`.** `DeathCause` (`src-tauri/src/core/mortality.rs:187-192`) and `lethal_cause` (`:220-242`) only cover `Starvation` and `Dehydration`. `DemographicTracker` in `mortality_system` still labels `lifespan >= 300` as `"Senescence"` (`:173-180`), which an inline comment explicitly warns is wrong because no aging exists yet (`:170-172`). `VitalStatistics` (`src-tauri/src/core/resources.rs:474-571`, with `sample_window` / `peek_cause_window` at `:535-554`) tracks `total_deaths_starvation` and `total_deaths_dehydration`, with no senescence death counter.
5. **Hard prerequisites.** Card `V3` depends on:
   - **`V0`** (`develop_at_birth` + serialized `DevelopedPhenotype` per [`ADR-0001`](ADR-0001-creature-development-lifecycle.md) and [`2026-11-24-v0-develop-at-birth-spec.md`](../ai/planning/2026-11-24-v0-develop-at-birth-spec.md)) so every birth (`SpawnOffspringCommand` at `src-tauri/src/core/agent_systems.rs:514`) develops once at birth instead of calling legacy `decode_genotype`.
   - **`V2`** (`SpeciesProfile` for rabbit and wolf) so maturity age, reproductive investment/threshold, cooldown, and maximum lifespan are data-driven per species rather than hardcoded to `Prey` / `Predator`.

## Decision Drivers

Every option must uphold the architectural invariants in [`CLAUDE.md`](../../CLAUDE.md), [`CREATURE_DEVELOPMENT_CONTRACT.md`](../reference/CREATURE_DEVELOPMENT_CONTRACT.md), [`ADR-0001`](ADR-0001-creature-development-lifecycle.md), [`ADR-0003`](ADR-0003-evolved-per-agent-brains.md), [`ADR-0005`](ADR-0005-live-population-turnover.md), and [`ADR-0012`](ADR-0012-ui-free-simulation-core.md):

1. **Legacy path stays default and bit-identical.** Without an explicit opt-in (`LifecyclePolicy` resource absent or `enabled == false`), existing desktop runs, E2/E3b experiments, and all trunk checksums remain bit-for-bit identical. Zero extra `SimRng` draws when enabled or disabled unless an explicitly chosen RNG option requires one (Option A needs zero RNG draws). `LIVE_MODEL_VERSION` (`"live-bevy-world/2"` at `src-tauri/src/core/live_experiment.rs:67`) is unchanged while opt-in.
2. **No Lamarckian inheritance.** Age, `LifeStage`, reproductive cooldown, and maturation progress belong strictly to `RuntimeState` (`LifecycleState`), never written back into `MorphologyGenotype` or `BrainGenotype` (nor may `AgentBrain.learned` be copied to offspring).
3. **Development happens once at birth (`ADR-0001` D01/D02).** Offspring develop their body geometry and physiological baseline once via `develop_at_birth` (`V0`). Reaching `Adult` or `Senescent` transitions reproductive/metabolic stage without re-running morphological development or silently mutating `Segment`/`RigidBody`/`SpatialCollider`/`JointConstraint` dimensions mid-life (which would violate D05 geometry coherence).
4. **Restore, migration, wake, and rehydration carry the state they were given (`D01`, `D02`, `D09`).** `LifecycleState` must serialize additively (`#[serde(default, skip_serializing_if = "Option::is_none")]`) inside `SerializedAgent` (`src-tauri/src/core/simulation_state.rs:105`), `AgentMigrationData` (`src-tauri/src/core/components.rs:166`), `DormantAgentRecord` (`src-tauri/src/core/agent_systems.rs:205`, via `SerializedAgent`), and `ArchivedIndividual` (`src-tauri/src/core/aggregate_population.rs:224`). Legacy saves with `lifecycle: None` load cleanly and retain pre-lifecycle behavior.
5. **Every new continuous energy charge goes inside `total_cost` in `metabolic_decay_system` (`src-tauri/src/core/world_systems.rs:157`).** Any maturation surcharge or senescence frailty cost must be summed into `total_cost` so it flows through `decay` -> `respired` -> `pool.detritus` (`:174-177`, `:222-224`). Separate deductions leak closed EU. Discrete birth transfer continues to use `debit_reserve` (`agent_systems.rs:1090`) and `settle_reserve_outflow_to_detritus` (`:1112-1117`). Every death — including `DeathCause::Senescence` — despawns through `ReclaimAndDespawnAgentCommand` (`agent_systems.rs:135`; later routed into `V4` carcasses/decomposers).
6. **Zero heap allocations in tick systems (`CLAUDE.md`, S17).** Age advancement, stage transitions, metabolic cost surcharges, senescence mortality checks, and maturity gating must execute with `0` heap allocations per tick.
7. **Single total schedule order (`F-041`).** New lifecycle tick systems must hold an explicit slot inside `core::simulation_schedule::build_tick_schedule` (`src-tauri/src/core/simulation_schedule.rs:121`), passing `schedule_ambiguity_tests.rs:201` (`the_tick_schedule_has_no_ambiguous_pair_at_all`) with `0` ambiguous pairs.
8. **UI-free simulation core (`ADR-0012`).** Headless (`LiveExperimentAdapter`) and desktop runs share the same ECS components, policies, and schedule; frontend code reads lifecycle/vital telemetry passively and contains zero lifecycle logic.

## Options Considered

### Option A — Opt-in Deterministic Stage Machine (`Juvenile -> Adult -> Senescent`) with Physiological Maturity Cost (Recommended)

Introduce an additive ECS component `LifecycleState` (`age_ticks: u64`, `stage: LifeStage`, `reproduction_cooldown: u32`) and resource `LifecyclePolicy` (disabled/absent by default; enabled via `live.lifecycle = 1.0` in experiments or `ANIMA_LIFECYCLE` in desktop), parameterized per species by `V2`'s `SpeciesProfile`:

1. **Schedule placement (`build_tick_schedule`, `src-tauri/src/core/simulation_schedule.rs:178-192`):**
   - Insert `lifecycle_age_system` immediately **before** `metabolic_decay_system` (`:179`), reusing existing flush boundaries cleanly (it mutates `&mut LifecycleState` without spawning/despawning via `Commands`).
   - When `LifecyclePolicy` is absent or `!policy.enabled` (or an entity lacks `LifecycleState` on legacy saves), `lifecycle_age_system` returns early without touching entities or advancing state — preserving bit-identity with trunk `1a2d1d62`.
   - When enabled: `age_ticks` increments by `1` every base tick regardless of momentary zero energy; `reproduction_cooldown` decrements via `saturating_sub(1)`; `stage` transitions deterministically (`Juvenile` -> `Adult` at `age_ticks >= profile.maturity_age_ticks`; `Adult` -> `Senescent` at `age_ticks >= profile.senescence_age_ticks`).
2. **Stage initialization at creation (preserving D01/D02 on restore/migration/rehydrate):**
   - **Genesis founders** (`SimulationEngine::start` at `simulation_loop.rs:1676`, `live_experiment::genesis` at `live_experiment.rs:3108`): spawn as `LifeStage::Adult` with `age_ticks = maturity_age_ticks` and `reproduction_cooldown = 0` so founder populations are immediately reproductively capable rather than starting barren for a full juvenile period.
   - **Biological offspring** (`SpawnOffspringCommand::apply` at `agent_systems.rs:514`): spawn as `LifeStage::Juvenile` with `age_ticks = 0` and `reproduction_cooldown = 0`.
   - **Epoch replacement** (`SpawnGenotypeCommand::apply` at `agent_systems.rs:392`): treated as a newly replaced individual (`Adult` at `maturity_age_ticks` under legacy replacement, cleanly labeled separate from biological birth per `D06`).
   - **Restore / Migration / Wake / Rehydrate** (`spawn_serialized_agent` at `simulation_state.rs:922`, `SpawnMigrationCommand` at `world_systems.rs:980`, `WakeAgentCommand` at `agent_systems.rs:309`, `RehydrateCommand` at `aggregate_population.rs:1164`): carry the exact saved `LifecycleState` verbatim (`D01`/`D02`), never resetting `age_ticks`, `stage`, or `reproduction_cooldown`.
3. **Metabolic coupling (`metabolic_decay_system`, `world_systems.rs:85`, `:157`):**
   - Query `Option<&LifecycleState>` alongside `Option<&SpeciesId>` (when `V2` lands) and `LifecyclePolicy`.
   - Add stage surcharge inside `total_cost = k_base + maintenance + activity_cost + cognition_cost + lifecycle_cost` (`:157`):
     - `Juvenile`: `+ maturation_surcharge` (models somatic maturation energy budget).
     - `Adult`: `+ 0.0` baseline stage surcharge (maintenance already scales by `body_mass` via MTE at `:142-146`).
     - `Senescent`: `+ senescence_surcharge` (models declining homeostatic efficiency in old age).
   - If `!policy.enabled` or component absent, `lifecycle_cost = 0.0` (add `0.0f32` is avoided via branch or kept zero so legacy IEEE-754 sums are untouched).
4. **Costly reproduction gate & cooldown (`autonomous_reproduction_system`, `agent_systems.rs:704`):**
   - When `LifecyclePolicy.enabled`: an agent may reproduce only when `stage == LifeStage::Adult` (neither `Juvenile` nor post-reproductive `Senescent`, or `Senescent` gated by policy — see `D1–D5`), `reproduction_cooldown == 0`, reserve `>=` species birth threshold, and `reserve > safe_reserve`.
   - On successful birth (`execute_reproduction_birth`, `:1059`): besides the conserved reserve transfer (`debit_reserve` -> `child_endowment` + `overhead_waste` into detritus), set parent's `reproduction_cooldown = profile.reproduction_cooldown_ticks` and increment parent's `LifecycleState.offspring_count`.
   - Single-parent asexual inheritance matching [`ADR-0005`](ADR-0005-live-population-turnover.md) and `SpawnOffspringCommand` is kept as the default `V3` path unless the owner chooses bi-parental mating in gate `D1`.
5. **Mortality extension (`mortality.rs:32`, `:187-242` and `resources.rs:474-570`):**
   - Add `DeathCause::Senescence` (`src-tauri/src/core/mortality.rs:187`).
   - Add `senescence: bool` / chronological `max_age_ticks`: when `LifecyclePolicy.enabled`, an agent whose `age_ticks >= max_age_ticks` dies at that tick's `mortality_system` pass (still reclaimed and despawned via `ReclaimAndDespawnAgentCommand`, keeping EU conserved).
   - Extend `lethal_cause` attribution deterministically when multiple causes qualify simultaneously (overshoot past `starve_ticks`, `dehydration_ticks`, or `max_age_ticks`, tie-breaking `Starvation > Dehydration > Senescence` to match `mortality.rs:210`).
   - Extend `VitalStatistics` (`resources.rs:474`) with `total_deaths_senescence` and matching `sampled_deaths_senescence` so `live.deaths_starvation + live.deaths_dehydration + live.deaths_senescence == live.deaths`, and fix `DemographicTracker` (`mortality.rs:173-180`) when `LifecyclePolicy` is active so `"Senescence"` reflects real `DeathCause::Senescence` rather than `lifespan >= 300`.

- **Pros:** Minimal invasive surface on `build_tick_schedule`; preserves `ADR-0001` D05 rigid-body invariants (no mid-life collider resizing); zero extra `SimRng` draws (keeps cross-process determinism simple); closed-EU by construction (`total_cost` -> `respired`); direct prerequisite support for `V4` carcasses and `V8` rabbit/wolf population cycles.
- **Cons / Trade-offs:** Does not simulate physical juvenile body growth (collider radius stays fixed from birth per V0) or bi-parental mate-finding unless `D1`/`D3` select them; requires `world_checksum` (`snapshot.rs:789`) extension gated strictly on `LifecyclePolicy.enabled` so save/load equivalence catches missing lifecycle persistence without altering legacy flag-off checksums — critically, when enabled, `LifecycleState` fields (`age_ticks`, `stage`, `reproduction_cooldown`) must be hashed **inside** `world_checksum`'s sorted per-agent tuple (`src-tauri/src/core/snapshot.rs:798-816`), not via archetype-order iteration, so two agents matching in `[u32; 6]` never introduce archetype-order dependence.

### Option B — Full Sexual Mating + Gestation + Continuous Physical Growth in V3

Require two opposite-`AgentSex` adults (`agent_systems.rs:419-430`) within proximity, run `crossover_genotypes` (`src-tauri/src/evolution/crossover.rs:32`) + gestation timers, and scale `Segment` / `RigidBody` / `SpatialCollider` / `JointConstraint.anchor_offset` each tick as juveniles mature.

- **Pros:** Closer to biological sexual dimorphism and ontogenetic scaling.
- **Cons & Risks:** Continuous collider/joint rescaling violates `D05` static developed-phenotype geometry assumptions in `V0`, risks physics solver instability / zero-alloc pressure in `resolve_joints_system`, introduces Allee mating-failure extinction risk on 10-founder tests (`WORLD_SIMULATION_PLAN.md` §9 baseline), and couples `V3` to spatial mate-search (`locomotion.rs`) before `V8` calibration. High rework risk; premature before `V0` and `V8`.

### Option C — Status Quo (`ADR-0005` Only)

Keep `autonomous_reproduction_system` and `Starvation`/`Dehydration` mortality unchanged without age, stages, maturation, or senescence.

- **Cost:** Blocks `V3`, `V4` (mortality-driven carcass flux), and `V8` (species-differentiated rabbit fast-breeding vs wolf slow-breeding predator–prey cycles in `WORLD_SIMULATION_PLAN.md` §9). Causes founder immediate breeding overshoot (`ADR-0007:234-239`) and mislabels long starvation survival (`lifespan >= 300`) as `"Senescence"` (`mortality.rs:170-180`). Rejected.

## Proposed Decision & Open Owner Gates (`D1–D5`)

**Recommendation:** Adopt **Option A** as the architecture for `V3` (behind opt-in `LifecyclePolicy` + `live.lifecycle` / `ANIMA_LIFECYCLE`, preserving `1a2d1d62` default trajectories bit-for-bit), pending explicit owner decisions on the following five open design gates (`D1–D5`) before card `V3` begins:

| Gate | Question | Options | Recommendation |
|---|---|---|---|
| **D1** | Reproductive mode in `V3` | **A:** Asexual single-parent (per `ADR-0005`) with maturity age + cooldown + species investment cost<br>**B:** Require bi-parental proximity mating + `AgentSex` + `crossover_genotypes` | **A** — isolates life-history dynamics cleanly for `V8` and avoids small-population Allee collapse; `crossover_genotypes` stays available for epochal/later sexual mating cards |
| **D2** | Senescence death rule | **A:** Deterministic age ceiling (`age_ticks >= max_age_ticks`) + increasing metabolic `senescence_surcharge` before ceiling (`0` RNG draws)<br>**B:** Age-dependent stochastic Gompertz hazard drawn from a dedicated `sim_stream` | **A** — deterministic, zero RNG perturbation, easier closed-EU & multi-seed attribution |
| **D3** | Juvenile body growth representation | **A:** Fixed developed geometry from `V0` `develop_at_birth` + physiological `Juvenile` maturation gate & metabolic surcharge (`D05` untouched)<br>**B:** Multi-stage scale multiplier updating `Segment`/`RigidBody`/`SpatialCollider`/`JointConstraint` during simulation | **A** — respects `ADR-0001` D01/D05 ("develop once at birth") and avoids per-tick physics collider rebuilds |
| **D4** | Offspring count per birth (`litter_size`) | **A:** Single offspring per cooldown (`litter_size = 1`, differentiated via species cooldown/threshold/maturity from `V2`)<br>**B:** Configurable multi-offspring litter dividing parent `debited * REPRODUCTION_EFFICIENCY` across `K` newborns | **A** (or `litter_size` field defaulting to `1` in `V2` `SpeciesProfile`, with `prey_cap` / strict-cap semantics unchanged for `1`) |
| **D5** | Activation default after `V3` gates pass | **A:** Opt-in (`live.lifecycle = 1.0`, `ANIMA_LIFECYCLE=1`), default off until `V8`/`V10` pass<br>**B:** Enable by default in desktop app while keeping headless manifest opt-in | **A** — preserves all existing trunk/E3b baselines during `V3–V7` integration; flip only by explicit later decision |

## Consequences

### Positive
- Separates chronological biological age (`LifecycleState.age_ticks`) from non-depleted fitness counters (`AgentEvaluation::survival_ticks`) and rehydration resets.
- Enables species-specific life histories (`V2` fast-maturing rabbits vs slow-maturing wolves) required by the `V8` vertical slice (`WORLD_SIMULATION_PLAN.md` §9).
- Keeps closed-EU conservation exact by construction (`metabolic_decay_system`'s `total_cost` -> `detritus`, and `ReclaimAndDespawnAgentCommand` -> detritus/`V4` carcass).
- Fixes the long-standing `DemographicTracker` `"Senescence"` misattribution (`mortality.rs:170-180`) cleanly without fabricating causes when `LifecyclePolicy` is off.

### Negative / Risks
- **Prey/Predator extinction under immature founders or high maturation tax:** If genesis founders started as `Juvenile` or maturation surcharge exceeded baseline foraging intake, founder cohorts could starve before breeding. Mitigation: spawn genesis founders at `Adult` (`age_ticks = maturity_age_ticks`), and verify viability across multi-seed runs before toggling defaults (`D5`).
- **Snapshot & Migration surface growth:** `SerializedAgent`, `AgentMigrationData`, `DormantAgentRecord`, and `ArchivedIndividual` must carry `Option<LifecycleState>`. Legacy saves lacking the field decode to `None` and stay inert when `LifecyclePolicy` is off; when restoring into a lifecycle-enabled world without saved `LifecycleState`, apply an explicit deterministic legacy migration rule (e.g., default `Adult` at `maturity_age_ticks`) rather than resetting every saved adult to `Juvenile`.
- **`world_checksum` stability (`snapshot.rs:789`):** Hashing `LifecycleState` (`age_ticks`, `stage`, `reproduction_cooldown`) **inside** `world_checksum`'s sorted per-agent tuple (`src-tauri/src/core/snapshot.rs:798-816`) is required when `LifecyclePolicy.enabled` so save/load divergence is detected (including cooldown differences) without introducing archetype-order dependence across equal `[u32; 6]` tuples. It must hash **nothing** when absent/`!enabled`, preserving `100%` of existing E2/E3b checksums.

## Implementation & Rollback Plan (Post-Acceptance, Card `V3`)

1. **Prerequisites merged first:** Confirm owner accepts `ADR-0013` (`D1–D5` recorded) and both `V0` (`develop_at_birth` + `DevelopedPhenotype`) and `V2` (`SpeciesProfile`) are merged on trunk.
2. **Types & pure helpers:** Add `LifeStage`, `LifecycleState`, `LifecyclePolicy`, and `DeathCause::Senescence` (plus `VitalStatistics::total_deaths_senescence`) without changing any default trajectory.
3. **Schedule & systems:** Add `lifecycle_age_system` immediately before `metabolic_decay_system` in `build_tick_schedule` (`simulation_schedule.rs:179`); wire `total_cost` lifecycle surcharge (`world_systems.rs:157`), `autonomous_reproduction_system` stage/cooldown gates, and `mortality_system` senescence checks + tie-breaks.
4. **Persistence round-trip:** Wire `Option<LifecycleState>` into `SerializedAgent`, `serialize_world_state`, `serialize_single_agent`, `spawn_serialized_agent`, `AgentMigrationData` (`check_migration_boundaries_system`, `manual_migration_system`, `SpawnMigrationCommand`), `DormantAgents` (`agent_systems.rs:213`, restored at `simulation_loop.rs:1491`, via `HibernateAgentCommand` / `WakeAgentCommand` reusing `serialize_single_agent` / `spawn_serialized_agent`), and `ArchivedIndividual` (`DormantCohorts` at `aggregate_population.rs:148`, `DehydrateAgentCommand::apply` at `:1039, 1122`, and `RehydrateCommand` at `:1164`), plus conditional `world_checksum` hashing (`snapshot.rs:798-816`).
5. **Opt-in wiring:** Add `live.lifecycle` (`LIVE_KEYS`, `LiveWorldConfig`) and `ANIMA_LIFECYCLE`, keeping absent/off bit-identical to `V2` trunk.
6. **Rollback:** Keep reader support for `Option<LifecycleState>` (`#[serde(default)]`) and flip `live.lifecycle` / `ANIMA_LIFECYCLE` back to disabled.

## Proposed Verification Gates for Card `V3` (`LC-S01`–`LC-S08`)

| Gate | Requirement & Measurement | Threshold | Status |
|---|---|---|---|
| **LC-S01** | Flag-off bit-identity (`live.lifecycle = 0` / absent vs baseline without resource) | Bit-exact `world_checksum` & observables across seeds | proposed |
| **LC-S02** | Maturation & cooldown gating (juvenile cannot breed even at `energy >= threshold`; cooldown blocks immediate consecutive births; founder spawns `Adult`) | Exact tick transition & negative control fails if gate removed | proposed |
| **LC-S03** | Senescence & cause attribution (`age_ticks >= max_age_ticks` triggers `DeathCause::Senescence`; simultaneous tie-break matches spec; `VitalStatistics` partition exact) | `starv + dehyd + senesc == total_deaths` exact | proposed |
| **LC-S04** | Closed-EU invariant with nonzero maturation & senescence surcharges inside `total_cost` | `\|closed_eu_residual\| / initial <= 1e-9` | proposed |
| **LC-S05** | Non-Lamarck invariant (`LifecycleState` & `AgentBrain.learned` never mutate `MorphologyGenotype`/`BrainGenotype`; newborn always starts `Juvenile` age `0`) | Bit-exact genotype checks on clone birth | proposed |
| **LC-S06** | Restore, migration, wake & rehydration carry exact `LifecycleState`; `run N == run K -> save -> load -> run N-K` bit-exact | Equal multi-tick `world_checksum` + legacy `None` fixture loads | proposed |
| **LC-S07** | Zero-heap-allocation hot loop (`TrackingAllocator` across lifecycle age + metabolism + mortality + reproduction tick) | `0` allocations per tick | proposed |
| **LC-S08** | Schedule total order (`schedule_ambiguity_tests.rs` & multi-process determinism across 3 processes) | `0` ambiguous pairs; equal multi-process checksums on same OS | proposed |

## Affected Documents

- Master Plan: [`docs/ai/planning/2026-11-05-swarm-master-plan.md`](../ai/planning/2026-11-05-swarm-master-plan.md) (`Đợt 6` card `V3`)
- Architecture Plan: [`WORLD_SIMULATION_PLAN.md`](../../WORLD_SIMULATION_PLAN.md) (§5.5, §5.7, §9)
- Related ADRs: [`ADR-0001`](ADR-0001-creature-development-lifecycle.md), [`ADR-0003`](ADR-0003-evolved-per-agent-brains.md), [`ADR-0005`](ADR-0005-live-population-turnover.md), [`ADR-0007`](ADR-0007-live-ecology-sustainability.md), [`ADR-0012`](ADR-0012-ui-free-simulation-core.md)
- Prerequisite Specs: [`2026-11-24-v0-develop-at-birth-spec.md`](../ai/planning/2026-11-24-v0-develop-at-birth-spec.md) (`V0`), [`2026-11-24-v1-dynamic-fields-schedule-spec.md`](../ai/planning/2026-11-24-v1-dynamic-fields-schedule-spec.md) (`V1`)
- Index: [`docs/decisions/README.md`](README.md)

## Owner Decision Block (`QD-OPENQ`, Accepted `2026-11-24`)

```text
QD-OPENQ (owner, 2026-11-24, "Duyệt A và B"; incorporating QD-PLAYRULES #2, QD-LIFE, QD-BRAINBODY) = ACCEPT ADR-0013:
  D1 = B (sexual reproduction via homologous crossover; QD-PLAYRULES #2; card B7),
  D2 = A (deterministic senescence),
  D3 = B (physical growth of scale, gated by ADR-0019 / QD-BRAINBODY),
  D4 = A (single offspring per cooldown / litter_size = 1 default, as recommended),
  D5 = A (opt-in default off until V8/V10 pass, as recommended);
  V3 authorized per ADR-0013 (status -> accepted, decision_date -> 2026-11-24).
  Implementation sub-gates D-GROWTH-1..4 and D-REPRO-1..4 remain open for card-level sign-off.
```

---

## Amendment (2026-11-24): Physical Growth and Biological Sexual Reproduction

> **Status: `accepted` (`2026-11-24` — `QD-OPENQ` Group B, `"Duyệt A và B"`; `D13 GROWTH-AMEND` folded into `D22 ADR-STATUS`; implementation sub-gates `D-GROWTH-1..4` and `D-REPRO-1..4` remain open for card-level implementation sign-off).**
> Trunk code anchors verified against `origin/feat/ecological-selection` at
> `902e505d58dbe034a1771bcea352742c296985d9` (`0` code diffs relative to `facc800c50a15cdca960c8c006cc5f861e4c703a`).
> The original `ADR-0013` body above (`Option A`, `D1–D5`, `LC-S01`–`LC-S08`, verified on `1a2d1d62`) remains preserved intact as the historical baseline:
> - `## Physical growth` amends Gate `D3` to `B` (`QD-OPENQ`, gated behind `ADR-0019` / `QD-BRAINBODY`); and
> - `## Biological Sexual Reproduction` (`QD-PLAYRULES` #2 Amendment to `D1`) amends Gate `D1` to `B` (`QD-OPENQ`, opt-in two-sex homologous mating and biological species isolation).
> Both extensions are strictly **opt-in** and keep all trunk, `E2`, and `E3b` trajectories
> bit-identical when absent or disabled ([`ADR-0008`](ADR-0008-world-past-and-time.md)).

## Physical growth

### Hard Prerequisite: Brain–Body Interface ADR (`QD-BIO-D4`) & Rollout Order

Ontogenetic juvenile body scaling **must wait for the brain–body interface ADR (`QD-BIO-D4`,
assigned to `anima-case` as `C13 BRAIN-BODY-ADR` / `ADR-0019`, accepted `2026-11-24` via
`QD-BRAINBODY` in Draft `PR #259`, awaiting merge)**
before any implementation card (`V3-GROWTH`) begins, in addition to prerequisites `V0`
([`2026-11-24-v0-develop-at-birth-spec.md`](../ai/planning/2026-11-24-v0-develop-at-birth-spec.md))
and `V2` ([`2026-11-24-v2-species-profile-spec.md`](../ai/planning/2026-11-24-v2-species-profile-spec.md)):

1. **Trunk multi-segment actuation cap (`agent_systems.rs:1883-1909`, `genotype.rs:195, 253`):**
   `apply_inertia_to_oscillators` reads fixed `InertiaComponent::cpg_parameters: [f32; 4]`
   (`src-tauri/src/core/components.rs:466`) and modulates only sorted slots `0` and `1`
   (`src-tauri/src/core/agent_systems.rs:1896-1908`). Moreover, `decode_genotype` attaches
   `ParentAgent(root_entity)` (`src-tauri/src/evolution/genotype.rs:195`) to the oscillator-less root
   entity, causing slot `0` to be silently discarded while every remaining segment runs hardcoded
   `CpgOscillator::new(1.0, 0.5)` (`src-tauri/src/evolution/genotype.rs:253`).
2. **Gait–body sensitivity & metabolic mass calibration ([`ADR-0003`](ADR-0003-evolved-per-agent-brains.md), `world_systems.rs:142-146`):**
   `ADR-0003` Gate `EB-S09` (`lines 118-130`) already measured a `53%` displacement reversal across
   body topologies (`0.490` vs `0.321`). Continuously scaling `Segment` dimensions, `RigidBody.mass`,
   and `JointConstraint.anchor_offset` during ontogeny shifts limb inertia, spring leverage in
   `resolve_joints_system` (`src-tauri/src/physics/dynamics.rs:24-119`), and MTE maintenance scaling
   (`metabolic_rate` at `src-tauri/src/core/world_systems.rs:142-146`). `QD-BIO-D4` (`ADR-0019`) locks down
   dimensionless / body-aware motor coupling first so growing juveniles do not break actuator control.
3. **Staged landing:** Base `V3` core lifecycle ([`2026-11-24-v3-lifecycle-spec.md`](../ai/planning/2026-11-24-v3-lifecycle-spec.md):
   chronological age, stage transitions, starvation/dehydration/senescence mortality on fixed `V0`
   bodies) may proceed ahead of physical growth; **`V3-GROWTH` waits on merged `ADR-0019` (`QD-BRAINBODY`, accepted `2026-11-24`, `PR #259`)**.

### Architectural Invariants for Juvenile Physical Growth

1. **Development stays "once at birth" for the body plan ([`ADR-0001`](ADR-0001-creature-development-lifecycle.md), [`CREATURE_DEVELOPMENT_CONTRACT.md`](../reference/CREATURE_DEVELOPMENT_CONTRACT.md)):**
   - `develop_at_birth` (`V0`) resolves the node/edge topology, joint axes, habitat preference,
     pigment, and **target adult `DevelopedPhenotype` dimensions** exactly once at genesis or birth.
   - No post-birth branching or topology edits (no L-systems, no mid-life segment addition) and no
     mid-life re-evaluation of developmental reaction norms against current biomes.
   - Instead, a newborn hatches/spawns at initial linear juvenile scale `s_birth in (0, 1]` and
     scales smoothly over ontogeny toward `s = 1.0` (its pre-determined adult phenotype dimensions).
2. **Synchronized per-segment geometry, physics & IPC (`D05`, zero heap allocations):**
   - Store current realized scalar scale `growth_scale: f32` inside `LifecycleState` (`RuntimeState`
     only — never written into `MorphologyGenotype`, upholding non-Lamarckian inheritance).
   - Genesis founders (`LifeStage::Adult`) and evolutionary replacements initialize at `s = 1.0`;
     biological newborns (`SpawnOffspringCommand`) initialize at `s = s_birth`.
   - When growth advances during juvenility, a deterministic `growth_update_system` inside
     `core::simulation_schedule::build_tick_schedule` (ordered before `resolve_joints_system`,
     `rebuild_spatial_grid_system`, and `metabolic_decay_system`) updates all segments of each growing
     agent atomically with **`0` heap allocations**:
     - `Segment.length = adult_length * s` and `Segment.radius = adult_radius * s`, keeping `Segment.mass` (`src-tauri/src/core/components.rs:104-109`) aligned with `RigidBody.mass` under `D05`;
     - `SpatialCollider.radius = adult_collider_radius * s` (`src-tauri/src/physics/spatial.rs:7-9`);
     - `JointConstraint.anchor_offset = adult_anchor_offset * s` (`src-tauri/src/physics/dynamics.rs:17-22`),
       keeping child joint attachment flush with scaled parent extents in `resolve_joints_system`
       (`src-tauri/src/physics/dynamics.rs:80-82` computes `parent_pos + parent_rot * anchor_offset`
       against `child_segment.length / 2.0`);
     - `RigidBody.mass` (`src-tauri/src/physics/dynamics.rs:10-14`) scaled smoothly toward `adult_mass`
       per `D-GROWTH-2` (clamped finitely, avoiding uncalibrated kg/m^3 extremes relative to the MTE
       maintenance reference);
     - IPC `SegmentState` (`src-tauri/src/core/simulation_state.rs:23-39`) already exports live
       `segment.length`, `segment.radius`, and `jc.anchor_offset` (`src-tauri/src/core/simulation_loop.rs:2233-2260`),
       keeping renderers read-only ([`ADR-0012`](ADR-0012-ui-free-simulation-core.md)).
3. **Growth energy cost strictly inside `total_cost` (`CLAUDE.md` hard rule):**
   - Somatic tissue accretion during active growth adds an explicit continuous `growth_cost` term
     inside `total_cost` in `metabolic_decay_system` (`src-tauri/src/core/world_systems.rs:157`):
     `total_cost = k_base + maintenance + activity_cost + cognition_cost + lifecycle_cost + growth_cost`.
   - Everything in `total_cost` flows through `decay = total_cost * dt` into `respired`
     (`src-tauri/src/core/world_systems.rs:174-177`) and settles into `pool.detritus`
     (`:222-224`, or declared open-flow sinks once planned `ADR-0018` lands). Growth **never**
     subtracts energy outside `total_cost` and preserves closed-EU conservation by construction. When
     `!GrowthPolicy.enabled` or `s == 1.0`, `growth_cost` is bypassed cleanly by branch so legacy
     IEEE-754 sums stay untouched.
4. **Restore, migration, wake & rehydration carry current size verbatim (`D01`, `D02`, `D09`):**
   - Persist `growth_scale: Option<f32>` (via `#[serde(default, skip_serializing_if = "Option::is_none")]`,
     alongside `LifecycleState`) across `SerializedAgent` (`src-tauri/src/core/simulation_state.rs:105`),
     `AgentMigrationData` (`src-tauri/src/core/components.rs:166`), `DormantAgentRecord`
     (hibernation wake via `serialize_single_agent` / `spawn_serialized_agent`), and
     `ArchivedIndividual` (`src-tauri/src/core/aggregate_population.rs:224`).
   - Restore, migration, hibernation wake, and aggregate-LOD rehydration validate `0.0 < s <= 1.0`
     and materialize the exact saved scale `s` into `Segment`/`RigidBody`/`SpatialCollider`/`JointConstraint` —
     they **never** recompute current size from `age_ticks`, genotype mass, or destination climate.
     Legacy saves/payloads with `None` default to `1.0` (adult pre-growth baseline).
5. **Snapshot & `world_checksum` impact (`src-tauri/src/core/snapshot.rs:789-822`):**
   - **Flag off (`GrowthPolicy` absent or `!enabled`):** `world_checksum` hashes **zero extra bytes**,
     consumes zero `SimRng` draws, and leaves all trunk, `E2`, and `E3b` checksums bit-identical. Any
     new `live.growth` key must honor the E3b freeze-pin rule (`QD-BIO-D2`, `src-tauri/tests/prereg_e3b_manifest_tests.rs:417-426, 584-588`).
   - **Flag on:** append `growth_scale.to_bits()` (with `LifecycleState` fields) **inside
     `world_checksum`'s sorted per-agent tuple** (`agents: Vec<[u32; ...]>` at
     `src-tauri/src/core/snapshot.rs:798-816`, never raw archetype iteration order), ensuring
     `run N == run K -> save -> load -> run N-K` catches any dropped or recomputed body size while
     preserving restore-order independence (`snapshot.rs:785-788`).

### Open Owner D-Slots for Physical Growth (`D-GROWTH-1`–`D-GROWTH-4`)

| Slot | Question | Options | Recommendation |
|---|---|---|---|
| **D-GROWTH-1** | Ontogenetic growth curve shape | **A:** Bounded Von Bertalanffy / decelerating asymptotic curve normalized so `s(maturity_age_ticks) = 1.0`<br>**B:** Piecewise linear ramp `clamp(s_birth + r * t, s_birth, 1.0)`<br>**C:** Sigmoidal Gompertz curve | **A** — smooth deceleration reaching `1.0` exactly at `Adult` transition without collider step kinks |
| **D-GROWTH-2** | Newborn scale `s_birth` & mass law | **A:** Per-species `s_birth` in `V2` `SpeciesProfile` (e.g. `0.35–0.50` linear) + isometric mass `m_adult * s^3` clamped to `[MIN_MASS, m_adult]`<br>**B:** Global uniform `s_birth = 0.5` + linear `m_adult * s` | **A** — captures realistic juvenile mass scaling under species life histories once calibrated under `QD-BIO-D4` |
| **D-GROWTH-3** | Growth coupling under energy deficit | **A:** Resource-gated growth: advance `s` and assess `growth_cost` only while `homeo.energy > reserve_floor` (starving juveniles pause growth while chronological `age_ticks` keeps ticking)<br>**B:** Unconditional age-locked growth charging `growth_cost` regardless of energy deficit | **A** — avoids lethal compounding starvation spiral while linking foraging success to somatic maturation rate |
| **D-GROWTH-4** | Physics/collider sync cadence | **A:** Per-tick deterministic update inside dedicated `build_tick_schedule` slot before `resolve_joints_system` & `rebuild_spatial_grid_system`<br>**B:** Quantized milestone steps (every `K` ticks) to minimize per-tick collider changes | **A** (continuous smooth scaling with `0` heap allocations; fall back to `B` only if `QD-BIO-D4` solver benchmarks show instability) |

---

## Biological Sexual Reproduction (`QD-PLAYRULES` #2 Amendment to `D1`)

Following owner decision **`QD-PLAYRULES` #2** (*"as biologically right as possible"*), **animal
reproduction amends Gate `D1`** from fixed asexual single-parent cloning to **opt-in bi-parental
sexual reproduction** (keeping existing single-parent `live.reproduction` intact as the opt-in legacy
control; plant hermaphroditism, pollen/seed outcrossing, selfing inbreeding cost, and clonal grass
spread belong to separate planned `ADR-0015` under card `D14` and are not conflated here):

1. **Two discrete sexes (`AgentSex`, `src-tauri/src/core/components.rs:15-18`), rejecting automixis:**
   - Individuals carry discrete `AgentSex::Male` or `AgentSex::Female` (no automixis / uniparental
     cloning fallback inside sexual mode).
   - Trunk gap (`simulation_state.rs:130-134`): today `SpawnGenotypeCommand::apply`
     (`src-tauri/src/core/agent_systems.rs:419-430`) and `SpawnOffspringCommand::apply`
     (`:598-609`) attach `AgentSex`, whereas desktop genesis `SimulationEngine::start`
     (`src-tauri/src/core/simulation_loop.rs:1674-1701`) and headless genesis
     `live_experiment::genesis` (`src-tauri/src/core/live_experiment.rs:3108-3133`) do not. Under
     opt-in sexual reproduction, genesis founders must also receive deterministic `AgentSex` on an
     isolated RNG stream (never shifting legacy `SimRng` when disabled) and carry `AgentSex` cleanly
     across all four boundaries — snapshot restore and hibernation wake (`SerializedAgent.sex` at
     `:134` already exists), plus additive `#[serde(default, skip_serializing_if = "Option::is_none")]`
     `sex: Option<AgentSex>` fields on `AgentMigrationData` (`src-tauri/src/core/components.rs:166`)
     and `ArchivedIndividual` (`src-tauri/src/core/aggregate_population.rs:224`).
2. **Mate finding by proximity & evolved signals, with closed-EU conservation:**
   - Eligible partners require both parents to be `LifeStage::Adult`, opposite `AgentSex`, off
     `reproduction_cooldown`, and above species reserve thresholds.
   - Proximity search uses `SpatialHashGrid` in deterministic sorted order (zero archetype-order
     dependence), gated by evolved communication channels (planned `ADR-0017` / `D18`, never hardcoded
     oracle kin/RHP sensors).
   - Parental investment debits reserves via `debit_reserve` and routes overhead waste via
     `settle_reserve_outflow_to_detritus` (`src-tauri/src/core/agent_systems.rs:1089-1123`), keeping
     closed EU conservation exact.
3. **Homologous recombination (`MorphologyGenotype`, `BrainGenotype`, `EnergyPathwayGenotype`) & incompatibility rejection:**
   - **Morphology homology (`mutation.rs:167` vs `crossover.rs:32-170`):** Structural add-node mutation
     mints unique inherited node IDs (`reserve_node_ids`, `src-tauri/src/evolution/mutation.rs:167`),
     but current `crossover_genotypes` (`src-tauri/src/evolution/crossover.rs:32-170`) picks random
     subtrees and renumbers grafted IDs (`:72-82, :127-139`). Under sexual reproduction, homologous
     crossover aligns inherited node/edge IDs so recombination exchanges **only homologous body-plan
     parts** (`QD-PLAYRULES` #2).
   - **Incompatible pairs produce zero offspring (`None`):** Matches `EnergyPathwayGenotype::crossover`
     returning `None` on mismatching expressed `source_id` (`src-tauri/src/core/evolution_pathway.rs:160-179, :164-166`).
     Both homologous morphology crossover without compatible homologs and `BrainGenotype` recombination
     over incompatible `ArchSpec` dimensions return `None` (`0` births) instead of silently cloning
     `parent_a` (explicitly diverging from legacy `crossover_brains` fallback at
     `src-tauri/src/evolution/brain_genotype.rs:700-716, :706-708`).
   - Post-recombination mutation applies strictly to inherited `MorphologyGenotype` / `BrainGenotype`
     before `develop_at_birth` ([`ADR-0001`](ADR-0001-creature-development-lifecycle.md)); `AgentBrain.learned`
     is never written back ([`ADR-0003`](ADR-0003-evolved-per-agent-brains.md), non-Lamarckian).
4. **Genetic distance compatibility & Biological Species Concept (`AE-S11` / `AE-S14`):**
   - Reproductive compatibility declines monotonically with genetic distance across homologous
     morphology, brain/pathway alleles, and neutral loci; pairs past threshold (or lacking structural/pathway
     compatibility) produce no offspring.
   - Two-parent ancestry extends `LiveLineage::record_birth` (`src-tauri/src/core/live_lineage.rs:416-449`)
     and `SpawnOffspringCommand` `parent_ids` (`src-tauri/src/core/agent_systems.rs:1218`) to record
     both parent IDs (`vec![parent_a, parent_b]`).
   - In alignment with [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md)
     (`:308` `AE-S11`, `:311` `AE-S14`), a **"species"** denotes a **reproductively isolated cluster**
     supported by multi-seed reproductive/trait evidence rather than cosmetic tags or species badges —
     enabling emergent allopatric speciation after geographic isolation (planned `ADR-0016` / `D15`).

### Open Owner D-Slots for Biological Sexual Reproduction (`D-REPRO-1`–`D-REPRO-4`)

| Slot | Question | Options | Recommendation |
|---|---|---|---|
| **D-REPRO-1** | Offspring sex determination | **A:** Balanced `50/50` deterministic draw on dedicated birth sex stream (`live.sexual_reproduction`)<br>**B:** Heritable autosomal/sex-locus modifier clamped to `[0.1, 0.9]` | **A** initially (Fisherian balanced sex ratio without sex-chromosome drift collapse), leaving `B` optional |
| **D-REPRO-2** | Parental endowment split | **A:** Configurable per-species female/male share (e.g. `0.8` maternal / `0.2` paternal effort, conserved via `debit_reserve`)<br>**B:** Maternal-only endowment debit (male pays locomotion/signalling cost only) | **A** — parameterized in `V2` `SpeciesProfile`, both debits accounted through `debit_reserve` -> detritus |
| **D-REPRO-3** | Genetic distance gate function | **A:** Smooth probabilistic compatibility falloff `p_compat(d)` drawn on dedicated stream alongside hard homolog/`source_id` `None` rejection<br>**B:** Deterministic hard threshold `d <= d_max` (`0` RNG draws) | **A** (captures biologically gradual reproductive isolation/grey zone; deterministic ablation `B` optional) |
| **D-REPRO-4** | Vertical-slice rollout sequence | **A:** Stage `V3` (core opt-in lifecycle) -> `QD-BIO-D4` -> `V3-SEX` & `V3-GROWTH` cleanly as separate opt-in cards<br>**B:** Delay initial `V3` until growth, brain–body, and sexual mating merge in one card | **A** — minimizes integration risk while keeping legacy defaults bit-identical at every step |

### Additional Proposed Verification Gates (`LC-S09`–`LC-S12`)

| Gate | Requirement & Measurement | Threshold | Status |
|---|---|---|---|
| **LC-S09** | Physical growth conservation & `D05` sync: `growth_cost` summed inside `total_cost` (`metabolic_decay_system`), and `Segment`/`RigidBody`/`SpatialCollider`/`JointConstraint.anchor_offset` track `growth_scale` cleanly (gated on `QD-BIO-D4`) | `\|closed_eu_residual\| / initial <= 1e-9`; `0` anchor mismatches | proposed (blocked on `QD-BIO-D4`) |
| **LC-S10** | Growth persistence & `world_checksum` gate: save/restore & migration preserve `growth_scale < 1.0` verbatim without recomputing; altered `growth_scale` mutates `world_checksum` when enabled and `0` bytes when disabled | Bit-exact post-restore `world_checksum` & segment geometry | proposed (blocked on `QD-BIO-D4`) |
| **LC-S11** | Homologous crossover & reproductive isolation gate: homologous segments align by inherited `NodeId`; incompatible homolog/architecture/`source_id` or genetic distance beyond cutoff returns `None` (`0` births); two-parent edges recorded in `LiveLineage` | `100%` incompatibility rejection + negative control fails when distance check bypassed | proposed |
| **LC-S12** | Sexual genesis, restore & zero-alloc parity: genesis assigns deterministic `AgentSex` without shifting flag-off seeds; `AgentBrain.learned` never enters offspring genotype; `0` heap allocations on mate-search and growth tick paths | Exact `world_checksum` parity (`flag off`) & `0` hot-loop heap allocations | proposed |

## Owner Decision Block Update (Physical Growth & Sexual Reproduction Amendment)

```text
QD-LIFE-AMEND (owner, <YYYY-MM-DD>) = ACCEPT ADR-0013 AMENDMENT:
  D-GROWTH-1 = <A|B|C>, D-GROWTH-2 = <A|B>, D-GROWTH-3 = <A|B>, D-GROWTH-4 = <A|B>,
  D-REPRO-1  = <A|B>,   D-REPRO-2  = <A|B>, D-REPRO-3  = <A|B>, D-REPRO-4  = <A|B>;
  Note: V3-GROWTH remains gated behind QD-BIO-D4 (brain-body interface ADR).
```
