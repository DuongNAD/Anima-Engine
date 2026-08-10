# Original User Request

## Initial Request — 2026-06-12T15:32:32Z

Upgrade the raw UI dashboard of the Anima-Engine project into a polished, high-fidelity HUD (Heads-Up Display) with a retro/sci-fi game style (deep navy/metallic panels, glowing teal/gold terminal-style typography, styled PixiJS grid, and a clean layout).

Working directory: `/Users/duongnad/Documents/project/Anima-Engine`
Integrity mode: development

## Requirements

### R1. Sci-Fi Retro Game HUD (CSS & HTML)
- Replace all light/gray panel styles in App.tsx and index.css with a dark theme.
  - Set main background to deep navy/charcoal (`#0d1117` or `#0b0f19`).
  - Use custom card panel colors like dark metallic gray (`#161b22`) with subtle border patterns (e.g. 1px solid teal `#00f0ff` or gold `#ffd700`) and glowing box-shadows.
- Apply standard system monospace fonts (e.g., `Courier New, Consolas, Monaco, SFMono-Regular, monospace`) globally for a terminal-like HUD look.
- Re-organize the simulation status values at the top of the screen: group "Độ trễ TB của Tick" and "Backend FPS" into a thin, retro status bar with glowing text/numbers.

### R2. Immersive PixiJS Grid & Canvas Rendering
- Refactor the canvas in PixiViewport.tsx:
  - Procedurally draw a dark dungeon tile grid background (dark background with tile borders) rather than a solid light gray background (`0xf7fafc`).
  - Make gridlines thin and use a glowing teal color with fade in/out effects.
  - Highlight the target region (the highlighted box at the bottom right) using an intense blue glow effect with thick borders and 0.8 opacity (slightly transparent).
- Update the colors of existing rendering elements in the canvas (raycast beams, pheromones, agent segments) to match the dark sci-fi HUD theme.

### R3. Maintain Functionality & Unit Tests
- Do not break existing features: Tauri IPC state listening (`simulation-tick`, `map-elites-update`, etc.), simulation start/stop toggle, MAP-Elites parameter sliders, chronicle events, or port migration.
- Ensure all vitest unit tests in App.test.tsx continue to pass. Run `npm run test` to verify.

## Acceptance Criteria

### Visual Aesthetics
- [ ] Entire dashboard UI background is dark (deep navy/gray/black theme) instead of light gray/white.
- [ ] Text elements are styled with a monospace font family.
- [ ] Canvas displays a stylized dark dungeon grid with thin teal lines instead of a flat solid background.
- [ ] High-contrast highlighted blue zone in the canvas has a glow effect and opacity.
- [ ] Simulation statistics are grouped in a dedicated, retro-styled horizontal status bar.

### Code Integrity & Verification
- [ ] The app builds cleanly with no compile errors (`npm run build`).
- [ ] Running `npm run test` executes successfully and all 3 unit tests pass.

## Follow-up — 2026-06-12T16:27:59Z

Redesign the Anima-Engine dashboard into a modern, sleek, and highly readable tabbed interface, while upgrading the 2D PixiJS simulation canvas to render detailed, procedurally-animated top-down (god-eye) organisms resembling a 2D game.

Working directory: `/Users/duongnad/Documents/project/Anima-Engine`
Integrity mode: development

## Requirements

### R1. Minimal & Clean Tabbed Dashboard (CSS & HTML)
- Replace the CRT scanner/flicker theme in index.css and App.tsx with a clean, high-readability modern dark theme.
  - Remove all CRT scanlines, screen flickering, and radial vignette filters.
  - Use default clean sans-serif/monospace typography (system fonts or a clean font like Inter).
  - Use sleek, dark glassmorphic panels (semi-transparent dark gray backgrounds, thin borders, subtle cyan/blue glows).
- Re-arrange the dashboard layout using a clean tabbed navigation layout:
  - **Tab 1: World View & Status (Mô phỏng Thế giới)**: Contains the main simulation controls, projection selection, the main PixiJS viewport canvas, simulation stats, and the active agents telemetry list.
  - **Tab 2: Evolution & Archives (Tiến hóa & Phả hệ)**: Contains the MAP-Elites evolutionary archive grid, evolution parameter controls, the Genotype Lineage Graph SVG, and the Mother Mother Nature Chronicle event timeline.
  - **Tab 3: System Configurations (Cấu hình)**: Contains the distributed socket migration inputs and triggers, plus other advanced debug panels.

