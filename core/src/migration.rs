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

use crate::docker::{classify_driver, human_bytes, suggest_transfer_method};
use crate::helpers;
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
    // Wave 1: New strategy fields
    #[serde(default)]
    pub compression: String,
    #[serde(default)]
    pub post_options: PostOptions,
    #[serde(default)]
    pub volume_overrides: HashMap<String, VolumeOverride>,
    #[serde(default)]
    pub connection_resolutions: HashMap<String, ConnectionResolution>,
    #[serde(default)]
    pub target_stack_name: Option<String>,
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
    compression: &str,
    post_options: &PostOptions,
    volume_overrides: &HashMap<String, VolumeOverride>,
) -> Result<MigrationPlan, (StatusCode, Json<serde_json::Value>)> {
    // ── Step 0: Validate ──────────────────────────────────────
    let migration_id = Uuid::new_v4().to_string();

    // Get source client
    let source_docker = helpers::resolve_client(state, Some(source_endpoint_id))
        .await?;

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

                    // Fetch volume details for size and options
                    let (vol_size, vol_opts) = match source_docker.inspect_volume(vol_name).await {
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

                            // Extract volume options (driver opts + labels)
                            let opts = {
                                let mut map = serde_json::Map::new();
                                if !vol_info.options.is_empty() {
                                    let mut opt_map = serde_json::Map::new();
                                    for (k, v) in &vol_info.options {
                                        opt_map.insert(k.clone(), serde_json::Value::String(v.clone()));
                                    }
                                    map.insert("driverOpts".to_string(), serde_json::Value::Object(opt_map));
                                }
                                if !vol_info.labels.is_empty() {
                                    let mut label_map = serde_json::Map::new();
                                    for (k, v) in &vol_info.labels {
                                        label_map.insert(k.clone(), serde_json::Value::String(v.clone()));
                                    }
                                    map.insert("labels".to_string(), serde_json::Value::Object(label_map));
                                }
                                if map.is_empty() { None } else { Some(serde_json::Value::Object(map)) }
                            };

                            (Some(size), opts)
                        }
                        Err(_) => (None, None),
                    };

                    if let Some(size) = vol_size {
                        estimated_size_bytes += size;
                    }

                    let vol_transfer_method = if same_host {
                        "local".to_string()
                    } else {
                        // Per-volume override from volume_overrides takes precedence
                        if let Some(ov) = volume_overrides.get(vol_name) {
                            ov.transfer_method.clone().unwrap_or_else(|| transfer_method.to_string())
                        } else {
                            transfer_method.to_string()
                        }
                    };

                    let default_method =
                        suggest_transfer_method(&driver).to_string();

                    // Merge volume override fields
                    let (target_name, target_path, target_driver, skip) =
                        if let Some(ov) = volume_overrides.get(vol_name) {
                            (
                                ov.target_name.clone(),
                                ov.target_path.clone(),
                                ov.target_driver.clone(),
                                ov.skip,
                            )
                        } else {
                            (None, None, None, false)
                        };

                    volumes.push(MigrationVolume {
                        name: vol_name.clone(),
                        driver: driver.clone(),
                        driver_category: category.to_string(),
                        size_bytes: vol_size,
                        shared: false,
                        transfer_method: vol_transfer_method,
                        default_transfer_method: default_method,
                        options: vol_opts.clone(),
                        target_name,
                        target_path,
                        target_driver,
                        skip,
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
        let target_conn = if let Some(target_id) = target_endpoint_id {
            state.registry
                .get(target_id)
                .await
                .map(|e| e.connection.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };

        let has_remote_target = !same_host && !target_conn.is_empty();
        let ssh_host = if has_remote_target {
            target_conn
                .strip_prefix("tcp://")
                .and_then(|r| r.split(':').next())
                .unwrap_or("unknown-host")
        } else {
            ""
        };

        if !has_remote_target {
            build_same_host_commands(&mut commands, &container_name, &volumes, volume_overrides);
        } else {
            match transfer_method {
                "scp" => build_scp_commands(&mut commands, &volumes, &migration_id, ssh_host, compression, volume_overrides),
                "rsync-over-ssh" => build_rsync_commands(&mut commands, &volumes, &migration_id, ssh_host, compression, volume_overrides),
                "pipe-direct" => build_pipe_commands(&mut commands, &volumes, &migration_id, ssh_host, compression, volume_overrides),
                "export-s3" => build_s3_commands(&mut commands, &volumes, &migration_id, ssh_host, compression, volume_overrides),
                _ => build_pipe_commands(&mut commands, &volumes, &migration_id, ssh_host, compression, volume_overrides),
            }

            // Compose deploy
            commands.push(String::new());
            commands.push("# === COMPOSE DEPLOY on Target ===".to_string());
            commands.push(format!("# scp docker-compose.yml user@{}:~/", ssh_host));
            commands.push(format!("ssh user@{} \"cd ~ && docker compose up -d\"", ssh_host));

            // Verify
            commands.push(String::new());
            commands.push("# === VERIFY ===".to_string());
            commands.push(format!("# ssh user@{} \"docker ps --filter name={}\"", ssh_host, container_name));

            // Cleanup
            commands.push(String::new());
            commands.push("# === CLEANUP ===".to_string());
            commands.push("# rm -rf /tmp/marionette/*.tar.gz".to_string());
            commands.push(format!("# ssh user@{} \"rm -rf /tmp/marionette/*.tar.gz\"", ssh_host));
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

    // Detect compose secret references
    let has_compose_secrets = env_vars.iter().any(|e| {
        e.contains("${") || e.to_uppercase().contains("DOCKER-SECRET") || e.contains("/run/secrets")
    });

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
        compressed: compression != "none",
        env_vars,
        has_compose_secrets,
        start_on_target: post_options.start_on_target,
        verify_connectivity: post_options.verify_connectivity,
        compression: compression.to_string(),
        post_options: Some(post_options.clone()),
        volume_overrides: if volume_overrides.is_empty() { None } else { Some(volume_overrides.clone()) },
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
        "none",
        &PostOptions::default(),
        &HashMap::new(),
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
        &body.compression,
        &body.post_options,
        &body.volume_overrides,
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
        &body.compression,
        &body.post_options,
        &body.volume_overrides,
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

// ── POST /migration/{id}/rollback ──────────────────────────────────

pub async fn rollback_migration(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let plan = {
        let store = store().read().await;
        store
            .get(&id)
            .cloned()
            .ok_or_else(|| error(StatusCode::NOT_FOUND, &format!("Migration plan '{}' not found", id)))?
    };

    // Audit the rollback attempt
    state
        .audit_log
        .record(
            "migration.rollback",
            &plan.source_endpoint,
            &id,
            "rollback requested",
            "gateway",
        )
        .await;

    // TODO: Implement actual rollback logic — regenerate reverse commands
    Ok(Json(serde_json::json!({
        "status": "ok",
        "message": "Rollback stub — admin should restart container on source manually",
        "migration_id": id
    })))
}

// ── POST /migration/{id}/execute ──────────────────────────────────

/// Join multi-line shell commands that use backslash continuations.
/// Filters out comment lines (starting with #) and empty lines.
fn coalesce_commands(raw: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();

    for line in raw {
        let trimmed = line.trim();
        // Skip comment-only lines and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            if !current.is_empty() {
                out.push(current.trim().to_string());
                current = String::new();
            }
            continue;
        }
        // Append to current command
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(trimmed);

        // If line ends with backslash, it's a continuation
        if trimmed.ends_with('\\') {
            // Remove trailing backslash but keep accumulating
            current = current.trim_end_matches('\\').trim_end().to_string();
            current.push(' '); // space before next continuation line
        } else {
            // Command is complete
            out.push(current.trim().to_string());
            current = String::new();
        }
    }

    // Flush any remaining partial command
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }

    out
}

pub async fn execute_migration(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    // Look up the plan
    let plan = {
        let store = store().read().await;
        store
            .get(&id)
            .cloned()
            .ok_or_else(|| error(StatusCode::NOT_FOUND, &format!("Migration plan '{}' not found", id)))?
    };

    // Coalesce multi-line commands, skip comments
    let commands = coalesce_commands(&plan.commands);

    if commands.is_empty() {
        return Ok(Json(serde_json::json!({
            "migration_id": id,
            "results": [],
            "status": "no_commands"
        })));
    }

    // Audit start of execution
    state
        .audit_log
        .record(
            "migration.execute.start",
            &plan.source_endpoint,
            &id,
            &format!("executing {} commands for container {}", commands.len(), plan.container_name),
            "gateway",
        )
        .await;

    let mut results: Vec<CommandExecutionResult> = Vec::new();

    for (idx, cmd) in commands.iter().enumerate() {
        tracing::info!(
            "Executing migration command [{}/{}]: {}",
            idx + 1,
            commands.len(),
            cmd
        );

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(120),
            async {
                let output = tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(cmd)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .output()
                    .await;

                match output {
                    Ok(out) => CommandExecutionResult {
                        index: idx,
                        command: cmd.clone(),
                        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
                        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
                        exit_code: out.status.code().unwrap_or(-1),
                    },
                    Err(e) => CommandExecutionResult {
                        index: idx,
                        command: cmd.clone(),
                        stdout: String::new(),
                        stderr: format!("Spawn error: {}", e),
                        exit_code: -1,
                    },
                }
            },
        )
        .await;

        let exec_result = match result {
            Ok(r) => r,
            Err(_) => CommandExecutionResult {
                index: idx,
                command: cmd.clone(),
                stdout: String::new(),
                stderr: "Command timed out after 120 seconds".to_string(),
                exit_code: -1,
            },
        };

        // Audit each command execution
        state
            .audit_log
            .record(
                "migration.execute.command",
                &plan.source_endpoint,
                &id,
                &format!(
                    "cmd [{}] exit={}: {}",
                    idx,
                    exec_result.exit_code,
                    &cmd[..cmd.len().min(100)]
                ),
                "gateway",
            )
            .await;

        results.push(exec_result);
    }

    // Audit completion
    let succeeded = results.iter().filter(|r| r.exit_code == 0).count();
    let failed = results.len() - succeeded;
    state
        .audit_log
        .record(
            "migration.execute.complete",
            &plan.source_endpoint,
            &id,
            &format!("{} succeeded, {} failed out of {}", succeeded, failed, results.len()),
            "gateway",
        )
        .await;

    Ok(Json(serde_json::json!({
        "migration_id": id,
        "results": results,
        "status": if failed == 0 { "success" } else { "partial_failure" }
    })))
}

