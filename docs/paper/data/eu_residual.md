---
title: Ma trận dư lượng năng lượng đóng EU (M4)
status: active
owner: maintainers
last_reviewed: 2026-11-18
review_cycle: per-release
---

# Ma trận dư lượng năng lượng đóng EU (thẻ M4)

**Câu hỏi.** Bất biến bảo toàn năng lượng đóng (`live.closed_eu_total = plants + animals + detritus + dormant`, quản lý qua `EnergyLedger`) có giữ nguyên trên **mọi tổ hợp cờ chính sách đang dùng ở SEL2** (`plant_reserve` S10, `drift_control` SEL2-D, `mortality`, `reproduction`, `locomotion`, `evolved_brains`, `lifetime_learning`, `brain_metabolic_cost`, `kill_efficiency`, `kill_hydration`, `resource_capacity_scale`) và **mọi `InterventionKind`** hay không, khi chạy qua [`LiveExperimentAdapter`](../../../src-tauri/src/core/live_experiment.rs) và lịch tick thật `core::simulation_schedule::build_tick_schedule`?

---

## Ngưỡng dung sai khai báo trước khi chạy (Bước 3)

- **Ngưỡng áp dụng cho mọi tổ hợp:** **`|ΔEU| <= RESIDUAL_ABS_TOLERANCE_EU = 1.0e-3` EU** (`src-tauri/crates/anima-domain/src/energy.rs:123`). Nếu có bất kỳ tổ hợp nào vượt ngưỡng này, dừng sửa, cam kết test trượt và báo cáo phát hiện — tuyệt đối không nới ngưỡng để test qua.
- **Lý giải theo số học và tiền lệ E2:**
  1. Mọi giao dịch qua `EnergyLedger::transfer` (`energy.rs`) đều ghi nợ ngăn nguồn đúng bằng lượng thay đổi `f64` đo được ở ngăn đích nên tự nó có sai số tích luỹ bằng `0`. Sai số dư thừa hợp pháp của cả một lượt chạy chỉ đến từ phép cộng quy chiếu `f32 -> f64` trên `256 × 256 = 65.536` ô đồng cỏ `ResourceField::r` và quỹ `HomeostaticState::energy` của từng cá thể trong `ecosystem_census_system` (`src-tauri/src/core/environmental_systems.rs:178-228`). Ở quy mô thế giới `~1,5e5–2,6e5` EU, cận làm tròn lý thuyết nằm quanh `1e-6` EU; mốc `1,0e-3` EU rộng thêm ba bậc để tránh nhiễu số thực, trong khi vẫn nhỏ hơn nhiều bậc độ lớn so với một rò rỉ thật (một mẩu thức ăn hoặc một tick chuyển hoá bị bỏ quên là hàng chục EU).
  2. Mốc thực nghiệm E2: chênh lệch ghép cặp `live.closed_eu_total` giữa nhánh đối chứng và nhánh não tiến hoá đạt **`-2,56e-9` EU** ở lần chạy tháng 7 (`9c57184`) và **`-8,49e-11` EU** ở lần chạy lại (`900ba2d`) trên tổng quỹ khoảng **`1,55e5` EU**.
- **Ranh giới bất biến D06 giữa `tick = 0` và `tick = 1`:**
  `init_world()` (`src-tauri/src/core/ecs.rs:77-82`) khởi tạo `EcosystemBiomass { detritus: 0.0, plants: starting_plants, animals: 0.0, dormant: 0.0 }`, còn `decode_genotype` (`src-tauri/src/evolution/genotype.rs:147`) cấp cho mỗi cá thể sáng lập `HomeostaticState::default().energy = 100.0` EU. `EnergyLedger::lock_baseline` (`energy.rs:156`, gọi tại `environmental_systems.rs:222`) khoá `baseline` đúng một lần sau census đầu tiên ở cuối `tick = 1` (nhất quán với `live_turnover_tests.rs:342-344` và `plant_reserve_tests.rs:269-270`). Do đó ma trận đo và đối chiếu đồng thời:
  - `ΔEU(0 -> T) = live.closed_eu_total(T) - (live.closed_eu_total(0) + live.agent_count(0) * live.mean_agent_energy(0))` (so trực tiếp tổng thực lúc `genesis` với cuối lượt chạy);
  - `ΔEU(1 -> T) = live.closed_eu_total(T) - live.closed_eu_total(1)` và đỉnh sai lệch theo từng tick `max_{1 <= t <= T} |live.closed_eu_total(t) - live.closed_eu_total(1)|` (so với `EnergyLedger::baseline()`);
  - Độ lệch gương thực vật `max_t |EcosystemBiomass::plants - ResourceField::total_biomass()|`.

