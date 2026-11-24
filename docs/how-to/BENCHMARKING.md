---
title: Performance Benchmarking with Criterion
status: active
owner: maintainers
last_reviewed: 2026-07-27
review_cycle: per-release
---

# Performance Benchmarking per System with Criterion

> *Vietnamese version / Bản tiếng Việt: [BENCHMARKING.vi.md](BENCHMARKING.vi.md).*

This document is the **how-to**. Committed numbers live in [§ Baseline](#baseline-2026-07-26); how to interpret a report and required metadata remain in [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md).

> **Are you the project owner with only five minutes?** Skip the Criterion section. The only work a machine cannot perform is [§ One-run owner checklist](#one-run-owner-checklist) — launching the app once and collecting the remaining three pieces.

## Why This Suite Exists

The claim "60 FPS real-time" at release settings / 1,000 agents **has never been measured**: [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md) only holds Criterion per-system timings (a lower bound) and one full desktop app run in **debug mode with 10 agents** on 2026-07-27 (see [§ In-app tick capture](#in-app-tick-capture-2026-07-27)), whereas a full release-mode desktop measurement at 1,000 agents has still not been performed on the dev machine due to the constraint recorded there. That constraint still holds — so this benchmark suite **does not run the full backend**. Each benchmark drives a single system or pure function on self-constructed data: no Tauri, no window, no GPU device, and no simulation thread.

That is also why Criterion fits: a 60 FPS frame budget is **16.67 ms**, and frame time is a **sum across systems** — so per-system numbers form the components of that budget.

## Running

```bash
cargo bench --bench tick_systems
```

Run faster when only a rough number is needed (default is 3 s warm-up + 5 s measurement per item):

```bash
cargo bench --bench tick_systems -- --warm-up-time 1 --measurement-time 3
```

Run only one group:

```bash
cargo bench --bench tick_systems -- tick/dynamic_fields
```

*(Checklists on this page are written for Windows; equivalent macOS commands are listed in [How-to § On macOS and Linux](README.md#on-macos-and-linux). None of these checklists has yet been run on macOS.)*

Run from `src-tauri/`; **on Windows use PowerShell rather than Git Bash** — for the same reason as `cargo test` (see `STATE_OF_THE_PROJECT.md` §4).

## Comparing Against Previous Runs

Criterion stores results under `target/criterion/` automatically and prints `change: [...]` against the prior run on the same machine. Save a named baseline for later comparison:

```bash
cargo bench --bench tick_systems -- --save-baseline before
```

```bash
cargo bench --bench tick_systems -- --baseline before
```

`target/` is in `.gitignore`, so saved baselines are **local per machine**. The shared project baseline is the table below, and it is meaningful only alongside its hardware block.

## Disabling / Removing

`criterion` is a `dev-dependency` and does not enter the shipping binary. Removing it means deleting the `[dev-dependencies]` entry, the `[[bench]]` block in [`src-tauri/Cargo.toml`](../../src-tauri/Cargo.toml), and `src-tauri/benches/`. Zero production code paths depend on it.

Constraint to preserve: `cargo tree --no-default-features -e normal` **must not** show `criterion`. That is what feature-separation gate G2 verifies, and a `dev-dependency` is invisible to it by design. Verified 2026-07-26 — 0 matches.

---

## Baseline 2026-07-26

**These are measured numbers, not proxies.** Built with `--release`, 100 samples per item.

> **Which number to read: median.** The `time: [a b c]` line printed by `cargo bench` **is not the median** — under linear sampling, `b` is the **slope estimate**. The two values genuinely diverge: `step_water` reports a slope of 297.6 µs vs a median of 271.5 µs. The table below reports the **median**, read from `median.point_estimate` inside `target/criterion/**/new/estimates.json`, because it is more resilient to occasional outlier samples on a desktop machine with background load. Both median and mean are recorded in [`benchmark_report.json`](../../benchmark_report.json).

### Hardware — This Is the Target Machine

| | |
|---|---|
| CPU | Intel Core i5-14600KF · 14 cores / 20 threads · 3.5 GHz base |
| RAM | 47.8 GB |
| OS | Windows 11 Pro 10.0.26200 |
| Toolchain | rustc 1.95.0 · cargo 1.95.0 |
| Command | `cargo bench --bench tick_systems -- --warm-up-time 1 --measurement-time 3` |

The project's target hardware is now **this machine** (updated 2026-07-26, superseding the older Dell Vostro 3530 declaration in [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md)). Thus the tables below **are** measurements on target hardware.

Still, this **does not** suffice to close `STATE_OF_THE_PROJECT.md` §3.2 — see [§ What is still missing](#what-is-still-missing).

### World Fields, 256×256 (Real Dimension, `MapSettings::default()`)

| Function | Median | Mean | Notes |
|---|---:|---:|---|
| `ResourceField::step_regrowth` | 70.6 µs | 72.2 µs | Ungated, single pass |
| `ResourceField::step_regrowth_gated` | 218.5 µs | 269.9 µs | Two passes, detritus budget active |
| `ResourceField::step_regrowth_gated_strided` | **55.0 µs** | 57.4 µs | `REGROWTH_STRIDE = 4` — **3.97× faster** than non-strided path |
| `DynamicFields::step_water` | **271.5 µs** | 288.7 µs | **Most expensive among all per-tick systems** |
| `DynamicFields::step_soil` | 47.2 µs | 47.8 µs | |
| `DynamicFields::step_erosion` | 20.1 µs | 20.6 µs | Local formula, no sediment transport |

### By Agent Count (Median)

| Function | 100 | 1,000 | 10,000 | Slope / Agent |
|---|---:|---:|---:|---:|
| `integrate_physics_system` | 493 ns | 4.9 µs | 49.2 µs | **4.92 ns** |
| `rebuild_spatial_grid_system` | 13.4 µs | 94.3 µs | **734.5 µs** | **72.8 ns** + ~13 µs fixed |

### Outside the Tick Path (Median)

| Function | Median | Notes |
|---|---:|---|
| `a2c_loss` (batch 32, architecture 15→64→64→{4,1}) | **284.7 µs** | Per optimizer step, on **learner thread** — outside frame budget |
| `WorldArtifact::to_bytes` (256²) | 1.46 ms | Artifact ~1.05 MiB |
| `WorldArtifact::from_bytes` (256²) | 1.13 ms | |
| `WorldArtifact::checksum` (256²) | 1.37 ms | |

---

## Three Things These Numbers Say

### 1. Frame Budget — A Lower Bound, Not Full Frame Time

Summing per-tick systems at 1,000 agents:

| Component | µs |
|---|---:|
| `step_regrowth_gated_strided` | 55.0 |
| `step_water` | 271.5 |
| `step_soil` | 47.2 |
| `step_erosion` | 20.1 |
| `integrate_physics_system` | 4.9 |
| `rebuild_spatial_grid_system` | 94.3 |
| **Total** | **≈ 493 µs** |

≈ **3.0%** of a 16.67 ms frame. At 10,000 agents: ≈ 1.18 ms ≈ **7.1%**.

**This is a lower bound and must be read strictly as such.** It excludes brain inference, ECS scheduling, change detection, thread telemetry emission, collisions, CPG, metabolism, and everything absent from the table. A "60 FPS" claim **cannot** be inferred from this sum alone.

Linearly extrapolating by the measured slope/agent up to the EB-S12 population ceiling (~46,500 agents): physics ≈ 229 µs, spatial ≈ 3.40 ms, fields ≈ 394 µs → **≈ 4.02 ms ≈ 24%** of a frame. This is an **extrapolation, not a measurement** — the spatial hash grid uses fixed cell counts so per-cell agent density grows with N, and behavior outside the measured range is not guaranteed.

### 2. Cost Is Not Where People Usually Guess

`integrate_physics_system` is **cheap**: 4.92 ns/agent, scaling cleanly linearly over three orders of magnitude. At 10,000 agents it takes 49 µs — 0.3% of a frame.

`rebuild_spatial_grid_system` costs **nearly 15× more** at the same agent count (734 µs vs 49 µs). Its scaling shape also differs: 134 ns/agent at 100 agents vs 73 ns/agent at 10,000 agents — indicating a significant fixed portion at small scale comes from **clearing pre-allocated grid buckets**, not agent work. That is the first hotspot worth optimizing, not the physics solver.

`step_water` alone costs more than all other field passes combined, and more than physics at 10,000 agents.

### 3. The 4.2 ms Number in `ecology.rs` Did Not Reproduce Here

The doc comment on `ResourceField::REGROWTH_STRIDE` stated that the pre-stride regrowth path ran four passes per tick, **"measured ~4.2 ms/tick — a quarter of a 60 FPS frame budget"**.

Re-measured on this machine in release mode: `step_regrowth_gated` = **0.219 ms**. Adding the two `total_biomass()` passes needed by the old path (at most `step_regrowth` each, ~71 µs) yields **≈ 0.36 ms** — roughly **12× lower** than the doc comment's claim.

Keep these two points distinct, because conflating them invites wrong conclusions:

- **Striding itself is valid and genuinely faster.** Measured **3.97×** speedup. Nothing here calls for reverting it.
- **Its motivating headline figure did not reproduce.** That alone does not prove the doc wrong: the earlier figure could come from a debug build, a slower machine, or additional work in the same tick. Per Rule 6 of [documentation policy](../governance/DOCUMENTATION_POLICY.md), treat this as an **open finding for verification**, not a settled error.

**Counter-check 2026-11-17: 4.2 ms came from a debug build, not release.**
- Bench `tick_systems` now retains the exact old path: `pre_stride_gated_with_biomass_sandwich/256x256`, running the gated step sandwiched between two `total_biomass()` passes. Command: `cargo bench --bench tick_systems -- "tick/resource_field"`; medians read from `estimates.json`.
- Results on Apple M5 (10 cores, macOS), rustc 1.98.1:

  | Function | Median |
  |---|---:|
  | `step_regrowth` | 60.6 µs |
  | `step_regrowth_gated` | 154.5 µs |
  | `pre_stride_gated_with_biomass_sandwich` (old path) | **217.7 µs** |
  | `step_regrowth_gated_strided` | **39.0 µs** |

  Striding outperforms the old path by **5.6×**.
- That same old path in a **debug build** took **1.9 ms** (median over 300 iterations, timed with `Instant` in a temporary uncommitted test) on the same machine — ~7× slower than release.
- The 4.2 ms figure matches a debug measurement on a slower machine, whereas no release build produces that number. The doc comment on `REGROWTH_STRIDE` was updated accordingly: striding remains beneficial, with only the numerical rationale corrected.

## In-app tick capture (2026-07-27)

Criterion measures **individual functions** on synthetic fixtures outside any running engine. What it structurally cannot measure is **one full tick of the live schedule**: ECS scheduling, change detection, inference, and telemetry publishing. [`src-tauri/src/core/tick_capture.rs`](../../src-tauri/src/core/tick_capture.rs) measures exactly that, **without** running the desktop app.

Enabled via an environment variable at engine startup or via four IPC commands (`start_tick_capture` / `get_tick_capture_status` / `stop_tick_capture` / `export_tick_capture` — see [`IPC_CONTRACT.md`](../reference/IPC_CONTRACT.md)):

```bash
ANIMA_TICK_CAPTURE=warmup=300,capacity=1800,every=2
```

Three phases are **exact** because the simulation loop brackets them directly with `Instant`: `schedule` (around `schedule.run`), `telemetry_publish` (around telemetry extraction/publishing), and `full_tick` (sum of both, **excluding** frame-pacing sleep).

Four remaining phases (`sensor_brain`, `physics_movement`, `ecology_resources`, `schedule_tail`) are **checkpoint-bounded**, and that distinction lives inside the exported JSON rather than only in prose: Bevy exposes no per-system timing hook, and splitting the schedule to time systems separately would **alter execution order** — the one thing a profiler must never do. Each checkpoint only guarantees running **after** a named system; such a phase is the executor work between two checkpoints. `PhaseSummary.exact` states which kind a row belongs to, and `CaptureExport.executor` records which executor produced the data.

Quantile convention is **nearest rank** (`ceil(p/100·n) − 1`), so every reported figure is an actual sample rather than interpolated. `plant_soil_weather` appears under `unavailable` with a reason, because `core::dynamic_fields` is **not** in the live schedule — reporting 0 for it would be fabricated.

**Measured once in the full desktop app — 2026-07-27.** The project owner ran the checklist below and collected 1800 samples: `full_tick` p50 **1642.2 µs**, **debug** build, multi-threaded executor, 256², **10 agents**. Full numbers and context live in [`BENCHMARK_BASELINE.md` § In-app measurement](../../BENCHMARK_BASELINE.md#đo-trong-app--2026-07-27-bản-debug).

**What is still missing:** a **release** run and a run at **1,000 agents**. The run above used debug with 10 agents, so it closes three rows in the baseline table and **does not** close `Full-brain agents MVP`. Headless evidence keeps its full validity — identical schedule (`simulation_schedule::build_tick_schedule`, the exact function `SimulationEngine::start` calls) via `tests/tick_capture_tests.rs`.

The measurement procedure requires **a human opening the app**, documented under [§ One-run owner checklist](#one-run-owner-checklist) below. `CLAUDE.md` forbids agents from running the full desktop backend on this dev machine, so no automated session may execute it — the checklist exists for the owner to run directly.

## One-run owner checklist

> **ONE launch. ONE session. About 5 minutes.** This session captures three items no repo gate can obtain because all three require a human opening the app:
>
> | # | What Is Collected | What It Closes |
> |---|---|---|
> | (a) | Does the app boot cleanly under CSP with zero console violations? | [deployment §2.1](../ai/deployment/2026-07-27-feature-anima-completion.md) |
> | (b) | One exported tick-capture file | the `unmeasured` rows in [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md) |
> | (c) | One screenshot | visual evidence for (a) |
>
> Do not run three separate times for three items — all three happen inside **one app window**.
>
> ⚠️ **This session does NOT prove release readiness.** It only fills missing evidence. Read [§ What this session closes and does not close](#what-this-session-closes-and-does-not-close) before quoting any number.

### Step 0 — Prepare BEFOREhand (not counted toward the 5 minutes)

The first backend compilation (Bevy + Burn) takes tens of minutes. Running `npm run tauri:dev` straight on a cold `target/` invalidates the "5 minutes" expectation. Pre-warm from repo root and then `src-tauri/`:

```powershell
npm install
npm run build
```

```powershell
cargo build --features desktop
```

`cargo build` here **only compiles, never launches the app** — it does not violate the warning at the top of [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md). It warms the dependency tree; when `tauri dev` runs, only the `anima-engine` crate recompiles (`generate_context!` switches dev/production mode), so most compile time is already paid.

Also ensure port **5173** is free (`strictPort`), because `beforeDevCommand` spawns `npm run dev`.

> ⚠️ **Most time-consuming trap from the 2026-07-27 run, now fixed in config — still check the port.** That day `devUrl` used `http://localhost:5173`, and Node resolved `localhost` *verbatim* since v17 — `::1` first on both Windows and Linux. `[::1]:5173` was occupied by **another project's Vite dev server** (5173 is Vite's default so every project wants it), while Anima's server bound `127.0.0.1:5173`. Both servers lived side by side, and the webview loaded **the other project's app inside Anima's window**: blank page, zero errors — dev server reported `ready`, served 200, and CSP permitted `http://localhost:5173` because both servers shared that host.
>
> `devUrl` and `vite.config.ts` now both pin `127.0.0.1`, removing hostname resolution ambiguity. What has **not** disappeared is `strictPort`: if port 5173 is busy, `npm run dev` exits at `beforeDevCommand`. Check beforehand, reading **both the address and owning process**:
>
> ```powershell
> Get-NetTCPConnection -LocalPort 5173 -State Listen | Select-Object LocalAddress, OwningProcess
> ```
>
> Once the window appears, verify identity before trusting anything drawn: `window.__TAURI_INTERNALS__ !== undefined` must be `true`, and the top page heading (h1) must read `"Anima-Engine Control Center"` (the window title is `"Anima Engine - Evolution Simulator"`). Two seconds, and it is the only guarantee this is "your app" rather than "some other app at that address". See [deployment §2.4](../ai/deployment/2026-07-27-feature-anima-completion.md).

### Step 1 — Set the environment variable, then open the app (PowerShell, same window)

This variable is read by the **Rust process** in `SimulationEngine::start` ([`core/simulation_loop.rs`](../../src-tauri/src/core/simulation_loop.rs)), so set it **before** launch — setting it afterward has no effect.

```powershell
$env:ANIMA_TICK_CAPTURE = "warmup=300,capacity=1800,max_samples=1800"
npm run tauri:dev
```

Why those exact three parameters: 300 warm-up ticks ≈ **5 s** at 60 Hz (discard cold ticks), then 1800 consecutive samples ≈ **30 s**. At 1800 samples, capture enters `Complete` and **stops consuming ticks** — a sharp stop condition rather than guessing elapsed time. Total ≈ 2100 ticks ≈ 35 s wall clock. Full syntax and keys: `CaptureConfig::parse` in [`core/tick_capture.rs`](../../src-tauri/src/core/tick_capture.rs).

- ✅ **Good:** `"Anima Engine - Evolution Simulator"` window opens.
- ❌ **Bad:** terminal prints `ANIMA_TICK_CAPTURE is not a usable configuration (...)` → capture is **disabled**; the app runs normally and yields an empty file. Fix the string and relaunch.

### Step 2 — Open DevTools and **reload** to catch load-time CSP violations

`npm run tauri:dev` builds in debug mode so webview DevTools are enabled: right-click inside the app window → **Inspect**.

DevTools opened *after* initial page load **miss** initial load logs. Therefore, run in the **Console** tab:

```js
location.reload()
```

Reloading refreshes only the webview; the Rust backend and simulation thread **do not** restart, leaving capture undisturbed. Console is now watching a complete page load.

- ✅ **Good:** `"Anima-Engine Control Center"` UI loads with **zero** lines in Console containing `Content Security Policy` / `Refused to …`.
- ❌ **Bad:** any `Refused to load/connect/execute …` message → copy verbatim; catching that is why this session exists.
- ❌ **Bad:** `window.__TAURI_INTERNALS__ === undefined` → you are viewing `localhost:5173` in a plain browser, **not** the Tauri webview. Every `invoke` rejects; this matches the issue noted in [implementation §9](../ai/implementation/2026-07-27-feature-anima-completion.md).

### Step 3 — Start the simulation (capture begins recording ticks here)

Large button on the left below the header:

| Button Label | Meaning | Action |
|---|---|---|
| `Đang dựng thế giới chung…` ("Building shared world…", gray, disabled) | `2048²` world is generating (~7 s first time) | **wait** |
| `Bắt đầu mô phỏng` ("Start simulation", green) | ready | **click** |
| `Dừng mô phỏng` ("Stop simulation", red) | running | leave running |

Click and wait **~40 seconds**. Watch the **`Số Ticks:`** ("Tick count:") field in the status panel — it should increment steadily by **~2100** after clicking. (Track the delta, not absolute value: if autosave resumed, the counter continues from the saved tick.) Definitive completion is verified in Step 4.

> If the app auto-started on launch (an existing `saves/autosave.json` causes [`lib.rs`](../../src-tauri/src/lib.rs) to start the engine at setup), capture has been running since startup and `Số Ticks:` is already high. That is fine — continue to Step 4 and inspect status.

### Step 4 — Verify capture finished in Console

There is no UI button for tick capture: all four commands are **pure IPC** with no frontend caller yet (verified: no file under `src/` mentions `tick_capture`). Console is the only interface. `@tauri-apps/api` routes every `invoke` through `window.__TAURI_INTERNALS__.invoke(cmd, args)` — the same boundary implemented by the real webview and mirrored in [`tests/e2e/tauri-mock.ts`](../../tests/e2e/tauri-mock.ts).

```js
await window.__TAURI_INTERNALS__.invoke('get_tick_capture_status')
```

- ✅ **Good:** `status: "Complete"` and `accounting.samples_recorded: 1800`.
- 🟡 **Acceptable:** `status: "Recording"` with several hundred `samples_recorded` — wait briefly and re-check. Any sample count > 0 exports cleanly, just fewer samples.
- ❌ **Bad:** `status: "Idle"` → **two distinct causes, separated by `accounting.ticks_observed`.** The `ANIMA_TICK_CAPTURE` parser runs **inside** the engine thread ([`core/simulation_loop.rs`](../../src-tauri/src/core/simulation_loop.rs), at `CaptureConfig::from_env()`), so it only executes when the engine boots:
  - `ticks_observed: 0` → **engine never started**. You have not clicked `Bắt đầu mô phỏng` yet, not an env-var typo. Return to Step 3. *(Observed 2026-07-27: this is what actually happened, and an earlier version of this guide only listed the other cause so operators thought they mistyped the string.)*
  - `ticks_observed` > 0 → engine ran, **env var did not reach the process**; check Step 1. No need to restart — use the IPC path below.
- ❌ **Bad:** `workload.dimensions_measured: false` → zero ticks traversed the loop, same cause as `ticks_observed: 0`.
- ❌ **Worth recording:** `dropped_out_of_order` > 0 → multi-threaded executor reordered checkpoints. Not user error; **copy that number down** — it is empirical data on the executor itself. *(Observed 2026-07-27: **1** across 2101 ticks.)*

#### IPC Path — No Env Var, No Restart Needed

The env var in Step 1 is just convenience when set **before** launch. If forgotten, or to re-run with different parameters, `start_tick_capture` configures it **live mid-run** — the sink is always inserted precisely for this reason, avoiding world reconstruction:

```js
await window.__TAURI_INTERNALS__.invoke('start_tick_capture', { config: { warmup_ticks: 300, capacity: 1800, max_samples: 1800, sample_every: 1, groups: 127 } })
```

Returning `null` means success (`Result<(), String>`); a string indicates a validation error from `CaptureConfig::validate`, never silent clamping.

Two silent footguns in that payload object:

- **Fields inside `config` are `snake_case`.** `CaptureConfig` does not carry `#[serde(rename_all)]`, so `warmupTicks` is ignored/invalid — unlike `fileName` in Step 5, which is a **command argument** converted to camelCase. One command invocation can mix both conventions intentionally.
- **`groups` is an integer, not an array.** `PhaseMask(pub u16)` is a newtype serialized as a raw integer in JSON. `127` = all seven phases (`TickPhase::ALL`, indices 0–6). An empty mask is rejected by `validate`; a narrower mask **never alters** other phases' timings, only what is exported.

If the engine is paused, start it from Console; `toggle_simulation` takes no arguments and returns `true` when starting:

```js
await window.__TAURI_INTERNALS__.invoke('toggle_simulation')
```

### Step 5 — Export file (note: argument key is `fileName`, NOT `file_name`)

```js
await window.__TAURI_INTERNALS__.invoke('export_tick_capture', { fileName: 'tick-capture-2026-07-27' })
```

> ⚠️ **Silent trap — the only hazardous trap here.** `#[tauri::command]` renames arguments to **camelCase** (default `ArgumentCase::Camel`, `tauri-macros/src/command/wrapper.rs`), and the parameter is `Option<String>` — so passing `file_name` raises **zero error**: the command returns the JSON document nicely in Console while **writing nothing to disk**. Green Console, empty folder. It must be `fileName`.
>
> Filenames follow [`commands/save_paths.rs`](../../src-tauri/src/commands/save_paths.rs): `[A-Za-z0-9._-]` only, no slashes, no paths; `.json` is appended automatically.

The file lands under app data, **not** inside the repo:

```powershell
Get-ChildItem "$env:APPDATA\com.anima.engine\captures"
```

- ✅ **Good:** shows `tick-capture-2026-07-27.json`, a few KB in size.
- ❌ **Bad:** directory empty or missing → almost certainly typed `file_name`. Re-run Step 5 with `fileName`.

Inspect key columns quickly without opening an editor:

```powershell
(Get-Content "$env:APPDATA\com.anima.engine\captures\tick-capture-2026-07-27.json" -Raw | ConvertFrom-Json).phases | Format-Table phase, exact, count, p50_ns, p95_ns
```

`p50_ns` fills [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md) according to [§ Which baseline row this file fills](#which-baseline-row-this-file-fills).

### Step 6 — Take one screenshot **before** closing the app

`Win`+`Shift`+`S`, capturing both the app window **and** the DevTools panel in one frame showing: window title, `Số Ticks` > 2100, and Console clean of CSP violations.

Save anywhere outside the repo, e.g. `%USERPROFILE%\Desktop\anima-csp-boot-2026-07-27.png`.

> **There is no committed repo location for this screenshot** and this checklist intentionally adds none. Never drop it into [`map-views/`](../../map-views): the eight PNGs there are pinned by SHA-256 in `map_manifest.json` and enforced by `tests/frontend/mapManifestEvidence.test.ts` — adding files there breaks a green CI gate.

### Step 7 — Close the app and send four artifacts back

Close the window normally (on exit the app writes an autosave — expected behavior, not an error).

Send back to the agent:

1. **File** `%APPDATA%\com.anima.engine\captures\tick-capture-2026-07-27.json` — unmodified.
2. **Screenshot** from Step 6.
3. **Console text**: verbatim any red/yellow lines, or confirmation `"Console clean, zero lines mentioning Content Security Policy"`.
4. **Output of `get_tick_capture_status`** from Step 4 (`status` and `accounting` suffices).

With all four, an agent updates `BENCHMARK_BASELINE.md` and [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền) with command and date. Without (1), no numbers exist; without (3), item (a) remains open.

### Which baseline row this file fills

| Row in `BENCHMARK_BASELINE.md` | Read From | `exact` |
|---|---|---|
| `Physics tick` | `phases[] .phase == "physics_movement"` → `p50_ns` | **false** — checkpoint-bounded |
| `Brain/sensor` | `phases[] .phase == "sensor_brain"` → `p50_ns` | **false** — checkpoint-bounded |
| `UI telemetry` | `phases[] .phase == "telemetry_publish"` → `p50_ns` | **true** |
| `Full-brain agents MVP` | `phases[] .phase == "full_tick"` → `p50_ns` + `mean_ns_per_agent`, plus `workload` | **true** |

Three annotations must accompany every citation so numbers are not misread:

- **`exact: false` means "executor work between two checkpoints"**, not isolated single-system runtime. Carry that flag alongside `p50_ns`.
- **`profile` in this capture is `"debug"`**, because `tauri dev` runs debug builds, whereas Criterion above is **release**. Never compare those figures directly without labeling build profile beside each number.
- **Per-agent brains default to off** (`ANIMA_EVOLVED_BRAINS` unset, §3.1), so `sensor_brain` measures the **legacy path**, not neural inference. Label `Brain/sensor` as default legacy path rather than brain inference cost.

### What this session closes and does not close

**Closes:** real desktop app boots, live IPC responds, and the live schedule acquires its first real measurement on target hardware.

**Does NOT close — keep these caveats verbatim:**

- **CSP: this validates `devCsp`, not shipping `csp`.** `tauri dev` applies `app.security.devCsp`; `npm run check:csp` validates `app.security.csp`. Both blocks in [`tauri.conf.json`](../../src-tauri/tauri.conf.json) declare the same 13 directives, identical across **11** and differing in **two**: `script-src` (dev permits `'unsafe-inline'` and `http://127.0.0.1:5173`) and `connect-src` (dev adds `ws://127.0.0.1:5173` + `http://127.0.0.1:5173`). So this verifies 11 shipping directives faithfully while testing those two in a relaxed form.
  **Open finding:** [deployment §2.1](../ai/deployment/2026-07-27-feature-anima-completion.md) wrote that a human `npm run tauri:dev` session closes the CSP gate completely. Under measurement above, that overclaims slightly. Left unedited here per Rule 6 of [documentation policy](../governance/DOCUMENTATION_POLICY.md).
- **Why release cannot fill those two directives either:** release builds **have no DevTools**. `tauri` in [`src-tauri/Cargo.toml`](../../src-tauri/Cargo.toml) sets `features = ["test"]` without `devtools` — and DevTools is the only console reader. A release run still drops (a-console), and those two directives remain verified only in their relaxed form.
  **Update 2026-07-27:** part (b) is **no longer** lost in release — `ANIMA_TICK_CAPTURE_OUT` auto-writes the capture file on completion with no Console needed. See [§ Release run checklist (1000 agents)](#release-run-checklist-1000-agents).
- **Not "60 FPS proven".** One capture is a tick-time distribution for one workload, profile, and machine.
- **Not "live Bevy world experiment-ready".** The sole permitted claim stays *headless adapter verified* — see `CLAUDE.md`.
- **`plant_soil_weather` appears in `unavailable` with reason.** Expected, not a bug: `core::dynamic_fields` is not wired into the live schedule.

<a id="checklist-lần-chạy-release--1000-agent"></a>

## Release run checklist (1000 agents)

> **ONE launch, ~10 minutes plus compile time.** Captures the **one** metric the dev checklist cannot produce: **release-build** tick cost at **1,000 agents**, i.e. row `Full-brain agents MVP` in [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md).
>
> No DevTools and zero JavaScript Console typing required. That ergonomic gap is the sole difference from the dev checklist and why this section exists.

**Why this checklist could not be written before 2026-07-27.** Three code blockers stood in the way: release has no DevTools to invoke `export_tick_capture`; no disk auto-export existed; and founding population was hardcoded to `10` in `simulation_loop.rs` with linear spawn `x = i * 5.0` that pushed founder #21 past the `+100` world boundary. Two new env vars and grid founder placement removed all three blockers.

### Step 0 — Build BEFOREhand (excluded from the 10 minutes)

**Order matters.** Release embeds compiled frontend assets into the binary at build time, so `npm run build` must run **before** `cargo build --release`, otherwise the binary embeds a stale `dist/`.

```powershell
npm run build
```

```powershell
cargo build --release --features desktop
```

Do not omit `--features desktop`: without it you build `default = []` — no Neo4j lineage, no cross-shard migration, CPU learner — measuring a different product.

First release compilation takes noticeably longer than debug. Port **5173** only needs to be free if you plan to start a dev server alongside; the release build does not need it and serves its embedded frontend itself.

### Step 1 — Four environment variables, then launch the binary

Set in the same PowerShell window **before** launching — all four are read when the engine starts:

```powershell
$env:ANIMA_FRESH_START = "1"
$env:ANIMA_FOUNDING_POPULATION = "1000"
$env:ANIMA_TICK_CAPTURE = "warmup=300,capacity=1800,max_samples=1800"
$env:ANIMA_TICK_CAPTURE_OUT = "tick-capture-release-1000"
.\src-tauri\target\release\anima-engine.exe
```

| Variable | Effect | If Left Empty |
|---|---|---|
| `ANIMA_FRESH_START` | Skips loading autosave on startup and skips overwriting it on exit | **Added 2026-11-17, verified in `lib.rs`.** If an autosave exists (left by the dev checklist above), the engine **loads that save**, skips genesis, and ignores `ANIMA_FOUNDING_POPULATION` — leaving you measuring the old population while believing it is 1,000 |
| `ANIMA_FOUNDING_POPULATION` | Spawns 1,000 founders on a grid within `MapBounds` ±100 keeping 7/3 prey/predator ratio | Spawns the legacy 10 founders on a line — **bit-identical** to all prior runs |
| `ANIMA_TICK_CAPTURE` | Starts tick capture at engine init | No capture; only toggleable via IPC if DevTools exist (release has none) |
| `ANIMA_TICK_CAPTURE_OUT` | **Auto-writes capture file upon completion.** Only export path in release | Nothing written to disk; 1800 samples die in RAM on exit |

`ANIMA_TICK_CAPTURE_OUT` accepts a **filename stem**, not a path — matching save-name rules (`[A-Za-z0-9._-]`, no slashes). Paths are rejected by `save_paths`, printing an error on stderr rather than failing silently.

### Step 2 — Click `Bắt đầu mô phỏng` ("Start simulation"), then wait

With `ANIMA_FRESH_START=1`, the engine skips autosave and stays paused at boot. Click the green start button and wait. (Set `ANIMA_AUTOSTART=1` as well to auto-start without clicking.)

**Do not count 35 seconds as in the dev checklist.** At 1,000 agents each tick costs many times more, so 2100 ticks take much longer — and that figure is precisely what you are measuring, so guessing beforehand is pointless. The stop signal prints to terminal:

```
tick capture complete; wrote C:\Users\...\com.anima.engine\captures\tick-capture-release-1000.json (1800 samples)
```

That line prints **once** when capture fills. Seeing it means recording is finished.

- ❌ **Bad:** `ANIMA_TICK_CAPTURE is not a usable configuration (...)` → capture off; fix string and rerun.
- ❌ **Bad:** `ANIMA_FOUNDING_POPULATION is not usable (...)` → **genesis ran with 10 founders**. Close app, fix, rerun; do not keep measuring, as the numbers will be for 10 agents.
- ❌ **Bad:** `tick capture complete but ANIMA_TICK_CAPTURE_OUT could not be written (...)` → 1800 samples completed but could not flush to disk. Release has no Console recovery; fix name and rerun.

### Step 3 — Verify the file measured the intended workload

```powershell
$j = Get-Content "$env:APPDATA\com.anima.engine\captures\tick-capture-release-1000.json" -Raw | ConvertFrom-Json
"profile  : " + $j.profile
"executor : " + $j.executor
"world    : {0} x {1} (measured={2})" -f $j.workload.world_width, $j.workload.world_height, $j.workload.dimensions_measured
$ft = $j.phases | Where-Object { $_.phase -eq 'full_tick' }
"agents   : {0:N2}" -f ($ft.mean_ns / $ft.mean_ns_per_agent)
$j.phases | Format-Table phase, exact, count, p50_ns, p95_ns
```

All four checks must pass together:

- `profile` is **`release`**. `"debug"` means the wrong binary ran — incomparable to Criterion.
- `agents` reads **≈ 1000**. This crucial back-check derives from recorded data (`mean_ns / mean_ns_per_agent`), catching cases where an env var failed to reach the process while everything else looked green.
- `dimensions_measured` is `true`, world `256 × 256`.
- `count` of every phase equals **1800**.

> ⚠️ **If `agents` drops well below 1000:** the population died during capture. On 2026-07-27 all agents hit `Năng lượng: 0.0` ("Energy: 0.0") — the floor effect recorded in experiment E2. If population varies over capture, `mean_ns_per_agent` averages across a shrinking cohort and must carry that disclosure. Also inspect `samples_without_agents` — non-zero means some ticks had zero agents.

### Step 4 — Return Artifacts

Return the unmodified JSON file plus stderr output from Step 2. No screenshot needed: release skips CSP verification covered by the dev checklist.

Row mapping matches [§ Which baseline row this file fills](#which-baseline-row-this-file-fills), and this capture legitimately closes `Full-brain agents MVP` because it runs at 1,000 agents.

## Post-Branch-H Measurement Checklist — PF-S03 + PF-S04 (Drafted 2026-08-26, **Not Yet Run**)

Branch H (`0b29312`) rewrote seven systems from `O(agent × segment)` into **one** segment pass each. PF-S01 closed with the strongest automated check possible: checksum after 1500 ticks at seed `0x5EED` stayed **`4013327208`** before and after H — bit-identical. However, **no runtime speedup measurement exists yet**, leaving two human-operated gates open:

| Gate | Question | Answered in a Single Run? |
|---|---|---|
| **PF-S03** | `full_tick` p50 at 1,000 agents, release build | Yes |
| **PF-S04** | Per-tick cost scales **linearly** with agent count, not quadratically | **No** — needs at least three points |

PF-S04 is the sharper gate: PF-S01 catches bugs in what you modified, whereas PF-S04 catches **what you missed** — a single remaining O(N²) loop bends the scaling curve even while every checksum agrees.

Therefore: **run three times in one session, on one machine, varying only `N`.**

> 🖥️ **Record hardware accurately.** `BENCHMARK_BASELINE.md` defines target hardware as **i5-14600KF / Windows 11 Pro**. Measurements on a macOS dev machine remain valid, as a **distinct hardware row** — never overwrite target-hardware rows with them. JSON exports embed `hardware` automatically; simply avoid merging two hosts into one row.

### Step 0 — Build release

```powershell
cargo build --release --features desktop
```

Never drop `--features desktop` (see [§ Release run checklist (1000 agents)](#release-run-checklist-1000-agents)).

### Step 1 — Three runs, N = 250 / 500 / 1000

For **each** N, set variables, run binary, click `Bắt đầu mô phỏng` ("Start simulation"), wait for `tick capture complete`, close app, then move to the next N. Never run two instances concurrently — CPU contention turns scaling slopes into noise.

```powershell
# repeat this block three times for N = 250, 500, 1000
$N = 250
$env:ANIMA_FRESH_START = "1"   # otherwise, the previous run's autosave is loaded and N has no effect
$env:ANIMA_FOUNDING_POPULATION = "$N"
$env:ANIMA_TICK_CAPTURE     = "warmup=300,capacity=1800,max_samples=1800"
$env:ANIMA_TICK_CAPTURE_OUT = "h5-release-$N"
.\src-tauri\target\release\anima-engine.exe
```

```bash
# macOS/zsh — same variables, different binary path
for N in 250 500 1000; do echo "run N=$N then click the button, wait for 'tick capture complete', close app"; done
N=250
ANIMA_FRESH_START=1 \
ANIMA_FOUNDING_POPULATION=$N \
ANIMA_TICK_CAPTURE="warmup=300,capacity=1800,max_samples=1800" \
ANIMA_TICK_CAPTURE_OUT="h5-release-$N" \
  ./src-tauri/target/release/anima-engine
```

**Do NOT set `ANIMA_MORTALITY` or `ANIMA_FITNESS`.** Both default to off/`locomotion`, and this measures a pure refactor — enabling mortality or switching fitness mid-series confounds two changes. JSON files record `fitness_rule`; Step 2 checks all three runs match.

Three error signals identical to the 1000-agent checklist: `ANIMA_TICK_CAPTURE is not a usable configuration`, `ANIMA_FOUNDING_POPULATION is not usable` (genesis ran with 10 founders — close app, discard), and `tick capture complete but ANIMA_TICK_CAPTURE_OUT could not be written`.

### Step 2 — Inspect all three files before trusting numbers

Capture directory: `%APPDATA%\com.anima.engine\captures\` (Windows) or `~/Library/Application Support/com.anima.engine/captures/` (macOS).

```bash
python3 - <<'PY'
import json, glob, os, sys
d = os.path.expanduser("~/Library/Application Support/com.anima.engine/captures")
rows = []
for n in (250, 500, 1000):
    p = os.path.join(d, f"h5-release-{n}.json")
    if not os.path.exists(p):
        print(f"MISSING: {p}"); continue
    j = json.load(open(p, encoding="utf-8"))
    ft = next(x for x in j["phases"] if x["phase"] == "full_tick")
    w  = j["workload"]
    rows.append((n, j["profile"], j.get("fitness_rule"), w.get("agent_count_min"),
                 w.get("agent_count_max"), ft["count"], ft["p50_ns"], ft["p95_ns"],
                 ft.get("samples_without_agents")))
hdr = ("N","profile","fitness_rule","agents_min","agents_max","count","p50_ns","p95_ns","no_agent_samples")
print("\t".join(hdr))
for r in rows: print("\t".join(str(x) for x in r))
PY
```

**Six validity criteria — failing any one invalidates the run:**

1. `profile` == `release` across all three. `debug` means wrong binary.
2. `fitness_rule` **identical** across all three runs.
3. `agents_min` ≈ `agents_max` ≈ N in every run. Checks env-var propagation and catches mid-run die-offs (mortality is off, though starvation/dehydration floor effects from E2 still trigger).
4. `count` == 1800 across all three.
5. `no_agent_samples` == 0.
6. `workload` world **256 × 256**, `dimensions_measured` == `true`.

### Step 3 — Verdict Using Pre-Registered Thresholds

Let `t(N)` be `full_tick` `p50_ns`.

**PF-S03 passes** when `t(1000)` is recorded from a run passing all six criteria above — it is a recorded baseline figure rather than a fixed target, answering `Full-brain agents MVP` in [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md).

**PF-S04** requires two checks agreeing on linearity:

- **End-to-end ratio** `R = t(1000) / t(250)`. Pure linear gives `R = 4`; fixed per-tick overhead (256² fields, scheduling — N-independent) yields `R < 4`. Pure quadratic yields `R = 16`.
- **Midpoint convexity.** Linear interpolation at N=500 is `t_i = (t(250) + t(1000)) / 2`. Linear functions match `t_i` directly; a pure quadratic sits **33% below the chord** at `t(500)`.

| Result | Verdict |
|---|---|
| `R ≤ 6` **and** `\|t_i − t(500)\| / t(500) ≤ 15%` | **PF-S04 PASS** — linear |
| `R ≥ 10` **or** `t(500)` sits >25% below `t_i` | **PF-S04 FAIL** — remaining per-agent hotspot |
| anything else | **INCONCLUSIVE** — report raw values without claiming the gate; first suspect is a system still scanning linearly per agent |

These thresholds are **pre-registered criteria, not measurements** — written here before running to prevent post-hoc threshold moving.

### Step 4 — Where the Numbers Go

1. Keep and attach all three raw JSON files unmodified.
2. `BENCHMARK_BASELINE.md`: populate `Full-brain agents MVP` with `t(1000)` (naming host hardware).
3. `FINDINGS.md` entry **F-022**: update "Remaining H-5" with measured `R` and midpoint deviation.
4. `docs/ai/testing/2026-08-25-feature-ecological-selection.md`: mark PF-S03/PF-S04.
5. If PF-S04 fails: open a new finding naming the suspect system rather than patching in the same commit — unmeasured optimization patches cannot prove before/after improvement.

## What is still missing

Target hardware now matches, yet §3.2 of [`STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md) remains **open**, not because of hardware:

- **Per-system sums are lower bounds, not live frame times.** Rows `"Physics tick 60 Hz"` and `"Brain/sensor 10–20 Hz"` in [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md) ask for **real running-app cadence**. The measurement harness exists (above) and passes unit tests; what remains is **one real app run** for empirical numbers — the agent ban on launching the full backend remains in force, so see [§ One-run owner checklist](#one-run-owner-checklist).
- **Still unmeasured for the most expensive remaining system:** per-agent brain inference. It defaults to off (§3.1), leaving no default-path baseline yet.
- *(Edited 2026-11-17: `config.gridDim` in the report is now **256**; the paragraph below records the prior state.)*
  `config.gridDim` in [`benchmark_report.json`](../../benchmark_report.json) used to read **128**, whereas the real world runs **256²** (`MapSettings::default()`) and `DEFAULT_GRID_DIM` in `sim_rules.rs` **has been 256** since 2026-07-27 (`s03_default_grid_dim_tracks_map_settings_default`). This benchmark suite runs 256², and tick capture reads dimensions directly **from the live `ResourceField`** into `CaptureExport.workload` with `dimensions_measured` — so `128` lived only in the older report JSON, never in active measurement code.

## How to Add a Benchmark

Edit [`src-tauri/benches/tick_systems.rs`](../../src-tauri/benches/tick_systems.rs), keeping the four existing conventions:

1. **Use real world dimensions.** Fields run at 256²; measuring at 32² yields artificially comfortable numbers for a nonexistent workload.
2. **Import engine constants, never copy.** Learner benches import `STATE_DIM`/`HIDDEN_DIM`/`ACTION_DIM`/`BATCH_SIZE` from `core::training`. Hardcoding lets benchmarks keep printing numbers for an obsolete network after architecture changes.
3. **Construct fixtures at realistic operating states.** `from_biomes` initializes every cell **at** carrying capacity where logistic regrowth at `r == r_max` equals zero — measuring an early-return branch instead of regrowth. Lower initial biomass to half capacity here.
4. **Never hoist allocations the engine itself does not hoist.** `a2c_loss` constructs tensors every iteration because the learner allocates new ones from transition buffers on every step.

Benchmarks are compiled and linted under `cargo clippy --all-targets` across **both** feature configurations in CI, so benchmark code must pass clippy cleanly and compile under `--no-default-features`.
