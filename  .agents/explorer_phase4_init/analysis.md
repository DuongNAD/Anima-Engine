# Anima-Engine Phase 4 Architectural Exploration

## 1. Executive Summary
This document provides a comprehensive analysis of the **Anima-Engine** backend (Rust + Bevy ECS + Tauri v2) and frontend (React + TS + Vite) codebases. The objective is to evaluate the existing design and plan the implementation of **Phase 4 Requirements**:
1. **Neo4j Genotype Lineage Persistency** with in-memory mock fallback.
2. **Gemini LLM "Mother Nature"** environmental chronicles with a mock AI client fallback.
3. **Distributed Socket Migration** for multi-port node-to-node agent transfers.
4. **Lineage Graph & Meta-AI Chronicle UI** dashboards on the frontend.

We analyze the module layouts, Tauri IPC routes, the background simulation thread design, and the test infrastructure to propose precise code changes, directory extensions, and dependency updates.

---

## 2. Rust Backend Structure & Simulation Loop

### 2.1 Backend Directory Layout (`src-tauri/`)
The Rust backend is structured as a single package with a binary entrypoint (`src/main.rs`) and library module (`src/lib.rs`):
- `src/core/`: Initializes the Bevy ECS world and runs the simulation loop.
  - `ecs.rs`: Contains all components (`Agent`, `Position`, `Velocity`, `HomeostaticState`), Bevy resources, and systems (metabolic decay, food spawning, combat, etc.).
  - `engine.rs`: The core engine runner, which manages thread lifecycles, schedules, state synchronization, and Tauri event emissions.
- `src/ai/`: Handles controller networks and environmental sensors.
  - `cpg.rs`: Central Pattern Generators updating joint parameters based on brain outputs.
  - `model.rs`: Autodiff / inference Actor-Critic models using the `Burn` framework.
  - `pheromone.rs`: 1D pheromone diffusion and decay grid logic.
  - `hrrl.rs`: Intrinsic rewards and physiological deviations calculation.
- `src/physics/`: Core physics engines.
  - `dynamics.rs`: Spring-damper constraint solvers resolving multi-segment link attachments.
  - `spatial.rs`: Spatial hash grids pruning raycasting and collision candidate checks.
- `src/evolution/`: Core genetics pipeline.
  - `genotype.rs`: Genotype directed graph node/edge definitions and spawner decoders.
  - `map_elites.rs`: Multi-dimensional archive of phenotypic elites based on locomotion and efficiency.
  - `crossover.rs` & `mutation.rs`: Genetic operators.

### 2.2 Simulation Loop Organization
The simulation loop is implemented in `src-tauri/src/core/engine.rs` under `SimulationEngine::start()`. When started, it spawns **three parallel background threads**:

1. **Simulation Thread (ECS Loop)**:
   - Initializes a Bevy ECS `World` and runs a Bevy `Schedule` containing 18 systems (physics, brain inference, homeostasis, pheromones, combat, learning, and evolution mapping).
   - Decoupled frame-rate control: Executes a precision timing check sleeping the thread to achieve **exactly 60 FPS**.
   - Zero-allocation hot path: Uses pre-allocated vectors (`state_buffer`, `state_raycast_buffer`) and double-buffers state data. Once warmed up, it performs **zero heap allocations** per tick.
2. **Tauri Event Emission Thread**:
   - Periodically wakes up every **33ms (~30 FPS)** to read the double-buffered simulation states.
   - Emits Tauri events (`simulation-tick`, `pheromone-update`, `raycast-update`, `combat-event`) to push states to the frontend.
   - By running on a separate thread, it isolates slow JSON serialization allocation overhead from the main 60 FPS simulation loop.
3. **Asynchronous Evolution Thread**:
   - Receives epoch evaluation batches from the simulation thread via a crossbeam channel.
   - Updates the MAP-Elites grid and writes to the shared atomic settings.
   - Performs parent selection, crossover, and mutation, sending the next generation's genotypes back to the simulation loop thread to spawn offspring.

---

## 3. Tauri IPC Routes & Frontend Layout

### 3.1 Existing IPC Routes
Tauri IPC consists of commands (invoked by frontend) and events (emitted by backend):

| Type | Name | Payload / Return Type | Description |
|---|---|---|---|
| **Command** | `get_simulation_status` | `SimulationStatus` | Retrieves engine loop execution info (FPS, avg tick time). |
| **Command** | `toggle_simulation` | `bool` | Starts or stops the background engine thread. |
| **Command** | `get_map_elites_grid` | `MapElitesGridState` | Retrieves the archived 2D elite grid niches. |
| **Command** | `update_evolution_settings`| `bool` (args: `EvolutionSettings`)| Updates mutation rate, selection bias dynamically. |
| **Command** | `toggle_evolution` | `bool` | Enables or disables offspring generation sweeps. |
| **Command** | `get_pheromone_grid` | `PheromoneGridState` | Queries the current flat float array of pheromones. |
| **Command** | `get_active_raycasts` | `Vec<RaycastTelemetry>`| Queries current sensor vectors for rendering. |
| **Event** | `simulation-tick` | `Vec<SegmentState>` | Delivers 3D positions and angles of all agent joints. |
| **Event** | `map-elites-update` | `MapElitesGridState` | Emitted when a new elite replaces an archived niche. |
| **Event** | `pheromone-update` | `PheromoneGridState` | Pushes the 128x128 grid flat array to the frontend. |
| **Event** | `raycast-update` | `Vec<RaycastTelemetry>`| Pushes active raycast coordinates. |
| **Event** | `combat-event` | `CombatEvent` | Broadcasts predator-on-prey energy drain details. |

