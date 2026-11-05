---
title: Hợp đồng IPC Tauri — lệnh và sự kiện giữa Rust và webview
status: active
owner: maintainers
last_reviewed: 2026-11-05
review_cycle: per-release
---

# Hợp đồng IPC Tauri

Đây là **nguồn chuẩn duy nhất** cho bề mặt IPC giữa backend Rust và webview. Trước đây hợp đồng này
nằm trong `PROJECT.md` ("Interface Contracts"), và file đó đã bị tài liệu dự án của agent khác ghi đè
ba lần; lần nào hợp đồng cũng biến mất mà `CLAUDE.md` vẫn trỏ tới đó. Bản này dựng lại từ văn bản
cuối cùng có người duyệt (`PROJECT.md` tại `1b3e7ef`), rồi đối chiếu với mã hiện tại.

**Hai phần, hai cách giữ đúng:**

- **Bảng lệnh và bảng sự kiện** ở [§3](#3-bảng-sinh-từ-mã) được **sinh từ mã** bằng
  `node scripts/gen_ipc_contract.mjs`: đọc `generate_handler!` trong `src-tauri/src/lib.rs`, chữ ký
  mọi hàm `#[tauri::command]`, mọi lời gọi `emit("…")`, và mọi `invoke`/`listen` literal trong `src/`.
  `npm run check:ipc-contract` (tức `--check`) đỏ khi bảng lệch khỏi mã. Đừng sửa tay giữa hai marker.
- **Ngữ nghĩa** ở [§2](#2-ngữ-nghĩa-cần-biết-trước-khi-gọi) viết tay. Khi đổi hành vi một lệnh, sửa
  đoạn tương ứng trong cùng PR.

Ba gate khác canh cùng bề mặt này:

| Gate | Bắt lỗi gì |
|---|---|
| `npm run check:ipc` (`scripts/check_ipc_arg_case.mjs`) | Tên tham số sai kiểu chữ giữa JS và Rust. `#[tauri::command]` mặc định camelCase, nên `file_path` trong Rust phải được gửi là `filePath`. Lỗi này từng làm save/load **không bao giờ chạy** trong app thật |
| `src-tauri/tests/ipc_registration_tests.rs` | Một `#[tauri::command]` không được đăng ký trong `generate_handler!` sẽ trả `Unknown command` dù mã, kiểu và tài liệu đều đầy đủ |
| Parity `ts-rs` (`cargo test` rồi `git diff --exit-code -- src/types/generated`) | Kiểu payload lệch giữa Rust và TypeScript. Không bao giờ sửa tay file trong `src/types/generated/` |

## 1. Quy ước

- **Tên tham số một từ** khi có thể. Tên không có gạch dưới có cùng một cách viết ở cả hai ngôn ngữ,
  nên nằm ngoài loại lỗi mà `check:ipc` canh.
- **Lỗi trả về là `Err(String)` có lý do**, không phải giá trị mặc định. Cấu hình hỏng bị từ chối,
  không bị kẹp về một giá trị chạy được, vì một phép đo bị kẹp sẽ báo cáo một điều nó không đo.
- **Lệnh chỉ đọc không đổi quỹ đạo mô phỏng.** Lệnh nào ghi vào thế giới (ví dụ `set_lod_focus`,
  `trigger_migration`) phải nói rõ điều đó ở §2.

## 2. Ngữ nghĩa cần biết trước khi gọi

### 2.1 Lưu và nạp

`file_path` giữ tên tham số này vì tương thích IPC. **Nó là *tên* bản lưu, không phải đường dẫn.**
Cả hai lệnh phân giải nó bên trong thư mục dữ liệu của app và từ chối mọi thứ có dạng đường dẫn; trước
đây chúng đưa chuỗi thẳng vào `fs`, nên bất kỳ thứ gì gọi được `invoke` đều ghi được file ở bất kỳ đâu
tiến trình có quyền.

- `save_simulation_state(file_path)` → `bool`: tên khớp `[A-Za-z0-9._-]{1,100}`, chuẩn hoá thành
  `*.json`, ghi dưới `<app data>/saves/` dạng `SnapshotEnvelope` có phiên bản, bằng ghi nguyên tử.
  Dấu phân cách, ký tự ổ đĩa, `..`, tiền tố UNC, NTFS stream và tên thiết bị Windows (`CON`, `NUL`,
  `COM1`…) bị từ chối kèm lý do. Xem `src-tauri/src/commands/save_paths.rs`.
- `load_simulation_state(file_path)` → `bool`: cùng hợp đồng tên, đọc từ `<app data>/saves/`; kiểm
  checksum envelope và migrate schema cũ lên. Hợp đồng snapshot ở
  [`SNAPSHOT_CONTRACT.md`](SNAPSHOT_CONTRACT.md).
- `list_legacy_saves` → `LegacyImportListing`: `{ directory, names }` của `<app data>/legacy-import/`,
  hộp thả cho các bản lưu viết trước khi có giới hạn đường dẫn.
- `import_legacy_save(legacy_name, save_as)` → `String`: đọc một file từ hộp thả, đóng gói lại vào
  envelope hiện tại và ghi vào `saves/` với tên `save_as`. **Chỉ đọc** đối với file cũ.

  Vì sao là hộp thả chứ không phải tham số đường dẫn tuyệt đối: "người dùng chủ động chọn" phải nghĩa
  là *người dùng*, không phải trang web. Một lệnh nhận đường dẫn tuyệt đối sẽ trả lại đúng quyền mà
  việc giới hạn đã lấy đi.

Bản **autosave** (ghi khi thoát, nhận khi khởi động) là một bản lưu thường tên `autosave.json` trong
cùng thư mục, cùng envelope và ghi nguyên tử.

### 2.2 Đo tick trong tiến trình

Bằng chứng vận hành, không bao giờ là trạng thái mô phỏng: một capture không thay đổi được quỹ đạo, vì
không gì nó ghi đi vào `world_checksum`, một luồng RNG hay một snapshot
(`src-tauri/src/core/tick_capture.rs`; gate ở `src-tauri/tests/tick_capture_tests.rs`). Cách dùng:
[`BENCHMARKING.md`](../how-to/BENCHMARKING.md).

- `start_tick_capture(config)` → `()`: bắt đầu một capture, bỏ capture trước. Cấu hình hỏng bị **từ
  chối kèm lý do**.
- `stop_tick_capture` → `()`: dừng và giữ dữ liệu. Idempotent.
- `get_tick_capture_status` → `CaptureStatusReport`.
- `export_tick_capture(file_name)` → `CaptureExport`: phân vị **nearest rank**; khi có tên, ghi nguyên
  tử ra `<app data>/captures/<tên>.json` theo cùng hợp đồng tên của bản lưu.

`ANIMA_TICK_CAPTURE` bật một capture ngay khi engine khởi động, ví dụ
`ANIMA_TICK_CAPTURE=warmup=300,capacity=1800,every=2`.

### 2.3 Phả hệ

Các view chỉ đọc trên phả hệ mà tracker đang giữ; không lệnh nào ghi vào nó. Cả ba lệnh dưới đây
**từ chối** một đồ thị hỏng (chu trình, cạnh mồ côi, id trùng) thay vì trả lời từ nó. Mã ở
`src-tauri/src/evolution/{mrca,newick,simplify}.rs`.

- `get_lineage_mrca(individuals)` → `LineageMrcaPayload` (OSS-072). **Câu trả lời là một tập hợp.**
  Rỗng nghĩa là không có tổ tiên chung, điều bình thường vì mỗi founder là một gốc. Nhiều hơn một phần
  tử (`ambiguous: true`) nghĩa là phả hệ đã tách rồi nhập lại (crossover cho một cá thể hai cha mẹ),
  nên client hiển thị một giá trị phải kiểm `ambiguous` thay vì lấy `ancestors[0]`. Quan hệ tổ tiên
  có tính **phản xạ**. Không có mặc định "quần thể đang sống": tracker chỉ thấy các lần ghi, nên không
  biết ai còn sống. `nearest_edges`/`farthest_edges` đếm **cạnh đồ thị**, không phải số lần sinh sản.
- `export_lineage_newick` → `LineageNewickPayload` (OSS-070): phả hệ dạng rừng Newick, mỗi gốc một cây.
  `dropped_parent_edges > 0` nghĩa là Newick không biểu diễn được mọi cạnh cha (định dạng cây, còn
  crossover cho hai cha mẹ), nên `trees` khi đó là một **view**, không phải toàn bộ phả hệ.
- `get_simplified_lineage(samples)` → `SimplifiedLineagePayload` (OSS-071): phả hệ rút về tổ tiên của
  `samples`, nén các đường đơn nhánh.
- `export_phylogenetic_tree(req)` → `String`: đường xuất Newick thứ hai, đến từ đợt telemetry
  2026-10-22. Roadmap 2026-10-16 §4.1 cụm B ghi tham số `req` của nó **bị bỏ qua**. Hai đường xuất
  Newick cùng tồn tại là một finding mở; xem [§4](#4-finding-mở-trên-bề-mặt-này).

### 2.4 LOD mô phỏng

- `set_lod_focus(focus)` → `()`: **ghi vào thế giới**, đặt tâm của vùng mô phỏng chi tiết (vị trí người
  quan sát). `{ enabled: false, center: [0,0,0] }` là mặc định và đường rollback.
- `get_lod_focus` → `LodFocus`; `get_lod_bands` → `LodBands` (`hot_radius`, `warm_radius`,
  `warm_interval`). Viewport hỏi các ngưỡng này thay vì hard-code.

### 2.5 Telemetry và dữ liệu khoa học

`get_demographic_census`, `get_energy_flux_matrix`, `get_spatial_density_matrix`,
`export_scientific_dataset` đến từ đợt 2026-10-22. **Đọc §4 trước khi dùng số của chúng làm bằng
chứng**: roadmap 2026-10-16 và FINDINGS ghi các giá trị cứng trong đường này.

## 3. Bảng sinh từ mã

<!-- BEGIN GENERATED: scripts/gen_ipc_contract.mjs -->

### Lệnh (37, theo thứ tự trong `generate_handler!`)

| Lệnh | Tham số (JS gửi camelCase) | Trả về | Định nghĩa | Gọi từ `src/` |
|---|---|---|---|---|
| `get_simulation_status` | — | `Result<SimulationStatus, String>` | `src-tauri/src/commands/simulation.rs` | 2 chỗ |
| `toggle_simulation` | — | `Result<bool, String>` | `src-tauri/src/commands/simulation.rs` | 2 chỗ |
| `get_map_elites_grid` | — | `Result<MapElitesGridState, String>` | `src-tauri/src/commands/evolution.rs` | 2 chỗ |
| `update_evolution_settings` | `settings: EvolutionSettings` | `Result<bool, String>` | `src-tauri/src/commands/evolution.rs` | 4 chỗ |
| `toggle_evolution` | — | `Result<bool, String>` | `src-tauri/src/commands/evolution.rs` | 2 chỗ |
| `get_pheromone_grid` | — | `Result<crate::ai::pheromone::PheromoneGridState, String>` | `src-tauri/src/commands/environment.rs` | 2 chỗ |
| `get_environmental_elements` | — | `Result<crate::core::ecs::EnvironmentalState, String>` | `src-tauri/src/commands/environment.rs` | 2 chỗ |
| `get_active_raycasts` | — | `Result<Vec<crate::core::ecs::RaycastTelemetry>, String>` | `src-tauri/src/commands/environment.rs` | 2 chỗ |
| `get_lineage_graph` | — | `Result<LineageGraphPayload, String>` | `src-tauri/src/commands/evolution.rs` | 2 chỗ |
| `get_lineage_mrca` | `individuals: Vec<String>` | `Result<LineageMrcaPayload, String>` | `src-tauri/src/commands/evolution.rs` | không có lời gọi literal |
| `export_lineage_newick` | — | `Result<LineageNewickPayload, String>` | `src-tauri/src/commands/evolution.rs` | không có lời gọi literal |
| `get_simplified_lineage` | `samples: Vec<String>` | `Result<SimplifiedLineagePayload, String>` | `src-tauri/src/commands/evolution.rs` | không có lời gọi literal |
| `get_chronicle_history` | — | `Result<Vec<ChronicleEvent>, String>` | `src-tauri/src/commands/environment.rs` | 1 chỗ |
| `set_lod_focus` | `focus: crate::core::simulation_lod::LodFocus` | `Result<(), String>` | `src-tauri/src/commands/simulation.rs` | không có lời gọi literal |
| `get_lod_focus` | — | `Result<crate::core::simulation_lod::LodFocus, String>` | `src-tauri/src/commands/simulation.rs` | không có lời gọi literal |
| `get_lod_bands` | — | `crate::core::simulation_lod::LodBands` | `src-tauri/src/commands/simulation.rs` | không có lời gọi literal |
| `set_sharding_config` | `config: crate::core::ecs::ShardingConfig` | `Result<(), String>` | `src-tauri/src/commands/networking.rs` | không có lời gọi literal |
| `get_sharding_config` | — | `Result<crate::core::ecs::ShardingConfig, String>` | `src-tauri/src/commands/networking.rs` | không có lời gọi literal |
| `get_migration_handoff_diagnostics` | — | `Result<crate::core::resources::MigrationHandoffSnapshot, String>` | `src-tauri/src/commands/networking.rs` | không có lời gọi literal |
| `trigger_migration` | `target_port: u16` | `Result<(), String>` | `src-tauri/src/commands/networking.rs` | 1 chỗ |
| `get_test_rabbit_state` | — | `tauri::ipc::Response` | `src-tauri/src/commands/simulation.rs` | không có lời gọi literal |
| `save_simulation_state` | `file_path: String` | `Result<bool, String>` | `src-tauri/src/commands/simulation.rs` | 1 chỗ |
| `load_simulation_state` | `file_path: String` | `Result<bool, String>` | `src-tauri/src/commands/simulation.rs` | 1 chỗ |
| `get_terrain_map` | — | `Result<TerrainMapState, String>` | `src-tauri/src/commands/environment.rs` | 1 chỗ |
| `get_ecosystem_state` | — | `Result<EcosystemState, String>` | `src-tauri/src/commands/environment.rs` | 4 chỗ |
| `save_world_artifact` | `bytes: Vec<u8>` | `Result<String, String>` | `src-tauri/src/commands/environment.rs` | 1 chỗ |
| `list_legacy_saves` | — | `Result<LegacyImportListing, String>` | `src-tauri/src/commands/simulation.rs` | 1 chỗ |
| `import_legacy_save` | `legacy_name: String`, `save_as: String` | `Result<String, String>` | `src-tauri/src/commands/simulation.rs` | 1 chỗ |
| `start_tick_capture` | `config: CaptureConfig` | `Result<(), String>` | `src-tauri/src/commands/capture.rs` | không có lời gọi literal |
| `stop_tick_capture` | — | `Result<(), String>` | `src-tauri/src/commands/capture.rs` | không có lời gọi literal |
| `get_tick_capture_status` | — | `Result<CaptureStatusReport, String>` | `src-tauri/src/commands/capture.rs` | không có lời gọi literal |
| `export_tick_capture` | `file_name: Option<String>` | `Result<CaptureExport, String>` | `src-tauri/src/commands/capture.rs` | không có lời gọi literal |
| `get_demographic_census` | — | `Result<DemographicCensusState, String>` | `src-tauri/src/commands/environment.rs` | 1 chỗ |
| `get_energy_flux_matrix` | — | `Result<EnergyFluxReport, String>` | `src-tauri/src/commands/environment.rs` | 1 chỗ |
| `get_spatial_density_matrix` | `width: Option<u32>`, `height: Option<u32>` | `Result<SpatialDensityReport, String>` | `src-tauri/src/commands/environment.rs` | 1 chỗ |
| `export_phylogenetic_tree` | `req: Option<NewickExportRequest>` | `Result<String, String>` | `src-tauri/src/commands/evolution.rs` | 1 chỗ |
| `export_scientific_dataset` | `req: ScientificExportRequest` | `Result<ScientificExportResponse, String>` | `src-tauri/src/commands/simulation.rs` | 1 chỗ |

### Lời gọi từ frontend tới lệnh không tồn tại (2)

| Lệnh | Gọi tại |
|---|---|
| `dispatch_causal_intervention` | `src/components/ResearchDashboard/CausalInterventionPanel.tsx:115` |
| `get_causal_branch_tree` | `src/components/ResearchDashboard/CheckpointStudioPanel.tsx:54` |

### Sự kiện Rust → webview (7)

| Sự kiện | Phát tại | `listen` literal trong `src/` |
|---|---|---|
| `chronicle-event` | `src-tauri/src/core/simulation_loop.rs` | không có |
| `combat-event` | `src-tauri/src/core/emit.rs` | không có |
| `map-elites-update` | `src-tauri/src/core/simulation_loop.rs` | không có |
| `migration-event` | `src-tauri/src/core/networking_systems.rs` | không có |
| `pheromone-update` | `src-tauri/src/core/emit.rs` | 1 chỗ |
| `raycast-update` | `src-tauri/src/core/emit.rs` | 1 chỗ |
| `simulation-tick` | `src-tauri/src/core/emit.rs` | 1 chỗ |

<!-- END GENERATED -->

## 4. Finding mở trên bề mặt này

Các mục dưới đây là **giả thuyết hoặc quan sát đọc từ mã**, chưa phải phép đo. Mỗi mục có thẻ xử lý
trong [kế hoạch swarm](../ai/planning/2026-11-05-swarm-master-plan.md).

| Finding | Bằng chứng | Thẻ |
|---|---|---|
| Frontend gọi hai lệnh không tồn tại | Bảng "Lời gọi từ frontend tới lệnh không tồn tại" ở §3 (do script tự tìm) | G4 |
| `export_scientific_dataset` ghi `RunProvenance::new(42, …)`: seed cứng, không phải seed của run | `src-tauri/src/commands/simulation.rs` (tìm `RunProvenance::new`) | G3 |
| `get_energy_flux_matrix` dùng tỉ lệ cố định 70/30 và sinh khối mặt trời hằng `10000.0` | `src-tauri/src/core/telemetry_export.rs` (`generate_energy_flux_report`) | G3 |
| Hai đường xuất Newick (`export_lineage_newick`, `export_phylogenetic_tree`); tham số `req` của đường thứ hai bị bỏ qua | §2.3 | G3 |
| Nhiều lệnh đã đăng ký chưa có lời gọi literal nào trong `src/` | Cột cuối của bảng lệnh ở §3 | — (không phải lỗi; ghi để biết bề mặt nào chưa có UI) |
