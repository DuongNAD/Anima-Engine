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

## Follow-up — 2026-06-17T04:56:32Z

Upgrade an existing procedural ecosystem 3D terrain viewer into a stunning, photorealistic landscape showcase. The current implementation is a single-file HTML/JS app using Three.js r128 with a 200×200 noise-based terrain, basic vertex-colored biomes, instanced vegetation, and animated water. Transform it into a modular, multi-file application integrated into the existing Vite + React + TypeScript project, with dramatically improved visual fidelity across all aspects: terrain, water, vegetation, sky, lighting, weather, audio, and camera controls. Prioritize visual beauty over performance — this is a technical demo/showcase.

Working directory: e:\project\Anima-Engine
Integrity mode: development

### Existing Project Context

The project is a Vite + React + TypeScript app with Tauri backend support. It already has `three` (v0.184) and `@react-three/fiber` as dependencies. The existing source code in `src/` contains a 2D PixiJS viewport (`App.tsx`, `PixiViewport.tsx`) which should be preserved — the new 3D ecosystem viewer should be a **separate page/route or component**, not a replacement.

The user's HTML code (provided below as reference) contains the current terrain generation logic including:
- SimplexNoise-based elevation with continent shaping
- Hydrological system (rivers, lakes, waterfalls via moisture flags)
- Biome coloring by elevation + moisture
- Instanced vegetation (oaks, pines, bushes, rocks)
- Animated water surfaces

This code should be used as **design reference** for terrain generation logic, not copied verbatim. The new implementation should use modern Three.js (v0.184+), TypeScript, and ES modules.

## Requirements

### R1. Realistic Procedural Terrain (500×500+ with LOD)

The terrain must be procedurally generated using layered noise (similar to the reference code's approach but enhanced). It should feature:
- A map scale of at least 500×500 cells with Level-of-Detail (LOD) so distant terrain uses fewer polygons
- Diverse biomes determined by elevation and moisture (ocean, beach, grassland, forest, taiga, alpine rock, snow peaks)
- Terrain features that feel natural — smooth valleys, ridged mountain ranges, gentle rolling hills
- A hydrological system with rivers carving through terrain, alpine lakes on mountain plateaus, and waterfalls where rivers drop elevation sharply

### R2. Advanced Water Rendering

Water surfaces (ocean, rivers, lakes) must look convincingly liquid:
- Reflections of the surrounding terrain and sky on water surfaces
- Animated wave displacement that varies by water body type (calm lake ripples vs. ocean swells vs. river flow)
- Waterfall foam/spray particle effects where rivers drop sharply
- Transparency/depth coloring — shallow water shows the bed, deep water is dark

### R3. Rich Vegetation with Wind Animation

Vegetation should be diverse, dense where biome-appropriate, and feel alive:
- Multiple tree species with distinct silhouettes (broadleaf oaks, conical pines, birch, palm near beaches)
- Ground cover: grass patches, flowers, ferns in appropriate biomes
- Wind animation — trees and grass should sway gently, responding to a global wind direction
- Use GPU instancing for performance with large vegetation counts

### R4. Atmospheric Sky & Lighting (Day-Night Cycle)

The sky and lighting should create dramatic, cinematic moods:
- A dynamic skybox with procedural or HDR sky (sun position, atmospheric scattering)
- Volumetric or billboard clouds that drift across the sky
- A full day-night cycle with sunrise/sunset color transitions, moon, and stars
- God rays / crepuscular rays when the sun is near the horizon
- Shadows that move with the sun position

### R5. Weather Effects

Dynamic weather that visually transforms the scene:
- Rain with visible droplets, splashes on terrain, and wet surface darkening
- Fog/mist that rolls through valleys (volumetric or screen-space)
- Snow particles that accumulate on high-elevation surfaces
- Weather should transition smoothly (clear → cloudy → rain → clear)

### R6. Environmental Audio

Ambient soundscape that responds to the camera's position and surroundings:
- Water sounds (ocean waves, river flow, waterfall roar) with volume based on proximity
- Wind ambiance that increases with altitude and weather intensity
- Bird calls / forest sounds in vegetated areas
- Use Web Audio API with spatial audio positioning

### R7. Full Camera Controls

Smooth, intuitive camera system for exploring the landscape:
- Orbit mode (click-drag to rotate around a point, scroll to zoom)
- Fly-through mode (WASD + mouse look for free exploration)
- Smooth transitions between modes
- Collision with terrain (camera shouldn't go below ground level)
- Optional: cinematic auto-fly path for showcase mode

## Acceptance Criteria

### Terrain Quality
- [ ] Terrain grid is at least 500×500 cells with visible LOD (distant terrain is lower polygon count)
- [ ] At least 5 distinct biomes are visually distinguishable (differentiated by color and vegetation)
- [ ] Rivers visibly carve through terrain connecting higher to lower elevation
- [ ] At least one alpine lake exists on a mountain plateau with flat water surface
- [ ] Terrain has no visible grid artifacts or seams at LOD transitions

### Water Quality
- [ ] Water surfaces show reflections of sky (at minimum; terrain reflections are a bonus)
- [ ] Ocean water has visible animated wave displacement
- [ ] Lake water has calmer, smaller ripples distinct from ocean waves
- [ ] Water color darkens with depth (shallow areas are lighter/more transparent)

### Vegetation
- [ ] At least 3 visually distinct tree types are placed in biome-appropriate locations
- [ ] Trees and/or grass show visible wind sway animation
- [ ] Vegetation density varies by biome (forests are dense, alpine areas are sparse)
- [ ] No vegetation is placed underwater or on snow-capped peaks

### Sky & Lighting
- [ ] Sky color changes over a day-night cycle (sunrise warm → noon bright → sunset orange → night dark)
- [ ] Shadows from directional light move as the sun position changes
- [ ] Clouds are visible in the sky (procedural, billboard, or skybox-based)
- [ ] Scene is lit differently at day vs. night (darker at night with moon/star light)

### Weather
- [ ] At least 2 weather states are implemented (e.g., clear and rain, or clear and fog)
- [ ] Weather transitions smoothly (no abrupt visual pop-in)
- [ ] Weather visually affects the scene (e.g., rain darkens surfaces, fog reduces visibility)

### Audio
- [ ] Ambient sound plays when the scene loads (at least wind or water)
- [ ] Sound volume changes based on camera proximity to sound source (e.g., louder near waterfall)
- [ ] At least 2 distinct environmental sound layers exist (e.g., wind + water)

### Camera
- [ ] Orbit mode works: mouse drag rotates view, scroll zooms in/out
- [ ] Fly-through mode works: WASD moves camera, mouse controls look direction
- [ ] Camera does not clip below the terrain surface
- [ ] Switching between camera modes does not cause jarring jumps

### Technical
- [ ] Project builds successfully with `npm run build` (no TypeScript errors)
- [ ] Application runs without console errors on Chrome latest
- [ ] Code is organized in multiple TypeScript/GLSL files (not a single monolithic file)
- [ ] The existing `src/App.tsx` and Pixi viewport functionality is preserved (not deleted or broken)

---

### Reference Code

The user's current single-file HTML implementation is available as design reference. Key patterns to preserve/improve:
- Continent-shaping via noise + distance falloff
- Moisture flags encoded in float values for hydrology (rivers, lakes, waterfalls)
- Biome color palette based on elevation thresholds
- Instanced mesh approach for vegetation

Here is the full reference HTML code:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Procedural Ecosystem & Zero-Copy Pipeline</title>
    <style>
        body {
            margin: 0;
            overflow: hidden;
            background-color: #87CEEB;
            color: #ffffff;
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
        }
        #canvas-container {
            width: 100vw;
            height: 100vh;
            display: block;
        }
        #ui-panel {
            position: absolute;
            top: 20px;
            left: 20px;
            background: rgba(15, 20, 35, 0.85);
            padding: 20px;
            border-radius: 12px;
            border: 1px solid rgba(255, 255, 255, 0.15);
            backdrop-filter: blur(12px);
            pointer-events: none;
            box-shadow: 0 8px 32px rgba(0,0,0,0.5);
            z-index: 10;
        }
        h1 {
            margin: 0 0 10px 0;
            font-size: 1.2rem;
            color: #00e5ff;
            letter-spacing: 1px;
            text-transform: uppercase;
        }
        p {
            margin: 5px 0;
            font-size: 0.9rem;
            color: #c4d1de;
        }
        .highlight {
            color: #ffaa00;
            font-weight: bold;
        }
    </style>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/simplex-noise/2.4.0/simplex-noise.min.js"></script>
