---
phase: design
feature: weather-storms
title: Design — WX-D, stochastic weather, thunderstorms and rare tropical storms (proposed)
description: Proposed design for an opt-in weather law. A WGEN-type generator on its own RNG stream drives a tropical wet/dry season at about 11°N, convective thunderstorms with orographic enhancement, wind and lightning. Rain enters the conserved DynamicFields water budget (feeding W1/W3 floods), lightning offers ignitions to W3 wildfire, creatures feel weather only through total_cost and body temperature, and the renderer's clouds and rain follow the simulated state. Tropical storms are rare and off by default. Split into cards WX-0 to WX-8 with gates WX-S01 to WX-S08 and owner decisions WX-D1 to WX-D9.
status: proposed
owner: simulation-architecture
last_reviewed: 2026-11-26
---

# `WX-D`: weather, thunderstorms and storms

> **Status: `proposed`, docs only.** Requested by the owner on 2026-11-26 ("thêm cả thời tiết mưa bão
> nữa"). There is no Rust or frontend code here. Implementation waits for owner answers to `WX-D1`–`WX-D9`
> (§9) and to the time-law decision `GAP-D1`. `GAP-D1` and the wider gap analysis are in
> `docs/research/REAL_WORLD_GAP_ANALYSIS_2026-11-26.md` (PR #303). Code anchors were read on
> `feat/ecological-selection` at `38ea4621`. Re-read the symbols before editing.
>
> **Evidence labels.** **Verified** means read in this repository. **Source** means an external
> publication; each DOI was checked against Crossref on 2026-11-26. **Design value** is a number chosen
> here, to be calibrated, never a gate. **Unverified** means no checked source exists yet.

## 1. What exists today (Verified)

| Piece | Anchor | What it does, and what it lacks |
|---|---|---|
| World v2 regional climate (offline) | `src-tauri/crates/anima-worldgen/src/climate.rs:1-4, 38-65, 76-89, 180-181, 386-390` | Fixed `11°N` latitude, `T0 = 27.5 °C`, lapse rate `6.3 °C/km`. A 2-D orographic advection model (Smith & Barstad 2004; Roe 2005) with onshore wind blowing west to east in the grid frame and a sampled azimuth. Windward target `3.2 m/yr`, floor `0.15 m/yr`. Annual means only; no weather. |
| Offline storm-runoff coefficient | `anima-worldgen/src/config.rs:54-56` | Used by terrain generation only. |
| Sim artifact `ANMW v3` | `anima-worldgen/src/anmw_v3.rs:105-124, 140-143` | Carries normalized `temperature`, mapped linearly from `[-20, +40] °C`, and `moisture`. It carries **no** `precip_m_yr`. |
| `DynamicFields` | `src-tauri/src/core/dynamic_fields.rs:66-108` | Fields for air temperature, precipitation, surface water and groundwater, soil and erosion. The water ledger counts rain as a declared source (`step_water` `:314-322`, `cum_precip`), and the residual is computed at `:398-402`. |
| …not scheduled | `src-tauri/src/core/tick_capture.rs:173-180` | Nothing steps `DynamicFields` in the live world. Card `V1` (`docs/ai/planning/2026-11-24-v1-dynamic-fields-schedule-spec.md`) schedules it. |
| `step_climate` | `dynamic_fields.rs:256-307` | Season is `0.1·sin(phase)`. Temperature and rain fall off toward the map edges as if they were poles (`:258-271`). This contradicts World v2's single latitude and must not be the World v2 profile. `TemperatureDelta` and `RainfallDelta` act inside a region (`:285-302`). |
| Seasonal regrowth | `src-tauri/src/core/ecology.rs:240-273`; `environmental_systems.rs:35-55` | `1 + 0.5·sin(phase)`, with a default year of `100 s`. This is the only live "climate". |
| Legacy global events | `src-tauri/src/evolution/meta_ai.rs:12-18`; `core/world_systems.rs:7-35` | `ResourceDrought`, `TemperatureSpike`, `GlacialPeriod` and `ToxicDeluge` change the food cap or shift every agent's temperature target. |
| Energy and heat | `core/world_systems.rs:157, 159-176` | `total_cost` is the single charge path. Heat is dissipated toward the **target** `37 °C`, not toward ambient, and body temperature is clamped to `[30, 45] °C`. |
| Brain inputs | `core/agent_systems.rs:1833-1849`; `evolution/brain_genotype.rs:50-54` | The 15 inputs include the agent's own `homeo.temperature` and `temp_target`, so a creature already *feels* being chilled. There is no input for light, rain, wind or ambient temperature. |
| RNG streams | `core/resources.rs:925-948`; `core/aggregate_population.rs:190` | Slots 1–5 and 7 are in use. Proposals already collide: `V0` and `ADR-0015` both claim 6, `ADR-0015` claims 7, and `ADR-0016` claims 8. |
| Renderer weather | `src/components/Landscape/utils/weatherKind.ts:15`; `WorldWeather.tsx:20-40`; `WorldShowcase.tsx:231-235, 260-263, 351-353` | Four user-selected kinds (`clear`, `rain`, `snow`, `fog`), each with a fixed precipitation rate, and a frontend-only time of day. Nothing comes from the simulation. |
| `WV2-LOOK` clouds (prototype) | branch `mac/wv2-look-proto` `49c3dc24`, `src/components/Landscape/materials/atmosphere.ts:46-49, 82, 203` | Cloud sheet with `uAtmoCloudCover` (default `0.65`), a base height of about `1,900 m`, wind drift and cloud shadows. Rendering only; it merges after `WV2-6` (`QD-WV2-LOOK`). |
| Tick payload | `core/simulation_state.rs:52-56`; `core/emit.rs:296` | `simulation-tick` carries `SimulationTickPayload { segments, environmental_state, head_directions }`. |
| Decided neighbours | `ADR-0016` with cards `W1`–`W7`; `QD-WCHG` (branch `mac/decisions-1126-wchg`) | `W1` is the living surface (seasonal lake stage, runoff, snow). `W3` adds fire, flash floods and landslides on `WORLD_DISTURBANCE`. Hazards are opt-in and rare by default (`D-WCHG-5 = A`). Drowning and gas suffocation belong to `RESP-D`. |

## 2. The climate to reproduce (Source, with honesty notes)

- **Season.** The reference region is the Sierra Nevada de Santa Marta coast at about 11°N. Sources
  agree on a dry season of roughly December–April and a wet season of roughly May–November. Some report a
  drier spell in mid-year (the "veranillo"). Boundaries differ by source and station.
  - Read only as search-result excerpts, because the pages returned `403`: the WWF ecoregion NT0159,
    WeatherSpark for Santa Marta, and the Río Frío watershed study (Hoyos et al. 2019).
  - The Caribbean low-level jet is strong over the Colombian Caribbean and has a bimodal annual cycle,
    with maxima around June–July and December–January (Amador 2008, via a secondary excerpt). This is
    consistent with stronger trades in the dry season and the veranillo.
  - Treat the season table in §4.2 as a **design value** shaped by these sources, not a calibration.
- **Interannual drought.** The Caribbean had its worst drought since the 1950s in 2013–2016 (Hoyos et
  al. 2019, abstract). Multi-year dry spells are real and must be possible.
- **Thunderstorms.** Orographic lift enhances rain on windward slopes (Roe 2005), and the generator already
  encodes it as an annual mean. Lake Maracaibo, a few hundred kilometres from the reference region, is
  the top lightning hotspot on Earth: about `233` flashes km⁻² yr⁻¹, driven by nocturnal local-breeze
  convergence (Albrecht et al. 2016, via NASA summaries). Global lightning averages about `44` flashes/s,
  with a land-to-ocean ratio of about `10:1` (Christian et al. 2003, via NASA abstract). Lightning and
  fire are both GCOS essential climate variables. Whether storms peak in the afternoon or at night is
  **unverified** for the reference coast, so it is a law parameter.
- **Tropical storms are rare here.** The Atlantic season runs 1 June–30 November and peaks around
  10 September (NHC climatology, via search; the NHC page returned `403`).
  - The whole Colombian Caribbean saw about `60` tropical-cyclone events in 1900–2010, or `0.55`/yr
    (Rey et al. 2021, citing Ortiz-Royero 2012). Ortiz-Royero's abstract names San Andrés (about
    12.5°N) and La Guajira as the most exposed areas.
  - A direct hit on an 82 km massif near 11°N is therefore much rarer than `0.55`/yr. The exact local
    rate is **unverified**.
  - When storms do strike, they select: island lizards surviving hurricanes Irma and Maria differed in
    body size, relative limb length and toe-pad size from the pre-storm population (Donihue et al. 2018).
  - Hurricane effects on forests are reviewed in Lugo 2008.

**Consequence for design:** tropical storms are **off by default**. They can be declared, and at most
they occur at an honest climatological rate (`WX-D5`). Everyday weather means seasonal rain, thunderstorms,
wind, cloud and dry spells.

## 3. Goals and non-goals

**Goals.**
1. A deterministic stochastic weather generator on its **own** RNG stream (`sim_stream::WORLD_WEATHER`,
   slot assigned by `WX-0`). It never draws from `SimRng`, from mutation, or from any other stream.
2. Rain that enters the existing conserved water budget, and therefore runoff, rivers, lake stage
   (`W1`) and floods (`W3`). No new water ledger.
3. Lightning strikes offered to `W3` as wildfire ignitions.
4. Creature effects only through `total_cost` and physics, with interfaces for `AERO-D`, `INVERT-D`
   and `RESP-D`.
5. The renderer's clouds, rain, fog and lightning follow the simulated state.
6. Opt-in: with `weather = None`, every existing run, including the frozen `SEL2-P2` and every
   `prereg_*` test, stays bit-identical.

**Non-goals.** No atmospheric dynamics solver, and no per-cell humidity transport. There is no snow
weather below the snowline: snow stays altitudinal under `W1`. Climate change is out of scope, and so
is any new brain input (that is `GAP-D6`). There is no new death cause in the first cut.

## 4. Design

### 4.1 Time mapping (depends on `GAP-D1`)

The shared-island profile runs at 60 ticks/s, with one day = 20 min (72,000 ticks) and one year = 90 min
(324,000 ticks), which gives 4.5 days per year (`ADR-0014` D-TIME-1/2). The day clock is 72× faster than
real time and the year clock 5,844× faster, so per year a run sees about 81× fewer weather days than
reality (round-2 review, item 9). There is also a science profile with 60 ticks per simulated day
(`QD-OPENQ`).

Rule (`WX-D2 = A`):
- **Event durations follow the day clock.** A thunderstorm cell lives for sim-minutes, which is
  seconds of real time on the island profile.
- **Season statistics follow the year clock**: wet fraction, mean intensity and spell persistence by
  season phase.
- **The compression is disclosed.** A "year" holds only a handful of rain spells, and the UI and
  chronicle say so, as `ADR-0016` §4.3 requires for geological time.
- **Units.** Every WX rate is declared per sim-day or per sim-year and resolved through the manifest's
  time profile, never in ticks.
- **No calendar, no weather.** A weather law without a declared calendar fails preflight.

### 4.2 Regional generator (`WX-D1 = A`, WGEN family)

The structure follows Richardson (1981) and the review by Wilks & Wilby (1999). Wet or dry occurrence is
a first-order, two-state Markov chain. Other variables are conditioned on the wet/dry state. The step is
one **weather block** of `Δb` sim-hours (design value `3 h`, so 8 blocks per sim-day).

| State (one per island, fixed size) | Update per block |
|---|---|
| `wet: bool` | Markov chain with `p01(φ)` and `p11(φ)`, where `φ` is the year-clock season phase |
| `rain_rate` | Drawn from a skewed distribution with seasonal mean `μ(φ)` when wet, else `0`. Gamma or mixed exponential is `WX-D1`'s sub-choice; the WGEN family uses both. |
| `cloud_cover ∈ [0,1]`, `t_anom` (°C), `wind_speed`, `wind_dir` | AR(1) anomalies conditioned on `wet`. Wind direction is centred on the generator's own azimuth (`climate.rs:180-181`), not on a geographic compass. |
| `drought_index` | Slow AR(1) on the year clock, scaling `p01`, so that multi-year droughts occur |

Season table (design values; `WX-8` calibrates it against the sources in §2): dry phase
`p01 ≈ 0.05`, `p11 ≈ 0.3`; wet phase `p01 ≈ 0.35`, `p11 ≈ 0.6`; a smooth transition over a sixth of
the year; and an optional mid-year dip ("veranillo") with amplitude `veranillo_amp ∈ [0, 1]`.

### 4.3 Spatial field (`WX-D3 = A`, `WX-D4 = A`)

All arrays have fixed size and are allocated once:
- **Orographic map `O[i]`** (`256²`, mean 1). It comes from worldgen's `precip_m_yr` and is written
  **offline** as a sidecar beside the pinned artifact, never inside it (the `P7 OFFLINE-TERRAIN`
  pattern). Its SHA-256 is pinned in the weather law. Without the sidecar, `WX-D4 = B` falls back to
  the artifact's `moisture`, and the run declares the fallback.
- **Storm discs**: at most `K` (design value 8) convective cells, each with centre, radius, intensity,
  age and lifetime.
  - **Spawning.** Discs spawn during wet blocks, with probability weighted by `O[i]`, so windward slopes
    and high ground get more of them (orographic enhancement). A diurnal phase parameter sets afternoon
    or nocturnal peaking.
  - **Motion.** Discs drift with `wind_dir` and `wind_speed`.
- **Precipitation each tick:**
  `precip[i] = c_unit · (rain_rate · O[i] + Σ_k disc_k(i))`.
  - `c_unit` converts `m/yr` to the field's water-volume units per tick under the declared time
    profile.
  - It is calibrated so that the long-run mean of `precip` reproduces the climatological map on the year
    clock.
- **Air temperature:**
  `T[i] = T_clim[i] + diurnal(t) + t_anom − rain_cooling · wet`.
  - `T_clim[i]` comes from `ANMW v3` temperature, decoded to °C (`anmw_v3.rs:140-143`, under `GAP-D2`).
- **Where it runs.** With the law on, this replaces `step_climate`'s base for World v2. Regional
  `TemperatureDelta` and `RainfallDelta` interventions still apply on top, through their existing arms
  (`dynamic_fields.rs:285-302`).

Rain therefore enters only through `precipitation → step_water → surface_water`, counted in
`cum_precip` (`dynamic_fields.rs:314-322`). Runoff, infiltration, lake stage (`W1`) and flash floods
(`W3`) consume it unchanged. The water residual gate (`< 1e-4`) holds by construction, and `WX-S03`
tests it.

### 4.4 Lightning (`WX-D7 = A`)

- **Strikes.** Each live disc emits strikes as a Poisson draw on the weather stream. The rate is
  `λ · intensity · area`, and `λ` is a design value calibrated so that strike density sits in a plausible
  land range against the Christian et al. 2003 global mean. Ocean cells are scaled by about `1/10`.
- **Ring buffer.** Strikes go to a fixed-capacity `LightningStrikes` ring buffer with deterministic
  overwrite, recorded in the world record.
- **Ignition.** `W3` ignites deterministically: a strike starts a fire when fuel dryness (low
  `soil_moisture`, high standing biomass) exceeds a law threshold. **No draw happens on
  `WORLD_DISTURBANCE` for the coupling**, so the two streams stay independent. Fire spread and burned
  biomass (`Plants → Detritus` under closed EU) remain `W3`'s responsibility.
- **Effect.** Dry-season lightning ignites more often than wet-season lightning. This is a pattern
  (`WX-S05`), not a rule. Process-based global fire models likewise take lightning as a natural
  ignition source (SPITFIRE; Thonicke et al. 2010).

### 4.5 Tropical storms (`WX-D5 = A`)

- **Off by default.** A storm is a single tracked object with position, heading, radius, wind and rain.
  It crosses the map over sim-hours, adds wind and rain on top of the regional state, and raises surge
  only through `W3` and `D-WCHG-3`.
- **Two ways to trigger it:**
  1. A host or experiment `InterventionKind::TropicalStorm` with a `CauseId`. Every exhaustive match
     arm listed in card `W3`'s allowed paths must be extended (`world_record/chain.rs`, `ring.rs`,
     `scenario.rs`, the manifest tags).
  2. `Climatological { rate_per_sim_year }`, with an honest default derived in `WX-6`. The derivation
     starts from `≤ 0.55`/yr for the whole Colombian Caribbean domain and states its assumptions.

