use bevy_ecs::prelude::*;
use glam::Vec3;
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::ecs::{
    Agent, Food, Position, Velocity, ParentAgent,
    energy_decay_system, detect_food_collisions_system
};
use anima_engine_lib::physics::dynamics::{RigidBody, integrate_physics_system};
use anima_engine_lib::ai::cpg::TimeStep;

#[test]
fn test_homeostatic_energy_decay() {
    let mut world = World::new();
    world.insert_resource(TimeStep(1.0)); // 1 second
    
    let agent_entity = world.spawn((
        Agent,
        HomeostaticState {
            energy: 10.0,
            energy_target: 100.0,
            hydration: 100.0,
            hydration_target: 100.0,
            temperature: 36.5,
            temp_target: 36.5,
            previous_deviation: 0.0,
        },
    )).id();
    
    let mut schedule = Schedule::default();
    schedule.add_systems(energy_decay_system);
    schedule.run(&mut world);
    
    let homeo = world.get::<HomeostaticState>(agent_entity).unwrap();
    // Decay is 0.5 * time_step.0, with time_step.0 = 1.0, decay is 0.5. So 10.0 - 0.5 = 9.5
    assert!(homeo.energy < 10.0);
    assert_eq!(homeo.energy, 9.5);
}

#[test]
fn test_homeostatic_death_trigger() {
    let mut world = World::new();
    world.insert_resource(TimeStep(0.1));
    
    // Spawn Parent Agent with energy = 0.0
    let agent_entity = world.spawn((
        Agent,
        HomeostaticState {
            energy: 0.0, // Dead/depleted
            energy_target: 100.0,
            hydration: 100.0,
            hydration_target: 100.0,
            temperature: 36.5,
            temp_target: 36.5,
            previous_deviation: 0.0,
        },
    )).id();
    
    // Spawn Segment (which is a RigidBody) belonging to this parent agent
    let segment_entity = world.spawn((
        ParentAgent(agent_entity),
        RigidBody {
            mass: 1.0,
            velocity: Vec3::new(1.0, 0.0, 1.0),
            force: Vec3::new(10.0, 0.0, 10.0),
        },
        Position(Vec3::ZERO),
        Velocity(Vec3::new(1.0, 0.0, 1.0)),
    )).id();
    
    let mut schedule = Schedule::default();
    schedule.add_systems(integrate_physics_system);
    schedule.run(&mut world);
    
    // After running the physics integration, verify velocity and force are clamped to zero
    let body = world.get::<RigidBody>(segment_entity).unwrap();
    let vel = world.get::<Velocity>(segment_entity).unwrap();
    
    assert_eq!(body.velocity, Vec3::ZERO);
    assert_eq!(body.force, Vec3::ZERO);
    assert_eq!(vel.0, Vec3::ZERO);
}

#[test]
fn test_homeostatic_deviation_calc() {
    let state = HomeostaticState {
        energy: 80.0,
        energy_target: 100.0,
        hydration: 90.0,
        hydration_target: 100.0,
        temperature: 38.5,
        temp_target: 36.5,
        previous_deviation: 0.0,
    };
    
    // Calculation:
    // 0.0001 * (80 - 100)^2 = 0.0001 * 400 = 0.04
    // 0.0001 * (90 - 100)^2 = 0.0001 * 100 = 0.01
    // 0.0156 * (38.5 - 36.5)^2 = 0.0156 * 4 = 0.0624
    // Total = 0.04 + 0.01 + 0.0624 = 0.1124
    let dev = state.compute_deviation();
    assert!((dev - 0.1124).abs() < 1e-5);
}

#[test]
fn test_homeostatic_reward_calc() {
    let state = HomeostaticState {
        energy: 95.0,
        energy_target: 100.0,
        hydration: 95.0,
        hydration_target: 100.0,
        temperature: 37.0,
        temp_target: 36.5,
        previous_deviation: 0.0,
    };
    
    // If previous deviation was larger, reward should be positive.
    let prev_dev_large = 1.0;
    let reward_pos = state.compute_reward(prev_dev_large);
    assert!(reward_pos > 0.0);
    
    // If previous deviation was smaller, reward should be negative.
    let prev_dev_small = 0.00001;
    let reward_neg = state.compute_reward(prev_dev_small);
    assert!(reward_neg < 0.0);
}

#[test]
fn test_food_collision_consumption() {
    let mut world = World::new();
    
    // Spawn Agent
    let agent_entity = world.spawn((
        Agent,
        Position(Vec3::ZERO),
        HomeostaticState {
            energy: 50.0,
            energy_target: 100.0,
            hydration: 50.0,
            hydration_target: 100.0,
            temperature: 36.5,
            temp_target: 36.5,
            previous_deviation: 0.0,
        },
    )).id();
    
    // Spawn Food at the same spot (distance < 1.5)
    let food_entity = world.spawn((
        Food {
            energy_value: 30.0,
            hydration_value: 20.0,
        },
        Position(Vec3::new(0.1, 0.0, 0.1)),
    )).id();
    
    let mut schedule = Schedule::default();
    schedule.add_systems(detect_food_collisions_system);
    schedule.run(&mut world);
    
    // Verify food is despawned (does not exist in world)
    assert!(world.get_entity(food_entity).is_none());
    
    // Verify energy and hydration increased
    let homeo = world.get::<HomeostaticState>(agent_entity).unwrap();
    assert_eq!(homeo.energy, 80.0);
    assert_eq!(homeo.hydration, 70.0);
}

#[test]
fn test_homeostatic_decay_clamping() {
    let mut world = World::new();
    world.insert_resource(TimeStep(100.0)); // Huge time step to force decay to below zero
    
    let agent_entity = world.spawn((
        Agent,
        HomeostaticState {
            energy: 1.0,
            energy_target: 100.0,
            hydration: 1.0,
            hydration_target: 100.0,
            temperature: 36.5,
            temp_target: 36.5,
            previous_deviation: 0.0,
        },
    )).id();
    
    let mut schedule = Schedule::default();
    schedule.add_systems(energy_decay_system);
    schedule.run(&mut world);
    
    let homeo = world.get::<HomeostaticState>(agent_entity).unwrap();
    assert_eq!(homeo.energy, 0.0); // clamped to 0.0, not negative
    
    // Let's also verify that eating clamps to target
    let mut world2 = World::new();
    let agent2 = world2.spawn((
        Agent,
        Position(Vec3::ZERO),
        HomeostaticState {
            energy: 95.0,
            energy_target: 100.0,
            hydration: 95.0,
            hydration_target: 100.0,
            temperature: 36.5,
            temp_target: 36.5,
            previous_deviation: 0.0,
        },
    )).id();
    
    world2.spawn((
        Food {
            energy_value: 50.0,
            hydration_value: 50.0,
        },
        Position(Vec3::ZERO),
    ));
    
    let mut schedule2 = Schedule::default();
    schedule2.add_systems(detect_food_collisions_system);
    schedule2.run(&mut world2);
    
    let homeo2 = world2.get::<HomeostaticState>(agent2).unwrap();
    assert_eq!(homeo2.energy, 100.0); // clamped to target
    assert_eq!(homeo2.hydration, 100.0); // clamped to target
}
