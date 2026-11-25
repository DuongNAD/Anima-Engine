---
phase: planning
feature: v1-vertical-slice
title: Card V11 — Five Runnable Example Scenarios for Newcomers
description: Implementation spec for card V11 providing five self-contained example manifests with one verified command and one expected outcome each, gated by check_doc_commands and S67 smoke tests
status: proposed
owner: anima-dell
last_reviewed: 2026-11-24
---

# Card `V11` — Five Runnable Example Scenarios (`M11.6`, `S67`)

> **Design spec only (`status: proposed`).** No Rust, TypeScript, or docs-index edits are made in D8 (`V-SPECS-2`).
> All trunk `file:line` anchors below were checked against `origin/feat/ecological-selection` at
> `8d52341549007a50ce033a6d272c5dd22a71b1b1`. Anything not on trunk yet is labeled **`to be created by V11`**.

## 1. Goal, Dependencies, `START WHEN` & Scope

Enable any newcomer who has cloned the repository to run **5 concrete headless scenarios** end-to-end — each with **exactly one runnable command** and **one expected result** showing observed schema/behavior cleanly (`S67`, [`WORLD_SIMULATION_PLAN.md`](../../../WORLD_SIMULATION_PLAN.md) `§7 M11.6 :916`, `§8.6 S67 :1008`, `§9 :1048-1071`, and `2026-11-05-swarm-master-plan.md` Đợt 6 `V11`).

### Exact Dependency Order & `START WHEN`

- **Depends directly on:**
  - **[Card `V8`](2026-11-24-v8-slice-acceptance-spec.md)** (vertical-slice scenarios validated across seeds)
  - **`DOC1`** (already merged: [`docs/tutorials/01-first-headless-experiment.md`](../../tutorials/01-first-headless-experiment.md))
- **`START WHEN`:** `V8` is merged on trunk (`DOC1` is already merged) and owner decisions `D-V11-1`–`D-V11-4` are resolved.
- **Unblocks:** `H11` v1.0 release & external user trial onboarding (`MP-5.1` / `M11.6`).

### Explicit Non-Goals

- **No hand-typed / unexecuted output blocks (`2026-11-05-swarm-master-plan.md` Đợt 6 docs rule):** Every command written into `V11`'s user documentation **must** be executed on real hardware before committing, with platform/commit recorded honestly matching [`01-first-headless-experiment.md`](../../tutorials/01-first-headless-experiment.md) `:22-26, 52-57`.
- **No synthetic/cross-OS-pinned `world_checksum` examples:** Expected results describe the schema fields, direction of observables, causal root traces, and conservation residuals — never claiming a universal checksum across OSes.

## 2. Current Trunk Anchors (`8d523415`) & Target Layout

### 2.1 Existing CLI, Tutorial & Doc-Test Machinery Verified on Trunk

- **Headless runner CLI (`anima-lab`):** `src-tauri/src/bin/anima-lab.rs:43, 76, 95-108, 210, 427-445` (`anima-lab [ensemble|sweep|branch] --model <reference|live> <MANIFEST.json>`) plus [`RUN_EXPERIMENTS.md`](../../how-to/RUN_EXPERIMENTS.md) `:31-55` (verbatim `--help` output and exit codes `0`/`1`) and [`WRITE_A_MANIFEST.md`](../../how-to/WRITE_A_MANIFEST.md) `:17-55` (`ExperimentManifest` strict `#[serde(deny_unknown_fields)]` schema).
- **Existing experiment fixtures (`src-tauri/tests/fixtures/experiments/`):** `baseline-no-exotic.json`, `mana-patchy-renewable.json`.
- **Doc-command gate (`scripts/check_doc_commands.mjs:11-70`):** Extracts fenced `bash`/`sh` blocks marked with `<!-- doc-test -->` inside `DEFAULT_SUBDIRS = ['docs/tutorials', 'docs/how-to']`:
  - `node scripts/check_doc_commands.mjs --static` checks syntax/markers without running Cargo.
  - `npm run check:doc-commands` executes every `<!-- doc-test -->` block in clean repo order (`DOC8`).
- **Tutorial home & conventions:** [`docs/tutorials/README.md`](../../tutorials/README.md) `:9-55` and [`docs/tutorials/01-first-headless-experiment.md`](../../tutorials/01-first-headless-experiment.md) `:1-60` (toolchain prerequisites, single `npm run check:docs` parity, `<!-- doc-test -->` markers, trimmed observed output).

### 2.2 The Five Newcomer Scenarios (`to be created by V11`)

