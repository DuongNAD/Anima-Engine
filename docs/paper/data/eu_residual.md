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
| `live.starve_ticks` | `live_experiment.rs:167,228,2255` | **Đưa vào cùng yếu tố `mortality`** (`5` ở nhánh bật để có đủ cả tử vong do đói và khát; cũng kiểm chứng `120` mặc định ở tiền lệ). |
| `live.dehydration_ticks` | `live_experiment.rs:170,229,2256` | **Đưa vào cùng yếu tố `mortality`** (`30` ở nhánh bật để cân bằng giữa chết đói `1.209` và chết khát `1.189`). |
| `live.reproduction` | `live_experiment.rs:124,223,2260` | **Đưa vào ma trận (yếu tố `reproduction`)** — bật `ReproductionPolicy.enabled`; `autonomous_reproduction_system` (`agent_systems.rs:639`) chia quỹ mẹ cho con + đổ phí Lindeman về `detritus`. |
| `live.prey_cap` | `live_experiment.rs:179,235,2268` | **Đưa vào cùng yếu tố `reproduction`** (`18` từ `14` con mồi sáng lập để kích hoạt cả sinh sản thú săn mồi, sinh sản con mồi lẫn chạm trần). |
| `live.locomotion` | `live_experiment.rs:131,224,2278` | **Đưa vào ma trận (yếu tố `locomotion`)** — bật lực lái `steering_drive_system`, làm tăng `activity_cost` trong `metabolic_decay_system` và va chạm ăn/uống/săn mồi. |
| `live.drive_floor` | `live_experiment.rs:136,225,2280` | **Đưa vào cùng yếu tố `locomotion`** (`1.0` khi bật để thú săn mồi tốc độ tối đa `2,0` bắt kịp con mồi tốc độ `1,5` qua khoảng cách `9,52` đơn vị lưới trong `~412–550` tick). |
| `live.thirst_steering` | `live_experiment.rs:141,226,2281` | **Đưa vào cùng yếu tố `locomotion`** (`1.0` khi bật, hướng tới hồ nước khi khát). |
| `live.kill_efficiency` | `live_experiment.rs:150,233,2623` | **Đưa vào ma trận (yếu tố `predator_income`)** — đổi hiệu suất đồng hoá của `combat_system` (`world_systems.rs:346`) từ `0.40` lên `0.65`. |
| `live.kill_hydration` | `live_experiment.rs:146,232,2622` | **Đưa vào cùng yếu tố `predator_income`** (`0.5` khi tăng thu hoạch; kiểm chứng phần nước hồi ngoài EU không làm lệch EU). |
| `live.resource_capacity_scale` | `live_experiment.rs:190,236,2394,2405` | **Đưa vào ma trận (yếu tố `resource_capacity_scale`)** — nhân `0.5` lên `r_max` & `r` lúc genesis (`scale_pasture_capacity`), chuyển phần hụt vào `detritus`. |
| `live.plant_reserve` | `live_experiment.rs:199,237,2273` | **Đưa vào ma trận (yếu tố `plant_reserve`)** — đặt sàn dự trữ đồng cỏ `0.3` trong `herbivore_grazing_system` (`environmental_systems.rs:97`). |
| `live.drift_control` | `live_experiment.rs:209,238,1737,1772,3031` | **Đưa vào ma trận `drift_control`** — phát lại trung tính đồng bộ sinh/tử (`drift_control.rs:169`); đòi hỏi `mortality=1` & `reproduction=1` (`live_experiment.rs:613`). |
| `live.drift_reset_ticks` | `live_experiment.rs:212,239,607,3046` | **Đưa vào ma trận `drift_control`** (`K = 0` không reset và `K = 1.000` ở release `T = 3.000` / `K = 800` ở debug `T = 2.400`, mỗi cấu hình `3` cửa sổ Channon qua snapshot & `from_snapshot_with_drift_schedule`). |
| `live.founders` | `live_experiment.rs:88,216,402` | **Loại trừ khỏi nhân yếu tố (cố định `20`)** — tham số số cá thể khởi đầu lúc genesis, không thay đổi luật chuyển EU; `20` cá thể với `0,30` thú săn mồi bảo đảm có `6` thú săn mồi để mọi cấu hình não tiến hoá vẫn có cổng tấn công mở. |
| `live.predator_fraction` | `live_experiment.rs:90,217,406` | **Loại trừ (cố định `0.30`)** — cố định `6` thú săn mồi + `14` con mồi để cả nhánh săn mồi (`combat_system`) và gặm cỏ đều diễn ra. |
| `live.trees` | `live_experiment.rs:92,218,403` | **Loại trừ (cố định `4`)** — số cây ban đầu cho `fruit_growth_system` & `spawn_food_system`. |
| `live.lakes` | `live_experiment.rs:94,219,404` | **Loại trừ (cố định `1`)** — số hồ ban đầu cho nhánh uống nước / `thirst_steering`. |
| `live.food_cap` | `live_experiment.rs:96,220,405,2618` | **Loại trừ (cố định `20`)** — trần số mẩu thức ăn rải nền, chỉ đóng mở giới hạn spawn đã nằm trong `detritus <-> food` ở mức mặc định. |
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

