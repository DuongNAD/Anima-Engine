// Cross-shard WebSocket migration lives behind the `networking` feature (G2). Without it this
// suite has nothing to exercise, so the whole file compiles away rather than failing to link.
#![cfg(feature = "networking")]

#[path = "support/network_ready.rs"]
mod network_ready;

use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::ecs::{AgentClass, AgentMigrationData, OutboundMigration};
use anima_engine_lib::core::engine::{run_websocket_client, run_websocket_server};
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

#[tokio::test]
async fn test_high_throughput_websocket_transfers() {
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

    // Send 500 parallel migrations
    let count = 500;
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
                position: glam::Vec3::ZERO,
                velocity: glam::Vec3::ZERO,
                lineage_id: format!("high-throughput-lineage-{}", i),
                generation: i,
                agent_class: AgentClass::Prey,
                parent_ids: vec![],
                evaluation: None,
                feature_tracker: None,
                last_transition_state: None,
                source_port: 0,
                brain: None,
            };
            let _ = tx.send(OutboundMigration {
                target_port: port,
                data: agent,
                bounds_min_x: -100.0,
                bounds_max_x: 100.0,
            });
        });
        join_handles.push(handle);
    }

    for h in join_handles {
        h.await.unwrap();
    }

    // Wait and verify we receive all 500 on the server side
    let received_count = tokio::time::timeout(Duration::from_secs(10), async {
        let mut got = 0;
        let mut ids = std::collections::HashSet::new();
        loop {
            while let Ok(data) = server_inbound_rx.try_recv() {
                if data.lineage_id.starts_with("high-throughput-lineage-") {
                    got += 1;
                    ids.insert(data.lineage_id.clone());
                }
            }
            if got >= count {
                println!("Received {} messages with {} unique IDs", got, ids.len());
                return (got, ids);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;

    running.store(false, Ordering::SeqCst);
    let _ = tokio::join!(server_handle, client_handle);

    let (received, ids) = received_count.expect("Timeout waiting for high-throughput migrations");
    println!(
        "Unique IDs received (first 10): {:?}",
        ids.iter().take(10).collect::<Vec<_>>()
    );
    assert_eq!(
        received, count,
        "Expected {} messages but received {}",
        count, received
    );
}
