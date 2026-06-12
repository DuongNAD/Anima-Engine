# Anima-Engine E2E Testing Infrastructure (Phase 2 & Phase 3)

This document describes the design, feature inventory, testing methodology, and coverage goals for the End-to-End (E2E) testing infrastructure implemented for **Anima-Engine Phase 2 (Neural Control & Evolution)** and **Phase 3 (Socialization & Emergent Behaviors)**.

---

## 1. Testing Infrastructure Design

The testing infrastructure of Anima-Engine integrates deep learning inference, homeostatic reinforcement learning, evolutionary grid archives, spatial raycasting, contiguous pheromone diffusion grids, and predator-prey combat loops into the Bevy ECS, Tauri IPC, and React frontend environment:

```
                  ┌──────────────────────────────┐
                  │      Anima-Engine Test       │
                  └──────────────┬───────────────┘
                                 │
           ┌─────────────────────┼─────────────────────┐
           ▼                     ▼                     ▼
   ┌────────────────┐    ┌────────────────┐    ┌────────────────┐
   │ Mock Frontend  │    │ Mock Backend   │    │ Headless E2E   │
   │ (Vitest + TS)  │    │ (Rust + Cargo) │    │ (Playwright)   │
   └────────┬───────┘    └────────┬───────┘    └────────┬───────┘
            │                     │                     │
            ▼                     ▼                     ▼
   - Simulates sliders   - Runs Bevy ECS loop  - Spawns real app
   - Mocks grid IPC      - Integrates Burn net - Verifies settings
   - Mocks pheromones &  - Verifies HRRL and   - Triggers evolution
     raycast vectors       MAP-Elites grid       grid updates
   - Verifies canvas     - Computes spatial    - Validates telemetry
     render elements       raycasts, pheromone   panel structure
                           grids, and combat
```

1. **Mock Frontend Layer (Vitest + JSDOM)**: Verifies the App dashboard grid rendering, evolution controls, and Phase 3 telemetry visualization. Test cases mock IPC commands (`get_map_elites_grid`, `get_pheromone_grid`, `get_active_raycasts`) and Tauri events (`map-elites-update`, `pheromone-update`, `raycast-update`, `combat-event`, `simulation-tick`) using `@testing-library/react` and custom Vitest mocks.
2. **Mock Backend Layer (Rust Integration Tests)**: Integration tests under `src-tauri/tests` cover neural network inference (`burn_neural_net_tests.rs`), homeostatic decay/collision (`homeostatic_rl_tests.rs`), and MAP-Elites grid binning/selection/crossover/mutation (`map_elites_tests.rs`).
3. **Zero-Heap-Allocation Audit Layer**: Validates that all neural inference, physiological updates, and Bevy ECS systems run in the active hot-loop without any heap allocations (using the custom tracking allocator).
4. **Real E2E Playwright Layer**: Spawns the Tauri Webview (or web app) to ensure page layout, canvas elements, control panels, and IPC connections function correctly under live conditions. Playwright tests employ graceful skip logic if the backend process or dev server is offline.

---

## 2. Feature Inventory

The testing suite covers the following features and requirements for Phase 2 and Phase 3:

| Ref | Feature Name | Description | Verification Method |
|---|---|---|---|
| **F11** | Burn Inference | Batched neural network inference using the Burn framework | Rust integration tests (`burn_neural_net_tests.rs`) |
| **F12** | CPG Generation | Updates joint CPG parameters (amplitudes, frequencies) from neural net outputs | Bevy system integration tests (`burn_neural_net_tests.rs`) |
| **F13** | Homeostasis Decay | Physiological homeostasis decay (energy & hydration depletion over time) | Rust integration tests (`homeostatic_rl_tests.rs`) |
| **F14** | Food Spawn & Eat | Food spawn, collision, and consumption logic in Bevy ECS | Bevy collision and consumption tests |
| **F15** | Intrinsic Reward & RL | Weight updates driven by minimizing homeostatic deviations | Actor-Critic backpropagation & reward tests |
| **F16** | Niche Grid Archiving | MAP-Elites grid binning based on locomotion speed & metabolic efficiency | Grid coordinate mapping tests (`map_elites_tests.rs`) |
| **F17** | Selection & Mutation | Parent selection, mutation, and crossover to evolve genotypes | Evolutionary robust checks (`map_elites_tests.rs`) |
| **F18** | Tauri IPC Commands | Tauri commands (`get_map_elites_grid`, `update_evolution_settings`, `toggle_evolution`) and emitted update events | Vitest mock IPC tests & Rust integration tests |
| **F19** | React Grid UI | React grid rendering of MAP-Elites 2D matrix, interactive mutation rate and selection settings controls | Frontend dashboard Vitest test suite |
| **R1** | Spatial Hashing Raycasting | Computes high-performance sensor beams detecting nearby entities | Vitest canvas coordinate mapping tests & E2E telemetry checks |
| **R2** | Contiguous 1D Pheromone Grid | Contiguous 1D flat float array representing pheromones laid by agents | Vitest mock grid checks, event updates & overlay render tests |
| **R3** | Predator-Prey Combat | Combat logic transferring energy from prey to predators upon collision | Vitest mock event loop checks & UI logs verification |
| **R4** | Neural Sensory Integration | Feeds raycast inputs & local pheromone densities into neural networks | Vitest mock sensory state propagation & render tests |
| **R5** | UI & Telemetry Panel | Live visual telemetry displaying raycast vectors, pheromone heatmaps, and combat logs | Vitest frontend rendering checks & Playwright E2E panel checks |
| **F20** | Neo4j Lineage Persistency | Graph database persistence for family lineages with in-memory fallback | Rust integration tests (`lineage_tests.rs`) |
| **F21** | Gemini LLM Event Chronicle | Periodic LLM environment triggers with offline Mock client fallback | Rust integration tests (`meta_ai_tests.rs`) |
| **F22** | Distributed Socket Migration | WebSocket migration serialization & deserialization between local ports | Rust integration tests (`migration_tests.rs`) |
| **F23** | Lineage Graph UI | SVG tree rendering of family tree lineages on the React dashboard | Frontend Vitest & Playwright E2E tests |
| **F24** | Meta-AI Chronicle UI | Notification warnings and historical timeline of Gemini events | Frontend Vitest & Playwright E2E tests |