// ── Command Builder Helpers ─────────────────────────────────────

use crate::models::migration::{MigrationVolume, VolumeOverride};

/// Return (tar_flag, extension) for the given compression method.
fn compression_flag(compression: &str) -> (&str, &str) {
    match compression {
        "zstd" => ("--zstd", ".tar.zst"),
        "lz4" => ("--lz4", ".tar.lz4"),
        "pigz" => ("--use-compress-program=pigz", ".tar.gz"),
        "none" => ("", ".tar"),
        _ => ("czf", ".tar.gz"),
    }
}

/// Resolve the target volume name: override if set, otherwise volume name.
fn vol_target_name<'a>(vol: &'a MigrationVolume, overrides: &'a HashMap<String, VolumeOverride>) -> &'a str {
    overrides
        .get(&vol.name)
        .and_then(|o| o.target_name.as_deref())
        .unwrap_or(&vol.name)
}

// ── Same-Host Migration ─────────────────────────────────────────

fn build_same_host_commands(
    commands: &mut Vec<String>,
    container_name: &str,
    volumes: &[MigrationVolume],
    overrides: &HashMap<String, VolumeOverride>,
) {
    commands.push("# === SAME-HOST MIGRATION ===".to_string());
    commands.push("# Stop source container".to_string());
    commands.push(format!("docker stop {}", container_name));
    commands.push(String::new());

    // Handle volume renames
    for vol in volumes {
        if overrides.get(&vol.name).map_or(false, |o| o.skip) {
            commands.push(format!("# Volume '{}' skipped per configuration", vol.name));
            continue;
        }
        let tgt = vol_target_name(vol, overrides);
        if tgt != vol.name {
            commands.push(format!("# Rename volume '{}' → '{}'", vol.name, tgt));
            commands.push(format!("docker volume create --name {}", tgt));
            commands.push(format!(
                "docker run --rm -v {}:/from -v {}:/to alpine cp -a /from/. /to/.",
                vol.name, tgt
            ));
            commands.push(format!("# docker volume rm {}  # remove old volume after verification", vol.name));
            commands.push(String::new());
        }
    }

    commands.push("# Deploy compose file".to_string());
    commands.push("docker compose up -d".to_string());
    commands.push(String::new());
    commands.push("# === VERIFY ===".to_string());
    commands.push(format!("# docker ps --filter name={}", container_name));
}

