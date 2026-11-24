---
title: E3b Blind Smoke Results (Seed 999953, C2. E3B-SMOKE)
status: active
owner: anima-case
last_reviewed: 2026-11-24
description: Blind WP-2 smoke verification for Experiment E3b on seed 999953 (control_uniform, control_uniform_scarcity, treatment_gradient) on Windows 1 at trunk ed634d8c with zero body-mass inspection.
---

# E3b Blind Smoke Results (`C2. E3B-SMOKE`)

- **Frozen preregistration:** [2026-11-24-e3b-preregistration.md](2026-11-24-e3b-preregistration.md) (`preregistration_id = e3b-productivity-gradient-body-mass`, `preregistration_status = FROZEN`).
- **Runner specification (`C1`):** [2026-11-24-e3b-runner.md](2026-11-24-e3b-runner.md) ([run_e3b_experiment.rs](../../../src-tauri/examples/run_e3b_experiment.rs)).
- **Overall outcome:** **`wp2_passed = true`** across all four pre-registered smoke gates (`WP-2(a)`, `WP-2(b1)`, `WP-2(b2)`, `WP-2(c)`) on blind smoke seed `999953`, with `body_mass_consulted = false` and `0` `trait_mass` substrings anywhere under [artifacts/experiments/e3b-productivity-gradient-body-mass/smoke/](../../../artifacts/experiments/e3b-productivity-gradient-body-mass/smoke/smoke-report.json).

---

## 1. Execution Provenance & Isolation Checks

| Field | Value |
|---|---|
| Execution host | Windows 1 (`windows` / `x86_64`) |
| Pinned trunk commit | `ed634d8c0e646d6a78ca9e079f7bb5346d2d8b93` (contains PR `#231` `ebe636da` & PR `#234` `6488b410`) |
| Build profile & command | `--release --features desktop --example run_e3b_experiment` (`CARGO_BUILD_JOBS=8`, `ANIMA_USE_GPU=0`) |
| Compiler / package | `rustc 1.98.1 (48a229cea 2026-09-01)` / `package_version = 0.1.0` |
| Runner binary SHA-256 | `381d08907ce2945ccbc9a62059b15d3fbf84264263ad529c93aab9b4456435fd` |
| `ANIMA_WORLD_ARTIFACT` | Unset (`anima_world_artifact_unset = true`) |
| `%TEMP%\anima_world_artifact.anmw` | Absent (`temp_artifact_absent = true`, checked `C:/Users/Admin/AppData/Local/Temp/anima_world_artifact.anmw`) |
| `terrain_source` | `"generated"` (`MapSettings::default()`) |
| Observed `world_identity` | `seed = 1337`, `generator_version = 1`, `width = 256`, `height = 256`, `checksum = 3152323152` (informational) |
| Schedule & seed | `smoke_seed = 999953`, `duration_ticks = 18000`, `warmup_ticks = 3000`, `sample_period = 600` (`30` samples/arm) |
| Wall window (UTC) | `2026-11-24T00:38:29Z` -> `2026-11-24T00:39:07Z` (`38.4740051` s) |
| `build.git_dirty_paths` | `["artifacts/experiments/e3b-productivity-gradient-body-mass/"]` (`git_tree_dirty = true`) |

Note on `build.git_dirty_paths`: Before launching `--smoke`, the worktree was clean (`0` dirty or untracked paths, after restoring CRLF churn in `src-tauri/gen/schemas/`). During `--smoke`, `run_e3b_experiment` writes manifests and series under `--out` prior to invoking `git -c status.relativePaths=false status --porcelain`, so `provenance.json` records only the newly written artifact folder `artifacts/experiments/e3b-productivity-gradient-body-mass/` (an allowed path in `isAllowedDirtyPath` inside [verify_e3b_artifacts.mjs](../../../scripts/verify_e3b_artifacts.mjs)).

---

## 2. Raw Runner Command & Output

```text
$ cmd /c "cd /d D:\teamwork_projects\anima-wt\win-e3b-smoke && set ANIMA_WORLD_ARTIFACT=&& set ANIMA_USE_GPU=0&& E:\.smart_drive_caches\cargo-target\release\examples\run_e3b_experiment.exe --smoke --out artifacts/experiments/e3b-productivity-gradient-body-mass/smoke > ... 2>&1"   (EXIT=0)
E3b blind smoke seed 999953: wp2_passed=true (a=true, b1=true, b2=true, c=true) -> artifacts/experiments/e3b-productivity-gradient-body-mass/smoke
```

Runner self-test on the same binary:

```text
$ E:\.smart_drive_caches\cargo-target\release\examples\run_e3b_experiment.exe --self-test   (EXIT=0)
run_e3b_experiment --self-test PASSED
```

---

## 3. Preregistered `WP-2` Gate Evaluation (`seed = 999953`)

