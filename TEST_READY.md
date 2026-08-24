# E2E Test Suite Ready

> ## 📐 Design document — counts here are the **planned** test matrix, not a measurement
>
> Every number below (93 total, 5-per-feature tiers, the ✓ grid) is a **coverage design**: what the
> matrix is specified to contain. It is not the output of a test run, and it does not say how many
> tests passed today.
>
> **Measured status lives in one place:**
> [`docs/planning/STATE_OF_THE_PROJECT.md` §1](docs/planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền)
> — which records the `tests/` suite at **432 passed** on 2026-07-27, a different and larger number
> because the suite has grown well past this original matrix.

## Test Runner
- Command: `npm run test:frontend`
- Target: all tests pass with exit code 0 *(a target, not a recorded result — see the banner above)*

## Coverage Summary (planned matrix)
| Tier | Count | Description |
|------|------:|-------------|
| 1. Feature Coverage | 40 | 5 tests per feature for all 8 features |
| 2. Boundary & Corner | 40 | 5 tests per feature (min/max limits, invalid bounds) |
| 3. Cross-Feature | 8 | Pairwise combinations of feature interactions |
| 4. Real-World Application | 5 | Realistic application use-case scenario workloads |
| **Total** | **93** | |

## Feature Checklist
| Feature | Tier 1 | Tier 2 | Tier 3 | Tier 4 |
|---------|:------:|:------:|:------:|:------:|
| F1: Procedural Terrain | 5/5 | 5/5 | ✓ | ✓ |
| F2: Advanced Water Rendering | 5/5 | 5/5 | ✓ | ✓ |
| F3: Rich Vegetation | 5/5 | 5/5 | ✓ | ✓ |
| F4: Atmospheric Sky & Lighting | 5/5 | 5/5 | ✓ | ✓ |
| F5: Weather Effects | 5/5 | 5/5 | ✓ | ✓ |
| F6: Environmental Audio | 5/5 | 5/5 | ✓ | ✓ |
| F7: Camera Controls | 5/5 | 5/5 | ✓ | ✓ |
| F8: Technical/App Integration | 5/5 | 5/5 | ✓ | ✓ |
