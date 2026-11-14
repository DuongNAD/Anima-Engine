---
title: Trung tâm tài liệu Anima Engine
status: active
owner: maintainers
last_reviewed: 2026-11-14
review_cycle: per-release
---

# Trung tâm tài liệu Anima Engine

Tài liệu được tổ chức theo Diátaxis: **tutorial** để học theo lộ trình, **how-to** để
hoàn thành một việc, **reference** để tra cứu hợp đồng, và **explanation** để hiểu lý
do thiết kế. Kế hoạch, nghiên cứu, quyết định và quản trị được tách riêng để không
trộn “điều đang đúng” với “điều dự kiến làm”.

## Bản đồ tài liệu

```mermaid
flowchart TD
    H["README.md — điểm vào"] --> D["docs/README.md — mục lục"]
    D --> T["tutorials — học theo lộ trình"]
    D --> W["how-to — hoàn thành tác vụ"]
    D --> R["reference — hợp đồng chuẩn"]
    D --> E["explanation — lý do và mô hình"]
    D --> P["planning — lộ trình và công việc"]
    D --> X["research — bằng chứng bên ngoài"]
    D --> A["decisions — ADR"]
    D --> G["governance — quy tắc duy trì"]
    D --> L["ai — lifecycle feature cho agent"]
    R --> C["Code + schemas + fixtures"]
    P --> A
    X --> P
    A --> C
    L --> R
    L --> A
```

## Lối vào theo nhu cầu

| Tôi muốn… | Đi tới |
|---|---|
| **Biết dự án đang ở đâu và làm gì tiếp** | [`STATE_OF_THE_PROJECT.md`](planning/STATE_OF_THE_PROJECT.md) — đọc banner đầu §1 trước |
| **Giao việc cho coding agent** | [Hướng dẫn chạy agent](how-to/AGENT_WORKFLOW.md) — hàng đợi thẻ, quyết định đã điền, luật, lệnh xác minh |
| Chạy và hiểu dự án lần đầu | [Tutorials](tutorials/README.md) |
| Build, test, benchmark hoặc tạo artifact | [How-to](how-to/README.md) |
| Tra cứu luật mô phỏng, biome, tọa độ, manifest | [Reference](reference/README.md) |
| Sửa genotype, phenotype, spawn, save hoặc migration sinh vật | [Creature Development Contract](reference/CREATURE_DEVELOPMENT_CONTRACT.md) |
| Tạo thế giới khác luật, nguồn “mana”, fork và so sánh tiến hóa | [Evolution Experiment Contract](reference/EVOLUTION_EXPERIMENT_CONTRACT.md) |
| Sửa bộ não agent, gen não, hoặc không gian hành động | [ADR-0003](decisions/ADR-0003-evolved-per-agent-brains.md) |
| Xem requirements/design/testing/task của feature đang phát triển | [Chỉ mục `docs/ai`](ai/README.md) — mọi feature, mọi pha, kèm trạng thái |
| Tra bug thật đã chứng minh, có lệnh tái hiện | [`FINDINGS.md`](../FINDINGS.md) |
| Hiểu kiến trúc và nguyên nhân của quyết định | [Explanation](explanation/README.md) |
| Xem việc cần làm và thứ tự phụ thuộc | [Planning](planning/README.md) |
| Xem bằng chứng bên ngoài và khảo sát | [Research](research/README.md) |
| Xem đánh giá dự án nguồn mở | [Open-source landscape](research/OPEN_SOURCE_LANDSCAPE.md) |
| Xem đề xuất nâng cấp map và mô hình machine | [Map & ML upgrade research](research/MAP_AND_ML_UPGRADE_RESEARCH.md) *(proposed)* |
| Ghi hoặc tra cứu quyết định kiến trúc | [ADR index](decisions/README.md) |
| Xem quy tắc duy trì hệ thống | [Governance](governance/README.md) |
| Duy trì và tái cấu trúc tài liệu | [Documentation policy](governance/DOCUMENTATION_POLICY.md) |
| Thêm dependency hoặc sao chép mã/asset | [Open-source policy](governance/OPEN_SOURCE_POLICY.md) |
| Truy nguyên một tài liệu cũ đã bị thay | [Archive](archive/README.md) — không dùng làm nguồn triển khai |

## Nguồn sự thật

| Chủ đề | Nguồn chuẩn hiện tại |
|---|---|
| Phạm vi sản phẩm và kiến trúc | [`PROJECT.md`](../PROJECT.md) |
| Tầm nhìn thế giới | [`WORLD_DESIGN.md`](../WORLD_DESIGN.md) |
| Luật, đơn vị, bảo toàn và thứ tự tick | [`SIMULATION_RULES.md`](../SIMULATION_RULES.md) |
| Phân loại biome | [`BIOME_TAXONOMY.md`](../BIOME_TAXONOMY.md) |
| Hệ tọa độ và phép biến đổi | [`COORDINATE_CONTRACT.md`](../COORDINATE_CONTRACT.md) |
| Artifact ảnh và góc nhìn chuẩn | [`MAP_MANIFEST.md`](../MAP_MANIFEST.md) |
| Baseline hiệu năng | [`BENCHMARK_BASELINE.md`](../BENCHMARK_BASELINE.md) |
| Lộ trình mô phỏng | [`WORLD_SIMULATION_PLAN.md`](../WORLD_SIMULATION_PLAN.md) |
| Trạng thái đo được và backlog ưu tiên | [`STATE_OF_THE_PROJECT.md`](planning/STATE_OF_THE_PROJECT.md) §1 và §3 |
| Nhật ký công việc (đã làm gì, khi nào) | [`TODO.md`](../TODO.md) |
| Bug thật đã chứng minh | [`FINDINGS.md`](../FINDINGS.md) |
| Vòng đời genotype → phenotype → spawn | [`CREATURE_DEVELOPMENT_CONTRACT.md`](reference/CREATURE_DEVELOPMENT_CONTRACT.md) |
| World laws, exotic energy và thí nghiệm tiến hóa | [`EVOLUTION_EXPERIMENT_CONTRACT.md`](reference/EVOLUTION_EXPERIMENT_CONTRACT.md) *(proposed)* |
| Tất định, sổ cái năng lượng, snapshot | [Reference](reference/README.md) — ba hợp đồng `active` |

Các tài liệu chuẩn đang ở thư mục gốc được giữ nguyên để không phá liên kết và thay
đổi đang thực hiện. Việc di chuyển chỉ diễn ra theo từng đợt có redirect stub, được
mô tả trong [chính sách tài liệu](governance/DOCUMENTATION_POLICY.md).
