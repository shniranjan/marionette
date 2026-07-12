//! Switchover phase — stop source, deploy target, health checks.
//!
//! `execute_switchover()` stops the source stack via `compose.down`,
//! runs a final incremental sync for database volumes, deploys on the
//! target with `compose.up`, polls health checks with retry, and gates
//! on all services being healthy before returning success.

use relay_protocol::payloads::{ComposeDownResponse, ComposeUpRequest, ComposeUpResponse, DockerExecRequest};
use relay_protocol::Message;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::analyze::AnalyzeResult;

// ── Timestamp tracking for rollback window ──────────────────────────

/// Timestamp (epoch seconds) of the most recent successful switchover.
/// Used by rollback to verify the window is still open.
pub static SWITCHOVER_AT: LazyLock<Mutex<Option<u64>>> = LazyLock::new(|| Mutex::new(None));

// ── Helpers ──────────────────────────────────────────────────────────

fn relay_request(subtype: &str, payload: Value) -> Message {
    Message::new_request(Uuid::new_v4().to_string(), subtype, payload)
}

/// Extract host port from a port spec like "8080:8080" or "8080".
fn parse_host_port(port_spec: &str) -> String {
    port_spec
        .split(':')
        .last()
        .unwrap_or(port_spec)
        .to_string()
}

/// Run a health check exec inside a container and return the result.
async fn health_check_exec(
    host: &str,
    container: &str,
    cmd: &str,
) -> Result<HealthCheck, String> {
    let msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "docker.exec",
        serde_json::to_value(DockerExecRequest {
            container: container.to_string(),
            cmd: vec!["sh".to_string(), "-c".to_string(), cmd.to_string()],
            attach_stdout: true,
            attach_stderr: true,
            attach_stdin: false,
            workdir: None,
            env: None,
            user: None,
            timeout_secs: Some(30),
        })
        .unwrap_or_default(),
    );

    let resp =
        crate::ws_relay::send_relay_command(host, msg, 30).await?;
    let exec_resp: relay_protocol::payloads::DockerExecResponse =
        serde_json::from_value(resp.payload)
            .map_err(|e| format!("Parse exec response: {}", e))?;

    Ok(HealthCheck {
        check: cmd.to_string(),
        status: if exec_resp.exit_code == 0 {
            "healthy".to_string()
        } else {
            "unhealthy".to_string()
        },
        duration_ms: exec_resp.duration_ms,
    })
}

/// Choose the right health check command for a service based on its
/// image and ports.
fn health_check_cmd(svc: &super::analyze::ServicePlan) -> Option<String> {
    if !svc.ports.is_empty() {
        let port = parse_host_port(&svc.ports[0]);
        return Some(format!(
            "curl -sf http://localhost:{}/ || exit 1",
            port
        ));
    }
    let img = svc.image.to_lowercase();
    if img.contains("postgres") {
        Some("pg_isready -h localhost".to_string())
    } else if img.contains("mysql") || img.contains("mariadb") {
        Some("mysqladmin ping -h localhost".to_string())
    } else {
        None
    }
}

