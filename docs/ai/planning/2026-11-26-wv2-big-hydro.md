---
phase: planning
feature: map-v2
title: Card WV2-BIG-HYDRO — Pre-Registered C3 Lakes & C4 Rivers Investigation at 81.92 km
description: Pre-registered diagnosis and 3 candidates for C3 lake surface fraction and C4/HackLaw R^2 at 81.92 km (4096^2 @ 20 m) while keeping 40.96 km C01 bit-identical
status: active
owner: anima-case
last_reviewed: 2026-11-26
---

# Card `WV2-BIG-HYDRO` — `C3` Lakes & `C4` Rivers at `81.92 km`

> **Pre-registered before any measurement (`PROMPTS_2026-11-26b_DAY_WIN1_anima-case.md` D1; updated per `MSG_20261126_104000`).**
> **Trunk merged:** `origin/feat/ecological-selection` @ `1eff23907e19f9a39fb7d5cc43405d683c0b9118` (`#308`–`#321`).
> **Changed code paths:** strictly `src-tauri/crates/anima-worldgen/src/pipeline.rs` and this planning document (`verify/**`, `tectonic.rs`, `climate.rs`, `biomes.rs`, `deposition.rs`, `artifacts/world-v2/`, and `docs/ai/README.md` untouched).
> **Gating invariant:** Every change is gated on the `81.92 km` scale (`extent_m >= 80_000.0`) so default `40.96 km` (`C01` and `S2_6`) remain bit-identical (`49f1d62b...` / `079772566c...`).

---

## 1. Failing Sub-Values & Thresholds (`BIG-COMBO-X` `08549db5...` Baseline at `81.92 km @ 20 m`)

From `test_runs/anima-case_BIG-COMBO-X_report.md` (`BIG_4096_v2`, `4096^2`, `dx_m = 20.0`, `extent_m = 81920.0`):

### 1.1 `G-COMP-v2.C3.Lakes` (`FAIL`)

| Metric | Measured (`08549db5`) | Threshold (`VerifierProfile::large_82km()`) | Status |
|---|---|---|---|
| `lake_surface_fraction` | `0.004342` (raw `s6 = 0.007889`) | **`0.008..=0.045`** | **FAIL** (`~1.84x` below floor) |
| `total_mapped_basins` | `122` | diagnostic | `—` |
| `large_lake_count` (`> 0.1 km²`) | `26` | `>= 12` | PASS |
| `high_paramo_lake_count` | `95` | `>= 2` | PASS |
| `caldera_or_trough_lake_count` | `23` | `>= 2` | PASS |
| `small_paramo_pond_count` | `95` | `>= 20` | PASS |

### 1.2 `G-COMP-v2.C4.RiversFansDeltas` & `G-SHAPE.4.HackLaw` (`FAIL`)

| Metric | Measured (`08549db5`) | Threshold (`VerifierProfile::large_82km()`) | Status |
|---|---|---|---|
| `counted_river_surface_fraction` | `0.001062` (`189,778` cells) | `0.0005..=0.0030` | PASS |
| `monotonic_w2_w3_w4` | `true` (`[0.82, 1.84, 3.52, 7.05, 11.41, 18.45]`) | `true` | PASS |
| `fan_count` | `149` | `>= 6` | PASS |
| `delta_count` | `9` | `>= 4` | PASS |
| `oxbow_count` / `meander_scroll_cells` | `0` / `37,765` | `>= 1 \|\| > 0` | PASS |
| `hack_consistent` | **`false`** | `true` | **FAIL** |
| └─ `G-SHAPE.4` `sample_count` | `2,000` | `>= 500` | PASS |
| └─ `G-SHAPE.4` `exponent_h` | `0.605637` | `0.52..=0.62` | PASS |
| └─ `G-SHAPE.4` `prefactor_c_km` | `1.559344` | `1.0..=2.0` | PASS |
| └─ `G-SHAPE.4` **`r_squared`** | **`0.765826`** | **`>= 0.85`** | **FAIL** (`-0.0842`) |
| └─ `G-SHAPE.4` `median_mainstem_sinuosity` | `1.235296` | `>= 1.05` | PASS |

---

## 2. Suspected Stages

