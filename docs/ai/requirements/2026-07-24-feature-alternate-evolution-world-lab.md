---
phase: requirements
feature: alternate-evolution-world-lab
title: Requirements — Alternate Evolution & World Lab
description: Thế giới khác luật tạo lịch sử tiến hóa khác và có thể quan sát, replay, so sánh
status: proposed
owner: simulation-architecture
last_reviewed: 2026-07-25
contract: ../../reference/EVOLUTION_EXPERIMENT_CONTRACT.md
---

# Requirements — Alternate Evolution & World Lab

## Shared understanding

Người dùng muốn thay một điều kiện nền **từ sớm nhất** rồi xem toàn bộ lịch sử thế giới phân nhánh.
Ví dụ:

- nhánh A dùng thế giới hiện tại, không có năng lượng đặc biệt;
- nhánh B có một nguồn năng lượng hư cấu gọi là “mana”;
- sinh vật/thực vật ở B có thể tiến hóa khả năng cảm nhận, khai thác và phụ thuộc nguồn đó;
- qua nhiều thế hệ, B có thể xuất hiện ecotype, niche, food web và candidate species khác A;
- người dùng phải xem được biến nào đổi, đổi khi nào, qua cơ chế nào và kết luận có ổn định qua nhiều
  seed hay không.

Đây là yêu cầu về **alternate evolutionary regimes**, không phải yêu cầu thêm một buff Mana.

## Problem statement

### Vấn đề sản phẩm

Anima Engine hiện mô tả tốt câu hỏi “một intervention làm aggregate nào đổi” trên
`ReferenceEcosystem`, nhưng chưa cho người dùng:

- khai báo luật/nguồn của thế giới trước genesis;
- fork hai lịch sử từ cùng initial state;
- quan sát trait/lineage/species divergence;
- kiểm soát biến và chạy ensemble;
- phân biệt khác biệt thật do treatment với khác biệt ngẫu nhiên.

### Vấn đề kỹ thuật

- `Scenario` chưa chứa `WorldLawSet`, initial conditions, snapshot hoặc seed set.
- `SimModel::Default` không nhận cấu hình world law.
- Exotic energy chưa có field, unit, budget hoặc persistence.
- Genotype chưa có energy pathway; reproduction/species model chưa hoàn chỉnh.
- Live Bevy world chưa chạy qua scenario runner deterministic.
- Causal ledger chưa đủ cho multi-cause evolutionary attribution.
- Dashboard chưa có variable registry, branch tree hoặc run comparison.

### Người dùng mục tiêu

- Chủ dự án muốn thử “nếu thế giới khác từ đầu thì sao”.
- Người thiết kế game muốn tạo world fantasy có luật nhất quán.
- Người phát triển mô phỏng muốn debug parameter và causal chain.
- Người quan sát muốn xem sự sống thay đổi mà không cần đọc raw logs.

## Goals

### G1 — World law có thể cấu hình

Biểu diễn baseline và alternate regime bằng một schema versioned. “Mana” là một
`ExoticEnergyLaw`, không phải enum hard-code duy nhất.

### G2 — Evolution phải emergent

Nguồn mới tác động qua field, physiology, behavior, survival và reproduction. Không sửa genotype,
species id, population hoặc fitness trực tiếp.

### G3 — Giữ budget và baseline

Closed EU giữ contract hiện tại. MU có budget riêng. Khi exotic energy bị tắt, thế giới tương thích
baseline và không có cost/path ẩn.

### G4 — Thí nghiệm tái lập và phân nhánh được

Hỗ trợ:

- genesis fork;
- checkpoint fork;
- control/treatment;
- factorial factors;
- multi-seed ensemble;
- replay cùng checksum.

### G5 — Quan sát được mọi lớp quan trọng

Người dùng xem được:

- world/cell fields;
- energy flows;
- organism genotype/phenotype/runtime;
- lineage/trait/species dynamics;
- causal graph;
- control–treatment delta và uncertainty.

### G6 — Tích hợp từng bước

Chứng minh contract trên headless reference slice trước; nối live Bevy, persistence và UI sau mà
không đổi format thí nghiệm.

## Non-goals

