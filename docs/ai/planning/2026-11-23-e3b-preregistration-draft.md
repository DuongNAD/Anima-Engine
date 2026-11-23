---
phase: planning
feature: experiment-e3b-productivity-gradient
title: Draft Preregistration — E3b: Spatial Productivity Gradient and Body-Mass Dispersion (Post-VIAB-1 Redesign)
description: Documents-only draft preregistration for E3b based on VIAB-1 cell e3_b1_g4_fl1, specifying world overrides, predator_fraction and founder-pool trade-offs, 3-arm contrasts, template rules R1-R10, WP-2/smoke gates, and Windows 1 run budget
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
| **R6** Sample size arithmetic | Tied to `E3B-D4` | **4A:** `K = 21` pairs $\to$ `N = 30` seeds (`90` runs) · **4B:** `K = 48` pairs $\to$ `N = 64` seeds (`192` runs) / `60` WR |
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

**Plain Worst-Rank power disclosure:** **Worst-Rank power is not yet computed.** The pair counts below (`K = 21/27` and `K = 48/60`) come from the Template §4 paired-$t$ / Wilcoxon (`ARE = 0.955`) tables at two-sided Holm step-1 **$\alpha/3 = 0.016667$** (`m = 3`), which assume approximately normal paired differences and serve only as parametric / SACE design anchors (with exact SciPy TOST & binomial survival checks). Under `Option 3A` (Worst-Rank primary), **power must be confirmed by the pre-freeze non-mass Monte Carlo simulation** (`N_rep = 3,000–5,000`) before manifests freeze:

| Quantity | Option 4A (`d_z = 0.80`) | Option 4B (`d_z = 0.50`) |
|---|---|---|
| Pairs for paired-$t$ at $\alpha/3 = 0.01667$ (`80%` / `90%` normal-diff table) | `20` / `25` pairs | `45` / `57` pairs |
| Pairs for paired Wilcoxon (`ceil(n_t / 0.955)`, `80%` / `90%` normal-diff table) | **`K = 21`** / `27` pairs | **`K = 48`** / `60` pairs |
| Pairs for TOST equivalence `90% CI ⊂ [-SESOI, +SESOI]` at $\Delta = 0$ (`80%` / `90%`) | `t = 15 (W = 16)` / `t = 19 (W = 20)` | `t = 36 (W = 38)` / `t = 45 (W = 48)` |
| Binomial inflation for $P(\text{LL pairs} \ge K_{80\%}) \ge 0.90$ ($\pi_{LL} = 0.85$ conservative) | `N = 27` (`25` at $\pi_{LL} = 11/12$) | `N = 61` (`55` at $\pi_{LL} = 11/12$) |
| **Proposed Rounded Seed Count `N` (Total Runs `3N`)** | **`N = 30` seeds (`90` runs)** | **`N = 64` seeds (`192` runs)** *(or `N = 60` candidate if Worst-Rank alone)* |
| Achieved guarantees at chosen `N` | - Worst-Rank uses all `30` seeds (`> 27` Wilcoxon normal-diff anchor: **power to be confirmed by the pre-freeze Monte Carlo**)<br>- SACE $P(X \ge 21 \mid \pi_{LL}=0.85) = 0.9903$<br>- $P(X \ge 21 \mid \pi_{LL}=11/12) = 0.9999$<br>- $P(\text{LL} \ge 26\;[0.85N] \mid 11/12) = 0.9001$ | - Worst-Rank uses all `64` seeds (`> 60` Wilcoxon normal-diff anchor: **power to be confirmed by the pre-freeze Monte Carlo**; `N = 60` likewise **power to be confirmed by the pre-freeze Monte Carlo**)<br>- SACE $P(X \ge 48 \mid \pi_{LL}=0.85) = 0.9884$<br>- `N = 60` gives `0.8938` at `0.85` and `0.9989` at `11/12` |

*(Template §5 requirement: Before freezing manifests under Worst-Rank/RMST primary, a mandatory pre-freeze check simulates Worst-Rank and RMST power across $N_{\text{rep}} = 3,000\text{--}5,000$ Monte Carlo draws using only non-mass R5 nuisance distributions $\text{RMST}, p_{\text{ext}}, \pi_{LL}, \rho_{\text{CRN}}$ — with synthetic shift deltas $\Delta$ matching R4 so zero real `trait_mass` data are touched.)*

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
| **`E3B-D3`** | Primary endpoint rule under R3 (§3 R3) | **3A:** Lachin Worst-Rank on `100%` of seeds (`+ RMST + SACE`) · **3B:** Complete-pair SACE on `T - S` (`H1`) + Worst-Rank on `H2, H3` | **3A** (or note equivalence on `T-S` if `0` extinctions) |
| **`E3B-D4`** | SESOI & corresponding seed count `N` (§3 R4/R6) | **4A:** `d_z = 0.8` / `r_c = 0.50` (`N = 30`, `90` runs) · **4B:** `d_z = 0.5` / `r_c = 0.35` (`N = 64` / `192` runs or `60` WR) · **4C:** `±log(1.25)` ratio | **4B** |
| **`E3B-D5`** | Founder pool & comparability with E3 (§1.3, `PR #216`) | **5A:** Keep clonal `3.3` founder (E3-comparable, calibrated in VIAB-1; carries `3.3 -> ~2.5` sweep) · **5B:** Adapted 2-seg `~2.5` founder (needs opt-in key + VIAB-1 re-cal) · **5C:** Standing-variation pool (needs schema/opt-in + VIAB-1 re-cal) | Explicitly undecided (`5A` vs `5B` vs `5C`) |

### 5.2. Explicitly Not Yet Done (Deferred Pending Owner Approval)

1. No frozen preregistration JSON (`e3b-preregistration.json`) or 6 arm/smoke manifests have been written.
2. No Rust runner, manifest test, stats calculator (`Worst-Rank`, `RMST`, `SACE`, `r_c`, `BCa HL`, `TOST`, `Holm`, `BH-FDR`), or artifact verifier script has been implemented or modified.
3. No opt-in adapted-founder or standing-variation founder-pool key has been implemented in `src-tauri/` (required before freeze only if Owner picks `Option 5B` or `Option 5C` in `E3B-D5`).
4. Non-mass Monte Carlo power verification (`N_rep = 3,000–5,000`) for Worst-Rank / RMST (required by Template §5 before freezing) has not been executed yet.
5. No smoke (`999953`) or confirmation seeds (`810001..`) have been run.
6. `NR-SPIKE` (`win/nr-spike-protocol`) remains parked per lead instruction (`MSG_20261123_073000`) until E3b preregistration is approved.