### 3.2 Frontend Architecture
The React application (`src/App.tsx`) is currently lightweight and implements:
- **Continuous Rendering Loop**: Uses a Canvas 2D context synced to `requestAnimationFrame` (rAF). It draws the pheromone density heatmap, active sensor raycast beams, distinct agent shapes (Prey = blue circles, Predators = red triangles), and orientation vectors.
- **Hierarchical Tree Parser**: A recursive helper `buildAgentHierarchy` that aggregates flat segments sharing `agent_id` into a parent-child node layout to render a structured visual tree inspector.
- **Interactive Controls**: Sliders modifying the active mutation rate and selection bias, with toggle buttons for the simulation and evolution loops.

---

## 4. Existing Test Harness Analysis

The testing environment implements a multi-tier approach ensuring backend safety and frontend visual correctness:

1. **Rust Integration Tests (`src-tauri/tests/`)**:
   - Separate test suites exist for physics dampening (`physics_tests.rs`), neural nets (`burn_neural_net_tests.rs`), homeostasis decay (`homeostatic_rl_tests.rs`), Map-Elites (`map_elites_tests.rs`), and combat (`predator_prey_combat_tests.rs`).
   - Uses a custom global **`TrackingAllocator`** to programmatically assert that hot-loop operations (physics tick, raycast, combat resolution, pheromone diffusion) perform **exactly 0 heap allocations**.
2. **Frontend Vitest Suite (`tests/frontend/`)**:
   - Uses `JSDOM` and `@testing-library/react` to render dashboards.
   - Mocks the Tauri `@tauri-apps/api/core` invoke system to return stubbed payloads (e.g. `mockPheromoneGridState`, `mockRaycastTelemetry`).
   - Mocks Tauri `emit` calls to push fake event tick frames, verifying that the Canvas drawing context calls (`arc`, `moveTo`, `stroke`, etc.) are triggered with matching parameters.
3. **E2E Playwright Suite (`tests/e2e/`)**:
   - Spawns the live application shell.
   - Employs a **graceful check** that catches connection failures and skips E2E verification if the local Tauri compiler or server is offline, preventing pipeline blockers.

---

## 5. Phase 4 Implementation Strategy & Locations

### 5.1 Neo4j Lineage Client (R1)
- **Proposed Location**: Create a new module `src-tauri/src/evolution/lineage.rs`. Add dependency `neo4rs = "0.7"` (or standard driver) and `dotenvy = "0.15"` to `src-tauri/Cargo.toml`.
- **Structural Blueprint**:
  Define a `LineageTracker` struct that loads environment variables (`NEO4J_URI`, `NEO4J_USER`, `NEO4J_PASSWORD`) from `.env`.
  ```rust
  pub trait LineageStore {
      fn record_generation(&mut self, agent_id: u32, parent_a: Option<u32>, parent_b: Option<u32>, genotype: &MorphologyGenotype);
      fn get_lineage_graph(&self) -> Vec<LineageEdge>;
  }
  ```
- **Fallback Logic**:
  Upon initialization, the system attempts to establish a connection to Neo4j. If it fails, or if `.env` variables are missing, it initializes an in-memory fallback (e.g., standard `RwLock<HashMap<u32, LineageNode>>`).
- **Integration Points**:
  - The asynchronous **Evolution Thread** in `engine.rs` (lines 450-535) performs crossover and mutation, producing offspring. Hook the lineage tracker here:
    ```rust
    let offspring_id = next_id;
    lineage_tracker.record_generation(offspring_id, Some(parent_a_id), Some(parent_b_id), &offspring);
    ```

### 5.2 Gemini LLM "Mother Nature" Client (R2)
- **Proposed Location**: Create `src-tauri/src/ai/meta_ai.rs`. Add `ureq = { version = "2.9", features = ["json"] }` for blocking HTTP requests (or `reqwest`).
- **Structural Blueprint**:
  Define a `MetaAIClient` that communicates with the Gemini model endpoint using `GEMINI_API_KEY` from `.env`.
  ```rust
  pub trait MetaAI {
      fn analyze_telemetry(&self, summary: &str) -> Result<EnvironmentalEvent, String>;
  }
  ```
- **Fallback Logic**:
  If the API key is missing or the server is offline, a `MockMetaAI` implementation is returned. The mock client returns random predefined events (e.g. Drought, Temperature Spike, Predator Wave) every $N$ ticks.
