//! The shared-model learner: its objective, its loop, and the two backend-specific threads.
//!
//! Split out of `simulation_loop.rs`, which had grown to 1,754 lines with no `#[cfg(test)]` block at
//! all. Nothing here touches `SimulationEngine`'s internals — the learner communicates entirely over
//! crossbeam channels — so it is self-contained in a way the rest of that file is not.
//!
//! The part worth reading before changing anything is the sign discussion on [`a2c_loss`].

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use bevy_ecs::prelude::Resource;
use burn::backend::Autodiff;
use burn::module::{AutodiffModule, Module};
use burn::optim::{AdamConfig, GradientsParams, Optimizer};
use burn::record::{BinBytesRecorder, FullPrecisionSettings, Recorder};
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
/// Maximum transitions waiting between the real-time simulation and the learner.
pub const TRANSITION_QUEUE_CAPACITY: usize = 4_096;
pub const SHARED_MODEL_PARAMETER_COUNT: usize = (STATE_DIM * HIDDEN_DIM + HIDDEN_DIM)
    + (HIDDEN_DIM * HIDDEN_DIM + HIDDEN_DIM)
    + (HIDDEN_DIM * ACTION_DIM + ACTION_DIM)
    + (HIDDEN_DIM + 1);
/// At most one trained policy may wait for inference.
///
/// Model snapshots are replaceable intermediate results, not an event log. A deeper queue makes
/// inference replay stale policies and lets a slow consumer stop the learner. The producer uses
/// [`try_send_without_blocking`], and the learner waits before taking another batch while this slot
/// is occupied, so every completed optimiser step normally reaches inference.
pub(crate) const MODEL_UPDATE_QUEUE_CAPACITY: usize = 1;
/// Adam step size for the shared model.
const LEARNING_RATE: f64 = 1e-3;
/// Discount applied to the bootstrapped next-state value in the TD target.
pub const DISCOUNT: f32 = 0.99;

pub enum ModelUpdate {
    NdArray(ActorCriticModel<burn_ndarray::NdArray<f32>>),
    #[cfg(feature = "ml-wgpu")]
    Wgpu(ActorCriticModel<burn_wgpu::Wgpu<burn_wgpu::AutoGraphicsApi, f32, i32>>),
}

