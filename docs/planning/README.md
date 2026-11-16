---
title: Chỉ mục kế hoạch
status: active
owner: maintainers
last_reviewed: 2026-11-14
review_cycle: monthly
---

# Kế hoạch

| Phạm vi | Tài liệu | Vai trò |
|---|---|---|
| **Bắt đầu ở đây** | [`STATE_OF_THE_PROJECT.md`](STATE_OF_THE_PROJECT.md) | Trạng thái **đo được** + backlog ưu tiên kèm điểm neo và định nghĩa hoàn thành. Hai cây đã hợp nhất 2026-11-16 nhưng chưa đo lại trên thân cây; tới khi có `§1.g`, số đo mới nhất nằm tạm ở roadmap 31/10 §1 — banner đầu STATE §1 nói rõ |
| **Giao việc cho coding agent** | [`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md) | Hàng đợi thẻ chạy được ngay, khối `DECISIONS` đã điền để dán vào prompt lead, luật và lệnh xác minh. Thẻ và prompt: [kế hoạch swarm 05/11](../ai/planning/2026-11-05-swarm-master-plan.md), [file prompt](../ai/planning/2026-11-05-swarm-prompts.md) |
| **Kế hoạch thực thi hiện hành** | [`EXECUTION_PLAN_2026-10-31.md`](EXECUTION_PLAN_2026-10-31.md) (`proposed`) | Task HT-A / HT-B / HT-C từng mục, kèm file, lệnh xác minh và phụ thuộc. Brief cho agent: [Antigravity](../ai/planning/2026-10-31-antigravity-brief.md) |
| Tách crate con | [`CRATE_SPLIT_PLAN_2026-10-31.md`](CRATE_SPLIT_PLAN_2026-10-31.md) (`proposed`) | S0–S5: phá sáu vòng phụ thuộc bằng một tầng type còn thiếu, rồi tách 6 crate. Bộ ba kiểm V1/V2/V3 mỗi bước. Brief: [Antigravity](../ai/planning/2026-10-31-antigravity-brief-crate-split.md) |
| Định hướng và điều phối | [`DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md`](DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md) (`proposed`) | Đi tới đâu, theo chặng nào, ai quyết gì (QĐ-M), cái gì phải dừng. Không thay kế hoạch thực thi 31/10; thêm nhiều máy, người dùng đầu tiên và luật điều phối giữa các kế hoạch. Dựa trên [tổng hợp nghiên cứu 13/11](../research/RESEARCH_SYNTHESIS_2026-11-13.md) |
| Hoàn thiện — nghiên cứu mới nhất | [`COMPLETION_ROADMAP_2026-10-31.md`](COMPLETION_ROADMAP_2026-10-31.md) (`proposed`) | Đo 2026-10-31 trên hai cây mã, chẩn đoán phân kỳ `main` ↔ `feat/ecological-selection`, và thứ tự HT0′–HT2′. Thay §1 của roadmap 16/10 |
| Thứ tự tới bản phát hành | [`COMPLETION_ROADMAP_2026-10-16.md`](COMPLETION_ROADMAP_2026-10-16.md) (`proposed`) | Sáu giai đoạn HT0–HT5 từ hiện trạng tới phát hành, ba mức "hoàn thiện", quyết định cần chủ dự án |
| Toàn bộ hệ mô phỏng | [`WORLD_SIMULATION_PLAN.md`](../../WORLD_SIMULATION_PLAN.md) | Roadmap sản phẩm/kỹ thuật dài hạn |
| Creature morphogenesis | [`CREATURE_MORPHOGENESIS_PLAN.md`](CREATURE_MORPHOGENESIS_PLAN.md) | Genotype, phenotype, spawn, persistence và adaptation gates |
| Alternate evolution + World Lab | [Feature task plan](../ai/planning/2026-07-24-feature-alternate-evolution-world-lab.md) | World laws, exotic energy, forks, observability và species evidence |
| Áp dụng dự án nguồn mở | [Open-source adoption plan](OPEN_SOURCE_ADOPTION_PLAN.md) | Luồng việc tích hợp, thử nghiệm và loại bỏ |
| Thử nghiệm R&D: data plane, Meta-AI, asset 3D | [`RD_TECH_UPGRADE_PLAN_2026-08-07.md`](RD_TECH_UPGRADE_PLAN_2026-08-07.md) (`proposed`) | Mọi mục ở trạng thái *not started*; cần ADR trước khi thêm runtime dependency |
| Nhật ký công việc | [`TODO.md`](../../TODO.md) | Đã làm gì, khi nào, vì sao — theo thời gian ngược. **Không** phải backlog: backlog ưu tiên ở `STATE_OF_THE_PROJECT.md` §3 |
| Bug thật đã chứng minh | [`FINDINGS.md`](../../FINDINGS.md) | F-001…: mỗi mục có `file:line`, lệnh tái hiện và test khoá lại |
| Working set theo feature | [`docs/ai/`](../ai/README.md) | Requirements → design → planning → implementation → testing của từng feature, và các brief giao việc cho agent |

Kế hoạch mô tả **sẽ làm gì**; hợp đồng trong [Reference](../reference/README.md) mô tả
**điều đang đúng**. Khi kế hoạch được triển khai, cập nhật hợp đồng/test rồi mới đánh
dấu hoàn tất.
