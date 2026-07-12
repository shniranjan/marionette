//! Volume transfer handlers for relay-agent.
//!
//! Implements the WSS-proxy transfer pattern:
//! - `volume.transfer_out` — streams a Docker volume as base64-encoded tar via events
//! - `volume.transfer_in` — receives base64-encoded tar and extracts into a Docker volume
//!
//! Phase 1: transfer via WSS proxy (marionette relays data between endpoints).
//! Uses ephemeral alpine containers with tar pipe via bollard exec.

use base64::Engine;
use bollard::container::{
    Config, CreateContainerOptions, LogOutput, RemoveContainerOptions,
};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::models::HostConfig;
use bollard::volume::CreateVolumeOptions;
use futures_util::StreamExt;
use relay_protocol::Message;
use sha2::{Digest, Sha256};
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Handle `volume.transfer_out` — source relay.
///
/// 1. Parse `VolumeTransferOutRequest { volume, target_relay }` from payload.
/// 2. Verify volume exists via bollard `inspect_volume`.
/// 3. Create ephemeral alpine container with volume mounted read-only at `/from`.
/// 4. Exec `tar czf - -C /from .` with stdout attached.
/// 5. Stream tar bytes as base64-encoded events via `event_tx`.
/// 6. Return `VolumeTransferOutResponse { transfer_id, bytes_transferred, checksum }`.
/// 7. Cleanup: remove ephemeral container.
pub async fn handle_volume_transfer_out(
    msg: Message,
    event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    let docker = crate::docker::docker_client();

    let volume = msg
        .payload
        .get("volume")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if volume.is_empty() {
        return Some(Message::new_error(
            msg.id,
            "VOLUME.TRANSFER_FAILED",
            "Missing 'volume' field",
            None,
        ));
    }

    // Verify volume exists
    if let Err(e) = docker.inspect_volume(volume).await {
        return Some(Message::new_error(
            msg.id,
            "VOLUME.NOT_FOUND",
            format!("Volume '{}' not found: {}", volume, e),
            None,
        ));
    }

    let transfer_id = Uuid::new_v4().to_string();
    let container_name = format!("mari-xfer-out-{}", &transfer_id[..8]);

    // Track transfer start for debug.transfer observability
    crate::ops::track_transfer_start(&transfer_id, volume, "out");

    let msg_id = msg.id.clone();
    let event_tx = event_tx.clone();
    let vol = volume.to_string();

    let start = Instant::now();

    // ── Create ephemeral alpine container with volume mounted ro at /from ──
    let container = match docker
        .create_container(
            Some(CreateContainerOptions {
                name: container_name.clone(),
                platform: None,
            }),
            Config {
                image: Some("alpine:latest"),
                entrypoint: Some(vec![
                    "sh".into(),
                    "-c".into(),
                    "sleep 3600".into(),
                ]),
                host_config: Some(HostConfig {
                    binds: Some(vec![format!("{}:/from:ro", vol)]),
                    auto_remove: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
    {
        Ok(c) => c,
        Err(e) => {
            return Some(Message::new_error(
                msg.id,
                "VOLUME.TRANSFER_FAILED",
                format!("Failed to create container: {}", e),
                None,
            ));
        }
    };

    // Start container
    if let Err(e) = docker
        .start_container::<String>(&container.id, None)
        .await
    {
        let _ = docker
            .remove_container(&container.id, None::<RemoveContainerOptions>)
            .await;
        return Some(Message::new_error(
            msg.id,
            "VOLUME.TRANSFER_FAILED",
            format!("Failed to start container: {}", e),
            None,
        ));
    }

    // ── Exec tar czf - -C /from . ──
    let exec = match docker
        .create_exec(
            &container.id,
            CreateExecOptions::<String> {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                cmd: Some(vec![
                    "tar".into(),
                    "czf".into(),
                    "-".into(),
                    "-C".into(),
                    "/from".into(),
                    ".".into(),
                ]),
                ..Default::default()
            },
        )
        .await
    {
        Ok(e) => e,
        Err(e) => {
            let _ = docker
                .remove_container(&container.id, None::<RemoveContainerOptions>)
                .await;
            return Some(Message::new_error(
                msg.id,
                "VOLUME.TRANSFER_FAILED",
                format!("Failed to create exec: {}", e),
                None,
            ));
        }
    };

    let mut output = match docker.start_exec(&exec.id, None).await {
        Ok(StartExecResults::Attached { output, .. }) => output,
        Ok(StartExecResults::Detached) => {
            let _ = docker
                .remove_container(&container.id, None::<RemoveContainerOptions>)
                .await;
            return Some(Message::new_error(
                msg.id,
                "VOLUME.TRANSFER_FAILED",
                "Exec detached unexpectedly",
                None,
            ));
        }
        Err(e) => {
            let _ = docker
                .remove_container(&container.id, None::<RemoveContainerOptions>)
                .await;
            return Some(Message::new_error(
                msg.id,
                "VOLUME.TRANSFER_FAILED",
                format!("Failed to start exec: {}", e),
                None,
            ));
        }
    };

    // ── Read tar stdout, base64-encode chunks, stream as events ──
    let mut hasher = Sha256::new();
    let mut total_bytes: u64 = 0;
    let b64_engine = base64::engine::general_purpose::STANDARD;

    while let Some(item) = output.next().await {
        match item {
            Ok(LogOutput::StdOut { message }) => {
                hasher.update(&message);
                total_bytes += message.len() as u64;

                let encoded = b64_engine.encode(&message);
                let _ = event_tx.send(Message::new_event(
                    &msg_id,
                    "volume.transfer.progress",
                    serde_json::json!({
                        "bytes_sent": total_bytes,
                        "chunk": encoded,
                    }),
                ));
            }
            Ok(LogOutput::StdErr { message }) => {
                let text = String::from_utf8_lossy(&message);
                tracing::warn!(target: "transfer_out", "tar stderr: {}", text.trim());
            }
            _ => {}
        }
    }

    let checksum = format!("{:x}", hasher.finalize());
    let duration_ms = start.elapsed().as_millis() as u64;

    // ── Cleanup ──
    let _ = docker
        .remove_container(&container.id, None::<RemoveContainerOptions>)
        .await;

    tracing::info!(
        transfer_id = %transfer_id,
        volume = %vol,
        bytes = total_bytes,
        checksum = %checksum,
        duration_ms = duration_ms,
        "volume.transfer_out complete"
    );

    // Mark transfer complete for debug.transfer observability
    crate::ops::track_transfer_complete(&transfer_id);

    Some(Message::new_response(
        msg.id,
        "volume.transfer_out",
        serde_json::json!({
            "transfer_id": transfer_id,
            "bytes_transferred": total_bytes,
            "checksum": checksum,
            "duration_ms": duration_ms,
        }),
    ))
}

/// Handle `volume.transfer_in` — target relay.
///
/// 1. Parse `VolumeTransferInRequest { transfer_id, volume }` from payload.
///    Also reads optional `data` (base64-encoded tar) and `checksum` fields.
/// 2. Create target volume if it does not exist.
/// 3. Create ephemeral alpine container with volume mounted at `/to`.
/// 4. Exec `tar xzf - -C /to` with stdin attached.
/// 5. Decode base64 data, write to stdin in chunks.
/// 6. Track progress, verify checksum.
/// 7. Return `VolumeTransferInResponse { volume, bytes_received, checksum_match }`.
/// 8. Cleanup: remove ephemeral container.
pub async fn handle_volume_transfer_in(
    msg: Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    let docker = crate::docker::docker_client();

    let volume = msg
        .payload
        .get("volume")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let transfer_id = msg
        .payload
        .get("transfer_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let data_b64 = msg
        .payload
        .get("data")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let expected_checksum = msg
        .payload
        .get("checksum")
        .and_then(|v| v.as_str());

    if volume.is_empty() {
        return Some(Message::new_error(
            msg.id,
            "VOLUME.TRANSFER_FAILED",
            "Missing 'volume' field",
            None,
        ));
    }

    if data_b64.is_empty() {
        return Some(Message::new_error(
            msg.id,
            "VOLUME.TRANSFER_FAILED",
            "Missing 'data' field (base64-encoded tar stream)",
            None,
        ));
    }

    // Track transfer for debug.transfer observability
    crate::ops::track_transfer_start(transfer_id, volume, "in");

    // Decode base64 data
    let b64_engine = base64::engine::general_purpose::STANDARD;
    let data = match b64_engine.decode(data_b64) {
        Ok(d) => d,
        Err(e) => {
            return Some(Message::new_error(
                msg.id,
                "VOLUME.TRANSFER_FAILED",
                format!("Base64 decode failed: {}", e),
                None,
            ));
        }
    };

    let start = Instant::now();

    // ── Create target volume if not exists ──
    if docker.inspect_volume(volume).await.is_err() {
        if let Err(e) = docker
            .create_volume(CreateVolumeOptions {
                name: volume.to_string(),
                ..Default::default()
            })
            .await
        {
            return Some(Message::new_error(
                msg.id,
                "VOLUME.TRANSFER_FAILED",
                format!("Failed to create volume '{}': {}", volume, e),
                None,
            ));
        }
        tracing::info!(volume = %volume, "created target volume");
    }

    let container_name = format!(
        "mari-xfer-in-{}",
        &transfer_id
            .chars()
            .chain(std::iter::repeat('0'))
            .take(8)
            .collect::<String>()
    );

    // ── Create ephemeral alpine container with volume mounted at /to ──
    let container = match docker
        .create_container(
            Some(CreateContainerOptions {
                name: container_name.clone(),
                platform: None,
            }),
            Config {
                image: Some("alpine:latest"),
                entrypoint: Some(vec![
                    "sh".into(),
                    "-c".into(),
                    "sleep 3600".into(),
                ]),
                host_config: Some(HostConfig {
                    binds: Some(vec![format!("{}:/to", volume)]),
                    auto_remove: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
    {
        Ok(c) => c,
        Err(e) => {
            return Some(Message::new_error(
                msg.id,
                "VOLUME.TRANSFER_FAILED",
                format!("Failed to create container: {}", e),
                None,
            ));
        }
    };

    // Start container
    if let Err(e) = docker
        .start_container::<String>(&container.id, None)
        .await
    {
        let _ = docker
            .remove_container(&container.id, None::<RemoveContainerOptions>)
            .await;
        return Some(Message::new_error(
            msg.id,
            "VOLUME.TRANSFER_FAILED",
            format!("Failed to start container: {}", e),
            None,
        ));
    }

    // ── Exec tar xzf - -C /to with stdin attached ──
    let exec = match docker
        .create_exec(
            &container.id,
            CreateExecOptions::<String> {
                attach_stdin: Some(true),
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                cmd: Some(vec![
                    "tar".into(),
                    "xzf".into(),
                    "-".into(),
                    "-C".into(),
                    "/to".into(),
                ]),
                ..Default::default()
            },
        )
        .await
    {
        Ok(e) => e,
        Err(e) => {
            let _ = docker
                .remove_container(&container.id, None::<RemoveContainerOptions>)
                .await;
            return Some(Message::new_error(
                msg.id,
                "VOLUME.TRANSFER_FAILED",
                format!("Failed to create exec: {}", e),
                None,
            ));
        }
    };

    let mut input = match docker.start_exec(&exec.id, None).await {
        Ok(StartExecResults::Attached { input, .. }) => input,
        Ok(StartExecResults::Detached) => {
            let _ = docker
                .remove_container(&container.id, None::<RemoveContainerOptions>)
                .await;
            return Some(Message::new_error(
                msg.id,
                "VOLUME.TRANSFER_FAILED",
                "Exec detached unexpectedly",
                None,
            ));
        }
        Err(e) => {
            let _ = docker
                .remove_container(&container.id, None::<RemoveContainerOptions>)
                .await;
            return Some(Message::new_error(
                msg.id,
                "VOLUME.TRANSFER_FAILED",
                format!("Failed to start exec: {}", e),
                None,
            ));
        }
    };

    // ── Write decoded data to stdin and compute checksum ──
    let mut hasher = Sha256::new();
    let bytes_received = data.len() as u64;

    // Write in 64KB chunks to avoid overwhelming the pipe
    const CHUNK_SIZE: usize = 65536;
    let mut write_result: Result<(), String> = Ok(());

    for chunk in data.chunks(CHUNK_SIZE) {
        hasher.update(chunk);
        if let Err(e) = input.write_all(chunk).await {
            write_result = Err(format!("Failed to write to stdin: {}", e));
            break;
        }
    }

    let computed_checksum = format!("{:x}", hasher.finalize());

    // Close stdin
    let _ = input.shutdown().await;

    // Check write result
    if let Err(e) = write_result {
        let _ = docker
            .remove_container(&container.id, None::<RemoveContainerOptions>)
            .await;
        return Some(Message::new_error(msg.id, "VOLUME.TRANSFER_FAILED", e, None));
    }

    // ── Verify checksum ──
    let checksum_match = expected_checksum
        .map(|expected| expected == computed_checksum)
        .unwrap_or(true); // If no checksum provided, assume match

    let duration_ms = start.elapsed().as_millis() as u64;

    // ── Cleanup ──
    let _ = docker
        .remove_container(&container.id, None::<RemoveContainerOptions>)
        .await;

    tracing::info!(
        transfer_id = %transfer_id,
        volume = %volume,
        bytes = bytes_received,
        checksum = %computed_checksum,
        checksum_match = checksum_match,
        duration_ms = duration_ms,
        "volume.transfer_in complete"
    );

    // Mark transfer complete for debug.transfer observability
    crate::ops::track_transfer_complete(transfer_id);

    Some(Message::new_response(
        msg.id,
        "volume.transfer_in",
        serde_json::json!({
            "volume": volume,
            "bytes_received": bytes_received,
            "checksum_match": checksum_match,
            "checksum": computed_checksum,
            "duration_ms": duration_ms,
        }),
    ))
}