### R2. High-Fidelity 2D Game Simulation Canvas
- Refactor the canvas rendering in PixiViewport.tsx to resemble a polished top-down 2D game:
  - **Organisms/Agents**: Procedurally draw detailed creatures instead of flat circles or triangles.
    - **Predators**: Render a multi-segment body (e.g. sharp diamond head, tiny glowing red eyes, and a dynamically swaying tail or appendages that bend according to movement yaw).
    - **Prey**: Render a smooth multi-segment circular body with glowing blue eyes and a swimming tail.
    - **Segment Connections**: Render connections between agent segments as a tapering body (joints/bones getting smaller towards the tail) rather than simple gray lines.
  - **Pheromones**: Draw pheromones as soft glowing energy clouds/smoke (e.g., using radial gradient fills or smooth blending) instead of hard circles.
  - **Sensor Beams**: Render active raycasts as faint, high-tech radar scanning lines that light up when hitting food or other entities.

### R3. Maintain Functionality & Unit Tests
- Do not break existing features: Tauri IPC state listening (`simulation-tick`, `map-elites-update`, etc.), simulation start/stop toggle, MAP-Elites parameter sliders, chronicle events, or port migration.
- Ensure all vitest unit tests in App.test.tsx continue to pass. Run `npm run test` to verify.

## Acceptance Criteria

### Visual Aesthetics & Layout
- [ ] No CRT screen flicker, scanlines, or vignette filters are present on the dashboard.
- [ ] Dashboard is split into three clean tabs: "World View & Status", "Evolution & Archives", and "System Configurations".
- [ ] Main background is a clean dark theme, and cards use flat or glassmorphic dark gray borders without noisy CRT styling.
- [ ] 2D simulation canvas renders detailed organisms with segment joints, eyes, and tails rather than raw triangles/circles.
- [ ] Pheromones are displayed as smooth glowing fields or radial gradient fills.

### Code Integrity & Verification
- [ ] The app builds cleanly with no compile errors (`npm run build`).
- [ ] Running `npm run test` executes successfully and all unit tests pass.

## Follow-up — 2026-06-12T16:48:00Z

An ecosystem and terrain simulation module for Anima-Engine, integrating environmental dynamics (mountains, hills, seas, lakes, clouds, light-dark cycles) with biological entities (flora and fauna) to study ecological chains and social interactions.

Working directory: /Users/duongnad/Documents/project/Anima-Engine
Integrity mode: development

## Requirements

### R1. Procedural Terrain & Dynamic Vegetation
- The simulation must generate a procedural 2D terrain grid consisting of various biomes (plains, hills, mountains, sea, lakes, ponds).
- A dynamic vegetation (flora) system must be implemented where plants sprout, grow by consuming light and moisture, and wither if starved or overgrazed.

### R2. Dynamic Environmental Physics & Cycles
- Implement a day-night cycle that updates the global light levels, and moving clouds (weather) that cast moving shadow areas.
- Implement basic water dynamics (diffusion of water across cells/terrain) that creates moisture zones for vegetation.

### R3. Ecological Food Chain & Sensory Integration
- Physical agents (fauna) must interact with the terrain: movement speed must scale with terrain elevation and water depth (e.g. swimming in water, slower climbing on hills/mountains).
- Herbivorous agents (prey) must be able to eat plants to restore metabolic energy, creating a complete primary food chain.
- Agent sensory systems (raycast/olfactory) must be attenuated by light levels (darkness and cloud shadows).

### R4. God-Mode Controls & High-Fidelity Rendering Layer
- Provide frontend controls (sliders/toggles) in the dashboard to control environmental factors (e.g., speed up/pause day-night cycles, trigger storms/disasters, adjust terrain fertility).
- Update the PixiJS viewport to render the detailed terrain grid, water moisture levels, moving cloud sprites, and a dynamic ambient light overlay (day-night shadow mask).

