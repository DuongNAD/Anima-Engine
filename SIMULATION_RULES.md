# SIMULATION_RULES.md — Hợp đồng khoa học của Anima Engine (M0.1)

> Trạng thái: **Bản chốt M0** · Ngày: **2026-07-24**
> Đây là **nguồn sự thật** cho đơn vị đo, định luật bảo toàn và thang thời gian của mô phỏng.
> Bản cài đặt máy-kiểm-được (machine-checkable) nằm ở
> [`src-tauri/src/core/sim_rules.rs`](src-tauri/src/core/sim_rules.rs). **Khi sửa một hằng số ở
> một nơi, phải sửa ở cả hai** — các test S01/S03 sẽ chặn nếu tài liệu và mã lệch nhau.

Tài liệu đồng hành:
[BIOME_TAXONOMY.md](BIOME_TAXONOMY.md) ·
[COORDINATE_CONTRACT.md](COORDINATE_CONTRACT.md) ·
[MAP_MANIFEST.md](MAP_MANIFEST.md) ·
[BENCHMARK_BASELINE.md](BENCHMARK_BASELINE.md) ·
tổng thể: [WORLD_SIMULATION_PLAN.md](WORLD_SIMULATION_PLAN.md).

---

## 0. Cổng M0 (M0 gate)

> **Không triển khai bất kỳ hệ nhân–quả nào (M2+) khi đơn vị, pool bảo toàn và thang thời gian còn
> mơ hồ.** Mục đích của M0 là **xóa mơ hồ** đó và neo nó bằng test.

| Việc | Sản phẩm | Bằng chứng | Test |
|---|---|---|---|
| M0.1 | Tài liệu này + `sim_rules.rs` | Units table + pool + time scale được chốt | **S01** |
| M0.2 | [BIOME_TAXONOMY.md](BIOME_TAXONOMY.md) + map trong `world_artifact.rs` | 11↔22 không mơ hồ | **S02** |
| M0.3 | [COORDINATE_CONTRACT.md](COORDINATE_CONTRACT.md) + transforms trong `sim_rules.rs` | Contract dùng chung FE/BE | **S03** |
| M0.4 | [BENCHMARK_BASELINE.md](BENCHMARK_BASELINE.md) + harness | Báo cáo có seed/config/hardware | **S04** |
| M0.5 | [MAP_MANIFEST.md](MAP_MANIFEST.md) + schema + validator | Manifest thiếu field bắt buộc bị fail | **S05** |

---

## 1. Phạm vi MVP

MVP **chỉ** phủ vertical slice “**lưu vực – đồng cỏ – thỏ – sói**” (Mục 9,
[WORLD_SIMULATION_PLAN.md](WORLD_SIMULATION_PLAN.md)).

**Có trong MVP:**

- một World Artifact quyền lực duy nhất (elevation, biome, moisture, field-temperature, flow, water);
- pool **năng lượng/sinh khối đóng** (plants + animals + detritus);
- vòng đời tối thiểu cho thỏ (herbivore/prey) và sói (predator);
- decomposer cohort + carcass trả sinh khối về detritus;
- season clock điều khiển độ màu mỡ;
- causal ledger + scenario runner (M2) và bốn can thiệp mẫu: hạn hán, bỏ 80% sói, chặt 40% rừng
  đầu nguồn, bón nutrient quanh hồ.

**Hoãn (ngoài MVP):** bệnh/ký sinh/độc tố chi tiết (Mục 5.8), gió động, dòng hải lưu, hang động
THẬT, MAP-Elites nhiều chiều, và mọi pool bảo toàn ngoài năng lượng (nước/nutrient/toxin — xem §2.3).

---

## 2. Định luật bảo toàn (conservation)

### 2.1. Quyết định về năng lượng (chốt)

Theo Mục 2.3 của kế hoạch, có hai lựa chọn. **Chốt: năng lượng ≡ sinh khối tương đương, hệ ĐÓNG.**

- Năng lượng mất do hô hấp, phần săn mồi không hấp thụ và cái chết **được tái chế thành detritus**,
  không biến mất.
- Cây tái sinh bằng cách **rút từ pool detritus** (biomass-gated regrowth), nên tổng bảo toàn.
- Đây đúng là mô hình đang chạy: `EcosystemBiomass { detritus, plants, animals }` với
  `total() = detritus + plants + animals` giữ ~hằng số
  ([`core/ecology.rs`](src-tauri/src/core/ecology.rs)).

