# Project: Anima-Engine

## Architecture
Anima-Engine is a real-time, GPU-accelerated Artificial Life (ALife) and Evolution Simulator.
- **Backend**: Rust-based Tauri v2 application, Bevy ECS world for physics & neural control, background 60 FPS simulation loop thread.
- **Frontend**: TypeScript + React + Vite, communicates via Tauri IPC (commands/events).

## Milestones

### Phase 0: Foundation
| # | Name | Scope | Dependencies | Status | Agent ID |
|---|------|-------|-------------|--------|----------|
| T1 | Test Suite & Harness Setup | E2E integration test suite, runner, and basic IPC check mock cases | None | DONE | 46d9349d-dcb7-4667-9782-22bf510f4400 |
| T2 | Full Tier Coverage | Complete feature coverage, boundaries, pairwise, and workloads tests | T1 | DONE | 46d9349d-dcb7-4667-9782-22bf510f4400 |
| I1 | Project Initialization & Blueprint | Set up Tauri v2 React/TS template, stub structures under src-tauri | None | DONE | 3b004006-ccb5-4c74-a852-2a8b1007c343 |
| I2 | ECS Core Setup & Tick Loop | Bevy ECS initialization, background 60 FPS thread simulation loop | I1 | DONE | 3b004006-ccb5-4c74-a852-2a8b1007c343 |
| I3 | High-Performance IPC Eventing | Tauri commands/events serializing minimal agent data, zero-alloc in tick | I2 | DONE | 3b004006-ccb5-4c74-a852-2a8b1007c343 |
| I4 | E2E Verification & Integration | Integrate with frontend, verify with all E2E tests, pass E2E suite | I3, T2 | DONE | 3b004006-ccb5-4c74-a852-2a8b1007c343 |

### Phase 1: Morphological Evolution
| # | Name | Scope | Dependencies | Status | Agent ID |
|---|------|-------|-------------|--------|----------|
| T3 | E2E Setup & Infrastructure | Define multi-segment morphology, joint constraints, CPG oscillators tests | None | DONE | bbfae0d8-0cd6-453c-a78a-8b0ed21efd79 |
| I5 | Genotype-to-Phenotype Spawner | Bevy ECS MorphologyGenotype decoding, physics constraint solver | None | DONE | 357a8642-8284-47a0-8dd3-35717ac47436 |
| I6 | Joint-Level CPG Controller | Integrate CpgOscillator with joint constraints, rhythmic driving | I5 | DONE | 357a8642-8284-47a0-8dd3-35717ac47436 |
| I7 | Metabolic Energy Depletion | Energy decay scaling with mass, speed, and CPG forces | I6 | DONE | 357a8642-8284-47a0-8dd3-35717ac47436 |
| I8 | IPC Serialization & Frontend | Update Tauri IPC for segment states and hierarchy rendering in React | I7 | DONE | 357a8642-8284-47a0-8dd3-35717ac47436 |
| I9 | E2E Verification & Integration | Pass all Tier 1-4 tests, zero-allocation hot path verification | I8, T3 | DONE | 357a8642-8284-47a0-8dd3-35717ac47436 |

### Phase 2: Hybrid Neural Control & MAP-Elites
| # | Name | Scope | Dependencies | Status | Agent ID |
|---|------|-------|-------------|--------|----------|
| T5 | Test Suite & Harness Setup (Phase 2) | E2E integration tests design & runner setup for Actor-Critic, HRRL, and MAP-Elites | None | DONE | 0dcec7a3-eedb-4b42-b587-1da2e6debe27 |
| T6 | Full Tier Coverage (Phase 2) | Tier 1-4 tests covering Burn inference, intrinsic rewards, grid binning, and IPC settings | T5 | DONE | 0dcec7a3-eedb-4b42-b587-1da2e6debe27 |
| I10 | Actor-Critic via Pure-Rust Burn | Burn framework integration, lightweight network inference, CPG parameter updates | None | DONE | f3abf2af-2a14-4554-bb5b-96fb76961aad |
| I11 | HRRL & Foraging | Food spawn, collision-consumption, physiological homeostasis decay, intrinsic reward updates | I10 | DONE | f3abf2af-2a14-4554-bb5b-96fb76961aad |
| I12 | MAP-Elites Evolutionary Archive | Grid niche archiving, parent selection and mutation, dynamic config updates | I11 | DONE | f3abf2af-2a14-4554-bb5b-96fb76961aad |
| I13 | Tauri IPC & Frontend Grid Dashboard | commands (get_map_elites_grid, update_evolution_settings, toggle_evolution), grid UI | I12 | DONE | f3abf2af-2a14-4554-bb5b-96fb76961aad |
| I14 | E2E Verification & Hardening | Tier 1-4 pass, zero-allocation verification, Tier 5 white-box adversarial coverage | I13, T6 | DONE | f3abf2af-2a14-4554-bb5b-96fb76961aad |

