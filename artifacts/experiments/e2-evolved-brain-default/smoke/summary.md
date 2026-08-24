# E2 — per-agent evolved brain vs shared BrainModel

> ⛔ **Calibration only, seed 999983, EXCLUDED from every analysis, table, statistic and claim.** These numbers exist to time the harness and prove it runs. They are not evidence about brains and may not be quoted as any part of the E2 result.

Model: `core::live_experiment::LiveExperimentAdapter` — **headless adapter verified**, which is the only status claim permitted. Harness: `core::experiment_runner::run_paired_ensemble_with_control`.

- seeds run: `[999983]`
- duration: **18000 ticks** (committed 18000, ladder [18000, 12000, 6000]), sample period 600
- complete pairs: **1 of 1** (decision needs 10)
- wall clock: **10.2 s** (0.17 min) against a registered ceiling of 90 min
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
| `detritus` | reported | 1 | 1018.747442 | 1018.747442 | 1.312e-10 | 1.312e-10 | n/a | n/a | n/a | n/a | +0.00% | no |
| `live.agent_count` | secondary | 1 | 10.000000 | 10.000000 | 0.000000 | 0.000000 | n/a | n/a | n/a | n/a | +0.00% | no |
| `live.animals_eu` | secondary | 1 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | n/a | n/a | n/a | n/a | n/a | no |
| `live.closed_eu_total` | harness integrity | 1 | 154587.249863 | 154587.249863 | 5.821e-10 | 5.821e-10 | n/a | n/a | n/a | n/a | +0.00% | no |
| `live.food_items` | reported | 1 | 50.000000 | 50.000000 | 0.000000 | 0.000000 | n/a | n/a | n/a | n/a | +0.00% | no |
| `live.herbivore_count` | reported | 1 | 7.000000 | 7.000000 | 0.000000 | 0.000000 | n/a | n/a | n/a | n/a | +0.00% | no |
| `live.mean_agent_energy` | primary | 1 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | n/a | n/a | n/a | n/a | n/a | no |
| `live.predator_count` | secondary | 1 | 3.000000 | 3.000000 | 0.000000 | 0.000000 | n/a | n/a | n/a | n/a | +0.00% | no |
| `live.season_phase` | reported | 1 | 6.282562 | 6.282562 | 0.000000 | 0.000000 | n/a | n/a | n/a | n/a | +0.00% | no |
| `live.standing_crop` | reported | 1 | 153568.502421 | 153568.502421 | 0.000000 | 0.000000 | n/a | n/a | n/a | n/a | +0.00% | no |
| `plants` | reported | 1 | 153568.502421 | 153568.502421 | 4.657e-10 | 4.657e-10 | n/a | n/a | n/a | n/a | +0.00% | no |

## The primary metric, against the three registered conditions

`live.mean_agent_energy`, paired per seed, hypothesised **negative** (H1).

| condition | required | measured | met |
|---|---|---|:--:|
| 95% CI excludes 0 | CI must not contain 0 | undefined | no |
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