1. **`C3.Lakes` (`pipeline.rs:apply_late_quaternary_piedmont_subsidence`):**
   - At `40.96 km` (`1,677.7 km²`), 6 piedmont ellipses use fixed physical radii `rx_m = 920 * sqrt(basin_amp)`, `ry_m = 1,650 * sqrt(basin_amp)`.
   - At `81.92 km` (`6,710.9 km²`, `4x` domain area), `WV2-BIG-2` only increased `base_vs` slots from 6 to 12 (`[0.08..0.94]`, with `0.08` and `0.94` clamped into `margin = 0.10`), while keeping `rx_m` and `ry_m` fixed. Combined with `BIG-1` wider passive-margin lowlands, total lake area reaches only `~29.1 km²` (`0.4342%` of grid vs `>= 0.80%` = `53.69 km²`).
2. **`C4` / `G-SHAPE.4.HackLaw` (`verify/g_shape.rs`, `verify/mod.rs`, `pipeline.rs`):**
   - **(a) Lake-outlet length reset (`verify/g_shape.rs:363,978-1019`):** `flow_acc_m2` accumulates continuously through lakes on `filled_h`, whereas `is_channel` masks out lake cells so the Hack topological sweep (`longest_m`) resets to `dx` at every lake spillway — creating large-`A`, tiny-`L` outliers that pull down `R²`.
   - **(b) Unscaled upper area cutoff (`verify/g_shape.rs:1031`):** `eval_metric_4_hack_law` hardcodes `a_km2 <= 300.0` (appropriate for `40.96 km` where `VerifierProfile::standard_40km().flint_a_max_m2 = 300 km²`, whereas `VerifierProfile::large_82km().flint_a_max_m2 = 1,200 km²`), truncating Strahler 5–6 mainstems (`max_path_km = 47.62 km`).
   - **(c) Coastal-plain channel-head lag / basin disruption (`hydrology.rs`, `pipeline.rs`):** On broad low-slope coastal plains (`C5b`), `A * S² >= 500 m²` may delay channel initiation far down-catchment, or piedmont lake/fan carving may perturb mainstem connectivity.

---

## 3. Pre-Registered Candidates & Predictions (Written Before Measurement)

1. **Candidate H1 — Piedmont Lake Area & Placement Scaling (`pipeline.rs`, gated on `extent_m >= 80_000.0`):**
   - In `apply_late_quaternary_piedmont_subsidence`: when `extent_m >= 80_000.0`, scale `rx_m` and `ry_m` by `1.70x` (`~2.89x` individual basin footprint), place 12 unclamped latitude slots across `[0.11, 0.88]` with `min_spacing = 5_200.0 m`, and target `sill_target = h_lip + 4.5` while keeping elevation bounds (`10..220 m`, `h_lip in [38, 165]`) and `acc_cells < trunk_cells` intact.
   - **Prediction:** `lake_surface_fraction` at `81.92 km` increases from `0.00434` into `[0.010, 0.020]` (`C3` PASS), with `endo_humid == 0.0` preserved and zero regressions on `C1`, `C2`, `C5`, `C5b`, `G-SHAPE.1/6/4.2`, or `G-SIM`.

2. **Candidate H2 — *WITHDRAWN* (`MSG_20261126_104000` Lead Ruling):**
   - Verifier-side changes (`verify/g_shape.rs` `hack_a_max_km2 = 1200` and `hack_bridge_lakes = true`) are **withdrawn** under Lead ruling `MSG_20261126_104000` (*"Changing a verifier threshold or cap is NOT a fix. Report gates honestly."*). All `verify/**` files remain untouched vs trunk; `G-SHAPE.4.HackLaw` and `C4.hack_consistent` are evaluated strictly on the unmodified trunk verifier.

3. **Candidate H3 — Dual-Column Piedmont Lakes (`pipeline.rs`, gated on `extent_m >= 80_000.0`):**
   - Backup only if H1 still undershoots `C3` (`< 0.008`) on the true preset seed `1465266689`: add dual-column piedmont placement (`30%` and `70%` quantiles of `cands`, `dist_m >= 5_500 m`).