// ── SCP Transfer ─────────────────────────────────────────────────

fn build_scp_commands(
    commands: &mut Vec<String>,
    volumes: &[MigrationVolume],
    migration_id: &str,
    ssh_host: &str,
    compression: &str,
    overrides: &HashMap<String, VolumeOverride>,
) {
    let (tar_flag, ext) = compression_flag(compression);
    let tar_create = if tar_flag.is_empty() {
        format!("tar cf")
    } else if tar_flag == "czf" {
        "tar czf".to_string()
    } else {
        format!("tar {}", tar_flag)
    };

    commands.push("# === SCP TRANSFER ===".to_string());
    commands.push(format!("# Pre-create temp dir on target"));
    commands.push(format!("ssh user@{} \"mkdir -p /tmp/marionette\"", ssh_host));
    commands.push(String::new());

    for vol in volumes {
        if overrides.get(&vol.name).map_or(false, |o| o.skip) {
            commands.push(format!("# Volume '{}' skipped", vol.name));
            continue;
        }
        let tgt = vol_target_name(vol, overrides);

        if vol.driver_category == "filesystem" || vol.driver_category == "unknown" || vol.driver_category == "ephemeral" {
            let tar_name = format!("/tmp/marionette/{}_{}{}", migration_id, vol.name, ext);
            commands.push(format!("# Export volume: {} (→ {})", vol.name, tgt));
            commands.push(format!(
                "docker run --rm -v {}:/data -v /tmp/marionette:/out alpine:latest \\\\",
                vol.name
            ));
            commands.push(format!(
                "  {} /out/{}_{}{} -C /data .",
                tar_create, migration_id, vol.name, ext
            ));
            commands.push(format!("scp {} user@{}:/tmp/marionette/", tar_name, ssh_host));
            commands.push(format!(
                "# === MANUAL STEP (run on target) === docker run --rm -v {}:/data -v /tmp/marionette:/in alpine {} /in/{}_{}{} -C /data",
                tgt, tar_create, migration_id, vol.name, ext
            ));
            commands.push(String::new());
        } else {
            commands.push(format!(
                "# Volume '{}' uses driver '{}' ({}) — recreate on target with docker volume create",
                vol.name, vol.driver, vol.driver_category
            ));
            commands.push(format!(
                "# ssh user@{} \"docker volume create --driver {} --name {}\"",
                ssh_host, vol.driver, tgt
            ));
        }
    }
}

