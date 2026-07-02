use axum::extract::ws::{Message as AxumMsg, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use relay_protocol::payloads::PongResponse;
use relay_protocol::Message;
use std::sync::Arc;

use crate::relay::auth;
use crate::relay::session::SessionManager;
use crate::relay::signed::SignedMessage;

pub async fn relay_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<crate::AppState>>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |socket| handle_relay_connection(socket, state))
}

async fn handle_relay_connection(mut socket: WebSocket, state: Arc<crate::AppState>) {
    tracing::info!("relay connected");

    let mut sessions = SessionManager::new();
    let mut current_session_id: Option<String> = None;

    while let Some(msg) = socket.recv().await {
        match msg {
            Ok(AxumMsg::Text(text)) => {
                let signed: SignedMessage = match serde_json::from_str(&text) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to parse signed message");
                        continue;
                    }
                };

                let msg = signed.message;

                // Verify signature if we have an active session
                if let Some(ref sid) = current_session_id {
                    match signed.signature {
                        Some(ref sig) => {
                            let canonical =
                                serde_json::to_string(&msg).unwrap();
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
                            // Nonce check (use message id as nonce)
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

                // Build response
                let (subtype, response) = if msg.subtype == "register" {
                    let token = msg.payload.get("token")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    match auth::validate_registration_token(state.registry.db(), token) {
                        Ok(endpoint_id) => {
                            let session_id = uuid::Uuid::new_v4().to_string();
                            let session_key = sessions.create_session(
                                session_id.clone(),
                                endpoint_id,
                            );
                            current_session_id = Some(session_id.clone());
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

                let resp_msg =
                    Message::new_response(msg.id, subtype, response);
                let canonical = serde_json::to_string(&resp_msg).unwrap();
                let sig = current_session_id
                    .as_ref()
                    .and_then(|sid| sessions.sign(sid, canonical.as_bytes()))
                    .map(|s| hex::encode(s));

                let signed_resp = SignedMessage::new(resp_msg, sig);
                let json = serde_json::to_string(&signed_resp).unwrap();
                let _ = socket.send(AxumMsg::Text(json.into())).await;
            }
            Ok(AxumMsg::Close(_)) => break,
            Ok(_) => {} // ignore binary/ping/pong
            Err(e) => {
                tracing::warn!(error = %e, "websocket error");
                break;
            }
        }
    }

    tracing::info!("relay disconnected");
}
