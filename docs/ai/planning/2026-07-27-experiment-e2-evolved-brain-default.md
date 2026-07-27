---
phase: planning
feature: experiment-e2-evolved-brain-default
title: Plan — E2 run protocol, thresholds, cost budget and the ADR-0003 decision rule
description: N, seeds, T, sampling, stopping, failure handling, the materiality threshold, and what each outcome does to ADR-0003
status: active
owner: maintainers
last_reviewed: 2026-07-27
requirements: ../requirements/2026-07-27-experiment-e2-evolved-brain-default.md
design: ../design/2026-07-27-experiment-e2-evolved-brain-default.md
testing: ../testing/2026-07-27-experiment-e2-evolved-brain-default.md
state: ../../planning/STATE_OF_THE_PROJECT.md
---

# Plan — E2 run protocol, thresholds, cost budget and the ADR-0003 decision rule

> 🔒 **Preregistration.** Committed before any E2 run. No result, no observed direction. Parent
> commit `96d54d9`. The machine-readable form of everything below is
> `src-tauri/tests/fixtures/experiments_e2/e2-preregistration.json`, and
> `tests/prereg_e2_manifest_tests.rs` asserts the two agree.

## 1. N and the seeds

**N = 12 experimental seeds**, listed here in **execution order** — the manifest order, which is the
order `run_paired_ensemble_with_control` iterates:

```
700001  701001  702001  703001  704001  705001
706001  707001  708001  709001  710001  711001
```

Rule, fixed before any run: `seed_k = 700001 + 1000·k`, `k ∈ 0..11`. It is arithmetic and auditable
precisely so nobody can claim a seed was chosen for its result; `ChaCha12Rng::seed_from_u64` hashes
the value, so regular spacing carries no structure into the stream.
`twelve_experimental_seeds_are_declared_in_execution_order_and_are_unique` checks the committed list
against the rule.

**Minimum for a decision: 10 complete pairs.** Twelve are requested so two pairs may fail without
dropping the design below the bar the brief sets. Fewer than ten complete pairs is *insufficient
evidence* (§6), never a decision taken on nine.

## 2. The smoke seed

**999983.** One seed, its own two manifests, used to calibrate the harness and prove it runs. It is
**never** part of any analysis, table, statistic or claim.

The exclusion is **mechanical, not a promise**: 999983 appears in neither experimental manifest's
seed list, and `run_manifest_seed` returns `RunStatus::Failed { SeedNotInManifest }` for a seed the
manifest does not declare. It cannot contribute a run to the ensemble even if somebody asks for it
by hand. `the_smoke_seed_cannot_enter_the_experimental_ensemble` pins both halves.

**No smoke run happened in E2-A.** The smoke run is the first thing E2-B does, and it is the only
run that may precede the ensemble.

## 3. Duration, sampling, warm-up

| parameter | value | why |
|---|---|---|
| `duration_ticks` (**T**) | **18,000** | 300 s of simulated time at 60 Hz; 300 firings of the ecology band (period 60); 18 epoch boundaries at `ticks_per_epoch = 1000`. Long enough for the founding cohort's energy budget to be decided by feeding rather than by starting conditions |
| `sample_period` | **600** | 30 samples per run, one per 10 simulated seconds |
| warm-up | **first 3,000 ticks (5 samples)** | excluded from every *time-averaged* statistic; the primary metric is the value at T and is unaffected |
| tail window | ticks **14,400–18,000** (samples 24–30) | the preregistered secondary form of the primary metric, mean over the last 20 % of samples, to check the endpoint is not a lucky snapshot |
| stopping | **fixed horizon** | every run goes to T. No early stopping, no peeking, no "it looked converged" |

`drive()` already stops a run early and marks it `Failed` if any observable becomes non-finite. That
is the only early stop, it is the runner's, and it produces a preserved failure rather than a
truncated result.

### The duration ladder — the only permitted deviation

`[18000, 12000, 6000]`, descending, declared now. The rung is chosen **once**, from the smoke
calibration, **before any experimental seed runs**, and recorded in `provenance.json`. It may never
be changed after an experimental seed has run, and **N is never lowered** — the brief's rule and the
right one: shortening a run costs resolution, dropping seeds costs the design.

