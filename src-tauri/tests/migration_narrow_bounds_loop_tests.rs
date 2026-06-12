use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use bevy_ecs::prelude::*;
use glam::Vec3;

use anima_engine_lib::core::ecs::{
    ShardingConfig, InboundMigrationReceiver,
    OutboundMigrationSender, ShardingResource, check_migration_boundaries_system,
    process_inbound_migrations_system, AgentParentLineageIds, Prey, Agent,
    MapBounds, ChildrenLinks, Position, Velocity, Rotation
};
use anima_engine_lib::evolution::genotype::{MorphologyGenotype, MorphologyNode};
use anima_engine_lib::core::engine::{
    AgentGenotype, AgentLineageId, AgentGeneration,
    run_websocket_client
};
use anima_engine_lib::ai::hrrl::HomeostaticState;

#[tokio::test]
async fn test_migration_narrow_bounds_infinite_loop() {
    let mut world = World::new();

    let (inbound_tx, inbound_rx) = crossbeam_channel::unbounded();
    world.insert_resource(InboundMigrationReceiver(inbound_rx));

    let (outbound_tx, outbound_rx) = crossbeam_channel::unbounded();
    world.insert_resource(OutboundMigrationSender(outbound_tx));

    // Narrow map bounds: width = 0.5 (smaller than the hardcoded 1.0 offset)
    let bounds = MapBounds {
        min: Vec3::new(0.0, 0.0, 0.0),
        max: Vec3::new(0.5, 10.0, 0.5),
    };
    world.insert_resource(bounds);

    // Both left and right target ports are configured (e.g. sharding) and point to a closed port (9999)
    let sharding_config = Arc::new(RwLock::new(ShardingConfig {
        local_port: 8080,
        left_target_port: Some(9999),
        right_target_port: Some(9999),
    }));
    world.insert_resource(ShardingResource(sharding_config));

    // Spawn initial agent already moving right and out of bounds on the right (x = 0.6)
    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.2, mass: 1.5 });

    let _agent_entity = world.spawn((
        Agent,
        Prey,
        Position(Vec3::new(0.6, 0.0, 0.0)),
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
        AgentLineageId("loop-lineage-id".to_string()),
        AgentGeneration(1),
        AgentParentLineageIds(vec![]),
        ChildrenLinks(vec![]),
    )).id();

    // Start run_websocket_client on closed ports
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
        check_migration_boundaries_system,
        apply_deferred,
        process_inbound_migrations_system,
    ));

    // Tick 1: check_migration_boundaries_system should detect x = 0.6 > 0.5.
    // It should despawn the agent and send outbound migration.
    schedule.run(&mut world);

    // Let tokio scheduler yield so that websocket client can process the outbound migration, fail, and bounce back.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // After bounce back, inbound channel should have the bounced agent, and process_inbound_migrations_system should spawn it.
    schedule.run(&mut world);

    // Verify it spawned at x = bounds_max - offset = 0.5 - 0.05 = 0.45
    let mut query = world.query::<(&Position, &Velocity)>();
    let results: Vec<_> = query.iter(&world).collect();
    assert_eq!(results.len(), 1);
    let (pos, vel) = results[0];
    assert_eq!(pos.0.x, 0.45);
    assert!(vel.0.x < 0.0); // velocity reversed

    // Tick 2: check_migration_boundaries_system runs. It should detect that the agent is inside boundaries (0.45).
    // It should not despawn or re-migrate the agent.
    schedule.run(&mut world);

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify the agent remains inside boundaries at 0.45, was not despawned, and inbound queue is empty.
    let results: Vec<_> = query.iter(&world).collect();
    assert_eq!(results.len(), 1);
    let (pos, vel) = results[0];
    assert_eq!(pos.0.x, 0.45);
    assert!(vel.0.x < 0.0);

    {
        let inbound_rec = world.get_resource::<InboundMigrationReceiver>().unwrap();
        assert!(inbound_rec.0.is_empty(), "Inbound queue should be empty; no second migration should have occurred");
        let outbound_rec = world.get_resource::<OutboundMigrationSender>().unwrap();
        assert!(outbound_rec.0.is_empty(), "Outbound queue should be empty; no further migrations should have been triggered");
    }

    // Tick 3: check_migration_boundaries_system runs again.
    schedule.run(&mut world);

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify again that the agent remains inside boundaries at 0.45 and inbound queue is empty.
    let results: Vec<_> = query.iter(&world).collect();
    assert_eq!(results.len(), 1);
    let (pos, vel) = results[0];
    assert_eq!(pos.0.x, 0.45);
    assert!(vel.0.x < 0.0);

    {
        let inbound_rec = world.get_resource::<InboundMigrationReceiver>().unwrap();
        assert!(inbound_rec.0.is_empty(), "Inbound queue should still be empty; no loop should occur");
        let outbound_rec = world.get_resource::<OutboundMigrationSender>().unwrap();
        assert!(outbound_rec.0.is_empty(), "Outbound queue should still be empty; no loop should occur");
    }

    running.store(false, Ordering::SeqCst);
    let _ = client_handle.await;
}