> Không được vừa gọi “energy” là năng lượng vật lý (Joule) vừa dùng như vật chất tuần hoàn. Trong MVP
> nó là **một đại lượng sinh khối-tương đương duy nhất**, ký hiệu **EU**. Việc tách nhiệt thất thoát
> khỏi vật chất hữu cơ là mở rộng sau MVP.

### 2.2. Pool đang được bảo toàn (MVP)

| Pool | Thành phần | Bất biến | Nguồn/Sink hợp lệ duy nhất |
|---|---|---|---|
| **Năng lượng/sinh khối (EU)** | `plants` + `animals` + `detritus` | `total` hằng số (± tolerance) | can thiệp `add/remove` cá thể; NPP forcing khai báo tường minh |

### 2.3. Pool **đã khai báo nhưng CHƯA bảo toàn** (đến M3)

Ghi tên ngay để hợp đồng trung thực về “cái gì đang / chưa được theo dõi”. Các pool này xuất hiện
trong registry với vai trò `DeferredM3`:

- **Nước (WV)** — khí quyển đơn giản + nước mặt + nước đất;
- **Dinh dưỡng (NM)** — soil nitrogen/fertility + organic matter;
- **Chất ô nhiễm (PM)** — chỉ khi scenario có ô nhiễm.

Trước M3, **không** được viết cơ chế nào giả định các pool này đã bảo toàn.

### 2.4. Census cá thể

Sinh / chết / di cư phải khép kín theo đầu người: mọi thay đổi số cá thể phải truy được về một sự
kiện (birth, death→carcass, migration, hoặc command). Được hình thức hóa ở M5.

---

## 3. Bảng đơn vị (units table)

Đây là bản người-đọc của `STATE_VARIABLES` trong
[`sim_rules.rs`](src-tauri/src/core/sim_rules.rs); **S01** khẳng định bảng này đầy đủ và nhất quán.
`EU` = biomass-equivalent energy unit; `WU` = water-reserve unit; `WV/NM/PM` = water-volume /
nutrient-mass / pollutant-mass (deferred M3).

| Biến | Đơn vị | Miền | Bảo toàn | Ghi chú |
|---|---|---|---|---|
| `elevation` | normalized [0,1] | 0..1 | — | y thực = value · 10 world-unit |
| `moisture` | normalized [0,1] | 0..1 | — | độ ẩm nền |
| `field_temperature` | normalized [0,1] (cold→hot) | 0..1 | — | nhiệt **của ô đất**, KHÁC nhiệt cơ thể |
| `flow` | normalized [0,1] | 0..1 | — | river flow accumulation |
| `body_temperature` | **°C** | 30..45 | — | nhiệt **cơ thể** (`HomeostaticState`), clamp trong `world_systems.rs` |
| `energy` | **EU** | 0..target | ✅ ClosedEnergy | năng lượng cá thể |
| `hydration` | **WU** | 0..target | — | dự trữ nước cá thể |
| `plants` | **EU** | ≥0 | ✅ ClosedEnergy | sinh khối cây đứng |
| `animals` | **EU** | ≥0 | ✅ ClosedEnergy | năng lượng trong động vật sống |
| `detritus` | **EU** | ≥0 | ✅ ClosedEnergy | năng lượng tự do (mùn) |
| `surface_water` | WV *(deferred M3)* | ≥0 | ⏳ | chưa theo dõi |
| `soil_nutrient` | NM *(deferred M3)* | ≥0 | ⏳ | chưa theo dõi |
| `toxin` | PM *(deferred M3)* | ≥0 | ⏳ | chưa theo dõi |

> **Mơ hồ được xóa quan trọng nhất:** có **HAI** đại lượng tên “temperature”. `field_temperature`
> là **normalized [0,1]** (trường địa lý); `body_temperature` là **°C** (nội môi cá thể). Không
> được trộn hai đại lượng này.

---

## 4. Thang thời gian (time scale)

Hằng số ở [`sim_rules.rs`](src-tauri/src/core/sim_rules.rs); **S01** khẳng định chúng tự nhất quán.

