use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Query, State},
    response::IntoResponse,
    http::StatusCode,
    Json,
};
use bollard::container::{LogOutput, LogsOptions};
use futures::stream::StreamExt;
use futures::SinkExt;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

use crate::helpers;

/// Maximum number of containers allowed in a merged log view.
const MAX_MERGED_CONTAINERS: usize = 5;

/// Tagged log message from a single container stream.
struct TaggedLog {
    container_id: String,
    container_name: String,
    text: String,
    eof: bool,
    error: Option<String>,
}

/// Query parameters for merged logs WebSocket.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergedLogsQuery {
    /// Comma-separated container IDs.
    pub ids: String,
    /// Optional endpoint override.
    #[serde(default)]
    pub endpoint: Option<String>,
}

/// GET /api/containers/logs/merged?ids=id1,id2&endpoint=X → WebSocket upgrade
/// Streams merged logs from multiple containers to the client as JSON messages.
/// Each line is prefixed with `[containerName]`.
/// Maximum 5 containers.
pub async fn merged_logs_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<MergedLogsQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // Parse and validate container IDs
    let ids: Vec<String> = params
        .ids
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "No container IDs provided"})),
        ));
    }

    if ids.len() > MAX_MERGED_CONTAINERS {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!(
                    "Too many containers. Maximum {} allowed, got {}",
                    MAX_MERGED_CONTAINERS,
                    ids.len()
                )
            })),
        ));
    }

    Ok(ws.on_upgrade(move |socket| {
        handle_merged_logs_stream(socket, state, ids, params.endpoint)
    }))
}

async fn handle_merged_logs_stream(
    mut socket: WebSocket,
    state: Arc<crate::AppState>,
    container_ids: Vec<String>,
    endpoint: Option<String>,
) {
    let endpoint_id = helpers::resolve_endpoint_id(&state, endpoint.as_deref()).await;

    let docker = match helpers::resolve_client(&state, Some(&endpoint_id)).await {
        Ok(d) => d,
        Err((_, json)) => {
            let _ = socket
                .send(Message::Text(json.to_string().into()))
                .await;
            let _ = socket.close().await;
            return;
        }
    };

    // Resolve container names via inspect (best-effort, fall back to short ID)
    let mut container_names: HashMap<String, String> = HashMap::new();
    for cid in &container_ids {
        let name = match docker.inspect_container(cid, None).await {
            Ok(info) => {
                let raw_name = info.name.unwrap_or_else(|| cid.clone());
                raw_name.trim_start_matches('/').to_string()
            }
            Err(_) => {
                // Fall back to short ID (first 12 chars)
                if cid.len() > 12 {
                    cid[..12].to_string()
                } else {
                    cid.clone()
                }
            }
        };
        container_names.insert(cid.clone(), name);
    }

    // Use an mpsc channel to collect tagged log lines from per-container tasks.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<TaggedLog>();

    // Spawn a task for each container that reads its log stream and sends TaggedLog messages.
    let mut task_handles = Vec::new();
    for cid in &container_ids {
        let cid = cid.clone();
        let docker = docker.clone();
        let tx = tx.clone();
        let name = container_names.get(&cid).cloned().unwrap_or_else(|| cid.clone());

        let handle = tokio::spawn(async move {
            let mut stream = docker.logs(
                &cid,
                Some(LogsOptions::<String> {
                    follow: true,
                    stdout: true,
                    stderr: true,
                    tail: "100".to_string(),
                    ..Default::default()
                }),
            );

            while let Some(item) = stream.next().await {
                match item {
                    Ok(log_output) => {
                        let msg_text = match log_output {
                            LogOutput::StdOut { message } => {
                                String::from_utf8_lossy(&message).to_string()
                            }
                            LogOutput::StdErr { message } => {
                                String::from_utf8_lossy(&message).to_string()
                            }
                            _ => continue,
                        };

                        let trimmed = msg_text.trim_end_matches('\n').to_string();
                        if tx
                            .send(TaggedLog {
                                container_id: cid.clone(),
                                container_name: name.clone(),
                                text: trimmed,
                                eof: false,
                                error: None,
                            })
                            .is_err()
                        {
                            // Receiver dropped; exit task
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(TaggedLog {
                            container_id: cid.clone(),
                            container_name: name.clone(),
                            text: String::new(),
                            eof: false,
                            error: Some(e.to_string()),
                        });
                        break;
                    }
                }
            }

            // Signal EOF for this container
            let _ = tx.send(TaggedLog {
                container_id: cid.clone(),
                container_name: name.clone(),
                text: String::new(),
                eof: true,
                error: None,
            });
        });

        task_handles.push(handle);
    }

    // Drop the original tx so the channel closes when all tasks finish.
    drop(tx);

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

    // Track which containers have sent EOF
    let mut eof_count = 0;
    let total = container_ids.len();

    // Main multiplex loop: read from the mpsc channel and forward to the WebSocket.
    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Some(tagged) => {
                        if tagged.eof {
                            eof_count += 1;
                            // Send EOF notice
                            let json = serde_json::json!({
                                "stream": format!("[{}] --- log stream ended ---", tagged.container_name),
                                "container": tagged.container_name,
                                "containerId": tagged.container_id,
                                "eof": true
                            });
                            let _ = sender.send(Message::Text(json.to_string().into())).await;

                            if eof_count >= total {
                                let _ = sender.close().await;
                                return;
                            }
                        } else if let Some(err) = tagged.error {
                            let json = serde_json::json!({
                                "error": err,
                                "container": tagged.container_name,
                                "containerId": tagged.container_id
                            });
                            let _ = sender.send(Message::Text(json.to_string().into())).await;
                            // Don't count this as EOF — the task already exited after error
                            eof_count += 1;
                            if eof_count >= total {
                                let _ = sender.close().await;
                                return;
                            }
                        } else {
                            let line = format!("[{}] {}", tagged.container_name, tagged.text);
                            let json = serde_json::json!({
                                "stream": line,
                                "container": tagged.container_name,
                                "containerId": tagged.container_id
                            });

                            if sender
                                .send(Message::Text(json.to_string().into()))
                                .await
                                .is_err()
                            {
                                let _ = sender.close().await;
                                return;
                            }
                        }
                    }
                    None => {
                        // Channel closed (all tasks finished)
                        let _ = sender.close().await;
                        return;
                    }
                }
            }
            _ = &mut recv_task => {
                // Client disconnected
                // Abort all spawned stream tasks
                for h in task_handles {
                    h.abort();
                }
                let _ = sender.close().await;
                return;
            }
        }
    }
}