### 4.6 Effects on creatures (`WX-D6 = A`)

All effects are opt-in sub-flags of the law. Every charge goes into `total_cost` in
`metabolic_decay_system` (`world_systems.rs:157`), never into a separate deduction.

| Effect | Mechanism | Why this way |
|---|---|---|
| Thermoregulation | Heat exchange toward a local effective ambient temperature, `T_eff = T[i] − wind_chill(wind) − wet_chill(rain)`, replaces relaxation toward `temp_target` (`:168-172`). When body temperature is below target, an explicit thermogenesis term `c_therm · deficit` is added to `total_cost` and to heat production. | The cost of cold is paid in energy. The brain feels it through its existing `homeo.temperature` input (`agent_systems.rs:1841`), with no new sense. Ectotherms and opening the `[30, 45]` clamp are `GAP-D2`. |
| Exposure during activity | `activity_cost × (1 + a_w · wind² + a_r · rain_rate)` | Drag and a wet coat make movement dearer. Foraging falls because it costs more, not because a script says so. |
| Shelter | Cells with canopy (`root_cover`, `ADR-0015` trees) or cave entrances reduce `wind_chill` and `wet_chill` (from the `P7` sidecar) | Selection can favour policies that end up sheltered. Honest limit: there is no directional sense, so deliberate shelter-seeking waits for `GAP-D6`. |
| Visibility (optional, `WX-D6 = C`) | Scales the maximum ray distance by `v(rain, cloud)` | A physical effect on sensing. It is off in the first cut. |
| Storm mortality | No new `DeathCause` in the first cut (`mortality.rs:187-192`). Deaths arrive through existing exhaustion, `RESP-D` drowning in flood water and `W3` fire and landslides. | Direct wind or lightning deaths are a later card with an `S9`-style cause split. |
| Birds (`AERO-D`) | Interface `WeatherState::flight_conditions(cell) -> (wind_speed, gust, rain_rate)`. `AERO-D` prices flight through `total_cost`; "grounded" means flight is unaffordable, not forbidden. | `AERO-D` owns the flight model. Weather constrains migrating birds strongly (Shamoun-Baranes et al. 2017). |
| Insects (`INVERT-D`) | Interface `WeatherState::activity_drivers(cell) -> (T, rain_rate, wind_speed)` for guild-field thermal performance and rain suppression (Huey & Kingsolver 1989 for the curve shape) | `INVERT-D` owns the parameters. |
| Water (`RESP-D`, `V1`) | Rain raises `surface_water`. Drinking from puddles is a later `V1`/`RESP-D` option. | Keeps a single water path. |

