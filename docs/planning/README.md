---
title: Chỉ mục kế hoạch
status: active
owner: maintainers
last_reviewed: 2026-11-22
review_cycle: monthly
---

# Kế hoạch

| Phạm vi | Tài liệu | Vai trò |
|---|---|---|
| **Bắt đầu ở đây** | [`STATE_OF_THE_PROJECT.md`](STATE_OF_THE_PROJECT.md) | Trạng thái **đo được** + backlog ưu tiên kèm điểm neo và định nghĩa hoàn thành. Hai cây đã hợp nhất 2026-11-16; số đo mới nhất (lượt đo đầy đủ ngày 2026-11-18 tại `46e25d5`, thẻ R0) nằm ở [STATE `§1.i`](STATE_OF_THE_PROJECT.md#1i-đo-2026-11-18-tại-46e25d5--lượt-đo-lại-đầy-đủ-sau-đợt-2-trên-windows-11-x64-thẻ-r0), banner đầu §1 trỏ tới |
| **Đang ở chặng nào** | [STATE §3.00, bảng tiến độ theo chặng](STATE_OF_THE_PROJECT.md#300-việc-kế-tiếp--cập-nhật-2026-11-17) | Một bảng cho tám chặng của kế hoạch 13/11: cổng ra, trạng thái, phần còn lại. Ngày 2026-11-17: chặng 1 và 2 đã đóng, chặng 3 đang làm |
| **Quyết định của chủ dự án** | [Roadmap 31/10 §4](COMPLETION_ROADMAP_2026-10-31.md#4-quyết-định-chỉ-chủ-dự-án-đưa-ra-được) | Sổ quyết định: cái đã quyết, ai quyết, và hàng đợi còn trống xếp theo chặng |
| **Giao việc cho coding agent** | [`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md) | Hàng đợi thẻ chạy được ngay, khối `DECISIONS` đã điền để dán vào prompt lead, luật và lệnh xác minh. Thẻ và prompt: [kế hoạch swarm 05/11](../ai/planning/2026-11-05-swarm-master-plan.md), [file prompt](../ai/planning/2026-11-05-swarm-prompts.md) |
| **Việc đang chạy** | [Kế hoạch swarm 05/11](../ai/planning/2026-11-05-swarm-master-plan.md) (`active`) | Danh sách thẻ theo đợt. Trạng thái từng thẻ ở [`AGENT_WORKFLOW.md` §3](../how-to/AGENT_WORKFLOW.md#3-hàng-đợi-thẻ) |
| Kế hoạch thực thi 31/10 — **đã làm xong phần lớn** | [`EXECUTION_PLAN_2026-10-31.md`](EXECUTION_PLAN_2026-10-31.md) (`proposed`) | Task HT-A / HT-B / HT-C. HT-C đã merge ở #45 ngày 2026-11-15; phần HT-B còn lại (B6) chạy tiếp dưới tên thẻ G3–G5. Giữ làm bản ghi của từng task và lệnh xác minh. Brief cho agent: [Antigravity](../ai/planning/2026-10-31-antigravity-brief.md) |
| Tách crate con | [`CRATE_SPLIT_PLAN_2026-10-31.md`](CRATE_SPLIT_PLAN_2026-10-31.md) (`proposed`) | S0–S5: phá sáu vòng phụ thuộc bằng một tầng type còn thiếu, rồi tách 6 crate. Bộ ba kiểm V1/V2/V3 mỗi bước. Brief: [Antigravity](../ai/planning/2026-10-31-antigravity-brief-crate-split.md) |
| Định hướng và điều phối | [`DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md`](DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md) (`proposed`) | Đi tới đâu, theo chặng nào, ai quyết gì (QĐ-M), cái gì phải dừng. Không thay kế hoạch thực thi 31/10; thêm nhiều máy, người dùng đầu tiên và luật điều phối giữa các kế hoạch. Dựa trên [tổng hợp nghiên cứu 13/11](../research/RESEARCH_SYNTHESIS_2026-11-13.md) |
| Hoàn thiện — đo và chẩn đoán 31/10 | [`COMPLETION_ROADMAP_2026-10-31.md`](COMPLETION_ROADMAP_2026-10-31.md) (`proposed`) | Đo 2026-10-31 trên hai cây mã và chẩn đoán phân kỳ `main` ↔ `feat/ecological-selection`: nay là **lịch sử**, vì hai cây đã hợp nhất. Phần còn sống là §4, sổ quyết định |
| Thứ tự tới bản phát hành | [`COMPLETION_ROADMAP_2026-10-16.md`](COMPLETION_ROADMAP_2026-10-16.md) (`proposed`) | Sáu giai đoạn HT0–HT5 từ hiện trạng tới phát hành, ba mức "hoàn thiện", quyết định cần chủ dự án. HT0–HT1 đã được roadmap 31/10 thay; **HT2–HT5 vẫn là nguồn chuẩn** cho khoa học và phát hành |
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