## Trạng thái thực thi & bảng kết quả (Vòng đo chống rỗng — Fix Round 1)

**Kết luận tổng hợp (không phát hiện rò rỉ, mọi nhánh đều kích hoạt thực tế):** Trên toàn bộ `236` mục giai thừa đầy đủ (`192` tổ hợp chính sách `T = 3.000` tick + `16` cặp `drift_control` `T = 3.000` tick ở `K ∈ {0, 1.000}` + `27` mục `InterventionKind` + `1` mục khoá loại trừ) và bộ kiểm thử phủ cặp debug (`T = 2.000` / `2.400` tick), tổng cộng `violations = 0` và `vacuity_failures = 0`. Đỉnh sai lệch toàn cục là **`overall_worst = 1.033e-8` EU** trên tổng quỹ **`155.587,250` EU** (thấp hơn **`~9,68 × 10^4` lần** so với ngưỡng dung sai `1.0e-3` EU; tại `POLICY[072] m0_r1_l1_p0_c0_b(sh)_k0`, độ lệch gương thực vật `gap = 1.033e-8` EU).

### Cấu hình máy & chuỗi công cụ đo thực tế

| Thành phần | Giá trị đo từ lệnh (`Get-CimInstance` / `rustc -V` / `cargo -V`) |
|---|---|
| CPU | `Intel(R) Core(TM) i5-14600KF` (`14` nhân vật lý / `20` luồng logic, `MaxClockSpeed = 3500 MHz`) |
| Bộ nhớ RAM | `51.318.800.384` byte (`47,8 GiB`) |
| Hệ điều hành | `Microsoft Windows 11 Pro` (`Version: 10.0.26300`, `BuildNumber: 26300`) |
| Chuỗi công cụ Rust | `rustc 1.98.1 (48a229cea 2026-09-01)`, `cargo 1.98.1 (797e8a9bc 2026-08-05)`, `CARGO_BUILD_JOBS=6` |
| Commit gốc đo | `f4614665f7ccf527a6a93a5b0fcfd39aa4e3f948` (nhánh `win/m4-eu-matrix`, PR nháp `#137`) |

### Tỷ lệ kích hoạt thực tế từng yếu tố (Non-vacuity summary)

Mỗi dòng dưới đây báo số lượt có bằng chứng dòng chảy thực tế từ các nguồn công khai (`VitalStatistics`, `CombatEvents`, `DriftDiagnostics`, `CausalLedger`) trên tổng số lượt bật cấu hình đó:

- **`live.reproduction = 1` (`prey_cap = 18` trên `14` con mồi sáng lập):** **được kích hoạt thực tế ở `96 / 96` lượt chạy chính sách** (`births >= 14` ở mọi lượt bật; `0 / 96` khi tắt).
- **`live.mortality = 1` (`starve_ticks = 5`, `dehydration_ticks = 30`):** **được kích hoạt thực tế ở `96 / 96` lượt chạy chính sách** (`deaths >= 5` ở mọi lượt bật; tổng **`1.209` ca chết đói `Starvation`**, **`1.189` ca chết khát `Dehydration`**, `0` ca khác ở nhánh chọn; `0 / 96` khi tắt). Thời điểm cá thể đầu tiên chết (`first_death_tick`) qua `96` lượt: nhỏ nhất **`317`** (`m1_r1_l1_p1_c1_b(sh)_k1`), trung vị **`811`**, lớn nhất **`1.754`** (`m1_r0_l0_p0_c0_b(ev)_k0`).
- **`live.locomotion = 1` (`drive_floor = 1.0`, `thirst_steering = 1.0`):** **được kích hoạt thực tế ở `96 / 96` lượt chạy chính sách** (vận tốc lái làm thú săn mồi đạt bán kính đánh `< 3,0` và tăng chi phí chuyển hoá/ăn/uống).
- **`live.kill_efficiency = 0.65` & `live.kill_hydration = 0.5` (`predator_income`, `k = 1`):**
  - Khi **`locomotion = 1`**: **được kích hoạt thực tế ở `48 / 48` lượt chạy** (`CombatEvents` ghi nhận `pred_strikes >= 1` và `pred_eu > 0`; thời điểm cú săn mồi đầu tiên `first_pred_tick` nằm trong **`412 .. 550`**).
  - Khi **`locomotion = 0`**: **`0 / 48` lượt chạy — không thể kích hoạt về mặt cơ học**, vì `lattice_point` (`live_experiment.rs:2586`) đặt `20` cá thể sáng lập giãn cách `200 / 21 = 9,52 > 3,0` đơn vị bán kính đánh của `combat_system` (`world_systems.rs:446`) trong khi vận tốc bằng `0`. (`k = 0, locomotion = 1` cũng ghi nhận săn mồi trên cả `48 / 48` lượt.)
