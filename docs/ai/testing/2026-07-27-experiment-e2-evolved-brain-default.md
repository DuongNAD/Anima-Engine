---
phase: testing
feature: experiment-e2-evolved-brain-default
title: Testing & Evidence — E2 preregistration, and the gates E2-B must pass
description: What E2-A verified with commands and exit codes, what it deliberately did not run, and the gate list for the run session
status: active
owner: maintainers
last_reviewed: 2026-07-27
requirements: ../requirements/2026-07-27-experiment-e2-evolved-brain-default.md
planning: ../planning/2026-07-27-experiment-e2-evolved-brain-default.md
---

# Testing & Evidence — E2 preregistration, and the gates E2-B must pass

> 🔒 **Preregistration.** Every measurement below is a check on the *preregistration artifacts*.
> None is a measurement of the experiment. Parent commit `96d54d9`.

Machine: Intel Core i5-14600KF, Windows 11 Pro 26200. Worktree
`.worktrees/feature-anima-completion`, branch `feature-anima-completion`. All `cargo` runs from
PowerShell, never Git Bash — Git Bash's `PATH` makes feature-gated targets die at
`STATUS_ENTRYPOINT_NOT_FOUND` before running a test.

## 1. What E2-A measured

### 1.1 The EB-S04 re-baseline (commit `96d54d9`)

```
cargo test --features desktop --test brain_controlled_comparison_tests
```

`test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`

The two tests the new baseline rests on:

| statement of the re-baselined gate | test | result |
|---|---|---|
| (a) the same seed gives the same trajectory across two runs of the seeded build, `evolved = false` | `the_run_is_reproducible_under_one_seed` | ✅ |
| (b) installing `ActionGates` and leaving them open matches the pre-step-4 component layout | `installing_the_gates_changed_nothing_with_them_open` | ✅ |

Both compare `to_bits()` on position, energy, the four CPG parameters and the three gates. Nine
further tests in the same target — including `with_brains_off_no_agent_ever_gains_one` and
`with_brains_off_no_gate_ever_closes` — hold the legacy path to being legacy.

### 1.2 The preregistration artifacts

```
cargo test --features desktop --test prereg_e2_manifest_tests
```

`test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`

| check | test |
|---|---|
| All four manifests parse as `ExperimentManifest` and pass `validate` against `ObservableRegistry::live_default()` | `every_preregistered_manifest_parses_and_validates_against_the_live_registry` |
| The declared factor is exactly one initial-condition key, nothing is removed, every shared key holds an identical value | `the_declared_factor_is_exactly_one_initial_condition_key` |
| The arms share seeds *in order*, duration, sampling, world identity, laws and observables — and their fingerprints still differ, so a run records which arm it was | `the_two_arms_share_everything_the_pairing_depends_on` |
| Twelve unique seeds, in execution order, matching the stated rule | `twelve_experimental_seeds_are_declared_in_execution_order_and_are_unique` |
| The smoke seed is in neither experimental manifest, and is the only seed in the two smoke manifests | `the_smoke_seed_cannot_enter_the_experimental_ensemble` |
| Requested metrics are exactly `LIVE_OBSERVABLE_IDS`, and every hypothesis names a registry observable rather than a provenance field | `the_requested_metrics_are_exactly_what_the_live_adapter_emits` |
| 30 samples per run; the ensemble fits `MAX_ENSEMBLE_RESULT_BYTES` | `the_ensemble_fits_the_declared_memory_ceiling` |
| The plan JSON and the manifests cannot drift: seeds, T, sampling, the ladder's top rung, the total tick budget and the factor key all cross-check | `the_preregistration_document_agrees_with_the_manifests_it_describes` |
| The control arm is runnable today; the treatment arm is not, and `P1_SEAM_OPEN` records which | `the_control_arm_is_runnable_today_and_the_treatment_arm_is_not` |

These are **schema checks**. The target never constructs a `LiveExperimentAdapter`, never steps a
tick, and never touches the experiment runner — parsing a manifest is not running one, and keeping
those apart is why the preregistration is a separate commit from the run.

## 2. What E2-A did **not** run, and why

