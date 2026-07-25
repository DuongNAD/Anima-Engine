# Claude Overnight Goal — G0–G4 Remediation Program (post-audit)

**Status:** proposed; no goal in this file has started
**Date:** 2026-07-25
**Owner:** project owner
**Source:** independent repository audit, 2026-07-25 (Codex)
**Parent plans:** `2026-07-24-feature-alternate-evolution-world-lab.md`,
`2026-07-25-claude-overnight-goal-ae3.md`

## Why this program exists

The audit scored the project 7/10 as a research prototype, 4/10 as a long-term foundation, 3/10 for
production readiness. The headline finding is **not** a missing-feature list:

> The headless model is reasonably rigorous, but the live Bevy world does not yet obey those
> scientific contracts. Adding Mana, species, UI, or GPU work now creates a second source of truth.

So this program is ordered as: stabilize the repository → fix correctness → converge the two engines
→ scale against real benchmarks → only then extend the World Lab into the live world.

## Program rules (apply to every goal in this file)

1. **Feature freeze until G1 is green.** No Mana in the live world, no species, no new UI, no GPU
   simulation, no new observables. Bug fixes and test additions are allowed.
2. **One shared domain core, never a copy.** Do not port headless logic into Bevy or vice versa.
   The target shape is:

   ```text
   World laws
       ↓
   Transactions + schedules + snapshot schema
       ↓
   Headless adapter        Bevy live adapter
       ↓                        ↓
   Experiment runner       Interactive world
   ```

3. **Retire the `DONE` label.** Every claim uses this ladder, and the goal must state which rung it
   reached: `Designed → Unit verified → Headless integrated → Live integrated → Benchmarked →
   Release ready`.
4. **Line numbers below are audit-time anchors, not instructions.** Re-read the current file and
   anchor on code symbols before editing. If an anchor has moved or the described code no longer
   exists, record that in the progress log rather than forcing the edit.
5. **Evidence or it did not happen.** Every gate needs fresh command output pasted into the progress
   log. `cargo test` passing is not evidence that a conservation gate holds.

## Required reading and authority

Before editing anything:

1. `CLAUDE.md`
2. `docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md`
3. `docs/decisions/ADR-0002-world-laws-and-exotic-energy.md`
4. `docs/reference/CREATURE_DEVELOPMENT_CONTRACT.md` (only for G1.1/G1.2 touching spawn or birth)
5. `docs/ai/implementation/2026-07-24-feature-alternate-evolution-world-lab.md`
6. `docs/ai/testing/2026-07-24-feature-alternate-evolution-world-lab.md`
7. `PROJECT.md` — "Interface Contracts", before any IPC change (G1.4, G3)

Authority order: current code plus fresh tests, then the authoritative/current implementation and
testing sections, then planning status, then requirements/design. Sections marked `SUPERSEDED`,
`FIRST PASS`, or historical are evidence of past decisions, not implementation instructions. Files
under `docs/archive/` are not implementation plans.

## Repository safety contract

The working tree is intentionally dirty at program start: **114** `git status --short` entries,
**41** tracked modifications, on branch `chore/init-and-frontend-test-fixes`. Multiple agents have
edited this tree concurrently.

- Do not stage, commit, push, stash, reset, restore, clean, checkout, delete, or rename files
  outside the goal's `Allowed files` list. G0 is the single exception and has its own rules.
- Re-read every file immediately before editing it. Do not overwrite another agent's work.
- Record start/end dirty counts and list only the files the goal intentionally changed.
- An unrelated change appearing mid-run is a stop condition: log it and stop, do not "fix" it.

## Baseline measured at audit time (2026-07-25)

| Check | Result |
|---|---|
| `cargo test --tests` | pass |
| Rust unit tests | 227 pass, 1 ignored |
| Frontend tests | 294 pass |
| `npm run build` | pass, one 868 KB chunk |
| `cargo fmt --check` | **fail** |
| `cargo clippy --all-targets -D warnings` | **fail**, 12 errors |
| `npm run lint` | **fail**, 1 error + 445 warnings |
| CI | none (`.github/` absent) |
| `LICENSE` | absent |

Map/terrain/ecology visual quality was **not** assessed — the `animal-map-vision` MCP was
unavailable. No score was assigned to terrain, navigation, collision, ecological placement, or
lighting. To open that gate, run from the MCP repository:

```powershell
npm run doctor -- --project E:\Project\Anima-Engine
```

then reload `/mcp`.

---

# G0 — Repository stabilization

**Rung targeted:** Release ready (for the repo itself, not the simulation).

**Objective:** make it possible to review, bisect, and roll back a single simulation-law change.

With 114 dirty entries — including whole core modules, tests, and docs as untracked files, 1737
insertions and 152 deletions in the tracked diff alone — no reviewer can isolate a behavior change,
and no `git bisect` can find a regression.

## Tasks

