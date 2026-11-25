---
phase: planning
feature: experiment-sel2-food-scarcity
title: Preregistration (Frozen) — SEL2-P2: Ecological Selection vs Neutral Demographic Drift Under Food Scarcity
description: Frozen SEL2-P2 preregistration pinned to trunk f195a361ec50473932520ad30306bd366d719a6d with locked QD-SEL2P2-D1=(a)..D7=(a) (D4=(b)) decisions, blind 200k VIAB-2 pre-check passed (12/12 survival, 0 trait_mass), 31-key LIVE_KEYS partition (23 explicit + 8 default-absent), 48 paired confirmation seeds (900001..=947001), 12 baseline seeds (970501..=970512), blind smoke seed 999931, log mean ratio D = ln(mean_int_B/mean_int_A) with SESOI +-ln(1.10), Holm m=3, TOST, D6(a) R3 complete-pair/Lachin rule, and closed 3-label R10 decision table.
status: active
owner: anima-case
last_reviewed: 2026-11-24
---

# Preregistration (Frozen) — `SEL2-P2`: Ecological Selection vs. Neutral Demographic Drift Under Food Scarcity

> **FROZEN PREREGISTRATION (`status: active` per [`DOCUMENTATION_POLICY.md`](../../governance/DOCUMENTATION_POLICY.md), matching [Frozen E3b](2026-11-24-e3b-preregistration.md); JSON contract carries `"status": "FROZEN"`).**
> - **Pinned trunk commit:** `f195a361ec50473932520ad30306bd366d719a6d` (`f195a36`, containing `PR #247` draft, `PR #258` `SEL2P2-VIAB2`, `PR #256` `ADR-0014`, and `PR #249` `WV2-4`). Zero decisions changed from reviewed draft [2026-11-24-sel2-p2-preregistration-draft.md](2026-11-24-sel2-p2-preregistration-draft.md).
> - **Confirmation & smoke seeds executed at freeze:** `0` (`contains_results: false`). Neither confirmation (`900001..=947001`), baseline (`970501..=970512`), nor smoke (`999931`) seeds have been executed during freeze.
> - **Body-mass blindness (`QD-VIAB`):** `0` `VIAB-1`/`VIAB-1b`/`VIAB-2`, baseline, smoke, or confirmation `live.trait_mass_*` values have been consulted or read prior to confirmation. Blind smoke and `baseline_ref` manifests omit all `trait_mass` observables structurally (`0` substrings in JSON).
> - **Machine-readable contract & gates:** [`sel2p2-preregistration.json`](../../../src-tauri/tests/fixtures/experiments_sel2p2/sel2p2-preregistration.json) + `7` arm/baseline/smoke manifests under `src-tauri/tests/fixtures/experiments_sel2p2/`, verified by [`prereg_sel2p2_manifest_tests.rs`](../../../src-tauri/tests/prereg_sel2p2_manifest_tests.rs) and [`verify_sel2p2_artifacts.mjs`](../../../scripts/verify_sel2p2_artifacts.mjs) (`npm run check:sel2p2`).

---

## 0. Provenance & Locked Owner Decisions (`QD-SEL2P2-D1`..`D7`)

