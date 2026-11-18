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

**Tóm tắt kết quả (cận dưới headless, profile `--release`):** Cả 5 mốc sĩ số **`10`**, **`100`**, **`1.000`**, **`4.096`** và **`5.000`** đều chạy trọn vẹn (`BENCH_EXIT=0`, không trôi `live.agent_count`). Từ **`100`** (`p50 = 0,331 ms`) đến **`1.000`** (`p50 = 5,643 ms`), lịch trình headless còn dưới mốc `16,67 ms` (`60 Hz`), nhưng từ **`1.000`** lên **`4.096`** chi phí mỗi agent tăng vọt `3,37×` (`5,64 -> 19,02 µs/agent/tick`), đẩy `p50` lên **`77,904 ms`** ở `N = 4.096` và **`89,973 ms`** (`p95 = 96,010 ms`) ở `N = 5.000`. **Nhắc lại:** đây là **cận dưới headless**, **KHÔNG phải thời gian khung hình desktop** — app thật còn trả thêm chi phí render WebGL, IPC và các luồng phụ.

### Cấu hình máy & chuỗi công cụ đo thực tế

| Thành phần | Giá trị đo từ lệnh (`Get-CimInstance` / `rustc -V` / `cargo -V`) |
|---|---|
| CPU | `Intel(R) Core(TM) i5-14600KF` (`14` nhân vật lý / `20` luồng logic, `MaxClockSpeed = 3500 MHz`) |
| Bộ nhớ RAM | `51.318.800.384` byte (`47,8 GiB`) |
| Hệ điều hành | `Microsoft Windows 11 Pro` (`Version: 10.0.26300`, `BuildNumber: 26300`) |
| Chuỗi công cụ Rust | `rustc 1.98.1 (48a229cea 2026-09-01)`, `cargo 1.98.1 (797e8a9bc 2026-08-05)`, `CARGO_BUILD_JOBS=6` |
| Commit gốc đo | `f4614665f7ccf527a6a93a5b0fcfd39aa4e3f948` (nhánh `win/m3-scaling`, PR nháp `#138`) |

### Các lệnh thực thi & kiểm tra cổng

| Cổng / Lần chạy | Lệnh thực thi nguyên văn | Kết quả | Thời gian |
|---|---|---|---|
| Benchmark release | `set CARGO_BUILD_JOBS=6&& cargo bench --bench live_schedule_scaling` | `EXIT=0` (`5/5` mốc hoàn tất) | `423,32 s` (build `5m 24s` + đo) |
| Format Rust | `cargo fmt --all -- --check` | `EXIT=0` (sạch) | `2,88 s` |
| Clippy (desktop) | `cargo clippy --all-targets --features desktop -- -D warnings` | `EXIT=0` (0 warning) | `171,55 s` |
| Clippy (default) | `cargo clippy --all-targets --no-default-features -- -D warnings` | `EXIT=0` (0 warning) | `72,92 s` |
| Feature split gate | `cargo tree --no-default-features -e normal` | `EXIT=0` (`criterion` = `0` kết quả) | `0,68 s` |

---

### Bảng đường cong mở rộng (`p50` từ `estimates.json`, `p95` từ `sample.json`)

- Nguồn `p50` và `mean`: `src-tauri/target/criterion/live_schedule_scaling/<N>/new/estimates.json` (`median.point_estimate`, `median.confidence_interval`, `mean.point_estimate`; cột `slope` in ra stderr chỉ để đối chiếu quy tắc `STATE_OF_THE_PROJECT.md` §3.0).
- Nguồn `p95`: tính trực tiếp từ mảng `per_iter[i] = times[i] / iters[i]` trong `sample.json` theo chuẩn **nearest-rank** (`chỉ số ceil(0.95 × K) - 1`, tức mẫu lớn thứ `48/50` khi `K = 50` và `19/20` khi `K = 20`); cột ghi chú kèm giá trị nội suy tuyến tính R-7 cho `K = 20`.

