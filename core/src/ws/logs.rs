use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Path, Query, State},
    response::IntoResponse,
};
use bollard::container::{LogOutput, LogsOptions};
use futures::stream::StreamExt;
use futures::SinkExt;
use std::sync::Arc;

use crate::docker::get_client;
use crate::models::EndpointQuery;

/// GET /containers/:id/logs?endpoint=local (WebSocket upgrade)
/// Streams container logs to the client as JSON messages.
pub async fn container_logs_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<crate::AppState>>,
    Path(container_id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_logs_stream(socket, state, container_id, params))
}

async fn handle_logs_stream(
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

    let mut logs_stream = docker.logs(
        &container_id,
        Some(LogsOptions::<String> {
            follow: true,
            stdout: true,
            stderr: true,
            tail: "100".to_string(),
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

    // Stream logs to the WebSocket
    loop {
        tokio::select! {
            item = logs_stream.next() => {
                match item {
                    Some(Ok(log_output)) => {
                        let msg_text = match log_output {
                            LogOutput::StdOut { message } => {
                                String::from_utf8_lossy(&message).to_string()
                            }
                            LogOutput::StdErr { message } => {
                                String::from_utf8_lossy(&message).to_string()
                            }
                            _ => continue,
                        };
                        let json = serde_json::json!({"stream": msg_text});
                        if sender
                            .send(Message::Text(json.to_string().into()))
                            .await
                            .is_err()
                        {
                            break;
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
