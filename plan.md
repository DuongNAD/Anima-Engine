# Execution Plan - Anima-Engine Phase 1

## Objective
Implement M3-M7: CPG driving, metabolic energy depletion, multi-segment agent spawning and IPC serialization, frontend visualizer, and E2E verification. Ensure zero heap allocations in the hot path.

## Steps

### 1. Backend Implementation (M3/M4 & M5)
- **Explorer Stage**: Examine `src-tauri/src/core/engine.rs` to determine changes for:
  - Spawning 10 multi-segment agents using `decode_genotype` with a 3-segment morphology layout.
  - Updating double-buffer to use `SegmentState` instead of `AgentState`.
  - Storing and serializing detailed segment properties (`agent_id`, `segment_id`, `parent_segment_id`, spatial coordinates, joint anchors/axes, and agent energy).
  - Verifying all systems in the tick schedule are configured properly.
- **Worker Stage**: Implement the changes in `engine.rs`.
- **Reviewer/Challenger/Auditor Stage**: Review the code, run backend unit tests (`cargo test`), verify zero heap allocations in the hot path.

### 2. Frontend Implementation (M6)
- **Explorer Stage**: Examine `src/App.tsx` and determine how to:
  - Define `SegmentState`, `AgentHierarchy`, and `RenderSegment` interfaces.
  - Listen to `simulation-tick` and set/store hierarchies.
  - Add `buildAgentHierarchy` helper (like the one in tests).
  - Implement an HTML5 Canvas visualizer to render agents as nodes connected by joints.
- **Worker Stage**: Implement changes in `src/App.tsx`.
- **Reviewer Stage**: Verify TS builds cleanly and unit tests pass.

### 3. E2E Verification & Integration (M7)
- **Worker Stage**: Run `cargo build --release` to generate the release binary.
- **Worker Stage**: Run the Playwright E2E tests (`npm run test:e2e`).
- **Auditor Stage**: Run forensic integrity checks.
