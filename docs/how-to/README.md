---
title: How-to Guides
status: active
owner: maintainers
last_reviewed: 2026-11-23
review_cycle: per-release
---

# How-to

## Task Guides

| Task | Guide | Summary |
|---|---|---|
| Run headless experiments | [RUN_EXPERIMENTS.md](RUN_EXPERIMENTS.md) | Build and run `anima-lab` (`ensemble`, `sweep`, `branch`, JSON/CSV, negative controls) |
| Author and validate manifests | [WRITE_A_MANIFEST.md](WRITE_A_MANIFEST.md) | `ExperimentManifest`, `WorldLawSet`, `LIVE_KEYS`, `SweepManifest`, `BranchTreeManifest` schemas |
| Add a new observable | [ADD_AN_OBSERVABLE.md](ADD_AN_OBSERVABLE.md) | Register `ObservableSpec`, `LIVE_EXTENDED_OBSERVABLE_IDS`, `live_observables` vs peek, tests |
| Add a new intervention | [ADD_AN_INTERVENTION.md](ADD_AN_INTERVENTION.md) | `InterventionKind`, `validate_intervention`, canonical hash tags, `apply_live_interventions_system`, `EnergyLedger` |
| Add a new CI gate | [ADD_A_GATE.md](ADD_A_GATE.md) | Exit codes, negative controls (`selfTest()` / `--self-test` / Vitest), wiring `package.json` and CI |
| Reproduce E2 end-to-end | [REPRODUCE_E2.md](REPRODUCE_E2.md) | Run seam tests, `--smoke`, `--ensemble`, `--replay`, and verify with `npm run check:e2` |
| Dispatch or follow agent workflow | [AGENT_WORKFLOW.md](AGENT_WORKFLOW.md) | Card queues, recorded owner decisions, agent rules, and macOS verification commands |
| Benchmark per-system performance | [BENCHMARKING.md](BENCHMARKING.md) | Run Criterion `tick_systems`, `ANIMA_TICK_CAPTURE`, baseline table, and 1,000-agent checklist |
| Capture and compare 3D map views | [MAP_VISUAL_CHECK.md](MAP_VISUAL_CHECK.md) | `npm run capture:map` (prototype), before/after renders, reading FPS/draw calls |
| First external user trial run | [OWNER_TRIAL_RUN.md](OWNER_TRIAL_RUN.md) | Package Windows build 1, preflight checklist, and external-machine trial install |
| Install and run release 0.9 | [INSTALL.md](INSTALL.md) | Download CI installers, handle Windows SmartScreen / macOS Gatekeeper, prerequisites |
| Cut a release build | [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) | Pre-release gates, compliance artifacts (NOTICE, SBOM, licenses), audit, and MP-5.1 sessions |

## On macOS and Linux

Commands on this page and in [BENCHMARKING.md](BENCHMARKING.md) are written for **Windows PowerShell**,
the primary target machine. `npm`, `node`, and `cargo` commands work as-is in zsh/bash, with these differences:

