//! The shared learner's A2C objective must move the policy toward actions that turned out well.
//!
//! `run_training_loop` computed `mean((a − â)²·(−td))` — the intended objective with its sign
//! inverted. A positive advantage made the loss *decrease* as `(a − â)²` grew, so gradient descent
//! drove the shared policy away from actions that beat expectation and toward ones that missed it.
//! The network ran, the numbers stayed finite, and the loss went down; nothing about it looked
//! wrong. ADR-0003 recorded the discrepancy and had `brain_genotype::learn_step` implement the
//! correct sign rather than copy the defect, leaving two objectives that disagreed.
//!
//! ADR-0003 also says why one test is not enough here: *"a gradient check against finite differences
//! catches a wrong derivative, but passes for a wrong objective too. Pair it with a behavioural
//! assertion."* An inverted sign is a wrong objective whose derivative is internally consistent — a
//! gradient check alone would have signed it off. So this file has both:
//!
//! - `loss_surface_*` and `numerical_gradient_*` probe the loss surface directly by rebuilding the
//!   model from perturbed flat weights, which is the finite-difference half.
//! - `training_moves_the_policy_*` runs the real optimiser and asserts where the policy ends up,
//!   which is the half that fails under an inverted objective.

use anima_engine_lib::ai::model::ActorCriticModel;
use anima_engine_lib::core::simulation_loop::{a2c_loss, DISCOUNT};
use burn::backend::Autodiff;
use burn::module::AutodiffModule;
use burn::optim::{AdamConfig, GradientsParams, Optimizer};
use burn::tensor::{Data, Shape, Tensor};

type Backend = burn_ndarray::NdArray<f32>;
type Ad = Autodiff<Backend>;

const INPUTS: usize = 3;
const HIDDEN: usize = 4;
const OUTPUTS: usize = 2;
const BATCH: usize = 2;

fn flat_len() -> usize {
    (INPUTS * HIDDEN + HIDDEN)
        + (HIDDEN * HIDDEN + HIDDEN)
        + (HIDDEN * OUTPUTS + OUTPUTS)
        + (HIDDEN + 1)
}

/// Index of the first actor-head bias in the flat layout `BrainGenotype` uses, which is the layout
/// `from_flat_weights` reads. Nudging a bias moves the actor output directly, with no interaction
/// through the trunk, which is what makes it a clean probe.
fn actor_bias_offset() -> usize {
    (INPUTS * HIDDEN + HIDDEN) + (HIDDEN * HIDDEN + HIDDEN) + (HIDDEN * OUTPUTS)
}

/// Deterministic, non-degenerate weights: all-positive so the ReLU trunk passes signal through.
fn base_weights() -> Vec<f32> {
    (0..flat_len())
        .map(|k| 0.05 + 0.01 * ((k % 7) as f32))
        .collect()
}

fn tensor<const D: usize>(data: Vec<f32>, shape: [usize; D]) -> Tensor<Ad, D> {
    Tensor::<Ad, D>::from_data(Data::new(data, Shape::new(shape)), &Default::default())
}

fn states() -> Tensor<Ad, 2> {
    tensor(vec![0.5, 0.25, 0.75, 0.4, 0.6, 0.2], [BATCH, INPUTS])
}

fn next_states() -> Tensor<Ad, 2> {
    tensor(vec![0.5, 0.25, 0.75, 0.4, 0.6, 0.2], [BATCH, INPUTS])
}

fn actions() -> Tensor<Ad, 2> {
    // Both well away from the sigmoid's 0.5 resting point, so "toward" and "away" are measurable.
    tensor(vec![1.0, 0.0, 1.0, 0.0], [BATCH, OUTPUTS])
}

fn rewards(value: f32) -> Tensor<Ad, 2> {
    tensor(vec![value; BATCH], [BATCH, 1])
}

fn model_from(weights: &[f32]) -> ActorCriticModel<Ad> {
    ActorCriticModel::<Ad>::from_flat_weights(INPUTS, HIDDEN, OUTPUTS, weights, &Default::default())
        .expect("weights match the declared architecture")
}

