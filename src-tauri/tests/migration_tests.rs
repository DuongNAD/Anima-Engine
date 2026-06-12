use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use bevy_ecs::prelude::*;
use glam::Vec3;

use anima_engine_lib::core::ecs::{
    AgentMigrationData, ShardingConfig, OutboundMigration, InboundMigrationReceiver,
    OutboundMigrationSender, ShardingResource, check_migration_boundaries_system,
    process_inbound_migrations_system, AgentParentLineageIds, FeatureTracker, Velocity,
    Position, Rotation, ParentAgent, Segment, SegmentJointForce, Prey, AgentClass, EpochManager, Agent,
    MapBounds, ChildrenLinks
};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::evolution::genotype::{MorphologyGenotype, MorphologyNode};
use anima_engine_lib::core::engine::{
    AgentGenotype, AgentEvaluation, AgentLineageId, AgentGeneration,
    EvolutionSender, EvolutionReceiver, EvolutionQueue,
    run_websocket_server, run_websocket_client
};

#[tokio::test]
async fn test_agent_migration_serialization_and_resilience() {
    let genotype = {
        let mut g = MorphologyGenotype::new();
        g.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.2, mass: 1.5 });
        g
    };

    let agent = AgentMigrationData {
        genotype,
        homeostatic_state: HomeostaticState {
            energy: 88.2,
            energy_target: 100.0,
            hydration: 70.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
        position: Vec3::new(10.5, 0.0, -12.3),
        velocity: Vec3::new(1.0, 0.0, 0.0),
        lineage_id: "test-resilience-lineage".to_string(),
        generation: 42,
        agent_class: AgentClass::Prey,
        parent_ids: vec!["parent-1".to_string()],
        evaluation: Some(AgentEvaluation {
            start_position: Vec3::new(10.5, 0.0, -12.3),
            total_distance: 5.0,
            total_energy_expended: 12.0,
            survival_ticks: 100,
            last_position: Vec3::new(15.5, 0.0, -12.3),
        }),
        feature_tracker: Some(FeatureTracker {
            cumulative_distance: 5.0,
            cumulative_energy_decay: 12.0,
            tick_count: 100,
        }),
        last_transition_state: Some(anima_engine_lib::ai::hrrl::LastTransitionState {
            state: [1.0; 15],
            action: [2.0; 4],
            has_last: true,
        }),
        source_port: 0,
    };

    // Serialization / Deserialization check
    let serialized = serde_json::to_string(&agent).unwrap();
    let deserialized: AgentMigrationData = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.lineage_id, agent.lineage_id);
    assert_eq!(deserialized.generation, agent.generation);
    assert_eq!(deserialized.parent_ids, agent.parent_ids);
    assert_eq!(deserialized.homeostatic_state.energy, agent.homeostatic_state.energy);
    assert!(deserialized.evaluation.is_some());
    assert!(deserialized.feature_tracker.is_some());
    assert!(deserialized.last_transition_state.is_some());

    // Client closed-port bounce-back check
    let (inbound_tx, inbound_rx) = crossbeam_channel::unbounded();
    let (outbound_tx, outbound_rx) = crossbeam_channel::unbounded();
    let running = Arc::new(AtomicBool::new(true));

    let running_clone = Arc::clone(&running);
    let inbound_tx_clone = inbound_tx.clone();
    
    // Start run_websocket_client
    let client_handle = tokio::spawn(async move {
        run_websocket_client::<tauri::test::MockRuntime>(
            outbound_rx,
            inbound_tx_clone,
            running_clone,
            None,
            8080,
        ).await;
    });

    // Send a message targeting a closed port (9999)
    outbound_tx.send(OutboundMigration {
        target_port: 9999,
        data: agent.clone(),
        bounds_min_x: -100.0,
        bounds_max_x: 100.0,
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
    // Bounced back velocity should be negative and position flipped inward
    assert!(bounced.velocity.x < 0.0);
    assert!(bounced.position.x < 100.0);

    running.store(false, Ordering::SeqCst);
    let _ = client_handle.await;
}

#[tokio::test]
async fn test_migration_tier1_ports_8080_to_8081() {
    let genotype = {
        let mut g = MorphologyGenotype::new();
        g.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.2, mass: 1.5 });
        g
    };

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
        position: Vec3::new(10.0, 0.0, 0.0),
        velocity: Vec3::new(1.0, 2.0, 3.0),
        lineage_id: "tier1-lineage".to_string(),
        generation: 1,
        agent_class: AgentClass::Prey,
        parent_ids: vec![],
        evaluation: None,
        feature_tracker: None,
        last_transition_state: None,
        source_port: 0,
    };

    let (server_inbound_tx, server_inbound_rx) = crossbeam_channel::unbounded();
    let (client_inbound_tx, _client_inbound_rx) = crossbeam_channel::unbounded();
    let (outbound_tx, outbound_rx) = crossbeam_channel::unbounded();
    
    let running = Arc::new(AtomicBool::new(true));

    let running_server = Arc::clone(&running);
    let server_handle = tokio::spawn(async move {
        run_websocket_server::<tauri::test::MockRuntime>(
            8091,
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

    // Give server a bit of time to start up
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Send the agent to the client
    outbound_tx.send(OutboundMigration {
        target_port: 8091,
        data: agent.clone(),
        bounds_min_x: -100.0,
        bounds_max_x: 100.0,
    }).unwrap();

    // Verify server receives it
    let received = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(data) = server_inbound_rx.try_recv() {
                return data;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("Timeout waiting for migration to be received by server");

    assert_eq!(received.lineage_id, agent.lineage_id);
    assert_eq!(received.generation, agent.generation);

    running.store(false, Ordering::SeqCst);
    let _ = tokio::join!(server_handle, client_handle);
}

#[test]
fn test_migration_tier2_boundaries_and_serialization_failures() {
    let mut world = World::new();

    let (outbound_tx, outbound_rx) = crossbeam_channel::unbounded();
    world.insert_resource(OutboundMigrationSender(outbound_tx));

    let sharding_config = Arc::new(RwLock::new(ShardingConfig {
        local_port: 8080,
        right_target_port: Some(8081),
        left_target_port: None,
    }));
    world.insert_resource(ShardingResource(sharding_config));
    world.insert_resource(MapBounds::default());

    // Spawn segment entity associated with the agent
    let segment_entity = world.spawn((
        ParentAgent(Entity::PLACEHOLDER), // will be updated or just placeholder
        Position(Vec3::new(105.0, 0.0, 0.0)),
        Rotation(glam::Quat::IDENTITY),
        Velocity(Vec3::new(1.0, 0.0, 0.0)),
        Segment { id: 0, length: 1.0, radius: 0.2, mass: 1.0 },
        ChildrenLinks(Vec::new()),
    )).id();

    // Spawn an agent at x = 105.0 moving right (vx > 0.0) -> triggers outbound migration
    let genotype = MorphologyGenotype::new();
    let agent_entity = world.spawn((
        Agent,
        Position(Vec3::new(105.0, 0.0, 0.0)),
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
        AgentLineageId("boundary-lineage-id".to_string()),
        AgentGeneration(3),
        AgentParentLineageIds(vec!["p1".to_string()]),
        Prey,
        ChildrenLinks(vec![segment_entity]),
    )).id();

    // Update parent agent link
    world.entity_mut(segment_entity).insert(ParentAgent(agent_entity));

    let mut schedule = Schedule::default();
    schedule.add_systems(check_migration_boundaries_system);
    schedule.run(&mut world);

    // Verify agent is despawned (both parent and segment)
    assert!(world.get_entity(agent_entity).is_none());
    assert!(world.get_entity(segment_entity).is_none());

    // Verify outbound channel has the migration package
    let outbound = outbound_rx.try_recv().expect("Should have sent outbound migration");
    assert_eq!(outbound.target_port, 8081);
    assert_eq!(outbound.data.lineage_id, "boundary-lineage-id");
    assert_eq!(outbound.data.generation, 3);
    assert_eq!(outbound.data.parent_ids, vec!["p1".to_string()]);
    assert_eq!(outbound.data.position, Vec3::new(-95.0, 0.0, 0.0));
    assert_eq!(outbound.data.velocity, Vec3::new(1.0, 0.0, 0.0));
}

#[test]
fn test_migration_tier3_lineage_integration() {
    let mut world = World::new();

    let (inbound_tx, inbound_rx) = crossbeam_channel::unbounded();
    world.insert_resource(InboundMigrationReceiver(inbound_rx));

    let genotype = {
        let mut g = MorphologyGenotype::new();
        g.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.2, mass: 1.0 });
        g
    };

    let data = AgentMigrationData {
        genotype: genotype.clone(),
        homeostatic_state: HomeostaticState {
            energy: 90.0,
            energy_target: 100.0,
            hydration: 90.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
        position: Vec3::new(5.0, 2.0, 3.0),
        velocity: Vec3::new(-1.0, 0.0, 0.0),
        lineage_id: "inbound-lineage-id".to_string(),
        generation: 5,
        agent_class: AgentClass::Prey,
        parent_ids: vec!["parent-a".to_string(), "parent-b".to_string()],
        evaluation: None,
        feature_tracker: None,
        last_transition_state: None,
        source_port: 0,
    };

    inbound_tx.send(data).unwrap();

    let mut schedule = Schedule::default();
    schedule.add_systems(process_inbound_migrations_system);
    schedule.run(&mut world);

    // Verify spawned agent in world
    let mut query = world.query::<(
        &Position,
        &Velocity,
        &AgentGenotype,
        &AgentLineageId,
        &AgentGeneration,
        &AgentParentLineageIds,
        &HomeostaticState
    )>();

    let results: Vec<_> = query.iter(&world).collect();
    assert_eq!(results.len(), 1);

    let (pos, vel, gen, lineage, generation, parents, homeo) = results[0];
    assert_eq!(pos.0, Vec3::new(5.0, 2.0, 3.0));
    assert_eq!(vel.0, Vec3::new(-1.0, 0.0, 0.0));
    assert_eq!(lineage.0, "inbound-lineage-id");
    assert_eq!(generation.0, 5);
    assert_eq!(parents.0, vec!["parent-a".to_string(), "parent-b".to_string()]);
    assert_eq!(homeo.energy, 90.0);
}

#[tokio::test]
async fn test_migration_tier4_parallel_workload() {
    let port = 8092;
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

    // Send 10 parallel migrations
    let count = 10;
    let mut join_handles = vec![];
    for i in 0..count {
        let tx = outbound_tx.clone();
        let handle = tokio::spawn(async move {
            let genotype = MorphologyGenotype::new();
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
                position: Vec3::ZERO,
                velocity: Vec3::ZERO,
                lineage_id: format!("parallel-lineage-{}", i),
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

    // Wait and verify we receive all 10 on the server side
    let received_count = tokio::time::timeout(Duration::from_secs(5), async {
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
    }).await.expect("Timeout waiting for parallel migrations");

    assert_eq!(received_count, count);

    running.store(false, Ordering::SeqCst);
    let _ = tokio::join!(server_handle, client_handle);
}
