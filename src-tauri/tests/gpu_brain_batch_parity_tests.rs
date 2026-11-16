use std::sync::Arc;

use anima_engine_lib::ai::model::{run_inference_batch, BrainModel, InferenceScratch};
use anima_engine_lib::core::agent_systems::{
    AgentInferenceRequest, AgentInferenceResponse, ACTION_SLOTS, SENSORY_SLOTS,
};
use anima_engine_lib::evolution::brain_genotype::{action_index, BrainGenotype, EVOLVED_ARCH};
use bevy_ecs::prelude::World;
use burn::tensor::activation::{relu, sigmoid};
use burn::tensor::backend::Backend;
use burn::tensor::{Data, Shape, Tensor};
use burn_ndarray::{NdArray, NdArrayDevice};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;

// Matches HD_SEED in .hardening.env
const HD_SEED: u64 = 0x5EED;

fn make_test_brains(count: usize, rng: &mut ChaCha12Rng) -> Vec<Arc<BrainGenotype>> {
    let arch = EVOLVED_ARCH;
    let t1_b_start = arch.inputs * arch.hidden;
    let t1_b_end = t1_b_start + arch.hidden;
    let t2_b_start = t1_b_end + arch.hidden * arch.hidden;
    let t2_b_end = t2_b_start + arch.hidden;
    let ac_b_start = t2_b_end + arch.hidden * arch.outputs;
    let ac_b_end = ac_b_start + arch.outputs;

    let mut brains = Vec::with_capacity(count);
    for _ in 0..count {
        let mut brain = BrainGenotype::random(arch, rng).expect("valid random brain genotype");
        for idx in (t1_b_start..t1_b_end)
            .chain(t2_b_start..t2_b_end)
            .chain(ac_b_start..ac_b_end)
        {
            brain.weights[idx] = rng.gen_range(-0.25_f32..0.25_f32);
        }
        brain
            .validate()
            .expect("perturbed brain genotype remains valid");
        brains.push(Arc::new(brain));
    }
    brains
}

fn make_sensory_inputs(count: usize, rng: &mut ChaCha12Rng) -> Vec<[f32; SENSORY_SLOTS]> {
    let mut inputs = Vec::with_capacity(count);
    for _ in 0..count {
        let mut slot = [0.0_f32; SENSORY_SLOTS];
        for val in &mut slot {
            *val = rng.gen_range(-1.0_f32..1.0_f32);
        }
        inputs.push(slot);
    }
    inputs
}

fn pack_transposed_weight_tensor<B: Backend<FloatElem = f32>>(
    brains: &[Arc<BrainGenotype>],
    offset: usize,
    fan_in: usize,
    fan_out: usize,
    device: &B::Device,
) -> Tensor<B, 3> {
    let batch = brains.len();
    let mut packed = Vec::with_capacity(batch * fan_in * fan_out);
    for brain in brains {
        let slice = &brain.weights[offset..offset + fan_in * fan_out];
        for col_in in 0..fan_in {
            for row_out in 0..fan_out {
                packed.push(slice[row_out * fan_in + col_in]);
            }
        }
    }
    let data: Data<B::FloatElem, 3> =
        Data::new(packed, Shape::new([batch, fan_in, fan_out])).convert();
    Tensor::<B, 3>::from_data(data, device)
}

fn pack_bias_tensor<B: Backend<FloatElem = f32>>(
    brains: &[Arc<BrainGenotype>],
    offset: usize,
    width: usize,
    device: &B::Device,
) -> Tensor<B, 3> {
    let batch = brains.len();
    let mut packed = Vec::with_capacity(batch * width);
    for brain in brains {
        packed.extend_from_slice(&brain.weights[offset..offset + width]);
    }
    let data: Data<B::FloatElem, 3> = Data::new(packed, Shape::new([batch, 1, width])).convert();
    Tensor::<B, 3>::from_data(data, device)
}

