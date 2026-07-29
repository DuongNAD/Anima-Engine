//! The shared-model learner: its objective, its loop, and the two backend-specific threads.
//!
//! Split out of `simulation_loop.rs`, which had grown to 1,754 lines with no `#[cfg(test)]` block at
//! all. Nothing here touches `SimulationEngine`'s internals — the learner communicates entirely over
//! crossbeam channels — so it is self-contained in a way the rest of that file is not.
//!
//! The part worth reading before changing anything is the sign discussion on [`a2c_loss`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use burn::backend::Autodiff;
use burn::module::AutodiffModule;
use burn::optim::{AdamConfig, GradientsParams, Optimizer};
use burn::tensor::backend::Backend;
use burn::tensor::{Data, Shape, Tensor};

use crate::ai::hrrl::Transition;
use crate::ai::model::ActorCriticModel;

// These four are `pub` so `benches/tick_systems.rs` can build the learner at the shape the learner
// actually runs at. A benchmark that hard-codes 15/64/4/32 keeps reporting a number for the old
// architecture the first time one of them changes, and nothing fails — the bench still runs, still
// prints a time, and the time is for a network the engine no longer uses.

/// Observation width the shared model is built for, and the stride each transition contributes.
pub const STATE_DIM: usize = 15;
/// Hidden width of both trunk layers.
pub const HIDDEN_DIM: usize = 64;
/// The four CPG locomotion parameters. The ecological gates are evolved, not trained here.
pub const ACTION_DIM: usize = 4;
/// Transitions accumulated before one optimiser step.
pub const BATCH_SIZE: usize = 32;
/// Adam step size for the shared model.
const LEARNING_RATE: f64 = 1e-3;
/// Discount applied to the bootstrapped next-state value in the TD target.
pub const DISCOUNT: f32 = 0.99;

pub enum ModelUpdate {
    NdArray(ActorCriticModel<burn_ndarray::NdArray<f32>>),
    #[cfg(feature = "ml-wgpu")]
    Wgpu(ActorCriticModel<burn_wgpu::Wgpu<burn_wgpu::AutoGraphicsApi, f32, i32>>),
}

/// The four handles a learner thread needs: the running flag, the transition receiver, the model
/// sender and the old-model receiver.
pub type LearnArgs = (
    Arc<AtomicBool>,
    crossbeam_channel::Receiver<Transition>,
    crossbeam_channel::Sender<ModelUpdate>,
    crossbeam_channel::Receiver<ModelUpdate>,
    u64,
);

/// CPU learner. Always available — it is the fallback both when the GPU probe fails and when the
/// `ml-wgpu` feature is off entirely.
pub fn spawn_ndarray_learner(
    args: LearnArgs,
    exit: crate::core::thread_supervisor::ExitToken,
) -> thread::JoinHandle<()> {
    let (running, trans_rx, model_tx, old_model_rx, seed) = args;
    thread::spawn(move || {
        // Moved in so it drops when this thread's stack unwinds — on a normal return and on a panic.
        // Holding it outside would make the thread look permanently alive to `stop` (§3.7).
        let _exit = exit;
        let device = burn_ndarray::NdArrayDevice::Cpu;
        run_training_loop::<burn_ndarray::NdArray<f32>>(
            running,
            trans_rx,
            model_tx,
            old_model_rx,
            device,
            seed,
            ModelUpdate::NdArray,
        );
    })
}

/// GPU learner. Only exists with the `ml-wgpu` feature; the whole wgpu/naga/ash stack goes with it.
#[cfg(feature = "ml-wgpu")]
pub fn spawn_wgpu_learner(
    args: LearnArgs,
    exit: crate::core::thread_supervisor::ExitToken,
) -> thread::JoinHandle<()> {
    let (running, trans_rx, model_tx, old_model_rx, seed) = args;
    thread::spawn(move || {
        let _exit = exit;
        let device = burn_wgpu::WgpuDevice::default();
        run_training_loop::<burn_wgpu::Wgpu<burn_wgpu::AutoGraphicsApi, f32, i32>>(
            running,
            trans_rx,
            model_tx,
            old_model_rx,
            device,
            seed,
            ModelUpdate::Wgpu,
        );
    })
}

