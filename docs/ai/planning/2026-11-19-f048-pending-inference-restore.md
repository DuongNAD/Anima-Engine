---
phase: planning
feature: snapshot-restore-fidelity
title: Thẻ F048 — sửa va chạm sibling theo AgentLineageId khi khôi phục pending_inference
description: Định tuyến và kiểm tra hợp lệ các phản hồi PendingInference đã lưu theo cặp (lineage_id, request_id) khớp CognitiveState::PendingInference(ticket_id) qua hàm dùng chung route_restored_pending_inference, loại bỏ lệch khôi phục trong desktop app mà không đổi định dạng AgentLineageId hay phiên bản schema/mô hình.
status: completed
owner: simulation-architecture
last_reviewed: 2026-11-19
---

# Thẻ F048 — sửa va chạm sibling theo `AgentLineageId` khi khôi phục `pending_inference`

> **Thẻ sửa lỗi theo `Path I` cho chuỗi F046 -> F047 -> F048, xây dựng trên nhánh F047 (`fb88ba6f`) và thay thế PR `#144` (`Supersedes #144`). `SCHEMA_VERSION = 8` và `LIVE_MODEL_VERSION = "live-bevy-world/2"` không thay đổi.**

## 1. Các thay đổi đã thực hiện

Giữ nguyên cách đặt `AgentLineageId` cho con non tại `src-tauri/src/core/agent_systems.rs:1066` (`format!("{parent_lineage_id}.{}", parent_generation + 1)`) để không làm đổi chuỗi sinh, và sửa đồng bộ cả ba điểm từng giả định `lineage_id` là khoá duy nhất của phản hồi `PendingInference`:

1. **`src-tauri/src/core/simulation_state.rs` (`SavedSharedLearningState::validate`):**
   - Đổi tập kiểm tra trùng lặp từ `lineage_id.as_str()` sang `(response.lineage_id.as_str(), response.request_id)`, cho phép nhiều anh/chị em cùng `AgentLineageId` có vé `PendingInference` khác nhau cùng tồn tại hợp lệ trong một snapshot thật (`shared_learning = Some(...)`).
2. **`src-tauri/src/core/simulation_state.rs::route_restored_pending_inference` và `src-tauri/src/core/simulation_loop.rs` (`SimulationEngine::start`):**
   - Tách một hàm `pub` dùng chung `route_restored_pending_inference(world: &mut World, saved_batches: &[SavedInferenceResponseBatch]) -> (Vec<Vec<AgentInferenceResponse>>, usize)` lập chỉ mục `(lineage_id, ticket_id) -> Entity` chỉ từ các thực thể đang mang `CognitiveState::PendingInference(ticket_id)`, ghép từng phản hồi bằng `.remove(...)`, và bỏ qua + đếm mọi phản hồi không khớp (không bao giờ gán nhầm cho anh/chị em).
   - `SimulationEngine::start` gọi trực tiếp `route_restored_pending_inference` thay cho `entities_by_lineage: HashMap<String, Entity>`, nạp lại các batch hợp lệ vào `res_tx` và ghi cảnh báo nếu có phản hồi bị loại.
3. **`src-tauri/src/core/snapshot.rs` (`validate_state`):**
   - Đổi `pending_by_lineage` thành `HashSet<(&str, u64)>` theo `(lineage_id, request_id)` và kiểm tra song ánh 1-1 với mọi agent đang ở `CognitiveState::PendingInference(request_id)`, giữ nguyên thông báo lỗi khi thiếu hoặc lệch vé.

## 2. Xác minh & hiệu năng test (`< 90 s` debug)

Cập nhật `src-tauri/tests/f047_restore_divergence_tests.rs` gọi trực tiếp `route_restored_pending_inference`, chuyển toàn bộ các mốc `AppWorldHarness` (`R2a` và `R3`) sang khẳng định `div == None`, rút ngắn chân trời để toàn bộ file chạy xong dưới `90 s` ở cấu hình debug trên macOS ARM64 nhưng vẫn giữ cả mốc lẻ/chẵn và cả hai chế độ `LiveExperimentAdapter`, đồng thời bổ sung bài test vòng đĩa thật cho hai sibling cùng `PendingInference`:

| Bài test / cổng | Kết quả | Thời gian / ghi chú |
|---|---|---|
| `cargo test --features desktop --test f047_restore_divergence_tests` | `5 passed; 0 failed` | `30.82s` (tổng biên dịch + chạy `real 42.71s < 90s`) |
| `cargo test --no-default-features --test f047_restore_divergence_tests` | `5 passed; 0 failed` | `30.87s` |
| `cargo test --release --no-default-features --test f047_restore_divergence_tests` | `5 passed; 0 failed` | `2.59s` |
| `f048_two_pending_siblings_roundtrip_disk_snapshot_and_route_by_ticket` | `ok` | `SnapshotEnvelope::seal` -> `snapshot::write_atomic` -> `snapshot::read` với `shared_learning = Some(...)` và 2 sibling cùng `"founder-...0.1"`, khớp đúng từng `Entity` (`dropped = 0`), và loại bỏ phản hồi sai vé mà không gán cho sibling ở `Ready` (`dropped = 2`) |
| `f045_restore_equivalence_tests` (base vs head) | `5 passed; 0 failed` | Quỹ đạo liền mạch S6 `seed = 3` trùng bit: `0x30ea8bbf`, `0xf5876d6e`, `0xbc114750`, `0x21424343` |
| `live_experiment_tests` (base vs head) | `17 passed; 0 failed` | Đầu ra trùng khớp hoàn toàn (`uninterrupted: 294121680, control: 294121680, treatment: 1365939704`) |
| `restore_after_death_order_tests` | `4 passed; 0 failed` | `6.13s` |
| `brain_persistence_tests`, `persistence_tests`, `persistence_stress_tests`, `snapshot_checkpoint_tests`, `--lib snapshot` | Đạt 100% | `17` + `2` + `4` + `7` + `32` passed |
| `cargo fmt --all -- --check` và 2 profile `cargo clippy --all-targets -- -D warnings` | Thoát `0` | Đạt cả `--features desktop` và `--no-default-features` |
| `cargo test --features desktop` + `check_test_targets.mjs --profile desktop` | `157` targets: `1523 passed; 0 failed; 2 ignored` | `check_test_targets.mjs` thoát `0` (gồm `8` target feature-gated, `3` empty allow-listed, `2` ignored allow-listed) |

## 3. Bài học cho bộ test của F047 và F048 (tính bất biến liên nền tảng)

Trên PR `#144` (commit `fb88ba6f` của nhánh `F047`), job CI `hardening / determinism` trên Ubuntu x86\_64 đã báo đỏ tại bài kiểm tra hồi quy cũ `f047_app_harness_r2a_restore_checkpoints` (`R2a K=899` với khẳng định `div.is_some()`): do sai khác số thực dấu phẩy động theo nền tảng (`libm` / tối ưu phát hành), chế độ mật độ thấp không vận động `R2a` va chạm sibling tại đúng tick `899` trên macOS ARM64 nhưng không kích hoạt va chạm đó tại `K=899` trên Ubuntu x86\_64 (trong khi `R3` va chạm trên cả hai kiến trúc).

Vì vậy PR `#144` không tự merge và thẻ `F048` nối tiếp ngay trên `fb88ba6f` (`Supersedes #144`), thay toàn bộ khẳng định ghim lệch bằng khẳng định tương đương (`div == None`) đúng với mọi kiến trúc. **Quy tắc rút ra cho cả F047 và F048:** *một test chỉ được phép so sánh các lượt chạy trên cùng một máy (ví dụ: liền mạch so với khôi phục, hoặc đối chứng so với can thiệp), tuyệt đối không được khẳng định rằng một quỹ đạo phụ thuộc nền tảng phải chạm tới một trạng thái cụ thể* (kể cả khẳng định "phải tái hiện va chạm tại tick `K` trên mọi OS/CPU"). Khi cần kiểm thử trường hợp hai sibling cùng `PendingInference` một cách tất định trên mọi nền tảng, hãy dựng trạng thái đó trực tiếp trong fixture snapshot như `f048_two_pending_siblings_roundtrip_disk_snapshot_and_route_by_ticket`.
