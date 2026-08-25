---
phase: requirements
feature: ecological-selection
title: Requirements — Đóng vòng sinh thái
description: Nối tầng sinh thái đã mô phỏng vào cơ chế chọn lọc, để môi trường quyết định được ai truyền gene
status: proposed
owner: simulation-architecture
last_reviewed: 2026-08-25
design: ../design/2026-08-25-feature-ecological-selection.md
planning: ../planning/2026-08-25-feature-ecological-selection.md
implementation: ../implementation/2026-08-25-feature-ecological-selection.md
testing: ../testing/2026-08-25-feature-ecological-selection.md
---

# Requirements — Đóng vòng sinh thái

## Bộ tài liệu

| Tài liệu | Nội dung |
|---|---|
| **Requirements** (tài liệu này) | vấn đề, ràng buộc, tiêu chí nghiệm thu |
| [Design](../design/2026-08-25-feature-ecological-selection.md) | kiến trúc năm nhánh, điểm chèn, hợp đồng dữ liệu |
| [Plan](../planning/2026-08-25-feature-ecological-selection.md) | thứ tự triển khai, đồ thị phụ thuộc, rollback |
| [Task backlog](../implementation/2026-08-25-feature-ecological-selection.md) | việc có neo file và định nghĩa hoàn thành |
| [Testing](../testing/2026-08-25-feature-ecological-selection.md) | gate MO · FI · QD · RE · OB · PF |
| [Bản giao việc](../planning/2026-08-25-agent-goal-ecological-selection.md) | hợp đồng cho agent triển khai |

---

## 1. Shared understanding

Anima-Engine mô phỏng sinh thái ở mức chi tiết hiếm thấy trong phần mềm loại này: năng suất sơ cấp
theo biome, định luật Kleiber cho trao đổi chất, hệ số Boltzmann cho nhiệt độ, phản ứng Holling II
cho gặm cỏ và III cho săn mồi, hiệu suất Lindeman giữa các bậc dinh dưỡng, mùa vụ, và một sổ cái
năng lượng đóng đã được đo là bit-exact.

**Không một mảnh nào trong đó chạm tới việc ai được truyền gene.**

Đây không phải thiếu tính năng. Đây là chuyện vòng nhân quả trung tâm của một simulator tiến hoá
chưa được nối kín: môi trường → cơ thể → sống sót → sinh sản → gene → cơ thể. Vòng đó đứt đúng ở
mũi tên thứ ba.

Feature này nối ba mũi tên còn thiếu, và chỉ ba mũi tên đó. Nó **không** thiết kế lại sinh thái,
**không** đụng hợp đồng năng lượng, và **không** thay đổi hành vi mặc định cho tới khi một cờ được
bật một cách tường minh.

---

## 2. Problem statement

### 2.1 Vấn đề nghiên cứu

Một run mặc định hôm nay không thể trả lời câu hỏi rẻ nhất mà một ALife simulator phải trả lời
được: *nếu tôi làm sa mạc khô hơn, quần thể sẽ tiến hoá theo hướng nào?* Câu trả lời hiện tại là
**không hướng nào** — vì không có đường nào từ độ khô tới hàm fitness.

Hệ quả kéo theo đã hiện ra một lần và đã tốn một thí nghiệm hoàn chỉnh: thí nghiệm E2 cho
`paired_mean_delta` **đúng bằng 0 trên cả 12 seed**. Bản `RESULT.md` gọi đó là hiệu ứng sàn và đúng.
Nhưng nguyên nhân gốc sâu hơn một bậc: kể cả khi chưa chạm sàn, **không tồn tại kênh nào để một bộ
não tốt hơn biến thành nhiều con cháu hơn**. Bất kỳ thí nghiệm nào về hành vi, chạy trên engine
hôm nay, đều sẽ đo phương sai của bộ tạo số ngẫu nhiên.

### 2.2 Bằng chứng kỹ thuật (đọc mã 2026-08-25, chưa chạy lệnh)