## Acceptance Criteria

### Backend & Simulation Logic
- [ ] Terrain generation system initializes a grid of biomes and heights at startup.
- [ ] Day-night cycle advances time and cycles ambient light levels dynamically.
- [ ] Plants grow over time based on local moisture and light levels, and decay when resources are depleted.
- [ ] Herbivores detect and consume nearby plants, transferring energy to their metabolic store.
- [ ] Agent speed is reduced when traversing rugged terrain (hills, mountains) or deep water compared to plains.
- [ ] Raycast sensor outputs are scaled down in low-light or shadowed regions.

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

### Frontend HUD & Rendering (Local Environment Mutators)
- [ ] The dashboard includes an "Environment & God-Mode" section with controls for time speed, weather toggles, and mutators.
- [ ] The PixiJS viewport visualizes terrain tiles (color-coded by biome: green plains, yellow hills, brown mountains, blue water).
- [ ] Cloud layers and day-night lighting overlays are rendered on top of the terrain with smooth alpha transparency.
- [ ] The UI builds cleanly with `npm run build` and backend compile passes with `cargo check`.
- [ ] All existing and new unit/integration tests run successfully.

## Follow-up — 2026-06-13T11:19:42Z

Xây dựng và tối ưu hóa mô hình chú thỏ 3D low-poly phong cách papercraft (giấy gấp) trong Three.js/React Three Fiber. Mô hình mới cần có tỷ lệ cơ thể giống thỏ thật nhất ở tư thế ngồi (giống ảnh mẫu), sử dụng các hình học cơ bản (low-poly, flat shaded) lắp ghép thủ công, có viền đen sắc nét trên mọi góc cạnh, nhưng vẫn giữ nguyên các chuyển động hopping/chewing và đạt hiệu năng tối ưu nhất (vòng lặp vẽ mượt mà, dung lượng cực nhẹ và không rò rỉ bộ nhớ).

Working directory: /Users/duongnad/Documents/project/Anima-Engine
Integrity mode: benchmark

## Requirements

### R1. Low-Poly Papercraft Rabbit Morphology
Cập nhật mô hình chú thỏ trong file `playground/RabbitVisualizer.tsx` để có hình dáng giống chú thỏ ngồi trong ảnh mẫu. Mô hình phải được dựng bằng cách lắp ghép các hình học cơ bản có số lượng mặt ít (low-poly, flat shading, không dùng smooth gradients) như body, head, ears, legs, tail, cheeks, eyes, nose.

### R2. Sharp Outlines
Mọi cạnh sắc của các bộ phận trên chú thỏ phải được vẽ viền đen (outlines) rõ ràng và sắc nét, tạo cảm giác như mô hình giấy gấp. Sử dụng các kỹ thuật vẽ viền tối ưu (ví dụ: `THREE.EdgesGeometry` kết hợp `THREE.LineSegments`) để giảm tải cho GPU.

### R3. Hopping and Chewing Animations
Chú thỏ phải duy trì các chuyển động hoạt ảnh sinh động (nhảy - hopping, thở - breathing, nhai - chewing) bằng cách thay độ tọa độ, góc xoay hoặc tỷ lệ của các bộ phận trong hàm cập nhật khung hình (`useFrame`) dựa trên thời gian thực.

### R4. Performance Optimization
Mô hình phải cực kỳ nhẹ và có hiệu năng tốt nhất:
- Không được tải các tệp tin mô hình 3D bên ngoài (.gltf, .obj, .glb).
- Tái sử dụng tối đa các đối tượng hình học (Geometries) và chất liệu (Materials) để tránh tạo mới liên tục trên mỗi khung hình gây rác bộ nhớ (GC thrashing).
- Không có lỗi ngoại lệ Javascript (exceptions) hay cảnh báo rò rỉ bộ nhớ trong Console.

## Acceptance Criteria

