---
kind: agent-goal
agent: claude-code-background
feature: alternate-evolution-world-lab
title: Overnight Goal — Headless AE2.5 (runtime exotic-source forcings, effect size, fixtures)
status: active
created: 2026-07-25
owner: simulation-architecture
predecessor: 2026-07-24-claude-overnight-goal-ae1-ae2.md
---

# Overnight Goal — Headless AE2.5

## Why AE2.5 exists

AE1–AE2 landed and are verified (see the
[feature implementation notes](../implementation/2026-07-24-feature-alternate-evolution-world-lab.md)).
Three in-scope items from that slice remain **open**, and all three are backend-headless:

| Open item | Current status before this goal |
|---|---|
| **AE-209** — add/remove/pulse exotic **source interventions** | `[ ]` not started |
| **AE-210** — JSON fixtures + schema/size record | `[ ]` not started |
| **AE-S14 effect size** | **PARTIAL** — ensemble reports N/CI/failures but has **no control–treatment effect-size API** |

AE2.5 closes exactly those three and nothing else. It is deliberately *not* AE3: no genotype,
pathway, selection or speciation is introduced, so no adaptation/species claim becomes possible.

## Baseline at goal start (measured, not assumed)

- Branch `chore/init-and-frontend-test-fixes`, ahead 3. **No** branch/worktree/commit is created.
- **83** dirty working-tree entries (18 tracked-modified + untracked), all pre-existing user/agent work
  except the five AE files this feature owns.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` → **133 passed, 0 failed**.
- `cargo clippy --lib` → 4 warnings, **all pre-existing** (`dynamic_fields.rs:180`,
  `scenario.rs:409`, `sim_clock.rs:66`, `sim_clock.rs:77`); **0** in the AE modules.

### File ownership before this goal

Owned by this feature (safe to extend): `src-tauri/src/core/{experiment,experiment_runner,
exotic_energy,reference_world}.rs`, `src-tauri/tests/exotic_energy_zero_alloc_tests.rs`, and the
AE lifecycle docs. Only tracked file this feature has ever modified: `src-tauri/src/core/mod.rs`
(append-only `pub mod` lines). Everything else in the 83 entries is user/other-agent work and is
**not** touched.

## Hard scope

**In scope:** backend headless AE1–AE2.5 only.

**Out of scope (do not touch, do not claim):** UI/frontend, live Bevy, genotype/pathway/selection/
speciation, persistence/save migration, terrain/map/world generation, renderer, AE3+. **Map review is
not invoked and not claimed**. Its availability and required evidence are evaluated only when a
later map-scoped task runs the mandatory Animal Map Vision workflow.

## Hard invariants (carried from the contract)

- `WorldLawSet` is **immutable within a run**. A runtime exotic-source intervention is a **declared
  forcing / state effect on the field**, carrying a `CauseId` — it must **never** mutate the law or
  the law fingerprint (ER01). In this shipped headless slice, a law change requires a new genesis
  manifest/run; checkpoint branches add declared runtime forcings while retaining the law identity.
- **MU is not EU.** Every MU moved by `AddSource`/`Pulse` is booked in `sourced`.
  `RemoveSource` prevents only realizable renewable input, never moves stored MU, lowers
  `cum_sourced`, and records its counterfactual in `cum_source_suppressed`; it is not a
  `dissipated` sink. `ExoticEnergyBudget::balance_error` stays within tolerance and the closed-EU
  pools are untouched (ER04 / AE-S04 / AE-S05).
- Core names stay generic (`ExoticEnergy…`); **"Mana" is a display/fixture label only** (ADR-0002).
- Deterministic paths derive from declared seeds; **no `thread_rng()`**.
- Field hot loops keep **zero heap allocation** after initialization.
- Results/observables stay self-describing; no UI-side inference.
- No adaptation/ecotype/species claim may be derived from AE2.5 evidence.

## Ordered milestones

### M0 — Baseline audit and this document
Read CLAUDE.md, the experiment contract, ADR-0002, and the full feature lifecycle docs; inspect
`experiment.rs`, `experiment_runner.rs`, `intervention.rs`, `exotic_energy.rs`, `reference_world.rs`,
`causal.rs` and the tests directly. Record the measured baseline above.

### M1 — AE-209a: exotic-source intervention schema
A typed, versioned `ExoticIntervention` (add / remove / pulse) with region, time window, intensity,
curve and `CauseId`, plus a structured validator and canonical fingerprint contribution. Implemented
**alongside** the legacy `InterventionCommand` (legacy `intervention.rs` semantics unchanged).

### M2 — AE-209b: budgeted field forcing
Apply the forcings to `ExoticEnergyField` as declared source/sink transactions that keep the MU
ledger closed, with causal records rooted at a dedicated forcing `CauseId` (distinct from the
world-law cause), and a proof that the `WorldLawSet` fingerprint is unchanged.

### M3 — AE-209c: manifest / runner / checkpoint wiring
Carry the forcings through the manifest and the model factory, and offer them as a checkpoint
treatment channel (the "remove the source at generation G" experiment), preserving the existing
effective-treatment provenance contract.

### M4 — AE-S14: control–treatment effect size
A comparative effect-size API over ensembles (per-observable difference in means, a standardized
effect size, and an interval), preserving failed runs. Only after this may AE-S14 be described as
more than PARTIAL — and only to the extent actually implemented.

### M5 — AE-210: fixtures and schema record
Committed-in-tree JSON fixtures for a baseline manifest, a Mana treatment manifest and an invalid
manifest, with round-trip + fingerprint-stability tests and a recorded size/shape note.

## Acceptance gates

| Gate | Evidence required |
|---|---|
| **AE-S01** | Baseline (`exotic_energy = None`) checksum still equals the legacy scenario, bit-for-bit |
| **AE-S02** | Same manifest + seed + forcings → same checksum; replay stable |
| **AE-S03** | Law fingerprint **unchanged** by any runtime forcing; manifest fingerprint changes when forcings change |
| **AE-S04** | MU balance closes with forcings active (add/remove/pulse all booked) |
| **AE-S05** | Closed-EU pools unchanged by exotic forcings |
| **AE-S09** | Checkpoint fork with a source-removal forcing keeps identical pre-fork state |
| **AE-S12** (partial) | Causal chain: forcing `CauseId` → field change |
| **AE-S14** | Effect size + interval + preserved failures — status recorded honestly after M4 |
| Zero-alloc | Field hot loop still allocates 0 with forcings applied |

## Verification loop (every milestone)

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib <focused filter>
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo test --manifest-path src-tauri/Cargo.toml --test exotic_energy_zero_alloc_tests
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --lib
git diff --check
git status --short | wc -l   # ownership/dirty-count check
```

