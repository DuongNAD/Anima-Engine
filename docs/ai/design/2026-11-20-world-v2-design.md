---
phase: design
feature: map-v2
title: Design — World v2, the offline deterministic world generator (WV2-D, accepted)
description: Accepted design for Map v2 phase P1. A Rust workspace crate anima-worldgen generates a 40.96 km tropical coastal massif (reference the Sierra Nevada de Santa Marta) with tectonic uplift, coupled Fastscape-SD erosion, priority-flood routing, regional climate and an explicit ELA, writes a committed 256² simulation world plus an index of baked viewer tiles, and is split into eight cards WV2-1a to WV2-7 with gates G-SHAPE, G-COMP v2 and G-SIM.
status: accepted
owner: simulation-architecture
last_reviewed: 2026-11-23
---

> **Status (lead, 2026-11-20).** Accepted after four independent review rounds (r1–r3 FAIL, r4 PASS WITH
> NOTES); the verdicts are kept in the coordination hub (`verdicts/WV2-D*.md`). Author: the `anima-mac`
> agent, card WV2-D (read-only). The owner accepted the lead decisions LD-1 to LD-5 in chat on 2026-11-20
> ("oke đồng ý cho bạn tự làm"); they are recorded under QD-MAPV2 in
> [`AGENT_WORKFLOW.md`](../../how-to/AGENT_WORKFLOW.md). Notes carried into the cards: WV2-1b states the
> error norm of its transient test; WV2-2 gives a page or table locator for the lapse rate and P/PET ranges
> credited to Hoyos et al. 2019 [50] and [51], or relabels them as design values; WV2-6 adds a
> `bundle.resources` entry for the staged tiles. Line anchors below were read at trunk `1ef4f0d`/`cc40f11f`;
> re-read the symbols before editing (one known stale anchor: `build.rs:33` is now `build.rs:38`).

## Amendments after acceptance

### MAP-D2 (2026-11-23): WV2-2-PHYS terrain-physics rework

Owner decision in chat: "Duyệt, làm Step B". It applies to the plan in the card note
`docs/ai/planning/2026-11-23-wv2-2.md` (on branch `mac/wv2-2`, PR #199) at `593482cb`, reviewed by the lead in
hub verdicts 24 and 25. The Card 3 gates and the waterfall detector are unchanged (MAP-D1), and so is
every section below that this list does not name.

**Why the amendments were needed.** A 1-D column test (WV2-2-PHYS Step 0) found two things. Lithology
frozen at absolute elevation never forms a caprock step. A structural datum advected with uplift,
`z_c = z_c0 + U·t`, with lithology re-evaluated each step, does form one at n = 1 (Perne et al. 2017).
The lead re-ran the test script and it reproduced exactly.

| ID | Section | Change | Basis |
|---|---|---|---|
| A1 | §2.2 S1 item 3 | Caprock is no longer a band of absolute elevation. A non-repeating marginal column (Río Negro sandstone / Cogollo limestone / La Luna shale / upper caprock sandstone) dips 4° away from the massif spine, with the upper caprock preserved on the plateau. Lithology is evaluated against the advected datum and never bilinearly upsampled. | Formation identities are cited in the card note. Thicknesses 150/350/150/100 m, the dip, z_base = 650 m and A_fold = 45 m are model rationale, not sourced. Measured at t = 0: caprock fraction within [0.09, 0.21] on 17/17 seeds (mean 0.139). |
| A2 | §2.2 S2 and S8 | New coarse-level (512² and 1024²) S2b paleo-glacial pass: ELA 4,100 m, b0 = 0.005 /yr, K_g = 1e-4 with alpha_g = 1, 20 kyr, moraine sills from mass-conserving deposition. | Mark et al. 2005; MacGregor et al. 2000; Herman & Braun 2008 |
| A3 | §2.2 S1 item 2 | The rift trough is segmented along strike (L_seg = 15 km, relay ramps). | Jackson & White 1989 |
| A5 | §2.2 S3 | The binary switch to 34° scree when `sediment_s > 0.05 m` is replaced by a continuous blend α_eff = α_scree + (α_rock − α_scree)·exp(−H/H*), with H* = 1 m. | Bedrock-exposure term of Campforts et al. 2020 and Shobe et al. 2017, applied here as an **adaptation** to talus repose (not a published talus law) |
| — | §2.2 S2 multigrid schedule | 512² 80 × 1,000 yr, 1024² 40 × 1,000 yr, 2048² 25 × 200 yr, 4096² 40 × 100 yr, set so a caprock lip re-forms after upsampling (Step 0, experiment B). | Step 0 |
| — | §2.3 runtime | The 4096² generation is estimated at about 11–14 min on a Mac M5, against this document's analytical 10–18 s. The baseline already measured 351 s. | Recorded deviation; RSS stays ≤ 1.2 GB |

**Not amended.**
- G stays 0.85. A proposal to lower it to 0.25 was withdrawn: its cited source (Guérit et al. 2019) reports G > 0.4 for about two-thirds of the natural landscapes it measured.
- The endorheic definitions (§2.2.5 runoff, §4.1 humid threshold) are parked with the lead.

**Expected outcome, stated before implementation.** With G = 0.85, the card predicts 5–19 waterfalls per
seed (median about 10–14), below the Card 3 range of 20–80 on most seeds. If that holds, it is reported
as a gate failure, not fixed by tuning.

# World v2 Design (`A21 / WV2-D — Revision 4`)

- **Card:** `A21` (`WV2-D` — Map v2 Phase P1 design, strictly **READ-ONLY**: zero edits to `Anima-Engine` or worktrees, zero `cargo`/`npm` builds)
- **Author:** `anima-mac` (MacBook Air M5, macOS `arm64`, 10 cores, 32 GB)
- **Authoritative Sources & Commit Anchors:**
  - Inspected trunk (`origin/feat/ecological-selection`): `1ef4f0d303602d666f915aa20a6edfdcf20590db`
  - Merged Map v2 Plan & G-REF Reference Board (`PR #185`, `origin/feat/ecological-selection` `cc40f11ff071911c58a121c04c5759695ae824a2` / branch head `5949b5d3`):
    - [`docs/ai/requirements/2026-11-20-feature-map-v2.md`](../../ai/requirements/2026-11-20-feature-map-v2.md) (`§1–§8`, `§3b` frame budgets `:74-135`, `§3c` OxideSwarm bake `:136-170`)
    - [`docs/ai/design/2026-11-20-map-v2-reference-board.md`](../../ai/design/2026-11-20-map-v2-reference-board.md) + `artifacts/map-v2/reference-board.json` (`53` CC0/PD photos across `13` categories; `34` `P1` entries, `8` Category 12 `G-SHAPE` entries)
  - Accepted ADRs & Project Decisions: [`ADR-0010-pinned-world-artifact.md`](../../decisions/ADR-0010-pinned-world-artifact.md), [`ADR-0008-world-past-and-time.md`](../../decisions/ADR-0008-world-past-and-time.md), [`2026-11-19-feature-real-world-and-bodies.md`](../../ai/design/2026-11-19-feature-real-world-and-bodies.md), `QD-OXS` (`AGENT_RULES.md:26-28,98-179`), `QD-REALMS`, `QD-ASSETS`

> **Disclosure on measured facts vs. estimates (`§8`):** Every trunk code path, line number, struct layout, Cargo/lockfile version, and Map v2 requirement cited in this document was inspected directly at trunk `1ef4f0d3` and PR `#185` (`cc40f11f` / `5949b5d3`). **Explicit trunk caveat:** `live.founder_layout` (`1 | 2`) and `src-tauri/tests/founder_layout_tests.rs` did **not** exist at inspected trunk `1ef4f0d3`/`cc40f11f` (they subsequently merged at `4794b6fca720dc0fad4b3e66aa377a15e341bae6` via PR `#186`); `§5.2` (`GS-2`) records that exact dependency. All World v2 runtime durations and compressed tile-pack byte sizes at `2048²`–`8192²` are **analytical estimates** until `WV2-1b` measures Priority-Flood + Fastscape-SD step timings at `4096²` on Mac M5.

---

## Revision 4 Changelog (`verdicts/WV2-D_r3.md` — 3 Required Fixes + 7 Verdict Notes)

| Review Item | Resolution in Revision 4 | Section |
|---|---|---|
| **r4 Fix 1** ($\tilde{A}$ weighting in arid zones; `R` unique unit; arid test & neg control) | Changed Fastscape-SD $\tilde{A}$ weight from runoff `max(0, P-E)` to precipitation $\nu(\mathbf{x}) = P(\mathbf{x})/\nu_0$ with $P \ge P_{\min} = 0.15\text{ m/yr} > 0$ (`Yuan et al. 2019` `[44]`, `Braun 2022` `[49]`), keeping $\beta_i$ strictly finite at arid heads so wadis/gorges incise; restricted `max(0, P - E_pot)` strictly to $Q_{\text{ma}}$ hydraulic geometry; defined runoff rate $R(\mathbf{x}) = \max(0, P - E_{\text{pot}})$ uniquely in `m/yr` (cell volume $q_{\text{cell}} = R\,\Delta x^2$); added arid-cell test + unclamped-runoff negative control to `WV2-1b`. | `§2.2.2`, `§2.2.5`, `§8` (`WV2-1b`), `§9` |
| **r4 Fix 2** (`§5.1 C4` river-surface fraction band & counted cell rule) | Recomputed `C4` river-surface fraction to **`f_river ∈ [0.0005, 0.0030]`** matching `~6–18 m` trunks / `~1.1 m` Horton length-weighted mean width across $D_d \in [0.8, 3.5]\text{ km/km}^2$ (`~0.0006–0.0026`, mid `~0.0012` of `1,677.7 km²`); defined counted river surface explicitly as $f_{\text{river}} = \frac{1}{N^2}\sum_i \text{river\_amt}_i$ with $\text{river\_amt}_i = \operatorname{clamp}(w_i/\Delta x, 0, 1)$ on active non-ocean/non-lake channels ($A S^2 \ge \tau_{\text{chan}}$, $w_i \ge 0.5\text{ m}$). | `§5.1` (`C4`) |
| **r4 Fix 3** (`§2.2.7.1` snowline vs. ELA; `WV2-2` test alignment) | Adopted SNSM-calibrated environmental lapse rate $\Gamma = 6.3\text{ °C/km}$ (`Hoyos et al. 2019` `[50]`, $0\text{ °C}$ annual-mean air isotherm $\approx 4,365\text{ m}$) and **explicitly decoupled snow/glacier onset from $0\text{ °C}$**, driving `Snow`/`Glacier` from an explicit Equilibrium Line Altitude parameter **`ela_base_m = 4,900 m`** (`∈ [4,800, 5,100] m` `[47, 48]`, $\pm 180\text{ m}$ aspect / $\pm 120\text{ m}$ precip modulation); updated `WV2-2`'s snowline check to assert that exact `ela_base_m` / `ela_local_m` band. | `§2.1`, `§2.2.7`, `§8` (`WV2-2`), `§9` |
| **r4 Notes `1–7`** | **Note 1:** Qualified `1,000–3,300 m`, `2,600–3,300 m`, `3,300–4,800 m` belt boundaries as design parameterization informed by `[45]`'s Massenerhebung compression (`>1,700 m`). **Note 2:** Stated windward $P \approx 2.5\text{–}3.8\text{ m/yr}$ and lowland $\text{PET} \approx 1.2\text{–}1.5\text{ m/yr}$ behind $R \in [1.2, 2.4]\text{ m/yr}$ (`[50, 51]`). **Note 3:** Declared `Desert` (xerophytic thorn scrub / cardonal) & `Badlands` (eroded semi-arid slopes) as proxies, labeled `DES-01/02` dunes a stylised visual benchmark, and relabeled `RIV-03` as dry-plateau basalt gorge. **Note 4:** Recorded `simulation_loop.rs:1735` `elevation < 0.4` (`= 1,380 m` in v3) to audit/adjust in `WV2-7`. **Note 5:** Restricted static `tauri.conf.json` `bundle.resources` (`WV2-7`) strictly to committed `sim_256.anmw3` + `index.json`, staging fetched tiles in `WV2-6` via `beforeBundleCommand` (`tauri.conf.json` added to `WV2-6`, ownership `WV2-7 -> WV2-6`). **Note 6:** Bounded `WV2-1b` small-$\Delta t$ reference test to a short transient (`10 kyr`, elevated $K_{\text{test}}$, `< 1 s` debug). **Note 7:** Replaced $\zeta$ with Braun (2022) `[49]`'s dimensionless $G = d^* v_s / \nu_0$. | `§2.2.2`, `§2.2.5`, `§2.2.8`, `§3.3.2`, `§3.4`, `§4.3`, `§5.1`, `§7.1`, `§8`, `§9` |

## Revision 3 Changelog (`verdicts/WV2-D_r2.md` & Amended Lead Decisions `LD-1`, `LD-5`)

| Review Item | Resolution in Revision 3 | Section |
|---|---|---|
| **r3 Fix 1** (Stewart DOI 404, Cuffey DOI 404, mechanical check) | Fixed Stewart (1998) `[37]` DOI to `10.1109/2945.675656`; dropped Cuffey & Paterson `[9]`'s 404 DOI keeping ISBN `978-0-12-369461-4`; added 4 Handle/Crossref-verified SNSM/tropical-Andes references (`[45]–[48]`); mechanically verified all `47/47` DOIs across the `45` DOI-bearing entries via `doi.org/api/handles/` (`responseCode=1`). | `§9` |
| **r3 Fix 2** (MFD `p = 1.6` & Gray 1961 `1.40` attributions) | Clarified that MFD exponent `p = 1.6` is this design's own tuning choice (between Quinn et al. 1991 `p = 1` and Holmgren 1994 `p ∈ [4, 6]`); for Gray (1961) `[12]`, kept exponent `0.568` and dropped the unconfirmed `1.40` prefactor. | `§2.2.5`, `§4.1` |
| **r3 Fix 3 & `LD-1 (corrected)`** (Fixed latitude, no `10°` span) | Replaced the `~10° latitude span` error with **one fixed regional latitude** ($\phi_0 = 11^\circ\text{N}$, total map span `~0.37°`, zero latitudinal climate gradient); climate variety arises strictly from elevation lapse/snowline, prevailing wind, orographic rain/rain shadow, coastal distance, and `TWI`. | `§2.1`, `§2.2.7` |
| **r3 Fix 4 & Note 8** (Runoff-derived `Q_ma`, trunk widths, basin size) | Derived discharge strictly from `R = max(0, P - E)` over realistic `120–300 km²` basins fitting inside `40.96 km` (sinuosity `1.25–1.45` matching `~22–35 km` mainstem), giving mean annual trunk $Q_{\text{ma}} \approx 4.6\text{–}22.8\text{ m}^3/\text{s}$ and `~6–18 m` trunk widths (`~20–45 m` deltas), restated consistently in `§3.4`, `RNF-01`, and `C4`. | `§2.2.5`, `§3.4`, `§4.3`, `§5.1` (`C4`) |
| **r3 Fix 5 & `LD-5`** (Tropical coastal massif setting & `22`-biome partition) | Adopted the **Sierra Nevada de Santa Marta** tropical coastal massif setting (`~11°N`, `h_phys ∈ [-1,500 m, +5,700 m]` reaching tropical snowline/ELA `~4,800–5,100 m` within `40.96 km`); replaced the legacy `18`-biome floor with `C1`'s setting-derived list covering `16` zoned/proxy biomes + `4` water/strip biomes + `2` explicitly absent biomes (`Chaparral`, `Steppe`), and relabeled `RNF-01`/`TMP-01`/`TAI-*`/`TUN-*` benchmarks cleanly. | `§2.1`, `§2.2.7`, `§4.3`, `§5.1` |
| **r3 Fix 6 & Note 6** (Fastscape-SD $G>0$ units & node-$i$ local term) | Reformulated PDE with precipitation-weighted drainage area $\tilde{A}$ (`[L²]`) and dimensionless $G$; fixed the Gauss–Seidel iterate to `G · (Φ_in + local_term_i) / Ã_i` with **no extra `Δt`**, folded node $i$'s implicit local term `(h_i^t + Δt U_i - h_i^{(r+1)})Δx²` into LHS `1 + α_i + G Δx² / Ã_i`, and sized the small-$\Delta t$ test on a compact grid/1-D profile. | `§2.2.2`, `§8` (`WV2-1b`) |
| **r3 Fix 7** (CI clippy, `[[test]] required-features`, `check_test_targets.mjs`, `ci.yml:174`) | Added `cargo clippy -p anima-worldgen --all-targets --features reader,generate,bake -- -D warnings` to CI & `WV2-1a`; added `[[test]] required-features` + `test_target_policy.mjs` (`worldgen-cpu` profile) + `feature_gated_targets_tests.rs` + `check_test_targets.mjs` on the CI `-p anima-worldgen` step; fixed `ci.yml:157 -> :174` anchor. | `§1.1`, `§8` (`WV2-1a`, `WV2-5`) |
| **r3 Fix 8 & `LD-4`** (`WV2-7` before `WV2-6`, Tauri resource path for `sim_256`) | Ordered `WV2-7` **before** `WV2-6` (or single PR; banner stays active if `WV2-6` lands first); specified Tauri bundle resource lookup on the main thread (`app.path().resource_dir()`) handed to the sim thread & `world_seed_from_disk(path)`. | `§7.1`, `§8` (`WV2-7`, `WV2-6`) |
| **r3 Fix 9 & Notes `1–7`** (Sequential `WV2-4 -> WV2-5`, `world_artifact.rs`, notes) | Sequenced **`WV2-4 -> WV2-5`** cleanly in DAG & table; marked `world_artifact.rs` as **`WV2-3`, then `WV2-7`**; folded in Note 1 (`NOTICE` in `WV2-3`), Note 2 (`11` literals in `10` files of `46`), Note 3 (`world_seed_from_disk` loud error), Note 4 (`index.json` excluded from tile archive, release approval in `WV2-5`, offline cache env, `JOB-W6` via `--sync`), Note 5 (dropped `worldComposition.ts` from `WV2-6`; named `scripts/gen_world_v2_manifest.ts`), and Note 7 (`NR-SPIKE` `G-PERF`). | `§3.3`, `§6.1`, `§7.2`, `§8` |

## Revision 2 Summary (`verdicts/WV2-D.md` & `LD-1..LD-4`)

| Review Item | Resolution Closed in Revision 2 (Preserved in r3) | Section |
|---|---|---|
| **Fix 1** (r1 misattributions & dropped refs) | Fixed Barnes 2020/2021, Seybold 2017 (*GRL*), Tucker 2001 (*Geomorphology*); dropped Cortial, Parker, Bahr, Katsuki, Seibert, Freeman, `~13%`; added Wang et al. 2018 & Kirchner 1993 caveat. | `§2.2`, `§4.1`, `§9` |
| **Fix 2, 4, 5, 6, 9–13, `LD-2`, `LD-3`** | `sim_256` without digest; dual identities (`sim_world_identity` / `bundle_identity`); committed `sim_256` + `index.json` & build-time bundled tile release assets + Proposed ADR-0010 Addendum; `Copy`-safe `WorldIdentity::sim_world_sha256: Option<[u8; 32]>`; pure-Rust `reader`; paired `GS-5`; Rust-only `G-COMP v2`; Dell Iris Xe `1,000`-creature before/after gate; separate `world_v2_manifest.json` (`map_manifest.json` untouched). | `§1.2`, `§3.2`, `§3.3`, `§5`, `§6`, `§7`, `§8` |

---

## 1. Generator Location, G2 Feature Split, Determinism & Relation to `worldGen.ts`

### 1.1 Workspace Crate `src-tauri/crates/anima-worldgen` & Strict G2 Feature Contract

World v2 lives in a dedicated **library + CLI workspace member crate `src-tauri/crates/anima-worldgen/`**:

| Component | Path (proposed) | Role | Feature gate |
|---|---|---|---|
| Library core | `src-tauri/crates/anima-worldgen/src/` | Deterministic geomorphic generator, `ANMW v3` reader/writer, offline bakes, `G-SHAPE`/`G-COMP` | `default = []`, `reader`, `generate`, `bake`, `gpu-bake` |
| Worldgen CLI | `src-tauri/crates/anima-worldgen/src/bin/anima-worldgen.rs` | Offline deterministic generator + `G-SHAPE`/`G-COMP` verifier | `required-features = ["generate"]` |
| Offline Bake CLI | `src-tauri/crates/anima-worldgen/src/bin/anima-bake.rs` | Normals, SDF, flow, lighting, KTX2/BCn, impostors/LODs, GPU probe | `required-features = ["bake"]` (+ optional `--features gpu-bake`) |

**Concrete G2 & CI invariants (verified against trunk `1ef4f0d3` / `cc40f11f`):**
1. **Workspace, CI clippy & explicit CI test scheduling (`.github/workflows/ci.yml:126,174,182`):** `src-tauri/Cargo.toml:82-96` declares `members = ["crates/*"]` and `default-members = [".", "crates/*"]` (modeled on `crates/anima-domain/Cargo.toml:1-27`). **However**, CI runs `cargo clippy --all-targets --features desktop -- -D warnings` (`.github/workflows/ci.yml:126`), `--no-default-features` (`:135`), and `cargo test --features desktop` (`:174`) on the root workspace — Cargo only enables `--features` on packages declaring them, so a feature-gated `anima-worldgen` with `default = []` would otherwise leave `tectonic.rs`/generators un-linted and compile feature-gated integration tests as empty binaries (rejected by `check_test_targets.mjs`, `:182`)!
   - **Declare `[features]` + `[[test]] required-features` in `src-tauri/crates/anima-worldgen/Cargo.toml`:**
     ```toml
     [features]
     default = []
     reader = ["dep:sha2", "dep:serde", "dep:serde_json"] # Pure Rust, ZERO C deps, ZERO zstd-sys/wgpu
     generate = ["reader", "dep:libm", "dep:rand", "dep:rand_chacha", "dep:rayon", "dep:toml"]
     bake = ["generate", "dep:bytemuck", "dep:half", "dep:image", "dep:png", "dep:zstd", "dep:meshopt", "dep:bc7enc_rdo"] # empty stub in WV2-1a until WV2-5 populates deps
     gpu-bake = ["bake", "dep:wgpu"] # Pins wgpu = "30.0.2" matching Cargo.lock burn-wgpu; NEVER enabled by desktop

     [[test]]
     name = "wv2_1a_tectonic_tests"
     required-features = ["generate"]
     # Same [[test]] required-features = ["generate"] on wv2_1b_solver_hydro_tests, wv2_2_climate_biome_cave_tests,
     # wv2_4_g_shape_comp_tests; ["bake"] on wv2_5_bake_tests; and ["gpu-bake"] on wv2_5_gpu_bake_tests,
     # paired 1-to-1 with crate-level `#![cfg(feature = "...")]` at top of each integration test file.
     ```
   - `anima-engine` depends ONLY on `anima-worldgen = { path = "crates/anima-worldgen", default-features = false, features = ["reader"] }` while keeping `--no-default-features` free of heavy/C crates (`sim_256.anmw3` is uncompressed raw `~1.44 MiB`, so zero `zstd-sys`).
   - **CI Clippy + `worldgen-cpu` Test & Target-Policy Gate (`.github/workflows/ci.yml`):**
     1. Run **`cargo clippy -p anima-worldgen --all-targets --features reader,generate,bake -- -D warnings`** in CI (and in `WV2-1a`'s DoD) so `clippy.toml` `disallowed-methods` (`§1.2`) compiles and lints every generator & CPU-bake file on every PR.
     2. Run **`cargo test -p anima-worldgen --features "reader,generate,bake" -- --nocapture 2>&1 | tee ../cargo-test-worldgen.txt`** followed by **`node ../scripts/check_test_targets.mjs ../cargo-test-worldgen.txt --profile worldgen-cpu`**.
     3. Extend **`src-tauri/tests/feature_gated_targets_tests.rs:35-145`** and **`scripts/test_target_policy.mjs:105-120`** to scan `crates/anima-worldgen` as well, declaring the `worldgen-cpu: ['reader', 'generate', 'bake']` profile and registering every `anima-worldgen` feature-gated integration test (`wv2_1a_*`, `wv2_1b_*`, `wv2_2_*`, `wv2_4_*`, `wv2_5_bake_*`, plus `wv2_5_gpu_bake_tests` under `gpu-bake` for GPU-less `windows-latest` CI).
2. **Extended G2 `cargo tree` gate (`.github/workflows/ci.yml:136`):** Extend the CI G2 leak check to inspect **both** `cargo tree -p anima-engine --no-default-features -e normal` and `cargo tree -p anima-engine -e normal` (default features), rejecting any match for:
   `neo4rs|tokio-tungstenite|burn-wgpu|naga|zstd-sys|wgpu|ktx2|meshopt|bc7enc`
   (`anima-engine --features desktop` remains pinned to `wgpu 30.0.2` from `Cargo.lock` only via `burn-wgpu`; `gpu-bake` pins the identical `wgpu = "30.0.2"` so `cargo update`/resolver never builds two `wgpu` trees).
3. **Licensing & `dataflow.json` separation:** `[workspace.package]` (`src-tauri/Cargo.toml:98-108`) sets `license = "MIT OR Apache-2.0"`, `scripts/lib/licensing.mjs:218` treats workspace crates as first-party, and `scripts/gen_notice.mjs:190` registers `anima-worldgen` alongside `anima-domain`. Keeping geomorphic generation and offline baking inside `crates/anima-worldgen` leaves the internal `anima_engine_lib` `dataflow.json` graph unchanged during `WV2-1a..WV2-5`.

### 1.2 Cross-Machine CPU Determinism Contract & Classification

Per `QD-OXS` and Map v2 `§3c`, outputs must be explicitly classified into **exact CPU-deterministic** vs. **pinned-run with pre-declared tolerance**:

1. **`CPU-deterministic` (`JOB-W1`, `JOB-W2`, `JOB-W3`, `JOB-W4 --backend cpu`, `index.json` assembly):** Must yield **byte-identical SHA-256** across macOS `arm64` (MacBook Air M5) and Windows `x86_64` (`desktop-3epv830`):
   - **PRNG discipline:** `rand_chacha::ChaCha12Rng` (`src-tauri/Cargo.toml:64-67`), domain-separated per stage by `SHA-256("ANIMA-WV2" || seed_u64_le || width || height || schema_tag || stage_tag)`.
   - **Mandatory `libm` + scalar math enforcement (`src-tauri/crates/anima-worldgen/clippy.toml`):** Rust already guarantees IEEE-754 non-contracted arithmetic and hardware-rounded `sqrt`, so host divergence risk comes from std `f32`/`f64` transcendentals and SIMD reductions. Enforce `clippy.toml` **`disallowed-methods`** banning every `f32::`/`f64::` std transcendental (`sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `exp`, `exp2`, `ln`, `log2`, `log10`, `powf`, `powi`, `hypot`, `sinh`, `cosh`, `tanh`), routing all transcendentals through pure-Rust `libm` (`0.2.16` in `Cargo.lock`), and ban `glam` SIMD types (`Vec3A`, `Vec4`, SIMD `Mat4`) inside `anima-worldgen`.
   - **Order-independent buffers & reductions:** Dense `Vec<f64>`/`Vec<f32>` grids indexed by `y * w + x`, deterministic `BTreeMap` if sparse indexing is needed, fixed-order Priority-Flood tie-break `(elev_bits, cell_idx)`, and fixed deterministic reduction trees under `rayon` (never non-associative parallel float `.sum()`).
