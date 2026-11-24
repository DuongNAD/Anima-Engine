---
title: ADR-0016 — Changing World Surface (Three Timescales, Process Laws, Era Transitions & Allopatry)
status: proposed
owner: simulation-architecture
last_reviewed: 2026-11-24
decision_date: pending
supersedes: none
superseded_by: none
---

# ADR-0016 — Changing World Surface (Three Timescales, Process Laws, Era Transitions & Allopatry)

> **Status: `proposed` — awaiting owner acceptance and decisions `D-WCHG-1`–`D-WCHG-6`.**
> Do **not** begin Rust or frontend implementation of world-surface change until the owner accepts
> this ADR and resolves `D-WCHG-1`–`D-WCHG-6` (and note `QD-BIO-D2`: any new post-freeze `live.*` key or
> observable must obey the merged `C12 E3B-PIN` `#260` default-to-legacy partition in
> `src-tauri/tests/prereg_e3b_manifest_tests.rs:84, 120, 574-603, 753-760`).
> Initiated per `QD-LIFE` extension (`MSG_20261124_130000`, `133000`) and owner decision
> **`QD-BIO-D3`** (`MSG_20261124_133000_from_owner_research_decisions_dell.md`, `PR #255`).
> Code anchors below are verified on `origin/feat/ecological-selection` at trunk commit
> `e7b99745c59d7da35e884dd9414122b1d24fc648`.

---

## 1. Context & Verified Baseline (`e7b99745`)

Under the owner's `QD-LIFE` extension (*"change must also apply to the map"*), the physical landscape
cannot remain permanently static from genesis to infinity. Changing the world while preserving
scientific reproducibility and 60 Hz performance must respect five trunk invariants verified at
`e7b99745c59d7da35e884dd9414122b1d24fc648`:

1. **Pinned Immutable Base Artifact (`ADR-0010`, `world_artifact.rs`):**
   - [`ADR-0010`](ADR-0010-pinned-world-artifact.md) defines `WorldArtifact` (`ANMW v2` at
     `src-tauri/src/core/world_artifact.rs:41-43, 67-82`) and `WorldIdentity` (`:122-137`, including
     `sim_world_sha256: Option<[u8; 32]>` at `:136` added in `WV2-3` for `ANMW v3`). Two runs only
     share an environment when their pinned artifact digests match; regenerating a terrain produces
     a distinct artifact and hash rather than mutating bytes in place.
2. **Engine Compatibility Era vs Geological Eras (`world_record/types.rs:156-160`):**
   - `WorldRecordHeader` (`src-tauri/src/core/world_record/types.rs:132-163`) carries `era_index: u32`
     (`:157`, documented strictly as *"Engine compatibility era (`0` for a fresh continuous run)"*)
     and `prev_era_tail_hash: Option<SegmentHash>` (`:159-160`), implementing [`ADR-0008`](ADR-0008-world-past-and-time.md)
     engine-version boundary tracking. Overloading `era_index` for landscape evolution would mix
     software compatibility boundaries with geological history (`RESEARCH_REVIEW` §2 Finding 5).
3. **Pre-Allocated Dynamic State & Causal Interventions (`dynamic_fields.rs`, `intervention.rs`):**
   - `DynamicFields` (`src-tauri/src/core/dynamic_fields.rs:66-108`) provides pre-allocated SoA
     climate, groundwater/surface water, `soil_moisture`, `soil_nitrogen`, `soil_organic`,
     `root_cover`, `erosion` (`step_erosion` at `:472-480`), and `turbidity`, gated by zero-alloc
     conservation ledgers (`water_budget_residual` at `:400`, `nutrient_budget_residual` at `:461`).
   - `InterventionKind` / `InterventionCommand` (`src-tauri/crates/anima-domain/src/intervention.rs:108-162`)
     already expresses spatial `Region` (`Global | Cell | Rect | Radius`) + temporal `Curve` forcing
     tagged with explicit `CauseId`, paired with [`2026-11-24-v6-interventions-causeid-spec.md`](../ai/planning/2026-11-24-v6-interventions-causeid-spec.md)
     (`"no magic effects"`).
