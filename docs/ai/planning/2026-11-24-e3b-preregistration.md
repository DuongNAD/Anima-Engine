---
phase: planning
feature: experiment-e3b-productivity-gradient
title: Preregistration (Frozen) — E3b: Spatial Productivity Gradient and Body-Mass Variance Ratio
description: Frozen E3b preregistration pinned to trunk b0413f2eeecbab8e5ee045c22c72d5c00c16b87a with locked QD-E3B D1-D5 and QD-E3B-D1a decisions, 31-key LIVE_KEYS partition, explicit 64 confirmation seed triples (810001..=873001), blind smoke seed 999953, L_i with SESOI +-ln(1.5), Holm-Bonferroni m=3, TOST, Lee bounds, RMST, mediator analysis, and closed 4-label R10 decision table
status: active
owner: anima-case
last_reviewed: 2026-11-24
---

# Preregistration (Frozen) — E3b: Spatial Productivity Gradient and Body-Mass Variance Ratio

> **FROZEN PREREGISTRATION (`status: active` per [`DOCUMENTATION_POLICY.md`](../../governance/DOCUMENTATION_POLICY.md), which reserves `active` rather than `frozen` for live preregistrations — matching [Frozen E3](2026-08-26-experiment-e3-productivity-gradient-body-mass.md); JSON contract carries `"status": "FROZEN"`).**
> - **Pinned trunk commit:** `b0413f2eeecbab8e5ee045c22c72d5c00c16b87a` (`b0413f2`, merge of `PR #229` approving reviewed draft [2026-11-23-e3b-preregistration-draft.md](2026-11-23-e3b-preregistration-draft.md)). Zero decisions changed from `E3B-P-v2`.
> - **Confirmation seeds executed at freeze:** `0` (`contains_results: false`). Neither confirmation seeds (`810001..=873001`) nor smoke seed (`999953`) have been executed during freeze.
> - **Body-mass blindness (`QD-VIAB`):** `0` confirmation or calibration `live.trait_mass_*` values have been consulted or read prior to confirmation. Smoke manifests omit all `6` `trait_mass` observable IDs structurally.
> - **Machine-readable contract & gates:** [`e3b-preregistration.json`](../../../src-tauri/tests/fixtures/experiments_e3b/e3b-preregistration.json) + `6` arm/smoke manifests under `src-tauri/tests/fixtures/experiments_e3b/`, verified by [`prereg_e3b_manifest_tests.rs`](../../../src-tauri/tests/prereg_e3b_manifest_tests.rs) and [`verify_e3b_artifacts.mjs`](../../../scripts/verify_e3b_artifacts.mjs) (`npm run check:e3b`).

---

## 0. Provenance & Locked Owner Decisions (`D1`–`D5` + `QD-E3B-D1a`)

| Role | Reference | Revision / Commit | Body-Mass Values Used |
|---|---|---|---|
| Reviewed draft frozen here (`E3B-P-v2`) | [2026-11-23-e3b-preregistration-draft.md](2026-11-23-e3b-preregistration-draft.md) (`PR #229`) | `b0413f2eeecbab8e5ee045c22c72d5c00c16b87a` (`b0413f2`) | **None (`0` confirmation or calibration values)** |
| Blind 2-segment calibration (`VIAB-1b`) | [2026-11-23-viab1b-results.md](2026-11-23-viab1b-results.md) (`PR #228`) | `c2e0b6e0` (trunk `98054ba2`) | **None (`0` `trait_mass` matches across all files)** |
| Historical 3-segment calibration (`VIAB-1`) | [2026-11-23-viab1-results.md](2026-11-23-viab1-results.md) (`PR #215`) | `d49cf4ff` | **None** |
| Owner decisions `D1`–`D5` & `QD-E3B-D1a` | `QD-E3B` in [AGENT_WORKFLOW.md](../../how-to/AGENT_WORKFLOW.md) (`PR #223`) & Owner sign-off (`PR #229`) | `b0413f2` | **None** |
| 2-segment founder implementation (`FOUNDER-2SEG`) | [2026-11-23-founder-2seg.md](2026-11-23-founder-2seg.md) (`PR #224`) | Trunk `98054ba2` | Declared schema constant `1.5 + 1.0 = 2.5` only |
| Founder diagnosis provenance | [2026-11-23-mut0-collapse-diagnosis.md](2026-11-23-mut0-collapse-diagnosis.md) (`PR #216`) | `9062a7c3` | Cited `3.3 -> ~2.5` diagnosis on probe seeds `880001..=880006` only |
| Predecessor E3 & E3-X | [Frozen E3](2026-08-26-experiment-e3-productivity-gradient-body-mass.md) & [E3-X Results](2026-11-20-e3-x-confirmation-results.md) | `9958f27` / `24c6fb1c` | **None consulted** (`S/T/U` survival & timing only) |
| Statistics template & research | [Prereg Template](2026-11-23-prereg-statistics-template.md) & [VIABILITY_PRICE §3.3–3.5](../../research/VIABILITY_PRICE_STATISTICS_RESEARCH_2026-11-22.md) | `3fee64d5` | None |

