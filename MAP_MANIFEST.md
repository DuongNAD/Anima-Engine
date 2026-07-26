# Map Manifest — schema, canonical views và map-vision gate (M0.5 / S05)

> Milestone: **M0.5** — "Tạo map manifest schema và canonical camera/view list" ([WORLD_SIMULATION_PLAN.md](WORLD_SIMULATION_PLAN.md) §7, M0).
> Test: **S05** — "map manifest thiếu field bắt buộc phải fail" (§8.1).
> Bằng chứng hoàn tất: một manifest **validate được**; thiếu bất kỳ required field nào thì **invalid**.

Một **map manifest** là hợp đồng dùng chung nối một [World Artifact](src-tauri/src/core/world_artifact.rs) (định dạng `ANMW`, xem thêm [worldArtifact.ts](src/components/Landscape/utils/worldArtifact.ts)) với:

- **coordinate system** dùng chung cho render, collision, navigation và simulation;
- **biome taxonomy** (22 canonical frontend ↔ 11 legacy backend);
- **canonical camera/view list** mà map-vision gate sẽ inspect (§13).

Manifest là dữ liệu tĩnh, versioned; nó **không** chứa pixel — chỉ trỏ tới artifact bytes (`worldArtifact.path`) và tới các ảnh view đã render (`views[].imagePath`).

---

## 1. Artifacts của milestone này

| File | Vai trò |
|---|---|
| [`map_manifest.schema.json`](map_manifest.schema.json) | JSON-Schema **draft-07** — nguồn sự thật về hình dạng manifest. |
| [`map_manifest.json`](map_manifest.json) | Manifest **mẫu** hợp lệ, liệt kê đủ 8 canonical view. |
| [`src/components/Landscape/utils/mapManifest.ts`](src/components/Landscape/utils/mapManifest.ts) | Validator **thuần TS**, không dependency (`validateMapManifest`). |
| [`src/__tests__/mapManifest.test.ts`](src/__tests__/mapManifest.test.ts) | Vitest (S05): mẫu `ok:true`; xóa required field → `ok:false`. |

> Ghi chú: tại phiên làm việc này, MCP `animal-map-vision` **không** có mặt, nên đây là **schema + local validator** (kiểm tra cấu trúc), **chưa** phải visual gate. Bước inspect ảnh thực tế thuộc về gate ở §4.

---

## 2. Schema — các required field

Thiếu **bất kỳ** field bắt buộc nào ở dưới đây làm manifest **invalid** (đúng yêu cầu S05).

### 2.1. Top-level

| Field | Kiểu | Ràng buộc |
|---|---|---|
| `schemaVersion` | number | bắt buộc |
| `worldArtifact` | object | bắt buộc (xem 2.2) |
| `coordinateSystem` | object | bắt buộc (xem 2.3) |
| `biomeTaxonomy` | object | bắt buộc (xem 2.4) |
| `views` | array | bắt buộc, `minItems >= 1` (xem 2.5) |

### 2.2. `worldArtifact`

| Field | Kiểu | Ghi chú |
|---|---|---|
| `path` | string | bắt buộc — đường dẫn tới file `.anmw`. |
| `magic` | `"ANMW"` (const) | bắt buộc — `WORLD_ARTIFACT_MAGIC`. |
| `version` | integer ≥ 1 | bắt buộc — `WORLD_ARTIFACT_VERSION` (hiện tại 2). |
| `width` | integer ≥ 1 | bắt buộc. |
| `height` | integer ≥ 1 | bắt buộc. |
| `seaLevel` | number | bắt buộc — sea level chuẩn hóa `[0,1]`. |
| `checksum` | string | **tùy chọn** — ví dụ `sha256:...` của artifact bytes. |

### 2.3. `coordinateSystem`

Các giá trị cố định theo [SIMULATION_RULES.md](SIMULATION_RULES.md) / `MapBounds::default` trong [resources.rs](src-tauri/src/core/resources.rs).

| Field | Giá trị | Ý nghĩa |
|---|---|---|
| `worldMinXZ` | `-100` | min world X và Z (ô vuông world 200×200 unit). |
| `worldMaxXZ` | `100` | max world X và Z. |
| `worldMinY` | `0` | `WORLD_MIN_Y`. |
| `worldMaxY` | `10` | `WORLD_MAX_Y` (= `elevation * 10`). |
| `gridDim` | integer ≥ 1 | grid backend (`DEFAULT_GRID_DIM = 256`, khớp `MapSettings::default()`). |

