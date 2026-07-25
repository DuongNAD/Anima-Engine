---
kind: agent-goal
agent: claude-code-background
feature: alternate-evolution-world-lab
title: Overnight Goal — Headless Evolution Lab AE1–AE2
status: completed
created: 2026-07-24
completed: 2026-07-25
owner: simulation-architecture
---

# Overnight Goal — Headless Evolution Lab AE1–AE2

## Mission

Work continuously on the first safe, testable vertical foundation for the Alternate Evolution &
World Lab. Build the headless experiment contract and the disabled-by-default exotic-energy
subsystem needed to compare:

- the current baseline world, where exotic energy is absent; and
- an alternate world that has a renewable spatial energy source displayed as **Mana**.

Do not stop after analysis, planning, scaffolding, or the first passing test. Continue through the
ordered milestones below until every in-scope gate has fresh evidence or a genuine hard blocker
prevents safe progress.

This goal authorizes an **experimental, default-off AE1–AE2 implementation** using ADR-0002 option C
as the working direction. It does **not** authorize changing ADR-0002 from `proposed` to `accepted`.

## Required reading order

Read the current files directly. Do not rely on chat summaries or historical line numbers.

1. `CLAUDE.md`
2. `docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md`
3. `docs/decisions/ADR-0002-world-laws-and-exotic-energy.md`
4. `docs/explanation/ALTERNATE_EVOLUTIONARY_REGIMES.md`
5. `docs/ai/requirements/2026-07-24-feature-alternate-evolution-world-lab.md`
6. `docs/ai/design/2026-07-24-feature-alternate-evolution-world-lab.md`
7. `docs/ai/testing/2026-07-24-feature-alternate-evolution-world-lab.md`
8. `docs/ai/planning/2026-07-24-feature-alternate-evolution-world-lab.md`
9. `docs/ai/implementation/2026-07-24-feature-alternate-evolution-world-lab.md`
10. Current code at the symbols named in the implementation document.

If documents disagree, preserve the invariants in the experiment contract, record the conflict, and
choose the smallest reversible implementation. Do not silently invent a new contract.

## Workspace safety

The current working tree already contains extensive user and agent changes.

- Capture `git status --short --branch` before editing.
- Never run `git reset`, `git checkout --`, `git restore`, `git clean`, stash, recursive deletion, or
  any command that discards/replaces existing work.
- Do not create/switch branches or worktrees, commit, push, or open a PR.
- Treat every pre-existing modification and untracked file as user-owned.
- Prefer new modules and narrow edits at verified anchors. Before editing a dirty file, inspect its
  current diff and preserve unrelated hunks.
- Do not “fix” unrelated failures or reformat unrelated files.
- If safe ownership of an overlapping hunk cannot be established, record a blocker and continue with
  an independent task.

## Hard scientific and architectural constraints

- Core code uses generic `ExoticEnergy`; `Mana` is only a scenario/display label.
- `exotic_energy = None` is the default, rollback path, and baseline-compatibility gate.
- MU is not EU. Maintain a separate MU ledger and preserve the closed-EU semantics.
- Exotic energy must not directly rewrite genotype, assign species, increase population, or add
  fitness. AE1–AE2 stops before organism pathway/evolution behavior.
- World laws are fixed before genesis. A law change creates a declared fork; it is not a silent
  in-place mutation.
- Deterministic paths must not use `thread_rng()`. Derive streams from declared seeds and stable
  identities.
- Hot field updates preallocate buffers and perform zero heap allocations after initialization.
- Results and observables are self-describing; UI-specific inference is forbidden.
- Do not claim adaptation, ecotype, candidate species, or species from AE1–AE2 evidence.
- Do not touch map generation, terrain, renderer, placement, navigation, ecology visuals, or UI.
  `animal-map-vision` is unavailable, so map review remains explicitly blocked rather than passed.

## Ordered milestones

### M0 — Baseline and insertion-point audit

