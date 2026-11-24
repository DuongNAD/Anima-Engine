---
phase: planning
feature: evolving-plants
title: Evolving Plants Implementation Cards (P1-P8 Per ADR-0015)
description: Sequenced implementation cards P1 through P8 covering BENCH-ENTITIES scaling, individual tree and shrub genotypes and lifecycles, conserved plant energy and V5 nutrient coupling, evolving grass fields, hermaphrodite reproduction, Red Queen coevolution, G1/G2/G3 plant genesis, and plant inhabiting
status: proposed
owner: anima-dell
last_reviewed: 2026-11-24
---

# Evolving Plants — Implementation Cards (`P1`–`P8`)

> **Planning document only (`status: proposed`, card `D14 PLANT-EVO-ADR`).**
> No Rust, TypeScript, or contract edits are made under `D14`. Every card below waits for owner
> acceptance of [`ADR-0015`](../../decisions/ADR-0015-evolving-plants.md) (`D-PLANT-EVO-1`–`D-PLANT-EVO-6`),
> and cards introducing new `live.*` keys or observables follow merged `C12 E3B-PIN` (`#260` @ `7e383b8b`,
> `QD-BIO-D2`, `src-tauri/tests/prereg_e3b_manifest_tests.rs:84, 120, 574-603, 754-761` permitting
> absent-means-legacy `0.0 == Default` post-freeze keys).
> Code anchors were verified on `902e505d` and refreshed/re-verified on `origin/feat/ecological-selection` at trunk commit
> `aef12cb641d8d1e02dc35c97f2e74ee2fbb87b93`.

---

## Dependency Overview

```text
P1 (BENCH-ENTITIES: 500 & 2,000 sessile flora + swarm bench extension, read-only)
 └──► P2 (Individual PlantGenotype, once-at-germination dev, lifecycle & Tree death B6)
       └──► P3 (PlantBiomass EU + total_cost + V5 nutrient & fruit conservation bridge)
             ├──► P4 (EvolvingGrassField SoA trait selection & spatial dispersal on ResourceField)
             ├──► P5 (Hermaphrodite pollen/seed outcrossing, distance compatibility & selfing cost)
             │     └──► P6 (Herbivore-plant Red Queen coevolution & paired S43/AE-S14 harness)
             ├──► P7 (Plant Genesis G1 designed founders & G2 producers-first viability pools)
             └──► P8 (Inhabiting an individual Tree/Shrub + temporal rings + viewer data contract)
```

*(Note: `P1` shares benchmark coverage with `RESEARCH_REVIEW` Tier A `A2` / merged [`ADR-0018`](../../decisions/ADR-0018-ecosystem-scales-and-cycles.md) (`#263`, `proposed`)
[ecosystem card `E1`](2026-11-24-ecosystem-cards.md); implement once so plant and ecosystem teams measure on one canonical Criterion bench.)*

---

## Card `P1` — `BENCH-ENTITIES`: Sessile Flora & Field Scaling Baseline (`A2`)

