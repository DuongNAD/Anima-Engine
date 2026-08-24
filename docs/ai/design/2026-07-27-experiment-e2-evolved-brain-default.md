---
phase: design
feature: experiment-e2-evolved-brain-default
title: Design — E2 control/treatment, the seam it needs, and what a seed actually means
description: The exact manifest pair, the two blocking preconditions in the live adapter, and the inference boundary of N seeds under one world
status: active
owner: maintainers
last_reviewed: 2026-07-27
requirements: ../requirements/2026-07-27-experiment-e2-evolved-brain-default.md
decision: ../../decisions/ADR-0003-evolved-per-agent-brains.md
---

# Design — E2 control/treatment, the seam it needs, and what a seed actually means

> 🔒 Preregistration. No run, no result, no observed direction. Parent commit `96d54d9`.

## 1. The harness

`core::experiment_runner::run_paired_ensemble_with_control::<LiveExperimentAdapter>`.

Chosen over the alternatives for reasons that are properties of the code, not preferences:

- `run_paired_ensemble` derives its control with `ExperimentManifest::control_variant`, which only
  strips the exotic-energy law. Our factor is not the exotic regime, so the derived control would be
  identical to the treatment and the comparison would be empty.
- `compare_ensembles` is documented as **not** the causal design: it treats two ensembles as
  independent samples and never checks that they used the same seeds in the same order.
- `run_paired_ensemble_with_control` validates both manifests, validates the factor diff against an
  allowlist, **refuses** a pair whose seed lists differ in content or order, preserves one-sided
  failures instead of dropping them, and returns paired per-seed deltas. That is the design the
  question needs.

`genesis_fork` is not used: it runs a single seed, and a single seed cannot answer this question
(§4).

## 2. The manifest pair

Four manifests are committed under `src-tauri/tests/fixtures/experiments_e2/`. They are serialized
`core::experiment::ExperimentManifest` values — the same type, the same schema and the same JSON
shape as the AE-210 fixtures next door — so E2-B deserializes them rather than reconstructing them.

| file | role |
|---|---|
| `e2-control-shared-brain.json` | control, 12 experimental seeds |
| `e2-treatment-evolved-brain.json` | treatment, the same 12 seeds in the same order |
| `e2-smoke-control-shared-brain.json` | calibration only, seed 999983 |
| `e2-smoke-treatment-evolved-brain.json` | calibration only, seed 999983 |
| `e2-preregistration.json` | the analysis plan as data: hypotheses, thresholds, budgets, artifact schema |

They live in a **sibling** directory rather than `tests/fixtures/experiments/`, because
`ae210_reference_manifests_cover_exactly_the_committed_files` asserts that directory holds exactly
the three files `ae210_reference_manifests()` declares. Adding a fourth would fail a gate that is
doing its job.

### 2.1 What is identical, by construction

Everything except one initial-condition key:

```
schema_version    1
world_identity    WorldIdentity::default() — all zero (see §5.2)
laws              WorldLawSet::baseline()  — exotic_energy: null
interventions     []
exotic_interventions []
observer          {"mode": "absent"}
seeds             [700001, 701001, … 711001]   (same values, same order)
duration_ticks    18000
sample_period     600
observable_ids    all eleven of LIVE_OBSERVABLE_IDS, in emission order
initial_conditions
    live.founders           10
    live.predator_fraction   0.3
    live.trees               8
    live.lakes               2
    live.food_cap           50
```

Those five initial conditions are `LiveWorldConfig::default()` — the engine's own genesis, ten
founders at 30 % predators with the stock environment settings — declared explicitly rather than
defaulted, so the manifest describes the world it asks for.

### 2.2 What differs

The treatment adds one key:

```
    live.evolved_brains      1
```

and that is the whole declared factor. `experiment_id` and `name` also differ; both are labels,
excluded from `ExperimentManifest::fingerprint` and from `FactorDiff::diff_paths`.

The allowlist handed to the runner is `FactorDiff { allowed_paths: ["initial_conditions"] }`.

**That allowlist alone is not enough**, and the design says so rather than hoping. `FactorDiff`
works at manifest-path granularity, so it would tolerate a treatment that also moved
`live.founders`. `tests/prereg_e2_manifest_tests.rs::the_declared_factor_is_exactly_one_initial_condition_key`
closes the gap: it asserts the treatment adds exactly `live.evolved_brains`, adds nothing else,
removes nothing, and holds every shared key at an identical value.

## 3. Two blocking preconditions — the treatment arm does not exist yet

Read before anything else in this document. They were found by reading the adapter, not by running
it, and neither is worked around here.

### P1 — no manifest can request `evolved = true`

`core::live_experiment::build_live_world` does this, deliberately:

```rust
world.insert_resource(crate::core::resources::SimRng::from_seed(seed));
world.insert_resource(crate::core::resources::BrainPolicy::default());
```

The comment beside it is correct and the reason is good: `init_world` resolves `BrainPolicy` from
`ANIMA_EVOLVED_BRAINS`, and a run whose trajectory depends on a shell variable is not the run its
manifest describes. But the fix took the environment away without putting a *declared* input in its
place, so `BrainPolicy::default()` — `evolved: false` — is the only policy a live experiment can
have. **There is no treatment arm.**

