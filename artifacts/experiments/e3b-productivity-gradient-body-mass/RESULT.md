# E3b Result — Does a Spatial Productivity Gradient Increase Body-Mass Variance Under Matched Resources?

**Executed once on 2026-11-24 under the preregistration frozen at commit `b0413f2` (`PR #229` / `PR #231`).**
Confirmation ensemble commit `8d52341549007a50ce033a6d272c5dd22a71b1b1`, release binary SHA-256
`645564b730c43bbb1e51a247e8530f5cd93b15231e1ca16824fb00e81e603413`. All `192` runs (`64` seeds × `3` arms) completed on
`Windows 1` (`windows/x86_64`) after `C2. E3B-SMOKE` (`PR #240`, seed `999953`) passed all four `WP-2` gates (`body_mass_consulted: false`). Body-mass values remained unexamined during `C2` (`PR #240`) and `C3` (`PR #245`) and were unblinded only in `C4. E3B-RESULT` after `PR #245` merged into `feat/ecological-selection`.

> **Model status, stated the only way it may be stated: headless adapter verified.** No desktop app,
> Tauri handle, window, GPU device, renderer, learner thread, or evolution thread existed in the process that
> produced these numbers. Nothing here may be quoted as `"the live world is experiment-ready"`.
>
> **Experimental setting (frozen `§1.2`–`§1.3`):** E3b is a **two-trophic experiment with persistent
> predators** (`live.predator_fraction = 0.3`; predators persist to `T = 18,000` in `64/64` `U`, `53/64` `S`,
> and `63/64` `T` seeds — never *"effectively herbivore-only"*). Effects measure **body-mass variance ratio
> after `~k` generations (`~3.17 / 2.26 / 3.08` in `U / S / T`) of de novo input from an adapted
> clonal founder (`live.founder_genotype = 1.0`, two segments, total mass `2.5`, at the
> `nodes.len() > 2` operator-4 deletion boundary, with persistent predators)**.

---

## 1. Executive Decision (Frozen `R1`–`R10` Rules)

**Bundle integrity verifier command (passes with `0` `NOTE:` fallbacks on the committed confirmation bundle):**

```bash
npm run check:e3b
node scripts/verify_e3b_artifacts.mjs --self-test
```

| Contrast (`B / A`) | Hypothesis & Test Type | `n_LL / 64` ($\hat{\pi}_{LL}$) | $\bar{L}$ ($\varepsilon = 5\times 10^{-4}$) | Geom. Ratio $e^{\bar{L}}$ | 95% $t$-CI on $\bar{L}$ | 90% TOST CI vs $\pm\ln(1.5)$ (`±0.405465`) | $p_{\text{two-sided}}$ | $p_{\text{Holm}}$ ($m = 3$) | TOST Equiv. | Preregistered `R10` Label (`direction_tag`) |
|---|---|---:|---:|---:|---|---|---:|---:|---|---|
| **`H1` (`T / S`)** | Spatial gradient vs matched-resource uniform scarcity (`D2(a)` directional) | `64 / 64` (`1.0`) | **`+0.301635`** | `1.352068x` | `[-0.007090, +0.610360]` | `[+0.043728, +0.559542]` | `0.055332` | **`0.165995`** | `false` | **`insufficient_evidence`** (`inconclusive_wide_ci`) |
| **`H2` (`T / U`)** | Gradient + scarcity vs uniform abundant control (two-sided) | `64 / 64` (`1.0`) | **`+0.024247`** | `1.024543x` | `[-0.343577, +0.392070]` | `[-0.283031, +0.331524]` | `0.895618` | **`0.895618`** | **`true`** | **`equivalent_to_zero`** (`equivalent_to_zero_powered_null`) |
| **`H3` (`S / U`)** | Uniform scarcity + refill vs uniform abundant control (two-sided) | `64 / 64` (`1.0`) | **`-0.277388`** | `0.757760x` | `[-0.679130, +0.124353]` | `[-0.613001, +0.058224]` | `0.172530` | **`0.345060`** | `false` | **`insufficient_evidence`** (`inconclusive_wide_ci`) |

*Reproduced directly from committed artifacts by `npm run check:e3b` and verified against `evaluateE3bDataset()` in [`scripts/verify_e3b_artifacts.mjs`](../../../scripts/verify_e3b_artifacts.mjs):*

```bash
node --input-type=module -e "import fs from 'node:fs'; import { evaluateE3bDataset, loadCommittedPrereg, parseCsv } from './scripts/verify_e3b_artifacts.mjs'; const d='artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation'; const r=evaluateE3bDataset({ runs:parseCsv(fs.readFileSync(d+'/runs.csv','utf8')), series:parseCsv(fs.readFileSync(d+'/series.csv','utf8')), finalObservables:JSON.parse(fs.readFileSync(d+'/final-observables.json','utf8')), prereg:loadCommittedPrereg(), wp2AndHostValid:true }); console.log(r.primary_decision, r.primary_direction_tag); for (const c of ['H1','H2','H3']) { const x=r.contrasts[c], p=x.primary; console.log(c, x.decision, x.direction_tag, {n:p.n_complete_pairs, pi:p.pi_ll, mean_L:p.mean_L, geom:p.geom_var_ratio, sd_L:p.sd_L, ci95:[p.ci95_low,p.ci95_high], ci90:[p.ci90_tost_low,p.ci90_tost_high], p2:p.p_two_sided, p_holm:x.p_holm, tost:p.tost_equivalent, dz:p.paired_dz, wP:p.wilcoxon.p_value, rc:p.wilcoxon.r_c}); }"
```

**Primary outcome summary:**
- **`H1` (`T / S`) is inconclusive (`insufficient_evidence` / `inconclusive_wide_ci`):** Although the point estimate points upward (`mean_L = +0.301635`, `1.352x` geometric variance ratio, `59.4%` positive pairs), the observed between-seed SD (`sd_L = 1.235926`) exceeded the pre-run planning benchmark (`0.8109`), yielding an achieved `0.5 * sd_L` MDE of `0.617963` (exact `80%` Holm-1 MDE `0.4139 * sd_L = 0.511550`), both wider than the preregistered `±ln(1.5) = ±0.405465` SESOI. Consequently, neither superiority over zero (`p_holm = 0.165995 >= 0.05`) nor equivalence within `±ln(1.5)` (`90% TOST CI [+0.043728, +0.559542]` crosses `+0.405465`, `tost_p2 = 0.251995`) is established.
- **`H2` (`T / U`) clears `R9` equivalence (`equivalent_to_zero` / `equivalent_to_zero_powered_null`):** On the preregistered primary pseudocount (`eps = 5e-4`), the 90% TOST CI `[-0.283031, +0.331524]` lies strictly inside `(-0.405465, +0.405465)` (`tost_p1 = 0.011383`, `tost_p2 = 0.021225`, `tost_equivalent = true`). *(Sensitivity check at `eps = 5e-5` does not clear the upper bound because single-tick `var = 0` at seed `843001` `U` inflates `sd_L` from `1.4725` to `1.6898`; per `D4`, sensitivity carries no decision weight.)*
- **`H3` (`S / U`) is inconclusive (`insufficient_evidence` / `inconclusive_wide_ci`):** The 95% CI `[-0.679130, +0.124353]` and 90% TOST CI `[-0.613001, +0.058224]` straddle both `0` and `-ln(1.5)`.

