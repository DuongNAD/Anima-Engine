---
phase: planning
feature: experiment-sel2-food-scarcity
title: Thẻ S15 — đo các mẫu đối chứng ứng viên cho tỉ số standing-crop g tại 16 hồ trải rộng (scale 0,63)
description: Đo 24 lượt 200.000 tick (6 lượt khan hiếm prey_cap=320 / scale=0.63 trùng khớp tuyệt đối với S14 trên 6/6 seed, cùng 18 lượt đối chứng ứng viên ở scale=1.0 với prey_cap trong {80, 120, 160} và chuẩn g_pooled80); trình bày bảng theo từng seed, SUMMARY công bố (VIABLE = no) và cột C2 chẩn đoán cho từng ứng viên.
status: completed
owner: simulation-architecture
last_reviewed: 2026-11-19
---

# Thẻ S15 — đo các mẫu đối chứng ứng viên cho tỉ số `g` tại `16` hồ trải rộng (`scale = 0,63`)

- **Nhánh / Worktree:** `dell/s15-reference-check` (`E:\03_Development_Projects\project\anima-wt\s15`)
- **Base ban đầu (`origin/feat/ecological-selection`):** `ac5db986b4e29015e58a7c1caa1a429277652d7f` (sau đó merge trunk `1109da5e4ce5d02ea16c07c8b7a613ab7b782851` gồm `#139`, `#142`, `#143`, giữ nguyên mọi hàng trong `docs/ai/README.md`).
- **Phạm vi thay đổi (`AUTHORIZATION`):** chỉ đọc mô phỏng; không sửa bất kỳ file nào dưới `src-tauri/src/`, `src-tauri/tests/` hay `Cargo.toml`. Toàn bộ code đo nằm trong [`src-tauri/examples/sel2_reference_check.rs`](../../../src-tauri/examples/sel2_reference_check.rs), đọc đúng 7 observable trong danh sách cho phép (mù với khối lượng cơ thể).

## 1. Cấu hình thế giới, các ứng viên đối chứng và máy đo

- **Cố định (`sel2_reference_check.rs`, khớp S14):** `founders = 100`, `predator_fraction = 0`, `trees = 8`, `live.lakes = 16`, `live.lake_layout = 1`, `live.dehydration_ticks = 960`, `live.plant_reserve = 0`, `mortality = 1`, `reproduction = 1`, `locomotion = 1`, `drive_floor = 0.5`, `thirst_steering = 1`, `seeds = 1..=6`, `TICKS = 200.000`, lấy mẫu mỗi `50` tick.
- **`24` lượt chạy:**
  - **Nhánh khan hiếm (`6` lượt):** `prey_cap = 320`, `scale = 0.63`, `seeds = 1..=6`.
  - **Nhánh đối chứng ứng viên (`18` lượt, `scale = 1.0`):** `prey_cap ∈ {80, 120, 160}` × `seeds = 1..=6`.
- **Năm thước đo `g` báo cáo (chỉ `g_80` dùng trong `VIABLE` công bố):**
  - `g_80` *(luật công bố)*: `crop(0.63, seed) / crop(prey_cap=80, scale=1.0, seed)` (`floor = 0`).
  - `g_120` *(diagnostic, not the declared rule)*: `crop(0.63, seed) / crop(prey_cap=120, scale=1.0, seed)`.
  - `g_160` *(diagnostic, not the declared rule)*: `crop(0.63, seed) / crop(prey_cap=160, scale=1.0, seed)`.
  - `g_pooled80` *(diagnostic, not the declared rule)*: `crop(0.63, seed) / mean(crop(prey_cap=80, scale=1.0, các seed KHÔNG tuyệt chủng))`. Ở `seeds 1..=6`, `4/6` lượt `prey_cap = 80` sống sót (`seeds 2, 4, 5, 6`), cho `pooled80_crop = 113430.7`.
  - `g_ungrazed` *(diagnostic, not the declared rule)*: `crop(0.63, seed) / sum_r_max(scale=1.0)` (`153587.2`).

### Lệnh chạy, thời gian và thông số máy

