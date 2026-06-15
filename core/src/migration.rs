// ── Migration Orchestrator ─────────────────────────────────────────
// SECURITY-CRITICAL MODULE:
//   - Marionette NEVER holds or transmits SSH keys.
//   - Marionette generates shell commands; the admin runs them manually.
//   - All transfers use Option C (command generation).
//   - Credentials in env vars are ALWAYS masked.
//   - Every mutating action is audited.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::docker::*;
use crate::models::*;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn error(code: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({"error": msg})))
}

// ── In-memory migration plan store ────────────────────────────────

static MIGRATION_STORE: OnceLock<RwLock<HashMap<String, MigrationPlan>>> = OnceLock::new();

fn store() -> &'static RwLock<HashMap<String, MigrationPlan>> {
    MIGRATION_STORE.get_or_init(|| RwLock::new(HashMap::new()))
}

// ── Security helpers ──────────────────────────────────────────────

/// Mask credential-like env var values.
fn mask_env_var(key: &str, value: &str) -> String {
    let lower = key.to_lowercase();
    if lower.contains("password")
        || lower.contains("secret")
        || lower.contains("key")
        || lower.contains("token")
        || lower.contains("credential")
    {
        "••••••••".to_string()
    } else {
        value.to_string()
    }
}

/// Check if an env var looks like a DB connection string.
fn is_db_connection_var(key: &str) -> bool {
    let lower = key.to_lowercase();
    lower.contains("db_")
        || lower.contains("database_")
        || lower.contains("postgres")
        || lower.contains("mysql")
        || lower.contains("mongo")
        || lower.contains("redis")
        || lower.contains("sql")
        || lower.ends_with("_url")
        || lower.ends_with("_uri")
        || lower.ends_with("_dsn")
        || lower.ends_with("_host")
}

/// Detect if an env var target will break after migration.
fn will_connection_break(_key: &str, value: &str, on_same_host: bool) -> bool {
    let lower_val = value.to_lowercase();

    // If connecting to localhost/127.0.0.1 and moving to different host → will break
    if !on_same_host
        && (lower_val.contains("localhost")
            || lower_val.contains("127.0.0.1")
            || lower_val.contains("0.0.0.0"))
    {
        return true;
    }

    // If referencing the container's own hostname and moving
    if !on_same_host && (lower_val.contains("host.docker.internal") || lower_val.contains("docker.for.mac")) {
        return true;
    }

    false
}

/// Suggest a fix for a broken DB connection.
fn suggest_fix(key: &str, value: &str, on_same_host: bool) -> String {
    if on_same_host {
        return "Connection should work on same host".to_string();
    }

    let lower_val = value.to_lowercase();
    if lower_val.contains("localhost") || lower_val.contains("127.0.0.1") {
        format!(
            "Update {} to point to the target host's service address (e.g., new-db-host:5432)",
            key
        )
    } else {
        format!(
            "Verify {} is reachable from the target host. Consider updating to target-side address.",
            key
        )
    }
}

/// Check if a bind mount path is a kernel path that should be skipped.
fn is_kernel_path(path: &str) -> bool {
    path.starts_with("/proc")
        || path.starts_with("/sys")
        || path.starts_with("/var/run/docker")
        || path == "/var/run"
        || path == "/etc/hostname"
        || path == "/etc/hosts"
        || path == "/etc/resolv.conf"
}

// ── Request bodies ────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct AnalyzeRequest {
    pub source_endpoint: String,
    pub container_id: String,
}

#[derive(serde::Deserialize)]
pub struct PlanRequest {
    pub source_endpoint: String,
    pub target_endpoint: String,
    pub container_id: String,
    #[serde(default = "default_transfer_method")]
    pub transfer_method: String,
}

fn default_transfer_method() -> String {
    "pipe-direct".to_string()
}

// ── Shared: analyze container and build a MigrationPlan ───────────

