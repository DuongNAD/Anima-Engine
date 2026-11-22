---
title: Lộ trình làm quen Anima Engine
status: active
owner: maintainers
last_reviewed: 2026-11-22
review_cycle: per-release
---

# Tutorials

## Danh sách bài học thực hành

| Bài | Nội dung | Trạng thái |
|---|---|---|
| [01 — Từ clone tới thí nghiệm headless đầu tiên](01-first-headless-experiment.md) | Đi từ clone sạch tới cài dependency, chạy `e2_seam_tests`, build release `run_e2_brain_experiment`, chạy E2 smoke, kiểm tra digest với `npm run check:e2` và đọc ranh giới suy luận | `active` |

## Lộ trình 60 phút

### 1. Hiểu thế giới cần mô phỏng

Đọc theo thứ tự:

1. [`README.md`](../../README.md) — mục tiêu và kiến trúc.
2. [`WORLD_DESIGN.md`](../../WORLD_DESIGN.md) — trải nghiệm và thế giới đích.
3. [`SIMULATION_RULES.md`](../../SIMULATION_RULES.md) — đơn vị, bảo toàn và lát cắt MVP.

Kết quả mong đợi: giải thích được đường tác động
“mưa → nước/đất → cỏ → thỏ → sói” và phân biệt causal event với log kỹ thuật.

### 2. Chạy một vòng kiểm tra

Làm theo [How-to](../how-to/README.md): cài dependency, chạy frontend test và Cargo
test. Đọc [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md) trước khi tạo một report baseline trên
máy của bạn: script cần output của `cargo bench` và sẽ từ chối chạy trên một clone chưa bench (xem
How-to).

Kết quả mong đợi: biết test nào bảo vệ Rust, TypeScript và biên artifact.

### 3. Theo dấu một thay đổi

Chọn một scenario nhỏ trong [`WORLD_SIMULATION_PLAN.md`](../../WORLD_SIMULATION_PLAN.md):

1. tìm luật bị tác động trong `SIMULATION_RULES.md`;
2. tìm component/system liên quan trong `src-tauri/src/core/`;
3. tìm artifact/renderer consumer tương ứng;
4. xác định invariant, fixture và benchmark cần cập nhật;
5. ghi việc cần làm vào backlog ở [`STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md) §3
   (`TODO.md` chỉ là nhật ký việc đã làm); không định nghĩa lại luật ở đó.

### 4. Đề xuất một tích hợp nguồn mở

Đọc [khảo sát](../research/OPEN_SOURCE_LANDSCAPE.md), chọn một ứng viên `Pilot`, rồi
dùng [ADR template](../decisions/ADR-0000-template.md) để ghi ranh giới, baseline,
quality gate và rollback. Không thêm package trước khi license và nguồn sự thật được
xác định.

## Bài tiếp theo nên được bổ sung

- Tạo scenario “mưa tăng 20%” và đọc causal chain.
- Thêm một producer species từ schema đến renderer.
- Tạo scientific fixture có provenance.
- Migration `WorldArtifact` từ phiên bản N sang N+1.

Mỗi tutorial mới phải chạy được từ môi trường sạch, có kết quả quan sát được và link
tới reference thay vì sao chép hợp đồng.