---

## Kiểm kê đầy đủ `LIVE_KEYS`, nhánh `BrainPolicy` và `InterventionKind` (Bước 1)

Commit gốc đo tại `f4614665f7ccf527a6a93a5b0fcfd39aa4e3f948` (`win/m4-eu-matrix`).

### 1. Toàn bộ 24 khoá trong `LIVE_KEYS` (`src-tauri/src/core/live_experiment.rs:215-240`)

| Khoá / Tham số | Vị trí (`file:line`) | Vai trò trong dòng chảy EU & xử lý ở M4 |
|---|---|---|
| `live.evolved_brains` | `live_experiment.rs:108,221,2245,2526` | **Đưa vào ma trận (yếu tố `brain`)** — phát triển `AgentBrain` per-agent ở genesis và khi sinh sản/thay thế. |
| `BrainPolicy.lifetime_learning.enabled` | `resources.rs:781,796` | **Đưa vào ma trận (mức thứ 3 của yếu tố `brain`)** — bật cập nhật HRRL suốt đời trong `hrrl_learning_system`. |
| `BrainPolicy.brain_metabolic_cost` | `resources.rs:791`, `world_systems.rs:109,154-157` | **Đưa vào ma trận (mức thứ 3 của `brain`, `0.5` EU/s per 1k params)** — cộng `cognition_cost` vào `total_cost` trong `metabolic_decay_system` -> `detritus`. |
| `live.mortality` | `live_experiment.rs:117,222,2253` | **Đưa vào ma trận (yếu tố `mortality`)** — bật `MortalityPolicy.enabled`; `mortality_system` (`mortality.rs:32`) trả `energy` cá thể chết về `detritus`. |
| `live.starve_ticks` | `live_experiment.rs:167,228,2255` | **Đưa vào cùng yếu tố `mortality`** (`15` ở nhánh bật để chết thực sự xảy ra trong 90 tick; cũng test `120` mặc định ở cặp `drift_control`). |
| `live.dehydration_ticks` | `live_experiment.rs:170,229,2256` | **Đưa vào cùng yếu tố `mortality`** (`20` ở nhánh bật để có cả tử vong do khát). |
| `live.reproduction` | `live_experiment.rs:124,223,2260` | **Đưa vào ma trận (yếu tố `reproduction`)** — bật `ReproductionPolicy.enabled`; `autonomous_reproduction_system` (`agent_systems.rs:639`) chia quỹ mẹ cho con + đổ phí Lindeman về `detritus`. |
| `live.prey_cap` | `live_experiment.rs:179,235,2268` | **Đưa vào cùng yếu tố `reproduction`** (`14` từ `12` con mồi sáng lập để kích hoạt vừa sinh sản vừa chạm trần). |
| `live.locomotion` | `live_experiment.rs:131,224,2278` | **Đưa vào ma trận (yếu tố `locomotion`)** — bật lực lái `steering_drive_system`, làm tăng `activity_cost` trong `metabolic_decay_system` và va chạm ăn/uống. |
| `live.drive_floor` | `live_experiment.rs:136,225,2280` | **Đưa vào cùng yếu tố `locomotion`** (`0.5` khi bật, `0.0` khi tắt). |
| `live.thirst_steering` | `live_experiment.rs:141,226,2281` | **Đưa vào cùng yếu tố `locomotion`** (`1.0` khi bật, hướng tới hồ nước khi khát). |
| `live.kill_efficiency` | `live_experiment.rs:150,233,2623` | **Đưa vào ma trận (yếu tố `predator_income`)** — đổi hiệu suất đồng hoá của `combat_system` (`world_systems.rs:346`) từ `0.40` lên `0.65`. |
| `live.kill_hydration` | `live_experiment.rs:146,232,2622` | **Đưa vào cùng yếu tố `predator_income`** (`0.5` khi tăng thu hoạch; kiểm chứng phần nước hồi ngoài EU không làm lệch EU). |
| `live.resource_capacity_scale` | `live_experiment.rs:190,236,2394,2405` | **Đưa vào ma trận (yếu tố `resource_capacity_scale`)** — nhân `0.5` lên `r_max` & `r` lúc genesis (`scale_pasture_capacity`), chuyển phần hụt vào `detritus`. |
| `live.plant_reserve` | `live_experiment.rs:199,237,2273` | **Đưa vào ma trận (yếu tố `plant_reserve`)** — đặt sàn dự trữ đồng cỏ `0.3` trong `herbivore_grazing_system` (`environmental_systems.rs:97`). |
| `live.drift_control` | `live_experiment.rs:209,238,1737,1772,3031` | **Đưa vào ma trận `drift_control` (`run_live_drift_pair`)** — phát lại trung tính đồng bộ sinh/tử (`drift_control.rs:169`); đòi hỏi `mortality=1` & `reproduction=1` (`live_experiment.rs:613`). |
| `live.drift_reset_ticks` | `live_experiment.rs:212,239,607,3046` | **Đưa vào ma trận `drift_control`** (`K = 0` không reset và `K = 60` cửa sổ Channon qua snapshot & `from_snapshot_with_drift_schedule`). |
| `live.founders` | `live_experiment.rs:88,216,402` | **Loại trừ khỏi nhân yếu tố (cố định `16`)** — tham số số cá thể khởi đầu lúc genesis, không thay đổi luật chuyển EU; `16` cá thể bảo đảm cả hai bậc dinh dưỡng đều hoạt động. |
| `live.predator_fraction` | `live_experiment.rs:90,217,406` | **Loại trừ (cố định `0.25`)** — cố định `4` thú săn mồi + `12` con mồi để cả nhánh săn mồi (`combat_system`) và gặm cỏ đều diễn ra. |
| `live.trees` | `live_experiment.rs:92,218,403` | **Loại trừ (cố định `8`)** — số cây ban đầu cho `fruit_growth_system` & `spawn_food_system`. |
| `live.lakes` | `live_experiment.rs:94,219,404` | **Loại trừ (cố định `2`)** — số hồ ban đầu cho nhánh uống nước / `thirst_steering`. |
| `live.food_cap` | `live_experiment.rs:96,220,405,2618` | **Loại trừ (cố định `50`)** — trần số mẩu thức ăn rải nền, chỉ đóng mở giới hạn spawn đã nằm trong `detritus <-> food` ở mức mặc định. |
| `live.mutation_rate` | `live_experiment.rs:164,227,2264` | **Loại trừ khỏi nhân 7 yếu tố (giữ `0.05` + kiểm tra điểm `0.0`)** — chỉ đổi xác suất đột biến hình thái/não lúc sinh sản, không mang giá trị EU. |
| `live.target_min_energy` | `live_experiment.rs:154,234,2624` | **Loại trừ khỏi nhân 7 yếu tố (giữ `0.0` + kiểm tra điểm `25.0`)** — chỉ lọc mục tiêu cho steering thú săn mồi (`152-153`), không đổi chi trả của `combat_system`. |
| `live.ecological_fitness` & `live.w_surplus` | `live_experiment.rs:203,205,230,231,2286-2293` | **Loại trừ khỏi nhân 7 yếu tố (+ kiểm tra điểm `ecological_fitness=1.0, w_surplus=2.0`)** — chỉ chọn tiêu chí xếp hạng tại ranh giới epoch (`ticks_per_epoch = 1000`, `fitness.rs:52-166`), không chạm công thức di chuyển EU từng tick. |

