---
title: Đường cong mở rộng hiệu năng headless theo số lượng agent (M3)
status: active
owner: maintainers
last_reviewed: 2026-11-18
review_cycle: per-release
---

# Đường cong mở rộng hiệu năng headless (thẻ M3)

**Câu hỏi.** Chi phí thời gian mỗi tick của lịch trình mô phỏng thật ([`core::simulation_schedule::build_tick_schedule`](../../../src-tauri/src/core/simulation_schedule.rs) chạy qua [`LiveExperimentAdapter`](../../../src-tauri/src/core/live_experiment.rs)) tăng như thế nào khi số cá thể `N` tăng qua các mốc **`10`**, **`100`**, **`1.000`**, **`4.096`** và **`5.000`**?

> **Cảnh báo phạm vi đo (bắt buộc đọc trước số liệu):** Mọi con số ở đây là **cận dưới headless**, **KHÔNG phải thời gian khung hình của ứng dụng desktop**:
> - Chạy lịch mô phỏng Bevy đơn luồng tất định trong tiến trình benchmark (`cargo bench --bench live_schedule_scaling`, profile `--release`).
> - **Không** có cửa sổ Tauri, **không** có renderer WebGL/Three.js, **không** có tuần tự hoá/gửi sự kiện IPC, và **không** có các luồng học/tiến hoá/emitting nền của app.
> - Đồng thời dùng tập khoá manifest tối thiểu để **cố định sĩ số** suốt phép đo (`live.mortality = 0.0`, `live.reproduction = 0.0`) thay vì để sinh/tử làm `N` trôi giữa các mẫu Criterion.

---

## Thiết kế đo & tham số khai báo

- **Commit gốc (`origin/feat/ecological-selection`):** `f4614665f7ccf527a6a93a5b0fcfd39aa4e3f948` (nhánh `win/m3-scaling`).
- **Tập khoá `InitialConditionSet` dùng cho mọi mốc `N`:**
  - `live.founders`: `n.min(4096) as f64` (`LiveWorldConfig::from_initial_conditions` chặn trần `live.founders <= 4096.0` tại `src-tauri/src/core/live_experiment.rs:402`).
  - **Xử lý mốc `N = 5.000` không sửa mã nguồn engine:** adapter khởi tạo chuẩn ở `live.founders = 4096.0` (gồm `1.229` thú săn mồi + `2.867` con mồi), sau đó **trước tick 1** (`adapter.tick() == 0`), harness mượn `adapter.world()` để spawn thêm `904` cá thể sáng lập (`4096..5000`, gồm `271` `Predator` và `633` `Prey`, giữ đúng tỷ lệ `30%` toàn quần thể `1.500 / 5.000`) với cùng `MorphologyGenotype` `3` node và đặt trên dải `z_fraction = 0.45` qua `decode_genotype`. Cách làm này giữ nguyên 100% lịch `build_tick_schedule` nhưng không làm thay đổi mã nguồn `src-tauri/src/`.
  - `live.mortality = 0.0`, `live.reproduction = 0.0`: khoá cố định số cá thể; harness khẳng định `live.agent_count == N` cả **trước** lẫn **sau** khi Criterion chạy từng mốc.
  - `live.locomotion = 1.0`, `live.drive_floor = 0.5`, `live.thirst_steering = 1.0`: kích hoạt thật `steering_drive_system` và va chạm ăn/uống mỗi tick thay vì đo nhánh thoát sớm.
  - `live.predator_fraction = 0.30`, `live.trees = 8.0`, `live.lakes = 2.0`, `live.food_cap = 50.0`; địa hình mặc định `256 × 256` từ `MapSettings::default()`; `WorldLawSet::baseline()`, `seed = 20261118`.
- **Khởi động ấm (warm-up) & lấy mẫu Criterion (`src-tauri/benches/live_schedule_scaling.rs`):**
  1. Trước khi Criterion bắt đầu, mỗi thế giới chạy trước **`20` tick khởi động thế giới** qua `adapter.run_schedule_once()` để `EnergyLedger::baseline` khoá xong sau tick 1, `SpatialHashGrid` được điền đầy, và bước nhảy nhịp `REGROWTH_STRIDE = 4` ổn định.
  2. Criterion khởi động ấm **`3 s`** (`group.warm_up_time(Duration::from_secs(3))`) trước khi ghi mẫu.
  3. Cỡ mẫu & thời gian đo: `50` mẫu / `8 s` cho `N ∈ {10, 100, 1.000}`; `20` mẫu / `12 s` cho `N ∈ {4.096, 5.000}`.
- **Quy tắc trích xuất số liệu (`STATE_OF_THE_PROJECT.md` §3.0):** Criterion in ra stderr giá trị *hệ số góc hồi quy (slope)* chứ không phải trung vị. Toàn bộ **`p50`** trong bảng dưới đây được đọc trực tiếp từ **`median.point_estimate`** (kèm khoảng tin cậy `95%` của trung vị và `mean.point_estimate`) trong `target/criterion/live_schedule_scaling/<N>/new/estimates.json`, còn **`p95`** được tính trực tiếp từ mảng `times[i] / iters[i]` trong `sample.json` của chính lượt chạy đó.

---

## Trạng thái thực thi & bảng kết quả

*(Đang chuẩn bị chạy `cargo bench --bench live_schedule_scaling` trên Windows 11 x64 — bảng `p50`/`p95`, cấu hình máy từ lệnh và đầu ra Criterion sẽ được điền sau khi benchmark hoàn tất.)*