### Aesthetics & Morphology
- [ ] Chú thỏ có tỷ lệ giống thỏ thật ở tư thế ngồi với đầy đủ các bộ phận: tai dài dựng đứng, má hồng hồng, mũi, mắt đen, đùi sau gập lại, chân trước chống xuống đất.
- [ ] Toàn bộ các bộ phận của chú thỏ đều hiển thị viền đen rõ nét bao quanh các cạnh của đa giác.
- [ ] Chú thỏ thực hiện chuyển động nhảy (hopping) nhịp nhàng lên xuống và chuyển động nhai (chewing) ở miệng khi ăn.

### Technical & Quality
- [ ] Ứng dụng không tải bất kỳ tệp mô hình 3D (.gltf, .obj) nào từ bên ngoài.
- [ ] `npm run test` chạy thành công và vượt qua toàn bộ các bài kiểm thử frontend.
- [ ] `npm run build` hoàn thành biên dịch thành công mà không có lỗi TypeScript hay linter.

## Follow-up — 2026-06-13T11:44:18Z

Xây dựng và tối ưu hóa mô hình chú thỏ 3D low-poly phong cách papercraft (giấy gấp) trong Three.js/React Three Fiber và Rust Tauri. Mô hình mới cần có tỷ lệ cơ thể giống thỏ thật nhất ở tư thế ngồi (giống ảnh mẫu), sử dụng các hình học cơ bản (low-poly, flat shaded) lắp ghép thủ công, có viền đen sắc nét trên mọi góc cạnh, sửa lỗi tỷ lệ trông giống "quỷ" thành một chú thỏ dễ thương, đồng thời mở rộng khoảng zoom của camera để hỗ trợ thu nhỏ nhiều hơn.

Working directory: /Users/duongnad/Documents/project/Anima-Engine
Integrity mode: benchmark

## Requirements

### R1. Low-Poly Papercraft Rabbit Morphology
Cập nhật mô hình chú thỏ trong cả frontend (`playground/RabbitVisualizer.tsx`) và backend Rust (`src-tauri/src/lib.rs`) để có hình dáng giống chú thỏ ngồi trong ảnh mẫu.
- Mô hình phải được dựng bằng cách lắp ghép các hình học cơ bản có số lượng mặt ít (low-poly, flat shading).
- Sửa đổi tỷ lệ và vị trí các bộ phận (body, head, ears, legs, tail, muzzle/cheeks) để thỏ trông cân đối, dễ thương, không bị dị dạng hay giống "quỷ".
- Tai dài dựng đứng (hướng lên trên/hơi V-shape), đùi sau gập sát hông ở tư thế ngồi, chân trước chống thẳng xuống đất, đầu cân đối, mõm và má không bị nhô ra một cách kỳ dị.

### R2. Expand Zoom Range
Mở rộng phạm vi của thanh trượt Zoom của camera trong `playground/RabbitVisualizer.tsx` để hỗ trợ thu nhỏ hơn (zoom range từ `1.0` đến `15.0` thay vì `2.5` đến `8.0`).

### R3. Sharp Outlines
Mọi cạnh sắc của các bộ phận trên chú thỏ phải được vẽ viền đen (outlines) rõ ràng và sắc nét, tạo cảm giác như mô hình giấy gấp. Sử dụng các kỹ thuật vẽ viền tối ưu (ví dụ: `THREE.EdgesGeometry` kết hợp `THREE.LineSegments`) để giảm tải cho GPU.

### R4. Hopping and Chewing Animations
Chú thỏ phải duy trì các chuyển động hoạt ảnh sinh động (nhảy - hopping, thở - breathing, nhai - chewing) bằng cách thay đổi tọa độ, góc xoay hoặc tỷ lệ của các bộ phận trong hàm cập nhật khung hình dựa trên thời gian thực. Cần đảm bảo các chuyển động không làm biến dạng mô hình một cách kỳ quái.

### R5. Performance Optimization
Mô hình phải cực kỳ nhẹ và có hiệu năng tốt nhất:
- Không được tải các tệp tin mô hình 3D bên ngoài (.gltf, .obj, .glb).
- Tái sử dụng tối đa các đối tượng hình học (Geometries) và chất liệu (Materials) để tránh tạo mới liên tục trên mỗi khung hình gây rác bộ nhớ (GC thrashing).
- Không có lỗi ngoại lệ Javascript (exceptions) hay cảnh báo rò rỉ bộ nhớ trong Console.

