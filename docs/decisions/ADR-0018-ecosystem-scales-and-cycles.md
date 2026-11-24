---
title: ADR-0018 — Ecosystem Open Energy Flow, Multi-Scale Tiers, and Density-Dependent Cycles
status: proposed
owner: simulation-architecture
last_reviewed: 2026-11-24
decision_date: pending
supersedes: none
superseded_by: none
---

# ADR-0018 — Ecosystem Open Energy Flow, Multi-Scale Tiers, and Density-Dependent Cycles

> **Status: `proposed` — `QD-ECOSCALE` & `QD-BIO-D1` design record.**
> Recorded direction (`QD-ECOSCALE`, `MSG_20261124_154500`, `PR #255` track): density-dependent cycles
> (not hard population caps), three organism representation tiers (individuals, swarms /
> super-individuals, population fields), Kleiber allometric pace (`QD-PLAYRULES` `1–2 h` lifespan
> anchored to the typical inhabitable animal), and individuals-only inhabiting in Milestone 1.
> Open numeric/threshold gates (**`D-ECO-1`–`D-ECO-6`**) and full acceptance remain with the owner.
> Do **not** edit [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md)
> (`:121-124`) or begin `E1`–`E7` implementation until the owner accepts this ADR.
> Trunk code anchors below are verified on `origin/feat/ecological-selection` at
> `facc800c50a15cdca960c8c006cc5f861e4c703a`.

---

## 1. Context & Verified Trunk Bottlenecks (`facc800c`)

Today's ecosystem sustains populations artificially by recycling animal respiration straight back into
plant detritus and enforcing hard population caps:

1. **Thermodynamically closed respiration loop (`world_systems.rs:222-224`, `ecology.rs:153-158, 429-432`):**
   - Inside `metabolic_decay_system` (`src-tauri/src/core/world_systems.rs:157, 174-177, 222-224`),
     every joule/EU burned by `total_cost` flows into `respired` and adds directly into
     `pool.detritus += respired` (`:222-224`).
   - `ResourceField::step_regrowth_gated_strided` (`src-tauri/src/core/ecology.rs:429-432`, invoked in
     `resource_field_regrowth_system` at `src-tauri/src/core/environmental_systems.rs:72-85`) draws
     plant biomass exclusively from `pool.detritus`. Without solar input or respiratory heat dissipation,
     energy loops indefinitely through ecosystems with no thermodynamic dissipation sink.
   - `LINDEMAN_EFFICIENCY = 0.40` (`src-tauri/src/core/ecology.rs:155`, mirrored in
     `src-tauri/crates/anima-domain/src/laws.rs:46`) currently documents *"~10% canonical"*, even
     though `0.40` is physically the **per-strike predator assimilation efficiency**, while realized
     trophic transfer efficiency (TTE) across the whole food web emerges only once primary
     production enters from sunlight and consumer respiration exits as heat (`RESEARCH_REVIEW` §2 Finding 1).
2. **Hard population caps instead of dynamic cycles (`agent_systems.rs:694-744, 989-998`):**
   - `autonomous_reproduction_system` (`src-tauri/src/core/agent_systems.rs:704`) suppresses animal
     reproduction via hardcoded `prey_cap` (`DEFAULT_PREY_CAP = 80`, `live.prey_cap` at `:741-743`)
     and the predator rule `pred_count >= 8 || prey_count < pred_count * 5` (`:694-699, :990-993`).
   - Hard ceilings mask carrying-capacity dynamics, nutrient/detritus feedback, predation loops, and
     parasite/pathogen density regulation, whereas [`ADR-0007`](ADR-0007-live-ecology-sustainability.md)
     established only minimal opt-in foraging survival (`live.drive_floor`, `live.thirst_steering`).
