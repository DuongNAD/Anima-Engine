# Project: Anima-Engine

## Architecture
Anima-Engine is an autonomous natural world simulation and ecological AI platform coupling a high-performance Rust Bevy ECS simulation engine with a high-fidelity Three.js/React Three Fiber 3D visualizer and PixiJS 2D HUD wrapped in Tauri v2.

### Subsystems & Data Flow
1. **World Generation & Hydrology (Authoritative 2048x2048)**:
   - `worldGen.ts` generates 2048² SoA elevation, moisture, temperature, D8 flow accumulation, Barnes Priority-Flood lake basins, spillways, waterfalls, and 22 Whittaker biomes.
   - `worldArtifact.ts` & `world_artifact.rs` serialize/deserialize this into the canonical binary `WorldArtifact` format (`.anmw`, version 2, 36-byte header, FNV-1a checksum).
   - Ingested by Bevy ECS `init_world()` into `TerrainMap` (256x256 downsampled grid), binding `WorldIdentity` seed to `SimRng`.
2. **Closed Ecology & Energy Ledger**:
   - `EcosystemBiomass` & `EnergyLedger` in `anima-domain` maintain strict conservation ($E_{detritus} + E_{plants} + E_{animals} + E_{dormant} = \text{const}$) within $\le 10^{-3}$ EU tolerance.
   - Net Primary Production (`ResourceField`) seeded from Whittaker biomes with strided logistic regrowth.
   - Herbivore grazing (Holling Type II) and predator-prey combat (Holling Type III rarity refuge + 30% Lindeman trophic efficiency, 70% detritus return).
   - Metabolic Theory of Ecology (MTE): Kleiber's $M^{3/4}$ scaling $\times$ Arrhenius temperature factor.
   - Evolutionary niche space via MAP-Elites (`body-mass + trophic-ratio`).
3. **Creature Morphogenesis & Meshy AI Pipeline**:
   - `MorphologyGenotype` (acyclic tree of segment capsules & joint constraints).
   - Hybrid Architecture: Procedural 3D mesh with CPG phase oscillators + joint spring-dampers (60 FPS baseline fallback).
   - 9-slot deterministic prompt grammar (`CREATURE_PROMPT_CONTRACT.md`) generating concise prompts ($\le 500$ chars).
   - Archetype BLAKE3 hashing, local `.glb` file caching (`app_data_dir/models/archetypes/<hash>.glb`), A1–A7 acceptance gates, and per-individual morphological fit (scale, axial stretch, tinting, bone binding).
   - Terrain-following locomotion via raycast ground alignment and surface normal pitch/roll.
4. **Cinematic Atmosphere, Water Shaders & Chunking LOD**:
   - Celestial day/night cycle with dynamic sun/moon orbital trajectories and shadow frustum tracking.
   - 4 signature biome weather particle systems: Desert sandstorm, Snowy frost/blizzard, Rainforest bioluminescent spores, Volcanic ash/embers.
   - Water GLSL shader: Beer-Lambert depth absorption, dual-normal wave displacement, shoreline surf/foam, and underwater caustics.
   - Chunking LOD: 6x6 chunk grid with distance-based mesh resolution scaling and frustum culling (>70% triangle/draw-call reduction at 60 FPS).
5. **Desktop Packaging & Security**:
   - Tauri v2 configuration (`src-tauri/tauri.conf.json`, `capabilities/default.json`).
   - Strict Content Security Policy (CSP), verified by `scripts/check_csp_compat.mjs`.
   - Native macOS `.app` / `.dmg` bundle output and GitHub Actions multi-platform CI/CD for Windows (`.msi`) and Linux (`.AppImage`/`.deb`).
   - Repository hardening compliance verified by `just verify` ("QUA CONG").

