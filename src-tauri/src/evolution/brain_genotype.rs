//! Heritable per-agent brains.
//!
//! Today every agent shares one [`crate::ai::model::BrainModel`] resource, so behaviour cannot be
//! inherited, cannot vary and cannot be selected on — MAP-Elites illuminates only morphology. This
//! module is step 2 of [`ADR-0003`]: the genome, the operators and the forward pass, as **pure
//! functions with no ECS wiring**. Nothing in the running simulation reads these types yet.
//!
//! [`ADR-0003`]: ../../../docs/decisions/ADR-0003-evolved-per-agent-brains.md
//!
//! ## Why the topology is pinned
//!
//! [`ArchSpec`] describes exactly the network `ai::model::ActorCriticModel` already runs —
//! `inputs → hidden → hidden → {actor, critic}`, ReLU after each trunk layer, sigmoid on the actor
//! head, linear critic. That is not incidental: gate **EB-S02** compares this forward pass against
//! Burn's on identical weights, and gate **EB-S04** requires `brain_genotype = None` to reproduce
//! today's trajectory bit-for-bit. Both are meaningless if the shapes differ.
//!
//! ## Determinism
//!
//! Every function here takes `&mut impl Rng` and never touches process state. The run's stream comes
//! from [`crate::core::resources::SimRng`], which is seeded from the world's identity — invariant
//! **D07** of `docs/reference/CREATURE_DEVELOPMENT_CONTRACT.md`.

use rand::Rng;
use serde::{Deserialize, Serialize};

/// Bumped whenever the weight layout changes. Saves carry it so an old genome is rejected loudly
/// rather than silently reinterpreted as noise.
pub const BRAIN_GENOTYPE_VERSION: u16 = 1;

/// What each actor output means.
///
/// The first four are the CPG parameters the shared model already emits; the rest are the ecological
/// gates ADR-0003 decision 4 opened. Fixing the mapping here — rather than letting each consumer
/// index by hand — is what stops an off-by-one from turning "eat" into "attack" silently.
pub mod action_index {
    /// Locomotion: frequency and amplitude pairs consumed by [`crate::ai::cpg`].
    pub const CPG_LEN: usize = 4;
    pub const PHEROMONE_EMIT: usize = 4;
    pub const ATTACK_INTENT: usize = 5;
    pub const FEED_INTENT: usize = 6;
    /// Reserved for agent-to-agent signalling; nothing reads it yet.
    pub const SIGNAL: usize = 7;
    /// Total outputs an evolved brain emits.
    pub const COUNT: usize = 8;
}

/// The architecture an evolved brain is born with: the shared model's 15 inputs and 64 hidden units,
/// widened to carry the ecological gates alongside the CPG parameters.
pub const EVOLVED_ARCH: ArchSpec = ArchSpec {
    inputs: 15,
    hidden: 64,
    outputs: action_index::COUNT,
};

/// Shape of the policy network. Dimensions are data, not compile-time constants, so a later
/// architecture change does not invalidate the serialization format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchSpec {
    pub inputs: usize,
    pub hidden: usize,
    /// Actor outputs. Today's model emits 4 (the CPG parameters); ADR-0003 widens this to include
    /// the ecological action gates.
    pub outputs: usize,
}

impl ArchSpec {
    /// The architecture the live `BrainModel` is currently constructed with
    /// (`BrainModel::new(15, 64, 4)`), for parity testing against Burn.
    pub const LEGACY: ArchSpec = ArchSpec {
        inputs: 15,
        hidden: 64,
        outputs: 4,
    };

    pub fn new(inputs: usize, hidden: usize, outputs: usize) -> Self {
        Self {
            inputs,
            hidden,
            outputs,
        }
    }

    /// Total scalar parameters: two trunk layers, an actor head and a scalar critic head, each with
    /// a bias vector.
    pub fn param_count(&self) -> usize {
        self.checked_param_count()
            .expect("brain architecture parameter count overflow")
    }

    /// Parameter count for untrusted/deserialized architecture data.
    pub fn checked_param_count(&self) -> Option<usize> {
        let trunk1 = self
            .inputs
            .checked_mul(self.hidden)?
            .checked_add(self.hidden)?;
        let trunk2 = self
            .hidden
            .checked_mul(self.hidden)?
            .checked_add(self.hidden)?;
        let actor = self
            .hidden
            .checked_mul(self.outputs)?
            .checked_add(self.outputs)?;
        let critic = self.hidden.checked_add(1)?;
        trunk1
            .checked_add(trunk2)?
            .checked_add(actor)?
            .checked_add(critic)
    }

    /// A shape with a zero dimension has no meaningful forward pass; reject it at construction
    /// rather than producing silent NaNs later.
    pub fn is_valid(&self) -> bool {
        self.inputs > 0 && self.hidden > 0 && self.outputs > 0
    }

    // Offsets into the flat weight vector. The layout is
    // `[trunk1.w, trunk1.b, trunk2.w, trunk2.b, actor.w, actor.b, critic.w, critic.b]`, each weight
    // matrix row-major as `w[out * fan_in + in]` — one output neuron's fan-in is contiguous, which
    // is the order the forward pass reads it in.
    //
    // NOTE for the EB-S02 parity harness: this is the **transpose** of Burn's layout. `burn 0.13`
    // stores `Linear::weight` with shape `[d_input, d_output]` and computes `input.matmul(weight)`,
    // so copying a flat vector across the two representations without transposing produces a network
    // that runs, produces finite output, and is silently wrong.
    fn trunk1_w(&self) -> (usize, usize) {
        (0, self.inputs * self.hidden)
    }
    fn trunk1_b(&self) -> (usize, usize) {
        (self.trunk1_w().1, self.hidden)
    }
    fn trunk2_w(&self) -> (usize, usize) {
        let start = self.trunk1_b().0 + self.trunk1_b().1;
        (start, self.hidden * self.hidden)
    }
    fn trunk2_b(&self) -> (usize, usize) {
        (self.trunk2_w().0 + self.trunk2_w().1, self.hidden)
    }
    fn actor_w(&self) -> (usize, usize) {
        let start = self.trunk2_b().0 + self.trunk2_b().1;
        (start, self.hidden * self.outputs)
    }
    fn actor_b(&self) -> (usize, usize) {
        (self.actor_w().0 + self.actor_w().1, self.outputs)
    }
    fn critic_w(&self) -> (usize, usize) {
        let start = self.actor_b().0 + self.actor_b().1;
        (start, self.hidden)
    }
    fn critic_b(&self) -> (usize, usize) {
        (self.critic_w().0 + self.critic_w().1, 1)
    }
}

