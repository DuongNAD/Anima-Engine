---
phase: planning
feature: experiment-sel2-food-scarcity
title: Thẻ S12 — khoá live.lake_layout (rải hồ nước theo lưới 2-D khi khởi tạo)
description: Khoá live.lake_layout (LIVE_KEYS.len() = 25); layout=1 rút khoảng cách lớn nhất từ founder tới hồ gần nhất (92,2482 -> 52,0183) và giảm chết khát 20.000 tick từ 181 xuống 77; 1505/1505 test desktop đạt nhưng đo 100.000 tick sel2_lake_layout cho VIABLE = no ở cả 6 ô
status: completed
owner: simulation-architecture
last_reviewed: 2026-11-18
---

# Thẻ S12 — khoá `live.lake_layout` (rải hồ nước theo lưới 2-D khi khởi tạo)

- **Nhánh:** `dell/s12-lake-layout`
- **Base commit (`origin/feat/ecological-selection`):** `dea18583881a12ba54ea719f9b6d78dd6d18b05e`
- **Thay thế nút thắt từ S11:** Vì phép thử cơ chế S11 cho thấy khi tắt khát nước thú ăn cỏ giữ mức `0,89–0,98` `energy_target` (không thiếu cỏ), còn ở thế giới SEL2-E chết khát chiếm tới 71–80 % (S9) / 72–76 % (S11 đối chứng `prey_cap = 80`), S12 kiểm nghiệm trực tiếp việc tách `n` hồ nước khỏi hàng ngang đơn `z_fraction = 0,8` (`z = +60`) sang lưới 2-D toàn bản đồ tại bước `genesis`.

## 1. Công thức vị trí hồ và phạm vi thay đổi (`AUTHORIZATION`)

- **`src-tauri/src/core/live_experiment.rs`**:
  - Khai báo `LIVE_KEY_LAKE_LAYOUT = "live.lake_layout"` (`0.0` / `1.0`, vắng mặt = `0.0`), nâng `LIVE_KEYS` từ `24` lên `25` khoá, thêm trường `pub lake_layout: u8` (`#[serde(default)]`, mặc định `0`) vào `LiveWorldConfig`.
  - Kiểm tra giá trị boolean chặt (`0.0` / `1.0`, từ chối ngoài đoạn hoặc `NaN`/`Infinity`) và từ chối `live.lake_layout = 1.0` đi kèm `live.lakes = 0` ngay bước preflight (`ExperimentError::InvalidLaw`).
  - Trong `genesis`: chỉ đổi vị trí `Lake` khi `config.lake_layout == 1`; khi `0` vẫn gọi nguyên `lattice_point(i, config.lakes.max(1), bounds, 0.8)` (không sửa hàm `lattice_point`, không đụng `founders` tại `0.35`, `trees` tại `0.6`, không dùng RNG, không sửa vòng lặp mỗi tick).
  - Hàm chị em `lake_lattice_point(i, n, bounds)`: đặt `k = ceil(sqrt(n))` cột, `m = ceil(n / k)` hàng; hồ `i` ở cột `c = i % k`, hàng `r = i / k`, toạ độ `x = min.x + (c + 1) / (k + 1) * span_x`, `z = min.z + (r + 1) / (m + 1) * span_z` (`y = 0`). Nếu hàng cuối (`r == m - 1`) chưa đủ `k` hồ (`j = n - (m - 1) * k`), hàng cuối được căn giữa bằng phân số theo `x` là `(c + 1) / (j + 1)`.
- **`src-tauri/tests/e2_seam_tests.rs` & `src-tauri/tests/eco_card_b_tests.rs`**: cập nhật `LIVE_KEYS.len()` (`24 -> 25`).
- **`src-tauri/tests/lake_layout_tests.rs`**: 6 test (`0 #[ignore]`) kiểm chứng preflight và `DONE WHEN #1`–`#4`.
- **`src-tauri/examples/sel2_lake_layout.rs`**: driver đo thăm dò 100.000 tick (`DONE WHEN #7`), chỉ đọc 7 biến quan sát được phép (mù khối lượng cơ thể).

## 2. Kết quả kiểm chứng `DONE WHEN #1`–`#6`