/// The A2C objective for the shared model: `L = mean((a − â)²·td) + ½·mean(td²)`.
///
/// ### The sign
///
/// The actor term is advantage-weighted behavioural cloning, and its coefficient is `+td`. With a
/// positive advantage, minimising `td·(a − â)²` shrinks `(a − â)²` and so pulls the policy **toward**
/// the action that turned out better than expected; with a negative advantage the coefficient flips
/// and it pushes away. This matches [`crate::evolution::brain_genotype::learn_step`], which is the
/// per-agent implementation of the same objective.
///
/// The loop used to compute `(a − â)²·(−td)`, which is that objective inverted: a positive advantage
/// made the loss *decrease* as `(a − â)²` grew, so gradient descent drove the shared policy away from
/// actions that turned out well and toward ones that turned out badly. The network still ran and
/// still produced finite numbers, which is why it survived. ADR-0003 recorded the discrepancy and had
/// `learn_step` deliberately implement the correct sign rather than reproduce the defect; there is
/// now one objective rather than two that disagree.
///
/// Runs of the shared model from before the fix followed a different trajectory. That is the point —
/// the old one was descending the wrong gradient.
///
/// Separated from [`run_training_loop`] so it can be tested at all: that loop blocks on a channel
/// forever and owns its optimiser, so nothing about the objective was reachable from a test.
/// `a2c_loss_direction_tests` pairs a finite-difference probe of the loss surface with a behavioural
/// assertion, because a gradient check alone passes just as happily for an inverted objective.
pub fn a2c_loss<B>(
    model: &ActorCriticModel<B>,
    states: Tensor<B, 2>,
    next_states: Tensor<B, 2>,
    actions: Tensor<B, 2>,
    rewards: Tensor<B, 2>,
    discount: f32,
) -> Tensor<B, 1>
where
    B: Backend<FloatElem = f32>,
{
    let (actor_out, critic_out) = model.forward(states);
    let (_, critic_out_next) = model.forward(next_states);

    let target = rewards + critic_out_next.detach() * discount;
    let td_error = target - critic_out;

    let critic_diff = td_error.clone();
    let loss_critic = (critic_diff.clone() * critic_diff).mean();

    let diff = actor_out - actions;
    // `+td`, not `−td`. See the sign discussion above.
    let loss_actor = ((diff.clone() * diff) * td_error.detach()).mean();

    loss_actor + loss_critic * 0.5
}

fn seeded_training_model<B>(device: &B::Device, seed: u64) -> ActorCriticModel<Autodiff<B>>
where
    B: Backend<FloatElem = f32>,
    Autodiff<B>: Backend<FloatElem = f32, IntElem = B::IntElem, Device = B::Device>,
{
    let weights =
        crate::ai::model::BrainModel::seeded_weights(STATE_DIM, HIDDEN_DIM, ACTION_DIM, seed);
    ActorCriticModel::<Autodiff<B>>::from_flat_weights(
        STATE_DIM, HIDDEN_DIM, ACTION_DIM, &weights, device,
    )
    .expect("seeded weights were built for the learner architecture")
}