2. **`Pinned-run + Tolerance` (`JOB-W4 --backend wgpu`, `JOB-W5` C/C++ texture encoders, `JOB-W6` GPU impostors):**
   - Because `bc7enc_rdo` and Basis/UASTC encoders contain C/C++ SIMD code paths whose cross-ISA byte-identity between Apple Silicon `arm64` and `x86_64` AVX2 is **not proven without a replica**, `JOB-W5` texture encoding is classified **pinned-run + pre-declared tolerance (`Block PSNR >= 42 dB, SSIM >= 0.996`)** by default (promoted to `CPU-deterministic` only if a cross-OS replica run verifies exact SHA-256 parity). Because `sim_world_identity` (`LD-2`) hashes ONLY `sim_256.anmw3` (produced strictly by CPU `JOB-W1`), visual encoder differences never touch simulation identity.

### 1.3 Relation to TypeScript `worldGen.ts`

Today, the WebView generates `2048²` worlds via `src/components/Landscape/utils/worldGen.ts:17,1166-2075` (`WORLD_GEN_VERSION = 22`), configured in `src/utils/sharedWorld.ts:16-35`, downsampled to `256²` (`SIM_ARTIFACT_SIZE = 256`, `worldCache.ts:7`), and sent via IPC `save_world_artifact` (`commands/environment.rs:128`) to `$TMPDIR/anima_world_artifact.anmw` (`world_artifact.rs:462-466`), while `scripts/gen_world_manifest.ts` writes committed `artifacts/world_256.anmw` (`sha256:8da978ff...`, `map_manifest.json:19`).
- **Keep `worldGen.ts`, `artifacts/world_256.anmw`, and `map_manifest.json` 100% frozen/read-only during P1–P3:** preserves legacy v2 tests and `warmSharedWorld`. Never run `npm run gen:world-manifest` as part of World v2 cards (`Fix 13`).
- **Bypassed at World v2 switchover (`WV2-6` + `WV2-7`):** World v2 scenes load pre-baked tiles in Three.js and `sim_256.anmw3` in `AppArtifactOrGenerated`; `worldGen.worker.ts` and `save_world_artifact` are bypassed completely when World v2 is active. Once native renderer parity ships post-P4, `worldGen.ts` can be retired.

---

## 2. Tropical Coastal Massif Scale (`LD-1`, `LD-5`), Geomorphic Pipeline (`S1–S9`), & Runtime Basis

### 2.1 Coherent Regional Physical Scale (`LD-1` Corrected & `LD-5`)

1. **Terrain physical extent & setting (`LD-5`):** A single **tropical coastal massif** domain of **`40.96 km × 40.96 km`** (`40,960 m × 40,960 m`, total dry land area `~840–1,000 km²` at `oceanFraction ∈ [0.40, 0.50]`), modeled on the **Sierra Nevada de Santa Marta, Colombia** (`~11°N` fixed latitude, snowy/glaciated summits `> 5,500 m` within `~42 km` of the Caribbean coast; `[45, 46, 47, 48]`):
   - **`4096²` (P1 default target):** **`Δx = 10.0 m/cell`** (comparable to USGS 3DEP `10 m` DEMs; resolves gullies, meander point bars, beach berms, and glacial cirques cleanly)
   - **`8192²` (grid/high-res):** **`Δx = 5.0 m/cell`**
   - **`2048²` (fast CI/dev):** **`Δx = 20.0 m/cell`**
   - **`256²` sim grid:** **`Δx_sim = 160.0 m/cell`** mapped linearly into `[-100, +100]` sim units (`WORLD_MIN_XZ..WORLD_MAX_XZ`, `sim_rules.rs:51-53`; `0.78125` sim units/cell, `map_manifest.json:28`).
2. **Vertical scale (`LD-5` snowline & glaciers within `40.96 km`):** Physical elevation $h_{\text{phys}} \in [-1,500\text{ m}, +5,700\text{ m}]$ (`7,200 m` total span) with `sea_level_m = 0.0 m` ($h_{\text{norm}} = (h_{\text{phys}} + 1500)/7200 \in [0, 1]$, `sea_level_norm = 1500/7200 = 0.20833333`), mapping into `sim_rules.rs:56-58` `WORLD_MIN_Y..WORLD_MAX_Y = 0.0..10.0`. Raising the peak ceiling to `+5,700 m` (matching Sierra Nevada de Santa Marta summits `[46, 47]`) allows the world to genuinely cross the tropical glacier Equilibrium Line Altitude (`ela_base_m = 4,900 m ∈ [4,800, 5,100] m` `[46, 47, 48]`, with design altitudinal zoning informed by SNSM Massenerhebung compression `[45]`).
3. **Creature scale declaration (`LD-1`):** Creatures are simulated and rendered at a declared **stylised scale** that is **not to terrain scale** (analogous to real-time strategy and ecosystem simulations); creature body segment length (`length: 1.0` in `simulation_loop.rs:1553,1559,1565` and `live_experiment.rs:2767`), movement speed, and sensory/interaction radii are **never derived from terrain metres per cell**, leaving all simulation physics and registered experiments untouched.

### 2.2 Detailed Geomorphic & Climate Pipeline (`S1–S9`)

*Execution order rule (`Fix 10`):* S1 uplift + S2/S3 hillslope/fluvial erosion & S5 routing/discharge run **before** S4 alluvial fans, meanders, and coastal deltas (because fans, meanders, and deltas require routed discharge $Q$ and network topology!).

| Stage | Card / Module (proposed) | Replaces trunk v1 (`worldGen.ts`) | Core Output Buffers |
|---|---|---|---|
| **S1** Tectonic Uplift & Lithology | `WV2-1a` / `tectonic.rs` | `Pass 1` (`1204-1259`) fixed fBm | `bedrock_h`, `uplift_u`, `lithology_id`, `stratum_k` |
| **S2** Coupled Stream-Power LEM (Fastscape-SD) | `WV2-1b` / `erosion_fluvial.rs` | `Pass 1b` (`489-582,1267`) short droplets | `bedrock_h`, `sediment_s` (`512²→N²`) |
| **S3** Nonlinear Hillslope & Thermal Talus | `WV2-1b` / `erosion_thermal.rs` | None (v1 left spiky peaks) | `bedrock_h`, `sediment_s`, `talus_frac` |
| **S5** Hybrid MFD→D8 Flow Routing & Hydraulic Geom | `WV2-1b` / `hydrology.rs` | `Pass 2/2b` (`1327-1439`) D8 + ribbon | `flow_dir`, `flow_acc`, `discharge_q`, `river_w/d`, `water_h` |
| **S4** Alluvial Fans, Meanders & Coastal Deltas | `WV2-2` / `deposition.rs` | `Pass 4d` (`1710-1760`) 1–4px discs | `sediment_s`, `alluvial_mask`, `meander_scroll`, `water_turbidity` |
| **S6** Priority-Flood Lakes & Knickpoint Waterfalls | `WV2-2` / `lakes_falls.rs` | `computeLakes` (`584-852`), `Pass 4e` (`1762-1844`) | `lake_id`, `water_h`, `spillways`, `waterfalls[]` |
| **S7** Regional Orographic Climate & TWI | `WV2-2` / `climate.rs` | `Pass 3` (`1440-1569`) 1-D row E/W sweep | `temp_c`, `precip_m_yr`, `moisture`, `twi` |
| **S8** Continuous Biomes, Glaciers & Dunes | `WV2-2` / `biomes.rs` | `Pass 4/4a` (`272-488,1578-1633`) | `biome_ids[4]`, `biome_w[4]`, `glacier_h`, `dune_h` |
| **S9** Data-Only Karst/Basalt Caves | `WV2-2` / `caves.rs` | `Pass 4f` (`1845-1895`) slope dots | `cave_mask`, `cave_network.bin` (`QD-REALMS`) |

#### 1. Regional Uplift Structure & Stratigraphic Lithology (`S1`)
- **Sources:** Cordonnier et al. (2016) `[7]`, Cordonnier et al. (2018) `[8]`.
- **Formulation across `40.96 km` regional margin (`LD-1`, no plate counts / no supercontinent claims):**
  Compose regional uplift domains via signed distance fields while preserving `oceanFraction ∈ [0.40, 0.50]` (`worldGen.ts:1191-1203` composition balance):
  1. **Active Coastal Mountain Spine & Foreland Plain:** Asymmetric orogenic uplift profile $U(\mathbf{x}) = U_0\,\text{sech}^2((d_{\text{ridge}} - \delta)/w)\,(1 + 0.35\,\text{fbm}_{\text{warp}})$ with a steep oceanward flank plunging straight to surf (`CST-03` Lost Coast), a glaciated high alpine spine (`MTN-01`–`MTN-06`), and a gently dipping inland/coastal foreland (`TUN-02`).
  2. **Normal-Fault Rift / Structural Valley:** Fault-bounded trough capable of hosting elongated steep-walled lakes (`LAK-03`) and broad verdant valleys (`RIV-04`).
  3. **Horizontal Caprock Plateau (`15–20 km` tableland):** Resistant sandstone stratum ($K_{\text{cap}} = 0.22\,K_{\text{shale}}$ at $h \in [450\text{ m}, 1100\text{ m}]$) overlying softer shale so fluvial incision yields flat-topped plateaus split by branching steep-walled wadis/canyons (`AER-01`, `AER-06`, `DES-03`, `RIV-03`).
  4. **Volcanic Stratocone & Collapsed Caldera Rim:** Single volcanic edifice (`MTN-05`) with nested caldera crater rim hosting an alpine crater lake (`LAK-01`).
  5. **6-Class Lithology Map:** `0 = Granite/Crystalline`, `1 = Sandstone Caprock`, `2 = Soluble Limestone/Karst`, `3 = Basalt/Volcanic`, `4 = Soft Shale/Mudstone`, `5 = Unconsolidated Alluvium/Sand`, giving up to `8×` erodibility contrast and natural lithologic knickpoints for waterfalls (`RIV-02`).

