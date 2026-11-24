---
phase: planning
feature: v1-vertical-slice
title: Card V7 — Causal Timeline and Control-vs-Treatment Charts in UI (Real Data Only)
description: Implementation spec for card V7 wiring real CausalLedger traces and real control-vs-treatment comparison charts into ResearchDashboard with zero placeholder or synthetic series
status: proposed
owner: anima-dell
last_reviewed: 2026-11-24
---

# Card `V7` — Causal Timeline & Control-vs-Treatment Charts in UI (`M9.2`, `M9.4`)

> **Design spec only (`status: proposed`).** No Rust, TypeScript, or docs-index edits are made in D8 (`V-SPECS-2`).
> All trunk `file:line` anchors below were checked against `origin/feat/ecological-selection` at
> `8d52341549007a50ce033a6d272c5dd22a71b1b1`. Anything not on trunk yet is labeled **`to be created by V7`**
> (or **`to be created by V6`**).

## 1. Goal, Dependencies, `START WHEN` & Scope

Build the real-data causal-timeline (`M9.2` / `S51`) and control-vs-treatment comparison UI (`M9.4` / `S53`, optional `M9.3` / `S52` top-contributors drilldown per `D-V7-4`) in `ResearchDashboard`, replacing prototype unavailability banners strictly with backend/artifact data — **zero placeholder data, zero `Math.sin` curves, zero fabricated checksums, and zero hardcoded split heuristics** (`PROMPTS_2026-11-24b`, [`WORLD_SIMULATION_PLAN.md`](../../../WORLD_SIMULATION_PLAN.md) `§7 M9` `:874-887`, `§9 :1061-1062`, and `2026-11-05-swarm-master-plan.md` Đợt 6 `V7`).

### Exact Dependency Order & `START WHEN`

- **Depends directly on:**
  - **[Card `V6`](2026-11-24-v6-interventions-causeid-spec.md)** — must already be merged and emitting persisted multi-hop `CausalLedger` mechanism chains (`parent: Some(...)`, depth `>= 3`) via `CausalChainTracker` (`to be created by V6`).
  - **Gate `G4`** (frontend/CI baseline green in [`STATE_OF_THE_PROJECT.md`](../../planning/STATE_OF_THE_PROJECT.md)).
- **`START WHEN`:** `V6` is merged on trunk, `G4` is green, and owner decisions `D-V7-1`–`D-V7-4` below are resolved.
- **Unblocks:** End-to-end human inspection of vertical-slice causal timelines and control-vs-treatment plots in `H11`.

### Explicit Non-Goals (And Hard Truthfulness Rule)

- **Real data only (`PROMPTS_2026-11-24b`):** When no backend `CausalLedger` records exist or no `PairedEnsembleReport` / `BranchTreeReport` comparison artifact has been loaded, every `V7` panel **must** render an explicit empty/unavailable state (matching the discipline already used at `src/components/ResearchDashboard/CheckpointStudioPanel.tsx:12-35` and `src/components/ResearchDashboard/CausalInterventionPanel.tsx:315-318`). Never generate fallback sine waves, mock timelines, or dummy checksums.
- **Do not regress real `ResearchDashboard` truth fixes:** Preserve single-pool animal reporting (`src/components/ResearchDashboard/TelemetryOverviewPanel.tsx:25-27`), real initial baseline EU (`ResearchDashboard.tsx:27-29, 172-203`), and honest provenance display (`ResearchDashboard.tsx:225-236`).

## 2. Current Trunk Anchors (`8d523415`) & Target Architecture

### 2.1 Existing UI & Domain Primitives Verified on Trunk

- **Dashboard host & tabs:** `src/components/ResearchDashboard/ResearchDashboard.tsx:12` defines `DashboardTab = 'telemetry' | 'trophic' | 'evolution' | 'interventions' | 'checkpoints'`; `ResearchDashboard.tsx:34-50` polls `get_simulation_status` and `get_ecosystem_state`.
- **Intervention panel (`CausalInterventionPanel.tsx`):**
  - `src/components/ResearchDashboard/CausalInterventionPanel.tsx:17-23, 52` fetches only `get_spatial_density_matrix` (`16×16`) and renders `SpatialRegionCanvas` + `CurveRampPreview`.
  - `src/components/ResearchDashboard/CausalInterventionPanel.tsx:295-318` disables `apply-intervention-btn` and states `"Dispatch is not available in this build: the desktop backend has no command that applies an intervention. Headless runs declare interventions in an experiment manifest for anima-lab."`
