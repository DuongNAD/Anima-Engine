---
title: Bản nháp Claude — Kế hoạch Creature Morphogenesis
status: superseded
archive_state: do-not-use
owner: architecture
last_reviewed: 2026-07-24
superseded_by: ../planning/CREATURE_MORPHOGENESIS_PLAN.md
---

# CREATURE_MORPHOGENESIS_PLAN.md — Kế hoạch triển khai đầy đủ

> **Không triển khai theo bản này.** Bản nháp này áp plasticity vào cả restore và
> migration, không lưu phenotype độc lập, dùng S43 sai nghĩa và đánh giá thiếu luồng
> năng lượng của evolutionary replacement. Bản thay thế:
> [`docs/planning/CREATURE_MORPHOGENESIS_PLAN.md`](../planning/CREATURE_MORPHOGENESIS_PLAN.md).

> Bản kế hoạch thi công (implementation plan) cho hệ **tạo hình sinh vật theo môi trường**. Đây là companion của tài liệu thiết kế [`CREATURE_MORPHOGENESIS.md`](CREATURE_MORPHOGENESIS.md) — tài liệu kia giải thích *tại sao* (sinh học + kiến trúc 3 tầng), tài liệu này nói *làm gì, ở đâu, theo thứ tự nào, kiểm bằng gì*.
>
> Ánh xạ milestone: hiện thực M5 (vòng đời/sinh lý) và M7 (hành vi/sinh sản/tiến hóa gắn môi trường) trong [`WORLD_SIMULATION_PLAN.md`](WORLD_SIMULATION_PLAN.md). Mọi `file:line` dưới đây đã đối chiếu với code thật.

---

## 0. Nguyên tắc điều hướng (đọc trước khi code)

1. **Không magic effects.** Môi trường tác động qua *cơ chế trung gian* (mass→metabolism→sống/chết), không set thẳng chỉ số cuối như `is_adapted = true` hay `+survival`.
2. **3 tầng, tách bạch điểm nối:**
   - *Tầng 2 — Plasticity* (không di truyền): áp trong `decode_genotype` → chạm **mọi** lối sinh.
   - *Tầng 3 — Genesis prior* (chỉ gieo de-novo): thay khối genotype cứng ở `simulation_loop.rs`.
   - *Tầng 1 — Selection* (đã có phần lớn): siết + chứng minh ở Phase D.
3. **Tôn trọng hợp đồng cứng** (xem [`SIMULATION_RULES.md`](SIMULATION_RULES.md)): năng lượng đóng EU (S01), tách `body_temperature`°C ↔ `field_temperature`[0,1], nước/dưỡng chất là DeferredM3, cell-bucket khi hỏi biome, taxonomy 22↔11 (S02).
4. **Máy yếu:** không chạy full Tauri/Bevy backend. Kiểm bằng **headless `cargo test`** với world nhỏ + seed cố định; kiểm hình bằng Vitest + SwiftShader. Sau mỗi phase chạy skill `verify-anima`.
5. **Determinism:** mọi ngẫu nhiên seed từ `MapSettings.seed` (mặc định 1337) để benchmark tái lập; **không** dùng nguồn nhàm ngẫu nhiên không seed.

---

## 1. Xương sống kiến trúc: module `ecomorph`

Tạo module mới **`src-tauri/src/evolution/ecomorph.rs`** (đăng ký trong `evolution/mod.rs`). Đây là nơi tập trung *toàn bộ* logic env→phenotype để dễ test đơn vị và tinh chỉnh hằng số (giống cách `ecology.rs` gom hằng số MTE).

### 1.1. Lấy mẫu môi trường (dùng chung mọi phase)

