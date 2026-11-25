---
phase: design
feature: subterranean-life
title: Design — Subterranean and cave life (FOSS-D, proposed)
description: Proposed design for burrowing and cave-dwelling creatures in the live world. Digging is a heritable capacity whose cost per metre dwarfs walking, so burrows become capital. Burrows are a Tier-2 overlay field on the 256² sim grid that gives refuge and carries flood and gas risks. Being underground is an agent state with its own senses. Diets come from INVERT-D soil fauna and a declared below-ground share of grass. The design uses the World v2 cave graph and treats eye and pigment regression as an emergent, testable signature. It also covers respiration through RESP-D, rendering, and an opt-in default-off law, and it builds on the locomotion media framework of RESP-D (in progress).
status: proposed
owner: lead
last_reviewed: 2026-11-26
---

# Design — Subterranean and cave life (FOSS-D)

> **Status: `proposed`. Design only; no code is written under this card.** On 2026-11-26 the owner
> asked for "sinh vật ở dưới đất nx như chuột chũi, rắn đào hang" (underground creatures such as
> moles and burrowing snakes). Code anchors are verified on `origin/feat/ecological-selection` at
> `38ea4621`.
>
> - Under **QD-REALMS**, underground is the third realm, after water (AQUA-D) and sky (AERO-D,
>   PR #300), each with its own ADR. This design is the input to that ADR. It does not authorize
>   earlier work.
> - The locomotion media framework is defined in RESP-D (in progress); §3 lists the minimum assumed.
> - Soil fauna comes from INVERT-D (PR #299).
>
> The owner decisions are in §12 and the cards in §11.

## 1. What exists today (`38ea4621`)

| Fact | Anchor |
|---|---|
| Nothing digs, burrows or lives below the surface. Physics is planar with no gravity, and legality is a biome test. | `dynamics.rs:121-215`; `terrain_navigation.rs:31-33`, `:69` |
| Caves are **data-only**. The generator builds 30–48 karst (lithology 2) and basalt (lithology 3) cave systems as a graph: nodes carry position, radius and `light_decay`; edges carry passage kind; entrances are bound to nodes. Every entrance is `realm_status = 1`, "decorative until the underground ADR". | `caves.rs:1-3`, `:84-123`, `:605`; `2026-11-20-world-v2-design.md:268-276` |
| `cave_network.bin` ships in the viewer's build-time pack. The sim artifact carries cave entrances. But `terrain_map_from_v3_sim` copies **neither caves nor lithology** into the live `TerrainMap`. | `2026-11-20-world-v2-design.md:333`; `anmw_v3.rs:91-99`, `:123`; `world_artifact.rs:590-605` |
| Lithology has six classes with erodibility 0.15–1.20, but it exists only in the generator. The sim artifact has no lithology channel. | `tectonic.rs:15-30`; `anmw_v3.rs:106-124` |
| `WorldCaves` draws decorative cave mouths. | `WorldCaves.tsx:7`; `simEnvironment.ts:17` |
| Soil moisture, nitrogen and organic matter live in `DynamicFields`, which is **not in the live schedule** (V1 wires it). Its `root_cover` is a static per-biome erosion factor, not roots. | `dynamic_fields.rs:86-90`, `:112-125`; `tick_capture.rs:176-180` |
| There is no root state. ADR-0015 (proposed) adds a `root_depth` trait. | ADR-0015 trait table |
| Brain senses: one forward ray up to 10 units (distance, then food, predator or prey), two olfactory readings from the pheromone grid, and the internal state. There is no touch or vibration channel. | `agent_systems.rs:1777-1849` |
| There is no eye trait and **no pigment field**: renderers colour by predator or prey. Contract D08 fixes colour at birth; D09 makes legacy pigment `None`. | `CREATURE_MORPHOGENESIS.md:168-174`; contract D08, D09 |
| ADR-0016 lists "animal engineering (burrows/dams)" as a Tier-2 disturbance. QD-WCHG (PR #296, open) decided D-WCHG-1 (A): a compact `SurfaceDeltaOverlay` on the 256² grid. | ADR-0016 §3, Tier-2 row |
| Every energy charge goes into `total_cost`. | `world_systems.rs:157` |

The research review [QD_REALMS_HABITATS_RESEARCH](../../research/QD_REALMS_HABITATS_RESEARCH_2026-11-22.md)
§3–§4 keeps two ideas: a cave food web fed by detritus seeping from the surface cell above, and an
eye-maintenance term in `total_cost` so that selection can prune eyes in darkness. It rejects octree
routing and climbing on walls and ceilings for this engine.

## 2. Decision summary

1. **Digging is a heritable capacity** in RESP-D's framework. Its cost per metre is orders of magnitude
   above walking, so a burrow is capital: dug once, used cheaply.
2. **Burrows are a field, not geometry:** one `burrow_extent` channel on the 256² Tier-2 overlay.
   Molehills are drawn from it; no tunnel mesh exists.
3. **Underground is an agent state** (`Underground` in a burrowed cell, `InCave` at a cave-graph
   node) with changed senses and changed reach. It is not free 3-D movement.
4. **Body form, eyes and pigment are never scripted.** Elongation and limb reduction lower digging
   cost through the existing segment graph. Eye loss and pigment loss must emerge, and are tested
   (§8).
5. **Opt-in law** `WorldLawSet::subterranean` with `live.subterranean`. Absent means bit-identical.

## 3. Minimum assumed from RESP-D's locomotion media framework (in progress)

The same five points as AERO-D §3:

1. heritable continuous capacities developed at birth and stored in the phenotype (D01, D02, D04);
2. one legality function comparing the creature's traits with the cell, never inferring a medium
   from the cell;
3. one chosen medium state through a brain gate, with absence meaning legacy;
4. one cost path in `total_cost`;
5. restore and migration carry phenotype, state and brain.

FOSS-D adds `dig` to the capacity set, `Underground` and `InCave` to the state set, and needs RESP-D's
respiration model for burrow and cave air (§9). Any new sense shares RESP-D's single
`BRAIN_GENOTYPE_VERSION` bump.

## 4. Digging as a heritable capacity

**Genes.**

| Gene | Meaning |
|---|---|
| `dig_muscle_fraction` | share of body mass invested in digging muscle, taken from the other segments' mass |
| `eye` | investment in the visual sense: ray range and reliability, against a maintenance cost |

Body shape needs **no new gene**: elongation is chain length over the largest segment radius, and
limb reduction is the number and size of lateral branches in the existing segment graph
(`genotype.rs:5-24`). Legacy genomes read `dig_muscle_fraction = 0` and a full-strength `eye` by
default, so old creatures are today's.

**Developed at birth (once):**

- frontal cross-section `A_f`, from the largest radius;
- lateral drag `A_l`, from the protruding side segments;
- digging power `P_dig`, from the muscle mass;
- capacity `dig = P_dig / (A_f + c·A_l)`, normalized.

An elongate, narrow, limb-reduced body therefore digs more cheaply. That is the shape moles,
caecilians and burrowing snakes converged on: snake-like, limb-reduced bodies evolved many times in
squamates (Wiens et al. 2006), and subterranean mammals converge in form and physiology (Nevo 1979).
Nothing assigns that shape. It wins only where digging pays.

**Dig rate and substrate.** The rate is `dig × f(H)`, where `H` is a cell hardness:

- **Lithology class.** Rock classes are hard; alluvium and shale are soft. Rock outcrop and snow are
  undiggable. Today the live world has no lithology (§1), so the first slice uses a declared proxy
  from biome and slope (FO-D3).
- **Soil moisture.** Moist soil is softer, up to saturation. A saturated burrow floods (§5). This needs
  V1. Without V1 the static `TerrainMap::moistures` is used.

All hardness values are chưa xác minh.

**Cost.** For the pocket gopher, digging a tunnel costs roughly 360–3,400 times as much as walking
the same distance on the surface (Vleck 1979; range to re-check). The design keeps the *order of
magnitude* and charges three terms, all inside `total_cost`:

- digging, per unit of new burrow;
- moving inside an existing burrow, which is cheap;
- muscle overhead.

## 5. Burrows as world features

**Representation.** `burrow_extent[i] ∈ [0, 1]` is a new channel of the Tier-2
`SurfaceDeltaOverlay` (256 KiB at 256² × `f32`), present only when the law is on. It counts the
burrowed share of a cell, because one 320 m cell is far larger than any burrow system and creature
scale is stylised anyway (LD-1). Digging raises it; it decays slowly back toward zero through
collapse and infill (rate chưa xác minh), and faster when a flood fills it. It holds **no EU**: a
burrow is a physical state. The energy spent digging was respired through `total_cost` into
detritus like every other charge.

**What burrows give.** `Underground` agents in a burrowed cell are protected from:

- **fire:** the W3 burn timer does not reach them;
- **heat and cold:** burrow temperature is a damped running mean of the cell's temperature, a
  declared lag that is chưa xác minh;
- **surface predators:** `combat_system` gains the same medium check as AERO-D;
- **storms.**

**What burrows risk.**

- **Flooding** (W1 water stage, D-WCHG-3 runup): occupants are submerged and drain their oxygen
  store through RESP-D's drowning path.
- **Gas pooling:** RESP-D's CO2 field pools in hollows, so burrows and cave floors can hold
  suffocating air (§9).

**What burrows change, inside the existing ledgers only:**

- **aeration:** a local infiltration multiplier on `DynamicFields` (`INFIL_RATE`,
  `dynamic_fields.rs:35`), so water still moves only through the conserved water budget;
- **mixing:** a faster organic-to-nitrogen mineralization in burrowed cells, in NM under the nutrient
  ledger, as in INVERT-D's N1.

Both need V1 and are inert without it. This is ecosystem engineering in the sense of Jones et al.
(1994). No EU is created, stored or destroyed by a burrow. Molehills do not touch `delta_elev_m`:
at 320 m a molehill is far below the overlay's resolution, so it is drawn and not simulated.

## 6. Being underground

- **States.** `Surface`, `Underground(cell)` (legal where `burrow_extent > 0`, or while digging), and
  `InCave(node)` (legal at a cave node reached from an entrance, §8). Enter and exit are brain
  choices through RESP-D's medium output.
- **Movement.** Inside a burrowed cell or along cave edges movement is cheap. Extending a burrow into a
  neighbouring cell is digging (§4). The planar position stays the cell's, and nothing moves in 3-D.
- **Senses.**
  - The forward ray is scaled by local light: 0 underground, and the node's `light_decay` in a cave.
    It is also scaled by the `eye` trait, so in darkness a costly eye buys nothing.
  - Smell, the pheromone readings, keeps working.
  - A short-range **touch** input (contact within a small radius) is needed, plus later a
    **vibration** input for surface movement above. These are new inputs, shared with RESP-D's input
    change.
- **Reach.**
  - Surface predators cannot strike `Underground` or `InCave` agents.
  - Diggers can: a predator that is itself `Underground` in the same cell may strike.
  - An underground agent can strike only underground targets.

## 7. Diet

| Food | Source | Dependency |
|---|---|---|
| Soil fauna (earthworms, larvae: the mole diet) | INVERT-D `soil_fauna_eu(cell)` and `ground_prey_eu` via `take` (F6, Holling type III) | INVERT-D IN-5 |
| Roots and tubers | no root state exists; a declared below-ground share of the cell's grass, `ResourceField.r`, reachable only from `Underground`; the same `Plants` pool, so nothing new enters the ledger | today's field; ADR-0015 `root_depth` later |
| Other burrowers | underground predation in the same cell (a burrowing-snake niche) | `combat_system` medium check |
| Cave food | detritivores fed by detritus seeping from the cell above; later bat guano from AERO-D roosts; cave dwellers eat that field | INVERT-D; AERO-D for roosts |

Real cave food webs depend on surface input because there is no light for producers (Culver & Pipan
2009). The design keeps that: no photosynthesis below ground, and no chemosynthetic source.

## 8. Caves

- **Loading the graph.** When the law is on, the sim loads the cave graph as a read-only resource.
  Nodes are mapped to sim cells, and edges become `InCave` moves. Entrances connect a surface cell to
  its node.
- **Identity.** Today the graph is a viewer file under the bundle identity, not the sim identity
  (LD-2). A sim that reads it must pin it. Options are in FO-D4.
- **Light** at a node is `light_decay`, so deep nodes are dark and entrance zones are dim. Twilight
  dwellers and deep troglobites can then separate along a real gradient.

**The evolutionary prediction: emergent, not a rule.** In darkness, eyes and pigment should regress,
as in the cavefish *Astyanax mexicanus* (Jeffery 2009). Its albinism evolved convergently in separate
cave populations (Protas et al. 2006). The engine gives two mechanisms:

1. **Selection by energy economy.** The `eye` trait carries a maintenance term in `total_cost`; vision
   costs cavefish a measurable share of resting metabolism (Moran et al. 2015, figure to re-check). In
   darkness that cost buys nothing, so lower `eye` raises net energy.
2. **Relaxed selection plus mutation.** Where an eye does nothing, mutations that reduce it are not
   purged, and the trait drifts down slowly even at zero cost.

**Tests.**

- Paired seeds compare cave and surface lineages, with surface controls. The prediction is that
  `eye` falls in cave lineages and not on the surface.
- With the eye cost set to 0, regression must be slower: drift only. That separates mechanism 1 from
  mechanism 2.
- **Pigment** needs the planned pigment gene (CM-104; D08, D09). FOSS-D adds no pigment gene and
  consumes it once it exists. With no cost and no function in the dark, only the slow drift
  mechanism applies. Whether pigment synthesis has an energy cost worth modelling is chưa xác minh.
- Local adaptation is claimed only with CM-S11 reciprocal-transplant evidence. A dark-adapted
  lineage is a "cave morph", not a species (ER09).

## 9. Respiration underground

Burrow and cave air can be hypoxic and high in CO2. That goes through RESP-D's single respiration
model: a burrow cell's air is a declared function of occupancy and soil moisture, since wet soil
slows gas exchange (rates chưa xác minh), and CO2 pooled by RESP-D's gas field fills burrows and cave
floors first.

Tolerance is a heritable respiration trait in RESP-D, so hypoxia tolerance can evolve. The naked
mole-rat survives 18 minutes without oxygen (Park et al. 2017), which shows how far subterranean
mammals can go; its numbers are not used as constants here. Suffocated bodies reach detritus through
the existing reclaim path, with the cause logged.

## 10. Rendering, performance and safety

**Rendering.**

- `Underground` and `InCave` agents are not drawn, which makes them cheaper than surface agents on the
  GPU.
- Molehills and burrow entrances are one instanced mesh, placed deterministically from
  `burrow_extent` around the camera: one extra draw call on low.
- `WorldCaves` mouths stay; when the law is on, a mouth can show occupancy.
- A cutaway view is out of scope.

**Budget.** Subterranean animals are ordinary Tier-1 agents. They count against the 1,000-creature
Iris Xe gate and the trial target of about 100 (QD-SCALE-TRIAL). Each adds an O(1) state update and
a medium check in the existing spatial queries: nothing O(N²) and no allocation. The overlay channel
is allocated once, at genesis.

**Science safety.**

- `WorldLawSet::subterranean: Option<SubterraneanLaw>` emits zero canonical bytes when `None`, the
  ADR-0016 §5 pattern.
- `live.subterranean` absent or `0.0` must pass `validate_post_freeze_keys_default_to_legacy`
  (`prereg_sel2p2_manifest_tests.rs:422`).
- With the law off:
  - no overlay channel is allocated, no cave graph is loaded and no medium state is inserted;
  - `combat_system` and `terrain_navigation_system` take today's branch;
  - no RNG is drawn.

| Gate | Positive check | Negative control |
|---|---|---|
| FO-S01 default-off identity | the frozen SEL2-P2 manifests, every `prereg_*` test and the E2/E3/E3b checksums are bit-identical; fingerprint unchanged | a mutant that allocates `burrow_extent` or loads the cave graph when off must turn red |
| FO-S02 one cost path | closed EU within 1e-3 while digging | a mutant that charges digging outside `total_cost`, or stores EU in a burrow, must break the residual |
| FO-S03 D01 restore and migration | `run N == run K → save → load → run N−K`, with the `Underground` and `InCave` state, `burrow_extent`, phenotype and brain carried | a mutant that re-develops dig capacity on restore must fail the phenotype bit-compare |
| FO-S04 D04 | `dig` comes from traits only | a mutant that grants `Underground` because the cell is soft must fail |
| FO-S05 refuge and risk | fire spares `Underground` agents; a flood drowns them through RESP-D | with the burrow channel disabled, both effects disappear |
| FO-S06 eye regression | `eye` falls in cave lineages against surface controls | eye cost 0: slower decline (drift only); surface lineages: no decline |

## 11. Cards

All wait for QD-REALMS (water, then sky, then underground) and for the underground ADR built from
this design.

| Card | Size | Content | Waits for |
|---|---|---|---|
| FO-0 | S | underground-realm ADR from this design | sky realm accepted |
| FO-1 | M | `SubterraneanLaw`, `live.subterranean`, zero-tag fingerprint; FO-S01 | FO-0; RESP-D framework card |
| FO-2 | L | `dig_muscle_fraction` and `eye` genes, development of `A_f`, `A_l`, `dig`; costs in `total_cost`; FO-S02, FO-S03, FO-S04 | FO-1; V0 `develop_at_birth` |
| FO-3 | M | `burrow_extent` overlay channel, `Underground` state, refuge and reach rules; FO-S05 with W1/W3 | FO-2; ADR-0016 W2 |
| FO-4 | M | diets: soil fauna, below-ground grass share, underground predation | FO-3; INVERT-D IN-5 |
| FO-5 | M | senses: light-scaled ray, touch input (shared version bump) | FO-3; RESP-D senses card |
| FO-6 | L | cave graph in the sim, `InCave` movement, light; FO-S06 experiment | FO-5; FO-D4 identity decision |
| FO-7 | S | renderer: hide underground agents, molehill and entrance mesh; map visual check | FO-3 |
| FO-8 | M (later) | aeration and mixing through `DynamicFields`; burrow and cave air through RESP-D | V1; RESP-D gas field |

## 12. Owner decisions

| ID | Question | Options | Recommendation |
|---|---|---|---|
| FO-D1 | Burrow representation | (A) one `burrow_extent` channel on the Tier-2 overlay per sim cell. (B) individual burrow objects with tunnel geometry. | **A**: a 320 m cell cannot resolve tunnels, and the overlay is already decided (D-WCHG-1). |
| FO-D2 | Underground model | (A) agent states `Underground` and `InCave`, with no 3-D movement. (B) a depth coordinate. | **A**. |
| FO-D3 | Substrate hardness | (A) a declared proxy from biome and slope now, then the real lithology channel added to the sim artifact in the 81.92 km bake (WV2-BIG), whose identity changes anyway. (B) wait for lithology. | **A**: the bake is the cheapest moment to add a channel. That needs a word with WB-D6, which keeps the 44-byte header, so the channel would go behind a flag or a version. |
| FO-D4 | Cave graph identity | (A) pin `cave_network.bin` into the sim identity, with the sim world hash covering it, from the next bake on. (B) keep it viewer-only and leave caves decorative. | **A**, when FO-6 starts. |
| FO-D5 | Eye trait and cost | (A) one `eye` investment trait with a maintenance term in `total_cost`, testable with the cost set to 0. (B) no cost; drift only. | **A**: it lets the experiment separate selection from drift. |
| FO-D6 | Roots | (A) a declared below-ground share of the existing grass pool until ADR-0015 roots exist. (B) wait for ADR-0015. | **A**, labelled as a proxy. |
| FO-D7 | Order | (A) strictly after the water and sky realms (QD-REALMS). (B) earlier, as land-only burrows without caves. | **A**; B only if the owner wants burrows as land ecology. |

## 13. Sources

- Culver, D. C., Pipan, T. (2009). *The Biology of Caves and Other Subterranean Habitats*. Oxford University Press.
- Jeffery, W. R. (2009). Regressive evolution in *Astyanax* cavefish. *Annu. Rev. Genet.* 43:25–47.
- Jones, C. G., Lawton, J. H., Shachak, M. (1994). Organisms as ecosystem engineers. *Oikos* 69:373–386.
- Moran, D., Softley, R., Warrant, E. J. (2015). The energetic cost of vision and the evolution of eyeless Mexican cavefish. *Sci. Adv.* 1:e1500363.
- Nevo, E. (1979). Adaptive convergence and divergence of subterranean mammals. *Annu. Rev. Ecol. Syst.* 10:269–308.
- Park, T. J., et al. (2017). Fructose-driven glycolysis supports anoxia resistance in the naked mole-rat. *Science* 356:307–311.
- Protas, M. E., et al. (2006). Genetic analysis of cavefish reveals molecular convergence in the evolution of albinism. *Nat. Genet.* 38:107–111.
- Vleck, D. (1979). The energy cost of burrowing by the pocket gopher *Thomomys bottae*. *Physiol. Zool.* 52:122–136.
- Wiens, J. J., Brandley, M. C., Reeder, T. W. (2006). Why does a trait evolve multiple times within a clade? Repeated evolution of snakelike body form in squamate reptiles. *Evolution* 60:123–141.

Bibliographic details are from memory. Check them against the publisher before any number here is
used for calibration.
