---
title: Cách thêm một biến quan sát (observable) mới
status: active
owner: maintainers
last_reviewed: 2026-11-22
review_cycle: monthly
---

# Cách thêm một biến quan sát (observable) mới

Hướng dẫn này mô tả quy trình thêm một biến quan sát vô hướng mới vào thế giới sống (`LiveExperimentAdapter`) hoặc thế giới tham chiếu (`ReferenceEvolutionWorld`), bảo đảm đúng hợp đồng metadata (`ER10`), bảo toàn sổ cái năng lượng, và không phá hỏng các manifest E2 đã đóng băng.

> **Hợp đồng liên quan (không định nghĩa lại ở đây):** [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) *(proposed)* (`ER07`, `ER10`, `ER11`), [`ENERGY_LEDGER_CONTRACT.md`](../reference/ENERGY_LEDGER_CONTRACT.md) (§1, §1.1, §5), [`DETERMINISM_CONTRACT.md`](../reference/DETERMINISM_CONTRACT.md), và ví dụ triển khai S2 gần nhất tại [`2026-11-19-s2-observables.md`](../ai/planning/2026-11-19-s2-observables.md).

---

## Kiểm tra trước khi bắt đầu: ranh giới đóng băng và tính năng chưa có

1. **Tuyệt đối không sửa `LIVE_OBSERVABLE_IDS`** ([`src-tauri/src/core/live_experiment.rs`](../../src-tauri/src/core/live_experiment.rs)): danh sách 12 ID này bị ghim bởi tiền đăng ký E2 (`tests/e2_frozen_manifest_tests.rs`). Mọi biến sống mới phải nối vào cuối `LIVE_EXTENDED_OBSERVABLE_IDS` (hiện có 15 mục).
2. **Không dùng chung ID với `reference_default()`** trừ khi đo đúng cùng đại lượng vật lý, cùng `unit`, cùng `ConservationRole` và `ObservableScope`. Hiện tại chỉ có **đúng 2 ID** dùng chung là `plants` và `detritus` (`tests/live_experiment_tests.rs`: `live_and_reference_agree_on_shared_ids` khẳng định `shared == 2`).
3. **Không khai báo hoặc giả định các phần chưa triển khai ở thế giới sống:**
   - `LiveExperimentAdapter` **không** có trường ngoại lai `exotic.*` (`MU`) và **không** có quần thể AE3 `evolution.*` (chỉ có ở `ReferenceEvolutionWorld`).
   - `ObservableSpec` hiện **không** có trường `causal_parents` trong struct Rust (`src-tauri/src/core/experiment/observables.rs`), dù tài liệu mục tiêu `ER10` có nhắc tới.
   - `time-to-floor` cố ý được giữ là chỉ số **post-hoc** trong script phân tích (không phải observable phát theo tick), vì ngưỡng sàn phụ thuộc từng thí nghiệm và các lượt chạy chưa chạm sàn là dữ liệu censored.
   - Không phát giá trị sentinel như `-1.0` hay `NaN` khi quần thể vắng mặt hoặc tuyệt chủng — luôn phát giá trị hữu hạn trung thực (ví dụ `0.0`), hoặc chỉ bỏ qua theo hợp đồng tường minh như `live.deaths_starvation` trong nhánh `drift_control`.

---

## Bước 1 — Đăng ký `ObservableSpec` trong `ObservableRegistry`

Mở [`src-tauri/src/core/experiment/observables.rs`](../../src-tauri/src/core/experiment/observables.rs) và thêm một `ObservableSpec` vào `ObservableRegistry::live_default()` (hoặc `reference_default()` nếu dành cho thế giới tham chiếu):

- Điền đủ 11 trường: `id`, `display_name`, `unit` (ví dụ `EU`, `EU·tick`, `individuals`, `fraction`), `scope` (`ObservableScope::World`, v.v.), `cadence_name`, `cadence_period > 0` (`PHYSICS_PERIOD = 1` hoặc `ECOLOGY_PERIOD = 60`), `aggregation` (`Instant`, `Sum`, `Mean`, `Max`), `valid_min`, `valid_max`, `conservation` (`ClosedEu`, `ExoticMu`, `None`), và `source` (`"core::live_experiment::LiveExperimentAdapter"`).
- **Cấm dùng `f64::INFINITY` hoặc `f64::NEG_INFINITY`:** `serde_json` chuyển float không hữu hạn thành `null`. Với biên mở, dùng hằng số `Self::OPEN_LOWER_BOUND` (`-1e300`) và `Self::OPEN_UPPER_BOUND` (`1e300`).
- `ObservableRegistry::validate()` kiểm tra `OBSERVABLE_REGISTRY_VERSION == 2`, chuỗi khác rỗng, `cadence_period > 0`, `valid_min <= valid_max` hữu hạn, và không trùng `id`.

---

## Bước 2 — Nối ID vào `LIVE_EXTENDED_OBSERVABLE_IDS`

Mở [`src-tauri/src/core/live_experiment.rs`](../../src-tauri/src/core/live_experiment.rs) và thêm ID mới vào **cuối** mảng `LIVE_EXTENDED_OBSERVABLE_IDS` (tăng độ dài mảng lên `N + 1`). Hàm `all_live_observable_ids()` ghép `LIVE_OBSERVABLE_IDS` rồi tới `LIVE_EXTENDED_OBSERVABLE_IDS` để giữ thứ tự phát ổn định.

---

