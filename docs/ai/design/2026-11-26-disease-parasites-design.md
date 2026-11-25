---
phase: design
feature: disease-parasites
title: Design — Disease, parasites and parasitic organisms (PATHO-D, proposed)
description: Proposed design for opt-in disease in the live world. Per-agent SEIRS microparasites carry evolving strains on their own RNG stream under a virulence–transmission trade-off. Heritable resistance, tolerance and immune cost are billed through total_cost. Macroparasites are burden counts with a direct life cycle, and a micropredator feeding mode is the first parasitic creature. The design names observables with null controls, a host–parasite Red Queen experiment PATHO-RQ (never S43), hazard and weather interfaces and a render-only sickness cue, and splits the work into cards PD-1 to PD-10 with owner decisions PD-D1 to PD-D10.
status: proposed
owner: simulation-architecture
last_reviewed: 2026-11-26
---

> **Status (lead, 2026-11-26).** Proposed, docs only, no code. Owner request in chat on 2026-11-25/26:
> "thêm cả bệnh tật và ký sinh trùng nữa, sinh vật kí sinh nx" (add disease and parasites, and parasitic
> organisms too). Code anchors were read at trunk `38ea4621`. Re-read the symbols before editing. Sibling
> designs drafted the same night are cited by name as **in progress**, not redefined here: RESP-D
> (respiration, land-to-water continuum, locomotion media), INVERT-D (insect guild fields, aquatic larvae
> included), AQUA-D (fish and aquatic life), AERO-D (flight), FOSS-D (burrowers and caves), WX-D (weather)
> and WV2-D2 (the 82 km world under QD-WV2-BIG; the 256² sim grid stays the same size).
> Numbers without a citation are design values, marked **chưa xác minh**.

## 1. Request and scope

**In scope:** microparasites (viruses, bacteria, protozoa) as a per-agent infection state with a strain
id; pathogen evolution; host defence as heritable traits, and sickness behaviour; macroparasites (worms,
ticks, lice) as burden counts; whether parasitic organisms should be creatures; observables and null
controls; interfaces to hazards and weather; a render cue; science safety; cards and owner decisions.

**Out of scope:**
- **Vectors as organisms.** INVERT-D, ADR-0018 Tier 2 and its card E6 own them.
- **Fish and snail hosts.** These belong to AQUA-D.
- **Burrows.** These belong to FOSS-D.
- **Plant disease.** This belongs with [ADR-0015](../../decisions/ADR-0015-evolving-plants.md).
- **Named real pathogens and human disease.**
- **Any "species" label for a strain or a parasite morph.** ER09 and ADR-0003 decision 11 forbid it.

## 2. What the code has today (`38ea4621`)

A grep of `src-tauri/src`, `src-tauri/crates` and `src` was run for `infect|pathogen|parasit|immun|disease|contagi|epidem|virulen|sickness`. **There is no disease model:** no infection state, no immunity, no parasite and no contact transmission. The hits are:

| Fact | Anchor |
|---|---|
| A **declared intervention** `InterventionKind::PathogenTransmission { pathogen_id, transmission_rate, mortality_multiplier }`. | `crates/anima-domain/src/intervention.rs:136-141` |
| **The live handler strips a share of every living reserve into detritus.** Every living agent loses the same share, `clamp(\|intensity\| · rate · mult · 0.01, 0, 0.5)`, through `settle_reserve_outflow_to_detritus`. `pathogen_id` is ignored (`pathogen_id: _`). Nobody is infected, nobody recovers, and nobody is filed as dying of it. | `core/live_experiment.rs:1349-1404` |
| An EU leak in that handler was fixed in G5 (#95). | Test `ledger_bypass_tests.rs:235` |
| **Legacy aggregate model.** It moves herbivore biomass into detritus. | `core/scenario.rs:261-281` |
| **Validation** checks only that the parameters are finite. | `core/experiment/interventions.rs:150-160` |
| **The record and the manifest canon** carry the kind. | `world_record/ring.rs:178`, `experiment/manifest.rs:682` |
| **The intervention is host-only.** It is listed as forbidden to guests. | `island/intents.rs:53-68` |
| **The UI** offers it as the option "Pathogen Outbreak (Epidemic)". | `src/components/ResearchDashboard/CausalInterventionPanel.tsx:180` |
| `DemographicTracker::deaths_pathogen` is a label that **always reads 0**. No call site passes `"Pathogen"` to `record_death`. | `core/telemetry_export.rs:89, 109` |
| `DeathCause` has only `Starvation` and `Dehydration`. | `core/mortality.rs:187-192` |
| The World Record set is closed. ADR-0008 v1 `DeathCauseTag` = {starvation, dehydration, predation, reclaim, epoch_replacement}. | `world_record/types.rs:261-270` |
| **The other hits are not disease.** "Contagious" is used figuratively for NaN at `world_systems.rs:750` and for `None` at `evolution/lineage.rs:373`. `island/inhabit.rs:120` says guests get zero extra "immunity" privilege. | — |
| **Disease is planned, not built.** The plan has a `DiseaseState` target, a "disease contact" step in its update order, and §5.8 "after lifecycle is stable". ADR-0018 names opt-in "SEIR-lite" and puts virulence in `total_cost`. It also sketches a mosquito across three tiers. | `WORLD_SIMULATION_PLAN.md:281, 421, 627`; [ADR-0018](../../decisions/ADR-0018-ecosystem-scales-and-cycles.md) §3.1–3.2 |

Facts this design builds on:

| Fact | Anchor |
|---|---|
| `total_cost = k_base + maintenance + activity_cost + cognition_cost` | `core/world_systems.rs:157` |
| The same `total_cost` also drives hydration loss and body heat. Any infection cost therefore also raises thirst and temperature, a fever-like side effect that is not calibrated. | Hydration `:165`, heat `:168`, decay `:174-176` |
| A dead agent's reserve goes to detritus. `ReclaimAndDespawnAgentCommand` banks it at the despawn sync point. | `core/agent_systems.rs:135`, banking `:155-172` |
| **Reproduction has one parent.** `AgentSex` is a random label. | Single parent `parent_ids: vec![parent_lineage_id…]` at `agent_systems.rs:1218`; `AgentSex` at `:419-430` and `:598-609` |
| **Sexual reproduction is accepted but not in the code.** ADR-0013 accepted bi-parental mating as opt-in (V3-SEX). | — |
| **Nothing for mate choice, nests or parental care.** A grep for `nest\b\|parental\|provision` in `src-tauri/src` finds no hits. | — |
| `SENSORY_SLOTS = 15`. ADR-0017 forbids "oracle" input slots. | `agent_systems.rs:1484` |
| Predation moves EU from prey to predator, and the loss goes to detritus. | `combat_system` at `world_systems.rs:366`; `LINDEMAN_EFFICIENCY` at `laws.rs:46`; loss to detritus at `:485-513` |
| **RNG stream numbers.** `sim_stream` 1–5 are in the code, and `DORMANCY_STREAM` is 7. **Open proposals already collide:** V0 and ADR-0015 both claim 6, ADR-0015 claims 7 (taken by dormancy), and ADR-0016 claims 8. | `resources.rs:925-948`; `aggregate_population.rs:190` |
| `world_checksum` sorts agents by content, never by entity id. Entity ids differ after a restore. | `snapshot.rs:784-789` |
| **Measured tick cost** (headless, i5-14600KF): `0.331 ms` p50 at 100 agents, `5.643 ms` at 1,000, growing about `N^1.86` beyond that. QD-SCALE-TRIAL targets about 100 animals, with 1,000 kept as the design point. | [`scaling.md`](../../paper/data/scaling.md) lines 39 and 72 |

## 3. Rules this design applies

1. **The law is opt-in and absent by default.** With the law absent, everything matches the trunk bit for bit: the SEL2-P2 preregistration (#290), every `prereg_*` test and the `world_checksum` (§12).
2. **Closed EU.**
   - Every energy charge from infection, burden, immunity or sickness enters `total_cost` in `metabolic_decay_system`.
   - Disease deaths go through `ReclaimAndDespawnAgentCommand`.
   - There is no other source or sink.
   - Pathogen loads and parasite eggs are **counts, never EU**.
3. **No Lamarck.** Acquired immunity and infection are runtime state. They are never written into any genotype. Vertical transmission passes infection state, never genes.
4. **Restore, migration, wake and rehydration carry state verbatim** (D01/D02 extended):
   - `InfectionState`, `ParasiteBurden` and `DefenceGenotype` travel by value, as optional `#[serde(default)]` fields on four payloads: `SerializedAgent` (`simulation_state.rs:105`), `AgentMigrationData` (`components.rs:166`), `ArchivedIndividual` (`aggregate_population.rs:224`) and the dormant record.
   - Strain ids are local to a world, so payloads carry strain traits by value and the destination re-interns them.
5. **Determinism (D07).** All disease randomness comes from a dedicated `sim_stream::PATHOGEN` stream (PD-D9).
   - Draws are **counter-based**: `u = hash(run_seed, PATHOGEN, tick, agent_key, event)`.
   - The result does not depend on iteration order or entity ids. No RNG state has to be checkpointed.
   - The ecology `SimRng` never moves, so a disease-on run and a disease-off run share their ecology draws.
6. **Allocation-free ticks.**
   - Every buffer is allocated when the law activates.
   - The strain table has a fixed capacity.
   - Neighbour lookup uses `SpatialHashGrid` buckets (`physics/spatial.rs:31`), capped at K neighbours with the ADR-0017 S3 tie-break.
7. **The schedule is built once.** New systems go into `core::simulation_schedule::build_tick_schedule` (`simulation_schedule.rs:121`): contact at the plan's step 8, progression before `metabolic_decay_system`.
8. **No species claims.** A strain is a strain. S43 stays Red-Queen predator–prey; the host–parasite experiment is **PATHO-RQ**.

## 4. Microparasites: state, course and transmission

**State.** An optional component, `Copy`, at most 16 B (sizes are chưa xác minh):

```rust
pub struct InfectionState {
    pub stage: InfectionStage,  // Exposed | Infectious | Recovered
    pub strain: StrainIdx,      // u16 index into StrainTable
    pub immune_key: u8,         // key of the strain last cleared (Recovered only)
}
```

When the component is absent, the host is susceptible and has never been infected. The S-compartment therefore costs nothing.

**Course (SEIRS)** (Kermack & McKendrick 1927 for SIR; Keeling & Rohani 2008 for SEIR/SEIRS):

| Step | Per-tick probability |
|---|---|
| E→I | σ |
| I→R | γ |
| I→death | α_eff = α·(1 − tolerance) |
| R→S (immunity wanes) | ω |

- Constant per-tick probabilities give geometric stage durations: the cheapest form, and memoryless.
- **Design scale (chưa xác minh):** the infectious period is about 2–5 % of a mean lifespan. QD-PLAYRULES puts lifespan at 1–2 h.
- **Morbidity, only while Infectious.** Two effects:
  - An energy term in `total_cost`: `disease_cost = c_m · α_eff`. It is added **only when the component exists**, so the legacy f32 sum stays untouched, as for ADR-0013 growth.
  - **Sickness behaviour:** speed and feeding multipliers `1 − s_b·α_eff`, applied in `steering_drive_system` (`locomotion.rs:190`) and `detect_food_collisions_system` (`world_systems.rs:274`). Hart (1988) argues that lethargy and anorexia are adaptive. In v1 they are imposed, not evolved, and they also cut `activity_cost`.
- **A disease death** queues `ReclaimAndDespawnAgentCommand` with a new `DeathCause::Disease` (PD-D3). Once V4 lands, it leaves a carcass that carries the strain ([V4 spec](../planning/2026-11-24-v4-carcasses-decomposition-spec.md)).

**Transmission.** For each susceptible host `h` with an infectious neighbour or a contaminated cell:

```text
λ_h = (1 − resistance_h) · [ Σ_{j ∈ N_K(h), j ∈ I} β(α_j) · m(key_j, key_h) · w(d_hj)  +  u_env · E[cell(h)] ]
P(infect this tick) = 1 − exp(−λ_h · dt)      strain = weighted draw over the terms
```

- **Contact.** `N_K` is at most K = 16 neighbours within radius `r_c`. `w` falls off with distance. Transmission is density-dependent by construction: more hosts nearby means more terms. A frequency-dependent variant, which normalizes by neighbour count, is the null in §9.
- **Matching.** `m` is a matching term on an 8-bit key:
  - it is 1 when Hamming(strain key, host key) ≤ h0, and `m_min` otherwise;
  - recovered hosts are protected against strains whose keys lie within h0 of their `immune_key`;
  - `key_bits = 0` turns matching off.

  This is the cheapest form of genetic specificity that can produce negative frequency-dependent selection. The idea follows Hamilton (1980) and Hamilton et al. (1990); the exact functional form is a design choice (chưa xác minh against their models).
- **Environment.**
  - `E` is an environmental load field on the 256² grid: an f32 load plus a u16 dominant strain per cell, 384 KiB in all.
  - Infectious hosts shed into their own cell.
  - Load decays faster when the cell is dry and hot, reading `soil_moisture`/`surface_water` and `air_temperature` from `DynamicFields` (`dynamic_fields.rs:80-86`), and slower in water.
  - Drinking from a cell takes up load. The rates are chưa xác minh.
  - Rates are expressed **per unit area**, so they survive whatever WV2-D2 settles about cell size.
- **Vectors.**
  - The vector term reads `vector_contact(cell) -> f32` from a future biting-vector pool: an ADR-0018 Tier-2 swarm (card E6) fed by INVERT-D's aquatic larvae (IN-9).
  - Neither exists yet, and INVERT-D keeps vectors out of scope. Until they exist, a law that asks for vector transmission **fails preflight** rather than reporting zero (the AE3 rule).
  - Ewald (1983) predicts higher virulence for vector-borne pathogens. That is a testable observable once vectors exist.
- **Vertical transmission** is a law parameter `v`, default 0.
  - At birth (`SpawnOffspringCommand`, `agent_systems.rs:494`), an Infectious parent infects the newborn with probability `v`; the newborn starts in E with the parent's strain.
  - The newborn's genomes are identical to those of a run with `v = 0`.
  - Lipsitch et al. (1996) predict lower virulence as `v` rises.

**Seeding.** New declared interventions (PD-D5) introduce index cases, each with a `CauseId`:
- `SeedInfection { strain_template, count }`;
- `Cure { region }`, which clears infection or burden;
- `Vaccinate { fraction }`, which moves S→R.

`PathogenTransmission` stays exactly as it is.

## 5. Pathogen evolution

| Topic | Design |
|---|---|
| **Strain traits** | Virulence `α ∈ [0, α_max]` and the 8-bit `key`. There is no within-host model. |
| **Trade-off** | `β(α) = β_max · α / (α + α_half)`, so `R0(α) ∝ β(α) / (α + μ + γ)`. Anderson & May (1982) fitted myxomatosis through a virulence–recovery link; Alizon et al. (2009) review the general trade-off. Cressler et al. (2016) and de Roode et al. (2008) give the empirical support and its limits. |
| **Prediction** | For this curve, in a well-mixed endemic setting, α should approach **α\* = √(α_half · (μ + γ))**. The derivation is exact, but the curve is a design choice, and host mortality μ is emergent here, not a constant. Spatial structure should lower infectivity (Boots & Mealor 2007). |
| **Mutation** | At each transmission event, with probability `μ_s`, draw `α' = clamp(α + N(0, σ_α))` and flip one key bit with probability `μ_k`. Draws come from the PATHOGEN stream. |
| **Strain table** | Fixed capacity: 64 rows of about 40 B, chưa xác minh. Each row holds the parent strain, traits, origin tick and host count, so a strain phylogeny is available. When the table is full, a mutant keeps its parent's row and `disease.strain_saturation` counts the loss. A row is freed, in id order at the end of the tick, when it has no host and no environmental load. |

**What is honest at 100–1,000 hosts:**
- **Burn-out is the expected outcome.** The critical community size for measles is about 250,000–300,000 people (Bartlett 1957; Keeling & Grenfell 1997 model the persistence). An acute, immunizing SEIR pathogen in 100–1,000 hosts will usually burn out after one epidemic.
- **Persistence comes only from declared mechanisms** (PD-D6): waning immunity (ω > 0), the environmental reservoir, long infectious periods, or a declared spillover law with its own `CauseId`. There is never a silent re-seed. Fade-out time is a reported result.
- **Drift dominates.** The pathogen's effective population is about the number of infected hosts: tens. Few strains coexist, and drift dominates selection on α unless `s · N_I ≫ 1`.
- **Claims need ensembles.** Every claim about virulence evolution needs an ensemble and the neutral-strain null in §9. One run proves nothing.

## 6. Host defence

**`DefenceGenotype { resistance, tolerance, key }`** is an `Option` sibling of `BrainGenotype`, following ADR-0003 decision 1. When it is absent, the host keeps legacy susceptibility and pays no cost.
- **Founders** draw it on the PATHOGEN stream and make zero `SimRng` draws.
- **Offspring** copy it today and recombine it under V3-SEX. Mutation is keyed by the birth event on the PATHOGEN stream.

| Trait | Effect | Cost in `total_cost` | Source |
|---|---|---|---|
| Resistance `r ∈ [0,1)` | Scales infection probability by `(1 − r)`, and worm acquisition the same way | Constitutive `k_r·r` every tick, plus an induced cost while infected | Sheldon & Verhulst 1996 |
| Tolerance `t ∈ [0,1)` | Scales `α_eff` and the morbidity and burden costs by `(1 − t)`. **It does not reduce transmission.** | Constitutive `k_t·t` | Råberg et al. 2007 |
| `key` | The matching type of §4 | None | Hamilton 1980 |

**Predictions.**
- With the law on but no pathogen ever seeded, mean resistance and tolerance should fall, because they are pure cost.
- Resistance should lower prevalence, while tolerance does not. The theory that tolerance can instead raise prevalence (Roy & Kirchner 2000) is chưa xác minh here.

**Behavioural defence (card PD-8, gated by PD-D7).**
- **Avoidance of sick conspecifics.** It exists in nature: spiny lobsters avoid infected conspecifics before those become infectious (Behringer et al. 2006), and female mice discriminate the odour of parasitized males (Kavaliers & Colwell 1995). Hart (1990) lists avoidance and grooming among the behavioural defences.
- **There is no oracle `neighbour_is_sick` input.** ADR-0017 rejects oracle input slots (its §4 item 4). Instead, an Infectious host emits an involuntary cue scaled by `α_eff` on a chemical channel of ADR-0017. Any receiver senses it through its normal olfactory inputs, and that includes predators, which may then prefer sick prey.
- **Grooming** is a brain output. Above a threshold, it removes ectoparasites at an energy cost (in `total_cost`) and costs a tick of feeding and moving.
- **Both need more brain inputs and outputs** than `SENSORY_SLOTS = 15` and the current `ActionGates`. That means a versioned `ArchSpec` change. It should land after ADR-0017 S2 and D-SIG-2 provide channel slots.

## 7. Macroparasites

**`ParasiteBurden { endo: u16, ecto: u16 }`** is an optional per-agent component. It holds counts, not SEIR stages (Anderson & May 1978).

**The direct life cycle, recommended as the minimal honest version (PD-6):**
- **Endoparasites (worms).**
  - A host carrying burden `W` sheds eggs (`f·W` per tick) into an egg field: an f32 count per cell, 256 KiB.
  - Eggs develop and die at rates set by moisture and temperature.
  - A host in a cell acquires new worms with mean `u·(1 − r)·eggs·dt`, drawn on the PATHOGEN stream.
  - Worms die at rate `μ_w`.
- **Ectoparasites (ticks, lice).** They pass on contact through the same `N_K` neighbours, and later through FOSS-D burrow cells. Grooming removes them.
- **Cost.** Each worm or tick costs `c_w` per tick in `total_cost`; tolerance scales it. Above a threshold the sickness multipliers of §4 apply. Mortality that depends on intensity then emerges through starvation and is never scripted.
- **Aggregation must emerge, not be imposed.** Wildlife burdens are usually negative-binomial: Shaw et al. (1998) found it fitted 45 of 49 data sets, with `k < 1` in 43. Anderson & May (1978) show that aggregation stabilizes host–parasite dynamics.
  - **Observable:** a moment estimate `k = m²/(v − m)`.
  - **Null:** with identical hosts and uniform exposure, the burden should be close to Poisson (`v ≈ m`).

**Complex life cycles.**
1. **Two-host cycle, after AQUA-D lands.** The definitive host is a Tier-1 agent and the intermediate host is an aquatic field (snail or fish). Transmission is trophic: an agent that eats from the field acquires larvae.
2. **Host manipulation comes after that.** Lafferty & Morris (1996) found parasitized killifish substantially more prone to bird predation; Poulin (2010) reviews the field.
   - Manipulation would be a strain trait that acts only through the host's speed, feeding or cue multipliers.
   - It never touches brain weights or the genotype.

**Energy.** Parasite biomass is not modelled, and what parasites take is respired into detritus (PD-D8). This is a known simplification: Kuris et al. (2008) measured parasite biomass above that of top predators in estuaries.

## 8. Parasitic organisms as creatures

| Option | What it is | Cost against the budget | Verdict |
|---|---|---|---|
| **A. Fields and aggregate state** | Pathogens (§4–5) and burdens (§7) | Per agent ≤ 32 B. Contact is `O(N_I · K)` with K ≤ 16, a design estimate of ≤ 0.1 ms per tick at N = 1,000, which is ≤ 2 % of 5.643 ms (**not measured**). Fields come to 640 KiB in all, swept in strides. | **Recommended now** |
| **B. Tier-2 parasite swarms** | Parasitoid wasps and biting flies as super-individuals: one host per larva, and the host dies when the larva emerges (Godfray 1994). | `O(#swarms)`, but it needs ADR-0018's `SwarmPool` (not in code) and INVERT-D | **With INVERT-D/ADR-0018 Tier 2** |
| **C. Tier-1 evolving parasite agents** | Individual worms, wasps or cuckoos with bodies and brains | Doubling N costs about 2^1.86 ≈ 3.6× tick time on the measured curve. Parasitoids and worms are far below Tier-1 body size. Brood parasites (Davies 2000) need nests and parental care, and the code has neither (§2). | **Rejected** |
| **D. Micropredator feeding mode** | A heritable, opt-in trait. An agent takes small, non-lethal bites from a larger agent's reserve through `combat_system`'s existing ledgered transfer, like a leech, a lamprey or a vampire bat. Lafferty & Kuris (2002) separate strategies by the number of victims per stage and by whether the attack ends the host's fitness. | Zero new entities: one gate in an existing system | **First parasitic creature (PD-9)** |

Option A also means:
- **Rendering cost** is one byte per agent (§11), so the 1,000-creature Iris Xe gate in `NR-SPIKE` sees no new draw calls.
- **Allocation:** a new `pathogen_zero_alloc_tests.rs` extends the `TrackingAllocator` gates.
- **Speed:** a scaling row at N ∈ {100, 1,000} with the law on, measured with the `scaling.md` harness, has to stay ≤ 3 % of p50. That threshold is a design value.

Tierra showed parasites and hyper-parasites arising unprogrammed in a digital world (Ray 1991). That kind of emergence is out of reach while bodies and brains stay fixed-interface, and no option here claims it.

## 9. Ecology and evolution payoffs

New observables are appended **after** the 28 frozen ones (`SEL2P2_FREEZE_OBSERVABLE_IDS`, `prereg_sel2p2_manifest_tests.rs:125`). They are emitted only while the law exists. A manifest that requests one without the law fails preflight (`ensure_observables_supported`, `live_experiment.rs:849`). Cumulative counters use `Aggregation::Instant`.

| Payoff | Observables | Null control | Basis |
|---|---|---|---|
| **Epidemics** | `disease.prevalence`, `.incidence`, `.recovered`, `.deaths` (cumulative), `.r_eff` (secondary cases counted per infector), `.fadeout_tick` | Law off, or a law with no index case: zero by construction, and the checksum is unchanged | Kermack & McKendrick 1927 |
| **Density dependence** | Final size and early `r_eff` across founder density | A frequency-dependent variant removes the threshold density | Keeling & Rohani 2008 |
| **Population cycles** | Host density autocorrelation and spectral peak; then `Cure` all burdens at a declared tick | Same seeds with the law off; `Cure` applied in a run with the law off | Hudson et al. 1998: removing parasites stopped red grouse cycles |
| **Virulence evolution** | Infected-weighted mean and variance of α; strain phylogeny | Neutral strains: α mutates but has no effect (flat `β`, `α_eff = 0`); the drift band comes from an ensemble | Alizon et al. 2009; Ebert 1998 |
| **Defence evolution** | `host.resistance_mean/var`, `host.tolerance_mean` | Law on with the pathogen never seeded: must decline. The cost set to 0: drift only. | Sheldon & Verhulst 1996 |
| **Herd effects** | Incidence among susceptibles against the local fraction recovered; the `Vaccinate` intervention | The same coverage applied to already-recovered hosts gives no extra protection | Keeling & Rohani 2008 |
| **Aggregation** | `macro.burden_mean`, `macro.burden_k`, `macro.prevalence` | Identical hosts and uniform exposure give a Poisson burden | Shaw et al. 1998 |

**PATHO-RQ: the host–parasite Red Queen experiment, with its own ID and never S43.**
- **Setup:** the law on with `key_bits = 4`; host and strain keys both mutate.
- **Observables:**
  - the time-lagged cross-correlation between host-key and strain-key frequencies;
  - an offline time-shift assay. Snapshotted hosts from time `t` are challenged with strains from `t − Δ`, `t` and `t + Δ`, by analogy with Decaestecker et al. (2007), who did it with resurrected sediment.
- **Nulls:**
  1. `key_bits = 0`;
  2. host keys frozen, so only the parasite evolves;
  3. host–strain pairings shuffled in the assay.
- **Rules:** preregistered and run as an ensemble, like S43. No conclusion is drawn from a single run.

**Sexual selection for resistance is not possible today.** The code has no mating (§2). ADR-0013's V3-SEX is accepted but not built.
- **After V3-SEX:**
  - PATHO-RQ gains sexual and asexual arms, to test the parasite theory of sex (Hamilton 1980; Hamilton et al. 1990; Lively 1987).
  - Hamilton & Zuk (1982) additionally need mate choice on a costly, condition-dependent signal: ADR-0017 visual displays, which are billed in `total_cost`.
- **The observable is defined now and stays unused until then:** the correlation between a displayer's signal intensity and its burden or infection.

## 10. Interfaces to hazards and weather

These are interfaces only. Each one is a no-op until its owner lands, and each draws nothing from the PATHOGEN stream while it is a no-op.

| Link | Interface | Owner |
|---|---|---|
| **Floods spread waterborne pathogens** | `PathogenEnvField::advect(downstream, water_moved)` moves load and eggs along `DynamicFields::downstream` (`dynamic_fields.rs:76`) in proportion to the water that moves | [ADR-0016](../../decisions/ADR-0016-changing-world-surface.md) Tier 2 water stage; D-WCHG-3 |
| **The wet season raises vectors and environmental survival** | Decay and egg-development rates read soil moisture and surface water; `vector_contact(cell)` reads the future vector pool | WX-D (in progress) through `step_climate(tick, season_phase, …)` (`dynamic_fields.rs:256`); INVERT-D (in progress) |
| **Carcasses after disasters become reservoirs** | `CarcassRecord` gains `strain: Option<StrainIdx>, load`; scavenging is a contact | V4 spec (planned); ADR-0016 fire and flood deaths |
| **Burrows as reservoirs** | Burrow cells keep environmental load and ectoparasites with slow decay | FOSS-D (in progress) |
| **Aquatic hosts and intermediate hosts** | The two-host trophic link of §7 | AQUA-D (in progress) |
| **Flight and respiration** | No new mechanism. Infected fliers simply carry the infection further. Respiratory pathogens are out of scope. | AERO-D and RESP-D (in progress) |

## 11. Rendering

Rendering stays read-only (ADR-0012).
- **Payload.** The tick payload `SegmentState` (`simulation_state.rs:23-47`, exported through ts-rs) gains `#[serde(default, skip_serializing_if = "Option::is_none")] sickness: Option<u8>`, a quantized `α_eff` plus burden. With the law off the field is absent and the payload bytes are unchanged. Gate CM-S10 checks the Rust and TypeScript sides agree.
- **Posture comes from physics.** The speed multiplier slows the gait, and nothing is animated by hand.
- **Colour.** The renderer adds a subtle overlay: desaturation of up to 15 % and a head droop of up to 8°, both design values.
- **Pigment rules.**
  - The overlay **never writes `display_color`**. D08 fixes colour at birth unless a costly `DynamicChromatophore` trait exists.
  - When the pigment is `None` (D09), the overlay applies to the class fallback colour.
  - The overlay is removed on recovery.
- **Guests.** An inhabited organism shows the same cue and has no immunity privilege (ADR-0014).
- **Visual check.** The render card runs the `map-visual-check` creature close-up before and after.

## 12. Science safety and gates

The law is **`WorldLawSet::pathogen: Option<PathogenLaw>`**.
- It uses `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- When it is `None`, canonicalization emits **zero bytes**, so `WorldLawSet::baseline().fingerprint()` is unchanged. The pattern is `laws.rs:85-118`.
- The live adapter reads `live.disease`; absent or `0.0` means off. That key must pass `validate_post_freeze_keys_default_to_legacy` (`prereg_sel2p2_manifest_tests.rs:422`) under QD-BIO-D2.
- **Off means absent:** no resource, no component, no buffer, no RNG draw and no checksum byte. `world_checksum` hashes disease state only while the resource exists, and then by content: stage, strain *traits* and timers in the sorted per-agent tuple, following the ADR-0013 growth pattern.

| Gate | Claim | Negative control (must fail red under fault injection) |
|---|---|---|
| **PD-S01** | Law off ⇒ trunk `world_checksum`, every `prereg_*` test and the SEL2-P2 fixtures are bit-identical, with zero extra draws | Write one byte for `None` in canonicalize ⇒ the fingerprint test fails |
| **PD-S02** | **Infection never creates EU.** \|closed residual\| stays within the S01 tolerance under a 90 %-prevalence epidemic with burdens and deaths. | Deduct `disease_cost` outside `total_cost` ⇒ fails, as `ledger_bypass_tests.rs` does |
| **PD-S03** | A law with no index case and zero defence cost matches the law-off run in every agent state and ledger value (the checksum differs only by the disease bytes it adds) | Draw one PATHOGEN-stream value per tick ⇒ fails |
| **PD-S04** | **Vertical transmission never writes the genotype.** A newborn's morphology, brain and defence genomes equal those from a `v = 0` replay of the same birth. | Copy the parent's `immune_key` into `DefenceGenotype.key` ⇒ fails |
| **PD-S05** | `run N == run K → save → load → run N−K` holds during an epidemic. Migration, wake and rehydration keep infection, burden and defence. | Re-roll the stage on restore ⇒ fails |
| **PD-S06** | Same-seed replays are identical across thread counts. Draws do not depend on iteration order. | Iterate by entity id with a stateful RNG ⇒ a restore-order test fails |
| **PD-S07** | Zero heap allocations per tick in every disease system | Push into an unreserved `Vec` ⇒ `TrackingAllocator` fails |
| **PD-S08** | Disease death is filed as `Disease`. `deaths_pathogen` counts it, and so does the World Record tag once PD-D3 is decided. | — |
| **PD-S09** | An observable requested without the law fails preflight | — |
| **PD-S10** | The neutral-strain, Poisson-burden and frozen-key nulls of §9 stay inside their drift bands | — |

## 13. Cards

Rough sizes include tests: S < 500 lines, M 500–1,200, L > 1,200 (chưa xác minh).

| Card | Content | Size | Depends on |
|---|---|---|---|
| PD-1 | `PathogenLaw`, `live.disease`, `sim_stream::PATHOGEN`, the counter-based RNG helper, preflight; gates PD-S01, S03, S09 | S | PD-D4, PD-D9 |
| PD-2 | `InfectionState`, `StrainTable`, contact transmission, the SEIRS course, `disease_cost` and sickness multipliers, disease death, save/migration/wake/rehydrate carry, checksum when on, epidemic observables; gates PD-S02, S05–S08 | L | PD-1, PD-D3 |
| PD-3 | Strain mutation, the trade-off curve, recycling, phylogeny, neutral-strain null | M | PD-2 |
| PD-4 | `DefenceGenotype`, inheritance and mutation on the PATHOGEN stream, constitutive and induced costs, vertical transmission; gate PD-S04 | M | PD-2 |
| PD-5 | Environmental load field (water and cells) and the no-op interfaces of §10 | M | PD-2 |
| PD-6 | `ParasiteBurden` with a direct cycle, egg field, `Cure`, `k` observable, Poisson null | M | PD-1, PD-4 |
| PD-7 | `sickness` in the tick payload and the renderer overlay; CM-S10 and the map visual check | S | PD-2 |
| PD-8 | Sickness cue, avoidance and grooming through a versioned brain interface | L | PD-D7, ADR-0017 S2 and D-SIG-2 |
| PD-9 | Micropredator feeding mode (option D) | M | PD-D2 |
| PD-10 | Preregistrations for PATHO-EPI, PATHO-CYC, PATHO-VIR and **PATHO-RQ**, with manifests and fixtures, docs only, before any run | S each | PD-3, PD-4 (and PD-6 for PATHO-CYC) |

Later work waits on other designs: vectors on INVERT-D and ADR-0018 E6; two-host cycles and manipulation
on AQUA-D; parasitoid swarms on ADR-0018 Tier 2; Hamilton–Zuk on V3-SEX and ADR-0017 displays.

## 14. Owner decisions

| ID | Question | Options | Recommendation |
|---|---|---|---|
| **PD-D1** | Order of work | **A:** microparasites first (PD-1→PD-3), then defence, environment, macroparasites. **B:** macroparasites first. | **A.** It answers "bệnh tật" (disease) first, and everything else reuses its stream, law and carry code. |
| **PD-D2** | How parasitic organisms are represented | **A:** fields plus the micropredator mode (D). **B:** Tier-2 parasitoid swarms now. **C:** evolving Tier-1 parasite agents. | **A.** C costs about 3.6× tick time and has nothing to exploit (no nests); B waits on ADR-0018 Tier 2. |
| **PD-D3** | How a disease death is recorded | **A:** an explicit virulence hazard, `DeathCause::Disease`, and a new World Record tag (an ADR-0008 amendment). **B:** an energy drain only, with deaths filed as starvation. | **A.** B mislabels every disease death, as `deaths_pathogen` does today. |
| **PD-D4** | Where the law lives | **A:** `WorldLawSet::pathogen` plus `live.disease`. **B:** only `live.*` keys. | **A.** It is the INVERT-D and ADR-0016 pattern, and the fingerprint is unchanged when off. |
| **PD-D5** | The existing `PathogenTransmission` intervention | **A:** keep it unchanged and add `SeedInfection`, `Cure` and `Vaccinate`. **B:** make it seed infections while the law is on. | **A.** B silently changes the meaning of recorded manifests, ring records and the G5 test. |
| **PD-D6** | Persistence at small N | **A:** report fade-out and allow only declared persistence routes. **B:** re-seed automatically. | **A.** B makes prevalence an artefact. |
| **PD-D7** | Behavioural avoidance and grooming | **A:** a physical sickness cue on ADR-0017 channels, after S2 and D-SIG-2. **B:** an oracle `neighbour_sick` input now. | **A.** B is an oracle input slot, which ADR-0017 rejects. |
| **PD-D8** | Parasite energy | **A:** the drain goes through `total_cost` to detritus, and parasite biomass is not modelled. **B:** a new `Parasites` ledger compartment. | **A.** B needs its own ADR and a ledger change. |
| **PD-D9** | RNG stream | A dedicated `sim_stream::PATHOGEN` with counter-based draws; its number is assigned in one registry at integration | **Yes.** The lead should also settle the existing 6 and 7 collisions (§2). |
| **PD-D10** | Visible sickness cue | **A:** the subtle overlay of §11 plus a UI toggle. **B:** no visual cue. | **A**, prototype first, as the owner prefers for visual work. |

## 15. Unverified

- **No number in this document has been measured in Anima**, and none is calibrated: the tick-cost
  estimate, stage scales, field sizes, strain-table capacity, render limits and card sizes.
- **Exact model details were not checked against the papers:** the matching-key form against Hamilton et
  al. 1990; the resistance/tolerance prevalence prediction of Roy & Kirchner 2000 was not checked at all.
- **Whether the live lineage id is unique per individual and fit to be the `agent_key`** has not been checked. If it is not, a `u64` serial is needed.
- **Whether adding optional snapshot fields needs a schema bump** has not been checked; that is up to `SNAPSHOT_CONTRACT.md`.
- **Whether science runs use dormancy** has not been checked. While an agent is dormant, its disease clock pauses, and each card must say so.

## 16. References

1. Alizon S, Hurford A, Mideo N, van Baalen M (2009). Virulence evolution and the trade-off hypothesis. *J Evol Biol* 22:245–259. https://doi.org/10.1111/j.1420-9101.2008.01658.x
2. Anderson RM, May RM (1978). Regulation and stability of host-parasite population interactions I. *J Anim Ecol* 47:219–247. https://doi.org/10.2307/3933
3. Anderson RM, May RM (1982). Coevolution of hosts and parasites. *Parasitology* 85:411–426. https://doi.org/10.1017/S0031182000055360
4. Bartlett MS (1957). Measles periodicity and community size. *J R Stat Soc A* 120:48–70. https://doi.org/10.2307/2342553
5. Behringer DC, Butler MJ, Shields JD (2006). Avoidance of disease by social lobsters. *Nature* 441:421. https://doi.org/10.1038/441421a
6. Boots M, Mealor M (2007). Local interactions select for lower pathogen infectivity. *Science* 315:1284–1286. https://doi.org/10.1126/science.1137126
7. Cressler CE, McLeod DV, Rozins C, van den Hoogen J, Day T (2016). The adaptive evolution of virulence. *Parasitology* 143:915–930. https://doi.org/10.1017/S003118201500092X
8. Davies NB (2000). *Cuckoos, Cowbirds and Other Cheats*. T & AD Poyser. ISBN 0-85661-135-2.
9. de Roode JC, Yates AJ, Altizer S (2008). Virulence-transmission trade-offs and population divergence in virulence. *PNAS* 105:7489–7494. https://doi.org/10.1073/pnas.0710909105
10. Decaestecker E, et al. (2007). Host–parasite "Red Queen" dynamics archived in pond sediment. *Nature* 450:870–873. https://doi.org/10.1038/nature06291
11. Ebert D (1998). Experimental evolution of parasites. *Science* 282:1432–1436. https://doi.org/10.1126/science.282.5393.1432
12. Ewald PW (1983). Host-parasite relations, vectors, and the evolution of disease severity. *Annu Rev Ecol Syst* 14:465–485. https://doi.org/10.1146/annurev.es.14.110183.002341
13. Godfray HCJ (1994). *Parasitoids: Behavioral and Evolutionary Ecology*. Princeton University Press. ISBN 0-691-00047-6.
14. Hamilton WD (1980). Sex versus non-sex versus parasite. *Oikos* 35:282–290. https://doi.org/10.2307/3544435
15. Hamilton WD, Axelrod R, Tanese R (1990). Sexual reproduction as an adaptation to resist parasites. *PNAS* 87:3566–3573. https://doi.org/10.1073/pnas.87.9.3566
16. Hamilton WD, Zuk M (1982). Heritable true fitness and bright birds: a role for parasites? *Science* 218:384–387. https://doi.org/10.1126/science.7123238
17. Hart BL (1988). Biological basis of the behavior of sick animals. *Neurosci Biobehav Rev* 12:123–137. https://doi.org/10.1016/S0149-7634(88)80004-6
18. Hart BL (1990). Behavioral adaptations to pathogens and parasites: five strategies. *Neurosci Biobehav Rev* 14:273–294. https://doi.org/10.1016/S0149-7634(05)80038-7
19. Hudson PJ, Dobson AP, Newborn D (1998). Prevention of population cycles by parasite removal. *Science* 282:2256–2258. https://doi.org/10.1126/science.282.5397.2256
20. Kavaliers M, Colwell DD (1995). Discrimination by female mice between the odours of parasitized and non-parasitized males. *Proc R Soc B* 261:31–35. https://doi.org/10.1098/rspb.1995.0113
21. Keeling MJ, Grenfell BT (1997). Disease extinction and community size. *Science* 275:65–67. https://doi.org/10.1126/science.275.5296.65
22. Keeling MJ, Rohani P (2008). *Modeling Infectious Diseases in Humans and Animals*. Princeton University Press. ISBN 978-0-691-11617-4.
23. Kermack WO, McKendrick AG (1927). A contribution to the mathematical theory of epidemics. *Proc R Soc Lond A* 115:700–721. https://doi.org/10.1098/rspa.1927.0118
24. Kuris AM, et al. (2008). Ecosystem energetic implications of parasite and free-living biomass in three estuaries. *Nature* 454:515–518. https://doi.org/10.1038/nature06970
25. Lafferty KD, Kuris AM (2002). Trophic strategies, animal diversity and body size. *Trends Ecol Evol* 17:507–513. https://doi.org/10.1016/S0169-5347(02)02615-0
26. Lafferty KD, Morris AK (1996). Altered behavior of parasitized killifish increases susceptibility to predation by bird final hosts. *Ecology* 77:1390–1397. https://doi.org/10.2307/2265536
27. Lipsitch M, Siller S, Nowak MA (1996). The evolution of virulence in pathogens with vertical and horizontal transmission. *Evolution* 50:1729–1741. https://doi.org/10.1111/j.1558-5646.1996.tb03560.x
28. Lively CM (1987). Evidence from a New Zealand snail for the maintenance of sex by parasitism. *Nature* 328:519–521. https://doi.org/10.1038/328519a0
29. Poulin R (2010). Parasite manipulation of host behavior: an update and frequently asked questions. *Adv Study Behav* 41:151–186. https://doi.org/10.1016/S0065-3454(10)41005-0
30. Råberg L, Sim D, Read AF (2007). Disentangling genetic variation for resistance and tolerance to infectious diseases in animals. *Science* 318:812–814. https://doi.org/10.1126/science.1148526
31. Ray TS (1991). An approach to the synthesis of life. In Langton CG et al. (eds), *Artificial Life II*, Addison-Wesley, 371–408.
32. Shaw DJ, Grenfell BT, Dobson AP (1998). Patterns of macroparasite aggregation in wildlife host populations. *Parasitology* 117:597–610. https://doi.org/10.1017/S0031182098003448
33. Sheldon BC, Verhulst S (1996). Ecological immunology: costly parasite defences and trade-offs in evolutionary ecology. *Trends Ecol Evol* 11:317–321. https://doi.org/10.1016/0169-5347(96)10039-2

Related in this repo: [creature development contract](../../reference/CREATURE_DEVELOPMENT_CONTRACT.md),
[ADR-0001](../../decisions/ADR-0001-creature-development-lifecycle.md),
[ADR-0003](../../decisions/ADR-0003-evolved-per-agent-brains.md),
[ADR-0008](../../decisions/ADR-0008-world-past-and-time.md),
[ADR-0013](../../decisions/ADR-0013-live-creature-lifecycle.md),
[ADR-0014](../../decisions/ADR-0014-shared-island-host-and-guests.md),
[ADR-0017](../../decisions/ADR-0017-evolved-communication.md),
[evolution experiment contract](../../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) and
[agent workflow](../../how-to/AGENT_WORKFLOW.md) (QD-SCALE-TRIAL, QD-REALMS, QD-WV2-BIG).