/// Inspect a container on a source endpoint and produce an analyzed MigrationPlan.
/// Commands are only generated if `generate_commands` is true.
async fn build_migration_plan(
    state: &Arc<crate::AppState>,
    source_endpoint_id: &str,
    target_endpoint_id: Option<&str>,
    container_id: &str,
    transfer_method: &str,
    generate_commands: bool,
) -> Result<MigrationPlan, (StatusCode, Json<serde_json::Value>)> {
    // ── Step 0: Validate ──────────────────────────────────────
    let migration_id = Uuid::new_v4().to_string();

    // Get source client
    let clients = state.clients.read().await;
    let source_docker = get_client(source_endpoint_id, &clients)
        .await
        .map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, &e))?;
    drop(clients);

    // Inspect container
    let info = source_docker
        .inspect_container(container_id, None)
        .await
        .map_err(|e| error(StatusCode::NOT_FOUND, &format!("Container not found: {}", e)))?;

    let image = info
        .config
        .as_ref()
        .and_then(|c| c.image.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let container_name = info
        .name
        .as_deref()
        .unwrap_or("unknown")
        .trim_start_matches('/')
        .to_string();

    let env_vars: Vec<String> = info
        .config
        .as_ref()
        .and_then(|c| c.env.clone())
        .unwrap_or_default();

    // Determine if source and target are on the same host
    let same_host = if let Some(target_id) = target_endpoint_id {
        source_endpoint_id == target_id
    } else {
        true // analyze-only, assume same host for conservative checks
    };

    // ── Step 1: Analyze ───────────────────────────────────────

    let mut volumes: Vec<MigrationVolume> = Vec::new();
    let mut db_connections: Vec<DbConnection> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut estimated_size_bytes: u64 = 0;

    // Enumerate volumes from mounts
    if let Some(mounts) = &info.mounts {
        for mount in mounts {
            let source = mount.source.as_deref().unwrap_or("");
            let destination = mount.destination.as_deref().unwrap_or("");
            let mount_type = mount
                .typ
                .as_ref()
                .map(|t| format!("{:?}", t).to_lowercase())
                .unwrap_or_else(|| "bind".to_string());

            // Handle named volumes
            if mount_type == "volume" {
                if let Some(vol_name) = &mount.name {
                    let driver = mount
                        .driver
                        .clone()
                        .unwrap_or_else(|| "local".to_string());
                    let (category, _advice) = classify_driver(&driver);

                    // Fetch volume details for size
                    let vol_size = match source_docker.inspect_volume(vol_name).await {
                        Ok(vol_info) => {
                            let size = vol_info
                                .usage_data
                                .as_ref()
                                .map(|u| u.size)
                                .unwrap_or(0) as u64;
                            if !same_host && size >= 500_000_000_000 {
                                warnings.push(format!(
                                    "Volume '{}' is {} — pre-flight disk check on target required",
                                    vol_name,
                                    human_bytes(size)
                                ));
                            }
                            Some(size)
                        }
                        Err(_) => None,
                    };

                    if let Some(size) = vol_size {
                        estimated_size_bytes += size;
                    }

                    let vol_transfer_method = if same_host {
                        "local".to_string()
                    } else {
                        transfer_method.to_string()
                    };

                    volumes.push(MigrationVolume {
                        name: vol_name.clone(),
                        driver: driver.clone(),
                        driver_category: category.to_string(),
                        size_bytes: vol_size,
                        shared: false,
                        transfer_method: vol_transfer_method,
                    });
                }
            } else if mount_type == "bind" {
                // Warn about kernel path bind mounts
                if is_kernel_path(source) {
                    warnings.push(format!(
                        "Bind mount '{}:{}' is a kernel/system path — auto-skipping",
                        source, destination
                    ));
                    continue;
                }
                if !same_host {
                    warnings.push(format!(
                        "Bind mount '{}:{}' must exist on target host with same path",
                        source, destination
                    ));
                }
            }
        }
    }

    // Detect DB connections in env vars
    for env_line in &env_vars {
        if let Some((key, value)) = env_line.split_once('=') {
            if is_db_connection_var(key) {
                let breaks = will_connection_break(key, value, same_host);
                let fix = suggest_fix(key, value, same_host);

                if breaks {
                    warnings.push(format!(
                        "DB connection '{}' will likely break after migration",
                        key
                    ));
                }

                db_connections.push(DbConnection {
                    var_name: key.to_string(),
                    value_masked: mask_env_var(key, value),
                    target_container: None,
                    on_same_host: same_host,
                    will_break: breaks,
                    fix_suggestion: fix,
                });
            }
        }
    }

    // ── Step 2: Generate commands (if requested) ──────────────
    let mut commands: Vec<String> = Vec::new();

    if generate_commands {
        // Read source endpoint connection for SSH target inference
        let source_endpoints = state.endpoints.read().await;
        let source_ep = source_endpoints.get(source_endpoint_id);
        let _source_conn = source_ep
            .map(|e| e.connection.clone())
            .unwrap_or_else(|| "unix:///var/run/docker.sock".to_string());

        let target_conn = if let Some(target_id) = target_endpoint_id {
            source_endpoints
                .get(target_id)
                .map(|e| e.connection.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };
        drop(source_endpoints);

        let has_remote_target = !same_host && !target_conn.is_empty();

        // Build SSH host from target connection
        let ssh_host = if has_remote_target {
            // Extract host from tcp://host:port
            if let Some(rest) = target_conn.strip_prefix("tcp://") {
                rest.split(':').next().unwrap_or("unknown-host")
            } else {
                "unknown-host"
            }
        } else {
            ""
        };

        // Generate commands for each volume
        let mut vol_exports: Vec<String> = Vec::new();
        for vol in &volumes {
            if vol.driver_category == "filesystem" || vol.driver_category == "unknown" {
                let tar_name = format!("/tmp/marionette/{}_{}.tar.gz", migration_id, vol.name);
                commands.push(format!(
                    "# Export volume: {}",
                    vol.name
                ));
                commands.push(format!(
                    "docker run --rm -v {}:/data -v /tmp/marionette:/out alpine:latest \\",
                    vol.name
                ));
                commands.push(format!(
                    "  tar czf /out/{}_{}.tar.gz -C /data .",
                    migration_id, vol.name
                ));
                if has_remote_target {
                    commands.push(format!(
                        "# === ADMIN MUST EXECUTE THE FOLLOWING scp COMMAND MANUALLY ===",
                    ));
                    commands.push(format!(
                        "scp {} user@{}:/tmp/marionette/",
                        tar_name, ssh_host
                    ));
                }
                vol_exports.push(format!("{}_{}.tar.gz", migration_id, vol.name));
            } else {
                commands.push(format!(
                    "# Volume '{}' uses driver '{}' ({}) — may require reconnection on target",
                    vol.name, vol.driver, vol.driver_category
                ));
            }
        }

        if has_remote_target {
            commands.push(String::new());
            commands.push("# === PIPE-DIRECT TRANSFER (recommended) ===".to_string());
            commands.push("# On source host:".to_string());
            for vol in &volumes {
                if vol.driver_category == "filesystem" || vol.driver_category == "unknown" {
                    commands.push(format!(
                        "docker run --rm -v {}:/data alpine:latest tar czf - -C /data . | \\",
                        vol.name
                    ));
                    commands.push(format!(
                        "  ssh user@{} \"docker run --rm -i -v {}:/data alpine:latest tar xzf - -C /data\"",
                        ssh_host, vol.name
                    ));
                }
            }

            commands.push(String::new());
            commands.push("# === COMPOSE DEPLOY on Target ===".to_string());
            commands.push(format!(
                "# scp docker-compose.yml user@{}:~/",
                ssh_host
            ));
            commands.push(format!(
                "ssh user@{} \"cd ~ && docker compose up -d\"",
                ssh_host
            ));

            commands.push(String::new());
            commands.push("# === VERIFY ===".to_string());
            commands.push(format!(
                "# ssh user@{} \"docker ps --filter name={}\"",
                ssh_host, container_name
            ));

            commands.push(String::new());
            commands.push("# === CLEANUP ===".to_string());
            commands.push("# rm -rf /tmp/marionette/*.tar.gz".to_string());
            commands.push(format!(
                "# ssh user@{} \"rm -rf /tmp/marionette/*.tar.gz\"",
                ssh_host
            ));
        } else {
            // Same host — much simpler commands
            commands.push(String::new());
            commands.push("# === SAME-HOST MIGRATION ===".to_string());
            commands.push("# Stop source container".to_string());
            commands.push(format!("docker stop {}", container_name));
            commands.push(String::new());
            commands.push("# Deploy compose file".to_string());
            commands.push("docker compose up -d".to_string());
            commands.push(String::new());
            commands.push("# === VERIFY ===".to_string());
            commands.push(format!("# docker ps --filter name={}", container_name));
        }
    }

    // ── Warn about security issues ───────────────────────────
    for env_line in &env_vars {
        let lower = env_line.to_lowercase();
        if lower.contains("rsync") && !lower.contains("ssh") {
            warnings.push(
                "WARNING: rsync without SSH detected in env — unencrypted transfer. Recommend SSH.".to_string()
            );
            break;
        }
    }

    Ok(MigrationPlan {
        migration_id,
        source_endpoint: source_endpoint_id.to_string(),
        target_endpoint: target_endpoint_id.unwrap_or("unknown").to_string(),
        container_name,
        container_id: container_id.to_string(),
        image,
        volumes,
        db_connections,
        commands,
        warnings,
        estimated_size_bytes,
        compressed: true,
    })
}

// ── POST /migration/analyze ───────────────────────────────────────

pub async fn analyze_migration(
    State(state): State<Arc<crate::AppState>>,
    Json(body): Json<AnalyzeRequest>,
) -> ApiResult<MigrationPlan> {
    let plan = build_migration_plan(
        &state,
        &body.source_endpoint,
        None,
        &body.container_id,
        "scp",
        false, // no commands
    )
    .await?;

    // Store plan for later retrieval
    let plan_id = plan.migration_id.clone();
    store().write().await.insert(plan_id, plan.clone());

    // Audit
    state
        .audit_log
        .record(
            "migration.analyze",
            &body.source_endpoint,
            &body.container_id,
            &format!("analyzed container, {} volumes, {} warnings", plan.volumes.len(), plan.warnings.len()),
            "gateway",
        )
        .await;

    Ok(Json(plan))
}

// ── POST /migration/plan ──────────────────────────────────────────

pub async fn plan_migration(
    State(state): State<Arc<crate::AppState>>,
    Json(body): Json<PlanRequest>,
) -> ApiResult<MigrationPlan> {
    let plan = build_migration_plan(
        &state,
        &body.source_endpoint,
        Some(&body.target_endpoint),
        &body.container_id,
        &body.transfer_method,
        true, // generate commands
    )
    .await?;

    let plan_id = plan.migration_id.clone();

    // Store plan
    store().write().await.insert(plan_id.clone(), plan.clone());

    // Audit (real audit, not dry-run)
    state
        .audit_log
        .record(
            "migration.plan",
            &body.source_endpoint,
            &body.container_id,
            &format!(
                "plan → {}; {} volumes; {} cmds; {} warnings",
                body.target_endpoint,
                plan.volumes.len(),
                plan.commands.len(),
                plan.warnings.len()
            ),
            "gateway",
        )
        .await;

    Ok(Json(plan))
}

// ── POST /migration/dry-run ───────────────────────────────────────

pub async fn dry_run_migration(
    State(state): State<Arc<crate::AppState>>,
    Json(body): Json<PlanRequest>,
) -> ApiResult<serde_json::Value> {
    let plan = build_migration_plan(
        &state,
        &body.source_endpoint,
        Some(&body.target_endpoint),
        &body.container_id,
        &body.transfer_method,
        true,
    )
    .await?;

    let plan_id = plan.migration_id.clone();
    store().write().await.insert(plan_id.clone(), plan.clone());

    // Audit as dry-run (marked differently)
    state
        .audit_log
        .record(
            "migration.dry_run",
            &body.source_endpoint,
            &body.container_id,
            &format!(
                "dry-run → {}; {} commands generated for review",
                body.target_endpoint,
                plan.commands.len()
            ),
            "gateway",
        )
        .await;

    let plan = store().read().await.get(&plan_id).cloned().unwrap();
    Ok(Json(serde_json::json!({
        "dry_run": true,
        "plan": plan
    })))
}

// ── GET /migration/:id ────────────────────────────────────────────

pub async fn get_migration(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
) -> ApiResult<MigrationPlan> {
    let store = store().read().await;
    let plan = store
        .get(&id)
        .cloned()
        .ok_or_else(|| error(StatusCode::NOT_FOUND, &format!("Migration plan '{}' not found", id)))?;

    // Audit retrieval
    state
        .audit_log
        .record(
            "migration.get",
            &plan.source_endpoint,
            &id,
            "retrieved migration plan",
            "gateway",
        )
        .await;

    Ok(Json(plan))
}
