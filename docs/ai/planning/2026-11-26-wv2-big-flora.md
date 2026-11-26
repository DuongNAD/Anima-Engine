---
phase: planning
feature: map-v2
title: Card WV2-BIG-FLORA — Area-Invariant Decorative Flora Placement Across Cell Sizes
status: active
owner: anima-mac
last_reviewed: 2026-11-26
description: Pre-registered fix and validation for area-invariant flora density per km^2 in bake/flora_place.rs at dx_m = 20 m while preserving bit-identical C01 flora sectors and tile-pack SHA-256.
---

# Card `WV2-BIG-FLORA` — Area-Invariant Decorative Flora Placement (`LR-AUTH-FLORA-AREA`)

## 1. Authorization & Strict Scope

Under **`LR-AUTH-FLORA-AREA`** (approved by the owner in `OWNER_APPROVALS_2026-11-26_morning.md` and dispatched in `START_2026-11-26_PM.md` / `PROMPTS_2026-11-26b_DAY_MAC_anima-mac.md`), this card edits strictly **3 files**:

1. `src-tauri/crates/anima-worldgen/src/bake/flora_place.rs`
2. `src-tauri/crates/anima-worldgen/tests/wv2_5_bake_tests.rs`
3. `docs/ai/planning/2026-11-26-wv2-big-flora.md`

`artifacts/**`, `docs/ai/README.md`, `Cargo.toml`, `scripts/test_target_policy.mjs`, and all other bake, pipeline, and verifier files are untouched.

---

## 2. Pre-Registered Method & Predictions (Recorded Before Post-Edit Measurement)

### 2.1 Root Cause on Trunk (`40551113`)

In [`flora_place.rs`](../../../src-tauri/crates/anima-worldgen/src/bake/flora_place.rs), `generate_decorative_flora_sectors` iterates over each `256 × 256` sector with fixed `step_by(2)` (`2 × 2` cell stride) and evaluates per sampled cell:

```rust
let eff_prob = (prob * 4.0).clamp(0.0, 0.35);
```

- At `dx_m = 10 m` (`C01`, `4096²` covering `40.96 km`), each sampled `2 × 2` stride block covers `(2 × 10 m)² = 400 m²`, giving `2,500` samples/km². Multiplying `prob` by `4.0` yields an expected placement rate of `10,000 * prob` plants/km² (when unclamped), matching one draw per `10 m × 10 m` cell at probability `prob`.
- At `dx_m = 20 m` (`BIG-COMBO`, `4096²` covering `81.92 km`), each sampled `2 × 2` stride block covers `(2 × 20 m)² = 1,600 m²`, giving `625` samples/km² (`4×` fewer samples per km²) while keeping the identical per-sample probability `(prob * 4.0).clamp(0.0, 0.35)`. Under equal `(biome, moisture, twi, river_w_m, slope)` conditions, density per km² therefore drops by **`4.0×`** (`-75%`).

### 2.2 Pre-Registered Area-Invariant Algorithm

Define constants relative to the `10 m` reference:

- `REFERENCE_DX_M = 10.0`
- `REFERENCE_SAMPLE_SPAN_M = 20.0` (`2 × 10 m` reference stride footprint)
- `REFERENCE_AREA_CELLS = 4.0` (`(20 m / 10 m)²`)
- `MAX_FLORA_DRAW_PROB = 0.35`

Input validation: reject `!dx_m.is_finite() || dx_m <= 0.0` with `BakeError`.

1. **Exact `C01` legacy path (`(dx_m - 10.0).abs() <= 1e-6`):**
   Keep `stride = 2` (`(0..256).step_by(2)`), `draws = 1`, and the exact trunk expression `(prob * 4.0).clamp(0.0, 0.35)` with identical float operations and `ChaCha12Rng` draw order. This guarantees `0`-ULP bit-identical `flora/s_<tx>_<ty>.anmf` files and identical `C01` tile-pack `release_archive_sha256` (`4fa1c1544147347b161532509cc286ac93bf85b4e92f4ea9630c2c648548e20e`) and `bundle_identity` (`2f002d1bf976131e3034f9d574a3b7089c291a0c6c0803236ee6817c0ab8cafe`).
