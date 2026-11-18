---
phase: planning
feature: experiment-sel2-food-scarcity
title: Thẻ S11 — khoá live.grass_steering (tìm cỏ cho thú ăn cỏ)
description: Khoá live.grass_steering (LIVE_KEYS.len() = 25), giữ GRASS_STAY_FRACTION = 0.25 và GRASS_SCAN_RADIUS = 3; đạt #1, #3, #4, #5, kiểm thử đơn vị WHAT và fmt/clippy/dataflow nhưng BLOCKED tại DONE WHEN #2; đo thăm dò 100.000 tick cho VIABLE = no ở cả scale 1 / 0.7 / 0.5
status: active
owner: simulation-architecture
last_reviewed: 2026-11-18
---

# Thẻ S11 — khoá `live.grass_steering` (tìm cỏ cho thú ăn cỏ)

> Trạng thái hiện tại: **`BLOCKED` tại `DONE WHEN #2`** (giữ nguyên ngưỡng khoá theo Quy tắc R2; chờ quyết định của chủ dự án trước khi đánh dấu sẵn sàng hoặc chỉnh sửa ngoài phạm vi `AUTHORIZATION`).
> Nhánh: `dell/s11-grass-steering` | Base commit (`origin/feat/ecological-selection`): `dea18583881a12ba54ea719f9b6d78dd6d18b05e`.

## 1. Phạm vi thay đổi (đúng theo `AUTHORIZATION`)

- `src-tauri/src/core/resources.rs`: thêm trường `pub grass_steering: bool` vào `LocomotionPolicy` (mặc định `false` trong `LocomotionPolicy::disabled()`).
- `src-tauri/src/core/locomotion.rs`: thêm hai hằng `GRASS_STAY_FRACTION = 0.25` và `GRASS_SCAN_RADIUS = 3`, cập nhật `steering_drive_system` cho thú ăn cỏ (`Option<&Prey>`, `predator.is_none()`): giữ nguyên ưu tiên cao nhất cho tìm nước (`thirst_steering`, khi `hydration < 0.50 * hydration_target` và `lake_cache` khác rỗng). Khi không hướng tới nước: (1) nếu ô cỏ hiện tại có `cur_r_max > 0.0` và `cur_grazeable >= 0.25 * cur_r_max`, đặt ngay `target_velocity = Vec3::ZERO` (đứng gặm, không chạy theo `Food`); (2) ngược lại quét cửa sổ `(2 * GRASS_SCAN_RADIUS + 1)^2` (`7x7`) đã cắt biên lưới để tìm ô có `grazeable` lớn nhất vượt hẳn `cur_grazeable` (hoà lấy `(row, col)` nhỏ nhất): nếu thấy thì hướng tới tâm ô đó trừ khi có `Food` **gần hơn hẳn** (`min_food_dist_sq < grass_dist_sq`), còn nếu không có ô nào hơn ô hiện tại thì giữ fallback về `Food`. Kẻ săn mồi giữ nguyên hành vi cũ; không cấp phát heap trong vòng lặp ấm.
- `src-tauri/src/core/live_experiment.rs`: khai báo `LIVE_KEY_GRASS_STEERING = "live.grass_steering"`, mở rộng `LIVE_KEYS` lên `25` khoá, thêm trường `grass_steering: bool` vào `LiveWorldConfig`, kiểm tra giá trị boolean chặt (`0.0`/`1.0`) và từ chối `live.grass_steering=1` khi `live.locomotion=0` ngay bước preflight (`InvalidLaw`), và truyền xuống `LocomotionPolicy` trong `build_live_world`.
- `src-tauri/tests/e2_seam_tests.rs` & `src-tauri/tests/eco_card_b_tests.rs`: cập nhật kiểm tra `LIVE_KEYS.len() == 25` và trường `grass_steering: false` trong cấu trúc tường minh của `eco_card_b_tests`.
- `src-tauri/tests/grass_steering_tests.rs`: 8 kiểm thử đơn vị/tích hợp (`0 #[ignore]`) bao phủ `DONE WHEN #1`–`#4` và các kiểm thử hợp đồng `WHAT`.
- `src-tauri/examples/sel2_grass_steering.rs`: driver đo thăm dò 100.000 tick (`DONE WHEN #7`) trên `scale in {1.0, 0.7, 0.5}` với `prey_cap = 320` (seeds `1..=6`) và đối chứng `prey_cap = 80` (`scale = 1.0`, seeds `1..=6`), chỉ đọc 7 biến quan sát được phép (mù khối lượng cơ thể).