</head>
<body>
    <div id="ui-panel">
        <h1>Ecosystem Engine</h1>
        <p>Map Scale: <span class="highlight">200x200 (Massive)</span></p>
        <p>FPS: <span id="fps-counter" class="highlight">0</span></p>
        <p>Hydrology: <span class="highlight">Alpine Lakes & Waterfalls</span></p>
        <p>Flora: <span class="highlight">Vibrant Oaks, Pines & Autumn Trees</span></p>
    </div>
    <div id="canvas-container"></div>

    <script>
        // === SYSTEM CONFIGURATION ===
        const GRID_WIDTH = 200;
        const GRID_HEIGHT = 200;
        const TOTAL_CELLS = GRID_WIDTH * GRID_HEIGHT;
        const CELL_SIZE = 2.5; 
        
        const ELEVATION_BYTES = TOTAL_CELLS * 4; 
        const MOISTURE_BYTES = TOTAL_CELLS * 4; 
        const TOTAL_BUFFER_SIZE = ELEVATION_BYTES + MOISTURE_BYTES; 
        
        const ELEVATION_OFFSET = 0;
        const MOISTURE_OFFSET = ELEVATION_BYTES;

        // === SCENE INIT ===
        const container = document.getElementById('canvas-container');
        const scene = new THREE.Scene();
        scene.background = new THREE.Color(0x87CEEB); 
        scene.fog = new THREE.FogExp2(0x87CEEB, 0.002); 

        const camera = new THREE.PerspectiveCamera(45, window.innerWidth / window.innerHeight, 0.1, 2000);
        camera.position.set(0, 150, 250); 
        camera.lookAt(0, 0, 0);

        const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
        renderer.setSize(window.innerWidth, window.innerHeight);
        renderer.setPixelRatio(window.devicePixelRatio);
        renderer.shadowMap.enabled = true; 
        renderer.shadowMap.type = THREE.PCFSoftShadowMap;
        container.appendChild(renderer.domElement);

        // === LIGHTING ===
        const hemiLight = new THREE.HemisphereLight(0xffffff, 0x555566, 0.65);
        hemiLight.position.set(0, 300, 0);
        scene.add(hemiLight);

        const dirLight = new THREE.DirectionalLight(0xfff5b6, 0.95); 
        dirLight.position.set(150, 220, -120); 
        dirLight.castShadow = true;
        
        const d = 220; 
        dirLight.shadow.camera.left = -d;
        dirLight.shadow.camera.right = d;
        dirLight.shadow.camera.top = d;
        dirLight.shadow.camera.bottom = -d;
        dirLight.shadow.camera.far = 600;
        dirLight.shadow.bias = -0.0005;
        dirLight.shadow.mapSize.width = 2048;
        dirLight.shadow.mapSize.height = 2048;
        scene.add(dirLight);

        // === BIOME PALETTE ===
        const PALETTE = {
            waterOceanDeep: new THREE.Color(0x0a3b66),
            waterOceanShallow: new THREE.Color(0x1e81d0),
            waterFresh: new THREE.Color(0x2bbdc4),
            waterAlpine: new THREE.Color(0x1a8c9e),
            sand: new THREE.Color(0xe8d69d),
            dryGrass: new THREE.Color(0xb5a65c),
            lushGrass: new THREE.Color(0x6aa84f), 
            forest: new THREE.Color(0x2d5e1e),    
            taiga: new THREE.Color(0x4b6b58),     
            rock: new THREE.Color(0x6b6966),
            snow: new THREE.Color(0xfffafa)
        };

        const LVL_DEEP = 2.0, LVL_SHALLOW = 6.0, LVL_SAND = 9.0;
        const LVL_GRASS = 25.0, LVL_FOREST = 45.0, LVL_ROCK = 65.0;

        function getBiomeColor(elevation, moisture, time = 0) {
            let color = new THREE.Color();
            
            let isLowerFresh = moisture >= 8.0 && moisture < 15.0;
            let isMountainLake = moisture >= 15.0 && moisture < 25.0;
            let isWaterfall = moisture >= 25.0;

            if (isWaterfall) {
                let flow = (Math.sin(elevation * 3.0 - time * 18.0) + 1.0) * 0.5; 
                color.copy(PALETTE.waterFresh).lerp(new THREE.Color(0xffffff), flow * 0.65 + 0.25);
                return color;
            }
            if (isMountainLake) {
                color.copy(PALETTE.waterAlpine);
                return color;
            }

            if (elevation < LVL_DEEP) {
                color.copy(isLowerFresh ? PALETTE.waterFresh : PALETTE.waterOceanDeep);
            } 
            else if (elevation <= LVL_SHALLOW) {
                if (isLowerFresh) {
                    color.copy(PALETTE.waterFresh);
                } else {
                    let t = (elevation - LVL_DEEP) / (LVL_SHALLOW - LVL_DEEP);
                    color.copy(PALETTE.waterOceanDeep).lerp(PALETTE.waterOceanShallow, t);
                }
            } 
            else if (elevation < LVL_SAND) color.copy(PALETTE.sand);
            else if (elevation < LVL_GRASS) {
                color.copy(moisture < 0 ? PALETTE.dryGrass : PALETTE.lushGrass);
            } 
            else if (elevation < LVL_FOREST) {
                if (moisture < -0.2) color.copy(PALETTE.dryGrass); 
                else if (moisture > 0.3) color.copy(PALETTE.forest); 
                else color.copy(PALETTE.lushGrass).lerp(PALETTE.forest, 0.5); 
            } 
            else if (elevation < LVL_ROCK) {
                color.copy(moisture > 0 ? PALETTE.taiga : PALETTE.rock);
            } 
            else color.copy(PALETTE.snow);

            if (!isLowerFresh && !isMountainLake && !isWaterfall) {
                let modMoisture = moisture > 5.0 ? moisture % 10.0 : moisture; 
                color.multiplyScalar(0.85 + Math.min(Math.abs(modMoisture), 0.3));
            }
            
            return color;
        }

        // === TERRAIN INIT ===
        const terrainGeo = new THREE.BufferGeometry();
        const numTriangles = (GRID_WIDTH - 1) * (GRID_HEIGHT - 1) * 2;
        const positions = new Float32Array(numTriangles * 3 * 3);
        const colors = new Float32Array(numTriangles * 3 * 3);

        let vIdx = 0;
        for (let y = 0; y < GRID_HEIGHT - 1; y++) {
            for (let x = 0; x < GRID_WIDTH - 1; x++) {
                const x0 = x * CELL_SIZE - (GRID_WIDTH * CELL_SIZE / 2);
                const z0 = y * CELL_SIZE - (GRID_HEIGHT * CELL_SIZE / 2);
                const x1 = (x + 1) * CELL_SIZE - (GRID_WIDTH * CELL_SIZE / 2);
                const z1 = (y + 1) * CELL_SIZE - (GRID_HEIGHT * CELL_SIZE / 2);

                positions[vIdx*3] = x0; positions[vIdx*3+1] = 0; positions[vIdx*3+2] = z0; vIdx++;
                positions[vIdx*3] = x0; positions[vIdx*3+1] = 0; positions[vIdx*3+2] = z1; vIdx++;
                positions[vIdx*3] = x1; positions[vIdx*3+1] = 0; positions[vIdx*3+2] = z0; vIdx++;

                positions[vIdx*3] = x1; positions[vIdx*3+1] = 0; positions[vIdx*3+2] = z0; vIdx++;
                positions[vIdx*3] = x0; positions[vIdx*3+1] = 0; positions[vIdx*3+2] = z1; vIdx++;
                positions[vIdx*3] = x1; positions[vIdx*3+1] = 0; positions[vIdx*3+2] = z1; vIdx++;
            }
        }

        terrainGeo.setAttribute('position', new THREE.BufferAttribute(positions, 3));
        terrainGeo.setAttribute('color', new THREE.BufferAttribute(colors, 3));

        const terrainMat = new THREE.MeshPhongMaterial({
            vertexColors: true, flatShading: true, shininess: 0, specular: 0x111111
        });
        const terrainMesh = new THREE.Mesh(terrainGeo, terrainMat);
        terrainMesh.receiveShadow = true;
        terrainMesh.castShadow = true;
        scene.add(terrainMesh);

        // === INSTANCED MESHES (VEGETATION & ROCKS) ===
        const MAX_OAKS = 5000;
        const MAX_PINES = 5000;
        const MAX_BUSHES = 6000;
        const MAX_ROCKS = 2500;
        const dummy = new THREE.Object3D();
        const colorObj = new THREE.Color();

        const woodMat = new THREE.MeshPhongMaterial({ color: 0x543b23, flatShading: true });
        const leafMat = new THREE.MeshPhongMaterial({ color: 0xffffff, flatShading: true }); 
        const rockMat = new THREE.MeshPhongMaterial({ color: 0xffffff, flatShading: true });

        const oakTrunkIM = new THREE.InstancedMesh(new THREE.CylinderGeometry(0.35, 0.5, 2.5, 5), woodMat, MAX_OAKS);
        const oakLeafIM = new THREE.InstancedMesh(new THREE.DodecahedronGeometry(2.2, 1), leafMat, MAX_OAKS);
        
        const pineTrunkIM = new THREE.InstancedMesh(new THREE.CylinderGeometry(0.25, 0.4, 2.0, 5), woodMat, MAX_PINES);
        const pineLeaf1IM = new THREE.InstancedMesh(new THREE.ConeGeometry(2.0, 3.5, 5), leafMat, MAX_PINES);
        const pineLeaf2IM = new THREE.InstancedMesh(new THREE.ConeGeometry(1.5, 2.8, 5), leafMat, MAX_PINES);
        const pineLeaf3IM = new THREE.InstancedMesh(new THREE.ConeGeometry(0.9, 2.0, 5), leafMat, MAX_PINES);

        const bushIM = new THREE.InstancedMesh(new THREE.DodecahedronGeometry(1.2, 0), leafMat, MAX_BUSHES);
        const rockIM = new THREE.InstancedMesh(new THREE.DodecahedronGeometry(1.5, 0), rockMat, MAX_ROCKS);

        [oakTrunkIM, oakLeafIM, pineTrunkIM, pineLeaf1IM, pineLeaf2IM, pineLeaf3IM, bushIM, rockIM].forEach(mesh => {
            mesh.castShadow = true; mesh.receiveShadow = true;
            scene.add(mesh);
        });

        // === TERRAIN GENERATION WITH HYDROLOGY ===
        const noiseElev = new SimplexNoise('terrain-elev-master');
        const noiseMoist = new SimplexNoise('terrain-moist-master'); 
        const noiseRiver = new SimplexNoise('terrain-river-master'); 
        const noiseLake = new SimplexNoise('terrain-lake-master');   
        
        let baseElevation = new Float32Array(TOTAL_CELLS);
        let baseMoisture = new Float32Array(TOTAL_CELLS);
        let isMapGenerated = false;

        function generateStaticMap() {
            let cOak = 0, cPine = 0, cBush = 0, cRock = 0;

            for (let y = 0; y < GRID_HEIGHT; y++) {
                for (let x = 0; x < GRID_WIDTH; x++) {
                    const i = y * GRID_WIDTH + x;
                    let nx = x / GRID_WIDTH - 0.5; 
                    let nz = y / GRID_HEIGHT - 0.5;

                    let e = 1.00 * noiseElev.noise2D(nx * 2.5, nz * 2.5)
                          + 0.50 * noiseElev.noise2D(nx * 5, nz * 5)
                          + 0.25 * noiseElev.noise2D(nx * 10, nz * 10);
                    e = (e / 1.75 + 1.0) / 2.0; 
                    
                    let dist = Math.sqrt(nx*nx + nz*nz) * 2.2; 
                    e = e * Math.max(0, 1.0 - Math.pow(dist, 1.8)); 
                    let elevation = Math.pow(e, 1.5) * 90.0; 
                    let originalElevation = elevation;

                    let m = 1.00 * noiseMoist.noise2D(nx * 3, nz * 3) + 0.50 * noiseMoist.noise2D(nx * 6, nz * 6);
                    m = m / 1.5;

                    let isLake = false;
                    let isMountainLake = false;
                    let isRiver = false;
                    let isWaterfall = false;

                    let lakeNoise = noiseLake.noise2D(nx * 3.5, nz * 3.5);
                    if (lakeNoise > 0.6) {
                        let lakeAlpha = Math.min(1.0, (lakeNoise - 0.6) / 0.12); 
                        let lakeLevel = LVL_SHALLOW - 0.5;
                        
                        if (originalElevation > 55.0) { 
                            lakeLevel = 55.0; 
                            if (lakeNoise > 0.63) isMountainLake = true;
                        } else if (originalElevation > 35.0) {
                            lakeLevel = 35.0;
                            if (lakeNoise > 0.63) isMountainLake = true;
                        } else {
                            if (lakeNoise > 0.63) isLake = true;
                        }

                        if (originalElevation > lakeLevel) {
                            elevation = originalElevation * (1.0 - lakeAlpha) + lakeLevel * lakeAlpha;
                            m += lakeAlpha * 2.5; 
                        }
                    }

                    let warpX = noiseRiver.noise2D(nx * 2.5, nz * 2.5) * 0.2;
                    let warpZ = noiseRiver.noise2D(nx * 2.5 + 5.3, nz * 2.5 + 2.1) * 0.2;
                    let riverNoise = Math.abs(noiseRiver.noise2D(nx * 1.5 + warpX, nz * 1.5 + warpZ));

                    if (riverNoise < 0.05 && originalElevation < LVL_ROCK + 15.0) { 
                        let riverAlpha = 1.0 - (riverNoise / 0.05);
                        let valleyAlpha = Math.pow(riverAlpha, 0.7);
                        
                        let carveDepth = 3.0 + (originalElevation / 100.0) * 45.0; 
                        let targetElevation = Math.max(LVL_SHALLOW - 0.5, originalElevation - carveDepth);
                        
                        elevation = elevation * (1.0 - valleyAlpha) + targetElevation * valleyAlpha;
                        m += valleyAlpha * 3.5;
                        
                        if (riverNoise < 0.012) {
                            if (elevation > LVL_SHALLOW + 1.0 && elevation < originalElevation - 5.0) {
                                isWaterfall = true;
                            } else {
                                isRiver = true;
                            }
                        } 
                    }

                    if (isWaterfall) {
                        m = 30.0 + m; 
                    } else if (isMountainLake) {
                        elevation = Math.floor(elevation);
                        m = 20.0 + m;
                    } else if (isLake || isRiver) {
                        if (elevation <= LVL_SHALLOW + 1.0) {
                            elevation = Math.min(elevation, LVL_SHALLOW - 0.2); 
                            m = 10.0 + m;
                        } else {
                            m = 30.0 + m; 
                        }
                    }

                    baseElevation[i] = elevation;
                    baseMoisture[i] = m;

                    const worldX = x * CELL_SIZE - (GRID_WIDTH * CELL_SIZE / 2);
                    const worldZ = y * CELL_SIZE - (GRID_HEIGHT * CELL_SIZE / 2);
                    const rnd = Math.random();

                    if (elevation >= LVL_SAND && elevation < LVL_ROCK && m < 8.0) {
                        let posX = worldX + (Math.random() - 0.5) * CELL_SIZE;
                        let posZ = worldZ + (Math.random() - 0.5) * CELL_SIZE;
                        let scale = 0.6 + Math.random() * 0.8;

                        if (m <= -0.2) {
                            if (rnd < 0.08 && cBush < MAX_BUSHES) {
                                dummy.position.set(posX, elevation, posZ); dummy.scale.set(scale, scale*0.5, scale); dummy.updateMatrix();
                                bushIM.setMatrixAt(cBush, dummy.matrix); 
                                colorObj.copy(PALETTE.dryGrass).lerp(new THREE.Color(0xa39247), Math.random());
                                bushIM.setColorAt(cBush, colorObj);
                                cBush++;
                            } else if (rnd < 0.12 && cRock < MAX_ROCKS) {
                                dummy.position.set(posX, elevation - (scale * 0.5), posZ); dummy.scale.set(scale*1.5, scale, scale*1.5); 
                                dummy.rotation.set(rnd*3, rnd*3, rnd*3); dummy.updateMatrix();
                                rockIM.setMatrixAt(cRock, dummy.matrix); 
                                colorObj.setHex(0x6e6e6e).multiplyScalar(0.7 + Math.random() * 0.6); 
                                rockIM.setColorAt(cRock, colorObj);
                                cRock++;
                            }
                        }
                        else if (elevation >= LVL_GRASS + 5 && elevation < LVL_ROCK - 5 && m > -0.2) {
                            if (rnd < 0.18 && cPine < MAX_PINES) {
                                dummy.position.set(posX, elevation + (1.0 * scale), posZ); dummy.scale.set(scale, scale, scale); dummy.updateMatrix(); pineTrunkIM.setMatrixAt(cPine, dummy.matrix);
                                colorObj.copy(PALETTE.forest).lerp(new THREE.Color(0x193817), Math.random()); 
                                
                                dummy.position.set(posX, elevation + (2.2 * scale), posZ); dummy.updateMatrix(); pineLeaf1IM.setMatrixAt(cPine, dummy.matrix); pineLeaf1IM.setColorAt(cPine, colorObj);
                                dummy.position.set(posX, elevation + (3.4 * scale), posZ); dummy.updateMatrix(); pineLeaf2IM.setMatrixAt(cPine, dummy.matrix); pineLeaf2IM.setColorAt(cPine, colorObj);
                                dummy.position.set(posX, elevation + (4.5 * scale), posZ); dummy.updateMatrix(); pineLeaf3IM.setMatrixAt(cPine, dummy.matrix); pineLeaf3IM.setColorAt(cPine, colorObj);
                                cPine++;
                            }
                        }
                        else if (elevation < LVL_FOREST && m > 0.1) {
                            if (rnd < 0.15 && cOak < MAX_OAKS) {
                                dummy.position.set(posX, elevation + (1.2 * scale), posZ); dummy.scale.set(scale, scale, scale); dummy.rotation.y = rnd * Math.PI; dummy.updateMatrix(); oakTrunkIM.setMatrixAt(cOak, dummy.matrix);
                                dummy.position.set(posX, elevation + (3.0 * scale), posZ); dummy.scale.set(scale, scale*0.8, scale); dummy.updateMatrix(); oakLeafIM.setMatrixAt(cOak, dummy.matrix);
                                
                                if (Math.random() < 0.1) colorObj.setHex(0xd47a1c).lerp(new THREE.Color(0xc94b1c), Math.random());
                                else colorObj.copy(PALETTE.lushGrass).lerp(PALETTE.forest, Math.random());
                                
                                oakLeafIM.setColorAt(cOak, colorObj);
                                cOak++;
                            } else if (rnd < 0.25 && cBush < MAX_BUSHES) {
                                dummy.position.set(posX, elevation, posZ); dummy.scale.set(scale, scale*0.6, scale); dummy.updateMatrix();
                                bushIM.setMatrixAt(cBush, dummy.matrix); 
                                colorObj.copy(PALETTE.lushGrass).lerp(new THREE.Color(0x568241), Math.random());
                                bushIM.setColorAt(cBush, colorObj);
                                cBush++;
                            }
                        }
                    }
                }
            }

            oakTrunkIM.count = oakLeafIM.count = cOak;
            pineTrunkIM.count = pineLeaf1IM.count = pineLeaf2IM.count = pineLeaf3IM.count = cPine;
            bushIM.count = cBush; rockIM.count = cRock;

            [oakTrunkIM, oakLeafIM, pineTrunkIM, pineLeaf1IM, pineLeaf2IM, pineLeaf3IM, bushIM, rockIM].forEach(m => {
                m.instanceMatrix.needsUpdate = true;
                if(m.instanceColor) m.instanceColor.needsUpdate = true;
            });
            isMapGenerated = true;
        }

        function fetchBinaryPayloadFromRust(time) {
            if (!isMapGenerated) generateStaticMap();

            const buffer = new ArrayBuffer(TOTAL_BUFFER_SIZE);
            const elevationsMock = new Float32Array(buffer, ELEVATION_OFFSET, TOTAL_CELLS);
            const moistureMock = new Float32Array(buffer, MOISTURE_OFFSET, TOTAL_CELLS);
            
            for (let i = 0; i < TOTAL_CELLS; i++) {
                const x = i % GRID_WIDTH;
                const z = Math.floor(i / GRID_WIDTH);
                let elevation = baseElevation[i];
                let moisture = baseMoisture[i];
                
                let isLowerFresh = moisture >= 8.0 && moisture < 15.0;
                let isMountainLake = moisture >= 15.0 && moisture < 25.0;
                let isWaterfall = moisture >= 25.0;
                
                if (elevation <= LVL_SHALLOW && !isMountainLake && !isWaterfall) {
                    if (isLowerFresh) {
                        elevation = LVL_SHALLOW * 0.95 + Math.sin(x * 0.5 + time * 3.0) * Math.cos(z * 0.5 + time * 2.0) * 0.08;
                    } else {
                        elevation = LVL_SHALLOW * 0.85 + Math.sin(x * 0.15 + time * 1.5) * Math.cos(z * 0.15 + time * 1.0) * 0.4;
                    }
                } 
                else if (isMountainLake) {
                    elevation += Math.sin(x * 0.8 + time * 2.0) * Math.cos(z * 0.8 + time * 1.5) * 0.05;
                } 
                else if (isWaterfall) {
                    elevation += Math.sin(time * 25.0 + x * 3.0) * 0.15;
                }
                
                elevationsMock[i] = elevation;
                moistureMock[i] = moisture;
            }
            return buffer;
        }

        // === SMOOTH CAMERA CONTROLS ===
        let mouseX = 0, mouseY = 0;
        let targetCamX = 0, targetCamY = 160, targetCamZ = 240;

        document.addEventListener('mousemove', (event) => {
            mouseX = (event.clientX - window.innerWidth / 2) * 0.4;
            mouseY = (event.clientY - window.innerHeight / 2) * 0.4;
        });

        document.addEventListener('wheel', (event) => {
            targetCamY += event.deltaY * 0.1;
            targetCamZ += event.deltaY * 0.15;
            targetCamY = Math.max(30, Math.min(300, targetCamY));
            targetCamZ = Math.max(60, Math.min(450, targetCamZ));
        });

        // === RENDER LOOP ===
        const clock = new THREE.Clock();
        let frameCount = 0, lastTime = performance.now();
        const fpsElement = document.getElementById('fps-counter');

        function animate() {
            requestAnimationFrame(animate);
            const time = clock.getElapsedTime();

            const rawBuf = fetchBinaryPayloadFromRust(time);
            const elevView = new Float32Array(rawBuf, ELEVATION_OFFSET, TOTAL_CELLS);
            const moistView = new Float32Array(rawBuf, MOISTURE_OFFSET, TOTAL_CELLS);

            const posArray = terrainGeo.attributes.position.array;
            const colArray = terrainGeo.attributes.color.array;

            let cIdx = 0, pIdx = 1; 

            for (let y = 0; y < GRID_HEIGHT - 1; y++) {
                for (let x = 0; x < GRID_WIDTH - 1; x++) {
                    const i00 = y * GRID_WIDTH + x, i10 = y * GRID_WIDTH + (x + 1);
                    const i01 = (y + 1) * GRID_WIDTH + x, i11 = (y + 1) * GRID_WIDTH + (x + 1);

                    posArray[pIdx] = elevView[i00]; pIdx += 3;
                    posArray[pIdx] = elevView[i01]; pIdx += 3;
                    posArray[pIdx] = elevView[i10]; pIdx += 3;

                    const color1 = getBiomeColor((elevView[i00] + elevView[i01] + elevView[i10])/3, moistView[i00], time);
                    for(let i=0; i<3; i++) { colArray[cIdx++] = color1.r; colArray[cIdx++] = color1.g; colArray[cIdx++] = color1.b; }

                    posArray[pIdx] = elevView[i10]; pIdx += 3;
                    posArray[pIdx] = elevView[i01]; pIdx += 3;
                    posArray[pIdx] = elevView[i11]; pIdx += 3;

                    const color2 = getBiomeColor((elevView[i10] + elevView[i01] + elevView[i11])/3, moistView[i00], time);
                    for(let i=0; i<3; i++) { colArray[cIdx++] = color2.r; colArray[cIdx++] = color2.g; colArray[cIdx++] = color2.b; }
                }
            }

            terrainGeo.attributes.position.needsUpdate = true;
            terrainGeo.attributes.color.needsUpdate = true;
            terrainGeo.computeVertexNormals();

            camera.position.x += (mouseX - camera.position.x) * 0.05;
            camera.position.y += (targetCamY - mouseY - camera.position.y) * 0.05;
            camera.position.z += (targetCamZ - camera.position.z) * 0.05;
            camera.lookAt(0, 30, 0);

            renderer.render(scene, camera);

            frameCount++;
            const now = performance.now();
            if (now - lastTime >= 1000) {
                fpsElement.textContent = frameCount;
                frameCount = 0; lastTime = now;
            }
        }

        window.addEventListener('resize', () => {
            camera.aspect = window.innerWidth / window.innerHeight;
            camera.updateProjectionMatrix();
            renderer.setSize(window.innerWidth, window.innerHeight);
        });

        animate();
    </script>
