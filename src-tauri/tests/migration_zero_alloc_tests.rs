mod common;

use std::sync::{Arc, Mutex};
use bevy_ecs::prelude::*;
use glam::Vec3;

use anima_engine_lib::core::ecs::{
    Agent, Prey, Position, Velocity, Rotation, MapBounds, ChildrenLinks, ParentAgent, Segment,
    ShardingConfig, ShardingResource, OutboundMigrationSender, InboundMigrationReceiver,
    check_migration_boundaries_system, process_inbound_migrations_system,
    AgentParentLineageIds,
};
use anima_engine_lib::core::engine::{
    AgentGenotype, AgentEvaluation, AgentLineageId, AgentGeneration,
};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::evolution::genotype::MorphologyGenotype;

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator = common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_migration_systems_zero_allocations_on_hot_path() {
    let _lock = TEST_LOCK.lock().unwrap();

    let mut world = World::new();

    let bounds = MapBounds {
        min: Vec3::new(-100.0, 0.0, -100.0),
        max: Vec3::new(100.0, 10.0, 100.0),
    };
    world.insert_resource(bounds);

    let (outbound_tx, _outbound_rx) = crossbeam_channel::unbounded();
    world.insert_resource(OutboundMigrationSender(outbound_tx));

    let (_inbound_tx, inbound_rx) = crossbeam_channel::unbounded();
    world.insert_resource(InboundMigrationReceiver(inbound_rx));

    let sharding_config = Arc::new(std::sync::RwLock::new(ShardingConfig {
        local_port: 8080,
        left_target_port: Some(8079),
        right_target_port: Some(8081),
    }));
    world.insert_resource(ShardingResource(sharding_config));

    // Setup schedule systems
    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    schedule.add_systems((
        check_migration_boundaries_system,
        process_inbound_migrations_system,
    ));

    // Spawn 1 agent with lineage components and children
    let initial_pos = Vec3::new(0.0, 0.0, 0.0); // Safe inside boundary [-100, 100]
    let segment_entity = world.spawn((
        ParentAgent(Entity::PLACEHOLDER),
        Position(initial_pos),
        Rotation(glam::Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        Segment { id: 0, length: 1.0, radius: 0.2, mass: 1.0 },
        ChildrenLinks(Vec::new()),
    )).id();

    let agent_entity = world.spawn((
        Agent,
        Prey,
        Position(initial_pos),
        Rotation(glam::Quat::IDENTITY),
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
        AgentGenotype(MorphologyGenotype::default()),
        AgentEvaluation {
            start_position: initial_pos,
            total_distance: 0.0,
            total_energy_expended: 0.0,
            survival_ticks: 0,
            last_position: initial_pos,
        },
        AgentLineageId("some-test-lineage-id".to_string()),
        AgentGeneration(0),
        AgentParentLineageIds(Vec::new()),
        ChildrenLinks(vec![segment_entity]),
    )).id();

    world.entity_mut(segment_entity).insert(ParentAgent(agent_entity));

    // Warm-up to initialize internal Bevy archetype arrays & cache query states
    for _ in 0..10 {
        schedule.run(&mut world);
    }

    // Start tracking allocations
    ALLOCATOR.start_tracking();

    // Run systems on hot path (no agent is out of bounds, no inbound migrations)
    for _ in 0..100 {
        schedule.run(&mut world);
    }

    // Stop tracking
    let allocations = ALLOCATOR.stop_tracking();

    // Assert zero heap allocations on the hot path
    assert_eq!(allocations, 0, "Expected 0 allocations on hot path, but recorded {}", allocations);
}
