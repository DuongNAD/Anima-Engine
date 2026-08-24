---
kind: agent-goal
agent: claude-code-background
feature: alternate-evolution-world-lab
title: Overnight Goal — AE3 Headless Pathway and Selection Slice
status: completed
created: 2026-07-25
completed: 2026-07-25
owner: simulation-architecture
predecessor: 2026-07-25-claude-overnight-goal-ae25.md
---

# Claude Overnight Goal — AE3 Headless Pathway and Selection Slice

> ## 📜 Historical package record — not current status
>
> This document belongs to **one dated work package**. Every count in it (test totals, warning
> counts, target counts, coverage numbers) is a **historical measurement**: true when the command
> ran during that package, and not a description of the tree today.
>
> **Current measured status lives in exactly one place:**
> [`docs/planning/STATE_OF_THE_PROJECT.md` §1](../../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).


**Status:** complete for the scoped headless reference slice; independent audit passed  
**Date:** 2026-07-25  
**Owner:** project owner  
**Implementation agent:** Claude Code, `claude-opus-5`, maximum effort  
**Independent reviewer:** Codex  
**Parent plan:** `2026-07-24-feature-alternate-evolution-world-lab.md`

## Objective

Implement the smallest deterministic, headless vertical slice that can honestly demonstrate:

```text
exotic world law
  → spatial MU field
  → heritable energy pathway
  → uptake and explicit pathway cost
  → measured performance difference
  → explicit reproduction/selection event
  → pathway-frequency change
```

This goal covers AE-301…AE-310 for the **reference model only**. It must make AE-S06, AE-S07,
AE-S10, the AE3 portion of AE-S12, and a paired multi-seed AE-S14 experiment testable. It must not
claim speciation, live-world integration, persistence, UI completeness, or map completion.

## Required reading and authority

Read these files before editing:

1. `CLAUDE.md`
2. `docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md`
3. `docs/decisions/ADR-0002-world-laws-and-exotic-energy.md`
4. `docs/explanation/ALTERNATE_EVOLUTIONARY_REGIMES.md`
5. `docs/ai/requirements/2026-07-24-feature-alternate-evolution-world-lab.md`
6. `docs/ai/design/2026-07-24-feature-alternate-evolution-world-lab.md`
7. `docs/ai/planning/2026-07-24-feature-alternate-evolution-world-lab.md`
8. `docs/ai/implementation/2026-07-24-feature-alternate-evolution-world-lab.md`
9. `docs/ai/testing/2026-07-24-feature-alternate-evolution-world-lab.md`
10. `docs/ai/planning/2026-07-25-claude-overnight-goal-ae25.md`
11. Current symbols in `src-tauri/src/core/{reference_world,exotic_energy,experiment,
    experiment_runner,causal,sim_clock}.rs`

Authority order is current code plus fresh tests, then authoritative/current implementation and
testing sections, then planning, then requirements/design. Historical or superseded notes are not
instructions.

## Repository safety contract

The starting working tree is intentionally dirty: **83** `git status --short` entries, including
**18** tracked modifications, on branch `chore/init-and-frontend-test-fixes`. The AE1–AE2.5
foundation includes untracked files, so do not create or switch worktrees for this run.

- Do not stage, commit, push, stash, reset, restore, clean, checkout, delete, or rename unrelated
  files.
- Do not overwrite user changes. Re-read every file immediately before editing it.
- Record start/end dirty counts and list only files this goal intentionally changed.
- A changed dirty count is not automatically a failure: new AE3 files/docs are expected. Any
  unrelated change is a stop condition.

## Named slice assumptions

These assumptions are local to the AE3 reference slice and are **not** production schema decisions:

1. **Reference cohort model.** Add an opt-in, fixed-capacity headless population with two inherited
   strategies: a legacy no-pathway genotype and an exotic-pathway genotype. It may use aggregate
   cohorts rather than live ECS entities.
2. **Opt-in compatibility.** If the AE3 initial-condition keys are absent, the population is
   disabled and the shipped AE1–AE2.5 path remains bit-identical. Existing fixtures need no schema
   migration.
3. **Separate deterministic RNG stream.** Population variation/reproduction uses a stream derived
   deterministically from the manifest seed and does not consume or reorder the legacy ecology RNG.
