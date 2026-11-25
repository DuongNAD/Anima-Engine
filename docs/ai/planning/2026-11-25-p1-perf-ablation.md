---
phase: planning
feature: perf-ablation
title: P1 PERF-ABLATION — 16-Cell Live Schedule Ablation Grid & N=1,000 Sampling Profile
description: Criterion ablation across N={100,1000,2048,4096} x predator_fraction={0.3,0} x evolved_brains={0,1} plus N=1,000 Windows CPU sampling profile testing whether predator x prey loops drive the scaling knee.
status: proposed
owner: anima-case
last_reviewed: 2026-11-25
---

# `P1` — `PERF-ABLATION` (Benchmarks & Sampling Profile, No Engine Change)

## Benchmark Grid (`src-tauri/benches/live_schedule_ablation.rs`)

- Separate Criterion bench (`[[bench]] name = "live_schedule_ablation"` in `src-tauri/Cargo.toml`); `src-tauri/benches/live_schedule_scaling.rs` was left completely unmodified.
- World setup: `LiveExperimentAdapter::from_manifest` on `(256, 256)`, `100,000` EU, `SEED = 20_261_125`, `20` warm-up ticks, `live.mortality = 0.0`, `live.reproduction = 0.0`, `live.trees = 8.0`, `live.lakes = 2.0`, `live.food_cap = 50.0` (agent & predator counts verified invariant before and after every cell).

### Sample Medians (`ms / tick`) Across All `16` Cells

| Arm (`predator_fraction`, `evolved_brains`) | `N = 100` | `N = 1,000` | `N = 2,048` | `N = 4,096` | Log-Log Exponent (`100..4,096`) | Knee Exponent (`1,000..4,096`) | Top Step (`2,048 -> 4,096`) |
|---|---|---|---|---|---|---|---|
| `pred = 0.30`, `evolved = 0` (`pred0.3_evolved0`) | `0.3685` | `5.5938` | `19.5783` | `70.7068` | `1.383` | **`1.799`** | `1.853` |
| `pred = 0.00`, `evolved = 0` (`pred0_evolved0`) | `0.4023` | `5.0876` | `17.4859` | `63.8489` | `1.327` | **`1.794`** | `1.868` |
| `pred = 0.30`, `evolved = 1` (`pred0.3_evolved1`) | `0.6090` | `9.2160` | `27.1617` | `85.6712` | `1.308` | **`1.581`** | `1.657` |
| `pred = 0.00`, `evolved = 1` (`pred0_evolved1`) | `0.6179` | `8.6716` | `25.2760` | `77.3394` | `1.278` | **`1.552`** | `1.613` |

## Sampling Profiler at `N = 1,000` (`pred0.3_evolved0`)

- **Method:** External Win32 `SuspendThread`/`GetThreadContext` + `dbghelp.dll` (`StackWalk64` + `SymFromAddrW` against `.pdb`, `109,596 / 110,501 = 99.2%` frames resolved) sampling `live_schedule_ablation-39896bdb84dc8800.exe` at `~1 kHz` during `"live_schedule_ablation/pred0.3_evolved0/1000"`.
- **In-measurement window (`Bencher::iter` stacks only):** `133` active main-thread CPU samples (`100%` inside `LiveExperimentAdapter::run_tick`, zero setup/warmup contamination).

### Top 5 Bevy ECS Systems at `N = 1,000` (`pred0.3_evolved0`)

| Rank | System / Helper (`src-tauri/src/`) | Inclusive Samples (`%` of `133`) | Notes |
|---|---|---|---|
| **1** | `action_resolution_system` (`core/agent_systems.rs:1922`) | **`46` (`34.6%`)** | Dominated by `apply_inertia_to_oscillators` (`agent_systems.rs:1894`, **`43` self = `32.3%`**), which performs a full `segment_query.iter()` scan over all `3N` segments inside each agent's response (`O(N^2)`). |
| **2** | `sensory_system` (`core/agent_systems.rs:1633`) | **`42` (`31.6%`)** | Calls `SpatialHashGrid::raycast` (`9` self in outer function + raycast subcalls) and scans `prey_cache` (`:1747`) or `food_cache` (`:1761`). |
| **3** | `hrrl_learning_system` (`ai/model.rs:1082`) | **`37` (`27.8%`)** | Recomputes raycasts (`ai/model.rs:1227`) alongside transition preparation (`4` self in outer function). |
| **4** | `rebuild_spatial_grid_system` (`physics/spatial.rs`) | **`4` (`3.0%`)** | Re-inserts `3N` segment colliders into the `256x256` hash grid. |
| **5** | `resolve_joints_system` (`physics/dynamics.rs:23`) | **`2` (`1.5%`)** | Tied at `2` samples (`1.5%`) with `update_pheromone_grid_system` and `dormant_cohort_ecology_system`; `steering_drive_system` and `update_cpg_system` recorded `1` sample (`0.8%`) each. |

> [!NOTE]
> Spanning systems #2 (`sensory_system`) and #3 (`hrrl_learning_system`), `SpatialHashGrid::raycast` (`src-tauri/src/physics/spatial.rs:141`) itself accounts for **`66 / 133 = 49.6%` inclusive (`55 / 133 = 41.4%` self)** of all tick CPU time at `N = 1,000`. Notice every tick invokes `16` raycasts per agent across both systems inside a fixed `256x256` arena whose per-cell collider occupancy grows linearly with `N`.

## Hypothesis Outcome (`agent_systems.rs:1747` & `world_systems.rs:461-463`)

- **Hypothesis tested:** Whether the all-pairs predator × prey loops (`sensory_system` `prey_cache` scan at `agent_systems.rs:1747` and `combat_system` predator × prey centroid pairs at `world_systems.rs:461-463`) drive the scaling knee.
- **Measured verdict: Falsified as the primary driver of the scaling knee.**
  - Removing all predators (`predator_fraction = 0.0`) only reduces `N = 4,096` median tick time by `9.7%` (`70.7068 ms -> 63.8489 ms`) under `evolved = 0` and `9.7%` (`85.6712 ms -> 77.3394 ms`) under `evolved = 1`.
  - Crucially, the `1,000 -> 4,096` knee exponent does **not** fall cleanly: it stays virtually unchanged (`1.799 -> 1.794` for `evolved = 0`, and even rises slightly on `2,048 -> 4,096` from `1.853` to `1.868`; `1.581 -> 1.552` for `evolved = 1`).
  - **Root cause explained by the `N = 1,000` CPU profile:** The quadratic/superlinear knee is driven primarily by two predator-independent loops that execute for **100% of agents** (both prey and predators):
    1. `action_resolution_system -> apply_inertia_to_oscillators` (`agent_systems.rs:1902`) iterates the full `segment_query` per agent (`O(N_agents * N_segments)`), taking `34.6%` of tick CPU at `N = 1,000`;
    2. `SpatialHashGrid::raycast` (`physics/spatial.rs:141`) across `sensory_system` + `hrrl_learning_system` (`49.6%` combined CPU) traverses `256x256` hash cells whose collider count grows linearly with `N` (`O(N)` rays × `O(N)` cell density on a fixed-size arena). Meanwhile `combat_system` (`world_systems.rs:461-463`) had `0 / 133` sampled hits at `N = 1,000`.
