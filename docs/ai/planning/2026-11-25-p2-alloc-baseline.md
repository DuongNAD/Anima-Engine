---
phase: planning
feature: alloc-baseline
title: P2 ALLOC-BASELINE — Live Schedule Heap Allocation Baseline via LiveExperimentAdapter
description: 5-run median measurement of per-tick heap allocations through LiveExperimentAdapter on the live schedule, isolating ai/model.rs:349-354 via an evolved_brains=1.0 negative control.
status: proposed
owner: anima-case
last_reviewed: 2026-11-25
---

# `P2` — `ALLOC-BASELINE` (Measurement & Finding Only)

## Methodology

- **Test binary:** `src-tauri/tests/alloc_baseline_tests.rs` (single `#[test]` `measure_live_schedule_allocations_shared_vs_evolved_brains`, running all `5` paired seeds sequentially in `~1.52s`).
- **World setup (`QD-SCALE-TRIAL` baseline):** `LiveExperimentAdapter::from_manifest(&WorldLawSet::baseline(), &initial, &[], seed, (256, 256), 100_000)` with `live.founders = 100.0`, `live.predator_fraction = 0.30`, `live.mortality = 0.0`, `live.reproduction = 0.0`, `live.locomotion = 1.0`, `live.drive_floor = 0.5`, `live.thirst_steering = 1.0`.
- **Arms:**
  - **Baseline (`live.evolved_brains = 0.0`):** shared `BrainModel` batched inference (`batch_size == 100`).
  - **Negative control (`live.evolved_brains = 1.0`):** every founder carries `AgentBrain`, making `batch_size == 0` and skipping `src-tauri/src/ai/model.rs:344-354`.
- **Tracking window:** `60` warm-up ticks, then process-wide `ALLOCATOR.start_tracking()` (`src-tauri/tests/common/allocator.rs`) across `120` measured ticks (`adapter.run_schedule_once()`). The test reports counts without gating on the baseline number and asserts only the negative-control drop.

> [!NOTE]
> **Libtest main-thread noise caveat:** Because the live schedule runs systems that may use worker threads, `alloc_baseline_tests` keeps the process-wide counter (`ALLOCATOR.start_tracking()`), per `common/allocator.rs`. Process-wide tracking can still include a few allocations made by libtest's own main thread while handling harness events; across our `5` runs (`1.52s` total), run-to-run jitter was `0–11` allocations over `120` ticks (`<= 0.09/tick`), whereas the `AgentBrain` negative-control drop is **`8,880` allocations (`74.00/tick`)**.

## Measured Results (`5` Seeds, `120` Measured Ticks Each)

| Seed | Shared (`evolved_brains = 0.0`) Allocs / `120`t | Shared Allocs / Tick | Evolved Control (`evolved_brains = 1.0`) Allocs / `120`t | Evolved Allocs / Tick | Delta (`Shared - Evolved`) |
|---|---|---|---|---|---|
| `9001001` | `17,694` | `147.45` | `8,803` | `73.36` | `8,891` (`74.09/tick`) |
| `9001002` | `17,683` | `147.36` | `8,814` | `73.45` | `8,869` (`73.91/tick`) |
| `9001003` | `17,683` | `147.36` | `8,803` | `73.36` | `8,880` (`74.00/tick`) |
| `9001004` | `17,683` | `147.36` | `8,803` | `73.36` | `8,880` (`74.00/tick`) |
| `9001005` | `17,683` | `147.36` | `8,803` | `73.36` | `8,880` (`74.00/tick`) |
| **Median (`5` runs)** | **`17,683`** | **`147.36`** | **`8,803`** | **`73.36`** | **`8,880` (`74.00/tick`, `-50.2%`)** |

## Attribution (`FINDINGS.md` `F-082`, Status `OPEN`)

- `src-tauri/src/ai/model.rs:349-354`:
  1. `scratch.inputs.clone()` clones the contiguous `f32` batch input buffer into a fresh `Vec<f32>` every tick.
  2. `TensorData::new(..., [batch_size, SENSORY_SLOTS])` + `Tensor::<2>::from_data(data, device)` allocate Burn tensor backing buffers every tick.
  3. `actor_out.into_data().try_into_vec::<f32>()` materializes a new output `Vec<f32>` every tick.
- Skipping this shared forward pass (`live.evolved_brains = 1.0`) eliminates `8,880` heap allocations over `120` ticks (`74.00` allocs/tick), leaving `8,803` allocations (`73.36`/tick) from other remaining non-shared-forward-pass sites on the live schedule. No engine code was modified in `P2`.