### 2. Toàn bộ 8 biến thể `InterventionKind` (`src-tauri/crates/anima-domain/src/intervention.rs:104-138`)

| Biến thể `InterventionKind` | Vị trí định nghĩa & xử lý (`file:line`) | Cơ chế tác động EU |
|---|---|---|
| `RainfallDelta` | `intervention.rs:106`, `live_experiment.rs:886,919,1173` | Đổi `fertility_multiplier`, tác động lượng tái sinh qua `resource_field_regrowth_system` (`detritus <-> plants`). |
| `TemperatureDelta` | `intervention.rs:111`, `live_experiment.rs:889,937,1173` | Dịch `temp_target_shift_c`, tăng/giảm chi phí cân bằng nhiệt trong `metabolic_decay_system` (`animals -> detritus`). |
| `Deforest` | `intervention.rs:113`, `live_experiment.rs:958-987` | Cắt sinh khối đồng cỏ (`strip_region`) và chuyển thẳng sang `detritus` qua `EnergyLedger::transfer`. |
| `AddNutrient` | `intervention.rs:117`, `live_experiment.rs:988-1017` | Rút một phần `detritus` bón vào `ResourceField` (`fertilise_region`) qua `EnergyLedger::transfer`. |
| `RemovePredators` | `intervention.rs:115`, `live_experiment.rs:1018-1047` | Despawn tiền tố thú săn mồi qua `ReclaimAndDespawnAgentCommand`, thu hồi EU vào `detritus`. |
| `GeneticInjection` (+ cả 5 `TraitMod`) | `intervention.rs:88-99,119-123`, `live_experiment.rs:1048-1063` | Ghi nhận 5 loại biến đổi tính trạng (`ScaleMorphology`, `MassMultiplier`, `MetabolicEfficiencyDelta`, `SensoryRangeMultiplier`, `BrainParameterShift`) lên `CausalLedger` mà không thêm/bớt EU. |
| `ToxicityPlume` | `intervention.rs:125-131`, `live_experiment.rs:1064-1115` | Triệt tiêu sinh khối đồng cỏ trong bán kính và chuyển vào `detritus` qua `EnergyLedger::transfer`. |
| `PathogenTransmission` | `intervention.rs:133-137`, `live_experiment.rs:1116-1172` | Tước trực tiếp một tỷ lệ `HomeostaticState::energy` (`debit_reserve`) và tất toán sang `detritus` qua `settle_reserve_outflow_to_detritus`. |

