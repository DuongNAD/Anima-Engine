---
phase: planning
feature: snapshot-restore-fidelity
title: Thẻ F045 — khôi phục checkpoint thế giới sống trùng bit với lượt chạy liền mạch
description: Sửa ba lệch trạng thái khi save/restore thế giới sống (IndexedInGrid cho thực thể sinh sau rebuild_spatial_grid_system, AgentSex trên SerializedAgent, và NextNodeId trên SavedSimulationState) mà không đổi quỹ đạo chạy liền mạch hay tăng LIVE_MODEL_VERSION / SCHEMA_VERSION.
status: completed
owner: simulation-architecture
last_reviewed: 2026-11-18
---

# Thẻ F045 — khôi phục checkpoint thế giới sống trùng bit với lượt chạy liền mạch

> **Thẻ sửa lỗi khôi phục checkpoint (`Path I`), không thay đổi quỹ đạo chạy liền mạch.** Vì mọi lượt
> chạy không ngắt quãng giữ nguyên từng bit, `LIVE_MODEL_VERSION` (ADR-0008 §5) và `SCHEMA_VERSION`
> (giữ tương thích đọc ngược bằng `#[serde(default)]`, tránh xung đột không cần thiết với PR `#127` nâng
> schema 8) đều không thay đổi.

## Nguyên nhân gốc và ba điểm sửa

Phép đo nền trên `origin/feat/ecological-selection` (`2969e16ca33309c284c5159303a7d645c329acca`,
thế giới nền S6, hạt giống `3`) cho thấy sai khác khôi phục chia thành hai cơ chế độc lập cùng một
lỗi tiềm ẩn thứ ba về bộ đếm nút hình thái:

1. **Ranh giới lập chỉ mục `SpatialHashGrid` (`K = 1 -> 2`):** trong lịch chạy chính quy
   (`simulation_schedule.rs`), `rebuild_spatial_grid_system` chạy trước `spawn_food_system`,
   `seed_dropping_system` và `autonomous_reproduction_system`. Trước khi sửa, `populate_spatial_grid`
   khi khôi phục quét toàn bộ ECS World và lập chỉ mục luôn các con non/thức ăn vừa sinh sau pha
   `rebuild_spatial_grid_system` ở tick `K` (240 mục trong lưới so với 100 ở lượt liền mạch tại
   tick `1`).
   - **Sửa (Fix 1):** thêm component `IndexedInGrid(pub bool)` (`src-tauri/src/physics/spatial.rs`),
     khởi tạo `false` khi sinh collider (`decode_genotype`, `spawn_food_system`,
     `seed_dropping_system`, `live_experiment::genesis`) và **lật tại chỗ** `.0 = true` ngay trong
     lượt duyệt 2 của `rebuild_spatial_grid_system` (không `insert`/`remove` component trong tick nên
     không dịch chuyển archetype và 0 cấp phát heap). `SavedSimulationState` lưu `unindexed_colliders:
     Option<SavedUnindexedColliders>` (`#[serde(default)]`); khi khôi phục,
     `populate_restored_spatial_grid` đồng bộ lại `IndexedInGrid` và chỉ nạp vào `SpatialHashGrid`
     những thực thể đã được lập chỉ mục trước đó (cả ở `LiveExperimentAdapter` lẫn
     `SimulationEngine`).
2. **Tách bảng ECS theo `AgentSex` (`K = 600 -> 1.200`):** trong thế giới gốc, founder được sinh từ
   `live_experiment::genesis` không mang `AgentSex` (`A_founder`), còn con sinh trong mô phỏng mang
   `AgentSex` (`A_offspring`). Trước khi sửa, `SerializedAgent` bỏ qua `AgentSex` nên khi khôi phục
   toàn bộ quần thể gộp vào một bảng duy nhất; các ca chết (`swap_remove`) từ sau tick `600` làm
   đảo thứ tự duyệt `Agent`/`Prey` qua `herbivore_grazing_system` và các hệ không sắp thứ tự, dẫn
   tới rẽ nhánh ở tick `859` (`0xe20bddee` so với `0x38bbb839`).
   - **Sửa (Fix 2):** thêm `#[serde(default)] pub sex: Option<AgentSex>` trên `SerializedAgent`,
     ghi ở cả `serialize_single_agent` lẫn `serialize_world_state`, và trong `spawn_serialized_agent`
     chỉ chèn `AgentSex` khi `Some` để giữ nguyên trạng thái founder / save cũ (`None`).
3. **Bộ đếm `NextNodeId` bị đặt lại về `3` (`live_experiment.rs`):** `install_runtime_resources`
   luôn chèn `NextNodeId(3)` kể cả khi khôi phục từ checkpoint, khiến đột biến `AddNode` sau khôi
   phục có thể cấp trùng id nút hình thái.
   - **Sửa (Fix 3):** thêm `#[serde(default)] pub next_node_id: Option<u32>` vào
     `SavedSimulationState`, ghi giá trị thật từ `World` (kể cả khi `NextNodeId > max_saved_node_id + 1`
     do đột biến thất bại hoặc dòng dõi đã tuyệt chủng), và dùng `restored_next_node_id` trong cả
     `LiveExperimentAdapter::install_runtime_resources` lẫn `simulation_loop.rs`. Với save cũ
     (`None`), hàm rơi về `(max_saved_node_id + 1).max(3)` trên cả `agents` lẫn `dormant_agents`.

## Số đo kiểm chứng (`f045_restore_equivalence_tests`)

Chạy trên `macOS arm64` (`origin/feat/ecological-selection` = `2969e16ca33309c284c5159303a7d645c329acca`,
cấu hình nền S6, `seed = 3`):

| Điều kiện | Trước khi sửa (`2969e16`) | Sau khi sửa (`Path I`) |
|---|---|---|
| Khôi phục `1 -> 2` (lưới + checksum) | `100` vs `240`; `0xf5876d6e` vs `0x109047fe` (**FAIL**) | `100 == 100`; `0xf5876d6e == 0xf5876d6e` (**PASS**) |
| Khôi phục `600 -> 1.200` (từng tick) | Lệch đầu ở tick `859`; cuối `0xe20bddee` vs `0x38bbb839` (**FAIL**) | `first_divergence = None`; cuối `0xe20bddee == 0xe20bddee` (**PASS**) |
| Đột biến `AddNode` + giữ `NextNodeId` | Đặt lại `NextNodeId(3)` khi `from_snapshot` | Tại tick `394` (`pre_max_nodes = 4`), `expected = 6 == restored = 6`, không trùng id và khớp tiếp 50 tick (**PASS**) |
| Save cũ không có trường mới | Chưa có các trường F-045 | Nạp JSON bỏ `sex`/`next_node_id`/`unindexed_colliders` qua `SnapshotEnvelope`, fallback `NextNodeId = 6` (**PASS**) |
| Quỹ đạo liền mạch (tick `1`, `2`, `600`, `1.200`) | `0x30ea8bbf`, `0xf5876d6e`, `0xe6d29cc5`, `0xe20bddee` | `0x30ea8bbf`, `0xf5876d6e`, `0xe6d29cc5`, `0xe20bddee` (trùng bit) |