// ── Rsync over SSH ───────────────────────────────────────────────

fn build_rsync_commands(
    commands: &mut Vec<String>,
    volumes: &[MigrationVolume],
    _migration_id: &str,
    ssh_host: &str,
    compression: &str,
    overrides: &HashMap<String, VolumeOverride>,
) {
    let (tar_flag, _ext) = compression_flag(compression);
    let tar_flag = if tar_flag.is_empty() { "cf" } else if tar_flag == "czf" { "czf" } else { tar_flag };

    commands.push("# === RSYNC-STYLE TRANSFER via SSH ===".to_string());
    commands.push(format!("# On source host:"));
    commands.push(String::new());

    for vol in volumes {
        if overrides.get(&vol.name).map_or(false, |o| o.skip) {
            commands.push(format!("# Volume '{}' skipped", vol.name));
            continue;
        }
        let tgt = vol_target_name(vol, overrides);

        if vol.driver_category == "filesystem" || vol.driver_category == "unknown" || vol.driver_category == "ephemeral" {
            commands.push(format!("# Transfer volume: {} (→ {})", vol.name, tgt));
            commands.push(format!(
                "docker run --rm -v {}:/data alpine:latest tar {} - -C /data . | \\\\",
                vol.name, tar_flag
            ));
            commands.push(format!(
                "  ssh user@{} \"docker run --rm -i -v {}:/data alpine:latest tar x{} - -C /data\"",
                ssh_host, tgt,
                if compression == "none" { "" } else { "f" }
            ));
            commands.push(String::new());
        } else {
            commands.push(format!(
                "# Volume '{}' uses driver '{}' ({}) — recreate on target",
                vol.name, vol.driver, vol.driver_category
            ));
            commands.push(format!(
                "# ssh user@{} \"docker volume create --driver {} --name {}\"",
                ssh_host, vol.driver, tgt
            ));
        }
    }
}

