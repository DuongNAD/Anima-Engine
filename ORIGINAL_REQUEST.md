# Original User Request

## Initial Request — 2026-06-11T13:41:43+07:00

Anima-Engine is a real-time, GPU-accelerated Artificial Life (ALife) and Evolution Simulator. This initial run implements Phase 0 (architectural foundation): setting up the core Rust ECS skeleton, Tauri v2 shell, physics/AI module structures, and automated tests to verify low-latency simulation ticks and IPC.

Working directory: E:\Project\Anima-Engine
Integrity mode: development

## Requirements

### R1. Tauri v2 & Rust ECS Core Setup
Establish a Tauri v2 project structure where the Rust backend initializes a Bevy ECS (or Flecs) World. The simulation tick loop must run on a background thread/task at a target rate (e.g., 60 FPS), updating physical agent entities.

### R2. High-Performance IPC Protocol
Implement Tauri command handlers or event emitters that serialize minimal simulation state (agent positions, rotations, and status) from the ECS world to the TypeScript frontend. Ensure the payload is optimized for high-frequency updates.

### R3. Modular Simulation Blueprint
Create stub structures and modules matching the planned layout:
- `src-tauri/src/physics` (dynamics skeleton)
- `src-tauri/src/ai` (neural networks, CPG, and HRRL stubs)
- `src-tauri/src/evolution` (genotype-to-phenotype blueprint)

### R4. Automated Verification Suite
Create automated test scripts/suites:
- Rust unit/integration tests running via `cargo test` to verify ECS world setup, entity updates, and tick loops.
- Frontend or integration tests to verify the receipt of simulation tick payloads from the backend.

## Acceptance Criteria

### Backend Verification
- [ ] Running `cargo test` in `src-tauri` passes successfully, verifying entity creation and simulation ticks.
- [ ] Rust codebase compiles cleanly without errors or warnings.
- [ ] Simulation hot loop does not perform heap allocations (no dynamic `Vec`/`String` allocation inside the tick update).

### Frontend & IPC Verification
- [ ] TypeScript/Vite application builds successfully (`npm run build`).
- [ ] An automated integration test verifies that IPC events/commands successfully transmit mock agent coordinates from Rust to TypeScript.

## Follow-up — 2026-06-11T17:13:13+07:00

Anima-Engine Phase 1 (Morphological Evolution): Decode directed graph genotypes into 3D articulated body structures (phenotypes) within the Bevy ECS, integrate CPGs to apply rhythmic joint torques, and simulate metabolic energy consumption.

Working directory: E:\Project\Anima-Engine
Integrity mode: development

## Requirements

### R1. Genotype-to-Phenotype Articulated Spawner
Extend the Bevy ECS world to decode a `MorphologyGenotype` (directed graph representing body segments as nodes, joint connections as edges). Spawn the corresponding physical hierarchy (rigid bodies connected by joints) in the ECS. The physics solver (either custom spring-damper constraints, Verlet integration, or an external library like Rapier3d) must be chosen and implemented by the agent team to resolve constraints.

### R2. Joint-Level CPG Controller
Integrate `CpgOscillator` instances with spawned joint constraints. The oscillator output must dynamically drive the target joint angles, angular velocities, or muscle forces during the simulation loop to produce rhythmic leg/arm movements.

### R3. Metabolic Energy Consumption Logic
Formulate and implement metabolic energy depletion in `HomeostaticState`. Energy cost per tick must scale dynamically with:
1. The agent's total body mass (sum of segment masses).
2. Active muscle forces/torques applied by CPGs.
3. Actual joint angular velocities.
If energy hits zero, the agent should transition to an inactive/dead state.

### R4. IPC State Serialization & Frontend Telemetry
Update the Tauri IPC serialization to stream the morphological hierarchy (the relative joint positions and connections) along with the standard spatial tick state to the TypeScript frontend. The frontend telemeter/canvas must reflect these multi-segment agents moving dynamically.