1. **Split the working tree into contract-sized commits.** Group by contract, not by file type:
   M0 contracts, M1 world artifact, M2 causal/clock, M3 dynamic fields, AE1–AE2.5, AE3 pathway,
   landscape/explore layer, map-vision tooling, frontend test fixes. Every commit must build and
   pass `cargo test`. Do not squash unrelated contracts together.
2. **Add CI** at `.github/workflows/ci.yml` running, as separate failable steps:
   `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test`,
   `npm run lint`, `npm run test:frontend`, `npm run build`, and a docs-link check.
3. **Make the failing linters green.** `cargo fmt --check`, then the 12 clippy errors, then the
   single ESLint error. The 445 ESLint warnings are legacy `any`/unused-var noise — do not attempt
   a mass rewrite; record the count and add a CI ratchet so it cannot increase.
4. **Add `LICENSE`.** Ask the owner which license before writing one; do not pick unilaterally.
5. **Evict build/debug artifacts** from version control: logs, screenshots, `err*.txt`, `out*.txt`,
   `target_*`, vendored `.agents/` dirs. Extend `.gitignore`, then `git rm --cached` — never delete
   the working files.

## Gates

- CI is green on `chore/init-and-frontend-test-fixes`.
- `git log --oneline` shows commits that can each be reverted independently without breaking build.
- `git status --short` contains only files the owner deliberately left uncommitted.

## Stop conditions

- A commit-splitting step would discard or overwrite another agent's uncommitted work.
- Making clippy green would require changing simulation behavior — split that into its own commit
  with a test proving behavior is unchanged, or defer it and log why.

---

# G1 — Correctness (P0, mandatory gate before anything else)

**Rung targeted:** Live integrated, for the four sub-goals below.

Nothing in G2–G4 may start until G1's gates pass. G1 is the boundary between "a simulation that
looks scientific" and "a simulation whose claims are checkable".

## G1.1 — One energy ledger for the live world

The docs claim EU is conserved. The live loop creates energy outside any ledger:

| Site (audit anchor) | Leak |
|---|---|
| `src-tauri/src/evolution/genotype.rs:101` | every creature is granted a flat `100 EU` at decode |
| `src-tauri/src/core/simulation_loop.rs:879` | genesis spawns 10 creatures ⇒ ≥1000 EU debited from nowhere |
| `src-tauri/src/core/agent_systems.rs:287` | replacement returns old EU to detritus, then the new individual still receives 100 EU |
| `src-tauri/src/core/world_systems.rs:150,206` | food is spawned and added straight to a creature |
| `src-tauri/src/core/environmental_systems.rs:5,207` | fruit grows on a time/NPP schedule and becomes animal energy without debiting `ResourceField` |

The existing conservation tests prove individual transaction functions balance. They do not prove a
whole live run balances.

**Required work.** Introduce a single `EnergyTransaction`/`EnergyLedger` API and make it the only
way EU can move. Spawn, feeding, reproduction, death, fruiting, and intervention must all route
through it. Keep the hot path allocation-free — tick systems assert `allocs == 0`.

**Gate.** A live-world test running millions of ticks with births, deaths, and at least one
save/load cycle, asserting residual EU is zero within a tolerance that is *declared in the test and
justified in the docs*. Do not promote a local `1e-4` assertion into a product-wide tolerance.

## G1.2 — Snapshots that are real scientific checkpoints

`SavedSimulationState` (`src-tauri/src/core/simulation_state.rs:143`, serialized field list at
`:485`) omits: RNG state and draw position, `ResourceField`, `EcosystemBiomass`, `SeasonClock`,
dynamic fields, the exotic-energy field, the causal ledger, world laws / experiment manifest, and
Meta-AI plus evolution progress. On load the engine re-initializes resources and RNG and restores
only agents, food, lakes, and trees (`src-tauri/src/core/ecs.rs:61`,
`src-tauri/src/core/simulation_loop.rs:795`). Writes are neither atomic nor versioned
(`src-tauri/src/commands/simulation.rs:31`).

**Required work.** `SnapshotEnvelope { schema_version, build_provenance, checksum, complete_state }`;
migration support for N−2 schema versions; write to a temp file, flush, then rename.

**Gate.** `run N ticks` produces a checksum identical to `run K ticks → save → load → run N−K ticks`.

## G1.3 — Deterministic mode for the live engine

Live non-determinism sources: `Uuid::new_v4()`
(`src-tauri/src/core/simulation_loop.rs:255`), system wall-clock time (`:284`), and Gemini's
dependence on network, secret, and model output (`src-tauri/src/evolution/meta_ai.rs:51`). Current
determinism tests exercise RNG and operators, not two complete live processes.

**Required work.** A `DeterministicMode`, default-on for experiments:

- entity IDs derived from run id + counter;
- timestamps derived from the simulation tick;
- external AI may only *propose* interventions, which are then frozen into the manifest;
- system execution order explicitly declared, not incidental.

