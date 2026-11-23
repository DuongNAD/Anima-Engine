---
phase: planning
feature: experiment-e3-productivity-gradient
title: Kế hoạch — E3, gradient năng suất theo không gian có làm phân tán khối lượng cơ thể không
description: Tiền đăng ký đóng băng E3 fix round 3 (3 arm ghép cặp gồm đối chứng khan hiếm đồng đều tổng ~64k EU, WP-1..WP-4 trên trunk 9958f27, loại trừ per-contrast, nhạy Bessel H1, S4/S5 structural zero, ngưỡng vật chất máy đọc)
status: active
owner: simulation-architecture
last_reviewed: 2026-11-19
---

# Kế hoạch — E3: gradient năng suất theo không gian và phân tán khối lượng cơ thể

> 🔒 **TIỀN ĐĂNG KÝ ĐÃ ĐÓNG BĂNG — BẢN FIX ROUND 3 (2026-11-19).** Hoàn thành đủ **WP-1 đến WP-4** trước
> khi chạy bất kỳ hạt giống thí nghiệm nào:
> - **Parent commit hiện tại trên `feat/ecological-selection`:** `9958f27bdf5f979f03aeee213c8d0b5304566cf2` (`9958f27`; đóng băng lần đầu ở `91def40db1148c4526230704e5f002fe61f77c24`, bản nháp ở `445c9c8`).
> - **Phiên bản mô hình:** `LIVE_MODEL_VERSION = "live-bevy-world/2"` (`model_status`: `"headless adapter verified; NOT a live-world experiment-ready claim"`), snapshot schema `8`.
> - **Thiết kế 3 arm (giải quyết nhiễu tổng lượng thức ăn ở Fix Round 1):** `control_uniform` (đồng đều dồi dào ~153.6k EU), **`control_uniform_scarcity` (đối chứng chính mới — đồng đều khan hiếm ~64.5k EU, `live.resource_capacity_scale = 0.42` + `AddNutrient 1.00` toàn bản đồ)**, và `treatment_gradient` (~64.1k EU sau khởi động, Tây chặt 0.30 / Đông bón 1.00). Tổng `36` lượt chạy (`3` arm × `12` seed).
> - **Fixture máy đọc & test đối chiếu:** `src-tauri/tests/fixtures/experiments_e3/e3-preregistration.json` + 6 manifest, kiểm tra bởi `src-tauri/tests/prereg_e3_manifest_tests.rs`.
> - **Bằng chứng hiệu chỉnh mù (WP-2/WP-3):** `artifacts/experiments/e3-productivity-gradient-body-mass/smoke/` (`seed = 999979`, profile `--release`, `body_mass_consulted: false`).
> - **Số hạt giống thí nghiệm đã chạy tính tới lúc đóng băng:** **0** (`730001..741001` chưa từng chạy; `contains_results: false`).

## 1. Câu hỏi, giới hạn cơ chế và lựa chọn giải quyết nhiễu tổng năng suất (Design Confound)

Câu hỏi sinh học gốc (kế hoạch feature §10): **tại cùng tổng lượng thức ăn thực vật sẵn có, một gradient năng suất sơ cấp theo không gian có làm phân tán khối lượng cơ thể và phân hoá kích thước giữa nửa giàu và nửa nghèo không?**

Đối chiếu mã trên trunk `9958f27` cho thấy phạm vi **nhân tố** và **dụng cụ đo**:

| Hạng mục | Bằng chứng trong mã (`9958f27`) | Hệ quả cho E3 |
|---|---|---|
| **Chưa có núm chỉnh `r_max` theo vùng bằng can thiệp** | `biome_npp()` (`core/ecology.rs:63`) là hằng số theo `BiomeType`; `ResourceField::from_biomes` (`core/ecs.rs:65`) khởi tạo mọi ô **đúng bằng sức chứa `r = r_max`** (`153.587,25 EU`); bản đồ đến từ `MapSettings::default()`, `seed: 1337` (`core/terrain.rs:99`). Hiện chỉ có `live.resource_capacity_scale` nhân đều toàn bản đồ lúc khởi sinh (`core/live_experiment.rs:2568`) | `fertilise_region` (`core/live_experiment.rs:1236-1269`, trần tại `field.r_max[i]`) **không bao giờ đẩy ô vượt quá `r_max`**. Nếu chỉ dùng 2 arm không sửa `src-tauri/src/`, mọi cách giữ tổng năng suất bằng nhau bằng cách "bù Đông cao hơn sức chứa gốc" đều **bất khả thi về mặt cơ học** |
| **Can thiệp đứng trên standing crop đóng EU** | `InterventionCommand` (`crates/anima-domain/src/intervention.rs:143`), `strip_region` / `fertilise_region` (`core/live_experiment.rs:1214, 1236`), trần `NUTRIENT_FULL_SCALE_FRACTION = 0.10` (`core/live_experiment.rs:80`) | Tây `Deforest 0.30` + Đông `AddNutrient 1.00` làm nửa Đông giữ gần mức sức chứa (`~64,8k EU`), nửa Tây tụt gần `0`, và **tổng `standing_crop` toàn bản đồ giảm `-52,47 %`** so với đối chứng không chặt (`64.099` vs `134.867 EU`) |
| **Đã có observable khối lượng theo vùng (Card S2, PR `#148`)** | `LIVE_EXTENDED_OBSERVABLE_IDS` (`core/live_experiment.rs:1428`) có đủ `live.trait_mass_mean_{west,east}` (cột `0..=127` vs `128..=255`) cùng `live.trait_mass_{mean,var,p10,p90}` | `H1` đo phân tán toàn cục (`live.trait_mass_var`), còn `S3` đo trực tiếp tương phản không gian `east − west` |

### Lựa chọn thiết kế giải quyết nhiễu tổng lượng thức ăn (ghi rõ cho owner duyệt)

Trong bản nháp 2 arm ban đầu, một khác biệt ở `live.trait_mass_var` giữa `treatment_gradient` (~64,1k EU) và `control_uniform` (~134,9k EU) bị **lẫn hoàn toàn giữa gradient không gian và khan hiếm thức ăn toàn cục (~−52 %)** (nhân tố của E-SEL1). Vì không thể đẩy nửa Đông vượt quá `r_max` nếu không sửa `src-tauri/src/`, E3 Fix Round 1 **chọn Phương án B: bổ sung arm thứ 3 — `control_uniform_scarcity` (`e3-control-uniform-scarcity.json`) — làm đối chứng chính khớp tổng năng suất**, thay vì chỉ giữ 2 arm bị nhiễu:

1. **Đặt `live.resource_capacity_scale = 0.42` từ tỉ lệ hình học giải tích (sau quan sát smoke round 0, mù hoàn toàn với khối lượng cơ thể):**
   Sau khi quan sát mức `standing_crop` và `agent_count` của lượt smoke round 0 (công khai trong round-0 verdict; **không có bất kỳ giá trị `trait_mass_*` nào được xem hay tham khảo**, `body_mass_consulted: false`), Fix Round 1 tính tỉ lệ hình học tiên nghiệm của tài nguyên khởi sinh: `East_capacity / Total_capacity = 64.829,50 / 153.587,25 = 0,4221`, làm tròn hai chữ số thành **`0,42`**. Tại khởi sinh (`scale_pasture_capacity`, `core/live_experiment.rs:2568`), `control_uniform_scarcity` giữ đúng **`64.506,64 EU`** thực vật (đồng đều `0,42×` trên cả nửa Tây lẫn nửa Đông) và chuyển phần dư `89.080,61 EU` sang `detritus`, bảo toàn cùng một `closed_eu_total = 153.587,25 EU`.
2. **Bật `AddNutrient 1.00` đồng đều toàn bản đồ (`Rect{0,0,255,255}`) với `Deforest 0.0`:**
   Xuất phát từ quan sát dân số ở lượt smoke round 0 (`control_uniform` không bón thêm nên tụt còn `13` cá thể tại `T` so với `99` ở `treatment_gradient`; không đụng khối lượng cơ thể): `AddNutrient 1.00` toàn bản đồ giúp `control_uniform_scarcity` có cùng vòng tái nạp detritus→cỏ sau khi gặm với nửa Đông của arm điều trị. Trong smoke 3 arm, độ lệch `live.standing_crop` chỉ là **`−0,44 %`** trung bình sau khởi động theo `(T − S)/T` (`−280,49 / 64.098,52 EU = −0,4376 %`, khớp `scarcity_vs_treat_post_warmup_relative_mismatch` trong `smoke-report.json`; nếu chia `S` là `−0,4357 %`, cùng làm tròn `−0,44 %`), **`−1,86 %`** tại `T = 18.000` theo `(T − S)/S` (`63.282,05` vs `64.483,46 EU`), và **`−1,68 %`** ở trung bình cửa sổ đuôi `14400..=18000` theo `(T − S)/S` (`63.381,92` vs `64.467,85 EU`) — giữ khác biệt không gian thuần tuý ở sự phân cực Tây nghèo / Đông giàu (~`2,38×` mật độ cục bộ ở Đông so với `control_uniform_scarcity`, ~`0×` ở Tây).
3. **Giữ lại `control_uniform` để phân tích tách thành phần (phân rã cộng tính):**
   Với cùng bộ `N = 12` seed chạy trên 3 arm (`36` lượt chạy), trên tập các seed hợp lệ ở cả ba arm, phân rã cộng tính chính xác:
   `(T − U) = (T − S) + (S − U)`, trong đó `T − S` cô lập **gradient không gian tại cùng tổng ~64k EU**, còn `S − U` đo **hiệu ứng khan hiếm + tái nạp đồng đều (`AddNutrient 1.00` toàn bản đồ, điều mà `control_uniform` không có)** (khớp JSON `secondary_contrasts[1]`).