---

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---|---|---|---|
| 1 | 2048x2048 Procedural Terrain | 8-octave fBm, ridged multifractals, hydraulic erosion | M1 | Survey 1 |
| 2 | D8 River Routing & Meanders | Flow accumulation, counting-sort descent, channel carving | M1 | Survey 1 |
| 3 | Barnes Priority-Flood Lakes | Lake basin filling, spillway overflow, endorheic salt lakes | M1 | Survey 1 |
| 4 | 22 Whittaker Biomes | Ecotones mapped from temperature lapse & orographic moisture | M1 | Survey 1 |
| 5 | Binary World Artifact (.anmw) | Version 2, 36-byte header, FNV-1a checksum, 22↔11 biome mapping | M1 | Survey 1 |
| 6 | Bevy ECS Terrain Sync | Ingest .anmw into `TerrainMap`, bind `WorldIdentity` seed | M1 | Survey 1 |
| 7 | Closed-EU Energy Ledger | Conserve EU across 4 compartments (detritus, plants, animals, dormant) | M2 | Survey 2 |
| 8 | MTE Metabolic Scaling | Kleiber's $M^{3/4}$ allometry $\times$ Arrhenius temperature factor | M2 | Survey 2 |
| 9 | Holling II & III Dynamics | Type II grazing, Type III predation rarity refuge, 30% Lindeman | M2 | Survey 2 |
| 10 | 10k+ Ticks Population Stability | Seasonal fertility, Whittaker NPP, Shannon/Simpson index > 0 | M2 | Survey 2 |
| 11 | Procedural 3D Mesh Fallback | CPG oscillator + joint constraints + SLERP/IK at 60 FPS | M3 | Survey 2 |
| 12 | 9-Slot Meshy Prompt Generator | Genotype to concise descriptive text prompt ($\le 500$ chars) | M3 | Survey 2 |
| 13 | Local Archetype Model Caching | BLAKE3 hash indexing, local `.glb` storage, A1–A7 gates | M3 | Survey 2 |
| 14 | Terrain-Following Locomotion | Raycast surface height and normal slope-following orientation | M3 | Survey 2 |
| 15 | Celestial Day/Night Cycle | 4-phase sun/moon orbits, dynamic lighting & shadow tracking | M4 | Survey 3 |
| 16 | Biome Weather Particles | Desert sandstorm, snow frost, rainforest spores, volcanic ash | M4 | Survey 3 |
| 17 | Realistic Water GLSL Shader | Depth absorption (Beer-Lambert), wave foam, caustics | M4 | Survey 3 |
| 18 | Distance Chunking & Frustum LOD | 6x6 chunk grid, distance tiers, skirts, >70% draw reduction | M4 | Survey 3 |
| 19 | Tauri v2 Packaging & Security | CSP verification, window permissions, macOS `.app`/`.dmg` | M5 | Survey 3 |
| 20 | Multi-Platform CI/CD Workflows | GitHub Actions matrix for macOS, Windows, and Linux | M5 | Survey 3 |
| 21 | Verification & Hardening Gate | Passing `just verify` ("QUA CONG"), full Vitest & Cargo tests | M5 | Survey 3 |

---

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|---|---|---|---|
| M1 | Unified Natural Map & Binary Sync | 2048² world generation, D8 flow, Barnes lakes, 22 biomes, `.anmw` format, Bevy ECS `init_world` sync | none | DONE |
| M2 | Closed Ecology & Population Dynamics | Closed-EU ledger ($\le 10^{-3}$ EU), MTE metabolic rate, Holling II/III, 10k-tick stability ($H>0$) | M1 | DONE |
| M3 | Creature Morphogenesis & Meshy AI | Hybrid procedural IK/CPG mesh fallback, 9-slot prompt generator, BLAKE3 cache, terrain locomotion | M1, M2 | DONE |
| M4 | Cinematic Atmosphere, Water & LOD | Day/night cycle, 4-biome particle systems, water shader (depth/caustics), 6x6 chunk LOD & culling | M1 | DONE |
| M5 | Desktop Packaging & Hardening | Tauri v2 configuration, CSP/IPC permissions, macOS packaging, CI/CD scripts, `just verify` | M1, M2, M3, M4 | DONE |
| M6 | Final Integration & E2E Acceptance | 100% pass across E2E Tiers 1-4, Tier 5 adversarial coverage hardening | M1, M2, M3, M4, M5, E2E | DONE |

---

## Interface Contracts

Each contract below has one canonical source; this section summarises it and links there. The
previous version of this section restated all four inline and three of them had drifted from the
code — the `.anmw` header, both biome lists, and render-space numbers that no frontend code uses.
Re-checked against the code on 2026-11-14.

### 1. World Artifact Binary Format (`.anmw` v2)

Canonical: the layout comment at the top of
[`src-tauri/src/core/world_artifact.rs`](src-tauri/src/core/world_artifact.rs), mirrored by
[`src/components/Landscape/utils/worldArtifact.ts`](src/components/Landscape/utils/worldArtifact.ts);
a committed fixture plus a cross-language test keep the two in sync.

- Header, 36 bytes (`WORLD_ARTIFACT_HEADER_LEN`), little-endian:

  | Offset | Type | Field |
  |---|---|---|
  | 0 | `[u8; 4]` | magic `ANMW` |
  | 4 | `u32` | version (`WORLD_ARTIFACT_VERSION` = 2) |
  | 8 | `u32` | width $W$ |
  | 12 | `u32` | height $H$ |
  | 16 | `f32` | sea level |
  | 20 | `u32` | world seed |
  | 24 | `u32` | generator version |
  | 28 | `f32` | world scale (world-units per axis; canonical 200) |
  | 32 | `u32` | FNV-1a 32 checksum of the payload (offset 36 to end) |

