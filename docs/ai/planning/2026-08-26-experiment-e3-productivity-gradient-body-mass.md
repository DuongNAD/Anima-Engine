---
phase: planning
feature: experiment-e3-productivity-gradient
title: Kế hoạch — E3, gradient năng suất theo không gian có làm phân tán khối lượng cơ thể không
description: Tiền đăng ký đóng băng E3 (WP-1..WP-4, 2026-11-19) — nhân tố, N=12 cặp, T=18.000, metric chính/thứ cấp gồm S2 theo vùng, ngưỡng vật chất, và kết quả smoke WP-2/WP-3
status: active
owner: simulation-architecture
last_reviewed: 2026-11-19
---

# Kế hoạch — E3: gradient năng suất theo không gian và phân tán khối lượng cơ thể

> 🔒 **TIỀN ĐĂNG KÝ ĐÃ ĐÓNG BĂNG (2026-11-19).** Hoàn thành đủ **WP-1 đến WP-4** trước khi chạy
> bất kỳ hạt giống thí nghiệm nào:
> - **Parent commit trên `feat/ecological-selection`:** `91def40db1148c4526230704e5f002fe61f77c24` (`91def40`, sau khi PR `#148` Card S2 hợp nhất).
> - **Phiên bản mô hình:** `LIVE_MODEL_VERSION = "live-bevy-world/2"` (`model_status`: `"headless adapter verified; NOT a live-world experiment-ready claim"`), snapshot schema `8`.
> - **Fixture máy đọc & test đối chiếu:** `src-tauri/tests/fixtures/experiments_e3/e3-preregistration.json` + các manifest điều khiển/điều trị/smoke, kiểm tra bởi `src-tauri/tests/prereg_e3_manifest_tests.rs`.
> - **Bằng chứng hiệu chỉnh mù (WP-2/WP-3):** `artifacts/experiments/e3-productivity-gradient-body-mass/smoke/` (`seed = 999979`, profile `--release`, `body_mass_consulted: false`).
> - **Số hạt giống thí nghiệm đã chạy tính tới lúc đóng băng:** **0** (`730001..741001` chưa từng chạy; `contains_results: false`).

## 1. Câu hỏi và những gì mã thật sự cho phép đo (đã đối chiếu với `91def40`)

Câu hỏi sinh học gốc (kế hoạch feature §10): **một gradient năng suất sơ cấp theo không gian có sinh ra các loại khối lượng cơ thể khác nhau ở vùng giàu và vùng nghèo không?**

Đọc mã ở bản nháp `445c9c8` và đối chiếu lại với trunk `91def40` cho thấy phạm vi **nhân tố** và **dụng cụ đo** cụ thể như sau:

| Hạng mục | Bằng chứng trong mã (`91def40`) | Hệ quả cho E3 |
|---|---|---|
| **Chưa có núm nào chỉnh `r_max` (NPP) theo biome** | `biome_npp()` (`core/ecology.rs:63`) là hàm hằng của `BiomeType`; `ResourceField::from_biomes` đọc nó ở `core/ecs.rs:65`; bản đồ biome đến từ `TerrainMap::load_or_generate(&MapSettings::default(), …)` với `seed: 1337` cố định (`core/terrain.rs:99`) | Gradient NPP nền theo biome là **hằng số của thế giới**, giống hệt ở mọi arm và mọi seed. Không thể dùng `biome_npp` hay `r_max` làm nhân tố |
| **Đã có observable khối lượng theo vùng (Card S2, PR `#148`)** | `LIVE_EXTENDED_OBSERVABLE_IDS` (`core/live_experiment.rs:1428`, trước đây là `:857`) đã bổ sung `live.trait_mass_mean_west` (cột `0..=127`) và `live.trait_mass_mean_east` (cột `128..=255`) bên cạnh `live.trait_mass_{mean,var,p10,p90}` toàn quần thể | Ở bản nháp `445c9c8`, mục #1 ở [§10](#10-hai-dụng-cụ-sẽ-mở-khoá-câu-hỏi-gốc) còn thiếu nên E3 chỉ đặt giả thuyết chính trên **độ phân tán toàn cục (`live.trait_mass_var`)**. Trên `91def40`, S2 đã giao bộ đo theo vùng **trước khi có dữ liệu**: E3 giữ `live.trait_mass_var` làm metric chính `H1` cho phân tán và đăng ký trước bộ đôi Tây/Đông làm metric thứ cấp `S3` |

