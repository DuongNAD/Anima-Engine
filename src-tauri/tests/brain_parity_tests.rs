//! **EB-S02** — the hand-written per-agent forward pass must agree with the Burn model it replaces.
//!
//! ADR-0003 moves inference off Burn: each agent carries its own weights, so the problem becomes
//! "N models × 1 input" rather than "1 model × N inputs", which is a small matmul best done against
//! pre-allocated buffers. That is only safe if the hand-written arithmetic is provably the same
//! arithmetic. Numerical code fails quietly — a wrong layout still returns finite, plausible floats —
//! so this gate exists to make the failure loud.
//!
//! It also guards gate **EB-S04**: `brain_genotype = None` is supposed to reproduce today's
//! trajectory. That claim is only meaningful while the two implementations compute the same function.

use anima_engine_lib::ai::model::ActorCriticModel;
use anima_engine_lib::evolution::brain_genotype::{ArchSpec, BrainGenotype};
use burn::tensor::{Data, Shape, Tensor};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

type Backend = burn_ndarray::NdArray<f32>;

/// Both sides accumulate in `f32` but in different orders — Burn's matmul is blocked, the hand-written
/// pass walks fan-in linearly — so bit-equality is not achievable and would be the wrong thing to
/// assert.
///
/// Measured on 2026-07-25 across every architecture below, the worst observed disagreement was
/// `1.8e-7` on the actor outputs and `8.0e-7` on the critic value. The actor figure is exactly
/// `f32::EPSILON`, i.e. one unit in the last place: the two implementations agree to the limit of the
/// type. This bound therefore leaves 1–2 orders of magnitude of headroom over float noise while
/// staying far below any real defect — the `parity_is_sensitive_*` tests demonstrate that a single
/// wrong weight, or one transposed layer, blows straight past it.
const TOLERANCE: f32 = 1e-5;

fn max_abs_diff(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "compared vectors must have equal length");
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f32, f32::max)
}

/// Run one input row through Burn and return `(actions, value)`.
fn burn_forward(model: &ActorCriticModel<Backend>, inputs: &[f32]) -> (Vec<f32>, f32) {
    let device = burn_ndarray::NdArrayDevice::Cpu;
    let data = Data::new(inputs.to_vec(), Shape::new([1, inputs.len()]));
    let tensor = Tensor::<Backend, 2>::from_data(data, &device);
    let (actor, critic) = model.forward(tensor);
    (actor.into_data().value, critic.into_data().value[0])
}

fn model_for(genotype: &BrainGenotype) -> ActorCriticModel<Backend> {
    ActorCriticModel::<Backend>::from_flat_weights(
        genotype.arch.inputs,
        genotype.arch.hidden,
        genotype.arch.outputs,
        &genotype.weights,
        &burn_ndarray::NdArrayDevice::Cpu,
    )
    .expect("weights must load")
}

fn random_inputs(n: usize, rng: &mut impl Rng) -> Vec<f32> {
    (0..n).map(|_| rng.gen_range(-2.0..2.0)).collect()
}

/// Compare both implementations across several input rows; returns the worst deviation seen for the
/// actor outputs and for the critic value.
fn worst_deviation(arch: ArchSpec, seed: u64, rows: usize) -> (f32, f32) {
    let mut rng = StdRng::seed_from_u64(seed);
    let genotype = BrainGenotype::random(arch, &mut rng).expect("architecture must be valid");
    let model = model_for(&genotype);

    let mut worst_actions = 0.0f32;
    let mut worst_value = 0.0f32;
    for _ in 0..rows {
        let inputs = random_inputs(arch.inputs, &mut rng);
        let (mine_actions, mine_value) = genotype.forward(&inputs).expect("forward must succeed");
        let (burn_actions, burn_value) = burn_forward(&model, &inputs);

        worst_actions = worst_actions.max(max_abs_diff(&mine_actions, &burn_actions));
        worst_value = worst_value.max((mine_value - burn_value).abs());
    }
    (worst_actions, worst_value)
}

