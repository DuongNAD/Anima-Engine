---
title: ADR-0017 — Evolved Communication & Local Perceptual Channels (Chemical, Acoustic, Visual; Evidence-Gated Language Claims)
status: proposed
owner: simulation-architecture
last_reviewed: 2026-11-25
decision_date: pending
supersedes: none
superseded_by: none
---

# ADR-0017 — Evolved Communication & Local Perceptual Channels

> **Status: `proposed` — awaiting owner acceptance and decisions `D-SIG-1`–`D-SIG-6`.**
> Do **not** begin Rust or frontend implementation of signaling, olfactory wiring, local target
> replacement, or UI communication surfaces until the owner accepts this ADR and resolves
> `D-SIG-1`–`D-SIG-6` (and any card adding new post-freeze `live.*` manifest keys or observables
> must obey the merged `C12 E3B-PIN` / `QD-BIO-D2` partition in
> `src-tauri/tests/prereg_e3b_manifest_tests.rs:84, 120, 574-603, 753-760` and coordinate existing
> fixed `LIVE_KEYS.len() == 31` test assertions).
> Initiated per owner decision **`QD-SIGNALS`** (`MSG_20261124_150000`, `RESEARCH_REVIEW` §3 & §4 Topic 5,
> `QD-SCALE-TRIAL` `154500`). Code anchors verified on `origin/feat/ecological-selection` at
> `a146d2f2aaa6625d1aeae8bb8ab5e94c07f4231a` (shifted from initial `e7b99745` baseline).

---

## 1. Context & Verified Baseline (`a146d2f2`)

Under [`ADR-0003`](ADR-0003-evolved-per-agent-brains.md), Anima moved from one shared neural policy
to per-agent heritable brains (`BrainGenotype`) with an explicit placeholder output slot for signaling,
yet six structural gaps make creatures functionally mute and perceptually omniscient at `a146d2f2`:

1. **Pheromone subsystem is scheduled but unattached (`pheromone.rs`, `simulation_schedule.rs:171-173`):**
   - `PheromoneGrid` (`src-tauri/src/ai/pheromone.rs:5-126`, `GRID_SIZE = 128`, `CELL_COUNT = 16384`,
     `MAX_CONCENTRATION = 10.0`, bilinear sampling `sample_bilinear` at `:75-125`, diffusion/decay
     `update_pheromone_grid_system` at `:185-220`, and telemetry `PheromoneGridState` at `:240-244`
     served read-only by `get_pheromone_grid` at `src-tauri/src/commands/environment.rs:81-91` and
     saved in `SerializedPheromoneGrid` (`src-tauri/src/core/simulation_state.rs:318-322`,
     `serialize_world_state` `:1247-1260`, restored in `simulation_loop.rs:1101-1110`)) is live in the tick schedule via `agent_release_pheromone_system`
     (`pheromone.rs:161-182`), `update_pheromone_grid_system`, and `agent_read_pheromone_system`
     (`pheromone.rs:223-235`) in `build_tick_schedule` (`src-tauri/src/core/simulation_schedule.rs:171-173`).
   - However, **zero** production spawn/birth/restore/migration call sites attach
     `OlfactorySensors` (`pheromone.rs:130-146`) or `PheromoneReleaser` (`pheromone.rs:150-158`): neither
     `spawn.rs:139-143`, `SimulationEngine::start` (`simulation_loop.rs:1680-1706`),
     `live_experiment::genesis` (`live_experiment.rs:3108-3133`), `SpawnOffspringCommand`
     (`agent_systems.rs:494-652`), `SerializedAgent` (`simulation_state.rs:105-145`),
     `spawn_serialized_agent` (`simulation_state.rs:922-1039`), `AgentMigrationData`
     (`components.rs:166-185`), `DormantAgentRecord` (`agent_systems.rs:205-208`), nor
     `ArchivedIndividual` (`aggregate_population.rs:224-236`) attaches them.
   - Moreover, `agent_release_pheromone_system` scales emission by
     `ActionGates::of(gates).pheromone_scale()` (`pheromone.rs:176`, clamping `ActionGates::pheromone_emit`
     at `components.rs:501-548`) with **zero energy charge** (`pheromone.rs:178-179`).