</body>
</html>
```

## Follow-up — 2026-06-17T06:19:08Z

Fix critical rendering bugs in the existing Landscape Showcase at `e:\project\Anima-Engine\src\components\Landscape\`. The showcase currently renders a black screen because of coordinate system mismatches and camera issues. All component files already exist and have good architecture — they just need targeted bug fixes to work correctly.

Working directory: e:\project\Anima-Engine
Integrity mode: development

## Context

The standalone entry point is at `landscape.html` → `src/landscape.tsx`, which renders `LandscapeShowcase` directly (bypassing the Tauri-dependent `App.tsx`). Run `npm run dev` and open `http://localhost:5173/landscape.html` to test.

The project uses Vite + React + Three.js (`three` v0.184) + `@react-three/fiber` (v8.13). All source files are in `src/components/Landscape/`.

## Requirements

### R1. Fix Coordinate System (CRITICAL — root cause of black screen)

All terrain/water geometry uses **XY plane with Z-up** (positions[i*3]=X, positions[i*3+1]=Y, positions[i*3+2]=Z_height). But Three.js and R3F convention is **XZ plane with Y-up** (X=horizontal, Y=height, Z=depth).

Files to fix:
- `Terrain.tsx` lines 60-62: `posX = gx - width/2; posY = gy - height/2; posZ = cell.elevation * 0.15` → should be `posX, elevation_Y, posZ`
- `Water.tsx` lines 141-148: same XY→XZ issue
- `Water.tsx` vertex shader lines 33-39: wave displacement on `pos.z` should be `pos.y`
- `Water.tsx` waterfall particle velocities (line 231): falling in Z should be falling in Y
- `CameraControls.tsx`: fly mode moves in X/Z correctly but terrain height lookup uses Z incorrectly

