---
phase: planning
feature: ecological-selection
title: Thẻ S2 — thêm live.mean_energy_integral và khối lượng cơ thể theo vùng Đông/Tây
description: Bổ sung ba observable nối vào cuối LIVE_EXTENDED_OBSERVABLE_IDS (live.mean_energy_integral, live.trait_mass_mean_west, live.trait_mass_mean_east), giữ nguyên LIVE_OBSERVABLE_IDS và quỹ đạo mô phỏng, chịu được ngã rẽ checkpoint qua đĩa
status: completed
owner: simulation-architecture
last_reviewed: 2026-11-19
---

# Thẻ S2 — các observable cho SEL2-P, E2R và E3

## 1. Ba observable mới

Cả ba được nối vào cuối `LIVE_EXTENDED_OBSERVABLE_IDS` (`12 -> 15` mục; `LIVE_OBSERVABLE_IDS` giữ nguyên ở 12 mục) và đăng ký trong `ObservableRegistry::live_default()` với `Aggregation::Instant` và `ConservationRole::None`:

| ID | Đơn vị | Cadence | Nghĩa vật lý | Giá trị khi vắng cá thể |
|---|---|---|---|---|
| `live.mean_energy_integral` | `EU·tick` | `physics` (`1` tick) | Tổng tích luỹ trên mọi tick đã mô phỏng của `live.mean_agent_energy` | Cộng `0.0` ở mỗi tick không còn cá thể sống (giữ nguyên phần tích luỹ trước khi tuyệt chủng, `0.0` ở tick 0; không dùng sentinel) |
| `live.trait_mass_mean_west` | `EU` | `ecology` (`60` tick) | Khối lượng cơ thể trung bình (`AgentGenotype.0.total_mass()`) ở nửa Tây (`col in 0..=127`, khớp `Region::Rect{0,0,127,255}`) | `0.0` (qua `mass_statistics(&[])`, đồng bộ với `live.trait_mass_mean`) |
| `live.trait_mass_mean_east` | `EU` | `ecology` (`60` tick) | Khối lượng cơ thể trung bình ở nửa Đông (`col in 128..=255`, khớp `Region::Rect{128,0,255,255}`) | `0.0` |

## 2. Quyết định thiết kế

1. **Bộ tích luỹ nằm trên `LiveExperimentAdapter`, ngoài `build_tick_schedule`**: `LiveExperimentAdapter` giữ một trường `mean_energy_integral: f64`, được cập nhật đúng một lần sau mỗi tick trong `LiveExperimentAdapter::step` và `run_schedule_once` (cùng giá trị trung bình mà `live.mean_agent_energy` đọc). Giá trị này được phản chiếu vào `LiveExperimentState::mean_energy_integral` (`#[serde(default)]`) để:
   - `read_live_observables(&mut World)` chỉ đọc thuần tuý (không sửa trạng thái nên đọc từng tick, theo cửa sổ hay không đọc ở giữa lượt đều cho cùng `world_checksum` và cùng tích phân);
   - đường lưu/nạp `snapshot()` / `from_snapshot()` qua `SavedSimulationState::experiment` khép kín hoàn toàn trong `src-tauri/src/core/live_experiment.rs` mà không phải sửa `simulation_state.rs` hay `world_checksum`.
2. **Ánh xạ toạ độ thế giới → nửa Tây/Đông**: dùng đúng công thức chỉ số cột của `ResourceField::cell_index` và `TerrainMap::get_map_indices`: `u = ((pos.x - min_x) / (max_x - min_x)).clamp(0.0, 1.0)`, `col = ((u * width as f32) as usize).min(width - 1)`. Nửa Tây là `col < width / 2` (`0..=127` trên lưới 256×256) và nửa Đông là `col >= width / 2` (`128..=255`).
3. **Time-to-floor giữ làm đại lượng dẫn xuất** trong script phân tích hậu kỳ, vì ngưỡng sàn phụ thuộc vào từng giao thức thực nghiệm và các tick trước khi chạm sàn (hoặc cả lượt không chạm sàn) đều bị cắt phải (right-censored); mọi mã hoá từng tick đều buộc phải gán sentinel làm nhiễu hiệu số ghép cặp hoặc nhập nhằng với thời gian sống sót.
4. **Hỗ trợ đối chứng drift**: cả ba observable được phát vô điều kiện cho cả nhánh selected và nhánh replay `drift_control`, nên ghép cặp cửa sổ không làm phình `treatment_only`.

## 3. Kiểm chứng

- Kiểm thử riêng cho S2: `src-tauri/tests/s2_observables_tests.rs` (không nhiễu loạn theo nhịp lấy mẫu, khép kín ngã rẽ checkpoint qua đĩa khi không lấy mẫu giữa chừng, ranh giới cột 127/128 của lưới 256×256 và nửa rỗng, quần thể 0 cá thể từ đầu và tuyệt chủng giữa chừng, chạy ghép cặp `run_live_drift_pair`), mở rộng `observables_tests.rs` và `live_experiment_tests.rs`.
- Chi tiết chạy lệnh được ghi trong báo cáo tại hub (`test_runs/anima-case_S2_report.md` khi mở khoá đủ điều kiện nộp).
