use bevy_ecs::prelude::*;
use burn::module::Module;
use burn::nn::{Linear, LinearConfig};
use burn::tensor::backend::Backend;
use burn::tensor::{Data, Shape, Tensor};

use crate::ai::cpg::CpgOscillator;
use crate::ai::hrrl::HomeostaticState;
use crate::core::ecs::{Food, ParentAgent, Position, Predator, Prey, Rotation, Segment};

pub type DefaultBackend = burn_ndarray::NdArray<f32>;

#[cfg(feature = "ml-wgpu")]
type WgpuBackend = burn_wgpu::Wgpu<burn_wgpu::AutoGraphicsApi, f32, i32>;

/// Run one forward pass and read it back, so the wgpu probe measures the thing it claims to.
///
/// The probe used to wrap only construction. On a GPU-less CI runner that returned `Ok`, the caller
/// took the Wgpu branch, and `No adapter found for graphics API AutoGraphicsApi` arrived at the
/// first tensor operation — outside the `catch_unwind` meant to catch it. The fallback was written,
/// was correct, and never ran.
///
/// Exactly *when* burn-wgpu demands an adapter is not something this comment should assert. An
/// attempt to pin it down locally, by building a `Wgpu<Metal>` model on Windows, panicked at
/// construction — the opposite of what the CI log shows for `AutoGraphicsApi`. So the honest
/// statement is narrower: the moment varies, and the guard has to span all of it. That is what this
/// does — construction and a real operation, both inside one `catch_unwind`.
///
/// `into_data` is the part that makes it an operation rather than a queue entry: it synchronises
/// and copies back to the host, forcing the work to actually execute.
#[cfg(feature = "ml-wgpu")]
fn wgpu_survives_one_forward_pass(
    model: &ActorCriticModel<WgpuBackend>,
    device: &burn_wgpu::WgpuDevice,
    input_dim: usize,
) {
    materialize_params(model, device, input_dim);
}

/// Run one forward pass so every [`burn::module::Param`] resolves its lazy initializer **here**,
/// on the constructing thread, before the model can be shared.
///
/// # Why this is a soundness requirement, not a warm-up
///
/// `linear_from_parts` builds parameters with `Param::uninitialized`, which stores a `OnceCell<T>`
/// plus a closure and fills the cell on first `val()`. That is deliberate — eagerly uploading every
/// layer at construction pushed `SimulationEngine::start` past the point the stress tests observe a
/// first tick — but it means a freshly built model carries **unsynchronised interior mutability**.
///
/// `BrainModel` is then handed out as a Bevy resource, and Bevy runs `Res<T>` readers in parallel.
/// Two systems first-touching the same parameter at the same time would race on that `OnceCell`,
/// and `unsafe impl Sync` would let it compile. Draining the laziness before the value escapes the
/// constructor is what makes the cell write-once-then-read-only, which is the invariant the
/// `unsafe impl` at the bottom of this file actually depends on.
///
/// `into_data` is load-bearing: it synchronises and copies back to the host, so the work is really
/// executed rather than merely queued.
fn materialize_params<B: Backend>(
    model: &ActorCriticModel<B>,
    device: &B::Device,
    input_dim: usize,
) {
    let data: Data<B::FloatElem, 2> =
        Data::new(vec![0.0f32; input_dim], Shape::new([1, input_dim])).convert();
    let input = Tensor::<B, 2>::from_data(data, device);
    let (actor, critic) = model.forward(input);
    let _ = actor.into_data();
    let _ = critic.into_data();
}

#[derive(Module, Debug)]
pub struct ActorCriticModel<B: Backend> {
    trunk1: Linear<B>,
    trunk2: Linear<B>,
    actor_head: Linear<B>,
    critic_head: Linear<B>,
}

impl<B: Backend> ActorCriticModel<B> {
    pub fn new(input_dim: usize, hidden_dim: usize, action_dim: usize, device: &B::Device) -> Self {
        let trunk1 = LinearConfig::new(input_dim, hidden_dim).init(device);
        let trunk2 = LinearConfig::new(hidden_dim, hidden_dim).init(device);
        let actor_head = LinearConfig::new(hidden_dim, action_dim).init(device);
        let critic_head = LinearConfig::new(hidden_dim, 1).init(device);
        Self {
            trunk1,
            trunk2,
            actor_head,
            critic_head,
        }
    }

    /// Rebuild this model from a flat weight vector laid out the way
    /// [`crate::evolution::brain_genotype::BrainGenotype`] stores one.
    ///
    /// Exists for gate **EB-S02** of ADR-0003: the per-agent brain runs its own hand-written forward
    /// pass, and that pass is only trustworthy if it provably agrees with the Burn model it replaces
    /// on identical weights. Comparing them requires putting the same numbers into both, which is
    /// what this does. It is a constructor, not a mutation — nothing about the running model changes.
    ///
    /// ### The transpose is the whole point
    ///
    /// `BrainGenotype` stores each weight matrix output-major, `w[out * fan_in + in]`, so one
    /// neuron's fan-in is contiguous for the forward pass. Burn's [`Linear`] stores `[d_input,
    /// d_output]` and computes `input.matmul(weight)`. Copying the flat vector across without
    /// transposing yields a network that runs, produces finite output, and is silently wrong — for
    /// square layers it would not even change the shape. Hence [`transpose_to_burn`].
    ///
    /// Dimensions are plain `usize` rather than an `ArchSpec` on purpose: `ai` must not depend on
    /// `evolution`, and `evolution::brain_genotype` must stay free of Burn (ADR-0003 decision 5).
    pub fn from_flat_weights(
        inputs: usize,
        hidden: usize,
        outputs: usize,
        weights: &[f32],
        device: &B::Device,
    ) -> Result<Self, String> {
        if inputs == 0 || hidden == 0 || outputs == 0 {
            return Err(format!(
                "degenerate architecture {inputs}x{hidden}x{outputs}"
            ));
        }
        let expected = (inputs * hidden + hidden)
            + (hidden * hidden + hidden)
            + (hidden * outputs + outputs)
            + (hidden + 1);
        if weights.len() != expected {
            return Err(format!(
                "expected {expected} weights for {inputs}x{hidden}x{outputs}, got {}",
                weights.len()
            ));
        }

        let mut at = 0usize;
        let mut take = |n: usize| {
            let slice = &weights[at..at + n];
            at += n;
            slice
        };

        let trunk1_w = take(inputs * hidden);
        let trunk1_b = take(hidden);
        let trunk2_w = take(hidden * hidden);
        let trunk2_b = take(hidden);
        let actor_w = take(hidden * outputs);
        let actor_b = take(outputs);
        let critic_w = take(hidden);
        let critic_b = take(1);

        Ok(Self {
            trunk1: linear_from_parts::<B>(trunk1_w, trunk1_b, inputs, hidden, device),
            trunk2: linear_from_parts::<B>(trunk2_w, trunk2_b, hidden, hidden, device),
            actor_head: linear_from_parts::<B>(actor_w, actor_b, hidden, outputs, device),
            critic_head: linear_from_parts::<B>(critic_w, critic_b, hidden, 1, device),
        })
    }

