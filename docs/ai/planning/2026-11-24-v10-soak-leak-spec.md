---
phase: planning
feature: v1-vertical-slice
title: Card V10 — Headless Long-Run Soak Test and Leak Stability Gates
description: Implementation spec for card V10 defining headless soak execution and explicit leak checks (heap allocations count, entity count, file handles, and conservation drift) with owner D-slots for numeric thresholds
status: proposed
owner: anima-dell
last_reviewed: 2026-11-24
---

# Card `V10` — Headless Long-Run Soak & Leak Stability Gates (`M11.4`, `S65`)

> **Design spec only (`status: proposed`).** No Rust, TypeScript, or docs-index edits are made in D8 (`V-SPECS-2`).
> All trunk `file:line` anchors below were checked against `origin/feat/ecological-selection` at
> `8d52341549007a50ce033a6d272c5dd22a71b1b1`. Anything not on trunk yet is labeled **`to be created by V10`**.
> **No numeric leak/duration thresholds are invented below** — all numeric acceptance thresholds are
> explicit owner `D-V10-*` decision slots (`PROMPTS_2026-11-24b`).

## 1. Goal, Dependencies, `START WHEN` & Scope

Verify long-running headless stability across the complete v1.0 vertical slice (`V0`–`V6` enabled together with interventions, reproduction, mortality, decomposition, soil/water feedback, and causal/world recording) per [`WORLD_SIMULATION_PLAN.md`](../../../WORLD_SIMULATION_PLAN.md) `§7 M11.4` (`:914`), `§8.6 S65` (`:1006`), `§10.2 :1110-1112`, and `2026-11-05-swarm-master-plan.md` Đợt 6 `V10`:

- **Zero `NaN`/`Inf`** across live agent, field, carcass, ledger, and observable state over extended tick runs.
- **Zero unbounded conservation drift** across closed EU (`src-tauri/crates/anima-domain/src/energy.rs:123`), hydrological water (`src-tauri/src/core/dynamic_fields.rs:559-576`), and soil nitrogen (`dynamic_fields.rs:757-771`).
- **Explicit resource-leak instrumentation** across **heap allocations count (`allocs`)**, **entity count**, and **OS file handles**, plus process RSS & progress watchdog (`src-tauri/tests/common/stall_detector.rs:20-50`).

### Exact Dependency Order & `START WHEN`

- **Depends directly on:** **[Card `V8`](2026-11-24-v8-slice-acceptance-spec.md)** (and transitively `V0`–`V6` so the soak driver exercises all vertical-slice systems via `LiveExperimentAdapter` / `build_tick_schedule`).
- **`START WHEN`:** `V8` is merged on trunk and owner decisions `D-V10-1`–`D-V10-5` supply concrete target numbers for each soak tier and leak ceiling.
- **Unblocks:** `H11` v1.0 release gate.

### Explicit Non-Goals

- **No invented thresholds:** Starter budget numbers like `"Soak test: 6–24 giờ tùy tier"` (`WORLD_SIMULATION_PLAN.md:1110`) explicitly carry `"Không khóa các con số hiệu năng trước khi chạy M0.4"` (`:1112`). Exact CI smoke vs full release soak tick/wall-clock durations and leak bounds must come from `D-V10-1`–`D-V10-5`.
- **No full windowed Bevy/Tauri soak in automated CI:** Consistent with `CLAUDE.md:52`, `V10` is **headless** (desktop UI/WebView long-run manual check belongs to `H11`).

## 2. Current Trunk Anchors (`8d523415`) & Leak Measurement Protocol

### 2.1 Existing Allocator, Stress & Cleanup Probes Verified on Trunk