- **START WHEN:** Approved by lead (`A2`, zero production runtime semantics changed). May run before
  or immediately alongside `ADR-0015` acceptance to calibrate `D-PLANT-EVO-2` individual caps.
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/benches/live_schedule_scaling.rs`
  - `src-tauri/Cargo.toml` (only if helper bench target added; zero new dependencies)
  - `docs/paper/data/scaling.md`
  - `docs/ai/planning/` (report note)
- **Scope:** Extend `live_schedule_scaling` (`N = 1,000` active animals baseline = `5.643 ms p50` at
  `docs/paper/data/scaling.md:72`) with parameterized sessile plant entity counts (`500` and `2,000`
  trees/shrubs across `256x256` `ResourceField` + spatial index), recording `p50`, `p95`, and
  per-entity marginal `us/tick` on the real `build_tick_schedule` (`simulation_schedule.rs:130-202`).
- **`DONE WHEN`:**
  - `cargo bench --bench live_schedule_scaling` exits `0` with `500` & `2,000` sessile scenarios measured
    and archived in `docs/paper/data/scaling.md`; `cargo fmt --check`, `cargo clippy --features desktop
    --all-targets -- -D warnings`, and `npm run check:docs` exit `0`.
  - **Negative control:** Perturb schedule by skipping `seed_dropping` / `fruit_growth` query or fixing
    `trees = 0` -> sessile benchmark delta collapses (documented control check).

---

## Card `P2` — Individual `PlantGenotype`, Germination Development & Lifecycle (`B6`, `PLT-S04`)

- **START WHEN:** `ADR-0015` accepted (`D-PLANT-EVO-1`, `D-PLANT-EVO-2`) AND `P1` measured (`C12 E3B-PIN` `#260` @ `7e383b8b` absent-means-legacy gate honored if adding `live.plant_evolution`).
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/components.rs` (`Tree` opt-in genetic/stage extensions or `IndividualPlant`)
  - `src-tauri/src/core/resources.rs` (`sim_stream::PLANT_EVOLUTION = 6`, `sim_stream::PLANT_GENESIS = 7`)
  - `src-tauri/src/core/environmental_systems.rs` (`fruit_growth_system`, `seed_dropping_system` opt-in path)
  - `src-tauri/src/core/simulation_schedule.rs`, `src-tauri/tests/schedule_ambiguity_tests.rs` (only if new lifecycle systems are registered)
  - `src-tauri/src/core/simulation_state.rs`, `src-tauri/src/core/snapshot.rs` (conditional validation + `world_checksum`)
  - `src-tauri/src/core/experiment/laws.rs`, `src-tauri/src/core/live_experiment.rs` (`PlantEvolutionLaw` opt-in gate)
  - `src-tauri/tests/plant_lifecycle_tests.rs` (new)
- **Scope:** Introduce `PlantGenotype`, `PlantPhenotype` evaluated strictly once at germination
  (`ADR-0001`), ontogenetic growth stages (`Seed -> Sapling -> Adult -> Seeding -> Senescent -> Dead`),
  individual tree/shrub death with bounded mutation on heritable traits (`growth_rate`, `max_fruit`,
  `seed_drop_cooldown`, `seed_spread_radius`, `root_depth`, defences), keeping flag-off `Tree` and
  `SimRng` bit-identical (`PLT-S01`, `PLT-S04`).
- **`DONE WHEN`:**
  - `cargo test --features desktop` (`--` filtered + full suite via `check_test_targets.mjs --profile desktop`),
    `cargo fmt --check`, `cargo clippy --features desktop --all-targets -- -D warnings` exit `0`; `TrackingAllocator`
    asserts `0` heap allocs/tick across 1,000 ticks; `run(N) == run(K) -> save -> load -> run(N-K)` exact.
  - **Negative controls:** (1) Re-evaluate `PlantPhenotype` from current environment on snapshot restore ->
    `PLT-S04` restore test fails red; (2) toggle off `PlantEvolutionLaw` with modified draw in `SimRng` ->
    flag-off bit-identity test fails red.

---

## Card `P3` — Plant Metabolic `total_cost`, Living `PlantBiomass` & `V5` Soil Coupling (`PLT-S03`)

- **START WHEN:** `P2` merged, `V4` & `V5` merged (if using `DynamicFields` / carcass litter), and
  merged [`ADR-0018`](../../decisions/ADR-0018-ecosystem-scales-and-cycles.md) (`#263`, `proposed`) `OpenEnergyFlowLaw` ready (or explicitly tested under both open-flow and
  closed-EU detritus modes).
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/crates/anima-domain/src/energy.rs`, `src-tauri/src/core/energy_ledger.rs`
  - `src-tauri/src/core/environmental_systems.rs` (`detect_environmental_collisions_system` fruit ledger path, plant decay -> `Detritus`)
  - `src-tauri/src/core/ecology.rs`, `src-tauri/src/core/world_systems.rs`
  - `src-tauri/tests/plant_energy_conservation_tests.rs` (new)
- **Scope:** Route all plant growth, basal maintenance, and defence biosynthesis costs through a
  single plant `total_cost` budget inside `EnergyLedger`; replace trunk's `Detritus`-claim fruit
  payout (`environmental_systems.rs:357-380`) with accounted living `PlantBiomass` (or open solar
  input `SolarPrimaryProduction` -> `RespiredHeatLoss` + `Detritus` deadfall/litter); bind root/canopy
  coupling cleanly to `V5` `DynamicFields` `cum_nutrient_uptake` (`dynamic_fields.rs:106`).
- **`DONE WHEN`:**
  - Long runs (`>= 10,000` ticks) maintain `|closed_eu_residual| <= 1e-3` (closed mode) and/or
    `|R_open| <= 1e-3` (open-flow mode), `|nutrient_budget_residual| / scale < 1e-4`, and `0` heap
    allocs/tick under `TrackingAllocator`.
  - **Negative controls:** (1) Omit deducting eaten fruit from tree `PlantBiomass` (or charge defence
    outside `total_cost`) -> residual exceeds `1e-3` and fails red; (2) leave legacy `Detritus` credit
    active while also crediting tree EU -> double-spend caught by ledger gate.

---

## Card `P4` — Evolving Grass Population Field (`EvolvingGrassField`, `PLT-S02`, `PLT-S03`)

- **START WHEN:** `P3` merged and owner resolved `D-PLANT-EVO-3`.
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/ecology.rs` (`EvolvingGrassField` paired SoA trait buffers sharing `ResourceField::r[i]` + strided update)
  - `src-tauri/src/core/environmental_systems.rs` (`resource_field_regrowth_system`, `herbivore_grazing_system` palatability/defence intake modifier)
  - `src-tauri/src/core/aggregate_population.rs` (`DormantCohorts::graze_all_with_reserve` parity!)
  - `src-tauri/src/core/simulation_state.rs`, `src-tauri/src/core/snapshot.rs`, `src-tauri/src/core/telemetry_export.rs` (`SavedSimulationState` + `vegetation_density` & grass trait summaries)
  - `src-tauri/tests/grass_evolution_tests.rs` (new)
