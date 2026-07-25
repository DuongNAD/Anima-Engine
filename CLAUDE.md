# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Anima-Engine is a real-time, GPU-accelerated Artificial Life & Evolution simulator built as a **Tauri v2 desktop app**. Backend is Rust (Bevy ECS + Burn ML) running a 60 FPS background simulation thread; frontend is TypeScript/React/Vite communicating over Tauri IPC. Cargo workspace rooted at `src-tauri/` (the `anima-engine` package, plus `crates/anima-domain` holding the world laws); `tests/` is a second npm package with its own `node_modules`.

## Commands

Frontend (run from repo root):
- `npm run dev` — Vite dev server, fixed port **5173** (`strictPort`).
- `npm run build` — `tsc && vite build`; typechecks first, builds two entries (`index.html`, `landscape.html`).
- `npm run test` — Vitest over `src/**`.
- `npm run test:frontend` — Vitest over the dedicated `tests/` suite (`--root tests`). This is the suite handoff docs use.
- `npm run lint` — ESLint (flat config in `eslint.config.js`). Errors block; legacy `any` and unused-var issues are warnings only.

- `npm run tauri:dev` / `npm run tauri:build` — the desktop app. **Use these, not bare `tauri dev`/`tauri build`:** they pass `--features desktop`, and `tauri.conf.json` has no field for Cargo features, so the bare commands ship a `default = []` binary with no Neo4j lineage, no cross-shard migration and the CPU learner. All three have fallbacks, so nothing fails loudly.

Backend (run from `src-tauri/`):
- `cargo test --features desktop` — unit + integration tests, across both workspace packages (`default-members` in `Cargo.toml`; without it Cargo would run the root package only). **Pass `--features desktop`, which is what CI runs:** seven test files carry a crate-level `#![cfg(feature = "networking")]` or `#![cfg(feature = "ml-wgpu")]`, so a bare `cargo test` compiles them to empty binaries that report `running 0 tests` and exit 0. That silently skipped 1,877 lines of migration, cross-shard and GPU-fallback coverage. `node scripts/check_test_targets.mjs <captured-output>` fails when any target runs zero tests.
- `cargo clippy` — Rust linter (ships with the toolchain).
- `cargo build --release --features desktop` — required before Playwright E2E (it expects the release binary in `src-tauri/target/release/`).

There is no Makefile. CI is `.github/workflows/ci.yml` (Rust on `windows-latest`, frontend on `ubuntu-latest`); it runs the gates above plus a `cargo tree` check that a default build really excludes the gated crates. `tsc` strict mode runs on build; ESLint (frontend) and clippy (backend) are the linters. Edited `.rs` files are auto-formatted by rustfmt via a PostToolUse hook (`.claude/hooks/rustfmt-on-edit.ps1`).

## Gotchas

- **Test-mode mock aliasing.** When Vitest runs (`mode === 'test'`), `three` and `@react-three/fiber` are aliased to mocks in `tests/mocks/`. These aliases are duplicated in `vite.config.ts`, `tsconfig.json` paths, and `tests/vitest.config.ts` — keep them in sync. Tests run under jsdom; WebGL/R3F are mocked. `PixiViewport.tsx` falls back to a Canvas 2D path under Vitest to avoid jsdom WebGL crashes.
- **Zero-heap-allocation hot loop.** Simulation tick systems (physics, CPG, collision) must not allocate on the heap; use pre-allocated buffers. Tests assert `allocs == 0` — don't introduce allocations in tick paths.
- **Two Vitest configs.** Root `vite.config.ts` includes `src/**`; `tests/vitest.config.ts` includes `tests/frontend/**` with `testTimeout: 15000` and re-pins `react`/`react-dom`.
- **Running the full Bevy/Tauri backend (`npm run tauri dev` / `cargo run`) is heavy and has crashed the dev machine.** For 3D model / rabbit work, serve `rabbit-standalone/` statically instead (e.g. `py -m http.server 8000`).
- Ignore build-artifact noise: `src-tauri/target_*` dirs, `*.log`, `err*.txt`, `out*.txt`, and the root `.agents/` / `$.agents/` vendored cargo dirs.