| # | Sự thật | Bằng chứng |
|---|---|---|
| T1 | Cả crate có **đúng một** định nghĩa fitness: `cumulative_distance + tick_count` | [`agent_systems.rs:528`](../../../src-tauri/src/core/agent_systems.rs) |
| T2 | `tick_count` tăng **vô điều kiện** mỗi tick cho mọi agent có `FeatureTracker` | [`world_systems.rs:171`](../../../src-tauri/src/core/world_systems.rs) |
| T3 | Sau một epoch, T2 làm `tick_count` gần như bằng nhau ⇒ xếp hạng fitness ≈ xếp hạng quãng đường | suy ra từ T1 + T2 |
| T4 | `energy <= 0` không kết thúc gì — hệ đánh giá chỉ `continue` | [`agent_systems.rs:452`](../../../src-tauri/src/core/agent_systems.rs) |
| T5 | Ở tầng quần thể tổng hợp, "không chết đói" là **invariant có test khoá** | [`aggregate_population.rs:1653`](../../../src-tauri/src/core/aggregate_population.rs) |
| T6 | `AgentEvaluation.survival_ticks` **chỉ tăng khi còn năng lượng và nước** — đúng đại lượng cần, đã tính sẵn mỗi tick | [`agent_systems.rs:452-462`](../../../src-tauri/src/core/agent_systems.rs) |
| T7 | `check_epoch_completion_system` **vứt bỏ** `AgentEvaluation`: nó bind `_eval, _homeo` rồi đọc `FeatureTracker` | [`agent_systems.rs:523`](../../../src-tauri/src/core/agent_systems.rs) |
| T8 | Trục MAP-Elites thứ hai `foraging_range = cumulative_distance` — tức là **chính fitness trừ một hằng số** | [`agent_systems.rs:531`](../../../src-tauri/src/core/agent_systems.rs) vs `:528` |
| T9 | Quần thể mặc định **10 cá thể**; mỗi 1000 tick **toàn bộ** bị thay | [`resources.rs:1215`](../../../src-tauri/src/core/resources.rs), [`simulation_loop.rs:1323`](../../../src-tauri/src/core/simulation_loop.rs) |
| T10 | Bảy hệ thống lặp **toàn bộ segment** bên trong vòng lặp agent — O(agent × segment) | `world_systems.rs:121,236,337,436,454` · `agent_systems.rs:468` · `environmental_systems.rs:282` |
| T11 | `sensory_system` quét **tuyến tính** danh sách thức ăn và mồi cho từng agent, dù `SpatialHashGrid` đã dựng sẵn và đang được dùng cho raycast ngay bên dưới | [`agent_systems.rs:877,890`](../../../src-tauri/src/core/agent_systems.rs) |
| T12 | Mẫu sửa đúng **đã có trong repo**: `combat_system` dựng `centroid_index: Local<EntityHashMap>` một lần rồi quét segment O(S) | [`world_systems.rs:300-340`](../../../src-tauri/src/core/world_systems.rs) |
| T13 | 11 observable của `LiveExperimentAdapter` đều là tổng hoặc trung bình; không cái nào phát hiện được tiến hoá | [`live_experiment.rs:721-794`](../../../src-tauri/src/core/live_experiment.rs) |
| T14 | Shannon và Simpson **đã được tính** nhưng chỉ chảy vào panel UI, không vào tầng observable | [`simulation_loop.rs:2192`](../../../src-tauri/src/core/simulation_loop.rs) |
| T15 | `ReclaimAndDespawnAgentCommand` — cơ chế trả EU về detritus khi một cá thể biến mất — **đã viết xong và đã có gate** | [`agent_systems.rs:112-152`](../../../src-tauri/src/core/agent_systems.rs) |

T6, T7, T12 và T15 là bốn sự thật quan trọng nhất trong bảng: **phần khó của cả bốn nhánh đầu đã
được viết rồi**. Cái thiếu là dây nối, không phải cơ chế.

### 2.3 Điều KHÔNG phải vấn đề

Đừng đi sửa những thứ sau, chúng đúng như hiện tại:

- **Sổ cái năng lượng.** Nó đóng, có gate, và không có lỗi nào ở đây liên quan tới nó.
- **Sinh thái.** `biome_npp`, `metabolic_rate`, `herbivore_intake`, `predation_capture` đều được
  gọi thật và đều đúng. Holling II/III được dùng gián tiếp qua hai hàm sau; đừng báo cáo chúng là
  mã chết.
- **Tính tất định.** `SimRng`, thứ tự lịch trình đã khai báo, checkpoint — tất cả đều lành.
- **`AgentEvaluation`.** Nó tính đúng thứ cần tính. Vấn đề là không ai đọc nó.

---

## 3. Yêu cầu chức năng

### R1 — Tử vong (mortality)

- **R1.1** Một cá thể cạn năng lượng hoặc cạn nước liên tục quá một ngưỡng khai báo được thì chết.
- **R1.2** Cái chết đi qua đúng một đường: `ReclaimAndDespawnAgentCommand`, nên EU của xác trả về
  ngăn `detritus` **nguyên tử với** việc despawn.
- **R1.3** Ngưỡng chết là tham số của thế giới, không phải hằng số nằm rải trong mã.
- **R1.4** Tử vong **tắt mặc định**. Cờ `ANIMA_MORTALITY` bật nó.
- **R1.5** Khi tắt, mọi quỹ đạo trùng **bit** với hôm nay.

### R2 — Fitness sinh thái

- **R2.1** Hàm fitness là **một điểm cắm khai báo được**, không phải một biểu thức nằm giữa vòng lặp.
- **R2.2** Có ít nhất hai chế độ: `locomotion` (nguyên trạng hôm nay, mặc định) và `ecological`.
- **R2.3** Chế độ `ecological` chấm điểm từ `AgentEvaluation` — đại lượng đã tính sẵn và **có** điều
  kiện sống — chứ không từ `FeatureTracker`.
- **R2.4** Chế độ được ghi vào manifest thí nghiệm và vào provenance của run, để một kết quả không
  bao giờ mơ hồ về việc nó được chấm bằng luật nào.
- **R2.5** Chế độ `locomotion` giữ nguyên dòng hiện tại, nên mọi artifact E2 đã ghi vẫn replay được.

### R3 — Trục mô tả độc lập với fitness

- **R3.1** Không trục MAP-Elites nào được là một hàm đơn điệu của fitness.
- **R3.2** Trục mới mô tả **cách sống**, không mô tả **sống tốt đến đâu**.
- **R3.3** Đổi trục là đổi hợp đồng của kho lưu trữ: kho cũ không tương thích và phải bị từ chối
  tường minh khi nạp, không được nạp im lặng rồi diễn giải sai.

### R4 — Sinh sản theo ngưỡng năng lượng

- **R4.1** Một cá thể tích đủ dự trữ thì sinh, và **trả bằng năng lượng thật** lấy từ dự trữ của
  chính nó.
- **R4.2** Quy mô quần thể trở thành **kết quả**, không phải tham số.
- **R4.3** Có trần quần thể cứng, và chạm trần là một sự kiện quan sát được, không phải một lỗi im lặng.
- **R4.4** Thay thế theo epoch vẫn chạy được khi cờ tắt — hai cơ chế không được chạy chồng nhau.
- **R4.5** Không Lamarck: con nhận genome, không nhận thứ cha mẹ học được
  ([ADR-0003](../../decisions/ADR-0003-evolved-per-agent-brains.md)).

### R5 — Quan sát được tiến hoá

- **R5.1** Có observable về **phân bố**, không chỉ về trung bình: phương sai và phân vị của ít nhất
  một tính trạng di truyền.
- **R5.2** Có observable về **luân chuyển**: số ca sinh, số ca tử, tuổi thọ trung bình.
- **R5.3** Có observable về **đa dạng**: Shannon và Simpson nối từ chỗ đã tính vào tầng observable.
- **R5.4** Có observable về **độ phủ** kho MAP-Elites.
- **R5.5** Mọi observable mới chỉ phát ra khi cấu trúc sinh ra nó tồn tại. Một manifest xin một
  observable không tồn tại phải **fail ở preflight**, không được trả về số 0.