3. **One-size-fits-all individual representation vs 60 Hz headroom (`docs/paper/data/scaling.md:39, 72, 86`):**
   - Measured headless tick schedule cost (`live_experiment.rs` / `build_tick_schedule`) is already
     **`5.643 ms` p50 at `N = 1,000`** agents (`33.9%` of a `16.67 ms` 60 Hz frame on `i5-14600KF`,
     growing ~`N^1.86` toward `77.904 ms` at `N = 4,096`). Representing thousands of grass clumps,
     detritivores, soil bacteria, insect swarms, or schooling small fauna as individual rigid-body/neural
     ECS entities exceeds real-time frame budgets.
4. **Contract & ledger baseline (`energy.rs:19-150, 375-377`, `EVOLUTION_EXPERIMENT_CONTRACT.md:121-124`):**
   - `EcosystemBiomass` (`src-tauri/crates/anima-domain/src/energy.rs:19-31`) tracks four closed
     compartments (`detritus`, `plants`, `animals`, `dormant`), checked by `closed_total_eu` (`:375-377`)
     inside `ecosystem_census_system` (`src-tauri/src/core/environmental_systems.rs:178-229`) against
     `RESIDUAL_ABS_TOLERANCE_EU = 1e-3` (`energy.rs:123`).
   - `WorldLawSet` (`src-tauri/src/core/experiment/laws.rs:28-32, 79-90`) currently pairs
     `BaselineEnergyLaw` (`eu_unit = "EU"`) with `exotic_energy: Option<ExoticEnergyLaw>`.
   - `EVOLUTION_EXPERIMENT_CONTRACT.md:121-124` states *"Closed EU vẫn là biomass-equivalent
     `plants + animals + detritus`"* (explicitly noting in `:123-124` that materializing biomass
     requires its own ADR and ledger extension).

---

## 2. Central Decision (`QD-BIO-D1`): Opt-In Open Energy Flow

Per owner decision **`QD-BIO-D1`** (`MSG_20261124_133000`, `PR #255`), the engine retains exact closed-EU
conservation as its legacy default and adds an **opt-in open-flow world law** (`OpenEnergyFlowLaw`) with
explicit **declared sources and declared sinks**:

### 2.1. World-Law Declaration & Bit-Identical Default (`src-tauri/src/core/experiment/laws.rs:28-90`)

- Extend `WorldLawSet` (`laws.rs:28-32`) with:
  `#[serde(default, skip_serializing_if = "Option::is_none")] pub open_energy_flow: Option<OpenEnergyFlowLaw>`
  and include it in `WorldLawSet::canonicalize` / `fingerprint()` (`:79-90`) so any experiment or
  island toggling open energy flow receives a distinct `WorldLawSet` fingerprint.
- **Bit-identical fallback rule:** When `open_energy_flow` is `None` (default), `pool.detritus += respired`
  at `world_systems.rs:222-224`, detritus-gated regrowth at `ecology.rs:429-494` / `environmental_systems.rs:72-85`,
  and closed `closed_total_eu` accounting (`energy.rs:375-377`) execute **unchanged** with `0` extra
  RNG draws, keeping trunk baselines (`E2`, `E3`, `E3b`, `SEL2-P2`) bit-for-bit identical.

### 2.2. Solar Input via Light-Use Efficiency (LUE)

- Under an active `OpenEnergyFlowLaw`, autotrophs (evolving plants under planned `ADR-0015` and/or
  `ResourceField` grass under [`2026-11-24-v5-fields-plants-feedback-spec.md`](../ai/planning/2026-11-24-v5-fields-plants-feedback-spec.md))
  fix biomass energy from **declared incident solar radiation**:
  $$\Delta E_{\text{NPP}}(i) = I_{\text{solar}}(i, t) \cdot \text{LUE}_i \cdot f_w(i) \cdot f_n(i) \cdot f_t(i) \cdot f_{\text{light}}(i) \cdot \Delta t$$
  where $I_{\text{solar}}(i, t)$ follows diurnal/seasonal irradiance phase (`SeasonClock` or `QD-PLAYRULES`
  real-time day/year clock), $\text{LUE}_i$ is bounded photosynthetic light-use efficiency, and
  $(f_w, f_n, f_t, f_{\text{light}})$ enforce soil moisture, soil nitrogen stoichiometry, temperature,
  and canopy shading (`V5`).