- **Process-wide vs thread-scoped allocation tracker:** `src-tauri/tests/common/allocator.rs:5-115, 158-175` provides `TrackingAllocator::new()`, `start_tracking()` (all threads, default when rayon workers run inside the window), `start_tracking_this_thread()` (excludes libtest harness threads for single-threaded hot loops), `stop_tracking()`, and `ThreadTrackingAllocator`. Critical documented rule (`allocator.rs:5-49`): **any binary installing `#[global_allocator]` must carry only ONE `#[test]`** (running sub-phases sequentially inside it) so sibling test threads spawned by `libtest` never contaminate allocation counts.
- **Stall watchdog:** `src-tauri/tests/common/stall_detector.rs:20-50` (`StallDetector::new(budget)`, `observe(counter, tick)`, `stalled_for()`) used by `src-tauri/tests/persistence_stress_tests.rs:14-45` to fail loudly with `--nocapture` cycle diagnostics instead of silently hitting CI timeouts.
- **Drop-safe engine guard:** `src-tauri/tests/environmental_elements_stress_tests.rs:24-45` documents panicking-test thread-leak contamination under `#[global_allocator]` and protects worker shutdown via `EngineGuard`.
- **Segment/tree despawn hygiene:** `src-tauri/tests/birth_death_zero_alloc_tests.rs:308, 593` verifies `despawn_takes_every_segment_of_branched_and_oversized_trees` and birth/death zero-alloc paths.
- **Hot-loop field zero-alloc proof:** `src-tauri/tests/dynamic_fields_zero_alloc_tests.rs:32-60` verifies `step_climate` / `step_water` / `step_soil` make zero heap allocations under `TrackingAllocator`.

### 2.2 The Three Required Leak Checks + Conservation/Finite Probe (`to be created by V10`)

Create a dedicated single-`#[test]` integration binary (`src-tauri/tests/v10_soak_leak_tests.rs`, `to be created by V10`) plus an optional extended headless soak runner (`src-tauri/examples/run_v10_soak.rs`, `to be created by V10`) implementing all four probes on a single `#[global_allocator]`:

| Check | Exact Mechanism & How Measured (`V10`) | Threshold Source |
|---|---|---|
| **1. Heap allocations count (`allocs`) & RSS** | Wrap sampled tick windows (post-warmup hot ticks *and* intervention/cold checkpoints separately) with `TrackingAllocator::start_tracking_this_thread()` / `stop_tracking()` (`allocator.rs:100, 113`; or `start_tracking()` if rayon tasks run inside the window). Record `hot_tick_allocs` per window and cold-path delta allocations (`CausalLedger`, snapshot serialization, `world_record` flush), plus sample process RSS at `t_start`, midpoint checkpoints, and `t_end` via OS helper (`to be created by V10`). | **`D-V10-2`** (hot tick zero-alloc invariant vs cold/RSS slope) |
| **2. Entity count & orphan check** | At fixed checkpoint intervals, sample `world.entities().len()`, live creature count, live body-segment entity count, `Tree` count, and check for detached/orphaned `Segment` (`src-tauri/src/core/components.rs:104`, `birth_death_zero_alloc_tests.rs:649`) or dangling `ParentAgent` entities (`birth_death_zero_alloc_tests.rs:593-652`). Separately assert bounded pre-allocated storage for non-ECS records such as `CarcassPool::records.len()` (`to be created by V4` in `core/carcass.rs` per [`2026-11-24-v4-carcasses-decomposition-spec.md`](2026-11-24-v4-carcasses-decomposition-spec.md) `:48-64`), and compare total `world.entities().len()` against expected living-population + static-fixture envelope across cycles. | **`D-V10-3`** (0 orphan segments; entity growth envelope) |
| **3. OS file handles (FDs / handles)** | Query open OS handle/FD count (`open_handle_count()`, `to be created by V10`: Linux `/proc/self/fd` entry count, macOS `/dev/fd` count, Windows Win32 `GetProcessHandleCount(GetCurrentProcess())`) **before** starting `M` repeated checkpoint save/load + `world_record` segment writes, **after** intermediate cycles (forcing file close/drop), and at soak end. | **`D-V10-4`** (net open-handle delta tolerance after save/record drop) |
| **4. Finite state & conservation drift** | Assert `.is_finite()` across all agent energies/positions/brain states, `ResourceField`, and `DynamicFields`; verify `energy.rs:123` closed EU residual, `dynamic_fields.rs:570` water residual, and `dynamic_fields.rs:762` nutrient residual at every checkpoint; track `StallDetector` progress across the run. | **`D-V10-5`** |

