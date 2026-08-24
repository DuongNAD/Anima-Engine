// The WebSocket stack is behind the `networking` feature (G2). `MigrationPayload` and
// `hash_lineage_id` below are plain data and stay available either way, because other modules
// glob-import this one and only the transport is optional.
#[cfg(feature = "networking")]
use futures_util::{SinkExt, StreamExt};
#[cfg(feature = "networking")]
use socket2::{Domain, Protocol, Socket, Type};
#[cfg(feature = "networking")]
use std::net::SocketAddr;
#[cfg(feature = "networking")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "networking")]
use std::sync::Arc;
#[cfg(feature = "networking")]
use std::time::Duration;
#[cfg(feature = "networking")]
use tauri::Emitter;
#[cfg(feature = "networking")]
use tokio_tungstenite::accept_async_with_config;

/// Upper bound before JSON deserialization. The current 5,769-parameter evolved brain serializes
/// well below this; oversized frames are rejected by tungstenite before allocating an agent graph.
#[cfg(feature = "networking")]
pub const MAX_MIGRATION_WEBSOCKET_MESSAGE_BYTES: usize = 1024 * 1024;
/// A shard accepts at most this many concurrently open migration sockets.
#[cfg(feature = "networking")]
pub const MAX_MIGRATION_WEBSOCKET_CONNECTIONS: usize = 64;
#[cfg(feature = "networking")]
pub const MIGRATION_ACK_ACCEPTED: &str = "anima-migration-v1:accepted";
#[cfg(feature = "networking")]
pub const MIGRATION_ACK_REJECTED: &str = "anima-migration-v1:rejected";
#[cfg(feature = "networking")]
const MIGRATION_ACK_WAIT: Duration = Duration::from_millis(500);
#[cfg(feature = "networking")]
const INBOUND_BACKPRESSURE_WAIT: Duration = Duration::from_millis(300);

/// Which way an agent crossed a shard boundary.
///
/// An enum rather than a `String`, and the reason is a drift the type system should have been
/// holding: the field was `String` with `// "incoming" | "outgoing"` beside it, while `App.tsx`
/// declared the union for real. The comment was the only thing keeping the two in agreement, and a
/// comment cannot fail a build. `#[serde(rename_all = "lowercase")]` reproduces the exact strings
/// this has always emitted, so the wire format is unchanged and old saves/logs still parse.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MigrationDirection {
    Incoming,
    Outgoing,
}

/// Whether the transfer completed.
///
/// Capitalised on the wire, unlike [`MigrationDirection`] — an inconsistency this preserves rather
/// than tidies. Renaming it would be a silent IPC break for the sake of symmetry nobody consumes.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationStatus {
    Success,
    Failed,
}

/// The `migration-event` payload.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug)]
pub struct MigrationPayload {
    pub agent_id: u32,
    pub direction: MigrationDirection,
    pub source_port: u16,
    pub target_port: u16,
    pub status: MigrationStatus,
    pub timestamp: u64,
}

pub fn hash_lineage_id(id: &str) -> u32 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(id, &mut hasher);
    (std::hash::Hasher::finish(&hasher) & 0x7FFFFFFF) as u32
}

#[cfg(feature = "networking")]
/// Compatibility entry point for callers that do not need observable rejection counters.
///
/// Production uses [`run_websocket_server_with_diagnostics`].
pub async fn run_websocket_server<R: tauri::Runtime>(
    port: u16,
    inbound_tx: crossbeam_channel::Sender<crate::core::ecs::AgentMigrationData>,
    running: Arc<AtomicBool>,
    app_handle: Option<tauri::AppHandle<R>>,
) -> Result<(), String> {
    run_websocket_server_with_diagnostics(
        port,
        inbound_tx,
        running,
        app_handle,
        crate::core::resources::MigrationHandoffDiagnostics::default(),
    )
    .await
}

