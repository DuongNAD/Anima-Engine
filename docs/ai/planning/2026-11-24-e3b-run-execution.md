---
title: E3b Confirmation Ensemble Execution Note (C3 E3B-RUN)
status: active
owner: anima-case
last_reviewed: 2026-11-24
description: Execution provenance, operational gate checks, closed-EU drift audit, and artifact checksums for the 192 E3b confirmation runs (body-mass analysis deferred to C4 E3B-RESULT).
---

# E3b Confirmation Ensemble Execution (`C3. E3B-RUN`)

- **Frozen preregistration:** [2026-11-24-e3b-preregistration.md](2026-11-24-e3b-preregistration.md) (`preregistration_id = e3b-productivity-gradient-body-mass`, `preregistration_status = FROZEN`).
- **Smoke prerequisite (`C2`):** [2026-11-24-e3b-smoke-results.md](2026-11-24-e3b-smoke-results.md) (`PR #240`, head `d252f37d7b35108efa6710d3299327b69bf62be7`, merged at `9eeb3130`, `wp2_passed = true`).
- **Runner specification (`C1`):** [2026-11-24-e3b-runner.md](2026-11-24-e3b-runner.md) ([run_e3b_experiment.rs](../../../src-tauri/examples/run_e3b_experiment.rs)).
- **Confirmation bundle:** [provenance.json](../../../artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation/provenance.json), [runs.csv](../../../artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation/runs.csv), [checksums.sha256](../../../artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation/checksums.sha256).

> **Blinding / Phase Separation (`C3` vs `C4`):** Per Wave card `C3. E3B-RUN` and `C4. E3B-RESULT` (*"Only now may body-mass values be read"*), this note covers execution provenance, completion, determinism, closed-EU conservation, and `npm run check:e3b` bundle integrity only. Body-mass observables (`trait_mass*`), log-variance ratios (`L_i`), Holm-adjusted `p`-values, TOST equivalence bounds, and the R10 outcome label are intentionally left unexamined and unprinted until `C4. E3B-RESULT` (`RESULT.md`) after `C3` merges.

---

## 1. Dual Binary & Commit Provenance (`C2` Smoke vs `C3` Confirmation)

Because trunk `origin/feat/ecological-selection` advanced after `C2. E3B-SMOKE` (merging `#240`, `#236`, `#241`, `#237`), the release runner binary was rebuilt once on newest trunk `8d52341549007a50ce033a6d272c5dd22a71b1b1` before executing `--ensemble`. Both commit and binary SHAs are recorded below:

| Phase / Card | Git Commit | Release Binary SHA-256 (`run_e3b_experiment.exe`) | Reference Provenance / Outcome |
|---|---|---|---|
| **Smoke (`C2. E3B-SMOKE`)** | `ed634d8c0e646d6a78ca9e079f7bb5346d2d8b93` (PR `#240` merged at `9eeb3130`) | `381d08907ce2945ccbc9a62059b15d3fbf84264263ad529c93aab9b4456435fd` | `provenance.json` SHA-256 `346210c095e632136d1fbf37832584a45a01babcdf725c956f2912d3cf563516`; `smoke-report.json` SHA-256 `003685a5b53d4b6278ea15e6c5de0ade4323fbb7447d68ac22a7c3b98d0840d6`; `wp2_passed = true` |
| **Confirmation (`C3. E3B-RUN`)** | `8d52341549007a50ce033a6d272c5dd22a71b1b1` (`git_describe: 8d52341`) | `645564b730c43bbb1e51a247e8530f5cd93b15231e1ca16824fb00e81e603413` (`50,830,336` bytes) | Executed `192` confirmation runs once on `windows`/`x86_64` (`rustc 1.98.1 (48a229cea 2026-09-01)`, `package_version: 0.1.0`) |

**Smoke `WP-2(b2)` sign-convention record (from Lead verdict):** In [smoke-report.json](../../../artifacts/experiments/e3b-productivity-gradient-body-mass/smoke/smoke-report.json), `b2` (`rel_mismatch`) is printed using `(mean_post_warmup(S) - mean_post_warmup(T)) / mean_post_warmup(T) = -0.005175446571556897` (scarcity standing crop `0.5175%` below treatment), whereas `(mean_post_warmup(T) - mean_post_warmup(S)) / mean_post_warmup(T) = +0.005175446571556897`. Because the preregistered `WP-2(b2)` gate compares `|rel_mismatch| <= 0.10`, both sign conventions are algebraically equivalent and pass cleanly.

---

## 2. Confirmation Execution Parameters & Isolation Provenance

