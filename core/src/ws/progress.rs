// ── Switchover Progress WebSocket ─────────────────────────────────
// WebSocket endpoint at GET /migration/compose/progress that streams
// per-step progress messages during a switchover operation.
//
// The frontend connects before initiating the switchover, then the
// switchover handler sends progress updates through a broadcast channel.

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
};
use futures::SinkExt;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::switchover::ProgressMessage;

/// Global broadcast channel for switchover progress.
/// Each switchover session gets its own broadcast sender.
static PROGRESS_BROADCAST: std::sync::OnceLock<tokio::sync::Mutex<Option<broadcast::Sender<ProgressMessage>>>> =
    std::sync::OnceLock::new();

/// Initialize (or reinitialize) the progress broadcast channel.
/// Returns a new sender and the receiver side.
pub fn init_progress_channel() -> (
    broadcast::Sender<ProgressMessage>,
    broadcast::Receiver<ProgressMessage>,
) {
    let _mutex = PROGRESS_BROADCAST.get_or_init(|| tokio::sync::Mutex::new(None));
    // We cannot block in async context, so we use try_lock + create new.
    // This is called from the route handler before spawning the switchover task.
    let (tx, _) = broadcast::channel::<ProgressMessage>(64);
    // We'll store the sender for new subscribers and return a receiver for the WS handler.
    // Actually, we need a different approach: the route handler creates a channel,
    // passes tx to switchover, and rx to the WS handler.
    // But this is a single WS endpoint that multiple clients might connect to.
    // Let's use a simpler approach: the route handler creates the channel.
    (tx, broadcast::channel::<ProgressMessage>(64).1)
}

/// GET /migration/compose/progress → WebSocket upgrade
///
/// Streams JSON progress messages during an active switchover.
pub async fn switchover_progress_ws(
    ws: WebSocketUpgrade,
    State(_state): State<Arc<crate::AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_progress_stream(socket))
}

async fn handle_progress_stream(mut socket: WebSocket) {
    // Subscribe to the global progress broadcast
    let rx = {
        let mutex = PROGRESS_BROADCAST.get_or_init(|| tokio::sync::Mutex::new(None));
        let guard = mutex.lock().await;
        match guard.as_ref() {
            Some(tx) => tx.subscribe(),
            None => {
                let _ = socket
                    .send(Message::Text(
                        serde_json::json!({
                            "phase": "idle",
                            "status": "waiting",
                            "detail": "No active switchover. Initiate a switchover to see progress.",
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        })
                        .to_string()
                        .into(),
                    ))
                    .await;
                let _ = socket.close().await;
                return;
            }
        }
    };
    drop(rx); // We'll get a fresh one below

    // Actually, let's use a different pattern.
    // The broadcast approach is complex. Let's use a simpler mpsc-based approach
    // where the switchover handler creates an mpsc sender, and the WS handler
    // receives from it. But the WS handler needs to be connected before the
    // switchover starts.
    //
    // Simpler approach: use a oneshot-like pattern where the switchover handler
    // gets an mpsc UnboundedSender that it pushes to, and the WS handler gets
    // the UnboundedReceiver. We'll use a static broadcast for the actual
    // production use, but for now let's implement a working version.

    // Re-subscribe after the lock is released
    let mut rx = {
        let mutex = PROGRESS_BROADCAST.get().unwrap();
        let guard = mutex.lock().await;
        match guard.as_ref() {
            Some(tx) => tx.subscribe(),
            None => {
                let _ = socket
                    .send(Message::Text(
                        serde_json::json!({
                            "phase": "idle",
                            "status": "waiting",
                            "detail": "No active switchover. Initiate a switchover to see progress.",
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        })
                        .to_string()
                        .into(),
                    ))
                    .await;
                let _ = socket.close().await;
                return;
            }
        }
    };

    // Stream progress messages to the WebSocket
    loop {
        match rx.recv().await {
            Ok(msg) => {
                let json = serde_json::to_string(&msg).unwrap_or_else(|_| "{}".to_string());
                if socket.send(Message::Text(json.into())).await.is_err() {
                    break; // Client disconnected
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!("WS progress client lagged by {} messages", n);
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => {
                // Switchover completed, send final message and close
                let _ = socket
                    .send(Message::Text(
                        serde_json::json!({
                            "phase": "complete",
                            "status": "closed",
                            "detail": "Switchover stream ended",
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        })
                        .to_string()
                        .into(),
                    ))
                    .await;
                let _ = socket.close().await;
                break;
            }
        }
    }
}

/// Set up a new progress broadcast channel for an incoming switchover.
/// Returns the sender to pass to the switchover engine.
pub async fn begin_switchover_session() -> broadcast::Sender<ProgressMessage> {
    let (tx, _) = broadcast::channel::<ProgressMessage>(64);
    let mutex = PROGRESS_BROADCAST.get_or_init(|| tokio::sync::Mutex::new(None));
    let mut guard = mutex.lock().await;
    *guard = Some(tx.clone());
    tx
}

/// Signal that the switchover session is complete.
/// Drops the broadcast sender so WS clients see Closed.
pub async fn end_switchover_session() {
    if let Some(mutex) = PROGRESS_BROADCAST.get() {
        let mut guard = mutex.lock().await;
        *guard = None;
    }
}