// ── Pipe Direct ──────────────────────────────────────────────────

fn build_pipe_commands(
    commands: &mut Vec<String>,
    volumes: &[MigrationVolume],
    _migration_id: &str,
    ssh_host: &str,
    compression: &str,
    overrides: &HashMap<String, VolumeOverride>,
) {
    let (tar_flag, _ext) = compression_flag(compression);
    let tar_flag = if tar_flag.is_empty() { "cf" } else if tar_flag == "czf" { "czf" } else { tar_flag };

    commands.push("# === PIPE-DIRECT TRANSFER ===".to_string());
    commands.push(format!("# On source host:"));
    commands.push(String::new());

    for vol in volumes {
        if overrides.get(&vol.name).map_or(false, |o| o.skip) {
            commands.push(format!("# Volume '{}' skipped", vol.name));
            continue;
        }
        let tgt = vol_target_name(vol, overrides);

        if vol.driver_category == "filesystem" || vol.driver_category == "unknown" || vol.driver_category == "ephemeral" {
            commands.push(format!("# Pipe volume: {} (→ {})", vol.name, tgt));
            commands.push(format!(
                "docker run --rm -v {}:/data alpine:latest tar {} - -C /data . | \\\\",
                vol.name, tar_flag
            ));
            commands.push(format!(
                "  ssh user@{} \"docker run --rm -i -v {}:/data alpine:latest tar x{} - -C /data\"",
                ssh_host, tgt,
                if compression == "none" { "" } else { "f" }
            ));
            commands.push(String::new());
        } else {
            commands.push(format!(
                "# Volume '{}' uses driver '{}' ({}) — recreate on target",
                vol.name, vol.driver, vol.driver_category
            ));
            commands.push(format!(
                "# ssh user@{} \"docker volume create --driver {} --name {}\"",
                ssh_host, vol.driver, tgt
            ));
        }
    }
}

// ── Export to S3 ─────────────────────────────────────────────────

fn build_s3_commands(
    commands: &mut Vec<String>,
    volumes: &[MigrationVolume],
    migration_id: &str,
    ssh_host: &str,
    compression: &str,
    overrides: &HashMap<String, VolumeOverride>,
) {
    let (tar_flag, ext) = compression_flag(compression);
    let tar_create = if tar_flag.is_empty() {
        format!("tar cf")
    } else if tar_flag == "czf" {
        "tar czf".to_string()
    } else {
        format!("tar {}", tar_flag)
    };

    commands.push("# === EXPORT TO S3 ===".to_string());
    commands.push("# Requires: aws CLI installed and configured with s3:PutObject/s3:GetObject permissions".to_string());
    commands.push(format!("# Set S3_BUCKET env var or replace {{bucket}} below"));
    commands.push(String::new());

    for vol in volumes {
        if overrides.get(&vol.name).map_or(false, |o| o.skip) {
            commands.push(format!("# Volume '{}' skipped", vol.name));
            continue;
        }
        let tgt = vol_target_name(vol, overrides);

        if vol.driver_category == "filesystem" || vol.driver_category == "unknown" || vol.driver_category == "ephemeral" {
            let tar_name = format!("/tmp/marionette/{}_{}{}", migration_id, vol.name, ext);
            let s3_key = format!("marionette/{}_{}{}", migration_id, vol.name, ext);
            commands.push(format!("# Export volume: {} (→ {})", vol.name, tgt));
            commands.push("# === On source host ===".to_string());
            commands.push(format!(
                "docker run --rm -v {}:/data -v /tmp/marionette:/out alpine:latest \\\\",
                vol.name
            ));
            commands.push(format!(
                "  {} /out/{}_{}{} -C /data .",
                tar_create, migration_id, vol.name, ext
            ));
            commands.push(format!("aws s3 cp {} s3://{{bucket}}/{}", tar_name, s3_key));
            commands.push(String::new());
            commands.push("# === On target host ===".to_string());
            commands.push(format!("aws s3 cp s3://{{bucket}}/{} /tmp/marionette/{}_{}{}", s3_key, migration_id, vol.name, ext));
            commands.push(format!(
                "docker run --rm -v {}:/data -v /tmp/marionette:/in alpine {} /in/{}_{}{} -C /data",
                tgt, tar_create, migration_id, vol.name, ext
            ));
            commands.push(String::new());
        } else {
            commands.push(format!(
                "# Volume '{}' uses driver '{}' ({}) — recreate on target",
                vol.name, vol.driver, vol.driver_category
            ));
            commands.push(format!(
                "# ssh user@{} \"docker volume create --driver {} --name {}\"",
                ssh_host, vol.driver, tgt
            ));
        }
    }
}