- **`live.plant_reserve = 0.3`:** **được kích hoạt thực tế ở `96 / 96` lượt chạy chính sách bật cờ** và **`8 / 8` cặp `drift_control` bật cờ** trong `herbivore_grazing_system`.
- **`live.resource_capacity_scale = 0.5`:** **được kích hoạt thực tế ở `96 / 96` lượt chạy chính sách bật cờ** (tất toán một nửa sinh khối đồng cỏ sang `detritus` tại genesis).
- **`brain` (`live.evolved_brains` & `BrainPolicy`):** `evolved` (`b = 1`) **được kích hoạt thực tế ở `64 / 64` lượt chạy**; `evolved + lifetime_learning + brain_metabolic_cost = 0.5` (`b = 2`) **được kích hoạt thực tế ở `64 / 64` lượt chạy**.
- **`live.drift_control = 1.0` (`K ∈ {0, 1.000}`):** **được kích hoạt thực tế ở `16 / 16` cặp chính sách** (`drf_actual_births >= 14` và `drf_actual_deaths >= 21` phát lại trên nhánh drift ở mọi cặp, cộng thêm kiểm chứng trùng khớp checksum & `DriftDiagnostics` với `run_live_drift_pair` trong debug test) và **`9 / 9` cặp can thiệp `drift_k1000`** (`drf_b >= 14`, `drf_d >= 33`).
- **`InterventionKind` (`8` biến thể đơn lẻ + `All8Combined`):** **được kích hoạt thực tế ở `27 / 27` mục** (`eff = 1` hoặc `8` bản ghi `CausalLedger`). Ở hồ sơ dài `T = 3.000`, các can thiệp `1`-tick đặt tại `fire_mid = 120` (bội số của `ECOLOGY_PERIOD = 60`, trước `first_death_tick = 278..366` để mọi tác tử đều sống lúc can thiệp) với `RemovePredators intensity = 0.25` (loại `1/6` thú săn mồi, giữ `5` cá thể tiếp tục tạo `>= 38` cú săn mồi).

### Lý do tách covering design (debug `< 120 s`) và full factorial (`--release`)

- Bộ test debug (`eu_residual_matrix_tests.rs`, chạy song song 4 hàm test ở `POLICY_TICKS = 2.000`, `DRIFT_TICKS = 2.400`, `DRIFT_RESET_K = 800`, `BASELINE_INTERVENTION_TICKS = 180`) hoàn tất trong **`68,93 s`** ở cấu hình `--features desktop` và **`91,18 s`** ở cấu hình `--no-default-features` — đều dưới trần CI `120 s`. (Bỏ qua bước gọi `observables_unsampled()` lặp `65.536` ô mỗi tick trung gian; thay bằng `EcosystemBiomass` hằng thời gian mỗi tick và đối chiếu `observables()` + `ResourceField::total_biomass()` tại tick `0, 1`, định kỳ `250` và tick cuối.)
- Toàn bộ ma trận giai thừa đầy đủ `236` mục ở `T = 3.000` tick (`192` chính sách + `16` cặp `drift_control` + `27` can thiệp + `1` khoá loại trừ = `261` thế giới mô phỏng) chạy trong bản `--release` `target\release\examples\eu_residual_matrix.exe` mất **`227,15 s`** tường (`226,68 s` nội bộ).

### Các lệnh thực thi & kiểm tra cổng