- **Checkpoint / counterfactual comparison panel (`CheckpointStudioPanel.tsx`):**
  - `src/components/ResearchDashboard/CheckpointStudioPanel.tsx:3-35` renders only an unavailable banner because `"The backend has no get_causal_branch_tree command"`.
  - Reusable UI primitives already present on trunk: `src/components/ResearchDashboard/common/BranchTreeCanvas.tsx:14-42` (`CausalBranchTreeState`, `src/components/ResearchDashboard/types.ts:11-23`), `src/components/ResearchDashboard/common/TimeSeriesChart.tsx:3-50` (`TimeSeriesItem`, supports `referenceLine` and multiple overlaid lines for control vs treatment), and `src/components/ResearchDashboard/DataExportDialog.tsx:20-45` (`export_scientific_dataset`).
- **Rust causal & counterfactual structures ready in backend:**
  - `src-tauri/crates/anima-domain/src/causal.rs:48-63` (`EffectRecord { effect_id, cause_id, parent, tick, target, quantity, delta, mechanism }`), `:127-165` (`CausalLedger::all`, `trace_to_root`, `root_cause`, `top_contributors`).
  - `src-tauri/src/core/branch_tree.rs:345-435` (`BranchExecutionStatus`, `BranchNodeResult`, `BranchTreeReport`, `ObservableDivergence`, `PairwiseBranchDivergence`, `dynamic_time_warping`, `compute_pairwise_divergence`).
  - `src-tauri/src/core/experiment_runner/ensemble.rs:135-225` (`PairedEffect`, `PairedEnsembleReport`, `run_paired_ensemble_with_control`).
- **IPC surface today:** `src-tauri/src/lib.rs:63-100` registers `36` commands (`scripts/check_ipc_reference.mjs:25-33` asserts exact parity with [`IPC_CONTRACT.md`](../../reference/IPC_CONTRACT.md)); zero commands currently return `CausalLedger` records, `trace_to_root` chains, or loaded `BranchTreeReport` / `PairedEnsembleReport` deltas.

### 2.2 Target Components & Backend Commands (`to be created by V7`)

1. **Read-only causal timeline IPC & panel (`M9.2` / `S51`, `to be created by V7`):**
   - Add Tauri command `get_causal_timeline` (returning bounded slices/pages of `CausalLedger::all()` plus optional root/target filter, backed by the live engine/ledger state when `V6` is enabled; `to be created by V7`) and `trace_causal_effect` (wrapping `CausalLedger::trace_to_root` and `top_contributors` from `src-tauri/crates/anima-domain/src/causal.rs:134, 159`; `to be created by V7`).
   - Add a `CausalTimelineView` component (`to be created by V7` under `src/components/ResearchDashboard/`) showing chronological `EffectRecord` events grouped/navigable by parent `EffectId` -> root `CauseId` (`S51`: clicking a downstream node highlights every upstream link returned by `trace_causal_effect`).
   - If `CausalLedger` is empty (or desktop `V6` is inactive), show `"No causal effects recorded yet"` with zero fabricated rows.
2. **Real control-vs-treatment comparison view (`M9.4` / `S53`, `to be created by V7`):**
   - Per `D-V7-2`, wire an IPC command `load_comparison_report` (`to be created by V7` in `src-tauri/src/commands/`) that reads and validates a real serialized `BranchTreeReport` (`branch_tree.rs:363`) or `PairedEnsembleReport` (`ensemble.rs:160`) JSON artifact produced by `anima-lab` / `V8` — or, only if `D-V7-2 = B` is approved, a real paired simulation comparison — never `Math.sin` or hardcoded series.
   - Render aligned tick axes in `TimeSeriesChart` (`TimeSeriesChart.tsx:22-50`) for baseline vs treatment trajectories (`S53`) and `BranchTreeCanvas` (`BranchTreeCanvas.tsx:14`) when a real `BranchTreeReport` is loaded, clearly tagging `seed`, `control_manifest_fingerprint`, `treatment_manifest_fingerprint`, and incomplete pairs (`ensemble.rs:171, 183-188`).
3. **IPC bindings, contract, & camelCase parity:**
   - Register every new command in `tauri::generate_handler![...]` (`src-tauri/src/lib.rs:63`), add matching rows to [`IPC_CONTRACT.md`](../../reference/IPC_CONTRACT.md), export TypeScript payload `.ts` types via `ts-rs` into existing `src/types/generated/` (new `V7` payload bindings `to be created by V7`), and satisfy `npm run check:ipc` (`scripts/check_ipc_arg_case.mjs:1-25`) so JS `invoke` keys match Rust parameter names in `lowerCamelCase`.