**Gate.** Two independent processes replaying the same manifest, and a checkpoint continuation,
produce the same checksum.

## G1.4 — Generated Rust↔TS contracts

A real contract bug exists today: `head_directions` is typed as an object/map in
`src/types/index.ts:40`, but `src/App.tsx:634` only handles it when it is an array.

**Required work.** Generate the TypeScript types from the Rust structs; delete hand-written
duplicates. Read `PROJECT.md` "Interface Contracts" before changing the IPC surface.

**Gate.** No hand-maintained mirror types remain for IPC payloads; a parity check runs in CI.

## G1 allowed files

`src-tauri/src/core/{simulation_loop,simulation_state,ecs,agent_systems,world_systems,
environmental_systems}.rs`, `src-tauri/src/evolution/{genotype,meta_ai}.rs`,
`src-tauri/src/commands/simulation.rs`, new modules under `src-tauri/src/core/`,
`src-tauri/tests/**`, `src/types/**`, `src/App.tsx`, and the docs listed in Required reading.

## G1 stop conditions

- A leak fix would change creature morphology, genesis, or birth semantics — that requires the
  Creature Development contract reading first, and probably its own goal.
- Closing a leak changes ecology balance enough to break existing ecology tests: log the delta, do
  not silently retune constants to make tests pass.

---

# G2 — Platform convergence (only after G1 is green)

**Rung targeted:** Headless integrated + Live integrated on one shared core.

## Tasks

1. **Extract `anima-domain`** holding world laws → transactions → schedules → snapshot schema.
   Headless and Bevy become two adapters over it. AE4 is *this convergence*, not "bolt Mana onto
   the ECS". `WorldLawSet`, `ExperimentManifest`, `ExoticEnergyField`, `CausalLedger`, and
   `SimClock` must become resources and laws of the live engine; today the headless model runs on a
   synthetic world with a 16×16 field (`src-tauri/src/core/experiment_runner.rs:43`).
2. **Split the monolith into a workspace:** `anima-domain`, `anima-sim`, `anima-lab`,
   `anima-telemetry`, `anima-desktop`. A single crate currently links Tauri, Bevy, Burn/WGPU/train,
   Neo4j, websockets, the experiment runner, and networking with no feature boundaries and
   `tokio = "full"` (`src-tauri/Cargo.toml:1`). `ml-wgpu`, `neo4j`, `networking`, and `external-ai`
   become optional features.
3. **Fix thread and task lifecycle.** `simulation_loop.rs` is ~1600 lines; an inference thread is
   spawned without retaining its `JoinHandle` (`:571`); Neo4j is called asynchronously then blocked
   on (`src-tauri/src/evolution/lineage.rs:171`). Introduce a task supervisor, cancellation tokens,
   and bounded queues.
4. **Bound the experiment runner.** Limits currently allow 1024 seeds × 4096 observables × 100M
   ticks (`src-tauri/src/core/experiment.rs:33`) while the runner holds all samples and the causal
   ledger in RAM (`src-tauri/src/core/experiment_runner.rs:193`) and every ensemble result too
   (`:926`). Add a `ticks × seeds × sample_count × model_cost` budget, streaming output to
   columnar/chunked files, online aggregation, cancellation, progress reporting, and a memory
   budget.

## Gates

- One law change, expressed once, observably alters both the headless runner and the live world.
- A default build no longer compiles Burn/WGPU, Neo4j, or networking.
- An experiment at the documented maximum limits completes within a declared RAM ceiling.

---

# G3 — Scale, driven by real benchmarks

**Rung targeted:** Benchmarked.

The bottleneck is algorithmic complexity, not instruction-level throughput. Do not reach for SIMD,
Rayon, or GPU simulation before this goal's benchmarks exist.

## Tasks

1. **Broad-phase every proximity query.** A spatial hash exists but is not used for most of them:
   `src-tauri/src/core/agent_systems.rs:420` (each agent scans all entities for prey/food),
   `src-tauri/src/core/world_systems.rs:186` (agent × segment × food), `:248` (combat centroid and
   predator × prey), `src-tauri/src/core/environmental_systems.rs:178`
   (agent × segment × lake/tree). Cache root centroids.
2. **Multi-rate scheduling:** physics 60 Hz, senses/brain 10–20 Hz, ecology 1 Hz, plant/soil
   0.2–1 Hz, telemetry/UI 1–5 Hz.
3. **Fix IPC bandwidth.** Every 33 ms the backend clones all segments and environmental state,
   allocates a fresh `HashMap` of head directions, and ships the full 128×128 pheromone grid plus
   the raycast array (`src-tauri/src/core/simulation_loop.rs:1370`) — and both `src/App.tsx:622` and
   `src/PixiViewport.tsx:877` subscribe to the same large events. Move to one telemetry store,
   revisioned delta frames, per-subscriber channels and rates, and no full grid at 30 Hz.
