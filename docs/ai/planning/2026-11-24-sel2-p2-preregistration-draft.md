---
phase: planning
feature: experiment-sel2-food-scarcity
title: Preregistration Draft — SEL2-P2: Ecological Selection vs Neutral Demographic Drift Under Food Scarcity
status: proposed
owner: anima-case
last_reviewed: 2026-11-24
description: Body-mass-blind preregistration draft for SEL2-P2 with owner decisions QD-SEL2P2-D1 through QD-SEL2P2-D7 recorded and blind 200,000-tick VIAB-2 pre-check passed (`12/12` survival in all arms, `0` trait_mass), awaiting dedicated freeze PR before execution of smoke (`999931`) and confirmation (`900001..=947001`).
---

# Preregistration Draft — `SEL2-P2`: Ecological Selection vs. Neutral Demographic Drift Under Food Scarcity

> **PROPOSED DRAFT (`status: proposed` — DECISIONS `QD-SEL2P2-D1`..`D7` LOCKED BY OWNER `2026-11-24` (`MSG_20261124_080000`) & BLIND `VIAB-2` PRE-CHECK PASSED; NOT YET FROZEN — `0` SMOKE OR CONFIRMATION SEEDS EXECUTED).**
> - **Frozen preregistration:** Frozen without decision changes in [2026-11-24-sel2-p2-preregistration.md](2026-11-24-sel2-p2-preregistration.md) (pinned to trunk `f195a361ec50473932520ad30306bd366d719a6d`).
> - **Hard stop & prior-history disclosure (`QD-SEL2P2`):** Predecessor [SEL2-P (`2026-11-19-sel2-p-preregistration-draft.md`)](2026-11-19-sel2-p-preregistration-draft.md) stopped permanently at its blind smoke gate on `seed = 999961` (`200,000` ticks, [2026-11-19-sel2-p-smoke.md](2026-11-19-sel2-p-smoke.md)): pooled baseline survived `5/6`, but all three scarce arms extincted at ticks `4,400..5,050` (`216/218` deaths by starvation while pasture remained filled, later traced to soft `prey_cap = 320` founder boom-crash, `F-052`), failing `C1`, `C2`, `C5`, and `drift_valid`. Zero SEL2-P confirmation seeds (`780001..=799001`) were ever executed.
> - **Body-mass blindness (`QD-VIAB`):** Under `QD-VIAB` (`PR #215`, `PR #228`, `PR #247`, `C5`/`C5b`), `SEL2-P2` is designed and viability-gated strictly from survival, extinction timing, population floors, generation counts, `C1..C5` pasture/starvation gates, and `SEL2-D` drift diagnostics — **zero confirmation, `VIAB-1`/`VIAB-1b`, or `VIAB-2` body-mass values (`live.trait_mass_*`) were read, computed, or used**.
> - **Decision & freeze gate status:** Owner approved all seven decisions (`QD-SEL2P2-D1=(a)`, `D2=(a)`, `D3=(a)`, `D4=(b)`, `D5=(a)`, `D6=(a)`, `D7=(a)`) in [§6](#6-gate--owner-decisions-qd-sel2p2-d1-through-qd-sel2p2-d7-locked-2026-11-24) (`MSG_20261124_080000`), and the mandatory blind `200,000`-tick `VIAB-2` pre-check on `880001..=880012` **PASSED** ([2026-11-24-sel2-p2-viab2-results.md](2026-11-24-sel2-p2-viab2-results.md): `12/12` survival in all arms, `0` `trait_mass`, `D2(b)` escalation not triggered). The dedicated freeze card (fixtures, manifest tests, verifier, freeze doc) comes next; no blind smoke (`999931`) or confirmation run (`900001..=947001`) may start until the freeze PR merges.

---

## 0. Provenance, Governing Rules & Design Basis

| Source / Artifact | Reference / PR / Commit | Role in `SEL2-P2` Design | Body-Mass Values Consulted |
|---|---|---|---|
| **Governing decisions (`QD-VIAB`, `QD-SEL2P2`, `QD-SEL2g`, `QD-ISLAND`)** | [AGENT_WORKFLOW.md §4](../../how-to/AGENT_WORKFLOW.md) | Requires blind `VIAB-1` survival calibration first, discloses `SEL2-P` smoke failure, locks `QD-SEL2g` pooled baseline rule | **None** |
| **Prior `SEL2-P` draft & smoke stop** | [2026-11-19-sel2-p-preregistration-draft.md](2026-11-19-sel2-p-preregistration-draft.md), [2026-11-19-sel2-p-smoke.md](2026-11-19-sel2-p-smoke.md), [sel2p-preregistration.json](../../../src-tauri/tests/fixtures/experiments_sel2p/sel2p-preregistration.json) | Supplies three-arm Robertson–Price schema, `200k` horizon, `QD-SEL2g`, and `C1..C5`; documents `seed = 999961` failure | **None** (`contains_body_mass = false`) |
| **Shared blind exploration (`VIAB-1`, `W15`)** | [2026-11-23-viab1-results.md](2026-11-23-viab1-results.md) (`PR #215`), [summary.json](../../../artifacts/experiments/viab1-shared-viability/summary.json) | `25` `200k`-tick SEL2-P cells (`840` runs on `880001..=880012`); exposes `scarce_mut0` collapse (`0..1/12`) and identifies best `selected` / `scarce_drift` survival cells | **None (`0` `trait_mass` matches across `57` artifacts)** |
| **`mut0` collapse diagnosis (`PR #216`)** | [2026-11-23-mut0-collapse-diagnosis.md](2026-11-23-mut0-collapse-diagnosis.md) | Proves `scarce_mut0` collapse under `3`-segment clone (`mass = 3.3`) is real selection/evolutionary rescue (`3.3 -> ~2.5`) rather than an engine bug | Disclosed: inspected `trait_mass` on probe seeds `880001..=880006` only (**disjoint from `SEL2-P2`**) |
| **2-segment founder key (`FOUNDER-2SEG`)** | [2026-11-23-founder-2seg.md](2026-11-23-founder-2seg.md) (`PR #224` -> `98054ba2`) | Adds opt-in `live.founder_genotype = 1.0` (`2`-segment clone, `mass = 2.5`, SHA-256 `dfeb2762...`) & discloses `mutation.rs:211` operator-4 wall | Schema constant `2.5` only |
| **Blind 2-seg check (`VIAB-1b`)** | [2026-11-23-viab1b-results.md](2026-11-23-viab1b-results.md) (`PR #228`), [summary.json](../../../artifacts/experiments/viab1b-founder-2seg/summary.json) | Verifies `12/12` survival of `mut=0.05` & `mut=0.0` at `T = 18,000` (`e3b_fg1_g4_fl1[_mut0]`) | **None (`0` `trait_mass` matches)** |
| **Blind 200k pre-check (`VIAB-2`)** | [2026-11-24-sel2-p2-viab2-results.md](2026-11-24-sel2-p2-viab2-results.md) (`C5b`), [viab2_fg1_s075_k2k_fl0.json](../../../artifacts/experiments/sel2-p2/viab2/viab2_fg1_s075_k2k_fl0.json) | Verifies `12/12` survival across all 4 arms at `T = 200,000` (`founder_genotype = 1.0`, `s = 0.75`, `fl = 0`, `k = 2000`); `D1(a)` PASSED, `D2(b)` not triggered | **None (`0` `trait_mass` matches)** |
| **Statistical rules template (`#214`)** | [2026-11-23-prereg-statistics-template.md](2026-11-23-prereg-statistics-template.md) & [VIABILITY_PRICE §3.3–3.5](../../research/VIABILITY_PRICE_STATISTICS_RESEARCH_2026-11-22.md) | Supplies rules `R1`–`R10`: paired CRN, Holm `m <= 3`, `R3` extinction threshold (`<= 5%`), TOST, closed `R10` table | None |
| **Preregistration format model (`E3b`)** | [2026-11-24-e3b-preregistration.md](2026-11-24-e3b-preregistration.md) (`PR #231`) | Defines `31`-key `LIVE_KEYS` lock, explicit default-absent audit, disjoint seed table, and verifier structure | **None** |

---

## 1. World Configuration, `31` `LIVE_KEYS` Partition & Calibration Evidence

### 1.1. Base World Parameters & `31` `LIVE_KEYS` Partition (`590aa032`)

`SEL2-P2` runs via [`LiveExperimentAdapter`](../../../src-tauri/src/core/live_experiment.rs) (`LIVE_MODEL_VERSION = "live-bevy-world/2"`, `SCHEMA_VERSION = 8`, `WorldLawSet::baseline()`, `exotic_energy: None` per [ADR-0002](../../decisions/ADR-0002-world-laws-and-exotic-energy.md) and [EVOLUTION_EXPERIMENT_CONTRACT](../../reference/EVOLUTION_EXPERIMENT_CONTRACT.md)) on `Generated` terrain (`MapSettings::default()`, world `seed = 1337`, `256x256`; never pinned across OS/arch). Per rule `D-1`, manifest keys partition cleanly across all `31 == LIVE_KEYS.len()` keys:

- **Fixed shared overrides (`17` keys common across `selected`, `scarce_mut0`, `scarce_drift`):**
  - `live.ecological_fitness = 1.0`, `live.mortality = 1.0`, `live.reproduction = 1.0`, `live.locomotion = 1.0`, `live.drive_floor = 0.5`, `live.thirst_steering = 1.0`, `live.evolved_brains = 0.0` (locks neural topology/weights so morphological selection is isolated),
  - `live.founders = 100.0`, `live.predator_fraction = 0.0` (single herbivore trophic level; no predators), `live.trees = 8.0`, `live.lakes = 16.0`, `live.lake_layout = 1.0`, `live.plant_reserve = 0.0`, `live.food_cap = 50.0`, `live.dehydration_ticks = 960.0`,
  - **`live.prey_cap = 320.0`** (`80.0` in `QD-SEL2g` `baseline_ref`) + **`live.strict_prey_cap = 1.0`** (`F-052` fix, eliminates the `250`-tick founder boom-crash observed in `SEL2-P` smoke `999961`).
- **Per-arm factor overrides (`3` keys):**
  - `live.mutation_rate`: `0.05` (`selected`, `scarce_drift`, `baseline_ref`) vs. `0.0` (`scarce_mut0`)
  - `live.drift_control`: `0.0` (`selected`, `scarce_mut0`, `baseline_ref`) vs. `1.0` (`scarce_drift`, `SEL2-D`)
  - `live.drift_reset_ticks`: `0.0` (`selected`, `scarce_mut0`, `baseline_ref`) vs. **`2000.0`** (`scarce_drift`, rolling cohort reset from `VIAB-1`)
- **Locked design keys under `QD-SEL2P2-D1=(a)` & `QD-SEL2P2-D2=(a)` (`3` keys):**
  - **`live.founder_genotype = 1.0`** (`QD-SEL2P2-D1=(a)`, `2`-segment founder across all arms; `VIAB-2` pre-check passed `12/12`)
  - **`live.resource_capacity_scale = 0.75`** (`QD-SEL2P2-D2=(a)` across scarce arms; `1.0` in `baseline_ref`)
  - **`live.founder_layout = 0.0`** (`QD-SEL2P2-D2=(a)` across all arms; `D2(b)` fallback `2.0` not triggered)
- **Locked absent engine-default keys (`8` keys in `default_keys_left_absent`; `17 + 3 + 3 + 8 = 31 == LIVE_KEYS.len()`):**
  - `live.w_surplus = 1.0`, `live.starve_ticks = 120`, `live.kill_hydration = 0.0`, `live.kill_efficiency = 0.4`, `live.target_min_energy = 0.0`, `live.land_mask = 0`, `live.grass_steering = 0`, `live.lineage_tracking = 0`. *(Note: engine-absent defaults `live.ecological_fitness = false` (`0.0`) and `live.food_cap = FoodSpawnSettings::default().max_food_count` (`50`) are overridden explicitly above rather than left absent.)*
- **Simulation horizon & sampling:** `duration_ticks = 200_000`, `warmup_ticks = 20_000`, confirmation `sample_period = 500` (`400` samples, `360` post-warmup in `(20_000, 200_000]`), blind smoke `sample_period = 50` (`4,000` samples).

### 1.2. Blind Survival & Gate Summary from `VIAB-1` (`PR #215`) & `VIAB-1b` (`PR #228`)

Across all `25` `SEL2-P` cells in `VIAB-1` (`840` runs on exploratory seeds `880001..=880012`, `0` `trait_mass`), fixing soft `prey_cap` via `live.strict_prey_cap = 1` and setting rolling drift cohort reset `live.drift_reset_ticks = 2000` eliminated catastrophic early failure in `selected` and `scarce_drift` while exposing two structural facts:

1. **Four 12-seed `VIAB-1` Phase B candidates (`founder_genotype = 0`, `200,000` ticks, seeds `880001..=880012`):**

| Cell ID | Overrides vs. Registered `SEL2-P` | Wall (`s`) | Surv (`B/S/M/D`) | `S+D` Both Surv | `min_N >= 10` (`B/S/M/D`) | `selected` `min_N` | `selected` Med `N` / Gen | `selected` Starv Med | `g_selected` (`[0.4,0.6]` on surv.) | `selected` `C2/C3/C4/C5` | `drift_valid` (`mis: c/e/i/d`) |
|---|---|---:|---|:---:|---|---|---|---:|---|---|---|
| **`sel2p_b0_s075_k2k_fl0_12s`** | `strict=1, s=0.75, k=2000, fl=0` | `584.1` | `11/11/0/11` | **`11/12`** (`91.7%`) | `11/10/0/10` | `0..52` | `127.8` / `33.92` (`31.82..45.23`) | `0.686` | `0.352..0.578` (`10/11`, med `0.553`) | `10/12/12/11` | `11/12` (`0/0/2/0`) |
| **`sel2p_b2_s075_k2k_fl2_12s`** | `strict=1, s=0.75, k=2000, fl=2` | `5027.5` | `9/12/1/12` | **`12/12`** (`100%`) | `7/10/0/10` | `7..66` | `126.4` / `33.70` (`31.93..45.48`) | `0.664` | `0.381..0.576` (`11/12`, med `0.544`) | `11/12/12/12` | **`12/12`** (`0/0/0/0`) |
| `sel2p_b1_s075_k2k_fl1_12s` | `strict=1, s=0.75, k=2000, fl=1` | `560.2` | `9/11/0/11` | `11/12` (`91.7%`) | `9/8/0/8` | `0..43` | `128.7` / `34.45` (`31.54..45.56`) | `0.672` | `0.351..0.612` (`9/11`, med `0.526`) | `9/12/12/11` | `11/12` (`0/0/1/0`) |
| `sel2p_b0_s063_k2k_fl0_12s` | `strict=1, s=0.63, k=2000, fl=0` | `722.4` | `11/11/0/10` | `10/12` (`83.3%`) | `11/6/0/6` | `0..35` | `104.7` / `32.75` (`29.85..45.41`) | `0.742` | `0.360..0.516` (`8/11`, med `0.495`) | `8/11/12/11` | `10/12` (`0/1/14/0`) |

2. **`scarce_mut0` collapse without scarcity (`sel2p_f6`) & `PR #216` diagnosis:** In `sel2p_f6_strict_s100_cap80_mut0` (`scale = 1.0`, `prey_cap = 80`, `strict = 1`, seeds `880001..=880006`), `baseline_ref` survived `5/6` while `scarce_mut0` (differing only by `mutation_rate = 0`) survived `1/6`. Across all `210` `mut0` runs in `VIAB-1`, `205` extincted (median `21,400` ticks). `PR #216` confirmed this stems from the legacy `3`-segment founder (`mass = 3.3`, suboptimal `~30–38 EU` vs. `100 EU` target) requiring structural tail deletion to `2` segments (`mass = 2.5`) before stabilizing.
3. **2-segment rescue at `18k` ticks (`VIAB-1b`) and `200k`-tick `VIAB-2` validation (`QD-SEL2P2-D1=(a)`):** Starting directly from `live.founder_genotype = 1.0` (`node 0` mass `1.5`, `node 1` mass `1.0`, edge `0 -> 1`, canonical JSON SHA-256 `dfeb276205f818acf3f34f5d716cbf163598a03a23ffc6964ff85753e280c719`) yielded `12/12` survival in `VIAB-1b` (`T = 18,000`) and then **`12/12` survival across all 4 arms (`baseline_ref`, `selected`, `scarce_mut0`, `scarce_drift`) over `200,000` ticks** in blind `VIAB-2` (`viab2_fg1_s075_k2k_fl0`, seeds `880001..=880012`, `0` `trait_mass`; `min_N >= 59`, `11/12` `C2` `selected`, `12/12` `C2` `scarce_mut0`, `12/12` `drift_valid`; see [2026-11-24-sel2-p2-viab2-results.md](2026-11-24-sel2-p2-viab2-results.md)).
4. **Operator-4 wall disclosure ([`mutation.rs#L209-L231`](../../../src-tauri/src/evolution/mutation.rs#L209-L231)):** Structural deletion (`operator 4`) enforces `if genotype.nodes.len() > 2`. Any run starting with `live.founder_genotype = 1.0` (`2` nodes) sits directly on this lower bound: structural mutations can add segments (`>= 3`) or tune node mass/size bilaterally, never prune down to `1` node. Therefore, `H3` (`scarce_drift / scarce_mut0`) explicitly measures directional mutational transmission bias under the `nodes.len() >= 2` constraint.

---

## 2. Experimental Arms, Robertson–Price Decomposition & Hypotheses

### 2.1. Three Experimental Arms + `QD-SEL2g` Baseline Reference

| Arm | `mutation_rate` | `drift_control` | `drift_reset_ticks` | Role & Scientific Contrast |
|---|---:|---:|---:|---|
| **`selected`** | `0.05` | `0.0` | `0.0` | Natural ecological selection under `scale = 0.75` (`QD-SEL2P2-D2=(a)`), `strict_prey_cap = 1`, `prey_cap = 320` |
| **`scarce_drift` (`SEL2-D`)** | `0.05` | `1.0` | `2000.0` | Neutral demographic + mutational drift control executed via `run_live_drift_pair` (`live.drift_reset_ticks = 2000`) |
| **`scarce_mut0`** | `0.0` | `0.0` | `0.0` | Clonal zero-mutation control retained under `QD-SEL2P2-D1=(a)` (`trait_mass_mean` pinned identically to `2.5`, `trait_mass_var == 0`) |
| `baseline_ref` (`QD-SEL2g`) | `0.05` | `0.0` | `0.0` | Pooled abundant reference (`scale = 1.0`, `prey_cap = 80`, `12` baseline seeds `970501..=970512`, `min_surviving = 6`, `QD-SEL2P2-D7=(a)`) for pasture grazing gate `C2` |

### 2.2. Primary Endpoint & Robertson–Price Decomposition (`H1`, `H2`, `H3`)

Because `LiveExperimentAdapter` maps genotype directly to phenotype (`DevelopedPhenotype` plasticity is inactive), `live.trait_mass_mean` measures additive genotypic body mass $\overline{z}_g$. For each seed run over post-warmup samples $t \in (20\,000, 200\,000]$ (`sample_period = 500`, `n_samples = 360`):

$$\overline{z}_{A,i}^{\text{int}} = \frac{1}{360}\sum_{m=1}^{360} \text{live.trait\_mass\_mean}(t_m)$$

Under **`QD-SEL2P2-D4=(b)`**, the locked primary paired log-mean endpoint for contrast $B / A$ is:

$$D_{B/A,i} = \ln\left(\frac{\overline{z}_{B,i}^{\text{int}}}{\overline{z}_{A,i}^{\text{int}}}\right), \qquad \text{SESOI} = \pm\ln(1.10) \approx \pm 0.09531018 \quad ([0.909\times, 1.100\times])$$

Following the Robertson–Price identity and **`QD-SEL2P2-D3=(a)`** ($m = 3$ Holm–Bonferroni confirmatory family):
- **`H1` (`selected / scarce_drift`, PRIMARY, two-sided):** Isolates genuine ecological selection $\operatorname{Cov}(w, z_g)$ net of demographic drift and mutation bias at matched `mutation_rate = 0.05`. Two-sided because food limitation can favor either larger reserve/efficiency morphology or smaller total maintenance cost.
- **`H2` (`selected / scarce_mut0`, confirmatory under `m = 3`):** Total evolutionary shift away from the fixed clonal founder (`2.5` under `D1(a)`).
- **`H3` (`scarce_drift / scarce_mut0`, confirmatory under `m = 3`):** Measures pure mutational transmission bias $\mathbb{E}[w \cdot \Delta z_g]$ (especially informative given `mutation.rs:211`'s `nodes.len() > 2` asymmetry).

### 2.3. Secondary, Robustness & Integrity Observables (`decision_effect: none`)

All secondary comparisons outside the confirmatory family use Benjamini–Hochberg FDR (`q = 0.05`) and never alter the `R10` classification:
1. Tail-20% mean of `live.trait_mass_mean` (`tick > 160_000`, continuity with `E-SEL1` / `SEL2-P`).
2. `window_net_selection_deltas["live.trait_mass_mean"]`.
3. `live.trait_mass_var` (additive genetic variance proxy) and post-warmup time-integrated `live.mean_energy_integral`: $\overline{E}_{\text{post}} = (I(T) - I(T_{\text{warm}})) / (T - T_{\text{warm}})$.
4. Survival `RMST(tau = 200_000)` across ALL `N` assigned seeds and Lee (2009) trimming bounds when attrition is present.
5. `live.births`, `live.deaths`, and **`live.mean_lifespan` read from `series.csv` (NEVER `final_observables`, respecting `F-053` structural zero on drained final windows)** plus `live.cumulative_mean_lifespan`.
6. Closed-EU conservation ledger audit (`max_abs_eu_drift <= 1.0e-6 EU` across all sampled ticks in every run).

---

## 3. Pre-Registered Statistical Rules `R1`–`R10`, Disjoint Seeds & Power

| Rule | Specification (Locked by `QD-SEL2P2-D1`..`D7`, Linked to Prereg Template `#214`) |
|---|---|
| **`R1` Unit & Disjoint Seeds** | Paired CRN by seed on `Windows 1`; **`N = 48`** confirmation pairs **`seed_k = 900001 + 1000 * k` (`k = 0..47`, `900001..=947001`, `144` runs)** within `cap N <= 70` (`<= 969001 < 970501` baseline block) per **`QD-SEL2P2-D5=(a)`**; smoke **`999931`**; pooled baseline **`970501..=970512`** (`12` seeds, `min_surviving = 6`) per **`QD-SEL2P2-D7=(a)`**; see [§3.1](#31-exhaustive-disjoint-seed-inventory--proof). |
| **`R2` Multiplicity** | **`m = 3`** confirmatory hypotheses `{H1 (selected/scarce_drift), H2 (selected/scarce_mut0), H3 (scarce_drift/scarce_mut0)}`, Holm–Bonferroni FWER $\alpha = 0.05$ (`0.016667`, `0.025`, `0.05`), locked by **`QD-SEL2P2-D3=(a)`**. |
| **`R3` Extinction & Attrition** | Locked by **`QD-SEL2P2-D6=(a)`** (aligned strictly with `#214` Template `R3`): complete pairs primary iff arm extinction `<= 5%` (`VIAB-2` observed **`0/12` extinction** in all four arms under `fl=0, s=0.75`) AND $\hat{\pi}_{LL} \ge 0.85$, paired with Lee bounds + `RMST(tau=200k)`; otherwise switch primary to Lachin worst-rank across all `N = 48` seeds + separate `RMST(200k)` + SACE. Always require `drift_valid == true` and `has_shortfall == false` per contrast. |
| **`R4` SESOI & MDE** | Locked by **`QD-SEL2P2-D4=(b)`** before unblinding (`0` confirmation/VIAB mass data used): **log mean ratio** $\ln(\overline{z}_{B,i}^{\text{int}} / \overline{z}_{A,i}^{\text{int}})$ with **SESOI $= \pm\ln(1.10) = \pm 0.09531018$** (`[0.909x, 1.100x]`). Pre-registered achieved MDE reported as `0.4818 * sd_D` (`80%` Holm-1 at `N=48`) and `0.5 * sd_D` alongside observed `sd_D`. |
| **`R5` Blind Nuisance Parameters** | Strictly from `VIAB-1` / `VIAB-1b` / `VIAB-2` (`0` `trait_mass`): `VIAB-2` (`fl=0, s=0.75, fg=1, k=2000`) observed **`12/12` survival across all 4 arms**, `min_N >= 59`, `mean_N` med `136.65` (`selected`), `gen` med `32.92`, `drift_valid = 12/12` (`0` shortfalls), `C2` `11/12` `selected` / `12/12` `scarce_mut0` -> `D2(b)` fallback NOT triggered. |
| **`R6` Power & Sample Size** | Derived analytically + via blind MC (`0` empirical mass variance assumed beyond standardized $d_z$ sweeps); see [§3.2](#32-analytical-power-attrition-probability--exact-r7-tables). **`N = 48`** pairs (`144` runs on `Windows 1`) locked by **`QD-SEL2P2-D5=(a)`**. |
| **`R7` Signed-Rank Floor** | At `m = 3` ($\alpha/3 = 0.016667$), exact two-sided Wilcoxon / sign test requires `n_non_zero >= 7` (`2/2^7 = 0.015625 < 0.016667`). For **`N = 48`**, `p_min = 7.11e-15`. |
| **`R8` Effect-Size Bundle** | Log-mean ratio $\bar{D}$ and geometric ratio $\exp(\bar{D})$, raw linear delta + `%` shift (descriptive), standardized $d_z$, Kerby paired $r_c$ (calibrated benchmarks $r_c = 2\Phi(\sqrt{2}d_z) - 1$: `0.5205` at $d_z = 0.50$, `0.6641` at $d_z = 0.66$, `0.7421` at $d_z = 0.80$), Hodges–Lehmann pseudomedian with `10,000`-resample deterministic 95% BCa CI, positive-pair fraction, Lee bounds, and `RMST(200k)`. |
| **`R9` TOST Equivalence** | `90%` paired $t$-CI on $\bar{D}$ strictly inside `(-ln(1.10), +ln(1.10))` ($\alpha_{\text{TOST}} = 0.05$ per tail) separates true powered equivalence (`equivalent_to_zero`) from `insufficient_evidence`. |
| **`R10` Closed Decision Table** | Closed 3-label set `{supports, equivalent_to_zero, insufficient_evidence}` for two-sided `H1` (`direction_tag` distinguishes `predicted_two_sided_positive` vs. `predicted_two_sided_negative`); see [§3.3](#33-closed-r10-decision-table-two-sided-h1). |

### 3.1. Exhaustive Disjoint Seed Inventory & Proof

The `173` distinct prior/reserved seeds itemized in the rows below (`1..=3` is a subset of `1..=6`, so `6 + 3 + 13 + 13 + 12 + 13 + 36 + 65 + 12 = 173`; `179` including E3b's reserved `k = 64..69` cap band) plus repo-wide fixture/test scans were checked programmatically against the `95` new seeds allocated to `SEL2-P2`:

| Registry / Series | Exact Seed Set | Count | Usage Status |
|---|---|---:|---|
| **Prior / Reserved Sets (`173` itemized unique seeds across repo series)** | | | |
| `AE` baseline toys | `1..=3` *(subset of `1..=6` below)* | `3` | Reserved fixture seeds |
| `S6..S15` (`SEL2` exploration) | `1..=6` *(adds `4..=6` beyond `1..=3`)* | `6` | Executed exploratory seeds |
| `e3b_runner_tests` unit suite | `613001, 613002, 613999` | `3` | Rust integration tests |
| `E2` (PR `#100+`, merged) | `700001 + 1000*k` (`k=0..11`: `700001..=711001`) + smoke `999983` | `13` | Locked / executed |
| `E3` / `E3-X` (PR `#181`, merged) | `730001 + 1000*k` (`k=0..11`: `730001..=741001`) + smoke `999979` | `13` | Locked / executed |
| `E2R` exploratory range | `750001 + 1000*k` (`k=0..11`: `750001..=761001`) | `12` | Reserved in `prereg_*` |
| `E-SEL1` | `760001 + 1000*k` (`k=0..11`: `760001..=771001`) + smoke `999971` | `13` | Locked / executed |
| `SEL2-P` (retired at smoke) | `780001..=799001` (`20`) + baseline `799501..=799506` (`6`) + smoke `999961` (`1`) + pipeline `999801..=999806, 999811, 999821..=999822` (`9`) | `36` | Retired (`799501..506` & `999961` executed) |
| `E3b` (`PR #231` / `PR #245`) | `810001 + 1000*k` (`k=0..63`: `810001..=873001`, cap band `k <= 69` <= `879001`) + smoke `999953` | `65` (+`6` cap) | Frozen / executed (`PR #245`, `RESULT.md` in `#252`) |
| `VIAB-1` / `VIAB-1b` / `PR #216` / `VIAB-2` probes | `880001..=880012` | `12` | Executed calibration & `VIAB-2` seeds |
| **Proposed `SEL2-P2` Sets (`95` unique seeds; `N=48` active confirmation + `12` baseline + `1` smoke)** | | | |
| `SEL2-P2` confirmation pool (cap `N <= 70`; **locked `N = 48`**) | **`900001 + 1000 * k` (`k = 0..47` -> `900001..=947001`; cap band `k <= 69` -> `969001`)** | `70` (max) | **FRESH / UNRUN** (`48` selected by `D5(a)`) |
| `SEL2-P2` `QD-SEL2g` pooled baseline | **`970501..=970512`** (`12` seeds, `min_surviving = 6` by `D7(a)`) | `12` | **FRESH / UNRUN** |
| `SEL2-P2` blind smoke (`C1..C5`) | **`999931`** (descending prime below `999953`) | `1` | **FRESH / UNRUN** |
| `SEL2-P2` `2,000`-tick pipeline check | `999701..=999706` (base), `999711` (smoke), `999721, 999722` (pairs) | `9` | **FRESH / UNRUN** |
| `SEL2-P2` synthetic unit test seeds (`viab2_probe_tests` & future freeze tests) | `614001, 614002, 614999` | `3` | Tiny synthetic unit tests only |

**Disjointness proof:** `|Prior_Itemized_Union| = 173` (`179` with E3b's reserved `k = 64..69` ceiling `874001..=879001`; `0` collisions across all repo `.json`/`.csv`/`.rs`/`.md` fixtures verified during `PR #247` review), `|SEL2P2_Union| = 95`, and:
1. `Prior_Repo_Union ∩ SEL2P2_Union = ∅` (`0` collisions).
2. Confirmation cap `[900001, 969001]` clears `VIAB-1/1b/2` max (`880012`) by **`+19,989`** and E3b's cap (`879001`) by **`+21,000`**, and stays strictly below `SEL2-P2` baseline `[970501, 970512]` (`970501 - 969001 = 1,500`, leaving `1,499` integers strictly between; locked `947001` leaves `23,500` gap).
3. `999931` (`prime`) sits strictly below `E3b` (`999953`), `SEL2-P` (`999961`), `E-SEL1` (`999971`), `E3` (`999979`), and `E2` (`999983`), and above `999801..=999822` and `999701..=999722`.
4. All `5` internal `SEL2-P2` subsets (`confirm_cap70`, `baseline_12`, `smoke`, `pipeline_check`, `synth_tests`) are strictly pairwise disjoint.

### 3.2. Analytical Power, Attrition Probability & Exact `R7` Tables

Exact paired-$t$ power (`scipy.stats.nct`, two-sided), exact Wilcoxon signed-rank minimum $p$-value ($2/2^n$), and binomial probability $P(n_{LL} \ge \lceil 0.85 N \rceil)$ of clearing the $\pi_{LL} \ge 0.85$ attrition floor (at conservative $\pi_{LL} = 0.90$, `VIAB-1` exact $11/12 \approx 0.9167$, and `0.95`; note blind `VIAB-2` subsequently achieved **`12/12`** survival in all arms):

| `N` | `k_min = ceil(0.85*N)` | $P(n_{LL} \ge k_{\min})$ at $\pi_{LL} = 0.90$ / exact `11/12` / `0.95` | `R7` $p_{\min}$ (`n = N`) | Holm $m=3$ `80%` MDE ($d_z$) | Single $m=1$ `80%` MDE ($d_z$) | Power at $d_z = 0.50$ (`m=3` / `m=1`) | Power at $d_z = 0.66$ (`m=3` / `m=1`) | Approx `80%` TOST $d_{\text{SESOI}}$ |
|---:|---:|---|---:|---:|---:|---|---|---:|
| `20` *(ref)* | `17` | — | `1.91e-06` | `0.7835` | `0.6604` | `0.3725` / `0.5645` | `0.6307` / `0.7995` | `0.6835` |
| `24` | `21` | `0.7857` / `0.8653` / `0.9702` | `1.19e-07` | `0.7050` | `0.5972` | `0.4615` / `0.6504` | `0.7376` / `0.8720` | `0.6192` |
| `32` | `28` | `0.7885` / `0.8765` / `0.9796` | `4.66e-10` | `0.6000` | `0.5113` | `0.6192` / `0.7823` | `0.8779` / `0.9511` | `0.5312` |
| **`48` *(D5(a))*** | **`41`** | **`0.8979` / `0.9565` / `0.9976`** | **`7.11e-15`** | **`0.4818`** | **`0.4129`** | **`0.8324` / `0.9240`** | **`0.9792` / `0.9941`** | **`0.4298`** |
| `64` | `55` | `0.8972` / `0.9616` / `0.9988` | `1.08e-19` | `0.4139` | `0.3557` | `0.9350` / `0.9761` | `0.9971` / `0.9994` | `0.3706` |

- **Compute projection (`Windows 1`, release, `--jobs 8`, `fl=0` empirical `VIAB-1` & `VIAB-2` timing):**
  - Parallel `--jobs 8` `VIAB-2` (`48` runs across `12` seeds × `4` arms at `200k` ticks) completed in `1001.74 s` (`16.7 min`), so `N = 48` confirmation pairs (`144` runs across `selected` / `scarce_mut0` / `scarce_drift`) plus `12` `QD-SEL2g` baseline runs is projected at `~30–40 min` on `Windows 1`. (`fl=2` ran slower at `5027.5s` and was not triggered.)

### 3.3. Closed `R10` Decision Table (Two-Sided `H1`)

Every confirmation dataset maps uniquely onto `{supports, equivalent_to_zero, insufficient_evidence}` (with `supports` further tagged positive or negative in `direction_tag`):

| Gates (`Smoke C1..C5` + `Host`) | Attrition & `drift_valid` Rule Valid (`SEL2P2-D6`) | `p_adj < 0.05` | Effect Magnitude vs. `SESOI` | TOST 90% CI Strictly Inside `(-SESOI, +SESOI)` | `R10` Label | `direction_tag` |
|---|---|---|---|---|---|---|
| Fail | Any | Any | Any | Any | **`insufficient_evidence`** | `harness_invalid` |
| Pass | Fail | Any | Any | Any | **`insufficient_evidence`** | `attrition_below_floor_or_drift_invalid` |
| Pass | Pass | Yes | `\|Delta\| >= SESOI` | No | **`supports`** | `two_sided_positive` / `two_sided_negative` |
| Pass | Pass | Yes | `\|Delta\| < SESOI` | Yes | **`equivalent_to_zero`** | `trivial_below_sesoi_equivalent` |
| Pass | Pass | Yes | `\|Delta\| < SESOI` | No | **`insufficient_evidence`** | `inconclusive_straddles_sesoi` |
| Pass | Pass | No | `\|Delta\| < SESOI` | Yes | **`equivalent_to_zero`** | `equivalent_to_zero_powered_null` |
| Pass | Pass | No | Any | No | **`insufficient_evidence`** | `inconclusive_wide_ci` |
| Pass | Pass | Any | `\|Delta\| >= SESOI` | Yes *(guard)* | **`insufficient_evidence`** | `ci_inconsistent_impossible` |

---

## 4. Blind Smoke Gate (`C1..C5`, `seed = 999931`) & Execution Contract

1. **Single execution host (`SEL2P2-D5`):** Confirmation and smoke run on one deterministic release binary (`Windows 1` recommended, matching `E3-X` and `E3b`), with `ANIMA_WORLD_ARTIFACT` unset, `%TEMP%\anima_world_artifact.anmw` absent, `ANIMA_USE_GPU=0`, `terrain_source == "generated"`, identical `world_identity`, `max_abs_eu_drift <= 1.0e-6 EU`, and an external resumable `--checkpoint-dir` outside `--out`.
2. **Pre-smoke pooled baseline (`QD-SEL2g`, `970501..`):** Must run and compute `pooled_crop_second_half` before any scarce smoke or confirmation observable is emitted; fails `C2` automatically if surviving baseline count `< min_surviving_baseline_seeds`.
3. **Blind smoke (`seed = 999931`, `sample_period = 50`, `body_mass_consulted: false`, `0` `trait_mass` substrings):** Evaluates `C1..C5` (`min_agents >= 5`, `g in [0.4, 0.6]`, `generations >= 30`, `starvation_share > 0.50`, `last_quarter_births > 0 && deaths > 0`) plus `drift_valid`. Hard stop if any smoke gate fails: `0` confirmation seeds run.

---

## 5. What `SEL2-P2` Will NOT Show (Scope Boundaries)

1. **`headless adapter verified` only:** Not evidence for the desktop GUI app, GPU/Bevy runtime scheduling, or live multi-threaded simulation.
2. **Single herbivore trophic slice without neural evolution:** `live.evolved_brains = 0.0` and `live.predator_fraction = 0.0`; excludes brain plasticity and predator–prey arms races (`E2` / `E3b`).
3. **No genotype-to-phenotype plasticity separation:** `DevelopedPhenotype` is absent; effects operate exclusively on additive `AgentGenotype.0.total_mass()`.
4. **Structural deletion asymmetry (if `D1(a)` chosen):** Under `live.founder_genotype = 1.0`, `operator 4` cannot shrink bodies below `2` nodes (`mass = 2.5`), though Gaussian size/mass mutations act continuously in both directions.
5. **Isolated parameter slice & `Generated` terrain:** Results bind strictly to `16` spread lakes, `lake_layout = 1`, `dehydration_ticks = 960`, `strict_prey_cap = 1`, `prey_cap = 320`, and `MapSettings::default()` generated terrain — not World v2 (`ANMW v3`) before `WV2-7`.

---

## 6. GATE — Owner Decisions (`QD-SEL2P2-D1` through `QD-SEL2P2-D7`, Locked `2026-11-24`)

All seven decisions below were approved by the owner (`"duyệt theo khuyến nghị"`, recorded in Hub `MSG_20261124_080000` at `2026-11-24T08:00:00Z`), and the mandatory blind `VIAB-2` gate passed ([2026-11-24-sel2-p2-viab2-results.md](2026-11-24-sel2-p2-viab2-results.md)). Every unchosen option is retained below and explicitly marked *(not chosen)* for full provenance. The dedicated freeze card (fixtures, manifest tests, verifier, freeze doc) runs next.

### `SEL2P2-D1` — Founder Genotype (`live.founder_genotype`) and Mandatory `200,000`-Tick `VIAB-2` Check
- **Owner Decision (`QD-SEL2P2-D1`):** **`(a)`** *(owner, 2026-11-24, `MSG_20261124_080000`)* — Set `live.founder_genotype = 1.0` across all arms AND pass the blind `200,000`-tick `VIAB-2` pre-check on `880001..=880012` (`>= 11/12` survival in `scarce_mut0`, `selected`, and `scarce_drift`, `0` `trait_mass`). **`VIAB-2` PASSED (`12/12` survival across all four arms, `min_N >= 59`, `0` `trait_mass`)**; `mutation.rs:211` `nodes.len() > 2` operator-4 wall disclosed in [§1.2](#12-blind-survival--gate-summary-from-viab-1-pr-215--viab-1b-pr-228) and [§5](#5-what-sel2-p2-will-not-show-scope-boundaries).
- **Context:** `VIAB-1` proved the `3`-segment clone (`founder_genotype = 0`) causes `scarce_mut0` to collapse (`0..1/12` at `200k` ticks). `VIAB-1b` proved `founder_genotype = 1` rescues `mut0` (`12/12`, `min_N >= 108`) at `18,000` ticks, and `VIAB-2` (`viab2_fg1_s075_k2k_fl0`) extended that proof to `200,000` ticks (`12/12` survival across all arms).
- **Options:**
  - **(a) [CHOSEN]** Set `live.founder_genotype = 1.0` across all arms AND require one blind `200,000`-tick **`VIAB-2`** check (`12` seeds `880001..=880012`, `0` `trait_mass`) verifying `scarce_mut0`, `selected`, and `scarce_drift` all survive `>= 11/12` before freeze. Keep `scarce_mut0` (`H2`, `H3`) intact; disclose the `mutation.rs:211` operator-4 wall.
  - **(b) *(not chosen)*** Drop `scarce_mut0` and run a **two-arm** design (`selected` vs. `scarce_drift`) under either `founder_genotype = 0` or `1` (avoids needing `mut0` to survive `200k`, at the cost of losing the clonal genetic-freeze baseline and Robertson–Price `H3` transmission-bias arm).
  - **(c) *(not chosen)*** Authorize new Rust work for an initialized multi-genotype adapted founder pool before `SEL2-P2` freeze.
- **`anima-case` Recommendation:** Option `(a)` *(approved by owner)*.

### `SEL2P2-D2` — Scarcity Scale (`resource_capacity_scale`), Founder Layout (`founder_layout`) & Drift Cadence
- **Owner Decision (`QD-SEL2P2-D2`):** **`(a)`** *(owner, 2026-11-24, `MSG_20261124_080000`)* — `resource_capacity_scale = 0.75`, `founder_layout = 0`, `drift_reset_ticks = 2000`; escalate to `(b)` (`founder_layout = 2`) only if `VIAB-2` at `fl = 0` falls short on `min_N` or `C2`. **Fallback `(b)` NOT triggered:** `VIAB-2` achieved `12/12` `min_N >= 10` in every scarce arm (lowest `min_N = 59` in `scarce_mut0`, `63` in `selected`/`scarce_drift`) and `11/12` `C2` pass in `selected` (`12/12` in `scarce_mut0`; sole `C2` edge miss at seed `880004` `g_ratio = 0.370941`), with `min_n_shortfall = false`, `c2_shortfall = false`, and `d2b_escalation_recommended = false`.
- **Options:**
  - **(a) [CHOSEN]** `scale = 0.75`, `founder_layout = 0`, `drift_reset_ticks = 2000`.
  - **(b) *(not chosen — fallback not triggered)*** `scale = 0.75`, `founder_layout = 2`, `drift_reset_ticks = 2000` (`sel2p_b2_s075_k2k_fl2_12s`: `12/12` `selected` & `scarce_drift` survival, `0` drift misses, `11/12` `C2`, but `baseline_ref` dropped to `9/12` and runtime ~`8.6x` slower).
  - **(c) *(not chosen)*** `scale = 0.63`, `founder_layout = 0`, `drift_reset_ticks = 2000` (retains `SEL2-P`'s `0.63` scale despite `83.3%` joint survival `10/12` and higher starvation).
- **`anima-case` Recommendation:** Option `(a)` *(approved by owner; fallback not triggered)*.

### `SEL2P2-D3` — Confirmatory Hypothesis Family (`m = 3` Holm vs. `m = 1` Single Primary `H1`)
- **Owner Decision (`QD-SEL2P2-D3`):** **`(a)`** *(owner, 2026-11-24, `MSG_20261124_080000`)* — `m = 3` confirmatory family `{H1 (selected/scarce_drift), H2 (selected/scarce_mut0), H3 (scarce_drift/scarce_mut0)}` with Holm–Bonferroni $\alpha = 0.05$.
- **Options:**
  - **(a) [CHOSEN]** `m = 3` confirmatory family `{H1 (selected-drift), H2 (selected-mut0), H3 (drift-mut0)}` with Holm–Bonferroni $\alpha = 0.05$ (`0.016667` first step — matches template `#214` and frozen `E3b`).
  - **(b) *(not chosen)*** `m = 1` single confirmatory primary hypothesis `H1 (selected - scarce_drift)` at $\alpha = 0.05$, treating `H2` and `H3` as secondary (BH-FDR `q = 0.05`) — recovers `~15%` higher power at fixed `N`.
- **`anima-case` Recommendation:** Option `(a)` *(approved by owner)*.

### `SEL2P2-D4` — Primary Endpoint Scale, SESOI Magnitude & TOST Equivalence Margin
- **Owner Decision (`QD-SEL2P2-D4`):** **`(b)`** *(owner, 2026-11-24, `MSG_20261124_080000`)* — Primary endpoint **`ln(z_sel / z_drift)`** (generalized $\ln(\overline{z}_{B,i}^{\text{int}} / \overline{z}_{A,i}^{\text{int}})$ across `H1`/`H2`/`H3`), **`SESOI = ±ln(1.10)`** (`±0.09531018`, `[0.909x, 1.100x]`).
- **Options:**
  - **(a) *(not chosen)*** Raw post-warmup time-integrated difference `Delta = mean_int(selected) - mean_int(scarce_drift)` in EU/mass units, with **SESOI $= \pm 0.125$** (`±5%` of `2.5` two-seg founder, continuity with `SEL2-P`'s `5% * |control_mean|` criterion while keeping bounds fixed a priori without peeking at control outcomes) + paired TOST at `90% CI inside (-0.125, +0.125)`.
  - **(b) [CHOSEN]** Log mean ratio $\ln(\overline{z}_{\text{sel}}^{\text{int}} / \overline{z}_{\text{drift}}^{\text{int}})$ with **SESOI $= \pm \ln(1.10) = \pm 0.0953$** (`[0.909x, 1.100x]` body-mass ratio).
  - **(c) *(not chosen)*** Standardized paired difference `|d_z| >= 0.50` (`±0.50 * sigma_hat`) as TOST equivalence margin.
- **`anima-case` Recommendation:** Option `(b)` *(approved by owner)*.

### `SEL2P2-D5` — Confirmation Sample Size `N` (`900001 + 1000*k`, `N <= 70`) & Execution Host
- **Owner Decision (`QD-SEL2P2-D5`):** **`(a)`** *(owner, 2026-11-24, `MSG_20261124_080000`)* — **`N = 48`** pairs, seeds `900001 + 1000*k`, `k = 0..47` (`900001..=947001`, `144` confirmation runs), executed on `Windows 1`.
- **Options:**
  - **(a) [CHOSEN]** `N = 48` pairs (`900001..=947001`, `144` runs; `>= 80%` Holm `m=3` power down to `d_z = 0.4818` and `P(n_LL >= 41) = 95.65%` at `11/12` pair survival; `~30–40 min` on `Windows 1` with `12` baseline runs).
  - **(b) *(not chosen)*** `N = 64` pairs (`900001..=963001`, `192` runs; matches `E3b` power down to `d_z = 0.4139` and `P(n_LL >= 55) = 96.16%`; `~40–55 min` on `Windows 1`).
  - **(c) *(not chosen)*** `N = 32` pairs (`900001..=931001`, `96` runs; `87.8%` power at `d_z = 0.66`, only `61.9%` at `d_z = 0.50` under `m=3`).
- **`anima-case` Recommendation:** Option `(a)` *(approved by owner)*.

### `SEL2P2-D6` — Attrition Primary Rule (`R3`), Worst-Rank Fallback & `drift_valid` Shortfall Rule
- **Owner Decision (`QD-SEL2P2-D6`):** **`(a)`** *(owner, 2026-11-24, `MSG_20261124_080000`)* — Align strictly with `#214` Template `R3`: complete pairs primary iff `p_ext <= 5%` and `pi_LL >= 0.85`, with Lee bounds and `RMST(200k)`; else Lachin worst-rank over all `N`; require `drift_valid == true` and `has_shortfall == false` per contrast.
- **Options:**
  - **(a) [CHOSEN]** Align strictly with `#214` Template `R3`: use **complete pairs as primary iff observed extinction `p_ext <= 5%` AND `pi_LL >= 0.85`** (`VIAB-2` observed `0/12` extinction in all four arms under `fl=0, s=0.75`), plus Lee (2009) bounds and `RMST(200k)`; otherwise switch primary to **Lachin worst-rank across all `N` seeds** (requiring `drift_valid == true` and complete `has_shortfall == false` on paired runs, marking shortfall seeds invalid per-contrast).
  - **(b) *(not chosen)*** Match frozen `E3b` (`D3`): **complete-pair primary** (`pi_LL >= 0.85`, per-contrast exclusion) paired with Lee bounds + `RMST(200k)` regardless of `5%` extinction rate.
- **`anima-case` Recommendation:** Option `(a)` *(approved by owner)*.

### `SEL2P2-D7` — `QD-SEL2g` Baseline Reference Count (`12` vs. `6` Seeds) & Smoke `C1..C5` Thresholds
- **Owner Decision (`QD-SEL2P2-D7`):** **`(a)`** *(owner, 2026-11-24, `MSG_20261124_080000`)* — `QD-SEL2g` pooled baseline of `12` seeds `970501..=970512`, `min_surviving_baseline_seeds = 6`; smoke thresholds `C1 >= 5`, `C2 in [0.4, 0.6]`, `C3 >= 30`, `C4 > 0.50`, `C5 > 0`.
- **Options:**
  - **(a) [CHOSEN]** Upgrade `QD-SEL2g` baseline pool to **`12` seeds `970501..=970512`** (`min_surviving_baseline_seeds = 6`) while keeping `C1 >= 5`, `C2 in [0.4, 0.6]`, `C3 >= 30` gen, `C4 > 0.50` starvation, `C5` last-quarter births & deaths `> 0` (halves baseline reference Monte Carlo variance relative to `6` seeds).
  - **(b) *(not chosen)*** Keep **`6` baseline seeds `970501..=970506`** (`min_surviving_baseline_seeds = 3`), matching frozen `SEL2-P`.
- **`anima-case` Recommendation:** Option `(a)` *(approved by owner)*.
