---
title: ADR-0005 — Vòng đời quần thể (sinh sản và chết) trong adapter thí nghiệm live
status: proposed
owner: simulation-architecture
last_reviewed: 2026-11-16
decision_date: pending
supersedes: none
superseded_by: none
---

# ADR-0005 — Vòng đời quần thể trong adapter thí nghiệm live

> **Trạng thái: đề xuất, chờ chủ dự án.** Chưa có dòng `QĐ-4` / `QĐ-5` nào được điền. Số liệu ở
> mục "Bằng chứng" **chưa đăng ký trước, mang tính thăm dò** (đo 2026-11-16, build release macOS
> arm64, trunk + bản vá F-037), không phải kết quả thí nghiệm.

## Bối cảnh

Thế giới live qua `LiveExperimentAdapter` hiện **không thể cho ra kết quả tiến hóa**, vì không có
ca sinh nào trong adapter:

- [`build_live_world`](../../src-tauri/src/core/live_experiment.rs) cắm cứng
  `ReproductionPolicy::disabled()` (dòng 1562-1564, chú thích "until QD-4 / Card S1"). Mặc định của
  tài nguyên là `enabled: true`
  ([`resources.rs`](../../src-tauri/src/core/resources.rs) 558-572), nên app bật còn adapter tắt.
- [`drain_absent_thread_queues`](../../src-tauri/src/core/live_experiment.rs) (1349-1352) vứt hàng đợi
  `epoch_stats`. `check_epoch_completion_system`
  ([`agent_systems.rs`](../../src-tauri/src/core/agent_systems.rs) 881-967) gửi vào kênh đó, không có
  luồng tiến hóa nào đọc, nên `apply_staggered_evolution_system` (970) thấy hàng đợi rỗng:
  `EvolutionaryReplacement` không bao giờ chạy trong adapter (phát hiện E2-F3, xem
  [`RESULT.md`](../../artifacts/experiments/e2-evolved-brain-default/RESULT.md) §4; trên base này
  RESULT nằm ở đường dẫn đó, chưa có thư mục `superseded/`).
- Chết đói chỉ có khi `live.mortality=1` (`MortalityPolicy`, 1557-1561; phân tích khoá ở 255-275).

Hệ quả: E2 cho H1 null vì quần thể **đứng yên** (10 thân thể mất năng lượng, không ai chết, không ai
được thay). Một thí nghiệm không có vòng đời quần thể không đo được chọn lọc.

Cơ chế liên tục sẵn có là
[`autonomous_reproduction_system`](../../src-tauri/src/core/agent_systems.rs) (635-800), đã nằm trong
lịch tick ([`simulation_schedule.rs`](../../src-tauri/src/core/simulation_schedule.rs) 247):

- Cổng năng lượng: con mồi sinh khi `energy >= 0.65 * energy_target`, ăn thịt khi `>= 0.95`
  (710-717); cha đầu tư `min(invest_fraction * energy, surplus)` (719).
- Cha trả quỹ qua `debit_reserve`; con nhận `debited * REPRODUCTION_EFFICIENCY` (732), phần dư vào
  `detritus` (739) — EU kín.
- Kiểu gen con qua `mutate_genotype(..., 0.05, rng)` (770, xác suất cứng); não qua `mutate_brain`
  (785), sao chép genome chứ không sao chép trọng số đã học (không Lamarck); id dòng dõi
  `"cha.thế_hệ"` (791). Thứ tự duyệt sắp theo id dòng dõi (680).
- Con mồi chỉ sinh khi `prey_count < 80`; ăn thịt chỉ sinh khi `pred_count < 8` **và**
  `prey_count >= 5 * pred_count` (701-706). Đây là **cổng sinh, không phải trần**.