2. **`action_index::SIGNAL` is computed and discarded (`brain_genotype.rs:43`, `agent_systems.rs:1953-1957`):**
   - `EVOLVED_ARCH` (`src-tauri/src/evolution/brain_genotype.rs:50-54`) defines `15 -> 64 -> 64 -> 8`
     with `action_index::SIGNAL = 7` (`:43`, documented `"Reserved for agent-to-agent signalling; nothing reads it yet"`)
     and `ACTION_SLOTS = 8` (`src-tauri/src/core/agent_systems.rs:1486-1499`).
   - `action_resolution_system` (`agent_systems.rs:1911-1989`, `:1953-1957`) copies `0..4` into
     `InertiaComponent::cpg_parameters` and indices `4..=6` into `ActionGates`, while silently
     discarding `response.actions[7]`.
3. **Omniscient target vector removes selection pressure for sensory cues (`agent_systems.rs:1744-1772`, `model.rs:1181-1252`):**
   - `sensory_system` (`agent_systems.rs:1633-1881`) constructs `target_caches` across the whole world
     (`:1677-1688`) and computes `local_target_vec` pointing directly to the globally nearest `Prey`
     (for `Predator`) or `Food` (`:1744-1772`), placing it into `state_arr[0..3]` while olfactory
     readings sit at `state_arr[13..=14]` (`:1824-1846`) alongside a `10.0`-unit raycast
     (`SpatialHashGrid::raycast` at `src-tauri/src/physics/spatial.rs:141-314`).
   - Crucially, `hrrl_learning_system` (`src-tauri/src/ai/model.rs:1082-1350`, `:1181-1270`; and legacy
     path at `ai/model.rs:800-834`) independently rebuilds the exact same global target vector and
     feeds `LastTransitionState`. Changing `sensory_system` alone without matching `hrrl_learning_system`
     would corrupt lifetime-learning `(s, a, r, s')` tuples.
4. **Free predator–prey combat & deterministic actor policies (`world_systems.rs:366-588`):**
   - `combat_system` has two parallel strike paths (`:461-531` when `CombatEvents` is present, and
     `:532-587` when absent) and does not attenuate strike capture under coordinated prey groups or
     aposematic/deceptive displays. Both `BrainModel` and `BrainGenotype` actors terminate in
     **deterministic sigmoids** (`AgentInferenceResponse::actions_are_valid` at
     `agent_systems.rs:1516-1524`), meaning offline information-theoretic causal attribution cannot
     rely on Gaussian policy KL/entropy metrics.
5. **Compute, Freeze Pin & Determinism Envelope (`scaling.md`, `live_experiment.rs`, `observer.rs`):**
   - Measured headless schedule (`docs/paper/data/scaling.md:71-72`) is **`0.3307 ms` p50 at `N = 100`**
     (current `QD-SCALE-TRIAL` island target, `MSG_20261124_154500`) and **`5.6432 ms` p50 at
     `N = 1,000`** (`33.9%` of a `60 Hz` tick, long-term design target). All signaling passes must be
     `O(N)` via grids/`SpatialHashGrid` with `0` per-tick heap allocations.
   - `LIVE_KEYS` (`31` keys at `src-tauri/src/core/live_experiment.rs:280-312`) and observables (`12 + 16 = 28`
     at `:1589-1658`) are frozen for `E3b` by merged `C12 E3B-PIN` (`#260`, `QD-BIO-D2`,
     `src-tauri/tests/prereg_e3b_manifest_tests.rs:84, 120, 574-603, 753-760`); `sim_stream` holds slots
     `1..=5` (`src-tauri/src/core/resources.rs:925-948`, plus `DORMANCY_STREAM = 7` at
     `aggregate_population.rs:190`); `ObserverPolicy::Inhabit { cause_id }` lives at
     `src-tauri/src/core/observer.rs:49-72` (`Inhabit` at `:71`).

---

## 2. Core Architectural Principles (`QD-SIGNALS`)