fn loss_of(weights: &[f32], reward: f32) -> f32 {
    let model = model_from(weights);
    a2c_loss(
        &model,
        states(),
        next_states(),
        actions(),
        rewards(reward),
        DISCOUNT,
    )
    .into_scalar()
}

/// Mean actor output for output 0 across the batch. The recorded action for that output is 1.0, so
/// larger means closer to the action taken.
fn actor_output_0(model: &ActorCriticModel<Backend>) -> f32 {
    let device = Default::default();
    let s = Tensor::<Backend, 2>::from_data(
        Data::new(
            vec![0.5, 0.25, 0.75, 0.4, 0.6, 0.2],
            Shape::new([BATCH, INPUTS]),
        ),
        &device,
    );
    let (actor_out, _) = model.forward(s);
    let values: Vec<f32> = actor_out.into_data().convert::<f32>().value;
    // Column 0 of each row.
    (values[0] + values[OUTPUTS]) / 2.0
}

/// A reward large enough that `td = r + γ·V(s') − V(s)` is comfortably positive for these weights.
const REWARDING: f32 = 10.0;
/// And one large and negative enough for the opposite.
const PUNISHING: f32 = -10.0;

#[test]
fn advantage_signs_are_what_the_tests_below_assume() {
    // Guards the premise rather than the behaviour: if the fixture ever stopped producing a
    // positive advantage for REWARDING, every directional assertion below would still pass while
    // testing nothing. Reconstructs td from the critic the same way `a2c_loss` does.
    let weights = base_weights();
    let model = model_from(&weights);
    let (_, critic) = model.forward(states());
    let (_, critic_next) = model.forward(next_states());
    let v: Vec<f32> = critic.into_data().convert::<f32>().value;
    let v_next: Vec<f32> = critic_next.into_data().convert::<f32>().value;

    let td_rewarding = REWARDING + DISCOUNT * v_next[0] - v[0];
    let td_punishing = PUNISHING + DISCOUNT * v_next[0] - v[0];
    assert!(
        td_rewarding > 0.0,
        "expected positive advantage, got {td_rewarding}"
    );
    assert!(
        td_punishing < 0.0,
        "expected negative advantage, got {td_punishing}"
    );
}

// --- finite differences on the loss surface -------------------------------------------------

#[test]
fn loss_surface_falls_when_a_rewarded_action_is_approached() {
    // The defining property of the objective: with a positive advantage, being closer to the action
    // that was taken must cost less. Under the inverted sign this assertion reverses exactly.
    let mut nearer = base_weights();
    // Output 0's recorded action is 1.0; raising its bias raises the sigmoid toward 1.0.
    nearer[actor_bias_offset()] += 0.5;

    let base = loss_of(&base_weights(), REWARDING);
    let closer = loss_of(&nearer, REWARDING);

    assert!(
        closer < base,
        "moving toward a rewarded action should lower the loss, but {closer} >= {base}"
    );
}

#[test]
fn loss_surface_rises_when_a_punished_action_is_approached() {
    let mut nearer = base_weights();
    nearer[actor_bias_offset()] += 0.5;

    let base = loss_of(&base_weights(), PUNISHING);
    let closer = loss_of(&nearer, PUNISHING);

    assert!(
        closer > base,
        "moving toward a punished action should raise the loss, but {closer} <= {base}"
    );
}

#[test]
fn numerical_gradient_of_the_actor_bias_has_the_sign_the_objective_implies() {
    // A central-difference derivative, which is the gradient-check half of the ADR-0003 pairing.
    //
    // For output n the actor term is `mean((aₙ − âₙ)²·td)`, so
    //   ∂L/∂bₙ = 2·(aₙ − âₙ)·td·σ'(zₙ)/N.
    // σ' > 0 always, and â₀ = 1 with a sigmoid output in (0,1) makes (a₀ − â₀) < 0. So with td > 0
    // the derivative is negative — increasing the bias lowers the loss — and with td < 0 it is
    // positive. The inverted objective produces both with the opposite sign.
    let eps = 1e-3;
    let offset = actor_bias_offset();

    for (reward, want_negative) in [(REWARDING, true), (PUNISHING, false)] {
        let mut up = base_weights();
        let mut down = base_weights();
        up[offset] += eps;
        down[offset] -= eps;

        let derivative = (loss_of(&up, reward) - loss_of(&down, reward)) / (2.0 * eps);

        assert!(
            derivative.is_finite(),
            "derivative must be finite, got {derivative}"
        );
        assert!(
            derivative.abs() > 1e-6,
            "derivative should be measurably non-zero, got {derivative}"
        );
        if want_negative {
            assert!(
                derivative < 0.0,
                "with a positive advantage, raising the bias toward the taken action must lower \
                 the loss (∂L/∂b < 0), got {derivative}"
            );
        } else {
            assert!(
                derivative > 0.0,
                "with a negative advantage, raising the bias toward the taken action must raise \
                 the loss (∂L/∂b > 0), got {derivative}"
            );
        }
    }
}