### R5. Automated Verification Suite
Create automated tests to programmatically verify correctness:
- Rust tests (`cargo test`) verifying that a mock genotype is decoded, spawned with proper constraints, moves via CPGs, and depletes energy over ticks.
- Frontend tests verifying the parser can receive and structure the multi-segment agent state correctly.

## Acceptance Criteria

### Morphological Simulation Loop
- [ ] Backend compiles cleanly and `cargo test` passes all tests verifying morphological decoding, constraint resolution, and CPG joint updates.
- [ ] Pre-allocated buffers are used inside the physics and CPG systems to guarantee the hot loop maintains zero-heap-allocation during execution.

### Energy & Metabolic Depletion
- [ ] Automated tests verify that agent energy depletes faster when executing larger joint movements or when carrying heavier bodies, and that they stop moving upon energy exhaustion.

### Frontend Integration
- [ ] TypeScript/Vite application builds successfully (`npm run build`).
- [ ] E2E integration test verifies the frontend successfully parses and displays telemetered jointed morphology data sent over Tauri IPC.

## Follow-up — 2026-06-11T21:37:53+07:00

Anima-Engine Phase 2 (Hybrid Neural Control & MAP-Elites): Integrate lightweight Actor-Critic neural networks using the Burn framework, implement Homeostatic Reinforcement Learning (HRRL) to drive foraging behavior, and run the MAP-Elites evolutionary archive with real-time Tauri IPC controls and dashboard visualization.

Working directory: E:\Project\Anima-Engine
Integrity mode: development

## Requirements

### R1. Actor-Critic Control via Pure-Rust Burn
Implement a lightweight Actor-Critic neural network using the Pure-Rust `Burn` framework. The network must run batched inference (preferably targeting WGPU or CPU backend based on configuration) to parse agent sensory inputs (vector to nearest food, homeostatic states) and update output targets (amplitudes, frequencies) for the joint CPGs.

### R2. Homeostatic Reinforcement Learning (HRRL) & Foraging
Establish environmental food spawning and collision-consumption logic in the Bevy ECS. Implement physiological homeostasis decay (energy and hydration depletion over time) and reinforcement learning weight updates driven by minimizing homeostatic deviations (intrinsic rewards).

### R3. Interactive MAP-Elites Evolutionary Archive
Implement the MAP-Elites grid in `src-tauri/src/evolution/map_elites.rs` to bin agents based on phenotypic behavioral traits (e.g., locomotion speed, metabolic efficiency). The system must support real-time configuration updates (such as changing mutation rates or selection bias) during active simulation sweeps.

### R4. Tauri IPC commands & Frontend Grid Dashboard
Expose Tauri IPC commands (`get_map_elites_grid`, `update_evolution_settings`, `toggle_evolution`) and emit grid/simulation update events. The React/Vite frontend dashboard must render a visual 2D grid matrix of the MAP-Elites archive and allow the user to modify mutation rates and selection parameters on-the-fly.

### R5. Automated Verification Suite
Create automated verification tests:
- Rust tests (`cargo test`) verifying Burn model execution, HRRL learning updates, food replenishment, and MAP-Elites grid operations.
- Frontend tests verifying the deserialization and rendering of the MAP-Elites grid data.

## Acceptance Criteria

### Neural & Evolutionary Simulation
- [ ] Backend compiles successfully with `Burn` framework integration and runs tests without runtime panics.
- [ ] Simulation hot loop performs zero heap allocations during active simulation steps, utilizing double-buffered vectors or pre-allocated tensors.
- [ ] The MAP-Elites grid archives, mutates, and selects parents to evolve agents over multiple generations.

### Interactive Control & IPC
- [ ] The frontend can send real-time settings adjustments (e.g., mutation rate slider updates) via Tauri IPC, which are immediately applied in the ECS simulation.
- [ ] The frontend correctly retrieves and displays the 2D MAP-Elites matrix grid of behavioral niches dynamically.

### Verification Run Results
- [ ] `cargo test` in `src-tauri` passes all tests validating learning updates, food ingestion, and grid binning.
- [ ] `npm run build` compiles frontend assets successfully.

