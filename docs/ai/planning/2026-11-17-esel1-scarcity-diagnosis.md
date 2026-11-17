---
phase: planning
feature: experiment-e-sel1-food-scarcity
title: Chẩn đoán sau E-SEL1 — cái gì giới hạn quần thể con mồi, và yếu tố khan hiếm nào dùng được
description: Đo thăm dò, không đọc khối lượng cơ thể, sau khi E-SEL1 đóng tại smoke. Kết quả - đồng cỏ là nguồn sống và không bao giờ cạn vì sinh sản dừng ở 80 con mồi; nước giới hạn quần thể; chỉ sức chứa đồng cỏ hạ được standing crop. Đầu vào cho quyết định QĐ-SEL1b của chủ dự án
status: active
owner: simulation-architecture
last_reviewed: 2026-11-17
---

# Chẩn đoán sau E-SEL1: cái gì giới hạn quần thể con mồi

> **Thăm dò, chưa đăng ký, chưa được kiểm độc lập.** Một agent đo bằng bộ đếm tạm (scratch) gắn vào bốn hệ.
> Bộ đếm đó không được commit. Hai điểm then chốt đã được kiểm lại trong code: trần sinh sản 80 con mồi và bộ
> đếm chung của tử vong. Mọi con số khác là số đo thăm dò.
>
> **Mù với kết quả.** Không giá trị `live.trait_mass_*`, kích thước genotype hay khối lượng cơ thể nào được đọc,
> in hay dùng. Seed dùng là 1–6: không phải seed thí nghiệm `760001 + 1000·k`, cũng không phải seed smoke 999971.
> Tài liệu này không thay đổi gì của [bản đăng ký E-SEL1](2026-11-16-experiment-e-sel1-food-scarcity-body-mass.md),
> bản đã đóng không có thí nghiệm.

## Cấu hình

Cấu hình chung của E-SEL1: `live.founders` 30, `predator_fraction` 0, 8 cây, 2 hồ, tử vong, sinh sản, vận động,
`drive_floor` 0.5, `thirst_steering` 1, não chung, `food_cap` 50 trừ khi ghi khác. Bản release, 18 000 tick, lấy
mẫu mỗi 50 tick.

## 1. Nguồn năng lượng của con mồi

| Trần `Food` | Gặm đồng cỏ | Quả cây | Mẩu `Food` |
|---|---|---|---|
| 50 | 90–93 % (44k–56k EU) | 4,6–5,6 % | 2,5–5,7 % (44–106 mẩu / 18 000 tick) |
| 6 | phần còn lại | 0–6,6 % | 0,5–1,9 % (6–15 mẩu) |

Nơi cộng năng lượng:
- gặm: `environmental_systems.rs:133-136`;
- `Food`: `world_systems.rs:289-296`;
- quả: `environmental_systems.rs:335-340`.

`Food` và quả chỉ là phần rút từ hồ detritus. Chỉ có gặm là rút từ đồng cỏ. **Trần `Food` đánh vào một nguồn chỉ
chiếm vài phần trăm thu nhập**, nên hai lần smoke của E-SEL1 không hạ được standing crop là điều dễ hiểu.

## 2. Đồng cỏ không bao giờ cạn, vì sinh sản có trần

- **Standing crop luôn cao.** Ở trần 50, nó nằm ở 0,75–0,85 sức chứa (trung bình nửa sau 115,9k–129,9k, so với
  sức chứa khoảng 155k). Kể cả ở trần 0, nó chỉ xuống 0,67–0,73.
- **Tiêu thụ quá nhỏ.** Khoảng 65 agent ăn khoảng 2,5–3 EU mỗi tick, so với kho khoảng 120k EU.
- **Thêm founder cũng không giúp.** Với 200 founder, crop vẫn 124k–137k.
- **Sinh sản có trần cứng** (`agent_systems.rs:715`, đã kiểm). Con mồi không sinh khi đã có ≥ 80 con mồi. Thú săn
  không sinh khi đã có ≥ 8 thú săn, hoặc khi số con mồi < 5 × số thú săn. Quần thể vì thế tăng rồi sụp quanh trần
  80, không bao giờ đông tới mức gặm cạn đồng cỏ. [ADR-0005](../../decisions/ADR-0005-live-population-turnover.md)
  đã ghi các hằng này là "quyết định riêng (thẻ tham số hoá)".
- **Hạ tốc độ mọc lại không có tác dụng.** Đặt `growth_rate` × 0,1 hoặc × 0,3, crop vẫn 115k–124k.

