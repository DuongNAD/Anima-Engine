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

*(Phần bảng đo sẽ được điền nguyên văn từ `cargo test --features desktop --test eu_residual_matrix_tests` ở cấu hình phủ cặp debug `< 120 s` và `eu_residual_matrix.exe` ở cấu hình `--release` cho toàn ma trận đầy đủ sau khi chạy ở bước kế tiếp.)*
