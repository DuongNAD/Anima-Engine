---
title: ADR-0002 — World laws, exotic energy và World Lab
status: proposed
owner: simulation-architecture
last_reviewed: 2026-07-24
decision_date: pending
supersedes: none
superseded_by: none
---

# ADR-0002 — World laws, exotic energy và World Lab

## Bối cảnh

Anima Engine đã có scenario deterministic, control/treatment và causal ledger tối thiểu, nhưng
`Scenario` mới chứa seed, duration và intervention. Engine chưa biểu diễn một câu hỏi lớn hơn:

> Nếu từ `t=0` thế giới có luật hoặc nguồn năng lượng khác, lịch sử tiến hóa, niche và loài sẽ
> phân nhánh như thế nào?

Ví dụ sản phẩm đầu tiên là một nguồn năng lượng đặc biệt hiển thị với tên “mana”. Nếu hard-code
`Mana` vào organism hoặc cộng thẳng fitness, hệ thống sẽ tạo hiệu ứng game nhưng không còn là mô
phỏng nhân–quả, phá closed EU và không thể dùng cùng framework cho nguồn khác.

## Động lực quyết định

- Cùng framework phải mô tả baseline hiện tại và nhiều alternate regime.
- Tác động từ genesis phải tách khỏi intervention giữa run.
- Tắt feature phải tương thích baseline và rollback được.
- Nguồn mới phải có budget, field, source/sink và chi phí khai thác.
- Tiến hóa phải xuất hiện qua survival/reproduction, không qua genotype rewrite.
- Người dùng phải xem được biến, lineage, causal chain và control/treatment.
- Kết luận phải tái lập, multi-seed và không phụ thuộc một screenshot.

## Các phương án

### A. Hard-code `Mana` vào organism và cộng fitness

Rẻ và dễ thấy nhưng không tổng quát, không có budget, không giải thích được loài mới và vi phạm
“no magic effects”. **Bị từ chối.**

### B. Xem mana như một phần của closed EU

Ít ledger hơn nhưng trộn biomass-equivalent với một nguồn năng lượng hư cấu, khiến đơn vị và
conservation không còn rõ. **Bị từ chối.**

### C. Nguồn năng lượng riêng + pathway di truyền + experiment manifest

Tạo `WorldLawSet`, `ExoticEnergyLaw`, field/budget MU và trait khai thác có chi phí. MU không phải EU;
nó thay đổi cơ chế/rate chuyển EU chứ không tạo biomass miễn phí. Control/treatment dùng cùng
manifest trừ factor được khai báo. **Được đề xuất chọn.**

### D. Tạo một engine riêng cho “thế giới phép thuật”

Cách ly tốt nhưng nhân đôi world, ECS, save, renderer, evolution và test; các nhánh không còn so sánh
được trong cùng framework. **Bị từ chối.**

## Quyết định đề xuất

1. Thêm `WorldLawSet` versioned vào experiment/save identity.
2. Gọi khái niệm lõi là `ExoticEnergy`; “Mana” là label/config của scenario mẫu.
3. Baseline dùng `exotic_energy: None` và phải đạt AE-S01.
4. MU có ledger riêng; closed EU vẫn giữ semantics hiện tại.
5. Energy pathway là genotype/phenotype/runtime data có cost/trade-off.
6. World law được chốt trước genesis; thay giữa lịch sử tạo branch/intervention có `CauseId`.
7. Mở rộng scenario runner thành `ExperimentManifest` + ensemble + checkpoint fork.
8. Tạo `ObservableRegistry` dùng chung cho backend result và frontend World Lab.
9. “Species” là kết quả detector versioned; trước gate dùng morph/ecotype/candidate species.
10. Triển khai headless reference slice trước, sau đó mới nối live Bevy và renderer.

## Hệ quả

### Tích cực

- Có thể thử mana, bức xạ, hóa năng hoặc nguồn hư cấu khác mà không đổi kiến trúc.
- Baseline và alternate regime so sánh được từ cùng initial state.
- Causal Explorer giải thích được từ world law tới trait frequency/speciation.
- Rollback đơn giản bằng `exotic_energy=None`.
- Giữ đúng closed EU và Creature Development Contract.

### Tiêu cực / chi phí

- Scenario/save schema lớn hơn và cần migration.
- `SimModel::Default` không đủ; model phải khởi tạo từ manifest/snapshot.
- Causal ledger một-parent hiện tại cần mở rộng cho multi-cause contribution.
- Live Bevy determinism là dependency cứng cho World Lab đầy đủ.
- Species detection cần reproduction/lineage dài hạn và hiệu chỉnh threshold.
- UI có nhiều lớp dữ liệu; phải có sampling/downsample để không vượt ngân sách.

## Rollout và rollback

1. Contract/schema/headless manifest, feature flag off.
2. Exotic field + budget trong reference model.
3. Pathway trait + selection experiment headless.
4. Live Bevy adapter và persistence.
5. World Lab UI đọc artifact kết quả.
6. Speciation detector chỉ bật sau ensemble gate.

Rollback không xóa reader/schema. Tắt exotic law, giữ parser cho artifact cũ và xác minh AE-S01.

## Bằng chứng cần trước khi accepted

| Hạng mục | Bằng chứng |
|---|---|
| Baseline compatibility | AE-S01 |
| Budget semantics | AE-S04/AE-S05 |
| Evolution mechanism | AE-S06/AE-S07/AE-S10 |
| Experiment reproducibility | AE-S02/AE-S03/AE-S08/AE-S09 |
| Observability | AE-S12/AE-S13/AE-S14 |
| Persistence | AE-S15 |
| Map/ecology placement | Animal Map Vision; hiện blocked vì MCP không khả dụng |

## Tài liệu liên quan

- [Evolution Experiment Contract](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md)
- [Alternate Evolutionary Regimes](../explanation/ALTERNATE_EVOLUTIONARY_REGIMES.md)
- [Feature requirements](../ai/requirements/2026-07-24-feature-alternate-evolution-world-lab.md)
- [Feature design](../ai/design/2026-07-24-feature-alternate-evolution-world-lab.md)
- [Feature plan](../ai/planning/2026-07-24-feature-alternate-evolution-world-lab.md)
- [Creature Development Contract](../reference/CREATURE_DEVELOPMENT_CONTRACT.md)
