---
phase: planning
feature: experiment-sel2-food-scarcity
title: Đề xuất làm lại SEL2 từ hai báo cáo nghiên cứu ngày 2026-11-18
description: Sau khi lượt đo S6 không tìm ra chế độ khan hiếm thức ăn khả thi, đề xuất một chuỗi thẻ mới (S9 tách nguyên nhân chết, S10 phần cây dự trữ, SEL2-E đo thăm dò thứ hai, SEL2-D đối chứng drift) và đổi cách đo của SEL2 sang endpoint không bị sàn che. Chờ chủ dự án quyết
status: proposed
owner: simulation-architecture
last_reviewed: 2026-11-18
---

# Đề xuất làm lại SEL2

> **Đề xuất, chờ chủ dự án quyết.** Không thẻ nào dưới đây được giao, và chưa có dòng `AUTHORIZATION` nào
> có hiệu lực. Các mục cần quyết nằm ở [§7](#7-chủ-dự-án-cần-quyết).

## 1. Tóm tắt

Ba lần thử đo chọn lọc tự nhiên trên thế giới sống đều chưa ra kết quả: E2 null vì quần thể chạm sàn năng
lượng, E-SEL1 đóng ngay tại smoke, và lượt đo thăm dò S6 không tìm ra cấu hình nào vừa đưa standing crop vào
dải 0,4–0,6 vừa giữ quần thể ≥ 5 ở mọi seed. Hai báo cáo nghiên cứu ngày 2026-11-18 chỉ ra một lời giải thích
có cơ sở lý thuyết cho lần thứ ba: đồng cỏ mọc lại theo logistic và **không có phần nào con vật ăn không tới**,
nên một quần thể gặm đủ mạnh để tạo khan hiếm sẽ không dừng ở mức khan hiếm vừa phải mà trượt xuống tận đáy.
Đề xuất dưới đây làm bốn việc theo thứ tự, mỗi việc chỉ bắt đầu khi việc trước đã cho số: **tách nguyên nhân
chết** (đói hay khát), **thêm một khoá phần cây dự trữ**, **đo thăm dò lại** với nhiều founder hơn và nhiều
nước hơn, rồi mới viết **bản đăng ký SEL2 mới** với endpoint không bị sàn che và một nhóm đối chứng drift.
Lão hoá, thức ăn dạng mảng và locus trung tính để sau, vì chúng chạm vòng đời hoặc genotype và cần ADR.

## 2. Nguồn và độ tin cậy

Hai báo cáo do Gemini Deep Research soạn theo
[prompt nghiên cứu](#phụ-lục-prompt-đã-dùng) ngày 2026-11-18; chủ dự án giữ bản gốc, repo không lưu chúng:

| Báo cáo | Câu hỏi | Dùng ở đây cho |
|---|---|---|
| *Sustaining Selection in ALife Ecologies* | Câu 1: chọn lọc đo được mà quần thể không sụp | §3, §4, §5 |
| *Desktop Multi-Agent Simulation Throughput* | Câu 3: thế giới 1.000 agent đủ nhanh trên một máy | §6 |

Cả hai đã được đọc hết và đối chiếu với mã trên thân cây tại `8c7c2c7`. Ba điều cần biết khi dùng chúng:

- **Các con số cốt lõi của báo cáo 1 là mô phỏng riêng của Gemini, không phải nguồn.** Bảng "mô phỏng chăn thả
  12 seed", các mức power 69–91 % cho RMST và AUC, và bảng ngưỡng drift theo cỡ quần thể đều trích về nguồn số
  5, mà nguồn đó là `unknown_url`. Chúng là kết quả của một mô hình đồ chơi do chính báo cáo dựng. Ở đây chúng
  chỉ được dùng như giả thuyết, không như bằng chứng về thế giới này.
- **Báo cáo 2 giả định một hệ thống khác.** Nó tính cho não 107.000 tham số và vật lý Rapier 3D. Não của dự án
  là 15 → 64 → 64 → đầu ra (`EVOLVED_ARCH` trong `evolution/brain_genotype.rs`), khoảng 5.500 tham số, và vật lý
  là mã riêng. Các ước lượng mili-giây của nó không chuyển sang đây được.
- Nhiều nguồn trích là blog, wiki, Reddit, hoặc ghi ngày 7–9/2026 mà không kiểm lại được trong lượt này.

Số liệu duy nhất ở trang này được tính lại độc lập là cỡ hiệu ứng tối thiểu của phép kiểm t ghép cặp ở §5.

## 3. Bối cảnh: ba lần thử và chúng hỏng ở đâu

| Lần thử | Kết quả | Tài liệu |
|---|---|---|
| E2 (não tiến hoá so với não chung, 12 cặp seed, 18.000 tick) | Null. Cả hai nhánh chạm năng lượng trung bình 0 trước tick ~9.000 và quần thể thôi đổi, nên đo ở tick 18.000 không còn gì để so | `artifacts/experiments/e2-evolved-brain-default/RESULT.md` |
| E-SEL1 (khan hiếm thức ăn và khối lượng cơ thể) | Đóng tại smoke, không seed thí nghiệm nào chạy | [bản đăng ký](2026-11-16-experiment-e-sel1-food-scarcity-body-mass.md), [chẩn đoán](2026-11-17-esel1-scarcity-diagnosis.md) |
| S6 (nâng `live.prey_cap`, hạ `live.resource_capacity_scale`) | Không cấu hình nào đạt cả hai điều kiện; hạ sức chứa thì quần thể chết ở 5 trên 6 seed | [lượt đo S6](2026-11-17-s6-sel2-exploration.md) |

Thế giới nền của S6 là của E-SEL1: 30 founder con mồi, không thú săn, 8 cây, 2 hồ; bật tử vong, sinh sản, vận
động với `live.drive_floor` 0,5 và lái tới nước; não chung; `live.food_cap` 50. Lượt S6 **không** tách nguyên
nhân chết, và bản chẩn đoán trước nó đã thấy nước giới hạn quần thể ở cấu hình nền.

## 4. Chẩn đoán của báo cáo 1, đối chiếu với mã

| Khẳng định của báo cáo | Mã nói gì | Kết luận |
|---|---|---|
| Đồng cỏ mọc lại logistic không có phần dự trữ thì không đứng yên ở mức khan hiếm vừa phải (lý thuyết Noy-Meir 1975, Rosenzweig–MacArthur) | `ecology::logistic_regrowth` là `R + g·R·(1 − R/R_max)`; chỉ gieo lại 0,1 % `r_max` khi ô gần cạn. `herbivore_grazing_system` cho con vật ăn theo Holling II trên **toàn bộ** `r` của ô | **Khớp.** Không có phần nào của ô mà con vật không ăn tới |
| Hạ sức chứa để tạo khan hiếm đồng thời hạ tổng dòng năng lượng, nên quần thể nhỏ lại tới mức nhiễu nhân khẩu học giết nó | S6: ở hệ số 0,5 và 0,35, tỉ số crop vào dải nhưng quần thể chết ở 2/6 và 5/6 seed | **Khớp** với kết quả S6 |
| Năng lượng về 0 mà không chết thì quần thể "đóng băng" | Tử vong do đói và khát chỉ có khi khai `live.mortality`; E2 chạy trước khi có khoá đó | **Khớp** với E2 |
| Không có lão hoá thì cá thể "đủ tốt" sống mãi và chặn lớp sau | Mã không có trường tuổi nào và không có chết vì già | **Khớp** |
| Nên thêm nguồn bổ sung từ bên ngoài ("semi-chemostat") | Sổ EU khép kín: cây chỉ mọc bằng detritus (`step_regrowth_gated`), mọi EU lấy ra đều trả về đâu đó | **Không áp dụng được.** Chỉ lấy được ý "phần dự trữ", vốn giữ được sổ khép kín |
| Nên thêm mùa | `ecology::seasonal_fertility`, biên độ `SEASON_AMPLITUDE` = 0,5 | **Đã có**, và biên độ còn cao hơn mức báo cáo gợi ý (0,25–0,40) |
| Cây mọc lại tại chỗ ở mọi ô thì ngồi yên có lợi hơn di chuyển ("bẫy mọc tại chỗ") | Đồng cỏ phủ mọi ô có `r_max` > 0 và mọc tại chỗ | **Có khả năng khớp**, chưa đo. Liên quan trực tiếp tới E2R vì E2R hỏi về não lái |
| 10 founder là quá ít | Mặc định của app là 10; thế giới nền của S6 đã dùng 30 | Một phần. S6 đã ở 30, báo cáo khuyên 60–100 |

Một quan sát phụ khi đọc `mortality_system` (`core/mortality.rs`): cái chết vì khát được ghi vào
`DemographicTracker` dưới nhãn `"Starvation"`, và mọi cái chết của cá thể sống ≥ 300 tick được ghi là
`"Senescence"` dù mã không có lão hoá. Hai nhãn đó vì thế không dùng được để đếm nguyên nhân chết. Thẻ S9 dưới
đây sửa đúng chỗ này cho các lượt đo.

## 5. Đề xuất: bốn thẻ theo thứ tự

Mỗi thẻ chỉ bắt đầu khi thẻ trước đã merge và cho số. Mọi khoá mới là opt-in: khoá vắng thì thế giới chạy đúng
như trước, và thẻ chứng minh điều đó bằng so sánh các lượt chạy với nhau, không ghim checksum tuyệt đối.

### S9 — tách nguyên nhân chết (`[RUST]`, nhỏ)

**Vì sao trước tiên.** Nếu khát giết phần lớn quần thể ở các cấu hình khan hiếm, thì thao tác thức ăn không thể
là yếu tố duy nhất của SEL2, và mọi lượt đo thăm dò tiếp theo phải tính tới nước. Lượt S6 không biết điều này.

**Việc.** `mortality_system` ghi riêng chết vì đói và chết vì khát vào `VitalStatistics`, và adapter sống có hai
observable mới, `live.deaths_starvation` và `live.deaths_dehydration`, nằm trong `LIVE_EXTENDED_OBSERVABLE_IDS`.
Danh sách `LIVE_OBSERVABLE_IDS` giữ nguyên vì nó đã đóng băng (QĐ-E2M). Không đổi quỹ đạo.

**Xong khi:**
1. Tổng hai observable mới bằng `live.deaths` ở mọi mẫu.
2. Một test chạy cùng seed có và không đọc hai observable mới cho checksum trùng nhau.
3. Lượt đo lại các ô có hệ số 0,5 và 0,7 của lưới S6, seed 1–6, báo tỉ lệ chết vì đói và vì khát. Mù với khối
   lượng như S6.

### S10 — khoá `live.plant_reserve` (`[RUST]`, nhỏ)

**Việc.** Một phân số `ρ ∈ [0, 1)` của `r_max` mỗi ô mà con vật không ăn được: `herbivore_grazing_system` đưa cho
`herbivore_intake` lượng `max(r − ρ·r_max, 0)` thay vì `r`. Phần gặm cỏ của quần thể gộp
(`aggregate_population.rs`, chỗ gọi `herbivore_intake` theo đầu cá thể) dùng cùng quy tắc. Cây vẫn mọc lại bằng
detritus như cũ, nên sổ EU vẫn khép kín: phần dự trữ chỉ là cỏ còn lại trên ô, không phải năng lượng mới.

**Xong khi:**
1. Khoá vắng (hoặc `0`) thì quỹ đạo trùng với lượt chạy trước khi có khoá, cùng seed.
2. Với `ρ > 0`, không ô nào bị gặm xuống dưới `ρ·r_max` trong một lượt chạy có quần thể đông.
3. Dư sổ EU vẫn trong ngưỡng của các test sổ cái hiện có.
4. Cùng manifest cho cùng checksum ở nhiều tiến trình riêng.
5. Nếu tỉ số crop của observable đổi nghĩa khi có dự trữ, thẻ nói rõ và đề xuất cách đọc.

### SEL2-E — đo thăm dò lần hai (mù với khối lượng, như S6)

**Câu hỏi.** Có cấu hình nào giữ quần thể sống ở mọi seed, đủ thế hệ, trong khi thức ăn thật sự giới hạn?

**Lưới đề xuất** (thế giới nền của S6, `live.prey_cap` 320, `live.resource_capacity_scale` 1,0):
`live.founders` ∈ {30, 60, 100} × `live.plant_reserve` ∈ {0; 0,2; 0,3; 0,4} × `live.lakes` ∈ {2, 4} × seed 1–6,
tức 144 lượt 18.000 tick. Nếu S9 cho thấy khát không giết đáng kể, bỏ chiều `live.lakes`. Seed 1–6 là seed thăm
dò; bản đăng ký sau đó phải dùng seed riêng.

**Điều kiện một ô lưới được coi là khả thi** (thêm vào hai điều kiện cũ của S6):
1. quần thể tối thiểu ≥ 5 ở mọi seed;
2. tỉ số standing crop nằm trong dải đã khai (cách đo theo S10.5);
3. **ít nhất 30 thế hệ** trong lượt chạy, ước lượng từ số lần sinh và tuổi thọ trung bình (`live.mean_lifespan`);
4. phần lớn cái chết là do đói, không do khát (từ S9);
5. trong suốt lượt chạy vẫn có sinh và chết, tức quần thể không đóng băng.

Điều kiện 3 đến từ báo cáo 1, vốn đề nghị đo theo thế hệ sinh học (30–90) thay vì số tick. Nếu không ô nào đạt,
lượt đo trả kết quả âm như S6, và bước kế tiếp là các mục ở §5.5, không phải nới điều kiện.

### SEL2-P — bản đăng ký mới (chỉ khi SEL2-E tìm ra ô khả thi)

Theo mẫu của E2: fixture, smoke qua kiểm tra thao tác, `RESULT.md` kể cả khi null. Bốn thay đổi so với bản
đăng ký E-SEL1:

1. **Endpoint không bị sàn che.** Endpoint chính là một đại lượng tích phân theo thời gian (diện tích dưới đường
   năng lượng trung bình, hoặc thời gian sống sót trung bình có giới hạn, RMST) thay vì ảnh chụp cuối lượt.
   Thẻ S2 đã định làm observable tích luỹ; SEL2-P dùng nó.
2. **Đo chọn lọc trên genotype, không trên kiểu hình.** Vì mô phỏng biết genotype chính xác của từng cá thể, hiệp
   phương sai giữa fitness tương đối và giá trị di truyền (đồng nhất thức Robertson–Price) loại được nhiễu "sinh
   ra ở ô nhiều cỏ thì béo hơn" mà gradient Lande–Arnold trên kiểu hình mắc phải.
3. **Số seed.** Cỡ hiệu ứng ghép cặp tối thiểu phát hiện được ở power 80 %, α = 0,05 hai phía, tính lại độc lập
   cho trang này: 12 seed `d_z` ≈ 0,89; 16 seed ≈ 0,75; 20 seed ≈ 0,66; 24 seed ≈ 0,60. Đề xuất **16–24 seed**,
   tuỳ thời gian chạy đo được ở SEL2-E.
4. **Nhóm đối chứng.** `live.mutation_rate = 0` (đã có) cho nhóm "không đột biến". Nhóm "drift" là thẻ SEL2-D.

### SEL2-D — nhóm đối chứng drift khoá bước (`[RUST]`, vừa)

**Việc.** Ghi số sinh và chết theo tick của một lượt "có chọn lọc", rồi phát lại từ cùng founder nhưng mỗi cái
chết rơi vào một cá thể chọn ngẫu nhiên và mỗi lần sinh lấy cha mẹ ngẫu nhiên, cùng quy tắc đột biến. Cách này
là "neutral shadow" của Bedau–Packard và chế độ "passive" của Polyworld; mô phỏng tất định của dự án làm được
nó. Thay đổi trait trong nhóm có chọn lọc phải vượt nhóm này mới được gọi là chọn lọc.

Thẻ này lớn hơn S9 và S10 vì nó thêm một chế độ chạy. Có thể làm song song với SEL2-E.

### 5.5 Để sau, cần ADR

| Việc | Vì sao để sau | ADR liên quan |
|---|---|---|
| Lão hoá (chết vì tuổi) | Thêm trường tuổi vào sinh vật, chạm vòng đời và định dạng lưu | ADR-0001, ADR-0005; vùng của thẻ V3 |
| Thức ăn dạng mảng rời để di chuyển có lợi | Đổi luật đồng cỏ; liên quan tới câu hỏi não lái của E2R | ADR-0007 cập nhật |
| Locus trung tính ẩn trong genome | Đổi genotype | ADR-0001 |
| PRNG không trạng thái theo `(seed, tick, agent)` | Chỉ cần nếu LOD hay fork sau này làm lệch thứ tự rút số | ADR-0004 |

## 6. Từ báo cáo 2: hiệu năng và thời gian

Không thẻ nào ở đây; đây là ghi chú cho các thẻ đã có.

- **Không cần GPU ở 1.000 sinh vật.** Với não khoảng 5.500 tham số, suy luận theo batch trên CPU rẻ hơn độ trễ cố
  định của một lần gọi GPU. Điều này củng cố việc để GPU tắt mặc định. Lý do của thẻ R1c vì thế là giấy phép của
  `hexf-parse`, không phải hiệu năng.
- **Cảnh báo "LOD theo camera làm đổi lịch sử" đã được xử lý.** `simulation_lod.rs`: ở chế độ `Spectate` camera
  không tác động gì; ở `Inhabit` tác động của nó là một can thiệp được khai (ADR-0004).
- **Cho thẻ H6 (lượt đo app bản release, 1.000 sinh vật).** Bắt đầu từ một snapshot đã ổn định chứ không từ tick
  0; bỏ vài trăm tick khởi động; chạy ngâm nhiệt khoảng 15 phút; báo p50/p95/p99. i5-14600KF có cả lõi P lẫn lõi
  E, nên kết quả có thể dao động theo lõi mà hệ điều hành chọn. Tick capture đã báo p50 và p95.
- **Cho QĐ-TIME và ADR-0008.** Số tick mỗi giây cần đạt = số tick của một ngày mô phỏng / 7.200. Nếu một ngày là
  N tick, ngân sách mỗi tick khoảng 7.200 / N giây trừ phần dành cho hiển thị.
- **Cho QĐ-M6 (tái lập giữa các máy).** Ba hướng đáng thử: tắt gộp nhân-cộng (`-C llvm-args=-fp-contract=off`),
  dùng một thư viện `libm` thuần Rust cho các hàm siêu việt mà thẻ M1 thấy lệch 1 ULP, và đặt chế độ
  flush-to-zero giống nhau trên mọi luồng. Con số cụ thể của báo cáo về các hướng này chưa kiểm.
- **Không áp dụng:** cấu hình Bevy 0.18–0.20, Rapier/Avian, ONNX Runtime, và `-C target-cpu=native`. Cờ cuối
  làm bản phát hành không chạy được trên máy khác và hại tái lập.

## 7. Chủ dự án cần quyết

| ID | Câu hỏi | Đề xuất |
|---|---|---|
| QĐ-SEL2a | Đi theo chuỗi S9 → S10 → SEL2-E → SEL2-P (kèm SEL2-D), thay cho "đo dày quanh hệ số 0,6–0,7" hoặc "chuyển sang khan hiếm nước" mà lượt S6 nêu? | Có |
| QĐ-SEL2b | Thêm "≥ 30 thế hệ" và "chết chủ yếu vì đói" vào điều kiện khả thi của SEL2-E? | Có |
| QĐ-SEL2c | SEL2-P dùng 16 hay 24 seed? | Quyết sau SEL2-E, khi biết một lượt chạy tốn bao lâu |
| QĐ-SEL2d | Uỷ quyền R5 cho S9 và S10 | Dòng nháp bên dưới |

Dòng uỷ quyền nháp. Danh sách path được soạn từ việc đọc mã, chưa thử bằng một lần sửa thật; thẻ dừng và hỏi nếu
cần sửa file nào khác dưới `src-tauri/src`.

```text
AUTHORIZATION (owner, <YYYY-MM-DD>): for card S9 only, you may write to src-tauri/src/core/mortality.rs,
src-tauri/src/core/resources.rs (VitalStatistics only), src-tauri/src/core/live_experiment.rs (two new
ids in LIVE_EXTENDED_OBSERVABLE_IDS and the code that reports them), one new test file under
src-tauri/tests/, and src/types/generated/** only if ts-rs regenerates it. This overrides AGENTS.md's
read-only declaration and BLOCK 0 R5 for exactly these paths and no others.

AUTHORIZATION (owner, <YYYY-MM-DD>): for card S10 only, you may write to
src-tauri/src/core/environmental_systems.rs (herbivore_grazing_system only),
src-tauri/src/core/aggregate_population.rs (the per-capita grazing call only),
src-tauri/src/core/live_experiment.rs (the live.plant_reserve key), src-tauri/src/core/resources.rs
(only if the reserve must live in a resource), one new test file under src-tauri/tests/, and
src/types/generated/** only if ts-rs regenerates it. This overrides AGENTS.md's read-only declaration
and BLOCK 0 R5 for exactly these paths and no others.
```

## 8. Điều đề xuất này không khẳng định

- Không khẳng định phần cây dự trữ sẽ cho ra một chế độ khả thi trong thế giới này. Lý thuyết nói nó loại bỏ
  điểm rẽ không ổn định; SEL2-E là phép đo trả lời.
- Không khẳng định khát hay đói giết nhiều hơn. Đó là việc của S9.
- Không dùng bất kỳ con số nào từ mô phỏng riêng của báo cáo 1 làm ngưỡng hay dự đoán.
- Không đổi điều kiện của bản đăng ký nào đã đóng băng.

## Phụ lục: prompt đã dùng

Hai báo cáo trả lời câu 1 và câu 3 trong bộ bảy câu hỏi nghiên cứu soạn ngày 2026-11-18 trong chat, dán sau một
khối bối cảnh chung mô tả hệ thống mà không nêu tên dự án. Năm câu còn lại (tái lập giữa các máy, nâng `burn`,
lưu lịch sử bất biến, thí nghiệm rẽ luật, thế giới nhiều máy) chưa có báo cáo.
