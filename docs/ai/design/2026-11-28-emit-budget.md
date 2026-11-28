---
phase: design
feature: emit-budget
title: EMIT-BUDGET — Tauri IPC Emit Payload Measurements and Reduction Options
description: Design proposal measuring headless Tauri event payloads (simulation-tick, raycast-update, pheromone-update, combat-event) at 10, 100, and 1,000 agents and evaluating five mitigation options with IPC contract impacts and risks.
status: proposed
owner: anima-case
last_reviewed: 2026-11-28
---

# `EMIT-BUDGET` — Tauri IPC Emit Stream Budget & Reduction Options

> **Status: `proposed` (design options proposal only — zero implementation code).**
> Triggered by `START_2026-11-28_AM.md` (`N1 EMIT-BUDGET`) to address the `WEBVIEW-MEM` finding (`~36.6 MB/s` JSON / `~73 MB/s` UTF-16 `ICoreWebView2::ExecuteScript` flood). All codebase anchors and byte measurements below were verified on trunk `origin/feat/ecological-selection` at `283bd659e68578a3eb2fb96981d74d11bee3308d`. The owner chooses among the options below; nothing in this proposal alters production code or the IPC contract yet.

---

## 1. Problem Statement & Current Architecture

In the desktop build, [`spawn_emit_thread`](../../../src-tauri/src/core/emit.rs) publishes simulation telemetry unconditionally from a background thread to every webview window whenever `app_handle.is_some()`:

| Event | Source & Line | Emit Cadence | Conditioning / Gate Today | Current Frontend Listeners |
|---|---|---|---|---|
| `simulation-tick` | [`emit.rs`](../../../src-tauri/src/core/emit.rs#L296-L303) | Every `EMIT_INTERVAL = 33 ms` (`30.30 Hz`) | Unconditional (`refresh_tick_payload`) | [`App.tsx`](../../../src/App.tsx#L461-L505), [`PixiViewport.tsx`](../../../src/PixiViewport.tsx#L1171-L1192), [`LiveAgents.tsx`](../../../src/components/Landscape/LiveAgents.tsx#L324), [`LiveEnvironment.tsx`](../../../src/components/Landscape/LiveEnvironment.tsx#L211), [`tickRecording.ts`](../../../src/components/Landscape/utils/tickRecording.ts#L534) |
| `raycast-update` | [`emit.rs`](../../../src-tauri/src/core/emit.rs#L313-L318) | Every `EMIT_INTERVAL = 33 ms` (`30.30 Hz`) | Unconditional (`active_raycasts`) | [`App.tsx`](../../../src/App.tsx#L326-L330), [`PixiViewport.tsx`](../../../src/PixiViewport.tsx#L1194-L1200) (unused in 3D Landscape) |
| `pheromone-update` | [`emit.rs`](../../../src-tauri/src/core/emit.rs#L305-L311) | Up to `PHEROMONE_EMIT_INTERVAL = 100 ms` (`10.0 Hz`) | [`PheromoneEmitGate`](../../../src-tauri/src/core/emit.rs#L123-L187) (skips exact-unchanged `f32` grids) | [`App.tsx`](../../../src/App.tsx#L321-L325), [`PixiViewport.tsx`](../../../src/PixiViewport.tsx#L1203-L1210) (unused in 3D Landscape) |
| `combat-event` | [`emit.rs`](../../../src-tauri/src/core/emit.rs#L322-L324) | Per combat event per `33 ms` wakeup | Swapped out of `combat_events` (zero-alloc drain when headless) | [`App.tsx`](../../../src/App.tsx#L331-L335) (truncated to latest `50`) |

### Why WebView2 Stalls (`WEBVIEW-MEM` Diagnosis)
1. **Unbounded `ExecuteScript` string injection (~2x UTF-16 expansion):** Tauri v2 `handle.emit(...)` serialises Rust structs into UTF-8 JSON and dispatches scripts into the WebView2 UI queue (`ICoreWebView2::ExecuteScript`) as UTF-16 strings without checking whether the webview consumed prior frames.
2. **Double-buffering + V8 parse pressure:** Under initial shader compilation, `worldGen.worker` bursts, or landing-page DOM updates (`App.tsx`), any UI-thread pause causes the `EMIT` thread to backlog serialized JSON strings (`~40.1 MB/s` UTF-8 -> `~80.2 MB/s` UTF-16 at 1,000 agents with active pheromones), directly accounting for the Dell Iris Xe `1.0 GB -> 4.5 GB -> 2.4-3.8 GB` renderer working-set surge recorded in `anima-mac_WEBVIEW-MEM.md`.
3. **Unnecessary fan-out to inactive views:** When the user opens `LandscapeShowcase` (`showLandscape` / `showMap2`), `App.tsx` listeners and 2D Pixi telemetry streams stay active even though neither `LiveAgents.tsx` nor `LiveEnvironment.tsx` reads `raycast-update` or `pheromone-update`. Meanwhile `SimulationTickPayload` resends unchanged static segment bones (`parent_segment_id`, `length`, `radius`, `joint_anchor_*`, `joint_axis_*`, `agent_type`), unused `joint_axis_*` fields, unchanged `environmental_state` (`55` static tree/lake strings + floats), and a `head_directions` map that duplicates `root.head_direction`.

---

## 2. Headless Payload Measurements (`10`, `100`, and `1,000` Agents)

### 2.1 Method
Measurements were taken headlessly on trunk `283bd659` (`Windows 1` / `DESKTOP-3EPV830`, `rustc 1.98.1`, `ANIMA_DETERMINISTIC=1`, `ANIMA_SEED=42`, `ANIMA_USE_GPU=0`, `tauri::test::MockRuntime`) by booting `SimulationEngine::start`, sampling live `agent_states`, `environmental_state`, `pheromone_grid_state`, and `active_raycasts` at `t1_tick = 10` and `t2_tick = 12` (`1` emit interval apart at `60 Hz` sim), passing them through [`new_tick_payload`](../../../src-tauri/src/core/emit.rs#L40-L46), [`refresh_tick_payload`](../../../src-tauri/src/core/emit.rs#L54-L74), and [`PheromoneEmitGate::poll`](../../../src-tauri/src/core/emit.rs#L144-L186), and measuring exact `serde_json::to_vec` byte lengths. Raw command output is archived in hub `test_runs/anima-case_EMIT-BUDGET_report.md`.

### 2.2 Live Engine Cohorts vs. Exact `10 / 100 / 1,000` Agent Slices

Running `SimulationEngine` with `ANIMA_FOUNDING_POPULATION` set to `10` and `100` grew to `17` and `170` live agents by `tick = 10` through early ecosystem spawning, while `1000` stayed at `1,000` live agents (`3,000` segments). To report both unmodified engine ticks and exact `10 / 100 / 1,000` cohorts (`3` segments/agent), the `t2_tick = 12` `1,000`-agent snapshot was also sliced by the first `10`, `100`, and `1,000` unique `agent_id`s through `refresh_tick_payload`:

| Population Cohort | Segments | Env Elements | Raycasts | `simulation-tick` (JSON B) | `segments` (JSON B) | `environmental_state` (JSON B) | `head_directions` (JSON B) | `raycast-update` (JSON B) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| **`10` agents** *(exact slice)* | `30` | `55` | `10` | **`15,573`** | `11,154` | `4,177` | `187` | **`1,330`** |
| **`17` agents** *(live `pop_cfg=10`)* | `51` | `55` | `10` | **`23,634`** | `19,090` | `4,162` | `327` | **`1,324`** |
| **`100` agents** *(exact slice)* | `300` | `55` | `100` | **`118,127`** | `111,932` | `4,177` | `1,963` | **`13,475`** |
| **`170` agents** *(live `pop_cfg=100`)* | `506` | `55` | `170` | **`197,210`** | `189,603` | `4,189` | `3,363` | **`22,789`** |
| **`1,000` agents** *(live `pop_cfg=1000`)* | `3,000` | `55` | `1,000` | **`1,150,670`** | `1,125,809` | `4,177` | `20,629` | **`135,949`** |

*Note on auxiliary events and sub-tick invariants (`t1_tick = 10 -> t2_tick = 12`, measured on the live runs):*
- **`combat-event`:** **`71 B`** per `CombatEvent` JSON object (`predator_id`, `prey_id`, `damage`, `energy_transferred`).
- **Dynamic vs. static segment delta (`t1 -> t2`):** Across one `33 ms` emit interval, **`100%` (`51/51`, `506/506`, `3,000/3,000`)** of segments changed pose (`> 1e-4` delta in `x, y, z, yaw, pitch, roll, energy, hydration`), whereas **`0 / 3,000` (`0%`)** changed any static morphology field (`parent_segment_id`, `length`, `radius`, `joint_anchor_{x,y,z}`, `joint_axis_{x,y,z}`, `agent_type`), and **`0`** new segment keys appeared.
- **`pheromone-update` (`128 × 128 = 16,384` `f32` cells):**

| Active Non-Zero Grid Fraction | Non-Zero Cells | Serialized JSON (`PheromoneGridState`) | Bytes / Tick (`10.0 Hz` max) |
|---|---:|---:|---:|
| `0%` (all `0.0` at startup; `gate_t2 = false` when unchanged) | `0` | **`65,571 B`** | `0.656 MB/s` when changing from prior frame, `0 MB/s` when steady |
| `10%` (realistic active diffusion `f32` values) | `1,638` | **`74,735 B`** | `0.747 MB/s` |
| `50%` (broad active pheromone coverage) | `8,192` | **`111,498 B`** | `1.115 MB/s` |
| `100%` (saturated `f32` field) | `16,384` | **`157,253 B`** | `1.573 MB/s` |

### 2.3 Aggregate Throughput (`MB/s` on Wire & UTF-16 `ExecuteScript` Load)

Rates use exact `EMIT_INTERVAL = 33 ms` (`30.30 Hz`) and `PHEROMONE_EMIT_INTERVAL = 100 ms` (`10.0 Hz`). Nominal `30.0 Hz` (`10.0 Hz` pheromone) figures are shown in parentheses for `0%` pheromone (`gate_t2 = false` / unchanged grid) vs. `50%` active diffusing pheromone (`111,498 B` @ `10 Hz`):

| Cohort | Per-Tick Total (`sim-tick` + `raycast`, JSON B) | Steady Grid (`0%` active pheromone emitted @ `30.30 Hz`) | Active `50%` Pheromone (`30.30 Hz` tick+ray + `10 Hz` phero) | Active `100%` Pheromone (`10 Hz`) | WebView2 UTF-16 `ExecuteScript` Rate (`50%` phero, ~2x) |
|---|---:|---:|---:|---:|---:|
| **`10` agents** *(slice)* | `16,903 B` | `0.512 MB/s` (`0.507` @ 30 Hz) | **`1.627 MB/s`** (`1.622`) | `2.085 MB/s` | `~3.25 MB/s` |
| **`17` agents** *(live `pop=10`)* | `24,958 B` | `0.756 MB/s` (`0.749` @ 30 Hz) | **`1.871 MB/s`** (`1.864`) | `2.329 MB/s` | `~3.74 MB/s` |
| **`100` agents** *(slice)* | `131,602 B` | `3.988 MB/s` (`3.948` @ 30 Hz) | **`5.103 MB/s`** (`5.063`) | `5.560 MB/s` | `~10.21 MB/s` |
| **`170` agents** *(live `pop=100`)* | `219,999 B` | `6.667 MB/s` (`6.600` @ 30 Hz) | **`7.782 MB/s`** (`7.715`) | `8.239 MB/s` | `~15.56 MB/s` |
| **`1,000` agents** *(live `pop=1000`)* | `1,286,619 B` | `38.988 MB/s` (`38.599` @ 30 Hz) | **`40.103 MB/s`** (`39.713`) | `40.561 MB/s` | **`~80.21 MB/s`** |

Key scaling takeaway:
- At **`10` agents**, fixed overhead dominates: **`pheromone-update` (`68.5%` at `50%` active) + `environmental_state` (`4.2 KB`) = `~80%` of all IPC traffic**.
- At **`1,000` agents**, [`SegmentState`](../../../src-tauri/src/core/simulation_state.rs#L17-L44) arrays dominate (`1,125,809 B / 1,286,619 B = 87.5%` per tick, `34.12 MB/s`), followed by `raycast-update` (`135,949 B × 30.30 Hz = 4.12 MB/s`, `10.6%`), `head_directions` (`0.63 MB/s`), and active `pheromone-update` (`0.66–1.57 MB/s`).

---

## 3. Mitigation Options

### Option 1 — Send Only What the Active View Needs

- **Mechanism:**
  1. **Subscription-gated channels:** Replace unconditional `handle.emit` fan-out with a lightweight frontend subscription state (e.g. `set_emit_subscriptions({ raycasts, pheromone, telemetry, simulation_tick })` or ref-counted channel subscriptions). While `LandscapeShowcase` / `Map2Showcase` is active and 2D telemetry drawers are closed, stop emitting `raycast-update` (`135.9 KB` @ 30.3 Hz) and `pheromone-update` (`65.6–157.3 KB` @ 10 Hz), and allow existing on-demand getters (`get_active_raycasts`, `get_pheromone_grid`, `get_environmental_elements`) to initialize views on switch.
  2. **Environment mutation guard & field trimming:** Skip emitting `environmental_state` (`~4.18 KB/tick`) when trees/lakes have not changed, omit the duplicate `head_directions` map (`20.6 KB` at 1,000 agents, recoverable on frontend from `root.head_direction` already carried on every root segment), and omit/deprecate unread `joint_axis_{x,y,z}` floats (`~170 KB` estimated across `3,000` segments in JSON) if unused by telemetry cards.
- **Measured / Estimated Saving (Active 3D View, Keeping JSON Transport):**
  - *Dropping unused `raycast-update`, `pheromone-update`, `environmental_state`, `head_directions` (`simulation-tick` delivers `segments` only -> `11,154 B` / `111,932 B` / `1,125,809 B` + small JSON wrapper `~14 B` — **measured `segs_B`**):*
    - **`10` agents:** `1.627 -> 0.338 MB/s` (**`-1.289 MB/s`, `-79.2%`**; retains `1,115 B/agent` segments).
    - **`100` agents:** `5.103 -> 3.392 MB/s` (**`-1.711 MB/s`, `-33.5%`**).
    - **`1,000` agents:** `40.103 -> 34.116 MB/s` (**`-5.987 MB/s`, `-14.9%`**).
  - *Additionally dropping `3` unused `joint_axis_*` JSON fields (`~56 B/seg`, estimated):* **`1,000` agents** drops another `~5.1 MB/s` -> **`~29.0 MB/s` (`-27.7%` total)**; **`100` agents** -> **`~2.88 MB/s` (`-43.5%`)**; **`10` agents** -> **`~0.29 MB/s` (`-82.4%`)**.
- **Effect on IPC Contract:**
  - Subscription gating + environment dirty-flag requires documenting view-subscription semantics (or keeping `environmental_state` as optional `Option<EnvironmentalState>` on [`SimulationTickPayload`](../../../src-tauri/src/core/simulation_state.rs#L48-L52)) in [`IPC_CONTRACT.md`](../../reference/IPC_CONTRACT.md) and updating `src-tauri/src/commands/environment.rs` / frontend fallback readers. Removing `joint_axis_*` or `head_directions` alters `ts-rs` exports (`SegmentState.ts`, `SimulationTickPayload.ts`) and breaks [`tickRecording.ts`](../../../src/components/Landscape/utils/tickRecording.ts#L140-L195) strict validator unless recording schema bumps or falls back to root `head_direction` / `0`. `src/App.tsx` landing panel still shows `5` agents (`FE-MEM-1` already slices DOM; `App.tsx` can still fetch raycasts/pheromones when telemetry tab or Pixi view is open).
- **Risks:**
  - **Stale state after view toggle:** If a user opens 2D Pixi or telemetry mid-run without fetching initial snapshots (`get_pheromone_grid`, `get_active_raycasts`, `get_environmental_elements`), the panel starts blank until the next mutation/tick. Must issue one explicit seed fetch when subscribing.
  - **Insufficient alone at `1,000` agents:** Even cutting all auxiliary events leaves **`29–34 MB/s` JSON** (`~58–68 MB/s` UTF-16 `ExecuteScript`) because `SegmentState[]` JSON dominates at scale.

---

### Option 2 — Lower Emit Rates When Window Is Hidden or Minimised

- **Mechanism:** Hook `tauri::WindowEvent::Focused(false)`, minimize checks (`window.is_minimized()` / `window.is_visible()`), and/or frontend `document.visibilityState === 'hidden'` (`visibilitychange` / `blur`) to throttle `EMIT_INTERVAL` on [`spawn_emit_thread`](../../../src-tauri/src/core/emit.rs#L207) (matching [`lib.rs`](../../../src-tauri/src/lib.rs#L217-L228) where `handle_run_event` already observes `tauri::RunEvent::WindowEvent`):
  - **Visible & unfocused (background window / secondary monitor):** optional mild throttle, e.g. `30 Hz -> 5–10 Hz`, or full rate if user watches side-by-side.
  - **Minimized / occluded / `hidden`:** pause UI pushes completely (`0 Hz`) or emit `1 Hz` heartbeat/status only while continuing simulation physics uninterrupted; flush one full snapshot immediately on restore/focus (`is_minimized() == false`).
- **Measured / Estimated Saving (Calculated from Measured Per-Event Bytes):**
  - **Paused (`0 Hz`) while minimized / hidden (`-100%` IPC while hidden):**
    - **`10` agents:** `1.627 -> 0.000 MB/s` (**`-1.627 MB/s`**)
    - **`100` agents:** `5.103 -> 0.000 MB/s` (**`-5.103 MB/s`**)
    - **`1,000` agents:** `40.103 -> 0.000 MB/s` (**`-40.103 MB/s`**; **`0 MB/s`** UTF-16 queue accumulation while backgrounded)
  - **`1 Hz` background heartbeat (`30.3x` reduction vs. `30.30 Hz`):**
    - **`10` agents:** `1.627 -> 0.128 MB/s` (`-92.1%`, with 10 Hz pheromone also clamped to `1 Hz`)
    - **`100` agents:** `5.103 -> 0.243 MB/s` (`-95.2%`)
    - **`1,000` agents:** `40.103 -> 1.398 MB/s` (`-96.5%`)
- **Effect on IPC Contract:**
  - **Zero wire-format breaking change** when retaining `simulation-tick` / `raycast-update` / `pheromone-update` shapes (`IPC_CONTRACT.md` only documents cadence adaptation when unfocused/minimized unless a status-only command is added).
- **Risks:**
  - **Does zero work when foregrounded:** During active interactive play (`WORLD` / `LandscapeShowcase` visible and focused — the exact Dell Iris Xe test scenario), savings are **`0%`**.
  - **Tick recorder & E2E interaction:** Background recording (`tickRecording.ts` / Playwright headless windows where `visibilityState` or focus behaves differently) and IPC telemetry panels must keep running if an active capture (`start_tick_capture`) or telemetry listener explicitly overrides pause-on-unfocused, and minimize/restore must not trigger a `TICK_RENDER_DELAY_MS` interpolation snap glitch.

---

### Option 3 — Typed or Binary Payloads (`tauri::ipc::Channel` / Raw `tauri::ipc::Response`)

- **Mechanism:** Move hot per-tick geometry & grids off JSON + `ExecuteScript` (`handle.emit`) onto binary byte buffers — the codebase already uses [`get_test_rabbit_state`](../../../src-tauri/src/commands/simulation.rs#L605-L624) returning `tauri::ipc::Response::new(Vec<u8>)` with packed little-endian `f32`/`u32` bytes:
  - **Path 3A — `tauri::ipc::Channel<InvokeResponseBody::Raw>` push** (`tauri = "2.0.0"` supports raw binary channel frames without JSON serialisation or JS script evaluation) or **Path 3B — Frontend pull via `invoke('get_tick_binary_frame') -> ArrayBuffer`** (`tauri::ipc::Response::new(Vec<u8>)`), returning an `ArrayBuffer` directly usable via `DataView` / `Float32Array` / `Uint32Array` typed views without `JSON.parse`.
  - Binary packing layout (`4`-byte aligned, little-endian, matching Rust `SegmentState` exact precision with **zero precision loss**):
    - `SegmentState` (`3 × u32` IDs/parent-flag [`u32` sentinel `u32::MAX` for `None` or packed `agent_type` flags] + `17 × f32` geometry/pose/state + `4` B flags = **`84 B/segment`** (`80 B` if `agent_type` & `has_parent` pack into 1 `u32`), plus `12 B` header: `magic/tick_u32, segment_count_u32, env_count_u32`).
    - `EnvironmentalElement`: `20 B` (`x, y, radius, resources` as `4 × f32` + enum `u32` for `"tree"`/`"lake"`, replacing JSON string literals).
    - `head_directions`: redundant `0 B` (read directly from root segment slot), or `16 B/agent` (`u32` id + `3 × f32`).
    - `RaycastTelemetry`: `32 B/raycast` (`6 × f32` origin/dir + `1 × f32` dist + `u16` entity_type + `u32` id padded, or `36 B`).
    - `PheromoneGridState`: raw `[f32; 16384]` = **`65,536 B`** (exact) or quantized `u8[16384]` (`0..MAX_CONCENTRATION=10.0`) = **`16,384 B`**.
- **Measured / Estimated Saving (Exact Packed Binary Sizes Computed from Measured Counts, `30.30 Hz` Tick+Ray + `10.0 Hz` Pheromone):**

| Cohort | Current JSON (`50%` Phero) | Binary Full-Parity (`12 + 84*seg + 20*env` tick, `32*ray` ray, `65,544 B f32` phero) | Binary Rate & Saving | Quantized Pheromone (`16,392 B u8`) + Binary | Combined Binary Saving |
|---|---:|---:|---:|---:|---:|
| **`10` agents** *(slice, 30 segs, 10 rays)* | `1.627 MB/s` | `3,632 B` tick + `320 B` ray + `65,544 B` phero | `0.775 MB/s` (**`-52.4%`**) | `0.284 MB/s` | **`-82.5%`** |
| **`17` agents** *(live, 51 segs, 10 rays)* | `1.871 MB/s` | `5,396 B` tick + `320 B` ray + `65,544 B` phero | `0.829 MB/s` (**`-55.7%`**) | `0.337 MB/s` | **`-82.0%`** |
| **`100` agents** *(slice, 300 segs, 100 rays)* | `5.103 MB/s` | `26,312 B` tick + `3,200 B` ray + `65,544 B` phero | `1.550 MB/s` (**`-69.6%`**) | `1.058 MB/s` | **`-79.3%`** |
| **`170` agents** *(live, 506 segs, 170 rays)* | `7.782 MB/s` | `43,616 B` tick + `5,440 B` ray + `65,544 B` phero | `2.142 MB/s` (**`-72.5%`**) | `1.650 MB/s` | **`-78.8%`** |
| **`1,000` agents** *(live, 3,000 segs, 1,000 rays)* | **`40.103 MB/s`** | `253,112 B` tick + `32,000 B` ray + `65,544 B` phero | **`9.295 MB/s` (`-76.8%`)** | **`8.804 MB/s`** | **`-78.0%`** |

  *Crucial WebView2 bonus beyond raw byte count:* Removing `handle.emit` eliminates **`~80.2 MB/s` of UTF-16 `ICoreWebView2::ExecuteScript` JS evaluation + `JSON.parse`** completely; frontend typed-array decoding (`Float32Array` view over `ArrayBuffer`) produces **near-zero V8 GC heap churn**.
- **Effect on IPC Contract:**
  - **Contract change required** in [`IPC_CONTRACT.md`](../../reference/IPC_CONTRACT.md): replaces JSON events (`simulation-tick`, `raycast-update`, `pheromone-update`) with a binary `Channel` or `tauri::ipc::Response` command (`check_ipc_reference.mjs` requires code + doc update together). Note that `ts-rs` cannot auto-generate binary byte-offset schemas, so binary header/stride constants must be tested by an explicit Rust ↔ TypeScript layout parity test. `WorldShowcase` JS decoding stays well within its tight bundle budget (`check:bundle`, currently `6 B` headroom before optimization if kept minimal or put in a shared helper chunk).
- **Risks:**
  - **Tauri v2 WebView2 IPC transport detail:** Must verify whether Tauri v2 `Channel` uses `custom-protocol` / `postMessage` or wraps bytes in Base64/Array over `ExecuteScript` on Windows WebView2 (if `Channel` falls back to JSON arrays for `Vec<u8>`, bandwidth worsens unless explicitly `tauri::ipc::InvokeResponseBody::Raw`, whereas `tauri::command -> tauri::ipc::Response` already transfers raw `ArrayBuffer` over Tauri's custom protocol in [`get_test_rabbit_state`](../../../src-tauri/src/commands/simulation.rs#L605-L624)). Needs a 1-day prototype check on Windows WebView2 before committing!
  - **Schema drift & test mocking:** Existing frontend unit/Vitest tests and [`tickRecording.ts`](../../../src/components/Landscape/utils/tickRecording.ts) emit JSON `SimulationTickPayload`; binary paths require either maintaining a JSON adapter for `.anmr` replay files or recording binary frames cleanly.

---

### Option 4 — Delta Encoding (Split Static Morphology from Per-Tick Dynamic Pose)

- **Mechanism:** Measured `t1 -> t2` intervals prove **`100%` (`3,000/3,000`) of segments move every `33 ms`**, while **`0%` (`0/3,000`) change static morphology** (`parent_segment_id`, `length`, `radius`, `joint_anchor_{x,y,z}`, `joint_axis_{x,y,z}`, `agent_type`):
  1. **Static vs. Dynamic Segment Split:** Emit full segment descriptors **only once per `(agent_id, segment_id)`** (on genesis, birth, or mutation -> zero per-tick cost in steady state unless population turns over), plus lightweight death ID tombstones `[u32]`. Per-tick updates carry only dynamic pose + state: `x, y, z, yaw, pitch, roll, energy, hydration` (`8` floats) + `(agent_id, segment_id)` (`2` ints; or implicit index matching a persistent snapshot slot when topology matches) + root `head_direction` (`3` floats per agent, omitting `head_directions` map duplication).
  2. **Sparse Pheromone & Environmental Deltas:** Send changed pheromone cells only (`(u16 cell_idx, f32 value)` = `6 B/cell` binary, or quantized `3 B/cell`), and emit `environmental_state` updates **only on resource change / delta** (ignoring zero-change frames).
- **Measured / Estimated Saving:**
  - **Under JSON (`simulation-tick` stripped to dynamic pose fields `{i,s,x,y,z,yaw,pitch,roll,e,h}` + sparse `env` delta, using `100%` moving segments as measured):**
    - Per-segment JSON drops from `~375 B` (`1,125,809 / 3,000`) to `~155 B` (short keys / compact JSON array per segment: `~115 B/seg`) -> **`~10.4–14.1 MB/s` (`-65%` vs `40.1 MB/s`)** at `1,000` agents, **`~1.1–1.4 MB/s`** at `100` agents, **`~0.11–0.15 MB/s`** at `10` agents.
  - **Combined with Binary Encoding (Option 3 + Option 4):**
    - Steady-state per-tick dynamic pose = `12 B` header + **`40 B/seg`** (`agent_id: u32, segment_id: u32, 8 × f32`, dropping static `44 B/seg` to `0`!) + `12 B/agent` root `head_direction` (`3 × f32`):
      - **`10` agents (`30` segs):** **`1,332 B/tick` (`0.040 MB/s`)**; with sparse 10% pheromone delta (`9.8 KB × 10 Hz` = `0.098 MB/s`) -> **`0.138 MB/s` (`-91.5%` vs `1.627 MB/s`)**.
      - **`100` agents (`300` segs):** **`13,212 B/tick` (`0.400 MB/s`)**; with sparse 10% pheromone delta -> **`0.498 MB/s` (`-90.2%` vs `5.103 MB/s`)**.
      - **`1,000` agents (`3,000` segs):** **`132,012 B/tick` (`4.000 MB/s`)** (`4.4–5.0 MB/s` even with raycast binary deltas / sparse pheromone -> **`-87.5%` to `-90.0%` vs `40.103 MB/s`**)!
  - **Position delta (`dx, dy, dz` `i16` quantization):** `i16` relative movement saves another `~30%` over `40 B/seg` binary if desired (`~28 B/seg` -> `2.67 MB/s`), at the cost of accumulator drift management.
- **Effect on IPC Contract:**
  - Requires splitting `simulation-tick` into `topology-delta` (births/deaths/static morphology) + `pose-tick` (dynamic state) in [`IPC_CONTRACT.md`](../../reference/IPC_CONTRACT.md) and updating [`App.tsx`](../../../src/App.tsx), [`PixiViewport.tsx`](../../../src/PixiViewport.tsx), [`agentPose.ts`](../../../src/components/Landscape/utils/agentPose.ts), and `tickRecording.ts` to maintain a persistent client-side morphology table.
- **Risks:**
  - **Desync on dropped frame / late listener mount:** If Option 4 delta messages are combined with Option 5 frame dropping (backpressure), a dropped `topology-delta` birth/death packet would orphan or corrupt dynamic poses until resync! **Safety invariant:** births/deaths/static morphology MUST go over a reliable sequence-numbered queue or monotonic epoch (`topology_epoch: u64`), while only dynamic pose snapshots may be dropped/coalesced; a viewer mounting mid-run must request one full keyframe first.
  - **Churn edge case:** Under rapid high reproduction/mortality tests, `topology-delta` traffic spikes briefly; bounded by cohort birth/death rate.

---

### Option 5 — Backpressure (Coalesced Latest-Wins ACK / Credit or Frontend rAF Pull)

- **Mechanism:** Ensure no second IPC frame can enter WebView2's `ExecuteScript` / IPC queue before the frontend finishes processing the previous frame, **capping in-flight backlog to `<= 1` frame regardless of UI stall length**:
  - **Design 5A — Credit / ACK Push (`latest-wins` coalesce):** Rust `EMIT` holds an `AtomicU64` sequence + `in_flight` permit (limit `1`). It pushes tick `N` only when `ack >= N` (or permit free), dropping intermediate snapshots silently so slow receivers skip ticks instead of queueing `1.8–3.6 GB` of stale frames. Frontend sends a tiny ACK (`ack_tick(N)`) inside `requestAnimationFrame` / post-handler microtask.
  - **Design 5B — Frontend Pull (`requestAnimationFrame` / timer calling `invoke('poll_sim_frame') -> Response`):** The renderer pulls the latest completed simulation frame at its own cadence (`<= display refresh` or `30 Hz`). Zero queue buildup is **architecturally guaranteed** because Rust never pushes asynchronously (`EMIT` thread either disappears or only stages the latest lock-free snapshot buffer).
- **Measured / Estimated Saving:**
  - **On fast GPUs meeting `>= 30 FPS` (`16.7–33 ms/frame`):** Throughput equals the underlying payload rate (`1×` @ `30 Hz`), so steady-state wire bytes match Options 1/3/4 **unless** FPS dips.
  - **Under startup / shader stalls or low-FPS Iris Xe (`9.2–24.0 FPS` B0 runs in `WEBVIEW-MEM`):**
    - On `9.2 FPS` (`~109 ms/frame`), `EMIT` naturally drops from `30.30 Hz` to **`9.2 Hz`** -> **cuts `simulation-tick` + `raycast` traffic by `~70%` (`40.1 -> ~12.1 MB/s`) immediately** while keeping **`100%` eliminating the `50 s` `~1.8 GB` JSON / `~3.6 GB` UTF-16 startup queue spike** recorded during B0/A1 launch stalls!
    - During a `1.8 s` stall frame, queued `simulation-tick` frames drop from **`~54` stacked frames (~`62 MB` JSON / `~124 MB` UTF-16)** to **exactly `1` latest frame**.
- **Effect on IPC Contract:**
  - **Design 5A (ACK push):** Adds one lightweight control command `ack_ipc_frame(seq: u64)` (or internal channel ack) in [`IPC_CONTRACT.md`](../../reference/IPC_CONTRACT.md); existing `simulation-tick` / `raycast-update` / `pheromone-update` listener names stay unchanged (`drop-in` for `App.tsx`, `PixiViewport.tsx`, `LiveAgents.tsx`).
  - **Design 5B (Pull command):** Replaces pushed events with `poll_simulation_frame()` (pairs cleanly with Option 3 `tauri::ipc::Response` returning `ArrayBuffer`); requires updating `App.tsx` / `LiveAgents.tsx` / `tickRecording.ts` to poll instead of `listen(...)`.
- **Risks:**
  - **ACK starvation / deadlock risk (Design 5A):** If a frontend handler throws or unmounts before sending ACK, emission stalls permanently unless a watchdog fallback timeout (e.g. `500 ms`–`1,000 ms` idle re-arm or credit reset on `poll`/heartbeat) exists. Design 5B (frontend pull) has **zero deadlock risk**.
  - **Transient `combat-event` dropping:** Never coalesce/drop discrete `combat-event`s (`71 B/event`) using `latest-wins`; keep transient discrete events batched or separate from snapshot coalescing.
  - **Interpolation jitter:** When `poll`/ACK rate dips near `30 Hz`, [`TickInterpolationBuffer`](../../../src/components/Landscape/agentInterpolation.ts#L506-L510) (`TICK_RENDER_DELAY_MS = 33`) handles dropped/late frames smoothly via wall-clock timestamping, provided `prev`/`curr` gaps stay clamped rather than extrapolating.

---

## 4. Comparison Matrix & Composable Stacks

### 4.1 Per-Option Summary (`1,000` Agents Baseline = `40.10 MB/s` JSON w/ `50%` Active Pheromone)

| Option | Target Bottleneck | Steady Rate @ `10 / 100 / 1,000` Agents (Foreground) | Hidden / Stall Effect | IPC Contract Change? | Implementation Risk |
|---|---|---|---|---|---|
| **Baseline (trunk `283bd659`)** | JSON `ExecuteScript` flood (`30.3 Hz` tick+ray, `10 Hz` phero) | `1.63 / 5.10 / 40.10 MB/s` (`~80 MB/s` UTF-16 @ 1k) | Unbounded queue (`~1.8 GB` JSON in 50 s stall) | Baseline | Current WebView2 `4.6 GB` bloat |
| **O1. View-scoped events + trim unused** | `raycast-update`, active `phero`, `env`, `head_directions`, `joint_axis` | **`0.34 / 3.39 / 34.12 MB/s`** *(no joint_axis: `~0.29 / ~2.88 / ~29.0`)* | None by itself | Minor (`env` optional + subscription cmd) | Low (seed fetch on view switch) |
| **O2. Pause/throttle hidden & minimized** | Background / minimized `EMIT` waste | No change foreground; **`0 MB/s`** minimized (`1.40 MB/s` @ 1 Hz) | Stops background buildup | Minimal / doc cadence note | Low (respect `start_tick_capture`) |
| **O3. Binary payloads (`Response` / `Channel`)** | JSON bloat + UTF-16 `ExecuteScript` + `JSON.parse` GC | **`0.78 / 1.55 / 9.30 MB/s`** *(u8 phero: `0.28 / 1.06 / 8.80`)* | Eliminates `ExecuteScript` JS eval | Major (`Channel`/binary `Response` + test parity) | Medium (verify WebView2 raw IPC path + `.anmr` adapter) |
| **O4. Delta split (static vs dynamic pose)** | Resending static morphology (`0%` change/tick) + static `env` | JSON: **`~0.13 / ~1.2 / ~12 MB/s`**; w/ O3 binary: **`0.04 / 0.40 / 4.00 MB/s`** (`segs+heads`) | Same unless paired w/ O5 | Major (`topology-delta` + `pose-tick` epoch) | Medium-High (must never drop birth/death deltas) |
| **O5. Backpressure (ACK coalesce / rAF pull)** | Unbounded WebView2 queue during boot & Iris Xe stalls | Equal at `>=30 FPS`; **`-70%` (`~12.1 MB/s`) at `9.2 FPS` stall** | **Cures queue backlog (`<=1` frame in-flight)** | Low (5A ACK cmd) or Medium (5B pull cmd) | Low-Med (5B zero deadlock; preserve `combat-event`) |

### 4.2 Recommended Composable Combinations *(For Owner Selection)*

1. **Stack S1 — Fast JSON-Safe Stabilization (`O1 + O2 + O5A`):**
   - Keeps JSON `SegmentState` + `tickRecording.ts` compatibility intact while adding view-scoped `raycast`/`pheromone` gating + `environmental_state` dirty flag (`O1`), minimize throttling (`O2`), and single-frame `latest-wins` backpressure (`O5A` or pull JSON). Eliminates the **`~1.8 GB` stall backlog** immediately and trims `~15–28%` foreground JSON bytes, but still runs `~29–34 MB/s` JSON `ExecuteScript` during foreground `30 Hz` play at 1,000 agents.
2. **Stack S2 — Binary Pull / Raw Channel + Scope + Throttle (`O1 + O2 + O3 + O5B`):**
   - Replaces pushed JSON events with frontend-driven binary `invoke` pull (proven pattern: [`get_test_rabbit_state`](../../../src-tauri/src/commands/simulation.rs#L605-L624)) or verified raw `Channel`, plus view filtering (`O1`) and pause-when-hidden (`O2`). **Zero `ExecuteScript`**, **zero `JSON.parse` GC**, **zero queue backlog**, and **`<= 7.64 MB/s` @ 1,000 agents (`252,012 B/tick` w/o env @ `30.30 Hz` full 84 B/seg binary, dropping `env`/`raycasts`/`phero`)** with zero topology-desync complexity.
3. **Stack S3 — Full Stack (`O1 + O2 + O3 + O4 + O5B`):**
   - Adds reliable `topology_epoch` static/dynamic split (`O4`) atop `S2`, dropping foreground `1,000`-agent wire rate all the way to **`4.00 MB/s` (`0.40 MB/s` @ 100 agents, `0.04 MB/s` @ 10 agents)** (`-90.0%` vs today's `40.10 MB/s`) while holding steady `f32` precision. Note: keep `WorldShowcase` decoding compact (or outside the `WorldShowcase` Vite chunk, which sits near its `175 KiB` bundle budget).

---

## 5. Open Owner Decisions (`TBD`)

| ID | Question | Choice Slot |
|---|---|---|
| **EB-D1** | Which target stack should be implemented after `v0.9.0` / post-`#291`: **`S1`** (`O1+O2+O5A`, JSON-preserving), **`S2`** (`O1+O2+O3+O5B`, binary without topology delta), or **`S3`** (`O1..O5`, binary + epoch delta)? | *(TBD — owner decision)* |
| **EB-D2** | Under `O1` / `O4`, is dropping `joint_axis_{x,y,z}` from per-tick `SegmentState` IPC (and removing `head_directions` map duplication in favor of `root.head_direction`) approved, or keep for [`tickRecording.ts`](../../../src/components/Landscape/utils/tickRecording.ts) schema continuity? | *(TBD — owner decision)* |
| **EB-D3** | Under `O2`, when the main window is visible-but-unfocused vs. minimized/hidden, should `EMIT` throttle to `1–5 Hz` or pause completely (`0 Hz`) until focus/restore? | *(TBD — owner decision)* |
| **EB-D4** | For `O3` / `O5`, prefer **pull `invoke` -> `tauri::ipc::Response` (`ArrayBuffer`)** (`O5B`, proven raw transport in `get_test_rabbit_state`, zero deadlock) or **push `tauri::ipc::Channel`** (after verifying Windows WebView2 binary path)? | *(TBD — owner decision)* |