## 2. Kết quả kiểm tra trong đợt này

- **DONE WHEN #1** (`absent_and_zero_match_every_sampled_tick_with_locomotion_on`): khớp từng bit toàn bộ 16 mẫu `S11_TRAJ` ở thế giới A (`tick 100..=600`: `0xfc93dfcc`, `0x0ece3108`, `0x2b2d8654`, `0xb798318f`, `0x24683ee3`, `0x3b935a42`) và thế giới B (`tick 100..=1000`: `0xd1f70df2`, `0xdd224cdd`, `0x1b2ea787`, `0x73b828c6`, `0x8878f271`, `0x7cbd9f17`, `0x20b394a5`, `0xb2908b42`, `0x44534e87`, `0x1a062d2f`) so với commit nền `dea18583`.
- **DONE WHEN #2** (`sel2e_best_world_behaviour_restores_under_prey_grass_and_breeding_energy`): **ĐỎ / `BLOCKED`** (xem chi tiết mục 3 bên dưới).
- **DONE WHEN #3** (`steering_drive_system_allocates_nothing_once_warm_with_grass_steering_on`): `S11_ZERO_ALLOC allocations=0`.
- **DONE WHEN #4** (`grass_steering_checksum_agrees_across_four_processes_and_keeps_closed_eu`): 4 tiến trình độc lập đều ra `0x27da543a`; EU kín `start=156587.24986362457`, `worst_relative_drift=0`.
- **DONE WHEN #5** (7 suite hồi quy):
  - `e2_seam_tests`: `9 passed; 0 failed; 0 ignored`
  - `eco_card_b_tests`: `12 passed; 0 failed; 0 ignored`
  - `ecology_closed_ledger_integration_tests`: `6 passed; 0 failed; 0 ignored`
  - `energy_conservation_tests`: `3 passed; 0 failed; 1 ignored` (giữ nguyên `diagnose_which_system_moves_the_closed_total` được ignore sẵn từ upstream)
  - `plant_reserve_tests`: `11 passed; 0 failed; 0 ignored`
  - `sel2_drift_control_tests`: `9 passed; 0 failed; 0 ignored`
  - `steering_zero_energy_tests`: `2 passed; 0 failed; 0 ignored`
- **Kiểm thử đơn vị hợp đồng `WHAT`** (`prey_stays_above_threshold_and_respects_s10_plant_reserve`, `window_clipping_tie_breaking_food_strictly_closer_thirst_priority_and_predators_unchanged`, `key_is_registered_and_refused_at_preflight_without_locomotion`, và helper tiến trình con `s11_child_entry_point`): đều đạt (tổng 7/8 test trong `grass_steering_tests` đạt, 1 test `#2` đỏ).
- **DONE WHEN #6** (gate tĩnh và `dataflow`):
  - `cargo fmt --check`: đạt (0 diff).
  - `cargo clippy --all-targets --features desktop -- -D warnings`: đạt (0 cảnh báo).
  - `cargo clippy --all-targets --no-default-features -- -D warnings`: đạt (0 cảnh báo).
  - `cargo modules dependencies --lib --no-externs --package anima-engine` + `scripts/verify_dataflow.py dataflow.json dataflow-truth.dot 1`: `QUA CONG — 16 edge khop hoan toan voi cargo-modules` (`dataflow.json` không cần sửa).
  - Lưu ý: chưa chạy `cargo test --features desktop > out.txt 2>&1` toàn workspace + `node scripts/check_test_targets.mjs src-tauri/out.txt --profile desktop` vì `grass_steering_tests` đang đỏ `DONE WHEN #2` nên toàn suite chưa thể thoát 0 trong trạng thái `BLOCKED` (file `grass_steering_tests.rs` thêm 8 test, `0 #[ignore]`).