| Gate | Preregistered Criterion (`§4`) | Observed Values (`smoke-report.json` + independent `smoke-series.csv` recomputation) | Verdict |
|---|---|---|---|
| **`WP-2(a)`** | `256x256` `ResourceField`; identical `153,587.25` EU initial `ResourceField` capacity across `U, S, T` (with identical genesis RNG stream & `world_identity`; `S` scaled to `0.42`) | `width = 256`, `height = 256`; `world_identity_identical = true`, `rng_stream_identical = true`; `initial_closed_eu_{U, S, T} = 153587.24986362457` EU (plus `16,000` EU initial founder biomass = `169,587.24986362457` EU closed total across `smoke-series.csv`); `uniform_capacity_eu = 153587.24986362457`, `scarcity_capacity_eu = 64506.642210796475`, `capacity_ratio = 0.4199999822125479` | **PASS** (`true`) |
| **`WP-2(b1)`** | `\|(mean_post_warmup(T) - mean_post_warmup(U)) / mean_post_warmup(U)\| >= 0.10`; `T` executes scheduled events (`Deforest > 0`, `AddNutrient > 0`); distinct `final_checksum` | `(64047.13821968107 - 124673.58954909166) / 124673.58954909166 = -0.4862814293602915` (`\|rel_shift\| = 0.4862814293602915 >= 0.10`); `treatment_deforest_events = 300`, `treatment_add_nutrient_events = 300`; checksums `U = 4154339718` vs `T = 2934444428` | **PASS** (`true`) |
| **`WP-2(b2)`** | `\|(mean_post_warmup(S) - mean_post_warmup(T)) / mean_post_warmup(T)\| <= 0.10`; `0` `Deforest` events in `S`; `AddNutrient > 0` in `S`; distinct `final_checksum` | `(63715.66567776399 - 64047.13821968107) / 64047.13821968107 = -0.005175446571556897` (`\|rel_mismatch\| = 0.005175446571556897 <= 0.10`); `scarcity_deforest_events = 0`, `scarcity_add_nutrient_events = 300`; checksums `S = 3415484089` vs `T = 2934444428` | **PASS** (`true`) |
| **`WP-2(c)`** | `detritus > 0` before tick `3,000` (`tick = 600`) AND at `tick = 3,000` in all `3` arms | `tick 600`: `U = 2546.9743583446975`, `S = 90593.72165071477`, `T = 86836.9112282136`; `tick 3000`: `U = 11584.075412276323`, `S = 92758.99763079468`, `T = 93191.46631665426` (all `> 0`) | **PASS** (`true`) |

### Per-Arm Blind Smoke Metrics (`seed = 999953`)

| Arm | Status | `final_checksum` | `max_abs_eu_drift` (`<= 1e-6`) | `standing_crop` post-warmup mean (`ticks 3600..18000`) | `standing_crop` final | `detritus` (`600` / `3000` / `18000`) | `agent_count` final (`min` across samples) | `predator_count` final (`min` across samples) | `births` / `deaths` sample sums |
|---|---|---|---|---|---|---|---|---|---|
| `control_uniform` (`U`) | `Completed` | `4154339718` | `1.8917489796876907e-9` | `124673.58954909166` | `107714.67657537013` | `2546.97` / `11584.08` / `56371.56` | `165.0` (`165.0`) | `5.0` (`5.0`) | `570.0` / `565.0` |
| `control_uniform_scarcity` (`S`) | `Completed` | `3415484089` | `4.307366907596588e-8` | `63715.66567776399` | `63690.741382559994` | `90593.72` / `92759.00` / `101688.04` | `157.0` (`132.0`) | `4.0` (`4.0`) | `367.0` / `370.0` |
| `treatment_gradient` (`T`) | `Completed` | `2934444428` | `4.5547494664788246e-8` | `64047.13821968107` | `63958.5336730327` | `86836.91` / `93191.47` / `98119.67` | `164.0` (`164.0`) | `4.0` (`4.0`) | `479.0` / `475.0` |

Exact `smoke-series.csv` detritus values are recorded in the `WP-2(c)` table above (`smoke-report.json`); summary table rounds `detritus` to two decimal places for readability only.

---

## 4. Blinding Verification (`QD-VIAB`)

- `body_mass_consulted: false` in both [smoke-report.json](../../../artifacts/experiments/e3b-productivity-gradient-body-mass/smoke/smoke-report.json) and [provenance.json](../../../artifacts/experiments/e3b-productivity-gradient-body-mass/smoke/provenance.json).
- `smoke-series.csv` (`90` rows = `3` arms x `30` ticks) contains `25` columns and omits every `trait_mass` observable:
  `seed, arm, tick, plants, detritus, live.animals_eu, live.dormant_eu, live.closed_eu_total, live.agent_count, live.herbivore_count, live.predator_count, live.food_items, live.standing_crop, live.mean_agent_energy, live.season_phase, live.shannon, live.simpson, live.births, live.deaths, live.mean_lifespan, live.archive_coverage, live.deaths_starvation, live.deaths_dehydration, live.mean_energy_integral, live.cumulative_mean_lifespan`.
