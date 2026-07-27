---
phase: implementation
feature: experiment-e2-evolved-brain-default
title: Implementation — the E2 seam and the headless runner
description: What E2-B built to make the treatment arm exist, what it deliberately did not build, and the red/green evidence for each behaviour
status: active
owner: maintainers
last_reviewed: 2026-07-27
requirements: ../requirements/2026-07-27-experiment-e2-evolved-brain-default.md
design: ../design/2026-07-27-experiment-e2-evolved-brain-default.md
planning: ../planning/2026-07-27-experiment-e2-evolved-brain-default.md
testing: ../testing/2026-07-27-experiment-e2-evolved-brain-default.md
decision: ../../decisions/ADR-0003-evolved-per-agent-brains.md
---

# Implementation — the E2 seam and the headless runner

> This document describes **code**, written before any experimental run. It contains no result and no
> observed direction. The preregistration it implements is frozen at commit `ce761d1`; nothing in
> §1–§3 below chose a parameter that package had not already fixed.

## 1. The seam, clause by clause against design §3

The design specified this seam in advance precisely so it could not be tuned after seeing data. Each
clause, and where it landed:

| design §3 clause | implementation | test |
|---|---|---|
| 1. `LIVE_KEY_EVOLVED_BRAINS`, in `LIVE_KEYS` and `LiveWorldConfig`; only `0.0`/`1.0`; absent ⇒ `false` | `core/live_experiment.rs` — new const, `LIVE_KEYS: [&str; 6]`, `LiveWorldConfig.evolved_brains: bool` (`#[serde(default)]`), exact match in `from_initial_conditions` else `ExperimentError::OutOfRange` | `the_live_world_honours_exactly_one_new_initial_condition_key`, `absent_means_false_so_every_existing_live_manifest_keeps_its_behaviour`, `one_means_true_and_nothing_else_is_accepted` |
| 2. `build_live_world` inserts `BrainPolicy { evolved: config.evolved_brains, ..Default::default() }` | same file; `lifetime_learning` and `brain_metabolic_cost` stay at their defaults, so the factor is one flag and not a bundle | covered by the two arm-construction tests below |
| 3. `genesis` builds a brain per founder when the policy is on, from a **derived** stream | `genesis` now takes the run seed and draws from `derived_rng(seed, sim_stream::LIVE_GENESIS_BRAINS)`; the policy is read from the resource, so "policy says evolved" and "founders have brains" cannot disagree | `the_treatment_builds_founders_with_brains_and_the_control_builds_none`, `founder_brains_leave_the_ecology_stream_exactly_where_the_control_left_it` |
| 4. flip `P1_SEAM_OPEN` in the same commit | `tests/prereg_e2_manifest_tests.rs` — `true` | `the_seam_state_is_exactly_what_the_preregistration_recorded` |
| 5. a brain-presence test | `tests/e2_seam_tests.rs`, plus the same check against the **committed** manifests | `the_committed_e2_manifests_build_the_arms_they_declare` |

`sim_stream::LIVE_GENESIS_BRAINS = 3` is new. It is the only addition outside the clauses above, and
it exists so a future author cannot collide with the brain stream by accident.

### Why the derived stream is the whole design and not a detail

`BrainPolicy::new_brain` draws from whatever RNG it is handed, and the app's genesis hands it
`SimRng`. Had the adapter done the same, the treatment would have pulled roughly
`10 founders × 5,769 f32` out of the ecology stream **before the first tick**, displacing every later
draw in the run — every food position, every seed drop. The arms would then have differed in the
brain *and* in the realised random sequence, inseparably, at every seed, and no amount of statistics
afterwards could have separated them.

`founder_brains_leave_the_ecology_stream_exactly_where_the_control_left_it` measures this rather than
asserting it: it compares the seed, the ChaCha word position and the next eight draws of `SimRng`
across arms. All three match.

