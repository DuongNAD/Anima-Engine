// Cross-shard WebSocket migration lives behind the `networking` feature (G2). Without it this
// suite has nothing to exercise, so the whole file compiles away rather than failing to link.
#![cfg(feature = "networking")]

mod common;

use glam::Vec3;
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::ecs::{AgentClass, AgentMigrationData, OutboundMigration};
use anima_engine_lib::core::engine::{run_websocket_client, run_websocket_server};
use anima_engine_lib::evolution::genotype::MorphologyGenotype;
use anima_engine_lib::evolution::lineage::FallbackLineageTracker;
use anima_engine_lib::evolution::meta_ai::{EnvironmentalEvent, GeminiMetaAiClient, MetaAiClient};

// Held for the whole async test on purpose: these bind fixed ports and must not overlap.
// `std::sync::Mutex` is the wrong tool for that — its guard is not `Send` across an await, which
// clippy flags as `await_holding_lock`. Tokio.s mutex is built for exactly this.
static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// 1. Test port binding failure: Ensure the server handles bind errors gracefully and exits cleanly.
#[tokio::test]
async fn test_adversarial_port_binding_failure() {
    let _lock = TEST_LOCK.lock().await;

    // Bind a standard TcpListener to reserve a port.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let (inbound_tx, _inbound_rx) = crossbeam_channel::unbounded();
    let running = Arc::new(AtomicBool::new(true));

    // Now attempt to run run_websocket_server on the SAME port.
    // It should log/eprint an error and return without crashing or panicking.
    let running_clone = Arc::clone(&running);
    let server_handle = tokio::spawn(async move {
        // The server returns a Result on exit; the test asserts on the channel traffic instead, so
        // discard it explicitly rather than leaving a must_use dangling.
        let _ =
            run_websocket_server::<tauri::test::MockRuntime>(port, inbound_tx, running_clone, None)
                .await;
    });

    // Wait a brief period and ensure the server handle completes/exits immediately
    // because binding fails.
    let result = tokio::time::timeout(Duration::from_secs(2), server_handle).await;
    match result {
        Ok(join_res) => {
            // Task exited cleanly (did not panic or block forever)
            assert!(join_res.is_ok());
        }
        Err(_) => {
            panic!("run_websocket_server hung even when port binding failed!");
        }
    }

    running.store(false, Ordering::SeqCst);
}

/// 2. Test client read timeout / silent connection: Ensure that a silent client
/// connecting to the server does not block the server from exiting when stopped.
#[tokio::test]
async fn test_adversarial_silent_client_shutdown() {
    let _lock = TEST_LOCK.lock().await;

    // Bind server on a random free port
    let server_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = server_listener.local_addr().unwrap().port();
    drop(server_listener); // release so server can bind

    let (inbound_tx, _inbound_rx) = crossbeam_channel::unbounded();
    let running = Arc::new(AtomicBool::new(true));

    let running_clone = Arc::clone(&running);
    let server_handle = tokio::spawn(async move {
        // The server returns a Result on exit; the test asserts on the channel traffic instead, so
        // discard it explicitly rather than leaving a must_use dangling.
        let _ =
            run_websocket_server::<tauri::test::MockRuntime>(port, inbound_tx, running_clone, None)
                .await;
    });

    // Give server time to bind
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Connect a silent client using tokio TcpStream
    let client_stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port)).await;
    assert!(client_stream.is_ok(), "Client failed to connect to server");

    // Do NOT send any data, and then signal the server to stop.
    running.store(false, Ordering::SeqCst);

    // Verify that the server stops promptly and the task joins.
    let join_result = tokio::time::timeout(Duration::from_secs(2), server_handle).await;
    assert!(
        join_result.is_ok(),
        "Server hung with a silent client connected when shut down"
    );
}

