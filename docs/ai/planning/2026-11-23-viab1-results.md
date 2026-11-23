---
phase: planning
feature: ecological-selection
kind: đo
title: Card VIAB-1 (W15) — Shared Viability Exploration Results for E3b and SEL2-P2
description: Measurement-only, body-mass-blind viability exploration across 55 cells (30 E3 and 25 SEL2-P; 1,920 runs on seeds 880001..=880012) at Phase A d90be0bd and Phase B 4794b6fc, including completion of sel2p_b2_s075_k2k_fl2_12s after the 2026-11-20 Windows Update interruption
status: completed
owner: anima-case
last_reviewed: 2026-11-23
---

# Card `VIAB-1` (`W15`) — Shared Viability Exploration Results for `E3b` and `SEL2-P2`

## 1. Provenance, Commits, Seeds, Blindness & `QD-VIAB1C` Check

- **Host**: `Windows 1` (`DESKTOP-3EPV830`, `windows/x86_64`), release binary `D:\teamwork_projects\viab1_target\release\viab1_probe.exe`; desktop app never launched.
- **Commits executed**:
  - **Phase A (`30` cells, `1,080` runs)**: clean detached trunk `d90be0bd26fe637e2a3db1ce0c238845cf4211fd` (`18` E3 cells × `12` seeds = `648` runs; `6` SEL2-P cells × `12` seeds + `6` SEL2-P screening cells × `6` seeds = `432` runs).
  - **Phase B (`25` cells, `840` runs)**: clean detached post-PR `#186` (`FOUNDER-LAYOUT`) trunk `4794b6fca720dc0fad4b3e66aa377a15e341bae6` (`12` E3 cells × `12` seeds = `432` runs; `9` SEL2-P Stage B1 screening cells × `6` seeds = `216` runs; `4` SEL2-P Stage B2 confirmation cells × `12` seeds = `192` runs).
- **Total run coverage**: **`55` cells**, **`1,920` simulation runs** (`1487.8s` E3 wall + `19030.6s` SEL2-P wall = **`20518.4s`** total).
- **Exploratory seeds**: `880001..=880012` (`12`-seed cells) and `880001..=880006` (`6`-seed mechanistic screen cells); no preregistered or smoke seed (`999961`, E-SEL1, E2, E3) was executed.
- **World source & identity**: `ANIMA_WORLD_ARTIFACT` unset and `%TEMP%\anima_world_artifact.anmw` absent before execution; `all_terrain_generated = true` in **`55/55`** JSON outputs (`WorldIdentity(seed=1337, generator_version=1, width=256, height=256, checksum=3152323152)` across every run, informational only and never pinned).
- **Body-mass blindness**: `viab1_probe` removes all `trait_mass*` observables before execution and never reads, prints, or branches on `live.trait_mass_*`. Programmatic audit across all `55` cell JSONs, [summary.json](../../../artifacts/experiments/viab1-shared-viability/summary.json), and [cells_summary.csv](../../../artifacts/experiments/viab1-shared-viability/cells_summary.csv): **`0` `trait_mass` matches**.
- **`QD-VIAB1C` check (PR `#212`)**: `LiveWorldConfig::default()` sets `grass_steering: false` and `land_mask: false` ([live_experiment.rs](../../../src-tauri/src/core/live_experiment.rs)); `live.land_mask` is absent (`false`) in all E3/SEL2-P base manifests and all `55/55` VIAB-1 cell overrides (`0` families combine `live.grass_steering` and `live.land_mask`).
- **Probe source provenance (stored on Hub pending lead path authorization)**:
  - Hub copy: `test_runs/anima-case_VIAB-1/viab1_probe/`
  - `src/main.rs` (`67,763` B): SHA-256 `df07eee73b2b344e0201c5d0524a32537a4890ae7d69152928d32d06a6aba0f3`
  - `Cargo.toml` (`289` B): SHA-256 `09f6c9592af1d77c2d1cc6a47907055704e5ca458aa8602348a962f7549d5c6c`
  - `Cargo.lock` (`256,123` B): SHA-256 `c4d024b487f29947285f89c29a5ded4a00b01c0522d8fe803b1d9d81e55bf920`
  - Compiled binary `D:\teamwork_projects\viab1_target\release\viab1_probe.exe` (Option 1 pre-run and post-run SHA-256): `79167208818D1CF6BCD212CC420DEEC175F93D0F2CADA5355C36A91144F77309`

## 2. 2026-11-20 Windows Update Interruption & Option 1 Completion