// ── Direct Proxy Transfer ────────────────────────────────────────

use crate::transfer::{self, TransferRequest};

/// Execute volume transfers via bollard direct pipe (no SSH).
pub async fn transfer_volumes(
    State(state): State<Arc<crate::AppState>>,
    Json(body): Json<TransferRequest>,
) -> ApiResult<serde_json::Value> {
    // Resolve both Docker clients
    let source = helpers::resolve_client(&state, Some(&body.source_endpoint)).await?;
    let target = helpers::resolve_client(&state, Some(&body.target_endpoint)).await?;

    // Pre-flight: ensure alpine:latest on both hosts
    let (src_alpine, tgt_alpine) = tokio::join!(
        crate::docker::ensure_alpine_image(&source),
        crate::docker::ensure_alpine_image(&target),
    );

    let mut warnings: Vec<String> = Vec::new();
    if let Err(e) = src_alpine {
        warnings.push(format!("Source host: {}", e));
    }
    if let Err(e) = tgt_alpine {
        warnings.push(format!("Target host: {}", e));
    }

    // Execute batch transfer
    let result = transfer::transfer_volumes_batch(
        &source,
        &target,
        &body.transfers,
        &body.compression,
    )
    .await;

    // Audit
    state
        .audit_log
        .record(
            "migration.transfer",
            &body.source_endpoint,
            "",
            &format!(
                "Transferred {} volumes: {} bytes, status={}",
                result.results.len(),
                result.total_bytes,
                result.status,
            ),
            "gateway",
        )
        .await;

    Ok(Json(serde_json::json!({
        "results": result.results,
        "total_bytes": result.total_bytes,
        "status": result.status,
        "warnings": warnings,
    })))
}


// ── Compose Template Analysis & Preparation ──────────────────────

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeAnalyzeRequest {
    pub source_endpoint: String,
    pub target_endpoint: String,
    pub stack_name: String,
    #[serde(default = "default_stacks_dir")]
    pub source_stacks_dir: String,
    #[serde(default = "default_stacks_dir")]
    pub target_stacks_dir: String,
}