fn run_training_loop<B>(
    running: Arc<AtomicBool>,
    trans_rx: crossbeam_channel::Receiver<Transition>,
    model_tx: crossbeam_channel::Sender<ModelUpdate>,
    old_model_rx: crossbeam_channel::Receiver<ModelUpdate>,
    device: B::Device,
    seed: u64,
    to_model_update: impl Fn(ActorCriticModel<B>) -> ModelUpdate + Send + 'static,
) where
    B: Backend<FloatElem = f32> + 'static,
    B::Device: Clone + Send + Sync + 'static,
    Autodiff<B>: Backend<FloatElem = f32, IntElem = B::IntElem, Device = B::Device>
        + burn::tensor::backend::AutodiffBackend<
            Device = B::Device,
            FloatElem = f32,
            IntElem = B::IntElem,
        > + 'static,
    ActorCriticModel<Autodiff<B>>:
        AutodiffModule<Autodiff<B>, InnerModule = ActorCriticModel<B>> + Send + 'static,
{
    let mut train_model = seeded_training_model::<B>(&device, seed);
    let mut optim = AdamConfig::new().init();

    let mut batch = Vec::new();
    while running.load(Ordering::SeqCst) {
        match trans_rx.recv_timeout(Duration::from_millis(10)) {
            Ok(transition) => {
                batch.push(transition);
                if batch.len() >= BATCH_SIZE {
                    let mut states_vec = Vec::with_capacity(BATCH_SIZE * STATE_DIM);
                    let mut next_states_vec = Vec::with_capacity(BATCH_SIZE * STATE_DIM);
                    let mut actions_vec = Vec::with_capacity(BATCH_SIZE * ACTION_DIM);
                    let mut rewards_vec = Vec::with_capacity(BATCH_SIZE);
                    for t in batch.iter() {
                        states_vec.extend_from_slice(&t.state);
                        next_states_vec.extend_from_slice(&t.next_state);
                        actions_vec.extend_from_slice(&t.action);
                        rewards_vec.push(t.reward);
                    }

                    let states_tensor = Tensor::<Autodiff<B>, 2>::from_data(
                        Data::new(states_vec, Shape::new([BATCH_SIZE, STATE_DIM])),
                        &device,
                    );
                    let next_states_tensor = Tensor::<Autodiff<B>, 2>::from_data(
                        Data::new(next_states_vec, Shape::new([BATCH_SIZE, STATE_DIM])),
                        &device,
                    );
                    let actions_tensor = Tensor::<Autodiff<B>, 2>::from_data(
                        Data::new(actions_vec, Shape::new([BATCH_SIZE, ACTION_DIM])),
                        &device,
                    );
                    let rewards_tensor = Tensor::<Autodiff<B>, 2>::from_data(
                        Data::new(rewards_vec, Shape::new([BATCH_SIZE, 1])),
                        &device,
                    );

                    let loss_total = a2c_loss(
                        &train_model,
                        states_tensor,
                        next_states_tensor,
                        actions_tensor,
                        rewards_tensor,
                        DISCOUNT,
                    );

                    let grads = loss_total.backward();
                    let grads_params = GradientsParams::from_grads(grads, &train_model);
                    train_model = optim.step(LEARNING_RATE, train_model, grads_params);

                    let eval_model = train_model.valid();
                    let _ = model_tx.send(to_model_update(eval_model));
                    batch.clear();
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
        while let Ok(old_model) = old_model_rx.try_recv() {
            drop(old_model);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type B = burn_ndarray::NdArray<f32>;

    /// The batch shapes the loop builds must match what the model was constructed for. A mismatch is
    /// a panic deep inside Burn at the first full batch, minutes into a run.
    #[test]
    fn the_batch_dimensions_match_the_model_the_loop_builds() {
        let device = burn_ndarray::NdArrayDevice::Cpu;
        let model = ActorCriticModel::<B>::new(STATE_DIM, HIDDEN_DIM, ACTION_DIM, &device);

        let states = Tensor::<B, 2>::from_data(
            Data::new(
                vec![0.1; BATCH_SIZE * STATE_DIM],
                Shape::new([BATCH_SIZE, STATE_DIM]),
            ),
            &device,
        );
        let (actor_out, critic_out) = model.forward(states);

        assert_eq!(actor_out.dims(), [BATCH_SIZE, ACTION_DIM]);
        assert_eq!(critic_out.dims(), [BATCH_SIZE, 1]);
    }

    #[test]
    fn the_loss_is_a_finite_scalar() {
        let device = burn_ndarray::NdArrayDevice::Cpu;
        let model = ActorCriticModel::<B>::new(STATE_DIM, HIDDEN_DIM, ACTION_DIM, &device);
        let states = Tensor::<B, 2>::from_data(
            Data::new(
                vec![0.1; BATCH_SIZE * STATE_DIM],
                Shape::new([BATCH_SIZE, STATE_DIM]),
            ),
            &device,
        );
        let actions = Tensor::<B, 2>::from_data(
            Data::new(
                vec![0.5; BATCH_SIZE * ACTION_DIM],
                Shape::new([BATCH_SIZE, ACTION_DIM]),
            ),
            &device,
        );
        let rewards = Tensor::<B, 2>::from_data(
            Data::new(vec![1.0; BATCH_SIZE], Shape::new([BATCH_SIZE, 1])),
            &device,
        );

        let loss =
            a2c_loss(&model, states.clone(), states, actions, rewards, DISCOUNT).into_scalar();

        assert!(loss.is_finite(), "loss must be finite, got {loss}");
    }

    #[test]
    fn learner_and_inference_start_from_the_same_seeded_policy() {
        let seed = 0x5EED_CAFE;
        let device = burn_ndarray::NdArrayDevice::Cpu;
        let learner = seeded_training_model::<B>(&device, seed).valid();
        let inference =
            crate::ai::model::BrainModel::new_seeded_cpu(STATE_DIM, HIDDEN_DIM, ACTION_DIM, seed);
        let inference_model = match inference.backend() {
            crate::ai::model::BrainModelBackend::NdArray(model, _) => model,
            #[cfg(feature = "ml-wgpu")]
            crate::ai::model::BrainModelBackend::Wgpu(..) => {
                panic!("the explicitly CPU-seeded inference model must use ndarray");
            }
        };
        let input = Tensor::<B, 2>::from_data(
            Data::new(vec![0.25; STATE_DIM], Shape::new([1, STATE_DIM])),
            &device,
        );

        let (learner_actor, learner_critic) = learner.forward(input.clone());
        let (inference_actor, inference_critic) = inference_model.forward(input);

        assert_eq!(
            learner_actor.into_data().value,
            inference_actor.into_data().value
        );
        assert_eq!(
            learner_critic.into_data().value,
            inference_critic.into_data().value
        );
    }
}
