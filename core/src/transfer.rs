// ── Direct Proxy Volume Transfer Engine ──────────────────────────
// Pipes Docker volumes between two bollard clients via tar streams.
// No SSH. No temp files. Streaming, not buffering.

use bollard::Docker;
use bollard::container::{
    Config, CreateContainerOptions, LogOutput, LogsOptions, RemoveContainerOptions,
    StartContainerOptions,
};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::models::HostConfig;
use futures::StreamExt;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;

/// Request to transfer one or more volumes between endpoints.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferRequest {
    pub source_endpoint: String,
    pub target_endpoint: String,
    pub transfers: Vec<VolumeTransfer>,
    /// Compression: "none", "gzip", "zstd", "lz4" (default: "gzip")
    #[serde(default = "default_compression")]
    pub compression: String,
}

fn default_compression() -> String {
    "gzip".to_string()
}

/// A single volume to transfer.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeTransfer {
    pub source_volume: String,
    pub target_volume: String,
    /// Custom extract path inside the target container (default: "/data")
    #[serde(default = "default_target_path")]
    pub target_path: String,
}

fn default_target_path() -> String {
    "/data".to_string()
}

/// Result of a single volume transfer.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferResult {
    pub source_volume: String,
    pub target_volume: String,
    pub target_path: String,
    pub bytes_transferred: u64,
    pub status: String, // "success" | "failed" | "skipped"
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// Aggregate result for a batch transfer request.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchTransferResult {
    pub results: Vec<TransferResult>,
    pub total_bytes: u64,
    pub status: String, // "success" | "partial_success" | "failed"
}

/// Errors that can occur during transfer.
#[derive(Debug)]
pub enum TransferError {
    Docker(String),
    Io(String),
    Timeout(String),
    VolumeNotFound(String),
}

impl std::fmt::Display for TransferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Docker(e) => write!(f, "Docker error: {}", e),
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::Timeout(e) => write!(f, "Timeout: {}", e),
            Self::VolumeNotFound(v) => write!(f, "Volume not found: {}", v),
        }
    }
}

/// Compression flag mapping for tar commands.
fn tar_compression_flag(compression: &str) -> (&str, &str) {
    match compression {
        "zstd" => ("--zstd", "tar --zstd"),
        "lz4" => ("--lz4", "tar --lz4"),
        "none" => ("", "tar"),
        _ => ("z", "tar czf"), // default: gzip
    }
}

