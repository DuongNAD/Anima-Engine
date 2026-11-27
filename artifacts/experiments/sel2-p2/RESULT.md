# SEL2-P2 Result — Does Ecological Selection Shift Genotypic Body Mass Under Food Scarcity?

**Executed once on 2026-11-26 under the preregistration frozen at commit `81eb1e8e` (`PR #290`, pinned to trunk parent `f195a361ec50473932520ad30306bd366d719a6d`), with confirmation re-emitted once from the 48 checkpoints on 2026-11-27 under `QD-GATES-1127 (2)` (`PR #336`).**
- **Run commit (`win/sel2p2-runner`, `PR #330`):** `247abf42c8fffaf96a756342a60e1b5c2cab49cb`
- **Runner binary (`run_sel2p2_experiment.exe`, `--release --features desktop`):**
  - **Invocation 1 (`--run-all`, 2026-11-26):** before `5217be55851b1389afcb6d4078df6f1630c3ac335095cccb8cc5b39cfc661186` -> after `5217be55851b1389afcb6d4078df6f1630c3ac335095cccb8cc5b39cfc661186`
  - **Invocation 2 (`--confirm` from 48 checkpoints, no rebuild, 2026-11-27):** before `5217be55851b1389afcb6d4078df6f1630c3ac335095cccb8cc5b39cfc661186` -> after `5217be55851b1389afcb6d4078df6f1630c3ac335095cccb8cc5b39cfc661186`
- All `15` smoke/baseline runs (`12` `baseline_ref` + `3` blind smoke at seed `999931`) and `144` confirmation runs (`48` paired seeds `900001..=947001` × `3` arms) completed on `Windows 1` (`windows/x86_64`) with `0` failures and `0` extinctions. Body-mass values remained unexamined during blind smoke (`body_mass_consulted: false`, `0` `trait_mass` substrings in `smoke/`) and throughout simulation execution.

