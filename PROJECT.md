# Project: Anima Engine — Map 2 Scientific WorldGen Upgrade

## Architecture
Comprehensive upgrade and optimization of Anima Engine 3D Map 2 based on scientific world generation models (Plate Tectonics, Hydraulic & Thermal Erosion, Orographic Rain Shadow, Dendritic River Basin, Whittaker Biome Classification, and 60 FPS Three.js WebGL Rendering).

```
                      +------------------------------------------+
                      |         Scientific WorldGen Engine       |
                      +------------------------------------------+
                                           |
                   +-----------------------+-----------------------+
                   |                                               |
                   v                                               v
      [Geology & Hydrology]                              [Climate & Ecology]
  - Tectonic Plates (Voronoi)                        - Lapse Rate (-6.5°C/km)
  - Convergent Fold Mountains                        - Orographic Windward Rain
  - Divergent Rift Valleys                           - Leeward Rain Shadow Desert
  - Hydraulic Gully Erosion                          - Whittaker 8-Biome Matrix
  - Thermal Scree Talus Slope (35°)                  - Softmax Ecotone Blending
  - Dendritic Gravity River Routing                  - Carrying Capacity (>=65% plains)
                   |                                               |
                   +-----------------------+-----------------------+
                                           |
                                           v
                      +------------------------------------------+
                      |       High-Performance Three.js WebGL     |
                      +------------------------------------------+
                      - Watertight Mesh with Perimeter Skirts
                      - Triplanar & Softmax Biome Terrain Shaders
                      - 4-Octave Gerstner Waves & Waterfalls
                      - Rayleigh & Mie Atmospheric Scattering
                      - 12 GPU Instanced Flora Species (Wind Sway)
                      - Autonomous Wildlife (Boids, Shoals, Herds)
                      - Dual Entry: map2.html & App.tsx Dashboard Toggle
```

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Plate Tectonics & Continental Shields | Voronoi tectonic plate simulation with convergent fold mountain chains, divergent rift valleys, and continental shield plains (>=85% land area). | M1 | R1 |
| 2 | Hydraulic & Thermal Erosion Engine | Fluvial stream-power gully/ridge carving + thermal scree talus angle of repose (35°) slope relaxation. | M1 | R1 |
| 3 | Atmospheric Temperature Lapse Rate | Standard International Atmosphere elevation temperature lapse rate (-6.5°C/km) driving high-altitude freezing. | M2 | R2 |
| 4 | Orographic Precipitation & Rain Shadow | 2D windward slope moisture condensation vs leeward adiabatic rain shadow desert/savanna depletion. | M2 | R2 |
| 5 | Dendritic Hydrological River Routing | Monotonic downhill gravity flow routing (dh/ds <= 0), dendritic tributary branching, lake pour-point spillways, and smooth shorelines. | M3 | R3 |
| 6 | Whittaker Biome Classification & Ecotones | 2D Whittaker matrix (Temp vs Moisture) for 8 biomes, continuous softmax partition of unity (sum w_i = 1.0) ecotones. | M4 | R4 |
| 7 | Ecological Carrying Capacity & Grove Layout | Open grassland dominance (>=65%), organic tree clustering (groves), strict spawn clearance (D >= 6m), and wildlife density limits. | M4 | R4 |
| 8 | High-Performance WebGL Shaders & Lighting | ACESFilmic tone mapping, Rayleigh & Mie sky dome, Gerstner wave displacement with analytical normals, Beer-Lambert water absorption. | M5 | R5 |
| 9 | Dual Integration & Zero-Defect Hygiene | Standalone map2.html and App.tsx dashboard toggle with mutual exclusion, 60 FPS (~27 draw calls), 0 TS/lint errors, 100% test pass. | M5 | R5 |
| 10| Animal Map Review MCP Verification | discover_map_artifacts, validate_map_manifest, prepare_team_review, inspect_map_views across 4 expert roles and Integrator gate. | M6 | Acceptance Criteria |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Tectonic Geology & Hydraulic/Thermal Erosion | Implement Voronoi plate crust, fold mountains, rift valleys, hydraulic gully carving, and thermal talus repose in `map2TerrainGen.ts`. | None | **DONE** |
| M2 | Atmospheric Climate & Orographic Rain Shadow | Implement altitude lapse rate (-6.5°C/km) and 2D windward/leeward rain shadow moisture physics in `map2TerrainGen.ts` and `map2AtmosphereMath.ts`. | M1 | **DONE** |
| M3 | Dendritic River Basin & Hydrological Network | Implement gravity flow routing, dendritic river network, lake basin drainages, and waterfall connections in `map2TerrainGen.ts` and `map2WaterMath.ts`. | M1, M2 | **DONE** |
| M4 | Whittaker Biome Classification & Carrying Capacity | Implement Whittaker 2D matrix, softmax ecotones, organic grove clustering, >=65% open plains, and spawn clearance in `map2BiomeConfig.ts` and `Map2Vegetation.tsx`. | M2, M3 | **DONE** |
| M5 | WebGL Rendering, Shaders, Instancing & Dual Integration | Optimize ACESFilmic tone mapping, Gerstner water shaders, instanced flora/wildlife, resolve TS6133/lint hygiene, ensure locked 60 FPS. | M1, M2, M3, M4 | **DONE** |
| M6 | E2E Testing Suite Pass, Animal Map Review & Final Hardening | Verify 100% pass across 5-Tier E2E test suite, execute Animal Map Review MCP tools, and verify zero critical/high findings. | M1, M2, M3, M4, M5 | **DONE** |

