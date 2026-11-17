---
phase: planning
feature: experiment-sel2-food-scarcity
title: Thẻ S6 — đo thăm dò cho SEL2 với trần sinh sản và sức chứa đồng cỏ khai báo được
description: Đo thăm dò, mù với khối lượng cơ thể, sau khi có live.prey_cap và live.resource_capacity_scale. Kết quả - không cấu hình nào vừa đưa standing crop vào dải 0,4–0,6 vừa giữ quần thể tối thiểu ≥ 5 ở mọi seed. Đầu vào cho bước kế tiếp của QĐ-SEL1b
status: active
owner: simulation-architecture
last_reviewed: 2026-11-17
---

# Thẻ S6 — đo thăm dò cho SEL2: nâng trần sinh sản có tạo ra khan hiếm thức ăn không?

> **Thăm dò, chưa đăng ký, mù với khối lượng cơ thể.** Driver (`src-tauri/examples/sel2_exploration.rs`) đọc
> observable qua một danh sách cho phép gồm `live.agent_count`, `live.standing_crop`, `live.births` và
> `live.deaths`. Mọi observable khác bị bỏ mà không đọc. Không giá trị `live.trait_mass_*`, kích thước genotype
> hay khối lượng cơ thể nào được in, lưu hay dùng; đầu ra có 0 dòng chứa chữ `mass`. Seed 1–6 giống bản
> [chẩn đoán sau E-SEL1](2026-11-17-esel1-scarcity-diagnosis.md) và **không** phải seed xác nhận: bản đăng ký
> nào dựa trên tài liệu này phải chọn seed riêng.

## Câu hỏi

[QĐ-SEL1b = A](2026-11-17-esel1-scarcity-diagnosis.md) đặt giả thuyết sau. Trần sinh sản 80 con mồi
(`agent_systems.rs`) giữ quần thể quá thưa để gặm cạn đồng cỏ. Nâng trần lên (`live.prey_cap`) thì quần thể đủ
đông để thức ăn trở thành giới hạn. Khi đó hạ sức chứa đồng cỏ (`live.resource_capacity_scale`) sẽ tạo ra một chế
độ khan hiếm thật.

Bản đăng ký cần đồng thời hai điều:
- tỉ số standing crop (trung bình nửa sau so với nền cùng seed) nằm trong dải **0,4–0,6**;
- quần thể tối thiểu **≥ 5 ở mọi seed**.

## Cách đo

- **Thế giới nền** của E-SEL1: 30 founder con mồi, không thú săn, 8 cây, 2 hồ. Bật tử vong, sinh sản, vận động
  với `drive_floor` 0,5 và lái tới nước. Não chung, `food_cap` 50.
- **Lưới:** `live.prey_cap` ∈ {80, 160, 320} × `live.resource_capacity_scale` ∈ {1,0; 0,7; 0,5; 0,35} × seed
  1–6, tức 72 lượt. Mỗi lượt 18.000 tick, bản release, lấy mẫu số agent mỗi 50 tick, đọc sinh và chết ở mọi tick.
- **Tỉ số crop** = trung bình standing crop nửa sau chia cho nền cùng seed (trần 80, hệ số 1,0), như bản chẩn
  đoán.