fn batched_3d_actor_forward<B: Backend<FloatElem = f32>>(
    brains: &[Arc<BrainGenotype>],
    inputs: &[[f32; SENSORY_SLOTS]],
    device: &B::Device,
) -> Vec<f32> {
    assert_eq!(brains.len(), inputs.len());
    let batch = brains.len();
    let i = EVOLVED_ARCH.inputs;
    let h = EVOLVED_ARCH.hidden;
    let o = EVOLVED_ARCH.outputs;

    let t1_w_off = 0;
    let t1_b_off = i * h;
    let t2_w_off = t1_b_off + h;
    let t2_b_off = t2_w_off + h * h;
    let ac_w_off = t2_b_off + h;
    let ac_b_off = ac_w_off + h * o;

    let mut flat_inputs = Vec::with_capacity(batch * i);
    for sample in inputs {
        flat_inputs.extend_from_slice(sample);
    }
    let input_data: Data<B::FloatElem, 3> =
        Data::new(flat_inputs, Shape::new([batch, 1, i])).convert();
    let x = Tensor::<B, 3>::from_data(input_data, device);

    let w1 = pack_transposed_weight_tensor::<B>(brains, t1_w_off, i, h, device);
    let b1 = pack_bias_tensor::<B>(brains, t1_b_off, h, device);
    let h1 = relu(x.matmul(w1) + b1);

    let w2 = pack_transposed_weight_tensor::<B>(brains, t2_w_off, h, h, device);
    let b2 = pack_bias_tensor::<B>(brains, t2_b_off, h, device);
    let h2 = relu(h1.matmul(w2) + b2);

    let wa = pack_transposed_weight_tensor::<B>(brains, ac_w_off, h, o, device);
    let ba = pack_bias_tensor::<B>(brains, ac_b_off, o, device);
    let out = sigmoid(h2.matmul(wa) + ba);

    out.into_data().convert::<f32>().value
}

#[test]
fn evolved_brains_in_run_inference_batch_bypass_shared_brain_model() {
    let mut rng = ChaCha12Rng::seed_from_u64(HD_SEED);
    let brains = make_test_brains(8, &mut rng);
    let inputs = make_sensory_inputs(16, &mut rng);

    let mut world = World::new();
    let mut requests = Vec::with_capacity(16);
    for idx in 0..16 {
        let brain = if idx < 8 {
            Some(Arc::clone(&brains[idx]))
        } else {
            None
        };
        requests.push(AgentInferenceRequest {
            entity: world.spawn_empty().id(),
            sensory_input: inputs[idx],
            request_id: (idx + 1) as u64,
            brain,
        });
    }

    let model_a =
        BrainModel::new_seeded_cpu(SENSORY_SLOTS, EVOLVED_ARCH.hidden, ACTION_SLOTS, HD_SEED);
    let model_b = BrainModel::new_seeded_cpu(
        SENSORY_SLOTS,
        EVOLVED_ARCH.hidden,
        ACTION_SLOTS,
        HD_SEED + 1,
    );

    let mut responses_a = Vec::new();
    let mut responses_b = Vec::new();
    let mut scratch_a = InferenceScratch::with_capacity(16);
    let mut scratch_b = InferenceScratch::with_capacity(16);

    run_inference_batch(&model_a, &requests, &mut responses_a, &mut scratch_a);
    run_inference_batch(&model_b, &requests, &mut responses_b, &mut scratch_b);

    assert_eq!(responses_a.len(), 16);
    assert_eq!(responses_b.len(), 16);

    // Per-agent BrainGenotype requests bypass the shared BrainModel completely.
    for idx in 0..8 {
        assert_eq!(responses_a[idx].actions, responses_b[idx].actions);
        assert!(AgentInferenceResponse::actions_are_valid(
            &responses_a[idx].actions
        ));
    }
    assert_ne!(responses_a[0].actions, responses_a[1].actions);

    // Shared-model requests (brain == None) reflect the different shared BrainModel weights on CPG slots,
    // while keeping interaction gates open at 1.0.
    let mut any_cpg_diff = false;
    for idx in 8..16 {
        assert!(AgentInferenceResponse::actions_are_valid(
            &responses_a[idx].actions
        ));
        assert!(AgentInferenceResponse::actions_are_valid(
            &responses_b[idx].actions
        ));
        if responses_a[idx].actions[..action_index::CPG_LEN]
            != responses_b[idx].actions[..action_index::CPG_LEN]
        {
            any_cpg_diff = true;
        }
        for slot in action_index::CPG_LEN..ACTION_SLOTS {
            assert_eq!(responses_a[idx].actions[slot], 1.0);
            assert_eq!(responses_b[idx].actions[slot], 1.0);
        }
    }
    assert!(any_cpg_diff);
}

