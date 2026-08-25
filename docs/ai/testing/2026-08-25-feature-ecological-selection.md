---
phase: testing
feature: ecological-selection
title: Testing — Đóng vòng sinh thái
description: Gate MO · FI · QD · RE · OB · PF, và ba gate chứng minh kênh nhân quả thật sự tồn tại
status: proposed
owner: simulation-architecture
last_reviewed: 2026-08-25
requirements: ../requirements/2026-08-25-feature-ecological-selection.md
design: ../design/2026-08-25-feature-ecological-selection.md
planning: ../planning/2026-08-25-feature-ecological-selection.md
contract: ../../reference/ENERGY_LEDGER_CONTRACT.md
---

# Testing — Đóng vòng sinh thái

Quy tắc chung của repo áp nguyên: một gate xanh phải là **kết quả một lệnh chạy được**, có ngày,
có cấu hình. Số không kèm lệnh là lịch sử, không phải trạng thái —
[STATE_OF_THE_PROJECT §1.1](../../planning/STATE_OF_THE_PROJECT.md).

---

## 0. Quy tắc riêng của feature này

Feature này thêm một loại gate mà repo chưa có, và đó là phần quan trọng nhất của tài liệu này.

Bộ gate hiện tại rất mạnh ở việc bắt lỗi **bên trong** một hệ thống: năng lượng có rò không, RNG
có tái lập không, lịch trình có ổn định không, binding hai phía có khớp không. Không gate nào bắt
lỗi **giữa** các hệ thống — kiểu "sinh thái này có ảnh hưởng tới tiến hoá kia không". Cả năm phát
hiện dẫn tới feature này đều nằm đúng ở khoảng trống đó, và cả năm đều exit 0 hôm nay.

Vì vậy ba gate dưới đây là **gate kênh nhân quả**, và mỗi cái có một **đối chứng âm** bắt buộc:
gate phải **đỏ** trên bản dựng cũ. Một gate mới chỉ xanh trên bản mới thì không chứng minh được
điều gì — nó có thể đang khẳng định một tautology.

| Gate | Khẳng định | Đối chứng âm bắt buộc |
|---|---|---|
| **CA-S01** | Đổi một tham số sinh thái ⇒ phân bố tính trạng đổi | cùng test trên `ANIMA_FITNESS=locomotion` phải **không** đổi |
| **CA-S02** | Quần thể sập khi NPP = 0 | với `ANIMA_MORTALITY` tắt, quần thể **không** sập |
| **CA-S03** | Chuỗi observable phân biệt được run có tiến hoá với run đóng băng | 11 observable **cũ** phải **không** phân biệt được |

---

## 1. Gate tử vong (MO)

| Gate | Bằng chứng | Bắt được gì |
|---|---|---|
| **MO-S01** | Bật tử vong, chạy 120.000 tick ⇒ residual EU ≤ `1e-3` | rò EU khi thêm đường chết |
| **MO-S02** | Một cá thể chết ⇒ `detritus` tăng **đúng bằng** dự trữ nó mang, đo trước và sau | reclaim sai số lượng |
| **MO-S03** | Cá thể ăn được một lần ⇒ `starving_ticks` về 0 | quên reset ⇒ mọi con chết đúng hạn bất kể ăn bao nhiêu |
| **MO-S04** | `mortality_system` chạy **sau** `metabolic_decay`, **trước** `ecosystem_census` | census đếm một thế giới nửa vời |
| **MO-S05** | `ANIMA_MORTALITY` unset ⇒ checksum một run có seed **trùng bit** với bản trước feature | vi phạm N3 |
| **MO-S06** | Save trước feature nạp được; `starving_ticks` đọc thành 0 | thiếu `serde(default)` |
| **CA-S02** | NPP = 0 ⇒ quần thể về 0; tắt tử vong ⇒ **không** về 0 | gate kênh nhân quả, xem §0 |