---

## 2. Provenance, Execution Isolation & Operational Gate Audit

| Field | Smoke (`C2. E3B-SMOKE`) | Confirmation (`C3. E3B-RUN`) |
|---|---|---|
| Frozen preregistration | `b0413f2eeecbab8e5ee045c22c72d5c00c16b87a` (`PR #229`) / `ebe636daf96b18a3e3c483fec951f5c12230c633` (`PR #231`) | Same ([`2026-11-24-e3b-preregistration.md`](../../../docs/ai/planning/2026-11-24-e3b-preregistration.md)) |
| Execution commit & PR | `ed634d8c0e646d6a78ca9e079f7bb5346d2d8b93` (`PR #240`, merged `9eeb3130`) | `8d52341549007a50ce033a6d272c5dd22a71b1b1` (`PR #245`, merged `4a5ee26a`) |
| Runner binary SHA-256 (`run_e3b_experiment.exe`) | `381d08907ce2945ccbc9a62059b15d3fbf84264263ad529c93aab9b4456435fd` | `645564b730c43bbb1e51a247e8530f5cd93b15231e1ca16824fb00e81e603413` |
| Host, compiler, profile | `windows` / `x86_64` (`Windows 1`), `rustc 1.98.1 (48a229cea 2026-09-01)`, `release` (`--features desktop`) | Same (`CARGO_BUILD_JOBS=8`, `ANIMA_USE_GPU=0`, `--jobs 8`) |
| World isolation | `ANIMA_WORLD_ARTIFACT` unset, `%TEMP%/anima_world_artifact.anmw` absent, `terrain_source = "generated"` | Same across all `192/192` runs |
| Observed `world_identity` | `seed = 1337`, `generator_version = 1`, `256x256`, `checksum = 3152323152` | Identical across all `192/192` runs (`world_identity_all_runs_identical = true`) |
| Model & registry | `LiveExperimentAdapter` (`live-bevy-world/2`), `ObservableRegistry::live_default` (`1935740381389641332`) | Same |
| Seeds & sample grid | Blind seed `999953` × `3` arms (`22` non-mass observables, `0` `trait_mass`) | `64` seeds `810001..=873001` (`810001 + 1000*k`) × `3` arms = `192` runs, `T = 18,000`, `sample_period = 600` (`30` samples), `warmup = 3,000`, `tail = [14400, 18000]` |
| Wall-clock time | `38.4740051 s` (`2026-11-24T00:38:29Z` -> `00:39:07Z`) | `321.0087407 s` (`2026-11-24T05:28:11Z` -> `05:33:32Z`; `192` runs: min `10.6982 s`, median `13.09385 s`, max `15.5829 s`, sum `2,532.5561 s`) |
| Smoke `WP-2` gate check | **`wp2_passed = true`**: `WP-2(a)` initial EU `153,587.249864` (`0.42` ratio); `WP-2(b1) = -48.6281%`; `WP-2(b2) = -0.5175%` (`0/300`); `WP-2(c)` detritus `>0` at `600` & `3,000` | `smoke_reference.wp2_passed = true` verified; `stopped_early = false`, `failed_runs_count = 0`, `s6_lifespan_source = "series.csv"` |

*Provenance & timing reproduction command (`npm run check:e3b` verifies `provenance.json` + [`confirmation/checksums.sha256`](confirmation/checksums.sha256)):*

```bash
node --input-type=module -e "import fs from 'node:fs'; import { median, parseCsv } from './scripts/verify_e3b_artifacts.mjs'; const root='artifacts/experiments/e3b-productivity-gradient-body-mass', d=root+'/confirmation'; const prov=JSON.parse(fs.readFileSync(d+'/provenance.json','utf8')), spro=JSON.parse(fs.readFileSync(root+'/smoke/provenance.json','utf8')), smk=JSON.parse(fs.readFileSync(root+'/smoke/smoke-report.json','utf8')); const runs=parseCsv(fs.readFileSync(d+'/runs.csv','utf8')); const ts=runs.map(r=>Number(r.elapsed_secs)); console.log({smoke_commit:spro.build.git_commit, smoke_bin:spro.build.binary_sha256, smoke_s:smk.elapsed_seconds, wp2:smk.wp2_passed, init_eu:smk.wp2_a_resource_field.initial_closed_eu_uniform, b1:smk.wp2_b1_gradient_vs_uniform.relative_post_warmup_shift, b2:smk.wp2_b2_scarcity_vs_gradient.relative_post_warmup_mismatch, commit:prov.build.git_commit, bin:prov.build.binary_sha256, smoke_ref:prov.smoke_reference.wp2_passed, n_runs:runs.length, wall_s:prov.timing.wall_clock_seconds, min:Math.min(...ts), med:median(ts), max:Math.max(...ts), sum:ts.reduce((a,b)=>a+b,0)});"
```

### Closed-EU Conservation Audit (`I1`, Ceiling `<= 1.0e-6 EU`)

| Arm | `Completed` / `Failed` | `extinct` | `singleton_final` | `singleton_tail` | `empty_half` (`W`/`E`/`Both`) | `max_abs_eu_drift` Min | `max_abs_eu_drift` Median | `max_abs_eu_drift` Max | Runs `> 1e-6` |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `control_uniform` (`U`) | `64` / `0` | `0` | `0` | `0` | `0` / `0` / `0` | `7.275958e-10` | `2.823072e-9` | `1.044828e-8` | `0` |
| `control_uniform_scarcity` (`S`) | `64` / `0` | `0` | `0` | `0` | `0` / `0` / `0` | `9.225914e-9` | `3.038440e-8` | `5.791662e-8` | `0` |
| `treatment_gradient` (`T`) | `64` / `0` | `0` | `0` | `0` | `0` / `0` / `0` | `2.226443e-8` | `4.182220e-8` | `7.098424e-8` | `0` |
| **All arms (`192` runs)** | **`192` / `0`** | **`0`** | **`0`** | **`0`** | **`0` / `0` / `0`** | **`7.275958e-10`** | **`2.876914e-8`** | **`7.098424e-8`** | **`0`** |

*Reproduced from [`confirmation/effects.json`](confirmation/effects.json) (`arm_diagnostics`):*