## Follow-up — 2026-06-11T22:43:28+07:00

Anima-Engine Phase 3 (Socialization & Emergent Behaviors): Implement spatial hash-accelerated raycasting, a flat 1D array pheromone grid with diffusion/decay dynamics, predator-prey combat networks, and Tauri IPC telemetry to render active sensor beams and pheromone heatmaps.

Working directory: E:\Project\Anima-Engine
Integrity mode: development

## Requirements

### R1. Spatial Hash-Accelerated Raycasting
Implement a zero-allocation raycasting system in the Bevy ECS. Ray-bounding box intersection checks must run directly on contiguous agent coordinate arrays, leveraging the existing Spatial Hashing system to prune candidate checks, avoiding external physics engine overhead.

### R2. Contiguous 1D Pheromone Grid (Diffusion & Decay)
Model environmental pheromones using a flat, contiguous 1D array of floats representing a fixed-size grid (e.g. 128x128). Implement diffusion (loang) and decay (bay hơi) equations using linear array iterations to guarantee maximum cache locality. Agents must write pheromones to the grid at their coordinates and read surrounding cells via olfactory sensors.

### R3. Predator-Prey Dynamics & Ecological Combat
Classify ECS agents using `Predator` and `Prey` components. Prey feed on spawned food nodes. Predators track and chase Prey, executing combat algorithms upon collision (prey damage, energy transfer, and carcass removal) while tracking metabolic costs.

### R4. Neural Sensory Integration & Emergent Behavior
Feed raycast results (distance, object type) and olfactory pheromone readings as inputs into the Burn-based Actor-Critic networks. Evolve neural control weights to exhibit emergent flocking/schooling (Prey escaping predators) and pack hunting (Predators cornering prey).

### R5. Sensor Telemetry & Heatmap UI
Expose Tauri IPC endpoints to stream active raycast vectors and the flat pheromone grid states. The React/Vite frontend canvas must display active sensor beams, distinct predator/prey geometries, and render the pheromone grid as a dynamic 2D color heatmap.

## Acceptance Criteria

### Performance & Memory Constraints
- [ ] Physics and pheromone updates run within the 60 FPS simulation loop with zero heap allocations during the active hot path.
- [ ] Pheromone diffusion and decay are executed via contiguous 1D float array sweeps (avoiding HashMaps/pointers).

### Ecological & Neural Execution
- [ ] Predators successfully detect, pursue, and consume Prey, transferring homeostatic energy.
- [ ] Rust tests (`cargo test`) verify ray intersection logic against spatial cells, pheromone diffusion rate math, and predator combat calculations.

### Telemetry & Frontend Build
- [ ] The React frontend parses the flat pheromone array and renders it as a WebGL/Canvas 2D heatmap.
- [ ] `npm run build` compiles frontend assets successfully.

## Follow-up — 2026-06-11T17:04:58Z

Anima-Engine Phase 4 (Distributed Universe, Meta-AI & Neo4j): Persist phylogenetic lineages via Neo4j with in-memory fallback, integrate Gemini LLM-driven environmental events with mock AI client testing, establish network socket boundaries for local multi-port agent migration, and add tree visualization dashboards to the Tauri UI.

Working directory: E:\Project\Anima-Engine
Integrity mode: development

## Requirements

### R1. Lineage Persistency with Neo4j & Mock Fallback
Implement a graph database connector in Rust to store agent genotype family trees. The system must support loading connection credentials from `.env` and **must feature an automated mock/in-memory fallback** if Neo4j is offline, allowing unit/integration tests (`cargo test`) to pass without dependencies.

### R2. Gemini LLM "Mother Nature" & Mock Client
Establish a connection to the Gemini API using an API key from `.env` to periodically analyze simulation telemetry and trigger environmental chronicles/events (e.g., resource droughts, temp shifts, predator waves). Implement a **Mock AI client** to simulate LLM responses for offline and automated unit test executions.

