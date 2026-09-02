---
phase: planning
feature: experiment-e3-productivity-gradient
title: Kế hoạch — E3, gradient năng suất theo không gian có làm phân tán khối lượng cơ thể không
description: Dự thảo tiền đăng ký E3 — nhân tố, N, seed, T, metric chính, ngưỡng vật chất, và hai dụng cụ còn thiếu so với câu hỏi gốc
status: draft
owner: simulation-architecture
last_reviewed: 2026-08-26
---

# Kế hoạch — E3: gradient năng suất theo không gian và phân tán khối lượng cơ thể

> ⚠️ **ĐÂY CHƯA PHẢI TIỀN ĐĂNG KÝ.** Nó là **dự thảo** của một tiền đăng ký. E2 chỉ thành tiền đăng
> ký khi có đủ ba thứ: tài liệu này đóng băng, một fixture JSON máy đọc được, và một test khẳng định
> hai bên khớp nhau. E3 hiện có **một** trong ba. Xem [§9](#9-việc-phải-làm-trước-khi-đây-thành-tiền-đăng-ký).
> Không con số nào dưới đây là kết quả quan sát — chưa có lần chạy nào.
>
> Commit cha khi soạn: `445c9c8` (nhánh `feat/ecological-selection`).

## 1. Vì sao đề bài lệch khỏi câu hỏi trong kế hoạch feature

§10 của [kế hoạch ecological-selection](2026-08-25-feature-ecological-selection.md) đề xuất E3 là:
*"một gradient NPP giữa hai biome có tạo ra phân hoá khối lượng cơ thể không?"*

Đọc mã trước khi viết giao thức cho thấy câu hỏi **đúng nguyên văn đó** hiện không trả lời được, vì
hai dụng cụ:

| Thiếu | Bằng chứng trong mã | Hệ quả |
|---|---|---|
| **Không có núm nào chỉnh NPP theo biome** | `biome_npp()` (`core/ecology.rs:63`) là hàm hằng của `BiomeType`; `ResourceField::from_biomes` đọc nó ở `core/ecs.rs:65`; bản đồ biome đến từ `TerrainMap::load_or_generate(&MapSettings::default(), …)` với `seed: 1337` cố định (`core/terrain.rs:99`) | Gradient NPP theo biome là **hằng số của thế giới**, giống hệt ở mọi arm và mọi seed. Không thể dùng làm nhân tố |
| **Không có observable nào theo vùng** | `live.trait_mass_{mean,var,p10,p90}` (`LIVE_EXTENDED_OBSERVABLE_IDS`, `core/live_experiment.rs:857`) tính trên **toàn quần thể** | Không đọc được "nặng ở nửa giàu, nhẹ ở nửa nghèo". Chỉ đọc được **độ phân tán**, không đọc được **cấu trúc không gian** |

Cái **có sẵn** thì mạnh hơn tưởng:

- `InterventionCommand` mang `region: Region::Rect{…}`, `duration_ticks` và `curve`
  (`crates/anima-domain/src/intervention.rs:105`) — tức là một forcing **theo vùng và kéo dài**, chứ
  không phải một cú sốc một tick.
- `InterventionKind::Deforest` gọi `strip_region`, `AddNutrient` gọi `fertilise_region`
  (`core/live_experiment.rs:597, 627`). Cả hai đi qua `EnergyLedger::transfer` nên **EU vẫn đóng**:
  deforest chuyển standing crop → detritus, add-nutrient rút từ **chính pool detritus đó** ra trở
  lại standing crop, trần là `NUTRIENT_FULL_SCALE_FRACTION = 0.10` mỗi lần band sinh thái nổ.
- Thế giới headless **có** trường tài nguyên: `build_live_world` gọi `core::ecs::init_world()` ở
  dòng đầu (`core/live_experiment.rs:1410`), và chính `init_world` cắm `ResourceField` +
  `EcosystemBiomass`. Nên can thiệp theo vùng có chỗ để tác động, và điều đó **cần được xác minh
  bằng một lần chạy smoke chứ không bằng lập luận này** ([§9](#9-việc-phải-làm-trước-khi-đây-thành-tiền-đăng-ký) WP-2).

**Vì vậy E3 đổi nhân tố, giữ nguyên câu hỏi sinh học.** Thay vì gradient NPP theo biome (không chỉnh
được), E3 áp một **gradient năng suất theo không gian bằng can thiệp**: bỏ rừng nửa Tây, bón nửa
Đông, kéo dài suốt lượt chạy. Đó là cùng một loại nhân quả — một nửa thế giới nuôi được nhiều hơn
nửa kia — chỉ khác đường đặt vào.

> 🔬 **Giới hạn phải nói trước, không phải nói sau khi có số.** Với observable toàn cục, E3 đo
> **phân tán** (variance, khoảng p10–p90), **không** đo phân hoá theo không gian. Một quần thể trải
> rộng khối lượng vì gradient và một quần thể trải rộng vì lý do khác **cho cùng một con số**. E3 vì
> vậy trả lời được "gradient có làm phân bố khối lượng rộng ra không", và **không** trả lời được
> "cá thể ở nửa giàu có nặng hơn không". Câu sau cần một observable theo vùng — ghi ở [§10](#10-hai-dụng-cụ-sẽ-mở-khoá-câu-hỏi-gốc).

## 2. Giả thuyết

- **H1 (chính):** dưới cùng luật fitness sinh thái, một gradient năng suất theo không gian làm
  **tăng** độ phân tán khối lượng cơ thể của quần thể so với một thế giới đồng nhất.
- **H0:** không tăng — khoảng tin cậy 95 % của hiệu ứng ghép cặp phủ 0, hoặc dịch chuyển tương đối
  dưới 5 %.

Hướng của H1 được khai **trước**. Nếu kết quả có ý nghĩa nhưng **ngược hướng** (gradient làm phân bố
**hẹp** lại), đó là một kết quả thật, được báo cáo là bác bỏ H1 theo hướng ngược, **không** được viết
lại thành "có ảnh hưởng".

## 3. Thiết kế: hai arm ghép cặp theo seed

Cùng manifest, cùng seed, khác đúng **một** nhân tố — cường độ can thiệp.

| | Control | Treatment |
|---|---|---|
| Can thiệp 1 | `Deforest`, `Rect{0,0,127,255}` (nửa Tây), `intensity = 0.0` | như control nhưng `intensity = 0.30` |
| Can thiệp 2 | `AddNutrient`, `Rect{128,0,255,255}` (nửa Đông), `intensity = 0.0` | như control nhưng `intensity = 1.00` |
| `start_tick` / `duration_ticks` | `1` / `T` (suốt lượt chạy) | giống hệt |
| `curve` | `Step` | giống hệt |
| `reversible` | `false` | giống hệt |

Control **giữ nguyên hai lệnh** với cường độ 0 thay vì bỏ chúng đi. `validate_intervention` chấp
nhận `intensity == 0.0` (chỉ từ chối số âm hoặc phi hữu hạn — `core/experiment.rs:1067–1072`), và giữ lệnh
lại nghĩa là hai arm đi qua **cùng một đường mã**, chỉ khác độ lớn. Bỏ lệnh đi sẽ biến "cường độ" và
"có hay không một lệnh trong hàng đợi" thành hai nhân tố lẫn vào nhau.

**Hằng số của cả hai arm** (khai ở đây để không ai coi chúng là nhân tố):

| Khoá | Giá trị | Vì sao |
|---|---|---|
| `live.ecological_fitness` | `1` | Không bật thì kênh nhân quả bị cắt: fitness rơi về `Locomotion` và sinh thái không quyết định ai truyền gene. Đây là toàn bộ lý do nhánh F tồn tại |
| `live.mortality` | `1` | Không có cái chết thì áp lực chọn lọc không có răng; nhánh M tồn tại cho việc này |
| `live.founders` | `120` | Đủ để phương sai khối lượng có nghĩa, đủ nhỏ để 24 lượt chạy vừa ngân sách |
| `live.predator_fraction` | mặc định | E3 không hỏi về bậc dinh dưỡng |
| `live.w_surplus`, `live.starve_ticks`, `live.dehydration_ticks` | mặc định | mỗi cái là một câu hỏi khác |
| bản đồ | `MapSettings::default()`, seed `1337` | địa hình là **hằng số**, không phải biến nhiễu |

## 4. N và các seed

**N = 12 cặp**, seed theo quy tắc số học khai trước — cùng lý do như E2 (`seed_k = 730001 + 1000·k`,
`k ∈ 0..11`), theo thứ tự thực thi:

```
730001  731001  732001  733001  734001  735001
736001  737001  738001  739001  740001  741001
```

**Tối thiểu để ra quyết định: 10 cặp hoàn chỉnh.** Dưới 10 là *bằng chứng không đủ*, không bao giờ là
một quyết định lấy trên 9.

**Seed smoke: `999979`.** Một seed riêng, manifest riêng, dùng để hiệu chỉnh harness. **Không bao giờ**
vào bất kỳ bảng, thống kê hay tuyên bố nào. Việc loại trừ phải **cơ học** như E2 — seed smoke không
nằm trong danh sách seed của hai manifest thí nghiệm, nên `run_manifest_seed` từ chối nó.

## 5. T, lấy mẫu, khởi động

Giữ nguyên lưới của E2 để hai thí nghiệm so được với nhau:

| tham số | giá trị | vì sao |
|---|---|---|
| `duration_ticks` (**T**) | **18.000** | 300 s mô phỏng ở 60 Hz; 300 lần nổ của band sinh thái (chu kỳ 60); 18 biên epoch |
| `sample_period` | **600** | 30 mẫu mỗi lượt chạy |
| khởi động | **3.000 tick đầu (5 mẫu)** | loại khỏi mọi thống kê trung bình theo thời gian |
| cửa sổ đuôi | tick **14.400–18.000** | dạng thứ cấp của metric chính, để kiểm điểm cuối không phải một lát cắt may mắn |
| dừng | **chân trời cố định** | không dừng sớm, không ngó trước, không "trông đã hội tụ" |

Một lưu ý riêng của E3, không có ở E2: `AddNutrient` rút từ pool **detritus**, mà detritus khởi đầu
bằng `0.0` (`core/ecs.rs:78`). Nên vế bón phân **không làm gì** cho tới khi có cái chết và phân huỷ
nạp detritus. Điều đó khiến 3.000 tick khởi động **không chỉ là quy ước** ở đây — nó là thời gian để
kênh nhân quả thực sự mở. Nếu lần chạy smoke cho thấy detritus vẫn ≈ 0 ở tick 3.000, **T và cửa sổ
khởi động phải được chọn lại trước khi bất kỳ seed thí nghiệm nào chạy**, và ghi vào `provenance.json`.

## 6. Metric

**Chính:** `live.trait_mass_var` — phương sai tổng thể của khối lượng cơ thể, tại T.

**Thứ cấp, khai trước:**

1. khoảng `live.trait_mass_p90 − live.trait_mass_p10` (độ trải robust, ít nhạy với đuôi hơn phương sai);
2. `live.trait_mass_mean` (để phân biệt "trải rộng" với "dịch cả khối");
3. `live.shannon` (gradient tạo ổ sinh thái thì đa dạng cũng phải nhúc nhích);
4. `live.archive_coverage` (nhánh Q: gradient có mở thêm ô trong lưới MAP-Elites không);
5. `live.mean_lifespan`, `live.births`, `live.deaths` (bối cảnh dân số — một hiệu ứng trên phương sai
   khối lượng đi kèm sụp dân số là một câu chuyện khác hẳn).

Toàn bộ 22 observable (12 cũ + 10 mở rộng) vẫn được báo cáo đầy đủ. Việc chỉ định chính/thứ cấp làm
**ở đây, trước khi có số**, và không được đổi sau.

## 7. Thống kê và ngưỡng "khác đủ nhiều"

Thống kê: `PairedEffect` do runner tính, y như E2 — `paired_mean_delta` (treatment − control) là hiệu
ứng chính, `paired_dz` là Cohen's *d_z*; `None` báo cáo là `None`, không bao giờ báo cáo là 0. Kèm
theo: **trung vị** của các delta theo seed, **phương sai giữa các seed**, và **bảng delta từng seed**.

Với metric chính, hiệu ứng được coi là **vật chất** chỉ khi **cả ba** cùng đúng:

1. khoảng tin cậy 95 % của `paired_mean_delta` **không chứa 0**;
2. `|paired_dz| ≥ 0,8`;
3. `|paired_mean_delta| ≥ 0,05 × |control_mean|`.

**Null:** CI chứa 0, **hoặc** dịch chuyển tương đối dưới 5 %.
**Ở giữa:** CI không chứa 0 nhưng trượt điều kiện 2 hoặc 3 ⇒ báo cáo là *phát hiện được nhưng dưới
ngưỡng vật chất đã đăng ký*, và đó **không** phải "khác đủ nhiều". Không nâng hạng khi khiếu nại.

**Không đổi metric.** Nếu H1 null mà một observable khác lại vật chất, điều đó được báo cáo đúng bản
chất: một quan sát **chưa đăng ký**, sinh giả thuyết, không phải một phát hiện.

## 8. Xử lý thất bại và ngân sách

- `drive()` dừng sớm và đánh dấu `Failed` khi một observable thành phi hữu hạn. Đó là **cơ chế dừng
  sớm duy nhất**, và nó tạo ra một thất bại được lưu lại, không phải một kết quả bị cắt cụt.
- Cặp nào có một vế `Failed` thì **bỏ cả cặp**, và liệt kê ra. 12 cặp yêu cầu để 2 cặp được phép hỏng.
- 24 lượt chạy × 18.000 tick. **`--release` là bắt buộc** — cùng lý do E2 §5.
- Một khuyết tật của harness **không bao giờ** được báo cáo thành "bằng chứng không đủ". Sửa khuyết
  tật, chạy lại, nói rõ đã chạy lại.

## 9. Việc phải làm trước khi đây thành tiền đăng ký

| # | Gói việc | Xong khi |
|---|---|---|
| WP-1 | Fixture JSON máy đọc được `src-tauri/tests/fixtures/experiments_e3/e3-preregistration.json` | có file, và một test kiểu `prereg_e2_manifest_tests.rs` khẳng định nó khớp tài liệu này |
| WP-2 | **Lượt chạy smoke ở seed 999979** | chứng minh được ba điều: (a) `ResourceField` có mặt trong thế giới headless, (b) hai can thiệp thật sự đổi `live.standing_crop` giữa hai arm, (c) `detritus` > 0 trước tick 3.000. Thiếu (b) thì nhân tố **không tồn tại** và cả thiết kế vô nghĩa |
| WP-3 | Chốt `T` và cửa sổ khởi động theo kết quả WP-2, ghi vào `provenance.json` | trước khi seed thí nghiệm đầu tiên chạy; sau đó không đổi |
| WP-4 | Đóng băng tài liệu này (`status: frozen`), ghi commit cha | không sửa nội dung sau khi seed đầu tiên chạy |
| WP-5 | Chạy ensemble 12 cặp, phân tích theo §7 | có `summary.md`, mọi con số truy được về một file trong thư mục artifact |

Thứ tự là bắt buộc. WP-2 có quyền **giết** thiết kế này: nếu can thiệp theo vùng không dịch được
`live.standing_crop`, thì E3 phải đợi dụng cụ ở §10 chứ không được chạy cho có.

## 10. Hai dụng cụ sẽ mở khoá câu hỏi gốc

Không thuộc phạm vi E3, ghi lại để không phải tìm lại:

1. **Observable khối lượng theo vùng** — ví dụ `live.trait_mass_mean.west` / `.east`, hoặc một
   observable nhận tham số vùng. Đây là thứ biến "phân tán" thành "phân hoá không gian", và là dụng
   cụ duy nhất cho phép hỏi đúng câu §10 của kế hoạch feature.
2. **Một can thiệp chạm `r_max`, không chỉ `r`** — hiện `strip_region`/`fertilise_region` đổi
   **standing crop**, còn sức chứa `r_max` (thứ mà `biome_npp` đặt ra) thì bất khả xâm phạm. Một
   `InterventionKind` đổi sức chứa theo vùng mới là "đổi NPP" đúng nghĩa; cái đang có là "đổi lượng
   cây đang đứng", và tăng trưởng logistic sẽ kéo nó về sức chứa cũ khi forcing tắt.

## 11. Ngôn ngữ mà thí nghiệm này bị ràng buộc

- "Không phát hiện được hiệu ứng" ≠ "không có hiệu ứng". Một kết quả null được viết là null.
- Mỗi con số trong báo cáo phải chỉ ra được file sinh ra nó.
- Lượt chạy hỏng được liệt kê, không được lặng lẽ bỏ.
- **Kết quả âm là một kết quả.** "Gradient năng suất không làm phân bố khối lượng rộng ra trong
  harness này" là một câu trả lời dùng được, và nó phải được viết ra y như một kết quả dương.