```bash
node --input-type=module -e "import fs from 'node:fs'; import { median } from './scripts/verify_e3b_artifacts.mjs'; const ds=JSON.parse(fs.readFileSync('artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation/effects.json','utf8')).arm_diagnostics; for (const arm of ['control_uniform','control_uniform_scarcity','treatment_gradient','ALL']) { const a=arm==='ALL'?ds:ds.filter(x=>x.arm===arm), e=a.map(x=>x.max_abs_eu_drift); console.log(arm, {completed:a.filter(x=>x.completed).length, ext:a.filter(x=>x.extinct).length, sgl:a.filter(x=>x.singleton_final||x.singleton_tail).length, eh:a.filter(x=>x.empty_half!==null).length, min:Math.min(...e), med:median(e), max:Math.max(...e), gt1e6:e.filter(v=>v>1e-6).length}); }"
```

---

## 3. Confirmatory Statistical Analysis (`R1`–`R10`)

### 3.1. Detailed Primary Contrasts, Non-Parametric Tests, and `R8` Effect-Size Bundle ($\varepsilon = 5 \times 10^{-4}$)

| Metric | `H1` (`T / S`, directional) | `H2` (`T / U`, two-sided) | `H3` (`S / U`, two-sided) |
|---|---:|---:|---:|
| Complete pairs `n_LL / 64` ($\hat{\pi}_{LL}$) | `64 / 64` (`1.000000`) | `64 / 64` (`1.000000`) | `64 / 64` (`1.000000`) |
| `mean_L` | `+0.301635` | `+0.024247` | `-0.277388` |
| Geometric variance ratio $\exp(\bar{L})$ | `1.352068` | `1.024543` | `0.757760` |
| `median_L` | `+0.278964` | `-0.149029` | `-0.474100` |
| `sd_L` / `se_L` | `1.235926` / `0.154491` | `1.472515` / `0.184064` | `1.608299` / `0.201037` |
| Paired `t` (`df = 63`) / `p_two_sided` | `+1.952449` / `0.055332` | `+0.131729` / `0.895618` | `-1.379786` / `0.172530` |
| Holm-adjusted `p_holm` ($m = 3$) | **`0.165995`** | **`0.895618`** | **`0.345060`** |
| 95% $t$-CI on $\bar{L}$ | `[-0.007090, +0.610360]` | `[-0.343577, +0.392070]` | `[-0.679130, +0.124353]` |
| 90% TOST $t$-CI on $\bar{L}$ | `[+0.043728, +0.559542]` | `[-0.283031, +0.331524]` | `[-0.613001, +0.058224]` |
| TOST `p1` ($-\text{SESOI}$) / `p2` ($+\text{SESOI}$) | `0.000011` / `0.251995` | `0.011383` / `0.021225` | `0.263191` / `0.000593` |
| TOST equivalent ($\pm 0.405465$) | `false` | **`true`** | `false` |
| Paired Cohen's `d_z` | `+0.244056` | `+0.016466` | `-0.172473` |
| Pre-registered `0.5 * sd_L` (`80%` Holm-1 `0.4139 * sd_L`) | `0.617963` (`0.511550`) | `0.736257` (`0.609474`) | `0.804149` (`0.665675`) |
| Wilcoxon signed-rank `W+` / `W-` | `1309.0` / `771.0` | `1006.0` / `1074.0` | `763.0` / `1317.0` |
| Wilcoxon `p_value` (`R7` `p_min = 1.084e-19`, met) | `0.072028` (`true`) | `0.820132` (`true`) | `0.063963` (`true`) |
| Matched-pairs rank-biserial `r_c` | `+0.258654` | `-0.032692` | `-0.266346` |
| Concordance (`L_i > 0` / `L_i < 0`) | `38/64` (`0.59375`) / `26/64` (`0.40625`) | `29/64` (`0.453125`) / `35/64` (`0.546875`) | `22/64` (`0.34375`) / `42/64` (`0.65625`) |
| Hodges–Lehmann pseudomedian [`95% BCa CI`] | `+0.300394` `[-0.019578, +0.590164]` | `-0.050391` `[-0.359642, +0.294807]` | `-0.382213` `[-0.717146, +0.026470]` |
| Observed `L_i` range (`[min, max]`) | `[-2.206028, +3.471196]` | `[-3.611345, +4.244440]` | `[-4.049522, +3.930831]` |

- **Exact per-seed log additivity verified:** Across all `64` seeds valid in all three arms, $\max_i |L_i(T/U) - [L_i(T/S) + L_i(S/U)]| = 4.44 \times 10^{-16}$ (`0.024247 = +0.301635 + (-0.277388)`).

*Reproduced directly from committed [`confirmation/effects.json`](confirmation/effects.json) and `hodgesLehmannBcaCi()` in [`scripts/verify_e3b_artifacts.mjs`](../../../scripts/verify_e3b_artifacts.mjs):*

```bash
node --input-type=module -e "import fs from 'node:fs'; import { hodgesLehmannBcaCi } from './scripts/verify_e3b_artifacts.mjs'; const c=JSON.parse(fs.readFileSync('artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation/effects.json','utf8')).contrasts; for (const id of ['H1','H2','H3']) { const x=c[id], p=x.primary, b=hodgesLehmannBcaCi(p.diffs); console.log(id, {p, p_holm:x.p_holm, mde_holm1_80:0.4139*p.sd_L, hl_bca:b, min:Math.min(...p.diffs), max:Math.max(...p.diffs)}); } console.log('additivity_max_err', Math.max(...c.H2.primary.diffs.map((d,i)=>Math.abs(d-(c.H1.primary.diffs[i]+c.H3.primary.diffs[i])))));"
```

### 3.2. Attrition, Lee (2009) Trimming Bounds, and Survival `RMST(tau = 18,000)` (`R3` / `D3`)

Because `192/192` runs survived through `T = 18,000` without extinction, singleton exclusion, or missing runs (`n_a = n_b = k = 64`), Lee bounds collapse to point-identified equality (`width = 0.0`) and `RMST` is degenerately constant at `18,000.0` ticks in every arm.

| Contrast | `n_a / n_b / k` | Lee `[lee_lo, lee_hi]` | Lee `width` | `RMST` Control / Treatment Mean (ticks) | `RMST` Paired Delta (`95% CI`) |
|---|---:|---|---:|---:|---:|
| **`H1` (`T / S`)** | `64 / 64 / 64` | `[+0.301635, +0.301635]` | `0.000000` | `18000.00` / `18000.00` | `0.00` (`[0, 0]`) |
| **`H2` (`T / U`)** | `64 / 64 / 64` | `[+0.024247, +0.024247]` | `0.000000` | `18000.00` / `18000.00` | `0.00` (`[0, 0]`) |
| **`H3` (`S / U`)** | `64 / 64 / 64` | `[-0.277388, -0.277388]` | `0.000000` | `18000.00` / `18000.00` | `0.00` (`[0, 0]`) |

*Reproduced from [`confirmation/effects.json`](confirmation/effects.json) and `npm run check:e3b`:*

