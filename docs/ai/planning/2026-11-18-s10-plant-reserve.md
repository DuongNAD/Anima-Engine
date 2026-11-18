---
phase: planning
feature: experiment-sel2-food-scarcity
title: Thẻ S10 — khoá live.plant_reserve (phần cây con vật không ăn được)
description: Thế giới sống có khoá opt-in live.plant_reserve, một phân số ρ của r_max mỗi ô mà con vật không gặm được, áp cho cả gặm cỏ sống lẫn quần thể gộp. Khoá vắng thì quỹ đạo trùng commit trước S10; với ρ = 0,3 không ô nào xuống dưới ρ·r_max trong một quần thể 68 con, sổ EU vẫn khép kín, bốn tiến trình cho cùng checksum. Tỉ số standing crop của S6 có sàn ≈ ρ khi có dự trữ, nên đề xuất đọc phần cỏ ăn được thay vào đó
status: completed
owner: simulation-architecture
last_reviewed: 2026-11-18
---

# Thẻ S10 — khoá `live.plant_reserve`

> **Thẻ thêm một khoá, không phải một lượt đo.** Mọi con số dưới đây là đầu ra của test hoặc của một probe so
> hai commit, chạy trên macOS arm64 (Apple M5). Không giá trị khối lượng cơ thể nào được đọc.

## Vì sao

[Đề xuất làm lại SEL2](2026-11-18-sel2-redesign-proposal.md) §4: đồng cỏ mọc lại logistic và không có phần nào
con vật ăn không tới, nên một quần thể gặm đủ mạnh để tạo khan hiếm sẽ trượt xuống tận đáy thay vì dừng ở mức
khan hiếm vừa phải. Một phần dự trữ ăn không tới là cách giữ sổ EU khép kín mà vẫn chặn được cú trượt đó.

## Cái đã thay đổi trong mã

- `resources::PlantReservePolicy { fraction }`, với `floor(r_max)` = `ρ·r_max` hoặc `0` khi tắt. Vắng tài nguyên
  này thì coi như `ρ = 0`; app desktop không bao giờ chèn nó.
- `herbivore_grazing_system`: khi `ρ > 0`, con vật được mời `max(r − ρ·r_max, 0)` thay vì `r`. Sau khi gặm, ô bị
  ghim ở đúng `ρ·r_max` nếu phép trừ `f32` rơi thấp hơn một ULP. Lượng `removed` đo **sau** bước ghim, nên sổ cái
  vẫn chính xác. Bước ghim là phòng thủ: bỏ nó đi thì test sàn vẫn xanh trong lượt đo này. Còn bỏ quy tắc "chỉ mời
  phần trên sàn" (đưa lại `r`) thì test đỏ ngay tick 5 với 1 ô dưới sàn, và xanh lại khi khôi phục.
- `DormantCohorts::graze_all_with_reserve` (quần thể gộp): tổng `standing` chỉ tính phần trên sàn, mỗi ô mất một
  phần của phần trên sàn và không xuống dưới sàn. Lượng mất của từng ô đo bằng `f64`. `graze_all(field, dt)`
  giữ nguyên chữ ký và gọi bản mới với `ρ = 0`. `dormant_cohort_ecology_system` đọc cùng tài nguyên.
- `live_experiment.rs`: hằng `LIVE_KEY_PLANT_RESERVE`, thêm vào `LIVE_KEYS` (22 khoá), trường
  `LiveWorldConfig::plant_reserve` (`#[serde(default)]`, nên cấu hình cũ đọc lại là `0`), kiểm khoảng `[0, 1)`,
  và chèn tài nguyên trước nhánh restore nên một lượt chạy tiếp từ checkpoint giữ nguyên phần dự trữ.
- **Với `ρ = 0`, cả hai đường gặm cỏ chạy đúng phép tính cũ.** Mã rẽ nhánh theo `fraction > 0.0`, không dựa vào
  việc `r − 0·r_max` bằng `r`.
- Test: `src-tauri/tests/plant_reserve_tests.rs`, 11 test.

## Năm điều kiện của thẻ

| # | Điều kiện | Bằng chứng | Kết quả |
|---|---|---|---|
| 1 | Khoá vắng hoặc `0` thì quỹ đạo trùng lượt chạy trước khi có khoá | Probe so hai commit (dưới đây) và `absent_zero_and_no_policy_resource_are_bit_identical` | Trùng tới từng bit |
| 2 | Với `ρ > 0`, không ô nào bị gặm xuống dưới `ρ·r_max` trong một lượt có quần thể đông | `no_cell_is_grazed_below_the_reserve_in_a_dense_population`, quét mọi ô ở mọi tick | 0 ô dưới sàn trong 1.500 tick, đỉnh 68 con; đối chứng không dự trữ có tới 2.037 ô dưới cùng mức |
| 3 | Sổ EU vẫn trong ngưỡng của các test sổ cái hiện có | `closed_eu_stays_closed_with_a_reserve` (ngưỡng `1e-6` tương đối của `live_turnover_tests`, ngưỡng `1e-3` EU của `plants_tracks_the_field_it_describes`) và toàn bộ `cargo test --features desktop` | Lệch 0 và 0 |
| 4 | Cùng manifest cho cùng checksum ở nhiều tiến trình riêng | `the_reserve_checksum_agrees_across_four_separate_processes` | 4/4 tiến trình cho `0x7562a930` |
| 5 | Tỉ số crop có đổi nghĩa khi có dự trữ không | Mục dưới | **Có** |

