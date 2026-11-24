# Project: Anima Engine Photorealistic Natural Terrain & 3D Environment Redesign

## Architecture
- **src/components/Landscape/utils/terrainGenerator.ts**: Organic multi-fractal noise pipeline (Ridged Multi-fractal + OpenSimplex2), hydraulic erosion simulation, winding river generator, Voronoi/Perlin biome blending, fast-path IndexedDB cache integration.
- **src/components/Landscape/utils/terrainSerializer.ts**: Compact binary serialization/deserialization for TypedArrays (`Float32Array`, `Uint8Array`).
- **src/components/Landscape/utils/terrainCacheDB.ts**: IndexedDB promise wrapper (`AnimaEngine_TerrainCache_v1`) with versioning, LRU eviction, and cache clearing APIs.
- **src/components/Landscape/Terrain.tsx**: 3D LOD terrain mesh with triplanar PBR biome texture splatting, slope-based normal/cliff mapping, and distance fog shader.
- **src/components/Landscape/Water.tsx**: GLSL water shader featuring Gerstner wave equations, sky-aware Fresnel reflections, shoreline/crest foam, river flow vector fields, and depth transparency.
- **src/components/Landscape/Vegetation.tsx**: Eco-clustered GPU instanced vegetation sampling (dense foliage in fertile valleys and near rivers, zero vegetation on cliffs >0.45 slope or snow peaks).
- **src/components/Landscape/Sky.tsx & Weather.tsx**: Camera-following shadow frustum, Rayleigh/Mie atmospheric scattering sky dome, dynamic distance fog.
- **src/components/Landscape/LandscapeControlsOverlay.tsx & LandscapeShowcase.tsx**: Controls HUD, cache status indicator, "Clear Cache & Regenerate" control.

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Photorealistic Natural Terrain & Biome Generation | Refactor `terrainGenerator.ts` to replace 4-fan sector division & circular hole with Ridged Multi-fractal noise, erosion simulation, winding rivers, smooth Voronoi/Perlin biome blending. | none | DONE |
| M2 | Realistic Environment Shading, Water & Eco-Clustered Vegetation | Upgrade `Terrain.tsx` slope PBR shader, `Water.tsx` transparent depth & ripple shader, `Vegetation.tsx` eco-clustering distribution rules, `Sky.tsx`/`Weather.tsx` distance fog. | M1 | DONE |
| M3 | Map Caching Integration & Full Verification | Verify IndexedDB serialization with natural terrain, UI "Clear Cache & Regenerate" action, 60 FPS GPU instancing/LOD, `npm run build`, `npm test` & `npm run test:frontend`, Forensic Audit. | M1, M2 | DONE |

## Interface Contracts
- **`serializeChunk` / `deserializeChunk`**: Binary conversion of elevation, biomes, hydrology, flora data to/from ArrayBuffers.
- **`generateTerrainAsync`**: Async terrain generator with fast-path IndexedDB cache check and organic noise fallback.
- **`clearTerrainCache()`**: Purges IndexedDB and memory caches on user demand.
- **`calculateSlope`**: Slope calculation utility for slope-based PBR shading and vegetation exclusion.
