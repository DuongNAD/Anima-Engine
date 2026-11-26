---
phase: design
feature: flight
title: Design — Flight, birds and other flying creatures (AERO-D, proposed)
description: Proposed design for evolved flight in the live world. Wings and flight muscle are heritable traits developed once at birth; powered flight and gliding have physical costs in total_cost and an emergent mass ceiling. Flying is an agent state over the planar 256² sim, with diets through INVERT-D, fruit, AQUA-D and V4. Hazards, the agent budget, real flying agents replacing the decorative WorldBirds, and an opt-in world law that defaults off are covered. It builds on the locomotion media framework of RESP-D (in progress).
status: proposed
owner: lead
last_reviewed: 2026-11-26
---

# Design — Flight: birds and other flying creatures (AERO-D)

> **Status: `proposed`. Design only; no code is written under this card.** On 2026-11-25 the owner
> asked to "thêm cả chim và sinh vật bay nữa" (add birds and flying creatures too). Code anchors are
> verified on `origin/feat/ecological-selection` at `38ea4621`.
>
> - Under **QD-REALMS**, sky is the second realm: it comes after the SEL2-P result and after the water
>   realm (AQUA-D), each realm with its own ADR. This design is the input to that ADR. It does not
>   authorize work before the water realm lands.
> - The shared **locomotion media framework** is defined in RESP-D (in progress); §3 lists the minimum
>   that AERO-D assumes from it.
> - Insects come from INVERT-D (PR #299, proposed).
>
> The owner decisions are in §11 and the cards in §10.

## 1. What exists today (`38ea4621`)

| Fact | Anchor |
|---|---|
| Nothing in the simulation flies. No flight, wing or glide logic exists in `src-tauri/src/core`. | grep |
| `WorldBirds` draws about 18 two-triangle silhouettes circling in one instanced mesh, with the flap faked by Y-scale. `simEnvironment.ts` labels it decoration. | `WorldBirds.tsx:9-11`, `:38`, `:80`; mounted at `WorldShowcase.tsx:602`; `simEnvironment.ts:17` |
| The body is a graph of segments (`length`, `radius`, `mass`) joined by edges (anchor, axis). There is no appendage type, so no limb, fin or wing. | `genotype.rs:5-24` |
| Physics is planar and has no gravity: `velocity += force/mass·dt`, `pos += vel·dt`. A steering drive writes `InertiaComponent.target_velocity` (QD-LOC). | `dynamics.rs:121-215`; `components.rs:464`; ADR-0006 |
| Movement legality is a biome test. Ocean and DeepOcean are not walkable; River is. When `TerrainNavigation` is on, a root position on an illegal cell reverts to `LastWalkablePosition`. | `terrain_navigation.rs:31-33`, `:69`, `:197-260`; `components.rs:689` |
| The world **wraps**: positions are folded modulo `MapBounds` each tick. | `wrap_coordinates_system`, `world_systems.rs:46` |
| Every energy charge goes into `total_cost` in `metabolic_decay_system` and is respired to detritus. | `world_systems.rs:157`, `:223` |
| The brain shape is fixed at 15 inputs, 64 hidden and 8 outputs: 4 CPG, 3 gates (`pheromone_emit`, `attack_intent`, `feed_intent`) and 1 reserved `SIGNAL`. ADR-0019 (accepted) keeps that shape. | `brain_genotype.rs:36-55`; `components.rs:501-519` |
| `LocomotionMedium`, `HabitatPreference`, `DevelopedPhenotype` and `develop_at_birth` do not exist yet. The morphogenesis page lists `Aerial` as one medium and names flight-muscle overhead in `total_cost` as a candidate cost. | contract "Trạng thái triển khai"; `CREATURE_MORPHOGENESIS.md:154-166` |
| Creatures have a declared stylised scale, never derived from terrain metres (LD-1). One sim cell is 320 m at 81.92 km. | `2026-11-20-world-v2-design.md:188`; QD-WV2-D2 WB-D7 (PR #295, open) |
| The sim artifact carries elevation, moisture, temperature, flow, slope, biomes and water kind. It carries **no wind**: the generator's 2-D wind exists only offline, for rainfall. | `anmw_v3.rs:106-124`; `climate.rs:62-63` |
| Trees hold fruit, which prey eat on contact (`EnergyEvent::Fruiting`). | `components.rs:137`; `environmental_systems.rs:370` |
| Low tier allows 60 draw calls. Creatures use three LOD tiers, with 8 near skinned meshes on low. | `qualityTiers.ts:65`; `creatureLod.ts:24-31` |

The research review
[QD_REALMS_HABITATS_RESEARCH](../../research/QD_REALMS_HABITATS_RESEARCH_2026-11-22.md) §2 already
kept three ideas. Flight is costed by power (EU/s), not only by distance. A flight-muscle budget
competes with the rest of the body. A cheap vertical-wind field would favour soaring. The same
review marks the external report's flight mass ceilings (583 g to 25 kg) **chưa xác minh**; this
design does not reuse them.

## 2. Decision summary

1. **Flight is evolved anatomy.** Each segment carries a heritable wing trait, and the body carries
   a flight-muscle fraction. Development computes wing area, wing loading and the power margin once
   at birth (ADR-0001). Capacity to fly or glide follows from physics; no class tag grants it.
2. **Flying is a state, not a third axis.** Physics stays planar. An agent is `Grounded`, `Perched`
   or `Airborne`, with a bounded scalar altitude used for gliding, stooping and rendering.
3. **Costs go through `total_cost` only:**
   - a flight-muscle overhead while alive;
   - a high power draw while airborne;
   - a takeoff burst.

   Speed makes flight cheap per distance. The mass ceiling emerges from power scaling and is not a
   clamp.
4. **Individual flying agents** (Tier 1) within the existing agent budget. No flocks in this design.
   Aerial insects stay an INVERT-D field.
5. **Opt-in law** `WorldLawSet::flight` with `live.flight`. Absent means bit-identical, and the
   decorative `WorldBirds` stays.

## 3. Minimum assumed from RESP-D's locomotion media framework (in progress)

AERO-D does not define the framework. It assumes these five points and adds only what is
flight-specific:

1. **Capacities, not an enum.** Habitat is a set of continuous heritable capacities (walk, swim,
   dive, fly or glide) plus a breath type, developed once at birth and stored in the phenotype
   (contract D01, D02, D04). AERO-D supplies how `fly` and `glide` are computed (§4).
2. **One legality function.** `medium_legal(capacities, state, cell)` replaces the bare
   `is_pos_walkable` test for agents that carry capacities. It compares the creature's traits with
   the cell's class and never infers the medium from the cell. Agents without capacities keep
   today's test, unchanged.
3. **One chosen medium state.** Transitions between states are the agent's choice through a brain
   output (an ADR-0003 gate). A missing component reads as the legacy state (`Grounded`) and never
   forces a transition.
4. **One cost path.** Every medium cost is a term inside `total_cost`.
5. **Persistence.** Capacities live in the developed phenotype, and the current medium state lives
   in the saved runtime state. Restore and migration carry both and never re-develop (D01).

If RESP-D settles the brain output differently, for example as a new output after a
`BRAIN_GENOTYPE_VERSION` bump rather than a reuse, AERO-D follows it. Flight then adds one more
target state to the same output, never a second one. The reserved `SIGNAL` slot belongs to
ADR-0017 and is not taken.

## 4. Flight as evolved anatomy

**Genes.**

| Gene | Where | Meaning |
|---|---|---|
| `wing` | per segment | span and chord factors; both 0 = no wing. A membrane between segments works too (bat, pterosaur), so it is not tied to limbs. |
| `flight_muscle_fraction` | per body | share of body mass invested in flight muscle, taken from the other segments' mass |

All values are bounded. Legacy genomes get 0 by `#[serde(default)]`, which means no wing, so old
creatures are exactly today's.

**Developed at birth (once):**

- wing area `S`;
- wing loading `W/S`, with the weight in declared units;
- aspect ratio;
- muscle mass `m_f`;
- the power margin `R = P_avail / P_req` at the minimum-power speed.

**Physics** (Norberg 1990; Pennycuick 2008):

- Lift is `½ ρ v² S C_L`. The minimum flight speed grows with the square root of the wing loading,
  and wing loading grows about `M^(1/3)` under geometric similarity.
- The power required for level flight scales about `M^(7/6)`, while the power the flight muscle can
  deliver scales about `M^(2/3)`. So `R` falls about `M^(−1/2)`. Above some mass, powered flight is
  impossible (Pennycuick 1968; Schmidt-Nielsen 1984). The exponents are the classical
  geometric-similarity values, to re-check before tuning.
- Takeoff needs a flight-muscle ratio of roughly 16% or more of body mass (Marden 1987; value to
  re-check).

**Capacities** (from §3):

- `fly = clamp(R − 1, 0, 1)`: powered flight exists only when `R > 1`.
- `glide > 0` when `S > 0` and `R ≤ 1`: gliding without powered flight.

**The upper mass limit.** The largest living flapping birds weigh about 12–18 kg (bustards, swans;
per-species figures chưa xác minh). The extinct *Argentavis* (about 70 kg) is thought to have soared
rather than flapped (Chatterjee et al. 2007). Scaling of seabird wingbeat frequency puts a flapping
limit near 41 kg (Sato et al. 2009). The design never clamps mass. It fixes the constant in `R` so that
`R = 1` at a declared `M_flap_max`, and heavier bodies lose powered flight through physics.

The sim has **no declared kilograms**: creature scale is stylised (LD-1). So the law must declare a
mass-unit-to-kg mapping, or set `M_flap_max` relative to a founder mass (AE-D3).

**Gliding without gravity.** The sim has no gravity, so a glide is declared as a geometric rule: from
a launch height `h` (a perch on a tree or cliff, or the cell's elevation drop), an agent may travel
`glide_ratio × h` horizontally at low cost. `glide_ratio` rises with aspect ratio. Gliding animals
launch from height and use it to cross gaps (Dudley et al. 2007). Glide ratios for this world: chưa
xác minh.

## 5. What flight buys and costs

**Costs, all inside `total_cost`:**

- **Overhead:** flight-muscle maintenance while alive, as part of maintenance mass. This is the
  trade-off that lets flight be lost.
- **Airborne power:** `P_flight(M, W/S, state)`. It is several times the resting rate, a multiple
  that is chưa xác minh per taxon, and it is lower while gliding or soaring on declared uplift.
- **Takeoff burst:** a one-tick charge per takeoff.

Per second, flight is the most expensive way to move. Per kilometre it is cheaper than running at
the same mass, because the speed is far higher; only swimming is cheaper (Tucker 1970; Schmidt-Nielsen
1972). The airborne target speed is the walking speed times a declared ratio (chưa xác minh; real
birds cruise roughly an order of magnitude faster than small mammals walk). Both the cost and the
speed come from the agent's own phenotype, as QD-LOC requires.

**Gains:**

- **Crossing barriers.** Airborne agents may enter water cells. Rivers, lakes, straits and the sea
  stop being barriers, within their energy reserve. Running out over water means forced descent,
  and RESP-D's drowning path decides the outcome.
- **Escaping ground predators.** A grounded predator cannot strike an airborne agent. `combat_system`
  (`world_systems.rs:366`) gains a medium check, and only an airborne attacker may strike a grounded
  target (§6).
- **Reaching islands** (ADR-0016 W7). Flight raises gene flow across water, so the prediction is
  *less* allopatric divergence in flying lineages than in walkers across the same strait.
- **Losing flight on islands.** Where predators are absent and food is on the ground, the muscle
  overhead is pure cost. Flight capacity should then regress, as flightlessness repeatedly evolved
  on islands (McNab 1994; Roff 1994). This is a direct, testable signature for QD-ISLAND's
  predation-release question, and it must emerge rather than be scripted.

**Landing and perching:**

- Landing is legal on walkable cells, or on a perch: a `Tree` within reach, or a cliff cell above a
  declared slope.
- `Perched` agents pay maintenance only, cannot be reached by grounded non-climbers, and are where
  nesting happens (§7).
- With no legal landing cell within reach, the agent stays airborne and keeps paying.

## 6. Diets and niches

Flight changes where an agent can feed, never what it can digest. Diet traits stay those of the
existing body and brain (`feed_intent`, `attack_intent`).

| Niche | Resource | Dependency | When |
|---|---|---|---|
| Aerial insect eating | INVERT-D `aerial_prey_eu(cell)` via `take` (F6, Holling type III) | INVERT-D IN-5 | first slice |
| Fruit eating | `Tree.current_fruit`, reachable while `Perched`; seed dispersal by birds later through ADR-0015 | today's fruit path; ADR-0015 for seeds | first slice |
| Aerial predation on ground prey | an airborne attacker strikes a grounded target within strike radius (a stoop); the reverse is illegal | `combat_system` medium check | first slice |
| Fish diving | a short `Submerged` excursion under RESP-D's breath-hold, taking AQUA-D aquatic prey | RESP-D, AQUA-D | after the water realm |
| Scavenging | V4 `CarcassPool` records, found from the air over a wider radius | V4 | after V4 |

## 7. Space and time

- **Altitude is state, not a coordinate.** `Airborne` carries `altitude ∈ [0, 1]` × a declared
  ceiling, used for the glide budget, for stoop strikes (only from altitude) and for rendering. The
  sim stays on the 256² plane, with no 3-D collision and no octree.
- **Senses.** An airborne agent needs at least its own medium state and the local aerial-prey density
  as brain inputs. Any input-layout change is shared with RESP-D's new senses and bumps
  `BRAIN_GENOTYPE_VERSION` once.
- **Nest sites.** Reproduction of a flying lineage is placed at a perch (tree or cliff) or on a cell
  that grounded predators cannot reach, such as an island. Which sites count is a trait-versus-cell
  comparison, as in D04.
- **Daily and seasonal movement.** At 320 m per cell, a flyer can cross the 82 km world within its
  reserve. Real daily distances cannot be claimed: creature scale is stylised (LD-1) and time is
  compressed (QD-TIME; ADR-0016 quotes `1 day = 20 min`). The world sits at one latitude
  (WB-D2), so long-distance migration off the map cannot occur. Altitudinal movement up and down the
  massif with the seasons can emerge, and is the only migration claimed.
- **Map edges.** The world wraps (§1). Walkers rarely notice because the sea stops them, but a flyer
  leaving over the sea would re-enter on the opposite edge. Proposed: flyers reflect at the map edge
  when the law is on (AE-D6).

## 8. Hazards

Every rate here is chưa xác minh, and every charge goes through `total_cost`.

- **Storms and wind.** The sim carries no wind (§1). A storm is a declared forcing (`CauseId`) that
  multiplies airborne cost and forces landing inside its region. Agents forced down over water
  follow RESP-D. A real wind field from the generator's offline wind is a later card.
- **Volcanic ash** (ADR-0016 W3, opt-in under QD-WCHG): an ash plume region raises airborne cost and
  blocks takeoff; deposited ash acts on food through the plants and INVERT-D.
- **Wildfire smoke:** dose and breathing go through RESP-D's respiration model. AERO-D adds only
  that an airborne agent takes its dose at its altitude band. Whether bird lungs make them more
  sensitive is chưa xác minh.
- Deaths reach `Detritus` (or V4 carcasses) through the existing reclaim path, with the cause logged.

## 9. Population, performance, rendering and safety

**Budget.** Flyers are ordinary Tier-1 agents. They count against the 1,000-creature Iris Xe gate
(WV2-6, `2026-11-20-world-v2-design.md:515`) and the trial target of about 100 (QD-SCALE-TRIAL).
The headless cost is 5.643 ms p50 at N = 1,000 (ADR-0018 §1). A flyer adds:

- an O(1) state update;
- a medium check in strike and feed queries that already go through the spatial grid.

So nothing becomes O(N²) and nothing allocates. Flocks as Tier-2 super-individuals (ADR-0018 E6) are
left for a later card, for small birds only if needed.

**Rendering.**

- A flying agent uses the existing creature LOD tiers, drawn at `terrain height + altitude`.
- Wings come from its developed wing segments, flapping on its CPG phase.
- On low that is one extra instanced layer for wings in the mid tier, while `WorldBirds` (one draw
  call) is unmounted when the law is on. The net change is 0, within 60.
- With the law off, `WorldBirds` stays as the labelled decorative fallback.

**Science safety.**

- `WorldLawSet::flight: Option<FlightLaw>` (mass mapping, `M_flap_max`, speed ratio, glide and
  edge rules) emits zero canonical bytes when `None`, the ADR-0016 §5 pattern.
- `live.flight` absent or `0.0` must pass `validate_post_freeze_keys_default_to_legacy`
  (`prereg_sel2p2_manifest_tests.rs:422`).
- With the law off:
  - no wing genes are read and no medium state is inserted;
  - `terrain_navigation_system` and `combat_system` take today's branch;
  - no RNG draw is added.

| Gate | Positive check | Negative control |
|---|---|---|
| AE-S01 default-off identity | the frozen SEL2-P2 manifests, every `prereg_*` test and the E2/E3/E3b checksums are bit-identical; fingerprint unchanged | a mutant that inserts a medium state when off, or emits a tag byte for `None`, must turn red |
| AE-S02 one cost path | closed EU within 1e-3 with flyers active | a mutant that deducts flight cost from the reserve outside `total_cost` must break the residual |
| AE-S03 D01 restore and migration | `run N == run K → save → load → run N−K`; migration preserves wing phenotype, medium state and brain | a mutant that re-develops wings on restore must fail the phenotype bit-compare |
| AE-S04 D04 | capacity comes from traits only | a mutant that grants `Airborne` because the target cell is water must fail |
| AE-S05 emergent ceiling | a genome above `M_flap_max` has `fly = 0` through `R`; no mass clamp exists in the flight code | a mutant that gives required power the same exponent as available power must remove the ceiling, and the test must notice |
| AE-S06 island signature | paired seeds: on a predator-free island, flight capacity declines against a mainland control | with predators present, or with the muscle overhead set to zero, there is no decline |

## 10. Cards

All of these wait for QD-REALMS: the SEL2-P result, then the water realm (AQUA-D and its ADR), then
the sky ADR built from this design.

| Card | Size | Content | Waits for |
|---|---|---|---|
| AE-0 | S | sky-realm ADR from this design | water realm accepted |
| AE-1 | M | `FlightLaw` schema, `live.flight`, zero-tag fingerprint; AE-S01 | AE-0; RESP-D framework card |
| AE-2 | L | wing and muscle genes, development of `S`, `W/S` and `R`, capacities in the phenotype; AE-S03, AE-S04 | AE-1; V0 `develop_at_birth` |
| AE-3 | M | medium state machine, takeoff and landing, perch legality, edge rule, costs in `total_cost`; AE-S02, AE-S05 | AE-2; RESP-D legality card |
| AE-4 | M | diets: insects (INVERT-D), fruit while perched, stoop strikes; brain inputs | AE-3; INVERT-D IN-5 |
| AE-5 | M | renderer: flying agents in creature LOD, wing layer, unmount `WorldBirds` when on; map visual check | AE-3 |
| AE-6 | M | hazards: storm forcing, ash, smoke via RESP-D | AE-3; W3; RESP-D |
| AE-7 | L | island experiment: flight loss under predation release (AE-S06); allopatry with W7 | AE-4; ADR-0016 W7 |
| AE-8 | M (later) | fish diving, scavenging | AQUA-D, V4 |

## 11. Owner decisions

| ID | Question | Options | Recommendation |
|---|---|---|---|
| AE-D1 | Wing representation | (A) a heritable `wing` trait on any segment plus a body `flight_muscle_fraction`. (B) a new appendage node type in the genome graph. | **A**: no genome-graph change, and membrane wings (bats, pterosaurs) fit too. |
| AE-D2 | Space model | (A) planar sim with an `Airborne` state and a scalar altitude. (B) a real third coordinate with 3-D collision. | **A**: B needs gravity and 3-D queries the engine does not have. |
| AE-D3 | Mass ceiling units | (A) the law declares kilograms per sim mass unit and `M_flap_max` (about 15 kg for flapping, chưa xác minh), and the ceiling emerges from `R`. (B) set `M_flap_max` as a multiple of the founder mass, with no kilograms. | **A**: honest units; B is the fallback if LD-1 forbids kilograms. |
| AE-D4 | Population | (A) individual flying agents inside the existing agent budget; no flocks. (B) also Tier-2 flocks now. | **A**. |
| AE-D5 | First diets | (A) insects, fruit and stoop predation first; diving and scavenging after AQUA-D and V4. (B) all five together. | **A**. |
| AE-D6 | Map edge for flyers | (A) reflect flyers at the edge when the law is on. (B) keep wrapping for everyone. | **A**: wrapping makes the open sea a shortcut. |
| AE-D7 | Brain output | (A) share RESP-D's medium-intent output and add one target state. (B) a separate flight output. | **A**, following RESP-D. |
| AE-D8 | Order | (A) strictly after the water realm, per QD-REALMS. (B) let the sky go first. | **A**. |

## 12. Sources

- Chatterjee, S., Templin, R. J., Campbell, K. E. (2007). The aerodynamics of *Argentavis*, the world's largest flying bird from the Miocene of Argentina. *PNAS* 104:12398–12403.
- Dudley, R., et al. (2007). Gliding and the functional origins of flight: biomechanical novelty or necessity? *Annu. Rev. Ecol. Evol. Syst.* 38:179–201.
- Marden, J. H. (1987). Maximum lift production during takeoff in flying animals. *J. Exp. Biol.* 130:235–258.
- McNab, B. K. (1994). Energy conservation and the evolution of flightlessness in birds. *Am. Nat.* 144:628–642.
- Norberg, U. M. (1990). *Vertebrate Flight*. Springer.
- Pennycuick, C. J. (1968). Power requirements for horizontal flight in the pigeon *Columba livia*. *J. Exp. Biol.* 49:527–555.
- Pennycuick, C. J. (2008). *Modelling the Flying Bird*. Academic Press.
- Roff, D. A. (1994). The evolution of flightlessness: is history important? *Evol. Ecol.* 8:639–657.
- Sato, K., et al. (2009). Scaling of soaring seabirds and implications for flight abilities of giant pterosaurs. *PLoS ONE* 4:e5400.
- Schmidt-Nielsen, K. (1972). Locomotion: energy cost of swimming, flying, and running. *Science* 177:222–228.
- Schmidt-Nielsen, K. (1984). *Scaling: Why Is Animal Size So Important?* Cambridge University Press.
- Tucker, V. A. (1970). Energetic cost of locomotion in animals. *Comp. Biochem. Physiol.* 34:841–846.

Bibliographic details are from memory. Check them against the publisher before any number here is
used for calibration.