    pub fn forward(&self, input: Tensor<B, 2>) -> (Tensor<B, 2>, Tensor<B, 2>) {
        let x = self.trunk1.forward(input);
        let x = burn::tensor::activation::relu(x);
        let x = self.trunk2.forward(x);
        let x = burn::tensor::activation::relu(x);

        let actor_out = self.actor_head.forward(x.clone());
        let actor_out = burn::tensor::activation::sigmoid(actor_out);

        let critic_out = self.critic_head.forward(x);

        (actor_out, critic_out)
    }
}

/// Re-lay an output-major matrix `w[out * d_in + in]` into Burn's `[d_in, d_out]` order.
///
/// Separated out and unit-tested because a transpose bug here is invisible: both orderings have the
/// same length, and for a square layer the result still runs.
fn transpose_to_burn(src: &[f32], d_in: usize, d_out: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; d_in * d_out];
    for o in 0..d_out {
        for i in 0..d_in {
            out[i * d_out + o] = src[o * d_in + i];
        }
    }
    out
}

/// Build a `Linear` whose weights are fixed but whose tensors are materialised **lazily**, exactly
/// as `LinearConfig::init` does.
///
/// Laziness matters for startup cost, not correctness: eagerly uploading every layer to the GPU at
/// construction pushed `SimulationEngine::start` past the point where the stress tests observe a
/// first tick. Determinism is unaffected — the values are decided here; only the moment they reach
/// the device is deferred.
fn linear_from_parts<B: Backend>(
    weight: &[f32],
    bias: &[f32],
    d_in: usize,
    d_out: usize,
    device: &B::Device,
) -> Linear<B> {
    let w = transpose_to_burn(weight, d_in, d_out);
    let b = bias.to_vec();

    let weight_param = burn::module::Param::uninitialized(
        burn::module::ParamId::new(),
        move |device: &B::Device, require_grad: bool| {
            let data: Data<B::FloatElem, 2> =
                Data::new(w.clone(), Shape::new([d_in, d_out])).convert();
            let tensor = Tensor::<B, 2>::from_data(data, device);
            if require_grad {
                tensor.require_grad()
            } else {
                tensor
            }
        },
        device.clone(),
        true,
    );

    let bias_param = burn::module::Param::uninitialized(
        burn::module::ParamId::new(),
        move |device: &B::Device, require_grad: bool| {
            let data: Data<B::FloatElem, 1> = Data::new(b.clone(), Shape::new([d_out])).convert();
            let tensor = Tensor::<B, 1>::from_data(data, device);
            if require_grad {
                tensor.require_grad()
            } else {
                tensor
            }
        },
        device.clone(),
        true,
    );

    Linear {
        weight: weight_param,
        bias: Some(bias_param),
    }
}

/// Reusable buffers for [`run_inference_batch`].
///
/// Held by the caller and reused across batches: the inference worker sits on the tick path's
/// critical chain, so allocating per batch would show up as frame jitter.
#[derive(Default)]
pub struct InferenceScratch {
    inputs: Vec<f32>,
    shared_slots: Vec<usize>,
    shared_actions: Vec<Option<[f32; crate::core::agent_systems::ACTION_SLOTS]>>,
    brain_hidden: Vec<f32>,
    brain_outputs: Vec<f32>,
}

impl InferenceScratch {
    pub fn with_capacity(agents: usize) -> Self {
        Self {
            inputs: Vec::with_capacity(agents * 15),
            shared_slots: Vec::with_capacity(agents),
            shared_actions: Vec::with_capacity(agents),
            brain_hidden: Vec::with_capacity(256),
            brain_outputs: Vec::with_capacity(crate::core::agent_systems::ACTION_SLOTS),
        }
    }
}

