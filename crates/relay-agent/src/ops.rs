//! Operation handlers for Compose, Filesystem, Host, Image, Relay, and Debug.
//!
//! Each handler accepts a `Message` by value and an event sender for streaming.
//! Returns `Option<Message>` — `None` means the handler sent nothing back.
//!
//! Pattern: Clone `msg.id`/`msg.subtype` before moving `msg.payload` into closures.

use bollard::Docker;
use futures_util::StreamExt;
use relay_protocol::{payloads, Message};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::Instant;
use tokio::process::Command;
use tokio::sync::mpsc;

// ── Global Docker client ───────────────────────────────────────────────

static DOCKER: OnceLock<Docker> = OnceLock::new();

/// Get or initialize the global Docker client, connected via DOCKER_HOST or Unix socket.
pub fn get_docker() -> &'static Docker {
    DOCKER.get_or_init(|| {
        let host =
            std::env::var("DOCKER_HOST").unwrap_or_else(|_| "unix:///var/run/docker.sock".into());
        tracing::info!(%host, "ops: connecting to Docker");

        if let Some(path) = host.strip_prefix("unix://") {
            tracing::info!(%path, "ops: using Unix socket connection");
            Docker::connect_with_unix(path, 120, bollard::API_DEFAULT_VERSION)
                .expect(&format!("failed to connect to Docker at {}", host))
        } else {
            Docker::connect_with_http(&host, 120, bollard::API_DEFAULT_VERSION)
                .expect(&format!("failed to connect to Docker at {}", host))
        }
    })
}

// ── Uptime helper ──────────────────────────────────────────────────────

static START_TIME: OnceLock<Instant> = OnceLock::new();

fn uptime_secs() -> u64 {
    START_TIME.get_or_init(Instant::now).elapsed().as_secs()
}

// ── Dispatch ───────────────────────────────────────────────────────────

/// Route a non-Docker message to the appropriate ops handler.
pub async fn dispatch(
    msg: Message,
    event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    match msg.subtype.as_str() {
        // Compose
        "compose.up" => handle_compose_up(msg, event_tx).await,
        "compose.down" => handle_compose_down(msg, event_tx).await,
        "compose.logs" => handle_compose_logs(msg, event_tx).await,
        "compose.config" => handle_compose_config(msg, event_tx).await,

        // Filesystem
        "fs.list" => handle_fs_list(msg, event_tx).await,
        "fs.read" => handle_fs_read(msg, event_tx).await,
        "fs.write" => handle_fs_write(msg, event_tx).await,

        // Host
        "host.info" => handle_host_info(msg, event_tx).await,

        // Image
        "image.ensure" => handle_image_ensure(msg, event_tx).await,

        // Relay management
        "relay.audit" => handle_relay_audit(msg, event_tx).await,
        "relay.update" => handle_relay_update(msg, event_tx).await,

        // Volume transfer
        "volume.transfer_out" => crate::transfer::handle_volume_transfer_out(msg, event_tx).await,
        "volume.transfer_in" => crate::transfer::handle_volume_transfer_in(msg, event_tx).await,

        // Debug
        "relay.debug.state" => handle_debug_state(msg, event_tx).await,
        "relay.debug.stats" => handle_debug_stats(msg, event_tx).await,
        "relay.debug.transfer" => handle_debug_transfer(msg, event_tx).await,
        "relay.debug.events" => handle_debug_events(msg, event_tx).await,
        "relay.debug.replay" => handle_debug_replay(msg, event_tx).await,
        "relay.debug.config" => handle_debug_config(msg, event_tx).await,

        _ => None, // caller (handlers.rs) handles "not found"
    }
}

// ═══════════════════════════════════════════════════════════════════════
// COMPOSE HANDLERS
// ═══════════════════════════════════════════════════════════════════════

// ── compose.up ────────────────────────────────────────────────────────

