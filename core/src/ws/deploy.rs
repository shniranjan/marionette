use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Path, Query, State},
    response::IntoResponse,
};
use futures::stream::StreamExt;
use futures::SinkExt;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::helpers;
use crate::models::EndpointQuery;

/// Messages flowing from the docker compose process to the WS loop.
enum StreamMessage {
    Line(String),
    Exit(i32),
}

/// GET /stacks/:name/deploy/stream?endpoint=local (WebSocket upgrade)
/// Runs `docker compose up -d` in the stack directory and streams
/// stdout/stderr line-by-line to the client. Sends a final completion
/// message when the command finishes.
pub async fn deploy_stream_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<crate::AppState>>,
    Path(stack_name): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_deploy_stream(socket, state, stack_name, params))
}

async fn handle_deploy_stream(
    mut socket: WebSocket,
    state: Arc<crate::AppState>,
    stack_name: String,
    params: EndpointQuery,
) {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;

    // Verify endpoint connectivity
    {
        if let Err((_, json)) = helpers::resolve_client(&state, Some(&endpoint_id)).await {
            let _ = socket
                .send(Message::Text(
                    json.to_string().into(),
                ))
                .await;
            let _ = socket.close().await;
            return;
        }
    }

    let stack_dir = state.stacks_dir.join(&stack_name);
    if !stack_dir.exists() {
        let _ = socket
            .send(Message::Text(
                serde_json::json!({"error": format!("Stack '{}' not found", stack_name)})
                    .to_string()
                    .into(),
            ))
            .await;
        let _ = socket.close().await;
        return;
    }

    // Spawn `docker compose up -d` in the stack directory
    let mut child = match Command::new("docker")
        .args(["compose", "up", "-d"])
        .current_dir(&stack_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = socket
                .send(Message::Text(
                    serde_json::json!({"error": format!("Failed to start docker compose: {}", e)})
                        .to_string()
                        .into(),
                ))
                .await;
            let _ = socket.close().await;
            return;
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Channel to funnel stdout lines, stderr lines, and exit code to the WS loop
    let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<StreamMessage>();

    // Spawn a reader for stdout
    if let Some(stdout) = stdout {
        let tx = msg_tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if tx.send(StreamMessage::Line(line)).is_err() {
                    break;
                }
            }
        });
    }

    // Spawn a reader for stderr
    if let Some(stderr) = stderr {
        let tx = msg_tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if tx
                    .send(StreamMessage::Line(format!("[stderr] {}", line)))
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    // Spawn a task to wait for the child process and report the exit code
    tokio::spawn(async move {
        let status = child.wait().await;
        let code = status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
        let _ = msg_tx.send(StreamMessage::Exit(code));
    });

    // Split the WebSocket so we can detect client disconnect
    let (mut sender, mut receiver) = socket.split();

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    // Send initial status
    let _ = sender
        .send(Message::Text(
            serde_json::json!({"type": "status", "message": "Deploying..."})
                .to_string()
                .into(),
        ))
        .await;

    let mut exit_code: Option<i32> = None;

    // Main event loop: stream lines to WS, wait for exit code,
    // detect client disconnect.
    loop {
        tokio::select! {
            msg = msg_rx.recv() => {
                match msg {
                    Some(StreamMessage::Line(text)) => {
                        let json = serde_json::json!({"type": "output", "line": text});
                        if sender
                            .send(Message::Text(json.to_string().into()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Some(StreamMessage::Exit(code)) => {
                        exit_code = Some(code);
                        // The channel will close once all senders are dropped,
                        // which happens after stdout/stderr readers finish
                        // and the exit message is sent. Continue looping
                        // to drain any remaining lines that arrived before exit.
                    }
                    None => {
                        // Channel closed — all senders dropped.
                        // This means stdout, stderr, AND exit have all been handled.
                        break;
                    }
                }
            }
            _ = &mut recv_task => {
                // Client disconnected — drop out immediately
                return;
            }
        }
    }

    // Send final completion message
    let final_code = exit_code.unwrap_or(-1);
    let final_msg = serde_json::json!({
        "type": "complete",
        "exit_code": final_code,
        "stack": stack_name
    });
    let _ = sender
        .send(Message::Text(final_msg.to_string().into()))
        .await;
    let _ = sender.close().await;
}
