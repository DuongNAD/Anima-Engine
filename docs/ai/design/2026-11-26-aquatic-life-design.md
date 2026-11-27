---
title: Design — Fish and Aquatic Life (AQUA-D)
phase: design
feature: aquatic-life
status: proposed
owner: anima-dell
last_reviewed: 2026-11-26
description: Proposed design for evolved fish and aquatic life (AQUA-D) reusing RESP-D's Locomotion Media Framework and OutOfWaterGillsOnly mirror, ADR-0019 CPG undulation, buoyancy and salinity traits, closed-EU aquatic food web with INVERT-D IN-9, hybrid Iris Xe DormantCohorts budget, GPU-instanced WorldFish replacement, ADR-0016 aquatic disasters, AQUA-1..5, and AQ-D1..8.
---

# Design — Fish and Aquatic Life (`AQUA-D`)

> **Status: `proposed` (`MSG_20261125_183500`, `MSG_20261125_191000`) — design only, zero code.**
> Trunk baseline verified on `origin/feat/ecological-selection` at `38ea46214a136d59a13af6466aa38975805248d8`.
> Builds directly upon `RESP-D` (`docs/ai/design/2026-11-26-respiration-swimming-suffocation-design.md`, Draft PR `#298`, `VERIFIED` @ `4f91689d`), composes with `WV2-D2` (`docs/ai/design/2026-11-26-world-v2-scale-up-design.md`, Draft PR `#294`, `VERIFIED` @ `6646d1b9`, `WB-D1..WB-D8` approved) and `INVERT-D` (`docs/ai/design/2026-11-26-invertebrates-design.md`, Draft PR `#299`, lead). Empirical constants not verified against raw primary tables in this session are marked **`"chưa xác minh"`**; unmeasured runtime metrics are marked **`to measure`**.

---

## 1. Owner Direction, Scope & Verified Trunk Baseline