### R3. Distributed Socket Handoff & Sharding
Implement WebSocket or gRPC transport layers in Rust enabling agents to serialize, transmit, and deserialize to migrate between simulation nodes. Verify sharding by spawning **two local server instances on different ports** (e.g., 8080 and 8081) and demonstrating seamless agent transfers.

### R4. Lineage Graph & Meta-AI Chronicle Dashboard
Expose Tauri IPC endpoints to query ancestral trees and fetch Meta-AI environmental event timelines. The React/Vite frontend dashboard must display dynamic timeline warning notifications and draw interactive visual family tree diagrams.

### R5. Automated Verification Suite
Add automated verification tests:
- Rust tests (`cargo test`) verifying Neo4j mock fallback operations, mock LLM event trigger logic, and agent serialization/socket transmission.
- Frontend tests verifying lineage tree deserialization and event chronologer panels.

## Acceptance Criteria

### Data & Network Infrastructure
- [ ] Backend compiles successfully, and all tests pass with Neo4j and Gemini API offline (using fallback mocks).
- [ ] Rust tests demonstrate successful agent serialization, socket handoff, and deserialization between two separate local port listeners.
- [ ] Active simulation updates maintain the zero heap allocation constraint in hot loop iterations.

### Interactive Telemetry & UI
- [ ] Tauri frontend builds successfully (`npm run build`).
- [ ] The React dashboard renders visual family tree relationships and logs chronological Meta-AI event warnings.

## Follow-up — 2026-06-12T14:44:20Z

Setup the Anima-Engine project workspace, install dependencies, run and verify the standard backend and frontend unit tests, and confirm that the development environment is fully prepared for optimization and coding.

Working directory: E:/project/Anima-Engine
Integrity mode: benchmark

## Requirements

### R1. Dependency Setup
- Install NPM dependencies in the root directory.
- Verify cargo/rust toolchain is ready.

### R2. Backend Verification
- Run the Rust backend test suite via `cargo test` in the `src-tauri` directory.
- Ensure all tests pass successfully.

### R3. Frontend Verification
- Run the React/TypeScript unit tests via `npm run test:frontend` in the root (or `tests`) directory.
- Ensure all frontend tests pass successfully.

## Acceptance Criteria

### Project Readiness
- [ ] Frontend dependencies are fully installed (no missing imports/types).
- [ ] Backend Rust workspace compiles and tests pass.
- [ ] Frontend tests pass successfully.
- [ ] Verification script or build runs without errors.

## Follow-up — 2026-06-13T05:28:01Z

Fix the React Three Fiber crash and ensure the rabbit procedural morphology renders correctly in the Anima-Engine simulation.

Working directory: e:\project\Anima-Engine
Integrity mode: development

## Requirements

### R1. Resolve Dependency Incompatibility
Downgrade `@react-three/fiber` to version 8 (e.g. `^8.13.0`) in `package.json` to match the React 18 environment and run `npm install` to update the dependencies, resolving the `Cannot read properties of undefined (reading 'S')` crash.

### R2. Fix Buffer Parsing in RabbitVisualizer
Correct the binary buffer parsing in `playground/RabbitVisualizer.tsx`. The Tauri `invoke('get_test_rabbit_state')` command resolves to an `ArrayBuffer` directly. Change `new Float32Array(buffer.buffer)` to `new Float32Array(buffer)` to avoid reading from undefined.

### R3. Verify InstancedMesh Initialization
Ensure that `instancedMesh` in `playground/RabbitVisualizer.tsx` initializes correctly without throwing errors under React Three Fiber version 8.

## Acceptance Criteria

### Dependency and Compilation
- [ ] `@react-three/fiber` package is downgraded to v8 (e.g., `^8.13.0` or matching version compatible with React 18).
- [ ] No compilation or bundler errors occur during `npm run build`.

### Runtime Visualization
- [ ] IPC binary data is successfully fetched from the backend Tauri command `get_test_rabbit_state` and parsed into positions and scales of the rabbit parts.

## Follow-up — 2026-06-13T06:06:10Z