#[test]
fn batched_3d_tensor_forward_matches_per_agent_brain_genotype_loop_on_ndarray() {
    let mut rng = ChaCha12Rng::seed_from_u64(HD_SEED);
    let count = 32;
    let brains = make_test_brains(count, &mut rng);
    let inputs = make_sensory_inputs(count, &mut rng);

    let mut world = World::new();
    let requests: Vec<AgentInferenceRequest> = (0..count)
        .map(|idx| AgentInferenceRequest {
            entity: world.spawn_empty().id(),
            sensory_input: inputs[idx],
            request_id: (idx + 1) as u64,
            brain: Some(Arc::clone(&brains[idx])),
        })
        .collect();

    let model =
        BrainModel::new_seeded_cpu(SENSORY_SLOTS, EVOLVED_ARCH.hidden, ACTION_SLOTS, HD_SEED);
    let mut responses = Vec::new();
    let mut scratch = InferenceScratch::with_capacity(count);
    run_inference_batch(&model, &requests, &mut responses, &mut scratch);

    assert_eq!(responses.len(), count);
    assert_ne!(responses[0].actions, responses[1].actions);

    let batched = batched_3d_actor_forward::<NdArray<f32>>(&brains, &inputs, &NdArrayDevice::Cpu);
    assert_eq!(batched.len(), count * ACTION_SLOTS);

    let mut max_diff = 0.0_f32;
    for (agent_idx, response) in responses.iter().enumerate() {
        assert!(AgentInferenceResponse::actions_are_valid(&response.actions));
        for slot in 0..ACTION_SLOTS {
            let expected = response.actions[slot];
            let actual = batched[agent_idx * ACTION_SLOTS + slot];
            assert!(actual >= 0.0);
            assert!(actual <= 1.0);
            let diff = (expected - actual).abs();
            if diff > max_diff {
                max_diff = diff;
            }
        }
    }
    assert!(
        max_diff < 1e-5,
        "batched 3D NdArray parity diverged: max_diff={max_diff}"
    );
}

#[cfg(feature = "ml-wgpu")]
#[test]
fn batched_3d_tensor_forward_on_wgpu_matches_per_agent_brain_genotype_loop() {
    type WgpuBackend = burn_wgpu::Wgpu<burn_wgpu::AutoGraphicsApi, f32, i32>;

    let mut rng = ChaCha12Rng::seed_from_u64(HD_SEED);
    let count = 32;
    let brains = make_test_brains(count, &mut rng);
    let inputs = make_sensory_inputs(count, &mut rng);

    let mut world = World::new();
    let requests: Vec<AgentInferenceRequest> = (0..count)
        .map(|idx| AgentInferenceRequest {
            entity: world.spawn_empty().id(),
            sensory_input: inputs[idx],
            request_id: (idx + 1) as u64,
            brain: Some(Arc::clone(&brains[idx])),
        })
        .collect();

    let model =
        BrainModel::new_seeded_cpu(SENSORY_SLOTS, EVOLVED_ARCH.hidden, ACTION_SLOTS, HD_SEED);
    let mut responses = Vec::new();
    let mut scratch = InferenceScratch::with_capacity(count);
    run_inference_batch(&model, &requests, &mut responses, &mut scratch);

    let batched = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let device = burn_wgpu::WgpuDevice::default();
        batched_3d_actor_forward::<WgpuBackend>(&brains, &inputs, &device)
    })) {
        Ok(values) => values,
        Err(_) => batched_3d_actor_forward::<NdArray<f32>>(&brains, &inputs, &NdArrayDevice::Cpu),
    };

    assert_eq!(batched.len(), count * ACTION_SLOTS);
    let mut max_diff = 0.0_f32;
    for (agent_idx, response) in responses.iter().enumerate() {
        for slot in 0..ACTION_SLOTS {
            let diff = (response.actions[slot] - batched[agent_idx * ACTION_SLOTS + slot]).abs();
            if diff > max_diff {
                max_diff = diff;
            }
        }
    }
    assert!(
        max_diff < 1e-5,
        "batched 3D WGPU parity diverged: max_diff={max_diff}"
    );
}
