---
phase: planning
feature: snapshot-restore-fidelity
title: Thẻ F047 — cô lập nguyên nhân lệch khôi phục của desktop app sau F046
description: Xác nhận LiveExperimentAdapter khớp từng bit ở R3-like (cùng R3_like evolved) và thế giới S12/S13, đồng thời cô lập nguyên nhân lệch khôi phục trong harness desktop app là va chạm AgentLineageId giữa các con cùng bố mẹ khi khôi phục pending_inference theo lineage_id đơn lẻ.
status: completed
owner: simulation-architecture
last_reviewed: 2026-11-19
---

# Thẻ F047 — cô lập nguyên nhân lệch khôi phục của desktop app sau F046

> **Thẻ chẩn đoán + khoá test (`Path I` chỉ đọc mã nguồn — 0 sửa đổi trong `src-tauri/src/`).**

## 1. Kết quả `LiveExperimentAdapter` (`WHAT #1`)

Chạy tuần tự qua vòng đĩa thật (`SnapshotEnvelope::seal` -> `snapshot::write_atomic` -> `snapshot::read` -> `LiveExperimentAdapter::from_snapshot`), gọi `observables()` sau mỗi bước và so khớp `adapter.checksum()` từng tick với lượt chạy liền mạch (`seed = 3`, vùng `16 × 16`):

| Cấu hình | Các mốc khôi phục `K -> H` | `imm_eq` | `first_divergence` | Kết luận |
|---|---|---|---|---|
| `R3-like` não dùng chung (`30` founder, `predator_fraction = 0.25`, `locomotion = 1`, `drive_floor = 0.5`, `thirst_steering = 1`, `mortality = 1`, `reproduction = 1`) | `1, 899, 900, 901, 950 -> 1.900` | `true` (tất cả 5 mốc) | `None` (tất cả 5 mốc) | Khớp bit từng tick |
| `R3-like` kèm `evolved_brains = 1.0` | `1, 899, 900, 901, 950 -> 1.900` | `true` (tất cả 5 mốc) | `None` (tất cả 5 mốc) | Khớp bit từng tick |
| Thế giới `S12/S13` (`100` founder, `16` hồ, `lake_layout = 1`, `prey_cap = 320`, `resource_capacity_scale = 0.63`, `dehydration_ticks = 960`) | `899, 900, 2000 -> 4.000` | `true` (tất cả 3 mốc) | `None` (tất cả 3 mốc) | Khớp bit từng tick |

**Lý do `LiveExperimentAdapter` không lệch:** trong lịch chính quy (`simulation_schedule.rs:137-139`), `live_inference_pump_system` trả lời batch suy luận đồng bộ ngay trong cùng tick trước khi `action_resolution_system` tiêu thụ. Tại mọi ranh giới tick để save/load, mọi agent đều ở `CognitiveState::Ready` và hàng đợi phản hồi luôn rỗng.

## 2. Phân tách hai nửa (`WHAT #2`) trên harness desktop app (`R2a` và `R3`)

Tái dựng `AppWorldHarness` của F046 kèm bộ so khớp từng trường `SavedSimulationState` tại mỗi tick và đo ba phép thử cô lập trên bản sao tạm `f047-probe`:

| Phép thử | `R2a` (`K = 898, 899, 900, 901, 950 -> 1.900`) | `R3` (`K = 1, 899, 900, 950 -> 1.900`) | Hệ quả |
|---|---|---|---|
| **Baseline** (`entities_by_lineage` như `simulation_loop.rs`) | `K=898, 900, 950 -> None`; `K=899 -> 917`; `K=901 -> 919` | `K=1 -> None`; `K=899 -> 900`; `K=900 -> 902`; `K=950 -> 951` | Lệch ở cả `R2a` (lẻ) và `R3` (hậu sinh sản) |
| **Bisect `(a)`**: giữ `InferenceTicketCursor` & `LodTickCursor` qua restore | Không đổi (`K=899 -> 917`, `K=901 -> 919`) | `K=1` xoá luôn diff số ticket (`first_state_diff = None`), nhưng `K=899 -> 900`, `900 -> 902`, `950 -> 951` giữ nguyên từng bit | Bác bỏ giả thuyết parity/ticket `Local<u64>` |
| **Bisect `(b)`**: sắp xếp mỗi ô `SpatialHashGrid` theo `entity.index()` | Không đổi (`K=899 -> 917`, `K=901 -> 919`) | Không đổi (`K=899 -> 900`, `900 -> 902`, `950 -> 951`) | Bác bỏ giả thuyết thứ tự ô `SpatialHashGrid` |
| **Bisect `(c)`** (đối chứng căn nguyên): tra cứu `pending_batches` theo `(lineage_id, request_id)` khớp `CognitiveState::PendingInference(ticket_id)` | **`None` ở toàn bộ 5/5 mốc** | **`None` ở toàn bộ 4/4 mốc** | **Loại bỏ 100% lệch ở cả `R2a` và `R3`** |