4. **Generation boundary.** Heritable frequency may change only inside a named reproduction method
   on a declared deterministic cadence. Sensing, uptake, metabolism, performance calculation,
   survival accounting, and observability may not write genotype or frequency directly.
5. **Performance is a mechanism result, not a fitness field.** A genotype pays its declared
   maintenance/opportunity cost. MU can improve reproductive performance only after an atomic
   field→storage uptake and storage→dissipated spend. Do not credit MU to the closed-EU ledger and do
   not write a fitness scalar directly from `has_exotic`.
6. **No hidden product decision.** ADR-0002 remains proposed. Do not promote the local `1e-4`
   conservation assertion into the product tolerance/schema policy.

If a materially better minimal model is found, update this section with the exact replacement and
the reason before implementing it. Do not silently broaden scope.

## Required model boundaries

Prefer a new pure module such as `src-tauri/src/core/evolution_pathway.rs` for AE3 reference types
and mechanics. Integrate it into `ReferenceEvolutionWorld`; do not retrofit live
`MorphologyGenotype`, Bevy components, spawning, or Creature Development in this goal.

Required concepts (exact internal names may vary only if the docs are reconciled):

- `EnergyPathwayGenotype`: source id and bounded sensing, uptake, storage, efficiency, tolerance,
  maintenance-cost, and allocation traits; legacy/default is disabled and zero-cost.
- Seeded bounded mutation and crossover APIs taking an explicit RNG.
- `DevelopedEnergyPathway`: one-time materialization from genotype for the reference birth path.
- `ExoticEnergyState`: runtime stored MU, last uptake/spend, and toxicity load.
- A minimal population/reproduction state whose parent selection is derived from measured
  performance.
- Explicit counters for births/reproductive success and generation.

All public numeric inputs must reject or safely normalize non-finite/out-of-range values. Keep hot
field transactions allocation-free.

## Initial-condition seam

Use version-1 `InitialConditionSet` scalar keys as an opt-in **reference-fixture seam**, not as a new
world-law schema. Define constants and document every accepted key in the implementation/testing
docs. At minimum, a fixture must be able to declare:

- initial total/capacity and pathway fraction;
- generation cadence or equivalent deterministic reference setting;
- the pathway genotype/cost used by the factorial experiment.

Defaults must preserve legacy behavior. Validation must fail structurally for impossible enabled
population states; do not silently accept NaN, negative counts/capacity, frequency outside `[0,1]`,
or a pathway source id incompatible with the active exotic law.

## Update order and causal chain

On the AE3 cadence:

1. advance the existing closed-EU ecology exactly as before;
2. apply exotic forcings and field dynamics exactly as AE2.5 defines;
3. sense and atomically uptake MU into organism storage;
4. pay pathway maintenance/opportunity cost and, when useful, atomically spend MU;
5. derive finite non-negative performance from those mechanism outputs;
6. at a generation boundary, execute reproduction/selection;
7. record frequency delta only after offspring composition is resolved.

For a present-source treatment, record a traceable chain:

```text
CAUSE_EXOTIC_WORLD_LAW or forcing CauseId
  → exotic uptake/storage
  → pathway performance
  → reproduction/offspring
  → pathway frequency
```

The final frequency effect must trace to the original cause. In an absent-source world, pathway cost
may be rooted in `CAUSE_BACKGROUND`; do not fabricate a Mana cause.

Checkpoint restore must preserve population/genotype/runtime/RNG state through the existing in-memory
snapshot clone while resetting only ledger-local effect ids as required by the current runner.

## Required observables

Extend the authoritative registry and reference model together. Use stable, self-describing ids,
finite bounds, correct units, cadence, scope, aggregation, conservation role, and source. Include at
least:

- total reference population;
- pathway-bearing population or frequency;
- generation;
- per-strategy performance or performance delta;
- births/reproductive success;
- MU uptake/spend/storage needed to explain the mechanism.

When the opt-in population is disabled, do not emit misleading AE3 values. A manifest that requests
an AE3 observable must enable a valid reference population or fail validation/preflight rather than
returning a fabricated zero.

## Factorial evidence

Implement tests/fixtures or test builders for a clean 2×2 comparison:

| Exotic source | Pathway cost | Expected evidence |
|---|---:|---|
| absent | zero/legacy | control |
| absent | positive | pathway does not receive free advantage; frequency falls or reproductive success is lower |
| present | zero/low | MU uptake/spend changes performance through the transaction path |
| present | positive | benefit must overcome cost; result reports the actual trade-off |

Use same-seed paired ensembles through `run_paired_ensemble` or
`run_paired_ensemble_with_control`. Preserve failures and report paired effect/interval. Do not
declare adaptation from one seed.

## Acceptance tests

Tests must pin these behaviors:

- **AE-S01 regression:** no AE3 initial keys + `exotic_energy=None` remains bit-identical to
  `ReferenceEcosystem`.
- **AE-S02:** same manifest and seed reproduces population state, causal ledger, series, and checksum.
- **AE-S04/05 regression:** uptake/spend closes MU accounting; EU observables remain isolated from
  exotic conversion.
- **AE-S06:** in a source-absent world, a positive-cost pathway has lower reproductive performance
  than legacy/no-cost and does not increase by magic.
- **AE-S07:** in a source-present world, the pathway changes performance only after measured
  uptake/storage/spend; disabling uptake or efficiency removes the benefit.
- **AE-S10:** sensing/uptake/performance ticks cannot change frequency; a reproduction event can,
  and the delta equals resolved offspring composition.
- **AE-S12:** a final pathway-frequency effect traces through reproduction and performance to the
  exotic world-law/forcing cause.
- **AE-S14:** same-seed paired multi-seed report includes finite paired effect/interval and preserves
  failures; state the seed count without overclaiming statistical confidence.
- Seeded mutation/crossover is replay-deterministic and every trait remains bounded.
- Snapshot/checkpoint continuation restores exact population/RNG/runtime state and keeps pre-fork
  equality.
- Registry/result metadata has no missing-spec warning for any emitted AE3 observable.

Use focused tests first. For each production behavior, observe the relevant test fail for the
expected reason before implementing the smallest behavior. Record the red/green evidence in this
document or the testing document.

## Allowed files

Expected:

- `src-tauri/src/core/evolution_pathway.rs` (new)
- `src-tauri/src/core/mod.rs`
- `src-tauri/src/core/reference_world.rs`
- `src-tauri/src/core/experiment.rs`
- focused additions in `src-tauri/src/core/experiment_runner.rs` only if the generic runner needs a
  real contract fix
- AE feature documents under `docs/ai/`
- this goal document and `CLAUDE.md`
- small experiment JSON fixtures under `src-tauri/tests/fixtures/experiments/` only if generated by
  the real serializer and covered by round-trip/fingerprint tests

Do not change frontend, IPC, live Bevy systems, terrain/map/world generation, rendering,
save/load/migration, species clustering, or Creature Morphogenesis files.

## Verification loop

