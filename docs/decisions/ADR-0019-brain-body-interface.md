---
title: ADR-0019 — Brain–Body Interface (Shared Motor Modulation + Per-Segment Gene for Variable Morphologies)
status: accepted
owner: simulation-architecture
last_reviewed: 2026-11-24
decision_date: 2026-11-24
supersedes: none
superseded_by: none
---

# ADR-0019 — Brain–Body Interface (Per-Segment CPG Drive for Variable Morphologies)

> **Status: `accepted` (2026-11-24 — owner decision `QD-BRAINBODY` `D1–D5 = A`, "Đồng ý" on the lead's summary of Option A).**
> Initiated per `QD-BIO-D4` (`MSG_20261124_133000`, `PR #255`) from `RESEARCH_REVIEW_2026-11-24_modern_biology.md` §2 Finding 2;
> accepted in chat to the lead on `2026-11-24` and recorded via `MSG_20261124_143000` (`## 9`).
> Implementation cards follow separately after PR `#259` merges and wait for `C12 E3B-PIN` (`QD-BIO-D2`) because `live.brain_body_interface` adds a new `live.*` key.
> Code anchors below are verified on `origin/feat/ecological-selection` at commit
> `facc800c50a15cdca960c8c006cc5f861e4c703a`.

---

## 1. Context

When [`ADR-0003`](ADR-0003-evolved-per-agent-brains.md) established evolved per-agent brains and
[`ADR-0006`](ADR-0006-locomotion-actuation.md) wired CPG amplitude into locomotion speed, all
founder morphologies had 3 nodes (`[0, 1, 2]`), and the interface between the brain's actor head
and the body's joint oscillators remained fixed at `[f32; 4]`. Auditing that path against
variable morphologies, physical growth (`ADR-0013` physical growth amendment), and upcoming
homologous crossover exposes a structural bottleneck and an active wiring defect:

### 1.1. The Double CPG-Slot Defect on Trunk `facc800c`

1. **Hard ceiling at 2 `(frequency, amplitude)` pairs (`core/components.rs:466`, `core/agent_systems.rs:1883-1909`):**
   - `InertiaComponent::cpg_parameters` is fixed-size `[f32; 4]` (`core/components.rs:466`), holding
     only `action_index::CPG_LEN = 4` floats (`evolution/brain_genotype.rs:38`): two `(freq, amp)`
     channels indexed at `seg_idx * 2` and `seg_idx * 2 + 1` inside `apply_inertia_to_oscillators`
     (`core/agent_systems.rs:1897-1908`).
   - Every child segment at sorted index `seg_idx >= 2` receives `cpg_parameters.get(freq_idx) == None`
     and `None` for amplitude, so it is **never** modulated by the brain (or by `Inhabit` controls).
     It permanently runs the hardcoded literal `CpgOscillator::new(1.0, 0.5)` assigned at decode time
     (`evolution/genotype.rs:253`). Morphology mutation can already grow trees up to `15` nodes
     (`evolution/mutation.rs:163`), and snapshot/migration validation permits up to
     `MAX_MIGRATION_MORPHOLOGY_NODES = 128` (`core/components.rs:192`); segments `>= 2` are dead
     weight as far as neural control is concerned.
2. **Root entity silently consumes `seg_idx = 0` (`evolution/genotype.rs:195`, `core/agent_systems.rs:1891-1898`):**
   - `decode_genotype` spawns the root entity **without** a `CpgOscillator` (`evolution/genotype.rs:156-189`),
     yet attaches `ParentAgent(root_entity)` to it (`genotype.rs:195`) alongside every child segment
     (`genotype.rs:254`).
   - `apply_inertia_to_oscillators` queries `segment_query` (`(Entity, &ParentAgent, &Segment)`),
     collects every entity matching `parent.0 == agent_entity` (`agent_systems.rs:1891-1895`),
     and sorts `child_buf` ascending by `segment.id` (`:1896`).
   - Because `root_node.id` (`0` in genesis founders and post-mutation root lineages) is strictly
     less than its children's IDs (`1, 2, ...`), `root_entity` takes **`seg_idx = 0`**, fails
     `oscillator_query.get_mut(seg_entity)` (no `CpgOscillator`), and silently **discards**
     `cpg_parameters[0..2]`.
   - Even on the legacy 3-segment founder (`nodes [0, 1, 2]`), only **one** child (`id = 1`, at
     `seg_idx = 1`) receives brain parameters (`cpg_parameters[2..4]`), while child `id = 2`
     remains permanently stuck at `(frequency = 1.0, amplitude = 0.5)`. On the 2-segment founder
     (`live.founder_genotype = 1.0`, `nodes [0, 1]`), only `cpg_parameters[2..4]` controls the sole
     child joint while `cpg_parameters[0..2]` are completely unused outputs.

### 1.2. Downstream Consequences on Behavior, Morphology & Future Evolution

1. **Gait modulation & locomotion speed (`core/locomotion.rs:175-186, 320`):**
   `drive_from_cpg` computes `speed` by averaging `osc.amplitude.abs() / CPG_AMPLITUDE_SCALE` over
   direct root `ChildrenLinks` (`locomotion.rs:175-186`). Uncontrolled child joints pinned at
   `amplitude = 0.5` dilute brain speed modulation, and branching subtrees beyond depth `1` are
   not averaged in `ChildrenLinks`.
2. **No inter-segment phase lag / traveling waves (`ai/cpg.rs:11-26`, `physics/dynamics.rs:69-77`):**
   `CpgOscillator::new` sets `phase = 0.0` on every child segment (`ai/cpg.rs:14`). With shared or
   single-channel parameters and zero per-joint phase offset, multi-segment organisms exhibit
   in-phase standing oscillations around fixed joint axes rather than coordinated metachronal /
   traveling waves — the exact failure mode [`ADR-0006`](ADR-0006-locomotion-actuation.md)
   documented when anisotropic drag produced zero propulsion under standing 3-segment oscillation.
3. **Co-evolution evidence already observed (`ADR-0003` EB-S09):**
   [`ADR-0003`](ADR-0003-evolved-per-agent-brains.md) (`lines 118-130`) measured a 53%
   displacement reversal between 2/5/8-segment bodies (gait #2 best) and 3-segment bodies (gait #1
   best). Variable morphology cannot co-adapt locomotion without a morphology-independent
   brain–body interface.

### 1.3. Cross-Cutting Invariants That Constrain the Design

1. **Fixed brain shape, Burn parity, and snapshot compatibility ([`ADR-0003`](ADR-0003-evolved-per-agent-brains.md)):**
   - `EVOLVED_ARCH = ArchSpec { inputs: 15, hidden: 64, outputs: 8 }` (`5,769` parameters,
     `BRAIN_GENOTYPE_VERSION = 1`, `evolution/brain_genotype.rs:29-54`) matches Burn's
     `ActorCriticModel` topology (`I -> H -> H -> {actor, critic}`). Gate **EB-S02**
     (`brain_genotype.rs:14, 123-126`, `ai/model.rs:64, 383, 546`) asserts exact forward-pass
     parity between Burn and hand-written inference.
   - `crossover_brains` (`brain_genotype.rs:700`) and `mutate` assume compatible architectures.
   - Snapshot **`SCHEMA_VERSION = 8`** (`core/snapshot.rs:58`, `MIN_SUPPORTED_SCHEMA = 6` at `:68`)
     and `AgentMigrationData` (`MAX_MIGRATION_BRAIN_PARAMETERS = 16_384`, `core/components.rs:196`)
     must continue loading pre-ADR-0019 saves (`D09` backward compatibility).
2. **Somatic physical growth (`QD-LIFE`, physical growth section in [`ADR-0013`](ADR-0013-live-creature-lifecycle.md)):**
   Development of topology stays once-at-birth ([`ADR-0001`](ADR-0001-creature-development-lifecycle.md),
   [`CREATURE_DEVELOPMENT_CONTRACT.md`](../reference/CREATURE_DEVELOPMENT_CONTRACT.md)), while
   juveniles physically grow in segment `length`, `radius`, `mass`, and `JointConstraint.anchor_offset`
   toward adult size during life. The brain–body interface **must not** depend on segment
   metric size or mutate neural architecture/weights during juvenile maturation.
3. **Homologous crossover (`mutation.rs:167`, `crossover.rs:72-81, 127-139`):**
   Structural add-node mutation already mints stable inherited node IDs (`reserve_node_ids`,
   `mutation.rs:167`). Today `crossover_genotypes` discards original node IDs by reserving fresh
   IDs for grafted subtrees (`crossover.rs:72-81, 127-139`); future homologous crossover (`B7` in
   `QD-PLAYRULES`) preserves homology by node/edge identity. A brain–body interface indexed by
   contiguous array slot `0..K` or dense brain-weight indices breaks whenever body topology swaps
   homologous limbs unless motor parameters live cleanly with the morphological genes or map
   deterministically without brain-tensor surgery.
4. **Budget & determinism invariants:**
   Measured headless schedule cost is already **`5.643 ms` p50 at `N = 1,000`** (`34%` of a `60 Hz`
   frame on `i5-14600KF`, growing ~`N^1.86`, [`docs/paper/data/scaling.md`](../paper/data/scaling.md)).
   Every tick must remain `0`-allocation, strictly ordered in `build_tick_schedule`, closed-EU
   (`total_cost` only), non-Lamarckian, and **bit-identical** on trunk baselines (including E3, E3b,
   and `SEL2-P2`) whenever the opt-in flag is off.

---

## 2. Decision Drivers

1. **Bit-identical legacy default:** With `BrainBodyPolicy` absent or `enabled == false` (and on
   all legacy saves without new fields), trajectories and `world_checksum` (`snapshot.rs:789`) MUST
   remain bit-for-bit identical to trunk `facc800c` (zero RNG draws added when off).
2. **Every joint in every valid morphology (`2..=128` nodes) is controllable:** Eliminates both the
   `seg_idx = 0` root-slot bug and the `seg_idx >= 2` hardcoded-oscillator wall without dropping
   branches or capping body size at `2` limbs.
3. **Brain topology stability (`EB-S02` & Schema 8):** Preserves `EVOLVED_ARCH (15 -> 64 -> 64 -> 8)`
   or documents exact `BRAIN_GENOTYPE_VERSION` / schema impact if head size changes; must keep
   `ActorCriticModel` Burn parity testable.
4. **Zero-allocation, `O(segments)` tick execution:** Fits inside the `5.643 ms p50 @ N = 1,000`
   headless budget without spawning secondary neural networks per segment.
5. **Growth & homologous crossover orthogonality:** Works identically as juvenile bodies scale up
   from birth to adult (`ADR-0013` amendment), and travels naturally with morphological subtrees
   when future homologous crossover preserves inherited node IDs (`mutation.rs:167` / `crossover.rs`).
6. **Non-Lamarckian inheritance & D01/D02 persistence:** Effective per-joint modulation belongs to
   genotype/runtime cleanly; lifetime learning never overwrites heritable morphology or brain genes;
   restore, migration, hibernate/wake, and rehydrate preserve active oscillator/gene state verbatim.
7. **Inhabit readiness (`QD-SHAREDISLAND`):** An external `Inhabit` control override writing the
   agent's motor command drives the *entire* organism coherently without needing per-limb manual
   UI sliders.

---

## 3. Options Considered

### Option A — Fixed Brain Shape + Shared 2-Channel Motor Modulation + Per-Edge Heritable `JointCpgGene` *(Recommended)*

Keep **`EVOLVED_ARCH = 15 -> 64 -> 64 -> 8`** (`cpg_parameters: [f32; 4]`) intact as **two shared
descending motor drive channels** `(freq_A, amp_A)` and `(freq_B, amp_B)`, and attach an additive
heritable local coupler **`JointCpgGene`** directly onto each morphological edge
(`MorphologyEdge`, `#[serde(default, skip_serializing_if = "Option::is_none")]`):

- **Data model (`evolution/genotype.rs`, `ai/cpg.rs`, `#[serde(default)]`):**
  ```rust
  pub struct JointCpgGene {
      /// Channel mix in [0.0, 1.0]: 0.0 follows brain channel A (cpg[0..2]),
      /// 1.0 follows brain channel B (cpg[2..4]); intermediates linearly blend both.
      pub channel_mix: f32,
      /// Phase offset relative to the agent's oscillator clock, in [-PI, PI] radians.
      pub phase_offset: f32,
      /// Intrinsic frequency multiplier around neural drive, clamped in [0.25, 2.0].
      pub freq_scale: f32,
      /// Intrinsic amplitude gain around neural drive, clamped in [0.0, 1.5].
      pub amp_gain: f32,
  }
  ```
  - Absent (`None`, default on legacy genotypes/saves): treated as legacy defaults (`freq_scale = 1.0`,
    `amp_gain = 1.0`, `phase_offset = 0.0`, deterministic legacy channel or `channel_mix = 0.0` when
    opt-in policy is active) — zero wire change for legacy saves (`D09`).
- **Actuation mapping (inside `apply_inertia_to_oscillators`, behind opt-in `BrainBodyPolicy`):**
  1. Skip the root entity explicitly (`Without<ParentLink>` or only iterate entities that actually
     carry `CpgOscillator` — **fixing the `seg_idx = 0` slot loss** without touching `segment_query`
     when `BrainBodyPolicy` is off).
  2. For every child segment `e` with `CpgOscillator` and `JointCpgGene`:
     - Blend the brain's two motor channels (`0..2` and `2..4` of `cpg_parameters`):
       $$\text{freq\_drive} = (1 - m)\cdot c_0 + m \cdot c_2, \qquad \text{amp\_drive} = (1 - m)\cdot c_1 + m \cdot c_3$$
       where $m = \text{clamp}(\text{channel\_mix}, 0, 1)$.
     - Map into valid oscillator parameters (matching `agent_systems.rs:1902-1905` baseline scale):
       $$\omega = \text{clamp}\big((0.1 + 2.9 \cdot \text{freq\_drive}) \cdot \text{freq\_scale},\ 0.05,\ 6.0\big)$$
       $$A = \text{clamp}\big((1.5 \cdot \text{amp\_drive}) \cdot \text{amp\_gain},\ 0.0,\ 2.25\big)$$
     - Keep `osc.phase` evolving smoothly on `update_cpg_system` (`ai/cpg.rs:21-25`) and apply
       `phase_offset` deterministically at waveform read (`(self.phase + self.phase_offset).sin()`),
       preventing instantaneous phase snaps when `frequency` updates asynchronously at inference
       boundaries.
- **Why this cleanly resolves all three coupling dimensions:**
  - **Brain shape / `EB-S02` / Schema 8:** Brain stays `15 -> 64 -> 64 -> 8` (`BRAIN_GENOTYPE_VERSION = 1`,
    `SCHEMA_VERSION = 8` preserved). `ActorCriticModel` Burn parity gate **EB-S02**, `crossover_brains`,
    A2C 4-slot transition updates (`agent_systems.rs:1962-1965`), and `Inhabit` 4-float motor control
    work unchanged across bodies of `2` to `128` segments!
  - **Somatic growth (`ADR-0013` amendment):** Because `JointCpgGene` encodes dimensionless phase lag,
    channel routing, and frequency/amplitude scaling, juvenile growth scaling `length`/`radius`/`mass`
    requires zero brain weight adaptation and no phase/gene recomputation during maturation.
  - **Homologous crossover (`mutation.rs:167` / future `crossover.rs`):** Every limb edge carries its
    own `JointCpgGene` **inside `MorphologyEdge`** (`get_subtree` / `Mutation` operator 2 / operator 3).
    Grafting a fin or leg subtree from Parent B automatically transplants its phase relationship and
    motor channel assignment without breaking any neural output index alignment! Adding a new segment
    via operator 3 clones/perturbs its parent edge's `JointCpgGene` using the same mutation RNG
    stream (zero extra RNG draws when `BrainBodyPolicy` is off).

### Option B — Per-Segment Brain Outputs (Two Sub-Variants)

Directly expand the actor head so neural inference outputs dedicated `(freq_i, amp_i)` per child
segment:

- **Sub-Option B1: Dynamic `ArchSpec.outputs = 4 + 2 * num_child_segments` (variable per morphology).**
  - Adding/removing a leaf node (`mutation.rs:161-230`) changes `MorphologyGenotype.nodes.len()`, forcing
    structural surgery on `BrainGenotype` (appending/deleting rows of `actor_w` & `actor_b` at birth).
  - Breaks `crossover_brains` (`brain_genotype.rs:700`) whenever parents have different segment counts,
    requires rewriting `EVOLVED_ARCH` / `EB-S02` per-N, and requires extending shared A2C learner head
    dimensions (`ActorCriticModel`).
- **Sub-Option B2: Fixed Padded Output Head (e.g. `MAX_BRAIN_SLOTS = 14`, `outputs = 32` -> `7,329` params).**
  - Keeps `ArchSpec` uniform for all `<= 15`-node morphologies, still within `MAX_MIGRATION_BRAIN_PARAMETERS = 16_384`.
  - **Severe weaknesses:**
    1. **Requires bumping `BRAIN_GENOTYPE_VERSION` (`1 -> 2`) and breaking `EVOLVED_ARCH` / Burn `ActorCriticModel` parity** (`8` -> `32` outputs + `LastTransitionState` `action: [f32; 4]` expansion + shared-learner burnpack schema check).
    2. **Slot-mapping fragility across structural mutation & homologous crossover:** Mapping slot `i` by sorted `segment.id` shifts every downstream joint's brain channel whenever crossover preserves parent B's arbitrary `node_id_counter` values or grafts a subtree; mapping by contiguous `seg_idx` scrambles meanings on middle-branch insertion/deletion; unused slots `(K..14)` accumulate unpenalized genetic drift unless masked and purged cleanly.
    3. **Hard-caps brain control again at `14` child joints** (`MAX_MIGRATION_MORPHOLOGY_NODES = 128` still leaves >100 segments uncontrolled).

### Option C — Indirect Spatial Encoding (`CPPN` / `HyperNEAT` Substrate Query or Per-Joint Sub-Policy, [`ADR-0003`](ADR-0003-evolved-per-agent-brains.md) Option D)

Evaluate a spatial CPPN `(anchor_src, anchor_dst, depth) -> (phase, freq, amp)` or run a shared
neural policy per joint across the morphological graph.

- **Pros:** Arbitrary spatial symmetry and smooth body-size scaling for very large bodies.
- **Cons & Risks:** Adds `K×` neural forward passes per agent (or CPPN substrate pre-pass at birth +
  per-tick conditioning), eroding the `5.643 ms p50 @ N = 1,000` headless ceiling; abandons fixed
  `ActorCriticModel` `EB-S02` parity; introduces co-optimization premature-convergence risk cited in
  [`ADR-0003`](ADR-0003-evolved-per-agent-brains.md) (`lines 108-114`). Deferred until empirical
  multi-segment evidence (`Option A`) shows shared 2-channel + per-edge gene expression is insufficient.

### Option D — Status Quo (No Change)

Leave `cpg_parameters: [f32; 4]` with root consuming `seg_idx = 0` and segments `>= 2` frozen at
`(1.0, 0.5)`.

- **Cost:** Unacceptable once multi-segment body evolution, physical growth (`ADR-0013`), or
  heterogeneous island species activate — leaves up to 13 of 14 joints non-evolvable in motor
  pattern and wastes 50% of existing neural CPG capacity even on 2- and 3-segment founders. Rejected.

### Side-by-Side Comparison Matrix

| Criterion | **Option A: 2-Channel + `JointCpgGene`** *(Rec.)* | **Option B1: Variable Outputs** | **Option B2: Padded Head (`32` out)** | **Option C: CPPN / Per-Joint Net** |
|---|---|---|---|---|
| All `2..128` segments controlled | **Yes (unbounded tree)** | Yes (weight surgery) | Capped at `14` children | Yes |
| Fixes `root` consuming `seg_idx = 0` | **Yes (opt-in)** | Yes (opt-in) | Yes (opt-in) | Yes |
| Traveling waves / phase offsets | **Yes (`phase_offset` per edge)** | Only if phase outputs added | Only if phase outputs added | Yes |
| `EVOLVED_ARCH (15->64->64->8)` & `EB-S02` parity | **Preserved verbatim** | Broken per topology | Broken (`8 -> 32` outputs) | Broken / new CPPN |
| `SCHEMA_VERSION = 8` save compat (`D09`) | **Exact (`#[serde(default)]` field)** | Needs brain weight migration | Needs `BRAIN_GENOTYPE_VERSION = 2` | New genotype format |
| Compatible with somatic growth (`ADR-0013`) | **Yes (dimensionless edge genes)** | Yes | Yes | Yes |
| Compatible with homologous crossover (`B7`) | **Yes (genes travel on subtree edges)** | Breaks when `K_A != K_B` | Scrambles node-slot indices | Needs spatial matching |
| Zero-alloc tick cost (`N = 1,000`) | **Minimal (`O(segs)` float math)** | Higher + variable tensor shape | `+27%` brain params (`7,329`) | High (`K×` inference passes) |
| Bit-identical legacy default | **Yes** | Yes | Yes | Yes |

---

## 4. Proposed Decision & Open Owner Decision Gates (`QD-BRAINBODY` `D1–D5`)

**Recommendation:** Adopt **Option A (Fixed Brain Shape + Shared 2-Channel Motor Drive + Per-Edge
Heritable `JointCpgGene`)** gated behind an additive `BrainBodyPolicy` (`live.brain_body_interface = 1.0`,
default off/`0.0`), leaving existing trunk/E2/E3/E3b/`SEL2-P2` trajectories bit-for-bit identical.

Implementation cards wait until the owner explicitly accepts this ADR and resolves **`D1–D5`**:

| Gate | Question | Options | Recommendation |
|---|---|---|---|
| **D1** | Brain–body actuation architecture | **A:** Keep `EVOLVED_ARCH (15->64->64->8)` + shared 2-channel `(A, B)` modulation + per-edge `JointCpgGene` (`channel_mix`, `phase_offset`, `freq_scale`, `amp_gain`)<br>**B:** Fixed padded actor head (`B2`, `32` outputs) driving child joints by slot<br>**C:** CPPN / per-joint controller | **A** — keeps `EB-S02` parity, Schema 8, `Inhabit` simplicity, and makes motor genes naturally homologous on morphological subtrees |
| **D2** | Legacy root-entity slot bug & rollout mode | **A:** Opt-in ONLY (`BrainBodyPolicy.enabled = true`, `live.brain_body_interface = 1.0`); flag off preserves exact trunk bug-for-bug (`root` skips slot `0`, `id >= 2` fixed `(1.0, 0.5)`)<br>**B:** Unconditionally fix root skipping `CpgOscillator` on trunk baseline | **A** — strict contract compliance: off/absent keeps E3/E3b/`SEL2-P2` and all trunk checksums bit-identical |
| **D3** | Number of shared brain CPG channels | **A:** `2` channels (`cpg_parameters[0..4]` `(freq_A, amp_A, freq_B, amp_B)`) modulated by `channel_mix in [0, 1]` on each edge — **zero** change to `EVOLVED_ARCH` (`8` outputs)<br>**B:** Keep 2 pairs + add dedicated neural phase-shift output (bumps outputs `8 -> 9`, `BRAIN_GENOTYPE_VERSION = 2`) | **A** — `2` independent frequency/amplitude channels blended per limb (e.g., left/right or axial/appendage specialization) with per-edge evolvable phase lag solves gait coordination without touching `ArchSpec` |
| **D4** | Multi-segment locomotion aggregation in `steering_drive_system` (`locomotion.rs:175`) | **A:** When `BrainBodyPolicy.enabled`, compute `drive_from_cpg` over **all** descendant segments belonging to `ParentAgent(agent_entity)` (using mass-weighted or uniform mean across actual oscillating joints, deterministic sorted `segment.id` sum order) instead of depth-1 `ChildrenLinks` only<br>**B:** Keep depth-1 `ChildrenLinks` average when enabled | **A** — deeper branches (`depth >= 2`) must contribute to thrust/speed proportionally; deterministic `segment.id`-sorted summation prevents float drift across restore/table swaps (F-040) |
| **D5** | Coupling with homologous crossover (`B7`) | **A:** Land `BrainBodyPolicy` + `JointCpgGene` (`morphological mutation` operators 2/3 evolve `JointCpgGene`) on existing `crossover.rs` first, and have later `B7` homologous crossover preserve matching `MorphologyNode.id` / edge IDs cleanly<br>**B:** Defer `ADR-0019` implementation until `B7` homologous crossover lands | **A** — `JointCpgGene` lives inside `MorphologyEdge`, so it immediately improves asexual reproduction and current grafting crossover, and automatically becomes homologous when `B7` stops renumbering grafted subtrees |

---

## 5. Consequences

### Positive

- **Zero neural ABI breakage:** `EVOLVED_ARCH`, Burn `ActorCriticModel`, `EB-S02` parity tests,
  `BRAIN_GENOTYPE_VERSION = 1`, `LastTransitionState`, and snapshot `SCHEMA_VERSION = 8` all stay
  valid; no burnpack weights are invalidated.
- **Unlocks real morphological–behavioral co-evolution:** Multi-segment creatures (`3..=15` nodes in
  evolution, up to `128` validated) can evolve traveling waves, regional specialization (`channel_mix`),
  and bilateral/axial coordination instead of fixed-axis `1.0 Hz / 0.5 rad` quivering.
- **Clean foundation for growth & shared islands:** Juvenile scaling under the `ADR-0013` growth
  amendment changes segment geometry continuously without invalidating neural control, and `Inhabit`
  guests (`QD-SHAREDISLAND`) drive coherent multi-limbed organisms through the same 4 CPG slots.

### Negative / Risks & Mitigations

1. **Smooth phase continuity on `frequency` changes (`ai/cpg.rs:21-25`):**
   - *Risk:* If `phase_offset` were added incrementally into `osc.phase` every inference tick, or if
     `osc.frequency` jumped discretely every interval without retaining phase state, joints could
     twitch noisily.
   - *Mitigation:* Store `phase_offset: f32` directly on `CpgOscillator` (default `0.0`, `#[serde(default)]`
     in `CpgOscillatorState` at `simulation_state.rs:88`), advance `osc.phase` strictly via `2π * freq * dt`
     in `update_cpg_system`, and compute `osc.output = osc.amplitude * (osc.phase + osc.phase_offset).sin()`.
     Serialized `SerializedSegmentState` preserves both `phase` and `phase_offset` verbatim (`D01`/`D02`).
2. **RNG stream contamination if `JointCpgGene` initializes on legacy paths:**
   - *Risk:* Drawing random `JointCpgGene` fields during baseline genesis or operator 2/3 mutation
     when `BrainBodyPolicy` is disabled would alter `SimRng` trajectories even if oscillators ignored
     the gene.
   - *Mitigation:* **Strict zero-draw rule when off:** keep `edge.cpg_gene = None` and draw **zero**
     RNG values for `JointCpgGene` unless `BrainBodyPolicy.enabled == true` (and in `WorldLawSet`,
     derive/consume via the morphology mutation/genesis stream strictly behind the opt-in gate).
3. **`world_checksum` coverage (`core/snapshot.rs:789`):**
   - *Risk:* If save/load lost per-segment `phase_offset` / updated oscillator state under
     `BrainBodyPolicy.enabled`, trajectory divergence after restore could go undetected by `world_checksum`
     alone (which currently hashes `HomeostaticState` and root `Position`, `snapshot.rs:800-814`).
   - *Mitigation:* When `BrainBodyPolicy.enabled`, include deterministic sorted `MorphologyEdge::cpg_gene` /
     oscillator state hash inside the policy-gated `world_checksum` extension (mirroring `ADR-0013`'s
     gated checksum pattern), while hashing **nothing** extra when disabled so E2/E3/E3b checksums
     never move.
4. **Energy accounting (`total_cost`):**
   - Actuating more limbs must never create free speed boost or bypass metabolic costs: keep speed
     bounded by `PREY_MAX_SPEED` / `PREDATOR_MAX_SPEED` (`locomotion.rs:51-53`) with `drive_from_cpg`
     normalized to `[0.0, 1.0]` (`locomotion.rs:185`), and route activity cost strictly through
     `total_cost` in `metabolic_decay_system`.

---

## 6. Implementation, Compatibility & Rollback Plan (Post-Acceptance)

*(Implementation cards begin only after owner accepts `ADR-0019` and sets `D1–D5`; scheduled in
Lane W after Island Milestone 1 cards `I1–I11` per `MSG_20261124_133000` §4.)*

1. **Additive schema & gene types (zero behavior change):**
   - Add `JointCpgGene` and `#[serde(default, skip_serializing_if = "Option::is_none")] pub cpg_gene: Option<JointCpgGene>`
     to `MorphologyEdge` (`evolution/genotype.rs:13`) plus validation bounds in `validate_scientific_agent_state`
     (`core/components.rs:198+`: finite values inside `[0,1]`, `[-PI, PI]`, `[0.25, 2.0]`, `[0.0, 1.5]`).
   - Add `#[serde(default)] pub phase_offset: f32` to `CpgOscillator` and `CpgOscillatorState`
     (`ai/cpg.rs:4`, `core/simulation_state.rs:88`). Legacy JSON/bincode without these fields decodes
     cleanly to `None` / `0.0`.
   - Add `BrainBodyPolicy` resource (`enabled: bool`, default `false`) + manifest key `live.brain_body_interface`
     and update `InitialConditionSet` / snapshot manifest metadata (`LIVE_MODEL_VERSION` incremented only
     if live physics behavior changes when enabled, or version-gated alongside `BrainBodyPolicy`).
2. **Decode, actuation & steering wiring (behind `BrainBodyPolicy.enabled`):**
   - `decode_genotype` initializes `CpgOscillator::phase_offset` from matching `edge.cpg_gene` when
     present (`0.0` otherwise).
   - `apply_inertia_to_oscillators`: when `!policy.enabled`, run exact legacy loop (`agent_systems.rs:1890-1909`);
     when `policy.enabled`, filter out root (`Without<ParentLink>` / only entities holding `CpgOscillator`),
     look up each child edge's `JointCpgGene`, and compute blended `(frequency, amplitude)` for every joint.
   - `drive_from_cpg` (`locomotion.rs:175`): when `policy.enabled` and `D4 = A`, sum over all
     oscillating descendants of `ParentAgent(agent_entity)` in ascending `segment.id` order (via reused
     `Local<Vec<...>>`, zero allocations).
3. **Evolution operators (behind `BrainBodyPolicy.enabled`):**
   - Operator 2 (`mutation.rs:135`) and Operator 3 (`mutation.rs:161`) initialize/mutate `edge.cpg_gene`
     strictly when morphological/brain-body evolution is enabled (leaving legacy runs untouched).
   - Future homologous crossover (`B7` / `QD-PLAYRULES`): once `crossover.rs` retains homologous IDs
     instead of unconditionally renumbering grafted subtrees (`crossover.rs:72-81, 127-139`), each
     grafted `MorphologyEdge` carries its parent's `cpg_gene` intact.
4. **Restore, migration, hibernate/wake, rehydrate:**
   - All 7 spawn paths enumerated in [`CREATURE_DEVELOPMENT_CONTRACT.md`](../reference/CREATURE_DEVELOPMENT_CONTRACT.md)
     (`simulation_loop.rs`, `live_experiment.rs`, `SpawnOffspringCommand`, `SpawnGenotypeCommand`,
     `spawn_serialized_agent`, `SpawnMigrationCommand`, `RehydrateCommand`) preserve `MorphologyEdge::cpg_gene`
     and `CpgOscillator::phase_offset` without re-sampling genesis defaults.
5. **Rollback:** Keep `#[serde(default)]` deserializers indefinitely and disable `BrainBodyPolicy`
   (`live.brain_body_interface = 0.0` / absent) to revert immediately to trunk `facc800c` behavior.

---

## 7. Proposed Verification Gates (`BB-S01`–`BB-S07`)

| Gate | Requirement & Test Verification | Acceptance Criterion | Status |
|---|---|---|---|
| **BB-S01** | **Legacy flag-off bit-identity:** Run `LiveExperimentAdapter` with `BrainBodyPolicy` absent/`false` vs pre-change trunk across default, `SEL2-P2` 2-seg, 3-seg evolved-brain, and turnover seeds (`0..18,000` ticks) | Identical `world_checksum` at every checkpoint; `0` extra RNG draws | proposed |
| **BB-S02** | **All child joints actuated (`2..=15` nodes) + no root slot loss:** With `BrainBodyPolicy` enabled, assert root never consumes a channel slot and every child joint (`seg_idx = 0..K-1`, including `K >= 3`) receives modulated `(freq, amp)` distinct from literal `(1.0, 0.5)` | `100%` of child joints respond to brain channels A/B; negative control forcing `seg_idx = 0` root fails test | proposed |
| **BB-S03** | **Inter-segment phase lag & traveling waveform:** Opposite `phase_offset` / `channel_mix` edges produce distinct oscillatory phase differences across `K >= 3` segments without discontinuity spikes on frequency change | Phase lag equals declared gene delta modulo `2π` over `>= 180` ticks | proposed |
| **BB-S04** | **`EB-S02` Burn parity & Schema 8 legacy restore:** Existing Burn parity harness (`ai/model.rs`) passes unmodified; pre-ADR-0019 `SCHEMA_VERSION = 8` JSON/bincode fixtures lacking `cpg_gene`/`phase_offset` load cleanly; `run N == run K -> save -> load -> run N-K` holds with `BrainBodyPolicy` enabled | Parity `max_diff <= 1e-5`; bit-identical checkpoint resume `world_checksum` | proposed |
| **BB-S05** | **`ADR-0013` somatic growth & non-Lamarck check:** Scaling segment `length`/`radius`/`mass` mid-life does not mutate `MorphologyEdge.cpg_gene` or `BrainGenotype`; runtime learning never writes back to `cpg_gene` or `BrainGenotype` | Bit-exact genotype equality between birth and adult/learned parent vs offspring | proposed |
| **BB-S06** | **Grafting & homologous crossover compatibility:** `crossover_genotypes` preserves valid `cpg_gene` across grafted subtrees cleanly (`validate()` passes; `0` orphaned joints/slots) | `100%` valid child decode & finite oscillator outputs across `>= 100` random morphologies | proposed |
| **BB-S07** | **Zero-alloc hot loop, total schedule order & scaling bench:** `TrackingAllocator` across sensory + inference + `apply_inertia_to_oscillators` + `update_cpg_system` + `steering_drive_system`; `schedule_ambiguity_tests`; `live_schedule_scaling` at `N = 1,000` | `0` heap allocations/tick; `0` ambiguous pairs; no p50 regression vs baseline within noise | proposed |

---

## 8. Affected Documents & Contracts

- Index (update upon acceptance): [`docs/decisions/README.md`](README.md)
- Related ADRs: [`ADR-0001`](ADR-0001-creature-development-lifecycle.md), [`ADR-0003`](ADR-0003-evolved-per-agent-brains.md), [`ADR-0006`](ADR-0006-locomotion-actuation.md), [`ADR-0013`](ADR-0013-live-creature-lifecycle.md)
- Contracts & benchmarks: [`CREATURE_DEVELOPMENT_CONTRACT.md`](../reference/CREATURE_DEVELOPMENT_CONTRACT.md), [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md), [`docs/paper/data/scaling.md`](../paper/data/scaling.md)
- Research input: `RESEARCH_REVIEW_2026-11-24_modern_biology.md` §2 Finding 2 (`Tier C / C2`)

---

## 9. Owner Decision Block (Recorded 2026-11-24)

```text
QD-BRAINBODY (owner, 2026-11-24, "Đồng ý" on the lead's summary of Option A) = ACCEPT ADR-0019:
  D1 = A, D2 = A, D3 = A, D4 = A, D5 = A;
  status -> accepted, decision_date -> 2026-11-24;
  implementation cards follow after PR #259 merges (and wait for C12 E3B-PIN / QD-BIO-D2
  for the new live.brain_body_interface key).
```

| Slot | Decision |
|---|---|
| **D1** | Keep `EVOLVED_ARCH 15->64->64->8`. Add shared 2-channel modulation and a per-edge heritable `JointCpgGene` (`channel_mix`, `phase_offset`, `freq_scale`, `amp_gain`). |
| **D2** | Opt-in only: `BrainBodyPolicy.enabled` / `live.brain_body_interface = 1.0`. With the flag off, the trunk is preserved bug-for-bug: the root skips slot 0, and ids >= 2 stay fixed at `(1.0, 0.5)`. |
| **D3** | 2 channels, no change to the 8 outputs. |
| **D4** | With the policy on, `drive_from_cpg` aggregates over all oscillating descendant segments, in a deterministic order. |
| **D5** | Land `BrainBodyPolicy` + `JointCpgGene` on today's `crossover.rs` first. The later B7 homologous crossover preserves node and edge ids. |
