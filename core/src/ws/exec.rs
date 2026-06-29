use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Path, Query, State},
    response::IntoResponse,
};
use bollard::container::LogOutput;
use bollard::exec::{CreateExecOptions, StartExecResults};
use futures::stream::StreamExt;
use futures::SinkExt;
use serde::Deserialize;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use crate::helpers;

/// Query parameters for container exec WebSocket.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecQuery {
    #[serde(default = "default_cmd")]
    pub cmd: String,
    #[serde(default)]
    pub endpoint: Option<String>,
}

fn default_cmd() -> String {
    "bash".to_string()
}

/// Messages from the output reader to the WS send loop.
enum OutputMsg {
    Data(String),
    Done,
}

/// GET /api/containers/{id}/exec?cmd=bash&endpoint=X → WebSocket upgrade
/// Streams stdin/stdout/stderr between the client and a container exec session.
pub async fn container_exec_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<crate::AppState>>,
    Path(container_id): Path<String>,
    Query(params): Query<ExecQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_exec_stream(socket, state, container_id, params))
}

async fn handle_exec_stream(
    mut socket: WebSocket,
    state: Arc<crate::AppState>,
    container_id: String,
    params: ExecQuery,
) {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;

    let docker = {
        match helpers::resolve_client(&state, Some(&endpoint_id)).await {
            Ok(d) => d,
            Err((_, json)) => {
                let _ = socket
                    .send(Message::Text(json.to_string().into()))
                    .await;
                let _ = socket.close().await;
                return;
            }
        }
    };

    let cmd: Vec<&str> = params.cmd.split_whitespace().collect();
    if cmd.is_empty() {
        let _ = socket
            .send(Message::Text(
                serde_json::json!({"error": "No command specified"}).to_string().into(),
            ))
            .await;
        let _ = socket.close().await;
        return;
    }

    // Create an exec instance
    let exec = match docker
        .create_exec(
            &container_id,
            CreateExecOptions {
                attach_stdin: Some(true),
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                tty: Some(true),
                cmd: Some(cmd),
                ..Default::default()
            },
        )
        .await
    {
        Ok(e) => e,
        Err(e) => {
            let _ = socket
                .send(Message::Text(
                    serde_json::json!({"error": format!("Failed to create exec: {}", e)})
                        .to_string()
                        .into(),
                ))
                .await;
            let _ = socket.close().await;
            return;
        }
    };

    // Start the exec and attach
    let (mut output, mut input) = match docker.start_exec(&exec.id, None).await {
        Ok(StartExecResults::Attached { output, input }) => (output, input),
        Ok(StartExecResults::Detached) => {
            let _ = socket
                .send(Message::Text(
                    serde_json::json!({"error": "Exec session detached (unexpected)"})
                        .to_string()
                        .into(),
                ))
                .await;
            let _ = socket.close().await;
            return;
        }
        Err(e) => {
            let _ = socket
                .send(Message::Text(
                    serde_json::json!({"error": format!("Failed to start exec: {}", e)})
                        .to_string()
                        .into(),
                ))
                .await;
            let _ = socket.close().await;
            return;
        }
    };

    // Channel to forward output from reader task to WS sender
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<OutputMsg>();

    // Task 1: Read exec output and send through channel
    tokio::spawn(async move {
        while let Some(item) = output.next().await {
            let text = match item {
                Ok(LogOutput::StdOut { message }) => {
                    String::from_utf8_lossy(&message).to_string()
                }
                Ok(LogOutput::StdErr { message }) => {
                    String::from_utf8_lossy(&message).to_string()
                }
                Ok(_) => continue,
                Err(e) => format!("\r\n[exec error: {}]\r\n", e),
            };
            if out_tx.send(OutputMsg::Data(text)).is_err() {
                break;
            }
        }
        let _ = out_tx.send(OutputMsg::Done);
    });

    // Split the WebSocket for bidirectional handling
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Task 2: Forward WebSocket → exec input
    let mut input_handle = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if input.write_all(text.as_bytes()).await.is_err() {
                        break;
                    }
                }
                Ok(Message::Binary(data)) => {
                    if input.write_all(&data).await.is_err() {
                        break;
                    }
                }
                Ok(Message::Close(_)) => break,
                _ => {}
            }
        }
        let _ = input.shutdown().await;
    });

    // Main loop: forward output channel → WebSocket, detect close
    loop {
        tokio::select! {
            msg = out_rx.recv() => {
                match msg {
                    Some(OutputMsg::Data(text)) => {
                        if ws_sender
                            .send(Message::Text(text.into()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Some(OutputMsg::Done) | None => {
                        // Exec output stream ended
                        break;
                    }
                }
            }
            _ = &mut input_handle => {
                // Input task ended (client disconnected or error)
                break;
            }
        }
    }

    // Graceful close
    let _ = ws_sender.close().await;
}