The camera in `LandscapeShowcase.tsx` is at `[0, 5, 20]` — after fixing coordinates, terrain should be visible at this position.

### R2. Fix Camera Controls

- **Orbit mode** currently does nothing. Implement proper orbit: mouse drag to rotate around terrain center, scroll to zoom in/out.
- **All modes** need `camera.lookAt()` — currently camera position moves but never points at anything.
- **Fly mode** should include mouse look (pointer lock or mouse movement for yaw/pitch).
- After fixes, the default view should show the full terrain landscape from a good angle.

### R3. Fix Vegetation Rendering

`Vegetation.tsx` creates `instancedMesh` elements with correct counts but never calls `setMatrixAt()` to position individual instances. Trees/bushes/rocks all render at origin (0,0,0) instead of their biome-appropriate positions.

Fix: use terrain data to place vegetation instances at correct world positions using `setMatrixAt()` with proper transforms. Also implement the wind sway animation using the `windSpeed` prop (currently accepted but unused).

### R4. Fix Terrain Scale and Camera Defaults

- Terrain elevation multiplier is only `0.15` which makes the terrain extremely flat. Increase to make hills and mountains visible.
- Default camera position should give a good overview of the landscape — position it higher and further back (e.g., `[0, 40, 80]`) looking toward the terrain center.
- The LOD setup in `Terrain.tsx` uses `addLevel()` imperatively — make sure it works with R3F's render loop.