| Field | Observed Value (`confirmation/provenance.json`) |
|---|---|
| Host & OS | Windows 1 (`windows` / `x86_64`, `DESKTOP-3EPV830`) |
| Build profile & command | `release` (`cargo build --release --features desktop --example run_e3b_experiment`, `CARGO_BUILD_JOBS=8`, `ANIMA_USE_GPU=0`) |
| Model & registry | `core::live_experiment::LiveExperimentAdapter` (`live-bevy-world/2`), `core::experiment::ObservableRegistry::live_default` (`registry_fingerprint: 1935740381389641332`) |
| `ANIMA_WORLD_ARTIFACT` | Unset (`anima_world_artifact_unset = true`) |
| `%TEMP%\anima_world_artifact.anmw` | Absent (`temp_artifact_absent = true`, `C:/Users/Admin/AppData/Local/Temp/anima_world_artifact.anmw`) |
| `terrain_source` | `"generated"` across all `192` runs |
| `world_identity` | `seed = 1337`, `generator_version = 1`, `width = 256`, `height = 256`, `checksum = 3152323152` (`world_identity_all_runs_identical = true`, matches `smoke_reference.world_identity_observed`) |
| `smoke_reference` | `smoke_dir = "artifacts/experiments/e3b-productivity-gradient-body-mass/smoke"`, `wp2_passed = true` |
| Seeds & arms | `64` seeds `810001..=873001` (`810001 + 1000*k`, `k = 0..63`) x `3` arms (`control_uniform`, `control_uniform_scarcity`, `treatment_gradient`) = `192` runs (`0` occurrences of `smoke_seed = 999953`) |
| Resumable checkpoint dir | External `%TEMP%\e3b_c3_ckpt` (cleaned after push; never inside `--out` or git working tree) |
| `stopped_early` | `false` (`stop_reason: null`) |
| `s6_lifespan_source` | `"series.csv"` (F-053 guard satisfied) |
| `build.git_dirty_paths` | `["artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation/"]` (`0` disallowed dirty paths; schema churn restored before `--ensemble`) |
| Wall window (UTC) | `2026-11-24T05:28:11Z` -> `2026-11-24T05:33:32Z` (`321.0087407` s; `jobs = 8`) |
| Per-run `elapsed_secs` (`192` runs) | `min = 10.6982` s, `median = 13.0938` s, `max = 15.5829` s, `sum = 2532.5561` s |

---

## 3. Operational Completion, Survival Flags & Closed-EU Drift Audit

Every run completed (`192/192`), emitted finite closed-EU totals (`0` non-finite), survived with non-singleton populations and populated west/east halves through `T = 18,000`, and stayed more than an order of magnitude below the `1e-6` `max_abs_eu_drift` ceiling (**`0` exceptions**):

| Arm | Runs (`Completed` / `Failed`) | `extinct` | `singleton_final` | `singleton_tail` | `empty_half` (`W`/`E`/`Both`) | `max_abs_eu_drift` Min | `max_abs_eu_drift` Median | `max_abs_eu_drift` Max | Runs `> 1e-6` |
|---|---|---|---|---|---|---|---|---|---|
| `control_uniform` | `64` / `0` | `0` | `0` | `0` | `0` / `0` / `0` | `7.275957614183e-10` | `2.823071554303e-09` | `1.044827513397e-08` | `0` |
| `control_uniform_scarcity` | `64` / `0` | `0` | `0` | `0` | `0` / `0` / `0` | `9.225914254785e-09` | `3.038439899683e-08` | `5.791662260890e-08` | `0` |
| `treatment_gradient` | `64` / `0` | `0` | `0` | `0` | `0` / `0` / `0` | `2.226443029940e-08` | `4.182220436633e-08` | `7.098424248397e-08` | `0` |
| **All arms combined** | **`192` / `0`** | **`0`** | **`0`** | **`0`** | **`0` / `0` / `0`** | **`7.275957614183e-10`** | **`2.876913640648e-08`** | **`7.098424248397e-08`** | **`0`** |

---

## 4. Runner & Verifier Command Outputs

### Confirmation Ensemble Execution (`--ensemble`)

```text
$ cmd /c "cd /d D:\teamwork_projects\anima-wt\win-e3b-run && set ANIMA_WORLD_ARTIFACT=&& set ANIMA_USE_GPU=0&& E:\.smart_drive_caches\cargo-target\release\examples\run_e3b_experiment.exe --ensemble --smoke-dir artifacts/experiments/e3b-productivity-gradient-body-mass/smoke --out artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation --checkpoint-dir %TEMP%\e3b_c3_ckpt --jobs 8 > ... 2>&1"   (EXIT=0)
E3b confirmation ensemble completed: 192 runs in 321.01s -> artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation
```