- During Phase B Stage B2 batch 2/2 on `2026-11-20`, `sel2p_b1_s075_k2k_fl1_12s.json` completed at `2026-11-20T13:19:46Z` (`560.2s`), and `viab1_probe.exe` started the 55th cell (`sel2p_b2_s075_k2k_fl2_12s`).
- At `2026-11-20T13:21:11Z` (`20:21:11+07:00`), Windows Update (`C:\Windows\uus\AMD64\MoNotificationUx.exe`) initiated a planned OS restart (Windows System Event Log **Event ID `1074`**, `User32`, `Operating System: Service pack (Planned)`, `Reason Code: 0x80020010`). One second later at `2026-11-20T13:21:12Z`, `viab1_probe.exe` and the Hub watcher exited with code `1073807364` (`0x40010004`).
- Under lead authorization (`UNBLOCK VIAB-1` Option 1, `2026-11-23`), **only** the missing cell `sel2p_b2_s075_k2k_fl2_12s` (`48` runs) was executed without rebuilding, using the exact existing binary (`PRE_SHA256 == POST_SHA256 == 79167208818D1CF6BCD212CC420DEEC175F93D0F2CADA5355C36A91144F77309`) from `2026-11-23T05:15:37.0764306Z` to `2026-11-23T06:39:24.9924281Z` (`5027.5s` wall, `PROBE_EXIT=0`).

## 3. Per-Cell Viability Tables & Exact Gate Pass/Fail

Card targets: every arm survives in `>= 11/12` seeds, `min_N >= 10` per arm in `>= 11/12` seeds, E3 `WP-2` (`b1`, `b2`, `c`) in `12/12`, and SEL2-P `C1..C5` + `drift_valid`.

### Table 3A — E3 Family (`30` cells × `12` seeds `880001..=880012` × `3` arms = `1,080` runs, `18,000` ticks)

