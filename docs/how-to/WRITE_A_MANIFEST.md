---
title: How to Write and Validate an Experiment Manifest
status: active
owner: maintainers
last_reviewed: 2026-11-23
review_cycle: per-release
---

# How to Write and Validate an Experiment Manifest

This guide explains how to author, validate, and test an `ExperimentManifest` (single configuration or paired control/treatment arms), as well as how to wrap one inside a `SweepManifest` or `BranchTreeManifest`.

> **Authoritative schemas:** this guide reflects the structs in [`src-tauri/src/core/experiment/`](../../src-tauri/src/core/experiment) (`manifest.rs`, `laws.rs`, `interventions.rs`, `factor_diff.rs`), [`sweeps.rs`](../../src-tauri/src/core/sweeps.rs), and [`branch_tree.rs`](../../src-tauri/src/core/branch_tree.rs). Scientific invariants (`ER01`–`ER11`) live in [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) *(proposed)*.

---

## 1. Choose the Right Manifest Type

| Goal | Struct / Subcommand | When to Use |
|---|---|---|
| Run 1 config over 1 or more seeds | `ExperimentManifest` (`anima-lab ensemble`) | Single exploratory run, baseline, or one arm of a pair |
| Run a paired control/treatment comparison | Two `ExperimentManifest` files (`control` + `treatment`) | Same ordered `seeds`, differing only in declared factor paths (`FactorDiff`) |
| Sweep multi-dimensional parameter space | `SweepManifest` (`anima-lab sweep`) | `Grid`, `Sobol`, or `LatinHypercube` sampling over `base_manifest` |
| Counterfactual fork at `fork_tick` | `BranchTreeManifest` (`anima-lab branch`) | Share prefix execution and measure post-fork trajectory divergence |

---

## 2. Required Fields of `ExperimentManifest`

`ExperimentManifest` uses `#[serde(deny_unknown_fields)]` and `schema_version: 1` (`MANIFEST_SCHEMA_VERSION`), so unknown keys or wrong types fail JSON parsing immediately.

| Field | Required | Type & Constraints |
|---|---|---|
| `schema_version` | Yes | Integer `1` |
| `experiment_id` | Yes | Non-empty string (excluded from `manifest_fingerprint`) |
| `name` | Yes | Non-empty string (excluded from `manifest_fingerprint`) |
| `world_identity` | Yes | `{ seed, generator_version, width, height, checksum }` |
| `laws` | Yes | `WorldLawSet` (`schema_version: 1`, `baseline_energy`, `exotic_energy`) |
| `initial_conditions` | Yes | `InitialConditionSet` (`schema_version: 1`, `values: [[key, f64], ...]`) |
| `interventions` | Yes | Array of `InterventionCommand` (at most `4096`, unique `id`) |
| `seeds` | Yes | Non-empty `u64` array, at most `1024`, unique values |
| `duration_ticks` | Yes | `1..=100_000_000` |
| `sample_period` | Yes | `u64` (`0` disables time-series sampling while still emitting `final_observables`) |
| `observable_ids` | Yes | Unique string array, at most `4096`; all IDs must exist in the selected `--model`'s `ObservableRegistry` |
| `exotic_interventions` | Optional | Defaults to `[]`; allowed non-empty only when `laws.exotic_energy != null` |
| `observer` | Optional | `ObserverPolicy`, defaults to `{"mode": "absent"}` |

> **Preflight memory bound:** manifests estimated above `MAX_ENSEMBLE_RESULT_BYTES = 2 GiB` (`seeds × samples × observables`) are rejected before execution.

---

## 3. Configure `laws` (`WorldLawSet`)

- **`baseline_energy`:** required `{"eu_unit": "EU"}`.
- **`exotic_energy`:** set `null` for the baseline regime, or specify `ExoticEnergyLaw` (supported only under `--model reference`; `--model live` rejects any manifest with `exotic_energy != null`):

```json
"exotic_energy": {
  "id": "arcane_flux",
  "display_name": "Mana",
  "unit": "MU",
  "source_model": "Renewable",
  "topology": { "Patchy": { "hotspot_count": 4, "radius_cells": 2.5 } },
  "initial_amount": 150.0,
  "source_rate": 0.05,
  "diffusion_rate": 0.1,
  "decay_rate": 0.01,
  "max_density": 10.0,
  "boundary": "Closed"
}
```

Validation rules enforced in `ExoticEnergyLaw::validate()`:
- `id` and `display_name` non-empty; `unit` non-empty and **not** `"EU"`.
- `source_model` currently supports `"Renewable"` only.
- `topology` is `"Uniform"` or `{"Patchy": {"hotspot_count": >= 1, "radius_cells": > 0.0}}`.
- `boundary` is `"Closed"` or `"Open"`.
- `initial_amount >= 0.0`, `source_rate >= 0.0`, `0.0 <= diffusion_rate <= 0.25`, `0.0 <= decay_rate <= 1.0`, `max_density > 0.0` (all finite).