| Source / Artifact | Reference / PR / Commit | Role in `SEL2-P2` Freeze | Body-Mass Values Consulted |
|---|---|---|---|
| **Governing decisions (`QD-VIAB`, `QD-SEL2P2`, `QD-SEL2g`, `QD-BIO-D2`)** | [AGENT_WORKFLOW.md §4](../../how-to/AGENT_WORKFLOW.md) & `MSG_20261124_080000` | Locks blind survival calibration, `SEL2-P` smoke disclosure, `QD-SEL2g`, and freeze-time key/observable pinning | **None** |
| **Locked owner decisions (`D1=(a)..D7=(a)`, `D4=(b)`)** | [2026-11-24-sel2-p2-preregistration-draft.md](2026-11-24-sel2-p2-preregistration-draft.md) (`PR #247` -> `f976e66d`, updated `fce7b981`) | Fixes founder genotype (`1`), scarcity (`0.75`), `m=3` Holm, `ln` ratio `SESOI = +-ln(1.10)`, `N=48`, `D6(a)` `R3`, `12` baseline seeds | **None** |
| **Blind 200k pre-check (`VIAB-2`, `C5b`)** | [2026-11-24-sel2-p2-viab2-results.md](2026-11-24-sel2-p2-viab2-results.md) (`PR #258` -> `fce7b981`) | Confirms `12/12` survival across all 4 arms over `200,000` ticks (`880001..=880012`, `min_N >= 59`, `12/12` `drift_valid`); `D2(b)` not triggered | **None (`0` `trait_mass` matches)** |
| **Historical calibration (`VIAB-1` & `VIAB-1b`)** | [VIAB-1](2026-11-23-viab1-results.md) (`PR #215`, `f15b2c18`) & [VIAB-1b](2026-11-23-viab1b-results.md) (`PR #228`, `5721f1fb`) | Documents `3`-segment `mut0` collapse (`PR #216`: probe seeds `880001..=880006` only) and `2`-segment `12/12` rescue (`PR #224` -> `98054ba2`) | Disclosed probe `3.3 -> ~2.5` only (`VIAB-1`/`1b` `0` `trait_mass`) |
| **Prior `SEL2-P` retired draft & smoke stop** | [SEL2-P Draft](2026-11-19-sel2-p-preregistration-draft.md), [Smoke 999961](2026-11-19-sel2-p-smoke.md) | Discloses permanent smoke stop (`seed = 999961`, extinction at `4,400..5,050` ticks from soft `prey_cap = 320`, `F-052`; `0` confirm runs) | **None** (`contains_body_mass = false`) |
| **Statistical rules template (`#214`)** | [Prereg Template](2026-11-23-prereg-statistics-template.md) & [VIABILITY_PRICE §3.3–3.5](../../research/VIABILITY_PRICE_STATISTICS_RESEARCH_2026-11-22.md) | Supplies rules `R1`–`R10`, `D6(a)` `5%` extinction / Lachin switch, TOST, Lee bounds, and closed `R10` table | None |

### Locked `QD-SEL2P2` Decision Register (`MSG_20261124_080000`)

| Decision ID | Locked Choice | Summary |
|---|---|---|
| **`QD-SEL2P2-D1`** | **Option `(a)`** | `live.founder_genotype = 1.0` across all arms + mandatory `200k` blind `VIAB-2` pre-check **PASSED** (`12/12` survival in all 4 arms); `mutation.rs:209-231` `nodes.len() > 2` operator-4 wall disclosed. |
| **`QD-SEL2P2-D2`** | **Option `(a)`** | `live.resource_capacity_scale = 0.75` (`1.0` in `baseline_ref`), `live.founder_layout = 0.0`, `live.drift_reset_ticks = 2000.0`; `D2(b)` (`fl=2`) fallback not triggered. |
| **`QD-SEL2P2-D3`** | **Option `(a)`** | Confirmatory family size **`m = 3`** (`H1 selected/scarce_drift`, `H2 selected/scarce_mut0`, `H3 scarce_drift/scarce_mut0`), Holm–Bonferroni FWER `alpha = 0.05` (`0.016667`, `0.025`, `0.05`). |
| **`QD-SEL2P2-D4`** | **Option `(b)`** | Primary endpoint log mean ratio **`D = ln(mean_int_B / mean_int_A)`** over `(20_000, 200_000]`, **`SESOI = +-ln(1.10) = +-0.09531018`** (`[0.909x, 1.100x]`), `90%` TOST CI. |
| **`QD-SEL2P2-D5`** | **Option `(a)`** | **`N = 48`** paired confirmation seeds **`900001 + 1000 * k` (`k = 0..47`, `900001..=947001`, `144` runs)** within cap `N <= 70` (`<= 969001`), executed on `Windows 1`. |
| **`QD-SEL2P2-D6`** | **Option `(a)`** | Aligned strictly with `#214` Template `R3`: complete pairs primary iff `p_ext <= 5%` AND `pi_LL >= 0.85` (with Lee bounds + `RMST(200k)`); else Lachin worst-rank across all `N = 48` + `RMST(200k)` + SACE; require `drift_valid == true` and `has_shortfall == false` per contrast. |
| **`QD-SEL2P2-D7`** | **Option `(a)`** | `QD-SEL2g` pooled baseline of **`12` seeds `970501..=970512`** (`min_surviving_baseline_seeds = 6`); smoke `C1 >= 5`, `C2 in [0.4, 0.6]`, `C3 >= 30`, `C4 > 0.50`, `C5 > 0`. |

