---
phase: implementation
feature: readme-claude-md-refresh
title: Proposal — Refresh of README.md, README.vi.md, and CLAUDE.md against trunk e576350
description: Proposal-only audit and minimal diffs for README.md, README.vi.md, and CLAUDE.md against origin/feat/ecological-selection (e576350) with verified source commits.
status: proposed
owner: maintainers
last_reviewed: 2026-11-23
---

# Proposal: Refresh of `README.md`, `README.vi.md`, and `CLAUDE.md` (`CARD DOC7`)

- **Baseline trunk:** `origin/feat/ecological-selection` at `e576350fc2a8b9c6765eea81a94ecfa705fcc528` (`e576350`, Merge pull request #213, `2026-11-23T13:35:50+07:00`).
- **Pending dependency note:** PRs `#201`–`#205` (along with `#196`, `#199`, `#200`, `#208`, `#210`, `#211`) remain unmerged on `e576350`; a final re-pass on these proposals may follow once `#201`–`#205` land on trunk.
- **Scope:** Proposal-only (`status: proposed`). All target documents (`README.md`, `README.vi.md`, `CLAUDE.md`) and `docs/ai/README.md` are untouched here because root `README*` and `CLAUDE.md` are human-reviewed R5 contracts requiring explicit owner authorization ([`AGENT_WORKFLOW.md`](../../how-to/AGENT_WORKFLOW.md) §4–§5).
- **Policy adherence:** Follows [`DOCUMENTATION_POLICY.md`](../../governance/DOCUMENTATION_POLICY.md) Rule 4 (`README` is a map that links to canonical sources rather than copying them), Rule 8 ([`STATE_OF_THE_PROJECT.md`](../../planning/STATE_OF_THE_PROJECT.md) §1 is the single home for authoritative measurements), Rule 9 (classify numbers), and Rule 10 (preserve honesty boundaries verbatim).

---

## 1. Target File Resolution: Retirement of `README.en.md` (`d2ae358`)

Although `CARD DOC7` in [`docs/ai/planning/2026-11-05-swarm-prompts.md`](../planning/2026-11-05-swarm-prompts.md) (lines 1789–1802) originally named `README.md`, `README.en.md`, and `CLAUDE.md`, **`README.en.md` no longer exists at `e576350`**:

- Commit `d2ae358` (PR `#187` `c674db8`, `2026-11-20`): deleted `README.en.md`, promoted the English README to [`README.md`](../../../README.md) (320 lines), moved the Vietnamese README to [`README.vi.md`](../../../README.vi.md) (315 lines), and updated [`scripts/root_allowlist.json`](../../../scripts/root_allowlist.json) lines 28–29 (`"README.md": "README in English, the default GitHub shows"`, `"README.vi.md": "README in Vietnamese"`).
- **Decision:** Do **not** recreate `README.en.md`. All remaining English README updates (`R1`–`R14`) target [`README.md`](../../../README.md), and matching parity updates (`V1`–`V14`) are proposed for [`README.vi.md`](../../../README.vi.md) alongside remaining [`CLAUDE.md`](../../../CLAUDE.md) updates (Section 6).

### Note on `DOC1`–`DOC5` Dependencies at `e576350`

None of `DOC1`–`DOC5` have merged on `e576350` (`docs/tutorials/` contains only `README.md`; `docs/how-to/` lacks `RUN_EXPERIMENTS.md` et al.; `docs/explanation/` lacks `ARCHITECTURE.md` / `WHY_THE_GATES.md`; `docs/reference/` lacks `OBSERVABLES.md` / `ANIMA_LAB_CLI.md` / `MANIFEST_SCHEMA.md`). Every link proposed below targets **only files that already exist on `e576350`**.

---

## 2. Already Accurate on `e576350` (No Spurious Edits Proposed)

Verification against `e576350` (incorporating PR `#213` `4fe3d7a` and PR `#212` `14e7c14`) confirms the following items are **already accurate** and require no rewrite:

| Document & Lines | Existing Accurate Content | Trunk Verification |
|---|---|---|
| [`CLAUDE.md`](../../../CLAUDE.md):13 | Three workspace crates: `anima-engine`, `crates/anima-domain`, and offline `crates/anima-worldgen`. | Updated in `4fe3d7a` (PR `#213` `e576350`); `src-tauri/Cargo.toml:85`. |
| [`CLAUDE.md`](../../../CLAUDE.md):34 | Full `--features desktop` rationale (2,040 skipped lines matching `check_test_targets.mjs:15`) plus `cargo test -p anima-worldgen --features "reader,generate,bake"` (`--profile worldgen-cpu`). | Updated in `4fe3d7a` (PR `#213` `e576350`). |
| [`CLAUDE.md`](../../../CLAUDE.md):35 | `check_test_targets.mjs --profile {default\|desktop\|worldgen-cpu}`. | Updated in `4fe3d7a` (PR `#213` `e576350`). |
| [`CLAUDE.md`](../../../CLAUDE.md):37 | Release desktop binary vs Playwright E2E (`test:e2e` runs against Vite dev server with `tests/e2e/tauri-mock.ts`). | Updated in `4fe3d7a` (PR `#213` `e576350`). |
| [`CLAUDE.md`](../../../CLAUDE.md):39 | Dependency advisory gates `cargo audit` and `npm run check:audit` (`scripts/check_npm_audit.mjs`). | `18ae1e6`; `package.json:46` defines `check:audit` (not `check:npm-audit`). |
| [`CLAUDE.md`](../../../CLAUDE.md):41 | `**burn is at 0.22.1**`, `burn-flex` deterministic reference (`Device::flex()`), `burn-wgpu` under `ml-wgpu` + `ANIMA_USE_GPU`, `LIVE_MODEL_VERSION` `"live-bevy-world/2"`, snapshot schema 8, `bevy_ecs` 0.13. | `74dcd31`, PR `#127` `f461466`; `src-tauri/Cargo.toml:48,55`; zero `0.13.2` occurrences in `README.md`, `README.vi.md`, or `CLAUDE.md`. |
| [`CLAUDE.md`](../../../CLAUDE.md):47 | `@react-three/fiber` mocked in Vitest while real `three` runs headless. | Updated in `4fe3d7a` (PR `#213` `e576350`). |
| [`CLAUDE.md`](../../../CLAUDE.md):111 | Single A2C objective in `core::training::a2c_loss` (actor coefficient `+td`). | Updated in `4fe3d7a` (PR `#213` `e576350`). |
| [`CLAUDE.md`](../../../CLAUDE.md):193 | `36 commands` in `tauri::generate_handler![...]` and `7 events` via `handle.emit(...)`; mentions `npm run check:docs` and `npm run check:ipc`. | `e0ec7ed`, `19e67fa`; `src-tauri/src/lib.rs:63-100`. |
| [`README.md`](../../../README.md):63-64 & [`README.vi.md`](../../../README.vi.md):60-62 | F051 resolved: headless runs generate their own terrain and never read the app's world file. | Updated in `4fe3d7a` (PR `#213` `e576350`). |
| [`README.md`](../../../README.md):99 & [`README.vi.md`](../../../README.vi.md):97 | Burn actor-critic, `burn-flex` CPU deterministic reference, `burn-wgpu` GPU on request. | `74dcd31`, `f461466`. |
| [`README.md`](../../../README.md):88-91 & [`README.vi.md`](../../../README.vi.md):86-89 | `utf8-chars` 3.0.7 Apache-2.0 election under `MIT OR Apache-2.0`; `licensing/UNRESOLVED.md` empty; open legal question preserved. | `dc66ec8`, `f99dc6d`, PR `#175` `0dc00fb`. |

---

## 3. Boundary: Merged Trunk (`e576350`) vs. Open Draft PRs & Non-Trunk Work

The proposals in Sections 4–6 reflect **only** what is merged in `e576350`. None of the unmerged items below may be claimed as landed:

| Item / PR | Status at `e576350` | Ref | What Is Excluded from `CARD DOC7` Proposed Diffs |
|---|---|---|---|
| PR `#208` | OPEN / Draft (`NOT_ANCESTOR`) | `mac/doc8-doc-commands-gate` | `npm run check:doc-commands`, `scripts/check_doc_commands.mjs`, `.github/workflows/docs-commands.yml`. |
| PR `#207` | OPEN (`NOT_ANCESTOR`) | `fix/live-lineage-births` | `F-066` fix, `live.lineage_tracking`. |
| PR `#205` | OPEN / Draft (`NOT_ANCESTOR`) | `mac/doc2-how-to` | Six `DOC2` how-to guides (`RUN_EXPERIMENTS.md`, `WRITE_A_MANIFEST.md`, etc.). |
| PR `#204` | OPEN / Draft (`NOT_ANCESTOR`) | `mac/doc1-first-experiment` | `DOC1` `docs/tutorials/01-first-headless-experiment.md`. |
| PR `#202` | OPEN / Draft (`NOT_ANCESTOR`) | `mac/doc3-explanation` | `DOC3` `docs/explanation/ARCHITECTURE.md` and `WHY_THE_GATES.md`. |
| PR `#201` | OPEN / Draft (`NOT_ANCESTOR`) | `mac/doc5-doc6-reconcile-contributing` | `DOC5`/`DOC6` `CONTRIBUTING.md` CI alignment and `STATE`/`BENCHMARKING` updates. |
| PR `#200` | OPEN / Draft (`NOT_ANCESTOR`) | `mac/r5-findings-fixes` | `F053-FIX` (`live.mean_lifespan` windowing) and `F-055`..`F-065` fixes. |
| PR `#199` | OPEN / Draft (`NOT_ANCESTOR`) | `mac/wv2-2` | `WV2-2` stages `S4`/`S6`/`S7`/`S8`/`S9`, `pipeline.rs`, and `[[bin]] anima-worldgen` CLI (`src/bin/anima-worldgen.rs`). On `e576350`, `anima-worldgen` contains **only `WV2-1a..1c`** (`S1` tectonics, `S2` hydrology, `S3` fluvial/thermal erosion). |
| PR `#196` | OPEN / Draft (`NOT_ANCESTOR`) | `mac/m2-gate-controls` | `scripts/paper/gate_controls.mjs`, `docs/paper/data/gate_controls.md`, `tests/frontend/gateControls.test.ts`. |
| PR `#188` | OPEN / Draft (`NOT_ANCESTOR`) | `dell/m2-gate-controls` | Earlier `M2` gate-controls draft. |
| Sibling `M2` cards | Pending (not on `e576350`) | `mac/p1-incidents`, `mac/gate-conflict-marker-scan` | `docs/paper/data/incidents.csv` (`CARD P1`) and `npm run check:conflict-markers` (`M2` Finding #5). |
| Proposed contracts | `status: proposed` at `e576350` | — | [`ADR-0009`](../../decisions/ADR-0009-burn-022-flex-wgpu-upgrade.md), [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../../reference/EVOLUTION_EXPERIMENT_CONTRACT.md), and [`CREATURE_PROMPT_CONTRACT.md`](../../reference/CREATURE_PROMPT_CONTRACT.md) remain `proposed`, not `accepted`. |

---

## 4. Proposed Changes to `README.md` (`R1`–`R14`)

All line numbers and current excerpts below were verified directly against `e576350:README.md` (320 lines).

### `R1` — Optional link to `DETERMINISM_CONTRACT.md` (`F051` already landed in `#213`)

- **Target:** `README.md:62-64`
- **Source commits:** `62c70cc` (PR `#174`), `6a9b9a2` (PR `#179`), `4fe3d7a` (PR `#213` `e576350`)
- **Reason:** `#213` (`4fe3d7a`) already removed the resolved F051 exception from `README.md:63-64`; only a concise pointer to [`DETERMINISM_CONTRACT.md`](../../reference/DETERMINISM_CONTRACT.md) remains optional.

Current (`e576350:README.md:62-64`):
```markdown
- **Reproducible.** The same seed, commit and operating system give the same checksum. Trajectories differ across
  operating systems, so tests compare two runs instead of pinning an absolute number. Headless runs generate their
  own terrain and never read the app's world file (card F051), and each run records the terrain it used.
```

Proposed:
```markdown
- **Reproducible.** The same seed, commit and operating system give the same checksum. Trajectories differ across
  operating systems, so tests compare two runs instead of pinning an absolute number. Headless runs generate their
  own terrain and never read the app's world file (card F051), and each run records the terrain it used
  ([`DETERMINISM_CONTRACT.md`](docs/reference/DETERMINISM_CONTRACT.md)).
```

### `R2` — Update boundaries section date

- **Target:** `README.md:67`
- **Source commit:** `e576350`
- **Reason:** Reflects trunk updates merged between 2026-11-19 and 2026-11-23 (F051/F051b, SEL2-P smoke, E3 confirmation, `FOUNDER-LAYOUT`, `WV2-1a..1c`, F-054 CI fixes, `#212`/`#213`).

Current (`README.md:67`):
```markdown
## Current boundaries (2026-11-19)
```

Proposed:
```markdown
## Current boundaries (2026-11-23)
```

### `R3` — Update scientific experiment boundary (SEL2-P & E3, linking `summary.md` and `STATE §3`)

- **Target:** `README.md:74-79`
- **Source commits:** `e9c92dc` (PR `#173`), `d90be0b` (PR `#181`), `1ef4f0d` (PR `#183`), `a3d7ab6` (PR `#191` `d64e536`)
- **Reason:** Per Rule 4 (`README` links rather than mirroring outcome counts) and Rule 8, link directly to E3 [`summary.md`](../../../artifacts/experiments/e3-productivity-gradient-body-mass/confirmation/summary.md) and [`STATE §3`](../../planning/STATE_OF_THE_PROJECT.md) without embedding experimental run counts here.

Current (`README.md:74-79`):
```markdown
- **There is no strong scientific result yet.**
  - The experiment infrastructure exists.
  - Experiment E2 gave a null result because of a floor effect.
  - The E-SEL1 preregistration was closed before it ran.
  - The status of SEL2-P (the first preregistered experiment on the live world) and of E3 is in
    [STATE §3](docs/planning/STATE_OF_THE_PROJECT.md).
```

Proposed:
```markdown
- **There is no strong scientific result yet.**
  - The experiment infrastructure exists.
  - Experiment E2 gave a null result because of a floor effect, and the E-SEL1 preregistration was closed before it ran.
  - SEL2-P stopped at its smoke gate (`gate_pass=false`), and E3 confirmation ended with `insufficient_evidence`
    ([E3 summary.md](artifacts/experiments/e3-productivity-gradient-body-mass/confirmation/summary.md)). Current outcomes
    and follow-up work (VIAB-1, E3b, SEL2-P2) live in [STATE §3](docs/planning/STATE_OF_THE_PROJECT.md).
```

### `R4` — Document additional opt-in `live.*` genesis/ecological controls

- **Target:** `README.md:85-87`
- **Source commits:** `4794b6f` (PR `#186`), `7bdddf7` (PR `#182`), `f2887f1` (PR `#177`), `eedf577` (PR `#180`), `5cc4675` (PR `#135`)
- **Reason:** `src-tauri/src/core/live_experiment.rs:255` (`LIVE_KEYS`) now exposes 29 opt-in manifest keys including `live.strict_prey_cap`, `live.grass_steering`, `live.founder_layout`, and `live.lake_layout`, all off/legacy by default so unflagged worlds remain unchanged.

Current (`README.md:85-87`):
```markdown
- **Mortality, ecological fitness, continuous reproduction and locomotion steering are in the code but off by
  default.** A default world still has no death, and fitness is scored by distance. The land-only walking rule is on
  in the app; in experiments the manifest key `live.land_mask` turns it on.
```

Proposed:
```markdown
- **Mortality, ecological fitness, continuous reproduction and locomotion steering are in the code but off by
  default.** A default world still has no death, and fitness is scored by distance. The land-only walking rule is on
  in the app; in experiments `live.land_mask` turns it on, and newer ecology/genesis controls (`live.strict_prey_cap`,
  `live.grass_steering`, `live.founder_layout`, `live.lake_layout`) are likewise opt-in `live.*` manifest keys
  (`src-tauri/src/core/live_experiment.rs`).
```

### `R5` — Mention offline `anima-worldgen` World v2 crate (`WV2-1a..1c` only)

- **Target:** `README.md:97`
- **Source commits:** `f523e3c` (PR `#193` `cb61b00`), `25cf7f6` (PR `#194` `bfbb9c2`), `95b1338` (PR `#195` `45238ff`), `7f2e730` (PR `#192` `d871fd6`)
- **Reason:** `src-tauri/crates/anima-worldgen/` now exists on trunk with Stages S1 (tectonics), S2 (Priority-Flood/MFD-D8 hydrology), and S3 (coupled Fastscape-SD fluvial + thermal erosion), while not yet wired into `anima-engine` (`crates/anima-worldgen/Cargo.toml:21`, `.github/workflows/ci.yml:143-148`). Strictly excludes unmerged PR `#199` (`S4`/`S6`/`S7`/`S8`/`S9` and CLI binary).

Current (`README.md:97`):
```markdown
| **World** | A TypeScript world generator driven by one seed: layered noise, erosion, lakes by priority flood, rivers, waterfalls at knickpoints, cave mouths, climate from latitude and altitude, 22 biomes. The world reaches the simulation as a versioned, checksummed `WorldArtifact`. Climate, water and soil fields change over time under conserved water and nutrient budgets |
```

Proposed:
```markdown
| **World** | A TypeScript world generator driven by one seed: layered noise, erosion, lakes by priority flood, rivers, waterfalls at knickpoints, cave mouths, climate from latitude and altitude, 22 biomes. The world reaches the simulation as a versioned, checksummed `WorldArtifact`. Climate, water and soil fields change over time under conserved water and nutrient budgets. An offline Rust World v2 generator crate (`crates/anima-worldgen`, tectonic uplift + Priority-Flood/MFD-D8 hydrology + coupled Fastscape-SD/thermal erosion) is under staged build-out and not yet wired into `anima-engine` |
```

### `R6` — Fix `LINDEMAN_EFFICIENCY` value (`about 30%` -> `40%`)

- **Target:** `README.md:101`
- **Source commits:** `61d7805` (and `e576350` code in `src-tauri/crates/anima-domain/src/laws.rs:46` & `src-tauri/src/core/ecology.rs:155`)
- **Reason:** Both `anima-domain/src/laws.rs:46` and `core/ecology.rs:155` define `LINDEMAN_EFFICIENCY = 0.40` (40%), matching `PROJECT.md:14`. Saying "about 30%" contradicts the code. Also link [`ENERGY_LEDGER_CONTRACT.md`](../../reference/ENERGY_LEDGER_CONTRACT.md).

Current (`README.md:101`):
```markdown
| **Ecology** | A **closed** energy ledger (EU): plants → herbivores → detritus → plants. Holling type II/III functional responses. Lindeman transfer of about 30%. Logistic primary-production regrowth per biome. Seasons. Mortality, continuous reproduction, locomotion and the ecology keys are optional and off by default ([ADR-0005](docs/decisions/ADR-0005-live-population-turnover.md), [ADR-0006](docs/decisions/ADR-0006-locomotion-actuation.md), [ADR-0007](docs/decisions/ADR-0007-live-ecology-sustainability.md)) |
```

Proposed:
```markdown
| **Ecology** | A **closed** energy ledger (EU, [`ENERGY_LEDGER_CONTRACT.md`](docs/reference/ENERGY_LEDGER_CONTRACT.md)): plants → herbivores → detritus → plants. Holling type II/III functional responses. Lindeman transfer of 40% (`LINDEMAN_EFFICIENCY = 0.40`). Logistic primary-production regrowth per biome. Seasons. Mortality, continuous reproduction, locomotion and the ecology keys are optional and off by default ([ADR-0005](docs/decisions/ADR-0005-live-population-turnover.md), [ADR-0006](docs/decisions/ADR-0006-locomotion-actuation.md), [ADR-0007](docs/decisions/ADR-0007-live-ecology-sustainability.md)) |
```

### `R7` — Add `map2.html` alongside `landscape.html` in Interface row

- **Target:** `README.md:104`
- **Source commit:** `fceb027`
- **Reason:** `vite.config.ts:246-250` builds three entries (`index.html`, `landscape.html`, `map2.html`), and `src/components/Map2/` implements the Map 2 open-world 3D view.

Current (`README.md:104`):
```markdown
| **Interface** | 3D map (`landscape.html`): day and night, weather, PBR terrain materials (CC0), instanced vegetation, several camera modes, four graphics tiers. A PixiJS 8 2D viewport. Ecosystem, evolution, lineage and chronicle panels |
```

Proposed:
```markdown
| **Interface** | 3D maps (`landscape.html`, `map2.html`): day and night, weather, PBR terrain materials (CC0), instanced vegetation, several camera modes, four graphics tiers. A PixiJS 8 2D viewport. Ecosystem, evolution, lineage and chronicle panels |
```

### `R8` — Add contract, ADR, and Map v2 / World v2 pointers to `Where to start`

- **Target:** `README.md:107-122`
- **Source commits:** `491ec61`, `7f2e730` (PR `#192` `d871fd6`), `fe9c942` (PR `#185` `cc40f11`), `dcff379` (PR `#155` `391e5c6`)
- **Reason:** `README.md` omits direct pointers to the reference index (`DETERMINISM_CONTRACT.md`, `ENERGY_LEDGER_CONTRACT.md`, `SNAPSHOT_CONTRACT.md`, `IPC_CONTRACT.md`), the ADR index (`ADR-0001`..`ADR-0010`), and the accepted Map v2 / World v2 planning/design docs. Adding concise map rows follows Policy Rule 4 without duplicating contract prose.

Current (`README.md:117-122`):
```markdown
| Changing genes, bodies or creatures | [Creature Development Contract](docs/reference/CREATURE_DEVELOPMENT_CONTRACT.md) |
| Changing brains or the action space | [ADR-0003](docs/decisions/ADR-0003-evolved-per-agent-brains.md) |
| Running experiments under different world laws | [Evolution Experiment Contract](docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md) |
| A first trial run with outside users | [First trial run](docs/how-to/OWNER_TRIAL_RUN.md) |
| Working as one of the team's agents | [AGENT_WORKFLOW.md](docs/how-to/AGENT_WORKFLOW.md) |
| All documentation | [Documentation hub](docs/README.md) |
```

Proposed:
```markdown
| Changing genes, bodies or creatures | [Creature Development Contract](docs/reference/CREATURE_DEVELOPMENT_CONTRACT.md) |
| Changing brains or the action space | [ADR-0003](docs/decisions/ADR-0003-evolved-per-agent-brains.md) |
| Running experiments under different world laws | [Evolution Experiment Contract](docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md) |
| World v2 & Map v2 roadmap and generator design | [Map v2 plan](docs/ai/requirements/2026-11-20-feature-map-v2.md) and [World v2 design](docs/ai/design/2026-11-20-world-v2-design.md) |
| Reference contracts (determinism, energy, snapshots, IPC) | [Reference index](docs/reference/README.md) |
| Architecture decisions (`ADR-0001`..`ADR-0010`) | [ADR index](docs/decisions/README.md) |
| A first trial run with outside users | [First trial run](docs/how-to/OWNER_TRIAL_RUN.md) |
| Working as one of the team's agents | [AGENT_WORKFLOW.md](docs/how-to/AGENT_WORKFLOW.md) |
| All documentation | [Documentation hub](docs/README.md) |
```

### `R9` — Update `Repository layout` for 3 Vite entries, `anima-worldgen`, `anima-lab`, `licensing/`, `map-views/`

- **Target:** `README.md:147-160`
- **Source commits:** `fceb027`, `f523e3c`, `25cf7f6`, `95b1338`, `920c369`, `84e77c3`
- **Reason:** `vite.config.ts:246-250` has three HTML entries (`index.html`, `landscape.html`, `map2.html`) plus `src/components/Map2/`; `src-tauri/` includes `crates/anima-worldgen/` and `src/bin/anima-lab.rs`; root includes `licensing/` and `map-views/` (`scripts/root_allowlist.json:47-48`).

Current (`README.md:147-160`):
```text
src/                      React + TypeScript (Vite, two entries: index.html, landscape.html)
  components/Landscape/   3D map, world generator, world cache, materials, creatures
  utils/sharedWorld.ts    the one identity of the shared world
src-tauri/                Rust crate `anima-engine` (lib `anima_engine_lib`)
  src/core/               ECS, tick loop, terrain, ecology, energy ledger, snapshots, experiments
  src/ai/                 Burn models, CPG, pheromones
  src/evolution/          MAP-Elites, genotype, mutation, crossover, brain genes, lineage
  src/commands/           the Tauri IPC command surface
  examples/               runners for the preregistered experiments
  crates/anima-domain/    the world laws
tests/                    separate npm package: Vitest (frontend/) and Playwright (e2e/)
docs/                     Diátaxis documentation (tutorial / how-to / reference / explanation)
scripts/                  gates, manifest and map-evidence generators, benchmarks
artifacts/                pinned, hashed artifacts: the 256² world, map evidence, experiments
```

Proposed:
```text
src/                      React + TypeScript (Vite, three entries: index.html, landscape.html, map2.html)
  components/Landscape/   Map 1 3D view, world generator, world cache, materials, creatures
  components/Map2/        Map 2 3D view, camera rig, HUD
  utils/sharedWorld.ts    the one identity of the shared world
src-tauri/                Rust workspace: `anima-engine` (lib `anima_engine_lib`, bin `anima-lab`)
  src/core/               ECS, tick loop, terrain, ecology, energy ledger, snapshots, experiments
  src/ai/                 Burn models, CPG, pheromones
  src/evolution/          MAP-Elites, genotype, mutation, crossover, brain genes, lineage
  src/commands/           the Tauri IPC command surface
  examples/               runners for the preregistered experiments
  crates/anima-domain/    the world laws
  crates/anima-worldgen/  offline World v2 geomorphic generator (tectonic, hydrology, erosion; not yet wired)
tests/                    separate npm package: Vitest (frontend/) and Playwright (e2e/)
docs/                     Diátaxis documentation (tutorial / how-to / reference / explanation)
scripts/                  gates, manifest and map-evidence generators, benchmarks
artifacts/                pinned, hashed artifacts: the 256² world, map evidence, experiments
licensing/                third-party licence texts and bundle closure
map-views/                canonical 1280x720 Map 1 PNG renders referenced by map_manifest.json
```

### `R10` — Keep `Verification` Frontend quick-start concise and link to `CONTRIBUTING.md` (`## Gates`)

- **Target:** `README.md:195-204`
- **Source commits:** `2d7543e`, `19e67fa`, `920c369`, `84e77c3`, `ce558b2`, `7d53660`, `18ae1e6`, `b507bb4`, `427a0e6` (PR `#197` `03b9d3d`), `6adf6e3`
- **Reason:** Per Policy Rule 4, `README.md` should link to [`CONTRIBUTING.md`](../../../CONTRIBUTING.md#gates) and [`.github/workflows/ci.yml`](../../../.github/workflows/ci.yml) instead of copying every CI gate (`eslint_ratchet`, `typecheck:tests`, `check:audit`, `check:text-hygiene`, `check:root-hygiene`, `check:ipc`, `check:e2`, `check:e3`, `check:csp`, `check:bundle`, `check:bundle-closure`, `check_e2e_specs.mjs`, `check:flora-footprint`, `check:compliance`). Note explicitly that `npm run check:compliance` (licenses, SBOM, NOTICE) runs against the Windows host `cargo tree --features desktop` inventory (`.github/workflows/ci.yml:210-213`) and reports stale on macOS/Linux by design — never suggest running it on macOS without that caveat.

Current (`README.md:195-204`):
````markdown
Frontend:

```bash
npm run lint
npm run test
npm run test:frontend
npm run test:e2e
npm run build
npm run check:docs
```
````

Proposed:
````markdown
Frontend (common local loop):

```bash
npm run lint
npm run test
npm run test:frontend
npm run test:e2e
npm run build
npm run check:docs
```

For the full CI gate list (including `eslint_ratchet`, `typecheck:tests`, `check:ipc`, `check:bundle`, `check:flora-footprint`, `check:e2`, `check:e3`, and Windows-only `check:compliance` per `.github/workflows/ci.yml:210-213`), see [`CONTRIBUTING.md`](CONTRIBUTING.md#gates) and [`.github/workflows/ci.yml`](.github/workflows/ci.yml).
````

### `R11` — Link Backend CI gates to `CONTRIBUTING.md` instead of copying the list

- **Target:** `README.md:206-212`
- **Source commits:** `f523e3c` (PR `#193` `cb61b00`), `25cf7f6` (PR `#194` `bfbb9c2`), `427a0e6` (PR `#197` `03b9d3d`), `3c06491`, `043930c`, `9a31a99`, `f402447`
- **Reason:** Per Policy Rule 4, keep the everyday Backend verification commands in `README.md` concise (adding `cargo test -p anima-worldgen --features "reader,generate,bake"` alongside `anima-engine`) and link to [`CONTRIBUTING.md`](../../../CONTRIBUTING.md#gates) and [`.github/workflows/ci.yml`](../../../.github/workflows/ci.yml) for the complete Rust CI sequence (`cargo fmt --check`, `cargo deny check`, `--no-default-features` + `cargo tree` boundary checks, `export_bindings`, `verify_dataflow.py`, and Windows compliance).

Current (`README.md:206-212`):
````markdown
Backend (run in `src-tauri/`):

```bash
cargo test --features desktop
cargo clippy --all-targets --features desktop -- -D warnings
cargo audit
```
````

Proposed:
````markdown
Backend (run in `src-tauri/`):

```bash
cargo test --features desktop
cargo test -p anima-worldgen --features "reader,generate,bake"
cargo clippy --all-targets --features desktop -- -D warnings
cargo audit
```

See [`CONTRIBUTING.md`](CONTRIBUTING.md#gates) and [`.github/workflows/ci.yml`](.github/workflows/ci.yml) for the full CI gate list (`cargo fmt --check`, `cargo deny check`, `cargo tree` isolation checks, `export_bindings`, `dataflow.json`, and Windows-host licensing/SBOM/NOTICE checks).
````

### `R12` — Document `--profile worldgen-cpu` alongside `--profile desktop`

- **Target:** `README.md:216-219`
- **Source commits:** `f523e3c` (PR `#193` `cb61b00`), `427a0e6` (PR `#197` `03b9d3d`)
- **Reason:** `scripts/test_target_policy.mjs:127-131` and `.github/workflows/ci.yml:203` enforce `--profile worldgen-cpu` on `anima-worldgen` test output (covering `wv2_1a_tectonic_tests` and `wv2_1b_solver_hydro_tests`).

Current (`README.md:216-219`):
```markdown
- **`--features desktop` is required.** Without it Cargo does not schedule the test files that need the
  `networking` or `ml-wgpu` features, and the migration, cross-shard and GPU-fallback tests silently do not run.
  `node scripts/check_test_targets.mjs <output> --profile desktop` checks this. Capture the output into one file,
  with both streams in write order.
```

Proposed:
```markdown
- **`--features desktop` (and `--features "reader,generate,bake"` for `anima-worldgen`) are required.** Without them
  Cargo does not schedule the feature-gated test targets (`networking`, `ml-wgpu`, or `generate`).
  `node scripts/check_test_targets.mjs <output> --profile desktop` and `--profile worldgen-cpu` verify this; capture
  output into one file with both streams in write order.
```

### `R13` — Point to operational/diagnostic `ANIMA_*` flags in code (Rule 4)

- **Target:** `README.md:248-249`
- **Source commit:** `e576350` (`src-tauri/src/lib.rs:105,186`, `src-tauri/src/core/resources.rs:602-603,1277,1315`, `src-tauri/src/evolution/fitness.rs:84`, `src-tauri/src/core/tick_capture.rs:368,394`, `src-tauri/src/core/aggregate_population.rs:205`)
- **Reason:** Alongside the 10 simulation variables in the table, code reads operational/diagnostic flags (`ANIMA_FRESH_START`, `ANIMA_AUTOSTART`, `ANIMA_STARVE_TICKS`, `ANIMA_DEHYDRATION_TICKS`, `ANIMA_W_SURPLUS`, `ANIMA_TICK_CAPTURE`, `ANIMA_TICK_CAPTURE_OUT`, `ANIMA_AGGREGATE_LOD`). Per Rule 4, keep the table focused and link to the source files rather than mirroring every env var.

Current (`README.md:248-249`):
```markdown
In experiments the manifest declares the matching values, such as `live.mortality` or `live.land_mask`. A manifest
never reads environment variables.
```

Proposed:
```markdown
Additional operational and diagnostic flags (`ANIMA_FRESH_START`, `ANIMA_AUTOSTART`, `ANIMA_STARVE_TICKS`,
`ANIMA_DEHYDRATION_TICKS`, `ANIMA_W_SURPLUS`, `ANIMA_TICK_CAPTURE*`, `ANIMA_AGGREGATE_LOD`) live in
`src-tauri/src/lib.rs`, `src-tauri/src/core/resources.rs`, `src-tauri/src/core/tick_capture.rs`,
`src-tauri/src/core/aggregate_population.rs`, and `src-tauri/src/evolution/fitness.rs`. In experiments the manifest
declares the matching values, such as `live.mortality` or `live.land_mask`. A manifest never reads environment
variables.
```

### `R14` — Note `npm run check:ipc` alongside `npm run check:docs` in `IPC contract`

- **Target:** `README.md:255-257`
- **Source commits:** `19e67fa`, `427a0e6` (PR `#197` `03b9d3d`)
- **Reason:** `npm run check:docs` verifies `IPC_CONTRACT.md` command/event parity (`check_ipc_reference.mjs`), while `npm run check:ipc` (`check_ipc_arg_case.mjs`, wired into `.github/workflows/ci.yml:330`) verifies camelCase argument keys at every frontend `invoke(...)` call site.

Current (`README.md:255-257`):
```markdown
`load_simulation_state`, and the `simulation-tick` event. The full list with payloads is in
[`docs/reference/IPC_CONTRACT.md`](docs/reference/IPC_CONTRACT.md). Update it in the same change whenever you change
the IPC surface: `npm run check:docs` fails when the list and the code disagree.
```

Proposed (`README.md:255-257`):
```markdown
`load_simulation_state`, and the `simulation-tick` event. The full list with payloads is in
[`docs/reference/IPC_CONTRACT.md`](docs/reference/IPC_CONTRACT.md). Update it in the same change whenever you change
the IPC surface: `npm run check:docs` fails when the list and the code disagree, and `npm run check:ipc` checks
camelCase argument keys at every frontend `invoke(...)` call site.
```

---

## 5. Proposed Parity Updates to `README.vi.md` (`V1`–`V14`)

`README.vi.md` (315 lines at `e576350`, created in `d2ae358` / PR `#187` `c674db8`) is the Vietnamese translation of `README.md` and exhibits the matching discrepancies. Each `V#` corresponds 1-to-1 with `R#` and cites the same verified trunk commits.

### `V1` — Optional link to `DETERMINISM_CONTRACT.md` (`F051` already landed in `#213`, `README.vi.md:59-62`)

- **Target:** `README.vi.md:59-62`
- **Source commits:** `62c70cc` (PR `#174`), `6a9b9a2` (PR `#179`), `4fe3d7a` (PR `#213` `e576350`)
- **Reason:** Matches `R1`; `#213` (`4fe3d7a`) already updated `README.vi.md:60-62` for F051.

Current (`e576350:README.vi.md:59-62`):
```markdown
- **Tái lập được.** Cùng hạt giống, cùng commit và cùng hệ điều hành thì cho cùng checksum. Giữa các hệ điều hành,
  quỹ đạo mô phỏng có khác, nên test luôn so hai lượt chạy với nhau, không ghim một con số tuyệt đối. Lượt chạy
  không giao diện tự sinh địa hình và không bao giờ đọc file thế giới của app (thẻ F051); mỗi lượt ghi lại địa hình
  nó đã dùng.
```

Proposed:
```markdown
- **Tái lập được.** Cùng hạt giống, cùng commit và cùng hệ điều hành thì cho cùng checksum. Giữa các hệ điều hành,
  quỹ đạo mô phỏng có khác, nên test luôn so hai lượt chạy với nhau, không ghim một con số tuyệt đối. Lượt chạy
  không giao diện tự sinh địa hình và không bao giờ đọc file thế giới của app (thẻ F051); mỗi lượt ghi lại địa hình
  nó đã dùng ([`DETERMINISM_CONTRACT.md`](docs/reference/DETERMINISM_CONTRACT.md)).
```

### `V2` — Update boundaries section date (`README.vi.md:65`)

- **Target:** `README.vi.md:65`
- **Source commit:** `e576350`
- **Reason:** Matches `R2`.

Current (`README.vi.md:65`):
```markdown
## Ranh giới hiện tại (2026-11-19)
```

Proposed:
```markdown
## Ranh giới hiện tại (2026-11-23)
```

### `V3` — Update scientific experiment boundary (`README.vi.md:72-77`)

- **Target:** `README.vi.md:72-77`
- **Source commits:** `e9c92dc` (PR `#173`), `d90be0b` (PR `#181`), `1ef4f0d` (PR `#183`), `a3d7ab6` (PR `#191` `d64e536`)
- **Reason:** Matches `R3` (link directly to E3 [`summary.md`](../../../artifacts/experiments/e3-productivity-gradient-body-mass/confirmation/summary.md) and [`STATE §3`](../../planning/STATE_OF_THE_PROJECT.md) without duplicating experimental run counts).

Current (`README.vi.md:72-77`):
```markdown
- **Chưa có kết quả khoa học có sức mạnh.**
  - Hạ tầng thí nghiệm đã có.
  - Thí nghiệm E2 cho kết quả null vì hiệu ứng sàn.
  - Bản đăng ký E-SEL1 bị đóng trước khi chạy.
  - Tình trạng của SEL2-P (thí nghiệm đăng ký trước đầu tiên trên thế giới sống) và E3 nằm ở
    [STATE §3](docs/planning/STATE_OF_THE_PROJECT.md).
```

Proposed:
```markdown
- **Chưa có kết quả khoa học có sức mạnh.**
  - Hạ tầng thí nghiệm đã có.
  - Thí nghiệm E2 cho kết quả null vì hiệu ứng sàn, và bản đăng ký E-SEL1 bị đóng trước khi chạy.
  - SEL2-P dừng ở cổng smoke (`gate_pass=false`), còn E3 kết thúc vòng xác nhận với kết luận
    `insufficient_evidence` ([E3 summary.md](artifacts/experiments/e3-productivity-gradient-body-mass/confirmation/summary.md)). Kết quả và việc tiếp nối (VIAB-1, E3b, SEL2-P2) nằm ở [STATE §3](docs/planning/STATE_OF_THE_PROJECT.md).
```

### `V4` — Document additional opt-in `live.*` controls (`README.vi.md:83-85`)

- **Target:** `README.vi.md:83-85`
- **Source commits:** `4794b6f`, `7bdddf7`, `f2887f1`, `eedf577`, `5cc4675`
- **Reason:** Matches `R4` (`LIVE_KEYS` in `src-tauri/src/core/live_experiment.rs:255`).

Current (`README.vi.md:83-85`):
```markdown
- **Tử vong, fitness sinh thái, sinh sản liên tục và bộ lái vận động có trong mã nhưng tắt theo mặc định.** Một
  thế giới mặc định vẫn không có cái chết, và fitness được chấm bằng quãng đường. Luật "chỉ đi trên đất" thì bật
  sẵn trong app, còn trong thí nghiệm thì phải bật bằng khoá `live.land_mask`.
```

Proposed:
```markdown
- **Tử vong, fitness sinh thái, sinh sản liên tục và bộ lái vận động có trong mã nhưng tắt theo mặc định.** Một
  thế giới mặc định vẫn không có cái chết, và fitness được chấm bằng quãng đường. Luật "chỉ đi trên đất" thì bật
  sẵn trong app, còn trong thí nghiệm thì bật bằng `live.land_mask`, và các điều khiển sinh thái/khởi tạo mới hơn
  (`live.strict_prey_cap`, `live.grass_steering`, `live.founder_layout`, `live.lake_layout`) cũng là khoá manifest
  opt-in `live.*` (`src-tauri/src/core/live_experiment.rs`).
```

### `V5` — Mention offline `anima-worldgen` World v2 crate (`README.vi.md:95`)

- **Target:** `README.vi.md:95`
- **Source commits:** `f523e3c`, `25cf7f6`, `95b1338`, `7f2e730`
- **Reason:** Matches `R5` (`WV2-1a..1c` only; excludes unmerged `#199`).

Current (`README.vi.md:95`):
```markdown
| **Thế giới** | Bộ sinh thế giới TypeScript chạy từ một hạt giống: noise nhiều lớp, xói mòn, hồ sinh bằng *priority flood*, sông, thác ở các điểm gãy, cửa hang, khí hậu theo vĩ độ và độ cao, 22 biome. Thế giới được giao sang mô phỏng dưới dạng `WorldArtifact` có phiên bản và checksum. Trường khí hậu, nước và đất biến động theo thời gian, với ngân sách nước và dinh dưỡng được bảo toàn |
```

Proposed:
```markdown
| **Thế giới** | Bộ sinh thế giới TypeScript chạy từ một hạt giống: noise nhiều lớp, xói mòn, hồ sinh bằng *priority flood*, sông, thác ở các điểm gãy, cửa hang, khí hậu theo vĩ độ và độ cao, 22 biome. Thế giới được giao sang mô phỏng dưới dạng `WorldArtifact` có phiên bản và checksum. Trường khí hậu, nước và đất biến động theo thời gian, với ngân sách nước và dinh dưỡng được bảo toàn. Một crate Rust sinh World v2 ngoại tuyến (`crates/anima-worldgen`, kiến tạo + thuỷ văn Priority-Flood/MFD-D8 + xói mòn Fastscape-SD/nhiệt) đang được dựng theo chặng và chưa nối vào `anima-engine` |
```

### `V6` — Fix `LINDEMAN_EFFICIENCY` value (`README.vi.md:99`)

- **Target:** `README.vi.md:99`
- **Source commit:** `61d7805` (and `e576350` code in `laws.rs:46`, `ecology.rs:155`)
- **Reason:** Matches `R6` (`0.40` / 40%, not "khoảng 30%").

Current (`README.vi.md:99`):
```markdown
| **Sinh thái** | Sổ năng lượng **khép kín** (EU): thực vật → ăn cỏ → xác mục → thực vật. Phản ứng chức năng Holling II/III. Hiệu suất chuyển hoá Lindeman khoảng 30%. Năng suất sơ cấp tái sinh logistic theo biome. Chu kỳ mùa. Tử vong, sinh sản liên tục, vận động và các khoá sinh thái là tuỳ chọn, mặc định tắt ([ADR-0005](docs/decisions/ADR-0005-live-population-turnover.md), [ADR-0006](docs/decisions/ADR-0006-locomotion-actuation.md), [ADR-0007](docs/decisions/ADR-0007-live-ecology-sustainability.md)) |
```

Proposed:
```markdown
| **Sinh thái** | Sổ năng lượng **khép kín** (EU, [`ENERGY_LEDGER_CONTRACT.md`](docs/reference/ENERGY_LEDGER_CONTRACT.md)): thực vật → ăn cỏ → xác mục → thực vật. Phản ứng chức năng Holling II/III. Hiệu suất chuyển hoá Lindeman 40% (`LINDEMAN_EFFICIENCY = 0.40`). Năng suất sơ cấp tái sinh logistic theo biome. Chu kỳ mùa. Tử vong, sinh sản liên tục, vận động và các khoá sinh thái là tuỳ chọn, mặc định tắt ([ADR-0005](docs/decisions/ADR-0005-live-population-turnover.md), [ADR-0006](docs/decisions/ADR-0006-locomotion-actuation.md), [ADR-0007](docs/decisions/ADR-0007-live-ecology-sustainability.md)) |
```

### `V7` — Add `map2.html` in Giao diện row (`README.vi.md:102`)

- **Target:** `README.vi.md:102`
- **Source commit:** `fceb027`
- **Reason:** Matches `R7`.

Current (`README.vi.md:102`):
```markdown
| **Giao diện** | Bản đồ 3D (`landscape.html`): ngày đêm, thời tiết, vật liệu địa hình PBR (CC0), thảm thực vật instanced, nhiều chế độ camera, bốn mức đồ hoạ. Viewport 2D PixiJS 8. Các bảng hệ sinh thái, tiến hoá, phả hệ và biên niên |
```

Proposed:
```markdown
| **Giao diện** | Bản đồ 3D (`landscape.html`, `map2.html`): ngày đêm, thời tiết, vật liệu địa hình PBR (CC0), thảm thực vật instanced, nhiều chế độ camera, bốn mức đồ hoạ. Viewport 2D PixiJS 8. Các bảng hệ sinh thái, tiến hoá, phả hệ và biên niên |
```

### `V8` — Add contract, ADR, and Map v2 / World v2 pointers (`README.vi.md:115-120`)

- **Target:** `README.vi.md:115-120`
- **Source commits:** `491ec61`, `7f2e730`, `fe9c942`, `dcff379`
- **Reason:** Matches `R8`.

Current (`README.vi.md:115-120`):
```markdown
| Sửa gen, thân hay sinh vật | [Creature Development Contract](docs/reference/CREATURE_DEVELOPMENT_CONTRACT.md) |
| Sửa bộ não hoặc không gian hành động | [ADR-0003](docs/decisions/ADR-0003-evolved-per-agent-brains.md) |
| Chạy thí nghiệm với luật thế giới khác | [Evolution Experiment Contract](docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md) |
| Chạy thử app với người ngoài | [Buổi dùng thử đầu tiên](docs/how-to/OWNER_TRIAL_RUN.md) |
| Làm việc như một agent trong nhóm | [AGENT_WORKFLOW.md](docs/how-to/AGENT_WORKFLOW.md) |
| Toàn bộ tài liệu | [Trung tâm tài liệu](docs/README.md) |
```

Proposed:
```markdown
| Sửa gen, thân hay sinh vật | [Creature Development Contract](docs/reference/CREATURE_DEVELOPMENT_CONTRACT.md) |
| Sửa bộ não hoặc không gian hành động | [ADR-0003](docs/decisions/ADR-0003-evolved-per-agent-brains.md) |
| Chạy thí nghiệm với luật thế giới khác | [Evolution Experiment Contract](docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md) |
| Lộ trình Map v2 và thiết kế World v2 | [Kế hoạch Map v2](docs/ai/requirements/2026-11-20-feature-map-v2.md) và [Thiết kế World v2](docs/ai/design/2026-11-20-world-v2-design.md) |
| Hợp đồng tham chiếu (tất định, năng lượng, snapshot, IPC) | [Chỉ mục reference](docs/reference/README.md) |
| Quyết định kiến trúc (`ADR-0001`..`ADR-0010`) | [Chỉ mục ADR](docs/decisions/README.md) |
| Chạy thử app với người ngoài | [Buổi dùng thử đầu tiên](docs/how-to/OWNER_TRIAL_RUN.md) |
| Làm việc như một agent trong nhóm | [AGENT_WORKFLOW.md](docs/how-to/AGENT_WORKFLOW.md) |
| Toàn bộ tài liệu | [Trung tâm tài liệu](docs/README.md) |
```

### `V9` — Update `Bố cục kho mã` (`README.vi.md:144-157`)

- **Target:** `README.vi.md:144-157`
- **Source commits:** `fceb027`, `f523e3c`, `25cf7f6`, `95b1338`, `920c369`, `84e77c3`
- **Reason:** Matches `R9`.

Current (`README.vi.md:144-157`):
```text
src/                      React + TypeScript (Vite, hai entry: index.html, landscape.html)
  components/Landscape/   bản đồ 3D, bộ sinh thế giới, cache thế giới, vật liệu, sinh vật
  utils/sharedWorld.ts    danh tính duy nhất của thế giới dùng chung
src-tauri/                crate Rust `anima-engine` (lib `anima_engine_lib`)
  src/core/               ECS, vòng tick, địa hình, sinh thái, sổ năng lượng, snapshot, thí nghiệm
  src/ai/                 mô hình Burn, CPG, pheromone
  src/evolution/          MAP-Elites, gen, đột biến, lai ghép, gen não, phả hệ
  src/commands/           bề mặt lệnh Tauri IPC
  examples/               bộ chạy thí nghiệm đăng ký trước
  crates/anima-domain/    các luật của thế giới
tests/                    gói npm riêng: Vitest (frontend/) và Playwright (e2e/)
docs/                     tài liệu theo Diátaxis (tutorial / how-to / reference / explanation)
scripts/                  cổng kiểm tra, sinh manifest và bằng chứng bản đồ, benchmark
artifacts/                artifact đã ghim và băm: thế giới 256², bằng chứng bản đồ, thí nghiệm
```

Proposed:
```text
src/                      React + TypeScript (Vite, ba entry: index.html, landscape.html, map2.html)
  components/Landscape/   góc nhìn 3D Map 1, bộ sinh thế giới, cache thế giới, vật liệu, sinh vật
  components/Map2/        góc nhìn 3D Map 2, camera rig, HUD
  utils/sharedWorld.ts    danh tính duy nhất của thế giới dùng chung
src-tauri/                workspace Rust: `anima-engine` (lib `anima_engine_lib`, bin `anima-lab`)
  src/core/               ECS, vòng tick, địa hình, sinh thái, sổ năng lượng, snapshot, thí nghiệm
  src/ai/                 mô hình Burn, CPG, pheromone
  src/evolution/          MAP-Elites, gen, đột biến, lai ghép, gen não, phả hệ
  src/commands/           bề mặt lệnh Tauri IPC
  examples/               bộ chạy thí nghiệm đăng ký trước
  crates/anima-domain/    các luật của thế giới
  crates/anima-worldgen/  bộ sinh World v2 ngoại tuyến (kiến tạo, thuỷ văn, xói mòn; chưa nối vào engine)
tests/                    gói npm riêng: Vitest (frontend/) và Playwright (e2e/)
docs/                     tài liệu theo Diátaxis (tutorial / how-to / reference / explanation)
scripts/                  cổng kiểm tra, sinh manifest và bằng chứng bản đồ, benchmark
artifacts/                artifact đã ghim và băm: thế giới 256², bằng chứng bản đồ, thí nghiệm
licensing/                văn bản giấy phép bên thứ ba và bundle closure
map-views/                tám ảnh PNG chuẩn 1280x720 của Map 1 ghi trong map_manifest.json
```

### `V10` — Keep `Kiểm chứng` Frontend concise and link to `CONTRIBUTING.md` (`## Gates`, `README.vi.md:192-201`)

- **Target:** `README.vi.md:192-201`
- **Source commits:** `2d7543e`, `19e67fa`, `920c369`, `84e77c3`, `ce558b2`, `7d53660`, `18ae1e6`, `b507bb4`, `427a0e6`, `6adf6e3`
- **Reason:** Matches `R10` (links to [`CONTRIBUTING.md`](../../../CONTRIBUTING.md#gates) and [`.github/workflows/ci.yml`](../../../.github/workflows/ci.yml) instead of copying the full gate list, with the explicit Windows-only note for `check:compliance` per `.github/workflows/ci.yml:210-213`).

Current (`README.vi.md:192-201`):
````markdown
Frontend:

```bash
npm run lint
npm run test
npm run test:frontend
npm run test:e2e
npm run build
npm run check:docs
```
````

Proposed:
````markdown
Frontend (vòng kiểm tra cục bộ thường dùng):

```bash
npm run lint
npm run test
npm run test:frontend
npm run test:e2e
npm run build
npm run check:docs
```

Danh sách đầy đủ các cổng CI (gồm `eslint_ratchet`, `typecheck:tests`, `check:ipc`, `check:bundle`, `check:flora-footprint`, `check:e2`, `check:e3`, và `check:compliance` chỉ chạy trên Windows theo `.github/workflows/ci.yml:210-213`) nằm tại [`CONTRIBUTING.md`](CONTRIBUTING.md#gates) và [`.github/workflows/ci.yml`](.github/workflows/ci.yml).
````

### `V11` — Link `Kiểm chứng` Backend CI gates to `CONTRIBUTING.md` (`README.vi.md:203-209`)

- **Target:** `README.vi.md:203-209`
- **Source commits:** `f523e3c`, `25cf7f6`, `427a0e6`, `3c06491`, `043930c`, `9a31a99`, `f402447`
- **Reason:** Matches `R11`.

Current (`README.vi.md:203-209`):
````markdown
Backend (chạy trong `src-tauri/`):

```bash
cargo test --features desktop
cargo clippy --all-targets --features desktop -- -D warnings
cargo audit
```
````

Proposed:
````markdown
Backend (chạy trong `src-tauri/`):

```bash
cargo test --features desktop
cargo test -p anima-worldgen --features "reader,generate,bake"
cargo clippy --all-targets --features desktop -- -D warnings
cargo audit
```

Xem [`CONTRIBUTING.md`](CONTRIBUTING.md#gates) và [`.github/workflows/ci.yml`](.github/workflows/ci.yml) để biết chuỗi cổng Rust đầy đủ trên CI (`cargo fmt --check`, `cargo deny check`, `cargo tree` cô lập feature, `export_bindings`, `dataflow.json`, và các cổng license/SBOM/NOTICE trên Windows).
````

### `V12` — Document `--profile worldgen-cpu` (`README.vi.md:213-216`)

- **Target:** `README.vi.md:213-216`
- **Source commits:** `f523e3c`, `427a0e6`
- **Reason:** Matches `R12`.

Current (`README.vi.md:213-216`):
```markdown
- **`--features desktop` là bắt buộc.** Thiếu nó, Cargo không lên lịch các file test cần feature
  `networking` hay `ml-wgpu`, và các test migration, cross-shard, GPU-fallback lặng lẽ không chạy.
  `node scripts/check_test_targets.mjs <output> --profile desktop` kiểm tra điều này. Hứng output vào một file,
  gộp cả hai luồng theo đúng thứ tự.
```

Proposed:
```markdown
- **`--features desktop` (và `--features "reader,generate,bake"` cho `anima-worldgen`) là bắt buộc.** Thiếu chúng,
  Cargo không lên lịch các target test bị gate theo feature (`networking`, `ml-wgpu`, hoặc `generate`).
  `node scripts/check_test_targets.mjs <output> --profile desktop` và `--profile worldgen-cpu` kiểm tra điều này;
  hứng output vào một file, gộp cả hai luồng theo đúng thứ tự.
```

### `V13` — Point to operational/diagnostic `ANIMA_*` flags (`README.vi.md:245-246`)

- **Target:** `README.vi.md:245-246`
- **Source commit:** `e576350` (`resources.rs`, `tick_capture.rs`, `fitness.rs`, `lib.rs`, `aggregate_population.rs`)
- **Reason:** Matches `R13`.

Current (`README.vi.md:245-246`):
```markdown
Trong thí nghiệm, manifest khai báo các giá trị tương ứng, chẳng hạn `live.mortality` hay `live.land_mask`.
Manifest không đọc biến môi trường.
```

Proposed:
```markdown
Các cờ vận hành và chẩn đoán bổ sung (`ANIMA_FRESH_START`, `ANIMA_AUTOSTART`, `ANIMA_STARVE_TICKS`,
`ANIMA_DEHYDRATION_TICKS`, `ANIMA_W_SURPLUS`, `ANIMA_TICK_CAPTURE*`, `ANIMA_AGGREGATE_LOD`) nằm ở
`src-tauri/src/lib.rs`, `src-tauri/src/core/resources.rs`, `src-tauri/src/core/tick_capture.rs`,
`src-tauri/src/core/aggregate_population.rs` và `src-tauri/src/evolution/fitness.rs`. Trong thí nghiệm, manifest
khai báo các giá trị tương ứng, chẳng hạn `live.mortality` hay `live.land_mask`. Manifest không đọc biến môi trường.
```

### `V14` — Note `npm run check:ipc` (`README.vi.md:252-254`)

- **Target:** `README.vi.md:252-254`
- **Source commits:** `19e67fa`, `427a0e6`
- **Reason:** Matches `R14`.

Current (`README.vi.md:252-254`):
```markdown
`simulation-tick`. Danh sách đầy đủ kèm payload nằm ở
[`docs/reference/IPC_CONTRACT.md`](docs/reference/IPC_CONTRACT.md). Hãy cập nhật nó trong cùng thay đổi khi sửa
bề mặt IPC: `npm run check:docs` đỏ khi danh sách lệch với mã.
```

Proposed (`README.vi.md:252-254`):
```markdown
`simulation-tick`. Danh sách đầy đủ kèm payload nằm ở
[`docs/reference/IPC_CONTRACT.md`](docs/reference/IPC_CONTRACT.md). Hãy cập nhật nó trong cùng thay đổi khi sửa
bề mặt IPC: `npm run check:docs` đỏ khi danh sách lệch với mã, và `npm run check:ipc` kiểm tra tên tham số
camelCase ở mọi điểm gọi `invoke(...)` phía giao diện.
```

---

## 6. Proposed Code-Driven Changes to `CLAUDE.md` (`C1`–`C11`)

Per `CARD DOC7`, proposals for `CLAUDE.md` (193 lines at `e576350`) are strictly limited to changes made necessary by trunk code: new workspace crates, new commands, new CI gates, or correcting factually stale build/mock/script statements. Note that PR `#213` (`4fe3d7a`) already landed several of these fixes (`C1`, `C5`, `C7`, `C10`, plus most of `C4`) directly onto `e576350`; those rows record `ALREADY LANDED IN #213` while preserving their historical IDs.

### `C1` — Add `crates/anima-worldgen` to workspace package summary *(ALREADY LANDED IN `#213` `4fe3d7a`)*

- **Target:** `CLAUDE.md:13`
- **Source commits:** `f523e3c` (PR `#193` `cb61b00`), `25cf7f6` (PR `#194` `bfbb9c2`), `95b1338` (PR `#195` `45238ff`), `4fe3d7a` (PR `#213` `e576350`)
- **Status at `e576350`:** Already updated on trunk (`CLAUDE.md:13` now lists `anima-engine`, `crates/anima-domain`, and `crates/anima-worldgen, the offline World v2 generator`). Only an optional phrase noting `(not yet wired into anima-engine)` could be appended.

### `C2` — Update Vite entry count (`two entries` -> `three entries`)

- **Target:** `CLAUDE.md:19`
- **Source commit:** `fceb027`
- **Reason:** `vite.config.ts:246-250` builds three entries: `index.html`, `landscape.html`, and `map2.html`.

Current (`CLAUDE.md:19`):
```markdown
- `npm run build` — `tsc && vite build`; typechecks first, builds two entries (`index.html`, `landscape.html`).
```

Proposed:
```markdown
- `npm run build` — `tsc && vite build`; typechecks first, builds three entries (`index.html`, `landscape.html`, `map2.html`).
```

### `C3` — Add missing Frontend CI gates to `## Commands`

- **Target:** `CLAUDE.md:24-27`
- **Source commits:** `920c369`, `84e77c3`, `ce558b2`, `7d53660`, `19e67fa`, `18ae1e6`, `b507bb4`, `427a0e6` (PR `#197` `03b9d3d`), `6adf6e3`
- **Reason:** `.github/workflows/ci.yml:229-261,299-440` and `package.json:25-51` run these mandatory gates in CI, yet they are omitted from the Frontend commands list. Keeping `- node scripts/gen_notice.mjs [--check]` intact preserves the regeneration command while cross-referencing `check:notice` inside `check:compliance`.

Current (`CLAUDE.md:24-27`):
```markdown
- `npm run check:bundle` — per-chunk and total JS budgets. Raising a budget is a decision: change the number in the same commit that causes the growth, and say why.
- `node scripts/gen_notice.mjs [--check]` — regenerates `NOTICE` from `cargo tree --features desktop -e normal` plus npm `dependencies`. `--check` fails when it is stale.
- `npm run check:docs` — docs link check plus `scripts/check_docs_metadata.mjs`: required front matter, allowed `status`/`review_cycle` values, a README index in every `docs/` directory, and every document linked from an index — except that a note under `docs/ai/` may arrive **without** a row in `docs/ai/README.md`: it is *pending* when its front matter has `title`, `status` and `description`, and `npm run gen:docs-index` (run by the lead in a docs PR) appends its row. Agents do not edit that README. Both scripts also read untracked files, so a new document is checked before it is staged. The metadata checker runs its own negative controls first and exits 2 if one of them passes. The rules it enforces are in `docs/governance/DOCUMENTATION_POLICY.md` ("Metadata tối thiểu", and "Chỉ mục sinh" for the generated index, whose logic lives in `scripts/lib/docs_index.mjs`); change them together. `--report` lists overdue reviews, which are reported but never fail the run.
- `npm run gen:world-manifest` — regenerates `map_manifest.json` **and** `artifacts/world_256.anmw` together, with a real SHA-256. Never hand-edit that manifest; `tests/frontend/mapManifestEvidence.test.ts` hashes the bytes it names.
```

Proposed (`CLAUDE.md:24-27`):
```markdown
- `npm run check:bundle` — per-chunk and total JS budgets. Raising a budget is a decision: change the number in the same commit that causes the growth, and say why.
- `npm run typecheck:tests` — `tsc -p tests/tsconfig.json --noEmit`; CI runs this before Vitest so test mocks cannot silently drift from generated types.
- `npm run check:audit` — npm advisory gate across root and `tests/` (`scripts/check_npm_audit.mjs`, policy in `scripts/npm_audit_policy.mjs`).
- `npm run check:text-hygiene` — fails on raw ASCII control bytes in tracked/non-ignored text files (`scripts/check_text_hygiene.mjs`).
- `npm run check:root-hygiene` — enforces `scripts/root_allowlist.json` on git-tracked repository root entries.
- `npm run check:e2` / `npm run check:e3` — verify committed E2 / E3 experiment artifacts against their preregistrations.
- `npm run check:ipc` — verifies camelCase argument keys at every frontend `invoke(...)` call site (`scripts/check_ipc_arg_case.mjs`).
- `npm run check:bundle-closure` — verifies `licensing/bundle-closure.json` matches a fresh `dist/` build.
- `node scripts/check_e2e_specs.mjs && npm run test:e2e` — verifies Playwright schedules the committed spec policy (`scripts/e2e_spec_policy.mjs`) and runs browser-scope E2E against `tests/e2e/tauri-mock.ts`.
- `npm run check:flora-footprint` — verifies declared flora radii in `floraClearance.ts` match headless `three` geometry (not part of `check:compliance`).
- `npm run check:compliance` — checks `check:licenses`, `check:sbom`, `check:sbom-schema` (CycloneDX 1.5), `check:notice` (which runs `node scripts/gen_notice.mjs --check` below), and `check:text-hygiene`. Targets the Windows `cargo tree --features desktop` graph (`.github/workflows/ci.yml:210-213`), so `check:licenses`, `check:sbom`, and `check:notice` report stale off Windows by design.
- `node scripts/gen_notice.mjs [--check]` — regenerates `NOTICE` from `cargo tree --features desktop -e normal` plus npm `dependencies`. `--check` fails when it is stale.
- `npm run check:docs` — docs link check plus `scripts/check_docs_metadata.mjs`: required front matter, allowed `status`/`review_cycle` values, a README index in every `docs/` directory, and every document linked from an index — except that a note under `docs/ai/` may arrive **without** a row in `docs/ai/README.md`: it is *pending* when its front matter has `title`, `status` and `description`, and `npm run gen:docs-index` (run by the lead in a docs PR) appends its row. Agents do not edit that README. Both scripts also read untracked files, so a new document is checked before it is staged. The metadata checker runs its own negative controls first and exits 2 if one of them passes. The rules it enforces are in `docs/governance/DOCUMENTATION_POLICY.md` ("Metadata tối thiểu", and "Chỉ mục sinh" for the generated index, whose logic lives in `scripts/lib/docs_index.mjs`); change them together. `--report` lists overdue reviews, which are reported but never fail the run.
- `npm run gen:world-manifest` — regenerates `map_manifest.json` **and** `artifacts/world_256.anmw` together, with a real SHA-256. Never hand-edit that manifest; `tests/frontend/mapManifestEvidence.test.ts` hashes the bytes it names.
````

### `C4` — Keep full `--features desktop` rationale (`2,040` lines, already updated in `#213`) and note 2 `anima-worldgen` gated targets

- **Target:** `CLAUDE.md:34`
- **Source commits:** `f523e3c` (PR `#193` `cb61b00`), `25cf7f6` (PR `#194` `bfbb9c2`), `95b1338` (PR `#195` `45238ff`), `4fe3d7a` (PR `#213` `e576350`)
- **Reason:** PR `#213` (`4fe3d7a`) already updated `e576350:CLAUDE.md:34` to **`2,040`** lines (`scripts/check_test_targets.mjs:15`), added the separate `cargo test -p anima-worldgen --features "reader,generate,bake"` (`--profile worldgen-cpu`) note, and **preserved the full `--features desktop` rationale**. Any remaining proposal only adds the count of two `generate`-gated `anima-worldgen` targets (`scripts/test_target_policy.mjs:122-123` and `src-tauri/tests/feature_gated_targets_tests.rs:25-32`) while keeping the entire `--features desktop` explanation and `2,040` count intact.

Current (`e576350:CLAUDE.md:34`):
```markdown
- `cargo test --features desktop` — unit + integration tests, across the workspace's default members (`default-members` in `Cargo.toml`; without it Cargo would run the root package only). It does **not** run `anima-worldgen`'s tests: every module and test target there needs its `generate` feature, so CI runs them in a separate step, `cargo test -p anima-worldgen --features "reader,generate,bake"`, checked with `--profile worldgen-cpu`. **Pass `--features desktop`, which is what CI runs:** eight test files carry a crate-level `#![cfg(feature = "networking")]` or `#![cfg(feature = "ml-wgpu")]` **and** a matching `[[test]] required-features` entry, so a bare `cargo test` does not schedule them at all rather than compiling them into empty binaries that report `running 0 tests` and exit 0. That silently skipped 2,040 lines of migration, cross-shard and GPU-fallback coverage. `tests/feature_gated_targets_tests.rs` keeps the two halves of that declaration from drifting.
```

Proposed (preserves the full `e576350` rationale and `2,040` line count verbatim, only noting the 2 `anima-worldgen` gated targets in the last sentence):
```markdown
- `cargo test --features desktop` — unit + integration tests, across the workspace's default members (`default-members` in `Cargo.toml`; without it Cargo would run the root package only). It does **not** run `anima-worldgen`'s tests: every module and test target there needs its `generate` feature, so CI runs them in a separate step, `cargo test -p anima-worldgen --features "reader,generate,bake"`, checked with `--profile worldgen-cpu`. **Pass `--features desktop`, which is what CI runs:** eight test files carry a crate-level `#![cfg(feature = "networking")]` or `#![cfg(feature = "ml-wgpu")]` **and** a matching `[[test]] required-features` entry, so a bare `cargo test` does not schedule them at all rather than compiling them into empty binaries that report `running 0 tests` and exit 0. That silently skipped 2,040 lines of migration, cross-shard and GPU-fallback coverage. `tests/feature_gated_targets_tests.rs` keeps those declarations (plus two `generate`-gated targets in `crates/anima-worldgen/tests/`) in sync with `[[test]] required-features`.
```

### `C5` — Add `worldgen-cpu` profile to `check_test_targets.mjs` *(ALREADY LANDED IN `#213` `4fe3d7a`)*

- **Target:** `CLAUDE.md:35`
- **Status at `e576350`:** Already updated in `4fe3d7a` (`--profile {default|desktop|worldgen-cpu}`). No change needed.

### `C6` — Document exact CI `cargo clippy` invocations

- **Target:** `CLAUDE.md:36`
- **Source commits:** `f523e3c` (PR `#193` `cb61b00`), `78b2e85`
- **Reason:** `.github/workflows/ci.yml:126,130,139` enforces `-D warnings` across three feature configurations: `--features desktop`, `--no-default-features`, and `-p anima-worldgen --features reader,generate,bake`.

Current (`CLAUDE.md:36`):
```markdown
- `cargo clippy` — Rust linter (ships with the toolchain).
```

Proposed:
```markdown
- `cargo clippy --all-targets --features desktop -- -D warnings` (plus `--no-default-features` and `-p anima-worldgen --all-targets --features reader,generate,bake -- -D warnings` in CI).
```

### `C7` — Playwright E2E does not need the release binary *(ALREADY LANDED IN `#213` `4fe3d7a`)*

- **Target:** `CLAUDE.md:37`
- **Status at `e576350`:** Already corrected in `4fe3d7a` (`CLAUDE.md:37` now states: `Playwright E2E does **not** need it: test:e2e runs against the Vite dev server with the Tauri mock in tests/e2e/tauri-mock.ts (tests/e2e/playwright.config.ts).`, with `tests/e2e/real_backend.spec.ts` using `ANIMA_E2E_REQUIRE_BACKEND=1` optionally). No further edit needed unless noting `ANIMA_E2E_REQUIRE_BACKEND=1`.

### `C8` — Add `cargo fmt --check`, `cargo deny check`, `export_bindings`, and `dataflow` gate to Backend commands

- **Target:** after `CLAUDE.md:37` (retaining `e576350:CLAUDE.md:37` / `C7` unchanged above the inserted lines)
- **Source commits:** `f523e3c`, `427a0e6` (PR `#197` `03b9d3d`), `3c06491`, `043930c`, `9a31a99`, `f402447`
- **Reason:** `.github/workflows/ci.yml:92,122,157-158,263-264` runs all four as mandatory Rust CI gates (`src-tauri/deny.toml`, `src/types/generated/`, `dataflow.json`).

Current (`e576350:CLAUDE.md:37`):
```markdown
- `cargo build --release --features desktop` — the release desktop binary. Playwright E2E does **not** need it: `test:e2e` runs against the Vite dev server with the Tauri mock in `tests/e2e/tauri-mock.ts` (`tests/e2e/playwright.config.ts`).
```

Proposed (keeping `e576350:CLAUDE.md:37` verbatim and inserting the 4 CI commands below it):
```markdown
- `cargo build --release --features desktop` — the release desktop binary. Playwright E2E does **not** need it: `test:e2e` runs against the Vite dev server with the Tauri mock in `tests/e2e/tauri-mock.ts` (`tests/e2e/playwright.config.ts`).
- `cargo fmt --check` — formatting gate (first step of Rust CI).
- `cargo deny check` — Rust license allow-list gate (`src-tauri/deny.toml`).
- `cargo test --lib export_bindings && git diff --exit-code -- ../src/types/generated` — verifies `ts-rs` TypeScript IPC bindings match Rust structs.
- `cargo modules dependencies --lib --no-externs --package anima-engine > ../dataflow-truth.dot && python3 ../scripts/verify_dataflow.py ../dataflow.json ../dataflow-truth.dot 1` — verifies the module dependency graph against `dataflow.json`.
```

### `C9` — Update CI `cargo tree` isolation summary for `anima-worldgen`

- **Target:** `CLAUDE.md:43`
- **Source commits:** `f523e3c` (PR `#193` `cb61b00`), `7ebf149`
- **Reason:** `.github/workflows/ci.yml:140-148` now runs three `cargo tree` checks: `--no-default-features` excludes optional crates, default `anima-engine` excludes `anima-worldgen`, and `anima-worldgen --features reader,generate,bake` excludes `bevy_ecs`/`tauri`/`wgpu`.

Current (`CLAUDE.md:43` sentence 2):
```markdown
CI is `.github/workflows/ci.yml` (Rust on `windows-latest`, frontend on `ubuntu-latest`); it runs the gates above plus a `cargo tree` check that a default build really excludes the gated crates.
```

Proposed:
```markdown
CI is `.github/workflows/ci.yml` (Rust on `windows-latest`, frontend on `ubuntu-latest`; alongside `determinism-matrix.yml` and `hardening.yml`); it runs the gates above plus `cargo tree` checks confirming that `--no-default-features` excludes optional crates, default `anima-engine` excludes `anima-worldgen`, and `anima-worldgen` excludes `bevy_ecs`/`tauri`/`wgpu`.
```

### `C10` — Vitest `@react-three/fiber` mock vs headless `three` *(ALREADY LANDED IN `#213` `4fe3d7a`)*

- **Target:** `CLAUDE.md:47`
- **Status at `e576350`:** Already updated in `4fe3d7a` (`CLAUDE.md:47` now states `@react-three/fiber` is mocked and real `three` runs headless). No change needed.

### `C11` — Fix `npm run tauri dev` typo (`npm run tauri:dev`)

- **Target:** `CLAUDE.md:52`
- **Source commit:** `e576350` (`package.json:15-16`; also preserves the historical `fddbdee` deletion note verbatim)
- **Reason:** `package.json:16` defines `tauri:dev` (`tauri dev --features desktop`), whereas `npm run tauri dev` invokes bare `tauri` without `--features desktop`. Matches `CLAUDE.md:31`.

Current (`CLAUDE.md:52`):
```markdown
- **Running the full Bevy/Tauri backend (`npm run tauri dev` / `cargo run`) is heavy and has crashed the dev machine.** Prefer the headless paths — `cargo test`, the Vitest suites — and launch the app only when a human has to see it. (`rabbit-standalone/`, the static page older notes suggest serving for 3D/rabbit work, was deleted at `fddbdee`; the only rabbit visualizer left is `playground/RabbitVisualizer.tsx`, and `playground/` was declared scratch by QĐ-D on 2026-10-31.)
```

Proposed (`CLAUDE.md:52`):
```markdown
- **Running the full Bevy/Tauri backend (`npm run tauri:dev` / `cargo run`) is heavy and has crashed the dev machine.** Prefer the headless paths — `cargo test`, the Vitest suites — and launch the app only when a human has to see it. (`rabbit-standalone/`, the static page older notes suggest serving for 3D/rabbit work, was deleted at `fddbdee`; the only rabbit visualizer left is `playground/RabbitVisualizer.tsx`, and `playground/` was declared scratch by QĐ-D on 2026-10-31.)
```

---

## 7. Adjacent Stale Notes Observed Outside `CARD DOC7` Scope

Recorded here for visibility only — **not** part of `README.md`, `README.vi.md`, or `CLAUDE.md`:

- `scripts/root_allowlist.json:62` still describes `"src-tauri"` as `"Rust workspace: the anima-engine package and crates/anima-domain"` (omits `crates/anima-worldgen`).
- `docs/planning/STATE_OF_THE_PROJECT.md:1589` carries an older Phase 1 dependency row `burn 0.13.2` (while `STATE` §3.00 line 697 already reflects `burn 0.22.1`).
- `.github/workflows/ci.yml:162` still has a comment mentioning `ndarray CPU backend` whereas CPU execution uses `burn-flex` (`74dcd31`).

---

## 8. Verified Trunk Source Commit Ledger (`e576350` Ancestry)

Every SHA cited in this proposal was verified on `e576350` via `git cat-file -t <sha>` (`commit`) and `git merge-base --is-ancestor <sha> e576350` (exit `0`):

| Short SHA | Full 40-char SHA | Committer Date | Subject |
|---|---|---|---|
| `e576350` | `e576350fc2a8b9c6765eea81a94ecfa705fcc528` | `2026-11-23T13:35:50+07:00` | `Merge pull request #213 from DuongNAD/lead/docs-drift-2026-11-23` |
| `14e7c14` | `14e7c142fd9d3b260320c119544105231c7874ce` | `2026-11-23T12:57:45+07:00` | `Merge pull request #212 from DuongNAD/lead/decisions-2026-11-23` |
| `4fe3d7a` | `4fe3d7afd35af797dd7cbe98fc40beb41d5aa362` | `2026-11-23T12:15:18+07:00` | `docs: correct statements in CLAUDE.md and READMEs that the code contradicts` |
| `f40ae07` | `f40ae078b95aaa34d29807e7e63969796e07e285` | `2026-11-23T11:59:11+07:00` | `docs(workflow): record the owner's 2026-11-23 decisions and authorizations` |
| `6495c5c` | `6495c5c5a39b3bd171f900501624d7cf5cf4a93f` | `2026-11-23T00:54:08+07:00` | `Merge pull request #206 from DuongNAD/mac/research-docs-2026-11-22` |
| `45238ff` | `45238ffee7794a54468510edfbb4e948c62a085c` | `2026-11-22T21:20:28+07:00` | `Merge pull request #195 from DuongNAD/mac/wv2-1c` |
| `03b9d3d` | `03b9d3d93d1669d9fc1e5bf1a4b62b7c3d95b288` | `2026-11-22T20:56:27+07:00` | `Merge pull request #197 from DuongNAD/mac/fix-vacuous-gates-f054` |
| `427a0e6` | `427a0e619592483caca73c44fa968a2c8a39c5d4` | `2026-11-21T21:06:29+07:00` | `fix(ci,bundle): close 4 vacuous workflow gates and F-054 route-split detection` |
| `95b1338` | `95b133876098f98b1a6033f5e5e85bebadb872b2` | `2026-11-21T02:57:00+07:00` | `feat(worldgen): WV2-1c hybrid receiver-first/fallback GS, convergence bounds, and independent 512^2 residual check` |
| `bfbb9c2` | `bfbb9c2afdefea2590da9848b89ef6590c554971` | `2026-11-21T02:19:32+07:00` | `Merge pull request #194 from DuongNAD/mac/wv2-1b` |
| `cb61b00` | `cb61b00dd307996764db76275d348e07439a0fdc` | `2026-11-21T01:01:29+07:00` | `Merge pull request #193 from DuongNAD/mac/wv2-1a` |
| `7ebf149` | `7ebf149a33d37fff850eafb463861c192eee6f5b` | `2026-11-20T22:28:12+07:00` | `fix(worldgen): WV2-1a r1 G2 regex, clippy math ban, 512x512 & structure/bounds tests, crates thread_rng scan` |
| `25cf7f6` | `25cf7f6166c007b39f196222af26e62e2a2ef85d` | `2026-11-20T21:58:59+07:00` | `feat(worldgen): WV2-1b Priority-Flood/MFD-D8 hydrology, coupled G>0 Fastscape-SD, and thermal/hillslope creep` |
| `d871fd6` | `d871fd6201ece12e4ba33fd2c7a9e6121c5954c9` | `2026-11-20T21:17:41+07:00` | `Merge pull request #192 from DuongNAD/lead/world-v2-design` |
| `d64e536` | `d64e536ded17476f4958da6d1ad6c3dcb1ef4832` | `2026-11-20T21:04:57+07:00` | `Merge pull request #191 from DuongNAD/lead/state-2026-11-20` |
| `f523e3c` | `f523e3c682e62574ad501cf7230b1098f65c16f7` | `2026-11-20T20:54:09+07:00` | `feat(worldgen): WV2-1a crate scaffold, libm clippy ban, Stage S1 tectonics and CI/G2 gates` |
| `7f2e730` | `7f2e730d8b6570404c1aacedca30289abc3121c3` | `2026-11-20T20:02:24+07:00` | `docs: land the accepted World v2 design (WV2-D) and record LD-1..LD-5 under QD-MAPV2` |
| `a3d7ab6` | `a3d7ab630f209954a1ca29d9f5271dee6f519f1b` | `2026-11-20T19:58:09+07:00` | `docs(state): FOUNDER-LAYOUT, VIAB-1 phase B, M2 round, Map v2 status (2026-11-20)` |
| `4794b6f` | `4794b6fca720dc0fad4b3e66aa377a15e341bae6` | `2026-11-20T17:13:42+07:00` | `Merge pull request #186 from DuongNAD/mac/founder-layout` |
| `c674db8` | `c674db821f2b9b6d00b11fc77026034ebcaa9cba` | `2026-11-20T15:15:51+07:00` | `Merge pull request #187 from DuongNAD/docs/readme-english-default` |
| `cc40f11` | `cc40f11ff071911c58a121c04c5759695ae824a2` | `2026-11-20T14:55:37+07:00` | `Merge pull request #185 from DuongNAD/lead/map-v2-plan` |
| `d2ae358` | `d2ae358b4127e8fc7a8d21d47bb8587e7c662ad9` | `2026-11-20T14:07:51+07:00` | `docs: make the English README the default, Vietnamese as README.vi.md` |
| `1ef4f0d` | `1ef4f0d303602d666f915aa20a6edfdcf20590db` | `2026-11-20T12:52:23+07:00` | `Merge pull request #183 from DuongNAD/lead/findings-e3-result` |
| `fe9c942` | `fe9c942eeb15d4149d0135a873932ee31f97ccdc` | `2026-11-20T12:35:15+07:00` | `docs: Map v2 plan (accepted), QD-MAPV2; QD-SCOPE amended for graphics` |
| `d90be0b` | `d90be0bd26fe637e2a3db1ce0c238845cf4211fd` | `2026-11-20T12:02:25+07:00` | `Merge pull request #181 from DuongNAD/win/e3x-results` |
| `7bdddf7` | `7bdddf7f048118fb7a3ae56213c90a3cf02d6732` | `2026-11-20T12:00:44+07:00` | `Merge pull request #182 from DuongNAD/mac/sel2p2-prep` |
| `eedf577` | `eedf577e98d4f67aef5fa71e214d82632818111a` | `2026-11-20T07:14:27+07:00` | `Merge pull request #180 from DuongNAD/mac/fix-cap2-drift-strict` |
| `6a9b9a2` | `6a9b9a2cbf5f0b1798ed1e0355485f4df6fbb4bc` | `2026-11-20T05:11:11+07:00` | `Merge pull request #179 from DuongNAD/mac/f051b-world-inputs` |
| `7d53660` | `7d53660fee9e8a428020191dc77541db59102f03` | `2026-11-20T03:07:45+07:00` | `feat(e3-x): implement release-only E3 runner, independent verifier, and tests (W13)` |
| `f2887f1` | `f2887f1bc4fe1978cf1436f09d8748f5aee8de1a` | `2026-11-20T01:33:05+07:00` | `Merge pull request #177 from DuongNAD/mac/fix-cap-strict` |
| `0dc00fb` | `0dc00fba2dc47cbbb37f7f722eec9e81315ca00a` | `2026-11-20T01:21:57+07:00` | `Merge pull request #175 from DuongNAD/lead/license-elect-apache` |
| `62c70cc` | `62c70cc672037afa296f11560637507380d539ca` | `2026-11-19T23:30:11+07:00` | `Merge pull request #174 from DuongNAD/mac/f051-world-source` |
| `e9c92dc` | `e9c92dcf25e26e69c68d1f1f3c8cafa9caa51b89` | `2026-11-19T23:27:56+07:00` | `Merge pull request #173 from DuongNAD/lead/sel2p-outcome` |
| `f99dc6d` | `f99dc6df795f893a3710ece5834e243cfcb9d623` | `2026-11-19T23:16:58+07:00` | `fix(licensing): make the election checks match the prose; name the open legal question` |
| `dc66ec8` | `dc66ec8294e6c9e0af4e3996587b10d794c90d5c` | `2026-11-19T22:06:45+07:00` | `feat(licensing): close utf8-chars by electing Apache-2.0 from MIT OR Apache-2.0` |
| `e0ec7ed` | `e0ec7ed024d31cad6ff7b0ea097e29f6376f3b35` | `2026-11-19T13:41:27+07:00` | `fix(w2-lite): clear stale banner, refuse scheduled interventions, gate on worldReady, verify fixture, update CLAUDE.md count` |
| `dcff379` | `dcff37962b6222300608f55b2066bc1513a67569` | `2026-11-19T13:17:14+07:00` | `Merge pull request #155 from DuongNAD/docs/adr-0010-world-artifact` |
| `391e5c6` | `391e5c6a558527cf2f0582448fd95131a9b486f3` | `2026-11-19T10:41:15+07:00` | `docs(adr): ADR-0010, a pinned and tiled world artifact as the single source of truth` |
| `5cc4675` | `5cc4675664d3c7231a78798d11122a031708f329` | `2026-11-19T00:59:01+07:00` | `Merge pull request #135 from DuongNAD/dell/s12-lake-layout` |
| `f461466` | `f4614665f7ccf527a6a93a5b0fcfd39aa4e3f948` | `2026-11-18T22:33:30+07:00` | `Merge pull request #127 from DuongNAD/win/r1c-burn-upgrade` |
| `74dcd31` | `74dcd31348b14dc1c6fa215159bd1300b793dd8b` | `2026-11-18T13:50:46+07:00` | `build(ml): upgrade burn 0.13.2 -> 0.22.1 with burn-flex and snapshot schema 8` |
| `84e77c3` | `84e77c3dbbf989d29476db658d0077e7c5d5cb5b` | `2026-11-18T06:01:14+07:00` | `ci(root): gate the repository root against a reviewed allow-list (card G9)` |
| `18ae1e6` | `18ae1e6261c9411f10873bd1378ad87a39d8bce4` | `2026-11-18T05:26:19+07:00` | `ci(audit): gate npm advisories through a reasoned exemption list` |
| `61d7805` | `61d78050e3129882837b730068d3b230f967f0ee` | `2026-11-17T17:20:21+07:00` | `docs: full accuracy pass — root docs, how-to, reference contracts, docs/ai statuses` |
| `b507bb4` | `b507bb4a44fc32bd3ee59396b49b9b762713e485` | `2026-11-15T23:16:52+07:00` | `test(e2e): un-narrow test:e2e and gate the spec list in both directions` |
| `491ec61` | `491ec61e149d3661b5c1790ce9335d6e25d4cd2d` | `2026-11-14T09:13:27+07:00` | `docs: make the docs agent-ready — metadata and IPC gates, code-checked contracts, owner decisions` |
| `78b2e85` | `78b2e85872dc429a9da9da9ee68c4c508908ea75` | `2026-10-31T22:28:59+07:00` | `chore(ci): clear all eight failing Rust and frontend CI gates` |
| `3c06491` | `3c064915ef76c4ce539764476d2a3b28c7e8f8e5` | `2026-10-31T14:47:16+07:00` | `feat(ci): declare the module dependency graph and gate it (S0.2, closes F-005)` |
| `b576309` | `b576309fa63ba41bd511b735d7ab59fe8c76eb18` | `2026-10-22T00:32:10+07:00` | `test(frontend): add Research Dashboard unit/stress suites and update Vitest/Tauri mocks` |
| `fceb027` | `fceb027d1f2b1683c744f09f26bbc2d8efe7aaf7` | `2026-10-22T00:31:08+07:00` | `feat(map2): add Map 2 3D scene components, HUD, camera rig, and Vite entry point` |
| `fddbdee` | `fddbdee8e9a1bc5fdefeb2c2961859cc321e6f3f` | `2026-08-10T22:46:53+07:00` | `chore: commit toàn bộ công việc dở dang trước khi hardening` |
| `043930c` | `043930c1076b444a3ac337914f33a83fdce69b97` | `2026-08-08T12:51:45+07:00` | `chore(license): proprietary -> MIT OR Apache-2.0, and GPL stays blocked for a new reason` |
| `ce558b2` | `ce558b22b35ae2f81a0ea26ece9d42a1eee662ff` | `2026-07-28T22:59:31+07:00` | `fix(e2): verify artifacts against committed manifest bytes` |
| `19e67fa` | `19e67fa838b674c43d3002dc4ef400a5639cab41` | `2026-07-27T20:00:19+07:00` | `fix(ipc): send camelCase argument keys, which is what tauri::command expects` |
| `920c369` | `920c36913730446545716839c48609956792b4ed` | `2026-07-27T13:37:06+07:00` | `feat(licensing): package the licence texts, and measure what actually ships` |
| `67db7c6` | `67db7c6d61d82fd82278da3c6e62871f32317ce0` | `2026-07-27T12:18:02+07:00` | `refactor(types): remove the last casts, and type the boundaries they were hiding` |
| `2d7543e` | `2d7543e732ed946f0d63a9f2b5868db1474d6186` | `2026-07-27T11:37:13+07:00` | `chore(lint): finish the test suites, and lock the ratchet at zero` |
| `9230e1d` | `9230e1d704dd7a9a5039f24fb3d2d04b7205878b` | `2026-07-27T05:20:11+07:00` | `test(e2e): stop skipping, and stop shouting` |
| `6adf6e3` | `6adf6e3e59d473ce32b9e56c58dd60a2569a84f9` | `2026-07-27T04:11:54+07:00` | `fix(map): one clearance policy, and a manifest for the world we actually ship` |
| `9a31a99` | `9a31a99616bce7ac78400cb214b85b10b8ea462e` | `2026-07-25T16:29:02+07:00` | `feat(ipc): generate the TypeScript contract types from the Rust structs` |
| `f402447` | `f4024477b3b1901dad1f6d94827352c70bff769c` | `2026-07-25T11:07:58+07:00` | `chore(ci): add a GitHub Actions pipeline with a lint ratchet and docs-link check` |
