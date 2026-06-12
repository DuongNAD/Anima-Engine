use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use bevy_ecs::prelude::*;
use glam::Vec3;

use anima_engine_lib::core::ecs::{
    ShardingConfig, InboundMigrationReceiver,
    OutboundMigrationSender, ShardingResource, check_migration_boundaries_system,
    process_inbound_migrations_system, update_positions_system, AgentParentLineageIds, Prey, Agent,
    MapBounds, ChildrenLinks, Position, Velocity, Rotation
};
use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::evolution::genotype::{MorphologyGenotype, MorphologyNode};
use anima_engine_lib::core::engine::AgentGenotype;
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::engine::{AgentLineageId, AgentGeneration, run_websocket_client};

#[test]
#[should_panic(expected = "min > max, or either was NaN")]
fn test_migration_bounds_too_narrow_panic() {
    let mut world = World::new();

    let (_inbound_tx, inbound_rx) = crossbeam_channel::unbounded();
    world.insert_resource(InboundMigrationReceiver(inbound_rx));

    let (outbound_tx, _outbound_rx) = crossbeam_channel::unbounded();
    world.insert_resource(OutboundMigrationSender(outbound_tx));

    // Bounds width = 0.01 (< 0.02)
    let bounds = MapBounds {
        min: Vec3::new(0.0, 0.0, 0.0),
        max: Vec3::new(0.01, 10.0, 0.01),
    };
    world.insert_resource(bounds);

    let sharding_config = Arc::new(RwLock::new(ShardingConfig {
        local_port: 8080,
        left_target_port: Some(9999),
        right_target_port: Some(9999),
    }));
    world.insert_resource(ShardingResource(sharding_config));

    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.2, mass: 1.5 });

    // Spawn agent out of bounds on the right (x = 0.02)
    let _agent_entity = world.spawn((
        Agent,
        Prey,
        Position(Vec3::new(0.02, 0.0, 0.0)),
        Rotation(glam::Quat::IDENTITY),
        Velocity(Vec3::new(1.0, 0.0, 0.0)),
        AgentGenotype(genotype.clone()),
        HomeostaticState {
            energy: 100.0,
            energy_target: 100.0,
            hydration: 100.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
        AgentLineageId("panic-lineage-id".to_string()),
        AgentGeneration(1),
        AgentParentLineageIds(vec![]),
        ChildrenLinks(vec![]),
    )).id();

    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    schedule.add_systems((
        check_migration_boundaries_system,
        apply_deferred,
    ).chain());

    // This should panic due to clamp(0.01, 0.0) where min > max
    schedule.run(&mut world);
}