- Spell system, combat magic, skill tree hoặc VFX phép thuật.
- Tạo class `ManaRabbit`/`ManaWolf` hard-code.
- Tuyên bố mô hình hư cấu là sinh học thực.
- Full physical mass/thermodynamic simulation trong MVP.
- Materialize biomass trực tiếp từ MU.
- Auto-tune mọi parameter bằng AI.
- Triển khai aerial/swimming/species taxonomy đầy đủ trong cùng phase.
- Kết luận từ một seed, một screenshot hoặc một fitness proxy.

## User stories

### US1 — Tạo hai lịch sử từ genesis

Là người thử nghiệm, tôi chọn cùng map/seed, bật Mana ở treatment và chạy control không Mana để xem
lịch sử tiến hóa khác nhau.

### US2 — Thay một biến, khóa các biến còn lại

Tôi có thể xem diff của hai manifest và hệ thống từ chối comparison nếu artifact, seed schedule hoặc
model version khác ngoài factor được khai báo.

### US3 — Quan sát field và transaction

Tôi bật layer Mana để xem density/source/uptake/depletion và click một cell để xem budget/time series.

### US4 — Quan sát sinh vật và lineage

Tôi click organism/lineage để xem pathway genotype, phenotype cost, storage runtime, ancestors và
reproductive success.

### US5 — Xem vì sao một trait tăng

Tôi chọn trait frequency và thấy causal chain:

```text
Mana hotspot → uptake advantage → net energy/work advantage
→ survival/reproduction delta → offspring trait frequency tăng
```

### US6 — Fork tại checkpoint

Sau 100 thế hệ tôi snapshot, cho một nhánh tiếp tục và rút Mana ở nhánh còn lại để đo dependency,
extinction debt và recovery.

### US7 — Đánh giá candidate species

Tôi xem detector giải thích vì sao một cluster được gọi là morph, ecotype hay candidate species, với
threshold/version/evidence rõ ràng.

### US8 — Chạy ensemble

Tôi chọn 5–30 seed, chạy batch, xem effect size/confidence interval và các run lỗi thay vì một đường
cong duy nhất.

### US9 — Replay và export

Tôi lưu/export manifest + result, replay trên cùng build và nhận cùng checksum.

## Functional requirements

| ID | Yêu cầu |
|---|---|
| FR-01 | `WorldLawSet` versioned và fingerprinted |
| FR-02 | `WorldLawSet.exotic_energy=None` là disabled path duy nhất; `Some(ExoticEnergyLaw)` hỗ trợ ít nhất `Renewable` với `Uniform/Patchy` |
| FR-03 | `ExperimentManifest` chứa artifact, laws, initial state, interventions, seeds, sampling, metrics |
| FR-04 | Runner hỗ trợ genesis fork và checkpoint fork |
| FR-05 | Exotic field có source/sink/uptake/storage/balance audit |
| FR-06 | Energy pathway là trait di truyền bounded với explicit cost |
| FR-07 | Save/migration giữ law fingerprint, field và organism storage |
| FR-08 | Observable registry là nguồn chung backend/UI/export |
| FR-09 | Result chứa time series, budget, causal ledger, lineage/speciation events, checksum |
| FR-10 | Compare view hiển thị factor diff, aligned timeline, delta và uncertainty |
| FR-11 | Species detector versioned và không dựa riêng morphology/color |
| FR-12 | User có thể inspect world/cell/organism/lineage/species/run |
| FR-13 | Feature có `exotic_energy=None` rollback path |
| FR-14 | Map/ecology visualization dùng dữ liệu sim quyền lực, không dữ liệu trang trí |

## Success criteria

| ID | Kết quả đo được |
|---|---|
| SC-01 | AE-S01: exotic disabled giữ baseline checksum/tolerance |
| SC-02 | AE-S02: replay cùng manifest + seed có divergence 0 |
| SC-03 | AE-S04/05: MU và EU audit không drift ngoài tolerance |
| SC-04 | AE-S06: pathway có cost đo được khi nguồn vắng |
| SC-05 | AE-S07/10: treatment đổi performance rồi reproductive/trait frequency qua cơ chế |
| SC-06 | AE-S08/09: genesis/checkpoint fork khóa initial state đúng |
| SC-07 | AE-S12: selected KPI có causal trace tới law/intervention |
| SC-08 | AE-S13: backend result, chart và map layer dùng cùng observable id/unit |
| SC-09 | AE-S14: ensemble báo N, effect size, interval và failures |
| SC-10 | Không dùng từ “species” trước khi AE-S11 và policy gate pass |
| SC-11 | Reference vertical slice hoàn tất trước live Bevy adapter |
| SC-12 | Map gate không còn critical/high trước khi tuyên bố visual/ecology hoàn tất |