- **Lệnh đo:** `cargo run --release --example sel2_reference_check` (`CARGO_BUILD_JOBS=4`, `CARGO_INCREMENTAL=0`, `ANIMA_USE_GPU=0`, `ANIMA_SEL2_THREADS=4`).
- **Máy đo (`DESKTOP-IT8UKCD`):** `13th Gen Intel(R) Core(TM) i7-1355U` (`10` nhân / `12` luồng), Windows 11 x64 (`10.0.26300`), nguồn AC (`BatteryStatus = 2`, `100%`), `rustc 1.98.1 (48a229cea 2026-09-01)`, `cargo 1.98.1 (797e8a9bc 2026-08-05)`.
- **Thời gian chạy (`STAGE_WALL`):** `STAGE_WALL stage=S15 lakes=16 scale=0.63 reserve=0 runs=24 threads=4 wall_secs=2502.93 wall_mins=41.72` (`41.72` phút).

## 2. Đối chiếu tái lập S14 (`prey_cap = 320, scale = 0,63`, `6/6` seed)

Cả `6/6` lượt khan hiếm ở S15 **khớp tuyệt đối từng con số** với kết quả S14 trên Dell (`S14_PARITY_OVERALL all_6_seeds_exact_match=yes`):

| Seed | `g_80` (S14 / S15) | `min_agents` (S14 / S15) | `mean_agents` (S14 / S15) | `final_agents` (S14 / S15) | `births` (S14 / S15) | `deaths` (S14 / S15) | `starvation` (S14 / S15) | `dehydration` (S14 / S15) | `last_q b/d` (S14 / S15) | Khớp |
|---:|---|---|---:|---:|---:|---:|---:|---:|---|---|
| `1` | `0.322` / `0.322` | `9` / `9` | `123.7` / `123.7` | `130` / `130` | `5255` / `5255` | `5225` / `5225` | `3948` / `3948` | `1277` / `1277` | `1330/1307` / `1330/1307` | **`yes`** |
| `2` | `0.528` / `0.528` | `20` / `20` | `101.9` / `101.9` | `126` / `126` | `3240` / `3240` | `3214` / `3214` | `2321` / `2321` | `893` / `893` | `996/993` / `996/993` | **`yes`** |
| `3` | `0.299` / `0.299` | `10` / `10` | `133.4` / `133.4` | `161` / `161` | `5682` / `5682` | `5621` / `5621` | `4260` / `4260` | `1361` / `1361` | `1526/1512` / `1526/1512` | **`yes`** |
| `4` | `0.520` / `0.520` | `41` / `41` | `115.8` / `115.8` | `91` / `91` | `3616` / `3616` | `3625` / `3625` | `2588` / `2588` | `1037` / `1037` | `971/990` / `971/990` | **`yes`** |
| `5` | `0.546` / `0.546` | `25` / `25` | `96.7` / `96.7` | `143` / `143` | `3171` / `3171` | `3128` / `3128` | `2254` / `2254` | `874` / `874` | `840/827` / `840/827` | **`yes`** |
| `6` | `0.558` / `0.558` | `9` / `9` | `78.2` / `78.2` | `85` / `85` | `2573` / `2573` | `2588` / `2588` | `1876` / `1876` | `712` / `712` | `804/843` / `804/843` | **`yes`** |

## 3. Bảng theo từng seed (`sống sót đối chứng`, `standing crop`, và mọi tỉ số `g`)

| Seed | `b80_ext_tick` | `b80_crop` | `b120_ext_tick` | `b120_crop` | `b160_ext_tick` | `b160_crop` | `pooled80_crop` | `scarcity_crop` (`0.63`) | `g_80` (luật công bố) | `g_120` | `g_160` | `g_pooled80` | `g_ungrazed` |
|---:|---|---:|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `1` | **`8545`** | `153586.9` | `none` | `89683.8` | `none` | `73562.6` | `113430.7` | `49464.7` | `0.322` | `0.552` | `0.672` | `0.436` | `0.322` |
| `2` | `none` | `114041.5` | `none` | `98141.0` | `none` | `86893.5` | `113430.7` | `60241.4` | `0.528` | `0.614` | `0.693` | `0.531` | `0.392` |
| `3` | **`20787`** | `153586.7` | `none` | `88013.1` | `none` | `75138.8` | `113430.7` | `45969.4` | `0.299` | `0.522` | `0.612` | `0.405` | `0.299` |
| `4` | `none` | `113036.9` | `none` | `98903.4` | `none` | `87654.4` | `113430.7` | `58745.2` | `0.520` | `0.594` | `0.670` | `0.518` | `0.382` |
| `5` | `none` | `113080.4` | `none` | `98915.1` | `none` | `88693.7` | `113430.7` | `61760.6` | `0.546` | `0.624` | `0.696` | `0.544` | `0.402` |
| `6` | `none` | `113563.7` | `none` | `97837.6` | `none` | `89716.0` | `113430.7` | `63417.1` | `0.558` | `0.648` | `0.707` | `0.559` | `0.413` |

