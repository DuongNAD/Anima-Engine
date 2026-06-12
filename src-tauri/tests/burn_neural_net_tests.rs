use burn_ndarray::NdArrayDevice;
use burn::tensor::{Tensor, Data, Shape};
use anima_engine_lib::ai::model::{ActorCriticModel, BrainModel, BrainInferenceBuffer, brain_inference_system, DefaultBackend};
use anima_engine_lib::core::ecs::{Agent, Position, Rotation, Segment, ParentAgent, Food};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::ai::cpg::CpgOscillator;
use bevy_ecs::prelude::*;
use glam::{Vec3, Quat};

#[test]
fn test_burn_inference_success() {
    let device = NdArrayDevice::Cpu;
    let model = ActorCriticModel::<DefaultBackend>::new(15, 64, 4, &device);
    let data = Data::new(vec![0.5; 15], Shape::new([1, 15]));
    let input = Tensor::<DefaultBackend, 2>::from_data(data, &device);
    let (actor_out, critic_out) = model.forward(input);
    
    assert_eq!(actor_out.shape().dims, [1, 4]);
    assert_eq!(critic_out.shape().dims, [1, 1]);
}

#[test]
fn test_burn_inference_dimensions() {
    let device = NdArrayDevice::Cpu;
    let model = ActorCriticModel::<DefaultBackend>::new(15, 64, 4, &device);
    
    let data = Data::new(vec![1.0; 15], Shape::new([1, 15]));
    let input = Tensor::<DefaultBackend, 2>::from_data(data, &device);
    let (actor_out, _) = model.forward(input);
    assert_eq!(actor_out.shape().dims[1], 4);
}

#[test]
fn test_burn_inference_bound() {
    let device = NdArrayDevice::Cpu;
    let model = ActorCriticModel::<DefaultBackend>::new(15, 64, 4, &device);
    
    let inputs = vec![
        1000.0, -1000.0, 500.0, -500.0, 100.0, -100.0, 50.0, -50.0, 10.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
    ];
    let data = Data::new(inputs, Shape::new([1, 15]));
    let input = Tensor::<DefaultBackend, 2>::from_data(data, &device);
    let (actor_out, _) = model.forward(input);
    let values = actor_out.into_data().value;
    
    for val in values {
        assert!((0.0..=1.0).contains(&val), "Output value {} is not in range [0.0, 1.0]", val);
    }
}

#[test]
fn test_burn_inference_zero_input() {
    let device = NdArrayDevice::Cpu;
    let model = ActorCriticModel::<DefaultBackend>::new(15, 64, 4, &device);
    
    let data = Data::new(vec![0.0; 15], Shape::new([1, 15]));
    let input = Tensor::<DefaultBackend, 2>::from_data(data, &device);
    let (actor_out, _) = model.forward(input);
    let values = actor_out.into_data().value;
    
    for val in values {
        assert!(val.is_finite());
        assert!((0.0..=1.0).contains(&val));
    }
}

#[test]
fn test_brain_inference_system_execution() {
    let mut world = World::new();
    world.insert_resource(BrainModel::new(15, 64, 4));
    world.insert_resource(BrainInferenceBuffer::default());
    
    // Spawn Agent
    let agent_entity = world.spawn((
        Agent,
        Position(Vec3::new(0.0, 0.0, 0.0)),
        Rotation(Quat::IDENTITY),
        HomeostaticState {
            energy: 100.0,
            energy_target: 100.0,
            hydration: 100.0,
            hydration_target: 100.0,
            temperature: 36.5,
            temp_target: 36.5,
            previous_deviation: 0.0,
        },
    )).id();
    
    // Spawn Food
    world.spawn((
        Food { energy_value: 30.0, hydration_value: 20.0 },
        Position(Vec3::new(2.0, 0.0, 0.0)),
    ));
    
    // Spawn Segments
    let segment0 = world.spawn((
        ParentAgent(agent_entity),
        Segment { id: 0, length: 1.0, radius: 0.2, mass: 1.0 },
        CpgOscillator::new(1.0, 1.0),
    )).id();
    
    let segment1 = world.spawn((
        ParentAgent(agent_entity),
        Segment { id: 1, length: 1.0, radius: 0.2, mass: 1.0 },
        CpgOscillator::new(1.0, 1.0),
    )).id();
    
    // Run the Bevy schedule containing our system
    let mut schedule = Schedule::default();
    schedule.add_systems(brain_inference_system);
    schedule.run(&mut world);
    
    // Assert CpgOscillators are updated within range
    let osc0 = world.get::<CpgOscillator>(segment0).unwrap();
    let osc1 = world.get::<CpgOscillator>(segment1).unwrap();
    
    assert!(osc0.frequency >= 0.1 && osc0.frequency <= 3.0);
    assert!(osc0.amplitude >= 0.0 && osc0.amplitude <= 1.5);
    assert!(osc1.frequency >= 0.1 && osc1.frequency <= 3.0);
    assert!(osc1.amplitude >= 0.0 && osc1.amplitude <= 1.5);
}

#[test]
fn test_burn_inference_nan_inf() {
    let device = NdArrayDevice::Cpu;
    let model = ActorCriticModel::<DefaultBackend>::new(15, 64, 4, &device);
    
    let inputs = vec![
        f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, 1.0, f32::NAN, 0.5, f32::INFINITY, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
    ];
    let data = Data::new(inputs, Shape::new([1, 15]));
    let input = Tensor::<DefaultBackend, 2>::from_data(data, &device);
    
    let (actor_out, critic_out) = model.forward(input);
    let values = actor_out.into_data().value;
    
    for val in values {
        // Output might be NaN or valid float, but verify no panic occurred
        assert!(!val.is_infinite());
    }
    
    let critic_values = critic_out.into_data().value;
    for val in critic_values {
        assert!(!val.is_infinite());
    }
}