### Phase 3: Socialization & Emergent Behaviors
| # | Name | Scope | Dependencies | Status | Agent ID |
|---|------|-------|-------------|--------|----------|
| T7 | Test Suite & Harness Setup (Phase 3) | E2E integration tests design & runner setup for Raycasting, Pheromones, and Predator-Prey dynamics | None | DONE | 4454e0e2-2113-4fb6-9af0-22539d46684f |
| T8 | Full Tier Coverage (Phase 3) | Tier 1-4 tests covering raycasting accuracy, 1D diffusion/decay, combat calculations, and neural integration | T7 | DONE | 4454e0e2-2113-4fb6-9af0-22539d46684f |
| I15 | Spatial Hash Raycasting | Promote SpatialHashGrid to Bevy resource, implement grid rebuild and ray casting systems | None | DONE | b821cbf7-9edc-4b01-8168-0028113832d0 |
| I16 | Contiguous 1D Pheromone Grid | Implement 1D array pheromone grid with diffusion and decay, write/read systems | None | DONE | fbf568bc-d746-46e1-9c1d-7345cd6dabb1 |
| I17 | Predator-Prey Dynamics | Add Predator/Prey components, chase AI, and combat/metabolic transfer physics | None | DONE | ed197dc9-b210-4017-8ca7-f3edd99a526c |
| I18 | Neural Sensory Integration | Expand Burn model inputs, integrate raycast & olfactory sensors, map to CPG actions | I15, I16 | DONE | dcab9368-20ee-41b1-b81f-63c4c957c8e8 |
| I19 | Telemetry, UI, & E2E Integration | Expose raycast vectors & pheromone states via IPC, render canvas overlays & heatmap, pass E2E | I17, I18, T8 | DONE | 1528b765-d714-4363-a90c-4af1322ce483 |
| I20 | Adversarial Coverage Hardening | Tier 5 white-box adversarial coverage for Phase 3 features | I19 | DONE | eb27d25f-e794-4880-a665-849b6ec5d29b |

### Phase 4: Distributed Universe, Meta-AI & Neo4j
| # | Name | Scope | Dependencies | Status | Agent ID |
|---|------|-------|-------------|--------|----------|
| T9 | Test Suite & Harness Setup (Phase 4) | E2E integration tests design & runner setup for Lineage, Gemini, and Socket migration | None | DONE | 42c1dcad-431c-4001-9085-cb405f06146d |
| T10 | Full Tier Coverage (Phase 4) | Tier 1-4 tests covering Neo4j fallback, Mock LLM events, Socket migration, and UI | T9 | DONE | 42c1dcad-431c-4001-9085-cb405f06146d |
| I21 | Lineage Persistency with Neo4j & Fallback | Rust Neo4j connector, credentials from `.env`, in-memory fallback | None | DONE | ed3bc824-769a-45f6-810e-7c42206789fb |
| I22 | Gemini Mother Nature & Mock Client | Gemini API integration, mock AI client, Bevy chronicle trigger system | I21 | DONE | ed3bc824-769a-45f6-810e-7c42206789fb |
| I23 | Distributed Socket Handoff & Sharding | WebSocket server/client, multi-port local node migration, boundary systems | I22 | DONE | ed3bc824-769a-45f6-810e-7c42206789fb |
| I24 | Lineage Graph & Meta-AI Chronicle UI | React components for SVG family tree, timeline warning alerts | I23 | DONE | ed3bc824-769a-45f6-810e-7c42206789fb |
| I25 | E2E Verification & Integration | Final assembly, pass all Phase 4 E2E tests, verify zero allocation path | I24, T10 | DONE | ed3bc824-769a-45f6-810e-7c42206789fb |
| I26 | Adversarial Coverage Hardening | Tier 5 white-box coverage for Phase 4 features | I25 | DONE | 484c9dc2-93a5-4644-b615-9eaf930f542d |

