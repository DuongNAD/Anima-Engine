---
title: Chỉ mục kế hoạch
status: active
owner: maintainers
last_reviewed: 2026-07-26
---

# Kế hoạch

| Phạm vi | Tài liệu | Vai trò |
|---|---|---|
| **Bắt đầu ở đây** | [`STATE_OF_THE_PROJECT.md`](STATE_OF_THE_PROJECT.md) | Trạng thái **đo được** + backlog ưu tiên kèm điểm neo và định nghĩa hoàn thành |
| **Kế hoạch thực thi hiện hành** | [`EXECUTION_PLAN_2026-10-31.md`](EXECUTION_PLAN_2026-10-31.md) (`proposed`) | Task HT-A / HT-B / HT-C từng mục, kèm file, lệnh xác minh và phụ thuộc. Brief cho agent: [Antigravity](../ai/planning/2026-10-31-antigravity-brief.md) |
| Tách crate con | [`CRATE_SPLIT_PLAN_2026-10-31.md`](CRATE_SPLIT_PLAN_2026-10-31.md) (`proposed`) | S0–S5: phá sáu vòng phụ thuộc bằng một tầng type còn thiếu, rồi tách 6 crate. Bộ ba kiểm V1/V2/V3 mỗi bước. Brief: [Antigravity](../ai/planning/2026-10-31-antigravity-brief-crate-split.md) |
| Hoàn thiện — nghiên cứu mới nhất | [`COMPLETION_ROADMAP_2026-10-31.md`](COMPLETION_ROADMAP_2026-10-31.md) (`proposed`) | Đo 2026-10-31 trên hai cây mã, chẩn đoán phân kỳ `main` ↔ `feat/ecological-selection`, và thứ tự HT0′–HT2′. Thay §1 của roadmap 16/10 |
| Toàn bộ hệ mô phỏng | [`WORLD_SIMULATION_PLAN.md`](../../WORLD_SIMULATION_PLAN.md) | Roadmap sản phẩm/kỹ thuật dài hạn |
| Creature morphogenesis | [`CREATURE_MORPHOGENESIS_PLAN.md`](CREATURE_MORPHOGENESIS_PLAN.md) | Genotype, phenotype, spawn, persistence và adaptation gates |
| Alternate evolution + World Lab | [Feature task plan](../ai/planning/2026-07-24-feature-alternate-evolution-world-lab.md) | World laws, exotic energy, forks, observability và species evidence |
| Áp dụng dự án nguồn mở | [Open-source adoption plan](OPEN_SOURCE_ADOPTION_PLAN.md) | Luồng việc tích hợp, thử nghiệm và loại bỏ |
| Backlog thực thi hiện tại | [`TODO.md`](../../TODO.md) | Trạng thái công việc ngắn hạn |

Kế hoạch mô tả **sẽ làm gì**; hợp đồng trong [Reference](../reference/README.md) mô tả
**điều đang đúng**. Khi kế hoạch được triển khai, cập nhật hợp đồng/test rồi mới đánh
dấu hoàn tất.