- **DONE WHEN #7** (`src-tauri/examples/sel2_grass_steering.rs`): chạy xong toàn bộ `24/24` lượt đo 100.000 tick `--release` (xem mục 4).

## 3. Chẩn đoán thất bại ở `DONE WHEN #2` (`BLOCKED`)

Kiểm thử `sel2e_best_world_behaviour_restores_under_prey_grass_and_breeding_energy` (kịch bản tốt nhất từ SEL2-E: `scale=0.7`, `grace=960`, `founders=100`, `lakes=8`, `prey_cap=320`, mẫu mỗi 500 tick từ `2000..=6000`) đỏ theo đúng ngưỡng khoá ban đầu tại `src-tauri/tests/grass_steering_tests.rs:640`:

```text
S11_BEHAVIOUR_SUMMARY grass=0 median_under_prey=0.0000 median_field=1.0000 total_breed_ready=0
S11_BEHAVIOUR_SUMMARY grass=1 median_under_prey=0.0000 median_field=1.0000 total_breed_ready=0
thread 'sel2e_best_world_behaviour_restores_under_prey_grass_and_breeding_energy' panicked at tests\grass_steering_tests.rs:640:5:
treatment (grass_steering=1) under-prey median 0.0000 is less than half field median 1.0000
```

Phép đo phụ trợ trong khoảng `500..=6000` tick ở bản `--release` cho hai bán kính `rad=3` và `rad=16` (cùng giữ `stay=0.25`) cho kết quả gần như y hệt (`grass=0` và `grass=1` đều có `median_under_prey=0.0000`, `total_breed=0`):

| Phép thử | `median_under_prey` | `median_field` | `hyd_med` (`t=2000..6000`) | `eng_max` (`t=500..6000`) | `total_breed` |
| --- | ---: | ---: | ---: | ---: | ---: |
| `rad=3, grass=0` | `0.0000` | `1.0000` | `0.000..0.420` | `0.617..0.647` | `0` |
| `rad=3, grass=1` | `0.0000` | `1.0000` | `0.000..0.390` | `0.595..0.649` | `0` |
| `rad=16, grass=1` | `0.0000` | `1.0000` | `0.000..0.482` | `0.602..0.647` | `0` |

Ba nguyên nhân vật lý và lịch hệ thống khiến điều kiện `#2` không thể đồng thời thoả dưới thiết kế hiện tại của thẻ S11:

1. **Ưu tiên khát nước (`THIRST_STEERING_FRACTION = 0.50`) lấn át tìm cỏ trong cửa sổ `2000..=6000` tick**: Thế hệ sáng lập xuất phát tại `z = -30`, cách 8 hồ nước tại `z = +60` tới 90 đơn vị. Từ tick `2000` đến `6000`, trung vị độ ẩm cơ thể `hyd_med` nằm trong `0.000..0.482 < 0.50` khi đàn kéo lên phía bắc (`med_z` từ `-35` lên `+17`), nên nhánh tìm nước vô hiệu hoá nhánh tìm cỏ cho phần lớn cá thể trong chính khoảng tick mà `#2` lấy mẫu.
2. **Thời gian ăn cạn ô (`<= 37` tick, còn trên ngưỡng `0.25 * r_max` là `<= 9` tick) ngắn hơn thời gian rời ô (`~68` tick)**: Kể cả khi `grass_steering` hoạt động ở `tick 500..=1500` (`hyd_med = 0.567..0.740`), `under_med` vẫn là `0.0000`. Trên lưới `256 x 256` phủ miền `[-100, 100]`, mỗi ô rộng `0.78125` đơn vị; ở `drive_floor = 0.5`, tốc độ tối đa là `0.75` u/s (`0.0125` u/tick -> `62.5` tick để băng qua ô, cộng thêm `tau = 30` tick tăng tốc từ trạng thái đứng yên), trong khi `herbivore_grazing_system` hút `8.0 / 60 = 0.1333` EU/tick trên ô `Grassland` `scale = 0.7` (`r_max = 4.9` EU). Thú ăn sạch ô xuống `0.0000` trước khi đi ra khỏi ô đó.
3. **Sinh sản ở cuối tick ghim chỉ số `energy / energy_target` sau tick luôn dưới `0.65`**: `autonomous_reproduction_system` chạy vào cuối mỗi tick và rút `50%` năng lượng để sinh sản ngay khi `prey_count < prey_cap` (`320`) và `energy >= 0.65 * energy_target`. Vì số thú ăn cỏ chỉ đạt `60..233 < 320`, không cá thể nào kết thúc tick với `energy >= 0.65 * energy_target` (`eng_max = 0.595..0.649 < 0.650`, `total_breed_ready = 0` ở cả control lẫn treatment).