---

## 4. Configure `initial_conditions` per Model

`InitialConditionSet` has `{"schema_version": 1, "values": [["<key>", <f64>], ...]}` — unique keys, all finite values.

### 4.1 Under `--model reference`

- **7 baseline ecological keys:** `precip`, `temperature`, `npp`, `plants`, `herbivores`, `predators`, `detritus` (omitted keys take `ReferenceEcosystem` defaults).
- **13 AE3 evolution keys (optional, opt-in):** `ae3.population_total`, `ae3.population_capacity`, `ae3.pathway_fraction`, `ae3.generation_ticks`, `ae3.base_performance`, `ae3.mutation_rate`, `ae3.pathway_sensing`, `ae3.pathway_uptake`, `ae3.pathway_storage`, `ae3.pathway_efficiency`, `ae3.pathway_tolerance`, `ae3.pathway_maintenance`, `ae3.pathway_allocation`. Note: requesting `evolution.*` observables without `ae3.population_total` fails validation immediately.

### 4.2 Under `--model live`

`LiveExperimentAdapter` enforces a strict list of **29 `LIVE_KEYS`** (`src-tauri/src/core/live_experiment.rs`); any unrecognized key (including `plants` or `precip`) is rejected:

- Population & world setup: `live.founders`, `live.founder_layout`, `live.predator_fraction`, `live.trees`, `live.lakes`, `live.lake_layout`, `live.food_cap`, `live.land_mask`, `live.resource_capacity_scale`, `live.plant_reserve`.
- Brains, behavior & biology: `live.evolved_brains`, `live.mortality`, `live.reproduction`, `live.locomotion`, `live.drive_floor`, `live.thirst_steering`, `live.grass_steering`, `live.mutation_rate`, `live.starve_ticks`, `live.dehydration_ticks`, `live.ecological_fitness`, `live.w_surplus`, `live.kill_hydration`, `live.kill_efficiency`, `live.target_min_energy`, `live.prey_cap`, `live.strict_prey_cap`.
- Drift controls: `live.drift_control`, `live.drift_reset_ticks`.

**Coupled key constraints under `--model live`:**
- `live.lake_layout = 1.0` requires `live.lakes >= 1`.
- `live.drift_control = 1.0` requires both `live.mortality = 1.0` and `live.reproduction = 1.0`.
- `live.drift_reset_ticks > 0` requires `live.drift_control = 1.0`.
- `live.grass_steering = 1.0` requires `live.locomotion = 1.0`.
- Requesting `live.deaths*` / `live.mean_lifespan` requires `live.mortality = 1.0` (and rejects `live.drift_control` for `deaths_starvation` / `deaths_dehydration`); `live.births` requires `live.reproduction = 1.0`.

---

## 5. Configure `interventions`, `exotic_interventions`, and `observer`

- **`InterventionCommand`:** unique `id`, `cause_id` (not `CAUSE_OBSERVER`), `kind` (one of 8 variants such as `"RainfallDelta"`, `"TemperatureDelta"`, `"Deforest"`, `"RemovePredators"`, `"AddNutrient"`, or parameterized variants `GeneticInjection`, `ToxicityPlume`, `PathogenTransmission`), `region` (`"Global"`, `{"Cell": {"x": ..., "y": ...}}`, `{"Rect": {...}}`, `{"Radius": {...}}`), `start_tick`, `duration_ticks` (`0` means 1 tick), `intensity` (`>= 0.0`, finite; sign controlled by `signed_negative`), `signed_negative`, `curve` (`"Step"`, `"RampUp"`, `"RampDown"`, `"Triangle"`), and `reversible`. `[start_tick, start_tick + max(duration_ticks, 1))` must overlap `1..=duration_ticks`.
- **`ExoticIntervention`:** applies to the `MU` field (`AddSource`, `RemoveSource`, `Pulse`), requires `laws.exotic_energy != null`, and must cover at least one `ECOLOGY_PERIOD` tick.
- **`ObserverPolicy`:** `{"mode": "absent"}` for standard headless runs; `{"mode": "spectate"}` must match `Absent` identically; `{"mode": "inhabit", "cause_id": <u32>}` forbids `cause_id == 0` (`CAUSE_BACKGROUND`).

---

## 6. Pair Control and Treatment Arms (`FactorDiff`)

When pairing `control` and `treatment` manifests (as in E2):
1. `FactorDiff::diff_paths` ignores `experiment_id` and `name` while checking all 12 remaining factor paths (`observer`, `world_identity`, `laws.*`, `initial_conditions`, `interventions`, `exotic_interventions`, `seeds`, `duration_ticks`, `sample_period`, `observables`).
2. `run_paired_ensemble_with_control` requires `control.seeds == treatment.seeds` in the **same order**.
3. Only the registered factor path(s) may differ between arms.

---

## 7. Expand to `SweepManifest` or `BranchTreeManifest`