4. **Offline Geomorphic Solver & `WV2` Verification Pipeline (`anima-worldgen`):**
   - `src-tauri/crates/anima-worldgen` runs offline deterministic multigrid generation
     (`run_worldgen_to_dir` at `pipeline.rs:587-593` and `run_worldgen_to_dir_with_schedule` at
     `:597-601`, configured via `WorldV2Config` at `config.rs:23-48`): `TectonicField` & segmented
     rifts (`tectonic.rs:41-52, 294-334`), `FluvialParams` (`erosion_fluvial.rs:39-60`) + coupled
     `step_fastscape_sd_dag` (`:293`), `DepositionConfig` alluvial/meander/delta deposition
     (`deposition.rs:51-76`), lakes/waterfalls, climate, biomes, and caves, emitting `AnmwV3Sim`
     (`anmw_v3.rs:106-124`) and baked `4096²` textures consumed by `WorldTerrainLod`
     (`src/components/Landscape/WorldTerrainLod.tsx:26-42`).
   - Solver physics work established in `WV2-2-PHYS` ([`2026-11-23-wv2-2.md`](../ai/planning/2026-11-23-wv2-2.md) `:132-160, 1117-1340`),
     the offline bake engine in `WV2-5` (`#265`, [`2026-11-24-wv2-5.md`](../ai/planning/2026-11-24-wv2-5.md)), and follow-up
     `WV2-PHYS2` under `MAP-D4` provide the physical erosion/uplift/bake engine and quantitative
     verification suite (`G-SHAPE`, `G-COMP v2`, `G-SIM` at
     `src-tauri/tests/world_v2_sim_gate_tests.rs:1-16`,
     [`2026-11-20-world-v2-design.md`](../ai/design/2026-11-20-world-v2-design.md)).
5. **Reproducibility & Experiment Contract (`EVOLUTION_EXPERIMENT_CONTRACT.md`):**
   - `WorldLawSet` (`src-tauri/src/core/experiment/laws.rs:28-32, 79`), `WorldIdentity`
     (`world_artifact.rs:122`), and `ExperimentManifest::fingerprint` (`manifest.rs:448`) lock
     initial laws and world inputs before `t = 0` (`ER01`, `ER07`, `ER08`), while `ER09` (`AE-S11`,
     `AE-S14`) and `AE-S12` govern species claims and causal attribution.

---

## 2. Core Governance Decision (`QD-BIO-D3`, Decided 2026-11-24)

Per owner decision **`QD-BIO-D3`** (`MSG_20261124_133000`, `PR #255`):

- **`era_index` stays engine-only (`world_record/types.rs:157`):** `WorldRecordHeader::era_index` and
  `prev_era_tail_hash` (`:159-160`) strictly track engine/schema compatibility breaks where old logic
  cannot replay newer ticks. Geological era transitions **never** increment or overload `era_index`.
- **Dedicated Geological Era Fields on `WorldRecordHeader` (`to be implemented by W4`):** Extend
  `WorldRecordHeader` (`world_record/types.rs:132-163`) with opt-in, serde-backward-compatible fields:
  - `#[serde(default, skip_serializing_if = "is_zero_u32")] pub geological_era: u32` (`0` = baseline / initial genesis artifact),
  - `#[serde(default, skip_serializing_if = "Option::is_none", with = "opt_hex32")] pub prev_geological_era_tail_hash: Option<SegmentHash>`,
  - `#[serde(default, skip_serializing_if = "Option::is_none", with = "crate::core::world_artifact::opt_sha256_hex")] pub geological_era_artifact_sha256: Option<[u8; 32]>` (`world_artifact.rs:84-115`, pinning the new `ANMW v3` artifact hash generated for that geological era),
  - `#[serde(default, skip_serializing_if = "Option::is_none")] pub era_transition_reason: Option<EraTransitionReason>` (natural tectonic/volcanic/erosional threshold vs declared `CauseId` intervention; absent on era-0 / legacy records so header digests stay backward-compatible).

---

## 3. Three-Timescale Architecture (`Tier 1 -> Tier 2 -> Tier 3`)

Landscape dynamics decompose by physical timescale and implementation order:

| Tier | Physical Phenomena | Data Representation & Artifact Impact | Tick Budget & Rollback | Rollout Order |
|---|---|---|---|---|
| **Tier 1: Living Surface** | Soil moisture/N/organic dynamics (`V5`), grass & evolving canopy/roots (`ADR-0015`), seasonal lake/river stage fluctuation, snowpack accumulation/melt | Already live `Vec<f32>` SoA arrays in `DynamicFields` (`dynamic_fields.rs:66-98`) + minimal seasonal `snow_depth` / `river_stage_delta` buffers. **Pinned `WorldArtifact` untouched.** | Fixed pre-allocated `256²` buffers, strided where appropriate, **`0` heap allocations/tick**; flag-off bit-identical. | **First (`W1`, after `V5`)** |
| **Tier 2: Local Disturbances** | Wildfire burn scars & heat/ash release, flash floods, landslides, river bank avulsion, localized fault offset, minor lava/tephra cones, animal engineering (burrows/dams) | Bounded pre-allocated `SurfaceDeltaOverlay` resource (`delta_elev_m: Vec<f32>`, `delta_water_stage: Vec<f32>`, `cover_disturbance: Vec<u8>`, `burn_heat_timer: Vec<f32>`, dirty chunk bitset). Recorded in snapshots & `WorldRecord`; renderer patches delta tiles cleanly. **Base artifact stays immutable.** | Pre-allocated once (`<= D-WCHG-1` cap), **`0` heap allocs/tick**; conditional `world_checksum`; absent = bit-identical to trunk. | **Second (`W2`–`W3`)** |
| **Tier 3: Geological Era Transitions** | Major volcanic island creation, mountain uplift/rift collapse, canyon carving/delta progradation, catastrophic sea-level change | Pause island cleanly -> **offline** `anima-worldgen` re-bake (`WV2-2-PHYS` / `WV2-PHYS2` under `MAP-D4`) from previous era terrain + Tier-2 `SurfaceDeltaOverlay` + forcing -> emit new pinned `ANMW v3` + baked `4096²` tiles + `G-SHAPE`/`G-COMP v2`/`G-SIM` report -> open new `geological_era` on `WorldRecord`. | **Zero in-process terrain re-bakes** (`RESEARCH_REVIEW` §3); `0` per-tick overhead during normal play; cross-era migration preserves `ADR-0001` phenotypes & `AgentBrain`s. | **Last (`W4`–`W6`)** |

### 3.1. Concrete Design Guarantees Per Tier

1. **Tier 1 (`DynamicFields` + Hydrology/Snow):** Seasonal lake elevation and snowpack freeze/melt
   adjust snow/surface-water stores and walkable shoreline masks (`TerrainNavigation`) through
   conserved water transfers without mutating `TerrainMap` base elevation (`water_budget_residual`
   at `dynamic_fields.rs:400` stays `< 1e-4`), keeping closed-EU / open-energy invariants intact.
2. **Tier 2 (`SurfaceDeltaOverlay`, Energy Conservation & Organism `total_cost`):** Effective cell
   elevation at `(col, row)` during simulation and hydrology/walkability queries is
   `base_elevation(artifact) + overlay.delta_elev_m[i]`. Every Tier-2 disturbance either decays
   slowly back toward substrate equilibrium (temporary floodwater, vegetation scar recovery) or persists
   in the overlay until folded into the next Tier-3 geological bake (`SurfaceDeltaOverlay` resets to
   zero once absorbed into the new pinned `ANMW v3` artifact). Animal engineering (beaver-style dams)
   extends `DynamicFields::set_dam` (`dynamic_fields.rs:244`). Two strict conservation rules apply:
   - **Wildfire & ash biomass accounting (`EnergyLedger`):** Under default closed EU (`open_energy_flow = None`,
     `src-tauri/crates/anima-domain/src/energy.rs:49-83, 123`), wildfire-burned plant biomass
     transfers `Compartment::Plants -> Compartment::Detritus` (`|closed_total_eu - baseline| <= 1e-3`).
     Only when [`ADR-0018`](ADR-0018-ecosystem-scales-and-cycles.md) (`proposed`, `#263` merged) `OpenEnergyFlowLaw` is active does radiative combustion heat exit via
     declared boundary sink `EnergyEvent::RespiredHeatLoss` while ash/char enters `Detritus` & soil `N`/`organic` (`|R_open| <= 1e-3`).
   - **Organism disturbance costs strictly through `total_cost` (`CLAUDE.md` hard rule):** Every
     continuous organism stressor caused by surface change (tsunami saltwater stress under `D-WCHG-3`,
     fire/smoke/ash metabolic burden, snow/cold exposure, slope terrain difficulty) must add into
     **`total_cost` in `metabolic_decay_system`** (`src-tauri/src/core/world_systems.rs:157, 174-177`)
     (or the plant `total_cost` accumulator under `ADR-0015`) so 100% of spent EU routes cleanly through
     `EnergyLedger` — never subtracted from organism reserves outside `total_cost`.