---

## 1. World Configuration, `31` `LIVE_KEYS` Partition & Locked Disclosures

### 1.1. Exact `31` `LIVE_KEYS` Configuration on Trunk `b0413f2`

E3b uses `core::live_experiment::LiveExperimentAdapter` (`LIVE_MODEL_VERSION = "live-bevy-world/2"`, snapshot schema `8`) with `MapSettings::default()` (`seed = 1337`, `256x256` `Generated` terrain; informational `WorldIdentity` checksum on Windows 1 `x86_64`: `3152323152`, never pinned across host OS/architectures). Every manifest carries all §1.1 overrides explicitly (including `live.founder_genotype = 1.0`, `live.predator_fraction = 0.3`, and `live.mutation_rate = 0.05` which equals the engine default and is written explicitly per freeze specification), leaving all remaining `11` engine-default keys absent and registered in `default_keys_left_absent` (`20 + 11 = 31 == LIVE_KEYS.len()`):

- **20 explicit keys per manifest (`19` shared + `1` per-arm `live.resource_capacity_scale`):**
  1. `live.ecological_fitness = 1.0`
  2. `live.mortality = 1.0`
  3. `live.reproduction = 1.0`
  4. `live.locomotion = 1.0`
  5. `live.drive_floor = 0.5`
  6. `live.thirst_steering = 1.0`
  7. `live.evolved_brains = 0.0`
  8. `live.founders = 160.0` *(changed vs E3 `120.0`)*
  9. `live.founder_layout = 1.0` *(changed vs E3 `0.0` / absent: 2D land-only grid)*
  10. `live.founder_genotype = 1.0` *(changed vs E3 `0.0` / absent: `D5` Opt. `(b)` 2-seg clone)*
  11. `live.predator_fraction = 0.3` *(`D1` + `QD-E3B-D1a` locked)*
  12. `live.trees = 8.0`
  13. `live.lakes = 16.0` *(changed vs E3 `2.0`)*
  14. `live.lake_layout = 1.0` *(changed vs E3 `0.0` / absent: 2D lattice)*
  15. `live.food_cap = 50.0`
  16. `live.prey_cap = 160.0` *(changed vs E3 `80.0` default)*
  17. `live.strict_prey_cap = 1.0` *(changed vs E3 `0.0` / absent: F-052 fix)*
  18. `live.dehydration_ticks = 960.0` *(changed vs E3 `120.0` default)*
  19. `live.mutation_rate = 0.05` *(explicitly declared in all E3b manifests)*
  20. `live.resource_capacity_scale` = `1.0` (`control_uniform`, `treatment_gradient`) or **`0.42`** (`control_uniform_scarcity`)
- **11 keys at `LiveWorldConfig::default()` left absent from manifests and locked in `default_keys_left_absent`:**
  `live.w_surplus = 1.0`, `live.starve_ticks = 120`, `live.plant_reserve = 0.0`, `live.kill_hydration = 0.0`, `live.kill_efficiency = 0.4`, `live.target_min_energy = 0.0`, `live.drift_control = 0.0`, `live.drift_reset_ticks = 0`, `live.land_mask = 0`, `live.grass_steering = 0`, `live.lineage_tracking = 0`.

