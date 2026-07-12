//! WebSocket relay handler — the controller side of the relay tunnel.
//!
//! Part of the controller-bridge per tunnel-loom spec §5.3.
//! Accepts WebSocket connections from relay agents, manages the global
//! relay pool (RELAYS), and provides the public API for sending commands
//! to connected relays.

use axum::extract::ws::{Message as AxumMsg, WebSocket, WebSocketUpgrade};
use std::collections::HashMap;
use std::sync::LazyLock;
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::registry::{EndpointInfo, RelayCommand, RelayState};

// ── Global relay pool ───────────────────────────────────────────────

/// Global pool of all connected relays, keyed by hostname (lowercased).
pub static RELAYS: LazyLock<Mutex<HashMap<String, RelayState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// ── Pending response map ────────────────────────────────────────────

/// Map from message ID → oneshot sender awaiting the response.
pub type PendingMap = HashMap<String, oneshot::Sender<relay_protocol::Message>>;

// ── Axum handler (public) ───────────────────────────────────────────

/// Axum WebSocket upgrade handler for `/relay`.
///
/// Relay agents connect here to register and receive commands.
pub async fn relay_handler(ws: WebSocketUpgrade) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |socket| handle_relay_connection(socket))
}

// ── Connection lifecycle ────────────────────────────────────────────

/// Main loop for a single relay agent WebSocket connection.
async fn handle_relay_connection(mut socket: WebSocket) {
    tracing::info!("relay connected");

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<RelayCommand>();
    let pending: Mutex<PendingMap> = Mutex::new(HashMap::new());
    let mut registered_hostname: Option<String> = None;

    // Spawn a dedicated task that forwards commands from cmd_rx into a
    // proxy channel so the select! loop below polls both sources fairly.
    let (diag_tx, mut diag_rx) = mpsc::unbounded_channel::<RelayCommand>();
    let _cmd_tx_alive = cmd_tx.clone();
    tokio::spawn(async move {
        let mut rx = cmd_rx;
        loop {
            match rx.recv().await {
                Some(cmd) => {
                    if diag_tx.send(cmd).is_err() {
                        break;
                    }
                }
                None => break,
            }
        }
    });

    loop {
        tokio::select! {
            // ── Outbound: command from controller → relay agent ──
            cmd = diag_rx.recv() => {
                match cmd {
                    Some(cmd) => {
                        let msg_id = cmd.message.id.clone();
                        // Stash the response channel so the relay's reply can
                        // be routed back when it arrives on the socket.
                        pending.lock().await.insert(msg_id.clone(), cmd.response_tx);

                        let json = serde_json::to_string(&cmd.message).unwrap_or_default();
                        if socket.send(AxumMsg::Text(json.into())).await.is_err() {
                            tracing::warn!(msg_id = %msg_id, "ws send failed");
                            break;
                        }
                    }
                    None => break,
                }
            }

            // ── Inbound: message from relay agent ──
            msg = socket.recv() => {
                match msg {
                    Some(Ok(AxumMsg::Text(text))) => {
                        let msg: relay_protocol::Message = match serde_json::from_str(&text) {
                            Ok(m) => m,
                            Err(e) => {
                                tracing::warn!(error = %e, "parse error");
                                continue;
                            }
                        };

                        // If it's a response, route it to the pending channel.
                        if msg.msg_type == relay_protocol::MessageType::Response {
                            let msg_id = msg.id.clone();
                            if let Some(tx) = pending.lock().await.remove(&msg_id) {
                                let _ = tx.send(msg);
                                continue;
                            }
                        }

                        // ── Registration ──
                        if msg.subtype == "register" {
                            let token = msg.payload
                                .get("token")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            // In the controller-bridge MVP, any non-empty token
                            // is accepted. Full auth validation is in Phase 2.
                            if token.is_empty() {
                                tracing::warn!("registration rejected: empty token");
                                let err = relay_protocol::Message::new_error(
                                    &msg.id, "AUTH.INVALID", "Missing registration token", None,
                                );
                                let json = serde_json::to_string(&err).unwrap();
                                let _ = socket.send(AxumMsg::Text(json.into())).await;
                                continue;
                            }

                            if let Some(hi) = msg.payload.get("host_info") {
                                let hostname = hi.get("hostname")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_lowercase();
                                if !hostname.is_empty() {
                                    let info = EndpointInfo {
                                        id: 0, // assigned by DB later
                                        name: hostname.clone(),
                                        hostname: hostname.clone(),
                                        relay_connected: true,
                                        relay_hostname: Some(hostname.clone()),
                                        arch: hi.get("arch")
                                            .and_then(|v| v.as_str())
                                            .map(String::from),
                                        os: hi.get("os")
                                            .and_then(|v| v.as_str())
                                            .map(String::from),
                                        docker_version: hi.get("docker_version")
                                            .and_then(|v| v.as_str())
                                            .map(String::from),
                                    };
                                    RELAYS.lock().await.insert(hostname.clone(), RelayState {
                                        cmd_tx: cmd_tx.clone(),
                                        info: info.clone(),
                                    });
                                    registered_hostname = Some(hostname.clone());
                                    tracing::info!(%hostname, arch = ?info.arch, os = ?info.os,
                                        "relay registered");
                                }
                            }

                            let session_id = uuid::Uuid::new_v4().to_string();
                            let resp = relay_protocol::Message::new_response(
                                &msg.id, "register_ok",
                                serde_json::json!({"session_id": session_id}),
                            );
                            let json = serde_json::to_string(&resp).unwrap();
                            let _ = socket.send(AxumMsg::Text(json.into())).await;
                            continue;
                        }

                        // ── Ping / pong ──
                        if msg.subtype == "ping" {
                            if registered_hostname.is_none() {
                                if let Some(h) = msg.payload.get("hostname")
                                    .and_then(|v| v.as_str())
                                {
                                    let hn = h.to_lowercase();
                                    if !hn.is_empty() {
                                        let info = EndpointInfo {
                                            id: 0,
                                            name: hn.clone(),
                                            hostname: hn.clone(),
                                            relay_connected: true,
                                            relay_hostname: Some(hn.clone()),
                                            arch: msg.payload.get("arch")
                                                .and_then(|v| v.as_str()).map(String::from),
                                            os: msg.payload.get("os")
                                                .and_then(|v| v.as_str()).map(String::from),
                                            docker_version: msg.payload.get("docker_version")
                                                .and_then(|v| v.as_str()).map(String::from),
                                        };
                                        RELAYS.lock().await.insert(hn.clone(), RelayState {
                                            cmd_tx: cmd_tx.clone(),
                                            info: info,
                                        });
                                        registered_hostname = Some(hn);
                                    }
                                }
                            }
                            let pong = relay_protocol::Message::new_response(
                                &msg.id, "pong",
                                serde_json::json!({"uptime_secs": 0}),
                            );
                            let json = serde_json::to_string(&pong).unwrap();
                            let _ = socket.send(AxumMsg::Text(json.into())).await;
                            continue;
                        }

                        // ── Unknown subtype ──
                        let err = relay_protocol::Message::new_error(
                            &msg.id, "NOT_IMPLEMENTED",
                            &format!("Unknown subtype: {}", msg.subtype), None,
                        );
                        let json = serde_json::to_string(&err).unwrap();
                        let _ = socket.send(AxumMsg::Text(json.into())).await;
                    }
                    Some(Ok(AxumMsg::Close(_))) => break,
                    Some(Ok(_)) => {} // ignore binary / pong / ping frames
                    Some(Err(e)) => {
                        tracing::warn!(error = %e, "ws error");
                        break;
                    }
                    None => break,
                }
            }
        }
    }

    // ── Cleanup ──
    if let Some(ref hostname) = registered_hostname {
        RELAYS.lock().await.remove(hostname);
        tracing::info!(%hostname, "relay disconnected");
    } else {
        tracing::info!("relay disconnected (was not registered)");
    }
}