| Cổng / Lần chạy | Lệnh thực thi nguyên văn | Kết quả | Thời gian |
|---|---|---|---|
| Format Rust | `cargo fmt --all -- --check` | `EXIT=0` (sạch) | `1,89 s` |
| Clippy (desktop) | `cargo clippy --all-targets --features desktop -- -D warnings` | `EXIT=0` (0 warning) | `27,07 s` |
| Clippy (default) | `cargo clippy --all-targets --no-default-features -- -D warnings` | `EXIT=0` (0 warning) | `60,40 s` |
| Debug test (desktop) | `cargo test --features desktop --test eu_residual_matrix_tests -- --nocapture` | `4 passed; 0 failed` (`EXIT=0`) | `68,93 s` binary (`236,39 s` gồm biên dịch) |
| Debug test (default) | `cargo test --no-default-features --test eu_residual_matrix_tests -- --nocapture` | `4 passed; 0 failed` (`EXIT=0`) | `91,18 s` binary (`162,82 s` gồm biên dịch) |
| Build release example | `cargo build --release --features desktop --example eu_residual_matrix` | `EXIT=0` | `13,25 s` |
| Chạy release example | `.\target\release\examples\eu_residual_matrix.exe` (không dùng `cargo run`) | `EXIT=0`, `violations=0`, `vacuity_failures=0`, `overall_worst=1.033e-8` | `227,15 s` tường (`226,68 s` nội bộ) |

---

### Bảng 1 — Mảng phủ cặp bậc 2 trong debug test (`8` hàng, `96/96` cặp, `T = 2.000`, `36,72 s`)

Ký hiệu: `b` = `births`, `d=N(sS,hH,d1=T)` = tổng ca tử (`S` chết đói, `H` chết khát, `d1` tick chết đầu tiên), `p=K(E)` = số cú đánh săn mồi `CombatEvents` và tổng EU chuyển giao qua săn mồi.

```text
M4_COVERING_DESIGN runs=8 pairwise_tuples=96
M4_PAIR[00] m0_r0_l0_p0_c0_b(sh)_k0         eu_0=155587.250 d0=-6.112e-10 d1=-6.112e-10 wt1=6.112e-10 gap=3.783e-10 b=0 d=0(s0,h0,d1=-) p=0(0.0)
M4_PAIR[01] m1_r1_l1_p1_c1_b(ev+ll+cost)_k1 eu_0=155587.250 d0=+0.000e0 d1=+0.000e0 wt1=0.000e0 gap=0.000e0 b=14 d=34(s34,h0,d1=365) p=130(45.0)
M4_PAIR[02] m0_r0_l0_p1_c1_b(ev)_k1         eu_0=155587.250 d0=+0.000e0 d1=+0.000e0 wt1=0.000e0 gap=0.000e0 b=0 d=0(s0,h0,d1=-) p=0(0.0)
M4_PAIR[03] m1_r1_l1_p0_c0_b(ev)_k0         eu_0=155587.250 d0=+4.366e-10 d1=+4.366e-10 wt1=5.821e-10 gap=4.220e-9 b=23 d=18(s1,h17,d1=1182) p=1627(144.0)
M4_PAIR[04] m0_r0_l1_p0_c0_b(ev+ll+cost)_k1 eu_0=155587.250 d0=-8.731e-11 d1=-8.731e-11 wt1=3.783e-10 gap=2.939e-9 b=0 d=0(s0,h0,d1=-) p=623(125.9)
M4_PAIR[05] m1_r1_l0_p1_c1_b(sh)_k0         eu_0=155587.250 d0=+0.000e0 d1=+0.000e0 wt1=0.000e0 gap=0.000e0 b=14 d=33(s24,h9,d1=345) p=0(0.0)
M4_PAIR[06] m0_r1_l0_p0_c1_b(ev+ll+cost)_k0 eu_0=155587.250 d0=+2.037e-10 d1=+2.037e-10 wt1=2.328e-10 gap=1.484e-9 b=14 d=0(s0,h0,d1=-) p=0(0.0)
M4_PAIR[07] m1_r0_l1_p1_c0_b(sh)_k1         eu_0=155587.250 d0=+0.000e0 d1=+0.000e0 wt1=0.000e0 gap=0.000e0 b=0 d=19(s6,h13,d1=822) p=937(138.9)
M4_POLICY_COVERING_SUMMARY runs=8 elapsed_s=36.72 max_abs_residual=4.220e-9
```

---

### Bảng 2 — Ma trận giai thừa đầy đủ `192` tổ hợp chính sách (`--release`, `T = 3.000`, `161,81 s`)

Chạy đủ `2^5 × 3 × 2 = 192` tổ hợp (`T = 3.000` tick mỗi lượt). Tổng kết: **`runs = 192` · `violations = 0` · `vacuity_failures = 0` · `worst = 1.033e-8` EU** (`POLICY[072] m0_r1_l1_p0_c0_b(sh)_k0`). Các hàng góc và cực trị nguyên văn từ `eu_residual_matrix.exe`:

```text
POLICY[000] m0_r0_l0_p0_c0_b(sh)_k0          eu_0=155587.250 delta_t0=-6.112e-10 delta_t1=-6.112e-10 worst_t1=6.112e-10 gap=3.783e-10 b=0 d=0(s0,h0) d1=- p=0(0.0)
POLICY[001] m0_r0_l0_p0_c0_b(sh)_k1          eu_0=155587.250 delta_t0=-6.112e-10 delta_t1=-6.112e-10 worst_t1=6.112e-10 gap=3.783e-10 b=0 d=0(s0,h0) d1=- p=0(0.0)
POLICY[002] m0_r0_l0_p0_c0_b(ev)_k0          eu_0=155587.250 delta_t0=-4.366e-10 delta_t1=-4.366e-10 worst_t1=4.366e-10 gap=6.985e-10 b=0 d=0(s0,h0) d1=- p=0(0.0)
POLICY[003] m0_r0_l0_p0_c0_b(ev)_k1          eu_0=155587.250 delta_t0=-4.366e-10 delta_t1=-4.366e-10 worst_t1=4.366e-10 gap=6.985e-10 b=0 d=0(s0,h0) d1=- p=0(0.0)
POLICY[006] m0_r0_l0_p0_c1_b(sh)_k0          eu_0=155587.250 delta_t0=+3.056e-9  delta_t1=+3.056e-9  worst_t1=3.056e-9  gap=5.617e-9  b=0 d=0(s0,h0) d1=- p=0(0.0)
POLICY[007] m0_r0_l0_p0_c1_b(sh)_k1          eu_0=155587.250 delta_t0=+3.056e-9  delta_t1=+3.056e-9  worst_t1=3.056e-9  gap=5.617e-9  b=0 d=0(s0,h0) d1=- p=0(0.0)
POLICY[052] m0_r1_l0_p0_c0_b(ev+ll+cost)_k0  eu_0=155587.250 delta_t0=+3.347e-9  delta_t1=+3.347e-9  worst_t1=3.347e-9  gap=7.625e-9  b=14 d=0(s0,h0) d1=- p=0(0.0)
POLICY[053] m0_r1_l0_p0_c0_b(ev+ll+cost)_k1  eu_0=155587.250 delta_t0=+3.347e-9  delta_t1=+3.347e-9  worst_t1=3.347e-9  gap=7.625e-9  b=14 d=0(s0,h0) d1=- p=0(0.0)
POLICY[072] m0_r1_l1_p0_c0_b(sh)_k0          eu_0=155587.250 delta_t0=-1.077e-9  delta_t1=-1.077e-9  worst_t1=1.135e-9  gap=1.033e-8  b=14 d=0(s0,h0) d1=- p=707(87.7)
POLICY[096] m1_r0_l0_p0_c0_b(sh)_k0          eu_0=155587.250 delta_t0=-8.149e-10 delta_t1=-8.149e-10 worst_t1=8.731e-10 gap=5.530e-10 b=0 d=20(s4,h16) d1=1471 p=0(0.0)
POLICY[098] m1_r0_l0_p0_c0_b(ev)_k0          eu_0=155587.250 delta_t0=-3.783e-10 delta_t1=-3.783e-10 worst_t1=3.783e-10 gap=6.985e-10 b=0 d=5(s0,h5)  d1=1754 p=0(0.0)
POLICY[100] m1_r0_l0_p0_c0_b(ev+ll+cost)_k0  eu_0=155587.250 delta_t0=-8.731e-11 delta_t1=-8.731e-11 worst_t1=1.164e-10 gap=8.440e-10 b=0 d=20(s12,h8) d1=1022 p=0(0.0)
POLICY[175] m1_r1_l1_p0_c1_b(sh)_k1          eu_0=155587.250 delta_t0=+0.000e0   delta_t1=+0.000e0   worst_t1=5.821e-11 gap=3.638e-10 b=14 d=34(s30,h4) d1=320 p=183(39.9)
POLICY[177] m1_r1_l1_p0_c1_b(ev)_k1          eu_0=155587.250 delta_t0=+0.000e0   delta_t1=+0.000e0   worst_t1=2.910e-11 gap=1.513e-9  b=21 d=34(s3,h31) d1=733 p=2199(170.0)
POLICY[179] m1_r1_l1_p0_c1_b(ev+ll+cost)_k1  eu_0=155587.250 delta_t0=+0.000e0   delta_t1=+0.000e0   worst_t1=5.821e-11 gap=6.257e-10 b=14 d=34(s34,h0) d1=380 p=170(70.5)
POLICY[187] m1_r1_l1_p1_c1_b(sh)_k1          eu_0=155587.250 delta_t0=+0.000e0   delta_t1=+0.000e0   worst_t1=0.000e0   gap=0.000e0   b=14 d=34(s32,h2) d1=317 p=112(19.4)
POLICY[189] m1_r1_l1_p1_c1_b(ev)_k1          eu_0=155587.250 delta_t0=+0.000e0   delta_t1=+0.000e0   worst_t1=0.000e0   gap=0.000e0   b=17 d=33(s12,h21) d1=724 p=1030(105.0)
POLICY[191] m1_r1_l1_p1_c1_b(ev+ll+cost)_k1  eu_0=155587.250 delta_t0=+0.000e0   delta_t1=+0.000e0   worst_t1=0.000e0   gap=0.000e0   b=14 d=34(s34,h0) d1=365 p=130(45.0)
FULL_POLICY_SUMMARY runs=192 elapsed_s=161.81 worst=1.033e-8 worst_case=m0_r1_l1_p0_c0_b(sh)_k0
FACTOR_COVERAGE repro=96/96 mort=96/96(starv=1209,dehyd=1189,d1=317/811/1754) loc=96/96 resv=96/96 cap=96/96 br_ev=64/64 br_ll=64/64 pred_l1=48/48(p1=412..550) pred_l0=0/48
```