- Solar primary production no longer drains `pool.detritus` as its energy source: instead, soil
  organic matter mineralization supplies **mineral nutrients (N)** (`DynamicFields`, `V5` / `V4`), while
  energy itself enters `Plants` directly through a declared ledger source transaction
  (`EnergyEvent::SolarPrimaryProduction`), measured from actual stored `f32 -> f64` cell/plant deltas so
  rounding never creates phantom drift.

### 2.3. Respired Heat Loss vs Detritus Recycling (`world_systems.rs:157, 222-224`)

- **Hard `CLAUDE.md` rule preserved:** Every continuous animal/plant metabolic cost — basal MTE
  maintenance, activity, cognition, lifecycle, growth, and signalling — continues to sum strictly
  **inside `total_cost` in `metabolic_decay_system`** (`src-tauri/src/core/world_systems.rs:157`). No
  energy dissipation calculation or "heat sink" may subtract outside `total_cost` (`RESEARCH_REVIEW` §3
  exclusion enforced).
- When `open_energy_flow` is active, instead of routing 100% of `respired` back into `pool.detritus`
  (`world_systems.rs:222-224`), `respired` splits cleanly by the law's declared thermal dissipation
  share (`heat_loss_fraction in (0, 1]`, default `1.0` per `D-ECO-1`):
  - `(heat_loss_fraction) * respired` exits the biosphere to the declared **`HeatLoss`** boundary sink
    (`EnergyEvent::RespiredHeatLoss`), matching real thermodynamics where cellular respiration leaves
    living biomass as heat;
  - `(1.0 - heat_loss_fraction) * respired` (zero when `1.0`), **plus** all unassimilated capture
    waste (`(1 - LINDEMAN_EFFICIENCY) * captured`), reproductive overhead
    (`settle_reserve_outflow_to_detritus` at `src-tauri/src/core/energy_ledger.rs:35-54`), senescence/starvation
    carcasses ([`2026-11-24-v4-carcasses-decomposition-spec.md`](../ai/planning/2026-11-24-v4-carcasses-decomposition-spec.md)),
    and plant litter/root exudates, continue entering `Detritus` / `Carcasses` to feed decomposers and
    regenerate mineral nutrients.
- **Clarification of `LINDEMAN_EFFICIENCY = 0.40` (`ecology.rs:153-155`):** Keeps `0.40` untouched as
  per-strike prey assimilation efficiency (preserving frozen E3/E3b manifests); realized whole-ecosystem
  trophic transfer efficiency (~10% benchmark target, never hardcoded) emerges naturally once metabolic
  respiration dissipates as heat rather than feeding herbivores recursively via `detritus`.

### 2.4. `EnergyLedger` Extension (`anima-domain/src/energy.rs:19-150, 375-377`)

- Keep existing `f64` store arithmetic (`RESEARCH_REVIEW` §3 rejects a wholesale `i64` ledger rewrite;
  an optional parallel `i64` shadow diagnostic audit remains permissible if needed in tests).
- Extend `EnergyLedger` (`anima-domain/src/energy.rs:131-150`) and `EnergyEvent` (`:64-83`) with:
  - **Declared source tracking:** `cum_solar_in: f64` incremented strictly via `EnergyLedger::record_declared_source(..., EnergyEvent::SolarPrimaryProduction, actual_f64_gain)` in the same pass where plant stores are credited;
  - **Declared sink tracking:** `cum_heat_out: f64` incremented strictly via `EnergyLedger::record_declared_sink(..., EnergyEvent::RespiredHeatLoss, actual_f64_loss)` in `metabolic_decay_system` / plant maintenance respiration;
  - **Unified conservation identity (`open_energy_flow = Some(...)`):**
    $$R_{\text{open}}(t) = \Big(E_{\text{plants}} + E_{\text{animals}} + E_{\text{detritus}} + E_{\text{dormant}} + E_{\text{carcasses}} + E_{\text{swarms}} + E_{\text{fields}}\Big)(t) - \Big(E_0 + \text{cum\_solar\_in}(t) - \text{cum\_heat\_out}(t)\Big)$$
    Every step checks $|R_{\text{open}}(t)| \le \text{RESIDUAL\_ABS\_TOLERANCE\_EU}$ (`1e-3` EU,
    `anima-domain/src/energy.rs:123`; tight baseline equivalence `<= 1e-6` verified in tests). Under
    `open_energy_flow = None`, `cum_solar_in == 0.0` and `cum_heat_out == 0.0`, reducing identically to
    `closed_total_eu` (`energy.rs:375-377`).