### Nhận xét chính theo số đo

1. **Sống sót của đối chứng:** Nâng `prey_cap` lên `120` hoặc `160` xoá hoàn toàn tuyệt chủng trên cả `6/6` seed (`0/6` tuyệt chủng, so với `2/6` ở `prey_cap = 80`).
2. **Độ gặm cỏ của đối chứng:** Đối chứng `prey_cap = 120` gặm mạnh hơn đáng kể (`crop = 88.013–98.915`, tức `0.573–0.644` đồng cỏ không gặm) so với `prey_cap = 80` sống sót (`113.037–114.042`, tức `0.736–0.743`), và `prey_cap = 160` gặm còn mạnh hơn nữa (`73.563–89.716`, tức `0.479–0.584`). Vì vậy ở `scale = 0.63`, `g_120` bị đẩy vượt `0.6` ở `seeds 2, 5, 6` (`0.614, 0.624, 0.648`), còn `g_160` vượt `0.6` ở cả `6/6` seed (`0.612..0.707`).
3. **Chênh lệch giữa `seeds 1, 3` và `seeds 2, 4, 5, 6` ở `scale = 0.63`:** Bản thân `scarcity_crop` của `seed 1` (`49.464,7`) và `seed 3` (`45.969,4`) thấp hơn `seeds 2, 4, 5, 6` (`58.745,2–63.417,1`) vì quần thể ở hai seed này lớn hơn (`mean_agents = 123.7` và `133.4`), nên ngay cả khi chia cho cùng mẫu số `pooled80_crop = 113.430,7`, `g_pooled80` của cả `6/6` seed rơi gọn vào `[0.405, 0.559]` (`seeds 1, 3` là `0.436, 0.405`; `seeds 2, 4, 5, 6` là `0.531, 0.518, 0.544, 0.559`).

## 4. `SUMMARY` công bố và bảng `C2` chẩn đoán cho từng ứng viên

### `SUMMARY` theo luật công bố (`g_80`, không thay đổi định nghĩa `VIABLE`)

| `lakes` | `scale` | `reserve` | `prey_cap` | `extinctions` | `c1_min_agents` | `mean_agents_med` | `c2_g80_med` | `c2_g80_min..max` | `c3_generations_med` | `c3_generations_min` | `c4_share_starvation` | `c4_share_dehydration` | `c4_share_min..max` | `c5_seeds_active` | `VIABLE` (luật công bố) | `failed_conditions` |
| ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | --- | --- | --- | --- |
| `16` | `0.63` | `0` | `320` | `0/6` | `9` | `108.9` | `0.524` | `0.299..0.558` | `32.8` | `31.2` | `0.737` | `0.263` | `0.714..0.758` | `6/6` | `no` | `C2(g_out_of_[0.4,0.6])` |

### `SUMMARY_REF` — phán quyết `C2` (`0.4 <= g <= 0.6` trên cả 6 seed) cho từng ứng viên

| Ứng viên | Tuyệt chủng ở đối chứng | `ref_crop_min..max` | `g_med` | `g_min..max` | Số seed trong `[0.4, 0.6]` | Kết luận `C2` | Vai trò / nhãn |
|---|---|---:|---:|---:|---:|---|---|
| `g_80` | `2/6` | `113036.9..153586.9` | `0.524` | `0.299..0.558` | `4/6` | `no` | `declared rule` |
| `g_120` | `0/6` | `88013.1..98915.1` | `0.604` | `0.522..0.648` | `3/6` | `no` | `diagnostic, not the declared rule` |
| `g_160` | `0/6` | `73562.6..89716.0` | `0.683` | `0.612..0.707` | `0/6` | `no` | `diagnostic, not the declared rule` |
| `g_pooled80` | `2/6` (`4/6` sống, `mean=113430.7`) | `113430.7..113430.7` | `0.524` | `0.405..0.559` | `6/6` | `yes` | `diagnostic, not the declared rule` |
| `g_ungrazed` | `0/6` (`sum_r_max`) | `153587.2..153587.2` | `0.387` | `0.299..0.413` | `2/6` | `no` | `diagnostic, not the declared rule` |