Run focused tests after each milestone, then finish with fresh output from:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo test --manifest-path src-tauri/Cargo.toml --test exotic_energy_zero_alloc_tests
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --lib
git diff --check
git status --short
```

Also run AI DevKit base/feature documentation lint if available. Do not claim a coverage percentage
unless a coverage tool actually produced it.

## Milestones

- **M0 — contract lock:** record baseline, named assumptions, exact code seams, focused failing tests.
- **M1 — pathway types:** genotype, explicit seeded variation, development, runtime state.
- **M2 — physiology:** transactional uptake/spend, explicit cost, finite performance.
- **M3 — generations:** opt-in population, reproduction-only frequency change, deterministic RNG.
- **M4 — observability/causality:** registry entries and full mechanism chain.
- **M5 — factorial paired evidence:** absent/present × cost tests over multiple same-seed pairs.
- **M6 — reconciliation:** current implementation/testing/planning status and honest remaining gaps.
- **M7 — independent audit handoff:** exact files, commands, counts, warnings, and uncompleted scope.

## Stop conditions

Stop and report rather than improvising if:

- an implementation requires changing `WorldLawSet` or manifest schema version;
- an existing AE1–AE2.5 test must be weakened or deleted;
- closed-EU compatibility or MU conservation cannot be retained;
- the cleanest path requires live ECS, persistence, IPC, UI, species, or map work;
- repository changes appear outside the allowed files;
- a gate can pass only by directly writing fitness/genotype/frequency from exotic presence.

## Progress log (2026-07-25, Claude Code implementation run)

### ⚠️ Stop condition triggered — concurrent third-party edits in the shared tree

**"Repository changes appear outside the allowed files" fired, and it was NOT caused by this goal.**
While this run was in progress, another actor (agent or user) edited the same crate, performing what
looks like a `thread_rng()`-removal / determinism refactor. Nothing was reverted, restored, staged or
cleaned — those files were left exactly as found.

| Measure | Goal start | Goal end |
|---|---:|---:|
| `git status --short` entries | 83 | 98 |
| Tracked modifications (` M`) | 18 | 31 |

Attribution of the +15:

- **+1 is this goal**: `src-tauri/src/core/evolution_pathway.rs` (new, untracked).
- **+14 are concurrent third-party work**, untouched by this run: 13 newly-modified tracked files —
  `src-tauri/src/core/{agent_systems,environmental_systems,resources,world_systems}.rs`,
  `src-tauri/src/evolution/{crossover,map_elites,mutation}.rs`,
  `src-tauri/tests/{challenger_meta_ai,evolution_robustness,hrrl,lineage_stress,map_elites,meta_ai_stress}_tests.rs`
  — plus one new untracked file `src-tauri/tests/sim_determinism_tests.rs`.

Evidence it is not this goal's work: `git diff src-tauri/src/evolution/crossover.rs` shows
`crossover_genotypes` gaining an `rng: &mut impl rand::Rng` parameter and dropping
`rand::thread_rng()` — a determinism change with no AE3 surface — and file mtimes interleave with
this run (`crossover.rs` 08:30:57, `sim_determinism_tests.rs` 08:38:30, `hrrl_tests.rs` 08:49:56,
versus `reference_world.rs` 08:50:19 and `evolution_pathway.rs` 08:52:39).

**Consequence for the audit:** every count below is a snapshot of a tree that contains that in-flight
third-party work. The suite was green at the moment of measurement, but an independent audit must
re-run on a quiesced tree before treating these numbers as final.

### Files this goal changed (the complete list)

| File | Status | Change |
|---|---|---|
| `src-tauri/src/core/evolution_pathway.rs` | new | The whole AE3 module: genotype, variation, development, runtime state, cohort physiology, population/reproduction, initial-condition seam |
| `src-tauri/src/core/mod.rs` | tracked, +1 line | `pub mod evolution_pathway;` (append-only) |
| `src-tauri/src/core/experiment.rs` | untracked (AE-owned) | 10 AE3 `ObservableSpec`s, `ExperimentError::InvalidPopulation`, AE3 manifest validation, 3 tests |
| `src-tauri/src/core/reference_world.rs` | untracked (AE-owned) | Population wiring, `step_population`, checksum/observables/budget/snapshot integration, 10 tests |
| `src-tauri/tests/exotic_energy_zero_alloc_tests.rs` | untracked (AE-owned) | 1 AE3 physiology zero-alloc test |
| `docs/ai/{planning,implementation,testing}/…` | docs | This log plus the reconciliation below |

`experiment_runner.rs` was **not** modified: the generic runner needed no contract fix — the paired
ensemble, checkpoint fork and result assembly all carried AE3 unchanged.

### Milestones

- **M0 — DONE.** Baseline measured on the real tree: 83 dirty entries / 18 tracked mods, lib
  **173 passed / 0 failed / 1 ignored**, zero-alloc **2 passed**, clippy **4 warnings all
  pre-existing** (`dynamic_fields.rs:180`, `scenario.rs:409`, `sim_clock.rs:66`, `sim_clock.rs:77`),
  `fmt --check` clean. Required reading completed; code seams read directly, not from prose.
- **M1 — DONE, test-first.** 6 tests written and observed failing (`cannot find type
  EnergyPathwayGenotype / DevelopedEnergyPathway / ExoticEnergyState`) before any production type
  existed. `EnergyPathwayGenotype` (legacy default disabled and zero-cost; every constructor
  normalizes non-finite/out-of-range input), seeded bounded `mutate`/`crossover` taking an explicit
  RNG, `DevelopedEnergyPathway::develop` (pure, one-time, environment-free), `ExoticEnergyState`.
  Strategy identity (`expressed`) is not mutable: legacy can never mutate or recombine into a free
  pathway.
- **M2 — DONE, test-first, with a real red→green on behaviour.** 6 tests observed failing
  (`cannot find type PathwayCohort`). Implemented `PathwayCohort::{uptake, metabolize,
  update_performance}` as three ordered, separately-testable steps, none of which may write `count`
  or `genotype`. **A behavioural assertion then genuinely failed** — `MU is held before the boundary`
  — because metabolic demand had been scaled to the uptake surface, so storage drained to empty every
  firing and `storage_capacity` was a dead trait. Demand is now a fraction of the *reserve*
  (`SPEND_FRACTION`), so storage settles near `intake / SPEND_FRACTION` and buffers across firings.
- **M3 — DONE, test-first.** 6 tests observed failing (`cannot find type
  ReferencePopulation{,Config}`). Fixed-capacity two-strategy population, structural config
  validation, and `reproduce` as the **only** writer of composition. Its RNG is a separate stream
  (`seed ^ RNG_DOMAIN`), so it never consumes or reorders the legacy ecology stream — which is what
  keeps AE-S01 bit-identical. Parental MU storage is released into `dissipated` at each generation
  boundary, so the ledger closes across replacement.
- **M4 — DONE, test-first.** 13 tests observed failing (missing `ae3::AE3_KEY_*`/`AE3_OBSERVABLE_IDS`,
  then `no method named population`). Initial-condition seam (13 documented version-1 scalar keys, an
  unknown `ae3.` key is **rejected**, not ignored), 10 registry specs, `InvalidPopulation`, manifest
  preflight that refuses an AE3 observable without a population, and the full ordered update in
  `ReferenceEvolutionWorld::step_population`.
- **M5 — DONE, but NOT a red/green cycle (recorded honestly).** The three factorial/AE-S14 tests
  passed on first execution because the mechanism they assert had already landed in M1–M4. Instead of
  claiming TDD process evidence, their discriminating power was **proved by deliberate mutation**:
  replacing the earned gain with a flat bonus keyed on `developed.expressed` (the exact forbidden
  "write fitness from pathway presence" shortcut) made **6 tests fail** —
  `ae306_absent_source_drives_a_costly_pathway_down_not_up`,
  `ae306_maintenance_cost_is_paid_even_when_no_mu_exists`,
  `ae307_performance_gain_requires_a_real_spend_not_mere_source_presence`,
  `ae310_factorial_absent_source_never_gives_a_pathway_a_free_advantage`,
  `ae310_factorial_present_source_advantage_flows_through_the_transaction`,
  `ae_s14_ae3_paired_multi_seed_reports_a_finite_effect_and_interval`. The mutation was reverted and
  the suite returned to green.
- **M6 — DONE.** Implementation, testing and planning docs reconciled; this log written.

### The 2×2 factorial as measured

| Exotic source | Pathway cost | Measured outcome |
|---|---|---|
| absent | zero | control: frequency only drifts (`|f − 0.5| < 0.15`), `exotic.uptake = 0`, performance delta exactly `0.0` |
| absent | positive (0.05) | frequency **falls below 0.5** and below the zero-cost cell; performance delta **negative** |
| present | zero | uptake > 0, spend > 0, performance delta > 0, frequency **rises above 0.5** |
| present | positive (0.05) | still rises, but **strictly lower than the zero-cost cell** — the trade-off is reported, not assumed |

AE-S14: `run_paired_ensemble` over **5 same-seed pairs** (2026–2030), declared factor
`laws.exotic_energy` only; 5/5 complete pairs preserved, finite paired mean delta / SD / SE / 95% CI
/ Cohen's *d_z*, interval excluding zero, and every EU observable showing **exactly** `0.0` paired
effect. Five seeds is stated as the ensemble size; **no statistical-confidence claim is made**.

### Verification (fresh, end of run)

| Command | Result |
|---|---|
| `cargo test --manifest-path src-tauri/Cargo.toml --lib` | **204 passed, 0 failed, 1 ignored** (173 baseline + 31 AE3) |
| `… --lib evolution_pathway` | 18 passed |
| `… --lib ae3` | 31 passed |
| `… --lib ae30` | 21 passed |
| `cargo test … --test exotic_energy_zero_alloc_tests` | **3 passed** (field, forcing, and the new AE3 physiology hot path) |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | clean (exit 0) |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --lib` | **4 warnings, all pre-existing, 0 in any AE module** (including the new `evolution_pathway.rs`) |
| `git diff --check` | clean (LF/CRLF advisories only) |

