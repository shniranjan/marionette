use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Path, Query, State},
    response::IntoResponse,
};
use bollard::container::StatsOptions;
use futures::stream::StreamExt;
use futures::SinkExt;
use std::sync::Arc;

use crate::helpers;
use crate::models::EndpointQuery;
use uuid::Uuid;

/// GET /containers/:id/stats?endpoint=local (WebSocket upgrade)
/// Streams live container resource statistics to the client as JSON messages.
pub async fn container_stats_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<crate::AppState>>,
    Path(container_id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_stats_stream(socket, state, container_id, params))
}

async fn handle_stats_stream(
    mut socket: WebSocket,
    state: Arc<crate::AppState>,
    container_id: String,
    params: EndpointQuery,
) {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;

    // ── Try relay path first ──────────────────────────────────────
    if let Some(relay_host) =
        crate::ws_relay::get_relay_for_endpoint(&endpoint_id).await
    {
        handle_stats_via_relay(socket, &container_id, &relay_host).await;
        return;
    }

    // ── Fallback: direct bollard (current behavior) ───────────────
    let docker = {
        match helpers::resolve_client(&state, Some(&endpoint_id)).await {
            Ok(d) => d,
            Err((_, json)) => {
                let _ = socket
                    .send(Message::Text(
                        json.to_string().into(),
                    ))
                    .await;
                let _ = socket.close().await;
                return;
            }
        }
    };

    let mut stats_stream = docker.stats(
        &container_id,
        Some(StatsOptions {
            stream: true,
            ..Default::default()
        }),
    );

    // Split the socket so we can detect client disconnect while streaming
    let (mut sender, mut receiver) = socket.split();

    // Spawn a task to handle incoming messages (detect close)
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    // Stream stats to the WebSocket
    loop {
        tokio::select! {
            item = stats_stream.next() => {
                match item {
                    Some(Ok(stats)) => {
                        match serde_json::to_string(&stats) {
                            Ok(json) => {
                                if sender
                                    .send(Message::Text(json.into()))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Err(e) => {
                                let json = serde_json::json!({"error": format!("Serialization failed: {}", e)});
                                let _ = sender.send(Message::Text(json.to_string().into())).await;
                                break;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        let json = serde_json::json!({"error": e.to_string()});
                        let _ = sender.send(Message::Text(json.to_string().into())).await;
                        break;
                    }
                    None => break,
                }
            }
            _ = &mut recv_task => {
                // Client disconnected
                break;
            }
        }
    }

    // Graceful close
    let _ = sender.close().await;
}

/// Stream stats for a container through the relay protocol.
///
/// Sends a `docker.stats` request to the relay and forwards every
/// Event (DockerStatsSnapshot) directly to the browser WebSocket.
async fn handle_stats_via_relay(
    mut socket: WebSocket,
    container_id: &str,
    relay_host: &str,
) {
    use relay_protocol::Message as RelayMessage;
    use relay_protocol::MessageType;

    // Build the docker.stats request for a single container
    let msg = RelayMessage::new_request(
        Uuid::new_v4().to_string(),
        "docker.stats",
        serde_json::json!({
            "containers": [container_id],
            "stream": true,
        }),
    );

    // Open a streaming relay command — returns a receiver that yields
    // every Event message followed by the final Response.
    let mut rx = match crate::ws_relay::send_relay_command_streaming(relay_host, msg).await {
        Ok(rx) => rx,
        Err(e) => {
            let json = serde_json::json!({"error": format!("Relay error: {}", e)});
            let _ = socket.send(Message::Text(json.to_string().into())).await;
            let _ = socket.close().await;
            return;
        }
    };

    // Split the socket so we can detect client disconnect concurrently
    let (mut sender, mut receiver) = socket.split();

    // Spawn a task to handle incoming messages (detect close)
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    // Stream relay events to the browser
    loop {
        tokio::select! {
            item = rx.recv() => {
                match item {
                    Some(relay_msg) => {
                        match relay_msg.msg_type {
                            // Forward event payload (DockerStatsSnapshot JSON) to browser
                            MessageType::Event => {
                                let json = serde_json::to_string(&relay_msg.payload);
                                match json {
                                    Ok(text) => {
                                        if sender.send(Message::Text(text.into())).await.is_err() {
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        let err = serde_json::json!({"error": format!("Serialization failed: {}", e)});
                                        let _ = sender.send(Message::Text(err.to_string().into())).await;
                                        break;
                                    }
                                }
                            }
                            // Final Response — stop streaming
                            MessageType::Response => {
                                break;
                            }
                            // Unexpected message type — ignore or log
                            MessageType::Request => {
                                // Should not happen; relay doesn't send requests back
                                continue;
                            }
                        }
                    }
                    None => break, // relay stream ended
                }
            }
            _ = &mut recv_task => {
                // Client disconnected
                break;
            }
        }
    }

    // Graceful close
    let _ = sender.close().await;
}
