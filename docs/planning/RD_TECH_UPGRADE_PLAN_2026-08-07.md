---
title: Kế hoạch thử nghiệm nâng cấp data plane, Meta-AI và asset 3D
status: proposed
owner: maintainers
last_reviewed: 2026-08-07
---

# Kế hoạch nâng cấp từ đánh giá R&D 2026-08-07

Nguồn bằng chứng: [`RD_TECH_UPGRADE_ASSESSMENT_2026-08-07.md`](../research/RD_TECH_UPGRADE_ASSESSMENT_2026-08-07.md).
Mọi mục bắt đầu ở trạng thái **not started** và cần quyết định/ADR trước khi thêm runtime dependency.

## Mục tiêu và ngoài phạm vi

Mục tiêu:

- giảm serialize/copy và main-thread cost ở payload lớn;
- làm output Meta-AI có kiểu, giới hạn retry và deterministic fallback;
- thử pipeline tạo asset 3D ngoại tuyến có provenance;
- chuẩn bị contract LIVA ↔ Anima mà không ghép chặt hai runtime.

Ngoài phạm vi:

- thay toàn bộ Tauri command plane bằng shared memory;
- đưa Python/PydanticAI vào Rust runtime;
- chia sẻ pointer/KV-cache;
- chạy generative 3D trong simulation tick;
- tích hợp quantum solver khi chưa có benchmark workload.

## Milestone A — Baseline data plane

| ID | Trạng thái | Kết quả | Dependency | Bằng chứng nghiệm thu |
|---|---|---|---|---|
| RD-A1 | not started | Benchmark `save_world_artifact` qua Tauri với artifact 256² (~1,05 MiB) | fixture artifact hiện có | p50/p95 encode, wire bytes, Rust receive và peak allocation |
| RD-A2 | not started | Capture `simulation-tick` và `pheromone-update` trong app release | capture không chạy full backend quá tải | event rate, payload bytes/s, JS parse/decode p95, dropped/long frames |
| RD-A3 | not started | Chốt SLO và ngưỡng quyết định binary pilot | RD-A1, RD-A2 | SLO có đơn vị, máy/commit/config và rollback trigger |

Gate A: không thiết kế shared memory hoặc chọn codec trước khi A1–A3 có số đo tái lập được.

## Milestone B — Binary payload pilot

| ID | Trạng thái | Kết quả | Dependency | Bằng chứng nghiệm thu |
|---|---|---|---|---|
| RD-B1 | not started | Binary transport cho world artifact, giữ format/checksum hiện tại | RD-A3 | Rust/TS golden fixture parity; malformed/oversize fail-closed; p95 và allocation tốt hơn baseline |
| RD-B2 | not started | Packed SoA frame thử nghiệm cho pheromone hoặc simulation tick | RD-A3 | schema versioned; decoder test; wire bytes giảm ≥70% so với JSON cho cùng fixture |
| RD-B3 | not started | Backpressure, sequence id và resync snapshot | RD-B2 | consumer chậm không chặn simulation; drop/reconnect phục hồi từ snapshot |
| RD-B4 | not started | So sánh binary WebSocket/custom protocol với shared-memory ring | RD-B1–B3 | chỉ đề xuất shared memory nếu binary transport không đạt SLO; có Windows/Tauri lifecycle test |

Gate B: giữ JSON control plane. Không nhận Apache Arrow vào runtime nếu packed SoA đạt SLO với ít
dependency hơn. Arrow chỉ là ứng viên cho export telemetry dạng bảng/Parquet.

## Milestone C — Structured Meta-AI

| ID | Trạng thái | Kết quả | Dependency | Bằng chứng nghiệm thu |
|---|---|---|---|---|
| RD-C1 | not started | Response contract serde cho `EnvironmentalEvent` | provider capability audit | valid/invalid/extra-text fixtures; không substring classification |
| RD-C2 | not started | Tối đa một repair retry và deterministic fallback | RD-C1 | retry budget test; timeout/error không đổi state; deterministic mode thực hiện 0 network call |
| RD-C3 | not started | Telemetry outcome không chứa secret/prompt thô | RD-C2 | success/repair/fallback counters và redaction tests |

Gate C: malformed output không được tạo event ngoài enum hợp lệ và không làm replay mất tính xác định.

## Milestone D — Hunyuan3D offline pilot

| ID | Trạng thái | Kết quả | Dependency | Bằng chứng nghiệm thu |
|---|---|---|---|---|
| RD-D1 | not started | Kiểm license code/model/input/output và chọn checkpoint nhỏ | policy/ADR | hồ sơ version, hash, license, provenance và uninstall path |
| RD-D2 | not started | Job tách biệt tạo shape trên GPU mục tiêu | RD-D1 | peak VRAM, thời gian, cancel và recovery; không chạy đồng thời LIVA model |
| RD-D3 | not started | Mesh validation → simplify/LOD → GLB | RD-D2 | finite vertices, bounds, triangle/material budgets, GLB round-trip và render fixture |
| RD-D4 | not started | Quyết định adopt/reject | RD-D2–D3 | chất lượng, thời gian và chi phí vượt baseline thủ công theo ngưỡng đã chốt |

## Milestone E — Contract LIVA ↔ Anima

| ID | Trạng thái | Kết quả | Dependency | Bằng chứng nghiệm thu |
|---|---|---|---|---|
| RD-E1 | not started | Typed/authenticated control contract | ownership và threat model hai repo | version negotiation, permission, timeout, cancellation và negative tests |
| RD-E2 | not started | Binary snapshot/telemetry contract | RD-B3 | length cap, backpressure, reconnect, producer isolation và end-to-end p95 |
| RD-E3 | not started | Quyết định có cần shared memory hay không | RD-E2 | benchmark trên Windows mục tiêu; không dựa vào tuyên bố “<1 ms” bên ngoài |

## Thứ tự đề xuất

1. RD-A1 → RD-A3.
2. RD-C1 → RD-C3, vì phạm vi nhỏ và tăng độ đúng ngay cả khi binary pilot bị loại.
3. RD-B1 → RD-B3.
4. RD-D1 → RD-D4.
5. RD-E1 → RD-E3 khi có use case tích hợp thật.

Quantum/Classiq và “ZCAO” không có task thực thi. Chúng chỉ được xem lại khi xuất hiện workload,
upstream định danh được và benchmark chứng minh lợi ích.

