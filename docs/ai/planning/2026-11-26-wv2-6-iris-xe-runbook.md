---
phase: planning
feature: map-v2
title: WV2-6 Dell Iris Xe Performance Gate Runbook
description: Owner execution runbook for the WV2-6 Dell Iris Xe Low-quality 1,000-creature before/after performance gate, covering build-pair acquisition, app setup, existing vs missing instrumentation, proposed measurement protocol, and result template
status: proposed
owner: anima-dell
last_reviewed: 2026-11-26
---

# `WV2-6` — Dell Iris Xe Performance Gate Runbook

> **Status: `proposed` (documents only — zero code, zero builds, zero desktop app launches).**
> - **Authority:** Written for the project owner under lead authorization (`2026-11-25`, Night Queue Card N2) so the Dell Iris Xe gate of `WV2-6` can be executed quickly and unambiguously when the `WV2-6` branch is ready.
> - **Provenance:** All code paths, symbols, and line numbers in this note were verified against `origin/feat/ecological-selection` at `3d8bce73aa727a72a1c190bd2dd7509f878cb5e4` (`#287`, 2026-11-25) plus the pending decisions recorded in Draft PR `#288` (`2345d847664120885c58ab8cada7eeaa2f8459c8`, `VERIFIED` at review).
> - **No cargo on Dell (`QD-WAVE1124b` / `AGENT_RULES.md §5`):** The Dell Vostro 3530 (`i7-1355U`, `16 GB` shared RAM, Intel Iris Xe) **never compiles Rust** for this gate; it installs or runs prebuilt Windows binaries only.
> - **Owner decision tags:** Accepted gates come verbatim from the World v2 design, Map v2 requirements, and `AGENT_WORKFLOW.md`. Protocol parameters not yet fixed in an accepted spec (warm-up, duration, repetition count, summary statistic, and regression tolerance) are explicitly marked **`PROPOSED — OWNER DECISION IX-D1..IX-D7`**.

---

## 1. What the Design Requires

The `WV2-6` performance gate is defined in two places in [`2026-11-20-world-v2-design.md`](../design/2026-11-20-world-v2-design.md):

- **§7.2 item 4 (`2026-11-20-world-v2-design.md:515`):**
  > **`WV2-6`'s Done gate is NO FPS drop at `Low` quality measured before vs. after inside the real desktop app on the Dell Iris Xe laptop with `1,000` creatures**, reporting **`p5` FPS and per-system ms breakdown against `§3b`** (using WebGL2 GPU timer queries aligned with `NR-SPIKE`'s `G-PERF` definition; `<= 60` draw calls, `qualityTiers.ts:65`).
- **§8 Card 8 Definition of Done (`2026-11-20-world-v2-design.md:623`):**
  > Build-time prefetch bundles tiles cleanly (`0` runtime network calls), `check:bundle` / `check:docs` / `check:ipc` pass, AND **real Dell Iris Xe desktop app benchmark at `Low` quality with `1,000` creatures shows NO FPS drop before vs. after**, reporting **`p5` FPS and per-system ms aligned with `NR-SPIKE` `G-PERF` against `§3b`** (`<= 60` draw calls).

### 1.1 Hard Technical Constraints for `Low` Quality