## 3. Nước, không phải thức ăn, giới hạn quần thể

| Đại lượng | Số đo mỗi lượt chạy |
|---|---|
| Chết khát | 300–430 |
| Chết đói | 50–70 |
| Sinh | 407–537 |

Ở smoke seed 999971, nhánh dồi dào có quần thể tối thiểu thấp hơn nhánh khan hiếm. Hiện tượng đó **không lặp lại
thành một hiệu ứng của trần** trên seed 1–6:
- trần 0 cho quần thể cao nhất;
- trần 6 cho tối thiểu thấp nhất, và tuyệt chủng ở seed 1 và 3.

Nên đọc nó là dao động tăng-sụp. Giả thuyết, chưa kiểm: `steering_drive_system` (`locomotion.rs:107-131`) cho
con mồi không khát lái tới mẩu `Food` gần nhất. Ở trần 0 chúng không có đích nên đứng yên.

## 4. `live.mutation_rate = 0` và tuyệt chủng (n = 6)

| Tốc độ đột biến | 0 | 0,01 | 0,03 | 0,05 | 0,1 | 0,2 |
|---|---|---|---|---|---|---|
| Seed tuyệt chủng / 6 | 2 | 1 | 1 | 0 | 0 | 0 |
| Quần thể tối thiểu trung bình | 9 | 18 | 27 | 30 | 35 | 37 |

Đây là đáp ứng theo liều trên cùng sáu seed, nên khó là một ảo ảnh của luồng RNG thuần. Ở tốc độ 0, con là bản
sao chính xác (`agent_systems.rs:774-786`). Vì sao đột biến giúp quần thể hồi phục thì chưa xác định: muốn biết
phải nhìn dữ liệu tính trạng, mà làm vậy là phá tính mù. Hệ quả cho mọi thiết kế có nhánh "drift" với đột biến 0:
nhánh đó có thể tuyệt chủng vì lý do không liên quan tới chọn lọc.

## 5. Yếu tố khan hiếm ứng viên

Tỉ số dưới đây là crop nửa sau so với nền (trần 50, trung bình 122,3k). Quần thể tối thiểu là khoảng trên sáu
seed.

| Yếu tố | Tỉ số crop | Quần thể tối thiểu | Tuyệt chủng | Khoá manifest |
|---|---|---|---|---|
| `food_cap` 25 / 13 / 6 / 0 | 0,94–1,2 | | | có (`live.food_cap`) |
| `growth_rate` × 0,1 / 0,3 | 0,96–1,0 | 6–50 | 0 | chưa có |
| 0 cây | ≈ 1,0 | 0–34 | 1 | có (`live.trees`) |
| 200 founder | ≥ 1,0 | 0–55 | 1 | có |
| sức chứa `r_max` × 0,7 | 0,72 | 0–35 | 1 | chưa có |
| `r_max` × 0,6 | 0,61–0,73 | 0–16 | 1 | chưa có |
| `r_max` × 0,5 | 0,55–0,61 | 0–5 | 2 | chưa có |
| `r_max` × 0,35 | ≈ 0,42 | 0–3 | 5 | chưa có |
| `r_max` × 0,6 + đột biến 0,2 | 0,61–0,73 | 0–27 | 1 | chưa có |

Cách đo `r_max`: nhân `r_max` và `r` lúc tick 0, rồi chuyển phần EU bị bớt từ đồng cỏ sang detritus để sổ EU vẫn
kín.

Đọc bảng:
- Chỉ **sức chứa đồng cỏ** đưa được tỉ số vào dải 0,4–0,6. Nhưng tỉ số đi theo `r_max` gần như tuyến tính, nên
  phần lớn là do cách dựng.
- Ở trần sinh sản 80, sức chứa thấp làm quần thể chết, nên ràng buộc "tối thiểu ≥ 5" hỏng ở 3–4 trên 6 seed.
- Hạ sức chứa làm giảm quần thể (trung bình 5–55 con), nên nó không cô lập được "thức ăn" như một yếu tố riêng.
- `LiveForcings.fertility_multiplier` không đặt sẵn được, vì `apply_live_interventions_system` tính lại nó mỗi tick.

## 6. Nghi vấn trong code