#### 2. Coupled Stream-Power Fluvial Erosion–Deposition (`S2`, r3 Fix 6, r4 Fix 1 & Note 7)
- **Sources:** Braun & Willett (2013) `[5]`, Davy & Lague (2009) `[10]`, Yuan, Braun, Guerit, Rouby & Cordonnier (2019) `[44]`, Braun (2022) `[49]`, Whipple & Tucker (1999) `[42]`.
- **Dimensionally Consistent Governing PDE (Precipitation-Weighted Area Form, Yuan et al. 2019 `[44]`, Braun 2022 `[49]` eqs. 7–8):**
  Let $\tilde{A}(\mathbf{x}) = \int_{\mathcal{A}(\mathbf{x})} \nu(\mathbf{x}') \, dA'$ (`[L²]`) be upstream **precipitation-weighted** drainage area (NOT runoff `max(0, P - E)`!), where $\nu(\mathbf{x}) = P(\mathbf{x}) / \nu_0$ is dimensionless relative precipitation (with reference rate $\nu_0 = 1.0\text{ m/yr}$ and strictly positive annual rainfall everywhere including leeward rain shadows $P(\mathbf{x}) \ge P_{\min} = 0.15\text{ m/yr} \implies \nu(\mathbf{x}) \ge 0.15 > 0$, so $\tilde{A}_i \ge \nu_{\min}\Delta x^2 > 0$ everywhere and rain-shadow wadis/gorges `AER-01`, `DES-03`, `RIV-03` still incise without $\beta_i \to \infty$ singularity; Braun 2022 `[49]`'s dimensionless deposition number **$G = d^* v_s / \nu_0$** uses dimensionless $d^*$ instead of Carretier's dimensional $\zeta$ `[T/L]`):
  $$\frac{\partial h}{\partial t} = U(\mathbf{x}, t) - K(\mathbf{x}, h) \, \tilde{A}(\mathbf{x})^m \, |\nabla h|^n + \frac{G}{\tilde{A}(\mathbf{x})} \int_{\mathcal{A}(\mathbf{x})} \left(U(\mathbf{x}', t) - \frac{\partial h}{\partial t}(\mathbf{x}')\right) dA' + D(\mathbf{x})\nabla^2 h$$
  Verify units: left-hand side `[L/T]`; $(G / \tilde{A}) \int (U - \partial_t h)\, dA' \sim (1/L^2)(L/T \cdot L^2) = [L/T]$; and at steady state ($\partial_t h = 0$) under uniform $\nu = 1$ ($\tilde{A} = A$), $K \tilde{A}^m S^n = (1 + G) U$, giving exact analytic steady-state slope $S^* = ((1+G) U / (K \tilde{A}^m))^{1/n}$ with $n = 1.0$, $m = 0.45$ ($\theta = m/n = 0.45 \in [0.35, 0.65]$, `[42]`), and $G \in [0.0, 1.5]$ ($G = 0.85$ default).
- **True Discrete Algorithm (Including Local Node $i$ Term & No Extra $\Delta t$):**
  - When **$G = 0$**: classic Fastscape solves implicitly in a single downstream-to-upstream receiver-stack pass:
    $$h_i^{t+\Delta t} = \frac{h_i^t + \Delta t U_i + \alpha_i \, h_{r(i)}^{t+\Delta t}}{1 + \alpha_i}, \qquad \alpha_i = \frac{K_i \tilde{A}_i^m \Delta t}{d_i}$$
  - When **$G > 0$**: exact sediment deposition at node $i$ integrates $(U - \partial_t h)\,dA'$ over all upstream nodes $k \in \text{up}(i)$ **and node $i$ itself**, giving a per-step volume `(Φ_in(i) + local_term_i)` divided by $\tilde{A}_i$ (`G · (Φ_in + local_term_i) / Ã_i`, `[L]`, **zero extra `Δt`**). Writing $\beta_i = G \Delta x^2 / \tilde{A}_i$ (bounded strictly finite $\beta_i \le G/\nu_{\min}$ everywhere because $\nu_i = P_i / \nu_0 \ge 0.15 > 0$), Yuan et al. (2019) `[44]` solves implicitly in $h$ via a deterministic Gauss–Seidel fixed-point iteration over $\Phi_{\text{in}}$ until convergence ($R_{\max} = 32$, $\max_i |h_i^{(r+1)} - h_i^{(r)}| < 10^{-6}\text{ m}$):
    1. Initialize $h^{(0)} = h^t$.
    2. For $r = 0, 1, \dots, R_{\max}-1$:
       - **Upstream-to-downstream flux sweep:**
         $$\Phi_{\text{in}}^{(r)}(i) = \sum_{k \in \text{up}(i)} \Bigl[\bigl(h_k^t + \Delta t U_k - h_k^{(r)}\bigr)\Delta x^2 + \Phi_{\text{in}}^{(r)}(k)\Bigr]$$
       - **Downstream-to-upstream implicit height update (with node $i$'s own local term folded cleanly into both sides):**
         $$h_i^{(r+1)} - h_i^t = \Delta t U_i - \alpha_i \bigl(h_i^{(r+1)} - h_{r(i)}^{(r+1)}\bigr) + \frac{G}{\tilde{A}_i}\Bigl[\Phi_{\text{in}}^{(r)}(i) + \bigl(h_i^t + \Delta t U_i - h_i^{(r+1)}\bigr)\Delta x^2\Bigr]$$
         $$\implies h_i^{(r+1)} = \frac{(h_i^t + \Delta t U_i)(1 + \beta_i) + \alpha_i \, h_{r(i)}^{(r+1)} + \frac{G}{\tilde{A}_i}\Phi_{\text{in}}^{(r)}(i)}{1 + \alpha_i + \beta_i}$$
    Check channel heads ($\tilde{A}_i = \nu_i\Delta x^2 \implies \beta_i = G/\nu_i$, $\Phi_{\text{in}} = 0$): at $\nu_i = 1$ and steady state $h_i^{(r+1)} = h_i^t$, this yields $(1 + G)\Delta t U_i = \alpha_i (h_i - h_{r(i)}) \implies K \tilde{A}_i^m S_i = (1+G)U_i$, matching Yuan et al. `[44]` identically!
- **Multigrid schedule:** Solve coarse-to-fine `512²` (`60` macro steps) $\to$ `1024²` (`40` steps) $\to$ `2048²` (`25` steps) $\to$ `4096²` (`15` steps) $\to$ optional `8192²` (`8` steps), replacing trunk v1's `<= 120,000` short droplets (`worldGen.ts:1267`, `terrain.rs:72-73`).

#### 3. Thermal Erosion & Nonlinear Hillslope Diffusion (`S3`)
- **Sources:** Musgrave, Kolb & Mace (1989) `[27]`, Roering, Kirchner & Dietrich (1999) `[34]`.
- Nonlinear soil creep $q_s = -D \nabla h / (1 - (|\nabla h|/S_c)^2)$ rounds mountain ridges (`TMP-01`, `TUN-01`), eliminating v1 needle spikes; lithology-dependent talus relaxation ($\alpha_{\text{scree}} = 34^\circ$, $\alpha_{\text{soil}} = 28^\circ$, $\alpha_{\text{caprock}} = 65^\circ$, $\alpha_{\text{granite}} = 58^\circ$) sheds excess regolith downslope into explicit `sediment_s` aprons (`MTN-01`, `DES-03`, `MTN-06`).

#### 5. Hybrid MFD→D8 Flow Routing & Runoff-Derived Hydraulic Geometry (`S5`, r3 Fixes 2 & 4, Note 8)
- **Sources:** O'Callaghan & Mark (1984) `[28]`, Quinn et al. (1991) `[31]`, Holmgren (1994) `[16]`, Barnes, Lehman & Mulla (2014) `[1]`, Montgomery & Dietrich (1988, 1989) `[26]`, Leopold & Maddock (1953) `[23]`.
- **Hybrid MFD→D8 routing (design choice & tuning):** After exact $O(N \log N)$ Priority-Flood depression resolution (`Barnes et al. 2014` `[1]`), divergent hillslopes below channel initiation ($A S^2 < \tau_{\text{chan}}$, `Montgomery & Dietrich 1988, 1989` `[26]`) distribute runoff with multiple flow direction (MFD). **MFD exponent `p = 1.6` is this design's own tuning parameter** (chosen intermediate between Quinn et al. 1991 `[31]` linear MFD `p = 1` and Holmgren 1994 `[16]` sharper exponent `p ∈ [4, 6]`), transitioning cleanly at $A S^2 \ge \tau_{\text{chan}}$ (`Q >= Q_chan`) to single-receiver D8 (`O'Callaghan & Mark 1984` `[28]`) so trunk channels form a strict topological tree. *(Explicitly not attributed to either paper as `1.6`.)*
- **Runoff-derived mean annual discharge & regional river widths (`r3 Fix 4`, `r4 Fix 1`, Note 2):** Effective runoff `max(0, P - E_pot)` is used **strictly for downstream hydraulic-geometry discharge $Q_{\text{ma}}$** (never in Fastscape-SD $\tilde{A}$). Define local mean annual runoff **rate** uniquely in `m/yr` as **$R(\mathbf{x}) = \max(0, P(\mathbf{x}) - E_{\text{pot}}(T))$** (`[L/T]`), so per-cell runoff volume is $q_{\text{cell}}(\mathbf{x}) = R(\mathbf{x})\,\Delta x^2$ (`m³/yr`), routed down the DAG to give catchment discharge $Q_{\text{ma}} = \bar{R}\cdot A$. Behind our humid windward $R \in [1.2, 2.4]\text{ m/yr}$ (upper bound `3.0 m/yr`), the climate model sets windward precipitation **$P \approx 2.5\text{–}3.8\text{ m/yr}$** (pluvial windward slopes up to `~4.0 m/yr`) against tropical lowland **$\text{PET} \approx 1.2\text{–}1.5\text{ m/yr}$** (decreasing with altitude), while leeward rain-shadow basins receive **$P \approx 0.15\text{–}0.50\text{ m/yr} < \text{PET}$** (water-balance structure informed by SNSM hydro-climatic studies `Hoyos et al. 2019` `[50]` and `Valencia et al. 2022` `[51]`). Across our `40.96 km` coastal massif (`~840–1,000 km²` dry land, `~20–24 km` coast-to-divide width), major windward drainage basins span **$A \in [120, 300]\text{ km}^2$** with mainstem paths of `~22–35 km` (sinuosity `~1.25–1.45` fitting comfortably inside the coastal massif land depth; `[12, 13, 32]`), yielding mean annual trunk discharge **$Q_{\text{ma}} = \bar{R} \cdot A \approx 4.6\text{–}22.8\text{ m}^3/\text{s}$** ($\lesssim 29\text{ m}^3/\text{s}$ for a `300 km²` catchment). Applying Leopold & Maddock (1953) `[23]` downstream hydraulic geometry ($b=0.50$, $f=0.40$, $m=0.10$, $w = k_w Q_{\text{ma}}^{0.50}$ with $k_w \approx 2.6\text{–}3.4\text{ m}^{-0.5}\text{s}^{0.5}$, $d = k_d Q_{\text{ma}}^{0.40}$ with $k_d \approx 0.25\text{ m}^{-0.2}\text{s}^{0.4}$) yields **`~6–18 m` wide regional trunk rivers** (`0.6–1.8` cells @ `10 m/px`, widening across coastal tidal mouths & braided deltas to **`~20–45 m`**), stored with unit 2-D flow vector $(\hat{u}_x, \hat{u}_z)$ and `water_h`.

#### 4. Sediment Deposition: Fans, Meanders & Coastal Deltas (`S4`, runs *after* `S5`)
- **Sources:** Yuan et al. (2019) `[44]`, Cordonnier et al. (2016) `[7]`, Howard & Knutson (1984) `[19]`, Liang, Voller & Paola (2015) `[24]`.
- **Mechanisms:**
  1. **Piedmont Alluvial Fans (`AER-03`):** At canyon-front slope drops ($\ge 4\times$), fan packet deposition spreads radially over `sediment_s` leaving one active wet distributary channel and dry abandoned braids.
  2. **Meandering Floodplains, Oxbow Lakes & Scroll Bars (`AER-07`, `RIV-01`, `RIV-04`):** Low-slope high-$Q$ trunk reaches ($S < 0.004$) evolve lateral curvature migration for `35` kinematic steps (`Howard & Knutson 1984` `[19]`), carving flat floodplains, inner sand/gravel point bars, outer cut banks, oxbow cutoffs, and concentric scroll topography.
  3. **Tundra Anastomosing Channels & Prograding Deltas (`AER-08`, `AER-04`, `AER-05`, `WET-04`):** Coastal river mouths on shallow shelves ($\Delta z < 40\text{ m}$) run reduced-complexity DeltaRCM parcel routing (`1,500–4,000` sediment+water parcels per major delta mouth, `Liang et al. 2015` `[24]`) building protruding distributary lobes, beach ridges, tidal channels, and coastal turbidity plumes (`water_turbidity`).

#### 6. Lakes by Depression Hierarchy & Knickpoint Waterfalls (`S6`, Fix 1)
- **Sources:** **Barnes, Callaghan & Wickert (2020)** `[2]` (*"Finding hierarchies in depressions and morphological segmentations"*, *ESurf* 8(2):431–445) for building the depression hierarchy; **Barnes, Callaghan & Wickert (2021)** `[3]` (*"Part 3: Fill–Spill–Merge: flow routing in depression hierarchies"*, *ESurf* 9(1):105–121) for **Fill–Spill–Merge** lake-level & spillway routing; **Howard, Dietrich & Seidl (1994)** `[20]` & **Whipple & Tucker (1999)** `[42]` for knickpoint migration.
- Humid-basin lakes (`Q_in > E_lake`) fill to their exact spillway contour (`LAK-01`..`03`) and discharge continuously downstream (`0` severed spillways); rain-shadow arid closed basins (`Q_in <= E_lake`) equilibrate cleanly at $A(h_{\text{lake}})\,\text{PET} = Q_{\text{in}}$ (`EndorheicLake`). Waterfalls form structurally at lithologic caprock drops ($\Delta h \ge 12\text{ m}$ over $\le 30\text{ m}$, upstream $S < 0.08$), carving gorge lips (`RIV-03`), plunge pools (`RIV-02`), and `WaterfallRecord` entries (`[20, 80]`).

#### 7. Regional Climate: Fixed Latitude, Orographic Rain, Rain Shadow, Lapse Rate & TWI (`S7`, `LD-1` Corrected / `LD-5`, `r4 Fix 3`)
- **Sources:** Smith & Barstad (2004) `[36]`, Roe (2005) `[33]`, Beven & Kirkby (1979) `[4]`, Fortier et al. (2025) `[45]`, López-Moreno et al. (2020) `[46]`, Poveda & Pineda (2009) `[47]`, Rabatel et al. (2013) `[48]`, Hoyos et al. (2019) `[50]`, Valencia et al. (2022) `[51]`.
- **Strict single-latitude regional formulation (`LD-1` corrected & `LD-5` — ZERO latitudinal temperature/wind belts across the map):**
  1. **One fixed regional latitude ($\phi_0 = 11^\circ\text{N}$, total `40.96 km` map spans only `~0.37°`, no latitudinal climate gradient) & explicit ELA (`r4 Fix 3`):** Warm tropical Caribbean sea-level baseline $T_0 \approx 27.5\text{ °C}$ + coastal maritime buffering + SNSM-calibrated environmental lapse rate **$\Gamma = 6.3\text{ °C/km}$ (`Hoyos et al. 2019` `[50]`)** + solar aspect modulation ($\mathbf{n}\cdot\mathbf{s}_{\text{noon}}$) compresses air temperature from hot lowlands (`~27 °C`) across the annual-mean $0\text{ °C}$ air isotherm (`~4,365 m`, nocturnal-frost superpáramo) up to freezing high summits (`-8.4 °C` @ `+5,700 m`). **Perennial `Snow` and `Glacier` onset is explicitly decoupled from the $0\text{ °C}$ air isotherm and driven by an explicit Equilibrium Line Altitude parameter `ela_base_m = 4,900 m`** (`∈ [4,800, 5,100] m` `[47, 48]`), modulated locally via $\text{ELA}(\mathbf{x}) = \text{ela\_base\_m} + \Delta_{\text{aspect}}(\mathbf{n}\cdot\mathbf{s}_{\text{noon}}) + \Delta_{\text{precip}}(P)$ ($\pm 180\text{ m}$ aspect, $\pm 120\text{ m}$ snow accumulation), reproducing Sierra Nevada de Santa Marta Massenerhebung altitudinal compression (`Fortier et al. 2025` `[45]`) and keeping glacier/perennial-snow coverage strictly aligned with the cited `4,800–5,100 m` ELA.
  2. **Prevailing tropical onshore wind & 2-D linear orographic precipitation (`Smith & Barstad 2004` `[36]`; `Roe 2005` `[33]`):** A single regional prevailing onshore wind $\mathbf{u}_0$ (deflected smoothly around major peaks on a `512²` grid) drives column moisture advection + windward condensation ($\propto \max(0, \mathbf{u}\cdot\nabla h)$) and downwind rainout/leeward adiabatic drying, generating **humid windward coastal lowlands and montane cloud/rainforest (`RNF-01`, `TMP-01`, `TAI-01`)** on the windward massif flank and a **pronounced arid leeward rain-shadow basin/badlands (`DES-01..03`)** on the leeward flank — eliminating trunk v1's 1-D row streaks (`worldGen.ts:1290`).
  3. **Topographic Wetness Index (`Beven & Kirkby 1979` `[4]`):** $\text{TWI} = \ln((A_{\text{spec}} + \Delta x)/\max(\tan\beta, 0.002))$ concentrates persistent moisture in valley floors (`RIV-04`), riparian bands (`TAI-02`), and wetlands (`WET-02`).
  4. **2-pass climate–erosion coupling** runs at `512²→1024²` so wetter windward flanks incise denser drainage networks than leeward rain shadows.

#### 8. Continuous Biomes, Glaciers & Aeolian Dunes (`S8`, Fix 1)
- **Sources:** Whittaker (1975) `[43]`, Holdridge (1947) `[15]`, Oerlemans (2001) `[29]`, Cuffey & Paterson (2010) `[9]`, Werner (1995) `[41]`.
- Continuous smooth $C^2$ log-likelihood blending across the 22 canonical biomes (`world_artifact.rs:399-401`) outputting **top-4 `biome_ids` (`u8×4`) + normalized `biome_weights` (`sum = 255`)**, domain-warped cleanly at micro-ecotone scale (`40–150 m`). Alpine cirque glaciers (`Oerlemans 2001` `[29]`; `Cuffey & Paterson 2010` `[9]`) sit strictly above local `ELA(x)` (`§2.2.7` item 1) and exclude slopes $> 42^\circ$ (`MTN-06`), carrying supraglacial moraine debris (`MTN-03`); the driest leeward sandy pocket organizes wind-aligned dune fields (`Werner 1995` `[41]`, `~12°` stoss / `32–34°` avalanche slip faces, **`DES-01`/`DES-02` stylised visual benchmark**) grading smoothly into leeward xerophytic thorn scrub (`Desert` proxy) and eroded semi-arid slopes (`Badlands` proxy; Note 3).

#### 9. Data-Only Cave Networks (`S9`, `QD-REALMS`)
- **Sources:** Palmer (1991) `[30]`.
- Generated exclusively in soluble limestone/karst (`lithology == 2`) and basalt (`lithology == 3`), avoiding permafrost peaks and oceans (`count ∈ [30, 48]`, pairwise spacing $\ge 0.01 \cdot \text{size}$, `worldComposition.ts:95-96`).
- **Explicit binary format for `cave_network.bin` (hand-specified little-endian layout, avoiding Rust RC serializer lock-in):**
  Little-endian packed layout readable identically in Rust and TypeScript:
  - Header (`24` bytes): `magic = b"ANMC"` (`0..4`), `version = 1u32` (`4..8`), `flags = 0u32` (`8..12`), `node_count u32` (`12..16`), `edge_count u32` (`16..20`), `entrance_count u32` (`20..24`).
  - `node_count × 20` bytes: `(x_m: f32, y_m: f32, z_m: f32, radius_m: f32, light_decay: f32)`.
  - `edge_count × 12` bytes: `(from_idx: u32, to_idx: u32, passage_kind: u8, pad: [u8; 3])`.
  - `entrance_count × 20` bytes: `(node_idx: u32, mouth_kind: u8, realm_status: u8 /* 1 = decorative_until_underground_adr */, pad: [u8; 2], yaw_rad: f32, elevation_m: f32, clearance_m: f32)`.
- **Candidate for a future underground-realm ADR (not decided, not part of `WV2-2`):** the graph could later carry cave energy pools fed by surface detritus seeping down from the cells above each node, moving EU between pools that join `closed_total_eu` rather than creating any. See [`QD_REALMS_HABITATS_RESEARCH_2026-11-22.md`](../../research/QD_REALMS_HABITATS_RESEARCH_2026-11-22.md) §3.

### 2.3 Algorithm Cost Basis & Runtime Estimates (`2048²`, `4096²`, `8192²`)

Hardware baselines (`AGENT_RULES.md:26-28`): Mac M5 (10 cores, 32 GB, fanless) vs. `desktop-3epv830` (i5-14600KF 14c/20t, RTX 5060 Ti).

- **Cost basis justifying key estimates (non-blocking note fix):**
  - `JOB-W4` CPU horizon lighting uses **Stewart (1998)** `[37]` fast $O(N \cdot A)$ progressive-horizon sweep along each azimuth line rather than naive raymarching ($O(N \cdot A \cdot R)$): at `4096²` (`16.78M` cells) × `16–32` azimuths, total work is `2.7e8–5.4e8` simple max-slope scans (NOT `1.4e11` samples!), fitting `~22–42 s` across 10 M5 cores.
  - `DeltaRCM` (`Liang et al. 2015` `[24]`) runs `1,500–4,000` water/sediment parcels per major river mouth (`2–4` delta mouths -> `6k–16k` short weighted-random walks of `~100–300` hops = `~1–4M` scalar ops total, `< 2 s`).
  - `WV2-1b` **must record measured Mac M5 single-step timings** for 1 `4096²` Priority-Flood pass and 1 `4096²` Fastscape-SD iteration to anchor this analytical table in real benchmarks.

| Workload | `2048²` (`20 m`/px) Est. | `4096²` (`10 m`/px) Est. (P1 Default) | `8192²` (`5 m`/px) Est. | Peak RAM (`4096²` / `8192²`) |
|---|---|---|---|---|
| **`JOB-W1` Core Worldgen (`S1–S9`)** Mac M5 / i5-14600KF | `~2.5–4.5 s` / `~1.8–3.5 s` | **`~10–18 s`** / **`~7–14 s`** | `~45–85 s` / `~32–65 s` | `~1.05 GB` / `~4.2 GB` |
| **`JOB-W2` `G-SHAPE`/`G-COMP` Verify** | `~0.6–1.2 s` | **`~2.5–5.0 s`** | `~11–22 s` | `~0.45 GB` / `~1.8 GB` |
| **`JOB-W3` Fields + Meijster SDF + Flora** | `~1.5–3.0 s` | **`~6–11 s`** | `~26–48 s` | `~0.60 GB` / `~2.4 GB` |
| **`JOB-W4` Static Lighting** (Stewart CPU sweep on M5 / RTX `wgpu`) | `~5–10 s` / `<0.5 s` | **`~22–42 s`** / **`~1–2.5 s`** | `~95–190 s` / `~4–9 s` | `~0.50 GB` / `~1.9 GB` (`0.4 GB` VRAM) |
| **`JOB-W5` KTX2/BCn & Mips** (M5 / i5) | `~3–6 s` | **`~12–25 s`** / `~8–16 s` | `~55–110 s` / `~32–65 s` | `~0.75 GB` / `~2.5 GB` |
| **End-to-End Build + Bake** | `~13–25 s` | **`~53–101 s` (M5)** / **`~26–52 s` (Grid RTX)** | `~230–455 s` / `~85–185 s` | `<1.2 GB` / `<4.3 GB` |

---

## 3. Baked Layers (`§3b`), `ANMW v3`, Dual Identities (`LD-2`), Storage (`LD-3`), & Compatibility

### 3.1 Anchoring to Map v2 `§3b` Frame Budgets (`2026-11-20-feature-map-v2.md:74-135`)

| System | **Iris Xe Low** (`33.3 ms`, 720p upscaled) | **RTX 5060 Ti High** (`16.6 ms`) | What World v2 Offline Output Makes Zero-Cost (`~0 ms` runtime) |
|---|---|---|---|
| Terrain | `6.0 ms` | `3.0 ms` | Eroded DEM + BC5 normals + pre-baked directional `terrain_lighting` (horizon AO, sky visibility, 4-azimuth horizon elevation angles for `R3` day/night shadows without shadow maps, bounce probe) + top-4 `biome_ids`/`weights` + `macro_albedo` |
| Vegetation | `6.0 ms` | `4.0 ms` | Pre-solved decorative `flora_tiles` (`§6` toggled) + shared offline `meshopt` LODs & octahedral impostors |
| Water | `3.0 ms` | `2.0 ms` | Precomputed `water_surface` + exact Meijster `[25]` `shore_sdf` + `hydro_flow` directional advection |
| Sky & Atmosphere | `1.5 ms` | `1.0 ms` | Coupled with `R3` offline LUTs; `terrain_lighting` horizon angles track `R3` sun elevation on Low & High |
| Creatures (`1,000`) | `5.0 ms` | `2.5 ms` | Consumes `256²` `sim_256.anmw3` `TerrainMap` & `LAND1` walkability unchanged |
| Post / Reserve | `11.8 ms` (`4.0 + 7.8`) | `4.1 ms` (`2.0 + 2.1`) | Zero JS terrain gen or horizon AO CPU stall on boot |

### 3.2 Baked Layer Inventory, WebGL2 Formats & Honest Size Totals

**Tile Pyramid Geometry:** Fixed **`256 × 256` active texels + `2`-texel bilinear overlap gutter = `260 × 260` texels per tile** (`65 × 65` `4×4` BCn blocks — exact multiple of 4 for `BC5`/`BC7`/`UASTC` alignment and valid WebGL2 `TEXTURE_2D` storage!). Power-of-two mip levels (`4096²`: `L0 = 16×16` (`256` tiles) down to `L4 = 1×1`; overhead $\approx 1.333\times$).

**WebGL2 / macOS WKWebView Capability & Decoder Contract (non-blocking notes):**
- **`R16_UNORM` height:** Requires `EXT_texture_norm16`; when absent in WebGL2, viewer decodes tile `height` into `RG8_UNORM` 16-bit packed integers and reconstructs `(r * 255 + g / 256) / 255` in the vertex shader (zero visual stairstepping).
- **`BC5`/`BC7` (`EXT_texture_compression_rgtc`/`bptc` unverified in macOS WKWebView):** Ship Basis/UASTC inside KTX2 for compressed color/lighting/SDF layers and transcode at tile decode time to the best hardware format supported by the active WebGL2 context (`BPTC`/`RGTC` $\to$ `ASTC` $\to$ `ETC2` $\to$ uncompressed `RGBA8` fallback).
- **Bundle & `NOTICE` accounting:** Any Basis/KTX2/zstd JS/WASM decoder package added for WebView tile decoding (`WV2-6`) counts against `npm run check:bundle` and must be registered in `NOTICE` / licensing allowlists.

| # | Layer Name | Resolution (`4096²` / `8192²`) | Channels & Encoding | Size (`4096²` / `8192²`) | Committed vs. Build Asset & Role |
|---|---|---|---|---|---|
| 1 | `height` | `4096²` / `8192²` | `R16_UNORM` (`[-1500,+5700] m`, `0.110 m` step) + `f32` border (WebGL2 `EXT_texture_norm16` or `RG8` fallback); zstd tiles | `~18–26 MiB` / `~70–105 MiB` | Build-time tile pack (`CDLOD`/clipmap displacement) |
| 2 | `normal_xy` | `4096²` / `8192²` | 2-ch signed $(n_x, n_z)$ KTX2 UASTC -> `BC5`/native fallback | `~14 MiB` / `~55 MiB` | Build-time tile pack (ravine/bank shading) |
| 3 | `terrain_lighting` | `4096²` / `8192²` + `512²` probe | `RGBA8` `(ao, svf, bounce_luma, unused)` + 4-azimuth directional **horizon elevation angles** `(θ_E, θ_W, θ_N, θ_S)` + `512²` sky/bounce probe | `~18–24 MiB` / `~68–90 MiB` | Build-time tile pack (`§3b` shadowless Low lighting + **syncs with `R3` dynamic sun angle**) |
| 4 | `hydro_flow` | `4096²` / `8192²` | `RG`: $(\hat{u}_x, \hat{u}_z)$, `B`: $\log_2(1+Q)$, `A`: `river_amt` + waterfall wetness | `~9–14 MiB` / `~35–55 MiB` | Build-time tile pack (`§3b` river flow vectors) |
| 5 | `water_surface` | `4096²` / `8192²` | `R16`: `water_h`, `G8`: `water_kind` (`0..6`), zstd sparse-elided | `~6–10 MiB` / `~24–40 MiB` | Build-time tile pack (sea, lakes, sloped rivers) |
| 6 | `shore_sdf` | `4096²` / `8192²` | Exact Meijster `[25]` EDT `[-500, +500] m` + coastal wave exposure | `~10–14 MiB` / `~40–55 MiB` | Build-time tile pack (`§3b` wave foam & damp sand) |
| 7 | `biome_ids` & `weights` | `4096²` (`4k` hybrid at `8k`) | `4× u8` canonical biome IDs + `4× u8` weights (`sum = 255`) | `~12–20 MiB` / `~45–75 MiB` (`14` hybrid) | Build-time tile pack (`<= 4` terrain layers/px) |
| 8 | `lithology` | `4096²` (`4k` hybrid at `8k`) | `lithology_id`, `regolith_thickness`, `talus_frac`, `moraine/wetness` | `~10–15 MiB` / `~38–58 MiB` (`12` hybrid) | Build-time tile pack (cliff rock, talus, moraine) |
| 9 | `climate` | `2048²` / `4096²` | Normalized `temp`, `moisture/TWI`, `precip`, `snow_ice` | `~2.5 MiB` / `~9 MiB` | Build-time tile pack (valley lushness, treeline cues) |
| 10 | `macro_albedo` | `1024²` / `2048²` | Low-frequency `sRGB` patch modulation (`§3b.3`) | `~1.5 MiB` / `~5 MiB` | Build-time tile pack |
| 11 | `cave_mask` & `cave_network.bin` | Sparse `4096²` + graph | Empty tiles omitted; `cave_network.bin` hand-specified LE binary (`§2.2.9`) | `~0.4 MiB` / `~0.8 MiB` | Build-time pack (decorative until Underground ADR) |
| 12 | `flora_tiles` | `256²` sector blobs | Packed `8`-byte decorative tree/shrub placements (`~130k–300k` inst.); **labeled decorative behind `§6` toggle** | `~1.2–2.2 MiB` / `~2.5–5.0 MiB` | Build-time tile pack (not simulation trees; ADR-0010 §4) |
| 13 | **`sim_256.anmw3`** | Fixed `256 × 256` | **Uncompressed raw LE** `f32[65536]` (`elev`, `moist`, `temp`, `flow`, `slope`) + `u8[65536]` (`biome_22`, `water_kind`) + `waterfalls[]` + `cave_entrances[]` | **`~1.44 MiB`** raw | **COMMITTED IN GIT (`~0` C deps in `reader`)** |
| 14 | **`index.json`** | Manifest (`~3.4k–4.4k` @ `4k²`; `~14k–17k` @ `8k²` entries) | Sorted-key deterministic JSON indexing `sim_world_identity`, release asset pack hashes, & per-tile SHA-256 | **`~0.45–0.65 MiB`** (`4k²`) / **`~1.9–2.5 MiB`** (`8k²`) | **COMMITTED IN GIT** |
| — | **TOTALS** | — | Committed in Git (`sim_256.anmw3` + `index.json`): **`~2.0 MiB` (`4096²`) / `~3.4–4.0 MiB` (`8192²`)** | Streaming tile pack: **`~103–145 MiB`** (`4k²`) / **`~378–535 MiB`** (`8k²`) | Resident Iris Xe Low VRAM window: **`~35–55 MiB`** |

### 3.3 `ANMW v3` Container, Dual Identities (`LD-2`), Storage/CSP (`LD-3`), & `WorldIdentity`

#### 3.3.1 Binary Header & Circular-Hash Fix (`sim_256.anmw3`, Fix 4 & `LD-2`)
`sim_256.anmw3` carries **NO `world_identity` or bundle digest inside itself** (eliminating circularity). Fixed `44`-byte little-endian header:
- `0..4`: `b"ANMW"` magic (`world_artifact.rs:204`)
- `4..8`: `version = 3u32 LE`
- `8..12`: `width = 256u32 LE`
- `12..16`: `height = 256u32 LE`
- `16..20`: `sea_level_norm_bits` (`f32::to_bits(1500.0_f32 / 7200.0_f32)` `= 0.20833333` LE for `[-1500 m, +5700 m]`)
- `20..24`: **`sim_seed_u32 = (seed_u64 ^ (seed_u64 >> 32)) as u32` LE** — exact same byte offset (`20..24`, `world_artifact.rs:195`) as ANMW v2 `peek_seed`, letting `AnmwV3Sim::peek_seed(bytes)` resolve `SimRng` seed in $O(1)$ before building ECS!
- `24..32`: `seed_u64 LE` (full 64-bit worldgen seed provenance)
- `32..36`: `generator_version u32 LE`
- `36..40`: `world_scale_bits` (`f32::to_bits(200.0)` LE)
- `40..44`: `flags u32 LE` (`0` for standard uncompressed v3 sim bundle)
- Followed directly by contiguous raw LE arrays (`65536 × f32` × 5 + `65536 × u8` × 2) + trailing length-prefixed `waterfalls[]` & `cave_entrances[]` summary tables (`~1.44 MiB` total).

#### 3.3.2 Dual-Identity Contract (`LD-2`) & Build-Time Bundled Tile Packs (`LD-3`, Fix 5, Note 4)
- **Two distinct identities (`LD-2`):**
  1. **`sim_world_identity` = `sha256(sim_256.anmw3 raw bytes)`:** Pinned by simulation manifests, `SavedSimulationState`, headless opt-in tests, and ADR-0010 preflight. Re-baking viewer lighting, KTX2 textures, or decorative flora alters **only** `bundle_identity` and **never changes `sim_world_identity`**.
  2. **`bundle_identity` = `sha256(committed index.json raw bytes)`:** Verifiers hash the exact committed `index.json` file bytes on disk (**no JSON re-canonicalisation on read**; `anima-bake pack` emits deterministic sorted-key JSON once at bake time). `index.json` records `sim_world_identity`, release-asset archive URL/tag + archive SHA-256, and every tile's `(layer, mip, tx, ty, byte_len, sha256)` so a clean checkout verifies `sim_256.anmw3`, `index.json`, and every fetched tile pack without ambiguity. **Crucially, the release tile archive does NOT contain `index.json`** (`Note 4`, preventing a hash cycle with `bundle_identity`).
- **Where the bytes live (`LD-3`, preserving strict CSP):**
  - `artifacts/world-v2/sim_256.anmw3` and `artifacts/world-v2/index.json` are **committed in Git** (`~2.0 MiB` at `4096²`).
  - The `~103–145 MiB` viewer tile pack archive is published as a **GitHub Release asset** (**publishing a public release requires explicit owner approval in `WV2-5`**) pinned by `bundle_identity` and archive SHA-256 in committed `index.json`. A build/CI/dev prefetch script (`scripts/fetch_world_v2_tiles.mjs`, supporting `ANIMA_WORLD_V2_TILES_DIR` for offline/CI caches) downloads the release asset **strictly at build time** and verifies `sha256(archive)` and every tile SHA-256 against `index.json`. **Tauri build safety (`r4 Note 5`):** because `src-tauri/build.rs:33` calls `tauri_build::build()`, which verifies every path in `tauri.conf.json` `bundle.resources` at `cargo build` time, **static `bundle.resources` in `WV2-7` lists ONLY Git-committed files (`../artifacts/world-v2/sim_256.anmw3` and `../artifacts/world-v2/index.json`)**, so CI `cargo test --features desktop` never fails before tiles are prefetched; viewer tiles are staged solely via `build.beforeBundleCommand` in `WV2-6` (adding `tauri.conf.json` to `WV2-6` after `WV2-7`). **Zero network fetches occur at runtime** (`tauri.conf.json` CSP stays unchanged; any Tauri asset-protocol scope or IPC loader command updates `docs/reference/IPC_CONTRACT.md` in the same commit).
- **Proposed ADR-0010 Amendment Paragraph (`LD-3`; for lead to land separately — `ADR-0010` not edited here):**
  > *Proposed Addendum to ADR-0010 (Dual-Identity & Build-Time Tile-Pack Refinement, 2026-11-20):* For multi-resolution `ANMW v3` worlds (`>= 4096²`), `ADR-0010`'s committed world truth splits cleanly into two SHA-256 identities: (1) `sim_world_identity = sha256(sim_256.anmw3)` pins the committed `256²` simulation terrain (`TerrainMap`, walkability, hydrology mask) stored in Git alongside `index.json`, so ecological experiments and saves remain comparable across visual-only viewer re-bakes; (2) `bundle_identity = sha256(index.json)` pins `index.json` (recording `sim_world_identity`) plus every render tile's SHA-256. While `sim_256.anmw3` and `index.json` live directly in Git, multi-hundred-MiB KTX2/float render tile packs may be hosted as GitHub Release assets pinned by SHA-256 inside committed `index.json`, fetched and SHA-256-verified **strictly at build time** (never at runtime, preserving offline Tauri bundles and strict CSP).

#### 3.3.3 `WorldIdentity` Struct Update & `peek_seed` Specification (Fix 6, Anchor 4)
- **Not "non-breaking":** At `1ef4f0d3`, `WorldIdentity` (`world_artifact.rs:89-95`) derives `Clone, Copy, PartialEq, Eq, Default` across 5 `u32` fields (`seed`, `generator_version`, `width`, `height`, `checksum`), appears across **`46` files in `src-tauri/`** (including struct literals such as `examples/run_e3_experiment.rs:870`), and is `.copied()` at `simulation_loop.rs:1065-1066`.
- **Clean `Copy`-preserving design (`Fix 6`):** Extend `WorldIdentity` with:
  ```rust
  #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::core::world_artifact::opt_sha256_hex")]
  pub sim_world_sha256: Option<[u8; 32]>, // SHA-256 of sim_256.anmw3 bytes; None for legacy v1/v2 saves
  ```
  - Keeping `[u8; 32]` preserves `Copy`, avoids heap allocation inside tick loops, keeps `.copied()` at `simulation_loop.rs:1066` valid, and maintains backward-compatible `serde` deserialization of legacy `SavedSimulationState` snapshots (`world_artifact.rs:686-702`).
  - `WV2-3` updates the **`11` `WorldIdentity { ... }` struct-literal sites across `10` `src-tauri/` files** (`world_artifact.rs:108,308`, `experiment/manifest.rs:36`, `experiment/tests.rs:22`, `examples/run_e3_experiment.rs:870`, and `6` integration tests: `adversarial_branch_tree_parity`, `adversarial_challenger_m2_r2_stress`, `adversarial_sweep_stress`, `causal_branch_tree`, `deterministic_replay`, `parameter_sweep`; the remaining `36` files mentioning `WorldIdentity` compile unchanged).
- **`peek_seed` & loud pre-switchover refusal (`Fix 6`, Note 3):** `AnmwV3Sim::peek_seed(bytes)` reads header bytes `20..24` (`sim_seed_u32`) after validating `b"ANMW"` and `version == 3`. Prior to `WV2-7`, `WorldArtifact::peek_seed` / `from_bytes` (`world_artifact.rs:181-213`) rejects `version == 3` with an explicit version-mismatch error, **and both `load_shared_world_artifact` (`ecs.rs:16-34`) and `world_seed_from_disk()` (`world_artifact.rs:476-481`) promote v3 detection to an explicit loud failure** rather than silently falling back to generated noise or default seed; starting in `WV2-7`, the desktop loader reads bundled `sim_256.anmw3` cleanly via `AnmwV3Sim::peek_seed` + `from_bytes`.

### 3.4 Preserving `TerrainMap`, Normalized Elevation & `LAND1` Walkability (`Anchor 2`, Notes)

Verified directly at `src-tauri/src/core/terrain.rs:112-139`, `terrain_navigation.rs:28-100`, `ecology.rs:63-87,362`, `core/energy_ledger.rs`, and `world_artifact.rs:341-430`:
1. **Strahler-order block river projection (`4096² -> 256²`):** Replace the naive `"≥ 25% river cells in 16×16 block"` rule (which fails for `~6–18 m` channels inside `160 m` blocks and breaks connectivity) with:
   - `elevation` (normalized `[0, 1]` over `[-1500 m, +5700 m]` with `sea_level_norm = 0.20833333`, matching `TerrainMap::get_elevation_at_pos` & `sim_rules.rs:56-58,73` `y = elevation * WORLD_MAX_Y`), `moisture`, `temperature`, `slope`: **`16×16` arithmetic mean computed from the raw `f32` `JOB-W1` buffers** (NOT `R16_UNORM` quantized tiles whose `1.5e-5` step exceeds `GS-1`'s `1e-5` parity tolerance!).
   - **`biomes`:** Assign `BiomeType::River (3)` whenever a `16×16` block is **crossed by a channel of Strahler order $\omega \ge 4$ (or active channel width $w \ge 8\text{ m}$)** and oceanic fraction $< 50\%$, followed by **8-connected path-bridging** along the upstream/downstream Strahler $\ge 4$ centerline so major rivers never break into disconnected cells on `256²` (`GS-1`).
   - Deep oceanic water (`water_kind == 1` and `elevation_m < -220 m`) maps to `BiomeType::DeepOcean (0)`; shallow shelf water to `BiomeType::Ocean (1)`; inland lakes (`water_kind ∈ {2, 3}`) map to `BiomeType::Ocean (1)` (`world_artifact.rs:420`), keeping `is_biome_walkable == false` and `is_water_biome == true`. Non-water cells pick the weighted modal canonical biome mapped via `map_biome_frontend_to_backend` (`world_artifact.rs:404-430`).
2. **`LAND1` & `ResourceField` preservation:** `is_biome_walkable` (`DeepOcean`/`Ocean` = `false`, `River` & land = `true`), `is_water_biome` (`DeepOcean`/`Ocean`/`River` = `true`), and `ResourceField::from_biomes` (`ecology.rs:362`, reading `biome_npp` at `ecology.rs:63-87` and conserving closed EU in `core/energy_ledger.rs`) remain completely untouched.
3. **Desktop genesis lake-candidate threshold under v3 normalization (`r4 Note 4`):** When `LAND1` is off, `src-tauri/src/core/simulation_loop.rs:1735` selects lake candidates via `(biome == 0 || biome == 1 || biome == 3) && elevation < 0.4`. Under v3's `[-1,500 m, +5,700 m]` normalization (`sea_level_norm = 0.20833333`), `0.4` corresponds to `+1,380 m` (`0.4 × 7200 - 1500`). **`WV2-7` explicitly audits & updates/parameterizes this threshold relative to `sea_level_norm`** when switching desktop genesis onto `sim_256.anmw3` (preserving coastal/lowland lake placement while accounting for high-altitude páramo lakes above `1,380 m`).

---

## 4. `G-SHAPE` Metrics, Real-Terrain Grounding & `G-REF` Traceability

### 4.1 `G-SHAPE` Quantitative Geomorphology Metrics (`§4.1` Fixed Sources & Caveats)

All metrics execute deterministically inside `anima_worldgen::verify::g_shape` (`anima-worldgen verify`) at regional `40.96 km` scale (`10 m/cell` @ `4096²`):

| Metric | Exact Computation Algorithm | Verified Primary Sources `[Ref]` | Regional `40.96 km` Target Band | Failure Mode Caught |
|---|---|---|---|---|
| **1. Strahler Order & Horton Ratios ($R_b, R_L$)** | Extract channel mask ($A S^2 \ge \tau_{\text{chan}}$ & $Q \ge Q_{\text{min}}$) on Priority-Flood DAG; assign Strahler order $\omega$ bottom-up; fit $\log N_\omega$ and $\log \bar{L}_\omega$ over $\omega \in [1, \omega_{\max}-1]$. | Horton (1945) `[17]`; Strahler (1952, 1957) `[38]`. **Caveat — Kirchner (1993) `[21]`:** Horton ratios $R_b \in [3, 5]$, $R_L \in [1.5, 3.0]$ are statistically inevitable for almost any random planar tree and **cannot validate realism alone**; they serve strictly as a **necessary degeneracy floor** paired with metrics 2–7. | $\omega_{\max} \ge 4$; $R_b \in [3.0, 5.0]$; $R_L \in [1.5, 3.0]$; strict DAG (`0` inland non-braided cycles outside deltas/tundra) | Flat river streaks, disconnected noise rills, grid parallel hatching |
| **2. Tributary Junction Angles ($\theta_j$)** | At every confluence where two D8 channels $\omega \ge 2$ join, measure tangent vectors $150\text{–}400\text{ m}$ upstream; compute interior junction angle $\theta_j$. | Howard (1971) `[18]`; **Seybold, Rothman & Kirchner (2017)** `[35]`, *Geophys. Res. Lett.* 44(5), 2272–2280 (`~45°` arid to `~72°` humid mean). | Median $\theta_j \in [40^\circ, 78^\circ]$; $< 10\%$ obtuse ($> 100^\circ$) junctions; rain-shadow plateau $\bar{\theta}_j <$ windward humid $\bar{\theta}_j$ | Orthogonal `90°` or comb-parallel joins (`AER-01`, `AER-02`) |
| **3. Drainage Density ($D_d$)** | Total channel length $\sum L_{\text{chan}}\text{ (km)}$ divided by non-glacial/non-lake dry land $A_{\text{land}}\text{ (km}^2\text{)}$ + per-`2 km × 2 km` tile $D_d$ distribution on uniform plateaus. | Horton (1945) `[17]`; **Tucker, Catani, Rinaldo & Bras (2001)** `[39]`, *Geomorphology* 36(3–4), 187–202. | Global $D_d \in [0.8, 3.5]\text{ km/km}^2$; dissected mountain/windward $D_d \ge 1.5\times$ alluvial/rain-shadow plains; uniform-caprock tile CV $\le 0.45$ | Pancake blobs (`Dd < 0.3`) or noisy Swiss-cheese slopes (`Dd > 6`) |
| **4. Hack's Law ($L = C A^h$)** | Sample $\ge 500$ channel nodes across orders $2..\omega_{\max}$; compute upstream drainage area $A\text{ (km}^2\text{)}$ (`<= 300 km²`) & longest upstream path $L\text{ (km)}$ (`~22–35 km` w/ sinuosity `1.25–1.45`); fit OLS $\ln L = \ln C + h \ln A$. | **Hack (1957)** `[13]` ($L = 1.4 A^{0.6}$ in mi/mi²); **Gray (1961)** `[12]` (scaling exponent $h = 0.568$; prefactor omitted); **Rigon et al. (1996)** `[32]`. | $h \in [0.52, 0.62]$; $C_{\text{km}} \in [1.0, 2.0]$ (`<= 300 km²` basins); Pearson $R^2 \ge 0.85$ | Straight grid-axis channels ($h \approx 0.50$ trivial or $> 0.70$) |
| **5. Valley Cross-Section ($b, V_f$) & Flint Concavity ($\theta$)** | Sample `40` upper V-valleys ($\omega \in \{2,3\}$) and `40` downstream alluvial/glacial valleys ($\omega \ge 4$): (a) power cross-section $z(r) = z_0 + k|r|^b$; (b) Bull–McFadden $V_f$; (c) Flint slope-area $\log S = \log k_s - \theta\log A$. | Bull & McFadden (1977) `[6]`; Harbor (1992) `[14]`; Flint (1974) `[11]`; Whipple & Tucker (1999) `[42]`. | **Upper V-valleys:** $b_{\text{up}} \in [0.85, 1.35]$, $V_{f,\text{up}} \in [0.05, 0.80]$. **Downstream valleys:** $b_{\text{down}} \in [1.60, 3.50]$, $V_{f,\text{down}} \ge 1.50$, ratio $V_{f,\text{down}}/V_{f,\text{up}} \ge 2.5$. **Concavity:** $\theta \in [0.35, 0.65]$. | Inverted valleys or uniform V-notches down to coast |
| **6. Endorheic Basins & Spillways** | Audit final water/DEM hierarchy: verify `0` unmodeled micro-pits outside declared lakes, `0` severed humid spillways ($P/\text{PET} \ge 0.5$), and bound leeward rain-shadow endorheic land area. | Barnes et al. (2014) `[1]`; Barnes et al. (2020) `[2]`; **Barnes et al. (2021)** `[3]`; **Wang et al. (2018)** `[40]`, *Nat. Geosci.* 11(12), 926–932 (`~20%` of Earth land area endorheic, predominantly arid); Lehner & Grill (2013) `[22]` (HydroSHEDS framework). | `0` unmodeled micro-pits; `0` severed humid spillways; rain-shadow arid endorheic fraction `<= 10%` of land (`0%` in humid zones) | Rivers terminating abruptly in random noise craters |
| **7. Hypsometric Integral ($HI$) & Spike Check** | Dimensionless $HI = (\bar{h} - h_{\min})/(h_{\max} - h_{\min})$ across `25` catchments & landmass; check 99.9th-percentile hillslope angle away from mapped cliffs/waterfalls. | Strahler (1952) `[38]`, *GSA Bull.* 63, 1117–1142. | $HI_{\text{global}} \in [0.22, 0.55]$; median catchment $HI \in [0.30, 0.60]$; `0` non-cliff needle spikes ($>75^\circ$ local Laplacian reversal) | Perlin needle cones (`Map v2 §2.2`) or flat un-eroded slabs |

### 4.2 Measuring Category 12 Real Reference Terrain (`--dem-input`)

Category 12 photos (`AER-01`..`AER-08` in `reference-board.json`) are optical satellite/aerial imagery rather than DEMs. `anima-worldgen verify --dem-input <file.f32|geotiff>` therefore runs the exact same Rust `G-SHAPE` code directly on public-domain Earth `10–30 m` DEM patches (`Hadramawt AER-01`, `Kunlun AER-03`, `Cotahuasi AER-06`, `Red River AER-07`, pending explicit owner approval before downloading external DEMs), alongside 2-D planform network checks (`R_b`, junction angle, scroll periodicity, box-counting shoreline fractal dimension $D_{\text{shore}} \in [1.15, 1.45]$).

### 4.3 Full `G-REF` Traceability (`Category 12` + All `34` `P1` Reference Photos)

| G-REF ID(s) | Environment | Core `P1` Visual Criteria (`reference-board.json`) | World v2 Mechanism & Checkable Target (`LD-1` Regional Scale) |
|---|---|---|---|
| **`AER-01`, `AER-02`** | Cat 12: Wadi Hadramawt & Empty Quarter | Acute tributary joins; Strahler growth; strict tree topology; flat interfluve plateau | S1 caprock plateau + S5 Hybrid MFD→D8 routing + S6 pit-free DAG -> `G-SHAPE` (`R_b ∈ [3,5]`, $\theta_j \in [40^\circ,78^\circ]$, `0` cycles) |
| **`AER-03`** | Cat 12: Kunlun–Altun Alluvial Fan | Radiating smooth fan cone below canyon apex; dissected mountain behind; one active wet braid | S4 radial MFD fan-packet deposition below sharp canyon-front slope drop -> `alluvial_mask` + active vs dry braid wetness |
| **`AER-04`, `AER-05`, `WET-04`** | Cat 12/7: Yellow River Delta, Guinea-Bissau, Brahmani | Distributaries fork downstream; protruding lobe; brown sediment plume; tidal roots & beach ridges | S4 `DeltaRCM` `[24]` parcel progradation + `water_kind=6` `water_turbidity` plume + `shore_sdf` beach harmonics |
| **`AER-06`** | Cat 12: Cotahuasi Canyon | Deep dendritic canyons & knife-edge ridges; intact plateau rims; snow only on highest volcanic peaks | S1 caprock + volcanic cone + S2 Fastscape-SD deep incision + S7 lapse-rate snowline |
| **`AER-07`, `RIV-01`** | Cat 12/10: Red River & Innoko River | Meandering single-thread trunk; inner point bars vs outer cut banks; oxbow lakes & scroll bars | S4 Howard & Knutson (1984) `[19]` meander evolution + oxbow lake basins + concentric scroll relief |
| **`AER-08`** | Cat 12: Anastomosing Braided Ponds (`Arctic NWR` benchmark -> Páramo/Cirque Braids) | Braided shallow upland/wetland channels splitting & rejoining around thaw/glacial ponds; intricate shore | S4 flat-slope high-altitude basin braiding + micro-ponds (`>= 10` @ `> 3,300 m`) + fractal shore (`D_shore ∈ [1.15, 1.45]`) |
| **`DES-01`, `DES-02`, `DES-03`** | Cat 1: Leeward Semi-Arid Scrub & Gorge (`Desert` / `Badlands` proxies; `DES-01`/`02` dunes stylised visual benchmark; `DES-03` Wadi Rum sandstone gorge) | Continuous wind-aligned crests (`~12°` stoss / `~32°` slip face, stylised sandy pocket benchmark); smooth xerophytic-scrub transition; cliff-to-sand break | S7 leeward rain shadow + S8 Werner (1995) `[41]` dune morphology (stylised pocket) + S3 cliff talus foot break + continuous `biome_weights` decay (`Desert`/`Badlands` proxies) |
| **`RNF-01`** | Windward Lowland Rainforest (Tropical Coastal Foothills; visual canopy benchmark `RNF-01`) | Continuous dense humid broadleaf canopy; winding coastal rivers widening downstream | S7 wet tropical onshore windward slopes + S5 Leopold & Maddock `[23]` $w \propto Q_{\text{ma}}^{0.50}$ (`~6–18 m` trunk rivers widening to `~20–45 m` delta mouths) |
| **`TMP-01`** | Montane Broadleaf & Cloud Forest on Rounded Ridges (visual benchmark `TMP-01`) | Continuous montane canopy draped across smooth rounded ridges; sunlit/shaded aspect variation | S3 Roering et al. (1999) `[34]` nonlinear diffusion rounding ridges + `terrain_lighting` aspect/horizon shading |
| **`TAI-01`, `TAI-02`** | High-Montane Conifer / Elfin Forest Belt (`Taiga` proxy; visual benchmark `TAI-01`/`TAI-02`) | Patchy dark-conifer vs deciduous ecotone transition; gravel/sand bars; riparian tree bands, pools | S8 altitudinal treeline transition (`2,600–3,300 m`) + S4 point bars + S7 `TWI` riparian density + decorative `flora_tiles` |
| **`TUN-01`, `TUN-02`, `TUN-03`** | Páramo & Superpáramo Uplands (`Tundra` proxy; visual benchmark `TUN-01..03`) | Treeless grass/cushion uplands (`3,300–4,800 m`); windswept ridge mats thinning upward into frost scree & bare rock | S1 massif shoulders + S3 smooth diffusion + S7 freezing high-altitude lapse + S8 continuous alpine-to-rock ecotone blending |
| **`WET-02`** | Coastal / Alluvial Marsh & Tree Islands (visual benchmark `WET-02`) | Raised vegetated tree islands in flat coastal/floodplain sloughs | S7 high `TWI` lowland basin + low-relief `0.5–1.5 m` micro-hummocks driving local swamp/cypress island weights |
| **`MTN-01`, `MTN-02`, `MTN-03`, `MTN-04`, `MTN-05`, `MTN-06`** | Cat 8: High Mountains, Treeline, Snowline, Glaciers | Fingered ragged treeline (`MTN-01`); irregular altitude snowline (`MTN-02`); valley glaciers with dark moraine (`MTN-03`), glacierized peaks above forested valleys + cirques (`MTN-04`), bare faces (`MTN-06`), stratocone (`MTN-05`), talus | S3 talus aprons & bare rock $>42^\circ$ + S7 aspect-warped lapse snowline + S8 Oerlemans `[29]`/Cuffey `[9]` cirque glaciers with `moraine_debris` + S1 cone |
| **`CST-03`** | Cat 9: Lost Coast | Mountains rising straight out of surf without wide beach; narrow dark sand strip; breaker lines | S1 steep convergent ocean margin + `shore_sdf` high wave-exposure mask |
| **`RIV-02`, `RIV-03`, `RIV-04`** | Cat 10: Falls & Valley | Montmorency cliff drop -> plunge pool -> wider reach (`RIV-02`); **dry-plateau basalt gorge (visual benchmark `RIV-03`)**; lush verdant valley floor + treeless upper slopes (`RIV-04`) | S6 knickpoint waterfalls + plunge pools + wet dark rock + S1 caprock gorge + S7 `TWI` greener valley bottom & S8 altitudinal zoning |
| **`LAK-01`, `LAK-02`, `LAK-03`** | Cat 11: Crater, Alpine & Trough Lakes | Crater Lake steep caldera walls & shallow shelf (`LAK-01`); contour alpine basin above treeline (`LAK-02`); narrow steep-walled rift trough lake (`LAK-03`) | S1 caldera & fault-trough relief + S6 `FillSpillMerge` `[3]` exact shoreline contours aligned with elevation zones |

*(Remaining `19` `R1`–`R4` G-REF photos are renderer/lighting/atmosphere targets consumed by Phase P3 on Windows 1 from this container's baked inputs.)*

---

## 5. `G-COMP v2` Extension & `G-SIM` Simulation Gates

### 5.1 `G-COMP v2` — Tropical Coastal Massif Setting & Biome Partition (`LD-5`, r3 Fixes 4 & 5)

**Replacement of the legacy `18`-biome `REGION_BIOMES` floor (`worldComposition.ts:53-56,91-97`):** The legacy check enforces a minimum patch size across `18` planetary biomes simultaneously (looping `REGION_BIOMES` over `Jungle`, `Mangrove`, `Savanna`, `Taiga`, `Tundra`, `Bog`, `Glacier`, `Desert`, `Badlands`, etc.). A single fixed-latitude (`~11°N`) `40.96 km` world cannot physically host mid-latitude steppe or Mediterranean chaparral alongside tropical coastal rainforest, so **World v2 replaces `REGION_BIOMES`'s global floor with `C1`'s setting-derived altitudinal & rain-shadow floor** implemented once in Rust (`anima-worldgen verify`, emitting `artifacts/world-v2/gates.json` consumed read-only by TS tests — `0` duplicate TS validator logic).

**Complete Partition of All `22` Canonical `Biome` Variants (`worldGen.ts:19-43` for `LD-5` Sierra Nevada de Santa Marta setting `[15, 45, 46, 47, 48]`):**
- **Water & coastal/river strips (`4`, gated by `oceanFraction ∈ [0.40, 0.50]`, `C3`, `C4`, `C5` — not `C1` area floor):** `Ocean (0)`, `Beach (1)`, `River (13)`, `Lake (14)`.
- **Present & Gated by `C1` Area Floor (`16` terrestrial biomes, $\ge 2{,}500$ contiguous cells @ `4096²` $\approx 0.25\text{ km}^2$ each):**
  - *Windward humid coastal & lowlands (`0–1,000 m`):* `Mangrove (15)`, `Swamp (10)`, `Jungle (7)`, `Grassland (4)`.
  - *Leeward rain-shadow basin & foothills (`0–1,600 m`, arid/semi-arid):* `Desert (2)` (**declared proxy** for leeward xerophytic thorn scrub / cardonal & dry sandy pocket; `DES-01`/`DES-02` dune fields are a stylised visual benchmark, not claimed natively inside a `41 km` SNSM window, `r4 Note 3`), `Badlands (19)` (**declared proxy** for eroded semi-arid rain-shadow slopes & dry canyons), `Savanna (3)`, `Shrubland (5)`.
  - *Submontane & Montane cloud/elfin forest (design belt limits `1,000–3,300 m` and `2,600–3,300 m` upper-montane conifer/elfin parameterization informed by SNSM Massenerhebung compression `[45]`, which documents cloud-forest & páramo floras above `1,700 m` at lower limits than larger cordilleras, `r4 Note 1`):* `Forest (6)` (tropical montane/cloud forest), `Taiga (8)` (declared ecological proxy for upper-montane conifer/elfin belt).
  - *Páramo & Sub-nival (design belt parameterization `3,300–4,800 m` informed by SNSM Massenerhebung zonation `[45, 46]`, below `ela_base_m`):* `Alpine (18)` (lower páramo/alpine meadow), `Tundra (9)` (declared proxy for high-altitude superpáramo cushion grassland `3,800–4,700 m`), `Bog (21)` (high-altitude páramo cushion-peat bog), `Rock (11)` (bare steep crag/talus `> 42°`).
  - *Nival zone (`>= ELA(x)` centered at `ela_base_m = 4,900 m ∈ [4,800, 5,100] m`, up to `5,700 m` `[46, 47, 48]`):* `Snow (12)` (perennial snowpack), `Glacier (20)` (tropical summit/cirque glaciers).
- **Explicitly Absent by Design (`2` biomes, excluded from `C1` floor and asserted `0%` area):** `Chaparral (16)` (Mediterranean winter-rain sclerophyll) and `Steppe (17)` (mid-latitude continental semi-arid steppe).

| `G-COMP v2` Check | Target Band / Formula | Purpose & Gap Closed |
|---|---|---|
| **`C1` Regional Altitudinal & Rain-Shadow Partition** | `oceanFraction ∈ [0.40, 0.50]`, `waterfalls ∈ [20, 80]`, `caves ∈ [30, 48]`, spacing `>= 0.01 * size`; each of the **`16` present `LD-5` biomes** above has a connected region `>= 2,500` cells @ `4096²`; `Chaparral (16)` + `Steppe (17)` stay at `0` cells. | Guarantees full `LD-5` altitudinal + windward/rain-shadow zonation with zero unphysical mid-latitude steppe/chaparral leakage |
| **`C2` Continuous Ecotone Band** | Land fraction with dominant weight $w_0/255 < 0.90$ lies in `[0.15, 0.55]`; raw non-cliff single-cell jumps $> 0.70$ `< 3%` of land edges. | Replaces hard binary biome walls (`Map v2 §2.1`) |
| **`C3` Multi-Basin Lake Diversity** | Lake surface `∈ [0.008, 0.045]`, `>= 5` lakes `> 0.1 km²`, including `>= 1` high-páramo glacial lake (`> 3,300 m`), `>= 1` caldera/trough lake (wall slope `> 25°` or elongation `> 3.5`), and `>= 10` small páramo/cirque ponds (`AER-08`). | Prevents degenerate zero-lake or flooded worlds |
| **`C4` Runoff-Consistent Hydraulic Progression, Fans & Deltas (`r3 Fix 4`, `r4 Fix 2`)** | **Counted river surface fraction $f_{\text{river}} = \frac{1}{N^2}\sum_{i=1}^{N^2}\text{river\_amt}_i \in [0.0005, 0.0030]$** where $\text{river\_amt}_i = \operatorname{clamp}(w_i/\Delta x, 0, 1)$ for active non-ocean/non-lake channel cells ($A S^2 \ge \tau_{\text{chan}}$ and $w_i \ge 0.5\text{ m}$; `0` elsewhere — matching Horton length-weighted mean width `~1.1 m` across $D_d \in [0.8, 3.5]\text{ km/km}^2$ on `840–1,000 km²` land plus `20–45 m` deltas $\implies$ `~0.0006–0.0026`, mid `~0.0012` of the `1,677.7 km²` map); mean channel width increases strictly with Strahler order ($\bar{w}_2 < \bar{w}_3 < \bar{w}_4$, trunk widths `~6–18 m`, delta/estuary mouths `~20–45 m` from `Q_ma` over `120–300 km²` basins); check Hack $C_{\text{km}}$ consistency with catchment area `<= 300 km²` & sinuosity; `>= 2` piedmont alluvial fans; `>= 2` coastal deltas (`AER-04`, `WET-04`). | Aligns river geometry AND `C4` area band strictly with runoff budget & `6–18 m` coastal massif channels |
| **`C5` Coastal Morphology & Sightlines** | Coastline `>= 20%` gentle sandy slope (`< 0.18`) and `>= 10%` steep rocky cliff (`>= 0.35`, `CST-03`); all canonical camera viewpoints sit unobstructed above terrain/water. | Guarantees coastal variety & clean canonical captures |

### 5.2 `G-SIM` Gates (`src-tauri/tests/world_v2_sim_gate_tests.rs`, Fix 9 & Anchors 2/3)

| `G-SIM` ID | Concrete Verification Check | Trunk Anchor | Pass Criterion |
|---|---|---|---|
| **`GS-1` Render–Sim Grid Parity** | Decode `sim_256.anmw3` and compare directly against **`16×16` block averages of raw `f32` `JOB-W1` height/fields** (NOT quantized `R16_UNORM` tiles whose `1.5e-5` step exceeds `1e-5`) and Strahler $\ge 4$ river connectivity. | `world_artifact.rs:341-430`; `terrain.rs:129-139`; `sim_rules.rs:56-58` | Max mean normalized elevation diff $\le 10^{-5}$; all `biomes[i] <= 10`; `Ocean`/`DeepOcean` water mask `100%` exact; every Strahler $\ge 4$ river stays 8-connected on `256²` |
| **`GS-2` `LAND1` Walkability & Connectivity** | Validate `is_biome_walkable` & `is_water_biome` on World v2 `256²` `TerrainMap`; 4-connected land graph check; step 1,000 ticks with 200 agents under `TerrainNavigation { enabled: true }` (note: testing with `live.founder_layout = 1 | 2` uses PR `#186`, which was absent at `1ef4f0d3`/`cc40f11f` and merged at `4794b6fc`; falls back cleanly to default lattice genesis if pinned to `<= cc40f11f`). | `terrain_navigation.rs:28-100`; `live_experiment.rs`; PR `#186` (`4794b6fc`) `founder_layout_tests.rs` | Largest connected walkable component `>= 80%` of land; `>= 500` walkable shoreline cells; `0` agents ever end a tick on `DeepOcean`/`Ocean` |
| **`GS-3` Closed-EU Ledger & Realistic `ResourceField`** | Run 1,000-tick closed ecosystem simulation on `sim_256.anmw3` using `ResourceField::from_biomes` (`ecology.rs:362`), `biome_npp` (`ecology.rs:63-87`), and closed-EU `EnergyLedger` (`core/energy_ledger.rs`). | `ecology.rs:63-87,362`; `core/energy_ledger.rs`; `ecs.rs:105-128` | Global plant capacity within `[0.4×, 2.0×]` of baseline World v1 capacity; **closed-EU residual drift `< 1e-6`**; zero `NaN`/`Inf` |
| **`GS-4` Zero-Allocation Tick Path** | Verify World v2 `TerrainMap` / `TerrainNavigation` introduces zero heap allocations inside `build_tick_schedule` hot loop. | `*_zero_alloc*` suites; Map v2 `§6` | All existing `9` zero-alloc targets + World v2 `LAND1` tick probe record `0` allocations after warm-up |
| **`GS-5` Paired-Run Isolation & `sim_world_identity` Rejection** | (1) Attempt to verify/boot with 1 tampered byte in `sim_256.anmw3` or mismatched `sim_world_identity` (refused loudly). (2) Run **paired same-machine comparisons** of `E-SEL1` and `SEL2-P` under `TerrainSource::Generated`: **clean environment without World v2 on disk vs. same binary/seed with World v2 `ANMW v3` present at default / `ANIMA_WORLD_ARTIFACT` paths** (*never* compare against a hardcoded pinned checksum). | `ecs.rs:43-51,60-75`; `simulation_loop.rs:1058-1073`; `world_source_tests.rs` | Tampered/mismatched `sim_world_identity` returns `Err`; **paired same-machine `E-SEL1`/`SEL2-P` trajectories & checksums match bit-for-bit** regardless of v3 files on disk |

---

## 6. OxideSwarm Grid Bake Jobs (`§3c` / `QD-OXS`)

### 6.1 Grid Operational Contract (`AGENT_RULES.md:26-28,98-179`)

1. **Worker Exclusivity (`QD-OXS`):** Every OxideSwarm grid job passes **`--worker desktop-3epv830`** explicitly; never omit `--worker` and never run on friends' cluster laptops.
2. **Pre-flight & Workspace Rules (`AGENT_RULES.md:132,145-147`):**
   - Check `~/OxideSwarm-trial/run.sh workers --json` (`state:connected`, `feat:workspaces`, `tool:cargo`) and `anima_bridge.py --agent-id anima-mac activity --show` so no job collides with `anima-case` on `DESKTOP-3EPV830`.
   - **Approved workspace (`AGENT_RULES.md:145-147`):** Submit inside existing registered workspace **`anima-t1`** (or **`anima-t2`**) unless/until the lead/owner explicitly registers a dedicated `anima-worldbake` workspace on `desktop-3epv830`. Respect the `20 GiB` workspace limit (`out/` staged bundles stay `< 650 MiB`).
   - **Mandatory Step 0 GPU Probe before GPU jobs (`Fix §7`):** `tool:cargo` readiness does **not** prove Windows worker session access to the RTX 5060 Ti. Before submitting `JOB-W4 --backend wgpu` or `JOB-W6`, run `JOB-W0` (`anima-bake probe-gpu`) to confirm `wgpu` enumerates the RTX 5060 Ti adapter cleanly; if headless session blocks DX12/Vulkan, `JOB-W4` stays on `--backend cpu` without stalling P1!
3. **External Assets, Release Publication & CI Policy (`JOB-W6`, `Note 4`):**
   - OxideSwarm workers (`iso:fs`) may have no outbound network access: pre-approved CC0 source assets (`ambientCG`, `Poly Haven`) are staged locally on Mac with an **explicit owner approval line in `WV2-5`** and synced into the worker via **`--sync`** rather than downloaded inside the worker.
   - Publishing the tile bundle as a public GitHub Release asset also requires an **explicit owner approval line inside `WV2-5`**, and the release tile archive **excludes `index.json`** (`LD-2`/`bundle_identity` cycle guard).
   - CPU-vs-GPU SSIM integration tests (`wv2_5_gpu_bake_tests`) require real GPU hardware and are explicitly registered in `scripts/test_target_policy.mjs` (`[[test]] required-features = ["gpu-bake"]`) for GPU-less `windows-latest` CI.

### 6.2 Grid Job Matrix (`JOB-W0..JOB-W6`)

| Job ID | Command (`--worker desktop-3epv830 --cwd src-tauri`) | Inputs Pinned | Outputs & Class (`Fix 12`) | Est. Runtime (`4k²` / `8k²`) | Sharding |
|---|---|---|---|---|---|
| **`JOB-W0`** `gpu-probe` | `cargo run --release -p anima-worldgen --features gpu-bake --bin anima-bake -- probe-gpu` | Commit SHA | Adapter JSON + exit `0` (**prereq for `W4-gpu` & `W6`**) | `< 3 s` | `1` probe task |
| **`JOB-W1`** `worldgen-core` | `cargo run --release -p anima-worldgen --features generate --bin anima-worldgen -- generate --seed <SEED> --size <N> --out <OUT>/raw` | Repo SHA + preset TOML + `--seed` | Raw `f32`/`u8` buffers + **`sim_256.anmw3`** + `cave_network.bin`. **`CPU-deterministic`** (byte-identical Mac M5 vs i5-14600KF; `sim_world_identity = sha256(sim_256.anmw3)`). | `4k²`: `~7–14 s` (i5) / `~10–18 s` (M5); `8k²`: `~32–65 s` | `1` unit per `(seed, N)`; sweep across seeds |
| **`JOB-W2`** `worldgen-verify` | `cargo run --release -p anima-worldgen --features generate --bin anima-worldgen -- verify --raw <OUT>/raw --strict-gates --report <OUT>/gates.json` | Commit SHA + `W1` hashes | `gates.json` (`G-SHAPE`, `G-COMP v2`, `G-SIM` raw checks). **`CPU-deterministic`**. | `4k²`: `~2–5 s`; `8k²`: `~10–22 s` | `1` unit per candidate seed |
| **`JOB-W3`** `bake-fields` | `cargo run --release -p anima-worldgen --features bake --bin anima-bake -- fields --raw <OUT>/raw --tile-strip <S>/<K> --out <OUT>/fields` | Commit SHA + `W1` hashes | `normal_xy`, exact Meijster `[25]` `shore_sdf`, `hydro_flow`, `flora_tiles`. **`CPU-deterministic`** (`libm` + Meijster 2-pass). | `4k²`: `~6–11 s`; `8k²`: `~25–48 s` | Pass A global `<1 s`; Pass B sharded into $K$ tile strips w/ `2`-cell halo |
| **`JOB-W4`** `bake-lighting` | **CPU:** `... anima-bake -- lighting --backend cpu --raw <OUT>/raw --tile-range <A>..<B>`<br>**GPU:** `... anima-bake -- lighting --backend wgpu --raw <OUT>/raw --samples 64` | Commit SHA + `W1` + `--samples`/`driver` | `terrain_lighting` (AO, SVF, 4-azimuth horizon angles, bounce probe). **CPU mode:** **`CPU-deterministic`** (Stewart sweep `[37]`). **GPU mode:** **GPU-pinned** (RTX 5060 Ti + driver; tol: max delta $\le 2/255$, RMSE $\le 0.4/255$, SSIM $\ge 0.999$). | CPU `4k²` `~22–42 s`, `8k²` `~95–190 s`; GPU `4k²` `~1–2.5 s`, `8k²` `~4–9 s` | Independent per `256×256` tile w/ `256`-cell horizon halo |
| **`JOB-W5`** `bake-tiles` | `cargo run --release -p anima-worldgen --features bake --bin anima-bake -- pack --in <OUT> --release-tag <TAG> --out <OUT>/world_v2` | Commit SHA + `W1..W4` hashes + `gates.json` PASS | `world_v2_<id8>/` (`sim_256.anmw3`, sorted-key **`index.json`**, release archive of KTX2/zstd tiles **excluding `index.json`**). **Zstd/JSON assembly: `CPU-deterministic`**. **C/C++ KTX2/BCn/Basis encoding (`bc7enc_rdo`/UASTC): `Pinned-run + tolerance` (`Block PSNR >= 42 dB`, `SSIM >= 0.996`)** unless cross-OS replica proves exact SHA-256 identity. | CPU `4k²`: `~8–16 s` (i5) / `~12–25 s` (M5); `8k²`: `~32–65 s` | Base `mip0` tiles compressed independently; final `reduce` writes `index.json` |
| **`JOB-W6`** `bake-assets` | `cargo run --release -p anima-worldgen --features gpu-bake --bin anima-bake -- assets --manifest artifacts/assets-v2/sources.json --out artifacts/assets-v2/baked` *(pre-approved CC0 inputs synced via `--sync`)* | Commit SHA + pinned CC0 SHA-256s | `species_*.meshopt.lod0..3.bin` (**`CPU-deterministic`**) + octahedral impostor KTX2 (**`GPU-pinned`**, tol: alpha $\le 1/255$, albedo RMSE $\le 1.5/255$, SSIM $\ge 0.995$). | `~1–3 s` / species (`~25–60 s` / 20 species) | `1` species/pack per work unit |

---

## 7. Migration Plan, Desktop Switchover (`LD-4`), ADR-0010 `W1`/`W2` Disposition, & Iris Xe Gate

### 7.1 Zero Silent Breakage, `ADR-0010` Disposition & Desktop Switchover (`LD-4`, Fix 8)

1. **Relationship to `2026-11-19-feature-real-world-and-bodies.md` (`W1`/`W2`):**
   - `WV2-1a..WV2-5` **supersede card `W1` (`ANMW v3`):** Rust `anima-worldgen` replaces TypeScript as the v3 writer and offline baker; `reader` in `anima-engine` fulfills the Rust reader half.
   - `WV2-3` + `WV2-7` **absorb `W2`'s artifact-loading & `sim_world_identity` preflight path** (`W2-lite` `#159` stays intact); ecological replacement of runtime-generated trees/lakes by terrain-pinned entities remains a separate post-P1 card, so baked `flora_tiles` and terrain lakes remain explicitly decorative behind the `§6` UI toggle (`ADR-0010 Decision 4`).
2. **No silent fallback (`Fix 8`, Note 3):** Until **`WV2-7`** lands, if a v3 artifact exists at `ANIMA_WORLD_ARTIFACT` or `$TMPDIR/anima_world_artifact.anmw`, **both `load_shared_world_artifact` (`ecs.rs:16-34`) and `world_seed_from_disk()` (`world_artifact.rs:476-481`) fail loudly** rather than swallowing `WorldArtifact::peek_seed`/`from_bytes` version errors and silently falling back to generated noise or default seed!
3. **Coordinated Desktop Switchover & Packaged Tauri Resource Path (`LD-4`, r3 Fix 8):**
   - **`WV2-7` merges BEFORE `WV2-6` (or inside the same PR):** switching `TerrainSource::AppArtifactOrGenerated` (`ecs.rs:76-90`) onto the bundled `sim_256.anmw3` first guarantees trunk never draws World v2 over the old v1/noise simulation. (If `WV2-6` is ever previewed on a dev branch before `WV2-7` lands, the UI renders an unmistakable `"World v2 Preview — not the simulated world"` banner until `WV2-7` merges.)
   - **How the packaged desktop app finds `sim_256.anmw3` (`Fix 8`, `r4 Note 5`):** `tauri.conf.json` static `bundle.resources` declares **only Git-committed `artifacts/world-v2/sim_256.anmw3` and `artifacts/world-v2/index.json`** (`src-tauri/build.rs:33` calls `tauri_build::build()`, which checks `bundle.resources` at every `cargo build`, so CI `cargo test --features desktop` never breaks on un-fetched tiles; `WV2-6` stages the fetched release tile pack solely via `build.beforeBundleCommand`). Because the background simulation thread has no Tauri `AppHandle` (`world_artifact.rs:458-466`, `simulation_loop.rs:431,1056`), the **Tauri main thread resolves `app.path().resource_dir()?.join("world-v2/sim_256.anmw3")` at startup** (with dev-mode fallback to `artifacts/world-v2/sim_256.anmw3` unless overridden by `ANIMA_WORLD_ARTIFACT`) and **hands that resolved `PathBuf` into `SimulationEngine` / `world_seed_from_disk(Some(&path))` & `init_world`** before spawning evolution/sim threads.
4. **Headless experiments untouched (`F051`/`F051b`):** `LiveExperimentAdapter` keeps `TerrainSource::Generated` (`ecs.rs:64-75`) for `E2`, `E-SEL1`, `E3`, and `SEL2-P` (optional opt-in terrain-source manifest key in `WV2-3` defaults to `Generated`).

### 7.2 Three.js Viewer Integration & Manifest Hygiene (`Fixes 11 & 13`, Notes 5 & 7)

1. **Build-time bundled v3 loading (`src/components/Landscape/utils/worldArtifactV3.ts`, `worldCache.ts`):** Decode build-time-fetched bundled v3 tiles matching `QualityTier` (`1024²` low, `2048²` medium, `4096²` high zoom) with WebGL2 capability fallback (`EXT_texture_norm16` or `RG8`, Basis UASTC transcode to native GPU format) — disabling `worldGen.worker.ts`, stopping calls to `save_world_artifact`, and dropping runtime `computeHorizonAoMap` (`worldTerrainTextures.ts:122`).
2. **Dynamic Day/Night & Water (`terrainPbrMaterial.ts`, `WorldWater.tsx`):** Shader evaluates solar elevation from `R3` against the 4 baked azimuth horizon elevation angles in `terrain_lighting` (preventing Low vs. High divergence), blends continuous top-4 biomes + `macro_albedo`, and binds baked `shore_sdf` + `hydro_flow` into `WorldWater.tsx`.
3. **Manifest hygiene (`Fix 13`, Note 5):** **`artifacts/map_manifest.json` (`mapManifestEvidence.test.ts`) is NEVER hand-edited** and `npm run gen:world-manifest` is NEVER invoked on World v2 (so `artifacts/world_256.anmw` stays untouched at `sha256:8da978ff...`). World v2 uses its own TypeScript script `scripts/gen_world_v2_manifest.ts` (run via `node scripts/run_ts.mjs scripts/gen_world_v2_manifest.ts`) generating `artifacts/world-v2/world_v2_manifest.json`, gated by `tests/frontend/worldV2ManifestEvidence.test.ts`.
4. **P1 Performance Gate (`Fix 11`, Note 7):** **`WV2-6`'s Done gate is NO FPS drop at `Low` quality measured before vs. after inside the real desktop app on the Dell Iris Xe laptop with `1,000` creatures**, reporting **`p5` FPS and per-system ms breakdown against `§3b`** (using WebGL2 GPU timer queries aligned with `NR-SPIKE`'s `G-PERF` definition; `<= 60` draw calls, `qualityTiers.ts:65`).

---

## 8. P1 Implementation Card Breakdown (Strictly Sequenced Ownership, `8` Cards)

```text
  [WV2-1a] Crate Scaffold + CI Clippy/G2 Wiring + [[test]] policy + clippy libm + Regional Tectonic Core
     │
     ▼
  [WV2-1b] Coupled G>0 Fastscape-SD Solver + Thermal/Hillslope + Priority-Flood + MFD->D8 Routing
     │
     ▼
  [WV2-2]  Post-Routing Deposition (Fans/Meanders/Deltas) + Lakes/Falls + Regional Climate + Biomes + Caves + CLI generate
     │
     ▼
  [WV2-3]  ANMW v3 Codec + Dual Identity + WorldIdentity Copy + NOTICE + Loud v3 Error + G-SIM Harness
     │
     ▼
  [WV2-4]  G-SHAPE + G-COMP v2 Rust Verifier + Calibrate & Commit Official sim_256 & gates.json (100% Local!)
     │
     ▼
  [WV2-5]  Offline Bake Engine (AO/SDF/KTX2/LODs/GPU Probe) + OxideSwarm Grid Scripts + Release Tile Pack
     │
     ▼
  [WV2-7]  Desktop Switchover (Tauri bundle.resources -> sim_256.anmw3 in AppArtifactOrGenerated)
     │     (or single combined PR with WV2-6; if WV2-6 previews earlier, "not the simulated world" banner active)
     ▼
  [WV2-6]  Three.js Bundled Viewer + worldV2ManifestEvidence + Iris Xe Low 1,000-Creature Before/After
```

### Shared-File Ownership Table (`r3 Fixes 8 & 9` — Zero Concurrent Edit Conflicts)

| Shared File | Sequential Ownership Order |
|---|---|
| `src-tauri/crates/anima-worldgen/Cargo.toml` & `src/lib.rs` & `Cargo.lock` | Created in **`WV2-1a`** (`default = []`, `bake` stub for CI clippy, `[[test]] required-features`); extended strictly sequentially along **`WV2-1b` $\to$ `WV2-2` $\to$ `WV2-3` $\to$ `WV2-4` $\to$ `WV2-5`**. |
| `src-tauri/crates/anima-worldgen/src/anmw_v3.rs` | Created & owned by **`WV2-3`**; called read-only by `WV2-4` & `WV2-5`. |
| `src-tauri/crates/anima-worldgen/src/bin/anima-worldgen.rs` | Created in **`WV2-2`** (`generate`); extended sequentially by **`WV2-4`** (`verify`). |
| `src-tauri/crates/anima-worldgen/src/bin/anima-bake.rs` | Created & owned solely by **`WV2-5`**. |
| `.github/workflows/ci.yml`, `scripts/test_target_policy.mjs`, `src-tauri/tests/feature_gated_targets_tests.rs` | Updated in **`WV2-1a`** (CI clippy `-p anima-worldgen --features reader,generate,bake`, CI `-p anima-worldgen` `check_test_targets.mjs --profile worldgen-cpu`, initial `[[test]]` policy entries); extended sequentially in **`WV2-1b`**, **`WV2-2`**, **`WV2-4`**, and **`WV2-5`** (which adds `wv2_5_gpu_bake_tests`). |
| `src-tauri/src/core/world_artifact.rs` | Updated sequentially by **`WV2-3`** (`WorldIdentity::sim_world_sha256` across `11` literal sites in `10` files + `world_seed_from_disk` loud v3 error), then by **`WV2-7`** (reads bundled `sim_256.anmw3` v3 path). |
| `src-tauri/src/core/{ecs.rs,simulation_loop.rs}` | **`WV2-3`** makes `ecs.rs:16-34` reject v3 loudly; **`WV2-7`** switches `AppArtifactOrGenerated` onto `sim_256.anmw3` cleanly (`live_experiment.rs` & `EVOLUTION_EXPERIMENT_CONTRACT.md` touched only in `WV2-3`; `simulation_loop.rs:1735` `elevation < 0.4` lake-threshold audited in `WV2-7`). |
| `src-tauri/tauri.conf.json` | **`WV2-7`** adds **only** committed `sim_256.anmw3` + `index.json` to static `bundle.resources` (safe for `tauri_build::build()` in CI); **`WV2-6`** adds `build.beforeBundleCommand` to prefetch/stage render tiles without touching static `bundle.resources`. |
| `NOTICE` & licensing artifacts | Regenerated by **`WV2-3`** (`gen_notice.mjs --check` when `sha2` becomes normal engine dep via `reader`) and **`WV2-6`** (if JS/WASM Basis/zstd decoder added). |
| `artifacts/world-v2/` (`index.json`, `sim_256.anmw3`, `gates.json`, `world_v2_manifest.json`) | Created in **`WV2-4`** (`preset_v2.toml`, `sim_256.anmw3`, initial `index.json`, `gates.json`); sequentially updated in **`WV2-5`** (`index.json` updated with release asset SHA-256s); `world_v2_manifest.json` generated in **`WV2-6`**. |

### Detailed Card Specs (Quantitative DoD + Negative Controls on Every Card)

#### Card 1: `WV2-1a` — Crate Scaffold, CI Clippy & Policy Gates, `clippy.toml` `libm` & Regional Tectonic Core (`~1.5d`)
- **Files:** `src-tauri/crates/anima-worldgen/{Cargo.toml,clippy.toml,src/{lib.rs,rng.rs,config.rs,tectonic.rs},tests/wv2_1a_tectonic_tests.rs}` (`#![cfg(feature = "generate")]` + `[[test]] required-features = ["generate"]`), `.github/workflows/ci.yml`, `scripts/test_target_policy.mjs`, `src-tauri/tests/feature_gated_targets_tests.rs`, `scripts/gen_notice.mjs`, `Cargo.lock`.
- **Tests & Negative Controls (`r3 Fix 7`):**
  - `single_vs_multi_thread_rayon_invariance` (explicitly NOT named a cross-OS probe): `1`-thread vs `N`-thread `rayon` produces bit-identical SHA-256 on `tectonic::generate_uplift` (`512²`).
  - Verify `oceanFraction ∈ [0.40, 0.50]`, `6` lithology classes (`<= 8x` erodibility), and `[-1500, +5700] m` massif peak relief range; verify `feature_gated_targets_tests` & `check_test_targets.mjs --profile worldgen-cpu` cleanly.
  - **Negative controls:** (a) mutating one std `.sin()` in `tectonic.rs` **fails** `cargo clippy -p anima-worldgen --all-targets --features reader,generate,bake -- -D warnings`; (b) mutating `anima-engine` default features to include `generate`/`wgpu` **fails** CI G2 grep; (c) omitting `[[test]] required-features` **fails** `feature_gated_targets_tests` / `check_test_targets.mjs`.
- **Definition of Done:** CI passes `cargo clippy -p anima-worldgen --all-targets --features reader,generate,bake -- -D warnings`, `cargo test -p anima-worldgen --features "reader,generate,bake"` piped through `check_test_targets.mjs --profile worldgen-cpu`, `cargo test --features desktop`, and both G2 `cargo tree -p anima-engine` checks with zero `bevy_ecs`/`tauri`/`wgpu` in `anima-worldgen`.

#### Card 2: `WV2-1b` — Coupled $G>0$ Fastscape-SD, Thermal/Nonlinear Hillslope, Priority-Flood & Hybrid MFD→D8 Routing (`~2.5d`)
- **Files:** `src-tauri/crates/anima-worldgen/{Cargo.toml,src/{lib.rs,erosion_fluvial.rs,erosion_thermal.rs,hydrology.rs},tests/wv2_1b_solver_hydro_tests.rs}` (registered in `test_target_policy.mjs` with `required-features = ["generate"]`).
- **Tests & Negative Controls (`r3 Fix 6`, `r4 Fix 1`, & `r4 Note 6`):**
  - **Paired Fastscape behavioral tests:**
    1. $G = 0$: steady-state $\log S$ vs. $\log \tilde{A}$ converges to $\theta = m/n$ within `±0.03`.
    2. **$G > 0$ coupled solver (`Yuan et al. 2019` `[44]`, `Braun 2022` `[49]`, `§2.2.2`):** fixed-point Gauss–Seidel iterate agrees at steady state with **$S^* = ((1+G)U/(K \tilde{A}^m))^{1/n}$** within **`1%` including channel-head nodes** (using implicit large-$\Delta t$ steps); AND for a **short transient** (`Note 6`: `20` implicit steps at $\Delta t = 500\text{ yr}$ vs `20,000` explicit forward-Euler steps at $\Delta t = 0.5\text{ yr}$ over `10 kyr` with elevated $K_{\text{test}} = 10^{-3}$ on a **`64`-node 1-D stream profile & `32×32` mini-grid**, keeping debug `cargo test < 1 s`) agrees within `2%` RMS error.
    3. **Arid-cell / rain-shadow $\tilde{A}$ positivity test (`r4 Fix 1`):** run $G > 0$ Fastscape-SD across a simulated leeward rain-shadow basin ($P = 0.20\text{ m/yr} < \text{PET} = 1.4\text{ m/yr}$, precipitation weight $\nu = P/\nu_0 = 0.20 > 0$) and verify zero `NaN`/`Inf`, finite $\beta_i \le G/\nu_{\min}$, and measurable fluvial gorge incision (`max_incision > 0.5 m`), while hydraulic-geometry discharge independently reads $\max(0, P - E_{\text{pot}})$.
  - Thermal/hillslope mass conservation (`|Δmass|/mass < 1e-6`), zero `>75°` non-cliff needle spikes, exact zero-pit Priority-Flood invariant (`0` unmodeled pits), and record measured Mac M5 single-step Priority-Flood & Fastscape-SD timings at `4096²`.
  - **Negative controls:** (a) skipping Gauss–Seidel $\Phi_{\text{in}}$ iteration or omitting node $i$'s local term **fails** channel-head `1%` $S^*$ and short-transient RMS checks; (b) replacing precipitation weight $\nu = P/\nu_0$ with unclamped runoff `max(0, P - E)/ν₀` in the arid rain-shadow test **fails** (produces `NaN`/`Inf` at $\tilde{A}_i = 0$ channel heads or zero fluvial incision); (c) disabling Priority-Flood **fails** zero-pit check; (d) pure D8-everywhere routing **fails** hillslope isotropy check.
- **Definition of Done:** $G=0$ concavity error `<= ±0.03`, $G>0$ steady-state error `<= 1%` across all nodes (incl. heads), short-transient `< 1 s` debug small-$\Delta t$ RMS `<= 2%`, arid rain-shadow test passes w/ unclamped-runoff negative control failing loudly, `0` post-Flood pits, and measured `4096²` step timings recorded.

#### Card 3: `WV2-2` — Deposition (Fans, Meanders, Deltas), Lakes/Falls, Regional Fixed-Latitude Climate, Zoned Biomes, Caves & CLI `generate` (`~2.5–3d`)
- **Files:** `src-tauri/crates/anima-worldgen/{Cargo.toml,src/{lib.rs,deposition.rs,lakes_falls.rs,climate.rs,biomes.rs,caves.rs,bin/anima-worldgen.rs},tests/wv2_2_climate_biome_cave_tests.rs}` (`required-features = ["generate"]` + policy entry).
- **Tests & Negative Controls:**
  - Fixed-latitude ($\phi_0 = 11^\circ\text{N}$) tropical massif climate: leeward precipitation $< 0.5\times$ windward peak (`P >= 0.15 m/yr` everywhere), temperature lapse rate $\Gamma = 6.3\text{ °C/km}$ (`[50]`), **explicit snowline/glacier onset driven by `ela_base_m = 4,900 m ∈ [4,800, 5,100] m` (`[47, 48]`, modulated locally by aspect/precip — asserting perennial `Snow`/`Glacier` appears strictly above `ela_local_m` up to `5,700 m` peaks and `0` `Glacier` below `ela_base_m - 350 m`, `r4 Fix 3`)**; `FillSpillMerge` `[3]` zero severed humid spillways; top-4 biome weights sum to `255` with ecotone fraction `∈ [0.15, 0.55]` across the `16` zoned biomes (`0` cells in `Chaparral`/`Steppe`); caves `∈ [30, 48]`, spacing `>= 0.01 * size`, zero sea/glacier caves; full `S1`–`S9` SHA-256 repeatability at `1024²`.
  - **Negative controls:** (a) disabling `FillSpillMerge` spillover step **fails** humid outlet continuity; (b) flattening peaks **fails** snowline & leeward rain-shadow asymmetry (`ratio >= 0.85`).
- **Definition of Done:** `anima-worldgen generate --size 4096` produces deterministic `raw` buffers within `<= 1.2 GB` peak RAM passing all quantitative invariants above cleanly on repeated runs.

#### Card 4: `WV2-3` — `ANMW v3` Codec, Dual Identities (`LD-2`), `WorldIdentity` (`Copy`), `NOTICE`, Loud v3 Error, & `G-SIM` Harness (`~2d`)
- **Files:** `src-tauri/crates/anima-worldgen/src/{lib.rs,anmw_v3.rs}`, `src-tauri/Cargo.toml`, `NOTICE` & licensing artifacts (`gen_notice.mjs --check`, `Note 1`), `src-tauri/src/core/{world_artifact.rs,ecs.rs,live_experiment.rs,simulation_loop.rs}` + the **`10` files (`11` struct-literal sites, `Note 2`)** needing `sim_world_sha256: None`, `docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md`, `src-tauri/tests/world_v2_sim_gate_tests.rs`.
- **Tests & Negative Controls:**
  - Pure-Rust `reader` check (`0` `zstd-sys`/`wgpu`; `gen_notice.mjs --check` green); `ANMW v3` round-trip (`sim_256.anmw3` uncompressed raw, `AnmwV3Sim::peek_seed`, `sim_world_identity`, `bundle_identity`, Strahler $\ge 4$ connected river projection); all `5` `G-SIM` gates (`GS-1..GS-5`).
  - **Negative controls:** (a) placing `ANMW v3` at `ANIMA_WORLD_ARTIFACT` in pre-switchover app causes **both `load_shared_world_artifact` (`ecs.rs:16-34`) and `world_seed_from_disk()` (`world_artifact.rs:476-481`, `Note 3`) to fail loudly**; (b) flipping `1` byte in `sim_256` or `index.json` **fails** SHA-256 preflight; (c) `Ocean` under a founder fails `GS-2`.
- **Definition of Done:** `GS-1..GS-5` pass quantitatively (`GS-1 <= 1e-5` vs raw `f32`, `GS-3 < 1e-6`, `GS-4` `0` allocs, `GS-5` `100%` paired bit-identity), `cargo test --features desktop` compiles cleanly, `gen_notice.mjs --check` passes, and `--no-default-features` / default G2 tree checks stay clean.

#### Card 5: `WV2-4` — `G-SHAPE` + `G-COMP v2` Rust Verifier & Local World v2 Seed Calibration + Commit (`~2d`)
- **Files:** `src-tauri/crates/anima-worldgen/{Cargo.toml,src/{lib.rs,verify/{mod.rs,g_shape.rs,g_comp.rs},bin/anima-worldgen.rs},tests/wv2_4_g_shape_comp_tests.rs}` (`required-features = ["generate"]` + policy entry), `artifacts/world-v2/{preset_v2.toml,index.json,sim_256.anmw3,gates.json}`. *(NEVER touches `artifacts/map_manifest.json` or `artifacts/world_256.anmw`!)*
- **Tests & Negative Controls (`100%` closable locally on Mac without waiting on OxideSwarm grid!):**
  - Planted negative controls: (a) parallel-comb DEM **fails** junction angle / Horton; (b) cratered noise **fails** micro-pit / $HI$; (c) inverted valley **fails** $V_{f,\text{down}}/V_{f,\text{up}}$; (d) uniform-biome or steppe-containing DEM **fails** `G-COMP v2` `C1`/`C2`.
- **Definition of Done:** Generated `4096²` official seed passes `100%` of `G-SHAPE` (`§4.1`), `G-COMP v2` (`§5.1`: `16` `LD-5` zoned biomes `>= 2,500` cells each, `Chaparral`/`Steppe` == `0` cells, runoff-derived `C4` river widths), and `G-SIM` inside committed `gates.json`, with `sim_256.anmw3` + initial `index.json` committed (`~2.0 MiB`; `world_256.anmw` `8da978ff...` untouched).

#### Card 6: `WV2-5` — Offline Bake Pipeline (`AO`, `SDF`, Flow, KTX2/Basis, LODs/Impostors, Step-0 GPU Probe) & Release Tile Pack (`~3d`, runs strictly *after* `WV2-4`)
- **Files:** `src-tauri/crates/anima-worldgen/{Cargo.toml,src/{lib.rs,bake/{mod.rs,fields.rs,lighting_cpu.rs,lighting_gpu.rs,pack_ktx2.rs,flora_place.rs,assets.rs},bin/anima-bake.rs},tests/{wv2_5_bake_tests.rs,wv2_5_gpu_bake_tests.rs}}`, `scripts/grid_bake_world_v2.sh`, `scripts/test_target_policy.mjs`, `src-tauri/tests/feature_gated_targets_tests.rs`, updated `artifacts/world-v2/index.json` (`tiles/` archive published as release asset **without `index.json` inside archive**). *(Requires explicit owner approval lines inside card before syncing CC0 assets to `JOB-W6` via `--sync` and publishing the public GitHub Release.)*
- **Tests & Negative Controls:**
  - Exact Meijster EDT vs brute-force $O(N^2)$ on `128²`; strip-sharding byte-identity (`strip 0..K` == global pass); `cave_network.bin` LE binary parser; KTX2 header + mip completeness + `Block PSNR >= 42 dB, SSIM >= 0.996`; GPU-hardware `wv2_5_gpu_bake_tests` (SSIM `>= 0.999`, policy-registered for `gpu-bake`).
  - **Negative controls:** (a) removing `2`-texel tile gutter **fails** boundary equality; (b) tampering `1` tile byte **fails** `index.json` archive verifier; (c) including `index.json` in the archive **fails** packager self-check.
- **Definition of Done:** Verified `4096²` release tile archive (`~103–145 MiB`, excluding `index.json`) published/staged cleanly matching `index.json` `bundle_identity`, `sim_world_identity` unchanged from `WV2-4`, measured bake timings recorded.

#### Card 7: `WV2-7` — Desktop Simulation Switchover onto Bundled `sim_256.anmw3` (`LD-4`, r3 Fixes 8 & 9, r4 Notes 4 & 5 — merges *before* or *with* `WV2-6`, `~1d`)
- **Files:** `src-tauri/tauri.conf.json` (`bundle.resources` staging **only Git-committed** `artifacts/world-v2/sim_256.anmw3` & `artifacts/world-v2/index.json` so `tauri_build::build()` never fails CI before tile prefetch, `r4 Note 5`), `src-tauri/src/core/{ecs.rs,world_artifact.rs,simulation_loop.rs}`, `src-tauri/tests/world_v2_desktop_switch_tests.rs`.
- **Tests & Negative Controls:**
  - Packaged & dev Tauri startup resolves `sim_256.anmw3` via `app.path().resource_dir().join("world-v2/sim_256.anmw3")` on the main thread and passes that `PathBuf` explicitly to `world_seed_from_disk` & `SimulationEngine` sim thread (`world_artifact.rs:458-466`), verifying `sim_world_identity` (`sha256`), resolving `AnmwV3Sim::peek_seed`, and booting `AppArtifactOrGenerated`.
  - **Audit & adjust/parameterize non-LAND1 lake-candidate `elevation < 0.4` check (`simulation_loop.rs:1735`, `1,380 m` in v3, `r4 Note 4`)** relative to `sea_level_norm` cleanly under v3.
  - Paired same-machine `TerrainSource::Generated` (`E-SEL1`/`SEL2-P`) remains `100%` bit-identical (`GS-5`).
  - **Negative controls:** (a) missing/corrupt `sim_256.anmw3` or mismatched `sim_world_sha256` **fails loudly** (`Err`, never falls back to procedural noise); (b) legacy `.anmw` v2 fixtures (`world_real_128.anmw`) continue decoding cleanly.
- **Definition of Done:** Desktop simulation runs natively on committed `artifacts/world-v2/sim_256.anmw3` via Tauri resource handoff with `sim_world_identity` preflight active, passing `cargo test --features desktop` & `check_test_targets.mjs`.

#### Card 8: `WV2-6` — Three.js Bundled World v2 Viewer, `gen_world_v2_manifest.ts` & Dell Iris Xe Before/After Gate (`~2–2.5d`)
- **Files:** `src-tauri/tauri.conf.json` (`build.beforeBundleCommand` invoking `fetch_world_v2_tiles.mjs` into bundle staging — leaves static `bundle.resources` limited to Git-committed files, `r4 Note 5`), `scripts/fetch_world_v2_tiles.mjs`, `scripts/gen_world_v2_manifest.ts` (run via `node scripts/run_ts.mjs`), `src/components/Landscape/utils/{worldArtifactV3.ts,worldCache.ts,worldTerrainTextures.ts}`, `src/components/Landscape/materials/terrainPbrMaterial.ts`, `src/components/Landscape/{WorldTerrain.tsx,WorldTerrainLod.tsx,WorldWater.tsx,WorldWaterfalls.tsx,WorldCaves.tsx,WorldShowcase.tsx}`, `artifacts/world-v2/world_v2_manifest.json`, `tests/frontend/{worldArtifactV3.test.ts,worldV2ManifestEvidence.test.ts,worldV2Viewer.test.tsx}`, `docs/reference/IPC_CONTRACT.md` (if asset/IPC surface updated), `NOTICE`. *(Touches NEITHER `worldComposition.ts` NOR `map_manifest.json`!)*
- **Tests & Negative Controls:**
  - Vitest `worldArtifactV3.test.ts`: tile SHA-256 checks, `sim_256` & `cave_network.bin` LE decoding parity, WebGL2 `RG8`/Basis fallback, decorative `§6` UI toggle (`flora_tiles`/lakes/caves), zero calls to `worldGen.worker.ts` or `save_world_artifact` (banner shown if previewed without `WV2-7`).
  - **Negative controls:** (a) single-byte tamper in tile pack or `index.json` **fails** prefetch/loader; (b) stale `world_v2_manifest.json` **fails** `worldV2ManifestEvidence.test.ts` while legacy `mapManifestEvidence.test.ts` (`8da978ff...`) stays green.
- **Definition of Done (`Fix 11`, Note 7):** Build-time prefetch bundles tiles cleanly (`0` runtime network calls), `check:bundle` / `check:docs` / `check:ipc` pass, AND **real Dell Iris Xe desktop app benchmark at `Low` quality with `1,000` creatures shows NO FPS drop before vs. after**, reporting **`p5` FPS and per-system ms aligned with `NR-SPIKE` `G-PERF` against `§3b`** (`<= 60` draw calls).

---

## 9. Verified Reference List (`r3 Fix 1` & `r4 Additions` — Mechanically Checked `2026-11-20`)

Every DOI below was resolved mechanically on `2026-11-20` against `doi.org/api/handles/<doi>` (`50/50` individual DOIs across `48` DOI-bearing entries returned `responseCode = 1`, HTTP `200`; `[26]` and `[38]` each cite 2 DOIs; `[9]`, `[29]`, `[43]` are books verified by ISBN/library record).

1. Barnes, R., Lehman, C., & Mulla, D. (2014). *Priority-Flood: An Optimal Depression-Filling and Watershed-Labeling Algorithm for Digital Elevation Models*. *Computers & Geosciences*, 62, 117–127. `https://doi.org/10.1016/j.cageo.2013.04.024`
2. Barnes, R., Callaghan, K. L., & Wickert, A. D. (2020). *Computing water flow through complex landscapes – Part 2: Finding hierarchies in depressions and morphological segmentations*. *Earth Surface Dynamics*, 8(2), 431–445. `https://doi.org/10.5194/esurf-8-431-2020`
3. Barnes, R., Callaghan, K. L., & Wickert, A. D. (2021). *Computing water flow through complex landscapes – Part 3: Fill–Spill–Merge: flow routing in depression hierarchies*. *Earth Surface Dynamics*, 9(1), 105–121. `https://doi.org/10.5194/esurf-9-105-2021`
4. Beven, K. J., & Kirkby, M. J. (1979). *A physically based, variable contributing area model of basin hydrology*. *Hydrological Sciences Bulletin*, 24(1), 43–69. `https://doi.org/10.1080/02626667909491834`
5. Braun, J., & Willett, S. D. (2013). *A very efficient O(n), implicit and parallel method to solve the stream power equation governing fluvial incision and landscape evolution*. *Geomorphology*, 180–181, 170–179. `https://doi.org/10.1016/j.geomorph.2012.10.008`
6. Bull, W. B., & McFadden, L. D. (1977). *Tectonic geomorphology north and south of the Garlock Fault, California*. In D. O. Doehring (Ed.), *Geomorphology in Arid Regions*, pp. 115–138. `https://doi.org/10.4324/9780429299230-5`
7. Cordonnier, G., Braun, J., Cani, M.-P., Benes, B., Galin, E., Peytavie, A., & Guérin, E. (2016). *Large Scale Terrain Generation from Tectonic Uplift and Fluvial Erosion*. *Computer Graphics Forum*, 35(2), 165–175. `https://doi.org/10.1111/cgf.12820`
8. Cordonnier, G., Cani, M.-P., Benes, B., Braun, J., & Galin, E. (2018). *Sculpting Mountains: Interactive Terrain Modeling Based on Subsurface Geology*. *IEEE Transactions on Visualization and Computer Graphics*, 24(5), 1756–1769. `https://doi.org/10.1109/TVCG.2017.2689022`
9. Cuffey, K. M., & Paterson, W. S. B. (2010). *The Physics of Glaciers* (4th ed.). Academic Press. ISBN `978-0-12-369461-4`.
10. Davy, P., & Lague, D. (2009). *Fluvial erosion/transport equation of landscape evolution models revisited*. *Journal of Geophysical Research: Earth Surface*, 114(F3), F03007. `https://doi.org/10.1029/2008JF001146`
11. Flint, J. J. (1974). *Stream gradient as a function of order, magnitude, and discharge*. *Water Resources Research*, 10(5), 969–973. `https://doi.org/10.1029/WR010i005p00969`
12. Gray, D. M. (1961). *Interrelationships of Watershed Characteristics*. *Journal of Geophysical Research*, 66(4), 1215–1223. `https://doi.org/10.1029/JZ066i004p01215`
13. Hack, J. T. (1957). *Studies of Longitudinal Stream Profiles in Virginia and Maryland*. U.S. Geological Survey Professional Paper 294-B. `https://doi.org/10.3133/pp294B`
14. Harbor, J. M. (1992). *Numerical modeling of the development of U-shaped valleys by glacial erosion*. *GSA Bulletin*, 104(10), 1364–1375. `https://doi.org/10.1130/0016-7606(1992)104<1364:NMOTDO>2.3.CO;2`
15. Holdridge, L. R. (1947). *Determination of World Plant Formations from Simple Climatic Data*. *Science*, 105(2727), 367–368. `https://doi.org/10.1126/science.105.2727.367`
16. Holmgren, P. (1994). *Multiple flow direction algorithms for runoff modelling in grid based elevation models: An empirical evaluation*. *Hydrological Processes*, 8(4), 327–334. `https://doi.org/10.1002/hyp.3360080405`
17. Horton, R. E. (1945). *Erosional development of streams and their drainage basins; hydrophysical approach to quantitative morphology*. *GSA Bulletin*, 56(3), 275–370. `https://doi.org/10.1130/0016-7606(1945)56[275:EDOSAT]2.0.CO;2`
18. Howard, A. D. (1971). *Optimal angles of stream junction: Geometric, stability to capture, and minimum power criteria*. *Water Resources Research*, 7(4), 863–873. `https://doi.org/10.1029/WR007i004p00863`
19. Howard, A. D., & Knutson, T. R. (1984). *Sufficient conditions for river meandering: A simulation approach*. *Water Resources Research*, 20(11), 1659–1667. `https://doi.org/10.1029/WR020i011p01659`
20. Howard, A. D., Dietrich, W. E., & Seidl, M. A. (1994). *Modeling fluvial erosion on regional to continental scales*. *Journal of Geophysical Research: Solid Earth*, 99(B7), 13971–13986. `https://doi.org/10.1029/94JB00744`
21. Kirchner, J. W. (1993). *Statistical inevitability of Horton's laws and the apparent randomness of stream channel networks*. *Geology*, 21(7), 591–594. `https://doi.org/10.1130/0091-7613(1993)021<0591:SIOHSL>2.3.CO;2`
22. Lehner, B., & Grill, G. (2013). *Global river hydrography and network routing: Baseline data and new approaches to study the world's large river systems*. *Hydrological Processes*, 27(15), 2171–2186. `https://doi.org/10.1002/hyp.9740`
23. Leopold, L. B., & Maddock, T. (1953). *The hydraulic geometry of stream channels and some physiographic implications*. U.S. Geological Survey Professional Paper 252. `https://doi.org/10.3133/pp252`
24. Liang, M., Voller, V. R., & Paola, C. (2015). *A reduced-complexity model for river delta formation – Part 1: Modeling deltas with channel dynamics*. *Earth Surface Dynamics*, 3(1), 67–86. `https://doi.org/10.5194/esurf-3-67-2015`
25. Meijster, A., Roerdink, J. B. T. M., & Hesselink, W. H. (2000). *A General Algorithm for Computing Distance Transforms in Linear Time*. In *Mathematical Morphology and its Applications to Image and Signal Processing*, pp. 331–340. `https://doi.org/10.1007/0-306-47025-X_36`
26. Montgomery, D. R., & Dietrich, W. E. (1988). *Where do channels begin?* *Nature*, 336, 232–234. `https://doi.org/10.1038/336232a0`; and Montgomery, D. R., & Dietrich, W. E. (1989). *Source areas, drainage density, and channel initiation*. *Water Resources Research*, 25(8), 1907–1918. `https://doi.org/10.1029/WR025i008p01907`
27. Musgrave, F. K., Kolb, C. E., & Mace, R. S. (1989). *The Synthesis and Rendering of Eroded Fractal Terrains*. *ACM SIGGRAPH Computer Graphics*, 23(3), 41–50. `https://doi.org/10.1145/74334.74337`
28. O'Callaghan, J. F., & Mark, D. M. (1984). *The extraction of drainage networks from digital elevation data*. *Computer Vision, Graphics, and Image Processing*, 28(3), 323–344. `https://doi.org/10.1016/S0734-189X(84)80011-0`
29. Oerlemans, J. (2001). *Glaciers and Climate Change: A Meteorologist's View*. A.A. Balkema Publishers. ISBN `978-90-265-1813-3`.
30. Palmer, A. N. (1991). *Origin and morphology of limestone caves*. *GSA Bulletin*, 103(1), 1–21. `https://doi.org/10.1130/0016-7606(1991)103<0001:OAMOLC>2.3.CO;2`
31. Quinn, P., Beven, K., Chevallier, P., & Planchon, O. (1991). *The prediction of hillslope flow paths for distributed hydrological modelling using digital terrain models*. *Hydrological Processes*, 5(1), 59–79. `https://doi.org/10.1002/hyp.3360050106`
32. Rigon, R., Rodríguez-Iturbe, I., Maritan, A., Giacometti, A., Tarboton, D. G., & Rinaldo, A. (1996). *On Hack's law*. *Water Resources Research*, 32(11), 3367–3374. `https://doi.org/10.1029/96WR02397`
33. Roe, G. H. (2005). *Orographic Precipitation*. *Annual Review of Earth and Planetary Sciences*, 33, 645–671. `https://doi.org/10.1146/annurev.earth.33.092203.122541`
34. Roering, J. J., Kirchner, J. W., & Dietrich, W. E. (1999). *Evidence for Nonlinear, Diffusive Sediment Transport on Hillslopes and Implications for Landscape Morphology*. *Water Resources Research*, 35(3), 853–870. `https://doi.org/10.1029/1998WR900090`
35. Seybold, H., Rothman, D. H., & Kirchner, J. W. (2017). *Climate's watermark in the geometry of stream networks*. *Geophysical Research Letters*, 44(5), 2272–2280. `https://doi.org/10.1002/2016GL072089`
36. Smith, R. B., & Barstad, I. (2004). *A Linear Theory of Orographic Precipitation*. *Journal of the Atmospheric Sciences*, 61(12), 1377–1391. `https://doi.org/10.1175/1520-0469(2004)061<1377:ALTOOP>2.0.CO;2`
37. Stewart, A. J. (1998). *Fast Horizon Computation at All Points of a Terrain with Visibility and Shading Applications*. *IEEE Transactions on Visualization and Computer Graphics*, 4(1), 82–93. `https://doi.org/10.1109/2945.675656`
38. Strahler, A. N. (1952). *Hypsometric (area-altitude) analysis of erosional topography*. *GSA Bulletin*, 63(11), 1117–1142. `https://doi.org/10.1130/0016-7606(1952)63[1117:HAAOET]2.0.CO;2`; and Strahler, A. N. (1957). *Quantitative analysis of watershed geomorphology*. *Transactions, American Geophysical Union*, 38(6), 913–920. `https://doi.org/10.1029/TR038i006p00913`
39. Tucker, G. E., Catani, F., Rinaldo, A., & Bras, R. L. (2001). *Statistical analysis of drainage density from digital terrain data*. *Geomorphology*, 36(3–4), 187–202. `https://doi.org/10.1016/S0169-555X(00)00056-8`
40. Wang, J., Song, C., Reager, J. T., Yao, F., Famiglietti, J. S., Sheng, Y., MacDonald, G. M., Brun, F., Müller Schmied, H., Marston, R. A., & Wada, Y. (2018). *Recent global decline in endorheic basin water storages*. *Nature Geoscience*, 11(12), 926–932. `https://doi.org/10.1038/s41561-018-0265-7`
41. Werner, B. T. (1995). *Eolian dunes: Computer simulations and attractor interpretation*. *Geology*, 23(12), 1107–1110. `https://doi.org/10.1130/0091-7613(1995)023<1107:EDCSAA>2.3.CO;2`
42. Whipple, K. X., & Tucker, G. E. (1999). *Dynamics of the stream-power river incision model*. *Journal of Geophysical Research: Solid Earth*, 104(B8), 17661–17674. `https://doi.org/10.1029/1999JB900120`
43. Whittaker, R. H. (1975). *Communities and Ecosystems* (2nd ed.). Macmillan, New York. ISBN `0-02-427390-2` / `978-0024273900`.
44. Yuan, X. P., Braun, J., Guerit, L., Rouby, D., & Cordonnier, G. (2019). *A New Efficient Method to Solve the Stream Power Law Model Taking Into Account Sediment Deposition*. *Journal of Geophysical Research: Earth Surface*, 124(6), 1346–1365. `https://doi.org/10.1029/2018JF004867`
45. Fortier, R. P., Kullberg, A. T., Aragón, L., Bernal-Escobar, M., Contreras, M. P., Palacios Hurtado, C., Lautenschlager, L., & Feeley, K. J. (2025). *Plant Diversity and Endemism of the Sierra Nevada de Santa Marta, Colombia, with Floristic Comparisons to Surrounding Mountains*. *Annals of the Missouri Botanical Garden*, 110, 71–87. `https://doi.org/10.3417/2025921`
46. López-Moreno, J. I., Ceballos, J. L., Rojas-Heredia, F., Zabalza-Martinez, J., Vidaller, I., Revuelto, J., Alonso-González, E., Morán-Tejeda, E., & García-Ruiz, J. M. (2020). *Topographic control of glacier changes since the end of the Little Ice Age in the Sierra Nevada de Santa Marta mountains, Colombia*. *Journal of South American Earth Sciences*, 104, 102803. `https://doi.org/10.1016/j.jsames.2020.102803`
47. Poveda, G., & Pineda, K. (2009). *Reassessment of Colombia's tropical glaciers retreat rates: are they bound to disappear during the 2010–2020 decade?* *Advances in Geosciences*, 22, 107–116. `https://doi.org/10.5194/adgeo-22-107-2009`
48. Rabatel, A., Francou, B., Soruco, A., Gomez, J., Cáceres, B., Basantes, R., Vuille, M., Sicart, J.-E., Huggel, C., Scheel, M., Lejeune, Y., Arnaud, Y., Collet, M., Condom, T., Consoli, G., Favier, V., Jomelli, V., Galarraga, R., Ginot, P., Maisincho, L., Mendoza, J., Ménégoz, M., Ramirez, E., Ribstein, P., Suarez, W., Villacis, M., & Wagnon, P. (2013). *Current state of glaciers in the tropical Andes: a multi-century perspective on glacier evolution and climate change*. *The Cryosphere*, 7, 81–102. `https://doi.org/10.5194/tc-7-81-2013`
49. Braun, J. (2022). *Comparing the transport-limited and ξ–q models for sediment transport*. *Earth Surface Dynamics*, 10(2), 301–327. `https://doi.org/10.5194/esurf-10-301-2022`
50. Hoyos, N., Correa-Metrio, A., Jepsen, S. M., Wemple, B., Valencia, S., Marsik, M., Doria, R., Escobar, J., Restrepo, J. C., & Velez, M. I. (2019). *Modeling Streamflow Response to Persistent Drought in a Coastal Tropical Mountainous Watershed, Sierra Nevada De Santa Marta, Colombia*. *Water*, 11(1), 94. `https://doi.org/10.3390/w11010094`
51. Valencia, J., Frankenberger, J., Cherkauer, K., Martín-López, J. M., Monserrate, F., & da Silva, M. (2022). *Water sufficiency for cacao production in the Sierra Nevada de Santa Marta (SNSM) region, Colombia*. *Journal of Hydrology: Regional Studies*, 44, 101255. `https://doi.org/10.1016/j.ejrh.2022.101255`