// ── Public API ──────────────────────────────────────────────────────

/// Send a command to the relay identified by hostname and wait for the
/// response (with a configurable timeout).
pub async fn send_relay_command(
    hostname: &str,
    msg: relay_protocol::Message,
    timeout_secs: u64,
) -> Result<relay_protocol::Message, String> {
    let (response_tx, response_rx) = oneshot::channel();
    let msg_id = msg.id.clone();

    let cmd_tx = {
        let guard = RELAYS.lock().await;
        tracing::info!(
            %hostname,
            relay_count = guard.len(),
            msg_id = %msg_id,
            "send_relay_command: looking up relay"
        );
        guard
            .get(hostname)
            .ok_or_else(|| format!("No relay connected with hostname: {}", hostname))?
            .cmd_tx
            .clone()
    };

    tracing::info!(%hostname, msg_id = %msg_id, "send_relay_command: got cmd_tx, sending");
    cmd_tx
        .send(RelayCommand {
            message: msg,
            response_tx,
        })
        .map_err(|_| {
            tracing::error!(%hostname, msg_id = %msg_id, "send_relay_command: cmd_tx send FAILED (channel closed)");
            "Relay disconnected while sending command".to_string()
        })?;

    tracing::info!(%hostname, msg_id = %msg_id, "send_relay_command: waiting for response");
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        response_rx,
    )
    .await
    .map_err(|_| {
        tracing::error!(%hostname, msg_id = %msg_id, "send_relay_command: TIMEOUT ({}s)", timeout_secs);
        format!("Relay response timeout after {}s", timeout_secs)
    })?
    .map_err(|_| {
        tracing::error!(%hostname, msg_id = %msg_id, "send_relay_command: response channel dropped");
        "Relay dropped response channel".to_string()
    });

    tracing::info!(%hostname, msg_id = %msg_id, "send_relay_command: response received");
    result
}

/// Return public metadata for all connected relays.
pub async fn list_relays() -> Vec<EndpointInfo> {
    RELAYS
        .lock()
        .await
        .values()
        .map(|s| s.info.clone())
        .collect()
}

/// Count of currently connected relays.
pub async fn relay_count() -> usize {
    RELAYS.lock().await.len()
}