---

## Trạng thái thực thi & bảng kết quả

**Kết luận tổng hợp (không phát hiện rò rỉ):** Mọi tổ hợp chính sách (`192/192` giai thừa đầy đủ + `8` hàng mảng phủ cặp), mọi cặp `drift_control` (`16/16` ở `K ∈ {0, 60}`), mọi biến thể `InterventionKind` (`27/27` mục gồm cả `GeneticInjection` với cả `5` `TraitMod` và tổ hợp đồng thời `All8Combined`), và tổ hợp khoá loại trừ (`mutation_rate`, `target_min_energy`, `ecological_fitness`, `w_surplus`) đều bảo toàn `live.closed_eu_total` tới nhiễu cộng dấu phẩy động (`<= 3.929e-10` EU trên tổng quỹ `155.187,250` EU), thấp hơn **`2,5 × 10^6` lần** so với ngưỡng `1.0e-3` EU (`violations = 0`).

### Cấu hình máy & chuỗi công cụ đo thực tế

| Thành phần | Giá trị đo từ lệnh (`Get-CimInstance` / `rustc -V` / `cargo -V`) |
|---|---|
| CPU | `Intel(R) Core(TM) i5-14600KF` (`14` nhân vật lý / `20` luồng logic, `MaxClockSpeed = 3500 MHz`) |
| Bộ nhớ RAM | `51.318.800.384` byte (`47,8 GiB`) |
| Hệ điều hành | `Microsoft Windows 11 Pro` (`Version: 10.0.26300`, `BuildNumber: 26300`) |
| Chuỗi công cụ Rust | `rustc 1.98.1 (48a229cea 2026-09-01)`, `cargo 1.98.1 (797e8a9bc 2026-08-05)`, `CARGO_BUILD_JOBS=6` |
| Commit gốc đo | `f4614665f7ccf527a6a93a5b0fcfd39aa4e3f948` (nhánh `win/m4-eu-matrix`, PR nháp `#137`) |

### Lý do tách covering design (debug test) và full factorial (release example)

