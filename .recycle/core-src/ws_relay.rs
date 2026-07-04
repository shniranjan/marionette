use axum::extract::ws::{Message as AxumMsg, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use chrono::Utc;
use relay_protocol::payloads::PongResponse;
use relay_protocol::Message;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::relay::auth;
use crate::relay::session::SessionManager;
use crate::relay::signed::SignedMessage;

/// Pending relay requests awaiting response.
pub type PendingMap = Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Message>>>>;

/// State for a single connected relay.
struct RelayState {
    cmd_tx: mpsc::UnboundedSender<RelayCommand>,
    info: RelayInfo,
}

/// Multi-relay registry: hostname → RelayState.
static RELAYS: std::sync::OnceLock<Mutex<HashMap<String, RelayState>>> =
    std::sync::OnceLock::new();

fn relays() -> &'static Mutex<HashMap<String, RelayState>> {
    RELAYS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub struct RelayCommand {
    pub message: Message,
    pub response_tx: oneshot::Sender<Message>,
    /// When set, all events AND the final response are forwarded through this
    /// mpsc sender instead of going through the oneshot wrapper. Used by the
    /// streaming WebSocket endpoint.
    pub stream_tx: Option<mpsc::UnboundedSender<Message>>,
}

/// Relay connection status and host information.
#[derive(Debug, Clone, Serialize)]
pub struct RelayInfo {
    pub connected: bool,
    pub hostname: String,
    pub docker_version: String,
    pub arch: String,
    pub os: String,
    pub relay_version: String,
    pub connected_at: String,
    pub endpoint_id: Option<String>,
}

/// Return status for all connected relays.
pub async fn get_all_relay_status() -> HashMap<String, RelayInfo> {
    relays()
        .lock()
        .await
        .iter()
        .map(|(hostname, state)| (hostname.clone(), state.info.clone()))
        .collect()
}

/// Find the hostname of a relay registered for the given endpoint.
pub async fn get_relay_for_endpoint(endpoint_id: &str) -> Option<String> {
    relays()
        .lock()
        .await
        .iter()
        .find(|(_, state)| state.info.endpoint_id.as_deref() == Some(endpoint_id))
        .map(|(hostname, _)| hostname.clone())
}

/// Send a command to the relay identified by hostname and wait for response.
pub async fn send_relay_command(hostname: &str, msg: Message) -> Result<Message, String> {
    let (response_tx, response_rx) = oneshot::channel();
    let msg_id = msg.id.clone();

    let cmd_tx = {
        let guard = relays().lock().await;
        tracing::info!(%hostname, relay_count = guard.len(), msg_id = %msg_id, "send_relay_command: looking up relay");
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
            stream_tx: None,
        })
        .map_err(|_| {
            tracing::error!(%hostname, msg_id = %msg_id, "send_relay_command: cmd_tx send FAILED (channel closed)");
            "Relay disconnected while sending command".to_string()
        })?;

    tracing::info!(%hostname, msg_id = %msg_id, "send_relay_command: waiting for response");
    let result = tokio::time::timeout(std::time::Duration::from_secs(30), response_rx)
        .await
        .map_err(|_| {
            tracing::error!(%hostname, msg_id = %msg_id, "send_relay_command: TIMEOUT (30s)");
            "Relay response timeout".to_string()
        })?
        .map_err(|_| {
            tracing::error!(%hostname, msg_id = %msg_id, "send_relay_command: response channel dropped");
            "Relay dropped response channel".to_string()
        });
    tracing::info!(%hostname, msg_id = %msg_id, "send_relay_command: response received");
    result
}

use crate::models::DockerEndpoint;
use crate::models::EndpointStatus;
use crate::registry::EndpointRegistry;

