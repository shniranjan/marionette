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
            response_tx,
            stream_tx: None,
        })
        .map_err(|_| "Relay disconnected while sending command".to_string())?;

    tokio::time::timeout(std::time::Duration::from_secs(30), response_rx)
        .await
        .map_err(|_| "Relay response timeout".to_string())?
        .map_err(|_| "Relay dropped response channel".to_string())
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

    // Set up command channel for this relay session
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<RelayCommand>();

    let sessions = Mutex::new(SessionManager::new());
    let current_session_id = Mutex::new(None::<String>);

    // Pending map for request-response matching
    let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));

    // Track whether we've registered this relay in the RELAYS map
    let mut registered_hostname: Option<String> = None;

    loop {
        tokio::select! {
            // Command from API → forward to relay
            Some(cmd) = cmd_rx.recv() => {
                let msg_id = cmd.message.id.clone();
                if let Some(stream_tx) = cmd.stream_tx {
                    // Streaming mode: insert the caller's mpsc sender directly
                    // so all events AND the final response flow to the caller.
                    pending.lock().await.insert(msg_id, stream_tx);
                } else {
                    // Non-streaming mode: drain events, forward only final Response
                    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
                    pending.lock().await.insert(msg_id, tx);
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
                // Sign the command if we have an active session (required by relay)
                let sig = {
                    let sid_guard = current_session_id.lock().await;
                    if let Some(ref sid) = *sid_guard {
                        let canonical = serde_json::to_string(&cmd.message).unwrap();
                        let mut sm = sessions.lock().await;
                        sm.sign(sid, canonical.as_bytes()).map(|b| hex::encode(b))
                    } else {
                        None
                    }
                };
                let signed = SignedMessage::new(cmd.message, sig);
                let json = serde_json::to_string(&signed).unwrap();
                if socket.send(AxumMsg::Text(json.into())).await.is_err() {
                    tracing::warn!("failed to forward command to relay");
                    break;
                }
            }

            // Message from relay → handle or resolve pending
            msg = socket.recv() => {
                match msg {
                    Some(Ok(AxumMsg::Text(text))) => {
                        let signed: SignedMessage = match serde_json::from_str(&text) {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to parse signed message");
                                continue;
                            }
                        };

                        let msg = signed.message;

                        // Check if this is a response or event for a pending command
                        if msg.msg_type == relay_protocol::MessageType::Response
                            || msg.msg_type == relay_protocol::MessageType::Event
                        {
                            let msg_id = msg.id.clone();
                            let is_response = msg.msg_type == relay_protocol::MessageType::Response;
                            if let Some(tx) = pending.lock().await.get(&msg_id) {
                                let _ = tx.send(msg);
                                if is_response {
                                    pending.lock().await.remove(&msg_id);
                                }
                                continue;
                            }
                        }

                        // Verify signature if we have an active session
                        {
                            let sid = current_session_id.lock().await;
                            if let Some(ref sid) = *sid {
                                let mut sessions = sessions.lock().await;
                                match signed.signature {
                                    Some(ref sig) => {
                                        let canonical = serde_json::to_string(&msg).unwrap();
                                        let sig_bytes = match hex::decode(sig) {
                                            Ok(b) => b,
                                            Err(_) => {
                                                tracing::warn!("invalid signature hex");
                                                continue;
                                            }
                                        };
                                        if !sessions.verify(sid, canonical.as_bytes(), &sig_bytes) {
                                            tracing::warn!("invalid HMAC signature");
                                            continue;
                                        }
                                        if !sessions.check_nonce(sid, &msg.id) {
                                            tracing::warn!(nonce = %msg.id, "replay detected");
                                            continue;
                                        }
                                    }
                                    None => {
                                        tracing::warn!("missing signature on authenticated session");
                                        continue;
                                    }
                                }
                            }
                        }

                        // Build response (for register/ping from relay)
                        if msg.subtype == "register" {
                            let token = msg.payload.get("token")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");

                            // Validate token first to get endpoint_id
                            let endpoint_id = match auth::validate_registration_token(state.registry.db(), token) {
                                Ok(endpoint_id) => endpoint_id,
                                Err(e) => {
                                    tracing::warn!(error = %e, "registration failed");
                                    let err_resp = Message::new_error(msg.id, "AUTH.INVALID", &e, None);
                                    let json = serde_json::to_string(&SignedMessage::unsigned(err_resp)).unwrap();
                                    let _ = socket.send(AxumMsg::Text(json.into())).await;
                                    continue;
                                }
                            };

                            // Capture host_info from the register payload and register in RELAYS
                            if let Some(hi) = msg.payload.get("host_info") {
                                let hostname = hi.get("hostname")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_lowercase();

                                let info = RelayInfo {
                                    connected: true,
                                    hostname: hostname.clone(),
                                    docker_version: hi.get("docker_version").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    arch: hi.get("arch").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    os: hi.get("os").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    relay_version: hi.get("relay_version").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    connected_at: Utc::now().to_rfc3339(),
                                    endpoint_id: Some(endpoint_id.clone()),
                                };

                                if !hostname.is_empty() {
                                    let state = RelayState {
                                        cmd_tx: cmd_tx.clone(),
                                        info,
                                    };
                                    relays().lock().await.insert(hostname.clone(), state);
                                    registered_hostname = Some(hostname);
                                }
                            }

                            let session_id = uuid::Uuid::new_v4().to_string();
                            let mut sm = sessions.lock().await;
                            let session_key = sm.create_session(
                                session_id.clone(),
                                endpoint_id,
                            );
                            *current_session_id.lock().await = Some(session_id.clone());
                            tracing::info!(%session_id, "relay registered successfully");

                            let resp_msg = Message::new_response(
                                msg.id,
                                "register_ok",
                                serde_json::json!({
                                    "session_id": session_id,
                                    "session_key": hex::encode(&session_key)
                                }),
                            );
                            let canonical = serde_json::to_string(&resp_msg).unwrap();
                            let sig = {
                                let sid_guard = current_session_id.lock().await;
                                sid_guard.as_ref().and_then(|sid| {
                                    sessions.try_lock().ok().and_then(|s| s.sign(sid, canonical.as_bytes()))
                                })
                            }
                            .map(|s| hex::encode(s));
                            let signed_resp = SignedMessage::new(resp_msg, sig);
                            let json = serde_json::to_string(&signed_resp).unwrap();
                            let _ = socket.send(AxumMsg::Text(json.into())).await;
                            continue;
                        }

                        let (subtype, response) = if msg.subtype == "ping" {
                            // Capture host_info from ping and register in RELAYS if not yet registered
                            if registered_hostname.is_none() {
                                let hostname = msg.payload.get("hostname")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_lowercase();

                                if !hostname.is_empty() {
                                    // Resolve endpoint_id: match hostname to existing endpoints,
                                    // or create a new default endpoint so unauthenticated relays
                                    // are visible to get_relay_for_endpoint().
                                    let endpoint_id = resolve_endpoint_for_hostname(
                                        &state.registry, &hostname).await;

                                    let info = RelayInfo {
                                        connected: true,
                                        hostname: hostname.clone(),
                                        docker_version: msg.payload.get("docker_version").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                        arch: msg.payload.get("arch").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                        os: msg.payload.get("os").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                        relay_version: msg.payload.get("relay_version").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                        connected_at: Utc::now().to_rfc3339(),
                                        endpoint_id: Some(endpoint_id),
                                    };

                                    let state = RelayState {
                                        cmd_tx: cmd_tx.clone(),
                                        info,
                                    };
                                    relays().lock().await.insert(hostname.clone(), state);
                                    registered_hostname = Some(hostname);
                                }
                            }

                            ("pong", serde_json::to_value(PongResponse {
                                uptime_secs: 0,
                                docker_version: "26.1.3".into(),
                                arch: "aarch64".into(),
                                os: "linux".into(),
                                relay_version: None,
                            }).unwrap())
                        } else {
                            ("error", serde_json::json!({
                                "error_code": "NOT_IMPLEMENTED",
                                "message": format!("Unknown subtype: {}", msg.subtype)
                            }))
                        };

                        let resp_msg = Message::new_response(msg.id, subtype, response);
                        let canonical = serde_json::to_string(&resp_msg).unwrap();
                        let sig = {
                            let sid_guard = current_session_id.lock().await;
                            sid_guard.as_ref().and_then(|sid| {
                                sessions.try_lock().ok().and_then(|s| s.sign(sid, canonical.as_bytes()))
                            })
                        }
                        .map(|s| hex::encode(s));

                        let signed_resp = SignedMessage::new(resp_msg, sig);
                        let json = serde_json::to_string(&signed_resp).unwrap();
                        let _ = socket.send(AxumMsg::Text(json.into())).await;
                    }
                    Some(Ok(AxumMsg::Close(_))) => break,
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        tracing::warn!(error = %e, "websocket error");
                        break;
                    }
                    None => break,
                }
            }
        }
    }

    // Remove this relay from the registry on disconnect
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
