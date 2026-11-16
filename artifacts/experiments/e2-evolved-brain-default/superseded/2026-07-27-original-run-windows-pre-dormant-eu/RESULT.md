# E2 result — does a per-agent evolved brain change outcomes?

**Run once, on 2026-07-27, under the preregistration frozen at commit `ce761d1`.**
Ensemble commit `9c57184`, binary SHA-256 `3773a50113781a3728120a37c0c9be5ef5ba8eaa7bda3aeaf2eef8f8db8128d3`.

> Model status, stated the only way it may be stated: **headless adapter verified.** No desktop app,
> Tauri handle, window, GPU device, renderer, learner thread or evolution thread existed in the
> process that produced these numbers. Nothing here may be quoted as "the live world is
> experiment-ready".

---

## 1. The question, exactly as registered before the run

> Under the same world identity and the same seed, does a per-agent evolved brain (`evolved = true`)
> change outcomes relative to the default shared `BrainModel`, and in which direction?

**Primary metric (H1):** `live.mean_agent_energy`, final value at tick T, paired per seed,
hypothesised **negative**. Secondaries: `live.agent_count` (H2, non-positive), `live.animals_eu`
(H3, negative), `live.predator_count` (H4, two-sided). Harness integrity: `live.closed_eu_total`
(H5, no decision weight).

**Materiality required all three**, fixed in advance: 95 % CI excludes 0; `|d_z| ≥ 0.8`;
`|paired_mean_delta| ≥ 0.05 × |control_mean|`.

## 2. Audit disclosure — read this before the numbers

Before the preregistration commit `ce761d1`, session E2-A ran the existing EB-S04 unit target
`brain_controlled_comparison_tests` (11/11) as the owner-required DEC-1 re-baseline gate.

**That run was not an E2 experiment and is not evidence for anything below.** It did not use
`ExperimentManifest`, `LiveExperimentAdapter`, the shared experiment runner, any E2 seed, or any
preregistered observable. It re-proved the already-documented seeded-build contract and a checksum
inequality, nothing more. It is disclosed here because an honest audit trail says what was run
before the registration, not only what was run after.

## 3. The headline result

**H1 is NULL. The registered primary effect is exactly zero.**

| | value |
|---|---|
| complete pairs | **12 of 12** (registered minimum 10) |
| `control_mean` | `0.000000` EU |
| `treatment_mean` | `0.000000` EU |
| `paired_mean_delta` | `0.000000` EU |
| median of per-seed deltas | `0.000000` |
| paired SD / between-seed variance | `0.000000` / `0.000000` |
| SE | `0.000000` |
| 95 % CI | `[0.000000, 0.000000]` |
| `d_z` | **undefined** — the paired SD is zero, so `delta / sd` has no value |
| relative shift | undefined — `control_mean` is zero |

**Materiality: 1 of 3 conditions met, so NOT material.** The CI does not exclude 0 (it is a point at
0); `|d_z| ≥ 0.8` is not met because `d_z` is undefined; the relative-shift condition evaluates true
only degenerately, because `0 ≥ 0.05 × 0`. A null on the registered rule.

**Every one of the twelve pairs had a primary metric of exactly `0.0` in both arms.** The delta is
not "small"; it is identically zero on every seed.

## 4. Why the null carries almost no evidential weight about brains

This is the most important sentence in the document: **the registered endpoint has no power.**

At T = 18,000 the entire founding cohort — in *both* arms, on *all twelve* seeds — sits at exactly
zero energy and has done so for thousands of ticks. The primary metric is measured at a floor that
both arms reach long before the endpoint.

Sampled trajectory, seed 700001 (every second sample):

| tick | control mean E | treatment mean E | agents |
|--:|--:|--:|--:|
| 600 | 93.262753 | 85.518138 | 10 |
| 1800 | 74.996746 | 57.001490 | 10 |
| 3000 | 56.089204 | 37.313265 | 10 |
| 4200 | 37.179924 | 20.973913 | 10 |
| 5400 | 18.900540 | 10.520307 | 10 |
| 6600 | 5.083083 | 5.284621 | 10 |
| 7800 | **0.000000** | 0.669415 | 10 |
| 9000 → 18000 | **0.000000** | **0.000000** | 10 |

Tick at which each arm first reads zero, all twelve seeds:

| seed | control | treatment | | seed | control | treatment |
|---|--:|--:|---|---|--:|--:|
| 700001 | 7800 | 8400 | | 706001 | 9000 | 8400 |
| 701001 | 6600 | 7800 | | 707001 | 8400 | 9000 |
| 702001 | 2400 | 8400 | | 708001 | 3000 | 9000 |
| 703001 | 3600 | 8400 | | 709001 | 1800 | 9000 |
| 704001 | 6000 | 9000 | | 710001 | 9000 | 9000 |
| 705001 | 3000 | 9000 | | 711001 | 2400 | 8400 |

The last informative sample is around tick 9,000. The registered endpoint is **9,000 ticks past the
point where the measurement stopped being able to distinguish anything.**

### The mechanism, traced in the code rather than guessed

Two facts compose:

1. **The engine has no starvation death.** `update_agent_evaluation_system` reacts to
   `homeo.energy <= 0.0` with `continue` — it stops accruing fitness and does not kill. The only
   callers of `ReclaimAndDespawnAgentCommand` are evolutionary replacement, predation, and the
   `RemovePredators` intervention. An agent at zero energy persists indefinitely. In the *app* this
   is a design choice, not a bug: turnover is meant to come from epoch replacement.
2. **The adapter runs no evolutionary replacement** — this is finding **E2-F3**, recorded in the
   preregistration before the run. `check_epoch_completion_system` feeds a channel the adapter
   drains; `apply_staggered_evolution_system` finds an empty queue.

Together: nothing kills a starved agent and nothing replaces it, so `live.agent_count` is **10 in
every arm of every seed at every sample**, and the population freezes as ten inert bodies holding
zero EU. E2-F3 was registered as a scope limit on *selection*. This run measures its second and
unregistered consequence: it also destroys any long-horizon energy metric.

**This is not a defect in the evolved-brain path.** Both arms do it identically, and the cause is
present with `evolved = false`. It is a property of the harness, and it is the reason T = 18,000 was
the wrong endpoint — a fact that could only have been known by running it.

## 5. Every observable, as registered

All eleven are reported with the same statistics. Roles were fixed before the numbers existed.

| observable | role | n | control mean | treatment mean | delta | median | SD | SE | 95 % CI | d_z | rel. | material |
|---|---|--:|--:|--:|--:|--:|--:|--:|:--:|--:|--:|:--:|
| `live.mean_agent_energy` | **primary** | 12 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | [0, 0] | n/a | n/a | **no** |
| `live.agent_count` | secondary | 12 | 10.000000 | 10.000000 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | [0, 0] | n/a | +0.00 % | no |
| `live.animals_eu` | secondary | 12 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | [0, 0] | n/a | n/a | no |
| `live.predator_count` | secondary | 12 | 3.000000 | 3.000000 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | [0, 0] | n/a | +0.00 % | no |
| `live.closed_eu_total` | integrity | 12 | 154587.249864 | 154587.249864 | −2.559e−9 | 8.789e−9 | 3.900e−8 | 1.126e−8 | [−2.462e−8, 1.951e−8] | −0.0656 | −0.00 % | no |
| `detritus` | reported | 12 | 1020.354665 | 1020.859713 | +0.505048 | +1.163787 | 4.557490 | 1.315634 | [−2.073595, 3.083690] | 0.1108 | +0.05 % | no |
| `plants` | reported | 12 | 153566.895198 | 153566.390151 | −0.505048 | −1.163787 | 4.557490 | 1.315634 | [−3.083690, 2.073595] | −0.1108 | −0.00 % | no |
| `live.standing_crop` | reported | 12 | 153566.895198 | 153566.390151 | −0.505048 | −1.163787 | 4.557490 | 1.315634 | [−3.083690, 2.073595] | −0.1108 | −0.00 % | no |
| `live.herbivore_count` | reported | 12 | 7.000000 | 7.000000 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | [0, 0] | n/a | +0.00 % | no |
| `live.food_items` | reported | 12 | 50.000000 | 50.000000 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | [0, 0] | n/a | +0.00 % | no |
| `live.season_phase` | reported | 12 | 6.282562 | 6.282562 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | [0, 0] | n/a | +0.00 % | no |

**Nothing is material. H2, H3 and H4 are all exactly zero** — like H1, at the floor.

