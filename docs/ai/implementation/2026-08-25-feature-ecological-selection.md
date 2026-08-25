---
phase: implementation
feature: ecological-selection
title: Task Backlog — Đóng vòng sinh thái
description: Danh sách đầy đủ việc cần làm, có neo file, phụ thuộc và định nghĩa hoàn thành cho từng mục
status: proposed
owner: simulation-architecture
last_reviewed: 2026-08-25
requirements: ../requirements/2026-08-25-feature-ecological-selection.md
design: ../design/2026-08-25-feature-ecological-selection.md
planning: ../planning/2026-08-25-feature-ecological-selection.md
testing: ../testing/2026-08-25-feature-ecological-selection.md
handoff: ../planning/2026-08-25-agent-goal-ecological-selection.md
---

# Task Backlog — Đóng vòng sinh thái

Mỗi mục có: **neo file**, **phụ thuộc**, và **định nghĩa hoàn thành** đo được. Một mục không có
định nghĩa hoàn thành đo được là một mục chưa sẵn sàng để giao.

> Trạng thái đo được của toàn dự án nằm ở **một chỗ duy nhất**:
> [`STATE_OF_THE_PROJECT.md` §1](../../planning/STATE_OF_THE_PROJECT.md). Đừng chép số sang đây.

**Ký hiệu:** ⬜ chưa làm · 🔒 bị chặn · ⚠️ có bẫy, đọc cột ghi chú.

---

## 0. Sự thật hiện trạng (đọc mã 2026-08-25, chưa chạy lệnh)

Kiểm lại bảng này trước khi bắt đầu. Nếu một dòng đã lệch thì kế hoạch lệch theo.

| # | Sự thật | Bằng chứng | Lệnh kiểm |
|---|---|---|---|
| 1 | Có **đúng một** dòng gán fitness trong cả crate | `core/agent_systems.rs:528` | `grep -rn 'fitness =' src-tauri/src --include='*.rs'` |
| 2 | `tick_count` tăng vô điều kiện | `core/world_systems.rs:171` | đọc mã |
| 3 | `energy <= 0` chỉ `continue` | `core/agent_systems.rs:452` | `grep -rn 'energy <= 0' src-tauri/src` |
| 4 | `AgentEvaluation.survival_ticks` **có** điều kiện sống | `core/agent_systems.rs:452-462` | đọc mã |
| 5 | `check_epoch_completion_system` vứt `_eval, _homeo` | `core/agent_systems.rs:523` | đọc mã |
| 6 | `foraging_range == cumulative_distance` | `core/agent_systems.rs:531` | `sed -n '526,532p'` |
| 7 | `ReclaimAndDespawnAgentCommand` đã có và nguyên tử | `core/agent_systems.rs:112-152` | đọc mã |
| 8 | `EcosystemBiomass` có **bốn** ngăn (`dormant` là ngăn thứ tư, F1) | `crates/anima-domain/src/energy.rs:19-38` | đọc mã |
| 9 | Bảy hệ thống O(agent × segment) | xem §6 | `grep -rn 'segment_query.iter()' src-tauri/src` |
| 10 | `combat_system` **đã** dùng mẫu chỉ mục đúng | `core/world_systems.rs:300-340` | đọc mã |
| 11 | Observable hiện tại **không cái nào phát hiện được tiến hoá** | `core/live_experiment.rs:721-794` | đọc mã |

> Đính chính 2026-08-25 (chặng 2): số observable hôm nay là **12**, không phải 11 như bảng gốc ghi —
> `live.dormant_eu` được nhánh living-world F1 thêm vào `LIVE_OBSERVABLE_IDS`. Đối chứng âm CA-S03
> áp cho **cả 12**. Số dòng cũng trôi: `LIVE_OBSERVABLE_IDS` nay ở `core/live_experiment.rs:718`.
>
> Cũng đính chính: `core/experiment_runner.rs` nay **4477 dòng** (bảng P2 §3.12 của
> STATE_OF_THE_PROJECT ghi 3173). **Không tách file trong chặng sinh thái** — một diff dời file
> khổng lồ cùng lúc với cổng "checksum trùng bit" là cách chắc chắn nhất để không ai audit được
> cái gì. Tách sau khi vòng sinh thái đóng.
| 12 | Shannon/Simpson đã tính, chỉ chảy vào UI | `core/simulation_loop.rs:2192` | đọc mã |
| 13 | Quần thể mặc định **10**, epoch **1000** tick | `core/resources.rs:1215`, `core/simulation_loop.rs:1323` | đọc mã |

