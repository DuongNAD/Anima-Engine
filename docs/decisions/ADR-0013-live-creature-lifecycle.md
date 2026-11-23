---
title: ADR-0013 — Live Creature Lifecycle (Age, Life Stages, Maturation, Costly Reproduction, and Senescence Mortality) for V3
status: proposed
owner: simulation-architecture
last_reviewed: 2026-11-24
decision_date: pending
supersedes: none
superseded_by: none
---

# ADR-0013 — Live Creature Lifecycle (Age, Life Stages, Maturation, Costly Reproduction, and Senescence Mortality) for V3

> **Status: `proposed` — awaiting owner decision (`D1–D5`).**
> Do **not** implement card `V3` until the owner accepts this ADR and records decisions `D1–D5`.
> Code anchors below are verified on `origin/feat/ecological-selection` at commit
> `1a2d1d62de52334e675108852311a87c3c1bb10f`.

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

## Owner Decision Block (To Be Filled on Acceptance)

```text
QD-LIFECYCLE (owner, <YYYY-MM-DD>) = ACCEPT ADR-0013:
  D1 = <A|B>, D2 = <A|B>, D3 = <A|B>, D4 = <A|B>, D5 = <A|B>;
  V3 authorized per ADR-0013 (status -> accepted, decision_date -> <YYYY-MM-DD>)
```
