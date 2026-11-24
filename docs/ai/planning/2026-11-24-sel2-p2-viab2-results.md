---
phase: planning
feature: experiment-sel2-food-scarcity
title: Card C5b (SEL2P2-VIAB2) — Blind 200,000-Tick 2-Segment Viability Pre-Check Results
description: Reports blind 200,000-tick VIAB-2 survival and gate results across seeds 880001 through 880012 under QD-SEL2P2-D1(a) and D2(a), passing all 4 arms 12/12 with zero body-mass inspection and no D2(b) escalation.
status: completed
owner: anima-case
last_reviewed: 2026-11-24
---

# Card `C5b. SEL2P2-VIAB2` — Blind `200,000`-Tick Viability Pre-Check Results (`live.founder_genotype = 1.0`)

**Executed once under strict `QD-VIAB` body-mass blinding on `Windows 1` (`windows/x86_64`) using exploratory calibration seeds `880001..=880012` (disjoint from all `SEL2-P2` confirmation, baseline, and smoke seeds).**
All `48` runs (`12` seeds × `4` arms) survived to `T = 200,000` ticks (`12/12` in `baseline_ref`, `selected`, `scarce_mut0`, and `scarce_drift`), clearing **`QD-SEL2P2-D1=(a)`** (`>= 11/12` survival in every scarce arm) without triggering the pre-approved **`QD-SEL2P2-D2=(b)`** (`founder_layout = 2`) fallback.

> **Model status, stated the only way it may be stated: headless adapter verified.** No desktop app,
> Tauri handle, window, GPU device, renderer, learner thread, or evolution thread existed in the process that
> produced these numbers. Nothing here may be quoted as `"the live world is experiment-ready"`.
>
> **Strict blinding (`mass_observables_excluded = true`):** `run_viab2_probe` requests ONLY non-mass survival,
> demographic, crop, lifespan, cause-of-death, and EU-conservation observables (`8` IDs on standard arms,
> `6` IDs on `scarce_drift`) via [`LiveExperimentAdapter`](../../../src-tauri/src/core/live_experiment.rs).
> Zero `trait_mass*` or `body_mass` values were computed, printed, logged, or committed (`cmd /c 'findstr /i "trait_mass body_mass" artifacts\experiments\sel2-p2\viab2\*'` exits `1`, `0` matches), and
> [`viab2_probe_tests.rs`](../../../src-tauri/tests/viab2_probe_tests.rs) enforces that injected
> `trait_mass_mean` JSON or CSV content causes immediate failure.

---

## 1. Provenance, Authorized Scope & Exact Execution Command

| Field | Value (`viab2_fg1_s075_k2k_fl0`) |
|---|---|
| Governing authorizations | `MSG_20261124_080000` (`QD-SEL2P2-D1..D7`, card `C5b. SEL2P2-VIAB2`) + `MSG_20261124_084500` (blind probe & test) |
| Preregistration draft updated | [`2026-11-24-sel2-p2-preregistration-draft.md`](2026-11-24-sel2-p2-preregistration-draft.md) (`status: proposed`; `QD-SEL2P2-D1..D7` locked) |
| Execution commit & clean tree | `ad8045803b285d6d9ec3037c057b2c6059fe1344` (`git_tree_dirty: false`) |
| Trunk merged afterward (`#247/#245/#246/#248`) | `590aa03284bd8c13c5ec93dd7d5f355e015c592b` (`run_viab2_probe.rs` & engine simulation paths unchanged relative to `ad804580`) |
| Probe source & test suite | [`src-tauri/examples/run_viab2_probe.rs`](../../../src-tauri/examples/run_viab2_probe.rs), [`src-tauri/tests/viab2_probe_tests.rs`](../../../src-tauri/tests/viab2_probe_tests.rs) (`uses run_live_drift_pair`) |
| Release binary path & SHA-256 | `E:\.smart_drive_caches\cargo-target\release\examples\run_viab2_probe.exe` (`2f6f3b1d7d57c70f23792aa207507d59de5a0cdd7441302214606564d70503ff`) |
| Host & toolchain | `Windows 1` (`windows/x86_64`), `rustc 1.98.1 (48a229cea 2026-09-01)`, `ANIMA_USE_GPU=0`, `--jobs 8` |
| World & artifact isolation | `ANIMA_WORLD_ARTIFACT` unset (`preflight_world_artifact_unset: true`), `%TEMP%\anima_world_artifact.anmw` absent (`preflight_temp_artifact_absent: true`), `all_terrain_generated: true` (`seed = 1337`, `generator_version = 1`, `256x256`, `checksum = 3152323152`) |
| Execution UTC & wall clock | Completed `2026-11-24T09:03:59Z`; cell runtime `1001.7407205 s` (`16.70 min`, `48` runs: `12` seeds × `4` arms, `200,000` ticks, `sample_period = 50`, `drift_reset_ticks = 2000`) |