---

## 1. M — Tử vong

| ID | Trạng thái | Việc | Neo | Phụ thuộc | Định nghĩa hoàn thành | Ghi chú |
|---|---|---|---|---|---|---|
| M-1 | ✅ | `starving_ticks: u32` vào `HomeostaticState`, `#[serde(default)]` | `src-tauri/src/ai/hrrl.rs:6` | — | snapshot cũ nạp được, round-trip giữ giá trị | ⚠️ thiếu `serde(default)` ⇒ mọi save cũ hỏng |
| M-2 | ✅ | Tăng khi ở đáy, **reset về 0 khi ăn/uống được** | `src-tauri/src/core/world_systems.rs:121-178` | M-1 | unit test: ăn một lần ⇒ bộ đếm về 0 | ⚠️ quên reset ⇒ mọi con chết sau `starve_ticks` bất kể ăn bao nhiêu |
| M-3 | ✅ | `MortalityPolicy` + `ANIMA_MORTALITY` | `src-tauri/src/core/resources.rs` | — | cờ unset ⇒ checksum trùng bit | dùng `Option<Res<…>>`, xử lý `None` tường minh |
| M-4 | ✅ | `mortality_system` → `ReclaimAndDespawnAgentCommand` | file mới `src-tauri/src/core/mortality.rs` | M-1…M-3 | MO-S01…MO-S04 xanh | ⚠️ **không** trừ năng lượng ở đây; command tự làm |
| M-5 | ✅ | Khai lịch trình: sau `metabolic_decay`, trước `ecosystem_census` | `src-tauri/src/core/simulation_schedule.rs` | M-4 | test thứ tự; census đếm thế giới sau khi xác thành detritus | ⚠️ khai ở **một** chỗ duy nhất — `build_tick_schedule` |
| M-6 | ✅ | Cập nhật `ENERGY_LEDGER_CONTRACT` nếu đường EU mới xuất hiện | `docs/reference/ENERGY_LEDGER_CONTRACT.md` | M-4 | — | nếu làm đúng thì **không** có đường mới, và đó là câu trả lời |

---

## 2. F — Fitness sinh thái

| ID | Trạng thái | Việc | Neo | Phụ thuộc | Định nghĩa hoàn thành | Ghi chú |
|---|---|---|---|---|---|---|
| F-1 | ✅ | `trait FitnessRule` + `LocomotionFitness` chép **nguyên văn** | file mới `src-tauri/src/evolution/fitness.rs` | — | FI-S01: cùng seed ⇒ checksum trùng bit | ⚠️ đổi thứ tự hai số hạng `f32` cũng có thể đổi làm tròn |
| F-2 | ✅ | `EcologicalFitness` đọc `AgentEvaluation` | cùng file | F-1 | FI-S02 | không có số hạng quãng đường — nếu có, P-04 quay lại |
| F-3 | ✅ | Bỏ `_` ở `_eval, _homeo`, truyền vào luật | `src-tauri/src/core/agent_systems.rs:523` | F-1 | biên dịch, FI-S01 | ⚠️ **không** thêm query thứ hai; query hiện tại đã có cả hai |
| F-4 | ✅ | `ANIMA_FITNESS` + trường trong `ExperimentManifest` | `src-tauri/src/core/resources.rs`, `src-tauri/src/core/experiment.rs` | F-1 | manifest có trường, preflight từ chối giá trị lạ | ✏️ chặng 2: cài qua **khoá điều kiện đầu khai báo** (`live.ecological_fitness`, `live.w_surplus`) thay vì một trường cấu trúc mới — initial_conditions đã nằm trong fingerprint manifest và được preflight từ chối khoá lạ; tránh hai nguồn sự thật. `parse_rule` từ chối id lạ |
| F-5 | ✅ | `id()` vào tick capture export + provenance ensemble | `src-tauri/src/core/tick_capture.rs` | F-4 | FI-S04: mọi report nói được nó chấm bằng luật nào | |
| F-6 | ✅ | Sinh lại binding `ts-rs` nếu payload đổi | `src/types/generated/` | F-4 | `git diff --exit-code -- src/types/generated` | **không sửa tay** thư mục này |