/// Resolve an endpoint_id for a relay hostname.
/// Matches hostname to existing endpoints (case-insensitive by name),
/// or creates a new endpoint so unauthenticated relays are visible
/// to get_relay_for_endpoint().
async fn resolve_endpoint_for_hostname(
    registry: &Arc<EndpointRegistry>,
    hostname: &str,
) -> String {
    // Try to match hostname to existing endpoint names (case-insensitive)
    let existing = registry.list().await;
    if let Some(ep) = existing.iter().find(|ep| ep.name.to_lowercase() == hostname.to_lowercase()) {
        tracing::info!(%hostname, endpoint_id = %ep.id, "bound unauthenticated relay to existing endpoint");
        return ep.id.clone();
    }

    // No match — create a new default endpoint
    let new_id = uuid::Uuid::new_v4().to_string();
    let ep = DockerEndpoint {
        id: new_id.clone(),
        name: hostname.to_string(),
        connection: "unix:///var/run/docker.sock".to_string(),
        status: EndpointStatus::Disconnected,
        tags: vec!["relay".to_string(), "auto".to_string()],
        cert_path: None,
        stacks_dir: None,
    };
    match registry.create(ep).await {
        Ok(created) => {
            tracing::info!(%hostname, endpoint_id = %created.id, "created default endpoint for unauthenticated relay");
            created.id
        }
        Err(e) => {
            tracing::warn!(%hostname, error = %e, "failed to create default endpoint for relay");
            new_id // Return ID even if create partially failed
        }
    }
}

pub async fn relay_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<crate::AppState>>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |socket| handle_relay_connection(socket, state))
}