**No Lamarck.** Weather changes state and costs only, never a genotype.

### 4.7 Rendering link (`WX-D8 = A`)

- **Payload.** `WeatherSnapshot` is a fixed-size struct of about 200 bytes. It holds the regional
  state, `K` discs, up to `L` strikes since the last emit, and an optional storm. It is added as an
  optional field to `SimulationTickPayload` (`simulation_state.rs:52-56`), absent when the law is off.
  The ts-rs type is regenerated. No new command or event is needed, so `check_ipc_reference` is
  unaffected; confirm with `npm run check:ipc` and update the `simulation-tick` row of
  `IPC_CONTRACT.md` if the payload description changes.
- **Island guests.** The snapshot joins the `I3` island delta later.
- **Frontend wiring.**
  - When a live world is attached, `WorldWeather` derives `weather` and `precipitationRate` from the
    snapshot, replacing the user's state at `WorldShowcase.tsx:263`.
  - `WV2-LOOK`'s `uAtmoCloudCover` follows `cloud_cover`, and its wind follows `wind_*`.
  - Discs render as local dark cloud with rain shafts.
  - Strikes render as short sky flashes.
  - Fog derives from `cloud_cover`, rain and the time of day.
  - Snow stays altitude-driven.
- **Manual selector.** It remains for the showcase and capture (`capture?.weather`) and when no
  world is attached. Over a live world it shows a "preview weather, not simulated" label, like `C14`
  `HONEST-UI`.