```rust
// evolution/ecomorph.rs  (SKETCH — chưa phải bản cuối)
use crate::core::terrain::{BiomeType, TerrainMap};
use crate::core::ecology::{biome_from_u8, ResourceField, biome_carrying_capacity};
use crate::core::resources::MapBounds;
use glam::Vec3;

/// Ảnh chụp môi trường tại một điểm — đầu vào thuần cho mọi quy tắc ecomorph.
#[derive(Clone, Copy, Debug)]
pub struct EnvSample {
    pub valid: bool,        // false nếu pos ngoài biên lưới
    pub biome: BiomeType,
    pub field_temp: f32,    // [0,1]  (KHÔNG phải °C)
    pub moisture: f32,      // [0,1]
    pub elevation: f32,     // [0,1]  (bilinear)
    pub flow: f32,          // [0,1]
    pub npp_capacity: f32,  // sức chứa/ô  (biome_npp · NPP_TO_CAPACITY), ~[0.5 .. 22]
}

/// Cell-bucket cho trường phân loại (biome/temp/moisture/flow); bilinear cho elevation.
pub fn sample_environment(
    terrain: &TerrainMap,
    resource: Option<&ResourceField>,
    bounds: &MapBounds,
    pos: Vec3,
) -> EnvSample {
    match terrain.get_map_indices(pos, bounds) {          // terrain.rs:563 — cell-bucket, None ngoài biên
        None => EnvSample { valid: false, biome: BiomeType::Ocean, field_temp: 0.5,
                            moisture: 0.5, elevation: 0.0, flow: 0.0, npp_capacity: 1.0 },
        Some((col, row)) => {
            let i = row * terrain.width + col;
            let biome = biome_from_u8(terrain.biomes[i]);  // ecology.rs:404
            let npp = resource
                .and_then(|r| r.cell_index(pos.x, pos.z))  // ecology.rs:363
                .map(|ci| resource.unwrap().r_max[ci])
                .unwrap_or_else(|| biome_carrying_capacity(biome));
            EnvSample {
                valid: true, biome,
                field_temp: terrain.temperatures[i],
                moisture: terrain.moistures[i],
                elevation: terrain.get_elevation_at_pos(pos, bounds), // terrain.rs:579 — bilinear
                flow: terrain.flows[i],
                npp_capacity: npp,
            }
        }
    }
}
```

### 1.2. Hằng số ecomorph (tập trung, tunable)

```rust
pub const K_BERGMANN: f32       = 0.6;   // cực lạnh → mass ×(1+0.6)
pub const ALLEN_MIN_SLENDER: f32 = 0.5;  // lạnh: chi ngắn-dày
pub const ALLEN_MAX_SLENDER: f32 = 2.0;  // nóng: chi dài-mảnh
pub const TEMP_C_MIN: f32       = 30.0;  // khớp domain body_temperature [30,45]
pub const TEMP_C_MAX: f32       = 45.0;
pub const HYDR_LOSS_MIN: f32    = 0.6;   // sa mạc giữ nước tốt
pub const HYDR_LOSS_MAX: f32    = 1.4;
pub const RAINFOREST_CAP: f32   = 22.0;  // = biome_npp(Rainforest)·0.01, để chuẩn hóa NPP→[0,1]
```

---

## 2. Phase A — Developmental plasticity (Tầng 2)

> **Mục tiêu:** môi trường định hình *kích thước, tỉ lệ, ngưỡng nhiệt, giữ nước* của **mọi** cá thể tại thời điểm decode — **không cần gene mới, không đổi IPC.** Rẻ nhất, rủi ro thấp nhất, mở đường cho Phase C.

### 2.1. Norm of reaction (hàm thuần, dễ test)

```rust
// evolution/ecomorph.rs
pub struct Plasticity {
    pub mass_mult: f32,          // nhân vào node.mass
    pub slenderness: f32,        // length/radius của node ngoại vi
    pub temp_target_c: f32,      // °C — thay 37.0 cứng
    pub hydration_loss_mult: f32,
}

pub fn reaction_norm(env: &EnvSample) -> Plasticity {
    let t = env.field_temp.clamp(0.0, 1.0);
    Plasticity {
        mass_mult: 1.0 + K_BERGMANN * (1.0 - t),                       // Bergmann
        slenderness: ALLEN_MIN_SLENDER + (ALLEN_MAX_SLENDER - ALLEN_MIN_SLENDER) * t, // Allen
        temp_target_c: (TEMP_C_MIN + (TEMP_C_MAX - TEMP_C_MIN) * t)    // thermal niche → °C
            .clamp(TEMP_C_MIN, TEMP_C_MAX),
        hydration_loss_mult: HYDR_LOSS_MIN
            + (HYDR_LOSS_MAX - HYDR_LOSS_MIN) * env.moisture.clamp(0.0, 1.0),
    }
}
```

### 2.2. Nối vào `decode_genotype` ([`genotype.rs:50`](src-tauri/src/evolution/genotype.rs))