- **F-044 (SMELL, đã kiểm trong code).** `mortality_system` (`mortality.rs:58`) so cả hai điều kiện chết đói và
  chết khát với **cùng một** bộ đếm `starving_ticks`. Bộ đếm này tăng khi năng lượng **hoặc** nước chạm đáy
  (`world_systems.rs` trong `metabolic_decay_system`).
  - Khi `starve_ticks` khác `dehydration_ticks`, hai mức ân hạn không độc lập. Một con đã đói 100 tick sẽ chết khát
    ngay tick nước chạm 0, không có ân hạn khát nào.
  - Với mặc định (120 / 120), quy tắc tương đương "120 tick liên tiếp ở đáy, bất kể đáy nào".
  - Chết khát được ghi là "Starvation" hoặc "Senescence" trong `DemographicTracker`. Code tự ghi chú điều này; chỉ
    ảnh hưởng telemetry.
- **`drive_floor = 0`** cho seed 1 và 3 trùng từng bit với `drive_floor = 0,5`, còn seed 2, 4, 5, 6 tuyệt chủng
  quanh tick 5 500. Điều này khớp với bằng chứng của
  [ADR-0007](../../decisions/ADR-0007-live-ecology-sustainability.md): founder của một số seed có biên độ CPG đủ
  lớn nên sàn không bao giờ chạm, còn founder của seed khác gần như bất động. Chưa coi là lỗi.

## 7. Chưa đo

- Chi phí di chuyển của từng agent.
- Nước (dung tích hồ, tốc độ bổ sung) như một yếu tố khan hiếm.
- Seed ngoài 1–6.
- Liên hệ nhân quả giữa lái và quần thể.
- Seed 999971: tỉ số 0,94–1,03 của smoke chưa được lặp lại mà cũng chưa bị bác.
- Mọi thứ về khối lượng cơ thể, cố ý.

## 8. Quyết định cho chủ dự án: QĐ-SEL1b

> **Đã quyết 2026-11-17: phương án A.** Bản ghi ở
> [roadmap 31/10 §4.1](../../planning/COMPLETION_ROADMAP_2026-10-31.md#41-quyết-định-sau-2026-11-14-ghi-thêm-2026-11-17);
> thẻ S6 và SEL2 trong kế hoạch swarm. Phần dưới là câu hỏi như đã đặt ra.

Câu hỏi: có làm lại thí nghiệm chọn lọc theo khan hiếm không, và nếu có thì với yếu tố nào? Mọi phương án đều
cần khoá manifest mới (vắng khoá thì như cũ) và một bản đăng ký mới. Không phương án nào sửa bản E-SEL1 đã đóng.

- **A. Trần sinh sản khai báo được, rồi khan hiếm đồng cỏ.**
  - Khoá `live.prey_cap` (vắng = 80) và `live.resource_capacity_scale` (vắng = 1).
  - Trần đủ cao thì quần thể mới đông tới mức thức ăn giới hạn nó. Đó là điều kiện để "khan hiếm thức ăn" có nghĩa
    trong thế giới này.
  - Trước khi đăng ký, cần một phép đo thăm dò (mù như trên): với trần cao hơn, sức chứa nào cho tỉ số 0,4–0,6 mà
    tối thiểu ≥ 5 trên đủ seed.
  - Gần với [dự thảo E3 về gradient năng suất](2026-08-26-experiment-e3-productivity-gradient-body-mass.md).
- **B. Khan hiếm nước.** Nước mới là giới hạn thật. Cần khoá cho dung tích hoặc tốc độ bổ sung của hồ. Câu hỏi
  đổi thành "khan hiếm nước có dịch chuyển khối lượng cơ thể không". Chưa đo.
- **C. Chỉ hạ sức chứa đồng cỏ, giữ trần 80.** Đây là ứng viên duy nhất đã đo vào dải 0,4–0,6, nhưng tỉ số phần
  lớn do dựng, và sống sót sát ngưỡng (hỏng ở 3–4 trên 6 seed).
- **D. Dừng dòng chọn lọc theo khan hiếm**, chuyển sức sang S43 (P1).

**Khuyến nghị: A**, kèm phép đo thăm dò trước khi đăng ký. Lý do: trần 80 là nguyên nhân cấu trúc khiến quần thể
không bị tài nguyên giới hạn, và nó ảnh hưởng mọi thí nghiệm chọn lọc trên thế giới live, không chỉ E-SEL1. Cả A,
B, C đều không phá đường mặc định.

Có một hệ quả độc lập với lựa chọn trên. Một nhánh "drift" dùng `mutation_rate = 0` có thể tuyệt chủng vì lý do
khác chọn lọc (§4). Bản đăng ký sau nên dùng một nhánh trôi dạt khác (ví dụ đột biến bằng nhau, chọn lọc tắt),
hoặc ghi rõ rủi ro này.