| Cell ID | Phase | Overrides vs Registered E3 | Wall (s) | Surv (`U/S/T/All`) | `min_N >= 10` (`U/S/T/All`) | `min_N` Range (`U` / `S` / `T`) | `mean_N` (`U` / `S` / `T`) | `WP-2` (`b1/b2/c/all`) | `b1` Shift (`T` vs `U`) | `b2` Mismatch (`T` vs `S`) | Starv % (`U/S/T`) | Max `|EU drift|` | Card Targets |
|---|---|---|---:|---|---|---|---|---|---|---|---|---:|---|
| `e3_a_reg` | A (`d90be0bd`) | `registered` | 39.2 | 10/5/8/5 | 8/0/4/0 | 0..37 / 0..3 / 0..20 | 55.4 / 21.4 / 43.9 | 12/12/12/12 | `-0.562..-0.507` | `-0.0067..+0.0053` | 25%/9%/18% | `1.50e-08` | FAIL (surv<11, minN<10) |
| `e3_b_strict` | A (`d90be0bd`) | `strict=1` | 41.2 | 10/5/9/5 | 8/1/6/1 | 0..52 / 0..14 / 0..34 | 52.5 / 22.2 / 37.9 | 12/12/12/12 | `-0.563..-0.505` | `-0.0027..+0.0064` | 17%/5%/11% | `1.50e-08` | FAIL (surv<11, minN<10) |
| `e3_c_grass` | A (`d90be0bd`) | `grass=1` | 72.9 | 11/5/7/3 | 9/1/4/1 | 0..48 / 0..12 / 0..18 | 76.2 / 28.6 / 54.0 | 12/12/12/12 | `-0.550..-0.452` | `-0.0226..+0.0304` | 14%/8%/13% | `1.14e-08` | FAIL (surv<11, minN<10) |
| `e3_d_both` | A (`d90be0bd`) | `strict=1, grass=1` | 47.1 | 9/4/8/3 | 7/1/4/0 | 0..63 / 0..11 / 0..24 | 49.8 / 25.2 / 46.1 | 12/12/12/12 | `-0.576..-0.487` | `-0.0193..+0.0099` | 11%/6%/10% | `1.32e-08` | FAIL (surv<11, minN<10) |
| `e3_e1_sym100` | A (`d90be0bd`) | `strict=1, grass=1, f=100, cap=100` | 54.5 | 11/6/8/6 | 8/0/5/0 | 0..75 / 0..7 / 0..62 | 70.8 / 27.7 / 56.7 | 12/12/12/12 | `-0.566..-0.463` | `-0.0268..+0.0053` | 15%/10%/12% | `1.73e-08` | FAIL (surv<11, minN<10) |
| `e3_e2_sym160` | A (`d90be0bd`) | `strict=1, grass=1, f=160, cap=160` | 94.1 | 11/6/8/5 | 8/0/7/0 | 0..49 / 0..9 / 0..75 | 94.7 / 39.9 / 95.3 | 12/12/12/12 | `-0.563..-0.436` | `-0.0522..+0.0100` | 20%/13%/13% | `1.69e-08` | FAIL (surv<11, minN<10) |
| `e3_f1_lakes16` | A (`d90be0bd`) | `strict=1, grass=1, lakes=16, ll=1` | 42.1 | 8/12/12/8 | 2/0/5/0 | 0..25 / 2..7 / 2..49 | 36.8 / 26.4 / 40.1 | 12/12/12/12 | `-0.564..-0.535` | `-0.0090..+0.0051` | 41%/27%/23% | `4.27e-08` | FAIL (surv<11, minN<10) |
| `e3_f2_dehyd960` | A (`d90be0bd`) | `strict=1, grass=1, dehyd=960` | 53.3 | 12/8/11/8 | 11/4/8/4 | 5..61 / 0..22 / 0..55 | 75.0 / 38.1 / 71.5 | 12/12/12/12 | `-0.521..-0.495` | `-0.0234..+0.0047` | 32%/21%/17% | `1.48e-08` | FAIL (surv<11, minN<10) |
| `e3_f3_lakes16_dehyd960` | A (`d90be0bd`) | `strict=1, grass=1, lakes=16, ll=1, dehyd=960` | 50.2 | 8/12/12/8 | 1/2/5/0 | 0..21 / 3..10 / 1..66 | 38.0 / 30.0 / 46.8 | 12/12/12/12 | `-0.564..-0.533` | `-0.0112..+0.0053` | 67%/49%/40% | `3.82e-08` | FAIL (surv<11, minN<10) |
| `e3_f4_sym100_water` | A (`d90be0bd`) | `strict=1, grass=1, f=100, cap=100, lakes=16, ll=1, dehyd=960` | 44.1 | 10/12/12/10 | 1/2/6/0 | 0..10 / 2..12 / 3..41 | 34.7 / 31.7 / 44.3 | 12/12/12/12 | `-0.566..-0.537` | `+0.0008..+0.0053` | 77%/48%/42% | `4.37e-08` | FAIL (surv<11, minN<10) |
| `e3_f5_sym160_water` | A (`d90be0bd`) | `strict=1, grass=1, f=160, cap=160, lakes=16, ll=1, dehyd=960` | 54.4 | 9/12/12/9 | 6/2/6/1 | 0..42 / 2..11 / 1..46 | 59.3 / 45.4 / 58.7 | 12/12/12/12 | `-0.560..-0.501` | `-0.0000..+0.0108` | 74%/48%/48% | `4.58e-08` | FAIL (surv<11, minN<10) |
| `e3_f6_herb_sym100_water` | A (`d90be0bd`) | `strict=1, grass=1, f=100, cap=100, lakes=16, ll=1, dehyd=960, pred=0` | 49.8 | 8/12/12/8 | 5/8/6/5 | 0..100 / 4..41 / 1..78 | 55.0 / 42.2 / 45.8 | 12/12/12/12 | `-0.558..-0.522` | `-0.0030..+0.0114` | 52%/30%/36% | `4.22e-08` | FAIL (surv<11, minN<10) |
| `e3_g1_strict_lakes16` | A (`d90be0bd`) | `strict=1, lakes=16, ll=1` | 26.2 | 9/12/12/9 | 9/6/7/5 | 0..70 / 1..37 / 1..61 | 59.7 / 38.0 / 49.0 | 12/12/12/12 | `-0.566..-0.514` | `-0.0057..+0.0053` | 40%/21%/26% | `3.94e-08` | FAIL (surv<11, minN<10) |
| `e3_g2_strict_lakes16_dehyd960` | A (`d90be0bd`) | `strict=1, lakes=16, ll=1, dehyd=960` | 26.8 | 10/12/12/10 | 8/9/9/7 | 0..71 / 1..44 / 2..70 | 58.5 / 45.8 / 51.4 | 12/12/12/12 | `-0.563..-0.531` | `-0.0020..+0.0094` | 71%/42%/45% | `4.98e-08` | FAIL (surv<11, minN<10) |
| `e3_g3_strict_sym100_water` | A (`d90be0bd`) | `strict=1, f=100, cap=100, lakes=16, ll=1, dehyd=960` | 30.6 | 11/12/12/11 | 9/8/11/7 | 0..78 / 1..41 / 1..82 | 71.8 / 46.9 / 72.3 | 12/12/12/12 | `-0.572..-0.512` | `-0.0061..+0.0134` | 73%/41%/43% | `3.61e-08` | FAIL (minN<10) |
| `e3_g4_strict_sym160_water` | A (`d90be0bd`) | `strict=1, f=160, cap=160, lakes=16, ll=1, dehyd=960` | 36.2 | 11/12/11/11 | 10/10/8/8 | 0..83 / 1..31 / 0..59 | 91.1 / 57.4 / 73.6 | 12/12/12/12 | `-0.565..-0.493` | `-0.0051..+0.0093` | 75%/44%/50% | `4.27e-08` | FAIL (minN<10) |
| `e3_g5_strict_herb_sym100_water` | A (`d90be0bd`) | `strict=1, f=100, cap=100, lakes=16, ll=1, dehyd=960, pred=0` | 29.1 | 11/12/11/11 | 8/10/10/8 | 0..100 / 3..75 / 0..86 | 70.6 / 53.0 / 63.3 | 12/12/12/12 | `-0.557..-0.517` | `-0.0023..+0.0048` | 63%/29%/40% | `1.78e-08` | FAIL (minN<10) |
| `e3_g6_strict_herb_sym160_water` | A (`d90be0bd`) | `strict=1, f=160, cap=160, lakes=16, ll=1, dehyd=960, pred=0` | 36.4 | 10/12/12/10 | 9/9/9/7 | 0..119 / 5..70 / 1..87 | 95.1 / 65.7 / 71.4 | 12/12/12/12 | `-0.558..-0.492` | `-0.0068..+0.0064` | 61%/34%/43% | `5.74e-08` | FAIL (surv<11, minN<10) |
| `e3_b1_reg_fl1` | B (`4794b6fc`) | `fl=1` | 45.0 | 9/11/9/8 | 7/1/7/1 | 0..72 / 0..20 / 0..77 | 68.4 / 33.7 / 56.8 | 12/12/12/12 | `-0.559..-0.465` | `-0.0138..+0.0054` | 18%/12%/20% | `1.82e-08` | FAIL (surv<11, minN<10) |
| `e3_b2_reg_fl2` | B (`4794b6fc`) | `fl=2` | 48.7 | 11/9/8/7 | 10/2/6/2 | 0..80 / 0..34 / 0..69 | 74.3 / 30.2 / 58.9 | 12/12/12/12 | `-0.564..-0.447` | `-0.0088..+0.0053` | 25%/15%/27% | `2.95e-08` | FAIL (surv<11, minN<10) |
| `e3_b1_strict_fl1` | B (`4794b6fc`) | `strict=1, fl=1` | 43.0 | 9/12/10/8 | 6/4/7/4 | 0..80 / 1..15 / 0..80 | 52.7 / 36.8 / 53.7 | 12/12/12/12 | `-0.560..-0.506` | `-0.0080..+0.0179` | 16%/8%/13% | `2.46e-08` | FAIL (surv<11, minN<10) |
| `e3_b2_strict_fl2` | B (`4794b6fc`) | `strict=1, fl=2` | 44.8 | 11/12/8/8 | 9/4/5/4 | 0..58 / 1..14 / 0..79 | 59.3 / 33.7 / 41.0 | 12/12/12/12 | `-0.530..-0.471` | `-0.0108..+0.0129` | 25%/12%/23% | `1.48e-08` | FAIL (surv<11, minN<10) |
| `e3_b1_g2_fl1` | B (`4794b6fc`) | `strict=1, lakes=16, ll=1, dehyd=960, fl=1` | 48.4 | 11/12/11/10 | 9/9/11/7 | 0..64 / 4..59 / 0..80 | 65.6 / 57.1 / 65.4 | 12/12/12/12 | `-0.549..-0.516` | `-0.0025..+0.0055` | 77%/42%/49% | `2.70e-08` | FAIL (minN<10) |
| `e3_b2_g2_fl2` | B (`4794b6fc`) | `strict=1, lakes=16, ll=1, dehyd=960, fl=2` | 53.1 | 10/12/12/10 | 9/10/9/7 | 0..70 / 5..44 / 1..76 | 66.3 / 56.3 / 55.1 | 12/12/12/12 | `-0.537..-0.502` | `-0.0032..+0.0075` | 84%/42%/53% | `3.67e-08` | FAIL (surv<11, minN<10) |
| `e3_b1_g3_fl1` | B (`4794b6fc`) | `strict=1, f=100, cap=100, lakes=16, ll=1, dehyd=960, fl=1` | 56.7 | 11/12/12/11 | 10/9/9/8 | 0..80 / 2..58 / 4..96 | 79.4 / 54.2 / 74.1 | 12/12/12/12 | `-0.558..-0.495` | `-0.0055..+0.0052` | 76%/40%/47% | `4.22e-08` | FAIL (minN<10) |
| `e3_b2_g3_fl2` | B (`4794b6fc`) | `strict=1, f=100, cap=100, lakes=16, ll=1, dehyd=960, fl=2` | 59.3 | 10/12/12/10 | 8/11/11/7 | 0..95 / 9..53 / 2..79 | 72.1 / 59.2 / 70.3 | 12/12/12/12 | `-0.566..-0.485` | `-0.0034..+0.0071` | 83%/37%/45% | `4.33e-08` | FAIL (surv<11, minN<10) |
| `e3_b1_g4_fl1` | B (`4794b6fc`) | `strict=1, f=160, cap=160, lakes=16, ll=1, dehyd=960, fl=1` | 70.0 | 11/12/12/11 | 11/11/11/11 | 0..74 / 7..65 / 2..115 | 100.4 / 79.8 / 99.5 | 12/12/12/12 | `-0.549..-0.448` | `-0.0075..+0.0067` | 75%/38%/47% | `2.99e-08` | **PASS** |
| `e3_b2_g4_fl2` | B (`4794b6fc`) | `strict=1, f=160, cap=160, lakes=16, ll=1, dehyd=960, fl=2` | 72.9 | 9/12/12/9 | 9/11/10/7 | 0..81 / 1..38 / 5..62 | 89.9 / 75.9 / 86.9 | 12/12/12/12 | `-0.567..-0.434` | `-0.0048..+0.0080` | 85%/36%/49% | `5.85e-08` | FAIL (surv<11, minN<10) |
| `e3_b1_g5_fl1` | B (`4794b6fc`) | `strict=1, f=100, cap=100, lakes=16, ll=1, dehyd=960, pred=0, fl=1` | 57.5 | 12/12/12/12 | 10/12/9/7 | 1..100 / 11..95 / 3..100 | 81.8 / 72.5 / 70.0 | 12/12/12/12 | `-0.530..-0.499` | `-0.0010..+0.0119` | 64%/26%/41% | `2.62e-08` | FAIL (minN<10) |
| `e3_b2_g5_fl2` | B (`4794b6fc`) | `strict=1, f=100, cap=100, lakes=16, ll=1, dehyd=960, pred=0, fl=2` | 60.2 | 10/12/12/10 | 9/11/11/8 | 0..100 / 8..100 / 3..100 | 79.6 / 70.7 / 76.5 | 12/12/12/12 | `-0.532..-0.509` | `-0.0032..+0.0094` | 62%/25%/38% | `1.77e-08` | FAIL (surv<11, minN<10) |