- Bộ phủ cặp trong test debug (`45` mục kiểm = `58` lượt mô phỏng thế giới thật qua `LiveExperimentAdapter`) chạy mất **`31,62 s`** thực thi nhị phân ở cấu hình `--features desktop` (`57,36 s` tường gồm `24,90 s` kiểm tra/liên kết ấm; lần biên dịch lạnh đầu tiên mất `683,9 s` với test binary `29,06 s`), và **`25,67 s`** ở cấu hình `--no-default-features` (`46,27 s` tường). Trung bình mỗi lượt thế giới debug mất `~0,55 s` và mỗi cặp `drift_control` mất `~1,64 s`.
- Ngoại suy cả `236` mục giai thừa đầy đủ (`192` tổ hợp chính sách + `16` cặp `drift_control` + `27` mục `InterventionKind` + `1` mục khoá loại trừ = `261` lượt mô phỏng, `2/3` mang não Burn từng cá thể) sang debug không tối ưu vượt **`~140–165 s > 120 s`**, vượt trần CI `120 s`. Do đó:
  1. `src-tauri/tests/eu_residual_matrix_tests.rs` chạy mảng phủ cặp bậc 2 (`8` hàng phủ `96/96 = 100%` cặp giá trị giữa `7` yếu tố `[2, 2, 2, 2, 2, 3, 2]`), toàn bộ `8` biến thể `InterventionKind` + `All8Combined` trên `baseline`, `sel2_full` và `drift_k60`, `4` cặp góc `drift_control` (`K ∈ {0, 60}`), và mục khoá loại trừ (`< 32 s` ở debug).
  2. `src-tauri/examples/eu_residual_matrix.rs` chạy toàn bộ ma trận giai thừa đầy đủ `192` tổ hợp chính sách × `16` cặp `drift_control` × `27` mục `InterventionKind` × mục khoá loại trừ ở cấu hình `--release` trong **`16,70 s`** tường (`total_s = 16.18` nội bộ).

### Các lệnh thực thi & kiểm tra cổng

| Cổng / Lần chạy | Lệnh thực thi nguyên văn | Kết quả | Thời gian |
|---|---|---|---|
| Format Rust | `cargo fmt --all -- --check` | `EXIT=0` (sạch) | `1,21 s` |
| Clippy (desktop) | `cargo clippy --all-targets --features desktop -- -D warnings` | `EXIT=0` (0 warning) | `7,82 s` |
| Clippy (default) | `cargo clippy --all-targets --no-default-features -- -D warnings` | `EXIT=0` (0 warning) | `27,32 s` |
| Debug test (desktop) | `cargo test --features desktop --test eu_residual_matrix_tests -- --nocapture` | `4 passed; 0 failed` (`EXIT=0`) | `31,62 s` binary (`57,36 s` tường) |
| Debug test (default) | `cargo test --no-default-features --test eu_residual_matrix_tests -- --nocapture` | `4 passed; 0 failed` (`EXIT=0`) | `25,67 s` binary (`46,27 s` tường) |
| Build release example | `cargo build --release --features desktop --example eu_residual_matrix` | `EXIT=0` | `70,8 s` (lạnh `610,78 s`) |
| Chạy release example | `.\target\release\examples\eu_residual_matrix.exe` (không dùng `cargo run`) | `EXIT=0`, `violations=0`, `overall_worst=3.929e-10` | `16,7 s` tường (`16,18 s` nội bộ) |

---

### Bảng 1 — Mảng phủ cặp bậc 2 trong debug test (`8` hàng, `96/96` cặp, `31,62 s`)

Ký hiệu yếu tố: `m` = `mortality` (`0/1`), `r` = `reproduction` (`0/1`), `l` = `locomotion` (`0/1`), `p` = `plant_reserve` (`0` = `0.0`, `1` = `0.3`), `c` = `resource_capacity_scale` (`0` = `1.0`, `1` = `0.5`), `b` = `brain` (`sh` = shared, `ev` = evolved, `ev+ll+cost` = evolved + lifetime_learning + cost `0.5`), `k` = `predator_income` (`0` = baseline `0.40/0.0`, `1` = boosted `0.65/0.5`).