---

## 3. Q — Trục mô tả độc lập với fitness

| ID | Trạng thái | Việc | Neo | Phụ thuộc | Định nghĩa hoàn thành | Ghi chú |
|---|---|---|---|---|---|---|
| Q-1 | ✅ | Bộ đếm EU nạp theo nguồn (thực vật / mồi) | `src-tauri/src/core/components.rs` | — | unit test cộng đúng | ✅ chặng 2: `AgentEvaluation.prey_energy_gained` ghi tại 2 điểm săn mồi (ledger tự đo credit); gặm cỏ/food đi vào `total_energy_gained` |
| Q-2 | ✅ | `trophic_ratio` từ bộ đếm | `src-tauri/src/core/ecology.rs` | Q-1 | QD-S01 | ✅ chia cho 0 ⇒ 0.0 định nghĩa tường minh; NaN/âm bị chặn ở đầu vào |
| Q-3 | ✅ | Đổi trục 2 của `ecological_descriptors`, sửa doc comment | `src-tauri/src/core/ecology.rs:170` | F-2, Q-2 | QD-S02: trục **không** tương quan đơn điệu với fitness | ✅ trục 2 = `trophic_ratio`; doc comment viết lại; caller duy nhất (`simulation_loop.rs:715`) chuyển sang stats intake |
| Q-4 | ✅ | `descriptor_set_id`; `from_saved` **từ chối** id lệch | `src-tauri/src/evolution/map_elites.rs:58-110` | Q-3 | QD-S03: nạp archive v1 vào v2 là **lỗi**, không phải kho lệch | ✅ `body-mass+trophic-ratio/v2`; serde(default) để save cũ đọc thành rỗng rồi bị TỪ CHỐI |
| Q-5 | ✅ | Kiểm bão hoà `FORAGING_REFERENCE`/trục mới | `src-tauri/src/core/ecology.rs:161` | Q-3 | có số đo phân bố trục, không phải phỏng đoán | ✅ FORAGING_REFERENCE xoá; trục mới là tỉ lệ [0,1] sẵn nên không còn điểm bão hoà nhân tạo nào để kiểm |

> Cập nhật chặng 2 (2026-08-25): Q-1…Q-5 ✅. Lưu ý: `foraging_range` vẫn giữ trong
> `AgentEpochStats` cho telemetry nhưng **không còn** là trục niche. CA-S03/QD-S02 được chứng minh
> bằng số học descriptor (kênh độc lập với fitness theo cấu trúc); phân bố trục trên run thật
> cần nhánh R để quần thể thay đổi thế hệ (E2-F3).

---

## 4. R — Sinh sản theo ngưỡng