## 3. Applicable [`CLAUDE.md`](../../../CLAUDE.md) Traps & Invariants

- **Zero-heap-allocation hot loop (`CLAUDE.md:50`, `allocator.rs:5-49`):** Any soak binary using `#[global_allocator]` **must** keep exactly one `#[test]` entry point (`v10_soak_and_leak_stability_gate`) to avoid false `allocs > 0` failures from concurrent `libtest` threads; never ignore intermittent `allocs > 0` failures — treat thread interference vs real hot-loop allocation explicitly per `allocator.rs:21-50`.
- **Single schedule source (`CLAUDE.md:183-185`):** Drive the headless soak strictly through `core::simulation_schedule::build_tick_schedule` (`LiveExperimentAdapter`), never a hand-rolled `.chain()` subset.
- **`total_cost` & closed EU (`CLAUDE.md:105-107`, `energy.rs:123`):** Every reproduction, metabolic, starvation, carcass decomposition, and nutrient charge stays accounted in EU; any slow EU leak surfaces in the soak conservation residual check.
- **No Lamarck & world-law immutability (`CLAUDE.md:97-98, 145-153`):** Long runs with births/deaths never write `AgentBrain.learned` back to `.genotype` and keep `WorldLawSet` (`exotic_energy = None` baseline) immutable across ticks.
- **Checksums never pinned across platforms (`CLAUDE.md:41, 176-182`, `docs/decisions/ADR-0008-world-past-and-time.md:175, 283`, `src-tauri/tests/cross_platform_checksum_probe.rs:235`):** Soak tests check conservation, finiteness, allocation/entity/FD bounds, and optional same-OS save/load equivalence — never hardcoded cross-OS `world_checksum` strings.
- **Bounded causal logs (`WORLD_SIMULATION_PLAN.md:1126`, `V6 D-V6-3`):** Confirm `CausalLedger` / `CausalChainTracker` compaction or significance-gating prevents unbounded heap/RSS growth over extended soak durations.

## 4. Open Owner Decisions (`D-V10-1`–`D-V10-5` — Thresholds Held for Owner)

| Slot | Parameter / Threshold to Lock | Options | Recommendation |
|---|---|---|---|
| **`D-V10-1`** | Soak tier tick lengths & checkpoint cadence (PR CI smoke vs release/nightly extended soak): | **(A)** Tier 1 (CI smoke `cargo test`): short multi-intervention soak; Tier 2 (`run_v10_soak --long` release candidate): extended multi-hour or multi-100k-tick soak.<br>**(B)** Single fixed tick budget run in every CI PR. | **(A)** Two-tier split (`Tier 1` fast smoke in `v10_soak_leak_tests.rs`, `Tier 2` deep soak in `run_v10_soak.rs` before `H11`), matching `CLAUDE.md` CI runtime limits. |
| **`D-V10-2`** | Heap allocation & process RSS bounds (`hot_tick_allocs`, cold checkpoint allocs, RSS drift): | **(A)** `hot_tick_allocs == 0` strictly on every sampled tick window (matching `CLAUDE.md:50`), plus owner-chosen max RSS growth slope/ceiling over Tier 2.<br>**(B)** Assert `hot_tick_allocs == 0` only (no RSS slope assertion). | **(A)** Strict `0` hot-tick allocations + explicit RSS stabilization check across `[t_25%, t_100%]`. |
| **`D-V10-3`** | Entity-count & orphan-segment ceiling: | **(A)** `0` orphaned segments (`ParentAgent` despawned or missing) + dead entities fully reclaimed after every step, with total entity table size bounded by owner-specified headroom over peak living count.<br>**(B)** End-of-run check only. | **(A)** Check `0` orphan segments and entity delta at **every** checkpoint stride. |
| **`D-V10-4`** | Open OS file-handle (`FD` / Win32 handle) leak tolerance across `M` save/load + `world_record` cycles: | **(A)** Net delta `<= 0` (or explicit small warmup tolerance `D-V10-4` on first lazy runtime initialization, then `0` net growth from cycle `2..=M`).<br>**(B)** Check end vs start only. | **(A)** Measure net delta across cycles `[2..=M]` after one warmup cycle so OS/libc lazy opens do not mask recurring handle leaks. |
| **`D-V10-5`** | Long-run cumulative conservation drift tolerances & `StallDetector` timeout budget: | **(A)** Keep trunk per-run bounds (`energy.rs:123` `RESIDUAL_ABS_TOLERANCE_EU = 1e-3`, `dynamic_fields.rs:570, 762` relative `< 1e-4`) or tighten/adjust for multi-100k ticks.<br>**(B)** Set soak-specific drift budget per tick. | **(A)** Enforce trunk tolerances (`<= 1e-3` EU / `< 1e-4` water & nitrogen) continuously at every checkpoint plus owner-set `StallDetector` wall budget. |