- **Scope:** Attach fixed pre-allocated per-cell trait distribution buffers (matching `D-PLANT-EVO-3`)
  alongside `ResourceField::r[i]`, updated in-place on `REGROWTH_STRIDE = 4` (`ecology.rs:422`).
  Implement grazing differential mortality on palatability/defence, adjacent-cell pollen/seed dispersal,
  clonal vegetative expansion, and `DormantCohorts` grazing parity so hibernating chunks never bypass
  plant defence selection.
- **`DONE WHEN`:**
  - Grazing selectivity shifts surviving grass trait distributions in grazed vs ungrazed patches;
    `DormantCohorts` and live `Prey` observe identical intake scaling (`sleeping_is_not_cheaper` check);
    `TrackingAllocator` = `0`; flag-off `ResourceField` remains bit-identical.
  - **Negative controls:** (1) Zero `chemical_defence` grazing attenuation -> differential trait shift
    disappears; (2) omit updating `DormantCohorts::graze_all_with_reserve` -> parity test fails red.

---

## Card `P5` — Hermaphrodite Sexual Outcrossing, Genetic Compatibility & Selfing Cost (`PLT-S05`)

- **START WHEN:** `P2` + `P4` merged and owner decided `D-PLANT-EVO-4`.
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/environmental_systems.rs` (`seed_dropping_system` / pollen-dispersal system)
  - `src-tauri/src/core/simulation_schedule.rs`, `src-tauri/tests/schedule_ambiguity_tests.rs` (only if pollen dispersal adds a distinct system)
  - `src-tauri/src/core/ecology.rs` (grass pollen exchange)
  - `src-tauri/src/evolution/` (plant homologous recombination helper)
  - `src-tauri/tests/plant_reproduction_tests.rs` (new)
- **Scope:** Implement hermaphroditic pollen sampling within `pollen_radius` on
  `sim_stream::PLANT_EVOLUTION`, homologous `PlantGenotype` recombination with smooth genetic-distance
  compatibility decay, isolated-patch self-fertilization carrying the `D-PLANT-EVO-4` inbreeding penalty,
  and clonal stolon/tiller spread for grasses (`QD-PLAYRULES` #2).
- **`DONE WHEN`:**
  - Reproductively incompatible distant plant lineages exhibit measurable drop in viable cross-seed
    set; selfed cohorts pay `D-PLANT-EVO-4` viability penalty vs outcrossed controls on `>= 5` seeds;
    bit-exact same-seed replay holds.
  - **Negative controls:** (1) Disable compatibility distance check -> reproductively divergent lineages
    cross at 100% fertility; (2) zero selfing penalty -> isolated selfers match outcrossed vigor.

---

## Card `P6` — Herbivore–Plant Red Queen Coevolution & `AE-S14` Evidence Suite (`PLT-S06`)

- **START WHEN:** `P4` + `P5` merged (and additive herbivore counter-adaptation traits enabled if part of scenario).
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/experiment_runner/` (plant coevolution paired ensemble & time-shift fixture helpers)
  - `src-tauri/src/core/live_lineage.rs` (lineage summary exports if extended cleanly off-tick)
  - `src-tauri/tests/plant_coevolution_tests.rs` (new)
  - `docs/ai/planning/` (pre-registered / post-run note)
- **Scope:** Build paired same-seed multi-seed coevolution experiments (`run_paired_ensemble` at
  `ensemble.rs:201`: active herbivory vs herbivore-excluded control + neutral-drift control) plus
  epoch-lagged time-shift reciprocal performance assays (`S43`). Assert strict adherence to
  `EVOLUTION_EXPERIMENT_CONTRACT.md` `ER09` vocabulary (`"lineage"`/`"clade"`/`"ecotype"` only unless
  `AE-S11` + `AE-S14` pass).
- **`DONE WHEN`:**
  - `>= 12`-seed paired ensemble detects significant defence-growth shift and time-shift fitness signal
    with all incomplete runs reported cleanly (`ER11`); `check_docs` & full `cargo test --features desktop` pass.
  - **Negative controls:** (1) Scramble maternal plant trait inheritance randomly each generation ->
    coevolutionary Red Queen lag signal vanishes (`p`-threshold fails); (2) emit `"species"` label without
    `AE-S11` detector pass -> metadata/label check fails red.