`decode_genotype` đã cầm `&mut World` → tự đọc resource, **không đổi chữ ký** (giữ nguyên 3 call-site).

1. Đầu hàm: đọc `TerrainMap` + `MapBounds` (+ `ResourceField`) từ `world`, gọi `sample_environment(initial_pos)` → `EnvSample`, rồi `reaction_norm` → `Plasticity`. **Trích các scalar ra biến cục bộ và thả borrow trước khi `world.spawn`** (tránh xung đột mượn resource khi spawn).
2. Root `HomeostaticState` (hiện `genotype.rs:99-107`): đổi `temperature`/`temp_target` từ `37.0` → `plasticity.temp_target_c`.
3. Mọi `RigidBody.mass` và `Segment.mass`: `node.mass * plasticity.mass_mult`.
4. Node ngoại vi (leaf/child): điều biến `length`/`radius` theo `slenderness` **giữ thể tích ~không đổi** (không phá cân bằng mass). Clamp trong `length 0.1..5`, `radius 0.05..1` (khớp bound mutation).
5. `hydration_loss_mult` không có chỗ trong `HomeostaticState` → thêm **component mới không-di-truyền**:

```rust
// core/components.rs
#[derive(Component, Clone, Copy, Debug)]
pub struct EnvAdaptation { pub hydration_loss_mult: f32 }   // phenotype, không nằm trong genotype
```
gắn lên root trong `decode_genotype`, và **đọc trong `metabolic_decay_system`** ([`world_systems.rs`](src-tauri/src/core/world_systems.rs) ~64-143) khi trừ hydration: `d_hydration *= env_adapt.map(|e| e.hydration_loss_mult).unwrap_or(1.0)`.

> ⚠️ Vì plasticity nằm ở decode chứ trong genotype nên **không di truyền** — đúng bản chất. Con của cùng cha mẹ sinh ở 2 biome sẽ khác nhau, nhưng genome vẫn y hệt.

### 2.3. Test (headless)

| Test | Nội dung | Gate |
|---|---|---|
| `reaction_norm_monotonic` | lạnh (t→0) ⇒ `mass_mult` cao & `temp_target_c` thấp; khô (moisture→0) ⇒ `hydration_loss_mult` thấp | đơn điệu |
| `temp_target_in_domain` | ∀ `field_temp∈[0,1]` ⇒ `temp_target_c∈[30,45]` | property |
| `sample_environment_cellbucket` | pos ở tâm ô X ⇒ đúng biome ô X; pos ngoài biên ⇒ `valid=false` | tái dùng **S03** |
| `decode_env_differs` | decode cùng genotype ở ô lạnh vs ô nóng ⇒ mass & temp_target khác nhau, deterministic | acceptance |

**Rủi ro:** chi phí chuyển hóa đổi → động lực quần thể dịch. → chạy lại benchmark, cập nhật [`BENCHMARK_BASELINE.md`](BENCHMARK_BASELINE.md) nếu cần.
**Rollback:** một cờ `ecomorph_plasticity_enabled` (mặc định on) để tắt nhanh khi so sánh.

---

## 3. Phase B — Genesis prior + archetype topology (Tầng 3, đạt S27)

> **Mục tiêu:** hạt giống de-novo có **kiểu cơ thể + kích thước + guild** hợp môi trường ngay từ đầu, và **không bao giờ spawn phi pháp** (loài cạn giữa hồ / loài nước trên núi).

### 3.1. Locomotion medium & tính hợp lệ spawn

```rust
// evolution/ecomorph.rs
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LocomotionMedium { Aquatic, Terrestrial, Alpine, Arboreal }

pub fn medium_for(env: &EnvSample) -> LocomotionMedium {
    use BiomeType::*;
    match env.biome {
        DeepOcean | Ocean | River => LocomotionMedium::Aquatic,
        MountainRock | Snow if env.elevation > 0.78 => LocomotionMedium::Alpine,
        Rainforest | TemperateForest | BorealForest => LocomotionMedium::Arboreal,
        _ => LocomotionMedium::Terrestrial,
    }
}

/// S27: cá thể có medium M có được phép ở ô env không?
pub fn is_spawn_legal(medium: LocomotionMedium, env: &EnvSample) -> bool {
    let aquatic_cell = matches!(env.biome, BiomeType::DeepOcean | BiomeType::Ocean | BiomeType::River);
    match medium {
        LocomotionMedium::Aquatic => aquatic_cell,
        _ => env.valid && !aquatic_cell,     // loài cạn không xuống nước sâu
    }
}
```

