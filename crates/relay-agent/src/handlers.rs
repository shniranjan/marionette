use relay_protocol::{ErrorCode, ErrorPayload, Message};
use tokio::sync::mpsc;

use crate::docker::*;

/// Dispatch an incoming message to the appropriate handler.
///
/// Routes messages by subtype across all operation namespaces.
/// Unknown subtypes fall through to the `RELAY.NOT_IMPLEMENTED` catch-all.
pub async fn dispatch(
    msg: &Message,
    event_tx: &mpsc::UnboundedSender<Message>,
) -> Result<Message, ErrorPayload> {
    tracing::debug!(subtype = %msg.subtype, "dispatching message");

    crate::ops::increment_calls();

    match msg.subtype.as_str() {
        // ── Docker operations ─────────────────────────────────
        "docker.ps" => handle_docker_ps(msg, event_tx).await,
        "docker.inspect" => handle_docker_inspect(msg, event_tx).await,
        "docker.stop" => handle_docker_stop(msg, event_tx).await,
        "docker.start" => handle_docker_start(msg, event_tx).await,
        "docker.restart" => handle_docker_restart(msg, event_tx).await,
        "docker.exec" => handle_docker_exec(msg, event_tx).await,
        "docker.logs" => handle_docker_logs(msg, event_tx).await,
        "docker.stats" => handle_docker_stats(msg, event_tx).await,

        // ── Ops handlers (compose, fs, host, image, relay, debug, volume) ──
        "compose.up" | "compose.down" | "compose.logs" | "compose.config"
        | "fs.list" | "fs.read" | "fs.write"
        | "host.info"
        | "image.ensure"
        | "relay.audit" | "relay.update"
        | "relay.debug.state" | "relay.debug.stats" | "relay.debug.transfer"
        | "relay.debug.events" | "relay.debug.replay" | "relay.debug.config"
        | "volume.transfer_out" | "volume.transfer_in" => {
            crate::ops::dispatch(msg.clone(), event_tx).await.ok_or_else(|| {
                crate::ops::increment_errors();
                ErrorPayload::new(
                    ErrorCode::InternalError,
                    format!("RELAY.NOT_IMPLEMENTED: operation '{}'", msg.subtype),
                )
            })
        }

        // ── Protocol-level ────────────────────────────────────
        "ping" | "register" | "pong" => Ok(Message::new_response(
            msg.id.clone(),
            "pong",
            serde_json::json!({"status": "ok"}),
        )),

        // ── Catch-all ─────────────────────────────────────────
        _ => {
            crate::ops::increment_errors();
            Err(ErrorPayload::new(
                ErrorCode::InternalError,
                format!("RELAY.NOT_IMPLEMENTED: operation '{}'", msg.subtype),
            ))
        }
    }
}