async fn handle_compose_up(
    msg: Message,
    event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    let project_dir = msg
        .payload
        .get("project_dir")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if project_dir.is_empty() {
        return Some(Message::new_error(
            msg.id,
            "COMPOSE.NOT_AVAILABLE",
            "Missing project_dir",
            None,
        ));
    }
    let project_name = msg.payload.get("project_name").and_then(|v| v.as_str());
    let file = msg.payload.get("file").and_then(|v| v.as_str());
    let detach = msg
        .payload
        .get("detach")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let build = msg
        .payload
        .get("build")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut cmd = Command::new("docker");
    cmd.arg("compose");
    if let Some(pn) = project_name {
        cmd.args(["-p", pn]);
    }
    if let Some(f) = file {
        cmd.args(["-f", f]);
    }
    cmd.arg("up");
    if detach {
        cmd.arg("-d");
    }
    if build {
        cmd.arg("--build");
    }
    if let Some(services) = msg.payload.get("services").and_then(|v| v.as_array()) {
        for svc in services {
            if let Some(s) = svc.as_str() {
                cmd.arg(s);
            }
        }
    }
    if let Some(profiles) = msg.payload.get("profiles").and_then(|v| v.as_array()) {
        for p in profiles {
            if let Some(s) = p.as_str() {
                cmd.args(["--profile", s]);
            }
        }
    }
    cmd.current_dir(project_dir);
    if let Some(env_map) = msg.payload.get("env").and_then(|v| v.as_object()) {
        for (k, v) in env_map {
            if let Some(val) = v.as_str() {
                cmd.env(k, val);
            }
        }
    }
    run_compose_command(msg, event_tx, cmd).await
}

// ── compose.down ──────────────────────────────────────────────────────

async fn handle_compose_down(
    msg: Message,
    event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    let project_dir = msg
        .payload
        .get("project_dir")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if project_dir.is_empty() {
        return Some(Message::new_error(
            msg.id,
            "COMPOSE.NOT_AVAILABLE",
            "Missing project_dir",
            None,
        ));
    }
    let project_name = msg.payload.get("project_name").and_then(|v| v.as_str());
    let file = msg.payload.get("file").and_then(|v| v.as_str());
    let volumes = msg
        .payload
        .get("volumes")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let remove_orphans = msg
        .payload
        .get("remove_orphans")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut cmd = Command::new("docker");
    cmd.arg("compose");
    if let Some(pn) = project_name {
        cmd.args(["-p", pn]);
    }
    if let Some(f) = file {
        cmd.args(["-f", f]);
    }
    cmd.arg("down");
    if volumes {
        cmd.arg("--volumes");
    }
    if remove_orphans {
        cmd.arg("--remove-orphans");
    }
    cmd.current_dir(project_dir);
    run_compose_command(msg, event_tx, cmd).await
}

// ── compose.logs ──────────────────────────────────────────────────────

async fn handle_compose_logs(
    msg: Message,
    event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    let project_dir = msg
        .payload
        .get("project_dir")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if project_dir.is_empty() {
        return Some(Message::new_error(
            msg.id,
            "COMPOSE.NOT_AVAILABLE",
            "Missing project_dir",
            None,
        ));
    }
    let project_name = msg.payload.get("project_name").and_then(|v| v.as_str());
    let file = msg.payload.get("file").and_then(|v| v.as_str());
    let follow = msg
        .payload
        .get("follow")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut cmd = Command::new("docker");
    cmd.arg("compose");
    if let Some(pn) = project_name {
        cmd.args(["-p", pn]);
    }
    if let Some(f) = file {
        cmd.args(["-f", f]);
    }
    cmd.arg("logs");
    if follow {
        cmd.arg("--follow");
    }
    if let Some(tail) = msg.payload.get("tail").and_then(|v| v.as_u64()) {
        cmd.args(["--tail", &tail.to_string()]);
    }
    if let Some(services) = msg.payload.get("services").and_then(|v| v.as_array()) {
        for svc in services {
            if let Some(s) = svc.as_str() {
                cmd.arg(s);
            }
        }
    }
    cmd.current_dir(project_dir);
    run_compose_command(msg, event_tx, cmd).await
}