**Exact command executed to produce the committed `artifacts/experiments/sel2-p2/viab2/` bundle:**

```bash
cargo run --manifest-path src-tauri/Cargo.toml --release --features desktop --example run_viab2_probe -- --layout 0 --seeds 880001..=880012 --duration-ticks 200000 --sample-period 50 --drift-reset-ticks 2000 --min-surviving-baseline 6 --jobs 8 --checkpoint-dir "%TEMP%\viab2_ckpt" --out artifacts\experiments\sel2-p2\viab2
```

**Committed artifact SHA-256 inventory ([`checksums.sha256`](../../../artifacts/experiments/sel2-p2/viab2/checksums.sha256)):**

| File | SHA-256 (`lowercase hex`) |
|---|---|
| [`provenance.json`](../../../artifacts/experiments/sel2-p2/viab2/provenance.json) | `009f531f728d78fa8c9a339fb29038eeafae441b9a88475617c36c126363f20d` |
| [`summary.csv`](../../../artifacts/experiments/sel2-p2/viab2/summary.csv) | `4a14c4fc16c6b7c29b78fca4546fd38f81c9b574659961fed9d981506b002b1d` |
| [`viab2_fg1_s075_k2k_fl0.json`](../../../artifacts/experiments/sel2-p2/viab2/viab2_fg1_s075_k2k_fl0.json) | `aed68b80ea60757d58abf5e83e36dc0c9aa84e24ed9b5f63d561939517be0958` |

---

## 2. Gate Decision & Aggregate Summary (`QD-SEL2P2-D1=(a)` & `D2=(a)`)

| Criterion / Metric | Observed Result (`fl = 0, s = 0.75, k = 2000, fg = 1`) | Threshold / Rule | Status |
|---|---|---|---|
| **Pooled `baseline_ref` (`QD-SEL2g`)** | **`12 / 12` survived**, `min_N = 80..80`, `final_N = 80..80`, `pooled_crop_second_half = 125,402.343700` | `>= 6 / 12` surviving (`D7(a)`) | **PASS** |
| **`selected` survival (`T = 200,000`)** | **`12 / 12` (`100%`)**, `min_N` `[63, 109]` (`12/12 >= 10`), `mean_N` med `136.65`, `gen` med `32.92` `[31.46, 44.79]` | `>= 11 / 12` (`D1(a)`), `min_N >= 5` (`C1`) | **PASS** |
| **`scarce_mut0` survival (`T = 200,000`)** | **`12 / 12` (`100%`)**, `min_N` `[59, 107]` (`12/12 >= 10`), `mean_N` med `130.96`, `gen` med `32.65` `[32.08, 44.89]` | `>= 11 / 12` (`D1(a)`), `min_N >= 5` (`C1`) | **PASS** |
| **`scarce_drift` survival (`T = 200,000`)** | **`12 / 12` (`100%`)**, `min_N` `[63, 109]` (`12/12 >= 10`), `mean_N` med `136.65`, `gen` med `32.92` `[30.63, 44.79]` | `>= 11 / 12` (`D1(a)`), `min_N >= 10` | **PASS** |
| **`C2` pasture ratio (`g_ratio in [0.40, 0.60]`)** | `selected`: **`11 / 12`** (med `0.5379`, range `[0.3709, 0.5658]`; sole miss: seed `880004` at `0.370941`)<br>`scarce_mut0`: **`12 / 12`** (med `0.5542`, range `[0.4217, 0.5884]`)<br>`both`: **`11 / 12`** | Pre-approved `D2(b)` (`fl=2`) escalation check (`no shortfall`) | **PASS (`no D2(b) escalation`)** |
| **`C3` generations (`>= 30`), `C4` starv. (`> 0.50`), `C5` active** | `C3 = 12/12`, `C4 = 12/12` (`selected` `[0.6267, 0.6974]`, med `0.6530`), `C5 = 12/12` | Smoke gate criteria `C3..C5` | **PASS (`12 / 12`)** |
| **`drift_valid` & `has_shortfall == false`** | **`12 / 12` valid**, `0` missed deaths, `0` missed births (`100` windows per seed) | `drift_valid == true`, `0` shortfalls | **PASS (`12 / 12`)** |
| **Closed-EU conservation (`I1 <= 1.0e-6 EU`)** | `48 / 48` runs `<= 1.0e-6`; overall max **`8.559437e-8`** (seed `880006` `scarce_mut0`) | `<= 1.0e-6` everywhere | **PASS (`48 / 48`)** |
| **Blinding (`trait_mass` absent)** | `mass_observables_excluded = true`, `0` substrings in `artifacts/experiments/sel2-p2/viab2/*` | Zero body-mass computation or inspection | **PASS (`findstr` exit `1`)** |