---

## 1. World Configuration, `31` `LIVE_KEYS` Partition & Calibration Basis

### 1.1. Exact `31` `LIVE_KEYS` Partition on Trunk `f195a361`

`SEL2-P2` runs via [`LiveExperimentAdapter`](../../../src-tauri/src/core/live_experiment.rs) (`LIVE_MODEL_VERSION = "live-bevy-world/2"`, `SCHEMA_VERSION = 8`, `WorldLawSet::baseline()`, `exotic_energy: None`, `ObserverPolicy::Absent`) on `Generated` terrain (`MapSettings::default()`, world `seed = 1337`, `256x256`; never pinned across OS/architectures). Each manifest explicitly declares `23` keys and leaves `8` engine-default keys absent in `default_keys_left_absent` (`23 + 8 = 31` freeze-time `LIVE_KEYS` pinned under `QD-BIO-D2` subset semantics):

- **Shared constants across all scarce arms & smoke (`20` keys; `baseline_ref` overrides `prey_cap = 80.0` and `resource_capacity_scale = 1.0`):**
  1. `live.ecological_fitness = 1.0`
  2. `live.mortality = 1.0`
  3. `live.reproduction = 1.0`
  4. `live.locomotion = 1.0`
  5. `live.drive_floor = 0.5`
  6. `live.thirst_steering = 1.0`
  7. `live.evolved_brains = 0.0`
  8. `live.founders = 100.0`
  9. `live.predator_fraction = 0.0` *(single herbivore trophic level; no predators)*
  10. `live.trees = 8.0`
  11. `live.lakes = 16.0`
  12. `live.lake_layout = 1.0`
  13. `live.plant_reserve = 0.0`
  14. `live.food_cap = 50.0`
  15. `live.dehydration_ticks = 960.0`
  16. `live.prey_cap = 320.0` (`80.0` in `baseline_ref`)
  17. `live.strict_prey_cap = 1.0` *(`F-052` fix eliminating boom-crash)*
  18. `live.founder_genotype = 1.0` *(`QD-SEL2P2-D1=(a)` 2-segment founder)*
  19. `live.founder_layout = 0.0` *(`QD-SEL2P2-D2=(a)` legacy line layout)*
  20. `live.resource_capacity_scale = 0.75` (`1.0` in `baseline_ref`, `QD-SEL2P2-D2=(a)`)
- **Per-arm factor keys (`3` keys):**
  21. `live.mutation_rate`: `0.05` (`selected`, `scarce_drift`, `baseline_ref`) vs. `0.0` (`scarce_mut0`)
  22. `live.drift_control`: `0.0` (`selected`, `scarce_mut0`, `baseline_ref`) vs. `1.0` (`scarce_drift`)
  23. `live.drift_reset_ticks`: `0.0` (`selected`, `scarce_mut0`, `baseline_ref`) vs. `2000.0` (`scarce_drift`)
- **Locked absent engine-default keys (`8` keys in `default_keys_left_absent`, verified against `LiveWorldConfig::default()`):**
  `live.w_surplus = 1.0`, `live.starve_ticks = 120`, `live.kill_hydration = 0.0`, `live.kill_efficiency = 0.4`, `live.target_min_energy = 0.0`, `live.land_mask = 0`, `live.grass_steering = 0`, `live.lineage_tracking = 0`.
