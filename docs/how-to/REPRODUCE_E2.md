---
title: How to Reproduce Experiment E2 (evolved-brain vs shared BrainModel)
status: active
owner: maintainers
last_reviewed: 2026-11-23
review_cycle: per-release
---

# How to Reproduce Experiment E2 (`evolved-brain` vs `shared BrainModel`)

This guide covers full end-to-end reproduction of the preregistered **E2** experiment on the headless live world (`LiveExperimentAdapter`): running all preregistration seam tests (`E2-G1..G4`, `E2-G6`), calibrating with `--smoke`, executing the 12-seed `--ensemble` and `--replay` (`E2-G5`), and verifying artifacts independently with `npm run check:e2`.

> **First time setting up or looking for a single quick smoke run?** Follow the onboarding tutorial (`01-first-headless-experiment.md`, indexed in [`docs/tutorials/README.md`](../tutorials/README.md)) first; this how-to focuses on the full reproduction and audit workflow without repeating clone/toolchain setup.
>
> **Preregistration & contracts:** [`e2-preregistration.json`](../../src-tauri/tests/fixtures/experiments_e2/e2-preregistration.json), [`RESULT.md`](../../artifacts/experiments/e2-evolved-brain-default/RESULT.md), [`2026-07-27-experiment-e2-evolved-brain-default.md`](../ai/testing/2026-07-27-experiment-e2-evolved-brain-default.md), and [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) *(proposed)* (`ER07`, `ER08`, `ER11`).
>
> **Inference boundary:** conclusions from this runner establish **`headless adapter verified`** only, on one generated `256 × 256` world, without launching the desktop app or multi-generation evolutionary replacement.

---

## Step 1 — Run Preregistration, Seam, and Frozen-Manifest Tests (`E2-G1..G4`, `E2-G6`)

Run the three backend test targets that lock `live.evolved_brains` isolation, `SimRng` stream alignment, `WorldIdentity` equality, the `cargo run` prohibition, and byte-identity of the published E2 manifests:

<!-- doc-test -->
```bash
cargo test --manifest-path src-tauri/Cargo.toml --test e2_seam_tests
cargo test --manifest-path src-tauri/Cargo.toml --test prereg_e2_manifest_tests
cargo test --manifest-path src-tauri/Cargo.toml --test e2_frozen_manifest_tests
```

Summarized results — *not raw verbatim stdout* — on `macOS arm64` (`2026-11-22`, commit `03b9d3d`; authoritative repo totals live in [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền)):

```text
[Summary across the 3 commands — not verbatim stdout]
e2_seam_tests: 9 passed, 0 failed, 0 ignored
prereg_e2_manifest_tests: 9 passed, 0 failed, 0 ignored
e2_frozen_manifest_tests: 2 passed, 0 failed, 0 ignored
```

Summary of the six E2 gates verified across tests and runner output:

| Gate | Guarantee Verified |
|---|---|
| `E2-G1` | `live.evolved_brains = 1.0` constructs `10/10` founders carrying `AgentBrain`; control carries `0/10` |
| `E2-G2` | Manifests omitting `live.evolved_brains` preserve legacy behavior identically |
| `E2-G3` | `SimRng` seed, stream position, and next 8 draws match between arms immediately after genesis |
| `E2-G4` | `FactorDiff` allows differences solely in `initial_conditions` (`live.evolved_brains`) |
| `E2-G5` | `--replay` reproduces the exact `(seed, arm)` `final_checksum` |
| `E2-G6` | `WorldIdentity` in `provenance.json` matches between control and treatment arms (checksums are platform-specific: e.g., `1212187560` on macOS arm64, `3152323152` on Windows, `22481267` on Linux per [`determinism_matrix.md`](../paper/data/determinism_matrix.md)) |

---

## Step 2 — Build `--release --features desktop` and Run `--smoke` Calibration

Because E2 preregistration, `e2_seam_tests`, and `scripts/verify_e2_artifacts.mjs` forbid `cargo run`, compile `run_e2_brain_experiment` first and invoke the executable directly into a gitignored directory under `src-tauri/target/` (never overwrite [`artifacts/experiments/e2-evolved-brain-default/`](../../artifacts/experiments/e2-evolved-brain-default)):

<!-- doc-test -->
```bash
cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop --example run_e2_brain_experiment
./src-tauri/target/release/examples/run_e2_brain_experiment --smoke \
  --manifest-dir src-tauri/tests/fixtures/experiments_e2 \
  --out src-tauri/target/doc2-e2-repro
```

> **Windows PowerShell *(not verified on Windows here)*:**
>
> ```powershell
> .\src-tauri\target\release\examples\run_e2_brain_experiment.exe --smoke `
>   --manifest-dir src-tauri\tests\fixtures\experiments_e2 `
>   --out src-tauri\target\doc2-e2-repro
> ```

Observed output (`macOS arm64`, `2026-11-22`, commit `03b9d3d`):

```text
E2 Smoke: 1 seeds x 2 arms x 18000 ticks, sample period 600
  integrity: control brains 0 / treatment brains 10 of 10 founders; ecology stream identical: true; world identity identical: true
  finished in 4.4s: 1 complete pairs of 1
  artifacts written to src-tauri/target/doc2-e2-repro/smoke
```

> **Duration ladder (`--duration-rung`):** the runner accepts `--duration-rung <ticks>` restricted to `[18000, 12000, 6000]` registered in `e2-preregistration.json` (default `18000`). Lock one rung once from `--smoke` calibration to stay within the 90-minute ceiling before running `--ensemble`.