## Acceptance Criteria

### Aesthetics & Morphology
- [ ] Chú thỏ có tỷ lệ giống thỏ thật ở tư thế ngồi với đầy đủ các bộ phận: tai dài dựng đứng, má hồng hồng, mũi, mắt đen, đùi sau gập lại, chân trước chống xuống đất.
- [ ] Mõm/miệng của chú thỏ trông tự nhiên và dễ thương, không có các khối màu hồng kỳ dị nhô ra làm thỏ giống "quỷ".
- [ ] Toàn bộ các bộ phận của chú thỏ đều hiển thị viền đen rõ nét bao quanh các cạnh của đa giác.
- [ ] Chú thỏ thực hiện chuyển động nhảy (hopping) nhịp nhàng lên xuống và chuyển động nhai (chewing) ở miệng khi ăn.
- [ ] Thanh trượt Zoom hoạt động mượt mà với dải giá trị mở rộng từ `1.0` (phóng to rất gần) đến `15.0` (thu nhỏ ra rất xa).

### Technical & Quality
- [ ] Ứng dụng không tải bất kỳ tệp mô hình 3D (.gltf, .obj) nào từ bên ngoài.
- [ ] `npm run test` chạy thành công và vượt qua toàn bộ các bài kiểm thử frontend.
- [ ] `npm run build` hoàn thành biên dịch thành công mà không có lỗi TypeScript hay linter.

## Follow-up — 2026-06-13T18:55:48+07:00

Tái cấu trúc và tối ưu hóa tỷ lệ, vị trí các bộ phận của mô hình chú thỏ 3D low-poly phong cách papercraft (giấy gấp) trong Three.js/React Three Fiber và Rust Tauri để chú thỏ trông cân đối, dễ thương và ngồi vững vàng trên bục (pedestal) thay vì bị chôn một nửa dưới đất hay có tỷ lệ dị dạng.

Working directory: /Users/duongnad/Documents/project/Anima-Engine
Integrity mode: benchmark

## Requirements

### R1. Low-Poly Papercraft Rabbit Morphology & Proportions
Cập nhật mô hình chú thỏ trong cả frontend (`playground/RabbitVisualizer.tsx`) và backend Rust (`src-tauri/src/lib.rs`) để có hình dáng giống chú thỏ ngồi trong ảnh mẫu.
1. **Nâng độ cao cơ thể (curY)**: Thay đổi gốc tọa độ Y của thỏ từ `-0.5` thành `0.4` (trong hàm `generateSingleRabbitJS` và `generate_dynamic_rabbit`) để thỏ ngồi ngay ngắn trên mặt bục pedestal thay vì bị chôn sâu dưới đất.
2. **Tỷ lệ các bộ phận (Scales)**:
   - Thân (Body): `(1.6, 1.8, 1.4)` (không dùng tỷ lệ cũ `2.0, 3.2, 2.2` quá to và cao).
   - Đầu (Head): `(1.0, 1.0, 1.0)`.
   - Tai (Ears): `(1.8, 0.35, 0.2)`.
   - Chân trước (Front Legs): `(0.9, 0.3, 0.3)`.
   - Chân sau (Hind Legs): `(1.2, 0.7, 0.6)`.
   - Đuôi (Tail): `(0.4, 0.4, 0.4)`.
   - Miệng (Mouth): `(0.2, 0.15, 0.2)`.
3. **Vị trí tương đối của các bộ phận (Positions)**:
   - Đầu: đặt tại `localToWorld(0.5, 1.1, 0.0)`.
   - Tai Trái: đặt tại `localToWorld(0.2, 1.6, 0.3)`, tai xoay chữ V (`rx: 0.25`, `rz: curRot + 1.57 + earBreathing`).
   - Tai Phải: đặt tại `localToWorld(0.2, 1.6, -0.3)`, tai xoay chữ V (`rx: -0.25`, `rz: curRot + 1.57 - earBreathing`).
   - Chân trước Trái: đặt tại `localToWorld(0.7, 0.2, 0.35)`, xoay thẳng đứng xuống đất (`rz: curRot - 1.57`).
   - Chân trước Phải: đặt tại `localToWorld(0.7, 0.2, -0.35)`, xoay thẳng đứng xuống đất (`rz: curRot - 1.57`).
   - Chân sau Trái: đặt tại `localToWorld(-0.4, 0.15, 0.55)`, xoay gập hông (`rz: curRot + 0.2`).
   - Chân sau Phải: đặt tại `localToWorld(-0.4, 0.15, -0.55)`, xoay gập hông (`rz: curRot + 0.2`).
   - Đuôi: đặt tại `localToWorld(-0.8, 0.3, 0.0)`.
   - Miệng: đặt tại `localToWorld(0.85, 0.95, 0.0)`, màu trắng sữa `(0.95, 0.95, 0.95)`.

