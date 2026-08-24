---
phase: requirements
feature: experiment-e2-evolved-brain-default
title: Requirements — E2, does a per-agent evolved brain change outcomes?
description: The fixed question, the directional hypotheses, and the metrics chosen from what the live adapter already emits
status: active
owner: maintainers
last_reviewed: 2026-07-27
decision: ../../decisions/ADR-0003-evolved-per-agent-brains.md
state: ../../planning/STATE_OF_THE_PROJECT.md
---

# Requirements — E2, does a per-agent evolved brain change outcomes?

> ## 🔒 Preregistration — registered before the run, and it contains no result
>
> This package was written and committed **before any E2 run existed**. It contains no measurement
> of the experiment, no observed direction, and no conclusion. Its parent commit is `96d54d9`; the
> git history is the evidence that the hypothesis, the metrics, N, T, the stopping rule, the
> materiality threshold, the seed semantics, the cost estimate and the smoke-seed exclusion all
> landed first.
>
> A later session (**E2-B**) runs the experiment. Nothing here may be edited to match what it finds.

## 1. The fixed question

> **Under the same world identity and the same seed, does a per-agent evolved brain
> (`evolved = true`) change outcomes relative to the default shared `BrainModel`, and in which
> direction?**

The question is fixed. It is not "are evolved brains better", and it is not "should the default be
flipped" — those are decisions the answer feeds, under the rule in
[the planning doc](../planning/2026-07-27-experiment-e2-evolved-brain-default.md) §6.

## 2. Why this experiment exists at all

[`STATE_OF_THE_PROJECT.md`](../../planning/STATE_OF_THE_PROJECT.md) §3.1 has carried "turn on
per-agent evolved brains by default" as a P0 for weeks, on the strength of an argument rather than a
measurement: every agent currently shares one `BrainModel`, which
[`MAP_AND_ML_UPGRADE_RESEARCH.md`](../../research/MAP_AND_ML_UPGRADE_RESEARCH.md) calls the largest
gap in the engine.

Two things changed on 2026-07-27 and together they make the measurement possible:

- **EB-S04 was re-baselined** (DEC-1 option 1, ADR-0003 decision 12). Until then the gate demanded a
  bit-identical match against a build that had no deterministic trajectory at all, so there was no
  baseline to compare a treatment against. There is one now.
- **`LiveExperimentAdapter` reached *headless adapter verified*** — the live Bevy schedule runs under
  a manifest, a clock, an intervention queue, a causal ledger and an observable registry, and
  replays to the same checksum from the same seed.

The engine can therefore be asked the question in a controlled way for the first time. What it still
cannot be asked is anything about the desktop app: see §6.

## 3. Every observable the harness can produce

Selection had to come from what already exists, so this is the complete list first, and the choice
second. Nothing was added for this experiment.

### 3.1 Registry observables — `ObservableRegistry::live_default()`

These are the eleven ids in `core::live_experiment::LIVE_OBSERVABLE_IDS`, emitted in this order by
`live_observables()`, every one of them supported by `LiveExperimentAdapter`. `source` is
`core::live_experiment::LiveExperimentAdapter` for all eleven.

| # | id | unit | scope | cadence (period) | aggregation | valid range | conservation |
|---|---|---|---|---|---|---|---|
| 1 | `plants` | EU | World | ecology (60) | Instant | [0, 1e300] | ClosedEu |
| 2 | `detritus` | EU | World | ecology (60) | Instant | [0, 1e300] | ClosedEu |
| 3 | `live.animals_eu` | EU | World | ecology (60) | Instant | [0, 1e300] | ClosedEu |
| 4 | `live.closed_eu_total` | EU | World | ecology (60) | Instant | [0, 1e300] | None |
| 5 | `live.agent_count` | individuals | World | physics (1) | Instant | [0, 1e300] | None |
| 6 | `live.herbivore_count` | individuals | World | physics (1) | Instant | [0, 1e300] | None |
| 7 | `live.predator_count` | individuals | World | physics (1) | Instant | [0, 1e300] | None |
| 8 | `live.food_items` | individuals | World | physics (1) | Instant | [0, 1e300] | None |
| 9 | `live.standing_crop` | EU | World | ecology (60) | Instant | [0, 1e300] | None |
| 10 | `live.mean_agent_energy` | EU | World | ecology (60) | Instant | [0, 1e300] | None |
| 11 | `live.season_phase` | fraction | World | ecology (60) | Instant | [0, 1] | None |

`plants` and `detritus` are deliberately shared with the reference registry, at the same unit and
the same conservation role — those are the shared-law quantities, and sharing the id is what lets a
result from one path be compared *in direction and meaning* with the other.

### 3.2 Provenance and checksum fields — **not** observables

These travel with a run and describe it, but none of them is a measured quantity of the world and
none may be used as a metric. Listed so the distinction is explicit rather than assumed.

| Source | Fields |
|---|---|
| `RunProvenance` | `experiment_id`, `run_id`, `parent_run_id`, `fork_tick`, `seed`, `manifest_fingerprint`, `law_fingerprint`, `registry_fingerprint`, `model_version`, `build_id` |
| `RunResult` | `status`, `final_checksum`, `series`, `ledger`, `exotic_budget`, `observable_specs`, `warnings` |
| `PairedEnsembleReport` | `control_manifest_fingerprint`, `treatment_manifest_fingerprint`, `control_law_fingerprint`, `treatment_law_fingerprint`, `registry_fingerprint`, `declared_factors`, `seed_order`, `control_only`, `treatment_only` |

