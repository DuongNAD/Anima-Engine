# E2E Test Infra: Anima-Engine Phase 8 Fantasy Map & Telemetry Dashboard

## Test Philosophy
- Opaque-box, requirement-driven. No dependency on implementation design.
- Methodology: Category-Partition + Boundary Value Analysis (BVA) + Pairwise Combinatorial Testing + Real-World Workload Testing.
- Execution environment: JSDOM + Vitest.

## Feature Inventory
| # | Feature | Source (requirement) | Tier 1 (Coverage) | Tier 2 (Boundary) | Tier 3 (Pairwise) | Tier 4 (Workload) |
|---|---------|---------------------|:------:|:------:|:------:|:------:|
| 1 | Base Texture Rendering | Phase 8 R1 | 5 | 5 | ✓ | ✓ |
| 2 | Region Selection | Phase 8 R1 | 5 | 5 | ✓ | ✓ |
| 3 | Volcano Smoke Particles | Phase 8 R2 | 5 | 5 | ✓ | ✓ |
| 4 | Volcano Lava Glow | Phase 8 R2 | 5 | 5 | ✓ | ✓ |
| 5 | Jungle Waterfall Spray | Phase 8 R2 | 5 | 5 | ✓ | ✓ |
| 6 | Jungle Waterfall Foam | Phase 8 R2 | 5 | 5 | ✓ | ✓ |
| 7 | World Tree Spores | Phase 8 R2 | 5 | 5 | ✓ | ✓ |
| 8 | World Tree Light Rays | Phase 8 R2 | 5 | 5 | ✓ | ✓ |
| 9 | Falling Snow | Phase 8 R2 | 5 | 5 | ✓ | ✓ |
| 10| Sailing Ships | Phase 8 R2 | 5 | 5 | ✓ | ✓ |
| 11| Sea Monster Tentacles | Phase 8 R2 | 5 | 5 | ✓ | ✓ |
| 12| Zoom & Pan Damping | Phase 8 R2 | 5 | 5 | ✓ | ✓ |
| 13| Grid/Highlight Caching | Phase 8 R3 | 5 | 5 | ✓ | ✓ |
| 14| Split-Screen Layout | Phase 8 R4 | 5 | 5 | ✓ | ✓ |
| 15| ECharts Population Chart | Phase 8 R4 | 5 | 5 | ✓ | ✓ |
| 16| ECharts Energy/Resource Telemetry | Phase 8 R4 | 5 | 5 | ✓ | ✓ |
| 17| Mock Data & Transition Animations | Phase 8 R4 | 5 | 5 | ✓ | ✓ |

## Test Architecture
- **Test Runner**: Vitest (`npm run test:frontend`) configured with JSDOM.
- **Mocks**: 
  - `pixi.js`: Mocked to capture graphic draws, event registration, and ticker loops.
  - `echarts`: Mocked to verify that charts are correctly initialized, updated, and disposed without executing heavy DOM/Canvas operations.
- **Test Case Format**: Each test renders the component via React Testing Library (`@testing-library/react`), triggers simulated actions (pointer, wheel, time ticks), and queries DOM elements/PixiJS mocks to verify state/attributes.
- **Directory Layout**:
  - `tests/frontend/fantasy_map.test.tsx` (Tiers 1, 2, and 3)
  - `tests/frontend/fantasy_map_stress.test.tsx` (Tier 2 and Tier 4)

## Real-World Application Scenarios (Tier 4)
| # | Scenario | Features Exercised | Complexity |
|---|----------|--------------------|------------|
| 1 | Map Idle State & Ambient Effects | F1, F3, F4, F5, F6, F7, F8, F9, F10, F11, F13, F17 | High |
| 2 | Zoom-Pan and Region Inspect | F2, F12, F14, F17 | Medium |
| 3 | Region Lore Display & Sidebar Update | F2, F14, F15, F16, F17 | High |
| 4 | Rapid Multi-Region Navigation | F2, F12, F14, F17 | Medium |
| 5 | Simulation Lifecycle Telemetry Update | F14, F15, F16, F17 | High |
| 6 | Extreme Pan Zoom Damping Decay | F12 | High |
| 7 | Particle Lifecycle Under Tick Load | F3, F5, F7, F9 | High |
| 8 | High-Throughput User Actions & Memory Safety | F2, F12, F13, F14, F17 | High |

## Coverage Thresholds
- **Tier 1 (Feature Coverage)**: ≥5 test cases per feature (17 * 5 = 85 tests total).
- **Tier 2 (Boundary & Corner)**: ≥5 test cases per feature covering boundaries (85 tests total).
- **Tier 3 (Cross-Feature)**: ≥17 test cases covering pairwise combinations of major feature areas.
- **Tier 4 (Workload/Application)**: ≥8 complex scenarios exercising the application end-to-end.
