---
phase: planning
feature: ecosystem-scales-and-cycles
title: Ecosystem Scales & Open Energy Flow Cards (E1-E7 Per ADR-0018)
description: Sequenced implementation cards E1 through E7 covering BENCH-ENTITIES, open solar and heat-loss EnergyLedger flow, allometric pace, density-dependent cap retirement, and swarm or field organism tiers
status: proposed
owner: anima-dell
last_reviewed: 2026-11-24
---

# Ecosystem Scales, Cycles & Open Energy Flow — Implementation Cards (`E1`–`E7`)

> **Planning document only (`status: proposed`, card `D19 ECOSYSTEM-ADR`).**
> No Rust, TypeScript, or [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../../reference/EVOLUTION_EXPERIMENT_CONTRACT.md)
> code/contract edits are performed here. Every card below waits for owner acceptance of
> [`ADR-0018`](../../decisions/ADR-0018-ecosystem-scales-and-cycles.md) (`QD-ECOSCALE-ACCEPT`,
> `D-ECO-1`–`D-ECO-6`), respects `QD-REALMS` ([`AGENT_WORKFLOW.md`](../../how-to/AGENT_WORKFLOW.md) `:320-323`),
> and begins in Lane W strictly **after Island Milestone 1 cards (`I1`–`I11`)** except read-only
> observability/benchmark work (`E1`, per `MSG_20261124_133000`).
> All trunk anchors are verified on `origin/feat/ecological-selection` at
> `facc800c50a15cdca960c8c006cc5f861e4c703a`.

---

## Dependency Overview

```text
E1 (BENCH-ENTITIES: 500 & 2,000 sessile entities + swarm pool baseline)
 └──► E2 (OpenEnergyFlowLaw, EnergyLedger solar/heat tracking + ER04 contract amendment)
       ├──► E3 (Solar LUE primary production & total_cost heat dissipation sink)
       │     └──► E4 (Kleiber/lifespan allometric pace & opt-in density-dependent cap retirement)
       │           ├──► E5 (Tier 3 Population Fields: decomposers & realm-gated larvae)
       │           └──► E6 (Tier 2 Swarms / Super-Individuals pool; gated by E1 headroom)
       └──► E7 (Read-only TTE/NPP/lake-refill observables & V10 multi-seed ecosystem soak)
```

*Cross-dependencies:* `E3` couples solar LUE with `V5`'s moisture/N/temperature/shade limiters ([`2026-11-24-v5-fields-plants-feedback-spec.md`](2026-11-24-v5-fields-plants-feedback-spec.md)); `E4` + `E5` consume `V4` carcass decomposition ([`2026-11-24-v4-carcasses-decomposition-spec.md`](2026-11-24-v4-carcasses-decomposition-spec.md)); `E5`/`E6` follow `QD-REALMS` order (`water -> sky -> underground`) before instantiating aquatic/aerial organisms.

---

## Card `E1` — `BENCH-ENTITIES` (500 & 2,000 Sessile Entities + Swarm Pool Scaling Benchmark)

- **START WHEN:** `ADR-0018` accepted (`BENCH-ENTITIES` may also be authorized standalone as non-behavioral Tier A benchmark work, `RESEARCH_REVIEW` `A2`).
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/benches/live_schedule_scaling.rs` (`:32`, `:58-102`)
  - `docs/paper/data/scaling.md` *(append benchmark run only; never invent µs numbers)*
- **Scope:** Extend Criterion benchmark harness `live_schedule_scaling` (`f4614665` baseline: `5.643 ms p50 @ N = 1,000` in [`docs/paper/data/scaling.md`](../../paper/data/scaling.md) `:39, 72`) with realistic mixed workloads (`500` and `2,000` sessile plant/Tree entities plus a bounded swarm array/spatial sweep) on `LiveExperimentAdapter` / `build_tick_schedule`. No simulation logic changes.
- **DONE WHEN:**
  - `cargo bench --bench live_schedule_scaling -- --quick` (or full release bench) exits `0`;
  - Raw Criterion `estimates.json` `p50` / `p95` recorded in [`docs/paper/data/scaling.md`](../../paper/data/scaling.md) with machine specs (`CPU`, `rustc -V`, commit SHA);
  - `git diff --stat` contains strictly `live_schedule_scaling.rs` and `scaling.md`; `cargo fmt --check`, `cargo clippy --features desktop --all-targets -- -D warnings`, `check:docs` all exit `0`.
- **Negative Control:** Confirm running the existing `AGENT_COUNTS = [10, 100, 1_000]` cohort still populates all baseline fixtures with `live.agent_count == N` invariant before and after Criterion samples (harness fails if entities leak across ticks).

---

## Card `E2` — Open-Flow `EnergyLedger`, `WorldLawSet::open_energy_flow`, and `ER04` Contract Amendment

- **START WHEN:** `ADR-0018` accepted (`QD-ECOSCALE-ACCEPT`, `D-ECO-1`, `D-ECO-6`) + `E1` baseline recorded (or parallel after acceptance).
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/crates/anima-domain/src/energy.rs` (`:19-150, :375-377`)
  - `src-tauri/crates/anima-domain/src/lib.rs` (exports if needed)
  - `src-tauri/src/core/energy_ledger.rs` (`:11-54`)
  - `src-tauri/src/core/experiment/laws.rs` (`:28-90`)
  - `src-tauri/src/core/environmental_systems.rs` (`ecosystem_census_system` `:178-229`)
  - `src-tauri/tests/energy_conservation_tests.rs` (new/extended tests)
  - `docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md` (`:121-124` verbatim draft from `ADR-0018` §2.5)