1. Inspect current diffs in every file that may be touched.
2. Read `core/scenario.rs`, `core/sim_rules.rs`, `core/ecology.rs`, `core/causal.rs`,
   `core/world_artifact.rs`, and `core/mod.rs`.
3. Run the current Rust library tests before feature edits and record the exact command, exit code,
   pass/fail count, and any pre-existing failure.
4. Reconcile task IDs and real code symbols in the planning/implementation documents if an anchor is
   stale. Do not mark production tasks complete during this audit.

### M1 — AE1 manifest identity and observability

Implement with tests first:

- AE-101: `core/experiment.rs` and structured validation errors.
- AE-102: versioned `WorldLawSet`, `InitialConditionSet`, and deterministic canonical identity.
- AE-103: `ExperimentManifest`, validator, declared-factor allowlist, and stable fingerprint.
- AE-109: `ObservableRegistry` with stable id, unit, scope, source, cadence, range/tolerance, and
  conservation metadata.

Required evidence:

- Same logical manifest with reordered non-semantic input has the same fingerprint.
- Every material world-law change changes the fingerprint.
- Invalid/unknown versions, units, ranges, duplicate seeds, and undeclared factor differences fail
  with structured errors.
- Observable IDs are unique and registry metadata validates.

Do not use map order or unstable debug formatting as canonical serialization.

### M2 — AE1 deterministic runner, forks, provenance, and result schema

Implement while preserving the legacy API:

- AE-104: manifest-aware model factory/adapter for the headless reference model.
- AE-105: `RunProvenance` with experiment/run/parent/fork identity.
- AE-106: genesis control/treatment fork with identical shared inputs and declared differences only.
- AE-108: seed-set ensemble runner that preserves failed runs.
- AE-110: self-describing result schema with manifest fingerprint, versions, seeds, series,
  observables, budgets/ledger slots, checksums, warnings, and failures.
- AE-111: compatibility adapter for the existing `Scenario` callers and current S10–S14 behavior.

Checkpoint fork AE-107 is optional only if a current snapshot seam exists without touching
persistence/live Bevy. Otherwise record it as a dependency; do not fake a checkpoint with
re-simulation and call it identical.

Required evidence:

- Same manifest + seed + build path gives the same checksum.
- Genesis forks differ only in declared factors.
- Ensemble summaries keep N, per-run status, failures, and deterministic seed order.
- All pre-existing scenario/reference-model tests remain green.

### M3 — AE2 exotic-energy field and MU budget

Implement with the feature absent by default:

- AE-201: typed energy source/unit identifiers so MU and EU cannot be silently mixed.
- AE-202: renewable `Uniform`/`Patchy` exotic-energy laws with validation. (As implemented, the
  "disabled" configuration is `WorldLawSet.exotic_energy = None`; there is no
  `ExoticSourceModel::Disabled` variant, so a `Some(law)` is always a live `Renewable` source.)
- AE-203: deterministic preallocated `ExoticEnergyField` with source, diffusion, decay, bounds, and
  explicit boundary mode.
- AE-204: `ExoticEnergyBudget` covering initial amount, sources, field/storage delta, sinks, and
  balance error.
- AE-205: atomic transaction helpers for field/storage/dissipation seams, even if organism storage is
  represented only by a reference-model test double in this slice.
- AE-208: disabled fast path and baseline parity.

Required evidence:

- Uniform and patchy initialization are deterministic and bounded.
- Diffusion does not create or destroy MU beyond the declared numeric tolerance.
- Source/decay updates close the MU balance equation.
- Deliberate leakage is detected.
- The disabled path creates no hidden field, cost, RNG draw, or changed baseline checksum.
- A zero-allocation test or instrumentation proves no hot-loop allocation after initialization.

### M4 — AE2 headless reference vertical slice

Implement:

- AE-206: integrate the field/budget into the headless reference model only.
- AE-207: causal records from world-law/source changes to field/budget effects.

