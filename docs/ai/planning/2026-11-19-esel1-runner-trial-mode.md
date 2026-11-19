---
phase: planning
feature: experiment-e-sel1-food-scarcity
title: E-SEL1 runner trial mode — one seed and one parameter set per grid work unit
description: run_esel1_experiment gains --trial with --seed, --founders, --food-cap-abundant, --food-cap-scarce and --duration-ticks, built-in smoke manifests and a deterministic verify_checksums.txt, so one OxideSwarm task runs one trial of the pilot sweep and a replica can compare it byte for byte
status: completed
owner: simulation-architecture
last_reviewed: 2026-11-19
---

# E-SEL1 runner trial mode

**What it is for.** The OxideSwarm pilot campaign (OxideSwarm design 0094, decision Q94) runs an
Anima Engine parameter sweep as about 200 work units, 4 founder counts x 5 food-cap pairs x 10
seeds, and verifies each unit by running it again on a second machine of the same OS and CPU
architecture and comparing one file. Before this change `run_esel1_experiment` could not be that
unit: `--smoke` runs only the smoke seed at 18,000 ticks and has no parameter flags, every mode needs
a staged `esel1-preregistration.json`, and the smoke's `checksums.sha256` hashes files that carry
wall-clock time, so two identical runs never match. The owner gave this change to the session that
wrote it in chat on 2026-11-19 ("Bạn tự làm có toàn quyền tôi cấp cho làm đi").

**What changed.** One file of code, `src-tauri/examples/run_esel1_experiment.rs`, and one new test
file, `src-tauri/tests/esel1_trial_mode_tests.rs`. Nothing under `src-tauri/src/` changed, no fixture
changed, and `--smoke`, `--ensemble` and `--replay` behave as before (their loop over runs is now a
shared function, with the same output).

## One work unit

```text
run_esel1_experiment --trial --out out --seed <SEED> --founders <N0> \
  --food-cap-abundant <FA> --food-cap-scarce <FS> --duration-ticks <T>
```