#[tokio::test]
async fn test_high_velocity_migration_loop_under_narrow_bounds() {
    let mut world = World::new();

    let (inbound_tx, inbound_rx) = crossbeam_channel::unbounded();
    world.insert_resource(InboundMigrationReceiver(inbound_rx));

    let (outbound_tx, outbound_rx) = crossbeam_channel::unbounded();
    world.insert_resource(OutboundMigrationSender(outbound_tx));

    // Narrow map bounds: width = 0.5 (offset = 0.05)
    let bounds = MapBounds {
        min: Vec3::new(0.0, 0.0, 0.0),
        max: Vec3::new(0.5, 10.0, 0.5),
    };
    world.insert_resource(bounds);

    // Simulation timestep resource
    world.insert_resource(TimeStep(0.1));

    // Both left and right target ports are configured and point to a closed port (9999)
    let sharding_config = Arc::new(RwLock::new(ShardingConfig {
        local_port: 8080,
        left_target_port: Some(9999),
        right_target_port: Some(9999),
    }));
    world.insert_resource(ShardingResource(sharding_config));

    // Spawn agent moving right at high speed: x = 0.6, velocity = 10.0 (vel * dt = 1.0, greater than width 0.5)
    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.2, mass: 1.5 });

    let _agent_entity = world.spawn((
        Agent,
        Prey,
        Position(Vec3::new(0.6, 0.0, 0.0)),
        Rotation(glam::Quat::IDENTITY),
        Velocity(Vec3::new(10.0, 0.0, 0.0)),
        AgentGenotype(genotype.clone()),
        HomeostaticState {
            energy: 100.0,
            energy_target: 100.0,
            hydration: 100.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
        AgentLineageId("loop-lineage-id-2".to_string()),
        AgentGeneration(1),
        AgentParentLineageIds(vec![]),
        ChildrenLinks(vec![]),
    )).id();

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = Arc::clone(&running);
    let inbound_tx_clone = inbound_tx.clone();

    let client_handle = tokio::spawn(async move {
        run_websocket_client::<tauri::test::MockRuntime>(
            outbound_rx,
            inbound_tx_clone,
            running_clone,
            None,
            8080,
        ).await;
    });

    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    schedule.add_systems((
        update_positions_system,
        check_migration_boundaries_system,
        apply_deferred,
        process_inbound_migrations_system,
    ).chain());

    println!("--- TICK 1 START ---");
    // Tick 1: agent is at x = 0.6. Out of bounds. Despawned & migrated to port 9999.
    schedule.run(&mut world);
    
    let mut query = world.query::<(&Position, &Velocity)>();
    println!("After Tick 1 run, agents in world: {:?}", query.iter(&world).map(|(p, v)| (p.0.x, v.0.x)).collect::<Vec<_>>());

    tokio::time::sleep(Duration::from_millis(50)).await;

    println!("--- TICK 1 BOUNCE-BACK SPAWN ---");
    // Bounce-back yields: agent spawned at x = 0.45, velocity = -10.0.
    schedule.run(&mut world);
    println!("After Tick 1 bounce-back spawn, agents in world: {:?}", query.iter(&world).map(|(p, v)| (p.0.x, v.0.x)).collect::<Vec<_>>());

    // Verify it spawned at x = 0.45, vel = -10.0
    let results: Vec<_> = query.iter(&world).collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0.0.x, 0.45);
    assert_eq!(results[0].1.0.x, -10.0);

    println!("--- TICK 2 START ---");
    // Tick 2:
    // First, update_positions_system runs. Position becomes 0.45 + (-10.0) * 0.1 = -0.55.
    // Then check_migration_boundaries_system runs. Detects x = -0.55 < 0.0.
    // Out of bounds on the left! Despawned & migrated to port 9999.
    schedule.run(&mut world);
    println!("After Tick 2 run, agents in world: {:?}", query.iter(&world).map(|(p, v)| (p.0.x, v.0.x)).collect::<Vec<_>>());

    tokio::time::sleep(Duration::from_millis(50)).await;

    println!("--- TICK 2 BOUNCE-BACK SPAWN ---");
    // Bounce-back yields: agent spawned at x = 0.05, velocity = 10.0.
    schedule.run(&mut world);
    println!("After Tick 2 bounce-back spawn, agents in world: {:?}", query.iter(&world).map(|(p, v)| (p.0.x, v.0.x)).collect::<Vec<_>>());

    // Verify it spawned at x = 0.05, vel = 10.0
    let results: Vec<_> = query.iter(&world).collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0.0.x, 0.05);
    assert_eq!(results[0].1.0.x, 10.0);

    println!("--- TICK 3 START ---");
    // Tick 3:
    // update_positions_system: Position becomes 0.05 + 10.0 * 0.1 = 1.05.
    // check_migration_boundaries_system: Detects x = 1.05 > 0.5.
    // Out of bounds on the right! Despawned & migrated.
    schedule.run(&mut world);
    println!("After Tick 3 run, agents in world: {:?}", query.iter(&world).map(|(p, v)| (p.0.x, v.0.x)).collect::<Vec<_>>());

    tokio::time::sleep(Duration::from_millis(50)).await;

    println!("--- TICK 3 BOUNCE-BACK SPAWN ---");
    // Bounce-back yields: agent spawned at x = 0.45, velocity = -10.0.
    schedule.run(&mut world);
    println!("After Tick 3 bounce-back spawn, agents in world: {:?}", query.iter(&world).map(|(p, v)| (p.0.x, v.0.x)).collect::<Vec<_>>());

    // Verify it spawned at x = 0.45, vel = -10.0
    let results: Vec<_> = query.iter(&world).collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0.0.x, 0.45);
    assert_eq!(results[0].1.0.x, -10.0);

    running.store(false, Ordering::SeqCst);
    let _ = client_handle.await;
}