| ID | Trạng thái | Việc | Neo | Phụ thuộc | Định nghĩa hoàn thành | Ghi chú |
|---|---|---|---|---|---|---|
| R-1 | 🔒 | `ReproductionPolicy` + `ANIMA_REPRODUCTION` | `src-tauri/src/core/resources.rs` | M, F xanh | cờ unset ⇒ epoch replacement như cũ | |
| R-2 | 🔒 | `SpawnOffspringCommand` — **một** command, hai vế ledger | `src-tauri/src/core/agent_systems.rs` | R-1 | RE-S01: residual ≤ 1e-3 sau 120k tick | ⚠️ hai bước qua `Commands` = đúng chế độ hỏng mà `ReclaimAndDespawn` đã sửa |
| R-3 | 🔒 | `reproduction_system` + trần + `births_suppressed_by_cap` | file mới | R-2 | RE-S02, RE-S03 | ⚠️ trần im lặng làm mọi kết luận quần thể sai không phát hiện được |
| R-4 | 🔒 | Ghi cạnh lineage **trong cùng** command | `src-tauri/src/evolution/lineage.rs` | R-2 | RE-S04: không cạnh mồ côi | ⚠️ một cạnh mồ côi phá **toàn bộ** đồ thị: `to_newick` và `simplify` đều từ chối |
| R-5 | 🔒 | Preflight: R + epoch replacement cùng bật là lỗi | `src-tauri/src/core/experiment.rs` | R-1 | RE-S05 | |
| R-6 | 🔒 | Không Lamarck: con nhận genome, không nhận `learned` | `src-tauri/src/evolution/brain_genotype.rs` | R-2 | test khẳng định `.genotype` không bị ghi | [ADR-0003](../../decisions/ADR-0003-evolved-per-agent-brains.md) |

---

## 5. O — Quan sát được tiến hoá

| ID | Trạng thái | Việc | Neo | Phụ thuộc | Định nghĩa hoàn thành | Ghi chú |
|---|---|---|---|---|---|---|
| O-1 | ✅ | Nối Shannon/Simpson vào observable | `src-tauri/src/core/simulation_loop.rs:2192` → `src-tauri/src/core/live_experiment.rs:721` | — | OB-S01 | rẻ nhất trong nhánh: số đã tính rồi |
| O-2 | ✅ | Phân bố khối lượng: mean/var/p10/p90 | `src-tauri/src/core/live_experiment.rs` | — | OB-S02: hai quần thể cùng trung bình, khác phương sai ⇒ observable khác nhau | đây là test chứng minh vì sao trung bình không đủ |
| O-3 | ✅ | `births`/`deaths`/`mean_lifespan` theo cửa sổ | cùng file | M-4 (và R-3 cho births) | OB-S03 | ⚠️ đếm theo cửa sổ ≠ tích luỹ; đừng chép quy tắc `Aggregation` của `evolution.births` |
| O-4 | ✅ | `archive_coverage` | `src-tauri/src/evolution/map_elites.rs` | — | OB-S04 | |
| O-5 | ✅ | Preflight fail khi xin observable mà cấu trúc không tồn tại | `src-tauri/src/core/experiment.rs` | O-3 | OB-S05: manifest xin `live.deaths` trên run tắt tử vong ⇒ **fail**, không trả 0 | ⚠️ trả 0 tệ hơn fail: nó trông như một phép đo |

> Cập nhật chặng 2 (2026-08-25): O-1…O-5 ✅. Ghi chú phạm vi: `live.births` được nối vào đúng
> đường thay thế thế giới (`apply_staggered_evolution_system`) nên **đếm được trong app**, nhưng
> headless nó luôn = 0 cho tới khi nhánh R hoặc một evolution pump trong world tồn tại (E2-F3 —
> cùng rào chắn đã ghi ở CA-S01). `live.mean_lifespan` đọc `survival_ticks` của agent lúc chết,
> không thêm trường born_tick. Emission tách thành legacy 12 (đóng băng bởi prereg E2) +
> extended 10; registry version 1→2; ba test census (e2_seam, live_cross_process_probe,
> live_experiment_tests) cập nhật theo tập đầy đủ.

---

## 6. H — Hiệu năng

Mẫu chép: `combat_system` tại `src-tauri/src/core/world_systems.rs:300-340`.

