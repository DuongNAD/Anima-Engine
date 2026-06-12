# ANIMA-ENGINE: SYSTEM CONTEXT & AI GUIDELINES

## 1. PROJECT OVERVIEW (Tổng Quan Dự Án)
- **Project Name:** Anima-Engine
- **Vision:** A real-time, GPU-accelerated Artificial Life (ALife - Đời sống nhân tạo) and Evolution Simulator (Trình mô phỏng tiến hóa).
- **Core Mechanism:** Co-evolution (Đồng tiến hóa) of physical body structures (Morphology - Hình thái học) via Directed Graphs (Đồ thị có hướng) and brain functions (Neural Control - Kiểm soát thần kinh) via Actor-Critic networks, driven by Homeostatic Reinforcement Learning (HRRL - Học tăng cường cân bằng nội môi).
- **Target Performance:** Simulate tens of thousands of physically articulated agents (Thực thể khớp nối vật lý) concurrently at 60 FPS on consumer hardware.
- **Target Hardware Specifications:**
  - CPU: Intel Core i5-14600KF (20 threads / 20 luồng)
  - GPU: NVIDIA RTX 5060 Ti (16GB VRAM)
  - RAM: 48GB
- **Target Audience:** Developers, Artificial Intelligence (AI) researchers, ALife enthusiasts, and educational platforms.

---

## 2. THE 4 SCIENTIFIC PILLARS (4 Trụ Cột Khoa Học)
To design the simulator correctly, the AI must align with these four academic and research pillars:

### 2.1. Morphological Evolution (Tiến Hóa Hình Thái)
- **Directed Graph Representation:** Genotype (Kiểu gen) is represented as a directed graph where nodes correspond to body segments (e.g., limbs, torso) and edges represent physical connections or joints (recursive joints - khớp đệ quy).
- **Phenotype Mapping:** The process of translating genotype graphs into a 3D physical structure (Phenotype - Kiểu hình) inside the physics engine.
- **Energy Cost Constraint:** Large or complex morphologies require more metabolic energy (Tiêu hao năng lượng) to move and sustain, creating an evolutionary trade-off (Sự đánh đổi tiến hóa).

### 2.2. Hybrid Neural Architecture (Kiến Trúc Thần Kinh Lai)
- **Central Pattern Generators (CPGs):** Embedded in the lower-level motor control (spinal cord - tủy sống) to generate rhythmic muscle contractions (co thắt cơ nhịp nhàng) without requiring high-level brain commands.
- **Lightweight Actor-Critic Networks:** The high-level brain (cerebral cortex - vỏ não) that processes sensory inputs (e.g., vision, smell) and outputs targets/parameters for the CPGs.
- **Learning & Adaptation:** Combined instinct (hardwired CPGs) and plasticity (hành vi học hỏi thông qua mạng nơ-ron).

### 2.3. Homeostatic Reinforcement Learning (HRRL)
- **Homeostasis (Cân bằng nội môi):** Physical agents must maintain internal physiological variables (biến sinh lý nội tại) like energy (năng lượng), hydration (nước), and temperature within viable bounds (ngưỡng sinh tồn).
- **Intrinsic Reward (Phần thưởng nội tại):** Instead of simple task-based rewards, the reward function is the minimization of homeostatic deviation (giảm thiểu độ lệch nội môi). The agent is rewarded for staying "healthy".
- **Survival Drive (Động lực sinh tồn):** Drives emergent behaviors (hành vi đột phát) such as foraging (tìm kiếm thức ăn), resting, or seeking shelter.

### 2.4. Diversity Incubator (MAP-Elites)
- **Illumination Algorithm:** Use Multi-dimensional Archive of Phenotypic Elites (MAP-Elites) to search for a diverse set of high-performing designs rather than a single optimal solution.
- **Feature Space / Niche Spaces:** Divide the population into bins based on phenotypic traits (e.g., height, locomotion speed, limb count).
- **Stepping Stones (Bước đệm tiến hóa):** Preserving weird or unique mutations that might not be globally optimal yet, but serve as evolutionary paths to future adaptations.

---

## 3. TECH STACK & ENVIRONMENT (Công Nghệ & Môi Trường)
- **Core Engine (Logic & Physics):**
  - **Language:** Rust (for thread safety, zero-cost abstractions, and memory safety without a garbage collector).
  - **Libraries:** Bevy ECS (Entity Component System) or Flecs bindings for parallel execution.