```text
M4_COVERING_DESIGN runs=8 pairwise_tuples=96
M4_PAIR[00] m0_r0_l0_p0_c0_b(sh)_k0  eu_0=155187.250 delta_t0=+2.910e-11 delta_t1=+2.910e-11 worst_t1=2.910e-11 gap=8.731e-11
M4_PAIR[01] m1_r1_l1_p1_c1_b(ev+ll+cost)_k1 eu_0=155187.250 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=0.000e0
M4_PAIR[02] m1_r1_l1_p0_c0_b(ev)_k0  eu_0=155187.250 delta_t0=+8.731e-11 delta_t1=+8.731e-11 worst_t1=1.164e-10 gap=8.731e-11
M4_PAIR[03] m0_r0_l0_p1_c1_b(ev)_k1  eu_0=155187.250 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=0.000e0
M4_PAIR[04] m1_r1_l0_p1_c1_b(sh)_k0  eu_0=155187.250 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=0.000e0
M4_PAIR[05] m0_r0_l1_p0_c0_b(ev+ll+cost)_k1 eu_0=155187.250 delta_t0=+2.910e-11 delta_t1=+2.910e-11 worst_t1=5.821e-11 gap=1.164e-10
M4_PAIR[06] m1_r0_l1_p1_c0_b(sh)_k1  eu_0=155187.250 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=0.000e0
M4_PAIR[07] m0_r1_l0_p0_c1_b(ev+ll+cost)_k0 eu_0=155187.250 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=1.455e-11
M4_POLICY_COVERING_SUMMARY runs=8 elapsed_s=2.89 max_abs_residual=1.164e-10
```

---

### Bảng 2 — Ma trận giai thừa đầy đủ `192` tổ hợp chính sách (`--release`, `5,16 s`)

Chạy đủ `2^5 × 3 × 2 = 192` tổ hợp (`T = 90` tick mỗi lượt). Tổng kết: **`runs = 192` · `violations = 0` · đỉnh toàn cục `worst = 3.929e-10` EU** (lệch gương thực vật `gap = 3.929e-10` EU tại `POLICY[054]` / `[055]` / `[150]` / `[151]`, còn `max |ΔEU(0->T)| = max |ΔEU(1->T)| = 2.037e-10` EU tại `POLICY[002..003]`). Dưới đây là các hàng góc và các hàng cực đại nguyên văn từ `eu_residual_matrix.exe`:

```text
POLICY[000] m0_r0_l0_p0_c0_b(sh)_k0  eu_0=155187.250 delta_t0=+2.910e-11 delta_t1=+2.910e-11 worst_t1=2.910e-11 gap=8.731e-11
POLICY[001] m0_r0_l0_p0_c0_b(sh)_k1  eu_0=155187.250 delta_t0=+2.910e-11 delta_t1=+2.910e-11 worst_t1=2.910e-11 gap=8.731e-11
POLICY[002] m0_r0_l0_p0_c0_b(ev)_k0  eu_0=155187.250 delta_t0=+2.037e-10 delta_t1=+2.037e-10 worst_t1=2.037e-10 gap=2.328e-10
POLICY[003] m0_r0_l0_p0_c0_b(ev)_k1  eu_0=155187.250 delta_t0=+2.037e-10 delta_t1=+2.037e-10 worst_t1=2.037e-10 gap=2.328e-10
POLICY[005] m0_r0_l0_p0_c0_b(ev+ll+cost)_k1 eu_0=155187.250 delta_t0=+1.164e-10 delta_t1=+1.164e-10 worst_t1=1.164e-10 gap=2.037e-10
POLICY[041] m0_r0_l1_p1_c0_b(ev+ll+cost)_k1 eu_0=155187.250 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=0.000e0
POLICY[050] m0_r1_l0_p0_c0_b(ev)_k0  eu_0=155187.250 delta_t0=+5.821e-11 delta_t1=+5.821e-11 worst_t1=8.731e-11 gap=2.328e-10
POLICY[051] m0_r1_l0_p0_c0_b(ev)_k1  eu_0=155187.250 delta_t0=+5.821e-11 delta_t1=+5.821e-11 worst_t1=8.731e-11 gap=2.328e-10
POLICY[054] m0_r1_l0_p0_c1_b(sh)_k0  eu_0=155187.250 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=3.929e-10
POLICY[055] m0_r1_l0_p0_c1_b(sh)_k1  eu_0=155187.250 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=3.929e-10
POLICY[149] m1_r1_l0_p0_c0_b(ev+ll+cost)_k1 eu_0=155187.250 delta_t0=+2.910e-11 delta_t1=+2.910e-11 worst_t1=8.731e-11 gap=2.037e-10
POLICY[150] m1_r1_l0_p0_c1_b(sh)_k0  eu_0=155187.250 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=3.929e-10
POLICY[151] m1_r1_l0_p0_c1_b(sh)_k1  eu_0=155187.250 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=3.929e-10
POLICY[185] m1_r1_l1_p1_c0_b(ev+ll+cost)_k1 eu_0=155187.250 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=0.000e0
POLICY[191] m1_r1_l1_p1_c1_b(ev+ll+cost)_k1 eu_0=155187.250 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=0.000e0
FULL_POLICY_SUMMARY runs=192 elapsed_s=5.16 worst=3.929e-10 worst_case=m0_r1_l0_p0_c1_b(sh)_k0
```

