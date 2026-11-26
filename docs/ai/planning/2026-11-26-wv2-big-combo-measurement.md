---
phase: planning
feature: map-v2
title: World v2 81.92 km BIG-COMBO Cross-Platform Measurement (WV2-BIG-1 + WV2-BIG-2)
status: active
owner: anima-dell
last_reviewed: 2026-11-26
description: Verbatim transcription of the 2026-11-26 cross-platform 81.92 km (4096^2 at 20 m) BIG-COMBO and BIG-COMBO-X measurements across macOS arm64 and Windows 1, including source SHAs, identical hashes, 15-gate table with C5b sub-values, biome shares, both WB-D8 coast-mix measurements, and C01 bit-identity.
---

# World v2 `81.92 km` `BIG-COMBO` Cross-Platform Measurement (`WV2-BIG-1` + `WV2-BIG-2`)

> **Measurement-only record (transcribed verbatim from the two hub reports; zero reinterpretation, zero tuning, zero `artifacts/` changes).**
> - **Source reports (in `Anima_Agent_Hub`):**
>   1. `test_runs/anima-mac_BIG-COMBO_report.md` (`anima-mac`, `macOS arm64`, Apple M5, `CARGO_BUILD_JOBS=4`, `RAYON_NUM_THREADS=4`, card `MSG_20261125_231500_from_owner_big_combo_mac.md`).
>   2. `test_runs/anima-case_BIG-COMBO-X_report.md` (`anima-case`, `Windows 1`, `DESKTOP-3EPV830`, `CARGO_BUILD_JOBS=8`, `RAYON_NUM_THREADS=4`, card `MSG_20261125_231600_from_owner_big_combo_crosscheck_case.md`).
> - **Verifier threshold source (trunk `40551113`):** [`verify/mod.rs`](../../../src-tauri/crates/anima-worldgen/src/verify/mod.rs), [`verify/g_shape.rs`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs), [`verify/g_comp.rs`](../../../src-tauri/crates/anima-worldgen/src/verify/g_comp.rs).

---

## 1. Source SHAs & Commands

- **Combined source commits (both reports merged locally with `git merge --no-ff --no-commit`, `0` conflicts, `10` staged files reported by `anima-case_BIG-COMBO-X_report.md`):**
  - **Base trunk (`origin/feat/ecological-selection`, including merged `#308` `WV2-BIG-2`):** `3b78b4b8b45d8d2640b4659849a6d7641d3fe55a` (`3b78b4b8`; both reports).
  - **Combined candidate (`origin/mac/wv2-big-1`, PR `#311` `WV2-BIG-1`, subsequently merged at `1b988167`):** `11e3b66901f1d338fe059c0d6d3006620069488f` (`11e3b669`; both reports).
- **Unit/integration suite & release build (`anima-case_BIG-COMBO-X_report.md` §1):**
  - `cargo test --manifest-path src-tauri/Cargo.toml -p anima-worldgen --features reader,generate,bake`: **`131 passed; 0 failed; 6 ignored`** across `8` test binaries (`wall = 230.14 s`, `EXIT 0`).
  - `--release` binary build (`anima-worldgen`): `8.78 s` (`EXIT 0`).
- **81.92 km generate + verify commands:**
  - `anima-mac_BIG-COMBO_report.md`: `./src-tauri/target/release/anima-worldgen generate --preset src-tauri/crates/anima-worldgen/presets/wv2_big_81920.toml --out /tmp/wv2_big_combo/raw` -> `./src-tauri/target/release/anima-worldgen verify --raw /tmp/wv2_big_combo/raw --report-only --out-sim ... --out-gates ... --out-index ...`
  - `anima-case_BIG-COMBO-X_report.md`: `anima-worldgen generate --preset src-tauri/crates/anima-worldgen/presets/wv2_big_81920.toml` -> `anima-worldgen verify --raw ... --report-only`

---

## 2. Cross-Platform `81.92 km` Identity, Performance & Macro Geometry