impl ModelUpdate {
    pub fn checkpoint_weights(&self) -> Result<Vec<f32>, String> {
        match self {
            Self::NdArray(model) => model.to_flat_weights(STATE_DIM, HIDDEN_DIM, ACTION_DIM),
            #[cfg(feature = "ml-wgpu")]
            Self::Wgpu(model) => model.to_flat_weights(STATE_DIM, HIDDEN_DIM, ACTION_DIM),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SavedLearnerWorkerState {
    pub training_model_record: Vec<u8>,
    pub optimizer_record: Vec<u8>,
    pub partial_batch: Vec<Transition>,
}

pub struct LearnerCheckpointRequest {
    pub reply: crossbeam_channel::Sender<Result<SavedLearnerWorkerState, String>>,
    pub resume: crossbeam_channel::Receiver<()>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModelUpdateDelivery {
    Published,
    Backpressured,
    Disconnected,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModelUpdateSnapshot {
    pub published: u64,
    pub backpressured: u64,
    pub disconnected: u64,
}

#[derive(Default)]
struct ModelUpdateCounters {
    published: AtomicU64,
    backpressured: AtomicU64,
    disconnected: AtomicU64,
}

/// Monotonic accounting for trained-policy delivery to the inference worker.
///
/// Publishing is nonblocking because a policy snapshot must never strand either worker. The learner
/// normally paces itself before training while one snapshot is pending; these counters make any
/// raced backpressure or disconnect visible and disclose how many optimiser steps reached inference.
#[derive(Resource, Clone, Default)]
pub struct ModelUpdateDiagnostics(Arc<ModelUpdateCounters>);

impl ModelUpdateDiagnostics {
    pub fn from_snapshot(snapshot: ModelUpdateSnapshot) -> Self {
        Self(Arc::new(ModelUpdateCounters {
            published: AtomicU64::new(snapshot.published),
            backpressured: AtomicU64::new(snapshot.backpressured),
            disconnected: AtomicU64::new(snapshot.disconnected),
        }))
    }

    fn increment(counter: &AtomicU64) {
        let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            Some(value.saturating_add(1))
        });
    }

    pub(crate) fn record(&self, outcome: ModelUpdateDelivery) {
        match outcome {
            ModelUpdateDelivery::Published => Self::increment(&self.0.published),
            ModelUpdateDelivery::Backpressured => Self::increment(&self.0.backpressured),
            ModelUpdateDelivery::Disconnected => Self::increment(&self.0.disconnected),
        }
    }

    pub fn snapshot(&self) -> ModelUpdateSnapshot {
        ModelUpdateSnapshot {
            published: self.0.published.load(Ordering::Relaxed),
            backpressured: self.0.backpressured.load(Ordering::Relaxed),
            disconnected: self.0.disconnected.load(Ordering::Relaxed),
        }
    }
}

/// Deliver a replaceable model snapshot without ever waiting for the consumer.
///
/// The returned value is intentionally explicit so callers choose how to react to backpressure and
/// can stop once the consumer has disconnected. The same primitive returns retired models to the
/// learner for off-path destruction; when that recycle queue is full, dropping on the inference
/// thread is still safer than blocking the simulation's response path.
pub(crate) fn try_send_without_blocking<T>(
    sender: &crossbeam_channel::Sender<T>,
    value: T,
) -> ModelUpdateDelivery {
    match sender.try_send(value) {
        Ok(()) => ModelUpdateDelivery::Published,
        Err(crossbeam_channel::TrySendError::Full(_)) => ModelUpdateDelivery::Backpressured,
        Err(crossbeam_channel::TrySendError::Disconnected(_)) => ModelUpdateDelivery::Disconnected,
    }
}

/// The handles a learner thread needs: liveness, transition/model channels, delivery accounting and
/// the run seed.
pub type LearnArgs = (
    Arc<AtomicBool>,
    crossbeam_channel::Receiver<Transition>,
    crossbeam_channel::Sender<ModelUpdate>,
    crossbeam_channel::Receiver<ModelUpdate>,
    ModelUpdateDiagnostics,
    u64,
    Option<SavedLearnerWorkerState>,
    crossbeam_channel::Receiver<LearnerCheckpointRequest>,
);

/// CPU learner. Always available — it is the fallback both when the GPU probe fails and when the
/// `ml-wgpu` feature is off entirely.
pub fn spawn_ndarray_learner(
    args: LearnArgs,
    exit: crate::core::thread_supervisor::ExitToken,
) -> thread::JoinHandle<()> {
    let (running, trans_rx, model_tx, old_model_rx, model_diagnostics, seed, resume, checkpoint_rx) =
        args;
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
            model_diagnostics,
            device,
            seed,
            resume,
            checkpoint_rx,
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
    let (running, trans_rx, model_tx, old_model_rx, model_diagnostics, seed, resume, checkpoint_rx) =
        args;
    thread::spawn(move || {
        let _exit = exit;
        let device = burn_wgpu::WgpuDevice::default();
        run_training_loop::<burn_wgpu::Wgpu<burn_wgpu::AutoGraphicsApi, f32, i32>>(
            running,
            trans_rx,
            model_tx,
            old_model_rx,
            model_diagnostics,
            device,
            seed,
            resume,
            checkpoint_rx,
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

/// Decode opaque Burn records before any live worker starts.
///
/// Burn 0.13's in-memory recorder panics on malformed bincode in a few paths, so local snapshot
/// bytes are still treated as untrusted and the decoder is contained behind `catch_unwind`.
pub fn validate_saved_learner_worker(
    saved: &SavedLearnerWorkerState,
    seed: u64,
) -> Result<(), String> {
    if saved.partial_batch.len() >= BATCH_SIZE
        || saved
            .partial_batch
            .iter()
            .any(|transition| !transition.is_finite())
    {
        return Err("learner partial batch is invalid".into());
    }

    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        type B = burn_ndarray::NdArray<f32>;
        type AB = Autodiff<B>;
        let device = burn_ndarray::NdArrayDevice::Cpu;
        let recorder = BinBytesRecorder::<FullPrecisionSettings>::default();

        let model_record =
            Recorder::<AB>::load(&recorder, saved.training_model_record.clone(), &device)
                .map_err(|error| format!("learner model record is unreadable: {error}"))?;
        let _model = seeded_training_model::<B>(&device, seed).load_record(model_record);

        let optimizer_record =
            Recorder::<AB>::load(&recorder, saved.optimizer_record.clone(), &device)
                .map_err(|error| format!("Adam record is unreadable: {error}"))?;
        let _optimizer = AdamConfig::new()
            .init::<AB, ActorCriticModel<AB>>()
            .load_record(optimizer_record);
        Ok::<(), String>(())
    }))
    .map_err(|_| "learner checkpoint decoder panicked on malformed bytes".to_owned())?
}

fn run_training_loop<B>(
    running: Arc<AtomicBool>,
    trans_rx: crossbeam_channel::Receiver<Transition>,
    model_tx: crossbeam_channel::Sender<ModelUpdate>,
    old_model_rx: crossbeam_channel::Receiver<ModelUpdate>,
    model_diagnostics: ModelUpdateDiagnostics,
    device: B::Device,
    seed: u64,
    resume: Option<SavedLearnerWorkerState>,
    checkpoint_rx: crossbeam_channel::Receiver<LearnerCheckpointRequest>,
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
    if let Some(saved) = resume {
        let recorder = BinBytesRecorder::<FullPrecisionSettings>::default();
        let model_record =
            match Recorder::<Autodiff<B>>::load(&recorder, saved.training_model_record, &device) {
                Ok(record) => record,
                Err(error) => {
                    eprintln!("learner checkpoint model is unreadable: {error}");
                    running.store(false, Ordering::SeqCst);
                    return;
                }
            };
        train_model = train_model.load_record(model_record);
        let optimizer_record =
            match Recorder::<Autodiff<B>>::load(&recorder, saved.optimizer_record, &device) {
                Ok(record) => record,
                Err(error) => {
                    eprintln!("learner checkpoint optimizer is unreadable: {error}");
                    running.store(false, Ordering::SeqCst);
                    return;
                }
            };
        optim = optim.load_record(optimizer_record);
        batch = saved.partial_batch;
    }

    while running.load(Ordering::SeqCst) {
        if let Ok(request) = checkpoint_rx.try_recv() {
            let recorder = BinBytesRecorder::<FullPrecisionSettings>::default();
            let checkpoint = (|| {
                let training_model_record = Recorder::<Autodiff<B>>::record(
                    &recorder,
                    train_model.clone().into_record(),
                    (),
                )
                .map_err(|error| format!("cannot record learner model: {error}"))?;
                let optimizer_record =
                    Recorder::<Autodiff<B>>::record(&recorder, optim.to_record(), ())
                        .map_err(|error| format!("cannot record Adam state: {error}"))?;
                Ok(SavedLearnerWorkerState {
                    training_model_record,
                    optimizer_record,
                    partial_batch: batch.clone(),
                })
            })();
            if request.reply.send(checkpoint).is_ok() {
                loop {
                    if !running.load(Ordering::SeqCst) {
                        break;
                    }
                    match request.resume.recv_timeout(Duration::from_millis(10)) {
                        Ok(()) | Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                    }
                }
            }
            continue;
        }

        while let Ok(old_model) = old_model_rx.try_recv() {
            drop(old_model);
        }

        // An optimiser step is useful only if inference can eventually observe its policy. The
        // previous blocking `send` accidentally enforced that pacing after a 32-model backlog, but
        // could strand the learner forever. Waiting here keeps a single published policy in flight
        // without consuming more transitions or burning CPU on snapshots that would be discarded.
        // The sleep is off the tick path and bounds shutdown latency to one millisecond.
        if model_tx.is_full() {
            thread::sleep(Duration::from_millis(1));
            continue;
        }

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
                    let delivery =
                        try_send_without_blocking(&model_tx, to_model_update(eval_model));
                    model_diagnostics.record(delivery);
                    if delivery == ModelUpdateDelivery::Disconnected {
                        break;
                    }
                    batch.clear();
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use burn::module::Module;
    use burn::record::{BinBytesRecorder, FullPrecisionSettings, Recorder};

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
    fn learner_model_and_adam_resume_from_portable_records() {
        type AB = Autodiff<B>;

        fn one_step(
            mut model: ActorCriticModel<AB>,
            mut optim: impl Optimizer<ActorCriticModel<AB>, AB>,
            device: &burn_ndarray::NdArrayDevice,
        ) -> (
            ActorCriticModel<AB>,
            impl Optimizer<ActorCriticModel<AB>, AB>,
        ) {
            let states = Tensor::<AB, 2>::from_data(
                Data::new(
                    (0..BATCH_SIZE * STATE_DIM)
                        .map(|index| index as f32 * 0.001)
                        .collect(),
                    Shape::new([BATCH_SIZE, STATE_DIM]),
                ),
                device,
            );
            let next_states = states.clone() + 0.01;
            let actions = Tensor::<AB, 2>::from_data(
                Data::new(
                    vec![0.25; BATCH_SIZE * ACTION_DIM],
                    Shape::new([BATCH_SIZE, ACTION_DIM]),
                ),
                device,
            );
            let rewards = Tensor::<AB, 2>::from_data(
                Data::new(vec![0.5; BATCH_SIZE], Shape::new([BATCH_SIZE, 1])),
                device,
            );
            let loss = a2c_loss(&model, states, next_states, actions, rewards, DISCOUNT);
            let grads = loss.backward();
            let grads = GradientsParams::from_grads(grads, &model);
            model = optim.step(LEARNING_RATE, model, grads);
            (model, optim)
        }

        let device = burn_ndarray::NdArrayDevice::Cpu;
        let model = seeded_training_model::<B>(&device, 0x5EED);
        let optim = AdamConfig::new().init();
        let (model, optim) = one_step(model, optim, &device);

        let recorder = BinBytesRecorder::<FullPrecisionSettings>::default();
        let model_bytes = Recorder::<AB>::record(&recorder, model.clone().into_record(), ())
            .expect("training model must record");
        let optimizer_bytes = Recorder::<AB>::record(&recorder, optim.to_record(), ())
            .expect("Adam state must record");

        let model_record = Recorder::<AB>::load(&recorder, model_bytes, &device)
            .expect("training model must load");
        let restored_model = seeded_training_model::<B>(&device, 0x5EED).load_record(model_record);
        let optimizer_record = Recorder::<AB>::load(&recorder, optimizer_bytes, &device)
            .expect("Adam state must load");
        let restored_optim = AdamConfig::new().init().load_record(optimizer_record);

        let (continued_model, _) = one_step(model, optim, &device);
        let (restored_model, _) = one_step(restored_model, restored_optim, &device);
        let continued = continued_model
            .valid()
            .to_flat_weights(STATE_DIM, HIDDEN_DIM, ACTION_DIM)
            .unwrap();
        let restored = restored_model
            .valid()
            .to_flat_weights(STATE_DIM, HIDDEN_DIM, ACTION_DIM)
            .unwrap();
        assert_eq!(
            continued
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            restored
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            "model + Adam records must produce the same next optimizer step"
        );
    }

    #[test]
    fn malformed_learner_records_are_refused_without_escaping_a_panic() {
        let saved = SavedLearnerWorkerState {
            training_model_record: vec![0xFF, 0x00, 0xAA],
            optimizer_record: vec![0x13, 0x37],
            partial_batch: Vec::new(),
        };
        assert!(validate_saved_learner_worker(&saved, 7).is_err());
    }

    #[test]
    fn learner_checkpoint_pauses_with_its_partial_batch_and_resumes() {
        let running = Arc::new(AtomicBool::new(true));
        let (trans_tx, trans_rx) =
            crossbeam_channel::bounded::<Transition>(TRANSITION_QUEUE_CAPACITY);
        let trans_rx_observer = trans_rx.clone();
        let (model_tx, _model_rx) =
            crossbeam_channel::bounded::<ModelUpdate>(MODEL_UPDATE_QUEUE_CAPACITY);
        let (_old_model_tx, old_model_rx) = crossbeam_channel::bounded::<ModelUpdate>(1);
        let (checkpoint_tx, checkpoint_rx) =
            crossbeam_channel::bounded::<LearnerCheckpointRequest>(1);
        let running_worker = Arc::clone(&running);
        let worker = thread::spawn(move || {
            run_training_loop::<B>(
                running_worker,
                trans_rx,
                model_tx,
                old_model_rx,
                ModelUpdateDiagnostics::default(),
                burn_ndarray::NdArrayDevice::Cpu,
                0xC0FFEE,
                None,
                checkpoint_rx,
                ModelUpdate::NdArray,
            );
        });

        let transition = Transition {
            state: [0.1; STATE_DIM],
            action: [0.2; ACTION_DIM],
            reward: 0.3,
            next_state: [0.4; STATE_DIM],
        };
        trans_tx.send(transition).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !trans_rx_observer.is_empty() && std::time::Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(
            trans_rx_observer.is_empty(),
            "learner did not consume the transition"
        );

        let (reply_tx, reply_rx) = crossbeam_channel::bounded(1);
        let (resume_tx, resume_rx) = crossbeam_channel::bounded(1);
        checkpoint_tx
            .send(LearnerCheckpointRequest {
                reply: reply_tx,
                resume: resume_rx,
            })
            .unwrap();
        let saved = reply_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("learner checkpoint reply")
            .expect("learner checkpoint record");
        assert_eq!(saved.partial_batch, vec![transition]);
        validate_saved_learner_worker(&saved, 0xC0FFEE).expect("checkpoint records validate");

        resume_tx.send(()).unwrap();
        running.store(false, Ordering::SeqCst);
        worker.join().expect("learner exits after resume");
    }

    #[test]
    fn model_update_diagnostics_resume_monotonically() {
        let diagnostics = ModelUpdateDiagnostics::from_snapshot(ModelUpdateSnapshot {
            published: 4,
            backpressured: 3,
            disconnected: 2,
        });
        diagnostics.record(ModelUpdateDelivery::Published);
        diagnostics.record(ModelUpdateDelivery::Backpressured);
        diagnostics.record(ModelUpdateDelivery::Disconnected);
        assert_eq!(
            diagnostics.snapshot(),
            ModelUpdateSnapshot {
                published: 5,
                backpressured: 4,
                disconnected: 3,
            }
        );
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

    #[test]
    fn a_full_model_mailbox_never_blocks_its_producer() {
        let (model_tx, model_rx) = crossbeam_channel::bounded(MODEL_UPDATE_QUEUE_CAPACITY);
        model_tx.send(1_u8).expect("seed the one-slot mailbox");

        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let outcome = try_send_without_blocking(&model_tx, 2_u8);
            done_tx.send(outcome).expect("report delivery outcome");
        });

        assert_eq!(
            done_rx.recv_timeout(Duration::from_secs(1)),
            Ok(ModelUpdateDelivery::Backpressured),
            "a slow inference worker must not stall the learner"
        );
        assert_eq!(
            model_rx.recv().expect("the pending model is retained"),
            1,
            "a bounded mailbox keeps its already-published model until inference consumes it"
        );
        worker.join().expect("delivery worker must exit");
    }

    #[test]
    fn a_disconnected_model_mailbox_is_reported_without_blocking() {
        let (model_tx, model_rx) = crossbeam_channel::bounded::<u8>(1);
        drop(model_rx);

        assert_eq!(
            try_send_without_blocking(&model_tx, 1),
            ModelUpdateDelivery::Disconnected
        );
    }

    #[test]
    fn model_delivery_diagnostics_make_policy_backpressure_observable() {
        let diagnostics = ModelUpdateDiagnostics::default();
        diagnostics.record(ModelUpdateDelivery::Published);
        diagnostics.record(ModelUpdateDelivery::Backpressured);
        diagnostics.record(ModelUpdateDelivery::Backpressured);
        diagnostics.record(ModelUpdateDelivery::Disconnected);

        assert_eq!(
            diagnostics.snapshot(),
            ModelUpdateSnapshot {
                published: 1,
                backpressured: 2,
                disconnected: 1,
            }
        );
    }
}
