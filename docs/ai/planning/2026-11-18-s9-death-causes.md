---
phase: planning
feature: experiment-sel2-food-scarcity
title: Thẻ S9 — tách nguyên nhân chết (đói và khát) và đo lại bốn ô của lưới S6
description: Thế giới sống nay đếm riêng chết vì đói và chết vì khát (live.deaths_starvation, live.deaths_dehydration). Đo lại các ô hệ số 0,5 và 0,7 của lưới S6, mù với khối lượng cơ thể. Kết quả - khát giết 71–80 % số cá thể chết ở cả bốn ô; đói chỉ chiếm phần lớn ở 1 trong 18 lượt khác nhau (seed 3, hệ số 0,5, giống hệt nhau ở hai trần)
status: completed
owner: simulation-architecture
last_reviewed: 2026-11-18
---

# Thẻ S9 — trong các cấu hình khan hiếm, đói hay khát giết quần thể?

> **Thăm dò, chưa đăng ký, mù với khối lượng cơ thể.** Driver
> (`src-tauri/examples/s9_death_causes.rs`) đọc observable qua một danh sách cho phép gồm `live.agent_count`,
> `live.births`, `live.deaths`, `live.deaths_starvation` và `live.deaths_dehydration`. Mọi observable khác bị
> bỏ mà không đọc. Ngoài danh sách đó, driver đọc đúng một bộ đếm trong `VitalStatistics`:
> `total_deaths_both_lethal`. Không giá trị `live.trait_mass_*`, genotype hay kích thước cơ thể nào được in,
> lưu hay dùng; đầu ra có 0 dòng chứa chữ `mass` (`grep -ci mass` = 0). Seed 1–6 giống
> [S6](2026-11-17-s6-sel2-exploration.md) và **không** phải seed xác nhận.

## Câu hỏi