1. **Evolved, Never Scripted:**
   Signals are raw continuous `[0, 1]` brain outputs (`action_index::SIGNAL` and channel controls);
   percepts are continuous scalar inputs. Engine code defines **no** hardwired semantic signal taxonomy
   (`Alarm`, `MatingCall`, `FoodCall`, `Threat`) and **no** scripted receiver reflex. Functional meanings
   (alarm calls, mate attraction, food recruitment, agonistic threats, mimicry, strategic silence) emerge
   strictly through fitness and kin/mate selection over signal-percept feedback loops.
2. **Opt-In Per Channel; Flag-Off Bit-Identity (`ADR-0008`):**
   Every channel and mechanism (`SIGNAL` activation, olfactory/pheromone wiring, local target
   perception, predator confusion, acoustic events, visual displays) is independently opt-in behind
   world-law / `BrainPolicy` / `live.*` flags defaulting to off/absent (`None`/`false`). With flags
   absent, trajectories across trunk desktop, `E2`, `E3`, `E3b`, and `SEL2-P2` match `e7b99745`
   bit-for-bit (`0` extra `SimRng` draws, `0` tick allocations, `0` canonicalize tag bytes).
3. **Zero Tick Heap Allocations & Bounded `O(N)` Spatial Scaling (`CLAUDE.md`, `QD-SCALE-TRIAL`):**
   Emission, diffusion/decay, sensory sampling, and neighbor lookup must allocate **`0` heap bytes**
   per tick, operating over fixed grids and pre-allocated buffers (`SpatialHashGrid` cell queries with
   declared deterministic tie-breaking, never `O(N^2)` pairwise broadcasts over all entities).
4. **Public Channels, Honest Cost (`total_cost`) & Eavesdropping (`CLAUDE.md`, `world_systems.rs:157`):**
   Every emitted signal (chemical secretion, acoustic call, visual display, plant wound volatile) charges
   a metabolic rate proportional to realized output intensity strictly inside **`total_cost` in
   `metabolic_decay_system`** (`world_systems.rs:157`, flowing via `decay -> respired -> detritus` at
   `:174-177, 222-224`, or open-flow sink `RespiredHeatLoss` under [`ADR-0018`](ADR-0018-ecosystem-scales-and-cycles.md) (`proposed`, `#263` merged)). Direct reserve
   subtraction outside `total_cost` is forbidden. Furthermore, emitted physical fields are public to all
   spatial receivers in range: **predators eavesdrop** on prey calls/trails and prey eavesdrop on predator
   signals or alarm calls, sustaining handicap honesty, stealth, and dishonest mimicry trade-offs.
5. **Evidence Before Claims — `"Signals"`, Never `"Language"` Until Proven (`SIG-S06`):**
   Offline snapshot analysis (`S4`) measures whether evolved signals carry empirical information about
   external states (`I(Signal; Context)`) and causally alter receiver outputs relative to time-shuffled
   and sender-shuffled negative controls. Until preregistered evidence gates pass, UI panels, logs, and
   chronicles **MUST label emitted behavior as `"signals"`** (e.g., chemical/acoustic/visual signal),
   never `"language"`.