**TDD is mandatory**: write the focused test first, run it, observe the expected failure, then
implement the smallest contract-compliant behaviour and rerun. Record fresh counts only after gates.

## Progress log

Statuses here are updated **only after** the corresponding tests are observed failing, the fix lands,
and the gates rerun green. Nothing is marked done in advance.

> **Supervisor audit (2026-07-25, second AE2.5 pass).** The first pass shipped four real contract
> defects, all now corrected test-first. **M1–M5 below describe the first pass; read the audit section
> after them for what actually holds now.**

### Audit corrections (supersede the first-pass claims)

- **D1 — AE-S14 was NOT paired.** `compare_ensembles` computed an *independent-sample* pooled Hedges'
  *g* over two separately-run ensembles: it never verified same seeds, same seed order, or a validated
  factor diff, so it was not a same-seed causal design. It is now explicitly documented as a
  **descriptive helper that is not the AE-S14 gate**. The gate is the new
  `run_paired_ensemble` / `run_paired_ensemble_with_control` → `PairedEnsembleReport`.
- **D2 — `RemoveSource` was a drain.** It debited stored MU and credited `cum_dissipated` — a sink,
  not source removal. It now **suppresses the base renewable source** in its region/window, capped at
  the source contribution, and **never touches stored MU**; it lowers `cum_sourced` and records the
  counterfactual in `cum_source_suppressed`. Routing a `RemoveSource` through `apply_forcing` is now a
  no-op so the drain cannot return. A true drain would need a separate `DrainField` kind (not added).
- **D3 — cadence/spatial validation was missing.** A one-tick pulse on a non-ecology tick was accepted
  and would silently never apply. `amount` is now documented as **MU per ecology firing per affected
  cell**, and a window with no ecology firing inside the run is rejected. Grid applicability
  (`Cell` and full inclusive `Rect` contained; `Radius` finite/positive with in-grid centre and
  edge-overhang clipped) is validated at model construction and checkpoint pure preflight.
- **D4 — causal double attribution.** Forcing movement was counted *both* under its own `CauseId` and
  again inside the `CAUSE_EXOTIC_WORLD_LAW` delta (measured: world-law delta 506.16 absorbed a 512.0
  injection whose unforced baseline was −0.72). The field total is now re-baselined after forcings, so
  the world-law effect describes only source/decay/diffusion.
- **Checkpoint exotic channel — was missing.** `checkpoint_fork_with_exotic` adds a
  `treatment_extra_exotic` channel; `checkpoint_fork` delegates to it unchanged.

- **M0 — DONE.** Baseline measured (83 dirty entries; 133 lib tests pass; 4 pre-existing clippy
  warnings, 0 in AE modules). Required reading completed; this document created.
- **M1 — DONE** (3 tests, observed failing first as unresolved types).
  `ExoticInterventionKind{AddSource,RemoveSource,Pulse}`, `ExoticIntervention` (region/window/
  amount/curve/`CauseId`), `ExoticInterventionQueue` (validated, duplicate-id-rejecting,
  `(start_tick, id)`-ordered) in `exotic_energy.rs`. Legacy `intervention.rs` untouched — its
  `Region`/`Curve` are reused, its five-variant enum is **not** widened, so `scenario.rs` stays green.
- **M2 — DONE** (6 tests, observed failing first as a missing method).
  `ExoticEnergyField::apply_forcing` — region-scoped, `max_density`-bounded, never negative,
  allocation-free, and **fully booked**: adds/pulses credit `cum_sourced`, removals credit
  `cum_dissipated`, both by the *actual* delta, so the MU budget stays closed.