### P2 — genesis does not build brains even when the policy is on

`core::live_experiment::genesis` calls `decode_genotype` and inserts the evaluation, lineage and
role components. It never inserts an `AgentBrain`. The app's genesis in
`core::simulation_loop` does, gated on `policy.evolved`, with the comment "Genesis creates
individuals, so it develops brains (invariant D01)". The adapter's genesis is missing that block.

So even with P1 fixed, a treatment run would produce a population of brainless agents and report it
as the treatment. That is the failure mode this whole package exists to prevent: a run that
completes, returns finite numbers, and measures the wrong thing.

### P3 — no runner binary

Nothing in the repository runs a paired ensemble and writes artifacts. `examples/gen_ae_fixtures.rs`
is the only example, and it writes fixtures.

### The specification E2-B implements — and may not redesign

Preregistered so the treatment cannot be tuned after seeing data.

1. **`LIVE_KEY_EVOLVED_BRAINS = "live.evolved_brains"`**, added to `LIVE_KEYS` and to
   `LiveWorldConfig` as `evolved_brains: bool`. Accept only `0.0` or `1.0`; anything else is
   `ExperimentError::OutOfRange`. **Absent means `false`**, so every existing live manifest —
   including the control and the four in `live_experiment_tests.rs` — keeps its exact behaviour.
2. **`build_live_world` inserts `BrainPolicy { evolved: config.evolved_brains, ..Default::default() }`.**
   `lifetime_learning` stays off and `brain_metabolic_cost` stays `0.0`: they are separate flags and
   separate experiments, and folding them in would make the factor a bundle.
3. **`genesis` creates a brain per founder when the policy is on**, mirroring `simulation_loop`, and
   **draws from a stream of its own** — not from `SimRng`. See §4.3 for why that is the identifying
   assumption of the whole design and not a detail.
4. **Flip `P1_SEAM_OPEN` to `true`** in `tests/prereg_e2_manifest_tests.rs` in the same commit. The
   constant exists so the state of this precondition is a machine-checked fact rather than a
   sentence somebody has to remember to update.
5. **Add a test** that a manifest with `live.evolved_brains = 1` produces founders carrying
   `AgentBrain`, and one without produces founders carrying none. P2 is precisely the kind of gap a
   green suite would not notice.

Nothing else changes. In particular the manifests, seeds, T, sampling and thresholds are fixed.

## 4. Seed semantics, exactly

This is the section a reader should distrust most, so it is the most specific.

### 4.1 What the seed reaches

The `seed` in `seeds: [...]` is the run seed the runner hands to
`ExperimentModel::from_manifest`. Inside `LiveExperimentAdapter::build` it reaches exactly two
places:

| consumer | effect |
|---|---|
| `SimRng::from_seed(seed)` | the one seeded stream for **every** stochastic decision the live schedule makes: food spawn placement, tree seed dropping, environmental event rolls, migration, and any system taking `ResMut<SimRng>` |
| `BrainModel::new_seeded(15, 64, 4, seed)` | the weights of the **shared** model, drawn from a seeded stream rather than Burn's process-global generator — the fix that made EB-S04 re-baselineable at all |

The runner separately builds `StdRng::seed_from_u64(seed)` and passes it to `step`, and
`LiveExperimentAdapter::step` ignores it (`_rng`). The live world's randomness is `SimRng`.

### 4.2 What the seed does **not** reach

- **The terrain and the resource field.** `init_world` loads the shared world artifact and derives
  `ResourceField::from_biomes`. The run seed does not regenerate the world; every seed in this
  ensemble runs on the *same* map.
- **The founding layout.** `genesis` places founders, trees and lakes on a deterministic lattice
  (`lattice_point`), explicitly not the app's biome-weighted shuffle. Identical across all seeds.
- **The initial energy state.** Set by the terrain and the declared initial conditions.

### 4.3 The RNG-order confound, and the design that removes it

`BrainPolicy::new_brain` draws from whatever RNG it is given. The app's genesis gives it `SimRng`.
If the adapter's genesis did the same, the treatment would draw roughly `10 founders × 5,769 f32`
weights out of the ecology stream **before the first tick**, and every later draw in the run — every
food position, every seed drop — would be displaced. The two arms would then differ in the brain
*and* in the realised random sequence, inseparably, at every seed.

That is why §3 step 3 requires founder brains to come from a **derived stream** rather than
`SimRng`. `core::resources::derived_rng(run_seed, stream)` already exists for exactly this, and
`brain_controlled_comparison_tests.rs` already uses the same trick for the same reason:

> Brains are drawn from a stream of their own so the founding population is identical whether or not
> the rest of the world has consumed draws.

With it, the ecology draw order is **bit-identical** in both arms and the arms differ only in
whether agents carry brains. Without it, the experiment measures a bundle.