## 5. Future `V10` Card Boundaries (**Proposed** `ALLOWED PATHS`)

> Marked **`proposed`** for owner authorization when scheduling card `V10`.

- `src-tauri/tests/v10_soak_leak_tests.rs` (single-`#[test]` binary installing `TrackingAllocator`, `to be created by V10`)
- `src-tauri/examples/run_v10_soak.rs` (extended CLI soak runner for Tier 2, `to be created by V10`)
- `src-tauri/tests/common/` (cross-platform FD/Win32 handle & RSS sampler helper module, `to be created by V10`; zero edits breaking existing `allocator.rs` / `stall_detector.rs` callers)
- Minimal target-scoped leak fix ONLY if `V10` uncovers a concrete entity/FD/heap leak (or escalate to a dedicated fix card / `FINDINGS.md` entry per owner rules)
- `docs/ai/planning/` (soak results report for Tier 1 / Tier 2)

## 6. `DONE WHEN`, Verification Commands & Negative Controls (`A3 #2`)

### `DONE WHEN`
1. `D-V10-1`–`D-V10-5` thresholds are filled by the owner, and `v10_soak_leak_tests.rs` verifies all four checks (**finite/conservation**, **heap allocations count**, **entity count/orphans**, **file handles**) with a single `#[test]` under `TrackingAllocator`.
2. Concrete Verification Commands (after final edit, on host meeting `>= 5.00 GiB` RAM / CI):
   - `cargo fmt --check`
   - `cargo clippy --all-targets --features desktop -- -D warnings`
   - `cmd /c "cargo test --features desktop --test v10_soak_leak_tests -- --nocapture > ..\v10-soak.txt 2>&1"`
   - `cmd /c "cargo test --features desktop > ..\cargo-test-output.txt 2>&1"` + `node scripts/check_test_targets.mjs cargo-test-output.txt --profile desktop`
   - `cargo run --release --example run_v10_soak -- --ticks <D-V10-1>` (Tier 2 extended soak on qualified host)
   - `npm run check:docs`, `npm run check:text-hygiene`, `npm run check:root-hygiene`, `npm run check:conflict-markers`

### Negative Controls (Must Fail Red Under Fault Injection, `A3 #2`)
1. **Inject hot-loop heap allocation:** Push to an untracked `Vec` inside a tick system during a sampled window -> `TrackingAllocator` counter `> 0` fails the soak test red.
2. **Inject leaked entity / orphaned segment:** Skip `Commands::despawn` on one child segment or `Tree` inside death/deforest -> entity/orphan probe fails red.
3. **Inject leaked file handle:** Deliberately hold an open `std::fs::File` (or `mem::forget` a handle) inside the save/load checkpoint loop -> `open_handle_count()` delta fails red across cycles `[2..=M]`.

### Determinism Checks
- Tier 1 soak run executes two passes with identical seed/config on the same OS and records zero `NaN`, identical checkpoint entity counts, and `hot_tick_allocs == 0` in both passes.
