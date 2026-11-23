---
phase: planning
feature: experiment-e3b-productivity-gradient
title: Draft Preregistration (v2) — E3b: Spatial Productivity Gradient and Body-Mass Variance Ratio
description: Updated documents-only E3b draft incorporating Owner-approved QD-E3B decisions D1-D5, VIAB-1b non-mass nuisance calibration (2-segment founder, predator persistence 12/11/12, ~3.1/2.1/3.0 generations), log variance ratio L_i with SESOI +/-ln(1.5) and TOST at N=64, re-run R6b Monte Carlo, and R1/r_c bug fixes (awaiting Owner sign-off before freeze)
status: proposed
owner: anima-case
last_reviewed: 2026-11-23
---

# Draft Preregistration (v2) — E3b: Spatial Productivity Gradient and Body-Mass Variance Ratio

> **Status: `proposed` (documents only — DO NOT FREEZE until Owner approves `E3B-P-v2` and signs off on the two `VIAB-1b` findings below).**
> - **No code, manifests, fixtures, or scripts are modified by this PR.** Only this document is updated on trunk `98054ba206d275014ecce89e7ab7c2ad603ead1d`.
> - **Confirmation seeds executed:** `0` (`810001..=873001` and smoke seed `999953` have never been run).
> - **Body-mass blindness (`QD-VIAB`):** `anima-case` measured, read, or consulted `0` body-mass numbers (`live.trait_mass_*`) from [E3-X results](2026-11-20-e3-x-confirmation-results.md), [VIAB-1](2026-11-23-viab1-results.md), or `VIAB-1b` (`PR #228`). All `VIAB-1b` metrics here use exclusively non-mass fields (`0` `trait_mass` substrings). Founder choices cite only the lead's diagnosis in [2026-11-23-mut0-collapse-diagnosis.md](2026-11-23-mut0-collapse-diagnosis.md) (`PR #216`).

---

## Pre-Freeze Notice — Two Empirical Consequences of `VIAB-1b` (`e3b_fg1_g4_fl1`) Requiring Owner Sign-Off Before Freeze

Under merged `FOUNDER-2SEG` (`PR #224`, `live.founder_genotype = 1`), `VIAB-1b` (`PR #228`, `docs/ai/planning/2026-11-23-viab1b-results.md` and `artifacts/experiments/viab1b-founder-2seg/cells/e3b_fg1_g4_fl1.json`, `12` calibration seeds `880001..=880012` at trunk `98054ba2`) resolved the `mut0` viability collapse (`12/12` survival in `U/S/T` for both `mut=0.05` and `mut=0.0`), but shifted **two factual assumptions** of `QD-E3B` (`PR #223`):

1. **`D1` ecological shift — Predators persist (`12/11/12`), NOT effectively herbivore-only:**
   - Under the 3-segment founder (`e3_b1_g4_fl1`, `VIAB-1`), predators went extinct in `12/12` seeds in every arm (`pred_final > 0`: `0/0/0`).
   - Under the 2-segment founder (`e3b_fg1_g4_fl1`, `VIAB-1b`), **predators persist to `T = 18,000` in `12/12` (`U`: `3..11`, mean `8.00`), `11/12` (`S`: `0..8`, mean `2.50`, extinct only in seed `880004`), and `12/12` (`T`: `1..8`, mean `5.67`)** (and identically `12/11/12` in `e3b_fg1_g4_fl1_mut0`).
   - **Pre-freeze consequence:** Saying E3b is *"effectively herbivore-only"* is **false** under `live.founder_genotype = 1`. Body-mass variance is measured with **active predators present at different final abundances across arms**, and `live.shannon` (`S4`) is no longer structurally forced to `0`. Owner may either sign off on running E3b as a **two-trophic (predators-present)** experiment under `live.predator_fraction = 0.3`, or reverse `D1` to `0.0` (which would require a 1-cell `~82 s` `VIAB-1` calibration run before freezing).
2. **Shorter realized horizon (`~3.1 / 2.1 / 3.0` generations vs. `~4.0 / 1.9 / 4.0`):**
   - Because equilibrium `mean_N` roughly doubled with the adapted 2-segment founder (`178.78 / 167.10 / 178.43` vs. `100.4 / 79.8 / 99.5` in `VIAB-1`), realized generation turnover (`total_births / mean_n`) in `18,000` ticks shifted to **`3.091 / 2.111 / 3.041` (`U / S / T`)** (`3.170 / 2.178 / 2.993` in `mut0`). Note the `~1.44x` birth-turnover asymmetry between `T` and `S` (`ln(births_T / births_S) = +0.4396 +/- 0.1557`, range `+0.2684..+0.7459`).
   - **Why `T = 18,000` and `N = 64` are kept unchanged:** Keeping `duration_ticks = 18,000` preserves exact `VIAB-1b` calibration (`12/12` survival, `12/12` `WP-2` crop match, `~7–16 min` Windows 1 budget) without uncalibrated horizon extrapolation, preserves comparability of tick horizon with E3/E3-X, and directly accommodates `~2.1–3.1` generations of *de novo* input via D4's mandatory `ln(births_T / births_S)` mediator analysis.

---

## 0. Provenance, Decisions & Required Inputs

| Role | Artifact / Reference | Revision | Body-Mass Values Used |
|---|---|---|---|
| Calibration basis (`VIAB-1b`) | `docs/ai/planning/2026-11-23-viab1b-results.md` & `artifacts/experiments/viab1b-founder-2seg/cells/{e3b_fg1_g4_fl1,e3b_fg1_g4_fl1_mut0}.json` (`PR #228`) | Trunk `98054ba2` (`PR #228` ready `c2e0b6e0`) | **None (`0` `trait_mass` matches across all outputs)** |
| Historical 3-seg calibration | [VIAB-1 Results](2026-11-23-viab1-results.md) (`e3_b1_g4_fl1.json`, `PR #215`) | Phase B `4794b6fc` (`d49cf4ff`) | None (`0` `trait_mass` matches) |
| Owner decisions `D1`–`D5` | `QD-E3B` in [AGENT_WORKFLOW.md](../../how-to/AGENT_WORKFLOW.md) (`PR #223`, `bd73c96`) & `MSG_20261123_140000` / `153500` | `bd73c96` / `98054ba2` | None |
| 2-segment founder implementation | [2026-11-23-founder-2seg.md](2026-11-23-founder-2seg.md) (`PR #224`) | Trunk `98054ba2` | Declared schema/genesis constant `1.5 + 1.0 = 2.5` only |
| Founder diagnosis & provenance | [2026-11-23-mut0-collapse-diagnosis.md](2026-11-23-mut0-collapse-diagnosis.md) (`PR #216`) | `9062a7c3` | Cited `3.3 -> ~2.5` mass diagnosis on probe seeds `880001..=880006` only |
| Frozen predecessor & E3-X | [Frozen E3 Plan](2026-08-26-experiment-e3-productivity-gradient-body-mass.md) & [E3-X Results](2026-11-20-e3-x-confirmation-results.md) | `9958f27` / `24c6fb1c` | **None consulted** (`S/T/U` survival & timing only) |
| Statistics template | [2026-11-23-prereg-statistics-template.md](2026-11-23-prereg-statistics-template.md) (`PR #214`) & [Statistics Research §3.3–3.5](../../research/VIABILITY_PRICE_STATISTICS_RESEARCH_2026-11-22.md) | `3fee64d5` | None |

---

## 1. World Configuration, Locked `D1` & `D5` Disclosures (`31` `LIVE_KEYS`)

### 1.1. Exact Configuration on Trunk `98054ba2`

E3b builds on `core::live_experiment::LiveExperimentAdapter` (`LIVE_MODEL_VERSION = "live-bevy-world/2"`, snapshot schema `8`) with `MapSettings::default()` (`seed = 1337`, `256x256` `Generated` terrain on `Windows 1 x86_64`, observed `WorldIdentity` checksum `3152323152`, informational only and never pinned across OS/architectures). Relative to frozen E3 (`9958f27`), `e3b_fg1_g4_fl1` introduces **8 overrides** across all **`31` `LIVE_KEYS`** on trunk `98054ba2`:

| Key | Frozen E3 (`9958f27`) | E3b (`e3b_fg1_g4_fl1`) | Role of Change |
|---|---|---|---|
| `live.strict_prey_cap` | `0.0` (absent) | **`1.0`** | Enforces hard `prey_cap` birth ceiling (eliminates boom–bust spikes) |
| `live.founders` | `120.0` | **`160.0`** | Matches initial population symmetrically to `prey_cap = 160` |
| `live.prey_cap` | `80.0` (default) | **`160.0`** | Equilibrium herbivore cap matching `founders = 160` under `strict_prey_cap = 1` |
| `live.lakes` | `2.0` | **`16.0`** | Increases water source count to decouple dehydration crashes from food scarcity |
| `live.lake_layout` | `0.0` (absent) | **`1.0`** | Distributes all `16` lakes on a 2D spatial grid rather than one row |
| `live.dehydration_ticks` | `120.0` (default) | **`960.0`** | Gives surviving agents sufficient transit budget between 2D lakes |
| `live.founder_layout` | `0.0` (absent) | **`1.0`** | Spreads founders on a 2D land-only grid (never stacked on ocean tiles) |
| **`live.founder_genotype`** | `0.0` (absent, 3-seg `3.3`) | **`1.0`** (`D5` Opt. `(b)`) | Declared 2-segment founder (`nodes [0, 1]`, masses `1.5 + 1.0 = 2.5`; PR `#224`) |

All **31 `LIVE_KEYS`** on trunk `98054ba2` (`LIVE_KEYS: [&str; 31]`) partition completely (`8 + 11 + 12 = 31`):

- **8 changed / newly activated keys (table above):** `live.strict_prey_cap = 1.0`, `live.founders = 160.0`, `live.prey_cap = 160.0`, `live.lakes = 16.0`, `live.lake_layout = 1.0`, `live.dehydration_ticks = 960.0`, `live.founder_layout = 1.0`, **`live.founder_genotype = 1.0`**.
- **11 keys carried over explicitly from frozen E3:** `live.predator_fraction = 0.3` (`D1`), `live.ecological_fitness = 1.0`, `live.mortality = 1.0`, `live.reproduction = 1.0`, `live.locomotion = 1.0`, `live.drive_floor = 0.5`, `live.thirst_steering = 1.0`, `live.evolved_brains = 0.0`, `live.trees = 8.0`, `live.food_cap = 50.0`, and per-arm `live.resource_capacity_scale` (`1.0` in `U` & `T`, `0.42` in `S`).
- **12 keys at `LiveWorldConfig::default()` (`0` / absent):** `live.w_surplus = 1.0`, `live.starve_ticks = 120`, `live.mutation_rate = 0.05`, `live.plant_reserve = 0.0`, `live.kill_hydration = 0.0`, `live.kill_efficiency = 0.40`, `live.target_min_energy = 0.0`, `live.drift_control = 0.0`, `live.drift_reset_ticks = 0`, `live.land_mask = 0` (`false`), `live.grass_steering = 0` (`false`), and **`live.lineage_tracking = 0` (`false`, default-absent, F-066)**. Note that `grass_steering` and `land_mask` remain off (`QD-VIAB1C`).

### 1.2. Locked `D1`: `live.predator_fraction = 0.3` (Two-Trophic Persistence Disclosure)

Per Owner decision `D1`, E3b keeps **`live.predator_fraction = 0.3`** (`round(160 * 0.3) = 48` initial predators and `112` initial herbivores). Contrary to the pre-`VIAB-1b` expectation from 3-segment `e3_b1_g4_fl1` (`pred_final > 0`: `0/12` in `U`, `0/12` in `S`, `0/12` in `T`), **`VIAB-1b` proves that under `live.founder_genotype = 1`, predators persist to `T = 18,000` in almost every run**:

| Metric (`880001..=880012`) | `U` (`control_uniform`) | `S` (`control_uniform_scarcity`) | `T` (`treatment_gradient`) | Source |
|---|---:|---:|---:|---|
| 3-seg `e3_b1_g4_fl1` (`pred_final > 0`) | `0/12` | `0/12` | `0/12` | [VIAB-1](2026-11-23-viab1-results.md) |
| **2-seg `e3b_fg1_g4_fl1` (`pred_final > 0`)** | **`12/12`** | **`11/12`** (`0` on `880004`) | **`12/12`** | `VIAB-1b` (`PR #228`) |
| 2-seg `pred_final` mean ± SD (`min..max`) | `8.00 ± 2.37` (`3..11`) | `2.50 ± 2.15` (`0..8`) | `5.67 ± 2.23` (`1..8`) | `e3b_fg1_g4_fl1.json` |
| 2-seg `mut0` (`pred_final > 0`, `min..max`) | `12/12` (`4..11`) | `11/12` (`0..7`, `0` on `880009`) | `12/12` (`1..8`) | `e3b_fg1_g4_fl1_mut0.json` |

**Mandatory `D1` disclosure & reporting obligations:**
1. Every E3b report must state that E3b runs with **persistent predators (`~2.5` to `8.0` final predators across arms)** rather than as an herbivore-only run, and report `live.predator_count` (`U, S, T`) as an exploratory ecological covariate alongside `H1..H3`.
2. `live.shannon` (`S4`, two-trophic herbivore–predator Shannon balance in `live_experiment.rs`) is no longer structurally forced to `0.0` at `T = 18,000` (`11..12/12` seeds retain non-zero predator counts), though it still measures herbivore–predator evenness rather than morphological body-mass niche diversity and carries `decision_effect: none`.

### 1.3. Locked `D5`: 2-Segment Founder, Provenance, Operator-4 Wall & Interpretation Wording

Per Owner decision `D5` (Option `(b)`), every run uses `live.founder_genotype = 1.0` ([2026-11-23-founder-2seg.md](2026-11-23-founder-2seg.md), merged in `PR #224`):