## 3. Applicable [`CLAUDE.md`](../../../CLAUDE.md) Traps & Invariants

- **Do not run full Bevy/Tauri on low-RAM dev machines during automated tests (`CLAUDE.md:52`):** UI logic and comparison components **must** be testable headlessly in Vitest with typed IPC fixtures matching `ts-rs` structs (`tests/frontend/research_dashboard_suite.test.tsx:11`), and backend serialization/query helpers must be unit-tested in `cargo test` without spawning windowed WebViews.
- **Vitest mock constructors (`CLAUDE.md:47-51`):** Keep `@react-three/fiber` mock aliases synced across `vite.config.ts`, `tsconfig.json`, and `tests/vitest.config.ts:1-20`; write `new`-invoked mocks with `function`, not arrow syntax; keep `testTimeout: 15000` (`tests/vitest.config.ts:13`).
- **Single-source IPC contract (`CLAUDE.md:191-193`):** Adding a Tauri command in `src-tauri/src/lib.rs:63-100` without updating [`IPC_CONTRACT.md`](../../reference/IPC_CONTRACT.md) fails `npm run check:docs`; mismatching snake_case/camelCase keys fail `npm run check:ipc`; leaving ESLint warnings > 0 fails `node scripts/eslint_ratchet.mjs` (`CLAUDE.md:22`).
- **Causal provenance honesty (`CLAUDE.md:168-170`, `WORLD_SIMULATION_PLAN.md` `:1126`):** `CausalTimelineView` must display raw `CauseId`/`parent`/`mechanism` fields honestly (including `CAUSE_BACKGROUND` / multi-parent absence) and cap returned timeline rows (`D-V7-3`) so UI polling does not serialize unbounded logs on every tick.
- **No direct-checksum pinning across platforms (`CLAUDE.md:41, 176-182`, `docs/decisions/ADR-0008-world-past-and-time.md:175, 283`):** Comparison UI tests verify schema parsing, chart alignment, and parent-chain traversal — never hardcoding an expected `world_checksum` across platforms or `LIVE_MODEL_VERSION` upgrades.
- **World-law, zero-alloc, `total_cost`, & No Lamarck boundaries (`CLAUDE.md:50, 97-98, 105-107, 145-153`):** `V7` reads telemetry/artifacts only; any backend command (`get_causal_timeline`) must copy/clone bounded snapshots off the lock without allocating inside the ECS tick hot loop (`build_tick_schedule`), altering `WorldLawSet`, touching `total_cost`, or writing `AgentBrain.learned`.

## 4. Open Owner Decisions (`D-V7-1`–`D-V7-4`)

| Slot | Question | Options | Recommendation |
|---|---|---|---|
| **`D-V7-1`** | Should `V7` enable live desktop intervention dispatch (`dispatch_causal_intervention`) or keep `CausalInterventionPanel` dispatch-disabled until viewer/live divergence policy is settled? | **(A)** Read-only causal timeline + report comparison in `V7`; keep live dispatch disabled unless explicitly enabled by owner.<br>**(B)** Wire `dispatch_causal_intervention` into the desktop `SimulationEngine` queue using `validate_intervention`. | **(A)** Read-only causal UI first, avoiding unverified multi-threaded desktop engine divergence notes (`IPC_CONTRACT.md:45`, `CLAUDE.md:179`). |
| **`D-V7-2`** | Source of control-vs-treatment trajectories for `M9.4` / `S53`: | **(A)** Import validated `BranchTreeReport` / `PairedEnsembleReport` JSON files produced by `anima-lab` / `V8`.<br>**(B)** Launch paired control + treatment headless runs inside the desktop Tauri process. | **(A)** Artifact JSON import first; running two Bevy simulations concurrently inside Tauri risks heavy dev-host crashes (`CLAUDE.md:52`). |
| **`D-V7-3`** | Timeline query pagination / row cap (`get_causal_timeline`): | **(A)** Bounded cursor/window query (`offset`, `limit <= 500`, optional `cause_id`/`target` filter).<br>**(B)** Return full `CausalLedger::all()` unconditionally. | **(A)** Bounded window (`limit <= 500`) matching `WORLD_SIMULATION_PLAN.md:1126`. |
| **`D-V7-4`** | Include `M9.3` ("Why did this change?" top contributors via `CausalLedger::top_contributors`) inside `V7`? | **(A)** Include a compact top-contributors drawer alongside timeline navigation (`src-tauri/crates/anima-domain/src/causal.rs:159`).<br>**(B)** Defer `M9.3` to a follow-up card. | **(A)** Include top-contributors drawer — `CausalLedger::top_contributors` already exists at `src-tauri/crates/anima-domain/src/causal.rs:159`. |

