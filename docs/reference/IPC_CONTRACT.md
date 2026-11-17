---
title: Hợp đồng IPC Tauri (lệnh và event)
status: active
owner: architecture
last_reviewed: 2026-11-17
review_cycle: per-release
---

# Hợp đồng IPC Tauri

Danh sách **đầy đủ** lệnh mà frontend gọi được và event mà backend phát. Trước 2026-11-14 không có
tài liệu nào như vậy: CLAUDE.md trỏ tới mục "Interface Contracts" của `PROJECT.md`, nhưng mục đó
không có lệnh nào, và bảy lệnh CLAUDE.md tự liệt kê chỉ là một phần của 29 lệnh đã đăng ký.

**Nguồn chuẩn là mã**, tài liệu này là chỉ mục của nó:

- Lệnh: `tauri::generate_handler![…]` trong [`src-tauri/src/lib.rs`](../../src-tauri/src/lib.rs);
  thân lệnh ở `src-tauri/src/commands/`.
- Event: mọi lời gọi `handle.emit("…", …)` trong `src-tauri/src/`.
- Kiểu payload xuất sang TypeScript qua `ts-rs` vào `src/types/generated/`.

`node scripts/check_ipc_reference.mjs` (một phần của `npm run check:docs`, chạy trên CI) đỏ khi hai
bảng dưới đây và mã lệch nhau theo **cả hai chiều**: lệnh/event có trong mã mà thiếu ở đây, hoặc có
ở đây mà mã không còn.

## Quy tắc tên tham số

`tauri-macros` đổi tên tham số Rust sang **camelCase** phía JS: `file_path: String` nhận khoá
`filePath`. Gửi `file_path` làm lệnh lỗi khi deserialize; với `Option<T>` thì tệ hơn — khoá sai đọc
thành `None` và lệnh lặng lẽ đi đường mặc định. `npm run check:ipc` kiểm mọi `invoke(...)` của
frontend theo quy tắc này. Cột "Tham số JS" dưới đây đã viết sẵn ở dạng camelCase.

## Lệnh

Lệnh không có doc comment trong mã được mô tả bằng kiểu trả về, không đoán ngữ nghĩa.

### Mô phỏng — `commands/simulation.rs`

| Lệnh | Tham số JS | Trả về | Ghi chú |
|---|---|---|---|
| `get_simulation_status` | — | `SimulationStatus` | |
| `toggle_simulation` | — | `bool` | Bật/tắt vòng mô phỏng |
| `save_simulation_state` | `filePath: string` | `bool` | Một **tên** save, không phải đường dẫn — xem `commands/save_paths.rs` |
| `load_simulation_state` | `filePath: string` | `bool` | Như trên |
| `list_legacy_saves` | — | `LegacyImportListing` | Chỉ đọc thư mục app sở hữu, trả tên |
| `import_legacy_save` | `legacyName: string`, `saveAs: string` | `string` | Nhập save có từ trước khi giới hạn đường dẫn, **chỉ đọc**, vào thư mục save của app |
| `get_test_rabbit_state` | — | `tauri::ipc::Response` | Phản hồi thô, không qua JSON |
| `set_lod_focus` | `focus: LodFocus` | — | `enabled: false` trả engine về độ chi tiết đồng đều (mặc định) |
| `get_lod_focus` | — | `LodFocus` | |
| `get_lod_bands` | — | `LodBands` | Ranh giới tầng LOD của bản build |
| `export_scientific_dataset` | `req: ScientificExportRequest` | `ScientificExportResponse` | Gói `schemaVersion` 2. `provenance.seed` là seed thật mà luồng mô phỏng công bố (`SimulationEngine::run_seed`); **lỗi** khi chưa có lượt chạy nào công bố tick đầu tiên (chưa khởi động, đang khởi động, hoặc dừng trước tick đầu), thay cho seed 42 cũ; seed chỉ hiện sau tick đầu nên đi cùng trạng thái của chính lượt chạy đó (segments, hệ sinh thái, census, flux, status); bốn thứ **không** phải số đo theo lượt chạy được nêu ở câu "Chưa sạch" bên dưới. `preyBiomass` / `predatorBiomass` là reserve của các agent sống theo vai trò. Bốn trường hằng số (`energyResidual`, hai tốc độ trao đổi chất, `ambientTemperatureC`) đã bỏ. `energyFlux` là `null` khi chưa đo được flux. **Chưa sạch:** `spatialDensity.vegetationDensity` luôn 0.5 mỗi ô và số đực/cái của `demographics` được chia theo chẵn lẻ (chưa đo); `newickTree` (phả hệ) và `mapElitesSummary` (kho MAP-Elites dùng chung, chỉ có ô khi tiến hóa đang bật) là của cả tiến trình, `start` không reset, nên sau một lần khởi động lại chúng còn nhánh và ô của lượt chạy trước. Gói không phải một ảnh chụp nguyên tử: các giá trị được đọc qua nhiều khóa riêng |