## Bước 3 — Tính toán và phát giá trị trong `read_live_observables`

Trong `read_live_observables(world: &mut World, close_window: bool)` (`src-tauri/src/core/live_experiment.rs`):

1. **Đọc chỉ số tức thời:** chỉ đọc ECS resource/component; không làm thay đổi trạng thái `World` và không rút số từ `SimRng`.
2. **Đọc bộ đếm theo cửa sổ (windowed):** `experiment_runner::drive` gọi `observables_unsampled()` (`live_observables_peek`, `close_window = false`) trên **mọi tick** để kiểm tính hữu hạn và không trùng tên, và chỉ gọi `observables()` (`live_observables`, `close_window = true`) ở các tick lấy mẫu. Chỉ được reset/drain bộ đếm cửa sổ (`VitalStatistics::sample_window()`) khi `close_window == true`; khi `close_window == false` phải dùng đọc không phá hủy (`peek_window()`).
3. **Bộ tích lũy qua nhiều tick (mẫu S2 như `live.mean_energy_integral`):** nếu biến là tích phân hoặc tổng cộng dồn qua từng tick, tích lũy đúng một lần mỗi tick ở `LiveExperimentAdapter` (`step` và `run_schedule_once`), lưu bản sao có `#[serde(default)]` trên `LiveExperimentState` để phục hồi qua checkpoint đĩa (`snapshot`/`from_snapshot`) mà không đụng `SavedSimulationState` hay `world_checksum`, và chỉ đọc lại trong `read_live_observables`.
4. **Trường hợp `0` cá thể hoặc `drift_control`:** bảo đảm trả về `f64` hữu hạn (ví dụ `0.0`, như `mass_statistics(&[])[0]`). Kiểm tra rõ biến có phải phát ở cả hai nhánh `selected` và `drift_control` hay bị từ chối trong nhánh `drift_control`.
5. **Thứ tự và khớp tên:** mỗi ID trong `all_live_observable_ids()` phải xuất hiện đúng một lần với tên trùng khớp `ObservableSpec.id`, nếu không `specs_for_emitted` sẽ gắn warning và test registry sẽ fail.

---

## Bước 4 — Chặn cấu hình không hợp lệ ở `ensure_observables_supported`

Nếu observable mới đo sự kiện từ một luật tùy chọn (`live.mortality`, `live.reproduction`, v.v.), thêm kiểm tra vào `LiveWorldConfig::ensure_observables_supported` (`src-tauri/src/core/live_experiment.rs`) để từ chối sớm bằng `ExperimentError::InvalidObservableRequest` thay vì phát số `0` giả tạo khi luật đó tắt.

---

## Bước 5 — Viết và chạy bộ kiểm thử xác minh

Mở rộng test ở [`src-tauri/tests/observables_tests.rs`](../../src-tauri/tests/observables_tests.rs) (độ phủ registry, thứ tự phát ổn định, từ chối preflight) và/hoặc [`src-tauri/tests/s2_observables_tests.rs`](../../src-tauri/tests/s2_observables_tests.rs) (không nhiễu giữa đọc mỗi tick / cửa sổ / không lấy mẫu, checkpoint resume, quần thể rỗng). Nếu thêm file `tests/*.rs` mới, bảo đảm có ít nhất một test không gắn `#[ignore]` để tuân thủ [`scripts/test_target_policy.mjs`](../../scripts/test_target_policy.mjs).

Chạy toàn bộ các target kiểm chứng từ thư mục gốc repository:

<!-- doc-test -->
```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib core::experiment::tests::observable
cargo test --manifest-path src-tauri/Cargo.toml --lib core::experiment::tests::defect_a
cargo test --manifest-path src-tauri/Cargo.toml --test observables_tests
cargo test --manifest-path src-tauri/Cargo.toml --test s2_observables_tests
cargo test --manifest-path src-tauri/Cargo.toml --test live_windowed_observable_sampling_tests
cargo test --manifest-path src-tauri/Cargo.toml --test e2_frozen_manifest_tests
```

Kết quả quan sát được trên `macOS arm64` (chạy ngày `2026-11-22` tại commit `03b9d3d` bằng 6 lệnh `cargo test` ở trên, rút gọn phần biên dịch; tổng số test toàn dự án có thẩm quyền xem tại [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền)):

```text
running 2 tests
test core::experiment::tests::observable_registry_rejects_duplicate_ids ... ok
test core::experiment::tests::observable_registry_validates_and_is_unique ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 559 filtered out; finished in 0.00s

running 2 tests
test core::experiment::tests::defect_a_registry_validation_rejects_malformed_specs ... ok
test core::experiment::tests::defect_a_reference_registry_is_json_safe_and_finite ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 559 filtered out; finished in 0.00s

running 7 tests (observables_tests) ... ok. 7 passed; 0 failed; 0 ignored
running 5 tests (s2_observables_tests) ... ok. 5 passed; 0 failed; 0 ignored
running 2 tests (live_windowed_observable_sampling_tests) ... ok. 2 passed; 0 failed; 0 ignored
running 2 tests (e2_frozen_manifest_tests) ... ok. 2 passed; 0 failed; 0 ignored
```

---

## Bước tiếp theo

- Viết manifest dùng observable mới: [WRITE_A_MANIFEST.md](WRITE_A_MANIFEST.md).
- Thêm can thiệp mới: [ADD_AN_INTERVENTION.md](ADD_AN_INTERVENTION.md).