**Why `QD-SEL2P2-D2=(b)` (`founder_layout = 2`) was NOT triggered (`d2b_escalation_recommended = false`):**
Under `fl = 0`, `min_N` stayed well above threshold in every run (`>= 59` across all `48` runs, lowest `63` in `selected`/`scarce_drift` at seed `880008` and `59` in `scarce_mut0` at seed `880001`; `min_n_shortfall = false`), and `C2` passed in `12/12` `scarce_mut0` and `11/12` `selected` seeds (matching `fl = 2`'s `VIAB-1` `11/12` `C2` rate while retaining `12/12` `baseline_ref` survival vs. `fl = 2`'s `9/12` and avoiding `~5x–8x` wall-time slowdown; `c2_shortfall = false`). Seed `880004` `selected` reached high mean abundance (`mean_N = 179.224`, `min_N = 109`), grazing `g_ratio = 0.370941` (`< 0.40`), while its paired `scarce_mut0` stayed inside `C2` (`0.440404`).

---

## 3. Complete Per-Seed × Per-Arm Results (`880001..=880012`, `summary.csv`)

Every value below is taken verbatim from [`artifacts/experiments/sel2-p2/viab2/summary.csv`](../../../artifacts/experiments/sel2-p2/viab2/summary.csv) and [`viab2_fg1_s075_k2k_fl0.json`](../../../artifacts/experiments/sel2-p2/viab2/viab2_fg1_s075_k2k_fl0.json):

### 3.1. `selected` (`mut = 0.05`) and `scarce_mut0` (`mut = 0.0`)