| Đại lượng | Giá trị | Nguồn |
|---|---|---|
| Tick rate | **60 Hz** (`TICK_HZ`) | `TimeStep(1.0/60.0)`, `core/ecs.rs` |
| Bước thời gian cố định | **1/60 s ≈ 0.016667** sim-giây (`TICK_DT_SECONDS`) | như trên |
| Epoch | **1000 tick** (`TICKS_PER_EPOCH`) | `EpochManager`, `core/simulation_loop.rs` |
| Một năm mô phỏng | **100 sim-giây = 6000 tick** (`SECONDS_PER_YEAR`, `TICKS_PER_YEAR`) | `SeasonClock rate = τ/100`, `core/ecology.rs` |

Các **tần suất cập nhật mục tiêu** (ngân sách, [WORLD_SIMULATION_PLAN.md](WORLD_SIMULATION_PLAN.md) §10.2):
physics 60 Hz · brain/sensor 10–20 Hz batched · ecology cục bộ 1 Hz · plant/decomposition
0.1–0.2 Hz · UI telemetry 1–5 Hz · **hot-loop allocation = 0**.

---

## 5. Hệ tọa độ (tóm tắt)

Chi tiết + transforms ở [COORDINATE_CONTRACT.md](COORDINATE_CONTRACT.md); cài đặt +
round-trip test (**S03**) ở [`sim_rules.rs`](src-tauri/src/core/sim_rules.rs).

- Lưới sim backend mặc định **256×256** ô (`DEFAULT_GRID_DIM`, khớp `MapSettings::default()`),
  seed 1337 → **0.78125** world-unit mỗi ô.
- World-space: `x,z ∈ [-100, 100]` (200×200 world-unit), `y = elevation · 10`.
- Bốn không gian: **cell → uv[0,1] → world → render**. Quy ước tâm ô `uv = ((ix+.5)/W, (iy+.5)/H)`;
  world→cell dùng đúng luật `floor(coord·dim)` như `TerrainMap::get_map_indices`, nên tâm mỗi ô
  luôn bucket lại về chính ô đó.

---

## 6. Taxonomy biome (tóm tắt)

Chi tiết ở [BIOME_TAXONOMY.md](BIOME_TAXONOMY.md); map + round-trip test (**S02**) ở
[`world_artifact.rs`](src-tauri/src/core/world_artifact.rs).

- **Taxonomy quyền lực (mới): 22 biome** — enum `Biome` trong `worldGen.ts` (`CANONICAL_BIOME_COUNT`).
- **Taxonomy legacy: 11 biome** — enum `BiomeType` backend (`LEGACY_BIOME_COUNT`).
- Hai map toàn phần: `map_biome_frontend_to_backend` (22→11, đang chạy runtime) và
  `map_biome_backend_to_frontend` (11→22, forward lift). Round-trip là identity cho mọi biome legacy
  **trừ** `DeepOcean` (gộp vào `Ocean` — 22-palette không có deep-ocean).

---

## 7. Bất biến & tolerance

| Bất biến | Ngưỡng khởi đầu | Ghi chú |
|---|---|---|
| Sai số bảo toàn năng lượng / tick | `< 1e-9` (f64) trong pure test | Đo trên hệ đầy đủ ở M3+; siết dần |
| Deterministic replay divergence | 0 (cùng seed+config+chuỗi can thiệp) | Hình thức hóa ở M2.4 |
| Field normalized | luôn nằm trong [0,1] | S17 (M3) canh biên |
| Hot-loop allocation | **0** | Test `allocs == 0` hiện có |

> Con số hiệu năng (frame/tick/RAM) **không** được khóa trước khi chạy M0.4 trên phần cứng đích
> (Dell Vostro 3530). Xem [BENCHMARK_BASELINE.md](BENCHMARK_BASELINE.md).

---

## 8. Ánh xạ scenario → test (machine-check)

| Scenario | Ý nghĩa | Kiểm bởi |
|---|---|---|
| **S01** | pool bảo toàn + đơn vị machine-check | `core::sim_rules::tests::s01_*` (2 test) |
| **S02** | mọi biome legacy map sang taxonomy mới | `core::world_artifact::tests::s02_legacy_biome_lift_is_total_and_round_trips` |
| **S03** | round-trip world↔grid↔render coordinate | `core::sim_rules::tests::s03_*` (2 test) |
| **S04** | benchmark baseline có seed/config/hardware | [BENCHMARK_BASELINE.md](BENCHMARK_BASELINE.md) |
| **S05** | manifest thiếu field bắt buộc phải fail | [MAP_MANIFEST.md](MAP_MANIFEST.md) |

Chạy nhanh phần backend:

```bash
cd src-tauri && cargo test --lib -- s01_ s02_ s03_
```