### R5. Ensure Clean Build and Runtime

- Run `npm run build` — fix any TypeScript errors
- Run `npm run dev` and open `http://localhost:5173/landscape.html` — fix any console errors
- The landscape should visually show: colored terrain with biomes, water bodies, vegetation, sky with clouds, and lighting

## Acceptance Criteria

### Visual Rendering
- [ ] Opening `http://localhost:5173/landscape.html` shows a colored 3D terrain landscape (NOT a black screen)
- [ ] Terrain has visible elevation changes (hills, valleys, mountains are distinguishable)
- [ ] At least 3 biome colors are visible on the terrain (water blue, grass green, snow white, etc.)
- [ ] Water surfaces are visible and animated (ocean/lake/river)
- [ ] Sky dome is visible with appropriate coloring
- [ ] At least some vegetation (trees/bushes) is visible on the terrain at correct positions

### Camera Controls  
- [ ] Default camera view shows a good overview of the landscape on page load
- [ ] Mouse scroll zooms in/out
- [ ] Mouse drag rotates the view
- [ ] Pressing WASD in fly mode moves the camera

### Technical
- [ ] `npm run build` succeeds with 0 TypeScript errors
- [ ] Browser console shows 0 errors when viewing the landscape page
- [ ] Coordinate system consistently uses Y-up (Three.js convention) across all components