**Hai chỗ lệch giữa doc-comment và code, đã kiểm tại base `eb12e16`:** doc-comment của hàm
(629-634) nói `eta ~= 0.20` và ngưỡng `0.80`, nhưng hằng số là
`REPRODUCTION_EFFICIENCY = 0.60` ([`ecology.rs`](../../src-tauri/src/core/ecology.rs) 158, "60 % cho
con, 40 % vào detritus") và ngưỡng con mồi là `0.65`. Doc-comment cần sửa theo code (ngoài phạm vi
quyết định này); mọi mô tả "η≈0.20" ở nơi khác cần đọc lại là 0.60.

### Bằng chứng (thăm dò, chưa đăng ký)

`LiveExperimentAdapter::from_manifest`, `live.mortality=1`, `starve_ticks` mặc định, ghi đè
`ReproductionPolicy` thành bật sau khi dựng, 6000 tick, `build_tick_schedule` thật:

| Run | Số agent tại t=1000..6000 | Sinh / chết | Ghi chú |
|---|---|---|---|
| Đối chứng seed 1, sinh tắt | 10 phẳng, rồi chết hết, rỗng trước t=2000 | 0 / 10 | |
| Seed 1, 10 founder | 19/7/7/11/16/41 | 74 / 43 | founder hết trước t=2000; thế hệ tối đa 11; khối lượng 3.300 → 2.938; ăn thịt = 0 từ t=2000 |
| Seed 2, 10 founder | 25/45/73/93/103/113 | 232 / 129 | founder hết trước t=5000; vượt 80 (cổng, không phải trần); ăn thịt = 0 tại t=6000 |
| Seed 3, 10 founder | 19/10/8/4/4/6 | 32 / 36 | **tối thiểu 3 agent, suýt tuyệt chủng**; ăn thịt = 0 từ ~t=2400 |
| Seed 1, 60 founder | 102/14/26/68/90/135 | 239 / 164 | bùng, sập về 14, hồi phục; khối lượng 3.297 → 2.540 |