/// Turn a batch of inference requests into responses.
///
/// Extracted from the worker thread so the same code that runs the simulation can be driven
/// synchronously from a test. Logic that only exists inside a spawned closure cannot be checked, and
/// this is the function that decides what every agent does each tick.
///
/// Agents carrying their own brain are computed individually; the rest are batched through the
/// shared model. **Filtering** rather than running everything through Burn and overwriting is what
/// keeps the legacy path identical when no agent has a brain — the EB-S04 baseline — and avoids
/// paying for a forward pass whose result is discarded.
pub fn run_inference_batch(
    brain_model: &BrainModel,
    requests: &[crate::core::agent_systems::AgentInferenceRequest],
    responses: &mut Vec<crate::core::agent_systems::AgentInferenceResponse>,
    scratch: &mut InferenceScratch,
) {
    use crate::core::agent_systems::{AgentInferenceResponse, ACTION_SLOTS};
    use crate::evolution::brain_genotype::action_index;

    responses.clear();
    scratch.shared_slots.clear();
    scratch.inputs.clear();
    for (idx, req) in requests.iter().enumerate() {
        if req.brain.is_none() {
            scratch.shared_slots.push(idx);
            scratch.inputs.extend_from_slice(&req.sensory_input);
        }
    }
    let batch_size = scratch.shared_slots.len();

    // An empty batch would make Burn build a zero-row tensor, so skip it when every agent has a brain.
    let outputs_vec = if batch_size == 0 {
        Vec::new()
    } else {
        match &brain_model.backend {
            BrainModelBackend::NdArray(model, device) => {
                let data = Data::new(scratch.inputs.clone(), Shape::new([batch_size, 15]));
                let input_tensor = Tensor::<burn_ndarray::NdArray<f32>, 2>::from_data(data, device);
                let (actor_out, _) = model.forward(input_tensor);
                actor_out.into_data().value
            }
            #[cfg(feature = "ml-wgpu")]
            BrainModelBackend::Wgpu(model, device) => {
                let data = Data::new(scratch.inputs.clone(), Shape::new([batch_size, 15]));
                let input_tensor =
                    Tensor::<burn_wgpu::Wgpu<burn_wgpu::AutoGraphicsApi, f32, i32>, 2>::from_data(
                        data, device,
                    );
                let (actor_out, _) = model.forward(input_tensor);
                actor_out.into_data().value
            }
        }
    };

    scratch.shared_actions.clear();
    scratch.shared_actions.resize(requests.len(), None);
    for (row, &req_idx) in scratch.shared_slots.iter().enumerate() {
        let mut actions = AgentInferenceResponse::open_gates_default();
        for (k, action) in actions.iter_mut().take(action_index::CPG_LEN).enumerate() {
            if let Some(&val) = outputs_vec.get(row * action_index::CPG_LEN + k) {
                *action = val;
            }
        }
        scratch.shared_actions[req_idx] = Some(actions);
    }

    for (req_idx, req) in requests.iter().enumerate() {
        let actions = match (&req.brain, scratch.shared_actions[req_idx]) {
            // The agent's own brain: a hand-written forward pass over caller-owned scratch, proven
            // equivalent to Burn's on identical weights by the EB-S02 parity gate.
            (Some(brain), _) => {
                scratch.brain_hidden.clear();
                scratch.brain_hidden.resize(brain.scratch_len(), 0.0);
                scratch.brain_outputs.clear();
                scratch.brain_outputs.resize(brain.arch.outputs, 0.0);

                match brain.forward_into(
                    &req.sensory_input,
                    &mut scratch.brain_hidden,
                    &mut scratch.brain_outputs,
                ) {
                    Ok(_value) => {
                        let mut actions = [0.0f32; ACTION_SLOTS];
                        let n = brain.arch.outputs.min(ACTION_SLOTS);
                        actions[..n].copy_from_slice(&scratch.brain_outputs[..n]);
                        actions
                    }
                    // A brain that cannot run must not freeze its agent's ecology: fall back to open
                    // gates and no locomotion change, never to an all-zero vector — that would read
                    // as "every gate shut", i.e. an agent that silently stops eating.
                    Err(_) => AgentInferenceResponse::open_gates_default(),
                }
            }
            (None, Some(actions)) => actions,
            (None, None) => AgentInferenceResponse::open_gates_default(),
        };
        responses.push(AgentInferenceResponse {
            entity: req.entity,
            actions,
            request_id: req.request_id,
        });
    }
}

pub enum BrainModelBackend {
    NdArray(
        ActorCriticModel<burn_ndarray::NdArray<f32>>,
        burn_ndarray::NdArrayDevice,
    ),
    #[cfg(feature = "ml-wgpu")]
    Wgpu(
        ActorCriticModel<burn_wgpu::Wgpu<burn_wgpu::AutoGraphicsApi, f32, i32>>,
        burn_wgpu::WgpuDevice,
    ),
}

pub struct BrainModel {
    pub backend: BrainModelBackend,
    pub input_dim: usize,
    pub action_dim: usize,
}

// SAFETY: these are the only two `unsafe` blocks in the backend, and they used to carry no argument
// at all. The argument is below. It was checked against burn 0.13.2 on 2026-07-27 and it is
// **version-specific** — re-derive it on any burn upgrade.
//
// ## Why the auto traits do not apply
//
// Not because of `WgpuDevice`, which is the obvious suspect and is innocent. Replacing these impls
// with a `T: Send + Sync` static assertion produced the real reason, on **both** backends:
//
//     `std::cell::OnceCell<Tensor<NdArray, 2>>` cannot be shared between threads safely
//     `dyn Fn(&WgpuDevice, bool) -> Tensor<..., 2> + Send` cannot be shared between threads safely
//
// burn's `Param<T>` is `{ state: OnceCell<T>, initialization: Option<RwLock<Option<Uninitialized<T>>>> }`.
// It is `!Sync` because of the `OnceCell` and the `+ Send`-but-not-`+ Sync` initializer closure.
// So it is `Sync`, not `Send`, that fails, and it fails on the CPU path too — this was never a
// GPU-only concern.
//
// ## Why sharing is nonetheless sound here
//
// The hazard a `OnceCell` presents is *concurrent first write*. `BrainModel` is inserted as a Bevy
// resource and Bevy runs `Res<T>` readers in parallel, so two systems first-touching a parameter
// simultaneously would be a genuine data race.
//
// That cannot happen because every constructor of this type drains the laziness before the value
// escapes: `new` and `new_seeded` both call `materialize_params` (GPU path via
// `wgpu_survives_one_forward_pass`), which forces a full forward pass and `into_data` on the
// constructing thread. After that every `OnceCell` is populated, and a populated `OnceCell` is only
// ever read — `Param::val()` takes `&self` and cannot clear it. Read-only shared access to
// immutable data is safe regardless of the `Sync` bound.
//
// This was not true before 2026-07-27: only the wgpu path forced a pass, so the CPU path handed out
// a model with empty cells and the impls below were covering a real latent race.
//
// ## What must stay true
//
// 1. Every path that constructs a `BrainModel` calls `materialize_params` before returning it.
//    `a_freshly_built_model_has_no_lazy_parameters_left` is the gate.
// 2. Nothing mutates parameters through a shared `&BrainModel`. In-place learning goes through
//    `&mut` (`hrrl_learning_system`), which excludes concurrent readers by borrow.
// 3. On a burn upgrade, re-run the static assertion in that test: if `Param` becomes `Sync`, delete
//    these impls rather than keeping them, because a redundant `unsafe impl` silences the compiler
//    the day the bound is genuinely lost.
unsafe impl Send for BrainModel {}
unsafe impl Sync for BrainModel {}