| Seed | `selected` Surv | `min_N` / `mean_N` / `final_N` | `g_ratio` (`C2`) | `gen` / `starv` (`C3/C4/C5`) | `max_\|EU_drift\|` | `scarce_mut0` Surv | `min_N` / `mean_N` / `final_N` | `g_ratio` (`C2`) | `gen` / `starv` | `max_\|EU_drift\|` |
|---:|:--:|---|---:|---|---:|:--:|---|---:|---|---:|
| `880001` | `true` | `77` / `135.363` / `159` | `0.533547` (`true`) | `31.464` / `0.627381` (`true/true/true`) | `6.431947e-8` | `true` | `59` / `127.426` / `128` | `0.572588` (`true`) | `32.168` / `0.637927` | `8.067582e-8` |
| `880002` | `true` | `85` / `129.013` / `117` | `0.553429` (`true`) | `32.121` / `0.636540` (`true/true/true`) | `6.586197e-8` | `true` | `70` / `128.115` / `120` | `0.576423` (`true`) | `32.650` / `0.640163` | `7.179915e-8` |
| `880003` | `true` | `76` / `148.460` / `129` | `0.431794` (`true`) | `44.793` / `0.696118` (`true/true/true`) | `5.544280e-8` | `true` | `95` / `151.410` / `123` | `0.421705` (`true`) | `44.891` / `0.694715` | `6.266055e-8` |
| `880004` | `true` | `109` / `179.224` / `172` | `0.370941` (`false`) | `40.742` / `0.687690` (`true/true/true`) | `5.087350e-8` | `true` | `92` / `146.690` / `130` | `0.440404` (`true`) | `44.073` / `0.689510` | `6.068149e-8` |
| `880005` | `true` | `72` / `133.701` / `128` | `0.556913` (`true`) | `33.051` / `0.644500` (`true/true/true`) | `6.507616e-8` | `true` | `72` / `126.132` / `153` | `0.573254` (`true`) | `32.434` / `0.639673` | `6.437767e-8` |
| `880006` | `true` | `98` / `154.877` / `197` | `0.463226` (`true`) | `35.028` / `0.626689` (`true/true/true`) | `6.644404e-8` | `true` | `89` / `142.674` / `124` | `0.490624` (`true`) | `36.194` / `0.637354` | `8.559437e-8` |
| `880007` | `true` | `86` / `152.542` / `169` | `0.431977` (`true`) | `44.473` / `0.697394` (`true/true/true`) | `6.068149e-8` | `true` | `86` / `150.607` / `125` | `0.426359` (`true`) | `44.241` / `0.698554` | `6.658956e-8` |
| `880008` | `true` | `63` / `130.552` / `139` | `0.559028` (`true`) | `31.934` / `0.659564` (`true/true/true`) | `7.342896e-8` | `true` | `70` / `129.877` / `102` | `0.557633` (`true`) | `32.354` / `0.640238` | `5.678157e-8` |
| `880009` | `true` | `78` / `130.335` / `121` | `0.565836` (`true`) | `32.785` / `0.651458` (`true/true/true`) | `5.395850e-8` | `true` | `66` / `122.317` / `141` | `0.588421` (`true`) | `32.081` / `0.640742` | `7.177005e-8` |
| `880010` | `true` | `77` / `130.713` / `149` | `0.553299` (`true`) | `31.887` / `0.638262` (`true/true/true`) | `6.111804e-8` | `true` | `89` / `132.049` / `121` | `0.550786` (`true`) | `32.655` / `0.659287` | `6.848131e-8` |
| `880011` | `true` | `101` / `155.210` / `164` | `0.439911` (`true`) | `40.564` / `0.682606` (`true/true/true`) | `5.794573e-8` | `true` | `107` / `153.298` / `129` | `0.444516` (`true`) | `41.266` / `0.700651` | `5.468610e-8` |
| `880012` | `true` | `65` / `137.945` / `181` | `0.542310` (`true`) | `32.063` / `0.654537` (`true/true/true`) | `7.351628e-8` | `true` | `82` / `128.775` / `131` | `0.559889` (`true`) | `32.234` / `0.649272` | `7.095514e-8` |

*(In `scarce_mut0`, `C1 = C2 = C3 = C4 = C5 = true` for all `12/12` seeds; in `selected`, `C1 = C3 = C4 = C5 = true` for `12/12` seeds and `C2 = true` for `11/12` seeds.)*

### 3.2. `scarce_drift` (`k = 2000`, Lockstep Pair) and `baseline_ref` (`QD-SEL2g`)

