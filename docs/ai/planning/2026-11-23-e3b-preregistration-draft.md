---
phase: planning
feature: experiment-e3b-productivity-gradient
title: Draft Preregistration — E3b: Spatial Productivity Gradient and Body-Mass Dispersion (Post-VIAB-1 Redesign)
description: Documents-only draft preregistration for E3b based on VIAB-1 cell e3_b1_g4_fl1, specifying world overrides, predator_fraction and founder-pool trade-offs, 3-arm contrasts, template rules R1-R10, pre-freeze Monte Carlo power analysis (R6b + Appendix A), WP-2/smoke gates, and Windows 1 run budget
status: proposed
owner: anima-case
last_reviewed: 2026-11-23
---

# Draft Preregistration — E3b: Spatial Productivity Gradient and Body-Mass Dispersion

> **Status: `proposed` (documents only — awaiting Owner approval of `E3B-D1`..`E3B-D5`).**
> - **No code, manifests, fixtures, or scripts are changed by this draft.**
> - **Experimental seeds executed:** `0` (`810001..` and smoke seed `999953` have never been run).
> - **Body-mass blindness (`QD-VIAB`):** `anima-case` measured, read, or used `0` body-mass numbers (`live.trait_mass_*`) from [E3-X confirmation results](2026-11-20-e3-x-confirmation-results.md) or VIAB-1. The clonal-founder (`3.3`) and 2-segment adapted (`~2.5`) mass facts in §1.3 are cited strictly from the lead's diagnosis note (`PR #216`, `docs/ai/planning/2026-11-23-mut0-collapse-diagnosis.md`).

## 0. Provenance & Required Inputs

| Role | Artifact / Reference | Revision | Body-Mass Values Used |
|---|---|---|---|
| Calibration basis | VIAB-1 result note (`docs/ai/planning/2026-11-23-viab1-results.md`, PR `#215`) & cell `artifacts/experiments/viab1-shared-viability/cells/e3_b1_g4_fl1.json` | Phase B `4794b6fc` / PR `#215` `d49cf4ff` | None (`0` `trait_mass` matches) |
| Frozen predecessor | [Frozen E3 Plan](2026-08-26-experiment-e3-productivity-gradient-body-mass.md) & [e3-preregistration.json](../../../src-tauri/tests/fixtures/experiments_e3/e3-preregistration.json) | Trunk `9958f27` | N/A (design spec only) |
| Failure diagnosis | [E3-X Results](2026-11-20-e3-x-confirmation-results.md) (`insufficient_evidence`, `n = 6 < 10` due to `6/12` scarcity extinctions) | Trunk `24c6fb1c` | **None excluded/consulted** (`S`, `T`, `U` survival counts & wall timing only) |
| Founder diagnosis | Lead `mut0` collapse diagnosis (`docs/ai/planning/2026-11-23-mut0-collapse-diagnosis.md`, PR `#216` on `lead/mut0-diagnosis`) | `9062a7c3` | Cited mass facts only (`3.3` clonal founder & `~2.5` 2-segment fix; never measured by `anima-case`) |
| Statistics template | Lead statistics template (`docs/ai/planning/2026-11-23-prereg-statistics-template.md`, PR `#214`) & [Statistics Research §3.3–3.5](../../research/VIABILITY_PRICE_STATISTICS_RESEARCH_2026-11-22.md) | PR `#214` `3fee64d5` | None |

---

## 1. World Configuration (`e3_b1_g4_fl1`) & Predator Extinction Disclosure

### 1.1. Base World & Differences vs. Frozen E3

E3b builds on `core::live_experiment::LiveExperimentAdapter` (`LIVE_MODEL_VERSION = "live-bevy-world/2"`, snapshot schema `8`) with `MapSettings::default()` (`seed = 1337`, `256×256` `Generated` terrain on `Windows 1 x86_64`, observed `WorldIdentity` checksum `3152323152`, informational and never hard-pinned across OS/architectures).

Relative to frozen E3 (`9958f27`), VIAB-1 cell **`e3_b1_g4_fl1`** was the **`1/30`** E3-family configuration that passed every VIAB-1 target (`U=11/12, S=12/12, T=12/12` survival; `min_N >= 10` in `11/12` in all three arms, `all = 11/12`; `WP-2 = 12/12`). It introduces **exact overrides on 7 `LIVE_KEYS`**:

| Key | Frozen E3 | E3b (`e3_b1_g4_fl1`) | Role of Change |
|---|---|---|---|
| `live.strict_prey_cap` | `0.0` (absent) | **`1.0`** | Enforces hard `prey_cap` birth ceiling (eliminates uncontrolled boom–bust spikes) |
| `live.founders` | `120.0` | **`160.0`** | Raises founder count and matches initial population to `prey_cap` (`symmetric 160/160`) |
| `live.prey_cap` | `80.0` (default) | **`160.0`** | Matches equilibrium herbivore cap to founders under `strict_prey_cap = 1` |
| `live.lakes` | `2.0` | **`16.0`** | Increases water source count to decouple dehydration crashes from food scarcity |
| `live.lake_layout` | `0.0` (absent) | **`1.0`** | Distributes all `16` lakes on a 2D spatial grid rather than one row |
| `live.dehydration_ticks` | `120.0` (default) | **`960.0`** | Gives surviving agents sufficient transit budget between 2D lakes |
| `live.founder_layout` | `0.0` (absent) | **`1.0`** | Spreads founders on a 2D land-only grid (never stacked on ocean tiles) |

All **29 `LIVE_KEYS`** on trunk `e576350` are pinned as follows:

- **Shared explicit keys across all 3 arms (20 keys including `resource_capacity_scale`):** `live.ecological_fitness = 1.0`, `live.mortality = 1.0`, `live.reproduction = 1.0`, `live.locomotion = 1.0`, `live.drive_floor = 0.5`, `live.thirst_steering = 1.0`, `live.evolved_brains = 0.0`, `live.trees = 8.0`, `live.food_cap = 50.0`, plus the 7 `e3_b1_g4_fl1` overrides (`live.strict_prey_cap = 1.0`, `live.founders = 160.0`, `live.prey_cap = 160.0`, `live.lakes = 16.0`, `live.lake_layout = 1.0`, `live.dehydration_ticks = 960.0`, `live.founder_layout = 1.0`), per-arm `live.resource_capacity_scale` (`1.0` in `U` & `T`, `0.42` in `S`), and **`live.predator_fraction`** (`OWNER DECISION — E3B-D1`, below). *(Plus optional explicit zero or left default: `live.grass_steering = 0.0`.)*
- **Remaining `LIVE_KEYS` left at `LiveWorldConfig::default()` (`0` / absent):** `live.w_surplus = 1.0`, `live.starve_ticks = 120`, `live.mutation_rate = 0.05`, `live.plant_reserve = 0.0`, `live.kill_hydration = 0.0`, `live.kill_efficiency = 0.40`, `live.target_min_energy = 0.0`, `live.drift_control = 0.0`, `live.drift_reset_ticks = 0`, `live.land_mask = 0` (`false`), and `live.grass_steering = 0` (`false`). Note that `grass_steering` and `land_mask` remain off (`0` combinations of both keys, matching `QD-VIAB1C`).
- **Effective `live.predator_fraction` in VIAB-1 (`0.3` vs `"0.20"` typo):** Neither `e3_b1_g4_fl1.json` nor `e3_b1_g3_fl1.json` overrides `live.predator_fraction`, so **`0.3` is the effective value inherited from frozen E3**; the `"0.20"` parenthetical in the VIAB-1 result note (`PR #215` §4a, including the `e3_b1_g3_fl1` line) is a prose typo that the lead will fix later without blocking `E3B-P`.

### 1.2. Plain Predator Extinction Statement & `OWNER DECISION — E3B-D1`

> **Plain empirical fact (`e3_b1_g4_fl1` and frozen E3):** Predators go extinct in **`12/12` seeds of every arm** (`pred_final > 0` is **`U = 0/12`, `S = 0/12`, `T = 0/12`** in both `e3_b1_g4_fl1` and registered `e3_a_reg`). Consequently, **E3b is effectively herbivore-only by the end of the run (`T = 18,000`), just as frozen E3 was.**
>
> **Base-manifest audit note (`0.3` effective value):** Frozen E3 manifests (`e3-control-uniform.json`, `e3-preregistration.json`) and `LiveWorldConfig::default()` set **`live.predator_fraction = 0.3`** (`round(160 × 0.3) = 48` initial predators, `112` initial herbivores in `e3_b1_g4_fl1`), and neither `e3_b1_g4_fl1.json` nor `e3_b1_g3_fl1.json` overrides `live.predator_fraction`. Therefore `live.predator_fraction` requires an explicit decision before manifests are generated:

| Option (`OWNER DECISION — E3B-D1`) | `live.predator_fraction` | Initial Split (`founders=160`) | Trade-Off & Evidence Basis |
|---|---|---|---|
| **Option 1A — Keep `0.3` (exact `e3_b1_g4_fl1` setting)** | `0.3` | `48` predators / `112` herbivores | **Pros:** Exactly matches calibrated VIAB-1 cell `e3_b1_g4_fl1` (`U=11/12, S=12/12, T=12/12`, `min_N>=10: 11/11/11`, `WP-2: 12/12`); zero extra calibration needed; matches frozen E3 initial condition. **Cons:** Predators still die out in `12/12` seeds in every arm (`0/0/0`), leaving an unviable early predator transient and `live.shannon = 0` after predator extinction; claims must state *"initialized with `predator_fraction = 0.3`, effectively herbivore-only by `T = 18,000`"*. |
| **Option 1B — Set `0.0` (explicitly herbivore-only)** | `0.0` | `0` predators / `160` herbivores | **Pros:** Aligns initial conditions honestly with the realized herbivore-only ecology at `T = 18,000`, removes transient early predation noise and guarantees `0` trophic ambiguity. **Cons:** The exact tuple `(founders=160, prey_cap=160, fl=1, pred=0.0)` was **not** tested in VIAB-1 (`e3_b1_g5_fl1` used `100/100` with `fl=1, pred=0` -> `12/12/12` survival, `min_N>=10: 10/12/9`; `e3_g6` used `160/160` with `fl=0, pred=0` -> `10/12/12` survival, `min_N>=10: 9/9/9`). Starting `160` vs `112` initial herbivores may alter early starvation dynamics, so selecting Option 1B requires running **1 blind VIAB-1 calibration cell** (`880001..=880012`, `36` runs, ~`70 s`) before locking R3/R5/R6. |
| **Option 1C — Set `0.20`** *(not recommended)* | `0.20` | `32` predators / `128` herbivores | Uncalibrated in VIAB-1 (`32/128`), neither matches `e3_b1_g4_fl1` (`0.3`) nor removes extinct predators (`0.0`); offers no advantage over 1A or 1B. |

### 1.3. Founder Pool (`PR #216` Diagnosis & `OWNER DECISION — E3B-D5`)

In `PR #216` (`docs/ai/planning/2026-11-23-mut0-collapse-diagnosis.md` on `lead/mut0-diagnosis` @ `9062a7c3`), the lead diagnosed why the zero-mutation control collapsed in VIAB-1 even without scarcity (`sel2p_f6_strict_s100_cap80_mut0`). **It is not a code defect:**

- Every founder created by `live_experiment.rs::genesis` is the exact same hard-coded **3-segment clonal genotype** (segment masses `1.5 + 1.0 + 0.8 = 3.3` total mass, as cited in `PR #216`), which is only marginally viable.
- With mutation enabled (`live.mutation_rate = 0.05`), populations quickly evolve and fix a **2-segment body (mass `~2.5`, cited from `PR #216`) even at `resource_capacity_scale = 1.0`, with no scarcity**.

Because every E3b arm starts from that same clonal genotype unless changed, E3b carries two structural properties that must be stated upfront:

1. **Shared non-scarcity mass-reduction trend:** Every arm (`U`, `S`, `T`) undergoes a built-in evolutionary mass reduction (`3.3 -> ~2.5`) driven by the maladapted genesis clone rather than by scarcity or the productivity gradient.
2. **Zero initial variance (`trait_mass_var = 0.0` at tick `0`):** Every founder in every arm is genetically identical, so all initial dispersion is zero and all post-genesis trait divergence depends on *de novo* mutations occurring during the run.

The seed-paired contrasts (`T - S`, `T - U`, `S - U`) remain **internally valid** because all three arms start identically and share the same mutation law; however, **biological interpretation must state this explicitly** and treat the abundant uniform baseline (`U`) as measuring that background adaptation trend away from `3.3`.

