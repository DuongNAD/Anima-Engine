---
phase: planning
feature: experiment-e3-productivity-gradient
title: Thẻ W14 (E3-X-run) — Kết quả Xác nhận E3 (insufficient_evidence)
description: Ghi nhận kết quả chạy xác nhận E3-X đầy đủ 36 lượt trên Windows 1 tại commit 24c6fb1c cùng host smoke và kiểm chứng độc lập không cờ bỏ qua
status: active
owner: anima-case
last_reviewed: 2026-11-20
---

# Thẻ W14 (`E3-X-run`) — Kết quả Thí nghiệm Xác nhận E3

## 1. Phạm vi, Pin Commit và Kiểm tra Tiền điều kiện

- **Commit đóng băng (`feat/ecological-selection`, PR `#178`):** `24c6fb1cd6d19f189c3fd06ea2a44c50e28594e1`
- **Máy chạy đơn nhất (`single_host_required`):** `windows/x86_64` (`Windows_NT / AMD64`, Windows 1)
- **Trình biên dịch & hồ sơ:** `rustc 1.98.1 (48a229cea 2026-09-01)`, `cargo 1.98.1 (797e8a9bc 2026-08-05)`, hồ sơ `release` (`--features desktop`)
- **Kiểm tra tiền điều kiện trước khi chạy (`world_artifact_prerun_check` & trạng thái git):**
  - `ANIMA_WORLD_ARTIFACT`: `<unset>` (`anima_world_artifact_unset: true`)
  - `<temp_dir>/anima_world_artifact.anmw` (`C:\Users\Admin\AppData\Local\Temp\anima_world_artifact.anmw`): không tồn tại (`temp_artifact_absent: true`)
  - `git status --porcelain` trước khi build `e3_smoke`: rỗng
  - **Giải thích cờ `-dirty` ở `artifacts/experiments/e3-productivity-gradient-body-mass/host-smoke/smoke/provenance.json` (`git_describe: "24c6fb1-dirty"`, `git_tree_dirty: true`):** trong quá trình `cargo build --release --features desktop --example e3_smoke`, `tauri_build` đã ghi lại hai tệp capability schema desktop `src-tauri/gen/schemas/desktop-schema.json` và `src-tauri/gen/schemas/windows-schema.json` (chỉ đổi kết thúc dòng CRLF/LF, không thay đổi mã mô phỏng). Cả hai tệp này đã được khôi phục nguyên trạng commit `24c6fb1c` trước khi chạy ensemble xác nhận.
  - `git status --porcelain` ngay trước khi chạy `run_e3_experiment --ensemble`: chỉ có `?? artifacts/experiments/e3-productivity-gradient-body-mass/host-smoke/` (`artifacts/experiments/e3-productivity-gradient-body-mass/confirmation/provenance.json` ghi `git_describe: "24c6fb1"` và chỉ liệt kê `artifacts/experiments/e3-productivity-gradient-body-mass/host-smoke/` trong `git_dirty_paths`).
- **Nguồn địa hình & đồng nhất thế giới (`terrain_source` & `world_identity`):**
  - `terrain_source`: `"generated"` trên toàn bộ `36/36` lượt (`world_identity_all_runs_identical: true`)
  - `world_identity`: `seed = 1337`, `generator_version = 1`, `width = 256`, `height = 256`, `checksum = 3152323152`
- **Thời gian thực thi thực tế (UTC):**
  - Build `e3_smoke` (`--release --features desktop -j 4`): `2026-11-20T00:02:52Z` – `2026-11-20T00:13:06Z` (`613.99 s`, binary SHA-256 `79358a511421ac62fbd7db2e5ee58c9baf1c7ca4af460c8d51f92a671748c6b6`)
  - Chạy `e3_smoke` (vào `artifacts/experiments/e3-productivity-gradient-body-mass/host-smoke`): `2026-11-20T00:13:06Z` – `2026-11-20T00:13:22Z` (`15.35 s` CLI wall; `13.9428614 s` trong `timing`)
  - Build `run_e3_experiment` (`--release --features desktop`, `CARGO_BUILD_JOBS=6`): `2026-11-20T00:13:46Z` – `2026-11-20T00:13:55Z` (`8.92 s`, binary SHA-256 `aeead19f4288fff3f24652afedac86715b5cd3d5378805801a53ca78aa0f3c89`)
  - Chạy `run_e3_experiment --ensemble` (`36` lượt: `12` seed × `3` arm × `18000` tick): `2026-11-20T00:13:56Z` – `2026-11-20T00:16:29Z` (`153.55 s` CLI wall; `152.248569 s` trong `artifacts/experiments/e3-productivity-gradient-body-mass/confirmation/timing.json`)

