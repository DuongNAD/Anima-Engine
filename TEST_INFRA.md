# E2E Test Infra: Photorealistic Landscape Showcase

## Test Philosophy
- Opaque-box, requirement-driven. No dependency on implementation design.
- Methodology: Category-Partition + BVA + Pairwise + Workload Testing.

## Feature Inventory
| # | Feature | Source (requirement) | Tier 1 | Tier 2 | Tier 3 |
|---|---------|---------------------|:------:|:------:|:------:|
| 1 | Procedural Terrain | R1 | 5 | 5 | ✓ |
| 2 | Advanced Water Rendering | R2 | 5 | 5 | ✓ |
| 3 | Rich Vegetation | R3 | 5 | 5 | ✓ |
| 4 | Atmospheric Sky & Lighting | R4 | 5 | 5 | ✓ |
| 5 | Weather Effects | R5 | 5 | 5 | ✓ |
| 6 | Environmental Audio | R6 | 5 | 5 | ✓ |
| 7 | Full Camera Controls | R7 | 5 | 5 | ✓ |
| 8 | Technical/App Integration | R8 | 5 | 5 | ✓ |

## Test Architecture
- Test runner: Vitest inside JSDOM environment
- Test command: `npm run test:frontend`
- Test file: `tests/frontend/landscape_showcase.test.tsx`
- Mock setups: Mock WebGL/R3F Canvas, HTMLCanvasElement 2D context, and Web Audio API (AudioContext, GainNode, PannerNode).

## Real-World Application Scenarios (Tier 4)
| # | Scenario | Features Exercised | Complexity |
|---|----------|--------------------|------------|
| 1 | Cinematic Flyover | Camera, Audio, Sky | High |
| 2 | Weather Transition | Weather, Sky, Audio, Terrain | High |
| 3 | Alpine Exploration | Terrain, Vegetation, Weather, Audio | High |
| 4 | Beach Exploration | Terrain, Water, Vegetation, Audio | High |
| 5 | Ecosystem Speedup | Sky, Lighting, Weather, Audio, Integration | High |

## Coverage Thresholds
- Tier 1: Feature Coverage (>=5 per feature, total >=40 tests)
- Tier 2: Boundary & Corner Cases (>=5 per feature, total >=40 tests)
- Tier 3: Cross-Feature Combinations (pairwise coverage, total >=8 tests)
- Tier 4: Real-World Application Scenarios (total >=5 tests)
- Total: 93 tests