4. **Candidate H4 — `G-SHAPE.5.ValleysFlint` Floodplain & V-Valley Scaling (`pipeline.rs:widen_trunk_alluvial_floodplains`, gated on `extent_m >= 80_000.0` per `MSG_20261126_104000`):**
   - **Baseline failure (`08549db5`):** `vf_up = 1.1018` (`> 0.80` FAIL), `b_down = 1.1783` (`< 1.60` FAIL), `b_up = 0.8663`, `vf_down = 15.36`, `theta = 0.6003`.
   - **H4a (downstream `Strahler >= 4` `b_down`):** widen `w_fp` from `(280 + 40 * river_w_m).clamp(280, 560)` to `(640 + 45 * river_w_m).clamp(640, 1_240)` and `r_ref = 110.0_f32.max(5.0 * dx_m)` with `t_h = z_i + 0.36 * powf(u_r, 2.20)` so the full `1,200 m` verifier transect (`g_shape.rs:1319`) is U-shaped (`b_down >= 1.60`, never modifying `strahler[j] >= 1`).
   - **H4b (upstream `Strahler 2..=3` `vf_up`):** gate off flat-floor widening when `z_j1 - z_i < 0.14 * (z_j2 - z_i).max(15.0)` and gently steepen over-flattened `20 m` V-shoulders (`z_i > 40.0`, `z_j2 > z_i + 8.0`, `z_j1 < z_i + 0.35 * (z_j2 - z_i)` toward `z_i + 0.45 * (z_j2 - z_i)`, capped `+8 m`, `strahler[j1] == 0`) so `vf_up <= 0.80` while `b_up in [0.85, 1.35]` and `Flint theta` stay intact.
   - **Prediction:** `b_down` rises `>= 1.60` and `vf_up` drops `<= 0.80` (`G-SHAPE.5` PASS), without breaking `Flint theta` or `G-SIM`.

5. **Candidate H5 — `G-SHAPE.3.DrainageDensity` Mountain-vs-Plains Dissection (`pipeline.rs:worldgen_hydro_config`, gated on `extent_m >= 80_000.0` per `MSG_20261126_104000`):**
   - **Baseline failure (`08549db5`):** `dissection_ratio = 0.7948` (`< 1.50` FAIL), `windward_mountain_dd = 1.106`, `plains_rain_shadow_dd = 1.392`, `global_dd = 1.243`.
   - **Mechanism:** At `extent_m >= 80_000.0`, lower `channel_init_threshold` from `cfg.channel_init` (`500.0`) to `250.0` and raise `mfd_exponent_p` from `cfg.mfd_p` (`1.90`) to `2.40` (testing `dx = 20 m` headwater initiation on humid steep slopes without altering flat low-slope plain area).
   - **Prediction:** Steep windward mountains initiate denser first-order gullies (`windward_dd` increases faster than `plains_dd`, raising `dissection_ratio`), subject to screening at `1024²` so `C4.counted_river_surface_fraction` stays inside `[0.0005, 0.0030]` and `G-SIM` / `C5` / `C5b` do not regress.

6. **Candidate H6 — `G-SHAPE.4.HackLaw` / `C4` Pipeline Coherence (`pipeline.rs`, gated on `extent_m >= 80_000.0`):**
   - Keep piedmont lake subsidence off active mainstems (`acc_cells < trunk_cells`) and measure residual `HackLaw R²` honestly at `1024²` and `4096²` on true preset `seed = 1465266689` with the unmodified trunk verifier.

## 4. `1024²` Screening Results (`seed = 1465266689`, Unmodified Trunk Verifier)

All `1024²` screens ran `--release` (`RAYON_NUM_THREADS=4`) on `presets/wv2_big_81920.toml` (`--size 1024`, preserving `seed = 1465266689`) and were evaluated honestly via `verify --raw ... --report-only`.

| Candidate | `world_sha256` (prefix) | `C3.lake_frac` | `M5.b_down` | `M5.vf_up` | `M5.theta` / `R²` | `M3.dissect` | `M6.endo_humid` | Outcome |
|---|---|---|---|---|---|---|---|---|
| **H1** (1.70x radii, 12 single-col slots `[0.11..0.88]`) | `4f6555e3` | `0.004504` | `1.9715` | `2.9181` | `0.523` / `0.594` | `1.0293` | `0.0` (PASS) | Superseded by H3 (`C3 < 0.008`) |
| **H3** (`H1` + staggered col-2 `78%` quantile, `rim_trunk = 12 km²`) | `5adc6869` | **`0.015221`** | `1.5304` | `3.2444` | `0.715` / `0.964` | `1.0988` | `0.0` (PASS) | **KEPT** (`3.38x` lake boost) |
| **H3 + H4 v1** (`w_fp <= 1240`, `r_ref = 400`, `wall_k = 0.28`, `H4b`) | `ac24bd0e` | `0.015471` | `1.4862` | `1.0479` | `0.254` / `0.184` | `1.1035` | `0.0` (PASS) | Rejected (`Flint` collapse) |
| **H3 + H4 v2** (`discharge_q >= 0.020` guard, `r_ref = 200`, `wall_k = 0.55`) | `0877abf0` | `0.015780` | `1.4081` | `2.6711` | `0.720` / `0.962` | `1.1553` | `0.0` (PASS) | Tuned `wall_k` / `r_ref` |
| **H3 + H4 v3** (`r_ref = 200`, `wall_k = 0.32`, `pow = 2.35`, `H4b` `strahler==0`) | `f7597ca0` | `0.015760` | **`1.6684`** | `3.2649` | `0.722` / `0.962` | `1.1511` | `0.0` (PASS) | Baseline before H5/j3 |
| **H3 + H4 + H5** (`channel_init = 250`, `mfd_p = 2.40` — **REVERTED**) | `4ce1702b` | `0.015765` | `0.9589` | `1.0682` | `0.729` / `0.958` | `1.2550` | `0.0` (PASS) | **REVERTED** (`omega_max` `6->5`, `b_down < 1.60`) |
| **H3 + H4(j3)** (`H5` reverted; `3*dx` flat-floor `H4b` check) | `cd45cbcf` | `0.015725` | **`1.6684`** | `1.7784` | `0.723` / `0.961` | `1.1494` | `0.0` (PASS) | Promoted to `4096²` test |