## 3. Nguyên nhân gốc thực sự

1. `execute_reproduction_birth` (`src-tauri/src/core/agent_systems.rs:1066`) đặt định danh con bằng `format!("{parent_lineage_id}.{}", parent_generation + 1)`, nên mọi anh/chị em cùng bố mẹ mang **chung một chuỗi `AgentLineageId`** (như `drift_control.rs:357` cũng ghi nhận).
2. Khi desktop app save/restore giữa nhịp bất đồng bộ (tại tick `K`, các yêu cầu vừa chạy xong nằm trong `res_rx` chờ `action_resolution_system` đầu tick `K+1`):
   - `src-tauri/src/core/simulation_loop.rs:1713-1738` xây `entities_by_lineage: HashMap<String, Entity>` chỉ lấy `lineage_id` làm khoá, ghi đè entity của anh/chị trước bằng anh/chị sau cùng `lineage_id`. Khi nạp lại `pending_inference`, phản hồi mang `request_id` của sibling trước bị gán nhầm sang entity của sibling sau nên `action_resolution_system` (`if ticket_id == response.request_id`) không khớp; sibling thật kẹt ở `PendingInference` tới hết `K+1` (hoặc timeout `ticks_pending > 5` ở `dynamics.rs:163`), làm lệch `action_gates` và `cpg_parameters` rồi lan sang `world_checksum`.
   - Cùng lỗi giả định duy nhất theo `lineage_id` cũng tồn tại tại `src-tauri/src/core/snapshot.rs:678-715` (`pending_by_lineage: HashMap<&str, u64>`): nếu hai sibling trùng `lineage_id` cùng đang ở `PendingInference` lúc checkpoint, hàm `validate` đếm sai `pending_by_lineage.len() != pending_agents.count()` và từ chối snapshot hợp lệ bằng lỗi `InvalidData`.
3. Ở `R2a` có chu kỳ 2 tick nên sai lệch chỉ rơi vào các mốc lẻ (`K=899, 901`) trúng lúc sibling bị ghi đè đang giữ `action_gates.feed_intent ~ 0.62..0.66`; ở `R3` không trùng `lineage_id` lúc `K=1` (chỉ có founder) nên `K=1` khớp `None`, và lệch ngay tại `K=899, 900, 950` sau khi sinh sản tạo các cặp sibling trùng tên.

## 4. Test đã cam kết & đường dẫn khuyến nghị khi mở thẻ sửa

- File test: `src-tauri/tests/f047_restore_divergence_tests.rs` (4 test; các ca lệch đang pin bằng `assert!(div.is_some())` đúng `WHAT #3`, đồng thời chạy kèm cặp đối chứng `(lineage_id, request_id)` khẳng định `assert_eq!(fixed_div, None)`).
- Nếu mở thẻ sửa tiếp theo (khuyến nghị chọn phương án khoá theo `(lineage_id, request_id)` / `(request_id)` thay vì đổi định dạng `child_lineage` để giữ nguyên hành vi và tương thích snapshot cũ):
  1. `src-tauri/src/core/simulation_loop.rs` (`1712-1753`): dựng bản đồ `(lineage_id, ticket_id) -> Entity` từ các entity mang `CognitiveState::PendingInference(ticket_id)` (hoặc theo `request_id`) khi nạp `pending_inference`.
  2. `src-tauri/src/core/snapshot.rs` (`678-715`): đổi `pending_by_lineage` sang tập `(lineage_id, request_id)` khi kiểm tra hợp lệ `SavedSharedLearningState::pending_inference`.
  3. `src-tauri/tests/f047_restore_divergence_tests.rs`: giữ nguyên mọi test adapter & đối chứng, chuyển 5 assertion `div.is_some()` sang `None` và bổ sung test cho `snapshot::read` với 2 sibling cùng `PendingInference`.
- `SCHEMA_VERSION` (`8`) và `LIVE_MODEL_VERSION` (`"live-bevy-world/2"`) **không thay đổi**.
