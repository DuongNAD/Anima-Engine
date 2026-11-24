---
phase: planning
feature: changing-world-surface
title: Changing World Surface Implementation Cards (W1-W7 Per ADR-0016)
description: Sequenced implementation cards W1 through W7 covering Tier-1 living surface dynamics, Tier-2 zero-alloc SurfaceDeltaOverlay and local disturbances, dedicated geological_era records, offline anima-worldgen WV2-2-PHYS/WV2-PHYS2 era transitions, cross-era migration, and AE-S12 allopatric isolation validation
status: proposed
owner: anima-dell
last_reviewed: 2026-11-24
---

# Changing World Surface — Implementation Cards (`W1`–`W7`)

> **Planning document only (`status: proposed`, card `D15 WORLD-CHANGE-ADR`).**
> No Rust, TypeScript, or contract edits are performed here. All cards below wait for owner
> acceptance of [`ADR-0016`](../../decisions/ADR-0016-changing-world-surface.md) (`D-WCHG-1`–`D-WCHG-6`),
> respect `WV2-2-PHYS` ([`2026-11-23-wv2-2.md`](2026-11-23-wv2-2.md)) / follow-up `WV2-PHYS2`
> (`MAP-D4`), and any card adding new `live.*` manifest keys or observables waits for `C12 E3B-PIN`
> (`QD-BIO-D2`). Code anchors verified on `origin/feat/ecological-selection` at
> `902e505d58dbe034a1771bcea352742c296985d9`.

---

## Dependency Overview

```text
W1 (Tier 1 Living Surface: V5 DynamicFields + seasonal lake stage & snow cover)
 └──► W2 (Tier 2 Bounded SurfaceDeltaOverlay + zero-alloc snapshot / world_checksum / replay)
       └──► W3 (Tier 2 Process Laws: fire, floods, landslides, faults, renderer delta tile patches)
             └──► W4 (QD-BIO-D3 Schema: dedicated WorldRecordHeader::geological_era vs era_index)
                   └──► W5 (Tier 3 Offline anima-worldgen WV2-2-PHYS/WV2-PHYS2 Era Re-Bake + G-SHAPE/G-COMP/G-SIM)
                         └──► W6 (Cross-Era Migration preserving ADR-0001 phenotypes & honest geological time)
                               └──► W7 (AE-S12 Causal Chain & Paired Allopatric Speciation Suite AE-S11/AE-S14)
```

---

## Card `W1` — Tier 1 Living Surface: Seasonal Lake Stage, Runoff & Snow Cover (`WCHG-S02`)

- **START WHEN:** `ADR-0016` accepted (`D-WCHG-1`), `V5` ([`2026-11-24-v5-fields-plants-feedback-spec.md`](2026-11-24-v5-fields-plants-feedback-spec.md)) merged, and `C12 E3B-PIN` merged.
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/dynamic_fields.rs` (`snow_depth` / `river_stage_delta` SoA buffers, conserved phase-change water bookkeeping)
  - `src-tauri/src/core/environmental_systems.rs`, `src-tauri/src/core/terrain_navigation.rs`
  - `src-tauri/src/core/live_experiment.rs`, `src-tauri/src/core/experiment/laws.rs` (`WorldChangeLaw` Tier-1 toggles)
  - `src-tauri/src/core/simulation_state.rs`, `src-tauri/src/core/snapshot.rs`
  - `src-tauri/tests/living_surface_tests.rs` (new)
- **Scope:** Extend `DynamicFields` with conserved snow accumulation/melt (freezing precipitation below
  thermal threshold and releasing liquid water cleanly on warm phases without violating
  `water_budget_residual` at `dynamic_fields.rs:400`) and dynamic shoreline/lake inundation synced to
  walkable terrain masks; leave pinned `WorldArtifact` (`world_artifact.rs:41-43, 67-82`) untouched and keep
  flag-off bit-identical (`WCHG-S01`).
- **`DONE WHEN`:**
  - `|water_budget_residual()| / water_scale < 1e-4` over `>= 10,000` ticks across freeze-thaw seasons;
    `TrackingAllocator` = `0` heap allocs/tick; `cargo fmt --check`, `cargo clippy --features desktop --all-targets -- -D warnings`,
    and `cargo test --features desktop` (`check_test_targets.mjs --profile desktop`) exit `0`.
  - **Negative control:** Melt `snow_depth` into `surface_water` without adjusting conserved snow store ->
    `water_budget_residual` exceeds `1e-4` and fails red.

---

## Card `W2` — Tier 2 Bounded `SurfaceDeltaOverlay` & Zero-Alloc Replay (`WCHG-S01`, `WCHG-S03`)

- **START WHEN:** `W1` merged and `D-WCHG-1` resolved.
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/world_artifact.rs` (effective-elevation helper `base + overlay`), `src-tauri/src/core/dynamic_fields.rs`
  - `src-tauri/src/core/resources.rs` (`sim_stream::WORLD_DISTURBANCE = 8` — next free stream slot beyond `1..=5` at `:925-948`, `V0` development, and `DORMANCY_STREAM = 7` at `aggregate_population.rs:190`)
  - `src-tauri/src/core/simulation_state.rs`, `src-tauri/src/core/snapshot.rs` (`SavedSimulationState` overlay serialization & conditional `world_checksum`)
  - `src-tauri/src/core/world_record/types.rs` (Tier-2 event log entries)
  - `src-tauri/tests/surface_delta_overlay_tests.rs` (new)