## 4. Failure handling

- A one-sided failure keeps its `SeedPair`; only complete pairs contribute to statistics. This is
  the runner's own behaviour and is not overridden.
- **Every** failed run is reported with its seed and reason, in `paired-report.json` and in
  `summary.md`. None is dropped, retried into silence, or replaced by a fresh seed.
- A failed pair is **not** replaced by seed 712001 or any other. The seed list is closed.
- If both arms fail on the same seed for the same reason, that is a harness defect: stop, fix, and
  re-run the **whole** ensemble from the top under the same preregistration.
- If a `live.closed_eu_total` difference exceeds materiality, the run completes and the finding is
  filed as a defect (requirements §4). It does not become a result about brains.

## 5. Cost, and why `--release` is mandatory

**Total work:** 12 seeds × 2 arms × 18,000 ticks = **432,000 ticks**.

**Measured evidence.** From `benchmark_report.json` (Criterion medians, release, Intel Core
i5-14600KF — the same machine and the target hardware), the systems on the live schedule that have
per-tick numbers:

| system | µs/tick |
|---|---:|
| `ResourceField::step_regrowth_gated_strided` | 55.0 |
| `rebuild_spatial_grid_system` @ 100 agents | 13.4 |
| `integrate_physics_system` @ 100 agents | 0.5 |
| **measured subset** | **≈ 68.9** |

That is a **lower bound on a subset**, at an agent count above this experiment's ten, and it is not
a tick. The rest of `build_tick_schedule` — ECS scheduling, `sensory_system`, the Burn inference
pump, CPG, pheromone diffusion, collision, combat, metabolism, the census — has no measurement.
`step_water`/`step_soil`/`step_erosion` are **not** on this schedule and are excluded.

**Labelled engineering estimate, not a measurement:** full tick = 3×–15× the measured subset ⇒
0.21–1.03 ms/tick in release. Ensemble ⇒ **1.5–7.4 minutes**. In a debug build (10×–30× slower) the
same work is **15 minutes to 3.7 hours**, which is the entire reason the profile is part of the
preregistration rather than left to whoever types the command.

**Maximum wall-clock budget: 90 minutes** for the twelve-seed ensemble, excluding compilation and
the smoke run. T = 18,000 was chosen to sit an order of magnitude inside that, so the estimate can
be wrong by 10× and the budget still holds.

**If exceeded:** step down the ladder (§3) using the smoke calibration only, before any experimental
seed runs. Never lower N. Never change T once data exists.

## 6. Analysis, thresholds, and reporting

### 6.1 The statistic

Per observable, over complete pairs: `PairedEffect`, computed by the runner —
`n_requested`, `n_complete_pairs`, `control_mean`, `treatment_mean`, `paired_mean_delta`,
`paired_sd`, `paired_se`, `ci95_low`, `ci95_high`, `paired_dz`.

`paired_mean_delta` is treatment − control in the observable's own unit and is the **primary
effect**. `paired_dz` is Cohen's *d_z*. Both are `Option` and the type documents exactly when each
is undefined; a `None` is reported as `None`, never as zero.

Derived by E2-B from the same report, not by a new statistic:

- **median** of the per-seed deltas — reported beside the mean, because with n = 12 one outlying
  seed moves a mean and not a median, and the two disagreeing is itself informative;
- **between-seed variance** of the deltas, which is `paired_sd²`;
- the **per-seed delta table**, so every pair is visible rather than summarised away.

### 6.2 "Different enough" — all three conditions, decided in advance

For the primary metric, the effect counts as material only if **all** hold:

1. the 95 % CI on `paired_mean_delta` **excludes 0**;
2. `|paired_dz| ≥ 0.8` — a large effect by convention;
3. `|paired_mean_delta| ≥ 0.05 × |control_mean|` — at least a 5 % relative shift.

Requiring both a standardized and a relative threshold is deliberate. Condition 1 alone rewards a
tight CI on a trivial difference; condition 3 alone ignores how noisy the difference is.

**Null:** the CI includes 0, **or** the relative shift is under 5 %.

**Between:** CI excludes 0 but `|d_z| < 0.8` or the relative shift is under 5 % ⇒ reported as
*detectable but below the preregistered materiality threshold*, which counts as **not different
enough**. It is not upgraded on appeal.