```bash
node --input-type=module -e "import fs from 'node:fs'; const c=JSON.parse(fs.readFileSync('artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation/effects.json','utf8')).contrasts; for (const id of ['H1','H2','H3']) console.log(id, {lee:c[id].lee_bounds, rmst:c[id].rmst});"
```

### 3.3. Primary-Metric Sensitivity Check ($\varepsilon_{\text{sens}} = 5 \times 10^{-5}$, Diagnostic Only, `decision_effect: none`)

| Contrast | `mean_L` | Geom. Ratio | `median_L` | `sd_L` / `se_L` | `t` / `p_two_sided` | 95% CI | 90% TOST CI | TOST Equiv. ($\pm\ln 1.5$) | `d_z` | Wilcoxon `p` / `r_c` | `HL` [`95% BCa CI`] |
|---|---:|---:|---:|---:|---:|---|---|---:|---:|---|---|
| **`H1` (`T / S`)** | `+0.354714` | `1.425772x` | `+0.286629` | `1.427821` / `0.178478` | `+1.987441` / `0.051225` | `[-0.001945, +0.711373]` | `[+0.056763, +0.652665]` | `false` (`p2 = 0.388535`) | `+0.248430` | `0.067897` / `+0.262500` | `+0.311941` `[-0.019368, +0.625422]` |
| **`H2` (`T / U`)** | `+0.071470` | `1.074086x` | `-0.154581` | `1.689822` / `0.211228` | `+0.338356` / `0.736220` | `[-0.350635, +0.493575]` | `[-0.281154, +0.424094]` | `false` (`p2 = 0.059419`) | `+0.042294` | `0.809749` / `-0.034615` | `-0.057161` `[-0.374480, +0.302001]` |
| **`H3` (`S / U`)** | `-0.283244` | `0.753336x` | `-0.491333` | `1.908898` / `0.238612` | `-1.187046` / `0.239666` | `[-0.760072, +0.193585]` | `[-0.681583, +0.115096]` | `false` (`p1 = 0.305145`) | `-0.148381` | `0.064928` / `-0.265385` | `-0.393959` `[-0.740091, +0.035254]` |

*Reproduced by the same `npm run check:e3b` / `verify_e3b_artifacts.mjs` run:*

```bash
node --input-type=module -e "import fs from 'node:fs'; import { hodgesLehmannBcaCi } from './scripts/verify_e3b_artifacts.mjs'; const c=JSON.parse(fs.readFileSync('artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation/effects.json','utf8')).contrasts; for (const id of ['H1','H2','H3']) { const s=c[id].sensitivity_eps_5e5; console.log(id, {mean_L:s.mean_L, geom:s.geom_var_ratio, med:s.median_L, sd:s.sd_L, t:s.t_stat, p:s.p_two_sided, ci95:[s.ci95_low,s.ci95_high], ci90:[s.ci90_tost_low,s.ci90_tost_high], tost:s.tost_equivalent, p1:s.tost_p1, p2:s.tost_p2, dz:s.paired_dz, wP:s.wilcoxon.p_value, rc:s.wilcoxon.r_c, hl_bca:hodgesLehmannBcaCi(s.diffs)}); }"
```

### 3.4. Mandatory Turnover Mediator OLS ($L_i \sim \ln(\text{births}_B / \text{births}_A)$)

| Contrast | `n` | Realized $\ln(\text{births}_B / \text{births}_A)$ Mean ± SD | OLS `slope` | OLS `intercept` | $R^2$ | Pearson $r$ | `p_slope` | Spearman $\rho$ |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| **`H1` (`T / S`)** | `64` | `+0.355853 ± 0.090934` | `+0.356382` | `+0.174816` | `0.000688` | `+0.026221` | `0.837049` | `+0.007967` |
| **`H2` (`T / U`)** | `64` | `-0.029910 ± 0.070408` | `+5.403494` | `+0.185867` | `0.066754` | `+0.258369` | `0.039269` | `+0.159524` |
| **`H3` (`S / U`)** | `64` | `-0.385763 ± 0.116541` | `+1.024556` | `+0.117848` | `0.005512` | `+0.074242` | `0.559872` | `+0.108106` |

- Within-contrast between-seed variation in birth turnover explains `< 0.1%` of `L_i` variance in `H1` (`p = 0.8370`) and `< 0.6%` in `H3` (`p = 0.5599`), while `H2` shows a weak nominal positive association (`p = 0.0393`, BH exploratory `q = 0.0721`). Because between-arm `ln(births_T / births_S)` (`+0.3559`) is structurally collinear with treatment assignment, observational within-arm slope cannot isolate causation between arms.

*Reproduced from [`confirmation/effects.json`](confirmation/effects.json) and `pairedMediatorOls()` in `scripts/verify_e3b_artifacts.mjs`:*

```bash
node --input-type=module -e "import fs from 'node:fs'; import { mean, sampleSd } from './scripts/verify_e3b_artifacts.mjs'; const d='artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation'; const eff=JSON.parse(fs.readFileSync(d+'/effects.json','utf8')); const m=new Map(eff.arm_diagnostics.map(x=>[x.seed+'::'+x.arm, x.births_sum])); const seeds=[...new Set(eff.arm_diagnostics.map(x=>x.seed))]; for (const [id,a,b] of [['H1','control_uniform_scarcity','treatment_gradient'],['H2','control_uniform','treatment_gradient'],['H3','control_uniform','control_uniform_scarcity']]) { const lb=seeds.map(s=>Math.log(m.get(s+'::'+b)/m.get(s+'::'+a))); console.log(id, {lnB_mean:mean(lb), lnB_sd:sampleSd(lb), mediator:eff.contrasts[id].mediator_ln_births}); }"
```

---

## 4. Exploratory and Secondary Results (`decision_effect: none`)

> **Strict rule (`R2` / `C4`):** None of the metrics below modify any confirmatory `H1`–`H3` `R10` decision. All `42` paired exploratory `p`-values are adjusted jointly via Benjamini–Hochberg (`q < 0.05`) in Table 4.2 and remain hypothesis-generating only.

### 4.1. Per-Arm Ecological, Trophic, Demographic, and Body-Mass Descriptives (`n = 64` seeds per arm)