- **Scope:** Implement pre-allocated fixed-size `SurfaceDeltaOverlay` (`delta_elev_m`,
  `delta_water_stage`, `cover_disturbance`, dirty chunk bitset) capped per `D-WCHG-1`, dedicated PRNG
  stream `sim_stream::WORLD_DISTURBANCE`, deterministic snapshot/checksum gating, and `WorldRecord`
  disturbance recording (`0` tick allocations, legacy `WorldLawSet::baseline().fingerprint()` untouched).
- **`DONE WHEN`:**
  - `run(N) == run(K) -> save -> load -> run(N - K)` matches `world_checksum` and `SurfaceDeltaOverlay`
    buffers bit-for-bit; `TrackingAllocator = 0`; `WorldLawSet::baseline().fingerprint()` matches pre-card
    trunk verbatim (`WCHG-S01`).
  - **Negative controls:** (1) Omit `SurfaceDeltaOverlay` bytes from conditional `world_checksum` -> save/load
    tamper test fails red; (2) write tag byte inside `WorldLawSet::canonicalize()` when `world_change` is
    `None` -> legacy fingerprint test fails red.

---

## Card `W3` — Tier 2 Physical Disturbance Laws, `CauseId` Triggers & Renderer Patches (`WCHG-S03`)

- **START WHEN:** `W2` merged and `D-WCHG-2` / `D-WCHG-3` / `D-WCHG-5` decided.
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/crates/anima-domain/src/intervention.rs` (`GeologicalDisturbance` / wildfire / flood / fault `InterventionKind` variants if declared)
  - `src-tauri/src/core/world_record/{chain.rs, ring.rs}`, `src-tauri/src/core/scenario.rs` (exhaustive `InterventionKind`/`CompactInterventionKind` match arms in `chain.rs:61-118`, `ring.rs:203-248, 265-309`, `scenario.rs:205-283`)
  - `src-tauri/src/core/live_experiment.rs`, `src-tauri/src/core/environmental_systems.rs`, `src-tauri/src/core/ecology.rs`, `src-tauri/src/core/world_systems.rs` (`total_cost` organism disturbance load), `src-tauri/src/core/simulation_schedule.rs`
  - `src-tauri/src/core/emit.rs`, `src-tauri/src/commands/`, `src-tauri/src/lib.rs`, `docs/reference/IPC_CONTRACT.md`
  - `src/components/Landscape/WorldTerrainLod.tsx`, `src/components/Landscape/utils/` (dirty tile overlay patch)
  - `src-tauri/tests/schedule_ambiguity_tests.rs`, `src-tauri/tests/tier2_disturbance_tests.rs` (new)
- **Scope:** Implement physical local Tier-2 processes (wildfires, flash floods, slope landslides
  modulated by `DynamicFields::root_cover`, river bank avulsions, localized fault offsets, minor
  lava/tephra cones, animal engineering burrows/dams extending `DynamicFields::set_dam` at
  `dynamic_fields.rs:244`, plus optional coastal tsunami runup per `D-WCHG-3`), triggered naturally via
  `sim_stream::WORLD_DISTURBANCE` or host `CauseId` interventions, with scale router flagging threshold
  exceedances for Tier 3 and dirty-tile delta updates in `WorldTerrainLod` (`WorldTerrainLod.tsx:26-42`).
  Under default closed EU (`open_energy_flow = None`), burned biomass settles cleanly `Plants -> Detritus`
  (`|closed_total_eu - baseline| <= 1e-3`); when planned `ADR-0018` `OpenEnergyFlowLaw` is active,
  combustion heat exits via `EnergyEvent::RespiredHeatLoss` while ash enters `Detritus`/soil `N`/`organic`;
  any continuous animal/plant disturbance or tsunami saltwater stress routes strictly inside `total_cost`
  in `metabolic_decay_system` (`world_systems.rs:157, 174-177`).
- **`DONE WHEN`:**
  - Deforested steep cells fail at lower landslide trigger thresholds than vegetated `root_cover` controls;
    wildfires preserve closed-EU (`<= 1e-3`) and (when `ADR-0018` active) open-flow residuals (`<= 1e-3`),
    with organism disturbance loads verified inside `total_cost`; hazard sampling causes `0` drift on animal
    `SimRng`; `schedule_ambiguity_tests`, `npm run check:docs`, `npm run lint`, and `npm run build` pass.
  - **Negative controls:** (1) Draw natural hazard trigger from `SimRng` instead of `WORLD_DISTURBANCE` ->
    animal stream isolation test fails red; (2) allow unledgered fire deletion or charge disturbance stress
    outside `total_cost` -> conservation / `total_cost` gate fails red.

---

## Card `W4` — Dedicated `WorldRecordHeader::geological_era` Schema (`QD-BIO-D3`, `WCHG-S04`)

- **START WHEN:** `ADR-0016` accepted (implementable before or alongside `W3` since pure record schema work).
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/world_record/types.rs` (`geological_era`, `prev_geological_era_tail_hash`, `geological_era_artifact_sha256`, `era_transition_reason` on `:132-163`)
  - `src-tauri/src/core/world_record/` (header validation, era transition segment stitching, verify logic, `system.rs:557` struct literal)
  - `src-tauri/tests/world_record_format_and_immutability_tests.rs`, `src-tauri/tests/world_record_writer_tests.rs` (`:54-86` `sample_header` / `WorldRecordHeader` literal)