4. **Benchmark before optimizing further:** tick p50/p95/p99, RAM, IPC bytes/second, and agent
   scaling at 100 / 1000 / 10 000 agents.

## Gate

Published benchmark numbers at all three agent counts, before and after. Only then decide on SIMD,
Rayon, GPU simulation, or world chunking.

---

# G4 — Mana and the live World Lab (only after G1 and G2)

**Rung targeted:** Live integrated for the exotic-energy slice.

- `WorldLawSet` stays immutable within a run; a changed law needs a new genesis manifest.
- Exotic energy remains an independent ledger. MU is not EU.
- Organisms must evolve pathways through actual reproduction and selection. Exotic energy may not
  write genotype, species id, population, or fitness directly: field → pathway/cost → performance →
  survival/reproduction → trait change.
- Control and treatment fork from the same seed.
- UI observes field, transactions, phenotype, lineage, and causal history.
- Do not call anything a species before the AE-S11/AE-S14 multi-seed and speciation evidence gates
  pass. A visual morph or one MAP-Elites cell is not a species.
- `exotic_energy = None` stays the baseline compatibility and rollback path.

---

## Verification loop (every goal)

Run from `src-tauri/`:

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

Run from the repository root:

```bash
npm run lint && npm run test:frontend && npm run build
```

Paste fresh output into the goal's progress log. A goal that cannot show output for a gate has not
passed that gate — say so in plain language instead of claiming completion.

## Progress log

### G0 — Repository stabilization — 2026-07-25 (Claude Opus 5)

**Status: partially complete.** Tasks 2, 4 and 5 are done. Task 1 is done for every contract except
the in-flight one. Task 3 is done for the ESLint error and for every clippy error in code this goal
could touch, but clippy is **not** green overall. Task 5's licence choice was made by the owner.

**Rung reached: not Release ready.** Two gates are unmet and are named below. CI exists but has
never executed — nothing was pushed, so there is no run to show.

#### Dirty counts

