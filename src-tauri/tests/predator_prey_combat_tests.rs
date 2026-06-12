mod common;

use bevy_ecs::prelude::*;
use glam::{Quat, Vec3};
use anima_engine_lib::core::ecs::{
    Agent, Position, Rotation, Velocity, Predator, Prey, AgentClass, Food,
    combat_system, ParentAgent
};
use anima_engine_lib::ai::model::{BrainModel, BrainInferenceBuffer, brain_inference_system};
use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::physics::dynamics::{RigidBody, integrate_physics_system};
use std::sync::Mutex;

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_predator_prey_classification() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut world = World::new();
    
    // Set up a genotype
    let mut genotype = anima_engine_lib::evolution::genotype::MorphologyGenotype::new();
    genotype.add_node(anima_engine_lib::evolution::genotype::MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.2,
        mass: 1.5,
    });
    
    // Spawn Predator using SpawnGenotypeCommand
    let pred_cmd = anima_engine_lib::core::engine::SpawnGenotypeCommand {
        genotype: genotype.clone(),
        initial_pos: Vec3::new(0.0, 0.0, 0.0),
        initial_rot: Quat::IDENTITY,
        agent_class: AgentClass::Predator,
        lineage_id: "test-predator-lineage".to_string(),
        generation: 0,
        parent_ids: Vec::new(),
    };
    bevy_ecs::system::Command::apply(pred_cmd, &mut world);
    
    // Spawn Prey using SpawnGenotypeCommand
    let prey_cmd = anima_engine_lib::core::engine::SpawnGenotypeCommand {
        genotype: genotype.clone(),
        initial_pos: Vec3::new(10.0, 0.0, 0.0),
        initial_rot: Quat::IDENTITY,
        agent_class: AgentClass::Prey,
        lineage_id: "test-prey-lineage".to_string(),
        generation: 0,
        parent_ids: Vec::new(),
    };
    bevy_ecs::system::Command::apply(prey_cmd, &mut world);
    
    // Query and check
    let predators: Vec<Entity> = world.query_filtered::<Entity, With<Predator>>().iter(&world).collect();
    let preys: Vec<Entity> = world.query_filtered::<Entity, With<Prey>>().iter(&world).collect();
    
    assert_eq!(predators.len(), 1);
    assert_eq!(preys.len(), 1);
}

