---
phase: planning
feature: alternate-evolution-world-lab
title: Preregistration Draft — LAW1: First Law-Fork Experiment (Exotic Energy Regime vs QD-AE-C Controls)
status: proposed
owner: anima-case
last_reviewed: 2026-11-24
description: Body-only preregistration draft for LAW1 (first exotic-energy law-fork experiment under ADR-0002, QD-LAW1, and QD-AE-C four control arms), documenting the LiveExperimentAdapter refusal boundary, missing code cards (AE-C0/AE-C1/L0/L1/LAW1-VIAB/L2-P/L2-X), disjoint seed block 1010001..=1079001, and open owner decisions LAW1-D1 through LAW1-D6.
---

# Preregistration Draft — `LAW1`: First Law-Fork Experiment (`ExoticEnergyLaw` vs. `QD-AE-C` Controls)

> **PROPOSED DRAFT (`status: proposed` — `0` CODE CHANGES, `0` MANIFESTS WRITTEN, `0` SEEDS EXECUTED).**
> - **Trigger (`QD-LAW1`):** Under [AGENT_WORKFLOW.md §4](../../how-to/AGENT_WORKFLOW.md), `QD-LAW1` specifies that *"the first law-fork experiment runs right after the first powered experiment, before v1.0.0."* With `E3b` confirmation executed (`PR #245`) and `C4. E3B-RESULT` (`RESULT.md`) submitted (`PR #252`, verified `verdicts/E3B-RESULT_e5ff2ea4.md`), this draft scopes `LAW1` and exposes all code prerequisites and open owner decisions prior to any Rust work or fixture freeze.
> - **Hard target boundary ([`live_experiment.rs#L1985-L1994`](../../../src-tauri/src/core/live_experiment.rs#L1985-L1994)):** `LiveExperimentAdapter` (`live-bevy-world/2`) explicitly **refuses** any manifest where `laws.exotic_energy.is_some()` with `ExperimentError::InvalidLaw` (*"the live Bevy world has no exotic-energy field: nothing in its schedule sources, diffuses or spends MU"*). Today, only [`ReferenceEvolutionWorld`](../../../src-tauri/src/core/reference_world.rs) supports `ExoticEnergyField` + `EnergyPathwayGenotype`. See [§1](#1-world-target-analysis--missing-code-cards) and Gate **`LAW1-D1`**.
> - **Gate blocking freeze (`LAW1-D1`..`LAW1-D6`):** Section 5 leaves all **six** owner decisions explicitly empty (`OWNER DECISION`). Zero code edits, manifests, calibration runs (`1080001..=1080012`), smoke (`999917`), or confirmation (`1010001..`) may start until the owner decides `LAW1-D1`..`LAW1-D6` and prerequisite code cards land.

---

## 0. Provenance, Governing Decisions & World-Law Hard Rules

| Rule / Document | Reference | Requirement Imposed on `LAW1` |
|---|---|---|
| **`ADR-0002` (accepted headless reference slice only, `2026-11-17`)** | [ADR-0002-world-laws-and-exotic-energy.md](../../decisions/ADR-0002-world-laws-and-exotic-energy.md) | Points 1–8 and first half of 10 (`headless reference`) accepted; **NOT yet accepted** for `AE-S11` species detector, `AE-S15` full persistence, or live Bevy world (`AE4`/`L1`). Uses `core::exotic_energy` (`ExoticEnergyLaw`, `ExoticEnergyField`), `"MU"` separate ledger, and `exotic_energy = None` baseline compatibility. |
| **`QD-LAW1` (sequence)** | [AGENT_WORKFLOW.md §4](../../how-to/AGENT_WORKFLOW.md) & [2026-11-05-swarm-master-plan.md](2026-11-05-swarm-master-plan.md) | Runs right after the first powered experiment (`E3b` / `PR #252`), ahead of `v1.0.0`. Downstream execution cards: `L0`/`AE-C*` -> `L1` (if live) -> `L2-P` (freeze) -> `L2-X` (run + `RESULT.md`). |
| **`QD-AE-C` (four control arms, `2026-11-18`)** | [2026-11-18-research-law-fork-and-multi-machine.md §2.3](2026-11-18-research-law-fork-and-multi-machine.md#23-đề-xuất-qđ-ae-c-bốn-nhánh-đối-chứng-cho-luật-năng-lượng-lạ) | Mandatory four control arms besides `exotic_energy = None` (`T0`): **`T2a` ghost field** (`== T0` bit-for-bit checksum gate), **`T2b` unrewarded costly sham**, **`T3` iso-energetic**, and **`T4` elevated primary**. Requires amending `ER06`/`ER07` in [EVOLUTION_EXPERIMENT_CONTRACT.md](../../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) and running the `T2a` ghost-field gate first. |
| **World-law hard rules** | [CLAUDE.md](../../../CLAUDE.md) | `WorldLawSet` immutable within a run; `"Mana"` is display name only (`core::exotic_energy`, no `ExoticEnergy` struct); **MU is not EU** (separate budget `AE-S04`, closed EU preserved `AE-S05`); performance must derive from completed `state.last_spent_mu` transactions, never `expressed`/`has_exotic`; only `ReferencePopulation::reproduce` writes cohort counts/genotypes; AE3 opt-in via `ae3.` keys with bit-identical legacy fallback; `evolution.births` is `Aggregation::Instant`; `EnergyPathwayGenotype::crossover` requires matching `source_id`. |
| **Statistics template (`#214`) & format models** | [2026-11-23-prereg-statistics-template.md](2026-11-23-prereg-statistics-template.md), [2026-11-24-e3b-preregistration.md](2026-11-24-e3b-preregistration.md), [2026-11-24-sel2-p2-preregistration-draft.md](2026-11-24-sel2-p2-preregistration-draft.md) | Paired CRN (`run` unit = whole world, never individuals), Holm FWER (`m <= 3`), TOST equivalence margin, `R3` attrition + Lee bounds + `RMST`, `R7` signed-rank floor, closed `R10` table, disjoint seed block, and body/trait-blind pre-run calibration (`LAW1-VIAB`). |

---

## 1. World Target Analysis & Missing Code Cards

### 1.1. Current Code Inventory: `ReferenceEvolutionWorld` vs. `LiveExperimentAdapter`

Inspecting `src-tauri/src/core/` at trunk commit `590aa032` establishes an exact split between what exists headlessly in `ReferenceEvolutionWorld` and what is blocked in the live Bevy world:

| Subsystem / Capability | `ReferenceEvolutionWorld` (`core/reference_world.rs`) | `LiveExperimentAdapter` (`core/live_experiment.rs`) | Code Evidence (`590aa032`) |
|---|---|---|---|
| **Exotic world-law acceptance** | **Accepted** (`WorldLawSet::with_exotic(...)`, `ExoticEnergyLaw::mana_patchy` / `mana_uniform`) | **REFUSED (`InvalidLaw`)** whenever `laws.exotic_energy.is_some()` | [live_experiment.rs#L1985-L1994](../../../src-tauri/src/core/live_experiment.rs#L1985-L1994) |
| **Exotic MU field & budget (`AE-S04`, `AE-S05`)** | **Implemented** (`ExoticEnergyField`, `ExoticEnergyBudget`, closed EU unaffected) | **Absent** (no MU field, diffusion, uptake, or spend system in `simulation_schedule.rs`) | [exotic_energy.rs](../../../src-tauri/src/core/exotic_energy.rs), [reference_world.rs#L7](../../../src-tauri/src/core/reference_world.rs#L7) |
| **Pathway trait & evolution** | **Implemented** (`EnergyPathwayGenotype`: `source_id`, `expressed`, `sensing_affinity`, `uptake_rate`, `storage_capacity`, `utilization_efficiency`, `tolerance`, `maintenance_cost`; `13` `ae3.*` keys) | **Absent** (`AgentGenotype` holds morphology + optional `BrainGenotype`, no `EnergyPathwayGenotype`) | [evolution_pathway.rs#L41-L59](../../../src-tauri/src/core/evolution_pathway.rs#L41-L59), [evolution_pathway.rs#L561-L590](../../../src-tauri/src/core/evolution_pathway.rs#L561-L590) |
| **Observables** | **Implemented** (`5` `exotic.*` + `10` `AE3_OBSERVABLE_IDS`: `evolution.pathway_frequency`, `performance_legacy/pathway/delta`, `exotic.uptake/spent`, etc.) | `LIVE_KEYS` (`31` keys) has zero `ae3.*` or `exotic.*` observables | [observables.rs#L142-L280](../../../src-tauri/src/core/experiment/observables.rs#L142-L280) |
| **Fork & ensemble runners** | **Implemented** (`genesis_fork`, `checkpoint_fork`, `checkpoint_fork_with_exotic`, `run_ensemble`, `run_paired_ensemble`) | Same generic `experiment_runner` API, but `laws.exotic_energy` rejected on construction | [fork.rs#L50-L159](../../../src-tauri/src/core/experiment_runner/fork.rs#L50-L159), [ensemble.rs#L61-L218](../../../src-tauri/src/core/experiment_runner/ensemble.rs#L61-L218) |
| **`QD-AE-C` `T2a`/`T2b`/`T3`/`T4` controls** | **Missing explicit schema/gate helpers** (`ER06`/`ER07` amendment pending; no first-class `ghost_field` or calibrated `T3`/`T4` energy-matching preflight) | **Missing entirely** (depends on `L0`/`L1` first) | [2026-11-18-research §2.3](2026-11-18-research-law-fork-and-multi-machine.md#23-đề-xuất-qđ-ae-c-bốn-nhánh-đối-chứng-cho-luật-năng-lượng-lạ) |
| **Counter-based `ER08` RNG** | **Deferred** (`core::rng.rs` uses sequential `StdRng`; `evolution_pathway.rs` uses separate domain `RNG_DOMAIN`) | **Deferred** (two sequential `StdRng` streams; new `ADR-0008` era required to change) | [2026-11-18-research §2.1](2026-11-18-research-law-fork-and-multi-machine.md#21-báo-cáo-nói-gì-và-repo-đã-có-gì) |

**Target world consequence (`LAW1-D1`):**
- If `LAW1` runs on **`ReferenceEvolutionWorld`** first (recommended `LAW1-D1(a)` Stage 1 = `LAW1-ref`), zero engine-tick changes to the live Bevy world are needed; only `QD-AE-C` control helpers/contract (`AE-C0` + `AE-C1`) and a reference experiment runner/verifier are needed.
- If `LAW1` must run on **`LiveExperimentAdapter`** (Option `LAW1-D1(b)` or Stage 2 of `LAW1-D1(a)` = `LAW1-live`), the full `L0` + `L1` (`AE4`) live exotic-energy implementation MUST land and pass all acceptance gates before any `LAW1-live` freeze.

### 1.2. Missing Code & Contract Items as Future Cards

| Future Card | Scope & Target World | Dependency | Concrete Acceptance / Done Criteria |
|---|---|---|---|
| **`AE-C0`** `[CONTRACT + RUST]` | Amend `ER06`/`ER07` in `EVOLUTION_EXPERIMENT_CONTRACT.md` for all 4 `QD-AE-C` arms; implement **`T2a` Ghost Field** mode in `ExoticEnergyLaw` / `ReferenceEvolutionWorld` | After `SEL2-P2` chain (per `QD-AE-C`) or immediately after `D1` approval | `T2a` (`ghost_field: true`, uptake clamped `0`, pathway cost `0`) executes full MU field step & ledger while producing **bit-identical** world checksums and all non-exotic observables to `T0` (`exotic_energy = None`) across multi-seed tests (proving zero RNG/order leakage). |
| **`AE-C1`** `[RUST]` | Implement & test **`T2b` Costly Sham**, **`T3` Iso-Energetic**, and **`T4` Elevated Primary** parameterization + energy-equivalence preflight audit on `ReferenceEvolutionWorld` | `AE-C0` | (1) `T2b` charges `maintenance_cost > 0` and uptake without performance reward (`utilization_efficiency` / `PERFORMANCE_PER_MU = 0`); (2) `T3`/`T4` enforce deterministic baseline resource scaling matching measured `T1` total MU throughput within tolerance; (3) `AE-S01..AE-S10` and `AE-S14` green. |
| **`L0`** `[DOC / ADR]` *(if live world chosen)* | Design contract & ADR amendment for live Bevy MU field (`WorldLawSet` live parameters, `total_cost` metabolic routing per `CLAUDE.md`, zero-alloc ring budget, `SCHEMA_VERSION` `8 -> 9`, `LIVE_MODEL_VERSION` `"/2" -> "/3"`) | Owner chooses `LAW1-D1(a)` Stage 2 or `(b)` | Accepted ADR amendment + explicit zero-alloc, conservation, and D01 migration specs before `L1` edits any Rust code. |
| **`L1`** `[RUST]` *(if live world chosen)* | Live Bevy `ExoticEnergyField`, `EnergyPathwayGenotype` in `AgentGenotype`, `ae3.` / `exotic.` live keys, `AE-S15` persistence, lift [live_experiment.rs#L1985](../../../src-tauri/src/core/live_experiment.rs#L1985) refusal | `L0` | `exotic_energy = None` bit-identical to trunk; `T2a == T0` bit-identical on live world; `AE-S04` MU budget closed; `AE-S05` closed EU `<= 1e-6`; zero tick allocations; headless adapter verified. |
| **`LAW1-VIAB`** `[DATA]` | Blind pre-freeze calibration ensemble (`12` exploratory seeds `1080001..=1080012`, zero confirmatory endpoint peeking) | `AE-C1` (or `L1` if live) | Verifies `T2a == T0` bit-for-bit across all `12` seeds, `>= 11/12` population persistence, generation count `>= 30` (or calibrated target), realized `T1` MU expenditure used to calibrate `T3`/`T4`, and zero `max_abs_eu_drift > 1e-6`. |
| **`L2-P`** `[DOC + FIXTURES]` | Freeze card: locked `experiments_law1/*.json` manifests, `prereg_law1_manifest_tests.rs`, `verify_law1_artifacts.mjs`, frozen doc (`status: active`) | `LAW1-VIAB` pass + owner merge approval | Same strict invariant partition & seed-disjointness gate structure as `E3b` (`#231`). |
| **`L2-X`** `[DATA + RESULT]` | Execute blind smoke (`999917`), once-only confirmation ensemble (`N` seeds `1010001..`), and unblinded `RESULT.md` | `L2-P` merged | Smoke gates pass blindly; `100%` provenance & checksums verified; `RESULT.md` reports frozen `R10` labels verbatim. |

---

## 2. `QD-AE-C` Six-Arm Law-Fork Design & Strict Attribution Rule

### 2.1. The Six Experimental Arms (`T0` Baseline + `T1` Treatment + `T2a/T2b/T3/T4` Controls)

Every seed $i$ runs under Common Random Numbers (CRN) from an identical world seed (`genesis_fork`, `AE-S08`, or matched fork protocol per `LAW1-D5`) across the six `QD-AE-C` arms:

| Arm | Code / Regime | `laws.exotic_energy` | Pathway Maintenance Cost | Pathway Benefit (`MU -> perf`) | Primary Resource (`EU` / Base Prod.) | Causal Role (`QD-AE-C`) |
|---|---|---|---|---|---|---|
| **`T0`** | Baseline control | `None` | Inactive (legacy absent or `expressed=false`) | `0` | Nominal (`1.0x`) | Reference baseline (`AE-S01`). |
| **`T1`** | **Active exotic law (treatment)** | `Some(Renewable(...))` | Active (`maintenance_cost > 0`, `tolerance`) | Active (`state.last_spent_mu`) | Nominal (`1.0x`) | Focal alternate-law world (MU field + heritable pathway + cost). |
| **`T2a`** | **Ghost field control** *(hard gate)* | `Some(Ghost(...))` | **`0`** | **`0`** (uptake clamped `0`) | Nominal (`1.0x`) | **Safety / side-effect invariant:** MU field ticks & logs, uptake clamped to `0`. **MUST match `T0` checksums bit-for-bit.** |
| **`T2b`** | **Unrewarded costly sham** | `Some(Sham(...))` | Active (`maintenance_cost > 0`) | **`0`** (spent MU yields `0` performance) | Nominal (`1.0x`) | **Non-adaptive bias check:** tests whether mutation, sensing, or baseline asymmetry spreads the pathway without caloric reward. |
| **`T3`** | **Iso-energetic law** | `Some(Renewable(...))` | Active (`maintenance_cost > 0`) | Active | **Reduced** (`1.0 - Delta_MU_eq`) | Separates **"two-resource law structure"** from **"higher total carrying capacity"** while holding total available energy constant vs. `T0`. |
| **`T4`** | **Elevated primary control** | `None` | Inactive | `0` | **Elevated** (`1.0 + Delta_MU_eq`) | Separates **"novel exotic energy law"** from **"adding the same extra energy to the single baseline resource"**. |

### 2.2. Mandatory Four-Condition Attribution Standard ([`2026-11-18-research §2.3`](2026-11-18-research-law-fork-and-multi-machine.md#23-đề-xuất-qđ-ae-c-bốn-nhánh-đối-chứng-cho-luật-năng-lượng-lạ))

Per `QD-AE-C`, `LAW1` may claim **"the exotic energy law alters evolutionary history"** ONLY if all four conditions jointly hold under the preregistered statistical rules (`§3`):
1. **`T2a == T0` strict invariance:** Every `T2a` seed matches its `T0` baseline checksum and trajectory bit-for-bit (failing this aborts confirmation as a harness bug).
2. **`T2b` sham purge / non-spread (`H3` or manipulation check):** `T2b` does NOT show positive pathway takeover above `T0`/initial noise (if the sham spreads, any apparent `T1` gain is confounded by mutation/drift bias).
3. **`T1` diverges from `T4` (`H1`):** Simply adding the extra energy budget into the baseline resource (`T4`) does not replicate `T1`.
4. **`T3` diverges from `T0` (`H2`):** Even at matched total energy (`T3` total energy = `T0`), switching to the two-resource exotic law alters evolutionary trajectories relative to `T0`.

---

## 3. Pre-Registered Statistical Rules `R1`–`R10`, Disjoint Seeds & Power

| Rule | Specification (`#214` Template; Subject to `LAW1-D1`..`D6`) |
|---|---|
| **`R1` Unit & Disjoint Seeds** | Experimental unit = **1 independent world seed** (never individuals within a run, avoiding pseudoreplication). Confirmation formula **`seed_k = 1010001 + 1000 * k`** (`k = 0..69`, `1010001..=1079001`, locked subset `k = 0..N-1` per `LAW1-D6`); calibration **`1080001..=1080012`**; smoke **`999917`**; pipeline dry-check **`999601..=999622`**; unit tests **`615001, 615002, 615999`**. Strictly disjoint from every repo seed (`§3.1`). |
| **`R2` Multiplicity** | Max `3` confirmatory contrasts, Holm–Bonferroni FWER $\alpha = 0.05$ (`0.016667`, `0.025`, `0.05` for `m=3`; `0.025`, `0.05` for `m=2`). Final family set by **`LAW1-D3`**. |
| **`R3` Extinction & Attrition** | Aligned with `#214` Template `R3`: complete pairs primary iff arm extinction `<= 5%` AND complete-pair fraction $\hat{\pi}_{LL} \ge 0.85$, accompanied by Lee (2009) bounds and separate survival `RMST`; else Lachin worst-rank across all `N` assigned seeds + separate `RMST` + SACE. |
| **`R4` SESOI & TOST Margin** | Fixed a priori before unblinding (`LAW1-D4`): candidate `±0.10` post-warmup `evolution.pathway_frequency` shift (Option `(a)`) or `±ln(1.25)` ratio (Option `(b)`). Pre-registered MDE reported explicitly against observed $\hat{\sigma}_D$. |
| **`R5` Blind Nuisance Parameters** | Survival floor, generation count, MU throughput (`Delta_MU_eq` for `T3`/`T4`), and EU/MU conservation verified blindly on `LAW1-VIAB` (`1080001..=1080012`) before freeze, with zero inspection of confirmatory contrast separations. |
| **`R6` Power & Paired Correlation** | Because `ER08` counter-based RNG is deferred, seed-level trajectory phase can drift after branching (`2026-11-18` §2.1 & §2.4). `LAW1` therefore powers conservatively (`§3.2`, including un-correlated WMW Noether bounds) AND **must report the realized paired correlation $\hat{\rho}$** alongside paired $d_z$. Sample size `N` set by **`LAW1-D6`**. |
| **`R7` Exact Signed-Rank Floor** | Requires $2 / 2^{n_{\neq 0}} < \alpha / m$: `n_non_zero >= 7` for `m=3` (`0.015625 < 0.016667`) and `m=2` (`0.015625 < 0.025`). |
| **`R8` Effect-Size Bundle** | Mean paired delta & `%`/ratio shift, standardized `d_z`, realized paired correlation $\hat{\rho}$, Kerby $r_c$, Hodges–Lehmann pseudomedian with deterministic `10,000`-resample 95% BCa CI, positive-pair concordance, Lee bounds, and `RMST`. |
| **`R9` TOST Equivalence** | `90%` paired $t$-CI strictly inside `(-SESOI, +SESOI)` ($\alpha_{\text{TOST}} = 0.05$ per tail) cleanly separates true powered equivalence (`equivalent_to_zero`) from `insufficient_evidence`. |
| **`R10` Closed Decision Table** | Closed 3-label set `{supports, equivalent_to_zero, insufficient_evidence}` (two-sided; or 4-label if directional `H3`), audited via explicit deterministic verifier rules mirroring `E3b` and `SEL2-P2`. |

### 3.1. Exhaustive Disjoint Seed Inventory & Proof (`95` Fresh `LAW1` Seeds, `0` Collisions)

Programmatic verification across all `.json`, `.csv`, `.rs`, `.md`, and `.toml` files in `src-tauri/tests/`, `src-tauri/examples/`, `artifacts/experiments/`, and `docs/ai/planning/` at `590aa032` plus `SEL2-P2`'s reserved `95` seeds confirms **zero overlaps (`REPO_WIDE_OVERLAP_HITS: {}`, `OVERLAP_VS_SEL2P2: []`)**:

| Series / Allocation | Exact Seed Set | Count | Relationship to `LAW1` |
|---|---|---:|---|
| **All Prior & `SEL2-P2` Reserved Sets** | | | |
| Toy / `AE` / `SEL2` exploratory | `1..=6` | `6` | Strictly `< 613,001` |
| Existing unit tests (`e3b_runner`, `viab2_probe`) | `613001, 613002, 613999`, `614001, 614002, 614999` | `6` | Strictly `< 615,001` |
| Prior powered campaigns (`E2`, `E3`/`E3-X`, `E2R`, `E-SEL1`, `SEL2-P`, `E3b`) | `700001..=879001` + baseline `799501..=799506` | `151` *(+cap)* | Strictly `<= 879,001` |
| `VIAB-1` / `VIAB-1b` / `VIAB-2` calibration | `880001..=880012` | `12` | Strictly `<= 880,012` |
| `SEL2-P2` confirmation pool + `QD-SEL2g` baseline | `900001..=969001` + `970501..=970512` | `82` | Strictly `[900001, 970512]`, clears `1010001` by **`-39,489`** |
| Prior pipeline & smoke bands (`999701..=999983`) | `999701..999822`, `999931`, `999953`, `999961`, `999971`, `999979`, `999983` | `23` | Distinct from `999601..999622` & prime `999917` |
| **Proposed `LAW1` Fresh Sets (`95` unique seeds)** | | | |
| **`LAW1` confirmation pool (cap `N <= 70`)** | **`1010001 + 1000 * k` (`k = 0..69`, `1010001..=1079001`; active `k = 0..N-1`)** | `70` (max) | **FRESH (`0` repo hits)** |
| **`LAW1-VIAB` blind calibration block** | **`1080001..=1080012`** (`12` calibration seeds) | `12` | **FRESH (`0` repo hits)** |
| **`LAW1` blind smoke (`L2-X`)** | **`999917`** (prime `< 999931`) | `1` | **FRESH (`0` repo hits)** |
| **`LAW1` pipeline dry-check** | `999601..=999606`, `999611`, `999621`, `999622` | `9` | **FRESH (`0` repo hits)** |
| **`LAW1` synthetic unit test seeds** | `615001, 615002, 615999` | `3` | **FRESH (`0` repo hits)** |

### 3.2. Analytical Power (`scipy.stats.nct`), Exact `R7` Floor & Independent-Sample WMW Reference

| `N` (Seeds) | `R7` $p_{\min}$ | `80%` MDE ($d_z$, $m=3$ / $m=2$ / $m=1$) | Power at $d_z = 0.50$ ($m=3$ / $m=2$ / $m=1$) | Power at $d_z = 0.66$ ($m=3$ / $m=2$ / $m=1$) | Zero-Correlation Fallback Reference ([`2026-11-18` §2.4](2026-11-18-research-law-fork-and-multi-machine.md#24-cỡ-mẫu-con-số-kiểm-lại-được-và-nó-nói-gì-cho-sel2-p)) |
|---:|---:|---|---|---|---|
| `24` | `1.19e-07` | `0.7050` / `0.6662` / `0.5972` | `0.4615` / `0.5295` / `0.6504` | `0.7376` / `0.7919` / `0.8720` | Unpaired WMW 80% large effect (`A = 0.737`, $\delta = 0.474$): `N = 24`/arm |
| `32` | `4.66e-10` | `0.6000` / `0.5683` / `0.5113` | `0.6192` / `0.6814` / `0.7823` | `0.8779` / `0.9096` / `0.9511` | Unpaired WMW 90% large effect (`A = 0.737`): `N = 32`/arm |
| **`48` *(Rec.)*** | **`7.11e-15`** | **`0.4818` / `0.4573` / `0.4129`** | **`0.8324` / `0.8706` / `0.9240`** | **`0.9792` / `0.9863` / `0.9941`** | Clears `80%` power at $d_z = 0.50$ across all $m \in \{1, 2, 3\}$; ~unpaired WMW medium effect `N = 49` (`A = 0.665`, $\delta = 0.33$) |
| `64` | `1.08e-19` | `0.4139` / `0.3933` / `0.3557` | `0.9350` / `0.9533` / `0.9761` | `0.9971` / `0.9983` / `0.9994` | Approaches unpaired WMW 90% medium effect (`N = 65`/arm) |

---

## 4. What `LAW1` Will NOT Show (Scope Boundaries)

1. **No speciation (`ER09` / `AE-S11`) claim:** Per `ADR-0002` and `2026-11-18` §2.5, short-horizon `LAW1` runs measure **pathway trait spread, MU expenditure, and ecological/morphological divergence**, never biological species emergence (`4–10 * N_e` generations + reproductive isolation/null-model clustering gates under `AE-S11` remain future work).
2. **If `LAW1-ref` runs on `ReferenceEvolutionWorld`, no live Bevy world claim:** Results on `ReferenceEvolutionWorld` demonstrate counterfactual law divergence in the aggregate headless reference model, NOT live spatially embedded Bevy creatures (which require `L0` + `L1` and `LiveExperimentAdapter`). Even after `L1`, headless runs verify **`headless adapter verified`**, not multi-threaded desktop GPU/UI execution.
3. **No counter-based RNG (`ER08`) proof:** Because sequential `StdRng` streams still drift in phase after branching events diverge, paired comparisons rely on realized seed-level pairing correlation $\hat{\rho}$ alongside full sample size `N`.
4. **No alternative source topologies beyond frozen `ExoticEnergyLaw`:** Findings bind strictly to the frozen source distribution, diffusion/decay rates, and pathway cost parameters selected at freeze (`L2-P`).

---

## 5. GATE — Explicit Open Owner Decisions (`LAW1-D1` through `LAW1-D6`)

All six decisions below MUST be answered by the project owner before opening code cards (`AE-C0`/`AE-C1`/`L0`/`L1`) or freezing `LAW1`.

### `LAW1-D1` — Target Simulation World (`ReferenceEvolutionWorld` vs. `LiveExperimentAdapter`)
- **Owner Decision:** *(EMPTY — awaiting owner choice before freeze)*
- **Context:** [`LiveExperimentAdapter`](../../../src-tauri/src/core/live_experiment.rs#L1985-L1994) explicitly refuses `laws.exotic_energy.is_some()` because the live Bevy world currently has no MU field or pathway traits, whereas [`ReferenceEvolutionWorld`](../../../src-tauri/src/core/reference_world.rs) already implements `ExoticEnergyField`, `ExoticEnergyBudget`, and `EnergyPathwayGenotype`.
- **Options:**
  - **(a)** **Staged execution (`LAW1-ref` first, `LAW1-live` second):** Add `AE-C0` (`ER06`/`ER07` + `T2a` ghost-field gate) and `AE-C1` (`T2b`/`T3`/`T4`) on `ReferenceEvolutionWorld` first to complete the initial pre-`1.0` `QD-LAW1` gate rapidly, then schedule `L0` + `L1` (`AE4`) separately for live Bevy replication.
  - **(b)** **Live world only (defer `LAW1` until `L0` + `L1` land):** Do not run a reference-world `LAW1`; build the full live MU field, organism pathway genetics, `total_cost` metabolic routing, `schemas v8 -> v9`, and `T2a..T4` inside Bevy (`L0` -> `L1`), then run `LAW1` once on `LiveExperimentAdapter`.
  - **(c)** **Reference world only:** Permanently restrict `LAW1` to `ReferenceEvolutionWorld` without scheduling `L1` live integration before `v1.0.0`.
- **`anima-case` Recommendation:** **Option `(a)`** (staged `LAW1-ref` right after `SEL2-P2` while `L0`/`L1` are scoped cleanly without blocking pre-`1.0` counterfactual law validation on full `L1` engine migration), or **Option `(b)`** if the owner wants the sole pre-`1.0` law-fork milestone to measure spatial live creatures rather than the aggregate reference population.

### `LAW1-D2` — `QD-AE-C` Control-Arm Execution Scope
- **Owner Decision:** *(EMPTY — awaiting owner choice before freeze)*
- **Options:**
  - **(a)** **Full `QD-AE-C` 6-arm design `{T0, T1, T2a, T2b, T3, T4}`** (`T2a` verified bit-identical to `T0` on every seed in both `LAW1-VIAB` and confirmation, alongside `T2b`, `T3`, and `T4`).
  - **(b)** **Pre-gate `T2a` in `LAW1-VIAB` + 5-arm confirmation `{T0, T1, T2b, T3, T4}`** (saves 1 arm per confirmation seed once `T2a == T0` holds in deterministic tests + blind smoke/calibration).
  - **(c)** Reduced 4-arm `{T0, T1, T2a, T2b}` omitting iso-energetic (`T3`) and elevated-primary (`T4`) *(non-compliant with `QD-AE-C`; included for completeness only — not recommended)*.
- **`anima-case` Recommendation:** **Option `(a)`** (full `QD-AE-C` 6-arm ensemble; if `LAW1-D1` runs on `ReferenceEvolutionWorld`, `6` arms × `N` seeds executes in seconds on `Windows 1`).

### `LAW1-D3` — Confirmatory Hypothesis Family (`m = 3` vs. `m = 2`)
- **Owner Decision:** *(EMPTY — awaiting owner choice before freeze)*
- **Options:**
  - **(a)** **`m = 3` Holm–Bonferroni family** ($\alpha = 0.05$):
    - `H1`: **`T1` vs. `T4`** (two-sided: exotic law vs. calorie-matched elevated primary control),
    - `H2`: **`T3` vs. `T0`** (two-sided: iso-energetic two-law split vs. baseline),
    - `H3`: **`T2b` vs. `T0`** (non-spread / elimination of costly unrewarded sham pathway).
  - **(b)** **`m = 2` Holm–Bonferroni family `{H1: T1 vs T4, H2: T3 vs T0}`** ($\alpha/2 = 0.025$), treating **`T2b` sham non-spread** as a mandatory pre-registered manipulation validity gate (`R3`/manipulation check) alongside `T2a == T0`.
- **`anima-case` Recommendation:** **Option `(b)` (`m = 2` confirmatory + `T2b` manipulation gate)** if matching `QD-AE-C` §2.3 cleanly (`T1 != T4` AND `T3 != T0` with `T2b` not retaining pathway), or **Option `(a)` (`m = 3`)** if formal equality with `#214`'s `m = 3` Holm budget is preferred (`N = 48` reaches `> 83%` power at $d_z = 0.50$ under both).

### `LAW1-D4` — Primary Endpoint & SESOI Margin (`R4`)
- **Owner Decision:** *(EMPTY — awaiting owner choice before freeze)*
- **Options:**
  - **(a)** *(If `LAW1-D1(a)` / `(c)` on `ReferenceEvolutionWorld`):* Post-warmup time-integrated **`evolution.pathway_frequency` delta** (on `H1/H2/H3`; noting `T0`/`T4` fix `pathway_frequency = 0` or pre-seeded baseline — specifically: `evolution.pathway_frequency` comparing `T1` vs. `T2b` / `T3` vs. `0`, PLUS **`evolution.performance_delta`** / total biomass / mean performance **`SESOI = ±0.10`** (`10` pp on `[0, 1]` frequency or `±ln(1.15)` relative performance ratio) refined cleanly during `AE-C1` + `LAW1-VIAB`).
  - **(b)** Log-ratio of post-warmup total population / metabolic throughput (`ln(perf_B / perf_A)`) with **`SESOI = ±ln(1.25)`** (`±0.2231`) so every contrast compares well-defined non-zero observables even when `T0`/`T4` lack expressed exotic pathways.
  - **(c)** *(If `LAW1-D1(b)` live world chosen):* Two-domain endpoint on live agents (`evolution.pathway_frequency` or `exotic.spent` + morphological `live.trait_mass_mean` divergence) defined after `L0`/`L1` observables lock.
- **`anima-case` Recommendation:** **Option `(b)`** (`ln`-ratio of defined non-zero population performance / biomass throughput `±ln(1.25)` across `{T0, T1, T3, T4}`, paired with secondary `evolution.pathway_frequency` on `{T1, T2b, T3}`), avoiding boundary division or degenerate zero-variance `T0`/`T4` pathway comparisons.

### `LAW1-D5` — Fork Architecture (`genesis_fork` vs. Two-Phase vs. `checkpoint_fork`)
- **Owner Decision:** *(EMPTY — awaiting owner choice before freeze)*
- **Options:**
  - **(a)** **`t = 0` Genesis Fork (`genesis_fork`, `AE-S08`):** Diverge `T0, T1, T2a, T2b, T3, T4` from `t = 0` per independent world seed (directly tests counterfactual law from origin as stated in `ADR-0002` and `QD-LAW1`).
  - **(b)** **Two-phase Genesis + Checkpoint Fork (`2026-11-18` §2.1):** Multiple replicates per lineage history (`N` histories × `M` law draws) to partition law effect vs. historical contingency variance.
  - **(c)** **Checkpoint Fork only (`checkpoint_fork`, `AE-S09`):** Evolve baseline until checkpoint `T_fork`, then switch/branch `WorldLawSet`.
- **`anima-case` Recommendation:** **Option `(a)` (`genesis_fork` at `t = 0`)** for `LAW1` cleanly separating law genesis effects from runtime shocks.

### `LAW1-D6` — Confirmation Sample Size `N` & Execution Host (`1010001 + 1000*k`)
- **Owner Decision:** *(EMPTY — awaiting owner choice before freeze)*
- **Options:**
  - **(a)** **`N = 48` seeds** (`1010001..=1047001`; `83.2%` power at $d_z = 0.50$ under `m=3` / `87.1%` under `m=2`, matches `SEL2-P2` sample size and unpaired WMW `N = 49` medium-effect robustness bound), executed once on **`Windows 1`**.
  - **(b)** **`N = 64` seeds** (`1010001..=1063001`; `93.5%` / `95.3%` power at $d_z = 0.50$, matches `E3b`).
  - **(c)** **`N = 32` seeds** (`1010001..=1031001`; `87.8%` / `91.0%` power at $d_z = 0.66$, `61.9%` / `68.1%` at $d_z = 0.50`).
- **`anima-case` Recommendation:** **Option `(a)` (`N = 48` seeds `1010001..=1047001` on `Windows 1`)**; horizon (`duration_ticks`) calibrated blindly in `LAW1-VIAB` (`1080001..=1080012`) to achieve `>= 30` realized generations.