### 3.2. Bộ sinh genotype theo môi trường

```rust
// evolution/ecomorph.rs
use rand::Rng;

/// Sinh genotype khởi đầu theo môi trường + guild. Là PHÂN BỐ (có nhiễu), không tất định.
pub fn genesis_generator(env: &EnvSample, rng: &mut impl Rng) -> (MorphologyGenotype, AgentClass) {
    let medium = medium_for(env);
    // ngân sách khối lượng theo NPP (rừng mưa > sa mạc), có nhiễu
    let npp01 = (env.npp_capacity / RAINFOREST_CAP).clamp(0.05, 1.0);
    let mass_budget = (0.8 + 3.5 * npp01) * rng.gen_range(0.85..1.15);
    // Bergmann áp luôn vào prior
    let mass_budget = mass_budget * (1.0 + K_BERGMANN * (1.0 - env.field_temp));

    let geno = match medium {
        LocomotionMedium::Aquatic  => build_fusiform(mass_budget, env, rng),   // chuỗi thuôn 1 trục
        LocomotionMedium::Alpine   => build_compact(mass_budget, env, rng),    // ít node, thấp, chắc
        LocomotionMedium::Arboreal => build_limbed(mass_budget, env, rng, 4),  // nhiều chi
        LocomotionMedium::Terrestrial => build_cursorial(mass_budget, env, rng), // chân dài chạy
    };
    // guild: NPP đủ cao mới cho phép predator (cần đủ sinh khối prey)
    let class = if npp01 > 0.5 && rng.gen_bool(0.3) { AgentClass::Predator } else { AgentClass::Prey };
    (geno, class)
}
```
`build_*` là các factory topology (mỗi cái tạo `MorphologyNode`/`MorphologyEdge` với `joint_axis` phù hợp cách vận động: aquatic uốn ngang trục thân, cursorial chi dài…). Tất cả giữ trong clamp mutation để tương thích tiến hóa về sau, và cap `≤15 node` như `mutation.rs`.

### 3.3. Thay khối genesis cứng ([`simulation_loop.rs:757-817`](src-tauri/src/core/simulation_loop.rs))

1. **Dời** việc lấy `terrain_map` + `bounds` (hiện ở dòng 823) **lên trước** vòng `for i in 0..10`.
2. Với mỗi cá thể: chọn `spawn_pos` trên **ô hợp lệ** — lấy mẫu ứng viên rồi lọc bằng `is_spawn_legal`, tương tự cách chọn ứng viên hồ/cây ở `simulation_loop.rs:834+`. Nếu không có terrain (đường không-artifact), fallback về hành vi cũ (thẳng hàng) để không vỡ test hiện có.
3. `let env = sample_environment(&terrain, resource, &bounds, spawn_pos);`
4. `let (genotype, class) = genesis_generator(&env, &mut rng);` (rng = `StdRng::seed_from_u64(map_seed ^ i)`).
5. `decode_genotype` (đã env-aware từ Phase A) + gắn `AgentGenotype/Evaluation/Lineage/Generation` + `class`.

### 3.4. Ràng buộc năng lượng đóng (S01) tại genesis

Hiện mỗi con khởi tạo `energy = 100` "từ hư không". Quyết định thiết kế (ghi rõ trong code comment):
- **Genesis = điều kiện biên:** năng lượng của quần thể khởi đầu là *phần vốn ban đầu* của sổ `EcosystemBiomass.animals` — hợp lệ vì đây là t=0, không phải "sinh từ hư không" giữa chừng. **Khởi tạo ledger `animals` = Σ energy** để tổng nhất quán ngay từ đầu (S01 kiểm từ t=0).
- **Sinh sản (không phải genesis)** thì bắt buộc rút từ dự trữ cha mẹ — đường này **đã** trả xác về `detritus` ([`simulation_loop.rs:375`](src-tauri/src/core/simulation_loop.rs)); bổ sung: con non *trừ* năng lượng khỏi `energy` cha mẹ khi sinh, không cấp mới.

### 3.5. Test