## 4. Số đo thăm dò 100.000 tick (`sel2_grass_steering`, `DONE WHEN #7`)

Cấu hình chung: `grace=960`, `founders=100`, `lakes=8`, `reserve=0`, `grass_steering=1`, `seeds 1..=6`, chế độ `--release` (24 lượt chạy hoàn tất toàn bộ).

### Bảng tổng hợp (`prey_cap = 320`)

| `scale` | `prey_cap` | `extinctions` | `c1_min_agents` | `mean_agents_med` | `c2_g_med` | `c2_g_min..max` | `c3_generations_med` | `c3_generations_min` | `c4_share_starvation` | `c4_share_min..max` | `c5_seeds_active` | `VIABLE` |
| ---: | ---: | --- | ---: | ---: | ---: | --- | ---: | ---: | ---: | --- | --- | --- |
| `1` | `320` | `1/6` | `0` | `97.7` | `0.963` | `0.897..1.527` | `24.8` | `22.5` | `0.388` | `0.374..0.454` | `5/6` | `no` |
| `0.7` | `320` | `1/6` | `0` | `52.6` | `0.783` | `0.756..1.055` | `22.5` | `20.2` | `0.435` | `0.406..0.548` | `5/6` | `no` |
| `0.5` | `320` | `4/6` | `0` | `10.2` | `0.697` | `0.657..0.867` | `21.1` | `17.0` | `0.592` | `0.524..0.877` | `2/6` | `no` |

### Dữ liệu từng lượt chạy (`24/24`)