- **Frontend / UI:**
  - **Framework:** Tauri v2 (Rust-based web view wrapper) + TypeScript + React/Vite.
  - **Rendering:** WebGPU or WebGL (using Three.js) for high-performance 3D visualization.
- **Machine Learning (AI Lõi):**
  - **Framework:** Rust bindings for PyTorch (`tch-rs`) or `Burn` framework.
  - **Execution:** Batched Tensor Operations directly on the GPU to utilize the 16GB VRAM and avoid host-to-device transfer latency (Độ trễ truyền dữ liệu CPU-GPU).
- **Data Storage:**
  - **In-Memory:** Contiguous data structures managed inside the ECS.
  - **Phylogenetic Lineage Tracking:** Neo4j (Graph Database) to record parent-child relationships and evolutionary lineages (Cây phả hệ tiến hóa).
- **Infrastructure:**
  - Local workstation deployment.
  - Tauri-safe Inter-Process Communication (IPC) utilizing serialized message passing (Shared Memory / ring buffers where possible to avoid JSON serialization bottlenecks).

---

## 4. ARCHITECTURE & DESIGN PATTERNS (Kiến Trúc & Mẫu Thiết Kế)

### 4.1. Data-Oriented Design (DOD)
- **DOD over OOP:** Absolutely avoid Deep Inheritance Hierarchies (Kế thừa nhiều tầng) and pointer chasing (truy vết con trỏ). Simulating 10,000+ agents requires cache-friendly structures.
- **Structure of Arrays (SoA):** Prefer SoA over Array of Structures (AoS).
  ```rust
  // AoS (Bad for CPU Cache Locality)
  struct Agent {
      position: Vec3,
      velocity: Vec3,
      health: f32,
  }
  
  // SoA (Good for ECS and SIMD Vectorization)
  struct AgentPositions(Vec<Vec3>);
  struct AgentVelocities(Vec<Vec3>);
  struct AgentHealths(Vec<f32>);
  ```
- **Contiguous Memory Arrays:** Keep agent components sequential in memory to minimize cache misses (Lỗi truy cập bộ nhớ đệm) and maximize SIMD (Single Instruction, Multiple Data) potential.

### 4.2. Batched Simulation Pipeline (Luồng Xử Lý Gộp)
- To prevent GPU synchronization bottlenecks (nghẽn cổ chai đồng bộ), do not execute neural networks per-agent.
- **Unified Batched Inference:**
  1. Collect inputs (Sensory data) from all active agents into a single Tensor.
  2. Send the Tensor to the GPU in one operation.
  3. Execute batch inference (Actor-Critic evaluation).
  4. Stream actions back to the ECS agents in parallel.
- **Physics Batching:** Rigid body dynamics and collision detection must run in parallel (e.g., spatial hashing - băm không gian) using `Rayon`.

### 4.3. IPC Communication Protocol
- Send only minimal spatial updates (e.g., position, orientation, agent ID) per frame to the Tauri frontend.
- Do not serialize complex physics geometries over IPC on every tick. The frontend should load the mesh once and only update transforms (Vị trí & Góc xoay).

---

## 5. DIRECTORY STRUCTURE (Cấu Trúc Thư Mục)
```
/anima-engine
├── src-tauri                  # Tauri backend (Rust Core)
│   ├── Cargo.toml             # Rust package configuration
│   └── src
│       ├── main.rs            # Entry point for Tauri application
│       ├── core               # ECS World initialization, main game loop (Tick)
│       │   ├── mod.rs
│       │   ├── engine.rs      # Heartbeat of the simulation
│       │   └── ecs.rs         # Components, Systems, Resource configurations
│       ├── physics            # Rigid-body dynamics, joint systems, collisions
│       │   ├── mod.rs
│       │   ├── spatial.rs     # Spatial hashing for fast collision pruning
│       │   └── dynamics.rs    # Constraint resolution for jointed bodies
│       ├── ai                 # Neural network, CPG, and HRRL logic
│       │   ├── mod.rs
│       │   ├── model.rs       # PyTorch/Burn neural networks
│       │   ├── cpg.rs         # Central Pattern Generators (CPGs)
│       │   └── hrrl.rs        # Reward formulation and state transitions
│       └── evolution          # Evolutionary operations
│           ├── mod.rs
│           ├── genotype.rs    # Directed Graph genome representation
│           ├── crossover.rs   # Recombination operators
│           ├── mutation.rs    # Mutation rate, structural changes
│           └── map_elites.rs  # Archive of phenotypic elites
├── src                        # Tauri frontend (React / TypeScript)
│   ├── package.json
│   ├── tsconfig.json
│   ├── src
│   │   ├── main.tsx
│   │   ├── App.tsx
│   │   ├── components         # UI controls, settings panels, telemetry dashboards
│   │   └── renderer           # WebGL/WebGPU Three.js Canvas setup
└── docs                       # System diagrams, equations, and API specs
```