---

### Bảng 3 — Ma trận `drift_control` đầy đủ `16` cặp (`K ∈ {0, 1.000}`, `--release`, `T = 3.000`, `33,97 s`)

Mọi cặp bật `mortality = 1`, `reproduction = 1`. Cột `drf_d=N(oN,s0,h0)` phản ánh đúng việc nhánh phát lại drift trung tính chạy qua `vitals.record_death(lifespan)` (không gắn nhãn đói/khát) và khớp `DriftDiagnostics::actual_deaths`. Cả `16 / 16` cặp đều phát lại đầy đủ sinh (`14..34`) và tử (`21..42`).

```text
DRIFT_PAIR[00] k0_p0_l0_b0        eu_0=155587.250 sel_w=1.921e-9 drf_w=6.985e-10 sel_b=29 sel_d=42(s21,h21,d1=519) drf_b=27 drf_d=42(o42,s0,h0) pred=(0,0)
DRIFT_PAIR[01] k0_p0_l0_b1        eu_0=155587.250 sel_w=5.122e-9 drf_w=2.939e-9 sel_b=14 sel_d=21(s5,h16,d1=1135) drf_b=14 drf_d=21(o21,s0,h0) pred=(0,0)
DRIFT_PAIR[02] k0_p0_l1_b0        eu_0=155587.250 sel_w=5.733e-9 drf_w=3.551e-9 sel_b=16 sel_d=36(s16,h20,d1=331) drf_b=16 drf_d=36(o36,s0,h0) pred=(197,364)
DRIFT_PAIR[03] k0_p0_l1_b1        eu_0=155587.250 sel_w=7.247e-9 drf_w=7.567e-9 sel_b=34 sel_d=34(s1,h33,d1=1182) drf_b=34 drf_d=34(o34,s0,h0) pred=(1627,1682)
DRIFT_PAIR[04] k0_p1_l0_b0        eu_0=155587.250 sel_w=0.000e0 drf_w=0.000e0 sel_b=16 sel_d=35(s22,h13,d1=405) drf_b=16 drf_d=35(o35,s0,h0) pred=(0,0)
DRIFT_PAIR[05] k0_p1_l0_b1        eu_0=155587.250 sel_w=0.000e0 drf_w=0.000e0 sel_b=14 sel_d=21(s5,h16,d1=1137) drf_b=14 drf_d=21(o21,s0,h0) pred=(0,0)
DRIFT_PAIR[06] k0_p1_l1_b0        eu_0=155587.250 sel_w=0.000e0 drf_w=0.000e0 sel_b=14 sel_d=34(s22,h12,d1=324) drf_b=14 drf_d=34(o34,s0,h0) pred=(152,292)
DRIFT_PAIR[07] k0_p1_l1_b1        eu_0=155587.250 sel_w=0.000e0 drf_w=0.000e0 sel_b=27 sel_d=34(s4,h30,d1=748) drf_b=27 drf_d=34(o34,s0,h0) pred=(1475,442)
DRIFT_PAIR[08] k1000_p0_l0_b0     eu_0=155587.250 sel_w=1.921e-9 drf_w=1.979e-9 sel_b=29 sel_d=42(s21,h21,d1=519) drf_b=29 drf_d=42(o42,s0,h0) pred=(0,0)
DRIFT_PAIR[09] k1000_p0_l0_b1     eu_0=155587.250 sel_w=5.122e-9 drf_w=1.688e-9 sel_b=14 sel_d=21(s5,h16,d1=1135) drf_b=14 drf_d=21(o21,s0,h0) pred=(0,0)
DRIFT_PAIR[10] k1000_p0_l1_b0     eu_0=155587.250 sel_w=5.733e-9 drf_w=5.763e-9 sel_b=16 sel_d=36(s16,h20,d1=331) drf_b=16 drf_d=36(o36,s0,h0) pred=(197,364)
DRIFT_PAIR[11] k1000_p0_l1_b1     eu_0=155587.250 sel_w=7.247e-9 drf_w=8.353e-9 sel_b=34 sel_d=34(s1,h33,d1=1182) drf_b=34 drf_d=34(o34,s0,h0) pred=(1627,1753)
DRIFT_PAIR[12] k1000_p1_l0_b0     eu_0=155587.250 sel_w=0.000e0 drf_w=0.000e0 sel_b=16 sel_d=35(s22,h13,d1=405) drf_b=16 drf_d=35(o35,s0,h0) pred=(0,0)
DRIFT_PAIR[13] k1000_p1_l0_b1     eu_0=155587.250 sel_w=0.000e0 drf_w=0.000e0 sel_b=14 sel_d=21(s5,h16,d1=1137) drf_b=14 drf_d=21(o21,s0,h0) pred=(0,0)
DRIFT_PAIR[14] k1000_p1_l1_b0     eu_0=155587.250 sel_w=0.000e0 drf_w=0.000e0 sel_b=14 sel_d=34(s22,h12,d1=324) drf_b=14 drf_d=34(o34,s0,h0) pred=(152,292)
DRIFT_PAIR[15] k1000_p1_l1_b1     eu_0=155587.250 sel_w=0.000e0 drf_w=0.000e0 sel_b=27 sel_d=34(s4,h30,d1=748) drf_b=27 drf_d=34(o34,s0,h0) pred=(1475,786)
FULL_DRIFT_SUMMARY pairs=16 exercised=16/16 elapsed_s=33.97 worst=8.353e-9 worst_case=k1000_p0_l1_b1
```