- **DONE WHEN #1** (`absent_and_zero_match_every_sampled_tick_with_lakes_and_locomotion_on`):
  - Thế giới A (`founders=30, lakes=8, mortality=1, reproduction=1, locomotion=1, drive_floor=0.5, thirst_steering=1, seed=7, 600 tick`) và thế giới B (`founders=20, predator_fraction=0.25, trees=4, lakes=4, mortality=0, reproduction=0, locomotion=1, drive_floor=0.5, thirst_steering=1, seed=13, 1.000 tick`): toàn bộ 16 dòng `S12_TRAJ` khi khoá vắng mặt và khi `0.0` khớp nhau từng bit và khớp từng bit với commit nền `dea18583`. Khi bật `1.0` với `dehydration_ticks=960` (`2.500` tick), quỹ đạo phân kỳ rõ khi khát nước kích hoạt.
- **DONE WHEN #2** (`lake_positions_match_formula_lie_in_bounds_are_distinct_and_closer_to_founders`):
  - Với mọi `n in {1, 2, 3, 4, 8, 9}`, toạ độ toàn bộ `Lake` khớp đúng công thức lưới 2-D (`< 1e-4`), nằm gọn bên trong `MapBounds`, phân biệt từng cặp, và vị trí 100 `founders` cùng 8 `trees` giữ nguyên so với `layout=0`.
  - Với `n = 8`: khoảng cách lớn nhất từ một founder tới hồ gần nhất giảm từ `92.2482` (`layout=0`) xuống `52.0183` (`layout=1`) (`S12_FOUNDER_LAKE_DIST n=8 layout_0_max_min=92.2482 layout_1_max_min=52.0183`).
- **DONE WHEN #3** (`sel2e_best_world_layout_1_reduces_dehydration_deaths_over_20k_ticks`):
  - Thế giới tốt nhất SEL2-E (`founders=100, lakes=8, prey_cap=320, resource_capacity_scale=0.7, dehydration_ticks=960, mortality=1, reproduction=1, locomotion=1, drive_floor=0.5, thirst_steering=1, seed=2, 20.000 tick`):
    - `layout=0`: `births=394 deaths=459 starvation=278 dehydration=181 final_agents=35`
    - `layout=1`: `births=301 deaths=372 starvation=295 dehydration=77 final_agents=29`
  - Số ca chết khát giảm rõ rệt từ `181` xuống `77` (`77 < 181`).
- **DONE WHEN #4** (`lake_layout_checksum_agrees_across_four_processes_and_keeps_closed_eu`):
  - 4 tiến trình độc lập cùng ra `0x6d89ec9b`; EU kín `start=156587.24986362457`, `worst_relative_drift=3.3455402574440523e-15 < 1e-6`.
- **DONE WHEN #5** (7 suite hồi quy trong `cargo test --features desktop`):
  - `e2_seam_tests`: `9 passed; 0 failed; 0 ignored`
  - `eco_card_b_tests`: `12 passed; 0 failed; 0 ignored`
  - `ecology_closed_ledger_integration_tests`: `6 passed; 0 failed; 0 ignored`
  - `energy_conservation_tests`: `3 passed; 0 failed; 1 ignored` (ignore sẵn từ upstream `diagnose_which_system_moves_the_closed_total`)
  - `lake_layout_tests`: `6 passed; 0 failed; 0 ignored`
  - `plant_reserve_tests`: `11 passed; 0 failed; 0 ignored`
  - `sel2_drift_control_tests`: `9 passed; 0 failed; 0 ignored`
  - `steering_zero_energy_tests`: `2 passed; 0 failed; 0 ignored`
- **DONE WHEN #6**:
  - `cargo fmt --check`: đạt (exit `0`).
  - `cargo clippy --all-targets --features desktop -- -D warnings`: đạt (`0` cảnh báo).
  - `cargo clippy --all-targets --no-default-features -- -D warnings`: đạt (`0` cảnh báo).
  - `cargo modules dependencies --lib --no-externs --package anima-engine` + `scripts/verify_dataflow.py dataflow.json dataflow-truth.dot 1`: `QUA CONG — 16 edge khop hoan toan voi cargo-modules` (`dataflow.json` không cần sửa).
  - `cargo test --features desktop > ..\out.txt 2>&1`: `153` target (`1505 passed; 0 failed; 2 ignored`).
  - `node scripts/check_test_targets.mjs out.txt --profile desktop`: `test targets: 153, empty: 3 (allow-listed 3), ignored: 2 (allow-listed 2), feature-gated running under "desktop": 8` (exit `0`).

## 3. Kết quả đo thăm dò 100.000 tick (`sel2_lake_layout`, `DONE WHEN #7`)