---

## 2. Chạy lại Host Smoke (`artifacts/experiments/e3-productivity-gradient-body-mass/host-smoke/smoke`)

- **Seed hiệu chỉnh:** `999979` (bị loại cơ học khỏi mọi manifest thí nghiệm), `body_mass_consulted: false`
- **Các cổng WP-2, toàn vẹn & WP-3 (tất cả đạt `true`):**
  - `all_observables_emitted_and_finite`: `true`
  - `wp2_a_resource_field.passed`: `true` (`256x256`; `abundant_initial_total_biomass_eu = 153587.24986362457`, `scarcity_initial_total_biomass_eu = 64506.642210796475`, `scarcity_initial_detritus_eu = 89080.6076528281`)
  - `wp2_b_standing_crop_intervention.passed`: `true` (`b1_gradient_shifts_vs_abundant_passed = true`, `treat_vs_ctrl_post_warmup_relative_shift = -0.524726711045983`; `b2_scarcity_matches_treatment_total_passed = true`, `scarcity_vs_treat_post_warmup_relative_mismatch = -0.004375981051686321`)
  - `wp2_c_detritus_before_3000.passed`: `true`
  - `integrity.world_identity_identical`: `true` (`world_identity_observed.checksum = 3152323152`, `all_three_checksums_distinct = true`, `ecology_stream_identical = true`, `genesis_closed_eu_identical = true`)
  - `wp3_schedule_lock.within_budget`: `true` (`us_per_tick = 258.20113703703703`, `projected_minutes = 2.78857228` cho `36 x 18000` tick, `recommended_duration_ticks = 18000`)
- **Hash tham chiếu (`smoke_reference`):**
  - `provenance_sha256`: `b5d6630717f2f04c3e6c996889395511006d0460a6f346c05720705bd44e747f`
  - `smoke_report_sha256`: `d99505fa1875cc04a264df7ede05ec8dee13be6a7102cfaeabda52bd9cd53a5b`

---

## 3. Kiểm chứng Độc lập (`node scripts/verify_e3_artifacts.mjs`)

- Lệnh thực thi: `node scripts/verify_e3_artifacts.mjs` (không dùng `--prereg` hay `--skip-committed-manifests`).
- Mã thoát: `0` — xuất `verify_e3_artifacts OK (D:\teamwork_projects\anima-wt\e3x-run\artifacts\experiments\e3-productivity-gradient-body-mass\confirmation)`.
- **Không có bất kỳ dòng `NOTE:` nào được in ra** (mọi manifest và preregistration được đối chiếu trực tiếp tại commit `24c6fb1cd6d19f189c3fd06ea2a44c50e28594e1`, cùng toàn bộ `checksums.sha256`, `provenance.json`, `host-smoke`, các bảng báo cáo và tính lại toàn phần từ dữ liệu thô).

---

## 4. Kết quả Chính (`H1`) và Quyết định theo `decision_enum`

Áp dụng đúng ngưỡng đóng băng (`min_valid_pairs_for_decision = 10`, `dz_min = 0.8`, `relative_delta_min = 0.05`, `ci_excludes_zero_required = true`, `stopped_early = false`, `failed_runs = []`):

- **Quyết định chính (`primary_decision`):** **`insufficient_evidence`** (`n_complete_pairs = 6 < 10`; `n_extinct_pairs_dropped = 6`, `n_singleton_pairs_dropped = 0`). Theo ràng buộc ngôn ngữ của kế hoạch tiền đăng ký (`docs/ai/planning/2026-08-26-experiment-e3-productivity-gradient-body-mass.md`, §11 dòng 218): *"Không phát hiện được hiệu ứng" ≠ "không có hiệu ứng"* (*not detecting an effect is not evidence of no effect*); `insufficient_evidence` không phải kết quả `null` hay kết quả âm.
- **Tương phản chính (`treatment_gradient - control_uniform_scarcity`, `live.trait_mass_var` tại `T = 18000`):**
  - `control_mean = 0.13187942762959784`, `treatment_mean = 0.1391522290876734`
  - `paired_mean_delta = 0.007272801458075569`, `median_of_per_seed_deltas = -0.0020780901150238246`
  - `paired_sd = 0.09118272702731955`, `paired_se = 0.03722519242873629`
  - `ci95 = [-0.08841760202730037, 0.10296320494345151]`, `paired_dz = 0.0797607364374674`, `decision = "insufficient_evidence"`
