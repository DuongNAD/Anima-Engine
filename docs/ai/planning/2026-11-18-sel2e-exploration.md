---
phase: planning
feature: experiment-sel2-food-scarcity
title: Thẻ SEL2-E — đo thăm dò lần hai, kết quả âm
description: SEL2-E (216 lượt, 18.000 tick) không có ô khả thi - chỉ 5–7 thế hệ, cỏ không giới hạn quần thể ở hệ số 1,0, khát vẫn gây phần lớn cái chết dù có 8 hồ. Bốn phép đo tiếp theo cùng ngày - nới ân hạn khát chuyển cái chết sang đói; lượt 100.000 tick ở hệ số 0,7 và 0,5 cho tới 16–23 thế hệ nhưng phần lớn quần thể tuyệt chủng, kể cả nền không khan hiếm có ân hạn khát dài (3/12); chẩn đoán cho thấy chúng trượt dần trong đáy của chu kỳ bùng nổ và sụp đổ, nơi sinh sản dừng hẳn; phép cuối cho thấy cỏ đầy đồng (83–86 %) nhưng chỉ còn 6–7 % dưới chân con vật (đính chính sau S11: con số này phản ánh tốc độ gặm trụi ô, và giới hạn thật là khát). Mù với khối lượng
status: completed
owner: simulation-architecture
last_reviewed: 2026-11-18
---

# Thẻ SEL2-E — đo thăm dò lần hai và các phép đo tiếp theo: kết quả âm

> **Thăm dò, chưa đăng ký, mù với khối lượng cơ thể.** Driver (`src-tauri/examples/sel2e_exploration.rs`) đọc
> observable qua một danh sách cho phép gồm `live.agent_count`, `live.standing_crop`, `live.births`,
> `live.deaths`, `live.mean_lifespan`, `live.deaths_starvation` và `live.deaths_dehydration`. Mọi observable khác
> bị bỏ mà không đọc; không giá trị `live.trait_mass_*`, genotype hay kích thước cơ thể nào được in, lưu hay
> dùng. Seed 1–6 giống S6 và S9 và **không** phải seed xác nhận.

> **Đính chính 2026-11-18, sau [thẻ S11](2026-11-18-s11-grass-steering.md).** Số liệu của mục "Chẩn đoán đáy
> chu kỳ" giữ nguyên, nhưng hai cách đọc của nó sai:
>
> 1. Cột "Mẫu có con đủ ngưỡng sinh" (0/80, 0/54, 0/56) là hệ quả của cách lấy mẫu, không phải bằng chứng. Probe
>    đọc sau `run_schedule_once`, tức ở cuối tick, mà `autonomous_reproduction_system` chi 50 % năng lượng ngay trong
>    tick con vật chạm 0,65 mức mục tiêu (khi số con mồi còn dưới `prey_cap`). Mẫu cuối tick vì vậy gần như luôn đếm
>    được 0 con đủ ngưỡng, ở mọi thế giới. Muốn biết sinh sản có dừng không thì đếm `live.births`.
> 2. Cỏ dưới chân 6–7 % không chứng minh lái sai. Một con gặm ô `Grassland` ở hệ số 0,7 xuống dưới 25 % `r_max`
>    trong ≤ 9 tick và trụi hẳn trong ≤ 37 tick, còn băng qua một ô ở `drive_floor` 0,5 mất khoảng 68 tick. Ô dưới
>    chân một con đang ăn luôn gần cạn, dù nó lái thế nào (S11 §4, trở ngại 2).
>
> Đề xuất 1 ở cuối ghi chú (cho con vật tìm cỏ) đã được làm và đo ở S11, và kết quả âm. Ở phép thử cơ chế không
> khát, năng lượng trung bình giữ 0,89–0,98 mức mục tiêu suốt 3.000 tick, còn nhánh lái tới cỏ tụt dưới đối chứng từ
> tick 2.000. Thứ giới hạn là khát: mọi hồ nằm trên một hàng `z = +60`, đàn sáng lập ở `z = −30`. Thẻ kế tiếp là S12
> (`live.lake_layout`, rải hồ trên lưới hai chiều).

## Câu hỏi

[Đề xuất làm lại SEL2](2026-11-18-sel2-redesign-proposal.md) §5, thẻ SEL2-E: có cấu hình nào giữ quần thể sống ở
mọi seed, đủ thế hệ, trong khi thức ăn thật sự giới hạn?

## Cách đo

- **Thế giới nền** (S6/E-SEL1): không thú săn, 8 cây, bật tử vong, sinh sản, vận động với `drive_floor` 0,5 và
  lái tới nước, não chung. `live.prey_cap` 320, `live.resource_capacity_scale` 1,0, đúng lưới đề xuất.