async fn handle_relay_connection(mut socket: WebSocket, state: Arc<crate::AppState>) {
    tracing::info!("relay connected");

    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<RelayCommand>();
    let sessions = Mutex::new(SessionManager::new());
    let current_session_id = Mutex::new(None::<String>);
    let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
    let mut registered_hostname: Option<String> = None;

    // Spawn dedicated task that polls cmd_rx independently of the select! loop.
    // This prevents tokio::select! starvation where socket.recv() (heartbeats)
    // always wins over cmd_rx.recv(). The spawned task forwards commands to a
    // proxy channel that the select! can poll fairly.
    let cmd_rx_diag = cmd_rx;
    let (diag_tx, mut diag_rx) = mpsc::unbounded_channel::<RelayCommand>();
    let _cmd_tx_alive = cmd_tx.clone();
    tokio::spawn(async move {
        let mut rx = cmd_rx_diag;
        loop {
            match rx.recv().await {
                Some(cmd) => {
                    if diag_tx.send(cmd).is_err() { break; }
                }
                None => break,
            }
        }
    });

    loop {
        tokio::select! {
            cmd = diag_rx.recv() => {
                match cmd {
                    Some(cmd) => {
                        let msg_id = cmd.message.id.clone();
                        if let Some(stream_tx) = cmd.stream_tx {
                            pending.lock().await.insert(msg_id.clone(), stream_tx);
                        } else {
                            let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
                            pending.lock().await.insert(msg_id.clone(), tx);
                            let response_tx = cmd.response_tx;
                            tokio::spawn(async move {
                                while let Some(msg) = rx.recv().await {
                                    if msg.msg_type == relay_protocol::MessageType::Response {
                                        let _ = response_tx.send(msg);
                                        break;
                                    }
                                }
                            });
                        }
                        let sig = {
                            let sid_guard = current_session_id.lock().await;
                            if let Some(ref sid) = *sid_guard {
                                let canonical = serde_json::to_string(&cmd.message).unwrap();
                                let mut sm = sessions.lock().await;
                                sm.sign(sid, canonical.as_bytes()).map(|b| hex::encode(b))
                            } else { None }
                        };
                        let signed = SignedMessage::new(cmd.message, sig);
                        let json = serde_json::to_string(&signed).unwrap();
                        if socket.send(AxumMsg::Text(json.into())).await.is_err() {
                            tracing::warn!(msg_id = %msg_id, "ws send failed");
                            break;
                        }
                    }
                    None => break,
                }
            }

            msg = socket.recv() => {
                match msg {
                    Some(Ok(AxumMsg::Text(text))) => {
                        let signed: SignedMessage = match serde_json::from_str(&text) {
                            Ok(s) => s, Err(e) => { tracing::warn!(error = %e, "parse error"); continue; }
                        };
                        let msg = signed.message;

                        if msg.msg_type == relay_protocol::MessageType::Response
                            || msg.msg_type == relay_protocol::MessageType::Event
                        {
                            let msg_id = msg.id.clone();
                            let is_response = msg.msg_type == relay_protocol::MessageType::Response;
                            if let Some(tx) = pending.lock().await.get(&msg_id) {
                                let _ = tx.send(msg);
                                if is_response { pending.lock().await.remove(&msg_id); }
                                continue;
                            }
                        }

                        {
                            let sid = current_session_id.lock().await;
                            if let Some(ref sid) = *sid {
                                let mut sessions = sessions.lock().await;
                                match signed.signature {
                                    Some(ref sig) => {
                                        let canonical = serde_json::to_string(&msg).unwrap();
                                        match hex::decode(sig) {
                                            Ok(sig_bytes) => {
                                                if !sessions.verify(sid, canonical.as_bytes(), &sig_bytes)
                                                    { tracing::warn!("invalid HMAC"); continue; }
                                                if !sessions.check_nonce(sid, &msg.id)
                                                    { tracing::warn!("replay"); continue; }
                                            }
                                            Err(_) => { tracing::warn!("invalid sig hex"); continue; }
                                        }
                                    }
                                    None => { tracing::warn!("missing signature"); continue; }
                                }
                            }
                        }

                        if msg.subtype == "register" {
                            let token = msg.payload.get("token").and_then(|v| v.as_str()).unwrap_or("");
                            let endpoint_id = match auth::validate_registration_token(state.registry.db(), token) {
                                Ok(eid) => eid,
                                Err(e) => {
                                    tracing::warn!(error = %e, "registration failed");
                                    let err = Message::new_error(msg.id, "AUTH.INVALID", &e, None);
                                    let _ = socket.send(AxumMsg::Text(
                                        serde_json::to_string(&SignedMessage::unsigned(err)).unwrap().into())).await;
                                    continue;
                                }
                            };
                            if let Some(hi) = msg.payload.get("host_info") {
                                let hostname = hi.get("hostname").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
                                if !hostname.is_empty() {
                                    relays().lock().await.insert(hostname.clone(), RelayState {
                                        cmd_tx: cmd_tx.clone(),
                                        info: RelayInfo {
                                            connected: true, hostname: hostname.clone(),
                                            docker_version: hi.get("docker_version").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                            arch: hi.get("arch").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                            os: hi.get("os").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                            relay_version: hi.get("relay_version").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                            connected_at: Utc::now().to_rfc3339(), endpoint_id: Some(endpoint_id.clone()),
                                        },
                                    });
                                    registered_hostname = Some(hostname);
                                }
                            }
                            let session_id = uuid::Uuid::new_v4().to_string();
                            let mut sm = sessions.lock().await;
                            let session_key = sm.create_session(session_id.clone(), endpoint_id);
                            *current_session_id.lock().await = Some(session_id.clone());
                            let resp = Message::new_response(msg.id, "register_ok",
                                serde_json::json!({"session_id": session_id, "session_key": hex::encode(&session_key)}));
                            let canonical = serde_json::to_string(&resp).unwrap();
                            let sig = { let sg = current_session_id.lock().await;
                                sg.as_ref().and_then(|sid| sessions.try_lock().ok().and_then(|s| s.sign(sid, canonical.as_bytes())))
                            }.map(|s| hex::encode(s));
                            let _ = socket.send(AxumMsg::Text(
                                serde_json::to_string(&SignedMessage::new(resp, sig)).unwrap().into())).await;
                            continue;
                        }

                        let resp = if msg.subtype == "ping" {
                            if registered_hostname.is_none() {
                                if let Some(h) = msg.payload.get("hostname").and_then(|v| v.as_str()) {
                                    let hn = h.to_lowercase();
                                    if !hn.is_empty() {
                                        let eid = resolve_endpoint_for_hostname(&state.registry, &hn).await;
                                        relays().lock().await.insert(hn.clone(), RelayState {
                                            cmd_tx: cmd_tx.clone(),
                                            info: RelayInfo {
                                                connected: true, hostname: hn.clone(),
                                                docker_version: msg.payload.get("docker_version").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                arch: msg.payload.get("arch").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                os: msg.payload.get("os").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                relay_version: msg.payload.get("relay_version").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                connected_at: Utc::now().to_rfc3339(), endpoint_id: Some(eid),
                                            },
                                        });
                                        registered_hostname = Some(hn);
                                    }
                                }
                            }
                            Message::new_response(msg.id, "pong",
                                serde_json::to_value(PongResponse { uptime_secs: 0, docker_version: "26.1.3".into(),
                                    arch: "aarch64".into(), os: "linux".into(), relay_version: None }).unwrap())
                        } else {
                            Message::new_error(msg.id, "NOT_IMPLEMENTED", &format!("Unknown subtype: {}", msg.subtype), None)
                        };
                        let canonical = serde_json::to_string(&resp).unwrap();
                        let sig = { let sg = current_session_id.lock().await;
                            sg.as_ref().and_then(|sid| sessions.try_lock().ok().and_then(|s| s.sign(sid, canonical.as_bytes())))
                        }.map(|s| hex::encode(s));
                        let _ = socket.send(AxumMsg::Text(
                            serde_json::to_string(&SignedMessage::new(resp, sig)).unwrap().into())).await;
                    }
                    Some(Ok(AxumMsg::Close(_))) => break,
                    Some(Ok(_)) => {},
                    Some(Err(e)) => { tracing::warn!(error = %e, "ws error"); break; }
                    None => break,
                }
            }
        }
    }

    if let Some(hostname) = registered_hostname {
        relays().lock().await.remove(&hostname);
        tracing::info!(%hostname, "relay disconnected");
    } else {
        tracing::info!("relay disconnected (was not registered)");
    }
}


