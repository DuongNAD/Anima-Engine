mod common;

use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use bevy_ecs::prelude::*;
use glam::Vec3;

use anima_engine_lib::core::ecs::{
    AgentMigrationData, ShardingConfig, OutboundMigration, InboundMigrationReceiver,
    OutboundMigrationSender, ShardingResource, check_migration_boundaries_system,
    process_inbound_migrations_system, AgentParentLineageIds, FeatureTracker, Velocity,
    Position, Rotation, ParentAgent, Segment, Prey, AgentClass, Agent,
    MapBounds, ChildrenLinks
};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::evolution::genotype::{MorphologyGenotype, MorphologyNode};
use anima_engine_lib::core::engine::{
    AgentGenotype, AgentEvaluation, AgentLineageId, AgentGeneration,
    run_websocket_server, run_websocket_client
};

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator = common::allocator::TrackingAllocator::new();

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[tokio::test]
async fn test_stress_high_throughput_websocket_transfers() {
    let _lock = TEST_LOCK.lock().unwrap();

    let port = 8095;
    let (server_inbound_tx, server_inbound_rx) = crossbeam_channel::unbounded();
    let (client_inbound_tx, _client_inbound_rx) = crossbeam_channel::unbounded();
    let (outbound_tx, outbound_rx) = crossbeam_channel::unbounded();

    let running = Arc::new(AtomicBool::new(true));

    let running_server = Arc::clone(&running);
    let server_handle = tokio::spawn(async move {
        run_websocket_server::<tauri::test::MockRuntime>(
            port,
            server_inbound_tx,
            running_server,
            None,
        ).await;
    });

    let running_client = Arc::clone(&running);
    let client_handle = tokio::spawn(async move {
        run_websocket_client::<tauri::test::MockRuntime>(
            outbound_rx,
            client_inbound_tx,
            running_client,
            None,
            8080,
        ).await;
    });

    // Let the server start
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Send 200 high-throughput migrations concurrently
    let count = 200;
    let mut join_handles = vec![];
    for i in 0..count {
        let tx = outbound_tx.clone();
        let handle = tokio::spawn(async move {
            let mut genotype = MorphologyGenotype::new();
            genotype.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.2, mass: 1.5 });
            let agent = AgentMigrationData {
                genotype,
                homeostatic_state: HomeostaticState {
                    energy: 100.0,
                    energy_target: 100.0,
                    hydration: 100.0,
                    hydration_target: 100.0,
                    temperature: 37.0,
                    temp_target: 37.0,
                    previous_deviation: 0.0,
                },
                position: Vec3::new(i as f32, 0.0, 0.0),
                velocity: Vec3::new(1.0, 0.0, 0.0),
                lineage_id: format!("ht-lineage-{}", i),
                generation: i,
                agent_class: AgentClass::Prey,
                parent_ids: vec![],
                evaluation: None,
                feature_tracker: None,
                last_transition_state: None,
                source_port: 0,
            };
            tx.send(OutboundMigration {
                target_port: port,
                data: agent,
                bounds_min_x: -100.0,
                bounds_max_x: 100.0,
            }).unwrap();
        });
        join_handles.push(handle);
    }

    for h in join_handles {
        h.await.unwrap();
    }

    // Wait and verify we receive all 200 on the server side
    let received_count = tokio::time::timeout(Duration::from_secs(10), async {
        let mut got = 0;
        loop {
            while let Ok(_) = server_inbound_rx.try_recv() {
                got += 1;
            }
            if got >= count {
                return got;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("Timeout waiting for 200 parallel migrations");

    assert_eq!(received_count, count);

    running.store(false, Ordering::SeqCst);
    let _ = tokio::join!(server_handle, client_handle);
}

#[tokio::test]
async fn test_closed_port_bounce_back_custom_boundaries() {
    let _lock = TEST_LOCK.lock().unwrap();

    let genotype = {
        let mut g = MorphologyGenotype::new();
        g.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.2, mass: 1.5 });
        g
    };

    let agent = AgentMigrationData {
        genotype,
        homeostatic_state: HomeostaticState {
            energy: 80.0,
            energy_target: 100.0,
            hydration: 80.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
        position: Vec3::new(205.0, 0.0, 0.0),
        velocity: Vec3::new(5.0, 0.0, 0.0),
        lineage_id: "bounce-back-lineage".to_string(),
        generation: 1,
        agent_class: AgentClass::Prey,
        parent_ids: vec![],
        evaluation: None,
        feature_tracker: None,
        last_transition_state: None,
        source_port: 0,
    };

    // Client closed-port bounce-back check with custom map boundaries
    let (inbound_tx, inbound_rx) = crossbeam_channel::unbounded();
    let (outbound_tx, outbound_rx) = crossbeam_channel::unbounded();
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

    // Send a message targeting a closed port (9999) with custom bounds min: -200.0, max: 200.0
    outbound_tx.send(OutboundMigration {
        target_port: 9999,
        data: agent.clone(),
        bounds_min_x: -200.0,
        bounds_max_x: 200.0,
    }).unwrap();

    // Verify bounce back
    let bounced = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(bounced_data) = inbound_rx.try_recv() {
                return bounced_data;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("Timeout waiting for bounce back");

    assert_eq!(bounced.lineage_id, agent.lineage_id);
    // Bounced back velocity should be negative and position flipped inward based on max boundary 200.0
    assert!(bounced.velocity.x < 0.0);
    // position should be max - 1.0 = 199.0
    assert_eq!(bounced.position.x, 199.0);

    // Now test a negative velocity outbound migration (moving left, x < min_x)
    let left_outbound_agent = AgentMigrationData {
        position: Vec3::new(-205.0, 0.0, 0.0),
        velocity: Vec3::new(-5.0, 0.0, 0.0),
        ..agent.clone()
    };

    outbound_tx.send(OutboundMigration {
        target_port: 9999,
        data: left_outbound_agent,
        bounds_min_x: -200.0,
        bounds_max_x: 200.0,
    }).unwrap();

    let bounced_left = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(bounced_data) = inbound_rx.try_recv() {
                return bounced_data;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("Timeout waiting for left bounce back");

    // Bounced back velocity should be positive and position flipped inward based on min boundary -200.0
    assert!(bounced_left.velocity.x > 0.0);
    // position should be min + 1.0 = -199.0
    assert_eq!(bounced_left.position.x, -199.0);

    running.store(false, Ordering::SeqCst);
    let _ = client_handle.await;
}

#[test]
fn test_migration_systems_zero_heap_allocations_on_hot_path() {
    let _lock = TEST_LOCK.lock().unwrap();

    let mut world = World::new();

    let (outbound_tx, _outbound_rx) = crossbeam_channel::unbounded();
    world.insert_resource(OutboundMigrationSender(outbound_tx));

    let sharding_config = Arc::new(RwLock::new(ShardingConfig {
        local_port: 8080,
        right_target_port: Some(8081),
        left_target_port: None,
    }));
    world.insert_resource(ShardingResource(sharding_config));

    // Custom map bounds
    let bounds = MapBounds {
        min: Vec3::new(-150.0, 0.0, -150.0),
        max: Vec3::new(150.0, 10.0, 150.0),
    };
    world.insert_resource(bounds);

    let (inbound_tx, inbound_rx) = crossbeam_channel::unbounded();
    world.insert_resource(InboundMigrationReceiver(inbound_rx));

    // Spawn 10 agents and their child segments inside the map boundaries (so no migrations are triggered)
    for i in 0..10 {
        let initial_pos = Vec3::new((i as f32) * 10.0 - 50.0, 0.0, 0.0);
        let segment_entity = world.spawn((
            ParentAgent(Entity::PLACEHOLDER),
            Position(initial_pos),
            Rotation(glam::Quat::IDENTITY),
            Velocity(Vec3::ZERO),
            Segment { id: 0, length: 1.0, radius: 0.2, mass: 1.0 },
            ChildrenLinks(Vec::new()),
        )).id();

        let genotype = MorphologyGenotype::new();
        let agent_entity = world.spawn((
            Agent,
            Position(initial_pos),
            Rotation(glam::Quat::IDENTITY),
            Velocity(Vec3::ZERO),
            AgentGenotype(genotype),
            HomeostaticState {
                energy: 100.0,
                energy_target: 100.0,
                hydration: 100.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
            AgentLineageId(format!("hot-path-lineage-{}", i)),
            AgentGeneration(0),
            AgentParentLineageIds(vec![]),
            Prey,
            ChildrenLinks(vec![segment_entity]),
        )).id();

        world.entity_mut(segment_entity).insert(ParentAgent(agent_entity));
    }

    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    schedule.add_systems((
        check_migration_boundaries_system,
        process_inbound_migrations_system,
    ));

    // Warm-up to initialize internal Bevy structures
    for _ in 0..50 {
        schedule.run(&mut world);
    }

    // Start tracking allocations
    ALLOCATOR.start_tracking();

    // Run systems on hot path
    for _ in 0..100 {
        schedule.run(&mut world);
    }

    // Stop tracking
    let allocations = ALLOCATOR.stop_tracking();

    // Assert zero heap allocations on hot path
    assert_eq!(
        allocations, 0,
        "Expected 0 allocations on hot path for migration systems, but recorded {}",
        allocations
    );
}