- **Lưới:** `live.founders` ∈ {30, 60, 100} × `live.plant_reserve` ∈ {0; 0,2; 0,3; 0,4} × `live.lakes` ∈ {2, 4, 8}
  (QĐ-SEL2e) × seed 1–6 = 216 lượt, mỗi lượt 18.000 tick. Thêm 54 lượt **nền** cho tỉ số cỏ: cùng seed, cùng
  founders và cùng số hồ, `prey_cap` 80, không dự trữ.
- **Năm điều kiện khả thi** (QĐ-SEL2b), xét trên mọi seed của một ô trừ khi ghi khác:
  1. quần thể lấy mẫu mỗi 50 tick không xuống dưới 5;
  2. phần cỏ ăn được `g = (C̄ − ρ·Σr_max) / (C̄_nền − ρ·Σr_max)` của nửa sau nằm trong 0,4–0,6
     ([ghi chú S10](2026-11-18-s10-plant-reserve.md), điều kiện 5). Nền ở đây giữ cùng founders và cùng số hồ với ô
     đang đọc; nền của S6 thì cố định 30 founders và 2 hồ;
  3. ít nhất 30 thế hệ, lấy số nhỏ hơn của `18.000 / L̄` (`L̄` là tuổi thọ trung bình lúc chết, gộp cả lượt) và
     `số sinh / N̄` (số lần quần thể thay mới);
  4. đói gây hơn nửa số cái chết gộp của ô;
  5. có cả sinh lẫn chết trong một phần tư cuối của lượt.
- Mỗi tick đều kiểm `đói + khát == chết`; không lượt nào dừng vì sai.
- **Điều kiện đo:** mã của S10 tại `f9098a5` (nhánh `feat/s10-plant-reserve`; PR #124 đã merge vào thân cây ở `a128d18`
  mà không đổi `src-tauri/src` so với commit đó). Apple M5 (10 nhân, 32 GB), macOS, rustc 1.98.1, bản release, 8 luồng.
  Mất 12 phút 0 giây kể cả build.

```bash
cargo run --release --example sel2e_exploration > sel2e_exploration.tsv
```

## Kết quả

**Không ô nào khả thi (0/36).** Ba điều kiện hỏng ở **mọi** ô, không riêng gì một ô lưới.

| Điều kiện | Kết quả trên 216 lượt chính | Đạt ở |
|---|---|---|
| 3. ≥ 30 thế hệ | Tuổi thọ lúc chết trung vị 2.637 tick (1.144–3.086). Thế hệ ước lượng trung vị 6,8 theo tuổi thọ, 6,0 theo số lần thay mới; cao nhất 15,7 | 0/36 ô |
| 2. `g` trong 0,4–0,6 | `g` thấp nhất của mọi ô là 0,62; trung vị 0,90 khi không dự trữ, 1,22–1,48 khi có. Quần thể **không** gặm cạn đồng cỏ | 0/36 ô |
| 4. đói > 50 % số chết | Tỉ lệ đói gộp theo ô 0,16–0,46; trung vị theo lượt 0,275 | 0/36 ô |
| 1. tối thiểu ≥ 5 | Chỉ 5 ô đạt (bốn trong năm có 100 founders); 25/216 lượt tuyệt chủng | 5/36 ô |
| 5. còn sinh và chết ở phần tư cuối | 6/6 seed ở 18 ô | 18/36 ô |

Tác động biên của từng chiều (gộp 72 hoặc 54 lượt):

| Chiều | Mức | Tỉ lệ đói | `g` trung vị | Tuyệt chủng | N̄ trung vị |
|---|---|---|---|---|---|
| Hồ | 2 / 4 / 8 | 0,250 / 0,276 / 0,307 | 1,22 / 1,24 / 1,23 | 6 / 10 / 9 trên 72 | 73 / 71 / 75 |
| Dự trữ | 0 / 0,2 / 0,3 / 0,4 | 0,268 / 0,251 / 0,281 / 0,343 | 0,90 / 1,23 / 1,33 / 1,48 | 2 / 9 / 1 / 13 trên 54 | 102 / 82 / 63 / 43 |
| Founders | 30 / 60 / 100 | 0,222 / 0,275 / 0,319 | 1,28 / 1,23 / 1,21 | 8 / 13 / 4 trên 72 | 64 / 64 / 83 |

Bảng đầy đủ theo ô (`SUMMARY`) và theo lượt nằm trong đầu ra của driver; nó không được lưu vào repo.

## Đọc kết quả