#[test]
fn parity_on_the_live_architecture() {
    // The architecture the shared BrainModel is constructed with today: BrainModel::new(15, 64, 4).
    let (actions, value) = worst_deviation(ArchSpec::LEGACY, 20260725, 32);
    assert!(
        actions <= TOLERANCE && value <= TOLERANCE,
        "actor diff {actions:e}, critic diff {value:e} exceed {TOLERANCE:e}"
    );
}

#[test]
fn parity_on_the_widened_action_space() {
    // ADR-0003 decision 4 adds four ecological gates alongside the four CPG parameters.
    let (actions, value) = worst_deviation(ArchSpec::new(15, 64, 8), 4242, 32);
    assert!(
        actions <= TOLERANCE && value <= TOLERANCE,
        "actor diff {actions:e}, critic diff {value:e} exceed {TOLERANCE:e}"
    );
}

#[test]
fn parity_holds_when_every_dimension_differs() {
    // The layout guard. With `inputs != hidden != outputs` a transposed weight matrix is still a
    // buffer of the correct length, so nothing fails structurally — only the numbers disagree. A
    // square-only test would let that bug through.
    for (i, h, o) in [(3, 7, 2), (11, 5, 9), (2, 3, 5), (17, 4, 1)] {
        let (actions, value) = worst_deviation(ArchSpec::new(i, h, o), 99, 16);
        assert!(
            actions <= TOLERANCE && value <= TOLERANCE,
            "{i}x{h}x{o}: actor diff {actions:e}, critic diff {value:e} exceed {TOLERANCE:e}"
        );
    }
}

#[test]
fn parity_survives_saturating_inputs() {
    // Large magnitudes drive ReLU fully on/off and push the sigmoid into its flat tails, where a
    // disagreement in activation order would show up but a mild-input test would not notice.
    let mut rng = StdRng::seed_from_u64(7);
    let genotype = BrainGenotype::random(ArchSpec::new(8, 16, 4), &mut rng).unwrap();
    let model = model_for(&genotype);

    for scale in [0.0f32, 1e-4, 50.0, -50.0, 1e4] {
        let inputs = vec![scale; genotype.arch.inputs];
        let (mine_actions, mine_value) = genotype.forward(&inputs).unwrap();
        let (burn_actions, burn_value) = burn_forward(&model, &inputs);

        assert!(
            mine_actions.iter().all(|a| a.is_finite()) && mine_value.is_finite(),
            "scale {scale}: hand-written pass produced a non-finite value"
        );
        assert!(
            max_abs_diff(&mine_actions, &burn_actions) <= TOLERANCE,
            "scale {scale}: actor diff {:e}",
            max_abs_diff(&mine_actions, &burn_actions)
        );
        // The critic head is linear, so its magnitude grows with the input; compare relatively once
        // the value is large enough that an absolute bound would be meaningless.
        let denom = mine_value.abs().max(burn_value.abs()).max(1.0);
        assert!(
            (mine_value - burn_value).abs() / denom <= TOLERANCE,
            "scale {scale}: critic relative diff too large ({mine_value} vs {burn_value})"
        );
    }
}

#[test]
fn parity_is_sensitive_to_a_single_wrong_weight() {
    // Demonstrates the gate has power: if the tolerance were loose enough to accept an unrelated
    // network, every assertion above would be decorative.
    let mut rng = StdRng::seed_from_u64(5);
    let arch = ArchSpec::new(6, 12, 3);
    let genotype = BrainGenotype::random(arch, &mut rng).unwrap();

    let mut tampered = genotype.clone();
    tampered.weights[0] += 1.0;
    let model = model_for(&tampered);

    let inputs = random_inputs(arch.inputs, &mut rng);
    let (mine_actions, _) = genotype.forward(&inputs).unwrap();
    let (burn_actions, _) = burn_forward(&model, &inputs);

    assert!(
        max_abs_diff(&mine_actions, &burn_actions) > TOLERANCE,
        "one perturbed weight must break parity, otherwise the gate proves nothing"
    );
}