/// A brain as heritable data: architecture plus one flat weight vector.
///
/// This is a sibling of [`crate::evolution::genotype::MorphologyGenotype`], deliberately **not** a
/// field of a developed phenotype. ADR-0001's development refactor is accepted but unimplemented;
/// keeping the brain outside it means neither change has to wait for the other.
///
/// Weights learned during an individual's lifetime are runtime state and **must never** be written
/// back here — inheritance of acquired characteristics is not the model (ADR-0003 decision 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrainGenotype {
    pub version: u16,
    pub arch: ArchSpec,
    pub weights: Vec<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrainGenotypeError {
    InvalidArch(ArchSpec),
    /// Weight vector length disagrees with what the architecture requires.
    WeightCountMismatch {
        expected: usize,
        found: usize,
    },
    /// A NaN or infinity would make every downstream action scientifically uninterpretable.
    NonFiniteWeight {
        index: usize,
    },
    /// A corrupt observation must be rejected before matrix products can spread it through a brain.
    NonFiniteInput {
        index: usize,
    },
    /// Finite inputs and weights can still overflow in a pathological network.
    NonFiniteActivation {
        layer: &'static str,
        index: usize,
    },
    UnsupportedVersion(u16),
}

impl std::fmt::Display for BrainGenotypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidArch(a) => write!(f, "invalid brain architecture {a:?}"),
            Self::WeightCountMismatch { expected, found } => {
                write!(f, "brain expects {expected} weights, found {found}")
            }
            Self::NonFiniteWeight { index } => {
                write!(f, "brain weight {index} is NaN or infinite")
            }
            Self::NonFiniteInput { index } => {
                write!(f, "brain input {index} is NaN or infinite")
            }
            Self::NonFiniteActivation { layer, index } => {
                write!(f, "brain {layer} activation {index} is NaN or infinite")
            }
            Self::UnsupportedVersion(v) => write!(
                f,
                "unsupported brain genotype version {v} (this build reads {BRAIN_GENOTYPE_VERSION})"
            ),
        }
    }
}

impl std::error::Error for BrainGenotypeError {}

