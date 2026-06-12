use bevy_ecs::prelude::*;
use glam::{Quat, Vec3};
use crossbeam_channel::bounded;

use anima_engine_lib::ai::hrrl::{HomeostaticState, LastTransitionState, TransitionSender, Transition};
use anima_engine_lib::ai::model::hrrl_learning_system;
use anima_engine_lib::core::ecs::{
    Agent, Food, FoodSpawnSettings, MapBounds, Position, Rotation, Velocity,
    ParentAgent, Segment, SegmentJointForce, metabolic_decay_system,
    spawn_food_system, detect_food_collisions_system,
};
use anima_engine_lib::physics::dynamics::RigidBody;
use anima_engine_lib::ai::cpg::TimeStep;

#[test]
fn test_metabolic_decay_and_thermoregulation() {
    let mut world = World::new();
    world.insert_resource(TimeStep(1.0 / 60.0));

    // Spawn agent with initial homeostatic values
    let agent_entity = world.spawn((
        Agent,
        Position(Vec3::ZERO),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        HomeostaticState {
            energy: 100.0,
            energy_target: 100.0,
            hydration: 100.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
    )).id();

    // Spawn rigid body / segment child to generate metabolic cost
    world.spawn((
        ParentAgent(agent_entity),
        RigidBody {
            mass: 2.0,
            velocity: Vec3::new(2.0, 0.0, 0.0),
            force: Vec3::ZERO,
        },
        Velocity(Vec3::new(2.0, 0.0, 0.0)),
        Segment {
            id: 0,
            length: 1.0,
            radius: 0.2,
            mass: 2.0,
        },
        SegmentJointForce(5.0),
    ));

    // Run metabolic decay system
    let mut schedule = Schedule::default();
    schedule.add_systems(metabolic_decay_system);
    schedule.run(&mut world);

    let homeo = world.get::<HomeostaticState>(agent_entity).unwrap();
    // Verify energy decay, hydration decay, and temperature dynamics
    assert!(homeo.energy < 100.0, "Energy should decay. Current: {}", homeo.energy);
    assert!(homeo.hydration < 100.0, "Hydration should decay. Current: {}", homeo.hydration);
    assert!(homeo.temperature > 37.0, "Temperature should increase. Current: {}", homeo.temperature);
}

#[test]
fn test_food_spawning_and_collision_replenishment() {
    let mut world = World::new();
    world.insert_resource(MapBounds {
        min: Vec3::new(-10.0, 0.0, -10.0),
        max: Vec3::new(10.0, 10.0, 10.0),
    });
    world.insert_resource(FoodSpawnSettings {
        max_food_count: 5,
        default_energy: 30.0,
        default_hydration: 20.0,
    });

    // 1. Verify spawning
    let mut schedule = Schedule::default();
    schedule.add_systems(spawn_food_system);
    schedule.run(&mut world);

    let spawned_food_count = world.query::<&Food>().iter(&world).count();
    assert_eq!(spawned_food_count, 5, "Should spawn exactly 5 food entities");

    // 2. Verify collision & consumption
    // Despawn all food entities first to have controlled test environment
    let food_entities: Vec<Entity> = world.query::<(Entity, &Food)>().iter(&world).map(|(e, _)| e).collect();
    for e in food_entities {
        world.despawn(e);
    }

    // Spawn agent at position (0, 0, 0) with low energy and hydration
    let agent_entity = world.spawn((
        Agent,
        Position(Vec3::ZERO),
        Rotation(Quat::IDENTITY),
        HomeostaticState {
            energy: 50.0,
            energy_target: 100.0,
            hydration: 50.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
    )).id();

    // Spawn segment at (0.5, 0.0, 0.0) so agent centroid is close
    world.spawn((
        ParentAgent(agent_entity),
        Position(Vec3::new(0.5, 0.0, 0.0)),
    ));

    // Spawn food at (1.0, 0.0, 0.0) which is close enough to centroid (0.25)
    let food_entity = world.spawn((
        Food {
            energy_value: 30.0,
            hydration_value: 20.0,
        },
        Position(Vec3::new(1.0, 0.0, 0.0)),
    )).id();

    let mut collision_schedule = Schedule::default();
    collision_schedule.add_systems(detect_food_collisions_system);
    collision_schedule.run(&mut world);

    // Verify food entity despawned
    assert!(world.get::<Food>(food_entity).is_none(), "Food entity should be despawned after collision");

    // Verify homeostatic replenishment
    let homeo = world.get::<HomeostaticState>(agent_entity).unwrap();
    assert_eq!(homeo.energy, 80.0, "Energy should increase by 30.0");
    assert_eq!(homeo.hydration, 70.0, "Hydration should increase by 20.0");
}

#[test]
fn test_transition_collection_and_sending() {
    let mut world = World::new();
    let (tx, rx) = bounded::<Transition>(10);
    world.insert_resource(TransitionSender(tx));

    let agent_entity = world.spawn((
        Agent,
        Position(Vec3::ZERO),
        Rotation(Quat::IDENTITY),
        HomeostaticState {
            energy: 90.0,
            energy_target: 100.0,
            hydration: 90.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 1.0,
        },
        LastTransitionState {
            state: [0.1; 15],
            action: [0.2; 4],
            has_last: true,
        },
    )).id();

    // Run the learning system
    let mut schedule = Schedule::default();
    schedule.add_systems(hrrl_learning_system);
    schedule.run(&mut world);

    // Verify transition was sent
    let recv_result = rx.try_recv();
    assert!(recv_result.is_ok(), "Transition should be sent to the channel");
    let transition = recv_result.unwrap();

    assert_eq!(transition.state, [0.1; 15]);
    assert_eq!(transition.action, [0.2; 4]);
    
    // Check reward computation
    let homeo = world.get::<HomeostaticState>(agent_entity).unwrap();
    let expected_reward = 1.0 - homeo.compute_deviation();
    assert!((transition.reward - expected_reward).abs() < 1e-5);

    // Verify update of previous_deviation and LastTransitionState state
    assert_eq!(homeo.previous_deviation, homeo.compute_deviation());
    let last = world.get::<LastTransitionState>(agent_entity).unwrap();
    assert_eq!(last.state[3], 90.0); // energy index
    assert_eq!(last.state[5], 90.0); // hydration index
}