3. **Tier 3 (Strictly Offline Re-Bake & Deterministic Migration):** Consistent with
   `RESEARCH_REVIEW` §3, live tick systems **never** invoke `anima-worldgen` multigrid erosion inside the
   60 Hz loop. An era transition checkpoints `WorldRecord` cleanly (`geological_era += 1`), executes the
   offline deterministic `anima-worldgen` worker/CLI pass (updating lithology, topography, hydrology,
   lakes/waterfalls, climate, biomes, `ANMW v3`, and renderer tiles), validates `G-SHAPE`/`G-COMP v2`/`G-SIM`,
   and restores saved entities through `Migration`-grade rules preserving `AgentBrain`, `AgentLineageId` / `LiveLineage`,
   saved morphology/phenotypes (`MorphologyGenotype` / `DevelopedPhenotype` once `V0` lands), and
   flora (`PlantGenotype` once `ADR-0015` lands) verbatim without re-rolling.

---

## 4. Process Laws, Triggers, Scale Routing & Time Honesty (`MSG_20261124_133000`)

> **Core Invariant:** *"Every change = process law + trigger + scale rule. Nothing is hand-painted."*

### 4.1. Physical Process Models (`anima-worldgen` Reuse + New Geomorphic Operators)

1. **Existing `anima-worldgen` physics (`pipeline.rs`, `tectonic.rs`, `erosion_fluvial.rs`, `deposition.rs`, `WV2-2-PHYS` / `WV2-PHYS2`):**
   - Tectonic uplift fields (`uplift_u`), `L_seg` segmented rifts (`tectonic.rs:314-332`), Fastscape-SD
     stream-power bedrock/alluvial incision (`step_fastscape_sd_dag`, `erosion_fluvial.rs:293`),
     alluvial fans/meanders/DeltaRCM coastal progradation (`deposition.rs`), thermal slope diffusion,
     and priority-flood lakes/drainage.
2. **New physical process operators added as world-law parameters:**
   - **Volcanic eruption (`VolcanicProcessParams`):** Eruptive vent pressure / plume volume ejects lava
     flows (viscous steepest-descent routing + thermal cooling solidifying into `LithologyClass::BasaltVolcanic`
     in `tectonic.rs:19`) and wind-dispersed tephra/ash deposition raising surface elevation, temporarily
     loading local habitats (`Plants` burial -> `Detritus`/`soil_organic`), and potentially breaking sea
     level (`h > 0`) to construct new oceanic volcanic islands.
   - **Earthquake & fault displacement (`SeismicProcessParams`):** Bounded dislocation along fault azimuths
     applying step uplift/subsidence offsets, triggering Mohr-Coulomb-threshold shallow landslides on
     steep unrooted slopes (directly amplified when `DynamicFields::root_cover[i]` is stripped by fire
     or deforestation!), and (if `D-WCHG-3 = A`) spawning offshore-fault wave inundation pulses.
   - **Sea-level eustasy (`SeaLevelProcessParams`):** Monotone or cyclic relative sea-level offset
     merging or dividing islands and reshaping shallow coastal shelves (`d_coast`, `tectonic.rs:300-311`).

### 4.2. Stochastic Natural Triggers vs Declared `CauseId` Host Interventions

- **Natural event process (`sim_stream::WORLD_DISTURBANCE = 8`, extending `core/resources.rs:925-948`):**
  Uses dedicated stream ID `8` (distinct from `resources.rs:925-948` slots `1..=5`, `V0` `LIVE_DEVELOPMENT`,
  and `DORMANCY_STREAM = 7` at `aggregate_population.rs:190`; exact final slot reconciled at trunk integration).
  Event recurrence rates (`hazard_rate_per_era`, regional stress accumulators, climate flood/fire risk)
  live strictly inside the immutable genesis `WorldLawSet`. Sampling draws solely from dedicated stream
  `sim_stream::WORLD_DISTURBANCE` so toggling natural geological hazards **never perturbs animal ecology
  or mutation `SimRng` streams** (same isolation discipline as `AE3` / `LIVE_GENESIS_BRAINS`). While laws
  remain fixed for a run, each realized eruption, quake, or landslide event is logged as deterministic
  runtime state in `WorldRecord`.
- **Host / experiment intervention (`InterventionCommand`, `ADR-0004`):** Hosts and preregistered
  experiments may trigger specific geophysical events (fault slip, eruption at `(Region)`, ash pulse,
  sea-level step) with explicit `CauseId`, `start_tick`, and magnitude (`intervention.rs:108-162`).

