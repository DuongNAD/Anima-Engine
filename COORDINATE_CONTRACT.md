# COORDINATE_CONTRACT.md — hợp đồng toạ độ / không gian / thời gian (M0.3)

Tài liệu này là bản văn xuôi của *coordinate contract* dùng chung giữa **backend Rust** và
**frontend TypeScript**. Nó cố định bốn không gian toạ độ, các phép biến đổi chính xác giữa chúng,
hai luật world→grid khác nhau (cell-bucket vs node-interpolate), bảng độ phân giải, tóm tắt world/time
scale và tính chất round-trip **S03**.

Nguồn sự thật *machine-checked* (không được lệch khỏi tài liệu này):

- Backend: [`src-tauri/src/core/sim_rules.rs`](src-tauri/src/core/sim_rules.rs) — hằng số + hàm thuần.
- Backend (luật gốc): [`src-tauri/src/core/terrain.rs`](src-tauri/src/core/terrain.rs) — `TerrainMap`.
- Frontend (bản mirror): [`src/components/Landscape/utils/coordinate.ts`](src/components/Landscape/utils/coordinate.ts).
- Test S03 (frontend): [`src/__tests__/coordinateContract.test.ts`](src/__tests__/coordinateContract.test.ts).

---

## 1. Bốn không gian toạ độ (four spaces)

| Không gian | Kiểu | Miền giá trị | Ý nghĩa |
|---|---|---|---|
| **cell** | `(ix, iy)` nguyên | `0 ≤ ix < W`, `0 ≤ iy < H` | Chỉ số ô lưới. Flat index `i = iy*W + ix` (row-major). |
| **uv** | `(u, v)` thực | `[0,1] × [0,1]` | Toạ độ chuẩn hoá. Tâm ô = `((ix+0.5)/W, (iy+0.5)/H)`. |
| **world** | `(x, y, z)` thực | `x,z ∈ [minX,maxX]×[minZ,maxZ]`, `y ∈ [0,10]` | Không gian mô phỏng backend (world-units). |
| **render** | `(x, y, z)` thực | scale thuần của `world` | Cảnh 3D. Là một phép **scale thuần** của `world`, định nghĩa phía frontend, **không xoay / không shear**. |

Vì `render` chỉ là scale thuần của `world` nên mọi tính chất round-trip chứng minh trên `world` cũng
đúng trên `render` (một phép biến đổi affine đường chéo, khả nghịch).

---

## 2. Các phép biến đổi chính xác

Ký hiệu: `W = width`, `H = height`. Backend làm việc bằng `f32`; frontend bằng `f64` (`number`) — miền
giá trị ở đây đủ nhỏ để sai số `f64` không bao giờ đẩy `floor` sang ô khác (sai số ≪ 0.5).

### cell → uv (tâm ô)

```
u = (ix + 0.5) / W
v = (iy + 0.5) / H
```

