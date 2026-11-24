---
phase: planning
feature: evolved-communication
title: Evolved Communication & Local Perceptual Channels Implementation Cards (S1-S8 Per ADR-0017)
description: Sequenced implementation cards S1 through S8 covering evolved SIGNAL output capture, opt-in pheromone/olfactory wiring with total_cost accounting, local target perception in sensory_system and hrrl_learning_system, offline output-distance causal signal auditor, Baldwin telemetry, predator confusion, and Phase-2 acoustic/visual/plant-volatile channels
status: proposed
owner: anima-dell
last_reviewed: 2026-11-24
---

# Evolved Communication — Implementation Cards (`S1`–`S8`)

> **Planning document only (`status: proposed`, card `D18 SIGNALS-ADR`).**
> No Rust, TypeScript, or contract edits are performed here. All cards below wait for owner
> acceptance of [`ADR-0017`](../../decisions/ADR-0017-evolved-communication.md) (`D-SIG-1`–`D-SIG-6`),
> respect `RESEARCH_REVIEW_2026-11-24_modern_biology.md` Topic 5 (`S1`–`S6`) & §3 rejections, `QD-SIGNALS`,
> `QD-SCALE-TRIAL` (`N = 100` trial / `N = 1,000` design target), and any card adding new post-freeze `live.*`
> manifest keys or observables must obey merged `C12 E3B-PIN` (`#260`, `src-tauri/tests/prereg_e3b_manifest_tests.rs:84, 120, 574-603, 753-760`, `QD-BIO-D2`, and coordinate any fixed `LIVE_KEYS.len() == 31` assertions). Code anchors verified on
> `origin/feat/ecological-selection` at `e7b99745c59d7da35e884dd9414122b1d24fc648`.

---

## Dependency Overview

```text
S1 (Opt-in SignalOutputState reading action_index::SIGNAL = 7; 0 behavior change)
 └──► S2 (Opt-in PheromoneReleaser + OlfactorySensors attach/persist + total_cost emission cost + eavesdropping)
       ├──► S3 (Opt-in Local Target Vector in sensory_system & hrrl_learning_system + deterministic tie-break)
       │     ├──► S4 (Offline Output-Distance Causal & Shuffled-Signal Auditor; "signals" UI copy gate)
       │     └──► S7 (Phase 2: Bounded Zero-Alloc Acoustic Channel + SpatialHashGrid Range/Binaural)
       │           └──► S8 (Phase 2: Line-of-Sight Visual Displays, Kin Odour Signatures & Plant Wound Volatiles)
       └──► S6 (Opt-in Predator Confusion Multiplier in both combat_system branches)

S5 (Read-Only Baldwin Telemetry: AgentBrain.genotype vs AgentBrain.learned probe; independent post-ADR-0017)
```

---

## Card `S1` — Read `action_index::SIGNAL` into Optional `SignalOutputState` (`SIG-S01`)

- **START WHEN:** `ADR-0017` accepted (`D-SIG-1`; `C12 E3B-PIN` `#260` merged — any post-freeze `live.*` key obeys `prereg_e3b_manifest_tests.rs:84, 120, 574-603, 753-760` absent-means-legacy defaults).
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/components.rs` (`SignalOutputState` opt-in component beside `ActionGates` at `:501-548`)
  - `src-tauri/src/core/agent_systems.rs` (`action_resolution_system` at `:1911-1989`, wiring index `action_index::SIGNAL = 7` at `brain_genotype.rs:43` into `Option<&mut SignalOutputState>`)
  - `src-tauri/src/core/resources.rs`, `src-tauri/src/core/live_experiment.rs` (`CommunicationPolicy` opt-in toggle)
  - `src-tauri/src/core/simulation_state.rs` (`#[serde(default, skip_serializing_if = "Option::is_none")]` additive save field on `SerializedAgent` at `:105-145`)
  - `src-tauri/tests/signals_s1_wire_tests.rs` (new)