| Test | Nội dung | Gate |
|---|---|---|
| `s27_spawn_legality` | ∀ N pos ngẫu nhiên: aquatic chỉ ở ô nước; terrestrial/alpine không ở nước sâu | **S27** |
| `genesis_mass_scales_npp` | `total_mass` rừng mưa > thảo nguyên > sa mạc (đơn điệu theo `npp_capacity`) | property |
| `genesis_deterministic` | cùng seed ⇒ cùng bộ genotype | reproducibility |
| `genesis_ledger_consistent` | Σ energy khởi đầu == `EcosystemBiomass.animals` | **S01** |

**Rủi ro:** trung bình — cell-bucket phải đúng, ledger phải khởi đúng. **Manifest `spawn` view** ([`MAP_MANIFEST.md`](MAP_MANIFEST.md)) là gate trực quan cuối.

---

## 4. Phase C — Sắc tố + ngoại hình qua IPC + renderer 3D

> **Mục tiêu:** phần "ngoại hình" đúng nghĩa bạn hỏi — **màu sắc** theo biome/Gloger/ngụy trang, và **hình dạng** phản ánh sang render. Đắt nhất vì xuyên backend↔IPC↔frontend. Chia **C1 (rẻ) → C2 (đắt)**.

### 4.1. Gene sắc tố (heritable, để crypsis có thể tiến hóa)

```rust
// evolution/genotype.rs — thêm field cấp genotype (không per-node)
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Default)]
pub struct PigmentGene { pub r: f32, pub g: f32, pub b: f32 }   // [0,1]³

pub struct MorphologyGenotype {
    pub nodes: Vec<MorphologyNode>,
    pub edges: Vec<MorphologyEdge>,
    pub pigment: PigmentGene,                                   // MỚI
}
```
- Cập nhật `mutation.rs`: thêm operator jitter `pigment` có bound (**S41**); `crossover.rs`: blend/average pigment 2 cha mẹ.
- **Serialization:** `#[serde(default)]` cho `pigment` để save cũ vẫn load (world_artifact/simulation_state).

### 4.2. Màu hiển thị = gene ⊗ môi trường

```rust
pub fn displayed_color(gene: PigmentGene, env: &EnvSample) -> [f32; 3] {
    let biome_rgb = biome_base_rgb(env.biome);          // bảng khớp BIOME_RGB frontend
    let crypsis = 0.5;                                   // kéo về nền biome (ngụy trang)
    let gloger = 0.3 * env.moisture                      // ẩm → đậm
        + 0.3 * (env.elevation * (1.0 - env.field_temp)); // núi-lạnh-nắng → melanism
    // lerp gene→biome theo crypsis, rồi tối màu theo gloger
    lerp3(gene_to_rgb(gene), biome_rgb, crypsis).map(|c| c * (1.0 - gloger))
}
```

### 4.3. C1 — Nối màu vào render 2D hiện có (rẻ, làm trước)

- **IPC:** mở rộng `SegmentState` ([`types/index.ts:1`](src/types/index.ts)) thêm `color?: [number, number, number]`, và **thêm `radius?`, `length?`** (hiện SegmentState **không có** kích thước đoạn — cần cho C2). Cập nhật struct serialize phía Rust nơi phát `simulation-tick` (grep `SegmentState`/tick emitter trong `commands/`), và **PROJECT.md** "Interface Contracts".
- Frontend `PixiViewport.tsx` (~526): tô tam giác/tròn agent bằng `color` thay vì màu cứng theo `agent_type`. Đây là bằng chứng thị giác đầu tiên (sa mạc→cát, tuyết→nhạt).
- **Đồng bộ mock alias** `three`/`@react-three/fiber` ở 3 nơi (`vite.config.ts`, `tsconfig.json`, `tests/vitest.config.ts`) nếu chạm import — xem CLAUDE.md "Gotchas".

### 4.4. C2 — Renderer 3D cho agent sống (lift lớn)

Hiện agent sống **chỉ** có Pixi 2D + cây DOM text; không có mesh 3D. Thêm một component R3F dựng **instanced mesh** từ `RenderSegment` (đã có `AgentHierarchy` ở [`App.tsx:131`](src/App.tsx)) dùng `radius`/`length`/`color` mới. Có thể mượn pattern lắp primitive của [`WorldWildlife.tsx`](src/components/Landscape/WorldWildlife.tsx) (nhưng driven bằng data, không hardcode).