/// Send a command to a relay and get back a stream of ALL messages (Events +
/// final Response). Unlike `send_relay_command` which returns only the final
/// response, this returns an mpsc receiver that yields every Event as it
/// arrives, followed by the final Response.
pub async fn send_relay_command_streaming(
    hostname: &str,
    msg: Message,
) -> Result<tokio::sync::mpsc::UnboundedReceiver<Message>, String> {
    let (stream_tx, stream_rx) = tokio::sync::mpsc::unbounded_channel();
    let (dummy_tx, _dummy_rx) = oneshot::channel();

    let cmd_tx = {
        let guard = relays().lock().await;
        guard
            .get(hostname)
            .ok_or_else(|| format!("No relay connected with hostname: {}", hostname))?
            .cmd_tx
            .clone()
    };

    cmd_tx
        .send(RelayCommand {
            message: msg,
            response_tx: dummy_tx,
            stream_tx: Some(stream_tx),
        })
        .map_err(|_| "Relay disconnected while sending command".to_string())?;

    Ok(stream_rx)
}

/// WebSocket streaming endpoint for relay — /relay/stream/{hostname}
///
/// The browser sends JSON relay commands (same format as POST /relay/send)
/// and receives all Events + the final Response in real time over the
/// WebSocket.
pub async fn stream_handler(
    ws: WebSocketUpgrade,
    Path(hostname): Path<String>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |socket| handle_stream(socket, hostname))
}

async fn handle_stream(mut socket: WebSocket, hostname: String) {
    tracing::info!(%hostname, "relay stream connected");

    loop {
        let msg = socket.recv().await;

        match msg {
            Some(Ok(AxumMsg::Text(text))) => {
                // Parse incoming command from browser
                let cmd: Message = match serde_json::from_str(&text) {
                    Ok(m) => m,
                    Err(e) => {
                        let err = serde_json::json!({"error": format!("Invalid JSON: {}", e)});
                        let _ = socket.send(AxumMsg::Text(err.to_string().into())).await;
                        continue;
                    }
                };

                // Forward to relay and stream back all responses
                match send_relay_command_streaming(&hostname, cmd).await {
                    Ok(mut rx) => {
                        // Drain all events + final response, sending each to browser
                        while let Some(msg) = rx.recv().await {
                            let json = serde_json::to_string(&msg).unwrap_or_default();
                            if socket.send(AxumMsg::Text(json.into())).await.is_err() {
                                tracing::warn!(%hostname, "browser disconnected during stream");
                                return;
                            }
                            // Stop after the final response
                            if msg.msg_type == relay_protocol::MessageType::Response {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        let err = serde_json::json!({"error": e});
                        let _ = socket.send(AxumMsg::Text(err.to_string().into())).await;
                    }
                }
            }
            Some(Ok(AxumMsg::Close(_))) | None => {
                tracing::info!(%hostname, "relay stream disconnected");
                return;
            }
            _ => {}
        }
    }
}
