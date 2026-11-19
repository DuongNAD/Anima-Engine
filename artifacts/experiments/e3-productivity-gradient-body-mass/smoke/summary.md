# E3 WP-2 / WP-3 Smoke Calibration Report

> Calibration only, smoke seed `999979`, EXCLUDED from every analysis, table, statistic, and claim. Body-mass observables were never read or written (`body_mass_consulted = false`).

Model: `core::live_experiment::LiveExperimentAdapter` (`live-bevy-world/2`) — **headless adapter verified**.

- Smoke seed: `999979` (`2` arms x `18000` ticks, `sample_period = 600`)
- Profile: `release`
- Wall clock: **11.640 s** (`323.34 us/tick`)
- Checksums: control `1266395716`, treatment `2891653359` (differ: `true`)

## WP-2 (a) ResourceField in headless LiveExperimentAdapter

- Gate result: **PASS**
- Dimensions: `256x256` (`x in [-100.0, 100.0]`, `z in [-100.0, 100.0]`)
- Initial field biomass / capacity: `153587.250` / `153587.250` EU
- Initial West (`col < 128`) biomass / capacity: `88757.750` / `88757.750` EU
- Initial East (`col >= 128`) biomass / capacity: `64829.500` / `64829.500` EU
- Initial `EcosystemBiomass`: plants `153587.250`, detritus `0.000`, animals `0.000`, dormant `0.000`

## WP-2 (b) Regional interventions change `live.standing_crop`

- Gate result: **PASS**

| Metric | Control | Treatment | Treatment - Control |
|---|---:|---:|---:|
| `live.standing_crop` at T=18000 | 123300.837 | 63282.053 | -60018.784 |
| Post-warmup mean (`tick > 3000`) | 134866.660 | 64098.521 | -70768.139 |
| Tail window mean (`14400..=18000`) | 124085.042 | 63381.917 | -60703.126 |
| Max abs sample difference | — | — | 86992.337 |
| `Deforest` ledger firings / EU moved (`cause_id=1`) | 0 / 0.000 | 300 / 93891.597 | — |
| `AddNutrient` ledger firings / EU added (`cause_id=2`) | 0 / 0.000 | 300 / 33642.288 | — |

## WP-2 (c) `detritus > 0` before tick 3000

- Gate result: **PASS**
- First positive sample `< 3000`: control `Some((600, 1647.807546613461))`, treatment `Some((600, 86709.16853561654))`

| Tick | Control `detritus` (EU) | Treatment `detritus` (EU) |
|---:|---:|---:|
| 600 | 1647.808 | 86709.169 |
| 1200 | 3365.437 | 90785.145 |
| 1800 | 6624.690 | 94169.752 |
| 2400 | 9182.101 | 95790.687 |
| 3000 | 12530.862 | 97795.366 |

## WP-3 Schedule & Budget Calibration

| Ladder rung (`T`) | Projected 24-run minutes | <= 90 min budget |
|---:|---:|---|
| 18000 | 2.328 | yes |
| 12000 | 1.552 | yes |
| 6000 | 0.776 | yes |

- Recommended `T`: `Some(18000)`
- Warm-up window: `3000` ticks (`5` samples excluded)
- `sample_period`: `600`
- Tail window: `[14400, 18000]`

## Demographics & Closed-EU Diagnostics (Smoke Seed Only)

| Arm | Status | `agent_count` min / max / final | Sum `births` | Sum `deaths` (starve / dehydrate) | Max `closed_eu_total` abs drift |
|---|---|---:|---:|---:|---:|
| control | completed | 13 / 193 / 13 | 375 | 482 (100 / 382) | 1.076842e-9 |
| treatment | completed | 9 / 192 / 99 | 370 | 391 (58 / 333) | 4.656613e-10 |