impl bevy_ecs::system::Resource for BrainModel {}

impl BrainModel {
    /// Build the shared model with **reproducible** weights.
    ///
    /// The shared brain used to be different on every launch, so two runs of the same world with the
    /// same `SimRng` still diverged — invisible from outside, because a randomly initialised network
    /// behaves just as plausibly as any other. The controlled-comparison harness
    /// (`tests/brain_controlled_comparison_tests.rs`) surfaced it: positions and energies matched
    /// across runs while the CPG parameters did not.
    ///
    /// ### Why `Backend::seed` is not enough
    ///
    /// `LinearConfig::init` returns `Param::uninitialized`: the weights are materialised **lazily**,
    /// on first use, from a process-wide RNG that *advances* with every draw. Seeding before
    /// construction therefore fixes nothing — build two models, then run them, and the second draws
    /// from a generator the first has already advanced. Determinism has to come from supplying the
    /// numbers, not from seeding a generator that will be consumed at an unpredictable moment.
    ///
    /// So this draws the weights here, from an explicitly seeded stream, and installs them through
    /// [`ActorCriticModel::from_flat_weights`] — the same loader the EB-S02 parity gate exercises.
    /// The distribution is unchanged: Burn's `LinearConfig` default is `U(-k, k)` with
    /// `k = sqrt(1/fan_in)` for both weights and biases, and that is what is reproduced below.
    pub fn new_seeded(input_dim: usize, hidden_dim: usize, action_dim: usize, seed: u64) -> Self {
        use rand::{Rng, SeedableRng};

        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut weights = Vec::new();
        // `U(-k, k)`, `k = sqrt(1/fan_in)` — Burn's `LinearConfig` default, per layer.
        let mut draw = |count: usize, fan_in: usize, rng: &mut rand::rngs::StdRng| {
            let k = (1.0 / fan_in as f32).sqrt();
            weights.extend((0..count).map(|_| rng.gen_range(-k..k)));
        };

        draw(input_dim * hidden_dim, input_dim, &mut rng); // trunk1 weight
        draw(hidden_dim, input_dim, &mut rng); // trunk1 bias
        draw(hidden_dim * hidden_dim, hidden_dim, &mut rng); // trunk2 weight
        draw(hidden_dim, hidden_dim, &mut rng); // trunk2 bias
        draw(hidden_dim * action_dim, hidden_dim, &mut rng); // actor weight
        draw(action_dim, hidden_dim, &mut rng); // actor bias
        draw(hidden_dim, hidden_dim, &mut rng); // critic weight
        draw(1, hidden_dim, &mut rng); // critic bias

        #[cfg_attr(not(feature = "ml-wgpu"), allow(unused_variables))]
        let use_gpu = std::env::var("ANIMA_USE_GPU")
            .map(|val| val != "false" && val != "0")
            .unwrap_or(true);

        #[cfg(feature = "ml-wgpu")]
        if use_gpu {
            let built = std::panic::catch_unwind(|| {
                let device = burn_wgpu::WgpuDevice::default();
                let model = ActorCriticModel::<WgpuBackend>::from_flat_weights(
                    input_dim, hidden_dim, action_dim, &weights, &device,
                );
                // Inside the guard, and after the model exists: construction is lazy, so this is
                // the first moment an adapter is actually demanded. See
                // `wgpu_survives_one_forward_pass`.
                if let Ok(ref m) = model {
                    wgpu_survives_one_forward_pass(m, &device, input_dim);
                }
                (model, device)
            });
            if let Ok((Ok(model), device)) = built {
                return Self {
                    backend: BrainModelBackend::Wgpu(model, device),
                    input_dim,
                    action_dim,
                };
            }
            eprintln!("WGPU initialization failed, falling back to CPU NdArray.");
        }

        let device = burn_ndarray::NdArrayDevice::Cpu;
        let model = ActorCriticModel::<burn_ndarray::NdArray<f32>>::from_flat_weights(
            input_dim, hidden_dim, action_dim, &weights, &device,
        )
        .expect("weights were built for exactly this architecture");
        // The CPU path needs this for the same reason the GPU path does, and only the GPU path had
        // it: draining lazy `Param` initialization here is what makes `unsafe impl Sync` sound.
        materialize_params(&model, &device, input_dim);
        Self {
            backend: BrainModelBackend::NdArray(model, device),
            input_dim,
            action_dim,
        }
    }

    pub fn new(input_dim: usize, hidden_dim: usize, action_dim: usize) -> Self {
        #[cfg_attr(not(feature = "ml-wgpu"), allow(unused_variables))]
        let use_gpu = std::env::var("ANIMA_USE_GPU")
            .map(|val| val != "false" && val != "0")
            .unwrap_or(true);

        #[cfg(feature = "ml-wgpu")]
        if use_gpu {
            let wgpu_res = std::panic::catch_unwind(|| {
                let device = burn_wgpu::WgpuDevice::default();
                let model = ActorCriticModel::<WgpuBackend>::new(
                    input_dim, hidden_dim, action_dim, &device,
                );
                // Same reason as in `new_seeded`: without this the probe only proves that a struct
                // can be allocated, and the machine is asked for a GPU much later.
                wgpu_survives_one_forward_pass(&model, &device, input_dim);
                (model, device)
            });

            match wgpu_res {
                Ok((model, device)) => {
                    return Self {
                        backend: BrainModelBackend::Wgpu(model, device),
                        input_dim,
                        action_dim,
                    };
                }
                Err(_) => {
                    eprintln!("WGPU initialization failed, falling back to CPU NdArray.");
                }
            }
        }

        let device = burn_ndarray::NdArrayDevice::Cpu;
        let model = ActorCriticModel::<burn_ndarray::NdArray<f32>>::new(
            input_dim, hidden_dim, action_dim, &device,
        );
        materialize_params(&model, &device, input_dim);
        Self {
            backend: BrainModelBackend::NdArray(model, device),
            input_dim,
            action_dim,
        }
    }
}