2. **General area-invariant path (`dx_m != 10.0`):**
   - Choose power-of-two `stride` dividing `FLORA_SECTOR_SIZE` (`256`): start at `stride = 1` and double `stride *= 2` while `stride < FLORA_SECTOR_SIZE && (stride * 2) as f32 * dx_m <= REFERENCE_SAMPLE_SPAN_M + 1e-6`.
     - `dx_m = 5 m` → `stride = 4` (`20 m` sample span, `1` draw).
     - `dx_m = 15 m` → `stride = 1` (`15 m` sample span, `1` draw, `per_draw_area_scale = 2.25 / 4.0 = 0.5625`).
     - `dx_m = 20 m` → `stride = 1` (`20 m` sample span, `1` draw, `per_draw_area_scale = 4.0 / 4.0 = 1.0`, avoiding the `0.35` clamp saturation that `stride = 2` + `16 * prob` would hit in wet `Jungle`/`Forest`/`Mangrove`).
     - `dx_m = 40 m` → `stride = 1` (`40 m` sample span, `draws = ceil(16.0 / 4.0) = 4`, `per_draw_area_scale = 1.0`).
   - Compute `span_ratio = (stride as f32 * dx_m) / REFERENCE_DX_M` and `area_ratio_10m = span_ratio * span_ratio`.
   - Compute `draws = (libm::ceilf(area_ratio_10m / REFERENCE_AREA_CELLS) as usize).max(1)` and `per_draw_area_scale = area_ratio_10m / (REFERENCE_AREA_CELLS * draws as f32)`.
   - Per sampled cell: compute `ref_eff_prob = (prob * REFERENCE_AREA_CELLS).clamp(0.0, MAX_FLORA_DRAW_PROB)` and `eff_prob = (ref_eff_prob * per_draw_area_scale).clamp(0.0, MAX_FLORA_DRAW_PROB)`, then execute `draws` independent Bernoulli trials with the exact same RNG sequence `(trial, jx, jz, scale_u8, yaw_u8)` when a trial succeeds.

### 2.3 Pre-Registered Quantitative Predictions

1. **Synthetic uniform-biome `dx_m` invariance:** Across `dx_m ∈ {5.0, 10.0, 15.0, 20.0, 40.0}` on uniform flat `Jungle`, `Forest`, `Swamp`, `Savanna`, `Grassland`, `Taiga`, and `Mangrove`, every `dx_m` will match `dx_m = 10.0` per-km² density within **±15%** (expected within ~`±3%` for large grids, since `2,500 * ref_eff_prob` is invariant by construction).
2. **`C01` (`4096² @ 10 m`) bit-identity:** Zero byte differences across all `256` `flora/s_*.anmf` files and exact match on `release_archive_sha256` (`4fa1c154...`) and `bundle_identity` (`2f002d1b...`).
3. **`BIG-COMBO` (`4096² @ 20 m`) vs trunk `BIG-COMBO`:** Sampling shifts from `stride = 2` (`625` samples/km²) to `stride = 1` (`2,500` samples/km²) at identical `eff_prob = (prob * 4.0).clamp(0.0, 0.35)`. Therefore:
   - Raw `BIG-COMBO` total instances and per-biome densities will increase by **`~4.0×`** (`136,599` → **`~546,000`** instances).
   - At **equal `(biome, moisture, slope, twi, river_w_m)` conditions**, `BIG-COMBO` (`20 m`) plants/km² will match `C01` (`10 m`) within **`±15%`** (expected **`~0% ± 2%`** sampling variance) for every biome (`Jungle`, `Forest`, `Swamp`, `Savanna`, `Grassland`). Unstratified raw biome means will differ solely where `BIG-COMBO` and `C01` differ in slope/moisture/TWI distributions inside a biome.

---

## 3. Measured Results (`4096²` Before & After)

All `4096²` measurements ran on `anima-mac` (`macOS arm64`, Apple M5, `RAYON_NUM_THREADS=4`):
- **`C01` (`40.96 km`, `dx_m = 10 m`, `seed = 0x57563201`):** generated from provisional `f195a361` (`artifacts/world-v2/preset_v2.toml`) -> `world_sha256 = e0db7dc25624bf66364a310659ca1035276c77f893342ad9fd85d817cdb2f7e7`, `sim_world_identity = cbdd3ba9f57725ada233d1b43f2f5804fe8cb0fe0b098ca5ef8ae0aae5784c4d`.
- **`BIG-COMBO` (`81.92 km`, `dx_m = 20 m`, `seed = 1465266689`):** generated from trunk (`40551113`, `presets/wv2_big_81920.toml`) -> `world_sha256 = 08549db56c551c54ba428359f14d8fce666bca68fed8ee4b8c639f9e464a16b0`, `sim_world_identity = 132fa5fa2ebfe9529412b17059e8e02d1d506632cf23cc1e7af80b3e930338ec`.

### 3.1 `C01` Bit-Identity Proof (`4096² @ 10 m`)