- **Độ nhạy Bessel (`H1_bessel_sensitivity`, `var * n / (n - 1)` tại `T = 18000`):**
  - `n_complete_pairs = 6`, `control_mean = 0.14694552807200778`, `treatment_mean = 0.14579905868495305`
  - `paired_mean_delta = -0.001146469387054717`, `median = -0.01762562498543186`, `paired_sd = 0.10248013399267651`, `paired_se = 0.04183733950901781`
  - `ci95 = [-0.1086927743802854, 0.10639983560617597]`, `paired_dz = -0.011187235441521249`, `decision = "insufficient_evidence"`
- **Cửa sổ đuôi (`H1_tail_window`, `[14400, 18000]`):**
  - `n_complete_pairs = 6`, `control_mean = 0.10265723487114876`, `treatment_mean = 0.09089100711143269`
  - `paired_mean_delta = -0.011766227759716079`, `median = -0.001134038173172171`, `paired_sd = 0.10212574009911905`, `paired_se = 0.04169265880782214`
  - `ci95 = [-0.11894061917048607, 0.0954081636510539]`, `paired_dz = -0.11521314556248269`, `decision = "insufficient_evidence"`

---

## 5. Toàn bộ Các Endpoint Đánh giá Trên Cả Ba Tương phản

| Tương phản | Endpoint | `n` | Dropped (`ext/sgl/eh`) | Control Mean | Treatment Mean | Paired Delta | 95% CI | `dz` | Decision |
|---|---|---|---|---|---|---|---|---|---|
| `T - S` | `H1` | 6 | 6 / 0 / 0 | `0.13187943` | `0.13915223` | `0.00727280` | `[-0.08841760, 0.10296320]` | `0.07976074` | `insufficient_evidence` |
| `T - S` | `H1_bessel_sensitivity` | 6 | 6 / 0 / 0 | `0.14694553` | `0.14579906` | `-0.00114647` | `[-0.10869277, 0.10639984]` | `-0.01118724` | `insufficient_evidence` |
| `T - S` | `H1_tail_window` | 6 | 6 / 0 / 0 | `0.10265723` | `0.09089101` | `-0.01176623` | `[-0.11894062, 0.09540816]` | `-0.11521315` | `insufficient_evidence` |
| `T - S` | `S1` (`p90 - p10`) | 6 | 6 / 0 / 0 | `0.39999998` | `0.57048798` | `0.17048800` | `[-0.45835074, 0.79932674]` | `0.28451838` | `insufficient_evidence` |
| `T - S` | `S2` (`trait_mass_mean`) | 6 | 6 / 0 / 0 | `3.10157658` | `3.11910249` | `0.01752591` | `[-0.38079176, 0.41584358]` | `0.04617498` | `insufficient_evidence` |
| `T - S` | `S3` (`east - west`) | 3 | 6 / 0 / 3 | `0.30573273` | `0.13560088` | `-0.17013185` | `[-0.44864782, 0.10838412]` | `-1.51743884` | `insufficient_evidence` |
| `T - S` | `S4_structural_zero` | 12 | 0 / 0 / 0 | `0` | `0` | `0` | `[0, 0]` | `null` | `null` |
| `T - S` | `S5_structural_zero` | 12 | 0 / 0 / 0 | `0` | `0` | `0` | `[0, 0]` | `null` | `null` |
| `T - S` | `S6.mean_lifespan` | 12 | 0 / 0 / 0 | `0` | `0` | `0` | `[0, 0]` | `null` | `null` (`reported_not_interpreted_structural_zero (window drained before the final read)`) |
| `T - S` | `S6.births_sum` | 12 | 0 / 0 / 0 | `55.25` | `199.33333333` | `144.08333333` | `[71.82393872, 216.34272794]` | `1.26691045` | `material_positive` |
| `T - S` | `S6.deaths_sum` | 12 | 0 / 0 / 0 | `170.91666667` | `295.0` | `124.08333333` | `[68.18953799, 179.97712868]` | `1.41051056` | `material_positive` |
| `T - S` | `I1_closed_eu_total` | 12 | 0 / 0 / 0 | `165587.24986361366` | `165587.24986362437` | `1.07126349e-8` | `[7.52437693e-9, 1.39008929e-8]` | `2.13485970` | `null` |
| `T - S` | `M1_standing_crop_final` | 12 | 0 / 0 / 0 | `64465.11026401` | `64605.52474811` | `140.41448410` | `[-80.49299970, 361.32196791]` | `0.40385733` | `null` |
| `T - S` | `M1_standing_crop_post_warmup` | 12 | 0 / 0 / 0 | `64411.42824685` | `64536.08594762` | `124.65770077` | `[-38.12218586, 287.43758739]` | `0.48656948` | `null` |
| `T - U` | `H1` | 7 | 4 / 1 / 0 | `0.07111141` | `0.11985720` | `0.04874578` | `[-0.01348918, 0.11098075]` | `0.72438910` | `insufficient_evidence` |
| `T - U` | `H1_bessel_sensitivity` | 7 | 4 / 1 / 0 | `0.07212010` | `0.12555999` | `0.05343989` | `[-0.01305159, 0.11993137]` | `0.74330807` | `insufficient_evidence` |
| `T - U` | `H1_tail_window` | 7 | 4 / 1 / 0 | `0.05347176` | `0.07840530` | `0.02493354` | `[-0.01374660, 0.06361367]` | `0.59616329` | `insufficient_evidence` |
| `T - U` | `S1` (`p90 - p10`) | 7 | 4 / 1 / 0 | `0.51370641` | `0.48898969` | `-0.02471672` | `[-0.37254578, 0.32311234]` | `-0.06571950` | `insufficient_evidence` |
| `T - U` | `S2` (`trait_mass_mean`) | 8 | 4 / 0 / 0 | `2.99320056` | `3.06542975` | `0.07222919` | `[-0.13100437, 0.27546275]` | `0.29712177` | `insufficient_evidence` |
| `T - U` | `S3` (`east - west`) | 6 | 4 / 0 / 2 | `0.04643260` | `0.15982227` | `0.11338966` | `[-0.19080080, 0.41758012]` | `0.39118634` | `insufficient_evidence` |
| `T - U` | `S4_structural_zero` | 12 | 0 / 0 / 0 | `0` | `0` | `0` | `[0, 0]` | `null` | `null` |
| `T - U` | `S5_structural_zero` | 12 | 0 / 0 / 0 | `0` | `0` | `0` | `[0, 0]` | `null` | `null` |
| `T - U` | `S6.mean_lifespan` | 12 | 0 / 0 / 0 | `0` | `0` | `0` | `[0, 0]` | `null` | `null` (`reported_not_interpreted_structural_zero (window drained before the final read)`) |
| `T - U` | `S6.births_sum` | 12 | 0 / 0 / 0 | `388.66666667` | `199.33333333` | `-189.33333333` | `[-301.57453069, -77.09213598]` | `-1.07176922` | `material_negative_opposite` |
| `T - U` | `S6.deaths_sum` | 12 | 0 / 0 / 0 | `442.66666667` | `295.0` | `-147.66666667` | `[-235.25317820, -60.08015513]` | `-1.07120288` | `material_negative_opposite` |
| `T - U` | `I1_closed_eu_total` | 12 | 0 / 0 / 0 | `165587.24986362393` | `165587.24986362437` | `4.60810649e-10` | `[-7.65709155e-10, 1.68733045e-9]` | `0.23871210` | `null` |
| `T - U` | `M1_standing_crop_final` | 12 | 0 / 0 / 0 | `119410.94393507` | `64605.52474811` | `-54805.41918696` | `[-62507.57886182, -47103.25951211]` | `-4.52103092` | `null` |
| `T - U` | `M1_standing_crop_post_warmup` | 12 | 0 / 0 / 0 | `133528.66177039` | `64536.08594762` | `-68992.57582277` | `[-72942.04104666, -65043.11059888]` | `-11.09917137` | `null` |
| `S - U` | `H1` | 6 | 6 / 0 / 0 | `0.07808722` | `0.13187943` | `0.05379221` | `[-0.07694284, 0.18452726]` | `0.43180052` | `insufficient_evidence` |
| `S - U` | `H1_bessel_sensitivity` | 6 | 6 / 0 / 0 | `0.07920665` | `0.14694553` | `0.06773888` | `[-0.07335151, 0.20882927]` | `0.50384433` | `insufficient_evidence` |
| `S - U` | `H1_tail_window` | 6 | 6 / 0 / 0 | `0.05978151` | `0.10265723` | `0.04287572` | `[-0.04120952, 0.12696097]` | `0.53511546` | `insufficient_evidence` |
| `S - U` | `S1` (`p90 - p10`) | 6 | 6 / 0 / 0 | `0.59932415` | `0.39999998` | `-0.19932417` | `[-0.71448473, 0.31583639]` | `-0.40604407` | `insufficient_evidence` |
| `S - U` | `S2` (`trait_mass_mean`) | 6 | 6 / 0 / 0 | `3.02025589` | `3.10157658` | `0.08132070` | `[-0.43448550, 0.59712689]` | `0.16545136` | `insufficient_evidence` |
| `S - U` | `S3` (`east - west`) | 3 | 6 / 0 / 3 | `0.05256781` | `0.30573273` | `0.25316492` | `[-0.55903719, 1.06536703]` | `0.77431039` | `insufficient_evidence` |
| `S - U` | `S4_structural_zero` | 12 | 0 / 0 / 0 | `0` | `0` | `0` | `[0, 0]` | `null` | `null` |
| `S - U` | `S5_structural_zero` | 12 | 0 / 0 / 0 | `0` | `0` | `0` | `[0, 0]` | `null` | `null` |
| `S - U` | `S6.mean_lifespan` | 12 | 0 / 0 / 0 | `0` | `0` | `0` | `[0, 0]` | `null` | `null` (`reported_not_interpreted_structural_zero (window drained before the final read)`) |
| `S - U` | `S6.births_sum` | 12 | 0 / 0 / 0 | `388.66666667` | `55.25` | `-333.41666667` | `[-415.13180099, -251.70153235]` | `-2.59245542` | `material_negative_opposite` |
| `S - U` | `S6.deaths_sum` | 12 | 0 / 0 / 0 | `442.66666667` | `170.91666667` | `-271.75` | `[-335.06137300, -208.43862700]` | `-2.72718320` | `material_negative_opposite` |
| `S - U` | `I1_closed_eu_total` | 12 | 0 / 0 / 0 | `165587.24986362393` | `165587.24986361366` | `-1.02518243e-8` | `[-1.31505260e-8, -7.35312259e-9]` | `-2.24710890` | `null` |
| `S - U` | `M1_standing_crop_final` | 12 | 0 / 0 / 0 | `119410.94393507` | `64465.11026401` | `-54945.83367107` | `[-62641.32472549, -47250.34261664]` | `-4.53654184` | `null` |
| `S - U` | `M1_standing_crop_post_warmup` | 12 | 0 / 0 / 0 | `133528.66177039` | `64411.42824685` | `-69117.23352353` | `[-73060.59295811, -65173.87408896]` | `-11.13644236` | `null` |

