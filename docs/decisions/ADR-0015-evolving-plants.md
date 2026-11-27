---
title: ADR-0015 — Evolving Plants (Organisms, Fields, Plant Genesis, Coevolution & Inhabiting)
status: accepted
owner: simulation-architecture
last_reviewed: 2026-11-26
decision_date: 2026-11-26
supersedes: none
superseded_by: none
---

# ADR-0015 — Evolving Plants (Organisms, Fields, Plant Genesis, Coevolution & Inhabiting)

> **Status: `accepted` (`2026-11-26`, `QD-PLANT` in [`docs/how-to/AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md), PR `#305` / `31a45f9f`): owner approved all six gates `D-PLANT-EVO-1`–`D-PLANT-EVO-6` as recommended (Option A).**
> Do **not** begin Rust or frontend implementation of evolving plants until the owner accepts this
> ADR and answers `D-PLANT-EVO-1`–`D-PLANT-EVO-6` (per `QD-BIO-D2`, `C12 E3B-PIN` `#260` @ `7e383b8b` is now
> merged at `src-tauri/tests/prereg_e3b_manifest_tests.rs:84, 120, 574-603, 754-761`, permitting opt-in
> post-freeze `live.*` keys and observables provided absent-means-legacy `0.0 == Default` holds).
> Initiated per `QD-LIFE` (`MSG_20261124_124500`, corrected in `MSG_20261124_131500`),
> `QD-GENESIS` (`MSG_20261124_134500`, `140000`), and `QD-PLAYRULES` (`MSG_20261124_150000`).
> Code anchors below were verified at `902e505d58dbe034a1771bcea352742c296985d9` and refreshed/re-verified
> on `origin/feat/ecological-selection` at trunk commit `aef12cb641d8d1e02dc35c97f2e74ee2fbb87b93`.

---

## 1. Context & Verified Trunk Baseline (`902e505`)

Under owner decision **`QD-LIFE`** (*"To be like the real world, evolution and growing up must
apply to both animals and plants; inhabiting applies to plants as well as animals"*), plants can no
longer remain static props and an unselective scalar pasture grid. Auditing trunk
`902e505d58dbe034a1771bcea352742c296985d9` establishes four concrete baselines:

1. **Pasture grass (`ResourceField`) is a non-evolving biomass grid (`src-tauri/src/core/ecology.rs:331-358, 422-494`):**
   - `ResourceField` stores per-cell `r: Vec<f32>` (standing biomass) and static biome-derived
     `r_max: Vec<f32>` with uniform scalar `growth_rate: f32`.
   - `resource_field_regrowth_system` (`src-tauri/src/core/environmental_systems.rs:35-90`) runs
     `ResourceField::step_regrowth_gated_strided` (`REGROWTH_STRIDE = 4`, drawing biomass EU out of
     `pool.detritus` into `pool.plants` via exact measured `f32 -> f64` deltas).
   - Herbivores graze via `herbivore_grazing_system` (`environmental_systems.rs:97-173`) and
     `DormantCohorts::graze_all_with_reserve` (`src-tauri/src/core/aggregate_population.rs:858`),
     bounded below by opt-in `PlantReservePolicy` (`src-tauri/src/core/resources.rs:678`,
     `LIVE_KEY_PLANT_RESERVE` at `src-tauri/src/core/live_experiment.rs:235`). Grass carries no
     heritable traits, defence, drought adaptation, or genetic differentiation.
2. **Trees are clonal, immortal, non-evolving props with detritus-claim fruiting:**
   - `Tree` (`src-tauri/src/core/components.rs:137-144`; serialized by `SerializedTree` at
     `src-tauri/src/core/simulation_state.rs:334-343`, validated at
     `src-tauri/src/core/snapshot.rs:602-626`) holds `current_fruit`, `max_fruit`,
     `fruit_growth_rate`, `time_since_last_drop`, `seed_drop_cooldown`, and `seed_spread_radius`.
   - `fruit_growth_system` (`environmental_systems.rs:6-28`) advances `current_fruit` using static
     `r_max[i]`. `seed_dropping_system` (`:242-296`) clones identical `Tree` parameters up to
     `EnvironmentalSpawnSettings::max_tree_count = 50` (`resources.rs:1205-1227`), and trees never
     grow physically, respire, defend against browsers, or die (`Carcass`/detritus turnover).
   - When prey eat tree fruit in `detect_environmental_collisions_system`
     (`environmental_systems.rs:351-385`), EU is drawn straight from `Compartment::Detritus` via
     `EnergyEvent::Fruiting` (`:370`), rather than from living `Plant` biomass fixed by sunlight.
3. **Soil climate/nutrients (`DynamicFields`) & coupled `V5` spec exist, but plant evolution is absent:**
   - `DynamicFields` (`src-tauri/src/core/dynamic_fields.rs:66-108`) pre-allocates `soil_moisture`,
     `soil_nitrogen`, `soil_organic`, `air_temperature`, `precipitation`, `root_cover`,
     `cum_nutrient_uptake` (`:106`), `water_budget_residual` (`:400`), and
     `nutrient_budget_residual` (`:461`). Scheduling `DynamicFields` and coupling grass/`Tree`
     abiotic limits + canopy/root feedback belongs to `V1` and
     [`2026-11-24-v5-fields-plants-feedback-spec.md`](../ai/planning/2026-11-24-v5-fields-plants-feedback-spec.md).
4. **Performance headroom, renderer data seam & freeze pins constrain additions:**
   - Measured headless scaling (`docs/paper/data/scaling.md:39, 72, 86`;
     `src-tauri/benches/live_schedule_scaling.rs:32`) already consumes `5.643 ms` p50 at
     `N = 1,000` mobile agents (`33.9%` of a `16.67 ms` 60 Hz frame on `i5-14600KF`, scaling near
     `N^1.86`). Spawning thousands of individual plant ECS bodies without pooling and benchmarking
     would exhaust the 60 Hz budget (`BENCH-ENTITIES` required first).
   - UI tree rendering reads `EnvironmentalElement { type: "tree", x, y, radius, resources }`
     (`components.rs:149-156`) via `src/PixiViewport.tsx:604-648` and
     `src/components/Landscape/LiveEnvironment.tsx:20-310`, while grass maps
     `SpatialDensityReport::vegetation_density` (`src-tauri/src/core/telemetry_export.rs:420, 495`).
   - `LIVE_KEYS` (`31` keys at `live_experiment.rs:280`) and `28` emitted observables are pinned by
     `E3B_FREEZE_LIVE_KEYS` and `E3B_FREEZE_OBSERVABLE_IDS` (`src-tauri/tests/prereg_e3b_manifest_tests.rs:84, 120, 574-603, 754-761`,
     merged `C12 E3B-PIN` `PR #260` @ `7e383b8b`), permitting additive post-freeze keys only when
     absent-means-legacy (`0.0 == Default`, `QD-BIO-D2`).

---

## 2. Plants as Organisms: Genotype, Development, Lifecycle & Reproduction

Under this ADR, opt-in evolving flora elevate plants from passive counters to evolvable organisms
subject to [`ADR-0001`](ADR-0001-creature-development-lifecycle.md) (once-at-birth development),
[`ADR-0002`](ADR-0002-world-laws-and-exotic-energy.md) /
[`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) (`ER05` costed
traits, `ER08` determinism, `ER09` species gate), and [`ADR-0003`](ADR-0003-evolved-per-agent-brains.md)
(no inheritance of acquired states).

### 2.1. Heritable `PlantGenotype` & Trade-Off Structure (`ER05`)

Every plant taxon/lineage carries a declared, versioned `PlantGenotype` encoding bounded biological
trade-offs rather than free fitness (`ER05`):

| Trait Group | Fields (Normalized / Physical Bounded Units) | Selection & Energy Trade-Off |
|---|---|---|
| **Growth & Morphology** | `growth_rate`, `target_height`, `canopy_radius_ratio`, `root_depth`, `wood_density` | Higher height/canopy shades competitors (`V5` `f_light`), but demands greater structural EU construction + stem respiration and raises wind/drought vulnerability; deeper roots access groundwater at higher root maintenance cost. |
| **Abiotic Physiology** | `drought_tolerance`, `thermal_optimum`, `shade_tolerance`, `nutrient_efficiency`, `light_use_efficiency` | Osmotic/stomatal drought tolerance and shade survival carry baseline metabolic overhead so no generalist dominates all biomes (`soil_moisture` / `soil_nitrogen` / `air_temperature` gradients). |
| **Reproduction & Dispersal** | `seed_size`, `seed_count`, `dispersal_radius`, `flowering_age_ticks`, `pollen_radius` | Explicit offspring-size vs offspring-number conservation (`seed_eu_budget >= seed_size * seed_count + dispersal_coat_cost`): larger seeds survive deep shade/drought better; smaller seeds travel farther. |
| **Defence & Signalling** | `palatability`, `structural_defence` (thorns/tough leaf), `chemical_defence` (toxins/tannins), `volatile_signal_capacity` | Structural and chemical defences lower herbivore bite intake or increase grazer handling/toxin cost in `total_cost`, billed continuously from the plant's own metabolic `total_cost` budget (`metabolic_decay_system` / plant upkeep pass). `volatile_signal_capacity` prepares opt-in grazing-induced chemical emission for planned `ADR-0017` (`QD-SIGNALS`). |

### 2.2. Once-at-Germination Development, Somatic Growth & Non-Lamarckian Inheritance

- **Development at germination (`ADR-0001`, `RESEARCH_REVIEW` §3):** All `PlantGenotype -> PlantPhenotype`
  trait bounds, target dimensions, and physiological curves evaluate **exactly once at germination**
  from `PlantGenotype` + immutable `SeedEnvSample` (no runtime L-systems or reaction norms re-evaluated
  during growth). Restore from snapshot and world/migration load deserialize saved size/state verbatim.
- **Explicit individual plant life cycle (`PlantStage`):**
  1. `Seed`: dormant or germinating propagule holding embryonic EU, dispersal position, and fixed germination viability window (`Seed` reservoir decays cleanly to `Detritus` on failure).
  2. `Sapling`: juvenile stage allocating net photosynthetic surplus toward physical scaling (`height`, `canopy_radius`, `root_depth`, `stem_mass`) toward genotype `target_*`, vulnerable to understory shading and grazing (`ADR-0013`-aligned ontogenetic scale invariant).
  3. `Adult`: structurally mature individual balancing root/canopy maintenance, chemical/structural defence, fruit/reserve storage, and growth up to genotype plateau.
  4. `Seeding`: flowers, releases/receives pollen, provisions `Seed` propagules / fruit payloads out of its own living reserve.
  5. `Senescent / Dead`: age-dependent mortality (`senescence_rate` / drought-starvation exhaustion / fatal disturbance) converts remaining structural + reserve biomass 100% into `Detritus` (and decomposing plant litter / deadfall tracked alongside `V4` carcasses and `V5` `soil_organic`) — eliminating immortal clones.
- **Single metabolic sink path:** All basal tissue upkeep, defence synthesis, fruiting overhead, and root pumping sum inside plant `total_cost` before crediting detritus/heat sink (tracked consistently alongside animal respiration `world_systems.rs:157, 174-177, 222-224` and `EnergyLedger`), preventing any untracked cost outside `total_cost` (`CLAUDE.md` hard rule).
- **Non-Lamarckian rule (`ADR-0003`):** Runtime allocation plasticity (natural seasonal shifts or guest `Inhabit` allocation intents in §7) modifies the current organism's phenotype trajectory **only** and dies with the plant; progeny inherit `PlantGenotype` recombined from parents + mutation only.

### 2.3. Plant Reproduction (`QD-PLAYRULES` #2, Decided 2026-11-24)

Per owner decision **`QD-PLAYRULES` #2** (`MSG_20261124_150000`, *"as biologically right as possible"*):

- **Hermaphroditic sexuality (trees, shrubs & flowering grasses):** Every sexually mature individual carries both ovule and pollen capacity (`PlantSex::Hermaphrodite`).
- **Outcrossing via pollen & seed dispersal:** Pollen donors are sampled deterministically (derived `sim_stream::PLANT_EVOLUTION`, spatially bounded within `pollen_radius`). Recombination occurs homologously over shared `PlantGenotype` loci, with **viability / fertilization compatibility decaying smoothly with parent genetic distance**, enabling reproductive isolation to emerge naturally across geographic or habitat barriers (`ADR-0016` link). Resulting seeds disperse within `dispersal_radius` around the maternal plant (or neighbor cell for grass).
- **Self-fertilization allowed with explicit inbreeding cost (`D-PLANT-EVO-4`):** Selfing succeeds when outcross pollen is scarce or isolated (permitting single-seed island colonization), but incurs a declared **explicit inbreeding / seed-viability cost** because haploid plant trait vectors lack natural diploid recessive load (`RESEARCH_REVIEW` §4 `B7`). Selfed lineages must pay higher embryonic loss / reduced vigor, tunable via `D-PLANT-EVO-4`.
- **Clonal spread (grass & creeping flora):** Grasses/herbs may additionally spread clonally (vegetative stolons/tillers) into neighboring cells when cell density overflows capacity, inheriting maternal cell trait summary + somatic mutation rate (0 if exact clonal expansion, small mutation if configured) alongside sexual pollen/seed flux.

---

## 3. Scale Architecture for 60 Hz & Zero-Allocation Ticks

| Option | Design | 60 Hz & Zero-Alloc Compliance | `QD-LIFE` Compatibility | Verdict |
|---|---|---|---|---|
| **A (Recommended)** | **Hybrid Two-Tier (`Individual` Trees/Shrubs + `Population Field` Evolving Grass)** matching `ADR-0018` `QD-ECOSCALE` Tiers 1 & 3 | Pre-allocated individual plant pool/ECS (`O(M_trees)`, bounded `<= ~500..1,000` active individuals via spatial competition & canopy exclusion) + pre-allocated SoA trait-distribution buffers on `ResourceField` (`REGROWTH_STRIDE = 4`), **`0` heap allocations/tick**, gated on `P1` `BENCH-ENTITIES`. | Trees & shrubs are true individuals (`Inhabit`-eligible, individual genome, growth rings); grass cells evolve via selection/dispersal across cell trait distributions (`Watch`/`Chronicle`-eligible). | **Recommended (`D-PLANT-EVO-1 = A`)** |
| **B** | **All plants as individual ECS entities** (including every grass tussock / moss patch) | Exceeds `5.643 ms @ N=1,000` scaling budget (`scaling.md:72`) by orders of magnitude (`~10^4–10^6` blades/clumps). | Supports inhabiting grass blades, but unplayable at 60 Hz. | **Rejected** |
| **C** | **All plants strictly as grid fields** (even trees become grid layers) | Fastest CPU path (`O(grid)`), zero entity overhead. | Violates `QD-LIFE` (`MSG_20261124_131500`): removes individual inhabitable trees/shrubs, growth rings, and `Tree` `WorldRecord` identity. | **Rejected** |

Under **Option A**, the pre-allocated per-cell `EvolvingGrassField` tracks continuous trait statistics (per-cell biomass `ResourceField::r[i]` plus trait mean vector `mu_trait[k][i]` and bounded additive genetic variance / bin weights `var_trait[k][i]` in fixed `Vec<f32>` SoA arrays):
- Herbivores selectively harvest cells weighted by palatability/defence, differentially shifting remaining cell biomass & surviving sub-cohort traits (`selection`), spatial pollen/seed advection diffuses trait distributions between adjacent walkable cells (`gene flow`), and bounded genetic variance injects declared `mut_rate` diversity under derived `sim_stream::PLANT_EVOLUTION`.
- Spatial overlap between grazing `Morphology`-bearing herbivores (`herbivore_grazing_system`), dormant cohorts (`DormantCohorts::graze_all_with_reserve`), and `EvolvingGrassField` requires **zero `HashMap` lookups and zero per-tick allocations**.

---

## 4. Energy, Soil Nutrients & Exact Mapping Onto `ResourceField`

### 4.1. Compatibility With Closed-EU Default & Opt-In Open Energy Flow (`ADR-0002`, `QD-BIO-D1`, `V5`)

Evolving plants integrate cleanly into both energy regimes without rewriting the `f64` ledger (`RESEARCH_REVIEW` §3 rejects any wholesale `i64` rewrite):

1. **Primary ecosystem path (with merged [`ADR-0018`](ADR-0018-ecosystem-scales-and-cycles.md) (`#263`, `proposed`) `OpenEnergyFlowLaw` active):**
   - Solar irradiance (`EnergyEvent::SolarPrimaryProduction`) enters living grass cells (`EvolvingGrassField`) and individual trees/shrubs (`IndividualPlant`) via genotype/cell `light_use_efficiency` multiplied by abiotic limits `f_w(soil_moisture) * f_n(soil_nitrogen) * f_t(air_temperature) * f_light(canopy_cover)` ([`2026-11-24-v5-fields-plants-feedback-spec.md`](../ai/planning/2026-11-24-v5-fields-plants-feedback-spec.md) §2.2).
   - Plant maintenance, root pumping, chemical/structural defence synthesis, and reproductive seed-coat overhead charge `total_cost` and dissipate respired fractions via `EnergyEvent::RespiredHeatLoss` (heat sink) and shed litter / dead wood (`Dead` trees -> `Detritus` / `V4` carcass-decomposition pool -> `DynamicFields::soil_organic` -> mineral `soil_nitrogen` tracked against `DynamicFields::cum_nutrient_uptake` at [`dynamic_fields.rs:106, 461`](../../src-tauri/src/core/dynamic_fields.rs#L106)).
2. **Closed-EU standalone path (`open_energy_flow = None` + `plant_evolution = Some(...)`):**
   - Plant primary production draws strictly from `Compartment::Detritus` via two-pass budget scaling matching `ResourceField::step_regrowth_gated_strided` (`ecology.rs:453-494`), while plant upkeep, defence, senescence, and unassimilated waste return 100% to `Compartment::Detritus`.
   - Total conservation residual holds at `|closed_total_eu - locked_baseline| <= RESIDUAL_ABS_TOLERANCE_EU` (`1e-3`, [`src-tauri/crates/anima-domain/src/energy.rs:123`](../../src-tauri/crates/anima-domain/src/energy.rs#L123)).
3. **Replacing the trunk fruit-claim shortcut (`environmental_systems.rs:357-380`):**
   - With `plant_evolution` active, an individual tree's fruit belongs to the tree's own synthesized `PlantBiomass` (`Compartment::Plants` / `fruit_eu + structural_eu + reserve_eu`), funded prior to ripening from photosynthesis/detritus rather than drawing free `Compartment::Detritus` at the moment an animal picks fruit (`EnergyEvent::Fruiting`). Every eaten plant unit (grass bite or individual browse/fruit) debits actual stored `f32 -> f64` plant energy and credits animal/spill 1:1 in `EnergyLedger`.

### 4.2. Mapping Onto Today's `ResourceField` & Flag-Off Bit-Identity

- `ResourceField::r[i]` (`ecology.rs:340`) **remains the sole authoritative standing grass biomass array**. All existing readers — `herbivore_grazing_system` (`environmental_systems.rs:97-173`), `DormantCohorts::graze_all_with_reserve` (`aggregate_population.rs:858`), `PlantReservePolicy` (`resources.rs:678`), and `SpatialDensityReport::vegetation_density` (`telemetry_export.rs:420, 495`) — read the exact same `ResourceField` buffer without duplicating biomass stores or introducing double counting.
- **Bit-identical legacy rollback:** When `WorldLawSet.plant_evolution` is `None` and `live.plant_evolution` is absent/`0.0`, `EvolvingGrassField` and individual plant evolution pools are inactive/unallocated in hot loops, `seed_dropping_system` / `fruit_growth_system` / `resource_field_regrowth_system` run the unmodified trunk path (`902e505`), **zero extra RNG draws** occur on `SimRng`, and `E2`, `E3`, `E3b`, and `SEL2-P2` checksums stay bit-for-bit identical.
- **Dedicated derived PRNG streams (`resources.rs:925-948`):** Reserve `sim_stream::PLANT_EVOLUTION = 6` and `sim_stream::PLANT_GENESIS = 7` via `derived_rng(run_seed, ...)` so plant mutation, pollen/seed dispersal, and plant founder sampling never perturb animal `SimRng`, `EVOLUTION`, or `LIVE_GENESIS_BRAINS` draw order.

---

## 5. Plant Genesis (`QD-GENESIS` `G1` -> `G2` -> `G3`)

Today's trunk spawns genesis animals from fixed manifest founders (`live.founders`, `live.founder_layout`, `live.founder_genotype`, `live.predator_fraction` at `live_experiment.rs:88-118`, where `live.founder_genotype = 1.0` removes the tail leaf) and fixed identical trees (`live.trees` at `live_experiment.rs:3136-3154`, hardcoded `EnvironmentalSpawnSettings::default()` at `resources.rs:1215-1227`). Consistent with **`QD-GENESIS`** (`MSG_20261124_134500` & `140000`), plant genesis follows a strict three-phase progression:

1. **`G1` — Fixed Plant Founders (for the Demo, now):**
   - Alongside animal founder genotypes (`live.founder_genotype = 1.0`) and `live.predator_fraction`, genesis places a hard-coded, declared baseline plant set: **one declared grass genotype distribution**, **one declared shrub genotype**, and **one declared tree genotype** (placed cleanly across walkable biome-appropriate land cells).
   - Tagged visibly as `"designed founders"` in the genesis manifest, UI treatment banner, and chronicle.
2. **`G2` — Viability-Screened Plant Founder Pool (upgrade):**
   - Sample candidate plant genotypes across trait distributions on dedicated stream `sim_stream::PLANT_GENESIS`, keeping only founders passing an explicit `VIAB-2`-style multi-generation closed/open-budget viability screen before island start.
   - **Producers first:** Plants establish sustainable standing crop and seed/pollen balance (`G2` producer trigger, aligned with [`ADR-0014`](ADR-0014-shared-island-host-and-guests.md) `D-GENESIS-1`) *before* herbivores (and then predators) enter the ecosystem. Shared-island and science conclusions require robustness across `>= 3` independent founder pools rather than one hand-picked start.
3. **`G3` — Emergent First Organisms (time-boxed research spike):**
   - Treated strictly as an opt-in, separate world-law spike (`<= 2 weeks` of agent work) where minimal self-replicating genotype tapes compete and decode into valid genotypes until crossing `G2`'s viability threshold (explicitly declaring that prebiotic chemistry / chemical origin of life is **not modeled**). Legacy `G1`/`G2` paths remain untouched.

Every run stamps its exact `QD-GENESIS` phase (`G1 | G2 | G3`) into `WorldIdentity` (`world_artifact.rs:122`) and `ExperimentManifest::fingerprint` (`manifest.rs:448`); experiment preflight validation **must reject** any ensemble attempting to mix runs from different genesis phases.

---

## 6. Herbivore–Plant Coevolution (Red Queen) & Evidence Gates

Because plants possess heritable growth/defence trade-offs (`structural_defence`, `chemical_defence`, `palatability`, `target_height`, `seed_size`) that respond directly to browsing and grazing pressure:

- **Bidirectional selective feedback:** Intense herbivory favors defended/low-palatability or compensatory-regrowing grass and tall/spiny woody plants; rising plant defence reduces herbivore per-bite assimilation or adds metabolic detoxification cost inside animal `total_cost`, selecting in turn for herbivore traits (additive future evolvable herbivore digestion/detox traits per `RESEARCH_REVIEW` Tier C `C3`, **never** retiring existing `Predator`/`Prey` marker tags at `components.rs:8-12`, `world_systems.rs:137`).
- **Contract-compliant Red Queen (`S43`) verification ([`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) §ER09, `AE-S11`, `AE-S14`):**
  - Coevolution claims require **paired same-seed ensembles** (`run_paired_ensemble` at `src-tauri/src/core/experiment_runner/ensemble.rs:201`) contrasting grazed treatment vs ungrazed control + neutral-drift control, plus time-shift fitness assays (`S43`, e.g., herbivore populations from epoch `T` tested against contemporary vs ancestral/future plant traits) rather than single-run screenshots or correlations.
  - Telemetry and chronicle copy strictly enforce `"lineage"`, `"clade"`, `"ecotype"`, and `"morph"`; `"species"` (`"loài"`) is forbidden unless both `AE-S11` (sustained multi-generation trait + compatibility gap separation) and `AE-S14` pass (`RESEARCH_REVIEW` §3 rejection enforced).

---

## 7. Inhabiting a Plant (`QD-LIFE` Correction, `MSG_20261124_131500`)

Replacing any "animals only" restriction, owner correction **`QD-LIFE`** (*"đóng vai áp dụng cả với thực vật"*) extends `ObserverPolicy::Inhabit { cause_id }` ([`src-tauri/src/core/observer.rs:49-116`](../../src-tauri/src/core/observer.rs#L49-L116)) to individual flora:

- **What can be inhabited (Milestone 1):** Strictly **individual trees and shrubs** (`ADR-0015` Tier 1 individuals). `EvolvingGrassField` grid cells represent spatial populations rather than individuals; guests may **watch, inspect, and follow/pin** a grass cell's lineage trajectory in the chronicle, never steer it directly (any future `"patch"` mode is deferred to open post-M1 gate `D-PLANT-EVO-5`).
- **Guest intents & hard limits (all routed through typed intents -> `CauseId` interventions):**
  - Permitted `PlantGuestIntent` controls strictly stay within the plant's fixed `PlantGenotype` bounds and existing EU reserves:
    1. **Resource allocation split** (`roots` vs `height/canopy` vs `chemical/structural defence` vs `reproduction/storage`), normalized and clamped per tick;
    2. **Phenology timing** (trigger flowering / pollen release / fruiting / seed release once physiological threshold & EU reserve permit);
    3. **Reproductive investment trade-off** (`seed_size` vs `seed_count` within available maternal reproductive EU).
  - **Forbidden:** Mutating `PlantGenotype`, instant growth skips, locomotion/root displacement, spawning free water/nutrients/EU outside `EnergyLedger`, or altering weather/rainfall (`env` interventions remain host-only).
  - **No Lamarck & death handling:** Guest allocation choices die with the inhabited individual (`ADR-0003`); dispersed seeds inherit `PlantGenotype` only. Upon death (senescence, drought, wildfire/deforestation, fatal browsing), **no respawn occurs**: remaining structural biomass enters `Detritus` / `V4`/`V5` decay, the guest transitions automatically to `Spectate` at the death site, and may optionally claim one of its living offspring seedlings, a young lineage neighbor, or another host-allowed organism with **zero state or allocation carryover** (`QD-PLAYRULES` #3).
- **Time scale & temporal plant UI (`QD-PLAYRULES` #1):**
  - Shared islands advance on one authoritative synchronized ecological clock (`1 day = 20 min`, `1 year = 90 min`, anchored to `~1–2 h` typical mobile animal lifespan). **The host never warps or accelerates simulation time locally for a single plant guest.**
  - Because trees grow and turn over across longer horizons, plant guests receive read-only temporal visualization views derived off-tick from `WorldRecord` + ledger history: **annual/seasonal growth-ring cross-section view** (allocation, drought stress, browsing damage per season), **time-lapse replay of this plant's own recorded history**, and **seasonal/soil overlay lenses**. Whether a host may unlock a shared faster island clock *when every connected guest on that island is inhabiting a plant* remains open gate `D-PLANT-EVO-6`.
- **Camera & science boundary:**
  - Default camera: **third-person orbit** centered on the inhabited tree/shrub (`D-CAM-1` plant counterpart).
  - Optional post-M1 **sensory overlay (`D-CAM-2` plant counterpart)**: visualizes *only* local physical percepts the individual plant senses (`incident light / canopy shade`, `local soil_moisture`, `local soil_nitrogen`, and `recent grazing/browsing damage`).
  - Under [`observer.rs:88`](../../src-tauri/src/core/observer.rs#L88), any `Inhabit` session (`animal` or `plant`) has `is_comparable_to_headless() == false`, records all `PlantGuestIntent` frames into `WorldRecord` with `CauseId` attribution, and is strictly excluded from headless science confirmation ensembles (`ER01`–`ER08`).

---

## 8. Viewer / Renderer Data Contract (Data Only, No Art Decisions)

Extending trunk DTOs cleanly without dictating meshes, shaders, or visual style:

1. **Individual plants (`EnvironmentalElement` successor / `PlantRenderSample`):** Expose deterministic stable `plant_id`, `taxon_rank` (`"tree" | "shrub"`), `stage` (`Seed | Sapling | Adult | Seeding | Dead`), world `(x, z)`, `stem_height`, `canopy_radius`, `root_radius_hint`, `fruit_ratio`, `defence_index`, `flowering_active`, and `lineage_id` (plus inhabited flag when viewed on a shared island). Preserves existing `EnvironmentalElement` (`components.rs:149-156`) for legacy mode.
2. **Grass population field (`GrassFieldRenderTile` over `SpatialDensityReport::vegetation_density` at `telemetry_export.rs:420`):** Expose downsampled per-tile standing density `r / r_max`, dominant defence/palatability trait summary, and clonal/flowering indicator without inventing visual biomes or `"species"` badges in frontend TypeScript (`ER10` observable metadata rule).
3. **Plant guest temporal queries (off-tick / IPC on-demand):** Read-only summary slices for growth-ring seasonal records, local `(light, water, N, damage)` sensory readings, and historical `WorldRecord` bookmarks for the inhabited `plant_id` (`0` tick allocations).

---

## 9. Opt-In Rollback, Manifest Fingerprint & Explicit Exclusions (`RESEARCH_REVIEW` §3)

- **Opt-in schema (`laws.rs:28-32, 79` & `live_experiment.rs:280`):** Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub plant_evolution: Option<PlantEvolutionLaw>` on `WorldLawSet` and `live.plant_evolution` / `live.plant_genesis_phase` manifest keys (permitted by merged `C12 E3B-PIN` `#260` under `QD-BIO-D2` provided absent-means-legacy `0.0 == Default` holds at `prereg_e3b_manifest_tests.rs:574-603`). Inside `WorldLawSet::canonicalize()` (`laws.rs:85-95`), `plant_evolution: None` **must emit zero tag bytes** (matching `WorldIdentity::sim_world_sha256` at `manifest.rs:459-464`, unlike `exotic_energy` which emits `tag(0x00)` at `laws.rs:90`), keeping every legacy `WorldLawSet::baseline().fingerprint()` unchanged.
- **Conditional snapshot validation & `world_checksum` (`snapshot.rs:602-626, 789`):** Hash evolving grass trait arrays (`EvolvingGrassField`) and active `IndividualPlant` stage/genotype/reserve state **only** when `plant_evolution` is enabled; absent/disabled leaves `world_checksum`, `SerializedTree`, and all pre-ADR-0015 save fixtures bit-identical.
- **Explicit rejections upheld from `RESEARCH_REVIEW_2026-11-24_modern_biology.md` §3:**
  - Zero post-germination L-systems or ongoing genotype developmental plasticity (`ADR-0001`).
  - Zero energy drains or defence/growth maintenance outside `total_cost` (`CLAUDE.md` hard rule).
  - Zero wholesale `i64` ledger or `Threefry`/`Philox` PRNG swaps (`sim_stream` uses existing ChaCha `derived_rng`).
  - Zero oracle RHP/kin sensors (`volatile_signal_capacity` connects strictly to physical chemical fields).
  - Zero automatic `"species"` / `"loài"` labels without passing `AE-S11` and `AE-S14`.
  - Zero always-on metrics or telemetry touching simulation buffers mid-tick without zero-alloc / schedule isolation (`telemetry_export` runs off-tick / post-census).

---

## 10. Open Owner Decision Gates (`D-PLANT-EVO-1`–`D-PLANT-EVO-6`)

> **Resolved (`2026-11-26`, `QD-PLANT` in [`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md)):** Every gate `D-PLANT-EVO-1`–`D-PLANT-EVO-6` below is **decided Option (A)** as recommended.

The design principles in §2–§9 implement `QD-LIFE`, `QD-GENESIS`, and `QD-PLAYRULES`. Before implementation cards (`P1`–`P8`) start, the owner decides six scoped parameters on acceptance of `ADR-0015`:

| D-Slot | Question | Options (Lead Recommendation First) |
|---|---|---|
| **`D-PLANT-EVO-1`**<br>*Decided: (A), `QD-PLANT`* | Scale representation architecture (§3) | **(A) Recommended:** Hybrid Two-Tier (`Individual` trees/shrubs + `Population Field` evolving grass trait distributions). **(B)** All-individual plants (blocked on `scaling.md` 60 Hz budget). **(C)** Field-only plants (drops `QD-LIFE` individual inhabiting). |
| **`D-PLANT-EVO-2`**<br>*Decided: (A), `QD-PLANT`* | Maximum active individual tree/shrub pool ceiling & `BENCH-ENTITIES` threshold | **(A) Recommended:** Hard 60 Hz frame budget gate via `P1` (`BENCH-ENTITIES`, matching `5.643 ms` headless floor -> target default cap `~256–500` active trees/shrubs depending on benchmark knee, overriding legacy flat `50` at `resources.rs:1218` only when opt-in law is enabled). **(B)** Keep flat cap `50` across all islands. |
| **`D-PLANT-EVO-3`**<br>*Decided: (A), `QD-PLANT`* | Per-cell evolving grass trait encoding (`EvolvingGrassField`) | **(A) Recommended:** Mean + bounded genetic variance (`mu[k][i]`, `var[k][i]`) over quantized key life-history traits (`growth_rate`, `drought_tolerance`, `defence`, `palatability`, `seed_dispersal`) — fixed `O(K * N_cells)` memory, strided with `REGROWTH_STRIDE = 4`. **(B)** Discretized `M`-morph competition histogram per cell (e.g., `M = 4..8` morph biomass columns summing to `ResourceField::r[i]`). |
| **`D-PLANT-EVO-4`**<br>*Decided: (A), `QD-PLANT`* | Plant self-fertilization inbreeding/viability cost (`QD-PLAYRULES` #2, `B7`) | **(A) Recommended:** Configurable genetic-load germination/vigor penalty in `PlantEvolutionLaw` (default e.g. `30%` viability reduction / extra embryonic EU cost per selfed seed, tuned so colonizers can still found new patches while outcrossed populations outcompete inbred stands). **(B)** Flat germination rate regardless of selfing vs outcrossing (risks neutralizing sexual selection). |
| **`D-PLANT-EVO-5`**<br>*Decided: (A), `QD-PLANT`* | Post-Milestone-1 grass-cell `"patch"` inhabit mode (`QD-LIFE`, `MSG_20261124_131500`) | **(A) Recommended:** Defer `"patch"` allocation control for grass tiles until after Milestone 1 (`Trees`/`Shrubs` individual-only in M1, grass watch/chronicle-only). **(B)** Include experimental patch-controller allocation in Milestone 1 `P8`. |
| **`D-PLANT-EVO-6`**<br>*Decided: (A), `QD-PLANT`* | Shared island time speed when *only* plant guests are active (`MSG_20261124_131500`) | **(A) Recommended:** Keep shared island clock fixed at `1 day = 20 min` / `1 year = 90 min` whenever any guest is connected (animal or plant), reserving speedup solely for empty/prehistory islands (`QD-PLAYRULES` #1). **(B)** Allow host-configured shared fast-forward (e.g., `2x–10x`) strictly when 100% of connected active `Inhabit` guests are plants and all guests consent. |

---

## 11. Acceptance Evidence Gates (`PLT-S01`–`PLT-S08`) & Roadmap

Implementation proceeds strictly through sequenced cards **`P1`–`P8`** in
[`docs/ai/planning/2026-11-24-plant-cards.md`](../ai/planning/2026-11-24-plant-cards.md):

| Gate | Requirement Summary | Mandatory Negative Control |
|---|---|---|
| **`PLT-S01`** | Flag-off / absent `PlantEvolutionLaw` produces bit-identical `world_checksum`, `ResourceField.r`, `Tree` state, and zero extra `SimRng` draws across default, `E3b`, and `V5` seeds. | Enable minimal `PlantEvolutionLaw` default -> checksum diverges / preflight pins catch drift as expected. |
| **`PLT-S02`** | `BENCH-ENTITIES` (`P1`) proves `N = 1,000` animals + `500` / `2,000` sessile plant entities + `256x256` `EvolvingGrassField` sweep stays within 60 Hz budget with **`TrackingAllocator` = `0` heap allocs/tick**. | Trigger 1 `Vec` growth/allocation inside regrowth or seed dispersal tick -> alloc counter `> 0` fails gate. |
| **`PLT-S03`** | Dual EU + H2O + N conservation holds across thousands of ticks (`open_energy_flow = None` -> `\|closed_eu\| <= 1e-3`; `open_energy_flow = Some` -> `\|R_open\| <= 1e-3`; `V5` `\|nutrient_residual\|/scale < 1e-4`; fruit eats from living `Plant` reserve, not free `Detritus`). | Skip deducting seed coat, defence `total_cost`, or eaten fruit from `PlantBiomass` -> residual exceeds `1e-3` fails red. |
| **`PLT-S04`** | Once-at-germination `ADR-0001` & no-Lamarck `ADR-0003`: runtime scaling, seasonal shading, and guest allocation intents never mutate `PlantGenotype`; save/restore (`run N == run K -> save -> load -> run N-K`) produces bit-identical checksums. | Re-develop `target_height` on load or copy runtime plasticity into seed `PlantGenotype` -> tests fail red. |
| **`PLT-S05`** | Sexual outcrossing vs selfing penalty: pollen recombination follows genetic distance compatibility decay; selfed seeds pay declared `D-PLANT-EVO-4` inbreeding penalty; clonal grass spread preserves spatial gene flow. | Set compatibility threshold to ignore genetic distance or drop selfing penalty -> isolation / fitness tests fail red. |
| **`PLT-S06`** | Plant defence selection & Red Queen (`S43`): sustained grazing elevates defence/low-palatability trait frequency in grazed ensembles vs paired same-seed ungrazed & neutral-drift controls (`ER09`, `AE-S14`); no unverified `"species"` strings emitted. | Shuffled trait inheritance / zero bite penalty for high `chemical_defence` -> selection differential collapses & fails gate. |
| **`PLT-S07`** | Phased `QD-GENESIS` (`G1`/`G2`/`G3`): `WorldIdentity` + `ExperimentManifest::fingerprint` distinct per phase, mixed-phase ensemble rejected (`Err`), `G2` producers-first pool screen reproduces across `>= 3` seeds cleanly. | Mix a `G1` and `G2` manifest in one ensemble -> preflight must return `Err` (`mixed genesis phases`). |
| **`PLT-S08`** | Plant `Inhabit` safety & replay: `PlantGuestIntent` recorded in `WorldRecord` with `CauseId`, never overrides genotype or `Environment` rainfall, enforces zero respawn + carcass detritus return on death, keeps `Spectate`-only island parity, and exposes `ObserverPolicy::Inhabit` science block (`is_comparable_to_headless() == false`). | Allow guest genotype mutation, grass-cell direct claim in M1, or respawn on death -> `PLT-S08` negative controls fail red. |

---

## 12. Related Documents

- Lifecycle & development invariants: [`ADR-0001`](ADR-0001-creature-development-lifecycle.md), [`CREATURE_DEVELOPMENT_CONTRACT.md`](../reference/CREATURE_DEVELOPMENT_CONTRACT.md), [`ADR-0013`](ADR-0013-live-creature-lifecycle.md)
- World laws, observers & science gates: [`ADR-0002`](ADR-0002-world-laws-and-exotic-energy.md), [`ADR-0003`](ADR-0003-evolved-per-agent-brains.md), [`ADR-0004`](ADR-0004-observer-as-declared-intervention.md), [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md)
- Soil, water & decomposition specs: [`2026-11-24-v4-carcasses-decomposition-spec.md`](../ai/planning/2026-11-24-v4-carcasses-decomposition-spec.md), [`2026-11-24-v5-fields-plants-feedback-spec.md`](../ai/planning/2026-11-24-v5-fields-plants-feedback-spec.md), [`2026-11-24-v10-soak-leak-spec.md`](../ai/planning/2026-11-24-v10-soak-leak-spec.md)
- Benchmarks & card roadmap: [`docs/paper/data/scaling.md`](../paper/data/scaling.md), [`docs/ai/planning/2026-11-24-plant-cards.md`](../ai/planning/2026-11-24-plant-cards.md)
- Merged wave companion ADRs: [`ADR-0014`](ADR-0014-shared-island-host-and-guests.md) (`PR #256`, `accepted`, shared islands & `QD-GENESIS` `D-GENESIS-1`/`D-GENESIS-2`), [`ADR-0018`](ADR-0018-ecosystem-scales-and-cycles.md) (`PR #263`, `proposed`, `OpenEnergyFlowLaw`, `QD-ECOSCALE` three tiers).
- Unmerged wave ADR references (cited in backticks until merged to trunk): `ADR-0016` (allopatric worlds / changing surface), `ADR-0017` (evolved chemical/acoustic signals), `ADR-0019` (`PR #259`, brain–body interface).