Đầu ra của test (`cargo test --features desktop --test plant_reserve_tests -- --nocapture --test-threads=1`):

```text
S10_OFF absent=0x4ba7bde4 zero=0x4ba7bde4 bare=0x4ba7bde4
S10_EU rho=0.3 worst_relative_drift=0e0 worst_plants_vs_field_gap=0e0 control_drift=3.3162181394246865e-15 control_gap=1.6298145055770874e-8
S10_FLOOR rho=0.3 ticks=1500 peak_agents=68 births=50 min_headroom_with_reserve=0e0 control_min_headroom=-3.0000001192092896e-1 control_cells_below_max=2037
S10_AGGREGATE grazed=13.99999988079071 control_grazed=20 min_headroom=0e0 cells_below=0 control_cells_below=4 control_worst_field_vs_reported_gap=0e0
S10_CHECKSUMS 0x7562a930 0x7562a930 0x7562a930 0x7562a930
```

`min_headroom_with_reserve=0e0` nghĩa là quần thể **đã** gặm ít nhất một ô xuống tới đúng sàn, nên điều kiện 2
không đúng một cách hiển nhiên. `control_min_headroom=-0,3` nghĩa là không có dự trữ thì cùng seed gặm một ô tới 0.
Checksum tuyệt đối chỉ ghi để tham khảo: checksum phụ thuộc nền tảng, nên không test nào ghim chúng.

### Điều kiện 1: so với commit trước khi có khoá

Một probe tạm (không commit) dựng thế giới nền của S6 với `live.prey_cap` 160 cho seed 1–3, thế giới mặc định
seed 1, mỗi lượt 1.500 tick. Probe băm 12 observable cũ ở mọi tick và băm toàn bộ đồng cỏ cuối lượt. Nó cũng cho một
đàn gộp gặm 5.000 tick và chạy ba lần khôi phục checkpoint. Probe chạy trên thân cây `46e25d5` (trước S10) và trên
nhánh S10, cùng `--features desktop`, cùng thư mục target:

```text
S10_PROBE restore dense seed=3 k=1 total=2 uninterrupted=0xf5876d6e resumed=0x109047fe match=false
S10_PROBE restore dense seed=3 k=300 total=600 uninterrupted=0xe6d29cc5 resumed=0xe6d29cc5 match=true
S10_PROBE restore dense seed=3 k=600 total=1200 uninterrupted=0xe20bddee resumed=0x38bbb839 match=false
S10_PROBE dense seed=1 ticks=1500 checksum=0xf69ce1eb observables_fnv=0x2b115bb742bc3979 field_fnv=0xf9745896fe181e49 agents=54 standing_crop=149334.54249412596
S10_PROBE dense seed=2 ticks=1500 checksum=0xf722f714 observables_fnv=0x752a8be39ffa252c field_fnv=0x706c9b0ba2c1581a agents=110 standing_crop=150381.33039519476
S10_PROBE dense seed=3 ticks=1500 checksum=0x4ba7bde4 observables_fnv=0xcff5b44cb356319f field_fnv=0x00ba84e87975de33 agents=67 standing_crop=148063.16160081406
S10_PROBE default seed=1 ticks=1500 checksum=0x5e63e79d observables_fnv=0x5103b5dfa4c1132f field_fnv=0x8af2f2ce1fa26311 agents=10 standing_crop=154951.80242026754
S10_PROBE aggregate graze_all fnv=0xe7b4d3426bc6091a energy=339
```

`diff` giữa hai commit: không có dòng nào khác (8/8 dòng trùng). Hai lần khôi phục không khớp ở trên có ở **cả**
hai commit, xem "Điều bất ngờ".

## Điều kiện 5: tỉ số standing crop đổi nghĩa

`live.standing_crop` vẫn là tổng `r` của cả đồng cỏ, **gồm cả phần dự trữ**. Nghĩa của observable không đổi, nhưng
tỉ số S6 dựng trên nó thì đổi. Tỉ số S6 = trung bình standing crop nửa sau chia cho nền cùng seed (trần 80, hệ số
1,0). Có dự trữ thì standing crop không bao giờ dưới `ρ·Σr_max`, và nền không dự trữ thì không lớn hơn `Σr_max`. Vì
thế tỉ số có **sàn khoảng ρ**: ở `ρ = 0,4`, một quần thể gặm mọi ô xuống tận sàn vẫn cho tỉ số ≥ 0,4 và rơi đúng
vào mép dưới của dải 0,4–0,6. Dải đó không còn phân biệt "khan hiếm vừa phải" với "gặm trụi tới sàn".

**Đề xuất cách đọc cho SEL2-E** (chủ dự án quyết):