```
## Follow-up — 2026-06-17T18:11:40+07:00

Migrate the fully functional procedural 3D ecosystem map logic from the vanilla single-file prototype `public/ecosystem.html` and integrate it into the modular React + TypeScript + React Three Fiber (R3F) application located in `src/components/Landscape/`. The entry point for testing is `http://localhost:5173/landscape.html` (which renders `src/landscape.tsx`).

Working directory: e:\project\Anima-Engine
Integrity mode: development

## Requirements

### R1. Migrate Terrain, Biome, and Vegetation Logic to React / R3F
Migrate the heightmap generation (simplex noise with 180 height, soft edge falloff, mountain ridges, and smooth natural basins) and the biome color mapping to `Terrain.tsx`. Migrate the instanced vegetation (oak, pine, palm, jungle, birch, cactus, flowers, bushes, rocks) to `Vegetation.tsx` using R3F's `<instancedMesh>`. Ensure that vegetation instances reset rotation properly on each iteration and only render if count > 0 (to avoid renderer buffer issues).

### R2. Migrate Water Layer (Ocean, Lakes, and River Mesh)
Implement the water layers in `Water.tsx`:
- **Ocean**: Flat plane at Y=5.5.
- **Lakes**: Circular water planes (`THREE.CircleGeometry` based on natural basin / minRim height check to prevent floating water).
- **River Mesh**: A single 3D mesh (`riverMesh`) covering all riverbeds, waterfalls, and ponds, slightly elevated (Y + 0.15) above the terrain. In the R3F loop, animate the Y coordinates of the river vertices with flowing waves while keeping the underlying terrain static. Water materials must be shiny and semi-transparent.

