// ── Switchover Engine ────────────────────────────────────────────
// Orchestrates a live cutover from source to target endpoint:
//   1. Stop source containers
//   2. Transfer volumes
//   3. Deploy target
//   4. Health check polling
//   5. Rollback on failure
//
// Emits per-step progress messages via an mpsc channel for WebSocket
// streaming to the frontend.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use bollard::Docker;
use bollard::container::{
    ListContainersOptions, RemoveContainerOptions, StopContainerOptions,
};
use bollard::models::{HostConfig};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::transfer::{self, VolumeTransfer};

// ── Request / Response ─────────────────────────────────────────────

/// Incoming switchover request.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchoverRequest {
    pub source_endpoint: String,
    pub target_endpoint: String,
    pub stack_name: String,
    pub compose_yaml: String,
    /// Map of source volume name → target volume name
    #[serde(default)]
    pub volumes: HashMap<String, String>,
    /// URL to poll for health after deploy (e.g. "http://target:8080/health")
    pub health_check_url: Option<String>,
    /// Max seconds to wait for health check to pass (default 30)
    #[serde(default = "default_health_timeout")]
    pub health_check_timeout_secs: u64,
}

fn default_health_timeout() -> u64 {
    30
}

/// Final result of a switchover operation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchoverResult {
    pub status: String, // "success" | "failed" | "rolled_back"
    pub steps: Vec<SwitchoverStep>,
    pub rollback_performed: bool,
}

/// A single phase step recorded during switchover.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchoverStep {
    pub phase: String,
    pub status: String, // "started" | "success" | "failed" | "skipped"
    pub detail: String,
    pub duration_ms: u64,
}

// ── Progress Messages ──────────────────────────────────────────────

/// A progress update sent over the mpsc channel to WebSocket clients.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressMessage {
    pub phase: String,
    pub status: String,
    pub detail: String,
    pub timestamp: String, // ISO 8601
}