#[derive(Resource, Default)]
pub struct BrainInferenceBuffer {
    pub inputs: Vec<f32>,
    pub outputs: Vec<f32>,
    pub agent_entities: Vec<Entity>,
    pub child_segments: Vec<(u32, Entity)>,
    pub agent_states: Vec<[f32; 15]>,
    pub segment_by_parent: Vec<(Entity, u32, Entity)>,
    pub segment_list: Vec<(u32, Entity, Option<usize>)>,
    pub parent_head: std::collections::HashMap<Entity, usize>,
}

pub fn brain_inference_system(
    brain_model: Res<BrainModel>,
    mut brain_buf: ResMut<BrainInferenceBuffer>,
    agent_query: Query<
        (
            Entity,
            &Position,
            &Rotation,
            &HomeostaticState,
            Option<&Predator>,
            Option<&crate::ai::pheromone::OlfactorySensors>,
        ),
        With<crate::core::ecs::Agent>,
    >,
    food_query: Query<&Position, With<Food>>,
    prey_query: Query<(&Position, &HomeostaticState), (With<crate::core::ecs::Agent>, With<Prey>)>,
    mut oscillator_query: Query<&mut CpgOscillator>,
    segment_query: Query<(Entity, &ParentAgent, &Segment)>,
    mut last_state_query: Query<&mut crate::ai::hrrl::LastTransitionState>,
    spatial_grid: Option<Res<crate::physics::SpatialHashGrid>>,
    bounds: Option<Res<crate::core::ecs::MapBounds>>,
    collider_query: Query<(&Position, &crate::physics::SpatialCollider)>,
    food_tag_query: Query<(), With<Food>>,
    predator_tag_query: Query<(), With<Predator>>,
    prey_tag_query: Query<(), With<Prey>>,
    parent_agent_query: Query<&ParentAgent>,
    mut active_raycasts: Option<ResMut<crate::core::ecs::ActiveRaycasts>>,
) {
    if let Some(ref mut raycasts_res) = active_raycasts {
        raycasts_res.raycasts.clear();
    }

    let mut inputs = std::mem::take(&mut brain_buf.inputs);
    inputs.clear();

    let mut agent_entities = std::mem::take(&mut brain_buf.agent_entities);
    agent_entities.clear();

    let mut agent_inputs_list = std::mem::take(&mut brain_buf.agent_states);
    agent_inputs_list.clear();

    // Loop through agents and construct input features
    for (entity, agent_pos, rotation, homeo, opt_predator, opt_sensors) in agent_query.iter() {
        let is_predator = opt_predator.is_some();
        let target_pos = if is_predator {
            // Predator: target nearest active Prey agent
            let mut nearest_prey = None;
            let mut min_dist_sq = f32::MAX;
            for (prey_pos, prey_homeo) in prey_query.iter() {
                if prey_homeo.energy > 0.0 {
                    let dist_sq = agent_pos.0.distance_squared(prey_pos.0);
                    if dist_sq < min_dist_sq {
                        min_dist_sq = dist_sq;
                        nearest_prey = Some(prey_pos.0);
                    }
                }
            }
            nearest_prey
        } else {
            // Prey: target nearest active Food node
            let mut nearest_food = None;
            let mut min_dist_sq = f32::MAX;
            for food_pos in food_query.iter() {
                let dist_sq = agent_pos.0.distance_squared(food_pos.0);
                if dist_sq < min_dist_sq {
                    min_dist_sq = dist_sq;
                    nearest_food = Some(food_pos.0);
                }
            }
            nearest_food
        };

        let local_target_vec = if let Some(t_pos) = target_pos {
            rotation.0.inverse() * (t_pos - agent_pos.0)
        } else {
            glam::Vec3::ZERO
        };

        // 1. Raycast logic
        let mut hit_distance = 10.0; // max sensor range
        let mut hit_is_food = 0.0;
        let mut hit_is_predator = 0.0;
        let mut hit_is_prey = 0.0;
        let mut hit_type = crate::core::ecs::HitEntityType::None;
        let direction = rotation.0 * glam::Vec3::Z; // Forward is positive Z

        if let (Some(grid), Some(map_bounds)) = (&spatial_grid, &bounds) {
            let ray = crate::physics::Ray3D {
                origin: agent_pos.0,
                direction,
            };

            if let Some(hit) = grid.raycast(&ray, 10.0, map_bounds, &collider_query) {
                // Ignore self-collisions (own body segments)
                let root_agent_id = if let Ok(parent) = parent_agent_query.get(hit.entity) {
                    parent.0
                } else {
                    hit.entity
                };

                if root_agent_id != entity {
                    hit_distance = hit.distance;
                    if food_tag_query.get(hit.entity).is_ok() {
                        hit_is_food = 1.0;
                        hit_type = crate::core::ecs::HitEntityType::Food;
                    } else if predator_tag_query.get(root_agent_id).is_ok() {
                        hit_is_predator = 1.0;
                        hit_type = crate::core::ecs::HitEntityType::Predator;
                    } else if prey_tag_query.get(root_agent_id).is_ok() {
                        hit_is_prey = 1.0;
                        hit_type = crate::core::ecs::HitEntityType::Prey;
                    } else {
                        hit_type = crate::core::ecs::HitEntityType::Obstacle;
                    }
                }
            }
        }

        if let Some(ref mut raycasts_res) = active_raycasts {
            raycasts_res
                .raycasts
                .push(crate::core::ecs::RaycastTelemetry {
                    origin: agent_pos.0.to_array(),
                    direction: direction.to_array(),
                    hit_distance,
                    hit_entity_type: hit_type,
                    agent_id: entity.index(),
                });
        }

        // 2. Olfactory Readings
        let (left_reading, right_reading) = if let Some(sensors) = opt_sensors {
            (sensors.left_reading, sensors.right_reading)
        } else {
            (0.0, 0.0)
        };

        let state_arr = [
            local_target_vec.x,
            local_target_vec.y,
            local_target_vec.z,
            homeo.energy,
            homeo.energy_target,
            homeo.hydration,
            homeo.hydration_target,
            homeo.temperature,
            homeo.temp_target,
            hit_distance,
            hit_is_food,
            hit_is_predator,
            hit_is_prey,
            left_reading,
            right_reading,
        ];
        agent_inputs_list.push(state_arr);

        inputs.push(local_target_vec.x);
        inputs.push(local_target_vec.y);
        inputs.push(local_target_vec.z);
        inputs.push(homeo.energy);
        inputs.push(homeo.energy_target);
        inputs.push(homeo.hydration);
        inputs.push(homeo.hydration_target);
        inputs.push(homeo.temperature);
        inputs.push(homeo.temp_target);
        inputs.push(hit_distance);
        inputs.push(hit_is_food);
        inputs.push(hit_is_predator);
        inputs.push(hit_is_prey);
        inputs.push(left_reading);
        inputs.push(right_reading);

        agent_entities.push(entity);
    }

    let batch_size = agent_entities.len();
    if batch_size == 0 {
        // Return vectors to buffer
        brain_buf.inputs = inputs;
        brain_buf.agent_entities = agent_entities;
        brain_buf.agent_states = agent_inputs_list;
        return;
    }

    let outputs_vec = match &brain_model.backend {
        BrainModelBackend::NdArray(model, device) => {
            let data = Data::new(inputs, Shape::new([batch_size, 15]));
            let input_tensor = Tensor::<burn_ndarray::NdArray<f32>, 2>::from_data(data, device);
            let (actor_out, _) = model.forward(input_tensor);
            actor_out.into_data().value
        }
        #[cfg(feature = "ml-wgpu")]
        BrainModelBackend::Wgpu(model, device) => {
            let data = Data::new(inputs, Shape::new([batch_size, 15]));
            let input_tensor =
                Tensor::<burn_wgpu::Wgpu<burn_wgpu::AutoGraphicsApi, f32, i32>, 2>::from_data(
                    data, device,
                );
            let (actor_out, _) = model.forward(input_tensor);
            actor_out.into_data().value
        }
    };

    let mut segment_list = std::mem::take(&mut brain_buf.segment_list);
    segment_list.clear();
    let mut parent_head = std::mem::take(&mut brain_buf.parent_head);
    parent_head.clear();

    for (seg_entity, parent_agent, segment) in segment_query.iter() {
        let parent = parent_agent.0;
        let seg_idx = segment_list.len();
        let next = parent_head.insert(parent, seg_idx);
        segment_list.push((segment.id, seg_entity, next));
    }

    let mut child_segments = std::mem::take(&mut brain_buf.child_segments);

    for (agent_idx, &agent_entity) in agent_entities.iter().enumerate() {
        child_segments.clear();
        if let Some(&first_idx) = parent_head.get(&agent_entity) {
            let mut curr = Some(first_idx);
            while let Some(idx) = curr {
                let (id, seg_entity, next) = segment_list[idx];
                child_segments.push((id, seg_entity));
                curr = next;
            }
        }
        child_segments.sort_unstable_by_key(|&(id, _)| id);

        for (seg_idx, &(_, seg_entity)) in child_segments.iter().enumerate() {
            if let Ok(mut osc) = oscillator_query.get_mut(seg_entity) {
                let freq_idx = agent_idx * 4 + seg_idx * 2;
                let amp_idx = agent_idx * 4 + seg_idx * 2 + 1;

                if let Some(&freq_raw) = outputs_vec.get(freq_idx) {
                    osc.frequency = 0.1 + freq_raw * 2.9;
                }
                if let Some(&amp_raw) = outputs_vec.get(amp_idx) {
                    osc.amplitude = amp_raw * 1.5;
                }
            }
        }

        // Save last transition state
        let mut action = [0.0; 4];
        for (k, act_val) in action.iter_mut().enumerate() {
            if let Some(&val) = outputs_vec.get(agent_idx * 4 + k) {
                *act_val = val;
            }
        }
        if let Ok(mut last) = last_state_query.get_mut(agent_entity) {
            last.state = agent_inputs_list[agent_idx];
            last.action = action;
            last.has_last = true;
        }
    }

    // Reclaim vectors
    brain_buf.inputs = outputs_vec;
    brain_buf.agent_entities = agent_entities;
    brain_buf.child_segments = child_segments;
    brain_buf.agent_states = agent_inputs_list;
    brain_buf.segment_list = segment_list;
    brain_buf.parent_head = parent_head;
}