---

### Bảng 3 — Ma trận `drift_control` đầy đủ `16` cặp (`K ∈ {0, 60}`, `--release`, `5,14 s`)

Ký hiệu: `k0` / `k60` (`drift_reset_ticks = 0` / `60`), `p0` / `p1` (`plant_reserve = 0.0` / `0.3`), `l0` / `l1` (`locomotion = 0` / `1`), `b0` / `b1` (`shared` / `evolved + ll + cost`), đều với `mortality = 1`, `reproduction = 1`, `T = 120` tick.

```text
DRIFT_PAIR[00] k0_p0_l0_b0      eu_0=155187.250 sel_worst=1.455e-10 drf_worst=8.731e-11
DRIFT_PAIR[01] k0_p0_l0_b1      eu_0=155187.250 sel_worst=8.731e-11 drf_worst=5.821e-11
DRIFT_PAIR[02] k0_p0_l1_b0      eu_0=155187.250 sel_worst=1.164e-10 drf_worst=8.731e-11
DRIFT_PAIR[03] k0_p0_l1_b1      eu_0=155187.250 sel_worst=1.164e-10 drf_worst=1.455e-10
DRIFT_PAIR[04] k0_p1_l0_b0      eu_0=155187.250 sel_worst=0.000e0 drf_worst=0.000e0
DRIFT_PAIR[05] k0_p1_l0_b1      eu_0=155187.250 sel_worst=0.000e0 drf_worst=0.000e0
DRIFT_PAIR[06] k0_p1_l1_b0      eu_0=155187.250 sel_worst=0.000e0 drf_worst=0.000e0
DRIFT_PAIR[07] k0_p1_l1_b1      eu_0=155187.250 sel_worst=0.000e0 drf_worst=0.000e0
DRIFT_PAIR[08] k60_p0_l0_b0     eu_0=155187.250 sel_worst=1.455e-10 drf_worst=1.455e-10
DRIFT_PAIR[09] k60_p0_l0_b1     eu_0=155187.250 sel_worst=8.731e-11 drf_worst=8.731e-11
DRIFT_PAIR[10] k60_p0_l1_b0     eu_0=155187.250 sel_worst=1.164e-10 drf_worst=1.164e-10
DRIFT_PAIR[11] k60_p0_l1_b1     eu_0=155187.250 sel_worst=1.164e-10 drf_worst=1.164e-10
DRIFT_PAIR[12] k60_p1_l0_b0     eu_0=155187.250 sel_worst=0.000e0 drf_worst=0.000e0
DRIFT_PAIR[13] k60_p1_l0_b1     eu_0=155187.250 sel_worst=0.000e0 drf_worst=0.000e0
DRIFT_PAIR[14] k60_p1_l1_b0     eu_0=155187.250 sel_worst=0.000e0 drf_worst=0.000e0
DRIFT_PAIR[15] k60_p1_l1_b1     eu_0=155187.250 sel_worst=0.000e0 drf_worst=0.000e0
FULL_DRIFT_SUMMARY pairs=16 elapsed_s=5.14 worst=1.455e-10 worst_case=k0_p0_l0_b0
```

---

