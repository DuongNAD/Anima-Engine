---
phase: design
feature: map-v2
title: Card WV2-D2 — World v2 Scale-Up (~82 km) and Environments Design
description: Design proposal for scaling World v2 to ~81.92 km on a side with expanded physically coherent tropical environments, full code/verifier/viewer scale inventory, extent vs resolution trade-off, proposed gate recalibrations, release order options, and open owner decisions
status: proposed
owner: anima-dell
last_reviewed: 2026-11-26
---

# `WV2-D2` — World v2 Scale-Up (`~82 km`) and Environments Design

> **Status: `proposed` (documents only — zero code, zero generator/verifier/tile edits, zero `docs/ai/README.md` edits).**
> - **Provenance:** Code and doc anchors in this design were verified on `origin/feat/ecological-selection` at `81eb1e8ef39c6811b463d417984e9130ea03a258` (`#290` `C16 SEL2P2-FREEZE`, 2026-11-25), alongside pending decisions in Draft PR `#293` (`cf433685f78b9d596f9dd49bd29f7d33dcc4df1e`, `VERIFIED` at review), `WV2-PHYS3` (`PR #274` / `origin/mac/wv2-phys3`), and `WV2-6` (`Draft PR #291` / `origin/win/wv2-6` @ `cdcab9f0`, explicitly marked where cited).
> - **Gate authority:** Every new threshold, resolution choice, environment scope choice, or release-order recommendation in this document is **`PROPOSED`** and marked with **`WB-D1..WB-D8`** in [`§8`](#8-open-owner-decisions-wb-d1wb-d8).

---

## 1. Owner Direction & Unchanged Invariants

### 1.1 Exact Owner Direction (`QD-WV2-BIG`, PR `#293`)

On 2026-11-25 night, offered a choice between keeping the `40.96 km` World v2 footprint and fixing how it reads vs. option 2:

> *"make the map really larger, e.g. 82 km, four times the area"*

The owner answered:

> *"cách 2 đi và làm nhiều môi trường hơn"*

Under `QD-WV2-BIG` (`PR #293`, `docs/how-to/AGENT_WORKFLOW.md`), `MAP-D7` (replacing `C01` with a `40.96 km` candidate) is suspended while `WV2-PHYS4` continues on river, valley, and lake physics that scale with `dx_m` and `extent_m`.

### 1.2 What Stays Unchanged (`QD-WV2-BIG`)

1. **`LD-1` creature scale ([`2026-11-20-world-v2-design.md`](2026-11-20-world-v2-design.md#L188)):** Creatures remain simulated and rendered at their declared stylised scale (`simulation_loop.rs:1553,1559,1565`; `live_experiment.rs:2767`), decoupled from terrain metres per cell.
2. **The `256²` sim grid and `MapBounds` `200 × 200` ([`sim_rules.rs`](../../../src-tauri/src/core/sim_rules.rs#L47-L58)):** Stage 4 science experiments (`E3b`, `SEL2-P2`) use `TerrainSource::Generated` (`ecs.rs:64-75`) rather than `C01`, and the desktop `sim_256.anmw3` remains `256 × 256` mapped onto `[-100, +100]` sim units (`WORLD_MIN_XZ..WORLD_MAX_XZ = -100.0..100.0`, `WORLD_MIN_Y..WORLD_MAX_Y = 0.0..10.0`).
3. **`C01` as the provisional world ([`artifacts/world-v2/`](../../../artifacts/world-v2/index.json)):** `C01` (`sim_world_identity = cbdd3ba9...`, `MAP-D5`) stays committed until the scaled-up world passes all active gates and is explicitly approved.
4. **The tile pack published once ([`AGENT_WORKFLOW.md` `QD-GATES-1125 (4)`](../../how-to/AGENT_WORKFLOW.md)):** The ~145 MB+ GitHub Release tile archive is uploaded once for the final shipped world, never twice unless the owner explicitly chooses staged shipping in §6.

---

## 2. Inventory of Scale Assumptions (`file:line` Anchors)

All code below uses `40,960 m` (`40.96 km`) unless noted. Scaling to `81,920 m` (`81.92 km`, `4×` surface area) affects each subsystem differently depending on whether it reads `extent_m` / `dx_m` or embeds normalized box fractions (`[0, 1]`) and `40.96 km` constants.

### 2.1 `anima-worldgen` Core, Tectonics, Hydrology, Climate & Biomes

| Subsystem / Anchor | Current Value / Mechanism | Parameterised? | Effect at `81,920 m` (`4×` Area) |
|---|---|---|---|
| `src-tauri/crates/anima-worldgen/src/lib.rs:45` | `pub const WORLD_EXTENT_M: f32 = 40_960.0;` | **Hard-coded default constant** | Must remain `40_960.0` (or test-compatible) for legacy unit tests (`lib.rs:251`, M2 patch), while presets/config supply `81_920.0`. |
| `src-tauri/crates/anima-worldgen/src/config.rs:31,117,153-154,165-166` | `pub extent_m: f32` (default `WORLD_EXTENT_M`), `cell_size_m() = extent_m / grid_size as f32` | **Parameterised** | Cleanly computes `10.0 m` (`8192²`) or `20.0 m` (`4096²`); `bin/anima-worldgen.rs:50,338-339` already supports TOML/CLI `extent_m`. |
| `src-tauri/crates/anima-worldgen/src/tectonic.rs:209-214,318` | `RIFT_SEGMENT_LEN_M = 15_000.0`, `l_seg_norm = RIFT_SEGMENT_LEN_M / cfg.extent_m`, `STRAT_MIN_BED_THICKNESS_M = 100.0` | **Parameterised in metres** | Rift segment stays `15 km` (half the proportional map length at `81.92 km` unless deliberately extended for multiple rift basins). |
| `src-tauri/crates/anima-worldgen/src/tectonic.rs:256-331` | `smooth_coast = 0.45`, `ridge_center_u = smooth_coast + 0.22`, `width_asym = 0.098/0.115`, `seg_w = 0.032`, `du_plat / 0.268`, `dv_plat / 0.278` | **Hard-coded `[0, 1]` fractions** | **Silently doubles every landform width/spacing** (`~16 km` massif -> `~32 km` massif, `~15–20 km` caprock plateau -> `~30–40 km`). Must be refactored to explicit metre-based or multi-feature placement to hold sharp ridges and make room for new environments (§4). |
| `src-tauri/crates/anima-worldgen/src/pipeline.rs:140-159` | `compute_multigrid_schedule`: `&[(512, 65), (1024, 35), (2048, 20), (4096, 30)]`, `target_steps: 8192 => 8` | **Partially parameterised** | `lvl_dx = cfg.extent_m / lvl_u32 as f32` (`pipeline.rs:1426`) adapts per level. However, at `4096²` with `81.92 km` (`lvl_dx = 20.0`), lines `1464,1481` (`if lvl_dx <= 12.0` / `< 30.0`) shift timestep regime; at `8192²`, only `8` fine steps run on `8192` (`pipeline.rs:154`). |
| `src-tauri/crates/anima-worldgen/src/pipeline.rs:533-538` | Waterfall candidate `run_limit`: `30.0_f32.max(3.65 * dx_m)` when `dx_m > 20.0`, else `30.0`, `k_steps = (run_limit / dx_m).round().clamp(1, 3)` | **Partially parameterised** | **Critical boundary at `dx = 20.0 m` (`4096²` @ `81.92 km`):** `dx_m > 20.0` is false (`run_limit = 30.0 m`), so diagonal step `28.28 m` barely fits `<= 30 m` and 2-step drops fail (`>= 40 m`). Must use `dx_m >= 20.0` or `max(30.0, 2.5 * dx_m)`. |
| `src-tauri/crates/anima-worldgen/src/pipeline.rs:843-1040,1309-1315` | `apply_late_quaternary_piedmont_subsidence` (`12..320 m`, `drop_m <= 38 m`) and `clamp_opposing_needle_spikes` (`max_rev_dh = 2.8 * dx_m`) | **Parameterised in `dx_m` / `m`** | Needle clamp scales cleanly with `dx_m`; piedmont subsidence basin count/area may need tuning for `C3` at `81.92 km`. |
| `src-tauri/crates/anima-worldgen/src/lakes_falls.rs:55-80,1709` | `waterfall_min_drop_m = 12.0`, `waterfall_max_run_m = 30.0` (default) | **Hard-coded default (`30.0 m`)** | Same `dx = 20 m` bottleneck as `pipeline.rs:533` unless caller overrides `waterfall_max_run_m` to `>= 45–73 m`. |
| `src-tauri/crates/anima-worldgen/src/deposition.rs:314` | Fan/meander/delta simulation (`cfg.dx_m` checked `> 0`) | **Parameterised in `dx_m`** | Parcel walk lengths / fan footprints scale with `dx_m` and `Q`; wide lowland plains on `81.92 km` let meanders and deltas grow naturally. |
| `src-tauri/crates/anima-worldgen/src/caves.rs:351,369,403-406` | `dx_m: f32`, `30..=48` entrances, spacing `>= 0.01 * size` | **Parameterised in `dx_m`; fixed count** | At `81.92 km`, `0.01 * extent_m` doubles required spacing (`409.6 m -> 819.2 m`), while `30..=48` ceiling caps total caves across `4×` land area (§5). |
| `src-tauri/crates/anima-worldgen/src/climate.rs:36-39,74-82,108,444-445,567,587-600` | `dx_m = cfg.cell_size_m()`, `latitude_deg = 11.0`, `ela_base_m = 4_900.0` | **Parameterised in `dx_m`** | Orographic moisture advection and wind sweep already integrate over `dx_m`; longer `81.92 km` fetch strengthens the leeward rain shadow. |
| `src-tauri/crates/anima-worldgen/src/biomes.rs:379-389,538-800,1168,1379` | `dx_m = wcfg.cell_size_m()`, 5-cell smoothing proxies (`h_soft_m`, `t_soft`, `slope_soft_deg`) | **Mostly parameterised; cell-count smoothing** | Slope uses `2 * dx_m`; 5-cell box filter spans `50 m` at `10 m` vs `100 m` at `20 m`. `biomes.rs:1498` (`512²` test) gates `>= 40` Mangrove cells. |

### 2.2 Verifiers (`src-tauri/crates/anima-worldgen/src/verify/`)

| Gate / Anchor | Formula / Threshold | Parameterised? | Impact at `81,920 m` |
|---|---|---|---|
| `G-SHAPE.1` (`g_shape.rs:491,587-590`) | `omega_max >= 4`, `R_b in [3.0, 5.0]`, `R_l in [1.5, 3.0]` | **Dimensionless (scale-invariant)** | Unaffected; longer basins at `82 km` actually make `omega_max >= 4..6` easier. |
| `G-SHAPE.2` (`g_shape.rs:604,769-772`) | `junction_count >= 5`, `median_deg in [40, 78]`, `obtuse < 0.10`, `rs < ww` | **Dimensionless (scale-invariant)** | Unaffected; `82 km` gives larger sample counts (`n`). |
| `G-SHAPE.3` (`g_shape.rs:788,935-936`) | `global_dd in [0.8, 3.5] km/km²`, `dissection_ratio >= 1.5`, `caprock_tile_cv <= 0.45` | **Parameterised in `dx_m`** | Physical `km/km²` stays valid, though `20 m` cells miss `< 20 m` micro-rills (minor `D_d` shift). |
| `G-SHAPE.4` (`g_shape.rs:953,1056-1059`) | `sample_count >= 500`, `h in [0.52, 0.62]`, `C_km in [1.0, 2.0]`, `R² >= 0.85` | **Parameterised in `dx_m`** | Physical `L_km` and `A_km2` scale cleanly; larger catchments expand the `A` span for `R²`. |
| `G-SHAPE.5` (`g_shape.rs:1187,1351-1354`) | `b_up in [0.85, 1.35]`, `vf_up in [0.05, 0.80]`, `r_ref` valley width | **Parameterised in `dx_m`; `r_ref` sensitivity** | At `dx = 20 m`, narrow upland valleys near `r_ref = 65 m` are only ~3 cells wide; `vfu`/`vfd` calibration must account for `20 m` sampling. |
| `G-SHAPE.6` (`g_shape.rs:1375,1392-1395`) | `micro_pits == 0`, `severed == 0`, `endo_land <= 0.10`, `endo_humid == 0.0` | **Scale-invariant** | Unaffected. |
| `G-SHAPE.7` (`g_shape.rs:1408,1570`) | `hi_global in [0.22, 0.55]`, `catchment_hi in [0.30, 0.60]`, `needle_spikes == 0` | **Scale-invariant** | Unaffected (`clamp_opposing_needle_spikes` uses `dx_m`). |
| `G-SHAPE.4_2` (`g_shape.rs:1582,1656`) | Shoreline box-counting `D_shore in [1.15, 1.45]` | **Cell-grid box sizes** | Measured in cell octaves (`2..=128` cells); `4096² @ 20 m` preserves cell count (`4096`), `8192² @ 10 m` doubles coast cells. |
| `G-COMP-v2.C1` (`g_comp.rs:10,482,504-505,528-533`) | `ocean in [0.40, 0.50]`, `wf in [20, 80]`, `caves in [30, 48]`, `min_cave_spacing >= 0.01 * extent_m`, `MIN_PATCH_AREA_M2 = 250_000.0` (`0.25 km²`), `16/16` zoned biomes | **Mixed: area parameterised; counts hard-coded** | `250,000 m²` = `625` cells at `20 m` vs `2,500` cells at `10 m`; `wf [20, 80]` and `caves [30, 48]` are calibrated for `40.96 km` and under-represent `4×` area (§5). |
| `G-COMP-v2.C2` (`g_comp.rs:613`) | `ecotone in [0.15, 0.55]`, `non_cliff_jump < 0.03` | **Scale-invariant** | Unaffected. |
| `G-COMP-v2.C3` (`g_comp.rs:627-646`) | `lake_frac in [0.008, 0.045]`, `large_lake (> 100,000 m²) >= 5`, `paramo_lake >= 1`, `caldera >= 1`, `small_paramo_ponds (< 100,000 m²) >= 10` | **Parameterised in `m²`; counts fixed** | `100,000 m²` = `250` cells at `20 m` (`1,000` at `10 m`); `>= 5` large lakes is too small a floor for `82 km` (§5). |
| `G-COMP-v2.C4` (`g_comp.rs:238-242`) | `river_frac in [0.0005, 0.0030]`, `w2 < w3 < w4`, `hack_consistent`, `fan_count >= 2`, `delta_count >= 2` | **Fractions scale; counts fixed** | Longer catchments naturally produce more fans/deltas (`fan/delta >= 2` is trivial at `82 km`). |
| `G-COMP-v2.C5` (`g_comp.rs:319-322,358-367`) | `gentle >= 0.20`, `cliff >= 0.10`, `obstructed == 0` (top rim camera at `max_surface + 12_000 m`) | **Mostly scale-invariant; top cam Z fixed** | `40.96 km` vs `81.92 km`: camera rim uses normalized `(0.04..0.96) * wf`, while `+12_000 m` vertical offset scales cleanly (`+2,500/+2,800 m` rim views). |

### 2.3 Bake, `ANMW v3`, `WV2-7` Sim Mapping, Viewer, Capture & Tests

| Subsystem / Anchor | Current Value / Mechanism | Parameterised? | Impact at `81,920 m` |
|---|---|---|---|
| **Bake pyramid** (`bake/mod.rs:26,29,32,38,170-193`, `fields.rs`, `pack_ktx2.rs:1421`) | `TILE_ACTIVE_SIZE = 256`, `TILE_GUTTER = 2`, `HORIZON_HALO_CELLS = 256`; `compute_mip_pyramid(base_size)` loops down to `256²`; `pack_ktx2` writes `meta.extent_m` into `index.json` | **Parameterised** | `compute_mip_pyramid(4096)` yields `L0..L4` (`341` tiles/layer); `compute_mip_pyramid(8192)` already yields `L0..L5` (`1,365` tiles/layer). `HORIZON_HALO_CELLS = 256` covers `2.56 km` at `10 m` or `5.12 km` at `20 m` (exact sharded parity). |
| **`ANMW v3` binary** (`anmw_v3.rs:29,33,35,78-99`) | Fixed `44`-byte header (`width`, `height`, `sea_level_norm`, `sim_seed_u32`, `seed_u64`, `generator_version`, `world_scale = 200.0`, `flags`); **no `extent_m` field in header**; waterfalls/caves store `(x_m, z_m)` | **Partially parameterised** | As [`WORLD_SCALE_AND_MAP_GENERATION_RESEARCH_2026-11-22.md` §9](../../research/WORLD_SCALE_AND_MAP_GENERATION_RESEARCH_2026-11-22.md#L356-L378) notes, `sim_256.anmw3` lacks `world_extent_m_bits` in its `44`-byte header (`index.json` carries `extent_m`, while standalone `sim_256.anmw3` assumes `40_960` unless extended to 56 B or read from `index.json`). |
| **`WV2-7` sim mapping** (`sim_rules.rs:47,51-58`, `terrain.rs:89,97`, `world_artifact.rs:48`, `ecs.rs:10-31`) | `256²` sim grid (`DEFAULT_GRID_DIM = 256`) mapped onto `MapBounds = 200 × 200` (`CANONICAL_WORLD_SCALE = 200.0`) | **Scale-decoupled by `QD-WV2-BIG`** | At `81.92 km`, each `sim_256` cell represents **`320 m/cell`** (vs `160 m/cell` in `C01`), and `1` sim-unit = `409.6 m` (vs `204.8 m`). Coarse `block` projection in `anmw_v3.rs` preserves `LAND1` walkability invariants automatically (`GS-1`, `GS-2`), though river channels narrower than `320 m` remain sub-grid (`TerrainMap` blocks). |
| **Viewer (trunk)** (`WorldShowcase.tsx:84-100`, `floraClearance.ts:118`, `WorldWeather.tsx:58-61,119,132`) | `RENDER_SIZE = FLORA_RADIUS_REFERENCE_EXTENT = 1200`, `HEIGHT_RATIO = 0.14`, `TERRAIN_CHUNKED = false`, `TERRAIN_LOAD_RADIUS = 0`, linear `fogProfile` scaled by `worldScale` | **Fixed `RENDER_SIZE = 1200` box** | **Vertical exaggeration hazard:** `HEIGHT_RATIO = 0.14` means `1200 * 0.14 = 168` render units for `7,200 m` vertical span (`1` render unit = `42.9 m` vertical vs `34.1 m` horizontal at `40.96 km`, `1.26×` exaggeration). At `81.92 km`, `1` render unit = `68.3 m` horizontal, making `HEIGHT_RATIO = 0.14` a **`2×` steeper apparent relief** (`~2.5×` exaggeration if kept, or `HEIGHT_RATIO = 0.07` if true aspect ratio is preserved; `WB-D5`). |
| **Viewer (`#291` `win/wv2-6` @ `cdcab9f0`)** | `tileLoader.ts` (`extentM: index.extent_m`, `gridAtMip` dynamic; `wv2Mip` clamped & `macroAlbedoMip = Math.max(2, mip)` at `:74,233-234`); `WorldCaves.tsx:23,32-45` dynamic `extentM`; `types.ts:20-21` **`WORLD_EXTENT_M = 40960.0`** (unused) and **`FLORA_SECTORS_PER_SIDE = 16`** (`WorldV2Flora.tsx:78-79`, `worldV2Adapter.ts:187-188`); `WorldShowcase.tsx:541,1076` `'World v2 C01'` literal | **Mostly dynamic; `3` hard-coded items in `#291`** | For `4096² @ 20 m`, `FLORA_SECTORS_PER_SIDE = 16` (`4096/256`) and `L0..L4` still match! For `8192² @ 10 m`, `FLORA_SECTORS_PER_SIDE` must become `32` (`index.grid_size / 256`), `mip` range must allow `L0..L5`, and tile streaming (`TERRAIN_LOAD_RADIUS > 0` / frustum LRU) becomes mandatory for Iris Xe. |
| **Capture tools** (`mapManifest.ts:46-48,77,128`, `capture_map.mjs:451`) | `CANONICAL_XZ_EXTENT = 200`, `CANONICAL_MAX_Y = 10`, `CANONICAL_VIEW_CAMERAS` in `[-100, 100]` | **Coordinate-parameterised; poses fixed** | `canonicalCameraToRender` maps `[-100, 100]` into `RENDER_SIZE` cleanly, but the 8 canonical viewpoints (`overview`, `ecosystem`, `spawn`, etc.) must be re-aimed for the new `81.92 km` geography in `world_v2_manifest.json` (`map_manifest.json` untouched). |
| **Tests pinning `40,960` / `WORLD_EXTENT_M`** | `lib.rs:251` (`assert!((WORLD_EXTENT_M - 40_960.0)...)`), `scripts/paper/gate_faults/cargo_test_worldgen.patch:7`, `climate.rs:923,994,1116`, `wv2_1a_tectonic_tests.rs:5,316,326`, `wv2_2_climate_biome_cave_tests.rs:18,46,138,282`, `src-tauri/tests/world_v2_sim_gate_tests.rs:75,332` | **Pinned to `WORLD_EXTENT_M` constant** | Leaving `WORLD_EXTENT_M = 40_960.0` intact preserves every existing unit test and the `M2` `cargo_test_worldgen.patch` gate fault without touching any assertion; new `81.92 km` tests pass `extent_m: 81_920.0` explicitly. |

---

## 3. Extent and Resolution Options (`~81.92 km × 81.92 km`)

Baseline measurements on `4096²` (`40.96 km`, `10 m/cell`, MacBook Air M5 `RAYON_NUM_THREADS=4` unless noted):
- **Pack archive size (`C01`):** **`144,730,577 B` (`138.0 MiB`)**, `3,792` tar entries / `3,535` KTX2 files across `L0..L4` (`341` tiles per full layer; [`2026-11-24-wv2-5.md:47,51`](../planning/2026-11-24-wv2-5.md#L47-L51)).
- **Generate time & peak RSS (`4096²`):** `539.64 s` / `1,207.5 MiB` (`C01`, [`wv2-4.md:84`](../planning/2026-11-24-wv2-4.md#L84)); `471.46 s` / `1,183.4 MiB` (`PHYS2 S2_1`, [`wv2-phys2.md:126`](../planning/2026-11-24-wv2-phys2.md#L126)); `548.56 s` / `1,174.7 MiB` (`PHYS2 S2_2`); verify `3.48–3.57 s` / `1,470–1,471 MiB` (`wv2-phys2.md:127`).
- **Bake time & peak RSS (`C01` `4096²`):** `fields` `1.37 s` (`791.9 MiB`) + `lighting --backend cpu` `2.16 s` (`1,138.3 MiB`) + `pack --verify` `13.58 s` (`1,804.6 MiB`) = **`17.11 s` bake**, **`1,804.6 MiB` peak RSS** (`2026-11-24-wv2-5.md:44-47`).

> **Honesty note:** Neither `8192²` generation/bake nor `81.92 km` new-environment generation has been measured in this repo yet. Rows marked *(est., to measure)* below scale directly from the measured `4096²` baselines (`4×` pixel count for `8192²`, vs `1×` pixel count for `4096² @ 20 m`).

| Dimension | Option A: `8192² @ 10 m/cell` (`81.92 km`) | Option B: `4096² @ 20 m/cell` (`81.92 km`) |
|---|---|---|
| **Physical span & cell count** | `81,920 m × 81,920 m`, `67.11M` cells (`4×` pixels) | `81,920 m × 81,920 m`, `16.78M` cells (`1×` pixels) |
| **Tile pyramid** | `L0` (`32×32`)..`L5` (`1×1`), **`1,365` tiles/full layer** (`4.00×` `4096²`) | `L0` (`16×16`)..`L4` (`1×1`), **`341` tiles/full layer** (`1.00×` `C01`) |
| **Tile pack size** *(scaled from `144,730,577 B`)* | **`~550–580 MB`** (`~525–553 MiB`, est., to measure; flora & sparse wet layers vary with land/river share) | **`~140–155 MB`** (`~134–148 MiB`, est., to measure; matches `C01` `138.0 MiB`) |
| **Generate wall time & peak RSS** *(M5, `4` threads)* | **`~31–37 min`**, **`~4.7–5.1 GiB` peak RSS** (est., to measure; `4×` memory + >`4×` flow/erosion cost unless `8192` steps stay `<= 8`) | **`~8–10 min`**, **`~1.2–1.4 GiB` peak RSS** (est., to measure; same `16.78M` cells as `471–549 s` / `1.2 GiB` measured, +multi-massif cost) |
| **Bake wall time & peak RSS** *(unsharded M5)* | **`~68–80 s`**, **`~7.2 GiB` peak RSS** (est., to measure; requires `--tile-strip` / `--tile-range` sharding on 16 GB hosts) | **`~17–20 s`**, **`~1.8 GiB` peak RSS** (est., to measure; fits trivially in one pass) |
| **Dell Iris Xe (`16 GB` shared, `768 MiB` GPU ceiling) streaming** | **High risk without true tile streaming:** `WV2-6` (`#291`) currently loads all mip-level tiles at once (`tileLoader.ts`). At `L2` (`2048²`) it still fits, but `L1` (`4096²`) or `L0` (`8192²`) requires dynamic LRU frustum chunk streaming (`TERRAIN_LOAD_RADIUS > 0`), plus a `~560 MB` user download. | **Drop-in fit:** Identical tile count (`341` per layer) and GPU memory footprint to `C01` (`L2 = 1024²` on Low, `L1 = 2048²` on High), keeping `<= 768 MiB` GPU memory on Iris Xe with `WV2-6`'s existing loader. |
| **Near-camera geometric detail** | **`10 m/px`** height & `normal_xy`: sharp `10 m` sea cliffs, river banks, and gully walls; matches USGS `10 m` DEMs. | **`20 m/px`** height & `normal_xy`: `2×` wider minimum terrain step (`20 m`), softening small 10 m chutes/knickpoint lips unless masked by `WV2-LOOK` procedural micro-normals/splatting. |

### Recommendation (`WB-D1`): Option B (`4096² @ 20 m`) for `v0.9.0` / P1, with `8192² @ 10 m` Deferred to P2/P3

1. **Zero release blocker on Dell Iris Xe & download size:** Option B keeps the public GitHub Release tile archive at **`~145 MB`** (`1×` `C01`), keeps `generate` under `~10 min` / `1.3 GiB` (allowing rapid overnight `WV2-PHYS4` iteration on Mac & Windows 1 without OOM risk), and fits `WV2-6`'s non-streaming mip loader (`L0..L4`, `341` tiles/layer) without re-architecting Three.js tile LRU eviction in P1.
2. **Visual gap mitigated by `WV2-LOOK`:** Under `QD-WV2-LOOK`, distance-gated fine noise, triplanar steep-face rock shading, and real-scale flora stands supply sub-`20 m` surface detail on top of a `20 m` DEM.
3. **Clean upgrade path to Option A (`8192² @ 10 m`):** Because `anima-worldgen`'s `bake::compute_mip_pyramid`, `ANMW v3`, and `tileLoader.ts` (`gridAtMip`) already generalize to powers of two, upgrading the same `81.92 km` preset from `grid_size = 4096` to `8192` in Phase P2/P3 (`NR-SPIKE` native `wgpu` CDLOD/clipmap renderer) requires zero architectural throwaway work.

---

## 4. Physically Coherent Environment Set (`~11°N` Fixed Tropical Latitude)

Under `LD-1` corrected (`2026-11-20-world-v2-design.md:182,187`), the world sits at a **fixed `11.0°N` tropical maritime latitude** (`climate.rs:76`, `ela_base_m = 4,900 m`) inspired by the Sierra Nevada de Santa Marta / northern Andes. Expanding from `40.96 km` to **`81.92 km`** (`~3,700–4,300 km²` dry land at `0.40..0.50` ocean fraction) doubles cross-island wind fetch and creates space for multiple interacting landforms without breaking climatic coherence.

### 4.1 Environment Inventory, `G-REF` Mapping & Generator Mechanisms

| # | Environment | `G-REF` Group(s) | Generator Mechanism (`anima-worldgen`) | Existing vs. New | Proposed Land Share |
|---|---|---|---|---|---|
| **E1** | **Windward Tropical Rainforest & Montane Cloud Forest** | **3** (`RNF-01..03`), **4** analog (`TMP-*`), **13** (`ATM-01..04`) | Easterly/seaward orographic advection (`climate.rs`), `Jungle` (`7`) + `Forest` (`6`) + `Shrubland` (`5`) | **Existing** | `~20–24%` |
| **E2** | **Rain-Shadow Dry Side (Thorn Scrub, Tropical Savanna, Dry Valley & Coastal Dunes)** | **1** (`DES-01..04`), **2** (`SAV-01..03`) | Doubled `82 km` mountain barrier dries leeward air (`climate.rs` advection); `Desert` (`2`), `Savanna` (`3`), `Badlands` (`19`), coastal wind-blown `dune_h` (`biomes.rs`) | **Existing + tuned** (stronger dry-leeward valley behind dual massifs) | `~18–22%` |
| **E3** | **Lowland Alluvial Plain with Meanders & Oxbows** | **10** (`RIV-01,04`), **12** (`AER-03,07`) | Wide low-slope piedmont apron (`piedmont_gate` widened at `82 km`) + `deposition.rs` (`meander_scroll` point bars + **new** cutoff oxbow pools in low-gradient reaches) | **Partially existing + NEW** (wider plain + oxbow cutoff pools) | `~14–18%` |
| **E4** | **Estuarine Deltas, Mangrove Coast & Freshwater Swamps** | **7** (`WET-01..04`), **12** (`AER-04,05`) | `DeltaRCM` (`deposition.rs`) at high-`Q` mouths + `Mangrove` (`15`) / `Swamp` (`10`) / `Bog` (`21`) (`biomes.rs`), naturally expanded by `4×` catchment area | **Existing** (benefits directly from larger rivers) | `~6–9%` |
| **E5** | **Tectonic Rift & Glacial/Piedmont Lakes + Waterfalls** | **10** (`RIV-02,03`), **11** (`LAK-01..03`) | `tectonic.rs` segmented normal-fault rift (`15 km` segments, optionally 2 basins), `erosion_glacial.rs` cirque/moraine lakes, `pipeline.rs` piedmont subsidence, `lakes_falls.rs` sandstone knickpoint falls (`run_limit >= 45 m` at `20 m` dx) | **Existing + `dx`-fix** | `~2–4%` |
| **E6** | **Dissected Sandstone Caprock Plateau, Canyons & Karst Caves** | **10** (`RIV-04`), **12** (`AER-01,06`) | `tectonic.rs` dipping `SandstoneCaprock` (`lithology == 1`) tableland re-parameterised in metres (`~18–22 km` wide instead of `0.268`/`0.278` doubling to `44 km`) + `caves.rs` karst/basalt networks | **Existing + metre-fixed** | `~10–14%` |
| **E7** | **Primary Coastal Massif (`~5,600 m`) + Secondary Massif (`~3,500–4,200 m`)** | **8** (`MTN-01..06`), **13** (`ATM-01..04`) | Primary `sech²` massif (`tectonic.rs:288-310`) + **NEW** offset secondary volcanic/tectonic ridge (`~3.5–4.2 km` elevation) separated by an intermontane rift/alluvial corridor | **Primary existing + NEW secondary massif** | `~12–16%` |
| **E8** | **High-Altitude Páramo, Alpine Rock, Snow & Tropical Glaciers** | **6** (`TUN-03` alpine analog), **8** (`MTN-01..05`) | Elevation lapse rate (`climate.rs`) above treeline into `Alpine` (`18`), `Tundra` (`9`, alpine páramo band), `Rock` (`11`), `Snow` (`12`), `Glacier` (`20`, `ela_base_m = 4,900 m`) | **Existing** (second massif adds sub-glacial páramo at `3,300–4,100 m`) | `~5–8%` |
| **E9** | **Diverse Coastline, Offshore Islands & Sea Stacks** | **9** (`CST-01..04`) | `tectonic.rs` rocky cliff sectors + sandy beaches + **NEW** submerged-ridge offshore islands & sea-stack pinnacles near `d_coast < 0`, paired with `WV2-LOOK` surf/foam/wet-sand (`CST-01..04`) | **Shore existing + NEW offshore islands/stacks** | `~3–5%` |

*(Total land share sums to `~100%` of subaerial land; ocean remains `40–50%` of the `81.92 km` domain.)*

### 4.2 What One Fixed Tropical Latitude (`~11°N`) **Cannot** Honestly Produce (`WB-D2`)

An `81.92 km` square spans less than **`0.74°` of latitude** (`81.92 / 111.32 km/deg`). Under physical climate rules (`climate.rs:76` `latitude_deg = 11.0`, fixed tropical lapse rate):
- **True sea-level Boreal / Taiga forest (`G-REF` Group 5 `TAI-01..03`):** Needs cold-continental winter seasonality (`MAT ~ -5..+5°C` at low elevation), which does not occur at sea level at `11°N`. In `C01`/`biomes.rs`, `Taiga` (`8`) only appears as a narrow mid-montane conifer band (`~2,200–3,200 m`) — an honest tropical montane analog, not lowland boreal spruce/muskeg.
- **True Arctic Permafrost Tundra & Arctic Wetland (`G-REF` Group 6 `TUN-01,02`, Group 12 `AER-08`):** Requires polar insolation and permafrost thaw polygons; high-altitude `Tundra` (`9`) at `11°N` represents tropical **páramo** (`TUN-03` mountain-tundra analog), not lowland Arctic coast.
- **Four-season Temperate Deciduous Forest (`G-REF` Group 4 `TMP-02,03` autumn colour):** Tropical montane cloud forest (`Forest` `6`) matches evergreen/montane mist ridges (`TMP-01,04` look-alikes), not temperate autumn foliage.

**Owner decision (`WB-D2`) options:**
- **(a) Recommended:** Keep the coherent `~11°N` tropical super-massif (`E1–E9`, honest montane conifer/páramo analogs) and defer true boreal/temperate/arctic biomes to a **second regional preset** (`latitude_deg = 58°N`, e.g. post-`v0.9.0` world pack).
- **(b)** Add a synthetic N-to-S latitude temperature offset inside `climate.rs` (breaks physical realism at `82 km` where `Δlat < 0.75°`, NOT recommended).
- **(c)** Build and ship two presets/tile-packs before `v0.9.0` (doubles bake, QA, and download size, NOT recommended).

---

## 4b. Coastal Profile (Mountain -> Plain -> Sea)

Inspecting `C01`'s coast on 2026-11-25 night, the owner directed (`MSG_20261125_162000`):

> *"hình như trong tự nhiên núi xuống đồng bằng mới xuống biển chứ nhỉ"*
> (In nature, mountains come down to a plain before the sea.)

### 4b.1 Requirement & Geomorphic Targets (`PROPOSED`, `WB-D8`)

1. **Dominant profile (`mountain -> piedmont (alluvial fans) -> coastal/alluvial plain -> beach`):** Most of the `81.92 km` coastline must step down from highland ridges through coalescing alluvial fans (`deposition.rs`) and low-relief passive-margin coastal or alluvial plains before meeting sandy beaches, barrier lagoons, or river deltas.
2. **Minority steep mountain-to-sea coasts:** Steep coastal escarpments dropping straight into the surf are physically real — specifically `CST-03` (Black Sand Beach / Lost Coast, King Range NCA, California, [`2026-11-20-map-v2-reference-board.md`](2026-11-20-map-v2-reference-board.md#L534-L541)) and **Tayrona** along the northern flank of the Sierra Nevada de Santa Marta (while the rest of Santa Marta's periphery opens onto broad Caribbean coastal lowlands and the Ciénaga Grande / Magdalena delta). Steep/cliffed mountain-to-sea segments must stay a localized minority rather than wrapping most of the landmass.

| Coastal Segment Class | Example Analog (`G-REF`) | `PROPOSED` Coastline Share (`WB-D8`) |
|---|---|---|
| **Plain-backed passive-margin / lowland coast** | Broad coastal plain, sandy strandplains (`CST-01`, Ciénaga lowlands) | **`55–65%`** |
| **Alluvial fan / estuarine delta / mangrove-lagoon coast** | Piedmont fans coalescing to delta plain, Magdalena / Ciénaga mouths (`WET-01..04`, `AER-04,05`) | **`20–30%`** |
| **Steep / cliffed mountain-to-sea coast** | Lost Coast (`CST-03`), Tayrona headlands (`CST-02,03`) | **`10–20%`** *(strict minority)* |

### 4b.2 Generator Mechanisms That Build the Plain

To produce `75–90%` plain- or fan/delta-backed coastline while retaining a `10–20%` `CST-03`/Tayrona minority:
- **Sediment deposition (`src-tauri/crates/anima-worldgen/src/deposition.rs`, Stage S4 — tied directly to failing `G-COMP-v2.C4`):** Coalescing piedmont alluvial fans (`fan_slope_drop_ratio = 4.0`), Howard-Knutson lateral meandering floodplains (`meander_max_slope = 0.004`), and `DeltaRCM` prograding coastal deltas (`delta_max_shelf_depth_m = 40.0 m`, scaling `delta_max_mouths` beyond `4` at `81.92 km`) prograde sedimentary aprons across the low-gradient coastal margin once upstream `C4` river discharge/order reaches target.
- **Passive-margin flank (`tectonic.rs:372-392`):** Broaden the coastal `plain_ramp` (`d_coast`) around passive/lateral flanks of the landmass so a low-elevation continental shelf and `< 100 m` coastal lowland surround the main massif rather than mirroring a single coast-parallel ramp.
- **Lower uplift away from the massif (`tectonic.rs:288-310`, `generate_uplift_for_target`):** Replace fixed `[0, 1]` offsets (`ridge_center_u = smooth_coast + 0.22`, `piedmont_gate` onset `smooth_d_coast = 0.092`) with spatially varying metre setbacks (`>= 10–18 km` across plain-backed sectors, narrowing to `~1–3 km` only in the `10–20%` `CST-03`/Tayrona cliff arcs), tapering bedrock uplift to near zero across the lowland apron and restricting `coast_cliff_share` (presently `0.28` globally, proposed `~0.14–0.20`) to the headland minority.

### 4b.3 Checkable Metric (`PROPOSED`, `WB-D8`, No Verifier Change) & Current `C01` Status

- **`C5b.CoastalPlainProfile` (`PROPOSED` for `WV2-BIG-2`):**
  1. Share of coastline cells backed inland by **`>= 5.0 km`** of land below `100 m` elevation (`>= 3.0 km` at `40.96 km`): **`>= 0.60`** (and **`>= 0.75`** backed by `>= 3.0 km` `< 100 m` including fans/deltas).
  2. Median shortest horizontal coast-to-`1,000 m` elevation distance: **`>= 10.0 km`** at `81.92 km` (with `< 4.0 km` mountain-front segments capped at `10–20%`).
  3. Existing `C5` immediate shoreline checks (`gentle >= 0.20`, `cliff >= 0.10`, `obstructed == 0`) remain active, with a **`PROPOSED cliff <= 0.25`** ceiling so cliffed headlands stay a minority.
- **What `C01` (and `WV2-PHYS2`/`WV2-PHYS3`) has today:**
  - **Committed `C01` ([`artifacts/world-v2/gates.json`](../../../artifacts/world-v2/gates.json#L274-L307), [`2026-11-24-wv2-4.md:106-107`](../planning/2026-11-24-wv2-4.md#L106-L107)):** Immediate 1-cell shoreline `C5` reports `coastline_cells = 4,224`, `gentle_sandy_fraction = 1.0000` (`slope < 0.18`), `steep_cliff_fraction = 0.0000` (**FAIL**, zero coastal cliffs), `obstructed_sightlines = 0`, `D_shore = 0.9872` (**FAIL**); `C4` **FAILS** (`counted_river_surface_fraction = 0.000250`, `omega_max = 3`, `max_path_km = 9.89`, `fan_count = 4`, **`delta_count = 1`**). In [`tectonic.rs:288,304-310`](../../../src-tauri/crates/anima-worldgen/src/tectonic.rs#L288-L310), `ridge_center_u = smooth_coast + 0.22` (`~9.0 km` at `40.96 km`) and `piedmont_gate` begins uplifting the `5,350 m` massif at `smooth_d_coast = 0.092` (`~3.77 km` from `smooth_coast`).
  - **`WV2-PHYS2` (`S2_1`/`S2_2`) & `WV2-PHYS3` (`S2_6`):** `C5` immediate shoreline passes (`gentle = 0.722..0.775`, `cliff = 0.103..0.153`, `coast_cliff_share = 0.28`) and `C4` produces `26..35` fans / `4` deltas (`hack_consistent` still `false`), using the same `0.092` / `0.22` massif setback.
  - **Coastline fraction with `>= N km` below `100 m` behind it (`C01` / `PHYS2` / `PHYS3`):** **`to measure`**.
  - **Median coast-to-`1,000 m` horizontal distance (`C01` / `PHYS2` / `PHYS3`):** **`to measure`**.

---

## 5. Scale-Dependent Gate Recalibrations (`PROPOSED`, No Verifier Edits Yet)

All proposals below assume **`81,920 m × 81,920 m`** and Option B (**`4096²`, `dx = 20 m`**; parenthetical notes for `8192²`, `dx = 10 m`). Verifier files under `src/verify/**` remain untouched in `WV2-D2`.

| Gate | Trunk (`40.96 km`, `4096²`) | Scale Dependency | `PROPOSED` Target (`81.92 km`, `4096² @ 20 m`) |
|---|---|---|---|
| `G-SHAPE.1`, `.2`, `.6`, `.7`, `C2`, `GS-1`, `GS-2` | `g_shape.rs`, `g_comp.rs` | **Scale-invariant** (ratios, angles, counts `== 0`, grid parity) | **Unchanged** |
| `G-SHAPE.3` (`DrainageDensity`) | `dd in [0.8, 3.5]`, `dis >= 1.5`, `capCV <= 0.45` | Metric `km/km²` invariant; `20 m` misses tiny channels | **Keep `[0.8, 3.5]` & `<= 0.45`** (verify `Q_min` channel threshold at `dx = 20 m` in `WV2-BIG-2`) |
| `G-SHAPE.4` (`HackLaw`) | `samples >= 500`, `h in [0.52, 0.62]`, `C_km in [1.0, 2.0]`, `R² >= 0.85` | Physical `km/km²`; `4×` area yields larger trunks | **Keep thresholds** (expect higher `samples`, e.g. `>= 1,000` diagnostic) |
| `G-SHAPE.5` (`ValleysFlint`) | `b_up in [0.85, 1.35]`, `vf_up in [0.05, 0.80]`, `r_ref = 65 m` | At `dx = 20 m`, `r_ref = 65 m` spans only ~3 cells | **Parameterise `r_ref = max(65.0, 4.0 * dx_m)`** (`80 m` at `20 m`) or keep `65 m` (`to measure`) |
| `G-SHAPE.4_2` (`ShorelineFractal`) | `D_shore in [1.15, 1.45]` | Grid-box fractal dimension | **Unchanged** (offshore islands + ria coasts must stay `in [1.15, 1.45]`) |
| `G-COMP-v2.C1` (`Partition`) | `ocean [0.40, 0.50]`, `wf [20, 80]`, `caves [30, 48]`, `spacing >= 0.01 * extent_m`, `MIN_PATCH_AREA_M2 = 250,000` (`0.25 km²`), `16/16` biomes | Counts & `0.25 km²` patch floor assume `40.96 km` | **`PROPOSED` (`WB-D3`):** `wf` **`[40, 200]`** (scale-proportional `~2–3×`), `caves` **`[60, 140]`**, `spacing >= 400 m` (fixed in metres, not `0.01 * extent_m`), `MIN_PATCH_AREA_M2` **`500,000 m²` (`0.50 km²`, `1,250` cells @ `20 m`)**, `16/16` biomes (keep `Taiga` only if montane band remains in `ZONED_TERRESTRIAL_16`). |
| `G-COMP-v2.C3` (`Lakes`) | `lake_frac in [0.008, 0.045]`, `large (>0.1 km²) >= 5`, `paramo >= 1`, `caldera >= 1`, `ponds >= 10` | Absolute counts scale sub-linearly/linearly with area | **`PROPOSED` (`WB-D3`):** `lake_frac` **`[0.008, 0.045]`**, `large >= 12`, `paramo >= 2`, `caldera >= 2` (dual massif/rift), `ponds >= 20`. |
| `G-COMP-v2.C4` (`RiversFansDeltas`) | `river_frac [0.0005, 0.0030]`, `w2 < w3 < w4`, `fans >= 2`, `deltas >= 2` | Absolute counts assume `40.96 km` | **`PROPOSED` (`WB-D3`):** `river_frac [0.0005, 0.0030]`, `fans >= 6`, `deltas >= 4`, plus oxbow/meander presence check. |
| `G-COMP-v2.C5` (`CoastalSightlines`) | `gentle >= 0.20`, `cliff >= 0.10`, `obstructed == 0` | Rim camera offset `+12_000 m` top view | **Keep `gentle`/`cliff`/`obstructed == 0`** (`PROPOSED` `cliff <= 0.25` cap); set top view Z `+24_000 m` when `extent_m >= 80_000` so full `82 km` fits in FOV. |
| `C5b.CoastalPlainProfile` *(new)* | *(not gated in trunk)* | Plain-backed vs steep mountain-to-sea profile (`§4b`) | **`PROPOSED` (`WB-D8`):** `>= 0.60` coastline with `>= 5.0 km` `< 100 m` (`>= 0.75` @ `>= 3.0 km`); median coast-to-`1,000 m` distance `>= 10.0 km` (`10–20%` steep minority). |

---

## 6. Release Order Options

| Axis | Option (a): `WV2-6`, Tile Release & `v0.9.0-rc.1` Wait for `~82 km` World | Option (b): Ship `C01` (`40.96 km`) in `v0.9.0` First, Then Upgrade to `~82 km` |
|---|---|---|
| **Time to `v0.9.0-rc.1`** | Adds **`~1.5–2.5 weeks`** (WV2-BIG-1..6 + calibration + `WV2-LOOK` + bake + Iris Xe gate) before RC cut. | Faster RC (`WV2-6` `#291` is already `VERIFIED` code-side; needs only `C01` tile publish + Iris Xe gate), but spends `1` full release/trial cycle on a `2/15`-gate clipped map (`C01`). |
| **Public tile pack uploads (`QD-GATES-1125 (4)`)** | **`1` upload** (`~145 MB` for the final `82 km` world; matches `"published once, for the final world"`). | **`2` uploads** (`~145 MB` for `C01` in `0.9.0`, then another `~145–560 MB` for `82 km` in `0.9.x`/`1.0`), directly contradicting `QD-WV2-BIG` & `QD-GATES-1125 (4)` unless waived. |
| **`WorldIdentity` save refusals (`WV2-7`)** | **`0` save breakages** between `0.9.0` and the `82 km` world (`sim_world_identity` changes before first public release). | **Refuses every `0.9.0` desktop save** upon upgrading to `82 km` (`WorldIdentity::mismatch_against` in `world_artifact.rs:230-238`, enforced loudly by `WV2-7`). |
| **Engineering workflow** | `#291` (`WV2-6`) code can still be merged or tested internally via `QD-TRIAL-V3` while holding the public tile release & `v0.9.0-rc.1` tag until the `82 km` bake lands. | Ships known `C01` flaws (514 summit-clipped cells, small world, missing environments) to trial users first. |

### Recommendation (`WB-D4`): Option (a) — Hold Public Tile Release & `v0.9.0-rc.1` for the `~82 km` World

Option (a) honors the single-publication rule (`QD-GATES-1125 (4)` & `QD-WV2-BIG`), avoids breaking every trial user's `0.9.0` save via `WorldIdentity`, and ships the owner's chosen `82 km` multi-environment world directly in `v0.9.0`. Meanwhile, `#291` (`WV2-6`) viewer code and `WV2-LOOK` can continue iterating internally using `QD-TRIAL-V3` local tile staging.

---

## 7. Implementation Card Breakdown (`WV2-BIG-1..WV2-BIG-6`)

| Order | Card | Owner / Machine | Scope & Key Surfaces | Est. Size | Mandatory Verification & Negative Control |
|---|---|---|---|---|---|
| **1** | **`WV2-BIG-1`** Tectonic & Multi-Environment Refactor | `anima-mac` | Refactor `tectonic.rs` normalized box fractions to metre-based multi-massif, intermontane valley, dry leeward basin, wide piedmont/passive-margin coastal plain setback (`§4b`), localized `CST-03`/Tayrona cliff minority, and offshore islands/sea stacks; `biomes.rs` & `deposition.rs` oxbow/fan/delta hooks; keep `WORLD_EXTENT_M = 40_960.0` defaults untouched (`M2` fault patch green). | `~2–3 d` | Unit tests at `extent_m = 81_920.0` (`4096²`/`1024²`) + negative control asserting `extent_m = 40_960.0` default config stays bit-identical to trunk. |
| **2** | **`WV2-BIG-2`** Scale-Aware Hydrology, Waterfall `dx` Fix & Verifier Calibration | `anima-mac` | Fix `pipeline.rs:533` & `lakes_falls.rs:80` `dx_m >= 20 m` waterfall window (`run_limit = max(30, 3.65 * dx_m)`), add `81_920 m` / `WB-D3` + `C5b` (`WB-D8`) gate profile in `verify/` (keeping `40.96 km` profile unchanged), calibrate `WV2-PHYS4` physics at `81.92 km`. | `~2–3 d` | `1024²` -> `4096²` Stage-1/Stage-2 parity + negative control confirming an uncalibrated `40.96 km` candidate fails or trips `WB-D3`/`WB-D8` gates cleanly. |
| **3** | **`WV2-BIG-3`** Official `81.92 km` Generation, Bake & Artifact Commit | `anima-mac` (grid if `8192²`) | Generate winning `81.92 km` preset (`4096² @ 20 m` per `WB-D1`), run `anima-bake fields/lighting/pack --verify`, update `artifacts/world-v2/{preset_v2.toml,sim_256.anmw3,index.json,gates.json}` (replaces `C01` once!). Optional 56-B `ANMW v3` header bump (`WB-D6`). | `~1–1.5 d` | `wv2_4_g_shape_comp_tests`, `wv2_5_bake_tests`, `world_v2_sim_gate_tests`, 1-byte tamper negative control, and `WorldIdentity` mismatch test against old `C01` saves. |
| **4** | **`WV2-BIG-4`** Viewer & Capture Manifest Scale Sync | `anima-case` / `anima-mac` | Update `#291` `worldV2/` items (`FLORA_SECTORS_PER_SIDE` from `index.grid_size / 256`, HUD string `'World v2 C01'` -> dynamic ID, `HEIGHT_RATIO` per `WB-D5`), re-aim `world_v2_manifest.json` canonical poses, stage internal tiles for `test:e2e`. | `~1–1.5 d` | `tests/frontend/worldArtifactV3.test.ts`, `worldV2ManifestEvidence.test.ts`, Playwright `capture:map` (`map_manifest.json` untouched), stale-manifest negative control. |
| **5** | **`WV2-LOOK`** Realism Shader & Coast Pass (`QD-WV2-LOOK`) | `anima-mac` | Merge trunk (including `WV2-BIG-3/4` & `#291`), promote `mac/wv2-look-proto` (`CST-01..04` foam/wet sand, steep-face triplanar, aerial perspective, real-scale flora), fix `floraClearance` & map-edge side effects. | `~2 d` | `capture:map` before/after, `npm run test:e2e` with staged tiles, `<= 60` draw calls (`rw_budget`). |
| **6** | **`WV2-BIG-5`** Dell Iris Xe Gate & Single Tile Publication (`QD-REL09`) | `owner` on Dell + `anima-case` | Run `2026-11-26-wv2-6-iris-xe-runbook.md` on Dell (`Low`, `1,000` creatures, `<= 60` draw calls); on owner final yes, upload single `~145 MB` tile archive to GitHub Releases and cut `v0.9.0-rc.1`. | `~1 d` | Iris Xe `B0` vs `A1` pass table (`§6` of runbook) + `release-build.yml` green Windows & macOS artifacts. |

---

## 8. Open Owner Decisions (`WB-D1..WB-D8`)

| ID | Decision Question | Options | Recommendation | Fallback if Declined |
|---|---|---|---|---|
| **`WB-D1`** | Grid size & cell resolution for `~81.92 km` | **(A)** `8192² @ 10 m` (`~560 MB` pack, `~35 min` gen, requires tile streaming)<br>**(B)** `4096² @ 20 m` (`~145 MB` pack, `~9 min` gen, drop-in for `WV2-6`) | **Option B (`4096² @ 20 m`)** for `v0.9.0`, upgrading to `8192² @ 10 m` in Phase P2/P3 (`NR-SPIKE`). | If **A** chosen now, add `WV2-BIG-3b` LRU tile-streaming card and grid `JOB-W1..W5` sharding before `WV2-6` merges. |
| **`WB-D2`** | Handling non-tropical `G-REF` biomes (boreal Taiga, Arctic Tundra, autumn Temperate) | **(a)** Keep coherent `~11°N` super-massif (`E1–E9`); defer boreal/temperate to a second post-`0.9` preset<br>**(b)** Synthetic latitude gradient across `82 km`<br>**(c)** Ship two worlds/packs in `0.9` | **Option (a)** — preserves physical coherence (`LD-1` corrected) and single tile-pack budget. | If **(c)** chosen, add a second `58°N` preset card and double the release asset budget. |
| **`WB-D3`** | Recalibrated `81.92 km` verifier counts (`C1`, `C3`, `C4`, `G-SHAPE.5`) | Accept proposed `§5` numbers (`wf [40, 200]`, `caves [60, 140]`, `MIN_PATCH_AREA_M2 = 0.50 km²`, `large_lakes >= 12`, `fans >= 6`, `deltas >= 4`, `r_ref = max(65, 4*dx_m)`) | **Accept `§5` proposed ranges** under a separate `81.92 km` profile while keeping `40.96 km` verifier tests unchanged. | Keep `40.96 km` thresholds unchanged in `verify/` and log `81.92 km` scale metrics diagnostically. |
| **`WB-D4`** | Release order vs. `C01` | **(a)** Hold public tile release & `v0.9.0-rc.1` until `~82 km` world + `WV2-LOOK` pass gates (keep `#291` internal via `QD-TRIAL-V3`)<br>**(b)** Ship `C01` in `0.9.0` first, then break saves on upgrade | **Option (a)** — single tile upload, zero save invalidation, delivers the requested world in `0.9.0`. | If **(b)** chosen, publish `C01` tiles now, cut `0.9.0-rc.1` on `C01`, and explicitly disclose save break on `0.9.x` / `1.0`. |
| **`WB-D5`** | Viewer `HEIGHT_RATIO` at `81.92 km` (`RENDER_SIZE = 1200`) | **(a)** Keep `HEIGHT_RATIO = 0.14` (`2×` steeper relief, dramatic peaks)<br>**(b)** Halve to `0.07` (same physical slope ratio as `40.96 km`)<br>**(c)** Compromise `0.10` (`~1.8×` exaggeration, readable silhouettes) | **Option (c) `0.10`** (`to visually verify` on `WV2-BIG-4` `capture:map`), keeping `0.14` untouched for `C01`. | Keep `0.14` (if owner prefers tall stylized peaks) or `0.07` (strict true aspect ratio). |
| **`WB-D6`** | `ANMW v3` header `extent_m` encoding (`44 B` vs `56 B`) | **(a)** Keep `44`-byte `ANMW v3` header unchanged and read `extent_m` from `index.json` (`0` format break)<br>**(b)** Extend to `56`-byte header (`world_extent_m_bits`, `elev_min_m_bits`, `elev_max_m_bits` per scale research `§9`) | **Option (a)** if staying `4096²` / single `[-1500, +5700] m` range; **Option (b)** if adding a temperate `58°N` preset later. | Option (a) avoids bumping `sim_256.anmw3` codec version during `0.9`. |
| **`WB-D7`** | Desktop simulation `sim_256` grid (`320 m/cell` at `81.92 km`) | **(a)** Keep `256²` (`MapBounds 200×200`, `320 m/cell`, zero sim cost increase)<br>**(b)** Upgrade desktop sim to `512²` (`4×` field memory/cost) | **Option (a)** — required by `QD-WV2-BIG` (`"Unchanged: ... the 256^2 sim grid and MapBounds 200x200"`). | Revisit `512²` sim only after Stage 4 closes (`WORLD_SCALE_AND_MAP_GENERATION_RESEARCH_2026-11-22.md §1.4`). |
| **`WB-D8`** | Coastal profile shares & `C5b.CoastalPlainProfile` metric (`§4b`) | Target **`55–65%`** plain-backed, **`20–30%`** fan/delta, **`10–20%`** steep `CST-03`/Tayrona minority; `C5b`: `>= 0.60` @ `>= 5.0 km` `< 100 m`, median coast-to-`1,000 m` `>= 10.0 km`, `C5 cliff <= 0.25` | **Accept `§4b` targets & `C5b` metric** under the `81.92 km` profile (first measure `C01`/`PHYS2`/`PHYS3` baseline during `WV2-BIG-2`). | Keep `C5b` diagnostic-only in `WV2-BIG-2` while still enforcing the passive-margin plain setback in `WV2-BIG-1`. |