Store `5` validated, self-contained scenario JSON manifests (location per `D-V11-1`, e.g., `src-tauri/tests/fixtures/scenarios_v1/`) and document them in [`docs/tutorials/`](../../tutorials/README.md) (`02-five-example-scenarios.md` `to be created by V11`, linked from `docs/tutorials/README.md`):

| # | Scenario Manifest (`to be created by V11`) | Single `<!-- doc-test -->` Command Pattern | Expected Observable Outcome (`S67`) |
|---|---|---|---|
| **1** | `01-watershed-baseline.json` | Single-command `anima-lab` / runner invocation (`--model live`) | Grass + rabbit + wolf coexistence across warmup/cycle window; zero `NaN`; EU residual `<= 1e-3`; no unrequested interventions |
| **2** | `02-drought-30d.json` | Paired/single `anima-lab` command on drought manifest | Target-region `soil_moisture` falls -> grass NPP drops -> rabbit decline; `CausalLedger` root cause traces to drought `cause_id` (`V6`) |
| **3** | `03-remove-wolves-80pct.json` | Paired/single `anima-lab` command on wolf-cull manifest | Single-shot 80% wolf cull -> predator drop -> rabbit population surge -> grass depletion trophic cascade (`O2`) |
| **4** | `04-deforest-headwaters-40pct.json` | Paired/single `anima-lab` command on headwaters clear manifest | 40% headwaters `Tree` removal -> `canopy_cover` / `root_cover` drop -> higher runoff, erosion & downstream turbidity (`O3`) |
| **5** | `05-fertilize-lakes.json` *(or `05-alternate-mana-fork.json` per `D-V11-2`)* | Paired/single `anima-lab` command on scenario 5 manifest | Soil nitrogen rises via `DynamicFields::step_soil` -> grass growth & downstream lake `algal_bloom` / depressed `dissolved_oxygen` (`O4`), with water/nutrient conservation residual `< 1e-4` |

Add automated smoke test `src-tauri/tests/v11_example_scenarios_tests.rs` (`to be created by V11`) validating that all 5 committed scenario manifests parse strictly, pass `ExperimentManifest::validate` against `LiveExperimentAdapter`'s `ObservableRegistry`, and complete cleanly in fast smoke execution (`S67`), matching the exact commands documented under `<!-- doc-test -->`.

## 3. Applicable [`CLAUDE.md`](../../../CLAUDE.md) Traps & Invariants

- **Never spawn the heavy windowed Tauri desktop app in newcomer CLI scenarios (`CLAUDE.md:52`):** All 5 scenarios run purely headlessly via compiled `anima-lab` / cargo release example binary so newcomers on modest laptops never hit GUI startup crashes.
- **No fabricated numbers & no pinned cross-OS checksums (`CLAUDE.md:22, 41, 176-182`, `docs/decisions/ADR-0008-world-past-and-time.md:175, 283`, `src-tauri/tests/cross_platform_checksum_probe.rs:235`):** Never print a single `"expected SHA256/world_checksum"` as universal across Windows/macOS/Linux; show real observed stdout fields from the actual test host (noting OS & commit) and assert directional/conservation pass criteria instead.
- **`WorldLawSet` & `exotic_energy = None` default (`CLAUDE.md:145-153, 180-182`):** Note clearly that scenarios `1–4` (and `5` if lake fertilization under `D-V11-2 = A`) run on `--model live` (`exotic_energy = null`), whereas any Mana (`Renewable`) alternate-regime scenario runs strictly under `--model reference` (`CLAUDE.md:180`).
- **Closed EU, `total_cost`, zero-alloc, No Lamarck (`CLAUDE.md:50, 97-98, 105-107`):** Example manifests preserve all conservation flags enabled so newcomers observe conserved EU/water/nitrogen balances by default.
- **Root hygiene (`scripts/check_root_hygiene.mjs:12-20`):** Never drop scenario files in repo root or add `scripts/root_allowlist.json:1-20` entries unless explicitly decided in `D-V11-1`.

## 4. Open Owner Decisions (`D-V11-1`–`D-V11-4`)