/// Transfer a single Docker volume from source to target via bollard exec pipe.
///
/// Creates ephemeral alpine containers on both ends, mounts the volumes,
/// and pipes tar stdout → stdin through Marionette's memory.
pub async fn transfer_volume(
    source: &Docker,
    target: &Docker,
    vol: &VolumeTransfer,
    compression: &str,
    overall_timeout: Duration,
    _idle_timeout: Duration,
) -> TransferResult {
    let start = Instant::now();
    let target_name = &vol.target_volume;
    let target_path = &vol.target_path;
    let (tar_flag, _tar_desc) = tar_compression_flag(compression);

    // Build tar create command as a shell string
    let tar_create_line = if tar_flag.is_empty() {
        "tar c - -C /from .".to_string()
    } else {
        format!("tar c{}f - -C /from .", tar_flag)
    };

    // Build tar extract command as a shell string
    let tar_extract_line = if tar_flag.is_empty() {
        format!("tar x - -C {}", target_path)
    } else {
        format!("tar x{}f - -C {}", tar_flag, target_path)
    };

    // Shell commands for container execution
    let tar_create_cmd: Vec<&str> = vec!["sh", "-c", &tar_create_line];
    let tar_extract_cmd: Vec<&str> = vec!["sh", "-c", &tar_extract_line];

    let fail = |msg: &str, dur: u64| TransferResult {
        source_volume: vol.source_volume.clone(),
        target_volume: target_name.clone(),
        target_path: target_path.clone(),
        bytes_transferred: 0,
        status: "failed".to_string(),
        error: Some(msg.to_string()),
        duration_ms: dur,
    };

    // ── Step 1: Verify source volume exists ──────────────────
    let vol_exists = source.inspect_volume(&vol.source_volume).await.is_ok();
    if !vol_exists {
        return fail(
            &format!("Source volume '{}' not found", vol.source_volume),
            start.elapsed().as_millis() as u64,
        );
    }

    // ── Step 2: Create source container ──────────────────────
    let src_name = format!(
        "mari-src-{}",
        &vol.source_volume.chars().take(20).collect::<String>()
    );
    let src_container = match source
        .create_container(
            Some(CreateContainerOptions {
                name: src_name.clone(),
                platform: None,
            }),
            Config {
                image: Some("alpine:latest"),
                cmd: Some(tar_create_cmd.clone()),
                host_config: Some(HostConfig {
                    binds: Some(vec![format!("{}:/from:ro", vol.source_volume)]),
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
            return fail(
                &format!("Failed to create source container: {}", e),
                start.elapsed().as_millis() as u64,
            )
        }
    };

    // ── Step 3: Create target container ──────────────────────
    let tgt_name = format!(
        "mari-tgt-{}",
        &target_name.chars().take(20).collect::<String>()
    );
    let tgt_container = match target
        .create_container(
            Some(CreateContainerOptions {
                name: tgt_name.clone(),
                platform: None,
            }),
            Config {
                image: Some("alpine:latest"),
                cmd: Some(tar_extract_cmd.clone()),
                host_config: Some(HostConfig {
                    binds: Some(vec![format!("{}:/to", target_name)]),
                    auto_remove: Some(true),
                    ..Default::default()
                }),
                attach_stdin: Some(true),
                open_stdin: Some(true),
                ..Default::default()
            },
        )
        .await
    {
        Ok(c) => c,
        Err(e) => {
            let _ = source
                .remove_container(&src_container.id, None::<RemoveContainerOptions>)
                .await;
            return fail(
                &format!("Failed to create target container: {}", e),
                start.elapsed().as_millis() as u64,
            );
        }
    };

    // ── Step 4: Start source container ───────────────────────
    if let Err(e) = source
        .start_container(&src_container.id, None::<StartContainerOptions<String>>)
        .await
    {
        cleanup_containers(source, target, &src_container.id, &tgt_container.id).await;
        return fail(
            &format!("Failed to start source container: {}", e),
            start.elapsed().as_millis() as u64,
        );
    }

    // ── Step 5: Start target container ───────────────────────
    if let Err(e) = target
        .start_container(&tgt_container.id, None::<StartContainerOptions<String>>)
        .await
    {
        cleanup_containers(source, target, &src_container.id, &tgt_container.id).await;
        return fail(
            &format!("Failed to start target container: {}", e),
            start.elapsed().as_millis() as u64,
        );
    }

    // ── Step 6: Attach to source container logs ──────────────
    // Use container logs approach — LogOutput::StdOut { message } has accessible byte data.
    // logs() returns impl Stream directly — errors come through stream items, not the return.
    let mut src_logs = source.logs(
        &src_container.id,
        Some(LogsOptions::<String> {
            follow: true,
            stdout: true,
            stderr: true,
            ..Default::default()
        }),
    );

    // ── Step 7: Create exec on target for stdin ──────────────
    let tgt_exec = match target
        .create_exec(
            &tgt_container.id,
            CreateExecOptions {
                attach_stdin: Some(true),
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                cmd: Some(tar_extract_cmd.clone()),
                ..Default::default()
            },
        )
        .await
    {
        Ok(e) => e,
        Err(e) => {
            cleanup_containers(source, target, &src_container.id, &tgt_container.id).await;
            return fail(
                &format!("Failed to create target exec: {}", e),
                start.elapsed().as_millis() as u64,
            );
        }
    };

    // ── Step 8: Start target exec ────────────────────────────
    let mut tgt_input = match target.start_exec(&tgt_exec.id, None).await {
        Ok(StartExecResults::Attached { input, .. }) => input,
        Ok(StartExecResults::Detached) => {
            cleanup_containers(source, target, &src_container.id, &tgt_container.id).await;
            return fail(
                "Target exec detached unexpectedly",
                start.elapsed().as_millis() as u64,
            );
        }
        Err(e) => {
            cleanup_containers(source, target, &src_container.id, &tgt_container.id).await;
            return fail(
                &format!("Failed to start target exec: {}", e),
                start.elapsed().as_millis() as u64,
            );
        }
    };

    // ── Step 9: Pipe bytes ───────────────────────────────────
    let mut bytes: u64 = 0;
    let mut last_byte_time = Instant::now();

    let pipe_result: Result<u64, String> = async {
        while let Some(chunk) = src_logs.next().await {
            // Check overall timeout
            if start.elapsed() > overall_timeout {
                return Err(format!(
                    "Overall timeout ({:?}) exceeded after {} bytes",
                    overall_timeout, bytes
                ));
            }

            match chunk {
                Ok(LogOutput::StdOut { message }) => {
                    // Write raw bytes to target stdin
                    if let Err(e) = tgt_input.write_all(&message).await {
                        return Err(format!(
                            "Failed to write to target stdin after {} bytes: {}",
                            bytes, e
                        ));
                    }
                    bytes += message.len() as u64;
                    last_byte_time = Instant::now();
                }
                Ok(LogOutput::StdErr { message }) => {
                    let text = String::from_utf8_lossy(&message);
                    tracing::warn!("Source tar stderr: {}", text);
                }
                Ok(_) => {
                    // Console or other log types — ignore
                }
                Err(e) => {
                    return Err(format!("Source log stream error: {}", e));
                }
            }
        }
        Ok(bytes)
    }
    .await;

    // ── Step 10: Close stdin and cleanup ─────────────────────
    let _ = tgt_input.shutdown().await;
    cleanup_containers(source, target, &src_container.id, &tgt_container.id).await;

    match pipe_result {
        Ok(b) => TransferResult {
            source_volume: vol.source_volume.clone(),
            target_volume: target_name.clone(),
            target_path: target_path.clone(),
            bytes_transferred: b,
            status: "success".to_string(),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Err(e) => fail(&e, start.elapsed().as_millis() as u64),
    }
}

/// Clean up source and target containers (best-effort, ignore errors).
async fn cleanup_containers(source: &Docker, target: &Docker, src_id: &str, tgt_id: &str) {
    let _ = source.remove_container(src_id, None::<RemoveContainerOptions>).await;
    let _ = target.remove_container(tgt_id, None::<RemoveContainerOptions>).await;
}

/// Transfer multiple volumes sequentially, aggregating results.
pub async fn transfer_volumes_batch(
    source: &Docker,
    target: &Docker,
    transfers: &[VolumeTransfer],
    compression: &str,
) -> BatchTransferResult {
    let mut results = Vec::with_capacity(transfers.len());
    let mut total_bytes: u64 = 0;

    for vol in transfers {
        tracing::info!(
            "Transferring volume: {} → {} (path: {})",
            vol.source_volume,
            vol.target_volume,
            vol.target_path
        );

        let result = transfer_volume(
            source,
            target,
            vol,
            compression,
            Duration::from_secs(600), // 10 min overall
            Duration::from_secs(60),  // 60s idle timeout
        )
        .await;

        if result.status == "success" {
            total_bytes += result.bytes_transferred;
        }

        results.push(result);
    }

    let all_success = results.iter().all(|r| r.status == "success");
    let any_success = results.iter().any(|r| r.status == "success");

    BatchTransferResult {
        results,
        total_bytes,
        status: if all_success {
            "success".to_string()
        } else if any_success {
            "partial_success".to_string()
        } else {
            "failed".to_string()
        },
    }
}