### Phase 5: Architecture Optimization, GPU Acceleration, High-Performance Frontend & Gemini Web Integration
| # | Name | Scope | Dependencies | Status | Agent ID |
|---|------|-------|-------------|--------|----------|
| T11 | E2E Setup & Infrastructure (Phase 5) | Define & implement test harness, E2E integration tests for PixiJS, Fallback, modularity, reset channels, web-session Gemini | None | DONE | 15f12476-e4ec-44f8-87c1-b750b3d20bb2 |
| I27 | Codebase Modularization & Refactoring | Split engine.rs into distinct submodules (simulation_lifecycle, agent_systems, networking_systems); preserve zero heap allocs | None | DONE | 8d30d2be-6257-4c40-a1e5-ae24faf7529a |
| I28 | Burn GPU Acceleration with CPU Fallback | Transition BrainModel from ndarray to burn-wgpu with NdArray CPU fallback | I27 | DONE | 8d30d2be-6257-4c40-a1e5-ae24faf7529a |
| I29 | Crossbeam Channel Cleanup & Reset | Restructure start/stop/restart state machine to cleanly drain/reset channels | I27 | DONE | 8d30d2be-6257-4c40-a1e5-ae24faf7529a |
| I30 | High-Performance PixiJS Frontend | Port HTML5 Canvas 2D visualization panel to PixiJS WebGL/WebGPU | I27 | DONE | 9d5f58b8-10cd-4b85-9781-332fd119baab |
| I31 | Gemini Web-Session API Integration | Open-source web-session api wrapper integration, queries log to timeline | None | DONE | 9d5f58b8-10cd-4b85-9781-332fd119baab |
| I32 | E2E Verification & Integration | Integrate components, compile backend/frontend, pass Phase 5 E2E tests | I28, I29, I30, I31, T11 | DONE | 9d5f58b8-10cd-4b85-9781-332fd119baab |
| I33 | Adversarial Coverage Hardening | Tier 5 white-box coverage hardening for Phase 5 features | I32 | DONE | 9d5f58b8-10cd-4b85-9781-332fd119baab |

### Phase 6: Photorealistic Landscape Showcase
| # | Name | Scope | Dependencies | Status | Agent ID |
|---|------|-------|-------------|--------|----------|
| T12 | Test Suite & Harness Setup (Phase 6) | Setup Vitest E2E test suite & harness covering feature, boundary, pairwise, and application scenarios for landscape components | None | DONE | ee0b2e77-dcec-45fd-b889-570a358c00f6 |
| I34 | Procedural Heightmap & Hydrology | Layered noise-based 500x500 terrain generation with Level-of-Detail (LOD), biome classification, and river/lake/waterfall systems | None | DONE | 2d8652a3-d26c-4898-a2ee-7f7162969120 |
| I35 | Custom Terrain & Water Shaders | Custom shaders for terrain biome blending and animated water with reflection, wave displacement, and transparency/depth coloring | I34 | DONE | 2d8652a3-d26c-4898-a2ee-7f7162969120 |
| I36 | Instanced Vegetation with Wind Sway | GPU-instanced rendering of multiple tree species and ground cover with dynamic wind sway animation | I34 | DONE | 2d8652a3-d26c-4898-a2ee-7f7162969120 |
| I37 | Day-Night Cycle & Atmospheric sky | Dynamic skybox, procedural/billboard cloud rendering, and a complete day-night cycle with moving shadows | None | DONE | 2d8652a3-d26c-4898-a2ee-7f7162969120 |
| I38 | Weather Transitions & Effects | Dynamic weather rendering (rain splashes, snow accumulation, mist/fog) with smooth transitions | I37 | DONE | 2d8652a3-d26c-4898-a2ee-7f7162969120 |
| I39 | Positional Web Audio & Cameras | Web Audio API integration for 3D positional ambiance and Orbit/Fly camera modes with terrain collision | I34 | DONE | 2d8652a3-d26c-4898-a2ee-7f7162969120 |
| I40 | Dashboard Controls & App Integration | React UI dashboard for weather/time/camera controls, integrated seamlessly into the Vite + React frontend | I38, I39 | DONE | 2d8652a3-d26c-4898-a2ee-7f7162969120 |
| I41 | E2E Verification & Audit Hardening | Integrate components, compile frontend, pass 93 Vitest E2E test cases, and perform audit hardening | I35, I36, I40, T12 | DONE | 2d8652a3-d26c-4898-a2ee-7f7162969120 |
| I42 | Custom GLSL Water Shader Upgrade | Upgrade water bodies to custom ShaderMaterial with depth blending, shoreline foam, Day/Night lighting, and CPU optimization | I41 | DONE | c62e8336-a13e-4dd5-b541-68894ef08c8d |

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
