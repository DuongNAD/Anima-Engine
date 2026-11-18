# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Start here

`docs/planning/STATE_OF_THE_PROJECT.md` is the living status doc: what was last **measured** green (not what a doc claims), what rung each subsystem has reached, and the prioritized backlog with file-level anchors and a definition of done per item. Read it before picking up work, and update it when you finish something — it is what the next session reads first. `main` was fast-forwarded to the trunk `feat/ecological-selection` on 2026-11-16 (it was an ancestor with no commits of its own), but pull requests still target `feat/ecological-selection`; start with the banner at the top of STATE §1; the newest measurements (card R0: a full re-measurement on Windows 11 x64 at trunk `46e25d5`, cross-checked against CI, 2026-11-18) are in STATE §1.i, and STATE §3.00 opens with a one-table progress board for the stages of the 13/11 direction plan. Both branches are protected since 2026-11-17: every change, documentation included, lands through a pull request whose `Rust (src-tauri)` and `Frontend` jobs are green, and a direct push to the trunk is rejected. If you were dispatched as one of several coding agents, or are about to dispatch them, read `docs/how-to/AGENT_WORKFLOW.md` next: the card queue, the owner's recorded decisions and the authorization lines live there.

This file (CLAUDE.md) holds the rules that do not change between sessions. That one holds the state that does.

## What this is

Anima-Engine is a real-time, GPU-accelerated Artificial Life & Evolution simulator built as a **Tauri v2 desktop app**. Backend is Rust (Bevy ECS + Burn ML) running a 60 FPS background simulation thread; frontend is TypeScript/React/Vite communicating over Tauri IPC. Cargo workspace rooted at `src-tauri/` (the `anima-engine` package, plus `crates/anima-domain` holding the world laws); `tests/` is a second npm package with its own `node_modules`.

## Commands

Frontend (run from repo root):
- `npm run dev` — Vite dev server, fixed port **5173** (`strictPort`).
- `npm run build` — `tsc && vite build`; typechecks first, builds two entries (`index.html`, `landscape.html`).
- `npm run test` — Vitest over `src/**`.
- `npm run test:frontend` — Vitest over the dedicated `tests/` suite (`--root tests`). This is the suite handoff docs use.
- `npm run lint` — ESLint (flat config in `eslint.config.js`). Errors block; `any`, unused-var and React Compiler issues are warnings. `node scripts/eslint_ratchet.mjs` holds the baseline at **0**, so when the ratchet was green on the commit before yours, a warning means your commit introduced it. Fix it there; the baseline may only ever be lowered, and it is already at the floor. Whether it is green *today* is a measurement, and measurements do not live in this file. Current status for every gate: [`STATE_OF_THE_PROJECT.md` §1](docs/planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền), which is the only place a live count is maintained.
- `npm run check:csp` — validates the shipped `dist/` against the CSP declared in `tauri.conf.json`: no external origins, no inline `<script>` bodies, hardening directives present. It checks *artifacts against policy*; it cannot prove the app boots under that policy, which needs `npm run tauri:dev` and a human.
- `npm run check:bundle` — per-chunk and total JS budgets. Raising a budget is a decision: change the number in the same commit that causes the growth, and say why.
- `node scripts/gen_notice.mjs [--check]` — regenerates `NOTICE` from `cargo tree --features desktop -e normal` plus npm `dependencies`. `--check` fails when it is stale.
- `npm run check:docs` — docs link check plus `scripts/check_docs_metadata.mjs`: required front matter, allowed `status`/`review_cycle` values, a README index in every `docs/` directory, and every document linked from an index. Both scripts also read untracked files, so a new document is checked before it is staged. The metadata checker runs its own negative controls first and exits 2 if one of them passes. The rules it enforces are in `docs/governance/DOCUMENTATION_POLICY.md` ("Metadata tối thiểu"); change the two together. `--report` lists overdue reviews, which are reported but never fail the run.
- `npm run gen:world-manifest` — regenerates `map_manifest.json` **and** `artifacts/world_256.anmw` together, with a real SHA-256. Never hand-edit that manifest; `tests/frontend/mapManifestEvidence.test.ts` hashes the bytes it names.

**`esbuild` is not installed** — Vite 8 bundles with rolldown/oxc (the same reason `build.minify` must stay `true`). Any script invoking `esbuild` fails with `'esbuild' is not recognized`; `gen:manifest` did, silently, for as long as it existed. Run TypeScript scripts with `node scripts/run_ts.mjs <file.ts> [args]`.

- `npm run tauri:dev` / `npm run tauri:build` — the desktop app. **Use these, not bare `tauri dev`/`tauri build`:** they pass `--features desktop`, and `tauri.conf.json` has no field for Cargo features, so the bare commands ship a `default = []` binary with no Neo4j lineage, no cross-shard migration and the CPU learner. All three have fallbacks, so nothing fails loudly.