// ── compose.config ────────────────────────────────────────────────────

async fn handle_compose_config(
    msg: Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    let project_dir = msg
        .payload
        .get("project_dir")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if project_dir.is_empty() {
        return Some(Message::new_error(
            msg.id,
            "COMPOSE.NOT_AVAILABLE",
            "Missing project_dir",
            None,
        ));
    }
    let project_name = msg.payload.get("project_name").and_then(|v| v.as_str());
    let file = msg.payload.get("file").and_then(|v| v.as_str());

    let mut cmd = Command::new("docker");
    cmd.arg("compose");
    if let Some(pn) = project_name {
        cmd.args(["-p", pn]);
    }
    if let Some(f) = file {
        cmd.args(["-f", f]);
    }
    cmd.arg("config");
    cmd.current_dir(project_dir);

    match cmd.output().await {
        Ok(output) => {
            let config_yaml = String::from_utf8_lossy(&output.stdout).to_string();
            Some(Message::new_response(
                msg.id,
                "compose.config",
                serde_json::json!({"config_yaml": config_yaml}),
            ))
        }
        Err(e) => Some(Message::new_error(
            msg.id,
            "COMPOSE.CONFIG_FAILED",
            format!("{}", e),
            None,
        )),
    }
}

// ── compose helper: run a compose command and stream stdout/stderr ────