Cấu hình: `grace=960`, `founders=100`, `lakes=8`, `reserve=0`, `seeds 1..=6`, chế độ `--release`, `48/48` lượt chạy hoàn tất (`36` lượt ở `prey_cap=320` cho `layout in {0, 1} x scale in {1.0, 0.7, 0.5}`, cùng `12` lượt đối chứng `prey_cap=80, scale=1.0` cho từng `(layout, seed)` để tính `g` trên cùng hình học hồ). Theo đặc tả `ASSIGN S12`, không dùng số đếm `>= 0.65 * energy_target` cuối tick làm bằng chứng sinh sản vì `autonomous_reproduction_system` chạy cuối tick; dùng `births`.

### Bảng tổng hợp (`prey_cap = 320`)

| `layout` | `scale` | `prey_cap` | `extinctions` | `c1_min_agents` | `mean_agents_med` | `c2_g_med` | `c2_g_min..max` | `c3_generations_med` | `c3_generations_min` | `c4_share_starvation` | `c4_share_dehydration` | `c4_share_min..max` | `c5_seeds_active` | `VIABLE` |
| ---: | ---: | ---: | --- | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | --- | --- | --- |
| `0` | `1` | `320` | `1/6` | `0` | `135.4` | `0.710` | `0.438..0.955` | `23.1` | `21.1` | `0.471` | `0.529` | `0.430..0.502` | `6/6` | `no` |
| `0` | `0.7` | `320` | `4/6` | `0` | `32.9` | `0.678` | `0.355..1.045` | `23.6` | `21.0` | `0.538` | `0.462` | `0.506..0.607` | `2/6` | `no` |
| `0` | `0.5` | `320` | `5/6` | `0` | `10.4` | `0.505` | `0.470..0.746` | `20.4` | `19.1` | `0.658` | `0.342` | `0.591..0.866` | `1/6` | `no` |
| `1` | `1` | `320` | `1/6` | `0` | `118.8` | `0.787` | `0.515..1.360` | `23.0` | `21.1` | `0.486` | `0.514` | `0.464..0.578` | `5/6` | `no` |
| `1` | `0.7` | `320` | `1/6` | `0` | `79.2` | `0.672` | `0.404..0.750` | `19.1` | `17.4` | `0.567` | `0.433` | `0.515..0.629` | `5/6` | `no` |
| `1` | `0.5` | `320` | `4/6` | `0` | `14.4` | `0.594` | `0.308..0.680` | `19.2` | `17.8` | `0.719` | `0.281` | `0.674..0.824` | `2/6` | `no` |

Nhận xét từ bảng: tại `scale = 0.7`, rải hồ 2-D (`layout = 1`) giảm tuyệt chủng từ `4/6` xuống `1/6`, nâng trung vị quần thể từ `32.9` lên `79.2` (`births = 1,107..2,671` ở 5 seed sống đến hết 100.000 tick), tăng số seed hoạt động cuối kỳ từ `2/6` lên `5/6`, và đẩy tỉ trọng chết đói lên `0.567` (tỉ trọng chết khát giảm từ `0.462` xuống `0.433`). Tuy vậy chưa ô nào đạt chuẩn khả thi SEL2-E (`c1_min_agents = 0` vì vẫn còn 1–4 seed tuyệt chủng, `c2_g_max > 0.6` và `c3_generations_min < 30`).

### Dữ liệu từng lượt chạy (`48/48`)