| Windows / PowerShell | macOS / zsh | Tested on macOS? |
|---|---|---|
| `$env:NAME = "1"` then command | `NAME=1 command`, or `export NAME=1` | Standard POSIX syntax |
| `py scripts\verify_newick.py` | `python3 scripts/verify_newick.py` (needs `pip install dendropy`) | Run 2026-11-17 without `dendropy`: exited 1 with install hint. Not yet run with `dendropy` |
| `.\src-tauri\target\release\anima-engine.exe` | `./src-tauri/target/release/anima-engine` | No: desktop app not yet launched on this macOS host |
| `%APPDATA%\com.anima.engine\` (saves, captures) | `~/Library/Application Support/com.anima.engine/` | No: directory absent here because desktop app has not run; path inferred from `identifier` in `tauri.conf.json` |
| `Get-NetTCPConnection -LocalPort 5173 -State Listen` | `lsof -nP -iTCP:5173 -sTCP:LISTEN` | Run 2026-11-17: silent exit 1 when port is free |
| `Get-Content file.json -Raw \| ConvertFrom-Json` | `jq . file.json`, or `python3 -m json.tool file.json` | Tools installed; not tested on a capture JSON file |
| `cmd /c "cargo test ... > out.txt 2>&1"` | `cargo test ... > out.txt 2>&1` | Required form on macOS/Linux per `CLAUDE.md` |
| `Win+Shift+S` (screenshot) | `Cmd+Shift+4` | System shortcut |

Two platform rules are easy to overlook: `NOTICE` and license inventories must be generated on Windows
(`gen_notice.mjs --check` fails as stale on macOS by design); and generated terrain / simulation
checksums differ across OS `libm` targets, so always compare runs on the same platform.

## Install and Run Frontend

```powershell
npm install
npm run dev
```

Production build:

```powershell
npm run build
```

Launch full Tauri desktop app:

```powershell
npm run tauri:dev
```

Always use `tauri:dev` / `tauri:build`, never bare `tauri dev`: `tauri.conf.json` has no field for Cargo
features, so bare commands build `default = []` (no Neo4j lineage, no cross-shard migration, CPU learner)
without failing loudly. Running the full desktop app is heavy and has frozen dev machines; launch it only
when a human needs interactive UI inspection.

## Run Tests and Checks

```powershell
npm run test:frontend
npm run lint
cargo test --manifest-path src-tauri/Cargo.toml --features desktop
```

`--features desktop` is mandatory for full backend coverage and matches CI: without it, the test targets
gated on `networking` or `ml-wgpu` are skipped. Run Cargo tests after core changes, and run both Rust
and frontend checks after codec, artifact, or coordinate changes to catch cross-language drift.

## Measure Baselines

Two distinct tools exist; keep them separate:

**Real per-system timings** (OSS-010) — run inside `src-tauri/`:

```powershell
cargo bench --bench tick_systems
```

Usage, baseline comparison, rollback, and the **committed baseline table** live in
[`BENCHMARKING.md`](BENCHMARKING.md). This measures per-system headless cost with no Tauri or GPU device.

**Harness summary report** (M0.4) — run from repository root **after** `cargo bench --bench tick_systems`:

```powershell
node scripts/bench_baseline.mjs
```

Read mandatory interpretation and metadata rules in [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md).
The report combines `criterion/*` timings from `src-tauri/target/criterion` with a synthetic fBm **proxy**
(not the engine loop). On an unbenched clone, the script **refuses** to overwrite the committed report
(exits `1`); set `ANIMA_BENCH_ALLOW_PROXY_ONLY=1` only if you explicitly want a proxy-only report. Never
compare reports across differing build modes, workloads, seeds, or hardware without recording the difference.

## Validate Lineage Trees with an External Parser

The OSS-070 gate has **two halves over the same file**; neither half is sufficient alone.

```powershell
cargo test --test newick_export_tests
```

```powershell
py scripts\verify_newick.py
```

On macOS/Linux: `python3 scripts/verify_newick.py`. Without `dendropy`, the Python script exits non-zero
with installation instructions so a exit-0 run proves **DendroPy** actually parsed the output.

The Rust half (inside `src-tauri/`) writes deterministic Newick output to
[`tests/fixtures/newick/lineage_forest.nwk`](../../src-tauri/tests/fixtures/newick/lineage_forest.nwk);
the Python half parses that same file independently. A Rust-only round-trip proves only self-consistency;
external validation proves an independent phylogenetic parser accepts the tree. `dendropy` is **dev-only**
(`pip install dendropy`). Details live in [OPEN_SOURCE_ADOPTION_PLAN.md §OSS-070](../planning/OPEN_SOURCE_ADOPTION_PLAN.md).

## Generate or Verify World Artifact Fixtures

Existing generator script:

```powershell
npm run gen:artifact-fixture
```

After changing the artifact format:

1. bump the format version on breaking changes;
2. maintain readers/migrations for supported versions;
3. regenerate fixtures with provenance;
4. run Rust/TypeScript parity tests;
5. update reference/schema documentation.

## Required Map Vision Workflow

Per `AGENTS.md`, any work touching maps, terrain, biomes, ecosystems, navigation, collision, water,
or lighting must follow Animal Map Vision in order:

1. `discover_map_artifacts`
2. `validate_map_manifest`
3. `prepare_team_review`
4. `inspect_map_views`

If the MCP is missing, run from the MCP repository:

```powershell
npm run doctor -- --project <path-to-Anima-Engine>
```

Then reload `/mcp`. Do not mark map work complete without passing manifest validation, canonical
before/after renders, reachable navigation, and resolved critical/high ecological contradictions.

## Add an Open-Source Dependency

1. Read [OPEN_SOURCE_POLICY.md](../governance/OPEN_SOURCE_POLICY.md).
2. Record upstream repo/tag, license, and integration boundary.
3. Capture fixtures and baseline before piloting.
4. Draft an ADR from [ADR-0000-template.md](../decisions/ADR-0000-template.md).
5. Wrap behind an adapter + feature flag; add tests and rollback notes.
6. Update license inventories / `NOTICE` and affected documentation.