/// Apply one in-life learning step to every eligible agent.
///
/// ADR-0003 decision 6. Off unless both `evolved` and `lifetime_learning.enabled` are set, so a
/// default run is untouched: with the flag off this system reads one resource and returns.
///
/// The reward is the same homeostatic drive-reduction the shared model trains on — the improvement
/// in `HomeostaticState::compute_deviation` since last tick — so an agent is rewarded for moving
/// toward its own physiological setpoints rather than toward a designer's objective.
///
/// Learning never touches [`crate::core::components::AgentBrain::genotype`]: what an individual
/// learns dies with it. That is the Baldwin position — evolution can select for brains that *learn
/// well*, but not inherit what was learned.
pub fn lifetime_learning_system(
    policy: Option<Res<crate::core::resources::BrainPolicy>>,
    lod_focus: Option<Res<crate::core::simulation_lod::LodFocus>>,
    mut tick: Local<u32>,
    mut scratch: Local<crate::evolution::brain_genotype::LearnScratch>,
    mut agents: Query<(
        &Position,
        &HomeostaticState,
        &crate::ai::hrrl::LastTransitionState,
        &mut crate::core::components::AgentBrain,
    )>,
) {
    let Some(policy) = policy else { return };
    let cfg = policy.lifetime_learning;
    if !policy.evolved || !cfg.enabled || cfg.interval == 0 {
        return;
    }

    *tick = tick.wrapping_add(1);
    if !(*tick).is_multiple_of(cfg.interval) {
        return;
    }

    for (pos, homeo, last, mut brain) in agents.iter_mut() {
        if !last.has_last {
            continue; // nothing was done yet, so there is nothing to learn from
        }
        // Active-radius gate: agents outside it are not simulated in enough detail to learn from.
        //
        // Measured from the simulation-LOD focus, which is what ADR-0003 decision 6 asked for. Until
        // LOD existed this fell back to the world origin — a stand-in that made the constraint
        // testable but put the "active" region wherever the map happened to be centred. With no
        // focus set it still falls back to the origin, so headless runs are unchanged.
        let center = lod_focus
            .as_deref()
            .filter(|f| f.enabled)
            .map(|f| f.center)
            .unwrap_or(glam::Vec3::ZERO);
        if cfg.active_radius.is_finite() && pos.0.distance(center) > cfg.active_radius {
            continue;
        }

        let reward = homeo.previous_deviation - homeo.compute_deviation();
        let mut updated = (**brain.live()).clone();

        // `V(s')` for the TD target, from the network as it stands before the update.
        let next_value = match updated.forward(&last.state) {
            Ok((_, value)) => value,
            Err(_) => continue,
        };

        if crate::evolution::brain_genotype::learn_step(
            &mut updated,
            &last.state,
            &last.action,
            reward,
            next_value,
            cfg.discount,
            cfg.learning_rate,
            &mut scratch,
        )
        .is_ok()
        {
            brain.set_learned(updated);
        }
    }
}