### 1.2. Locked `D1` & `QD-E3B-D1a`: Two-Trophic Experiment With Persistent Predators (`live.predator_fraction = 0.3`)

Per Owner decisions `D1` and `QD-E3B-D1a` (*"Giữ 0.3, có thú săn"*), E3b fixes **`live.predator_fraction = 0.3`** (`round(160 * 0.3) = 48` initial predators and `112` initial herbivores). Contrary to 3-segment `e3_b1_g4_fl1` (`pred_final > 0`: `0/12` in all arms), `VIAB-1b` (`PR #228`) established that under `live.founder_genotype = 1.0`, predators persist to `T = 18,000` in **`12/12` (`U`: mean `8.00`, `3..11`), `11/12` (`S`: mean `2.50`, `0..8`), and `12/12` (`T`: mean `5.67`, `1..8`)** (and identically `12/11/12` under `mut0`).

Mandatory reporting obligations carried into all E3b reports:
1. State explicitly that E3b is a **two-trophic experiment with persistent predators** (`~2.5` to `8.0` final predators across arms), never *"effectively herbivore-only"*, and co-report `live.predator_count` (`U, S, T`) as an exploratory ecological covariate.
2. `live.shannon` (`S4`, two-class herbivore–predator Shannon balance in `live_experiment.rs`) is no longer structurally forced to `0.0`, but measures trophic class balance rather than morphological body-mass diversity and carries `decision_effect: none`.

### 1.3. Locked `D5` Option `(b)`: 2-Segment Founder, Operator-4 Wall & Wording Limits