---

## 5. `4096²` Progression & Final Authoritative Comparison (`seed = 1465266689`, Unmodified Verifier)

Three full `4096² @ 20 m` runs (`presets/wv2_big_81920.toml`, no `--seed`, no `--size`) measured the progression from `BIG-COMBO @ 08549db5`:

- **Run 1 (`4b61ce9e`, `GEN_SEC = 3497.06 s`, `VER_SEC = 14.89 s` — Superseded):** Included `H3` + `H4(j3)` with `q_channel = (q >= 0.05)`. Passed `C3.Lakes` (`0.009458`) and `M6` (`0.0`), but omitting `43,717` verified channels (`rw >= 0.5 && q < 0.05`) misclassified `rw = 4.5..8.0 m` trunks as `ord == 3`, hitting them with `H4b` and dropping `b_down` to `0.9661` and `b_up` to `0.7779`. Direct transect inspection also showed northern-half upstream valleys (`rows < 2200`) have median wall height `mh = 20.9 m` (`20 m / 20.9 m = 0.957 > 0.80`), confirming `vf_up <= 0.80` requires `tectonic.rs` North Massif relief (`#323 G3` achieved `vf_up = 0.3806`).
- **Run 2 (`c166e5c9`, `GEN_SEC = 3389.72 s`, `VER_SEC = 13.32 s` — Superseded):** Matched `q_channel` (`(is_ch && q >= 0.027_778) || rw >= 0.5`) and removed `H4b` (`fp_lift`). Improved score to `9/15` (`C3.Lakes = 0.009508`, `b_down = 1.6854` in-band, `vf_down = 28.82`, `flint_theta = 0.6096`, `M2.obtuse = 0.0936`, `M3.dissect = 0.8681`), but `H4a` widening (`<= 1,160 m`) overlapped `29.7%` of neighboring `ord == 2..=3` valleys (`med_b = 0.573` near trunks), dipping `b_up` to `0.8100`.
- **Final Authoritative `4096²` Run (`01956959`, `GEN_SEC = 3143.37 s`, `VER_SEC = 13.59 s` — KEPT):** Added `near_upper_valley` (`±5` cells / `±100 m`) shielding for `perp > 240 m` and gated the `ord 2..=3` trim to `discharge_q >= 0.05`.