### 2.5. Draft Contract Amendment Text (`EVOLUTION_EXPERIMENT_CONTRACT.md:121-124`)

> **Pending Acceptance Notice:** The amendment below is a **draft proposal only**.
> `docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md` must **not** be edited under card `D19`. Only
> after the owner accepts `ADR-0018` may card `E2` apply this amendment to lines `~121-124` of
> `docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md`:
>
> ```markdown
> Không gọi `MU` là `EU`. Khi luật thế giới dùng chế độ đóng mặc định (`open_energy_flow = None`),
> hệ tuân thủ **closed EU**: `plants + animals + detritus + dormant` là bất biến (cộng `carcasses`
> khi bật `V4`). Exotic energy có thể thay đổi **tốc độ chuyển EU giữa các pool**, nhưng không tạo
> biomass miễn phí; nếu một luật tương lai cho phép materialization, luật đó cần ADR riêng và một
> mass ledger mới. Khi thế giới khai báo luật dòng năng lượng mở đã được chấp nhận
> (`OpenEnergyFlowLaw`), quy tắc chuyển thành **hạch toán kín qua các nguồn và bồn đã khai báo**
> (*closed accounting over declared sources and sinks*): tổng mọi khoang sinh khối hiện hữu trừ
> `(baseline + cum_solar_in − cum_heat_out)` phải nằm trong ngưỡng sai số của `EnergyLedger`, và mọi
> chi phí tiêu hao sinh học vẫn đi qua `total_cost`.
> ```

---

## 3. Decided Architectural Pillars (`QD-ECOSCALE`, Decided 2026-11-24)

Per owner agreement **`QD-ECOSCALE`** (`MSG_20261124_154500`, confirming the lead proposal in
`MSG_20261124_153000`), the four structural choices below are **decided** for `ADR-0018` (only numeric
constants, exponents, tier boundaries, and post-Milestone-1 promotion rules remain open D-slots):

### 3.1. Density-Dependent Cycles Instead of Hard Population Caps

- Ecosystems sustain themselves through closed nutrient/matter cycles (`Detritus` -> soil `N`/`H2O` ->
  `Plants` -> `Herbivores` -> `Predators` -> `Carcasses` -> decomposers -> soil nutrients, `V4` + `V5`)
  paired with open solar energy flow (§2) and real density-dependent feedbacks: resource depletion,
  herbivore/predator foraging limits, intraspecific crowding/competition for space & water, and opt-in
  SEIR-lite infection / parasite transmission (`RESEARCH_REVIEW` Tier B `B9`, virulence billed strictly
  inside `total_cost`).
- **Legacy cap retirement (`agent_systems.rs:694-744, 989-998`):** `live.prey_cap` (`DEFAULT_PREY_CAP = 80`)
  and the predator rule `pred_count >= 8 || prey_count < pred_count * 5` remain available behind a
  legacy cap flag (`PreyCapPolicy`) for backward-compatible experiments (`E2`, `E3`, `E3b`), and become
  **disabled by default on shared islands** once density-dependent cycles pass multi-seed `V10` soak
  stability ([`2026-11-24-v10-soak-leak-spec.md`](../ai/planning/2026-11-24-v10-soak-leak-spec.md)) with
  zero unexplained EU drift (`|R_open| <= 1e-3`). Population booms, crashes, and extinctions are recorded
  honestly in the chronicle rather than suppressed by artificial ceilings.

### 3.2. Three Organism Representation Tiers (Super-Individual Individual-Based Ecology)