## 5. Dữ liệu thô đầy đủ (`24/24` lượt + khối `SUMMARY` nguyên văn)

```text
STAGE_WALL stage=S15 lakes=16 scale=0.63 reserve=0 runs=24 threads=4 wall_secs=2502.93 wall_mins=41.72
POOL_REF80 surviving=4/6 mean_crop=113430.7

S14_PARITY seed	s14_g80	s15_g80	s14_min	s15_min	s14_mean	s15_mean	s14_final	s15_final	s14_births	s15_births	s14_deaths	s15_deaths	s14_starv	s15_starv	s14_dehyd	s15_dehyd	s14_lq_b/d	s15_lq_b/d	match
S14_PARITY 1	0.322	0.322	9	9	123.7	123.7	130	130	5255	5255	5225	5225	3948	3948	1277	1277	1330/1307	1330/1307	yes
S14_PARITY 2	0.528	0.528	20	20	101.9	101.9	126	126	3240	3240	3214	3214	2321	2321	893	893	996/993	996/993	yes
S14_PARITY 3	0.299	0.299	10	10	133.4	133.4	161	161	5682	5682	5621	5621	4260	4260	1361	1361	1526/1512	1526/1512	yes
S14_PARITY 4	0.520	0.520	41	41	115.8	115.8	91	91	3616	3616	3625	3625	2588	2588	1037	1037	971/990	971/990	yes
S14_PARITY 5	0.546	0.546	25	25	96.7	96.7	143	143	3171	3171	3128	3128	2254	2254	874	874	840/827	840/827	yes
S14_PARITY 6	0.558	0.558	9	9	78.2	78.2	85	85	2573	2573	2588	2588	1876	1876	712	712	804/843	804/843	yes
S14_PARITY_OVERALL all_6_seeds_exact_match=yes

PER_SEED seed	b80_ext_tick	b80_crop	b120_ext_tick	b120_crop	b160_ext_tick	b160_crop	pooled80_crop	ungrazed_crop	scarcity_crop	g_80	g_120	g_160	g_pooled80	g_ungrazed
PER_SEED 1	8545	153586.9	none	89683.8	none	73562.6	113430.7	153587.2	49464.7	0.322	0.552	0.672	0.436	0.322
PER_SEED 2	none	114041.5	none	98141.0	none	86893.5	113430.7	153587.2	60241.4	0.528	0.614	0.693	0.531	0.392
PER_SEED 3	20787	153586.7	none	88013.1	none	75138.8	113430.7	153587.2	45969.4	0.299	0.522	0.612	0.405	0.299
PER_SEED 4	none	113036.9	none	98903.4	none	87654.4	113430.7	153587.2	58745.2	0.520	0.594	0.670	0.518	0.382
PER_SEED 5	none	113080.4	none	98915.1	none	88693.7	113430.7	153587.2	61760.6	0.546	0.624	0.696	0.544	0.402
PER_SEED 6	none	113563.7	none	97837.6	none	89716.0	113430.7	153587.2	63417.1	0.558	0.648	0.707	0.559	0.413

layout	scale	grace	founders	lakes	prey_cap	reserve	seed	sum_r_max	extinct_tick	crop_second_half	g_80	g_120	g_160	g_pooled80	g_ungrazed	min_agents	mean_agents	final_agents	births	deaths	starvation	dehydration	starvation_share	dehydration_share	mean_lifespan	gen_by_lifespan	gen_by_turnover	generations	births_last_q	deaths_last_q
1	1	960	100	16	80	0	1	153587.2	8545	153586.9	NaN	NaN	NaN	NaN	1.000	0	1.8	0	37	137	109	28	0.796	0.204	2129.0	93.9	20.3	20.3	0	0
1	1	960	100	16	80	0	2	153587.2	none	114041.5	NaN	NaN	NaN	NaN	0.743	74	104.9	112	2360	2348	1024	1324	0.436	0.564	7387.0	27.1	22.5	22.5	555	536
1	1	960	100	16	80	0	3	153587.2	20787	153586.7	NaN	NaN	NaN	NaN	1.000	0	2.6	0	105	205	162	43	0.790	0.210	2039.4	98.1	40.6	40.6	0	0
1	1	960	100	16	80	0	4	153587.2	none	113036.9	NaN	NaN	NaN	NaN	0.736	67	102.9	118	2492	2474	1105	1369	0.447	0.553	6867.3	29.1	24.2	24.2	585	550
1	1	960	100	16	80	0	5	153587.2	none	113080.4	NaN	NaN	NaN	NaN	0.736	80	104.2	84	2483	2499	1157	1342	0.463	0.537	7014.6	28.5	23.8	23.8	520	566
1	1	960	100	16	80	0	6	153587.2	none	113563.7	NaN	NaN	NaN	NaN	0.739	77	103.4	133	2573	2540	1153	1387	0.454	0.546	6726.6	29.7	24.9	24.9	594	581
1	1	960	100	16	120	0	1	153587.2	none	89683.8	NaN	NaN	NaN	NaN	0.584	26	142.0	152	4992	4940	2451	2489	0.496	0.504	4920.5	40.6	35.2	35.2	1276	1255
1	1	960	100	16	120	0	2	153587.2	none	98141.0	NaN	NaN	NaN	NaN	0.639	82	137.0	157	4169	4112	1999	2113	0.486	0.514	5703.0	35.1	30.4	30.4	1016	983
1	1	960	100	16	120	0	3	153587.2	none	88013.1	NaN	NaN	NaN	NaN	0.573	12	140.8	162	5036	4974	2371	2603	0.477	0.523	4794.7	41.7	35.8	35.8	1280	1296
1	1	960	100	16	120	0	4	153587.2	none	98903.4	NaN	NaN	NaN	NaN	0.644	28	115.6	165	3820	3755	2001	1754	0.533	0.467	5224.5	38.3	33.0	33.0	1027	992
1	1	960	100	16	120	0	5	153587.2	none	98915.1	NaN	NaN	NaN	NaN	0.644	27	129.3	156	3971	3915	1957	1958	0.500	0.500	5628.7	35.5	30.7	30.7	1008	982
1	1	960	100	16	120	0	6	153587.2	none	97837.6	NaN	NaN	NaN	NaN	0.637	70	140.3	120	3987	3967	1784	2183	0.450	0.550	5962.0	33.5	28.4	28.4	893	927
1	1	960	100	16	160	0	1	153587.2	none	73562.6	NaN	NaN	NaN	NaN	0.479	28	172.0	163	6667	6604	3431	3173	0.520	0.480	4468.1	44.8	38.8	38.8	1574	1589
1	1	960	100	16	160	0	2	153587.2	none	86893.5	NaN	NaN	NaN	NaN	0.566	70	152.2	184	5355	5271	2911	2360	0.552	0.448	5016.1	39.9	35.2	35.2	1323	1309
1	1	960	100	16	160	0	3	153587.2	none	75138.8	NaN	NaN	NaN	NaN	0.489	37	174.3	197	6545	6448	3202	3246	0.497	0.503	4590.4	43.6	37.5	37.5	1488	1480
1	1	960	100	16	160	0	4	153587.2	none	87654.4	NaN	NaN	NaN	NaN	0.571	57	163.7	203	5171	5068	2507	2561	0.495	0.505	5525.0	36.2	31.6	31.6	1268	1251
1	1	960	100	16	160	0	5	153587.2	none	88693.7	NaN	NaN	NaN	NaN	0.577	79	153.5	160	5322	5262	2829	2433	0.538	0.462	5056.7	39.6	34.7	34.7	1288	1262
1	1	960	100	16	160	0	6	153587.2	none	89716.0	NaN	NaN	NaN	NaN	0.584	80	147.8	149	5200	5151	2866	2285	0.556	0.444	5008.6	39.9	35.2	35.2	1344	1345
1	0.63	960	100	16	320	0	1	96760.0	none	49464.7	0.322	0.552	0.672	0.436	0.322	9	123.7	130	5255	5225	3948	1277	0.756	0.244	4272.8	46.8	42.5	42.5	1330	1307
1	0.63	960	100	16	320	0	2	96760.0	none	60241.4	0.528	0.614	0.693	0.531	0.392	20	101.9	126	3240	3214	2321	893	0.722	0.278	5656.8	35.4	31.8	31.8	996	993
1	0.63	960	100	16	320	0	3	96760.0	none	45969.4	0.299	0.522	0.612	0.405	0.299	10	133.4	161	5682	5621	4260	1361	0.758	0.242	4260.1	46.9	42.6	42.6	1526	1512
1	0.63	960	100	16	320	0	4	96760.0	none	58745.2	0.520	0.594	0.670	0.518	0.382	41	115.8	91	3616	3625	2588	1037	0.714	0.286	5802.1	34.5	31.2	31.2	971	990
1	0.63	960	100	16	320	0	5	96760.0	none	61760.6	0.546	0.624	0.696	0.544	0.402	25	96.7	143	3171	3128	2254	874	0.721	0.279	5487.5	36.4	32.8	32.8	840	827
1	0.63	960	100	16	320	0	6	96760.0	none	63417.1	0.558	0.648	0.707	0.559	0.413	9	78.2	85	2573	2588	1876	712	0.725	0.275	5415.7	36.9	32.9	32.9	804	843

SUMMARY stage	lakes	scale	reserve	prey_cap	extinctions	c1_min_agents	mean_agents_med	c2_g80_med	c2_g80_min..max	c3_generations_med	c3_generations_min	c4_share_starvation	c4_share_dehydration	c4_share_min..max	c5_seeds_active	VIABLE	failed_conditions
SUMMARY S15	16	0.63	0	320	0/6	9	108.9	0.524	0.299..0.558	32.8	31.2	0.737	0.263	0.714..0.758	6/6	no	C2(g_out_of_[0.4,0.6])

SUMMARY_REF candidate	ref_extinct	ref_crop_min..max	g_med	g_min..max	c2_in_band_seeds	c2_verdict	rule_status
SUMMARY_REF g_80	2/6	113036.9..153586.9	0.524	0.299..0.558	4/6	no	declared rule
SUMMARY_REF g_120	0/6	88013.1..98915.1	0.604	0.522..0.648	3/6	no	diagnostic, not the declared rule
SUMMARY_REF g_160	0/6	73562.6..89716.0	0.683	0.612..0.707	0/6	no	diagnostic, not the declared rule
SUMMARY_REF g_pooled80	2/6	113430.7..113430.7	0.524	0.405..0.559	6/6	yes	diagnostic, not the declared rule
SUMMARY_REF g_ungrazed	0/6	153587.2..153587.2	0.387	0.299..0.413	2/6	no	diagnostic, not the declared rule
```