### 4.3. Deterministic Tier-2 vs Tier-3 Scale Router & Honest Geological Time

- **Scale router rule (`D-WCHG-2`):** Localized landslides, bank breaches, wildfire scars, small fault
  steps, and cinder/lava lobes whose maximum vertical displacement `max(|Δh|) < H_era` and affected
  non-ocean area fraction `< A_era` apply immediately in real time via **Tier 2 (`SurfaceDeltaOverlay`)**.
  Events exceeding `H_era` or `A_era` — or **any submarine eruption / uplift that breaks the ocean
  surface to create a new emergent landmass** (`base_h <= 0 && post_h > 0` across `>= N_min` cells) or
  sea-level change crossing shoreline thresholds — route cleanly to a **Tier-3 offline era transition**.
- **Time honesty (`MSG_20261124_133000`):** Forming a volcanic island or carving a river delta takes
  thousands to millions of years physically. Both chronicle entries and UI HUD banners explicitly disclose
  when a **Tier-3 era transition compresses geological time** (`"Geological time skip: +Δt kyr"`),
  contrast immediate local catastrophes (`Tier 2`) from multi-millennial era jumps (`Tier 3`), and
  state migration rules clearly (`ADR-0001` phenotypes and `AgentBrain`s survive relocation verbatim).

---

## 5. Experiment Validity, `AE-S12` Causal Chain & Allopatric Speciation Target

- **Opt-in World Law (`laws.rs:28-32, 79`, `manifest.rs:448`):**
  - Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub world_change: Option<WorldChangeLaw>`
    on `WorldLawSet`. Following the baseline invariance discipline (`WorldIdentity::sim_world_sha256`
    at `manifest.rs:459-464`), `world_change: None` emits **zero tag bytes** inside
    `WorldLawSet::canonicalize()` (`laws.rs:85-118`, unlike `exotic_energy` `tag(0x00)` at `:90`),
    leaving every legacy `WorldLawSet::baseline().fingerprint()` and existing E2/E3/E3b/SEL2-P2 run
    bit-for-bit identical (`0` draws on `SimRng`, `0` overlay allocs).
  - Control vs treatment forks compare stationary terrain (`world_change: None`) vs active process laws
    or checkpoint-forked `CauseId` geophysical events pinned by `WorldIdentity` + geological era digests.
- **Explicit `AE-S12` causal mechanism chain (`CausalLedger`):**
  Every Tier-2 disturbance and Tier-3 era transition records verifiable multi-hop edges:
  `geophysical_or_climate_event (CauseId) -> terrain_or_overlay_delta -> habitat_or_barrier_shift -> altered_selection_or_dispersal -> lineage_divergence`
  so the chronicle ([`ADR-0014`](ADR-0014-shared-island-host-and-guests.md) `I8` track in [`2026-11-24-island-cards.md`](../ai/planning/2026-11-24-island-cards.md)) reports empirically verified history (*"Eruption E-04 created
  Basalt Island B in Geological Era 2; descendants of lineage L-02 colonized it at tick T"*).
- **Scientific target — Allopatric Speciation (`ER09`, `AE-S11`, `AE-S14`):**
  Emergent islands, uplifted mountain barriers, river avulsions, and rising sea straits split panmictic
  populations geographically, testing reproductive isolation via genetic compatibility decay ([`ADR-0013`](ADR-0013-live-creature-lifecycle.md)
  `accepted` `#262` sexual amendment / `ADR-0015` backticked flora). We name allopatric divergence strictly as an **empirical
  target to measure, not a guarantee**: any `"species"` claim remains gated by [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md)
  `ER09` (`AE-S11` multi-generation cluster & gap + `AE-S14` multi-seed paired ensembles), never automatic
  badges on morphs or islands.

---

## 6. Open Owner Decision Gates (`D-WCHG-1`–`D-WCHG-6`)