### R3. Camera Ground Collision and Control Sync
In `CameraControls.tsx`, integrate the Orbit, Fly, and Cinematic camera modes. Add ground-collision prevention: the camera height (Y) must never go below the terrain height at that position (using a height-lookup helper `getTerrainHeight(x, z)` based on the generated elevation data). When clicking the minimap, the camera target should align with the actual terrain height at the selected coordinates instead of a fixed height.

### R4. Sync UI, Minimap, and Web Audio
Ensure that `LandscapeShowcase.tsx` orchestrates all parts:
- **Minimap**: The 2D Canvas minimap must draw the terrain, lakes, and rivers accurately and track the camera's position. Clicking the minimap must move the camera target correctly.
- **Controls Overlay**: Sync the speed slider (time speed), volume slider (ambient sound), and weather selection buttons.
- **Audio**: Integrate the procedural Web Audio synthesizer from `public/ecosystem.html` to generate ambient wind/rain/snow sounds.

### R5. Code Cleanup and Build Success
Remove the prototype `public/ecosystem.html` once the migration is complete. Ensure that `npm run build` succeeds with zero TypeScript or bundler errors, and the app loads without console errors on `http://localhost:5173/landscape.html`.

---

## Acceptance Criteria

### Compilation & Runtime
- [ ] `npm run build` succeeds with 0 errors.
- [ ] Loading `http://localhost:5173/landscape.html` shows no errors or warnings in the browser console.

### Rendering & Aesthetics
- [ ] The terrain displays realistic mountain ranges (up to height 180) with snow caps and biome coloring.
- [ ] Lakes are perfectly circular and contained inside natural mountain basins (no floating water planes).
- [ ] Rivers, waterfalls, and ponds have a shiny, semi-transparent 3D water layer that animations dynamically with flowing ripples.
- [ ] Vegetation (trees, bushes, rocks, cacti) is placed in correct biomes and renders with correct orientation/color.