1. **18.000 tick chỉ đủ cho khoảng 5–7 thế hệ.** Thế giới sống không có lão hoá; con vật chỉ chết vì đói hoặc
   khát, và sống trung bình khoảng 2.600 tick. Muốn 30 thế hệ thì một lượt cần cỡ 80.000–100.000 tick ở tuổi thọ
   này. Điều kiện 3 không thể đạt với bất kỳ cấu hình nào của lưới, vì lưới giữ độ dài 18.000 tick.
2. **Ở hệ số 1,0, thức ăn không giới hạn quần thể.** Không dự trữ, cỏ nửa sau vẫn còn khoảng 90 % so với nền có
   trần 80. Quần thể dừng ở khoảng 70–100 con, xa dưới trần 320. Thứ giới hạn nó không phải cỏ.
3. **Dự trữ làm quần thể nhỏ đi, không làm nó vững hơn.** Mỗi mức dự trữ bớt phần cỏ ăn được, nên N̄ giảm từ 102
   xuống 43 và số lượt tuyệt chủng ở ρ = 0,4 là 13/54. `g` lớn hơn 1 vì phần dự trữ không bị ăn. Dự trữ được đề
   xuất để chống sụp đổ **khi** khan hiếm; ở hệ số 1,0 thì không có khan hiếm để chống.
4. **Thêm hồ gần như không giảm chết khát.** Từ 2 lên 8 hồ, tỉ lệ đói chỉ tăng từ 0,25 lên 0,31, và quần thể
   không lớn thêm. Vì thế khát không đơn giản là do thiếu nước trên bản đồ. Giả thuyết, chưa kiểm: con vật không
   tới được hồ hoặc không uống đủ trong ân hạn 120 tick (lái tới nước, tốc độ uống, khoảng cách).
5. **Tương quan cặp chỉ là ước lượng thô.** Hệ số Pearson giữa N̄ nửa sau của mỗi ô có dự trữ và ô không dự trữ
   cùng founders, cùng số hồ dao động từ −0,84 đến 0,99 trên 6 seed. Sáu điểm là quá ít để dùng cho QĐ-SEL2c;
   tương quan thật cần đo trên endpoint khối lượng, mà driver này không được đọc.

## Bốn phép đo tiếp theo (cùng ngày, cùng mã, cùng seed 1–6, vẫn mù với khối lượng)

### Chẩn đoán khát (`src-tauri/examples/sel2e_thirst_probe.rs`)

100 founders, 8 hồ, trần 320, không dự trữ, hệ số ∈ {1,0; 0,7} × `live.dehydration_ticks` ∈ {120, 240, 480, 960} ×
6 seed = 48 lượt 18.000 tick, 3 phút 25 giây.

| Hệ số | Ân hạn khát | Tỉ lệ đói | Tối thiểu | Tuyệt chủng | N̄ trung vị | Tuổi thọ gộp | Thế hệ |
|---|---|---|---|---|---|---|---|
| 1,0 | 120 | 0,325 | 13 | 0/6 | 121 | 2.589 | 5,4–7,1 |
| 1,0 | 240 | 0,347 | 8 | 0/6 | 132 | 2.588 | 5,2–6,7 |
| 1,0 | 480 | 0,404 | 10 | 0/6 | 150 | 2.550 | 5,0–6,5 |
| 1,0 | 960 | 0,493 | 36 | 0/6 | 169 | 2.527 | 4,6–5,8 |
| 0,7 | 120 | 0,425 | 1 | 0/6 | 69 | 2.536 | 4,5–6,7 |
| 0,7 | 240 | 0,464 | 1 | 0/6 | 70 | 2.518 | 4,4–7,0 |
| 0,7 | 480 | 0,517 | 3 | 0/6 | 76 | 2.485 | 4,6–6,9 |
| 0,7 | 960 | 0,613 | 15 | 0/6 | 91 | 2.533 | 4,2–6,5 |

Nới ân hạn khát chuyển cái chết từ khát sang đói, và quần thể lớn lên. Tuổi thọ lúc chết thì gần như không đổi
(khoảng 2.500 tick), nên số thế hệ trong 18.000 tick vẫn chỉ 4–7.

### Lượt dài 100.000 tick (`src-tauri/examples/sel2e_long_runs.rs`)

100 founders, 8 hồ, trần 320, hệ số ∈ {0,7; 0,5} × ân hạn khát ∈ {480, 960} × dự trữ ∈ {0; 0,2} × 6 seed, cộng 12
lượt nền (trần 80, hệ số 1,0, không dự trữ, theo từng ân hạn), mỗi lượt 100.000 tick, 6 phút 18 giây.