| D-Slot | Question | Options (Lead Recommendation First) |
|---|---|---|
| **`D-WCHG-1`** | Tier-2 `SurfaceDeltaOverlay` precision & memory cap | **(A) Recommended:** Compact `256²` sim-grid delta buffers (`f32` `delta_elev_m`, `delta_water_stage`, `u8` `cover_disturbance`, `f32` `burn_heat_timer`, dirty-chunk mask matching `WorldTerrainLod` chunk grid) + bounded `MAX_OVERLAY_EVENTS_PER_ERA` log (`~1 MiB` total fixed allocation). **(B)** Full high-res `4096²` runtime overlay (`~192 MiB+` — rejected for CPU/RAM bounds). |
| **`D-WCHG-2`** | Tier-2 vs Tier-3 scale thresholds (`H_era`, `A_era`, new island trigger) | **(A) Recommended:** Trigger Tier-3 era transition when localized event peak displacement `max(\|Δh\|) >= 120 m`, or affected walkable/coastal area fraction `>= 5%` of total land, or cumulative Tier-2 `\|Δh\|` drift reaches threshold, or any submarine/coastal event creates/submerges an isolated landmass (`>= 8` connected sim cells). **(B)** Only manual/host-triggered era transitions (automatic Tier 2 always stays in overlay). |
| **`D-WCHG-3`** | Earthquake / offshore landslide tsunami physics in Tier 2 | **(A) Recommended:** Include bounded coastal wave runup as a Tier-2 surface-water pulse routed through `DynamicFields` water conservation (adding saltwater stress + coastal erosion) after core `W2`/`W3` fault landslides land. **(B)** Defer tsunamis until dedicated oceanic/hydrodynamic phase after Tier 2. |
| **`D-WCHG-4`** | Cross-era organism migration & habitat-loss policy during Tier-3 era jumps | **(A) Recommended:** Classify existing cells post-bake: cells remaining safe walkable land carry creatures/plants directly (phenotype/brain intact); organisms stranded on newly flooded abyssal/active lava cells suffer realistic mortality to `Detritus` unless within coastal/short-range retreat radius `R_flee` where mobile animals relocate cleanly to nearest walkable land (`ADR-0001` phenotype preserved, `0` re-rolls). **(B)** Relocate 100% of organisms to nearest valid cell with zero era mortality. |
| **`D-WCHG-5`** | Natural hazard frequency on default shared islands (`1 day = 20 min`, `1 year = 90 min`) | **(A) Recommended:** Low natural Tier-2 disturbance cadence (subtle landslides/fire/floods on rare multi-day ecological intervals) and Tier-3 era transitions triggered only over multi-hour cumulative drift or explicit catastrophic scenario / host event. **(B)** High-frequency rapid eruptions every few real minutes (catastrophic showcase preset only). |
| **`D-WCHG-6`** | `G-SHAPE`, `G-COMP v2` & `G-SIM` verifier handling on post-era artifacts | **(A) Recommended:** Always execute `G-SHAPE`, `G-COMP v2`, and `G-SIM` offline after every Tier-3 bake and attach `gates.json` provenance honestly (`G-SIM` GS-1/GS-2 walkability & EU bounds required hard; geomorphic/biome `G-SHAPE`/`G-COMP v2` shifts vs baseline Sierra Nevada massif reported transparently, never tuned, since a newly built volcanic island legitimately shifts biome fractions). **(B)** Require every post-catastrophe era to pass original continental `G-COMP v2` biome ratios cleanly (risks blocking valid volcanic islands). |

---

## 7. Verification Matrix (`WCHG-S01`–`WCHG-S07`) & Roadmap

Implementation proceeds strictly through sequenced cards **`W1`–`W7`** in
[`docs/ai/planning/2026-11-24-world-change-cards.md`](../ai/planning/2026-11-24-world-change-cards.md):