#[test]
fn parity_is_sensitive_to_a_transposed_layer() {
    // The specific bug this gate exists for. Feeding Burn a pre-transposed trunk matrix cancels the
    // transpose `from_flat_weights` applies, reproducing exactly the mistake of copying a flat
    // vector across the two conventions. The result must be visibly wrong, not quietly close.
    let mut rng = StdRng::seed_from_u64(6);
    let arch = ArchSpec::new(5, 9, 3);
    let genotype = BrainGenotype::random(arch, &mut rng).unwrap();

    let (d_in, d_out) = (arch.inputs, arch.hidden);
    let mut scrambled = genotype.clone();
    for o in 0..d_out {
        for i in 0..d_in {
            // Read the genotype's `[out][in]` entry into the `[in][out]` slot, i.e. hand Burn the
            // layout it would receive if nobody transposed.
            scrambled.weights[i * d_out + o] = genotype.weights[o * d_in + i];
        }
    }
    let model = model_for(&scrambled);

    let inputs = random_inputs(arch.inputs, &mut rng);
    let (mine_actions, _) = genotype.forward(&inputs).unwrap();
    let (burn_actions, _) = burn_forward(&model, &inputs);

    assert!(
        max_abs_diff(&mine_actions, &burn_actions) > TOLERANCE,
        "a transposed trunk must break parity — otherwise the layout is untested"
    );
}

#[test]
fn rows_of_a_batch_are_independent() {
    // Burn runs the whole population in one batched matmul while the hand-written pass walks agents
    // one at a time. Those only stay interchangeable if a row's output depends on nothing but that
    // row, so batching cannot become a hidden source of cross-agent coupling.
    let mut rng = StdRng::seed_from_u64(31);
    let arch = ArchSpec::new(9, 13, 4);
    let genotype = BrainGenotype::random(arch, &mut rng).unwrap();
    let model = model_for(&genotype);

    let rows: Vec<Vec<f32>> = (0..5)
        .map(|_| random_inputs(arch.inputs, &mut rng))
        .collect();
    let flat: Vec<f32> = rows.iter().flatten().copied().collect();

    let device = burn_ndarray::NdArrayDevice::Cpu;
    let data = Data::new(flat, Shape::new([rows.len(), arch.inputs]));
    let (actor, _) = model.forward(Tensor::<Backend, 2>::from_data(data, &device));
    let batched = actor.into_data().value;

    for (r, row) in rows.iter().enumerate() {
        let (mine, _) = genotype.forward(row).unwrap();
        let slice = &batched[r * arch.outputs..(r + 1) * arch.outputs];
        assert!(
            max_abs_diff(&mine, slice) <= TOLERANCE,
            "row {r} of a batch diverged from the same row computed alone"
        );
    }
}

#[test]
fn a_mutated_genome_still_matches_burn() {
    // Weights that came out of the evolutionary operators, not just the initialiser — mutation and
    // crossover produce distributions the He/Xavier init never does.
    use anima_engine_lib::evolution::brain_genotype::{crossover_brains, mutate_brain};

    let mut rng = StdRng::seed_from_u64(77);
    let arch = ArchSpec::new(10, 24, 6);
    let mut a = BrainGenotype::random(arch, &mut rng).unwrap();
    let b = BrainGenotype::random(arch, &mut rng).unwrap();

    mutate_brain(&mut a, 0.5, 2.0, &mut rng).unwrap();
    let child = crossover_brains(&a, &b, &mut rng).unwrap();
    let model = model_for(&child);

    for _ in 0..16 {
        let inputs = random_inputs(arch.inputs, &mut rng);
        let (mine, mine_value) = child.forward(&inputs).unwrap();
        let (theirs, burn_value) = burn_forward(&model, &inputs);
        assert!(max_abs_diff(&mine, &theirs) <= TOLERANCE);
        assert!((mine_value - burn_value).abs() <= TOLERANCE);
    }
}
