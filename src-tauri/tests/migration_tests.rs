// Cross-shard WebSocket migration lives behind the `networking` feature (G2). Without it this
// suite has nothing to exercise, so the whole file compiles away rather than failing to link.
#![cfg(feature = "networking")]

#[path = "support/network_ready.rs"]
mod network_ready;

use bevy_ecs::prelude::*;
use futures_util::{SinkExt, StreamExt};
use glam::Vec3;
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::ecs::{
    check_migration_boundaries_system, process_inbound_migrations_system, Agent, AgentBrain,
    AgentClass, AgentMigrationData, AgentParentLineageIds, ChildrenLinks, FeatureTracker,
    InboundMigrationReceiver, MapBounds, OutboundMigration, OutboundMigrationSender, ParentAgent,
    Position, Prey, Rotation, Segment, ShardingConfig, ShardingResource, Velocity,
};
use anima_engine_lib::core::engine::{
    run_websocket_client, run_websocket_server, run_websocket_server_with_diagnostics,
    AgentEvaluation, AgentGeneration, AgentGenotype, AgentLineageId,
    MAX_MIGRATION_WEBSOCKET_MESSAGE_BYTES,
};
use anima_engine_lib::core::resources::MigrationHandoffDiagnostics;
use anima_engine_lib::evolution::brain_genotype::{ArchSpec, BrainGenotype};
use anima_engine_lib::evolution::genotype::{MorphologyGenotype, MorphologyNode};

fn one_node_genotype() -> MorphologyGenotype {
    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.2,
        mass: 1.0,
    });
    genotype
}

fn migration_payload(genotype: MorphologyGenotype) -> AgentMigrationData {
    AgentMigrationData {
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
        velocity: Vec3::X,
        lineage_id: "network-validation".to_owned(),
        generation: 42,
        agent_class: AgentClass::Prey,
        parent_ids: vec!["parent-1".to_owned()],
        evaluation: None,
        feature_tracker: None,
        last_transition_state: None,
        source_port: 8080,
        brain: None,
    }
}

#[test]
fn largest_valid_dual_brain_payload_fits_the_transport_limit() {
    let arch = ArchSpec::new(72, 72, 72);
    let parameters = arch.checked_param_count().unwrap();
    assert!(parameters <= anima_engine_lib::core::ecs::MAX_MIGRATION_BRAIN_PARAMETERS);
    let inherited = BrainGenotype::from_weights(arch, vec![f32::MAX; parameters]).unwrap();
    let learned = BrainGenotype::from_weights(arch, vec![-f32::MAX; parameters]).unwrap();
    let mut brain = AgentBrain::from_genotype(inherited);
    brain.set_learned(learned);
    let mut payload = migration_payload(one_node_genotype());
    payload.brain = Some(brain);
    payload.validate().expect("boundary payload must be valid");

    let encoded = serde_json::to_vec(&payload).expect("serialize worst-case finite weights");
    assert!(
        encoded.len() <= MAX_MIGRATION_WEBSOCKET_MESSAGE_BYTES,
        "{} bytes exceed the {}-byte transport limit",
        encoded.len(),
        MAX_MIGRATION_WEBSOCKET_MESSAGE_BYTES
    );
}