- **Scope:** Enforce `QD-BIO-D3` in code and tests: add backward-compatible `geological_era` fields while
  guaranteeing `WorldRecordHeader::era_index` (`types.rs:157`) remains strictly engine-compatibility only.
  Existing v1 `WorldRecord` files lacking `geological_era` deserialize cleanly as `geological_era = 0` /
  `None`.
- **`DONE WHEN`:**
  - `world_record_format_and_immutability_tests` & full desktop tests verify JSON/binary round-trip for
    old records (`geological_era == 0`) and multi-geological-era records where `era_index` stays constant (`0`).
  - **Negative control:** Simulate advancing `era_index` for a geological event with unchanged model version ->
    `WCHG-S04` invariant assertion fails red.

---

## Card `W5` — Offline `anima-worldgen` `WV2-2-PHYS`/`WV2-PHYS2` Era Re-Bake & Verification (`WCHG-S05`)

- **START WHEN:** `W3` + `W4` merged, and `WV2-PHYS2` (`MAP-D4`) landed on trunk.
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/crates/anima-worldgen/src/{pipeline.rs, tectonic.rs, config.rs, anmw_v3.rs}` (re-bake entry point taking prior terrain + accumulated `SurfaceDeltaOverlay` + volcanic/tectonic/sea-level forcing; volcanic & fault process physics)
  - `src-tauri/crates/anima-worldgen/src/bin/anima-worldgen.rs` (`era-rebake` CLI subcommand)
  - `src-tauri/crates/anima-worldgen/tests/worldgen_era_rebake_tests.rs` (new)
  - `src-tauri/tests/world_v2_sim_gate_tests.rs`
- **Scope:** Add strictly offline deterministic `anima-worldgen era-rebake` pipeline building on
  `run_worldgen_to_dir` (`pipeline.rs:587`), incorporating volcanic island cones/basalt flows, fault
  uplift/subsidence, sea-level shift, and coupled `step_fastscape_sd_dag` (`erosion_fluvial.rs:293`) +
  `deposition.rs`, emitting a new `AnmwV3Sim` (`anmw_v3.rs:106`) + updated `sim_world_sha256` and running
  `G-SHAPE` / `G-COMP v2` / `G-SIM` gates transparently (`D-WCHG-6` honest `gates.json` emission).
- **`DONE WHEN`:**
  - Offline re-bake produces bit-identical `AnmwV3Sim` across repeats on same OS from `(prior ANMW v3,
    SurfaceDeltaOverlay, forcing, seed)`; submarine volcanic vent exceeding threshold creates new subaerial
    (`h > 0`) island landmass passing `G-SIM` walkability & zero NaNs; zero terrain generation code linked
    into `build_tick_schedule` (`WCHG-S05`).
  - **Negative controls:** (1) Attempt calling `anima-worldgen` re-bake inside a live ECS tick system or fail
    to rotate `sim_world_sha256` -> compile/manifest mismatch gate fails red; (2) invert elevation forcing ->
    coastal drainage/walkability check detects topology shift honestly.

---

## Card `W6` — Cross-Era Organism Migration & Honest Geological Time UI (`WCHG-S06`)

- **START WHEN:** `W5` merged and `D-WCHG-4` resolved (migrating `DevelopedPhenotype` if `V0` is landed and `PlantGenotype` if `ADR-0015` flora is landed; otherwise preserving trunk morphology, brains, and lineages).
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/ecs.rs`, `src-tauri/src/core/snapshot.rs`, `src-tauri/src/core/simulation_state.rs`, `src-tauri/src/core/terrain_navigation.rs`, `src-tauri/src/core/world_artifact.rs`, `src-tauri/src/core/world_record/`
  - `src-tauri/src/core/live_experiment.rs`, `src-tauri/src/core/environmental_systems.rs` (safe post-era organism placement preserving saved phenotypes / `MorphologyGenotype`, `AgentBrain`, `AgentLineageId`, and `PlantGenotype` if active)
  - `src/` (geological era transition overlay banner disclosing time compression `"compressed geological time"`)
  - `src-tauri/tests/cross_era_migration_tests.rs` (new)