**H5, harness integrity, is clean and is the one number that carries real information.** The paired
difference in `live.closed_eu_total` is **−2.6 × 10⁻⁹ EU on a stock of 1.5 × 10⁵ EU**, a relative
difference of ~1.7 × 10⁻¹⁴ — floating-point noise, not a leak. Per-agent brains do not move energy
the ledger fails to account for. The closed-EU contract holds under the treatment.

The only observables with non-zero variance are `plants`/`standing_crop` and its mirror `detritus`:
the treatment left **0.505 EU more in detritus and 0.505 EU less in standing crop**, exactly
mirrored, conserved to 1e−9. That is the entire measurable footprint of the treatment at T, it is
immaterial by every registered threshold (`d_z` = 0.11, relative shift 0.05 %), and it is *reported*
rather than promoted.

## 6. Unregistered observations — hypothesis-generating, NOT findings

The preregistration forbids metric switching: "If H1 is null and some other observable is material,
that is reported as what it is — an unregistered observation, hypothesis-generating, not a finding."
Two are recorded here under that rule, and **neither may be cited as an E2 result.**

**(a) The treatment reaches the floor later than the control in 10 of 12 seeds.** Mean first-zero
tick: control **5,250**, treatment **8,650**. Treatment is strictly later in 10 seeds, equal in one
(710001), earlier in one (706001). Time-to-floor is not a registry observable, is measured only at
the 600-tick sampling resolution, and was never registered — but it points *opposite* to H1's
predicted direction, and it is the single most interesting thing this run produced.

**(b) The tick-600 energy delta does not support H1 either.** Mean +7.32 EU, SD 19.12, `d_z` 0.383,
**negative in only 6 of 12 seeds**. So the early-trajectory picture from seed 700001 alone — where
the treatment ran clearly lower — does not generalise. Anyone tempted to rescue a directional claim
from the trajectory should look at this: it is a coin flip.

Together (a) and (b) say the honest thing: **this experiment did not measure the direction of the
brain effect, in either direction.**

## 7. Defects found by this experiment, and what happened to them

Two were found by the **smoke calibration**, before any experimental seed ran, and both were fixed
and pushed before the duration lock. Neither is an evolved-brain defect; both would have corrupted
the result.

| id | defect | fixed in |
|---|---|---|
| — | `ecosystem_census_system` snapshots `pool.animals` but declared no order against the systems that move agent reserves, so it reported either this tick's metabolism or last tick's — 0.186 EU apart, chosen per process. Reached `live.animals_eu` (H3) and `live.closed_eu_total` (H5). | `993a587` |
| — | Seven unordered conflicting pairs among the systems that move EU, so the world checksum **and `live.mean_agent_energy`** moved with a per-process hash seed. The gate meant to catch this was itself vacuous: `ScheduleGraph::systems` is empty after initialization, so the name map was empty and every comparison silently failed to match. | `ec94933` |

A third, unrelated, was found by the full suite and fixed in `5f0383f`: a lifecycle test asserted
engine readiness after a blind 500 ms sleep, with only ~120–220 ms of real headroom.

**New finding from this run (not fixed):** the floor described in §4 — no starvation death plus no
replacement in the adapter ⇒ any long-horizon energy metric saturates. Recorded, not fixed: fixing
it means changing either the engine's death model or the adapter's evolution handling, both of which
would invalidate a preregistration written against current behaviour. It needs its own experiment.

## 8. Reproduction — never `cargo run`

```
cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop \
  --example run_e2_brain_experiment

src-tauri/target/release/examples/run_e2_brain_experiment.exe --ensemble \
  --manifest-dir src-tauri/tests/fixtures/experiments_e2 \
  --out artifacts/experiments/e2-evolved-brain-default
```

`cargo run` is forbidden on this machine by a standing owner rule that bars launching the app or the
full backend by any route. The preregistration's original text used it; the correction is recorded
verbatim in `e2-preregistration.json` under `reproduction_command_correction` and pinned by
`e2_seam_tests::the_preregistered_reproduction_never_launches_the_app`. No scientific parameter moved.

Verify everything with `npm run check:e2`, which recomputes every digest with `node:crypto` — a
different implementation from the Rust `core::sha256` that wrote them.

## 9. Provenance and checksums

