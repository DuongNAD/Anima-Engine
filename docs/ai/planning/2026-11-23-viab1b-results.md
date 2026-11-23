---
phase: planning
feature: ecological-selection
kind: đo
title: Card VIAB-1b — Blind Viability & mut0 Survival Check for the 2-Segment Founder (E3b)
description: Body-mass-blind calibration cell e3b_fg1_g4_fl1 and zero-mutation survival check e3b_fg1_g4_fl1_mut0 (72 runs across seeds 880001..=880012 at merged trunk 98054ba), verifying 12/12 survival, WP-2 gates, predator persistence, and generation counts without reading trait_mass
status: completed
owner: anima-case
last_reviewed: 2026-11-23
---

# Card `VIAB-1b` — Blind Viability & `mut0` Survival Check for the 2-Segment Founder (`E3b`)

## 1. Provenance, Trunk Pin, Blindness & Pre-Run Criterion

- **Host:** `Windows 1` (`DESKTOP-3EPV830`, `windows/x86_64`); release binary `E:\.smart_drive_caches\cargo-target\release\viab1b_probe.exe`; desktop app never launched.
- **Merged trunk commit (`PR #224` `FOUNDER-2SEG` + `PR #225` `cookie` fix):** **`98054ba206d275014ecce89e7ab7c2ad603ead1d`** (recorded inside all `VIAB-1b` artifacts and verified clean before compilation and launch).
- **Pre-run host & world identity:** `ANIMA_WORLD_ARTIFACT` unset and `%TEMP%\anima_world_artifact.anmw` absent prior to execution; `all_terrain_generated = true` in **`2/2`** cell JSONs and [summary.json](../../../artifacts/experiments/viab1b-founder-2seg/summary.json) (`WorldIdentity(seed=1337, generator_version=1, width=256, height=256, checksum=3152323152)` across all `72` runs, informational only and never pinned).
- **Exploratory seeds:** `880001..=880012` (`12` seeds × `3` arms = `36` runs per cell, `72` runs total, `18,000` ticks); no confirmation seed (`>= 880013`) was executed.
- **Body-mass blindness (`QD-VIAB`):** `viab1b_probe` restricts `observable_ids` strictly to the `12` non-mass observables (`plants`, `detritus`, `live.closed_eu_total`, `live.agent_count`, `live.herbivore_count`, `live.predator_count`, `live.standing_crop`, `live.births`, `live.deaths`, `live.mean_lifespan`, `live.deaths_starvation`, `live.deaths_dehydration`) and aborts on any `"trait_mass"` substring during serialization. Programmatic audit across all `4` committed data outputs: **`0` case-insensitive `trait_mass` matches**.
- **Probe provenance (built cleanly outside the repository at `D:\teamwork_projects\anima-viab1b-probe`):**
  - `Cargo.toml` SHA-256: `BD01C27DD95108D289D271F5EEC9E2BC2D957D0653898E6F6517AC83BD4257F0`
  - `Cargo.lock` SHA-256: `BACB3866B558473E3C6F9FF2692DCED1FD59583108ABF97FD95D00A01ED9638A`
  - `src/main.rs` SHA-256: `C3D6E95B25411E6FFF0F5F5EBEEBCA1EB685C42E89724BED90657B3FB977A6A7`
  - `E:\.smart_drive_caches\cargo-target\release\viab1b_probe.exe` pre-run & post-run SHA-256 (unchanged): `6C0295AD42BBDC7AA8865494A0F461E4FAA2E1CBD3FC3AB2DE28824AF281FE8F`
- **Pre-run criterion (recorded at `2026-11-23T15:16:30Z` BEFORE build/launch in `test_runs/anima-case_VIAB-1b_prerun_criterion.md`):**
  - **`e3b_fg1_g4_fl1` (`live.founder_genotype = 1`, `live.mutation_rate = 0.05`):** **`PASS`** iff arm survival `>= 11/12` in each arm (`U`, `S`, `T`, and `All`), `min_N >= 10` in `>= 11/12` seeds per arm, `WP-2` (`b1/b2/c/all`) `= 12/12`, `all_terrain_generated == true`, `max_abs_eu_drift < 1e-6`, and `0` `trait_mass` matches.
  - **`e3b_fg1_g4_fl1_mut0` (survival-only `mut0` check, exact same overrides + `live.mutation_rate = 0.0`):** **`PASS`** iff **`>= 11/12` seeds survive (`survived == true`, `final_n > 0` at `T = 18,000`) in EACH of the three arms `U`, `S`, and `T` (`U >= 11/12`, `S >= 11/12`, `T >= 11/12`)**, `all_terrain_generated == true`, `max_abs_eu_drift < 1e-6`, and `0` `trait_mass` matches; **`FAIL`** if any arm falls below `11/12`.

