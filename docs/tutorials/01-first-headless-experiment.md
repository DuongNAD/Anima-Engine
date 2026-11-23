---
title: Tutorial 01 — From a Clean Clone to Your First Headless Experiment
status: active
owner: maintainers
last_reviewed: 2026-11-23
review_cycle: per-release
---

# Tutorial 01 — From a Clean Clone to Your First Headless Experiment

This tutorial walks you from a clean repository clone to building and running your first headless **E2 `--smoke`** calibration, independently verifying its digests with `npm run check:e2`, and understanding what the output files do and do not prove.

> Authoritative project-wide test counts, clippy status, E2E status, and verification history live in one place: [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền) (latest full Windows 11 x64 measurement at [§1.i](../planning/STATE_OF_THE_PROJECT.md#1i-đo-2026-11-18-tại-46e25d5--lượt-đo-lại-đầy-đủ-sau-đợt-2-trên-windows-11-x64-thẻ-r0)). This tutorial does not copy repo-wide metrics that drift across commits.

---

## Prerequisites and Platform Notes

Before starting, make sure you have:

- **Git**, **Node.js `22`** (with `npm`), and **Rust via `rustup`** ([`rust-toolchain.toml`](../../rust-toolchain.toml) pins channel `1.98.1` along with `rustfmt` and `clippy`).
- **System build toolchain:**
  - **macOS (verified on `macOS arm64`):** Xcode Command Line Tools (`xcode-select --install`).
  - **Windows *(not verified on Windows here)*:** Microsoft C++ Build Tools ([Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)) with the Desktop development with C++ / MSVC toolchain and WebView2 required by Tauri v2 (see PowerShell shell equivalents in [`docs/how-to/README.md`](../how-to/README.md)).
  - **Linux *(not verified on Linux here)*:** because `tauri` is a direct dependency of `anima-engine`, even headless binaries require system WebKit/GTK development packages (`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libsoup-3.0-dev`).

The command blocks below use POSIX (`bash`/`zsh`) syntax from the **repository root**. Windows PowerShell equivalents are called out separately outside executable doc-test blocks.

---

## Step 1 — Clone the Repository

Clone the repository and check out the active trunk branch (`feat/ecological-selection`; see [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền)):

```bash
git clone https://github.com/DuongNAD/Anima-Engine.git
cd Anima-Engine
git checkout feat/ecological-selection
```

---

## Step 2 — Install JavaScript Dependencies

Install the Node packages used by frontend tools and artifact verification scripts cleanly from `package-lock.json`:

<!-- doc-test -->
```bash
npm ci
```

Observed output (`macOS arm64`, `2026-11-22`, trunk `bfbb9c2`, trimmed):

```text
added 290 packages, and audited 291 packages in 2s
...
```

---

## Step 3 — Run a Fast Backend Smoke Test Target

Before compiling the full `--release` experiment binary, run the small, non-stepping integration target `e2_seam_tests`. It runs under `default` Cargo features — intentionally omitting `--features desktop` because `e2_seam_tests` exercises only the E2 manifest interface, `AgentBrain` isolation, `SimRng` ecological stream equality, `WorldIdentity` equality, and the `cargo run` prohibition without requiring `networking` (`neo4rs`, `tokio-tungstenite`) or `ml-wgpu` (`burn-wgpu`). Note that running the **full** workspace test suite (and CI) *does* require `--features desktop` so the 8 feature-gated test targets covering 2,040 lines are not silently skipped.

<!-- doc-test -->
```bash
cargo test --manifest-path src-tauri/Cargo.toml --test e2_seam_tests
```

Observed output (`macOS arm64`, `2026-11-22`, trunk `bfbb9c2`, crate compilation trimmed):

```text
   Compiling anima-engine v0.1.0 (.../src-tauri)
...
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 13s
     Running tests/e2_seam_tests.rs (src-tauri/target/debug/deps/e2_seam_tests-...)

running 9 tests
test the_live_world_honours_exactly_one_new_initial_condition_key ... ok
test absent_means_false_so_every_existing_live_manifest_keeps_its_behaviour ... ok
test one_means_true_and_nothing_else_is_accepted ... ok
test the_preregistered_reproduction_never_launches_the_app ... ok
test the_treatment_builds_founders_with_brains_and_the_control_builds_none ... ok
test the_founding_population_differs_only_by_the_brain ... ok
test both_arms_observe_the_same_world_identity ... ok
test the_committed_e2_manifests_build_the_arms_they_declare ... ok
test founder_brains_leave_the_ecology_stream_exactly_where_the_control_left_it ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s
```

> To run the full backend verification suite instead of an onboarding target, see [`STATE_OF_THE_PROJECT.md` §5](../planning/STATE_OF_THE_PROJECT.md#5-lệnh-xác-minh-đầy-đủ).

---

## Step 4 — Build and Run Your First Headless `--smoke` Calibration

Compile the `run_e2_brain_experiment` example in `--release --features desktop` mode, then invoke the binary directly with `--smoke` writing to gitignored `src-tauri/target/tutorial-e2-smoke`:

<!-- doc-test -->
```bash
cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop --example run_e2_brain_experiment
./src-tauri/target/release/examples/run_e2_brain_experiment --smoke \
  --manifest-dir src-tauri/tests/fixtures/experiments_e2 \
  --out src-tauri/target/tutorial-e2-smoke
npm run check:e2 -- src-tauri/target/tutorial-e2-smoke
```

> **Windows PowerShell *(not verified on Windows here)*:** append `.exe` to the compiled example path:
>
> ```powershell
> .\src-tauri\target\release\examples\run_e2_brain_experiment.exe --smoke `
>   --manifest-dir src-tauri\tests\fixtures\experiments_e2 `
>   --out src-tauri\target\tutorial-e2-smoke
> ```

Observed output (`macOS arm64`, `2026-11-22`, trunk `bfbb9c2`, local paths trimmed with `...`):

```text
   Compiling anima-engine v0.1.0 (.../src-tauri)
...
    Finished `release` profile [optimized] target(s) in 7m 43s
E2 Smoke: 1 seeds x 2 arms x 18000 ticks, sample period 600
  integrity: control brains 0 / treatment brains 10 of 10 founders; ecology stream identical: true; world identity identical: true
  finished in 4.4s: 1 complete pairs of 1
  artifacts written to src-tauri/target/tutorial-e2-smoke/smoke

> anima-engine-frontend@0.1.0 check:e2
> node scripts/verify_e2_artifacts.mjs src-tauri/target/tutorial-e2-smoke

note  analysis: no ensemble has been run yet (no paired-report.json at the artifact root)
note  smoke: recomputed 27 per-seed deltas
note  analysis: no execution-lock.json — the duration rung was not locked from a smoke calibration

OK — every E2 artifact in .../src-tauri/target/tutorial-e2-smoke verifies against node:crypto and the preregistration.
```

Keep three hard rules in mind on every run:

1. **Never use `cargo run` for preregistered E2 runs:** [`e2-preregistration.json`](../../src-tauri/tests/fixtures/experiments_e2/e2-preregistration.json), `e2_seam_tests`, and `scripts/verify_e2_artifacts.mjs` forbid `cargo run`. Build first and invoke the binary directly so `provenance.json` pins `"profile": "release"` and `binary_sha256`.
2. **Never launch `npm run tauri:dev` for headless runs:** `run_e2_brain_experiment` drives `LiveExperimentAdapter` over `simulation_schedule::build_tick_schedule` in-process with no window, Tauri handle, GPU device, or WebSocket server.
3. **Never write tutorial runs to `artifacts/experiments/e2-evolved-brain-default`:** that directory holds committed reference evidence ([`RESULT.md`](../../artifacts/experiments/e2-evolved-brain-default/RESULT.md)); `src-tauri/target/` keeps `git status` clean. *(On a fresh smoke run at `bfbb9c2`, `check:e2` recomputes 27 observables rather than the 22 observables present in the 2026-11-16 committed artifact.)*

> The full E2 reproduction procedure — 12-seed `--ensemble`, `--replay` (`E2-G5`), `--duration-rung`, and auditing the committed root artifact — lives in the dedicated how-to guide (`docs/how-to/REPRODUCE_E2.md`, indexed in [`docs/how-to/README.md`](../how-to/README.md)).

---

## Step 5 — Inspect `smoke/summary.md` and the 10 Output Files

Print the generated Markdown report:

<!-- doc-test -->
```bash
cat src-tauri/target/tutorial-e2-smoke/smoke/summary.md
```

Observed output (`macOS arm64`, `2026-11-22`, trunk `bfbb9c2`, middle rows of the 27-observable table omitted):

```text
# E2 — per-agent evolved brain vs shared BrainModel

> ⛔ **Calibration only, seed 999983, EXCLUDED from every analysis, table, statistic and claim.** These numbers exist to time the harness and prove it runs. They are not evidence about brains and may not be quoted as any part of the E2 result.

Model: `core::live_experiment::LiveExperimentAdapter` — **headless adapter verified**, which is the only status claim permitted. Harness: `core::experiment_runner::run_paired_ensemble_with_control`.

- seeds run: `[999983]`
- duration: **18000 ticks** (committed 18000, ladder [18000, 12000, 6000]), sample period 600
- complete pairs: **1 of 1** (decision needs 10)
- wall clock: **4.4 s** (0.07 min) against a registered ceiling of 90 min
- declared factors: `["initial_conditions"]`

## Integrity (gates E2-G1, E2-G3, E2-G6)

| check | result |
|---|---|
| control founders carrying `AgentBrain` | 0 of 10 |
| treatment founders carrying `AgentBrain` | 10 of 10 |
| ecology stream identical after genesis | yes |
| world identity identical across arms | yes |

## Every observable the run produced
...
| `live.closed_eu_total` | harness integrity | 1 | 155971.799862 | 155971.799862 | 1.106e-9 | 1.106e-9 | n/a | n/a | n/a | n/a | +0.00% | no |
...
| `live.mean_agent_energy` | primary | 1 | 0.000000 | 0.000000 | 0.000000 | 0.000000 | n/a | n/a | n/a | n/a | n/a | no |
| `live.mean_energy_integral` | reported | 1 | 281958.324842 | 292190.542737 | 10232.217895 | 10232.217895 | n/a | n/a | n/a | n/a | +3.63% | no |
...
## The primary metric, against the three registered conditions

`live.mean_agent_energy`, paired per seed, hypothesised **negative** (H1).

| condition | required | measured | met |
|---|---|---|:--:|
| 95% CI excludes 0 | CI must not contain 0 | undefined | no |
| \|d_z\| ≥ 0.8 | large by convention | n/a | no |
| \|delta\| ≥ 5% of \|control mean\| | relative shift | undefined | yes |

**Material: no.**
...
## What this cannot establish
...
```

Inside `src-tauri/target/tutorial-e2-smoke/smoke/` you will find 10 files (there is **no `summary.json`**): `summary.md` (human report containing wall-clock time), `effects.json`, `paired-report.json`, `per-seed-deltas.csv` (27 rows + header), `runs.csv`, `provenance.json` (timestamps, host paths, binary SHA-256), `manifests/*.json` (3 preregistered files matching `HEAD` bytes), and `checksums.sha256` (covering those 9 files).

Two common points of confusion when reading smoke output:

- **Why is `live.mean_agent_energy` `0.000000` yet `|delta| >= 5%` shows `yes`?** Agents drain to `0.0` before tick `18000` under default survival settings (see [`RESULT.md`](../../artifacts/experiments/e2-evolved-brain-default/RESULT.md)); `0.0 >= 0.05 * 0.0` evaluates `true` formally even though `relative_shift` is `undefined`, while **`Material: no`** still holds because `95% CI` and `|d_z|` are `no`.
- **What does `live.closed_eu_total` show?** Total closed energy stays near `155971.799862` EU across arms within `1.106e-9` EU floating-point accumulation noise (well inside the closed-EU budget in [`ENERGY_LEDGER_CONTRACT.md` §6](../reference/ENERGY_LEDGER_CONTRACT.md#6-cổng-g11)).

---

## Inference Boundaries: What Smoke Proves and Does NOT Prove

### What `--smoke` Establishes

1. **The headless harness compiles and completes cleanly:** `LiveExperimentAdapter` steps 18,000 ticks across `control` and `treatment` at seed `999983` under `--release` without panics.
2. **Causal isolation and energy conservation hold:**
   - **E2-G1:** `0/10` control founders vs `10/10` treatment founders carry `AgentBrain`.
   - **E2-G3:** the ecological `SimRng` stream remains aligned immediately after genesis.
   - **E2-G4:** the arms differ only in `initial_conditions` (`live.evolved_brains`).
   - **E2-G6:** both arms build the identical `WorldIdentity` (`seed = 1337`, `256 × 256`; on **macOS arm64 only**, terrain checksum = `1212187560` — terrain generation checksums are **platform-specific** due to OS `libm` math differences, measuring `3152323152` on Windows and `22481267` on Linux in [`docs/paper/data/determinism_matrix.md`](../paper/data/determinism_matrix.md) line 18).
   - **Closed-EU:** total energy is conserved to `~1e-9` EU.
3. **Timing budget calibration:** 1 pair ran in ~`4.4 s` on Apple Silicon (informing rung selection under the 90-minute ceiling).
4. **Independent digest integrity:** `npm run check:e2` independently verifies every file against `checksums.sha256` using `node:crypto`.

### What `--smoke` Does NOT Establish

1. **Not scientific evidence about evolved brains:** seed `999983` is reserved for calibration and **excluded from every statistical analysis** (`kind: "e2-smoke-effects-EXCLUDED-FROM-ANALYSIS"`), with `n = 1` pair (10 pairs minimum required for inference).
2. **Not evolutionary selection or adaptation:** `LiveExperimentAdapter` logs founding individuals without multi-generation replacement.
3. **Not map-general:** this run covers a single generated `256 × 256` terrain (`seed = 1337`).
4. **Not desktop or GPU readiness:** status remains **`headless adapter verified`** only, not full Tauri/WGPU/WebView product validation.
5. **Not cross-OS bit-identity:** per [`DETERMINISM_CONTRACT.md` §1](../reference/DETERMINISM_CONTRACT.md#1-lời-hứa-và-giới-hạn-của-nó), determinism guarantees bit-identical digests for *the same manifest + same build on the same platform*; `final_checksum` and generated terrain checksums differ across OS `libm` targets (`determinism_matrix.md`).

---

## Further Reading

- [`docs/how-to/README.md`](../how-to/README.md) — index of how-to guides (including `docs/how-to/REPRODUCE_E2.md` for full 12-seed `--ensemble`, `--replay`, and `--duration-rung`).
- [`docs/reference/README.md`](../reference/README.md) — canonical contracts (`EVOLUTION_EXPERIMENT_CONTRACT.md`, `DETERMINISM_CONTRACT.md`, `ENERGY_LEDGER_CONTRACT.md`).
- [`WORLD_SIMULATION_PLAN.md` §3.2](../../WORLD_SIMULATION_PLAN.md#32-world-laws-và-experiment-manifest) — `ExperimentManifest` and `WorldLawSet`.
- [`ADR-0003`](../decisions/ADR-0003-evolved-per-agent-brains.md), [`RESULT.md`](../../artifacts/experiments/e2-evolved-brain-default/RESULT.md), and [E2 test record](../ai/testing/2026-07-27-experiment-e2-evolved-brain-default.md).