| Tier | Biological Scope | State & Selection Mechanics | Simulation / Compute Budget |
|---|---|---|---|
| **Tier 1: Individuals** | Medium/large animals (herbivores, predators, birds, fish once realms unlock), trees, shrubs (`Tree` / `ADR-0015` flora) | Full body morphology (`MorphologyGenotype`), evolved brains (`AgentBrain`), individual lifecycle (`ADR-0013`), two-parent sexual genetics, spatial physics (`RigidBody` / `SpatialCollider`). **Inhabitable in Milestone 1.** | Soft operational target **`<= ~1,000` active individuals** (staying near the `5.643 ms p50` headless budget measured at `N = 1,000` in `docs/paper/data/scaling.md:39, 72`), using zero-alloc ticks and `DormantCohorts` LOD. |
| **Tier 2: Swarms / Super-Individuals** | Flocks, schools, colonies, adult biting insects (e.g., mosquito swarms, social insect colonies, baitfish schools when water realm lands) | One pre-allocated super-individual record stores `count: u32`, shared spatial state, pooled EU reserve, and an **explicit heritable trait summary distribution** (mean/variance or quantized bins over life-history/foraging traits). Moves, disperses, feeds, and absorbs predation as a coherent unit. Zero ECS segment tree or brain per insect. | Bounded `Vec::with_capacity` resource pool (`SwarmPool`, modeled on `CarcassPool` in `V4`), **`0` heap allocations per tick**, O(`#swarms`). |
| **Tier 3: Population Fields** | Grasses (`ResourceField`), soil fauna & bacterial/fungal decomposers, phytoplankton/zooplankton, aquatic insect larvae | Per-cell grid buffers (`n = width * height`) carrying biomass density `B[i]` (`f64`/`f32` measured delta) **plus local heritable trait distributions** (matching planned `ADR-0015` evolving grass: e.g. palatability, defence, growth strategy, temperature tolerance). Evolves via selection + spatial seed/larval dispersal between neighboring cells (`Morphology`-free). | Fixed pre-allocated `Vec` buffers (`DynamicFields` pattern), strided updates where valid, **`0` heap allocations per tick**. |

- **Illustrative multi-tier life cycle & strict `QD-REALMS` gating (`docs/how-to/AGENT_WORKFLOW.md:320-323`):**
  A small vector organism such as a mosquito spans tiers naturally: aquatic **larvae** develop as a
  cell-level **Population Field** in still-water cells (`ResourceField`/`DynamicFields` grid); upon
  metamorphosis they emerge into a bounded terrestrial/airspace **Swarm / Super-Individual** that seeks
  large warm-blooded **Individuals**, takes conserved blood-meal EU (adding a real energetic drain +
  optional vector transmission charge inside the host's `total_cost` while crediting swarm EU 1:1 via
  `EnergyLedger`), and is predated by aquatic **fish** or aerial **birds**. Consistent with **`QD-REALMS`**
  (`AGENT_WORKFLOW.md:320-323`), aquatic, aerial (sky), and subterranean organisms unlock **strictly one
  realm at a time** after `SEL2-P` (water first, then sky, then underground); until each realm's dedicated
  ADR lands, visual fish/birds/wildlife remain explicitly labeled decorative/offline rather than active
  trophic agents.

### 3.3. Allometric Pace of Life (`anima-domain/src/laws.rs:40, 70-76`)

- Whole-organism metabolic rate obeys Kleiber scaling `B ~ M^0.75` (`KLEIBER_EXP = 0.75`,
  `mte_basal_metabolism` at `src-tauri/crates/anima-domain/src/laws.rs:40, 70-76`, coupled with
  Boltzmann thermal scaling `E_ANIMAL_EV = 0.65`, `E_PRODUCER_EV = 0.30`).
