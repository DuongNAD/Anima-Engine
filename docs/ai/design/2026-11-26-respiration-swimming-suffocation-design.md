---
phase: design
feature: respiration-and-locomotion
title: Card RESP-D — Locomotion Media Framework, Unified Respiration, Swimming Continuum & Gas Suffocation Design
description: Design proposal defining the shared Locomotion Media Framework (walk, swim, dive, fly/glide, dig/burrow; lungs/gills/skin under ADR-0001/D04), unified respiration model (drowning, volcanic CO2, wildfire smoke, burrow hypoxia/hypercapnia), semi-aquatic trait continuum, time compression mapping, 256^2 dense-gas overlay, closed-EU mortality, opt-in WorldLawSet safety, cards RESP-1..RESP-5, and open owner decisions RD-D1..RD-D9
status: proposed
owner: anima-dell
last_reviewed: 2026-11-26
---

# `RESP-D` — Locomotion Media Framework, Unified Respiration, Swimming Continuum & Gas Suffocation

> **Status: `proposed` (documents only — zero Rust/TS edits, zero `docs/ai/README.md` edits, zero `cargo` runs).**
> - **Provenance:** Verified against `origin/feat/ecological-selection` at `38ea46214a136d59a13af6466aa38975805248d8` (`#293` `QD-WV2-LOOK` + `QD-WV2-BIG`). Companion unmerged/pending tracks cited strictly in backticks: PR `#294` (`WV2-D2`, `docs/ai/design/2026-11-26-world-v2-scale-up-design.md`, `QD-WV2-D2` approved in `#295`), PR `#296` (`QD-WCHG` decisions on [`ADR-0016`](../../decisions/ADR-0016-changing-world-surface.md), whose own `status: proposed -> accepted` move is a separate follow-up), **`INVERT-D` (in progress, lead)**, **`AQUA-D`**, **`AERO-D`** (lead), and **`FOSS-D`** (lead).
> - **Epistemic rule:** Every physiological, allometric, or toxicological constant not yet verified against primary data inside this repo is explicitly marked **`"chưa xác minh"`** (`PROPOSED`, subject to calibration or source check before code lock).

---

## 1. Owner Direction, Scope & Verified Trunk Baseline (`38ea4621`)

### 1.1 Exact Owner Prompts Covered

1. Drowning: *"sinh vật trên cạn nếu bị chìm ở dưới nước bao lâu ngạt thở chết"* (how long before land creatures submerged underwater suffocate and die).
2. Swimming: *"sinh vật có thể bơi"* (creatures that can swim).
3. Volcanic gas: *"gặp núi lửa phun trào thì có khí CO2 đúng k, ngạt thở sau bao nhiêu thời gian"* (volcanic eruptions emit `CO2` — how long until suffocation).
4. Semi-aquatic continuum: *"sinh vật trên cạn cx có thể xuống nc nhưng cần theo quy luật sinh học và thực tế, sinh vật lưỡng cư, ví dụ gấu bắc cực có thể lao xuống săn cá"* (terrestrial creatures can also enter water following real biology; amphibians too; e.g., polar bear diving to hunt fish).
5. Living-world expansion (`MSG_20261125_185000`, `MSG_20261125_191000`): **`RESP-D` owns the shared Locomotion Media Framework** (covering `walk`, `swim`, `dive`, `fly/glide`, and `dig/burrow`, plus `lungs`, `gills`, and `skin` respiration) reused without restatement by `AQUA-D`, the lead's `AERO-D`, and the lead's `FOSS-D`, and referencing **`INVERT-D (in progress, lead)`** for shared insect/invertebrate guilds.

### 1.2 Verified Trunk State (`38ea4621`)

