use axum::extract::ws::{Message as AxumMsg, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use relay_protocol::payloads::PongResponse;
use relay_protocol::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::relay::auth;
use crate::relay::session::SessionManager;
use crate::relay::signed::SignedMessage;

/// Pending relay requests awaiting response.
pub type PendingMap = Arc<Mutex<HashMap<String, oneshot::Sender<Message>>>>;

/// Channel for sending commands to the relay handler — replaced on each reconnect.
static RELAY_TX: std::sync::OnceLock<Mutex<Option<mpsc::UnboundedSender<RelayCommand>>>> =
    std::sync::OnceLock::new();

fn relay_tx() -> &'static Mutex<Option<mpsc::UnboundedSender<RelayCommand>>> {
    RELAY_TX.get_or_init(|| Mutex::new(None))
}

pub struct RelayCommand {
    pub message: Message,
    pub response_tx: oneshot::Sender<Message>,
}

/// Send a command to the connected relay and wait for response.
pub async fn send_relay_command(msg: Message) -> Result<Message, String> {
    let (response_tx, response_rx) = oneshot::channel();

    let tx = {
        let guard = relay_tx().lock().await;
        guard
            .as_ref()
            .ok_or("No relay connected")?
            .clone()
    };

    tx.send(RelayCommand {
        message: msg,
        response_tx,
    })
    .map_err(|_| "Relay disconnected while sending command".to_string())?;

    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        response_rx,
    )
    .await
    .map_err(|_| "Relay response timeout".to_string())?
    .map_err(|_| "Relay dropped response channel".to_string())
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
    *relay_tx().lock().await = Some(cmd_tx);

    let sessions = Mutex::new(SessionManager::new());
    let current_session_id = Mutex::new(None::<String>);

    // Pending map for request-response matching
    let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));

    loop {
        tokio::select! {
            // Command from API → forward to relay
            Some(cmd) = cmd_rx.recv() => {
                let msg_id = cmd.message.id.clone();
                pending.lock().await.insert(msg_id, cmd.response_tx);

                let signed = SignedMessage::unsigned(cmd.message);
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

                        // Check if this is a response to a pending command
                        if msg.msg_type == relay_protocol::MessageType::Response {
                            if let Some(response_tx) = pending.lock().await.remove(&msg.id) {
                                let _ = response_tx.send(msg);
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
                        let (subtype, response) = if msg.subtype == "register" {
                            let token = msg.payload.get("token")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");

                            match auth::validate_registration_token(state.registry.db(), token) {
                                Ok(endpoint_id) => {
                                    let session_id = uuid::Uuid::new_v4().to_string();
                                    let mut sm = sessions.lock().await;
                                    let session_key = sm.create_session(
                                        session_id.clone(),
                                        endpoint_id,
                                    );
                                    *current_session_id.lock().await = Some(session_id.clone());
                                    tracing::info!(%session_id, "relay registered successfully");

                                    ("register_ok", serde_json::json!({
                                        "session_id": session_id,
                                        "session_key": hex::encode(&session_key)
                                    }))
                                }
                                Err(e) => {
                                    tracing::warn!(error = %e, "registration failed");
                                    ("error", serde_json::json!({
                                        "error_code": "AUTH.INVALID",
                                        "message": e
                                    }))
                                }
                            }
                        } else if msg.subtype == "ping" {
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

    // Clear the command channel on disconnect
    *relay_tx().lock().await = None;
    tracing::info!("relay disconnected");
}
