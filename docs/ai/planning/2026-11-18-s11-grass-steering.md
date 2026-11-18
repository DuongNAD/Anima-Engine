---
phase: planning
feature: experiment-sel2-food-scarcity
title: Thẻ S11 — khoá live.grass_steering (tìm cỏ cho thú ăn cỏ)
description: Kết quả âm, mã không merge. Khoá live.grass_steering (trên nhánh dell/s11-grass-steering tại 95044c4b, LIVE_KEYS.len() = 25), giữ GRASS_STAY_FRACTION = 0.25 và GRASS_SCAN_RADIUS = 3; phép thử cơ chế đối chứng (#2 mới theo DECISION S11 226d4b29) tăng năng lượng ở tick 1000–1500 nhưng giảm dưới đối chứng từ tick 2000–3000; #7 cho VIABLE = no ở cả scale 1 / 0.7 / 0.5. Khi bỏ khát, con mồi giữ 0,89–0,98 năng lượng mục tiêu, nên cỏ không phải giới hạn; khát mới là giới hạn (thẻ S12)
status: completed
owner: simulation-architecture
last_reviewed: 2026-11-18
---

# Thẻ S11 — khoá `live.grass_steering` (tìm cỏ cho thú ăn cỏ)

> **Kết luận của lead (2026-11-18, `verdicts/S11_95044c4b.md` trên hub): phát hiện được chấp nhận, mã không merge.**
> PR #134 đã đóng; nhánh `dell/s11-grass-steering` giữ nguyên ở `95044c4b` làm bằng chứng. Mọi file mã ghi chú này
> nhắc tới (`grass_steering_tests.rs`, `sel2_grass_steering.rs`, khoá `live.grass_steering`, `GRASS_STAY_FRACTION`,
> `GRASS_SCAN_RADIUS`) **chỉ có trên nhánh đó**. Trên thân cây, `LIVE_KEYS.len()` vẫn là 24.
>
> Đọc kết quả: ở phép thử cơ chế (bỏ khát, bỏ chết, bỏ sinh), năng lượng trung bình của cả hai nhánh nằm trong
> 0,89–0,98 mức mục tiêu suốt 3.000 tick. Khi không phải đi tìm nước, con mồi **không** thiếu ăn, và lái tới cỏ chỉ
> thêm quãng đi bộ. Trong thế giới SEL2-E đầy đủ, khát gây 72–76 % số chết (đối chứng `prey_cap` 80, §5), và mọi hồ
> nằm trên cùng một hàng `z = +60` trong khi đàn sáng lập ở `z = −30` (§4, trở ngại 1). Bước kế tiếp vì vậy là thẻ
> S12 (`live.lake_layout`, rải hồ trên lưới hai chiều), không phải chỉnh hằng số của S11.
>
> Hai chỉ tiêu cũ của `DONE WHEN #2` do lead đặt sai và đã được thay bằng phép thử cơ chế: số con "đủ ngưỡng sinh"
> đọc ở cuối tick luôn gần 0 theo cấu tạo (§4, trở ngại 3), và cỏ dưới chân phản ánh tốc độ gặm trụi ô chứ không phải
> hướng lái (§4, trở ngại 2). Đính chính tương ứng nằm trong
> [ghi chú SEL2-E](2026-11-18-sel2e-exploration.md).

> Trạng thái của worker khi nộp: **`finding — not ready`** theo `DECISION S11 226d4b29` (dừng theo đúng nhánh dừng của điều kiện `#2(a)` mà **không** tinh chỉnh `GRASS_STAY_FRACTION = 0.25` hay `GRASS_SCAN_RADIUS = 3`; giữ commit kèm test `#2` đỏ làm phát hiện khoa học).
> Nhánh: `dell/s11-grass-steering` | Base commit (`origin/feat/ecological-selection`): `dea18583881a12ba54ea719f9b6d78dd6d18b05e`.

## 1. Phạm vi thay đổi (đúng theo `AUTHORIZATION`)

- `src-tauri/src/core/resources.rs`: thêm trường `pub grass_steering: bool` vào `LocomotionPolicy` (mặc định `false` trong `LocomotionPolicy::disabled()`).
- `src-tauri/src/core/locomotion.rs`: thêm hai hằng `GRASS_STAY_FRACTION = 0.25` và `GRASS_SCAN_RADIUS = 3`, cập nhật `steering_drive_system` cho thú ăn cỏ (`Option<&Prey>`, `predator.is_none()`): giữ nguyên ưu tiên cao nhất cho tìm nước (`thirst_steering`, khi `hydration < 0.50 * hydration_target` và `lake_cache` khác rỗng). Khi không hướng tới nước: (1) nếu ô cỏ hiện tại có `cur_r_max > 0.0` và `cur_grazeable >= 0.25 * cur_r_max`, đặt ngay `target_velocity = Vec3::ZERO` (đứng gặm, không chạy theo `Food`); (2) ngược lại quét cửa sổ `(2 * GRASS_SCAN_RADIUS + 1)^2` (`7x7`) đã cắt biên lưới để tìm ô có `grazeable` lớn nhất vượt hẳn `cur_grazeable` (hoà lấy `(row, col)` nhỏ nhất): nếu thấy thì hướng tới tâm ô đó trừ khi có `Food` **gần hơn hẳn** (`min_food_dist_sq < grass_dist_sq`), còn nếu không có ô nào hơn ô hiện tại thì giữ fallback về `Food`. Kẻ săn mồi giữ nguyên hành vi cũ; không cấp phát heap trong vòng lặp ấm.
- `src-tauri/src/core/live_experiment.rs`: khai báo `LIVE_KEY_GRASS_STEERING = "live.grass_steering"`, mở rộng `LIVE_KEYS` lên `25` khoá, thêm trường `grass_steering: bool` vào `LiveWorldConfig`, kiểm tra giá trị boolean chặt (`0.0`/`1.0`) và từ chối `live.grass_steering=1` khi `live.locomotion=0` ngay bước preflight (`InvalidLaw`), và truyền xuống `LocomotionPolicy` trong `build_live_world`.
- `src-tauri/tests/e2_seam_tests.rs` & `src-tauri/tests/eco_card_b_tests.rs`: cập nhật kiểm tra `LIVE_KEYS.len() == 25` và trường `grass_steering: false` trong cấu trúc tường minh của `eco_card_b_tests`.
- `src-tauri/tests/grass_steering_tests.rs`: 8 kiểm thử đơn vị/tích hợp (`0 #[ignore]`) bao phủ `DONE WHEN #1`, `DONE WHEN #2` mới (theo `DECISION S11 226d4b29`), `#3`, `#4`, và các kiểm thử hợp đồng `WHAT`.
- `src-tauri/examples/sel2_grass_steering.rs`: driver đo thăm dò 100.000 tick (`DONE WHEN #7`) trên `scale in {1.0, 0.7, 0.5}` với `prey_cap = 320` (seeds `1..=6`) và đối chứng `prey_cap = 80` (`scale = 1.0`, seeds `1..=6`), chỉ đọc 7 biến quan sát được phép (mù khối lượng cơ thể).

## 2. Kết quả kiểm tra đạt trong đợt này (`#1`, `#3`, `#4`, `#5`, `WHAT`, `#6` tĩnh/`dataflow`)

- **DONE WHEN #1** (`absent_and_zero_match_every_sampled_tick_with_locomotion_on`): khớp từng bit toàn bộ 16 mẫu `S11_TRAJ` ở thế giới A (`tick 100..=600`: `0xfc93dfcc`, `0x0ece3108`, `0x2b2d8654`, `0xb798318f`, `0x24683ee3`, `0x3b935a42`) và thế giới B (`tick 100..=1000`: `0xd1f70df2`, `0xdd224cdd`, `0x1b2ea787`, `0x73b828c6`, `0x8878f271`, `0x7cbd9f17`, `0x20b394a5`, `0xb2908b42`, `0x44534e87`, `0x1a062d2f`) so với commit nền `dea18583`.
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
- **Kiểm thử đơn vị hợp đồng `WHAT`** (`prey_stays_above_threshold_and_respects_s10_plant_reserve`, `window_clipping_tie_breaking_food_strictly_closer_thirst_priority_and_predators_unchanged`, `key_is_registered_and_refused_at_preflight_without_locomotion`, và helper tiến trình con `s11_child_entry_point`): đều đạt (tổng 7/8 test trong `grass_steering_tests.rs` đạt, 1 test `#2` mới đỏ như phát hiện khoa học).
- **DONE WHEN #6** (gate tĩnh và `dataflow`):
  - `cargo fmt --check`: đạt (0 diff).
  - `cargo clippy --all-targets --features desktop -- -D warnings`: đạt (0 cảnh báo).
  - `cargo clippy --all-targets --no-default-features -- -D warnings`: đạt (0 cảnh báo).
  - `cargo modules dependencies --lib --no-externs --package anima-engine` + `scripts/verify_dataflow.py dataflow.json dataflow-truth.dot 1`: `QUA CONG — 16 edge khop hoan toan voi cargo-modules` (`dataflow.json` không cần sửa; `src-tauri/src/**` giữ nguyên từ `226d4b29`).
  - Lưu ý: chưa chạy `cargo test --features desktop > out.txt 2>&1` toàn workspace + `node scripts/check_test_targets.mjs` vì test `#2` mới được giữ ở trạng thái đỏ để ghi nhận kết quả âm của mệnh đề `(a)` theo `DECISION S11 226d4b29` (`src-tauri/tests/grass_steering_tests.rs` có 8 test, `0 #[ignore]`).

## 3. Kết quả `DONE WHEN #2` mới — Phép thử cơ chế có đối chứng (`DECISION S11 226d4b29`)

Cấu hình: `live.founders=20`, `live.predator_fraction=0`, `live.locomotion=1`, `live.drive_floor=0.5`, `live.thirst_steering=0`, `live.mortality=0`, `live.reproduction=0`, `live.resource_capacity_scale=0.7`, `live.lakes=8`, `seed=2`, `3.000` tick, lấy mẫu mỗi `500` tick từ `1.000` đến `3.000` (5 mẫu). Tất cả 10 khoá đều được preflight chấp nhận nguyên trạng. Nhánh A dùng `live.grass_steering=0`, Nhánh B dùng `live.grass_steering=1`.

```text
S11_MECHANISM tick=1000 arm_a_energy=0.975841 arm_b_energy=0.984370 diff=+0.008529 arm_a_depleted_zero_vel=0.000000 arm_b_depleted_zero_vel=0.000000
S11_MECHANISM tick=1500 arm_a_energy=0.956197 arm_b_energy=0.968910 diff=+0.012714 arm_a_depleted_zero_vel=0.000000 arm_b_depleted_zero_vel=0.000000
S11_MECHANISM tick=2000 arm_a_energy=0.955593 arm_b_energy=0.943615 diff=-0.011979 arm_a_depleted_zero_vel=0.000000 arm_b_depleted_zero_vel=0.000000
S11_MECHANISM tick=2500 arm_a_energy=0.941782 arm_b_energy=0.915397 diff=-0.026384 arm_a_depleted_zero_vel=0.000000 arm_b_depleted_zero_vel=0.000000
S11_MECHANISM tick=3000 arm_a_energy=0.926789 arm_b_energy=0.885736 diff=-0.041053 arm_a_depleted_zero_vel=0.000000 arm_b_depleted_zero_vel=0.000000

thread 'controlled_world_grass_steering_raises_intake_and_moves_off_depleted_cells' (21352) panicked at tests\grass_steering_tests.rs:623:9:
tick 2000: intake arm B (0.943615) did not exceed arm A (0.955593)
```

- Điều kiện **(b)** (tỉ lệ thú ăn cỏ đứng yên trên ô cỏ dưới ngưỡng `0.25 * r_max`) bằng `0.000000` ở cả hai nhánh tại cả 5 điểm mẫu (`b_dep <= a_dep` thoả ở 5/5 mẫu).
- Điều kiện **(a)** (năng lượng trung bình `energy / energy_target`) dương ở hai mẫu đầu (`+0.008529` tại `tick=1000`, `+0.012714` tại `tick=1500`) nhưng đảo dấu âm ở `tick=2000` (`-0.011979`), `tick=2500` (`-0.026384`) và `tick=3000` (`-0.041053`). Ngay cả khi đã loại bỏ khát nước, tử vong và sinh sản, chiến lược tìm cỏ theo cửa sổ cục bộ `GRASS_SCAN_RADIUS = 3` và giữ vị trí trên `GRASS_STAY_FRACTION = 0.25` không duy trì được lượng thu nạp cao hơn đối chứng qua tick `2000`. Theo `DECISION S11 226d4b29`, dừng tại đây và không tinh chỉnh hằng số.

## 4. Ba trở ngại đo được trong thế giới SEL2-E và số liệu `DONE WHEN #2` cũ (bằng chứng tham chiếu)

Ở thiết kế `DONE WHEN #2` ban đầu trên kịch bản tốt nhất từ SEL2-E (`scale=0.7`, `grace=960`, `founders=100`, `lakes=8`, `prey_cap=320`, `seed=2`, mẫu mỗi 500 tick từ `2000..=6000`), kiểm thử đo được:

```text
S11_BEHAVIOUR grass=0 tick=2000 prey_n=198 under_prey_med=0.0000 field_med=1.0000 breed_ready=0
S11_BEHAVIOUR grass=0 tick=2500 prey_n=200 under_prey_med=0.0000 field_med=1.0000 breed_ready=0
S11_BEHAVIOUR grass=0 tick=3000 prey_n=175 under_prey_med=0.0000 field_med=1.0000 breed_ready=0
S11_BEHAVIOUR grass=0 tick=3500 prey_n=126 under_prey_med=0.0000 field_med=1.0000 breed_ready=0
S11_BEHAVIOUR grass=0 tick=4000 prey_n=109 under_prey_med=0.0010 field_med=1.0000 breed_ready=0
S11_BEHAVIOUR grass=0 tick=4500 prey_n=79 under_prey_med=0.0010 field_med=1.0000 breed_ready=0
S11_BEHAVIOUR grass=0 tick=5000 prey_n=79 under_prey_med=0.0010 field_med=1.0000 breed_ready=0
S11_BEHAVIOUR grass=0 tick=5500 prey_n=87 under_prey_med=0.0000 field_med=1.0000 breed_ready=0
S11_BEHAVIOUR grass=0 tick=6000 prey_n=82 under_prey_med=0.0000 field_med=1.0000 breed_ready=0
S11_BEHAVIOUR_SUMMARY grass=0 median_under_prey=0.0000 median_field=1.0000 total_breed_ready=0
S11_BEHAVIOUR grass=1 tick=2000 prey_n=216 under_prey_med=0.0000 field_med=1.0000 breed_ready=0
S11_BEHAVIOUR grass=1 tick=2500 prey_n=208 under_prey_med=0.0000 field_med=1.0000 breed_ready=0
S11_BEHAVIOUR grass=1 tick=3000 prey_n=187 under_prey_med=0.0000 field_med=1.0000 breed_ready=0
S11_BEHAVIOUR grass=1 tick=3500 prey_n=129 under_prey_med=0.0000 field_med=1.0000 breed_ready=0
S11_BEHAVIOUR grass=1 tick=4000 prey_n=99 under_prey_med=0.0010 field_med=1.0000 breed_ready=0
S11_BEHAVIOUR grass=1 tick=4500 prey_n=70 under_prey_med=0.0000 field_med=1.0000 breed_ready=0
S11_BEHAVIOUR grass=1 tick=5000 prey_n=61 under_prey_med=0.0000 field_med=1.0000 breed_ready=0
S11_BEHAVIOUR grass=1 tick=5500 prey_n=60 under_prey_med=0.0000 field_med=1.0000 breed_ready=0
S11_BEHAVIOUR grass=1 tick=6000 prey_n=61 under_prey_med=0.0001 field_med=1.0000 breed_ready=0
S11_BEHAVIOUR_SUMMARY grass=1 median_under_prey=0.0000 median_field=1.0000 total_breed_ready=0
```

Phép đo phụ trợ bản `--release` trong khoảng `500..=6000` tick cho `rad=3` và `rad=16` (`stay=0.25`):

| Phép thử | `median_under_prey` | `median_field` | `hyd_med` (`t=2000..6000`) | `eng_max` (`t=500..6000`) | `total_breed` |
| --- | ---: | ---: | ---: | ---: | ---: |
| `rad=3, grass=0` | `0.0000` | `1.0000` | `0.000..0.420` | `0.617..0.647` | `0` |
| `rad=3, grass=1` | `0.0000` | `1.0000` | `0.000..0.390` | `0.595..0.649` | `0` |
| `rad=16, grass=1` | `0.0000` | `1.0000` | `0.000..0.482` | `0.602..0.647` | `0` |

Ba nguyên nhân vật lý và lịch hệ thống khiến các chỉ tiêu cũ không quan sát được:

1. **Hành trình khát từ `z = -30` lên hàng hồ `z = +60`**: Thế hệ sáng lập cách hàng 8 hồ nước tới 90 đơn vị; từ tick `2000..=6000`, `hyd_med` nằm trong `0.000..0.482 < 0.50` (`THIRST_STEERING_FRACTION`), nên ưu tiên tìm nước đè hoàn toàn tìm cỏ cho đa số cá thể khi đàn tiến lên phía bắc (`med_z: -35 -> +17`).
2. **Thời gian ăn cạn ô ngắn hơn thời gian rời ô**: Trên lưới `256 x 256` (`[-100, 100]`), mỗi ô rộng `0.78125` đơn vị; ở `drive_floor = 0.5`, tốc độ `0.75` u/s (`0.0125` u/tick) cần `62.5` tick để băng qua ô cộng thêm `tau = 30` tick tăng tốc từ nghỉ (~68 tick), trong khi `herbivore_grazing_system` hút `8.0 / 60 = 0.1333` EU/tick trên ô `Grassland` `scale = 0.7` (`r_max = 4.9` EU -> `<= 37` tick cạn sạch, `<= 9` tick dưới ngưỡng `0.25 * r_max`). Ngay cả ở `tick 500..=1500` khi `hyd_med = 0.567..0.740`, `under_med` đã là `0.0000`.
3. **Sinh sản ở cuối tick che khuất ngưỡng `0.65`**: `autonomous_reproduction_system` chạy vào cuối mỗi tick và chi `50%` năng lượng cho mọi cá thể có `energy >= 0.65 * energy_target` khi `prey_count < prey_cap` (`320`). Vì `prey_n` chỉ đạt `60..233 < 320`, mẫu cuối tick luôn thấy `eng_max = 0.595..0.649 < 0.650` (`total_breed_ready = 0`).

## 5. Số đo thăm dò 100.000 tick (`sel2_grass_steering`, `DONE WHEN #7`)

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

Ở đối chứng `prey_cap = 80` (`scale = 1`, `seeds 1..=6`), không có seed nào tuyệt chủng (`min_agents = 20..55`, `mean_agents = 78.0..90.1`), chết đói chiếm `0.238..0.278` tổng số chết (chết khát vẫn chiếm ~72–76 %). Ở `prey_cap = 320`, cả ba mức `scale = 1 / 0.7 / 0.5` đều có tuyệt chủng (`1/6`, `1/6`, `4/6`) và cho `VIABLE = no`.

## Không khẳng định

- **S11 một mình không làm thế giới SEL2 trở nên khả thi:** cả ba mức `scale = 1`, `0.7`, `0.5` tại `prey_cap = 320` đều ghi nhận `VIABLE = no` trên 100.000 tick, và trong phép thử cơ chế đối chứng `#2(a)` (không khát, không chết, không sinh sản), tìm cỏ cục bộ theo thông số mặc định chỉ cao hơn đối chứng đến tick `1500` rồi tụt xuống dưới đối chứng từ tick `2000..=3000`.
- Các lượt chạy 100.000 tick mang tính thăm dò (*exploratory*), mù khối lượng cơ thể (`body-mass blind`, tắt đột biến/di truyền), chỉ trên 6 seed cố định (`1..=6`), Windows 11 x64 (`rustc 1.98.1`, `--release`).
- Không tinh chỉnh `GRASS_STAY_FRACTION` hay `GRASS_SCAN_RADIUS`, và không nới lỏng khẳng định `(a)` trong `DONE WHEN #2` (dừng theo `DECISION S11 226d4b29` để gửi kết quả về lead/chủ dự án).
