# E2 result, re-run under amendment 1 (2026-11-16)

This file replaces the 2026-07-27 `RESULT.md`, which is kept unedited at
`superseded/2026-07-27-original-run-windows-pre-dormant-eu/RESULT.md` together with that run's
artifacts and their original `checksums.sha256`. Read that file for the full account of the
question, the audit disclosure, the mechanism of the floor, and the defects found in July. None of it
is repeated here; this file records only what the re-run measured and how it compares.

> Model status, unchanged: **headless adapter verified.** No desktop app, Tauri handle, window, GPU
> device, renderer, learner thread or evolution thread existed in the process that produced these
> numbers. Nothing here may be quoted as "the live world is experiment-ready".

## 1. Why there was a re-run

Commit `970d963` (2026-08-25, M3 dormancy) added `live.dormant_eu` to `observable_ids` in the four
manifest fixtures, because `prereg_e2_manifest_tests` pins that list to `LIVE_OBSERVABLE_IDS`. The
July artifacts carried the old manifests, so `scripts/verify_e2_artifacts.mjs` (CI step "E2 artifacts
match the committed preregistration") failed with four byte-mismatches. Decision QD-E2M = B: amend
the preregistration by a dated block (`amendment_1_live_dormant_eu` in `e2-preregistration.json`) and
re-run for real. The manifests were not copied beside the old results. Scientific parameters are
unchanged (question, hypotheses, metrics, thresholds, seeds, N, smoke seed, ladder, decision rule).

What the amendment does **not** hold constant: the engine code (everything on
`feat/ecological-selection` since July) and the platform (macOS arm64; the original was Windows
x86_64). A difference from July cannot be attributed to either alone.

## 2. What was run

Build commit `900ba2d` (clean tree at build; the runs' own untracked outputs make later `git status`
dirty, as in July), rustc 1.98.1, release, `--features desktop`, `-j 2`. Binary SHA-256
`44b9c370813e4c05044daf36dbc683939957c5ec8f49fea7bdd4ea1b8ea8b203`. Smoke (seed 999983) at 18,000
ticks, then `execution-lock.json` (rung 18,000, written before the ensemble), then the ensemble at
18,000 ticks, then the two `--replay` checks that July also ran (control 711001, treatment 700001).
Build 3 m 45 s; smoke 5.4 s; ensemble 66.9 s (ceiling 90 min). `node scripts/verify_e2_artifacts.mjs`
passes.

## 3. Result

12 of 12 complete pairs. **H1 is null again**: `live.mean_agent_energy` is exactly `0.0` in both arms
on all twelve seeds at tick 18,000; paired delta 0, SD 0, CI [0, 0], `d_z` undefined. Not material.
H2, H3, H4 are exactly zero as before. H5 is clean. The new observable `live.dormant_eu` is 0 in both
arms on every seed (I did not trace why; presumably nothing in this harness hibernates an agent). The floor described in July section 4
(no starvation death, no replacement in the adapter) is still present, so the registered endpoint
still has no power to distinguish the arms. Everything in July's "what this cannot establish" applies
unchanged.

## 4. Old versus new

| quantity | 2026-07-27 (Windows, commit 9c57184) | 2026-11-16 (macOS arm64, commit 900ba2d) |
|---|---|---|
| H1 verdict | null, not material | null, not material |
| H1 paired delta / CI / d_z | 0 / [0, 0] / undefined | 0 / [0, 0] / undefined |
| per-seed H1 deltas | all 12 exactly 0 | all 12 exactly 0 |
| complete pairs | 12 of 12 | 12 of 12 |
| `live.agent_count`, `animals_eu`, `predator_count` deltas | 0, 0, 0 | 0, 0, 0 |
| `live.closed_eu_total` control mean | 154587.249864 | 155971.799862 |
| `live.closed_eu_total` paired delta (d_z) | -2.56e-9 (-0.066) | -8.49e-11 (-0.272) |
| mean first-zero tick, control / treatment | 5250 / 8650 | 5250 / 8650 |
| treatment later / equal / earlier (seeds) | 10 / 1 / 1 | 10 / 1 / 1 |
| tick-600 energy delta: mean, SD, d_z, negative seeds | +7.322, 19.119, 0.383, 6 | +7.339, 19.134, 0.384, 6 |
| per-seed final checksums | e.g. 700001 control 3305441016 | e.g. 700001 control 2046314586 (all 24 differ) |
| replay checksum equals ensemble checksum | yes | yes (711001 control 2323209679, 700001 treatment 1081145212) |
| ensemble wall clock | 157.2 s | 66.9 s |
| smoke ensemble time | 10.2 s | 5.4 s |

The verdict did not change. The per-seed first-zero ticks are identical in all 24 runs, and the
tick-600 deltas agree to within about 0.15 EU per seed. The seed 700001 trajectory is the same shape
(control 84.4 vs 84.3 at tick 1200, both reaching 0.0 by tick 8400).

The raw checksums and the closed-EU stock (+1384 EU, about 0.9 %) do differ in every run, so the world
state is not bit-identical to July. That is expected if the engine changed between the two commits;
I did not bisect which change moved the stock, so that attribution is a hypothesis. Likewise I did not
isolate any platform float effect: a Windows run of the current commit would be needed, and none was
made. The time and tick-600 differences are small and were not tested for cause.

## 5. Observations, not findings

The "treatment reaches the floor later than control in 10 of 12 seeds" observation of July (section 6a)
reproduces exactly. It remains an unregistered, hypothesis-generating observation measured at
600-tick resolution, not an E2 result, and it still points opposite to H1's predicted direction. The
conclusion of the July report stands: this experiment did not measure the direction of the brain
effect, in either direction. By the preregistered decision rule, H1 null means the default stays
opt-in.

## 6. Reproduction (macOS/Linux form of the registered commands)

```
cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop \
  --example run_e2_brain_experiment -j 2

src-tauri/target/release/examples/run_e2_brain_experiment --ensemble \
  --manifest-dir src-tauri/tests/fixtures/experiments_e2 \
  --out artifacts/experiments/e2-evolved-brain-default
```

Never `cargo run`.