- **Điều kiện đo:** thân cây `25c0458` cộng commit S6 (#110), macOS arm64. Tổng thời gian 118 giây.

```bash
cargo run --release --example sel2_exploration > sel2_exploration.tsv
```

## Kết quả

| Trần | Hệ số | Tỉ số crop (min..max) | Quần thể tối thiểu thấp nhất | Seed tuyệt chủng | Seed có tối thiểu ≥ 5 |
|---|---|---|---|---|---|
| 80 | 1,0 | 1,000 (nền) | 8 | 0/6 | 6/6 |
| 80 | 0,7 | 0,682..0,787 | 0 | 1/6 | 4/6 |
| 80 | 0,5 | 0,546..0,604 | 0 | 2/6 | 1/6 |
| 80 | 0,35 | 0,405..0,432 | 0 | 5/6 | 0/6 |
| 160 | 1,0 | 0,852..1,009 | 2 | 0/6 | 5/6 |
| 160 | 0,7 | 0,691..0,769 | 2 | 0/6 | 4/6 |
| 160 | 0,5 | 0,546..0,604 | 0 | 2/6 | 1/6 |
| 160 | 0,35 | 0,405..0,432 | 0 | 5/6 | 0/6 |
| 320 | 1,0 | 0,800..1,009 | 2 | 0/6 | 5/6 |
| 320 | 0,7 | 0,691..0,769 | 2 | 0/6 | 4/6 |
| 320 | 0,5 | 0,546..0,604 | 0 | 2/6 | 1/6 |
| 320 | 0,35 | 0,405..0,432 | 0 | 5/6 | 0/6 |

Chi tiết từng seed ở hệ số 1,0 và 0,5:

| Trần | Hệ số | Seed 1–6: tối đa agent | Seed 1–6: tối thiểu agent | Sinh (khoảng) |
|---|---|---|---|---|
| 80 | 1,0 | 105, 109, 124, 116, 113, 118 | 8, 27, 28, 61, 44, 15 | 407–537 |
| 320 | 1,0 | 165, 149, 394, 216, 179, 229 | 7, 2, 31, 30, 17, 8 | 505–1.177 |
| 80, 160 và 320 (giống hệt) | 0,5 | 67, 76, 63, 76, 78, 76 | 1, 0, 0, 2, 3, 5 | 38–150 |

## Đọc kết quả

1. **Không cấu hình nào trong lưới đạt cả hai điều kiện.** Hệ số 0,5 và 0,35 đưa tỉ số vào dải (0,55–0,60 và
   0,41–0,43), nhưng quần thể chết hoặc gần chết ở 5 trên 6 seed. Hệ số 0,7 giữ được quần thể ở 4/6 seed, nhưng
   tỉ số chỉ xuống 0,68–0,79.
2. **Khi sức chứa đã bị hạ, trần không còn tác dụng.** Ở hệ số 0,5 và 0,35, ba mức trần cho kết quả **giống hệt
   nhau tới từng số**, vì quần thể không bao giờ tới 80 (tối đa 63–78). Tỉ số crop ở đó vì thế chủ yếu do chính
   phép nhân sức chứa đặt ra, không do bị gặm, đúng như bản chẩn đoán đã thấy với phương án C.
3. **Ở sức chứa đầy đủ, nâng trần cho bùng nổ rồi sụp.** Quần thể vượt xa 80 (tối đa 149–394 ở trần 320), số lần
   sinh tăng tới 1.177, đồng cỏ chỉ giảm còn 0,80–1,0, còn tối thiểu tụt xuống 2–8 ở 4 trên 6 seed. Gặm có cắn vào
   đồng cỏ, nhưng không tới mức một chế độ khan hiếm ổn định.
4. **Trần không chặn tuyệt đối.** Ở trần 80, số agent tối đa là 105–124. Trần được so với số con mồi đếm ở đầu
   system, nên mọi con đủ điều kiện trong cùng một tick đều sinh được. Hành vi này đã có từ trước và giữ nguyên.

**Kết luận thăm dò.** Ở thế giới live hôm nay, phương án A, tức nâng trần rồi hạ sức chứa, **không** cho ra một chế
độ mà thức ăn giới hạn một quần thể sống được. Một bản đăng ký SEL2 dựng trên A với lưới này sẽ hỏng ở điều kiện
"quần thể tối thiểu ≥ 5" trước khi chạy được seed nào.

## Chưa đo

- **Nguyên nhân chết** (đói hay khát) ở các cấu hình hạ sức chứa. Bản chẩn đoán đã thấy nước giới hạn quần thể
  ở cấu hình nền, nhưng lượt này không tách nguyên nhân.
- **Trần cao hơn 320, hệ số giữa 0,5 và 0,7**, và kết hợp với khoá nước.
- **Seed ngoài 1–6.**

## Gợi ý cho bước kế tiếp (cần chủ dự án quyết)

- **Đếm nguyên nhân chết** ở hệ số 0,5–0,7. Nếu khát chiếm đa số, khan hiếm thức ăn không thể là yếu tố duy nhất
  được thao tác. Đó là lý lẽ cho phương án B (khan hiếm nước) hoặc cho khoá nguồn nước của thẻ S7, làm trước SEL2.
- **Nếu vẫn giữ A:** đo dày quanh hệ số 0,6–0,7 với trần 160–320 và nhiều seed hơn, chấp nhận dải tỉ số rộng
  hơn, hoặc đổi điều kiện sống sót. Cả hai đều là quyết định thiết kế của bản đăng ký, không phải của thẻ này.