| Gate / Metric (`VerifierProfile::large_82km()`) | Baseline `BIG-COMBO` (`08549db5`) | Final `WV2-BIG-HYDRO` (`01956959`) | Status & Notes |
|---|---|---|---|
| `world_sha256` (prefix) | `08549db5` | `01956959c751458d` | Deterministic `seed = 1465266689` |
| `sim_world_identity` (prefix) | `132fa5fa` | `1d3130054d693942` | `sim_256_byte_len = 1,445,892` B |
| **Passing / Total (`15` gates)** | **`8 / 15`** | **`9 / 15`** | **`+1` gate (`C3.Lakes`), `0` passing gates lost** |
| `G-SHAPE.1.StrahlerHorton` | PASS (`6 / 4.175 / 1.645`) | **PASS** (`omega=5`, `r_b=4.3143`, `r_l=1.8018`) | Pass preserved |
| `G-SHAPE.2.JunctionAngles` | FAIL (`0.1263 / 75.02°`) | FAIL (`0.1032 / 71.57°`, `rain_shadow 69.83° < 70.98°`) | `tectonic.rs` / `deposition.rs` flat plain (`obtuse` `-0.023`) |
| `G-SHAPE.3.DrainageDensity` | FAIL (`1.243`, `dissect=0.7948`, `cv=0.7142`) | FAIL (`1.2194`, `dissect=0.8615`, `cv=0.7141`, `73` tiles) | `dissection_ratio` `+0.067`; `cv` & precip/plain remain |
| `G-SHAPE.4.HackLaw` | FAIL (`h=0.606`, `R²=0.7658`) | FAIL (`h=0.5977`, `C=1.5179`, `R²=0.7365`, `max_L=49.02 km`) | Trunk verifier unmodified (`300 km²` cap + lakes) |
| `G-SHAPE.5.ValleysFlint` | FAIL (`b_up=0.866`, `vf_up=1.102`, `b_dn=1.178`) | FAIL (`b_up=0.8129`, `vf_up=1.256`, **`b_dn=1.6711`**, `vf_dn=34.53`, `theta=0.6081`, `R²=0.958`) | **`b_down` & `vf_ratio=27.49` IN-BAND**; `vf_up` / `b_up` await `G3` upland relief (`WV2-BIG-3`) |
| `G-SHAPE.6.EndorheicSpillways` | PASS (`endo_humid = 0.0`) | **PASS** (`0` pits, `0` severed, `endo_humid = 0.0`) | Zero regressions |
| `G-SHAPE.7.HypsometrySpikes` | FAIL (`hi=0.1585`, `0` spikes) | FAIL (`hi=0.1583`, `0` spikes, `p999=74.74°`) | `tectonic.rs` (`#325 G3`) |
| `G-SHAPE.4_2.ShorelineFractal` | PASS (`1.1939`) | **PASS** (`d_shore=1.1901`, `R²=0.9969`) | Zero regressions |
| `G-COMP-v2.C1.Partition` | PASS (`16/16`, `wf=58`, `caves=90`) | **PASS** (`16/16`, `wf=59`, `caves=90`, `spacing=431.74 m`) | Zero regressions |
| `G-COMP-v2.C2.Ecotone` | PASS | **PASS** (`ecotone=0.4261`, `sharp_jump=0.01107`) | Zero regressions |
| **`G-COMP-v2.C3.Lakes`** | **FAIL (`0.004342`)** | **PASS (`0.009526`, `141` basins, `45/95/36/95`)** | **Fixed by H3 (`2.19x` baseline)** |
| `G-COMP-v2.C4.RiversFansDeltas` | FAIL (`hack_consistent=false`) | FAIL (`frac=0.000980`, `mono=true`, `fans=135`, `deltas=9`, `hack_consistent=false`) | Tracks `G-SHAPE.4` cleanly |
| `G-COMP-v2.C5.CoastalSightlines` | PASS | **PASS** (`gentle=0.7930`, `cliff=0.1041`, `obstructed=0`) | Zero regressions |
| `G-COMP-v2.C5b.CoastalPlainProfile` | PASS (`5km=0.7461`) | **PASS** (`5km=0.7439`, `3km=0.8026`, `med=12.87 km`, `front=0.0818`) | Zero regressions |
| `G-SIM.Raw` | PASS (`walkable=0.9959`) | **PASS** (`walkable=0.9958`, `mismatches=0`, `round_trip_ok=true`) | Zero regressions |

---

## 6. `C01` (`4096² @ 10 m`) Bit-Identity Proof & Negative-Control Drill

- **Final `C01` Control (`--seed 0x0057563220261120 --size 4096`, `GEN_SEC = 1161.43 s`, `VER_SEC = 12.17 s`):**
  - `world_sha256 = 49f1d62b11fc598cd43161af967c4fd566962cf735fe2112c6c706b667644c28` (**exact match to trunk `C01`**).
  - `sim_world_identity = 079772566c4f111b500940103d023cc9a875dead153f924b722211d043a59fd2` (**exact match to trunk `C01`**).
  - `failed_gates`: exact 6 trunk gates (`G-SHAPE.2`, `G-SHAPE.3`, `G-SHAPE.4`, `G-SHAPE.5`, `C3.Lakes`, `C4.RiversFansDeltas`); `G-SIM = PASS`.
- **Live A3 Negative-Control Drill (`src-tauri/crates/anima-worldgen/src/pipeline.rs`):**
  - Temporarily disabled the `extent_m >= 80_000.0` gate in `widen_trunk_alluvial_floodplains` (`let is_large = false;`): `cargo test ... --lib piedmont_subsidence_tests` failed as expected (**`NEG_CTRL_EXIT = 101`**, `widen_trunk_alluvial_floodplains_preserves_40km_and_shapes_82km FAILED`).
  - Restored cleanly (`sha256` exact match): `piedmont_subsidence_tests` re-passed **`2 passed; 0 failed` (`RESTORE_EXIT = 0`, `0.49 s`)**.