| Quantity (`T = 18,000` unless noted) | `control_uniform` (`U`) | `control_uniform_scarcity` (`S`) | `treatment_gradient` (`T`) |
|---|---:|---:|---:|
| `live.trait_mass_var` Mean ± SD (Median) | `0.028131 ± 0.021538` (`0.026244`) | `0.020782 ± 0.017961` (`0.014211`) | `0.026956 ± 0.021159` (`0.023320`) |
| `live.trait_mass_var` Range (`[min, max]`) | `[0.000000, 0.086570]` | `[0.000000, 0.082409]` | `[0.001290, 0.090741]` |
| `live.trait_mass_var` Bessel Mean ± SD | `0.028310 ± 0.021671` | `0.020928 ± 0.018095` | `0.027120 ± 0.021288` |
| Runs with `trait_mass_var == 0` at `T` | `1` (`seed 843001`, `n=94`, tail `0.009088`) | `1` (`seed 859001`, `n=161`, tail `0.003827`) | `0` (`min = 0.001290`) |
| `S2`: `live.trait_mass_mean` Mean ± SD | `2.504737 ± 0.039804` | `2.509983 ± 0.019216` | `2.515282 ± 0.031790` |
| `S1`: `trait_mass_p90 - p10` Mean ± SD | `0.066698 ± 0.127034` | `0.019961 ± 0.057367` | `0.042704 ± 0.115109` |
| `S3`: `mass_mean_east - west` Mean ± SD | `-0.002559 ± 0.057433` | `+0.002208 ± 0.029996` | `+0.018194 ± 0.033731` |
| Final `live.agent_count` Mean ± SD `[min, max]` | `159.17 ± 17.90` `[80, 171]` | `148.86 ± 20.95` `[92, 168]` | `165.91 ± 2.40` `[160, 169]` |
| Series-mean `mean_N` (30 ticks) | `178.94` | `171.61` | `178.47` |
| **`live.predator_count > 0`** / Mean ± SD `[min, max]` | **`64/64`** (`8.19 ± 1.38`, `[4, 11]`) | **`53/64`** (`2.94 ± 2.42`, `[0, 8]`) | **`63/64`** (`6.05 ± 2.33`, `[0, 9]`) |
| `S6`: `births_sum` / `deaths_sum` Mean | `567.00` / `567.83` | `388.27` / `399.41` | `550.33` / `544.42` |
| Realized generations (`births / mean_N`) Mean ± SD | `3.168 ± 0.304` | `2.256 ± 0.298` | `3.085 ± 0.331` |
| `S4`: `live.shannon` Mean ± SD (trophic balance) | `0.203708 ± 0.033910` | `0.088751 ± 0.062842` | `0.152625 ± 0.048914` |
| `S5`: `live.archive_coverage` max | `0.0` | `0.0` | `0.0` |
| `S6`: `mean_lifespan` at `T` (`series.csv`) | `4094.19` | `4520.74` | `3520.36` |
| `S6`: `lifespan_post_warmup_pos` (`series.csv`) | `3988.93` | `4611.23` | `4010.87` |
| `S6`: `cumulative_mean_lifespan` (`final`) | `3900.92` | `4676.52` | `3934.10` |
| `M1`: `post_warmup_crop` (EU) Mean ± SD | `121,523.59 ± 2,711.04` | `63,707.24 ± 94.07` | `63,945.15 ± 100.72` |

- **Confirmation `WP-2` crop check (informative across all 64 seeds):** `M1(b1)` `(T - U) / U = -47.3805%` (`|shift| >= 10%`); `M1(b2)` `(T - S) / T = +0.3720%` (`|mismatch| <= 10%`), confirming that `T` and `S` stayed closely resource-matched at `~64k EU` standing crop across all `64` seeds while differing strongly in spatial heterogeneity (`T` asymmetric `Deforest 0.30 / AddNutrient 1.00` vs `S` uniform `0.42` scarcity + whole-map refill).
- **Root mechanism behind `sd_L = 1.236` (`> 0.811` planning benchmark):** Two single-tick zero-variance excursions at `T = 18,000` (`seed 843001` in `U`, `94` clonal-mass survivors after nonzero tail `0.009088`; `seed 859001` in `S`, `161` clonal-mass survivors after nonzero tail `0.003827`) produce extreme single-tick ratios (`H1` max `+3.4712` at `859001`, `H2` max `+4.2444` at `843001`, `H3` min `-4.0495` at `859001`). Averaging the 7-sample tail window `[14400, 18000]` shrinks `sd_L` from `1.2359` to `1.0001` (`H1`), `1.4725` to `1.1134` (`H2`), and `1.6083` to `1.0397` (`H3`).

*Reproduced from committed `confirmation/{final-observables.json,series.csv,effects.json}`:*

```bash
node --input-type=module -e "import fs from 'node:fs'; import { mean, median, parseCsv, sampleSd } from './scripts/verify_e3b_artifacts.mjs'; const d='artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation'; const eff=JSON.parse(fs.readFileSync(d+'/effects.json','utf8')); const fin=JSON.parse(fs.readFileSync(d+'/final-observables.json','utf8')); const ser=parseCsv(fs.readFileSync(d+'/series.csv','utf8')); const sm=new Map(); for (const r of ser) { const k=r.seed+'::'+r.arm; (sm.get(k)??sm.set(k,[]).get(k)).push(r); } const fm=new Map(fin.map(f=>[f.seed+'::'+f.arm, f.observables])); const out={}; for (const arm of ['control_uniform','control_uniform_scarcity','treatment_gradient']) { const rows=eff.arm_diagnostics.filter(x=>x.arm===arm).map(x=>{ const f=fm.get(x.seed+'::'+arm), mN=mean(sm.get(x.seed+'::'+arm).map(r=>Number(r['live.agent_count']))); return {seed:x.seed, v:Number(f['live.trait_mass_var']), vb:Number(f['live.trait_mass_var'])*x.final_agents/(x.final_agents-1), m:Number(f['live.trait_mass_mean']), iqr:Number(f['live.trait_mass_p90'])-Number(f['live.trait_mass_p10']), emw:Number(f['live.trait_mass_mean_east'])-Number(f['live.trait_mass_mean_west']), n:x.final_agents, mN, pr:x.final_predators, b:x.births_sum, d:x.deaths_sum, gen:x.births_sum/mN, sh:Number(f['live.shannon']), ac:Number(f['live.archive_coverage']), lst:x.lifespan_at_t, lspw:x.lifespan_post_warmup_pos, lsc:x.cumulative_mean_lifespan, crop:x.post_warmup_crop}; }); out[arm]={var_m:mean(rows.map(r=>r.v)), var_sd:sampleSd(rows.map(r=>r.v)), var_med:median(rows.map(r=>r.v)), var_min:Math.min(...rows.map(r=>r.v)), var_max:Math.max(...rows.map(r=>r.v)), vb_m:mean(rows.map(r=>r.vb)), vb_sd:sampleSd(rows.map(r=>r.vb)), zeros:rows.filter(r=>r.v===0).map(r=>r.seed), mass_m:mean(rows.map(r=>r.m)), mass_sd:sampleSd(rows.map(r=>r.m)), iqr_m:mean(rows.map(r=>r.iqr)), iqr_sd:sampleSd(rows.map(r=>r.iqr)), emw_m:mean(rows.map(r=>r.emw)), emw_sd:sampleSd(rows.map(r=>r.emw)), n_m:mean(rows.map(r=>r.n)), n_sd:sampleSd(rows.map(r=>r.n)), n_rng:[Math.min(...rows.map(r=>r.n)),Math.max(...rows.map(r=>r.n))], mN_m:mean(rows.map(r=>r.mN)), pred_pos:rows.filter(r=>r.pr>0).length, pred_m:mean(rows.map(r=>r.pr)), pred_sd:sampleSd(rows.map(r=>r.pr)), pred_rng:[Math.min(...rows.map(r=>r.pr)),Math.max(...rows.map(r=>r.pr))], b_m:mean(rows.map(r=>r.b)), d_m:mean(rows.map(r=>r.d)), gen_m:mean(rows.map(r=>r.gen)), gen_sd:sampleSd(rows.map(r=>r.gen)), sh_m:mean(rows.map(r=>r.sh)), sh_sd:sampleSd(rows.map(r=>r.sh)), ac_max:Math.max(...rows.map(r=>r.ac)), lst_m:mean(rows.map(r=>r.lst)), lspw_m:mean(rows.map(r=>r.lspw)), lsc_m:mean(rows.map(r=>r.lsc)), crop_m:mean(rows.map(r=>r.crop)), crop_sd:sampleSd(rows.map(r=>r.crop))}; console.log(arm, out[arm]); } console.log('WP2_M1', {b1:(out.treatment_gradient.crop_m-out.control_uniform.crop_m)/out.control_uniform.crop_m, b2:(out.treatment_gradient.crop_m-out.control_uniform_scarcity.crop_m)/out.treatment_gradient.crop_m});"
```