---

## 2. Summary Results & Comparison vs. 3-Segment `e3_b1_g4_fl1` (`VIAB-1`)

Base `e3_b1_g4_fl1` overrides across both cells (`live.predator_fraction = 0.3` from registered E3): `live.strict_prey_cap = 1`, `live.founders = 160`, `live.prey_cap = 160`, `live.lakes = 16`, `live.lake_layout = 1`, `live.dehydration_ticks = 960`, `live.founder_layout = 1`, plus **`live.founder_genotype = 1`** ([2026-11-23-founder-2seg.md](2026-11-23-founder-2seg.md)).

| Cell ID | Trunk SHA | Config Delta vs. `e3_b1_g4_fl1` | Wall (s) | Surv (`U/S/T/All`) | `min_N >= 10` (`U/S/T/All`) | `pred_final > 0` (`U/S/T`) | `min_N` Range (`U` / `S` / `T`) | `mean_N` (`U` / `S` / `T`) | `final_N` (`U` / `S` / `T`) | Mean Births (`U` / `S` / `T`) | Mean Generations (`births / mean_n`, `U` / `S` / `T`) | `WP-2` (`b1/b2/c/all`) | `b1` Shift (`T` vs `U`) | `b2` Mismatch (`T` vs `S`) | Starv % (`U/S/T`) | Max `|EU drift|` | Criterion Verdict |
|---|---|---|---:|---|---|---|---|---|---|---|---|---|---|---|---|---:|---|
| *Ref (`VIAB-1`)* `e3_b1_g4_fl1` | `4794b6fc` | `founder_genotype=0` (3-seg, `mut=0.05`) | 70.0 | 11/12/12/11 | 11/11/11/11 | 0/0/0 | 0..74 / 7..65 / 2..115 | 100.4 / 79.8 / 99.5 | — | — | `~4.0 / 1.9 / 4.0` | 12/12/12/12 | `-0.549..-0.448` | `-0.0075..+0.0067` | 75%/38%/47% | `2.99e-08` | **PASS** ([`VIAB-1`](2026-11-23-viab1-results.md)) |
| **`e3b_fg1_g4_fl1`** | `98054ba2` | **`founder_genotype=1`** (2-seg, `mut=0.05`) | **82.4** | **12/12/12/12** | **12/12/12/12** | **12/11/12** | `91..169` / `58..166` / `131..168` | **178.78 / 167.10 / 178.43** | **150.17 / 135.83 / 165.67** | **552.83 / 355.83 / 542.58** | **3.091 / 2.111 / 3.041** | **12/12/12/12** | `-0.4916..-0.4585` | `-0.0037..+0.0070` | 59.2%/18.2%/33.6% | `7.72e-08` | **PASS** |
| **`e3b_fg1_g4_fl1_mut0`** | `98054ba2` | **`founder_genotype=1` + `mutation_rate=0`** | **86.5** | **12/12/12/12** | **12/12/12/12** | **12/11/12** | `130..171` / `108..165` / `158..168` | **180.20 / 171.91 / 179.03** | **156.50 / 149.67 / 166.08** | **571.17 / 375.67 / 535.58** | **3.170 / 2.178 / 2.993** | **12/12/12/12** | `-0.4901..-0.4565` | `-0.0017..+0.0081` | 59.5%/17.3%/33.5% | `6.17e-08` | **PASS** |

### Generation & Ecological Observations (`880001..=880012`)

1. **`mut0` collapse hypothesis confirmed & resolved (`PR #216` follow-up):** Under the 3-segment founder (`mass = 3.3`), `mut0` collapsed (`0..1/12` survival across all `VIAB-1` `SEL2-P` checks; [2026-11-23-mut0-collapse-diagnosis.md](2026-11-23-mut0-collapse-diagnosis.md)). Starting directly at the declared 2-segment founder (`live.founder_genotype = 1`, `mass = 2.5`) yields **`12/12` survival in `U`, `12/12` in `S`, and `12/12` in `T`** (`min_N >= 58` across all `36` runs of `e3b_fg1_g4_fl1`, and **`min_N >= 108`** across all `36` runs of `e3b_fg1_g4_fl1_mut0`). Both cells clear the pre-run `>= 11/12` criterion (**PASS**).
2. **Realized generations (`births / mean_n` across `18,000` ticks):**
   - `e3b_fg1_g4_fl1`: **`U`: mean `3.091`** (median `3.091`, range `2.784..3.517`), **`S`: mean `2.111`** (median `1.981`, range `1.651..2.648`), **`T`: mean `3.041`** (median `2.821`, range `2.759..3.681`).
   - `e3b_fg1_g4_fl1_mut0`: **`U`: mean `3.170`** (median `3.050`, range `2.764..3.588`), **`S`: mean `2.178`** (median `2.003`, range `1.885..2.711`), **`T`: mean `2.993`** (median `2.881`, range `2.659..3.515`).
   - Note the slight shift vs. `e3_b1_g4_fl1` (`~4.0 / 1.9 / 4.0`): higher sustained `mean_N` (`~167–180` vs. `~80–100`) increases denominator `mean_N`, bringing `U` and `T` to `~3.0–3.2` generations and `S` to `~2.1–2.2` generations (`S` rises from `1.9` to `2.1`).