- **Verification.** The `map-visual-check` captures run before and after.

### 4.8 Opt-in law and invariants

- **The law.** Add `#[serde(default, skip_serializing_if = "Option::is_none")] weather:
  Option<WeatherLaw>` to `WorldLawSet` (`experiment/laws.rs:28-32`). With `None`, `canonicalize`
  (`:85`) writes **zero** bytes, as `ADR-0016` does for `world_change`.
- **What the law pins:**
  - the calendar reference;
  - the season table;
  - the orographic sidecar hash;
  - the storm mode;
  - the lightning-to-fire flag;
  - the creature-effect flags.
- **No new `live.*` keys.** This avoids the frozen 31-key partition (`prereg_e3b_manifest_tests.rs:84`,
  `prereg_sel2p2_manifest_tests.rs:89`).
- **What stays unchanged:**
  - the legacy `SeasonClock` and `seasonal_fertility`;
  - `step_climate` for non-World-v2 maps;
  - legacy `EnvironmentalEvent`.
- **Schedule.** One system, `weather_step_system`, goes in `build_tick_schedule`
  (`simulation_schedule.rs:121`), ordered before the `V1` climate and water steps. It allocates nothing
  per tick: discs, strikes and the snapshot are fixed arrays.
- **Persistence.** The weather state and stream position enter `Snapshot` and `world_checksum`
  (`snapshot.rs:789`) only when the law is present.