#[cfg(feature = "networking")]
pub async fn run_websocket_server_with_diagnostics<R: tauri::Runtime>(
    port: u16,
    inbound_tx: crossbeam_channel::Sender<crate::core::ecs::AgentMigrationData>,
    running: Arc<AtomicBool>,
    app_handle: Option<tauri::AppHandle<R>>,
    diagnostics: crate::core::resources::MigrationHandoffDiagnostics,
) -> Result<(), String> {
    if port == 0 {
        return Ok(());
    }
    let addr = format!("127.0.0.1:{}", port);

    // Configure socket2 with SO_REUSEADDR before binding
    let socket = match Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)) {
        Ok(s) => s,
        Err(e) => {
            let err_msg = format!("Failed to create socket: {}", e);
            eprintln!("{}", err_msg);
            return Err(err_msg);
        }
    };
    if let Err(e) = socket.set_reuse_address(true) {
        let err_msg = format!("Failed to set SO_REUSEADDR: {}", e);
        eprintln!("{}", err_msg);
        return Err(err_msg);
    }
    let address: SocketAddr = match addr.parse() {
        Ok(a) => a,
        Err(e) => {
            let err_msg = format!("Failed to parse address {}: {}", addr, e);
            eprintln!("{}", err_msg);
            return Err(err_msg);
        }
    };
    if let Err(e) = socket.bind(&address.into()) {
        let err_msg = format!("Failed to bind to {}: {}", addr, e);
        eprintln!("{}", err_msg);
        return Err(err_msg);
    }
    if let Err(e) = socket.listen(128) {
        let err_msg = format!("Failed to listen: {}", e);
        eprintln!("{}", err_msg);
        return Err(err_msg);
    }
    if let Err(e) = socket.set_nonblocking(true) {
        let err_msg = format!("Failed to set nonblocking: {}", e);
        eprintln!("{}", err_msg);
        return Err(err_msg);
    }
    let std_listener: std::net::TcpListener = socket.into();
    let listener = match tokio::net::TcpListener::from_std(std_listener) {
        Ok(l) => l,
        Err(e) => {
            let err_msg = format!("Failed to convert TcpListener to tokio: {}", e);
            eprintln!("{}", err_msg);
            return Err(err_msg);
        }
    };
    let reported_invalid = Arc::new(AtomicBool::new(false));
    let reported_connection_limit = Arc::new(AtomicBool::new(false));
    let reported_backpressure = Arc::new(AtomicBool::new(false));
    let reported_disconnected = Arc::new(AtomicBool::new(false));
    let connection_slots = Arc::new(tokio::sync::Semaphore::new(
        MAX_MIGRATION_WEBSOCKET_CONNECTIONS,
    ));

    while running.load(Ordering::SeqCst) {
        tokio::select! {
            accept_res = listener.accept() => {
                if let Ok((stream, _)) = accept_res {
                    let Ok(connection_slot) =
                        Arc::clone(&connection_slots).try_acquire_owned()
                    else {
                        diagnostics.record_connection_limit_rejection();
                        if !reported_connection_limit.swap(true, Ordering::Relaxed) {
                            eprintln!(
                                "migration connection limit reached; refusing excess peers \
                                 (further reports are suppressed)"
                            );
                        }
                        continue;
                    };
                    let inbound_tx = inbound_tx.clone();
                    let running = running.clone();
                    let app_handle = app_handle.clone();
                    let diagnostics = diagnostics.clone();
                    let reported_invalid = Arc::clone(&reported_invalid);
                    let reported_backpressure = Arc::clone(&reported_backpressure);
                    let reported_disconnected = Arc::clone(&reported_disconnected);
                    tokio::spawn(async move {
                        let _connection_slot = connection_slot;
                        let websocket_config =
                            tokio_tungstenite::tungstenite::protocol::WebSocketConfig {
                                max_message_size: Some(MAX_MIGRATION_WEBSOCKET_MESSAGE_BYTES),
                                max_frame_size: Some(MAX_MIGRATION_WEBSOCKET_MESSAGE_BYTES),
                                ..Default::default()
                            };
                        if let Ok(ws_stream) =
                            accept_async_with_config(stream, Some(websocket_config)).await
                        {
                            let (mut write, mut read) = ws_stream.split();
                            while running.load(Ordering::SeqCst) {
                                let next_msg_fut = read.next();
                                match tokio::time::timeout(Duration::from_secs(5), next_msg_fut).await {
                                    Ok(Some(Ok(msg))) => {
                                        if msg.is_text() || msg.is_binary() {
                                            let data_str = msg.to_text().unwrap_or("");
                                            match serde_json::from_str::<crate::core::ecs::AgentMigrationData>(data_str) {
                                                Ok(data) => {
                                                let agent_id = hash_lineage_id(&data.lineage_id);
                                                let source_port = data.source_port;
                                                let status = match data.validate() {
                                                    Ok(()) => {
                                                        let mut pending = data;
                                                        let mut observed_backpressure = false;
                                                        let deadline = tokio::time::Instant::now()
                                                            + INBOUND_BACKPRESSURE_WAIT;
                                                        loop {
                                                            match inbound_tx.try_send(pending) {
                                                                Ok(()) => break MigrationStatus::Success,
                                                                Err(crossbeam_channel::TrySendError::Full(data)) => {
                                                                    pending = data;
                                                                    if !observed_backpressure {
                                                                        diagnostics.record_inbound_backpressure();
                                                                        observed_backpressure = true;
                                                                    }
                                                                    if !reported_backpressure.swap(true, Ordering::Relaxed) {
                                                                        eprintln!(
                                                                            "inbound migration queue is full; applying connection backpressure (further reports are suppressed)"
                                                                        );
                                                                    }
                                                                    if !running.load(Ordering::SeqCst) {
                                                                        break MigrationStatus::Failed;
                                                                    }
                                                                    if tokio::time::Instant::now() >= deadline {
                                                                        break MigrationStatus::Failed;
                                                                    }
                                                                    tokio::time::sleep(Duration::from_millis(10)).await;
                                                                }
                                                                Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                                                                    diagnostics.record_disconnected_rejection();
                                                                    if !reported_disconnected.swap(true, Ordering::Relaxed) {
                                                                        eprintln!(
                                                                            "inbound migration could not enter the simulation queue because it is disconnected (further reports are suppressed)"
                                                                        );
                                                                    }
                                                                    break MigrationStatus::Failed;
                                                                }
                                                            }
                                                        }
                                                    },
                                                    Err(reason) => {
                                                        diagnostics.record_invalid_rejection();
                                                        if !reported_invalid.swap(true, Ordering::Relaxed) {
                                                            eprintln!(
                                                                "network migration rejected invalid scientific state ({reason}) (further reports are suppressed)"
                                                            );
                                                        }
                                                        MigrationStatus::Failed
                                                    }
                                                };
                                                if let Some(ref handle) = app_handle {
                                                    let payload = MigrationPayload {
                                                        agent_id,
                                                        direction: MigrationDirection::Incoming,
                                                        source_port,
                                                        target_port: port,
                                                        status,
                                                        timestamp: std::time::SystemTime::now()
                                                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                                            .unwrap_or_default()
                                                            .as_millis() as u64,
                                                    };
                                                    let _ = handle.emit("migration-event", &payload);
                                                }
                                                let acknowledgement = match status {
                                                    MigrationStatus::Success => MIGRATION_ACK_ACCEPTED,
                                                    MigrationStatus::Failed => MIGRATION_ACK_REJECTED,
                                                };
                                                if write
                                                    .send(tokio_tungstenite::tungstenite::Message::Text(
                                                        acknowledgement.to_owned(),
                                                    ))
                                                    .await
                                                    .is_err()
                                                {
                                                    break;
                                                }
                                                }
                                                Err(reason) => {
                                                    diagnostics.record_invalid_rejection();
                                                    if !reported_invalid.swap(true, Ordering::Relaxed) {
                                                        eprintln!(
                                                            "network migration rejected malformed JSON ({reason}) (further reports are suppressed)"
                                                        );
                                                    }
                                                    let _ = write
                                                        .send(tokio_tungstenite::tungstenite::Message::Text(
                                                            MIGRATION_ACK_REJECTED.to_owned(),
                                                        ))
                                                        .await;
                                                }
                                            }
                                        }
                                    }
                                    Ok(Some(Err(reason))) => {
                                        diagnostics.record_invalid_rejection();
                                        if !reported_invalid.swap(true, Ordering::Relaxed) {
                                            eprintln!(
                                                "network migration rejected an invalid WebSocket message ({reason}) (further reports are suppressed)"
                                            );
                                        }
                                        break;
                                    }
                                    Ok(None) => {
                                        break;
                                    }
                                    Err(_) => {
                                        eprintln!("WebSocket read timeout reached, closing connection");
                                        break;
                                    }
                                }
                            }
                        }
                    });
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
            }
        }
    }
    Ok(())
}