Backend (run from `src-tauri/`):
- `cargo test --features desktop` — unit + integration tests, across both workspace packages (`default-members` in `Cargo.toml`; without it Cargo would run the root package only). **Pass `--features desktop`, which is what CI runs:** eight test files carry a crate-level `#![cfg(feature = "networking")]` or `#![cfg(feature = "ml-wgpu")]` **and** a matching `[[test]] required-features` entry, so a bare `cargo test` does not schedule them at all rather than compiling them into empty binaries that report `running 0 tests` and exit 0. That silently skipped 1,877 lines of migration, cross-shard and GPU-fallback coverage. `tests/feature_gated_targets_tests.rs` keeps the two halves of that declaration from drifting.
- `node scripts/check_test_targets.mjs <captured-output> --profile {default|desktop}` — the test-target **policy** gate. Every zero-test target and every `#[ignore]`d test must be named with a reason in `scripts/test_target_policy.mjs`; anything unlisted fails, and so does a listed entry that no longer applies, so the allow-list cannot silently grow or rot. With `--profile` it also checks that the feature-gated targets are absent under `default` and running under `desktop`. **Capture into one file with both streams in write order:** on macOS/Linux a plain `cargo test ... > out.txt 2>&1` in bash/zsh does that; on Windows use `cmd /c "cargo test ... > out.txt 2>&1"`, **not a PowerShell pipeline** — PowerShell buffers cargo's two streams separately, which reorders the `Running` lines away from their counts and makes the script under-report targets.
- `cargo clippy` — Rust linter (ships with the toolchain).
- `cargo build --release --features desktop` — required before Playwright E2E (it expects the release binary in `src-tauri/target/release/`).

Dependency advisories are gated: `cargo audit` (backend, config in `src-tauri/.cargo/audit.toml`) and `npm run check:audit` (`scripts/check_npm_audit.mjs`, both npm packages, threshold `high`). Neither gate has a blanket exemption. Each Rust advisory ignored in `audit.toml` is a lockfile-only entry under an optional `burn` dependency no enabled feature compiles, with the verifying command recorded beside it. An npm advisory is tolerated only when `scripts/npm_audit_policy.mjs` names it with a reason, and the gate re-checks on every run that it is absent from `npm audit --omit=dev` (what ships) and fails on a stale entry. Do not add to either list without recording the check that shows the code cannot reach a user.