### 4.2. Secondary and Exploratory Contrast Family with BH-FDR (`m = 42`, `q = 0.05`, `decision_effect: none`)

| Contrast | Endpoint (`decision_effect: none`) | `n` | Paired Delta (or Est.) | 95% CI | `d_z` (or $r$) | `p_two_sided` | BH `q` (`m = 42`) | BH `< 0.05` |
|---|---|---:|---:|---|---:|---:|---:|:--:|
| `H1` (`T / S`) | `tail_window_L` (`[14400, 18000]`) | `64` | `+0.283700` (`sd 1.000125`) | `[+0.033876, +0.533523]` | `d_z = +0.283664` | `2.6682e-2` | `5.3365e-2` | `false` |
| `H1` (`T / S`) | `mediator_ln_births` (OLS slope) | `64` | `+0.356382` | `R^2 = 0.000688` | `r = +0.026221` | `8.3705e-1` | `8.4468e-1` | `false` |
| `H1` (`T / S`) | `S1_iqr` (`p90 - p10`) | `64` | `+0.022743` | `[-0.007844, +0.053330]` | `+0.185734` | `1.4230e-1` | `2.0609e-1` | `false` |
| `H1` (`T / S`) | `S2_mean` (`trait_mass_mean`) | `64` | `+0.005298` | `[-0.002887, +0.013483]` | `+0.161698` | `2.0053e-1` | `2.7169e-1` | `false` |
| `H1` (`T / S`) | **`S3_east_minus_west`** | `64` | **`+0.015987`** | `[+0.003839, +0.028134]` | `+0.328741` | `1.0720e-2` | `2.3698e-2` | `true` |
| `H1` (`T / S`) | `raw_var_delta` (`trait_mass_var`) | `64` | `+0.006174` | `[+0.000196, +0.012152]` | `+0.258001` | `4.3141e-2` | `7.2123e-2` | `false` |
| `H1` (`T / S`) | `bessel_var_delta` (`var * N/(N-1)`) | `64` | `+0.006192` | `[+0.000173, +0.012210]` | `+0.256986` | `4.3943e-2` | `7.2123e-2` | `false` |
| `H1` (`T / S`) | `lnCVR_nonzero` (note `eps=5e-4`: `p=0.05710`) | `63` | `+0.134036` | `[-0.025651, +0.293722]` | `+0.211392` | `9.8406e-2` | `1.4761e-1` | `false` |
| `H1` (`T / S`) | **`predator_count`** (covariate §1.2) | `64` | **`+3.109375`** | `[+2.289780, +3.928970]` | `+0.947662` | `1.9278e-10` | `7.3607e-10` | `true` |
| `H1` (`T / S`) | **`S4_shannon`** (trophic balance) | `64` | **`+0.063874`** | `[+0.044368, +0.083381]` | `+0.817947` | `1.2394e-8` | `4.0043e-8` | `true` |
| `H1` (`T / S`) | **`S6_births_sum`** | `64` | **`+162.0625`** | `[+154.2931, +169.8319]` | `+5.210435` | `1.3877e-47` | `1.9427e-46` | `true` |
| `H1` (`T / S`) | **`S6_deaths_sum`** | `64` | **`+145.0156`** | `[+139.0410, +150.9902]` | `+6.062989` | `1.3179e-51` | `5.5352e-50` | `true` |
| `H1` (`T / S`) | **`S6_lifespan_post_warmup_pos`** (`series`) | `64` | **`-600.3548`** | `[-645.6132, -555.0965]` | `-3.313514` | `7.0848e-36` | `4.9594e-35` | `true` |
| `H1` (`T / S`) | **`S6_cumulative_mean_lifespan`** | `64` | **`-742.4234`** | `[-775.5434, -709.3033]` | `-5.599388` | `1.7213e-49` | `3.6146e-48` | `true` |
| `H2` (`T / U`) | `tail_window_L` (`[14400, 18000]`) | `64` | `-0.069270` (`sd 1.113415`) | `[-0.347393, +0.208853]` | `d_z = -0.062214` | `6.2042e-1` | `6.8573e-1` | `false` |
| `H2` (`T / U`) | `mediator_ln_births` (OLS slope) | `64` | `+5.403494` | `R^2 = 0.066754` | `r = +0.258369` | `3.9269e-2` | `7.2123e-2` | `false` |
| `H2` (`T / U`) | `S1_iqr` (`p90 - p10`) | `64` | `-0.023994` | `[-0.065926, +0.017938]` | `-0.142935` | `2.5717e-1` | `3.2730e-1` | `false` |
| `H2` (`T / U`) | `S2_mean` (`trait_mass_mean`) | `64` | `+0.010545` | `[-0.001035, +0.022124]` | `+0.227460` | `7.3559e-2` | `1.1443e-1` | `false` |
| `H2` (`T / U`) | **`S3_east_minus_west`** | `64` | **`+0.020753`** | `[+0.003926, +0.037581]` | `+0.308071` | `1.6457e-2` | `3.4560e-2` | `true` |
| `H2` (`T / U`) | `raw_var_delta` (`trait_mass_var`) | `64` | `-0.001176` | `[-0.008257, +0.005906]` | `-0.041467` | `7.4119e-1` | `7.7825e-1` | `false` |
| `H2` (`T / U`) | `bessel_var_delta` (`var * N/(N-1)`) | `64` | `-0.001191` | `[-0.008315, +0.005934]` | `-0.041742` | `7.3954e-1` | `7.7825e-1` | `false` |
| `H2` (`T / U`) | `lnCVR_nonzero` (`eps=5e-4`: `p=0.93167`) | `63` | `-0.018547` | `[-0.206997, +0.169904]` | `-0.024786` | `8.4468e-1` | `8.4468e-1` | `false` |
| `H2` (`T / U`) | **`predator_count`** (covariate §1.2) | `64` | **`-2.140625`** | `[-2.799063, -1.482187]` | `-0.812093` | `1.4933e-8` | `4.4798e-8` | `true` |
| `H2` (`T / U`) | **`S4_shannon`** (trophic balance) | `64` | **`-0.051083`** | `[-0.065399, -0.036767]` | `-0.891310` | `1.1834e-9` | `4.1420e-9` | `true` |
| `H2` (`T / U`) | **`S6_births_sum`** | `64` | **`-16.6719`** | `[-26.4967, -6.8471]` | `-0.423878` | `1.2069e-3` | `3.1681e-3` | `true` |
| `H2` (`T / U`) | **`S6_deaths_sum`** | `64` | **`-23.4063`** | `[-34.0791, -12.7334]` | `-0.547812` | `4.5318e-5` | `1.2689e-4` | `true` |
| `H2` (`T / U`) | `S6_lifespan_post_warmup_pos` (`series`) | `64` | `+21.9451` | `[-31.0096, +74.8998]` | `+0.103517` | `4.1072e-1` | `4.9286e-1` | `false` |
| `H2` (`T / U`) | `S6_cumulative_mean_lifespan` | `64` | `+33.1856` | `[-15.0345, +81.4056]` | `+0.171910` | `1.7392e-1` | `2.4348e-1` | `false` |
| `H3` (`S / U`) | **`tail_window_L`** (`[14400, 18000]`) | `64` | **`-0.352970`** (`sd 1.039687`) | `[-0.612676, -0.093264]` | `d_z = -0.339496` | `8.5188e-3` | `1.9877e-2` | `true` |
| `H3` (`S / U`) | `mediator_ln_births` (OLS slope) | `64` | `+1.024556` | `R^2 = 0.005512` | `r = +0.074242` | `5.5987e-1` | `6.3553e-1` | `false` |
| `H3` (`S / U`) | **`S1_iqr`** (`p90 - p10`) | `64` | **`-0.046737`** | `[-0.081046, -0.012428]` | `-0.340274` | `8.3770e-3` | `1.9877e-2` | `true` |
| `H3` (`S / U`) | `S2_mean` (`trait_mass_mean`) | `64` | `+0.005246` | `[-0.005713, +0.016206]` | `+0.119581` | `3.4240e-1` | `4.2297e-1` | `false` |
| `H3` (`S / U`) | `S3_east_minus_west` | `64` | `+0.004767` | `[-0.010998, +0.020532]` | `+0.075530` | `5.4785e-1` | `6.3553e-1` | `false` |
| `H3` (`S / U`) | `raw_var_delta` (`trait_mass_var`) | `64` | `-0.007350` | `[-0.014500, -0.000199]` | `-0.256758` | `4.4125e-2` | `7.2123e-2` | `false` |
| `H3` (`S / U`) | `bessel_var_delta` (`var * N/(N-1)`) | `64` | `-0.007382` | `[-0.014582, -0.000182]` | `-0.256107` | `4.4647e-2` | `7.2123e-2` | `false` |
| `H3` (`S / U`) | `lnCVR_nonzero` (`eps=5e-4`: `p=0.16369`) | `62` | `-0.132232` | `[-0.345903, +0.081439]` | `-0.157160` | `2.2065e-1` | `2.8960e-1` | `false` |
| `H3` (`S / U`) | **`predator_count`** (covariate §1.2) | `64` | **`-5.250000`** | `[-5.953711, -4.546289]` | `-1.863565` | `2.4650e-22` | `1.1503e-21` | `true` |
| `H3` (`S / U`) | **`S4_shannon`** (trophic balance) | `64` | **`-0.114957`** | `[-0.134413, -0.095502]` | `-1.475949` | `1.2907e-17` | `5.4211e-17` | `true` |
| `H3` (`S / U`) | **`S6_births_sum`** | `64` | **`-178.7344`** | `[-190.2539, -167.2149]` | `-3.875737` | `7.3078e-40` | `6.1385e-39` | `true` |
| `H3` (`S / U`) | **`S6_deaths_sum`** | `64` | **`-168.4219`** | `[-179.2596, -157.5841]` | `-3.881844` | `6.6575e-40` | `6.1385e-39` | `true` |
| `H3` (`S / U`) | **`S6_lifespan_post_warmup_pos`** (`series`) | `64` | **`+622.2999`** | `[+545.8975, +698.7023]` | `+2.034569` | `3.0032e-24` | `1.5767e-23` | `true` |
| `H3` (`S / U`) | **`S6_cumulative_mean_lifespan`** | `64` | **`+775.6089`** | `[+711.7416, +839.4763]` | `+3.033497` | `1.1316e-33` | `6.7898e-33` | `true` |