### R6 — Quy mô đủ để sinh thái có nghĩa

- **R6.1** Bảy hệ thống ở T10 chuyển từ O(agent × segment) sang O(segment), bằng đúng mẫu ở T12.
- **R6.2** `sensory_system` hỏi `SpatialHashGrid` thay vì quét tuyến tính.
- **R6.3** Đây là refactor **thuần**: cùng seed phải cho cùng checksum trước và sau.
- **R6.4** Có một số đo `full_tick` ở 1000 agent, bản release, đo bằng đường `ANIMA_TICK_CAPTURE`
  đã ship.

---

## 4. Yêu cầu phi chức năng

| # | Yêu cầu | Ngưỡng | Vì sao |
|---|---|---|---|
| N1 | Residual năng lượng | ≤ `1e-3` EU trong **mọi** cấu hình cờ | [ENERGY_LEDGER_CONTRACT §5](../../reference/ENERGY_LEDGER_CONTRACT.md) — không được nâng ngưỡng để test xanh |
| N2 | Cấp phát trong tick path | `allocs == 0` | quy tắc đã có, gate đã có |
| N3 | Tương thích ngược | mọi cờ tắt ⇒ checksum **trùng bit** với hôm nay | nếu không, mọi artifact đã ghi thành vô giá trị |
| N4 | Tất định | cùng seed ⇒ cùng checksum, **giữa các tiến trình** | [DETERMINISM_CONTRACT](../../reference/DETERMINISM_CONTRACT.md); E2 đã một lần trượt đúng chỗ này |
| N5 | Không Lamarck | không ghi trạng thái runtime ngược vào genotype | [ADR-0003](../../decisions/ADR-0003-evolved-per-agent-brains.md) |
| N6 | Luật thế giới bất biến trong một run | đổi luật ⇒ manifest mới | [ADR-0002](../../decisions/ADR-0002-world-laws-and-exotic-energy.md) ER01 |
| N7 | Snapshot | mọi trường mới phải round-trip | [SNAPSHOT_CONTRACT](../../reference/SNAPSHOT_CONTRACT.md) |

---

## 5. Tiêu chí nghiệm thu

Feature này **xong** khi cả năm câu dưới đây đều đúng và có lệnh chứng minh:

1. Bật `ANIMA_MORTALITY`, đặt `biome_npp` về 0 ⇒ quần thể **sập về 0** và residual EU vẫn ≤ 1e-3.
2. Bật `ANIMA_FITNESS=ecological`, chạy hai thế giới khác nhau ở đúng **một** tham số sinh thái
   ⇒ phân bố tính trạng sau N epoch **khác nhau có ý nghĩa**; cùng test đó chạy trên
   `ANIMA_FITNESS=locomotion` thì **không** khác. Gate phải chứng minh kênh nhân quả tồn tại, chứ
   không chỉ chứng minh code chạy.
3. Tắt hết cờ mới ⇒ `cargo test --features desktop` xanh và checksum một run có seed **trùng bit**
   với bản trước feature.
4. Một run có tiến hoá và một run bị đóng băng cho ra **hai chuỗi observable phân biệt được**; ghi
   lại rằng chuỗi hôm nay **không** phân biệt được, làm đối chứng âm.
5. `full_tick` p50 ở **1000 agent**, bản release, có số — và `allocs == 0` vẫn giữ.

---

## 6. Ngoài phạm vi

- Hình thái nhìn thấy được, animation, tính toán phân tán — đó là
  [living-world](2026-08-25-feature-living-world.md), một feature riêng, chạy song song được.
- Sinh mô hình 3D bằng AI. Không liên quan.
- Phân loại loài tự động, MRCA, nén phả hệ — đã có mục riêng trong
  [STATE_OF_THE_PROJECT §3.15](../../planning/STATE_OF_THE_PROJECT.md).
- Đổi sinh thái. Sinh thái đúng rồi; feature này chỉ nối nó vào chỗ nó chưa tới.