| Slot | Question | Options | Recommendation |
|---|---|---|---|
| **`D-V11-1`** | Where should the 5 scenario JSON manifests live? | **(A)** `src-tauri/tests/fixtures/scenarios_v1/` (zero root-allowlist churn; directly loadable by `v11_example_scenarios_tests.rs`).<br>**(B)** New top-level `examples/scenarios/` folder (requires updating `scripts/root_allowlist.json:1-20`). | **(A)** `src-tauri/tests/fixtures/scenarios_v1/` — matches existing `experiments/` / `experiments_e3b/` convention without touching `scripts/root_allowlist.json:3`.<br>**DECIDED (2026-11-24, QD-OPENQ): (A).** |
| **`D-V11-2`** | Choice for Scenario #5 (given baseline + 4 §9 interventions already total `5` items): | **(A)** Lake fertilization (`05-fertilize-lakes.json`), giving newcomers 1 baseline + all 4 `§9` vertical-slice interventions.<br>**(B)** Alternate-regime Mana checkpoint fork (`05-mana-checkpoint-fork.json` on `--model reference`), folding drought+forest+lake into 3 examples. | **(A)** Baseline + the 4 `§9` interventions (total 5) keeps every scenario using one consistent `--model live` binary & schema.<br>**DECIDED (2026-11-24, QD-OPENQ): (A).** |
| **`D-V11-3`** | Per-scenario CI doc-test runtime ceiling (`npm run check:doc-commands` execution tier): | **(A)** Short `duration_ticks` in the 5 doc-test example manifests (`< 15 s` per scenario so full `check:doc-commands` stays CI-friendly), with clear comment showing how a reader extends `duration_ticks` / `seeds`.<br>**(B)** Full `1,800+` tick multi-seed ensembles inside `<!-- doc-test -->` (slow CI gate). | **(A)** Fast doc-test scenario runs (`<= 15 s` each), linking to `V8` manifests for full multi-seed statistical runs.<br>**DECIDED (2026-11-24, QD-OPENQ): (A).** |
| **`D-V11-4`** | Document organization under `docs/tutorials/`: | **(A)** One tutorial `docs/tutorials/02-five-example-scenarios.md` with 5 numbered sections (`S67`) indexed in `docs/tutorials/README.md`.<br>**(B)** Five separate tutorial markdown files. | **(A)** One clean `02-five-example-scenarios.md` page — easy for newcomers to skim side-by-side and simpler to maintain.<br>**DECIDED (2026-11-24, QD-OPENQ): (A).** |

## 5. Future `V11` Card Boundaries (**Proposed** `ALLOWED PATHS`)

> Marked **`proposed`** for owner authorization when scheduling card `V11`.

- `src-tauri/tests/fixtures/scenarios_v1/` (5 validated scenario manifests, `to be created by V11`)
- `src-tauri/tests/v11_example_scenarios_tests.rs` (`S67` smoke test validating all 5 scenarios execute & satisfy directional/conservation checks, `to be created by V11`)
- `docs/tutorials/02-five-example-scenarios.md` (`to be created by V11`) and `docs/tutorials/README.md` (index link + table update)
- `docs/planning/STATE_OF_THE_PROJECT.md` / `TODO.md` handled separately by recurring `D1` `STATE-SYNC` (do not edit in `V11`)

## 6. `DONE WHEN`, Verification Commands & Negative Controls (`A3 #2`)

### `DONE WHEN`
1. All `5` example scenario manifests exist, validate cleanly, and pass headless execution (`S67`), each documented with **one copy-pasteable `<!-- doc-test -->` command** and **one real observed expected-result section**.
2. Concrete Verification Commands (after final edit):
   - `npm run check:docs` (`exit 0`, zero broken links or metadata errors)
   - `node scripts/check_doc_commands.mjs --static` (`exit 0`, all `<!-- doc-test -->` fenced blocks valid)
   - `npm run check:doc-commands` (live execution of all doc-test blocks — run only on machine/CI meeting `>= 5.00 GiB` RAM when `cargo` binary build is involved)
   - `cmd /c "cargo test --features desktop --test v11_example_scenarios_tests > ..\v11-test.txt 2>&1"` & `node scripts/check_test_targets.mjs cargo-test-output.txt --profile desktop` (on full test capture)
   - `npm run check:text-hygiene`, `npm run check:root-hygiene`, `npm run check:conflict-markers`

### Negative Controls (Must Fail Red Under Fault Injection, `A3 #2`)
1. **Unverified shell block syntax / disallowed fence:** Tag a non-POSIX PowerShell/text block with `<!-- doc-test -->` or omit closing fence -> `node scripts/check_doc_commands.mjs --static` exits `1`.
2. **Invalid scenario manifest command:** Pass a nonexistent manifest path or unknown JSON key (`deny_unknown_fields` in `WRITE_A_MANIFEST.md:30`) in one scenario -> `anima-lab` & `v11_example_scenarios_tests.rs` exit `1`.
3. **Broken tutorial link:** Point `docs/tutorials/README.md` to a missing scenario file -> `npm run check:docs` exits `1`.

### Determinism Checks
- Re-running each of the 5 newcomer scenario commands twice on the same machine prints identical `world_checksum` and `final_observables` (documented honestly as same-OS deterministic replay).
