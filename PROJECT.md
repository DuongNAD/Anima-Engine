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

### Phase 7: Ecosystem Dynamics (Metabolic Theory, Functional Responses, Closed Energy)
Grounded in the ecology foundational reference (Brown et al. 2004 MTE; Holling 1959; Lindeman 1942; Whittaker 1975; Rosenzweig 1971 paradox of enrichment). Backbone module `core/ecology.rs` (pure, unit-tested, zero-alloc on the hot path).
| # | Name | Scope | Status |
|---|------|-------|--------|
| E1 | Metabolic Theory of Ecology | `metabolic_rate = i0·M^¾·e^(−E/kT)` (Kleiber + Arrhenius, E=0.65 eV animals / 0.30 eV producers); replaces the linear mass term in `metabolic_decay_system` so metabolism scales sub-linearly with body mass and speeds with warmth (Q10≈2.4) | DONE |
| E2 | Holling functional responses | Type II/III + `predation_capture()`; combat uses Type III so a healthy prey is not zeroed in one strike and rare/weak prey are spared (rarity refuge → anti prey-extinction) | DONE |
| E3 | Lindeman transfer + closed loop | Predators assimilate only ~30% of captured energy; the unassimilated remainder returns to a conserved `EcosystemBiomass` ledger (detritus/plants/animals) — energy conservation as the primary anti-collapse device | DONE |
| E4 | NPP resource field | `ResourceField`: per-cell logistic regrowth `R+g·R(1−R/R_max)` with `R_max` from Whittaker biome NPP; SoA buffers, in-place `step_regrowth` (zero-alloc), `graze()`, world↔cell mapping; live `resource_field_regrowth_system` in the tick schedule, seeded from the terrain biomes in `init_world` | DONE |
| E5 | Biodiversity diagnostics | Shannon (−Σpᵢln pᵢ) and Gini–Simpson (1−Σpᵢ²) indices as pure functions for dashboards/telemetry | DONE |
| E6 | Grazing + closed energy loop | `herbivore_grazing_system`: prey graze `ResourceField` (Type II saturating intake → depleted cells disperse herbivores = giving-up-density refuge); biomass-GATED regrowth (`step_regrowth_gated`) so plants only grow by drawing detritus; metabolism routes respired energy → detritus; `ecosystem_census_system` tallies living-animal energy — the full plant→herbivore→detritus→plant cycle is conserved (unit-tested) | DONE |
| E7 | MAP-Elites ecological niche axes + NPP fruiting | Archive descriptors switched from locomotion (speed/efficiency) to ECOLOGICAL niche axes — body mass (`MorphologyGenotype::total_mass()`, the MTE master trait) × foraging range (distance roamed), normalized to [0,1] via `ecological_descriptors()`; so quality-diversity illuminates ecological diversity and predator/prey arms races spread the grid. `fruit_growth_system` now scales fruiting by local biome NPP (rainforest fruits fast, desert slow; trees without a position fall back to base rate) | DONE |
| E8 | Corpse decomposition + seasonal fertility | Death half of the closed loop — a replaced agent's remaining reserve energy returns to the detritus pool (conserved) via `apply_staggered_evolution_system`; a `SeasonClock` drives a `seasonal_fertility()` sine (summer boosts regrowth, winter suppresses) so the resource field booms and busts on a yearly cycle, a periodic disturbance that sustains predator-prey cycles | DONE |
| E9 | Live ecosystem dashboard over IPC | `get_ecosystem_state` command + per-tick publish of the conserved `EcosystemState` (detritus/plants/animals/total, prey/predator counts, Shannon/Simpson); frontend `EcosystemPanel` polls it and renders the stacked biomass bar + population split + diversity indices (unit-tested) | DONE |
| E10 | Ecosystem time-series visualization | `EcosystemPanel` keeps a rolling one-minute history and renders two validated inline-SVG sparklines — predator vs prey population (the Lotka-Volterra cycle) and the three biomass compartments over time — single-axis each, entity-fixed colors (CVD/contrast validated), legend + end-dot labels; unit-tested | DONE |
| E11 | Coevolution / Red-Queen metrics | `niche_divergence()` — normalized separation of prey vs predator mean body mass = the character-displacement / arms-race signal; plus MAP-Elites `archive_coverage` (occupied niche cells = open-endedness proxy). Both added to `EcosystemState`, shown in the panel (readouts + a divergence sparkline). Unit-tested | DONE |
| E12 | (Optional) Connectance + IDH study | Food-web connectance & chain length; an intermediate-disturbance-frequency diversity experiment (a study/harness, best run on the target hardware) | TODO |

## Interface Contracts
### Tauri Commands
- `get_simulation_status` -> `SimulationStatus`
- `toggle_simulation` -> `bool`
- `get_map_elites_grid` -> `MapElitesGridState`
- `update_evolution_settings(settings: EvolutionSettings)` -> `bool`
- `toggle_evolution` -> `bool`
- `get_pheromone_grid` -> `PheromoneGridState`
- `get_active_raycasts` -> `Vec<RaycastTelemetry>`
- `get_lineage_graph` -> `LineageGraphState`
- `get_chronicle_history` -> `Vec<ChronicleEvent>`
- `get_ecosystem_state` -> `EcosystemState` (closed-energy ledger: detritus/plants/animals/total, prey/predator counts, Shannon/Simpson) — published each tick, polled by the frontend `EcosystemPanel`
- `set_lod_focus(focus: LodFocus)` -> `()` — where simulation detail is centred, i.e. the observer's world position. `{ enabled: false, center: [0,0,0] }` is the default and the rollback: every agent `Hot`, thinking every tick, exactly as before simulation LOD existed. Applied on the next tick by `sync_lod_focus_system`, not immediately — the world belongs to the simulation thread.
- `get_lod_focus` -> `LodFocus` — the focus the engine is currently using
- `get_lod_bands` -> `LodBands` — tier boundaries (`hot_radius`, `warm_radius`, `warm_interval`). The agent viewport asks for these rather than hardcoding them: it only sets a focus while everything on screen fits inside `hot_radius`, so that no agent the user is looking at is ever tiered down.

### Tauri Events
- `simulation-tick` (Payload: `Vec<SegmentState>` / `SimulationTickPayload`)
- `map-elites-update` (Payload: `MapElitesGridState`)
- `pheromone-update` (Payload: `PheromoneGridState`)
- `raycast-update` (Payload: `Vec<RaycastTelemetry>`)
- `combat-event` (Payload: `CombatEvent`)
- `chronicle-event` (Payload: `ChronicleEvent`)
- `migration-event` (Payload: `MigrationPayload`)

## Code Layout
- `src-tauri/`: Rust workspace backend
- `src/`: React + TypeScript frontend
- `tests/`: Integration tests

### Frontend landscape internals

Not IPC — these are in-process contracts inside the older `LandscapeShowcase` stack, kept because
that stack is still what `src/App.tsx` renders and what roughly half the frontend suite exercises.

- **determineBiome**: `determineBiome(elevation: number, moisture: number, temperature?: number): BiomeType` — `temperature` is optional so pre-temperature callers keep working.
- **Vegetation Instancing**: instanced meshes use precomputed layout data to avoid overlap without an $O(N^2)$ pass.
- **Water Shader Uniforms**: the water shader takes temperature/biome mapping (texture or uniforms) so it can render lava and ice sheets.