### Table 3B — SEL2-P Family (`25` cells, `200,000` ticks, `840` total simulation runs)

Joint gate columns (`C1m5 / C1s10 / C1b10 / C2 / C3 / C4 / C5 / d_valid / pass`) follow the SEL2-P preregistration directly (where `C1..C5` require both `selected` and `scarce_mut0` to meet the predicate); `sel_only` reports the same predicates on the `selected` arm alone (`C1m5 / C1m10 / C2 / C3 / C4 / C5`). Cells with `N_seeds = 6` are exploratory screens.

| Cell ID | Phase | Config Summary | `N_seeds` | Wall (s) | Surv (`B/S/M/D`) | `min_N >= 10` (`B/S/M/D`) | `min_N` Range (`B`/`S`/`M`/`D`) | `g_S` (surviving) | Joint Gates (`C1m5/C1s10/C1b10/C2/C3/C4/C5/dv/pass`) | `sel_only` (`C1m5/C1m10/C2/C3/C4/C5`) | Drift `mis(cap/ext/ins/d)` | Max `|EU drift|` | Card Targets |
|---|---|---|---:|---:|---|---|---|---|---|---|---|---|---|
| `sel2p_a_reg` | A (`d90be0bd`) | `registered (strict=0, grass=0, fl=0)` | 12 | 472.3 | 9/11/0/0 | 9/6/0/0 | 0..80/0..35/0..0/0..0 | `0.401..0.574` | `0/0/0/0/11/12/2/0/0` | `7/6/11/11/12/11` | 0/36845/398/35839 | `4.29e-08` | FAIL (M collapse; joint gate_pass=0) |
| `sel2p_b_strict` | A (`d90be0bd`) | `strict=1 (grass=0, fl=0)` | 12 | 546.6 | 11/11/0/0 | 11/6/0/0 | 0..80/0..35/0..0/0..0 | `0.360..0.516` | `0/0/0/0/11/12/2/0/0` | `7/6/8/11/12/11` | 0/36845/398/35839 | `4.29e-08` | FAIL (M collapse; joint gate_pass=0) |
| `sel2p_c_grass` | A (`d90be0bd`) | `grass=1 (strict=0, fl=0)` | 12 | 321.8 | 7/6/0/0 | 6/0/0/0 | 0..68/0..2/0..0/0..0 | `0.606..0.850` | `0/0/0/0/8/10/0/0/0` | `0/0/0/8/10/6` | 0/5991/118/5740 | `3.70e-08` | FAIL (M collapse; joint gate_pass=0) |
| `sel2p_d_both` | A (`d90be0bd`) | `strict=1, grass=1 (fl=0)` | 12 | 317.5 | 9/6/0/0 | 7/0/0/0 | 0..80/0..2/0..0/0..0 | `0.557..0.781` | `0/0/0/0/8/10/0/0/0` | `0/0/1/8/10/6` | 0/5991/118/5740 | `3.70e-08` | FAIL (M collapse; joint gate_pass=0) |
| `sel2p_e1_sym100` | A (`d90be0bd`) | `strict=1, grass=1, f=100, cap=100 (all)` | 12 | 319.3 | 8/4/0/0 | 5/0/0/0 | 0..75/0..3/0..0/0..0 | `0.577..0.961` | `0/0/0/0/0/6/0/0/0` | `0/0/1/3/6/4` | 0/5348/139/5256 | `3.65e-08` | FAIL (M collapse; joint gate_pass=0) |
| `sel2p_e2_sym160` | A (`d90be0bd`) | `strict=1, grass=1, f=160, cap=160 (all)` | 12 | 398.4 | 11/4/2/0 | 6/0/0/0 | 0..41/0..5/0..5/0..0 | `0.718..1.106` | `0/0/0/0/0/8/2/0/0` | `1/0/0/3/8/6` | 0/6252/127/6127 | `4.11e-08` | FAIL (M collapse; joint gate_pass=0) |
| `sel2p_f1_strict_sym100_nograss` | A (`d90be0bd`) | `strict=1, grass=0, f=100, cap=100 (all)` | 6 | 243.7 | 5/4/0/0 | 3/4/0/0 | 0..98/0..31/0..0/0..0 | `0.568..0.592` | `0/0/0/0/0/6/0/0/0` | `4/4/4/1/6/4` | 0/7171/228/6999 | `4.25e-08` | SCREEN (6s) / FAIL |
| `sel2p_f2_strict_sym160_nograss` | A (`d90be0bd`) | `strict=1, grass=0, f=160, cap=160 (all)` | 6 | 474.3 | 5/2/0/0 | 4/2/0/0 | 0..93/0..40/0..0/0..0 | `0.670..0.698` | `0/0/0/0/1/6/0/0/0` | `2/2/0/1/6/2` | 0/6157/140/6057 | `4.40e-08` | SCREEN (6s) / FAIL |
| `sel2p_f3_strict_s075_cap320` | A (`d90be0bd`) | `strict=1, grass=0, s=0.75, cap=320` | 6 | 214.0 | 5/5/0/0 | 5/4/0/0 | 0..80/0..52/0..0/0..0 | `0.354..0.572` | `0/0/0/0/6/6/1/0/0` | `5/4/4/6/6/5` | 0/23988/405/23587 | `7.06e-08` | SCREEN (6s) / FAIL |
| `sel2p_f4_strict_s063_k10k` | A (`d90be0bd`) | `strict=1, grass=0, s=0.63, K=10000` | 6 | 256.3 | 5/5/0/3 | 5/2/0/2 | 0..80/0..35/0..0/0..35 | `0.380..0.520` | `0/0/0/0/6/6/2/3/0` | `3/2/3/6/6/5` | 0/20/34/18 | `4.19e-08` | SCREEN (6s) / FAIL |
| `sel2p_f5_strict_s063_k2k` | A (`d90be0bd`) | `strict=1, grass=0, s=0.63, K=2000` | 6 | 277.8 | 5/5/0/5 | 5/2/0/2 | 0..80/0..35/0..0/0..35 | `0.380..0.520` | `0/0/0/0/6/6/2/5/0` | `3/2/3/6/6/5` | 0/0/0/0 | `4.19e-08` | SCREEN (6s) / FAIL |
| `sel2p_f6_strict_s100_cap80_mut0` | A (`d90be0bd`) | `strict=1, grass=0, s=1.0, cap=80 (all)` | 6 | 228.8 | 5/5/1/1 | 5/5/1/1 | 0..80/0..80/0..20/0..80 | `0.977..1.025` | `1/1/0/0/0/1/1/1/0` | `5/5/0/1/1/5` | 0/3738/316/3734 | `2.51e-08` | SCREEN (6s) / FAIL |
| `sel2p_b1_strict_s063_k0_fl1` | B (`4794b6fc`) | `strict=1, s=0.63, K=0, fl=1` | 6 | 1369.2 | 5/4/0/0 | 5/1/0/0 | 0..79/0..20/0..0/0..0 | `0.311..0.515` | `0/0/0/0/6/6/0/0/0` | `3/1/2/6/6/4` | 0/19020/236/18673 | `3.84e-08` | SCREEN (6s) / FAIL |
| `sel2p_b2_strict_s063_k0_fl2` | B (`4794b6fc`) | `strict=1, s=0.63, K=0, fl=2` | 6 | 821.1 | 5/4/0/0 | 4/3/0/0 | 0..80/0..16/0..0/0..0 | `0.327..0.598` | `0/0/0/0/6/6/0/0/0` | `4/3/3/6/6/4` | 0/11834/179/11517 | `4.72e-08` | SCREEN (6s) / FAIL |
| `sel2p_b1_strict_s063_k2k_fl1` | B (`4794b6fc`) | `strict=1, s=0.63, K=2000, fl=1` | 6 | 2480.9 | 5/4/0/4 | 5/1/0/1 | 0..79/0..20/0..0/0..20 | `0.311..0.515` | `0/0/0/0/6/6/0/4/0` | `3/1/2/6/6/4` | 0/0/0/0 | `3.90e-08` | SCREEN (6s) / FAIL |
| `sel2p_b2_strict_s063_k2k_fl2` | B (`4794b6fc`) | `strict=1, s=0.63, K=2000, fl=2` | 6 | 1261.2 | 5/4/0/4 | 4/3/0/3 | 0..80/0..16/0..0/0..16 | `0.327..0.598` | `0/0/0/0/6/6/0/4/0` | `4/3/3/6/6/4` | 0/0/1/0 | `4.94e-08` | SCREEN (6s) / FAIL |
| `sel2p_b1_strict_s063_k10k_fl1` | B (`4794b6fc`) | `strict=1, s=0.63, K=10000, fl=1` | 6 | 560.1 | 5/4/0/3 | 5/1/0/0 | 0..79/0..20/0..0/0..8 | `0.311..0.515` | `0/0/0/0/6/6/0/3/0` | `3/1/2/6/6/4` | 0/14/161/3 | `3.96e-08` | SCREEN (6s) / FAIL |
| `sel2p_b2_strict_s063_k10k_fl2` | B (`4794b6fc`) | `strict=1, s=0.63, K=10000, fl=2` | 6 | 336.6 | 5/4/0/3 | 4/3/0/2 | 0..80/0..16/0..0/0..16 | `0.327..0.598` | `0/0/0/0/6/6/0/3/0` | `4/3/3/6/6/4` | 0/98/62/49 | `6.53e-08` | SCREEN (6s) / FAIL |
| `sel2p_b1_strict_s075_k2k_fl1` | B (`4794b6fc`) | `strict=1, s=0.75, cap=320, K=2000, fl=1` | 6 | 460.5 | 5/5/0/5 | 5/4/0/4 | 0..79/0..43/0..0/0..43 | `0.351..0.612` | `0/0/0/0/6/6/0/5/0` | `5/4/3/6/6/5` | 0/0/1/0 | `7.31e-08` | SCREEN (6s) / FAIL |
| `sel2p_b2_strict_s075_k2k_fl2` | B (`4794b6fc`) | `strict=1, s=0.75, cap=320, K=2000, fl=2` | 6 | 423.8 | 5/6/1/6 | 4/4/0/4 | 0..80/7..44/0..3/7..44 | `0.383..0.574` | `0/0/0/0/6/6/1/6/0` | `6/4/5/6/6/6` | 0/0/0/0 | `7.23e-08` | SCREEN (6s) / FAIL |
| `sel2p_b1_strict_sym100_k2k_fl1` | B (`4794b6fc`) | `strict=1, f=100, cap=100, s=0.63, K=2000, fl=1` | 6 | 352.0 | 6/3/0/3 | 6/1/0/1 | 17..55/0..13/0..0/0..13 | `0.494..0.606` | `0/0/0/0/1/6/0/3/0` | `2/1/2/1/6/3` | 0/0/0/0 | `2.92e-08` | SCREEN (6s) / FAIL |
| `sel2p_b0_s063_k2k_fl0_12s` | B (`4794b6fc`) | `strict=1, s=0.63, K=2000, fl=0 (12s)` | 12 | 722.4 | 11/11/0/10 | 11/6/0/6 | 0..80/0..35/0..0/0..35 | `0.360..0.516` | `0/0/0/0/11/12/2/10/0` | `7/6/8/11/12/11` | 0/1/14/0 | `4.36e-08` | FAIL (M collapse; joint gate_pass=0) |
| `sel2p_b0_s075_k2k_fl0_12s` | B (`4794b6fc`) | `strict=1, s=0.75, cap=320, K=2000, fl=0 (12s)` | 12 | 584.1 | 11/11/0/11 | 11/10/0/10 | 0..80/0..52/0..0/0..52 | `0.352..0.578` | `0/0/0/0/12/12/1/11/0` | `11/10/10/12/12/11` | 0/0/2/0 | `7.10e-08` | FAIL (M collapse; joint gate_pass=0) |
| `sel2p_b1_s075_k2k_fl1_12s` | B (`4794b6fc`) | `strict=1, s=0.75, cap=320, K=2000, fl=1 (12s)` | 12 | 560.2 | 9/11/0/11 | 9/8/0/8 | 0..80/0..43/0..0/0..43 | `0.351..0.612` | `0/0/0/0/12/12/1/11/0` | `11/8/9/12/12/11` | 0/0/1/0 | `7.78e-08` | FAIL (M collapse; joint gate_pass=0) |
| `sel2p_b2_s075_k2k_fl2_12s` | B (`4794b6fc`) | `strict=1, s=0.75, cap=320, K=2000, fl=2 (12s)` | 12 | 5027.5 | 9/12/1/12 | 7/10/0/10 | 0..80/7..66/0..3/7..66 | `0.381..0.576` | `0/0/0/0/12/12/1/12/0` | `12/10/11/12/12/12` | 0/0/0/0 | `8.92e-08` | FAIL (M collapse; joint gate_pass=0) |

