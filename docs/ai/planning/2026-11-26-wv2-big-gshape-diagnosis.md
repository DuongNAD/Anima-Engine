---
phase: planning
feature: map-v2
title: Card WV2-BIG-GSHAPE — 81.92 km G-SHAPE Diagnosis and Pre-Registered Candidates
status: active
owner: anima-mac
last_reviewed: 2026-11-26
description: Root-cause diagnosis of the five failing G-SHAPE gates on the 81.92 km BIG-COMBO world, stage-owner attribution across anima-mac and anima-case, and pre-registered MultiEnvironment candidates measured at 4096^2.
---

# `WV2-BIG-GSHAPE` — `81.92 km` `G-SHAPE` Diagnosis & Pre-Registered Candidates

## 1. Baseline Status (`BIG-COMBO @ 08549db5`, Trunk `40551113`)

Under `VerifierProfile::large_82km()` (`81.92 km`, `4096² @ 20 m`, `presets/wv2_big_81920.toml`, `world_sha256 = 08549db56c551c54ba428359f14d8fce666bca68fed8ee4b8c639f9e464a16b0`), **`8/15` gates pass** (`G-SHAPE.1`, `G-SHAPE.6`, `G-SHAPE.4_2`, `C1`, `C2`, `C5`, `C5b`, `G-SIM.Raw`) and **`7/15` fail**: the five `G-SHAPE` gates below plus `C3.Lakes` and `C4.RiversFansDeltas` (`WV2-BIG-HYDRO`, `anima-case`). Note that `C4.hack_consistent` directly embeds `G-SHAPE.4.HackLaw.passed` ([`g_comp.rs#L298`](../../../src-tauri/crates/anima-worldgen/src/verify/g_comp.rs#L298)).

| Gate | Observed Value | `"81.92km"` Threshold (`file:line`) | Primary Driving Stage(s) & Owner |
|---|---|---|---|
| `G-SHAPE.2.JunctionAngles` | `obtuse_fraction = 0.1263` (`48/380`), `median_deg = 75.02°`, `rain_shadow < windward = true` | `obtuse < 0.10`, `median in [40, 78]`, `junctions >= 5`, `rain_shadow < windward` ([`g_shape.rs#L780-L783`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs#L780-L783)) | **`tectonic.rs` (`anima-mac`)** flat coastal plain (`slope p50 = 0.0154` at `z < 50 m`) + **`deposition.rs` (`anima-mac`)** 149 alluvial fans + **`pipeline.rs` (`anima-case`)** 122 piedmont lake basins |
| `G-SHAPE.3.DrainageDensity` | `global_dd = 1.2426` (**PASS**), **`dissection_ratio = 0.7948`**, **`caprock_tile_cv = 0.7142`** (`73` tiles) | `global in [0.8, 3.5]`, `dissection_ratio >= 1.5`, `caprock_tile_cv <= 0.45` ([`g_shape.rs#L946-L947`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs#L946-L947)) | **`tectonic.rs` (`anima-mac`)** `260*(plat_u_c - u)` caprock tilt (`cv = 0.7142`); **`climate.rs` (`anima-mac`)** + **`hydrology.rs`/`g_shape.rs` (`anima-case`)** (`dissection_ratio = 0.7948`) |
| `G-SHAPE.4.HackLaw` | `h = 0.6056`, `C = 1.559 km`, **`r_squared = 0.7658`** (`2,000` samples, `max_path = 47.62 km`) | `samples >= 500`, `h in [0.52, 0.62]`, `C in [1.0, 2.0]`, **`r_squared >= 0.85`** ([`g_shape.rs#L1067-L1070`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs#L1067-L1070)) | **`pipeline.rs`/`hydrology.rs` (`anima-case`)** 122 lake basins + Priority-Flood flat-plain paths & **`g_shape.rs#L1031`** (`300 km²` cap); **`tectonic.rs` (`anima-mac`)** bimodal compact-massif vs elongated-corridor basins |
| `G-SHAPE.5.ValleysFlint` | **`vf_up = 1.1018`**, **`b_down = 1.1783`**, `b_up = 0.8663`, `vf_dn = 15.361`, `vf_ratio = 13.942`, `theta = 0.6003` | `b_up in [0.85, 1.35]`, **`vf_up in [0.05, 0.80]`**, **`b_dn in [1.60, 3.50]`**, `vf_dn >= 1.50`, `vf_ratio >= 2.5`, `theta in [0.35, 0.65]` ([`g_shape.rs#L1364-L1371`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs#L1364-L1371)) | **`pipeline.rs` (`anima-case`)** `widen_trunk_alluvial_floodplains` ([`pipeline.rs#L1199-L1330`](../../../src-tauri/crates/anima-worldgen/src/pipeline.rs#L1199-L1330)) + `erosion_fluvial.rs`/`erosion_thermal.rs` (`anima-case`) + `dx = 20 m` `Strahler 2..=3` floor quantization (`>= 20 m`) |
| `G-SHAPE.7.HypsometrySpikes` | **`hi_global = 0.1585`**, `catchment_hi = 0.3969`, `needle_spikes = 0`, `slope_p999 = 74.74°` | **`hi_global in [0.22, 0.55]`**, `catchment_hi in [0.30, 0.60]`, `needle_spikes == 0` ([`g_shape.rs#L1582-L1583`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs#L1582-L1583)) | **`tectonic.rs` (`anima-mac`)** low interior hypsometry (`76.04%` of land `< 1,000 m`, `mean = 894.58 m` vs `max = 5,645.42 m`) |

---

## 2. Detailed Root-Cause Evidence (`BIG-COMBO` Raw Analysis)

### 2.1 `G-SHAPE.2.JunctionAngles` (`obtuse_fraction = 0.1263 >= 0.10`)
- [`eval_metric_2_junction_angles`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs#L615-L785) traces `250 m` (`~12.5` cells at `20 m`) up the two dominant `Strahler >= 2` tributaries meeting at a junction and counts obtuse angles (`> 100°`).
- On `BIG-COMBO`, `20.25%` of all active channel cells (`40,002 / 197,501` at `Q >= 0.0278 m³/s`) sit below `50 m` elevation on the `14.5 km` coastal plain (`plain_ramp = 48 m / 12 km = 0.004`), where median channel slope is `0.0154`. `coast_ria_2d` undulation (`~2.8 m`), `149` alluvial fan cones (`deposition.rs`), and `122` piedmont lake basins (`pipeline.rs`) deflect tributaries laterally before confluence, pushing `48/380` junctions past `100°`.

### 2.2 `G-SHAPE.3.DrainageDensity` (`dissection_ratio = 0.7948 < 1.5`, `caprock_tile_cv = 0.7142 > 0.45`)
- **Why `dissection_ratio = 0.7948`:** In [`eval_metric_3_drainage_density`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs#L852-L860):
  - `windward_cells` (`(p >= 1.3 && elev > 150) || (slope >= 0.08 && p >= 0.9)`) covers `30.53%` (`2,724,519` cells) of dry land with channel fraction `0.01937` (`1.106 km/km²`).
  - `plains_cells` (`p < 0.8 || (slope < 0.045 && elev < 600)`) covers `57.12%` (`5,096,815` cells) with channel fraction `0.02513` (`1.392 km/km²`).
  - Crucially, `plains_cells` splits into two opposite regimes:
    1. Arid/rain-shadow plains (`p < 0.8`, `37.61%` of dry land): `ch_frac = 0.01929`.
    2. **Wet flat coastal plains** (`p >= 0.8, slope < 0.045, elev < 600 m`, `19.50%` of dry land): **`ch_frac = 0.03637` (`~2.0 km/km²`)**, because every `Strahler 4..=6` trunk river and braided fan crosses the `14.5 km` windward coastal plain into the ocean! Meanwhile on steep windward mountain slopes at `dx = 20 m` (`400 m²/cell`), `Q >= 0.027778 m³/s` delays headwater initiation down-slope (`ch_frac = 0.01937`). Both `climate.rs` (runoff contrast) and `anima-case`-owned `hydrology.rs`/`pipeline.rs` (hillslope MFD convergence vs plain braiding) drive this ratio.
- **Why `caprock_tile_cv = 0.7142`:** Measuring the `73` qualifying `2 km × 2 km` (`100 × 100` cell) caprock tiles on `BIG-COMBO` raw (`cap_t >= 0.8 * dry_t`):
  - Channel fraction spans **`28×`** across adjacent tiles: lowest tiles `(23,31), (22,31), (29,31)` have `0.0033..0.0038` while `(25,31), (25,32)` have **`0.0807..0.0927`** (`corr(ch, precip) = -0.224`).
  - Root cause in [`tectonic.rs#L816-L817`](../../../src-tauri/crates/anima-worldgen/src/tectonic.rs#L816-L817): `plateau_surface = caprock_target_m + 10.0 * fbm_detail + 14.0 * plateau_subtile + 260.0 * (plat_u_c - u)`. At `81.92 km`, the `260.0 * (plat_u_c - u)` term tilts the `~19.6 km` plateau by `~62 m` west-to-east and `14.0 * plateau_subtile` creates closed sub-basins whose outflow routes concentrate into a handful of mainstem slots (`tr = 25, tc = 31..32`), starving neighboring tiles.

### 2.3 `G-SHAPE.4.HackLaw` (`r_squared = 0.7658 < 0.85`)
- [`eval_metric_4_hack_law`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs#L1025-L1070) fits `ln(L) = ln(C) + h * ln(A)` across `2,000` `Strahler >= 2` cells with `0.02 < A_km2 <= 300.0` (note: hardcoded `300.0 km²` upper bound even at `81.92 km`).
- Mixing compact steep massif basins (`10–15 km` runs), elongated `47.6 km` corridor/coastal-plain trunks, and `122` flat lake/rift basins (`pipeline.rs`, `lakes_falls.rs`, `hydrology.rs`) widens the `L(A)` scatter (`R² = 0.7658`). Because `C4` requires `hack_consistent == true`, `G-SHAPE.4` also forces `C4.RiversFansDeltas` to fail.

### 2.4 `G-SHAPE.5.ValleysFlint` (`vf_up = 1.1018 > 0.80`, `b_down = 1.1783 < 1.60`)
- **`b_down = 1.1783 < 1.60` (`anima-case`-owned `pipeline.rs`):** [`widen_trunk_alluvial_floodplains`](../../../src-tauri/crates/anima-worldgen/src/pipeline.rs#L1199-L1286) modifies cells only within `perp <= w_fp` (`280..560 m`, less than half the `1,200 m` transect in [`g_shape.rs#L1279`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs#L1279)) and only lowers terrain where `t_h < fp_target[j]` (zero effect on flat coastal plains where surrounding banks are already `< t_h`, and outer canyon walls `600..1,200 m` dominate the OLS fit with `b ~ 1.1` elsewhere).
- **`vf_up = 1.1018 > 0.80` (`anima-case`-owned `pipeline.rs` + `dx = 20 m`):** At `dx = 20 m`, [`sample_valley_transect`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs#L1166-L1177) has minimum floor width `left_floor_w + right_floor_w >= 2 * 0.5 * 20 m = 20 m` (`2×` larger than `10 m`), and `up_cands` (`Strahler 2..=3` everywhere with `hl, hr >= 4 m`) includes gentle piedmont/plateau reaches (`pipeline.rs#L1286-L1315` only trims `1..=2` cells). **Cannot be fixed in `tectonic.rs`/`climate.rs` alone; requires `anima-case` (`BLOCKED` sent).**

### 2.5 `G-SHAPE.7.HypsometrySpikes` (`hi_global = 0.1585 < 0.22`)
- `hi_global = (894.58 - 0.0) / (5,645.42 - 0.0) = 0.158461`. Land breakdown on `BIG-COMBO`:
  - `[0, 100) m`: `22.78%` | `[100, 500) m`: `33.42%` | `[500, 1000) m`: `19.84%` (`76.04% < 1,000 m`)
  - North Massif (`rows 0..1400`, `32.2%` of land): `mean = 555.3 m`, `max = 4,071.8 m`
  - Corridor (`rows 1400..2100`, `17.5%` of land): `mean = 359.2 m`
  - South Massif (`rows 2100..3400`, `33.2%` of land): `mean = 1,659.8 m`, `max = 5,645.4 m`
- Meanwhile `C5b` has generous headroom (`5 km = 0.7461 >= 0.60`, `3 km = 0.8051 >= 0.75`, `med_1000m = 12.88 km >= 10.0 km`, `front < 4 km = 0.0806 <= 0.20`). Raising interior upland elevation (`d_coast_m >= 14 km`), broadening the Northern Massif + Southern Massif flanks (without raising `max_z`), and raising the caprock plateau increases `mean_z` past `1,240 m` (`HI_global >= 0.22`) while preserving the `< 100 m` coastal plain (`>= 20%`).

---

## 3. Pre-Registered `MultiEnvironment`-Only Candidates (`anima-mac`-Owned Files)

All changes below are gated strictly behind `WorldLayout::MultiEnvironment` (`tectonic.rs`, `climate.rs`); `SingleMassif` (`C01`) stays bit-identical. No `anima-case` files (`pipeline.rs`, `hydrology.rs`, `erosion_*.rs`, `lakes_falls.rs`, `verify/**`) are touched.

### Candidate `G1` — Tectonic Interior Hypsometry + Caprock Tilt Flatten + Coastal Plain Slope (`tectonic.rs`)
- **Parameter changes (inside `eval_multi_strat_and_envelope` & `eval_cell_multi_raw`, `MultiEnvironment` only):**
  1. `plain_ramp`: `48.0 -> 64.0` m over `12 km` (`< 100 m` everywhere, `+33%` seaward slope to reduce flat-plain obtuse junctions).
  2. `inland_ramp`: `185.0 * tanh((d_coast_m - 12_000) / 16_000)` -> `560.0 * tanh((d_coast_m - 14_000).max(0) / 11_000)` (`0 m` inside `14 km` so `C5b` coastal plain is untouched; `+375 m` interior uplift beyond `14 km`).
  3. Northern Massif (`massif2_h`): `dv2` width `8_500 -> 11_200 m`, `w2` `(5_200, 5_800) -> (7_400, 8_000) m`, amplitude `3_150 -> 3_950 m` (raises North Massif mean from `555 m` to `~1,050 m` while keeping `north_max < 5,000 m`).
  4. Southern Massif (`massif1_h`): `dv1` width `10_500 -> 12_200 m`, `w1` `7_400 -> 8_800 m`, peak coeff `5_380 -> 5_050 m` (broadens mid-elevation `1–3 km` flanks while holding `land_max ~ 5,550 m`).
  5. Caprock plateau (`plateau_surface`): `caprock_target_m` offset `+120 -> +380 m` (`~1,186 m`), regional tilt `260.0 * (plat_u_c - u) -> 45.0 * (plat_u_c - u)`, `plateau_subtile` amplitude `14.0 -> 6.0`.
- **Pre-registered predictions (`G1`):**
  - `G-SHAPE.7`: **`HI_global` rises from `0.1585` to `0.22–0.26` (PASS)**, `catchment_hi` stays in `[0.30, 0.60]`, `needle_spikes == 0`.
  - `G-SHAPE.3.caprock_tile_cv`: drops from `0.7142` toward **`0.40–0.55`**.
  - `G-SHAPE.2.obtuse_fraction`: drops slightly from `0.1263` toward **`0.10–0.12`**.
  - `G-SHAPE.5` and `G-SHAPE.3.dissection_ratio`: remain **FAIL** (driven by `pipeline.rs`/`hydrology.rs` and `plains_flat_wet`).
  - `C5b`, `C5`, `C1`, `C2`, `G-SIM`: remain **PASS** (`plain_backed_5km >= 0.65`, `E1..E5` inside bands, though `E5` rises `+2–4%` from cooler interior elevation).

### Candidate `G2` — `G1` + Windward Orographic Precipitation Gain (`tectonic.rs` + `climate.rs`)
- **Parameter changes (`G1` + `ClimateConfig::for_world_with_target` `MultiEnvironment` only):**
  - `windward_precip_target_m_yr`: `3.2 -> 3.9` (`+22%` windward mountain runoff while keeping `leeward_rain_shadow_gain = 1.32` so existing `climate.rs` unit test stays intact).
- **Pre-registered predictions (`G2` vs `G1`):**
  - `G-SHAPE.3.dissection_ratio`: rises moderately from `~0.80` toward **`~0.90–1.05`** (still likely `< 1.5` without `hydrology.rs`/`pipeline.rs` changes).
  - `C1`/`C2`/`E1..E5`: `Jungle`/`Mangrove` expand slightly (`E1` could approach or exceed the `29%` upper bound if precipitation rises too much).

---

## 4. `4096²` Candidate Measurements (`G1` & `G2` — Both Reverted)

Both candidates were built in `--release` and measured end-to-end (`generate` + `verify --report-only`) at `4096²` (`81.92 km`, `dx = 20 m`, `seed = 0x57563201`) on macOS `arm64` (`RAYON_NUM_THREADS=4`). Per the strict non-regression rule (and `MSG_20261126_104100`), **neither candidate is kept in code** because each introduced a gate regression without clearing `G-SHAPE.{2,3,4,5}`. Both `tectonic.rs` and `climate.rs` were reverted so this PR delivers the diagnosis and measured `4096²` evidence cleanly ahead of `WV2-BIG-HYDRO`.

### 4.1 Gate & Identity Comparison (`BIG-COMBO` Baseline vs. `G1` vs. `G2`)

| Gate / Metric | `BIG-COMBO` (`08549db5`) | `G1` (`tectonic.rs` uplift) | `G2` (`G1 + windward 3.9`) | Pass / Regression Note |
|---|---|---|---|---|
| `world_sha256` (prefix) | `08549db5` | `b762c65b` | `d18349cb` | Deterministic seed `1465266689` |
| `sim_world_identity` (prefix) | `132fa5fa` | `ffea30bb` | `b61309f5` | Raw `4096² -> 256²` bridge |
| Passing / Failing (`15` gates) | `8 / 15` (`7` fail) | `9 / 15` (`6` fail) | `8 / 15` (`7` fail) | Neither clears `G-SHAPE` |
| `G-SHAPE.1` (`omega / r_b / r_l`) | **PASS** (`6 / 4.175 / 1.645`) | **PASS** (`5 / 4.171 / 1.826`) | **PASS** (`6 / 4.022 / 1.715`) | `omega_max` drops `6 -> 5` in `G1` |
| `G-SHAPE.2` (`obtuse_fraction / med`) | FAIL (`0.1263 / 75.02°`, `380`) | FAIL (`0.1168 / 71.63°`, `368`) | FAIL (`0.1153 / 72.90°`, `451`) | Improves slightly; still `>= 0.10` |
| `G-SHAPE.3.global_dd` | PASS (`1.2426`) | PASS (`1.2942`) | PASS (`1.3440`) | In `[0.8, 3.5]` |
| `G-SHAPE.3.dissection_ratio` | FAIL (`0.7948`) | FAIL (`1.0665`) | FAIL (`1.0410`) | `+0.25..0.27`, still `< 1.50` (`anima-case` hydro/plain) |
| `G-SHAPE.3.caprock_tile_cv` | FAIL (`0.7142`, `73` tiles) | FAIL (`0.7501`, `65` tiles) | FAIL (`0.7652`, `65` tiles) | **Worsens** (`+0.036..0.051`) |
| `G-SHAPE.4` (`R² / h / C`) | FAIL (`0.7658 / 0.606 / 1.559`) | FAIL (`0.7778 / 0.604 / 1.630`) | FAIL (`0.8169 / 0.641 / 1.482`) | `G2` raises `R²`, pushes `h > 0.62` |
| `G-SHAPE.5.vf_up` (`[0.05, 0.80]`) | FAIL (`1.1018`) | **in-band (`0.6795`)** | **in-band (`0.7280`)** | Enters band via steeper upper terrain |
| `G-SHAPE.5.b_down` (`[1.60, 3.50]`) | FAIL (`1.1783`) | FAIL (`1.1777`) | FAIL (`0.9898`) | Trunk widening untouched (`pipeline.rs`) |
| `G-SHAPE.5.flint_theta / R²` | **in-band (`0.6003 / 0.9316`)** | **FAIL (`0.2321 / 0.2838`)** | **in-band (`0.4137 / 0.9066`)** | **`G1` collapses Flint concavity** |
| `G-SHAPE.6.EndorheicSpillways` | **PASS** (`endo_humid = 0.0`) | **PASS** (`endo_humid = 0.0`) | **FAIL** (`endo_humid = 0.000001`) | **`G2` regresses `M6` to FAIL** |
| `G-SHAPE.7.HypsometrySpikes` | FAIL (`hi = 0.1585`, `0` spikes) | **PASS** (`hi = 0.2401`, `0` spikes) | **PASS** (`hi = 0.2387`, `0` spikes) | Fixed by interior + massif uplift |
| `G-SHAPE.4_2.ShorelineFractal` | **PASS** (`1.1939`, `R²=0.9968`) | **PASS** (`1.1808`, `R²=0.9971`) | **PASS** (`1.1973`, `R²=0.9966`) | Stable |
| `C1.Partition` / `C2.Ecotone` | **PASS** / **PASS** | **PASS** / **PASS** | **PASS** / **PASS** | All `16` zoned biomes pass |
| `C3.Lakes` / `C4.RiversFansDeltas` | FAIL (`122`) / FAIL | FAIL (`154`) / FAIL | FAIL (`151`) / FAIL | `anima-case` (`WV2-BIG-HYDRO`) |
| `C5.Sightlines` / `C5b.Plain5km` | **PASS** / **PASS (`0.7461`)** | **PASS** / **PASS (`0.7261`)** | **PASS** / **PASS (`0.7527`)** | `C5b` stays `>= 0.72` (`>= 0.60` req.) |
| `G-SIM.Raw` (`walkable_frac`) | **PASS** (`0.9959`) | **PASS** (`0.9962`) | **PASS** (`0.9963`) | Passes |
| Macro biomes `E1..E5` (`% land`) | `25.9 / 13.2 / 29.1 / 9.2 / 13.4` | `20.4 / 20.0 / 16.5 / 11.5 / 15.3` | `21.8 / 21.5 / 15.3 / 9.6 / 15.8` | Higher interior shifts `E3 -> E2` |

### 4.2 Timing, Memory, and Key Takeaways for Combined Tuning (After `WV2-BIG-HYDRO`)

- **Compute cost (`macOS arm64`, `RAYON_NUM_THREADS=4`):**
  - `G1`: `generate` = `1,388.27 s`, `verify --report-only` = `4.39 s`.
  - `G2`: `generate` = `1,197.66 s` (`max RSS = 1,119,223,808 B` / `1.04 GiB`), `verify --report-only` = `4.34 s` (`max RSS = 1,542,733,824 B` / `1.44 GiB`).
- **Why `G1` & `G2` were reverted and what works next:**
  1. **`G-SHAPE.7` (`hi_global >= 0.22`) IS solvable in `tectonic.rs` alone** without hurting `C5b` or `E1..E5`: raising `inland_ramp` outside `14 km` and broadening the Northern/Southern Massif flanks pushed `hi_global` cleanly to `0.2401` (`G1`) / `0.2387` (`G2`) while keeping `C5b 5km = 0.726..0.753` and `needle_spikes = 0`.
  2. **Steep `inland_ramp` alone breaks `flint_concavity_theta`:** `G1`'s `+560 m` `inland_ramp` + `+380 m` flat caprock created a break in `S(A)` at intermediate `Q`, collapsing `flint_theta` from `0.6003` (`R² = 0.9316`) to `0.2321` (`R² = 0.2838`). Raising `windward_precip_target_m_yr` in `G2` recovered `flint_theta = 0.4137` (`R² = 0.9066`), **but** crossed the humid-cell runoff threshold inside closed depressions, regressing `G-SHAPE.6.EndorheicSpillways` (`endo_humid = 0.000001 > 0`).
  3. **Tilted caprock (`cv`) got worse when plateau elevation rose:** reducing `260*(plat_u_c - u)` to `45*(plat_u_c - u)` in `G1`/`G2` while shrinking qualifying `cap_t >= 0.8` tiles (`73 -> 65`) increased `caprock_tile_cv` (`0.7142 -> 0.7501 / 0.7652`). Edge drainage off the higher plateau dominates tile variance rather than intra-plateau west-east tilt.
  4. **Conclusion (`WV2-BIG-3` coordination):** `tectonic.rs` uplift for `G-SHAPE.7` and caprock noise/CV tuning should be tuned **after** `anima-case`'s `WV2-BIG-HYDRO` (`pipeline.rs`, `hydrology.rs`, `lakes_falls.rs`, `erosion_*.rs`) lands, so spillway routing (`M6`), Hack scatter (`M4`), downstream floodplain width (`M5.b_down`), and `plains_flat_wet` braiding (`M3.dissection_ratio`) are fixed in one coherent `WV2-BIG-3` pass.
