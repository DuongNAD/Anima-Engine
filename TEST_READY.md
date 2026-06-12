# E2E Test Suite Ready - Phase 5

## Test Runner
- **Backend Tests**:
  - Command: `cd src-tauri && cargo test`
  - Expected: all tests pass with exit code 0
- **Frontend Tests**:
  - Command: `cd tests && npm run test:frontend`
  - Expected: all tests pass with exit code 0
- **E2E Tests**:
  - Command: `cd tests && npm run test:e2e`
  - Expected: all tests pass or skip gracefully with exit code 0
- **Production Build**:
  - Command: `npm run build`
  - Expected: project builds successfully

## Coverage Summary
| Tier | Count | Description |
|------|------:|-------------|
| 1. Feature Coverage | 20 | Verify modular backend compile, WGPU model execution, crossbeam channel startup, PixiJS mounting, and Gemini client wrappers. |
| 2. Boundary & Corner | 8 | Verify ndarray fallback under WGPU initialization failures, empty world zero-heap ECS ticks, empty telemetry UI loads, and offline LLM fallbacks. |
| 3. Cross-Feature | 8 | Verify CPG controllers under CPU fallback, channel shutdown message drains, and canvas renderer predator/prey overlays. |
| 4. Real-World Application | 8 | Verify rapid toggling of simulation engine (25+ times) under load, and chronicle timeline deltas warning alerts. |
| **Total** | **44** | |

## Feature Checklist
| Feature | Tier 1 | Tier 2 | Tier 3 | Tier 4 |
|---------|:------:|:------:|:------:|:------:|
| Codebase Modularity & Zero-Heap ECS | 5 | 5 | ✓ | ✓ |
| Burn GPU Acceleration & CPU Fallback | 5 | 5 | ✓ | ✓ |
| Crossbeam Channel Reset Lifecycle | 5 | 5 | ✓ | ✓ |
| PixiJS Frontend Canvas Renderer | 5 | 5 | ✓ | ✓ |
| Gemini Web-Session API Wrapping & Logs | 5 | 5 | ✓ | ✓ |