- **Exact genotype:** genesis 3-segment clone minus the tail leaf — **`nodes = [{id: 0, length: 1.0, radius: 0.2, mass: 1.5}, {id: 1, length: 1.0, radius: 0.2, mass: 1.0}]`**, **`edges = [{source_node: 0, target_node: 1, joint_anchor: [1.0, 0.0, 0.0], joint_axis: [0.0, 0.0, 1.0]}]`**, **`total_mass = 2.5`**, canonical `serde_json` SHA-256 **`dfeb276205f818acf3f34f5d716cbf163598a03a23ffc6964ff85753e280c719`**. Initial `live.trait_mass_var` at tick `0` is `0.0` in all three arms, and restore does not re-roll genotypes.
- **Disclosed operator-4 wall (`src-tauri/src/evolution/mutation.rs:211`):** Structural node removal in [`mutate_genotype`](../../../src-tauri/src/evolution/mutation.rs#L210-L230) checks `if genotype.nodes.len() > 2`. Because the `live.founder_genotype = 1` founder has exactly `2` nodes, it sits directly on the operator-4 wall: **structural mutation can only add segments (growing `>= 3` segments), never prune down to 1 segment** (continuous node-mass/size Gaussian mutations still operate bilaterally).
- **Disclosed founder provenance (`PR #216` -> `PR #224` -> `VIAB-1b`):** Selecting the 2-segment founder came directly from the lead's diagnosis in [2026-11-23-mut0-collapse-diagnosis.md](2026-11-23-mut0-collapse-diagnosis.md) (`PR #216`), which inspected body mass on exploratory probe seeds `880001..=880006` (`3.3 -> ~2.5` rapid selective sweep under the 3-segment clone). Those probe seeds **overlap the `880001..=880012` calibration range** used in `VIAB-1` and `VIAB-1b`, but are **100% disjoint from E3b's confirmation seeds `810001..=873001`**. Subsequent blind `VIAB-1b` tests at trunk `98054ba2` (`mut = 0.05` and `mut = 0.0`, `0` `trait_mass` consulted) confirmed `12/12` survival across `U`, `S`, and `T` (`min_N >= 58` with mutation, `min_N >= 108` with `mut0`), closing the `PR #216` fragility concern.
- **Mandatory biological interpretation wording:** Because initial variance is `0.0` and only `~2.1–3.1` realized generations elapse in `18,000` ticks, all E3b reports are restricted to stating:
  > **"body-mass variance ratio after ~k generations (`~3.1 / 2.1 / 3.0` in `U / S / T`) of *de novo* input from an adapted clonal founder (`live.founder_genotype = 1`, with persistent predators)"**
  — never *"long-run evolutionary diversification"* or *"standing genetic sorting"*, and noting `T`'s `Deforest 0.30` / `AddNutrient 1.00` mechanism vs `S`'s genesis `resource_capacity_scale = 0.42`.

---

## 2. Arms, Primary Log Variance Ratio Metric `L_i` & Exploratory Family (`D2`, `D4`)

### 2.1. Three Matched Arms (`U`, `S`, `T`)

| Item | `control_uniform` (`U`) | `control_uniform_scarcity` (`S`) | `treatment_gradient` (`T`) |
|---|---|---|---|
| `live.resource_capacity_scale` | `1.0` (`153,587.25 EU` plants) | **`0.42`** (`64,506.64 EU` plants + `89,080.61 EU` detritus) | `1.0` (`153,587.25 EU` plants) |
| Intervention 1 (`id=1, Deforest`, West `0..=127`) | `intensity = 0.0` | `intensity = 0.0` | **`intensity = 0.30`** |
| Intervention 2 (`id=2, AddNutrient`) | `Rect{128,0,255,255}`, `intensity = 0.0` | **`Rect{0,0,255,255}` (whole map), `intensity = 1.00`** | **`Rect{128,0,255,255}` (East), `intensity = 1.00`** |
| Schedule (`start_tick / duration / curve`) | `1 / 18000 / Step` | `1 / 18000 / Step` | `1 / 18000 / Step` |

### 2.2. Primary Metric & Pseudocount Calibration (`D4`)

Let $V_{A,i}$ denote `live.trait_mass_var` at `T = 18,000` (`final_observables`) in arm $A \in \{U, S, T\}$ on valid seed $i$ (`survived && live.agent_count >= 2` at `T`). For any ordered contrast $(B - A)$, the primary per-seed endpoint is the **log variance ratio**:

$$L_i(B/A;\,\varepsilon) = \ln\!\left(\frac{V_{B,i} + \varepsilon}{V_{A,i} + \varepsilon}\right)$$

- **Scale label & interpretation:** $L_i$ is explicitly a **per-seed log variance ratio** ($\exp(\bar{L}) = \text{geometric mean variance ratio}$); zero-variance clonal-monomorphic survivors are regularized symmetrically by **$\varepsilon = 5 \times 10^{-4}$** (fixed before freeze).
- **Mandatory pseudocount sensitivity check:** Recompute and report $\bar{L}$, TOST 90% CI, and Holm $p$-values at **$\varepsilon_{\text{sens}} = 5 \times 10^{-5}$** on the exact same complete-pair set (diagnostic sensitivity only; never overrides primary $\varepsilon = 5 \times 10^{-4}$).
- **Confirmatory Primary Family (`m = 3`, Holm–Bonferroni $\alpha = 0.05$, Option `2A` + `(a)`):**
  1. **`H1` (`T / S`: $L_i = \ln[(V_{T,i}+\varepsilon)/(V_{S,i}+\varepsilon)]$, directional prediction `T > S`, $\mathbb{E}[L_i] > 0$):** Isolates the spatial productivity gradient at matched ~`64k EU` standing crop. Mapped via `D2` Option `(a)` (`4`-label R10 set).
  2. **`H2` (`T / U`: $L_i = \ln[(V_{T,i}+\varepsilon)/(V_{U,i}+\varepsilon)]$, two-sided):** Total gradient + scarcity effect relative to abundant uniform control.
  3. **`H3` (`S / U`: $L_i = \ln[(V_{S,i}+\varepsilon)/(V_{U,i}+\varepsilon)]$, two-sided):** Uniform scarcity + whole-map `AddNutrient 1.00` refill effect relative to abundant uniform control.
- **Primary inferential test on $\bar{L}$:** Two-sided paired $t$-test (`df = n_LL - 1`) with **TOST equivalence test** against $\pm\ln(1.5)$ (90% $t$-CI), plus **exact/normal-approx Wilcoxon signed-rank on $L_i$** co-reported as a mandatory non-parametric robustness check. Across seeds valid in all 3 arms, $\bar{L}(T/U) = \bar{L}(T/S) + \bar{L}(S/U)$ holds algebraically identically per seed.

### 2.3. Exploratory Family (BH-FDR `q = 0.05`), Turnover Mediator & Secondary Metrics

Controlled separately from confirmatory Holm FWER (`decision_effect: none` on `H1..H3`), explicitly labeled exploratory under Benjamini–Hochberg `q = 0.05`:

1. **Mandatory turnover mediator analysis (`D4`):** Because `VIAB-1b` shows `T` averages `3.041` realized generations vs `2.111` in `S` (`ln(births_T / births_S) = +0.4396 +/- 0.1557`, range `+0.2684..+0.7459`), regress (OLS + Pearson/Spearman correlation, plus partial residual slope) **$L_i$ against $\ln(\text{births}_{B,i} / \text{births}_{A,i})$** across `H1` (`T/S`), `H2` (`T/U`), and `H3` (`S/U`), estimating how much of any $L_i$ difference is mediated by cumulative birth turnover (`live.births` 30-sample sum).
2. **Secondary / exploratory dispersion & mean metrics:**
   - Unregularized & Bessel-corrected raw paired differences (`var_T - var_S`, `var * n / (n - 1)`), `lnCVR` (log coefficient-of-variation ratio), standardized $d_z$, matched-pairs Kerby $r_c$ (with corrected thresholds `0.52` / `0.74`), and Hodges–Lehmann pseudomedian of $L_i$ with 95% (and 90%) BCa CIs.
   - `H1_tail_window`: equal-weight 7-sample tail mean over `[14400, 18000]` applied to `L_i`.
   - `S1` (`trait_mass_p90 - trait_mass_p10` at `T`), `S2` (`trait_mass_mean` at `T`), `S3` (`trait_mass_mean_east - trait_mass_mean_west` at `T`, subject to `empty_half_rule`), `S6` (`births` 30-sum, `deaths` 30-sum, **`live.mean_lifespan` from sampled `series.csv` at tick `18,000` + positive-death post-warmup mean — NEVER `final_observables`**, avoiding `F-053` drained-window structural zeros).
   - **Trophic predator covariate:** `live.predator_count` (`U, S, T` final and post-warmup mean) alongside `L_i`, plus separate survival `RMST` ($\tau = 18,000$) on `100%` of seeds (`D3`).
   - **Structural / integrity diagnostics (`decision_effect: none`):** `S4` (`live.shannon`), `S5` (`live.archive_coverage`, structurally `0`), `I1` (`live.closed_eu_total <= 1e-6`), and `M1` (`live.standing_crop` `WP-2 b1/b2`).

---

## 3. Pre-Registered Statistical Design — Template Rules `R1`–`R10`

| Rule | Status | Locked Value (`QD-E3B` + `VIAB-1b` Recomputed) |
|---|---|---|
| **R1** Unit, disjoint seeds & bug fix | Locked | Paired CRN by seed (`U, S, T`); **`seed_k = 810001 + 1000 * k` capped at `N <= 70`** (`k <= 69` prevents `k = 70` collision with `880001`); `N = 64` -> `810001..=873001`; smoke `999953` |
| **R2** Confirmatory & exploratory families | Locked (`D2`) | Confirmatory `m = 3` (`H1, H2, H3` on $L_i$, Holm $\alpha = 0.05$: `0.01667, 0.025, 0.05`; directional `H1` Opt. `(a)`); Exploratory BH-FDR `q = 0.05` + `ln(births_T/births_S)` mediator |
| **R3** Attrition & contrast rules | Locked (`D3`) | `VIAB-1b`: `0/12` (`0%`) extinct in `U, S, T` (`80%` UB `12.55%` arm, `4.37%` pooled `0/36`) -> **complete-pair `H1 (T-S)` primary** (`pi_LL >= 0.85, n_LL >= 7`); **`H2, H3` complete-pair primary + Lee (2009) bounds sensitivity + separate `RMST`** |
| **R4** SESOI & achieved MDE | Locked (`D4`) | **SESOI $= \pm\ln(1.5) = \pm 0.405465$** on $\bar{L}$ (`[0.667x, 1.500x]` variance ratio); **pre-registered achieved MDE $= 0.5 \cdot \hat{\sigma}_L$**; `d_z`/`r_c`/`lnCVR`/mean diff secondary only |
| **R5** Blind non-mass nuisance params | Concrete (`VIAB-1b`) | Strictly non-mass fields from `e3b_fg1_g4_fl1.json` & `e3b_fg1_g4_fl1_mut0.json` (`98054ba2`, `880001..=880012`, `0` `trait_mass`) |
| **R6** Sample size arithmetic | Locked (`D4`) | **$N = 64$ seed triples (`192` runs)**; exact `80%` Holm-1 MDE `0.4139 * sigma_L`; **`0.5 * sigma_L` gives `93.50%` superiority power over zero** (`>= 80%` superiority at `ln(1.5)` up to `sigma_L = 0.9796`; `>= 80%` TOST up to `sigma_L = 1.0966`; strict R10 `supports` is `~50%` at `ln(1.5)` and `99.8%` at `ln(2.0)`) |
| **R6b** Monte Carlo on `L` & `ln(1.5)` | Executed (`N_rep = 5,000`) | Re-run via `python -I` (`Appendix A`) across blind `sigma_L in [0.40, 1.10]`, Gaussian & `t_5`, `V1..V5` attrition, Lee bounds, and separate RMST |
| **R7** Minimum exact $p$ vs $\alpha/3$ | Concrete | Floor $n \ge 7$ ($2/2^7 = 0.015625 < 0.016667$); at `N = 64`, $2/2^{64} = 1.08 \times 10^{-19}$; at 85% validity floor ($n = 55$), $5.55 \times 10^{-17}$ |
| **R8** Full paired effect-size bundle | Concrete (`YES`) | Primary $\bar{L}$ (95% & 90% $t$-CI) + `eps = 5e-5` check; secondary $d_z$, $r_c$ (**corrected `0.5205` / `0.7421`**), HL + 95% BCa CI, `lnCVR`, mean diff, `% > 0`, mediator |
| **R9** TOST equivalence bounds | Locked (`D4`) | **90% $t$-CI of $\bar{L}$ strictly inside $(-\ln(1.5), +\ln(1.5)) = (-0.405465, +0.405465)$** ($\alpha_{\text{TOST}} = 0.05$ each tail) |
| **R10** Closed 4-label decision table | Locked (`D2` `(a)`) | `4`-label `{supports, opposite, equivalent_to_zero, insufficient_evidence}`, updated for complete-pair `pi_LL >= 0.85` & `L_i / +-ln(1.5)`; **`48/48` verified** |

### R1 — Experimental Unit, Disjoint Seeds & Collision Bug Fix

- **Experimental unit:** `seed` (`N = 64` independent deterministic RNG trajectories evaluated across `{U, S, T}` under CRN on `Windows 1 x86_64`).
- **Bug fix #1 (preventing `k = 70 -> 880001` calibration collision):** Previous drafts used `seed_k = 810001 + 1000 * k` with unbounded `k`. At `k = 70`, `810001 + 1000 * 70 = 880001`, which collides directly with `VIAB-1` / `VIAB-1b` calibration seeds (`880001..=880012`) and `PR #216` probe seeds (`880001..=880006`).
- **Locked confirmation seed range (`N = 64`, hard cap `N <= 70`):**
  - Rule: `seed_k = 810001 + 1000 * k` with an explicit **hard invariant `N <= 70` (`k <= 69`, guaranteeing `max_seed <= 879001 < 880001`)**.
  - At locked `N = 64` (`k = 0..63`): **`810001, 811001, ..., 873001`** (`873001 < 880001`).
  - Smoke seed: **`999953`** (mechanically absent from all confirmation manifests).
  - **Programmatic disjointness verification (`Part 3` in `Appendix A`, `0` overlap):** Disjoint from `E2` (`700001..711001`, `999983`), `E3` (`730001..741001`, `999979`), `E2R` (`750001..`), `E-SEL1` (`760001..771001`, `999971`), `SEL2-P` (`780001..799506`, `999961`, `999801..999822`), `VIAB-1` / `VIAB-1b` (`880001..=880012`), `PR #216` probes (`880001..=880006`), `AE` (`1..3`), and smoke (`999953`).

### R2 — Multiplicity Control (`D2`)

- **Confirmatory Primary Family (`m = 3`):** Two-sided paired $t$-tests on $L_i$ controlled at FWER **$\alpha = 0.05$** via **Holm–Bonferroni** step-down (`0.016667`, `0.025`, `0.05`) across `H1` (`T/S`), `H2` (`T/U`), and `H3` (`S/U`). Per `D2` Option `(a)`, statistical $p$-values are two-sided, while `H1`'s directional prediction (`T > S`, $\bar{L} > 0$) is enforced in `R10` (mapping significant negative differences at $|\bar{L}| \ge \ln(1.5)$ to `opposite`).
- **Exploratory Family:** `ln(births_B / births_A)` mediator regressions, secondary dispersion/mean metrics (`S1..S3, S6`), predator covariate regressions, and separate survival `RMST`, controlled by **Benjamini–Hochberg FDR (`q = 0.05`)**.

### R3 — Recomputed `VIAB-1b` Extinction Bounds, Complete-Pair Primary & Lee-Bounds Sensitivity (`D3`)

From `artifacts/experiments/viab1b-founder-2seg/cells/e3b_fg1_g4_fl1.json` (`12` seeds `880001..=880012`, `0` extinctions and `0` singletons across all `36` arm-runs — minimum `min_n` across the entire cell is `58` in `S`, `91` in `U`, `131` in `T`):

| Arm / Contrast | Extinctions (`n=12`) | Point $\hat{p}_{\text{ext}}$ | Exact 1-Sided 80% Clopper–Pearson UB | Exact 1-Sided 95% UB | Realized $\hat{\pi}_{LL}$ (`surv & final_n >= 2`) | 80% 1-Sided LB on $\pi_{LL}$ |
|---|---:|---:|---:|---:|---:|---:|
| `control_uniform` (`U`) | `0/12` | `0.00%` | `12.55%` | `22.09%` | — | — |
| `control_uniform_scarcity` (`S`) | `0/12` | `0.00%` | `12.55%` | `22.09%` | — | — |
| `treatment_gradient` (`T`) | `0/12` | `0.00%` | `12.55%` | `22.09%` | — | — |
| Pooled `T + S` (`0/24`) / All 3 (`0/36`) | `0/24` · `0/36` | `0.00%` | `6.49%` · `4.37%` | `11.73%` · `7.98%` | — | — |
| Contrast `H1` (`T / S`) | — | — | — | — | **`12/12` (`100.00%`)** | **`87.45%`** (`>= 0.85`) |
| Contrasts `H2` (`T / U`), `H3` (`S / U`), All 3 | — | — | — | — | **`12/12` (`100.00%`)** | **`87.45%`** |

*(Identical `0/12` extinctions and `min_n >= 108` were also observed in `e3b_fg1_g4_fl1_mut0.json`.)*

Per Owner decision **`D3`**:
1. **`H1` (`T / S`) — Complete-pair analysis is primary:** Both `T` and `S` have `0/12` extinctions in both `VIAB-1` and `VIAB-1b` (`0/24` pooled, `pi_LL = 100%`, `80%` LB `87.45% >= 85%`), where worst-rank reduces identically to complete-pair analysis. Validity requires realized **$\hat{\pi}_{LL} \ge 0.85$** (`n_LL >= 55` out of `64`) and **$n_{LL} \ge 7$**.
2. **`H2` (`T / U`) and `H3` (`S / U`) — Complete-pair primary + Lee (2009) bounds sensitivity + separate `RMST`:**
   - Complete-pair analysis on $\bar{L}$ (`surv & agent_count >= 2` in both arms, subject to $\hat{\pi}_{LL} \ge 0.85$) is paired with **Lee (2009) monotone trimming bounds** across all surviving runs in each arm: trimming $|n_B - n_A|$ extreme observations from the higher-survival arm to `k = min(n_A, n_B)` yields `[Lee_lo, Lee_hi] = [mean(bottom k of B) - mean(top k of A), mean(top k of B) - mean(bottom k of A)]` on $Y = \ln(\text{trait\_mass\_var} + \varepsilon)$ (collapsing to width `0.0` at `0` differential attrition). If `n_A = n_B = 64`, Lee bounds equal $\bar{L}$ with width `0`; if differential extinction occurs in `U`, we report whether `[Lee_lo, Lee_hi]` still excludes `0` (or lies inside `(-ln(1.5), +ln(1.5))`).
   - **Survival (`RMST` at $\tau = 18,000$ ticks)** is reported as a **separate survival endpoint** across all `N = 64` seeds (`D3`: Lachin worst-rank is explicitly **not** primary for a two-sided body-mass dispersion endpoint).

### R4 — Pre-Specified SESOI & Achieved MDE Benchmark (`D4`)

- **Locked SESOI:** **$\pm \ln(1.5) = \pm 0.405465$** on paired $\bar{L}$ (corresponding to variance ratios `1.500x` or `0.667x`). Values inside `(-0.405465, +0.405465)` confirmed by TOST (`90%` CI strictly inside bounds) establish practical equivalence (`equivalent_to_zero`).
- **Pre-registered achieved MDE formula:** Because $\sigma_L = \text{SD}(L_i)$ cannot be inspected pre-freeze (`QD-VIAB`), E3b pre-registers the achieved MDE benchmark **`0.5 * sigma_hat_L`** alongside observed `80%` MDE (`0.4139 * sigma_hat_L` at $N = 64, \alpha/3 = 0.016667$). Note from `§3 R6` that **$0.5 \cdot \sigma_L \le \ln(1.5) \iff \sigma_L \le 0.8109$**, which gives **`93.50%`** superiority power over zero at $\Delta_L = \ln(1.5)$ (with strict R10 `supports` at `~50%` on the exact $\ln(1.5)$ boundary and `99.8%` at $\ln(2.0)$).
- **Secondary effect sizes only:** $d_z$, matched-pairs $r_c$ (**corrected calibration `d_z = 0.50 -> r_c = 0.5205`, `d_z = 0.80 -> r_c = 0.7421`**), Hodges–Lehmann with BCa, `lnCVR`, and raw mean differences remain strictly secondary / exploratory.

### R5 — Blind Non-Mass Nuisance Parameters (`VIAB-1b`, Trunk `98054ba2`)

All values below are recomputed strictly from `artifacts/experiments/viab1b-founder-2seg/cells/e3b_fg1_g4_fl1.json` (plus `e3b_fg1_g4_fl1_mut0.json` comparison; seeds `880001..=880012`, `0` `trait_mass` matches):

| Nuisance Parameter (`e3b_fg1_g4_fl1`) | `U` (`control_uniform`) | `S` (`control_uniform_scarcity`) | `T` (`treatment_gradient`) | Pairwise / `mut0` Cross-Check (`e3b_fg1_g4_fl1_mut0`) |
|---|---:|---:|---:|---|
| $\hat{p}_{\text{ext}}$ (tick `<= 18,000`) | `0/12` (`0.00%`) | `0/12` (`0.00%`) | `0/12` (`0.00%`) | `mut0`: `0/12` in all `3` arms (`RMST = 18000.0 +/- 0.0`) |
| `min_n` mean ± SD (`min..max`) | `149.1 ± 25.8` (`91..169`) | `126.8 ± 36.7` (`58..166`) | `162.7 ± 10.2` (`131..168`) | `mut0`: `min_n` `130..171 / 108..165 / 158..168` (`0` singletons) |
| `mean_n` mean ± SD (`min..max`) | `178.8 ± 4.7` (`165.5..184.0`) | `167.1 ± 12.4` (`141.7..178.1`) | `178.4 ± 2.2` (`172.4..180.8`) | `mut0`: `180.2 / 171.9 / 179.0` |
| `final_n` mean ± SD (`min..max`) | `150.2 ± 25.2` (`94..169`) | `135.8 ± 36.0` (`67..168`) | `165.7 ± 2.2` (`161..168`) | `mut0`: `156.5 / 149.7 / 166.1` |
| `predator_final > 0` (`mean ± SD`, `min..max`) | `12/12` (`8.00 ± 2.37`, `3..11`) | `11/12` (`2.50 ± 2.15`, `0..8`) | `12/12` (`5.67 ± 2.23`, `1..8`) | `mut0`: **`12/12` (`8.08`), `11/12` (`2.92`), `12/12` (`6.08`)** |
| `total_births` mean ± SD (`min..max`) | `552.8 ± 51.4` (`465..627`) | `355.8 ± 79.2` (`240..471`) | `542.6 ± 62.9` (`487..654`) | `mut0`: `571.2 / 375.7 / 535.6` |
| `generations` (`births / mean_n`) mean ± SD | **`3.091 ± 0.258`** (`2.78..3.52`) | **`2.111 ± 0.348`** (`1.65..2.65`) | **`3.041 ± 0.353`** (`2.76..3.68`) | `mut0`: **`3.170 / 2.178 / 2.993`** |
| `crop_post_warmup` mean ± SD (EU) | `122073.7 ± 2750.6` | `63775.1 ± 151.6` | `63902.5 ± 133.5` | `WP-2 b1: -0.4916..-0.4585`; `b2: -0.0037..+0.0070` (`12/12`) |
| `starvation_share` mean ± SD | `0.5916 ± 0.0707` | `0.1821 ± 0.0355` | `0.3362 ± 0.0418` | `mut0`: `0.5954 / 0.1725 / 0.3352` |
| CRN Pearson $\hat{\rho}$ (`T-S` / `T-U` / `S-U`) | — | — | — | `generations`: **`+0.923 / +0.905 / +0.776`**<br>`total_births`: `+0.848 / +0.842 / +0.606`<br>`predator_final`: `+0.511 / +0.447 / +0.142`<br>`mean_n`: `-0.478 / +0.009 / -0.138`<br>`crop_post_warmup`: `-0.410 / -0.142 / +0.606` |
| Mediator baseline `ln(births_B / births_A)` | — | — | — | `T/S`: **`+0.4396 ± 0.1557`** (`+0.2684..+0.7459`)<br>`T/U`: `-0.0206 ± 0.0632` · `S/U`: `-0.4602 ± 0.1978` |
| Unobserved $\sigma_L$ (trait variance ratio SD) | Unmeasured (`QD-VIAB`) | Unmeasured (`QD-VIAB`) | Unmeasured (`QD-VIAB`) | Swept explicitly across `[0.40, 1.10]` in `§3 R6` & `§3 R6b` |

### R6 — Sample Size Arithmetic at Locked `N = 64` (`D4`)

Because `trait_mass_var` is blind pre-freeze, `sigma_L = SD(L_i)` is unknown. At locked **`N = 64`** (`df = 63`, Holm step-1 $\alpha/3 = 0.016667$, TOST $\alpha_{\text{TOST}} = 0.05$, $\text{SESOI} = \pm\ln(1.5) = \pm 0.405465$):

- **Exact 80% MDE (non-central paired-$t$):** `0.4139 * sigma_L` (`d_z = 0.4139` gives `0.8000` power at $\alpha/3$).
- **Pre-registered achieved MDE benchmark:** **`0.5 * sigma_L`**, which corresponds to exact paired-$t$ Holm-1 **superiority power over zero** of **`0.9350`** (`> 0.90`).
- **Blind `sigma_L` operating envelope at `N = 64` (`L` scale, exact analytical curves):**
  - Superiority over zero at $\Delta_L = \ln(1.5)$ clears **`>= 0.90`** for all $\sigma_L \le 0.8623$ and **`>= 0.80`** for all **$\sigma_L \le 0.9796$**.
  - Achieved MDE benchmark satisfies **`0.5 * sigma_L <= ln(1.5)`** for all **$\sigma_L \le 0.8109$** ($\ln(1.5)/0.5 = 0.810930$).
  - TOST equivalence power at $\Delta_L = 0$ clears **`>= 0.95`** for $\sigma_L \le 0.892$, **`>= 0.88`** at $\sigma_L = 1.00$, and **`>= 0.80`** up to **$\sigma_L = 1.0966$**.
- **Pair survival reserve (`N = 64`):** Even at the conservative `0/36` pooled 80% UB extinction rate (`4.37%` independent), mean `n_LL = 58.50` keeps `P(pi_LL >= 0.85) = 0.9582` and superiority power over zero at `Delta = ln(1.5)` (`sigma_L = 0.8109`) at **`0.9070` (`t`) / `0.8914` (Wilcoxon)** (`> 0.89`). Under `VIAB-1b`'s observed `0/12`, `P(pi_LL >= 0.85) = 1.0000` (`n_LL = 64`, superiority power over zero **`0.9356`**).

> **Power scope note (`R6` / `R6b`):** The headline powers **`93.50%`** (`R6` analytical) and **`0.9356`** (`R6b` `V1` Monte Carlo) at $\Delta_L = \ln(1.5), \sigma_L = 0.8109$ are for **superiority over zero** ($p < \alpha/3$ with $\bar{L} > 0$), **not** for the `R10` `supports` verdict: when the true effect equals the SESOI ($\Delta_L = \ln(1.5)$), `R10` labels about **49–50%** of runs `supports` (and **97.6–99.8%** once $\Delta_L = \ln(2.0)$).

### R6b — Re-Run Monte Carlo Verification for `L_i` and `SESOI = ±ln(1.5)` (`python -I`, `Appendix A`)

Executed via `python -I` (`SeedSequence(2026112302)`, `N_rep = 5,000`, binomial `MCSE <= 0.0071`, `0` `'trait_mass'` substrings asserted via `--viab1b-cell e3b_fg1_g4_fl1.json`; runtime `7.12 s`). Note that `93.56%` (`t` power at `Delta = ln(1.5)`, `sigma_L = 0.8109`) measures **superiority over zero**, whereas `R10` labels **`~49–50%`** of runs `supports` at the exact `ln(1.5)` boundary (`99.80%` at `Delta = ln(2.0)`).

#### 1. Blind $\sigma_L$ Sweep at Locked `N = 64` (Gaussian vs. Heavy-Tailed Student-$t_5$, Unit Variance)

| $\sigma_L$ (`L_i` SD) | Dist | `0.5 * sigma_L` | $\Delta = 0$ `t` Type I | $\Delta = 0$ `W` Type I | **$\Delta = 0$ TOST Power** | **$\Delta = \ln(1.5)$ `t` Power** | $\Delta = \ln(1.5)$ `W` Power | R10 `supports` ($\Delta=\ln 1.5$) | $\Delta = \ln(2.0)$ `t` Power | **R10 `supports` ($\Delta=\ln 2.0$)** |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `0.4000` | `norm` / `t5` | `0.2000` | `0.0160` / `0.0120` | `0.0160` / `0.0128` | **`1.0000` / `0.9998`** | **`1.0000` / `0.9994`** | `1.0000` / `1.0000` | `0.4992` / `0.4984` | `1.0000` / `1.0000` | **`1.0000` / `1.0000`** |
| `0.6000` | `norm` / `t5` | `0.3000` | `0.0136` / `0.0166` | `0.0128` / `0.0162` | **`1.0000` / `0.9988`** | **`0.9988` / `0.9928`** | `0.9976` / `0.9994` | `0.5066` / `0.4976` | `1.0000` / `0.9998` | **`1.0000` / `0.9996`** |
| `0.7000` | `norm` / `t5` | `0.3500` | `0.0154` / `0.0188` | `0.0136` / `0.0186` | **`0.9966` / `0.9892`** | **`0.9838` / `0.9702`** | `0.9802` / `0.9902` | `0.4954` / `0.4934` | `1.0000` / `0.9998` | **`0.9996` / `0.9990`** |
| **`0.8109`** *(`0.5*sd=ln1.5`)* | `norm` / `t5` | **`0.4055`** | `0.0182` / `0.0168` | `0.0152` / `0.0190` | **`0.9788` / `0.9678`** | **`0.9356` / `0.9252`** | `0.9270` / `0.9658` | `0.4904` / `0.5012` | `1.0000` / `0.9990` | **`0.9980` / `0.9970`** |
| `0.9000` | `norm` / `t5` | `0.4500` | `0.0172` / `0.0112` | `0.0174` / `0.0144` | **`0.9486` / `0.9436`** | **`0.8646` / `0.8604`** | `0.8444` / `0.9230` | `0.4934` / `0.4884` | `1.0000` / `0.9980` | **`0.9942` / `0.9946`** |
| `1.0000` | `norm` / `t5` | `0.5000` | `0.0174` / `0.0182` | `0.0174` / `0.0166` | **`0.8852` / `0.8720`** | `0.7844` / `0.7846` | `0.7586` / `0.8506` | `0.4940` / `0.4958` | `0.9986` / `0.9940` | **`0.9882` / `0.9840`** |
| `1.1000` | `norm` / `t5` | `0.5500` | `0.0144` / `0.0172` | `0.0140` / `0.0188` | **`0.8024` / `0.7964`** | `0.6856` / `0.6926` | `0.6584` / `0.7748` | `0.4926` / `0.4898` | `0.9948` / `0.9850` | **`0.9822` / `0.9756`** |

*(Exact R10 strict-decision property: at the exact SESOI boundary $\Delta_L = \ln(1.5)$, $|\bar{L}| \ge \ln(1.5)$ holds 50% of runs regardless of $N$, so R10 `supports` is `~0.49–0.50`; once $\Delta_L$ reaches $\ln(2.0)$ (`2x` variance ratio), R10 strict `supports` exceeds **`0.975–1.000`** across the entire $\sigma_L \le 1.10$ sweep, and paired Wilcoxon outperforms $t$ under heavy-tailed $t_5$ noise.)*

#### 2. Attrition, Lee (2009) Trimming Bounds & Separate RMST (`N in {32, 48, 60, 64}`, $\sigma_L = 0.8109$)

Hypothetical `V2..V4` attrition uses `VIAB-1b` pooled/arm Clopper–Pearson bounds (`0/36 -> 4.37%`, `0/12 -> 12.55%`) plus `EXT_TICKS_VIAB1` solely for counterfactual `RMST` (`VIAB-1b` itself had `0/36` extinctions). Marginal arm SD is calibrated so $\text{SD}(Y_B - Y_A) = \sigma_L = 0.8109$ ($\rho_{\text{gen}}$ & $\rho_{\text{births}}$ from `VIAB-1b` anchor $\rho_Y = 0.85$ for `T-S` and conservative `0.60` for `U` contrasts):

| Scenario | `N` | Mean `n_LL` | $P(\hat{\pi}_{LL}\ge 0.85)$ | $\Delta=0$ `t` T1 / TOST | Separate `RMST` ($\alpha/3$) | $\Delta=\ln(1.5)$ `t` / `W` Power | $\Delta=\ln(1.5)$ Mean Lee Width | $P(\text{Lee}_{\text{lo}} > 0)$ at `ln(1.5)` / `ln(2.0)` |
|---|---:|---:|---:|---|---:|---|---|---|
| `V1` Obs (`0%/0%, rho=0.85`) | `32` | `32.00` | `1.0000` | `0.0168` / `0.7488` | `0.0000` | `0.6326` / `0.5960` | `0.0000` | `0.9978` / `1.0000` |
| `V1` Obs (`0%/0%, rho=0.85`) | **`64`** | **`64.00`** | **`1.0000`** | `0.0152` / **`0.9790`** | `0.0000` | **`0.9362` / `0.9234`** | **`0.0000`** | **`1.0000` / `1.0000`** |
| `V2c` Sym 4.37% (`rE=0.85`) | `64` | `59.94` | `0.9926` | `0.0164` / `0.9728` | `0.0018` | **`0.9118` / `0.8964`** | `0.1276` | `0.9962` / `1.0000` |
| `V2i` Sym 4.37% (`rE=0.00`) | `64` | `58.50` | `0.9582` | `0.0154` / `0.9702` | `0.0022` | **`0.9070` / `0.8914`** | `0.1833` | `0.9878` / `1.0000` |
| `V3` Asym (`U=5%, T=0%, rho=0.60`) | `32` | `30.39` | `0.9800` | `0.0190` / `0.7106` | `0.0010` | `0.5932` / `0.5570` | `0.1775` | `0.9724` / `1.0000` |
| `V3` Asym (`U=5%, T=0%, rho=0.60`) | **`64`** | **`60.75`** | **`0.9982`** | `0.0154` / **`0.9738`** | **`0.0420`** | **`0.9176` / `0.9018`** | **`0.1887`** | **`0.9972` / `1.0000`** |
| `V4` Asym 80% UB (`U=12.55%, rho=0.60`) | **`64`** | `56.00` | `0.7218` | `0.0160` / `0.9600` | **`0.7022`** | `0.8944` / `0.8746` | **`0.4155`** | `0.9326` / `0.9994` |
| `V5a` Heavy-tail `t5` (`0%/0%, rho=0.85`) | `64` | `64.00` | `1.0000` | `0.0158` / `0.9698` | `0.0000` | **`0.9288` / `0.9640`** | `0.0000` | `0.9998` / `1.0000` |
| `V5b` Heavy-tail `t5` (`U=5%, rho=0.60`) | `64` | `60.78` | `0.9992` | `0.0154` / `0.9718` | `0.0378` | **`0.9156` / `0.9498`** | `0.1968` | `0.9950` / `1.0000` |

#### 3. Bug-Fix #2 Verification — Corrected Matched-Pairs Kerby $r_c$ Bounds (`Part 3`)

Under Gaussian paired differences $D_i \sim \mathcal{N}(\mu, \sigma^2)$ with $d_z = \mu/\sigma$, the asymptotic Kerby rank-biserial correlation for Wilcoxon signed-rank satisfies **$r_c = 2\Phi(\sqrt{2}\, d_z) - 1$** (replacing the erroneous `0.35` / `0.50` figures in `E3B-P`):

- $d_z = 0.50 \implies 2\Phi(0.707107) - 1 =$ **`0.5205`** (`0.52`; finite-sample `N=64` MC mean `0.5155 +/- 0.0017`).
- $d_z = 0.80 \implies 2\Phi(1.131371) - 1 =$ **`0.7421`** (`0.74`; finite-sample `N=64` MC mean `0.7376 +/- 0.0012`).

### R7 — Minimum Attainable Exact $p$-Value vs. Holm Step-1 $\alpha/3$

For exact Wilcoxon / sign-flip tests on $n$ non-zero pairs, $p_{\min} = 2 / 2^n$ against $\alpha/3 = 0.016667$: minimum feasible count is **$n \ge 7$** ($0.015625 < 0.016667$). At E3b's **`N = 64`**, $p_{\min}(64) = 1.08 \times 10^{-19}$; at the $\hat{\pi}_{LL} = 0.85$ validity floor (`n_LL = 55`), $p_{\min}(55) = 5.55 \times 10^{-17}$ (exact paired-$t$ on continuous `L_i` is unclipped at `df = n_LL - 1`).

### R8 — Full Paired Effect-Size & Diagnostics Bundle (`YES`, Updated for `L_i`)

1. **Primary contrast summary:** Paired mean log variance ratio $\bar{L}$ (`eps = 5e-4`, labelled explicitly as log variance ratio and geometric variance ratio $\exp(\bar{L})$) with **95% $t$-CI**, **90% TOST $t$-CI**, two-sided paired-$t$ $p$, Holm adjusted $p_{\text{Holm}}$, exact/approx Wilcoxon $p$, and mandatory **`eps = 5e-5` sensitivity table**. For `H2` & `H3` (`U` contrasts), report **Lee (2009) trimming interval `[Lee_lo, Lee_hi]`** and separate **`RMST(tau=18000)`**.
2. **Secondary / exploratory paired effect sizes (never primary decision rules per `D4`):** Paired $d_z$, Kerby $r_c$ (with corrected benchmarks `0.52` / `0.74`), paired Hodges–Lehmann pseudomedian on $L_i$ with **95% BCa bootstrap CI** (`10,000` resamples, deterministic seed), concordance $\hat{p}_{L_i > 0}$, `lnCVR`, raw mean & variance differences, `agent_count` / `predator_count` at `T = 18,000`, extinction/singleton/empty-half tables, `ln(births_B / births_A)` mediator regression, and 3-arm identity `L(T/U) == L(T/S) + L(S/U)`.

### R9 — TOST Equivalence Bounds (`D4`)

Two one-sided paired $t$-tests on $\bar{L}$ (`eps = 5e-4`) at **$\alpha_{\text{TOST}} = 0.05$ each tail** — equivalent to verifying whether the **90% $t$-CI** lies strictly inside **$(-\ln(1.5), +\ln(1.5)) = (-0.405465, +0.405465)$** (`[0.667x, 1.500x]` variance ratio). Checked jointly against $\varepsilon_{\text{sens}} = 5\times 10^{-5}$ in sensitivity diagnostics.

### R10 — Closed 4-Label Decision Table (`D2` `(a)` Default, Updated for Complete-Pair & `L_i`)

Per `D2` Option `(a)` (`H1` directional `T > S`; `H2, H3` two-sided), every primary contrast maps uniquely into **`{supports, opposite, equivalent_to_zero, insufficient_evidence}`** (`opposite` is a distinct refutation outcome; **`48/48` exhaustive 5-column check passes** in `Appendix A`):

| `WP-2` & Host Integrity Valid? | Complete-Pair Validity (`0` failed runs, $\hat{\pi}_{LL} \ge 0.85$ & $n_{LL} \ge 7$)? | Holm Adjusted $p_{\text{Holm}} < 0.05$ (`t`-test on `L_i`)? | Point $\bar{L}$ Magnitude & Sign ($\text{SESOI} = \ln(1.5) = 0.405465$)? | R9 TOST 90% CI Strictly Inside $(-\ln(1.5), +\ln(1.5))$? | Closed Label (`R10`) | Mandatory Direction Tag (`direction_tag`) |
|---|---|---|---|---|---|---|
| **No** | Any | Any | Any | Any | **`insufficient_evidence`** *(stop campaign on harness bug)* | `harness_invalid` |
| Yes | **No** ($n_{LL} < 7$ or $\hat{\pi}_{LL} < 0.85$) | Any | Any | Any | **`insufficient_evidence`** | `attrition_below_pi85_floor` |
| Yes | Yes | **Yes** | Positive (`T > S` on `H1`; or $\lvert \bar{L} \rvert \ge \text{SESOI}$ on `H2,H3`) AND **$\lvert \bar{L} \rvert \ge \ln(1.5)$** | No | **`supports`** | `predicted_positive_L` (`H1`) / `non_zero_two_sided_L` (`H2,H3`) |
| Yes | Yes | **Yes** | Negative on `H1` ($\bar{L} \le -\ln(1.5)$, `T < S`) | No | **`opposite`** *(refutes `H1` in opposite direction)* | **`opposite_negative_L`** — triggers Stage 4 finding note |
| Yes | Yes | **Yes** | $\lvert \bar{L} \rvert < \ln(1.5)$ *(statistically non-zero, trivially small)* | **Yes** | **`equivalent_to_zero`** | `trivial_below_sesoi_equivalent` |
| Yes | Yes | **Yes** | $\lvert \bar{L} \rvert < \ln(1.5)$ | **No** *(90% CI straddles $\pm\ln(1.5)$)* | **`insufficient_evidence`** | `inconclusive_straddles_sesoi` |
| Yes | Yes | **No** | $\lvert \bar{L} \rvert < \ln(1.5)$ | **Yes** | **`equivalent_to_zero`** | `equivalent_to_zero_powered_null` |
| Yes | Yes | **No** | Any (`>=` or `< ln(1.5)`) | **No** | **`insufficient_evidence`** | `inconclusive_wide_ci` |
| Yes | Yes | Any | $\lvert \bar{L} \rvert \ge \ln(1.5)$ | **Yes** *(degenerate impossible CI guard)* | **`insufficient_evidence`** | `ci_inconsistent_impossible` |

- **Partition check (`Part 3`, `48/48` unique):** `supports = 1`, `opposite = 1`, `equivalent_to_zero = 2`, `insufficient_evidence = 44` (`len(matches) == 1` on all `48` combinations).
- **Sensitivity flagging rule (non-overriding):** Flag in the narrative (without altering the R10 label) if Wilcoxon signed-rank diverges from paired-$t$ on Holm significance, if `eps = 5e-5` flips the sign or significance of $\bar{L}$, or if on `H2/H3` the Lee trimming interval `[Lee_lo, Lee_hi]` spans `0` while complete-pair $\bar{L}$ rejects `0`.
- **Mandatory `H1` phrasing constraint:** Every `H1` claim must read strictly: *"a productivity gradient created by West `Deforest 0.30` and East `AddNutrient 1.00` altered the body-mass variance ratio (`L_i`) after ~3.0 (`T`) vs ~2.1 (`S`) generations of de novo input from the adapted 2-segment clonal founder (`live.founder_genotype = 1`), with predators persisting at `T = 18,000`, relative to uniform scarcity (`resource_capacity_scale = 0.42` + whole-map `AddNutrient 1.00`) at matched ~64k EU standing crop."*

---

## 4. Smoke Gates, `WP-2` Gates, Host Rules & Windows 1 Run Budget

### 4.1. Pre-Execution Host & World Isolation Guards (Carried Over)

1. **Single execution host (`QD-E3HOST`):** All confirmation runs (`192` runs) and the smoke run (`seed = 999953`) execute on **`Windows 1` (`windows/x86_64`)** from one frozen `--release --features desktop` binary logged in `provenance.json`.
2. **Environment isolation (`F-051`):** Verify and record `ANIMA_WORLD_ARTIFACT` is unset (`<unset>`) and `<temp_dir>/anima_world_artifact.anmw` does not exist before execution; confirm `terrain_source == "generated"` and identical `world_identity` (`3152323152` on Windows `x86_64`) across every run.
3. **Closed EU conservation:** `max_abs_eu_drift <= 1.0e-6 EU` across all arms and ticks (observed in `VIAB-1b` `e3b_fg1_g4_fl1`: max `7.718e-08`).

### 4.2. Blind Smoke Check (`seed = 999953`) & `VIAB-1b` `WP-2` Benchmarks

Before executing `810001..=873001`, run 1 blind `--release` smoke triple (`3` arms x `seed = 999953`, `body_mass_consulted: false` checking only `is_finite()`):

- **`WP-2(a)` ResourceField & Genesis EU:** `256x256` grid; identical `153,587.25 EU` across `U`, `S`, `T`.
- **`WP-2(b1)` Treatment standing-crop shift:** `|(mean_post_warmup(T) - mean_post_warmup(U)) / mean_post_warmup(U)| >= 0.10` (observed `VIAB-1b`: `12/12` pass, range `-0.4916..-0.4585`).
- **`WP-2(b2)` Scarcity standing-crop match:** `|(mean_post_warmup(T) - mean_post_warmup(S)) / mean_post_warmup(T)| <= 0.10` with `0` `Deforest` events in `S` (observed `VIAB-1b`: `12/12` pass, range `-0.0037..+0.0070`).
- **`WP-2(c)` Early detritus flow:** `detritus > 0` before tick `3,000` (first sample `600`) and at tick `3,000` (`12/12` in `VIAB-1b`).
- **Horizon (`WP-3`):** `duration_ticks = 18,000`, `sample_period = 600` (`30` samples), `warmup_ticks = 3,000`, `tail_window = [14400, 18000]`. Do **not** alter horizon or `N = 64`.

### 4.3. Windows 1 Run Budget (`N = 64` -> `192` Runs)

Anchored directly to Windows 1 release timings:
- **12-thread `VIAB-1b` benchmark (`e3b_fg1_g4_fl1`, trunk `98054ba2`):** `82.394 s` for `36` runs = **`2.289 s/run`** -> **`439.4 s` (`7.32 min`)** for `192` runs (`64` seed triples x `3` arms x `18,000` ticks = `3,456,000` ticks).
- **Sequential runner bound (`E3-X` single-thread baseline `4.265 s/run`, scaled by `2.289/1.945` for larger equilibrium `mean_N`):** **`~16.0 min`** for `192` runs (still well inside the `90 min` ceiling; plus one-time `--release` build `~10.4 min`).

---

## 5. Locked Decisions Summary (`QD-E3B`), Two Open Sign-Offs & Not Yet Done

### 5.1. Summary of Owner-Approved `D1`–`D5` & Two `VIAB-1b` Sign-Off Items Before Freeze

| ID | Decision (`PR #223` / `MSG_20261123_140000`) | Pre-Freeze Verification / Action Needed from Owner |
|---|---|---|
| **`E3B-D1`** | **`live.predator_fraction = 0.3`** kept | **SIGN-OFF NEEDED (`VIAB-1b` finding):** Under 2-seg `founder_genotype=1`, predators **persist** to `T=18,000` in **`12/11/12`** seeds (`U/S/T`, `1..11` predators), NOT extinct `12/12` as in 3-seg `VIAB-1`. Approve running E3b as **two-trophic (predators present)**, or switch to `0.0` (needs 1 blind calibration cell). |
| **`E3B-D2`** | Option **`2A` + `(a)`** (directional `H1: T > S`, 4-label R10) | Locked in §2.2 and §3 R2/R10 (`48/48` verified). |
| **`E3B-D3`** | **Complete-pair primary** on `H1 (T-S)` (`pi_LL >= 0.85`); **`U` contrasts** complete-pair + **Lee bounds sensitivity** + separate **RMST**; no worst-rank | Locked in §3 R3 & R6b (`pi_LL = 100%` observed in `VIAB-1b`). |
| **`E3B-D4`** | **$L_i = \ln[(\text{var}_B+\varepsilon)/(\text{var}_A+\varepsilon)]$**, **$\varepsilon=5\times 10^{-4}$** (sens. $5\times 10^{-5}$), **SESOI $\pm\ln(1.5)$**, **TOST on $L$**, **$N=64$**, MDE $0.5\cdot\hat{\sigma}_L$, secondary $d_z$/$r_c$ (`0.52`/`0.74`)/HL/lnCVR/mean, **`ln(births_T/births_S)` mediator** | Locked in §2.2, §2.3, §3 R4–R9. Also confirm keeping **`T = 18,000`** & **`N = 64`** unchanged under `VIAB-1b`'s shorter realized horizon **`3.091 / 2.111 / 3.041` generations** (`~3.1 / 2.1 / 3.0`) with mandatory *"after ~k generations"* wording. |
| **`E3B-D5`** | Option **`(b)`** declared **2-segment founder** (`live.founder_genotype = 1.0`, `2.5` mass) | Implemented & merged in `PR #224` (`98054ba2`), blind-verified `12/12` (`mut` & `mut0`) in `VIAB-1b` (`PR #228`); `PR #216` provenance & `mutation.rs:211` operator-4 wall disclosed in §1.3. |

### 5.2. Explicitly Not Yet Done (Held Pending Owner Approval of `E3B-P-v2`)

1. No frozen `e3b-preregistration.json` or 6 arm/smoke manifests have been created (`status` stays `proposed`).
2. No Rust experiment runner, manifest test, stats calculator (`L_i`, TOST on `L_i`, Lee bounds, RMST, `r_c` = `0.5205`/`0.7421`, BCa HL, `lnCVR`, mediator regression, `4`-label R10), or artifact verifier has been modified.
3. Zero smoke (`999953`) or confirmation seeds (`810001..=873001`) have been run.
4. `NR-SPIKE` protocol remains ready (`PR #221` merged at `ca9e1ff`); spike implementation stays parked until E3b freeze/execution clears.

---

## Appendix A — Reproducible `L_i` Monte Carlo & Bug-Fix Verification Script

Run standalone with `python -I` (NumPy + SciPy), or pass `--viab1b-cell <path-to-e3b_fg1_g4_fl1.json>` to assert `0` `'trait_mass'` substrings and verify exact numerical agreement with `VIAB-1b` (`PR #228`). Runtime: `~6.5 s`.

```python
"""Pre-freeze, body-mass-blind Monte Carlo & verification script for E3B-P-v2.

Runs standalone with `python -I` (NumPy + SciPy). Optionally pass
`--viab1b-cell <path-to-e3b_fg1_g4_fl1.json>` to assert 0 case-insensitive
`'trait_mass'` substrings and verify exact agreement with all non-mass
VIAB-1b calibration constants from PR #228.

Verifies and computes:
1. Exact VIAB-1b non-mass nuisance metrics and Clopper-Pearson bounds.
2. Part 1: Analytical + Monte Carlo (N_rep = 5,000) superiority & TOST power on
   the primary per-seed log variance ratio L_i = ln[(var_A + eps)/(var_B + eps)]
   at N = 64 across blind sigma_L values under both Gaussian and heavy-tailed t_5
   (unit-variance) paired errors, plus achieved MDE benchmarks (exact 80% vs 0.5*sigma_L).
3. Part 2: Complete-pair, Lee (2009) trimming-bounds sensitivity, and separate
   RMST behavior across N in {32, 48, 60, 64} under observed (0%/0%) and
   stress attrition scenarios (pooled 4.37%/4.37%, U=5%/T=0%, U=12.55%/T=0%),
   plus heavy-tailed t_5 checks.
4. Part 3: Bug-fix verifications:
   - Exact Kerby matched-pairs r_c bounds (d_z=0.50 -> 0.5205; d_z=0.80 -> 0.7421);
   - R1 seed range disjointness and N <= 70 cap against k = 70 (880001) collision;
   - R10 exhaustive 48/48 discrete tuple partition under default (a) 4-label rules.
"""

import argparse
import itertools
import json
import math
import pathlib
import time

import numpy as np
from scipy import optimize, stats

BASE_SEED = 2026112302
N_REP = 5000
ALPHA_FWER = 0.05
ALPHA_HOLM1 = ALPHA_FWER / 3.0  # 0.0166667 (Holm step-1 across m=3 hypotheses)
ALPHA_TOST = 0.05               # Each tail -> 90% t-CI inside (-SESOI, +SESOI)
SESOI_L = math.log(1.5)         # 0.4054651081081644
LN_2 = math.log(2.0)            # 0.6931471805599453
TAU = 18000.0
MIN_NZ_R7 = 7                   # 2 / 2^6 = 0.03125 > 0.01667; 2 / 2^7 = 0.015625 < 0.01667

N_GRID = (32, 48, 60, 64)
SIGMA_L_REF = SESOI_L / 0.5     # 0.8109302162163288 (makes Delta_L = ln(1.5) <=> d_z = 0.50)
SIGMA_L_GRID = (0.40, 0.50, 0.60, 0.70, SIGMA_L_REF, 0.90, 1.00, 1.10)

# Empirical 20-extinction-tick pool from VIAB-1 (used ONLY to parameterize
# counterfactual RMST under hypothetical V2-V4 stress; VIAB-1b had 0/36 extinctions).
EXT_TICKS_VIAB1 = np.array(
    [
        1800, 3000, 3600, 4200, 4800, 9000, 9600, 9600, 10200, 10800,
        10800, 10800, 10800, 13200, 14400, 14400, 15000, 15600, 16800, 17400,
    ],
    dtype=np.float64,
)

# Exact VIAB-1b primary cell (e3b_fg1_g4_fl1.json, trunk 98054ba, n=12 seeds) constants:
VIAB1B_EXPECTED = {
    "ext_counts": {"u": 0, "s": 0, "t": 0},
    "mean_n": {"u": 178.7778, "s": 167.0972, "t": 178.4333},
    "min_n_range": {"u": (91, 169), "s": (58, 166), "t": (131, 168)},
    "final_n_mean": {"u": 150.1667, "s": 135.8333, "t": 165.6667},
    "pred_pos_count": {"u": 12, "s": 11, "t": 12},
    "generations_mean": {"u": 3.0906, "s": 2.1111, "t": 3.0410},
    "ln_births_ts_mean": 0.4396,
    "ln_births_ts_sd": 0.1557,
    "rho_gen": {"ts": 0.9234, "tu": 0.9050, "su": 0.7757},
    "rho_births": {"ts": 0.8482, "tu": 0.8423, "su": 0.6061},
    "rho_pred": {"ts": 0.5112, "tu": 0.4472, "su": 0.1419},
    "rho_mean_n": {"ts": -0.4783, "tu": 0.0089, "su": -0.1376},
    "rho_crop": {"ts": -0.4103, "tu": -0.1419, "su": 0.6059},
}


def pearson(x: list[float], y: list[float]) -> float:
    mx, my = sum(x) / len(x), sum(y) / len(y)
    vx = sum((a - mx) ** 2 for a in x)
    vy = sum((b - my) ** 2 for b in y)
    return sum((a - mx) * (b - my) for a, b in zip(x, y)) / math.sqrt(vx * vy)


def verify_viab1b_cell(cell_path: pathlib.Path | None) -> None:
    print("=== VIAB-1b Non-Mass Nuisance Constants & Clopper-Pearson Bounds ===")
    for label, n in [("Per-arm 0/12", 12), ("T-S pair 0/24", 24), ("3-arm pooled 0/36", 36)]:
        ub80 = 1.0 - 0.20 ** (1.0 / n)
        ub95 = 1.0 - 0.05 ** (1.0 / n)
        print(f"  {label:18s}: 80% 1-sided UB = {ub80*100:.2f}% (surv LB = {(1-ub80)*100:.2f}%), 95% UB = {ub95*100:.2f}%")

    if cell_path is None:
        print("  (Standalone mode using locked VIAB-1b constants; pass --viab1b-cell to cross-check raw JSON)")
        return

    raw = cell_path.read_text(encoding="utf-8")
    assert "trait_mass" not in raw.lower(), f"BLINDNESS VIOLATION in {cell_path}"
    data = json.loads(raw)
    ps = data["per_seed"]
    assert len(ps) == 12
    assert data["all_terrain_generated"] is True

    for arm in ("u", "s", "t"):
        ext = sum(1 for r in ps if not r[arm]["survived"])
        mean_n = sum(r[arm]["mean_n"] for r in ps) / 12.0
        min_rng = (int(min(r[arm]["min_n"] for r in ps)), int(max(r[arm]["min_n"] for r in ps)))
        final_n = sum(r[arm]["final_n"] for r in ps) / 12.0
        pred_pos = sum(1 for r in ps if r[arm]["predator_final"] > 0)
        gens = sum(r[arm]["generations"] for r in ps) / 12.0
        assert ext == VIAB1B_EXPECTED["ext_counts"][arm]
        assert abs(mean_n - VIAB1B_EXPECTED["mean_n"][arm]) < 1e-3
        assert min_rng == VIAB1B_EXPECTED["min_n_range"][arm]
        assert abs(final_n - VIAB1B_EXPECTED["final_n_mean"][arm]) < 1e-3
        assert pred_pos == VIAB1B_EXPECTED["pred_pos_count"][arm]
        assert abs(gens - VIAB1B_EXPECTED["generations_mean"][arm]) < 1e-3

    lr = [math.log(r["t"]["total_births"] / r["s"]["total_births"]) for r in ps]
    lr_mean = sum(lr) / 12.0
    lr_sd = math.sqrt(sum((v - lr_mean) ** 2 for v in lr) / 11.0)
    assert abs(lr_mean - VIAB1B_EXPECTED["ln_births_ts_mean"]) < 1e-3
    assert abs(lr_sd - VIAB1B_EXPECTED["ln_births_ts_sd"]) < 1e-3

    for pair, a, b in [("ts", "t", "s"), ("tu", "t", "u"), ("su", "s", "u")]:
        for key, field in [
            ("rho_gen", "generations"),
            ("rho_births", "total_births"),
            ("rho_pred", "predator_final"),
            ("rho_mean_n", "mean_n"),
            ("rho_crop", "crop_post_warmup"),
        ]:
            val = pearson([r[a][field] for r in ps], [r[b][field] for r in ps])
            assert abs(val - VIAB1B_EXPECTED[key][pair]) < 1e-3, (key, pair, val)

    print(f"  [VERIFIED] {cell_path.name}: 0 'trait_mass' matches, 12/12 seeds & all correlations match exactly.")


def exact_t_sup_power(n: int, delta_l: float, sigma_l: float, alpha: float = ALPHA_HOLM1) -> float:
    """Two-sided (with correct positive sign) paired-t Holm-1 power."""
    df = n - 1
    nc = (delta_l / sigma_l) * math.sqrt(n)
    t_crit = stats.t.ppf(1.0 - alpha / 2.0, df)
    return float(1.0 - stats.nct.cdf(t_crit, df, nc))


def exact_tost_power(n: int, delta_l: float, sigma_l: float, sesoi: float = SESOI_L, alpha_tost: float = ALPHA_TOST) -> float:
    """Exact probability that the 90% paired t-CI falls strictly inside (-sesoi, +sesoi)."""
    df = n - 1
    t_crit = stats.t.ppf(1.0 - alpha_tost, df)
    qs = (np.arange(1, 2000) - 0.5) / 2000.0
    s_ratio = np.sqrt(stats.chi2.ppf(qs, df) / df)
    bound = sesoi - t_crit * sigma_l * s_ratio / math.sqrt(n)
    se_mean = sigma_l / math.sqrt(n)
    prob = np.where(
        bound > 0.0,
        stats.norm.cdf((bound - delta_l) / se_mean) - stats.norm.cdf((-bound - delta_l) / se_mean),
        0.0,
    )
    return float(np.mean(prob))


def exact_t_mde(n: int, target_power: float = 0.80, alpha: float = ALPHA_HOLM1) -> float:
    """Returns MDE / sigma_L (i.e. d_z reaching target_power at alpha)."""
    df = n - 1
    t_crit = stats.t.ppf(1.0 - alpha / 2.0, df)
    dz = optimize.brentq(
        lambda d: (1.0 - stats.nct.cdf(t_crit, df, d * math.sqrt(n))) - target_power,
        0.05,
        3.0,
    )
    return float(dz)


def rmst_p_matrix(t1: np.ndarray, t2: np.ndarray) -> np.ndarray:
    """Paired two-sided RMST Wilcoxon p-values across (N_rep, N) matrices."""
    diffs = t1 - t2
    pvals = np.ones(diffs.shape[0], dtype=np.float64)
    nz_counts = np.sum(diffs != 0.0, axis=1)
    for idx in np.flatnonzero(nz_counts >= MIN_NZ_R7):
        nz = diffs[idx][diffs[idx] != 0.0]
        if np.all(nz > 0.0) or np.all(nz < 0.0):
            pvals[idx] = 2.0 / (2.0 ** nz.size)
        else:
            pvals[idx] = float(
                stats.wilcoxon(nz, zero_method="wilcox", alternative="two-sided", method="approx")[1]
            )
    return pvals


def part1_sigma_l_sweep(seed_seq: np.random.SeedSequence) -> None:
    print("\n=== Part 1: Primary L_i Power & TOST Sweep across Blind sigma_L (N = 64, N_rep = 5,000) ===")
    n = 64
    t_holm = float(stats.t.ppf(1.0 - ALPHA_HOLM1 / 2.0, n - 1))
    t_tost = float(stats.t.ppf(1.0 - ALPHA_TOST, n - 1))
    dz_mde80 = exact_t_mde(n, 0.80, ALPHA_HOLM1)
    pwr_dz05_t = exact_t_sup_power(n, 0.50, 1.0, ALPHA_HOLM1)

    max_sigma_sup80 = optimize.brentq(lambda s: exact_t_sup_power(n, SESOI_L, s) - 0.80, 0.1, 3.0)
    max_sigma_sup90 = optimize.brentq(lambda s: exact_t_sup_power(n, SESOI_L, s) - 0.90, 0.1, 3.0)
    max_sigma_tost80 = optimize.brentq(lambda s: exact_tost_power(n, 0.0, s) - 0.80, 0.1, 3.0)
    print(f"  Analytical N=64 crossovers (alpha/3 = {ALPHA_HOLM1:.6f}, SESOI = ln(1.5) = {SESOI_L:.6f}):")
    print(f"    Exact 80% MDE         : {dz_mde80:.4f} * sigma_L")
    print(f"    Pre-registered 0.5*sd : exact paired-t power = {pwr_dz05_t:.4f} (> 0.90)")
    print(f"    max sigma_L for 80% superiority at Delta=ln(1.5) : {max_sigma_sup80:.4f}")
    print(f"    max sigma_L for 90% superiority at Delta=ln(1.5) : {max_sigma_sup90:.4f}")
    print(f"    max sigma_L for 0.5*sigma_L <= ln(1.5)           : {SIGMA_L_REF:.4f}")
    print(f"    max sigma_L for 80% TOST equiv at Delta=0.0      : {max_sigma_tost80:.4f}")

    scale_t5 = math.sqrt(3.0 / 5.0)
    for dist_name, dist_kind in [("Gaussian", "norm"), ("Student-t5 (unit var)", "t5")]:
        print(f"\n  [{dist_name}] N=64, N_rep={N_REP}")
        for sigma_l in SIGMA_L_GRID:
            rng = np.random.default_rng(seed_seq.spawn(1)[0])
            if dist_kind == "norm":
                z = rng.standard_normal((N_REP, n))
            else:
                z = rng.standard_t(df=5, size=(N_REP, n)) * scale_t5
            x0 = z * sigma_l

            def eval_shift(delta: float):
                x = x0 + delta
                means = x.mean(axis=1)
                ses = x.std(axis=1, ddof=1) / math.sqrt(n)
                t_stats = means / ses
                t_pos = float(np.mean(t_stats > t_holm))
                t_two = float(np.mean(np.abs(t_stats) > t_holm))
                r10_sup = float(np.mean((t_stats > t_holm) & (means >= SESOI_L)))
                tost_eq = float(np.mean(np.abs(means) + t_tost * ses < SESOI_L))
                w_pvals = stats.wilcoxon(x, axis=1, zero_method="wilcox", alternative="two-sided", method="approx").pvalue
                w_two = float(np.mean(w_pvals < ALPHA_HOLM1))
                w_pos = float(np.mean((means > 0.0) & (w_pvals < ALPHA_HOLM1)))
                return t_pos, t_two, w_pos, w_two, r10_sup, tost_eq

            _, t_t1, _, w_t1, _, eq0 = eval_shift(0.0)
            t15, _, w15, _, r10_15, _ = eval_shift(SESOI_L)
            t20, _, _, _, r10_20, _ = eval_shift(LN_2)
            ex_tost0 = exact_tost_power(n, 0.0, sigma_l) if dist_kind == "norm" else float("nan")
            ex_sup15 = exact_t_sup_power(n, SESOI_L, sigma_l) if dist_kind == "norm" else float("nan")
            tag = " [0.5*sd=ln1.5]" if abs(sigma_l - SIGMA_L_REF) < 1e-6 else ""
            ex_str = f" (exact t={ex_sup15:.4f}, TOST={ex_tost0:.4f})" if dist_kind == "norm" else ""
            print(
                f"    sigma_L={sigma_l:.4f}{tag}: MDE0.5={0.5*sigma_l:.4f} | "
                f"D=0: t_T1={t_t1:.4f} w_T1={w_t1:.4f} TOST={eq0:.4f} | "
                f"D=ln(1.5): t={t15:.4f} w={w15:.4f} R10_sup={r10_15:.4f} | "
                f"D=ln(2.0): t={t20:.4f} R10_sup={r10_20:.4f}{ex_str}"
            )


def eval_part2_delta(ya: np.ndarray, yb0: np.ndarray, sa: np.ndarray, sb: np.ndarray, ll: np.ndarray, delta: float):
    """Vectorized grouped complete-pair t/Wilcoxon/TOST + exact Lee (2009) marginal bounds."""
    yb = yb0 + delta
    n_rep, _ = ya.shape
    t_rej = np.zeros(n_rep, dtype=bool)
    w_rej = np.zeros(n_rep, dtype=bool)
    tost_ok = np.zeros(n_rep, dtype=bool)

    n_ll_arr = ll.sum(axis=1)
    for k in np.unique(n_ll_arr):
        k_int = int(k)
        if k_int < 2:
            continue
        rows = np.flatnonzero(n_ll_arr == k_int)
        d_sub = (yb[rows] - ya[rows])[ll[rows]].reshape(rows.size, k_int)
        m_sub = d_sub.mean(axis=1)
        se_sub = d_sub.std(axis=1, ddof=1) / math.sqrt(k_int)
        df = k_int - 1
        t_crit = float(stats.t.ppf(1.0 - ALPHA_HOLM1 / 2.0, df))
        t_tost = float(stats.t.ppf(1.0 - ALPHA_TOST, df))
        t_stats = np.divide(m_sub, se_sub, out=np.zeros_like(m_sub), where=se_sub > 0.0)
        tost_ok[rows] = np.abs(m_sub) + t_tost * se_sub < SESOI_L
        if delta == 0.0:
            t_rej[rows] = np.abs(t_stats) > t_crit
            if k_int >= MIN_NZ_R7:
                wp = stats.wilcoxon(d_sub, axis=1, zero_method="wilcox", alternative="two-sided", method="approx").pvalue
                w_rej[rows] = wp < ALPHA_HOLM1
        else:
            t_rej[rows] = t_stats > t_crit
            if k_int >= MIN_NZ_R7:
                wp = stats.wilcoxon(d_sub, axis=1, zero_method="wilcox", alternative="two-sided", method="approx").pvalue
                w_rej[rows] = (m_sub > 0.0) & (wp < ALPHA_HOLM1)

    # Vectorized exact Lee (2009) marginal trimming bounds grouped by (na, nb)
    na_arr = sa.sum(axis=1)
    nb_arr = sb.sum(axis=1)
    lee_lo = np.full(n_rep, np.nan, dtype=np.float64)
    lee_hi = np.full(n_rep, np.nan, dtype=np.float64)
    pairs = np.column_stack((na_arr, nb_arr))
    for pair in np.unique(pairs, axis=0):
        na_val, nb_val = int(pair[0]), int(pair[1])
        if na_val == 0 or nb_val == 0:
            continue
        rows = np.flatnonzero((na_arr == na_val) & (nb_arr == nb_val))
        va = np.sort(ya[rows][sa[rows]].reshape(rows.size, na_val), axis=1)
        vb = np.sort(yb[rows][sb[rows]].reshape(rows.size, nb_val), axis=1)
        k = min(na_val, nb_val)
        lee_lo[rows] = vb[:, :k].mean(axis=1) - va[:, -k:].mean(axis=1)
        lee_hi[rows] = vb[:, -k:].mean(axis=1) - va[:, :k].mean(axis=1)

    valid_lee = ~np.isnan(lee_lo)
    return (
        float(np.mean(t_rej)),
        float(np.mean(w_rej)),
        float(np.mean(tost_ok)),
        float(np.mean((lee_hi - lee_lo)[valid_lee])),
        float(np.mean(lee_lo[valid_lee] > 0.0)),
    )


def part2_attrition_lee_rmst(seed_seq: np.random.SeedSequence) -> None:
    print(f"\n=== Part 2: Complete-Pair, Lee Bounds & RMST across Attrition (sigma_L = {SIGMA_L_REF:.4f}, d_z(ln1.5)=0.50) ===")
    scenarios = [
        ("V1", "VIAB-1b obs (0%/0%, rho=0.85, norm)", 0.0000, 0.0000, 0.85, 0.00, "norm"),
        ("V2c", "Pooled UB sym (4.37%/4.37%, corr rE=0.85)", 0.0437, 0.0437, 0.85, 0.85, "norm"),
        ("V2i", "Pooled UB sym (4.37%/4.37%, ind rE=0.00)", 0.0437, 0.0437, 0.85, 0.00, "norm"),
        ("V3", "Asym U-arm stress (A=5%, B=0%, rho=0.60)", 0.0500, 0.0000, 0.60, 0.00, "norm"),
        ("V4", "Asym U-arm 80% UB (A=12.55%, B=0%, rho=0.60)", 0.1255, 0.0000, 0.60, 0.00, "norm"),
        ("V5a", "Heavy-tail t5 (0%/0%, rho=0.85)", 0.0000, 0.0000, 0.85, 0.00, "t5"),
        ("V5b", "Heavy-tail t5 asym (A=5%, B=0%, rho=0.60)", 0.0500, 0.0000, 0.60, 0.00, "t5"),
    ]
    scale_t5 = math.sqrt(3.0 / 5.0)

    for sid, desc, pa, pb, rho_y, r_ext, dist in scenarios:
        print(f"\n[{sid}] {desc}")
        sigma_arm = SIGMA_L_REF / math.sqrt(2.0 * (1.0 - rho_y))
        chol_y = np.array([[1.0, 0.0], [rho_y, math.sqrt(1.0 - rho_y ** 2)]])
        chol_e = np.array([[1.0, 0.0], [r_ext, math.sqrt(1.0 - r_ext ** 2)]])
        ta = float(stats.norm.ppf(pa)) if pa > 0.0 else -math.inf
        tb = float(stats.norm.ppf(pb)) if pb > 0.0 else -math.inf

        for n in N_GRID:
            rng = np.random.default_rng(seed_seq.spawn(1)[0])
            if dist == "norm":
                zy = rng.standard_normal((N_REP, n, 2))
            else:
                zy = rng.standard_t(df=5, size=(N_REP, n, 2)) * scale_t5
            latent = zy @ chol_y.T * sigma_arm
            ya = latent[:, :, 0]
            yb0 = latent[:, :, 1]

            if pa == 0.0 and pb == 0.0:
                sa = np.ones((N_REP, n), dtype=bool)
                sb = np.ones((N_REP, n), dtype=bool)
            else:
                ze = rng.standard_normal((N_REP, n, 2)) @ chol_e.T
                sa = ze[:, :, 0] > ta
                sb = ze[:, :, 1] > tb

            ll = sa & sb
            n_ll = ll.sum(axis=1)
            pi_ll = n_ll / n
            p_pi85 = float(np.mean(pi_ll >= 0.85))
            mean_n_ll = float(np.mean(n_ll))

            rmst_a = np.full((N_REP, n), TAU, dtype=np.float64)
            rmst_b = np.full((N_REP, n), TAU, dtype=np.float64)
            if pa > 0.0:
                rmst_a[~sa] = rng.choice(EXT_TICKS_VIAB1, size=int((~sa).sum()))
            if pb > 0.0:
                rmst_b[~sb] = rng.choice(EXT_TICKS_VIAB1, size=int((~sb).sum()))
            rmst_p = rmst_p_matrix(rmst_b, rmst_a)
            rmst_rate = float(np.mean(rmst_p < ALPHA_HOLM1))

            t0, w0, eq0, _, _ = eval_part2_delta(ya, yb0, sa, sb, ll, 0.0)
            t15, w15, _, lw15, lgt15 = eval_part2_delta(ya, yb0, sa, sb, ll, SESOI_L)
            t20, _, _, _, lgt20 = eval_part2_delta(ya, yb0, sa, sb, ll, LN_2)
            print(
                f"  N={n:2d} nLL={mean_n_ll:5.2f} P(pi>=0.85)={p_pi85:.4f} | "
                f"D=0: tT1={t0:.4f} wT1={w0:.4f} TOST={eq0:.4f} RMST={rmst_rate:.4f} | "
                f"D=ln(1.5): t={t15:.4f} w={w15:.4f} LeeW={lw15:.4f} P(Lee_lo>0)={lgt15:.4f} | "
                f"D=ln(2.0): t={t20:.4f} P(Lee_lo>0)={lgt20:.4f}"
            )


def part3_bug_fixes(seed_seq: np.random.SeedSequence) -> None:
    print("\n=== Part 3: Verification of Reported Bug Fixes ===")
    print("  [1] Kerby r_c calibration (r_c = 2 * Phi(sqrt(2) * d_z) - 1):")
    for dz in (0.50, 0.80):
        rc_exact = 2.0 * float(stats.norm.cdf(math.sqrt(2.0) * dz)) - 1.0
        rng = np.random.default_rng(seed_seq.spawn(1)[0])
        diffs = rng.normal(loc=dz, scale=1.0, size=(N_REP, 64))
        ranks = stats.rankdata(np.abs(diffs), method="average", axis=1)
        r_plus = np.sum(np.where(diffs > 0.0, ranks, 0.0), axis=1)
        r_minus = np.sum(np.where(diffs < 0.0, ranks, 0.0), axis=1)
        rc_vals = (r_plus - r_minus) / (r_plus + r_minus)
        mc_mean = float(np.mean(rc_vals))
        mc_se = float(np.std(rc_vals, ddof=1) / math.sqrt(N_REP))
        print(f"    d_z = {dz:.2f} -> exact r_c = {rc_exact:.4f} (MC N=64: {mc_mean:.4f} +/- {mc_se:.4f})")

    print("  [2] R1 Seed Formula Collision Guard (seed_k = 810001 + 1000 * k):")
    assert 810001 + 1000 * 70 == 880001
    seeds_e3b = [810001 + 1000 * k for k in range(64)]
    assert len(seeds_e3b) == 64 and len(set(seeds_e3b)) == 64
    assert seeds_e3b[0] == 810001 and seeds_e3b[-1] == 873001
    assert max(seeds_e3b) < 880001, "Capped at N <= 70 (k <= 69) so max seed < 880001"
    forbidden = (
        set(range(1, 4))
        | set(700001 + 1000 * k for k in range(12))
        | {999983}
        | set(730001 + 1000 * k for k in range(12))
        | {999979}
        | set(750001 + 1000 * k for k in range(12))
        | set(760001 + 1000 * k for k in range(12))
        | {999971}
        | set(780001 + 1000 * k for k in range(20))
        | set(range(799501, 799507))
        | {999961}
        | set(range(999801, 999823))
        | set(range(880001, 880013))
        | {999953}
    )
    overlap = set(seeds_e3b) & forbidden
    assert not overlap, f"Overlap found: {overlap}"
    print(f"    k=70 collision = {810001 + 1000 * 70}; N=64 range = [{seeds_e3b[0]}..{seeds_e3b[-1]}] (< 880001); forbidden overlap = {len(overlap)}")

    print("  [3] R10 Exhaustive 48/48 Partition Check (Default Option (a) 4-label set):")
    counts = {"supports": 0, "opposite": 0, "equivalent_to_zero": 0, "insufficient_evidence": 0}
    for wp2_valid, strat_valid, holm_sig, eff, tost_eq in itertools.product(
        (True, False), (True, False), (True, False), ("pos_ge_sesoi", "neg_ge_sesoi", "lt_sesoi"), (True, False)
    ):
        matches = []
        if not wp2_valid:
            matches.append("insufficient_evidence")
        elif not strat_valid:
            matches.append("insufficient_evidence")
        elif eff != "lt_sesoi" and tost_eq:
            matches.append("insufficient_evidence")
        elif holm_sig and eff == "pos_ge_sesoi" and not tost_eq:
            matches.append("supports")
        elif holm_sig and eff == "neg_ge_sesoi" and not tost_eq:
            matches.append("opposite")
        elif holm_sig and eff == "lt_sesoi" and tost_eq:
            matches.append("equivalent_to_zero")
        elif holm_sig and eff == "lt_sesoi" and not tost_eq:
            matches.append("insufficient_evidence")
        elif (not holm_sig) and eff == "lt_sesoi" and tost_eq:
            matches.append("equivalent_to_zero")
        elif (not holm_sig) and not tost_eq:
            matches.append("insufficient_evidence")
        assert len(matches) == 1, (wp2_valid, strat_valid, holm_sig, eff, tost_eq, matches)
        counts[matches[0]] += 1
    assert sum(counts.values()) == 48
    print(f"    ENUM_OK (48/48 unique matches): {counts}")


def main() -> None:
    parser = argparse.ArgumentParser(description="E3B-P-v2 blind Monte Carlo power & verification script.")
    parser.add_argument(
        "--viab1b-cell",
        type=pathlib.Path,
        default=None,
        help="Optional path to artifacts/experiments/viab1b-founder-2seg/cells/e3b_fg1_g4_fl1.json",
    )
    args = parser.parse_args()

    t0 = time.time()
    print(
        f"E3B-P-v2 Pre-Freeze Verification & MC | N_REP={N_REP} | BASE_SEED={BASE_SEED} | "
        f"ALPHA_HOLM1={ALPHA_HOLM1:.6f} | SESOI_L=ln(1.5)={SESOI_L:.6f}"
    )
    verify_viab1b_cell(args.viab1b_cell)
    ss = np.random.SeedSequence(BASE_SEED)
    s1, s2, s3 = ss.spawn(3)
    part1_sigma_l_sweep(s1)
    part2_attrition_lee_rmst(s2)
    part3_bug_fixes(s3)
    print(f"\nCompleted in {time.time() - t0:.2f}s")


if __name__ == "__main__":
    main()
```