*Reproduced directly from committed [`confirmation/effects.json`](confirmation/effects.json) and `final-observables.json`:*

```bash
node --input-type=module -e "import fs from 'node:fs'; import { pairedStats, studentTTwoSidedP } from './scripts/verify_e3b_artifacts.mjs'; const d='artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation'; const eff=JSON.parse(fs.readFileSync(d+'/effects.json','utf8')); const fin=JSON.parse(fs.readFileSync(d+'/final-observables.json','utf8')); const fm=new Map(fin.map(f=>[f.seed+'::'+f.arm, f.observables])); const dm=new Map(eff.arm_diagnostics.map(x=>[x.seed+'::'+x.arm, x])); const seeds=[...new Set(eff.arm_diagnostics.map(x=>x.seed))]; const tt=(ps)=>({n:ps.n_complete_pairs, delta:ps.paired_mean_delta, ci95:[ps.ci95_low,ps.ci95_high], dz:ps.paired_dz, p:studentTTwoSidedP(ps.paired_mean_delta/ps.paired_se, ps.n_complete_pairs-1)}); const tests=[]; for (const [id,a,b] of [['H1','control_uniform_scarcity','treatment_gradient'],['H2','control_uniform','treatment_gradient'],['H3','control_uniform','control_uniform_scarcity']]) { const ga=seeds.map(s=>({f:fm.get(s+'::'+a), d:dm.get(s+'::'+a)})), gb=seeds.map(s=>({f:fm.get(s+'::'+b), d:dm.get(s+'::'+b)})); const c=eff.contrasts[id], tw=c.tail_window_L, med=c.mediator_ln_births; const lnCv=(r)=>Math.log(Math.sqrt(Number(r.f['live.trait_mass_var']))/Number(r.f['live.trait_mass_mean'])); const nz=ga.map((x,i)=>[lnCv(x),lnCv(gb[i])]).filter(([u,v])=>Number.isFinite(u)&&Number.isFinite(v)); const eps={tail_window_L:{n:tw.n_complete_pairs,delta:tw.mean_L,sd:tw.sd_L,ci95:[tw.ci95_low,tw.ci95_high],dz:tw.paired_dz,p:tw.p_two_sided}, mediator_ln_births:{n:med.n,delta:med.slope,r2:med.r2,r:med.pearson_r,p:med.p_slope}, S1_iqr:tt(c.s1_iqr), S2_mean:tt(c.s2_mean), S3_east_minus_west:tt(c.s3_east_minus_west), raw_var_delta:tt(pairedStats(ga.map(x=>Number(x.f['live.trait_mass_var'])), gb.map(x=>Number(x.f['live.trait_mass_var'])))), bessel_var_delta:tt(pairedStats(ga.map(x=>Number(x.f['live.trait_mass_var'])*x.d.final_agents/(x.d.final_agents-1)), gb.map(x=>Number(x.f['live.trait_mass_var'])*x.d.final_agents/(x.d.final_agents-1)))), lnCVR_nonzero:tt(pairedStats(nz.map(z=>z[0]), nz.map(z=>z[1]), 64)), predator_count:tt(pairedStats(ga.map(x=>x.d.final_predators), gb.map(x=>x.d.final_predators))), S4_shannon:tt(pairedStats(ga.map(x=>Number(x.f['live.shannon'])), gb.map(x=>Number(x.f['live.shannon'])))), S6_births_sum:tt(pairedStats(ga.map(x=>x.d.births_sum), gb.map(x=>x.d.births_sum))), S6_deaths_sum:tt(pairedStats(ga.map(x=>x.d.deaths_sum), gb.map(x=>x.d.deaths_sum))), S6_lifespan_post_warmup_pos:tt(pairedStats(ga.map(x=>x.d.lifespan_post_warmup_pos), gb.map(x=>x.d.lifespan_post_warmup_pos))), S6_cumulative_mean_lifespan:tt(pairedStats(ga.map(x=>x.d.cumulative_mean_lifespan), gb.map(x=>x.d.cumulative_mean_lifespan)))}; for (const [k,v] of Object.entries(eps)) tests.push({id,k,...v}); } const m=tests.length, ord=tests.map((e,i)=>({...e,i})).sort((x,y)=>x.p-y.p); let pq=1; for (let r=m-1; r>=0; r--) { pq=Math.min(pq, (m/(r+1))*ord[r].p); tests[ord[r].i].q=pq; } for (const t of tests) console.log(JSON.stringify(t)); console.log('lnCVR_eps_5e4_p', ['H1','H2','H3'].map(id=>[id, tt(pairedStats(seeds.map(s=>{const o=fm.get(s+'::'+(id==='H1'?'control_uniform_scarcity':'control_uniform')); return 0.5*Math.log(Number(o['live.trait_mass_var'])+5e-4)-Math.log(Number(o['live.trait_mass_mean']));}), seeds.map(s=>{const o=fm.get(s+'::'+(id==='H3'?'control_uniform_scarcity':'treatment_gradient')); return 0.5*Math.log(Number(o['live.trait_mass_var'])+5e-4)-Math.log(Number(o['live.trait_mass_mean']));}))).p]));"
```