### Camera & UI Controls
- [ ] Camera cannot go below the terrain surface in Orbit or Fly modes.
- [ ] Clicking the minimap centers the camera look-at target on the actual terrain surface.
- [ ] Minimap renders the map layout accurately (terrain, lakes, rivers) and shows the red dot tracking the camera.
- [ ] Sound volume and time speed sliders function as expected.

## Follow-up — 2026-06-17T19:42:54+07:00

Conduct a comprehensive visual and technical audit of the Anima Engine's 3D ecosystem map (`ecosystem.html`). Use browser agents to explore the map, identify any illogical terrain, water, or vegetation placements, and fix the underlying generation algorithms.

Working directory: e:/project/Anima-Engine
Integrity mode: development

## Requirements

### R1. Visual & Logical Audit
Use browser automation to inspect the map from multiple angles and times of day. Look for floating vegetation, water on steep slopes, jagged unnatural terrain spikes, and rendering glitches. 

### R2. Safe Algorithm Refinement
Adjust the procedural generation rules (slopes, noise thresholds, height mapping) in `ecosystem.html` to fix identified anomalies. Do not perform major rewrites of the core terrain generation engine; focus on tweaking the existing logic safely to preserve the current mountain ranges and biomes.

## Acceptance Criteria

### Map Quality & Logic
- [ ] No objects (trees, rocks, foliage) are floating above or clipping underneath the terrain surface.
- [ ] Water bodies strictly conform to logical depressions and valleys (no "slanted" water on hillsides).
- [ ] No extremely jagged or single-vertex terrain spikes that look like rendering bugs.

### Verification
- [ ] The browser agent can successfully orbit the island at varying camera angles without encountering visual anomalies.
- [ ] Map generation finishes quickly without any JavaScript console errors.

## Follow-up — 2026-06-17T21:26:15+07:00

Fix the critical water plane bug in the Anima Engine's 3D ecosystem map (`ecosystem.html`). Currently, large square translucent water planes (lakes) are generating on top of the new tall mountains and clipping mid-air.

Working directory: e:/project/Anima-Engine
Integrity mode: development

## Requirements

### R1. Fix Lake Placement Algorithm
The current algorithm (Pass 2) identifies mountain peaks and places physical water planes (lakes) on them. With the new tall mountains, this results in massive floating square water planes. The team must rewrite the lake placement logic so that large physical water planes only spawn in logical low-lying depressions or valleys, NEVER on steep mountain peaks.

### R2. Visual Cleanup
Ensure that any water plane that is generated perfectly blends with the terrain (no sharp square edges jutting out into the air). If physical water planes cannot be placed cleanly without clipping into the sky, consider removing them in favor of the existing terrain-colored ponds, or constrain them strictly to flat lowlands.

## Acceptance Criteria

### Map Quality
- [ ] No square water planes are visible floating in the air or jutting out of mountain peaks.
- [ ] Water features (lakes/ponds) only exist in logical valley depressions or lowlands.
- [ ] The tall snow-capped mountains remain intact without any water planes intersecting them at high altitudes.

### Verification
- [ ] The browser agent verifies visually that the mountain peaks are clear of hovering water planes.
- [ ] Map generation finishes without errors.


## Follow-up — 2026-06-17T15:02:00Z

Fix the hydrological system in the Anima Engine's 3D ecosystem map so that mountain streams and waterfalls follow the terrain naturally without visual artifacts (clipping, floating).

Working directory: `e:\project\Anima-Engine`
Integrity mode: development

## Requirements

### R1. Natural Stream Pathing & Rendering
The river and stream ribbon meshes must precisely hug the underlying terrain. They should not float above the ground or cut through hillsides. The current mathematical smoothing on the Y-axis of the `riverPts` array is too aggressive and must be replaced or adjusted to ensure the water surface accurately tracks the carved riverbed at a slight offset (e.g., +0.5).

### R2. Strict Waterfall Triggers
The waterfall mesh (`fallWaterMat`) is currently triggering on gentle slopes because the `drop` threshold is too sensitive or inaccurately measured. Adjust the logic so that waterfalls ONLY render on genuine, steep vertical cliffs (e.g., a massive elevation drop between adjacent tiles). For normal downhill slopes, use the regular stream mesh (`lakeWaterMat`).

## Acceptance Criteria

### Visual Accuracy
- [ ] Streams cleanly rest on the ground without geometric clipping (no straight white lines piercing through mountains).
- [ ] Waterfalls only appear on steep, cliff-like drops, not on regular hillsides.
- [ ] The water flow continuously connects from the mountain source down to the ocean/lake without awkward gaps.

## Follow-up — 2026-06-17T15:37:26Z

Upgrade the visual fidelity of the Anima Engine's 3D ecosystem map by implementing a high-quality custom GLSL shader for all water bodies (ocean, lakes, rivers, ponds) to achieve ultra-realistic effects like ripples, refraction, and foam, while maintaining the overall low-poly aesthetic.

Working directory: `e:\project\Anima-Engine`
Integrity mode: development

## Requirements

### R1. Custom GLSL Water Shader
Replace the standard `MeshPhongMaterial` used for water bodies (`oceanMat`, `lakeWaterMat`, `pondWaterMat`, `fallWaterMat`) with a custom `THREE.ShaderMaterial`. The shader must feature time-based dynamic ripples, wave normals, and simulate depth/refraction.

### R2. Edge Foam & Depth Blending
The shader must detect where the water geometry intersects with the terrain (using a depth texture or distance fields) to render realistic white foam edges and fade the water color based on depth.

### R3. Lighting Integration
The custom shader must react correctly to the existing dynamic Day/Night cycle (sunlight, moonlight, ambient lighting, and fog), ensuring it doesn't look out of place when the environment changes.

## Acceptance Criteria

### Technical & Visual Verification
- [ ] Water meshes in the scene are using a custom `ShaderMaterial` instead of a basic material.
- [ ] The water surface is animated over time, displaying dynamic ripples or waves.
- [ ] A visible foam line or distinct color fade is present where the water meets the terrain shores.
- [ ] The water's brightness and color correctly shift during the Day/Night cycle transition.
- [ ] The application continues to render smoothly without crashing (FPS > 30).