| Gate | Positive Assertion | Mandatory Negative Control |
|---|---|---|
| **`WCHG-S01`** | Flag-off / `world_change: None` emits `0` canonicalize tag bytes (`WorldLawSet::baseline().fingerprint()` unchanged), allocates `0` overlay buffers in default loops, keeps `era_index` untouched, and matches trunk `world_checksum` bit-for-bit. | Mutate `WorldLawSet::canonicalize()` when `world_change` is `None` or allocate empty `SurfaceDeltaOverlay` touching `SimRng` -> bit-identity fails red. |
| **`WCHG-S02`** | Tier 1 (`W1`) seasonal lake/snow & `V5` erosion updates preserve `water_budget_residual` (`< 1e-4`), `nutrient_budget_residual` (`< 1e-4`), and closed/open EU (`<= 1e-3`) with `TrackingAllocator = 0`. | Allow snow/water level shift without accounting `total_water` -> `water_budget_residual` test fails red. |
| **`WCHG-S03`** | Tier 2 (`W2`–`W3`) `SurfaceDeltaOverlay` events (fire, flood, landslide, fault offset, damming) execute with `0` tick heap allocations, route wildfire/organism stress strictly via `EnergyLedger` / `total_cost`, draw solely from `sim_stream::WORLD_DISTURBANCE`, replay bit-identically via `WorldRecord`, and match `run(N) == run(K) -> save -> load -> run(N-K)`. | Omit `SurfaceDeltaOverlay` from `world_checksum` or `Snapshot`, charge disturbance stress outside `total_cost`, or draw hazard RNG from animal `SimRng` -> gate fails red. |
| **`WCHG-S04`** | `QD-BIO-D3` schema separation: Tier-3 geological era transitions advance `WorldRecordHeader::geological_era` while keeping `WorldRecordHeader::era_index` unchanged (`0` if engine compatibility unchanged). | Increment `era_index` on geological era transition -> `QD-BIO-D3` invariant test fails red. |
| **`WCHG-S05`** | Tier 3 (`W5`) offline `anima-worldgen` re-bake (`WV2-2-PHYS` / `WV2-PHYS2` + lava/ash/sea-level) runs purely offline (zero in-tick rebuilds), generates a valid `ANMW v3` artifact with updated `sim_world_sha256`, and records `G-SHAPE`/`G-COMP v2`/`G-SIM` honestly. | Trigger terrain re-bake synchronously inside `build_tick_schedule` or skip updating `WorldIdentity::sim_world_sha256` -> CI/runtime check fails red. |
| **`WCHG-S06`** | Cross-era migration (`W6`) carries saved phenotypes (`MorphologyGenotype` / `DevelopedPhenotype` under `V0`), `AgentBrain`, `AgentLineageId`, and (`PlantGenotype` under `ADR-0015`) into the new geological era identically (`run -> save -> era_transition` equals across same-OS processes), with zero phenotype re-rolls. | Re-run `decode_genotype` (or `develop_at_birth` when `V0` is active) across era migration -> `WCHG-S06` migration test fails red. |
| **`WCHG-S07`** | `AE-S12` & `AE-S14`: Causal graph traces `event (CauseId) -> delta -> barrier -> selection -> lineage` across paired same-seed ensembles; UI/chronicle discloses compressed geological time and obeys `ER09` (`AE-S11` before `"species"` labels). | Drop `CauseId` parent linkage or emit `"species"` label without `AE-S11` pass -> contract verifier fails red. |

---

## 8. Related Documents

- Pinned artifact, world history & contracts: [`ADR-0008`](ADR-0008-world-past-and-time.md), [`ADR-0010`](ADR-0010-pinned-world-artifact.md), [`ADR-0013`](ADR-0013-live-creature-lifecycle.md) (`accepted`, `#262`), [`ADR-0014`](ADR-0014-shared-island-host-and-guests.md) (`accepted`, `#256`), [`ADR-0018`](ADR-0018-ecosystem-scales-and-cycles.md) (`proposed`, `#263`), [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md), [`2026-11-20-world-v2-design.md`](../ai/design/2026-11-20-world-v2-design.md), [`2026-11-23-wv2-2.md`](../ai/planning/2026-11-23-wv2-2.md), [`2026-11-24-wv2-5.md`](../ai/planning/2026-11-24-wv2-5.md)
- Predecessor mechanisms & cards: [`2026-11-24-v1-dynamic-fields-schedule-spec.md`](../ai/planning/2026-11-24-v1-dynamic-fields-schedule-spec.md), [`2026-11-24-v5-fields-plants-feedback-spec.md`](../ai/planning/2026-11-24-v5-fields-plants-feedback-spec.md), [`2026-11-24-v6-interventions-causeid-spec.md`](../ai/planning/2026-11-24-v6-interventions-causeid-spec.md), [`2026-11-24-ecosystem-cards.md`](../ai/planning/2026-11-24-ecosystem-cards.md), [`2026-11-24-world-change-cards.md`](../ai/planning/2026-11-24-world-change-cards.md)
- Unmerged parallel references (cited strictly in backticks until merged to trunk): `ADR-0015` (draft `#268`, evolving plants Tier-1 vegetation), `ADR-0017` (local `D18`, evolved signals), `ADR-0019` (`QD-BRAINBODY`, unmerged `PR #259`, brain-body interface), `WV2-PHYS2` (`MAP-D4` provisional `C01` rework track).
