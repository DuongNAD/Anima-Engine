---
title: ADR-0007 — Duy trì quần thể trong thế giới live (QĐ-ECO)
status: accepted
owner: simulation-architecture
last_reviewed: 2026-11-17
decision_date: 2026-11-16
supersedes: none
superseded_by: none
---

# ADR-0007 — Duy trì quần thể trong thế giới live (QĐ-ECO)

> **Trạng thái: đề xuất, chờ chủ dự án.** Chưa có dòng `QĐ-ECO` nào được điền. **QĐ-ECO là quyết
> định mới**, chưa có trong `AGENT_WORKFLOW.md` §4. Số liệu ở "Bằng chứng" **chưa đăng ký trước,
> mang tính thăm dò, đo 2026-11-16 bằng prototype scratch (không nằm trong cây mã) trên `55ce76c`**,
> adapter live, `mortality=1`, `reproduction=1`, `locomotion=1`, 6.000 tick, seed 1-5, 10 founder =
> 3 thú săn + 7 con mồi; không phải kết quả thí nghiệm.

## Bối cảnh

Sau [ADR-0005](ADR-0005-live-population-turnover.md) (tử vong + sinh sản) và
[ADR-0006](ADR-0006-locomotion-actuation.md) (`live.locomotion`, PR #76), sinh vật đã di chuyển được
nhưng **quần thể vẫn không bền**. Điều này chặn E-SEL1 (thiết kế chỉ con mồi) và S43 (Red-Queen cần cả
thú săn lẫn con mồi). Mỏ neo đã kiểm trên `55ce76c`:

- Giết con mồi: `combat_system` (`world_systems.rs` 309) có hai nhánh đòn đánh với cùng công thức,
  nhánh telemetry (khoảng 419-431) và nhánh còn lại (khoảng 492-500). Cả hai gọi
  `ecology::predation_capture` rồi nhân `LINDEMAN_EFFICIENCY` (0.40, `ecology.rs` 155). Thú săn chọn
  **con mồi đầu tiên trong thứ tự truy vấn** trong bán kính 3.0 (`world_systems.rs` 412 ở nhánh đầu).
- `predation_capture_with_refuge` (`ecology.rs` 192-199): Holling III có nơi trú 0.20
  (`DEFAULT_RARITY_REFUGE`, 178), `a = h = 0.02` (`CAPTURE_ATTACK`/`CAPTURE_HANDLING`), chặn bởi
  `deficit / 0.40`.
- Nước: `metabolic_decay_system` (`world_systems.rs` 80) trừ `(0.05 + 0.15·total_cost + sweat)·dt`
  (dòng 154-161); uống chỉ khi tâm trong bán kính hồ (`environmental_systems.rs` 306-318).
  **Hydration không nằm trong sổ EU**, nên đổi cân bằng nước không thể phá EU kín (`closed_eu_total`
  chỉ nhiễu f64 trong mọi run).
- Tốc độ di truyền của #76: `class_max × clamp(biên độ CPG TB / 1.5)` (`locomotion.rs` 21, 49-63).
  Bộ lái chỉ nhắm thức ăn/con mồi, **không bao giờ nhắm nước**.

## Bằng chứng (thăm dò, chưa đăng ký)

Prototype scratch, chỉ đo, không commit; cấu hình ở đầu tài liệu.

- **Nền**: thú săn tuyệt chủng 5/5 seed (t ≈ 1.600-4.925), không bao giờ sinh sản; con mồi cũng tuyệt
  chủng ở vài seed. **Chỉ con mồi** (`predator_fraction = 0`) **cũng không bền**: tuyệt chủng trước
  6.000 tick ở 2/5 seed (khát).
- **Founder seed 2, 4, 5 gần như bất động** khi bật vận động: biên độ CPG ≈ 0.05-0.1 (seed 1 ≈ 1.3),
  nên quy tắc tốc độ di truyền cho ≈ 0.1 thay vì ≈ 1.7. Ví dụ seed 2: founder đứng ở nơi sinh
  (z ≈ -30, hồ ở z = 60) và chết khát t ≈ 4.400-5.400.
- **Từng núm riêng lẻ** (hydration từ giết k = 1 hoặc 3; hao nước thú săn ×0.5 hoặc ×0; nơi trú
  Holling 0 hoặc type II; hiệu suất giết 0.40 → 0.60; chỉ nhắm con mồi có E ≥ 10; lái tới hồ gần nhất
  khi hydration < 50%): **không núm nào cho cùng tồn tại ở cả 5 seed**; vài núm trì hoãn tuyệt chủng
  hoặc nghiền con mồi.
- **Tổ hợp**, với sàn lực lái `df = max(drive CPG, 0.5)`:
  - `df` + lái tới nước: con mồi phát triển ở 4 seed, thú săn vẫn tuyệt chủng.
  - `df` + nước + hydration từ giết (k = 1): thú săn còn sống ở t = 6.000 ở seed 2 và 4.
  - Tốt nhất: `df` + nước + hydration từ giết + hiệu suất 0.6 + nhắm con mồi E ≥ 10 →
    **cùng tồn tại ở 2/5 seed** (seed 2: P3/Q30; seed 4: P2/Q82; dao động nhẹ, Pmax 6-7, Pmin 2-3).
    Seed 1, 3, 5 vẫn hỏng (seed 5: thu nhập năng lượng của thú săn; seed 3: thú săn giết quá tay một
    nền con mồi nhỏ rồi con mồi sụp kiểu Allee).
- **Tất định chỉ kiểm cho nền và chỉ-con-mồi** (hai lần chạy trong tiến trình khớp); **chưa** kiểm cho
  các tổ hợp.

## Quyết định cần đưa ra

**QĐ-ECO** (mới) — "Thay đổi sinh thái tối thiểu nào để thế giới live duy trì quần thể (trước hết con
mồi một mình, sau đó cả hai) khi `live.locomotion` bật?" Mọi phương án **opt-in qua khoá manifest**
`live.*`, mặc định không đổi (khoá vắng = đường hiện tại). Tên khoá dưới đây **đề xuất**, chưa tồn tại.

