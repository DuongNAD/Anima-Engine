---
phase: planning
feature: island-evolution-observatory
title: Preregistration Draft — REPLAY-1: Replaying Island Worlds from a Checkpoint (QD-ISLAND)
status: proposed
owner: anima-dell
last_reviewed: 2026-11-26
description: Body-only preregistration draft for REPLAY-1 under QD-FOCUS FOCUS-3 and QD-ISLAND, replaying island worlds from a single checkpoint to test whether island-rule outcomes repeat and diverge under a changed factor, documenting LiveExperimentAdapter checkpoint anchors, missing code cards, disjoint seeds 1110001..=1179001, and open owner slots REPLAY1-D1..D5.
---

# Preregistration Draft — `REPLAY-1`: Replaying Island Worlds from a Checkpoint (`QD-ISLAND`)

> **PROPOSED DRAFT (`status: proposed` — `0` CODE CHANGES, `0` MANIFESTS WRITTEN, `0` SEEDS EXECUTED).**
> - **Governing Mandate (`QD-FOCUS FOCUS-3`, PR `#314` / `MSG_20261126_030000`):** Under [`QD-FOCUS`](../../how-to/AGENT_WORKFLOW.md) and the owner's 2026-11-26 answer (**`"Phục vụ QD-ISLAND"`**), **`QD-ISLAND` stays the project's flagship scientific question** ([`2026-11-19-feature-island-evolution-observatory.md`](../requirements/2026-11-19-feature-island-evolution-observatory.md)), and **`REPLAY-1` serves `QD-ISLAND`** by replaying island worlds from one checkpoint (`SEL2-P` / `SEL2-P2` is `QD-ISLAND`'s first brick).
> - **Framing Decided — Remaining Slots Empty (`Rule L4`):** The framing (**replays of island worlds serving `QD-ISLAND`**) is **decided** and is NOT an owner slot. Section 5 leaves all **five** remaining owner decisions (`REPLAY1-D1`..`REPLAY1-D5`: the changed factor, `N` and power method, divergence metric, run length, and platform) explicitly empty (`OWNER DECISION REQUIRED`). Zero code edits, manifests, calibration runs (`1180001..=1180012`), smoke (`999907`), or confirmation seeds may run until the owner decides `REPLAY1-D1`..`REPLAY1-D5` and prerequisite code cards land.
> - **Rule L7 Status:** Code PRs `#308` (`WV2-BIG-2`, `3b78b4b8`) and `#311` (`WV2-BIG-1`, `1b988167`) merged into `origin/feat/ecological-selection` after the previous planning document.

---

## 0. Decided Framing, Scientific Question & Provenance

### 0.1 Scientific Question (`MSG_20261126_030000`, `QD-ISLAND`)

From one checkpoint of an island world:
1. **Does the island-rule outcome** (body-size shift under resource limitation vs. predation release, anchored to [`2026-11-19-feature-island-evolution-observatory.md`](../requirements/2026-11-19-feature-island-evolution-observatory.md) §1–§4) **repeat across replays?**
2. **And does one changed factor move it more than the replays differ among themselves?**

### 0.2 Governing Rules & Prior Bricks

| Rule / Document | Reference | Role in `REPLAY-1` |
|---|---|---|
| **`QD-ISLAND` (flagship question, `2026-11-19`)** | [AGENT_WORKFLOW.md §4](../../how-to/AGENT_WORKFLOW.md) & [2026-11-19-feature-island-evolution-observatory.md](../requirements/2026-11-19-feature-island-evolution-observatory.md) | Tests the mechanism of the island rule (`H1`, `H1-R` resource limitation, `H1-P` predation release); "island size" = carrying capacity (`live.prey_cap`, `live.resource_capacity_scale`), never terrain area (`Q3`); `SEL2-P` / `SEL2-P2` (`#290`) is **`QD-ISLAND`'s first brick**, testing resource limitation on one island. |
| **`QD-FOCUS FOCUS-3` (`2026-11-26`)** | `PR #314` (`ea78df18`) & `MSG_20261126_030000` | Frozen `SEL2-P2` (`#290`) runs once on `Windows 1`; `REPLAY-1` is drafted docs-only serving `QD-ISLAND`; order between `REPLAY-1` and `LAW1` ([`2026-11-24-law1-fork-experiment-draft.md`](2026-11-24-law1-fork-experiment-draft.md)) is set once both are ready to freeze. |
| **Hard simulation invariants** | [CLAUDE.md](../../../CLAUDE.md), [EVOLUTION_EXPERIMENT_CONTRACT.md](../../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) | `WorldLawSet` immutable within a run (`ER01`); no Lamarck (`ADR-0003`); phenotype developed once at birth (`ADR-0001`); closed EU (`max_abs_eu_drift <= 1e-6`, `AE-S05`); every new energy cost inside `total_cost`; opt-in flags keep baselines and `SEL2-P2` bit-identical; `live.births` is `Aggregation::Instant` (`F-053` read from `series.csv`, never `final_observables`); no cross-OS trajectory pooling (`ADR-0010`). |
| **Statistics template (`#214`) & `QD-VIAB`** | [2026-11-23-prereg-statistics-template.md](2026-11-23-prereg-statistics-template.md), [2026-11-24-sel2-p2-preregistration.md](2026-11-24-sel2-p2-preregistration.md) | Independent world checkpoint seeds as experimental unit (pseudoreplication across individuals or un-adjusted intra-checkpoint replays forbidden); Holm FWER (`m <= 3`); TOST equivalence (`90%` CI); `R3` extinction/attrition (`5%` / `pi_LL >= 0.85` / Lachin worst-rank / Lee bounds / `RMST`); `R7` floor; closed `R10` table; blind pre-freeze calibration (`QD-VIAB`, `0` body-mass peeking). |

---

## 1. Existing Mechanism Inventory (Verified at Trunk `40551113`)

Inspecting `src-tauri/` on `origin/feat/ecological-selection` at `40551113` establishes what already exists for checkpoint forking and observables, and where `REPLAY-1` hits structural gaps:

### 1.1 `LiveExperimentAdapter` Checkpoint Fork & State Serialization

| Mechanism | Current Implementation on Trunk (`40551113`) | Verified Anchor |
|---|---|---|
| **Adapter & model identity** | `LiveExperimentAdapter` (`LIVE_MODEL_VERSION = "live-bevy-world/2"`), immutable `WorldLawSet` (`ER01`), `DeterministicMode::on()`, single `build_tick_schedule` | [`live_experiment.rs#L67`](../../../src-tauri/src/core/live_experiment.rs#L67), [`live_experiment.rs#L1935-L2045`](../../../src-tauri/src/core/live_experiment.rs#L1935-L2045) |
| **Checkpoint capture & resume (`ExperimentModel`)** | `snapshot()` refreshes `VitalStatistics` + accumulators into `LiveExperimentState` and calls `serialize_world_state`; `from_snapshot()` checks `check_resumable()`, enforces matching `law_fingerprint` (`ER01`), and rebuilds via `LiveExperimentAdapter::build(..., Some(&snapshot.saved), ...)` | [`live_experiment.rs#L914-L953`](../../../src-tauri/src/core/live_experiment.rs#L914-L953), [`live_experiment.rs#L1968-L2031`](../../../src-tauri/src/core/live_experiment.rs#L1968-L2031), [`live_experiment.rs#L2494-L2541`](../../../src-tauri/src/core/live_experiment.rs#L2494-L2541) |
| **Bit-exact checkpoint & disk round-trip tests** | `a_fork_from_a_known_tick_shares_its_prefix_and_diverges_only_afterwards` (control matches uninterrupted run bit-for-bit; `Deforest` treatment diverges) and `run_k_then_save_load_then_run_the_rest_matches_an_uninterrupted_run` (`through_disk`, `SCHEMA_VERSION`, `live.mean_energy_integral` bit-equal) | [`live_experiment_tests.rs#L398-L558`](../../../src-tauri/tests/live_experiment_tests.rs#L398-L558) |
| **31 `LIVE_KEYS` configuration surface** | Covers `live.founder_genotype`, `live.predator_fraction`, `live.prey_cap`, `live.resource_capacity_scale`, `live.drift_control`, `live.drift_reset_ticks`, `live.strict_prey_cap`, `live.mutation_rate`, `live.lineage_tracking` (`F-066`), etc. | [`live_experiment.rs#L277-L315`](../../../src-tauri/src/core/live_experiment.rs#L277-L315) |
| **Drift-control RNG carry-over** | `from_snapshot_with_drift_schedule` & `drift_rng_clone` carry `LIVE_DRIFT_SELECTION` `SimRng` state across windows for neutral drift pairs (`run_live_drift_pair`, `run_live_drift_ensemble`) | [`live_experiment.rs#L2146-L2270`](../../../src-tauri/src/core/live_experiment.rs#L2146-L2270), [`live_experiment.rs#L3954-L4163`](../../../src-tauri/src/core/live_experiment.rs#L3954-L4163) |
| **Serialization & migration primitives** | `spawn_serialized_agent` and `SpawnMigrationCommand` exist for single-agent transfer, though no multi-world archipelago runner exists yet | [`simulation_state.rs#L922`](../../../src-tauri/src/core/simulation_state.rs#L922), [`world_systems.rs#L980`](../../../src-tauri/src/core/world_systems.rs#L980) |

### 1.2 `experiment_runner` & Current Replay/Factor Limitations

| Mechanism | Current Behaviour & Anchors (`40551113`) | Why Insufficient Alone for `REPLAY-1` |
|---|---|---|
| **`ExperimentModel` trait & `run_manifest_seed`** | `ExperimentModel::snapshot` / `from_snapshot` / `step` / `checksum` / `observables` / `observables_unsampled`; `run_manifest_seed` drives genesis runs with `catch_unwind` | Single linear seed run; `VitalStatistics` open-window events are not in `SavedSimulationState`, so fork at non-sample tick starts branches with an empty window ([`experiment_runner.rs#L72-L160`](../../../src-tauri/src/core/experiment_runner.rs#L72-L160), [`#L636-L699`](../../../src-tauri/src/core/experiment_runner.rs#L636-L699)). |
| **`checkpoint_fork` & `CheckpointForkReport`** | Runs 1 shared prefix `1..=fork_tick`, captures `snapshot` + `rng_at_fork.clone()`, and spawns **exactly 2** branches (`control` vs `treatment`) over `fork_tick+1..=duration_ticks` | Only `1` control + `1` treatment per call; no `K`-replicate stochastic fan-out from one checkpoint ([`fork.rs#L86-L166`](../../../src-tauri/src/core/experiment_runner/fork.rs#L86-L166), [`#L413-L481`](../../../src-tauri/src/core/experiment_runner/fork.rs#L413-L481)). |
| **Internal `SimRng` vs runner `_rng`** | `LiveExperimentAdapter::step` ignores the runner's `_rng: &mut StdRng` and advances `world`'s internal `SimRng` restored from `(state.sim_rng_seed, state.sim_rng_pos)` | Cloning `LiveSnapshot` gives every branch the **identical** post-fork `SimRng` state — repeated control branches without a post-fork re-seed seam are bit-identical copies, NOT independent replays ([`live_experiment.rs#L2548`](../../../src-tauri/src/core/live_experiment.rs#L2548), [`#L2771-L2776`](../../../src-tauri/src/core/live_experiment.rs#L2771-L2776)). |
| **Post-fork `InterventionKind` surface** | Supports `RainfallDelta`, `TemperatureDelta`, `Deforest`, `AddNutrient`, and `RemovePredators` (deterministic entity-index-sorted cull prefix, closed-EU/ledger tracked); `GeneticInjection` disabled (`F-071`) | `RemovePredators` exists for predation release, but **no** post-fork `InterventionKind` alters carrying capacity (`live.resource_capacity_scale` / pasture `r_max` or `live.prey_cap`) or initial conditions ([`intervention.rs#L108-L162`](../../../src-tauri/crates/anima-domain/src/intervention.rs#L108-L162), [`live_experiment.rs#L1189-L1280`](../../../src-tauri/src/core/live_experiment.rs#L1189-L1280)). |
| **Ensemble helpers** | `run_ensemble`, `run_paired_ensemble`, `run_paired_ensemble_with_control` run genesis manifests (`SeedPair` per seed) | No built-in `checkpoint -> K replays × M arms` driver or within-checkpoint variance vs between-arm decomposition ([`ensemble.rs#L61-L240`](../../../src-tauri/src/core/experiment_runner/ensemble.rs#L61-L240)). |

### 1.3 Existing Live Observables (`28` Keys in `observables.rs` & `live_experiment.rs`)

`ObservableRegistry::live_default` ([`observables.rs#L306-L411`](../../../src-tauri/src/core/experiment/observables.rs#L306-L411)) and `LiveExperimentAdapter` ([`live_experiment.rs#L1589-L1658`](../../../src-tauri/src/core/live_experiment.rs#L1589-L1658)) already emit **`28` scalar observables** (`12` `LIVE_OBSERVABLE_IDS` + `16` `LIVE_EXTENDED_OBSERVABLE_IDS`):

- **Morphology & island-rule body-size metrics (`6`):** `live.trait_mass_mean`, `live.trait_mass_var`, `live.trait_mass_p10`, `live.trait_mass_p90`, `live.trait_mass_mean_west`, `live.trait_mass_mean_east` (`observables.rs#L371-L374, #L394-L395`).
- **Genotype / lineage diversity (`2`):** `live.shannon`, `live.simpson` (`observables.rs#L375-L376`, plus `live.lineage_tracking` `F-066` at `live_experiment.rs#L277`).
- **Demography & turnover (`9`):** `live.agent_count`, `live.herbivore_count`, `live.predator_count`, `live.births`, `live.deaths`, `live.deaths_starvation`, `live.deaths_dehydration`, `live.mean_lifespan`, `live.cumulative_mean_lifespan` (`observables.rs#L351-L353, #L377-L383, #L399`).
- **Energy & closed-EU integrity (`9`):** `plants`, `detritus`, `live.animals_eu`, `live.dormant_eu`, `live.closed_eu_total`, `live.food_items`, `live.standing_crop`, `live.mean_agent_energy`, `live.mean_energy_integral` (`observables.rs#L346-L356, #L387`).
- **Archive & environment (`2`):** `live.archive_coverage`, `live.season_phase` (`observables.rs#L358, #L380`).
- **Missing derived quantities:** Cross-replay dispersion (`Var_within`, `IQR`, pairwise branch divergence) and archipelago/per-island observables (`2026-11-19` §5) do not yet exist in `observables.rs`; they must either be computed from committed per-replay `series.csv` outputs or added behind opt-in keys (`QD-BIO-D2`).

---

## 2. Missing Code as Named Cards (Names & Scope Only — No Detailed Design)

> Detailed designs are intentionally deferred until `REPLAY1-D1`..`REPLAY1-D5` are answered and `FOCUS-1` allows implementation in sequence (`FOCUS-3` sets order vs `LAW1`).

| Future Card | Scope & Dependency | Role for `REPLAY-1` |
|---|---|---|
| **`REPLAY-C1`** `[RUST]` | Multi-replay post-checkpoint fan-out & opt-in deterministic `SimRng` (and `DriftControlState`) replay-stream re-seeding from `(checkpoint_seed, replay_index)` in `experiment_runner` / `LiveExperimentAdapter` | Enables `K` independent stochastic replays from one shared checkpoint while keeping `checkpoint_fork` and all baselines bit-identical when unused. |
| **`REPLAY-C2`** `[RUST]` | Declared post-checkpoint island-rule factor switch (`CauseId`-attributed, closed-EU): e.g., post-fork resource-capacity scaling / `prey_cap` change and/or sustained predation-release forcing, paired with `checkpoint_fork` / `REPLAY-C1` | Needed if `REPLAY1-D1` picks a carrying-capacity shift at the checkpoint (or sustained predator exclusion vs single `RemovePredators` cull). |
| **`REPLAY-C3`** `[RUST]` *(conditional on `REPLAY1-D1`)* | Archipelago runner, migration EU ledger, and `live.founder_genotype` large-vs-small ancestor key (`2026-11-19-feature-island-evolution-observatory.md` §5 Milestone `M1`) | Required **only if** `REPLAY1-D1` selects multi-island migration or explicit large-vs-small ancestor sizing rather than single-island resource/predation replay. |
| **`REPLAY-VIAB`** `[DATA]` | Blind pre-freeze viability & variance calibration on `1180001..=1180012` (`0` `trait_mass` inspection per `QD-VIAB`) | Measures `p_ext`, `pi_LL`, generation count, and checkpoint survival before locking `N`, `K`, `T_fork`, and `T_total`. |
| **`REPLAY-1-FREEZE`** `[DOC + FIXTURES]` | Locked `experiments_replay1/*.json` manifests, `prereg_replay1_manifest_tests.rs`, `verify_replay1_artifacts.mjs`, and `status: active` frozen doc | Mirrors `E3B-FREEZE` (`#231`) and `SEL2P2-FREEZE` (`#290`). |
| **`REPLAY-1-RUN` & `REPLAY-1-RESULT`** `[DATA + RESULT]` | Blind smoke (`999907`), once-only confirmation run, and unblinded `RESULT.md` under closed `R10` labels | Produces Stage 4-grade evidence serving `QD-ISLAND`. |

---

## 3. Pre-Registered Statistical Framework (`R1`–`R10`) & Disjoint Seed Block

### 3.1 Mapping [`2026-11-23-prereg-statistics-template.md`](2026-11-23-prereg-statistics-template.md) Rules `R1`–`R10`

- **`R1` Experimental unit & disjoint seeds:** Each independent **checkpoint world seed** $i \in \{1, \dots, N\}$ is the primary independent statistical unit (replays $r \in \{1, \dots, K\}$ nested within checkpoint $i$ are aggregated to one checkpoint-level summary $Y_i$ or analyzed via hierarchical mixed/permutation pairing by checkpoint $i$ so replays are never treated as $N \times K$ independent samples).
- **`R2` Multiplicity:** At most `m <= 3` confirmatory hypotheses under Holm–Bonferroni FWER $\alpha = 0.05$ (contrasting: (a) between-factor displacement vs. within-arm replay dispersion / drift, and/or (b) treatment-vs-control repeatability across replays).
- **`R3` Extinction & attrition:** Complete checkpoints primary iff arm/replay extinction $\le 5\%$ and $\hat{\pi}_{LL} \ge 0.85$ (with Lee bounds and survival `RMST`); otherwise Lachin worst-rank across all $N$ assigned checkpoints + `RMST` + `SACE`.
- **`R4` / `R9` SESOI & TOST:** Pre-registered SESOI and `90%` TOST equivalence CI on the scale chosen in `REPLAY1-D3`, cleanly separating `equivalent_to_zero` from `insufficient_evidence`.
- **`R5` Blind nuisance parameters:** Survival floor, generation count, checkpoint viability, and closed-EU drift (`<= 1e-6`) calibrated blindly in `REPLAY-VIAB` (`1180001..=1180012`) before freeze (`QD-VIAB`).
- **`R6` Power:** Per [`2026-11-23-prereg-statistics-template.md`](2026-11-23-prereg-statistics-template.md) §4–§5, standard paired-`t` / Wilcoxon `scipy.stats.nct` bounds apply to checkpoint-level summaries, and §5 explicitly requires **Monte Carlo simulation** on blind `REPLAY-VIAB` nuisance parameters when hierarchical checkpoint × replay dispersion or worst-rank/RMST endpoints are primary (`REPLAY1-D2`).
- **`R7` / `R8` / `R10`:** Exact Wilcoxon floor ($n_{\neq 0} \ge 7$ for $m \in \{2, 3\}$), full effect-size bundle ($d_z$, $r_c$, Hodges–Lehmann + `10,000`-resample 95% BCa CI, positive concordance, Lee bounds, `RMST`, plus within-checkpoint vs. between-factor variance ratio), and deterministic closed `R10` label table.

### 3.2 Exhaustive Disjoint Seed Inventory (`0` Collisions Checked on `40551113`)

Every existing preregistration, draft, fixture directory, and smoke/test seed band in the repository was checked (`git grep -w` across `src-tauri/`, `docs/`, `artifacts/`, and `scripts/` at `40551113` -> **0 collisions**):

| Campaign / Checked File | Exact Seed Range | Status vs. `REPLAY-1` |
|---|---|---|
| Toy / `AE` / `SEL2` exploratory (`S6..S15`) | `1..=6` | Checked (`< 613,001`) |
| Unit tests (`e3b_runner`, `viab2_probe`, `LAW1` draft) | `613001..=615999` (`613001, 613002, 613999`, `614*`, `615*`) | Checked (`< 616,001`) |
| `E2` ([`experiments_e2/`](../../../src-tauri/tests/fixtures/experiments_e2/)) | `700001..=711001` (`700001 + 1000*k`) | Checked |
| `E3` / `E3-X` ([`experiments_e3/`](../../../src-tauri/tests/fixtures/experiments_e3/)) | `720001..=731001` | Checked |
| `E2R` & `E-SEL1` ([`experiments_esel1/`](../../../src-tauri/tests/fixtures/experiments_esel1/)) | `740001..=771001` | Checked |
| `SEL2-P` ([`2026-11-19-sel2-p-preregistration-draft.md`](2026-11-19-sel2-p-preregistration-draft.md)) | `780001..=799001` + baseline `799501..=799506` | Checked |
| `E3b` ([`2026-11-24-e3b-preregistration.md`](2026-11-24-e3b-preregistration.md)) | `800001..=879001` (`N=64`: `800001..=863001`) | Checked |
| `VIAB-1` / `VIAB-1b` / `VIAB-2` | `880001..=880012` | Checked |
| `SEL2-P2` ([`2026-11-24-sel2-p2-preregistration.md`](2026-11-24-sel2-p2-preregistration.md)) | `900001..=969001` (`N=48`: `900001..=947001`) + baseline `970501..=970512` | Checked |
| `LAW1` draft ([`2026-11-24-law1-fork-experiment-draft.md`](2026-11-24-law1-fork-experiment-draft.md)) | `1010001..=1079001` (`1010001 + 1000*k`), `1080001..=1080012`, `999917`, `999601..=999622`, `615001..=615999` | Checked (`<= 1,080,012`, clears `1,110,001` by `29,989`) |
| Prior pipeline & smoke bands | `999601..=999822`, `999917`, `999931`, `999953`, `999961`, `999971`, `999979`, `999983` | Checked (`!= 999907` and `999501..=999522`) |
| **Proposed `REPLAY-1` Confirmation Checkpoint Pool** | **`1110001 + 1000 * k` (`k = 0..69`, `1110001..=1179001`; replay streams `seed_k + r` inside `[seed_k + 1, seed_k + 999]`)** | **FRESH (`0` repo hits)** |
| **Proposed `REPLAY-VIAB` Calibration Block** | **`1180001..=1180012`** (`12` calibration checkpoints) | **FRESH (`0` repo hits)** |
| **Proposed `REPLAY-1` Blind Smoke Seed** | **`999907`** (prime `< 999917`) | **FRESH (`0` repo hits)** |
| **Proposed `REPLAY-1` Pipeline Dry-Check** | **`999501..=999506`, `999511`, `999521`, `999522`** | **FRESH (`0` repo hits)** |
| **Proposed `REPLAY-1` Synthetic Unit Tests** | **`616001, 616002, 616999`** | **FRESH (`0` repo hits)** |

---

## 4. What `REPLAY-1` Will NOT Show (Scope Boundaries)

1. **No real-world biological claim:** Per [`2026-11-19-feature-island-evolution-observatory.md`](../requirements/2026-11-19-feature-island-evolution-observatory.md) §12, results establish only what mechanisms and replay repeatability hold in an explicitly declared model world, never empirical claims about wild island species.
2. **No speciation (`AE-S11` / `AE-S14`) claim:** Body-size shift and lineage diversity (`live.trait_mass_mean`, `live.shannon`, `live.simpson`) do not establish biological speciation without `AE-S11`/`AE-S14` gates.
3. **No `ER08` counter-based RNG claim:** Sequential `SimRng` phase drift after divergence remains active; repeatability vs. divergence is quantified empirically across explicit post-checkpoint replay streams (`REPLAY-C1`).
4. **Island size = carrying capacity, not terrain area:** Follows `QD-ISLAND` Q3 (`live.prey_cap`, `live.resource_capacity_scale`), while terrain uses deterministic `Generated` (`TerrainSource::Generated`).
5. **Headless adapter verified, not desktop app:** Same provenance rule as `E2` and `E3b`; zero confirmation runs or manifests are permitted before owner decisions and freeze.

---

## 5. GATE — Explicit Empty Owner Slots & Options Table (`REPLAY1-D1` through `REPLAY1-D5`, Rule L4)

> **Note:** Framing (**`REPLAY-1` replays island worlds serving `QD-ISLAND`**) is **DECIDED** (`QD-FOCUS FOCUS-3`, `MSG_20261126_030000`) and is not listed below. Every slot `REPLAY1-D1`..`REPLAY1-D5` below remains **EMPTY** (`OWNER DECISION REQUIRED`) — the per-slot options table and reasoned recommendations strictly prepare the owner's decision without filling any slot.

### 5.1 Consolidated Per-Slot Options & Recommendations

| Owner Slot | Options (2–3 Choices) | Recommended Option | Citations & Rationale (`Template` / `E3b Power` / `QD-ISLAND`) | Owner Decision |
|---|---|---|---|---|
| **`REPLAY1-D1`** Factor (`QD-ISLAND` mechanism) | **(a)** Predation release (`RemovePredators` cull vs. control); **(b)** Carrying-capacity shift (`REPLAY-C2`: `resource_capacity_scale` / `prey_cap` vs. control); **(c)** Paired 2-factor arms (`H1-R` scarcity + `H1-P` predation release) vs. shared control | **(b)** *(note: **(a)** requires zero new intervention code if `REPLAY-C2` is deferred)* | **Template** ([`#214`](2026-11-23-prereg-statistics-template.md) §3 `R2`): keeps confirmatory hypotheses within `m <= 3` without diluting $\alpha/m$. **`E3b`** ([`prereg`](2026-11-24-e3b-preregistration.md) §1.2, [`results`](2026-11-24-e3b-results.md)): proved `resource_capacity_scale` perturbations sustain `64/64` two-trophic pairs (`predator_fraction = 0.3`). **`QD-ISLAND`** ([req](../requirements/2026-11-19-feature-island-evolution-observatory.md) §1, §3–§4): defines island size as carrying capacity (`live.prey_cap`, `live.resource_capacity_scale`) and builds directly on `SEL2-P` / `SEL2-P2` (`H1-R`), while staying strictly one factor vs. control (`FOCUS-3`). | *(EMPTY)* |
| **`REPLAY1-D2`** `N × K` & power method | **(a)** Analytical `N = 48` checkpoints × fixed `K` (`scipy.stats.nct`); **(b)** Blind `REPLAY-VIAB` (`1180001..=1180012`) -> hierarchical Monte Carlo solving `(N, K)` for `>= 80%` Holm power; **(c)** Analytical `N = 32` or `N = 64` checkpoints | **(b)** *(targeting `N >= 48` checkpoints within `<= 70` pool)* | **Template** ([`#214`](2026-11-23-prereg-statistics-template.md) §1, §4–§5): warns pre-guessing `N` caused `E3`'s `1.9%` viability trap and explicitly requires **Monte Carlo simulation** on blind `R5` nuisance parameters whenever endpoints are hierarchical (`N × K`) or worst-rank/RMST. **`E3b`** ([`prereg`](2026-11-24-e3b-preregistration.md) `R6/R6b`, [`results`](2026-11-24-e3b-results.md) §1): even `N = 64` fell `insufficient_evidence` on `H1`/`H3` when realized `sd_L` (`1.24–1.61`) exceeded analytical planning `0.8109`. **`QD-ISLAND`** ([req](../requirements/2026-11-19-feature-island-evolution-observatory.md) §7): mandates deriving final seed counts from blind exploratory calibration (`"Không chọn số seed theo kết quả"`). | *(EMPTY)* |
| **`REPLAY1-D3`** Divergence metric & SESOI (`R4`/`R9`) | **(a)** Time-integrated `live.trait_mass_mean` log-ratio + ICC / variance ratio; **(b)** Post-fork time-integrated factor-vs-replay separation $d_{\text{replay}}$ paired with `live.trait_mass_mean` log-ratio TOST `±SESOI`; **(c)** 3-metric Holm family (`mean`, `var`, `shannon`) | **(b)** *(with multi-tick tail/window aggregation, not single-tick)* | **Template** ([`#214`](2026-11-23-prereg-statistics-template.md) §3 `R4`/`R8`/`R9`/`R10`): requires pre-registered `±SESOI` and `90%` TOST CI so powered convergence/null separation maps deterministically to `equivalent_to_zero`. **`E3b`** ([`results`](2026-11-24-e3b-results.md) §1): demonstrated single-tick variance spikes (`sd_L = 1.24–1.61`) tighten `~20–35%` under tail-window aggregation (`1.00–1.11`), supporting time-integrated / tail-window summaries. **`QD-ISLAND`** ([req](../requirements/2026-11-19-feature-island-evolution-observatory.md) §3–§4): pins post-warmup time-integrated genotypic body mass (`live.trait_mass_mean`) for comparability with `SEL2-P` / `SEL2-P2`. | *(EMPTY)* |
| **`REPLAY1-D4`** Run length (`T_fork`, `T_total`) | **(a)** Medium `SEL2-P2` horizon (`T_fork = 20,000`, `T_total = 200,000`, `sample_period = 500`); **(b)** Extended two-phase (`T_fork = 50k–100k`, `T_total = 200k–300k`); **(c)** Short `E3b` horizon (`T_fork = 6,000`, `T_total = 18,000–36,000`) | **(a)** *(generation count & survival verified in `REPLAY-VIAB`)* | **Template** ([`#214`](2026-11-23-prereg-statistics-template.md) §2): favors extra machine runtime over under-run inconclusive loops. **`E3b`** ([`prereg`](2026-11-24-e3b-preregistration.md) §1.3, [`results`](2026-11-24-e3b-results.md)): showed `T = 18,000` yields only `~2.1–3.1` generations with `founder_genotype = 1.0`, whereas `200,000` ticks matches `SEL2-P2`'s blind `12/12` `VIAB-2` survival envelope (`#258`). **`QD-ISLAND`** ([req](../requirements/2026-11-19-feature-island-evolution-observatory.md) §7–§8): budgets island-rule runs on `200,000`-tick horizons matching `SEL2-P` / `SEL2-P2`. | *(EMPTY)* |
| **`REPLAY1-D5`** Execution platform (`ADR-0010`) | **(a)** `Windows 1` (`anima-case`, `windows/x86_64`); **(b)** `macOS arm64` (`anima-mac`); **(c)** Homogeneous single-OS `OxideSwarm` worker batch | **(a)** | **Template** ([`#214`](2026-11-23-prereg-statistics-template.md) §2): calibrated `Windows 1` throughput benchmarks (`E3-X` `153.55 s` for `36` runs; `VIAB-1` `560–722 s` for `48` runs). **`E3b`** ([`results`](2026-11-24-e3b-results.md) / [`run`](2026-11-24-e3b-run-execution.md)): `192/192` reproducible headless confirmation runs executed on `Windows 1`, matching `SEL2-P2` (`#290`). **`QD-ISLAND`** ([req](../requirements/2026-11-19-feature-island-evolution-observatory.md) §5, §6): cites `ADR-0010` cross-OS drift after tick `~180`, keeping runs in one OS family comparable to `SEL2-P2`. | *(EMPTY)* |

### 5.2 Detailed Slot Descriptions

### `REPLAY1-D1` — The Changed Factor (`QD-ISLAND` Mechanism)
- **Owner Decision:** *(EMPTY — OWNER DECISION REQUIRED; Rule L4)*
- **Options:**
  - **(a)** **Predation release at checkpoint** (uses existing [`InterventionKind::RemovePredators`](../../../src-tauri/src/core/live_experiment.rs#L1251-L1280) from a shared two-trophic `predator_fraction = 0.3` prefix, testing `H1-P` directly; smallest delta if sustained exclusion or one-shot cull is specified).
  - **(b)** **Resource carrying-capacity shift at checkpoint** (adds `REPLAY-C2` post-fork `resource_capacity_scale` / `prey_cap` change from a shared abundant prefix, testing `H1-R` directly and pairing naturally with `SEL2-P2`).
  - **(c)** **Both single-factor arms (`H1-R` scarcity + `H1-P` predation release) vs. shared control**, optionally adding a `live.mutation_rate = 0` or `live.drift_control = 1` replay reference arm.
  - **(d)** **Deferred archipelago option (`REPLAY-C3`, out of `REPLAY-1` fast-path unless promoted):** Full multi-island `2×2×2` archipelago (`H1`, `H1-R`, `H1-P` with large vs. small ancestors; requires archipelago runner, EU migration ledger, and founder-size key before `REPLAY-1`).

### `REPLAY1-D2` — Sample Size (`N` Checkpoints × `K` Replays) & Power Method
- **Owner Decision:** *(EMPTY — OWNER DECISION REQUIRED; Rule L4)*
- **Options (from [`2026-11-23-prereg-statistics-template.md`](2026-11-23-prereg-statistics-template.md) §4–§5):**
  - **(a)** **Paired checkpoint-level summary (`N = 48` checkpoints × `K` replays/arm, `scipy.stats.nct`):** `83.2%` power at $d_z = 0.50$ under Holm `m = 3` (`87.1%` under `m = 2`, `R7` $p_{\min} = 7.11 \times 10^{-15}$), matching `SEL2-P2` and `LAW1` (`N = 48`); `K` (e.g. `4` or `8` replays/checkpoint/arm) calibrated via `REPLAY-VIAB` to bound within-checkpoint estimation error.
  - **(b)** **Hierarchical Monte Carlo power calibration (`REPLAY-VIAB` -> `N × K` grid):** Use blind `1180001..=1180012` pilot dispersion (template §5) to solve `(N, K)` jointly for `>= 80%` (or `90%`) power at Holm `alpha / m` on the chosen repeatability/divergence ratio (`REPLAY1-D3`).
  - **(c)** **`N = 32` or `N = 64` checkpoints** (per template §4: `N = 32` gives `61.9%`/`68.1%` at $d_z = 0.50$ and `87.8%`/`91.0%` at $d_z = 0.66$; `N = 64` gives `93.5%`/`95.3%` at $d_z = 0.50$, matching `E3b`).

### `REPLAY1-D3` — Primary Divergence / Repeatability Metric & SESOI (`R4` / `R9`)
- **Owner Decision:** *(EMPTY — OWNER DECISION REQUIRED; Rule L4)*
- **Options:**
  - **(a)** **Post-fork time-integrated body-mass log-ratio & repeatability ICC / dispersion ratio:** Contrast mean post-fork $\ln(\bar{z}_{\text{treat}} / \bar{z}_{\text{ctrl}})$ on `live.trait_mass_mean` (matching `SEL2-P2`'s endpoint) alongside the within-arm vs. between-factor variance ratio $\ln(\text{MSE}_{\text{factor}} / \text{MSE}_{\text{replay}})$ (or directional sign-concordance across replays).
  - **(b)** **Endpoint displacement vs. within-arm replay spread (`d_replay`):** Per-checkpoint standardized separation $|\bar{Y}_{i,\text{treat}} - \bar{Y}_{i,\text{ctrl}}| / s_{i,\text{within}}$ (directly operationalizing *"does one changed factor move it more than the replays differ among themselves?"*), paired with TOST bounds `±SESOI` on time-integrated `live.trait_mass_mean` log-ratio.
  - **(c)** **Multi-metric family (`m <= 3`):** Primary `live.trait_mass_mean` log-ratio + secondary `live.trait_mass_var` or `live.shannon` lineage divergence under Holm–Bonferroni `m = 3`.

### `REPLAY1-D4` — Run Length (`T_fork`, `T_total`, `sample_period`, Analysis Window)
- **Owner Decision:** *(EMPTY — OWNER DECISION REQUIRED; Rule L4)*
- **Options:**
  - **(a)** **Medium horizon (`T_fork = 20,000`, `T_total = 200,000`, `sample_period = 500`):** Matches `SEL2-P2`'s `20,000`-tick warmup + `180,000`-tick post-fork window (`360` post-fork samples, ~`10+` generations depending on `predator_fraction` in `REPLAY-VIAB`).
  - **(b)** **Extended two-phase horizon (`T_fork = 50,000`–`100,000`, `T_total = 200,000`–`300,000`):** Lets an island lineage adapt or stabilize in prefix before the factor fork, then tracks post-fork divergence across `100,000–200,000` ticks (exact `T_fork` and `T_total` verified blindly in `REPLAY-VIAB` for target generation count).
  - **(c)** **Short `E3b`-scale horizon (`T_fork = 6,000`, `T_total = 18,000`–`36,000`, `sample_period = 200`):** Fast sweep (`~2–4` generations), lower runtime per replay, higher risk of transient dynamics dominating evolutionary divergence.

### `REPLAY1-D5` — Execution Platform (`ADR-0010` Single OS Family)
- **Owner Decision:** *(EMPTY — OWNER DECISION REQUIRED; Rule L4)*
- **Options:**
  - **(a)** **`Windows 1` (`anima-case`, `windows/x86_64`):** Same host and OS family as `E3b` (`#245`) and `SEL2-P2` (`#290`), allowing direct single-OS-class comparison with `SEL2-P2`.
  - **(b)** **`macOS arm64` (`anima-mac`):** Offloads compute from `Windows 1`, but results belong to the `macOS arm64` OS class (`ADR-0010`).
  - **(c)** **Homogeneous `OxideSwarm` worker batch** (restricted strictly to one OS family per `AGENT_RULES.md` §4 & `ADR-0010`).