MO-S05 là gate quan trọng nhất của nhánh M: nó là điều duy nhất giữ cho mọi artifact thí nghiệm
đã ghi còn giá trị.

---

## 2. Gate fitness (FI)

| Gate | Bằng chứng | Bắt được gì |
|---|---|---|
| **FI-S01** | `LocomotionFitness` cho **cùng bit** kết quả với dòng `agent_systems.rs:528` hiện tại, trên một tập giá trị gồm cả giá trị cận biên | viết lại "cho gọn" đổi làm tròn `f32` |
| **FI-S02** | `EcologicalFitness` xếp hạng hai cá thể cùng quãng đường nhưng khác `survival_ticks` **khác nhau** | luật mới vẫn ngầm chấm bằng quãng đường |
| **FI-S03** | `EcologicalFitness` **không** chứa số hạng quãng đường: đổi `cumulative_distance` không đổi điểm | P-04 quay lại qua cửa sau |
| **FI-S04** | Mọi report — manifest, tick capture, provenance ensemble — mang `id()` của luật đang chạy | một kết quả không nói được nó chấm bằng luật nào |
| **FI-S05** | Manifest xin luật không tồn tại ⇒ **fail ở preflight** | rơi về mặc định im lặng |
| **CA-S01** | Hai thế giới khác đúng một tham số sinh thái ⇒ phân bố tính trạng khác; trên `locomotion` thì **không** khác | gate kênh nhân quả, xem §0 |

> **CA-S01 là gate lý do tồn tại của cả feature.** Nếu nó xanh trên **cả hai** chế độ, nghĩa là
> phép đo đang bắt một thứ khác (thường là drift RNG), không phải kênh nhân quả mới. Nếu nó đỏ
> trên `ecological`, kênh vẫn chưa nối. Chỉ đúng một hình dạng kết quả là đúng.

---

## 3. Gate trục mô tả (QD)

| Gate | Bằng chứng | Bắt được gì |
|---|---|---|
| **QD-S01** | `trophic_ratio` của con ăn cỏ thuần = 0, ăn thịt thuần = 1, hỗn hợp ở giữa | bộ đếm nguồn cộng nhầm chỗ |
| **QD-S02** | Trên một quần thể fixture, tương quan hạng giữa trục 2 và fitness **không** đơn điệu | trục mới vẫn là fitness trá hình |
| **QD-S03** | Nạp archive có `descriptor_set_id` cũ vào bản dựng mới ⇒ **lỗi có thông báo** | nạp im lặng đặt cá thể vào ô sai |
| **QD-S04** | Chưa ăn gì ⇒ `trophic_ratio` có giá trị khai báo được, **không** NaN | NaN vào `get_bin_coords` rơi vào ô gốc, im lặng (F-006) |
| **QD-S05** | Trục 2 không bão hoà: phân bố trên fixture không dồn về một đầu | `clamp` biến trục thành hằng số |

---

## 4. Gate sinh sản (RE)

| Gate | Bằng chứng | Bắt được gì |
|---|---|---|
| **RE-S01** | Bật sinh sản, 120.000 tick ⇒ residual EU ≤ `1e-3` | hai vế giao dịch không cùng sync point |
| **RE-S02** | Quần thể **dao động**, không phân kỳ, trên ít nhất 3 seed | thiếu phản hồi âm |
| **RE-S03** | Chạm trần ⇒ `births_suppressed_by_cap` tăng, và có observable | trần im lặng |
| **RE-S04** | Sau N ca sinh, `to_newick` và `simplify` chạy được ⇒ 0 cạnh mồ côi | cạnh ghi ở hệ thống sau, cha đã despawn |
| **RE-S05** | Bật sinh sản **và** thay thế theo epoch ⇒ lỗi cấu hình ở preflight | hai cơ chế chạy chồng |
| **RE-S06** | `.genotype` của con **không** chứa gì cha mẹ học được | Lamarck, [ADR-0003](../../decisions/ADR-0003-evolved-per-agent-brains.md) |