6. **Guest Inhabitation Parity & Chat Isolation (`QD-PLAYRULES` #5, `ObserverPolicy::Inhabit`):**
   An observer inhabiting an organism (`ObserverPolicy::Inhabit` at `observer.rs:71`) communicates
   in-world **solely** through that organism's normal signal effectors (`SIGNAL` / channel controls),
   paying the identical `total_cost`, range decay, occlusion, and eavesdropping penalties. Human guest
   text/voice chat belongs strictly out-of-world in UI/network state (`ADR-0012`) and never enters
   ECS components, `PheromoneGrid`, brain inputs, or `WorldRecord` scientific state.

---

## 3. Three Physical Signal Channels (`Chemical -> Acoustic -> Visual`)

| Channel | Physical Propagation & Medium | Brain Interface & Flora Role | Rollout Stage |
|---|---|---|---|
| **Chemical (Pheromones & Volatiles)** | Slow, lingering `128x128` toroidal `PheromoneGrid` (`pheromone.rs:5-126`) diffusion + decay, read bilinearly (`:75-125`) by `OlfactorySensors` left/right offsets (`:130-146`). Extended under `D-SIG-2` with heritable odour signature bands (avoiding oracle kin bits) and grazing-induced plant wound volatiles. | Animal emission scales via `ActionGates::pheromone_scale()` (`:176`) + `SIGNAL` modulation (`S1`–`S2`). Plants (`ADR-0015`, planned) release herbivory wound volatiles on grazing damage (`S8`). | **First (`S1`–`S2`, using existing grid; flora volatiles in `S8` after `ADR-0015` `P1`–`P3`)** |
| **Acoustic** | Fast (same-tick), range-limited radial propagation with inverse-square decay and directional binaural left/right amplitude split, stored in a pre-allocated bounded `AcousticEventBuffer` indexed over `SpatialHashGrid`. | Animal call intensity + spectral tag encoded continuously from evolved output slots; receivers hear summed directional envelope within cutoff radius `R_ac`. | **Second (`S7`, after `S1`–`S4` & `D-SIG-4`)** |
| **Visual Displays** | Instantaneous, line-of-sight gated (`SpatialHashGrid::raycast` at `spatial.rs:141-314` within view cone/radius) surface display scalar modulated by brain output rather than passive body color alone. | Allows aposematic threat displays, courtship displays, or distraction without leaving persistent chemical trails; blocked by terrain/occluders when occlusion lands. | **Second/Third (`S8`, after `S1`–`S4` & `D-SIG-4`)** |

### 3.1. Replacing Omniscient Target Vectors Without Breaking Legacy Experiments (`S3`)

- In `S3` (`local_perception = true`), **preserve `SENSORY_SLOTS = 15`** (`agent_systems.rs:1484`) and
  replace only the global target calculation feeding `state_arr[0..3]` in **both** `sensory_system`
  (`agent_systems.rs:1744-1772`) and `hrrl_learning_system` (`ai/model.rs:1181-1252`) via one shared
  pure helper `compute_local_target_vec(...)`:
  - Candidate targets come strictly from local `SpatialHashGrid` queries (`cell_radius` bounded by view
    range `R_view` and/or forward cone angle per `D-SIG-3`) combined with `OlfactorySensors` bilateral
    gradient (`left_reading - right_reading`), returning `Vec3::ZERO` when no target is locally
    perceptible.
  - **Deterministic tie-break rule:** Compare squared toroidal distance `(dist_sq.to_bits(),
    pos_x.to_bits(), pos_z.to_bits(), agent_generation_or_kind)` — never hash-map iteration or unstable
    `Entity` allocation order unless reconciled identically across runs (`snapshot.rs:785-788`).
  - **Explicit Rejection (`RESEARCH_REVIEW` §4 Topic 5):** Permanent removal of `target_pos` with a
    runtime assertion is **rejected**, as it would break `E2`, `E3`, and `E3b` frozen baselines.

---

## 4. Offline Causal Auditor (`S4`), Confusion (`S6`), Persistence & Stream Isolation

1. **Offline Signal Information & Causal Auditor (`S4`, Zero Tick Cost):**
   - Runs strictly post-hoc over captured checkpoints/snapshots (never inside `build_tick_schedule`),
     recording zero overhead during simulation ticks.
   - Because evolved actors emit deterministic sigmoids (`[0, 1]`), `S4` evaluates:
     1. *Contextual Mutual Information:* binned discrete/KSG estimator `I(SIGNAL_i; Context_i)` across
        predator distance bins, food proximity, mate proximity (`AgentSex` opposite), and internal
        energy state.
     2. *Output-Distance Causal Receiver Response:* Counterfactual intervention substituting sender
        signal intensities seen by nearby receivers with time-shuffled and sender-shuffled null ensembles
        while re-evaluating receiver forward passes (`BrainGenotype::forward_into`), measuring RMS
        action shift `||a(signal) - a(shuffled)||_2` (`cpg_parameters` + `ActionGates`) and downstream
        survival/predation divergence (`p`-values reported honestly against shuffled controls).
2. **Baldwin Plasticity Telemetry (`S5`) & Predator Confusion (`S6`):**
   - `S5` adds an offline/read-only diagnostic comparing `BrainGenotype` (innate) vs `AgentBrain::learned`
     (`components.rs:561-612`) outputs on a fixed standardized sensory probe vector without mutating
     world state.
   - `S6` adds opt-in group-confusion attenuation on `captured` (`predation_capture`) symmetrically in
     **both** branches of `combat_system` (`world_systems.rs:461-531` and `:532-587`) using local
     nearby conspecific/display counts around the target prey (`SpatialHashGrid` neighborhood query).
3. **Persistence (`D01`, `D02`, `D09`) & RNG Stream Isolation:**
   - Any opt-in communication components (`SignalOutputState`, `OlfactorySensors`, `PheromoneReleaser`,
     `DisplayState`, heritable `OdourSignature` bytes, `AcousticEventBuffer`) serialize additively via
     `#[serde(default, skip_serializing_if = "Option::is_none")]` on `SerializedAgent`
     (`simulation_state.rs:105-145`), `AgentMigrationData` (`components.rs:166-185`), `DormantAgentRecord`
     (`agent_systems.rs:205-208`), and `ArchivedIndividual` (`aggregate_population.rs:224-236`), and
     hash inside `world_checksum` (`snapshot.rs:789-886`, sorted per-agent tuple loop `:798-822`) **only** when
     enabled (`0` bytes hashed when disabled).
   - Legacy saves lacking communication fields deserialize cleanly to `None` and match trunk behavior;
     restore/migration/wake/rehydrate carry exact saved sensory/signalling components without re-rolling.
   - Any stochastic option (e.g., stochastic confusion fallback if selected in `D-SIG-5`, or shuffled
     negative control draws offline) uses dedicated `sim_stream::COMMUNICATION` (isolated from
     `resources.rs:925-948` `1..=5`, `V0`, `DORMANCY_STREAM = 7`, and [`ADR-0016`](ADR-0016-changing-world-surface.md)'s `WORLD_DISTURBANCE`).
4. **Explicitly Rejected & Deferred Proposals (`RESEARCH_REVIEW` §3 & §4 Topic 5):**
   - **Rejected:** permanent global target-vector assertion; oracle `RHP` (Resource Holding Potential),
     oracle kin, or oracle bond input slots (kin/mate discrimination must arise via heritable odour cues
     or evolved signals); `MaternalStressMark` (violates `ADR-0003` non-Lamarckian invariant); signal
     metabolic costs outside `total_cost`; and unverified `"language"` labels in UI/docs.
   - **Deferred:** `rtNEAT` and Hebbian synaptic plasticity (`ADR-0003` rewrite out of scope);
     ground-contact CPG feedback (awaits locomotion/physics work); full acoustics (`S7`) & visual/plant
     volatiles (`S8`) staged behind `S1`–`S4`; and shared-island guest rules (Topic-5 draft parameters
     such as `2%` cap / random juvenile / `60s` lock triaged out of `ADR-0017` into the product domain
     of [`ADR-0014`](ADR-0014-shared-island-host-and-guests.md), `accepted`, `#256`, which uses `QD-PLAYRULES` #3/#5 and `ISLAND-D2(a)` exclusive lock).

---

## 5. Open Owner Decision Gates (`D-SIG-1`–`D-SIG-6`)

| D-Slot | Question | Options (Lead Recommendation First) |
|---|---|---|
| **`D-SIG-1`** | `S1`–`S2` Brain `SIGNAL` channel & emission metabolic coefficient (`k_signal`) | **(A) Recommended:** Wire existing `action_index::SIGNAL` (`brain_genotype.rs:43`) directly to pheromone modulation (`S1` read + `S2` opt-in `PheromoneReleaser`/`OlfactorySensors`) with `cost = k_chem * scale + k_acoustic * amp + k_disp * disp` summed cleanly into `total_cost` (`world_systems.rs:157`). **(B)** Introduce a separate multi-dimensional `[SIGNAL_0, SIGNAL_1, SIGNAL_2]` action vector widening `EVOLVED_ARCH` immediately (breaks parity with `ADR-0003` `EVOLVED_ARCH` (`15 -> 64 -> 64 -> 8`) checkpoints). |
| **`D-SIG-2`** | Kin/mate chemical specificity on `PheromoneGrid` | **(A) Recommended:** Start `S2` on existing single-scalar `128x128` `PheromoneGrid`, then add compact `K`-channel signature slots (`K = 2` or `4` pre-allocated toroidal fields or sender-band hash derived from heritable genotype traits) in a scoped follow-up rather than oracle kin tags. **(B)** Keep single scalar pheromone forever (predator/prey/mate smells cannot diverge cleanly). |
| **`D-SIG-3`** | `S3` local target replacement rule (`local_perception = true`) | **(A) Recommended:** Finite local `R_view` `SpatialHashGrid` neighborhood search (optionally angularly weighted to forward hemisphere) + declared `(dist_sq, x, z)` deterministic tie-break, keeping `SENSORY_SLOTS = 15`. **(B)** Remove raycast/target completely and rely 100% on `OlfactorySensors` gradient (harsher starvation collapse risk without progressive calibration). |
| **`D-SIG-4`** | Rollout gating for Phase-2 acoustic (`S7`) & visual/plant volatile (`S8`) channels | **(A) Recommended:** Stage cleanly: merge & calibrate `S1`–`S6` first (`PheromoneGrid` + local target + causal auditor + confusion), then land `S7` (acoustic) & `S8` (visual displays + `ADR-0015` plant wound volatiles). **(B)** Bundle chemical, acoustic, and visual channels into one large initial PR. |
| **`D-SIG-5`** | `S6` predator confusion formulation in `combat_system` | **(A) Recommended:** Deterministic continuous dilution factor `1 / (1 + alpha * (nearby_prey_count - 1).max(0))` modulated by nearby prey `SIGNAL`/display density (using `SpatialHashGrid` query, `0` RNG draws). **(B)** Probabilistic hit/miss roll drawn from dedicated `sim_stream::COMMUNICATION`. |
| **`D-SIG-6`** | Minimum statistical threshold (`SIG-S06`) to unlock `"proto-language"` / `"referential signal"` UI copy (replacing default `"signals"`) | **(A) Recommended:** Require all 3 criteria over `>= 12` seeds (`AE-S14`): (i) significant context mutual information `I(Signal; Context)` exceeding `p < 0.01` vs time/sender-shuffled nulls, (ii) non-trivial causal receiver action shift `||a - a_shuffled||` on causal auditor (`S4`), and (iii) verified fitness advantage or receiver survival improvement over muted/shuffled negative controls. **(B)** Keep `"signals"` permanently across UI regardless of empirical metrics. |

---

## 6. Verification Matrix (`SIG-S01`–`SIG-S08`) & Roadmap

Implementation proceeds strictly through cards **`S1`–`S8`** in
[`docs/ai/planning/2026-11-24-signals-cards.md`](../ai/planning/2026-11-24-signals-cards.md):

| Gate | Positive Assertion | Mandatory Negative Control |
|---|---|---|
| **`SIG-S01`** | With communication/local-perception flags absent/`false`, trunk desktop, `E2`, `E3`, and `E3b` trajectories, `SimRng` draws, `WorldLawSet::baseline().fingerprint()`, and `world_checksum` remain bit-for-bit identical to `e7b99745`; `0` heap allocs in tick (`TrackingAllocator`). | Enable pheromone emission or read `SIGNAL` into `ActionGates` without flag gate -> E2/E3b bit-identity test fails red. |
| **`SIG-S02`** | All chemical/acoustic/display emission energy charges flow exclusively through `total_cost` in `metabolic_decay_system` -> `respired` -> detritus / declared sink (`|closed_eu_residual| / initial <= 1e-9` closed, `|R_open| <= 1e-3` open); eavesdropping predator sensing responds identically to public field. | Deduct emission cost directly from `homeo.energy` outside `total_cost` -> EU closure verifier fails red. |
| **`SIG-S03`** | `S3` `sensory_system` and `hrrl_learning_system` invoke identical `compute_local_target_vec` with finite radius & deterministic tie-break; same-OS multi-process trajectories match identically and zero targets past `R_view`. | Desynchronize `sensory_system` and `hrrl_learning_system` target functions, or place prey outside `R_view` and assert non-zero `local_target_vec` -> parity / locality test fails red. |
| **`SIG-S04`** | Persistence (`SerializedAgent`, `AgentMigrationData`, `DormantAgentRecord`, `ArchivedIndividual`) carries exact signaling components (`OlfactorySensors`, `PheromoneReleaser`, display/signature state); `run(N) == run(K) -> save -> load -> run(N-K)` bit-exact when enabled, legacy `None` saves load unchanged. | Drop `OlfactorySensors` across snapshot save or alter `world_checksum` with flags off -> round-trip / invariance gate fails red. |
| **`SIG-S05`** | Scale & `0`-alloc gate (`QD-SCALE-TRIAL` `N = 100` & design target `N = 1,000`): `schedule_ambiguity_tests` = `0` ambiguous pairs; emission, acoustic buffer, neighbor scan (`S3`, `S6`), and `PheromoneGrid` produce `0` heap allocations per tick (`TrackingAllocator`). | Allocate per-tick neighbor `Vec` inside `S3`/`S6`/`S7` -> zero-alloc test fails red. |
| **`SIG-S06`** | Offline causal auditor (`S4`): synthetic ground-truth sender-receiver checkpoint detects `I(Signal; Context) > 0` and non-zero output-distance receiver action shift relative to time-shuffled and sender-shuffled null ensembles. UI/docs copy gate fails any unverified `"language"` claim. | Feed pure random / time-shuffled signal stream -> auditor reports null significance (zero false positive); inject `"language"` string before `D-SIG-6` threshold pass -> doc/copy lint fails red. |
| **`SIG-S07`** | `S6` predator confusion symmetrically attenuates `combat_system` capture across both telemetry-event and non-event branches; identical world trajectory whether `CombatEvents` resource is inserted or absent. | Apply confusion multiplier in only one branch of `combat_system` -> event vs non-event parity test fails red. |
| **`SIG-S08`** | Phase-2 multi-seed (`S7` acoustic, `S8` visual/plant volatiles) preserves EU/determinism/eavesdropping invariants and guest `ObserverPolicy::Inhabit` parity (`total_cost` & physical occlusion apply identically to guest and AI agents; chat never reaches ECS). | Bypass `total_cost` or range/occlusion checks for `ObserverPolicy::Inhabit` guest -> observer parity test fails red. |

---

## 7. Related Documents

- Core brain, lifecycle, island, ecosystem & determinism ADRs: [`ADR-0001`](ADR-0001-creature-development-lifecycle.md), [`ADR-0003`](ADR-0003-evolved-per-agent-brains.md), [`ADR-0004`](ADR-0004-observer-as-declared-intervention.md), [`ADR-0008`](ADR-0008-world-past-and-time.md), [`ADR-0012`](ADR-0012-ui-free-simulation-core.md), [`ADR-0013`](ADR-0013-live-creature-lifecycle.md) (`accepted`, `#262`, `QD-PLAYRULES` #2 two-sex mate-finding & homologous crossover), [`ADR-0014`](ADR-0014-shared-island-host-and-guests.md) (`accepted`, `#256`, shared islands & guest rules), [`ADR-0015`](ADR-0015-evolving-plants.md) (`proposed`, `#268`, evolving plants & herbivory wound volatiles), [`ADR-0016`](ADR-0016-changing-world-surface.md) (`proposed`, `#271`, changing world surface / geographic divergence), [`ADR-0018`](ADR-0018-ecosystem-scales-and-cycles.md) (`proposed`, `#263`, `OpenEnergyFlowLaw` & `RespiredHeatLoss`), [`ADR-0019`](ADR-0019-brain-body-interface.md) (`accepted`, `#259`, `QD-BRAINBODY`, brain-body interface), [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md), [`scaling.md`](../paper/data/scaling.md)
- Implementation cards: [`2026-11-24-signals-cards.md`](../ai/planning/2026-11-24-signals-cards.md), [`2026-11-24-island-cards.md`](../ai/planning/2026-11-24-island-cards.md), [`2026-11-24-ecosystem-cards.md`](../ai/planning/2026-11-24-ecosystem-cards.md), [`2026-11-24-plant-cards.md`](../ai/planning/2026-11-24-plant-cards.md)
