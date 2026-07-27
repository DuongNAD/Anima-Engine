# E2 — per-agent evolved brain vs shared BrainModel

Model: `core::live_experiment::LiveExperimentAdapter` — **headless adapter verified**, which is the only status claim permitted. Harness: `core::experiment_runner::run_paired_ensemble_with_control`.

- seeds run: `[700001, 701001, 702001, 703001, 704001, 705001, 706001, 707001, 708001, 709001, 710001, 711001]`
- duration: **18000 ticks** (committed 18000, ladder [18000, 12000, 6000]), sample period 600
- complete pairs: **12 of 12** (decision needs 10)
- wall clock: **157.2 s** (2.62 min) against a registered ceiling of 90 min
- declared factors: `["initial_conditions"]`

## Integrity (gates E2-G1, E2-G3, E2-G6)

| check | result |
|---|---|
| control founders carrying `AgentBrain` | 0 of 10 |
| treatment founders carrying `AgentBrain` | 10 of 10 |
| ecology stream identical after genesis | yes |
| world identity identical across arms | yes |

## Every observable the run produced

Roles were fixed before the numbers existed and cannot move afterwards. `delta` is treatment − control in the observable's own unit.

| observable | role | n | control mean | treatment mean | delta | median delta | SD | SE | 95% CI | d_z | rel. shift | material |
|---|---|--:|--:|--:|--:|--:|--:|--:|:--:|--:|--:|:--:|
| `detritus` | reported | 12 | 1020.354665 | 1020.859713 | 0.505048 | 1.163787 | 4.557490 | 1.315634 | [-2.073595, 3.083690] | 0.110817 | +0.05% | no |
| `live.agent_count` | secondary | 12 | 10.000000 | 10.000000 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | [0.000000, 0.000000] | n/a | +0.00% | no |
| `live.animals_eu` | secondary | 12 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | [0.000000, 0.000000] | n/a | n/a | no |
| `live.closed_eu_total` | harness integrity | 12 | 154587.249864 | 154587.249864 | -2.559e-9 | 8.789e-9 | 3.900e-8 | 1.126e-8 | [-2.462e-8, 1.951e-8] | -0.065613 | -0.00% | no |
| `live.food_items` | reported | 12 | 50.000000 | 50.000000 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | [0.000000, 0.000000] | n/a | +0.00% | no |
| `live.herbivore_count` | reported | 12 | 7.000000 | 7.000000 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | [0.000000, 0.000000] | n/a | +0.00% | no |
| `live.mean_agent_energy` | primary | 12 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | [0.000000, 0.000000] | n/a | n/a | no |
| `live.predator_count` | secondary | 12 | 3.000000 | 3.000000 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | [0.000000, 0.000000] | n/a | +0.00% | no |
| `live.season_phase` | reported | 12 | 6.282562 | 6.282562 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | [0.000000, 0.000000] | n/a | +0.00% | no |
| `live.standing_crop` | reported | 12 | 153566.895198 | 153566.390151 | -0.505048 | -1.163787 | 4.557490 | 1.315634 | [-3.083690, 2.073595] | -0.110817 | -0.00% | no |
| `plants` | reported | 12 | 153566.895198 | 153566.390151 | -0.505048 | -1.163787 | 4.557490 | 1.315634 | [-3.083690, 2.073595] | -0.110817 | -0.00% | no |

## The primary metric, against the three registered conditions

`live.mean_agent_energy`, paired per seed, hypothesised **negative** (H1).

| condition | required | measured | met |
|---|---|---|:--:|
| 95% CI excludes 0 | CI must not contain 0 | [0.000000, 0.000000] | no |
| \|d_z\| ≥ 0.8 | large by convention | n/a | no |
| \|delta\| ≥ 5% of \|control mean\| | relative shift | undefined | yes |

**Material: no.**

## Failures and warnings

No run failed.

No run produced a warning.

## What this cannot establish

- **Nothing about evolution.** The adapter drains epoch statistics and applies no replacement, so the founding brains are the only brains a run ever has and nothing is ever selected.
- **Nothing about worlds.** These seeds are stochastic samples of ONE world design; the map never varied, so between-world variance was never sampled.
- **Nothing about the desktop app.** No app was started; the status of the live world is *headless adapter verified* and this run does not change it.
- **Nothing about behavioural diversity**, which is EB-S11's bespoke harness and not a registry observable.

Every number above traces to a file in this directory; `checksums.sha256` covers all of them.