**The cost, stated because it is real:** this is not exactly what flipping the app's default would
do, since the app's genesis would shift its own stream. That shift carries no directional information
— it is a reshuffle, not a treatment — but it does mean E2 measures the brain effect with the
stochastic trajectory held fixed, which is the cleaner question and not quite the same one. The
design said this before the seam existed (§4.3); the implementation did not choose it.

## 2. The runner — `examples/run_e2_brain_experiment.rs`

Precondition P3 was "no runner binary exists". There is one now. It loads the committed manifests
rather than reconstructing them, drives
`experiment_runner::run_paired_ensemble_with_control::<LiveExperimentAdapter>`, and writes the design
§7 artifact set.

**It builds no Tauri handle, no window, no GPU device, no renderer, no learner thread, no evolution
thread and no websocket server.** `LiveExperimentAdapter` is the whole surface. The status of the
live world is unchanged and is stated the only way it may be stated: **headless adapter verified**.

Three refusals are mechanical rather than promised, and each was exercised:

- **Modes are exclusive.** `--smoke`, `--ensemble` and `--replay` cannot be combined; an ambiguous
  mode is how a calibration run ends up filed as a result.
- **Seeds must be the registered ones.** `--ensemble` refuses a manifest whose seed list is not
  *exactly* the preregistered execution order, and refuses the smoke seed outright; `--smoke` refuses
  anything but `[999983]`. This sits in front of `run_manifest_seed`'s own
  `SeedNotInManifest`, so a wrong seed set never reaches a model.
- **The duration rung must be on the ladder.** `--duration-rung 9999` is refused against the
  registered `[18000, 12000, 6000]`.

Smoke output goes to `<out>/smoke/` and replay output to `<out>/replay/`, by construction rather than
by convention, so calibration cannot be read as evidence.

### Artifacts

`paired-report.json`, `effects.json`, `per-seed-deltas.csv`, `runs.csv`, `provenance.json`,
`summary.md`, a byte-exact `manifests/` copy of everything the run read, and `checksums.sha256` over
all of it. `core::sha256` is new: a hand-written FIPS 180-4 implementation, checked against the
published NIST vectors, added rather than depended on because a dependency for it would move
`NOTICE`, the SBOM and the audit surface for a hash this repository already computes in Node.

`scripts/verify_e2_artifacts.mjs` (`npm run check:e2`) is the independent check. It recomputes every
digest with `node:crypto` — a different implementation, in a different language, on the same bytes —
so the two implementations check each other rather than one checking itself. It also verifies what a
checksum cannot: that the copied manifests are byte-identical to the committed ones, that the seed
order run is the registered one, that the smoke seed is absent from the analysis, that the profile
was `release`, that gates E2-G1/G3/G4/G6 are recorded green, and that every per-seed delta really is
`treatment_final − control_final`.

`artifacts/experiments/**` is pinned `-text` in `.gitattributes`. `core.autocrlf=true` is set on this
machine and on `windows-latest`; without the pin a checkout would rewrite an LF inside a hashed
result and fire the verifier as a tampering alarm on an untouched file.

## 3. Two findings recorded, not fixed — and now with evidence

Both were predicted by reading the code during preregistration. Both are now **measured**, from the
dry run's `provenance.json`:

- **E2-F1** — `RunProvenance::derive` hard-codes `model_version`. Every live `RunResult` records
  `reference-evolution-world/1`; the model that ran is `live-bevy-world/1`. `provenance.json` carries
  both, side by side, under a note naming the finding. The fix is filed separately; changing
  `RunProvenance` would alter the bytes of every `RunResult` and is not part of this experiment.
- **E2-F2** — the manifest's declared `world_identity` is inert on this path. Both manifests declare
  all-zero; the world actually built reports `seed 1337, generator_version 1, 256×256,
  checksum 3152323152`. Gate E2-G6 is met by *observing* the identity in each arm and voiding the run
  if they differ, which is the mitigation the design registered.

## 4. Transport correction to the reproduction command