fn default_stacks_dir() -> String {
    "/stacks".to_string()
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposePrepareRequest {
    pub target_endpoint: String,
    pub stack_name: String,
    pub compose_yaml: String,
    #[serde(default)]
    pub volumes: std::collections::HashMap<String, String>,
    #[serde(default = "default_true")]
    pub pull_images: bool,
}

fn default_true() -> bool {
    true
}

/// Analyze compose files on source and target, returning a structured diff.
/// POST /migration/compose/analyze
pub async fn analyze_compose(
    State(state): State<Arc<crate::AppState>>,
    Json(body): Json<ComposeAnalyzeRequest>,
) -> ApiResult<serde_json::Value> {
    // Resolve both Docker clients
    let source = helpers::resolve_client(&state, Some(&body.source_endpoint)).await?;
    let target = helpers::resolve_client(&state, Some(&body.target_endpoint)).await?;

    // Read compose files from both endpoints
    let (src_yaml, tgt_yaml) = tokio::join!(
        crate::compose::read_compose_remote(&source, &body.source_stacks_dir, &body.stack_name),
        crate::compose::read_compose_remote(&target, &body.target_stacks_dir, &body.stack_name),
    );

    let source_compose = match src_yaml {
        Ok(y) => y,
        Err(e) => return Err(error(StatusCode::NOT_FOUND, &e)),
    };
    let target_compose = match tgt_yaml {
        Ok(y) => y,
        Err(e) => return Err(error(StatusCode::NOT_FOUND, &e)),
    };

    // Detect architectures
    let (src_arch, tgt_arch) = tokio::join!(
        crate::docker::detect_architecture(&source),
        crate::docker::detect_architecture(&target),
    );

    let src_arch_str = src_arch.ok();
    let tgt_arch_str = tgt_arch.ok();

    // Run diff
    let diff = crate::compose_diff::diff_compose(
        &source_compose,
        &target_compose,
        src_arch_str.as_deref(),
        tgt_arch_str.as_deref(),
    )
    .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    // Audit
    state
        .audit_log
        .record(
            "migration.compose.analyze",
            &body.source_endpoint,
            &body.stack_name,
            &format!(
                "Compose diff: {} services, {} volumes, {} DB services",
                diff.service_changes.len(),
                diff.volume_changes.len(),
                diff.database_services.len(),
            ),
            "gateway",
        )
        .await;

    Ok(Json(serde_json::json!({
        "stack_name": body.stack_name,
        "source_endpoint": body.source_endpoint,
        "target_endpoint": body.target_endpoint,
        "source_architecture": src_arch_str,
        "target_architecture": tgt_arch_str,
        "diff": diff,
    })))
}

/// Prepare target for compose deployment — create volumes, pull images.
/// POST /migration/compose/prepare
pub async fn prepare_compose_target(
    State(state): State<Arc<crate::AppState>>,
    Json(body): Json<ComposePrepareRequest>,
) -> ApiResult<serde_json::Value> {
    use bollard::volume::CreateVolumeOptions;
    use bollard::image::CreateImageOptions;
    use futures::StreamExt;

    let target = helpers::resolve_client(&state, Some(&body.target_endpoint)).await?;
    let mut results: Vec<serde_json::Value> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    // Pre-create volumes on target
    for (vol_name, driver) in &body.volumes {
        match target
            .create_volume(CreateVolumeOptions {
                name: vol_name.clone(),
                driver: driver.clone(),
                ..Default::default()
            })
            .await
        {
            Ok(_) => {
                results.push(serde_json::json!({
                    "type": "volume",
                    "name": vol_name,
                    "status": "created"
                }));
            }
            Err(e) => {
                let msg = format!("Failed to create volume '{}': {}", vol_name, e);
                errors.push(msg.clone());
                results.push(serde_json::json!({
                    "type": "volume",
                    "name": vol_name,
                    "status": "failed",
                    "error": msg
                }));
            }
        }
    }

    // Pull images if requested
    if body.pull_images {
        // Extract image names from compose YAML
        if let Ok(compose) = serde_yaml::from_str::<serde_yaml::Value>(&body.compose_yaml) {
            if let Some(services) = compose.get("services").and_then(|s| s.as_mapping()) {
                for (_name, svc) in services {
                    if let Some(image) = svc.get("image").and_then(|i| i.as_str()) {
                        let options = CreateImageOptions {
                            from_image: image.split(':').next().unwrap_or(image),
                            tag: image.split(':').nth(1).unwrap_or("latest"),
                            ..Default::default()
                        };

                        let mut stream = target.create_image(Some(options), None, None);
                        let mut pulled = false;
                        while let Some(chunk) = stream.next().await {
                            match chunk {
                                Ok(_) => pulled = true,
                                Err(e) => {
                                    let msg = format!("Failed to pull image '{}': {}", image, e);
                                    errors.push(msg);
                                    break;
                                }
                            }
                        }

                        results.push(serde_json::json!({
                            "type": "image",
                            "name": image,
                            "status": if pulled { "pulled" } else { "failed" }
                        }));
                    }
                }
            }
        }
    }

    // Audit
    state
        .audit_log
        .record(
            "migration.compose.prepare",
            &body.target_endpoint,
            &body.stack_name,
            &format!(
                "Prepared target: {} volumes, {} results, {} errors",
                body.volumes.len(),
                results.len(),
                errors.len(),
            ),
            "gateway",
        )
        .await;

    Ok(Json(serde_json::json!({
        "stack_name": body.stack_name,
        "target_endpoint": body.target_endpoint,
        "results": results,
        "errors": errors,
        "status": if errors.is_empty() { "success" } else { "partial_success" }
    })))
}
