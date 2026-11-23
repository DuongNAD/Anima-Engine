---
phase: planning
feature: map-v2
title: Protocol — NR-SPIKE: Native Renderer Comparative Benchmark & ADR-0011 Decision Protocol (Map v2 Phase P2)
description: Documents-only measurement protocol for Map v2 Phase P2 (NR-SPIKE), defining the fixed World v2 scene (terrain, water, 10000 trees, 1000 creatures), three renderer candidates (wgpu in Tauri, newer Bevy, Godot 4), RTX High/Low and Dell Iris Xe Low prebuilt-binary test matrix, capture commands, RAM/VRAM ceilings, integration/packaging scoring, and ADR-0011 decision rule
status: proposed
owner: anima-case
last_reviewed: 2026-11-23
---

# Protocol — `NR-SPIKE`: Native Renderer Comparative Benchmark & `ADR-0011` Decision Protocol (Map v2 Phase P2)

> **Status: `proposed` (documents only — awaiting Lead verification & Owner approval before writing any spike code or installing external tools).**
> - **Zero spike code exists or is added by this PR:** No Rust, TypeScript, WGSL, GDScript, `Cargo.toml`, or fixture changes are made here; per [Map v2 Requirements §4 P2](../requirements/2026-11-20-feature-map-v2.md), **spike code is never merged** (only measurement notes and `ADR-0011`).
> - **`ADR-0011` does not yet exist:** `docs/decisions/` currently ends at [ADR-0010](../../decisions/ADR-0010-pinned-world-artifact.md). All architectural preferences or cost estimates in the [Native Renderer Research Note](../../research/NATIVE_RENDERER_WGPU_TAURI_BAKING_RESEARCH_2026-11-22.md) are **explicitly marked `[HYPOTHESIS TO TEST]`** below, never treated as measured facts or pre-decided choices.
> - **Accepted Gates vs. `PROPOSED — OWNER DECISION NR-D1..NR-D7`:** Every threshold/budget defined in [Map v2 Requirements](../requirements/2026-11-20-feature-map-v2.md) or [World v2 Design (`WV2-D`)](../design/2026-11-20-world-v2-design.md) stays locked as an **ACCEPTED** hard gate. Every *new* ceiling, rubric weight, selection margin, tie-breaker, or local tool installation introduced by this protocol is explicitly labelled **`PROPOSED — OWNER DECISION NR-D<n>`** (collected in [`§9`](#9-owner-decision-summary-nr-d1nr-d7)) and is **not enforced or installed until the Owner approves it**.

---

## 0. Scope, Provenance & Preserved Contracts

### 0.1 Authoritative Inputs & Observed Trunk Pins (`c0ce44b2`)

| Source | Role | Pin / Status on `origin/feat/ecological-selection` |
|---|---|---|
| [Map v2 Requirements](../requirements/2026-11-20-feature-map-v2.md) | Hard gates (`G-REF`, `G-PERF`, `G-SIM`), `§3b` frame budgets, `§4` Phase P2 mandate, `§6` Honesty rule | `accepted` (`c0ce44b2`) |
| [World v2 Design (`WV2-D`)](../design/2026-11-20-world-v2-design.md) | `4096²` tile pack format (`ANMW v3`, `260×260` guttered tiles), `bc7enc_rdo` + KTX2, `Block PSNR >= 42 dB, SSIM >= 0.996` (`WV2-5`), `G-SIM` | `accepted` (`c0ce44b2`) |
| [Map v2 G-REF Reference Board](../design/2026-11-20-map-v2-reference-board.md) | 53 CC0/PD visual benchmarks across 13 biome/coastal/water categories | `accepted` (`c0ce44b2`) |
| [Native Renderer Research Note](../../research/NATIVE_RENDERER_WGPU_TAURI_BAKING_RESEARCH_2026-11-22.md) | Architecture deep-dive for candidate `(a)` & offline baking hypotheses (`[HYPOTHESIS TO TEST]` only) | `proposed` (`c0ce44b2`) |
| [Readable World Plan](2026-11-16-feature-readable-world.md) | Existing Three.js baseline tiers (`qualityTiers.ts`), `1,000 × 3`-segment creatures, canonical views | `active` (`c0ce44b2`) |
| [ADR-0008](../../decisions/ADR-0008-world-past-and-time.md), [ADR-0009](../../decisions/ADR-0009-burn-022-flex-wgpu-upgrade.md), [ADR-0010](../../decisions/ADR-0010-pinned-world-artifact.md) | Trajectory era discipline, `burn 0.22.1` + `wgpu 30.0.2` stream isolation, pinned world artifact contract | `accepted` / `proposed` (`ADR-0009`) |
| `src-tauri/Cargo.lock` & `.github/workflows/ci.yml` | Verified pins at `c0ce44b2`: `tauri 2.11.5`, `wry 0.55.1`, `tao 0.35.3`, `wgpu 30.0.2`, `naga 30.0.2`, `bevy_ecs 0.13.2`, `burn 0.22.1`, `raw-window-handle 0.6.2`, `libm 0.2.16`, `glam 0.25.0` | Inspected at `c0ce44b2` |

### 0.2 Preserved Requirements That Spike Candidates Must Not Drop

1. **Low-tier zero-shadow-map rule (`Map v2 §3b` line 109 / `req :109`):** At **Low** quality, terrain/world lighting MUST come exclusively from pre-baked `terrain_lighting` (AO, SVF, 4-azimuth horizon elevation angles synced with sun elevation, bounce probe), distant flora uses impostors, and creatures use simple contact/blob shadows — **zero real-time shadow maps and zero CSM passes**. Any `~4.2 ms` Low-tier CSM estimate in the research note (`§2.1`) conflicts with `req :109` and is excluded from the Low benchmark configuration.
2. **High vs. Low visual cohesion (`Map v2 §3b` line 111):** High adds real-time CSM shadows, real-time AO, volumetric fog/aerial perspective, and water SSR, while preserving the same terrain silhouettes, baked albedo/normal materials, sky/sun direction, and creature morphology so Low and High never look like two unrelated scenes.
3. **Accepted texture & bake quality gates (`WV2-D` lines 117, 440–442, 553):** Every World v2 tile input used in the spike MUST satisfy **`Block PSNR >= 42 dB, SSIM >= 0.996`** (`bc7enc_rdo` + KTX2 UASTC/transcoded, or a candidate native BC7/BC5 transcode parity check that still clears `SSIM >= 0.996` and `Block PSNR >= 42 dB`). Any GPU lighting bake satisfies `SSIM >= 0.999`; impostor bakes satisfy `SSIM >= 0.995`. The research note §3.4's looser `SSIM >= 0.98` proposal is explicitly rejected unless `WV2-D` is formally amended.
4. **`G-SIM` & `§6` Honesty invariants (`Map v2 §1, §6`; `WV2-D §5.2`):** The simulation tick path remains zero-alloc (`GS-4`), `sim_256.anmw3` remains the single simulation height/walkability source (`sim_world_identity`), closed EU stays `< 1e-6`, decorative flora (`10,000` baked tree instances) and decorative caves carry a working UI honesty toggle, and all geometry/texture assets are strictly **CC0** (`QD-ASSETS`).
5. **CI G2 feature isolation (`src-tauri/Cargo.toml`, `.github/workflows/ci.yml:136-148`):** Because CI rejects `wgpu`, `naga`, `zstd-sys`, `meshopt`, `bc7enc`, and `ktx2` in `--no-default-features` and default `anima-engine` builds — and `wgpu 30.0.2` enters `--features desktop` solely via `burn/wgpu` (`ml-wgpu`) — every candidate spike binary builds inside an isolated standalone spike workspace/directory (never modifying default `anima-engine` feature resolution).

---

## 1. The One Fixed Scene (`S-NR-FIXED`), Quality Tiers & Camera Runs

All three candidates **`(a)`**, **`(b)`**, and **`(c)`** MUST render the **exact same frozen scene package** identified by one `scene_bundle_sha256` manifest (`scene_bundle_manifest.json`), with identical terrain geometry/layers, water geometry/masks, tree placements, creature trajectory, and camera poses.

### 1.1 Fixed Scene Contents (`40.96 km × 40.96 km` World v2 Domain)

| Scene Element | Exact Fixed Specification | Source & Alignment Contract |
|---|---|---|
| **Terrain** | One SHA-256-pinned **`4096²` (`10 m/cell`) World v2 artifact** (`256²` `sim_256.anmw3` + `260×260` guttered `L0..L4` tiles, up to 4 active splat/biome layers per pixel + `macro_albedo` + `normal_xy` + `terrain_lighting` [AO, SVF, 4-azimuth horizon angles, bounce probe], `mesh` CDLOD/clipmap displacement) | `WV2-D §3.2`. **Pre-`WV2-5` contingency:** If P2 executes before `WV2-5` publishes official release tiles, freeze one deterministic `anima-worldgen` `4096²` export + `bc7enc_rdo` KTX2 bake clearing `Block PSNR >= 42 dB, SSIM >= 0.996` and pin its SHA-256 as `scene_bundle_sha256` across all 3 candidates. |
| **Water** | World v2 `water_surface` (`water_h`, `water_kind` across ocean, mountain/rift lakes, sloped Strahler $\ge 4$ rivers), baked `shore_sdf` coastal foam/wet-sand mask, and baked `hydro_flow` $(\hat{u}_x, \hat{u}_z, Q)$ flow advection | `Map v2 §3b.5`, `WV2-D §3.2`. Low renders Fresnel + baked sky reflection + depth tint + `shore_sdf` foam (**no SSR**); High adds **SSR** + refraction/ripples. |
| **Trees (`10,000`)** | Fixed deterministic placement table of **`10,000` trees** across `4` CC0 tree species (broadleaf, montane conifer, riparian palm/shrub, dry-slope scrub), authored from pinned CC0 meshes processed with 3 LODs + hemi-octahedral impostors (`Block PSNR >= 42 dB, SSIM >= 0.996`, mip coverage preserved), plus a working `§6` decorative visibility toggle | `Map v2 §4 P2` mandatory count (`10,000` trees). Frustum culling and distance-based LOD/impostor transitions are **allowed and encouraged**; artificially dropping instances from the world or culling visible trees below camera sightlines to inflate FPS is **disallowed** (verified against canonical stills). Optional `>100k`-tree tests in research §4 Mốc 2 are **non-gating** extras only. |
| **Creatures (`1,000`)** | Fixed deterministic `30 s` looped **`30 Hz` tick stream** of **`1,000` multi-segment articulated creatures × `3` segments (`3,000` segments)** (`length`, `radius`, world `(x, y, z)`, `yaw/pitch/roll` in `YXZ` Euler order, energy tint, predator/prey head shape), interpolated cleanly per rendered frame using the `RW3-1` delay-buffer interpolation contract (`0` allocations in render loop) | `Map v2 §1 G-PERF`, `2026-11-16-feature-readable-world.md §4` (`1,000 × 3`). Candidates `(a)` and `(b)` consume the shared slice in-process (simulating `bevy_ecs` direct readback); candidate `(c)` receives the exact same serialized `30 Hz` `3,000`-segment frame over local loopback IPC (`127.0.0.1`) from a headless Rust emitter process. |
| **Sky & Atmosphere** | Sun elevation + azimuth driven by fixed scene clock (`11:00` daytime default + slow sun-elevation sweep test), physical sky scattering LUTs / zenith-horizon sky + distance fog; High adds aerial-perspective froxel / volumetric haze + AgX tone mapping | `Map v2 §3b.7`, `§4 P3 R3`. |
| **HUD Overlay** | Minimal fixed overlay present in all frames: pause button, quality selector (`Low`/`High`), `§6` decorative-flora toggle, FPS counter readout, and an active text input box (to verify Windows TSF / Vietnamese IME focus) | Ensures WebView2/`WKWebView` transparency/compositor overhead (`(a)`/`(b)`) or OS window chrome (`(c)`) is measured honestly. |

### 1.2 Quality Tier Parameter Contract (`Low` vs. `High`)

| Parameter | **Low Tier** (`Dell Iris Xe` & `RTX 5060 Ti` baseline) | **High Tier** (`RTX 5060 Ti` only) | Source Rule |
|---|---|---|---|
| Display / Window Size | `1920 × 1080` physical window (`scale = 1.0`, fullscreen-borderless or fixed client `1920×1080`) | `1920 × 1080` physical window | Standardizes desktop pixel load across RTX and Dell FHD panel |
| Internal 3D Resolution | `1280 × 720` (`0.67×` scale), upscaled to `1920 × 1080` (default candidate temporal/spatial upscale; diagnostic DRS fallback down to `0.50×` logged separately if needed for `H-NR-01`, never disguised as `0.67×`) | `1920 × 1080` (`1.0×` native internal) | `Map v2 §3b` (`720p` upscaled on Low), research §2.1 |
| Real-Time Shadow Maps / CSM | **OFF (`0` cascades, `0` shadow passes — hard requirement)** | **ON** (`3` cascades, `2048²`, percentage/PCF) | **`Map v2 §3b` line 109 (`req :109`)** |
| Terrain Lighting & Creature Shadows | Baked `terrain_lighting` (AO, SVF, horizon angles) + creature blob/contact shadow (`0` shadow passes) | Dynamic CSM shadows + real-time SSAO/GTAO combined with baked bounce/SVF | `Map v2 §3b` lines 84–86, 109–111 |
| Vegetation (`10,000` trees) | Near LOD meshes `< 160` sim-units, hemi-octahedral impostors `>= 160` sim-units | Full LOD0/1 near meshes `< 360` sim-units, mid LOD to `700`, impostors beyond; trees cast CSM shadows | `Map v2 §3b.4`, `qualityTiers.ts` aligned to `10,000` fixed scene |
| Water | Fresnel + sky reflection + depth tint + `shore_sdf` foam + `hydro_flow` (**SSR OFF**) | Adds **SSR** + wave normal ripples + refraction | `Map v2 §3b.5` (`lines 101–105`) |
| Fog & Tone Mapping | Analytic distance fog + Hillaire-style 2D Sky LUT + ACES/AgX fit | Volumetric aerial perspective + AgX tone mapping | `Map v2 §4 P3 R3/R6` |
| Present Mode | `Immediate` / `AutoNoVsync` during FPS measurement (so 60 Hz panel vsync does not cap RTX High at 60 FPS); optional VSync tear-free stability check in interactive pass | `Immediate` / `AutoNoVsync` during FPS measurement | Uncaps raw GPU frame-time measurement above monitor refresh |

### 1.3 Standardized Camera Trajectory (`90 s` Benchmark + `15 min` Thermal Soak)

Every run executes a deterministic scripted camera path derived from Anima's canonical views (`CANONICAL_VIEW_CAMERAS` in `src/components/Landscape/utils/mapManifest.ts`, mapped cleanly onto the `40.96 km` World v2 massif coordinates so zero views stare into empty sky/subsurface):

| Phase | Window | Duration | Camera Path & Scene Load | Counted in Metrics? |
|---|---|---|---|---|
| **Warmup** | `0..15 s` | `15 s` | Initial settle at `overview` + shader/pipeline compilation flush | **Excluded** (recorded separately to catch shader stalls) |
| **Segment A — `overview`** | `15..30 s` | `15 s` | Slow `30°` elevated orbit overlooking full massif, ocean, lakes, rivers & `10,000`-tree forest (`high draw/cull breadth`) | Yes |
| **Segment B — `spawn`** | `30..45 s` | `15 s` | Low oblique dolly across dense `10,000`-tree forest canopy with active `1,000` moving 3-segment creatures (`alpha-test/impostor/skinning load`) | Yes |
| **Segment C — `ecosystem`** | `45..60 s` | `15 s` | River gorge + coastal estuary pan showing `shore_sdf` foam, `hydro_flow`, terrain splatting, and creatures (`water + terrain + creatures`) | Yes |
| **Segment D — `massif_sweep`** | `60..75 s` | `15 s` | Fast camera pan + zoom across cliff talus (`Normal` / mip stress + temporal reprojection / disocclusion stress) | Yes |
| **Static Hold (`ecosystem`)** | `75..90 s` | `15 s` | Stationary camera at `ecosystem` (`jitter / temporal shimmer / memory drift` settle) | Yes |
| **Total Standard Run** | **`0..90 s`** | **`90 s`** | **`60 s` active tour (`15..75 s`) + `15 s` static (`75..90 s`) = `75 s` scored window** (or exact `60 s` ETW timed window `15..75 s` aligned with `PresentMon --delay 15 --timed 60`) | `15..75 s` primary (`3` repeats, report median run + spread) |
| **Dell Thermal Soak** | **`0..900 s`** | **`15 min`** | **Dell Iris Xe Low only:** Continuous loop of Segments A–D on AC power (`15..900 s` ETW + CSV), comparing **`min 1..2`** vs. **`min 13..15`** to measure thermal throttling | Diagnostic (`H-NR-01`); hard soak floor `>= 28.0 FPS` is **`PROPOSED — OWNER DECISION NR-D2`** |

---

## 2. Candidate Specifications & Required Audits

Each candidate renders `S-NR-FIXED` inside a **non-merged spike directory** (e.g., an external/unmerged spike branch outside default `anima-engine` crates so trunk CI gates are unaffected).

### 2.1 Candidate `(a)` — Custom `wgpu 30.0.2` Inside the Tauri v2 Window

- **Crate/API Alignment:** MUST pin **`wgpu = "30.0.2"`** and **`raw-window-handle = "0.6.2"`** matching `src-tauri/Cargo.lock` (`burn 0.22.1 -> cubecl-wgpu 0.11.4 -> wgpu 30.0.2` and `anima-worldgen gpu-bake`). Spike code verifies **zero duplicate `wgpu` major trees** when compiled alongside `burn/wgpu` (`ml-wgpu`).
- **Tauri Compositor Integration Tested:**
  - **Windows 11 DX12 (`wry 0.55.1` WebView2):** Test the **Child HWND `DxgiFromHwnd`** path (separate non-`WS_CLIPSIBLINGS` child `HWND` beneath `WRY_WEBVIEW` with WebView2 `DefaultBackgroundColor` alpha `= 0`), AND compare against **`WS_EX_NOREDIRECTIONBITMAP` + `IDCompositionVisual`** (`SurfaceTargetUnsafe::CompositionVisual`) during the Mốc 1 stability suite (marking research §1.1's Child-HWND claims as `[HYPOTHESIS TO TEST]`). If external **`dxcompiler.dll` + `dxil.dll >= v1.8.2502`** (latest `v1.9.2609`) are bundled alongside the Windows binary rather than `static-dxc` (`x86_64`), download/staging requires prior approval under **`PROPOSED — OWNER DECISION NR-D7`**.
  - **macOS Metal smoke (`WKWebView`, macOS packaging check):** Sibling `NSView` + `CAMetalLayer` beneath `WKWebView` using `drawsBackground = NO` and dynamic `presentsWithTransaction` only during live resize (documenting Apple's private-API policy vs. Developer ID notarization per research §1.1 `[2, 8]`).
- **Input Routing & Surface Lifecycle (`CurrentSurfaceTexture`):** Use batched DOM-First event forwarding (`pointer-events: none` background / `auto` HUD) for Mốc 1, implement all **7 branches** of `wgpu::CurrentSurfaceTexture` (`Success`, `Suboptimal`, `Timeout`, `Occluded`, `Outdated`, `Lost`, `Validation`), and test Mốc 1 stability: `50` minimize/restore cycles (**`PROPOSED — OWNER DECISION NR-D4`**; research §4 hypothesis vs. smaller `10`-cycle sanity check), 4-edge drag live resize, `100% <-> 150%` DPI transition, zero DWM black/transparent flickers, and working Windows Vietnamese TSF/IME input in the HUD text field.
- **Renderer Features Evaluated (`[HYPOTHESIS TO TEST]`):** Two-Phase Hi-Z occlusion culling + fixed `(Species, LOD)` MDI buckets (`atomicAdd` compute cull + single `multi_draw_indexed_indirect` matching `wgpu-hal` 30.0.2 Metal fallback constraint), hemi-octahedral `1024²` BC7/BC5/BC4 impostors, 4-layer weight-pruned terrain splatting + Top-2 Hex-Tiling (`720p -> 1080p` Low), Hillaire sky LUTs, and comparison of Lightweight WGSL TAAU + RCAS vs. AMD FSR 1 (`[HYPOTHESIS]`).

### 2.2 Candidate `(b)` — Newer Bevy (Native Render Graph + Upgrade Audit)

- **Version Pin:** Pin one specific released Bevy minor/patch version inside the isolated spike workspace on Day 1 (latest verified upstream release: **`v0.20.1`**, published `2026-10-28`, `MIT OR Apache-2.0` per `Cargo.toml`, `https://github.com/bevyengine/bevy/releases/tag/v0.20.1`; or an explicit Owner-confirmed `>= 0.15` pin — fetched by Cargo only during spike build, never floating git HEAD) and keep **every asset identical** (`S-NR-FIXED` tiles converted strictly without changing World v2 source data or dropping `SSIM >= 0.996`).
- **Windowing / Embedding Strategy Tested:** Measure Bevy standalone native window + minimal egui/HUD overlay **and** assess headless/embedded Tauri composition viability (because Tauri owns `tao` while Bevy owns `winit`, creating a known single-main-thread event-loop ownership conflict on macOS/Windows).
- **Mandatory Migration & Coexistence Audit Deliverable (Non-Optional):** In addition to FPS/GPU/RAM/VRAM numbers, candidate `(b)` MUST produce an empirical audit table answering:
  1. **`bevy_ecs 0.13.2 -> Bevy v0.20.1` (or pinned `>= 0.15`) blast radius:** Exact file/call-site diff estimate across `src-tauri/src/` (`Schedule`, `Query`, `SystemParam`, `Event`, `Commands`, `GS-4` zero-alloc tick path, and [`ADR-0008`](../../decisions/ADR-0008-world-past-and-time.md) deterministic replay / seeded RNG invariance).
  2. **`wgpu` conflict with `burn 0.22.1` (`ADR-0009`):** Whether newer Bevy pulls a different `wgpu` major version than `wgpu 30.0.2`, forcing two `wgpu` dependency trees (and duplicate native backends/shader compilers) or breaking `ml-wgpu` unless `burn` is upgraded simultaneously.
  3. **CI G2 & Licensing impact:** `cargo deny`, `THIRD_PARTY_LICENSES.txt`, binary size delta, clean `--release` compile time, and impact on [`CI G2`](../../decisions/ADR-0009-burn-022-flex-wgpu-upgrade.md) (`--no-default-features` exclusion of rendering crates from `anima-lab` / headless CI).

### 2.3 Candidate `(c)` — Godot 4 as a Separate Viewer Process

- **Version Pin:** Pin official **`Godot 4.7.2-stable`** (`MIT`, published `2026-08-18`, `https://github.com/godotengine/godot/releases/tag/4.7.2-stable`) binary & export templates at spike start (**`PROPOSED — OWNER DECISION NR-D7` — do not install until approved**).
- **Process Architecture Tested:** Two-process model per [Map v2 §4 P2](../requirements/2026-11-20-feature-map-v2.md) — headless Rust simulation emitter streams the `30 Hz` `3,000`-segment creature frame over local loopback IPC (`127.0.0.1` TCP/UDP or shared memory) to an exported standalone Godot 4 executable (`Forward+` DX12/Vulkan for High; `Forward+` and/or `Compatibility` for Low — whichever meets `req :109` & `SSIM` visual fidelity without crashes on Iris Xe).
- **Additional Telemetry Required for `(c)`:**
  - **Combined dual-process footprint:** Sum of `sim_emitter.exe` + `godot_viewer.exe` (never measuring Godot alone).
  - **IPC serialization + transport latency (`p50`, `p95` ms):** Wall-clock delta from Rust frame emission to Godot transform buffer upload, plus dropped/late tick count over the `90 s` tour.
  - **HUD & Packaging reality check:** Document how Anima's React/HTML UI (experiment runner, graphs, EvoTree, inspector) would communicate across Tauri <-> Godot, window docking/focus behavior, and macOS code-signing/notarization of an external sidecar `.app`/binary.

---

## 3. Machine Test Matrix, Prebuilt-Binary Rule & Quantitative Ceilings

### 3.1 Machine Allocation & Dell Prebuilt-Binary Workflow (`AGENT_RULES.md §1, §5`)

| Machine | Specs | Allowed Tiers | Candidates Measured | Build & Execution Rule |
|---|---|---|---|---|
| **Windows 1 (`DESKTOP-3EPV830`)** | Intel Core i5-14600KF (`14C/20T`), `47.8 GB` RAM, **NVIDIA RTX 5060 Ti (`16,311 MiB`, driver `621.24`)**, Windows 11 Pro `26300` | **High AND Low** | `(a)`, `(b)`, `(c)` | Builds all `--release` Windows `x86_64` spike packages (including the portable bundle for Dell); max **1 heavy Rust/GPU job at a time** (`CARGO_BUILD_JOBS=6`, check OxideSwarm worker idle state first per `AGENT_RULES.md §5`). |
| **Dell Laptop (`DESKTOP-IT8UKCD`)** | Dell Vostro 3530, Intel Core i7-1355U, `16 GB` shared RAM, **Intel Iris Xe Graphics (`0xA7A1`)**, plugged into AC power | **Low ONLY** | `(a)`, `(b)`, `(c)` | **MUST run ONLY from prebuilt portable binaries compiled on Windows 1 (`0` `cargo`, `0` `rustc`, `0` Godot editor builds on Dell)** (`MSG_20261123_084500`, `AGENT_RULES.md §5`). Pre-flight requires `tasklist \| findstr /i "cargo rustc"` empty, `>= 5 GB` free physical RAM, AC power connected, and SHA-256 verification of the prebuilt binary + `scene_bundle_sha256`. |
| **MacBook Air M5** | Apple M5 (`10C`, `32 GB`, fanless), macOS `arm64` | **Smoke / Packaging Only** | `(a)`, `(b)`, `(c)` | Used strictly to verify cross-platform compilation, Metal shader acceptance (`TIMESTAMP_QUERY` boundary check), Retina `2x` resize behavior, and `.app` bundle/signing steps for `§6` scoring (`CARGO_BUILD_JOBS=4 nice -n 10`). |

**Dell Portable Package Contract (`bin + dxcompiler.dll + dxil.dll + scene_bundle + run_spike.ps1`):** Windows 1 produces one self-contained folder per candidate (`dist_win_x64_<cand>/`) accompanied by `PACKAGE_SHA256SUMS.txt`. Copying via Hub/USB/local share onto Dell requires zero toolchain installs; Dell runs only the signed-off capture script against that exact binary hash.

### 3.2 Per-Pass GPU Frame Budgets (`Map v2 §3b` Authoritative)

Per [Map v2 Requirements §3b](../requirements/2026-11-20-feature-map-v2.md), these millisecond ceilings are the starting budgets against which P2 measures every candidate:

| System Pass | **Dell Iris Xe Low Budget (`33.3 ms` ceiling, `720p -> 1080p`)** | **RTX 5060 Ti High Budget (`16.6 ms` ceiling, `1080p`)** | Notes |
|---|---:|---:|---|
| Terrain | **`6.0 ms`** | **`3.0 ms`** | Splatting / height displacement / baked lighting eval |
| Vegetation (`10,000` trees) | **`6.0 ms`** | **`4.0 ms`** | GPU cull + LOD meshes + impostors (+ CSM on High) |
| Water | **`3.0 ms`** | **`2.0 ms`** | Lakes/rivers/ocean (`shore_sdf` + flow; SSR High only) |
| Sky & Atmosphere | **`1.5 ms`** | **`1.0 ms`** | Sky LUTs, distance fog (Low), aerial froxels (High) |
| Creatures (`1,000 × 3` seg) | **`5.0 ms`** | **`2.5 ms`** | GPU instanced body transforms + eyes/snout (+ blob shadow Low / CSM High) |
| Post & Upscale | **`4.0 ms`** | **`2.0 ms`** | `720p -> 1080p` TAAU/FSR (Low), SSAO + AgX + post (High) |
| Reserve (CPU / Sim / UI / Compositor) | **`7.8 ms`** | **`2.1 ms`** | Includes ECS/IPC tick ingest + WebView2/DWM HUD composite (`~2.0 ms` `[HYPOTHESIS]`) |
| **Total Frame Ceiling** | **`<= 33.33 ms` (`>= 30 FPS`)** | **`<= 16.67 ms` (`>= 60 FPS`)** | Thermal hypothesis on Dell: `<= 26.7 ms` sustained (`[HYPOTHESIS]`, see `H-NR-01`) |

*(Shadow Note: High-tier CSM pass time MUST be broken out cleanly inside/beside Vegetation + Terrain + Creatures or reported as its own `gpu_csm_ms` column so Low `0.00 ms` CSM compliance under `req :109` is mechanically auditable.)*

### 3.3 FPS, RAM, VRAM & Stability Ceilings (Accepted vs. Proposed `NR-D1..NR-D3`)

In the table below, ONLY the **median FPS** ceilings (`30 FPS` Dell Low, `60 FPS` RTX High) come from an accepted document ([Map v2 §1 `G-PERF`](../requirements/2026-11-20-feature-map-v2.md)). Every additional numeric floor or memory ceiling (`p5` floor, `15-min` soak floor, `G-MEM` limits) is a **`PROPOSED — OWNER DECISION NR-D<n>`** pending Owner approval before Stage 1 enforcement:

| Metric | **Dell Iris Xe — Low Ceiling** | **RTX 5060 Ti — Low (Ref.)** | **RTX 5060 Ti — High Ceiling** | Governance & Rationale / Source |
|---|---|---|---|---|
| **Median FPS (`60 s` tour)** | **`>= 30.0 FPS`** (`<= 33.33 ms`) | Diagnostic ref. (`>= 60.0 FPS` expected) | **`>= 60.0 FPS`** (`<= 16.67 ms`) | **ACCEPTED HARD GATE** (`Map v2 §1 G-PERF`) |
| **`p5` FPS (`1000 / P95 msBetweenPresents`)** | **`>= 25.0 FPS`** (`<= 40.00 ms`) | Reported verbatim | **`>= 50.0 FPS`** (`<= 20.00 ms`) | **`PROPOSED — OWNER DECISION NR-D1`** — `Map v2 G-PERF` requires reporting `p5` without a floor; `25/50` (`~83%` of median) bounds tail frame hitches (`Research §2.1`). Fallback if declined: report verbatim only. |
| **p1 FPS (`1000 / P99`, secondary stutters)** | Reported verbatim (flag `< 18 FPS`) | Reported verbatim | Reported verbatim (flag `< 35 FPS`) | Diagnostic risk log only (non-gating in Stage 1; recorded in `ADR-0011`) |
| **15-min Thermal Soak Median FPS (Dell)** | **`>= 28.0 FPS`** in min `13..15` (`>= 30.0` target w/ diagnostic DRS `>= 0.50×`) | N/A | N/A | **`PROPOSED — OWNER DECISION NR-D2`** — Quantifies `H-NR-01` `15 W` thermal throttle (`Research §2.1`); not in `Map v2`. Fallback if declined: diagnostic `15-min` delta only. |
| **Process-Tree RAM (Working Set / Commit Peak)** | **`<= 2,048 MiB`** (target `<= 1,536 MiB`, excl. shared GPU double-count) | Reported verbatim | **`<= 4,096 MiB`** (target `<= 3,072 MiB`) | **`PROPOSED — OWNER DECISION NR-D3`** (`G-MEM`) — Not in `Map v2`; sized from Dell `16 GB` shared host (`~11.5 GB` OS resident -> `>= 5 GB` free RAM check per `AGENT_RULES.md §5`). |
| **VRAM / Shared GPU Memory Ceiling** | **`<= 768 MiB` DXGI `Local/NonLocal` usage** (target `<= 512 MiB` on unified `16 GB` host) | Reported verbatim | **`<= 3,072 MiB` dedicated VRAM** (target `<= 2,048 MiB`) | **`PROPOSED — OWNER DECISION NR-D3`** (`G-MEM`) — Not in `Map v2`; protects Dell `16 GB` unified RAM and keeps `<= 19%` of RTX `16,311 MiB` VRAM. |
| **Post-Warmup Leak Drift (`15..90 s` & `2..15 min`)** | **`<= +2.0 MiB/min`** OLS slope on RAM & GPU mem | `<= +2.0 MiB/min` | `<= +2.0 MiB/min` | **`PROPOSED — OWNER DECISION NR-D3`** (`G-MEM`) — Bounds unbounded descriptor/buffer leaks; fallback if declined: log OLS slope in `ADR-0011`. |

---

## 4. Exact Capture Tools, Metric Definitions & CLI Commands

### 4.1 Observed Local Tooling Inventory & Proposed Installs (`OWNER DECISION NR-D7`)

Direct CLI inspection on Windows 1 (`2026-11-23`) verified:
- **Already preinstalled:** `C:\Windows\System32\typeperf.exe`, `C:\Windows\System32\dxdiag.exe`, `nvidia-smi` (RTX 5060 Ti driver `621.24`), `cargo`, `rustup`, `cmake`, `python 3.11.9`.
- **Not installed — `PROPOSED — OWNER DECISION NR-D7` (External Tool Installs; DO NOT DOWNLOAD OR INSTALL ANY TOOL UNTIL OWNER APPROVES):** Every standalone tool below stays untracked outside git (`tools/` ignored or system install) and **MUST NOT** be installed prior to Owner sign-off on `NR-D7`:

| Tool | Target Machine(s) | Exact Version | Source URL | Licence | Purpose / Necessity |
|---|---|---|---|---|---|
| **Intel `PresentMon`** (`PresentMon.exe`) | Windows 1 + Dell *(copied in portable dir)* | **`v2.6.0`** (`2026-09-21`) | `https://github.com/GameTechDev/PresentMon/releases/tag/v2.6.0` | **MIT** | Mandatory ETW swapchain ground-truth FPS (`median`, `p5`, `p1`) across all 3 candidates |
| **`RenderDoc`** (`renderdoccmd.exe`) | Windows 1 only | **`v1.47`** (`2026-10-31`) | `https://github.com/baldurk/renderdoc/releases/tag/v1.47` | **MIT** | Frame-120 DX12/Vulkan pass audit (`0` CSM on Low `req :109`, BC7/BC5 formats, draw counts) |
| **Microsoft `PIX` on Windows** (`pixtool.exe`) | Windows 1 only *(optional)* | **`PIX 2611.19`** *(latest on download page)* | `https://devblogs.microsoft.com/pix/download/` | **Proprietary Microsoft Software License Terms (EULA)** *(installer-bundled; optional `WinPixEventRuntime` MIT)* | Only required if comparing `CompositionVisual` mode where RenderDoc cannot hook DXGI swapchain (`Research §1.1`) |
| **`Godot 4` Engine + Export Templates** | Windows 1 only *(Dell runs exported `.exe`/`.pck`)* | **`4.7.2-stable`** (`2026-08-18`) | `https://github.com/godotengine/godot/releases/tag/4.7.2-stable` | **MIT** | Required strictly to build and export Candidate `(c)` standalone viewer bundle |
| **DirectX Shader Compiler (`dxcompiler.dll`, `dxil.dll`)** *(if not using `static-dxc`)* | Windows 1 + Dell *(in portable `dist_win_x64_a/`)* | **`v1.9.2609`** *(`>= v1.8.2502` min per `wgpu 30`)* | `https://github.com/microsoft/DirectXShaderCompiler/releases/tag/v1.9.2609` | **LLVM Release License (University of Illinois / NCSA Open Source)** | Needed on Windows DX12 if Candidate `(a)` links runtime DXC DLLs rather than `static-dxc` |

### 4.2 Swapchain Ground-Truth FPS (`p5`, `median`, `p1`) — `PresentMon` (ETW DXGI/DWM)

All three candidates MUST be measured externally at the OS DXGI/DWM present boundary using Intel **`PresentMon`** so swapchain queue, compositor overhead, and Godot vs. Tauri differences are captured identically:

```powershell
# Run per (candidate, tier) during the scripted 90s tour (captures seconds 15..75 after 15s warmup)
# On Dell, run from the copied prebuilt package directory against the prebuilt executable name.
$bin = "anima-spike-wgpu.exe"   # or anima-spike-bevy.exe / anima-spike-godot.exe
$outCsv = "artifacts/nr-spike/<host>_<cand>_<tier>_presentmon_r1.csv"
.\tools\PresentMon.exe `
  --process_name $bin `
  --output_file $outCsv `
  --delay 15 `
  --timed 60 `
  --stop_existing_session `
  --no_console_stats
```

For the Dell `15-min` soak run, pass `--delay 15 --timed 885` so `t = 15..900 s` is captured continuously into one CSV.

**Exact Metric Formulas (computed by deterministic Python reduction from `msBetweenPresents` in the PresentMon CSV):**
- Filter strictly to presented swapchain rows (`PresentFlags` valid / non-dropped; also record dropped-frame count separately).
- **`median_fps`** $= 1000 / \mathrm{P}_{50}(\text{msBetweenPresents})$.
- **`p5_fps`** (5th-percentile FPS = 95th-percentile slow frame time, matching Map v2 §1 `G-PERF`) $= 1000 / \mathrm{P}_{95}(\text{msBetweenPresents})$.
- **`p1_fps`** (1% low FPS = 99th-percentile slow frame time) $= 1000 / \mathrm{P}_{99}(\text{msBetweenPresents})$.
- **`stutter_ratio`** $= \text{fraction of frames with } \text{msBetweenPresents} > 2 \times \mathrm{P}_{50}$.

*(Note: Using an engine-internal frame timer alone is forbidden because it hides swapchain `Present` stalls, WebView2 compositor waits, and DXGI queuing; internal frametimes are cross-checked against `PresentMon` rather than replacing it.)*

### 4.3 Per-Pass GPU Milliseconds — In-Engine Timestamp Queries + Single-Frame External Audit

1. **In-Engine Pass-Boundary GPU Timers (Mandatory in all 3 spike binaries):**
   - **Candidates `(a)` & `(b)`:** Request `wgpu::Features::TIMESTAMP_QUERY` and attach **pass-boundary** `RenderPassTimestampWrites` / `ComputePassTimestampWrites` (`beginning_of_pass_write_index` / `end_of_pass_write_index`, **never** calling `write_timestamp` mid-pass so identical pass-boundary code works unchanged on macOS Metal `[60]`). Resolve timestamp ticks via `queue.get_timestamp_period()` (`ns -> ms`) into a per-frame ring buffer logged to `<host>_<cand>_<tier>_passes_r1.csv`.
   - **Candidate `(c)` (Godot 4):** Enable `RenderingServer.viewport_set_measure_render_time(vp, true)` + GPU profiler pass breakdown mapped cleanly onto the 6 canonical pass buckets (`terrain`, `vegetation`, `water`, `sky_atm`, `creatures`, `post_upscale`) plus `csm_shadow` (asserted `0.00` on Low!) and `ipc_ingest_ms`, written to the identical CSV schema:
     `frame_idx,sim_or_ipc_ms,cpu_frame_ms,gpu_csm_ms,gpu_terrain_ms,gpu_vegetation_ms,gpu_water_ms,gpu_sky_atm_ms,gpu_creatures_ms,gpu_post_upscale_ms,gpu_total_ms,draw_calls,triangles,render_scale`
2. **External Single-Frame GPU Pass Audit on Windows 1 (RTX 5060 Ti, Frame `120` at `ecosystem`):**
   - For Child HWND `DxgiFromHwnd` (`(a)`) and DX12/Vulkan (`(b)`, `(c)`):
     ```powershell
     "C:\Program Files\RenderDoc\renderdoccmd.exe" capture `
       --working-dir $pkgDir `
       --capture-file "artifacts/nr-spike/rtx_<cand>_<tier>_frame120.rdc" `
       "$pkgDir\$bin" --capture-frame 120 --tier <low|high> --scene "$sceneDir"
     ```
   - For Candidate `(a)` `WS_EX_NOREDIRECTIONBITMAP` + `CompositionVisual` mode (where research §1.1 notes `DxgiFromVisual` bypasses RenderDoc), capture via Microsoft PIX:
     ```powershell
     & "C:\Program Files\Microsoft PIX\pixtool.exe" launch "$pkgDir\$bin" `
       --command-line="--capture-frame 120 --tier <low|high> --scene `"$sceneDir`"" `
       take-capture --frames=1 save-capture "artifacts/nr-spike/rtx_a_compvisual_<tier>_frame120.wpix"
     ```
   - Purpose: Confirm **0 CSM shadow render passes** on Low (`req :109`), verify active compressed texture formats (`BC7`/`BC5` vs `RGBA8`), and verify draw call & triangle counts match the telemetry CSV.

### 4.4 Process-Tree RAM & VRAM Sampling (`typeperf.exe`, `nvidia-smi`, & DXGI)

1. **In-Engine DXGI Adapter Memory Query (`QueryVideoMemoryInfo`):** Every spike binary polls `IDXGIAdapter3::QueryVideoMemoryInfo` (exposed directly or via thin `windows` crate call in the spike harness) every `1 s` on Windows, logging `Local` (`Budget` & `CurrentUsage` — dedicated VRAM on RTX 5060 Ti) and `NonLocal` (shared system-memory GPU usage on Intel Iris Xe).
2. **External OS Process-Tree RAM & GPU Usage (Windows 1 & Dell):**
   Built-in Windows `typeperf.exe` captures 1 Hz CPU, process Working Set / Private Bytes across the full process tree (capturing WebView2 `msedgewebview2.exe` child renderers for `(a)` and `sim_emitter.exe` + `godot.exe` for `(c)`):
   ```powershell
   # Run concurrently with the 90s tour (1 Hz samples; covers Tauri + WebView2 OR Sim + Godot PID trees)
   typeperf -si 1 -sc 90 `
     "\Memory\Available MBytes" `
     "\Process($procPattern)\Working Set - Private" `
     "\Process($procPattern)\Private Bytes" `
     "\GPU Process Memory(*)\Local Usage" `
     "\GPU Process Memory(*)\Non-Local Usage" `
     -f CSV -o "artifacts/nr-spike/<host>_<cand>_<tier>_sysmem_r1.csv"
   ```
   *(Helper script sums exact child PIDs belonging to the spike process tree so browser tabs owned by the desktop user are never mixed in.)*
3. **Dedicated VRAM Cross-Check on Windows 1 (RTX 5060 Ti only):**
   ```powershell
   nvidia-smi --query-gpu=timestamp,utilization.gpu,clocks.gr,temperature.gpu,power.draw,memory.used `
     --format=csv,nounits -lms 500 > "artifacts/nr-spike/rtx_<cand>_<tier>_nvml_r1.csv"
   ```
   *(Note: Run via `cmd /c` or UTF-8 redirect in scripts so CSV stays UTF-8.)*

### 4.5 Visual Verification & Pre-Flight Bake Gate (`PSNR`, `SSIM`, `LDR-FLIP`, Canonical Stills)

1. **Pre-Flight Asset Gate:** Before running candidate `(a)`, `(b)`, or `(c)`, record the input tile pack verification (`Block PSNR >= 42.0 dB`, `SSIM >= 0.996` against uncompressed `f32`/`RGBA8` master layers) inside `scene_bundle_manifest.json`.
2. **Canonical Stills Capture:** Capture 4 uncompressed RGBA PNG screenshots per `(candidate, tier)` at fixed scene clock `11:00`, frame `120` (`overview`, `spawn`, `ecosystem`, `massif_ridge`), saving raw images for side-by-side G-REF visual inspection and `SSIM` / `LDR-FLIP` comparison (optional Low-vs-supersampled reference comparison for TAAU vs. FSR 1).

---

## 5. Research-Note Hypothesis Register (`[HYPOTHESIS TO TEST]`)

Every numerical estimate and architectural claim extracted from [`NATIVE_RENDERER_WGPU_TAURI_BAKING_RESEARCH_2026-11-22.md`](../../research/NATIVE_RENDERER_WGPU_TAURI_BAKING_RESEARCH_2026-11-22.md) is registered here explicitly as an **unverified hypothesis to test (`[HYPOTHESIS TO TEST]`)**, not an established fact:

| ID | Research-Note Claim / Estimate | Classification | Exact Verification in `NR-SPIKE` | Conflict / Protocol Rule |
|---|---|---|---|---|
| **`H-NR-01`** | Iris Xe 15 W thermal throttling cuts sustained budget from `33.33 ms` down to **`<= 26.7 ms` (20% margin)**; OEM single-channel RAM (`25.6 GB/s` vs `51.2+`) forces DRS `0.59×`/`0.50×` (research §2.1) | **`[HYPOTHESIS TO TEST]`** | Dell `15-min` thermal soak comparing `min 1..2` vs `min 13..15` `p5` & median FPS, logged alongside Dell exact RAM channel config from `wmic` / PowerShell `Get-CimInstance Win32_PhysicalMemory` | `Map v2 §3b` `33.3 ms` remains the hard gate; `26.7 ms` is checked diagnostically |
| **`H-NR-02`** | `~4.2 ms` CSM 2–3 cascade shadow cost on Iris Xe Low (research §1 & §2.1) | **`[HYPOTHESIS TO TEST]`** | **Excluded from Low**; CSM is enabled ONLY on High (`RTX`), while Low verifies `gpu_csm_ms == 0.00` | **Resolved in favor of `Map v2 §3b` line 109 (`req :109`)**: no shadow map on Low |
| **`H-NR-03`** | `~2.0 ms` DWM + WebView2 `1080p` compositing cost (research §2.1) | **`[HYPOTHESIS TO TEST]`** | Measured on Dell & RTX by comparing PresentMon wall-clock frame time vs. in-engine `gpu_total_ms` + isolated HUD toggle (`opaque` vs `transparent` WebView2 vs Godot native window) | Included inside `7.8 ms` (Low) / `2.1 ms` (High) reserve budget |
| **`H-NR-04`** | Hillaire (2020) 4 sky LUTs cost **`~0.5 ms`** total (`< 0.55 MB` VRAM, `~0.27 MB` `Rg11b10Ufloat` + `32×32×16` froxel) on Iris Xe (research §2.2) | **`[HYPOTHESIS TO TEST]`** | Measured via `gpu_sky_atm_ms` timestamp pass on Dell Low (`1.5 ms` budget) & RTX High (`1.0 ms` budget) | Pass budget comes from `Map v2 §3b`, not research estimate |
| **`H-NR-05`** | 4-layer weight-pruned splatting + Top-2 Hex-Tiling uses ~`5.4 taps/px` (`~4.3 MB/frame` @ `720p`) vs. RVT `3 taps/px` (`~2.4 MB/frame`), only paying off above 4 layers (research §2.4) | **`[HYPOTHESIS TO TEST]`** | Measure `gpu_terrain_ms` and Iris Xe `NonLocal` bandwidth / frame-time delta against the `6.0 ms` (`Map v2 §3b`) terrain ceiling | Keep `<= 4` splat layers per `Map v2 §3b.3` for P2 |
| **`H-NR-06`** | Lightweight WGSL TAAU + RCAS (~**`2.8 ms`**, `50.1 MB` BW) outperforms FSR 1 (~**`1.4 ms`**, `30.8 MB` BW) on foliage shimmer while FSR 2.2 (~**`4.8 ms`**, `69.4 MB` BW) exceeds Iris Xe Low `4.0 ms` upscale budget (research §2.5; Weak Source `[9, 95]`) | **`[HYPOTHESIS TO TEST]`** | Run A/B toggle inside Candidate `(a)` on Dell Low across Segments B (`spawn`) & D (`massif_sweep`): measure `gpu_post_upscale_ms`, `p5` FPS, and foliage temporal shimmer (`LDR-FLIP` vs supersampled still) | `4.0 ms` (`Map v2 §3b`) governs; FSR 1 is fallback if TAAU exceeds `4.0 ms` |
| **`H-NR-07`** | Two-Phase Hi-Z SPD + Fixed-Bucket MDI (`48–96` buckets) + `1024²` hemi-octahedral BC7/BC5/BC4 impostors (`3.33 MB/species`, stochastic single-view + Hashed Alpha) fits cleanly within `6.0 ms` vegetation budget (research §2.3) | **`[HYPOTHESIS TO TEST]`** | Measure `gpu_vegetation_ms`, DXGI VRAM footprint, and visual foliage fidelity across all 3 candidates (`10,000` trees) | Must obey `§3b` `6.0 ms` (Low) / `4.0 ms` (High) |
| **`H-NR-08`** | Headless `wgpu` readback -> IPC -> `<canvas>` adds **`~300 ms`** latency on weak hardware, and `wry 0.55.1` + WebView2 149 shows `256 px` stale compositor tile regression (research §0.2 & §1.1) | **`[HYPOTHESIS TO TEST]`** | Never used as primary architecture; optionally log whether `wry 0.55.1` exhibits stale compositor artifacts during Mốc 1 resize/DOM updates | Requires local reproduction check, not assumed |
| **`H-NR-09`** | Windows DX12 Child HWND `DxgiFromHwnd` vs `WS_EX_NOREDIRECTIONBITMAP` + `CompositionVisual` (1 px border caveat), DOM-First event routing, and macOS `drawsBackground = NO` + dynamic `presentsWithTransaction` give `0`-panic, `0`-black-frame Tauri composition (research §1.1–1.3; Weak Source `[2]`) | **`[HYPOTHESIS TO TEST]`** | Executed in Candidate `(a)` Mốc 1 torture test (`50` minimize/restore cycles per `NR-D4` if approved, 4-edge drag, `100% <-> 150%` DPI, Vietnamese TSF/IME typing, macOS Retina resize smoke) | Any DWM glitch, IME break, or `panic!` drops Candidate `(a)` integration score or fails `G-STAB` |
| **`H-NR-10`** | FXC compiles `2.3–7.2 s` and fails `SHADER_F16`/compute (`12/224` kernels, Weak Source `[58, 59]`), requiring bundled `dxcompiler.dll` + `dxil.dll >= v1.8.2502` and small compute submits to avoid WDDM TDR (research §1.4) | **`[HYPOTHESIS TO TEST]`** | Measure cold shader/pipeline compile time (`0..15 s` warmup window) on Dell & RTX with DXC (`NR-D7` if dynamic DLLs) + `PipelineCache`, plus `SHADER_F16` on/off delta on Iris Xe | Confirms cold-start UX and TDR resilience |
| **`H-NR-11`** | Basis/UASTC CPU transcode takes **`~160 ms`** on Ryzen 5 3600 (vs `0 ms` Native BC7/BC5), `mipple`/`bc7e` cuts BD-rate `-31.5%`/`-26.1%`, and `SSIM >= 0.98` suffices (research §3.1, §3.4; Weak Source `[112]`) | **`[HYPOTHESIS TO TEST]`** | Measure actual tile-decode latency during Dell/RTX scene load; **strictly enforce `WV2-D` `SSIM >= 0.996` and `Block PSNR >= 42 dB`** (do NOT adopt `0.98`) | Changing KTX2 encoding from `UASTC + bc7enc_rdo` requires separate `WV2-D` amendment after P2 |

---

## 6. Scoring Rubric: Integration Cost & Windows/macOS Packaging (`PROPOSED — OWNER DECISION NR-D5`)

Candidates that clear Stage 1 (`§7.1`) are scored on a **`PROPOSED — OWNER DECISION NR-D5`** `0.00–5.00` weighted rubric splitting **Performance Headroom (`35%`: `15/10/10`)**, **Integration Cost (`35%`: `12/13/10`)**, and **Windows/macOS Packaging & Delivery (`30%`: `10/10/10`)**. *(Rationale & source: Balances [`Map v2 §4 P2`](../requirements/2026-11-20-feature-map-v2.md) FPS/GPU, integration cost, and cross-OS delivery goals rather than weighting FPS alone; not specified in `Map v2`. Anchor thresholds in `A1..A3` reference `NR-D1..NR-D3` if approved, or report headroom qualitatively against accepted `30/60 FPS` gates otherwise.)*

| Dimension | Weight (`NR-D5`) | Anchor `0` (Unacceptable) | Anchor `2` (High Friction) | Anchor `3` (Workable w/ Effort) | Anchor `5` (Clean Fit) |
|---|---:|---|---|---|---|
| **A1. Dell Iris Xe Low FPS & Thermal Headroom (`p5`, median, 15-min soak)** | **15%** | Fails accepted `30` med (or approved `NR-D1`/`D2` floor) | `30.0`–`31.9 FPS` med with DRS `< 0.67×` or `p5` `25–27` | `>= 32 FPS` med, `p5 >= 28 FPS`, `<= 1` budget pass barely over | `>= 36 FPS` med (`<= 27 ms`), `p5 >= 32 FPS`, all 6 GPU passes inside `§3b`, zero soak drop |
| **A2. RTX 5060 Ti High FPS & Per-Pass Headroom (`p5`, median, CSM/SSR)** | **10%** | Fails accepted `60` med (or approved `NR-D1` `p5` floor) | `60–65 FPS` med or `1–2` passes exceed `§3b` ms | `65–85 FPS` med, `p5 >= 58 FPS`, passes inside `§3b` | `>= 90 FPS` med, `p5 >= 75 FPS`, wide margin under `16.6 ms` |
| **A3. Combined RAM, VRAM & Leak Margin** | **10%** | Exceeds approved `NR-D3` limit or leaks uncontrollably | Within `10%` of `NR-D3` (`1.85–2.05 GiB` Dell) | `< 80%` of `NR-D3` (`<= 1.6 GiB` Dell, `<= 2.5 GiB` RTX), `<= 0.5 MiB/min` drift | `< 65%` of `NR-D3`, flat `0 MiB/min` drift, fast scene load `< 2 s` |
| **B1. Simulation Coupling & Zero-IPC Parity (`bevy_ecs 0.13.2` / `GS-4` / `ADR-0008`)** | **12%** | Breaks `GS-4` zero-alloc or `ADR-0008` determinism | External process IPC with measurable `> 33 ms` lag, stale state, or dual lifecycle | External IPC `< 16 ms` cleanly bounded, OR minor ECS adapter layer | Same-process direct read of `bevy_ecs` world (`0` serialization, `0` IPC tick jitter) |
| **B2. Upgrade Blast Radius, `wgpu` Alignment (`burn 0.22.1`) & CI G2 / Licensing** | **13%** | GPL/copyleft dep or breaks CI G2 / headless builds | Large `bevy_ecs 0.13 -> v0.20.1` migration across `src-tauri/` + forces two `wgpu` trees or breaks `ml-wgpu` | Moderate feature-gated additions inside isolated module; `cargo deny` clean | Pins exact `Cargo.lock` `wgpu 30.0.2` (`0` duplicate trees), `0` `ecs` migration, G2 & `cargo deny` green |
| **B3. HUD, Input Routing, Focus & TSF/IME Compositing Maintainability** | **10%** | Severe flicker, broken IME, or dead mouse picking | Dual windows fighting Alt-Tab/fullscreen or fragile native pointer hooks across OSes | Workable DOM-First or sidecar sync with documented Win32/AppKit quirks | Single unified window, intact React/TypeScript HUD, `0` DWM black frames, intact Vietnamese IME |
| **C1. Windows Packaging (NSIS/MSI Installer, `dxcompiler.dll`, Sidecars, Edge WebView2)** | **10%** | Installer broken or misses signed/unsigned runtime deps | Must bundle & supervise second `.exe` sidecar / separate runtime window (`~100+ MB`) | Bundles explicit `dxcompiler.dll` + `dxil.dll` alongside `anima-engine.exe` in Tauri NSIS/MSI cleanly | Standard `tauri:build` single executable + verified DLL/tile staging, `< +15 MB` binary delta |
| **C2. macOS Packaging (`.app` Bundle, Universal/arm64, Developer ID Signing, MAS Disclosure)** | **10%** | Fails macOS arm64 Metal build or cannot notarize | Sidecar `.app` signing/Entitlements complexity OR hard-blocks future Mac App Store with no fallback | Developer ID notarization passes cleanly (`drawsBackground = NO` documented as non-MAS) or clean public API | Developer ID + notarization + CI macOS runner green with public/documented APIs |
| **C3. CI Determinism, Prebuilt-Artifact Portability & Phase P3 Velocity** | **10%** | Fails prebuilt Dell run or non-reproducible build | Custom export templates (`Godot`) outside Cargo or brittle cross-OS shader CI | Standard Rust workspace + reproducible headless/shader CI checks | Pure Cargo build (`tauri:build`), portable Dell test binary, cleanest path to P3 (`R1..R6`) |

---

## 7. Decision Rule for `ADR-0011`

When `NR-SPIKE` measurements complete on both **Windows 1 (`High + Low`)** and **Dell Vostro 3530 (`Low`, prebuilt binary)** — plus macOS packaging smoke check — `ADR-0011` applies a strict **two-stage rule** (no post-hoc threshold relaxation):

### 7.1 Stage 1 — Hard Binary Gates (Accepted Gates + Approved `NR-D1..NR-D4` Only)

> **Stage 1 Gating Authority Rule:** Stage 1 enforces **ONLY** the **ACCEPTED** contract gates from [`Map v2`](../requirements/2026-11-20-feature-map-v2.md) and [`World v2 Design (`WV2-D`)`](../design/2026-11-20-world-v2-design.md) **PLUS** whichever `PROPOSED — OWNER DECISION NR-D1..NR-D4` thresholds the Owner explicitly approves before execution. Any `NR-D<n>` threshold not approved by the Owner is still measured and reported verbatim in `ADR-0011` as diagnostic context, never used for binary disqualification.

A candidate **PASSES** Stage 1 only if ALL active hard gates hold:

1. **`G-PERF-LOW` (Dell Iris Xe Prebuilt Binary):**
   - **ACCEPTED (`Map v2 §1 G-PERF`, `§3b`):** `60 s` Low (`no CSM`, `req :109`) median `>= 30.0 FPS` (`<= 33.33 ms`) AND `p5` FPS measured and reported verbatim (vs. `§3b` `6/6/3/1.5/5/4/7.8 ms` pass budgets).
   - **`PROPOSED — OWNER DECISION NR-D1` (if approved):** `p5 >= 25.0 FPS` (`<= 40.00 ms`). *(Rationale: bounds stutter tails.)*
   - **`PROPOSED — OWNER DECISION NR-D2` (if approved):** `15-min` soak `min 13..15` median `>= 28.0 FPS`. *(Rationale: bounds `15 W` thermal throttle.)*
2. **`G-PERF-HIGH` (RTX 5060 Ti):**
   - **ACCEPTED (`Map v2 §1 G-PERF`, `§3b`):** `60 s` High (`3-cascade CSM + SSR + volumetrics`) median `>= 60.0 FPS` (`<= 16.67 ms`) AND `p5` reported verbatim (vs. `§3b` `3/4/2/1/2.5/2/2.1 ms` pass budgets).
   - **`PROPOSED — OWNER DECISION NR-D1` (if approved):** `p5 >= 50.0 FPS` (`<= 20.00 ms`).
3. **`G-MEM` (RAM, VRAM & Leak — `PROPOSED — OWNER DECISION NR-D3` if approved; diagnostic otherwise):** Peak process-tree RAM `<= 2,048 MiB` & GPU mem `<= 768 MiB` on Dell Low; RAM `<= 4,096 MiB` & VRAM `<= 3,072 MiB` on RTX High; post-warmup drift `<= +2.0 MiB/min`.
4. **`G-STAB` (Zero Crashes / Panic-Free Lifecycle):**
   - **ACCEPTED baseline:** `0` panics, `0` WDDM TDR / `Device(Lost)`, `0` DWM permanent black frames, live resize works cleanly, intact Vietnamese TSF/IME text input.
   - **`PROPOSED — OWNER DECISION NR-D4` (if approved):** Must clear `50/50` minimize/restore + resize cycles cleanly (`Research §4 Mốc 1` stress count; otherwise verified via smaller sanity check).
5. **`G-QUAL` (Fidelity & Honesty — ACCEPTED `Map v2 §1, §6, req :109/:111`, `WV2-D`):** Renders all `10,000` trees and `1,000 × 3` moving creatures without count cheating; clears input `Block PSNR >= 42 dB, SSIM >= 0.996`; obeys `req :109` (`0` CSM passes on Low); keeps `§6` decorative toggle and visually cohesive Low/High canonical stills.
6. **`G-POLICY` (`G-SIM` + G2 + License — ACCEPTED):** Preserves `GS-4` zero-alloc sim ticks, `ADR-0008` determinism, `sim_world_identity`, `MIT OR Apache-2.0` / compatible license policy (`QD-ASSETS` CC0), and leaves `--no-default-features` CI builds free of graphics crates (`G2`).

### 7.2 Stage 2 — Weighted Selection, Margin & Close-Call Escalation (`PROPOSED — OWNER DECISION NR-D5 & NR-D6`)

For all Stage 1 survivors, compute the `NR-D5` weighted score $\text{Score}(c) = \sum_k w_k \cdot s_{c,k} \in [0.00, 5.00]$ (if the Owner approves `NR-D5` weights; otherwise present the unweighted multi-criteria table to the Owner):

1. **Zero survivors (`0` pass Stage 1):** **Stop — do NOT weaken accepted `G-PERF` (`30/60 FPS`) or `SSIM 0.996` silently.** Keep Three.js ([`WV2-6`](../design/2026-11-20-world-v2-design.md)) as the active desktop viewer and escalate a diagnostic bottleneck report to the Owner proposing scoped engineering options (e.g., lowered asset LOD/density cap on Low, FSR 1 fallback, or deferred P3).
2. **One survivor (`1` passes Stage 1):** Adopt that candidate in `ADR-0011` provided its lowest rubric sub-score `min(s_{c,k}) >= 2.0` (**`PROPOSED — OWNER DECISION NR-D5`**; rationale: guards against critical integration/packaging regressions; if any sub-score `< 2.0`, log an explicit mitigation condition for Owner sign-off).
3. **Multiple survivors (`>= 2` pass Stage 1) — `PROPOSED — OWNER DECISION NR-D6`:**
   - **Clear winner:** Top candidate leads runner-up by **`>= 0.35` weighted points** (`7%` of `5.00` scale — prevents deciding on subjective noise; not in `Map v2`) AND has `min(s_{top,k}) >= 2.0` (`NR-D5`) $\implies$ recommend top candidate cleanly for `ADR-0011`.
   - **Close call (`< 0.35` margin, or top candidate has a sub-score `< 2.0`):** Escalate both finalists with full evidence tables to the Owner applying this proposed **3-step tie-break hierarchy** (`NR-D6`; fallback: direct Owner choice across survivors):
     1. *Priority 1 (Iris Xe Headroom):* Higher sustained `15-min` Dell Iris Xe **`p5` FPS** (headroom above accepted `30.0 FPS` median / `NR-D1` floor under `req :109` Low).
     2. *Priority 2 (Simulation & Burn Safety):* Same-process zero-IPC rendering without requiring an incompatible `bevy_ecs 0.13.2` rewrite or breaking `burn 0.22.1` / `wgpu 30.0.2` (`ADR-0009`).
     3. *Priority 3 (Single-Bundle Portability):* Simpler single-binary Windows NSIS + macOS `.app` Developer ID packaging without a separate runtime viewer sidecar.

---

## 8. Spike Execution Sequence & Protocol Boundaries

### 8.1 Recommended Spike Execution Sequence (Only After Owner Decisions `NR-D1..NR-D7`)

1. **Obtain Owner Decisions & Setup Tools (`NR-D7` first):** Freeze one `4096²` World v2 scene bundle (`scene_bundle_sha256`, verifying `Block PSNR >= 42 dB, SSIM >= 0.996`) + `10,000`-tree placement + `30 s` `1,000 × 3` creature stream; install ONLY the Owner-approved `NR-D7` standalone tools (`PresentMon v2.6.0`, `RenderDoc v1.47`, optional `PIX`, `Godot 4.7.2-stable`, optional DXC `v1.9.2609` DLLs) locally on Windows 1 (untracked).
2. **Build & Run on Windows 1 (`RTX High + Low`):** Implement isolated non-merged spikes for `(a)`, `(b)`, `(c)`, execute Mốc 1 stability + `3 × 90 s` timed captures per `(candidate, tier)` + Frame-120 RenderDoc/PIX pass audit, and export `dist_win_x64_<cand>/` + `PACKAGE_SHA256SUMS.txt`.
3. **Run Prebuilt Binaries on Dell (`Iris Xe Low Only`):** Copy verified `dist_win_x64_<cand>/` bundles to Dell (`0` Cargo/Godot compilation), run pre-flight machine check (`0` `cargo`/`rustc`, `>= 5 GB` free RAM, AC plugged), execute `3 × 90 s` Low captures + `15-min` thermal soak per candidate.
4. **macOS M5 Smoke & Packaging Check:** Compile/package check on `arm64` Metal (focusing on Mốc 1 Retina `2x` resize + `.app` notarization audit).
5. **Compute Rubric & Draft `ADR-0011`:** Fill exact measured numbers (testing `H-NR-01..H-NR-11`, applying Stage 1 accepted gates + Owner-approved `NR-D1..NR-D6`), merge **only** the benchmark report note + `ADR-0011` (`0` spike code merged per `Map v2 §4 P2`).

### 8.2 Explicitly Not Done by This Protocol PR

1. `0` lines of Rust, WGSL, TypeScript, GDScript, or Cargo configuration modified or added.
2. `0` external binaries (`PresentMon`, `RenderDoc`, `PIX`, `Godot 4`, `DXC`) downloaded, installed, or committed (`NR-D7` remains unexecuted pending Owner approval).
3. `0` benchmarks executed on Windows 1, Dell Iris Xe, or MacBook Air M5.
4. `ADR-0011` is not authored yet; it follows strictly from executing this approved protocol.

---

## 9. OWNER DECISION Summary (`NR-D1..NR-D7`)

All items below go beyond accepted [`Map v2 Requirements`](../requirements/2026-11-20-feature-map-v2.md) and [`World v2 Design`](../design/2026-11-20-world-v2-design.md). Stage 1 enforces ONLY accepted gates plus the `NR-D<n>` rows approved by the Owner, and **zero external tools are installed until `NR-D7` is approved**:

| Decision ID | Topic | Proposed Value / Action | Rationale & Source | Fallback if Declined / Deferred | Status |
|---|---|---|---|---|---|
| **`NR-D1`** | `p5` FPS hard floor (`G-PERF-LOW` / `HIGH`) | Dell Low `p5 >= 25.0 FPS` (`<= 40.0 ms`); RTX High `p5 >= 50.0 FPS` (`<= 20.0 ms`) | `Map v2 G-PERF` requires reporting `p5` without a floor; `~83%` of `30/60` median bounds tail stutters (`Research §2.1`) | Report `p5` verbatim as diagnostic metric; enforce median `30/60 FPS` only | `proposed — awaiting Owner decision` |
| **`NR-D2`** | `15-min` Dell thermal soak floor | `min 13..15` median `>= 28.0 FPS` on Dell Low | Bounds `H-NR-01` `15 W` sustained TDP throttle (`Research §2.1`); not in `Map v2` | Log `15-min` thermal drop diagnostically without Stage 1 disqualification | `proposed — awaiting Owner decision` |
| **`NR-D3`** | `G-MEM` RAM, VRAM & leak ceilings | Dell Low RAM `<= 2,048 MiB` / GPU `<= 768 MiB`; RTX High RAM `<= 4,096 MiB` / VRAM `<= 3,072 MiB`; OLS drift `<= +2.0 MiB/min` | Sized from Dell `16 GB` shared host (`AGENT_RULES.md §5`) & RTX `16 GB` VRAM; not in `Map v2` | Record RAM, VRAM, and OLS drift verbatim for `ADR-0011` without hard DQ | `proposed — awaiting Owner decision` |
| **`NR-D4`** | `G-STAB` minimize / restore torture cycles | **`50/50`** clean minimize/restore + 4-edge resize + DPI cycles | Adopted from `Research §4 Mốc 1` stress hypothesis (`H-NR-09`); no count in `Map v2` | Use a `10/10` cycle sanity check alongside mandatory `0`-panic / IME checks | `proposed — awaiting Owner decision` |
| **`NR-D5`** | Stage 2 rubric weights & min sub-score floor | `35%` Perf (`15/10/10`), `35%` Integration (`12/13/10`), `30%` Packaging (`10/10/10`), plus `min(s_{c,k}) >= 2.0` | Balances `Map v2 §4 P2` axes and rejects candidates with an unusable dimension (`< 2.0`) | Owner sets custom weights or reviews unweighted matrix across Stage 1 survivors | `proposed — awaiting Owner decision` |
| **`NR-D6`** | Stage 2 win margin & 3-step tie-break | Clear-win margin **`>= 0.35`** pts; close calls use `(1) Iris Xe 15-min p5 -> (2) zero-IPC/burn safety -> (3) single-bundle portability` | Avoids fragile `< 7%` score noise and encodes P2 survival priorities into tie-breaks | Owner chooses directly among Stage 1 survivors without preset margin | `proposed — awaiting Owner decision` |
| **`NR-D7`** | External local tool installs (Windows 1 / Dell) | Approve untracked install of **`PresentMon v2.6.0`** (MIT, `GameTechDev/PresentMon`), **`RenderDoc v1.47`** (MIT, `baldurk/renderdoc`), optional **`PIX 2611.19`** (MS EULA, `devblogs.microsoft.com/pix/download/`), **`Godot 4.7.2-stable`** (MIT, `godotengine/godot`), & optional **`DXC v1.9.2609`** (LLVM NCSA, `microsoft/DirectXShaderCompiler`) | Required by `§4` ETW/pass captures & Candidate `(c)`; Dell gets prebuilt exports only (`AGENT_RULES.md §5`) | Do NOT install unapproved tools (e.g., omit `PIX` if Child HWND suffices, or drop `(c)` if `Godot` declined) | **`proposed — DO NOT INSTALL until approved`** |