Both platforms produced bit-identical `81.92 km` (`4096² @ 20 m`, `seed = 1465266689 / 0x0000000057563201`, `schema_tag = "ANMW3-BIG-v1"`, `verifier_profile = "81.92km"`) outputs (`anima-mac_BIG-COMBO_report.md` §1; `anima-case_BIG-COMBO-X_report.md` §3):

| Metric | `anima-mac` (`macOS arm64`, `anima-mac_BIG-COMBO_report.md` §1) | `anima-case` (`Windows 1`, `anima-case_BIG-COMBO-X_report.md` §3) |
|---|---|---|
| `world_sha256` | `08549db56c551c54ba428359f14d8fce666bca68fed8ee4b8c639f9e464a16b0` (`08549db5`) | `08549db56c551c54ba428359f14d8fce666bca68fed8ee4b8c639f9e464a16b0` *(identical)* |
| `sim_world_identity` | `132fa5fa2ebfe9529412b17059e8e02d1d506632cf23cc1e7af80b3e930338ec` | `132fa5fa2ebfe9529412b17059e8e02d1d506632cf23cc1e7af80b3e930338ec` *(identical)* |
| `sim_256.anmw3` byte len | `1,445,860` B | `1,445,860` B |
| `verifier_profile` | `"81.92km"` | `"81.92km"` (`width=4096`, `height=4096`, `dx_m=20.0`, `extent_m=81920.0`) |
| `generate` wall time | `1,048.26 s` (`17m 28s` real; `1,429.16 s` user, `4.25 s` sys) | `2,122.88 s` (`EXIT 0`) |
| `generate` peak RSS | `1,126,334,464 B` (`1,074.2 MiB`) | *Not reported in `anima-case_BIG-COMBO-X_report.md`* |
| `verify` wall time | `4.40 s` real | `7.92 s` (`EXIT 0`) |
| `verify` peak RSS | `1,542,537,216 B` (`1,471.1 MiB`) | *Not reported in `anima-case_BIG-COMBO-X_report.md`* |
| Elevation `[min, max]` / clips | `[-1,333.84 m, +5,645.42 m]` / `0` low clips, `0` high clips | *Not separately printed in `anima-case_BIG-COMBO-X_report.md`* |
| Massifs & corridor saddle | North `4,071.77 m` (row `875`), South `5,645.42 m` (row `2,785`), Saddle `759.94 m` (row `1,710`) | *Not separately printed in `anima-case_BIG-COMBO-X_report.md`* |

---

## 3. Complete `15`-Gate Table (`"81.92km"` Profile)

