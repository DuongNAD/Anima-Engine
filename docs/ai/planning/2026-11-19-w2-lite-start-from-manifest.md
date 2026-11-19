---
phase: planning
feature: real-world-and-bodies
title: Thẻ W2-lite — khởi động desktop app từ manifest thí nghiệm (viewer only, prototype, to be replaced)
description: Cho phép desktop app khởi động trực tiếp từ nội dung JSON của manifest thí nghiệm qua lệnh IPC start_from_manifest(manifestJson), dùng chung hàm khởi tạo và luật thế giới với LiveExperimentAdapter dưới chế độ viewer-only (prototype, to be replaced).
status: completed
owner: simulation-architecture
last_reviewed: 2026-11-19
---

# Thẻ W2-lite — khởi động desktop app từ manifest thí nghiệm (`prototype, to be replaced`)

> **Prototype, to be replaced** (theo chỉ đạo `PROTOTYPE-FIRST` ngày 2026-11-19 và thiết kế `W2` trong `docs/ai/design/2026-11-19-feature-real-world-and-bodies.md` trên PR #150; bước `W1` của `ADR-0010` sẽ đến sau). **Chế độ chỉ xem (viewer only):** các luồng nền của desktop app (`learn`, `inference`, `evo`, `emit`, `net`) vẫn chạy như thường lệ nên quỹ đạo khi chạy trong app có thể khác lượt chạy headless qua `LiveExperimentAdapter`; màn hình và tài liệu này tuyệt đối **không** tuyên bố tái lập kết quả thí nghiệm.

## 1. Các thay đổi đã thực hiện

1. **Hàm khởi tạo & luật thế giới dùng chung (`src-tauri/src/core/live_experiment.rs`):**
   - Tách `insert_policy_resources` và hàm `pub fn apply_live_genesis_and_policies(world: &mut World, seed: u64, config: &LiveWorldConfig, bounds: MapBounds)` từ `build_live_world`.
   - Hàm mới cài `SimRng::from_seed(seed)`, toàn bộ 10 resource chính sách từ manifest (`BrainPolicy`, `MortalityPolicy`, `ReproductionPolicy`, `MutationPolicy`, `PreyCapPolicy`, `PlantReservePolicy`, `LocomotionPolicy`, `FitnessRuleSetting`, `LiveFoodCap`, `PredatorIncomePolicy`), `FoodSpawnSettings`, tỉ lệ sức chứa đồng cỏ `scale_pasture_capacity`, và `genesis` (founders, cây, hồ). `build_live_world` gọi trực tiếp hàm này (nhánh khôi phục giữ nguyên hành vi bit-for-bit).
2. **Hàng đợi manifest trên `SimulationEngine` (`src-tauri/src/core/simulation_loop.rs`):**
   - Thêm `pub struct PendingManifest` (`name`, `seed`, `config`) và khe `pending_manifest: Arc<Mutex<Option<PendingManifest>>>` trên `SimulationEngine`.
   - `SimulationEngine::start` lấy `pending_manifest` khi không có `pending_load_state`, dùng `manifest.seed` làm `startup_run_seed`, gọi `apply_live_genesis_and_policies` thay cho nhánh genesis mặc định, và đăng ký gốc phả hệ vào `lineage_tracker`. Nhánh khởi động cũ (`pending_manifest = None`) và mọi luồng/hệ thống khác giữ nguyên hoàn toàn.
3. **Lệnh IPC `start_from_manifest` (`src-tauri/src/commands/simulation.rs`, `src-tauri/src/lib.rs`, `docs/reference/IPC_CONTRACT.md`):**
   - `parse_and_validate_manifest_for_viewer` đọc JSON văn bản (giới hạn 4 MiB, không nhận đường dẫn), kiểm tra với `ObservableRegistry::live_default()`, `manifest.validate(&registry)`, `LiveWorldConfig::from_initial_conditions`, `ensure_observables_supported`, đồng thời từ chối `laws.exotic_energy`, `exotic_interventions`, và `live.drift_control = 1.0` đúng như `LiveExperimentAdapter`.
   - `start_from_manifest(manifestJson: string)` dừng lượt chạy hiện tại nếu có, xoá `pending_load_state`, đặt `pending_manifest`, khởi động `SimulationEngine` và trả tên manifest. Nếu manifest không hợp lệ, mô phỏng đang chạy (nếu có) **không** bị dừng.
   - Đăng ký trong `tauri::generate_handler!` và bổ sung dòng tương ứng trong [`docs/reference/IPC_CONTRACT.md`](../../reference/IPC_CONTRACT.md).
4. **Điều khiển UI `ExperimentWorldControl` (`src/components/ExperimentWorldControl.tsx`, `src/App.tsx`):**
   - Thêm `ExperimentWorldControl` (đánh dấu `"prototype, to be replaced"`), đóng gói tĩnh manifest nhánh chọn lọc `SEL2-P selected` (100 founders, 16 hồ rải lưới `live.lake_layout = 1`, `live.resource_capacity_scale = 0.63`, `live.prey_cap = 320`, `live.dehydration_ticks = 960`, `seed = 780001`).
   - Gắn trực tiếp vào hàng điều khiển mô phỏng trong `src/App.tsx` cạnh `toggle-simulation-button` (theo uỷ quyền mở rộng `UNBLOCK W2-lite` ngày 2026-11-19 sau khi xác minh `SimulationControls.tsx` không có importer), đồng bộ trạng thái `running` khi mở thành công, hiển thị tên manifest đang chạy kèm cảnh báo **Viewer only**, và hiển thị nguyên văn lỗi preflight từ backend khi bị từ chối.

## 2. Xác minh

- **Rust headless (`src-tauri/tests/start_from_manifest_tests.rs`):**
  - So sánh trực tiếp thế giới ở `tick 0` trên cùng máy giữa `apply_live_genesis_and_policies` và `LiveExperimentAdapter::from_manifest` (cho cả manifest `SEL2-P selected` lẫn manifest bật toàn bộ 10 resource chính sách): khớp 100% tập agent, vị trí/loại hồ, vị trí cây, `ResourceField` (`r`, `r_max`), `EcosystemBiomass`, `FoodSpawnSettings`, `SimRng`, cả 10 policy resource và `world_checksum` cùng máy (không ghim checksum cứng).
  - Chạy `SimulationEngine::start::<tauri::test::MockRuntime>` từ JSON `SEL2-P selected` qua `start_from_manifest_with_engine`: phát hành `seed = 780001`, 16 hồ lưới khớp toạ độ `LiveExperimentAdapter`, 8 cây, `detritus > 1000` (tỉ lệ sức chứa `0.63`), quần thể prey tăng qua ngưỡng `100` ở priming tick (`<= 320`), và đối chứng `live.reproduction = 0.0` giữ đúng `100` prey / `300` segment.
  - Xác nhận từ chối nguyên văn cho năng lượng ngoại lai, can thiệp ngoại lai, khoá khởi tạo lạ (`"plants"`), `lake_layout = 1` với `lakes = 0`, `drift_control = 1`, observable `live.deaths` khi `live.mortality = 0`, và JSON lỗi; lỗi không làm dừng mô phỏng đang chạy.
  - Xác nhận nhánh cũ (`pending_manifest = None`) vẫn sinh `<= 5` hồ, `3` predator và kết quả giống hệt nhau giữa 2 lần khởi động.
- **Tương thích `LiveExperimentAdapter` (`live_experiment_tests`, `lake_layout_tests`):** giữ nguyên không đổi.
- **Frontend (`tests/frontend/experimentWorldControl.test.tsx`):** render `<App />`, kiểm tra nút gắn cạnh `toggle-simulation-button`, không gọi `start_from_manifest` khi mount, gọi IPC với khoá camelCase `manifestJson`, đồng bộ nút dừng, ẩn nhãn khi dừng, và hiển thị nguyên văn lỗi preflight (`3 passed`).
- **Tổng cổng đã chạy sau khi sửa lần cuối:** `cargo fmt --check`, cả 2 profile `clippy -D warnings`, `cargo test --lib export_bindings` (50/50, `src/types/generated` không lệch), `cargo test --features desktop` (`159` target, `1532 passed`, `0 failed`, `2 ignored` theo allow-list, `check_test_targets.mjs` pass), `verify_dataflow.py` (`16` edge khớp), `npm run lint` (`0` cảnh báo), `npm run typecheck:tests`, `npm run test` (`14/14` file, `109/109`), `npm run test:frontend` (`69/69` file, `703/703`), `npm run build` (`main = 272.2 KiB / 320 KiB`, tổng `1893.7 / 2000 KiB`), `check:bundle`, `check:bundle-closure`, `check:csp`, `check:docs` (`36` lệnh, `7` event, `0` sai khác), `check:ipc`, `check:text-hygiene`, `check:root-hygiene`.