- Payload, $n = W \times H$, in this order: elevation, moisture, temperature, flow as `f32[n]`
  each, then biome as `u8[n]` (canonical 22-biome indices).
- A v1 artifact is rejected with a "regenerate the world" message; there is no in-place migration.

### 2. Biome Taxonomy Translation (22 Canonical ↔ 11 Legacy)

Canonical: [`BIOME_TAXONOMY.md`](BIOME_TAXONOMY.md). The 22 canonical biomes are the `Biome` enum in
`src/components/Landscape/utils/worldGen.ts` (frontend world generation and rendering); the 11 legacy
biomes are `BiomeType` in the Bevy simulation. Two maps join them — `map_biome_backend_to_frontend`
(lift) and `map_biome_frontend_to_backend` (downsample). Read the index tables there rather than
copying them here: they are the part that drifted.

### 3. Coordinate Contract

Canonical: [`COORDINATE_CONTRACT.md`](COORDINATE_CONTRACT.md). Four spaces — cell, uv, world and
render. The simulation's world space is $x, z \in$ `MapBounds` (canonical extent 200, i.e.
$[-100, 100]$) and $y \in [0, 10]$. Render space is a pure per-axis scale of world space defined on
the frontend: no rotation, no shear.

### 4. Creature Prompt Grammar IR (`MorphoSpec`) — *proposed, not in code*

Canonical: [`docs/reference/CREATURE_PROMPT_CONTRACT.md`](docs/reference/CREATURE_PROMPT_CONTRACT.md)
(status `proposed`). `MorphoSpec` does not exist in the code yet; treat the contract as a design
target, not an interface you can call.

### 5. Tauri IPC (commands and events)

Canonical: [`docs/reference/IPC_CONTRACT.md`](docs/reference/IPC_CONTRACT.md). It lists every command
registered in `generate_handler!` and every event the backend emits; `npm run check:docs` fails when
the code and that list disagree.

---

## Code Layout
```
Anima-Engine/
├── src/                                  # Frontend TypeScript & React Three Fiber
│   ├── components/
│   │   ├── Landscape/                    # 3D World Visualization
│   │   │   ├── WorldShowcase.tsx         # Master 3D scene coordinator
│   │   │   ├── WorldTerrainLod.tsx       # 6x6 Chunked terrain mesh with skirts
│   │   │   ├── WorldWater.tsx            # Realistic water GLSL shader (absorption/caustics)
│   │   │   ├── WorldSky.tsx              # GLSL skydome & day/night celestial orbits
│   │   │   ├── WorldWeather.tsx          # Biome particle systems (dust/snow/spores/ash)
│   │   │   ├── WorldVegetation.tsx       # 14 instanced flora types
│   │   │   ├── LiveAgents.tsx            # Hybrid creature rendering & terrain following
│   │   │   └── utils/                    # World gen, artifact binary codec, chunk LOD
│   │   └── EcosystemPanel.tsx            # Telemetry & god-mode controls
│   ├── PixiViewport.tsx                  # 2D God-eye canvas & organism morphology
│   ├── App.tsx                           # Main HUD interface & tabbed navigation
│   └── types/generated/                  # ts-rs generated Rust-to-TypeScript bindings
├── src-tauri/                            # Rust Backend
│   ├── src/                              # READ-ONLY core simulation logic
│   │   ├── core/                         # Bevy ECS systems, terrain, ecology, energy ledger
│   │   ├── evolution/                    # MAP-Elites, genotype graph
│   │   ├── physics/                      # Dynamics, joints, CPG oscillators
│   │   └── commands/                     # Tauri IPC command endpoints
│   ├── tests/                            # READ-WRITE verification test suites
│   ├── Cargo.toml                        # Rust dependencies & feature definitions
│   └── tauri.conf.json                   # Tauri v2 desktop application manifest
├── docs/reference/                       # Specifications & contracts
│   ├── BIOME_TAXONOMY.md
│   ├── COORDINATE_CONTRACT.md
│   ├── CREATURE_PROMPT_CONTRACT.md
│   └── MAP_MANIFEST.md
├── scripts/                              # Verification & quality scripts
│   ├── verify.sh                         # Hardening gate ('QUA CONG')
│   ├── check_csp_compat.mjs
│   └── check_bundle_budget.mjs
└── .agents/                              # Teamwork agent metadata & workspace state
```
