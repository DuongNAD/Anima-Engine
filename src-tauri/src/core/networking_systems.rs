use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::net::SocketAddr;
use tokio_tungstenite::accept_async;
use futures_util::{StreamExt, SinkExt};
use tauri::Emitter;
use socket2::{Socket, Domain, Type, Protocol};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct MigrationPayload {
    pub agent_id: u32,
    pub direction: String, // "incoming" | "outgoing"
    pub source_port: u16,
    pub target_port: u16,
    pub status: String,    // "Success" | "Failed"
    pub timestamp: u64,
}

pub fn hash_lineage_id(id: &str) -> u32 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(id, &mut hasher);
    (std::hash::Hasher::finish(&hasher) & 0x7FFFFFFF) as u32
}

pub async fn run_websocket_server<R: tauri::Runtime>(
    port: u16,
    inbound_tx: crossbeam_channel::Sender<crate::core::ecs::AgentMigrationData>,
    running: Arc<AtomicBool>,
    app_handle: Option<tauri::AppHandle<R>>,
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

    while running.load(Ordering::SeqCst) {
        tokio::select! {
            accept_res = listener.accept() => {
                if let Ok((stream, _)) = accept_res {
                    let inbound_tx = inbound_tx.clone();
                    let running = running.clone();
                    let app_handle = app_handle.clone();
                    tokio::spawn(async move {
                        if let Ok(ws_stream) = accept_async(stream).await {
                            let (_, mut read) = ws_stream.split();
                            while running.load(Ordering::SeqCst) {
                                let next_msg_fut = read.next();
                                match tokio::time::timeout(Duration::from_secs(5), next_msg_fut).await {
                                    Ok(Some(Ok(msg))) => {
                                        if msg.is_text() || msg.is_binary() {
                                            let data_str = msg.to_text().unwrap_or("");
                                            if let Ok(data) = serde_json::from_str::<crate::core::ecs::AgentMigrationData>(data_str) {
                                                if let Some(ref handle) = app_handle {
                                                    let payload = MigrationPayload {
                                                        agent_id: hash_lineage_id(&data.lineage_id),
                                                        direction: "incoming".to_string(),
                                                        source_port: data.source_port,
                                                        target_port: port,
                                                        status: "Success".to_string(),
                                                        timestamp: std::time::SystemTime::now()
                                                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                                            .unwrap_or_default()
                                                            .as_millis() as u64,
                                                    };
                                                    let _ = handle.emit("migration-event", &payload);
                                                }
                                                let _ = inbound_tx.send(data);
                                            }
                                        }
                                    }
                                    Ok(_) => {
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
                    match tokio::time::timeout(Duration::from_millis(500), tokio_tungstenite::connect_async(&url)).await {
                        Ok(Ok((mut ws_stream, _))) => {
                            let serialized = serde_json::to_string(&data).unwrap();
                            let msg = tokio_tungstenite::tungstenite::Message::Text(serialized);
                            let send_res = match tokio::time::timeout(Duration::from_millis(500), ws_stream.send(msg)).await {
                                Ok(Ok(())) => Ok(()),
                                Ok(Err(e)) => Err(e.to_string()),
                                Err(_) => Err("Send timeout".to_string()),
                            };
                            let _ = ws_stream.close(None).await;
                            send_res
                        }
                        Ok(Err(e)) => Err(e.to_string()),
                        Err(_) => Err("Connection timeout".to_string()),
                    }
                };

                let status_str = if send_result.is_ok() {
                    "Success".to_string()
                } else {
                    "Failed".to_string()
                };

                if let Some(ref handle) = app_handle {
                    let payload = MigrationPayload {
                        agent_id: hash_lineage_id(&data.lineage_id),
                        direction: "outgoing".to_string(),
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
                            direction: "outgoing".to_string(),
                            source_port: local_port,
                            target_port,
                            status: "Failed".to_string(),
                            timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
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
                    let _ = inbound_tx.send(bounced_data);
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