---

## Step 3 — Run Full 12-Seed `--ensemble` and `--replay` (`E2-G5`)

After smoke timing is confirmed, run all 12 registered seeds (`700001..711001`) with `--ensemble`, then verify bit-exact reproducibility (`E2-G5`) on both arms using `--replay` (passing both `--replay-seed` from the manifest and `--replay-arm control|treatment`):

```bash
./src-tauri/target/release/examples/run_e2_brain_experiment --ensemble \
  --manifest-dir src-tauri/tests/fixtures/experiments_e2 \
  --out src-tauri/target/doc2-e2-repro

./src-tauri/target/release/examples/run_e2_brain_experiment --replay \
  --manifest-dir src-tauri/tests/fixtures/experiments_e2 \
  --out src-tauri/target/doc2-e2-repro \
  --replay-seed 700001 \
  --replay-arm treatment

./src-tauri/target/release/examples/run_e2_brain_experiment --replay \
  --manifest-dir src-tauri/tests/fixtures/experiments_e2 \
  --out src-tauri/target/doc2-e2-repro \
  --replay-seed 700001 \
  --replay-arm control
```

Observed output (`macOS arm64`, `2026-11-22`, commit `03b9d3d`; `52.3s` across all 12 pairs):

```text
E2 Ensemble: 12 seeds x 2 arms x 18000 ticks, sample period 600
  integrity: control brains 0 / treatment brains 10 of 10 founders; ecology stream identical: true; world identity identical: true
  finished in 52.3s: 12 complete pairs of 12
  artifacts written to src-tauri/target/doc2-e2-repro

replay treatment seed 700001: checksum 1081145212 (completed in 2.4s) -> src-tauri/target/doc2-e2-repro/replay/replay-treatment-seed-700001-t18000.json
replay control seed 700001: checksum 3017397262 (completed in 2.1s) -> src-tauri/target/doc2-e2-repro/replay/replay-control-seed-700001-t18000.json
```

Verify that `final_checksum` in `replay/replay-*-seed-700001-t18000.json` matches the `700001` rows in `runs.csv` from **that same run** (`3017397262` for `control`, `1081145212` for `treatment`).

> **Why `final_checksum` may differ from committed repo artifacts:** per [`DETERMINISM_CONTRACT.md` §1](../reference/DETERMINISM_CONTRACT.md#1-lời-hứa-và-giới-hạn-của-nó), bit-identical replay applies to **the same manifest + same build on the same platform**. Currently `artifacts/experiments/e2-evolved-brain-default/` stores a `2026-11-16` `macOS arm64` re-run at `900ba2d` (22 observables), while the original `2026-07-27` `Windows x86_64` run is archived in [`superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md`](../../artifacts/experiments/e2-evolved-brain-default/superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md). Local checksums differ from committed runs due to both cross-OS `libm` math (`determinism_matrix.md`) and post-`900ba2d` engine updates; always compare `--replay` against `runs.csv` from the **same build and run**.

---

## Step 4 — Verify Digests and Deltas with `npm run check:e2`

Run [`scripts/verify_e2_artifacts.mjs`](../../scripts/verify_e2_artifacts.mjs) (`node:crypto`) against both your output directory and the committed reference artifact:

<!-- doc-test -->
```bash
npm run check:e2 -- src-tauri/target/doc2-e2-repro
npm run check:e2
```

Observed output (`macOS arm64`, `2026-11-22`, commit `03b9d3d`) when checking a `--smoke`-only directory:

```text
note  analysis: no ensemble has been run yet (no paired-report.json at the artifact root)
note  smoke: recomputed 27 per-seed deltas
note  analysis: no execution-lock.json — the duration rung was not locked from a smoke calibration

OK — every E2 artifact in .../src-tauri/target/doc2-e2-repro verifies against node:crypto and the preregistration.
```

And after completing Step 3 (`--ensemble`: 12 seeds × 27 observables = `324` deltas plus `27` smoke deltas):

```text
note  analysis: recomputed 324 per-seed deltas
note  smoke: recomputed 27 per-seed deltas
note  analysis: no execution-lock.json — the duration rung was not locked from a smoke calibration

OK — every E2 artifact in .../src-tauri/target/doc2-e2-repro verifies against node:crypto and the preregistration.

> node scripts/verify_e2_artifacts.mjs
note  analysis: recomputed 264 per-seed deltas
note  smoke: recomputed 22 per-seed deltas

OK — every E2 artifact in .../artifacts/experiments/e2-evolved-brain-default verifies against node:crypto and the preregistration.
```

Generated files under `--out`:
- `manifests/` (3 JSON files byte-identical to `HEAD:src-tauri/tests/fixtures/experiments_e2/*`), `paired-report.json`, `effects.json`, `per-seed-deltas.csv`, `runs.csv`, `provenance.json`, `summary.md`, `checksums.sha256`.
- `smoke/` (same layout for calibration seed `999983`) and `replay/` (`replay-<arm>-seed-<seed>-t<rung>.json`).
- Committed `artifacts/experiments/e2-evolved-brain-default/` additionally tracks `execution-lock.json`, `RESULT.md`, and `superseded/`.

---

## Next Steps

- Run arbitrary `anima-lab` ensembles, sweeps, and branch trees: [RUN_EXPERIMENTS.md](RUN_EXPERIMENTS.md).
- Build new verification gates like `verify_e2_artifacts.mjs`: [ADD_A_GATE.md](ADD_A_GATE.md).
