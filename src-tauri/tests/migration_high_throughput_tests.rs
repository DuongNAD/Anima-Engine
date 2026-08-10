use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anima_engine_lib::core::ecs::{
    AgentMigrationData, OutboundMigration, InboundMigrationReceiver,
    OutboundMigrationSender, AgentClass, Prey
};
use anima_engine_lib::evolution::genotype::MorphologyGenotype;
use anima_engine_lib::core::engine::{
    run_websocket_server, run_websocket_client
};
use anima_engine_lib::ai::hrrl::HomeostaticState;

#[tokio::test]
async fn test_high_throughput_websocket_transfers() {
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let (server_inbound_tx, server_inbound_rx) = crossbeam_channel::unbounded();
    let (client_inbound_tx, client_inbound_rx) = crossbeam_channel::unbounded();
    let (outbound_tx, outbound_rx) = crossbeam_channel::unbounded();

    let running = Arc::new(AtomicBool::new(true));

    let running_server = Arc::clone(&running);
    let server_handle = tokio::spawn(async move {
        let _ = run_websocket_server::<tauri::test::MockRuntime>(
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
            port + 1,
        ).await;
    });

    // Let the server start
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send 500 parallel migrations
    let count = 500usize;
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
                position: glam::Vec3::ZERO,
                velocity: glam::Vec3::ZERO,
                lineage_id: format!("high-throughput-lineage-{}", i),
                generation: i as u32,
                agent_class: AgentClass::Prey,
                parent_ids: vec![],
                evaluation: None,
                feature_tracker: None,
                last_transition_state: None,
                source_port: 0,
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

    // Wait and verify we receive all 500 on either server or client bounceback (due to network load/timeout)
    let received_count = tokio::time::timeout(Duration::from_secs(15), async {
        let mut got_server = 0;
        let mut got_client = 0;
        let mut ids = std::collections::HashSet::new();
        loop {
            while let Ok(data) = server_inbound_rx.try_recv() {
                if data.lineage_id.starts_with("high-throughput-lineage-") {
                    if ids.insert(data.lineage_id.clone()) {
                        got_server += 1;
                    }
                }
            }
            while let Ok(data) = client_inbound_rx.try_recv() {
                if data.lineage_id.starts_with("high-throughput-lineage-") {
                    if ids.insert(data.lineage_id.clone()) {
                        got_client += 1;
                    }
                }
            }
            if ids.len() >= count {
                println!(
                    "Received {} messages total ({} server, {} client bounced)",
                    ids.len(), got_server, got_client
                );
                return (ids.len(), ids);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await;

    running.store(false, Ordering::SeqCst);
    let _ = tokio::join!(server_handle, client_handle);

    let (received, ids) = received_count.expect("Timeout waiting for high-throughput migrations");
    println!("Unique IDs received (first 10): {:?}", ids.iter().take(10).collect::<Vec<_>>());
    assert_eq!(received, count as usize, "Expected {} unique IDs but received {}", count, received);
}
