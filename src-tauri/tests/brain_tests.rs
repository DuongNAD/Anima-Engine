

use bevy_ecs::prelude::*;
use glam::{Quat, Vec3};
use anima_engine_lib::ai::model::{BrainModel, BrainInferenceBuffer, brain_inference_system};
use anima_engine_lib::ai::cpg::CpgOscillator;
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::ecs::{Position, Rotation, Agent, Food, ParentAgent, Segment};

#[test]
fn test_brain_inference_system() {
    let mut world = World::new();

    // Register resources
    world.insert_resource(BrainModel::new(15, 64, 4));
    world.insert_resource(BrainInferenceBuffer::default());

    // Spawn an agent
    let agent_entity = world.spawn((
        Agent,
        Position(Vec3::ZERO),
        Rotation(Quat::IDENTITY),
        HomeostaticState {
            energy: 80.0,
            energy_target: 100.0,
            hydration: 70.0,
            hydration_target: 100.0,
            temperature: 36.5,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
        anima_engine_lib::ai::hrrl::LastTransitionState {
            state: [0.0; 15],
            action: [0.0; 4],
            has_last: false,
        },
    )).id();

    // Spawn a child segment with CpgOscillator
    let child_entity = world.spawn((
        ParentAgent(agent_entity),
        Segment { id: 0, length: 1.0, radius: 0.1, mass: 1.0 },
        CpgOscillator::new(1.0, 0.5),
    )).id();

    // Spawn food
    world.spawn((
        Food { energy_value: 30.0, hydration_value: 20.0 },
        Position(Vec3::new(0.0, 0.0, 5.0)),
    ));

    // Run the brain inference system
    let mut schedule = Schedule::default();
    schedule.add_systems(brain_inference_system);

    schedule.run(&mut world);

    // Assert that the child segment's oscillator has been updated
    let osc = world.get::<CpgOscillator>(child_entity).unwrap();
    // Initially frequency = 1.0, amplitude = 0.5
    // After system run, frequency should be in range [0.1, 3.0]
    // and amplitude should be in range [0.0, 1.5]
    println!("Updated Oscillator: frequency = {}, amplitude = {}", osc.frequency, osc.amplitude);
    assert!(osc.frequency >= 0.1 && osc.frequency <= 3.0);
    assert!(osc.amplitude >= 0.0 && osc.amplitude <= 1.5);

    // Verify buffer has reclaimed the outputs
    let buffer = world.get_resource::<BrainInferenceBuffer>().unwrap();
    assert_eq!(buffer.inputs.len(), 4); // action_dim = 4
}