**burn is at 0.22.1** (PR #127, merged 2026-11-18; `docs/decisions/ADR-0009-burn-022-flex-wgpu-upgrade.md`, proposed). The CPU path is `burn-flex` through an explicit `Device::flex()`, and it is the deterministic reference. The GPU path is `burn-wgpu`, compiled only under `ml-wgpu` (part of `desktop`) and used at runtime only when `ANIMA_USE_GPU` asks for it (`resources::gpu_backend_requested`). Never write `Device::default()`, which picks the GPU whenever `ml-wgpu` is compiled, and never enable burn's `cpu` (CubeCL JIT) or a BLAS feature. `burn` is declared with `default-features = false` and an explicit feature list; the defaults bring back autotune persistence (a Turso/SQLite tree), fusion, `burn-train` and `burn-dataset`, and with them licence and advisory work, so widening that list is a decision with its own ADR line. Snapshot schema is 8: shared-learner records are burnpack, and a schema-7 world restores with a fresh shared learner and a `Schema7LearnerReset` warning. The upgrade moved trajectories of worlds that use the shared learner, so `LIVE_MODEL_VERSION` is `"live-bevy-world/2"`; under ADR-0008 a checksum from `/1` is never compared with one from `/2`. `bevy_ecs` is still at 0.13 with no advisory pressure.

There is no Makefile. CI is `.github/workflows/ci.yml` (Rust on `windows-latest`, frontend on `ubuntu-latest`); it runs the gates above plus a `cargo tree` check that a default build really excludes the gated crates. `tsc` strict mode runs on build; ESLint (frontend) and clippy (backend) are the linters. Edited `.rs` files are auto-formatted by rustfmt via a PostToolUse hook (`.claude/hooks/rustfmt-on-edit.mjs`, run with `node` so it works on macOS, Linux and Windows). Until 2026-11-14 the hook called `powershell.exe` and did nothing off Windows, so `.rs` files edited on a Mac before then may not be formatted — run `cargo fmt --check` from `src-tauri/` rather than assuming they are. The hook only loads when a session starts.

## Gotchas

- **Test-mode mock aliasing.** When Vitest runs (`mode === 'test'`), `three` and `@react-three/fiber` are aliased to mocks in `tests/mocks/`. These aliases are duplicated in `vite.config.ts`, `tsconfig.json` paths, and `tests/vitest.config.ts` — keep them in sync. Tests run under jsdom; WebGL/R3F are mocked. `PixiViewport.tsx` falls back to a Canvas 2D path under Vitest to avoid jsdom WebGL crashes.
- **A mock invoked with `new` needs a `function`, not an arrow.** Under `@vitest/spy` 4 a mock constructed with `new` reaches its implementation via `Reflect.construct`, and an arrow has no `[[Construct]]` slot — `vi.fn().mockImplementation(() => ({...}))` throws `TypeError: … is not a constructor` from inside the spy. Vitest 1 called the implementation plainly, so the arrow form worked there. This bites every `vi.mock('pixi.js', …)`; `PIXI.Application`, `Graphics`, `Container` and `Sprite` are all `new`ed by `PixiViewport.tsx`.
- **Vite 8 bundles with rolldown/oxc and does not ship esbuild.** Naming `"esbuild"` in `build.minify` routes through the deprecated `transformWithEsbuild` and fails with `Cannot find package 'esbuild'`. Leave `minify: true`.
- **Zero-heap-allocation hot loop.** Simulation tick systems (physics, CPG, collision) must not allocate on the heap; use pre-allocated buffers. Tests assert `allocs == 0` — don't introduce allocations in tick paths.
- **Two Vitest configs.** Root `vite.config.ts` includes `src/**`; `tests/vitest.config.ts` includes `tests/frontend/**` and re-pins `react`/`react-dom`. Both set `testTimeout: 15000`: the first `render(<App />)` in a file pays for the whole lazy module graph plus world generation under jsdom — ~4s on a dev machine, ~11s on a GitHub ubuntu runner — while later renders in the same file are ~150ms. The root config lacked it and the src suite timed out on CI while passing locally.
- **Running the full Bevy/Tauri backend (`npm run tauri dev` / `cargo run`) is heavy and has crashed the dev machine.** Prefer the headless paths — `cargo test`, the Vitest suites — and launch the app only when a human has to see it. (`rabbit-standalone/`, the static page older notes suggest serving for 3D/rabbit work, was deleted at `fddbdee`; the only rabbit visualizer left is `playground/RabbitVisualizer.tsx`, and `playground/` was declared scratch by QĐ-D on 2026-10-31.)
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

**Target versus code — read before grepping.** The development types these rules name —
`DevelopedPhenotype`, `EnvSample`, `RuntimeState`, `LocomotionMedium`, `develop_at_birth` — are the
accepted *target* of ADR-0001 and **do not exist in the code** on this branch or on `origin/main`
(checked 2026-11-14; `DevelopedPhenotype` exists only on `origin/fix/ht0-5c-sim-semantics`). Today
genotype → ECS still goes through `decode_genotype` at seven call sites (counted 2026-11-17; the
contract lists them): genesis in `SimulationEngine::start` and in `live_experiment::genesis`,
`SpawnGenotypeCommand::apply`, `SpawnOffspringCommand::apply`, `spawn_serialized_agent`,
`SpawnMigrationCommand::apply` and `RehydrateCommand::apply`. So a missing type does not mean a rule is obsolete, and it is not a
licence to add the type ad hoc: introducing it is planned work in
`docs/planning/CREATURE_MORPHOGENESIS_PLAN.md`. The contract's "Trạng thái triển khai" table says
which names exist.

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
- Core code uses the generic `core::exotic_energy` module (`ExoticEnergyField`, `EnergySourceId`);
  “Mana” is a scenario/UI display name. There is no type called `ExoticEnergy`.
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
  `ReferenceEvolutionWorld` proves AE3 only for the opt-in aggregate reference population.
- **The live Bevy world now has a deterministic experiment adapter, and its gates pass headlessly**
  — `core/live_experiment.rs` (`LiveExperimentAdapter: ExperimentModel`) driven through the shared
  `experiment_runner`, on the schedule `simulation_loop::start` itself runs
  (`core/simulation_schedule.rs`). `tests/live_experiment_tests.rs` covers same-seed checksum
  identity, exact-tick intervention firing, a checkpoint fork, and
  `run N == run K → save to disk → load → run N−K`. State that as *headless adapter verified*, and
  keep saying what is still **not** claimed: no full desktop-app run has been made under the
  multi-threaded executor; the adapter refuses `laws.exotic_energy` (the live world has no MU field)
  and has no AE3 reference population; and numerical agreement with `ReferenceEvolutionWorld` is
  never claimed — only agreement on the *direction and meaning* of a shared law.
- The live schedule is built in exactly one place, `core::simulation_schedule::build_tick_schedule`.
  A headless test that declares its own `.chain()`ed subset is testing a schedule the app does not
  run; call the builder instead.

## Code style

- No Prettier — match surrounding style. TS strict flags and the `@/*` alias live in `tsconfig.json`.

## Tauri IPC contract

Every command and event is listed in `docs/reference/IPC_CONTRACT.md` — read it before changing the IPC surface. The source of truth is the code: `tauri::generate_handler![…]` in `src-tauri/src/lib.rs` (35 commands, bodies in `src-tauri/src/commands/`) and the `handle.emit("…")` calls under `src-tauri/src/` (7 events). `scripts/check_ipc_reference.mjs` (in `npm run check:docs`) fails when the code and that list disagree, so add or remove a command and its row in the same commit. Argument keys are camelCase on the JS side (`file_path` → `filePath`); `npm run check:ipc` checks every `invoke(...)` call site.
