---
phase: design
feature: fungi-microbes
title: Design — Fungi and free-living microbes as soil fields (MICRO-D, proposed)
description: Proposed design for the fungal and microbial soil ecosystem in the live world. Bacteria and saprotrophic fungi are per-cell biomass fields on the 256² sim grid (ADR-0018 E5) that decompose a new per-cell litter field. The design gives temperature (Q10) and moisture-optimum rates, a fungal-to-bacterial balance that emerges, and stoichiometric net N mineralization in the DynamicFields nutrient ledger. Every EU flow is named and audited, with respiration to Detritus or to declared heat loss under QD-BIO-D1. It also covers mycorrhizal exchange with ADR-0015 plants, mushrooms as food, interfaces with INVERT-D, FOSS-D, WX-D, PATHO-D and the ADR-0016 hazards, field-driven mushrooms, and an opt-in law that defaults off.
status: proposed
owner: lead
last_reviewed: 2026-11-26
---

# Design — Fungi and free-living microbes as soil fields (MICRO-D)

> **Status: `proposed`. Design only; no code is written under this card.**
>
> The owner asked on 2026-11-26 to "thêm cả hệ sinh thái nấm và vi sinh vật nữa" (add the fungal and
> microbial ecosystem too). Code anchors are verified on `origin/feat/ecological-selection` at
> `dcaf6e7a`.
>
> **Neighbours and their status:**
>
> - The gap note (PR #303, open) ranks "decomposers and the soil cycle (V4, V5, E5)" P1.
> - INVERT-D (#299, proposed) owns the shredders, the soil fauna. This design owns the decomposers
>   and mineralisers, and sits beside INVERT-D rather than repeating it.
> - PATHO-D (#306, proposed) owns every pathogen, including pathogenic fungi such as chytrid.
>   ADR-0015 owns plant disease.
> - FOSS-D (#302) owns burrows. WX-D (#304) owns rain and drought.
> - QD-PLANT (#305, open) decides ADR-0015's gates.
>
> The owner decisions are in §12 and the cards in §11.

## 1. What the code has today (`dcaf6e7a`)

| Fact | Anchor |
|---|---|
| No microbe or fungus is modelled. | grep over `src-tauri/` |
| **Detritus is one global number**, the pool regrowth draws on. Dead organic matter and the "available" budget are the same thing, and there is no per-cell litter. So no decomposer can be spatial today, and none can be rate-limiting. | `energy.rs:19-21`; `environmental_systems.rs:72-85` (`pool.detritus -= grown`, `:83`) |
| Respiration and deaths go straight into that pool. | `world_systems.rs:223`; `agent_systems.rs:170` |
| `DynamicFields` holds per-cell `soil_moisture`, `soil_nitrogen` and `soil_organic`, in NM, under a nutrient ledger (`cum_nutrient_added`, `_leached`, `_uptake`; residual check). It is **not in the live schedule**; V1 wires it in. | `dynamic_fields.rs:86-90`, `:104-106`, `:461`; `tick_capture.rs:176-180` |
| Mineralization is a fixed 1% of `soil_organic` per soil step, with no temperature, moisture or organism behind it. | `SOIL_MINERALIZE`, `dynamic_fields.rs:45`; `step_soil` `:413-425` |
| The rate band for "plant growth, decomposition" already exists. | `PLANT_PERIOD = 300` (0.2 Hz), `sim_clock.rs:21` |
| V5 (proposed) makes grass take N from `soil_nitrogen` through `cum_nutrient_uptake`. V4 (proposed) returns carcass N to `soil_organic` as a declared source (`CARCASS_EU_TO_ORGANIC_NM`). | [V5 spec](../planning/2026-11-24-v5-fields-plants-feedback-spec.md) §2; [V4 spec](../planning/2026-11-24-v4-carcasses-decomposition-spec.md) §2 item 6 |
| ADR-0018 (proposed; pillars decided under QD-ECOSCALE) puts "bacterial/fungal decomposers" in Tier-3 population fields. Under QD-BIO-D1's opt-in open flow, respired EU leaves as declared `RespiredHeatLoss`. Card E5 is the field card. | [ADR-0018](../../decisions/ADR-0018-ecosystem-scales-and-cycles.md) §2.3, §3.2; [ecosystem cards](../planning/2026-11-24-ecosystem-cards.md) E5 |
| ADR-0015's grass traits (QD-PLANT D-PLANT-EVO-3) are growth rate, drought tolerance, defence, palatability and seed dispersal. It has `root_depth` and `nutrient_efficiency`, but **no mycorrhizal trait**. Dead plants go to detritus or V4/V5 decay. | [ADR-0015](../../decisions/ADR-0015-evolving-plants.md) trait table, §"Senescent / Dead" |
| Temperature: on a World v2 sim world, `T_c = −20 + 60·t`; INVERT-D §2 gives the anchors. | `anmw_v3.rs:140-143` |

## 2. Decision summary

1. **Microbes are Tier-3 fields** (ADR-0018 E5), never agents and never genomes.
2. **Minimal set, now:**
   - two decomposer pools, **bacteria** and **saprotrophic fungi**;
   - a two-class **litter** field, fast (leaves, frass, carcass remains) and slow (wood, recalcitrant
     material), which supplies the slow stage decomposition needs.

   **Later:** mycorrhizal fungi, once ADR-0015 lands, and nitrogen fixation as a declared N source.
3. **Decomposition gates energy.** Dead matter enters per-cell litter. Only microbial uptake and
   respiration move it on: to `Detritus` in closed mode, to `HeatLoss` under open flow. Net N
   mineralization follows from stoichiometry instead of a fixed 1%.
4. **Opt-in law**, `WorldLawSet::soil_biota`, plus `live.soil_biota`. Absent means bit-identical.
5. **Evolution is honest.** At this scale a field cannot hold microbial genetics. What can change is
   community composition (the fungal-to-bacterial balance) and, optionally, community-weighted trait
   means. Real heritable evolution sits on the plant side: mycorrhizal dependence.

## 3. Pools and state

| Pool (per cell) | Unit | What it stands for | Fed by | Drained by |
|---|---|---|---|---|
| `litter_fast` | EU | leaf litter, frass, carcass remains, microbial necromass | V4 carcass decay; ADR-0015 grass senescence and V5 wilt; INVERT-D frass and mortality | bacteria and fungi; INVERT-D detritivores |
| `litter_slow` | EU | dead wood and recalcitrant litter | ADR-0015 dead trees and shrubs | fungi, mostly; bacteria only weakly |
| `bacteria` | EU | free-living heterotrophic bacteria | uptake of `litter_fast` | respiration, turnover to `litter_fast`, grazing by soil fauna |
| `fungi` | EU | saprotrophic hyphae, including a fruiting share (§7) | uptake of both litter classes | respiration, turnover, grazing, mycophagy |
| `mycorrhiza` (later) | EU | AM and ECM fungi paid by plants | plant carbon (§6) | respiration, turnover |

**Memory:** four pools, plus the later fifth and a fruiting fraction, at 65,536 cells × 4 B is about
1.5 MiB. It is allocated once at genesis when the law is on, never when it is off.

**Update:** one system on the `PLANT_PERIOD` band, strided like `ResourceField`, with the stride phase
stored on the field. It runs after the V4 carcass step and before regrowth, which is where V4 already
places decomposition. It is allocation-free and runs in a fixed order.

The CENTURY model's split into metabolic and structural litter (Parton et al. 1987) is the precedent
for the two litter classes. Keeping only two is the minimal form.

## 4. Decomposition and the nutrient cycle

**Rates.** For each microbial pool and litter class:

```text
uptake = k_max · Q10^((T_soil − T_ref)/10) · f_W(θ) · f_O2 · B_microbe · L / (K + L)
```

- **Temperature.** Q10 near 2 is a common default for soil respiration. Real values vary with
  temperature, substrate and acclimation (Lloyd & Taylor 1994; Davidson & Janssens 2006). This
  world's value is chưa xác minh. `T_soil` is the cell's air temperature passed through a declared lag.
- **Moisture.** `f_W` peaks at intermediate wetness. Aerobic microbial activity is highest at
  roughly 60% water-filled pore space and falls when soil is drier or waterlogged (Linn & Doran 1984;
  Moyano et al. 2013). `θ` is `DynamicFields::soil_moisture`, so the law needs V1.
- **Fungi versus bacteria.** Fungi tolerate drought better than bacteria (Manzoni et al. 2012
  *Ecology*), and they alone degrade wood efficiently. So `f_W` and access to `litter_slow` differ
  between the two pools.
- **Oxygen.** `f_O2` falls under waterlogging and rises with FOSS-D burrow aeration.

**Fungal-to-bacterial balance.** It emerges from competition for litter under these different
responses. It is never set. Fungal-dominated soils cycle carbon and N slowly and conservatively;
bacterial-dominated soils cycle fast (Wardle et al. 2004; Strickland & Rousk 2010). Here that follows
from the pools' parameters:

| | Bacteria | Fungi |
|---|---|---|
| Carbon-use efficiency (CUE) | lower | higher |
| C:N ratio | narrower | wider |
| Turnover | faster | slower |

All three parameter pairs span wide ranges in the literature, chưa xác minh for this world
(Six et al. 2006; Strickland & Rousk 2010).

**Net N mineralization** (Manzoni & Porporato 2009). When a pool decomposes `U` EU of litter with
ratio `CN_L`, keeps `CUE·U` as biomass with ratio `CN_M`, and respires the rest:

```text
N_net = U · (1/CN_L − CUE/CN_M)
```

- **Positive:** `soil_organic → soil_nitrogen` (mineralization).
- **Negative:** `soil_nitrogen → soil_organic` (immobilization), capped by the mineral N present.

The transfer stays inside the existing NM ledger and replaces the fixed `SOIL_MINERALIZE` step when
the law is on. Wood (high `CN_L`) therefore immobilizes N while it decays, and fresh frass releases
it. All C:N values are chưa xác minh.

**Drought and rewetting.** Microbes that die in drought become necromass in `litter_fast`. When the
soil is rewetted, fast decomposition of that necromass gives a respiration and N pulse. This is the
Birch effect (Birch 1958): it must emerge, and it is tested (MI-S06).

## 5. Energy flows inside the closed EU

**Compartments.** The litter and microbial pools hold EU, so they must enter the census.

- `Detritus` is regrowth's free budget, so they cannot be folded into it. If they were, regrowth
  would eat living microbes and undecomposed wood.
- They cannot go into `Animals` either, because the census overwrites it.

INVERT-D already proposes a fifth compartment for its fields (IN-D3). MI-D2 proposes one mechanism for
every Tier-3 animal and microbe field: ADR-0018's `E_fields` term as one `Compartment::Fields`, with
named sub-ledgers (`invertebrates`, `litter`, `microbes`), each reported separately.

| # | Flow | From → to | New `EnergyEvent` | Audit |
|---|---|---|---|---|
| M1 | Litter input: plant death, wilt, frass, carcass remains | `Plants`, `Invertebrates`, V4 `Carcasses` → `Fields.litter` | `LitterFall` | measured `f32` delta per cell; replaces today's direct `→ Detritus` for those sources only when the law is on |
| M2 | Microbial uptake | `Fields.litter` → `Fields.microbes` | `MicrobialUptake` | measured delta; split into kept (`CUE·U`) and respired |
| M3 | Microbial respiration | `Fields.microbes` → `Detritus` (closed); `HeatLoss` × `heat_loss_fraction` under QD-BIO-D1 | `MicrobialRespiration` | one per-cell accumulator, one transfer per step |
| M4 | Turnover (death) | `Fields.microbes` → `Fields.litter` (fast) | `MicrobialTurnover` | internal to `Fields`; still logged |
| M5 | Grazing on microbes by soil fauna | `Fields.microbes` → `Invertebrates` | INVERT-D's `Detritivory` | INVERT-D `take`, which reads this pool when both laws are on |
| M6 | Mycophagy (mushrooms) | `Fields.microbes` (fruiting share) → agent reserve; unassimilated → `Detritus` | `Mycophagy` | `transfer_into_reserve` (`energy.rs:233`) |
| M7 | Mycorrhizal payment (later) | `Plants` → `Fields.mycorrhiza` | `MycorrhizalCarbon` | measured debit of plant biomass, inside plant `total_cost` per ADR-0015 |
| M8 | Fire combustion of litter | `Fields.litter` → `Detritus` (closed) or `HeatLoss` (open), as ADR-0016 books burnt plants | ADR-0016's event | W3 |

**Identity with the law on.** The sum of plants, animals, detritus, dormant, (carcasses) and fields
equals the baseline within `RESIDUAL_ABS_TOLERANCE_EU = 1e-3` in closed mode, or matches
`R_open` under open flow. Every transfer uses `EnergyLedger` and the measured-delta pattern, never a
raw `pool.x +=`.

**What changes in closed mode.** Dead matter no longer feeds regrowth at once: it waits in litter
until microbes respire it into `Detritus`. Decomposition becomes the rate-limiting step it is in
nature, and warm, moist cells recycle faster than cold or dry ones. Under open flow, microbes are
where most dead-matter EU leaves as heat, and their role in regrowth is the N they release (§4).

**N in living biomass.** Biomass carries no N today: V5 books uptake as a sink, V4 books carcass
return as a source. MI-D4 keeps that pair for now and adds a derived observable,
`n_in_biomass = cum_uptake − cum_returned`, so the cycle can be audited without new state.

## 6. Mycorrhizal symbiosis with plants (after ADR-0015)

About 8% of vascular plants have no mycorrhizal partner at all (Brundrett & Tedersoo 2018, cited in
the gap note). In the exchange, plants pay carbon (often quoted as 4–20% of net photosynthate, Smith &
Read 2008; to re-check) for nutrients and water.

**The sim has no phosphorus** (the gap note marks it P3), and phosphorus is the main currency of
arbuscular mycorrhiza. The exchange here is therefore **N and water for EU**, and is labelled a
declared approximation.

- **Plant side (heritable).**
  - A `mycorrhizal_dependence` trait, added to ADR-0015's tree and shrub genotype and as one more
    quantized grass-cell trait beside D-PLANT-EVO-3's five (MI-D5).
  - The plant pays `dependence × cost` of its growth into `mycorrhiza` (M7) inside its `total_cost`.
  - In return it gets a higher effective `f_n` and `f_w` (V5's limiters), scaled by
    `min(dependence, mycorrhiza biomass)`.
- **Fungal side.** One per-cell pool, no traits.
- **What should emerge** is the mutualism–parasitism continuum (Johnson et al. 1997): where mineral N
  and water are plentiful, the payment buys nothing, and dependence should fall; where they are poor,
  it should rise. That is a testable cline (MI-S07). No plant is told to be mycorrhizal.

## 7. Fungi as food and as habitat

- **Mushrooms.**
  - A fruiting share of `fungi` builds up after a wet spell: rain from WX-D raises `θ` past a
    threshold. It decays in dry weather.
  - Agents eat it through `feed_intent` (M6) when they stand in the cell.
  - INVERT-D soil fauna grazes hyphae (M5). Springtails and mites are mostly fungal and microbial
    grazers, which is why M5 rather than litter is their main food when both laws are on.
- **Dead wood and litter as habitat.** `litter_slow` marks dead-wood cover, and `litter_fast` marks
  the litter layer. INVERT-D detritivore carrying capacity and FOSS-D digging hardness may read them
  as cover. A cell's dead-wood cover also feeds the decaying-log visuals (§9).

## 8. Interactions

- **Fire** (ADR-0016 W3, QD-WCHG).
  - Litter burns (M8).
  - The microbial pools in burnt cells lose a declared fraction to `Detritus`. Severe fire sterilises
    the top layer; light fire only thins it (Certini 2005; fractions chưa xác minh).
  - Part of the organic N volatilises as a declared NM sink, and an ash pulse of mineral N follows.
  - Recovery comes from survivors and from diffusion out of neighbouring cells. It is never scripted.
- **Ash fall** buries `litter_fast` for a declared time and slows uptake.
- **Flood** lowers `f_O2`, so decomposition slows while waterlogged.
- **Drought and the wet season** (WX-D). `θ` follows WX-D rain through the conserved water budget,
  and `drought_index` lengthens dry spells. Fungi gain share in drought. Rewetting gives the Birch
  pulse.
- **Burrows** (FOSS-D). `burrow_extent` raises `f_O2` and mixes surface litter downward, which speeds
  decomposition. FOSS-D's aeration multiplier then acts through this field.
- **INVERT-D.** With both laws on, shredders fragment litter: detritivores eat `litter_slow` and
  return frass to `litter_fast`, and they graze microbes (M5). This replaces INVERT-D's N1 and N2
  speed-up multipliers by a mechanism. With INVERT-D alone, N1 and N2 stand.
- **RESP-D.** Microbial respiration is the main source of soil CO2. An interface
  `soil_respiration(cell)` lets RESP-D and FOSS-D raise burrow and cave CO2 from it (rates chưa xác
  minh).
- **PATHO-D.** Pathogenic microbes and fungi, chytrid included, are PATHO-D's. MICRO-D only exposes
  soil moisture, temperature and `soil_respiration` for its environmental load field.

## 9. Evolution, rendering, performance and time

**What evolves.**

- Mycorrhizal dependence in plants (§6), as a genuine heritable trait.
- The fungal-to-bacterial balance, which is community change between two pools, not genetics.
- Optionally (MI-D6), community-weighted trait means per cell: a thermal optimum per microbial pool,
  moved by the same moment update as INVERT-D IN-D5. This represents thermal adaptation of soil
  respiration as a community shift (Bradford et al. 2008). It is labelled "community trait", never a
  lineage.

**What does not evolve:** microbial species, horizontal gene transfer, any per-microbe genome, and
pathogen evolution (PATHO-D's). The field is deterministic, so nothing drifts.

**Rendering.**

- Mushrooms appear on forest and grass cells near the camera after rain. Their density comes from the
  fruiting share. One instanced mesh: +1 draw call on low.
- Decaying logs are ADR-0015's dead-tree instances, tinted by their cell's `litter_slow` decay state.
  That adds no draw call; until ADR-0015 lands there are no logs.
- The total stays within 60 on low. With the law off, nothing is drawn.

**Performance.** Each visited cell does a few multiply-adds per pool on a 0.2 Hz band with a stride,
far below the per-tick `ResourceField` step. The cost is not measured yet and is to be added to
ADR-0018 E1's bench.

**Time honesty.** Tropical leaf litter decays in months and wood in years (Aerts 1997; Parton et al.
2007). Both are compressed with the sim year, which is 100 s by default and 90 min on shared islands
in ADR-0016's quote. The design keeps the order of the rates (leaf faster than wood, wet and warm
faster than dry and cold), not the durations.

## 10. Science safety

- **Law.** `WorldLawSet::soil_biota: Option<SoilBiotaLaw>` emits zero canonical bytes when `None`
  (the ADR-0016 §5 pattern). `live.soil_biota` absent or `0.0` must pass
  `validate_post_freeze_keys_default_to_legacy` (`prereg_sel2p2_manifest_tests.rs:422`). Preflight
  refuses the law without V1's `DynamicFields`.
- **Off means absent.**
  - No field is allocated.
  - Deaths and respiration go to `Detritus` exactly as today.
  - `step_soil` keeps `SOIL_MINERALIZE`.
  - `world_checksum` hashes the field only when it exists.
  - No RNG is drawn.
- **RNG.** The field is deterministic. If a later card adds a draw, its stream number comes from the
  single registry card of the lead's ruling **LR-RNG-1**. This design claims no slot.

| Gate | Positive check | Negative control |
|---|---|---|
| MI-S01 default-off identity | the frozen SEL2-P2 manifests, every `prereg_*` test and the E2/E3/E3b checksums are bit-identical; fingerprint unchanged | a mutant that allocates the field, routes one death through `LitterFall`, or emits a tag byte when off must turn red |
| MI-S02 EU closure | closed `\|R\| ≤ 1e-3` (or `\|R_open\| ≤ 1e-3`) over ≥ 100,000 ticks | a mutant that credits `microbes` from litter without debiting litter, or respires outside M3, must exceed the tolerance |
| MI-S03 N closure | `nutrient_budget_residual / scale < 1e-4` with mineralization and immobilization | a mutant that moves N to `soil_nitrogen` without debiting `soil_organic` must fail |
| MI-S04 zero-alloc and replay | `TrackingAllocator == 0`; three-process same-OS agreement; `run N == run K → save → load → run N−K` | a save that drops the stride phase must break replay |
| MI-S05 climate response | warm, moist cells decompose faster; a drought raises the fungal share | with `Q10 = 1` and flat `f_W`, both patterns must vanish |
| MI-S06 Birch pulse | rewetting after drought raises respiration and mineral N above a never-dry control | with turnover off, there is no pulse |
| MI-S07 mycorrhizal cline (after ADR-0015) | dependence falls with soil N and moisture across cells | with the payment's benefit set to 0, dependence falls everywhere |

## 11. Cards

| Card | Size | Content | Waits for |
|---|---|---|---|
| MI-1 | M | law, live key, `Fields` sub-ledgers, events, census; MI-S01 | this design and ADR-0018 accepted; V1 |
| MI-2 | L | litter and microbial pools, rates (Q10, `f_W`, `f_O2`), stride and save; flows M1–M4; MI-S02, MI-S04 | MI-1; V4 for carcass input |
| MI-3 | M | stoichiometric net mineralization replacing `SOIL_MINERALIZE` when on; MI-S03 | MI-2; V5 |
| MI-4 | S | interfaces: INVERT-D coupling (M5, fragmentation), FOSS-D aeration, RESP-D `soil_respiration`, PATHO-D drivers | MI-2; the neighbour cards |
| MI-5 | M | hazards: fire (M8, sterilisation, N loss), ash, flood; drought and rewetting; MI-S05, MI-S06 | MI-3; W3; WX-D |
| MI-6 | S | mushrooms: fruiting share, mycophagy (M6), renderer (+1 draw call) | MI-2 |
| MI-7 | M (later) | mycorrhiza pool and plant trait (M7); MI-S07 | ADR-0015 accepted; its plant cards |
| MI-8 | M (later) | community trait means; N fixation as a declared NM source with an EU cost | MI-3; owner decision |

## 12. Owner decisions

| ID | Question | Options | Recommendation |
|---|---|---|---|
| MI-D1 | Minimal pool set | (A) bacteria, saprotrophic fungi, and fast and slow litter now; mycorrhiza with ADR-0015; N fixers later. (B) all five now. | **A**: mycorrhiza has no plant partner yet, and fixation needs its own source declaration. |
| MI-D2 | Compartment layout | (A) one `Compartment::Fields` (ADR-0018's `E_fields`) with sub-ledgers for invertebrates, litter and microbes; this amends INVERT-D IN-D3 to a sub-ledger. (B) one compartment per field. | **A**: one census term, separate reports. |
| MI-D3 | Litter routing with the law on | (A) deaths, plant litter and frass enter per-cell litter, and only microbes release them to `Detritus`. (B) keep direct `→ Detritus` and only scale rates. | **A**: B leaves decomposers with nothing to limit. |
| MI-D4 | N in biomass | (A) keep V4/V5's declared uptake and return pair, plus a derived `n_in_biomass` observable. (B) track N in every EU pool by fixed ratios now. | **A**, with B as a later card if the cycle needs it. |
| MI-D5 | Mycorrhizal trait | (A) add `mycorrhizal_dependence` to ADR-0015's tree genotype and as a sixth grass-cell trait (an amendment after QD-PLANT). (B) no plant trait; a fixed exchange. | **A**: it is the one place where real heritable evolution happens in this design. |
| MI-D6 | Microbial trait evolution | (A) none in the first slice; the fungal-to-bacterial balance only. (B) community thermal-optimum means per pool now. | **A**, with B in MI-8, labelled "community trait". |
| MI-D7 | Respired heat | (A) closed mode to `Detritus`; open flow by ADR-0018's `heat_loss_fraction` (QD-BIO-D1). (B) a microbe-specific heat fraction. | **A**: one rule for all respiration. |
| MI-D8 | Visible cues | (A) mushrooms after rain, +1 draw call; logs tinted on ADR-0015 dead trees. (B) none. | **A**. |

## 13. Sources

- Aerts, R. (1997). Climate, leaf litter chemistry and leaf litter decomposition in terrestrial ecosystems: a triangular relationship. *Oikos* 79:439–449.
- Birch, H. F. (1958). The effect of soil drying on humus decomposition and nitrogen availability. *Plant and Soil* 10:9–31.
- Bradford, M. A., et al. (2008). Thermal adaptation of soil microbial respiration to elevated temperature. *Ecol. Lett.* 11:1316–1327.
- Brundrett, M. C., Tedersoo, L. (2018). Evolutionary history of mycorrhizal symbioses and global host plant diversity. *New Phytol.* 220:1108–1115.
- Certini, G. (2005). Effects of fire on properties of forest soils: a review. *Oecologia* 143:1–10.
- Davidson, E. A., Janssens, I. A. (2006). Temperature sensitivity of soil carbon decomposition and feedbacks to climate change. *Nature* 440:165–173.
- Johnson, N. C., Graham, J. H., Smith, F. A. (1997). Functioning of mycorrhizal associations along the mutualism–parasitism continuum. *New Phytol.* 135:575–585.
- Linn, D. M., Doran, J. W. (1984). Effect of water-filled pore space on carbon dioxide and nitrous oxide production in tilled and nontilled soils. *Soil Sci. Soc. Am. J.* 48:1267–1272.
- Lloyd, J., Taylor, J. A. (1994). On the temperature dependence of soil respiration. *Funct. Ecol.* 8:315–323.
- Manzoni, S., Porporato, A. (2009). Soil carbon and nitrogen mineralization: theory and models across scales. *Soil Biol. Biochem.* 41:1355–1379.
- Manzoni, S., Schimel, J. P., Porporato, A. (2012). Responses of soil microbial communities to water stress: results from a meta-analysis. *Ecology* 93:930–938.
- Moyano, F. E., Manzoni, S., Chenu, C. (2013). Responses of soil heterotrophic respiration to moisture availability: an exploration of processes and models. *Soil Biol. Biochem.* 59:72–85.
- Parton, W. J., Schimel, D. S., Cole, C. V., Ojima, D. S. (1987). Analysis of factors controlling soil organic matter levels in Great Plains grasslands. *Soil Sci. Soc. Am. J.* 51:1173–1179.
- Parton, W., et al. (2007). Global-scale similarities in nitrogen release patterns during long-term decomposition. *Science* 315:361–364.
- Six, J., Frey, S. D., Thiet, R. K., Batten, K. M. (2006). Bacterial and fungal contributions to carbon sequestration in agroecosystems. *Soil Sci. Soc. Am. J.* 70:555–569.
- Smith, S. E., Read, D. J. (2008). *Mycorrhizal Symbiosis*, 3rd ed. Academic Press.
- Strickland, M. S., Rousk, J. (2010). Considering fungal:bacterial dominance in soils — methods, controls, and ecosystem implications. *Soil Biol. Biochem.* 42:1385–1395.
- Wardle, D. A., et al. (2004). Ecological linkages between aboveground and belowground biota. *Science* 304:1629–1633.

Bibliographic details are from memory, except Brundrett & Tedersoo, which is taken from the gap
note. Check them against the publisher before any number here is used for calibration.