| Metric | `C01` Before (trunk `40551113`) | `C01` After (`mac/wv2-big-flora`) | Match? |
|---|---|---|---|
| `flora/s_*.anmf` + `flora_summary.json` (`diff -r`) | `256` sectors (`332,808` B) | `256` sectors (`332,808` B) | **Exact** (`diff -r` exit `0`, `0` diffs) |
| `total_instances` | `41,089` | `41,089` | **Exact** |
| `release_archive_sha256` (`v0.9.0-internal`) | `4fa1c1544147347b161532509cc286ac93bf85b4e92f4ea9630c2c648548e20e` | `4fa1c1544147347b161532509cc286ac93bf85b4e92f4ea9630c2c648548e20e` | **Exact** |
| `bundle_identity` | `2f002d1bf976131e3034f9d574a3b7089c291a0c6c0803236ee6817c0ab8cafe` | `2f002d1bf976131e3034f9d574a3b7089c291a0c6c0803236ee6817c0ab8cafe` | **Exact** |
| `archive_byte_len` | `144,730,577` B | `144,730,577` B | **Exact** |

### 3.2 Per-Biome Density Table (`C01` `10 m` vs `BIG-COMBO` `20 m` Before & After)

Raw per-biome counts and densities over valid flora-eligible land (`total_h_m > 0`, `water_kind == None`, `glacier_h_m <= 0.1`), plus total non-water land (`total_h_m > 0`, `water_kind == None`; `915.96 km²` on `C01`, `3,588.49 km²` on `BIG-COMBO`):

| Biome | `C01` (`10 m`) Area (`km²`) | `C01` Plants (`/km²`) | `BIG-COMBO` Area (`km²`) | `BIG` Before (`count`, `/km²`) | `BIG` After (`count`, `/km²`) | Mean Slope (`C01` vs `BIG`) |
|---|---:|---:|---:|---:|---:|---|
| **Jungle** | `7.93` | `114.24` (`906`) | `840.06` | `76,249` (`90.77`) | `307,749` (**`366.34`**) | `0.487` vs `0.097` |
| **Forest** | `61.76` | `182.56` (`11,274`) | `342.09` | `22,509` (`65.80`) | `89,895` (**`262.78`**) | `0.349` vs `0.155` |
| **Swamp** | `67.50` | `183.62` (`12,394`) | `48.57` | `3,060` (`63.01`) | `12,585` (**`259.14`**) | `0.214` vs `0.029` |
| **Savanna** | `70.68` | `72.05` (`5,093`) | `373.08` | `6,872` (`18.42`) | `27,381` (**`73.39`**) | `0.143` vs `0.129` |
| **Grassland** | `53.60` | `47.07` (`2,523`) | `668.47` | `14,777` (`22.11`) | `59,837` (**`89.51`**) | `0.323` vs `0.068` |
| Mangrove | `14.22` | `297.68` (`4,233`) | `90.48` | `7,308` (`80.77`) | `29,057` (`321.14`) | `0.083` vs `0.042` |
| Taiga | `0.18` | `56.88` (`10`) | `130.07` | `2,835` (`21.80`) | `11,463` (`88.13`) | `0.467` vs `0.447` |
| Alpine | `8.07` | `14.00` (`113`) | `12.06` | `65` (`5.39`) | `202` (`16.75`) | `0.600` vs `0.598` |
| Shrubland | `25.23` | `61.64` (`1,555`) | `181.20` | `1,624` (`8.96`) | `6,268` (`34.59`) | `0.166` vs `0.335` |
| Badlands | `104.18` | `13.32` (`1,388`) | `273.59` | `572` (`2.09`) | `2,363` (`8.64`) | `0.469` vs `0.438` |
| Desert | `80.52` | `8.94` (`720`) | `57.93` | `212` (`3.66`) | `879` (`15.17`) | `0.504` vs `0.217` |
| Bog | `22.21` | `15.98` (`355`) | `84.68` | `500` (`5.90`) | `1,948` (`23.00`) | `0.509` vs `0.406` |
| Tundra | `12.82` | `40.96` (`525`) | `3.09` | `16` (`5.19`) | `113` (`36.62`) | `0.247` vs `0.316` |
| **Valid land** | `791.92` | `51.89` (`41,089`) | `3,494.37` | `136,599` (`39.09`) | `549,740` (**`157.32`**) | — |
| **All non-water land** | `915.96` | `44.86` (`41,089`) | `3,588.49` | `136,599` (`38.07`) | `549,740` (**`153.20`**) | — |

### 3.3 Equal-Condition (`slope`, `moisture`, `TWI` / `river_w_m`) Density Check