Create a minimal deterministic demonstration:

```text
same world/artifact + same initial state + same seed schedule
  control:   exotic_energy = None
  treatment: Renewable Patchy ExoticEnergy, display_name = "Mana"
```

The expected AE2 result is a measurable spatial field and a closed MU ledger. It is correct for
organism traits/population to remain unaffected because pathway and selection are intentionally
outside this goal.

Required evidence:

- Replaying either branch is deterministic.
- The manifest diff identifies only the declared exotic-energy law.
- Treatment produces the expected field/source time series and causal records.
- MU closes within the documented tolerance.
- Closed EU remains unchanged by exotic-field bookkeeping.
- Control remains compatible with the pre-feature reference result.

## TDD and verification loop

For each task group:

1. Add a focused failing test that represents the relevant AE-S gate.
2. Run it and confirm the failure is caused by missing behavior.
3. Implement the smallest contract-compliant behavior.
4. Run focused tests.
5. Run the full Rust library regression suite.
6. Run formatting/diff checks.
7. Record fresh evidence in the testing/implementation documents before moving on.

Use real test names rather than the placeholder `ae_` filter. Preferred checkpoints:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path src-tauri/Cargo.toml --lib
git diff --check
npx --yes --package ai-devkit@latest --package telegraf ai-devkit lint --feature alternate-evolution-world-lab
```

If the feature lint reports only the already-known missing feature branch, record it as infrastructure
evidence. Do not create/switch a branch in this dirty workspace merely to satisfy that check.

Do not claim success from old output. Every completed task/gate needs a fresh command, exit code, and
test count or named passing test.

## Progress protocol

After each milestone:

- Update task status in
  `docs/ai/planning/2026-07-24-feature-alternate-evolution-world-lab.md` only for work actually
  implemented and verified.
- Append concise implementation decisions and exact code anchors to
  `docs/ai/implementation/2026-07-24-feature-alternate-evolution-world-lab.md`.
- Update checked tests and verification evidence in
  `docs/ai/testing/2026-07-24-feature-alternate-evolution-world-lab.md`.
- Re-read `git diff --stat` and `git status --short` to detect accidental scope expansion.
- Continue automatically to the next milestone.

The optional AI DevKit `task` command is not available in this installation. Do not spend time
repeatedly probing it; the lifecycle documents above are the durable checkpoint record.

## Blocker and continuation rules

A single failing test, unavailable optional tool, or difficult subtask is not a reason to stop.

- Diagnose failures and distinguish new regression from pre-existing workspace failure.
- If one task is blocked, continue with dependency-independent in-scope work.
- Make up to three evidence-based attempts for a repeated technical blocker.
- Stop only when all remaining tasks depend on user authority, destructive conflict resolution,
  unavailable required external state, or an unresolved contract contradiction.
- Never bypass invariants to make a test pass.
- Never expand into AE3–AE7 just because AE1–AE2 finishes early.

## Explicitly out of scope

- Accepting ADR-0002 or setting a final MU tolerance/product schema policy on the user's behalf.
- Energy pathway genotype, phenotype/development, mutation/crossover, selection, or reproduction.
- Live Bevy ECS integration, save migration, frontend/World Lab UI, rendering, map work, or species
  detection.
- Performance claims on target hardware beyond focused local measurements.
- Commits, pushes, PRs, releases, or destructive workspace cleanup.

## Morning handoff

Leave a self-contained report in the implementation/testing documents and in the final agent message:

1. milestones and AE task IDs completed, partial, or blocked;
2. exact files and public symbols added/changed;
3. gate-to-test mapping and fresh command results;
4. baseline compatibility and MU/EU audit results;
5. unresolved blockers or contract decisions needed from the user;
6. the smallest safe next task, without claiming AE3+ completion.

The goal is successful when the in-scope headless foundation is implemented and verified, or when the
remaining work is genuinely blocked and the evidence is sufficient for another agent to resume
without repeating the investigation.