## 5. Gates (`WX-S01`–`WX-S08`)

| Gate | Positive assertion | Negative control that must fail red |
|---|---|---|
| `WX-S01` | `weather = None`: unchanged `WorldLawSet::baseline().fingerprint()` and `world_checksum`. Every `prereg_*` test and the frozen `SEL2-P2` pass unmodified. | Write a tag byte for `None`, or allocate weather buffers in the default world. |
| `WX-S02` | Weather on with all creature and field couplings off: the ecology and mutation streams draw the same sequence as with weather off. | Draw any weather variable from `SimRng`. |
| `WX-S03` | `|water_budget_residual| / scale < 1e-4` over at least one sim-year, with storms and droughts | Add disc rain to `surface_water` without passing through `precipitation`/`cum_precip`. |
| `WX-S04` | Climatology patterns (pattern-oriented; Grimm et al. 2005), all checked across multiple seeds: (a) wet fraction in the dry phase is below the wet phase; (b) the wet-spell length distribution matches `1/(1−p11)`; (c) the long-run mean precipitation map ranks like `O[i]` (Spearman design value `≥ 0.9`); (d) interannual totals vary, so droughts occur. | Shuffle blocks in time, so (b) fails; set `O ≡ 1`, so (c) fails; set `drought_index ≡ 0`, so (d) collapses. |
| `WX-S05` | With fuel dryness above threshold, strike-to-ignition is higher than on wet fuel, and dry-season ignitions exceed wet-season ignitions. With the flag off, there are zero ignitions. EU is conserved per `W3`. | Ignite on every strike regardless of fuel, so the season contrast vanishes; or sample ignition on `WORLD_DISTURBANCE`, so a stream-isolation test fails. |
| `WX-S06` | Thermal and exposure charges appear only inside `total_cost`. The EU ledger closes. Behavioural assertion: an endotherm in cold, wet wind spends more energy than in calm, dry weather. Two tests, per `CLAUDE.md`. | Deduct the charge outside `total_cost`, so the ledger leaks. |
| `WX-S07` | The payload field is absent when the law is off. The frontend falls back to manual selection. Capture runs stay deterministic. `map-visual-check` produces before/after images. | Render live weather while the law is off. |
| `WX-S08` | `weather_step_system` cost is measured against the 100-agent tick (about `0.33 ms`, `QD-SCALE-TRIAL`), together with `P8 STEP-WATER-STRIDE`, and reported with no µs claim before measurement. Allocations per tick: 0. | Allocate a `Vec` per block, so the allocation test fails. |