| not run | reason |
|---|---|
| The E2 ensemble | It is E2-B's work. Running it in the session that wrote the hypothesis would make the preregistration worthless |
| A smoke run, trial seed, pilot or preview | Same reason. The smoke seed exists and has manifests; neither has been executed |
| Any Tauri / Bevy desktop app (`npm run tauri:dev`, `cargo run`) | Forbidden by CLAUDE.md on this machine, and unnecessary |
| The full `cargo test --features desktop` suite | It would schedule `live_experiment_tests`, which drives the live adapter. Those tests are not the E2 ensemble, but the two targets above are the ones this package's claims rest on and running more would not support a claim made here |

## 3. The gates E2-B must pass

| gate | evidence required |
|---|---|
| **E2-G1** seam | A manifest with `live.evolved_brains = 1` builds founders carrying `AgentBrain`; one without builds none. `P1_SEAM_OPEN` flipped in the same commit |
| **E2-G2** legacy untouched | Every existing live manifest — the four in `live_experiment_tests.rs` and the E2 control — behaves exactly as before. `the_same_seed_and_manifest_give_the_same_live_checksum` still green |
| **E2-G3** stream isolation | The ecology draw order is bit-identical in both arms, i.e. founder brains come from a derived stream and not from `SimRng` (design §4.3). Without this the experiment measures a bundle |
| **E2-G4** factor purity | `run_paired_ensemble_with_control` returns `declared_factors == ["initial_conditions"]`, and `prereg_e2_manifest_tests` is still green against unmodified manifests |
| **E2-G5** replay | Re-running one seed of one arm reproduces its `final_checksum` |
| **E2-G6** world identity | The `WorldIdentity` observed in each arm is recorded in `provenance.json` and is identical across arms (design §5.2) |
| **E2-G7** budget | Wall-clock inside 90 minutes, or the ladder rung stepped down from the smoke calibration *before* any experimental seed ran, and recorded |
| **E2-G8** completeness | ≥ 10 complete pairs, every failure listed with its reason, no seed substituted |
| **E2-G9** suite | `cargo test --features desktop` green, and `node scripts/check_test_targets.mjs <captured> --profile desktop` clean |
| **E2-G10** allocation | If the default is flipped, EB-S03 still measures `allocs == 0` on the tick path |

A gate that cannot be met is reported as **blocked** with the reason, never as a pass.

## 4. Statement of record

**No experiment was run in E2-A.** No E2 ensemble, no smoke run, no trial seed, no pilot, no
preview. No desktop app or full backend was started. No result, effect size, direction or
conclusion about per-agent evolved brains exists anywhere in this package — the treatment arm does
not yet exist in the code (design §3), which is itself the strongest evidence that nothing was
measured.

The live world's status is unchanged and is stated the only way it may be stated: **headless adapter
verified**.

## 5. Findings raised while writing this package

Recorded here rather than fixed, because fixing them inside a preregistration commit would change
the thing being registered.

| id | finding | severity | status |
|---|---|---|---|
| **E2-F1** | `RunProvenance::derive` hard-codes `model_version = "reference-evolution-world/1"` for every run, so a live run's provenance names the wrong model. `LIVE_MODEL_VERSION = "live-bevy-world/1"` exists, documents exactly why it must be distinct, and is referenced nowhere in `src/` or `tests/` | Medium | Open — E2-B records the true version in `provenance.json` and files the fix separately |
| **E2-F2** | `LiveExperimentAdapter` never checks the manifest's declared `world_identity` against the world `init_world` builds. The field enters the fingerprint and is otherwise inert, so a live manifest cannot pin the world it runs on | Medium | Open — mitigated for E2 by gate **E2-G6** |
| **E2-F3** | The adapter runs no evolutionary replacement: `check_epoch_completion_system` feeds a channel the adapter drains and `apply_staggered_evolution_system` finds an empty queue. Brains are created at genesis and never selected | Informational — a scope limit, not a defect | Recorded in requirements §6 and in the decision rule so no E2 result is read as evidence about selection |
| **E2-P1** | `build_live_world` hard-codes `BrainPolicy::default()`, so no manifest can request `evolved = true` | Blocking | Specified in design §3 for E2-B |
| **E2-P2** | `live_experiment::genesis` never inserts an `AgentBrain`, unlike the app genesis in `simulation_loop` | Blocking | Specified in design §3 for E2-B |