- Biological timescales (maturation age, gestation/reproduction cooldown, lifespan ceiling, generation
  time) scale monotonically with body mass (`~M^beta`, `beta ~ 0.25` default in `D-ECO-2`):
  - The **`1–2 hour` real-time lifespan baseline** decided in `QD-PLAYRULES` anchors the **typical
    inhabitable animal tier** (`M_ref`, `D-ECO-2`);
  - Small organisms (micro-herbivores, insects, plankton/decomposer fields) live **minutes**, turn over
    rapidly, and evolve fast on observable real-time timescales;
  - Large long-lived flora (`Tree` individuals under planned `ADR-0015`) span **days or geological eras**.

### 3.4. Inhabiting Policy Across Tiers & Pre-Card Benchmark Gate

- **Inhabiting (`QD-ECOSCALE`, Milestone 1 decided):** Guests may inhabit **Tier 1 Individuals only**
  (individual animals or individual trees/shrubs). Tier 2 Swarms and Tier 3 Population Fields can be
  watched, inspected in overlays/chronicle, and queried via world records, **never inhabited directly**
  in Milestone 1. Any post-Milestone-1 temporary promotion of one swarm/field member into an inhabitable
  individual stays gated under **`D-ECO-5`** and must conserve exact headcount (`N_swarm -> N_swarm - 1`
  + `1` individual) and exact EU (`0` free energy/mass on promote or demote).
- **Mandatory Benchmark Gate Before Entity Expansion (`BENCH-ENTITIES`, Card `E1`):** Before adding
  swarm pools or hundreds of individual flora/fauna entities onto `build_tick_schedule`, extend
  `src-tauri/benches/live_schedule_scaling.rs` with 500 and 2,000 sessile entities plus a swarm pool
  (`RESEARCH_REVIEW` Tier A `A2`) to measure real headless tick margins against `16.67 ms` (`60 Hz`).

---

## 4. Open Owner Decision Slots (`D-ECO-1`–`D-ECO-6`)

*(Only parameters, thresholds, and post-Milestone-1 deferral rules remain open; architectural pillars
above are settled under `QD-ECOSCALE` & `QD-BIO-D1`.)*

| Slot | Question | Options | Recommendation |
|---|---|---|---|
| **D-ECO-1** | Open-flow solar irradiance, LUE & heat-loss fraction | **A:** Unit-normalized irradiance `I_solar` scaled by latitude/season/diurnal curve + full `heat_loss_fraction = 1.0` (`respired` exits 100% as `HeatLoss`; mortality/egestion/overhead still cycle via `Detritus`)<br>**B:** Configurable partial fraction `heat_loss_fraction in (0, 1]` | **A** cleanly separates thermodynamic energy dissipation from mineral detritus recycling (`B` can expose `heat_loss_fraction` in `OpenEnergyFlowLaw` defaulting to `1.0`) |
| **D-ECO-2** | Allometric exponents & reference inhabitable animal mass | **A:** Kleiber `B ~ M^0.75` (`KLEIBER_EXP`), biological lifespan/cooldown `~ M^0.25`, with `M_ref` (e.g. legacy 3-seg herbivore mass `~3.3` or calibrated `SpeciesProfile` rabbit baseline) mapped to `1.0–2.0 h` real time under the island ecology clock<br>**B:** Per-species lookup constants without shared `M^beta` scaling law | **A** — derives consistent multi-taxa pace from one law while clamping extrema safely for numerical stability |
| **D-ECO-3** | Thresholds partitioning Tier 1 (`Individual`), Tier 2 (`Swarm`), Tier 3 (`Field`) | **A:** Body mass + characteristic count bands per taxonomic group in `SpeciesProfile` / realm manifest, capped (`MAX_ACTIVE_INDIVIDUALS`, `MAX_SWARMS`) against `BENCH-ENTITIES` (`E1`)<br>**B:** Author-chosen fixed taxonomic assignment only (each taxon statically tagged `Individual`, `Swarm`, or `Field`) | **B** for Milestone 1 taxa (clear, deterministic, no runtime tier-hopping churn), with `A` budgets enforced by `E1` |
| **D-ECO-4** | Legacy cap retirement criteria (`live.prey_cap` / predator `5:1` ratio) | **A:** Disable hard caps by default in shared-island profiles only after `E4` + `V10` multi-seed soak shows bounded coexistence without runaway extinction or `\|R_open\| > 1e-3`<br>**B:** Keep a gentle safety emergency ceiling disabled in experiments | **A** — empirical `V10` soak proof before retiring `PreyCapPolicy` on islands; legacy key untouched for science experiments |
| **D-ECO-5** | Swarm/Field promotion to temporary `Individual` for `Inhabit` | **A:** Deferred post-Milestone 1 (`Inhabit` remains strictly Tier 1 `Individuals`); when added later, require atomic EU & headcount conservation on promote/demote and re-demotion after guest detach<br>**B:** Include single-specimen promotion in Milestone 1 | **A** — keeps Milestone 1 scope clean and prevents `Dormant` / `SwarmPool` / `LiveLineage` conservation corner cases |
| **D-ECO-6** | Manifest opt-in schema & E3b freeze pin coordination (`QD-BIO-D2`) | **A:** `WorldLawSet::open_energy_flow` + opt-in `live.*` flags (`live.allometry`, `live.density_cycles`, `live.population_fields`, `live.swarms`), keeping omitted = bit-identical and coordinating new `LIVE_KEYS` entries with `QD-BIO-D2` freeze-pin test (`prereg_e3b_manifest_tests.rs:417-426, 584-588`)<br>**B:** Bundled single `live.ecosystem_v2` toggle | **A** — clean per-feature ablation, independent negative controls, zero interference with frozen E3b |