/// One standard-normal sample by Box–Muller.
///
/// Hand-rolled rather than pulling in `rand_distr`: the dependency policy asks for a reason to add a
/// crate, and one transform is a poorer reason than a dozen lines. `f64` internally so the `ln` near
/// `u1 → 0` keeps its precision; `u1` is drawn from the half-open range excluding zero so `ln` never
/// sees it.
fn standard_normal(rng: &mut impl Rng) -> f32 {
    let u1: f64 = rng.gen_range(f64::MIN_POSITIVE..1.0);
    let u2: f64 = rng.gen_range(0.0..1.0);
    ((-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()) as f32
}

impl BrainGenotype {
    /// Build from explicit weights, validating shape.
    pub fn from_weights(arch: ArchSpec, weights: Vec<f32>) -> Result<Self, BrainGenotypeError> {
        let Some(expected) = arch.checked_param_count().filter(|_| arch.is_valid()) else {
            return Err(BrainGenotypeError::InvalidArch(arch));
        };
        if weights.len() != expected {
            return Err(BrainGenotypeError::WeightCountMismatch {
                expected,
                found: weights.len(),
            });
        }
        if let Some(index) = weights.iter().position(|weight| !weight.is_finite()) {
            return Err(BrainGenotypeError::NonFiniteWeight { index });
        }
        Ok(Self {
            version: BRAIN_GENOTYPE_VERSION,
            arch,
            weights,
        })
    }

    /// A fresh random brain.
    ///
    /// Layer-appropriate scaling rather than one global sigma: He (`sqrt(2/fan_in)`) for the ReLU
    /// trunks, Xavier (`sqrt(1/fan_in)`) for the sigmoid actor and linear critic heads. A single
    /// sigma across all layers saturates the sigmoid head at this width, and a saturated actor emits
    /// near-constant CPG parameters — the founding population would look behaviourally identical for
    /// reasons that have nothing to do with selection.
    ///
    /// Biases start at zero, which is the conventional companion to He initialisation. This is a
    /// deliberate departure from Burn's `LinearConfig`, which draws both weights *and* biases from
    /// `U(-k, k)` with `k = sqrt(1/d_input)` — so an EB-S02 parity run must copy weights across
    /// rather than assume the two initialisers agree. They are only required to agree on shape.
    pub fn random(arch: ArchSpec, rng: &mut impl Rng) -> Result<Self, BrainGenotypeError> {
        let Some(expected) = arch.checked_param_count().filter(|_| arch.is_valid()) else {
            return Err(BrainGenotypeError::InvalidArch(arch));
        };
        let mut weights = vec![0.0f32; expected];

        let he = |fan_in: usize| (2.0 / fan_in as f32).sqrt();
        let xavier = |fan_in: usize| (1.0 / fan_in as f32).sqrt();

        let mut fill = |range: (usize, usize), std: f32, rng: &mut dyn FnMut() -> f32| {
            for w in weights[range.0..range.0 + range.1].iter_mut() {
                *w = rng() * std;
            }
        };
        let mut draw = || standard_normal(rng);

        fill(arch.trunk1_w(), he(arch.inputs), &mut draw);
        fill(arch.trunk2_w(), he(arch.hidden), &mut draw);
        fill(arch.actor_w(), xavier(arch.hidden), &mut draw);
        fill(arch.critic_w(), xavier(arch.hidden), &mut draw);
        // Bias ranges stay at their zero initialisation.

        Ok(Self {
            version: BRAIN_GENOTYPE_VERSION,
            arch,
            weights,
        })
    }

    /// Reject a genome this build cannot interpret, instead of reading its weights as noise.
    pub fn validate(&self) -> Result<(), BrainGenotypeError> {
        if self.version != BRAIN_GENOTYPE_VERSION {
            return Err(BrainGenotypeError::UnsupportedVersion(self.version));
        }
        let Some(expected) = self
            .arch
            .checked_param_count()
            .filter(|_| self.arch.is_valid())
        else {
            return Err(BrainGenotypeError::InvalidArch(self.arch));
        };
        if self.weights.len() != expected {
            return Err(BrainGenotypeError::WeightCountMismatch {
                expected,
                found: self.weights.len(),
            });
        }
        if let Some(index) = self.weights.iter().position(|weight| !weight.is_finite()) {
            return Err(BrainGenotypeError::NonFiniteWeight { index });
        }
        Ok(())
    }

    /// Scratch buffer size [`Self::forward_into`] needs. Callers allocate this once and reuse it, so
    /// the tick path stays allocation-free.
    pub fn scratch_len(&self) -> usize {
        self.arch.hidden * 2
    }

    /// Heap bytes this genome occupies — the weight vector; everything else is inline.
    ///
    /// Per-agent memory is the scaling risk ADR-0003 names, so it is measurable from code rather
    /// than something to estimate on a whiteboard. Gate **EB-S12** holds it to a published budget.
    pub fn heap_bytes(&self) -> usize {
        self.weights.len() * std::mem::size_of::<f32>()
    }

    /// Forward pass, writing into caller-owned buffers.
    ///
    /// Allocation-free by construction: `scratch` holds both hidden layers and `actions` receives
    /// the actor output. Returns the critic's scalar value estimate. This is the form the tick path
    /// will use; [`Self::forward`] is the allocating convenience wrapper for tests.
    ///
    /// Mirrors `ActorCriticModel::forward` exactly: linear, ReLU, linear, ReLU, then a sigmoid actor
    /// head and a linear critic head off the same trunk output.
    pub fn forward_into(
        &self,
        inputs: &[f32],
        scratch: &mut [f32],
        actions: &mut [f32],
    ) -> Result<f32, BrainGenotypeError> {
        self.validate()?;
        let (i, h, o) = (self.arch.inputs, self.arch.hidden, self.arch.outputs);
        if inputs.len() != i || scratch.len() < h * 2 || actions.len() != o {
            return Err(BrainGenotypeError::WeightCountMismatch {
                expected: i + h * 2 + o,
                found: inputs.len() + scratch.len() + actions.len(),
            });
        }
        if let Some(index) = inputs.iter().position(|input| !input.is_finite()) {
            return Err(BrainGenotypeError::NonFiniteInput { index });
        }

        let w = &self.weights;
        let (h1, h2) = scratch.split_at_mut(h);
        let h1 = &mut h1[..h];
        let h2 = &mut h2[..h];

        let (t1w, _) = self.arch.trunk1_w();
        let (t1b, _) = self.arch.trunk1_b();
        for (n, out) in h1.iter_mut().enumerate() {
            let mut acc = w[t1b + n];
            for (k, x) in inputs.iter().enumerate() {
                acc += w[t1w + n * i + k] * x;
            }
            *out = acc.max(0.0); // relu
            if !out.is_finite() {
                return Err(BrainGenotypeError::NonFiniteActivation {
                    layer: "trunk1",
                    index: n,
                });
            }
        }

        let (t2w, _) = self.arch.trunk2_w();
        let (t2b, _) = self.arch.trunk2_b();
        for (n, out) in h2.iter_mut().enumerate() {
            let mut acc = w[t2b + n];
            for (k, x) in h1.iter().enumerate() {
                acc += w[t2w + n * h + k] * x;
            }
            *out = acc.max(0.0); // relu
            if !out.is_finite() {
                return Err(BrainGenotypeError::NonFiniteActivation {
                    layer: "trunk2",
                    index: n,
                });
            }
        }

        let (aw, _) = self.arch.actor_w();
        let (ab, _) = self.arch.actor_b();
        for (n, out) in actions.iter_mut().enumerate() {
            let mut acc = w[ab + n];
            for (k, x) in h2.iter().enumerate() {
                acc += w[aw + n * h + k] * x;
            }
            *out = 1.0 / (1.0 + (-acc).exp()); // sigmoid
            if !out.is_finite() {
                return Err(BrainGenotypeError::NonFiniteActivation {
                    layer: "actor",
                    index: n,
                });
            }
        }

        let (cw, _) = self.arch.critic_w();
        let (cb, _) = self.arch.critic_b();
        let mut value = w[cb];
        for (k, x) in h2.iter().enumerate() {
            value += w[cw + k] * x;
        }
        if !value.is_finite() {
            return Err(BrainGenotypeError::NonFiniteActivation {
                layer: "critic",
                index: 0,
            });
        }
        Ok(value)
    }

    /// Allocating convenience wrapper. Not for the tick path — see [`Self::forward_into`].
    pub fn forward(&self, inputs: &[f32]) -> Result<(Vec<f32>, f32), BrainGenotypeError> {
        let mut scratch = vec![0.0; self.scratch_len()];
        let mut actions = vec![0.0; self.arch.outputs];
        let value = self.forward_into(inputs, &mut scratch, &mut actions)?;
        Ok((actions, value))
    }
}

/// One in-life learning step: an A2C update applied to a single agent's own weights.
///
/// ADR-0003 decision 6 — the Baldwin half of the hybrid. Evolution decides where a brain *starts*;
/// this refines it within one lifetime, and the result is deliberately **not** inherited.
///
/// ### The objective
///
/// `target = r + γ·V(s')`, `td = target − V(s)`, and
/// `L = mean((a − a_taken)² · td_detached) + ½·mean(td²)`.
/// The actor term is advantage-weighted behavioural cloning: with a positive advantage, minimising
/// `td·(a − â)²` pulls the policy **toward** the action it took; with a negative advantage the
/// coefficient flips and it pushes away. The critic term fits the value estimate.
///
/// ### The shared model now uses the same sign
///
/// `run_training_loop` in `core/simulation_loop.rs` used to compute `(a − â)² · (−td)`. That
/// coefficient is inverted: a *positive* advantage makes the loss decrease as `(a − â)²` grows, so
/// gradient descent drove the shared policy **away** from actions that turned out better than
/// expected, and toward ones that turned out worse. A `learn_step` written to match would have
/// reproduced the defect rather than the intent, so this implemented the correct sign and ADR-0003
/// recorded the discrepancy as outstanding.
///
/// That has since been fixed: the shared objective lives in
/// [`crate::core::training::a2c_loss`] and carries `+td`, so there is now one objective
/// rather than two that disagree. Shared-model runs from before the fix followed a different
/// trajectory, which was the point of fixing it.
///
/// The direction here is pinned by `learning_moves_the_policy_toward_a_rewarded_action`, and the
/// gradient itself by `the_learning_gradient_matches_finite_differences` — a check on the derivative
/// alone would have accepted the inverted objective too. `a2c_loss_direction_tests` applies the same
/// pairing to the shared model and additionally asserts the two agree.
///
/// ### Two deliberate departures, both about cost
///
/// - **Plain SGD, not Adam.** Adam needs two moment buffers per parameter, tripling the per-agent
///   memory that ADR-0003 already flags as the scaling risk. Per-agent optimiser state is a decision
///   for when there is evidence it pays.
/// - **Only the CPG outputs are trained.** `LastTransitionState.action` records the four locomotion
///   parameters and nothing else, so no target exists for the ecological gates. In v1 that gives a
///   clean division of labour: evolution sets ecological policy, lifetime learning refines gait.
///   Training the gates needs the taken gate values recorded too — a save-format change, deferred.
///
/// `next_value` is `V(s')`, supplied by the caller because it needs a second forward pass the caller
/// may already have done. Returns the TD error, which is the useful thing to log.
#[allow(clippy::too_many_arguments)]
pub fn learn_step(
    genotype: &mut BrainGenotype,
    state: &[f32],
    action_taken: &[f32],
    reward: f32,
    next_value: f32,
    discount: f32,
    learning_rate: f32,
    scratch: &mut LearnScratch,
) -> Result<f32, BrainGenotypeError> {
    genotype.validate()?;
    let (i, h, o) = (
        genotype.arch.inputs,
        genotype.arch.hidden,
        genotype.arch.outputs,
    );
    if state.len() != i || action_taken.is_empty() || action_taken.len() > o {
        return Err(BrainGenotypeError::WeightCountMismatch {
            expected: i,
            found: state.len(),
        });
    }
    if !reward.is_finite() || !next_value.is_finite() || !(0.0..=1.0).contains(&learning_rate) {
        return Ok(0.0);
    }

    scratch.prepare(i, h, o);
    let a = genotype.arch;
    let w = &mut genotype.weights;

    // --- forward, caching pre-activations because the ReLU derivative needs them ---
    let (t1w, _) = a.trunk1_w();
    let (t1b, _) = a.trunk1_b();
    for n in 0..h {
        let mut acc = w[t1b + n];
        for (k, x) in state.iter().enumerate() {
            acc += w[t1w + n * i + k] * x;
        }
        scratch.h1_pre[n] = acc;
        scratch.h1[n] = acc.max(0.0);
    }

    let (t2w, _) = a.trunk2_w();
    let (t2b, _) = a.trunk2_b();
    for n in 0..h {
        let mut acc = w[t2b + n];
        for k in 0..h {
            acc += w[t2w + n * h + k] * scratch.h1[k];
        }
        scratch.h2_pre[n] = acc;
        scratch.h2[n] = acc.max(0.0);
    }

    let (aw, _) = a.actor_w();
    let (ab, _) = a.actor_b();
    for n in 0..o {
        let mut acc = w[ab + n];
        for k in 0..h {
            acc += w[aw + n * h + k] * scratch.h2[k];
        }
        scratch.act[n] = 1.0 / (1.0 + (-acc).exp());
    }

    let (cw, _) = a.critic_w();
    let (cb, _) = a.critic_b();
    let mut value = w[cb];
    for k in 0..h {
        value += w[cw + k] * scratch.h2[k];
    }

    let td = reward + discount * next_value - value;
    if !td.is_finite() {
        return Ok(0.0);
    }

    // --- backward ---
    // `∂(½·mean(td²))/∂V = −td`.
    let dv = -td;
    // `∂(mean((a−â)²·td))/∂aₙ`, averaged over the outputs that actually have a target.
    let trained = action_taken.len();
    let inv = 1.0 / trained as f32;
    for (n, d) in scratch.d_act.iter_mut().enumerate().take(o) {
        *d = match action_taken.get(n) {
            Some(target) => {
                let diff = scratch.act[n] - target;
                let da = 2.0 * diff * td * inv;
                da * scratch.act[n] * (1.0 - scratch.act[n]) // through the sigmoid
            }
            // No recorded target: the ecological gates are evolved, not trained (see above).
            None => 0.0,
        };
    }

    // Gradient into the shared trunk output, from both heads.
    for k in 0..h {
        let mut acc = dv * w[cw + k];
        for n in 0..o {
            acc += scratch.d_act[n] * w[aw + n * h + k];
        }
        scratch.d_h2[k] = if scratch.h2_pre[k] > 0.0 { acc } else { 0.0 };
    }

    for k in 0..h {
        let mut acc = 0.0;
        for n in 0..h {
            acc += scratch.d_h2[n] * w[t2w + n * h + k];
        }
        scratch.d_h1[k] = if scratch.h1_pre[k] > 0.0 { acc } else { 0.0 };
    }

    // --- SGD, applied last so every gradient above was read from the pre-update weights ---
    for n in 0..o {
        let g = scratch.d_act[n];
        if g != 0.0 {
            for k in 0..h {
                w[aw + n * h + k] -= learning_rate * g * scratch.h2[k];
            }
            w[ab + n] -= learning_rate * g;
        }
    }
    for k in 0..h {
        w[cw + k] -= learning_rate * dv * scratch.h2[k];
    }
    w[cb] -= learning_rate * dv;

    for n in 0..h {
        let g = scratch.d_h2[n];
        if g != 0.0 {
            for k in 0..h {
                w[t2w + n * h + k] -= learning_rate * g * scratch.h1[k];
            }
            w[t2b + n] -= learning_rate * g;
        }
    }
    for n in 0..h {
        let g = scratch.d_h1[n];
        if g != 0.0 {
            for (k, x) in state.iter().enumerate() {
                w[t1w + n * i + k] -= learning_rate * g * x;
            }
            w[t1b + n] -= learning_rate * g;
        }
    }

    Ok(td)
}

/// Caller-owned buffers for [`learn_step`], so a learning tick allocates nothing.
#[derive(Default, Debug)]
pub struct LearnScratch {
    h1_pre: Vec<f32>,
    h1: Vec<f32>,
    h2_pre: Vec<f32>,
    h2: Vec<f32>,
    act: Vec<f32>,
    d_act: Vec<f32>,
    d_h2: Vec<f32>,
    d_h1: Vec<f32>,
}

impl LearnScratch {
    fn prepare(&mut self, _inputs: usize, hidden: usize, outputs: usize) {
        for buf in [
            &mut self.h1_pre,
            &mut self.h1,
            &mut self.h2_pre,
            &mut self.h2,
            &mut self.d_h2,
            &mut self.d_h1,
        ] {
            buf.clear();
            buf.resize(hidden, 0.0);
        }
        for buf in [&mut self.act, &mut self.d_act] {
            buf.clear();
            buf.resize(outputs, 0.0);
        }
    }
}

/// Perturb weights in place.
///
/// `rate` is the per-weight probability of being touched and `sigma` the perturbation scale, so
/// mutation can be sparse-and-large or dense-and-small rather than one fixed shape. Architecture is
/// never mutated in v1: a fixed interface is what keeps the brain independent of morphology, and
/// changing it is the subject of a later ADR.
///
/// Returns the number of weights actually perturbed, which lets a caller assert that a mutation with
/// `rate = 0.0` really is a no-op.
pub fn mutate_brain(
    genotype: &mut BrainGenotype,
    rate: f64,
    sigma: f32,
    rng: &mut impl Rng,
) -> Result<usize, BrainGenotypeError> {
    genotype.validate()?;
    if !(0.0..=1.0).contains(&rate) || !sigma.is_finite() || sigma < 0.0 {
        return Ok(0);
    }
    let mut touched = 0;
    for w in genotype.weights.iter_mut() {
        if rng.gen_bool(rate) {
            *w += standard_normal(rng) * sigma;
            touched += 1;
        }
    }
    Ok(touched)
}

/// Uniform per-weight recombination.
///
/// Uniform rather than single-point because a flat weight vector has no meaningful locality — the
/// index of a weight says nothing about which behaviour it contributes to, so a crossover point
/// would be an arbitrary cut through an unstructured space.
///
/// Parents whose architectures differ cannot be blended coherently; the child is a clone of
/// `parent_a`, mirroring how [`crate::evolution::crossover::crossover_genotypes`] falls back rather
/// than fabricating a hybrid.
pub fn crossover_brains(
    parent_a: &BrainGenotype,
    parent_b: &BrainGenotype,
    rng: &mut impl Rng,
) -> Result<BrainGenotype, BrainGenotypeError> {
    parent_a.validate()?;
    if parent_b.validate().is_err() || parent_a.arch != parent_b.arch {
        return Ok(parent_a.clone());
    }
    let mut child = parent_a.clone();
    for (w, b) in child.weights.iter_mut().zip(parent_b.weights.iter()) {
        if rng.gen_bool(0.5) {
            *w = *b;
        }
    }
    Ok(child)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn rng(seed: u64) -> StdRng {
        StdRng::seed_from_u64(seed)
    }

    #[test]
    fn param_count_matches_the_live_burn_model() {
        // BrainModel::new(15, 64, 4): trunk1 15x64+64, trunk2 64x64+64, actor 64x4+4, critic 64+1.
        assert_eq!(ArchSpec::LEGACY.param_count(), 1024 + 4160 + 260 + 65);
    }

    #[test]
    fn widened_action_space_costs_what_it_should() {
        // ADR-0003 adds four ecological gates on top of the four CPG parameters.
        let widened = ArchSpec::new(15, 64, 8);
        assert_eq!(widened.param_count(), 5769);
        assert_eq!(
            widened.param_count() - ArchSpec::LEGACY.param_count(),
            4 * 64 + 4
        );
    }

    #[test]
    fn weight_ranges_tile_the_vector_without_gap_or_overlap() {
        let a = ArchSpec::new(7, 5, 3);
        let ranges = [
            a.trunk1_w(),
            a.trunk1_b(),
            a.trunk2_w(),
            a.trunk2_b(),
            a.actor_w(),
            a.actor_b(),
            a.critic_w(),
            a.critic_b(),
        ];
        let mut cursor = 0;
        for (start, len) in ranges {
            assert_eq!(start, cursor, "range must start where the previous ended");
            cursor += len;
        }
        assert_eq!(cursor, a.param_count(), "ranges must cover every parameter");
    }

    #[test]
    fn random_is_reproducible_and_finite() {
        let a = BrainGenotype::random(ArchSpec::LEGACY, &mut rng(7)).unwrap();
        let b = BrainGenotype::random(ArchSpec::LEGACY, &mut rng(7)).unwrap();
        let c = BrainGenotype::random(ArchSpec::LEGACY, &mut rng(8)).unwrap();

        assert_eq!(a, b, "same seed must produce the same brain");
        assert_ne!(a, c);
        assert!(a.weights.iter().all(|w| w.is_finite()));
        a.validate().unwrap();
    }

    #[test]
    fn biases_start_at_zero() {
        let g = BrainGenotype::random(ArchSpec::new(4, 6, 3), &mut rng(1)).unwrap();
        for (start, len) in [
            g.arch.trunk1_b(),
            g.arch.trunk2_b(),
            g.arch.actor_b(),
            g.arch.critic_b(),
        ] {
            assert!(g.weights[start..start + len].iter().all(|w| *w == 0.0));
        }
    }

    #[test]
    fn head_weights_are_scaled_more_tightly_than_trunk_weights() {
        // Guards the layer-appropriate initialisation: He on a 15-wide fan-in is visibly wider than
        // Xavier on a 64-wide fan-in, and collapsing them would saturate the sigmoid actor head.
        let g = BrainGenotype::random(ArchSpec::LEGACY, &mut rng(3)).unwrap();
        let spread = |(start, len): (usize, usize)| {
            let s: f32 = g.weights[start..start + len].iter().map(|w| w * w).sum();
            (s / len as f32).sqrt()
        };
        assert!(spread(g.arch.trunk1_w()) > spread(g.arch.actor_w()) * 2.0);
    }

    #[test]
    fn rejects_shapes_and_lengths_it_cannot_interpret() {
        assert_eq!(
            BrainGenotype::random(ArchSpec::new(0, 4, 2), &mut rng(1)),
            Err(BrainGenotypeError::InvalidArch(ArchSpec::new(0, 4, 2)))
        );
        assert_eq!(
            BrainGenotype::from_weights(ArchSpec::LEGACY, vec![0.0; 3]),
            Err(BrainGenotypeError::WeightCountMismatch {
                expected: ArchSpec::LEGACY.param_count(),
                found: 3,
            })
        );
        let mut non_finite = vec![0.0; ArchSpec::LEGACY.param_count()];
        non_finite[7] = f32::NAN;
        assert_eq!(
            BrainGenotype::from_weights(ArchSpec::LEGACY, non_finite),
            Err(BrainGenotypeError::NonFiniteWeight { index: 7 })
        );

        let overflow = ArchSpec::new(usize::MAX, usize::MAX, usize::MAX);
        assert_eq!(
            BrainGenotype::from_weights(overflow, Vec::new()),
            Err(BrainGenotypeError::InvalidArch(overflow))
        );

        let mut stale = BrainGenotype::random(ArchSpec::LEGACY, &mut rng(1)).unwrap();
        stale.version = BRAIN_GENOTYPE_VERSION + 1;
        assert_eq!(
            stale.validate(),
            Err(BrainGenotypeError::UnsupportedVersion(
                BRAIN_GENOTYPE_VERSION + 1
            ))
        );
    }

    #[test]
    fn forward_is_deterministic_and_bounded() {
        let g = BrainGenotype::random(ArchSpec::LEGACY, &mut rng(11)).unwrap();
        let inputs = vec![0.3f32; ArchSpec::LEGACY.inputs];

        let (a1, v1) = g.forward(&inputs).unwrap();
        let (a2, v2) = g.forward(&inputs).unwrap();

        assert_eq!(a1, a2);
        assert_eq!(v1, v2);
        assert_eq!(a1.len(), ArchSpec::LEGACY.outputs);
        // Sigmoid actor head: every action is a valid CPG parameter in [0, 1].
        assert!(a1.iter().all(|a| (0.0..=1.0).contains(a) && a.is_finite()));
        assert!(v1.is_finite());
    }

    #[test]
    fn forward_into_matches_the_allocating_wrapper() {
        let g = BrainGenotype::random(ArchSpec::new(6, 8, 3), &mut rng(5)).unwrap();
        let inputs = [0.1, -0.4, 0.9, 0.0, 0.5, -1.0];

        let (want_actions, want_value) = g.forward(&inputs).unwrap();

        let mut scratch = vec![0.0; g.scratch_len()];
        let mut actions = vec![0.0; g.arch.outputs];
        let value = g.forward_into(&inputs, &mut scratch, &mut actions).unwrap();

        assert_eq!(actions, want_actions);
        assert_eq!(value, want_value);
    }

    #[test]
    fn forward_into_reuses_buffers_without_carrying_state() {
        // The tick path calls this repeatedly on one scratch buffer; stale hidden activations must
        // not leak from the previous agent into the next.
        let g = BrainGenotype::random(ArchSpec::new(5, 7, 2), &mut rng(9)).unwrap();
        let mut scratch = vec![0.0; g.scratch_len()];
        let mut actions = vec![0.0; g.arch.outputs];

        let a = [1.0, 2.0, 3.0, 4.0, 5.0];
        let b = [-1.0, 0.0, 1.0, 0.0, -1.0];

        let va = g.forward_into(&a, &mut scratch, &mut actions).unwrap();
        let first = actions.clone();
        let _ = g.forward_into(&b, &mut scratch, &mut actions).unwrap();
        let va_again = g.forward_into(&a, &mut scratch, &mut actions).unwrap();

        assert_eq!(actions, first);
        assert_eq!(va, va_again);
    }

    #[test]
    fn forward_rejects_wrong_sized_buffers() {
        let g = BrainGenotype::random(ArchSpec::new(4, 4, 2), &mut rng(2)).unwrap();
        let mut scratch = vec![0.0; g.scratch_len()];
        let mut actions = vec![0.0; 2];
        assert!(g
            .forward_into(&[0.0; 3], &mut scratch, &mut actions)
            .is_err());
        assert!(g
            .forward_into(&[0.0; 4], &mut scratch[..1], &mut actions)
            .is_err());
    }

    #[test]
    fn mutation_replays_and_respects_rate() {
        let base = BrainGenotype::random(ArchSpec::new(6, 8, 4), &mut rng(4)).unwrap();

        let run = |seed: u64| {
            let mut g = base.clone();
            let touched = mutate_brain(&mut g, 0.25, 0.1, &mut rng(seed)).unwrap();
            (g, touched)
        };
        assert_eq!(run(1), run(1), "same seed must give the same mutation");
        assert_ne!(run(1).0, run(2).0);

        let mut untouched = base.clone();
        assert_eq!(mutate_brain(&mut untouched, 0.0, 0.5, &mut rng(1)), Ok(0));
        assert_eq!(untouched, base, "rate 0.0 must be a true no-op");

        let mut all = base.clone();
        let touched = mutate_brain(&mut all, 1.0, 0.05, &mut rng(1)).unwrap();
        assert_eq!(touched, base.arch.param_count());
        assert!(all.weights.iter().all(|w| w.is_finite()));
    }

    #[test]
    fn mutation_ignores_nonsensical_parameters() {
        let mut g = BrainGenotype::random(ArchSpec::new(4, 4, 2), &mut rng(1)).unwrap();
        let before = g.clone();
        assert_eq!(mutate_brain(&mut g, 1.5, 0.1, &mut rng(1)), Ok(0));
        assert_eq!(mutate_brain(&mut g, 0.5, f32::NAN, &mut rng(1)), Ok(0));
        assert_eq!(mutate_brain(&mut g, 0.5, -1.0, &mut rng(1)), Ok(0));
        assert_eq!(g, before);
    }

    #[test]
    fn mutation_never_changes_architecture() {
        let mut g = BrainGenotype::random(ArchSpec::LEGACY, &mut rng(1)).unwrap();
        mutate_brain(&mut g, 1.0, 1.0, &mut rng(2)).unwrap();
        assert_eq!(g.arch, ArchSpec::LEGACY);
        assert_eq!(g.weights.len(), ArchSpec::LEGACY.param_count());
        g.validate().unwrap();
    }

    #[test]
    fn crossover_replays_and_draws_from_both_parents() {
        let arch = ArchSpec::new(5, 6, 3);
        let a = BrainGenotype::from_weights(arch, vec![0.0; arch.param_count()]).unwrap();
        let b = BrainGenotype::from_weights(arch, vec![1.0; arch.param_count()]).unwrap();

        let child = crossover_brains(&a, &b, &mut rng(3)).unwrap();
        assert_eq!(child, crossover_brains(&a, &b, &mut rng(3)).unwrap());
        child.validate().unwrap();

        // Every weight came from one parent or the other, and neither parent supplied all of them.
        assert!(child.weights.iter().all(|w| *w == 0.0 || *w == 1.0));
        assert!(child.weights.contains(&0.0));
        assert!(child.weights.contains(&1.0));
    }

    #[test]
    fn crossover_of_mismatched_architectures_falls_back_to_parent_a() {
        let a = BrainGenotype::random(ArchSpec::new(4, 5, 2), &mut rng(1)).unwrap();
        let b = BrainGenotype::random(ArchSpec::new(6, 5, 2), &mut rng(2)).unwrap();
        assert_eq!(crossover_brains(&a, &b, &mut rng(3)).unwrap(), a);
    }

    /// The exact objective `learn_step` differentiates, evaluated directly.
    ///
    /// `td_detached` stands in for autodiff's `.detach()`: the actor term treats the advantage as a
    /// constant, so the numeric check has to hold it fixed while perturbing weights, or it would be
    /// checking a different function than the one the analytic gradient describes.
    fn loss_of(
        g: &BrainGenotype,
        state: &[f32],
        action: &[f32],
        reward: f32,
        next_value: f32,
        discount: f32,
        td_detached: f32,
    ) -> f32 {
        let (actions, value) = g.forward(state).unwrap();
        let td = reward + discount * next_value - value;
        let actor: f32 = action
            .iter()
            .enumerate()
            .map(|(i, target)| (actions[i] - target).powi(2) * td_detached)
            .sum::<f32>()
            / action.len() as f32;
        actor + 0.5 * td * td
    }

    #[test]
    fn the_learning_gradient_matches_finite_differences() {
        // Hand-written backpropagation is the archetypal silently-wrong code: a sign slip or a
        // transposed index still trains, just toward nothing. Comparing every analytic gradient
        // against a numerical one is the only check that actually settles it.
        //
        // `f64` accumulation and a relatively large epsilon keep the finite difference out of `f32`
        // cancellation noise while staying inside the linear regime.
        let arch = ArchSpec::new(4, 6, 3);
        let base = BrainGenotype::random(arch, &mut rng(2024)).unwrap();
        let state = [0.6f32, -0.3, 0.9, 0.15];
        let action = [0.2f32, 0.7, 0.4];
        let (reward, next_value, discount) = (0.35f32, 0.8f32, 0.99f32);
        let lr = 1e-3f32;

        // Advantage from the unperturbed weights — the value `learn_step` detaches.
        let (_, v0) = base.forward(&state).unwrap();
        let td0 = reward + discount * next_value - v0;

        let mut trained = base.clone();
        let mut scratch = LearnScratch::default();
        let td = learn_step(
            &mut trained,
            &state,
            &action,
            reward,
            next_value,
            discount,
            lr,
            &mut scratch,
        )
        .unwrap();
        assert!(
            (td - td0).abs() < 1e-5,
            "reported TD error must be the real one"
        );

        let eps = 1e-3f32;
        let mut worst = 0.0f32;
        for idx in 0..base.weights.len() {
            // SGD applied `w -= lr * grad`, so the gradient it used is recoverable exactly.
            let analytic = (base.weights[idx] - trained.weights[idx]) / lr;

            let mut up = base.clone();
            up.weights[idx] += eps;
            let mut down = base.clone();
            down.weights[idx] -= eps;
            let numeric = (loss_of(&up, &state, &action, reward, next_value, discount, td0)
                - loss_of(&down, &state, &action, reward, next_value, discount, td0))
                / (2.0 * eps);

            let scale = analytic.abs().max(numeric.abs()).max(1e-3);
            worst = worst.max((analytic - numeric).abs() / scale);
        }

        assert!(
            worst < 2e-2,
            "analytic and numerical gradients disagree by {worst:e} relative"
        );
    }

    #[test]
    fn learning_moves_the_policy_toward_a_rewarded_action() {
        // The direction test. With a positive advantage the policy should end up closer to the
        // action it took; a sign error would pass the gradient check only if the loss itself were
        // wrong, so the two together pin both.
        let arch = ArchSpec::new(5, 8, 2);
        let mut g = BrainGenotype::random(arch, &mut rng(31)).unwrap();
        let state = [0.4f32; 5];
        let action = [1.0f32, 0.0];

        let before: f32 = {
            let (a, _) = g.forward(&state).unwrap();
            a.iter().zip(&action).map(|(x, t)| (x - t).powi(2)).sum()
        };

        let mut scratch = LearnScratch::default();
        for _ in 0..200 {
            // A large positive reward relative to the value estimate keeps the advantage positive.
            learn_step(&mut g, &state, &action, 5.0, 0.0, 0.99, 0.05, &mut scratch).unwrap();
        }

        let after: f32 = {
            let (a, _) = g.forward(&state).unwrap();
            a.iter().zip(&action).map(|(x, t)| (x - t).powi(2)).sum()
        };
        assert!(
            after < before,
            "a rewarded action should become more likely: {before} → {after}"
        );
        assert!(g.weights.iter().all(|w| w.is_finite()));
    }

    #[test]
    fn learning_does_not_touch_the_untrained_outputs() {
        // The ecological gates have no recorded target, so their head must be left exactly alone
        // rather than drifting on a zero gradient that is actually garbage.
        let arch = ArchSpec::new(4, 5, 7);
        let base = BrainGenotype::random(arch, &mut rng(41)).unwrap();
        let mut g = base.clone();
        let mut scratch = LearnScratch::default();
        learn_step(
            &mut g,
            &[0.5; 4],
            &[0.3, 0.6],
            1.0,
            0.2,
            0.99,
            0.01,
            &mut scratch,
        )
        .unwrap();

        let (aw, _) = arch.actor_w();
        let (ab, _) = arch.actor_b();
        let h = arch.hidden;
        for n in 2..arch.outputs {
            assert_eq!(
                &g.weights[aw + n * h..aw + (n + 1) * h],
                &base.weights[aw + n * h..aw + (n + 1) * h],
                "output {n} has no target and must not be trained"
            );
            assert_eq!(g.weights[ab + n], base.weights[ab + n]);
        }
    }

    #[test]
    fn learning_is_reproducible_and_rejects_nonsense() {
        let arch = ArchSpec::new(4, 5, 3);
        let base = BrainGenotype::random(arch, &mut rng(51)).unwrap();

        let run = || {
            let mut g = base.clone();
            let mut s = LearnScratch::default();
            for _ in 0..10 {
                learn_step(&mut g, &[0.2; 4], &[0.5; 3], 0.4, 0.1, 0.99, 0.01, &mut s).unwrap();
            }
            g
        };
        assert_eq!(run(), run(), "learning has no randomness of its own");

        let mut g = base.clone();
        let mut s = LearnScratch::default();
        // A non-finite reward or an out-of-range learning rate must leave the brain untouched
        // rather than filling it with NaN that only surfaces as an agent behaving strangely.
        assert_eq!(
            learn_step(
                &mut g,
                &[0.2; 4],
                &[0.5; 3],
                f32::NAN,
                0.0,
                0.99,
                0.01,
                &mut s
            ),
            Ok(0.0)
        );
        assert_eq!(
            learn_step(&mut g, &[0.2; 4], &[0.5; 3], 1.0, 0.0, 0.99, 5.0, &mut s),
            Ok(0.0)
        );
        assert_eq!(g, base);
    }

    #[test]
    fn round_trips_through_serde() {
        let g = BrainGenotype::random(ArchSpec::new(5, 9, 4), &mut rng(6)).unwrap();
        let json = serde_json::to_string(&g).unwrap();
        let back: BrainGenotype = serde_json::from_str(&json).unwrap();
        assert_eq!(back, g);
        back.validate().unwrap();
    }
}