---

## 5. Deviations from Preregistration

- **Scientific / statistical protocol (`R1`–`R10`, hypotheses, SESOI, seeds, manifests, analysis rules):** **None.** All `64` locked seeds (`810001..=873001`) ran once on `Windows 1` (`windows/x86_64`), all manifests match committed fixtures byte-for-byte, `failed_runs_count = 0`, `max_abs_eu_drift <= 7.098424e-8 <= 1e-6`, `s6_lifespan_source = "series.csv"`, and all confirmatory labels match the frozen `R10` table.
- **Recorded operational provenance notes (already disclosed in `C2` & `C3`):**
  1. **Dual binary SHA-256 (`C2` vs `C3`):** Trunk `feat/ecological-selection` merged `PR #240`, `#236`, `#241`, and `#237` between `C2. E3B-SMOKE` (`ed634d8c`, binary `381d0890...`) and `C3. E3B-RUN` (`8d523415`, binary `645564b7...`); `run_e3b_experiment.exe` was rebuilt once at `8d523415` prior to confirmation and both commit/binary SHAs are recorded in `confirmation/provenance.json`.
  2. **`smoke-report.json` `WP-2(b2)` algebraic sign convention (`C2`):** `smoke/smoke-report.json` recorded `(S - T)/T = -0.00517545`, whereas preregistration prose wrote `(T - S)/T = +0.00517545`; both share identical magnitude (`|-0.00517545| <= 0.10`), passing `WP-2(b2)`.

---

## 6. What This Result Does NOT Show

1. **No inference across arbitrary maps, worlds, or live desktop UI execution:** `192` runs across `64` seeds under one generated terrain (`world_identity` checksum `3152323152`) inside `LiveExperimentAdapter` (`live-bevy-world/2`) measure stochastic realizations of **this single world design**, not general behavior across World v2 terrain, desktop GPU/UI scheduling, or asynchronous learner threads.
2. **`H1` (`insufficient_evidence`) neither confirms nor refutes an effect of the spatial productivity gradient on body-mass variance:** With observed `sd_L = 1.235926` vs pre-run planning benchmark `0.8109` (inflated in part by single-tick `var = 0` excursions in `U` `843001` and `S` `859001`), `N = 64` lacked power to distinguish a `+0.3016` log-variance shift (`1.352x`) from either `0` or `+ln(1.5)` (`0.4055`). Calling `H1` "proven positive" or "proven null" is invalid under `R10`.
3. **Exploratory `S3`, `S6`, and `predator_count` shifts carry zero confirmatory weight:** Although predators (`H1 +3.11`, `H3 -5.25`), demographic births/deaths (`H1 +162.1` births), east-minus-west mass gradient (`S3`: `H1 +0.01599, q = 0.0237`), and `H3` `tail_window_L` (`-0.3530, q = 0.0199`) show `q < 0.05` exploratory separation, they were pre-declared `decision_effect: none` and remain hypotheses for separate preregistered experiments (`SEL2-P2`, `LAW1`, `S7`).
4. **Short-horizon clonal de novo regime only:** Findings reflect `~2.26–3.17` realized generations from the `2`-segment clonal founder (`live.founder_genotype = 1.0`, mass `2.5`) pinned at the `mutation.rs:210-230` node-deletion wall, comparing recurring `Deforest 0.30 / AddNutrient 1.00` (`T`) against genesis `0.42` scarcity + refill (`S`) and uniform abundance (`U`).
