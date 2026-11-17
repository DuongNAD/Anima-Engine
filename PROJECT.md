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
   - Herbivore grazing (Holling Type II) and predator-prey combat (Holling Type III rarity refuge + 40% Lindeman trophic efficiency, `LINDEMAN_EFFICIENCY = 0.40`, the remaining 60% returned to detritus).
   - Metabolic Theory of Ecology (MTE): Kleiber's $M^{3/4}$ scaling $\times$ Arrhenius temperature factor.
   - Evolutionary niche space via MAP-Elites (`body-mass + trophic-ratio`).
3. **Creature Morphogenesis**:
   - `MorphologyGenotype` (acyclic tree of segment capsules & joint constraints).
   - Procedural 3D mesh with CPG phase oscillators + joint spring-dampers.
   - **Proposed, not in code:** the text-to-3D prompt and asset pipeline (9-slot prompt grammar, archetype hashing, a local `.glb` cache, acceptance gates A1–A7). It is a design in [`docs/reference/CREATURE_PROMPT_CONTRACT.md`](docs/reference/CREATURE_PROMPT_CONTRACT.md), status `proposed`. Checked 2026-11-17: no `meshy`, `blake3`, `GLTFLoader` or `MorphoSpec` under `src/` or `src-tauri/src/`.
   - Terrain-following locomotion via raycast ground alignment and surface normal pitch/roll.
4. **Cinematic Atmosphere, Water Shaders & Chunking LOD**:
   - Celestial day/night cycle with dynamic sun/moon orbital trajectories and shadow frustum tracking.
   - Four global weathers, `clear`, `rain`, `snow` and `fog` (`utils/weatherKind.ts`), with precipitation particles and distance fog (`WorldWeather.tsx`). There are no per-biome sandstorm, spore or ash systems.
   - Water shader (`WorldWater.tsx`): shallow-to-deep colour grading, wave detail and shoreline foam. The Map 2 scene (`src/components/Map2/`) has its own shader with Beer-Lambert extinction. No caustics are implemented.
   - Chunked terrain LOD (`WorldTerrainLod.tsx`, default 6x6, frustum culling, skirts) exists behind `TERRAIN_CHUNKED`, which is `false`: the shipped scene uses the single-mesh terrain.
5. **Desktop Packaging & Security**:
   - Tauri v2 configuration (`src-tauri/tauri.conf.json`, `capabilities/default.json`).
   - Strict Content Security Policy (CSP), verified by `scripts/check_csp_compat.mjs`.
   - CI (`.github/workflows/ci.yml`) runs the Rust gates on `windows-latest` and the frontend gates on `ubuntu-latest`. No workflow builds or publishes an installer; bundles are produced locally with `npm run tauri:build`.
   - Repository hardening compliance verified by `just verify` ("QUA CONG").

---

## Status

This file describes the architecture. It does not track what is done. The Feature Inventory and
Milestones tables that used to sit here were removed on 2026-11-17 by the owner's decision: a check
against the code found seven of their rows false, and their M1–M6 numbering collided with the
M0–M11 of [`WORLD_SIMULATION_PLAN.md`](WORLD_SIMULATION_PLAN.md).

- Measured status, per gate, with the command and the date:
  [`docs/planning/STATE_OF_THE_PROJECT.md` §1](docs/planning/STATE_OF_THE_PROJECT.md).
- What each subsystem has reached, and the stage the project is in: §2 and §3.00 of the same file.
- The long-range simulation roadmap: [`WORLD_SIMULATION_PLAN.md`](WORLD_SIMULATION_PLAN.md).

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
│   │   │   ├── WorldTerrainLod.tsx       # Chunked terrain mesh with skirts (off: TERRAIN_CHUNKED = false)
│   │   │   ├── WorldWater.tsx            # Water shader (depth colour grading, waves, foam)
│   │   │   ├── WorldSky.tsx              # GLSL skydome & day/night celestial orbits
│   │   │   ├── WorldWeather.tsx          # Weather: clear / rain / snow / fog
│   │   │   ├── WorldVegetation.tsx       # 14 instanced flora types
│   │   │   ├── LiveAgents.tsx            # Hybrid creature rendering & terrain following
│   │   │   └── utils/                    # World gen, artifact binary codec, chunk LOD
│   │   ├── Map2/                         # Second map scene (entry: map2.html), own water shader
│   │   ├── ResearchDashboard/            # Experiment dashboard tabs
│   │   └── EcosystemPanel.tsx            # Telemetry & god-mode controls
│   ├── PixiViewport.tsx                  # 2D God-eye canvas & organism morphology
│   ├── App.tsx                           # Main HUD interface & tabbed navigation
│   └── types/generated/                  # ts-rs generated Rust-to-TypeScript bindings
├── src-tauri/                            # Rust Backend
│   ├── src/                              # Core simulation logic (read-only for hardening agents, see AGENTS.md)
│   │   ├── core/                         # Bevy ECS systems, terrain, ecology, experiments, snapshots
│   │   ├── ai/                           # CPG oscillators, HRRL, actor-critic model, pheromone
│   │   ├── evolution/                    # MAP-Elites, genotype graph, fitness, lineage, MRCA
│   │   ├── physics/                      # Dynamics, joints, spatial hash
│   │   └── commands/                     # Tauri IPC command endpoints
│   ├── crates/anima-domain/              # World laws shared by both engines: energy ledger, causal ledger, clock
│   ├── tests/                            # Verification test suites (the hardening agents' write area)
│   ├── Cargo.toml                        # Rust dependencies & feature definitions
│   └── tauri.conf.json                   # Tauri v2 desktop application manifest
├── BIOME_TAXONOMY.md, COORDINATE_CONTRACT.md, MAP_MANIFEST.md, SIMULATION_RULES.md   # Root-level contracts
├── docs/reference/                       # Contracts: IPC, snapshot, determinism, energy ledger,
│                                         # creature development, creature prompt, evolution experiment
├── scripts/                              # Verification & quality scripts
│   ├── verify.sh                         # Hardening gate ('QUA CONG')
│   ├── check_csp_compat.mjs
│   └── check_bundle_budget.mjs
└── .agents/skills/                       # Hardening skills used by AGENTS.md
```