- **Exact genotype (`live.founder_genotype = 1.0`):** `nodes = [{id: 0, length: 1.0, radius: 0.2, mass: 1.5}, {id: 1, length: 1.0, radius: 0.2, mass: 1.0}]`, `edges = [{source_node: 0, target_node: 1, joint_anchor: [1.0, 0.0, 0.0], joint_axis: [0.0, 0.0, 1.0]}]`, **`total_mass = 2.5`**, canonical `serde_json` SHA-256 **`dfeb276205f818acf3f34f5d716cbf163598a03a23ffc6964ff85753e280c719`**. Initial `live.trait_mass_var` at tick `0` is `0.0` across all arms.
- **Disclosed operator-4 wall ([`mutation.rs:210-230`](../../../src-tauri/src/evolution/mutation.rs#L210-L230)):** Structural node deletion requires `genotype.nodes.len() > 2`; the `2`-node founder sits directly on this boundary, so structural mutations can only add segments (`>= 3`), never prune to `1` segment (continuous Gaussian node mass/size mutations act bilaterally).
- **Disclosed provenance:** Choice of the 2-segment clone originates from the lead's `PR #216` diagnosis inspecting probe seeds `880001..=880006` (`3.3 -> ~2.5`), disjoint from E3b confirmation seeds `810001..=873001`, and blindly verified `12/12` in `VIAB-1b` (`mut` and `mut0`).
- **Mandatory biological interpretation wording:** Under `T = 18,000` ticks, realized generation turnover (`births / mean_N`) in `VIAB-1b` is **`3.091 / 2.111 / 3.041`** (`U / S / T`; `~3.1 / 2.1 / 3.0` generations). All E3b claims are restricted to stating: *"body-mass variance ratio after ~k generations (`~3.1 / 2.1 / 3.0` in `U / S / T`) of de novo input from an adapted clonal founder (`live.founder_genotype = 1`, with persistent predators)"*, and explicitly noting `T`'s recurring `Deforest 0.30` / `AddNutrient 1.00` mechanism vs `S`'s genesis `resource_capacity_scale = 0.42`.

---

## 2. Experimental Arms, Primary Metric `L_i` & Hypotheses (`D2`, `D4`)

### 2.1. Three Matched Arms (`U`, `S`, `T`)

| Item | `control_uniform` (`U`) | `control_uniform_scarcity` (`S`) | `treatment_gradient` (`T`) |
|---|---|---|---|
| Confirmation manifest | `e3b-control-uniform.json` | `e3b-control-uniform-scarcity.json` | `e3b-treatment-gradient.json` |
| Smoke manifest (`seed = 999953`) | `e3b-smoke-control-uniform.json` | `e3b-smoke-control-uniform-scarcity.json` | `e3b-smoke-treatment-gradient.json` |
| `live.resource_capacity_scale` | `1.0` (`153,587.25 EU` plants) | **`0.42`** (`64,506.64 EU` plants + `89,080.61 EU` detritus) | `1.0` (`153,587.25 EU` plants) |
| Intervention `id=1` (`Deforest`, West `Rect{0,0,127,255}`) | `intensity = 0.0` | `intensity = 0.0` | **`intensity = 0.30`** |
| Intervention `id=2` (`AddNutrient`) | East `Rect{128,0,255,255}`, `intensity = 0.0` | **Whole-map `Rect{0,0,255,255}`, `intensity = 1.00`** | **East `Rect{128,0,255,255}`, `intensity = 1.00`** |
| Schedule (`start / duration / curve`) | `1 / 18000 / Step` | `1 / 18000 / Step` | `1 / 18000 / Step` |
| Observables requested | All `28` live observables | All `28` live observables | All `28` live observables |
| Smoke observables (`seed = 999953`) | `22` non-mass only (`0` `trait_mass`) | `22` non-mass only (`0` `trait_mass`) | `22` non-mass only (`0` `trait_mass`) |

### 2.2. Primary Metric `L_i`, Confirmatory Family (`m = 3`) & Sensitivity (`D4`)

Let $V_{A,i}$ be `live.trait_mass_var` at `T = 18,000` (`final_observables`) in arm $A \in \{U, S, T\}$ on valid pair $i$ (`survived && live.agent_count >= 2` at `T` and throughout sampled grid; `agent_count < 2` is singleton-excluded, `agent_count == 0` is extinct-excluded). For ordered contrast $(B/A)$:

$$L_i(B/A;\,\varepsilon) = \ln\!\left(\frac{V_{B,i} + \varepsilon}{V_{A,i} + \varepsilon}\right)$$

- **Primary pseudocount:** **$\varepsilon = 5 \times 10^{-4}$**. Mandatory sensitivity recomputation at **$\varepsilon_{\text{sens}} = 5 \times 10^{-5}$** on the same complete-pair set (diagnostic only; never overrides primary $\varepsilon$).
- **Confirmatory family (`m = 3`, Holm–Bonferroni FWER $\alpha = 0.05$, thresholds `0.016667`, `0.025`, `0.05`):**
  1. **`H1` (`T / S`: $L_i = \ln[(V_{T,i}+\varepsilon)/(V_{S,i}+\varepsilon)]$, directional `T > S`, `D2` Option `(a)`):** Primary test isolating the spatial gradient at matched `~64k EU` standing crop.
  2. **`H2` (`T / U`: $L_i = \ln[(V_{T,i}+\varepsilon)/(V_{U,i}+\varepsilon)]$, two-sided):** Total gradient + scarcity effect relative to abundant uniform control.
  3. **`H3` (`S / U`: $L_i = \ln[(V_{S,i}+\varepsilon)/(V_{U,i}+\varepsilon)]$, two-sided):** Uniform scarcity + whole-map `AddNutrient 1.00` refill effect relative to abundant uniform control.
- **Test & robustness co-reports:** Two-sided paired $t$-test (`df = n_LL - 1`) with 95% $t$-CI and 90% TOST $t$-CI against `±ln(1.5)` (`±0.4054651081081644`), co-reported with Wilcoxon signed-rank on $L_i$. Across seeds valid in all 3 arms, $\bar{L}(T/U) = \bar{L}(T/S) + \bar{L}(S/U)$ holds identically per seed.

### 2.3. Exploratory Family (BH-FDR `q = 0.05`) & Mediator (`D4`)

1. **Mandatory turnover mediator analysis:** Regress (OLS slope, intercept, $R^2$, Pearson $r$, Spearman $\rho$) $L_i(B/A)$ against $\ln(\text{births}_{B,i} / \text{births}_{A,i})$ across `H1`, `H2`, and `H3`, where `births` is the 30-sample cumulative sum of `live.births`.
2. **Secondary / exploratory endpoints (`decision_effect: none` on `H1..H3`):**
   - Unregularized & Bessel-corrected (`var * n / (n - 1)`) raw variance deltas, `lnCVR`, standardized $d_z$, Kerby matched-pairs $r_c$ (calibrated benchmarks `0.5205` for `d_z = 0.50`, `0.7421` for `d_z = 0.80`), Hodges–Lehmann pseudomedian on $L_i$ with deterministic 95% BCa CI (`10,000` bootstrap resamples).
   - `H1_tail_window`: 7-sample tail mean over `[14400, 18000]` applied to `L_i`.
   - `S1` (`trait_mass_p90 - trait_mass_p10`), `S2` (`trait_mass_mean`), `S3` (`trait_mass_mean_east - trait_mass_mean_west` under `empty_half_rule`), `S6` (`births` 30-sum, `deaths` 30-sum, **`live.mean_lifespan` read from `series.csv` at tick `18,000` + positive-death post-warmup mean — NEVER `final_observables`**, avoiding `F-053` drained-window structural zero; plus `live.cumulative_mean_lifespan` at `T`), `live.predator_count`, `RMST(tau=18000)`, `S4` (`live.shannon`), `S5` (`live.archive_coverage`, structurally `0`), `I1` (`live.closed_eu_total <= 1e-6`), and `M1` (`live.standing_crop` `WP-2 b1/b2`).

---

## 3. Pre-Registered Statistical Rules `R1`–`R10`

| Rule | Locked Specification |
|---|---|
| **R1** Unit & Disjoint Seeds | Paired CRN across `{U, S, T}`; **`seed_k = 810001 + 1000 * k` capped at `N <= 70`** (`k <= 69`, `max <= 879001 < 880001`); locked **`N = 64`** (`810001..=873001`); smoke seed **`999953`** |
| **R2** Multiplicity (`D2`) | Confirmatory `m = 3` (`H1, H2, H3` on $L_i$, Holm $\alpha = 0.05$: `0.016667, 0.025, 0.05`; directional `H1` Opt. `(a)`); Exploratory BH-FDR `q = 0.05` + mediator |
| **R3** Attrition (`D3`) | `H1 (T/S)`: complete-pair primary (`pi_LL >= 0.85` i.e. `n_LL >= 55`, `n_LL >= 7`, `0` failed runs); `H2 (T/U)` & `H3 (S/U)`: complete-pair primary + **Lee (2009) trimming bounds** on `Y = ln(var + eps)` + **separate survival `RMST(tau=18000)`** on all `64` seeds |
| **R4** SESOI & MDE (`D4`) | **SESOI $= \pm\ln(1.5) = \pm 0.4054651081081644$** on $\bar{L}$ (`[0.667x, 1.500x]` variance ratio); pre-registered achieved MDE benchmark **$0.5 \cdot \hat{\sigma}_L$** (exact 80% Holm-1 MDE `0.4139 * sigma_hat_L`) |
| **R5** Blind Nuisance Params | From `VIAB-1b` (`880001..=880012`, `0` `trait_mass`): `0/36` extinct, `min_n >= 58`, `mean_n = 178.8 / 167.1 / 178.4`, `pred_final > 0: 12/11/12`, `generations = 3.091 / 2.111 / 3.041`, `ln(births_T/births_S) = +0.4396 +/- 0.1557` |
| **R6 / R6b** Power & MC (`N=64`) | Superiority over zero at `Delta = ln(1.5)` is `93.50%` analytical (`93.56%` MC, `V1`) at `sigma_L = 0.8109` (`0.5*sd = ln(1.5)`); `>= 80%` superiority up to `sigma_L = 0.9796`; `>= 80%` TOST up to `sigma_L = 1.0966`; strict R10 `supports` `~49–50%` at boundary `ln(1.5)` and `97.6–100%` at `ln(2.0)` (`Appendix A` of [draft](2026-11-23-e3b-preregistration-draft.md#appendix-a--reproducible-l_i-monte-carlo--bug-fix-verification-script)) |
| **R7** Exact Signed-Rank Floor | Minimum non-zero pairs $n \ge 7$ (`2/2^7 = 0.015625 < 0.016667`); at `N = 64`, $2/2^{64} = 1.08 \times 10^{-19}$ |
| **R8** Effect-Size Bundle | Primary $\bar{L}$ (95% & 90% CI) + `eps = 5e-5` check + Lee (`H2/H3`) + `RMST`; secondary $d_z$, $r_c$ (`0.5205` / `0.7421`), HL + 95% BCa CI, `lnCVR`, raw diffs, mediator |
| **R9** TOST Equivalence | **90% paired $t$-CI of $\bar{L}$ strictly inside $(-\ln(1.5), +\ln(1.5)) = (-0.405465, +0.405465)$** ($\alpha_{\text{TOST}} = 0.05$ each tail) |
| **R10** 4-Label Decision Table | `{supports, opposite, equivalent_to_zero, insufficient_evidence}` (`opposite` strictly refutes `H1`; `48/48` unique discrete partition) |

### 3.1. Exact Locked Confirmation Seed Schedule (`N = 64`, `810001..=873001`)

Formula: `seed_k = 810001 + 1000 * k` for `k = 0..=63` (enforcing `N <= 70` so `k <= 69` and `max_seed <= 879001 < 880001`, preventing the `k = 70 -> 880001` calibration collision):

| `k = 0..15` | `k = 16..31` | `k = 32..47` | `k = 48..63` |
|---|---|---|---|
| `0`: `810001` | `16`: `826001` | `32`: `842001` | `48`: `858001` |
| `1`: `811001` | `17`: `827001` | `33`: `843001` | `49`: `859001` |
| `2`: `812001` | `18`: `828001` | `34`: `844001` | `50`: `860001` |
| `3`: `813001` | `19`: `829001` | `35`: `845001` | `51`: `861001` |
| `4`: `814001` | `20`: `830001` | `36`: `846001` | `52`: `862001` |
| `5`: `815001` | `21`: `831001` | `37`: `847001` | `53`: `863001` |
| `6`: `816001` | `22`: `832001` | `38`: `848001` | `54`: `864001` |
| `7`: `817001` | `23`: `833001` | `39`: `849001` | `55`: `865001` |
| `8`: `818001` | `24`: `834001` | `40`: `850001` | `56`: `866001` |
| `9`: `819001` | `25`: `835001` | `41`: `851001` | `57`: `867001` |
| `10`: `820001` | `26`: `836001` | `42`: `852001` | `58`: `868001` |
| `11`: `821001` | `27`: `837001` | `43`: `853001` | `59`: `869001` |
| `12`: `822001` | `28`: `838001` | `44`: `854001` | `60`: `870001` |
| `13`: `823001` | `29`: `839001` | `45`: `855001` | `61`: `871001` |
| `14`: `824001` | `30`: `840001` | `46`: `856001` | `62`: `872001` |
| `15`: `825001` | `31`: `841001` | `47`: `857001` | `63`: `873001` |

- **Smoke seed:** `999953` (present only in `e3b-smoke-*.json`; excluded mechanically from confirmation manifests and confirmation analysis).
- **Disjointness proof:** `810001..=873001` has `0` overlap with `VIAB-1` / `VIAB-1b` calibration (`880001..=880012`), `PR #216` probes (`880001..=880006`), `E2` (`700001..=711001`, `999983`), `E3` (`730001..=741001`, `999979`), `E2R` (`750001..=761001`), `E-SEL1` (`760001..=771001`, `999971`), `SEL2-P` (`780001..=799506`, `999961`, `999801..=999822`), `AE` (`1..=3`), and smoke (`999953`).

### 3.2. Closed 4-Label `R10` Mapping (`D2` Option `(a)`)

| `WP-2` & Host Valid | Complete-Pair Valid (`0` fail, $\hat{\pi}_{LL} \ge 0.85$, $n_{LL} \ge 7$) | $p_{\text{Holm}} < 0.05$ | Point $\bar{L}$ ($\text{SESOI} = \ln(1.5)$) | TOST 90% CI Inside $(-\text{SESOI}, +\text{SESOI})$ | `R10` Label | `direction_tag` |
|---|---|---|---|---|---|---|
| No | Any | Any | Any | Any | **`insufficient_evidence`** | `harness_invalid` |
| Yes | No | Any | Any | Any | **`insufficient_evidence`** | `attrition_below_pi85_floor` |
| Yes | Yes | Yes | `+` (`H1` > `SESOI`, or `\|L\| >= SESOI` for `H2,H3`) | No | **`supports`** | `predicted_positive_L` / `non_zero_two_sided_L` |
| Yes | Yes | Yes | `H1` `L <= -SESOI` (`T < S`) | No | **`opposite`** | `opposite_negative_L` |
| Yes | Yes | Yes | `\|L\| < SESOI` | Yes | **`equivalent_to_zero`** | `trivial_below_sesoi_equivalent` |
| Yes | Yes | Yes | `\|L\| < SESOI` | No | **`insufficient_evidence`** | `inconclusive_straddles_sesoi` |
| Yes | Yes | No | `\|L\| < SESOI` | Yes | **`equivalent_to_zero`** | `equivalent_to_zero_powered_null` |
| Yes | Yes | No | Any | No | **`insufficient_evidence`** | `inconclusive_wide_ci` |
| Yes | Yes | Any | `\|L\| >= SESOI` | Yes *(guard)* | **`insufficient_evidence`** | `ci_inconsistent_impossible` |

---

## 4. Execution Host, Blind Smoke Gates & Run Budget

1. **Single execution host:** All `192` confirmation runs (`64` seeds × `3` arms) and the blind smoke triple (`seed = 999953`) run on **`Windows 1` (`windows/x86_64`)** from a single `--release --features desktop` binary logged in `provenance.json`.
2. **World isolation & EU conservation:** Verify `ANIMA_WORLD_ARTIFACT` is unset, `<temp_dir>/anima_world_artifact.anmw` is absent, `terrain_source == "generated"`, `world_identity` matches across all runs, and `max_abs_eu_drift <= 1.0e-6 EU`.
3. **Blind smoke gates (`WP-2`, `seed = 999953`, `body_mass_consulted: false`):**
   - `WP-2(a)`: `256x256` `ResourceField`, identical `153,587.25 EU` initial closed EU across `U, S, T`.
   - `WP-2(b1)`: `|(mean_post_warmup(T) - mean_post_warmup(U)) / mean_post_warmup(U)| >= 0.10`.
   - `WP-2(b2)`: `|(mean_post_warmup(T) - mean_post_warmup(S)) / mean_post_warmup(T)| <= 0.10` with `0` `Deforest` events in `S`.
   - `WP-2(c)`: `detritus > 0` before tick `3,000` (sample `600`) and at tick `3,000` in all `3` arms.
4. **Locked schedule & Windows 1 budget:** `duration_ticks = 18000`, `sample_period = 600` (`30` samples), `warmup_ticks = 3000` (`5` warmup samples excluded), `tail_window = [14400, 18000]` (`7` samples). Estimated `~7.32 min` (12-thread benchmark) to `~16.0 min` (sequential bound) within the `90 min` ceiling.

---

## 5. Artifact Paths & Independent Verification Contract

- **Future execution artifact roots (empty until runner/confirmation assignment):**
  - Smoke: `artifacts/experiments/e3b-productivity-gradient-body-mass/smoke/` (`0` `trait_mass` substrings allowed anywhere).
  - Confirmation: `artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation/`.
- **Verification commands:**
  - `cargo test --manifest-path src-tauri/Cargo.toml --features desktop --test prereg_e3b_manifest_tests`
  - `node scripts/verify_e3b_artifacts.mjs --self-test`
  - `npm run check:e3b` (returns pre-run ok until `confirmation/` exists; verifies full confirmation bundle once executed).