*(Trong bảng trên, `T = treatment_gradient`, `S = control_uniform_scarcity`, `U = control_uniform`; `zero_control_mean_fallback_used = false` ở mọi mục.)*

- **Ghi chú bằng không do cấu trúc (`S4`, `S5`, và `S6.mean_lifespan`):**
  - `S4_structural_zero` (`live.shannon`) và `S5_structural_zero` (`live.archive_coverage`) bằng không do cấu trúc trong headless adapter và không được diễn giải (theo kế hoạch §11).
  - `S6.mean_lifespan` (`live.mean_lifespan`) bằng `0` trong `final-observables.json` ở toàn bộ `36/36` lượt là **bằng không do cấu trúc** (`reported_not_interpreted_structural_zero (window drained before the final read)`), không phải kết quả sinh học hay kết quả `null`. Nguyên nhân: `final_observables = model.observables()` (`src-tauri/src/core/experiment_runner.rs:548`) được đọc sau khi mẫu tick `18000` đã rút cạn cửa sổ tuổi thọ, nên adapter trả `mean_lifespan.unwrap_or(0.0)` (`src-tauri/src/core/live_experiment.rs:1709-1711`), khiến định nghĩa đã đăng ký (`src-tauri/tests/fixtures/experiments_e3/e3-preregistration.json:218,359`) đọc một cửa sổ rỗng. Do tiền đăng ký cấm đổi metric sau khi có kết quả (`e3-preregistration.json:391`, `metric_switching_after_results: "forbidden"`), báo cáo giữ nguyên định nghĩa đã đăng ký và không thay thế bằng chỉ số tuổi thọ khác; hiện tượng cửa sổ bị rút cạn trước lần đọc cuối được ghi nhận như một phát hiện để sửa về sau.