## Required reading for creature spawn and morphology

Before changing genotype, phenotype, genesis, birth, epoch replacement, save/load,
migration, pigment, creature physics geometry, live-agent rendering, **agent brains**, or the
**action space** (the pheromone / attack / feed gates), read in order:

1. `docs/reference/CREATURE_DEVELOPMENT_CONTRACT.md`
2. `docs/decisions/ADR-0001-creature-development-lifecycle.md`
3. `docs/explanation/CREATURE_MORPHOGENESIS.md`
4. `docs/planning/CREATURE_MORPHOGENESIS_PLAN.md`
5. `docs/decisions/ADR-0003-evolved-per-agent-brains.md` (accepted) — for anything touching
   `BrainGenotype`, `AgentBrain`, `ActionGates`, `BrainPolicy`, inference or lifetime learning

Hard rules:

- Do not read environment inside `decode_genotype` and apply development to every
  call-site. Restore and migration preserve a serialized `DevelopedPhenotype`.
- Development happens once at genesis/birth; ECS spawning consumes a phenotype.
- `LocomotionMedium` is a creature trait, not a value inferred from the destination
  cell during legality checking.
- Keep S43 for Red-Queen predator/prey coevolution. Local adaptation uses CM-S11
  reciprocal-transplant evidence.
- Current epoch evolution is `EvolutionaryReplacement`, not biological reproduction.
- Use code symbols as anchors and re-read current files before implementation; line
  numbers in archived drafts are historical.
- Files in `docs/archive/` are superseded and must not be used as implementation plans.

Hard rules from ADR-0003 (brains and action space) — each one is a trap that produces code
that runs, returns finite numbers and is silently wrong:

- **No Lamarck.** `AgentBrain.learned` is runtime state and must never be written back into
  `.genotype`. Reproduction copies the genome; what an individual learned dies with it.
- **Restore and migration carry the brain they were given**, never roll a new one (D01). Only
  genesis and evolutionary replacement create brains.
- **The legacy path is the default and must stay reachable.** `AgentBrain` absent, `ActionGates`
  absent-or-open, `ANIMA_EVOLVED_BRAINS` / `ANIMA_LIFETIME_LEARNING` unset and
  `brain_metabolic_cost = 0.0` all mean "behave as before". A missing `ActionGates` reads as fully
  **open**, never shut — the other default would silently stop an agent eating.
- **Any new energy charge goes into `total_cost` in `metabolic_decay_system`**, never a separate
  deduction. Only `total_cost` flows through `respired` into the detritus pool, so a separate
  subtraction leaks EU while looking entirely reasonable.
- **`BrainGenotype`'s weight layout is `w[out * fan_in + in]` — the transpose of Burn's
  `[d_input, d_output]`.** Copy flat weights across without transposing and the network still runs.
  Use `ActorCriticModel::from_flat_weights`, which transposes, and keep the EB-S02 parity gate green.
- **There is one A2C objective, `simulation_loop::a2c_loss`, and its actor coefficient is `+td`.**
  The shared learner previously used `(a − â)²·(−td)`, which drove the policy away from actions that
  beat expectation; it now matches `brain_genotype::learn_step`. `a2c_loss_direction_tests` fails if
  the sign is reinverted. If you add a third learner, call `a2c_loss` rather than restating it.
- Numerical code needs **two** kinds of test: a gradient check against finite differences catches a
  wrong derivative, but passes for a wrong objective too. Pair it with a behavioural assertion.

## Required reading for alternate world laws and evolution experiments

Before changing world genesis conditions, simulation laws, scenario/experiment schemas, exotic
energy (“mana”), energy pathways, lineage/species diagnostics, causal observability or World Lab UI,
read in order:

1. `docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md`
2. `docs/decisions/ADR-0002-world-laws-and-exotic-energy.md`
3. `docs/explanation/ALTERNATE_EVOLUTIONARY_REGIMES.md`
4. `docs/ai/requirements/2026-07-24-feature-alternate-evolution-world-lab.md`
5. `docs/ai/design/2026-07-24-feature-alternate-evolution-world-lab.md`
6. `docs/ai/planning/2026-07-24-feature-alternate-evolution-world-lab.md`
7. `docs/ai/implementation/2026-07-24-feature-alternate-evolution-world-lab.md`
8. `docs/ai/testing/2026-07-24-feature-alternate-evolution-world-lab.md`
9. For AE2.5 continuation/audit only:
   `docs/ai/planning/2026-07-25-claude-overnight-goal-ae25.md`
10. For the AE3 headless pathway/selection slice and its independent audit:
    `docs/ai/planning/2026-07-25-claude-overnight-goal-ae3.md`

Authority rule: current code plus fresh tests win over prose; then use the authoritative/current
sections in implementation and testing, then planning status, then the requirements/design target.
Sections explicitly marked `SUPERSEDED`, `FIRST PASS`, or historical are evidence of past decisions,
not implementation instructions. Re-read symbols before editing; do not infer current APIs from a
historical sketch.

Hard rules:

- `WorldLawSet` is immutable within a run. In the shipped headless slice, a changed law requires a
  new genesis manifest/run; a checkpoint branch changes declared runtime interventions/forcings
  with `CauseId`s, never the law fingerprint.
- Core code uses generic `ExoticEnergy`; “Mana” is a scenario/UI display name.
- MU is not EU. Keep the accepted closed-EU contract and audit exotic sources/sinks separately.
- Exotic energy must not rewrite genotype, species id, population or fitness directly. Effects go
  through field → pathway/cost → performance → survival/reproduction → trait/lineage change.
- `exotic_energy=None` is the baseline compatibility and rollback path.
- Do not call a visual morph or one MAP-Elites cell a species. Follow AE-S11/AE-S14 evidence gates.
- Start implementation at AE1 manifest/runner, then AE2 field/budget. Do not start by adding Mana
  fields to every organism or by building the UI.
- AE3 lives in `core/evolution_pathway.rs` and is **opt-in**: with no `ae3.` initial-condition key the
  population is disabled and the AE1–AE2.5 path stays bit-identical. Performance must be derived from
  a completed uptake→spend transaction (`state.last_spent_mu`), never from `expressed`/`has_exotic`.
  Only `ReferencePopulation::reproduce` may write cohort counts or genotypes; sensing, uptake,
  metabolism and performance accounting may not. The population uses its own seeded RNG stream and
  must never draw from the ecology stream, or the AE-S01 baseline checksum diverges.
- AE3 observables are emitted only when the population exists; a manifest requesting one without a
  population must fail preflight rather than report a zero.
- `EnergyPathwayGenotype::crossover` accepts only matching expressed source ids; an incompatible
  source pair returns `None`. Do not silently combine source-specific traits.
- `evolution.births` is cumulative and therefore has `Aggregation::Instant`, not `Sum`.
- Causal provenance may root at a forcing only when the run can prove that forcing was the sole
  effective MU origin. Mixed-origin fields keep the conservative world-law/field parent until the
  ledger supports multiple parents.
- `Scenario`/`ReferenceEcosystem` remains the legacy headless machinery;
  `ReferenceEvolutionWorld` proves AE3 only for the opt-in aggregate reference population. Do not
  claim the live Bevy world is experiment-ready until its deterministic adapter and persistence
  gates pass.

## Code style

- No Prettier — match surrounding style. TS strict flags and the `@/*` alias live in `tsconfig.json`.

## Tauri IPC contract

Commands and events are documented in `PROJECT.md` ("Interface Contracts") — read it before changing the IPC surface. Commands include `get_simulation_status`, `toggle_simulation`, `get_map_elites_grid`, `get_pheromone_grid`, `get_lineage_graph`, `save_simulation_state`/`load_simulation_state`; events include `simulation-tick`, `map-elites-update`, `pheromone-update`, `chronicle-event`, `migration-event`.