| Option (`OWNER DECISION — E3B-D5`) | Founder Pool at Genesis | Effect on Comparability with Frozen E3 | Pros & Cons / Calibration Impact |
|---|---|---|---|
| **Option 5A — Keep the clonal 3-segment founder (`1.5 + 1.0 + 0.8 = 3.3`, as E3 did)** | Current `live_experiment.rs::genesis` clone | **Direct comparability with frozen E3:** identical initial genotype, identical `LIVE_KEYS`, zero harness/code changes needed. | **Pros:** Immediately matches VIAB-1 cell `e3_b1_g4_fl1` (`0` extra VIAB-1 calibration runs if paired with Option 1A). **Cons:** Every arm starts at `trait_mass_var = 0.0` and carries a non-scarcity `3.3 -> ~2.5` reduction trend that reports must explicitly disclose; variation cannot respond until *de novo* mutations arise. |
| **Option 5B — A declared adapted founder genotype (e.g., the 2-segment body, mass `~2.5`)** | Uniform adapted 2-segment clone | **Breaks exact genesis comparability with frozen E3:** changes baseline morphology at tick `0`. | **Pros:** Eliminates the shared `3.3 -> ~2.5` non-scarcity sweep and removes marginal viability fragility near genesis. **Cons:** `trait_mass_var` still starts at `0.0`; no `LIVE_KEYS` opt-in key currently selects an adapted founder genotype in `LiveWorldConfig` (requires an `ADR-0008` opt-in code card first) plus a fresh blind VIAB-1 calibration cell before locking R3/R5/R6. |
| **Option 5C — A declared founder pool with standing genetic variation** | Diverse pre-declared initial genotype mix (`trait_mass_var > 0` at tick `0`) | **Breaks genesis comparability with frozen E3:** replaces the clonal starting state with standing variation. | **Pros:** Ecological selection can sort standing body-mass variation from tick `0` rather than waiting on *de novo* mutation rates, cleanly decoupling ecological sorting from mutational rescue. **Cons:** No `LIVE_KEYS` founder-pool mechanism exists yet in `src-tauri/` (requires a deterministic opt-in schema/code card under `ADR-0008` + new VIAB-1 viability calibration before E3b can freeze). |

---

## 2. Arms, Contrasts & Metric Scope

### 2.1. Three Matched Arms (Unchanged Mechanism from Frozen E3)

E3b retains E3's 3-arm design and intervention structure (preserving identical code paths via explicit zero-intensity commands in `U` and `S`):

| Item | `control_uniform` (`U`) | `control_uniform_scarcity` (`S`) | `treatment_gradient` (`T`) |
|---|---|---|---|
| `live.resource_capacity_scale` | `1.0` (`153,587.25 EU` initial plants) | **`0.42`** (`64,506.64 EU` plants + `89,080.61 EU` detritus) | `1.0` (`153,587.25 EU` initial plants) |
| Intervention 1 (`id=1, Deforest`, West `0..=127`) | `intensity = 0.0` | `intensity = 0.0` | **`intensity = 0.30`** |
| Intervention 2 (`id=2, AddNutrient`) | `Rect{128,0,255,255}`, `intensity = 0.0` | **`Rect{0,0,255,255}` (whole map), `intensity = 1.00`** | **`Rect{128,0,255,255}` (East), `intensity = 1.00`** |
| Schedule (`start_tick / duration / curve`) | `1 / 18000 / Step` | `1 / 18000 / Step` | `1 / 18000 / Step` |

The frozen E3 mechanistic limitation remains explicitly declared in advance: `T` holds the West down via recurring `Deforest 0.30` every 60 ticks while `S` scales `r_max` uniformly at genesis (`0.42`). Across seeds valid in all 3 arms, the exact decomposition `(T - U) == (T - S) + (S - U)` holds identically.

### 2.2. Confirmatory Primary Family (`m = 3`, Template R2) — `OWNER DECISION — E3B-D2`

Template R2 limits the Holm-controlled confirmatory family to **at most 3 primary contrasts**. Two sub-decisions sit under **`OWNER DECISION — E3B-D2`**:

1. **Choice of the 3 contrasts:**
   - **Option 2A (Recommended — variance dispersion decomposition across all 3 arms on `live.trait_mass_var` at `T = 18,000`):**
     1. **`H1` (`T - S` on `live.trait_mass_var`):** Isolates spatial productivity gradient at matched ~`64k EU` total standing plant food.
     2. **`H2` (`T - U` on `live.trait_mass_var`, two-sided):** Total gradient + scarcity effect relative to abundant uniform control.
     3. **`H3` (`S - U` on `live.trait_mass_var`, two-sided):** Uniform scarcity + whole-map `AddNutrient 1.00` refill effect relative to abundant uniform control.
   - **Option 2B (Alternative — include regional East–West differentiation `S3` inside primary family):** Keep `H1` (`T - S`, `live.trait_mass_var`) and `H2` (`T - U`, `live.trait_mass_var`), and replace `H3` with **`H3_alt` (`T - S` on `live.trait_mass_mean_east - live.trait_mass_mean_west`, two-sided)**. *Trade-off:* brings regional differentiation directly under Holm FWER control, but requires an additional SESOI in mean-mass EU units and introduces uncalibrated `empty_half_rule` exclusions (`VIAB-1` did not record regional half-map occupancy).
2. **`H1` directionality and R10 label rule:**
   - **Default `(a)` (Directional `H1` with 4-label R10 set):** Predicts `T > S` (`H1` positive dispersion under spatial gradient); extends R10's closed decision set to **`{supports, opposite, equivalent_to_zero, insufficient_evidence}`** so a Holm-significant negative shift (`T < S`, $|\hat{\Delta}| \ge \text{SESOI}$) maps cleanly to **`opposite`** (refuting the directional prediction; never `supports` and never `equivalent_to_zero`).
   - **Alternative `(b)` (Two-sided `H1` with 3-label R10 set):** Leaves `H1` undirected (`T != S`) under `{supports, equivalent_to_zero, insufficient_evidence}`, so a Holm-significant negative shift (`T < S`, $|\hat{\Delta}| \ge \text{SESOI}$) is labeled `supports` and reported mandatorily with its negative sign (`T < S`). All hypotheses stay statistically tested via two-sided $p$-values under Holm–Bonferroni in both `(a)` and `(b)`.

### 2.3. Sensitivity Checks, Exploratory Family & Structural Zeros

- **Pre-registered sensitivity checks of `H1` (diagnostic only, never override `H1`):**
  1. `H1_bessel_sensitivity`: `var * n / (n - 1)` on `live.agent_count >= 2` at `T = 18,000`, accompanied by `live.agent_count` per arm reported explicitly as an ecological mediator.
  2. `H1_tail_window`: Equal-weight 7-sample mean over ticks `[14400, 18000]` (`14400, 15000, 15600, 16200, 16800, 17400, 18000`).
  3. Paired SACE on $G = LL$ (`survived && agent_count >= 2`) vs. Lachin Worst-Rank vs. RMST cross-check.
- **Exploratory Family (Benjamini–Hochberg FDR `q = 0.05`):** `S1` (`p90 - p10` at `T`, `n >= 2`), `S2` (`trait_mass_mean` at `T`), `S3` (`trait_mass_mean_east - trait_mass_mean_west` at `T` if Option 2A chosen, subject to `empty_half_rule`), and `S6` (`births` 30-window sum, `deaths` 30-window sum, and **`live.mean_lifespan` read from sampled `series.csv` at tick `18,000` plus post-warmup mean across positive-death windows — NOT from `final_observables`**, preventing the `F-053` drained-window structural zero observed in [E3-X Results §5](2026-11-20-e3-x-confirmation-results.md)).
- **Structural zeros & integrity checks (`decision_effect: none`):** `S4` (`live.shannon`, expected zero after predator extinction if `predator_fraction > 0` or permanent zero if `0.0`), `S5` (`live.archive_coverage`, structurally zero in headless adapter), `I1` (`live.closed_eu_total`, EU conservation `<= 1e-6`), and `M1` (`live.standing_crop` manipulation check `WP-2 b1/b2`).

---

## 3. Pre-Registered Statistical Design — Template Rules R1–R10

| Rule | Status | Concrete Value / `OWNER DECISION` |
|---|---|---|
| **R1** Seed unit & disjoint range | Concrete | Paired CRN by seed (`U, S, T`); confirmation `seed_k = 810001 + 1000*k` (`k = 0..N-1`); smoke `999953` |
| **R2** Confirmatory & exploratory families | `E3B-D2` (`2A` + `(a)` def.) | Primary `m = 3` (`H1, H2, H3`, Holm $\alpha = 0.05$: `0.01667, 0.025, 0.05`; def. `(a)` directional `H1` vs alt. `(b)` two-sided `H1`); Exploratory BH-FDR `q = 0.05` |
| **R3** Extinction rate & primary endpoint | `E3B-D3` (default `3A`) | `e3_b1_g4_fl1`: $p_{\text{ext}}$ `U=8.33%` (80% UB `22.96%`), `S=0%` (`12.55%`), `T=0%` (`12.55%`) $\implies$ **Worst-Rank primary** + RMST + SACE ($\hat{\pi}_{LL} \ge 0.85$) |
| **R4** Pre-specified SESOI | `OWNER DECISION` (`E3B-D4`) | **4A:** `\|d_z\| = 0.8` / `\|r_c\| = 0.50` · **4B (rec.):** `\|d_z\| = 0.5` / `\|r_c\| = 0.35` · **4C:** `±log(1.25) = ±0.2231` |
| **R5** Blind non-mass nuisance parameters | Concrete | Extracted strictly from `e3_b1_g4_fl1.json` (`880001..=880012`, `0` body-mass data read) |
| **R6** Sample size arithmetic | Tied to `E3B-D4` | Parametric/SACE anchors: **4A:** `K = 21` pairs $\to$ `N = 30` seeds (`90` runs) · **4B:** `K = 48` pairs $\to$ `N = 64` seeds (`192` runs) |
| **R6b** Pre-freeze Monte Carlo | Executed (`N_rep = 5,000`) | Worst-Rank, RMST & SACE power ± MCSE, Type I error, and `N_min(0.80)` across `S1..S8` (`Appendix A`, `python -I`, `0` `trait_mass` read) |
| **R7** Minimum attainable $p$ vs $\alpha/3$ | Concrete | Threshold $n \ge 7$ ($2/2^7 = 0.01563 < 0.01667$); Worst-Rank $p_{\min}(30) = 1.86\times 10^{-9}$; SACE floor $p_{\min}(26) = 2.98\times 10^{-8}$ |
| **R8** Full paired effect-size bundle | Concrete (`YES`) | $d_z$, $r_c$ (Kerby), $\hat{\Delta}_{\text{HL}}$ + 95% BCa CI, proportion $d_i > 0$ |
| **R9** TOST equivalence bounds | Tied to `E3B-D4` | `[-SESOI, +SESOI]` with 90% CI ($\alpha_{\text{TOST}} = 0.05$ each tail) |
| **R10** Closed decision table | `E3B-D2` (`(a)` default) | Closed mapping across all `48/48` combinations to `{supports, opposite, equivalent_to_zero, insufficient_evidence}` (alt. `(b)` 3-label if `H1` two-sided) |

### R1 — Experimental Unit, Disjoint Confirmation Seeds & Forbidden List

- **Experimental unit:** `seed` (`N` independent deterministic terrain/agent/ecology RNG trajectories, each evaluated across all 3 arms `{U, S, T}` under Common Random Numbers on one fixed host `Windows 1 x86_64`). Multiple agents within a run are never treated as independent replicates.
- **Proposed fresh confirmation seed sequence:**
  `seed_k = 810001 + 1000 * k` for `k ∈ 0..N-1` (for `N = 30`: `810001..839001`; for `N = 64`: `810001..863001`).
- **Proposed fresh smoke seed (mechanically excluded from confirmation):** `999953` (absent from confirmation manifests).
- **Verified disjointness (`0` hits in `src-tauri/tests/fixtures/` at `e576350fc`):** Completely disjoint from VIAB-1 calibration (`880001..=880012`), E3 (`730001..741001`, smoke `999979`), E2 (`700001..711001`, smoke `999983`), E2R (`750001..`), E-SEL1 (`760001..771001`, smoke `999971`), SEL2-P (`780001..799001`, `799501..=799506`, smoke `999961`), and AE (`1..3`). All listed prior seeds are explicitly forbidden in E3b manifests.

### R2 — Multiplicity Control

- **Confirmatory Primary Family (`m = 3`):** Two-sided tests controlled at family-wise error rate **$\alpha = 0.05$** using **Holm–Bonferroni** step-down (`\alpha/3 = 0.016667`, `\alpha/2 = 0.025`, `\alpha = 0.05`). Under `Option 2A` (`H1: T - S`, `H2: T - U`, `H3: S - U` on `live.trait_mass_var`), `H1` is tested at step 1 against ordered $p$-values (every hypothesis stays two-sided; predicted sign `H1 > 0` is enforced strictly at interpretation/decision labeling per R10).
- **Exploratory Family:** Secondary endpoints (`S1`, `S2`, `S3`, `S6`) across all 3 contrasts controlled separately via **Benjamini–Hochberg FDR ($q = 0.05$)** and explicitly labeled exploratory.

### R3 — Extinction Rates (`e3_b1_g4_fl1`) & Primary Endpoint Choice

From `e3_b1_g4_fl1.json` (`n = 12` calibration seeds `880001..=880012`; single extinction in `U` seed `880004` at tick `9,000`; `0` singletons `agent_count < 2` among survivors — minimum surviving `final_n` is `U = 25`, `S = 15`, `T = 5`):

| Arm / Stratum | Extinctions (`n=12`) | Point $\hat{p}_{\text{ext}}$ | Exact 1-Sided 80% Clopper–Pearson UB | Exact 1-Sided 95% UB | Joint Survival $\hat{\pi}_{LL}$ (`surv & final_n >= 2`) | 80% Lower Bound on $\pi_{LL}$ |
|---|---:|---:|---:|---:|---:|---:|
| `control_uniform` (`U`) | `1/12` | `8.33%` | **`22.96%`** | `33.87%` | — | — |
| `control_uniform_scarcity` (`S`) | `0/12` | `0.00%` | **`12.55%`** | `22.09%` | — | — |
| `treatment_gradient` (`T`) | `0/12` | `0.00%` | **`12.55%`** | `22.09%` | — | — |
| Contrast `T - S` (`H1`) | — | — | `6.49%` (pooled `0/24`) | `11.73%` | **`12/12` (`100.00%`)** | **`87.45%`** |
| Contrast `T - U` (`H2`) & `S - U` (`H3`) | — | — | — | — | **`11/12` (`91.67%`)** | **`77.04%`** |
| All 3 arms valid (`U & S & T`) | — | — | — | — | **`11/12` (`91.67%`)** | **`77.04%`** |