pub fn hrrl_learning_system(
    mut agent_set: ParamSet<(
        Query<(&Position, &HomeostaticState), (With<crate::core::ecs::Agent>, With<Prey>)>,
        Query<(
            Entity,
            &Position,
            &Rotation,
            &mut HomeostaticState,
            &mut crate::ai::hrrl::LastTransitionState,
            Option<&Predator>,
            Option<&crate::ai::pheromone::OlfactorySensors>,
        )>,
    )>,
    food_query: Query<&Position, With<Food>>,
    transition_sender: Option<Res<crate::ai::hrrl::TransitionSender>>,
    spatial_grid: Option<Res<crate::physics::SpatialHashGrid>>,
    bounds: Option<Res<crate::core::ecs::MapBounds>>,
    collider_query: Query<(&Position, &crate::physics::SpatialCollider)>,
    food_tag_query: Query<(), With<Food>>,
    predator_tag_query: Query<(), With<Predator>>,
    prey_tag_query: Query<(), With<Prey>>,
    parent_agent_query: Query<&ParentAgent>,
) {
    let mut prey_data = [(glam::Vec3::ZERO, 0.0f32); 256];
    let mut prey_count = 0;
    for (pos, homeo) in agent_set.p0().iter() {
        if prey_count < 256 {
            prey_data[prey_count] = (pos.0, homeo.energy);
            prey_count += 1;
        }
    }

    let mut agent_query = agent_set.p1();
    for (entity, agent_pos, rotation, mut homeo, mut last, opt_predator, opt_sensors) in
        agent_query.iter_mut()
    {
        let is_predator = opt_predator.is_some();
        let target_pos = if is_predator {
            // Predator: target nearest active Prey agent from the pre-collected stack buffer
            let mut nearest_prey = None;
            let mut min_dist_sq = f32::MAX;
            for &(prey_pos, prey_energy) in prey_data.iter().take(prey_count) {
                if prey_energy > 0.0 {
                    let dist_sq = agent_pos.0.distance_squared(prey_pos);
                    if dist_sq < min_dist_sq {
                        min_dist_sq = dist_sq;
                        nearest_prey = Some(prey_pos);
                    }
                }
            }
            nearest_prey
        } else {
            // Prey: target nearest active Food node
            let mut nearest_food = None;
            let mut min_dist_sq = f32::MAX;
            for food_pos in food_query.iter() {
                let dist_sq = agent_pos.0.distance_squared(food_pos.0);
                if dist_sq < min_dist_sq {
                    min_dist_sq = dist_sq;
                    nearest_food = Some(food_pos.0);
                }
            }
            nearest_food
        };

        let local_target_vec = if let Some(t_pos) = target_pos {
            rotation.0.inverse() * (t_pos - agent_pos.0)
        } else {
            glam::Vec3::ZERO
        };

        // 1. Raycast logic
        let mut hit_distance = 10.0;
        let mut hit_is_food = 0.0;
        let mut hit_is_predator = 0.0;
        let mut hit_is_prey = 0.0;

        if let (Some(grid), Some(map_bounds)) = (&spatial_grid, &bounds) {
            let direction = rotation.0 * glam::Vec3::Z;
            let ray = crate::physics::Ray3D {
                origin: agent_pos.0,
                direction,
            };
            if let Some(hit) = grid.raycast(&ray, 10.0, map_bounds, &collider_query) {
                let root_agent_id = if let Ok(parent) = parent_agent_query.get(hit.entity) {
                    parent.0
                } else {
                    hit.entity
                };

                if root_agent_id != entity {
                    hit_distance = hit.distance;
                    if food_tag_query.get(hit.entity).is_ok() {
                        hit_is_food = 1.0;
                    } else if predator_tag_query.get(root_agent_id).is_ok() {
                        hit_is_predator = 1.0;
                    } else if prey_tag_query.get(root_agent_id).is_ok() {
                        hit_is_prey = 1.0;
                    }
                }
            }
        }

        // 2. Olfactory Readings
        let (left_reading, right_reading) = if let Some(sensors) = opt_sensors {
            (sensors.left_reading, sensors.right_reading)
        } else {
            (0.0, 0.0)
        };

        let current_state = [
            local_target_vec.x,
            local_target_vec.y,
            local_target_vec.z,
            homeo.energy,
            homeo.energy_target,
            homeo.hydration,
            homeo.hydration_target,
            homeo.temperature,
            homeo.temp_target,
            hit_distance,
            hit_is_food,
            hit_is_predator,
            hit_is_prey,
            left_reading,
            right_reading,
        ];

        let current_deviation = homeo.compute_deviation();

        if last.has_last {
            let reward = homeo.previous_deviation - current_deviation;
            if let Some(ref sender) = transition_sender {
                let transition = crate::ai::hrrl::Transition {
                    state: last.state,
                    action: last.action,
                    reward,
                    next_state: current_state,
                };
                let _ = sender.0.send(transition);
            }
        }

        homeo.previous_deviation = current_deviation;
        last.state = current_state;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Requirement 1 of the `unsafe impl Send/Sync` proof above, made checkable.
    ///
    /// The soundness of sharing a `BrainModel` rests on every parameter's lazy `OnceCell` having
    /// been filled on the constructing thread. burn 0.13 exposes no way to *ask* a `Param` whether
    /// it is initialized, so this scans the source instead — the same idiom `sim_determinism_tests`
    /// uses to keep `thread_rng()` out. It is not elegant; it is the only thing that fails when a
    /// future constructor forgets, which is exactly the mistake that made the CPU path unsound for
    /// as long as it did.
    #[test]
    fn every_brain_model_constructor_materializes_its_parameters() {
        let whole = include_str!("model.rs");

        // Production code only. Without this cut the scan matches its OWN search string a few lines
        // below and reports a construction site inside this test — which is exactly what happened
        // the first time it ran.
        let src = whole
            .split_once("\n#[cfg(test)]")
            .map(|(before, _)| before)
            .expect("this file has a test module");

        // Each place a `BrainModel` is produced with a backend in it.
        let sites: Vec<usize> = src
            .match_indices("backend: BrainModelBackend::")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(
            sites.len(),
            4,
            "expected exactly the four known construction sites (new/new_seeded x wgpu/ndarray); \
             found {}. A new one needs a materialize_params call, not a bigger number here.",
            sites.len()
        );

        for site in sites {
            // Look back over the enclosing construction for a materialization call. The window is
            // generous because the wgpu paths route through `wgpu_survives_one_forward_pass`.
            let start = site.saturating_sub(1400);
            let window = &src[start..site];
            assert!(
                window.contains("materialize_params(")
                    || window.contains("wgpu_survives_one_forward_pass("),
                "a BrainModel is constructed at byte {site} without draining lazy Param \
                 initialization first. See the SAFETY note on `unsafe impl Send for BrainModel`: \
                 an unmaterialized OnceCell shared across Bevy's parallel `Res<T>` readers is a \
                 data race that `unsafe impl Sync` will happily compile."
            );
        }

        // Negative control: the scan can fail. If this ever matches, the search string drifted and
        // the loop above has been vacuously passing.
        assert!(
            !src.contains("backend: BrainModelBackendThatDoesNotExist::"),
            "control string should never match"
        );
    }

    /// A shared `&BrainModel` must survive being held by many threads at once — the shape Bevy's
    /// parallel `Res<T>` scheduling creates.
    ///
    /// Deliberately modest about what it proves: a data race is non-deterministic, so a green run
    /// here is **not** evidence of soundness and must not be cited as such. The deterministic gate
    /// is `every_brain_model_constructor_materializes_its_parameters` above; this one only catches
    /// the blunt failure, a model that cannot cross a thread boundary at all.
    #[test]
    fn a_model_can_be_shared_across_threads() {
        let model = std::sync::Arc::new(BrainModel::new_seeded(15, 64, 4, 7));
        assert_eq!((model.input_dim, model.action_dim), (15, 4));

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let m = std::sync::Arc::clone(&model);
                std::thread::spawn(move || (m.input_dim, m.action_dim))
            })
            .collect();

        for h in handles {
            assert_eq!(
                h.join()
                    .expect("no thread may panic holding a shared model"),
                (15, 4)
            );
        }
    }

    #[test]
    fn transpose_reorders_output_major_into_burn_order() {
        // Output-major 2x3 (`d_in = 3`, `d_out = 2`): neuron 0 reads [1,2,3], neuron 1 reads [4,5,6].
        let src = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        // Burn wants `[d_in, d_out]`: row i holds input i's weight to each output.
        assert_eq!(
            transpose_to_burn(&src, 3, 2),
            vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]
        );
    }

    #[test]
    fn transpose_is_an_involution_on_square_layers() {
        let src: Vec<f32> = (0..16).map(|i| i as f32).collect();
        let once = transpose_to_burn(&src, 4, 4);
        assert_ne!(once, src, "a square transpose must still move elements");
        assert_eq!(transpose_to_burn(&once, 4, 4), src);
    }

    #[test]
    fn from_flat_weights_rejects_bad_shapes_and_lengths() {
        let device = burn_ndarray::NdArrayDevice::Cpu;
        type B = burn_ndarray::NdArray<f32>;

        assert!(ActorCriticModel::<B>::from_flat_weights(0, 4, 2, &[], &device).is_err());
        assert!(ActorCriticModel::<B>::from_flat_weights(3, 4, 2, &[0.0; 7], &device).is_err());

        let ok_len = (3 * 4 + 4) + (4 * 4 + 4) + (4 * 2 + 2) + (4 + 1);
        assert!(
            ActorCriticModel::<B>::from_flat_weights(3, 4, 2, &vec![0.0; ok_len], &device).is_ok()
        );
    }
}