### Phương án A — Chỉ sửa vận động: sàn lực lái + lái tới nước

- *Khoá*: `live.drive_floor` (sàn trên drive CPG, đo với 0.5) và `live.thirst_steering` (khi
  hydration < ngưỡng, mục tiêu là hồ gần nhất; đo với 50%).
- *Mã*: chỉ trong `core/locomotion.rs` (`drive_from_cpg`, chọn mục tiêu). Không chạm sổ EU, không RNG.
- *Bằng chứng*: tổ hợp đầu cho con mồi phát triển ở 4/5 seed; là điều kiện cần của mọi phương án sau.
- *Rủi ro*: sàn lái làm tốc độ bớt "di truyền" (chọn lọc tác động lên phần trên sàn); cần ghi rõ
  trong tài liệu để không diễn giải nhầm là tiến hoá vận động. Hai nửa nên đo riêng khi vào thẻ.

### Phương án B — A + nước từ giết + hiệu suất giết + sàn năng lượng con mồi

- *Khoá*: thêm `live.kill_hydration` (k), `live.kill_efficiency` (thay 0.40), `live.target_min_energy`
  (con mồi E ≥ 10).
- *Mã*: `combat_system` (cả **hai** nhánh đòn đánh, phải cùng sửa), `locomotion.rs`. Hydration ngoài
  sổ EU nên an toàn với EU kín; **hiệu suất giết đổi phần EU quay về detritus** nên phải giữ nhánh
  "đọc lại phần thực sự đổi" của f32 và chạy lại cổng EU kín.