Cross-platform note: the generator uses `libm` transcendentals, as World v2 does. Bit-identity is
claimed per platform class (`QD-M6`). Never pin an absolute checksum; compare runs.

## 6. Cards

| Card | Scope | Starts when | Size |
|---|---|---|---|
| `WX-0` TIME-AND-STREAMS | Docs and tests. Apply `GAP-D1` and `GAP-D2` as a declared calendar and °C mapping. Write one `sim_stream` slot registry that reconciles the 6/7/8 claims and assigns `WORLD_WEATHER`. | Owner answers `GAP-D1`, `GAP-D2`, `WX-D9` | S |
| `WX-1` WGEN-LIB | Pure, unscheduled functions (Markov chain, intensity sampler, AR(1), season table, solar and diurnal), with known-value tests. Overlaps round-2 `P6 CAL-FUNCS`; merge the two. | `WX-0` | M |
| `WX-2` WEATHER-LAW | `WeatherLaw`, `weather_step_system`, the orographic sidecar from worldgen, writes to precipitation and temperature, snapshot and checksum. Gates `WX-S01`–`S03`. | `WX-1`, `V1` merged | M |
| `WX-3` CONVECTION-LIGHTNING | Storm discs, orographic and diurnal spawning, the `LightningStrikes` ring, and the `W3` ignition hook interface. Gate `WX-S05` jointly with `W3`. | `WX-2`; `W2` for the hook | M |
| `WX-4` CREATURE-EXPOSURE | Thermal exchange toward `T_eff`, thermogenesis and exposure through `total_cost`, and canopy/cave shelter factors. Gate `WX-S06`. | `WX-2`, `GAP-D2`; `V0`/`V3` if they have landed | M |
| `WX-5` RENDER-LINK | Payload field, ts-rs type, `WorldWeather` and `WV2-LOOK` binding, "preview" label. Gate `WX-S07`. | `WX-2`, `WV2-LOOK` merged | M |
| `WX-6` TROPICAL-STORM | Storm object, the `TropicalStorm` `InterventionKind` with every exhaustive arm, and a climatological-rate derivation note | `WX-3` | M |
| `WX-7` REALM-INTERFACES | `flight_conditions`, `activity_drivers` and puddle water accessors only; no behaviour | `WX-2`; consumed by `AERO-D`, `INVERT-D`, `RESP-D` | S |
| `WX-8` CLIMATOLOGY-VALIDATION | `WX-S04` multi-seed suite and calibration note against §2's sources. Reports unverified items honestly. | `WX-3` | M |