---

## Card `P7` — Plant Genesis Phases (`G1` Fixed Founders & `G2` Producers-First Pools, `PLT-S07`)

- **START WHEN:** `P2` + `P4` merged (honoring merged `C12 E3B-PIN` `#260` absent-means-legacy gate and aligning with [`ADR-0014`](../../decisions/ADR-0014-shared-island-host-and-guests.md) [`I11`](2026-11-24-island-cards.md) & `VIAB-2`).
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/live_experiment.rs` (`G1` / `G2` plant founders, `live.plant_genesis_phase`, mixed-phase preflight rejection)
  - `src-tauri/src/core/world_artifact.rs` (`WorldIdentity` genesis phase stamp)
  - `src-tauri/src/core/experiment/manifest.rs` (`ExperimentManifest::fingerprint` propagation)
  - `src-tauri/tests/plant_genesis_tests.rs` (new)
- **Scope:** Implement **`G1`** declared fixed grass, shrub, and tree founder genotypes (tagged
  `"designed founders"`) and **`G2`** producers-first screened plant founder pools on
  `sim_stream::PLANT_GENESIS`, requiring `>= 3` independent pools for robustness (`D-GENESIS-1`). Reject
  any manifest or ensemble mixing `G1`/`G2`/`G3` phases cleanly before RNG work. (`G3` emergent replicator
  spike stays out of production trunk pending its own opt-in ADR.)
- **`DONE WHEN`:**
  - `WorldIdentity` and `ExperimentManifest::fingerprint` distinguish `G1` vs `G2` while keeping
    flag-absent legacy manifests bit-identical (`E3b` manifest tests untouched/passing); producers-first
    screened pools establish stable flora before animal introduction across `>= 3` seeds.
  - **Negative controls:** (1) Submit ensemble combining `G1` and `G2` manifests -> preflight fails with
    `Err`; (2) introduce herbivores before producers reach viability threshold in `G2` -> check fails.

---

## Card `P8` — Inhabiting an Individual Plant, Temporal Views & Renderer Contract (`PLT-S08`)

- **START WHEN:** `P2` + `P3` merged, `ADR-0015` `D-PLANT-EVO-5`/`D-PLANT-EVO-6` resolved, and
  shared-island `Inhabit` host/transport primitives ([`ADR-0014`](../../decisions/ADR-0014-shared-island-host-and-guests.md) [`I1`–`I6`](2026-11-24-island-cards.md)) merged on trunk.
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/observer.rs`, `src-tauri/src/core/world_record/` (`PlantGuestIntent` variants + `CauseId` recording)
  - `src-tauri/src/core/components.rs`, `src-tauri/src/core/telemetry_export.rs`, `src-tauri/src/core/emit.rs` (`PlantRenderSample`, `GrassFieldRenderTile`)
  - `src-tauri/src/commands/`, `src-tauri/src/lib.rs`, `docs/reference/IPC_CONTRACT.md` (`get_plant_guest_history` / sensory summary read-only IPC & generated TypeScript types via `npm run check:docs`)
  - `src/` (orbit camera, growth-ring panel, `Spectate` transition UI, read-only overlays; keeping `npm run check:csp` clean)
  - `src-tauri/tests/plant_inhabit_tests.rs` (new)
- **Scope:** Enable `ObserverPolicy::Inhabit { cause_id }` claiming of individual `Tree`/`Shrub`
  entities (`M1` exclusion of grass cells enforced). Support typed allocation, phenology, and
  seed-trade-off `PlantGuestIntent`s through `CauseId` + `WorldRecord` replay; disallow genotype edits,
  instant growth, locomotion, and weather manipulation; enforce no-respawn -> `Detritus` -> `Spectate`
  on death; expose 3P orbit camera, growth-ring history replay, and optional plant `"sensory"` overlay.
- **`DONE WHEN`:**
  - Offline `WorldRecord` replay with `Inhabit` plant intents matches live `world_checksum` and
    `CausalLedger` `CauseId` attribution bit-for-bit; `Spectate`-only hosts match headless runs;
    `LiveExperimentAdapter` / `ObserverPolicy::Absent` refuses `PlantGuestIntent` sessions;
    `npm run check:docs`, `npm run lint`, `npm run build`, and `cargo test --features desktop` pass.
  - **Negative controls:** (1) Submit forged genotype mutation or direct `soil_moisture` rainfall write
    via `PlantGuestIntent` -> decoder rejects loudly; (2) allow grass-cell claim in Milestone 1 or
    respawn dead inhabited tree -> `PLT-S08` tests fail red; (3) inject local speedup for one guest ->
    clock-sync invariant fails red.