- **Scope:** Read the already-computed sigmoid output `response.actions[action_index::SIGNAL]` into an
  opt-in `SignalOutputState` component **without** altering locomotion, `ActionGates`, or legacy
  defaults (`CommunicationPolicy` absent/`!enabled` skips attachment and leaves `world_checksum` and
  all existing runs bit-identical, `0` tick allocs).
- **`DONE WHEN`:**
  - Legacy runs with policy absent match `e7b99745` bit-for-bit across `E2`/`E3`/`E3b` fixtures; enabled
    runs record finite `[0, 1]` `SIGNAL` output on evolved brains cleanly across snapshot restore with
    `TrackingAllocator = 0`; `cargo fmt --check`, `cargo clippy --features desktop --all-targets -- -D warnings`,
    and `cargo test --features desktop` exit `0`.
  - **Negative control:** Populate `SignalOutputState` unconditionally on legacy agents or mutate
    `ActionGates` when policy is off -> `SIG-S01` bit-identity test fails red.

---

## Card `S2` — Opt-In Pheromone Emission, `OlfactorySensors` Attachment & `total_cost` Sink (`SIG-S02`, `SIG-S04`, `SIG-S05`)

- **START WHEN:** `S1` merged (`D-SIG-1`, `D-SIG-2`).
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/ai/pheromone.rs` (`PheromoneGrid`, `OlfactorySensors`, `PheromoneReleaser`, `agent_release_pheromone_system`, `agent_read_pheromone_system` at `:5-235`; modulated by `SignalOutputState` + `ActionGates::of(gates).pheromone_scale()` `:176`)
  - `src-tauri/src/core/world_systems.rs` (`metabolic_decay_system` `:85-225`: add signal emission metabolic surcharge inside `total_cost` at `:157` -> `respired` -> `pool.detritus` `:174-177, 222-224`; migration `AgentMigrationData` assembly `:728, 905` and `SpawnMigrationCommand` `:980-1121`)
  - `src-tauri/src/core/spawn.rs` (`:139-143`), `src-tauri/src/core/simulation_loop.rs` (`:1680-1706`), `src-tauri/src/core/live_experiment.rs` (`:3108-3133`), `src-tauri/src/core/agent_systems.rs` (`SpawnOffspringCommand` `:494-652`) (opt-in attachment of `PheromoneReleaser` & `OlfactorySensors` at genesis and birth)
  - `src-tauri/src/core/simulation_state.rs` (`SerializedAgent` `:105-145`, `spawn_serialized_agent` `:922-1039`), `src-tauri/src/core/components.rs` (`AgentMigrationData` `:166-185`), `src-tauri/src/core/aggregate_population.rs` (`ArchivedIndividual` `:224-236`), `src-tauri/src/core/snapshot.rs` (`world_checksum` `:789-886`, agent tuple loop `:798-822` conditional hashing)
  - `src-tauri/tests/signals_s2_pheromone_eu_tests.rs` (new)
- **Scope:** Wire `PheromoneReleaser` and `OlfactorySensors` at genesis and offspring birth strictly under
  opt-in flag (`0` attachment when off), carry components verbatim across all four persistence boundaries
  (restore, migration, `DormantAgentRecord` wake at `agent_systems.rs:205-208`, `ArchivedIndividual`
  rehydration — invariant `D01`/`D02`/`D09`), and charge metabolic emission cost **strictly inside
  `total_cost`** (`metabolic_decay_system` at `world_systems.rs:157`). Confirm predators and prey both
  read public `PheromoneGrid` concentration via bilateral `OlfactorySensors` (`state_arr[13..=14]`,
  enabling predator eavesdropping).
- **`DONE WHEN`:**
  - Nonzero chemical emission increases `respired` and preserves `|closed_eu_residual| / initial <= 1e-9`
    (and `<= 1e-3` under [`ADR-0018`](../../decisions/ADR-0018-ecosystem-scales-and-cycles.md) (`proposed`, `#263` merged) open flow); `run(N) == run(K) -> save -> load -> run(N-K)`
    matches `world_checksum` bit-for-bit when enabled; `0` tick heap allocs; `0` change when flag off.
  - **Negative controls:** (1) Deduct emission cost outside `total_cost` -> EU closure fails red (`SIG-S02`);
    (2) drop `OlfactorySensors` across snapshot save -> post-restore `world_checksum` or `SIG-S04` round-trip test fails red.