### R2. Keep Animations, Outlines & Zoom Range
- Giữ nguyên các hoạt ảnh hopping (nhảy nhịp nhàng), breathing (thở nhẹ), chewing (nhai ở miệng) dựa trên thời gian thực.
- Giữ nguyên dải zoom mở rộng từ `1.0` đến `15.0`.
- Giữ nguyên các viền đen sắc nét trên mọi cạnh đa giác bằng `THREE.EdgesGeometry`.

### R3. Performance Optimization
Mô hình phải cực kỳ nhẹ và có hiệu năng tốt nhất:
- Không được tải các tệp tin mô hình 3D bên ngoài (.gltf, .obj, .glb).
- Tái sử dụng tối đa các đối tượng hình học và chất liệu.
- Không có lỗi ngoại lệ Javascript hay rò rỉ bộ nhớ trong Console.

## Acceptance Criteria

### Aesthetics & Morphology
- [ ] Chú thỏ có tỷ lệ thỏ ngồi cân đối, dễ thương, không bị dị dạng.
- [ ] Chú thỏ ngồi ngay ngắn bên trên mặt bục pedestal, các chân chạm mặt bục tự nhiên, không bị chôn nửa thân dưới bục.
- [ ] Tai dài hướng lên trên hình chữ V cân đối. Mõm nhỏ xinh màu trắng ăn khớp với đầu.
- [ ] Chú thỏ thực hiện các hoạt ảnh hopping, breathing, chewing bình thường.

### Technical & Quality
- [ ] Ứng dụng không tải bất kỳ tệp mô hình 3D (.gltf, .obj) nào từ bên ngoài.
- [ ] `npm run test` và `npm run test:frontend` chạy thành công 100%.
- [ ] `npm run build` hoàn thành biên dịch thành công mà không có lỗi TypeScript hay linter.

## Follow-up — 2026-06-13T19:23:11+07:00

Delete the current rabbit model from the project, including both frontend visualizers/sandboxes and backend Tauri commands.

Working directory: /Users/duongnad/Documents/project/Anima-Engine
Integrity mode: development

## Requirements

### R1. Remove frontend rabbit visualizer components
- Delete the `playground/RabbitVisualizer.tsx` component file.
- Delete the entire `rabbit-standalone` directory.
- Remove the lazy import, the toggle state, and the Toggle button for "Rabbit Test" from the main application view in `src/App.tsx`.

### R2. Remove backend rabbit-related commands and functions
- Remove the `AdvancedRabbitPart` struct, the `generate_dynamic_rabbit` function, and the `get_test_rabbit_state` command from `src-tauri/src/lib.rs`.
- Remove the command registration for `get_test_rabbit_state` from the Tauri builder in `src-tauri/src/lib.rs`.

### R3. Remove obsolete tests
- Delete the frontend test files `tests/frontend/rabbit_visualizer_boundaries.test.tsx` and `tests/frontend/rabbit_visualizer_stress.test.tsx` that are testing the rabbit visualizer.

## Acceptance Criteria

### Project Integrity and Compilation
- [ ] No references to the deleted rabbit-related modules/components exist in any codebase file.
- [ ] Running `npm run cargo:check` or `cargo check` compiles successfully without errors or warnings regarding rabbit code.
- [ ] Running `npm run build` (tsc && vite build) succeeds without any TypeScript or bundler errors.