- **Scope:** Add opt-in `OpenEnergyFlowLaw` (`Option`, default `None`) to `WorldLawSet` + `fingerprint()` (`laws.rs:79`); extend `EnergyLedger` with `cum_solar_in: f64`, `cum_heat_out: f64`, `EnergyEvent::SolarPrimaryProduction`, `EnergyEvent::RespiredHeatLoss`, and open-residual formula `R_open(t)` (`|R_open| <= RESIDUAL_ABS_TOLERANCE_EU = 1e-3`); update `EVOLUTION_EXPERIMENT_CONTRACT.md:121-124` per `ADR-0018` §2.5. Keep `open_energy_flow = None` 100% bit-identical with `0` extra RNG draws.
- **DONE WHEN:**
  - `cargo test --features desktop --test energy_conservation_tests` + full `cargo test --features desktop` + `node scripts/check_test_targets.mjs ... --profile desktop` pass;
  - Determinism & `open_energy_flow = None` bit-identity tests pass (`world_checksum` exact match vs trunk);
  - `npm run check:docs`, `cargo fmt --check`, `cargo clippy` pass.
- **Negative Control:** Inject an undeclared plant credit or omit recording `cum_solar_in` / `cum_heat_out` cleanly in a dedicated test -> `R_open(t)` exceeds `RESIDUAL_ABS_TOLERANCE_EU` (`1e-3`) and assertion **fails** loudly; toggle `open_energy_flow` fingerprint test must differ from `WorldLawSet::baseline().fingerprint()`.

---

## Card `E3` — Solar LUE Primary Production & `total_cost`-Routed Heat Sink

- **START WHEN:** `E2` and `V5` ([`2026-11-24-v5-fields-plants-feedback-spec.md`](2026-11-24-v5-fields-plants-feedback-spec.md)) are merged on trunk; `QD-BIO-D2` (E3b freeze-pin update at `src-tauri/tests/prereg_e3b_manifest_tests.rs:417-426, 584-588`) resolved before touching `LIVE_KEYS`.
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/world_systems.rs` (`metabolic_decay_system` `:85-225`)
  - `src-tauri/src/core/environmental_systems.rs` (`resource_field_regrowth_system` `:35-90`, `fruit_growth_system` `:6-28`)
  - `src-tauri/src/core/ecology.rs` (`:331-494`, adding LUE open-flow step while keeping `step_regrowth_gated_strided` intact; updating comment `:153-155` cleanly)
  - `src-tauri/src/core/live_experiment.rs` (`LIVE_KEYS`, `LiveWorldConfig`, open-flow resource wiring)
  - `src-tauri/src/core/simulation_state.rs` (`SavedSimulationState`, `serialize_world_state`, `restore_energy_state` `:459, :863, :1151`) & `src-tauri/src/core/snapshot.rs` (`cum_solar_in` / `cum_heat_out` persistence)
  - `src-tauri/tests/open_energy_flow_tests.rs` (new; keep single-`#[test]` binary or `ThreadTrackingAllocator` per `tests/common/allocator.rs` if using `#[global_allocator]`)
  - *(plus `prereg_e3b_manifest_tests.rs` coordination if `QD-BIO-D2` has not already moved to a fixed freeze-time key list)*