`final_checksum` is the sharpest temptation here — it is a single number that differs between arms
and looks like an outcome. It is a content fingerprint of world state, not a measurement of
anything; two worlds differing by one float in one agent's position produce unrelated checksums. It
is used in this experiment for exactly one purpose: proving a run replayed.

### 3.3 What is **not** available, and was not added

- No behavioural-diversity observable (policy variance across agents). `EB-S11` measures that, but
  through a bespoke test harness, not through the registry — so it is not reachable from a manifest.
- No archive-coverage observable. It needs the evolution thread, which the adapter does not run.
- No per-agent or per-lineage observable at all: every live spec is `ObservableScope::World`.

Adding one was considered and rejected. An observable added in the same package that preregisters
the experiment is an observable chosen with the hypothesis in hand, and the eleven above are enough
to answer the question as asked.

## 4. Metrics

**Primary — `live.mean_agent_energy`, final value at tick T, paired per seed.** Continuous (so it
has an effect size), and directly downstream of both channels ADR-0003 opened: the `feed_intent`
gate that decides whether an agent takes energy from food it touches, and the locomotion parameters
that decide what movement costs. It is the outcome most tightly coupled to the mechanism under test.

**Secondary — `live.agent_count`, `live.animals_eu`, `live.predator_count`.** Survivorship of the
founding cohort, the cohort's total energy, and the predator sub-population that `attack_intent`
gates. Reported with full statistics but not decisive on their own.

**Harness integrity — `live.closed_eu_total`.** Reported for both arms and never treated as an
outcome. A paired difference past the materiality threshold is a **finding about the
implementation** — the treatment would be moving energy the ledger does not account for — and is
filed as a defect, not as evidence about brains.

**All remaining registry observables are reported** with the same statistics, so a reader can see
everything the run produced rather than the subset that supports a story. They carry no decision
weight.

## 5. Hypotheses, with the mechanism that predicts them

The treatment's brains are **randomly initialised and never selected**: the adapter drains epoch
statistics and never applies an evolutionary replacement, so the founding brains are the only brains
a run ever has. The actor head is a sigmoid, so a fresh network's gate outputs sit near 0.5;
`ACTION_GATE_THRESHOLD` is `0.5`; and the legacy path pins every gate fully open at `1.0`.

| id | role | observable | direction | reasoning |
|---|---|---|---|---|
| **H1** | primary | `live.mean_agent_energy` | **negative** | `feed_intent` near the threshold means an agent declines food it would previously have eaten; intake falls |
| **H2** | secondary | `live.agent_count` | **non-positive** | lower intake cannot raise survivorship |
| **H3** | secondary | `live.animals_eu` | **negative** | follows H1 and H2 |
| **H4** | secondary | `live.predator_count` | **two-sided** | `attack_intent` closing both relieves predation on prey and starves predators; the net sign is not predictable from the mechanism, so it is declared two-sided rather than guessed |
| **H5** | integrity | `live.closed_eu_total` | two-sided, no decision weight | see §4 |

H1 being directional and **negative** is worth stating plainly: this preregistration predicts that
switching per-agent brains on, *in this harness*, makes the population worse off. That is not an
argument against ADR-0003 — it is the expected cost of individuality without selection, and §6 is
where that limit is drawn.

## 6. What this experiment can and cannot establish

**It can** establish whether, and in which direction, the shipped `evolved = true` path changes the
outcome of the live schedule under a controlled same-seed comparison.

**It cannot** establish that per-agent brains help or hurt *evolution*, because there is no
evolution in the harness. `check_epoch_completion_system` sends epoch statistics into a channel the
adapter drains; `apply_staggered_evolution_system` finds an empty queue. No agent is ever replaced,
so nothing is ever selected, and the entire argument of ADR-0003 — that behaviour must be heritable
and variable for quality-diversity to mean anything — is untestable here.

**It says nothing about the desktop app.** The status of the live world is *headless adapter
verified*, and this experiment does not change it. No full desktop-app run is made, the adapter
refuses `laws.exotic_energy`, it has no AE3 reference population, and numerical agreement with
`ReferenceEvolutionWorld` is never claimed. Nothing in this package may be quoted as "the live world
is experiment-ready".

## 7. Success criteria for E2-A (this package)

- [x] The question, hypotheses, metrics, N, T, thresholds, seed semantics and cost estimate are
      committed **before** any run exists.
- [x] Metrics are chosen only from §3.1, and the registry/provenance distinction is explicit.
- [x] Control and treatment exist as machine-readable manifests a later session parses rather than
      retypes.
- [x] The smoke seed is named and is *mechanically* incapable of entering the analysis.
- [x] The decision rule for ADR-0003 is written down, including what a null result means.
- [x] Blocking preconditions are recorded as findings rather than worked around.
- [x] No experiment, trial, pilot or smoke run was performed. See
      [the testing doc](../testing/2026-07-27-experiment-e2-evolved-brain-default.md) §4.