- **Template R3 Rule:** Because `U` has $\hat{p}_{\text{ext}} = 8.33\% > 5\%$ (and the 80% one-sided upper bound exceeds `5%` on every arm: `22.96%` on `U`, `12.55%` on `S` and `T`), complete-pair truncation alone risks collider bias in contrasts involving `U`. Therefore:
  - **Primary Endpoint (`Option 3A` — Template R3 strict compliance):** **Two-part Lachin Worst-Rank Score across 100% of seeds** (`0` seeds dropped). Ranking rule (highest to lowest):
    1. All non-extinct runs with `live.agent_count >= 2` at `T = 18,000`, ordered strictly by observed endpoint value (`live.trait_mass_var`).
    2. Below all survivors, any surviving degenerate singleton (`live.agent_count == 1`), which carries structural `var = 0.0`.
    3. Below all survivors and singletons, all extinct runs (`live.agent_count == 0` at or before `T = 18,000`), ordered strictly by `first_extinct_tick` (earlier extinction ranks lower; equal extinction ticks tie).
    Paired contrast differences $d_i$ are tested with exact two-sided Wilcoxon signed-rank / sign-flip permutation, alongside:
  - **Co-reported Survival Component (`RMST`):** Restricted mean survival time at $\tau = 18,000$ ticks ($T_i = \min(\text{first\_extinct\_tick}, 18000)$) on `100%` of seeds.
  - **Co-reported Conditional Evolutionary Endpoint (`Paired SACE` on $G = LL$):** Paired `live.trait_mass_var` difference (`mean_delta`, Hodges–Lehmann, paired-$t$ and exact Wilcoxon) restricted to seeds surviving with `agent_count >= 2` in both arms, interpreted **only if realized $\hat{\pi}_{LL} \ge 0.85$** in that contrast.
- **`OWNER DECISION — E3B-D3`:** Confirm **`Option 3A`** (Worst-Rank Score on 100% of seeds primary across all `H1..H3`, with RMST + SACE when $\hat{\pi}_{LL} \ge 0.85$) **vs. `Option 3B`** (Hybrid: complete-pair SACE primary specifically on `H1` `T - S` — noting that with `0` extinctions in `S` and `T`, Worst-Rank reduces monotonically to the exact paired Wilcoxon rank test of `live.trait_mass_var` — and Worst-Rank primary on `H2`/`H3`).

### R4 — Pre-Specified SESOI (`OWNER DECISION — E3B-D4`)

No pilot or E3 body-mass effect size (`d = ...` or variance delta) was consulted. E3b requires selecting one SESOI prior to freezing (`Option 4B` recommended to provide informative TOST equivalence):

| Option (`OWNER DECISION — E3B-D4`) | Primary / SACE SESOI | Worst-Rank Rank-Biserial SESOI ($r_c$) | Frozen E3 `relative_delta_min` Guard | Scientific / Power Implication |
|---|---|---|---|---|
| **Option 4A** *(large effect, E3 continuity)* | **`\|d_z\| = 0.80`** (`SD_diff` units) | **`\|r_c\| = 0.50`** (`75%` concordance) | Co-reported (`>= 5%` of `control_mean`) | Detects only large gradient effects ($\ge 0.8$ SD); wide TOST equivalence band (`±0.8 SD`); needs `N = 30` seeds (`90` runs). |
| **Option 4B (Recommended)** *(moderate-to-large effect)* | **`\|d_z\| = 0.50`** (`SD_diff` units) | **`\|r_c\| = 0.35`** (`67.5%` concordance) | Co-reported (`>= 5%` of `control_mean`) | Detects moderate effects; tightens TOST band to `±0.5 SD` so `equivalent_to_zero` meaningfully rules out medium differences; needs `N = 64` seeds (`192` runs) for SACE/TOST, or `N = 60` (`180` runs) under Worst-Rank alone. |
| **Option 4C** *(ratio / natural log scale)* | **`±log(1.25) = ±0.2231`** on $\log(\text{var}_T / \text{var}_S)$ | `\|r_c\| = 0.35` (calibrated by non-mass MC) | Subsumed (`[0.80×, 1.25×]` ratio) | Scale-free ±25% variance ratio; requires specifying floor $\epsilon$ if any survivor has identical clone masses (`var == 0`) and running non-mass MC before freeze. |