### Bảng 4 — Toàn bộ `8` `InterventionKind` + `All8Combined` (`27` mục, `--release`, `5,84 s`) & kiểm tra khoá loại trừ

```text
INTERVENTION baseline/RainfallDelta             eff=1 delta_t0=+5.821e-11 delta_t1=+5.821e-11 worst_t1=5.821e-11 gap=1.455e-10
INTERVENTION sel2_full/RainfallDelta            eff=1 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=0.000e0
INTERVENTION drift_k60/RainfallDelta            eu_0=155187.250 sel_worst=0.000e0 drf_worst=0.000e0
INTERVENTION baseline/TemperatureDelta          eff=1 delta_t0=+8.731e-11 delta_t1=+8.731e-11 worst_t1=8.731e-11 gap=5.821e-11
INTERVENTION sel2_full/TemperatureDelta         eff=1 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=0.000e0
INTERVENTION drift_k60/TemperatureDelta         eu_0=155187.250 sel_worst=0.000e0 drf_worst=0.000e0
INTERVENTION baseline/Deforest                  eff=1 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=2.910e-11 gap=1.019e-10
INTERVENTION sel2_full/Deforest                 eff=1 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=0.000e0
INTERVENTION drift_k60/Deforest                 eu_0=155187.250 sel_worst=0.000e0 drf_worst=0.000e0
INTERVENTION baseline/AddNutrient               eff=1 delta_t0=+2.910e-11 delta_t1=+2.910e-11 worst_t1=2.910e-11 gap=0.000e0
INTERVENTION sel2_full/AddNutrient              eff=1 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=0.000e0
INTERVENTION drift_k60/AddNutrient              eu_0=155187.250 sel_worst=0.000e0 drf_worst=0.000e0
INTERVENTION baseline/RemovePredators           eff=1 delta_t0=+2.910e-11 delta_t1=+2.910e-11 worst_t1=2.910e-11 gap=8.731e-11
INTERVENTION sel2_full/RemovePredators          eff=1 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=0.000e0
INTERVENTION drift_k60/RemovePredators          eu_0=155187.250 sel_worst=0.000e0 drf_worst=0.000e0
INTERVENTION baseline/GeneticInjection(5Mods)   eff=1 delta_t0=+2.910e-11 delta_t1=+2.910e-11 worst_t1=2.910e-11 gap=8.731e-11
INTERVENTION sel2_full/GeneticInjection(5Mods)  eff=1 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=0.000e0
INTERVENTION drift_k60/GeneticInjection(5Mods)  eu_0=155187.250 sel_worst=0.000e0 drf_worst=0.000e0
INTERVENTION baseline/ToxicityPlume             eff=1 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=2.910e-11 gap=1.019e-10
INTERVENTION sel2_full/ToxicityPlume            eff=1 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=0.000e0
INTERVENTION drift_k60/ToxicityPlume            eu_0=155187.250 sel_worst=0.000e0 drf_worst=0.000e0
INTERVENTION baseline/PathogenTransmission      eff=1 delta_t0=+5.821e-11 delta_t1=+5.821e-11 worst_t1=5.821e-11 gap=2.910e-11
INTERVENTION sel2_full/PathogenTransmission     eff=1 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=0.000e0
INTERVENTION drift_k60/PathogenTransmission     eu_0=155187.250 sel_worst=0.000e0 drf_worst=0.000e0
INTERVENTION baseline/All8Combined              eff=8 delta_t0=+5.821e-11 delta_t1=+5.821e-11 worst_t1=5.821e-11 gap=1.237e-10
INTERVENTION sel2_full/All8Combined             eff=8 delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=0.000e0
INTERVENTION drift_k60/All8Combined             eu_0=155187.250 sel_worst=0.000e0 drf_worst=0.000e0
FULL_INTERVENTIONS_SUMMARY entries=27 elapsed_s=5.84 worst=1.455e-10 worst_case=baseline/RainfallDelta
EXCLUDED_GUARD delta_t0=+0.000e0 delta_t1=+0.000e0 worst_t1=0.000e0 gap=0.000e0
M4_OVERALL_SUMMARY total_s=16.18 tolerance=1.0e-3 overall_worst=3.929e-10 violations=0
```