- **`SweepManifest`** (`#[serde(deny_unknown_fields)]`): wraps `base_manifest` with `parameters` (1–32 dimensions, each specifying `name`, `target` in `InitialCondition` / `ExoticLawField` / `InterventionIntensity` / `InterventionStartTick` / `InterventionDuration`, and `spec` in `Continuous` / `Logarithmic` / `IntegerRange` / `Discrete`), `sampling` (`Grid`, `Sobol`, or `LatinHypercube`), and optional `max_threads`. Total grid/sample cap is `100_000` points.
- **`BranchTreeManifest`** (`#[serde(deny_unknown_fields)]`): wraps `base_manifest` with `nodes` (1–1024 nodes, exactly one root with `parent_id: null` and `fork_tick: 0`, child nodes enforcing `parent.fork_tick <= child.fork_tick <= parent.duration_ticks`, and child interventions restricted to `start_tick >= fork_tick` inside `(fork_tick, duration_ticks]`) plus `comparisons: [[id_a, id_b], ...]`. `seed_strategy` supports `InheritStream`, `DerivedStream { stream_id }`, or `ExplicitSeed { seed }`.

---

## 8. Validate Practically with `anima-lab` (Including a Negative Control)

The script below constructs a valid manifest with a `RampUp` negative `RainfallDelta` intervention, runs it through `anima-lab`, extracts the `u64` `manifest_fingerprint` without IEEE-754 precision loss (parsing raw JSON text before `JSON.parse` coerces integers > `2^53 - 1` into `Number`), and verifies that an invalid manifest fails closed:

<!-- doc-test -->
```bash
cargo build --manifest-path src-tauri/Cargo.toml --release --bin anima-lab
mkdir -p src-tauri/target/doc2-runs
cat <<'EOF' > src-tauri/target/doc2-runs/custom-manifest.json
{
  "schema_version": 1,
  "experiment_id": "doc2-custom-drought",
  "name": "drought-test",
  "world_identity": {
    "seed": 1337,
    "generator_version": 20,
    "width": 128,
    "height": 128,
    "checksum": 3735928559
  },
  "laws": {
    "schema_version": 1,
    "baseline_energy": { "eu_unit": "EU" },
    "exotic_energy": null
  },
  "initial_conditions": {
    "schema_version": 1,
    "values": [
      ["precip", 1.0],
      ["temperature", 0.5],
      ["plants", 100.0],
      ["herbivores", 40.0],
      ["predators", 8.0],
      ["detritus", 0.0]
    ]
  },
  "interventions": [
    {
      "id": 1,
      "cause_id": 100,
      "kind": "RainfallDelta",
      "region": "Global",
      "start_tick": 120,
      "duration_ticks": 300,
      "intensity": 0.3,
      "signed_negative": true,
      "curve": "RampUp",
      "reversible": true
    }
  ],
  "seeds": [1, 2],
  "duration_ticks": 600,
  "sample_period": 300,
  "observable_ids": ["precip", "plants", "herbivores"],
  "exotic_interventions": [],
  "observer": { "mode": "absent" }
}
EOF
./src-tauri/target/release/anima-lab \
  src-tauri/target/doc2-runs/custom-manifest.json \
  > src-tauri/target/doc2-runs/custom-out.json
node -e "const raw = require('fs').readFileSync('src-tauri/target/doc2-runs/custom-out.json','utf8'); const r = JSON.parse(raw); const fp = raw.match(/\"manifest_fingerprint\"\\s*:\\s*(\\d+)/)?.[1]; console.log({ id: r.experiment_id, fingerprint: fp, completed: r.completed_runs, metrics: r.metrics.map(m => [m.observable, Number(m.mean.toFixed(4))]) });"
! ./src-tauri/target/release/anima-lab \
  src-tauri/tests/fixtures/experiments/invalid-negative-source.json \
  2> src-tauri/target/doc2-runs/manifest-err.txt
grep -q "exotic energy source_rate must be >= 0" src-tauri/target/doc2-runs/manifest-err.txt
```

Observed output (`macOS arm64`, `2026-11-23`, prebuilt `anima-lab` binary at `c1ed63c`, verified identical experiment/math sources on `e576350`):

```text
{
  id: 'doc2-custom-drought',
  fingerprint: '12352719949087380577',
  completed: 2,
  metrics: [
    [ 'precip', 1 ],
    [ 'temperature', 0.5 ],
    [ 'npp', 0.9672 ],
    [ 'plants', 94.803 ],
    [ 'herbivores', 36.4096 ],
    [ 'predators', 7.825 ],
    [ 'detritus', 3.2859 ]
  ]
}
```

---

## Next Steps

- Run `ensemble`, `sweep`, and `branch`: [RUN_EXPERIMENTS.md](RUN_EXPERIMENTS.md).
- Add a new scalar observable: [ADD_AN_OBSERVABLE.md](ADD_AN_OBSERVABLE.md).
- Add a new intervention kind: [ADD_AN_INTERVENTION.md](ADD_AN_INTERVENTION.md).