Overall status on both platforms: **`8` passed, `7` failed** across the `15` gates (`overall_pass = false`; `anima-mac_BIG-COMBO_report.md` §2, `anima-case_BIG-COMBO-X_report.md` §3). Note `G-SIM.Raw` is counted as `1` row in the `15`-gate table in `anima-mac_BIG-COMBO_report.md` §2 (matching `GS-1` + `GS-2` in [`verify/mod.rs#L527-L538`](../../../src-tauri/crates/anima-worldgen/src/verify/mod.rs#L527-L538)).

| Gate | Status | `"81.92km"` Threshold ([`verify/`](../../../src-tauri/crates/anima-worldgen/src/verify/mod.rs)) | Observed (`anima-mac_BIG-COMBO_report.md` §2 & `anima-case_BIG-COMBO-X_report.md` §3) |
|---|---|---|---|
| `G-SHAPE.1.StrahlerHorton` | **PASS** | `omega_max >= 4`, `cycle_count == 0`, `R_b in [3.0, 5.0]`, `R_l in [1.5, 3.0]` ([`g_shape.rs#L598-L601`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs#L598-L601)) | `omega_max = 6`, `cycle_count = 0`, `r_b = 4.175390` (`4.175`), `r_l = 1.644491` (`1.644`), `counts = [0, 2176, 500, 123, 30, 7, 1]`, `mean_len_m = [0.0, 1104.47, 2447.89, 4315.89, 7281.45, 7701.95, 4951.03]` *(case / mac)* |
| `G-SHAPE.2.JunctionAngles` | **FAIL** | `junction_count >= 5`, `median_deg in [40.0, 78.0]`, `obtuse_fraction < 0.10`, `rain_shadow < windward` ([`g_shape.rs#L780-L783`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs#L780-L783)) | `junction_count = 380`, `median_deg = 75.018367` (`75.02°`), `obtuse_fraction = 0.126316` (`0.1263`, **FAIL** `>= 0.10`), `rain_shadow_mean_deg = 72.448929` (`72.45°`) `<` `windward_humid_mean_deg = 75.880844` (`75.88°`), `rain_shadow_samples = 93`, `windward_humid_samples = 258`, `rain_shadow_lt_windward = true` *(case / mac)* |
| `G-SHAPE.3.DrainageDensity` | **FAIL** | `global_dd in [0.8, 3.5] km/km²`, `dissection_ratio >= 1.5`, `caprock_tile_cv <= 0.45` ([`g_shape.rs#L946-L947`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs#L946-L947)) | `global_km_per_km2 = 1.242605` (`1.2426`), `dry_land_km2 = 3569.4744` (`3,569.47 km²`), `total_channel_km = 4435.4454` (`4,435.45 km`), `windward_mountain_km_per_km2 = 1.106244`, `plains_rain_shadow_km_per_km2 = 1.391802`, `dissection_ratio = 0.794829` (`0.7948`, **FAIL** `< 1.5`), `caprock_tile_count = 73`, `caprock_tile_cv = 0.714221` (`0.7142`, **FAIL** `> 0.45`) *(case / mac)* |
| `G-SHAPE.4.HackLaw` | **FAIL** | `sample_count >= 500`, `exponent_h in [0.52, 0.62]`, `prefactor_c_km in [1.0, 2.0]`, `r_squared >= 0.85` ([`g_shape.rs#L1067-L1070`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs#L1067-L1070)) | `sample_count = 2000`, `exponent_h = 0.605637` (`0.6056`), `prefactor_c_km = 1.559344` (`1.559 km`), `r_squared = 0.765826` (`0.7658`, **FAIL** `< 0.85`), `max_path_km = 47.616848` (`47.62 km`), `median_mainstem_sinuosity = 1.235296` (`1.235`) *(case / mac)* |
| `G-SHAPE.5.ValleysFlint` | **FAIL** | `b_up in [0.85, 1.35]`, `vf_up in [0.05, 0.80]`, `b_dn in [1.60, 3.50]`, `vf_dn >= 1.50`, `vf_ratio >= 2.5`, `theta in [0.35, 0.65]` (`flint_a_max_m2 = 1.2e9` in `81.92km`; [`g_shape.rs#L1364-L1371`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs#L1364-L1371), [`verify/mod.rs#L99`](../../../src-tauri/crates/anima-worldgen/src/verify/mod.rs#L99)) | `upper = 40`, `down = 40`, `b_up_median = 0.866341`, `vf_up_median = 1.101773` (`1.102`, **FAIL** `> 0.80`), `b_down_median = 1.178286` (**FAIL** `< 1.60`), `vf_down_median = 15.360963` (`15.361`), `vf_ratio_down_over_up = 13.942034` (`13.942`), `flint_bins = 18`, `flint_concavity_theta = 0.600256` (`0.6003`), `flint_r_squared = 0.931604` (`0.9316`) *(case / mac)* |
| `G-SHAPE.6.EndorheicSpillways` | **PASS** | `unmodeled_micro_pits == 0`, `severed_humid_spillways == 0`, `endorheic_land <= 0.10`, `endorheic_humid == 0.0` ([`g_shape.rs#L1405-L1408`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs#L1405-L1408)) | `unmodeled_micro_pits = 0`, `raw_non_water_land_pits = 0` (`raw_pits = 0`), `severed_humid_spillways = 0`, `endorheic_land_fraction = 0.000098` (`0.0000981` in mac report), `endorheic_humid_fraction = 0.0` *(case / mac)* |
| `G-SHAPE.7.HypsometrySpikes` | **FAIL** | `hi_global in [0.22, 0.55]`, `catchment_hi_median in [0.30, 0.60]`, `needle_spikes == 0` ([`g_shape.rs#L1582-L1583`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs#L1582-L1583)) | `hi_global = 0.158461` (`0.1585`, **FAIL** `< 0.22`), `catchment_count = 23`, `catchment_hi_median = 0.396944` (`0.3969`), `non_cliff_needle_spikes = 0`, `hillslope_p999_deg = 74.739620` (`74.74°`) *(case / mac)* |
| `G-SHAPE.4_2.ShorelineFractal` | **PASS** | `d_shore in [1.15, 1.45]` ([`g_shape.rs#L1669`](../../../src-tauri/crates/anima-worldgen/src/verify/g_shape.rs#L1669)) | `coastline_cells = 21996`, `box_sizes = [2, 4, 8, 16, 32, 64]`, `d_shore = 1.193864` (`1.1939`), `r_squared = 0.996781` (`0.9968`) *(case / mac)* |
| `G-COMP-v2.C1.Partition` | **PASS** | `ocean in [0.40, 0.50]`, `wf in 40..=200`, `caves in 60..=140`, `min_cave_spacing_m >= 400.0`, `invalid == 0`, `16/16` zoned biomes `>= 500,000 m²` (`1,250` cells @ `20 m`), `chaparral == 0`, `steppe == 0` ([`g_comp.rs#L963-L970`](../../../src-tauri/crates/anima-worldgen/src/verify/g_comp.rs#L963-L970), [`verify/mod.rs#L100-L105`](../../../src-tauri/crates/anima-worldgen/src/verify/mod.rs#L100-L105)) | `ocean_fraction = 0.449607` (`7,543,160` cells), `waterfall_count = 58`, `cave_entrance_count = 90`, `min_cave_spacing_m = 420.475921` (`420.48 m`), `min_required_cave_spacing_m = 400.0`, `invalid_cave_placements = 0`, `min_patch_cells_required = 1250`, `zoned_16_biomes_passing = 16`, `chaparral_cells = 0`, `steppe_cells = 0` *(case / mac)* |
| `G-COMP-v2.C2.Ecotone` | **PASS** | `ecotone_fraction in [0.15, 0.55]`, `non_cliff_jump_fraction < 0.03` ([`g_comp.rs#L1048`](../../../src-tauri/crates/anima-worldgen/src/verify/g_comp.rs#L1048)) | `land_cells = 9113285`, `ecotone_cells = 3864412`, `ecotone_fraction = 0.424042` (`0.4240`), `non_cliff_land_edges = 15837429`, `sharp_jump_edges = 162122`, `non_cliff_jump_fraction = 0.010237` (`0.01024`) *(case / mac)* |
| `G-COMP-v2.C3.Lakes` | **FAIL** | `lake_surface_fraction in [0.008, 0.045]`, `large >= 12`, `paramo >= 2`, `caldera >= 2`, `small_paramo_pond >= 20` ([`g_comp.rs#L1077-L1081`](../../../src-tauri/crates/anima-worldgen/src/verify/g_comp.rs#L1077-L1081), [`verify/mod.rs#L106-L109`](../../../src-tauri/crates/anima-worldgen/src/verify/mod.rs#L106-L109)) | `lake_surface_fraction = 0.004342` (**FAIL** `< 0.008`; raw basin stat `0.007889` / `0.0079`), `total_mapped_basins = 122`, `large_lake_count = 26`, `high_paramo_lake_count = 95`, `caldera_or_trough_lake_count = 23`, `small_paramo_pond_count = 95` *(case / mac)* |
| `G-COMP-v2.C4.RiversFansDeltas` | **FAIL** | `river_frac in [0.0005, 0.0030]`, `monotonic_w2_w3_w4 == true`, `hack_consistent == true`, `fan_count >= 6`, `delta_count >= 4`, `oxbow >= 1 \|\| meander_scroll > 0` ([`g_comp.rs#L302-L312`](../../../src-tauri/crates/anima-worldgen/src/verify/g_comp.rs#L302-L312), [`verify/mod.rs#L110-L112`](../../../src-tauri/crates/anima-worldgen/src/verify/mod.rs#L110-L112)) | `counted_river_surface_fraction = 0.001062`, `active_channel_cells = 189778`, `widths = [0.8245, 1.8445, 3.5201, 7.0508, 11.4060, 18.4548]`, `monotonic_w2_w3_w4 = true`, `hack_consistent = false` (**FAIL**), `fan_count = 149`, `delta_count = 9`, `oxbow_count = 0`, `meander_scroll_cells = 37765` *(case / mac)* |
| `G-COMP-v2.C5.CoastalSightlines` | **PASS** | `coast_cells >= 16`, `gentle >= 0.20`, `cliff >= 0.10`, `cliff <= 0.25`, `obstructed == 0` ([`g_comp.rs#L399-L403`](../../../src-tauri/crates/anima-worldgen/src/verify/g_comp.rs#L399-L403), [`verify/mod.rs#L113-L114`](../../../src-tauri/crates/anima-worldgen/src/verify/mod.rs#L113-L114)) | `coastline_cells = 21996`, `gentle_sandy_fraction = 0.792690` (`0.7927`), `steep_cliff_fraction = 0.104155` (`0.1042`), `sightline_checks = 8`, `obstructed_sightlines = 0` *(case / mac)* |
| `G-COMP-v2.C5b.CoastalPlainProfile` | **PASS** | `gated = true`, `5km >= 0.60`, `3km >= 0.75`, `median_coast_to_1000m >= 10.0 km`, `steep_mountain_front <= 0.20` ([`g_comp.rs#L787-L790`](../../../src-tauri/crates/anima-worldgen/src/verify/g_comp.rs#L787-L790), [`verify/mod.rs#L115-L119`](../../../src-tauri/crates/anima-worldgen/src/verify/mod.rs#L115-L119)) | `gated = true`, `passed = true`, `evaluated_coastline_cells = 19676`, `total_coastline_cells = 21996`, `excluded_low_islet_coast_cells = 2320`, `plain_backed_5km_fraction = 0.746087` (`0.7461`), `plain_backed_3km_fraction = 0.805143` (`0.8051`), `median_coast_to_1000m_km = 12.878315` (`12.878 km`), `steep_mountain_front_fraction = 0.080606` (`0.0806`) *(case / mac)* |
| `G-SIM.Raw` (`GS-1` & `GS-2`) | **PASS** | `round_trip_ok`, `max_diff <= 1e-5`, `all_biomes_in_range`, `water_mask_mismatches == 0`, `strahler_ge4_blocks > 0`, `block_mismatches == 0`, `disconnected_stems == 0`, `largest_walkable_fraction >= 0.80`, `walkable_shoreline_cells >= 500` ([`verify/mod.rs#L527-L535`](../../../src-tauri/crates/anima-worldgen/src/verify/mod.rs#L527-L535)) | `sim_seed_u32 = 1465266689`, `sim_world_identity = 132fa5fa2ebfe9529412b17059e8e02d1d506632cf23cc1e7af80b3e930338ec`, `sim_256_byte_len = 1445860`, `max_mean_norm_elev_diff = 2.9802322e-8`, `all_biomes_in_range = true`, `deep_ocean_cells = 24855`, `ocean_cells = 4873`, `water_mask_mismatches = 0`, `strahler_ge4_blocks = 1131`, `strahler_ge4_block_mismatches = 0`, `strahler_ge4_disconnected_stems = 0`, `walkable_cells = 35808`, `largest_walkable_component = 35662` (`35,808` in mac report summary), `largest_walkable_fraction = 0.995923` (`0.9959`), `walkable_shoreline_cells = 3455`, `round_trip_ok = true` *(case / mac)* |

---

## 4. Biome Composition (`E1..E9`) & `WB-D8` Coastline Mix

### 4.1 `16` Zoned Biomes (`C2` `land_cells = 9,113,285`) & Macro-Environments `E1..E9`

- **Individual biome shares (`anima-mac_BIG-COMBO_report.md` §3.1) & `total_cells / max_connected_cells` (`anima-case_BIG-COMBO-X_report.md` §3 `C1`):**
  - `Jungle`: `23.42%` (`2,134,130 / 840,371`)
  - `Grassland`: `18.72%` (`1,706,286 / 1,147,801`)
  - `Rock`: `10.34%` (`941,950 / 606,407`)
  - `Savanna`: `10.32%` (`940,847 / 436,449`)
  - `Forest`: `9.63%` (`877,387 / 605,180`)
  - `Badlands`: `7.60%` (`692,730 / 324,523`)
  - `Shrubland`: `5.12%` (`466,914 / 102,310`)
  - `Taiga`: `3.60%` (`328,317 / 123,229`)
  - `Mangrove`: `2.52%` (`229,362 / 129,180`)
  - `Bog`: `2.34%` (`213,596 / 58,195`)
  - `Glacier`: `2.24%` (`204,405 / 204,302`)
  - `Swamp`: `1.73%` (`157,900 / 8,699`)
  - `Desert`: `1.59%` (`144,967 / 19,208`)
  - `Snow`: `0.40%` (`36,256 / 35,681`)
  - `Alpine`: `0.33%` (`30,493 / 5,771`)
  - `Tundra`: `0.08%` (`7,745 / 3,447`)
- **Macro-environment groups (`anima-mac_BIG-COMBO_report.md` §3.1):**
  - **`E1` (Windward Tropical Humid — `Jungle + Swamp + Mangrove`):** **`27.67%`** *(design target `16–29%`)*
  - **`E2` (Rain-Shadow Dry — `Desert + Badlands + Savanna` / `+Shrubland`):** **`19.52%`** / **`24.64%`** *(target `14–26%`)*
  - **`E3` (Temperate Plains/Woodland — `Grassland + Forest`):** **`28.35%`** *(target `18–32%`)*
  - **`E4` (Wetlands & Deltas — `Mangrove + Swamp + Bog`):** **`6.59%`** *(target `4.5–10%`)*, `delta_count = 9`, `fan_count = 149`, `meander_scroll_cells = 37,765`
  - **`E5` (Montane & Nival — `Taiga + Alpine + Tundra + Rock + Snow + Glacier`):** **`17.00%`**
  - **`E6` (Lithology `%` of `9,234,042` subaerial cells):** `id=0: 16.72%`, `id=1 (caprock): 12.31%`, `id=2: 10.05%`, `id=3: 2.26%`, `id=4: 34.42%`, `id=5: 24.25%`
  - **`E7` (Karst & Lava Caves):** `90` entrances, `min_spacing = 420.48 m >= 400.0 m`, `0` invalid placements
  - **`E8` (Lakes / Waterfalls / Calderas):** `58` waterfalls (`73 m` window), `122` mapped basins (`26` large, `95` paramo, `23` caldera/trough)
  - **`E9` (Detached Offshore Islands):** `119` detached components (`7` high `>= 100 m`, `112` low `< 100 m`; `42,902` cells = `0.46%` of subaerial land; main landmass `9,191,140` cells = `99.54%`)

### 4.2 `WB-D8` Coastline Mix (`anima-mac_BIG-COMBO_report.md` §3.2)

> **Open owner decision (`QD-1126AM`, `AGENT_WORKFLOW.md:574`):** How `WB-D8`'s coast mix (`55–65%` Plain-Backed / `20–30%` Fan-Delta-Mangrove / `10–20%` Steep) is measured remains **open** for the owner. Both measurement windows reported in `anima-mac_BIG-COMBO_report.md` §3.2 using the `C5` shoreline predicate (`21,996` total coast cells; `19,676` `C5b` mainland cells excluding `2,320` low-islet coast cells) and central-difference steepness (`c5_slope >= 0.35`) are transcribed below:

| Classification Window | Plain-Backed Coast *(target `55–65%`)* | Fan / Delta / Mangrove *(target `20–30%`)* | Steep / Cliff `(>= 0.35)` *(target `10–20%`)* |
|---|---|---|---|
| **Exact shoreline cell (`0 m`, all `N=21,996`)** | **`81.82%`** (`C5 gentle = 79.27%`) | **`7.77%`** *(direct coast cells; sandy beach rim `Shallow` sits in front of mangroves)* | **`10.42%`** (`C5 PASS`) |
| **Mainland shoreline cell (`0 m`, `N=19,676`)** | **`81.05%`** (`C5 gentle = 78.65%`) | **`8.05%`** | **`10.90%`** |
| **`<= 40 m` (2-cell coastal band behind beach rim, all `N=21,996`)** | **`37.47%`** | **`52.12%`** *(1–2 cells behind beach rim intersects broader mangrove/alluvial apron)* | **`10.42%`** |
| **`<= 40 m` (`C5b` mainland `N=19,676`)** | **`35.26%`** | **`53.84%`** | **`10.90%`** |
| **`C5b` Inland Profile Summary (`19,676` mainland)** | **`74.61%` `@ 5 km`**, **`80.51%` `@ 3 km`**, `med = 12.88 km` | `9` deltas, `149` fans, `229,362` mangrove cells | **`8.06%`** `< 4 km` mountain front (`PASS`) |

---

## 5. `C01` Bit-Identity on the Combined Tree (`anima-case_BIG-COMBO-X_report.md` §2)

On the combined `3b78b4b8` + `11e3b669` tree, default `4096²` `C01` (`40.96 km @ 10 m`, `--seed 0x0057563220261120`, `verifier_profile = "40.96km"`, `schema_tag = "ANMW3-SNSM-v1"`) remained exact bit-identical (`anima-case_BIG-COMBO-X_report.md` §2):

- **Wall times:** `generate = 779.19 s` (`EXIT 0`), `verify = 7.52 s` (`EXIT 0`).
- **Hashes:**
  - `world_sha256`: `49f1d62b11fc598cd43161af967c4fd566962cf735fe2112c6c706b667644c28`
  - `sim_world_identity`: `079772566c4f111b500940103d023cc9a875dead153f924b722211d043a59fd2` (`sim_256_byte_len = 1,443,668`)
- **Gate summary (`overall_pass = false`, exact same `6` failed gates):**
  - **Failed (`6`):**
    - `G-SHAPE.2.JunctionAngles`: `junction_count=130`, `median_deg=56.2144`, `obtuse_fraction=0.0769`, `rain_shadow_mean_deg=60.4236`, `windward_humid_mean_deg=57.4365`, `rain_shadow_lt_windward=false`
    - `G-SHAPE.3.DrainageDensity`: `global_km_per_km2=1.3370`, `windward=1.8054`, `plains_rain_shadow=1.1363`, `dissection_ratio=1.5888`, `caprock_tile_cv=0.7462`
    - `G-SHAPE.4.HackLaw`: `h=0.6036`, `C=1.1891`, `R²=0.6056`, `max_path_km=16.3827`
    - `G-SHAPE.5.ValleysFlint`: `vf_up=0.5016`, `vf_down=0.8504`, `vf_ratio=1.6955`, `theta=0.4542`, `R²=0.6887`
    - `G-COMP-v2.C3.Lakes`: `lake_surface_fraction=0.002436` (`raw lakes=0.0044`), `total_mapped_basins=191`, `large=10`, `paramo=181`, `caldera_trough=24`, `small_paramo_pond=181`
    - `G-COMP-v2.C4.RiversFansDeltas`: `counted_river_surface_fraction=0.000746`, `active_channel_cells=97759`, `monotonic_w2_w3_w4=true`, `hack_consistent=false`, `fans=28`, `deltas=4`
  - **Passed (`9`, plus report-only `C5b`):**
    - `G-SHAPE.1`: `PASS` (`omega_max=4`, `cycle_count=0`, `r_b=3.5947`, `r_l=1.5808`)
    - `G-SHAPE.6`: `PASS` (`unmodeled_micro_pits=0`, `raw_non_water_land_pits=0`, `severed_humid_spillways=0`, `endorheic_land_fraction=0.0`, `endorheic_humid_fraction=0.0`)
    - `G-SHAPE.7`: `PASS` (`hi_global=0.3621`, `catchment_count=25`, `catchment_hi_median=0.4664`, `non_cliff_needle_spikes=0`, `hillslope_p999_deg=80.6175`)
    - `G-SHAPE.4.2`: `PASS` (`coastline_cells=23064`, `d_shore=1.182442`, `r_squared=0.995378`)
    - `C1`: `PASS` (`ocean_fraction=0.4425`, `waterfalls=27`, `caves=40`, `min_cave_spacing_m=410.4875`, `invalid_cave_placements=0`, `16/16` zoned biomes passing)
    - `C2`: `PASS` (`ecotone_fraction=0.4740`, `non_cliff_jump_fraction=0.003813`)
    - `C5`: `PASS` (`gentle_sandy_fraction=0.7010`, `steep_cliff_fraction=0.1845`, `obstructed_sightlines=0/8`)
    - `C5b` *(report-only, `gated=false`, `passed=false`)*: `evaluated_coastline_cells=17285`, `total_coastline_cells=23064`, `excluded_low_islet_coast_cells=5779`, `plain_backed_5km_fraction=0.0`, `plain_backed_3km_fraction=0.007000`, `median_coast_to_1000m_km=3.689119 km`, `steep_mountain_front_fraction=0.581429`
    - `G-SIM`: `PASS` (`walkable_cells=36386`, `largest_walkable_component=36238`, `largest_walkable_fraction=0.995933`, `walkable_shoreline_cells=1449`, `strahler_ge4_blocks=229`, `water_mask_mismatches=0`, `strahler_ge4_block_mismatches=0`, `strahler_ge4_disconnected_stems=0`, `round_trip_ok=true`)

---

## 6. Look-Only Preview Bake (`anima-mac_BIG-COMBO_report.md` §4)

Unpublished internal preview artifacts staged strictly in the hub `test_runs/` folder (`artifacts/` untouched):

- `anima-bake fields`: `1.76 s` real / `827,244,544 B` max RSS (`normal_sha = 558ebe99a8bf`, `hydro_sha = c4bb9ecce5dc`)
- `anima-bake lighting --backend cpu`: `2.37 s` real / `1,205,239,808 B` max RSS (`light_sha = 2fcb038b9ff9`, `hor_sha = 9d3e6ee9219e`)
- `anima-bake pack --release-tag big-combo-08549db5 --verify`: `17.90 s` real / `1,977,909,248 B` max RSS (`bc5_psnr = 51.05 dB`, `ssim = 0.99909`, `3,814` entries)
- **Hub `test_runs/` triple (verified via `anima-bake verify`, `EXIT 0`, `verified bundle_id=5cbd9f8728cd entries=3814 ktx2=3557 archive_sha=25475939e215`):**
  - `test_runs/anima-mac_big_combo_08549db5_tiles.tar.gz` — `187,531,078 B`, SHA-256 `25475939e215d8544c102ac4366993ad4534e63b61e4a1e2aadb87bedf9c356e`
  - `test_runs/anima-mac_big_combo_08549db5_index.json` — `936,066 B`, SHA-256 / `bundle_identity` `5cbd9f8728cd79f13ae2adf8eb7351c0d67fc626d75dae7c142e45997ecc5b1b`
  - `test_runs/anima-mac_big_combo_08549db5_sim_256.anmw3` — `1,445,860 B`, SHA-256 / `sim_world_identity` `132fa5fa2ebfe9529412b17059e8e02d1d506632cf23cc1e7af80b3e930338ec`