### Test Suites
- [ ] Running `npm run test` or `npm run test:frontend` in the project root runs successfully without trying to execute the deleted rabbit tests.
- [ ] The remaining test suites pass.

## Follow-up — 2026-06-13T19:40:12+07:00

Create an isolated 'playground' directory containing harnesses and scripts to test, debug, and experiment with individual modules of the Anima-Engine (such as Bevy ECS, Burn neural networks, Tauri IPC, and PixiJS frontend) in isolation.

Working directory: /Users/duongnad/Documents/project/Anima-Engine
Integrity mode: development

## Requirements

### R1. Rust Playground Sub-Project
- Create a new binary Cargo package under `playground/rust` (or as a separate crate in the workspace).
- Configure its `Cargo.toml` to depend on `anima_engine_lib` (using relative path `../../src-tauri` or similar).
- Provide a basic entry point (`main.rs`) that imports Bevy ECS components or Burn models from the engine to verify integration.

### R2. Frontend Playground Template
- Create a frontend playground under `playground/frontend` (e.g., as a simple standalone page or a sub-workspace in the project).
- Include a template using PixiJS to render mock organisms, joints, and pheromone clouds, allowing isolated visual experimentation.
- Ensure the dev server runs correctly and dependencies are properly configured.

### R3. Documentation
- Create a `playground/README.md` explaining how to build and run the Rust playground (`cargo run --package ...` or similar) and the frontend playground.

## Acceptance Criteria

### Rust Compilation
- [ ] Running `cargo check` or `cargo build` in `playground/rust` succeeds without compiling errors.
- [ ] Running the Rust binary output successfully prints a confirmation message and shows it can import modules from `anima_engine_lib`.

### Frontend Setup & Rendering
- [ ] The frontend playground serves/runs without compilation or bundle errors.
- [ ] A template page loads in the browser and displays a PixiJS canvas rendering mock organism components, segment joints, or pheromone grids.

### Documentation
- [ ] A `playground/README.md` is present and outlines step-by-step setup and execution instructions.

## Follow-up — 2026-06-13T12:54:45Z

Create an organism library containing preset configuration files (JSON) for various organism types. Implement a React UI tab to browse these presets and allow the user to select and spawn them into the active Bevy simulation world via Tauri IPC.

Working directory: /Users/duongnad/Documents/project/Anima-Engine
Integrity mode: development

## Requirements

### R1. Organism Preset Store
- Create five default preset JSON files or configurations representing:
  - **Default Prey**: Standard prey with average segment count and energy.
  - **Default Predator**: Standard predator with sensors and average speed.
  - **Giant Prey**: Heavy organism with a large number of body segments.
  - **Speedy Predator**: Highly agile predator with increased movement parameters.
  - **Long Snake**: Extremely long segmented body to test joint stability.
- The presets should define segment count, behavior/agent type, initial energy, and basic physical/CPG coefficients.

### R2. Tauri IPC Backend Integration
- Implement a Tauri command `get_organism_presets` that returns the list of available presets and their metadata (name, description, segment count, behavior type, energy).
- Implement a Tauri command `spawn_preset_organism` that parses the preset configuration and spawns the corresponding entities (as Bevy ECS segments and controllers) at a random or center coordinate in the simulation.
- Register these commands in the Tauri builder inside `lib.rs` / `main.rs`.

### R3. Organism Library Tab in React UI
- Add a 4th tab named **"Thư viện Sinh vật" (Organism Library)** to the navigation.
- Render cards or a list for all 5 presets. For each preset, show its name, type (Prey/Predator), segment count, description, and initial energy.
- Add a **"Spawn"** button on each card. Clicking it calls `spawn_preset_organism` via Tauri IPC to spawn the selected organism.

## Acceptance Criteria

### Rust Code & Tauri IPC
- [ ] Backend compiles successfully (`cargo check` in `src-tauri` passes).
- [ ] The `get_organism_presets` and `spawn_preset_organism` commands are registered and functional.

### UI Tab & Spawning
- [ ] The 4th tab "Thư viện Sinh vật" is fully visible and styled consistently with the modern glassmorphic theme.
- [ ] Clicking "Spawn" on a preset card successfully communicates with Tauri IPC and spawns the organism into the Bevy simulation without errors or crashes.
- [ ] Frontend builds successfully without typescript or linter warnings (`npm run build` passes).

