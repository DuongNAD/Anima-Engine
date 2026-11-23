---
title: Onboarding and First Headless Tutorials
status: active
owner: maintainers
last_reviewed: 2026-11-23
review_cycle: per-release
---

# Tutorials

## Hands-on Tutorials

| Tutorial | Contents | Status |
|---|---|---|
| [01 — From a Clean Clone to Your First Headless Experiment](01-first-headless-experiment.md) | Go from a clean clone to installing dependencies, running `e2_seam_tests`, building `run_e2_brain_experiment` (`--release`), running your first E2 `--smoke` calibration, verifying digests with `npm run check:e2`, and reading the inference boundaries | `active` |

## 60-Minute Onboarding Path

### 1. Understand the Simulated World

Read in order:

1. [`README.md`](../../README.md) — project goals and architecture.
2. [`WORLD_DESIGN.md`](../../WORLD_DESIGN.md) — target world and user experience.
3. [`SIMULATION_RULES.md`](../../SIMULATION_RULES.md) — units, conservation laws, and MVP slices.

Expected outcome: trace the causal chain
"rain → water/soil → grass → rabbit → wolf" and distinguish causal domain events from technical logs.

### 2. Run a Verification Pass

Follow [How-to](../how-to/README.md): install dependencies and run frontend and Cargo
tests. Read [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md) before generating a baseline report on
your machine: the script reads `cargo bench` output and refuses to run on an unbenched clone (see How-to).

Expected outcome: identify which checks guard Rust, TypeScript, and artifact boundaries.

### 3. Trace One Change End-to-End

Pick a small scenario from [`WORLD_SIMULATION_PLAN.md`](../../WORLD_SIMULATION_PLAN.md):

1. locate the affected law in `SIMULATION_RULES.md`;
2. locate the matching component/system under `src-tauri/src/core/`;
3. locate the corresponding artifact/renderer consumer;
4. identify invariants, fixtures, and benchmarks to update;
5. record follow-up work in the backlog at [`STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md) §3
   (`TODO.md` is a chronological log of completed work); do not redefine domain laws there.

### 4. Propose an Open-Source Integration

Read the [open-source landscape survey](../research/OPEN_SOURCE_LANDSCAPE.md), pick a `Pilot` candidate, and
use the [ADR template](../decisions/ADR-0000-template.md) to record boundaries, baseline,
quality gates, and rollback plan. Do not add a package before license and source-of-truth are settled.

## Suggested Next Tutorials

- Author a "rainfall +20%" scenario and inspect the causal chain.
- Add a producer species from schema to renderer.
- Build a scientific fixture with provenance.
- Migrate `WorldArtifact` from version N to N+1.

Every new tutorial must run from a clean checkout, include observed outputs, and link
to Reference contracts rather than duplicating specification prose.