---

## 6. CODING CONVENTIONS & BEST PRACTICES (Quy Chuẩn Lập Trình)

### 6.1. Zero-Allocation Hot Loops (Vòng Lặp Nóng Không Cấp Phát Bộ Nhớ)
- **Zero Allocations inside `Tick`:** Allocating heap memory (using `Vec::new()`, `Box::new()`, `format!()`) inside the tick loop is strictly prohibited.
- **Memory Pooling:** Pre-allocate all collections, matrices, and arrays during startup or system configuration. Use pooling libraries or pre-sized buffers (buffers được định kích thước sẵn) and clear/reuse them.
- **In-Place Mutation:** Mutate values in-place (thay đổi giá trị trực tiếp tại ô nhớ) rather than returning new instances.

### 6.2. High-Performance Concurrency (Đa Luồng Hiệu Năng Cao)
- **Rayon Integration:** Parallelize loops over ECS queries and data processing arrays.
- **Thread Safety:** Enforce strict Send/Sync implementations. Protect shared states via lock-free designs (Thiết kế không khóa) where possible, avoiding heavy Mutex locks inside the hot loop.

### 6.3. Error Handling & Robustness
- **No Panics:** Never use `.unwrap()` or `.expect()` in production paths. Always handle potential failures with `Option` or `Result` using combinators (`and_then`, `map_or`, `unwrap_or_default`) or the propagation operator (`?`).
- **Graceful Fallbacks:** If a sensory check or physical update fails, fall back to safe default states rather than crashing the system.

### 6.4. Security & Input Sanitization
- **IPC Safety:** Tauri IPC bindings must sanitize all external inputs to prevent malicious execution or stack overflows (tràn ngăn xếp).
- **Type Safety:** Leverage Rust's strong type system (Newtype pattern) to enforce unit verification (e.g., separating `Radians` from `Degrees`, `MetaboliteEnergy` from `RawCalories`).

---

## 7. AI-SPECIFIC DIRECTIVES (Chỉ Thị Ràng Buộc Riêng Cho AI)

### 7.1. Zero Tolerances for Placeholders
- **No placeholders:** Do not output code comments like `// implement later`, `// ...`, or `// Keep existing logic`.
- **Complete Blocks:** Always write the full implementation. If modifying a function, provide the entire function body containing the logic, imports, and variables.

### 7.2. Chain-of-Thought (CoT) Architecture Reasoning
- Before generating code for core modules (physics, neural inference, evolution), write a brief analysis of performance trade-offs:
  - Cache Line Utilization (Tối ưu hóa dòng cache)
  - SIMD Vectorization potential
  - Thread Contention risks (Tranh chấp luồng)
  - Host-to-Device Memory transfer overhead (Chi phí truyền bộ nhớ CPU-GPU)

### 7.3. Educational Communication
- Use clear Vietnamese explanations for architectural choices.
- Embed English terminology with clear Vietnamese context so that the developer masters industry terms (e.g., *Metabolic Rate*, *Joint Constraints*, *Latent Vector*, *Cache Locality*, *Spatial Hashing*).

### 7.4. Data-Oriented Design Guardrails
- Reject any code proposing classic OOP patterns (e.g., a base class `Agent` inherited by `Rabbit` and `Wolf`).
- Enforce the ECS paradigm where `Rabbit` and `Wolf` are represented by adding unique *Components* (e.g., `Predator`, `Prey`) to generic *Entities*.