### 6.3 Reporting rules

- All eleven observables get the full table. The primary and secondary designation was made here,
  before the numbers existed, and cannot move afterwards.
- **No metric switching.** If H1 is null and some other observable is material, that is reported as
  what it is — an unregistered observation, hypothesis-generating, not a finding.
- **A negative result is a result**, and so is a null one. "Turning per-agent brains on made the
  population measurably worse in this harness" is a publishable, useful answer and is what §5 of
  the requirements doc predicts.
- Failed runs are listed. Warnings from `RunResult.warnings` are listed.
- Every number in `summary.md` traces to a file in the artifact directory.

## 7. Ordered work packages for E2-B

| # | package | done when |
|---|---|---|
| 1 | Open the seam to the design §3 specification (P1, P2), flip `P1_SEAM_OPEN`, add the brain-presence test | `cargo test --features desktop` green; a treatment manifest builds founders with `AgentBrain` and a control builds none |
| 2 | Add `examples/run_e2_brain_experiment.rs` — load the four manifests, run the pair, write the artifact set of design §7 | the reproduction command runs end to end on the smoke manifests |
| 3 | **Smoke calibration.** Seed 999983 only, both arms, at T. Time it, project ×12, pick the ladder rung, record it | `smoke/` written; the rung recorded in `provenance.json`; **nothing merged into the analysis** |
| 4 | Run the twelve-seed paired ensemble, once | `paired-report.json` written with 12 pairs |
| 5 | Analysis and `summary.md` per §6 | every observable tabled; failures listed; the §8 decision stated |
| 6 | Record the outcome in ADR-0003 per §8, and update `STATE_OF_THE_PROJECT.md` §3.1 | ADR carries a dated decision item; the state doc's §3.0 row 2 closes |

Packages 1–2 are code and may be committed freely. Package 3 is the first run of anything.

## 8. The decision rule for ADR-0003

Written before the data, so the data cannot write it.

| outcome | what ADR-0003 records |
|---|---|
| H1 material **and positive** (treatment better) | `evolved: true` becomes the default, as a dated decision item, **provided** no harness-integrity finding is open, `cargo test --features desktop` is green with the new default, and EB-S03 still measures `allocs == 0` on the tick path |
| H1 material **and negative** | the default **stays opt-in**, recorded as a decision with the measured cost. This is the predicted outcome and it is a real finding: per-agent brains cost the population something when nothing selects them |
| H1 **null** | the default **stays opt-in**, recorded as "no material effect measured in the headless adapter at N = 12, T = 18,000" |
| fewer than 10 complete pairs, or a precondition satisfied differently from design §3 | **insufficient evidence**. Nothing about the default is recorded, and §8.1 names what is missing |

### 8.1 What "insufficient evidence" must name

Not a shrug — a specific next experiment. The standing one, known now:

> **A run in which evolutionary replacement actually operates.** The adapter drains epoch statistics
> and never applies a replacement, so E2 measures brains that are never selected. ADR-0003's central
> claim is that behaviour must be heritable *and under selection* for quality-diversity to mean
> anything, and that claim is untestable in this harness. Reaching it needs the evolution thread, or
> a headless stand-in for it, inside the experiment contract — which is the MAP-Elites
> archive-coverage work ADR-0003 already lists as not covered.

### 8.2 A defect is never insufficiency

If E2 surfaces a real defect in the evolved-brain path — a gate wired backwards, a brain that does
not reach inference, energy that leaks — it is **recorded as a defect and fixed**. It may not be
reported as "insufficient evidence", and the fix does not silently become part of the experiment:
the ensemble is re-run from the top afterwards, under this same preregistration.

## 9. Language this experiment is held to

The live world's status is **headless adapter verified**. Not "live world experiment-ready", not
"the app is deterministic", not "the simulation is validated". No E2 artifact, summary or commit
message may say otherwise. What stays unclaimed is unchanged from CLAUDE.md: no full desktop-app run
under the multi-threaded executor, the adapter refuses `laws.exotic_energy`, it has no AE3 reference
population, and numerical agreement with `ReferenceEvolutionWorld` is never claimed — only agreement
on the direction and meaning of a shared law.