From [`2026-11-20-feature-map-v2.md` §3b](../requirements/2026-11-20-feature-map-v2.md#3b-nhẹ-mà-đẹp-ngân-sách-khung-hình-và-kỹ-thuật) (lines `108-129`) and [`qualityTiers.ts`](../../../src/components/Landscape/utils/qualityTiers.ts#L45-L68):

| Parameter / Budget | Requirement | Source |
|---|---|---|
| Target frame rate & ceiling | `>= 30.0 FPS` median (`<= 33.33 ms` frame time, `720p -> 1080p` upscale budget in §3b) | `2026-11-20-feature-map-v2.md:120`, `qualityTiers.ts:66-67` |
| `p5` FPS (`1000 / P95 ms`) | Reported verbatim (`NR-SPIKE` `NR-D1` floor decided via `QD-OPENQ`: `>= 25.0 FPS` / `<= 40.0 ms` on Dell Low) | `2026-11-23-nr-spike-protocol.md:158,366` |
| Draw calls (`renderer.info`) | **`<= 60` draw calls** (`maxDrawCalls: 60`) | `qualityTiers.ts:65`, `world-v2-design.md:515,623` |
| Triangle caps (`Low`) | `maxTriangles: 1,200,000`; `maxCreatureTriangles: 300,000` | `qualityTiers.ts:63-64` |
| Shadows (`Low`) | `shadows: false` (no real-time shadow maps; `contactShadows: true`) | `2026-11-20-feature-map-v2.md:109`, `qualityTiers.ts:52,54` |
| Render scale & water (`Low`) | `dprRange: [0.75, 1]`, `defaultDpr: 1`, `waterReflections: false`, `waterRefractions: false`, `cloudLayers: 1`, `vegetationDensity: 0.25` | `qualityTiers.ts:49-58` |
| Creature LOD (`Low`) | `creatureFullDetailRadius: 90`, `creatureSimplifiedRadius: 220`, `CREATURE_NEAR_SKINNED_CAP.low = 8` | `qualityTiers.ts:61-62`, `creatureLod.ts:28-32` |
| Creature count for this gate | **1,000 creatures** (`QD-SCALE-TRIAL` uses ~100 for casual trials, while 1,000 remains the benchmark point enforced by this gate) | `world-v2-design.md:515,623`, `AGENT_WORKFLOW.md:406-412` |

### 1.2 `§3b` Per-System Frame Budgets (Iris Xe, `Low`, `33.3 ms` Total)

From `docs/ai/requirements/2026-11-20-feature-map-v2.md:120-128`:

| System (`§3b`) | Iris Xe `Low` Budget (ms) | Notes |
|---|---:|---|
| Terrain | `6.0` | Displacement/splatting/baked `terrain_lighting` |
| Vegetation | `6.0` | Instanced flora (`vegetationDensity: 0.25`), culling, LOD |
| Water | `3.0` | Ocean/lakes/rivers (`shore_sdf` + `hydro_flow`; no SSR on Low) |
| Sky & Atmosphere | `1.5` | Sky, fog, `cloudLayers: 1` |
| Creatures (`1,000`) | `5.0` | Articulated near `SkinnedMesh` (`K = 8`) + mid instanced + far markers + contact shadows |
| Post & Upscale | `4.0` | Post-processing / tone mapping / upscale |
| Reserve (CPU, Simulation, UI) | `7.8` | Rust simulation tick, IPC emission, React/WebView2 HUD & composite |
| **Total Frame Budget** | **`33.3`** | **`30.0 FPS`** (`1000 / 33.33`) |

---

## 2. The Build Pair (No Cargo on Dell)

### 2.1 Which Two Commits Are Compared

To isolate the rendering change of `WV2-6` cleanly without conflating simulation-world differences:

1. **Before build (`B0`):** The merge commit (or head) on `origin/feat/ecological-selection` **immediately preceding `WV2-6`**, which is already post-`WV2-7` (`#279`, merged into trunk at `9931efe8` / `3d8bce73`). Both `B0` and `A1` therefore run the exact same Rust backend against bundled `artifacts/world-v2/sim_256.anmw3` (`TerrainSource::AppArtifactOrGenerated`), and differ **only** in the Three.js viewer (`worldGen.ts` v1 procedural terrain in `B0` vs. bundled World v2 tiles in `A1`).
2. **After build (`A1`):** The exact head commit of the `WV2-6` pull request branch under review.

> **Important (`QD-GATES-1125` items 3 & 4, PR `#288`):** `WV2-6` fetches its baked tile pack during `build.beforeBundleCommand` (`fetch_world_v2_tiles.mjs`, `2026-11-20-world-v2-design.md:619`). Because the public GitHub Release tile asset is published **once for the final world before `v0.9.0-rc.1`** (`QD-GATES-1125 (4)`), a CI workflow run on `WV2-6` *before* that asset is published cannot download the tile pack from GitHub Releases. There are therefore two clean acquisition paths depending on timing:
>
> - **Path A (Standard CI artifact path, once the release tile asset is published; and always valid for `B0`):** Build via GitHub Actions `.github/workflows/release-build.yml` (`QD-REL09 G3`).
> - **Path B (Pre-publication internal trial path, `QD-GATES-1125 (3)` / `QD-TRIAL-V3`):** `B0` comes from `.github/workflows/release-build.yml`, and `A1` is the internal unsigned Windows test installer built by `anima-case` on Windows 1 from the exact `WV2-6` head SHA with locally staged tiles, delivered via the hub (with exact `git rev-parse HEAD` and installer SHA-256 logged; never published externally).

### 2.2 Getting Windows Installers from CI (`.github/workflows/release-build.yml`)

Per `QD-REL09 G3` (`docs/how-to/AGENT_WORKFLOW.md:401-402`) and [`docs/how-to/INSTALL.md`](../../how-to/INSTALL.md):

- **Workflow file:** `.github/workflows/release-build.yml` (workflow name: **`Release Build`**)
- **Trigger:** `workflow_dispatch` (or `pull_request` modifying `.github/workflows/release-build.yml`)
- **Job:** `build-windows` (**`Release Build (Windows)`** on `runs-on: windows-latest`, timeout `120` min; runs `npm ci` then `npm run tauri:build`, which invokes `tauri build --features desktop`)
- **Artifact name:** **`anima-engine-windows-installers`** (`retention-days: 14`)
- **Artifact contents:**
  - `src-tauri/target/release/bundle/msi/*.msi`
  - `src-tauri/target/release/bundle/nsis/*.exe`

**Steps to trigger and download on the Dell (no Rust/Cargo required):**

1. Trigger a build for the target branch/ref (or check **Actions -> Release Build** for an existing green run at that exact commit SHA):
   ```powershell
   gh workflow run release-build.yml --repo DuongNAD/Anima-Engine --ref <branch-or-tag>
   ```
   *(Note: GitHub `workflow_dispatch` runs against a branch or tag ref; for an arbitrary commit SHA not at a branch tip, push a temporary tag/branch or locate the existing run.)*
2. Watch the `Release Build (Windows)` job complete and record its run ID and `headSha`:
   ```powershell
   gh run list --repo DuongNAD/Anima-Engine --workflow release-build.yml --limit 5
   ```
3. Download **`anima-engine-windows-installers`** for each commit into separate directories (for example `C:\Users\DELL\Downloads\irisxe-before\` and `C:\Users\DELL\Downloads\irisxe-after\`):
   ```powershell
   gh run download <run-id> --repo DuongNAD/Anima-Engine -n anima-engine-windows-installers -D <dest-dir>
   ```
   Alternatively, open **GitHub -> Actions -> Release Build -> select run -> Artifacts -> `anima-engine-windows-installers`** while signed in to GitHub.
4. Record the SHA-256 of both `.exe` / `.msi` installers (`Get-FileHash <installer> -Algorithm SHA256`).
5. **Installing unsigned builds cleanly:** Because `0.9` pre-release installers are unsigned (`QD-REL09 G2`), Microsoft Defender SmartScreen shows **Windows protected your PC** -> click **More info** -> **Run anyway** (`docs/how-to/INSTALL.md` §3). Both `B0` and `A1` share `identifier: "com.anima.engine"` (`src-tauri/tauri.conf.json:5`), so uninstall `B0` via Windows **Installed apps** (or run the `A1` installer cleanly after finishing `B0`, or extract/install sequentially between interleaved passes) and verify the installed `Anima-Engine.exe` timestamp/hash before each test run.

---

## 3. Getting `1,000` Creatures and `Low` Quality in the App

### 3.1 Pre-Flight on the Dell (`AGENT_RULES.md §5`)

Before launching either build:
1. Plug the Dell Vostro 3530 into **AC mains power** and set a fixed Windows power mode (record it in §6).
2. Confirm **zero** Rust/Cargo activity twice, 60 s apart:
   ```powershell
   tasklist | findstr /i "cargo rustc"
   ```
   *(Must print nothing.)*
3. Confirm at least **5 GB** free physical RAM and close browsers, IDEs, screen recorders, and background updaters:
   ```powershell
   powershell -c "$os=Get-CimInstance Win32_OperatingSystem; [math]::Round($os.FreePhysicalMemory/1MB, 2)"
   ```
4. Keep Windows display resolution and scaling fixed across all runs (e.g., `1920x1080` at `100%` or `125%`, and keep the app window at default `1280x720` per `src-tauri/tauri.conf.json:16-17` or maximized — **identical** for `B0` and `A1`).

### 3.2 Step 1 — Set `1,000` Creatures (Environment Variable, No UI Control)

**UI check:** Neither [`src/App.tsx`](../../../src/App.tsx), [`src/components/SimulationControls.tsx`](../../../src/components/SimulationControls.tsx), nor [`src/components/ExperimentWorldControl.tsx`](../../../src/components/ExperimentWorldControl.tsx) provides a UI field to choose founding population count. Without an override, genesis spawns `DEFAULT_FOUNDING_POPULATION = 10` agents on `FoundingLayout::Line` (`src-tauri/src/core/resources.rs:1340,1400-1406`), and `ExperimentWorldControl` manifests spawn `100` (`QD-SCALE-TRIAL`).

Instead, the desktop app reads founding population from process environment variables at startup. Launch the installed `Anima-Engine.exe` from **the same PowerShell session** where these variables are set:

```powershell
# 1. Mandatory: ignore any existing saves/autosave.json (which may hold 10 agents)
#    and avoid overwriting the user's autosave on exit.
$env:ANIMA_FRESH_START = "1"

# 2. Spawn 1,000 founders (70% prey, 30% predator) on an inset square grid inside MapBounds.
$env:ANIMA_FOUNDING_POPULATION = "1000"

# 3. Optional: begin ticking immediately on launch without waiting for button click.
$env:ANIMA_AUTOSTART = "1"

# 4. Keep mortality & reproduction UNSET so population stays fixed at 1,000 for the whole run:
Remove-Item Env:ANIMA_MORTALITY -ErrorAction SilentlyContinue
Remove-Item Env:ANIMA_REPRODUCTION -ErrorAction SilentlyContinue

# 5. Optional: capture Rust backend CPU tick timings in release builds (no DevTools needed):
$env:ANIMA_TICK_CAPTURE = "warmup=300,samples=1800"
$env:ANIMA_TICK_CAPTURE_OUT = "irisxe-before-r1"   # change per run (e.g. irisxe-after-r1)

# Launch the installed binary (adjust path if installed per-machine vs per-user):
& "$env:LOCALAPPDATA\Anima-Engine\Anima-Engine.exe"
```

**Code anchors for every variable above:**
- `ANIMA_FRESH_START`: `fresh_start_from_env()` / `LaunchPersistencePolicy::from_env()` (`src-tauri/src/core/resources.rs:1299-1321`), checked in `src-tauri/src/lib.rs:32,108-113` so an existing `autosave.json` neither replaces the 1,000-creature genesis nor gets overwritten on exit.
- `ANIMA_FOUNDING_POPULATION`: `FoundingPlan::from_env()` / `FoundingPlan::parse()` (`src-tauri/src/core/resources.rs:1409-1460`, `MAX_FOUNDING_POPULATION = 10_000` at line `1352`), invoked in `src-tauri/src/core/simulation_loop.rs:1695`. Places `1,000` founders (`70%` prey / `30%` predator via `FoundingPlan::is_prey`) on `FoundingLayout::Grid`.
- `ANIMA_AUTOSTART`: `autostart_from_env()` (`src-tauri/src/core/resources.rs:1276-1284`). If left unset, click **`Bắt đầu mô phỏng`** (`src/components/SimulationControls.tsx:26`).
- `ANIMA_MORTALITY`: `MortalityPolicy::from_env()` (`src-tauri/src/core/resources.rs:587-605`, `src-tauri/src/core/simulation_loop.rs:1482`); leaving it unset keeps mortality off so count does not drift below 1,000.
- `ANIMA_TICK_CAPTURE` & `ANIMA_TICK_CAPTURE_OUT`: `src-tauri/src/core/simulation_loop.rs:1990-2013,2591-2598` and `export_capture_to_app_data` (`src-tauri/src/commands/capture.rs:89-105`), writing JSON under `%APPDATA%\com.anima.engine\captures\`.

**Confirm 1,000 creatures in the UI:** On the Control Center screen before switching to 3D, check the status readout (`src/App.tsx` / `StatusPanel.tsx`) to verify **1,000** live agents are present and ticks are advancing (`Backend FPS` > 0).

### 3.3 Step 2 — Enter the 3D Scene and Lock `Low` Quality

1. Click **`🏞️ Landscape Showcase`** (`src/App.tsx:547-565`, which flips `showLandscape` and renders `<LandscapeShowcase />` = lazy [`WorldShowcase.tsx`](../../../src/components/Landscape/WorldShowcase.tsx) at `src/App.tsx:29,695`). Wait for the 3D world to appear.
2. **Pitfall — default `'auto'` quality:** [`readInitialQualitySelection`](../../../src/components/Landscape/utils/qualityTiers.ts#L136-L145) (called at `WorldShowcase.tsx:265-270`) defaults to `'auto'`, which mounts `<AutoQualityMonitor>` (`WorldShowcase.tsx:156-174,545-547`) starting at `tier: 'high'` (`WorldShowcase.tsx:274`) and dynamically changes tier/DPR based on frame delta. **Do not leave it on `Tự động` (`auto`).**
3. In the top-left `WorldHud` control panel (`WorldShowcase.tsx:926-937`), on the **`GPU`** row, click **`Thấp`** (`data-testid="quality-btn-low"`, label from `QUALITY_SELECTION_LABELS.low = 'Thấp'` at `qualityTiers.ts:16`):
   - Sets `qualitySelection` to `'low'`, unmounts `AutoQualityMonitor` (`WorldShowcase.tsx:545`).
   - Invokes `QualityApplier` (`WorldShowcase.tsx:142-153`), immediately updating `gl.setPixelRatio(1)` (`defaultDpr: 1`), `gl.shadowMap.enabled = false`, `vegetationDensity: 0.25`, `cloudLayers: 1`, `waterReflections: false`, `waterRefractions: false`, `CREATURE_NEAR_SKINNED_CAP.low = 8` (`creatureLod.ts:29`), and `maxDrawCalls: 60`.
4. **Pin all other scene controls identically across `B0` and `A1`:**
   - Weather: **`clear`** (`WorldShowcase.tsx:263,885-889`).
   - Time / day-night speed: fixed (e.g., `1×` or paused `0×` at `11:00`, identical on both builds; `WorldShowcase.tsx:260-261,874-882`).
   - Decoration toggle (`data-testid="toggle-decoration-btn"`, `WorldShowcase.tsx:976-983`): leave in the identical state on both builds (default **`Show decoration`** active = `showDecoration = true`).
   - Camera pose/mode: keep the initial `orbit` spawn landing view (or switch identically for the second pose; see §5).

---

## 4. Reading `p5` FPS, Per-System ms, and Draw Calls

### 4.1 What Instrumentation Exists Today

| Metric Required by Gate | What Exists in Trunk Today | Release App Usability |
|---|---|---|
| **Instant / Smoothed FPS** | `WorldCameraRig.tsx:273-274` computes EMA `v.fps = v.fps * 0.9 + (1 / delta) * 0.1`, sampled every `250 ms` (`WorldShowcase.tsx:357-369`) and shown as integer `{fps} FPS` in `WorldHud` (`WorldShowcase.tsx:907-912`). | **Visible in HUD, but NOT `p5` FPS** (exponential smoothing hides spikes and cannot be exported). |
| **`p5` FPS (5th-percentile FPS / P95 frame time)** | **None in `src/`.** No frame-time percentile ring buffer or export exists in the frontend. Externally, `NR-SPIKE` (`2026-11-23-nr-spike-protocol.md:183-211`) specifies Intel `PresentMon.exe` (`1000 / P95(msBetweenPresents)`), which is not installed (`NR-D7`: no installs without explicit owner approval). | **Missing in app**; external `PresentMon` requires owner `NR-D7` sign-off. |
| **Per-system GPU ms (`§3b` Terrain / Vegetation / Water / Sky / Creatures / Post)** | **None anywhere in `src/` or `src-tauri/src/`.** Search across the entire repo finds zero `EXT_disjoint_timer_query_webgl2` / `TIME_ELAPSED_EXT` / `getQueryParameter` GPU timer code. | **Missing.** |
| **Reserve: CPU / Sim / UI ms (`§3b` `7.8 ms` row)** | `tick_capture` (`src-tauri/src/core/tick_capture.rs:95-103,874,923-925`) measures Rust simulation phases (`full_tick`, `schedule`, `sensor_brain`, `physics_movement`, `ecology_resources`, `schedule_tail`, `telemetry_publish`) with nearest-rank `p50_ns`/`p95_ns`/`p99_ns` and exports JSON via `ANIMA_TICK_CAPTURE_OUT` (`src-tauri/src/commands/capture.rs:89-105`). | **Works in release builds** without DevTools; covers **CPU sim only** (excludes WebView2/JS/GPU). |
| **Draw calls (`<= 60`) and triangles** | `installFrameCounters` (`src/components/Landscape/utils/diagnosticsWindow.ts:301-506`), mounted by `FrameDiagnostics` (`src/components/Landscape/WorldVegetation.tsx:401-422`), publishes `window.__animaFrameCounters` (`{ frame, total: { calls, triangles }, layers: { terrain, flora, creatures, water, other }, other }`) from `renderer.info`. | **Not accessible in release builds:** `src-tauri/Cargo.toml:21` omits Tauri `devtools` (`features = ["test"]`), so release installers have no DevTools console, and no HUD element displays `__animaFrameCounters`. (`tests/e2e/rw_budget.spec.ts` checks it only in Playwright dev Chromium, without the Rust 1,000-creature backend.) |

### 4.2 Plain Statement of Gaps

Today, a packaged Windows release installer built from trunk **cannot** report `p5` FPS, **cannot** report per-system GPU milliseconds (`§3b` Terrain/Vegetation/Water/Sky/Creatures/Post), and **cannot** display or export `window.__animaFrameCounters` draw calls (`<= 60`) inside the app because release builds ship without WebView2 DevTools.

### 4.3 Smallest Card to Close the Gap: `WV2-6-INSTR` (Frontend-Only, ~3 Files)

Before the owner can fill out §6 from release installers without external guesswork, a small **frontend-only** card (no Cargo changes) closes the in-app gap:

- **Scope (`WV2-6-INSTR`, 3 files):**
  1. `src/components/Landscape/utils/perfRecorder.ts` (new):
     - Fixed-capacity frame-delta ring buffer (`performance.now()` / R3F `useFrame` raw delta, zero per-frame heap allocations after warmup).
     - Nearest-rank summary computation (`median_fps = 1000 / P50_ms`, `p5_fps = 1000 / P95_ms`, `p1_fps = 1000 / P99_ms`, matching `tick_capture.rs:874`).
     - Reads `readFrameCounters()` (`diagnosticsWindow.ts:515-538`) for `total.calls`, `total.triangles`, and per-layer draw calls/triangles (`terrain`, `flora`, `creatures`, `water`, `other`), tracking peak and median draw calls.
     - Probes WebGL2 `EXT_disjoint_timer_query_webgl2`: if available and non-disjoint, brackets pass groups into per-system GPU ms; if unavailable or restricted by WebView2/ANGLE on Iris Xe, **explicitly reports `"unavailable"`** (never fabricates or estimates GPU ms).
  2. `src/components/Landscape/WorldShowcase.tsx` (minimal UI hook):
     - Adds a toggleable **`Perf`** button / `F3` overlay (mounted **only when `capture === null`**, leaving canonical view captures `100%` byte-identical) showing live `Median FPS`, `p5 FPS`, `Draw Calls (peak / current)`, `GPU Timers: <available|unavailable>`, and a **`Record 60s -> Copy JSON`** button so the owner can run release installers with zero DevTools.
  3. `tests/frontend/perfRecorder.test.ts` (new):
     - Unit tests for nearest-rank `p5` math, `readFrameCounters()` peak draw-call tracking, clean `"unavailable"` fallback when `EXT_disjoint_timer_query_webgl2` is absent, and `capture !== null` inertness.
- **Note on swapchain ground truth (`PresentMon`):** `WV2-6-INSTR` measures `requestAnimationFrame` / Three.js render-loop frame deltas inside WebView2. If `EXT_disjoint_timer_query_webgl2` is unavailable in WebView2/ANGLE or if the owner also wants DWM/DXGI swapchain `msBetweenPresents` (which captures WebView2 compositor stalls too), approving `PresentMon` under `NR-D7` (`2026-11-23-nr-spike-protocol.md:183-211`) pairs cleanly with `WV2-6-INSTR`.

---

## 5. Measurement Protocol (`PROPOSED — OWNER DECISION IX-D1..IX-D7`)

> Every parameter in this section is **PROPOSED** for owner sign-off because `2026-11-20-world-v2-design.md` specifies "NO FPS drop at Low quality ... with 1,000 creatures" and `p5` FPS without fixing run length, warm-up, repetition count, or noise tolerance.

| Decision ID | Parameter | Proposed Rule | Rationale |
|---|---|---|---|
| **`IX-D1`** | **Machine & Window State** | AC mains plugged in, fixed Windows power mode (Balanced or Best Performance, same for all runs), `tasklist \| findstr /i "cargo rustc"` empty, `>= 5 GB` free RAM, fixed display scaling & app window size (`1280x720` default or maximized `1920x1080`), `2 min` idle cool-down between runs. | Prevents `15 W` i7-1355U TDP starvation, background Cargo RAM exhaustion (`AGENT_RULES.md §5`), and resolution/DPR mismatch. |
| **`IX-D2`** | **Fixed Scene & Poses** | Launch with `ANIMA_FRESH_START=1`, `ANIMA_FOUNDING_POPULATION=1000`, `ANIMA_AUTOSTART=1`, mortality/reproduction unset; open `Landscape Showcase`, immediately click **`Thấp`** (`Low`), weather `clear`, fixed time (`11:00`, `0×` or `1×` identical across both), fixed decoration state. Measure **Pose A (`spawn` landing view)** and **Pose B (pan/zoom out to high terrain+water+creature coverage, or `overview` view once pose is pinned)**. | Isolates rendering cost with 1,000 live agents at identical `Low` LOD/density/DPR and prevents `AutoQualityMonitor` drift. |
| **`IX-D3`** | **Warm-up** | **`30 s`** (`>= 900` frames at `30 FPS`, `>= 300` simulation ticks) after switching to `Thấp`, before recording starts. | Flushes WebGL2 shader compiles, KTX2/tile decode & GPU texture upload, V8/JIT warmup, and initial CPU turbo boost. |
| **`IX-D4`** | **Run Length** | **`60 s`** per measured repetition (`~1,800` frames at `30 FPS`). If `A1` median FPS is within `2.0 FPS` of `30.0 FPS`, add one **`10 min` thermal check** at the end. | Matches `NR-SPIKE §4.2` `60 s` capture window and `BENCHMARKING.md` `1,800`-tick window; yields ~90 tail frames below `p5` for stable `P95` frame-time estimation without excessive thermal creep inside one run. |
| **`IX-D5`** | **Repetitions & Order** | **`3` interleaved repetitions** per pose: `B0-1 -> A1-1 -> B0-2 -> A1-2 -> B0-3 -> A1-3`, with a **`2 min`** cool-down between runs. | Interleaving cancels monotonic ambient warming, Windows Defender scans, or laptop thermal drift that would bias a sequential `3× Before` then `3× After` block. |
| **`IX-D6`** | **Statistics Compared** | Compare **median-of-3** for **Median FPS** (`1000 / P50_ms`) and **`p5` FPS** (`1000 / P95_ms`), plus **worst-of-3 `p5` FPS**, **max observed draw calls**, and **per-system ms** against `§3b`. | Median-of-3 rejects single OS background spikes while preserving honest central tendency; worst-of-3 `p5` exposes intermittent hitch regressions. |
| **`IX-D7`** | **Tolerance for "No FPS Drop"** | **Median FPS:** `A1 >= B0 - max(1.0 FPS, 3% of B0)` AND if `B0 >= 30.0 FPS`, `A1 >= 30.0 FPS`. **`p5` FPS:** `A1 >= B0 - max(1.5 FPS, 5% of B0)` (flag if `A1 < 25.0 FPS` per decided `NR-D1`). **Draw calls:** **zero tolerance** (`A1_max_calls <= 60`). | WebView2/DWM/laptop turbo variance is typically `~0.5–1.0 FPS` (`~1 ms` at `30 FPS`); a `max(1.0 FPS, 3%)` median band prevents false failures from OS jitter while catching any `>= 1.5 ms` shader regression. Draw calls are deterministic submission counts (`rw_budget.spec.ts:16-18`) and take zero tolerance. |

---

## 6. Owner Result Table Template

Copy this section into the gate report (or `test_runs/owner_irisxe_wv2_6_result.md`) and fill in measured values verbatim.

### 6.1 Build Pair & Host Provenance

| Field | Before (`B0`) | After (`A1`) |
|---|---|---|
| Git commit SHA (`40`-char) | | |
| Source (CI Workflow Run URL + ID, or `QD-TRIAL-V3` `anima-case` bundle) | | |
| Installer filename & SHA-256 | | |
| World artifact (`sim_256.anmw3` SHA-256 / tile pack version) | | |
| Machine / OS / GPU Driver | Dell Vostro 3530 / Win `____` / Intel Iris Xe `____` | *(same)* |
| Power & RAM at start (`AC?`, power mode, free RAM GB) | | |
| Window size / Windows DPI scaling / `quality` clicked | `____` / `____%` / `Thấp` (`low`) | *(same)* |
| Population verified (`1,000` live agents, mortality off) | `Yes / No` | `Yes / No` |

### 6.2 Interleaved Run Log (`Low` Quality, `1,000` Creatures, `30 s` Warm-up + `60 s` Measure)

| Run ID | Build | Pose | Median FPS | `p5` FPS | `p1` FPS | P50 Frame (ms) | P95 Frame (ms) | Peak Draw Calls (`<= 60`) | Total Triangles | Backend `full_tick` p50 / p95 (ms) | Notes / Anomalies |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---|---|
| `B0-1` | Before | | | | | | | | | | |
| `A1-1` | After | | | | | | | | | | |
| `B0-2` | Before | | | | | | | | | | |
| `A1-2` | After | | | | | | | | | | |
| `B0-3` | Before | | | | | | | | | | |
| `A1-3` | After | | | | | | | | | | |
| **Summary** | **Before (med-of-3)** | | | | | | | **Max:** | | | |
| **Summary** | **After (med-of-3)** | | | | | | | **Max:** | | | |
| **Delta** | **`After - Before`** | | | | | | | | | | |

### 6.3 Per-System Breakdown vs. `§3b` (`33.3 ms` Iris Xe `Low` Ceiling)

*(If WebGL2 GPU timer queries are unavailable in WebView2/ANGLE, record `"unavailable"` in the GPU rows rather than estimating, and fill the Reserve/CPU row from `ANIMA_TICK_CAPTURE_OUT`.)*

| System (`§3b`) | `§3b` Low Budget (ms) | Before (ms) | After (ms) | Delta (ms) | Measurement Source | Status (`<= Budget` / `Over` / `Unavailable`) |
|---|---:|---:|---:|---:|---|---|
| Terrain | `6.0` | | | | | |
| Vegetation | `6.0` | | | | | |
| Water | `3.0` | | | | | |
| Sky & Atmosphere | `1.5` | | | | | |
| Creatures (`1,000`) | `5.0` | | | | | |
| Post & Upscale | `4.0` | | | | | |
| Reserve (CPU / Sim / UI) | `7.8` | | | | `tick_capture` (`full_tick` p50/p95) + UI | |
| **Total Frame** | **`33.3`** | | | | | |

### 6.4 Gate Verdict & Owner Decisions

- **Proposed Protocol (`IX-D1..IX-D7`):** `[ ] Approved as written` / `[ ] Approved with changes: ____`
- **Instrumentation Choice:** `[ ] Authorize WV2-6-INSTR frontend card` / `[ ] Also approve PresentMon (NR-D7)`
- **Final `WV2-6` Iris Xe Gate Result:** `[ ] PASS` / `[ ] FAIL` / `[ ] BLOCKED-ON-INSTRUMENTATION`
- **Owner / Operator Notes:** `____`
