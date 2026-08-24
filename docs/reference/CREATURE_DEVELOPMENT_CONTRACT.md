---
title: Hợp đồng phát triển và sinh mới sinh vật
status: accepted
owner: simulation-architecture
last_reviewed: 2026-07-24
review_cycle: per-release
decision: ../decisions/ADR-0001-creature-development-lifecycle.md
---

# Hợp đồng phát triển và sinh mới sinh vật

Tài liệu này là nguồn chuẩn bắt buộc cho mọi thay đổi liên quan genotype, phenotype,
genesis, birth, evolutionary replacement, save/load, migration, pigment và hình học
sinh vật. Nếu kế hoạch, comment hoặc code khác tài liệu này, phải tạo ADR hoặc sửa
nguồn mâu thuẫn trước khi triển khai.

## Thứ tự đọc cho AI agent

1. Tài liệu này — những điều **MUST/MUST NOT**.
2. [ADR-0001](../decisions/ADR-0001-creature-development-lifecycle.md) — lý do quyết định.
3. [Giải thích morphogenesis](../explanation/CREATURE_MORPHOGENESIS.md) — mô hình khoa học.
4. [Kế hoạch triển khai](../planning/CREATURE_MORPHOGENESIS_PLAN.md) — task, dependency và gate.
5. Code hiện tại tại các symbol được liệt kê bên dưới. Không dựa vào số dòng cũ.

## Thuật ngữ chuẩn

| Thuật ngữ | Ý nghĩa |
|---|---|
| `MorphologyGenotype` | Dữ liệu di truyền: node/edge và các trait có thể di truyền |
| `EnvSample` | Ảnh chụp môi trường tại vị trí/thời điểm phát triển |
| `DevelopedPhenotype` | Hình học và trait đã phát triển từ genotype + môi trường sinh |
| `RuntimeState` | Position, velocity, homeostasis, CPG và trạng thái thay đổi mỗi tick |
| Genesis | Tạo quần thể ban đầu hoặc loài hoàn toàn mới |
| Birth | Sinh vật con mới, có tổ tiên và giao dịch năng lượng |
| Restore | Khôi phục cá thể đã tồn tại từ save |
| Migration | Chuyển cùng một cá thể sang shard/world partition khác |
| Evolutionary replacement | Cơ chế legacy thay cá thể ở cuối epoch; chưa phải reproduction M5 |

## Sự thật code hiện tại

`decode_genotype` hiện tạo cả phenotype, runtime state và ECS entities trong một hàm.
Nó được gọi ở bốn đường:

| Luồng | Symbol hiện tại | Ngữ nghĩa đúng sau refactor |
|---|---|---|
| Genesis | `SimulationEngine::start` trong `core/simulation_loop.rs` | Develop một lần rồi spawn |
| Epoch replacement | `SpawnGenotypeCommand::apply` | Develop cá thể thay thế; giao dịch EU riêng |
| Restore save | `spawn_serialized_agent` | Spawn phenotype đã lưu; không develop lại |
| Migration | `SpawnMigrationCommand::apply` | Spawn phenotype đã truyền; không develop lại |

`TerrainMap`, `ResourceField`, `WorldIdentity` và `MapBounds` đã được insert trong
`core::ecs::init_world` trước genesis. Seed runtime lấy từ `WorldIdentity.seed`;
`MapSettings` hiện không phải resource.

## Invariant bắt buộc

### D01 — Development chỉ xảy ra một lần

`develop_at_birth` chỉ được gọi cho `Genesis`, `Birth` hoặc một intervention được ghi
nhận rõ. `Restore` và `Migration` **MUST NOT** gọi development.

### D02 — Phenotype được lưu và di chuyển

`DevelopedPhenotype` phải có `version`, được serialize trong `SerializedAgent` và
`AgentMigrationData`. Restore/migration phải giữ nguyên mass, length, radius, anchor,
medium, color và các trait phenotype trong tolerance bit/exact đã công bố.

### D03 — Không áp plasticity hai lần

Một trait không được đồng thời nhân trong genesis prior và reaction norm nếu không có
công thức hợp thành được test. MVP: prior tác động lên **genotype distribution**;
reaction norm tác động đúng một lần lên **phenotype**.

### D04 — Habitat độc lập với ô đang đứng

`LocomotionMedium`/`HabitatPreference` là trait của sinh vật, không được suy ra lại từ
ô đích khi kiểm spawn. `is_habitat_legal(profile, env)` phải so hai dữ liệu độc lập.

### D05 — Hình học nhất quán

Khi development thay length/radius, phải cập nhật đồng bộ:

- `Segment`;
- `RigidBody`;
- `SpatialCollider`;
- child placement;
- `JointConstraint.anchor_offset`;
- edge/joint anchor đã clamp theo parent phát triển.

Không được chỉ đổi `Segment.length/radius` rồi giữ anchor genotype cũ.

### D06 — Giao dịch năng lượng rõ nguồn

- Genesis `t=0`: năng lượng quần thể là điều kiện biên; baseline closed-EU được chốt
  **sau** khi khởi tạo plants + animals + detritus.
- Birth M5: năng lượng con non lấy từ parent/resource transaction; áp S29.
- Evolutionary replacement legacy: năng lượng lấy từ cá thể bị thay hoặc một
  intervention có ledger event; không gọi đây là birth.
- Restore/migration: chuyển cùng reserve, không cộng thêm EU.

### D07 — Determinism