---

## 6. Bảng Loại trừ, Dân số tại `T = 18000` (`agent_count_at_T_by_arm`) và Phân rã Cộng tính

### 6.1. `failed_runs`, Bảng Tuyệt chủng, Đơn độc và Nửa bản đồ Trống

- **`failed_runs`:** `[]` (`36/36` lượt hoàn tất với `status = "completed"`).
- **`extinction_table_by_arm` (`11` lượt có `live.agent_count == 0`):**
  - `730001`: `control_uniform_scarcity` (`first_extinct_tick = 10200`), `treatment_gradient` (`4800`)
  - `732001`: `control_uniform` (`5400`), `control_uniform_scarcity` (`3600`), `treatment_gradient` (`6000`)
  - `734001`: `control_uniform_scarcity` (`4200`)
  - `736001`: `control_uniform_scarcity` (`3600`), `treatment_gradient` (`4800`)
  - `737001`: `control_uniform_scarcity` (`18000`)
  - `741001`: `control_uniform_scarcity` (`3600`), `treatment_gradient` (`4200`)
- **`singleton_table_by_arm` (`1` lượt):**
  - `737001`: `control_uniform`, `tick = 18000`, `agent_count = 1`, `scope = "final_T"`
- **`per_arm_empty_half_counts`:** `control_uniform = 1`, `control_uniform_scarcity = 3`, `treatment_gradient = 1`
- **`empty_half_table_by_arm` (`5` lượt còn sống tại `T = 18000` có nửa trống):**
  - `731001`: `control_uniform_scarcity` (`west`)
  - `733001`: `control_uniform_scarcity` (`east`)
  - `735001`: `control_uniform_scarcity` (`east`), `treatment_gradient` (`west`)
  - `737001`: `control_uniform` (`west`)