- **Scope:** When `OpenEnergyFlowLaw` is active:
  1. Route `respired * heat_loss_fraction` in `metabolic_decay_system` (`world_systems.rs:174-177, 222-224`) to `HeatLoss` (`cum_heat_out`) rather than `pool.detritus += respired` (all metabolic charges stay strictly inside `total_cost` at `:157`);
  2. Drive producer growth via incident sunlight × LUE × `V5` moisture/N/temp/shade limiters crediting `Plants` via `cum_solar_in` (reading exact `f32 -> f64` cell delta);
  3. Persist open-flow counters across save/load (`run N == run K -> save -> load -> run N-K`).
- **DONE WHEN:**
  - `cargo test --features desktop --test open_energy_flow_tests`, `schedule_ambiguity_tests`, and full `cargo test --features desktop` pass (`0` allocations on hot tick via `TrackingAllocator` in isolated single-`#[test]` binary / `ThreadTrackingAllocator`);
  - `flag off` (`open_energy_flow = None`) equals baseline trunk `world_checksum` bit-for-bit across E3b/E2 seeds.
- **Negative Control:**
  - Set `I_solar = 0.0` (darkness test) under `open_energy_flow = Some(...)`: NPP drops to `0`, plant biomass stops recovering, and total world EU decays monotonically via `cum_heat_out` while `|R_open| <= 1e-3`;
  - Attempt a direct subtraction outside `total_cost` in regression check -> conservation check detects discrepancy between `sum(decay)` and `cum_heat_out` and fails.

---

## Card `E4` — Kleiber / Lifespan Allometric Pace & Density-Dependent Cap Retirement

- **START WHEN:** `E3` and `V3` (`ADR-0013`) + `V4` (`CarcassPool`) are merged on trunk.
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/crates/anima-domain/src/laws.rs` (`KLEIBER_EXP` `:40`, allometric rate/timescale helpers)
  - `src-tauri/src/core/agent_systems.rs` (`autonomous_reproduction_system` `:694-744, :989-998`)
  - `src-tauri/src/core/resources.rs` (`PreyCapPolicy`, allometric pace policy)
  - `src-tauri/src/core/live_experiment.rs` (opt-in `live.allometry`, `live.density_cycles` keys)
  - `src-tauri/tests/allometry_and_density_tests.rs` (new)
- **Scope:** Implement opt-in `M^0.75` / `M^beta` (`D-ECO-2`) allometric scaling of lifespan, maturation, and cooldowns anchored cleanly to `1–2 h` real time for reference inhabitable animals (microfauna living minutes; flora spanning days/eras), and introduce explicit density-dependent regulation & opt-in retirement of `live.prey_cap` (`80`) and predator `pred_count >= 8 || prey_count < pred_count * 5` ceilings (`agent_systems.rs:694-699, :990-996`). Default `PreyCapPolicy` stays enabled (`80`) whenever opt-in flags are absent.
- **DONE WHEN:**
  - `cargo test --features desktop --test allometry_and_density_tests` verifies analytic `M^0.75` and timescale scaling parity (`<= 1e-6` relative), zero hot-tick allocations, and uncapped prey growth above `80` when resources allow followed by natural resource/predation pullback;
  - Trunk default runs with flags off remain bit-identical (`world_checksum` exact match).
- **Negative Control:**
  - Setting `live.prey_cap = 80.0` with opt-in `live.density_cycles` disabled **stops** prey count at `<= 80 + tick headroom` (matching `strict_prey_cap_tests.rs`); enabling `live.density_cycles` under abundant food breaks above `80` (fails if cap remains hardcoded).

---

## Card `E5` — Tier 3 Population Fields (`Decomposers` & Realm-Gated `Larvae` / Micro-Guilds)

- **START WHEN:** `E4` (`V4` + `V5` integrated) merged on trunk; any water/air larval/plankton cells strictly wait until the corresponding **`QD-REALMS`** ADR (`water` first, then `sky`, [`AGENT_WORKFLOW.md`](../../how-to/AGENT_WORKFLOW.md) `:320-323`) lands.
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/population_fields.rs` *(new)*
  - `src-tauri/src/core/mod.rs`
  - `src-tauri/src/core/environmental_systems.rs` & `src-tauri/src/core/simulation_schedule.rs`
  - `src-tauri/src/core/simulation_state.rs`, `src-tauri/src/core/snapshot.rs` & `src-tauri/src/core/live_experiment.rs`
  - `src-tauri/tests/population_fields_tests.rs` *(new; single-`#[test]` binary or `ThreadTrackingAllocator` per `tests/common/allocator.rs`)*