---

## 3. Testing Methodology (4-Tier Approach)

### Tier 1 - Feature Coverage (Happy-Path Operations)
- **MAP-Elites controls**: Assert settings updates dispatch the correct payload to the backend and toggle evolution states correctly.
- **Phase 3 telemetry IPC**: Verifies that frontend components successfully invoke `get_pheromone_grid` and `get_active_raycasts` on mount and process returned values.
- **Tauri event listeners**: Verifies that custom event emitters (`pheromone-update`, `raycast-update`, `combat-event`) trigger UI re-renders and update local states.
- **Lineage Graph & Mother Nature commands**: Verifies that frontend components successfully invoke `get_lineage_graph` and `get_chronicle_history` on mount and process returned JSON structures.
- **Tauri event listeners (Phase 4)**: Verifies that custom event emitters (`chronicle-event`, `migration-event`) trigger state updates, react alerts, and timeline log additions.
- **Distributed migration socket transmission**: Verifies local port setup, WebSocket connection, and correct serialized payload delivery under standard network conditions.

### Tier 2 - Boundary & Corner Cases
- **Empty / Default states**: Verifies frontend behavior when no pheromones are present or no raycasts are active.
- **Grid extremes**: Tests mapping coordinates at extreme bounds of the pheromone grid (e.g. at grid width/height index boundaries).
- **Homeostatic limits**: Tests agent rendering opacity levels as energy drops towards 0.
- **Neo4j Offline Fallback**: Verifies backend behavior when Neo4j is offline or credentials in `.env` are invalid. The system must switch to in-memory fallback without crashing.
- **Mock LLM Fallback**: Verifies that the Mother Nature environment engine falls back to generating random, valid offline events (e.g. Drought, Temperature Spike, Predator Wave) every $N$ ticks when the Gemini API is unreachable or no API key is set.
- **Migration socket failure boundaries**: Asserts that when the target migration port is closed, offline, or dropped mid-stream, the agent is not lost, or the server handles socket errors gracefully.

### Tier 3 - Cross-Feature Combinations
- **Canvas Rendering overlays**: Asserts that `canvas.getContext('2d')` drawing methods (`beginPath`, `arc`, `moveTo`, `lineTo`, `fill`, `stroke`) are invoked dynamically with correct color styles, geometries (triangles for predators, circles for prey), and lines (representing active raycast paths) when simulation updates occur.
- **Grid state sync**: Verifies interaction between interactive settings panel, grid updates, and backend telemetry values.
- **SVG Lineage Graph Tree Layout**: Verifies the calculation and rendering of parent-child node relationships, ensuring dynamic drawing of nodes and path strokes in the React hierarchy component.
- **Gemini environmental triggers**: Asserts the interaction between environmental state updates and Bevy simulation parameters, confirming that a Temperature Spike increases metabolic decay or a Drought lowers maximum food count in Bevy systems.

### Tier 4 - Real-World Application Scenarios
- **Headless E2E Simulation**: Playwright tests launch the frontend on `http://localhost:5173`, asserting that the canvas rendering canvas and the Phase 3 panel (with sections: Pheromone Heatmap, Sensor Beams, Combat Event Log) are visible and dynamically structured. Includes graceful skip fallback.
- **Headless E2E Simulation (Phase 4)**: Playwright tests launch the frontend on `http://localhost:5173`, asserting that the SVG lineage graph tree renders nodes correctly and the Mother Nature timeline alert panel lists historical events dynamically with the correct visual banners.

---

## 4. Coverage Goals

For **Phase 3**, coverage targets:
- **Rendering Correctness**: 100% verification that predator and prey entities are drawn with distinct shapes (predator as red triangles, prey as blue circles).
- **IPC Telemetry Pipeline**: Ensure raycasts, flat pheromone grids, and combat events are correctly parsed and rendered overlaying the canvas without frame drops.
- **Failure Resilience**: Assert E2E test suites pass under headless CLI execution by skipping safely if local ports or binaries are unavailable.

For **Phase 4**, coverage targets:
- **Fallback Integrity**: 100% verification that both Neo4j persistence and Gemini LLM integration degrade gracefully to in-memory history mapping and mock events under connection failure/offline states.
- **Zero-Data-Loss Migration**: Ensure agent genotypes, physiological states, positions, and velocities are completely preserved when serialized/deserialized across local sockets.
- **UI Dynamic Rendering**: Ensure the React SVG lineage tree adjusts its nodes dynamically based on active generation queries, and timeline alerts correctly display active warnings without UI blocking.