## Follow-up — 2026-06-16T00:48:06Z

An interactive, high-fidelity fantasy world map application styled after a hand-drawn cartography illustration (inspired by the reference image). Built using PixiJS and TypeScript as an isolated playground module inside the Anima Engine project for future integration.

Working directory: `/Users/duongnad/Documents/project/Anima-Engine/playground/fantasy-map`
Integrity mode: development

## Requirements

### R1. Render Engine & Layout (PixiJS)
- Initialize a PixiJS Application rendering inside an HTML container.
- Renders a vintage parchment texture as the map background.
- Draw or render the 8 distinct geographic regions arranged as shown in the reference image:
  - **Jungle & Waterfall** (West): Dense green canopy with vertical flowing waterfalls.
  - **Desert & Oasis** (South-West): Sandy dunes with a central palm tree oasis.
  - **Volcano** (South-Center): Dark volcanic rock with a lava-erupting cone.
  - **River Delta** (South-East): Distributary network of green wetlands and channels.
  - **World Tree** (Center): A massive magical tree on a central lake island.
  - **Dark Temple/Ruins** (Center-North): A stepped stone ziggurat.
  - **Pine Forest & Castles** (North): Dark green pine forest containing stone towers.
  - **Snowy Mountains** (North-East): Snow-capped jagged peaks.
  - **Snowy Floating Island** (Far North-East): An isolated island floating in the sky/sea.

### R2. Navigation (Zoom & Pan)
- Implement zoom (mouse wheel) and pan (click-and-drag) mechanics.
- Support smooth deceleration/damping for map panning.
- Constrain zoom and pan boundaries to prevent dragging the map off-screen.

### R3. Ambient Visual Effects
Implement unique animations for each region to bring the map to life:
- **Jungle**: Animated waterfall cascades (using tile scrolling or particle spray) and floating jungle mist.
- **Volcano**: Rising dark smoke particle system and a pulsing lava glow.
- **World Tree**: Magical glowing spores/particles floating around the tree, and subtle light rays.
- **Snowy Areas**: Gentle falling snow particles over the mountains and floating island.
- **Floating Island**: Subtle vertical bobbing (floating up/down) animation.
- **Water & Delta**: Water shimmer/ripple effects.

### R4. Sailing Ships & Sea Monster
- **Ships**: Animated sailing ships (min 2) that travel along path lines across the ocean.
- **Monster**: An animated kraken/octopus tentacle in the eastern sea that occasionally breaches the surface.

### R5. Interface Overlay & UI
- **Parchment Border & Grid**: A classical map grid overlay and border.
- **Decorative Elements**: Scroll, telescope, and crest illustration in the bottom-left corner.
- **Interactivity**: Clicking on a region opens a stylized fantasy UI pop-up box detailing the lore/information of that region.

---

## Acceptance Criteria

### Visuals & Styling
- [ ] Renders all 8 distinct geographic regions with art assets or custom Canvas/PixiJS drawings that resemble the color palette and composition of the reference image.
- [ ] Vintage grid overlay and parchment texture are layered correctly.
- [ ] Bottom-left scroll overlay is rendered.

### Navigation & Damping
- [ ] Drag-to-pan works with smooth momentum deceleration (damping).
- [ ] Zoom-in and zoom-out work smoothly centered on the mouse cursor position.

### Animations
- [ ] Particle systems are active (volcano smoke, falling snow, magical spores).
- [ ] Waterfall is animated (scrolling texture or particles).
- [ ] Sailing ships move along predefined bezier curves.
- [ ] Floating island bobs up and down using a sine wave function.

### Interaction
- [ ] Clicking on a region triggers a styled parchment dialog box with information about that region.
- [ ] Dialog can be closed, resuming map interaction.

### Build & Integration
- [ ] Integrates into the main project's Vite config as a new entry point: `playground/fantasy-map/index.html`.
- [ ] Standard command `npm run dev` builds and serves the map at `http://localhost:5173/playground/fantasy-map/`.
- [ ] No typescript or console errors on load.
