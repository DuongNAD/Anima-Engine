---
title: How to Run Headless Experiments with anima-lab
status: active
owner: maintainers
last_reviewed: 2026-11-23
review_cycle: per-release
---

# How to Run Headless Experiments with `anima-lab`

This guide shows how to compile and run the headless `anima-lab` CLI across its three subcommands: **ensemble** (multi-seed manifest execution), **sweep** (multi-dimensional parameter sampling), and **branch** (counterfactual causal branching).

> **Related onboarding and contracts:** if you are starting from a fresh clone, see the [Tutorials index](../tutorials/README.md). Quantitative contracts live in [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) *(proposed)*, [`DETERMINISM_CONTRACT.md`](../reference/DETERMINISM_CONTRACT.md), and [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).

---

## 1. Compile the `--release` `anima-lab` Binary

`anima-lab` is declared in [`src-tauri/Cargo.toml`](../../src-tauri/Cargo.toml) (`[[bin]] name = "anima-lab"`, source at [`src-tauri/src/bin/anima-lab.rs`](../../src-tauri/src/bin/anima-lab.rs)) and compiles cleanly under `default` features — **no** `--features desktop`, Tauri window, Neo4j client, or GPU device required.

From the repository root, build the release binary and print `--help`:

<!-- doc-test -->
```bash
cargo build --manifest-path src-tauri/Cargo.toml --release --bin anima-lab
./src-tauri/target/release/anima-lab --help
```

> **Windows PowerShell *(not verified on Windows here)*:** the binary path is `.\src-tauri\target\release\anima-lab.exe`. Building first and invoking the compiled release binary directly (avoiding `npm run tauri:dev` and avoiding `cargo run`) is good practice everywhere and **mandatory** for preregistered E2 verification (`e2-preregistration.json` / `scripts/verify_e2_artifacts.mjs`), which explicitly forbids `cargo run`.

Observed `--help` output verbatim (`macOS arm64`, `2026-11-22`, commit `03b9d3d`):

```text
anima-lab — headless experimentation, multidimensional parameter sweeps, and causal branching

Usage:
    anima-lab [ensemble] [--model reference|live] <MANIFEST.json>
    anima-lab sweep [--model reference|live] [--format json|csv] [--threads N] <SWEEP_MANIFEST.json>
    anima-lab branch [--model reference|live] <BRANCH_MANIFEST.json>
    anima-lab --help

Options:
    --model <reference|live>   Simulation execution model (default: reference)
    --format <json|csv>        Output format for sweep (default: json)
    --threads <N>              Rayon thread pool size for sweeps (default: auto)
    -h, --help                 Print this help text

Exit Codes:
    0: All runs/points completed successfully
    1: Preserved runtime failures or invalid manifest
```

CLI behavior notes from `anima-lab.rs`:

- Omitting a subcommand defaults to `ensemble`.
- `--format` and `--threads` apply **only** to `sweep` (passing them to `ensemble` or `branch` exits `1`).
- Maximum manifest file size is `16 MiB` (`16 * 1024 * 1024` bytes).
- Structured results (`ensemble` JSON, `sweep` JSON/CSV, `branch` JSON) print to **`stdout`**; validation errors print to **`stderr`**. If any seed panics/fails during an `ensemble`, `anima-lab` still prints full JSON to `stdout` (preserving failure metadata under `failed`) and exits `1`.

---

## 2. Run `ensemble` on the `reference` Model (Default)

When `--model` is `reference` (or omitted), `anima-lab` executes `ReferenceEvolutionWorld` with `ObservableRegistry::reference_default()`. Two committed fixtures under [`src-tauri/tests/fixtures/experiments/`](../../src-tauri/tests/fixtures/experiments) demonstrate runs without and with an exotic `Mana` (`MU`) field:

<!-- doc-test -->
```bash
mkdir -p src-tauri/target/doc2-runs
./src-tauri/target/release/anima-lab \
  src-tauri/tests/fixtures/experiments/baseline-no-exotic.json \
  > src-tauri/target/doc2-runs/baseline.json
./src-tauri/target/release/anima-lab ensemble --model reference \
  src-tauri/tests/fixtures/experiments/mana-patchy-renewable.json \
  > src-tauri/target/doc2-runs/mana.json
node -e "for (const f of ['baseline.json','mana.json']) { const r = JSON.parse(require('fs').readFileSync('src-tauri/target/doc2-runs/' + f, 'utf8')); console.log(f, { tool: r.tool, model: r.model, id: r.experiment_id, completed: r.completed_runs, failed: r.failed.length, metrics: r.metrics.map(m => [m.observable, Number(m.mean.toFixed(4))]) }); }"
```