// ── Result types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerStopResult {
    pub name: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerStartResult {
    pub name: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
    pub check: String,
    pub status: String, // "healthy" | "unhealthy"
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalSync {
    pub volumes_synced: Vec<String>,
    pub bytes_transferred: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchoverResult {
    pub success: bool,
    pub source_stopped: Vec<ContainerStopResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_sync: Option<FinalSync>,
    pub target_started: Vec<ContainerStartResult>,
    pub health_checks: HashMap<String, HealthCheck>,
    pub rollback_available: bool,
    pub rollback_window_secs: u64,
}

// ── Main function ────────────────────────────────────────────────────

/// Stop source stack, deploy on target, poll health checks.
///
/// On failure, the caller should trigger rollback. The rollback window
/// is 1 hour from switchover completion.
pub async fn execute_switchover(
    source_host: &str,
    target_host: &str,
    plan: &AnalyzeResult,
) -> Result<SwitchoverResult, String> {
    // ── Derive project info from compose file path ─────────────────
    let path = Path::new(&plan.compose_file_path);
    let project_dir = path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let project_name = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "migration".to_string());

    tracing::info!(
        %source_host, %target_host, %project_dir, %project_name,
        "switchover: starting"
    );

    // ── 1. Stop source containers ──────────────────────────────────
    tracing::info!("switchover: stopping source stack");

    let down_msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "compose.down",
        serde_json::json!({
            "project_dir": &project_dir,
            "project_name": &project_name,
            "file": null,
            "volumes": false,
            "remove_orphans": true,
        }),
    );

    let down_resp =
        crate::ws_relay::send_relay_command(source_host, down_msg, 60)
            .await
            .map_err(|e| format!("compose.down on source failed: {}", e))?;

    let down: ComposeDownResponse =
        serde_json::from_value(down_resp.payload)
            .map_err(|e| format!("Parse compose.down response: {}", e))?;

    let source_stopped = vec![ContainerStopResult {
        name: format!("{}-all", project_name),
        duration_ms: down.duration_ms,
    }];

    tracing::info!(
        exit_code = down.exit_code,
        duration_ms = down.duration_ms,
        "switchover: source stopped"
    );

    // ── 2. Final sync for database volumes ──────────────────────────
    let db_volumes: Vec<_> = plan
        .volumes
        .iter()
        .filter(|v| v.database_detected)
        .collect();

    let final_sync = if !db_volumes.is_empty() {
        let mut synced = Vec::new();
        let sync_start = std::time::Instant::now();

        for vol in &db_volumes {
            tracing::info!(volume = %vol.name, "switchover: final sync");

            // Transfer out from source
            let out_msg = relay_request(
                "volume.transfer_out",
                serde_json::json!({
                    "volume": vol.name,
                    "target_relay": target_host,
                }),
            );
            let out_resp =
                crate::ws_relay::send_relay_command(source_host, out_msg, 60)
                    .await
                    .map_err(|e| {
                        format!(
                            "volume.transfer_out for {} failed: {}",
                            vol.name, e
                        )
                    })?;

            let transfer_id = out_resp
                .payload
                .get("transfer_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Transfer in to target
            let in_msg = relay_request(
                "volume.transfer_in",
                serde_json::json!({
                    "transfer_id": &transfer_id,
                    "volume": vol.name,
                }),
            );
            let in_resp =
                crate::ws_relay::send_relay_command(target_host, in_msg, 60)
                    .await
                    .map_err(|e| {
                        format!(
                            "volume.transfer_in for {} failed: {}",
                            vol.name, e
                        )
                    })?;

            let bytes = in_resp
                .payload
                .get("bytes_received")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            synced.push(vol.name.clone());
        }

        Some(FinalSync {
            volumes_synced: synced,
            bytes_transferred: 0, // aggregate would need per-volume tracking
            duration_ms: sync_start.elapsed().as_millis() as u64,
        })
    } else {
        None
    };

    // ── 3. Deploy on target ────────────────────────────────────────
    tracing::info!("switchover: deploying on target");

    // Build env overrides from compose_diff
    let env_overrides: HashMap<String, String> = plan
        .compose_diff
        .env_changes
        .iter()
        .map(|ch| (ch.key.clone(), ch.new.clone()))
        .collect();

    let up_msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "compose.up",
        serde_json::to_value(ComposeUpRequest {
            project_dir: project_dir.clone(),
            project_name: Some(project_name.clone()),
            file: None,
            detach: true,
            build: false,
            timeout_secs: Some(120),
            env: if env_overrides.is_empty() {
                None
            } else {
                Some(env_overrides)
            },
            profiles: None,
            services: None,
        })
        .unwrap_or_default(),
    );

    let up_resp =
        crate::ws_relay::send_relay_command(target_host, up_msg, 120)
            .await
            .map_err(|e| format!("compose.up on target failed: {}", e))?;

    let up: ComposeUpResponse =
        serde_json::from_value(up_resp.payload)
            .map_err(|e| format!("Parse compose.up response: {}", e))?;

    let target_started = vec![ContainerStartResult {
        name: project_name.clone(),
        duration_ms: up.duration_ms,
    }];

    tracing::info!(
        exit_code = up.exit_code,
        duration_ms = up.duration_ms,
        "switchover: target deployed"
    );

    // ── 4–5. Health checks with retry (6×5s = 30s) ─────────────────
    tracing::info!("switchover: polling health checks");

    let max_retries: u32 = 6;
    let retry_delay_secs: u64 = 5;
    let mut health_checks: HashMap<String, HealthCheck> = HashMap::new();

    for attempt in 0..max_retries {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(
                retry_delay_secs,
            ))
            .await;
        }

        let mut all_healthy = true;
        health_checks.clear();

        for svc in &plan.services {
            let container_name =
                format!("{}-{}-1", project_name, svc.name);

            if let Some(cmd) = health_check_cmd(svc) {
                match health_check_exec(target_host, &container_name, &cmd)
                    .await
                {
                    Ok(hc) => {
                        let is_healthy = hc.status == "healthy";
                        if !is_healthy {
                            all_healthy = false;
                        }
                        health_checks.insert(svc.name.clone(), hc);
                    }
                    Err(e) => {
                        all_healthy = false;
                        health_checks.insert(
                            svc.name.clone(),
                            HealthCheck {
                                check: cmd.clone(),
                                status: "unhealthy".to_string(),
                                duration_ms: 0,
                            },
                        );
                        tracing::warn!(
                            service = %svc.name,
                            error = %e,
                            "switchover: health check failed"
                        );
                    }
                }
            }
        }

        if all_healthy {
            tracing::info!(
                attempt = attempt + 1,
                "switchover: all health checks passed"
            );
            break;
        }

        tracing::info!(
            attempt = attempt + 1,
            "switchover: health check retry"
        );
    }

    // ── Gate: all services must be healthy ─────────────────────────
    let any_unhealthy = health_checks
        .values()
        .any(|hc| hc.status != "healthy");

    if any_unhealthy {
        let unhealthy: Vec<_> = health_checks
            .iter()
            .filter(|(_, hc)| hc.status != "healthy")
            .map(|(name, _)| name.clone())
            .collect();

        let msg = format!(
            "Health check failed for services: {:?}",
            unhealthy
        );
        tracing::error!("switchover: {}", msg);
        return Err(msg);
    }

    // ── Record switchover timestamp for rollback window ────────────
    {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        *SWITCHOVER_AT.lock().await = Some(now);
    }

    tracing::info!("switchover: complete — source stopped, target healthy");

    Ok(SwitchoverResult {
        success: true,
        source_stopped,
        final_sync,
        target_started,
        health_checks,
        rollback_available: true,
        rollback_window_secs: 3600,
    })
}