## Interface Contracts

### `map2TerrainGen.ts` Interface
```typescript
export interface TerrainSampleResult {
  height: number;              // World Y elevation in meters [0, 90]
  normal: [number, number, number]; // Unit surface normal [nx, ny, nz]
  steepness: number;           // 1.0 - normal.y in [0, 1]
  temperature: number;         // Normalized [0, 1], influenced by elevation lapse rate
  moisture: number;            // Normalized [0, 1], influenced by orographic rain shadow
  riverDistance: number;       // Distance to nearest river centerline in meters
  riverFlow: number;           // Flow accumulation / river intensity [0, 1]
  isWater: boolean;            // True if height <= seaLevel (12.0m)
  waterDepth: number;          // Depth below seaLevel in meters
}
```

### `map2BiomeConfig.ts` Interface
```typescript
export enum Map2Biome {
  SNOW_PEAKS = 0,
  ROCKY_CLIFFS = 1,
  TEMPERATE_PLAINS = 2,
  TROPICAL_RAINFOREST = 3,
  ARID_SAVANNA = 4,
  DESERT_DUNES = 5,
  HYDROLOGY_WATER = 6,
  BIOLUMINESCENT_CAVES = 7,
}

export interface BiomeWeights {
  weights: Float32Array; // 8-element float array summing identically to 1.0
  primaryBiome: Map2Biome;
  secondaryBiome: Map2Biome;
}
```

## Code Layout
- Terrain Mathematics & Geology: `src/components/Map2/utils/map2TerrainGen.ts`
- Biome Classification & Ecology: `src/components/Map2/utils/map2BiomeConfig.ts`
- Atmosphere & Climate Physics: `src/components/Map2/utils/map2AtmosphereMath.ts`
- Hydrology & Water Mathematics: `src/components/Map2/utils/map2WaterMath.ts`
- GLSL Shaders: `src/components/Map2/utils/map2Shaders.ts`
- 3D Terrain Component: `src/components/Map2/Map2Terrain.tsx`
- 3D Water Component: `src/components/Map2/Map2Water.tsx`
- 3D Atmosphere Component: `src/components/Map2/Map2Atmosphere.tsx`
- 3D Vegetation Instancing: `src/components/Map2/Map2Vegetation.tsx`
- 3D Wildlife Instancing: `src/components/Map2/Map2Wildlife.tsx`
- Scene Showcase & Dual Entry: `src/components/Map2/Map2Showcase.tsx`, `src/App.tsx`, `src/map2.tsx`, `map2.html`
- Test Suites: `tests/frontend/map2/*`