Because `C01` is a steep `40.96 km` single massif (e.g. `Jungle` mean slope `0.487`) whereas `BIG-COMBO` has wide coastal plains and interior valleys (`Jungle` mean slope `0.097`), unstratified raw biome averages differ strongly. Controlling for `(slope, moisture)` across shared stratified bins (`slope ∈ [0, 0.1, 0.25, 0.45, 0.65]`, `moisture ∈ [0, 0.3, 0.5, 0.7, 0.85, 1.01]`, plus `river_w_m >= 0.5 || twi > 8.0` for `Grassland`, keeping shared bins with `>= 50` cells in both worlds) and comparing observed vs analytical `E[2500 * eff_prob]` on each world's exact cells confirms all five target biomes match within `±15%` (`-0.70%` to `+8.40%`):

| Biome | `C01` Obs / Exp (`/km²`) | `BIG` After Obs / Exp (`/km²`) | Matched-Bin `C01` (`/km²`) | Matched-Bin `BIG` After (`/km²`) | Matched Delta | Within `±15%`? |
|---|---:|---:|---:|---:|---:|---|
| **Jungle** | `114.24 / 122.35` (`0.934`) | `366.34 / 365.02` (`1.004`) | `131.08` | `134.01` | **`+2.23%`** | **PASS** |
| **Forest** | `182.56 / 184.41` (`0.990`) | `262.78 / 262.77` (`1.000`) | `206.49` | `205.04` | **`-0.70%`** | **PASS** |
| **Swamp** | `183.62 / 186.14` (`0.986`) | `259.14 / 258.43` (`1.003`) | `239.81` | `259.95` | **`+8.40%`** | **PASS** |
| **Savanna** | `72.05 / 72.03` (`1.000`) | `73.39 / 73.36` (`1.000`) | `72.55` | `72.60` | **`+0.07%`** | **PASS** |
| **Grassland** | `47.07 / 47.92` (`0.982`) | `89.51 / 89.82` (`0.997`) | `75.61` | `79.35` | **`+4.95%`** | **PASS** |
| Mangrove | `297.68 / 305.94` (`0.973`) | `321.14 / 323.13` (`0.994`) | `302.47` | `317.80` | `+5.07%` | **PASS** |

*(In addition, [`wv2_5_bake_tests.rs`](../../../src-tauri/crates/anima-worldgen/tests/wv2_5_bake_tests.rs) `flora_density_per_km2_is_invariant_to_cell_size_across_biomes` verifies `<= 15%` relative deviation on exact uniform flat fields across `dx_m ∈ {5, 10, 15, 20, 40}` for `Jungle`, `Forest`, `Swamp`, `Savanna`, `Grassland`, `Taiga`, and `Mangrove`.)*

### 3.4 Bake Wall Time, Peak RSS, and Instance Counts (`BIG-COMBO` `4096² @ 20 m`)

| Stage / Metric | Before (trunk `40551113`) | After (`mac/wv2-big-flora`) | Delta |
|---|---:|---:|---|
| `anima-bake fields` wall / peak RSS | `2.01 s` / `827,228,160` B | `1.46 s` / `831,094,784` B | `-0.55 s` (`+3.9 MB` RSS) |
| `anima-bake lighting --backend cpu` | `2.81 s` / `1,131,003,904` B | `2.38 s` / `1,131,167,744` B | `-0.43 s` |
| `anima-bake pack --verify` | `20.68 s` / `1,979,072,512` B | `18.37 s` / `1,979,514,880` B | `-2.31 s` |
| **Total flora instances** | `136,599` | **`549,740`** | **`4.024×`** (`+413,141`) |
| `flora_tiles` archive payload | `1,096,888` B | `4,402,016` B | `+3,305,128` B |
| Archive `.tar.gz` size | `187,531,078` B (`25475939e215...`) | `190,455,110` B (`1005d66c5287...`) | `+2,924,032` B (`+1.56%`) |
| `by_species` breakdown | `rainforest_canopy: 76,249`, `montane_broadleaf: 22,509`, `riparian_willow: 13,905`, `savanna_acacia: 10,804`, `mangrove_prop: 7,308`, `elfin_conifer: 2,835`, `dry_thorn_scrub: 2,408`, `paramo_espeletia: 581` | `rainforest_canopy: 307,749`, `montane_broadleaf: 89,895`, `riparian_willow: 57,411`, `savanna_acacia: 42,392`, `mangrove_prop: 29,057`, `elfin_conifer: 11,463`, `dry_thorn_scrub: 9,510`, `paramo_espeletia: 2,263` | `~4×` across every species |

Note: Flora species enum mapping (`FloraSpeciesId` vs renderer `LOOK_SPECIES`) is explicitly out of scope per `WV2-BIG-FLORA` and handled in `#291` / `WV2-LOOK`.
