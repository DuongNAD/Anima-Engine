# E2E Test Infra: Map 2 Scientific WorldGen

## Test Philosophy
- Requirement-driven, opaque-box, and analytical invariant verification.
- Derived directly from `ORIGINAL_REQUEST.md` (R1-R5) and scientific standards (WorldEngine, TerraForge3D, mapgen4, Whittaker).
- Methodology: Category-Partition + Boundary Value Analysis + Pairwise Combinations + Real-World Workloads + Adversarial Fuzzing.

## Feature Inventory & Test Mapping
| # | Feature | Requirement Source | Tier 1 (Features) | Tier 2 (Boundaries) | Tier 3 (Coupling) | Tier 4 (Workloads) | Tier 5 (Adversarial) |
|---|---------|-------------------|:-----------------:|:-------------------:|:-----------------:|:------------------:|:--------------------:|
| 1 | Plate Tectonics & Continental Shields | R1 | 5 tests | 5 tests | 2 tests | 1 scenario | Stress tests |
| 2 | Hydraulic & Thermal Erosion Engine | R1 | 5 tests | 5 tests | 2 tests | 1 scenario | Stress tests |
| 3 | Atmospheric Temperature Lapse Rate | R2 | 5 tests | 5 tests | 2 tests | 1 scenario | Stress tests |
| 4 | Orographic Rain Shadow & Wind Moisture | R2 | 5 tests | 5 tests | 2 tests | 1 scenario | Stress tests |
| 5 | Dendritic Hydrological River Routing | R3 | 5 tests | 5 tests | 2 tests | 1 scenario | Stress tests |
| 6 | Whittaker Biome Classification & Ecotones | R4 | 5 tests | 5 tests | 2 tests | 1 scenario | Stress tests |
| 7 | Carrying Capacity & Organic Glades | R4 | 5 tests | 5 tests | 2 tests | 1 scenario | Stress tests |
| 8 | High-Performance Shaders & Lighting | R5 | 5 tests | 5 tests | 2 tests | 1 scenario | Stress tests |
| 9 | Dual Integration & 60 FPS Performance | R5 | 5 tests | 5 tests | 2 tests | 1 scenario | Stress tests |
| 10| Animal Map Review MCP & Manifest Gates | Acceptance | 5 tests | 5 tests | 2 tests | 1 scenario | Stress tests |

## Test Architecture & Directory Layout
```
tests/frontend/map2/
├── map2TerrainMath.test.ts          # Tier 1 & Tier 2: Tectonics, erosion, height ranges, normals
├── map2AtmosphereWater.test.ts      # Tier 1 & Tier 2: Lapse rate, rain shadow, Gerstner waves
├── map2BiomeEcology.test.ts         # Tier 1 & Tier 2: Whittaker matrix, softmax partition of unity
├── map2VegetationWildlife.test.ts   # Tier 1 & Tier 3: Instancing, clearance, carrying capacity
├── map2Integration.test.ts          # Tier 1 & Tier 4: Dual entry, App.tsx toggle, lifecycle
├── map2AdversarialStress.test.ts    # Tier 5: 100k random coords, normal invariants
├── map2AdversarialChallenger.test.ts# Tier 5: 50 mount/unmount cycles, BFS reachability
├── map2WildlifeKinematicsStress.test.ts # Tier 5: 1000 frames herd/shoal/flock kinematics
└── map2ManifestValidation.test.ts   # Animal Map Review Manifest QA gates
```

## Coverage Thresholds
- Tier 1: ≥5 per feature (Happy-path isolation tests)
- Tier 2: ≥5 per feature (Boundary, limits, extreme coordinates, slope thresholds)
- Tier 3: Pairwise cross-feature interactions (e.g. Ridge × Wind => Rain shadow, River × Terrain => Gully)
- Tier 4: ≥5 realistic end-to-end application scenarios
- Tier 5: Adversarial chaos fuzzing, 100k sample stress tests, 100% PASS