---

## Card `S3` — Opt-In Local Target Vector (`sensory_system` & `hrrl_learning_system`, `SIG-S03`, `SIG-S05`)

- **START WHEN:** `S2` merged and `D-SIG-3` decided.
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/agent_systems.rs` (`sensory_system` `:1633-1881`, specifically `:1677-1688, 1744-1772, 1824-1846`)
  - `src-tauri/src/ai/model.rs` (`hrrl_learning_system` `:1082-1350`, specifically `:1181-1270`; shared helper reused cleanly without duplication)
  - `src-tauri/src/physics/spatial.rs` (bounded zero-alloc neighborhood query helper using pre-allocated scratch if needed, extending `:31-427`)
  - `src-tauri/src/core/resources.rs`, `src-tauri/src/core/live_experiment.rs` (`live.local_perception` / `CommunicationPolicy` flag, preserving `SENSORY_SLOTS = 15` `:1484`)
  - `src-tauri/tests/signals_s3_local_perception_tests.rs` (new)
- **Scope:** Implement one shared zero-allocation helper `compute_local_target_vec(...)` replacing the
  omniscient global scan in **both** `sensory_system` and `hrrl_learning_system` ONLY when
  `local_perception = true` (`SpatialHashGrid` neighborhood within `R_view` + optional cone/olfactory
  fallback per `D-SIG-3`, deterministic `(dist_sq, x, z)` tie-break). **Never** add an assertion
  banning the legacy global vector when disabled (keeps `E2`/`E3`/`E3b` bit-identical).
- **`DONE WHEN`:**
  - When enabled, prey/food placed strictly beyond `R_view` produce `local_target_vec == Vec3::ZERO`
    while targets inside `R_view` break ties identically regardless of ECS spawn/restore order;
    `sensory_system` and `hrrl_learning_system` return bit-identical target vectors on matching state;
    `TrackingAllocator = 0`; legacy flag-off mode stays bit-identical to `e7b99745`.
  - **Negative control:** Inject an isolated food node at distance `R_view + 20.0` (with local perception
    on) -> asserting `local_target_vec == Vec3::ZERO` passes cleanly and fails red if global scan leaks.

---

## Card `S4` — Offline Output-Distance Causal Auditor & Shuffled-Signal Controls (`SIG-S06`)

- **START WHEN:** `S3` merged and `D-SIG-6` threshold defined.
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/experiment_runner/` or `src-tauri/src/ai/signal_audit.rs` (new purely offline analysis module — zero imports into `build_tick_schedule`)
  - `src-tauri/src/core/live_experiment.rs` (`LIVE_EXTENDED_OBSERVABLE_IDS` `:1632-1649` if summary observability metrics are exposed obeying merged `C12 E3B-PIN` `#260`, `prereg_e3b_manifest_tests.rs:120, 753-760`)
  - `src-tauri/tests/signals_s4_causal_auditor_tests.rs` (new)
  - `docs/ai/planning/` (measurement findings note if warranted)
- **Scope:** Implement an offline (zero-tick-cost) analyzer over snapshot/checkpoint captures computing
  binned mutual information `I(Signal; Context)` and **output-distance receiver counterfactual response**
  (`||forward(real_percept) - forward(time_shuffled_percept)||_2` and sender-shuffled null baseline)
  matching deterministic sigmoid policies (`brain_genotype::forward_into`), verifying empirical signal
  meaning before any UI `"language"` wording is allowed (`D-SIG-6`).