## UX requirements

- Start screen phân biệt rõ **world law** với **intervention**.
- Mọi control hiển thị unit, range, default và “restart/branch required”.
- Trước khi run, UI hiển thị manifest diff giữa control/treatment.
- Trong run có layer legend, timeline cursor và inspector.
- Compare view đồng bộ time axis/generation axis.
- Cảnh báo khi sample cadence làm bỏ lỡ transient hoặc khi ensemble quá nhỏ.
- Species label kèm confidence/evidence state.
- Budget drift/NaN/run failure hiển thị rõ, không ẩn khỏi summary.
- Máy yếu có headless/batch mode và downsample UI.

## Constraints

- Rust + Bevy ECS backend; React/Pixi/Three frontend.
- Hot loop không allocation mới; field dùng SoA/preallocation.
- Current live Bevy path còn `thread_rng()` và chưa implement `SimModel`.
- Closed EU trong `SIMULATION_RULES.md` là contract accepted.
- Creature changes phải tuân `CREATURE_DEVELOPMENT_CONTRACT.md`.
- Một World Artifact là nguồn terrain/biome/water quyền lực.
- Save/schema/IPC phải versioned và có legacy defaults/migration.
- Kết luận tiến hóa dùng multi-seed; S43 vẫn dành cho Red-Queen.
- Map claims bị blocked khi `animal-map-vision` MCP không khả dụng.

## Chosen assumptions

Các quyết định dưới đây được chấp nhận cho MVP để không còn câu hỏi material chặn thiết kế:

1. Core gọi nguồn là `ExoticEnergy`; UI scenario mẫu gọi là “Mana”.
2. Unit MVP là `MU`; không đồng nhất với EU.
3. MVP hỗ trợ disabled qua `WorldLawSet.exotic_energy=None`; mọi `Some(ExoticEnergyLaw)` là
   `Renewable` với topology `Uniform`/`Patchy`. Không có enum variant `Disabled`.
4. MU không materialize biomass; nó chỉ cấp work/catalysis có transaction rõ.
5. World law bất biến trong run; mọi thay đổi tạo branch hoặc intervention.
6. Species detection ban đầu là diagnostic/candidate, không đổi mating rules.
7. Reference headless model là bước đầu; claim về live world chỉ sau Bevy adapter.
8. Threshold speciation là parameter versioned, hiệu chỉnh sau; không hard-code vào tài liệu.
9. UI MVP ưu tiên 2D layer/charts; 3D VFX là phase sau.

## Deferred questions

| Câu hỏi | Trạng thái/điều kiện mở lại |
|---|---|
| MU có thể tạo vật chất không? | Deferred; cần ADR tách mass/physical energy |
| Full multi-parent causal attribution | Deferred tới khi AE-S12 single-root slice ổn định |
| Reproductive isolation thật | Deferred tới reproduction/mate choice M7 |
| Default scientific/fantasy parameter values | Deferred tới sensitivity analysis; không chặn schema |
| Distributed ensemble/sharding | Deferred sau single-machine deterministic runner |

## Requirement trace

- Contract: [EVOLUTION_EXPERIMENT_CONTRACT.md](../../reference/EVOLUTION_EXPERIMENT_CONTRACT.md)
- Decision: [ADR-0002](../../decisions/ADR-0002-world-laws-and-exotic-energy.md)
- Design: [feature design](../design/2026-07-24-feature-alternate-evolution-world-lab.md)
- Testing: [testing strategy](../testing/2026-07-24-feature-alternate-evolution-world-lab.md)
- Plan: [task plan](../planning/2026-07-24-feature-alternate-evolution-world-lab.md)