### Known gaps this run does NOT close

1. **`crossover` is implemented and tested but not wired into reproduction.** The two-strategy cohort
   model inherits clonally within a strategy with bounded mutation, because recombining a legacy and
   a pathway genotype would make "pathway frequency" undefined. AE-302's seeded-variation API is met;
   recombination as a *reproduction mechanism* is not yet exercised in the reference model.
2. **`ReferencePopulation::reproduce` allocates** (it clones genotype source-id `String`s). It runs
   once per generation, not per tick, so it is deliberately excluded from the zero-alloc test rather
   than silently counted as zero.
3. **Source-id incompatibility is unreachable through the manifest seam** — the pathway is always
   tuned to the law the run declares. The mismatch rejection is real but is covered only by a
   direct-constructor test (`ae305_uptake_is_impossible_without_expression_sensing_or_a_matching_source`).
4. **The population is a two-cohort aggregate, not live ECS entities**, exactly as the named slice
   assumptions permit. No species, no live Bevy, no persistence, no UI, no map claim.
5. ADR-0002 remains `proposed`; the local `1e-4` MU tolerance stays a **test** tolerance and was not
   promoted to product policy. No manifest/world-law schema version moved.

## M7 — independent Codex closure pass (2026-07-25)

The independent audit re-read the implementation instead of accepting the agent summary, found four
in-scope defects/gaps, and closed them without touching live Bevy, persistence, IPC, UI, species, or
map code:

1. `EnergyPathwayGenotype` now derives serde and has an exact JSON round-trip test.
2. `EnergyPathwayGenotype::crossover` returns `None` for two expressed parents with incompatible
   `EnergySourceId`s; matching-source deterministic crossover remains unchanged.
3. `evolution.births` is emitted as a cumulative counter and its registry metadata is now
   `Aggregation::Instant`, avoiding repeated summation of cumulative snapshots.
4. AE-S12 now covers a forcing-rooted full chain. A forcing receives ancestry only in the
   unambiguous case where the field was empty, renewable source rate was zero, and exactly one
   forcing injected MU. Mixed-origin fields conservatively retain the existing world-law/field
   parent because `CausalLedger` supports one parent.

The stale `ReferenceEvolutionWorld` module docs, implementation status, testing status, planning
status, and `CLAUDE.md` reading rules were reconciled. Historical AE2/AE2.5 statements are labeled as
historical instead of being allowed to contradict the current AE3 slice.

### Independent verification snapshot

| Command | Result |
|---|---|
| `cargo test --manifest-path src-tauri/Cargo.toml --lib` | **224 passed, 0 failed, 1 ignored** |
| `… --lib evolution_pathway` | **20 passed** |
| `… --lib ae3` | **34 passed** |
| `… --lib ae30` | **24 passed** |
| `cargo test … --test exotic_energy_zero_alloc_tests` | **3 passed** |
| `cargo fmt … --all -- --check` | clean |
| `cargo clippy … --lib --message-format=short` | exit 0; 4 pre-existing warnings, 0 in AE modules |
| `git diff --check` | clean (line-ending advisories only) |
| AI DevKit base lint | pass |
| AI DevKit feature docs | all required docs pass; branch-only check remains unmet because the shared dirty tree is not on the feature-named branch |

The broader `cargo test` command is **not claimed green**: its 224-test lib binary passed, then
Windows failed to start the unrelated `adversarial_challenger_tests` binary with
`STATUS_ENTRYPOINT_NOT_FOUND` before that integration harness ran. The isolated concurrent
`sim_determinism_tests` binary passed 15/15. This environmental integration failure is outside AE3
and was not hidden or reclassified as success.

## Honest completion language

Passing this goal permits only:

> “The headless reference slice demonstrates a deterministic causal pathway from exotic-energy
> availability, through a costly inherited pathway and reproductive selection, to a measured
> pathway-frequency difference across a same-seed ensemble.”

It does **not** permit:

- “Mana created a new species.”
- “The live Anima world now evolves differently.”
- “The UI can already explore every variable.”
- “The map/ecosystem is complete.”