## 4. Ranked Candidate Short List (Gates & Survival Only — Blind to Body Mass)

### 4a. `E3b` Candidates

1. **`e3_b1_g4_fl1` (PASSES ALL CARD TARGETS)** — `U=11/12, S=12/12, T=12/12`; `min_N >= 10`: `U=11/12, S=11/12, T=11/12` (`all=11/12`); `WP-2=12/12` (`b1` shift `-0.549..-0.448`, `b2` mismatch `-0.0075..+0.0067`). Changes vs frozen E3: `live.strict_prey_cap=1`, `live.founders=160`, `live.prey_cap=160`, `live.lakes=16`, `live.lake_layout=1`, `live.dehydration_ticks=960`, `live.founder_layout=1` (`predator_fraction=0.20`, `grass_steering=0`). Does not change the scientific question (uniform vs gradient scarcity under trophic parity).
2. **`e3_b1_g5_fl1` (herbivore-only runner-up; fails `min_N >= 10` on `U` and `T`)** — `U=12/12, S=12/12, T=12/12`; `min_N >= 10`: `U=10/12, S=12/12, T=9/12` (`all=7/12`); `WP-2=12/12`. Changes vs frozen E3: sets `predator_fraction=0.0`, `founders=100, prey_cap=100, strict=1, lakes=16, lake_layout=1, dehydration_ticks=960, founder_layout=1`. Narrowing change: removes predator-prey trophic feedback.
3. **`e3_b1_g3_fl1` (keeps `founders=100`, predator parity; fails `min_N >= 10` target)** — `U=11/12, S=12/12, T=12/12`; `min_N >= 10`: `U=10/12, S=9/12, T=9/12` (`all=8/12`); `WP-2=12/12`. Changes vs frozen E3: `prey_cap=100`, `strict=1`, `lakes=16, lake_layout=1, dehydration_ticks=960, founder_layout=1`. Does not change the scientific question.