- Đọc **phần cỏ ăn được còn lại**: `g = (C̄ − ρ·Σr_max) / (C̄_nền − ρ·Σr_max)`. Ở đây `C̄` là trung bình
  `live.standing_crop` nửa sau, `C̄_nền` là của nền cùng seed. `g = 0` là gặm tới sàn; ở `ρ = 0` thì `g` đúng bằng
  tỉ số S6.
- `Σr_max` không cần observable mới. Mọi ô khởi đầu đầy (`ResourceField::from_biomes`), và `r_max` chỉ đổi ở
  genesis (`live.resource_capacity_scale` nhân cả `r` lẫn `r_max`). Vì vậy `Σr_max` bằng `live.standing_crop` đọc
  từ adapter vừa dựng, **trước tick 1**.
- Dải mục tiêu của SEL2-E phải khai trên `g`, hoặc trên tỉ số thô kèm `ρ` của từng ô, không dùng chung một dải
  thô cho mọi `ρ`.
- Chỉ số thảm thực vật của telemetry (`r / r_max`) cũng có sàn `ρ` theo cùng cách. Nó không dùng cho SEL2, ghi lại
  để người đọc dashboard biết.

## Điều bất ngờ

1. **Khôi phục checkpoint trong thế giới nền của S6 không đi theo lượt chạy liền mạch, trước cả S10.** Probe
   trên thân cây `46e25d5` (không có khoá) cho checksum khác nhau khi khôi phục ở tick 1 (chạy tới 2) và ở tick
   600 (chạy tới 1.200), seed 3. Khôi phục ở 10, 100 và 300 thì khớp. Bỏ từng khoá để thu hẹp, ở 600/1.200:
   - bỏ `live.locomotion` (cùng `drive_floor` và lái tới nước), hoặc bỏ `live.reproduction`, hoặc bỏ
     `live.mortality`: khớp;
   - bỏ `live.prey_cap`, `live.thirst_steering`, hoặc `drive_floor` cùng lái tới nước: vẫn lệch.

   Khôi phục ở tick 1 vẫn lệch khi đã bỏ vận động hoặc bỏ tử vong, và khớp khi bỏ sinh sản. F-040 đã sửa trường hợp
   tử vong + sinh sản **không** có vận động. Đây trông như một trường hợp anh em của nó, cần vận động, và một
   trường hợp riêng ở tick 1 gắn với sinh sản. Cơ chế chưa được chứng minh. S10 không sửa (ngoài phạm vi uỷ quyền).
   Test lưu/nạp của S10 chạy trên thế giới nền **bỏ** `live.reproduction`, nơi khôi phục đã khớp, và kiểm rằng
   phần dự trữ thật sự đổi thế giới đó. Đề xuất một mục FINDINGS mới (dưới đây).
2. **Trong lượt có dự trữ, sổ cái khớp tuyệt đối (lệch 0), còn đối chứng lệch `1,6e-8` EU.** Giải thích có lý,
   chưa chứng minh: không có dự trữ thì có ô bị gặm gần 0, nơi ULP của `f32` rất nhỏ, nên tổng `f64` làm tròn. Có
   dự trữ thì mọi ô đã gặm dừng ở sàn, giá trị thô hơn, và tổng chính xác. Cả hai đều dưới ngưỡng `1e-3`.
3. **Bản nháp đầu của nhánh gộp làm lệch sổ ở mức ULP** (`13,99999976` so với `14`). Phép trừ `before − after`
   bằng `f32` không chính xác khi một ô mất hơn nửa giá trị. Nhánh có dự trữ nay đo bằng `f64`. Nhánh không dự
   trữ giữ phép tính cũ cho đúng bit. Về lý thuyết nó có cùng điểm yếu khi một đàn gộp ăn hơn nửa cỏ của chunk
   trong một tick, nhưng test của S10 không thấy (`control_worst_field_vs_reported_gap=0e0`). Lớp gộp không chạy
   trong thế giới sống của các thí nghiệm.

## Đề xuất dòng cho các tài liệu của chủ dự án (S10 không sửa chúng)

- **STATE §3.00 / AGENT_WORKFLOW §3**: `S10 ✅ #<PR> (2026-11-18): live.plant_reserve; khoá vắng trùng quỹ đạo
  commit trước S10; tỉ số crop có sàn ≈ ρ, đọc g theo ghi chú S10`.
- **FINDINGS.md**, mục mới: "Restore một thế giới live có vận động, sinh sản và tử vong không đi theo lượt chạy
  liền mạch (seed 3, khôi phục ở 600 trong 1.200 tick, thế giới nền S6); khôi phục ở tick 1 lệch khi có sinh
  sản". Tái hiện bằng probe ở trên, không cần khoá S10.

## Không khẳng định

- Không nói phần dự trữ giữ được quần thể sống hay tạo ra khan hiếm vừa phải. Đó là câu hỏi của SEL2-E.
- Điều kiện 2 được kiểm ở một seed (3), `ρ = 0,3`, 1.500 tick. Mã chặn sàn thì không phụ thuộc seed: ghim tại
  chỗ sau mỗi lần gặm, ở cả hai đường.
- Điều kiện 4 được kiểm trên một máy. Tính tất định giữa các nền tảng là việc của M1.