### 4.5. Test

| Test | Nội dung | Gate |
|---|---|---|
| `pigment_mutation_bounded` | jitter pigment giữ trong `[0,1]`, biên độ có bound | **S41** |
| `gloger_monotonic` | moisture↑ ⇒ màu đậm hơn | property |
| `serde_pigment_default` | load save cũ (không pigment) không lỗi | backward-compat |
| Vitest `segment_color_render` | SegmentState có `color` → Pixi tô đúng | frontend |

**Rủi ro:** cao — đổi hợp đồng IPC (đồng bộ PROJECT.md + mock alias + vitest). **Làm C1 trước, C2 sau.**

---

## 5. Phase D — Siết chọn lọc & chứng minh thích nghi thật (Tầng 1, S43)

> **Mục tiêu:** chứng minh trait hội tụ theo biome là **thích nghi thật** (emergent), không chỉ do prior áp đặt.

- **Body-temp bám field-temp cục bộ:** để lệch nhiệt *thật sự* tốn năng lượng qua `metabolic_rate` (Arrhenius, `E_ANIMAL_EV=0.65`, [`ecology.rs:42`](src-tauri/src/core/ecology.rs)) → tạo áp lực chọn lọc lên `temp_target`.
- **Sensor đọc môi trường (M7.1):** thêm biome/elevation/water/shade vào input vector của brain ([`ai/model.rs`](src-tauri/src/ai/model.rs), hiện 15-dim raycast+homeostasis) → hành vi mới "biết" tìm bóng râm/tránh nước.
- **S43 harness:** ensemble nhiều seed, world nhỏ, epoch ngắn (headless). Đo: phân bố `mass`/`temp_target`/`pigment` theo biome có **phân kỳ và tương quan** với môi trường; survival/reproduction cải thiện so với nhóm chứng (tắt plasticity/prior). Tận dụng `niche_divergence`, `shannon_index`, MAP-Elites coverage đã có ([`ecology.rs`](src-tauri/src/core/ecology.rs)).

| Test | Gate |
|---|---|
| `s43_biome_trait_divergence` | trait/biome phân kỳ qua ≥5 seed | **S43** |
| `thermal_mismatch_costs` | temp_target lệch field_temp ⇒ chi phí năng lượng cao hơn | mechanism |

**Rủi ro:** trung bình — sim dài; máy yếu ⇒ giữ world nhỏ + short-horizon, hoặc chạy nơi khác.

---

## 6. Thứ tự, phụ thuộc & phạm vi

```
ecomorph.rs (EnvSample + hằng số)   ← nền, làm cùng Phase A
   │
   ├─ Phase A  Plasticity        (rẻ,  không đổi IPC/genome)      ← BẮT ĐẦU Ở ĐÂY
   ├─ Phase B  Genesis + S27     (vừa, chạm simulation_loop)      ← cần A (decode env-aware)
   ├─ Phase C  Màu + render      (đắt, xuyên tầng)  C1→C2         ← cần A (env), độc lập B
   └─ Phase D  Selection + S43   (vừa, sim dài)                   ← cần A+B (+C tùy chọn)
```

**Khuyến nghị:** làm **A → B** trước (đạt ~80% hiệu ứng "hợp môi trường" với rủi ro thấp, không đụng IPC), rồi **C1** (bằng chứng thị giác rẻ), sau đó **D**, cuối cùng **C2** (render 3D) khi cần đẹp cho explore mode.

| Phase | Đổi genome? | Đổi IPC? | File chạm chính | Rủi ro |
|---|---|---|---|---|
| A | Không | Không | `ecomorph.rs`(new), `genotype.rs`, `components.rs`, `world_systems.rs` | Thấp |
| B | Không | Không | `ecomorph.rs`, `simulation_loop.rs` | Vừa |
| C | **Có** (`pigment`) | **Có** (`SegmentState`) | `genotype.rs`, `mutation.rs`, `crossover.rs`, `types/index.ts`, `PixiViewport.tsx`, `PROJECT.md` | Cao |
| D | Không | Có thể (sensor) | `model.rs`, `world_systems.rs`, test harness | Vừa |

---

## 7. Checklist thi công