| Hệ số | Ân hạn | Dự trữ | Tuyệt chủng | N̄ trung vị | `g` | Thế hệ tối thiểu | Tỉ lệ đói |
|---|---|---|---|---|---|---|---|
| 0,7 | 480 | 0 | 4/6 | 26 | 0,37–1,05 | 22,9 | 0,443 |
| 0,7 | 480 | 0,2 | 3/6 | 40 | 0,48–1,06 | 18,6 | 0,338 |
| 0,7 | 960 | 0 | 2/6 | 65 | 0,52–1,00 | 20,0 | 0,528 |
| 0,7 | 960 | 0,2 | 4/6 | 54 | 0,61–1,14 | 18,4 | 0,443 |
| 0,5 | 480 | 0 | 5/6 | 9 | 0,50–0,79 | 18,6 | 0,585 |
| 0,5 | 480 | 0,2 | 6/6 | 7 | 0,44–0,75 | 18,6 | 0,680 |
| 0,5 | 960 | 0 | 4/6 | 19 | 0,45–0,72 | 18,9 | 0,612 |
| 0,5 | 960 | 0,2 | 5/6 | 16 | 0,46–0,75 | 15,9 | 0,592 |

Lượt nền (ân hạn khát 480 và 960): 3/12 tuyệt chủng; với ân hạn mặc định 120, nền không tuyệt chủng lượt nào (chẩn đoán dưới). Không ô nào khả thi; điều kiện 1 (tối thiểu ≥ 5) hỏng ở **mọi** ô.

**Đọc:** chạy lâu thì đủ gần 20 thế hệ và đói thành nguyên nhân chính ở hệ số 0,5. Nhưng quần thể sống của thế
giới này **không bền qua 100.000 tick** khi khan hiếm: phần lớn lượt tuyệt chủng. Phép chẩn đoán dưới đây xem vì sao.

### Chẩn đoán tuyệt chủng (`src-tauri/examples/sel2e_extinction_probe.rs`)

Hai thế giới, 100 founders, 8 hồ, seed 1–6, 100.000 tick, 1 phút 6 giây: **nền** (trần 80, hệ số 1,0, ân hạn khát
mặc định 120) và **tốt nhất** (trần 320, hệ số 0,7, ân hạn khát 960, ô ít tuyệt chủng nhất của lượt dài). Năm ở
đây là 6.000 tick, đúng một chu kỳ mùa.

| Thế giới | Tuyệt chủng | Quỹ đạo quần thể |
|---|---|---|
| Nền | 0/6 | Ghim ở trần 80 gần như cả lượt (thấp nhất mỗi năm thường đúng 80). Trần giữ quần thể, không phải cỏ |
| Tốt nhất | 2/6 (seed 5 ở tick 26.767, seed 6 ở tick 27.814, cả hai trong năm thứ 5) | Dao động bùng nổ và sụp đổ nhiều năm, biên độ khoảng 10 lần (ví dụ seed 1: 21–214, rồi 40–63, rồi 148–187, rồi 44–64). Seed 2 còn sống ở tick 100.000 nhưng chỉ 1–2 con |

Trong 3.000 tick trước tuyệt chủng, **không có lần sinh nào** (seed 5: 0 sinh, 2 chết đói; seed 6: 0 sinh, 4 chết
đói). Seed 2 cũng chỉ có 1 lần sinh trong 3.000 tick cuối. Pha mùa lúc tuyệt chủng khác nhau (`sin` 0,24 và −0,75),
nên không trùng một mùa cố định.

**Đọc:** quần thể không chết vì một cú sốc. Nó trượt dần trong một đáy của chu kỳ bùng nổ và sụp đổ, và ở mật độ
thấp thì **sinh sản dừng hẳn**. Đó là dấu hiệu của hiệu ứng Allee, tức sinh sản hỏng khi mật độ thấp, chứ không
phải của một mùa đông tệ. Vì sao sinh sản dừng khi chỉ còn vài con (ngưỡng năng lượng 0,65 của
`autonomous_reproduction_system`, cỏ quanh chỗ con vật đứng đã cạn, hay con vật không tìm được cỏ) thì chưa đo.

### Chẩn đoán đáy chu kỳ (`src-tauri/examples/sel2e_trough_probe.rs`)

Thế giới "tốt nhất" (100 founders, 8 hồ, trần 320, hệ số 0,7, ân hạn khát 960), seed 2, 5 và 6, tới 40.000 tick hoặc
tới khi tuyệt chủng, 14 giây. Mỗi 500 tick, probe đọc của từng con: tỉ lệ năng lượng (`energy / energy_target`), tỉ
lệ nước, và tỉ lệ cỏ `r / r_max` của ô dưới chân. Nó không đọc khối lượng, genotype hay observable tính trạng nào.
Con mồi sinh sản khi năng lượng ≥ 0,65 mức mục tiêu (`autonomous_reproduction_system`).

