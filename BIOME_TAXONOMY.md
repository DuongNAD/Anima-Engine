# BIOME_TAXONOMY — hợp đồng phân loại quần xã (M0.2)

> Tài liệu này là bản **human-readable companion** cho hai bản đồ biome đã được
> hiện thực + kiểm thử trong [`src-tauri/src/core/world_artifact.rs`](src-tauri/src/core/world_artifact.rs).
> Nó KHÔNG định nghĩa dữ liệu mới — nó chỉ mô tả lại đúng cái mà code đã chốt, để con
> người tra cứu mà không phải đọc Rust.

## 1. Mục đích

Anima-Engine tồn tại **hai không gian biome tách biệt**:

- **CANONICAL (22 biomes)** — enum `Biome` trong
  [`src/components/Landscape/utils/worldGen.ts`](src/components/Landscape/utils/worldGen.ts).
  Đây là taxonomy giàu, khí-hậu-thực-tế mà frontend worldgen sinh ra và render. Nó là
  **single source of truth** cho tên/emoji/RGB.
- **LEGACY (11 biomes)** — enum `BiomeType` trong
  [`src-tauri/src/core/terrain.rs`](src-tauri/src/core/terrain.rs). Đây là taxonomy cũ, thô
  mà backend `TerrainMap` dùng để seed NPP `ResourceField` cho hệ sinh thái.

Hai bản đồ nối chúng — `map_biome_backend_to_frontend` (legacy→canonical, chiều **lift**)
và `map_biome_frontend_to_backend` (canonical→legacy, chiều **downsample** khi runtime nạp
World Artifact vào sim) — sống trong
[`src-tauri/src/core/world_artifact.rs`](src-tauri/src/core/world_artifact.rs) và được test
`s02_legacy_biome_lift_is_total_and_round_trips` bảo vệ.

Liên quan:
[`SIMULATION_RULES.md`](SIMULATION_RULES.md) (keystone luật mô phỏng M0),
[`src-tauri/src/core/world_artifact.rs`](src-tauri/src/core/world_artifact.rs) (định dạng
World Artifact v2 + cả hai map),
[`src/components/Landscape/utils/worldGen.ts`](src/components/Landscape/utils/worldGen.ts)
(nguồn 22-biome).

Biome index xuất hiện trong **World Artifact v2** (`u8[n]` biome plane) luôn ở **không gian
canonical 22-biome** — backend project sang legacy tại thời điểm `to_terrain_map`.

## 2. Bảng canonical 22-biome (single source = `worldGen.ts`)

Tên EN = tên biến enum `Biome`; VI = `BIOME_NAMES_VI`; emoji = `BIOME_EMOJI`; RGB (0..255) =
`BIOME_RGB`. Tất cả cùng chỉ số, `BIOME_COUNT = 22`.

| Index | EN         | VI                | Emoji | RGB (0..255)      |
|-------|------------|-------------------|-------|-------------------|
| 0     | Ocean      | Đại dương         | 🌊    | (26, 60, 120)     |
| 1     | Beach      | Bãi biển          | 🏖    | (234, 216, 162)   |
| 2     | Desert     | Sa mạc            | 🏜    | (230, 196, 104)   |
| 3     | Savanna    | Xavan             | 🦁    | (200, 190, 96)    |
| 4     | Grassland  | Đồng cỏ           | 🌾    | (132, 196, 92)    |
| 5     | Shrubland  | Vùng cây bụi      | 🌿    | (150, 170, 92)    |
| 6     | Forest     | Rừng ôn đới       | 🌲    | (40, 124, 50)     |
| 7     | Jungle     | Rừng nhiệt đới    | 🌴    | (20, 98, 40)      |
| 8     | Taiga      | Rừng taiga        | 🌲    | (50, 104, 80)     |
| 9     | Tundra     | Đài nguyên        | ❄     | (166, 176, 154)   |
| 10    | Swamp      | Đầm lầy           | 🐊    | (66, 90, 54)      |
| 11    | Rock       | Núi đá            | ⛰     | (134, 128, 120)   |
| 12    | Snow       | Đỉnh tuyết        | 🏔    | (248, 251, 255)   |
| 13    | River      | Dòng sông         | 🏞    | (58, 132, 188)    |
| 14    | Lake       | Hồ nước           | 💧    | (42, 118, 176)    |
| 15    | Mangrove   | Rừng ngập mặn     | 🌴    | (60, 106, 68)     |
| 16    | Chaparral  | Rừng bụi khô      | 🍂    | (178, 158, 92)    |
| 17    | Steppe     | Thảo nguyên       | 🌾    | (190, 186, 120)   |
| 18    | Alpine     | Đồng cỏ núi cao   | 🏔    | (122, 162, 116)   |
| 19    | Badlands   | Đất cằn           | 🪨    | (176, 104, 66)    |
| 20    | Glacier    | Sông băng         | 🧊    | (220, 238, 248)   |
| 21    | Bog        | Đầm than bùn      | 🍄    | (72, 82, 58)      |