| Item | Value |
|---|---|
| Mode flag | `--trial` (required: the runner never infers a mode) |
| Seed | `--seed <SEED>`; `ANIMA_SIM_SEED` is used when `--seed` is absent; if both are set they must agree, else exit 2 |
| Parameters | `--founders`, `--food-cap-abundant`, `--food-cap-scarce`, `--duration-ticks`; each one left out keeps the smoke manifest's value (30, 50, 25, 18000) |
| Duration | any positive tick count (the sweep's 450,000 works); the preregistered ladder binds only `--smoke` and `--ensemble` |
| Arms | `control` at FA, `treatment` at FS, `drift` at FS with `live.mutation_rate = 0`, as in the smoke |
| Files to stage | **none** (`ANIMA_WORLD_ARTIFACT` ignored): without `--manifest-dir` the trial uses the preregistration and three smoke manifests compiled into the binary from `src-tauri/tests/fixtures/experiments_esel1/`, and `LiveExperimentAdapter` builds terrain from `MapSettings::default()` via `TerrainSource::Generated` regardless of host env vars or temp artifacts. A `--manifest-dir`, if given, must hold `esel1-preregistration.json` and the smoke manifests it names |
| Verification file | `<out>/trial/verify_checksums.txt`: three sorted LF lines `seed,arm,final_checksum` (`control`, `drift`, `treatment`) |
| Exit code | 0 on success, 2 on any refusal or error (as every mode) |

A trial also writes `runs.csv`, `trial-series.csv` and `provenance.json` (no wall-clock value in any
of them), `checksums.sha256` over those three and `verify_checksums.txt`, and `timing.json`, the only file with wall-clock
time and the only one `checksums.sha256` leaves out.

**Refused before a tick runs:** a seed of the preregistered execution order (the twelve seeds
760001, 761001, … 771001: a sweep may not spend a confirmation seed), equal food caps, a value the
live adapter rejects (founders above 4096, for example), a trial flag given twice or with a non-integer value, `--duration-rung`
with `--trial`, and any trial flag with `--smoke`, `--ensemble` or `--replay`.

## What is and is not claimed

- **Deterministic per binary, OS and architecture (independent of host env and app terrain artifacts).** The live adapter installs
  `DeterministicMode::on()`, a CPU brain (`BrainModel::new_seeded_cpu`) and `TerrainSource::Generated` (`MapSettings::default()` — ignoring `ANIMA_WORLD_ARTIFACT`, `ANIMA_CACHE_DIR` and `<temp_dir>/anima_world_artifact.anmw`) whatever the environment
  says, and `provenance.json` records both `"terrain_source": "generated"` and the observed `"world_identity"`. So by the code `ANIMA_DETERMINISTIC` and `ANIMA_USE_GPU` are recorded in `provenance.json` but
  do not change the run, and running the desktop app on a host no longer alters headless trial outputs. Every run measured here set `ANIMA_DETERMINISTIC=1` and `ANIMA_USE_GPU=0`;
  no run with other values was made, and none on a GPU. `final_checksum`
  differs between operating systems and architectures (`macos/aarch64` against `windows/x86_64` on the
  smoke record) because platform `libm` differs in terrain generation and tick dynamics, so replicas must run on the same OS and architecture.
- **Body mass stays blind.** A trial requests and writes only the smoke's allow-listed observables
  (`live.agent_count`, `live.standing_crop`, `live.closed_eu_total`, `live.births`, `live.deaths`)
  and refuses to write text naming a `live.trait_mass_*` observable. Body mass is the outcome of the
  island-rule question (QD-ISLAND); a sweep over food scarcity that looked at it would be a forking
  path for that preregistration. Writing body mass from a trial would be a new decision.
- **Exploratory, not an experiment.** A trial is not part of the E-SEL1 preregistration, whose
  registration closed at the smoke (amendment 2). That smoke also found `live.food_cap` a weak
  scarcity factor in this world (standing crop moved at most 6 percent between caps 50 and 6), so a
  sweep over food caps measures the grid more than it answers a biological question.

## Measured on this Mac

Release build of commit `3dc813a` (`cargo build --release --features desktop --example
run_esel1_experiment`, 13 min 32 s from a cold target directory), binary sha256
`7c3c290441b9f9f19d26d43a742ded74967872d6c6718e2a901b75425170a3e7`, on
an Apple M5 under macOS 27.2.1 (`macos/aarch64`). The machine was shared with other agents' jobs, so
each row gives the 1-minute load average around it. Every run set `ANIMA_DETERMINISTIC=1` and
`ANIMA_USE_GPU=0`; seed 100001 throughout. Times are wall clock from `timing.json` and
`/usr/bin/time -p`; the simulation runs on one core.

| Trial | control | treatment | drift | whole trial | load |
|---|---|---|---|---|---|
| N0 30, FA 50, FS 25, 18,000 ticks, run 1 | 4.0 s (224 us/tick) | 4.0 s (224) | 2.6 s (144; extinct at 9,000) | 11.7 s | 3.2 |
| same, run 2 | | | | 10.9 s | 3.1 |
| N0 30, FA 50, FS 25, 450,000 ticks | 141.8 s (315) | 152.7 s (339) | 54.7 s (122; extinct at 9,000) | 349 s | 2.3 to 6.5, this session's test builds running alongside |
| N0 80, FA 50, FS 4, 450,000 ticks | 116.7 s (259) | 42.4 s (94; extinct at 34,800) | 39.2 s (87; extinct at 9,600) | 199 s | 3.7 to 6.5 |

So one 450,000-tick arm whose population survives takes about 2 to 2.5 minutes here, and one
work unit (three arms) 3.3 to 5.8 minutes; an arm that goes extinct early costs less. Two runs of the
18,000-tick trial in separate processes wrote byte-identical `verify_checksums.txt`, `runs.csv`,
`trial-series.csv`, `provenance.json` and `checksums.sha256`; only `timing.json` differed.
The N0 80 unit was run a second time at the full 450,000 ticks (198.7 s, load 3.3 to 3.7), this time
with `ANIMA_SIM_SEED=100001` set beside `--seed 100001`: `verify_checksums.txt`, `runs.csv` and
`trial-series.csv` were byte-identical to the first run, and `provenance.json` differed in exactly
the two lines that record the seed source (`ANIMA_SIM_SEED`: `null` against `"100001"`, and `--seed`
against `--seed (ANIMA_SIM_SEED agrees)`), hence `checksums.sha256` too.

`provenance.json` records the environment and how the seed was given (`--seed`, `ANIMA_SIM_SEED`,
or both), so it is byte-identical only between replicas started the same way, which a campaign
driver does. `verify_checksums.txt` does not depend on either.

## Tests

`src-tauri/tests/esel1_trial_mode_tests.rs`, in process through `run_with_env` (which takes
`ANIMA_SIM_SEED` as an argument, so no test mutates the shared environment): two trials with the same
seed and parameters write byte-identical `verify_checksums.txt`, `runs.csv`, `trial-series.csv`,
`provenance.json` and `checksums.sha256`; another seed changes every arm's checksum; `ANIMA_SIM_SEED`
gives the same bytes as `--seed` and a contradicting pair is refused; the flags reach the manifests
and the world, and each food cap changes its own arms and no other (the manipulation check of E-SEL1
amendment 1); a trial at the smoke's seed and parameters ends on the smoke's checksums; the file set
is exact and names no body-mass observable; the refusals above; and the built-in manifests are
byte-identical to the committed fixtures and run the same world as `--manifest-dir` pointing at them.

Run on 2026-11-19 at `3dc813a`, from `src-tauri/`: `cargo test --features desktop --test
esel1_trial_mode_tests --test esel1_prerequisites_tests --test prereg_esel1_manifest_tests` (the CI
profile) passed 11, 12 and 16 tests, the trial file in 11.5 s; the same two runner files under
`--release` passed too. `cargo clippy --release --features desktop --example run_esel1_experiment
--test esel1_trial_mode_tests --test esel1_prerequisites_tests -- -D warnings` and `cargo fmt --check`
were clean. The full `cargo test --features desktop` suite was not run locally; CI runs it.

## Authorization recorded for the lead

Proposed line for `R5-AUTH` in [`AGENT_WORKFLOW.md`](../../how-to/AGENT_WORKFLOW.md) §4, which this
change does not edit (the lead writes that page):

```text
  AUTHORIZATION (owner, 2026-11-19, given in chat: "Bạn tự làm có toàn quyền tôi cấp cho làm đi"):
  for the E-SEL1 runner trial-mode change only, you may write to
  src-tauri/examples/run_esel1_experiment.rs (a --trial mode and the shared run loop; --smoke,
  --ensemble and --replay unchanged), one new test file src-tauri/tests/esel1_trial_mode_tests.rs,
  and this note under docs/ai/planning/. This overrides AGENTS.md's read-only declaration and BLOCK 0
  R5 for exactly these paths and no others.
```