Rust: `cell_center_uv` ([sim_rules.rs:83](src-tauri/src/core/sim_rules.rs#L83)).
TS: `cellCenterUv` ([coordinate.ts](src/components/Landscape/utils/coordinate.ts)).

### uv → cell (luật **cell-bucket**, floor)

```
ix = clamp(floor(u · W), 0, W-1)
iy = clamp(floor(v · H), 0, H-1)
```

Rust dùng `((u * W as f32) as usize).min(W-1)`; cast `as usize` của Rust *saturate* về 0 với số âm,
nên bản TS `clamp(..., 0, ...)` tái hiện đúng hành vi đó.
Rust: `uv_to_cell` ([sim_rules.rs:93](src-tauri/src/core/sim_rules.rs#L93)). TS: `uvToCell`.

### cell → world (tâm ô)

```
(u, v) = cell_center_uv(ix, iy, W, H)
x = minX + u · (maxX - minX)
z = minZ + v · (maxZ - minZ)
```

Rust: `cell_center_to_world_xz` ([sim_rules.rs:101](src-tauri/src/core/sim_rules.rs#L101)).
TS: `cellCenterToWorldXz`.

### world → cell (luật **cell-bucket**, nghịch đảo)

```
xRange = maxX - minX ; zRange = maxZ - minZ
nếu xRange ≤ 0 hoặc zRange ≤ 0        → None/null (bounds suy biến)
u = (x - minX) / xRange ; v = (z - minZ) / zRange
nếu u ∉ [0,1] hoặc v ∉ [0,1]          → None/null (ngoài biên)
ngược lại                             → uv_to_cell(u, v, W, H)
```

Kiểm tra `[0,1]` **bao gồm cả hai đầu**, nên góc `max` phân giải về ô cực đại (`W-1, H-1`).
Rust: `world_xz_to_cell` ([sim_rules.rs:119](src-tauri/src/core/sim_rules.rs#L119)) —
**giống hệt** `TerrainMap::get_map_indices` ([terrain.rs:563](src-tauri/src/core/terrain.rs#L563)).
TS: `worldXzToCell` (trả `null` khi ngoài biên).

---

## 3. HAI luật world→grid: cell-bucket vs node-interpolate

Đây là điểm dễ nhầm nhất, phải phân biệt rõ:

### (a) Luật **cell-bucket** — `get_map_indices` ([terrain.rs:563](src-tauri/src/core/terrain.rs#L563))

- Câu hỏi: *"điểm này thuộc **ô** nào?"* (quyền sở hữu ô: biome, occupancy, spatial hashing…).
- Chia miền thành `W × H` ô bằng nhau; `floor(coord · dim)` rồi clamp về ô cuối.
- Rời rạc, không nội suy. Đây là luật mà `sim_rules.rs` và `coordinate.ts` tái hiện, và là luật mà
  tính chất **S03** kiểm chứng.

### (b) Luật **node-interpolate** — `get_elevation_at_pos` ([terrain.rs:579](src-tauri/src/core/terrain.rs#L579))

- Câu hỏi: *"giá trị **trường** (elevation) *trơn* tại vị trí này là bao nhiêu?"*
- Coi các giá trị ô như mẫu tại **nút** trên lưới `(W-1) × (H-1)`: `fx = clamp(u,0,1) · (W-1)`,
  `ix = floor(fx)`, phần lẻ `tx = fx - ix`, rồi **nội suy song tuyến (bilinear)** giữa
  `h00, h10, h01, h11`.
- Liên tục, dùng để lấy độ cao mượt cho mesh/di chuyển — **không** dùng để hỏi "thuộc ô nào".

> Tại sao hệ số khác nhau? cell-bucket nhân với `dim` (số **ô**); node-interpolate nhân với `dim - 1`
> (số **khoảng** giữa các nút). Dùng nhầm luật sẽ lệch nửa ô ở biên. Quy tắc: **quyền sở hữu ô → (a);
> đọc giá trị trường trơn → (b).**

---

## 4. Bảng độ phân giải (resolution table)

MapBounds mặc định: `min = (-100, 0, -100)`, `max = (100, 10, 100)` → hình vuông ngang **200 × 200**
world-units ([resources.rs](src-tauri/src/core/resources.rs), `MapBounds::default`).

| Nguồn | Lưới | Phủ world | units / ô (ngang) |
|---|---|---|---|
| Backend sim (mặc định) | 128 × 128 (`DEFAULT_GRID_DIM`) | 200 × 200 | 200 / 128 = **1.5625** |
| Frontend worldGen | 128 | (theo bounds khi map sang world) | 200 / 128 = 1.5625 |
| Frontend worldGen | 256 | " | 200 / 256 = 0.78125 |
| Frontend worldGen | 1024 | " | 200 / 1024 = 0.1953125 |
| Frontend worldGen | 2048 | " | 200 / 2048 = 0.09765625 |

**`world_scale` trong World Artifact v2**: header dài **36 byte**
(`magic, version=2, width, height, seaLevel, seed, generator_version, world_scale, checksum` — xem
[`worldArtifact.ts`](src/components/Landscape/utils/worldArtifact.ts) /
[`world_artifact.rs`](src-tauri/src/core/world_artifact.rs)). `world_scale` là **trường tường minh**
(offset 28, `f32`) mang `CANONICAL_WORLD_SCALE = 200`, và giá trị này **bằng đúng độ rộng của
`MapBounds`**: một artifact `W × H` được trải lên hình chữ nhật `[minX,maxX] × [minZ,maxZ]`, mỗi ô rộng
`(maxX-minX)/W` × `(maxZ-minZ)/H` world-units. Frontend giữ mặc định này trong `DEFAULT_XZ_BOUNDS`
([coordinate.ts](src/components/Landscape/utils/coordinate.ts)).

Trường `y` (độ cao): `elevation ∈ [0,1]` chuẩn hoá → `y = elevation · 10` world-units
(`WORLD_MIN_Y = 0`, `WORLD_MAX_Y = 10`).

---

## 5. Tóm tắt world / time scale

**Không gian:**

- Lưới backend mặc định 128 × 128, seed 1337 (`MapSettings::default`, terrain.rs).
- Ngang: `x, z ∈ [-100, 100]` (200 × 200 world-units). Dọc: `y ∈ [0, 10]` từ `elevation·10`.
- Frontend: các trường chuẩn hoá `[0,1]`, `seaLevel: f32` theo từng world; kích thước ∈ {128, 256,
  1024, 2048}.

**Thời gian** (hằng số trong [sim_rules.rs](src-tauri/src/core/sim_rules.rs)):

| Hằng số | Giá trị |
|---|---|
| `TICK_HZ` | 60 |
| `TICK_DT_SECONDS` | 1/60 ≈ 0.016667 sim-giây |
| `TICKS_PER_EPOCH` | 1000 |
| `SECONDS_PER_YEAR` | 100 |
| `TICKS_PER_YEAR` | 6000 (= 100 × 60) |

`SeasonClock`: `phase` (radian), `rate = TAU / 100` rad/sim-giây,
`fertility = 1 + 0.5·sin(phase)` clamp `≥ 0`.

---

## 6. Tính chất S03 (round-trip identity)

> **S03** — tâm của một ô luôn bucket ngược lại đúng ô đó.
> Với mọi `(ix, iy)`: `world_xz_to_cell(cell_center_to_world_xz(ix, iy)) == (ix, iy)`.

Kéo theo: `world → cell → world-centre → cell` là **điểm bất động** (bucketing idempotent), và một
điểm ngoài biên trả `None`/`null`.

- Backend đã kiểm chứng:
  `core::sim_rules::tests::s03_cell_center_round_trips_through_world`
  (+ `s03_out_of_bounds_world_point_has_no_cell`), [sim_rules.rs:350](src-tauri/src/core/sim_rules.rs#L350).
- Frontend đã kiểm chứng: [`src/__tests__/coordinateContract.test.ts`](src/__tests__/coordinateContract.test.ts)
  chạy qua `npm run test` (Vitest + jsdom), sweep các lưới `{1², 4², 16², 128², 200×137}` cộng round-trip
  world và các trường hợp biên/ngoài-biên.

Vì `coordinate.ts` là bản mirror thuần của `sim_rules.rs`, hai phía FE/BE luôn chọn cùng một ô cho
cùng một điểm — điều kiện tiên quyết để agent (backend) và cảnh render (frontend) sống trong **cùng
một thế giới**.