### Môi trường — `commands/environment.rs`

| Lệnh | Tham số JS | Trả về | Ghi chú |
|---|---|---|---|
| `get_ecosystem_state` | — | `EcosystemState` | |
| `get_terrain_map` | — | `TerrainMapState` | |
| `get_pheromone_grid` | — | `PheromoneGridState` | |
| `get_environmental_elements` | — | `EnvironmentalState` | |
| `get_active_raycasts` | — | `RaycastTelemetry[]` | |
| `get_chronicle_history` | — | `ChronicleEvent[]` | |
| `save_world_artifact` | `bytes: number[]` | `string` | Ghi World Artifact do frontend sinh ra vào đường dẫn chuẩn, để `init_world` khởi động trên **đúng** thế giới đang render. Định dạng: [`PROJECT.md` §Interface Contracts 1](../../PROJECT.md) |
| `get_demographic_census` | — | `DemographicCensusState` | |
| `get_energy_flux_matrix` | — | `EnergyFluxReport` | **Lỗi** `energy flux is not measured…` khi chưa có `EnergyFluxMatrix`. Hiện luôn như vậy vì chưa có gì ghi nó; trước đây lệnh trả chín dòng chảy cố định (350, 120, 25, …) trông như số đo. Frontend coi lỗi này là "chưa có dữ liệu" |
| `get_spatial_density_matrix` | `width?: number`, `height?: number` | `SpatialDensityReport` | |

### Tiến hoá — `commands/evolution.rs`

| Lệnh | Tham số JS | Trả về | Ghi chú |
|---|---|---|---|
| `get_map_elites_grid` | — | `MapElitesGridState` | |
| `update_evolution_settings` | `settings: EvolutionSettings` | `bool` | |
| `toggle_evolution` | — | `bool` | |
| `get_lineage_graph` | — | `LineageGraphPayload` | |
| `get_lineage_mrca` | `individuals: string[]` | `LineageMrcaPayload` | OSS-072. Chỉ đọc. Kết quả là **tập** tổ tiên chung gần nhất (phả hệ là DAG vì crossover): rỗng = không có tổ tiên chung (bình thường ở rừng nhiều gốc), `ambiguous: true` = nhiều hơn một. Tổ tiên có tính phản xạ; truy vấn rỗng bị từ chối. Xem `evolution/mrca.rs` |
| `export_phylogenetic_tree` | `req?: NewickExportRequest` | `string` | |

### Mạng / sharding — `commands/networking.rs`

| Lệnh | Tham số JS | Trả về | Ghi chú |
|---|---|---|---|
| `trigger_migration` | `targetPort: number` | — | |
| `set_sharding_config` | `config: ShardingConfig` | — | |
| `get_sharding_config` | — | `ShardingConfig` | |
| `get_migration_handoff_diagnostics` | — | `MigrationHandoffSnapshot` | |

### Đo tick trong app — `commands/capture.rs`

Cách dùng: [BENCHMARKING.md](../how-to/BENCHMARKING.md).

| Lệnh | Tham số JS | Trả về | Ghi chú |
|---|---|---|---|
| `start_tick_capture` | `config: CaptureConfig` | — | Bỏ capture cũ. Cấu hình sai bị **từ chối**, không bị kẹp về giá trị hợp lệ |
| `stop_tick_capture` | — | — | Idempotent; giữ dữ liệu đã thu để còn export |
| `get_tick_capture_status` | — | `CaptureStatusReport` | |
| `export_tick_capture` | `fileName?: string` | `CaptureExport` | `fileName` là một tên, không phải đường dẫn |

Mọi lệnh trả `Result<T, String>` trừ `get_lod_bands` (trả thẳng `LodBands`) và
`get_test_rabbit_state`; phía JS, `Err` thành promise bị reject với chuỗi lỗi.

## Event (7)

| Event | Phát ở | Payload |
|---|---|---|
| `simulation-tick` | `core/emit.rs` | snapshot tick (`tick_payload`) |
| `pheromone-update` | `core/emit.rs` | lưới pheromone cục bộ |
| `raycast-update` | `core/emit.rs` | danh sách raycast của tick |
| `combat-event` | `core/emit.rs` | một `CombatEvent` mỗi lần phát; nhiều event mỗi tick |
| `migration-event` | `core/networking_systems.rs` | payload migration (ba điểm phát) |
| `chronicle-event` | `core/simulation_loop.rs` | `ChronicleEvent` |
| `map-elites-update` | `core/simulation_loop.rs` | lưới MAP-Elites |

## Khi đổi bề mặt IPC

1. Sửa lệnh/event trong mã **và** bảng ở đây trong cùng commit — gate đỏ nếu chỉ sửa một bên.
2. Nếu kiểu payload đổi, sinh lại binding `ts-rs` và chạy `npm run check:ipc`.
3. Một lệnh nhận tên file luôn nhận **tên**, không nhận đường dẫn (`commands/save_paths.rs`).