| item | value |
|---|---|
| ensemble commit | `9c57184d2927ecedd1fa6721e113d59984cf38fb` |
| binary SHA-256 | `3773a50113781a3728120a37c0c9be5ef5ba8eaa7bda3aeaf2eef8f8db8128d3` |
| toolchain / profile | `rustc 1.95.0 (59807616e 2026-04-14)` / `release`, `--features desktop` |
| host | Windows 11 Pro 26200, Intel Core i5-14600KF |
| seeds, in execution order | 700001, 701001, … 711001 (`seed_k = 700001 + 1000k`) |
| smoke seed (excluded) | 999983 — absent from both experimental manifests |
| T / sample period | 18,000 / 600 (top rung, no step-down) |
| ensemble wall clock | **157.2 s (2.62 min)** against a 90-minute ceiling |
| control manifest SHA-256 | `b5c6820713f46389e0eea8b851c1e47e5033687338bcc7a3fa2508159c879426` |
| treatment manifest SHA-256 | `b5618a58f3c089ba4ebe302fcf6da3d0325a3deedc0ec89eeb66a41631e2c27e` |
| preregistration SHA-256 | `0dfdf6f8989ffb843ad8245e3d6a3b634f46151d8552d672463e3e1fce3e9c19` |
| observed world identity (both arms) | seed 1337, gen 1, 256×256, checksum 3152323152 |
| failures / warnings | **none / none** |

### Gates

| gate | result |
|---|---|
| **E2-G1** seam | control 0 brains / 10 founders, treatment 10 / 10 |
| **E2-G2** legacy untouched | `live_experiment_tests` 17/17 and the whole suite green |
| **E2-G3** stream isolation | ecology stream bit-identical across arms after genesis |
| **E2-G4** factor purity | `declared_factors == ["initial_conditions"]` |
| **E2-G5** replay | seed 700001 treatment → `2643270831`, seed 711001 control → `207688652`, both reproduced exactly in fresh processes, all 11 observables identical |
| **E2-G6** world identity | one identity observed across both arms |
| **E2-G7** budget | 2.62 min of 90; rung locked from the smoke before seed 700001 (`9c57184`) |
| **E2-G8** completeness | 12/12 complete pairs, no failures, no seed substituted |
| **E2-G9** suite | `cargo test --features desktop` — 87 targets, 877 passed, 0 failed |
| **E2-G10** allocation | not triggered: the default was not flipped |

Cross-process reproducibility was accepted explicitly before the run: 24 independent processes at
T = 18,000 produced one outcome — identical checksums, all 11 observables, all 30 series points.

## 10. Inference boundary — say this sentence, not a stronger one

> **Twelve seeds under one world identity are twelve stochastic samples of ONE world design, not
> twelve worlds.**

Not permitted, in any wording: "in general", "across worlds", "in Anima"; any claim about evolution,
adaptation, selection or speciation (nothing reproduces here); any claim about the desktop app or
the multi-threaded executor; any claim about behavioural diversity, which is EB-S11's bespoke
harness and not a registry observable.

And specific to this result: **"per-agent brains make no difference" is NOT supported.** What is
supported is "at tick 18,000, in a harness where both arms have been at zero energy for thousands of
ticks, no difference is measurable" — which is a statement about the endpoint, not about brains.

## 11. Decision, by the rule written before the data

Registered rule (planning §8): H1 null ⇒ **the default stays opt-in**, recorded as "no material
effect measured in the headless adapter at N = 12, T = 18,000". Twelve complete pairs clears the
minimum of ten, and the seam was built to the design §3 specification, so "insufficient evidence" —
which is reserved for fewer than ten pairs or a precondition satisfied differently — does not apply.

**ADR-0003 records: `evolved` stays opt-in.** Not because per-agent brains were shown to be harmful
or useless — they were shown to be *unmeasured* — but because the registered rule flips the default
only on a material positive effect, and nothing material was measured.

### What the next experiment must have

1. **An endpoint before the floor.** T ≈ 3,000–6,000 on this genesis, or a metric that integrates
   over the informative window rather than sampling its end. Chosen from §4's evidence, and
   preregistered before running.
2. **A population that turns over.** Either starvation death, or the evolution thread inside the
   experiment contract, so `live.agent_count` can move and selection has something to act on. This
   is the standing item the preregistration already named in §8.1, now with a measured reason.
3. **Time-to-floor as a registered observable**, since §6(a) is the strongest unregistered signal
   here and deserves to be tested rather than admired.