EU kín giữ hằng ở mọi mẫu mọi run (155 971.800 với 10 founder; 160 971.800 với 60). Cùng seed, bật
luân chuyển: seed 1 cho checksum cuối `0x5ea86d18` ở hai tiến trình riêng. Lỗi F-039
(`autonomous_reproduction_system` không ghi `VitalStatistics` births, nên `live.births` đọc 0) đã
tìm ra và sửa trong PR chờ duyệt; F-037 (hai hệ chết, bỏ qua thời gian ân hạn) đã sửa ở PR #50. Lần
chạy lại E2 hôm nay (PR #59) cho H1 null lần nữa với first-zero tick y hệt 24 run tháng 7.

## Các quyết định cần đưa ra

Nguyên văn từ [`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md) §4 và Card S1 của
[`2026-11-05-swarm-prompts.md`](../ai/planning/2026-11-05-swarm-prompts.md):

- **QĐ-4** — "population turnover mechanism".
- **QĐ-5** — "biological reproduction ADR approved? yes/no + ADR path". Thẻ Wave 6 phụ thuộc
  "an ADR for QD-5 accepted by the owner". CLAUDE.md: "Current epoch evolution is
  `EvolutionaryReplacement`, not biological reproduction."

## Động lực quyết định

- Đường mặc định phải còn nguyên: khoá vắng = tắt = mọi checksum hiện có bit-identical (CLAUDE.md,
  ADR-0003 quyết định 7).
- Phát triển chỉ xảy ra một lần lúc sinh; không Lamarck; restore/migration mang theo não đã có.
- Mọi khoản năng lượng mới đi qua `total_cost` trong `metabolic_decay_system`; EU kín.
- Tất định: cùng seed cùng checksum; save/load/fork ở tick sau ca sinh đầu tiên khớp chạy liền.
- Không đọc kênh của luồng tiến hóa (Card S1 bước 1). Không dựng "thế giới thứ ba".

## Các phương án

### Phương án A — Bật sinh sản liên tục theo năng lượng, qua khoá manifest opt-in

Thêm khoá `live.reproduction` (1.0 = bật; vắng/0.0 = tắt), phân tích giống `live.mortality`
(`live_experiment.rs` 255-275), và `build_live_world` chèn
`ReproductionPolicy { enabled: config.reproduction }` thay vì `disabled()`.

- *Luật cứng*: dùng đường sinh đã có (`SpawnOffspringCommand`, `agent_systems.rs` 452), nên phát
  triển diễn ra một lần lúc sinh; não con từ `mutate_brain` trên genome, không Lamarck;
  restore/migration không đụng tới. Mặc định = tắt. Không có khoản phí mới (chi phí sinh là chuyển
  quỹ cha → con + detritus, không phải khoản trừ riêng).
- *EU kín / tất định*: đã đo giữ hằng; checksum tái lập hai tiến trình. Test (d) của Card S1 mới
  chứng minh được fork/save-load trong adapter; hiện chưa có.
- *Mã*: `live_experiment.rs` (hằng khoá, `LiveWorldConfig`, phân tích, dòng 1564, khai khoá vào
  fingerprint manifest), `tests/live_turnover_tests.rs` mới; bản sửa F-039 đã có. Không bump
  `LIVE_MODEL_VERSION` vì quỹ đạo mặc định không đổi; nếu test (a) lệch thì bump và giải thích.
- *Rủi ro thấy trong số liệu*: bùng-sập (60 founder: 102 → 14), gần tuyệt chủng (seed 3), ăn thịt
  tuyệt chủng ở mọi run. Tham số (0.60, 0.65, 0.05, 80) là hằng cứng, không khai báo được; khối
  lượng thân giảm khi quần thể tăng có thể là dịch chuyển do mật độ chứ chưa phải chọn lọc.
- *Giới hạn*: sinh sản kiểu sinh thái học (không giao phối, không giới tính, không crossover):
  chọn lọc theo khả năng đạt ngưỡng năng lượng, không phải GA.

### Phương án B — Chạy `EvolutionaryReplacement` bên trong tick, tất định, không qua kênh

Chạy đồng bộ logic của `check_epoch_completion_system` + `apply_staggered_evolution_system`, hoặc
một hệ mới trong `simulation_schedule.rs`, thay cho đường kênh.

- *Luật cứng*: giữ invariant D06 (`agent_systems.rs` 390-400): thân mới rút năng lượng của cá thể bị
  thay từ detritus; **não bốc mới chứ không kế thừa** (428-433), nên chọn lọc không truyền não —
  muốn truyền phải làm việc M5 chưa có. Phải chốt cách xếp hạng (`FitnessRuleSetting`) độc lập với
  luồng tiến hóa.
- *Tất định / save-load / fork*: `EpochManager` và hàng đợi phải nằm trong checkpoint; lịch tick đổi
  nên chạm `schedule_ambiguity_tests` và mọi checksum, phải bump `LIVE_MODEL_VERSION`.
- *Mã*: `agent_systems.rs` (tách phần thuần khỏi `EvolutionSender`/`EvolutionReceiver`),
  `live_experiment.rs` 1349-1352, `simulation_schedule.rs`, `evolution/*`.
- *Rủi ro*: dân số cố định (không thấy bùng-sập sinh thái), thay thế đồng loạt theo epoch tạo nhịp
  nhân tạo; phạm vi và rủi ro lớn nhất.

### Phương án C — Cả hai, chọn theo manifest

`live.reproduction` và `live.replacement` độc lập. Linh hoạt nhất, nhưng gồm toàn bộ chi phí của B,
và hai cơ chế chồng nhau trong một run (một cá thể vừa bị thay vừa sinh) chưa được phân tích về EU
và về kết luận chọn lọc. Hoãn tới khi A có kết quả và B có nhu cầu cụ thể.

### Giữ nguyên hiện trạng

Adapter không có sinh; mọi thí nghiệm live đo dân số đóng băng (như E2). Không khuyến nghị.

## Quyết định đề xuất

**QĐ-4 = A; QĐ-5 = có, ADR này là ADR đó, phạm vi giới hạn ở sinh sản liên tục theo năng lượng
trong thí nghiệm live.** Lý do: A là phương án duy nhất tái dùng cơ chế đã chạy trong app (không
"thế giới thứ ba"), đã đo hằng EU và tái lập tất định, chạm ít file nhất (một khoá, một dòng chèn,
test) và giữ đường mặc định bit-identical. B đụng lịch tick và checkpoint mà chưa có nhu cầu khoa
học chứng minh.

**ADR này KHÔNG quyết:**

- Tinh chỉnh cổng ăn thịt (`pred_count >= 8`, `5×`) hay các hằng (0.60, 0.65, 0.05, 80): quyết định
  riêng (thẻ tham số hoá), vì số liệu cho thấy ăn thịt tuyệt chủng ở mọi run.
- Bất kỳ khẳng định thích nghi hay hình thành loài. Dịch chuyển khối lượng ở trên chưa phân biệt
  được chọn lọc, trôi dạt và hiệu ứng mật độ; theo cổng bằng chứng AE-S11/AE-S14.
- S43 (Red-Queen ăn thịt-con mồi): cần ăn thịt tồn tại, mà bằng chứng hôm nay nói không.
- Phương án B/C, sinh sản hữu tính, và việc sửa doc-comment lệch (0.20 / 0.80).

## Hệ quả

- **Tích cực**: thí nghiệm live đo được chọn lọc; E2 chạy lại được với luân chuyển; checksum cũ
  nguyên vẹn.
- **Tiêu cực**: quỹ đạo bật sinh sản bùng-sập và dễ tuyệt chủng (seed 3), nên tiền đăng ký phải nêu
  quy tắc xử lý run tuyệt chủng; vẫn chưa có ăn thịt bền.
- **Thẻ tiếp theo**
  1. **Card S1 thu hẹp**: khoá `live.reproduction` + test (a)-(e) của Card S1 (tắt ⇒ checksum bằng
     BASE; bật ⇒ `agent_count` đổi trong một run; checksum khớp qua 6 tiến trình; fork và
     save → load sau ca chết và ca sinh đầu tiên khớp chạy liền; phần dư EU kín). Bước 0 của thẻ đã
     được số liệu thăm dò trả lời: không có khoá này, dân số không luân chuyển.
  2. **Card S2**: observables (tích phân năng lượng, khối lượng theo vùng, giá trị cho quần thể rỗng).
  3. **Thí nghiệm chọn lọc đầu tiên (đăng ký trước, làm sau)**: thiếu thức ăn so với dồi dào, quan
     sát khối lượng thân trung bình của quần thể (`live.trait_mass_mean`), chỉ con mồi, 12 seed, có
     nhánh đối chứng trôi dạt không đột biến. Preregistration riêng; chưa viết kết luận trước.

## Dòng quyết định để dán

Thay `<ngày>`. Phương án A (khuyến nghị):

```text
QD-4 (owner, <ngày>) = A: continuous energy-gated reproduction enabled in the live adapter behind
            manifest key live.reproduction (absent/0.0 = off); mechanism and constants are
            autonomous_reproduction_system as on base; ADR-0005
QD-5 (owner, <ngày>) = yes: docs/decisions/ADR-0005-live-population-turnover.md (scope: continuous
            reproduction in live experiments only; EvolutionaryReplacement unchanged)
```

Phương án B:

```text
QD-4 (owner, <ngày>) = B: EvolutionaryReplacement run inside the tick, deterministically, without
            the evolution thread's channel; ADR-0005
QD-5 (owner, <ngày>) = no: biological reproduction is not approved for live experiments
```

Phương án C:

```text
QD-4 (owner, <ngày>) = C: both, selectable per manifest (live.reproduction, live.replacement); ADR-0005
QD-5 (owner, <ngày>) = yes: docs/decisions/ADR-0005-live-population-turnover.md
```

Sau khi chủ dự án điền: đổi `status` thành `accepted`, điền `decision_date`, cập nhật chỉ mục.
