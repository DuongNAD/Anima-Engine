---
title: Lộ trình làm quen Anima Engine
status: active
owner: maintainers
last_reviewed: 2026-07-24
review_cycle: per-release
---

# Tutorials

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
test. Đọc [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md) rồi tạo một report
baseline trên máy của bạn.

Kết quả mong đợi: biết test nào bảo vệ Rust, TypeScript và biên artifact.

### 3. Theo dấu một thay đổi

Chọn một scenario nhỏ trong [`WORLD_SIMULATION_PLAN.md`](../../WORLD_SIMULATION_PLAN.md):

1. tìm luật bị tác động trong `SIMULATION_RULES.md`;
2. tìm component/system liên quan trong `src-tauri/src/core/`;
3. tìm artifact/renderer consumer tương ứng;
4. xác định invariant, fixture và benchmark cần cập nhật;
5. ghi task vào `TODO.md`, không định nghĩa lại luật ở đó.

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