## 6. Các cổng kiểm tra tĩnh (`DONE WHEN #1`)

- `cargo fmt --check`: đạt (exit `0`).
- `cargo clippy --all-targets --features desktop -- -D warnings`: đạt (exit `0`).
- `cargo clippy --all-targets --no-default-features -- -D warnings`: đạt (exit `0`).
- `npm run check:docs` (`check_docs_links.mjs`, `check_docs_metadata.mjs`, `check_ipc_reference.mjs`): đạt (exit `0`).

## 7. Không khẳng định

- **Phép đo thăm dò và vai trò của chủ dự án:** toàn bộ số đo chỉ là thăm dò trên tập `seeds 1..=6` (không phải seed xác nhận); mọi cột ngoài `g_80` đều là **chẩn đoán, không phải luật công bố (`diagnostic, not the declared rule`)**. **Việc chọn một mẫu đối chứng sau khi xem các con số này là quyết định thuộc về chủ dự án, phải được tuyên bố trong tiền đăng ký `SEL2-P` trước khi chạy bất kỳ seed xác nhận nào.**
- **Mù với khối lượng cơ thể:** driver `sel2_reference_check.rs` chỉ đọc đúng `7` observable được phép (`live.agent_count`, `live.standing_crop`, `live.births`, `live.deaths`, `live.mean_lifespan`, `live.deaths_starvation`, `live.deaths_dehydration`), hoàn toàn mù với khối lượng cơ thể và mọi đặc điểm tiến hoá.
- **Một máy đơn lẻ:** toàn bộ số đo thực hiện trên một máy (`Dell Vostro 3530`, Windows 11 x64, `rustc 1.98.1`, `--release`, `ANIMA_SEL2_THREADS = 4`); không khẳng định tính bất biến số học giữa các nền tảng.
- **Không sửa `src-tauri/src/` hay nới ngưỡng:** không chạm bất kỳ file nào dưới `src-tauri/src/` hay `src-tauri/tests/`.