Observed output (`macOS arm64`, `2026-11-22`, commit `03b9d3d`, summarized via Node):

```text
baseline.json {
  tool: 'anima-lab',
  model: 'reference',
  id: 'exp-1',
  completed: 3,
  failed: 0,
  metrics: [
    [ 'precip', 1 ],
    [ 'temperature', 0.5 ],
    [ 'npp', 1.0017 ],
    [ 'plants', 100.0939 ],
    [ 'herbivores', 36.3957 ],
    [ 'predators', 7.2839 ],
    [ 'detritus', 0.7081 ]
  ]
}
mana.json {
  tool: 'anima-lab',
  model: 'reference',
  id: 'ae-mana-patchy',
  completed: 3,
  failed: 0,
  metrics: [
    [ 'precip', 1 ],
    [ 'temperature', 0.5 ],
    [ 'npp', 1.0017 ],
    [ 'plants', 100.0939 ],
    [ 'herbivores', 36.3957 ],
    [ 'predators', 7.2839 ],
    [ 'detritus', 0.7081 ],
    [ 'exotic.density_total', 109.3622 ],
    [ 'exotic.sourced', 87.9029 ],
    [ 'exotic.dissipated', 128.5406 ],
    [ 'exotic.stored', 0 ],
    [ 'exotic.budget_error', -0 ]
  ]
}
```

Top-level keys in `EnsembleReport` JSON: `tool`, `manifest`, `model`, `registry`, `experiment_id`, `manifest_fingerprint`, `requested_runs`, `completed_runs`, `failed`, `seed_order`, `runs` (per-seed time series and `final_observables`), and `metrics` (cross-seed summaries per observable).

---

## 3. Run `ensemble` on the `live` Model

Pass `--model live` to drive `LiveExperimentAdapter` (the headless Bevy ECS schedule `simulation_schedule::build_tick_schedule`). The registry switches to `ObservableRegistry::live_default()` and `initial_conditions` accepts only `live.*` keys:

<!-- doc-test -->
```bash
mkdir -p src-tauri/target/doc2-runs
./src-tauri/target/release/anima-lab --model live \
  src-tauri/tests/fixtures/experiments_e2/e2-smoke-control-shared-brain.json \
  > src-tauri/target/doc2-runs/e2-live.json
node -e "const r = JSON.parse(require('fs').readFileSync('src-tauri/target/doc2-runs/e2-live.json','utf8')); console.log({ model: r.model, registry: r.registry, completed: r.completed_runs, final_count: r.runs[0].final_observables.length, final_head: r.runs[0].final_observables.slice(0, 5) });"
```

Observed output (`macOS arm64`, `2026-11-22`, commit `03b9d3d`, 1 seed × 18,000 ticks):

```text
{
  model: 'live',
  registry: 'core::experiment::ObservableRegistry::live_default',
  completed: 1,
  final_count: 27,
  final_head: [
    [ 'plants', 154951.8024202675 ],
    [ 'detritus', 1019.9974421173067 ],
    [ 'live.animals_eu', 0 ],
    [ 'live.dormant_eu', 0 ],
    [ 'live.closed_eu_total', 155971.7998623848 ]
  ]
}
```

> **Model boundary:** running `--model live` on `mana-patchy-renewable.json` fails validation (`observable 'herbivores' is not in the registry`, exit `1`), and `LiveExperimentAdapter` rejects manifests with `laws.exotic_energy != null`. For full preregistered E2 reproduction (12 paired seeds, `E2-G1..G6`, `npm run check:e2`), see [REPRODUCE_E2.md](REPRODUCE_E2.md).

---

## 4. Run `sweep` (JSON/CSV Output and `--threads`)

No committed JSON fixture exists for `SweepManifest` yet (tests build sweeps in Rust). To run a 4-point `Sobol` sweep across `precip` and `plants` into gitignored `src-tauri/target/doc2-runs/` with both `--format json` and `--format csv --threads 2`:

<!-- doc-test -->
```bash
mkdir -p src-tauri/target/doc2-runs
cat <<'EOF' > src-tauri/target/doc2-runs/sweep-sobol.json
{
  "schema_version": 1,
  "sweep_id": "doc2-sweep-precip",
  "name": "sobol-precip-plants",
  "base_manifest": {
    "schema_version": 1,
    "experiment_id": "doc2-sweep-base",
    "name": "sweep-base",
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
    "interventions": [],
    "seeds": [1],
    "duration_ticks": 600,
    "sample_period": 300,
    "observable_ids": ["plants", "herbivores"],
    "exotic_interventions": [],
    "observer": { "mode": "absent" }
  },
  "parameters": [
    {
      "name": "precip_init",
      "target": { "type": "InitialCondition", "key": "precip" },
      "spec": { "scale": "Continuous", "min": 0.5, "max": 1.5 }
    },
    {
      "name": "plants_init",
      "target": { "type": "InitialCondition", "key": "plants" },
      "spec": { "scale": "Continuous", "min": 80.0, "max": 120.0 }
    }
  ],
  "sampling": { "strategy": "Sobol", "num_samples": 4, "offset": 0 },
  "max_threads": 2
}
EOF
./src-tauri/target/release/anima-lab sweep --format json \
  src-tauri/target/doc2-runs/sweep-sobol.json \
  > src-tauri/target/doc2-runs/sweep.json
node -e "const s = JSON.parse(require('fs').readFileSync('src-tauri/target/doc2-runs/sweep.json','utf8')); console.log({ sweep_id: s.sweep_id, total_points: s.total_points, completed_points: s.completed_points, failed_runs: s.failed_runs, params: s.parameter_names, plants_corr: s.global_metrics.plants.parameter_correlations });"
./src-tauri/target/release/anima-lab sweep --format csv --threads 2 \
  src-tauri/target/doc2-runs/sweep-sobol.json \
  > src-tauri/target/doc2-runs/sweep.csv
head -n 5 src-tauri/target/doc2-runs/sweep.csv
```

Observed output (`macOS arm64`, `2026-11-22`, commit `03b9d3d`):

```text
{
  sweep_id: 'doc2-sweep-precip',
  total_points: 4,
  completed_points: 4,
  failed_runs: 0,
  params: [ 'precip_init', 'plants_init' ],
  plants_corr: [ -0.8285714285714282, 1 ]
}
point_index,precip_init,plants_init,completed_runs,failed_runs,detritus_mean,detritus_std,detritus_min,detritus_max,herbivores_mean,herbivores_std,herbivores_min,herbivores_max,...
0,1,100,1,0,2.3280312064081556,0,2.3280312064081556,2.3280312064081556,37.431961750649805,0,...
1,1.25,90,1,0,2.9911463719847187,0,2.9911463719847187,2.9911463719847187,36.67012882527874,0,...
2,0.75,110,1,0,1.6649160408315915,0,1.6649160408315915,1.6649160408315915,38.19379467602087,0,...
3,0.875,95,1,0,2.6595887891964374,0,2.6595887891964374,2.6595887891964374,37.05104528796427,0,...
```

Notes for reading `sweep` output:

- In `SweepResult` JSON, `global_metrics` maps `observable_id -> { mean, std, min, max, parameter_correlations }` ordered to match `parameter_names`.
- `--format csv` emits one row per point with `point_index,<params...>,completed_runs,failed_runs,<obs>_mean,<obs>_std,<obs>_min,<obs>_max` in alphabetical observable order.
- Any failed point (`failed_runs > 0`) returns exit code `1`.

---

## 5. Run `branch` (Counterfactual Branching Trees)

Like `sweep`, `BranchTreeManifest` currently has no committed JSON fixture under `src-tauri/tests/fixtures/`. The example below builds a two-node tree (root `id: 1` for 600 ticks; child `id: 2` forking at `fork_tick: 300`, keeping `InheritStream` RNG, and applying `Deforest` from tick `301` for `120` ticks) and computes pair divergence `(1, 2)`:

<!-- doc-test -->
```bash
mkdir -p src-tauri/target/doc2-runs
cat <<'EOF' > src-tauri/target/doc2-runs/branch-tree.json
{
  "schema_version": 1,
  "tree_id": "doc2-branch-deforest",
  "name": "deforest-counterfactual",
  "base_manifest": {
    "schema_version": 1,
    "experiment_id": "doc2-branch-base",
    "name": "branch-base",
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
    "interventions": [],
    "seeds": [1],
    "duration_ticks": 600,
    "sample_period": 300,
    "observable_ids": ["plants", "herbivores", "detritus"],
    "exotic_interventions": [],
    "observer": { "mode": "absent" }
  },
  "nodes": [
    {
      "id": 1,
      "parent_id": null,
      "fork_tick": 0,
      "duration_ticks": 600,
      "interventions": [],
      "exotic_interventions": [],
      "seed_strategy": { "type": "InheritStream" },
      "label": "baseline"
    },
    {
      "id": 2,
      "parent_id": 1,
      "fork_tick": 300,
      "duration_ticks": 600,
      "interventions": [
        {
          "id": 10,
          "cause_id": 500,
          "kind": "Deforest",
          "region": "Global",
          "start_tick": 301,
          "duration_ticks": 120,
          "intensity": 0.4,
          "signed_negative": false,
          "curve": "Step",
          "reversible": false
        }
      ],
      "exotic_interventions": [],
      "seed_strategy": { "type": "InheritStream" },
      "label": "deforest-t301"
    }
  ],
  "comparisons": [[1, 2]]
}
EOF
./src-tauri/target/release/anima-lab branch \
  src-tauri/target/doc2-runs/branch-tree.json \
  > src-tauri/target/doc2-runs/branch.json
node -e "const b = JSON.parse(require('fs').readFileSync('src-tauri/target/doc2-runs/branch.json','utf8')); console.log({ tree_id: b.tree_id, root_id: b.root_id, nodes: Object.keys(b.nodes).map(k => ({ id: k, label: b.nodes[k].spec.label, status: b.nodes[k].status })), phase_space: b.divergences[0].phase_space, plants_div: b.divergences[0].observables.find(o => o.observable_id === 'plants') });"
```

Observed output (`macOS arm64`, `2026-11-22`, commit `03b9d3d`):

```text
{
  tree_id: 'doc2-branch-deforest',
  root_id: 1,
  nodes: [
    { id: '1', label: 'baseline', status: 'Completed' },
    { id: '2', label: 'deforest-t301', status: 'Completed' }
  ],
  phase_space: {
    mean_distance: 3.854512240980469,
    max_distance: 7.709024481960938,
    final_distance: 7.709024481960938,
    trajectory_length_a: 1.2407503661557848,
    trajectory_length_b: 8.121790624402612
  },
  plants_div: {
    observable_id: 'plants',
    mean_delta: 3.6893211964892245,
    max_abs_delta: 7.378642392978449,
    rmsd: 5.217488072025596,
    relative_divergence: 1.4211062734777986,
    dtw_distance: 7.378642392978449
  }
}
```

---

## 6. Run a Negative Control with an Invalid Manifest

Always verify malformed manifests exit `1` rather than running silently. Fixture [`invalid-negative-source.json`](../../src-tauri/tests/fixtures/experiments/invalid-negative-source.json) intentionally sets `source_rate: -0.05`:

<!-- doc-test -->
```bash
mkdir -p src-tauri/target/doc2-runs
! ./src-tauri/target/release/anima-lab \
  src-tauri/tests/fixtures/experiments/invalid-negative-source.json \
  > src-tauri/target/doc2-runs/err-out.txt \
  2> src-tauri/target/doc2-runs/err.txt
cat src-tauri/target/doc2-runs/err.txt
grep -q "exotic energy source_rate must be >= 0" src-tauri/target/doc2-runs/err.txt
```

Observed `stderr` output (`macOS arm64`, `2026-11-22`, commit `03b9d3d`, exit `1`, empty `stdout`):

```text
anima-lab: ensemble rejected: invalid world law: exotic energy source_rate must be >= 0 (got -0.05)
```

---

## Next Steps

- Author or edit manifests directly: [WRITE_A_MANIFEST.md](WRITE_A_MANIFEST.md).
- Reproduce the full preregistered E2 suite: [REPRODUCE_E2.md](REPRODUCE_E2.md).
- Architecture & contract indexes: [Reference index](../reference/README.md) and [Explanation index](../explanation/README.md).