- **`DONE WHEN`:**
  - Synthetic sender-receiver test fixture yields statistically significant mutual information & causal
    action shift above time-shuffled/sender-shuffled controls; random noise fixture fails cleanly to reject
    null; `0` symbols added to `build_tick_schedule` (`schedule_ambiguity_tests` untouched and `EXIT=0`).
  - **Negative controls:** (1) Pass time-shuffled or noise signals into auditor -> reports null /
    non-significant causal effect (fails red if treated as valid communication); (2) emit `"language"`
    UI label while auditor reports null -> copy lint fails red.

---

## Card `S5` — Read-Only Baldwin Plasticity Telemetry (`AgentBrain.genotype` vs `learned`)

- **START WHEN:** Independent (depends only on `ADR-0017` acceptance; any post-freeze `live.*` metrics obey merged `C12 E3B-PIN` `#260`, `prereg_e3b_manifest_tests.rs:84, 120, 574-603, 753-760`).
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/telemetry_export.rs`, `src-tauri/src/core/live_experiment.rs` (offline/interval read-only comparison probe over `AgentBrain.genotype` vs `AgentBrain.learned` at `components.rs:561-612`)
  - `src-tauri/tests/signals_s5_baldwin_telemetry_tests.rs` (new)
- **Scope:** Evaluate `AgentBrain::genotype` and `AgentBrain::learned` across a fixed standardized
  sensory probe array when `LifetimeLearning` (`resources.rs:832-847`) is active, exporting
  phenotypic plasticity divergence metrics purely read-only without perturbing simulation state or
  mutating `genotype` (non-Lamarckian invariant).
- **`DONE WHEN`:**
  - Probe difference is exact `0.0` when `learned.is_none()` and finite positive when `learned.is_some()`;
    world trajectories with telemetry active vs inactive match bit-for-bit; `cargo test --features desktop` passes.
  - **Negative control:** Perturb `AgentBrain::genotype` weights inside telemetry probe -> immutability
    checksum check fails red.

---

## Card `S6` — Opt-In Predator Confusion Multiplier in Dual `combat_system` Branches (`SIG-S07`)

- **START WHEN:** `S2` merged (`D-SIG-5` decided).
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/core/world_systems.rs` (`combat_system` at `:366-588`, applying the shared confusion helper in **both** event branch `:461-531` and non-event branch `:532-587`)
  - `src-tauri/src/physics/spatial.rs` (neighbor prey count helper via `SpatialHashGrid`, `0` heap allocs)
  - `src-tauri/src/core/resources.rs`, `src-tauri/src/core/live_experiment.rs` (opt-in confusion toggle & parameter)
  - `src-tauri/tests/signals_s6_predator_confusion_tests.rs` (new)
- **Scope:** Apply opt-in group-confusion attenuation factor to `predation_capture` when multiple prey
  (modulated by evolved `SIGNAL`/grouping density) cluster near the strike target, guaranteeing bit-exact
  parity between `CombatEvents = Some(...)` (`world_systems.rs:403-531`) and `None` (`:532-587`), `0` tick
  allocations, and bit-identity when disabled.
- **`DONE WHEN`:**
  - Single-prey strike matches baseline; dense multi-prey cluster reduces `captured` monotonically per
    formula while maintaining EU conservation (`removed - gained` -> `detritus`); `CombatEvents` `Some` vs
    `None` yields exact same world state; `0` tick allocs.
  - **Negative control:** Omit confusion call in `:532-587` (`CombatEvents = None`) while calling it in
    `:461-531` -> dual-branch parity test fails red (`SIG-S07`).

---

## Card `S7` — Phase 2: Bounded Zero-Alloc Acoustic Channel & `ObserverPolicy::Inhabit` Parity (`SIG-S05`, `SIG-S08`)