impl ProgressMessage {
    pub fn new(phase: &str, status: &str, detail: &str) -> Self {
        Self {
            phase: phase.to_string(),
            status: status.to_string(),
            detail: detail.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

// ── Engine ─────────────────────────────────────────────────────────

/// Run the full switchover state machine.
///
/// Progress messages are sent through `progress_tx` (if provided) so that
/// WebSocket endpoints can stream them to the frontend.
pub async fn run_switchover(
    source: &Docker,
    target: &Docker,
    request: &SwitchoverRequest,
    source_stacks_dir: &str,
    target_stacks_dir: &str,
    source_host_stacks_dir: &str,
    _target_host_stacks_dir: &str,
    source_is_local: bool,
    _target_is_local: bool,
    progress_tx: Option<mpsc::UnboundedSender<ProgressMessage>>,
) -> SwitchoverResult {
    let mut steps: Vec<SwitchoverStep> = Vec::new();
    let mut rollback_performed = false;

    let send = |phase: &str, status: &str, detail: &str| {
        if let Some(tx) = &progress_tx {
            let _ = tx.send(ProgressMessage::new(phase, status, detail));
        }
    };

    let record_step = |steps: &mut Vec<SwitchoverStep>, phase: &str, status: &str, detail: &str, duration_ms: u64| {
        steps.push(SwitchoverStep {
            phase: phase.to_string(),
            status: status.to_string(),
            detail: detail.to_string(),
            duration_ms,
        });
    };

    // ── Phase 1: Stop source containers ──────────────────────
    send("stop-source", "started", "Stopping source containers...");
    let t0 = Instant::now();

    match stop_source_containers(source, &request.stack_name).await {
        Ok(count) => {
            let dur = t0.elapsed().as_millis() as u64;
            let detail = format!("Stopped {} container(s)", count);
            send("stop-source", "success", &detail);
            record_step(&mut steps, "stop-source", "success", &detail, dur);
        }
        Err(e) => {
            let dur = t0.elapsed().as_millis() as u64;
            let detail = format!("Failed to stop source: {}", e);
            send("stop-source", "failed", &detail);
            record_step(&mut steps, "stop-source", "failed", &detail, dur);

            return SwitchoverResult {
                status: "failed".to_string(),
                steps,
                rollback_performed: false,
            };
        }
    }

    // ── Phase 2: Transfer volumes ────────────────────────────
    if !request.volumes.is_empty() {
        send("transfer-volumes", "started", "Transferring volumes...");
        let t0 = Instant::now();

        let transfers: Vec<VolumeTransfer> = request
            .volumes
            .iter()
            .map(|(src, tgt)| VolumeTransfer {
                source_volume: src.clone(),
                target_volume: tgt.clone(),
                target_path: "/data".to_string(),
            })
            .collect();

        let result = transfer::transfer_volumes_batch(
            source,
            target,
            &transfers,
            "gzip",
        )
        .await;

        let dur = t0.elapsed().as_millis() as u64;

        if result.status == "failed" {
            let detail = format!(
                "Volume transfer failed: all {} transfers failed",
                result.results.len()
            );
            send("transfer-volumes", "failed", &detail);
            record_step(&mut steps, "transfer-volumes", "failed", &detail, dur);

            // Attempt rollback
            send("rollback", "started", "Rolling back: restarting source containers...");
            let rb_t0 = Instant::now();
            match restart_source_containers(source, &request.stack_name, source_stacks_dir, source_host_stacks_dir, source_is_local).await {
                Ok(_) => {
                    rollback_performed = true;
                    let rb_dur = rb_t0.elapsed().as_millis() as u64;
                    send("rollback", "success", "Rollback complete — source restarted");
                    record_step(&mut steps, "rollback", "success", "Source restarted after transfer failure", rb_dur);
                }
                Err(e) => {
                    let rb_dur = rb_t0.elapsed().as_millis() as u64;
                    let err = format!("Rollback failed: {}", e);
                    send("rollback", "failed", &err);
                    record_step(&mut steps, "rollback", "failed", &err, rb_dur);
                }
            }

            return SwitchoverResult {
                status: "rolled_back".to_string(),
                steps,
                rollback_performed,
            };
        }

        let detail = format!(
            "Transferred {} volumes ({} bytes, status={})",
            result.results.len(),
            result.total_bytes,
            result.status,
        );
        send("transfer-volumes", "success", &detail);
        record_step(&mut steps, "transfer-volumes", "success", &detail, dur);
    } else {
        send("transfer-volumes", "skipped", "No volumes to transfer");
        record_step(&mut steps, "transfer-volumes", "skipped", "No volumes configured", 0);
    }

    // ── Phase 3: Deploy on target ────────────────────────────
    send("deploy-target", "started", "Deploying compose on target...");
    let t0 = Instant::now();

    match deploy_compose_target(target, &request.compose_yaml, &request.stack_name, target_stacks_dir).await {
        Ok(output) => {
            let dur = t0.elapsed().as_millis() as u64;
            let detail = format!("Deployed: {}", output.lines().next().unwrap_or("ok"));
            send("deploy-target", "success", &detail);
            record_step(&mut steps, "deploy-target", "success", &detail, dur);
        }
        Err(e) => {
            let dur = t0.elapsed().as_millis() as u64;
            let detail = format!("Deploy failed: {}", e);
            send("deploy-target", "failed", &detail);
            record_step(&mut steps, "deploy-target", "failed", &detail, dur);

            // Attempt rollback
            send("rollback", "started", "Rolling back: restarting source containers...");
            let rb_t0 = Instant::now();
            match restart_source_containers(source, &request.stack_name, source_stacks_dir, source_host_stacks_dir, source_is_local).await {
                Ok(_) => {
                    rollback_performed = true;
                    let rb_dur = rb_t0.elapsed().as_millis() as u64;
                    send("rollback", "success", "Rollback complete — source restarted");
                    record_step(&mut steps, "rollback", "success", "Source restarted after deploy failure", rb_dur);
                }
                Err(e) => {
                    let rb_dur = rb_t0.elapsed().as_millis() as u64;
                    let err = format!("Rollback failed: {}", e);
                    send("rollback", "failed", &err);
                    record_step(&mut steps, "rollback", "failed", &err, rb_dur);
                }
            }

            return SwitchoverResult {
                status: "rolled_back".to_string(),
                steps,
                rollback_performed,
            };
        }
    }

    // ── Phase 4: Health check polling ────────────────────────
    if let Some(ref url) = request.health_check_url {
        send("health-check", "started", &format!("Polling health check: {}", url));
        let t0 = Instant::now();
        let timeout = Duration::from_secs(request.health_check_timeout_secs);

        match poll_health(url, timeout).await {
            Ok(()) => {
                let dur = t0.elapsed().as_millis() as u64;
                let detail = format!("Health check passed after {}ms", dur);
                send("health-check", "success", &detail);
                record_step(&mut steps, "health-check", "success", &detail, dur);
            }
            Err(e) => {
                let dur = t0.elapsed().as_millis() as u64;
                let detail = format!("Health check failed: {}", e);
                send("health-check", "failed", &detail);
                record_step(&mut steps, "health-check", "failed", &detail, dur);

                // Attempt rollback
                send("rollback", "started", "Rolling back: restarting source containers...");
                let rb_t0 = Instant::now();
                // Also stop the failed target containers
                let _ = stop_target_containers(target, &request.stack_name).await;
                match restart_source_containers(source, &request.stack_name, source_stacks_dir, source_host_stacks_dir, source_is_local).await {
                    Ok(_) => {
                        rollback_performed = true;
                        let rb_dur = rb_t0.elapsed().as_millis() as u64;
                        send("rollback", "success", "Rollback complete — source restarted");
                        record_step(&mut steps, "rollback", "success", "Source restarted after health check failure", rb_dur);
                    }
                    Err(e) => {
                        let rb_dur = rb_t0.elapsed().as_millis() as u64;
                        let err = format!("Rollback failed: {}", e);
                        send("rollback", "failed", &err);
                        record_step(&mut steps, "rollback", "failed", &err, rb_dur);
                    }
                }

                return SwitchoverResult {
                    status: "rolled_back".to_string(),
                    steps,
                    rollback_performed,
                };
            }
        }
    } else {
        send("health-check", "skipped", "No health check URL configured");
        record_step(&mut steps, "health-check", "skipped", "No health check URL", 0);
    }

    // ── Success ──────────────────────────────────────────────
    send("complete", "success", "Switchover completed successfully");
    record_step(&mut steps, "complete", "success", "All phases passed", 0);

    SwitchoverResult {
        status: "success".to_string(),
        steps,
        rollback_performed: false,
    }
}

// ── Phase impls ────────────────────────────────────────────────────

/// Stop all containers belonging to a compose stack on the source endpoint.
///
/// Finds containers by compose project label and stops + removes them.
async fn stop_source_containers(docker: &Docker, stack_name: &str) -> Result<usize, String> {
    let containers = list_stack_containers(docker, stack_name).await?;
    let count = containers.len();

    for container in &containers {
        if let Some(id) = &container.id {
            // Stop container
            docker
                .stop_container(id, None::<StopContainerOptions>)
                .await
                .map_err(|e| format!("Failed to stop container {}: {}", &id[..12.min(id.len())], e))?;

            // Remove container
            docker
                .remove_container(id, None::<RemoveContainerOptions>)
                .await
                .map_err(|e| format!("Failed to remove container {}: {}", &id[..12.min(id.len())], e))?;
        }
    }

    Ok(count)
}

/// Restart source containers by deploying the compose file on the source.
async fn restart_source_containers(
    docker: &Docker,
    stack_name: &str,
    stacks_dir: &str,
    host_stacks_dir: &str,
    is_local: bool,
) -> Result<(), String> {
    // Read the compose file from source stacks dir and redeploy
    let compose_yaml = crate::compose::read_compose_remote(
        docker,
        stacks_dir,
        if host_stacks_dir.is_empty() { None } else { Some(host_stacks_dir) },
        stack_name,
        is_local,
    ).await?;
    deploy_compose_target(docker, &compose_yaml, stack_name, stacks_dir).await?;
    Ok(())
}

/// Stop target containers (used during rollback).
async fn stop_target_containers(docker: &Docker, stack_name: &str) -> Result<(), String> {
    stop_source_containers(docker, stack_name).await?;
    Ok(())
}

/// List containers belonging to a compose stack by project label.
async fn list_stack_containers(docker: &Docker, stack_name: &str) -> Result<Vec<bollard::models::ContainerSummary>, String> {
    let mut filters = std::collections::HashMap::new();
    filters.insert(
        "label".to_string(),
        vec![format!("com.docker.compose.project={}", stack_name)],
    );

    let options = ListContainersOptions {
        all: true,
        filters,
        ..Default::default()
    };

    docker
        .list_containers(Some(options))
        .await
        .map_err(|e| format!("Failed to list containers for stack '{}': {}", stack_name, e))
}

/// Deploy a compose YAML on the target endpoint.
///
/// Writes the compose file to the target's stacks directory, then runs
/// `docker compose up -d` via a privileged container with Docker socket access.
async fn deploy_compose_target(
    docker: &Docker,
    compose_yaml: &str,
    stack_name: &str,
    stacks_dir: &str,
) -> Result<String, String> {
    use bollard::container::{
        Config, CreateContainerOptions, LogOutput, LogsOptions, RemoveContainerOptions,
        StartContainerOptions,
    };
    use bollard::exec::{CreateExecOptions, StartExecResults};

    let stack_dir = format!("{}/{}", stacks_dir.trim_end_matches('/'), stack_name);
    let compose_path = format!("{}/docker-compose.yml", stack_dir);

    // ── Step 1: Ensure stack directory exists and write compose file ──
    let setup_name = format!("mari-setup-{}", stack_name.chars().take(16).collect::<String>());

    // Create a one-shot alpine container with stacks dir mounted to write the file
    let setup_container = docker
        .create_container(
            Some(CreateContainerOptions {
                name: setup_name.clone(),
                platform: None,
            }),
            Config {
                image: Some("alpine:latest"),
                cmd: Some(vec![
                    "sh", "-c",
                    &format!("mkdir -p '{}' && cat > '{}'", stack_dir, compose_path),
                ]),
                host_config: Some(HostConfig {
                    binds: Some(vec![format!("{}:{}", stacks_dir, stacks_dir)]),
                    auto_remove: Some(true),
                    ..Default::default()
                }),
                attach_stdin: Some(true),
                open_stdin: Some(true),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| format!("Failed to create setup container: {}", e))?;

    // Start the setup container
    docker
        .start_container(&setup_container.id, None::<StartContainerOptions<String>>)
        .await
        .map_err(|e| format!("Failed to start setup container: {}", e))?;

    // Create exec to write compose content via stdin
    let write_exec = docker
        .create_exec(
            &setup_container.id,
            CreateExecOptions {
                attach_stdin: Some(true),
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                cmd: Some(vec!["sh", "-c", &format!("cat > '{}'", compose_path)]),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| format!("Failed to create write exec: {}", e))?;

    match docker.start_exec(&write_exec.id, None).await {
        Ok(StartExecResults::Attached { mut input, .. }) => {
            use tokio::io::AsyncWriteExt;
            input.write_all(compose_yaml.as_bytes()).await
                .map_err(|e| format!("Failed to write compose file: {}", e))?;
            let _ = input.shutdown().await;
        }
        Ok(StartExecResults::Detached) => {
            return Err("Write exec detached unexpectedly".to_string());
        }
        Err(e) => {
            let _ = docker.remove_container(&setup_container.id, None::<RemoveContainerOptions>).await;
            return Err(format!("Failed to start write exec: {}", e));
        }
    }

    // Wait briefly for the write to complete and cleanup
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _ = docker.remove_container(&setup_container.id, None::<RemoveContainerOptions>).await;

    // ── Step 2: Run docker compose up -d ──
    // Use a container with docker CLI and docker socket mounted
    let deploy_name = format!("mari-deploy-{}", stack_name.chars().take(14).collect::<String>());

    let deploy_container = docker
        .create_container(
            Some(CreateContainerOptions {
                name: deploy_name.clone(),
                platform: None,
            }),
            Config {
                image: Some("docker:cli"),
                cmd: Some(vec![
                    "sh", "-c",
                    &format!("cd '{}' && docker compose up -d", stack_dir),
                ]),
                host_config: Some(HostConfig {
                    binds: Some(vec![
                        format!("{}:{}", stacks_dir, stacks_dir),
                        "/var/run/docker.sock:/var/run/docker.sock".to_string(),
                    ]),
                    auto_remove: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| format!("Failed to create deploy container: {}", e))?;

    docker
        .start_container(&deploy_container.id, None::<StartContainerOptions<String>>)
        .await
        .map_err(|e| format!("Failed to start deploy container: {}", e))?;

    // Collect logs from the deploy container
    let mut logs = docker.logs(
        &deploy_container.id,
        Some(LogsOptions::<String> {
            follow: true,
            stdout: true,
            stderr: true,
            ..Default::default()
        }),
    );

    let mut output = String::new();
    let mut has_error = false;

    while let Some(chunk) = logs.next().await {
        match chunk {
            Ok(LogOutput::StdOut { message }) => {
                let text = String::from_utf8_lossy(&message);
                output.push_str(&text);
            }
            Ok(LogOutput::StdErr { message }) => {
                let text = String::from_utf8_lossy(&message);
                if !text.trim().is_empty() {
                    output.push_str(&format!("[stderr] {}", text));
                    has_error = true;
                }
            }
            Ok(_) => {}
            Err(e) => {
                return Err(format!("Deploy log stream error: {}", e));
            }
        }
    }

    if has_error && output.trim().is_empty() {
        return Err(format!("Deploy failed: {}", output.trim()));
    }

    Ok(output)
}

// ── Health Check Polling ───────────────────────────────────────────

/// Poll a health check URL every 2 seconds until it returns a 2xx status
/// or the timeout expires.
async fn poll_health(url: &str, timeout: Duration) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let start = Instant::now();
    let poll_interval = Duration::from_secs(2);

    loop {
        // Check timeout
        if start.elapsed() > timeout {
            return Err(format!(
                "Health check timed out after {}s polling {}",
                timeout.as_secs(),
                url
            ));
        }

        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                return Ok(());
            }
            Ok(resp) => {
                tracing::debug!(
                    "Health check {} returned {} — retrying...",
                    url,
                    resp.status()
                );
            }
            Err(e) => {
                tracing::debug!("Health check {} failed: {} — retrying...", url, e);
            }
        }

        tokio::time::sleep(poll_interval).await;
    }
}
