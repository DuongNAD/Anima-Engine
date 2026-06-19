# Project: Anima Engine Landscape Expansion & Biome Integration

## Architecture
- **src/components/Landscape/utils/terrainGenerator.ts**: Core math, noise generation, biome classification (including Desert, Jungle, Volcanic, and Glacier via temperature noise), and flora placement.
- **src/components/Landscape/Terrain.tsx**: Renders the 3D procedural terrain. Utilizes Level-of-Detail (LOD) and GPU/Vertex Shader-based wave animations to maintain performance at 1000x1000 scale.
- **src/components/Landscape/Vegetation.tsx**: Renders instanced trees, cacti, jungle palms, dead trunks, and snow pines using optimized GPU instancing.
- **src/components/Landscape/Water.tsx**: Renders water bodies (ocean, lakes, rivers) with custom shader-based depth transparency, lava river rendering for volcanic biomes, and ice sheet rendering for glaciers.
- **src/components/Landscape/Minimap.tsx**: Renders a 2D overview of the 1000x1000 map.
- **src/components/Landscape/LandscapeShowcase.tsx**: Parent component orchestrating canvas, sky, weather, audio, and UI overlays.

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | Exploration & Initial Verification | Scan files, locate components, run test suite to verify baseline. | none | DONE |
| 2 | E2E Test Suite Creation & Setup | Design and write/update E2E tests for the new biomes and scale. | M1 | PLANNED |
| 3 | Expand Map Scale to 1000x1000 | Expand grid dimensions to 1000x1000 and optimize performance (LOD, shaders, vegetation loops). | M1 | PLANNED |
| 4 | Integrate Desert, Jungle, Volcanic, and Glacier Biomes | Update terrain generator, biomes coloring, vegetation models, custom lava/ice water shaders. | M3 | PLANNED |
| 5 | Performance & Robustness | Apply optimizations to guarantee 60 FPS, verify layout and features. | M4 | PLANNED |
| 6 | Verification & Test Pass | Run full frontend test suite and Forensic Auditor checks. | M2, M5 | PLANNED |

## Interface Contracts
- **determineBiome**: Signature updated to `determineBiome(elevation: number, moisture: number, temperature?: number): BiomeType` to support backward compatibility.
- **Vegetation Instancing**: Instanced meshes use optimized layout data to prevent overlap without $O(N^2)$ complexity.
- **Water Shader Uniforms**: Water shader receives temperature/biome mapping texture or uniforms to render lava and ice sheets.
