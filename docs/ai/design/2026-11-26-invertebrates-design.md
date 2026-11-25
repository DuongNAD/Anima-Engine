---
phase: design
feature: invertebrates
title: Design — Insects and other invertebrates as guild fields (INVERT-D, proposed)
description: Proposed design for insects and soil fauna in the live world as Tier-3 guild biomass fields on the 256² sim grid (ADR-0018), with stage pools, ectotherm temperature and wet-season dependence, named closed-EU energy paths, aggregate trait evolution, field-driven decorative insects, a read interface for AQUA-D, AERO-D, FOSS-D and the ADR-0016 hazards, and an opt-in world law that defaults off.
status: proposed
owner: lead
last_reviewed: 2026-11-26
---

# Design — Insects and other invertebrates as guild fields (INVERT-D)

> **Status: `proposed`. Design only; no code is written under this card.** The owner asked on
> 2026-11-25 to "thêm cả côn trùng nữa" (add insects too). Code anchors are verified on
> `origin/feat/ecological-selection` at `38ea4621`. This design builds on
> [ADR-0018](../../decisions/ADR-0018-ecosystem-scales-and-cycles.md), which is itself `proposed`:
> its pillars were decided under `QD-ECOSCALE`, its numbers (`D-ECO-1`–`D-ECO-6`) are still open.
> QD-WV2-D2 (PR #295) and QD-WCHG (PR #296) are cited from their open pull requests. The owner
> decisions are in §11 and the cards in §10.

## 1. Purpose and boundaries

Insects are the food base for birds, fish, amphibians and many land animals; they also eat plants,
pollinate and speed decomposition. AQUA-D, AERO-D and FOSS-D need them, and they should read one
shared field rather than each invent their own.

Insects cannot be agents. Terrestrial arthropods hold about 0.2 Gt C against about 0.007 Gt C for
all wild mammals (Bar-On et al. 2018). The Tier-1 individual budget is about 1,000 agents at
5.643 ms p50 per tick, growing about N^1.86 (ADR-0018 §1, from `docs/paper/data/scaling.md`), and
the trial target is about 100 (QD-SCALE-TRIAL). Even ten insects per sim cell would be 655,360
individuals.

**Out of scope:** individual insect agents; flight mechanics (AERO-D); aquatic organisms before the
water realm (QD-REALMS, AQUA-D); insect swarms that bite hosts and vector disease (ADR-0018 card E6);
named species of any kind.

## 2. What the code has today (`38ea4621`)

| Fact | Anchor |
|---|---|
| No invertebrate exists. `insect`, `invertebrat` and `pollinat` match no Rust source under `src-tauri/`. | grep over `src-tauri/src`, `src-tauri/crates` |
| Closed EU has four compartments, `Plants`, `Animals`, `Detritus`, `Dormant`; the census sums them against a 1e-3 EU tolerance. | `energy.rs:19-31`, `:49-59`, `:123`, `closed_total_eu` `:375`; `ecosystem_census_system` `environmental_systems.rs:178-229` |
| The census **overwrites** `pool.animals` from agent reserves each tick, so nothing else may live in `Animals`. | `environmental_systems.rs:204` |
| Grass is `ResourceField`, a logistic field on the 256² grid whose regrowth draws on detritus, strided by 4. Vertebrate prey graze it. | `ecology.rs:331`, `:422`, `:453-494`; `environmental_systems.rs:35-85`, `:97` |
| Trees are clonal props: `seed_dropping_system` copies a parent's parameters; fruit is claimed from detritus. **No plant reproduces sexually, so pollination has nothing to act on.** ADR-0015 (proposed) adds pollen within `pollen_radius`, abiotically. | `components.rs:137`; `environmental_systems.rs:242`, `:370`; ADR-0015, outcrossing rule |
| There is no decomposer. Respiration goes straight to detritus; a death reclaims the reserve to detritus. Detritus is both dead matter and the regrowth budget, so there is no slow litter stage to speed up. V4 (proposed) adds a `CarcassPool`. | `world_systems.rs:223`; `agent_systems.rs:135`, `:170`; [V4 spec](../planning/2026-11-24-v4-carcasses-decomposition-spec.md) |
| `DynamicFields` has `soil_moisture`, `soil_nitrogen`, `soil_organic` and organic-to-N mineralization under a nutrient ledger, but **is not in the live schedule**; V1 (proposed) wires it in. | `dynamic_fields.rs:86-90`, `:45`, `step_soil` `:413`, `:461`; `tick_capture.rs:176-180` |
| Live temperature is `TerrainMap::temperatures`, normalized to [0,1]. On a pinned World v2 sim world it comes from °C through `normalize_temperature_c` over [−20, +40] °C, so `T_c = −20 + 60·t` is exact there. | `terrain.rs:134-135`; `anmw_v3.rs:50-51`, `:140-143`; `world_artifact.rs:601` |
| `climate.rs` is the offline generator's; it writes mean annual `temp_c` and `precip_m_yr` and has no seasonal cycle. | `anima-worldgen/src/climate.rs:168-171` |
| **The live world has no wet season.** `SeasonClock` is a sine with a 100 s year that scales plant fertility; `DynamicFields::step_climate` gives temperature a seasonal sine but precipitation none. | `ecology.rs:254-265`; `dynamic_fields.rs:257`, `:266-271` |
| Metabolic Theory helpers exist: `boltzmann_factor`, `E_ANIMAL_EV = 0.65`, Holling type III. | `ecology.rs:42`, `:30`, `:115` |
| Derived RNG streams in use: 1–5 and `DORMANCY_STREAM = 7`. ADR-0015 proposes 6 and 7 (7 collides), ADR-0016 proposes 8. | `resources.rs:925-948`; `aggregate_population.rs:190` |
| Decorative butterflies: up to 52 in one instanced mesh, labelled decoration. Low tier allows 60 draw calls. | `WorldWildlife.tsx:200`, `:363-367`; `simEnvironment.ts:17`; `qualityTiers.ts:65` |
| Sim grid 256² over MapBounds 200×200; at 81.92 km one sim cell is 320 m. | QD-WV2-D2 WB-D7 (PR #295, open) |

ADR-0018 already places "soil fauna & bacterial/fungal decomposers" and "aquatic insect larvae" in
Tier 3 population fields, and adult biting insects and social colonies in Tier 2 swarms (ADR-0018
§3.2). Its card E5 gates only the water and air larval cells on QD-REALMS
([ecosystem cards](../planning/2026-11-24-ecosystem-cards.md), E5 "START WHEN").

## 3. Decision summary

1. **Insects are Tier-3 guild fields**, the first concrete instance of ADR-0018 E5's
   `PopulationField`. No insect agent and no swarm is introduced here.
2. **Two guilds now:** *phytophages* (leaf and seed eaters, stage-structured) and *detritivores*
   (soil fauna, one pool). Pollinators wait for ADR-0015, aquatic larvae for AQUA-D. Predatory
   insects are not a guild; their effect is a declared density-dependent mortality term.
3. **One new compartment, `Invertebrates`.** Every flow in §6 moves EU between it and an existing
   compartment through `EnergyLedger`. Nothing is created or destroyed.
4. **Opt-in law**, `WorldLawSet::invertebrates`, plus a `live.invertebrates` key; absent means
   bit-identical to today (§9).
5. **Evolution is aggregate:** per-cell trait means and variances, never per-insect genotypes (§7).

## 4. Guilds and state

| Guild | Real members | Pools per cell | Eats | Role | When |
|---|---|---|---|---|---|
| Phytophage | caterpillars, leaf beetles, grasshoppers, seed bugs | egg, larva, pupa, adult | `ResourceField` grass (larva, adult) | herbivory; adults are aerial prey | now |
| Detritivore | springtails, mites, termites, beetle larvae, earthworms (not insects; grouped by function) | one | detritus; carcasses once V4 lands | decomposition; soil prey | now |
| Pollinator | bees, butterflies, hoverflies | adult | nectar (from `Plants`) | pollination | after ADR-0015 (IN-8) |
| Aquatic larva | mayflies, midges, mosquitoes, dragonflies | larva in water cells; emerges as adults | aquatic detritus and algae | fish and amphibian food | with AQUA-D (IN-9) |
| Predatory insect | ants, wasps, mantids | — | — | — | not modelled; mortality term |

**State** (structure of arrays, allocated once at genesis when the law is on, never when off):

- `bio[pool][cell]: f32`, EU per cell: five pools × 65,536 cells × 4 B = 1.25 MiB;
- `trait_mean` and `trait_var`: 2 guilds × 2 traits × 2 moments × 65,536 × 4 B = 2 MiB;
- one reused dispersal scratch buffer (256 KiB) and the stride phase, stored on the field so a
  checkpoint restores it (the lesson of `ResourceField::regrowth_phase`, `ecology.rs:357`).

**Update.** One system, on the `ECOLOGY_PERIOD = 60` band (1 Hz, `sim_clock.rs:20`) and strided
like `ResourceField`, placed after `herbivore_grazing_system` and before `ecosystem_census_system`
in `build_tick_schedule` (`simulation_schedule.rs:121`). Per visited cell: temperature, moisture and
season factors (§5) → feeding (§6) → stage transitions → egg laying → respiration and mortality →
adult dispersal. Allocation-free; fixed iteration order; no `HashMap`.

## 5. Biology

### 5.1 Ectotherm temperature

- Rates follow the Metabolic Theory: development time scales as `M^(1/4)·e^(E/kT)` (Gillooly et al.
  2002), with `E ≈ 0.65 eV` (Brown et al. 2004; already `E_ANIMAL_EV`).
- Above an optimum `T_opt`, performance falls steeply to a critical maximum `CT_max`: a thermal
  performance curve. Tropical insects live close to their optimum, so warming harms them first
  (Deutsch et al. 2008). That matters at 11°N. Guild values of `T_opt` and `CT_max`: chưa xác minh.
  `T_opt` is an evolving trait (§7).
- **Temperature source.** On a World v2 sim world, `T_c = −20 + 60·t` (§2). The generated legacy
  terrain declares no °C mapping, so the law refuses to start there unless the manifest declares
  one (IN-D6). Once V1 is live, `DynamicFields::air_temperature` replaces the static value.

### 5.2 Wet-season pulse

Tropical insect abundance tracks the rainy season (Wolda 1988). The live world has no rainfall
season today (§2), so the pulse needs a declared driver. Proposed, for IN-D6: the law declares a
wet-season window on the existing `SeasonClock` phase, giving a smooth rain index in [0,1]; when W1
gives `DynamicFields` a seasonal precipitation, that replaces it. Whether this world's 11°N climate
has one wet season or two: chưa xác minh. The pulse is not scripted. In dry conditions, eggs and pupae
develop at a reduced rate (diapause) and adults lose activity; at the onset of the rains,
development resumes and adults emerge together. The peak then emerges from the stage rates.

### 5.3 Life cycle as stage pools

- Phytophages are holometabolous: egg → larva → pupa → adult. Each step moves a fraction
  `r_s(T, W, S)·Δt` of a pool to the next, a linear chain that gives gamma-distributed stage
  durations. A declared share of each transition dies (moulting, pupation) and goes to detritus.
- Adults lay eggs from assimilated surplus; the egg pool holds that EU. Reproductive overhead goes
  to detritus.
- Detritivores are one pool with food-limited logistic growth: they are mostly ametabolous soil
  fauna, and no consumer needs their stages.

**Time honesty.** Real tropical phytophages complete a generation in weeks (chưa xác minh per taxon).
The world's year is compressed: the default `SeasonClock` year is 100 s, and the shared-island profile
quoted in ADR-0016 is `1 day = 20 min`, `1 year = 90 min`. An insect generation then lasts seconds to
minutes, as ADR-0018 §3.3 already expects for small organisms. The design keeps the *order* of the
timescales (insects turn over many times per vertebrate lifespan) and does not claim real
durations. Preflight rejects a profile where `r_s·Δt > 0.5` for any stage, because the chain would
skip stages.

### 5.4 Dispersal and density dependence

- Only adults move between cells: a 320 m cell is far larger than the daily movement of a larva.
  Field studies fit insect movement with diffusion models (Kareiva 1983); coefficients for this world
  are chưa xác minh.
- The scheme is conservative and synchronous: each land cell sends a fraction `m ≤ 0.2` of its adult
  pool to its four land neighbours through a scratch buffer, then applies it (the pattern of
  `DynamicFields::step_water`, `dynamic_fields.rs:354-378`). The moved amount is the measured `f32`
  delta, so mass is exact. Water cells reflect until AQUA-D. Trait moments mix weighted by mass.
- Food limitation: herbivory is Holling type II on grass above the S10 reserve floor
  (`PlantReservePolicy`, `resources.rs:678`), so insects cannot eat the reserve that vertebrate
  grazers are denied.
- Natural enemies: a smooth Beverton–Holt crowding term on the adult and larval pools stands in for
  predatory insects and parasitoids. It is named as such in the observables, not presented as a
  mechanism.

## 6. Energy paths inside the closed EU

**Compartment.** `Compartment::Invertebrates` with `EcosystemBiomass::invertebrates: f64`
(`#[serde(default)]`), the way `Dormant` was added (F1-1). It cannot be folded into `Animals`,
which the census overwrites, or into `Detritus`, which regrowth draws on freely. Under ADR-0018's open
flow it is part of the `E_fields` term of `R_open`.

| # | Flow | From → to | New `EnergyEvent` | How it is audited |
|---|---|---|---|---|
| F1 | Herbivory, assimilated part | `Plants` → `Invertebrates` | `InvertebrateHerbivory` | measured `f32` drop of `ResourceField.r[i]`; `pool.plants` debited as grazing does; flux `plants_to_invertebrates` |
| F2 | Frass (egested part of F1) | `Plants` → `Detritus` | same event | same measured drop, split by the declared assimilation share |
| F3 | Detritivory | `Detritus` → `Invertebrates` (and `Carcasses` → `Invertebrates` under V4) | `Detritivory` | `EnergyLedger::transfer` (`energy.rs:281`), clamped to what the source holds |
| F4 | Respiration | `Invertebrates` → `Detritus`; to `HeatLoss` under ADR-0018 open flow | `InvertebrateRespiration` | one per-cell cost accumulator, one transfer per step |
| F5 | Mortality: crowding, failed moults, reproductive overhead, hazards | `Invertebrates` → `Detritus` | `InvertebrateMortality` | cause tag in the observable; hazard deaths carry the `CauseId` |
| F6 | Insectivory (hook for AERO-D, FOSS-D, AQUA-D) | `Invertebrates` → agent reserve; unassimilated → `Detritus` | `Insectivory` | `transfer_into_reserve` (`energy.rs:233`) through `take` (§8) |
| F7 | Nectar (IN-8, later) | `Plants` → `Invertebrates` | `Nectar` | as F1 |
| N1 | Faster mineralization (needs V1) | `soil_organic` → `soil_nitrogen`, in NM, not EU | none | rate × `(1 + κ·detritivores)`; `nutrient_budget_residual` (`dynamic_fields.rs:461`) |
| N2 | Faster carcass decay (needs V4) | `Carcasses` → `Detritus` | V4's event | V4 rate × `(1 + κ'·detritivores)`; V4's audit |

**Identity with the law on:** `plants + animals + detritus + dormant (+ carcasses) + invertebrates`
equals the baseline within `RESIDUAL_ABS_TOLERANCE_EU = 1e-3` at every census. Every transfer goes
through `EnergyLedger`, never a raw `pool.x +=`, and uses the measured-delta pattern of
`step_regrowth_gated_strided`. A field's respiration is one term per cell, the field analogue of the
agent rule that every charge goes into `total_cost` (`world_systems.rs:157`). Any cost an agent pays
to forage for insects stays inside its `total_cost`.

**What decomposition can honestly do.** Soil fauna speed litter decomposition, by about 27% on
average across biomes and most in warm, wet climates (García-Palacios et al. 2013, exact figure chưa
xác minh; Wall et al. 2008). In today's closed-EU world, detritus is already available to regrowth, so
there is nothing slow to speed up. The speed-up therefore acts only on V4 carcasses (N2) and V5 soil
organic matter (N1). Without V1 or V4, detritivores are a reservoir that holds EU back from regrowth
and returns it through F4 and F5, and they are prey. That effect is real but small. The design says
so rather than inventing a litter pool.

**Numbers to calibrate against, never hard-coded.**

- Production efficiency of non-social insects is about 40%, against 1–3% for endotherms (Humphreys
  1979). Re-check the table before tuning.
- Tropical forests lose more leaf area to herbivores than temperate ones (Coley & Barone 1996). The
  share for this world is chưa xác minh.
- About 10% of energy crosses each trophic link (Lindeman 1942). This must emerge; it is not set.
  `LINDEMAN_EFFICIENCY = 0.40` is a per-strike assimilation constant (ADR-0018 §2.3) and is not the
  insect value.

## 7. Evolution

**What evolves.** Per cell and per guild, the mean and variance of two traits:

- phytophages: thermal optimum `T_opt`, and a host-use trait `h` that trades intake rate against
  development rate. Once ADR-0015's evolving grass exists, `h` pairs with grass palatability and
  defence, which is the coevolution hook.
- detritivores: `T_opt` and moisture tolerance.

**How.** Gaussian moment closure: the mean moves by `G·∂ln W̄/∂z̄` (Lande 1976), with `W̄` the local
per-capita growth from §5–§6. Selection erodes the variance and a declared mutational input
replenishes it, bounded within `[V_min, V_max]`. Dispersal mixes the moments, weighted by mass. The
update is deterministic, so there is **no genetic drift**, which is acceptable for very large local
populations and is stated as a limitation.

**What does not evolve:** guild identity, the stage structure, the number of guilds, body plan,
and new guilds. There is no speciation and no "species" label (ER09). A trait gradient is at most an
ecotype cline.

**Why no individual genotypes.** Section 1 gives the count. A super-individual Tier-2 pool (ADR-0018
E6) is the escape hatch for a later card that needs mobile, targeted insect groups, such as a
mosquito swarm seeking hosts. It is not proposed here.

**Testable prediction.** `T_opt` should form a cline along the temperature gradient of the massif.
With selection disabled the cline must not appear (IN-S06).

## 8. Interfaces for other designs and for hazards

A read-only resource view, `InvertebrateFields`, plus two mutating entry points:

| Call | Returns or does | Reader |
|---|---|---|
| `aerial_prey_eu(cell)` | adult phytophage EU in the cell | AERO-D insectivores |
| `ground_prey_eu(cell)` | larva plus pupa EU | ground foragers (Tier-1 agents) |
| `soil_fauna_eu(cell)` | detritivore EU | FOSS-D burrowers |
| `aquatic_larvae_eu(cell)` | 0 until IN-9 | AQUA-D fish and amphibians |
| `take(cell, pool, desired, reserve, cap)` | the **only** consumption path: Holling type III on density (`ecology.rs:115`) so rare prey keep a refuge; flow F6 | all consumers |
| `disturb(region, kind, severity, cause)` | hazard mortality to `Detritus` (F5), logged with its `CauseId` | ADR-0016 W1–W3 |

Hazards (QD-WCHG, opt-in), with every rate chưa xác minh:

- **Fire** (W3 burn timer): kills adults and larvae on burnt cells; eggs, pupae and soil fauna are
  partly buffered by the soil.
- **Ash fall:** lowers adult survival and, through the plants, food.
- **Flood** (W1 water stage, D-WCHG-3 runup): drowns soil fauna and pupae in proportion to how long
  the cell stays flooded; adult dispersal is biased away from flooded cells.
- **Recovery** is never scripted. It comes from the surviving egg and pupa bank plus recolonization by
  dispersal from neighbouring cells, so a large burn recovers from its edge inward.

RESP-D has no direct link: field insects have no oxygen store.

## 9. Science safety

- **Law.** `WorldLawSet::invertebrates: Option<InvertebrateLaw>`, serialized with
  `#[serde(default, skip_serializing_if = "Option::is_none")]`. When it is `None`, canonicalization
  emits zero bytes (the ADR-0016 §5 pattern; `laws.rs:85-118` today), so
  `WorldLawSet::baseline().fingerprint()` is unchanged. The live adapter reads `live.invertebrates`
  (absent or `0.0` = off), which must pass `validate_post_freeze_keys_default_to_legacy`
  (`prereg_sel2p2_manifest_tests.rs:422`) under QD-BIO-D2.
- **Off means absent.** No resource is inserted, no buffer is allocated and the system returns early
  on `Option<Res<_>>`. No RNG is drawn. `world_checksum` (`snapshot.rs:789`) hashes the field only
  when the resource exists (the V4 pattern), and the census calls the four-argument
  `closed_total_eu` unchanged.
- **RNG.** The field is deterministic; nothing is sampled. A later stochastic element takes its own
  new `sim_stream` id, chosen after the ADR-0015/0016 collision at 7 is reconciled.

| Gate | Positive check | Negative control |
|---|---|---|
| IN-S01 default-off identity | the frozen SEL2-P2 manifests, every `prereg_*` test and the E2/E3/E3b checksums are bit-identical with the law absent; fingerprint unchanged | a mutant that inserts an empty field when off, emits a tag byte for `None`, or draws one RNG value must turn the test red |
| IN-S02 EU closure | `\|R\| ≤ 1e-3` over ≥ 100,000 ticks with the law on | a mutant that credits F1 without debiting `pool.plants`, or drops the F2 frass, must exceed 1e-3 within the run |
| IN-S03 zero-alloc and replay | `TrackingAllocator == 0` on the step; three same-OS processes agree; `run N == run K → save → load → run N−K` | a save that omits the stride phase must break replay |
| IN-S04 causal herbivory | paired seeds: herbivory off leaves more standing grass than on | — |
| IN-S05 null field | law on with zero biomass and zero immigration: plants and agents match law-off exactly | — |
| IN-S06 evolution | `T_opt` cline correlates with cell temperature | selection off: no cline |
| IN-S07 hazard recovery | a burnt patch recovers from its edge | dispersal off: no recovery beyond the egg bank |

## 10. Cards

| Card | Size | Content | Waits for |
|---|---|---|---|
| IN-1 | M | law schema, live key, `Invertebrates` compartment, events, census term; IN-S01 | owner accepts this design and ADR-0018 (D-ECO-6) |
| IN-2 | L | field state and step: stages, temperature, moisture and season, dispersal, crowding; save/load; checksum only when on; IN-S03 | IN-1 (V1 optional) |
| IN-3 | M | flows F1–F5, frass, measured deltas; IN-S02, IN-S04, IN-S05 | IN-2 |
| IN-4 | M | trait moments and selection; IN-S06 | IN-3 |
| IN-5 | S | `InvertebrateFields` view, `take`, `disturb` (inert until W2/W3); IN-S07 once W3 lands | IN-3 |
| IN-6 | M | renderer: field-driven butterflies, fireflies, midge clouds; decorative fallback; map visual check | IN-3 |
| IN-7 | S | observables with ER10 metadata (biomass per guild and stage, flows, `T_opt`) | IN-3 |
| IN-8 | M | pollinator guild: nectar F7 and a pollen-success modifier on ADR-0015 outcrossing | ADR-0015 accepted and its plant cards |
| IN-9 | M | aquatic larva guild and emergence into the adult pool | AQUA-D accepted; water realm (QD-REALMS) |

Rendering detail for IN-6. A downsampled adult-density tile around the camera travels with the
existing density report (`SpatialDensityReport`, `telemetry_export.rs:463-470`, IPC
`get_spatial_density_matrix`), as an optional field changed together with its
[IPC contract](../../reference/IPC_CONTRACT.md) row. Instances are decorative; their density is
real.

| Kind | Where | Draw calls on low |
|---|---|---|
| Butterflies | by day over grass and forest | 0 extra: reuses the existing `WorldWildlife` mesh |
| Fireflies | dusk and night, moist cells; a visual proxy for adult insects, labelled as such | 1, additive points |
| Midge clouds | wet cells and shores | 1, points |

Bees appear only with IN-8. The total is two extra draw calls within the 60 on low. With the law off,
today's decorative butterflies stay as they are.

## 11. Owner decisions

| ID | Question | Options | Recommendation |
|---|---|---|---|
| IN-D1 | Which guilds first? | (A) phytophages and detritivores now; pollinators with ADR-0015; aquatic larvae with AQUA-D; predatory insects as a mortality term. (B) all five now. | **A**: pollination has no plant reproduction to act on yet, and larvae need the water realm. |
| IN-D2 | Representation | (A) Tier-3 fields only, with decorative rendering. (B) also Tier-2 swarms for flying adults. | **A**: swarms (E6) only when a consumer needs targeted mobile prey. |
| IN-D3 | Energy bookkeeping | (A) a fifth compartment `Invertebrates`, counted only when the law is on. (B) fold it into `Detritus`. | **A**: B lets regrowth eat live insects, and the census overwrites `Animals`. |
| IN-D4 | Order against ADR-0018 | (A) after ADR-0018 is accepted; IN-1 implements E5's `PopulationField` as its first instance, under closed EU, without waiting for E2/E3 open flow. (B) wait for the whole E1–E5 chain. | **A**: insects work under closed EU; open flow only reroutes F4 to `HeatLoss`. |
| IN-D5 | Evolution scope | (A) two traits per guild by moment closure, in a separate card (IN-4) after the ecology is green. (B) fixed guild parameters, no evolution. | **A**, with ER09 language: no species. |
| IN-D6 | Temperature and wet-season drivers | (A) `T_c = −20 + 60·t` on World v2 worlds, plus a declared wet-season window on `SeasonClock` until W1 adds seasonal rain; refuse legacy terrain without a declared mapping. (B) wait for V1 and W1. | **A**, labelled as declared drivers. |
| IN-D7 | Visible insects | (A) butterflies, fireflies and midges, two extra draw calls. (B) butterflies only. | **A**. |
| IN-D8 | Realm gating | (A) land invertebrate fields are part of the land ecosystem, not a QD-REALMS realm; only aquatic larvae wait for the water realm (as E5 says). (B) treat insects as a realm after water and sky. | **A**: ADR-0018 E5 already gates only the water and air cells. |

## 12. Sources

- Bar-On, Y. M., Phillips, R., Milo, R. (2018). The biomass distribution on Earth. *PNAS* 115:6506–6511.
- Brown, J. H., Gillooly, J. F., Allen, A. P., Savage, V. M., West, G. B. (2004). Toward a metabolic theory of ecology. *Ecology* 85:1771–1789.
- Coley, P. D., Barone, J. A. (1996). Herbivory and plant defenses in tropical forests. *Annu. Rev. Ecol. Syst.* 27:305–335.
- Deutsch, C. A., et al. (2008). Impacts of climate warming on terrestrial ectotherms across latitude. *PNAS* 105:6668–6672.
- García-Palacios, P., Maestre, F. T., Kattge, J., Wall, D. H. (2013). Climate and litter quality differently modulate the effects of soil fauna on litter decomposition across biomes. *Ecol. Lett.* 16:1045–1053.
- Gillooly, J. F., Charnov, E. L., West, G. B., Savage, V. M., Brown, J. H. (2002). Effects of size and temperature on developmental time. *Nature* 417:70–73.
- Humphreys, W. F. (1979). Production and respiration in animal populations. *J. Anim. Ecol.* 48:427–453.
- Kareiva, P. M. (1983). Local movement in herbivorous insects: applying a passive diffusion model to mark-recapture field experiments. *Oecologia* 57:322–327.
- Lande, R. (1976). Natural selection and random genetic drift in phenotypic evolution. *Evolution* 30:314–334.
- Lindeman, R. L. (1942). The trophic-dynamic aspect of ecology. *Ecology* 23:399–417.
- Wall, D. H., et al. (2008). Global decomposition experiment shows soil animal impacts on decomposition are climate-dependent. *Glob. Change Biol.* 14:2661–2677.
- Wolda, H. (1988). Insect seasonality: why? *Annu. Rev. Ecol. Syst.* 19:1–18.

Page and volume details are from memory and should be checked against the publisher before any of
these numbers is used for calibration.