- **Scope:** Add zero-alloc per-cell `PopulationField` resource storing biomass density + evolving trait distribution bins/moments across soil decomposers/fauna (coupled with `V4` carcass / `V5` soil mineralization) and subsequent realm-unlocked plankton/larval fields. Selection + neighbor cell dispersal act on the distribution using a dedicated RNG stream (`sim_stream`, never touching ecology/genetics streams); measured `f32 -> f64` cell deltas feed `EnergyLedger`.
- **DONE WHEN:**
  - `TrackingAllocator == 0` (single-`#[test]` binary / `ThreadTrackingAllocator`) on tick step; `schedule_ambiguity_tests` = `0` ambiguous pairs; `3`-process same-OS determinism exact match; `|R_open| <= 1e-3`; save/load restores exact trait distributions & biomass; flag off = bit-identical to pre-`E5` trunk.
- **Negative Control:** Mutating dispersal/selection disabled or pinning a synthetic trait disadvantage collapses that trait bin vs control; removing ledger sync triggers `|R_open| > 1e-3` failure.

---

## Card `E6` — Tier 2 Swarms / Super-Individuals (`SwarmPool`)

- **START WHEN:** `E1` (`BENCH-ENTITIES` headroom verified for swarm budget), `E4`, and `E5` merged on trunk; aerial/aquatic swarm behaviors strictly honor `QD-REALMS` ordering.
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/swarm_pool.rs` *(new, pre-allocated `Vec::with_capacity` mirroring `V4` `CarcassPool`)*
  - `src-tauri/src/core/mod.rs`
  - `src-tauri/src/core/simulation_schedule.rs`, `src-tauri/src/core/world_systems.rs`, `src-tauri/src/core/snapshot.rs`
  - `src-tauri/src/core/simulation_state.rs` (lightweight swarm state & DTOs; frontend stays passive)
  - `src-tauri/tests/swarm_pool_tests.rs` *(new; single-`#[test]` binary or `ThreadTrackingAllocator` per `tests/common/allocator.rs`)*
- **Scope:** Represent cohesive insect swarms / social colonies / schools as bounded super-individuals (`count`, `position`, `velocity`, `eu_reserve: f64`, heritable `trait_distribution`), foraging or biting hosts cleanly through `debit_reserve` -> `SwarmPool` reserve (`0` leaked EU; optional vector/irritation cost billed inside host `total_cost`). Milestone 1 allows watching swarms, **no** `Inhabit` (`D-ECO-5`).
- **DONE WHEN:**
  - `TrackingAllocator == 0` (single-`#[test]` binary / `ThreadTrackingAllocator`) over tick updates + swarm spawn/merge/bite/despawn inside pre-allocated capacity; `|R_open| <= 1e-3`; multi-seed determinism + save/load equivalence exact; `live_schedule_scaling` p50 stays inside the `E1` frame target.
- **Negative Control:** Exceeding pool cap settles overflow cleanly into detritus/death without panicking or allocating; disabling swarm intake prevents host energy loss and prevents swarm growth.

---

## Card `E7` — Ecosystem Telemetry (`realized_tte`, `npp`, `lake_refills`) & `V10` Multi-Seed Soak

- **START WHEN:** `E3`–`E6` (or initial `E3`–`E4` + `V10` milestone) merged on trunk and `QD-BIO-D2` live-key pin resolved.
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/live_experiment.rs` (additive observables: `live.realized_tte`, `live.npp`, `live.lake_refills`, `RESEARCH_REVIEW` `A3`)
  - `src-tauri/tests/v10_soak_leak_tests.rs` / `src-tauri/tests/ecosystem_telemetry_tests.rs`
- **Scope:** Add read-only ecosystem flux observables (`realized_tte`, `npp` from `EnergyFluxMatrix` + `cum_solar_in`, and `lake_refills` tracking unconditional vs hydrology-coupled refills) with checksum-identical tests verifying observables never alter world trajectories (`RESEARCH_REVIEW` `A3` / §3 rule); run multi-seed [`V10`](2026-11-24-v10-soak-leak-spec.md) headless soak covering individuals + fields + swarms + open energy flow (`allocs == 0`, handle/entity bounds, `|R_open| <= 1e-3`, honest crash/boom logging, no unverified species claims before `AE-S11`/`AE-S14`).
- **DONE WHEN:**
  - Checksum equality holds with telemetry enabled vs disabled; multi-seed soak passes all `V10` leak & conservation assertions; `cargo test --features desktop` + `check_test_targets --profile desktop` pass cleanly.
- **Negative Control:** Toggling `open_energy_flow = None` keeps legacy observables identical; forcing an artificial energy sink fails soak `|R_open| <= 1e-3`.