```text
layout	scale	grace	founders	lakes	prey_cap	reserve	seed	sum_r_max	g	min_agents	mean_agents	final_agents	births	deaths	starvation	dehydration	starvation_share	dehydration_share	mean_lifespan	gen_by_lifespan	gen_by_turnover	generations	births_last_q	deaths_last_q
0	1	960	100	8	80	0	1	153587.2	NaN	34	95.4	106	2025	2019	703	1316	0.348	0.652	3713.4	26.9	21.2	21.2	474	460
0	1	960	100	8	80	0	2	153587.2	NaN	0	25.2	0	561	661	266	395	0.402	0.598	3137.7	31.9	22.2	22.2	0	0
0	1	960	100	8	80	0	3	153587.2	NaN	0	4.3	0	106	206	88	118	0.427	0.573	1433.5	69.8	24.9	24.9	0	0
0	1	960	100	8	80	0	4	153587.2	NaN	0	28.0	0	590	690	246	444	0.357	0.643	3337.2	30.0	21.0	21.0	0	0
0	1	960	100	8	80	0	5	153587.2	NaN	70	93.4	86	1727	1741	553	1188	0.318	0.682	4415.3	22.6	18.5	18.5	396	409
0	1	960	100	8	80	0	6	153587.2	NaN	72	92.5	97	1725	1728	510	1218	0.295	0.705	4281.8	23.4	18.7	18.7	386	396
0	0.5	960	100	8	320	0	1	76793.6	0.470	8	66.8	134	1604	1570	999	571	0.636	0.364	3584.0	27.9	24.0	24.0	290	215
0	0.5	960	100	8	320	0	2	76793.6	0.504	0	10.7	0	215	315	224	91	0.711	0.289	3014.9	33.2	20.1	20.1	0	0
0	0.5	960	100	8	320	0	3	76793.6	0.500	0	2.8	0	116	216	187	29	0.866	0.134	1118.7	89.4	41.6	41.6	0	0
0	0.5	960	100	8	320	0	4	76793.6	0.506	0	13.7	0	262	362	214	148	0.591	0.409	3298.2	30.3	19.1	19.1	0	0
0	0.5	960	100	8	320	0	5	76793.6	0.726	0	9.9	0	206	306	194	112	0.634	0.366	2810.8	35.6	20.7	20.7	0	0
0	0.5	960	100	8	320	0	6	76793.6	0.746	0	10.2	0	201	301	201	100	0.668	0.332	3007.5	33.3	19.7	19.7	0	0
0	0.7	960	100	8	320	0	1	107511.1	0.616	20	96.4	284	2468	2284	1202	1082	0.526	0.474	3347.1	29.9	25.6	25.6	586	338
0	0.7	960	100	8	320	0	2	107511.1	0.698	0	26.0	0	564	664	403	261	0.607	0.393	3411.2	29.3	21.7	21.7	0	0
0	0.7	960	100	8	320	0	3	107511.1	0.355	17	107.5	161	2682	2621	1400	1221	0.534	0.466	3367.6	29.7	24.9	24.9	572	533
0	0.7	960	100	8	320	0	4	107511.1	0.659	0	39.8	0	834	934	473	461	0.506	0.494	3678.3	27.2	21.0	21.0	0	0
0	0.7	960	100	8	320	0	5	107511.1	1.017	0	15.3	0	370	470	264	206	0.562	0.438	2741.4	36.5	24.1	24.1	0	0
0	0.7	960	100	8	320	0	6	107511.1	1.045	0	16.5	0	380	480	269	211	0.560	0.440	2927.4	34.2	23.0	23.0	0	0
0	1	960	100	8	320	0	1	153587.2	0.647	52	147.1	113	3727	3714	1865	1849	0.502	0.498	3268.4	30.6	25.3	25.3	765	846
0	1	960	100	8	320	0	2	153587.2	0.548	31	148.2	192	3120	3028	1301	1727	0.430	0.570	4014.1	24.9	21.1	21.1	834	852
0	1	960	100	8	320	0	3	153587.2	0.438	44	148.0	159	3850	3791	1859	1932	0.490	0.510	3186.4	31.4	26.0	26.0	732	717
0	1	960	100	8	320	0	4	153587.2	0.773	0	84.9	0	1949	2049	967	1082	0.472	0.528	3527.1	28.4	23.0	23.0	3	7
0	1	960	100	8	320	0	5	153587.2	0.797	14	123.8	109	2788	2779	1265	1514	0.455	0.545	3641.0	27.5	22.5	22.5	668	688
0	1	960	100	8	320	0	6	153587.2	0.955	3	108.6	176	2526	2450	1128	1322	0.460	0.540	3582.7	27.9	23.2	23.2	722	714
1	1	960	100	8	80	0	1	153587.2	NaN	0	2.9	0	10	110	65	45	0.591	0.409	1945.1	51.4	3.4	3.4	0	0
1	1	960	100	8	80	0	2	153587.2	NaN	23	81.7	86	1480	1494	615	879	0.412	0.588	4310.2	23.2	18.1	18.1	371	388
1	1	960	100	8	80	0	3	153587.2	NaN	31	96.4	124	1892	1868	660	1208	0.353	0.647	3943.5	25.4	19.6	19.6	492	455
1	1	960	100	8	80	0	4	153587.2	NaN	0	35.7	0	726	826	392	434	0.475	0.525	3664.5	27.3	20.3	20.3	2	12
1	1	960	100	8	80	0	5	153587.2	NaN	33	85.0	110	1485	1475	579	896	0.393	0.607	4630.4	21.6	17.5	17.5	371	372
1	1	960	100	8	80	0	6	153587.2	NaN	13	77.9	93	1287	1294	466	828	0.360	0.640	4829.2	20.7	16.5	16.5	386	390
1	0.5	960	100	8	320	0	1	76793.6	0.308	2	75.0	51	1661	1710	1152	558	0.674	0.326	3775.8	26.5	22.1	22.1	411	407
1	0.5	960	100	8	320	0	2	76793.6	0.661	0	9.5	0	190	290	239	51	0.824	0.176	3025.6	33.1	20.0	20.0	0	0
1	0.5	960	100	8	320	0	3	76793.6	0.477	5	71.8	90	1627	1637	1152	485	0.704	0.296	3713.9	26.9	22.7	22.7	422	417
1	0.5	960	100	8	320	0	4	76793.6	0.536	0	12.8	0	228	328	250	78	0.762	0.238	3559.2	28.1	17.8	17.8	0	0
1	0.5	960	100	8	320	0	5	76793.6	0.653	0	16.0	0	296	396	318	78	0.803	0.197	3739.4	26.7	18.5	18.5	0	0
1	0.5	960	100	8	320	0	6	76793.6	0.680	0	9.2	0	165	265	216	49	0.815	0.185	3172.0	31.5	18.0	18.0	0	0
1	0.7	960	100	8	320	0	1	107511.1	0.404	7	113.0	139	2620	2581	1421	1160	0.551	0.449	3624.3	27.6	23.2	23.2	752	695
1	0.7	960	100	8	320	0	2	107511.1	0.718	19	66.7	78	1229	1251	746	505	0.596	0.404	4537.3	22.0	18.4	18.4	248	202
1	0.7	960	100	8	320	0	3	107511.1	0.565	10	111.4	125	2671	2646	1561	1085	0.590	0.410	3509.3	28.5	24.0	24.0	617	564
1	0.7	960	100	8	320	0	4	107511.1	0.750	0	18.1	0	358	458	288	170	0.629	0.371	3477.7	28.8	19.8	19.8	0	0
1	0.7	960	100	8	320	0	5	107511.1	0.626	16	91.6	106	1624	1618	833	785	0.515	0.485	4701.7	21.3	17.7	17.7	437	468
1	0.7	960	100	8	320	0	6	107511.1	0.736	15	63.6	55	1107	1152	658	494	0.571	0.429	4688.5	21.3	17.4	17.4	298	298
1	1	960	100	8	320	0	1	153587.2	0.515	9	153.8	159	3786	3727	1843	1884	0.494	0.506	3367.1	29.7	24.6	24.6	932	912
1	1	960	100	8	320	0	2	153587.2	0.877	18	101.0	116	2150	2134	1105	1029	0.518	0.482	3915.9	25.5	21.3	21.3	454	459
1	1	960	100	8	320	0	3	153587.2	0.735	26	147.8	102	3665	3663	1728	1935	0.472	0.528	3332.4	30.0	24.8	24.8	840	821
1	1	960	100	8	320	0	4	153587.2	0.703	45	118.3	147	2509	2462	1145	1317	0.465	0.535	3901.9	25.6	21.2	21.2	623	572
1	1	960	100	8	320	0	5	153587.2	0.838	28	119.3	82	2519	2537	1176	1361	0.464	0.536	3901.2	25.6	21.1	21.1	652	638
1	1	960	100	8	320	0	6	153587.2	1.360	0	25.9	0	644	744	430	314	0.578	0.422	2929.9	34.1	24.9	24.9	0	0
```