## 7. Risks

- **Time compression confuses viewers.** "A whole wet season in three days." Mitigation: §4.1
  disclosure plus `GAP-D1`.
- **Cost.** Writing a full `256²` precipitation array every tick has a cost. Mitigation: fuse the
  regional term into `step_water`'s existing loop and touch only disc footprints per tick. Measure in
  `WX-S08`.
- **Calibration debt.** The season table and `λ` are design values until `WX-8`. Never use them as gates.
- **Stream-slot collisions** between proposals. Mitigation: `WX-0` registry.
- **Science runs.** Weather adds variance. Experiments that want it declare it in a new preregistration,
  and frozen ones never see it.

## 8. Related documents

- [`ADR-0016`](../../decisions/ADR-0016-changing-world-surface.md) and [cards `W1`–`W7`](../planning/2026-11-24-world-change-cards.md)
- [`ADR-0014`](../../decisions/ADR-0014-shared-island-host-and-guests.md), for the time profile
- [`ADR-0018`](../../decisions/ADR-0018-ecosystem-scales-and-cycles.md), for diurnal irradiance in `D-ECO-1`
- [`ADR-0019`](../../decisions/ADR-0019-brain-body-interface.md), for the fixed inputs
- [`V1` spec](../planning/2026-11-24-v1-dynamic-fields-schedule-spec.md)
- [World v2 design](2026-11-20-world-v2-design.md), for the climate stage
- [`AGENT_WORKFLOW.md`](../../how-to/AGENT_WORKFLOW.md), for `QD-WV2-LOOK`, `QD-WV2-BIG` and `QD-REALMS`
- Gap analysis (PR #303); `QD-WCHG` and `QD-WV2-D2` on their decision branches

## 9. Owner decisions (`WX-D1`–`WX-D9`)

| Slot | Question | Options (recommendation first) |
|---|---|---|
| `WX-D1` | Generator family | **(A) Recommended:** WGEN-type block Markov chain with a skewed intensity distribution and conditioned AR(1) anomalies. Cheap, deterministic and well studied. (B) A physical moisture-advection model, rejected on cost. (C) Replay of observed series, which needs data licensing and is not generation. |
| `WX-D2` | Clocks | **(A) Recommended:** event durations on the day clock and season statistics on the year clock, with the compression disclosed. (B) Everything on the year clock, so storms last sim-seconds of a day. |
| `WX-D3` | Spatial model | **(A) Recommended:** one regional state × a static orographic map, plus `K ≤ 8` moving storm discs. (B) A full `256²` stochastic field each block. |
| `WX-D4` | Orographic map source | **(A) Recommended:** a new offline `256²` sidecar of worldgen `precip_m_yr`, hash pinned in the law, artifact untouched. (B) The artifact's `moisture` as a proxy, with no new file. |
| `WX-D5` | Tropical storms | **(A) Recommended:** off by default, available as a declared `CauseId` event or at an honest climatological rate. (B) On at showcase frequency. |
| `WX-D6` | Creature effects, first cut | **(A) Recommended:** thermal and exposure costs through `total_cost` only, with no new death cause and no new sense. (B) Also direct storm mortality. (C) Also reduced visibility. |
| `WX-D7` | Lightning-to-fire | **(A) Recommended:** WX draws strikes on its own stream, and `W3` ignites deterministically from fuel dryness. (B) `W3` samples ignitions from a lightning climatology. |
| `WX-D8` | Rendering authority | **(A) Recommended:** simulation-driven whenever a live world is attached; the manual selector only for showcase, capture or a labelled preview. (B) Manual always. |
| `WX-D9` | RNG slot registry | **(A) Recommended:** one registry table in `WX-0` that reconciles the colliding proposals (6, 7, 8) before any new stream lands. (B) Assign slots card by card. |

## References

- Albrecht, R. I., et al. (2016). Where are the lightning hotspots on Earth? *BAMS* 97, 2051–2068. doi:10.1175/BAMS-D-14-00193.1
- Amador, J. A. (2008). The Intra-Americas Sea low-level jet. *Ann. N.Y. Acad. Sci.* 1146, 153–188. doi:10.1196/annals.1446.012
- Christian, H. J., et al. (2003). Global frequency and distribution of lightning as observed from space by the OTD. *JGR* 108. doi:10.1029/2002JD002347
- Donihue, C. M., et al. (2018). Hurricane-induced selection on the morphology of an island lizard. *Nature* 560, 88–91. doi:10.1038/s41586-018-0352-3
- GCOS/WMO. Essential Climate Variables. https://gcos.wmo.int/site/global-climate-observing-system-gcos/essential-climate-variables (opened 2026-11-26)
- Grimm, V., et al. (2005). Pattern-oriented modeling of agent-based complex systems. *Science* 310, 987–991. doi:10.1126/science.1116681
- Hoyos, N., et al. (2019). Modeling streamflow response to persistent drought … Sierra Nevada de Santa Marta. *Water* 11(1), 94. doi:10.3390/w11010094
- Huey, R. B., & Kingsolver, J. G. (1989). Evolution of thermal sensitivity of ectotherm performance. *TREE* 4, 131–135. doi:10.1016/0169-5347(89)90211-5
- Lugo, A. E. (2008). Visible and invisible effects of hurricanes on forest ecosystems. *Austral Ecology* 33, 368–398. doi:10.1111/j.1442-9993.2008.01894.x
- Ortiz-Royero, J. C. (2012). Exposure of the Colombian Caribbean coast, including San Andrés Island, to tropical storms and hurricanes, 1900–2010. *Natural Hazards* 61, 815–827. doi:10.1007/s11069-011-0069-1
- Rey, W., et al. (2021). Hurricane flood hazard assessment for the Archipelago of San Andres, Providencia and Santa Catalina. *Front. Mar. Sci.* 8, 766258. doi:10.3389/fmars.2021.766258
- Richardson, C. W. (1981). Stochastic simulation of daily precipitation, temperature, and solar radiation. *Water Resour. Res.* 17, 182–190. doi:10.1029/WR017i001p00182
- Roe, G. H. (2005). Orographic precipitation. *Annu. Rev. Earth Planet. Sci.* 33, 645–671. doi:10.1146/annurev.earth.33.092203.122541
- Shamoun-Baranes, J., et al. (2017). Atmospheric conditions create freeways, detours and tailbacks for migrating birds. *J. Comp. Physiol. A* 203, 509–529. doi:10.1007/s00359-017-1181-9
- Smith, R. B., & Barstad, I. (2004). A linear theory of orographic precipitation. *J. Atmos. Sci.* 61, 1377–1391. doi:10.1175/1520-0469(2004)061<1377:ALTOOP>2.0.CO;2
- Thonicke, K., et al. (2010). The influence of vegetation, fire spread and fire behaviour on biomass burning and trace gas emissions: results from a process-based model. *Biogeosciences* 7, 1991–2011. doi:10.5194/bg-7-1991-2010
- Wilks, D. S., & Wilby, R. L. (1999). The weather generation game: a review of stochastic weather models. *Prog. Phys. Geogr.* 23, 329–357. doi:10.1177/030913339902300302