---

### Bảng 4 — Toàn bộ `8` `InterventionKind` + `All8Combined` (`27` mục, `--release`, `29,93 s`) & kiểm tra khoá loại trừ

```text
INTERVENTION baseline/RainfallDelta             eff=1 d0=-8.731e-11 d1=-8.731e-11 wt1=8.731e-11 gap=0.000e0 b=0 d=0 p=0
INTERVENTION sel2_full/RainfallDelta            eff=1 d0=+0.000e0 d1=+0.000e0 wt1=0.000e0 gap=0.000e0 b=14 d=34(s34,h0,d1=365) p=130(45.0)
INTERVENTION drift_k1000/RainfallDelta          eff=1 eu_0=155587.250 sel_w=0.000e0 drf_w=0.000e0 sel_b=17 sel_d=33 drf_b=17 drf_d=33(o33) p=(1030,1275)
INTERVENTION baseline/TemperatureDelta          eff=1 d0=-8.731e-11 d1=-8.731e-11 wt1=8.731e-11 gap=1.164e-10 b=0 d=0 p=0
INTERVENTION sel2_full/TemperatureDelta         eff=1 d0=+0.000e0 d1=+0.000e0 wt1=0.000e0 gap=0.000e0 b=14 d=34(s34,h0,d1=364) p=133(45.0)
INTERVENTION drift_k1000/TemperatureDelta       eff=1 eu_0=155587.250 sel_w=0.000e0 drf_w=0.000e0 sel_b=18 sel_d=33 drf_b=18 drf_d=33(o33) p=(893,1144)
INTERVENTION baseline/Deforest                  eff=1 d0=+0.000e0 d1=+0.000e0 wt1=2.910e-11 gap=1.019e-10 b=0 d=0 p=0
INTERVENTION sel2_full/Deforest                 eff=1 d0=+0.000e0 d1=+0.000e0 wt1=0.000e0 gap=0.000e0 b=14 d=34(s34,h0,d1=311) p=57(10.1)
INTERVENTION drift_k1000/Deforest               eff=1 eu_0=155587.250 sel_w=0.000e0 drf_w=0.000e0 sel_b=14 sel_d=34 drf_b=14 drf_d=34(o34) p=(422,599)
INTERVENTION baseline/AddNutrient               eff=1 d0=+0.000e0 d1=+0.000e0 wt1=2.910e-11 gap=2.037e-10 b=0 d=0 p=0
INTERVENTION sel2_full/AddNutrient              eff=1 d0=+0.000e0 d1=+0.000e0 wt1=0.000e0 gap=0.000e0 b=14 d=34(s34,h0,d1=366) p=130(45.1)
INTERVENTION drift_k1000/AddNutrient            eff=1 eu_0=155587.250 sel_w=0.000e0 drf_w=0.000e0 sel_b=17 sel_d=33 drf_b=17 drf_d=33(o33) p=(1030,1275)
INTERVENTION baseline/RemovePredators           eff=1 d0=+2.910e-11 d1=+2.910e-11 wt1=2.910e-11 gap=5.821e-11 b=0 d=0 p=0
INTERVENTION sel2_full/RemovePredators          eff=1 d0=+0.000e0 d1=+0.000e0 wt1=0.000e0 gap=0.000e0 b=14 d=33(s33,h0,d1=365) p=130(45.0)
INTERVENTION drift_k1000/RemovePredators        eff=1 eu_0=155587.250 sel_w=0.000e0 drf_w=2.910e-11 sel_b=19 sel_d=38 drf_b=19 drf_d=38(o38) p=(1549,1526)
INTERVENTION baseline/GeneticInjection(5Mods)   eff=1 d0=+2.910e-11 d1=+2.910e-11 wt1=2.910e-11 gap=5.821e-11 b=0 d=0 p=0
INTERVENTION sel2_full/GeneticInjection(5Mods)  eff=1 d0=+0.000e0 d1=+0.000e0 wt1=0.000e0 gap=0.000e0 b=14 d=34(s34,h0,d1=365) p=130(45.0)
INTERVENTION drift_k1000/GeneticInjection(5Mods) eff=1 eu_0=155587.250 sel_w=0.000e0 drf_w=0.000e0 sel_b=17 sel_d=33 drf_b=17 drf_d=33(o33) p=(1030,1275)
INTERVENTION baseline/ToxicityPlume             eff=1 d0=+0.000e0 d1=+0.000e0 wt1=2.910e-11 gap=1.019e-10 b=0 d=0 p=0
INTERVENTION sel2_full/ToxicityPlume            eff=1 d0=+0.000e0 d1=+0.000e0 wt1=0.000e0 gap=0.000e0 b=14 d=34(s34,h0,d1=311) p=57(10.1)
INTERVENTION drift_k1000/ToxicityPlume          eff=1 eu_0=155587.250 sel_w=0.000e0 drf_w=0.000e0 sel_b=14 sel_d=34 drf_b=14 drf_d=34(o34) p=(422,599)
INTERVENTION baseline/PathogenTransmission      eff=1 d0=+0.000e0 d1=+0.000e0 wt1=2.910e-11 gap=8.731e-11 b=0 d=0 p=0
INTERVENTION sel2_full/PathogenTransmission     eff=1 d0=+0.000e0 d1=+0.000e0 wt1=0.000e0 gap=0.000e0 b=14 d=34(s34,h0,d1=295) p=129(28.1)
INTERVENTION drift_k1000/PathogenTransmission   eff=1 eu_0=155587.250 sel_w=0.000e0 drf_w=0.000e0 sel_b=14 sel_d=34 drf_b=14 drf_d=34(o34) p=(589,607)
INTERVENTION baseline/All8Combined              eff=8 d0=+0.000e0 d1=+0.000e0 wt1=0.000e0 gap=5.093e-10 b=0 d=0 p=0
INTERVENTION sel2_full/All8Combined             eff=8 d0=+0.000e0 d1=+0.000e0 wt1=0.000e0 gap=0.000e0 b=14 d=33(s33,h0,d1=278) p=38(3.9)
INTERVENTION drift_k1000/All8Combined           eff=8 eu_0=155587.250 sel_w=0.000e0 drf_w=0.000e0 sel_b=14 sel_d=33 drf_b=14 drf_d=33(o33) p=(286,336)
FULL_INTERVENTIONS_SUMMARY entries=27 elapsed_s=29.93 worst=5.093e-10 worst_case=baseline/All8Combined
EXCLUDED_GUARD d0=+0.000e0 d1=+0.000e0 wt1=0.000e0 gap=0.000e0 b=15 d=34(s16,h18,d1=564) p=1256(184.1)
M4_OVERALL_SUMMARY total_s=226.68 tolerance=1.0e-3 overall_worst=1.033e-8 violations=0 vacuity_failures=0
```