3. **Important ecological shift in `predator_final` (`U/S/T = 12/11/12`):** Unlike 3-segment `e3_b1_g4_fl1` where predators went extinct in `12/12` seeds in every arm (`pred_final>0 = 0/0/0`), under `live.founder_genotype = 1` **predators persist to `T = 18,000` in `12/12` (`U`), `11/12` (`S`), and `12/12` (`T`)** in BOTH `e3b_fg1_g4_fl1` and `e3b_fg1_g4_fl1_mut0` (at final counts of `1..11` predators; only `S` in seed `880004` of `e3b_fg1_g4_fl1` and `S` in seed `880009` of `e3b_fg1_g4_fl1_mut0` dropped to `0`). **E3b (`D1` disclosure) cannot claim predators go extinct in `12/12` seeds under `live.founder_genotype = 1`**, because trophic persistence holds in `>= 11/12` seeds per arm!

---

## 3. Per-Seed Metrics (`880001..=880012`)

### Table 3.1 — Primary Cell `e3b_fg1_g4_fl1` (`live.founder_genotype = 1`, `mutation_rate = 0.05`)

| Seed | Surv (`U/S/T`) | `min_N` (`U/S/T`) | `final_N` (`U/S/T`) | `pred_final` (`U/S/T`) | `total_births` (`U/S/T`) | `generations = births / mean_n` (`U/S/T`) | `WP-2` (`b1`/`b2`/`c`/`all`) |
|---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| `880001` | 1/1/1 | 141/112/163 | 141/115/163 | 8/2/3 | 531/300/493 | 2.941 / 1.863 / 2.759 | 1/1/1/1 |
| `880002` | 1/1/1 | 145/63/168 | 155/67/168 | 9/1/8 | 554/240/506 | 3.078 / 1.651 / 2.798 | 1/1/1/1 |
| `880003` | 1/1/1 | 168/166/168 | 168/168/168 | 8/8/8 | 617/453/637 | 3.435 / 2.544 / 3.573 | 1/1/1/1 |
| `880004` | 1/1/1 | 168/160/164 | 168/160/164 | 8/0/4 | 627/446/654 | 3.517 / 2.560 / 3.681 | 1/1/1/1 |
| `880005` | 1/1/1 | 126/136/165 | 126/156/166 | 11/2/6 | 515/336/503 | 2.890 / 1.957 / 2.818 | 1/1/1/1 |
| `880006` | 1/1/1 | 169/120/166 | 169/152/166 | 9/3/6 | 569/371/529 | 3.114 / 2.226 / 2.955 | 1/1/1/1 |
| `880007` | 1/1/1 | 168/165/168 | 168/165/168 | 8/5/8 | 615/471/616 | 3.459 / 2.648 / 3.463 | 1/1/1/1 |
| `880008` | 1/1/1 | 164/58/165 | 164/72/165 | 11/3/5 | 571/243/503 | 3.103 / 1.714 / 2.796 | 1/1/1/1 |
| `880009` | 1/1/1 | 117/128/131 | 117/162/166 | 4/2/6 | 492/344/487 | 2.784 / 1.980 / 2.825 | 1/1/1/1 |
| `880010` | 1/1/1 | 91/113/165 | 94/113/165 | 8/1/5 | 465/297/503 | 2.809 / 1.829 / 2.786 | 1/1/1/1 |
| `880011` | 1/1/1 | 169/162/168 | 169/162/168 | 9/2/8 | 564/423/590 | 3.110 / 2.380 / 3.278 | 1/1/1/1 |
| `880012` | 1/1/1 | 163/138/161 | 163/138/161 | 3/1/1 | 514/346/490 | 2.847 / 1.981 / 2.761 | 1/1/1/1 |

### Table 3.2 — Survival-Only `mut0` Check `e3b_fg1_g4_fl1_mut0` (`live.founder_genotype = 1`, `mutation_rate = 0.0`)

