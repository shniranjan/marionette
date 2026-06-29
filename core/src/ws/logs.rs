use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Path, Query, State},
    response::IntoResponse,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    Json,
};
use bollard::container::{LogOutput, LogsOptions};
use futures::stream::StreamExt;
use futures::SinkExt;
use serde::Deserialize;
use std::sync::Arc;

use crate::helpers;
use crate::models::EndpointQuery;

/// Query parameters for log download.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadQuery {
    #[serde(default = "default_tail")]
    pub tail: String,
    #[serde(default)]
    pub timestamps: bool,
    #[serde(default)]
    pub endpoint: Option<String>,
}

fn default_tail() -> String {
    "all".to_string()
}

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
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;

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

/// GET /api/containers/{id}/logs/download?tail=all&timestamps=true&endpoint=local
/// Downloads container logs as a text file.
pub async fn download_logs(
    State(state): State<Arc<crate::AppState>>,
    Path(container_id): Path<String>,
    Query(params): Query<DownloadQuery>,
) -> Result<(HeaderMap, Vec<u8>), (StatusCode, Json<serde_json::Value>)> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    let mut logs_stream = docker.logs(
        &container_id,
        Some(LogsOptions::<String> {
            follow: false,
            stdout: true,
            stderr: true,
            tail: params.tail.clone(),
            timestamps: params.timestamps,
            ..Default::default()
        }),
    );

    let mut output: Vec<u8> = Vec::new();
    while let Some(item) = logs_stream.next().await {
        match item {
            Ok(LogOutput::StdOut { message }) | Ok(LogOutput::StdErr { message }) => {
                output.extend_from_slice(&message);
                if !message.ends_with(b"\n") {
                    output.push(b'\n');
                }
            }
            Err(e) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                ));
            }
            _ => continue,
        }
    }

    // Resolve container name for a human-friendly filename
    let filename = match docker.inspect_container(&container_id, None).await {
        Ok(info) => {
            let name = info.name.unwrap_or_else(|| container_id.clone());
            let name = name.trim_start_matches('/');
            format!("{}-logs.txt", name)
        }
        Err(_) => format!("container-{}-logs.txt", container_id),
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename))
            .unwrap_or(HeaderValue::from_static("attachment")),
    );

    Ok((headers, output))
}