## 5. Future `V7` Card Boundaries (**Proposed** `ALLOWED PATHS`)

> Marked **`proposed`** for owner authorization when scheduling card `V7` after `V6` merges.

- `src/components/ResearchDashboard/` (new causal timeline & control-vs-treatment components; updates to `ResearchDashboard.tsx`, `CausalInterventionPanel.tsx`, `CheckpointStudioPanel.tsx`, `types.ts`)
- `src/types/generated/` (regenerated `ts-rs` bindings only)
- `src-tauri/src/commands/` (`environment.rs` and `mod.rs` already on trunk, plus new `causal.rs` command module `to be created by V7`)
- `src-tauri/src/lib.rs:63-100` (register `V7` IPC commands in `generate_handler!`)
- `src-tauri/src/core/simulation_loop.rs:1-60` & `src-tauri/src/core/live_experiment.rs:1-20` (only if `D-V7-1 = B`, exposing read-only causal ledger state safely)
- `src-tauri/tests/` & `tests/frontend/` (`tests/frontend/causalTimelineUi.test.tsx`, `to be created by V7`, plus updating `tests/frontend/research_dashboard_suite.test.tsx:11-35` if tabs/assertions expand)
- `docs/reference/IPC_CONTRACT.md` (matching rows for added IPC commands/events)

## 6. `DONE WHEN`, Verification Commands & Negative Controls (`A3 #2`)

### `DONE WHEN`
1. **Zero placeholder data (`PROMPTS_2026-11-24b`):** Clicking any effect node on the `V7` causal timeline navigates parent -> root cause using `trace_to_root` output (`S51`); control-vs-treatment view aligns baseline and treatment ticks from real backend/report JSON (`S53`); empty backend/missing report renders explicit empty state with zero `Math.sin`, hardcoded series, or invented checksums.
2. **Concrete Verification Commands (all must exit `0` after final edit; Cargo runs only on hosts meeting `>= 5.00 GiB` RAM / CI):**
   - `npm run check:ipc` (arg-case parity via `scripts/check_ipc_arg_case.mjs:1-25`)
   - `npm run check:docs` (exact `lib.rs` <-> `IPC_CONTRACT.md` command/event parity + links/metadata)
   - `npm run lint` and `node scripts/eslint_ratchet.mjs` (zero errors, zero warning regressions)
   - `npm run test` and `npm run test:frontend` (with Vitest causal timeline + control-vs-treatment alignment suites)
   - `npm run build` and `npm run check:csp`
   - From `src-tauri/`: `cargo test --lib export_bindings` + `git diff --exit-code -- ../src/types/generated` (`docs/how-to/RELEASE_CHECKLIST.md:90`)
   - `cargo fmt --check`, `cargo clippy --all-targets --features desktop -- -D warnings`, and `cmd /c "cargo test --features desktop > ..\cargo-test-output.txt 2>&1"` + `node scripts/check_test_targets.mjs cargo-test-output.txt --profile desktop`

### Negative Controls (Must Fail Red Under Fault Injection, `A3 #2`)
1. **Missing `IPC_CONTRACT.md` row:** Register a new `V7` command in `src-tauri/src/lib.rs:63-100` without documenting it in `docs/reference/IPC_CONTRACT.md` -> `npm run check:docs` (`scripts/check_ipc_reference.mjs:25-33`) exits `1`.
2. **Wrong IPC argument casing:** Invoke a `V7` command from React using snake_case (`effect_id` instead of `effectId`) -> `npm run check:ipc` (`scripts/check_ipc_arg_case.mjs:1-25`) exits `1`.
3. **Simulated placeholder injection:** Feed synthetic/unbacked mock data or silently substitute `0` for an empty control/treatment series in UI components -> `S51`/`S53` Vitest assertions fail red.

### Determinism Checks
- Rendering identical `CausalLedger` / `PairedEnsembleReport` JSON inputs yields deterministic node/edge ordering and aligned tick series in Vitest (ordered strictly by `effect_id` and `tick`).