> Hai coordinate convention cùng tồn tại và **khác nhau** (xem `sim_rules.rs`):
> - **cell-bucket** (`get_map_indices`): `u=(x-min.x)/(max.x-min.x)`, `ix=floor(u*W).min(W-1)`; ngoài `[0,1]` → không có cell. Round-trip: tâm cell luôn bucket về đúng cell đó (property S03).
> - **node-interpolate** (`get_elevation_at_pos`): bilinear trên lưới node `(W-1)×(H-1)`, `fx=clamp(u,0,1)*(W-1)`.
> Render space là một **phép scale thuần** của world space (không xoay, không shear), định nghĩa phía frontend.

### 2.4. `biomeTaxonomy`

| Field | Giá trị | Ghi chú |
|---|---|---|
| `canonicalCount` | `22` | số biome canonical (frontend `Biome`). |
| `legacyCount` | `11` | số biome legacy (backend `BiomeType`). |

Round-trip 11→22→11 là identity cho mọi legacy biome **trừ** `DeepOcean(0)` — palette 22 không có thành viên deep-ocean riêng nên fold vào `Ocean` (xem `world_artifact.rs`).

### 2.5. `views[]`

Mỗi phần tử:

| Field | Kiểu | Ràng buộc |
|---|---|---|
| `id` | enum | một trong **8 canonical view id** (xem §3). |
| `imagePath` | string | bắt buộc — ảnh đã render cho view. |
| `camera` | object | bắt buộc: `position:[x,y,z]`, `target:[x,y,z]` (mỗi cái là triple số). |

---

## 3. Tám canonical view (plan §13)

Các view cần kiểm tra **nếu tồn tại**, theo đúng danh sách trong [WORLD_SIMULATION_PLAN.md](WORLD_SIMULATION_PLAN.md) §13:

| `id` | Mục đích kiểm định |
|---|---|
| `overview` | Toàn cảnh map — bố cục biome, bờ biển, khối núi. |
| `navigation` | Vùng đi lại được / navmesh reachability. |
| `collision` | Collider trùng với terrain/nước đang render. |
| `lighting` | Ánh sáng, bóng, tương phản ngày. |
| `spawn` | Vùng spawn hợp lệ (land vs water). |
| `water` | Hồ, sông, cửa thoát; mực nước render ↔ sim. |
| `biome_transition` | Chuyển tiếp biome không gãy/không seam. |
| `ecosystem` | Mật độ thực vật/wildlife phản ánh sinh khối thật. |

Camera position trong manifest mẫu nằm trong world space `[-100,100]`, `target` hướng vào vùng quan tâm.

---

## 4. Map-vision pipeline (thứ tự bắt buộc)

Mọi milestone chạm terrain, biome, ecosystem placement, navigation, collision, water hoặc lighting phải chạy đúng thứ tự (§13):

```text
1. discover_map_artifacts    → tìm artifact + manifest
2. validate_map_manifest     → BƯỚC NÀY: schema + validator ở trên
3. prepare_team_review       → gom views, region, baseline
4. inspect_map_views         → xem từng canonical view, ghi finding
```

`validateMapManifest` phủ **bước 2**. Bước 4 (inspect ảnh) cần MCP `animal-map-vision`, không có trong phiên này.

---

## 5. Finding record — các field bắt buộc (§13)

Mỗi finding khi inspect view phải có đủ:

- `severity`;
- `image path`;
- `region`;
- `observed evidence`;
- `hypothesis` — **tách riêng** khỏi evidence;
- `gameplay/ecology impact`;
- `proposed fix`;
- `before/after reproduction check`.

Không tuyên bố map hoàn tất khi: manifest chưa pass; còn critical/high finding; canonical before/after view chưa được xem; navigation chưa reachable; render/collider/navmesh/simulation/minimap chưa đồng nhất; còn mâu thuẫn sinh thái.

---

## 6. Dùng validator

```ts
import { validateMapManifest } from '@/components/Landscape/utils/mapManifest';

const res = validateMapManifest(JSON.parse(rawManifestText));
if (!res.ok) {
  console.error('manifest invalid:', res.errors);
}
```

`validateMapManifest(obj: unknown): { ok: boolean; errors: string[] }` — thuần, không dependency, không import (dùng được từ pure test). Nó kiểm tra presence + kiểu của mọi required field và enforce enum view id; extra property không khai báo được **bỏ qua** (forward-compatible). Nguồn sự thật hình dạng là [`map_manifest.schema.json`](map_manifest.schema.json); giữ hai bên đồng bộ.

---

## 7. Chạy test (S05)

```bash
npx vitest run src/__tests__/mapManifest.test.ts
```

Kỳ vọng: manifest mẫu `ok:true`; xóa một required field (hoặc dùng view id lạ) → `ok:false` kèm error mô tả.