/// 3. Test stale connection cache recovery: Ensure that the client recovers
/// from a broken connection cache if the target server restarts or drops.
#[tokio::test]
async fn test_adversarial_stale_connection_cache() {
    let _lock = TEST_LOCK.lock().await;

    // Use a random free port
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let (server_inbound_tx_1, server_inbound_rx_1) = crossbeam_channel::unbounded();
    let (client_inbound_tx, client_inbound_rx) = crossbeam_channel::unbounded();
    let (outbound_tx, outbound_rx) = crossbeam_channel::unbounded();

    let server_running_1 = Arc::new(AtomicBool::new(true));
    let client_running = Arc::new(AtomicBool::new(true));

    // Start Server 1
    let sr1 = Arc::clone(&server_running_1);
    let server_handle_1 = tokio::spawn(async move {
        // The server returns a Result on exit; the test asserts on the channel traffic instead, so
        // discard it explicitly rather than leaving a must_use dangling.
        let _ =
            run_websocket_server::<tauri::test::MockRuntime>(port, server_inbound_tx_1, sr1, None)
                .await;
    });

    // Start Client
    let cr = Arc::clone(&client_running);
    let client_handle = tokio::spawn(async move {
        run_websocket_client::<tauri::test::MockRuntime>(
            outbound_rx,
            client_inbound_tx,
            cr,
            None,
            8080,
        )
        .await;
    });

    tokio::time::sleep(Duration::from_millis(150)).await;

    let genotype = MorphologyGenotype::new();
    let agent = AgentMigrationData {
        genotype: genotype.clone(),
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
        velocity: Vec3::new(1.0, 0.0, 0.0),
        lineage_id: "stale-cache-agent-1".to_string(),
        generation: 1,
        agent_class: AgentClass::Prey,
        parent_ids: vec![],
        evaluation: None,
        feature_tracker: None,
        last_transition_state: None,
        source_port: 0,
        brain: None,
    };

    // Send first migration (should succeed and establish cached connection)
    outbound_tx
        .send(OutboundMigration {
            target_port: port,
            data: agent.clone(),
            bounds_min_x: -100.0,
            bounds_max_x: 100.0,
        })
        .unwrap();

    let received_1 = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(data) = server_inbound_rx_1.try_recv() {
                return data;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("First migration failed to deliver");
    assert_eq!(received_1.lineage_id, "stale-cache-agent-1");

    // Force Server 1 to exit
    server_running_1.store(false, Ordering::SeqCst);
    let _ = server_handle_1.await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Start Server 2 on the same port
    let (server_inbound_tx_2, server_inbound_rx_2) = crossbeam_channel::unbounded();
    let server_running_2 = Arc::new(AtomicBool::new(true));
    let sr2 = Arc::clone(&server_running_2);
    let server_handle_2 = tokio::spawn(async move {
        // The server returns a Result on exit; the test asserts on the channel traffic instead, so
        // discard it explicitly rather than leaving a must_use dangling.
        let _ =
            run_websocket_server::<tauri::test::MockRuntime>(port, server_inbound_tx_2, sr2, None)
                .await;
    });

    tokio::time::sleep(Duration::from_millis(150)).await;

    let agent2 = AgentMigrationData {
        lineage_id: "stale-cache-agent-2".to_string(),
        ..agent.clone()
    };

    // Send second migration (client cache still holds stale Server 1 connection)
    println!("Sending second migration (agent 2) targeting port {}", port);
    outbound_tx
        .send(OutboundMigration {
            target_port: port,
            data: agent2.clone(),
            bounds_min_x: -100.0,
            bounds_max_x: 100.0,
        })
        .unwrap();

    // Sleep to allow OS TCP stack to process the send and receive a TCP RST from the closed port
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Send third migration (agent 3). Since agent 2 sent on a dead socket, the socket should now be marked broken,
    // and sending agent 3 should fail, triggering a bounce back and clearing the cache.
    println!("Sending third migration (agent 3) targeting port {}", port);
    let agent3 = AgentMigrationData {
        lineage_id: "stale-cache-agent-3".to_string(),
        ..agent.clone()
    };
    outbound_tx
        .send(OutboundMigration {
            target_port: port,
            data: agent3.clone(),
            bounds_min_x: -100.0,
            bounds_max_x: 100.0,
        })
        .unwrap();

    // Verify if any bounce back is received.
    println!("Waiting for bounce backs...");
    let mut bounced_agents = Vec::new();
    let start_wait = std::time::Instant::now();
    while start_wait.elapsed() < Duration::from_secs(3) {
        if let Ok(data) = client_inbound_rx.try_recv() {
            println!("Received bounce back for: {}", data.lineage_id);
            bounced_agents.push(data.lineage_id);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Verify if Server 2 received agent 2 or agent 3.
    let mut received_by_server2 = Vec::new();
    while let Ok(data) = server_inbound_rx_2.try_recv() {
        println!("Server 2 received: {}", data.lineage_id);
        received_by_server2.push(data.lineage_id);
    }

    // Assertions for corrected robust behavior:
    // 1. Agent 2 was NOT lost and was successfully received by Server 2.
    assert!(
        received_by_server2.contains(&"stale-cache-agent-2".to_string()),
        "Agent 2 must be received by Server 2"
    );

    // 2. Agent 3 was NOT lost and was successfully received by Server 2.
    assert!(
        received_by_server2.contains(&"stale-cache-agent-3".to_string()),
        "Agent 3 must be received by Server 2"
    );

    // Cleanup
    server_running_2.store(false, Ordering::SeqCst);
    client_running.store(false, Ordering::SeqCst);
    let _ = tokio::join!(server_handle_2, client_handle);
}

/// 4. Test FallbackLineageTracker online/offline connection timeouts:
///    Ensure that attempting to connect to a silent server times out without blocking the constructor indefinitely.
#[test]
fn test_adversarial_lineage_tracker_connection_timeout() {
    let _lock = TEST_LOCK.blocking_lock();

    // Bind a TcpListener but do not accept connections (it will be silent/unresponsive)
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let start = std::time::Instant::now();
    // Constructor will attempt to connect, and ping, but it must timeout and report offline
    let tracker =
        FallbackLineageTracker::new(&format!("bolt://127.0.0.1:{}", port), "neo4j", "password");
    let dur = start.elapsed();

    assert!(
        !tracker.is_online(),
        "Tracker must report offline for unresponsive host"
    );
    // Connect timeout is 500ms, ping timeout is 500ms, total under 1.5 seconds.
    assert!(
        dur < Duration::from_millis(1500),
        "Constructor blocked for too long: {:?}",
        dur
    );
}

/// 5. Test Gemini client offline fallback when API key is set but server is unreachable or rate limited.
#[test]
fn test_adversarial_gemini_client_offline_fallback() {
    let _lock = TEST_LOCK.blocking_lock();

    // Set a dummy API key to force the Gemini client to try HTTP requests.
    std::env::set_var("GEMINI_API_KEY", "dummy_key_for_adversarial_testing");

    // Initialize the client with a very short timeout (e.g., 10 milliseconds).
    let client = GeminiMetaAiClient::new(Duration::from_millis(10));

    // Call generate_event. It will attempt to post to google API, but fails or times out
    // due to invalid/blocked endpoint, and must return the mock event promptly.
    let start = std::time::Instant::now();
    let event = client.generate_event(1, &[]);
    let duration = start.elapsed();

    // Assert it falls back to the mock event (ResourceDrought for epoch 1)
    assert_eq!(event, EnvironmentalEvent::ResourceDrought);

    // Assert it did not block excessively (less than 1500ms)
    assert!(
        duration < Duration::from_millis(1500),
        "Gemini client blocked for too long: {:?}",
        duration
    );

    // Clean up environment variable
    std::env::remove_var("GEMINI_API_KEY");
}