> 🔬 **Giới hạn còn lại được ghi rõ trước khi có số:**
> - Trong `treatment_gradient`, nửa Tây bị kéo xuống bằng `Deforest 0.30` lặp lại mỗi 60 tick trong khi `control_uniform_scarcity` thu hẹp `r_max` ngay lúc khởi sinh bằng `live.resource_capacity_scale = 0.42`. Cho đến khi nhánh mô phỏng có `InterventionKind` chỉnh `r_max` theo vùng (§10 #2), sự khác nhau giữa "áp lực chặt liên tục vs tỷ lệ `r_max` lúc khởi sinh" vẫn là giới hạn cơ chế của thiết kế này và được tuyên bố công khai.
> - Kết luận `H1` của E3 phải được phát biểu đúng nguyên văn theo nhân tố thực sự can thiệp: *"gradient năng suất tạo bởi Tây chặt 0.30 / Đông bón 1.00 so với đối chứng khan hiếm đồng đều ở cùng tổng ~64k EU"*.

## 2. Giả thuyết

- **H1 (chính — tương phản `treatment_gradient − control_uniform_scarcity`):** dưới cùng luật fitness, chu chuyển quần thể và **tổng lượng thức ăn thực vật tương đương (~64k EU)**, gradient năng suất theo không gian (Tây `Deforest 0.30`, Đông `AddNutrient 1.00`) làm **tăng** phương sai khối lượng cơ thể (`live.trait_mass_var` tại `T = 18.000`, `paired_mean_delta > 0`) so với đối chứng khan hiếm đồng đều (`live.resource_capacity_scale = 0.42`, `AddNutrient 1.00` toàn bản đồ).
- **H0:** không tăng trên tương phản chính — khoảng tin cậy 95 % của `paired_mean_delta` phủ 0, hoặc không đạt đồng thời hai ngưỡng vật chất (`|paired_dz| ≥ 0,8` và `|paired_mean_delta| ≥ 0,05 × |control_mean|`).
- **Tương phản tham chiếu bổ trợ (đăng ký trước trong E3-X):** `treatment_gradient − control_uniform` (tổng tác động) và `control_uniform_scarcity − control_uniform` (khan hiếm đồng đều cộng tái nạp `AddNutrient 1.00` toàn bản đồ; `control_uniform` không có tái nạp). Quy tắc loại trừ áp dụng **theo từng tương phản (`per_contrast`)** nên `H1` (`T − S`) chỉ tính các lượt `S` và `T`, còn bảng phân rã cộng tính `(T − U) = (T − S) + (S − U)` được báo cáo trên các seed hợp lệ ở cả ba arm.

Hướng của H1 được khai **trước**. Nếu `H1` đạt ngưỡng vật chất nhưng **ngược hướng** (`paired_mean_delta < 0`, tức `material_negative_opposite`), kết quả được báo cáo là bác bỏ H1 theo hướng ngược, **không** bao giờ viết lại thành ủng hộ H1.

## 3. Thiết kế: ba arm ghép cặp theo seed

Cùng luật, cùng cấu hình sinh vật/môi trường, cùng `12` seed, khác đúng một tổ hợp nhân tố khai tường minh (`interventions` và `live.resource_capacity_scale`):

| Hạng mục | `control_uniform` (đối chứng dồi dào) | `control_uniform_scarcity` (đối chứng chính khớp tổng EU) | `treatment_gradient` (điều trị gradient) |
|---|---|---|---|
| Manifest | `e3-control-uniform.json` | `e3-control-uniform-scarcity.json` | `e3-treatment-gradient.json` |
| `live.resource_capacity_scale` | `1.0` | `0.42` (`~64.5k EU` khởi đầu) | `1.0` |
| Can thiệp 1 (`id=1, cause_id=1`) | `Deforest`, `Rect{0,0,127,255}`, `intensity = 0.0` | `Deforest`, `Rect{0,0,127,255}`, `intensity = 0.0` | `Deforest`, `Rect{0,0,127,255}`, `intensity = 0.30` |
| Can thiệp 2 (`id=2, cause_id=2`) | `AddNutrient`, `Rect{128,0,255,255}`, `intensity = 0.0` | `AddNutrient`, `Rect{0,0,255,255}` (toàn bản đồ), `intensity = 1.00` | `AddNutrient`, `Rect{128,0,255,255}` (nửa Đông), `intensity = 1.00` |
| `start_tick` / `duration_ticks` / `curve` | `1` / `18000` / `Step` | giống hệt | giống hệt |
| `reversible` / `signed_negative` | `false` / `false` | giống hệt | giống hệt |

`control_uniform` và `control_uniform_scarcity` vẫn **giữ nguyên cả hai bản ghi lệnh** (`id=1, 2`) thay vì bỏ lệnh có `intensity = 0.0`: `validate_intervention` (`core/experiment/interventions.rs:16`) chấp nhận `intensity == 0.0`, nên mọi arm đi qua cùng đường mã can thiệp.

**Hằng số của cả ba arm (13 khoá khai tường minh trong `initial_conditions`, đối chiếu `9958f27` + `LAND1`):**

Vì `LIVE_KEYS` có 26 khoá sau merge `LAND1` (`core/live_experiment.rs:222`, gồm `live.land_mask = 0` mặc định tắt), mọi manifest đều khai đủ **13** khoá tường minh giống nhau về cấu trúc (12 khoá chung giống hệt giữa cả 3 arm + `live.resource_capacity_scale` là `1.0` ở hai arm dồi dào/điều trị và `0.42` ở arm khan hiếm đồng đều; lưu ý `scale_pasture_capacity` trả về sớm khi `factor == 1.0`, `core/live_experiment.rs:2569`, nên giữ nguyên bit-identity với lúc vắng mặt):

| Khoá | Giá trị ghim | Ghi chú đối chiếu mã (`9958f27`) |
|---|---|---|
| `live.ecological_fitness` | `1.0` | Nhánh F (`autonomous_reproduction_system` trong headless adapter) |
| `live.mortality` | `1.0` | Bật cái chết (ADR-0005) |
| `live.reproduction` | `1.0` | Bắt buộc `1.0` vì mặc định là `false` (Card S1) |
| `live.locomotion` | `1.0` | Bắt buộc `1.0` vì mặc định là `false` (ADR-0006) |
| `live.drive_floor` | `0.5` | Tránh đứng yên khi tắt não tiến hoá (ADR-0007 mục A) |
| `live.thirst_steering` | `1.0` | Hướng về hồ nước khi khát (ADR-0007 mục A) |
| `live.evolved_brains` | `0.0` | Tắt não tiến hoá để cô lập nhân tố sinh thái |
| `live.founders` | `120.0` | Đủ lớn cho phương sai khối lượng và quần thể ban đầu |
| `live.predator_fraction` | `0.3` | Khớp `LiveWorldConfig::default()` (`core/live_experiment.rs:351`) |
| `live.trees` | `8.0` | Khớp `LiveWorldConfig::default()` (`core/live_experiment.rs:351`) |
| `live.lakes` | `2.0` | Khớp `LiveWorldConfig::default()` (`core/live_experiment.rs:351`) |
| `live.food_cap` | `50.0` | Khớp `LiveWorldConfig::default()` (`core/live_experiment.rs:351`) |
| `live.resource_capacity_scale` | `1.0` (`control_uniform`, `treatment_gradient`) / `0.42` (`control_uniform_scarcity`) | Tỉ lệ sức chứa toàn bản đồ lúc khởi sinh (`scale_pasture_capacity`, `core/live_experiment.rs:2568`) |

**13 khoá `LIVE_KEYS` còn lại giữ nguyên mặc định của `LiveWorldConfig::default()` (`core/live_experiment.rs:360`):**
`live.w_surplus = 1.0`, `live.starve_ticks = 120`, `live.dehydration_ticks = 120`, `live.mutation_rate = 0.05`, `live.prey_cap = 80`, `live.plant_reserve = 0.0`, `live.lake_layout = 0`, `live.kill_hydration = 0.0`, `live.kill_efficiency = 0.40`, `live.target_min_energy = 0.0`, `live.drift_control = 0.0`, `live.drift_reset_ticks = 0`, `live.land_mask = 0` (tắt / `off`, giữ bit-identical với các lượt smoke trước `LAND1`). Địa hình cố định ở `MapSettings::default()`, `seed: 1337`; các manifest đặt `world_identity` toàn `0` (quan sát được trên smoke Windows là `seed=1337, generator_version=1, 256x256, terrain_checksum=3152323152` — giá trị quan sát phụ thuộc nền tảng, không phải hằng số ghim cứng).

## 4. N và các seed

**N = 12 bộ ba (12 seed × 3 arm = 36 lượt chạy)**, seed theo quy tắc số học khai trước (`seed_k = 730001 + 1000·k`, `k ∈ 0..11`), rời hoàn toàn khỏi E2 (`700001..711001`), E2R (`750001..`), E-SEL1 (`760001..771001`), SEL2-P (`780001..799506`), và AE (`1..3`):

```
730001  731001  732001  733001  734001  735001
736001  737001  738001  739001  740001  741001
```

**Tối thiểu để ra quyết định: 10 cặp hợp lệ (không Failed, không tuyệt chủng, và đáp ứng ngưỡng `min_agent_count`).** Dưới 10 cặp hợp lệ là `insufficient_evidence`, không bao giờ thay seed và không giảm `N`.

**Seed smoke: `999979`.** Chạy riêng qua ba manifest smoke (`e3-smoke-control-uniform.json`, `e3-smoke-control-uniform-scarcity.json`, `e3-smoke-treatment-gradient.json`), **không bao giờ** nhập vào phân tích hay bảng kết quả của E3 (bảo đảm cơ học vì `999979` vắng mặt trong mọi manifest thí nghiệm).

## 5. T, lấy mẫu, khởi động và hiệu chỉnh WP-2 / WP-3

Lịch chạy mục tiêu và thang dự phòng WP-3 (`duration_ticks_ladder = [18000, 12000, 6000]`):

| Tham số | Giá trị đã chốt (WP-3) | Vì sao |
|---|---|---|
| `duration_ticks` (**T**) | **18.000** | 300 s mô phỏng ở 60 Hz; 300 chu kỳ nổ band sinh thái (chu kỳ 60); 18 biên epoch |
| `sample_period` | **600** | 30 mẫu mỗi lượt chạy |
| Khởi động (`warmup_ticks`) | **3.000 tick đầu (5 mẫu)** | Loại khỏi mọi thống kê trung bình theo thời gian |
| Cửa sổ đuôi (`tail_window`) | tick **14.400–18.000** (7 mẫu gộp đầu cuối) | Kiểm tra thứ cấp trung bình số học đều 7 mẫu `14400..=18000` |
| Dừng | **chân trời cố định** | Không dừng sớm, không ngó kết quả |

### Bằng chứng WP-2 & chốt lịch WP-3 cho 3 arm ở seed smoke `999979` (`--release`, trunk `9958f27`)

Thực thi mù qua `src-tauri/examples/e3_smoke.rs` và lưu tại `artifacts/experiments/e3-productivity-gradient-body-mass/smoke/`. Trong quá trình chạy smoke, **không ghi, không in, và không dùng bất kỳ giá trị `live.trait_mass_*` nào trong bất kỳ cổng kiểm tra nào ngoại trừ kiểm tra đồng loạt tính hữu hạn `is_finite()` (`body_mass_consulted: false`)**:

- **(a) `ResourceField` & bảo toàn EU khởi sinh đồng nhất giữa cả 3 arm — ĐẠT:**
  - Lưới `256×256` (`x, z ∈ [-100,0, 100,0]`), tổng `WorldIdentity` và trạng thái `SimRng` đầu giống hệt nhau cả 3 arm.
  - Hai arm `scale = 1.0`: sinh khối / sức chứa ban đầu = `153.587,250 EU` (Tây `88.757,750 EU`, Đông `64.829,500 EU`), `detritus = 0,0 EU`.
  - Arm `control_uniform_scarcity` (`scale = 0.42`): thực vật ban đầu = `64.506,642 EU` (Tây `37.278,253 EU`, Đông `27.228,389 EU`) và `detritus = 89.080,608 EU` → **tổng EU kín lúc khởi sinh giống hệt cả 3 arm (`153.587,250 EU`)**.
- **(b1 & b2) Thao túng `live.standing_crop` và khớp tổng thức ăn ở đối chứng khan hiếm — ĐẠT:**
  - `control_uniform`: `standing_crop` tại `T=18000` = `123.300,837`, trung bình sau khởi động (`tick > 3000`) = **`134.866,660`**, trung bình đuôi = `124.085,042`, checksum `1266395716`.
  - `treatment_gradient`: `standing_crop` tại `T=18000` = `63.282,053`, trung bình sau khởi động = **`64.098,521`** (**`−52,47 %`** hay `−70.768,139 EU` so với `control_uniform`, đạt ngưỡng cổng `|Δ| ≥ 10 %`), trung bình đuôi = `63.381,917`, checksum `2891653359` (`300` lần `Deforest` chuyển `93.891,597 EU`; `300` lần `AddNutrient` bù `33.642,288 EU`).
  - `control_uniform_scarcity`: `standing_crop` tại `T=18000` = `64.483,464`, trung bình sau khởi động = **`64.379,015`**, trung bình đuôi (`14400..=18000`) = **`64.467,845`** (vs `63.381,917` ở `treatment_gradient`), checksum `2359367161` (`0` lần `Deforest`; `300` lần `AddNutrient` toàn bản đồ bù `8.521,110 EU`). **Chênh lệch `live.standing_crop` giữa `treatment_gradient` và `control_uniform_scarcity` chỉ là `−0,44 %` (`−280,494 EU`, công thức `(T − S)/T = −0,4376 %` theo `smoke-report.json`; chia `S` là `−0,4357 %`) trung bình sau khởi động, `−1,86 %` (`−1.201,411 EU`, công thức `(T − S)/S`) tại `T = 18.000`, và `−1,68 %` (`−1.085,928 EU`, công thức `(T − S)/S`) ở trung bình cửa sổ đuôi `14400..=18000` (cả ba đều nằm gọn trong ngưỡng `|Δ| ≤ 10 %`)**!
- **(c) `detritus > 0` trước tick `3000` và tại tick `3000` ở cả 3 arm — ĐẠT:** tick `600` đã dương trong cả 3 arm (`1.647,808` / `90.188,814` / `86.709,169 EU`), và tại tick `3000` lần lượt là `12.530,862` / `95.907,225` / `97.795,366 EU`.
- **Bảo toàn & dân số smoke:** độ trôi EU kín cực đại chỉ `1,077e-9` / `1,051e-8` / `4,657e-10`; đủ `27` observable hữu hạn; `agent_count` (min / max / final) là `13 / 193 / 13` (`control_uniform`), **`11 / 120 / 12` (`control_uniform_scarcity` — sống sót tốt qua `T=18.000`)**, và `9 / 192 / 99` (`treatment_gradient`).
- **Chốt WP-3 cho 36 lượt chạy (`smoke/provenance.json`):** `3` arm × `18000` tick hết `17,727 s` (`328,28 us/tick`), ngoại suy cho ensemble **`36 × 18.000`** = **`3,545 phút`** (`<< 90 phút`), nên chốt **`T = 18.000`**, `warmup_ticks = 3000`, `sample_period = 600`, `tail_window = [14400, 18000]` với **`0` seed thí nghiệm**.

### Công khai lịch sử chạy smoke, quy tắc chọn bản ghi & máy chạy thực nghiệm (Execution Disclosure)

1. **Lượt đóng băng đầu (`91def40-dirty`, thiết kế 2 arm):** `e3_smoke` chạy **2 lần** tất định và mù trên cùng commit — lần 1 để đo số WP-2/WP-3, lần 2 sau khi chép số đó vào `e3-preregistration.json` để `sha256_as_committed` khớp tệp commit. Vì mô phỏng tất định, mọi chuỗi số đều trùng khít từng bit.
2. **Fix Round 1 (`9958f27` / build `a1229a6`, thiết kế 3 arm):** vòng hash tuần hoàn đã được cắt bỏ bằng cách **không** đặt `e3-preregistration.json` vào `sha256_as_committed` (thay vào đó băm cả `src-tauri/examples/e3_smoke.rs` và `6` manifest bất biến). Trong `smoke/provenance.json`, trường `build.git_commit` là **`a1229a6b59f1849091d0f2e5c0028769f892c553` (`a1229a6-dirty`)** — chính là commit merge của nhánh `win/e3-prereg` từ trunk `9958f27` với `src-tauri/src/` giống hệt từng bit so với `9958f27`; cụm chữ `"this single deterministic run"` trong `preregistration_hash_note` chỉ **lần gọi 3**; còn `preregistration_sha256_at_run` (`681850c4...`) băm một bản trung gian chưa commit của JSON trước khi chép `wp2_smoke_calibration` và các chỉnh sửa câu chữ Round 2. Ba lần gọi ở Fix Round 1: lần 1 dừng ở `guard_no_body_mass` **trước khi ghi tệp nào** do chú thích đầu CSV; lần 2 ghi bộ artifact 3 arm hoàn chỉnh đầu tiên; lần 3 chạy lại sau `cargo fmt` + `clippy` trên `e3_smoke.rs` (`d9df2c29...`) với checksum mô phỏng y hệt (`1266395716 / 2359367161 / 2891653359`).
3. **Quy tắc chọn bản ghi, commit mới & máy thực thi cố định (`E3-X`):** bản ghi smoke hợp lệ luôn là **Fix Round 1 lần gọi 3 — lượt `--release` tất định hợp lệ đầu tiên trên các manifest và mã `e3_smoke.rs` đã định dạng cuối cùng**. Toàn bộ **`36` lượt `E3-X` (`3` arm × `12` seed) phải chạy trên cùng một máy (`OS/kiến trúc`) và ghi vào `provenance.json` của `E3-X`** (vì tạo địa hình headless phụ thuộc nền tảng: checksum Windows `x86_64` = `3152323152` vs macOS `arm64` = `1212187560`). Nếu `E3-X` chạy trên commit mới hơn `9958f27` hoặc trên OS/kiến trúc khác với máy smoke (`windows/x86_64`), `E3-X` chạy lại `e3_smoke` **1 lần** ở `--release` trên đúng commit/máy đó trước khi chạy seed thí nghiệm để xác nhận WP-2; **nếu bất kỳ cổng WP-2 nào trượt trên commit mới hoặc máy mới: dừng ngay và báo cáo, không bao giờ chỉnh tham số hay ngưỡng.** Ngoài cổng artifact địa hình, vì địa hình cũng có thể nạp qua cache (`ANIMA_CACHE_DIR` hoặc `<temp_dir>/anima_world_cache`), mọi lượt `E3-X` và lượt chạy lại `e3_smoke` trên máy thực thi đều phải ghi nhận `world_identity` quan sát được vào `provenance.json`, và toàn bộ `36` lượt `E3-X` cộng lượt `e3_smoke` trên máy đó phải báo cáo đúng **cùng một `world_identity`**; bất kỳ sai khác nào lập tức dừng chiến dịch (`stop and report, never tune`).

## 6. Metric

**Chính (`H1`):** `live.trait_mass_var` — phương sai tổng thể (chia `n`) của khối lượng cơ thể tại `T = 18.000` trên tương phản `treatment_gradient − control_uniform_scarcity` (kèm kiểm tra thứ cấp trên trung bình cửa sổ đuôi `14400..=18000`, áp dụng quy tắc loại trừ per-contrast đối với tuyệt chủng và `live.agent_count < 2`).
**Phân tích độ nhạy hiệu chỉnh Bessel đăng ký kèm `H1`:** `var · n/(n − 1)` với `n = live.agent_count` tại `T = 18.000` trên đúng các cặp hợp lệ `n ≥ 2` của `H1` (vì `live.trait_mass_var` chia `n`, cỡ quần thể nhỏ hơn ở `S` ~`9–12` vs `37–99` ở `T` gây thiên lệch ước lượng mẫu hữu hạn ~`+8 %` cùng chiều dự đoán của `H1`). Báo cáo đầy đủ `live.agent_count` tại `T = 18.000` cho từng arm; dân số là biến trung gian (*mediator*) của gradient chứ không thay thế `H1`.

**Thứ cấp đã đăng ký (`S1`–`S3`, `S6` — khớp `e3-preregistration.json`):**

1. `S1`: khoảng `live.trait_mass_p90 − live.trait_mass_p10` tại `T = 18.000` (độ trải robust, kèm quy tắc `live.agent_count >= 2`);
2. `S2`: `live.trait_mass_mean` tại `T = 18.000` (phân biệt trải rộng với dịch chuyển trung bình);
3. `S3` (**khai từ Card S2 PR `#148`, trước mọi dữ liệu**): `live.trait_mass_mean_east − live.trait_mass_mean_west` tại `T = 18.000` (hai phía, đo trực tiếp phân hoá khối lượng theo vùng Tây `0..=127` / Đông `128..=255`; bắt buộc áp dụng **`empty_half_rule`** trong §8);
4. `S6`: `live.births` (tổng qua 30 cửa sổ mỗi lượt), `live.deaths` (tổng qua 30 cửa sổ), và `live.mean_lifespan` (**giá trị cửa sổ cuối tại `T = 18.000` trong `final_observables`** — ghim một định nghĩa duy nhất).

**Metric ghi nhận cho đủ bộ nhưng KHÔNG diễn giải (`structural zero`, hạ hạng trước khi có dữ liệu):**

- `S4` (`live.shannon`, `role: reported_not_interpreted_structural_zero`, `decision_effect: none`): trong `core/live_experiment.rs:1611-1612`, hàm gọi `shannon_index(&[herbivores, predators])` — chỉ là **chỉ số cân bằng hai bậc dinh dưỡng ăn cỏ / săn mồi**, hoàn toàn không phải đa dạng kích thước cơ thể hay ổ sinh thái. Trong cả ba arm ở smoke, thú săn mồi đều tuyệt chủng trước tick `3.600`, kéo `live.shannon = 0.0` suốt phần còn lại tới `T`. Vì vậy `S4` **không** kiểm tra được giả thuyết phân niche và **không bao giờ** được diễn giải thành bằng chứng âm sinh học.
- `S5` (`live.archive_coverage`, `role: reported_not_interpreted_structural_zero`, `decision_effect: none`): đọc `BevyMapElitesArchive.archive.grid.len()` (`core/live_experiment.rs:1630-1633`), nhưng archive chỉ được nạp ở luồng tiến hoá của ứng dụng desktop (`core/simulation_loop.rs:832`), còn `LiveExperimentAdapter` chỉ chèn một archive rỗng (`core/live_experiment.rs:2893`). Toàn bộ `60/60` mẫu round 0 và `90/90` mẫu round 1 đều bằng `0.0`. Công cụ đo cấu trúc bằng `0` trong harness headless — báo cáo cho đủ sổ chứ không diễn giải.

Ngoài ra, `I1` (`live.closed_eu_total`) kiểm tra bảo toàn EU kín và `M1` (`live.standing_crop`) kiểm tra đồng thời hai vế thao túng (`T vs U` giảm mạnh và `T vs S` khớp tổng). Toàn bộ **27 observable** hiện hành lúc đăng ký (`all_live_observable_ids()[..27]`) vẫn được yêu cầu và báo cáo đầy đủ.

## 7. Thống kê, chi tiết tính toán và ngưỡng "khác đủ nhiều"

**Thống kê từng seed & gộp cặp:** `PairedEffect` tính trên các cặp hợp lệ sau loại trừ (`treatment − control`, trong đó tương phản chính `H1` dùng `treatment_gradient − control_uniform_scarcity`, và hai tương phản bổ trợ so với `control_uniform`). `None` báo cáo là `None`, không bao giờ đổi thành `0`. Kèm theo: **trung vị** các delta theo seed, **phương sai giữa các seed**, và **bảng delta từng seed**.

- **Cửa sổ đuôi (`H1_tail_window`):** trung bình số học có trọng số bằng nhau của **7** mẫu nằm trong đoạn đóng `[14400, 18000]` (`14400, 15000, 15600, 16200, 16800, 17400, 18000`) cho mỗi lượt chạy, sau đó mới lấy `PairedEffect` qua các cặp hợp lệ.
- **S1 & S3 theo seed:** tính trực tiếp tại `T = 18.000` (`final_observables`), `S1 = p90 − p10`, `S3 = mean_east − mean_west`.

**Năm nhãn quyết định rời nhau và vét cạn (`decision_enum`, khớp `e3-preregistration.json`):**

Với ngưỡng máy đọc `dz_min = 0,8`, `relative_delta_min = 0,05` (`5 %`), `min_valid_pairs = 10`, CI 95 % ghép cặp (`ci_level = 0,95`):

1. **`insufficient_evidence`:** số cặp hợp lệ `< 10` (chỉ cần hơn `2` cặp bị loại do hỏng/tuyệt chủng/đơn lẻ ở `H1`, hoặc trống nửa bản đồ ở `S3`).
2. **`null`:** số cặp hợp lệ `≥ 10` **VÀ** khoảng tin cậy 95 % của `paired_mean_delta` **chứa 0** (`ci95_low ≤ 0 ≤ ci95_high`, hoặc `ci95` là `None`).
3. **`between_detectable_below_materiality`:** số cặp hợp lệ `≥ 10`, CI 95 % **loại 0** (`ci95_low > 0` hoặc `ci95_high < 0`), **NHƯNG** trượt điều kiện `|paired_dz| ≥ 0,8` **HOẶC** trượt điều kiện `|paired_mean_delta| ≥ 0,05 × |control_mean|` (hoặc `paired_dz` là `None`). Báo cáo là *phát hiện được nhưng dưới ngưỡng vật chất* — **không** phải "khác đủ nhiều".
4. **`material_positive`:** số cặp hợp lệ `≥ 10`, CI 95 % loại 0, `paired_mean_delta > 0`, `|paired_dz| ≥ 0,8`, **VÀ** `|paired_mean_delta| ≥ 0,05 × |control_mean|`.
5. **`material_negative_opposite`:** số cặp hợp lệ `≥ 10`, CI 95 % loại 0, `paired_mean_delta < 0`, `|paired_dz| ≥ 0,8`, **VÀ** `|paired_mean_delta| ≥ 0,05 × |control_mean|` (bác bỏ H1 theo hướng ngược).

Các metric thứ cấp `S1, S2, S3, S6` dùng **cùng bộ ngưỡng** (`ci95` loại 0, `|dz| ≥ 0,8`, `|delta| ≥ 0,05 × |control_mean|`; riêng `S3` có dấu nên nếu `control_mean == 0.0` chính xác thì ghi chú rõ kiểm tra theo `ci95` loại 0 và `|dz| ≥ 0,8`). Kết quả ở metric thứ cấp **không bao giờ** nâng hạng thay `H1`.

## 8. Xử lý thất bại, quy tắc tuyệt chủng / đếm tối thiểu / nửa bản đồ trống, và ngân sách

**Tại sao phải có quy tắc loại trừ cơ học (sửa lỗi D1):**
`mass_statistics(&[])` (`core/live_experiment.rs:1458-1461`) trả về `[0.0; 4]`, và `live.trait_mass_mean_{west,east}` trả về `0.0` khi nửa bản đồ có 0 cá thể (`core/live_experiment.rs:1421-1422, 1605-1606`). Những giá trị này hữu hạn nên `drive()` **không** đánh dấu `RunStatus::Failed`, khiến `ensemble.rs` dễ nuốt số `0.0` giả làm giá trị quan sát thật (nhất là khi arm smoke chỉ còn `9–13` cá thể và nửa Tây ở điều trị bị chặt xuống ~`1 %` sức chứa). Vì mọi cá thể sống luôn có `total_mass() > 0.0` (`decode_genotype` cấm genotype rỗng ở `core/spawn.rs:75-78`), phát hiện `0.0` khi đếm rỗng là **chính xác 100 %** không mất thông tin.

1. **Thất bại kỹ thuật (`RunStatus::Failed`):** `drive()` dừng sớm khi gặp observable phi hữu hạn (cơ chế dừng sớm duy nhất). Gặp vế `Failed` thì **loại theo từng tương phản (`per_contrast`)** khỏi mọi bảng thống kê dùng arm đó và liệt kê đủ `(seed, arm, tick, reason)`.
2. **Quy tắc tuyệt chủng (`extinction_rule`, khớp tinh thần E-SEL1, phạm vi `per_contrast`):**
   - **Định nghĩa:** `live.agent_count == 0` tại bất kỳ mẫu nào trong `30` mẫu (`600, 1200, ..., 18000`) hoặc ở `final_observables` (`T = 18000`).
   - **Xử lý:** là một **kết cục thật được ghi nhận** (không gọi là `Failed`), và **việc loại trừ áp dụng theo từng tương phản (`per_contrast`)**: một lượt chạy tuyệt chủng ở một arm chỉ loại seed đó khỏi **các tương phản sử dụng arm đó** (ví dụ `H1` `T − S` chỉ tính hai arm `S` và `T` — nếu `control_uniform` (`U`) tuyệt chủng ở một seed thì seed đó bị loại khỏi `T − U` và `S − U` nhưng **vẫn giữ trong `H1` `T − S`**; bảng phân rã cộng tính `(T − U) = (T − S) + (S − U)` báo cáo trên tập seed hợp lệ ở cả ba arm). Không bao giờ đưa `0.0` vào delta `live.trait_mass_*`, và ghi đầy đủ vào `n_extinct_pairs_dropped` + `extinction_table_by_arm` `(seed, arm, first_extinct_tick)`.
3. **Quy tắc số cá thể tối thiểu cho độ phân tán (`min_agent_count_rule`, cho `H1`, `H1_tail_window`, `S1`, phạm vi `per_contrast`):**
   - **Định nghĩa:** `live.agent_count < 2` ở `final_observables` tại `T = 18000` (hoặc tại bất kỳ mẫu nào trong `14400..=18000` đối với `H1_tail_window`). Với `1` cá thể sống, `mass_statistics` trả `var = 0.0` và `p90 − p10 = 0.0` suy biến, không đo được độ phân tán.
   - **Xử lý:** loại seed theo từng tương phản khỏi `H1`, `H1_tail_window` và `S1` đối với những tương phản chứa arm đơn lẻ (`H1` chỉ xét `S` và `T`), báo cáo vào `n_singleton_pairs_dropped` + `singleton_table_by_arm`.
4. **Quy tắc nửa bản đồ trống (`empty_half_rule`, cho `S3`, phạm vi `per_contrast`):**
   - **Định nghĩa:** tại `T = 18000` (`final_observables`), một trong hai arm của **tương phản đang xét** có `live.trait_mass_mean_west == 0.0 || live.trait_mass_mean_east == 0.0`.
   - **Xử lý:** loại seed theo từng tương phản khỏi **`S3`** (`east − west`) đối với những tương phản dùng arm bị trống nửa bản đồ, ghi nhận rõ `(seed, arm, tick = 18000, empty_half = west | east | both)` vào `n_empty_half_pairs_dropped_s3`, `empty_half_table_by_arm`, và `per_arm_empty_half_counts`. Không loại seed đó khỏi `H1` / `S1` / `S2` toàn cục chừng nào vẫn đạt `extinction_rule` và `min_agent_count_rule`.
5. **Ngưỡng không đủ bằng chứng (`< 10` cặp hợp lệ trong một tương phản):** với `N = 12`, hễ **trên một tương phản** có `> 2` seed bị loại thì kết luận cho endpoint ở tương phản đó là `insufficient_evidence` (không chạy thêm seed bù, không hạ `N`).
6. **Ngân sách & máy thực thi cố định (`36` lượt):** `36` lượt chạy (`3` arm × `12` seed) × `18.000` tick = `648.000` tick. **`--release` là bắt buộc** (ước tính WP-3: **`3,55 phút`** `<< 90 phút`), và toàn bộ `36` lượt chạy trên **cùng một máy thực thi** (ghi OS/kiến trúc vào `provenance.json`; nếu khác máy smoke `windows/x86_64` thì chạy lại `e3_smoke` trước và dừng/báo cáo nếu bất kỳ cổng WP-2 nào trượt). Trước khi bắt đầu mọi lượt chạy `E3-X` và mọi lượt chạy lại `e3_smoke`, bắt buộc kiểm tra và ghi vào `provenance.json` rằng biến môi trường `ANIMA_WORLD_ARTIFACT` chưa được đặt (*unset*) và tệp `<temp_dir>/anima_world_artifact.anmw` không tồn tại; nếu một trong hai tồn tại thì **không được bắt đầu chạy** (tránh `build_live_world` → `init_world` âm thầm nạp địa hình từ ứng dụng desktop). Đồng thời, để phủ cả cache địa hình (`ANIMA_CACHE_DIR` / `<temp_dir>/anima_world_cache`) và sai khác môi trường, mọi lượt `E3-X` và lượt chạy lại `e3_smoke` trên máy thực thi đều phải ghi `world_identity` quan sát được vào `provenance.json`, và tất cả `36` lượt `E3-X` cộng lượt `e3_smoke` trên máy đó bắt buộc báo đúng **cùng một `world_identity`** — hễ sai khác là **dừng chiến dịch ngay và báo cáo, không bao giờ chỉnh tham số hay ngưỡng** (`stop and report, never tune`). Một khuyết tật của harness không bao giờ bị viết thành "bằng chứng không đủ" — sửa lỗi, chạy lại và công khai đã chạy lại.

## 9. Việc phải làm trước khi đây thành tiền đăng ký

| # | Gói việc | Trạng thái (`2026-11-19`, Fix Round 3) | Bằng chứng / Điều kiện xong |
|---|---|---|---|
| WP-1 | Fixture JSON máy đọc được (3 arm) & test đối chiếu | **XONG** | `src-tauri/tests/fixtures/experiments_e3/{e3-preregistration.json, e3-control-uniform.json, e3-control-uniform-scarcity.json, e3-treatment-gradient.json, e3-smoke-*.json}` và `src-tauri/tests/prereg_e3_manifest_tests.rs` |
| WP-2 | **Lượt chạy smoke 3 arm ở seed 999979** | **XONG** | `src-tauri/examples/e3_smoke.rs` → `artifacts/experiments/e3-productivity-gradient-body-mass/smoke/`: (a) `ResourceField` `256×256` & đóng EU giống hệt 3 arm; (b1) `standing_crop` `treat − ctrl_u` `−52,47 %`; (b2) `ctrl_s` khớp `treat` `−0,44 %` (sau khởi động), `−1,86 %` (tại `T`), `−1,68 %` (đuôi); (c) `detritus > 0` từ tick `600` ở cả 3 arm; `body_mass_consulted: false` |
| WP-3 | Chốt `T` và cửa sổ khởi động từ WP-2 | **XONG** | Ghi `wp3_locked_schedule` (`T = 18000`, `warmup_ticks = 3000`) cho `36` lượt chạy (`3,55` phút dự kiến `<< 90` phút) với `0` seed thí nghiệm |
| WP-4 | Đóng băng tài liệu này, ghi commit cha `9958f27` | **XONG** | `status: active` (`status: active \| completed` cho E3-X về sau), ghi quy tắc loại trừ D1, hạ hạng S4/S5 D2, giải quyết confound bằng arm thứ 3, và ngưỡng vật chất máy đọc §7 |
| WP-5 | Chạy ensemble 36 lượt (`3` arm × `12` seed), phân tích theo §7–§8 | **Chưa chạy (`E3-X`)** | `0` seed trong `730001..741001` đã chạy ở card `E3-P`; thực thi ở card riêng `E3-X` |

Thứ tự là bắt buộc. Cả WP-2 (b1) và (b2) đều đạt chặt chẽ trên seed smoke `999979`, nên tiền đăng ký đủ điều kiện chuyển sang WP-5 (`E3-X`) sau khi owner duyệt PR này.

## 10. Hai dụng cụ sẽ mở khoá câu hỏi gốc

1. **Observable khối lượng theo vùng — ĐÃ GIAO bởi Card S2 (PR `#148`):** `live.trait_mass_mean_west` (`0..=127`) và `live.trait_mass_mean_east` (`128..=255`) tại `core/live_experiment.rs:1428`, được đăng ký làm metric thứ cấp `S3` trong E3 kèm `empty_half_rule`.
2. **Một can thiệp chạm `r_max` theo vùng, không chỉ `r` — VẪN CÒN THIẾU:** `strip_region`/`fertilise_region` đổi standing crop còn `r_max` bất biến theo vùng trong suốt lượt chạy. Hiện tại E3 khắc phục nhiễu tổng thức ăn bằng arm đối chứng `control_uniform_scarcity` (`live.resource_capacity_scale = 0.42` + `AddNutrient 1.00` toàn bản đồ). Một `InterventionKind` chỉnh sức chứa theo vùng về sau vẫn cần thiết nếu muốn đổi NPP thật sự giữa chừng.

## 11. Ngôn ngữ mà thí nghiệm này bị ràng buộc

- "Không phát hiện được hiệu ứng" ≠ "không có hiệu ứng". Một kết quả `null` được viết là `null`.
- Mọi kết luận dương trên `H1` được phát biểu đúng theo thiết kế đã đăng ký: *"gradient năng suất tạo bởi Tây chặt 0.30 / Đông bón 1.00 làm thay đổi phương sai khối lượng cơ thể so với đối chứng khan hiếm đồng đều ở cùng tổng lượng thức ăn (~64k EU)"*, ghi nhận rõ giới hạn chặt liên tục vs tỷ lệ `r_max` lúc khởi sinh (§1).
- `S4` (`live.shannon`) và `S5` (`live.archive_coverage`) chỉ được ghi nhận với ghi chú *bằng không do cấu trúc trong headless adapter*, không bao giờ trích làm kết quả âm sinh học.
- Mỗi con số trong báo cáo phải chỉ ra được tệp sinh ra nó.
- Mọi cặp bị loại (do `Failed`, `live.agent_count == 0`, `< 2`, hoặc nửa bản đồ trống `0.0`) được liệt kê đầy đủ theo từng tương phản (`per_contrast`), không bao giờ lặng lẽ bỏ hoặc đưa sentinel `0.0` vào thống kê.
- **Dân số là biến trung gian (*mediator*):** báo cáo đầy đủ `live.agent_count` tại `T = 18.000` cho từng arm kèm kết quả phân tích độ nhạy hiệu chỉnh Bessel `var · n/(n − 1)` cạnh `H1`.
- **Kết quả âm là một kết quả.** "Gradient năng suất không làm phương sai khối lượng hay trung bình Tây/Đông khác nhau trong harness này" là một câu trả lời dùng được và được viết ra y như một kết quả dương.

2026-11-19 amendment (FIX-CAP): new live key live.strict_prey_cap registered as default-absent (0 = legacy); E3 runs are unaffected.
2026-11-20 amendment (SEL2P2-PREP): new live key live.grass_steering registered as default-absent (0 = legacy); E3 runs are unaffected.
2026-11-20 amendment (FOUNDER-LAYOUT): new live key live.founder_layout registered as default-absent (0 = legacy); E3 runs are unaffected.
2026-11-23 amendment (F-066): new live key live.lineage_tracking registered as default-absent (0 = legacy); E3 runs are unaffected.
