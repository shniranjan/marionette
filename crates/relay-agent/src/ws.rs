use crate::auth::AuthState;
use crate::config::Config;
use crate::handlers;
use crate::signed::SignedMessage;
use futures_util::{SinkExt, StreamExt};
use relay_protocol::{Message, MessageType};
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMsg};

pub async fn connect_loop(cfg: Config) -> anyhow::Result<()> {
    loop {
        match connect_and_serve(&cfg).await {
            Ok(()) => tracing::info!("connection closed cleanly, reconnecting..."),
            Err(e) => tracing::warn!(error = %e, "connection error, reconnecting..."),
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

async fn connect_and_serve(cfg: &Config) -> anyhow::Result<()> {
    let url = &cfg.relay.marionette_url;
    tracing::info!(%url, "connecting to marionette");

    let (ws_stream, _response) = connect_async(url).await?;
    let (mut write, mut read) = ws_stream.split();

    tracing::info!("connected");

    let mut auth = AuthState::new();

    // ── Registration ──────────────────────────────────────────────
    if let Some(ref token) = cfg.relay.token {
        let host_info = serde_json::json!({
            "hostname": hostname::get()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            "relay_version": env!("CARGO_PKG_VERSION"),
            "docker_version": "unknown",
            "arch": std::env::consts::ARCH,
            "os": std::env::consts::OS,
            "cpus": 1,
            "memory_bytes": 0
        });

        let register_msg = Message::new_request(
            "reg-001",
            "register",
            serde_json::json!({"token": token, "host_info": host_info}),
        );
        let signed = SignedMessage::unsigned(register_msg);
        let json = serde_json::to_string(&signed)?;
        write.send(WsMsg::Text(json.into())).await?;
        tracing::info!("registration request sent");
    } else {
        tracing::info!("no relay token configured, operating unauthenticated");
    }

    // ── Send initial ping to verify connectivity ──────────────────
    let ping = Message::new_request("ping-001", "ping", serde_json::json!({}));
    let signed_ping = SignedMessage::unsigned(ping);
    let json = serde_json::to_string(&signed_ping)?;
    write.send(WsMsg::Text(json.into())).await?;
    tracing::info!("sent initial ping");

    // ── Heartbeat + message loop ───────────────────────────────────
    let mut heartbeat = tokio::time::interval(
        std::time::Duration::from_secs(cfg.relay.heartbeat_interval_secs),
    );
    // Skip the first immediate tick
    heartbeat.tick().await;

    loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(WsMsg::Text(text))) => {
                        match serde_json::from_str::<SignedMessage>(&text) {
                            Ok(signed) => {
                                // Verify signature if we're authenticated
                                if auth.is_authenticated() {
                                    match &signed.signature {
                                        Some(sig) => {
                                            let canonical = serde_json::to_string(&signed.message)?;
                                            match hex::decode(sig) {
                                                Ok(sig_bytes) => {
                                                    if !auth.verify(canonical.as_bytes(), &sig_bytes) {
                                                        tracing::warn!("invalid signature, dropping message");
                                                        continue;
                                                    }
                                                }
                                                Err(e) => {
                                                    tracing::warn!(error = %e, "invalid hex in signature, dropping message");
                                                    continue;
                                                }
                                            }
                                        }
                                        None => {
                                            tracing::warn!("missing signature on authenticated connection, dropping message");
                                            continue;
                                        }
                                    }
                                }

                                // Intercept register_ok to extract session key
                                if signed.message.msg_type == MessageType::Response
                                    && signed.message.subtype == "register_ok"
                                {
                                    if let Some(payload) = signed.message.payload.as_object() {
                                        if let (Some(sid), Some(skey_hex)) = (
                                            payload.get("session_id").and_then(|v| v.as_str()),
                                            payload.get("session_key").and_then(|v| v.as_str()),
                                        ) {
                                            match hex::decode(skey_hex) {
                                                Ok(skey) => {
                                                    auth.session_id = Some(sid.to_string());
                                                    auth.session_key = Some(skey);
                                                    tracing::info!(session_id = %sid, "registration successful, session established");
                                                    continue;
                                                }
                                                Err(e) => {
                                                    tracing::warn!(error = %e, "invalid session_key hex in register_ok");
                                                    continue;
                                                }
                                            }
                                        }
                                    }
                                    tracing::warn!("register_ok missing session_id or session_key");
                                    continue;
                                }

                                // Also intercept error during registration
                                if !auth.is_authenticated()
                                    && signed.message.msg_type == MessageType::Response
                                    && signed.message.subtype == "error"
                                {
                                    let err_msg = signed.message.payload
                                        .get("message")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown error");
                                    tracing::error!(error = %err_msg, "registration failed");
                                    return Err(anyhow::anyhow!("Registration failed: {}", err_msg));
                                }

                                // Dispatch to handlers
                                let response = handlers::dispatch(signed.message).await;
                                if let Some(resp) = response {
                                    let canonical = serde_json::to_string(&resp)?;
                                    let sig = if auth.is_authenticated() {
                                        auth.sign(canonical.as_bytes()).map(|s| hex::encode(s))
                                    } else {
                                        None
                                    };
                                    let signed_resp = SignedMessage::new(resp, sig);
                                    let json = serde_json::to_string(&signed_resp)?;
                                    write.send(WsMsg::Text(json.into())).await?;
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to parse signed message");
                            }
                        }
                    }
                    Some(Ok(WsMsg::Close(_))) => {
                        tracing::info!("server closed connection");
                        return Ok(());
                    }
                    Some(Ok(_other)) => {
                        // Binary, Ping, Pong, Frame — silently ignore for now
                    }
                    Some(Err(e)) => return Err(e.into()),
                    None => return Ok(()),
                }
            }
            _ = heartbeat.tick() => {
                let hb_id = format!("hb-{}", uuid::Uuid::new_v4());
                let ping = Message::new_request(
                    &hb_id,
                    "ping",
                    serde_json::json!({}),
                );
                let signed_ping = SignedMessage::unsigned(ping);
                let json = serde_json::to_string(&signed_ping).unwrap_or_default();
                if write.send(WsMsg::Text(json.into())).await.is_err() {
                    return Err(anyhow::anyhow!("heartbeat send failed"));
                }
            }
        }
    }
}