| Contract / Module | Verified Status at `38ea4621` | Consequence for `RESP-D` |
|---|---|---|
| [`CREATURE_DEVELOPMENT_CONTRACT.md`](../../reference/CREATURE_DEVELOPMENT_CONTRACT.md#L81-L90) | `DevelopedPhenotype`, `EnvSample`, `EnvironmentSnapshot`, `develop_at_birth`, `LocomotionMedium`, and `HabitatPreference` are all **`❌ chưa`** (`decode_genotype` still has 7 direct call sites). | Respiration & locomotion traits belong on `MorphologyGenotype` + versioned `DevelopedPhenotype` under `ADR-0001` (`V0`), never re-developed on `Restore`/`Migration` (D01/D02). |
| [`terrain_navigation.rs`](../../../src-tauri/src/core/terrain_navigation.rs#L31-L42) & [`L197-L311`](../../../src-tauri/src/core/terrain_navigation.rs#L197-L311) | `is_biome_walkable` blocks `DeepOcean` (`0`) and `Ocean` (`1`), while treating `River` (`3`) and land (`2, 4..=10`) as walkable; `terrain_navigation_system` snaps offending roots back to [`LastWalkablePosition`](../../../src-tauri/src/core/components.rs#L689-L703). | Legacy `Ocean`/`DeepOcean` block remains the **exact flag-off behaviour** when `WorldLawSet.respiration == None`. |
| [`world_systems.rs`](../../../src-tauri/src/core/world_systems.rs#L157-L177) & [`energy_ledger.rs`](../../../src-tauri/src/core/energy_ledger.rs#L35-L54) | `total_cost = k_base + maintenance + activity_cost + cognition_cost`; `decay = total_cost * dt` flows into `EcosystemBiomass::detritus`. | Every swim/dive/burrow/flight/thermoregulatory cost **must enter `total_cost`** (`ADR-0003` hard rule), never subtracted externally. |
| [`mortality.rs`](../../../src-tauri/src/core/mortality.rs#L181-L242) & [`world_record/types.rs`](../../../src-tauri/src/core/world_record/types.rs#L264-L270) | `DeathCause` has `{Starvation, Dehydration}`; `DeathCauseTag` has `{Starvation, Dehydration, Predation, Reclaim, EpochReplacement}`; `ReclaimAndDespawnAgentCommand` is the sole atomic death primitive. | Suffocation ends solely via `ReclaimAndDespawnAgentCommand` -> `Detritus`, with extended observable cause tags (`§8`). |
| [`ecology.rs`](../../../src-tauri/src/core/ecology.rs#L63-L77) & [`WorldFish.tsx`](../../../src/components/Landscape/WorldFish.tsx#L10-L183) | `biome_npp` assigns NPP to `River` (`500`), `Ocean` (`250`), and `DeepOcean` (`125`), while `WorldFish.tsx` is a decorative 1-draw-call `InstancedMesh` with **zero aquatic ECS agents**. | Aquatic hunting needs an explicit food-source plan (`§4.4`, `RD-D3`) before fish prey exist (`AQUA-D`). |
| [`laws.rs`](../../../src-tauri/src/core/experiment/laws.rs#L28-L42) & [`L85-L118`](../../../src-tauri/src/core/experiment/laws.rs#L85-L118) | `WorldLawSet` holds `{schema_version, baseline_energy, exotic_energy}`; `None` tags are deterministically canonicalized. | New `respiration: Option<RespirationLaw>` must emit **0 bytes** when `None` so `#290` `SEL2-P2` and `prereg_*` stay bit-identical (`§10`). |

---

## 2. Locomotion Media Framework (Shared by `RESP-D`, `AQUA-D`, `AERO-D`, `FOSS-D`)

> **Shared architectural home:** `RESP-D` defines the single locomotion-media and respiration-apparatus contract here so `AQUA-D` (aquatic fauna), `AERO-D` (flight, lead), and `FOSS-D` (subterranean/cave fauna, lead) **reuse it directly without restatement or divergence**.

### 2.1 Capacities as Continuous Heritable Traits (Not a Single Enum, Contract D04)

Per [`CREATURE_DEVELOPMENT_CONTRACT.md` §D04](../../reference/CREATURE_DEVELOPMENT_CONTRACT.md#L112-L116) and [`ADR-0001`](../../decisions/ADR-0001-creature-development-lifecycle.md), a creature's locomotion medium is **NOT** inferred from the terrain cell it stands on and is **NOT** a mutually exclusive single enum variant (which could never represent a swimming jaguar, a diving seabird, or a burrowing mole-snake cleanly). Instead, each organism carries a continuous vector of bounded **heritable capacities** in `MorphologyGenotype` (all `f32 in [0.0, 1.0]`):

```rust
pub struct LocomotionCapacities {
    pub walk: f32,       // Terrestrial limb/undulation support on solid ground (legacy default: 1.0)
    pub swim: f32,       // Hydrodynamic propulsion & low aquatic Cost of Transport (legacy default: 0.0)
    pub dive: f32,       // Oxygen store boost, diving bradycardia & depth compression margin (legacy: 0.0)
    pub fly_glide: f32,  // Wing/patagium lift & aerodynamic flight/glide capacity for AERO-D (legacy: 0.0)
    pub dig_burrow: f32, // Fossorial forelimb/head excavation & tunnel locomotion for FOSS-D (legacy: 0.0)
}

pub struct RespirationApparatus {
    pub lung: f32,             // Aerial O2 uptake capacity (legacy default: 1.0)
    pub gill: f32,             // Dissolved aquatic O2 extraction capacity for AQUA-D (legacy default: 0.0)
    pub skin: f32,             // Cutaneous gas exchange fraction (wet air or oxygenated water, legacy: 0.0)
    pub hypoxia_tolerance: f32,// Tolerance to low O2 / high CO2 in burrows & thermal vents (legacy: 0.0)
}
```

- **Trade-off coupling (no free generalist):** Each capacity above `0.0` carries morphology/maintenance coupling via `develop_at_birth` and `total_cost` (e.g., dense diving bone/blubber impairs high `fly_glide` wing loading; heavy `swim`/`dig_burrow` limbs increase terrestrial walking drag; `aquatic_insulation` raises land overheating risk; `hypoxia_tolerance` trades off peak aerobic sprint rate). Pure generalists (`1.0` everywhere) are outperformed locally by specialists or balanced semi-specialists.
- **Derived `HabitatPreference` view (D04):** [`HabitatPreference`](../../reference/CREATURE_DEVELOPMENT_CONTRACT.md#L207-L210) (`{ medium, elevation_range }`) becomes a deterministic derived summary read strictly from the creature's own `DevelopedPhenotype` capacities (`Terrestrial`, `SemiAquatic`, `Aquatic`, `Aerial`, `Fossorial`, or composite flags) — used by `is_habitat_legal(habitat, env)` at genesis/migration validation without ever consulting the cell first to mutate the creature.

### 2.2 Development at Birth (`ADR-0001`), Persistence (`D02`) & Legacy Compatibility (`D09`)

1. **`develop_at_birth` (`ADR-0001` / `V0` [`2026-11-24-v0-develop-at-birth-spec.md`](../planning/2026-11-24-v0-develop-at-birth-spec.md)):** Develops `MorphologyGenotype`'s capacities once against `EnvSample` (bounded reaction slopes defaulting to `0.0`, D03) into `DevelopedPhenotype` (`version`, immutable after birth).
2. **Restore & Migration (`D01`, `D02`):** `SerializedAgent` and `AgentMigrationData` serialize `DevelopedPhenotype` (including locomotion & respiration capacities) with `#[serde(default)]`. Restore and migration spawn via `spawn_developed` verbatim with zero re-development.
3. **Legacy saves & default genotypes (`D09`):** Missing capacity fields deserialize cleanly to `{ walk: 1.0, swim: 0.0, dive: 0.0, fly_glide: 0.0, dig_burrow: 0.0 }` and `{ lung: 1.0, gill: 0.0, skin: 0.0, hypoxia_tolerance: 0.0 }`.

### 2.3 `LocomotionState`, Legal Cell Movement & `dig/burrow` Substrate Mechanics

At runtime, each agent tracks an instantaneous `LocomotionState`:

```rust
pub enum LocomotionState {
    SurfaceLand,     // Walking/crawling atop dry or shallow-edge cells
    SurfaceWater,    // Swimming at water surface (air breathing remains available)
    SubmergedWater,  // Diving beneath water surface (air breath-hold or gills required)
    Airborne,        // Active powered flight / soaring / gliding (reused by AERO-D)
    Underground,     // Excavating / occupying subterranean burrow layer (reused by FOSS-D)
}
```

| State / Transition | Entry Legality Check (`is_move_legal`) | Effective Movement & Cost Scaling | Shared Consumer |
|---|---|---|---|
| **`SurfaceLand`** | Walkable cell (`elev_eff > sea_level` or `River`/`Beach`/`Land`), `walk > 0` or amphibious crawl | Baseline [`locomotion.rs`](../../../src-tauri/src/core/locomotion.rs#L51-L54) speed & `total_cost` | Trunk + all |
| **`SurfaceWater`** | Any water cell (`DeepOcean`, `Ocean`, `River`, or `SurfaceDeltaOverlay` flooded cell) when `RespirationLaw` is active | Speed scaled by `(0.18 + 0.82 * swim)`; `swim` CoT penalty `(1.0 + k_drag * (1.0 - swim)^2)` when `swim` is low (`§4.1`) | **`RESP-D`**, **`AQUA-D`** |
| **`SubmergedWater`** | Water cell with column depth `depth_m >= body_height_m`; requires `swim > theta_swim` or `dive > 0` (voluntary dive) | 3D/depth envelope bounded by `dive` / `AQUA-D` depth range; `O2` supply governed by `gill` + `skin` vs breath-hold drain (`§3`) | **`RESP-D`**, **`AQUA-D`** |
| **`Airborne`** | `fly_glide >= theta_fly` and `total_mass <= M_fly_max(fly_glide)` (no collision/ceiling block) | Aerodynamic lift, wind advection & flight power curve in `total_cost` | **`AERO-D` (lead)** |
| **`Underground`** | `dig_burrow >= theta_dig`, non-submerged soil (`hardness < H_max(dig_burrow)`) | **Substrate-hardness cost:** energy & speed penalty scaled by `substrate_hardness(biome, root_cover)` (`Beach`/alluvial loam `0.2`, `Grassland` `0.45`, rooted `Rainforest` via `DynamicFields::root_cover` `0.85`, bedrock/volcanic `1.0` = impassable `§3.3`). | **`FOSS-D` (lead)** |

### 2.4 Unified Energy Cost Rule & Brain Control (`ADR-0003`)

- **Hard EU conservation invariant:** Every extra locomotion cost — inefficient splashing (`1 - swim`), flapping lift (`AERO-D`), substrate excavation (`substrate_hardness * dig_rate` for `FOSS-D`), conductive cold-water heat loss (`§4.3`), and tissue maintenance of gills/lungs/diving store — is added directly as `locomotion_medium_cost + medium_thermal_cost` inside **`total_cost`** in [`metabolic_decay_system`](../../../src-tauri/src/core/world_systems.rs#L157) (`world_systems.rs:174-177`). It therefore drains `homeo.energy` and credits `EcosystemBiomass::detritus` cleanly with `|closed_total_eu - baseline| <= 1e-3`.
- **Action selection (`ADR-0003`):** Brains never teleport across media. When `RespirationLaw` is active, an agent transitions between `SurfaceLand <-> SurfaceWater <-> SubmergedWater` (and `Airborne`/`Underground` under `AERO-D`/`FOSS-D`) strictly through opt-in `ActionGates` / continuous medium-intent actor outputs (`§9`) conditioned on evolved weights and homeostatic state.
- **Shared invertebrate food guilds (`INVERT-D` interface):** Under owner queue update `MSG_20261125_191000`, **`INVERT-D (in progress, lead)`** defines all invertebrate/insect guild fields: soil fauna (feeding `FOSS-D` burrowers), aquatic larvae (feeding `AQUA-D` fish and semi-aquatic foragers), and terrestrial/aerial insects (feeding `AERO-D` insectivores and small predators). `RESP-D` and `AQUA-D` reference **`INVERT-D (in progress, lead)`** directly and **do not** define competing insect fields.

---

## 3. Unified Respiration Model (Drowning, Volcanic `CO2`, Wildfire Smoke & Burrow Hypoxia)

Rather than three unrelated kill timers, every organism running under `RespirationLaw` shares **one** `RespirationState` component updated once per tick in `respiration_system` (scheduled right after locomotion/environment updates and before `mortality_system`):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BreathingHazard {
    #[default]
    None,
    SubmergedNoGills,    // Terrestrial / diving animal underwater holding breath (drowning)
    OutOfWaterGillsOnly, // Pure gill-breather stranded on dry land (AQUA-D mirror)
    VolcanicCo2,         // GasConcentrationField CO2_frac above physiological threshold
    WildfireSmoke,       // SurfaceDeltaOverlay wildfire smoke / anoxia plume (ADR-0016 W2/W3)
    BurrowHypoxiaCo2,    // Underground state gas buildup & O2 depletion (FOSS-D)
}

#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct RespirationState {
    pub oxygen_store: f32,         // Normalized reserve in [0.0, 1.0]; 1.0 = saturated at O2_cap
    pub co2_dose: f32,             // Accumulated hypercapnic/toxic dose in [0.0, 1.0] (1.0 = lethal)
    pub smoke_dose: f32,           // Accumulated particulate/CO smoke dose in [0.0, 1.0] (1.0 = lethal)
    pub hypoxia_ticks: u32,        // Consecutive ticks with oxygen_store <= 0.0 (cf. dehydrating_ticks)
    pub active_hazard: BreathingHazard,
}
```

### 3.1 One Pipeline for Four Hypoxic / Toxic Conditions

At the creature's wrapped `(col, row)` cell and `LocomotionState`:

1. **Compute effective gas uptake fraction `u_O2 ∈ [0, 1]` and toxic concentrations `(C_CO2, C_smoke)`:**
   - **`SurfaceLand` / `Airborne` / `SurfaceWater`:** Aerial `O2` uptake is `(lung + 0.35 * skin)` reduced by atmospheric `CO2` displacement (`1 - C_CO2 / C_anoxia`) and wildfire smoke (`1 - C_smoke`), using `GasConcentrationField.co2_frac[cell]` (`§7`) and `SurfaceDeltaOverlay::burn_heat_timer[cell]` smoke plume (`ADR-0016` `W2`/`W3`).
   - **`SubmergedWater`:** Dissolved aquatic `O2` uptake equals `clamp(gill + k_skin_aq * skin * o2_dissolved_water, 0, 1)` where `o2_dissolved_water` decreases at high water temperature (`climate.rs`). For lung-only animals (`gill = 0`, `skin ≈ 0`), `u_O2 ≈ 0`, so metabolism drains `oxygen_store` cleanly toward `0.0` (**drowning**).
   - **Out-of-water fish (`AQUA-D` mirror):** `LocomotionState::SurfaceLand` with `lung = 0`, `gill > 0` sets `u_O2 = k_skin_air * skin` (very low unless mudskipper/eel-like cutaneous trait exists), draining `oxygen_store` via the **exact same formula** (**aerial suffocation**).
   - **`Underground` (burrow hypoxia & hypercapnia for `FOSS-D`):** Closed tunnel soil pore ventilation is poor: effective ambient `O2` is depressed and burrow `CO2` accumulates (`C_CO2_burrow` rises with burrow depth/density and soil organic respiration, diluted by `soil_porosity` and ventilation vents). An animal with elevated `RespirationApparatus::hypoxia_tolerance` raises `C_CO2_thresh` and lowers metabolic hypoxia sensitivity (`§3.2`).
2. **Update `oxygen_store` and toxic dose buffers (`0` heap allocations):**
   - Oxygen demand rate scales with actual metabolic burn `total_cost / O2_cap(M, dive, bradycardia)` (`§5.1`). When uptake exceeds demand (`u_O2` in clean air or well-oxygenated water for gilled fauna), `oxygen_store` recovers smoothly toward `1.0`. When `u_O2` cannot meet demand, `oxygen_store` drains strictly toward `0.0`.
   - When `oxygen_store <= 0.0`, `hypoxia_ticks` increments (`saturating_add(1)`); once `hypoxia_ticks >= grace_hypoxia_ticks` **or** `co2_dose >= 1.0` **or** `smoke_dose >= 1.0`, `mortality_system` triggers [`ReclaimAndDespawnAgentCommand`](../../../src-tauri/src/core/mortality.rs#L181).
3. **Burrow Hypoxia/High-`CO2` & Substrate Mechanics (`FOSS-D` hook):**
   - `dig/burrow` excavation cost in `total_cost` = `k_dig * (0.2 + substrate_hardness(biome, root_cover)) * move_rate` (`MountainRock` / basaltic caprock hardness = impassable wall).
   - In `LocomotionState::Underground`, surface predators cannot strike without entering burrow or excavating, **but** ambient air renewal drops sharply: without evolved `hypoxia_tolerance` (analogous to naked/blind mole-rat hemoglobin `O2` affinity and hypercapnic blunting — exact burrow `CO2 %` ranges **`"chưa xác minh"`**, commonly cited 2–5% `CO2` / 6–14% `O2` in dense warrens `"chưa xác minh"`), surface animals chasing prey into burrows risk hypoxic/hypercapnic stupor or suffocation while foraging soil invertebrates (`INVERT-D (in progress, lead)`).

---

## 4. From Land to Water: A Trait Continuum, Not Fixed Classes (`MSG_20261125_182000`)

Water use is never a hard-coded "land vs water species switch". It emerges along a continuous evolutionary gradient governed by four heritable traits in `MorphologyGenotype` / `DevelopedPhenotype`:

1. **`swim_capacity ∈ [0, 1]`** — lowers aquatic Cost of Transport and raises swimming thrust (`§2.1`).
2. **`diving_capacity ∈ [0, 1]`** — enlarges functional `O2` store (myoglobin/blood volume scaling + dive bradycardia).
3. **`aquatic_insulation ∈ [0, 1]`** — thick subcutaneous/fat/pelt thermal barrier cutting conductive water heat loss (`§4.3`), trading off terrestrial heat dissipation (`h_diss`) on warm land.
4. **`aquatic_diet_affinity ∈ [0, 1]`** — digestive/behavioral efficiency on submerged vegetation, aquatic invertebrate larvae (`INVERT-D`), or fish (`AQUA-D`).

### 4.1 Biological Reference Points Across the Continuum

Because `QD-WV2-D2` fixes the `v0.9.0` World v2 climate at **`~11°N` tropical maritime** ([`climate.rs:39,76`](../../../src-tauri/crates/anima-worldgen/src/climate.rs#L39-L76): `t0_sea_level_c = 27.5 °C`, highland lapse `6.3 °C/km`), polar bears themselves cannot inhabit this warm world; instead, the **exact same semi-aquatic trait pattern** manifests in tropical river/mangrove/coast predators:

| Continuum Zone | `swim` / `dive` / `insul` / `skin` Profile (Indicative) | Reference Taxa & Biomechanical Anchor | `~11°N` Tropical Analog (`QD-WV2-D2`) | Breath-Hold / Swimming Empirical Status |
|---|---|---|---|---|
| **Obligate Terrestrial** | `swim ~ 0.05`, `dive ~ 0.0`, `insul ~ 0.05` | Ground ungulates, desert rodents | Tapir-like browsers? *(No; browsers vary)* — terrestrial dry savanna / steppe grazers | High aquatic CoT (`~4–10×` walking, Williams 1999 *Phil. Trans. R. Soc. B* — exact multiplier **`"chưa xác minh"`**); short panic swim & fast exhaustion; drowning ~`1–3 min` human-scale (`< 1 min` small mammals, **`"chưa xác minh"`**). |
| **Long-Distance Surface Swimmer / Short Diver** | `swim ~ 0.55–0.75`, `dive ~ 0.15–0.30`, `insul ~ 0.70` | **Polar bear** (*Ursus maritimus*): exceptional endurance surface swims across cold water, yet **short dives usually under `~1 min`** | **Jaguar** (*Panthera onca*, swims wide lowland rivers & stalks caiman/fish), **Fishing cat** (*Prionailurus viverrinus*, mangrove/estuary ambush) | Polar bear endurance swim & `< ~1 min` typical hunt dives (`"chưa xác minh"` for exact seconds/km without primary paper citation); jaguar tropical riparian swimmer (`"chưa xác minh"`). |
| **Semi-Aquatic Diver (Mammal / Reptile)** | `swim ~ 0.70–0.85`, `dive ~ 0.55–0.85`, `insul ~ 0.50–0.80` (endotherm) or `0.15` (ectotherm) | Sea/river otters, beaver, **capybara**, **crocodilians** | **Giant/neotropical river otter** (*Pteronura* / *Lontra*), **Capybara** (*Hydrochoerus hydrochaeris*, lowland plain & oxbows `E3`), **Caiman** (*Caiman crocodilus*, `E4` deltas) | Capybara ~`5 min` submerged predator evasion (`"chưa xác minh"`); crocodilian/caiman low resting `M` + bradycardia enabling `15–30+ min` submergence (`"chưa xác minh"`). |
| **Amphibian (Cutaneous + Aquatic Larva)** | `swim ~ 0.55`, `lung ~ 0.45`, **`skin ~ 0.55`**, terrestrial permeability risk | Frogs, salamanders, caecilians | Tropical treefrogs / stream salamanders in humid `E1` cloud forest & `E4` swamps | Skin breathing supports indefinite submergence in cool aerated water (`"chưa xác minh"`); **aquatic larval stage requires life-stage metamorphosis — explicitly deferred (`§4.5`, `RD-D7`)**. |
| **Obligate Aquatic (`AQUA-D`)** | `walk ~ 0.0`, `swim ~ 0.90–1.0`, `gill ~ 1.0`, `lung ~ 0.0` | Pelagic & freshwater fishes | Shelf marine, estuarine (`E4`), river & rift-lake (`E5`) fish in `AQUA-D` | Covered by companion `AQUA-D`; out-of-water suffocation uses the same `RespirationState` model (`§3`). |

### 4.2 Emergent Hunting Behavior (Zero Scripted Species Logic)

Anima contains **zero species enums or scripted predator behaviors** (e.g., there is no `"PolarBearHuntFish"` rule). A jaguar-like or bear-like semi-aquatic hunter **emerges solely when three preconditions co-occur**:
1. **Morphological affordability:** evolved `swim_capacity`, `diving_capacity`, and `aquatic_insulation` keep aquatic CoT and hypothermia loss below expected caloric gain (`D04` independent check);
2. **Prey / food presence:** edible energy exists in the river/lagoon/shelf cell (`ResourceField` + `INVERT-D` larvae initially, moving to fish agents once `AQUA-D` lands);
3. **Learned / evolved choice (`ADR-0003`):** agent neural brains (`BrainGenotype` + lifetime `learn_step`) experience net positive homeostatic reward when swimming into shallows and avoid deep/cold plunges when `oxygen_store` or `temperature` falls.

### 4.3 Honest Physical Costs Inside `total_cost`

Entering water is never free:
- **Hydrodynamic drag & `swim_cost`:** Aquatic propulsion consumes `k_swim * activity * (1.0 + k_cot * (1.0 - swim_capacity)^2)` routed strictly into **`total_cost`** in [`metabolic_decay_system`](../../../src-tauri/src/core/world_systems.rs#L157).
- **Aquatic conductive heat loss:** Water conducts heat strongly compared to air (~`24×` thermal conductivity `"chưa xác minh"`). In `metabolic_decay_system`, ambient surface water temperature `T_water` is read from `climate.rs` / `TerrainMap.temperatures` / `DynamicFields.air_temperature` (scaled to `°C` around `t0_sea_level_c = 27.5 °C`, dropping with `6.3 °C/km` lapse rate so mountain rivers & páramo lakes `E5`/`E8` are much colder than lowland `E4` deltas!). Heat dissipation rises by `h_water = k_water_cond * (1.0 - 0.85 * aquatic_insulation) * max(0.0, homeo.temperature - T_water)` (`world_systems.rs:168-172`), forcing poorly insulated animals to burn higher `total_cost` or suffer hypothermia.
- **Breath-hold drain & drowning:** Submerged ticks without `gill` deplete `oxygen_store` (`§5`) -> drowning at `hypoxia_ticks >= grace`.
- **Mobility & predation vulnerability:** Water drag slows terrestrial limbs (`0.18 + 0.82 * swim_capacity`), exposing terrestrial invaders to faster aquatic predators (`AQUA-D`).

### 4.4 Aquatic Food Dependency (`ecology.rs` & `WorldFish.tsx`)

Inspecting trunk at `38ea4621` confirms two hard facts:
- [`ecology.rs:63-77`](../../../src-tauri/src/core/ecology.rs#L63-L77) computes Whittaker NPP carrying capacities for water biomes (`BiomeType::River => 500.0`, `BiomeType::Ocean => 250.0` "shelf phytoplankton", `BiomeType::DeepOcean => 125.0`), so `ResourceField` **already supports aquatic primary production**;
- [`src/components/Landscape/WorldFish.tsx:10-183`](../../../src/components/Landscape/WorldFish.tsx#L10-L183) renders purely decorative circling triangles (`makeFishGeometry`, `1` `InstancedMesh` draw call, **zero** ECS components or biomass).

**Staged dependency recommendation (`RD-D3`):**
1. **Stage 1 (`RESP-3`):** Allow semi-aquatic/swimming agents to harvest conserved aquatic `ResourceField` capacity (macrophytes/riparian/phytoplankton) plus **`INVERT-D (in progress, lead)`** aquatic invertebrate larvae through the existing `EnergyLedger::transfer_into_reserve(Detritus -> Reserve)` path, while terrestrial prey fleeing into rivers also remain huntable in shallows.
2. **Stage 2 (`AQUA-D`):** Replace decorative `WorldFish` (when `AquaticLaw` is enabled) with evolved fish agents, unlocking genuine emergent semi-aquatic diving piscivory (`jaguar`/`otter`-style hunting) via `combat_system`.

### 4.5 Amphibian Metamorphosis Boundary (`RD-D7` — Explicitly Deferred)

True amphibians combine (a) high cutaneous gas exchange (`skin_capacity` in §2.1, carrying high desiccation risk via `sweat_rate` / evaporative hydration decay in `world_systems.rs:165`), and (b) an **aquatic larval (tadpole) stage** transforming into a terrestrial/amphibious adult. Because `CREATURE_DEVELOPMENT_CONTRACT.md` **D01** currently permits development strictly once (`Genesis`/`Birth`) and prohibits later life-stage morphological changes during runtime or `Restore`/`Migration`, **metamorphosis is NOT designed here (`RD-D7`)**. Supporting it in a future card will require:
- A versioned `LifeStage { Larva, Metamorphosing, Adult }` state in `DevelopedPhenotype` and `SerializedAgent`,
- A declared **single** ontogenetic transition event `metamorphose_at_maturity` (bounded EU cost, explicit RNG determinism, `CausalLedger` edge), distinct from banned "environment re-paint",
- Paired cutaneuous water-loss coupling (`hydration` drain on dry/warm land scaled by `skin_capacity`).

---

## 5. Breath-Hold Allometry & `CO2` Dose-Response Model

### 5.1 Metabolic Allometry of Breath-Hold (`t_hold`)

In [`ecology.rs:34-56`](../../../src-tauri/src/core/ecology.rs#L34-L56), animal maintenance metabolism obeys Kleiber's law + Arrhenius kinetics (Brown et al. 2004):

`metabolic_rate(M, T) = METABOLIC_NORM * M^(0.75) * boltzmann_factor(T, E_ANIMAL_EV)`

Physiological `O2` storage capacity scales linearly with body mass/blood volume (`∝ M^1.0`, Kooyman 1989 *Diverse Divers* `"chưa xác minh"` for exact species prefactor across clades). Dividing available `O2` store (`∝ M^1.0 * f_dive`) by aerobic consumption rate (`∝ M^0.75 * activity_factor * boltzmann_factor`) gives the fundamental quarter-power breath-hold scaling:

$$t_{\text{hold}}(M, T, d, a) = t_{\text{ref}} \cdot \left(\frac{M}{M_{\text{ref}}}\right)^{0.25} \cdot \text{boltzmann\_factor}(T, E_{\text{ANIMAL\_EV}})^{-1} \cdot f_{\text{dive}}(d) \cdot f_{\text{activity}}(a)$$

where `f_dive(d) = 1.0 + k_dive * d^2` (`d = diving_capacity ∈ [0, 1]`) reflects blood volume, myoglobin, and dive bradycardia, and `f_activity(a)` shrinks `t_hold` during high swimming exertion (`activity_cost`). **Crucial invariant satisfied:** smaller animals carry higher mass-specific metabolic demand (`M^(0.75)/M = M^(-0.25)`) and therefore exhaust their oxygen store and drown **faster** than large calm animals (`t_ref` and `k_dive` marked **`PROPOSED` / `"chưa xác minh"`**).

### 5.2 Volcanic `CO2` Toxic Dose-Response Model

Above normal atmospheric `CO2` (`~0.04% = 0.0004`), carbon dioxide acts both as an asphyxiant displacing `O2` and a direct hypercapnic/acidosis toxin. We model fractional toxic dose accumulation `co2_dose ∈ [0, 1]` via an allometric Haber-like concentration-time exponent (`k_co2`, `C_thresh`, `n` all **`PROPOSED` / `"chưa xác minh"`** pending validation):

$$\frac{d(\text{co2\_dose})}{dt} = \begin{cases} k_{\text{co2}} \cdot \left(\frac{M}{M_{\text{ref}}}\right)^{-0.25} \cdot \left(\max(0,\, C_{\text{CO2}} - C_{\text{thresh}}(h_{\text{tol}}))\right)^{n}, & C_{\text{CO2}} > C_{\text{thresh}} \\ -k_{\text{clear}} \cdot \text{co2\_dose}, & C_{\text{CO2}} \le C_{\text{thresh}} \end{cases}$$

where `C_thresh(h_tol) = C_base + delta_burrow * hypoxia_tolerance` (so fossorial creatures tolerate typical high-`CO2` burrow air while non-adapted surface animals accumulate dose).

**Empirical human reference tiers (explicitly `"chưa xác minh"` vs NIOSH/OSHA/EPA primary tables — NOT hardcoded sim constants):**

| Ambient `CO2` Concentration | Qualitative Physiological Benchmark | Verification Status |
|---|---|---|
| `~4%` (`0.04` vol. frac) | Rough occupational **IDLH** (Immediately Dangerous to Life or Health) threshold; rapid hyperventilation & headache | **`"chưa xác minh"`** — verify against NIOSH IDLH table before coding `RESP-2` |
| `~7–10%` (`0.07–0.10`) | Confusion and **unconsciousness within minutes** | **`"chưa xác minh"`** — verify against NIOSH/CDC/IVHHN volcanic gas guidance |
| `>= 15–20%` (`>= 0.15–0.20`) | Rapid respiratory arrest & **collapse within ~1 minute or less** | **`"chưa xác minh"`** — verify against primary toxicology/volcanic-mazuku case literature |

Smaller-mass animals with faster mass-specific respiration (`(M / M_ref)^(-0.25)`) accumulate lethal `co2_dose = 1.0` proportionately earlier than larger animals at identical plume density. Leaving the gas pocket into clean air (`C <= C_thresh`) clears `co2_dose` smoothly so fleeting border exposure is survivable.

---

## 6. Time Compression Honesty (`1 day = 20 min` Calendar vs Acute Physiological Time)

Per [`AGENT_WORKFLOW.md` `QD-ADR0014`](../../how-to/AGENT_WORKFLOW.md#L494) and [`ADR-0016` §4.3](../../decisions/ADR-0016-changing-world-surface.md#L184-L188):
- **World calendar timescale:** `1 sim day = 20 real minutes` = `1,200 s` (`72×` compression relative to an 86,400-s Earth day); `1 sim year = 90 real minutes`.
- **The `72×` trap on acute asphyxiation:** A literal `72×` compression turns a real `180 s` (`3 min`) terrestrial drowning into `180 / 72 = 2.5` real seconds (`150` ticks at `60 Hz`), and a `60 s` lethal `CO2` exposure into `0.83` seconds (`50` ticks). At those sub-second durations, neither an agent's steering/brain reaction nor a human observer watching Third-Person or Map view can meaningfully register danger, flee uphill, or rescue an animal.

**Proposed Time Mapping (`RD-D2`, explicitly disclosed):**
- **Already dual-timescale on trunk:** Creature walking speed (`1.5–2.0` sim units/s in `locomotion.rs:51-53`), grazing (`step_regrowth`), and combat strikes run on **watchable acute behavioral time** (~`1–2 s` per action), NOT compressed `72×` calendar time (walking 10 km does not cross the screen in milliseconds).
- **Acute Physiological Time Scale (`TimeScaleMode::WatchableAcute`, recommended `RD-D2` Option (a)):** Map real physiological breath-hold/suffocation times via a declared compression factor `tau_phys` (`~6–8×` vs real physiology, **not** `72×` calendar time):
  - Obligate terrestrial small/medium creature (`swim ~ 0.1`, `dive = 0.0`): **`~15–45 real seconds`** (`900–2,700` ticks @ `60 Hz`) from total submersion to drowning (scaled smoothly by `M^0.25`);
  - Severe `CO2 >= 15–20%` plume collapse (`~60 s` real): **`~8–15 real seconds`**; moderate `7–10%` plume (`~3–5 min` real): **`~25–50 real seconds`**;
  - Evolved semi-aquatic diver (`caiman`/`otter` analog, `dive -> 0.85`): **`~2–5+ real minutes`** submerged.
- **Transparency disclosure:** UI/HUD tooltips and the [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) observable metadata explicitly disclose `physiological_time_scale` alongside `1 sim day = 20 real min` (just as `ADR-0016` discloses `+Δt kyr` on Tier-3 geological jumps), so telemetry never confuses real-minute benchmarks with watchable game seconds.

---

## 7. Volcanic Gas as a Bounded `256²` Field (`ADR-0016` Tier-2 Overlay)

### 7.1 Physical Mechanism (`CO2` Density Stratification)

Pure carbon dioxide (`M_CO2 = 44.01 g/mol`) is **`~1.52×` denser than dry air** (`~28.97 g/mol`). In quiescent or low-wind topography, dense volcanic `CO2` hugs the terrain surface, flows downhill like an invisible heavy liquid, and pools to lethal concentrations (`10–90%` `"chưa xác minh"`) inside craters, valleys, and topographic hollows — the exact mechanism behind the **Virunga `"mazuku"` depressions (DR Congo)**, **Lake Nyos (Cameroon, 1986)**, and **Dieng Plateau (Indonesia, 1979)** (exact casualty counts `"chưa xác minh"` unless cited directly).

### 7.2 Bounded Fixed-Memory Data Layout (`256²` Sim Grid)

Respecting `QD-WV2-D2` `WB-D7` (the desktop simulation stays on the `256²` sim grid = `65,536` cells) and `D-WCHG-1` (`#296`), `GasConcentrationField` lives alongside `SurfaceDeltaOverlay` with zero runtime allocations (`0` allocs/tick):

| Field Buffer (`256² = 65,536` cells) | Element Type | Fixed Size | Purpose |
|---|---|---|---|
| `co2_frac: Vec<f32>` | `f32` (`4 B`) | `256 KiB` | Surface-layer `CO2` volume fraction `[0.0004, 1.0]` |
| `co2_scratch: Vec<f32>` | `f32` (`4 B`) | `256 KiB` | Synchronous double-buffer for downhill gravity advection + diffusion |
| *(Optional, behind `RD-D4`)* `irritant_gas: Vec<f32>` | `f32` (`4 B`) | `256 KiB` | Optional `SO2`/`H2S` toxic irritant concentration (`0` bytes unless enabled) |
| **Total Fixed Overhead** | — | **`512 KiB`** *(or `768 KiB` with `RD-D4`)* | Pre-allocated once when `RespirationLaw` is active; `0` bytes when `None` |

### 7.3 Downhill Gravity Flow, Wind Dilution & Event Coupling

1. **Emission:** Tied cleanly to **`ADR-0016` Card `W3` / `CauseId`** volcanic vents, craters (`E8` / `tectonic.rs:417-420`), or hydrothermal/limnic overturn triggers (drawn strictly from `sim_stream::WORLD_DISTURBANCE = 8`, `ADR-0016 §4.2`).
2. **Dense-gas downhill advection:** On each `GasField` step (strided, zero-alloc over active non-ambient dirty chunks), excess density `(co2_frac[i] - AMBIENT_CO2)` flows proportionally down elevation gradients along **effective terrain height** `h_eff[i] = base_elevation[i] + overlay.delta_elev_m[i]` (`ADR-0016 §3.1`), concentrating automatically inside topographic basins (`mazuku` hollows) and caldera bowls.
3. **Wind dilution & turbulent decay:** Horizontal advection and vertical atmospheric dispersion scale with local wind speed (`climate.rs` / seasonal storm state): **calm conditions** retain lethal pools for many minutes, whereas high wind across exposed ridges rapidly dilutes `co2_frac` back toward ambient `0.0004`.
4. **Wildfire smoke coupling:** Active wildfire cells in `SurfaceDeltaOverlay::burn_heat_timer > 0` (`W2`/`W3`) emit neutrally/weakly buoyant `smoke_intensity` downwind (no pooling in cold lakes), feeding directly into the same `RespirationState` toxicity check (`§3`).
5. **Out-of-scope faster volcanic killers (`ADR-0016` `W3`/`D-WCHG-4`):** **Pyroclastic density currents** (extreme thermal/kinetic surges) and **molten lava flows** (`D-WCHG-4` active lava death/burial) kill virtually instantaneously via thermal/mechanical trauma and belong to `ADR-0016` `W3`/`W6`, **not** this slow respiratory suffocation model.

---

## 8. Death, Closed-EU Accounting & Causal Observability

1. **Zero new EU source or sink (`energy_ledger.rs:35-54`, `agent_systems.rs:170-191`):** Suffocation mortality never touches EU formulas outside [`metabolic_decay_system`](../../../src-tauri/src/core/world_systems.rs#L174-L177) and [`ReclaimAndDespawnAgentCommand`](../../../src-tauri/src/core/agent_systems.rs#L170-L191). When `mortality_system` condemns a suffocating creature, it pushes `ReclaimAndDespawnAgentCommand { root: entity }`, atomically transferring 100% of remaining body reserve `homeo.energy` into **`EcosystemBiomass::detritus`** (`|closed_total_eu - baseline| <= 1e-3`).
2. **Causal attribution & telemetry granularity (`RD-D6`):**
   - Extend `DeathCause` ([`mortality.rs:187`](../../../src-tauri/src/core/mortality.rs#L187)) with **`SuffocationDrowning`**, **`SuffocationGas`**, **`SuffocationSmoke`**, and **`SuffocationAerial`** (for beached aquatic organisms in `AQUA-D`), OR a unified `DeathCause::Suffocation(BreathingHazard)` variant, preserving deterministic tie-breaking in `lethal_cause()` (`mortality.rs:220-242`).
   - Mirror the split into [`VitalStatistics`](../../../src-tauri/src/core/resources.rs#L474-L526) (`total_deaths_suffocation_*`) and `WorldRecord` [`DeathCauseTag`](../../../src-tauri/src/core/world_record/types.rs#L264-L270) + `CausalLedger` edge (`EruptionCauseId -> GasConcentrationField -> SuffocationGas`) for full replay & island chronicle observability (`I8`, `AE-S12`).
   - When `RespirationLaw == None`, `VitalStatistics` window counts (`starvation_deaths`, `dehydration_deaths`) and serialized `DeathCauseTag` outputs remain 100% untouched.

---

## 9. Sensory Inputs, Brain Behaviour & Legacy Default (`ADR-0003`)

Per [`ADR-0003`](../../decisions/ADR-0003-evolved-per-agent-brains.md) (`brain_genotype.rs:36-74`, `agent_systems.rs:1633-1845`):
1. **New opt-in sensory inputs (only appended when `RespirationLaw` is enabled; exact `ArchSpec` versioned per `brain_genotype.rs:58-65`):**
   - `cell_water_depth_norm`: normalized water depth under agent (`0.0` on dry land -> `1.0` deep ocean) + forward probe depth (`is_water_ahead`);
   - `oxygen_store`: current internal `[0, 1]` breath reserve (`RespirationState::oxygen_store`);
   - `air_hazard_intensity` & `uphill_clean_gradient`: normalized local `(co2_dose_rate + smoke_dose_rate)` and body-local directional gradient toward lower gas / higher elevation (`local_escape_x`, `local_escape_z`).
2. **Medium choice & escape learning (`ActionGates` + Baldwin reinforcement):**
   - Add opt-in actor intent gates (`dive_intent`, `surface_or_flee_intent`, plus `fly_intent`/`burrow_intent` slots for `AERO-D`/`FOSS-D`) alongside existing `PHEROMONE_EMIT`, `ATTACK_INTENT`, and `FEED_INTENT` ([`action_index`](../../../src-tauri/src/evolution/brain_genotype.rs#L36-L46)).
   - Fold acute hypoxia/hypercapnia drive distress into `HomeostaticState` reward signal for opt-in `lifetime_learning_system` (`learn_step`) while evolutionary selection favors heritable weights that exit water before `oxygen_store` empties and climb uphill out of `mazuku` `CO2` hollows.
3. **Hard Legacy Default Invariant (`RD-D5`, `RD-D8`):**
   - When `WorldLawSet.respiration == None` (the default for `SEL2-P2`, `E3b`, and existing desktop/headless presets):
     - `sensory_system` builds the exact 15-element `state_arr` (`agent_systems.rs:1833-1845`);
     - `ArchSpec::LEGACY` (`15×64×4`) and `EVOLVED_ARCH` (`15×64×8`) remain completely untouched;
     - `terrain_navigation_system` runs the exact `LAND1` shoreline reversion loop (`terrain_navigation.rs:197-311`);
     - zero gas buffers or `RespirationState` updates execute (`0` extra `SimRng` draws, `0` extra bytes, bit-identical trajectories).

---

## 10. Opt-In `WorldLawSet`, Science Safety (`#290` `SEL2-P2`) & Negative Controls

Following the baseline invariance pattern in [`laws.rs:28-32,85-118`](../../../src-tauri/src/core/experiment/laws.rs#L28-L32) and [`ADR-0016` §5](../../decisions/ADR-0016-changing-world-surface.md#L194-L202):
- Extend `WorldLawSet` with `#[serde(default, skip_serializing_if = "Option::is_none")] pub respiration: Option<RespirationLaw>`.
- In `WorldLawSet::canonicalize()`, `respiration: None` **emits zero bytes** (unlike `exotic_energy: None` which writes `0x00` tag at `laws.rs:90`), guaranteeing that `WorldLawSet::baseline().fingerprint()`, [`2026-11-24-sel2-p2-preregistration.md`](../planning/2026-11-24-sel2-p2-preregistration.md) (`#290`, `C16 SEL2P2-FREEZE`), `E3b` (`#252`/`#260`), and every `prereg_*` manifest & `world_checksum` remain **bit-identical** (`ER01`, `ER02`, `ER07`, `ER08`).

| Gate ID | Positive Assertion | Mandatory Negative Control (Must Fail Red) |
|---|---|---|
| **`RESP-S01`** | `respiration: None` produces bit-identical `WorldLawSet::baseline().fingerprint()`, `SEL2-P2` (`#290`) & `prereg_*` manifests, `0` allocs, and unchanged `LAND1` shoreline reversion. | Emit any tag byte in `canonicalize()` when `respiration` is `None`, or run `respiration_system` with law off -> baseline fingerprint/checksum diff fails red. |
| **`RESP-S02`** | `DevelopedPhenotype` (`walk..dig_burrow`, `lung..hypoxia_tolerance`) develops once at genesis/birth (`D01`), persists verbatim across save/`Restore`/`Migration` (`D02`), and defaults legacy saves safely (`D09`). | Re-run `develop_at_birth` during `spawn_serialized_agent` / migration, or infer `swim` from water biome cell (D04 violation) -> `D01`/`D04` invariant tests fail red. |
| **`RESP-S03`** | All swim, dive thermal, burrow hardness, and flight metabolic penalties fold strictly into `total_cost` (`world_systems.rs:157`); suffocation death via `ReclaimAndDespawnAgentCommand` keeps `\|closed_total_eu - baseline\| <= 1e-3`. | Deduct swim/thermal penalty directly from `homeo.energy` outside `total_cost` -> closed-EU conservation audit trips `> 1e-3` red. |
| **`RESP-S04`** | Smaller-mass terrestrial agents (`M_1 < M_2`) succumb faster than larger agents (`M^(0.25)`) in both water submersion and `CO2` gas exposure; semi-aquatic divers outlast obligate terrestrial agents. | Invert mass scaling exponent (`M^(-0.25)`) or zero `TotalCost` activity feedback -> allometry & dive-survival tests fail red. |
| **`RESP-S05`** | `GasConcentrationField` flows downhill into topographic basins on calm days, dilutes with wind, stays non-negative/bounded `[0.0004, 1.0]`, and runs at `TrackingAllocator = 0` heap allocs/tick. | Allocate temporary `Vec` inside `step_gas_field` or ignore elevation gradient (diffusing uniformly uphill into peaks) -> `TrackingAllocator` & `mazuku` basin-pooling tests fail red. |
| **`RESP-S06`** | Brain opt-in inputs/gates activate only when `respiration: Some(...)`, leave legacy `15×64×4`/`8` runs bit-identical, and verify `VitalStatistics` / `DeathCauseTag` match causal exposure (`Drowning` vs `Gas` vs `Smoke`). | Route gas deaths under `"Starvation"`/`"Senescence"` without `Suffocation*` accounting, or expand `ArchSpec` unconditionally -> telemetry/causal tests fail red. |

---

## 11. Interaction with `81.92 km` World (`QD-WV2-D2`) & Cross-Track Dependency Order

### 11.1 Coupling to the `81.92 km` Environments (`QD-WV2-D2`, `#294`/`#295`)

Under `QD-WV2-D2` (`WB-D1..WB-D8`):
- **Terrain vs Sim Grid (`WB-D7`):** `sim_256.anmw3` remains `256²` (`320 m/cell` at `81.92 km`, vs `160 m/cell` in `C01`). Rivers (`E3` alluvial meanders/oxbows), estuaries/mangroves (`E4`), rift/piedmont/caldera lakes (`E5`), offshore islands (`E9`), and wide `55–65%` plain-backed coasts (`§4b`) span multi-cell aquatic corridors and barrier straits on `256²`, making swimming (`swim_capacity`), drowning boundaries, and water crossing central to colonization & gene flow across the `82 km` landscape.
- **Volcanic & geothermal sites:** Calderas (`E8`, `tectonic.rs:417-420`) and fault rifts (`E5`, `tectonic.rs:314-332`) create natural enclosed topographic bowls where `W3` vents pool dense volcanic `CO2` (`mazuku` pockets), rewarding evolved gas-sensing avoidance.
- **Sub-grid rivers (`< 320 m`):** Shallow upland streams too narrow for `256²` `River` biome cells remain represented via `DynamicFields::surface_water` / flood stages (`W1`/`W3`), which feed water-depth checks cleanly alongside `TerrainMap.biomes`.

### 11.2 Dependency Order Across Active Tracks

```text
1. ADR-0001 V0 (`develop_at_birth` + `DevelopedPhenotype`) & QD-WCHG follow-up (`ADR-0016` -> accepted)
   ├──► 2. WV2-BIG-1..5 (81.92 km world terrain, plain coast & rivers/deltas/calderas)
   ├──► 3. INVERT-D (lead, shared soil fauna & aquatic larvae fields)
   └──► 4. RESP-1 (Locomotion Media Framework + Trait Continuum + Opt-In WorldLawSet)
         ├──► 5a. RESP-2 (Unified Respiration + Drowning + Mortality/Closed-EU)
         │     ├──► 6a. RESP-3 (Swimming/Thermal Costs + Aquatic ResourceField/INVERT-D Foraging)
         │     │     └──► 7a. AQUA-D (Full Aquatic Fauna, Gills, Predatory Fish replacing decorative WorldFish)
         │     └──► 6b. RESP-5 (Opt-In Brain Sensory/Action Escape + CausalTelemetry/Tests)
         ├──► 5b. ADR-0016 W1–W3 (SurfaceDeltaOverlay + Volcanic Vent & Wildfire Triggers)
         │     └──► 6c. RESP-4 (256² GasConcentrationField CO2 Pooling + Smoke Coupling -> feeds RESP-2)
         ├──► 7b. AERO-D (lead, Flight/Glide on Locomotion Media Framework + RESP-D Smoke/Gas)
         └──► 7c. FOSS-D (lead, Underground/Substrate Burrowing + RESP-D Burrow Hypoxia/CO2)
```

---

## 12. Implementation Cards (`RESP-1..RESP-5`) & Open Owner Decisions (`RD-D1..RD-D9`)

### 12.1 Sequenced Implementation Cards

| Card | Scope & Key Surfaces | Dependencies | Est. Size | Mandatory Verification & Negative Control |
|---|---|---|---|---|
| **`RESP-1`** Framework & Phenotype Continuum | Add `LocomotionCapacities` (`walk..dig_burrow`), `RespirationApparatus` (`lung..hypoxia_tolerance`), `LocomotionState`, derived D04 `HabitatPreference`, `RespirationState`, and `WorldLawSet::respiration: Option<RespirationLaw>` (`0` bytes when `None`). | `V0` (`ADR-0001` phenotype split) | `~2 d` | `RESP-S01` (`SEL2-P2` `#290` & baseline fingerprint unchanged), `RESP-S02` (save/restore/migration `D01`/`D02`/`D09`), negctrl tag byte & D04 test. |
| **`RESP-2`** Unified Respiration & Mortality | Implement `respiration_system` (oxygen reserve allometry `M^0.25`, `CO2`/smoke dose accumulation, hypoxia grace), extend `DeathCause` / `DeathCauseTag` / `VitalStatistics`, route death strictly via `ReclaimAndDespawnAgentCommand`. | `RESP-1` | `~1.5–2 d` | `RESP-S03` (closed EU `<= 1e-3`), `RESP-S04` (mass-size & dive scaling), negctrl wrong mass exponent & unlogged death cause. |
| **`RESP-3`** Swimming Access, Drag/Thermal Cost & Stage-1 Food | Relax `Ocean`/`DeepOcean` block cleanly under `RespirationLaw`, add swim drag & aquatic conductive heat loss (`T_water` from `climate.rs` & `aquatic_insulation`) into `total_cost`, unlock Stage-1 `ResourceField` + `INVERT-D` larvae aquatic foraging. | `RESP-1`, `RESP-2`, (`INVERT-D` larvae when ready) | `~2 d` | `TrackingAllocator = 0`, closed EU `<= 1e-3`, cold alpine vs warm delta thermal cost test; negctrl energy drain outside `total_cost` fails red. |
| **`RESP-4`** `256²` Dense-Gas Field & `W3` Coupling | Implement zero-alloc `512 KiB` `GasConcentrationField` (`co2_frac` downhill gravity advection, wind dilution, `mazuku` hollows) coupled to `ADR-0016` `W3` eruptions & `SurfaceDeltaOverlay` fire smoke + `FOSS-D` burrow air hook. | `RESP-2`, `ADR-0016` `W2`/`W3` | `~2 d` | `RESP-S05` (hill-vs-basin pooling, calm-vs-wind decay, `0` tick allocs), negctrl allocating `Vec` or uniform flat diffusion fails red. |
| **`RESP-5`** Opt-In Brain Escape, Sensors & Chronicle | Add versioned opt-in sensory inputs (water depth, `O2`, gas hazard/gradient) + `ActionGates` (`ArchSpec` untouched when law off), `learn_step` hypoxia cue, `CausalLedger` attribution, and Playwright/desktop visual smoke. | `RESP-2`–`RESP-4` | `~1.5–2 d` | `RESP-S06` (`15×64×4`/`8` parity when law off; emergent shore-exit / uphill escape when law on), negctrl mutating default `ArchSpec` fails red. |

### 12.2 Open Owner Decisions (`RD-D1..RD-D9`)

| ID | Decision Question | Options | Recommendation | Fallback if Declined |
|---|---|---|---|---|
| **`RD-D1`** | Model land-to-water access & shared media as continuous heritable capacities vs discrete classes? | **(a)** Continuous capacities `{walk, swim, dive, fly_glide, dig_burrow}` + `{lung, gill, skin, hypoxia_tolerance}` in `[0, 1]` with D04 derived summary<br>**(b)** Fixed enum (`Terrestrial \| Amphibious \| Aquatic \| Aerial \| Fossorial`) | **Option (a)** — naturally unifies polar-bear/jaguar surface swimmers, caimans, burrowers, and flyers without rigid buckets or cell inference. | Option (b) if `DevelopedPhenotype` simplicity is preferred early (harder to express gradients later). |
| **`RD-D2`** | Physiological asphyxiation timescale vs `72×` calendar compression (`1 day = 20 min`)? | **(a)** Watchable acute timescale (~`6–8×` vs real physiology: `15–45 s` base drowning, `8–15 s` high-`CO2` collapse, disclosed in UI/metadata)<br>**(b)** Strict `72×` calendar compression (`~2.5 s` drowning) | **Option (a)** — prevents unfair `< 1–2.5 s` instant kills and aligns with real-time walking/combat cadence (`locomotion.rs`). | Tune `tau_phys` via exposed field on `RespirationLaw` per scenario preset. |
| **`RD-D3`** | Stage-1 aquatic food source before `AQUA-D` fish agents land? | **(a)** Allow semi-aquatic agents to graze conserved `ResourceField` (`ecology.rs:69-72`) + `INVERT-D` aquatic larvae in `RESP-3`, then upgrade to predatory hunting once `AQUA-D` fish agents land<br>**(b)** Block all aquatic foraging until `AQUA-D` completes | **Option (a)** — respects dependency order (`RESP-3` -> `AQUA-D`) with honest EU accounting and immediate `River`/`Ocean` ecological utility. | Option (b) defers semi-aquatic predatory payoff until `AQUA-D` ships. |
| **`RD-D4`** | Gas overlay species: `CO2`-only (`512 KiB`) vs `CO2` + `SO2`/`H2S` (`768 KiB`)? | **(a)** Start with `CO2` (`co2_frac` + scratch = `512 KiB`) + wildfire smoke coupling; add `irritant_gas` (`+256 KiB`) only if geothermal vents differentiate `SO2`/`H2S`<br>**(b)** Always allocate both fields (`768 KiB`) | **Option (a)** — `CO2` covers `"mazuku"` pooling cleanly inside minimal fixed memory, reserving an optional slot for `W3` `SO2`/`H2S`. | Include `irritant_gas` under `RespirationLaw::enable_irritants` flag (still `0` bytes when off). |
| **`RD-D5`** | Opt-in neural sensory & action gates vs pure reflex steering for water/gas escape? | **(a)** Data-driven opt-in `ArchSpec` sensors & `ActionGates` (`ADR-0003`) gated strictly on `respiration: Some(...)`, preserving legacy `15×64×4`/`8` verbatim when off<br>**(b)** Hardcoded reflex flee steering without neural inputs | **Option (a)** — lets brains evolve genuine risk/reward foraging choices (no scripted "bear dives" behaviour). | Deterministic emergency up-slope steering fallback only for legacy shared-model tests. |
| **`RD-D6`** | Mortality cause reporting granularity (`VitalStatistics`, `DeathCauseTag`)? | **(a)** Explicit sub-causes `{SuffocationDrowning, SuffocationGas, SuffocationSmoke, SuffocationAerial}`<br>**(b)** Single combined `Suffocation` enum | **Option (a)** — answers `"how many drowned vs gassed"` directly in causal reports while defaulting legacy counts to unchanged. | Option (b) with secondary breakdown strictly in `CausalLedger` notes. |
| **`RD-D7`** | Amphibian larval stage & metamorphosis scope? | **(a)** Defer life-stage metamorphosis (`Larva -> Adult`) to a dedicated `ADR-0001` amendment after `V0`; keep adult cutaneous respiration (`skin_capacity`) in the `RESP-D` capacity model<br>**(b)** Design full tadpole-to-adult metamorphosis inside `RESP-D` | **Option (a)** — avoids violating `ADR-0001` D01 single-development contract (`§4.5`) before `V0` lands. | — *(requires `ADR-0001` amendment first)*. |
| **`RD-D8`** | World-law activation default across desktop & experiments? | **(a)** Opt-in (`WorldLawSet.respiration = None` default; enabled cleanly via scenario/live world presets once `V0`/`RESP-1..5` pass); zero impact on `SEL2-P2` `#290` / `prereg_*`<br>**(b)** Enabled by default on `desktop` immediately | **Option (a)** — required by [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) and `#290` preregistration freeze. | Decide default desktop toggle in a later release gate once all 6 `RESP-S01..S06` pass. |
| **`RD-D9`** | Subterranean burrow respiration & substrate hardness coupling for `FOSS-D`? | **(a)** Model `Underground` gas stress through `RESP-D`'s unified model using `hypoxia_tolerance` (parameterized `O2`/`CO2` bounds `"chưa xác minh"`) + biome/`root_cover` substrate hardness in `total_cost`<br>**(b)** Give `FOSS-D` a separate tunnel-stamina timer outside respiration | **Option (a)** — keeps one unified respiration & EU model across land, water, fire, volcanic gas, and burrows (`MSG_20261125_191000`). | Add tunnel oxygen vent density parameter in `FOSS-D` reusing `RespirationState` directly. |