**Nền + Phase A**
- [ ] Tạo `src-tauri/src/evolution/ecomorph.rs`; khai báo trong `evolution/mod.rs`
- [ ] `EnvSample` + `sample_environment` (+ unit test cell-bucket, tái dùng S03)
- [ ] Hằng số ecomorph + `reaction_norm` (+ test đơn điệu & domain [30,45])
- [ ] Component `EnvAdaptation` trong `components.rs`
- [ ] Nối vào `decode_genotype` (mass_mult, slenderness, temp_target; thả borrow trước spawn)
- [ ] Đọc `hydration_loss_mult` trong `metabolic_decay_system`
- [ ] Cờ `ecomorph_plasticity_enabled`; chạy `verify-anima`; cập nhật benchmark nếu lệch

**Phase B**
- [ ] `LocomotionMedium` + `medium_for` + `is_spawn_legal`
- [ ] `genesis_generator` + `build_fusiform/compact/limbed/cursorial`
- [ ] Sửa `simulation_loop.rs`: dời terrain fetch lên trước; đặt spawn hợp lệ; seed RNG từ map seed
- [ ] Khởi tạo `EcosystemBiomass.animals` = Σ energy (S01 từ t=0)
- [ ] Test S27, mass↔NPP, determinism, ledger; kiểm manifest `spawn` view

**Phase C**
- [ ] `PigmentGene` vào `MorphologyGenotype` (+ `#[serde(default)]`)
- [ ] Operator mutation + crossover cho pigment (S41)
- [ ] `displayed_color` (Gloger + crypsis) + bảng `biome_base_rgb`
- [ ] C1: mở rộng `SegmentState` (`color`,`radius`,`length`) + Rust tick emitter + `PROJECT.md`
- [ ] C1: tô màu Pixi; đồng bộ mock alias 3 nơi; Vitest màu
- [ ] C2: renderer 3D instanced cho agent sống

**Phase D**
- [ ] Body-temp bám field-temp cục bộ (mechanism cost)
- [ ] Sensor thêm biome/elevation (M7.1)
- [ ] S43 ensemble harness (world nhỏ, nhiều seed); đo phân kỳ trait/biome

---

## 8. Quyết định còn mở (chọn khi bắt tay)

1. **Độ mạnh của genesis prior (Tầng 3):** khuyến nghị *yếu* (nhiễu rộng) để không lấn Tầng 1. Núm điều chỉnh: biên độ `rng.gen_range` trong `genesis_generator`.
2. **`pigment` heritable hay phenotype-only:** khuyến nghị heritable (crypsis tiến hóa được) + điều biến plastic — nhưng nếu muốn tối giản Phase C, làm phenotype-only trước (bỏ mutation/crossover pigment).
3. **Guild predator/prey thành gene hay vẫn suy từ môi trường:** kế hoạch để suy-từ-môi-trường ở B; nâng thành `Diet` gene ở M5 khi làm vòng đời.
4. **Ngân sách năng lượng genesis:** chốt "điều kiện biên t=0" (khởi ledger) thay vì debit detritus — cần bạn xác nhận khi hiện thực S01.

---

## 9. Tham chiếu chéo
- Thiết kế & lý do: [`CREATURE_MORPHOGENESIS.md`](CREATURE_MORPHOGENESIS.md) (bảng ánh xạ §5, kiến trúc 3 tầng §4)
- Ý định gốc & milestone: [`WORLD_SIMULATION_PLAN.md`](WORLD_SIMULATION_PLAN.md) (M5, M7, §2.2, §5.5)
- Hợp đồng cứng: [`SIMULATION_RULES.md`](SIMULATION_RULES.md), [`COORDINATE_CONTRACT.md`](COORDINATE_CONTRACT.md), [`BIOME_TAXONOMY.md`](BIOME_TAXONOMY.md), [`MAP_MANIFEST.md`](MAP_MANIFEST.md)
- Điểm nối code: `decode_genotype` [`genotype.rs:50`](src-tauri/src/evolution/genotype.rs) · genesis [`simulation_loop.rs:757`](src-tauri/src/core/simulation_loop.rs) · `SpawnGenotypeCommand` [`agent_systems.rs:73`](src-tauri/src/core/agent_systems.rs) · `sample_environment` nguồn [`terrain.rs:563`](src-tauri/src/core/terrain.rs), [`ecology.rs:363`](src-tauri/src/core/ecology.rs)