**The cost, stated because it is real:** this is not exactly what flipping the app's default would
do, since the app's genesis draws brains from `SimRng` and would shift its own stream. That shift
carries no directional information — it is a reshuffle, not a treatment — but it does mean E2
measures the brain effect with the stochastic trajectory held fixed, which is the cleaner question
and not quite the same question.

### 4.4 The inference boundary — say this sentence, not a stronger one

> **Twelve seeds under one world identity are twelve stochastic samples of ONE world design, not
> twelve worlds.**

Permitted: "on this map, with this founding population, under this schedule, turning per-agent
brains on moved `live.mean_agent_energy` by X (95 % CI …) across twelve seeds."

Not permitted, in any wording:

- "in general", "across worlds", "in Anima" — the map never varied, so between-world variance was
  never sampled and cannot be estimated;
- any claim about **evolution, adaptation, selection or speciation** — nothing reproduces in this
  harness (requirements §6);
- any claim about the desktop app or the multi-threaded executor;
- any claim about **behavioural diversity**, which is EB-S11's bespoke harness and is not a registry
  observable.

Widening to a claim about worlds needs a second axis: several `world_identity` values, each with its
own seed set. That is a different, larger experiment, and it is deliberately not this one.

## 5. Controlling world identity

### 5.1 What "same world identity" means operationally

Both arms call the same `init_world`, from the same checkout, in the same process, against the same
`artifacts/world_256.anmw`. That is what holds the world fixed.

### 5.2 What the manifest field does — and a finding

`world_identity` in both manifests is `WorldIdentity::default()`, all zero, matching the convention
in `live_experiment_tests.rs`. It is **identical in both arms**, so it is a controlled constant and
never a differing factor.

But it is not a *check*. `LiveExperimentAdapter::from_manifest` never compares the declared identity
against the world `init_world` actually built — the field enters the manifest fingerprint and is
otherwise inert on this path. **Finding E2-F2**, recorded not fixed: a live manifest cannot pin the
world it runs on.

The mitigation is preregistered rather than assumed: E2-B reads the `WorldIdentity` resource out of
a built world for each arm and writes both into `provenance.json`, and the run is void if they
differ from each other.

## 6. Two more findings, recorded here and fixed by nobody in this package

**E2-F1 — a live run reports the reference model's version.** `RunProvenance::derive` hard-codes
`model_version: MODEL_VERSION` = `"reference-evolution-world/1"` for every run.
`live_experiment::LIVE_MODEL_VERSION` = `"live-bevy-world/1"` exists, is documented as existing
precisely because "a checksum from the live world and one from the reference world are not
comparable, and provenance is where that has to be visible", and is referenced nowhere in `src/` or
`tests/`. Every live `RunResult` therefore carries provenance naming the wrong model. E2-B records
the true model version in `provenance.json` and files the fix separately; it is not part of the
experiment.

**E2-F3 — the adapter runs no evolution, and that is a scope limit rather than a defect.**
Documented in requirements §6 so no reader of an E2 result mistakes it for evidence about selection.

## 7. Artifact schema E2-B writes

Root `artifacts/experiments/e2-evolved-brain-default/` (`artifacts/` is deliberately not gitignored).

| file | contents |
|---|---|
| `paired-report.json` | `serde_json` of the `PairedEnsembleReport`, verbatim |
| `effects.json` | per-observable table: the ten `PairedEffect` fields plus median of per-seed deltas and between-seed variance |
| `per-seed-deltas.csv` | `seed,observable,control_final,treatment_final,delta` |
| `provenance.json` | build commit, `rustc`/`cargo` versions, host, profile, start/end timestamps, both manifest fingerprints, law fingerprint, registry fingerprint, the observed `WorldIdentity` per arm, the seed list, the smoke seed, the duration rung used, and `LIVE_MODEL_VERSION` (E2-F1) |
| `summary.md` | the human table and the decision §6 of the planning doc produces |
| `smoke/` | the calibration outputs, kept separate and never merged into the analysis |

Reproduction command, preregistered so it is not invented afterwards:

```
cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop \
  --example run_e2_brain_experiment

src-tauri/target/release/examples/run_e2_brain_experiment.exe --ensemble \
  --manifest-dir src-tauri/tests/fixtures/experiments_e2 \
  --out artifacts/experiments/e2-evolved-brain-default
```

`--release` is not a preference; see the planning doc §5.

> **Transport correction, 2026-07-27, before any run.** This block originally read
> `cargo run --release --features desktop --example run_e2_brain_experiment -- …`. The owner's
> standing rule on this machine forbids launching the app or the full backend by any route, and
> `cargo run` is one of those routes — the rule is categorical, not a judgement about what this
> particular example does. Building the example and executing the compiled binary directly runs the
> same program by the same code path.
>
> **No scientific parameter moved.** The question, hypotheses, metrics, thresholds, seed list, N,
> smoke seed, duration ladder, sample period, failure handling and decision rule are untouched. The
> machine-readable form is corrected identically in `e2-preregistration.json`, which also records the
> superseded text verbatim, and
> `tests/e2_seam_tests.rs::the_preregistered_reproduction_never_launches_the_app` pins it so the
> command cannot drift back into one nobody is allowed to type.