// --- behavioural: run the optimiser and see where the policy ends up ------------------------

fn train(reward: f32, steps: usize) -> (f32, f32) {
    let mut model = model_from(&base_weights());
    let before = actor_output_0(&model.valid());

    let mut optim = AdamConfig::new().init();
    for _ in 0..steps {
        let loss = a2c_loss(
            &model,
            states(),
            next_states(),
            actions(),
            rewards(reward),
            DISCOUNT,
        );
        let grads = GradientsParams::from_grads(loss.backward(), &model);
        model = optim.step(1e-2, model, grads);
    }

    let after = actor_output_0(&model.valid());
    (before, after)
}

#[test]
fn training_moves_the_policy_toward_a_rewarded_action() {
    // This is the assertion the inverted sign fails. The recorded action for output 0 is 1.0, so a
    // learner that credits a positive advantage correctly must raise that output.
    let (before, after) = train(REWARDING, 40);

    assert!(
        after > before,
        "a rewarded action should be reinforced: output went {before} -> {after}"
    );
}

#[test]
fn training_moves_the_policy_away_from_a_punished_action() {
    let (before, after) = train(PUNISHING, 40);

    assert!(
        after < before,
        "a punished action should be discouraged: output went {before} -> {after}"
    );
}

#[test]
fn the_shared_objective_agrees_with_the_per_agent_learner() {
    // ADR-0003's point of contention: `learn_step` and this loop implement the same objective, and
    // for a while they disagreed on its sign. Asserting only that `learn_step` reinforces a reward
    // would pass whatever the shared model does — this has to compare the two, so it trains both on
    // the same rewarded action and requires them to move the same way.
    use anima_engine_lib::evolution::brain_genotype::{
        learn_step, ArchSpec, BrainGenotype, LearnScratch,
    };

    // Same flat layout `from_flat_weights` reads above, so both learners start from the same
    // numbers as well as the same architecture.
    let arch = ArchSpec::new(INPUTS, HIDDEN, OUTPUTS);
    let mut genotype =
        BrainGenotype::from_weights(arch, base_weights()).expect("weights match the arch");
    let state = [0.5f32, 0.25, 0.75];
    let action_taken = [1.0f32, 0.0];

    let before = genotype.forward(&state).expect("forward succeeds").0[0];

    let mut scratch = LearnScratch::default();
    for _ in 0..40 {
        learn_step(
            &mut genotype,
            &state,
            &action_taken,
            REWARDING,
            0.0,
            DISCOUNT,
            1e-2,
            &mut scratch,
        )
        .expect("learn_step succeeds");
    }

    let after = genotype.forward(&state).expect("forward succeeds").0[0];
    let per_agent_delta = after - before;

    // The shared learner, on the same rewarded action.
    let (shared_before, shared_after) = train(REWARDING, 40);
    let shared_delta = shared_after - shared_before;

    assert!(
        per_agent_delta > 0.0,
        "the per-agent learner must reinforce a rewarded action: {before} -> {after}"
    );
    assert!(
        shared_delta > 0.0,
        "the shared learner must reinforce a rewarded action: {shared_before} -> {shared_after}"
    );
    assert_eq!(
        per_agent_delta > 0.0,
        shared_delta > 0.0,
        "the two learners disagree on the direction of a rewarded action: per-agent moved \
         {per_agent_delta}, shared moved {shared_delta}"
    );
}