---

## 5. Consequences

### Positive
- Replaces perpetual respiratory energy recycling with physically grounded **solar LUE inflow** and
  **`total_cost`-routed heat dissipation**, unlocking realistic trophic pyramids and emerging ~10% TTE
  without breaking closed-EU accounting invariants.
- Keeps legacy closed-EU runs (`open_energy_flow = None`, `E2`, `E3`, `E3b`, `SEL2-P2`) **100%
  bit-identical** with zero RNG shift or schema breakage.
- Enables rich multi-scale ecosystems (trees, rabbits/wolves, swarms, soil/grass/larval fields) well
  inside the headless `~5.64 ms p50 @ N = 1,000` (`60 Hz`) compute envelope and strictly respects
  `QD-REALMS` ordering.

### Negative / Risks & Mitigations
- **Respiration heat out without balanced solar LUE inflow can starve ecosystems:** If solar LUE is
  calibrated too low or nutrient N (`V5`) locks up inside recalcitrant pools, ecosystems collapse to
  zero energy faster than closed loops. *Mitigation:* enforce `E1` (`BENCH-ENTITIES`) + multi-seed
  `V10` soak (`E7`) tuning checks before turning on shared-island defaults.
- **EU drift across multiple stores (`Plants` + `Swarms` + `Fields` + `Carcasses` + `Animals` + `Detritus`):**
  Forgetting one compartment delta (`credit_reserve` / `debit_reserve` vs raw `f32 +=`) creates subtle
  drift. *Mitigation:* apply the [`ecology.rs:434-440`](../../src-tauri/src/core/ecology.rs#L434-L440)
  *"measured stored `f32 -> f64` delta"* pattern everywhere and audit `|R_open| <= 1e-3` on every census tick.
- **Hot-loop allocation / archetype thrash:** Swarm creation/splitting/merging must live in pre-allocated
  resource pools (`SwarmPool`, `DynamicFields`) checked by `TrackingAllocator == 0`, never spawning ECS
  archetypes per insect/swarm per tick.

---

## 6. Proposed Verification Gates (`ECO-S01`–`ECO-S08`)

| Gate | Requirement & Verification Criterion | Threshold | Status |
|---|---|---|---|
| **ECO-S01** | **Flag-off bit-identity (`open_energy_flow = None`, all new tiers off):** Multi-seed headless trajectories (`E2`, `E3`, `E3b`) match trunk `world_checksum` and closed EU | Bit-exact `world_checksum`; `0` RNG draws added | proposed |
| **ECO-S02** | **Open-flow conservation (`|R_open| <= 1e-3`):** Combined solar LUE inputs (`cum_solar_in`), heat sinks (`cum_heat_out`), detritus, carcasses, swarms, and fields reconcile exactly at every `ecosystem_census_system` tick | `\|R_open\| <= 1e-3` EU across multi-thousand ticks | proposed |
| **ECO-S03** | **All `total_cost` coupling (`metabolic_decay_system`):** Every metabolic/allometric/disease/heat-loss rate routes through `total_cost` (`world_systems.rs:157`); direct energy deduction fails negative control | Exact heat-sink == `heat_loss_fraction * respired` (`f64` sum) | proposed |
| **ECO-S04** | **Causal solar & cap negative controls:** Blocking sunlight (`I_solar = 0`) stops LUE primary production -> biomass declines via heat loss; disabling `live.prey_cap` removes the `80`-prey kink cleanly | Distinct pre-registered trajectory response + cap negative control fails | proposed |
| **ECO-S05** | **Allometric scaling parity:** `M^0.75` basal rate (`mte_basal_metabolism`) and mass-scaled timescales satisfy analytic Kleiber/lifespan ratios within float tolerance | Exact formula match (`<= 1e-6` relative) | proposed |
| **ECO-S06** | **Tier 2/Tier 3 zero-alloc & determinism:** `TrackingAllocator` across field dispersion + swarm updates + biting transfers; `3`-process same-OS determinism parity | `0` hot-tick heap allocations; equal `world_checksum` | proposed |
| **ECO-S07** | **`BENCH-ENTITIES` (`E1`) benchmark gate:** `live_schedule_scaling` (`500` & `2,000` sessile entities + active swarms) stays comfortably inside real-time ceiling before enabling swarms/flora at scale | Measured `estimates.json` p50/p95 recorded (`docs/paper/data/scaling.md` format) | proposed |
| **ECO-S08** | **`V10` density-dependent soak & TTE/NPP observables (`E7`):** Extended multi-seed soak shows bounded dynamics without `NaN`/handle/entity leaks (`V10` gates) and matches honest emergent TTE/NPP telemetry | Zero leaks; `\|R_open\| <= 1e-3`; no ungrounded species labels (`AE-S11`) | proposed |

---

## 7. Affected Documents

- Implementation Card Plan: [`2026-11-24-ecosystem-cards.md`](../ai/planning/2026-11-24-ecosystem-cards.md) (`E1`–`E7`)
- Experiment Contract *(edit only post-acceptance under card `E2`)*: [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) (`:121-124`)
- Related ADRs & Specs: [`ADR-0002`](ADR-0002-world-laws-and-exotic-energy.md), [`ADR-0007`](ADR-0007-live-ecology-sustainability.md), [`ADR-0008`](ADR-0008-world-past-and-time.md), [`2026-11-24-v4-carcasses-decomposition-spec.md`](../ai/planning/2026-11-24-v4-carcasses-decomposition-spec.md), [`2026-11-24-v5-fields-plants-feedback-spec.md`](../ai/planning/2026-11-24-v5-fields-plants-feedback-spec.md), [`2026-11-24-v10-soak-leak-spec.md`](../ai/planning/2026-11-24-v10-soak-leak-spec.md)
- Benchmarks & Governance: [`docs/paper/data/scaling.md`](../paper/data/scaling.md), [`docs/how-to/AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md) (`QD-REALMS`), [`docs/decisions/README.md`](README.md)

---

## 8. Owner Decision Block (To Be Filled on Acceptance)

```text
QD-ECOSCALE-ACCEPT (owner, <YYYY-MM-DD>) = ACCEPT ADR-0018:
  D-ECO-1 = <A|B>, D-ECO-2 = <A|B>, D-ECO-3 = <A|B>,
  D-ECO-4 = <A|B>, D-ECO-5 = <A|B>, D-ECO-6 = <A|B>;
  Contracts & cards E1-E7 authorized per ADR-0018 (status -> accepted, decision_date -> <YYYY-MM-DD>)
```