| `N` (agents) | Mẫu `K` (`iters`) | `p50` (ms/tick) | Khoảng tin cậy `95%` của `p50` (ms) | `p95` nearest-rank (ms/tick) | `mean` (ms/tick) | `slope` stderr (ms/tick) | Chi phí `p50` / agent (µs) | Bội số so với `16,67 ms` (`60 Hz`) |
|---:|---:|---:|---|---:|---:|---:|---:|---:|
| `10` | `50` (`73.950`) | **`0,1062`** | `[0,1047 ; 0,1071]` | **`0,1124`** (`R-7: 0,1119`) | `0,1054` | `0,1023` | `10,62 µs` | `0,006×` |
| `100` | `50` (`26.775`) | **`0,3307`** | `[0,3289 ; 0,3330]` | **`0,3962`** (`R-7: 0,3922`) | `0,3378` | `0,3447` | `3,31 µs` | `0,020×` |
| `1.000` | `50` (`2.550`) | **`5,6432`** | `[5,6006 ; 5,6894]` | **`6,6193`** (`R-7: 6,5069`) | `5,7242` | `5,7524` | `5,64 µs` | `0,339×` |
| `4.096` | `20` (`210`) | **`77,9041`** | `[73,4105 ; 79,6636]` | **`90,9746`** (`R-7: 91,1495`) | `78,0158` | `74,6438` | `19,02 µs` | `4,673×` |
| `5.000` | `20` (`210`) | **`89,9732`** | `[86,2278 ; 92,1052]` | **`96,0098`** (`R-7: 96,0487`) | `89,9097` | `92,8661` | `17,99 µs` | `5,397×` |

Biên độ cực trị `min / max` từng mẫu từ `sample.json`:
- `N = 10`: `[0,0976 ; 0,1212] ms`
- `N = 100`: `[0,3151 ; 0,4130] ms`
- `N = 1.000`: `[5,3887 ; 6,7210] ms`
- `N = 4.096`: `[71,0958 ; 94,4730] ms`
- `N = 5.000`: `[84,7971 ; 96,7870] ms`

### Đọc đường cong mở rộng & lưu ý kỹ thuật

1. **Chế độ chi phí nền (`N = 10 -> 100`):** Tăng `10×` cá thể nhưng `p50` chỉ tăng `3,11×` (`0,1062 -> 0,3307 ms`) và chi phí trên mỗi cá thể giảm từ `10,62` xuống `3,31 µs/agent`, vì ở sĩ số thấp chi phí cố định của `ResourceField` (`256 × 256` ô), clock và tổng hợp Bevy schedule chiếm đa số.
2. **Đoạn siêu tuyến tính (`N = 1.000 -> 4.096 -> 5.000`):** Từ `1.000` (`5,6432 ms`) lên `4.096` (`77,9041 ms`), sĩ số tăng `4,096×` trong khi `p50` tăng **`13,80×`** (hệ số mũ hiệu dụng `~1,86` trên đoạn này) khi tương tác gần trong `SpatialHashGrid`, raycast cảm biến, và inference gộp lô chiếm ưu thế; từ `4.096` lên `5.000` (`1,221×` sĩ số), `p50` tăng `1,155×` (`77,9041 -> 89,9732 ms`).
3. **Ngưỡng ngân sách `16,67 ms` (`60 Hz`):** Ngay cả ở **cận dưới headless** (chưa tính render, IPC hay Tauri), mốc `N = 4.096` đã vượt ngân sách tick `60 Hz` tới `4,67×` (`77,90 ms`), trong khi `N = 1.000` dùng `33,9%` (`5,64 ms`). Trần mô phỏng thời gian thực `60 Hz` trên CPU đơn luồng tất định của máy này nằm chặt trong khoảng `(1.000 ; 4.096)` agent.

### Đầu ra Criterion nguyên văn (`stderr` / `stdout` đã lưu)

```text
Benchmarking live_schedule_scaling/10: Warming up for 3.0000 s
Benchmarking live_schedule_scaling/10: Collecting 50 samples in estimated 8.0729 s (74k iterations)
live_schedule_scaling/10     time: [101.01 µs 102.27 µs 103.85 µs]
Benchmarking live_schedule_scaling/100: Warming up for 3.0000 s
Benchmarking live_schedule_scaling/100: Collecting 50 samples in estimated 8.2061 s (27k iterations)
live_schedule_scaling/100    time: [332.57 µs 344.72 µs 357.41 µs]
Benchmarking live_schedule_scaling/1000: Warming up for 3.0000 s
Benchmarking live_schedule_scaling/1000: Collecting 50 samples in estimated 14.516 s (2550 iterations)
live_schedule_scaling/1000   time: [5.6568 ms 5.7524 ms 5.8648 ms]
Benchmarking live_schedule_scaling/4096: Warming up for 3.0000 s
Benchmarking live_schedule_scaling/4096: Collecting 20 samples in estimated 17.185 s (210 iterations)
live_schedule_scaling/4096   time: [73.361 ms 74.644 ms 76.901 ms]
Benchmarking live_schedule_scaling/5000: Warming up for 3.0000 s
Benchmarking live_schedule_scaling/5000: Collecting 20 samples in estimated 18.659 s (210 iterations)
live_schedule_scaling/5000   time: [90.901 ms 92.866 ms 94.289 ms]
```
