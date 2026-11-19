# E3 WP-2 / WP-3 Smoke Calibration Report (3-Arm Design)

> Calibration only, smoke seed `999979`, EXCLUDED from every analysis, table, statistic, and claim. No body-mass observable value was written, printed, or used in any gate except a finiteness check (`is_finite()` over every emitted observable; `body_mass_consulted = false`).

Model: `core::live_experiment::LiveExperimentAdapter` (`live-bevy-world/2`) -- **headless adapter verified**.

- Smoke seed: `999979` (`3` arms x `18000` ticks, `sample_period = 600`)
- Profile: `release`
- Wall clock: **17.727 s** (`328.28 us/tick`)
- Checksums: `control_uniform` `1266395716`, `control_uniform_scarcity` `2359367161`, `treatment_gradient` `2891653359`

## WP-2 (a) ResourceField in headless LiveExperimentAdapter

- Gate result: **PASS**
- Dimensions: `256x256` (`x in [-100.0, 100.0]`, `z in [-100.0, 100.0]`)
- Abundant arms (`scale=1.0`) initial plants / capacity: `153587.250` / `153587.250` EU (West `88757.750`, East `64829.500`, detritus `0.000`)
- Uniform-scarcity arm (`scale=0.42`) initial plants / capacity: `64506.642` / `64506.642` EU (West `37278.253`, East `27228.389`, detritus `89080.608`)
- Identical initial closed EU across all 3 arms: `153587.250` EU

## WP-2 (b1 & b2) Standing Crop Manipulation & Total-Scarcity Control Match

- Overall WP-2 (b) gate result: **PASS** (`b1`: `PASS`, `b2`: `PASS`)

| Metric | `control_uniform` | `control_uniform_scarcity` | `treatment_gradient` | `treat - ctrl_u` | `treat - ctrl_s` |
|---|---:|---:|---:|---:|---:|
| `live.standing_crop` at T=18000 | 123300.837 | 64483.464 | 63282.053 | -60018.784 | -1201.411 |
| Post-warmup mean (`tick > 3000`) | 134866.660 | 64379.015 | 64098.521 | -70768.139 (-52.47%) | -280.494 (-0.44%) |
| Tail window mean (`14400..=18000`) | 124085.042 | 64467.845 | 63381.917 | -60703.126 | -1085.928 |
| Max abs sample difference | -- | -- | -- | 86992.337 | 3402.588 |
| `Deforest` ledger (`cause_id=1`) | 0 (0.0 EU) | 0 (0.0 EU) | 300 (93891.6 EU) | -- | -- |
| `AddNutrient` ledger (`cause_id=2`) | 0 (0.0 EU) | 300 (8521.1 EU) | 300 (33642.3 EU) | -- | -- |

## WP-2 (c) `detritus > 0` before tick 3000 (all 3 arms)

- Gate result: **PASS**
- First positive `< 3000`: `control_uniform` `Some((600, 1647.807546613461))`, `control_uniform_scarcity` `Some((600, 90188.81396238769))`, `treatment_gradient` `Some((600, 86709.16853561654))`

| Tick | `control_uniform` detritus | `control_uniform_scarcity` detritus | `treatment_gradient` detritus |
|---:|---:|---:|---:|
| 600 | 1647.808 | 90188.814 | 86709.169 |
| 1200 | 3365.437 | 91266.918 | 90785.145 |
| 1800 | 6624.690 | 92755.881 | 94169.752 |
| 2400 | 9182.101 | 93978.302 | 95790.687 |
| 3000 | 12530.862 | 95907.225 | 97795.366 |

## WP-3 Schedule & Budget Calibration (36 Planned Runs = 3 Arms x 12 Seeds)

| Ladder rung (`T`) | Projected 36-run minutes | <= 90 min budget |
|---:|---:|---|
| 18000 | 3.545 | yes |
| 12000 | 2.364 | yes |
| 6000 | 1.182 | yes |

- Recommended `T`: `Some(18000)`
- Warm-up window: `3000` ticks (`5` samples excluded)
- `sample_period`: `600`
- Tail window: `[14400, 18000]`

## Demographics & Closed-EU Diagnostics (Smoke Seed Only)

| Arm | Status | `agent_count` min / max / final | Sum `births` | Sum `deaths` (starve / dehydrate) | Max `closed_eu_total` abs drift |
|---|---|---:|---:|---:|---:|
| control_uniform | completed | 13 / 193 / 13 | 375 | 482 (100 / 382) | 1.076842e-9 |
| control_uniform_scarcity | completed | 11 / 120 / 12 | 76 | 184 (0 / 184) | 1.050648e-8 |
| treatment_gradient | completed | 9 / 192 / 99 | 370 | 391 (58 / 333) | 4.656613e-10 |
