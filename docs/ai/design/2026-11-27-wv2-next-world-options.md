---
phase: design
feature: map-v2
title: World v2 Next-World Shape and Plant Data Options (WV2-NEXT-WORLD-OPTIONS)
description: Design options note comparing render-only terrain detail, tectonic/stream-power generator changes, ecosystem-driven plant mixtures, and v0.9 vs v1.0 release timing for World v2
status: proposed
owner: anima-dell
last_reviewed: 2026-11-27
review_cycle: per-release
---

# World v2 Next-World Shape & Plant Data Options (`WV2-NEXT-WORLD-OPTIONS`)

> **Status: `proposed` (map and world design options note — documents only, no code).**
> Allowed under [`QD-FOCUS`](../../how-to/AGENT_WORKFLOW.md#4-khối-decisions-để-dán-vào-prompt-0) as Map v2 / world design; leaves all frozen post-`0.9.0` living-world simulation designs (`FOCUS-1`) untouched. Numbers below come strictly from committed files on trunk `origin/feat/ecological-selection` (`2014fdcd`).

---

## 1. Observed Problem Statement (World Data Beyond Shader Look)

Owner review of World v2 captures judged that while the prototype rendering pass (`WV2-LOOK`) resolves most lighting/shading look issues, two visual and structural deficits live in the **world data itself** (`artifacts/world-v2/` + baked `flora/` sectors) and cannot be fixed by lighting shaders alone:

### 1.1 Shape Deficit (`artifacts/world-v2/gates.json` at `8d58a580`)
- Visually, the main massif reads as a single smooth dome with a flat summit, slopes lacking incised ridges and ravines, and a nearly uniform eastern ramp.
- Committed metrics in [`artifacts/world-v2/gates.json`](../../../artifacts/world-v2/gates.json) (`verifier_profile = "81.92km"`, `seed = 1465266689`, `status = "accepted"`, `decision = "MAP-D8"`, **`9/15` PASS / `6/15` FAIL**, `overall_pass = false`) match that visual reading across all six failing gates:
  1. **`G-SHAPE.2.JunctionAngles`** (`passed = false`): `junction_count = 339`, `median_deg = 71.57`, `obtuse_fraction = 0.1032`, `rain_shadow_mean_deg = 69.83`, `windward_humid_mean_deg = 70.98`. Per [`2026-11-26-wv2-big-gshape-diagnosis.md`](../planning/2026-11-26-wv2-big-gshape-diagnosis.md), gentle plain ramps and broad piedmont basins deflect low-gradient tributaries past the obtuse threshold.
  2. **`G-SHAPE.3.DrainageDensity`** (`passed = false`): `global_km_per_km2 = 1.2194`, `dissection_ratio = 0.8615` (`windward_mountain = 1.1355` vs `plains_rain_shadow = 1.3181`), `caprock_tile_cv = 0.7141` (`73` tiles). Smooth mountain flanks delay headwater dissection at `dx_m = 20.0`, while caprock plateau tilt concentrates drainage into selected slots ([`2026-11-26-wv2-big-gshape-diagnosis.md` §2.2](../planning/2026-11-26-wv2-big-gshape-diagnosis.md#22-g-shape3drainagedensity-dissection_ratio--07948--15-caprock_tile_cv--07142--045)).
  3. **`G-SHAPE.4.HackLaw`** (`passed = false`): `sample_count = 2000`, `exponent_h = 0.5977`, `prefactor_c_km = 1.5179`, `r_squared = 0.7365`, `max_path_km = 49.02`, `median_mainstem_sinuosity = 1.2789` (broad scatter mixing compact massif runs and elongated corridor/basin trunks).
  4. **`G-SHAPE.5.ValleysFlint`** (`passed = false`): `b_up_median = 0.8129`, `vf_up_median = 1.2563`, `b_down_median = 1.6711`, `vf_down_median = 34.5328`, `vf_ratio_down_over_up = 27.4886`, `flint_concavity_theta = 0.6081`, `flint_r_squared = 0.9582` (`dx = 20 m` floor quantization keeps `vf_up` wide while `b_up` slips slightly under smooth upstream reaches).
  5. **`G-SHAPE.7.HypsometrySpikes`** (`passed = false`): `hi_global = 0.1583`, `catchment_hi_median = 0.3874` (`24` catchments), `non_cliff_needle_spikes = 0`, `hillslope_p999_deg = 74.74°`. A broad low coastal/ramp fraction pulls `hi_global` below `[0.22, 0.55]` even when cliff spikes are zero ([`2026-11-26-wv2-big-gshape-diagnosis.md` §2.5](../planning/2026-11-26-wv2-big-gshape-diagnosis.md#25-g-shape7hypsometryspikes-hi_global--01585--022)).
  6. **`G-COMP-v2.C4.RiversFansDeltas`** (`passed = false`): `counted_river_surface_fraction = 0.000980`, `active_channel_cells = 184,265`, `fan_count = 135`, `delta_count = 9`, `oxbow_count = 0`, `meander_scroll_cells = 28,130`, `monotonic_w2_w3_w4 = true`, **`hack_consistent = false`** (directly gated by `G-SHAPE.4.HackLaw` in [`g_comp.rs`](../../../src-tauri/crates/anima-worldgen/src/verify/g_comp.rs)).

### 1.2 Plant Placement Deficit (`bake/flora_place.rs` at `2014fdcd`)
- In [`src-tauri/crates/anima-worldgen/src/bake/flora_place.rs`](../../../src-tauri/crates/anima-worldgen/src/bake/flora_place.rs), `FloraSpeciesId` defines **eight archetypes total** (`0..=7`: `RainforestCanopy`, `MontaneBroadleaf`, `ElfinConifer`, `DryThornScrub`, `MangroveProp`, `SavannaAcacia`, `ParamoEspeletia`, `RiparianWillow`), packed as `8`-byte decorative-only records (`FLORA_FLAG_DECORATIVE = 0x01`, `ANMF` v1 sectors of `256 × 256` cells per [`ADR-0010` §4](../../decisions/ADR-0010-pinned-world-artifact.md)).
- Placement runs a single `match BiomeId::from_u8(biome_id)` mapping **one archetype per biome** (with only `BiomeId::Grassland` branching at `river_w_m >= 0.5 || twi > 8.0` between `RiparianWillow` and `SavannaAcacia`), followed by scalar probability scaling (`slope_atten * moist_boost`, clamped to `0.12`). Temperature gradients, soil moisture niches, micro-slope exposure, and continuous distance-to-water transitions do not blend species mixtures across or within biomes.

---

## 2. Design & Timing Options

### Option (A) — Render-Only Detail (Draw-Time Normal Ravines & Relief)

- **What changes:** Add procedural multi-scale **normal-only** ravines, gully creases, and rocky flank micro-relief in the World v2 terrain shader/material pass (no heightfield edit, no generator change, no tile-pack rebuild). Reference method: Grenier, Guérin et al. (2024), *"Real-time Terrain Enhancement with Controlled Procedural Patterns"* (`https://diglib.eg.org/bitstream/handle/10.1111/cgf14992/v43i1_05_cgf14992.pdf`), conditioned on existing baked `height`, `normal_xy`, and `hydro_flow` layers so detail aligns with drainage slope and aspect.
  - **Hard physics constraint:** Shading perturbation **must not displace rendered vertex height** (`Δy_render = 0`). Moving the drawn ground vertically without changing `artifacts/world-v2/sim_256.anmw3` or height tiles detaches agents, props, and navigation contacts from the ground surface (`terrain_height` vs rendered mesh mismatch).
- **Failing gates targeted:** **None (`0/6`).** Existing gates evaluate committed terrain data in `anima-worldgen` (`G-SHAPE.*`, `G-COMP-v2.*`); visual detail only improves perceived relief and silhouette shading in the desktop renderer.
- **Cost:** Frontend/shader work in the `WV2-LOOK` card (`anima-mac`, ~1–2 days including E2E captures and `WV2-6` Iris Xe budget check).
- **Release impact:** **Zero world-data impact.** Preserves `QD-GATES-1125 (4)` (single public tile pack), keeps published `v0.9.0-internal` (`world_v2_1d313005_tiles.tar.gz`, `190,071,986` B, SHA-256 `4fdad1df…`), preserves `sim_world_identity` (`1d313005…`), and preserves existing `WV2-7` saves.
- **Risk:** Real macro-topography stays a smooth flat-topped dome under silhouette angles, rivers/hillsides still lack true geometric incision, and extra procedural texture/ALU work in the terrain fragment shader must stay inside the `33.3 ms` Dell Iris Xe `Low` frame budget ([`2026-11-26-wv2-6-iris-xe-runbook.md`](../planning/2026-11-26-wv2-6-iris-xe-runbook.md)).

### Option (B) — Generator Change (Tectonic Uplift + Coupled Stream-Power Erosion)

- **What changes:** Replace the analytical dome/ramp uplift field and residual smooth flanks in `anima-worldgen` with tectonic ridge/fault uplift coupled with continuous stream-power fluvial erosion, crest-sharpening hillslopes, and/or multi-scale terrain amplification — without changing physics/sim contracts or designing code here:
  - Cordonnier et al. (2016), *Large Scale Terrain Generation from Tectonic Uplift and Fluvial Erosion* (`https://www.cs.purdue.edu/homes/bbenes/papers/Cordonier16CGF.pdf`);
  - Cordonnier et al. (2018), *Authoring Landscapes by Combining Ecosystem and Terrain Erosion Simulation* (`https://www.cs.purdue.edu/homes/bbenes/papers/Cordonnier18TVCG.pdf`);
  - Multi-scale terrain amplification (`DOI 10.1145/3658200`).
  - **Affected `anima-worldgen` modules (no code design):**
    - Uplift & stratigraphy: [`src-tauri/crates/anima-worldgen/src/tectonic.rs`](../../../src-tauri/crates/anima-worldgen/src/tectonic.rs) (dome/ramp/caprock envelope & uplift rates), [`config.rs`](../../../src-tauri/crates/anima-worldgen/src/config.rs), and preset [`presets/wv2_big_81920.toml`](../../../src-tauri/crates/anima-worldgen/presets/wv2_big_81920.toml).
    - Coupled erosion & routing: [`erosion_fluvial.rs`](../../../src-tauri/crates/anima-worldgen/src/erosion_fluvial.rs), [`hydrology.rs`](../../../src-tauri/crates/anima-worldgen/src/hydrology.rs), [`erosion_thermal.rs`](../../../src-tauri/crates/anima-worldgen/src/erosion_thermal.rs), [`deposition.rs`](../../../src-tauri/crates/anima-worldgen/src/deposition.rs), [`lakes_falls.rs`](../../../src-tauri/crates/anima-worldgen/src/lakes_falls.rs), and stage orchestration in [`pipeline.rs`](../../../src-tauri/crates/anima-worldgen/src/pipeline.rs).
    - Downstream baked/verified artifacts & pinned tests: [`artifacts/world-v2/`](../../../artifacts/world-v2/index.json) (`preset_v2.toml`, `sim_256.anmw3`, `index.json`, `gates.json`), [`wv2_4_g_shape_comp_tests.rs`](../../../src-tauri/crates/anima-worldgen/tests/wv2_4_g_shape_comp_tests.rs), [`world_v2_desktop_switch_tests.rs`](../../../src-tauri/tests/world_v2_desktop_switch_tests.rs), and [`scripts/test_target_policy.mjs`](../../../scripts/test_target_policy.mjs) (`MAP-D8` pin / allowlist).
- **Failing gates targeted:** Directly targets **all `6` failing gates** in `artifacts/world-v2/gates.json`:
  - `G-SHAPE.7.HypsometrySpikes` (dissected ridge-and-valley uplift raises interior hypsometry toward `[0.22, 0.55]` without flat summits),
  - `G-SHAPE.2.JunctionAngles`, `G-SHAPE.3.DrainageDensity`, `G-SHAPE.4.HackLaw`, and `G-SHAPE.5.ValleysFlint` (true dendritic catchments, steeper tributary slopes, consistent Hack scaling), and
  - `G-COMP-v2.C4.RiversFansDeltas` (via `G-SHAPE.4` `hack_consistent` passing).
- **Cost:** `anima-mac` + `anima-case` (`worldgen` calibration, cross-platform Windows/macOS determinism verification, re-bake, new `WV2-BIG-*` verification + doc updates): roughly **1–2 weeks** of iterative experiments and cross-platform runs.
- **Release impact:** **Full world reset.** Under [`QD-GATES-1125 (4)`](../../how-to/AGENT_WORKFLOW.md#4-khối-decisions-để-dán-vào-prompt-0) and [`QD-GATES-1127 (1)`](../../how-to/AGENT_WORKFLOW.md#4-khối-decisions-để-dán-vào-prompt-0), changing terrain data produces a **new world**, a **new `sim_world_identity`** (`!= 1d313005…`), a **new `world_v2_*_tiles.tar.gz` tile pack**, and a **new release tag/asset** superseding `v0.9.0-internal`. Prior **`WV2-7` saves are loudly refused** by `WorldIdentity::mismatch_against` ([`world_artifact.rs`](../../../src-tauri/src/core/world_artifact.rs)). Note also [`LR-GSHAPE-GATE`](../../how-to/AGENT_WORKFLOW.md#4-khối-decisions-để-dán-vào-prompt-0): every currently passing gate (`G-SHAPE.1`, `.6`, `.4_2`, `C1`–`C3`, `C5`, `C5b`, `G-SIM`) must hold without regression.
- **Risk:** High iteration and gate-regression risk (as seen when `#325` `G3` improved `G-SHAPE.7` but regressed `C3.Lakes`), cross-OS floating-point drift in `libm`/multigrid loops if solver order changes, memory ceiling (`<= 1.2 GiB` RSS during `4096²` generation), and invalidating the already-published `v0.9.0-internal` tile release (`190,071,986` B) if shipped before `v0.9.0`.

### Option (C) — Plant Data Change (Climate/Slope/Water Species Mixtures)

- **What changes:** Replace the 1-archetype-per-biome table in [`src-tauri/crates/anima-worldgen/src/bake/flora_place.rs`](../../../src-tauri/crates/anima-worldgen/src/bake/flora_place.rs) with continuous weighted mixtures driven by temperature, moisture (`TWI` / precipitation), slope, and distance to water (`shore_sdf` and `hydro_flow`), referencing EcoBrush ecosystem placement (Gain 2017, *EcoBrush: Interactive Control of Visually Consistent Large-Scale Ecosystems*, `https://people.cs.uct.ac.za/~Jgain/wp-content/papercite-data/pdf/gain2017.pdf`).
  - **Module touch surface:** `bake/flora_place.rs` and `bake/fields.rs` (`generate_decorative_flora_sectors` inputs from `shore_sdf` / climate temperature / slope / river distance). If `8` `FloraSpeciesId` variants remain sufficient, `ANMF` v1 (`FLORA_VERSION = 1`, `u8 species_id`) stays format-compatible; expanding beyond 8 species additionally touches [`bake/assets.rs`](../../../src-tauri/crates/anima-worldgen/src/bake/assets.rs), [`bake/mod.rs`](../../../src-tauri/crates/anima-worldgen/src/bake/mod.rs), and frontend mesh/impostor tables. Keeps `FLORA_FLAG_DECORATIVE` untouched (purely visual flora, no simulation law change).
- **Failing gates targeted:** **None (`0/6`)** of the geomorphology `G-SHAPE.*` / `C4` gates (plant instances are decorative bake outputs checked by `flora_footprint` / placement tests, not terrain verification gates).
- **Cost:**
  - Using weighted mixtures across the **existing 8 `FloraSpeciesId` archetypes**: `anima-mac` (+ `anima-case` parity check), **~2–4 days** + full tile re-bake.
  - Expanding `FloraSpeciesId` with new 3D botanical assets/LODs/impostors + license/compliance checks: **~1–2 weeks**.
- **Release impact:** **New tile pack required even if terrain heightfield stays fixed.** Modifying `flora_place.rs` alters baked `flora/s_*.anmf` blobs inside the tile archive, which changes [`artifacts/world-v2/index.json`](../../../artifacts/world-v2/index.json) and `release_archive_sha256`, requiring a **new `world_v2_*_tiles.tar.gz` release asset** (per `QD-GATES-1125 (4)`, the project publishes the tile pack once per release). If `sim_256.anmw3` is left untouched, saves stay technically compatible (`sim_world_identity` unchanged); if paired with Option (B), both terrain, `sim_world_identity`, saves (`WV2-7` refusal), and tile pack reset together once.
- **Risk:** Re-baking and publishing a second `~190 MB` tile archive for flora alone doubles release churn; adding vegetation density or distinct species draw calls must also fit inside `WV2-6` GPU/VRAM limits (`<= 4 GB` VRAM, `<= 60` draw calls) and Iris Xe frame budgets.

### Option (D) — Timing Strategy: `(A)` in `v0.9.0` + `(B)+(C)` Together in `v1.0.0` vs. Before `v0.9.0`

| Dimension | Strategy D1 *(Split: `(A)` in `0.9`, `(B)+(C)` in `v1.0`)* | Strategy D2 *(Early: `(B)+(C)` Before `v0.9.0`)* |
|---|---|---|
| **What ships in `v0.9.0`** | Keeps committed `BIG_4096_v2` (`8d58a580`, `9/15` `MAP-D8`) and published `v0.9.0-internal` pack (`world_v2_1d313005_tiles.tar.gz`, `190,071,986` B); adds **Option (A)** normal-only micro-detail in `WV2-LOOK`. | Delays `v0.9.0` while building & cross-platform verifying **(B)** tectonic/stream-power worldgen + **(C)** mixed flora; supersedes `v0.9.0-internal` (`1d313005…`) and replaces `artifacts/world-v2/` again. |
| **Impact on `FOCUS-2` release path** | Preserves current `FOCUS-2` cadence (`WV2-BIG-3` done -> `v0.9.0-internal` published -> `WV2-6-INSTR` / `WV2-6` / Iris Xe -> `WV2-LOOK` -> `v0.9.0-rc.1` -> `3` `MP-5.1` sessions -> `v0.9.0`). | Resets `FOCUS-2` steps 1–4 (`WV2-BIG-3`, tile publication, `CI-TILES`, `WV2-BIG-4`, and Iris Xe before/after capture pair must all re-run on the new world). |
| **Release-asset & save churn (`WV2-7`)** | **Zero churn in `0.9`**, then **one** combined world + `sim_world_identity` + tile-pack reset at **`v1.0`** (clean save migration break once for `v1.0`, matching `(B)` + `(C)` together). | Creates an immediate extra tile-pack release replacement before `0.9` RC, plus another break if `v1.0` adds living-world fields (`ADR-0016`) or extra biomes/flora. |
| **Schedule & gate risk** | Lowest `v0.9.0` schedule risk; decouples first external user testing (`MP-5.1`) from open-ended geomorphology research in `(B)`. | Highest `v0.9.0` schedule risk (`~2–4` weeks delay); `(B)` must pass `LR-GSHAPE-GATE` without breaking any of the `9` currently passing gates before `rc.1`. |
| **User impression in `v0.9.0`** | Better close-up rock/ravine normal relief (`A`) on top of `WV2-LOOK`, but macro dome/east-ramp silhouette and 1-species-per-biome flora wait until `v1.0`. | True eroded ridges/dendritic valleys and mixed flora appear in `0.9.0`, at the cost of postponing `v0.9.0-rc.1` and redoing the `v0.9.0-internal` tile release. |

---

## 3. Summary Comparison Matrix

| Option | Target Problem | Failing Gates Addressed (`8d58a580`) | World Data / `sim_world_identity` | New Tile Pack Required? | `WV2-7` Old Saves |
|---|---|---|---|---|---|
| **(A) Render-only detail** *(Grenier et al. 2024)* | Close-up ravines / surface micro-relief (normal-only, zero displacement) | `0/6` (shader-only) | Unchanged (`1d313005…`) | **No** | Compatible |
| **(B) Generator change** *(Cordonnier 2016/2018; `10.1145/3658200`)* | Macro dome shape, flat summit, east ramp, dendritic incision | **Up to `6/6`** (`G-SHAPE.2,.3,.4,.5,.7` + `C4`) | **Replaced** (`new world`) | **Yes** | **Refused** |
| **(C) Plant data** *(EcoBrush, Gain 2017)* | Single-species biomes in `flora_place.rs` (`8` archetypes) | `0/6` (decorative `ANMF` bake) | `ANMF` tiles changed (`sim_256` optional) | **Yes** | Unchanged unless paired with `(B)` |
| **(D1) `(A)` in `0.9` + `(B)+(C)` in `v1.0`** | Ships `0.9` cleanly now; bundles data resets once at `v1.0` | `0/6` in `0.9`; **`6/6`** at `v1.0` | Unchanged in `0.9`; replaced once at `v1.0` | **Once** at `v1.0` | One break at `v1.0` |
| **(D2) `(B)+(C)` before `0.9`** | Fixes macro shape + flora before `v0.9.0-rc.1` | **`6/6`** before `0.9` | Replaced immediately (supersedes `v0.9.0-internal`) | **Yes**, before `0.9` | Immediate break |

---

## 4. Lead Recommendation Slot

_Left empty for the lead to fill._

---

## 5. Owner Decision Slots (`TBD`)

- **Option (A) in `WV2-LOOK`:** `[ ] Include normal-only detail (Grenier et al. 2024, Δy = 0)` / `[ ] Defer` / `[ ] Decide later`
- **Option (B) + (C) Timing (`D1` vs. `D2`):** `[ ] D1: Keep v0.9.0-internal and bundle (B)+(C) into v1.0 as one world + tile reset` / `[ ] D2: Hold v0.9.0-rc.1 and ship (B)+(C) before v0.9.0`
- **Option (C) Archetype Budget:** `[ ] Weighted mixtures over existing 8 FloraSpeciesId archetypes first` / `[ ] Expand asset catalogue beyond 8 archetypes`
- **Notes / Constraints:** `____`