- Recursive case-insensitive substring check across all `7` files in the smoke directory:

```text
$ cmd /c "findstr /s /i trait_mass D:\teamwork_projects\anima-wt\win-e3b-smoke\artifacts\experiments\e3b-productivity-gradient-body-mass\smoke\*"
(empty output, FINDSTR_EXIT=1 — 0 matches)
```

---

## 5. Smoke Artifact Inventory & Checksums

| File | Bytes | SHA-256 ([checksums.sha256](../../../artifacts/experiments/e3b-productivity-gradient-body-mass/smoke/checksums.sha256)) |
|---|---|---|
| `manifests/e3b-smoke-control-uniform-scarcity.json` | `3234` | `54ee98400200c51f6b87926b3a71c5dce009df1729fea47c4941cd70dddf07bb` |
| `manifests/e3b-smoke-control-uniform.json` | `3201` | `85035550454aac3245167c4d81efb14dfe50aa1180eba10662402b4cba77c856` |
| `manifests/e3b-smoke-treatment-gradient.json` | `3205` | `d0d50712b55b80a54dbcb0fa8f597c97a35ceed63cb731413260058884b40af5` |
| `provenance.json` | `1210` | `346210c095e632136d1fbf37832584a45a01babcdf725c956f2912d3cf563516` |
| `smoke-report.json` | `2795` | `003685a5b53d4b6278ea15e6c5de0ade4323fbb7447d68ac22a7c3b98d0840d6` |
| `smoke-series.csv` | `25114` | `34db8098c85f2dd1dbbd119f1a770b6951b073a5d8963ad9d69c6acf743956e2` |
| `checksums.sha256` (manifest file itself) | `582` | `291178f0ad80d64d227686c9985c6dd1e51f5db955f5af4b3f11597ff7bf96ba` |

Independent SHA-256 verification passed `6/6`, LF-normalized manifest identity against `src-tauri/tests/fixtures/experiments_e3b/` passed `3/3`, and independent Python recomputation of `WP-2(b1)`, `WP-2(b2)`, `WP-2(c)`, and EU conservation span matched `smoke-report.json`.

---

## 6. Verifier Output (`npm run check:e3b` & Self-Test)

```text
> anima-engine-frontend@0.1.0 check:e3b
> node scripts/verify_e3b_artifacts.mjs

check:e3b — no confirmation artifacts at artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation yet (pre-E3B-run ok)
```

```text
$ node scripts/verify_e3b_artifacts.mjs --self-test   (EXIT=0)
self-test ok    studentTInv(0.975, 63) ~ 1.998341 (got 1.998341)
self-test ok    studentTInv(0.975, 11) ~ 2.200985
self-test ok    evaluatePairedLogVarianceRatio positive ratio
self-test ok    hodgesLehmannBcaCi bracket
self-test ok    Lee bounds width === 0 when nA == nB
self-test ok    Lee bounds asymmetric trimming width > 0
self-test ok    R10 exhaustive 48/48 discrete partition (1/1/2/44)
self-test ok    clean synthetic N=64 bundle passes ()
self-test ok    negative control: trait_mass in smoke fails
self-test ok    negative control: tampered decision fails
self-test ok    negative control: checksum mismatch fails
verify_e3b_artifacts --self-test PASSED
```

---

## 7. What This Result Does NOT Show

1. **Zero trait-mass evidence:** All smoke manifests structurally omit the `6` `live.trait_mass_*` observables and `body_mass_consulted` is `false`. This result does not reveal any body-mass variance, ratio, mean, or evolutionary trend.
2. **No confirmatory test (`H1`, `H2`, `H3`):** Smoke evaluates only the `WP-2` world-physics and intervention-equivalence gates on a single dedicated seed (`999953`, disjoint from confirmation seeds `810001..=873001`).
3. **No statistical power or extinction generalization:** A single seed triple passing `WP-2` does not estimate confirmation attrition (`n_LL`) or `L_i` variance across `N = 64` seeds.
4. **No confirmation authorization yet:** `C3. E3B-RUN` (`192` confirmation runs) must NOT start until peer review (`anima-dell`) passes and the `win/e3b-smoke` PR is merged by the owner.

---

## 8. Windows Reproduction Command (`anima-dell` Peer Review)

From a clean checkout of `win/e3b-smoke` on Windows (`x86_64`), with `ANIMA_WORLD_ARTIFACT` unset and `%TEMP%\anima_world_artifact.anmw` absent:

```bat
cargo build --release --features desktop --example run_e3b_experiment
target\release\examples\run_e3b_experiment.exe --smoke --out "%TEMP%\e3b_smoke_repro"
findstr /s /i trait_mass "%TEMP%\e3b_smoke_repro\*"
```

`WP-2` gates, `final_checksum`, `standing_crop_post_warmup_mean`, and `detritus` trajectories on Windows `x86_64` should match [smoke-report.json](../../../artifacts/experiments/e3b-productivity-gradient-body-mass/smoke/smoke-report.json).