### Runner Self-Test (`--self-test`)

```text
$ E:\.smart_drive_caches\cargo-target\release\examples\run_e3b_experiment.exe --self-test   (EXIT=0)
run_e3b_experiment --self-test PASSED
```

### Full Bundle Verifier (`npm run check:e3b`)

```text
$ cmd /c "cd /d D:\teamwork_projects\anima-wt\win-e3b-run && npm run check:e3b > ... 2>&1"   (EXIT=0)

> anima-engine-frontend@0.1.0 check:e3b
> node scripts/verify_e3b_artifacts.mjs

verify_e3b_artifacts OK (D:\teamwork_projects\anima-wt\win-e3b-run\artifacts\experiments\e3b-productivity-gradient-body-mass\confirmation)
```

- Zero `NOTE:` fallback lines emitted (commit `8d52341549007a50ce033a6d272c5dd22a71b1b1` resolved directly from git; `manifests/*` verified byte-for-byte against committed fixtures; `smoke_reference` verified against `artifacts/experiments/e3b-productivity-gradient-body-mass/smoke`; `runs.csv`, `series.csv`, `final-observables.json`, and `effects.json` independently recomputed and matched).

### Verifier Self-Test (`node scripts/verify_e3b_artifacts.mjs --self-test`)

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

### Isolated Negative-Control Tamper Check (A3 #2)

On an isolated copy in `%TEMP%\e3b_c3_negctrl\confirmation`, flipping `1` byte in `runs.csv` caused `node scripts/verify_e3b_artifacts.mjs --dir %TEMP%\e3b_c3_negctrl\confirmation` to exit `1` and fail immediately with `confirmation: SHA-256 mismatch for runs.csv` (the temporary copy was then deleted cleanly).

---

## 5. Committed Artifact Inventory (`confirmation/`)

| Relative Path | Size (bytes) | SHA-256 ([checksums.sha256](../../../artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation/checksums.sha256)) |
|---|---|---|
| `effects.json` | `151,769` | `f9d5baf4658d04dba8bc6dec5db4c7d7c4b4e6a2adee6acbb14cbd4ff294ea4b` |
| `final-observables.json` | `283,849` | `3e530155e0270f31ff16367dd6f7a25b6a0246531cc43de2e43c7542e83a9b56` |
| `manifests/e3b-control-uniform-scarcity.json` | `4,189` | `958fc7cfbb2e987420369daff014e94dc5af129747884bbec2216697c51ddba1` |
| `manifests/e3b-control-uniform.json` | `4,154` | `520ce6c635e2edc773cc14a9305bb89c168f5be0d9bad8108393094962f85906` |
| `manifests/e3b-preregistration.json` | `17,282` | `55fd16b9ca1bb1c5f1ad54da4ac8c9ad00bae9cec0fa29626237f3786b7aa5d6` |
| `manifests/e3b-treatment-gradient.json` | `4,162` | `b3ae17a01543802ae26e36c9e94876fc211abb15fd2418e07b6311c82446a4f9` |
| `provenance.json` | `3,876` | `aac2246b3106174c2531ce89422e3a8a2e3649a56f87356fc60c239e0fc40aba` |
| `runs.csv` | `17,731` | `205117d74386367493be553c2aefbd7ddb9dd797666a54009068467824666eee` |
| `series.csv` | `989,702` | `1aed1addb9fcac6259e808f4261bc31b3a19e385d09aee6431619d8ef871f536` |
| `checksums.sha256` | `818` | Checksum manifest covering the `9` files above |

---

## 6. Peer Verification Guide (`anima-dell`)

1. Verify the complete bundle (confirmation + `smoke_reference` + independent R1–R10 statistical recomputation):

   ```bash
   npm run check:e3b
   node scripts/verify_e3b_artifacts.mjs --self-test
   ```

2. Build the release runner on `win/e3b-run` and reproduce any **three seeds** of `anima-dell`'s choice (across `control_uniform`, `control_uniform_scarcity`, `treatment_gradient`), verifying identical `final_checksum` and `world_identity` against [runs.csv](../../../artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation/runs.csv):

   ```cmd
   cd src-tauri && cargo build --release --features desktop --example run_e3b_experiment
   set ANIMA_WORLD_ARTIFACT=&& set ANIMA_USE_GPU=0&& target\release\examples\run_e3b_experiment.exe --replay --arm <arm> --seed <seed>
   ```