#[cfg(feature = "networking")]
pub async fn run_websocket_client<R: tauri::Runtime>(
    outbound_rx: crossbeam_channel::Receiver<crate::core::ecs::OutboundMigration>,
    inbound_tx: crossbeam_channel::Sender<crate::core::ecs::AgentMigrationData>,
    running: Arc<AtomicBool>,
    app_handle: Option<tauri::AppHandle<R>>,
    local_port: u16,
) {
    while running.load(Ordering::SeqCst) {
        match outbound_rx.try_recv() {
            Ok(migration) => {
                let target_port = migration.target_port;
                let data = migration.data;

                let send_result = if target_port == 9999 {
                    Err("Target connection refused (simulate closed port)".to_string())
                } else {
                    let url = format!("ws://127.0.0.1:{}", target_port);
                    match tokio::time::timeout(
                        Duration::from_millis(500),
                        tokio_tungstenite::connect_async(&url),
                    )
                    .await
                    {
                        Ok(Ok((mut ws_stream, _))) => {
                            // A failed serialization goes through the same Result the rest of this
                            // match arm uses, so the caller's bounce-back path puts the agent back
                            // inside local coordinates. It used to `.unwrap()`, which panics inside
                            // a tokio task on the networking thread — taking the migration channel
                            // down for the whole run over one unrepresentable agent.
                            let send_res = match serde_json::to_string(&data) {
                                Ok(serialized) => {
                                    let msg =
                                        tokio_tungstenite::tungstenite::Message::Text(serialized);
                                    let send_outcome = match tokio::time::timeout(
                                        Duration::from_millis(500),
                                        ws_stream.send(msg),
                                    )
                                    .await
                                    {
                                        Ok(Ok(())) => {
                                            // Version-1 peers acknowledge acceptance or rejection.
                                            // No reply within the compatibility window is treated
                                            // as a legacy peer, preserving the pre-ACK wire
                                            // behaviour during rolling upgrades.
                                            match tokio::time::timeout(
                                                MIGRATION_ACK_WAIT,
                                                ws_stream.next(),
                                            )
                                            .await
                                            {
                                                Ok(Some(Ok(reply)))
                                                    if reply.to_text().ok()
                                                        == Some(MIGRATION_ACK_ACCEPTED) =>
                                                {
                                                    Ok(())
                                                }
                                                Ok(Some(Ok(reply)))
                                                    if reply.to_text().ok()
                                                        == Some(MIGRATION_ACK_REJECTED) =>
                                                {
                                                    Err("Target rejected migration payload"
                                                        .to_owned())
                                                }
                                                // Only an explicit version-1 rejection transfers
                                                // ownership back. Unknown/control frames and read
                                                // errors retain the pre-ACK delivery semantics;
                                                // bouncing after the peer may have enqueued would
                                                // duplicate biomass across shards.
                                                Ok(Some(Ok(_))) | Ok(Some(Err(_))) => Ok(()),
                                                Ok(None) | Err(_) => Ok(()),
                                            }
                                        }
                                        Ok(Err(e)) => Err(e.to_string()),
                                        Err(_) => Err("Send timeout".to_string()),
                                    };
                                    send_outcome
                                }
                                Err(e) => Err(format!("serialize migration payload: {e}")),
                            };
                            let _ = ws_stream.close(None).await;
                            send_res
                        }
                        Ok(Err(e)) => Err(e.to_string()),
                        Err(_) => Err("Connection timeout".to_string()),
                    }
                };

                let status_str = if send_result.is_ok() {
                    MigrationStatus::Success
                } else {
                    MigrationStatus::Failed
                };

                if let Some(ref handle) = app_handle {
                    let payload = MigrationPayload {
                        agent_id: hash_lineage_id(&data.lineage_id),
                        direction: MigrationDirection::Outgoing,
                        source_port: data.source_port,
                        target_port,
                        status: status_str,
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                    };
                    let _ = handle.emit("migration-event", &payload);
                }

                if send_result.is_err() {
                    if let Some(ref handle) = app_handle {
                        let payload = MigrationPayload {
                            agent_id: hash_lineage_id(&data.lineage_id),
                            direction: MigrationDirection::Outgoing,
                            source_port: local_port,
                            target_port,
                            status: MigrationStatus::Failed,
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64,
                        };
                        let _ = handle.emit("migration-event", &payload);
                    }
                    // Bounce agent back inside local coordinates
                    let mut bounced_data = data;
                    let bounds_min_x = migration.bounds_min_x;
                    let bounds_max_x = migration.bounds_max_x;
                    let width = (bounds_max_x - bounds_min_x).max(0.0);
                    let offset = 1.0_f32.min(0.1 * width);
                    if bounced_data.velocity.x > 0.0 {
                        bounced_data.position.x = bounds_max_x - offset;
                        bounced_data.velocity.x = -bounced_data.velocity.x.abs();
                    } else {
                        bounced_data.position.x = bounds_min_x + offset;
                        bounced_data.velocity.x = bounced_data.velocity.x.abs();
                    }
                    // The source has already transferred ownership to this worker. Never block a
                    // Tokio thread on the bounded simulation queue and never discard the only copy
                    // merely because one tick is busy: retry asynchronously until the queue has
                    // room or the engine is shutting down.
                    let mut pending = bounced_data;
                    while running.load(Ordering::SeqCst) {
                        match inbound_tx.try_send(pending) {
                            Ok(()) => break,
                            Err(crossbeam_channel::TrySendError::Full(data)) => {
                                pending = data;
                                tokio::time::sleep(Duration::from_millis(10)).await;
                            }
                            Err(crossbeam_channel::TrySendError::Disconnected(_)) => break,
                        }
                    }
                }
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                break;
            }
        }
    }
}