#[tokio::test]
async fn websocket_server_rejects_invalid_scientific_state_before_the_inbound_queue() {
    let reservation = TcpListener::bind("127.0.0.1:0").expect("reserve a test port");
    let port = reservation.local_addr().expect("reserved address").port();
    drop(reservation);

    let (inbound_tx, inbound_rx) = crossbeam_channel::unbounded();
    let running = Arc::new(AtomicBool::new(true));
    let diagnostics = MigrationHandoffDiagnostics::default();
    let server_running = Arc::clone(&running);
    let server_diagnostics = diagnostics.clone();
    let server = tokio::spawn(async move {
        run_websocket_server_with_diagnostics::<tauri::test::MockRuntime>(
            port,
            inbound_tx,
            server_running,
            None,
            server_diagnostics,
        )
        .await
    });

    drop(network_ready::connect_when_ready(port).await);
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
        .await
        .expect("connect to migration server");
    let invalid = serde_json::to_string(&migration_payload(MorphologyGenotype::new())).unwrap();
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(invalid))
        .await
        .expect("send invalid migration payload");
    let acknowledgement = tokio::time::timeout(Duration::from_secs(2), socket.next())
        .await
        .expect("server must acknowledge rejection")
        .expect("server must keep the socket until acknowledgement")
        .expect("acknowledgement frame must be readable");
    assert_eq!(
        acknowledgement.to_text().unwrap(),
        anima_engine_lib::core::engine::MIGRATION_ACK_REJECTED
    );
    let _ = socket.close(None).await;

    tokio::time::timeout(Duration::from_secs(2), async {
        while diagnostics.snapshot().invalid_rejections == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("server must reject the invalid payload by the liveness deadline");

    assert!(inbound_rx.try_recv().is_err());
    assert_eq!(diagnostics.snapshot().invalid_rejections, 1);

    let (mut binary_socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
        .await
        .expect("connect binary-framed legacy peer");
    let valid = serde_json::to_vec(&migration_payload(one_node_genotype())).unwrap();
    binary_socket
        .send(tokio_tungstenite::tungstenite::Message::Binary(valid))
        .await
        .expect("send valid binary-framed migration payload");
    let acknowledgement = tokio::time::timeout(Duration::from_secs(2), binary_socket.next())
        .await
        .expect("server must acknowledge binary payload")
        .expect("server must retain socket for binary acknowledgement")
        .expect("binary acknowledgement must be readable");
    assert_eq!(
        acknowledgement.to_text().unwrap(),
        anima_engine_lib::core::engine::MIGRATION_ACK_ACCEPTED
    );
    assert_eq!(
        inbound_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("binary-framed payload must retain compatibility")
            .lineage_id,
        "network-validation"
    );
    let _ = binary_socket.close(None).await;

    running.store(false, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .expect("server must stop")
        .expect("server task must not panic")
        .expect("server must exit cleanly");
}

#[tokio::test]
async fn websocket_server_applies_lossless_backpressure_when_the_inbound_queue_is_full() {
    let reservation = TcpListener::bind("127.0.0.1:0").expect("reserve a test port");
    let port = reservation.local_addr().expect("reserved address").port();
    drop(reservation);

    let (inbound_tx, stalled_receiver) = crossbeam_channel::bounded(1);
    inbound_tx
        .try_send(migration_payload(one_node_genotype()))
        .expect("pre-fill the single queue slot");
    let running = Arc::new(AtomicBool::new(true));
    let diagnostics = MigrationHandoffDiagnostics::default();
    let server_running = Arc::clone(&running);
    let server_diagnostics = diagnostics.clone();
    let server = tokio::spawn(async move {
        run_websocket_server_with_diagnostics::<tauri::test::MockRuntime>(
            port,
            inbound_tx,
            server_running,
            None,
            server_diagnostics,
        )
        .await
    });

    drop(network_ready::connect_when_ready(port).await);
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
        .await
        .expect("connect to migration server");
    let valid = serde_json::to_string(&migration_payload(one_node_genotype())).unwrap();
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(valid))
        .await
        .expect("send valid migration payload");
    let _ = socket.close(None).await;

    tokio::time::timeout(Duration::from_secs(2), async {
        while diagnostics.snapshot().inbound_backpressure_events == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("a full inbound queue must apply asynchronous backpressure");
    let first = stalled_receiver
        .recv()
        .expect("drain the pre-filled payload");
    assert_eq!(first.lineage_id, "network-validation");
    let retained = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(data) = stalled_receiver.try_recv() {
                break data;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the server task must retain and eventually enqueue the accepted payload");
    assert_eq!(retained.lineage_id, "network-validation");
    assert_eq!(diagnostics.snapshot().inbound_backpressure_events, 1);

    running.store(false, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .expect("server must stop")
        .expect("server task must not panic")
        .expect("server must exit cleanly");
}

#[tokio::test]
async fn websocket_server_rejects_oversized_frames_before_json_allocation_and_stays_live() {
    let reservation = TcpListener::bind("127.0.0.1:0").expect("reserve a test port");
    let port = reservation.local_addr().expect("reserved address").port();
    drop(reservation);

    let (inbound_tx, inbound_rx) = crossbeam_channel::unbounded();
    let running = Arc::new(AtomicBool::new(true));
    let diagnostics = MigrationHandoffDiagnostics::default();
    let server_running = Arc::clone(&running);
    let server_diagnostics = diagnostics.clone();
    let server = tokio::spawn(async move {
        run_websocket_server_with_diagnostics::<tauri::test::MockRuntime>(
            port,
            inbound_tx,
            server_running,
            None,
            server_diagnostics,
        )
        .await
    });

    drop(network_ready::connect_when_ready(port).await);
    let (mut oversized_socket, _) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .expect("connect oversized sender");
    let oversized = "x".repeat(MAX_MIGRATION_WEBSOCKET_MESSAGE_BYTES + 1);
    let _ = oversized_socket
        .send(tokio_tungstenite::tungstenite::Message::Text(oversized))
        .await;

    tokio::time::timeout(Duration::from_secs(2), async {
        while diagnostics.snapshot().invalid_rejections == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("oversized frame must be refused by the transport limit");
    assert!(inbound_rx.try_recv().is_err());

    let (mut valid_socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
        .await
        .expect("server must accept a fresh connection after rejecting an oversized frame");
    let valid = serde_json::to_string(&migration_payload(one_node_genotype())).unwrap();
    valid_socket
        .send(tokio_tungstenite::tungstenite::Message::Text(valid))
        .await
        .expect("send valid migration payload");
    let _ = valid_socket.close(None).await;
    tokio::time::timeout(Duration::from_secs(2), async {
        while inbound_rx.is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("server must remain live after refusing the oversized peer");
    assert!(inbound_rx.try_recv().is_ok());

    running.store(false, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .expect("server must stop")
        .expect("server task must not panic")
        .expect("server must exit cleanly");
}

#[tokio::test]
async fn websocket_rejection_acknowledgement_returns_ownership_to_the_source_shard() {
    let reservation = TcpListener::bind("127.0.0.1:0").expect("reserve a test port");
    let port = reservation.local_addr().expect("reserved address").port();
    drop(reservation);

    let (server_inbound_tx, disconnected_server_receiver) = crossbeam_channel::bounded(1);
    drop(disconnected_server_receiver);
    let (client_inbound_tx, client_inbound_rx) = crossbeam_channel::bounded(1);
    let (outbound_tx, outbound_rx) = crossbeam_channel::bounded(1);
    let running = Arc::new(AtomicBool::new(true));

    let server_running = Arc::clone(&running);
    let server = tokio::spawn(async move {
        run_websocket_server_with_diagnostics::<tauri::test::MockRuntime>(
            port,
            server_inbound_tx,
            server_running,
            None,
            MigrationHandoffDiagnostics::default(),
        )
        .await
    });
    drop(network_ready::connect_when_ready(port).await);

    let client_running = Arc::clone(&running);
    let client = tokio::spawn(async move {
        run_websocket_client::<tauri::test::MockRuntime>(
            outbound_rx,
            client_inbound_tx,
            client_running,
            None,
            8080,
        )
        .await;
    });

    let mut data = migration_payload(one_node_genotype());
    data.position = Vec3::new(100.5, 2.0, 3.0);
    data.velocity = Vec3::new(4.0, 5.0, 6.0);
    outbound_tx
        .send(OutboundMigration {
            target_port: port,
            data,
            bounds_min_x: -100.0,
            bounds_max_x: 100.0,
        })
        .unwrap();

    let returned = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(data) = client_inbound_rx.try_recv() {
                break data;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("a rejected transfer must return the only authoritative payload");
    assert_eq!(returned.position.x, 99.0);
    assert_eq!(returned.velocity.x, -4.0);

    running.store(false, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .expect("server must stop")
        .expect("server task must not panic")
        .expect("server must exit cleanly");
    tokio::time::timeout(Duration::from_secs(2), client)
        .await
        .expect("client must stop")
        .expect("client task must not panic");
}

#[tokio::test]
async fn sustained_inbound_backpressure_rejects_before_the_source_ack_deadline() {
    let reservation = TcpListener::bind("127.0.0.1:0").expect("reserve a test port");
    let port = reservation.local_addr().expect("reserved address").port();
    drop(reservation);

    let (server_inbound_tx, server_inbound_rx) = crossbeam_channel::bounded(1);
    server_inbound_tx
        .send(migration_payload(one_node_genotype()))
        .expect("pre-fill target queue");
    let (client_inbound_tx, client_inbound_rx) = crossbeam_channel::bounded(1);
    let (outbound_tx, outbound_rx) = crossbeam_channel::bounded(1);
    let running = Arc::new(AtomicBool::new(true));

    let server_running = Arc::clone(&running);
    let server = tokio::spawn(async move {
        run_websocket_server_with_diagnostics::<tauri::test::MockRuntime>(
            port,
            server_inbound_tx,
            server_running,
            None,
            MigrationHandoffDiagnostics::default(),
        )
        .await
    });
    drop(network_ready::connect_when_ready(port).await);

    let client_running = Arc::clone(&running);
    let client = tokio::spawn(async move {
        run_websocket_client::<tauri::test::MockRuntime>(
            outbound_rx,
            client_inbound_tx,
            client_running,
            None,
            8080,
        )
        .await;
    });

    let mut data = migration_payload(one_node_genotype());
    data.position = Vec3::new(100.5, 0.0, 0.0);
    data.velocity = Vec3::new(2.0, 0.0, 0.0);
    outbound_tx
        .send(OutboundMigration {
            target_port: port,
            data,
            bounds_min_x: -100.0,
            bounds_max_x: 100.0,
        })
        .unwrap();

    let returned = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(data) = client_inbound_rx.try_recv() {
                break data;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("target must reject before the source's 500 ms acknowledgement deadline");
    assert_eq!(returned.position.x, 99.0);
    assert_eq!(returned.velocity.x, -2.0);
    assert_eq!(
        server_inbound_rx.len(),
        1,
        "the rejected payload must not enter the full target queue"
    );

    running.store(false, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .expect("server must stop")
        .expect("server task must not panic")
        .expect("server must exit cleanly");
    tokio::time::timeout(Duration::from_secs(2), client)
        .await
        .expect("client must stop")
        .expect("client task must not panic");
}

#[tokio::test]
async fn test_agent_migration_serialization_and_resilience() {
    let genotype = {
        let mut g = MorphologyGenotype::new();
        g.add_node(MorphologyNode {
            id: 0,
            length: 1.0,
            radius: 0.2,
            mass: 1.5,
        });
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
            pending_state: None,
        }),
        source_port: 0,
        brain: None,
    };

    // Serialization / Deserialization check
    let serialized = serde_json::to_string(&agent).unwrap();
    let deserialized: AgentMigrationData = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.lineage_id, agent.lineage_id);
    assert_eq!(deserialized.generation, agent.generation);
    assert_eq!(deserialized.parent_ids, agent.parent_ids);
    assert_eq!(
        deserialized.homeostatic_state.energy,
        agent.homeostatic_state.energy
    );
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
        )
        .await;
    });

    // Send a message targeting a closed port (9999)
    outbound_tx
        .send(OutboundMigration {
            target_port: 9999,
            data: agent.clone(),
            bounds_min_x: -100.0,
            bounds_max_x: 100.0,
        })
        .unwrap();

    // Verify bounce back
    let bounced = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(bounced_data) = inbound_rx.try_recv() {
                return bounced_data;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Timeout waiting for bounce back");

    assert_eq!(bounced.lineage_id, agent.lineage_id);
    // Bounced back velocity should be negative and position flipped inward
    assert!(bounced.velocity.x < 0.0);
    assert!(bounced.position.x < 100.0);

    running.store(false, Ordering::SeqCst);
    let _ = client_handle.await;
}

#[tokio::test]
async fn test_migration_tier1_ports_8080_to_8081() {
    let reservation = TcpListener::bind("127.0.0.1:0").expect("reserve a test port");
    let target_port = reservation.local_addr().expect("reserved address").port();
    drop(reservation);

    let genotype = {
        let mut g = MorphologyGenotype::new();
        g.add_node(MorphologyNode {
            id: 0,
            length: 1.0,
            radius: 0.2,
            mass: 1.5,
        });
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
        brain: None,
    };

    let (server_inbound_tx, server_inbound_rx) = crossbeam_channel::unbounded();
    let (client_inbound_tx, _client_inbound_rx) = crossbeam_channel::unbounded();
    let (outbound_tx, outbound_rx) = crossbeam_channel::unbounded();

    let running = Arc::new(AtomicBool::new(true));

    let running_server = Arc::clone(&running);
    let server_handle = tokio::spawn(async move {
        // The server returns a Result on exit; the test asserts on the channel traffic instead, so
        // discard it explicitly rather than leaving a must_use dangling.
        let _ = run_websocket_server::<tauri::test::MockRuntime>(
            target_port,
            server_inbound_tx,
            running_server,
            None,
        )
        .await;
    });

    let running_client = Arc::clone(&running);
    let client_handle = tokio::spawn(async move {
        run_websocket_client::<tauri::test::MockRuntime>(
            outbound_rx,
            client_inbound_tx,
            running_client,
            None,
            8080,
        )
        .await;
    });

    drop(network_ready::connect_when_ready(target_port).await);

    // Send the agent to the client
    outbound_tx
        .send(OutboundMigration {
            target_port,
            data: agent.clone(),
            bounds_min_x: -100.0,
            bounds_max_x: 100.0,
        })
        .unwrap();

    // Verify server receives it
    let received = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(data) = server_inbound_rx.try_recv() {
                return data;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Timeout waiting for migration to be received by server");

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
    let segment_entity = world
        .spawn((
            ParentAgent(Entity::PLACEHOLDER), // will be updated or just placeholder
            Position(Vec3::new(105.0, 0.0, 0.0)),
            Rotation(glam::Quat::IDENTITY),
            Velocity(Vec3::new(1.0, 0.0, 0.0)),
            Segment {
                id: 0,
                length: 1.0,
                radius: 0.2,
                mass: 1.0,
            },
            ChildrenLinks(Vec::new()),
        ))
        .id();

    // Spawn an agent at x = 105.0 moving right (vx > 0.0) -> triggers outbound migration
    let genotype = one_node_genotype();
    let agent_entity = world
        .spawn((
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
        ))
        .id();

    // Update parent agent link
    world
        .entity_mut(segment_entity)
        .insert(ParentAgent(agent_entity));

    let mut schedule = Schedule::default();
    schedule.add_systems(check_migration_boundaries_system);
    schedule.run(&mut world);

    // Verify agent is despawned (both parent and segment)
    assert!(world.get_entity(agent_entity).is_none());
    assert!(world.get_entity(segment_entity).is_none());

    // Verify outbound channel has the migration package
    let outbound = outbound_rx
        .try_recv()
        .expect("Should have sent outbound migration");
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
        g.add_node(MorphologyNode {
            id: 0,
            length: 1.0,
            radius: 0.2,
            mass: 1.0,
        });
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
        brain: None,
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
        &HomeostaticState,
    )>();

    let results: Vec<_> = query.iter(&world).collect();
    assert_eq!(results.len(), 1);

    let (pos, vel, _gen, lineage, generation, parents, homeo) = results[0];
    assert_eq!(pos.0, Vec3::new(5.0, 2.0, 3.0));
    assert_eq!(vel.0, Vec3::new(-1.0, 0.0, 0.0));
    assert_eq!(lineage.0, "inbound-lineage-id");
    assert_eq!(generation.0, 5);
    assert_eq!(
        parents.0,
        vec!["parent-a".to_string(), "parent-b".to_string()]
    );
    assert_eq!(homeo.energy, 90.0);
}

#[tokio::test]
async fn test_migration_tier4_parallel_workload() {
    let reservation = TcpListener::bind("127.0.0.1:0").expect("reserve a test port");
    let port = reservation.local_addr().expect("reserved address").port();
    drop(reservation);
    let (server_inbound_tx, server_inbound_rx) = crossbeam_channel::unbounded();
    let (client_inbound_tx, _client_inbound_rx) = crossbeam_channel::unbounded();
    let (outbound_tx, outbound_rx) = crossbeam_channel::unbounded();

    let running = Arc::new(AtomicBool::new(true));

    let running_server = Arc::clone(&running);
    let server_handle = tokio::spawn(async move {
        // The server returns a Result on exit; the test asserts on the channel traffic instead, so
        // discard it explicitly rather than leaving a must_use dangling.
        let _ = run_websocket_server::<tauri::test::MockRuntime>(
            port,
            server_inbound_tx,
            running_server,
            None,
        )
        .await;
    });

    let running_client = Arc::clone(&running);
    let client_handle = tokio::spawn(async move {
        run_websocket_client::<tauri::test::MockRuntime>(
            outbound_rx,
            client_inbound_tx,
            running_client,
            None,
            8080,
        )
        .await;
    });

    drop(network_ready::connect_when_ready(port).await);

    // Send 10 parallel migrations
    let count = 10;
    let mut join_handles = vec![];
    for i in 0..count {
        let tx = outbound_tx.clone();
        let handle = tokio::spawn(async move {
            let genotype = one_node_genotype();
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
                brain: None,
            };
            tx.send(OutboundMigration {
                target_port: port,
                data: agent,
                bounds_min_x: -100.0,
                bounds_max_x: 100.0,
            })
            .unwrap();
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
            while server_inbound_rx.try_recv().is_ok() {
                got += 1;
            }
            if got >= count {
                return got;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Timeout waiting for parallel migrations");

    assert_eq!(received_count, count);

    running.store(false, Ordering::SeqCst);
    let _ = tokio::join!(server_handle, client_handle);
}