Mọi RNG thuộc morphogenesis nhận seed/stream tường minh từ `WorldIdentity.seed`,
tick/event counter và lineage key. Không gọi `thread_rng()` trong genesis,
development, mutation hoặc crossover được kiểm deterministic.

### D08 — Màu không đổi tức thì theo biome

`display_color` được phát triển/lưu tại birth. Di chuyển sang biome khác không tự đổi
màu, trừ khi có trait riêng như `DynamicChromatophore` và một cơ chế có chi phí.

### D09 — Tương thích dữ liệu cũ

Field mới dùng `#[serde(default)]` hoặc migration có version. Default phải giữ hành vi
cũ; ví dụ pigment cũ dùng `None` để renderer fallback theo class, không mặc định đen.

### D10 — Taxonomy và sampling

- Biome/classification dùng cell bucket.
- Elevation liên tục có thể bilinear.
- `EnvSample` ngoài biên trả `Result::Err`, không tạo một “ocean mặc định”.
- Backend hiện dùng 11-biome projection; không tuyên bố phân biệt đủ 22 biome.
- `ResourceField.r_max` gọi là `resource_capacity`, không đồng nhất với sinh khối hiện
  tại `r` hoặc NPP thời gian thực.

## API đích

Các chữ ký dưới đây là contract cho implementation. Có thể thay module path bằng ADR,
nhưng không được gộp development và spawn trở lại.

```rust
pub fn sample_environment(
    terrain: &TerrainMap,
    resource: Option<&ResourceField>,
    bounds: &MapBounds,
    position: Vec3,
) -> Result<EnvSample, EcomorphError>;

pub fn develop_at_birth(
    genotype: &MorphologyGenotype,
    environment: EnvSample,
    rng: &mut impl rand::Rng,
) -> Result<DevelopedPhenotype, EcomorphError>;

pub fn spawn_developed(
    world: &mut World,
    genotype: &MorphologyGenotype,
    phenotype: &DevelopedPhenotype,
    state: SpawnRuntimeState,
) -> Result<Entity, SpawnError>;

pub fn is_habitat_legal(
    habitat: &HabitatPreference,
    environment: &EnvSample,
) -> bool;
```

Data model tối thiểu:

```rust
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DevelopmentalProgram {
    pub mass_temp_slope: f32,
    pub appendage_temp_slope: f32,
    pub pigment_moisture_slope: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HabitatPreference {
    pub medium: LocomotionMedium,
    pub elevation_range: [f32; 2],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DevelopedPhenotype {
    pub version: u16,
    pub birth_environment: EnvironmentSnapshot,
    pub nodes: Vec<DevelopedNode>,
    pub edges: Vec<DevelopedEdge>,
    pub total_mass: f32,
    pub display_color: Option<[f32; 3]>,
}
```

`DevelopmentalProgram::default()` phải có slope bằng `0.0` để save cũ không tự đổi
hình. Genesis mới có thể lấy mẫu slope nhỏ, có bound và có mutation/crossover.
`HabitatPreference::default()` dùng terrestrial + elevation range rộng để giữ hành vi
legacy; pigment cũ là `None`, không phải RGB đen.

## Ma trận luồng spawn

| Luồng | Lấy environment | Develop? | Phenotype source | Năng lượng |
|---|---:|---:|---|---|
| Genesis | Có | Có | Tạo mới | Điều kiện biên `t=0` |
| Birth | Có | Có | Tạo từ genotype con | Debit transaction |
| Evolutionary replacement | Có | Có | Tạo mới | Transfer/intervention, không phải birth |
| Restore | Không bắt buộc | Không | Save | Giữ nguyên |
| Migration | Có thể để validate | Không | Payload | Giữ nguyên |
| User inject | Có | Có hoặc payload | Command khai báo | Intervention event |

## Test gate của feature

| Gate | Bằng chứng |
|---|---|
| **CM-S01** | `sample_environment` đúng cell, field và lỗi ngoài biên |
| **CM-S02** | Cùng seed + genotype + env tạo phenotype giống nhau |
| **CM-S03** | Restore giữ phenotype và runtime state |
| **CM-S04** | Migration giữ phenotype, dù environment đích khác |
| **CM-S05** | Development chạy đúng một lần; không double Bergmann |
| **CM-S06** | Segment/collider/anchor/child placement nhất quán |
| **CM-S07** | Habitat trait độc lập; đạt S27 trên fixture water/land/alpine |
| **CM-S08** | Genesis/replacement/birth/restore/migration giữ closed EU |
| **CM-S09** | Mutation/crossover của trait mới có bound; ánh xạ S41 |
| **CM-S10** | Rust tick payload và TypeScript type/render parity |
| **CM-S11** | Reciprocal-transplant chứng minh local adaptation |

S43 vẫn là Red-Queen predator–prey theo
[`WORLD_SIMULATION_PLAN.md`](../../WORLD_SIMULATION_PLAN.md); không dùng CM-S11 thay S43.

## Cổng hoàn tất

Không tuyên bố feature hoàn tất nếu thiếu bất kỳ mục nào:

- CM-S01…CM-S10 pass; CM-S11 cần trước khi gọi trait là “thích nghi”.
- Save cũ load bằng default/migration đã test.
- Restore và migration không đổi phenotype.
- Closed-EU không nhảy tại mọi loại spawn.
- Benchmark spawn/tick và binary/IPC delta đã ghi.
- Canonical spawn view đã qua Animal Map Vision, không còn finding critical/high.