- **Integration Points**:
  - Add a Bevy resource `ChronicleHistory` to store the event feed.
  - Implement a Bevy system `meta_ai_trigger_system` in the ECS schedule. It reads the current world statistics (predator count, prey count, average energy, food density) every $K$ ticks, serializes a brief status report, and triggers the Gemini client in a separate thread (or non-blocking manner) to avoid freezing the simulation loop.
  - Apply the returned event payload:
    - **Resource Drought**: Reduces `max_food_count` in Bevy resource `FoodSpawnSettings`.
    - **Temperature Spike**: Increases decay speed factors in `metabolic_decay_system`.
    - **Predator Wave**: Spawns a batch of Predators.
  - Emit event alerts to the frontend via a new Tauri event `chronicle-event`.

### 5.3 Distributed Socket Migration (R3)
- **Proposed Location**: Create `src-tauri/src/network/migration.rs` (and register `pub mod network;` in `lib.rs`). Add dependency `tungstenite = "0.21"` (WebSocket) or similar lightweight socket.
- **Structural Blueprint**:
  Since `bincode` is already present in `Cargo.toml`, we define a serialize/deserialize envelope:
  ```rust
  #[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
  pub struct MigrationPayload {
      pub genotype: MorphologyGenotype,
      pub homeostasis: HomeostaticState,
      pub position: glam::Vec3,
      pub velocity: glam::Vec3,
      pub agent_class: AgentClass,
  }
  ```
- **Server/Client Port Configurations**:
  - The Tauri app initialization (`run()` in `lib.rs`) should read local server port settings from CLI parameters (e.g. `--port 8080 --peer-port 8081`).
  - Spawn a TCP/WebSocket listener thread on the designated local port. When a connection is established, deserialize the `MigrationPayload` and push it to a thread-safe queue: `Arc<Mutex<Vec<MigrationPayload>>>` represented as a Bevy resource.
- **Integration Points**:
  - In `ecs.rs`, implement a system `migration_receiver_system` that pops received payloads from the Bevy resource queue and spawns them into the Bevy world.
  - Implement a boundary system `migration_boundary_check_system`:
    ```rust
    for (entity, pos, vel, genotype, homeostasis, class) in agents.iter() {
        if pos.x.abs() > bounds.max.x - 5.0 || pos.z.abs() > bounds.max.z - 5.0 {
            // Serialize agent details
            let payload = MigrationPayload { genotype, homeostasis, position: pos, velocity: vel, agent_class: class };
            // Send asynchronously to peer port (e.g. localhost:8081)
            migration_client.send(payload);
            // Despawn agent and segments locally
            despawn_agent(entity);
        }
    }
    ```

### 5.4 Frontend Lineage Graph and Chronicle UI (R4)
- **Proposed Location**: Create components in `src/components/LineageGraph.tsx` and `src/components/ChroniclePanel.tsx`, and import them in `src/App.tsx`.
- **Chronicle Panel UI**:
  - Subscribes to `chronicle-event` on mount.
  - Displays a chronological list of Mother Nature events.
  - Renders a warning header card for active events (e.g., a flashing red banner when a temperature spike or drought is active).
- **Lineage Tree UI**:
  - Invokes `get_lineage_tree` to fetch the relationship tree from the backend.
  - Renders an interactive SVG hierarchy of genotype parentage. Clicking on nodes visualizes morphological segment specifications and fitness metrics.

---

## 6. Required Code & Dependency Adaptations

To support serialization of the agents across both Tauri IPC and the network socket channels, the following structs **must** be updated to derive `serde::Serialize` and `serde::Deserialize`:

1. **`MorphologyGenotype`** in `src-tauri/src/evolution/genotype.rs`:
   ```rust
   // Before:
   #[derive(Clone, Debug)]
   pub struct MorphologyNode { ... }
   
   // Proposed change:
   #[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
   pub struct MorphologyNode {
       pub id: u32,
       pub length: f32,
       pub radius: f32,
       pub mass: f32,
   }

   #[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
   pub struct MorphologyEdge {
       pub source_node: u32,
       pub target_node: u32,
       pub joint_anchor: Vec3,
       pub joint_axis: Vec3,
   }

   #[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
   pub struct MorphologyGenotype {
       pub nodes: Vec<MorphologyNode>,
       pub edges: Vec<MorphologyEdge>,
   }
   ```
2. **`HomeostaticState`** in `src-tauri/src/ai/hrrl.rs`:
   ```rust
   // Proposed change:
   #[derive(Component, serde::Serialize, serde::Deserialize, Clone, Debug)]
   pub struct HomeostaticState {
       pub energy: f32,
       pub energy_target: f32,
       pub hydration: f32,
       pub hydration_target: f32,
       pub temperature: f32,
       pub temp_target: f32,
       pub previous_deviation: f32,
   }
   ```

3. **`Cargo.toml` Additions**:
   ```toml
   [dependencies]
   dotenvy = "0.15"
   neo4rs = "0.7"
   ureq = { version = "2.9", features = ["json"] }
   tungstenite = "0.21"
   tokio = { version = "1.35", features = ["full"] }
   ```