| ID | Trạng thái | Việc | Neo | Định nghĩa hoàn thành |
|---|---|---|---|---|
| H-1 | ✅ | `metabolic_decay_system` | `src-tauri/src/core/world_systems.rs:121` | PF-S01 + `allocs == 0` |
| H-2 | ✅ | `update_agent_evaluation_system` | `src-tauri/src/core/agent_systems.rs:468` | PF-S01 |
| H-3 | ✅ | `detect_food_collisions_system` (hai vòng: `:236` segment, `:248` food) | `src-tauri/src/core/world_systems.rs:236` | PF-S01 |
| H-4 | ✅ | Ba chỗ còn lại trong `world_systems` | `src-tauri/src/core/world_systems.rs:337, 436, 454` | PF-S01 |
| H-5 | ✅ | `detect_environmental_collisions_system` | `src-tauri/src/core/environmental_systems.rs:282` | PF-S01 |
| H-6 | ✅ | `sensory_system` hỏi `SpatialHashGrid` | `src-tauri/src/core/agent_systems.rs:877, 890` | PF-S01, PF-S02 |
| H-7 | ⬜ | Đo `full_tick` p50 ở 1000 agent, **bản release** | `BENCHMARK_BASELINE.md` | PF-S03 — **cần người bấm chạy**, agent dừng ở đây |

> Cập nhật chặng 2 (2026-08-25): H-1…H-6 ✅. **PF-S01 chứng minh bằng probe**: checksum 1500 tick
> / seed 0x5EED, trước-H = sau-H = **4013327208**, trùng bit (probe: `tests/h_probe.rs`, so sánh
> cây làm việc với phiên bản `d8b39e5` của ba file bị sửa). Ghi chú H-6: hai vòng quét
> nearest-target được thay bằng cache snapshot một lượt duyệt theo thứ tự query — giữ nguyên kết
> quả chọn mục tiêu bit-for-bit; **không** truy vấn SpatialHashGrid trực tiếp vì grid sẽ đổi quy
> tắc phân giải khi hai mục tiêu cách đều (founders sinh trên một đường thẳng đối xứng — tie xảy
> ra thật); muốn dùng grid cần khai báo tiêu chuẩn tie-break trước, đó là quyết định thiết kế.

> ⚠️ **Bẫy chung của cả nhánh H.** `EntityHashMap` **không** duyệt theo thứ tự chèn. Chỗ nào kết
> quả phụ thuộc thứ tự duyệt — điển hình là ai giành được miếng ăn cuối cùng trong một ô — phải
> sắp xếp tường minh. Nếu checksum đổi sau một thay đổi thuộc H, đó là bug của H, không phải hệ
> quả phụ chấp nhận được.

---

## 7. T — Công cụ (tuỳ chọn, không chặn gì)

| ID | Trạng thái | Việc | Neo | Vì sao đáng làm |
|---|---|---|---|---|
| T-1 | ⬜ | `anima-lab` quét lưới tham số | `src-tauri/src/bin/anima-lab.rs` | bước ngắn nhất từ "chạy được thí nghiệm" tới "phân tích được thí nghiệm" |
| T-2 | ⬜ | Xuất CSV dạng tidy (một hàng cho một seed × tick × observable) | cùng file | mọi công cụ phân tích đều đọc được |
| T-3 | ⬜ | Trang HTML tự sinh từ `EnsembleSummary`, không phụ thuộc ngoài | `scripts/` | thư mục E2 có dữ liệu, chưa có gì vẽ |
| T-4 | ⬜ | Trình xem lại (replay) một snapshot trong khung cảnh 3D | `src/components/Landscape/` | công cụ quan sát rẻ nhất còn chưa xây |

---

## 8. Thứ tự giao việc gợi ý

```
M-1 → M-2 → M-3 → M-4 → M-5      (mốc 1: thế giới có cái chết)
H-1 … H-6                        (song song, bất cứ lúc nào)
F-1 → F-3 → F-2 → F-4 → F-5      (mốc 2: fitness cắm được)
Q-1 → Q-2 → Q-3 → Q-4            (sau khi F-2 chốt công thức)
O-1 → O-2 → O-4 → O-3 → O-5      (mốc 3: đo được tiến hoá)
R-1 … R-6                        (chỉ sau khi mốc 2 xanh)
H-7                              (cần người)
```