## 3. Bảng legacy 11-biome (backend `BiomeType`)

Nguồn = enum `BiomeType` (`#[repr(u8)]`) trong
[`src-tauri/src/core/terrain.rs`](src-tauri/src/core/terrain.rs).

| Index | Variant (backend `BiomeType`) |
|-------|-------------------------------|
| 0     | DeepOcean                     |
| 1     | Ocean                         |
| 2     | Beach                         |
| 3     | River                         |
| 4     | Grassland                     |
| 5     | TemperateForest               |
| 6     | BorealForest                  |
| 7     | Rainforest                    |
| 8     | Desert                        |
| 9     | MountainRock                  |
| 10    | Snow                          |

## 4. Forward map — legacy → canonical (11 → 22)

Hàm `map_biome_backend_to_frontend(back: u8) -> u8`. Đây là chiều **lift**: biểu diễn dữ liệu
world chỉ-có-ở-backend trong taxonomy 22-biome chung (ví dụ khi sim world được upsample ngược
lại không gian artifact). Mỗi legacy biome chọn một canonical **đại diện** gần nghĩa sinh thái
nhất.

| Legacy (11) | Variant         | → Canonical (22) | Biome        | Lý do sinh thái (rationale) |
|-------------|-----------------|------------------|--------------|-----------------------------|
| 0           | DeepOcean       | 0                | Ocean        | 22-palette không có thành viên "deep ocean" riêng → gộp vào Ocean chung (điểm mất mát duy nhất). |
| 1           | Ocean           | 0                | Ocean        | Ánh xạ trực tiếp cùng khái niệm nước biển hở. |
| 2           | Beach           | 1                | Beach        | Đới bờ cát, khớp 1-1. |
| 3           | River           | 13               | River        | Nước ngọt chảy, khớp 1-1. |
| 4           | Grassland       | 4                | Grassland    | Đồng cỏ ôn đới, khớp 1-1. |
| 5           | TemperateForest | 6                | Forest       | Rừng ôn đới lá rộng ⇒ `Forest` (Rừng ôn đới). |
| 6           | BorealForest    | 8                | Taiga        | Rừng lá kim phương bắc ⇒ Taiga đúng định nghĩa boreal. |
| 7           | Rainforest      | 7                | Jungle       | Rừng mưa nhiệt đới NPP cao ⇒ Jungle (Rừng nhiệt đới). |
| 8           | Desert          | 2                | Desert       | Sa mạc khô hạn, khớp 1-1. |
| 9           | MountainRock    | 11               | Rock         | Đá núi trơ, khớp 1-1. |
| 10          | Snow            | 12               | Snow         | Đỉnh phủ tuyết, khớp 1-1. |

## 5. Reverse map — canonical → legacy (22 → 11)

Hàm `map_biome_frontend_to_backend(front: u8) -> u8`. Đây là chiều **downsample** dùng ở
**runtime**: khi backend nạp World Artifact (`WorldArtifact::to_terrain_map`) nó chiếu 22-biome
palette xuống 11 legacy variant để seed NPP. Đây là collapse **lossy nhưng hợp lý sinh thái**:
các biome có năng suất (NPP) gần nhau gộp lại. Mọi index không xác định (`_`) rơi về Grassland (4).

