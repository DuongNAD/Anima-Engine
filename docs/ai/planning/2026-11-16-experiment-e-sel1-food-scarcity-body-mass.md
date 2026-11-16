---
phase: planning
feature: experiment-e-sel1-food-scarcity
title: Kế hoạch — E-SEL1, khan hiếm thức ăn có dịch chuyển khối lượng cơ thể di truyền của quần thể không
description: Tiền đăng ký (đã đóng băng 2026-11-17) thí nghiệm chọn lọc đầu tiên trên thế giới live — hai nhánh food_cap, sinh sản và chết đói bật, tiền điều kiện chặn, giao thức smoke, quy tắc quyết định
status: active
owner: simulation-architecture
last_reviewed: 2026-11-17
---

# Kế hoạch — E-SEL1: khan hiếm thức ăn và khối lượng cơ thể

> ✅ **Đã đóng băng 2026-11-17.** Tiền đăng ký này nay có đủ ba thứ theo tiền lệ E2: tài liệu này, fixture JSON
> máy đọc được cùng các manifest tại
> [`src-tauri/tests/fixtures/experiments_esel1/`](../../../src-tauri/tests/fixtures/experiments_esel1/)
> (`esel1-preregistration.json`, ba manifest thí nghiệm, hai manifest smoke), và test hình dạng
> `src-tauri/tests/prereg_esel1_manifest_tests.rs` khẳng định hai bên khớp. Được commit **trước mọi lần
> chạy smoke hay thí nghiệm**; **không có kết quả nào tồn tại** (`"contains_results": false`).
> Các quyết định cố định do phiên điều phối đưa ra theo uỷ quyền của chủ dự án (ghi trong JSON, khối
> `decisions_taken_by_lead_session`): cấu hình chung mọi nhánh gồm `live.mortality=1`, `live.reproduction=1`,
> `live.locomotion=1`, `live.drive_floor=0.5`, `live.thirst_steering=1` (ADR-0007 phương án A), `live.evolved_brains=0`,
> `predator_fraction=0`; nhánh thứ ba `drift` (= `scarce` + `live.mutation_rate=0`) **có**, vì P4 đã có (PR #81);
> seed `760001 + 1000·k`; seed smoke 999971. P1–P5 đã đáp ứng trên thân cây (PR #66, #76, #81, #82, F-039
> `b0a8c4d`), mỗi mục kèm bằng chứng trong JSON.
> Chỉ **smoke** được đặt `N0`, `F_hi`, `F_lo`, bậc thang thời lượng và chiều H1 (§6), theo quy tắc đã viết. Trong các manifest thí nghiệm
> chúng là **giá trị đệm âm** (`live.founders = -3`, `F_hi = -1`, `F_lo = -2`): bộ phân tích khoá `live.*` của adapter
> từ chối chúng nên không thể chạy nhầm trước khi được điền bằng một sửa đổi có ngày.
> Mọi chỗ ghi `[SMOKE]` dưới đây là chỗ trống có chủ đích đó.
> **Không con số nào ở đây là kết quả quan sát: chưa có lần chạy nào.**
>
> Tên **E-SEL1**, không dùng "E3": tên E3 đang mơ hồ (E3 cũ là
> [dự thảo gradient năng suất](2026-08-26-experiment-e3-productivity-gradient-body-mass.md), QĐ-EN
> còn mở). Soạn trên nhánh `feat/ecological-selection`, theo đề xuất nghiên cứu 2026-11-16.

## 1. Câu hỏi

Trong thế giới live, khi sinh sản bị giới hạn bởi năng lượng và chết đói có thật, **khan hiếm thức
ăn có làm dịch chuyển khối lượng cơ thể trung bình của quần thể** (`live.trait_mass_mean`) không?

Đây là câu hỏi **thực nghiệm về một quan hệ nhân quả trong mô hình**, không phải tuyên bố "thích
nghi". Xem §12 về những gì sẽ **không** được tuyên bố.

Vì sao bây giờ mới hỏi được: E2 cho H1 null vì quần thể đứng yên (không ai chết, không ai sinh;
[`RESULT.md`](../../../artifacts/experiments/e2-evolved-brain-default/RESULT.md) §3). Hai vật cản đó
được gỡ bởi ADR-0005 (Card S1, `live.reproduction`, PR #66) và ADR-0006 (`live.locomotion`).

## 2. Mô hình

- Mô hình: `core::live_experiment::LiveExperimentAdapter`, lịch tick từ
  `core::simulation_schedule::build_tick_schedule` (một chuỗi toàn phần sau F-041).
- Nhãn trạng thái giữ nguyên như E2: **"headless adapter verified"**. Không có app, GPU, luồng học
  hay luồng tiến hoá trong tiến trình chạy. Không được trích thành "thế giới live đã sẵn sàng làm
  thí nghiệm" hay "đã chạy dưới executor đa luồng của app".
- Tiến hoá ở đây là **sinh sản liên tục theo năng lượng** (`autonomous_reproduction_system`: con =
  đột biến của kiểu gen cha, id dòng dõi `cha.thế_hệ`), **không** phải `EvolutionaryReplacement`
  (adapter không chạy luồng tiến hoá; ADR-0005). Não con từ genome, không Lamarck.
- Một địa hình duy nhất (seed bản đồ cố định của adapter): khái quát hoá sang địa hình khác không
  được tuyên bố.

## 3. Các nhánh

Hai nhánh bắt buộc, giống hệt nhau **trừ** `live.food_cap` (khoá `LIVE_KEY_FOOD_CAP`, trần số mục
thức ăn được sinh; `FoodSpawnSettings::max_food_count`).

| Khoá `initial_conditions` | Nhánh ĐỐI CHỨNG `abundant` | Nhánh XỬ LÝ `scarce` |
|---|---|---|
| `live.food_cap` | `F_hi` `[SMOKE]` | `F_lo` `[SMOKE]` |
| `live.mortality` | 1.0 | 1.0 |
| `live.reproduction` | 1.0 | 1.0 |
| `live.locomotion` | 1.0 | 1.0 |
| `live.evolved_brains` | 0.0 | 0.0 |
| `live.predator_fraction` | 0.0 | 0.0 |
| `live.founders` | `N0` `[SMOKE]`, ≥ 30 | `N0` (cùng giá trị) |
| `live.starve_ticks` | mặc định (vắng) | mặc định (vắng) |

Nhân tố khai báo: **đúng một** hiệu `live.food_cap`; mọi khoá khác phải khớp (test hình dạng như
`prereg_e2_manifest_tests` phải kiểm điều này).

Nhánh thứ ba, **chỉ khi tiền điều kiện P4 có**: `scarce_mut0` = nhánh `scarce` với
`live.mutation_rate = 0.0`. Dùng làm đối chiếu trôi/phân loại, xem §5.3 và §11 về giới hạn của nó.
Nếu P4 không có, **không có nhánh thứ ba** và giới hạn được nêu trong báo cáo (§11), không lấp
bằng cách khác.

Chọn `F_hi`, `F_lo`, `N0`: §6. Con mồi-thuần (`predator_fraction=0`) là có chủ đích: thú săn hiện
tuyệt chủng sớm (F-042 và bằng chứng ADR-0005), nên giữ chúng chỉ thêm nhiễu và một nhánh bị
confound; đây là **giới hạn** được nêu, không phải điểm mạnh.

## 4. Quan sát và thống kê

- **Điểm cuối chính (H1)**: `live.trait_mass_mean`, lấy **trung bình trên 20% mẫu cuối** của mỗi lần
  chạy (các mẫu có tick > 0,8·T; ở T = 18 000 là các mẫu tại 14 400 < tick ≤ 18 000). Lý do dùng cửa
  sổ đuôi chứ không dùng một tick: giảm nhiễu dao động theo mùa và theo dao động bùng/sập.
- Thứ cấp, **mô tả**, không quyết định: `live.agent_count`, `live.standing_crop`, `live.food_items`,
  `live.births`, `live.deaths`, `live.mean_lifespan`, `live.trait_mass_var`, `live.trait_mass_p10/p90`.
- **Kiểm tính toàn vẹn** (hiệu lực `none`): `live.closed_eu_total` phải hằng trong từng lần chạy.
  Lệch là **lỗi**, ghi thành finding, không phải bằng chứng sinh học. Cũng kiểm `live.births >= 1`
  ở cả hai nhánh (F-039): nếu 0 mà quần thể đã tăng, lỗi đo.
- Chuỗi thời gian lấy mẫu mỗi **600 tick** (30 mẫu ở T = 18 000). Warm-up 3 000 tick (5 mẫu) bị loại
  khỏi mọi trung bình thời gian ngoài cửa sổ đuôi.
- **Thống kê**: `paired_mean_delta` = trung bình của (`scarce` − `abundant`) theo seed, trên các cặp
  hoàn chỉnh; CI95 (t, n−1 bậc tự do), `paired_dz`, trung vị hiệu theo seed, bảng hiệu theo seed
  (đúng các trường trong E2 `reporting`).

## 5. Giả thuyết và quy tắc quyết định

### 5.1 H1 (chính)

Khối lượng cơ thể trung bình cuối cửa sổ khác nhau giữa `scarce` và `abundant`.

**Chiều (dấu) của H1 CHƯA được cố định.** Cơ chế hợp lý đi cả hai hướng (thân nhỏ rẻ chi phí chuyển
hoá nên sống sót hơn khi khan hiếm; hoặc thân lớn đủ dự trữ qua đói). Chiều được **cố định một lần,
sau smoke, trước mọi seed thí nghiệm**, ghi vào fixture bằng một khối sửa đổi có ngày
(`direction_fixed_after_smoke`) và chỉ được dựa vào quan sát smoke ở seed 999971 cộng cơ chế đã
nêu. Nếu smoke không đủ để chọn: H1 giữ `two_sided`. Không được đổi chiều sau khi một seed thí
nghiệm đã chạy.

### 5.2 Ngưỡng vật chất (giống E2/E3)

`all_of`: (i) CI95 của `paired_mean_delta` không chứa 0; (ii) `|paired_dz| ≥ 0,8`; (iii)
`|paired_mean_delta| ≥ 0,05·|control_mean|`. **Null** = CI chứa 0 **hoặc** `|delta| < 5%` đối chứng.
Vùng giữa (phát hiện được nhưng dưới ngưỡng) tính là **không đủ khác**. Kết quả null được báo cáo
là null, theo cách của E2 `RESULT.md`; không đổi metric sau khi có kết quả.

### 5.3 Đối chiếu trôi (chỉ khi P4)

H2 (thứ cấp, mô tả): `scarce` so với `scarce_mut0`, cùng seed, cùng thống kê. Đọc thế nào cho trung
thực: `mutation_rate = 0` tắt sự **sinh mới** biến dị, nhưng quần thể vẫn có biến dị nền của founder
và vẫn trôi ngẫu nhiên theo cỡ mẫu nhỏ. Nếu `scarce` dịch chuyển còn `scarce_mut0` thì không, điều
đó gợi ý đột biến cung cấp nguyên liệu; nếu cả hai cùng dịch chuyển, nguyên nhân là **sắp xếp lại
biến dị founder và/hoặc trôi**, chưa tách được. Nó **không** chứng minh chọn lọc. Cả hai khả năng
đều được báo cáo.

### 5.4 Quy tắc quyết định

- **Có dịch chuyển (trong mô hình)**: H1 đạt ngưỡng vật chất, ≥ 10 cặp hoàn chỉnh, không finding
  toàn vẹn nào mở, `cargo test --features desktop` xanh. Kết luận chỉ được phát biểu là *"trong
  adapter live, ở một địa hình, với con mồi thuần, khan hiếm đi kèm dịch chuyển khối lượng theo
  chiều X"*.
- **Null**: H1 null hoặc dưới ngưỡng, ≥ 10 cặp. Báo cáo là null; không nâng bằng "xu hướng".
- **Không đủ bằng chứng**: < 10 cặp hoàn chỉnh (kể cả vì tuyệt chủng, §8), hoặc một tiền điều kiện
  được đáp ứng theo cách lệch khỏi tài liệu này.
- Lỗi thật phát hiện trong lúc chạy là **lỗi**, ghi finding và sửa; không bao giờ báo là "không đủ
  bằng chứng".

## 6. Giao thức smoke: được và không được đặt gì

Smoke: seed **999971**, **một** lần chạy mỗi nhánh, đủ T của bậc thang cao nhất. Seed này
**vắng** khỏi cả hai manifest thí nghiệm; `run_manifest_seed` trả
`RunStatus::Failed{SeedNotInManifest}` cho seed lạ nên nó không thể lọt vào tập phân tích.

**Smoke được phép đặt (và chỉ những thứ này):**

1. `N0` (`live.founders`): giá trị nhỏ nhất ≥ 30 sao cho ở **cả hai** giá trị `food_cap` thử, quần thể
   smoke không xuống dưới 5 agent ở bất kỳ mẫu nào (bằng chứng ADR-0005: 10 founder đã có run xuống 3).
2. `F_hi`: giá trị mặc định (`FoodSpawnSettings::default().max_food_count`) trừ khi nó làm quần thể
   vượt xa cổng sinh 80 con mồi trong 1 000 tick đầu (khi đó chọn giá trị thấp hơn gần nhất, ghi rõ).
3. `F_lo`: ứng viên lớn nhất trong lưới {1/2, 1/3, 1/4, 1/8}·`F_hi` mà **`live.standing_crop` (hoặc
   `live.food_items`) trung bình trên nửa sau smoke xấp xỉ một nửa so với `F_hi`** (trong ±20%) **và**
   quần thể smoke không tuyệt chủng. Nếu không ứng viên nào thoả cả hai: dừng, ghi finding, **không**
   hạ ràng buộc tuyệt chủng.
4. Bậc thang thời lượng (§7) và chiều của H1 (§5.1).

**Smoke KHÔNG được:** đổi câu hỏi, metric, ngưỡng, N, danh sách seed, quy tắc cửa sổ đuôi, hoặc chọn
`F_lo` theo kết quả **khối lượng** (chỉ theo thức ăn và sống sót). Không dùng seed thí nghiệm để
hiệu chỉnh. Đầu ra smoke nằm ở `smoke/`, không bao giờ trộn vào phân tích.

## 7. Bậc thang thời lượng và khoá thực thi

Bậc thang: **18 000 / 12 000 / 6 000 tick**, chọn **một lần** từ smoke (bậc cao nhất mà tổng ước
tính chạy ≤ 90 phút; smoke đo µs/tick, §10). Ghi `execution-lock.json` (bậc, `N0`, `F_hi`, `F_lo`,
chiều H1, hash manifest) **trước** seed thí nghiệm đầu tiên; sau khi một seed thí nghiệm đã chạy,
không được đổi T. Khi vượt ngân sách: hạ bậc theo smoke, **không** hạ N.

## 8. Tuyệt chủng

Một lần chạy mà `live.agent_count` = 0 ở **bất kỳ** mẫu nào là **kết cục được ghi nhận** (không có
nhập cư và không có sinh từ rỗng, nên không phục hồi). Cặp chứa nó bị **loại khỏi thống kê ghép** và
**đếm trong báo cáo** (seed, nhánh, tick tuyệt chủng, liệt kê trong `effects.json`), không bao giờ bị
bỏ im lặng. Tỷ lệ tuyệt chủng theo nhánh là một kết quả mô tả riêng; nếu nó chênh lệch lớn giữa hai
nhánh, đó là **thiên lệch sống sót** và phải nêu trong báo cáo. Nếu < 10 cặp hoàn chỉnh: "không đủ
bằng chứng" (§5.4); không bù seed mới, không đổi N.

## 9. Seed và tất định

- `seed_k = 760001 + 1000·k`, k = 0..11, **n = 12**, tối thiểu **10** cặp hoàn chỉnh. Rời nhau với E2
  (700001…), E3 dự thảo (730001…) và E2R (750001…). Smoke 999971, loại bằng cơ chế (§6).
- Tất định: cùng seed, tiến trình mới → cùng checksum (đảm bảo bởi thứ tự toàn phần F-041). Kiểm
  `--replay` hai seed (một mỗi nhánh) trong tiến trình riêng, như E2. Tính chất fork/save-load
  (F-040): chạy N == chạy K → lưu → nạp → chạy N−K, test sẵn có trong `live_experiment_tests.rs`;
  phải chứng minh được với `live.reproduction=1` (việc của thẻ P1, không phải tài liệu này).

## 10. Chi phí

Tham chiếu duy nhất đã đo: E2, 24 lần chạy × 18 000 tick, **10 agent**: **157 s** (Windows, trước
dormancy) và **66,9 s** (macOS arm64, chạy lại 2026-11-16). E-SEL1 dùng N0 ≥ 30 agent **và** sinh
sản/chết/vận động bật, quần thể có thể lên tới ~100+: chi phí mỗi tick **sẽ cao hơn, chưa đo**; không
ngoại suy tuyến tính ở đây. Smoke phải đo µs/tick thật trên bậc cao nhất và ghi vào fixture
(`cost.measured_*`). Số lần chạy: 24 (hai nhánh) hoặc 36 (có `scarce_mut0`). Trần 90 phút tường,
bản `release`, biên dịch `-j 2`; vượt trần thì hạ bậc §7.

## 11. Mối đe doạ tính hợp lệ

- **Một địa hình, con mồi thuần**: không khái quát được; không có thú săn nên không có áp lực săn.
- **Đột biến cứng 0,05** (`agent_systems.rs`) nên cường độ biến dị là hằng, không phải biến thí nghiệm.
  Nếu P4 không có, **không có đối chiếu trôi** và mọi dịch chuyển có thể là trôi + sắp xếp founder.
- **Khối lượng là hàm của kiểu gen qua phát triển/giải mã**: "di truyền" nghĩa là truyền qua
  `mutate_genotype`; không có kiểm chứng riêng rằng mọi biến thiên `trait_mass_mean` là do gen. Nêu
  rõ, không gọi là "tính trạng thích nghi".
- **Thiên lệch sống sót** từ tuyệt chủng (§8) và **động lực bùng/sập** (ADR-0005: 102→14→135): trung
  bình đuôi có thể phản ánh pha dao động thay vì xu hướng. Cửa sổ đuôi giảm nhưng không loại.
- **`live.mortality` với `starve_ticks` mặc định**: hai nhánh chia sẻ cùng ngưỡng; chọn khác đi sẽ là
  thí nghiệm khác.
- **Công suất**: dịch chuyển nhỏ so với biến thiên giữa seed có thể cho null chỉ vì n = 12; null
  không có nghĩa là "không có tác động".
- **Phụ thuộc ADR-0006**: nếu vận động làm con mồi tuyệt chủng (phương án (a) của ADR-0006 đã làm
  vậy 3/3), smoke sẽ chặn thí nghiệm; đó là kết quả smoke, không phải lý do hạ ràng buộc.
- **Đo thăm dò 2026-11-16, sau khi #76 merge (chưa đăng ký):** với `live.locomotion = 1`, tử vong và sinh sản
  bật, 10 founder, 6.000 tick, **chỉ con mồi** (`predator_fraction = 0`) vẫn **tuyệt chủng ở 2/5 seed** —
  chết khát, vì bộ lái chỉ hướng tới thức ăn, không bao giờ tới nước. Founder ở 3/5 seed gần như đứng yên vì
  biên độ CPG chỉ ≈ 0,05–0,1, mà tốc độ của #76 lấy theo biên độ. Hai việc này (sàn tốc độ + lái tới nước khi
  khát) là phương án A của đề xuất ADR-0007; nếu chưa có, smoke rất có thể chặn E-SEL1 vì tuyệt chủng lấn át.

## 12. Sẽ KHÔNG được tuyên bố

- "Thích nghi" hay "tiến hoá thích nghi": cần bằng chứng có tên (CLAUDE.md); thí nghiệm này chỉ ghi
  một chênh lệch có điều kiện.
- **Thích nghi địa phương**: CM-S11 (reciprocal-transplant) **không chạy**.
- **Phân loài/loài**: không dùng chữ "species"; không có bằng chứng AE-S11/AE-S14.
- Bất cứ điều gì về **thú săn, đồng tiến hoá Red-Queen** (S43 cần thú săn) hay **não** (`evolved_brains=0`).
- Khái quát ngoài **một địa hình**, ngoài adapter headless, hoặc "thế giới live sẵn sàng thí nghiệm".

## 13. Tiền điều kiện chặn (kiểm được)

Đóng băng và chạy smoke **bị chặn** cho tới khi các mục dưới đây đúng, mỗi mục kiểm bằng lệnh/test:

- **P1** — `live.reproduction` có trên trunk: PR **#66** đã merge vào `feat/ecological-selection`;
  `grep -n LIVE_KEY_REPRODUCTION src-tauri/src/core/live_experiment.rs` có kết quả và
  `build_live_world` không còn `ReproductionPolicy::disabled()` cắm cứng.
- **P2** — `live.locomotion` (ADR-0006, PR #76) đã merge: `LIVE_KEY_LOCOMOTION` có mặt và các test
  `locomotion_steering_tests` (vận động thật khi bật, đường mặc định không đổi, tất định qua nhiều tiến
  trình, EU kín) xanh trên thân cây. Bộ lái được cài ở dạng **opt-in**: thế giới mặc định không đổi,
  nên `LIVE_MODEL_VERSION` không tăng và test `founders_oscillate_but_do_not_translate` (thế giới mặc
  định) vẫn giữ nguyên. E-SEL1 bật nó tường minh bằng `live.locomotion = 1` ở cả hai nhánh.
- **P3** — Có **chương trình chạy biên dịch sẵn** theo mẫu `run_e2_brain_experiment`, tên dự kiến
  `run_esel1_experiment` (`src-tauri/examples/`), hỗ trợ `--smoke`, `--ensemble`, `--replay`, ghi
  `execution-lock.json`; **không bao giờ `cargo run`**. Cần **một thẻ** riêng để dựng nó (số thẻ gán
  khi tạo; chưa tồn tại).
- **P4 (ưu tiên, không bắt buộc cứng)** — khoá `live.mutation_rate` trong adapter (vắng = 0,05 như
  `agent_systems.rs` hiện cắm cứng, để mọi checksum cũ bit-identical). Có P4 → có nhánh
  `scarce_mut0` (§3). Không có P4 → chạy hai nhánh và nêu giới hạn §11, nhưng **quyết định này phải
  được ghi trước khi đóng băng**, không phải sau.
- **P5** — F-039 (`live.births` đúng khi sinh sản liên tục) có trên trunk (đã có; xác nhận lại bằng
  test lúc đóng băng).
- **P6** (công việc đóng băng, không chặn smoke) — fixture `e-sel1` + test hình dạng +
  `scripts/verify_esel1_artifacts.mjs` (mẫu `verify_e2_artifacts.mjs`).

## 14. Lệnh tái lập và loại trừ smoke

Chỉ biên dịch rồi chạy tệp nhị phân (không `cargo run`); xem `reproduction_command` và
`smoke_command` trong Phụ lục A. Smoke luôn dùng manifest `smoke_*` riêng (seed 999971).

## 15. Việc phải làm trước khi đây thành tiền đăng ký

1. P1–P3 xong và quyết định về P4 được ghi. 2. Commit Phụ lục A làm
`src-tauri/tests/fixtures/experiments_esel1/esel1-preregistration.json` cùng các manifest và test hình
dạng. 3. Chạy smoke, điền `[SMOKE]`, khoá bậc thang và chiều H1 bằng khối sửa đổi có ngày. 4. Chạy 12
seed. Đầu ra dưới `artifacts/experiments/e-sel1-food-scarcity/`.

## Phụ lục A — Dự thảo JSON tiền đăng ký (LỊCH SỬ — bản đóng băng là fixture)

> **DRAFT — freeze by committing it as a fixture before any smoke run.** Các giá trị `"[SMOKE...]"` là
> chỗ trống có chủ đích; chúng được điền bằng khối sửa đổi có ngày, không sửa im lặng.

```json
{
  "schema_version": 1,
  "preregistration_id": "e-sel1-food-scarcity-body-mass",
  "status": "DRAFT",
  "registered_on": null,
  "registered_at_parent_commit": null,
  "contains_results": false,
  "question": "In the live world with energy-limited reproduction and starvation mortality, does food scarcity (live.food_cap) shift the population's mean body mass (live.trait_mass_mean)?",
  "model": "core::live_experiment::LiveExperimentAdapter",
  "model_status": "headless adapter verified; NOT a live-world experiment-ready claim",
  "registry": "core::experiment::ObservableRegistry::live_default",
  "harness": "core::experiment_runner::run_paired_ensemble_with_control",
  "manifests": {
    "control": "esel1-control-abundant.json",
    "treatment": "esel1-treatment-scarce.json",
    "drift_control_optional_requires_P4": "esel1-drift-scarce-mutation-off.json",
    "smoke_control": "esel1-smoke-abundant.json",
    "smoke_treatment": "esel1-smoke-scarce.json"
  },
  "declared_factor": {
    "factor_diff_allowed_paths": ["initial_conditions"],
    "exact_initial_condition_difference": {
      "key": "live.food_cap",
      "control_value": "[SMOKE: F_hi]",
      "treatment_value": "[SMOKE: F_lo]"
    },
    "every_other_initial_condition_must_match": true
  },
  "common_initial_conditions": {
    "live.mortality": 1.0,
    "live.reproduction": 1.0,
    "live.locomotion": 1.0,
    "live.evolved_brains": 0.0,
    "live.predator_fraction": 0.0,
    "live.founders": "[SMOKE: N0, >= 30]"
  },
  "choice_rules_from_smoke": {
    "N0": "smallest value >= 30 such that the smoke population never falls below 5 agents at either food_cap",
    "F_hi": "FoodSpawnSettings::default().max_food_count unless it overshoots the 80-prey reproduction gate within the first 1000 ticks",
    "F_lo": "largest of {1/2,1/3,1/4,1/8} * F_hi whose smoke mean live.standing_crop over the second half is within 20% of half of F_hi's, and the smoke population does not go extinct; if none, stop and file a finding",
    "must_not_be_chosen_by": "any body-mass outcome"
  },
  "hypotheses": [
    {
      "id": "H1",
      "role": "primary",
      "observable": "live.trait_mass_mean",
      "statistic": "paired_mean_delta_of_tail_window_mean",
      "tail_window": "samples with tick > 0.8 * duration_ticks",
      "direction": "[SMOKE: fixed once after smoke, else two_sided]",
      "arm_order": "delta = scarce - abundant"
    },
    {
      "id": "H2",
      "role": "secondary_requires_P4",
      "observable": "live.trait_mass_mean",
      "statistic": "paired_mean_delta_of_tail_window_mean",
      "direction": "two_sided",
      "contrast": "scarce vs scarce_mutation_off",
      "decision_effect": "none",
      "rationale": "descriptive only; does not separate selection from drift (see plan section 5.3)"
    },
    {
      "id": "H3",
      "role": "descriptive",
      "observables": ["live.agent_count", "live.standing_crop", "live.food_items", "live.births", "live.deaths", "live.mean_lifespan", "live.trait_mass_var", "live.trait_mass_p10", "live.trait_mass_p90"],
      "decision_effect": "none"
    },
    {
      "id": "H4",
      "role": "harness_integrity",
      "observable": "live.closed_eu_total",
      "statistic": "constant_within_each_run",
      "decision_effect": "none",
      "rationale": "a drift in closed EU, or live.births == 0 while the population grew, is filed as a defect finding, not as biology"
    }
  ],
  "seeds": {
    "n_requested": 12,
    "min_complete_pairs_for_a_decision": 10,
    "execution_order": [760001, 761001, 762001, 763001, 764001, 765001, 766001, 767001, 768001, 769001, 770001, 771001],
    "rule": "seed_k = 760001 + 1000*k for k in 0..11, fixed before any run; disjoint from E2 700001.., E3 draft 730001.., E2R 750001..",
    "smoke_seed": 999971,
    "smoke_seed_is_excluded_from_analysis": true,
    "smoke_seed_exclusion_is_mechanical": "999971 is absent from every experimental manifest's seed list, and run_manifest_seed returns RunStatus::Failed{SeedNotInManifest} for a seed the manifest does not declare."
  },
  "duration": {
    "duration_ticks": "[SMOKE: one of the ladder]",
    "duration_ticks_ladder": [18000, 12000, 6000],
    "ladder_rule": "Chosen ONCE from the smoke, before any experimental seed runs, written to execution-lock.json. Never changed after an experimental seed has run.",
    "sample_period": 600,
    "warmup_ticks": 3000,
    "warmup_samples_excluded_from_time_averages": 5
  },
  "extinction_rule": {
    "definition": "live.agent_count == 0 at any sample",
    "handling": "recorded outcome; the pair is dropped from the paired statistic and counted in the report with seed, arm and tick; never silently",
    "fewer_than_10_complete_pairs": "insufficient_evidence; no replacement seeds, N is never changed"
  },
  "thresholds": {
    "materiality_rule": "all_of",
    "conditions": [
      "ci95 on paired_mean_delta excludes 0",
      "abs(paired_dz) >= 0.8",
      "abs(paired_mean_delta) >= 0.05 * abs(control_mean)"
    ],
    "null_result_rule": "ci95 includes 0, OR abs(paired_mean_delta) < 0.05 * abs(control_mean)",
    "between_rule": "detectable but below the preregistered materiality threshold; counts as NOT different enough"
  },
  "reporting": {
    "per_observable_fields": ["n_requested", "n_complete_pairs", "n_extinct_pairs_dropped", "control_mean", "treatment_mean", "paired_mean_delta", "paired_sd", "paired_se", "ci95_low", "ci95_high", "paired_dz"],
    "additional_derived": ["median_of_per_seed_deltas", "between_seed_variance_of_deltas", "per_seed_delta_table", "extinction_table_by_arm"],
    "failed_runs": "every failed run is reported with its seed and reason; none is dropped",
    "negative_and_null_results_are_valid_results": true,
    "metric_switching_after_results": "forbidden"
  },
  "cost": {
    "reference_measured": "E2: 24 runs x 18000 ticks at 10 agents = 157 s (Windows, July) and 66.9 s (macOS arm64, 2026-11-16 re-run)",
    "this_experiment": "larger populations (>= 30 founders, reproduction on) cost more per tick; UNMEASURED, to be measured in the smoke and recorded in cost.measured_us_per_tick_release",
    "runs": "24 (two arms x 12) or 36 (with drift arm)",
    "max_wall_clock_minutes": 90,
    "build_profile_required": "release",
    "on_budget_exceeded": "step down the duration ladder using the smoke only, before any experimental seed runs; never lower N"
  },
  "artifacts": {
    "root": "artifacts/experiments/e-sel1-food-scarcity/",
    "files": ["paired-report.json", "effects.json", "per-seed-deltas.csv", "timeseries.csv", "execution-lock.json", "provenance.json", "summary.md", "smoke/"]
  },
  "reproduction_command": "cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop --example run_esel1_experiment; then execute src-tauri/target/release/examples/run_esel1_experiment --ensemble --manifest-dir src-tauri/tests/fixtures/experiments_esel1 --out artifacts/experiments/e-sel1-food-scarcity",
  "smoke_command": "cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop --example run_esel1_experiment; then execute src-tauri/target/release/examples/run_esel1_experiment --smoke --manifest-dir src-tauri/tests/fixtures/experiments_esel1 --out artifacts/experiments/e-sel1-food-scarcity",
  "smoke_exclusion": "seed 999971 only in smoke_* manifests; absent from every experimental manifest",
  "blocking_preconditions": [
    "P1: live.reproduction exists on the trunk (PR #66 merged; LIVE_KEY_REPRODUCTION present; build_live_world no longer hard-codes ReproductionPolicy::disabled()).",
    "P2: live.locomotion exists on the trunk (ADR-0006 implementation, PR #76, merged; opt-in: default world unchanged, so LIVE_MODEL_VERSION and the F-042 characterization test stay as they are; locomotion_steering_tests green). Both arms set live.locomotion = 1.",
    "P3: the compiled runner example run_esel1_experiment exists (--smoke, --ensemble, --replay, execution-lock.json); needs its own card; never cargo run.",
    "P4 (preferred): live.mutation_rate manifest key (absent == 0.05) so a mutation-off drift arm can exist; if absent, the no-drift-control limitation is stated and this is recorded BEFORE freezing.",
    "P5: F-039 live.births fix on the trunk (verify at freeze time)."
  ],
  "decision_rule": {
    "shift_in_model_only_if": "H1 reaches materiality AND >= 10 complete pairs AND no harness-integrity finding is open AND cargo test --features desktop is green",
    "null_if": "H1 is null or below materiality with >= 10 complete pairs",
    "insufficient_evidence_if": "fewer than 10 complete pairs, or a blocking precondition was satisfied in a way that departs from this preregistration",
    "defects_are_not_insufficiency": "a real defect found during the run is recorded and fixed as a defect"
  },
  "will_not_be_claimed": [
    "adaptation or local adaptation (CM-S11 not run)",
    "speciation or species",
    "anything about predators, Red-Queen coevolution (S43) or brains",
    "generality beyond one terrain seed and the headless adapter"
  ]
}
```