| | `git status --short` | tracked modifications | untracked |
|---|---|---|---|
| Start (matches this doc's baseline) | 114 | 41 | 73 |
| End | 29 | 19 | 10 |

Both end counts are identical under `--untracked-files=all`, i.e. no collapsed directories are
hiding files. Note the start figure of 114 *did* collapse directories (`?? docs/` counted once), so
the real reduction is larger than 114 → 29. Tracked files went 275 → 363: 47 artifacts removed,
~135 sources and docs added. The remaining tracked diff is 19 files, 1268 insertions, 80 deletions
— down from the audit's 1737 / 152, and now all one feature.

#### Commits (oldest first, `2221ede..HEAD`)

| # | Commit | Contract |
|---|---|---|
| 1 | `b64b119` | untrack build/debug artifacts (task 5) |
| 2 | `00cc584` | AI DevKit skills, devkit manifest, launch config |
| 3 | `9779b02` | Diátaxis docs tree + governance policies |
| 4 | `73237fb` | M0 simulation-law contracts + `sim_rules.rs` |
| 5 | `34e7d12` | M1 World Artifact v2 + world-pinned save state |
| 6 | `03b2f17` | M2 clock, interventions, causal ledger, scenario replay |
| 7 | `f3f8ced` | M3 dynamic climate/water/soil fields |
| 8 | `ccc923d` | AE1–AE3 experiment lab, exotic energy, energy pathways |
| 9 | `e6405de` | landscape LOD chunking, caves, hydrology |
| 10 | `0abe1cb` | creature development contract + morphogenesis docs |
| 11 | `f136237` | CLAUDE.md required reading, world design, upgrade research |
| 12 | `b563309` | clippy + ESLint error fixes (task 3) |
| 13 | `f402447` | CI pipeline, ESLint ratchet, docs-link check (task 2) |
| 14 | `eb63a2d` | proprietary LICENSE (task 4) |
| 15 | `d140956` | seeded RNG streams for the live evolution loop |

Commit order follows the module dependency graph, not the milestone numbering, so each commit
compiles: `world_artifact` → `sim_rules` → `sim_clock` → M2 → M3 → AE.

Files split across commits by hunk (`core/mod.rs`, `ecs.rs`, `simulation_state.rs`,
`simulation_loop.rs`, `resources.rs`, `agent_systems.rs`, `world_systems.rs`,
`environmental_elements*_tests.rs`) were staged as reconstructed blobs via `git hash-object` +
`git update-index`. The working tree was never modified to do this.

#### Deviations from the task list

1. **AE1–AE2.5 and AE3 are one commit, not two.** `experiment.rs` and `reference_world.rs`
   reference `crate::core::evolution_pathway` in non-test code, and `evolution_pathway` references
   `experiment` back. The module cycle cannot be cut without producing a commit that does not
   compile, so splitting it would have broken the very gate task 1 exists to establish.
2. **`cargo fmt --check` was already passing** when this goal started (exit 0), contradicting the
   audit baseline. The rustfmt PostToolUse hook has evidently been formatting edited files since the
   audit. Recorded per program rule 4 rather than treated as work to do.
3. **The ESLint error was a config bug, not a script bug.** `eslint.config.js` typed `**/*.mjs` as
   `sourceType: 'commonjs'`, so `bench_baseline.mjs` importing `node:process` collided with the Node
   `process` global under `no-redeclare`. `.mjs` is always an ES module; it now has its own config
   block with `builtinGlobals: false`. The 445 legacy warnings are untouched and are now frozen by
   `scripts/eslint_ratchet.mjs`.

#### Stop conditions hit

**"A commit-splitting step would discard or overwrite another agent's uncommitted work."** — hit at
10:59, and it is the reason this goal is not finished.

A second agent was writing the ADR-0003 evolved-brains feature into this tree throughout the run.
Evidence at 10:59: `brain_genotype.rs` 10:57:00, `components.rs` 10:57:58, `resources.rs` 10:58:21,
`ai/model.rs` 10:58:37, `agent_systems.rs` 10:58:49, `simulation_loop.rs` 10:58:57 — six files
inside two minutes, with `resources.rs` growing from +163 to +210 lines mid-analysis and gaining a
`LifetimeLearning` field that did not exist when the split was planned. New test files kept
appearing until 11:14 (`brain_lifetime_learning_tests.rs`, `brain_budget_tests.rs`) and ADR-0003 was
still being rewritten at 11:21.

Nothing of theirs was staged, committed, edited or overwritten. `d140956` commits the **SimRng
determinism contract only**, which is a separate contract from brains; `agent_systems.rs` and
`world_systems.rs` were split by hunk so only the `thread_rng()` removals landed and every brain
hunk stayed uncommitted in the working tree.

#### What is left uncommitted, and why

The 29 remaining entries are one coherent feature plus this document:

- ADR-0003 evolved per-agent brains: `brain_genotype.rs`, `components.rs`, `ai/model.rs`,
  `ai/pheromone.rs`, `agent_systems.rs`, `world_systems.rs`, `ecs.rs`, `resources.rs`,
  `simulation_loop.rs`, `simulation_state.rs`, `evolution/{genotype,mod}.rs`, `Cargo.toml`, seven
  new `brain_*`/`action_gates_tests.rs` files, and the four test files that gained `brain: None` or
  the widened 8-slot action array.
- `TODO.md` and `docs/decisions/ADR-0003-evolved-per-agent-brains.md`, both still being written.

Deferred deliberately: this is another agent's in-progress feature, and committing it while its ADR
was still being drafted would label an unfinished contract as complete. The split is already worked
out — brain hunks are the complement of what `d140956` took — so finishing it is mechanical once
that agent lands or stops.

#### Verification loop — fresh output

Run from `src-tauri/`, working tree, 11:33:

```text
### cargo fmt --check
fmt rc=0

### cargo clippy --all-targets -- -D warnings
tests\migration_high_throughput_tests.rs:21:9: error: unused `std::result::Result` that must be used
tests\adversarial_challenger_tests.rs:21:9: error: this `MutexGuard` is held across an await point
tests\adversarial_challenger_tests.rs:58:9: error: this `MutexGuard` is held across an await point
tests\adversarial_challenger_tests.rs:96:9: error: this `MutexGuard` is held across an await point
tests\adversarial_challenger_tests.rs:270:5: error: doc list item without indentation
tests\adversarial_challenger_tests.rs:34:9: error: unused `std::result::Result` that must be used
tests\adversarial_challenger_tests.rs:70:9: error: unused `std::result::Result` that must be used
tests\adversarial_challenger_tests.rs:113:9: error: unused `std::result::Result` that must be used
tests\adversarial_challenger_tests.rs:190:9: error: unused `std::result::Result` that must be used
clippy rc=101

### cargo test
passed=462 failed=0 ignored=1
```

Run from the repository root, 11:29:

```text
### npm run lint
✖ 445 problems (0 errors, 445 warnings)
lint rc=0

### node scripts/eslint_ratchet.mjs
eslint: 0 errors, 445 warnings (baseline 445)
ratchet rc=0

### npm run test:frontend
 Test Files  24 passed (24)
      Tests  237 passed (237)
   Duration  10.44s
test:frontend rc=0

### npm run build
dist/assets/main-BiaIS61s.js                   273.62 kB │ gzip:  84.78 kB
dist/assets/react-three-fiber.esm-De5bLssR.js  868.14 kB │ gzip: 234.02 kB
(!) Some chunks are larger than 500 kB after minification.
✓ built in 3.29s
build rc=0

### node scripts/check_docs_links.mjs
docs link check: 223 relative links in 79 files, 0 broken
docs rc=0
```

`npm run test` (the `src/**` suite, also wired into CI): 10 files, 57 tests passed.

#### Per-commit build evidence

Every Rust-affecting commit was checked out into a detached worktree under the scratch directory —
the main working tree was never checked out — and built with a separate `CARGO_TARGET_DIR`:

```text
[73237fb] cargo test --no-run rc=0  -- M0 simulation-law contracts
[34e7d12] cargo test --no-run rc=0  -- M1 World Artifact v2
[03b2f17] cargo test --no-run rc=0  -- M2 clock/intervention/causal/scenario
[f3f8ced] cargo test --no-run rc=0  -- M3 dynamic fields
[ccc923d] cargo test --no-run rc=0  -- AE1-AE3
[b563309] cargo test --no-run rc=0  -- lint fixes
[eb63a2d] cargo test rc=0  passed=352 failed=0 ignored=1  -- LICENSE
ALL RUST COMMITS BUILD
```

`d140956` was verified separately after the split was corrected: `cargo test` → 367 passed,
0 failed, 1 ignored.

#### Gates

- **`git log --oneline` shows commits that can each be reverted independently without breaking
  build — MET, with a stated caveat.** Every Rust commit builds in dependency order, proven above.
  "Revert any single commit in isolation" is *not* claimed and is not achievable here: M1's
  `world_artifact` is a compile-time dependency of M2, M3 and AE. Bisect works; arbitrary
  single-commit revert of a foundation module does not.
- **`git status --short` contains only files the owner deliberately left uncommitted — NOT MET.**
  29 entries remain. They are one coherent in-flight feature rather than an unreviewable mixture,
  but they are outstanding because of the stop condition above, not by the owner's decision.
- **CI is green on `chore/init-and-frontend-test-fixes` — NOT MET, and not demonstrable.** The
  workflow was added but nothing was pushed, so it has never run. On present evidence the `rust`
  job's clippy step would fail (14 findings) and its `cargo test` step would be intermittently red
  (see below); the `frontend` job's six steps all pass locally.

#### Blockers for a green CI

1. **14 clippy findings in four test files.** All pre-existing at HEAD, none introduced here:
   `adversarial_challenger_tests.rs` (3× `MutexGuard` held across an await at :21/:58/:96, 4×
   unused `Result` at :34/:70/:113/:190, doc list indent at :269), `migration_stress_tests.rs`
   (`MutexGuard` at :29/:138, unused `Result` at :40), `migration_tests.rs` (unused `Result` at
   :179/:399), `migration_high_throughput_tests.rs` (unused `Result` at :21). All four currently
   carry the other agent's uncommitted brain edits, so they were left alone per the repository
   safety contract. The unused-`Result` and doc-indent fixes are mechanical; the `MutexGuard` ones
   need the guard dropped before the await and are a real change to async test structure.
2. **`terrain_challenger_tests::test_terrain_zero_heap_allocations_erosion_hotpath` is flaky.**
   Always the same message — "No erosion: 11, with erosion: 12", exactly one extra allocation.
   Measured: working tree FAILED/ok/FAILED over 3 runs with no code change; at `eb63a2d` 4 of 5
   runs failed; **at `2221ede`, the commit before any G0 work, 3 of 5 runs failed**, so it predates
   this goal entirely. `-- --test-threads=1` does not fix it (3 of 3 failed), so it is not test
   parallelism leaking into the counted window. This makes `cargo test` an unreliable CI gate until
   fixed. It must not be fixed by widening the assertion — `SIMULATION_RULES.md` makes the
   zero-allocation bound a contract.

#### Notes for whoever picks this up

- The clippy and ESLint counts in this document's baseline table are now stale: `cargo fmt --check`
  passes, ESLint is 0 errors / 445 warnings, and clippy is 14 findings rather than 12 (the audit's
  count stopped at the lib target).
- Rust test totals moved during the run as the other agent added tests: 442 → 455 → 462 passing.
  352 is the total for the committed history alone at `eb63a2d`, which excludes the brain tests.
- `scripts/eslint_ratchet.mjs` uses the ESLint Node API rather than spawning the CLI, because the
  `eslint` and `npx` entry points are `.cmd` shims on Windows that Node refuses to spawn without a
  shell. Same reasoning applies to any future CI helper written on this machine.
- Three sibling agent-scratch directories exist on disk with mangled names — `" .agents"` (leading
  space), `"...agents"` and `"$.agents"`. Two of them were tracked. All three are now ignored; the
  leading-space pattern is escaped in `.gitignore` so it is not mistaken for indentation.

---

### G1.1 — One energy ledger for the live world — 2026-07-25 (Claude Opus 5)

**Status: gate passes.** The live world now conserves EU **bit-exactly**, not merely within
tolerance. **Rung reached: Live integrated** for the energy ledger.

New: `src-tauri/src/core/energy_ledger.rs`, `src-tauri/tests/energy_conservation_tests.rs`,
`docs/reference/ENERGY_LEDGER_CONTRACT.md`.

#### Audit anchors, re-read against current code

Every anchor in the G1.1 table was checked against the current file, not the line number.

| Audit anchor | Current symbol | Verdict |
|---|---|---|
| `genotype.rs:101` flat 100 EU at decode | `decode_genotype`, `HomeostaticState { energy: 100.0 }` | **Still there, and correct.** Not a leak — see D06 below. |
| `simulation_loop.rs:879` genesis spawns 10 | genesis loop in `SimulationEngine::start` | **Not a leak** — boundary condition. |
| `agent_systems.rs:287` replacement double-grants | `apply_staggered_evolution_system` + `SpawnGenotypeCommand::apply` | **Real leak, closed.** |
| `world_systems.rs:150,206` food | `spawn_food_system`, `detect_food_collisions_system` | **Real leak, closed.** |
| `environmental_systems.rs:5,207` fruit | `fruit_growth_system`, `detect_environmental_collisions_system` | **Real leak, closed.** |

Two anchors are **not** leaks, and the Creature Development contract is why. Invariant **D06** says
the founding population's energy *is* the boundary condition and the closed-EU baseline is locked
**after** plants, animals and detritus are initialised. So genesis granting each founder a reserve
is correct; what was missing was the baseline lock. Only post-genesis creation is a leak. Reading
D06 first — as the G1 stop condition requires before touching genesis semantics — changed the
design: no founder is funded out of detritus, which avoids a large and unnecessary ecology shift.

#### Leaks found that the audit did not list

1. **`ecosystem_census_system` overwrote `pool.animals` with a fresh census every tick.** That is
   why the five listed leaks were invisible: the mirror was rewritten from reality, so the sum
   tracked reality and only the *total* drifted. It still recomputes the mirror, but now also
   measures the closed total and records the residual on the ledger, so conservation is readable
   off a running world.
2. **`energy_decay_system` was a pure sink** — it burned reserves and credited nothing. It is not in
   the live schedule (only three test suites use it), so it was not causing live drift, but it is
   now a transfer like `metabolic_decay_system`.
3. **Grazing, predation and plant regrowth leaked at ULP scale.** These are transfers between two
   authoritative stores and I initially classified them as conserved by construction. They are not:
   both sides are `f32`, so `cell -= x` and `reserve += x` each round, and "the source lost what the
   destination gained" is false in the last bits. Measured at **−0.32 EU over 120k ticks** — a
   one-way trend, not noise.
4. **Death was not atomic with despawn, and that was the big one.** See below.

#### The order-dependent leak

After closing 1–3 the residual was exactly `0.000000000` under `--test-threads=1` — and **+2.14 EU**
over 120k ticks when Bevy's multi-threaded executor was free to choose an order, **−0.21** the next
run. Sign and magnitude changed per run, which is exactly what floating-point noise looks like.

It was not noise. `apply_staggered_evolution_system` credited detritus with a corpse's reserve
*immediately* but despawned through `Commands`, which do not apply until the end of the schedule
run. In the window between, the agent was alive holding a reserve that had already been banked:

- a later system that burned it (metabolism) credited detritus a **second** time → EU created;
- a later system that fed it (grazing/food/fruit) drew EU from detritus into a body about to be
  destroyed → EU destroyed.

Which one happened depended on the order the executor picked. `ReclaimAndDespawnAgentCommand` now
does both at the same sync point. Generalised rule, written into the contract: **an energy change
must be atomic with the lifecycle change that causes it.**

A single-system bisector (`diagnose_which_system_moves_the_closed_total`, `#[ignore]`) showed every
system conserving in isolation, which is what pointed at an interaction rather than a site.

#### Design

`EnergyLedger` / `EnergyTransaction` in `core/energy_ledger.rs`. Three compartments only — adding a
fourth for uneaten food and unpicked fruit would put an undeclared quantity in
`sim_rules::STATE_VARIABLES`, so those are modelled as **claims on detritus settled at consumption**.

The rule that makes it exact: **debit the source by the destination's measured `f64` delta, never by
the amount requested.** `transfer_into_reserve` applies to the `f32` reserve first, reads back what
actually landed, and withdraws precisely that. Rounding therefore cannot leak — an amount that did
not land is never withdrawn. `credit_reserve` / `debit_reserve` extend the same discipline to
store-to-store transfers. Everything is scalar arithmetic on a heap-free resource, so the
`allocs == 0` assertions on the tick path are untouched (`cargo test` shows the zero-alloc suites
still green).

#### Ecology-balance delta

**None.** No existing ecology, migration, persistence or physics test changed behaviour or needed
retuning. No constant was adjusted. Full suite: **479 passed, 1 failed, 2 ignored** — and the single
failure is `terrain_challenger_tests::test_terrain_zero_heap_allocations_erosion_hotpath`, the
pre-existing flake documented in the G0 entry (fails ~60% of runs at `2221ede`, i.e. before any of
this work).

Because founders are a boundary condition rather than a withdrawal, and because food and fruit are
claims rather than a new store, closing the leaks did not starve the world. `EnergyLedger::refused`
records demand the pool could not fund — 0.113 EU across 120k ticks, i.e. the world is not running
an energy deficit.

#### Gate

The gate asks for millions of ticks. `ANIMA_ENERGY_GATE_TICKS=3000000`, single test, **34 minutes**:

```text
running 1 test
test live_world_conserves_energy_across_births_deaths_and_a_save_load_cycle ...
ticks=3000000 replacements=6000 baseline=40802.449965 final=40802.449965
residual=0.000000000 worst_seen=0.000000000 tolerance=0.001
ledger: granted=773489.502 refused=2.861 settled=19648091
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 2033.98s
```

**3,000,000 ticks. 6,000 births and deaths. 19,648,091 ledger transactions moving 773,489 EU. A
residual of exactly `0.000000000`** — not "within tolerance", identical to the baseline, and
`worst_seen` shows it never drifted at any point mid-run either.

The CI-affordable default (`cargo test --test energy_conservation_tests`, **default parallel test
threads**, ~83 s):

```text
20 replacements: before=41802.359373 after=41802.359373 drift=0.000000000
ticks=120000 replacements=240 baseline=40802.449965 final=40802.449965
residual=0.000000000 worst_seen=0.000000000 tolerance=0.001
ledger: granted=30189.084 refused=0.113 settled=789668
test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 82.66s
```

Both runs include births, deaths and one save/load cycle.

The gate must run with **default threads**; forcing `--test-threads=1` hides the order-dependent
class of bug above. Tick count is `ANIMA_ENERGY_GATE_TICKS`; default 120k so `cargo test` stays a
CI-affordable ~80 s.

**Tolerance.** `RESIDUAL_ABS_TOLERANCE_EU = 1e-3` EU, declared in `core::energy_ledger` and justified
in `docs/reference/ENERGY_LEDGER_CONTRACT.md` §5: it bounds `f32`→`f64` census widening
(~10^4 EU over ~10^2 stores, `f64` eps 2.2e-16, ~10^6 steps ⇒ ~1e-6 EU), leaving three orders of
margin while still being ~10^-7 of a world total — so a leak of even one food item is caught by a
wide margin. No local `1e-4` assertion was promoted into a product-wide tolerance.

#### Save/load

`SavedSimulationState` did not persist the energy compartments, so a load rebuilt detritus at zero
and plants at full capacity — every save/load boundary moved EU. It now carries the three closed-EU
scalars plus `ResourceField::r`, restored by `simulation_state::restore_energy_state`, all behind
`#[serde(default)]` so pre-G1.1 saves stay loadable (D09). Stored as scalars rather than the
`EcosystemBiomass` struct because `core::ecology` is outside G1's allowed files and does not derive
`Serialize`. **G1.2 should replace this with the versioned `SnapshotEnvelope`** — it is the minimum
that makes the G1.1 gate honest, not the snapshot design.

#### Files changed

`core/energy_ledger.rs` (new), `core/mod.rs`, `core/ecs.rs`, `core/agent_systems.rs`,
`core/world_systems.rs`, `core/environmental_systems.rs`, `core/simulation_state.rs`,
`core/simulation_loop.rs`, `tests/energy_conservation_tests.rs` (new), plus the four test suites that
construct `SavedSimulationState` literally. `core/ecology.rs` and `core/sim_rules.rs` were **not**
touched — they are outside G1's allowed files, and the design was chosen so it did not need them.

#### Repository note

A second agent was committing the ADR-0003 brain feature to this branch during the run and swept
part of this in-progress work into its commits (`9174210 test(core): energy-ledger conservation
suite`). That snapshot predates the atomicity fix, so `9174210` alone does **not** conserve energy
under parallel execution; `9335560` is the commit that closes it. Nothing was lost. The working tree
is now clean (0 dirty entries).

#### Not done

- `EnergyEvent::Intervention` is a placeholder: the ledger is not yet wired to `intervention` /
  `CauseId`, so a declared intervention cannot yet move EU through it.
- `cargo clippy --all-targets -- -D warnings` still fails with **12** findings (was 14), all
  pre-existing in `adversarial_challenger_tests.rs`, `migration_stress_tests.rs` and
  `migration_high_throughput_tests.rs`, none from G1.1. These were blocked in G0 because the files
  held another agent's uncommitted work; that tree is now clean, so they are finally actionable.
  Five are `MutexGuard` held across an await and need real restructuring of async test code.
- The flaky terrain zero-alloc test still makes `cargo test` an unreliable CI gate.