```text
scale	grace	founders	lakes	prey_cap	reserve	grass_steering	seed	sum_r_max	g	min_agents	mean_agents	final_agents	births	deaths	starvation	dehydration	starvation_share	mean_lifespan	gen_by_lifespan	gen_by_turnover	generations	births_last_q	deaths_last_q
1	960	100	8	80	0	1	1	153587.2	NaN	55	90.1	97	1971	1974	538	1436	0.273	3506.4	28.5	21.9	21.9	455	438
1	960	100	8	80	0	1	2	153587.2	NaN	41	82.9	86	1701	1715	471	1244	0.275	3762.5	26.6	20.5	20.5	384	405
1	960	100	8	80	0	1	3	153587.2	NaN	48	87.0	81	1993	2012	559	1453	0.278	3276.4	30.5	22.9	22.9	432	433
1	960	100	8	80	0	1	4	153587.2	NaN	42	82.0	89	1566	1577	419	1158	0.266	4136.1	24.2	19.1	19.1	356	315
1	960	100	8	80	0	1	5	153587.2	NaN	20	78.0	92	1533	1541	384	1157	0.249	3920.3	25.5	19.6	19.6	399	390
1	960	100	8	80	0	1	6	153587.2	NaN	25	80.4	93	1518	1525	363	1162	0.238	4078.8	24.5	18.9	18.9	305	241
0.5	960	100	8	320	0	1	1	76793.6	0.657	2	35.0	68	839	871	456	415	0.524	3111.4	32.1	24.0	24.0	128	66
0.5	960	100	8	320	0	1	2	76793.6	0.754	0	8.2	0	183	283	191	92	0.675	2504.4	39.9	22.2	22.2	0	0
0.5	960	100	8	320	0	1	3	76793.6	0.867	0	2.5	0	104	204	179	25	0.877	1080.7	92.5	41.6	41.6	0	0
0.5	960	100	8	320	0	1	4	76793.6	0.699	0	9.9	0	198	298	172	126	0.577	2829.2	35.3	20.0	20.0	0	0
0.5	960	100	8	320	0	1	5	76793.6	0.669	5	30.8	30	524	594	340	254	0.572	4319.8	23.1	17.0	17.0	115	101
0.5	960	100	8	320	0	1	6	76793.6	0.696	0	10.5	0	196	296	170	126	0.574	3044.0	32.9	18.7	18.7	0	0
0.7	960	100	8	320	0	1	1	107511.1	0.756	9	64.8	49	1641	1692	774	918	0.457	3061.1	32.7	25.3	25.3	357	386
0.7	960	100	8	320	0	1	2	107511.1	1.055	0	13.8	0	334	434	238	196	0.548	2624.9	38.1	24.1	24.1	0	0
0.7	960	100	8	320	0	1	3	107511.1	0.817	8	58.5	102	1441	1439	607	832	0.422	3182.8	31.4	24.6	24.6	171	98
0.7	960	100	8	320	0	1	4	107511.1	0.768	3	51.0	7	1056	1149	467	682	0.406	3607.7	27.7	20.7	20.7	256	312
0.7	960	100	8	320	0	1	5	107511.1	0.799	1	47.3	17	992	1075	438	637	0.407	3553.5	28.1	21.0	21.0	83	132
0.7	960	100	8	320	0	1	6	107511.1	0.759	5	54.3	81	1097	1116	480	636	0.430	3743.1	26.7	20.2	20.2	189	125
1	960	100	8	320	0	1	1	153587.2	0.971	29	106.6	171	2713	2642	1020	1622	0.386	3051.6	32.8	25.4	25.4	480	356
1	960	100	8	320	0	1	2	153587.2	0.989	6	100.9	192	2343	2251	861	1390	0.382	3461.4	28.9	23.2	23.2	608	475
1	960	100	8	320	0	1	3	153587.2	0.955	18	103.4	38	2679	2741	1055	1686	0.385	2975.1	33.6	25.9	25.9	794	793
1	960	100	8	320	0	1	4	153587.2	0.897	8	93.5	104	2103	2099	786	1313	0.374	3538.8	28.3	22.5	22.5	225	218
1	960	100	8	320	0	1	5	153587.2	1.527	0	28.7	0	776	876	398	478	0.454	2619.7	38.2	27.1	27.1	0	0
1	960	100	8	320	0	1	6	153587.2	0.914	4	94.6	194	2284	2190	847	1343	0.387	3324.6	30.1	24.2	24.2	589	428
```

Ở đối chứng `prey_cap = 80` (`scale = 1`, `seeds 1..=6`), không có seed nào tuyệt chủng (`min_agents = 20..55`, `mean_agents = 78.0..90.1`), và chết đói chiếm `0.238..0.278` tổng số chết (còn lại chủ yếu là chết khát). Ở `prey_cap = 320`, cả ba mức `scale = 1 / 0.7 / 0.5` đều có tuyệt chủng (`1/6`, `1/6`, `4/6`) và cho `VIABLE = no`.

## Không khẳng định

- Toàn bộ các lượt chạy 100.000 tick ở trên mang tính thăm dò (*exploratory*): tắt di truyền/đột biến theo ràng buộc mù khối lượng cơ thể của thẻ S11, chỉ chạy trên 6 seed cố định (`1..=6`), trên Windows 11 x64 (`rustc 1.98.1`, bản `--release`).
- Không khẳng định `live.grass_steering` giúp lưới SEL2-E trở nên khả thi: cả ba mức `scale = 1`, `0.7`, `0.5` tại `prey_cap = 320` đều ghi nhận `VIABLE = no`.
- Không nới lỏng khẳng định trong `DONE WHEN #2` và không thay đổi các thông số ngoài `AUTHORIZATION`; thẻ S11 hiện dừng ở trạng thái `BLOCKED` chờ hướng dẫn tiếp theo của chủ dự án/lead.