- **2-segment founder (`live.founder_genotype = 1.0`) & operator-4 wall ([`mutation.rs#L209-L231`](../../../src-tauri/src/evolution/mutation.rs#L209-L231)):**
  Founder genotype comprises `nodes = [1.5, 1.0]` (`total_mass = 2.5`, canonical `serde_json` SHA-256 `dfeb276205f818acf3f34f5d716cbf163598a03a23ffc6964ff85753e280c719`). Because `mutate_genotype` operator `4` requires `genotype.nodes.len() > 2`, structural mutations can add segments (`>= 3`) or adjust node sizes/masses continuously in either direction, never prune down to `1` node.

### 1.2. Blind `VIAB-2` Pre-Check Summary (`PR #258`, Seeds `880001..=880012`)

In [2026-11-24-sel2-p2-viab2-results.md](2026-11-24-sel2-p2-viab2-results.md) (`viab2_fg1_s075_k2k_fl0`, `T = 200,000` ticks, `0` `trait_mass`), all `48` runs (`12` seeds × `4` arms) survived `100%` (`12/12` in `baseline_ref`, `selected`, `scarce_mut0`, and `scarce_drift`): `min_N >= 59`, `C2` passed `11/12` in `selected` and `12/12` in `scarce_mut0`, `C3/C4/C5` passed `12/12`, `drift_valid = 12/12` (`0` shortfalls), and `max_abs_eu_drift <= 8.559437e-8 EU`.

---

## 2. Experimental Arms, Primary Endpoint `D` & Confirmatory Family (`m = 3`)

### 2.1. Matched Manifest Arms & `QD-SEL2g` Baseline

| Manifest File | Role | `sample_period` | Explicit Key Differences | Declared Observables |
|---|---|---|---|---|
| `sel2p2-selected.json` | Natural selection (`N=48`) | `500` | `scale=0.75`, `mut=0.05`, `drift=0`, `reset=0` | `26` drift-compatible (`incl. trait_mass_*`) |
| `sel2p2-scarce-mut0.json` | Clonal zero-mutation (`N=48`) | `500` | `scale=0.75`, **`mut=0.0`**, `drift=0`, `reset=0` | `26` drift-compatible (`incl. trait_mass_*`) |
| `sel2p2-scarce-drift.json` | Neutral drift control (`N=48`) | `500` | `scale=0.75`, `mut=0.05`, **`drift=1`**, **`reset=2000`** | `26` drift-compatible *(no cause-split)* |
| `sel2p2-baseline-ref.json` | `QD-SEL2g` pooled baseline (`12` seeds) | `50` | **`prey_cap=80`**, **`scale=1.0`**, `mut=0.05`, `drift=0` | **`9` blind non-mass (`0` `trait_mass`)** |
| `sel2p2-smoke-selected.json` | Blind smoke (`999931`) | `50` | Same IC as `sel2p2-selected.json` | **`9` blind non-mass (`0` `trait_mass`)** |
| `sel2p2-smoke-scarce-mut0.json` | Blind smoke (`999931`) | `50` | Same IC as `sel2p2-scarce-mut0.json` | **`9` blind non-mass (`0` `trait_mass`)** |
| `sel2p2-smoke-scarce-drift.json` | Blind smoke (`999931`) | `50` | Same IC as `sel2p2-scarce-drift.json` | **`7` blind non-mass (`0` `trait_mass`, no cause-split)** |

### 2.2. Locked Primary Endpoint (`QD-SEL2P2-D4=(b)`) & Confirmatory Hypotheses (`D3=(a)`)

For each run over post-warmup samples $t_m \in (20\,000, 200\,000]$ (`sample_period = 500`, `360` samples):

$$\overline{z}_{A,i}^{\text{int}} = \frac{1}{360}\sum_{m=1}^{360} \text{live.trait\_mass\_mean}(t_m), \qquad D_{B/A,i} = \ln\!\left(\frac{\overline{z}_{B,i}^{\text{int}}}{\overline{z}_{A,i}^{\text{int}}}\right)$$

- **Confirmatory family (`m = 3`, Holm–Bonferroni FWER `alpha = 0.05` -> thresholds `0.016667`, `0.025`, `0.05`):**
  1. **`H1` (`selected / scarce_drift`, PRIMARY, two-sided):** Isolates genuine ecological selection net of demographic drift and mutation bias at matched `mutation_rate = 0.05`.
  2. **`H2` (`selected / scarce_mut0`, confirmatory under `m = 3`, two-sided):** Total evolutionary shift vs. clonal founder (`mass = 2.5`).
  3. **`H3` (`scarce_drift / scarce_mut0`, confirmatory under `m = 3`, two-sided):** Pure mutational transmission bias under `nodes.len() >= 2`.
- **SESOI & TOST equivalence:** **$\text{SESOI} = \pm\ln(1.10) \approx \pm 0.09531018$** (`[0.909x, 1.100x]`). Paired `90%` $t$-CI strictly inside `(-ln(1.10), +ln(1.10))` (`tost_alpha = 0.05` per tail).
- **Exploratory & integrity checks (`decision_effect: none`, BH-FDR `q = 0.05`):** `R1_tail20` (`tick > 160_000`), `R2_window_net_selection_delta`, `S1` (`live.trait_mass_var`), `S2` post-warmup `live.mean_energy_integral`, `S3` (`live.births`, `live.deaths`, `live.mean_lifespan` read strictly from `series.csv` never `final_observables` per `F-053`, `live.cumulative_mean_lifespan`), `RMST(tau = 200_000)`, Lee (2009) trimming bounds, and closed-EU ledger check (`max_abs_eu_drift <= 1.0e-6 EU`).

---

## 3. Pre-Registered Rules `R1`–`R10`, Explicit Disjoint Seed Schedule & Decision Table

| Rule | Locked Specification |
|---|---|
| **`R1` Unit & Disjoint Seeds** | Paired CRN on `Windows 1`; **`N = 48`** confirmation pairs (`seed_k = 900001 + 1000 * k`, `k = 0..47` -> `900001..=947001`, cap `N <= 70` `<= 969001 < 970501`); **`12`** baseline seeds **`970501..=970512`** (`min_surviving = 6`); smoke **`999931`**. |
| **`R2` Multiplicity (`D3(a)`)** | `m = 3` confirmatory `{H1, H2, H3}` Holm–Bonferroni `alpha = 0.05` (`0.016667`, `0.025`, `0.05`); secondary checks BH-FDR `q = 0.05` (`decision_effect: none`). |
| **`R3` Attrition (`D6(a)`)** | Complete pairs primary **iff** both arms have extinction rate `p_ext <= 5%` AND `pi_LL >= 0.85` (`n_LL >= 41`), with Lee (2009) bounds + `RMST(200k)`; otherwise switch primary to **Lachin worst-rank** across all `N = 48` seeds + `RMST(200k)` + SACE. Always require `drift_valid == true` and `has_shortfall == false` per contrast. |
| **`R4` SESOI & MDE (`D4(b)`)** | $\text{SESOI} = \pm\ln(1.10) = \pm 0.09531018$ on $D$; report achieved MDE `0.4818 * sd_D` (`80%` Holm-1 at `N=48`) and `0.5 * sd_D`. |
| **`R5` Blind Nuisance** | Strictly from `VIAB-2` (`0` `trait_mass`): `12/12` survival in all 4 arms, `min_N >= 59`, `mean_N` med `136.65` (`selected`), `gen` med `32.92`, `drift_valid = 12/12`. |
| **`R6` Power (`N = 48`)** | At `pi_LL = 11/12`, `P(n_LL >= 41) = 95.65%`; `80%` Holm `m=3` power down to `d_z = 0.4818`, `97.92%` at `d_z = 0.66`. |
| **`R7` Signed-Rank Floor** | Exact Wilcoxon at `alpha/3 = 0.016667` requires `n_non_zero >= 7` (`2/2^7 = 0.015625`); at `N = 48`, `p_min = 7.11e-15`. |
| **`R8` Effect-Size Bundle** | Mean $D$, `exp(D)` ratio, raw mean shift, $d_z$, Kerby $r_c$ (`0.5205` / `0.6641` / `0.7421`), HL pseudomedian + `10,000`-resample BCa 95% CI, Lee bounds, `RMST(200k)`. |
| **`R9` TOST Equivalence** | `90%` paired $t$-CI strictly inside `(-ln(1.10), +ln(1.10))`. |
| **`R10` Closed 3-Label Set** | Strictly `{supports, equivalent_to_zero, insufficient_evidence}` (`direction_tag` marks positive/negative); see §3.2. |

### 3.1. Locked Seed Schedule (`N = 48` Pairs + `12` Baseline + Smoke + Pipeline)

- **Confirmation pairs (`N = 48`, `seed_k = 900001 + 1000 * k`, `k = 0..47`; cap `k <= 69` <= `969001 < 970501`):**

| `k = 0..11` | `k = 12..23` | `k = 24..35` | `k = 36..47` |
|---|---|---|---|
| `0`: `900001` | `12`: `912001` | `24`: `924001` | `36`: `936001` |
| `1`: `901001` | `13`: `913001` | `25`: `925001` | `37`: `937001` |
| `2`: `902001` | `14`: `914001` | `26`: `926001` | `38`: `938001` |
| `3`: `903001` | `15`: `915001` | `27`: `927001` | `39`: `939001` |
| `4`: `904001` | `16`: `916001` | `28`: `928001` | `40`: `940001` |
| `5`: `905001` | `17`: `917001` | `29`: `929001` | `41`: `941001` |
| `6`: `906001` | `18`: `918001` | `30`: `930001` | `42`: `942001` |
| `7`: `907001` | `19`: `919001` | `31`: `931001` | `43`: `943001` |
| `8`: `908001` | `20`: `920001` | `32`: `932001` | `44`: `944001` |
| `9`: `909001` | `21`: `921001` | `33`: `933001` | `45`: `945001` |
| `10`: `910001` | `22`: `922001` | `34`: `934001` | `46`: `946001` |
| `11`: `911001` | `23`: `923001` | `35`: `935001` | `47`: `947001` |

- **`QD-SEL2g` pooled baseline (`12` seeds, `min_surviving_baseline_seeds = 6`):**
  `970501`, `970502`, `970503`, `970504`, `970505`, `970506`, `970507`, `970508`, `970509`, `970510`, `970511`, `970512`.
- **Blind smoke seed:** `999931` (excluded mechanically from confirmation manifests and confirmation analysis).
- **Pipeline-check seeds (`2,000`-tick wiring verification only):** `999701..=999706` (baseline), `999711` (smoke), `999721`, `999722` (pairs).
- **Unit test seeds:** `614001`, `614002`, `614999`.
- **Disjointness guarantee:** Zero overlap with `1..=6`, `613001/613002/613999`, `E2` (`700001..=711001`, `999983`), `E3` (`730001..=741001`, `999979`), `E2R` (`750001..=761001`), `E-SEL1` (`760001..=771001`, `999971`), `SEL2-P` (`780001..=799506`, `999961`, `999801..=999822`), `E3b` (`810001..=879001`, `999953`), and `VIAB-1`/`VIAB-1b`/`VIAB-2` (`880001..=880012`).

### 3.2. Closed 3-Label `R10` Decision Table

| Smoke `C1..C5` & Host | Attrition, `drift_valid` & `has_shortfall == false` | `p_Holm < 0.05` | Effect Magnitude vs. `SESOI` / SACE | TOST 90% CI Inside `(-SESOI, +SESOI)` | `R10` Label | `direction_tag` |
|---|---|---|---|---|---|---|
| Fail | Any | Any | Any | Any | **`insufficient_evidence`** | `harness_invalid` |
| Pass | Fail | Any | Any | Any | **`insufficient_evidence`** | `attrition_below_floor_or_drift_invalid` |
| Pass | Pass (`complete_pairs`) | Yes | `\|D\| >= SESOI` | No | **`supports`** | `two_sided_positive` / `two_sided_negative` |
| Pass | Pass (`lachin_worst_rank`) | Yes | SACE interval outside `(-SESOI, +SESOI)` matching rank sign | N/A | **`supports`** | `lachin_two_sided_positive` / `lachin_two_sided_negative` |
| Pass | Pass (`lachin_worst_rank`) | Yes | Otherwise | N/A | **`insufficient_evidence`** | `attrition_sace_straddles_or_inconclusive` |
| Pass | Pass (`complete_pairs`) | Yes | `\|D\| < SESOI` | Yes | **`equivalent_to_zero`** | `trivial_below_sesoi_equivalent` |
| Pass | Pass (`complete_pairs`) | Yes | `\|D\| < SESOI` | No | **`insufficient_evidence`** | `inconclusive_straddles_sesoi` |
| Pass | Pass (`complete_pairs`) | No | `\|D\| < SESOI` | Yes | **`equivalent_to_zero`** | `equivalent_to_zero_powered_null` |
| Pass | Pass | No | Any | No | **`insufficient_evidence`** | `inconclusive_wide_ci` |
| Pass | Pass (`complete_pairs`) | Any | `\|D\| >= SESOI` | Yes *(guard)* | **`insufficient_evidence`** | `ci_inconsistent_impossible` |

---

## 4. Execution Contract & Scope Boundaries

1. **Deterministic single host:** Future runner (`run_sel2p2_experiment`, `--release --features desktop`) runs sequentially or deterministically via external `--checkpoint-dir` outside `--out` on `Windows 1`, with `ANIMA_WORLD_ARTIFACT` unset, `<temp_dir>/anima_world_artifact.anmw` absent, `ANIMA_USE_GPU=0`, `terrain_source == "generated"`, uniform `world_identity`, and `max_abs_eu_drift <= 1.0e-6 EU`.
2. **Pre-smoke `QD-SEL2g` + blind smoke `C1..C5` hard stop:** Must run `sel2p2-baseline-ref.json` (`12` seeds, `>= 6` surviving) and blind smoke `seed = 999931` (`0` `trait_mass` substrings anywhere under smoke output) and clear `C1..C5` + `drift_valid` before any confirmation seed executes.
3. **What `SEL2-P2` will NOT show:** (a) Headless adapter verified only — not desktop GUI, GPU, or multi-threaded runtime scheduling; (b) single herbivore trophic level without neural evolution (`evolved_brains = 0`, `predator_fraction = 0`); (c) no genotype-to-phenotype plasticity separation; (d) `mutation.rs:209-231` structural deletion wall (`nodes.len() > 2`) restricts structural pruning below `2` nodes; (e) binds only to this `Generated` `256x256` parameter slice (not World v2 before `WV2-7`).

---

## 5. Future Artifact Paths & Freeze Verification Commands

- **Future execution artifact paths (not yet populated at freeze):**
  - Blind smoke: `` `artifacts/experiments/sel2-p2/smoke/` `` (`0` `trait_mass` substrings allowed)
  - Confirmation: `` `artifacts/experiments/sel2-p2/confirmation/` ``
- **Verification commands:**
  - `cargo test --manifest-path src-tauri/Cargo.toml --features desktop --test prereg_sel2p2_manifest_tests`
  - `node scripts/verify_sel2p2_artifacts.mjs --self-test`
  - `npm run check:sel2p2`