| Seed | Kết cục | Mẫu có con đủ ngưỡng sinh | Cỏ dưới chân (trung vị) | Cỏ cả đồng (trung vị) | Tỉ lệ năng lượng (trung vị, cao nhất) |
|---|---|---|---|---|---|
| 2 | Còn 114 con ở tick 40.000 | 0/80 | 0,070 | 0,857 | 0,307; 0,380 |
| 5 | Tuyệt chủng ở tick 27.000 | 0/54 | 0,061 | 0,831 | 0,266; 0,385 |
| 6 | Tuyệt chủng ở tick 28.000 | 0/56 | 0,062 | 0,827 | 0,255; 0,366 |

> Đính chính 2026-11-18: cột thứ ba là hệ quả của việc lấy mẫu ở cuối tick, và đoạn "Cơ chế" dưới đây là giả thuyết
> đã bị thẻ S11 bác. Xem khối đính chính ở đầu ghi chú.

**Đọc:** cỏ **không** khan hiếm. Đồng cỏ còn khoảng 83–86 % sức chứa, nhưng ô dưới chân con vật chỉ còn khoảng 6–7 %.
Ở không một lần lấy mẫu nào có con đạt ngưỡng sinh sản. Các lần sinh vẫn xảy ra giữa hai lần lấy mẫu (618, 410 và 420
lần), khi một con vượt ngưỡng trong chốc lát, chẳng hạn sau khi ăn một mẩu `Food` hay quả cây, rồi đầu tư một nửa năng
lượng vào con.

**Cơ chế, đọc từ mã:** `steering_drive_system` (`core/locomotion.rs`) lái con vật tới hồ khi khát, và tới con mồi hoặc
mẩu `Food` khi đói. Nó **không bao giờ** nhìn đồng cỏ (`ResourceField`). Cỏ chỉ được ăn ở ô ngay dưới chân
(`herbivore_grazing_system`). Vì vậy con vật gặm trụi ô của mình, nhất là quanh hồ nơi lái-tới-nước kéo chúng lại, và
chết đói giữa một đồng cỏ gần như đầy. Đây là giải thích đơn giản nhất cho cả bốn phép đo trước: hệ số đồng cỏ tác
động yếu và gián tiếp, vì con vật gần như không dùng tới đồng cỏ ngoài ô dưới chân.

## Không khẳng định

- Không nói gì về khối lượng cơ thể hay chọn lọc. Driver mù với khối lượng.
- Không nói rằng không cấu hình nào khả thi. Chỉ nói lưới này, ở 18.000 tick và hệ số 1,0, không có ô nào.
- Giả thuyết ở mục 4 chưa được kiểm.

## Hệ quả cho bước kế tiếp (đề xuất, chủ dự án quyết)

Theo đề xuất SEL2 §5, khi SEL2-E âm thì bước kế tiếp là các mục ở §5.5, **không phải nới điều kiện**. Bốn phép đo
cùng chỉ về một điều: trước khi hỏi khan hiếm thức ăn chọn lọc gì, thế giới sống phải **duy trì được** một quần
thể qua vài chục thế hệ. Hiện nó không làm được. Đề xuất theo thứ tự:

1. **Cho con vật tìm cỏ** (sửa luật hành vi, mở kỷ engine mới theo ADR-0008, cần quyết định và có lẽ một cập nhật
   ADR-0006/ADR-0007): `steering_drive_system` thêm một mục tiêu "ô cỏ còn nhiều gần đây" khi đói, hoặc đi theo
   gradient của `r`. Phép chẩn đoán đáy chu kỳ cho thấy đây là thứ chặn: cỏ đầy đồng nhưng con vật không tới được.
   *(Đính chính 2026-11-18: đã làm ở thẻ S11, kết quả âm; xem khối đính chính ở đầu ghi chú.)*
   Đặt sau một khoá opt-in như mọi thay đổi sinh thái trước, để đường mặc định giữ nguyên.
2. **Khát:** một ân hạn dài hơn (`live.dehydration_ticks` 960) là cách rẻ nhất để đói thành nguyên nhân chính. Đổi nó
   là đổi một tham số thế giới, nên cần quyết định của chủ dự án và phải khai trong bản đăng ký.
3. **Lão hoá** (§5.5, cần ADR): một tuổi thọ tối đa sẽ đặt thời gian thế hệ độc lập với khủng hoảng đói/khát.