Cái **có sẵn** cho phép áp forcing theo vùng một cách cơ học và khép kín EU:

- `InterventionCommand` mang `region: Region::Rect{…}`, `duration_ticks` và `curve`
  (`crates/anima-domain/src/intervention.rs:143`, bản nháp cũ ghi `:105`) — tức là một forcing
  **theo vùng và kéo dài**, chứ không phải một cú sốc một tick.
- `InterventionKind::Deforest` gọi `strip_region`, `AddNutrient` gọi `fertilise_region`
  (`core/live_experiment.rs:1214, 1236`, bản nháp cũ ghi `:597, 627`). Cả hai đi qua `EnergyLedger::transfer`
  nên **EU vẫn đóng**: deforest chuyển standing crop → detritus, add-nutrient rút từ **chính pool
  detritus đó** ra trở lại standing crop, trần mỗi lần band sinh thái nổ là
  `NUTRIENT_FULL_SCALE_FRACTION = 0.10` (`core/live_experiment.rs:80, 1029`).
- Thế giới headless **có** trường tài nguyên: `build_live_world` gọi `core::ecs::init_world()` ở đầu
  hàm (`core/live_experiment.rs:2359`, bản nháp cũ ghi `:1410`), và `init_world` gắn `ResourceField`
  cùng `EcosystemBiomass`. Kết quả chạy smoke WP-2 ở [§5](#5-t-lấy-mẫu-khởi-động-và-hiệu-chỉnh-wp-2--wp-3)
  xác nhận tận nơi rằng `Deforest` + `AddNutrient` thực sự làm dịch `live.standing_crop` giữa hai arm.

**Vì vậy E3 đổi nhân tố nhưng giữ nguyên câu hỏi sinh học.** Thay vì gradient NPP `r_max` theo biome
(chưa chỉnh được bằng can thiệp), E3 áp một **gradient năng suất hữu dụng theo không gian bằng can thiệp
lên standing crop**: bỏ rừng nửa Tây (`Deforest`), bón nửa Đông (`AddNutrient`), kéo dài suốt lượt chạy.
Đó là cùng một loại nhân quả — một nửa thế giới duy trì được lượng thức ăn thực vật sẵn có khác nửa kia —
chỉ khác đường đặt vào.

> 🔬 **Giới hạn được ghi rõ trước khi có số.** Metric chính `live.trait_mass_var` đo **phân tán toàn cục**
> (kèm `p90 − p10`), còn metric thứ cấp `S3` (`live.trait_mass_mean_west` / `live.trait_mass_mean_east`,
> tương phản `east − west`) đo **phân hoá khối lượng trung bình theo hai nửa không gian**. Cả hai được
> chốt trước khi bất kỳ hạt giống thí nghiệm nào chạy; không có chuyện suy diễn "phân hoá vùng" chỉ từ
> variance toàn cục, và cũng không thay thế nhân tố còn thiếu #2 (chỉnh `r_max` theo vùng ở [§10](#10-hai-dụng-cụ-sẽ-mở-khoá-câu-hỏi-gốc)).

## 2. Giả thuyết

- **H1 (chính):** dưới cùng luật fitness sinh thái và chu chuyển quần thể, một gradient năng suất theo
  không gian (Tây `Deforest 0.30`, Đông `AddNutrient 1.00`) làm **tăng** độ phân tán khối lượng cơ thể
  của quần thể (`live.trait_mass_var` tại `T`, `paired_mean_delta > 0`) so với thế giới có hai lệnh
  can thiệp đồng nhất ở cường độ `0.0`.
- **H0:** không tăng — khoảng tin cậy 95 % của `paired_mean_delta` phủ 0, hoặc dịch chuyển tương đối
  dưới 5 %.

Hướng của H1 được khai **trước**. Nếu kết quả có ý nghĩa vật chất nhưng **ngược hướng** (`paired_mean_delta < 0`,
tức gradient làm phân bố khối lượng **hẹp** lại), đó là một kết quả thật, được báo cáo là bác bỏ H1 theo
hướng ngược, **không** được viết lại thành ủng hộ H1.

## 3. Thiết kế: hai arm ghép cặp theo seed

Cùng manifest, cùng seed, khác đúng **một** nhân tố — `interventions` (cường độ can thiệp).

| | Control (`e3-control-uniform.json`) | Treatment (`e3-treatment-gradient.json`) |
|---|---|---|
| Can thiệp 1 (`id=1, cause_id=1`) | `Deforest`, `Rect{min_x:0, min_y:0, max_x:127, max_y:255}` (nửa Tây), `intensity = 0.0` | như control nhưng `intensity = 0.30` |
| Can thiệp 2 (`id=2, cause_id=2`) | `AddNutrient`, `Rect{min_x:128, min_y:0, max_x:255, max_y:255}` (nửa Đông), `intensity = 0.0` | như control nhưng `intensity = 1.00` |
| `start_tick` / `duration_ticks` | `1` / `18000` (suốt lượt chạy) | giống hệt |
| `curve` | `Step` | giống hệt |
| `reversible` / `signed_negative` | `false` / `false` | giống hệt |

Control **giữ nguyên hai lệnh** với `intensity = 0.0` thay vì bỏ chúng đi. `validate_intervention`
(`core/experiment/interventions.rs:16`) chấp nhận `intensity == 0.0` (chỉ từ chối số âm hoặc phi hữu hạn),
và giữ lệnh lại nghĩa là hai arm đi qua **cùng một đường mã**, chỉ khác độ lớn. Bỏ lệnh đi sẽ biến
"cường độ" và "có hay không lệnh can thiệp" thành hai nhân tố lẫn vào nhau.

**Hằng số của cả hai arm (12 khoá khai tường minh trong `initial_conditions`, đối chiếu `91def40`):**

Vì bản đồ `LIVE_KEYS` đã tăng từ 11 lên 25 khoá kể từ `445c9c8` (`core/live_experiment.rs:218`), mọi khoá
mới quyết định khả năng sống sót, di chuyển và sinh sản của quần thể phải được **ghim tường minh** trước
khi chạy để hai arm hoàn toàn giống nhau và tránh các mặc định tắt locomotion/reproduction:

| Khoá | Giá trị ghim | Thay đổi so với bản nháp `445c9c8` & lý do |
|---|---|---|
| `live.ecological_fitness` | `1.0` | Giữ nguyên từ bản nháp; ghim hợp đồng nhánh F (lưu ý kỹ thuật ở `91def40`: trong `LiveExperimentAdapter`, chu chuyển sinh sản chạy qua `autonomous_reproduction_system`, còn `check_epoch_completion_system` chỉ rút hàng đợi rỗng ở headless) |
| `live.mortality` | `1.0` | Giữ nguyên từ bản nháp; bật cái chết (ADR-0005) để áp lực chọn lọc có răng |
| `live.reproduction` | `1.0` | **Mới so với 08-26 (Card S1 / ADR-0005):** mặc định của `LiveWorldConfig` là `false`; bắt buộc `1.0` để có sinh sản và `live.births` thực sự |
| `live.locomotion` | `1.0` | **Mới so với 08-26 (ADR-0006 / F-042):** mặc định là `false`; bật di chuyển để sinh vật kiếm ăn và đi tìm nước |
| `live.drive_floor` | `0.5` | **Mới so với 08-26 (ADR-0007 mục A / E-SEL1):** mặc định là `0.0`; tránh sinh vật đứng yên khi chưa dùng não tiến hoá |
| `live.thirst_steering` | `1.0` | **Mới so với 08-26 (ADR-0007 mục A / E-SEL1):** mặc định là `false`; cần để sinh vật hướng về hồ nước gần nhất thay vì chết khát đồng loạt |
| `live.evolved_brains` | `0.0` | Ghim tường minh tắt não tiến hoá (`E2` khảo sát riêng) để cách ly nhân tố sinh thái |
| `live.founders` | `120.0` | Giữ nguyên từ bản nháp: đủ lớn cho phương sai khối lượng, đủ nhỏ cho 24 lượt chạy trong 90 phút |
| `live.predator_fraction` | `0.3` | Chuyển từ ghi "mặc định" sang ghim tường minh `0.3`, khớp `LiveWorldConfig::default()` (`core/live_experiment.rs:351`) |
| `live.trees` | `8.0` | Ghim tường minh, khớp `LiveWorldConfig::default()` (`core/live_experiment.rs:351`) |
| `live.lakes` | `2.0` | Ghim tường minh, khớp `LiveWorldConfig::default()` (`core/live_experiment.rs:351`) |
| `live.food_cap` | `50.0` | Ghim tường minh, khớp `LiveWorldConfig::default()` (`core/live_experiment.rs:351`) |

**13 khoá `LIVE_KEYS` còn lại giữ nguyên mặc định của `LiveWorldConfig::default()` (vắng mặt trong manifest, đối chiếu `core/live_experiment.rs:351`):**
`live.w_surplus = 1.0`, `live.starve_ticks = 120`, `live.dehydration_ticks = 120`, `live.mutation_rate = 0.05`,
`live.prey_cap = 80` (Card S6), `live.resource_capacity_scale = 1.0` (Card S6), `live.plant_reserve = 0.0`
(Card S10), `live.lake_layout = 0` (Card S12, bố trí hàng ngang ở `z_fraction = 0.8`), `live.kill_hydration = 0.0`,
`live.kill_efficiency = 0.40`, `live.target_min_energy = 0.0` (ADR-0007 mục B), `live.drift_control = 0.0`,
`live.drift_reset_ticks = 0` (SEL2-D). Địa hình cố định ở `MapSettings::default()`, `seed: 1337`,
`world_identity`: `(1337, generator_version=1, 256x256, terrain_checksum=3152323152)`.

## 4. N và các seed

**N = 12 cặp**, seed theo quy tắc số học khai trước — cùng lý do như E2 (`seed_k = 730001 + 1000·k`,
`k ∈ 0..11`), theo thứ tự thực thi:

```
730001  731001  732001  733001  734001  735001
736001  737001  738001  739001  740001  741001
```

**Tối thiểu để ra quyết định: 10 cặp hoàn chỉnh.** Dưới 10 là *bằng chứng không đủ*, không bao giờ là
một quyết định lấy trên 9.

**Seed smoke: `999979`.** Một seed riêng, manifest riêng (`e3-smoke-control-uniform.json`,
`e3-smoke-treatment-gradient.json`), dùng để hiệu chỉnh harness ở WP-2/WP-3. **Không bao giờ** vào bất kỳ
bảng, thống kê hay tuyên bố kết quả nào của E3. Việc loại trừ được đảm bảo **cơ học**: `999979` không có
trong `seeds` của `e3-control-uniform.json` và `e3-treatment-gradient.json`, nên `run_manifest_seed` trả về
`RunStatus::Failed` (`SeedNotInManifest`) nếu bị gọi nhầm với manifest thí nghiệm.

## 5. T, lấy mẫu, khởi động và hiệu chỉnh WP-2 / WP-3

Lịch chạy mục tiêu và thang dự phòng WP-3 (`duration_ticks_ladder = [18000, 12000, 6000]`):

| tham số | giá trị đã chốt (WP-3) | vì sao |
|---|---|---|
| `duration_ticks` (**T**) | **18.000** | 300 s mô phỏng ở 60 Hz; 300 lần nổ của band sinh thái (chu kỳ 60); 18 biên epoch |
| `sample_period` | **600** | 30 mẫu mỗi lượt chạy |
| khởi động | **3.000 tick đầu (5 mẫu)** | loại khỏi mọi thống kê trung bình theo thời gian |
| cửa sổ đuôi | tick **14.400–18.000** | dạng thứ cấp của metric chính, để kiểm điểm cuối không phải một lát cắt may mắn |
| dừng | **chân trời cố định** | không dừng sớm, không ngó trước, không "trông đã hội tụ" |

### Bằng chứng WP-2 & chốt lịch WP-3 ở seed smoke `999979` (`--release`, parent `91def40`)

Một lưu ý riêng của E3: `AddNutrient` rút từ pool **detritus**, mà detritus khởi đầu bằng `0.0`
(`core/ecs.rs:78`). WP-2 đã chạy mù hoàn toàn (`body_mass_consulted: false` — không đọc hay ghi bất kỳ
giá trị `live.trait_mass_*` nào) qua `src-tauri/examples/e3_smoke.rs` và lưu ở
`artifacts/experiments/e3-productivity-gradient-body-mass/smoke/`:

- **(a) `ResourceField` có mặt trong thế giới headless — ĐẠT:** lưới `256×256` (`x ∈ [-100.0, 100.0]`,
  `z ∈ [-100.0, 100.0]`), tổng sinh khối khởi đầu bằng sức chứa = `153587.24986362457 EU` (nửa Tây
  `col < 128` = `88757.74991834164 EU`, nửa Đông `col >= 128` = `64829.499945282936 EU`),
  `EcosystemBiomass.detritus = 0.0` trước tick 1.
- **(b) Hai can thiệp thực sự đổi `live.standing_crop` giữa hai arm — ĐẠT (công tắc huỷ WP-2 KHÔNG kích hoạt):**
  - Control: `standing_crop` tại `T = 18000` là `123300.83728024106` (trung bình sau khởi động `tick > 3000`
    là `134866.6595674591`, trung bình đuôi `14400..=18000` là `124085.04243589146`, checksum
    `1266395716`, `0` mục sổ cái `Deforest`, `0` mục `AddNutrient`).
  - Treatment: `standing_crop` tại `T = 18000` là `63282.05338941543` (`delta = -60018.78389082563`),
    trung bình sau khởi động `64098.52086286802` (`delta = -70768.13870459108`, tương đương `-52.47 %`),
    trung bình đuôi `63381.91691425673` (`delta = -60703.12552163473`), độ lệch tuyệt đối cực đại theo
    mẫu `86992.33749733593`, checksum `2891653359`. Sổ cái ghi nhận đúng `300` lần `Deforest` chuyển
    `93891.59735265336 EU` và `300` lần `AddNutrient` bù `33642.28750495448 EU`.
- **(c) `detritus > 0` trước tick 3.000 — ĐẠT:** mẫu dương đầu tiên xuất hiện ngay ở **tick 600** trong cả hai
  arm (`1647.807546613461 EU` ở control, `86709.16853561654 EU` ở treatment); tại tick `3000` là
  `12530.86161665315 EU` (control) và `97795.36642170392 EU` (treatment). Kênh nhân quả mở sớm trong
  cửa sổ khởi động `3000`.
- **Bảo toàn & toàn vẹn:** độ trôi EU kín cực đại chỉ `1.076841726899147e-9` (control) và
  `4.656612873077393e-10` (treatment); toàn bộ 27 observable đều hữu hạn và được phát ra; ở seed smoke
  `999979`, số cá thể dao động trong khoảng `13..193` (cuối `13`, `375` sinh, `482` chết) ở control và
  `9..192` (cuối `99`, `370` sinh, `391` chết) ở treatment.
- **Chốt WP-3 từ đồng hồ `smoke/provenance.json`:** 2 lượt chạy × `18000` tick hết `11.6401228 s`
  (`323.3367444444445 us/tick`), ngoại suy cho ensemble 24 × `18000` = **`2.32802456 phút`** (`<< 90 phút`).
  Do đó chốt **`T = 18000`** và **`warmup_ticks = 3000`** (`sample_period = 600`, `tail_window = 14400..18000`)
  vào `2026-11-19` với **`0` seed thí nghiệm đã chạy**.

## 6. Metric

**Chính (`H1`):** `live.trait_mass_var` — phương sai tổng thể của khối lượng cơ thể, tại `T = 18000`
(kèm kiểm tra thứ cấp trên cửa sổ đuôi `14400..18000`).

**Thứ cấp, khai trước (khớp `S1`–`S6` trong `e3-preregistration.json`):**

1. `S1`: khoảng `live.trait_mass_p90 − live.trait_mass_p10` (độ trải robust, ít nhạy với đuôi hơn phương sai);
2. `S2`: `live.trait_mass_mean` (để phân biệt "trải rộng" với "dịch cả khối");
3. `S3` (**khai từ Card S2 PR `#148`, trước mọi dữ liệu**): `live.trait_mass_mean_west` (cột `0..=127`) và
   `live.trait_mass_mean_east` (cột `128..=255`), cùng hiệu ghép cặp `live.trait_mass_mean_east − live.trait_mass_mean_west`
   (hai phía, để đo trực tiếp phân hoá khối lượng giữa nửa Tây bị chặt và nửa Đông được bón);
4. `S4`: `live.shannon` (gradient tạo ổ sinh thái thì đa dạng cũng phải nhúc nhích);
5. `S5`: `live.archive_coverage` (nhánh Q: gradient có mở thêm ô trong lưới MAP-Elites không);
6. `S6`: `live.mean_lifespan`, `live.births`, `live.deaths` (bối cảnh dân số — một hiệu ứng trên phương sai
   khối lượng đi kèm sụp dân số là một câu chuyện khác hẳn).

Ngoài ra, `I1` (`live.closed_eu_total`) kiểm tra bảo toàn EU kín và `M1` (`live.standing_crop`) kiểm tra
thao túng thực địa.

Toàn bộ **27 observable** hiện hành (**12** cốt lõi + **15** mở rộng, tăng từ 22 ở bản nháp `445c9c8` do
S9 bổ sung `live.deaths_starvation`, `live.deaths_dehydration` và S2 bổ sung `live.mean_energy_integral`,
`live.trait_mass_mean_west`, `live.trait_mass_mean_east`) vẫn được yêu cầu và báo cáo đầy đủ. Việc chỉ
định chính/thứ cấp làm **ở đây, trước khi có số**, và không được đổi sau.

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
chất: một kết quả thứ cấp đã đăng ký (nếu nằm trong `S1`–`S6`), hoặc một quan sát **chưa đăng ký** sinh
giả thuyết, không phải thay thế H1.

## 8. Xử lý thất bại và ngân sách

- `drive()` dừng sớm và đánh dấu `Failed` khi một observable thành phi hữu hạn. Đó là **cơ chế dừng
  sớm duy nhất**, và nó tạo ra một thất bại được lưu lại, không phải một kết quả bị cắt cụt.
- Cặp nào có một vế `Failed` thì **bỏ cả cặp**, và liệt kê ra. 12 cặp yêu cầu để 2 cặp được phép hỏng.
- 24 lượt chạy × 18.000 tick. **`--release` là bắt buộc** — cùng lý do E2 §5 (ước tính WP-3: `2.33` phút << `90` phút).
- Một khuyết tật của harness **không bao giờ** được báo cáo thành "bằng chứng không đủ". Sửa khuyết
  tật, chạy lại, nói rõ đã chạy lại.

## 9. Việc phải làm trước khi đây thành tiền đăng ký

| # | Gói việc | Trạng thái (`2026-11-19`) | Bằng chứng / Điều kiện xong |
|---|---|---|---|
| WP-1 | Fixture JSON máy đọc được & test đối chiếu | **XONG** (`2026-11-19`) | `src-tauri/tests/fixtures/experiments_e3/{e3-preregistration.json, e3-control-uniform.json, e3-treatment-gradient.json, e3-smoke-control-uniform.json, e3-smoke-treatment-gradient.json}` và `src-tauri/tests/prereg_e3_manifest_tests.rs` |
| WP-2 | **Lượt chạy smoke ở seed 999979** | **XONG** (`2026-11-19`) | `src-tauri/examples/e3_smoke.rs` → `artifacts/experiments/e3-productivity-gradient-body-mass/smoke/`: (a) `ResourceField` `256×256` có mặt, (b) `live.standing_crop` đổi `-52.47 %` sau khởi động, (c) `detritus > 0` từ tick `600` (`< 3000`), `body_mass_consulted: false` |
| WP-3 | Chốt `T` và cửa sổ khởi động từ WP-2 | **XONG** (`2026-11-19`) | Ghi `wp3_locked_schedule` (`T = 18000`, `warmup_ticks = 3000`) trong `e3-preregistration.json` và `smoke/provenance.json` (`2.33` phút dự kiến << `90` phút) với `0` seed thí nghiệm |
| WP-4 | Đóng băng tài liệu này, ghi commit cha | **XONG** (`2026-11-19`) | `status: active` hợp lệ chính sách (không dùng `status: frozen` để `check:docs` xanh), ghi commit cha `91def40db1148c4526230704e5f002fe61f77c24`, đồng bộ toàn bộ khoá/ID/mặc định với trunk và không sửa sau khi seed đầu tiên chạy |
| WP-5 | Chạy ensemble 12 cặp, phân tích theo §7 | **Chưa chạy (`E3-X`)** | `0` seed trong `730001..741001` đã chạy ở card `E3-P`; thực thi ở card riêng `E3-X` |

Thứ tự là bắt buộc. WP-2 có quyền **giết** thiết kế này nếu can thiệp không dịch nổi `live.standing_crop`;
kết quả WP-2 ở trên cho thấy kênh nhân quả **hoạt động**, nên tiền đăng ký được phép chuyển sang WP-5 (`E3-X`).

## 10. Hai dụng cụ sẽ mở khoá câu hỏi gốc

Không thay đổi tiền đề rằng nhân tố E3 đánh vào standing crop chứ chưa đánh vào `r_max`:

1. **Observable khối lượng theo vùng — ĐÃ GIAO bởi Card S2 (PR `#148`):** `live.trait_mass_mean_west`
   (cột `0..=127`) và `live.trait_mass_mean_east` (cột `128..=255`) tại `core/live_experiment.rs:1428`.
   Được đăng ký làm metric thứ cấp `S3` trong tiền đăng ký đóng băng này trước khi có bất kỳ dữ liệu nào.
2. **Một can thiệp chạm `r_max`, không chỉ `r` — VẪN CÒN THIẾU:** hiện `strip_region`/`fertilise_region`
   đổi **standing crop**, còn sức chứa `r_max` (thứ mà `biome_npp` đặt ra) thì bất khả xâm phạm. Một
   `InterventionKind` đổi sức chứa theo vùng mới là "đổi NPP" đúng nghĩa; cái đang có là "đổi lượng
   cây đang đứng", và tăng trưởng logistic kéo nó về sức chứa cũ khi forcing tắt (trong E3 forcing
   kéo dài `1..=18000`).

## 11. Ngôn ngữ mà thí nghiệm này bị ràng buộc

- "Không phát hiện được hiệu ứng" ≠ "không có hiệu ứng". Một kết quả null được viết là null.
- Mỗi con số trong báo cáo phải chỉ ra được file sinh ra nó.
- Lượt chạy hỏng được liệt kê, không được lặng lẽ bỏ.
- **Kết quả âm là một kết quả.** "Gradient năng suất không làm phân bố khối lượng rộng ra trong
  harness này" là một câu trả lời dùng được, và nó phải được viết ra y như một kết quả dương.