async fn run_compose_command(
    msg: Message,
    event_tx: &mpsc::UnboundedSender<Message>,
    mut cmd: tokio::process::Command,
) -> Option<Message> {
    let msg_id = msg.id.clone();
    let event_tx = event_tx.clone();
    let subtype = msg.subtype.clone();
    let start = std::time::Instant::now();

    use tokio::io::AsyncBufReadExt;

    let mut child = match cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return Some(Message::new_error(
                msg_id,
                "COMPOSE.SPAWN_FAILED",
                format!("{}", e),
                None,
            ))
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let event_tx2 = event_tx.clone();
    let msg_id2 = msg_id.clone();
    let stdout_handle: tokio::task::JoinHandle<u64> = tokio::spawn(async move {
        let mut count: u64 = 0;
        if let Some(stdout) = stdout {
            let mut reader = tokio::io::BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim_end().to_string();
                        if !trimmed.is_empty() {
                            let _ = event_tx2.send(Message::new_event(
                                &msg_id2,
                                "compose.output",
                                serde_json::json!({"stream": "stdout", "line": trimmed}),
                            ));
                            count += 1;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
        count
    });

    let event_tx3 = event_tx;
    let msg_id3 = msg_id.clone();
    let msg_id3_err = msg_id.clone();
    let stderr_handle: tokio::task::JoinHandle<u64> = tokio::spawn(async move {
        let mut count: u64 = 0;
        if let Some(stderr) = stderr {
            let mut reader = tokio::io::BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim_end().to_string();
                        if !trimmed.is_empty() {
                            let _ = event_tx3.send(Message::new_event(
                                &msg_id3,
                                "compose.output",
                                serde_json::json!({"stream": "stderr", "line": trimmed}),
                            ));
                            count += 1;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
        count
    });

    let exit_status = child.wait().await;
    let _ = stdout_handle.await;
    let _ = stderr_handle.await;

    let (exit_code, duration_ms) = match exit_status {
        Ok(status) => (
            status.code().unwrap_or(-1),
            start.elapsed().as_millis() as u64,
        ),
        Err(e) => {
            return Some(Message::new_error(
                msg_id3_err,
                "COMPOSE.WAIT_FAILED",
                format!("{}", e),
                None,
            ))
        }
    };

    Some(Message::new_response(
        msg_id,
        &subtype,
        serde_json::json!({"exit_code": exit_code, "duration_ms": duration_ms}),
    ))
}

// ═══════════════════════════════════════════════════════════════════════
// FILESYSTEM HANDLERS
// ═══════════════════════════════════════════════════════════════════════

// ── fs.read ───────────────────────────────────────────────────────────

async fn handle_fs_read(
    msg: Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    let path = msg.payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return Some(Message::new_error(
            msg.id,
            "FS.FILE_NOT_FOUND",
            "Missing 'path' field",
            None,
        ));
    }
    // Path sandboxing: restrict reads to /stacks/ prefix
    if !path.starts_with("/stacks/") {
        return Some(Message::new_error(
            msg.id,
            "FS.PATH_NOT_ALLOWED",
            "Path must be under /stacks/",
            None,
        ));
    }
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let size = content.len() as u64;
            Some(Message::new_response(
                msg.id,
                "fs.read",
                serde_json::json!({"content": content, "size_bytes": size, "encoding": "utf8"}),
            ))
        }
        Err(e) => Some(Message::new_error(
            msg.id,
            "FS.FILE_NOT_FOUND",
            format!("Failed to read '{}': {}", path, e),
            None,
        )),
    }
}

// ── fs.write ──────────────────────────────────────────────────────────

async fn handle_fs_write(
    msg: Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    let path = msg.payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let content = msg
        .payload
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if path.is_empty() {
        return Some(Message::new_error(
            msg.id,
            "FS.PATH_NOT_ALLOWED",
            "Missing 'path' field",
            None,
        ));
    }
    // Path sandboxing: restrict writes to /stacks/ prefix
    if !path.starts_with("/stacks/") {
        return Some(Message::new_error(
            msg.id,
            "FS.PATH_NOT_ALLOWED",
            "Path must be under /stacks/",
            None,
        ));
    }
    // Create parent directories if needed
    if let Some(parent) = std::path::Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return Some(Message::new_error(
                    msg.id,
                    "FS.PERMISSION_DENIED",
                    format!("Failed to create parent dirs: {}", e),
                    None,
                ));
            }
        }
    }
    match std::fs::write(path, content) {
        Ok(()) => {
            let bytes_written = content.len() as u64;
            Some(Message::new_response(
                msg.id,
                "fs.write",
                serde_json::json!({"bytes_written": bytes_written, "path": path}),
            ))
        }
        Err(e) => Some(Message::new_error(
            msg.id,
            "FS.PERMISSION_DENIED",
            format!("Failed to write '{}': {}", path, e),
            None,
        )),
    }
}

// ── fs.list ───────────────────────────────────────────────────────────