---

## 5. Gate quan sát (OB)

| Gate | Bằng chứng | Bắt được gì |
|---|---|---|
| **OB-S01** | `live.shannon` khớp giá trị panel UI trên cùng trạng thái | nối vào sai nguồn |
| **OB-S02** | Hai quần thể **cùng trung bình, khác phương sai** ⇒ observable phân biệt được | đây là test chứng minh vì sao trung bình không đủ |
| **OB-S03** | `births`/`deaths` là đếm **theo cửa sổ**, reset mỗi cửa sổ | chép nhầm quy tắc tích luỹ của `evolution.births` |
| **OB-S04** | `archive_coverage` = số ô lấp / tổng ô, khớp `archive.grid.len()` | |
| **OB-S05** | Manifest xin `live.deaths` trên run tắt tử vong ⇒ **fail preflight** | trả 0 trông như một phép đo |
| **CA-S03** | Run có tiến hoá vs run đóng băng: chuỗi mới phân biệt được, chuỗi cũ **không** | gate kênh nhân quả, xem §0 |

---

## 6. Gate hiệu năng (PF)

| Gate | Bằng chứng | Bắt được gì |
|---|---|---|
| **PF-S01** | Cùng seed ⇒ **cùng checksum**, trước và sau mọi thay đổi thuộc H | thứ tự duyệt `EntityHashMap` lọt vào kết quả |
| **PF-S02** | `allocs == 0` giữ nguyên trong tick path | `new()` thay vì `clear()` trên bộ đệm `Local` |
| **PF-S03** | `full_tick` p50 ở **1000 agent**, bản **release**, qua `ANIMA_TICK_CAPTURE` | ⚠️ **cần người bấm chạy** |
| **PF-S04** | Ở 1000 agent, thời gian mỗi tick tăng **tuyến tính** theo số agent, không bậc hai | mẫu chỉ mục áp thiếu một chỗ |

PF-S04 là gate đáng giá nhất của nhánh H: nó bắt được **chỗ bị bỏ sót**, còn PF-S01 chỉ bắt được
chỗ làm sai.

---

## 7. Gate hồi quy — chạy nguyên bộ hiện có

Không gate nào ở trên thay thế bộ gate đang có. Mọi thay đổi phải giữ:

```
cd src-tauri && cargo test --features desktop --no-fail-fast
cd src-tauri && cargo fmt --check
cd src-tauri && cargo clippy --all-targets --features desktop -- -D warnings
cd src-tauri && cargo clippy --all-targets --no-default-features -- -D warnings
node scripts/check_test_targets.mjs <captured-output> --profile desktop
npm run lint && node scripts/eslint_ratchet.mjs
npm run test && npm run test:frontend && npm run build
cd src-tauri && cargo test --features desktop && git diff --exit-code -- ../src/types/generated
node scripts/check_docs_links.mjs
```

> ⚠️ Mọi test mới phải có mặt trong [`scripts/test_target_policy.mjs`](../../../scripts/test_target_policy.mjs)
> nếu nó tạo ra một target rỗng hoặc một test `#[ignore]`. Gate cưỡng chế **cả hai chiều**: mục
> ngoài danh sách fail, và mục trong danh sách đã hết hiệu lực cũng fail.

---

## 8. Thứ tự chạy gate khi nhận việc

1. **Trước khi viết dòng code nào:** chạy bộ §7, ghi lại kết quả làm baseline. Không có baseline
   thì không phân biệt được "mình làm hỏng" với "nó vốn đã đỏ".
2. **Sau mỗi task:** gate của nhánh đó + §7.
3. **Trước khi báo cáo milestone:** cả §7 lẫn ba gate kênh nhân quả CA-S01…CA-S03, **kèm đối chứng
   âm**. Báo cáo một gate kênh nhân quả mà không kèm đối chứng âm là báo cáo chưa đủ.
