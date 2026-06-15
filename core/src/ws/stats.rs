use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Path, Query, State},
    response::IntoResponse,
};
use bollard::container::StatsOptions;
use futures::stream::StreamExt;
use futures::SinkExt;
use std::sync::Arc;

use crate::docker::get_client;
use crate::models::EndpointQuery;

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
    let endpoint_id = params
        .endpoint
        .unwrap_or_else(|| state.default_endpoint.clone());

    let docker = {
        let clients = state.clients.read().await;
        match get_client(&endpoint_id, &clients).await {
            Ok(d) => d,
            Err(e) => {
                let _ = socket
                    .send(Message::Text(
                        serde_json::json!({"error": e}).to_string().into(),
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