The preregistration's reproduction command was
`cargo run --release --features desktop --example run_e2_brain_experiment -- …`. The owner's standing
rule on this machine forbids launching the app or the full backend by any route, and `cargo run` is
one of those routes — the rule is categorical, not a judgement about what this example does.

Corrected everywhere to build the example and execute the compiled binary directly. The superseded
text is recorded verbatim in `e2-preregistration.json` under `reproduction_command_correction`, and
`the_preregistered_reproduction_never_launches_the_app` pins it so it cannot drift back.

**No scientific parameter moved.** Question, hypotheses, metrics, thresholds, seed list, N, smoke
seed, duration ladder, sample period, failure handling and decision rule are untouched. `git diff
ce761d1` on the preregistration files shows exactly this correction and the mandated `P1_SEAM_OPEN`
flip (plus one test rename, disclosed in the file itself).

## 5. What was deliberately not built

- **No new observable.** Requirements §3.3 rejected adding one, and adding it here would be the same
  choice made later.
- **No change to `RunProvenance`, `PairedEffect` or the runner's statistics.** The registered
  analysis uses what the harness already computes.
- **No evolutionary replacement.** The adapter still drains epoch statistics and applies none. That
  is finding E2-F3, a scope limit rather than a defect, and it is the reason no E2 result may be read
  as evidence about selection.
- **No default flip.** ADR-0003's default is a decision the *result* feeds, under the rule in
  planning §8, and nothing in this commit touches it.

## 6. Evidence for this commit

Machine: Intel Core i5-14600KF, Windows 11 Pro 26200, worktree `.worktrees/feature-anima-completion`.
All `cargo` runs from PowerShell — Git Bash's `PATH` makes feature-gated targets die at
`STATUS_ENTRYPOINT_NOT_FOUND` before running a test.

| check | command | result |
|---|---|---|
| red, before the seam existed | `cargo test --features desktop --test e2_seam_tests` | `E0432: no LIVE_KEY_EVOLVED_BRAINS in core::live_experiment` + 4 × `E0609: no field evolved_brains` — the treatment arm did not exist |
| green, seam | `cargo test --features desktop --test e2_seam_tests` | `9 passed; 0 failed` |
| green, preregistration unchanged | `cargo test --features desktop --test prereg_e2_manifest_tests` | `9 passed; 0 failed` |
| **E2-G2** legacy untouched | `cargo test --features desktop --test live_experiment_tests --test brain_controlled_comparison_tests --test action_gates_tests --test brain_persistence_tests` | `17`, `11`, `13`, `14` passed; `0 failed` |
| SHA-256 against published vectors | `cargo test --features desktop --lib sha256` | `3 passed` (empty, `abc`, the two-block message, the padding boundaries, the million-`a` vector) |
| lints | `cargo clippy --features desktop --all-targets` | exit 0, no warnings |
| runner, end to end | the compiled example on **synthetic seeds 4242/4343** at 60 ticks, outside the repository | 2 complete pairs; integrity `control brains 0 / treatment brains 10 of 10`, ecology stream identical, world identity identical |
| runner refusals | no mode; two modes; `--duration-rung 9999`; `--replay-seed 999983` against the experimental manifest; unknown flag | each refused with a structured reason, exit 2 |

**No experimental seed was run.** The dry run used 4242 and 4343, which belong to no E2 manifest, and
wrote outside the repository. No smoke run has happened yet either — that is the next step, and it is
the first run of anything.

### External validation limitation

`npx ai-devkit@latest lint` cannot run here: the published package's cached install is missing
`telegraf` and the tool fails before it reads the repository. This is an environment limitation of an
external validator, not a finding about this work, and it is recorded rather than worked around. The
repository's own gates — `cargo test`, `cargo clippy`, `cargo fmt`, `scripts/check_text_hygiene.mjs`,
`scripts/check_docs_links.mjs`, `scripts/check_test_targets.mjs` — are used in its place.