> **Model status, stated the only way it may be stated: headless adapter verified.** No desktop app,
> Tauri handle, window, GPU device, renderer, learner thread, or evolution thread existed in the process that
> produced these numbers. Nothing here may be quoted as `"the live world is experiment-ready"`.
>
> **Experimental setting (frozen [`2026-11-24-sel2-p2-preregistration.md`](../../../docs/ai/planning/2026-11-24-sel2-p2-preregistration.md)):** Single herbivore trophic level (`live.predator_fraction = 0.0`, `live.evolved_brains = 0.0`), adapted 2-segment clonal founder (`live.founder_genotype = 1.0`, `nodes = [1.5, 1.0]`, initial `total_mass = 2.5` at the [`mutation.rs#L209-L231`](../../../src-tauri/src/evolution/mutation.rs#L209-L231) `nodes.len() > 2` operator-4 deletion boundary), strict prey cap (`live.prey_cap = 320.0`, `live.strict_prey_cap = 1.0`), viable scarcity (`live.resource_capacity_scale = 0.75`, `live.founder_layout = 0.0`), and lockstep demographic+mutation drift control (`live.drift_control = 1.0`, `live.drift_reset_ticks = 2000.0`) over `200,000` ticks (`Generated` `256x256` terrain, `world_identity` seed `1337`, checksum `3152323152`).

---

## 1. Executive Decision (Frozen `R1`–`R10` Rules)

**Bundle integrity verifier commands (exit `0` with `0` `NOTE:` fallbacks on the committed confirmation bundle):**

```bash
node scripts/verify_sel2p2_artifacts.mjs
node scripts/verify_sel2p2_artifacts.mjs --self-test
```

| Contrast (`B / A`) | Hypothesis (`m = 3`, two-sided) | `n_LL / 48` ($\hat{\pi}_{LL}$) | $\bar{D}$ ($\ln(\text{mean}_B/\text{mean}_A)$) | Geom. Ratio $e^{\bar{D}}$ | 95% $t$-CI on $\bar{D}$ | 90% TOST CI vs $\pm\ln(1.10)$ (`±0.095310`) | $p_{\text{two-sided}}$ | $p_{\text{Holm}}$ ($m = 3$) | TOST Equiv. | Preregistered `R10` Label (`direction_tag`) |
|---|---|---:|---:|---:|---|---|---:|---:|---|---|
| **`H1` (`selected / scarce_drift`, PRIMARY)** | Causal ecological selection net of drift & mutation bias | `48 / 48` (`1.0`) | **`-0.002971`** | `0.997034x` | `[-0.003280, -0.002661]` | `[-0.003229, -0.002713]` | `5.285e-24` | **`1.585e-23`** | **`true`** | **`equivalent_to_zero`** (`trivial_below_sesoi_equivalent`) |
| **`H2` (`selected / scarce_mut0`)** | Total evolutionary shift vs. clonal `2.5` founder | `48 / 48` (`1.0`) | **`-0.062145`** | `0.939747x` | `[-0.082151, -0.042138]` | `[-0.078831, -0.045458]` | `1.129e-7` | **`2.259e-7`** | **`true`** | **`equivalent_to_zero`** (`trivial_below_sesoi_equivalent`) |
| **`H3` (`scarce_drift / scarce_mut0`)** | Pure mutational transmission bias under `nodes.len() >= 2` | `48 / 48` (`1.0`) | **`-0.059174`** | `0.942543x` | `[-0.078935, -0.039413]` | `[-0.075656, -0.042692]` | `2.474e-7` | **`2.474e-7`** | **`true`** | **`equivalent_to_zero`** (`trivial_below_sesoi_equivalent`) |

*Reproduced directly from committed [`confirmation/effects.json`](confirmation/effects.json) by [`scripts/verify_sel2p2_artifacts.mjs`](../../../scripts/verify_sel2p2_artifacts.mjs):*

```bash
node --input-type=module -e "import fs from 'node:fs'; import { evaluateSel2p2Dataset, loadCommittedPrereg, parseCsv } from './scripts/verify_sel2p2_artifacts.mjs'; const d='artifacts/experiments/sel2-p2/confirmation'; const r=evaluateSel2p2Dataset({ runs:parseCsv(fs.readFileSync(d+'/runs.csv','utf8')), series:parseCsv(fs.readFileSync(d+'/series.csv','utf8')), finalObservables:JSON.parse(fs.readFileSync(d+'/final-observables.json','utf8')), prereg:loadCommittedPrereg(), smokeAndHostValid:true }); console.log(r.primary_decision, r.primary_direction_tag, r.primary_mode); for (const c of ['H1','H2','H3']) { const x=r.contrasts[c], p=x.complete_pairs; console.log(c, x.decision, x.direction_tag, {n:p.n_complete_pairs, pi:p.pi_ll, mean_D:p.mean_D, geom:p.geom_mean_ratio, sd_D:p.sd_D, ci95:[p.ci95_low,p.ci95_high], ci90:[p.ci90_tost_low,p.ci90_tost_high], p2:p.p_two_sided, p_holm:x.p_holm, tost:p.tost_equivalent, dz:p.paired_dz, wP:p.wilcoxon.p_value, rc:p.wilcoxon.r_c}); }"
```

**Primary outcome summary:**
- **Causal selection effect (`H1`: `selected / scarce_drift`) is `equivalent_to_zero` (`trivial_below_sesoi_equivalent`):** Across all `48 / 48` complete pairs (`100%` survival, `drift_valid = true`, `has_shortfall = false`), `selected` has a statistically consistent but biologically negligible downward shift relative to `scarce_drift` (`mean_D = -0.002971`, `0.997034x` geometric mean ratio, `-0.006880` raw mass units; `48/48` negative pairs, `p_holm = 1.585e-23`), with its entire `90%` TOST CI `[-0.003229, -0.002713]` lying more than `29x` inside the preregistered `±ln(1.10) = ±0.095310` equivalence bounds (`tost_p1 = 0.0`, `tost_p2 = 0.0`).
- **Mutational drift vs. clonal founder (`H2` & `H3`) dominates the within-arm trajectory, while remaining `equivalent_to_zero` over `(20k, 200k]`:** Both `selected` (`2.354494`) and `scarce_drift` (`2.361374`) shift downward by `~6%` over `(20,000, 200,000]` relative to clonal `scarce_mut0` (`2.500000`), yielding `mean_D = -0.062145` (`0.939747x`) in `H2` and `-0.059174` (`0.942543x`) in `H3`. Their `90%` TOST CIs (`[-0.078831, -0.045458]` and `[-0.075656, -0.042692]`) lie strictly inside `(-0.095310, +0.095310)` (`tost_p1 = 0.000836` and `0.000301`), classifying both as `equivalent_to_zero` (`trivial_below_sesoi_equivalent`) on the preregistered `(20,000, 200,000]` endpoint. *(Exploratory `R1_tail20` over `(160k, 200k]` reaches `0.8655x` in `H2` and `0.8684x` in `H3`, while `H1` remains `0.9966x`; per `R2`, exploratory endpoints carry `decision_effect: none`.)*

---

## 2. Provenance, Binary SHA-256 (Both Invocations), Smoke Gate & Integrity Audits

| Field | Invocation 1 (`--run-all`, Smoke + Original Confirm) | Invocation 2 (`--confirm` Checkpoint Re-emission under `QD-GATES-1127 (2)`) |
|---|---|---|
| Frozen preregistration | `81eb1e8e` (`PR #290`), pinned trunk `f195a361ec50473932520ad30306bd366d719a6d` | Same ([`2026-11-24-sel2-p2-preregistration.md`](../../../docs/ai/planning/2026-11-24-sel2-p2-preregistration.md)) |
| Run commit & PR | `247abf42c8fffaf96a756342a60e1b5c2cab49cb` (`win/sel2p2-runner`, `PR #330`) | Same (`247abf42c8fffaf96a756342a60e1b5c2cab49cb`, no rebuild) |
| Runner binary SHA-256 **before** | `5217be55851b1389afcb6d4078df6f1630c3ac335095cccb8cc5b39cfc661186` | `5217be55851b1389afcb6d4078df6f1630c3ac335095cccb8cc5b39cfc661186` |
| Runner binary SHA-256 **after** | `5217be55851b1389afcb6d4078df6f1630c3ac335095cccb8cc5b39cfc661186` | `5217be55851b1389afcb6d4078df6f1630c3ac335095cccb8cc5b39cfc661186` |
| Host, compiler, profile | `windows` / `x86_64` (`Windows 1`), `rustc 1.98.1 (48a229cea 2026-09-01)`, `release` | Same (`--jobs 8`, `ANIMA_USE_GPU=0`) |
| World isolation | `ANIMA_WORLD_ARTIFACT` unset, `%TEMP%/anima_world_artifact.anmw` absent, `terrain_source = "generated"` | Same across all `144/144` runs |
| Observed `world_identity` | `seed = 1337`, `generator_version = 1`, `256x256`, `checksum = 3152323152` | Identical across all `144/144` runs (`world_identity_all_runs_identical = true`) |
| Model & registry | `LiveExperimentAdapter` (`live-bevy-world/2`), `ObservableRegistry::live_default` (`1935740381389641332`) | Same |
| Timing (UTC) & wall-clock | Baseline `18:13:14Z -> 18:40:29Z`; Smoke `18:40:29Z -> 18:50:34Z`; Confirm `18:50:35Z -> 19:56:37Z` (`3961.7731 s`; `144` runs: min `189.5521 s`, median `216.1682 s`, max `264.5762 s`, sum `30,993.9818 s`) | `2026-11-27T04:55:46Z -> 04:55:47Z` (`0.1027 s`; `0` seeds simulated, `48/48` loaded from `D:\teamwork_projects\sel2p2_ckpt_247abf42`) |
| Dirty paths in `provenance.json` | `smoke/`: `src-tauri/gen/schemas/{desktop,windows}-schema.json` (retained untouched); orig confirm also listed those two + `smoke/` + `confirmation/` | Committed `confirmation/`: strictly `["artifacts/experiments/sel2-p2/confirmation/", "artifacts/experiments/sel2-p2/smoke/"]` (`git_describe: "247abf4"`) |

### `QD-SEL2g` Baseline & Blind Smoke Gate ([`smoke/smoke-report.json`](smoke/smoke-report.json), [`smoke/provenance.json`](smoke/provenance.json))
- **Pre-smoke `QD-SEL2g` pooled baseline (`970501..=970512`):** `12 / 12` surviving (min required `6`), `pooled_crop_second_half = 125360.321635`, `sufficient = true`.
- **Blind smoke (`seed = 999931`, `body_mass_consulted = false`, `0` `trait_mass` matches in `smoke/`):**
  - `c1_min_agents = true` (`selected` `110`, `scarce_mut0` `87`, threshold `>= 5`)
  - `c2_g_band = true` (`g_selected = 0.451169`, `g_scarce_mut0 = 0.477859`, diagnostic `g_scarce_drift = 0.453620`, band `[0.4, 0.6]`)
  - `c3_generations = true` (`selected = 38.7039`, `scarce_mut0 = 39.0669`, threshold `>= 30`)
  - `c4_starvation_share = true` (`selected = 0.681990`, `scarce_mut0 = 0.700102`, threshold `> 0.50`)
  - `c5_last_quarter_active = true` (`selected` `1476/1482`, `scarce_mut0` `1356/1313` births/deaths)
  - `drift_valid = true` (`scheduled_births = actual_births = 6120`, `scheduled_deaths = actual_deaths = 6091`, `0` shortfalls) -> **`gate_pass = true`**.

### `I1` Closed-EU Conservation & Drift-Control Audit (`144` Confirmation Runs, Ceiling `<= 1.0e-6 EU`)

| Arm | `Completed` / `Failed` | `extinct` | `drift_valid` | `has_shortfall` | Max Abs EU Drift vs Initial (Min / Median / Max) | Runs `> 1e-6` |
|---|---:|---:|---:|---:|---|---:|
| `selected` | `48` / `0` | `0` | `48 / 48` | `0 / 48` | `4.6275e-8` / `6.3257e-8` / `8.3353e-8` | `0` |
| `scarce_mut0` | `48` / `0` | `0` | `48 / 48` | `0 / 48` | `4.3510e-8` / `6.3170e-8` / `7.9046e-8` | `0` |
| `scarce_drift` | `48` / `0` | `0` | `48 / 48` | `0 / 48` | `4.6595e-8` / `6.3228e-8` / `8.1956e-8` | `0` |
| **All arms (`144` runs)** | **`144` / `0`** | **`0`** | **`144 / 144`** | **`0 / 144`** | **`4.3510e-8` / `6.3170e-8` / `8.3353e-8`** | **`0`** |

*(Max total EU span across all sampled ticks + final state per run: `8.3528e-8 EU`, well under `1.0e-6 EU`. `s3_lifespan_source = "series.csv"` per `F-053`.)*

---

## 3. Confirmatory Statistical Analysis (`R1`–`R10`)

### 3.1. Detailed Primary Contrasts, Non-Parametric Tests, and `R8` Effect-Size Bundle

| Metric | `H1` (`selected / scarce_drift`, PRIMARY) | `H2` (`selected / scarce_mut0`) | `H3` (`scarce_drift / scarce_mut0`) |
|---|---:|---:|---:|
| Complete pairs `n_LL / 48` ($\hat{\pi}_{LL}$) | `48 / 48` (`1.000000`) | `48 / 48` (`1.000000`) | `48 / 48` (`1.000000`) |
| `mean_D` | `-0.002971` | `-0.062145` | `-0.059174` |
| Geometric mean ratio $\exp(\bar{D})$ | `0.997034` | `0.939747` | `0.942543` |
| Raw mean shift ($\bar{z}_B - \bar{z}_A$) [`95% CI`] | `-0.006880` `[-0.007483, -0.006278]` | `-0.145506` `[-0.188475, -0.102538]` | `-0.138626` `[-0.181199, -0.096053]` |
| `median_D` | `-0.002951` | `-0.044862` | `-0.042078` |
| `sd_D` / `se_D` | `0.001065` / `0.000154` | `0.068900` / `0.009945` | `0.068054` / `0.009823` |
| Paired `t` (`df = 47`) / `p_two_sided` | `-19.327174` / `5.285e-24` | `-6.248935` / `1.129e-7` | `-6.024157` / `2.474e-7` |
| Holm-adjusted `p_holm` ($m = 3$) | **`1.585e-23`** | **`2.259e-7`** | **`2.474e-7`** |
| 95% $t$-CI on $\bar{D}$ | `[-0.003280, -0.002661]` | `[-0.082151, -0.042138]` | `[-0.078935, -0.039413]` |
| 90% TOST $t$-CI on $\bar{D}$ | `[-0.003229, -0.002713]` | `[-0.078831, -0.045458]` | `[-0.075656, -0.042692]` |
| TOST `p1` ($-\text{SESOI}$) / `p2` ($+\text{SESOI}$) | `0.0` / `0.0` | `0.000836` / `0.0` | `0.000301` / `0.0` |
| TOST equivalent ($\pm 0.095310$) | **`true`** | **`true`** | **`true`** |
| Paired Cohen's `d_z` | `-2.789637` | `-0.901956` | `-0.869512` |
| Achieved MDE: `0.5 * sd_D` (`80%` Holm-1 `0.4818 * sd_D`) | `0.000532` (`0.000513`) | `0.034450` (`0.033196`) | `0.034027` (`0.032789`) |
| Wilcoxon signed-rank `W+` / `W-` | `0.0` / `1176.0` | `25.0` / `1151.0` | `39.0` / `1137.0` |
| Wilcoxon `p_value` (`R7` `p_min = 7.11e-15`, met) | `1.631e-9` (`true`) | `7.721e-9` (`true`) | `1.793e-8` (`true`) |
| Matched-pairs rank-biserial `r_c` | `-1.000000` | `-0.957483` | `-0.933673` |
| Concordance (`D_i > 0` / `D_i < 0`) | `0/48` (`0.0`) / `48/48` (`1.0`) | `4/48` (`0.0833`) / `44/48` (`0.9167`) | `6/48` (`0.1250`) / `42/48` (`0.8750`) |
| Hodges–Lehmann pseudomedian [`95% BCa CI`] | `-0.002935` `[-0.003211, -0.002639]` | `-0.055378` `[-0.069921, -0.040044]` | `-0.052730` `[-0.066854, -0.037144]` |
| Observed `D_i` range (`[min, max]`) | `[-0.006389, -0.001170]` | `[-0.419702, +0.010691]` | `[-0.413313, +0.012847]` |

- **Exact per-seed log additivity verified:** Across all `48` seeds, $\max_i |D_i(H2) - [D_i(H1) + D_i(H3)]| = 1.8388 \times 10^{-16}$ (`-0.062145 = -0.002971 + (-0.059174)`).

### 3.2. Attrition, Lee (2009) Trimming Bounds, Lachin Worst-Rank, and `RMST(tau = 200,000)` (`R3` / `D6(a)`)

Because `144 / 144` runs survived through `T = 200,000` (`p_ext = 0.0`, `pi_LL = 1.0`, `failed_runs_count = 0`), `complete_pairs` is the primary mode (`QD-SEL2P2-D6=(a)`), Lee trimming bounds collapse to zero width (`n_a = n_b = k = 48`), and `RMST` is identically `200,000.0` ticks across all arms.

| Contrast | `n_a / n_b / k` | Lee `[lee_lo, lee_hi]` (`width`) | Lachin `mean_rank_diff` (`p_two_sided`, `r_c`) | `RMST` Control / Treatment Mean (delta) |
|---|---:|---|---:|---:|
| **`H1` (`selected / scarce_drift`)** | `48 / 48 / 48` | `[-0.002971, -0.002971]` (`0.0`) | `-2.50` (`1.246e-9`, `-1.0000`) | `200000.0` / `200000.0` (`0.0`) |
| **`H2` (`selected / scarce_mut0`)** | `48 / 48 / 48` | `[-0.062145, -0.062145]` (`0.0`) | `-40.00` (`5.024e-9`, `-0.9694`) | `200000.0` / `200000.0` (`0.0`) |
| **`H3` (`scarce_drift / scarce_mut0`)** | `48 / 48 / 48` | `[-0.059174, -0.059174]` (`0.0`) | `-36.00` (`1.791e-8`, `-0.9337`) | `200000.0` / `200000.0` (`0.0`) |

---

## 4. Exploratory and Secondary Results (`decision_effect: none`)

> **Strict rule (`R2`):** None of the metrics below modify any confirmatory `H1`–`H3` `R10` decision. All `21` paired exploratory `p`-values are adjusted jointly via Benjamini–Hochberg (`q < 0.05`) in Table 4.2 and remain hypothesis-generating only.

### 4.1. Per-Arm Descriptives (`N = 48` seeds per arm)

| Quantity | `selected` Mean ± SD (Median) | `scarce_mut0` Mean ± SD (Median) | `scarce_drift` Mean ± SD (Median) |
|---|---:|---:|---:|
| Post-warmup `mean_int` `(20k, 200k]` | `2.3545 ± 0.1480` (`2.3903`) | `2.5000 ± 0.0000` (`2.5000`) | `2.3614 ± 0.1466` (`2.3970`) |
| `R1_tail20` `(160k, 200k]` `trait_mass_mean` | `2.1815 ± 0.2634` (`2.1998`) | `2.5000 ± 0.0000` (`2.5000`) | `2.1886 ± 0.2628` (`2.2048`) |
| Final `T = 200,000` `live.trait_mass_mean` | `2.1295 ± 0.2795` (`2.1307`) | `2.5000 ± 0.0000` (`2.5000`) | `2.1453 ± 0.2836` (`2.1490`) |
| `S1`: Final `live.trait_mass_var` | `0.042290 ± 0.026058` (`0.042294`) | `0.000000 ± 0.000000` (`0.000000`) | `0.054842 ± 0.035390` (`0.046048`) |
| Final `live.agent_count` (`mean_N` across ticks) | `150.54 ± 29.58` (`147.84`) | `134.31 ± 25.76` (`140.88`) | `150.54 ± 29.58` (`147.84`) |
| `S3`: `births_sum` / `deaths_sum` | `5565.83` / `5515.29` | `5376.94` / `5342.63` | `5565.83` / `5515.29` |
| `S3`: `live.mean_lifespan` at `T` (`series.csv`, `F-053`) | `5700.51 ± 2301.09` (`4978.82`) | `5431.48 ± 2510.32` (`5094.53`) | `5787.44 ± 1958.51` (`5570.43`) |
| `S3`: `live.cumulative_mean_lifespan` | `4902.40 ± 668.22` (`4695.86`) | `4843.11 ± 695.24` (`4560.86`) | `4905.38 ± 669.14` (`4700.49`) |
| `S2`: Post-warmup `energy_post_mean` | `28.1071 ± 1.0575` (`28.3567`) | `28.1106 ± 0.9933` (`28.3936`) | `28.1137 ± 1.0526` (`28.3659`) |

- **`R2` Window Net Selection Deltas in `selected` (`N = 48`):** Cumulative within-interval ecological selection differentials (`window_net_selection_deltas`, counterfactual lockstep replay vs. non-selective reset) are consistently negative for body mass (`live.trait_mass_mean`: `-1.0913 ± 0.3026`, `d_z = -3.61`, `p = 8.69e-29`; east `-1.0763`, west `-1.0685`; `p10` `-0.4723`; `p90` `-0.6923`; `trait_mass_var` `-0.9710 ± 0.2250`), indicating that ecological selection itself slightly reinforced downward mass selection (`-0.297%` realized `H1` log ratio) while demographic turnover and `drift_control` resets maintained identical `births_sum` (`5565.83`), `deaths_sum` (`5515.29`), and `final_agents` (`150.54`) between `selected` and `scarce_drift`.

### 4.2. Secondary and Exploratory Contrast Family with BH-FDR (`m = 21`, `q = 0.05`, `decision_effect: none`)

| Contrast | Endpoint (`decision_effect: none`) | `n` | Paired Est. / Delta | 95% CI | `d_z` | `p_two_sided` | BH `q` (`m = 21`) | BH `< 0.05` |
|---|---|---:|---:|---|---:|---:|---:|:--:|
| `H1` (`sel / drift`) | `R1_tail20_D` (`> 160k`) | `48` | `-0.003359` (`0.9966x`) | `[-0.003938, -0.002779]` | `-1.6830` | `1.794e-15` | `3.767e-14` | `true` |
| `H1` (`sel / drift`) | `S1_trait_mass_var` | `48` | `-0.012552` | `[-0.019765, -0.005339]` | `-0.5053` | `1.027e-3` | `2.157e-3` | `true` |
| `H1` (`sel / drift`) | `S2_energy_post_mean` | `48` | `-0.006535` | `[-0.011590, -0.001480]` | `-0.3754` | `1.239e-2` | `2.002e-2` | `true` |
| `H1` (`sel / drift`) | `S3_births_sum` | `48` | `0.0` | `[0.0, 0.0]` | — | `1.000` | `1.000` | `false` |
| `H1` (`sel / drift`) | `S3_deaths_sum` | `48` | `0.0` | `[0.0, 0.0]` | — | `1.000` | `1.000` | `false` |
| `H1` (`sel / drift`) | `S3_cumulative_mean_lifespan` | `48` | `-2.9808` | `[-6.8581, +0.8965]` | `-0.2232` | `1.287e-1` | `1.930e-1` | `false` |
| `H1` (`sel / drift`) | `S3_lifespan_at_t_series` (`F-053`) | `48` | `-86.9291` | `[-941.6537, +767.7955]` | `-0.0295` | `8.388e-1` | `1.000` | `false` |
| `H2` (`sel / mut0`) | `R1_tail20_D` (`> 160k`) | `48` | `-0.144477` (`0.8655x`) | `[-0.183544, -0.105410]` | `-1.0738` | `1.766e-9` | `9.273e-9` | `true` |
| `H2` (`sel / mut0`) | `S1_trait_mass_var` | `48` | `+0.042290` | `[+0.034723, +0.049856]` | `+1.6229` | `6.373e-15` | `6.692e-14` | `true` |
| `H2` (`sel / mut0`) | `S2_energy_post_mean` | `48` | `-0.003489` | `[-0.063837, +0.056859]` | `-0.0168` | `9.079e-1` | `1.000` | `false` |
| `H2` (`sel / mut0`) | `S3_births_sum` | `48` | `+188.8958` | `[+113.2616, +264.5300]` | `+0.7252` | `7.748e-6` | `2.324e-5` | `true` |
| `H2` (`sel / mut0`) | `S3_deaths_sum` | `48` | `+172.6667` | `[+102.1280, +243.2053]` | `+0.7108` | `1.086e-5` | `2.533e-5` | `true` |
| `H2` (`sel / mut0`) | `S3_cumulative_mean_lifespan` | `48` | `+59.2850` | `[+22.2278, +96.3421]` | `+0.4645` | `2.338e-3` | `4.091e-3` | `true` |
| `H2` (`sel / mut0`) | `S3_lifespan_at_t_series` (`F-053`) | `48` | `+269.0296` | `[-554.0994, +1092.1586]` | `+0.0949` | `5.141e-1` | `6.747e-1` | `false` |
| `H3` (`drift / mut0`) | `R1_tail20_D` (`> 160k`) | `48` | `-0.141118` (`0.8684x`) | `[-0.179993, -0.102243]` | `-1.0540` | `2.845e-9` | `1.195e-8` | `true` |
| `H3` (`drift / mut0`) | `S1_trait_mass_var` | `48` | `+0.054842` | `[+0.044565, +0.065118]` | `+1.5496` | `3.079e-14` | `2.155e-13` | `true` |
| `H3` (`drift / mut0`) | `S2_energy_post_mean` | `48` | `+0.003046` | `[-0.058331, +0.064422]` | `+0.0144` | `9.209e-1` | `1.000` | `false` |
| `H3` (`drift / mut0`) | `S3_births_sum` | `48` | `+188.8958` | `[+113.2616, +264.5300]` | `+0.7252` | `7.748e-6` | `2.324e-5` | `true` |
| `H3` (`drift / mut0`) | `S3_deaths_sum` | `48` | `+172.6667` | `[+102.1280, +243.2053]` | `+0.7108` | `1.086e-5` | `2.533e-5` | `true` |
| `H3` (`drift / mut0`) | `S3_cumulative_mean_lifespan` | `48` | `+62.2657` | `[+25.5713, +98.9602]` | `+0.4927` | `1.329e-3` | `2.537e-3` | `true` |
| `H3` (`drift / mut0`) | `S3_lifespan_at_t_series` (`F-053`) | `48` | `+355.9587` | `[-597.8447, +1309.7621]` | `+0.1084` | `4.565e-1` | `6.391e-1` | `false` |

---

## 5. Deviation (Approved Under `QD-GATES-1127 (2)`)

- **Scientific / statistical protocol (`R1`–`R10`, hypotheses, SESOI, seeds, manifests, analysis rules):** **None.**
- **Confirmation provenance re-emission under `QD-GATES-1127 (2)` (owner approved 2026-11-27 11:43 ICT, `PR #336`):**
  1. **Why it occurred:** During Invocation 1 (`2026-11-26`, `--release --features desktop` build immediately before `--run-all`), `tauri-build` rewrote `src-tauri/gen/schemas/desktop-schema.json` and `src-tauri/gen/schemas/windows-schema.json` in place (line endings only, `0` byte-content diff under `git diff --binary` / `--ignore-cr-at-eol`). `node scripts/verify_sel2p2_artifacts.mjs` rejected `confirmation/provenance.json` strictly on those two `build.git_dirty_paths` entries (`EXIT=1`, zero other failures).
  2. **Exact option-1 procedure executed (`0` seeds simulated, no rebuild):**
     - The original `confirmation/` output was SHA-256-manifested (`confirmation_orig.sha256`, `10` files) and moved intact outside the repository to `D:\teamwork_projects\sel2p2_confirmation_orig_247abf42\`.
     - `git checkout -- src-tauri/gen/schemas/` restored both schema files so `git status --porcelain` no longer listed them.
     - The exact unchanged binary (`5217be55851b1389afcb6d4078df6f1630c3ac335095cccb8cc5b39cfc661186` before and after) was invoked once (`--confirm --smoke-dir artifacts/experiments/sel2-p2/smoke --out artifacts/experiments/sel2-p2/confirmation --checkpoint-dir D:\teamwork_projects\sel2p2_ckpt_247abf42 --jobs 8`), loading all `48 / 48` pairs from the original checkpoints in `0.1027432 s` with `0` seeds re-simulated and `0 / 48` checkpoint files modified.
     - `smoke/` was kept untouched, so `smoke/provenance.json` still lists `src-tauri/gen/schemas/desktop-schema.json` and `src-tauri/gen/schemas/windows-schema.json`.
     - [`scripts/verify_sel2p2_artifacts.mjs`](../../../scripts/verify_sel2p2_artifacts.mjs) was **not** edited (`git diff` empty) and exits `0` on both the bundle check and `--self-test`.
  3. **Step-5 comparison result (`test_runs/anima-case_SEL2-P2-RUN_option1/step5_comparison.txt`):**
     - **Byte-identical (`8 / 8` data and manifest files):** `effects.json`, `final-observables.json`, `runs.csv`, `series.csv`, and all `4` files under `manifests/` match `confirmation_orig.sha256` byte-for-byte.
     - **`checksums.sha256`:** differs strictly on the single `provenance.json` line (`9a69d91750df...` -> `eaf22576025d...`); all other `8` lines are identical.
     - **`provenance.json` changed fields (all expected):** `build.git_dirty_paths` (drops the two `src-tauri/gen/schemas/*.json` paths, leaving `["artifacts/experiments/sel2-p2/confirmation/", "artifacts/experiments/sel2-p2/smoke/"]`), `build.git_describe` (`"247abf4-dirty"` -> `"247abf4"`), `smoke_reference.smoke_dir` (`"artifacts/experiments/sel2-p2/smoke/"` -> `"artifacts/experiments/sel2-p2/smoke"`), and `timing.{started_utc, ended_utc, wall_clock_seconds}`.
     - **Hub evidence archive:** `confirmation_orig.sha256`, `step4_reemit.log`, and `step5_comparison.txt` are stored under `G:\My Drive\Anima_Agent_Hub\test_runs\anima-case_SEL2-P2-RUN_option1\`.

---

## 6. What This Result Does NOT Show

1. **No inference across arbitrary maps, worlds, or live desktop UI execution:** All `144` confirmation runs executed inside `LiveExperimentAdapter` (`live-bevy-world/2`) on one `Generated` `256x256` terrain (`world_identity` checksum `3152323152`) and do not establish behavior on World v2 terrain, desktop GPU/UI scheduling, or asynchronous learner threads.
2. **Single herbivore trophic level without neural evolution:** `live.predator_fraction = 0.0` and `live.evolved_brains = 0.0`; findings do not speak to predator-prey size selection or neural-plasticity co-evolution.
3. **2-segment clonal founder pinned at the `mutation.rs:209-231` deletion wall:** All lineages started from `live.founder_genotype = 1.0` (`nodes = [1.5, 1.0]`, mass `2.5`), where structural deletions require `nodes.len() > 2`; the downward shift in `H2`/`H3` (`0.94x` over `(20k, 200k]`, `0.87x` in `tail20`) reflects mutational/transmission dynamics from that founder rather than ecological selection (`H1 = 0.9970x`).
4. **No separation of developmental plasticity from genotypic change:** Body mass was measured as `live.trait_mass_mean` directly under the frozen `31`-key `LIVE_KEYS` partition.