- **M3 — DONE** (5 tests, observed failing first as a missing manifest field).
  `ExperimentManifest.exotic_interventions` (serde-defaulted), structured validation
  (`InvalidExoticIntervention`, duplicate ids, and rejection when `exotic_energy = None`), canonical
  fingerprint contribution, `FactorDiff` path, `control_variant` stripping, and model wiring:
  `ExperimentModel::from_manifest` now takes `forcings` + `run_ticks`; forcings fire on the ecology
  band **before** the field's own dynamics and are recorded under their **own** `CauseId`.
  Verified: law fingerprint unchanged by forcings, manifest fingerprint changed, MU closed,
  EU byte-identical, replay deterministic. Full lib suite **147 passed / 0 failed**.
- **M4 — DONE** (3 tests, observed failing first as a missing function).
  `EffectSize` + `EnsembleComparison` + `compare_ensembles`: per-observable mean difference (in the
  observable's unit), **Hedges' *g*** (small-sample-corrected Cohen's *d*), a 95% normal-approx
  interval on the mean difference, per-side completed-run N, and **preserved failures from both
  sides**. Observables present on only one side are listed (`control_only`/`treatment_only`), never
  compared against a fabricated zero. Degenerate (zero-variance) comparisons report `g = 0`, never
  `NaN`/`±inf`. Verified against a real drought treatment (correct sign, |g| > 1, interval excludes 0)
  and against an EU-neutral exotic treatment (effect ≈ 0, as AE-S05 requires).
- **M5 — DONE** (3 tests + 1 `#[ignore]` regenerator, observed failing first as missing fixtures).
  `src-tauri/tests/fixtures/experiments/{baseline-no-exotic,mana-patchy-renewable,
  invalid-negative-source}.json`, generated by the **real serializer** (`ae210_regenerate_fixtures`,
  ignored by default) so they cannot drift from the schema. Tests cover parse → validate →
  round-trip → fingerprint stability, structured rejection of the invalid fixture, and a size record
  (all < 8 KiB; actual 911 / 1349 / 1543 bytes, total 3803). The baseline/Mana pair differs **only** in the
  exotic regime, so it is a clean AE-S08 control/treatment pair.

### Audit-pass status (verified 2026-07-25)

| Item | Status | Evidence |
|---|---|---|
| D1 paired AE-S14 | **DONE** | 8 `d1_*` tests |
| D2 RemoveSource = source suppression | **DONE** | 6 `d2_*` tests (2 obsolete drain tests deleted, not adjusted) |
| D3 cadence + grid validation | **DONE** | 2 `d3_*` tests + zero-alloc forcing test |
| D4 causal double attribution | **DONE** | 2 `d4_*` tests (both failed on the old code) |
| AE-209 checkpoint exotic channel | **DONE** | 4 `ck_*` tests |
| AE-210 fixture audit | **DONE** | 3 `ae210_m5_*` tests + size table in the testing doc |

Gates: `cargo test --lib` **173 passed / 0 failed / 1 ignored**; zero-alloc **2 passed**; `fmt --check`
clean; `git diff --check` clean; clippy **4 warnings, all pre-existing, 0 in AE modules**.

**AE-S14 and AE-209 are now complete for the headless reference slice.** AE3+, adaptation/speciation,
UI, persistence and map remain open/out of scope; the feature as a whole stays active/proposed.

### Independent Codex closure pass (2026-07-25)

After the Opus 5 audit, Codex independently tightened the remaining edge contracts:

- `Cell` and the complete inclusive `Rect` must fit the 16×16 reference grid; checkpoint grid
  applicability is pure-preflighted before even a generic model factory/RNG can run.
- `RemoveSource` is capped by the renewable MU that could actually enter current cell headroom.
  Saturated cells report zero suppression; the per-firing suppression buffer is consumed by
  `step`, so stale requests cannot leak into a later ecology tick.
- Runtime injection is applied before suppression, overlapping suppression attribution is cumulative
  and deterministic, and no causal record pretends prevented MU moved.
- The checkpoint treatment test now covers `AddSource`, `RemoveSource`, and `Pulse`, proves branch
  equality through the ecology tick before the treatment first fires, divergence on that firing,
  immutable law identity, own-cause attribution, and unchanged EU observables.
- The non-degenerate drought paired case explicitly requires a finite signed Cohen's `d_z`.
- Requirements, design, planning, implementation, testing, `CLAUDE.md`, fixture sizes, and Claude
  memory were reconciled so current/authoritative sections no longer conflict with shipped APIs.

These closure refinements have fresh end-state regression coverage. They are not claimed as a new
red/green TDD cycle for every sub-bullet; the earlier audit's test-first process evidence remains
separate from this independent verification pass.

## Morning handoff

See the "AE2.5" sections of
[implementation](../implementation/2026-07-24-feature-alternate-evolution-world-lab.md) and
[testing](../testing/2026-07-24-feature-alternate-evolution-world-lab.md) for the completed/partial/
blocked table, exact commands, fresh counts and the smallest safe next task. Planning statuses are
reconciled **only after** verified implementation.