- *Bằng chứng*: cùng tồn tại 2/5 seed; 3/5 vẫn hỏng. Là **một phần**, chưa đủ cho S43.
- *Rủi ro*: nâng 0.40 → 0.60 trái hằng số sinh thái (Lindeman) trong `sim_rules`; chỉ nên là tham số
  khai báo của kịch bản, không đổi hằng toàn cục.

### Phương án C — B + nghiên cứu thu nhập năng lượng của thú săn

Chọn con mồi giàu nhất thay vì "đầu tiên trong truy vấn", săn theo nhóm, chuyển hoá thú săn. Chưa có
phát hiện nào; đây là **nghiên cứu** với phép đo đăng ký trước riêng, không phải thay đổi mã được
duyệt. Lỗi còn lại: seed 5 (thu nhập năng lượng), seed 3 (săn quá tay + sụp Allee).

### Phương án D — Giữ nguyên

Con mồi một mình có nguy cơ tuyệt chủng (2/5 seed) nên E-SEL1 chỉ-con-mồi bị rủi ro; thú săn không
sống nên S43 bị chặn. Không khuyến nghị.

## Quyết định đề xuất

**QĐ-ECO = A ngay; B là opt-in cho việc S43 thăm dò; C là nghiên cứu có phép đo riêng.** Lý do: A là
thay đổi ngữ nghĩa nhỏ nhất, nằm trọn trong một mô-đun, không chạm EU, và là điều kiện cần (không có
nó, 3/5 seed khởi đầu bằng founder bất động). Nó mở khoá thiết kế chỉ-con-mồi của E-SEL1. B tăng
ngữ nghĩa (đổi luật giết) mà bằng chứng mới cho 2/5 nên chỉ nên dùng thăm dò. **Không gì thành mặc
định nếu chủ dự án chưa quyết.**

**ADR này KHÔNG quyết:**

- Giá trị số (0.5, 50%, k, 0.6, E ≥ 10) chỉ là điểm đo, không phải hằng đề xuất; tên khoá chưa chốt.
- Có hay không đưa bất kỳ phương án nào thành mặc định.
- Cách giải quyết thu nhập năng lượng của thú săn (C), S43 và các khẳng định thích nghi/loài
  (cổng AE-S11/AE-S14).
- Tất định của B/C: chưa đo; thẻ cài đặt phải có test nhiều tiến trình trước khi tin.

**`LIVE_MODEL_VERSION`** (`live_experiment.rs`, `"live-bevy-world/1"`): **không** cần bump khi các
khoá vắng (checksum bằng thế giới không khoá, như ADR-0006). Chỉ cần bump nếu một phương án trở thành
**mặc định**, vì khi đó checksum mọi thế giới live đổi.

## Hệ quả

- **Tích cực**: có lộ trình tối thiểu, đo được, tách bạch từng tầng ngữ nghĩa; E-SEL1 được mở khoá.
- **Tiêu cực**: thêm khoá manifest; B sửa hai nhánh đòn đánh phải đồng bộ; bằng chứng chưa đăng ký.
- **Thẻ tiếp theo**
  1. Thẻ A: `live.drive_floor` + `live.thirst_steering` + test (founder bất động nay tới hồ, con mồi
     sống 6.000 tick ở đủ 5 seed theo ngưỡng chọn trước, tất định nhiều tiến trình, EU kín, khoá vắng
     thì checksum không đổi).
  2. Thẻ B (opt-in) sau A.
  3. Thẻ nghiên cứu C, đăng ký trước.

## Dòng quyết định để dán

Thay `<ngày>`. Phương án A (khuyến nghị):

```text
QĐ-ECO (owner, <ngày>) = A: opt-in live keys for a steering drive floor and thirst steering so
            founders can move and reach water; default world unchanged; ADR-0007
```

Phương án B:

```text
QĐ-ECO (owner, <ngày>) = B: A plus opt-in predator kill hydration, kill efficiency and a prey-energy
            floor in predator targeting, for exploratory S43 work only; default unchanged; ADR-0007
```