#[test]
fn test_predator_tracking_inputs() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut world = World::new();
    
    world.insert_resource(BrainModel::new(15, 64, 4));
    world.insert_resource(BrainInferenceBuffer::default());
    
    // Spawn Predator at (0, 0, 0)
    let predator = world.spawn((
        Agent,
        Predator,
        Position(Vec3::new(0.0, 0.0, 0.0)),
        Rotation(Quat::IDENTITY),
        HomeostaticState {
            energy: 80.0,
            energy_target: 100.0,
            hydration: 70.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
        anima_engine_lib::ai::hrrl::LastTransitionState::default(),
    )).id();
    
    // Spawn active Prey at (0, 0, 5.0)
    let prey = world.spawn((
        Agent,
        Prey,
        Position(Vec3::new(0.0, 0.0, 5.0)),
        Rotation(Quat::IDENTITY),
        HomeostaticState {
            energy: 80.0,
            energy_target: 100.0,
            hydration: 70.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
        anima_engine_lib::ai::hrrl::LastTransitionState::default(),
    )).id();
    
    // Spawn food at (0, 0, 10.0)
    world.spawn((
        Food { energy_value: 30.0, hydration_value: 20.0 },
        Position(Vec3::new(0.0, 0.0, 10.0)),
    ));
    
    let mut schedule = Schedule::default();
    schedule.add_systems(brain_inference_system);
    schedule.run(&mut world);
    
    // Verify target tracking inputs (stored in LastTransitionState.state)
    // For Predator: should target nearest active Prey (dist vector (0, 0, 5))
    let pred_last = world.get::<anima_engine_lib::ai::hrrl::LastTransitionState>(predator).unwrap();
    assert!((pred_last.state[0] - 0.0).abs() < 1e-4);
    assert!((pred_last.state[1] - 0.0).abs() < 1e-4);
    assert!((pred_last.state[2] - 5.0).abs() < 1e-4, "Predator target should be nearest Prey at relative (0,0,5), got state[2]={}", pred_last.state[2]);
    
    // For Prey: should target nearest Food (dist vector (0, 0, 5) relative to prey at (0,0,5))
    let prey_last = world.get::<anima_engine_lib::ai::hrrl::LastTransitionState>(prey).unwrap();
    assert!((prey_last.state[0] - 0.0).abs() < 1e-4);
    assert!((prey_last.state[1] - 0.0).abs() < 1e-4);
    assert!((prey_last.state[2] - 5.0).abs() < 1e-4, "Prey target should be nearest Food at relative (0,0,5), got state[2]={}", prey_last.state[2]);
}

#[test]
fn test_predator_prey_collision_and_combat() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut world = World::new();
    
    // Predator at (0,0,0)
    let predator = world.spawn((
        Agent,
        Predator,
        Position(Vec3::new(0.0, 0.0, 0.0)),
        HomeostaticState {
            energy: 50.0,
            energy_target: 100.0,
            hydration: 100.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
    )).id();
    
    // Prey at (1.0, 0.0, 0.0) (distance 1.0 < 1.5)
    let prey = world.spawn((
        Agent,
        Prey,
        Position(Vec3::new(1.0, 0.0, 0.0)),
        HomeostaticState {
            energy: 30.0,
            energy_target: 100.0,
            hydration: 100.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
    )).id();
    
    let mut schedule = Schedule::default();
    schedule.add_systems(combat_system);
    schedule.run(&mut world);
    
    let pred_homeo = world.get::<HomeostaticState>(predator).unwrap();
    let prey_homeo = world.get::<HomeostaticState>(prey).unwrap();
    
    // Predator needs 50 energy. Prey has 30 energy.
    // So 30 energy is transferred. Predator becomes 80. Prey becomes 0.
    assert_eq!(pred_homeo.energy, 80.0);
    assert_eq!(prey_homeo.energy, 0.0);
}

#[test]
fn test_prey_carcass_freezing() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut world = World::new();
    world.insert_resource(TimeStep(0.016));
    
    let prey = world.spawn((
        Agent,
        Prey,
        Position(Vec3::ZERO),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::new(1.0, 0.0, 0.0)),
        RigidBody {
            mass: 1.0,
            velocity: Vec3::new(1.0, 0.0, 0.0),
            force: Vec3::new(10.0, 0.0, 0.0),
        },
        HomeostaticState {
            energy: 0.0, // dead / frozen
            energy_target: 100.0,
            hydration: 100.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
    )).id();
    
    // Child segment
    let child = world.spawn((
        ParentAgent(prey),
        Position(Vec3::new(0.0, 0.0, 1.0)),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::new(1.0, 0.0, 0.0)),
        RigidBody {
            mass: 1.0,
            velocity: Vec3::new(1.0, 0.0, 0.0),
            force: Vec3::new(10.0, 0.0, 0.0),
        },
    )).id();
    
    let mut schedule = Schedule::default();
    schedule.add_systems(integrate_physics_system);
    schedule.run(&mut world);
    
    // Verify velocity is clamped to zero
    let prey_vel = world.get::<Velocity>(prey).unwrap().0;
    let prey_rb = world.get::<RigidBody>(prey).unwrap();
    let child_vel = world.get::<Velocity>(child).unwrap().0;
    let child_rb = world.get::<RigidBody>(child).unwrap();
    
    assert_eq!(prey_vel, Vec3::ZERO);
    assert_eq!(prey_rb.velocity, Vec3::ZERO);
    assert_eq!(prey_rb.force, Vec3::ZERO);
    
    assert_eq!(child_vel, Vec3::ZERO);
    assert_eq!(child_rb.velocity, Vec3::ZERO);
    assert_eq!(child_rb.force, Vec3::ZERO);
}

#[test]
fn test_zero_allocation_combat_hot_path() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut world = World::new();
    world.insert_resource(TimeStep(0.016));
    
    // Spawn a predator and a prey
    world.spawn((
        Agent,
        Predator,
        Position(Vec3::new(0.0, 0.0, 0.0)),
        HomeostaticState {
            energy: 50.0,
            energy_target: 100.0,
            hydration: 100.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
    ));
    
    world.spawn((
        Agent,
        Prey,
        Position(Vec3::new(1.0, 0.0, 0.0)),
        HomeostaticState {
            energy: 30.0,
            energy_target: 100.0,
            hydration: 100.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
    ));
    
    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    schedule.add_systems(combat_system);
    
    // Warm up
    for _ in 0..10 {
        schedule.run(&mut world);
    }
    
    ALLOCATOR.start_tracking();
    for _ in 0..10 {
        schedule.run(&mut world);
    }
    let allocs = ALLOCATOR.stop_tracking();
    
    assert_eq!(allocs, 0, "Combat hot path should perform 0 heap allocations, but made {}", allocs);
}