### 6.2. Biến trung gian Dân số tại `T = 18000` (`agent_count_at_T_by_arm`)

| Seed | `control_uniform` | `control_uniform_scarcity` | `treatment_gradient` |
|---|---|---|---|
| `730001` | 89 | 0 | 0 |
| `731001` | 70 | 2 | 18 |
| `732001` | 0 | 0 | 0 |
| `733001` | 70 | 15 | 16 |
| `734001` | 86 | 0 | 107 |
| `735001` | 43 | 2 | 14 |
| `736001` | 106 | 0 | 0 |
| `737001` | 1 | 0 | 23 |
| `738001` | 81 | 5 | 22 |
| `739001` | 93 | 11 | 64 |
| `740001` | 56 | 17 | 28 |
| `741001` | 97 | 0 | 0 |

### 6.3. Phân rã Cộng tính (`additive_decomposition_table_on_all_three_valid_seeds`, `6` seed sống cả 3 arm)

| Seed | `control_uniform` | `control_uniform_scarcity` | `treatment_gradient` | `T - U` | `T - S` | `S - U` | `residual` |
|---|---|---|---|---|---|---|---|
| `731001` | `0.11284896613919053` | `0.0` | `0.15802467252001406` | `0.045175706380823535` | `0.15802467252001406` | `-0.11284896613919053` | `0.0` |
| `733001` | `0.06353214545251551` | `0.2899106811702359` | `0.23351519206576632` | `0.1699830466132508` | `-0.056395489104469565` | `0.22637853571772037` | `0.0` |
| `735001` | `0.02830015251246497` | `0.0` | `0.006434411511268731` | `-0.02186574100119624` | `0.006434411511268731` | `-0.02830015251246497` | `0.0` |
| `738001` | `0.1167286313031241` | `0.15359998168945369` | `0.1430093899481373` | `0.02628075864501321` | `-0.01059059174131638` | `0.03687135038632959` | `0.0` |
| `739001` | `0.08853188341203042` | `0.25475553338276125` | `0.14994702638891583` | `0.06141514297688541` | `-0.10480850699384542` | `0.16622364997073083` | `0.0` |
| `740001` | `0.058581521378854674` | `0.0930103695351363` | `0.14398268209193832` | `0.08540116071308365` | `0.05097231255680201` | `0.03442884815628163` | `0.0` |