Phương án C:

```text
QĐ-ECO (owner, <ngày>) = C: B plus a pre-registered research card on predator energy income (richest
            prey selection, group hunting, predator metabolism); nothing default; ADR-0007
```

Phương án D:

```text
QĐ-ECO (owner, <ngày>) = D: keep as is; live populations are not sustained; E-SEL1 prey-only at
            risk and S43 blocked; ADR-0007
```

Sau khi chủ dự án điền: đổi `status` thành `accepted`, điền `decision_date`, cập nhật chỉ mục.

## Quyết định (2026-11-16)

**QĐ-ECO = A**: hai khoá opt-in cho sàn lực lái và lái tới nước khi khát; thế giới mặc định không đổi. B và C
không được chọn ở đây: B chỉ được dùng làm thăm dò cho S43 sau một quyết định riêng, C là thẻ nghiên cứu có
đăng ký trước. Quyết định do phiên Claude lead đưa ra theo uỷ quyền của chủ dự án trong chat ngày 2026-11-16
("bạn được phép tự làm theo bản kế hoạch và xử lý sao cho tốt nhất"). Chủ dự án có thể đảo lại: đổi `status`
và dòng QD-ECO trong khối `DECISIONS` của [`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md#4-khối-decisions-để-dán-vào-prompt-0).


## Thẻ B (2026-11-17)

Thẻ B (phương án B) đã được cài đặt như **ba khoá opt-in**, mặc định không đổi (khoá vắng = đường hiện tại,
checksum bằng thế giới không khoá). **B vẫn là opt-in, không thành mặc định**, `LIVE_MODEL_VERSION` không bump,
và QĐ-ECO vẫn là A: B chỉ dành cho việc thăm dò S43.

| Khoá | Giá trị | Mặc định | Ngữ nghĩa |
| --- | --- | --- | --- |
| `live.kill_hydration` | k >= 0 | 0 (tắt) | Đòn đánh chuyển ΔE EU (sau hiệu suất, phần **thực sự** đổi, `gained`) cho thú săn thì cộng thêm `k × ΔE` vào hydration của nó, chặn ở `hydration_target`. Hydration nằm ngoài sổ EU. |
| `live.kill_efficiency` | e trong (0, 1] | 0.40 (`LINDEMAN_EFFICIENCY`) | Thay hằng Lindeman trong **cả hai** nhánh đòn đánh của `combat_system`, chỉ cho thế giới này; hằng toàn cục không đổi. Phần `(1 − e)` vẫn về detritus qua `removed − gained` đo lại. |
| `live.target_min_energy` | E >= 0 | 0 (tắt) | Bộ lái của thú săn chỉ nhắm con mồi còn sống có `energy >= E`; không con nào đủ thì như hiện nay khi không có mục tiêu (đứng yên). Chỉ có tác dụng khi `live.locomotion = 1`. |

**Chọn ngữ nghĩa**: `target_min_energy` chỉ lọc **lái** (`locomotion.rs`, `steering_drive_system`), **không** lọc
`combat_system`: thú săn vẫn đánh con mồi đầu tiên trong thứ tự truy vấn trong bán kính 3.0, đúng định nghĩa
của thăm dò ("chỉ nhắm con mồi có E ≥ 10"). Chính sách nằm trong resource `PredatorIncomePolicy`
(`resources.rs`), chèn ở `install_runtime_resources` nên cả genesis lẫn restore đều nhận (bài học F-043).
`predation_capture` giữ nguyên trần `deficit / 0.40`, nên ở e > 0.40 phần vượt trần dinh dưỡng của thú săn
chảy về detritus chứ không mất. `LIVE_KEYS` từ 16 lên 19.

**Đo (thăm dò, không đăng ký, không phải assertion)**: seed 1-5, 6.000 tick, 10 founder, `predator_fraction` 0.3,
8 cây, 2 hồ, mortality/reproduction/locomotion 1, `drive_floor` 0.5, `thirst_steering` 1; B tại điểm tốt nhất
(k = 1, e = 0.6, E = 10), so với cùng cấu hình không có B:

| seed | thú săn / con mồi còn sống ở t = 6.000 (có B) | min-max thú săn | min-max con mồi | không B (thú săn / con mồi) |
| --- | --- | --- | --- | --- |
| 1 | 0 / 83 | 0-4 | 10-86 | 0 / 44 |
| 2 | 4 / 40 | 3-5 | 14-40 | 0 / 43 |
| 3 | 0 / 0 | 0-3 | 0-26 | 0 / 0 (giống hệt) |
| 4 | 2 / 82 | 1-6 | 14-83 | 0 / 75 |
| 5 | 0 / 70 | 0-6 | 14-73 | 0 / 70 (giống hệt) |

Cùng tồn tại ở **2/5 seed** (2 và 4), khớp bằng chứng thăm dò ở trên; seed 3 cả hai loài tuyệt chủng, seed 1 và 5
thú săn tuyệt chủng. Đây vẫn là **một phần**, chưa đủ cho S43. Kiểm chứng: tất định 3 tiến trình khớp
với cả ba khoá, EU kín (lệch tương đối ~4e-15 trên 3.000 tick), restore giữ chính sách và quỹ đạo,
khoá vắng = khai báo mặc định tường minh; từng cơ chế bị tắt thì test tương ứng đỏ. Test: `tests/eco_card_b_tests.rs`.

## Quyết định thẻ B (2026-11-17)

**QĐ-ECO-B = opt-in.** Ba khoá của thẻ B (#87) được merge để thăm dò S43, **không** thành mặc định. Đây là
"quyết định riêng" mà phần Quyết định (2026-11-16) nói tới. Phiên lead đưa ra theo uỷ quyền của chủ dự án trong
chat ngày 2026-11-17 ("làm theo bản kế hoạch"), đúng khuyến nghị của ADR này. Chủ dự án có thể đảo lại: đổi dòng
`QD-ECO-B` trong khối `DECISIONS` của [`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md#4-khối-decisions-để-dán-vào-prompt-0).

## Khoá P1 và bằng chứng cho phương án C (2026-11-17, thăm dò, chưa đăng ký)

**Khoá P1.** `tests/predator_prey_coexistence_tests.rs` dùng cấu hình thẻ A + B (k = 1, e = 0.6, E = 10),
10 founder, `predator_fraction` 0.3, 8 cây, 2 hồ, tử vong, sinh sản, vận động, chạy seed 1–10 tới t = 6.000. Test
đòi ≥ 3 seed có thú săn và con mồi cùng sống.

| Cấu hình | Seed cùng tồn tại (macOS arm64, bản debug) |
| --- | --- |
| A + B | **5/10**: seed 2, 4, 6, 8, 10 |
| Chỉ A (bỏ ba khoá B) | **1/10**: chỉ seed 10, nên test đỏ |

Ngưỡng là một số đếm có biên, không phải danh sách seed, vì quỹ đạo hỗn loạn và checksum khác nhau giữa các
nền tảng. Cùng với EU kín, tất định nhiều tiến trình và restore (`eco_card_b_tests`), test này đáp ứng định nghĩa
hoàn thành của P1 trong STATE. Nó **không** khẳng định S43, Red Queen hay sự ổn định sau t = 6.000.

**Vì sao các seed còn lại hỏng.** Số liệu từ bộ đếm tạm, không commit, seed 1–10, bản release, EU lệch
≤ 2,1e-14; không đọc khối lượng cơ thể.

- **Seed 1 và 5: thú săn founder chết đói với 0 đòn.** Founder đầy năng lượng nên **sinh sản ngay khi cổng
  "con mồi ≥ 5 × thú săn" mở** (khoảng tick 170, khi có 15 con mồi), trước lần săn đầu tiên. Năng lượng tụt từ 97
  xuống khoảng 34 rồi chia cho con.
  - Tốc độ thú săn là 2,0 × sàn 0,5 ≈ 1 đơn vị/s. Trong khoảng 1.500 tick chúng chỉ đi được khoảng 23 đơn vị.
  - Con mồi gần nhất thì cách 18–54 đơn vị. Ở seed 5 không con thú săn nào vào được bán kính 3,0.
  - Kết quả là cả nhóm chết đói ở khoảng tick 1.590.
- **Seed 3 và 9: con mồi bị nước giới hạn, không phải thú săn.** Thế giới chỉ con mồi cũng chỉ còn 12 và 20 con.
  Chết khát chiếm phần lớn ca chết của con mồi ở mọi seed.
- **Seed 7:** thú săn có thu nhập nhưng chết lẫn đói lẫn khát.
- **Thu nhập mỗi đòn rất nhỏ** (0,08–0,2 EU), vì trần thâm hụt nhỏ khi thú săn gần no: đòn chủ yếu là "nạp thêm".
- **Cổng chặn sinh sản là tỉ lệ con mồi:** khoảng 99 % số tick thú săn đủ năng lượng bị cổng 5× chặn. Cổng
  `pred_count >= 8` không chặn lần nào.

**Độ nhạy.** Mỗi thay đổi là một hằng tạm đặt riêng trên nền A + B; số trong bảng là số seed cùng tồn tại trên 10.

| Thay đổi | x/10 |
| --- | --- |
| nền A + B | 5 |
| chi phí trao đổi chất của thú săn × 0,5 (cũng như × 0,35 / 0,25 / 0,7) | 7 |
| Holling loại II | 6 |
| bỏ trần bắt `deficit / 0.40` | 5 (không ràng buộc ở e = 0,6) |
| bán kính đòn 5,0 | 5 |
| nơi trú 0 | 4 |
| ngưỡng sinh sản 0,75 | 4 |
| E = 0 | 4 |
| cổng tỉ lệ 3× | 3 |
| bỏ cổng tỉ lệ | 3 |
| tốc độ thú săn × 1,5 | 2 |
| tốc độ thú săn × 2 | 1 (nhanh hơn thì ăn quá tay) |
| chỉ sinh sau khi đã ăn (đòn đầu tiên, hoặc ≥ 20 / 50 EU từ con mồi) | 3–6; kèm chi phí × 0,5: 7 |
| **chi phí × 0,5 + bán kính 5,0** | **8** |
| **chi phí × 0,5 + cổng 3×** | **8** |

Không thay đổi đơn hay cặp nào đạt 9–10/10. Seed 3 và 9 không cùng tồn tại ở bất kỳ lượt nào, vì con mồi bị nước
giới hạn. Lưu ý: chi phí × 0,5 cũng làm chậm mất nước của thú săn, vì hao nước tính từ `total_cost`.

**Hệ quả cho C.** Thẻ tiếp theo, nếu chủ dự án chọn, là **C1 (opt-in)**: hai khoá `live.predator_cost_mult` (nhân
vào phần chi phí của thú săn trong `total_cost` ở `metabolic_decay_system`, đúng quy tắc năng lượng của CLAUDE.md)
và `live.strike_radius`. Sau đó cần một phép đo trên ≥ 20 seed, đăng ký trước. Gốc chung của các seed còn hỏng là
**nước**, cùng kết luận với
[chẩn đoán sau E-SEL1](../ai/planning/2026-11-17-esel1-scarcity-diagnosis.md). Một khoá về nguồn nước (dung tích
hoặc bổ sung hồ) có lẽ phục vụ cả S43 lẫn thí nghiệm chọn lọc. Không gì ở đây thành mặc định.