async fn handle_fs_list(
    msg: Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    let path = msg
        .payload
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("/stacks");

    match std::fs::read_dir(path) {
        Ok(entries) => {
            let items: Vec<payloads::FsEntry> = entries
                .filter_map(|e| e.ok())
                .map(|e| {
                    let file_type = e.file_type().ok();
                    let is_dir = file_type.as_ref().map(|t| t.is_dir()).unwrap_or(false);
                    let metadata = e.metadata().ok();
                    payloads::FsEntry {
                        name: e.file_name().to_string_lossy().to_string(),
                        is_dir,
                        size_bytes: if is_dir {
                            0
                        } else {
                            metadata.as_ref().map(|m| m.len()).unwrap_or(0)
                        },
                        modified: None,
                    }
                })
                .collect();

            let resp = payloads::FsListResponse {
                path: path.to_string(),
                entries: items,
            };
            Some(Message::new_response(
                msg.id,
                "fs.list",
                serde_json::to_value(resp).unwrap(),
            ))
        }
        Err(e) => Some(Message::new_error(
            msg.id,
            "FS.FILE_NOT_FOUND",
            format!("Failed to list '{}': {}", path, e),
            None,
        )),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// HOST HANDLER
// ═══════════════════════════════════════════════════════════════════════

// ── host.info ─────────────────────────────────────────────────────────

async fn handle_host_info(
    msg: Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    let docker = get_docker();

    let version = docker.version().await;
    let info = docker.info().await;

    let docker_version = version
        .as_ref()
        .ok()
        .and_then(|v| v.version.clone())
        .unwrap_or_else(|| "unknown".into());
    let api_version = version
        .as_ref()
        .ok()
        .and_then(|v| v.api_version.clone())
        .unwrap_or_else(|| "unknown".into());

    let response = payloads::HostInfoResponse {
        relay_version: env!("CARGO_PKG_VERSION").into(),
        hostname: hostname::get()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        docker: payloads::DockerInfo {
            version: docker_version,
            api_version,
            architecture: info
                .as_ref()
                .ok()
                .and_then(|i| i.architecture.clone())
                .unwrap_or_else(|| std::env::consts::ARCH.into()),
            os: info
                .as_ref()
                .ok()
                .and_then(|i| i.operating_system.clone())
                .unwrap_or_else(|| std::env::consts::OS.into()),
            cpus: info
                .as_ref()
                .ok()
                .and_then(|i| i.ncpu)
                .unwrap_or(1) as u32,
            memory_bytes: info
                .as_ref()
                .ok()
                .and_then(|i| i.mem_total)
                .unwrap_or(0) as u64,
            driver: info
                .as_ref()
                .ok()
                .and_then(|i| i.driver.clone())
                .unwrap_or_else(|| "unknown".into()),
            swarm_active: info
                .as_ref()
                .ok()
                .and_then(|i| i.swarm.as_ref())
                .and_then(|s| s.node_id.clone())
                .map(|_| true),
            compose_version: None,
        },
        uptime_secs: uptime_secs(),
    };

    Some(Message::new_response(
        msg.id,
        "host.info",
        serde_json::to_value(response).unwrap(),
    ))
}

// ═══════════════════════════════════════════════════════════════════════
// IMAGE HANDLER
// ═══════════════════════════════════════════════════════════════════════

// ── image.ensure ──────────────────────────────────────────────────────

async fn handle_image_ensure(
    msg: Message,
    event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    let docker = get_docker();
    let image = msg
        .payload
        .get("image")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if image.is_empty() {
        return Some(Message::new_error(
            msg.id,
            "IMAGE.NOT_FOUND",
            "Missing 'image' field",
            None,
        ));
    }
    let tag = msg.payload.get("tag").and_then(|v| v.as_str());
    let full_image = if let Some(t) = tag {
        format!("{}:{}", image, t)
    } else {
        image.to_string()
    };

    let msg_id = msg.id.clone();
    let msg_id_spawn = msg_id.clone();
    let event_tx = event_tx.clone();

    let handle: tokio::task::JoinHandle<Result<(bool, String), String>> = tokio::spawn(async move {
        let mut stream = docker.create_image(
            Some(bollard::image::CreateImageOptions {
                from_image: full_image.clone(),
                ..Default::default()
            }),
            None,
            None,
        );

        let mut pulled = false;
        let mut image_id = String::new();
        while let Some(item) = stream.next().await {
            match item {
                Ok(info) => {
                    if let Some(status) = &info.status {
                        let _ = event_tx.send(Message::new_event(
                            &msg_id_spawn,
                            "image.pull",
                            serde_json::json!({
                                "status": status,
                                "progress": info.progress.as_ref().map(|p| p.to_string()),
                                "id": info.id
                            }),
                        ));
                        if status.contains("Downloaded") || status.contains("Image is up to date") {
                            pulled = true;
                        }
                    }
                    if let Some(id) = &info.id {
                        image_id = id.clone();
                    }
                }
                Err(e) => {
                    return Err(format!("Pull failed: {}", e));
                }
            }
        }
        Ok((pulled, image_id))
    });

    match handle.await {
        Ok(Ok((pulled, image_id))) => Some(Message::new_response(
            msg_id,
            "image.ensure",
            serde_json::json!({"pulled": pulled, "image_id": image_id}),
        )),
        Ok(Err(e)) => Some(Message::new_error(
            msg_id,
            "IMAGE.PULL_FAILED",
            e,
            None,
        )),
        Err(e) => Some(Message::new_error(
            msg_id,
            "IMAGE.PULL_FAILED",
            format!("Join error: {}", e),
            None,
        )),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RELAY MANAGEMENT HANDLERS
// ═══════════════════════════════════════════════════════════════════════

// ── relay.audit ───────────────────────────────────────────────────────

async fn handle_relay_audit(
    msg: Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    Some(Message::new_response(
        msg.id,
        "relay.audit",
        serde_json::json!({"entries": []}),
    ))
}

// ── relay.update ──────────────────────────────────────────────────────

async fn handle_relay_update(
    msg: Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    Some(Message::new_response(
        msg.id,
        "relay.update",
        serde_json::json!({
            "updated": false,
            "new_version": "Self-update not supported in containerized deployment"
        }),
    ))
}

// ═══════════════════════════════════════════════════════════════════════
// DEBUG HANDLERS
// ═══════════════════════════════════════════════════════════════════════

// ── relay.debug.state ─────────────────────────────────────────────────

async fn handle_debug_state(
    msg: Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    let response = payloads::RelayDebugStateResponse {
        state: "CONNECTED".into(),
        uptime_secs: uptime_secs(),
        session_age_secs: None,
        active_requests: 0,
        locked_volumes: vec![],
        locked_projects: vec![],
    };
    Some(Message::new_response(
        msg.id,
        "relay.debug.state",
        serde_json::to_value(response).unwrap(),
    ))
}

// ── Per-operation statistics (production-grade atomics) ──────────

pub static TOTAL_CALLS: AtomicU64 = AtomicU64::new(0);
pub static TOTAL_ERRORS: AtomicU64 = AtomicU64::new(0);

/// Increment the total call counter (called from handlers::dispatch).
pub fn increment_calls() {
    TOTAL_CALLS.fetch_add(1, Ordering::Relaxed);
}

/// Increment the total error counter (called from handlers::dispatch).
pub fn increment_errors() {
    TOTAL_ERRORS.fetch_add(1, Ordering::Relaxed);
}

// ── Transfer tracking ────────────────────────────────────────────

/// Completed transfer counter.
pub static COMPLETED_TRANSFERS: AtomicU64 = AtomicU64::new(0);

/// Active transfer states keyed by transfer_id.
static ACTIVE_TRANSFERS: LazyLock<Mutex<std::collections::HashMap<String, TransferSnapshot>>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// Snapshot of an active transfer.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TransferSnapshot {
    pub volume: String,
    pub bytes_sent: u64,
    pub total_bytes: u64,
    pub direction: String,
    pub started_at: String,
}

/// Register the start of a transfer (called by transfer handlers).
pub fn track_transfer_start(transfer_id: &str, volume: &str, direction: &str) {
    if let Ok(mut map) = ACTIVE_TRANSFERS.lock() {
        map.insert(
            transfer_id.to_string(),
            TransferSnapshot {
                volume: volume.to_string(),
                bytes_sent: 0,
                total_bytes: 0,
                direction: direction.to_string(),
                started_at: chrono::Utc::now().to_rfc3339(),
            },
        );
    }
}

/// Mark a transfer as complete (called by transfer handlers).
pub fn track_transfer_complete(transfer_id: &str) {
    if let Ok(mut map) = ACTIVE_TRANSFERS.lock() {
        map.remove(transfer_id);
    }
    COMPLETED_TRANSFERS.fetch_add(1, Ordering::Relaxed);
}

// ── Event tracking ring buffer ───────────────────────────────────

/// Total events emitted.
pub static EVENT_COUNT: AtomicU64 = AtomicU64::new(0);

/// A recent event record.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RecentEvent {
    pub timestamp: String,
    pub subtype: String,
    pub summary: String,
}

/// Ring buffer of recent events (max 100).
static RECENT_EVENTS: LazyLock<Mutex<VecDeque<RecentEvent>>> =
    LazyLock::new(|| Mutex::new(VecDeque::with_capacity(100)));

/// Push an event into the ring buffer.
pub fn track_event(subtype: &str, summary: &str) {
    EVENT_COUNT.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut queue) = RECENT_EVENTS.lock() {
        if queue.len() >= 100 {
            queue.pop_front();
        }
        queue.push_back(RecentEvent {
            timestamp: chrono::Utc::now().to_rfc3339(),
            subtype: subtype.to_string(),
            summary: summary.to_string(),
        });
    }
}

// ── Debug: Stats ─────────────────────────────────────────────────

async fn handle_debug_stats(
    msg: Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    let response = payloads::RelayDebugStatsResponse {
        total_calls: TOTAL_CALLS.load(Ordering::Relaxed),
        total_errors: TOTAL_ERRORS.load(Ordering::Relaxed),
        uptime_secs: uptime_secs(),
    };
    Some(Message::new_response(
        msg.id,
        "relay.debug.stats",
        serde_json::to_value(response).unwrap(),
    ))
}

// ── relay.debug.transfer ──────────────────────────────────────────────

async fn handle_debug_transfer(
    msg: Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    let (active, completed) = {
        let active_count = ACTIVE_TRANSFERS
            .lock()
            .map(|map| map.len() as u32)
            .unwrap_or(0);
        let completed_count = COMPLETED_TRANSFERS.load(Ordering::Relaxed);
        (active_count, completed_count)
    };
    let response = payloads::RelayDebugTransferResponse {
        active_transfers: active,
        completed_transfers: completed,
    };
    Some(Message::new_response(
        msg.id,
        "relay.debug.transfer",
        serde_json::to_value(response).unwrap(),
    ))
}

// ── relay.debug.events ────────────────────────────────────────────────

async fn handle_debug_events(
    msg: Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    let (total, recent) = {
        let total = EVENT_COUNT.load(Ordering::Relaxed);
        let recent = RECENT_EVENTS
            .lock()
            .map(|queue| {
                queue
                    .iter()
                    .map(|e| format!("{} {} {}", e.timestamp, e.subtype, e.summary))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        (total, recent)
    };
    let response = payloads::RelayDebugEventsResponse {
        total_events: total,
        recent_events: recent,
    };
    Some(Message::new_response(
        msg.id,
        "relay.debug.events",
        serde_json::to_value(response).unwrap(),
    ))
}

// ── relay.debug.replay ────────────────────────────────────────────────

async fn handle_debug_replay(
    msg: Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    Some(Message::new_error(
        msg.id,
        "RELAY.NOT_IMPLEMENTED",
        "Replay not supported — events are transient",
        None,
    ))
}

// ── relay.debug.config ────────────────────────────────────────────────

async fn handle_debug_config(
    msg: Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Option<Message> {
    let relay_token = std::env::var("RELAY_TOKEN").ok().map(|tok| {
        if tok.len() <= 8 {
            "***".to_string()
        } else {
            format!("{}...{}", &tok[..4], &tok[tok.len() - 4..])
        }
    });
    let config = serde_json::json!({
        "marionette_url": std::env::var("MARIONETTE_URL").unwrap_or_default(),
        "docker_host": std::env::var("DOCKER_HOST").unwrap_or_default(),
        "rust_log": std::env::var("RUST_LOG").unwrap_or_default(),
        "relay_token": relay_token,
        "hostname": hostname::get().unwrap_or_default().to_string_lossy().to_string(),
        "arch": std::env::consts::ARCH,
        "os": std::env::consts::OS,
    });
    Some(Message::new_response(
        msg.id,
        "relay.debug.config",
        serde_json::json!({"config": config}),
    ))
}