| Seed | Surv (`U/S/T`) | `min_N` (`U/S/T`) | `final_N` (`U/S/T`) | `pred_final` (`U/S/T`) | `total_births` (`U/S/T`) | `generations = births / mean_n` (`U/S/T`) | `WP-2` (`b1`/`b2`/`c`/`all`) |
|---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| `880001` | 1/1/1 | 136/136/165 | 136/153/165 | 8/4/5 | 545/345/536 | 3.018 / 2.007 / 2.952 | 1/1/1/1 |
| `880002` | 1/1/1 | 130/133/167 | 130/152/168 | 9/1/8 | 502/333/492 | 2.764 / 1.942 / 2.719 | 1/1/1/1 |
| `880003` | 1/1/1 | 171/164/162 | 171/167/162 | 11/7/2 | 647/455/617 | 3.569 / 2.566 / 3.489 | 1/1/1/1 |
| `880004` | 1/1/1 | 168/164/168 | 168/166/168 | 8/6/8 | 602/480/615 | 3.373 / 2.711 / 3.453 | 1/1/1/1 |
| `880005` | 1/1/1 | 160/109/158 | 169/157/161 | 9/5/1 | 549/324/491 | 3.012 / 1.975 / 2.754 | 1/1/1/1 |
| `880006` | 1/1/1 | 166/161/162 | 166/161/163 | 8/1/3 | 607/393/518 | 3.352 / 2.226 / 2.922 | 1/1/1/1 |
| `880007` | 1/1/1 | 168/165/168 | 168/165/168 | 8/5/8 | 640/459/625 | 3.588 / 2.573 / 3.515 | 1/1/1/1 |
| `880008` | 1/1/1 | 131/108/162 | 131/129/168 | 8/1/8 | 520/306/475 | 2.997 / 1.918 / 2.659 | 1/1/1/1 |
| `880009` | 1/1/1 | 139/128/167 | 139/132/168 | 8/0/8 | 557/344/485 | 3.073 / 1.998 / 2.695 | 1/1/1/1 |
| `880010` | 1/1/1 | 160/110/168 | 164/114/168 | 4/2/8 | 518/309/503 | 2.869 / 1.885 / 2.779 | 1/1/1/1 |
| `880011` | 1/1/1 | 168/161/168 | 168/161/168 | 8/1/8 | 616/414/562 | 3.395 / 2.343 / 3.141 | 1/1/1/1 |
| `880012` | 1/1/1 | 168/139/162 | 168/139/166 | 8/2/6 | 551/346/508 | 3.027 / 1.990 / 2.839 | 1/1/1/1 |

---

## 4. Artifacts & SHA-256 Index

- [cells/e3b_fg1_g4_fl1.json](../../../artifacts/experiments/viab1b-founder-2seg/cells/e3b_fg1_g4_fl1.json) (`60,849` B): SHA-256 `88faa7041ab9162571fc2422e403f48e5d4f1d289a7f63554a4986219bda0d6c`
- [cells/e3b_fg1_g4_fl1_mut0.json](../../../artifacts/experiments/viab1b-founder-2seg/cells/e3b_fg1_g4_fl1_mut0.json) (`60,935` B): SHA-256 `ceac0f96e810f2d917e828ceb47bd71c109b7efccbeca9bdb81d16e76a79d2dc`
- [cells_summary.csv](../../../artifacts/experiments/viab1b-founder-2seg/cells_summary.csv) (`531` B): SHA-256 `2878e2b6b837854710412d6e9207707e7a5a1eadb1d81595b140da64f66fa5bf`
- [summary.json](../../../artifacts/experiments/viab1b-founder-2seg/summary.json) (`1,818` B): SHA-256 `86e7893052841488bf2c592a594b60b72e60120f000dde8bc2f9eeef55042b5e`
- [checksums.sha256](../../../artifacts/experiments/viab1b-founder-2seg/checksums.sha256): `4` verified SHA-256 entries matching every artifact byte.

---

## 5. What This Result Does NOT Show

- **No body-mass or evolutionary trait inference:** `VIAB-1b` remained 100% blind to `live.trait_mass_*` (`0` occurrences across all outputs); nothing here tests whether `T` vs. `S` changes body-mass variance `L_i` or mean.
- **Not preregistered E3b confirmation evidence:** All `72` runs used exploratory calibration seeds `880001..=880012` (not the disjoint E3b confirmation seeds).
- **200k-tick `SEL2-P2` persistence not tested here:** `VIAB-1b` ran `18,000` ticks (E3b horizon), not `200,000` ticks (`SEL2-P` horizon).
- **Single OS & `Generated` terrain only:** Executed on `Windows 1 x86_64` with `Generated` terrain (`WorldIdentity 3152323152`) at trunk `98054ba2`, without `live.land_mask = 1` or desktop multi-threaded runtime execution.