| Canonical (22) | Biome      | → Legacy (11) | Variant         |
|----------------|------------|---------------|-----------------|
| 0              | Ocean      | 1             | Ocean           |
| 1              | Beach      | 2             | Beach           |
| 2              | Desert     | 8             | Desert          |
| 3              | Savanna    | 4             | Grassland       |
| 4              | Grassland  | 4             | Grassland       |
| 5              | Shrubland  | 4             | Grassland       |
| 6              | Forest     | 5             | TemperateForest |
| 7              | Jungle     | 7             | Rainforest      |
| 8              | Taiga      | 6             | BorealForest    |
| 9              | Tundra     | 6             | BorealForest (backend thiếu tundra → nearest cold vegetated) |
| 10             | Swamp      | 7             | Rainforest (ẩm ướt, NPP cao)   |
| 11             | Rock       | 9             | MountainRock    |
| 12             | Snow       | 10            | Snow            |
| 13             | River      | 3             | River           |
| 14             | Lake       | 1             | Ocean (nước tù, NPP ≈ 0)       |
| 15             | Mangrove   | 7             | Rainforest (rừng ẩm ven biển)  |
| 16             | Chaparral  | 4             | Grassland (cây bụi khô)        |
| 17             | Steppe     | 4             | Grassland       |
| 18             | Alpine     | 4             | Grassland (đồng cỏ núi cao)    |
| 19             | Badlands   | 8             | Desert          |
| 20             | Glacier    | 10            | Snow            |
| 21             | Bog        | 6             | BorealForest (đất ngập nước lạnh) |

## 6. Round-trip (11 → 22 → 11)

`map_biome_frontend_to_backend(map_biome_backend_to_frontend(back))` là **identity** cho **mọi**
legacy biome, **NGOẠI TRỪ** `DeepOcean` (0): vì 22-palette không có thành viên "deep ocean"
riêng, `DeepOcean` lift lên `Ocean` (canonical 0) rồi khi downsample rơi về `Ocean` (legacy 1),
KHÔNG quay lại `DeepOcean`. Đây là collapse **có chủ ý và được ghi nhận**, không phải bug.

- Với `back ∈ {1..=10}`: round-trip trả về đúng `back`.
- Với `back == 0` (DeepOcean): round-trip trả về `1` (Ocean).

Tính chất này được test **S02** chứng minh:
`core::world_artifact::tests::s02_legacy_biome_lift_is_total_and_round_trips`
trong [`src-tauri/src/core/world_artifact.rs`](src-tauri/src/core/world_artifact.rs). Test cũng
kiểm cả tính **total**: mọi legacy (0..=10) lift vào dải < 22, và mọi canonical (0..22)
downsample vào dải ≤ 10.

Chạy lại:

```bash
cd src-tauri && cargo test --lib -- s02_
```

## 7. Cách đổi taxonomy an toàn

Taxonomy là một hợp đồng cross-language; đổi sai sẽ khiến render-world và sim-world lệch nhau
âm thầm. Khi thêm/sửa/xoá biome, làm ĐỦ các bước sau **cùng một commit**:

1. **Sửa enum canonical** `Biome` trong
   [`worldGen.ts`](src/components/Landscape/utils/worldGen.ts) và cập nhật đồng bộ cả bốn nguồn
   dữ liệu song hành: `BIOME_COUNT`, `BIOME_RGB`, `BIOME_NAMES_VI`, `BIOME_EMOJI` (mọi mảng phải
   cùng độ dài, cùng thứ tự chỉ số).
2. Nếu chạm tới legacy: **sửa enum `BiomeType`** trong
   [`terrain.rs`](src-tauri/src/core/terrain.rs) (giữ `#[repr(u8)]`, index liên tục).
3. **Cập nhật CẢ HAI map** trong
   [`world_artifact.rs`](src-tauri/src/core/world_artifact.rs): `map_biome_frontend_to_backend`
   (canonical→legacy, giữ output ≤ 10) và `map_biome_backend_to_frontend` (legacy→canonical, giữ
   output < BIOME_COUNT). Xác định biome mới thuộc nhóm NPP nào để chọn đại diện hợp lý.
4. **Chạy lại S02** (`cargo test --lib -- s02_`) — nó kiểm totality và round-trip. Nếu bạn cố ý
   thêm một biome canonical-only nữa "gập" khi round-trip (giống DeepOcean), phải cập nhật kỳ
   vọng của S02 cho đúng.
5. **Cập nhật tài liệu này** (mục 2–6) để bảng khớp code.
6. Regenerate fixture nếu layout biome plane đổi ý nghĩa
   (`scripts/gen_artifact_fixture.ts` → `src-tauri/tests/fixtures/`), rồi chạy full
   `cargo test --lib` để `decodes_frontend_generated_fixture` và
   `real_frontend_world_becomes_valid_terrain_map` vẫn xanh.
