//! Rollback phase — reverse switchover after failure or user request.
//!
//! `rollback_migration()` stops the target stack via `compose.down`,
//! restarts the source stack with `compose.up`, and polls source health
//! checks. It refuses to run if the rollback window (1 hour) has expired.

use relay_protocol::payloads::{
    ComposeDownResponse, ComposeUpRequest, ComposeUpResponse, DockerExecRequest,
};
use relay_protocol::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;

use super::analyze::AnalyzeResult;
use super::switchover::{self, HealthCheck, ContainerStartResult, ContainerStopResult};

// ── Helpers ──────────────────────────────────────────────────────────

/// Extract host port from a port spec like "8080:8080" or "8080".
fn parse_host_port(port_spec: &str) -> String {
    port_spec
        .split(':')
        .last()
        .unwrap_or(port_spec)
        .to_string()
}

/// Choose the right health check command for a service based on its
/// image and ports. Duplicated from switchover for module isolation.
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

    let resp = crate::ws_relay::send_relay_command(host, msg, 30).await?;
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

// ── Result types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackResult {
    pub success: bool,
    pub target_stopped: Vec<ContainerStopResult>,
    pub source_restarted: Vec<ContainerStartResult>,
    pub health_checks: HashMap<String, HealthCheck>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

// ── Main function ────────────────────────────────────────────────────

/// Reverse switchover: stop target, restart source, verify source health.
///
/// Refuses to run if more than `rollback_window_secs` has elapsed since
/// the switchover completed (to prevent data loss from conflicting writes).
pub async fn rollback_migration(
    source_host: &str,
    target_host: &str,
    plan: &AnalyzeResult,
) -> Result<RollbackResult, String> {
    let mut warnings: Vec<String> = Vec::new();

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

    // ── 1. Verify rollback window ──────────────────────────────────
    let rollback_window_secs: u64 = 3600; // 1 hour
    let switchover_at = switchover::SWITCHOVER_AT.lock().await;

    match *switchover_at {
        None => {
            return Err(
                "No switchover recorded — nothing to roll back".to_string()
            );
        }
        Some(at) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let elapsed = now.saturating_sub(at);

            if elapsed > rollback_window_secs {
                return Err(format!(
                    "Rollback window expired: {}s elapsed of {}s allowed — data loss risk prevents automatic rollback",
                    elapsed, rollback_window_secs
                ));
            }

            tracing::info!(
                elapsed_secs = elapsed,
                window_secs = rollback_window_secs,
                "rollback: window verified"
            );
        }
    }

    tracing::info!(
        %source_host, %target_host, %project_dir, %project_name,
        "rollback: starting"
    );

    // ── 2. Stop target containers ──────────────────────────────────
    tracing::info!("rollback: stopping target stack");

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
        crate::ws_relay::send_relay_command(target_host, down_msg, 60)
            .await
            .map_err(|e| {
                format!("compose.down on target failed: {}", e)
            })?;

    let down: ComposeDownResponse =
        serde_json::from_value(down_resp.payload)
            .map_err(|e| format!("Parse compose.down response: {}", e))?;

    let target_stopped = vec![ContainerStopResult {
        name: format!("{}-all", project_name),
        duration_ms: down.duration_ms,
    }];

    tracing::info!(
        exit_code = down.exit_code,
        duration_ms = down.duration_ms,
        "rollback: target stopped"
    );

    // ── 3. Restart source containers ────────────────────────────────
    tracing::info!("rollback: restarting source stack");

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
            env: None,
            profiles: None,
            services: None,
        })
        .unwrap_or_default(),
    );

    let up_resp =
        crate::ws_relay::send_relay_command(source_host, up_msg, 120)
            .await
            .map_err(|e| {
                format!("compose.up on source failed: {}", e)
            })?;

    let up: ComposeUpResponse =
        serde_json::from_value(up_resp.payload)
            .map_err(|e| format!("Parse compose.up response: {}", e))?;

    let source_restarted = vec![ContainerStartResult {
        name: project_name.clone(),
        duration_ms: up.duration_ms,
    }];

    tracing::info!(
        exit_code = up.exit_code,
        duration_ms = up.duration_ms,
        "rollback: source restarted"
    );

    // ── 4. Verify source health (same logic as switchover) ─────────
    tracing::info!("rollback: polling source health checks");

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
                match health_check_exec(source_host, &container_name, &cmd)
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
                            "rollback: health check failed"
                        );
                    }
                }
            }
        }

        if all_healthy {
            tracing::info!(
                attempt = attempt + 1,
                "rollback: all health checks passed"
            );
            break;
        }

        tracing::info!(
            attempt = attempt + 1,
            "rollback: health check retry"
        );
    }

    // ── Warn if any services are still unhealthy ────────────────────
    let unhealthy: Vec<_> = health_checks
        .iter()
        .filter(|(_, hc)| hc.status != "healthy")
        .map(|(name, _)| name.clone())
        .collect();

    if !unhealthy.is_empty() {
        let msg = format!(
            "Rollback completed but some services are still unhealthy: {:?}",
            unhealthy
        );
        warnings.push(msg.clone());
        tracing::warn!("rollback: {}", msg);
    }

    tracing::info!("rollback: complete");

    Ok(RollbackResult {
        success: true,
        target_stopped,
        source_restarted,
        health_checks,
        warnings,
    })
}