## Không khẳng định

- **Không khẳng định `live.lake_layout = 1` một mình làm thế giới SEL2-E trở nên khả thi:** dù rải hồ 2-D cải thiện đáng kể tỉ lệ sống sót ở `scale = 0.7` (tuyệt chủng `4/6 -> 1/6`, `mean_agents_med = 32.9 -> 79.2`, chết khát giảm ở `20.000` tick từ `181` xuống `77`), cả 6 tổ hợp `(layout, scale)` ở `prey_cap = 320` đều ghi `VIABLE = no` trên `100.000` tick vì vẫn còn tuyệt chủng (`1/6` đến `4/6`), số thế hệ tối thiểu `< 30` (`17.4..21.1`), và `c2_g_max > 0.6`.
- Các lượt đo `sel2_lake_layout` chỉ mang tính thăm dò (*exploratory*), mù khối lượng cơ thể (`body-mass blind`, chỉ đọc 7 biến quan sát được phép, tắt đột biến/di truyền), trên 6 seed cố định (`1..=6`), Windows 11 x64 (`rustc 1.98.1`, `--release`).
- Không khẳng định sự thay đổi của các đặc tả `energy >= 0.65 * energy_target` lấy mẫu ở cuối tick (do `autonomous_reproduction_system` tiêu thụ năng lượng cuối tick; bằng chứng sinh sản được đọc từ `live.births`).