Redesign the rabbit visualizer in Anima-Engine to render a cute, lightweight, low-poly 3D rabbit model with distinct features instead of stretched spheres.

Working directory: e:\project\Anima-Engine
Integrity mode: development

## Requirements

### R1. Redesign Rabbit Geometry using Separate 3D Meshes
Instead of using a single `instancedMesh` with generic stretched spheres, render separate `<mesh>` components for each of the 5 body parts (Body, Head, Left Ear, Right Ear, Hind Legs) so they are arranged properly in 3D space. Position them symmetrically on the Z-axis (e.g., ears and legs placed on left/right Z coordinates) to create a true 3D rabbit depth.

### R2. Add Distinct Rabbit Details (Cute Cartoon Style)
Make the model immediately recognizable as a rabbit by attaching details:
- **Ears**: Add pink inner ear overlay meshes on top of the main ear meshes.
- **Face**: Add two black spheres for eyes and a small pink sphere for the nose attached to the Head.
- **Tail**: Add a fluffy white sphere at the back of the Body.
Use distinct, soft colors (creamy white, grey, pink, and black) instead of plain grey.

### R3. Maintain Lightweight Performance & UI Controls
Keep the geometry counts low-poly (e.g., small segment values in `sphereGeometry` and simple shapes like `boxGeometry` or `capsuleGeometry`). Ensure all animations (hopping, breathing, limb movement) and control sliders (speed, rotation speed) are fully functional with the new 3D model.

## Acceptance Criteria

### Visualization and Aesthetics
- [ ] The model renders as a cute, recognizable low-poly 3D rabbit with white/grey body parts, pink inner ears, black eyes, a pink nose, and a fluffy tail.
- [ ] The rabbit has depth on the Z-axis (symmetrical ears and legs).
- [ ] No compilation or runtime errors.

### Performance
- [ ] The rendering is extremely lightweight, using standard low-vertex Three.js geometries, and runs at 60 FPS in a normal web browser.
- [ ] The control sliders (speed and rotation speed) successfully update the rabbit's animation and rotation in real-time.


## Follow-up — 2026-06-13T16:09:45+07:00

Redesign the 3D rabbit model in both the standalone sandbox and the React visualizer to match the sharp, faceted low-poly papercraft style with dark outlines.

Working directory: e:\project\Anima-Engine
Integrity mode: development

## Requirements

### R1. Implement Faceted Low-Poly Geometries
Configure all rabbit part geometries (Body, Head, Ears, Legs, Tail, Mouth, Snout, Eyes, Nose, Blush) with extremely low segment counts (e.g., 4 to 8 segments) to create a sharp, blocky, and angular silhouette. Use `flatShading: true` on all materials to ensure lighting is faceted per face rather than smooth-shaded.

### R2. Add Sharp Facet Outlines
For every mesh part of the rabbit, generate and overlay a dark outline wireframe using `THREE.EdgesGeometry` and `THREE.LineSegments` (with a dark color like `#1e293b`). The outlines must align precisely with the sharp edges of the faceted geometries, matching the papercraft aesthetic in the reference image.

### R3. Maintain Animations & Details
Keep the hopping, breathing, chewing, and rotation animations fully functional. Retain the detailed elements (muzzle, pink nose, blush cheeks, and eyes), but adapt them to be low-poly, flat-shaded, and outlined to match the overall papercraft style.

### R4. Verify Build and Unit Tests
Ensure all unit tests continue to pass and the project compiles with no TypeScript compilation errors.

## Acceptance Criteria

### Visualization and Style
- [ ] The rabbit renders in a faceted, low-poly papercraft style where each face is flat-shaded (no smooth gradients across vertices).
- [ ] Clear dark outlines are visible on all sharp edges of the rabbit meshes.
- [ ] The model remains animated (hopping and chewing) and interactive.
- [ ] Muzzle, blush cheeks, eyes, and nose are rendered in faceted style with outlines.

### Technical & Quality
- [ ] No Javascript exceptions or console errors.
- [ ] `npm run test:frontend` runs and passes successfully.
- [ ] `npm run build` succeeds.