### 4b. `SEL2-P2` Candidate Directions (`0/25` cells passed joint `C1..C5`)

No tested cell meets `>= 11/12` across all four arms and `C1..C5` because `scarce_mut0` (`M`) fails in `10–12/12` seeds in every cell (`M=1/6` even at baseline `scale=1.0, cap=80` in `sel2p_f6_strict_s100_cap80_mut0`). Ranking the 12-seed Phase B confirmations strictly by survival and `selected`/`scarce_drift` gates:
1. **`sel2p_b0_s075_k2k_fl0_12s`** — Surv `B=11/12, S=11/12, M=0/12, D=11/12`; `min_N >= 10`: `11/10/0/10`; `drift_valid=11/12`; `selected`-only `C2=10/12, C3=12/12, C4=12/12, C5=11/12`; joint `gate_pass=0/12`. Changes vs frozen SEL2-P: `strict_prey_cap=1`, `resource_capacity_scale=0.75` (scarce arms), rolling `drift_reset_ticks=2000`. Changes scarcity factor (`0.63 -> 0.75`) and drift reset cadence.
2. **`sel2p_b2_s075_k2k_fl2_12s` (Option 1 completed cell)** — Surv `B=9/12, S=12/12, M=1/12, D=12/12`; `min_N >= 10`: `7/10/0/10`; `drift_valid=12/12`; `selected`-only `C2=11/12, C3=12/12, C4=12/12, C5=12/12`; `mis(cap/ext/ins/d)=0/0/0/0`; joint `gate_pass=0/12`. Changes vs #1: adds `founder_layout=2`, which lifts `S`/`D` to `12/12` and `selected` `C2` to `11/12` while `B` survival drops to `9/12`.
3. **`sel2p_b0_s063_k2k_fl0_12s` (preserves `0.63` scarcity)** — Surv `B=11/12, S=11/12, M=0/12, D=10/12`; `min_N >= 10`: `11/6/0/6`; `drift_valid=10/12`; `selected`-only `C2=8/12, C3=11/12, C4=12/12, C5=11/12`; joint `gate_pass=0/12`. Preserves `scale=0.63`; changes `strict_prey_cap=1` and rolling `drift_reset_ticks=2000`.