- **START WHEN:** `S1`–`S4` merged, `D-SIG-4` resolved, and (`SpatialHashGrid` benchmark on `N = 100` & `N = 1,000` headroom confirmed).
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/ai/acoustic.rs` (new bounded pre-allocated `AcousticEventBuffer` resource, deterministic emitter cap, inverse-square directional sampling over `SpatialHashGrid`)
  - `src-tauri/src/core/simulation_schedule.rs`, `src-tauri/src/core/world_systems.rs` (`total_cost` acoustic cost inside `metabolic_decay_system:157`), `src-tauri/src/core/observer.rs` (`ObserverPolicy::Inhabit` routing guest calls through physical effectors, never bypassing cost/range)
  - `src-tauri/src/core/simulation_state.rs`, `src-tauri/src/core/snapshot.rs`, `src-tauri/src/core/components.rs`, `src-tauri/src/core/aggregate_population.rs`
  - `src-tauri/tests/schedule_ambiguity_tests.rs`, `src-tauri/tests/signals_s7_acoustic_tests.rs` (new)
- **Scope:** Add fast, range-limited, directional acoustic signaling channel and wire `ObserverPolicy::Inhabit`
  guest signaling inputs identically to AI organism actuators (same `total_cost`, same eavesdropping,
  human guest chat kept strictly out-of-world). Add dedicated `acoustic_system` slot cleanly inside
  `build_tick_schedule` (`simulation_schedule.rs:121-203`) with `0` ambiguous pairs and `0` tick allocations.
- **`DONE WHEN`:**
  - `schedule_ambiguity_tests` passes `0` ambiguous pairs; acoustic emission adds metabolic cost inside
    `total_cost`; distant/out-of-range calls read `0.0`; predators eavesdrop identically on acoustic calls;
    `ObserverPolicy::Inhabit` guest pays identical metabolic cost as AI emitters; `TrackingAllocator = 0`.
  - **Negative control:** Allow guest `Inhabit` emitter to skip `total_cost` or exceed `R_ac` -> observer
    parity gate fails red (`SIG-S08`).

---

## Card `S8` — Phase 2: Line-of-Sight Visual Displays, Kin Odour Signatures & Plant Wound Volatiles (`SIG-S02`, `SIG-S06`, `SIG-S08`)

- **START WHEN:** `S7` merged (and `ADR-0015` flora cards `P1`–`P3` merged for herbivory plant volatiles).
- **Proposed `ALLOWED PATHS`:**
  - `src-tauri/src/ai/pheromone.rs`, `src-tauri/src/core/environmental_systems.rs` (plant herbivory wound-volatile emission within plant metabolic/`total_cost` budget; genotype-derived heritable odour signature bands per `D-SIG-2`)
  - `src-tauri/src/physics/spatial.rs`, `src-tauri/src/core/agent_systems.rs` (raycast-gated `DisplayState` line-of-sight perception)
  - `src-tauri/src/core/world_systems.rs` (`total_cost` display cost), `src/` (read-only UI panels using strictly `"signals"` labels until `D-SIG-6` empirical threshold is cleared)
  - `src-tauri/tests/signals_s8_visual_plant_tests.rs` (new)
- **Scope:** Deliver raycast/line-of-sight gated visual display signals, heritable chemical odour cues
  replacing any oracle kin desire ([`ADR-0013`](../../decisions/ADR-0013-live-creature-lifecycle.md) `accepted` `#262` `D1 = B` sexual mate choice without oracle kin slots), and grazing-induced
  plant chemical wound volatiles (`ADR-0015` inter-kingdom signaling). Enforce strict UI copy discipline
  (`"signals"` until `SIG-S06` passes `D-SIG-6`).
- **`DONE WHEN`:**
  - Occluded visual displays behind obstacles read `0.0` while unobstructed displays vary perception cleanly;
    grazed plants trigger finite volatile plumes billed cleanly inside energy accounting; UI lint passes with
    zero unverified `"language"` claims; `check_docs`, `lint`, `build`, and full `cargo test --features desktop` exit `0`.
  - **Negative control:** Inject an opaque collider between display emitter and receiver -> line-of-sight
    assertion fails red if display signal still triggers receiver input.