*(Note: Template §5 leaves frozen E3's `dz_min = 0.8` and `relative_delta_min = 0.05` untouched for historical E3, while E3b's R4 locks E3b's own SESOI above.)*

### R5 — Nuisance Parameters from Calibration Seeds Only (Blind to Body Mass)

All values below were computed directly from `artifacts/experiments/viab1-shared-viability/cells/e3_b1_g4_fl1.json` (PR `#215` commit `d49cf4ff`, trunk `4794b6fc`, exploratory seeds `880001..=880012`, `0` `trait_mass` matches):

| Nuisance Parameter | `U` (`control_uniform`) | `S` (`control_uniform_scarcity`) | `T` (`treatment_gradient`) | Pairwise / Calibration Scope |
|---|---:|---:|---:|---|
| $\hat{p}_{\text{ext}}$ (tick $\le 18,000$) | `1/12` (`8.33%`) | `0/12` (`0.00%`) | `0/12` (`0.00%`) | `e3_b1_g4_fl1.json` |
| $\text{RMST}(\tau=18000)$ mean ± SD (min) | `17250.0 ± 2598.1` (`9000`) | `18000.0 ± 0.0` (`18000`) | `18000.0 ± 0.0` (`18000`) | All `12` seeds |
| `mean_N` mean ± SD | `100.4 ± 36.1` | `79.8 ± 27.5` | `99.5 ± 42.8` | Non-mass ecological proxy |
| `final_N` mean ± SD (min) | `74.6 ± 42.9` (`0`) | `52.2 ± 31.3` (`15`) | `92.6 ± 47.1` (`5`) | Singletons `agent_count < 2`: `0/36` surviving runs (`1` extinct in `U`) |
| `crop_post_warmup` mean ± SD (EU) | `125615.6 ± 6766.5` | `64217.2 ± 159.0` | `64139.5 ± 412.8` | Manipulation stability (`WP-2 b2`: `12/12` pass) |
| CRN $\hat{\rho}$ (`mean_N`) | — | — | — | `T–S = 0.937`, `T–U = 0.826`, `S–U = 0.770` |
| CRN $\hat{\rho}$ (`final_N`) | — | — | — | `T–S = 0.571`, `T–U = 0.776`, `S–U = 0.557` |
| CRN $\hat{\rho}$ (`crop_post_warmup`) | — | — | — | `T–S = 0.749`, `T–U = 0.527`, `S–U = 0.573` |
| Missing $\sigma(\Delta \text{trait\_mass\_var})$, $\rho(\text{mass})$ | Unmeasured (`QD-VIAB`) | Unmeasured (`QD-VIAB`) | Unmeasured (`QD-VIAB`) | Blindness forbids estimating mass variance SD from VIAB-1; hence standardized $d_z$ (Options 4A/4B) or pre-freeze non-mass MC (Option 4C). |

### R6 — Sample Size Calculation (`N` Seeds) Showing Arithmetic

The pair counts below (`K = 21/27` and `K = 48/60`) come from the Template §4 paired-$t$ / Wilcoxon (`ARE = 0.955`) tables at two-sided Holm step-1 **$\alpha/3 = 0.016667$** (`m = 3`), which assume approximately normal paired differences and serve as parametric / SACE design anchors (with exact SciPy TOST & binomial survival checks). **Worst-Rank and RMST power under `Option 1A` / `5A` calibration has now been simulated directly via non-mass Monte Carlo (`N_rep = 5,000`) in `§3 R6b`** (reproducible script in **Appendix A**); if Owner instead chooses uncalibrated `Option 1B`, `5B`, or `5C`, §3 R6b must be re-run using that fresh VIAB-1 calibration cell before freezing manifests:

| Quantity | Option 4A (`d_z = 0.80`) | Option 4B (`d_z = 0.50`) |
|---|---|---|
| Pairs for paired-$t$ at $\alpha/3 = 0.01667$ (`80%` / `90%` normal-diff table) | `20` / `25` pairs | `45` / `57` pairs |
| Pairs for paired Wilcoxon (`ceil(n_t / 0.955)`, `80%` / `90%` normal-diff table) | **`K = 21`** / `27` pairs | **`K = 48`** / `60` pairs |
| Pairs for TOST equivalence `90% CI ⊂ [-SESOI, +SESOI]` at $\Delta = 0$ (`80%` / `90%`) | `t = 15 (W = 16)` / `t = 19 (W = 20)` | `t = 36 (W = 38)` / `t = 45 (W = 48)` |
| Binomial inflation for $P(\text{LL pairs} \ge K_{80\%}) \ge 0.90$ ($\pi_{LL} = 0.85$ conservative) | `N = 27` (`25` at $\pi_{LL} = 11/12$) | `N = 61` (`55` at $\pi_{LL} = 11/12$) |
| **Proposed Rounded Seed Count `N` (Total Runs `3N`)** | **`N = 30` seeds (`90` runs)** | **`N = 64` seeds (`192` runs)** *(or `N = 60` candidate if `0%` extinction assumed)* |
| Achieved guarantees at chosen `N` | - Worst-Rank (`§3 R6b`): `0.9284 ± 0.0036` at `0%/0%` (`S1`), `0.8344 ± 0.0053` at `5%/5%` corr (`S4`), `0.7650 ± 0.0060` (`N_min = 33`) at `5%/5%` ind (`S5`)<br>- SACE $P(X \ge 21 \mid \pi_{LL}=0.85) = 0.9903$<br>- $P(X \ge 21 \mid \pi_{LL}=11/12) = 0.9999$<br>- $P(\text{LL} \ge 26\;[0.85N] \mid 11/12) = 0.9001$ | - Worst-Rank (`§3 R6b`, `N = 64`): `0.8862 ± 0.0045` at `0%/0%` (`S1`), `0.7982 ± 0.0057` (`N_min = 65`) at `5%/5%` corr (`S4`), `0.7390 ± 0.0062` (`N_min = 73`) at `5%/5%` ind (`S5`); at `N = 60`: `0.8624`, `0.7742`, `0.7116`<br>- SACE $P(X \ge 48 \mid \pi_{LL}=0.85) = 0.9884$<br>- `N = 60` gives `0.8938` at `0.85` and `0.9989` at `11/12` |

### R6b — Pre-Freeze Monte Carlo Power Analysis (Worst-Rank & RMST)

Executed via `python -I` using the self-contained script in **Appendix A** (`N_rep = 5,000` replicates per `(scenario, N, dz)` cell, `BASE_SEED = 20261123`, Holm step-1 threshold `alpha/3 = 0.05/3 = 0.016667`, `TAU = 18,000` ticks, binomial `MCSE = sqrt(p*(1 - p) / 5000) <= 0.0071`). **Body-mass blindness (`QD-VIAB`) is preserved 100%:** zero `trait_mass` fields were read or referenced (`--viab1-cells-dir` asserts `0` `'trait_mass'` substrings across all `12` VIAB-1 JSONs), and treatment effects are injected strictly as synthetic standardized paired differences `dz`.

#### 1. Blind Nuisance Calibration Inputs (`e3_b1_g4_fl1` & Water-Fixed VIAB-1 Neighbors)

Recomputed directly from raw VIAB-1 cell JSONs under `artifacts/experiments/viab1-shared-viability/cells/` (`PR #215` commit `d49cf4ff`, trunk `4794b6fc`, calibration seeds `880001..=880012` — distinct from E3b's confirmation seeds `810001..`):

| Calibration Group (No `trait_mass` Read) | Seeds / Arm | Observed `p_ext` (`U` / `S` / `T`) | Realized `pi_LL` (`T&S` / `T&U` / `S&U`) | `U/T` Joint Ext. (`both` / `onlyU` / `onlyT` / `bothSurv`) | Pooled `ext_ticks` (`mean`, `median`, `IQR`) | Median `rho_mean` (`TS` / `TU`) | Median `rho_min` (`TS` / `TU_all` / `TU_LL`) |
|---|---:|---|---|---|---|---|---|
| Target `e3_b1_g4_fl1` (`1` cell) | `12` | `8.33%` (`1/12`) / `0.00%` / `0.00%` | `1.0000` / `0.9167` / `0.9167` | `0 / 1 / 0 / 11` | `n=1`: `[9000]` (`9000.0`, `9000.0`, `[9000, 9000]`) | `0.937 / 0.826` | `0.784 / 0.501 / 0.386` |
| `fl1` water-fixed (`4` cells: `g2..g5`) | `48` | `6.25%` (`3/48`) / `0.00%` / `2.08%` (`1/48`) | `0.9792` / `0.9167` / `0.9375` | `0 / 3 / 1 / 44` | `n=4`: `9150.0`, `9300.0`, `[7650, 10800]` | `0.859 / 0.826` | `0.810 / 0.789 / 0.753` |
| `g2..g5` water-fixed (`12` cells: `fl1`, `fl2`, `g2..g5`) | `144` | `11.81%` (`17/144`) / `0.00%` / `2.08%` (`3/144`) | `0.9792` / `0.8750` / `0.8819` | `2 / 15 / 1 / 126` | `n=20`: `10290.0`, `10800.0`, `[7950, 14400]` | `0.777 / 0.826` | `0.810 / 0.762 / 0.742` |

- **Empirical 20-tick extinction pool (`EXT_TICKS_12`):** `[1800, 3000, 3600, 4200, 4800, 9000, 9600, 9600, 10200, 10800, 10800, 10800, 10800, 13200, 14400, 14400, 15000, 15600, 16800, 17400]` across all `432` arm-runs (`U = 17`, `S = 0`, `T = 3`).

#### 2. Explicit Simulation Model & Assumptions

1. **Calibration scope (`Option 1A` + `Option 5A`):** Nuisance parameters strictly reflect `e3_b1_g4_fl1` (`predator_fraction = 0.3`, clonal `3.3` founder) and its `11` water-fixed VIAB-1 neighbors. If Owner selects `Option 1B`, `5B`, or `5C`, nuisance parameters and R6b must be re-run after that single blind VIAB-1 cell.
2. **Latent surviving outcome & `d_z` scaling:** Within surviving runs, $(Y_{i,A}, Y_{i,B})$ are bivariate normal with unit marginal variance and CRN correlation $\rho_Y$, shifted by $\mu_\Delta = d_z \sqrt{2(1 - \rho_Y)}$ so that the true paired standardized effect size is $d_z \in \{0.0, +0.5, +0.8, -0.5, -0.8\}$. Note two consequences: (a) Lachin Worst-Rank is invariant to any strictly monotone transformation of surviving values across both arms, and (b) holding $d_z$ fixed across $\rho_Y$ shrinks raw marginal shift $\mu_\Delta$ as $\rho_Y$ grows, which slightly widens marginal-rank tail spread relative to paired differences $Y_{i,B} - Y_{i,A}$.
3. **CRN proxy choices:** Because `trait_mass_var` cannot be inspected, $\rho_Y = 0.78$ for `H1` (`T - S`) is anchored to `e3_b1_g4_fl1`'s `min_n` correlation (`0.784`, more conservative than `mean_n = 0.937`), `0.81` in `S3` matches the `12`-cell `rho_min` median (`0.810`), `0.50` for `H2` (`T - U`) matches `e3_b1_g4_fl1` `rho_min(TU) = 0.501` (higher than `LL`-only `0.386`), and `S2` tests zero CRN benefit (`rho = 0.00`).
4. **Gaussian copula extinction & zero singletons:** Survival indicators $(S_{i,A}, S_{i,B})$ follow a bivariate Gaussian copula with correlation $\rho_{\text{ext}}$, sampled both at $\rho_{\text{ext}} = \rho_Y$ (strongly CRN-coupled extinctions) and independently ($\rho_{\text{ext}} = 0.00$, worst-case discordant-pair stress, consistent with `16/18` discordant extinctions across `U/T` in the `12`-cell pool). Extinction ticks for dead runs are sampled uniformly with replacement from `EXT_TICKS_12`; `0` singletons (`agent_count == 1`) are assumed, matching `0/36` surviving runs in `e3_b1_g4_fl1`.
5. **Non-informative censoring assumption (`SACE` caveat):** Extinction indicators are simulated independently of latent $(Y_{i,A}, Y_{i,B})$. Under real ecological drift where small/fragile populations go extinct more often, SACE suffers collider bias unless attrition is negligible — which is precisely why Template R3 elevates Worst-Rank when $p_{\text{ext}} > 5\%$.
6. **Wilcoxon implementation & R7 floor:** Every test enforces `n_nz >= 7` (`p = 1.0` when `n_nz <= 6`, since exact $p_{\min}(6) = 2/2^6 = 0.03125 > \alpha/3$). Worst-Rank and SACE use two-sided tie-corrected normal-approximation Wilcoxon (`scipy.stats.wilcoxon(..., method="approx")`, verified to match scalar `method="auto"` to `0.000000` max `|diff|` on Worst-Rank because `100%` of integer rank-difference vectors contain ties); RMST uses exact `2 / 2^n_nz` when all non-zero RMST differences share one sign, exact half-integer DP Wilcoxon for mixed-sign `7 <= n_nz <= 15`, and tie-corrected normal approximation for mixed-sign `n_nz > 15`.

#### 3. Monte Carlo Power & Type I Error (`N_rep = 5,000`, `alpha/3 = 0.016667`, `est ± MCSE`)

*(Note: RMST depends solely on survival outcomes and extinction ticks, so its rejection rate is invariant across `dz` and shown once under `dz = 0.0`.)*

| Scenario | `N` | Realized `pi_LL` | `dz = 0.0` WR (Type I) | `dz = 0.0` RMST | `dz = 0.0` SACE (Type I) | `dz = +0.5` WR (`4B`) | `dz = +0.5` SACE | `dz = +0.8` WR (`4A`) | `dz = +0.8` SACE |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `S1` H1-obs (S=0%, T=0%, rho=0.78) | `24` | `1.0000` | `0.0134 ± 0.0016` | `0.0000 ± 0.0000` | `0.0122 ± 0.0016` | `0.3780 ± 0.0069` | `0.4304 ± 0.0070` | `0.8272 ± 0.0053` | `0.8788 ± 0.0046` |
| `S1` H1-obs (S=0%, T=0%, rho=0.78) | `30` | `1.0000` | `0.0182 ± 0.0019` | `0.0000 ± 0.0000` | `0.0160 ± 0.0018` | `0.4848 ± 0.0071` | `0.5378 ± 0.0071` | `0.9284 ± 0.0036` | `0.9556 ± 0.0029` |
| `S1` H1-obs (S=0%, T=0%, rho=0.78) | `48` | `1.0000` | `0.0160 ± 0.0018` | `0.0000 ± 0.0000` | `0.0148 ± 0.0017` | `0.7648 ± 0.0060` | `0.8136 ± 0.0055` | `0.9950 ± 0.0010` | `0.9976 ± 0.0007` |
| `S1` H1-obs (S=0%, T=0%, rho=0.78) | `60` | `1.0000` | `0.0164 ± 0.0018` | `0.0000 ± 0.0000` | `0.0192 ± 0.0019` | `0.8624 ± 0.0049` | `0.8986 ± 0.0043` | `0.9994 ± 0.0003` | `0.9998 ± 0.0002` |
| `S1` H1-obs (S=0%, T=0%, rho=0.78) | `64` | `1.0000` | `0.0158 ± 0.0018` | `0.0000 ± 0.0000` | `0.0156 ± 0.0018` | `0.8862 ± 0.0045` | `0.9126 ± 0.0040` | `0.9996 ± 0.0003` | `1.0000 ± 0.0000` |
| `S2` H1-noCRN (S=0%, T=0%, rho=0.00) | `24` | `1.0000` | `0.0144 ± 0.0017` | `0.0000 ± 0.0000` | `0.0146 ± 0.0017` | `0.4126 ± 0.0070` | `0.4322 ± 0.0070` | `0.8662 ± 0.0048` | `0.8820 ± 0.0046` |
| `S2` H1-noCRN (S=0%, T=0%, rho=0.00) | `30` | `1.0000` | `0.0172 ± 0.0018` | `0.0000 ± 0.0000` | `0.0174 ± 0.0018` | `0.5266 ± 0.0071` | `0.5496 ± 0.0070` | `0.9456 ± 0.0032` | `0.9528 ± 0.0030` |
| `S2` H1-noCRN (S=0%, T=0%, rho=0.00) | `48` | `1.0000` | `0.0172 ± 0.0018` | `0.0000 ± 0.0000` | `0.0174 ± 0.0018` | `0.7946 ± 0.0057` | `0.8100 ± 0.0055` | `0.9962 ± 0.0009` | `0.9974 ± 0.0007` |
| `S2` H1-noCRN (S=0%, T=0%, rho=0.00) | `60` | `1.0000` | `0.0164 ± 0.0018` | `0.0000 ± 0.0000` | `0.0156 ± 0.0018` | `0.8990 ± 0.0043` | `0.9088 ± 0.0041` | `1.0000 ± 0.0000` | `1.0000 ± 0.0000` |
| `S2` H1-noCRN (S=0%, T=0%, rho=0.00) | `64` | `1.0000` | `0.0180 ± 0.0019` | `0.0000 ± 0.0000` | `0.0154 ± 0.0017` | `0.9110 ± 0.0040` | `0.9162 ± 0.0039` | `1.0000 ± 0.0000` | `1.0000 ± 0.0000` |
| `S3` H1-nbhd (S=0%, T=2.08%, rho=0.81) | `24` | `0.9790` | `0.0148 ± 0.0017` | `0.0000 ± 0.0000` | `0.0146 ± 0.0017` | `0.3002 ± 0.0065` | `0.4236 ± 0.0070` | `0.7142 ± 0.0064` | `0.8696 ± 0.0048` |
| `S3` H1-nbhd (S=0%, T=2.08%, rho=0.81) | `30` | `0.9792` | `0.0160 ± 0.0018` | `0.0000 ± 0.0000` | `0.0156 ± 0.0018` | `0.3962 ± 0.0069` | `0.5414 ± 0.0070` | `0.8300 ± 0.0053` | `0.9370 ± 0.0034` |
| `S3` H1-nbhd (S=0%, T=2.08%, rho=0.81) | `48` | `0.9788` | `0.0192 ± 0.0019` | `0.0000 ± 0.0000` | `0.0156 ± 0.0018` | `0.6614 ± 0.0067` | `0.8114 ± 0.0055` | `0.9788 ± 0.0020` | `0.9958 ± 0.0009` |
| `S3` H1-nbhd (S=0%, T=2.08%, rho=0.81) | `60` | `0.9795` | `0.0202 ± 0.0020` | `0.0002 ± 0.0002` | `0.0168 ± 0.0018` | `0.7572 ± 0.0061` | `0.8862 ± 0.0045` | `0.9966 ± 0.0008` | `0.9998 ± 0.0002` |
| `S3` H1-nbhd (S=0%, T=2.08%, rho=0.81) | `64` | `0.9793` | `0.0162 ± 0.0018` | `0.0004 ± 0.0003` | `0.0148 ± 0.0017` | `0.7810 ± 0.0058` | `0.9120 ± 0.0040` | `0.9968 ± 0.0008` | `0.9996 ± 0.0003` |
| `S4` H1-sym5-corr (S=5%, T=5%, rho=0.78) | `24` | `0.9228` | `0.0126 ± 0.0016` | `0.0000 ± 0.0000` | `0.0136 ± 0.0016` | `0.3020 ± 0.0065` | `0.3902 ± 0.0069` | `0.7048 ± 0.0065` | `0.8362 ± 0.0052` |
| `S4` H1-sym5-corr (S=5%, T=5%, rho=0.78) | `30` | `0.9238` | `0.0138 ± 0.0016` | `0.0000 ± 0.0000` | `0.0150 ± 0.0017` | `0.3928 ± 0.0069` | `0.5016 ± 0.0071` | `0.8344 ± 0.0053` | `0.9340 ± 0.0035` |
| `S4` H1-sym5-corr (S=5%, T=5%, rho=0.78) | `48` | `0.9228` | `0.0170 ± 0.0018` | `0.0004 ± 0.0003` | `0.0162 ± 0.0018` | `0.6572 ± 0.0067` | `0.7710 ± 0.0059` | `0.9780 ± 0.0021` | `0.9956 ± 0.0009` |
| `S4` H1-sym5-corr (S=5%, T=5%, rho=0.78) | `60` | `0.9231` | `0.0128 ± 0.0016` | `0.0016 ± 0.0006` | `0.0142 ± 0.0017` | `0.7742 ± 0.0059` | `0.8738 ± 0.0047` | `0.9932 ± 0.0012` | `0.9994 ± 0.0003` |
| `S4` H1-sym5-corr (S=5%, T=5%, rho=0.78) | `64` | `0.9239` | `0.0172 ± 0.0018` | `0.0032 ± 0.0008` | `0.0166 ± 0.0018` | `0.7982 ± 0.0057` | `0.8886 ± 0.0044` | `0.9964 ± 0.0008` | `0.9998 ± 0.0002` |
| `S5` H1-sym5-ind (S=5%, T=5%, rE=0.00) | `24` | `0.9036` | `0.0174 ± 0.0018` | `0.0000 ± 0.0000` | `0.0136 ± 0.0016` | `0.2664 ± 0.0063` | `0.3752 ± 0.0068` | `0.6276 ± 0.0068` | `0.8226 ± 0.0054` |
| `S5` H1-sym5-ind (S=5%, T=5%, rE=0.00) | `30` | `0.9020` | `0.0168 ± 0.0018` | `0.0002 ± 0.0002` | `0.0142 ± 0.0017` | `0.3460 ± 0.0067` | `0.4890 ± 0.0071` | `0.7650 ± 0.0060` | `0.9252 ± 0.0037` |
| `S5` H1-sym5-ind (S=5%, T=5%, rE=0.00) | `48` | `0.9029` | `0.0144 ± 0.0017` | `0.0022 ± 0.0007` | `0.0144 ± 0.0017` | `0.5892 ± 0.0070` | `0.7580 ± 0.0061` | `0.9462 ± 0.0032` | `0.9944 ± 0.0011` |
| `S5` H1-sym5-ind (S=5%, T=5%, rE=0.00) | `60` | `0.9027` | `0.0160 ± 0.0018` | `0.0054 ± 0.0010` | `0.0144 ± 0.0017` | `0.7116 ± 0.0064` | `0.8616 ± 0.0049` | `0.9884 ± 0.0015` | `0.9992 ± 0.0004` |
| `S5` H1-sym5-ind (S=5%, T=5%, rE=0.00) | `64` | `0.9025` | `0.0164 ± 0.0018` | `0.0068 ± 0.0012` | `0.0164 ± 0.0018` | `0.7390 ± 0.0062` | `0.8828 ± 0.0045` | `0.9892 ± 0.0015` | `0.9994 ± 0.0003` |
| `S6` H1-sym10-ind (S=10%, T=10%, rE=0.00) | `24` | `0.8093` | `0.0184 ± 0.0019` | `0.0024 ± 0.0007` | `0.0118 ± 0.0015` | `0.1906 ± 0.0056` | `0.3114 ± 0.0065` | `0.4658 ± 0.0071` | `0.7658 ± 0.0060` |
| `S6` H1-sym10-ind (S=10%, T=10%, rE=0.00) | `30` | `0.8097` | `0.0176 ± 0.0019` | `0.0058 ± 0.0011` | `0.0154 ± 0.0017` | `0.2392 ± 0.0060` | `0.4348 ± 0.0070` | `0.5712 ± 0.0070` | `0.8816 ± 0.0046` |
| `S6` H1-sym10-ind (S=10%, T=10%, rE=0.00) | `48` | `0.8100` | `0.0166 ± 0.0018` | `0.0136 ± 0.0016` | `0.0162 ± 0.0018` | `0.4204 ± 0.0070` | `0.6842 ± 0.0066` | `0.8262 ± 0.0054` | `0.9854 ± 0.0017` |
| `S6` H1-sym10-ind (S=10%, T=10%, rE=0.00) | `60` | `0.8112` | `0.0142 ± 0.0017` | `0.0164 ± 0.0018` | `0.0150 ± 0.0017` | `0.5310 ± 0.0071` | `0.8042 ± 0.0056` | `0.9084 ± 0.0041` | `0.9964 ± 0.0008` |
| `S6` H1-sym10-ind (S=10%, T=10%, rE=0.00) | `64` | `0.8103` | `0.0154 ± 0.0017` | `0.0138 ± 0.0016` | `0.0152 ± 0.0017` | `0.5554 ± 0.0070` | `0.8326 ± 0.0053` | `0.9308 ± 0.0036` | `0.9988 ± 0.0005` |
| `S7` H2-obs (U=8.33%, T=0%, rho=0.50) | `24` | `0.9164` | `0.0302 ± 0.0024` | `0.0036 ± 0.0008` | `0.0162 ± 0.0018` | `0.5234 ± 0.0071` | `0.3730 ± 0.0068` | `0.8998 ± 0.0042` | `0.8272 ± 0.0053` |
| `S7` H2-obs (U=8.33%, T=0%, rho=0.50) | `30` | `0.9152` | `0.0326 ± 0.0025` | `0.0080 ± 0.0013` | `0.0130 ± 0.0016` | `0.6714 ± 0.0066` | `0.4930 ± 0.0071` | `0.9656 ± 0.0026` | `0.9240 ± 0.0037` |
| `S7` H2-obs (U=8.33%, T=0%, rho=0.50) | `48` | `0.9167` | `0.0456 ± 0.0030` | `0.0944 ± 0.0041` | `0.0188 ± 0.0019` | `0.8952 ± 0.0043` | `0.7582 ± 0.0061` | `0.9986 ± 0.0005` | `0.9944 ± 0.0011` |
| `S7` H2-obs (U=8.33%, T=0%, rho=0.50) | `60` | `0.9169` | `0.0528 ± 0.0032` | `0.2364 ± 0.0060` | `0.0136 ± 0.0016` | `0.9608 ± 0.0027` | `0.8652 ± 0.0048` | `1.0000 ± 0.0000` | `0.9996 ± 0.0003` |
| `S7` H2-obs (U=8.33%, T=0%, rho=0.50) | `64` | `0.9171` | `0.0564 ± 0.0033` | `0.2830 ± 0.0064` | `0.0132 ± 0.0016` | `0.9694 ± 0.0024` | `0.8990 ± 0.0043` | `1.0000 ± 0.0000` | `0.9992 ± 0.0004` |
| `S8` H2-UB (U=22.96%, T=0%, rE=0.00) | `24` | `0.7695` | `0.1520 ± 0.0051` | `0.3112 ± 0.0065` | `0.0148 ± 0.0017` | `0.7532 ± 0.0061` | `0.2960 ± 0.0065` | `0.9726 ± 0.0023` | `0.7222 ± 0.0063` |
| `S8` H2-UB (U=22.96%, T=0%, rE=0.00) | `30` | `0.7689` | `0.1998 ± 0.0057` | `0.5594 ± 0.0070` | `0.0130 ± 0.0016` | `0.8806 ± 0.0046` | `0.4102 ± 0.0070` | `0.9918 ± 0.0013` | `0.8572 ± 0.0049` |
| `S8` H2-UB (U=22.96%, T=0%, rE=0.00) | `48` | `0.7711` | `0.3768 ± 0.0069` | `0.9450 ± 0.0032` | `0.0146 ± 0.0017` | `0.9886 ± 0.0015` | `0.6758 ± 0.0066` | `0.9998 ± 0.0002` | `0.9808 ± 0.0019` |
| `S8` H2-UB (U=22.96%, T=0%, rE=0.00) | `60` | `0.7698` | `0.4832 ± 0.0071` | `0.9922 ± 0.0012` | `0.0138 ± 0.0016` | `0.9974 ± 0.0007` | `0.7888 ± 0.0058` | `1.0000 ± 0.0000` | `0.9962 ± 0.0009` |
| `S8` H2-UB (U=22.96%, T=0%, rE=0.00) | `64` | `0.7707` | `0.5002 ± 0.0071` | `0.9948 ± 0.0010` | `0.0192 ± 0.0019` | `0.9988 ± 0.0005` | `0.8214 ± 0.0054` | `1.0000 ± 0.0000` | `0.9984 ± 0.0006` |

**Opposing-Sign Sensitivity (`dz < 0`, Surviving Contrast Opposes Extinction Direction) on Asymmetric Scenarios:**

| Scenario | `N` | `dz = -0.5` WR | `dz = -0.5` SACE | `dz = -0.8` WR | `dz = -0.8` SACE |
|---|---:|---:|---:|---:|---:|
| `S3` H1-nbhd (S=0%, T=2.08%, rho=0.81) | `24` | `0.4014 ± 0.0069` | `0.4046 ± 0.0069` | `0.8512 ± 0.0050` | `0.8664 ± 0.0048` |
| `S3` H1-nbhd (S=0%, T=2.08%, rho=0.81) | `30` | `0.5344 ± 0.0071` | `0.5312 ± 0.0071` | `0.9268 ± 0.0037` | `0.9398 ± 0.0034` |
| `S3` H1-nbhd (S=0%, T=2.08%, rho=0.81) | `48` | `0.8018 ± 0.0056` | `0.7974 ± 0.0057` | `0.9964 ± 0.0008` | `0.9968 ± 0.0008` |
| `S3` H1-nbhd (S=0%, T=2.08%, rho=0.81) | `60` | `0.9022 ± 0.0042` | `0.9000 ± 0.0042` | `0.9996 ± 0.0003` | `0.9996 ± 0.0003` |
| `S3` H1-nbhd (S=0%, T=2.08%, rho=0.81) | `64` | `0.9152 ± 0.0039` | `0.9154 ± 0.0039` | `1.0000 ± 0.0000` | `1.0000 ± 0.0000` |
| `S7` H2-obs (U=8.33%, T=0%, rho=0.50) | `24` | `0.1516 ± 0.0051` | `0.3820 ± 0.0069` | `0.4718 ± 0.0071` | `0.8350 ± 0.0052` |
| `S7` H2-obs (U=8.33%, T=0%, rho=0.50) | `30` | `0.2006 ± 0.0057` | `0.5000 ± 0.0071` | `0.6054 ± 0.0069` | `0.9242 ± 0.0037` |
| `S7` H2-obs (U=8.33%, T=0%, rho=0.50) | `48` | `0.3366 ± 0.0067` | `0.7602 ± 0.0060` | `0.8528 ± 0.0050` | `0.9940 ± 0.0011` |
| `S7` H2-obs (U=8.33%, T=0%, rho=0.50) | `60` | `0.4380 ± 0.0070` | `0.8698 ± 0.0048` | `0.9268 ± 0.0037` | `0.9998 ± 0.0002` |
| `S7` H2-obs (U=8.33%, T=0%, rho=0.50) | `64` | `0.4614 ± 0.0070` | `0.8994 ± 0.0043` | `0.9464 ± 0.0032` | `1.0000 ± 0.0000` |
| `S8` H2-UB (U=22.96%, T=0%, rE=0.00) | `24` | `0.0186 ± 0.0019` | `0.2988 ± 0.0065` | `0.0704 ± 0.0036` | `0.7270 ± 0.0063` |
| `S8` H2-UB (U=22.96%, T=0%, rE=0.00) | `30` | `0.0180 ± 0.0019` | `0.3966 ± 0.0069` | `0.0982 ± 0.0042` | `0.8522 ± 0.0050` |
| `S8` H2-UB (U=22.96%, T=0%, rE=0.00) | `48` | `0.0178 ± 0.0019` | `0.6680 ± 0.0067` | `0.1456 ± 0.0050` | `0.9822 ± 0.0019` |
| `S8` H2-UB (U=22.96%, T=0%, rE=0.00) | `60` | `0.0200 ± 0.0020` | `0.7852 ± 0.0058` | `0.1762 ± 0.0054` | `0.9952 ± 0.0010` |
| `S8` H2-UB (U=22.96%, T=0%, rE=0.00) | `64` | `0.0204 ± 0.0020` | `0.8206 ± 0.0054` | `0.1860 ± 0.0055` | `0.9974 ± 0.0007` |

#### 4. Smallest `N` Reaching `0.80` Power (`max_n = 256`, `N_rep = 5,000`)

| Scenario | WR (`dz = +0.8`, `4A`) | WR (`dz = +0.5`, `4B`) | SACE (`0.8`) | SACE (`0.5`) | RMST (Survival Only) | WR (`dz = -0.8`) | WR (`dz = -0.5`) |
|---|---|---|---|---|---|---|---|
| `S1` H1-obs (S=0%, T=0%, rho=0.78) | `23 (0.8024 ± 0.0056)` | `53 (0.8074 ± 0.0056)` | `21 (0.8050 ± 0.0056)` | `48 (0.8136 ± 0.0055)` | — | — | — |
| `S2` H1-noCRN (S=0%, T=0%, rho=0.00) | `21 (0.8034 ± 0.0056)` | `49 (0.8028 ± 0.0056)` | `21 (0.8160 ± 0.0055)` | `48 (0.8100 ± 0.0055)` | — | — | — |
| `S3` H1-nbhd (S=0%, T=2.08%, rho=0.81) | `28 (0.8094 ± 0.0056)` | `66 (0.8014 ± 0.0056)` | `22 (0.8176 ± 0.0055)` | `48 (0.8114 ± 0.0055)` | `>256 (0.2842@256)` | `23 (0.8186 ± 0.0054)` | `48 (0.8018 ± 0.0056)` |
| `S4` H1-sym5-corr (S=5%, T=5%, rho=0.78) | `29 (0.8114 ± 0.0055)` | `65 (0.8030 ± 0.0056)` | `23 (0.8142 ± 0.0055)` | `52 (0.8044 ± 0.0056)` | — | — | — |
| `S5` H1-sym5-ind (S=5%, T=5%, rE=0.00) | `33 (0.8022 ± 0.0056)` | `73 (0.8044 ± 0.0056)` | `24 (0.8226 ± 0.0054)` | `52 (0.8054 ± 0.0056)` | — | — | — |
| `S6` H1-sym10-ind (S=10%, T=10%, rE=0.00) | `46 (0.8072 ± 0.0056)` | `101 (0.8026 ± 0.0056)` | `27 (0.8326 ± 0.0053)` | `59 (0.8008 ± 0.0056)` | — | — | — |
| `S7` H2-obs (U=8.33%, T=0%, rho=0.50) | `20 (0.8166 ± 0.0055)` | `39 (0.8122 ± 0.0055)` | `23 (0.8080 ± 0.0056)` | `52 (0.8030 ± 0.0056)` | `109 (0.8062 ± 0.0056)` | `43 (0.8026 ± 0.0056)` | `124 (0.8066 ± 0.0056)` |
| `S8` H2-UB (U=22.96%, T=0%, rE=0.00) | `16 (0.8220 ± 0.0054)` | `26 (0.8182 ± 0.0055)` | `28 (0.8206 ± 0.0054)` | `62 (0.8088 ± 0.0056)` | `38 (0.8048 ± 0.0056)` | `>256 (0.7016@256)` | `>256 (0.0316@256)` |

#### 5. Plain Decision Implications for `E3B-D3` and `E3B-D4`

1. **Baseline `0%/0%` `H1` (`S1` vs. `S2`):** Even with zero extinction, Worst-Rank needs slightly more seeds than SACE (`N_min = 23` vs `21` at `d_z = 0.8`; `53` vs `48` at `d_z = 0.5`) because high `rho_CRN = 0.78` leaves marginal rank tails wider relative to direct paired differences. Both `N = 30` (`4A`, WR `0.9284 ± 0.0036`) and `N = 60`–`64` (`4B`, WR `0.8624 ± 0.0049` to `0.8862 ± 0.0045`) clear `0.80` comfortably if `S` and `T` experience `0%` extinction.
2. **Symmetric `5%/5%` stress (`S4`, `S5`) and `10%/10%` stress (`S6`):** Under `5%/5%` extinction (still inside `0/12`'s 80% UB `12.55%`), discordant survivor–extinct pairs claim the largest signed ranks and raise Worst-Rank `N_min` for `4B` (`d_z = 0.5`) to **`65`** (`0.8030 ± 0.0056`, with `N = 64` at `0.7982 ± 0.0057` and `N = 60` at `0.7742 ± 0.0059`) when survival is CRN-correlated (`S4`), and to **`73`** (`N = 64` at `0.7390 ± 0.0062`) when extinction is independent (`S5`). For `4A` (`d_z = 0.8`), `N = 30` clears `0.80` under correlated `5%/5%` (`0.8344 ± 0.0053`, `N_min = 29`) but drops to `0.7650 ± 0.0060` (`N_min = 33`) under independent `5%/5%`. Meanwhile paired SACE stays `>= 0.8828` (`N = 64`, `4B`) and `>= 0.9252` (`N = 30`, `4A`) across `S4`–`S5` while `pi_LL >= 0.9025 > 0.85`.
3. **Asymmetric `H2/H3` (`S7: U = 8.33%, T = 0%`; `S8: U = 22.96%, T = 0%`):** Worst-Rank tests the **composite (survival + trait) null**. At `dz = 0.0` (zero conditional trait difference among survivors), Worst-Rank rejects **`3.0%–5.6%`** in `S7` and **`15.2%–50.0%`** in `S8` at `alpha/3 = 0.0167` because `U` has higher extinction (`RMST` alone reaches `0.80` at `N = 109` in `S7` and `N = 38` in `S8`). When a surviving trait shift aligns with survival (`dz > 0`, `T > U`), Worst-Rank gains power (`S7`: `N_min = 20 / 39`); when it opposes survival (`dz < 0`, `T < U`), the signals cancel cleanly (`S7` `N_min = 43 / 124`; `S8` drops to `0.0316 @ 256` at `dz = -0.5`), whereas conditional SACE keeps nominal Type I error (`~0.013–0.019`) and `0.82–0.90` power at `N = 64` (`d_z = 0.5`).
4. **Concrete summary for Owner decisions:** For **`E3B-D3`**, `Option 3A` (Worst-Rank primary on all arms) obeys Template R3 strictly against collider bias on `H2/H3`, but on `H2/H3` a Worst-Rank rejection MUST be decomposed with `RMST` and `SACE` so excess mortality in `U` is never mistaken for trait divergence among survivors; `Option 3B` (SACE primary on `H1` when `pi_LL >= 0.85`, Worst-Rank on `H2/H3`) eliminates the Worst-Rank discordant-extinction penalty on `H1` at `N = 30` (`4A`) or `N = 64` (`4B`), assuming negligible collider bias within `H1`. For **`E3B-D4`**, choosing `Option 4B` favors **`N = 64`** (`N = 60` sits below `0.80` for Worst-Rank in both `S4` and `S5`, and `N = 73` would be needed for full Worst-Rank immunity to `5%/5%` independent extinction).

### R7 — Minimum Attainable Two-Sided Exact $p$-Value

For a two-sided exact paired sign / Wilcoxon signed-rank test on $n$ non-zero pairs, $p_{\min} = 2 / 2^n$. Comparing against Holm step-1 threshold $\alpha/3 = 0.016667$:

- Feasibility floor: **$n = 6$ fails** ($2 / 2^6 = 0.03125 > 0.016667$ — the mathematical failure mode of E3-X); minimum feasible count is **$n \ge 7$** ($2 / 2^7 = 0.015625 < 0.016667$).
- At E3b's planned sample sizes:
  - Worst-Rank primary (`N = 30`): $p_{\min} = 2 / 2^{30} = 1.86 \times 10^{-9}$ (`N = 64`: $1.08 \times 10^{-19}$).
  - SACE 85% validity floor on `N = 30` ($n = 26$): $p_{\min} = 2 / 2^{26} = 2.98 \times 10^{-8}$.
  - Target Wilcoxon pair count `K = 21` (`4A`): $p_{\min} = 9.54 \times 10^{-7}$; `K = 48` (`4B`): $p_{\min} = 7.11 \times 10^{-15}$.

### R8 — Full Paired Effect-Size Bundle (`YES`)

**`YES` — Mandatory on every primary and exploratory contrast** (replaces two-sample Cliff's $\delta$):

1. Paired Cohen's $d_z = \bar{d} / s_d$ and mean difference $\bar{d}$ with 95% $t$-CI and 90% TOST CI.
2. Matched-pairs rank-biserial correlation $r_c = (R_+ - R_-) / (R_+ + R_-)$ (never Cliff's $\delta$; never two-sample Vargha–Delaney thresholds).
3. Paired Hodges–Lehmann pseudomedian $\hat{\Delta}_{\text{HL}} = \text{median}_{i \le j}\frac{d_i + d_j}{2}$ with **95% BCa bootstrap confidence interval** (`10,000` bootstrap resamples, fixed deterministic seed).
4. Per-seed concordance proportion $\hat{p}_{d>0} = \frac{1}{n}\sum_{i=1}^n \mathbf{1}[d_i > 0]$, plus per-seed delta table, `agent_count_at_T`, extinction/singleton/empty-half exclusion tables, and 3-arm additive decomposition table `(T - U) == (T - S) + (S - U)`.

### R9 — TOST Equivalence Bounds

Two one-sided tests at $\alpha_{\text{TOST}} = 0.05$ each tail — equivalent to checking whether the **90% confidence interval** lies strictly inside **$(-\text{SESOI}, +\text{SESOI})$**:

- Under **Option 4A:** $( -0.80, +0.80 )\,\text{SD}_{\text{diff}}$ on SACE (`[-0.50, +0.50]` on Worst-Rank $r_c$);
- Under **Option 4B:** $( -0.50, +0.50 )\,\text{SD}_{\text{diff}}$ on SACE (`[-0.35, +0.35]` on Worst-Rank $r_c$);
- Under **Option 4C:** $( -\log 1.25, +\log 1.25 ) = (-0.2231, +0.2231)$ on paired log-variance ratio (ratio inside `[0.80, 1.25]`).

### R10 — Closed Decision Table

Under **default `(a)` (`E3B-D2`: directional `H1` `T > S`; two-sided `H2, H3`)**, every primary hypothesis evaluates via the table below into the closed 4-label set **`{supports, opposite, equivalent_to_zero, insufficient_evidence}`** (`opposite` is a distinct refutation outcome — **never** `supports` and **never** `equivalent_to_zero`). Under **alternative `(b)` (`E3B-D2`: two-sided `H1`)**, Row 4 merges with Row 3 as `supports` (reported with negative sign `T < S`), using the 3-label set `{supports, equivalent_to_zero, insufficient_evidence}`:

| WP-2 & Host Integrity Valid? | Stratum Sample Size Valid? (Worst-Rank: $0$ failed runs & $N \ge 7$; SACE: $\hat{\pi}_{LL} \ge 0.85$ & $n_{LL} \ge 7$) | Holm Adjusted $p_{\text{Holm}} < 0.05$? | Point Magnitude & Pre-Registered Sign | R9 TOST 90% CI Strictly Inside $(-\text{SESOI}, +\text{SESOI})$? | Closed Label (`R10`, def. `(a)`) | Mandatory Direction Annotation (`direction_tag`) |
|---|---|---|---|---|---|---|
| **No** | Any | Any | Any | Any | **`insufficient_evidence`** *(stop campaign on harness failure; never mask harness bug)* | `harness_invalid` |
| Yes | **No** ($n < 7$, or SACE $\hat{\pi}_{LL} < 0.85$ when SACE is sole primary) | Any | Any | Any | **`insufficient_evidence`** | `underpowered_or_truncated` |
| Yes | Yes | **Yes** | Positive (`T > S` on `H1`; or $\|\hat{\Delta}\| \ge \text{SESOI}$ on two-sided `H2,H3`) AND **$\|\hat{\Delta}\| \ge \text{SESOI}$** | No | **`supports`** | `predicted_positive` (`H1`) / `non_zero_two_sided` (`H2,H3`) |
| Yes | Yes | **Yes** | Negative on directional `H1` (`T < S`) AND **$\|\hat{\Delta}\| \ge \text{SESOI}$** | No | **`opposite`** *(under `E3B-D2` alt. `(b)` two-sided `H1`: `supports` with sign `T < S`; **never** `equivalent_to_zero`)* | **`opposite_negative`** — material effect opposite to `H1`; triggers Stage 4 finding note |
| Yes | Yes | **Yes** | $\|\hat{\Delta}\| < \text{SESOI}$ *(statistically non-zero but trivially small)* | **Yes** | **`equivalent_to_zero`** | `trivial_below_sesoi_equivalent` |
| Yes | Yes | **Yes** | $\|\hat{\Delta}\| < \text{SESOI}$ | **No** *(90% CI still straddles $\pm\text{SESOI}$)* | **`insufficient_evidence`** | `inconclusive_straddles_sesoi` |
| Yes | Yes | **No** | $\|\hat{\Delta}\| < \text{SESOI}$ | **Yes** | **`equivalent_to_zero`** | `equivalent_to_zero_powered_null` |
| Yes | Yes | **No** | Any (`>=` or `< SESOI`) | **No** | **`insufficient_evidence`** | `inconclusive_wide_ci` |
| Yes | Yes | Any | $\|\hat{\Delta}\| \ge \text{SESOI}$ (pos or neg) | **Yes** *(degenerate CI inconsistency; impossible when $\hat{\Delta} \in \text{CI}_{90}$)* | **`insufficient_evidence`** | `ci_inconsistent_impossible` |

- **Exhaustive 5-column partition verification (`48/48` combinations):** Treating the five columns as discrete inputs — `wp2_valid ∈ {Yes, No}` (`2`) × `stratum_valid ∈ {Yes, No}` (`2`) × `holm_sig ∈ {Yes, No}` (`2`) × `effect_sign_mag ∈ {pos_ge_sesoi, neg_ge_sesoi, lt_sesoi}` (`3`) × `tost_equiv ∈ {Yes, No}` (`2`) = **`48` combinations** — an exhaustive Python `itertools.product` check verified that **all `48/48` combinations match at least one and at most one row** (`len(matches) == 1` for every tuple):
  - Default **(a)** (`4`-label set): `supports = 1`, `opposite = 1`, `equivalent_to_zero = 2`, `insufficient_evidence = 44` (`24` from Row 1 + `12` from Row 2 + `1` from Row 6 + `3` from Row 8 + `4` algebraically impossible guard tuples in Row 9; total = `48`).
  - Alternative **(b)** (`3`-label set, two-sided `H1`): `supports = 2` (with sign reported explicitly), `equivalent_to_zero = 2`, `insufficient_evidence = 44` (total = `48`).

*(Note on `H1`: On `H1`, any positive finding still carrying E3's pre-declared confound caveat is worded strictly as: "a productivity gradient created by West `Deforest 0.30` and East `AddNutrient 1.00` altered body-mass variance vs. uniform scarcity (`resource_capacity_scale = 0.42` + whole-map `AddNutrient 1.00`) at matched ~64k EU standing crop, initialized at `predator_fraction = <chosen>` and `<chosen founder pool>`, with predators extinct by `T = 18,000`." If `Option 5A` is chosen, the report must explicitly state that every arm started at `trait_mass_var = 0.0` from the `3.3`-mass clonal founder and carried the background `3.3 -> ~2.5` reduction trend measured in `U`.)*

---

## 4. Smoke Gates, `WP-2` Gates, Host Rules & Run Budget

### 4.1. Pre-Execution Host & Isolation Guards (Carried Over from Frozen E3 / E3-X)

1. **Single execution host (`QD-E3HOST`):** All confirmation runs and the host smoke re-run execute on **`Windows 1` (`windows/x86_64`)** using a single `--release --features desktop` binary built from the frozen commit, recorded in `provenance.json`.
2. **Environment isolation (`F-051`):** Before every smoke and confirmation invocation, verify and record `ANIMA_WORLD_ARTIFACT` is unset (`<unset>`) and `<temp_dir>/anima_world_artifact.anmw` does not exist; abort immediately if either exists.
3. **`world_identity` invariant:** Every run must report `terrain_source == "generated"` and an identical `world_identity` struct (observed on Windows `x86_64`: `seed = 1337, generator_version = 1, width = 256, height = 256, checksum = 3152323152`); any mismatch aborts the campaign immediately (*stop and report, never tune*).
4. **Closed EU conservation:** `max_abs_eu_drift <= 1.0e-6 EU` across all arms and ticks.

### 4.2. Blind Smoke Check (`seed = 999953`) & Carried-Over `WP-2` Gates

Before running any confirmation seed `810001..`, execute 1 blind `--release` smoke run (`3` arms × seed `999953`, `body_mass_consulted: false` checking only `is_finite()`):

- **`WP-2(a)` ResourceField & Genesis EU:** `256×256` grid present; identical `closed_eu_total` (`153,587.25 EU`) across `U`, `S`, `T` (`S` starts with `~64.5k EU` plants + `~89.1k EU` detritus).
- **`WP-2(b1)` Treatment standing-crop shift:** `|(mean_post_warmup(T) - mean_post_warmup(U)) / mean_post_warmup(U)| >= 0.10` (note in `e3_b1_g4_fl1`: `12/12` seeds passed with `-0.549..-0.448`).
- **`WP-2(b2)` Scarcity standing-crop match:** `|(mean_post_warmup(T) - mean_post_warmup(S)) / mean_post_warmup(T)| <= 0.10` with `0` `Deforest` events in `S` (in `e3_b1_g4_fl1`: `12/12` seeds passed with `-0.0075..+0.0067`).
- **`WP-2(c)` Early detritus flow:** `detritus > 0` strictly before tick `3,000` (first sample `600`) and at tick `3,000` in all 3 arms (`12/12` in `e3_b1_g4_fl1`).
- **Horizon (`WP-3`):** `duration_ticks = 18,000` (`duration_ticks_ladder = [18000, 12000, 6000]`), `sample_period = 600` (`30` samples), `warmup_ticks = 3,000` (`5` excluded samples), `tail_window = [14400, 18000]` (`7` samples). Step down the duration ladder **only** from blind smoke timing if budget `> 90 min`; never reduce `N`.

### 4.3. Windows 1 Run Budget Extrapolation

Anchored to two measured Windows 1 benchmarks (without extrapolation beyond observed timings):
- **Parallel calibration timing (VIAB-1 `e3_b1_g4_fl1`, `12` threads):** `70.032 s` for `36` runs = **`1.945 s/run`**;
- **Sequential runner timing (`E3-X` `run_e3_experiment --ensemble`, single-threaded release):** `153.55 s` CLI wall for `36` runs = **`4.265 s/run`**.

| Seed Plan (`N` Triples) | Total Runs (`3N`) | Total Ticks (`18,000`/run) | VIAB-1 12-Thread Extrapolation | E3-X Sequential Extrapolation | Within 90 min Ceiling? |
|---|---:|---:|---:|---:|---|
| `N = 24` | `72` runs | `1,296,000` | `140.1 s` (`2.33 min`) | `307.1 s` (`5.12 min`) | Yes |
| **`N = 30` (Option 4A)** | **`90` runs** | `1,620,000` | **`175.1 s` (`2.92 min`)** | **`383.9 s` (`6.40 min`)** | Yes |
| `N = 60` (Option 4B WR) | `180` runs | `3,240,000` | `350.2 s` (`5.84 min`) | `767.8 s` (`12.80 min`) | Yes |
| **`N = 64` (Option 4B rec.)** | **`192` runs** | `3,456,000` | **`373.5 s` (`6.23 min`)** | **`818.9 s` (`13.65 min`)** | Yes |

*(One-time `--release` build cost on Windows 1 from `E3-X`: ~`10.4 min`; sequential vs bounded parallel confirmation execution stays well inside `90 min` for every candidate `N`.)*

---

## 5. `OWNER DECISION` Summary & Not Yet Done

### 5.1. Items Awaiting Owner Decision Before Freezing E3b

| ID | Decision Question | Options Summary | Recommended Default |
|---|---|---|---|
| **`E3B-D1`** | Initial `live.predator_fraction` (§1.2) | **1A:** Keep `0.3` (calibrated in `e3_b1_g4_fl1`; predators extinct `12/12`) · **1B:** Set `0.0` (clean herbivore-only genesis; needs 1 blind 70 s VIAB-1 cell) | Explicitly undecided (`1A` vs `1B`) |
| **`E3B-D2`** | Third hypothesis & `H1` directionality / R10 (§2.2, §3 R10) | Contrast: **2A** (`H3 = S - U` `trait_mass_var`) vs **2B** (`T - S` `east - west`) · `H1`/R10: **(a)** directional `H1` (`T > S`) + 4-label `{supports, opposite, equivalent_to_zero, insufficient_evidence}` vs **(b)** two-sided `H1` (3-label) | **2A** + **(a)** |
| **`E3B-D3`** | Primary endpoint rule under R3 (§3 R3, R6b) | **3A:** Lachin Worst-Rank on `100%` of seeds (`+ RMST + SACE`) · **3B:** Complete-pair SACE on `T - S` (`H1`) + Worst-Rank on `H2, H3` | **3A** (decompose `H2/H3` with RMST + SACE; see §3 R6b `H1` `5%/5%` penalty vs **3B**) |
| **`E3B-D4`** | SESOI & corresponding seed count `N` (§3 R4/R6/R6b) | **4A:** `d_z = 0.8` / `r_c = 0.50` (`N = 30`, `90` runs) · **4B:** `d_z = 0.5` / `r_c = 0.35` (`N = 64` / `192` runs; §3 R6b shows `N = 60` < `0.80` under `5%` extinction) · **4C:** `±log(1.25)` ratio | **4B** (`N = 64`) |
| **`E3B-D5`** | Founder pool & comparability with E3 (§1.3, `PR #216`) | **5A:** Keep clonal `3.3` founder (E3-comparable, calibrated in VIAB-1; carries `3.3 -> ~2.5` sweep) · **5B:** Adapted 2-seg `~2.5` founder (needs opt-in key + VIAB-1 re-cal) · **5C:** Standing-variation pool (needs schema/opt-in + VIAB-1 re-cal) | Explicitly undecided (`5A` vs `5B` vs `5C`) |

### 5.2. Explicitly Not Yet Done (Deferred Pending Owner Approval)

1. No frozen preregistration JSON (`e3b-preregistration.json`) or 6 arm/smoke manifests have been written.
2. No Rust runner, manifest test, stats calculator (`Worst-Rank`, `RMST`, `SACE`, `r_c`, `BCa HL`, `TOST`, `Holm`, `BH-FDR`), or artifact verifier script has been implemented or modified.
3. No opt-in adapted-founder or standing-variation founder-pool key has been implemented in `src-tauri/` (required before freeze only if Owner picks `Option 5B` or `Option 5C` in `E3B-D5`).
4. Pre-freeze non-mass Monte Carlo power verification (`N_rep = 5,000`) is **completed for Option `1A` + `5A`** in **`§3 R6b`** and **`Appendix A`**; it must only be re-run if Owner selects an uncalibrated `Option 1B`, `5B`, or `5C`.
5. No smoke (`999953`) or confirmation seeds (`810001..`) have been run.
6. `NR-SPIKE` (`win/nr-spike-protocol`) remains parked per lead instruction (`MSG_20261123_073000`) until E3b preregistration is approved.

---

## Appendix A — Reproducible Monte Carlo Power Script

Run standalone with `python -I` (requires NumPy and SciPy; verified on Python `3.x` with NumPy `2.2.6` / SciPy `1.16.3`). Optionally pass `--viab1-cells-dir <path-to-viab1-cells>` to assert `0` `'trait_mass'` substrings and verify that `EXT_TICKS_12` and all nuisance calibration metrics match the raw VIAB-1 JSON artifacts from `PR #215`:

```python
"""Pre-freeze, body-mass-blind Monte Carlo power analysis for E3b (§3 R6b).

Calibrates non-mass nuisance parameters (extinction rates, empirical extinction
ticks, joint survival pi_LL, and CRN correlations on mean_n / min_n) strictly
from VIAB-1 calibration cell e3_b1_g4_fl1 and its 11 water-fixed neighbors
(12 cells total, 144 seeds/arm, zero trait_mass fields consulted).

Evaluates Lachin Worst-Rank paired Wilcoxon, RMST paired Wilcoxon, and paired
SACE Wilcoxon (reference) at Holm step-1 threshold alpha / 3 = 0.05 / 3 using
N_rep = 5000 replicates per scenario, N in {24, 30, 48, 60, 64}, synthetic
standardized paired shifts dz in {0.0, +0.5, +0.8} (plus {-0.5, -0.8} on
asymmetric extinction scenarios), and binary search for smallest N >= 0.80.

Run standalone:
    python -I e3b_power_mc.py

Run with live VIAB-1 calibration verification:
    python -I e3b_power_mc.py --viab1-cells-dir <path-to-viab1-cells>
"""

import argparse
import json
import pathlib
import time
import numpy as np
from scipy import stats

ALPHA = 0.05 / 3.0  # Holm step-1 threshold for m = 3 primary contrasts at FWER 0.05
TAU = 18000.0       # E3b horizon (ticks)
N_REP = 5000        # Replicates per scenario/N/delta (>= 3000 required; max MCSE <= 0.0071)
BASE_SEED = 20261123
N_GRID = (24, 30, 48, 60, 64)

# 12 water-fixed g-family VIAB-1 calibration cells (PR #215, d49cf4ff; zero trait_mass fields)
FL1_CELLS = ("e3_b1_g2_fl1", "e3_b1_g3_fl1", "e3_b1_g4_fl1", "e3_b1_g5_fl1")
WATER_NBHD_12_CELLS = FL1_CELLS + (
    "e3_b2_g2_fl2",
    "e3_b2_g3_fl2",
    "e3_b2_g4_fl2",
    "e3_b2_g5_fl2",
    "e3_g2_strict_lakes16_dehyd960",
    "e3_g3_strict_sym100_water",
    "e3_g4_strict_sym160_water",
    "e3_g5_strict_herb_sym100_water",
)

# Exact 20 extinct_tick observations across all 3 arms in the 12 water-fixed VIAB-1 cells:
# U (17/144) + S (0/144) + T (3/144) = 20 extinctions out of 432 runs.
EXT_TICKS_12 = np.array(
    [
        1800, 3000, 3600, 4200, 4800, 9000, 9600, 9600, 10200, 10800,
        10800, 10800, 10800, 13200, 14400, 14400, 15000, 15600, 16800, 17400,
    ],
    dtype=float,
)

# Scenarios: (id, label, p_ext_a, p_ext_b, rho_y, rho_ext)
# Convention: contrast = B - A (for H1 T-S: A=S, B=T; for H2 T-U: A=U, B=T).
SCENARIOS = (
    ("S1", "H1-obs (S=0%, T=0%, rho=0.78)",           0.00,   0.00,    0.78, 0.78),
    ("S2", "H1-noCRN (S=0%, T=0%, rho=0.00)",         0.00,   0.00,    0.00, 0.00),
    ("S3", "H1-nbhd (S=0%, T=2.08%, rho=0.81)",       0.00,   3/144,   0.81, 0.81),
    ("S4", "H1-sym5-corr (S=5%, T=5%, rho=0.78)",     0.05,   0.05,    0.78, 0.78),
    ("S5", "H1-sym5-ind (S=5%, T=5%, rE=0.00)",       0.05,   0.05,    0.78, 0.00),
    ("S6", "H1-sym10-ind (S=10%, T=10%, rE=0.00)",    0.10,   0.10,    0.78, 0.00),
    ("S7", "H2-obs (U=8.33%, T=0%, rho=0.50)",        1/12,   0.00,    0.50, 0.50),
    ("S8", "H2-UB (U=22.96%, T=0%, rE=0.00)",         0.2296, 0.00,    0.50, 0.00),
)

DZ_CODE = {0.0: 0, 0.5: 1, 0.8: 2, -0.5: 3, -0.8: 4}


def verify_and_print_viab1_nuisance(cells_dir: pathlib.Path) -> None:
    """Recompute non-mass nuisance metrics from VIAB-1 JSONs and verify blindness."""
    groups = [
        ("e3_b1_g4_fl1 (1 cell)", ("e3_b1_g4_fl1",)),
        ("fl1 water-fixed (4 cells)", FL1_CELLS),
        ("g2..g5 water-fixed (12 cells)", WATER_NBHD_12_CELLS),
    ]
    print("=== VIAB-1 Non-Mass Nuisance Verification ===")
    all_raw_check = []
    for cid in WATER_NBHD_12_CELLS:
        p = cells_dir / f"{cid}.json"
        raw = p.read_text(encoding="utf-8")
        assert "trait_mass" not in raw, f"Blindness violation in {p}"
        all_raw_check.append((cid, json.loads(raw)))
    by_id = dict(all_raw_check)
    print(f"Blindness check: 0 'trait_mass' substrings across {len(by_id)} VIAB-1 cell JSONs.")

    ticks_12 = []
    for gname, cids in groups:
        n_tot = 0
        ext = {"u": [], "s": [], "t": []}
        pi_ts = pi_tu = pi_su = 0
        co_ut = only_u = only_t = both_surv_ut = 0
        rm_ts, rm_tu, rmin_ts, rmin_tu = [], [], [], []
        rmin_tu_ll = []
        for cid in cids:
            ps = by_id[cid]["per_seed"]
            n_tot += len(ps)
            for arm in ("u", "s", "t"):
                ext[arm].extend(r[arm]["extinct_tick"] for r in ps if not r[arm]["survived"])
            su = np.array([r["u"]["survived"] for r in ps], dtype=bool)
            ss = np.array([r["s"]["survived"] for r in ps], dtype=bool)
            st = np.array([r["t"]["survived"] for r in ps], dtype=bool)
            pi_ts += int((st & ss).sum())
            pi_tu += int((st & su).sum())
            pi_su += int((ss & su).sum())
            co_ut += int((~su & ~st).sum())
            only_u += int((~su & st).sum())
            only_t += int((su & ~st).sum())
            both_surv_ut += int((su & st).sum())
            rm_ts.append(stats.pearsonr([r["t"]["mean_n"] for r in ps], [r["s"]["mean_n"] for r in ps]).statistic)
            rm_tu.append(stats.pearsonr([r["t"]["mean_n"] for r in ps], [r["u"]["mean_n"] for r in ps]).statistic)
            rmin_ts.append(stats.pearsonr([r["t"]["min_n"] for r in ps], [r["s"]["min_n"] for r in ps]).statistic)
            rmin_tu.append(stats.pearsonr([r["t"]["min_n"] for r in ps], [r["u"]["min_n"] for r in ps]).statistic)
            ll_tu = [r for r in ps if r["t"]["survived"] and r["u"]["survived"]]
            rmin_tu_ll.append(stats.pearsonr([r["t"]["min_n"] for r in ll_tu], [r["u"]["min_n"] for r in ll_tu]).statistic)
        all_t = sorted(ext["u"] + ext["s"] + ext["t"])
        if len(cids) == 12:
            ticks_12 = all_t
        print(
            f"  {gname:30s} (n={n_tot:3d}): "
            f"p_ext U={len(ext['u'])}/{n_tot} ({100*len(ext['u'])/n_tot:.2f}%), "
            f"S={len(ext['s'])}/{n_tot} ({100*len(ext['s'])/n_tot:.2f}%), "
            f"T={len(ext['t'])}/{n_tot} ({100*len(ext['t'])/n_tot:.2f}%) | "
            f"piLL TS={pi_ts/n_tot:.4f} TU={pi_tu/n_tot:.4f} SU={pi_su/n_tot:.4f} | "
            f"U/T joint ext: both={co_ut}, onlyU={only_u}, onlyT={only_t}, bothSurv={both_surv_ut}"
        )
        print(
            f"    ext_ticks (n={len(all_t)}): mean={np.mean(all_t):.1f}, median={np.median(all_t):.1f}, "
            f"IQR=[{np.percentile(all_t, 25):.0f}, {np.percentile(all_t, 75):.0f}] | "
            f"rho_mean_med TS={np.median(rm_ts):.3f} TU={np.median(rm_tu):.3f} | "
            f"rho_min_med TS={np.median(rmin_ts):.3f} TU(all)={np.median(rmin_tu):.3f} TU(LL)={np.median(rmin_tu_ll):.3f}"
        )
    assert np.array_equal(np.array(ticks_12, dtype=float), EXT_TICKS_12), "EXT_TICKS_12 mismatch!"
    print("Verification OK: EXT_TICKS_12 matches the 20 raw extinction ticks identically.\n")


def exact_wilcoxon_dp(nz_vals: np.ndarray) -> float:
    """Exact two-sided Wilcoxon signed-rank p-value handling arbitrary ties via DP."""
    k = len(nz_vals)
    r = stats.rankdata(np.abs(nz_vals), method="average")
    w2 = np.rint(2.0 * r).astype(np.int64)
    tot2 = int(w2.sum())
    wpos2 = int(w2[nz_vals > 0.0].sum())
    wobs2 = min(wpos2, tot2 - wpos2)
    dp = np.zeros(tot2 + 1, dtype=np.float64)
    dp[0] = 1.0
    for w in w2:
        dp[w:] += dp[:-w].copy()
    p_one = float(dp[: wobs2 + 1].sum()) / (2.0 ** k)
    return min(1.0, 2.0 * p_one)


def wilcoxon_2sided_p_rows(d: np.ndarray, exact_small: bool = False) -> np.ndarray:
    """Vectorized two-sided Wilcoxon p-values per row, enforcing R7 (n_nz >= 7)."""
    n_rep = d.shape[0]
    p = np.ones(n_rep, dtype=float)
    nz = np.sum(d != 0.0, axis=1)
    active = nz >= 7  # R7 floor: exact p_min(n <= 6) = 2 / 2^6 = 0.03125 > alpha / 3
    if not np.any(active):
        return p
    idx_act = np.flatnonzero(active)
    if exact_small:
        pos = np.sum(d[idx_act] > 0.0, axis=1)
        neg = np.sum(d[idx_act] < 0.0, axis=1)
        same_sign = (pos == 0) | (neg == 0)
        if np.any(same_sign):
            idx_ss = idx_act[same_sign]
            p[idx_ss] = np.minimum(1.0, 2.0 / np.power(2.0, nz[idx_ss]))
        idx_rem = idx_act[~same_sign]
        small = nz[idx_rem] <= 15
        for i in idx_rem[small]:
            row = d[i]
            p[i] = exact_wilcoxon_dp(row[row != 0.0])
        idx_large = idx_rem[~small]
        if len(idx_large) > 0:
            p_l = stats.wilcoxon(
                d[idx_large], axis=1, zero_method="wilcox", alternative="two-sided", method="approx"
            ).pvalue
            p[idx_large] = np.where(np.isnan(p_l), 1.0, p_l)
    else:
        p_act = stats.wilcoxon(
            d[idx_act], axis=1, zero_method="wilcox", alternative="two-sided", method="approx"
        ).pvalue
        p[idx_act] = np.where(np.isnan(p_act), 1.0, p_act)
    return p


def mcse(p_hat: float, n_rep: int = N_REP) -> float:
    return float(np.sqrt(p_hat * (1.0 - p_hat) / n_rep))


def sim_cell(
    sc_idx: int,
    N: int,
    dz: float,
    p_ext_a: float,
    p_ext_b: float,
    rho_y: float,
    rho_ext: float,
    n_rep: int = N_REP,
    need_wr: bool = True,
    need_rmst: bool = True,
    need_sace: bool = True,
) -> tuple[float, float, float, float]:
    """Simulate one (scenario, N, dz) cell and return (p_wr, p_rmst, p_sace, pi_ll)."""
    seed = BASE_SEED + sc_idx * 100000 + N * 100 + DZ_CODE[dz]
    rng = np.random.default_rng(seed)

    # Paired surviving outcomes: Var(Y_a) = Var(Y_b) = 1, Corr(Y_a, Y_b) = rho_y,
    # shifted so paired Cohen's d_z = mu_delta / SD(Y_b - Y_a) = dz.
    delta_raw = dz * np.sqrt(2.0 * (1.0 - rho_y))
    z1 = rng.standard_normal((n_rep, N))
    z2 = rng.standard_normal((n_rep, N))
    ya = z1
    yb = rho_y * z1 + np.sqrt(1.0 - rho_y ** 2) * z2 + delta_raw

    # Gaussian copula extinction process independent of Y (non-informative censoring)
    e1 = rng.standard_normal((n_rep, N))
    e2 = rng.standard_normal((n_rep, N))
    ea = e1
    eb = rho_ext * e1 + np.sqrt(1.0 - rho_ext ** 2) * e2
    thr_a = stats.norm.ppf(p_ext_a) if p_ext_a > 0.0 else -np.inf
    thr_b = stats.norm.ppf(p_ext_b) if p_ext_b > 0.0 else -np.inf
    sa = ea >= thr_a
    sb = eb >= thr_b

    # Empirical extinction ticks on [1800..17400] for extinct runs; TAU = 18000 for survivors
    ta = np.where(sa, TAU, rng.choice(EXT_TICKS_12, size=(n_rep, N)))
    tb = np.where(sb, TAU, rng.choice(EXT_TICKS_12, size=(n_rep, N)))

    wr = rmst = sace = float("nan")
    if need_wr:
        # Lachin Worst-Rank: pool 2N observations; extinct ordered by t_ext <= 18000 < surviving (20000 + y)
        comb = np.concatenate([np.where(sa, 20000.0 + ya, ta), np.where(sb, 20000.0 + yb, tb)], axis=1)
        ranks = stats.rankdata(comb, method="average", axis=1)
        d_wr = ranks[:, N:] - ranks[:, :N]
        p_wr = wilcoxon_2sided_p_rows(d_wr, exact_small=False)
        wr = float(np.mean(p_wr < ALPHA))

    if need_rmst:
        p_rmst = wilcoxon_2sided_p_rows(tb - ta, exact_small=True)
        rmst = float(np.mean(p_rmst < ALPHA))

    both = sa & sb
    if need_sace:
        d_sace = np.where(both, yb - ya, 0.0)
        p_sace = wilcoxon_2sided_p_rows(d_sace, exact_small=False)
        sace = float(np.mean(p_sace < ALPHA))

    return wr, rmst, sace, float(np.mean(both))


def find_min_n(
    sc_idx: int,
    dz: float,
    pa: float,
    pb: float,
    ry: float,
    re: float,
    metric: str,
    target: float = 0.80,
    max_n: int = 256,
) -> tuple[int | None, float]:
    """Find smallest N in [8, max_n] with simulated power >= target (bracketed binary search)."""
    idx = {"wr": 0, "rmst": 1, "sace": 2}[metric]
    kw = {"need_wr": metric == "wr", "need_rmst": metric == "rmst", "need_sace": metric == "sace"}
    lo, hi = 8, 24
    while True:
        val = sim_cell(sc_idx, hi, dz, pa, pb, ry, re, **kw)[idx]
        if val >= target or hi >= max_n:
            break
        lo = hi + 1
        hi = min(max_n, hi * 2)
    if val < target:
        return None, val
    while lo < hi:
        mid = (lo + hi) // 2
        val_m = sim_cell(sc_idx, mid, dz, pa, pb, ry, re, **kw)[idx]
        if val_m >= target:
            hi = mid
        else:
            lo = mid + 1
    val_lo = sim_cell(sc_idx, lo, dz, pa, pb, ry, re, **kw)[idx]
    return lo, val_lo


def fmt_est(p_hat: float) -> str:
    return f"{p_hat:.4f}+/-{mcse(p_hat):.4f}"


def fmt_min_n(res: tuple[int | None, float], max_n: int = 256) -> str:
    n_min, val = res
    if n_min is None:
        return f">{max_n} ({val:.4f}@{max_n})"
    return f"{n_min:3d} ({val:.4f}+/-{mcse(val):.4f})"


def main() -> None:
    parser = argparse.ArgumentParser(description="E3b pre-freeze Monte Carlo power analysis.")
    parser.add_argument(
        "--viab1-cells-dir",
        type=pathlib.Path,
        default=None,
        help="Optional VIAB-1 cells directory to verify 0 trait_mass hits and nuisance constants.",
    )
    args = parser.parse_args()

    t0 = time.time()
    print(f"E3b Pre-Freeze Monte Carlo | ALPHA={ALPHA:.6f} (0.05/3) | N_REP={N_REP} | BASE_SEED={BASE_SEED} | TAU={TAU:.0f}")
    if args.viab1_cells_dir is not None:
        verify_and_print_viab1_nuisance(args.viab1_cells_dir)

    print("=== Main Grid: Power & Type I Error (est +/- MCSE) across N in {24, 30, 48, 60, 64} ===")
    for sc_idx, (sid, label, pa, pb, ry, re) in enumerate(SCENARIOS):
        print(f"\n[{sid}] {label}")
        for N in N_GRID:
            res_by_dz = {dz: sim_cell(sc_idx, N, dz, pa, pb, ry, re) for dz in (0.0, 0.5, 0.8)}
            pi_ll = res_by_dz[0.0][3]
            wr0, rm0, sc0, _ = res_by_dz[0.0]
            wr5, _, sc5, _ = res_by_dz[0.5]
            wr8, _, sc8, _ = res_by_dz[0.8]
            print(
                f"  N={N:2d} piLL={pi_ll:.4f} | "
                f"dz=0.0 WR={fmt_est(wr0)} RMST={fmt_est(rm0)} SACE={fmt_est(sc0)} | "
                f"dz=+0.5 WR={fmt_est(wr5)} SACE={fmt_est(sc5)} | "
                f"dz=+0.8 WR={fmt_est(wr8)} SACE={fmt_est(sc8)}"
            )

    print("\n=== Opposing-Sign Sensitivity (dz < 0) on Asymmetric Extinction Scenarios ===")
    for sc_idx in (2, 6, 7):
        sid, label, pa, pb, ry, re = SCENARIOS[sc_idx]
        print(f"\n[{sid}] {label}")
        for N in N_GRID:
            wrm5, _, scm5, _ = sim_cell(sc_idx, N, -0.5, pa, pb, ry, re)
            wrm8, _, scm8, _ = sim_cell(sc_idx, N, -0.8, pa, pb, ry, re)
            print(
                f"  N={N:2d} | dz=-0.5 WR={fmt_est(wrm5)} SACE={fmt_est(scm5)} | "
                f"dz=-0.8 WR={fmt_est(wrm8)} SACE={fmt_est(scm8)}"
            )

    print("\n=== Smallest N Reaching 0.80 Power (Binary Search, max_n=256) ===")
    for sc_idx, (sid, label, pa, pb, ry, re) in enumerate(SCENARIOS):
        w8 = fmt_min_n(find_min_n(sc_idx, 0.8, pa, pb, ry, re, "wr"))
        w5 = fmt_min_n(find_min_n(sc_idx, 0.5, pa, pb, ry, re, "wr"))
        s8 = fmt_min_n(find_min_n(sc_idx, 0.8, pa, pb, ry, re, "sace"))
        s5 = fmt_min_n(find_min_n(sc_idx, 0.5, pa, pb, ry, re, "sace"))
        extra = ""
        if pa != pb:
            rm = fmt_min_n(find_min_n(sc_idx, 0.0, pa, pb, ry, re, "rmst"))
            wm8 = fmt_min_n(find_min_n(sc_idx, -0.8, pa, pb, ry, re, "wr"))
            wm5 = fmt_min_n(find_min_n(sc_idx, -0.5, pa, pb, ry, re, "wr"))
            extra = f" | RMST={rm} | WR(-0.8)={wm8} WR(-0.5)={wm5}"
        print(f"  [{sid}] {label:42s} | WR(+0.8)={w8} WR(+0.5)={w5} | SACE(0.8)={s8} SACE(0.5)={s5}{extra}")

    print(f"\nCompleted in {time.time() - t0:.2f}s")


if __name__ == "__main__":
    main()
```