- **Scope:** Implement deterministic cross-era transition checkpointing: swap pinned `TerrainMap`/`WorldIdentity`
  cleanly between eras, apply `D-WCHG-4` survival/relocation rules for organisms on flooded/buried cells,
  preserve surviving `AgentBrain`, `AgentLineageId` / `LiveLineage`, saved phenotypes (`DevelopedPhenotype`
  under `V0` or trunk `MorphologyGenotype`, plus `PlantGenotype` under `ADR-0015`), and reset
  `SurfaceDeltaOverlay`, while surfacing clear geological time-compression disclosures in chronicle & UI.
- **`DONE WHEN`:**
  - Surviving organisms preserve exact genotypes, saved phenotypes, `AgentLineageId`, and `AgentBrain`
    weights across an era boundary; EU accounting reconciles displaced/lost organisms to baseline records
    without leak; same-OS multi-run cross-era `world_checksum` is deterministic.
  - **Negative control:** Re-run `decode_genotype` (or `develop_at_birth` when `V0` is active) during cross-era
    load -> `WCHG-S06` phenotype-immutability test fails red.

---

## Card `W7` — `AE-S12` Causal Chronicle & Multi-Seed Allopatric Speciation Suite (`WCHG-S07`)

- **START WHEN:** `W6` merged alongside `ADR-0013` sexual compatibility amendment (and `ADR-0015` flora if plant vicariance is included).
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/experiment_runner/`, `src-tauri/src/core/live_lineage.rs`
  - `src-tauri/tests/allopatric_speciation_tests.rs` (new)
  - `docs/ai/planning/` (experimental protocol / findings note)
- **Scope:** Record full `AE-S12` multi-hop causal traces (`event -> delta -> geographic_barrier -> dispersal_drop -> reproductive_isolation`), run paired same-seed ensembles comparing barrier/island-forming eras
  against continuous connected-mainland controls under `ER09` (`AE-S11` divergence threshold + `AE-S14`
  multi-seed paired effect report), and verify strict chronicle copy discipline (zero unverified `"species"` badges).
- **`DONE WHEN`:**
  - `>= 12`-seed paired ensemble isolates causal effect of geographic barrier vs continuous control with
    complete `AE-S12` parent chains in `CausalLedger`; `ER11` reports all completed & failed runs cleanly;
    `check_docs` & full `cargo test --features desktop` exit `0`.
  - **Negative controls:** (1) Enable post-barrier gene flow bridge across strait in control check ->
    reproductive isolation metric collapses relative to separated islands; (2) break `CauseId` chain or emit
    `"species"` without `AE-S11` pass -> verifier fails red.