Đề xuất SEL2 (`docs/ai/planning/2026-11-18-sel2-redesign-proposal.md`, PR #116, chưa merge khi đo; QĐ-SEL2a) đặt S9 trước mọi lượt đo thăm dò mới. Nếu khát
giết phần lớn quần thể ở các cấu hình khan hiếm, thì thao tác thức ăn không thể là yếu tố duy nhất của SEL2.
Điều kiện khả thi thứ 4 của SEL2-E ("phần lớn cái chết là do đói, không do khát") cần chính con số này.

Trước S9, `mortality_system` chỉ ghi một cái chết không phân loại vào `VitalStatistics`. Còn
`DemographicTracker` ghi cái chết vì khát dưới nhãn `"Starvation"` và mọi cái chết ở tuổi ≥ 300 tick dưới nhãn
`"Senescence"`, dù mã không có lão hoá. Không con số nào dùng được.

## Cái đã thay đổi trong mã

- `core::mortality::lethal_cause` gán **đúng một nguyên nhân** cho mỗi cái chết:
  - chỉ một điều kiện gây chết thì đó là nguyên nhân;
  - cả hai cùng gây chết trong một tick thì nguyên nhân là điều kiện **gây chết trước**, tức chuỗi tick ở đáy
    vượt ân hạn của chính nó nhiều hơn (`chuỗi − ân hạn`);
  - hoà tuyệt đối thì tính là **đói**.

  Trong một lượt chạy bật tử vong từ đầu, con vật chết ngay ở tick đầu tiên có một điều kiện gây chết. Vì thế
  "cả hai cùng gây chết" chỉ xảy ra khi hai chuỗi chạm ân hạn cùng một tick, và khi đó quy tắc hoà quyết định.
- `VitalStatistics` thêm hai bộ đếm cộng dồn (`total_deaths_starvation`, `total_deaths_dehydration`). Chúng được
  lấy mẫu theo cùng cửa sổ với `live.deaths`, và `sample_window` đóng cả ba cùng lúc. Thêm một bộ đếm cộng dồn
  `total_deaths_both_lethal`, không phải observable, đếm số cái chết mà quy tắc trên đã phải quyết định.
- Adapter sống báo `live.deaths_starvation` và `live.deaths_dehydration`. Hai id này được nối vào cuối
  `LIVE_EXTENDED_OBSERVABLE_IDS` và chịu cùng cổng `live.mortality` như `live.deaths`. `LIVE_OBSERVABLE_IDS`
  không đổi (QĐ-E2M).
- Không đổi quỹ đạo: phần tách nằm ngoài `world_checksum`. `tests/death_cause_split_tests.rs` so hai lượt chạy
  cùng seed, một có và một không yêu cầu hai observable mới: checksum cuối và mọi giá trị của 12 observable
  cũ ở mọi mẫu đều trùng.
- `DemographicTracker` **chưa** được sửa; việc đó là một thẻ riêng.

## Cách đo

- **Thế giới nền** của S6 (cũng là của E-SEL1): 30 founder con mồi, không thú săn, 8 cây, 2 hồ. Bật tử vong,
  sinh sản và vận động với `drive_floor` 0,5 cùng lái tới nước. Não chung, `food_cap` 50. Ân hạn đói và khát đều
  là 120 tick (mặc định, không khai báo).
- **Lưới:** `live.prey_cap` ∈ {80, 160} × `live.resource_capacity_scale` ∈ {0,5; 0,7} × seed 1–6, tức 24 lượt.
  Mỗi lượt 18.000 tick, bản release. Sinh, chết và hai nguyên nhân được đọc ở **mọi** tick nên cửa sổ cộng lại
  thành tổng của lượt chạy. Mọi tick đều kiểm `đói + khát == chết`; nếu sai, lượt chạy dừng. Không lượt nào
  dừng.
- **Điều kiện đo:** commit `adfb819` (thân cây `8c7c2c7` cộng commit S9, nhánh `feat/s9-death-causes`, PR #117),
  Apple M5 (10 nhân, 32 GB), macOS 27.2.1, rustc 1.98.1. Mất 239 giây, trong đó 151 giây để build release.

```bash
cargo run --release --example s9_death_causes > s9_death_causes.tsv
```

## Kết quả

Gộp sáu seed của mỗi ô:

| Trần | Hệ số | Chết | Vì đói | Vì khát | Tỉ lệ đói | Tỉ lệ khát | Tỉ lệ đói theo seed (min..max) | Cả hai cùng gây chết | Seed tuyệt chủng |
|---|---|---|---|---|---|---|---|---|---|
| 80 | 0,5 | 749 | 214 | 535 | 0,286 | 0,714 | 0,190..0,529 | 0 | 2/6 |
| 80 | 0,7 | 1.589 | 317 | 1.272 | 0,199 | 0,801 | 0,152..0,277 | 0 | 1/6 |
| 160 | 0,5 | 749 | 214 | 535 | 0,286 | 0,714 | 0,190..0,529 | 0 | 2/6 |
| 160 | 0,7 | 1.641 | 363 | 1.278 | 0,221 | 0,779 | 0,178..0,291 | 0 | 0/6 |

Từng lượt (trần 160 ở hệ số 0,5 giống hệt trần 80 tới từng số nên không lặp lại):

| Trần | Hệ số | Seed | Agent cuối | Sinh | Chết | Vì đói | Vì khát | Tỉ lệ khát |
|---|---|---|---|---|---|---|---|---|
| 80 và 160 | 0,5 | 1 | 49 | 150 | 131 | 39 | 92 | 0,702 |
| 80 và 160 | 0,5 | 2 | 0 | 71 | 101 | 31 | 70 | 0,693 |
| 80 và 160 | 0,5 | 3 | 0 | 38 | 68 | 36 | 32 | 0,471 |
| 80 và 160 | 0,5 | 4 | 2 | 133 | 161 | 42 | 119 | 0,739 |
| 80 và 160 | 0,5 | 5 | 3 | 94 | 121 | 23 | 98 | 0,810 |
| 80 và 160 | 0,5 | 6 | 6 | 143 | 167 | 43 | 124 | 0,743 |
| 80 | 0,7 | 1 | 80 | 363 | 313 | 76 | 237 | 0,757 |
| 80 | 0,7 | 2 | 0 | 183 | 213 | 46 | 167 | 0,784 |
| 80 | 0,7 | 3 | 78 | 225 | 177 | 49 | 128 | 0,723 |
| 80 | 0,7 | 4 | 10 | 276 | 296 | 45 | 251 | 0,848 |
| 80 | 0,7 | 5 | 17 | 273 | 286 | 45 | 241 | 0,843 |
| 80 | 0,7 | 6 | 45 | 319 | 304 | 56 | 248 | 0,816 |
| 160 | 0,7 | 1 | 12 | 260 | 278 | 81 | 197 | 0,709 |
| 160 | 0,7 | 2 | 18 | 234 | 246 | 52 | 194 | 0,789 |
| 160 | 0,7 | 3 | 78 | 225 | 177 | 49 | 128 | 0,723 |
| 160 | 0,7 | 4 | 5 | 233 | 258 | 46 | 212 | 0,822 |
| 160 | 0,7 | 5 | 11 | 308 | 327 | 65 | 262 | 0,801 |
| 160 | 0,7 | 6 | 28 | 353 | 355 | 70 | 285 | 0,803 |

Số chết bao gồm cả 30 founder, nên chết có thể lớn hơn sinh.

## Đọc kết quả

1. **Khát là nguyên nhân chết chính ở cả bốn ô.** Khát chiếm 0,71–0,80 số cá thể chết khi gộp seed. Trong 24
   lượt (18 lượt khác nhau, vì ở hệ số 0,5 hai trần cho cùng một lượt), chỉ một lượt có đói nhiều hơn khát:
   hệ số 0,5, seed 3 (cùng kết quả ở trần 80 và 160), với 36 so với 32, ở một lượt mà quần thể tuyệt chủng sau 68
   cái chết.
2. **Hạ sức chứa đồng cỏ làm tăng tỉ lệ đói, nhưng không đổi thứ tự.** Từ hệ số 0,7 xuống 0,5, tỉ lệ đói gộp
   tăng từ 0,20–0,22 lên 0,29. Cả hai mức đều xa ngưỡng "phần lớn".
3. **Quy tắc hoà không quyết định cái chết nào.** `total_deaths_both_lethal` = 0 ở mọi lượt, nên các tỉ lệ trên
   không phụ thuộc vào việc hoà được tính là đói.
4. **Khớp với S6 ở những gì có thể so.** Số sinh ở hệ số 0,5 nằm trong khoảng 38–150 và số seed tuyệt chủng (2/6
   ở hệ số 0,5; 1/6 với trần 80 và 0/6 với trần 160 ở hệ số 0,7) trùng với S6, đo trên thân cây `25c0458`. Hai
   trần cho kết quả giống hệt nhau ở hệ số 0,5, như S6 đã thấy. Đây là một kiểm tra chéo yếu (S6 không lưu từng
   lượt), không phải bằng chứng quỹ đạo không đổi. Bằng chứng đó là test so checksum ở trên.

## Không khẳng định

- Đây là nguyên nhân **gần** (tài nguyên nào chạm đáy đủ lâu trước), không phải nguyên nhân sinh thái. Một con
  vật chết khát có thể đã khát vì mất thời gian kiếm ăn. Số liệu không tách được hai điều đó.
- Không nói gì về body mass hay chọn lọc. Driver mù với khối lượng.
- Chỉ đúng với thế giới nền này: 2 hồ, ân hạn 120/120 tick, vận động với lái tới nước. Đổi số hồ hay ân hạn có
  thể đổi thứ tự.

## Hệ quả cho bước kế tiếp (đề xuất, chủ dự án quyết)

- Điều kiện khả thi thứ 4 của SEL2-E ("phần lớn cái chết là do đói") **không đạt ở ô nào** trong bốn ô này.
- Đề xuất SEL2 nói bỏ chiều `live.lakes` nếu khát không giết đáng kể. Khát giết phần lớn, nên **giữ chiều
  `live.lakes`** trong lưới SEL2-E, và nên xét thêm mức hồ cao hơn 4 nếu điều kiện 4 vẫn được giữ.
- Một lựa chọn khác cho chủ dự án: nới ân hạn khát (`live.dehydration_ticks`) thay vì thêm hồ. Đề xuất SEL2 chưa
  bàn tới cách này.