| Seed | `scarce_drift` Surv | `min_N` / `mean_N` / `final_N` | `g_ratio` | `gen` | `drift_valid` (`shortfall`) | `max_\|EU_drift\|` | `baseline_ref` Surv | `min_N` / `mean_N` / `final_N` | `crop_second_half` | `gen` / `starv` | `max_\|EU_drift\|` |
|---:|:--:|---|---:|---:|:--:|---:|:--:|---|---:|---|---:|
| `880001` | `true` | `77` / `135.363` / `159` | `0.535558` | `30.634` | `true` (`false`) | `6.583286e-8` | `true` | `80` / `80.537` / `80` | `126416.367600` | `13.025` / `0.269411` | `3.521563e-9` |
| `880002` | `true` | `85` / `129.013` / `117` | `0.555340` | `32.121` | `true` (`false`) | `6.667688e-8` | `true` | `80` / `80.582` / `80` | `125717.552658` | `13.601` / `0.241935` | `2.764864e-9` |
| `880003` | `true` | `76` / `148.460` / `129` | `0.434540` | `44.793` | `true` (`false`) | `5.506445e-8` | `true` | `80` / `80.503` / `80` | `121474.280865` | `16.757` / `0.197955` | `7.130438e-9` |
| `880004` | `true` | `109` / `179.224` / `172` | `0.373647` | `40.742` | `true` (`false`) | `4.944741e-8` | `true` | `80` / `80.498` / `80` | `121208.740939` | `16.174` / `0.192890` | `4.307367e-9` |
| `880005` | `true` | `72` / `133.701` / `128` | `0.558859` | `33.051` | `true` (`false`) | `6.536720e-8` | `true` | `80` / `80.572` / `80` | `127670.994735` | `12.746` / `0.253104` | `2.124580e-9` |
| `880006` | `true` | `98` / `154.877` / `197` | `0.465590` | `35.028` | `true` (`false`) | `6.682239e-8` | `true` | `80` / `80.561` / `80` | `125851.999944` | `13.741` / `0.224490` | `3.899913e-9` |
| `880007` | `true` | `86` / `152.542` / `169` | `0.434783` | `44.473` | `true` (`false`) | `6.059418e-8` | `true` | `80` / `80.515` / `80` | `122056.580208` | `15.587` / `0.165490` | `3.929017e-9` |
| `880008` | `true` | `63` / `130.552` / `139` | `0.560802` | `31.934` | `true` (`false`) | `7.217750e-8` | `true` | `80` / `80.687` / `80` | `126967.499829` | `12.294` / `0.232213` | `5.180482e-9` |
| `880009` | `true` | `78` / `130.335` / `121` | `0.567593` | `32.785` | `true` (`false`) | `5.390029e-8` | `true` | `80` / `80.566` / `80` | `127063.996765` | `12.636` / `0.239884` | `6.897608e-9` |
| `880010` | `true` | `77` / `130.713` / `149` | `0.555267` | `31.887` | `true` (`false`) | `6.257324e-8` | `true` | `80` / `80.558` / `80` | `127274.229267` | `12.823` / `0.305793` | `1.775334e-9` |
| `880011` | `true` | `101` / `155.210` / `164` | `0.442614` | `40.564` | `true` (`false`) | `5.689799e-8` | `true` | `80` / `80.496` / `80` | `125849.489744` | `13.019` / `0.166667` | `1.804437e-9` |
| `880012` | `true` | `65` / `137.945` / `181` | `0.544034` | `32.063` | `true` (`false`) | `7.270137e-8` | `true` | `80` / `80.564` / `80` | `127276.391849` | `14.609` / `0.350877` | `2.910383e-9` |

*(In `scarce_drift`, `C1 = true` for `12/12` seeds, `missed_deaths = 0` and `missed_births_* = 0` across all `100` cohort-reset windows per seed.)*

---

## 4. What This Result Does NOT Show

1. **Zero body-mass / selection inference:** By strict `QD-VIAB` design (`mass_observables_excluded = true`), `VIAB-2` records no `trait_mass*` observable and gives zero signal on whether `selected`, `scarce_drift`, or `scarce_mut0` diverge in body mass.
2. **Does NOT freeze `SEL2-P2` or run confirmation seeds:** Dedicated freeze manifests (`experiments_sel2p2/*.json`), freeze manifest tests, the JS artifact verifier, blind smoke (`999931`), and confirmation runs (`900001..=947001`) await the separate freeze card PR.
3. **`880001..=880012` remain exploratory calibration seeds:** They are permanently excluded from `SEL2-P2` confirmation (`900001..=947001`) and baseline (`970501..=970512`).
4. **`headless adapter verified` on single generated terrain only:** Reflects `LiveExperimentAdapter` (`live-bevy-world/2`) on `Generated` terrain (`checksum = 3152323152`) with `live.predator_fraction = 0.0`, `live.evolved_brains = 0.0`, and the `mutation.rs:211` `nodes.len() > 2` floor active.