## 5. Artifacts & SHA-256 Index

- [summary.json](../../../artifacts/experiments/viab1-shared-viability/summary.json): SHA-256 `754740b61e83d8d4748077d3e562365a23b48de910fd388f706452cadc8f848b`
- [cells_summary.csv](../../../artifacts/experiments/viab1-shared-viability/cells_summary.csv): SHA-256 `2f48f442bb79d697e4307f13443b9c5945d241a6094afe2fa5c905334f07b3b7`
- [checksums.sha256](../../../artifacts/experiments/viab1-shared-viability/checksums.sha256): `57` verified SHA-256 entries covering all `55` raw cell JSONs (`cells/*.json`), `summary.json`, and `cells_summary.csv`.

## 6. What This Result Does NOT Show

- **No body-mass or evolutionary trait inference**: VIAB-1 was strictly blind to `live.trait_mass_*` (`0` hits across all outputs); nothing here tests whether any candidate produces or suppresses a body-mass difference.
- **Not preregistered confirmation evidence**: all runs used exploratory seeds `880001..=880012`; `E3b` and `SEL2-P2` require new preregistrations with fresh disjoint seeds.
- **6-seed screening cells are low-power**: `sel2p_f1..f6` and the `9` Stage B1 cells used only `6` seeds (`880001..=880006`) and are not equivalent to `12`-seed confirmation runs.
- **`scarce_mut0` remains unsolved in `SEL2-P2`**: `0/25` SEL2-P cells pass the joint `C1..C5` preregistered gate because `scarce_mut0` fails across all tested variants, including baseline abundance (`f6`).
- **Pre-PR `#200` / `#207` commits on a single OS/world**: Phase A ran at `d90be0bd` and Phase B at `4794b6fc` on `Windows 1 x86_64` with `Generated` terrain (`WorldIdentity 3152323152`), before PR `#200` (`F-055..F-065`, `F053-FIX`) and PR `#207` (`F-066`), and `QD-VIAB1C` holds only because `0/55` cells set `live.land_mask=1`. Cross-OS trajectories and desktop-app multi-threaded execution are not tested.
- **Probe source pending repo path authorization**: `viab1_probe` is stored on the Hub at `test_runs/anima-case_VIAB-1/viab1_probe/` (`BLOCKED VIAB-1 probe path`) rather than committed in this repository.