The owner added: **`"thêm cả cá và sinh vật dưới nước nữa"`** (`MSG_20261125_183500`). Under [`QD-REALMS`](../../how-to/AGENT_WORKFLOW.md#L320-L323), real aquatic ecosystems open the **water realm first** (`water -> sky -> underground`), turning Anima's oceans, rivers, lakes, and brackish deltas from decorative backdrops into an active, evolving, energy-conserving trophic web.

### 1.1 Verified Trunk State (`38ea4621`)

No simulated aquatic organism exists on trunk today:

| Subsystem | Verified Trunk Anchor (`38ea4621`) | Current Behaviour & Gap |
|---|---|---|
| Aquatic NPP & TTE | [`ecology.rs:63-87, 153-155`](../../../src-tauri/src/core/ecology.rs#L63-L87) | `River = 500`, `Ocean = 250` (*"shelf phytoplankton"*), `DeepOcean = 125` (`NPP_TO_CAPACITY = 0.01`); `LINDEMAN_EFFICIENCY = 0.40` (per-strike assimilation; doc notes ~10% canonical). Consumed only if a land/river edge overlaps, zero aquatic fauna. |
| Decorative fish | [`WorldFish.tsx:10-183`](../../../src/components/Landscape/WorldFish.tsx#L10-L183), [`0.9.0.md:67`](../../release/0.9.0.md#L67) | Pure visual `InstancedMesh` (`1` draw call): flat 2-triangle silhouettes in `12` ocean schools × `20` + up to `10` lake schools (`frustumCulled={false}`, `opacity: 0.6`, `"Decoration, not simulated"`). |
| Navigation & water block | [`terrain_navigation.rs:31-33, 106`](../../../src-tauri/src/core/terrain_navigation.rs#L31-L33) | `DeepOcean = 0`, `Ocean = 1` non-walkable; land agents cannot enter ocean cells (`RESP-D` PR `#298` unlocks `SurfaceWater`/`SubmergedWater` opt-in). |
| CPG swimming drive | [`ai/cpg.rs:3-26`](../../../src-tauri/src/ai/cpg.rs#L3-L26), [`ADR-0019`](../../decisions/ADR-0019-brain-body-interface.md#L31-L79) | Trunk initializes every child oscillator with `phase = 0.0` (standing wave -> zero RFT propulsion under `ADR-0006`). Accepted `ADR-0019` adds per-segment CPG modulation & phase lag. |
| Aggregate pooling (AE3) | [`aggregate_population.rs:1-80`](../../../src-tauri/src/core/aggregate_population.rs#L1-L80) | `DormantCohorts` pools distant agents into chunk cohorts with reservoir-sampled genomes and atomic `Compartment::Dormant` EU conservation (`DehydrateAgentCommand` / `RehydrateCommand`). |
| Closed-EU ledger | [`energy.rs:19-37, 375-377`](../../../src-tauri/crates/anima-domain/src/energy.rs#L19-L37) | `EcosystemBiomass { detritus, plants, animals, dormant }` audited via `closed_total_eu(...)` within `RESIDUAL_ABS_TOLERANCE_EU = 1e-3`. |
| Water temperature proxy | [`climate.rs:39, 76`](../../../src-tauri/crates/anima-worldgen/src/climate.rs#L39-L76) | World v2 tropical maritime baseline: `t0_sea_level_c = 27.5 °C`, highland lapse `6.3 °C/km` (`WV2-D2` `~11°N`). |
| Compute & Intel Iris Xe gates | [`scaling.md:39, 72, 86`](../../paper/data/scaling.md#L39-L86), [`2026-11-26-wv2-6-iris-xe-runbook.md:34-58`](../planning/2026-11-26-wv2-6-iris-xe-runbook.md#L34-L58) | Headless release tick `5.643 ms` p50 at `N = 1,000` (`~N^1.86` toward `77.904 ms` at `N = 4,096` on `i5-14600KF`); Dell Iris Xe `Low` preset budget is **`1,000` total creatures**, **`<= 60` draw calls**, `<= 300,000` creature tris, `5.0 ms` creature frame budget (`33.3 ms` @ `30 FPS`), `CREATURE_NEAR_SKINNED_CAP.low = 8`. |

---

## 2. Evolved Aquatic Organisms (Not Props)

### 2.1 Shared `ADR-0001` Lifecycle & `RESP-D` Framework

Every aquatic organism in `AQUA-D` is a **full evolved creature**, developed once from a genotype and never a scripted fish prefab:

1. **Genotype -> Development -> Brain:** Uses the identical `MorphologyGenotype` -> `develop_at_birth` ([`ADR-0001`](../../decisions/ADR-0001-creature-development-lifecycle.md), `V0`) -> immutable `DevelopedPhenotype` pipeline and [`ADR-0003`](../../decisions/ADR-0003-evolved-per-agent-brains.md) per-agent brain as terrestrial fauna.
2. **Reused `RESP-D` Locomotion Media Framework (not restated):** Under [`CREATURE_DEVELOPMENT_CONTRACT.md` §D04](../../reference/CREATURE_DEVELOPMENT_CONTRACT.md#L112-L116) and `RESP-D §2` (PR `#298`), `AQUA-D` reads strictly from `DevelopedPhenotype`:
   - `LocomotionCapacities { walk, swim, dive, fly_glide, dig_burrow }` (`walk ≈ 0.0`, `swim > theta_swim`, `dive >= 0.0`),
   - `RespirationApparatus { lung, gill, skin, hypoxia_tolerance }` (`gill > 0`, `lung = 0.0` for obligate water breathers; intermediate values permit air-gulping catfish or mudskipper-like littoral transitions without a hard class enum),
   - Derived `HabitatPreference` (`LocomotionMedium::Aquatic`) computed purely from capacities at birth/spawn — **never** inferred from terrain cells.
   - `LocomotionState::{SurfaceWater, SubmergedWater}` transitions governed by brain actions and water depth.

### 2.2 Undulatory Swimming from Segmented Bodies & `ADR-0019` CPG

`AQUA-D` relies on accepted [`ADR-0019`](../../decisions/ADR-0019-brain-body-interface.md) (`JointCpgGene`, per-segment phase lag `Δφ`, amplitude, and frequency) on existing segmented `MorphologyGenotype` bodies ([`ai/cpg.rs:3-26`](../../../src-tauri/src/ai/cpg.rs#L3-L26)):

- **Continuum of swimming gaits (no species/mode enum):**
  - **Anguilliform undulation** emerges automatically when an elongated body (`>= 4` flexible axial segments) carries roughly uniform segmental joint amplitudes and progressive anterior-to-posterior phase lag (`Δφ`), producing tight turning radii suited to mangrove roots, rivers, and reefs.
  - **Carangiform undulation** emerges when evolved joint stiffness concentrates lateral flexure into the posterior caudal peduncle and tail fin while keeping the anterior trunk streamlined, lowering pelagic cruising drag (qualitative biomechanics: Webb 1984; Sfakiotakis, Lane & Davies 1999).
- **Hydromechanical propulsion & CoT scaling:** Consistent with [`QD_REALMS_HABITATS_RESEARCH_2026-11-22.md` §1.1–1.2](../../research/QD_REALMS_HABITATS_RESEARCH_2026-11-22.md#L46-L65), while the engine lacks SI water density and gravity, thrust scales with caudal sweep amplitude × frequency × traveling-wave coherence (`sin Δφ > 0`), eliminating `ADR-0006`'s standing-wave stall. Active swimming cost `locomotion_medium_cost` adds directly into **`total_cost`** ([`world_systems.rs:157`](../../../src-tauri/src/core/world_systems.rs#L157)). Physically, aquatic swimming exhibits lower per-unit-mass-distance Cost of Transport than terrestrial running (Schmidt-Nielsen 1972); exact dimensionless CoT multipliers for Anima's `swim` trait: **`"chưa xác minh"`**.

### 2.3 Gills & Out-of-Water Suffocation (`RESP-D §3` Mirror)

Obligate aquatic organisms share **zero new death timer** — they execute the exact same `RespirationState` & `respiration_system` defined in `RESP-D §3` (PR `#298`):

- **Underwater (`SubmergedWater`):** Aquatic `O2` uptake equals `clamp(gill + k_skin_aq * skin * o2_dissolved_water, 0, 1)`. Dissolved oxygen `o2_dissolved_water` lives outside the EU ledger (mirroring hydration in [`ADR-0007:35`](../../decisions/ADR-0007-live-ecology-sustainability.md#L35)), decreases gently with temperature (`climate.rs`), and drops during localized anoxic blooms/ash (`§8`).
- **Stranded on land (`SurfaceLand`):** When a wave, flood recession, tsunami, or failed beaching puts a `lung = 0, gill > 0` organism onto a dry cell (`LocomotionState::SurfaceLand`), `u_O2` collapses to cutaneous residual `k_skin_air * skin` (`active_hazard = BreathingHazard::OutOfWaterGillsOnly`), draining `oxygen_store` -> incrementing `hypoxia_ticks` -> triggering [`ReclaimAndDespawnAgentCommand`](../../../src-tauri/src/core/mortality.rs#L181) into `Detritus`. Exact Anima tick constants for gill extraction efficiency and aerial stranding tolerance: **`"chưa xác minh"`**.

### 2.4 Depth Range & Buoyancy

Because trunk physics has no SI pressure/metres (`MapBounds` `[200 × 10 × 200]` engine units; [`QD_REALMS_HABITATS_RESEARCH_2026-11-22.md §1.2`](../../research/QD_REALMS_HABITATS_RESEARCH_2026-11-22.md#L61-L65)), `AQUA-D` adopts a bounded **normalized 1D water-column coordinate** `depth_norm ∈ [0.0, 1.0]` per water cell (`0.0 = surface photic`, `1.0 = seafloor`), mapped against two pairs of heritable traits (added to `MorphologyGenotype` / `DevelopedPhenotype` with `#[serde(default)]` legacy compatibility under `D09`):

```rust
pub struct AquaticTraits {
    pub preferred_depth_norm: f32, // Target [0.0..1.0] water-column fraction
    pub depth_tolerance: f32,      // Vertical niche width around preferred_depth_norm
    pub swim_bladder: f32,         // Gas-bladder neutral buoyancy investment in [0.0, 1.0]
    pub lipid_buoyancy: f32,       // Low-density lipid buoyancy vs hydrodynamic density
    pub salinity_optimum: f32,     // Normalized salinity target: 0.0 fresh .. 1.0 marine (~35 PSU)
    pub salinity_breadth: f32,     // Osmoregulatory tolerance (stenohaline -> euryhaline)
}
```

- **Buoyancy trade-off inside `total_cost`:** High `swim_bladder` yields near-zero hovering effort near `preferred_depth_norm` (teleost pelagic/reef hoverer) **at the cost of** physiological vertical-transit lag and an elevated `total_cost` penalty when changing `depth_norm` rapidly (slow gas secretion/resorption). Conversely, `swim_bladder ≈ 0` (shark/tuna/benthic lifestyle, partial `lipid_buoyancy`) eliminates vertical ascent limits for rapid pelagic dives/surfacing, **at the cost of** higher sustained cruising/hovering hydrodynamic lift cost inside `total_cost` (and lipid tissue energy). No single genotype dominates both vertical ambush diving and stationary reef hovering.

---

## 3. Aquatic Habitats, Salinity Tolerance & Temperature (`81.92 km` World v2)

`WV2-D2` (PR `#294`, `WB-D1..WB-D8` approved) expands World v2 to **`81.92 km × 81.92 km`** (`320 m/cell` sim grid) with highland lakes, broad coastal plains (`E3`), estuarine deltas/mangroves (`E4`), continental shelves, and deep basins. Aquatic habitats emerge continuously from topography and hydrology rather than species lists:

1. **Marine (Shelf & Deep Ocean):**
   - **Shelf (`BiomeType::Ocean`):** Shallow photic zone above continental shelf; high surface light drives baseline phytoplankton production (`ecology.rs:70`), warm tropical surface waters (`~27.5 °C`), and dense coastal predator-prey interactions.
   - **Deep Ocean (`BiomeType::DeepOcean`):** Deeper bathymetry (`lower NPP_TO_CAPACITY` at `:72`), cooler/dimmer pelagic layer (`~0.5–1.0` `depth_norm`) plus slower sinking detritus rain onto benthic consumers, functioning as a refuge from shallow predators and surface storms.
2. **Freshwater (Rivers & Lakes):**
   - Mountain/piedmont [`BiomeType::River`](../../../src-tauri/src/core/ecology.rs#L69) channels flowing into lowland oxbows, plus highland crater/rift/glacial lakes added in `WV2-D2` (`E5`). Lower thermal inertia cools highland waters per [`climate.rs:76`](../../../src-tauri/crates/anima-worldgen/src/climate.rs#L76) (`6.3 °C/km` lapse), while higher flow velocity selects for streamlined swimming capacity or sheltered eddy foraging.
3. **Brackish Nurseries (Deltas & Mangroves, `E4`):**
   - Where major rivers meet low-slope coastal plains (`WV2-D2 §4b` 55–65% lowland plain / 20–30% fan-delta shares), shallow water (`column_depth` physically blocking deep-bodied adult pelagic predators via depth/body-height clearance), terrestrial detritus runoff, rich `INVERT-D` larval productivity, and fluctuating salinity combine into **emergent nursery refuges** for small/juvenile fish without any scripted `"is_nursery"` rule.

### 3.1 Salinity Field, Heritable Osmoregulation & Temperature

- **Derived `256²` Salinity Field (`salinity_field: [f32; 65_536]`):** Normalized `S ∈ [0.0, 1.0]` mapping `0.0` freshwater rivers/lakes -> `0.15–0.75` brackish estuarine gradient -> `1.0` open ocean (`~35 PSU`, UNESCO Practical Salinity Scale). Static from worldgen bathymetry + river outflow in Phase 1, updated dynamically only when `ADR-0016` floods or saltwater intrusion (`W1` / `D-WCHG-3`) modify coastal salinity (`§8`).
- **Osmoregulation cost inside `total_cost`:**
  - Broad tolerance (`salinity_breadth` near `1.0`, **euryhaline**) pays a continuous baseline gill/kidney ion-pump maintenance surcharge in `total_cost`, preventing euryhaline fish from outcompeting stenohaline specialists in stable oceans or highland lakes.
  - Salinity mismatch (`excess = (|S_cell - salinity_optimum| - salinity_breadth).max(0.0)`) adds an osmotic stress term `k_osm * excess^2` inside `total_cost` (`world_systems.rs:157`), exhausting and starving misplaced stenohaline organisms naturally without hard instant-kill borders (qualitative physiology: Boeuf & Payan 2001; exact Anima osmotic maintenance fraction: **`"chưa xác minh"`**).
- **Water Temperature (`climate.rs`):** Sea-level water anchors near `t0_sea_level_c = 27.5 °C` (`climate.rs:39`) and scales downward with elevation at `6.3 °C/km` (`:76`) in highland rivers/lakes plus a modest vertical thermocline offset in `DeepOcean` (exact thermal damping factor: **`"chưa xác minh"`**). Temperature feeds standard ectotherm MTE (`mte_basal_metabolism`, `ecology.rs:55, 132`) and caps dissolved `O2`.

---

## 4. Closed-EU Aquatic Food Web (Zero New Source or Sink)

Every energy flow in `AQUA-D` routes exclusively between declared compartments in [`EcosystemBiomass`](../../../src-tauri/crates/anima-domain/src/energy.rs#L19-L38) and obeys `|closed_total_eu - baseline| <= RESIDUAL_ABS_TOLERANCE_EU` (`1e-3`, `energy.rs:123`). (When opt-in [`ADR-0018`](../../decisions/ADR-0018-ecosystem-scales-and-cycles.md) `OpenEnergyFlowLaw` lands later, aquatic phytoplankton cleanly switches its input from `Detritus` to `SolarPrimaryProduction`, just like terrestrial `ResourceField`.)

```
  +-------------------------------------------------------------------+
  |  Compartment::Plants (256² Water Cells, ResourceField extension)  |
  |  [Tier-3 Field] Phytoplankton (Ocean/DeepOcean/Lake) + Macrophytes|
  +----------+-----------------------------------------+--------------+
             ^ regrowth (draws Detritus under closed-EU)| grazed (Type III)
             |                                         v
  +----------+-----------+               +-------------+--------------+
  | Compartment::Detritus| <== waste === | [Tier-3] Zooplankton Field |
  | (Aquatic + Benthic)  | <== deaths == | + INVERT-D Aquatic Larvae  |
  +----------+-----------+               |   (IN-9 via take() API)    |
             ^                           +-------------+--------------+
             | carcasses / total_cost                  | preyed by small fish
             |                                         v
  +----------+-----------+               +-------------+--------------+
  | Semi-Aquatic Land    | <== hunted == | Small Fish (Tier-1 Active  |
  | Predators (RESP-D)   |               | / Tier-2 DormantCohorts)   |
  +----------------------+               +-------------+--------------+
             ^                                         | piscivory (0.40 assim)
             | hunted on shore/shallows                v
             +-------------------------- | Predatory Fish (Tier-1 /   |
                                         | Tier-2 DormantCohorts)     |
                                         +----------------------------+
```

### 4.1 Representation Choice per Trophic Level (Fields vs Agents — and Why)

| Trophic Level | Recommended Representation | Ledger Compartment | Why This Tier (Contract & Scaling Rationale) |
|---|---|---|---|
| **1. Aquatic Primary Production** (phytoplankton, benthic/littoral macrophytes) | **Tier-3 Aggregate Field** (`256²` `ResourceField` extension) | `Compartment::Plants` | Millions of microscopic/clonal producers per `320 m` cell; no individual locomotion or neural choice needed. Beer–Lambert depth & turbidity factor attenuates `biome_carrying_capacity` (`ecology.rs:85`) while keeping closed-EU regrowth strictly gated on `Detritus`. |
| **2a. Planktonic Grazers** (zooplankton) | **Tier-3 Aggregate Field** (`256²` scalar + local biomass pool) | `Compartment::Invertebrates` *(with `INVERT-D`)* or audited aquatic grazer sub-pool (`AQ-D3`) | Individual neural agents for copepods/krill would destroy the `1,000`-creature Iris Xe ceiling (`scaling.md`: `N = 4,096` = `77.9 ms`). Holling Type III (`ecology.rs:115`) grazing protects low-density phytoplankton refuges from total collapse. |
| **2b. Aquatic Insect Larvae** (mayflies, midges, dragonflies) | **Tier-3 Aggregate Field owned by `INVERT-D`** (PR `#299`, card `IN-9`) | `Compartment::Invertebrates` | **Zero duplication with `INVERT-D`:** `AQUA-D` calls strictly `InvertebrateFields::aquatic_larvae_eu(cell)` and consumes via `take(cell, pool, desired, reserve, cap)` (`0` when `IN-9` is not active, so small fish fall back cleanly to zooplankton/macrophytes/small detritivory). |
| **3. Small Fish** (planktivores, herbivorous grazers, invertivores) | **Tier-1 Individual Agents** (active zone) + **Tier-2 `DormantCohorts`** (`§5`) | `Compartment::Animals` / `Dormant` | Schooling micro-habitat choice, CPG swimming morphology evolution, nursery salinity tracking, and predator evasion are genuinely spatial/behavioral traits requiring evolved `MorphologyGenotype` + `ADR-0003` brains. |
| **4. Predatory Fish** (piscivores) | **Tier-1 Individual Agents** (active zone) + **Tier-2 `DormantCohorts`** (`§5`) | `Compartment::Animals` / `Dormant` | Larger body mass, carangiform pursuit vs ambush specialization, and depth hunting emerge continuously along `DevelopedPhenotype` + diet traits (zero scripted `"PredatorSpecies"`). Predation uses `LINDEMAN_EFFICIENCY = 0.40` per-strike assimilation (`ecology.rs:155`), routing `(1 - 0.40)` unassimilated capture straight to `Detritus`. |
| **5. Semi-Aquatic Land Predators** (`RESP-D`) | **Tier-1 Individual Agents** (`RESP-D §4` continuum) | `Compartment::Animals` | Existing land predators with evolved `swim`, `dive`, and `aquatic_diet_affinity` wade/swim into `SurfaceWater`/`SubmergedWater` to capture `AQUA-D` fish, bridging marine/freshwater productivity onto land within the unified `EnergyLedger`. |
| **6. Aquatic Detritus** | **Aggregate Pool (`EcosystemBiomass::detritus`)** + local coastal/benthic nutrient distribution | `Compartment::Detritus` | Every `total_cost` joule (under closed-EU), feeding waste, planktonic natural mortality, and stranded/predated carcass despawn (`ReclaimAndDespawnAgentCommand`, [`mortality.rs:181`](../../../src-tauri/src/core/mortality.rs#L181)) credits `Detritus` deterministically. |

- **Empirical Note (Lindeman 1942 vs Anima Assimilation):** Lindeman (1942) documented canonical whole-ecosystem **trophic transfer efficiency (TTE) of `~10%`** across entire trophic levels after respiratory and non-predatory losses; by contrast, trunk [`LINDEMAN_EFFICIENCY = 0.40`](../../../src-tauri/src/core/ecology.rs#L155) represents **per-capture assimilation efficiency** (`40%` assimilated, `60%` egested waste; whole-web realized TTE in Anima's aquatic chain remains **`"chưa xác minh"` / `to measure`**).

---

## 5. Population Model & Performance Budget (Dell Iris Xe `1,000`-Creature Gate)

Because trunk headless simulation scales super-linearly above `1,000` individual agents (`5.643 ms` p50 at `N = 1,000` -> `77.904 ms` at `N = 4,096` on `i5-14600KF`, [`scaling.md:39, 72`](../../paper/data/scaling.md#L39-L72)) and the Dell Iris Xe runbook caps `Low` at **`1,000` total creatures** within **`5.0 ms` creature frame time** and **`<= 60` draw calls** ([`2026-11-26-wv2-6-iris-xe-runbook.md:34-58`](../planning/2026-11-26-wv2-6-iris-xe-runbook.md#L34-L58)), how distant water populations are represented determines whether `81.92 km` oceans starve land life or blow the frame budget.

### 5.1 Model Comparison (`A` vs `B` vs `C`)

| Option | Architecture | Pros | Cons / Risks | Verdict |
|---|---|---|---|---|
| **(A) Individual Fish Agents Everywhere** | Every fish across the `81.92 km` ocean, lakes, and rivers is a live ECS + `AgentBrain` entity | Purest per-individual evolution and local swimming | Either floods `N > 1,000` (breaking Iris Xe & 60 Hz budgets) or leaves vast oceans nearly empty (< `0.05` fish/km²) | Reject as sole tier |
| **(B) Schools as Pure Aggregate Populations (Strict AE3-style Field)** | All fish represented solely as bulk `256²` / chunk biomass pools with moment equations (`DormantCohorts`-only, [`aggregate_population.rs:1-80`](../../../src-tauri/src/core/aggregate_population.rs#L1-L80)) | Negligible CPU & memory (`O(chunks)`), infinite school sizes | Loses individual embodied CPG swimming, individual neural foraging/escape learning (`ADR-0003`), and genuine predator-prey chases near the player/camera | Reject as sole tier |
| **(C) Hybrid: Tier-1 Individual Agents (Active Focus) + Tier-2 `DormantCohorts` School Cohorts (Recommended, `AQ-D2`)** | Live ECS+brain fish inside the active ecological/observer zone + ecological `DormantCohorts` / pelagic school cohorts across distant water chunks, linked atomically via `DehydrateAgentCommand` / `RehydrateCommand` | Full embodied evolution & CPG swimming where observed; distant oceanic/lake populations persist, migrate along salinity/NPP gradients, and replenish nurseries without blowing `N <= 1,000` | Requires careful reservoir-sampled genotype retention & EU accounting across `Compartment::Dormant <-> Animals` (pattern already proven in `aggregate_population.rs`) | **Recommend** |

### 5.2 Explicit Iris Xe Budget & Zero-Allocation Contract

Under **Option (C)**, total live ECS entities (**terrestrial + semi-aquatic + active aquatic combined**) stay bounded by the existing **`1,000`-creature Dell Iris Xe gate**:

- **Live Tier-1 Active Allocation (`<= 1,000` total):** Target nominal split `~650–750` land/semi-aquatic + **`~250–350` active aquatic Tier-1 fish** (`N_max = 1,000` hard invariant; dynamic hysteresis shifts spare slots when camera/focus centers on deep ocean vs highland land, leaving Tier-2 cohorts outside focus). At `22.5 KiB`/agent brain (`ADR-0003` EB-S12), `350` live fish brains occupy `~7.7 MiB` (`~15.4 MiB` with Baldwin `learned` weights), fitting cleanly inside `16 GB` RAM. Actual Iris Xe frame time for the mixed land+aquatic workload: **`to measure`** against `<= 33.3 ms` (`30 FPS`) and `<= 5.0 ms` creature budget.
- **Hot-Loop Zero-Allocation Guarantee:**
  - All `256²` grids (`phytoplankton` extension, `zooplankton`, `salinity`, `water_depth_o2_proxy`) are flat pre-allocated `Box<[f32; 65_536]>` (`256 KiB` each; `~1.0–1.5 MiB` total), updated on a strided schedule (every `8–16` ticks) matching `ResourceField::step_regrowth_gated_strided` (`ecology.rs`).
  - Aquatic sensory/spatial queries reuse `SpatialHashGrid` (split or layer-tagged by `LocomotionState`) + `Local<Vec<_>>` scratch buffers — **zero per-tick heap allocations** in `build_tick_schedule`.

---

## 6. Rendering: Replacing Decorative `WorldFish.tsx`

[`src/components/Landscape/WorldFish.tsx:10-183`](../../../src/components/Landscape/WorldFish.tsx#L10-L183) currently renders hardcoded circling triangles in **`1` `InstancedMesh` draw call**. Under `AQUA-D`:

1. **When `AquaticLaw` is ON (`live.aquatic` active):**
   - **Disable the decorative random school orbits** in `WorldFish.tsx` (do not double-render fake fish alongside real fish).
   - **Render real Tier-1 aquatic agents + distant Tier-2 cohort visual proxies via GPU-instanced rendering:**
     - **Mid/far & bulk Tier-1 fish (`>= 95%` of visible fish):** Rendered in **`1` `InstancedMesh` draw call** (`AquaticInstancedRenderer`) reading per-instance state attributes streamed from Rust (`position`, `heading`, `body_length`, `cpg_phase`, `cpg_freq`, `cpg_amp`, `color_rgb`). A lightweight vertex shader bends the streamlined finned mesh along its spine (`anguilliform` vs `carangiform` envelope driven directly by the agent's real CPG phase and amplitude — zero extra CPU bone skinning).
     - **Near-camera hero aquatic agents:** Share the global **`CREATURE_NEAR_SKINNED_CAP.low = 8`** articulated renderer cap (`2026-11-26-wv2-6-iris-xe-runbook.md:40`) so nearby predatory chases and coral/mangrove encounters show real multi-segment morphology without exceeding **`<= 60` total draw calls** or **`<= 300,000` creature triangles** on `Low`.
     - **Net Iris Xe draw-call impact:** Replaces `WorldFish.tsx`'s `1` decorative draw call with `1` real instanced aquatic draw call (`+0` to `+1` draw calls net delta).
2. **When `AquaticLaw` is OFF (`None`, default):**
   - Preserve decorative `WorldFish.tsx` **unchanged** as the fallback visual path (`"Decoration, not simulated"`, [`0.9.0.md:67`](../../release/0.9.0.md#L67)), guaranteeing zero visual regression on default presets and `v0.9.0` builds.

---

## 7. Save/Load, Migration, Experiment Contract (`ER01`–`ER15`) & Negative Controls

### 7.1 Persistence, Restore & Migration (`D01`, `D02`, `D09`)

- `SerializedAgent` (snapshot save/load) and `AgentMigrationData` (island migration) serialize `AquaticTraits` inside `DevelopedPhenotype` and `AgentBrain` **verbatim** (`#[serde(default)]` for `D09` backward compatibility: pre-`AQUA-D` saves default to `swim_bladder: 0.0, lipid_buoyancy: 0.0, salinity_optimum: 0.0, salinity_breadth: 1.0`, harmless when `AquaticLaw` is off).
- Restore and migration call `spawn_developed` verbatim — **never** re-running `develop_at_birth` or resetting learned neural weights (`D01`, [`ADR-0003`](../../decisions/ADR-0003-evolved-per-agent-brains.md)).
- `world_checksum` hashes `ZooplanktonField` / aquatic cohort pools **strictly when `AquaticLaw` is active** (the `V4` / `INVERT-D` gating pattern).

### 7.2 Opt-In `WorldLawSet::aquatic` & `SEL2-P2` Bit-Identity

```rust
pub struct WorldLawSet {
    // ... existing fields ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aquatic: Option<AquaticLaw>,
}
```

- **Zero-byte `None` canonicalization (`ADR-0016 §5` pattern):** When `world_laws.aquatic` is `None` (default), canonicalization emits **0 bytes**, preserving `WorldLawSet::baseline().fingerprint()`.
- **`QD-BIO-D2` live-key freeze guard:** `live.aquatic` defaults to off (`0.0` / absent) and must satisfy `validate_post_freeze_keys_default_to_legacy` so frozen **`SEL2-P2`** (`#290`) and every **`prereg_*`** scenario remain **bit-for-bit identical** (early return on `Option<Res<AquaticState>>`, `0` heap allocations, `0` RNG draws, `0` extra components on default land agents).

### 7.3 Mandatory Negative Controls (`AQUA-S01`..`AQUA-S06`)

| Control ID | Test Focus | Assertion / Pass Criterion |
|---|---|---|
| **`AQUA-S01`** | **Off-Gate Bit-Identity (`D09` / `ER15`)** | With `WorldLawSet::aquatic = None` (`live.aquatic = 0`), fixed-seed `SEL2-P2`, `E3`, and `prereg_*` runs yield identical `world_checksum` and trajectory outputs to trunk (`0` RNG draws, `0` canonicalization bytes). |
| **`AQUA-S02`** | **Closed-EU Conservation (`ER04`)** | 10,000+ ticks across Ocean/River/Brackish with spawning, feeding (including `IN-9` when present), predation, `DormantCohorts` dehydration/rehydration, and stranding maintain `|closed_total_eu - baseline| <= 1e-3`. |
| **`AQUA-S03`** | **Out-of-Water Gill Mirror (`RESP-D §3`)** | Moving a `lung = 0, gill > 0` fish onto `SurfaceLand` sets `BreathingHazard::OutOfWaterGillsOnly`, drains `oxygen_store -> 0.0`, despawns cleanly into `Detritus` via `ReclaimAndDespawnAgentCommand`, leaving `Compartment::Animals + Detritus` conserved. |
| **`AQUA-S04`** | **Salinity Niche Trade-Off (`ER09`)** | Stenohaline freshwater (`salinity_optimum = 0`) fish survive inland rivers and fail in Ocean; stenohaline marine fish survive shelf and fail inland; euryhaline generalists pay measurable osmotic baseline cost and lose pure-biome competition in stable extremes. |
| **`AQUA-S05`** | **Hybrid Budget & Iris Xe Guard** | Total live ECS creatures (`land + aquatic`) never exceeds `1,000` during massive plankton blooms (`DormantCohorts` absorbs distant surplus); aquatic tick additions perform `0` heap allocations and instanced render adds `<= 1` net draw call vs `WorldFish.tsx`. |
| **`AQUA-S06`** | **Restore & Migration Invariance (`D01`, `D02`)** | Snapshot save/reload and cross-island migration of swimming fish preserve exact `DevelopedPhenotype` (`AquaticTraits`, `LocomotionCapacities`, `RespirationApparatus`), `AgentBrain`, `RespirationState`, and `RehydrateCommand` EU. |

---

## 8. Disasters (`ADR-0016` / `QD-WCHG` Hazards)

When opt-in [`ADR-0016`](../../decisions/ADR-0016-changing-world-surface.md) / `QD-WCHG` surface disasters (`W1–W7`) are enabled alongside `AquaticLaw`, every aquatic impact acts through shared physical state variables (turbidity, `salinity_field`, water column elevation/flow, `o2_dissolved_water`, `RespirationState`) without scripted species survival rules (all empirical mortality/dose coefficients **`"chưa xác minh"`**):

1. **Volcanic Ash:** Ash fallout (`W2`/`W4`) raises surface water turbidity across affected oceans, lakes, and rivers, multiplicatively reducing photic-zone Beer–Lambert irradiance for phytoplankton (`Plants` primary production drops transiently) while suspended particulates reduce gill gas-exchange efficiency (`u_O2` attenuation in `RESP-D §3.1`, raising hypoxic risk for active pelagic swimmers in shallow plumes). Fast-flowing `River` channels flush ash downstream into coastal deltas, whereas closed highland crater lakes retain turbidity longer, naturally favoring hypoxia-tolerant benthic detritivores and deep-ocean refuges until settling clears the water column.
2. **Flood:** Extreme precipitation/river-stage surges (`W1`) transiently submerge lowland plains (`E3`) and delta margins, temporarily linking oxbow lagoons, widening habitable shallow water (`SurfaceWater`/`SubmergedWater`), and washing terrestrial `Detritus` plus soil nutrients into channels and estuaries. High peak discharge flushes slow weak swimmers downstream (increasing CoT to hold station), and when floodwaters recede, fish that fail to track shrinking channels back to main stems become stranded in `LocomotionState::SurfaceLand`, suffocating via `OutOfWaterGillsOnly` to enrich floodplain `Detritus` without arbitrary vanish rules.
3. **Saltwater Intrusion:** Storm surges, drought-reduced river discharge, or coastal subsidence under `W1`/`D-WCHG-3` push the high-salinity ocean wedge upstream across flat brackish deltas (`E4`) and coastal plains (`E3`), driving local `salinity_field[cell]` toward marine values. Stenohaline freshwater fish (`salinity_optimum ≈ 0`, narrow `salinity_breadth`) suffer compounding osmotic maintenance penalties in `total_cost` and die out or emigrate upstream along the learned `ADR-0003` gradient, while euryhaline estuarine species and marine shelf predators expand landward into flooded channels until freshwater runoff restores the gradient.
4. **Tsunami:** Under `D-WCHG-3`, coastal tsunami runup drives a brief, high-velocity marine inundation across beaches, mangroves, and low-lying coastal plains (`55–65%` plain + `20–30%` fan-delta exposure in `WV2-D2 §4b`). Hydrodynamic scour strips vulnerable nearshore macrophytes and coastal `DormantCohorts` back toward `Detritus`, and retreating backwash strands displaced shelf/reef organisms inland on dry cells where `OutOfWaterGillsOnly` converts their biomass cleanly into terrestrial `Detritus`; subsequent recovery emerges purely from pelagic/deep-ocean (`DeepOcean`) and upstream survivors dispersing back into vacated coastal shelves.

---

## 9. Implementation Cards (`AQUA-1`..`AQUA-5`) & Open Owner Decisions (`AQ-D1`..`AQ-D8`)

Under `QD-REALMS` (`water` first), implementation begins only after **`V0`**, **`RESP-1..RESP-2`** (`RESP-D`), and **`ADR-0019`** land on trunk; **`INVERT-D` `IN-9`** (`aquatic_larvae_eu`) and **`ADR-0016` `W1–W3`** plug in cleanly as optional couplings without blocking `AQUA-1..5`.

### 9.1 Implementation Cards (`AQUA-1`..`AQUA-5`)

| Card | Size | Scope & Deliverables | Hard Dependencies | Negative Controls |
|---|---|---|---|---|
| **`AQUA-1`** | `M` | `WorldLawSet::aquatic: Option<AquaticLaw>`, `live.aquatic` off-default, `AquaticTraits` in `MorphologyGenotype`/`DevelopedPhenotype` (`#[serde(default)]`), `256²` `salinity_field` + water-column depth/temp lookup, `HabitatPreference` aquatic spawn legality | `V0`, `RESP-1` | `AQUA-S01`, `AQUA-S06` |
| **`AQUA-2`** | `M` | `ADR-0019` CPG traveling-wave aquatic thrust + buoyancy (`swim_bladder` vs `lipid_buoyancy`) & osmotic mismatch cost inside `total_cost`; `SubmergedWater` 1D depth band + gill suffocation wiring (`OutOfWaterGillsOnly`) | `AQUA-1`, `RESP-1`, `ADR-0019` | `AQUA-S02`, `AQUA-S03`, `AQUA-S04` |
| **`AQUA-3`** | `M` | Closed-EU aquatic food web: water `ResourceField` depth-attenuated primary production (`Plants`), `ZooplanktonField` Type-III grazer (`§4.1`), optional `INVERT-D` `IN-9` consumer hook, piscivory `0.40` assimilation, `RESP-D` land-water predation | `AQUA-1`, `AQUA-2` *(optional: `INVERT-D` `IN-9`)* | `AQUA-S02`, `AQUA-S04` |
| **`AQUA-4`** | `M` | Hybrid population manager: shared `<= 1,000` live-creature budget + aquatic `DormantCohorts` school pooling (`DehydrateAgentCommand` / `RehydrateCommand` + reservoir sampling), zero-alloc strided updates & headless aquatic scaling benchmark | `AQUA-3` | `AQUA-S05`, `AQUA-S06` |
| **`AQUA-5`** | `M` | GPU-instanced real fish renderer (`1` `InstancedMesh` shader with CPG body-wave vertex bend + shared `CREATURE_NEAR_SKINNED_CAP.low = 8`), gated replacement of decorative `WorldFish.tsx`, `AQUA-D` hazard couplings (`W1–W4`), Iris Xe 30 FPS proof | `AQUA-4`, `RESP-5` *(optional: `ADR-0016` `W1–W3`)* | `AQUA-S01`, `AQUA-S05` |

### 9.2 Open Owner Decisions (`AQ-D1`..`AQ-D8`)

| Decision ID | Question | Options | Recommended Option & Rationale | Fallback if Rejected |
|---|---|---|---|---|
| **`AQ-D1`** | **CPG undulatory physics fidelity** | **(A)** `ADR-0019` traveling-wave CPG amplitude × `sin Δφ` thrust scaling inside existing water drag model; **(B)** full Kirchhoff–Lamb added-mass & 3D buoyancy tensor | **(A)**: matches current gravity-free 2D/heightmap physics (`QD_REALMS_RESEARCH §1.2`) and isolates gait evolution cleanly inside `ADR-0019` | **(B)** later if gravity & SI physics are introduced |
| **`AQ-D2`** | **Population scaling model (`§5`)** | **(A)** Hybrid: active Tier-1 agents (shared `<= 1,000` global cap) + Tier-2 `DormantCohorts` school cohorts; **(B)** strict aggregate-only schools; **(C)** unpooled individual agents | **(A)**: preserves real CPG + brain predation in active habitats while protecting Dell Iris Xe `<= 1,000` & `5.0 ms` limits across `81.92 km` seas | **(B)** if live `<= 350` aquatic budget still stresses low-end frames |
| **`AQ-D3`** | **Zooplankton EU compartment bookkeeping** | **(A)** Track zooplankton inside `Compartment::Invertebrates` alongside `INVERT-D` (or mirror under a unified planktonic/invertebrate term in `closed_total_eu`) when `AquaticLaw` is on; **(B)** add a 6th `Compartment::Zooplankton` | **(A)**: avoids proliferated EU compartments while isolating plankton dynamics from `Animals` & `Detritus` (`INVERT-D` `IN-D3` parity) | **(B)** if telemetry requires separated pelagic plankton vs benthic fauna |
| **`AQ-D4`** | **Depth-column representation (`§2.4`)** | **(A)** Normalized 1D continuous `depth_norm ∈ [0, 1]` trait band per cell (`Surface`/`Photic`/`Benthic` gradients); **(B)** full multi-layer 3D voxel ocean grid | **(A)**: `O(1)` per agent, `0` extra `256³` allocation, aligns with [`QD_REALMS_RESEARCH §1.1`](../../research/QD_REALMS_HABITATS_RESEARCH_2026-11-22.md#L54-L59) | **(B)** deferred past `v0.9.x` |
| **`AQ-D5`** | **Genesis aquatic population seeding** | **(A)** Seed diverse generalized swimming/gill founder morphs across shallow shelf + lowland rivers (`<= 120` active + initial cohorts) ONLY when `AquaticLaw` is enabled; **(B)** require evolution purely from land tetrapods | **(A)**: lets players observe marine/freshwater ecosystems immediately (days `1–10`) while still permitting land-to-water evolution (`RESP-D`) | **(B)** strict land-origin-only experimental preset |
| **`AQ-D6`** | **`Low` quality distant-school visualization** | **(A)** Render `Tier-1` active fish in `1` GPU CPG `InstancedMesh` + optional dim school glyphs for active `DormantCohorts` up to fixed vertex cap; **(B)** hide Tier-2 cohorts visually until rehydrated | **(A)** with hard fixed buffer (stays `1` draw call, keeps `<= 60` draw calls) so distant ocean/lake shelves remain visually alive like legacy `WorldFish.tsx` | **(B)** if Iris Xe vertex fill exceeds `5.0 ms` budget |
| **`AQ-D7`** | **Salinity gradient & coastal coupling timing** | **(A)** Static worldgen coastal/estuary salinity in `AQUA-1`, dynamic river/intrusion updates when `ADR-0016` `W1`/`D-WCHG-3` is active; **(B)** build standalone dynamic 2D Navier–Stokes salt transport first | **(A)**: unblocks `AQUA-1..3` immediately against existing `WV2-D2` bathymetry + river mask while plugging straight into `W1` & tsunami runup | **(B)** standalone advection-diffusion step if salt fronts prove critical |
| **`AQ-D8`** | **Aquatic sensory cost (`Moran et al. 2015`)** | **(A)** Fold vision/photoreceptor maintenance (`cognition_cost` / sensory term in `total_cost`) so deep/turbid water selects for reduced eyes vs lateral-line/chemosensing; **(B)** ignore vision cost | **(A)**: cheap single term in `total_cost` (`QD_REALMS_RESEARCH §4`), produces realistic cave/deep-sea blind traits without special rules | **(B)** defer eye-regression trait |

---

## 10. Sources & Epistemic Verification Ledger

### 10.1 Cited Literature

- **Boeuf, G. & Payan, P. (2001).** *"How should salinity influence fish growth?"* *Comp. Biochem. Physiol. Part C* 130(4):411–423 (osmoregulatory metabolic trade-offs across stenohaline vs euryhaline fish; exact Anima parameter values **`"chưa xác minh"`**).
- **Lindeman, R. L. (1942).** *"The trophic-dynamic aspect of ecology."* *Ecology* 23(4):399–417 (~`10%` whole-trophic-level transfer efficiency across natural food webs).
- **Moran, D., Softley, R. & Warrant, E. J. (2015).** *"The energetic cost of vision and the evolution of eyeless Mexican cavefish."* *Science Advances* 1(8):e1500363 (cited in [`QD_REALMS_HABITATS_RESEARCH_2026-11-22.md §4`](../../research/QD_REALMS_HABITATS_RESEARCH_2026-11-22.md#L111-L114); ~5–15% resting metabolic cost summary **`"chưa xác minh"`** against raw primary table here).
- **Schmidt-Nielsen, K. (1972).** *"Locomotion: energy cost of swimming, flying, and running."* *Science* 177(4045):222–228 (qualitative lower transport cost of swimming vs running; exact Anima multipliers **`"chưa xác minh"`**).
- **Sfakiotakis, M., Lane, D. M. & Davies, J. B. C. (1999).** *"Review of fish swimming modes for aquatic locomotion."* *IEEE J. Oceanic Eng.* 24(2):237–252 (anguilliform vs carangiform kinematics).
- **UNESCO / Millero et al.** Practical Salinity Scale (~`35 PSU` open marine reference salinity).
- **Webb, P. W. (1984).** *"Form and function in fish swimming."* *Sci. Am.* 251(1):72–82 (body morphology trade-offs between maneuvering/anguilliform and sustained pelagic cruising/carangiform).

### 10.2 Fact-Check Ledger

| Category | Verified / Audited Claims | Tagged Unverified / To Measure |
|---|---|---|
| **Trunk Code (`38ea4621`)** | `ecology.rs:63-87,155` (`River=500`, `Ocean=250`, `DeepOcean=125`, `NPP_TO_CAPACITY=0.01`, `LINDEMAN_EFFICIENCY=0.40`); `WorldFish.tsx:10-183` (`12×20` ocean + up to `10` lake schools, `1` draw call); `terrain_navigation.rs:31-33`; `cpg.rs:3-26` + `ADR-0019`; `aggregate_population.rs:1-80` (`DormantCohorts`); `energy.rs:19-37,375-377`; `climate.rs:39,76`; `scaling.md` (`5.643 ms` p50 @ `N=1,000`, `~N^1.86`); Iris Xe runbook (`1,000` creatures, `<= 60` draw calls, `5.0 ms` creature budget). | Mixed land+aquatic Iris Xe frame ms, vertex cost with CPG shader, realized whole-web aquatic TTE (`"chưa xác minh"` / `to measure`). |
| **Biology & Hazards** | Qualitative mechanisms cited to Lindeman 1942, Schmidt-Nielsen 1972, Webb 1984, Sfakiotakis et al. 1999, Boeuf & Payan 2001, Moran et al. 2015. | Exact swimming CoT coefficients, gill `O2` affinity/aerial survival ticks, osmoregulatory metabolic fraction, turbidity/ash gill lethality, salinity stress slopes (**`"chưa xác minh"`**). |
