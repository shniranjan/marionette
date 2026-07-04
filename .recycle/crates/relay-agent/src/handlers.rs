use bollard::container::ListContainersOptions;
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::Docker;
use futures_util::StreamExt;
use relay_protocol::{payloads, Message, MessageType};
use tokio::process::Command;
use tokio::sync::mpsc;

/// Global Docker client — connected to the host's Docker socket.
static DOCKER: std::sync::OnceLock<Docker> = std::sync::OnceLock::new();

pub fn docker_client() -> &'static Docker {
    DOCKER.get_or_init(|| {
        let host = std::env::var("DOCKER_HOST")
            .unwrap_or_else(|_| "unix:///var/run/docker.sock".into());
        tracing::info!(%host, "connecting to Docker");

        // bollard's connect_with_http only supports http:// and https:// URLs.
        // For unix:// sockets we must use connect_with_unix instead.
        if let Some(path) = host.strip_prefix("unix://") {
            tracing::info!(%path, "using Unix socket connection");
            Docker::connect_with_unix(path, 120, bollard::API_DEFAULT_VERSION)
                .expect(&format!("failed to connect to Docker at {}", host))
        } else {
            Docker::connect_with_http(&host, 120, bollard::API_DEFAULT_VERSION)
                .expect(&format!("failed to connect to Docker at {}", host))
        }
    })
}

pub async fn dispatch(msg: Message, event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    match msg.msg_type {
        MessageType::Request => handle_request(msg, event_tx).await,
        _ => None,
    }
}

async fn handle_request(msg: Message, event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    match msg.subtype.as_str() {
        "ping" => handle_ping(msg, event_tx).await,
        "docker.ps" => handle_docker_ps(msg, event_tx).await,
        "docker.inspect" => handle_docker_inspect(msg, event_tx).await,
        "docker.stop" => handle_docker_stop(msg, event_tx).await,
        "docker.start" => handle_docker_start(msg, event_tx).await,
        "docker.restart" => handle_docker_restart(msg, event_tx).await,
        "docker.exec" => handle_docker_exec(msg, event_tx).await,
        "docker.logs" => handle_docker_logs(msg, event_tx).await,
        "docker.stats" => handle_docker_stats(msg, event_tx).await,
        "compose.up" => handle_compose_up(msg, event_tx).await,
        "compose.down" => handle_compose_down(msg, event_tx).await,
        "compose.stop" => handle_compose_stop(msg, event_tx).await,
        "compose.logs" => handle_compose_logs(msg, event_tx).await,
        "compose.config" => handle_compose_config(msg, event_tx).await,
        "image.ensure" => handle_image_ensure(msg, event_tx).await,
        "volume.transfer_out" => handle_volume_transfer_out(msg, event_tx).await,
        "volume.transfer_in" => handle_volume_transfer_in(msg, event_tx).await,
        "host.info" => handle_host_info(msg, event_tx).await,
        "fs.list" => handle_fs_list(msg, event_tx).await,
        "fs.read" => handle_fs_read(msg, event_tx).await,
        "fs.write" => handle_fs_write(msg, event_tx).await,
        "relay.debug.state" => handle_debug_state(msg, event_tx).await,
        "relay.debug.stats" => handle_debug_stats(msg, event_tx).await,
        "relay.audit" => handle_relay_audit(msg, event_tx).await,
        "relay.update" => handle_relay_update(msg, event_tx).await,
        "relay.debug.transfer" => handle_debug_transfer(msg, event_tx).await,
        "relay.debug.events" => handle_debug_events(msg, event_tx).await,
        "relay.debug.replay" => handle_debug_replay(msg, event_tx).await,
        "relay.debug.config" => handle_debug_config(msg, event_tx).await,
        _ => Some(Message::new_error(
            msg.id,
            "RELAY.NOT_IMPLEMENTED",
            format!("Operation '{}' not yet implemented", msg.subtype),
            None,
        )),
    }
}

// ── ping ──────────────────────────────────────────────────────────

async fn handle_ping(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let docker = docker_client();
    let version = docker
        .version()
        .await
        .map(|v| v.version.unwrap_or_else(|| "unknown".into()))
        .unwrap_or_else(|_| "unknown".into());

    let pong = payloads::PongResponse {
        uptime_secs: uptime_secs(),
        docker_version: version,
        arch: std::env::consts::ARCH.into(),
        os: std::env::consts::OS.into(),
        relay_version: Some(env!("CARGO_PKG_VERSION").into()),
    };
    Some(Message::new_response(
        msg.id,
        "pong",
        serde_json::to_value(pong).unwrap(),
    ))
}

// ── docker.ps ─────────────────────────────────────────────────────

async fn handle_docker_ps(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let docker = docker_client();

    let req: payloads::DockerPsRequest =
        match serde_json::from_value(msg.payload.clone()) {
            Ok(r) => r,
            Err(e) => {
                return Some(Message::new_error(
                    msg.id,
                    "RELAY.INVALID_PAYLOAD",
                    format!("Failed to parse docker.ps request: {}", e),
                    None,
                ));
            }
        };

    let mut opts = ListContainersOptions::<String> {
        all: req.all,
        limit: req.limit.map(|l| l as isize),
        ..Default::default()
    };

    if let Some(ref filter) = req.filter {
        opts.filters.insert("name".into(), vec![filter.clone()]);
    }

    match docker.list_containers(Some(opts)).await {
        Ok(containers) => {
            let summaries: Vec<payloads::ContainerSummary> = containers
                .into_iter()
                .map(|c| payloads::ContainerSummary {
                    id: c.id.unwrap_or_default().chars().take(12).collect(),
                    name: c
                        .names
                        .unwrap_or_default()
                        .first()
                        .cloned()
                        .unwrap_or_default()
                        .trim_start_matches('/')
                        .to_string(),
                    image: c.image.unwrap_or_default(),
                    state: c.state.unwrap_or_default(),
                    status: c.status,
                    ports: c.ports.map(|p| {
                        p.iter()
                            .map(|port| {
                                if let Some(public) = &port.public_port {
                                    format!(
                                        "{}:{}/{}",
                                        port.ip.as_deref().unwrap_or(""),
                                        public,
                                        port.private_port
                                    )
                                } else {
                                    format!("{}/tcp", port.private_port)
                                }
                            })
                            .collect()
                    }),
                })
                .collect();

            let resp = payloads::DockerPsResponse {
                containers: summaries,
            };
            Some(Message::new_response(
                msg.id,
                "docker.ps",
                serde_json::to_value(resp).unwrap(),
            ))
        }
        Err(e) => Some(Message::new_error(
            msg.id,
            "DOCKER.ERROR",
            format!("Failed to list containers: {}", e),
            None,
        )),
    }
}

// ── docker.inspect ────────────────────────────────────────────────

async fn handle_docker_inspect(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let docker = docker_client();

    let container_id = msg
        .payload
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if container_id.is_empty() {
        return Some(Message::new_error(
            msg.id,
            "RELAY.INVALID_PAYLOAD",
            "Missing 'id' field in docker.inspect request",
            None,
        ));
    }

    match docker.inspect_container(container_id, None).await {
        Ok(info) => {
            let result = serde_json::json!({
                "id": info.id.unwrap_or_default(),
                "name": info.name.unwrap_or_default().trim_start_matches('/'),
                "image": info.config.as_ref().and_then(|c| c.image.clone()).unwrap_or_default(),
                "state": {
                    "status": info.state.as_ref().and_then(|s| s.status.as_ref().map(|st| format!("{:?}", st))).unwrap_or_else(|| "unknown".into()),
                    "running": info.state.as_ref().and_then(|s| s.running).unwrap_or(false),
                    "paused": info.state.as_ref().and_then(|s| s.paused).unwrap_or(false),
                    "started_at": info.state.as_ref().and_then(|s| s.started_at.clone()),
                },
                "created": info.created,
            });
            Some(Message::new_response(msg.id, "docker.inspect", result))
        }
        Err(e) => Some(Message::new_error(
            msg.id,
            "DOCKER.ERROR",
            format!("Failed to inspect container '{}': {}", container_id, e),
            None,
        )),
    }
}

// ── docker.stop ───────────────────────────────────────────────────

async fn handle_docker_stop(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let docker = docker_client();
    let container = msg.payload.get("container").and_then(|v| v.as_str()).unwrap_or("");
    if container.is_empty() {
        return Some(Message::new_error(msg.id, "DOCKER.CONTAINER_NOT_FOUND", "Missing 'container' field", None));
    }
    let timeout = msg.payload.get("timeout_secs").and_then(|v| v.as_i64());
    let start = std::time::Instant::now();
    match docker.stop_container(container, timeout.map(|t| bollard::container::StopContainerOptions { t })).await {
        Ok(_) => {
            let resp = payloads::DockerStopResponse { container: container.into(), stopped: true, duration_ms: start.elapsed().as_millis() as u64 };
            Some(Message::new_response(msg.id, "docker.stop", serde_json::to_value(resp).unwrap()))
        }
        Err(e) => Some(Message::new_error(msg.id, "DOCKER.STOP_FAILED", format!("{}", e), None)),
    }
}

// ── docker.start ──────────────────────────────────────────────────

async fn handle_docker_start(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let docker = docker_client();
    let container = msg.payload.get("container").and_then(|v| v.as_str()).unwrap_or("");
    if container.is_empty() {
        return Some(Message::new_error(msg.id, "DOCKER.CONTAINER_NOT_FOUND", "Missing 'container' field", None));
    }
    match docker.start_container::<String>(container, None).await {
        Ok(_) => {
            let resp = payloads::DockerStartResponse { container: container.into(), started: true };
            Some(Message::new_response(msg.id, "docker.start", serde_json::to_value(resp).unwrap()))
        }
        Err(e) => Some(Message::new_error(msg.id, "DOCKER.START_FAILED", format!("{}", e), None)),
    }
}

// ── docker.restart ────────────────────────────────────────────────

async fn handle_docker_restart(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let docker = docker_client();
    let container = msg.payload.get("container").and_then(|v| v.as_str()).unwrap_or("");
    if container.is_empty() {
        return Some(Message::new_error(msg.id, "DOCKER.CONTAINER_NOT_FOUND", "Missing 'container' field", None));
    }
    let timeout = msg.payload.get("timeout_secs").and_then(|v| v.as_i64());
    let start = std::time::Instant::now();
    let t_isize = timeout.map(|t| t as isize);
    match docker.restart_container(container, t_isize.map(|t| bollard::container::RestartContainerOptions { t })).await {
        Ok(_) => {
            let resp = payloads::DockerRestartResponse { container: container.into(), restarted: true, duration_ms: start.elapsed().as_millis() as u64 };
            Some(Message::new_response(msg.id, "docker.restart", serde_json::to_value(resp).unwrap()))
        }
        Err(e) => Some(Message::new_error(msg.id, "DOCKER.RESTART_FAILED", format!("{}", e), None)),
    }
}

// ── docker.logs (streaming) ───────────────────────────────────────

async fn handle_docker_logs(msg: Message, event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let docker = docker_client();
    let container = msg.payload.get("container").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if container.is_empty() {
        return Some(Message::new_error(msg.id, "DOCKER.CONTAINER_NOT_FOUND", "Missing 'container' field", None));
    }
    let follow = msg.payload.get("follow").and_then(|v| v.as_bool()).unwrap_or(false);
    let tail = msg.payload.get("tail").and_then(|v| v.as_u64()).map(|t| t as isize);
    let stdout = msg.payload.get("stdout").and_then(|v| v.as_bool()).unwrap_or(true);
    let stderr = msg.payload.get("stderr").and_then(|v| v.as_bool()).unwrap_or(true);

    let mut opts = bollard::container::LogsOptions::<String> {
        follow, stdout, stderr,
        ..Default::default()
    };
    if let Some(t) = tail { opts.tail = t.to_string(); }
    if let Some(s) = msg.payload.get("since").and_then(|v| v.as_str()) {
        if let Ok(ts) = s.parse::<i64>() { opts.since = ts; }
    }
    if let Some(u) = msg.payload.get("until").and_then(|v| v.as_str()) {
        if let Ok(ts) = u.parse::<i64>() { opts.until = ts; }
    }
    if msg.payload.get("timestamps").and_then(|v| v.as_bool()).unwrap_or(false) { opts.timestamps = true; }

    let msg_id = msg.id.clone();
    let msg_id_spawn = msg_id.clone();
    let event_tx = event_tx.clone();

    let handle: tokio::task::JoinHandle<Result<(u64, bool), String>> = tokio::spawn(async move {
        let mut stream = docker.logs(&container, Some(opts));
        let mut count: u64 = 0;
        let mut follow_ended = false;
        while let Some(item) = stream.next().await {
            match item {
                Ok(bollard::container::LogOutput::StdOut { message }) => {
                    let line = String::from_utf8_lossy(&message).to_string();
                    let _ = event_tx.send(Message::new_event(&msg_id_spawn, "docker.logs", serde_json::json!({
                        "stream": "stdout", "line": line.trim_end()
                    })));
                    count += 1;
                }
                Ok(bollard::container::LogOutput::StdErr { message }) => {
                    let line = String::from_utf8_lossy(&message).to_string();
                    let _ = event_tx.send(Message::new_event(&msg_id_spawn, "docker.logs", serde_json::json!({
                        "stream": "stderr", "line": line.trim_end()
                    })));
                    count += 1;
                }
                Ok(bollard::container::LogOutput::StdIn { .. }) => {}
                Ok(bollard::container::LogOutput::Console { message }) => {
                    let line = String::from_utf8_lossy(&message).to_string();
                    let _ = event_tx.send(Message::new_event(&msg_id_spawn, "docker.logs", serde_json::json!({
                        "stream": "stdout", "line": line.trim_end()
                    })));
                    count += 1;
                }
                Err(e) => {
                    tracing::debug!(error = %e, "logs stream ended or errored");
                    follow_ended = true;
                    break;
                }
            }
        }
        if !follow { follow_ended = true; }
        Ok((count, follow_ended))
    });

    match handle.await {
        Ok(Ok((count, follow_ended))) => {
            let resp = payloads::DockerLogsResponse { lines_streamed: count, follow_ended };
            Some(Message::new_response(msg_id, "docker.logs", serde_json::to_value(resp).unwrap()))
        }
        Ok(Err(e)) => Some(Message::new_error(msg_id, "DOCKER.LOGS_STREAM_ERROR", e, None)),
        Err(e) => Some(Message::new_error(msg_id, "DOCKER.LOGS_STREAM_ERROR", format!("Join error: {}", e), None)),
    }
}

// ── docker.stats (streaming) ──────────────────────────────────────

async fn handle_docker_stats(msg: Message, event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let docker = docker_client();
    let one_shot = msg.payload.get("one_shot").and_then(|v| v.as_bool()).unwrap_or(false);
    let should_stream = msg.payload.get("stream").and_then(|v| v.as_bool()).unwrap_or(true) && !one_shot;

    // Get list of containers to monitor
    let containers: Vec<String> = msg.payload.get("containers")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let target_containers = if containers.is_empty() {
        // If no containers specified, list all running containers
        match docker.list_containers::<String>(None).await {
            Ok(list) => list.iter()
                .filter_map(|c| c.names.as_ref().and_then(|n| n.first()).map(|n| n.trim_start_matches('/').to_string()))
                .collect(),
            Err(e) => {
                return Some(Message::new_error(msg.id, "DOCKER.STATS_STREAM_ERROR", format!("Failed to list containers: {}", e), None));
            }
        }
    } else {
        containers
    };

    if target_containers.is_empty() {
        let resp = payloads::DockerStatsResponse { snapshots_sent: 0 };
        return Some(Message::new_response(msg.id, "docker.stats", serde_json::to_value(resp).unwrap()));
    }

    let msg_id = msg.id.clone();
    let msg_id_spawn = msg_id.clone();
    let event_tx = event_tx.clone();

    let handle: tokio::task::JoinHandle<Result<u64, String>> = tokio::spawn(async move {
        let mut snapshots: u64 = 0;

        for container_name in &target_containers {
            let opts = Some(bollard::container::StatsOptions { stream: should_stream, one_shot });
            let mut stream = docker.stats(container_name, opts);
            while let Some(item) = stream.next().await {
                match item {
                    Ok(stats) => {
                        let mut snapshot = build_stats_snapshot(&stats);
                        snapshot.container = container_name.clone();
                        let _ = event_tx.send(Message::new_event(&msg_id_spawn, "docker.stats", serde_json::to_value(snapshot).unwrap()));
                        snapshots += 1;
                        if !should_stream && snapshots >= 1 { break; }
                    }
                    Err(e) => {
                        tracing::debug!(error = %e, "stats stream error");
                        if !should_stream { break; }
                    }
                }
            }
            if !should_stream && snapshots >= 1 { break; }
        }

        Ok(snapshots)
    });

    match handle.await {
        Ok(Ok(snapshots_sent)) => {
            let resp = payloads::DockerStatsResponse { snapshots_sent };
            Some(Message::new_response(msg_id, "docker.stats", serde_json::to_value(resp).unwrap()))
        }
        Ok(Err(e)) => Some(Message::new_error(msg_id, "DOCKER.STATS_STREAM_ERROR", e, None)),
        Err(e) => Some(Message::new_error(msg_id, "DOCKER.STATS_STREAM_ERROR", format!("Join error: {}", e), None)),
    }
}

/// Build a DockerStatsSnapshot from bollard's stats.
fn build_stats_snapshot(stats: &bollard::container::Stats) -> payloads::DockerStatsSnapshot {
    let cpu = &stats.cpu_stats;
    let precpu = &stats.precpu_stats;
    let mem = &stats.memory_stats;

    let cpu_delta = cpu.cpu_usage.total_usage as f64 - precpu.cpu_usage.total_usage as f64;
    let system_cpu_usage = cpu.system_cpu_usage.unwrap_or(0);
    let precpu_system_cpu_usage = precpu.system_cpu_usage.unwrap_or(0);
    let system_delta = system_cpu_usage as f64 - precpu_system_cpu_usage as f64;
    let cpu_percent = if system_delta > 0.0 && cpu_delta > 0.0 {
        (cpu_delta / system_delta) * (cpu.online_cpus.unwrap_or(1) as f64) * 100.0
    } else { 0.0 };

    let memory_usage = mem.usage.unwrap_or(0);
    let memory_limit = mem.limit.unwrap_or(0);
    let memory_percent = if memory_limit > 0 { (memory_usage as f64 / memory_limit as f64) * 100.0 } else { 0.0 };

    let mut snapshot = payloads::DockerStatsSnapshot {
        container: stats.name.trim_start_matches('/').to_string(),
        timestamp: stats.read.clone(),
        cpu_percent,
        memory_usage_bytes: memory_usage,
        memory_limit_bytes: memory_limit,
        memory_percent,
        network_rx_bytes: 0,
        network_tx_bytes: 0,
        block_read_bytes: 0,
        block_write_bytes: 0,
        pids: stats.pids_stats.current.unwrap_or(0) as u32,
    };

    // Network stats
    if let Some(net) = &stats.networks {
        for (_iface, net_stats) in net.iter() {
            snapshot.network_rx_bytes += net_stats.rx_bytes;
            snapshot.network_tx_bytes += net_stats.tx_bytes;
        }
    }

    // Block I/O stats
    if let Some(ops) = stats.blkio_stats.io_service_bytes_recursive.as_ref() {
        for op in ops {
            match op.op.as_str() {
                "read" => snapshot.block_read_bytes += op.value,
                "write" => snapshot.block_write_bytes += op.value,
                _ => {}
            }
        }
    }

    snapshot
}

// ── docker.exec (streaming) ───────────────────────────────────────

async fn handle_docker_exec(msg: Message, event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let docker = docker_client();
    let container = msg.payload.get("container").and_then(|v| v.as_str()).unwrap_or("");
    if container.is_empty() {
        return Some(Message::new_error(msg.id, "DOCKER.CONTAINER_NOT_FOUND", "Missing 'container' field", None));
    }
    let cmd: Vec<String> = msg.payload.get("cmd")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    if cmd.is_empty() {
        return Some(Message::new_error(msg.id, "DOCKER.EXEC_FAILED", "Missing 'cmd' array", None));
    }
    let attach_stdout = msg.payload.get("attach_stdout").and_then(|v| v.as_bool()).unwrap_or(true);
    let attach_stderr = msg.payload.get("attach_stderr").and_then(|v| v.as_bool()).unwrap_or(true);
    let attach_stdin = msg.payload.get("attach_stdin").and_then(|v| v.as_bool()).unwrap_or(false);
    let workdir = msg.payload.get("workdir").and_then(|v| v.as_str()).map(String::from);
    let user = msg.payload.get("user").and_then(|v| v.as_str()).map(String::from);

    let exec_opts = CreateExecOptions {
        attach_stdout: Some(attach_stdout),
        attach_stderr: Some(attach_stderr),
        attach_stdin: Some(attach_stdin),
        cmd: Some(cmd.clone()),
        working_dir: workdir,
        user,
        ..Default::default()
    };

    let exec = match docker.create_exec(&container, exec_opts).await {
        Ok(e) => e,
        Err(e) => return Some(Message::new_error(msg.id, "DOCKER.EXEC_FAILED", format!("{}", e), None)),
    };

    let start = std::time::Instant::now();
    match docker.start_exec(&exec.id, None).await {
        Ok(StartExecResults::Attached { mut output, .. }) => {
            let msg_id = msg.id.clone();
            let msg_id_spawn = msg_id.clone();
            let event_tx = event_tx.clone();
            let exec_id = exec.id.clone();
            let handle: tokio::task::JoinHandle<i64> = tokio::spawn(async move {
                while let Some(item) = output.next().await {
                    match item {
                        Ok(bollard::container::LogOutput::StdOut { message }) => {
                            let data = String::from_utf8_lossy(&message).to_string();
                            let _ = event_tx.send(Message::new_event(&msg_id_spawn, "docker.exec.stdout",
                                serde_json::json!({"data": data})));
                        }
                        Ok(bollard::container::LogOutput::StdErr { message }) => {
                            let data = String::from_utf8_lossy(&message).to_string();
                            let _ = event_tx.send(Message::new_event(&msg_id_spawn, "docker.exec.stderr",
                                serde_json::json!({"data": data})));
                        }
                        _ => {}
                    }
                }
                // Get exit code from inspect
                match docker_client().inspect_exec(&exec_id).await {
                    Ok(details) => details.exit_code.unwrap_or(-1),
                    Err(_) => -1,
                }
            });
            match handle.await {
                Ok(exit_code) => {
                    let resp = payloads::DockerExecResponse { exit_code, duration_ms: start.elapsed().as_millis() as u64 };
                    Some(Message::new_response(msg_id, "docker.exec", serde_json::to_value(resp).unwrap()))
                }
                Err(e) => Some(Message::new_error(msg_id, "DOCKER.EXEC_FAILED", format!("Join error: {}", e), None)),
            }
        }
        Ok(_) => Some(Message::new_error(msg.id, "DOCKER.EXEC_FAILED", "Exec not attached", None)),
        Err(e) => Some(Message::new_error(msg.id, "DOCKER.EXEC_FAILED", format!("{}", e), None)),
    }
}

// ── image.ensure ──────────────────────────────────────────────────

async fn handle_image_ensure(msg: Message, event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let docker = docker_client();
    let image = msg.payload.get("image").and_then(|v| v.as_str()).unwrap_or("");
    if image.is_empty() {
        return Some(Message::new_error(msg.id, "IMAGE.INVALID", "Missing 'image' field", None));
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
                        let _ = event_tx.send(Message::new_event(&msg_id_spawn, "image.pull",
                            serde_json::json!({
                                "status": status,
                                "progress": info.progress.as_ref().map(|p| p.to_string()),
                                "id": info.id
                            })));
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
        Ok(Ok((pulled, image_id))) => {
            Some(Message::new_response(msg_id, "image.ensure",
                serde_json::json!({"pulled": pulled, "image_id": image_id})))
        }
        Ok(Err(e)) => Some(Message::new_error(msg_id, "IMAGE.PULL_FAILED", e, None)),
        Err(e) => Some(Message::new_error(msg_id, "IMAGE.PULL_FAILED", format!("Join error: {}", e), None)),
    }
}

// ── host.info ─────────────────────────────────────────────────────

async fn handle_host_info(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let docker = docker_client();

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

// ── fs.list ───────────────────────────────────────────────────────

async fn handle_fs_list(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let path = msg
        .payload
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("/stacks");

    match std::fs::read_dir(path) {
        Ok(entries) => {
            let items: Vec<serde_json::Value> = entries
                .filter_map(|e| e.ok())
                .map(|e| {
                    let file_type = e.file_type().ok();
                    let is_dir = file_type.as_ref().map(|t| t.is_dir()).unwrap_or(false);
                    serde_json::json!({
                        "name": e.file_name().to_string_lossy(),
                        "is_dir": is_dir,
                        "size": if is_dir { 0 } else { e.metadata().map(|m| m.len()).unwrap_or(0) },
                    })
                })
                .collect();

            Some(Message::new_response(
                msg.id,
                "fs.list",
                serde_json::json!({ "path": path, "entries": items }),
            ))
        }
        Err(e) => Some(Message::new_error(
            msg.id,
            "FS.ERROR",
            format!("Failed to list '{}': {}", path, e),
            None,
        )),
    }
}

// ── fs.read ───────────────────────────────────────────────────────

async fn handle_fs_read(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let path = msg.payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return Some(Message::new_error(msg.id, "FS.INVALID_PATH", "Missing 'path' field", None));
    }
    // Path sandboxing: restrict reads to /stacks/ prefix
    if !path.starts_with("/stacks/") {
        return Some(Message::new_error(msg.id, "FS.FORBIDDEN", "Path must be under /stacks/", None));
    }
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let size = content.len() as u64;
            Some(Message::new_response(msg.id, "fs.read",
                serde_json::json!({"content": content, "size_bytes": size, "encoding": "utf8"})))
        }
        Err(e) => Some(Message::new_error(msg.id, "FS.READ_ERROR", format!("Failed to read '{}': {}", path, e), None)),
    }
}

// ── fs.write ──────────────────────────────────────────────────────

async fn handle_fs_write(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let path = msg.payload.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let content = msg.payload.get("content").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return Some(Message::new_error(msg.id, "FS.INVALID_PATH", "Missing 'path' field", None));
    }
    // Path sandboxing: restrict writes to /stacks/ prefix
    if !path.starts_with("/stacks/") {
        return Some(Message::new_error(msg.id, "FS.FORBIDDEN", "Path must be under /stacks/", None));
    }
    // Create parent directories if needed
    if let Some(parent) = std::path::Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return Some(Message::new_error(msg.id, "FS.WRITE_ERROR", format!("Failed to create parent dirs: {}", e), None));
            }
        }
    }
    match std::fs::write(path, content) {
        Ok(()) => {
            let bytes_written = content.len() as u64;
            Some(Message::new_response(msg.id, "fs.write",
                serde_json::json!({"bytes_written": bytes_written, "path": path})))
        }
        Err(e) => Some(Message::new_error(msg.id, "FS.WRITE_ERROR", format!("Failed to write '{}': {}", path, e), None)),
    }
}

// ── relay.debug.state ─────────────────────────────────────────────

async fn handle_debug_state(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
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

// ── relay.debug.stats ─────────────────────────────────────────────

/// Simple stats counter. In production this would use atomic counters.
static mut TOTAL_CALLS: u64 = 0;
static mut TOTAL_ERRORS: u64 = 0;

async fn handle_debug_stats(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let (calls, errors) = unsafe { (TOTAL_CALLS, TOTAL_ERRORS) };
    let response = payloads::RelayDebugStatsResponse {
        total_calls: calls,
        total_errors: errors,
        uptime_secs: uptime_secs(),
    };
    Some(Message::new_response(
        msg.id,
        "relay.debug.stats",
        serde_json::to_value(response).unwrap(),
    ))
}

// ── compose.up ────────────────────────────────────────────────────

async fn handle_compose_up(msg: Message, event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let project_dir = msg.payload.get("project_dir").and_then(|v| v.as_str()).unwrap_or("");
    if project_dir.is_empty() {
        return Some(Message::new_error(msg.id, "COMPOSE.INVALID_DIR", "Missing project_dir", None));
    }
    let project_name = msg.payload.get("project_name").and_then(|v| v.as_str());
    let file = msg.payload.get("file").and_then(|v| v.as_str());
    let detach = msg.payload.get("detach").and_then(|v| v.as_bool()).unwrap_or(true);
    let build = msg.payload.get("build").and_then(|v| v.as_bool()).unwrap_or(false);

    let mut cmd = Command::new("docker");
    cmd.arg("compose");
    if let Some(pn) = project_name { cmd.args(["-p", pn]); }
    if let Some(f) = file { cmd.args(["-f", f]); }
    cmd.arg("up");
    if detach { cmd.arg("-d"); }
    if build { cmd.arg("--build"); }
    if let Some(services) = msg.payload.get("services").and_then(|v| v.as_array()) {
        for svc in services { if let Some(s) = svc.as_str() { cmd.arg(s); } }
    }
    if let Some(profiles) = msg.payload.get("profiles").and_then(|v| v.as_array()) {
        for p in profiles { if let Some(s) = p.as_str() { cmd.args(["--profile", s]); } }
    }
    cmd.current_dir(project_dir);
    if let Some(env_map) = msg.payload.get("env").and_then(|v| v.as_object()) {
        for (k, v) in env_map { if let Some(val) = v.as_str() { cmd.env(k, val); } }
    }
    run_compose_command(msg, event_tx, cmd).await
}

// ── compose.down ──────────────────────────────────────────────────

async fn handle_compose_down(msg: Message, event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let project_dir = msg.payload.get("project_dir").and_then(|v| v.as_str()).unwrap_or("");
    if project_dir.is_empty() {
        return Some(Message::new_error(msg.id, "COMPOSE.INVALID_DIR", "Missing project_dir", None));
    }
    let project_name = msg.payload.get("project_name").and_then(|v| v.as_str());
    let file = msg.payload.get("file").and_then(|v| v.as_str());
    let volumes = msg.payload.get("volumes").and_then(|v| v.as_bool()).unwrap_or(false);
    let remove_orphans = msg.payload.get("remove_orphans").and_then(|v| v.as_bool()).unwrap_or(false);

    let mut cmd = Command::new("docker");
    cmd.arg("compose");
    if let Some(pn) = project_name { cmd.args(["-p", pn]); }
    if let Some(f) = file { cmd.args(["-f", f]); }
    cmd.arg("down");
    if volumes { cmd.arg("--volumes"); }
    if remove_orphans { cmd.arg("--remove-orphans"); }
    cmd.current_dir(project_dir);
    run_compose_command(msg, event_tx, cmd).await
}

// ── compose.stop ──────────────────────────────────────────────────

async fn handle_compose_stop(msg: Message, event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let project_dir = msg.payload.get("project_dir").and_then(|v| v.as_str()).unwrap_or("");
    if project_dir.is_empty() {
        return Some(Message::new_error(msg.id, "COMPOSE.INVALID_DIR", "Missing project_dir", None));
    }
    let project_name = msg.payload.get("project_name").and_then(|v| v.as_str());
    let file = msg.payload.get("file").and_then(|v| v.as_str());

    let mut cmd = Command::new("docker");
    cmd.arg("compose");
    if let Some(pn) = project_name { cmd.args(["-p", pn]); }
    if let Some(f) = file { cmd.args(["-f", f]); }
    cmd.arg("stop");
    cmd.current_dir(project_dir);
    run_compose_command(msg, event_tx, cmd).await
}

// ── compose.logs ──────────────────────────────────────────────────

async fn handle_compose_logs(msg: Message, event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let project_dir = msg.payload.get("project_dir").and_then(|v| v.as_str()).unwrap_or("");
    if project_dir.is_empty() {
        return Some(Message::new_error(msg.id, "COMPOSE.INVALID_DIR", "Missing project_dir", None));
    }
    let project_name = msg.payload.get("project_name").and_then(|v| v.as_str());
    let file = msg.payload.get("file").and_then(|v| v.as_str());
    let follow = msg.payload.get("follow").and_then(|v| v.as_bool()).unwrap_or(false);

    let mut cmd = Command::new("docker");
    cmd.arg("compose");
    if let Some(pn) = project_name { cmd.args(["-p", pn]); }
    if let Some(f) = file { cmd.args(["-f", f]); }
    cmd.arg("logs");
    if follow { cmd.arg("--follow"); }
    if let Some(tail) = msg.payload.get("tail").and_then(|v| v.as_u64()) {
        cmd.args(["--tail", &tail.to_string()]);
    }
    if let Some(services) = msg.payload.get("services").and_then(|v| v.as_array()) {
        for svc in services { if let Some(s) = svc.as_str() { cmd.arg(s); } }
    }
    cmd.current_dir(project_dir);
    run_compose_command(msg, event_tx, cmd).await
}

// ── compose.config ────────────────────────────────────────────────

async fn handle_compose_config(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let project_dir = msg.payload.get("project_dir").and_then(|v| v.as_str()).unwrap_or("");
    if project_dir.is_empty() {
        return Some(Message::new_error(msg.id, "COMPOSE.INVALID_DIR", "Missing project_dir", None));
    }
    let project_name = msg.payload.get("project_name").and_then(|v| v.as_str());
    let file = msg.payload.get("file").and_then(|v| v.as_str());

    let mut cmd = Command::new("docker");
    cmd.arg("compose");
    if let Some(pn) = project_name { cmd.args(["-p", pn]); }
    if let Some(f) = file { cmd.args(["-f", f]); }
    cmd.arg("config");
    cmd.current_dir(project_dir);

    match cmd.output().await {
        Ok(output) => {
            let config_yaml = String::from_utf8_lossy(&output.stdout).to_string();
            Some(Message::new_response(msg.id, "compose.config",
                serde_json::json!({"config_yaml": config_yaml})))
        }
        Err(e) => Some(Message::new_error(msg.id, "COMPOSE.CONFIG_FAILED", format!("{}", e), None)),
    }
}

// ── compose helper ────────────────────────────────────────────────

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
        Err(e) => return Some(Message::new_error(msg_id, "COMPOSE.SPAWN_FAILED", format!("{}", e), None)),
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
                            let _ = event_tx2.send(Message::new_event(&msg_id2, "compose.output",
                                serde_json::json!({"stream": "stdout", "line": trimmed})));
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
                            let _ = event_tx3.send(Message::new_event(&msg_id3, "compose.output",
                                serde_json::json!({"stream": "stderr", "line": trimmed})));
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
        Ok(status) => (status.code().unwrap_or(-1), start.elapsed().as_millis() as u64),
        Err(e) => return Some(Message::new_error(msg_id3_err, "COMPOSE.WAIT_FAILED", format!("{}", e), None)),
    };

    Some(Message::new_response(msg_id, &subtype,
        serde_json::json!({"exit_code": exit_code, "duration_ms": duration_ms})))
}

// ── volume.transfer_out (stub) ────────────────────────────────────

async fn handle_volume_transfer_out(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    Some(Message::new_error(msg.id, "RELAY.NOT_IMPLEMENTED",
        "Volume transfer requires relay-to-relay protocol — coming in Stage 8", None))
}

// ── volume.transfer_in (stub) ─────────────────────────────────────

async fn handle_volume_transfer_in(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    Some(Message::new_error(msg.id, "RELAY.NOT_IMPLEMENTED",
        "Volume transfer requires relay-to-relay protocol — coming in Stage 8", None))
}

// ── relay.audit (stub) ────────────────────────────────────────────

async fn handle_relay_audit(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    Some(Message::new_response(msg.id, "relay.audit",
        serde_json::json!({"entries": []})))
}

// ── relay.update (stub) ───────────────────────────────────────────

async fn handle_relay_update(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    Some(Message::new_response(msg.id, "relay.update",
        serde_json::json!({"updated": false, "new_version": "Self-update not supported in containerized deployment"})))
}

// ── relay.debug.transfer (stub) ───────────────────────────────────

async fn handle_debug_transfer(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    Some(Message::new_response(msg.id, "relay.debug.transfer",
        serde_json::json!({"active_transfers": 0, "completed_transfers": 0})))
}

// ── relay.debug.events (stub) ─────────────────────────────────────

async fn handle_debug_events(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    Some(Message::new_response(msg.id, "relay.debug.events",
        serde_json::json!({"total_events": 0, "recent_events": []})))
}

// ── relay.debug.replay (stub) ─────────────────────────────────────

async fn handle_debug_replay(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    Some(Message::new_error(msg.id, "RELAY.NOT_IMPLEMENTED",
        "Replay not supported — events are transient", None))
}

// ── relay.debug.config (stub) ─────────────────────────────────────

async fn handle_debug_config(msg: Message, _event_tx: &mpsc::UnboundedSender<Message>) -> Option<Message> {
    let config = serde_json::json!({
        "marionette_url": std::env::var("MARIONETTE_URL").unwrap_or_default(),
        "docker_host": std::env::var("DOCKER_HOST").unwrap_or_default(),
        "rust_log": std::env::var("RUST_LOG").unwrap_or_default(),
        "hostname": hostname::get().unwrap_or_default().to_string_lossy().to_string(),
        "arch": std::env::consts::ARCH,
        "os": std::env::consts::OS,
    });
    Some(Message::new_response(msg.id, "relay.debug.config",
        serde_json::json!({"config": config})))
}

// ── Helpers ───────────────────────────────────────────────────────

use std::sync::OnceLock;
use std::time::Instant;
static START_TIME: OnceLock<Instant> = OnceLock::new();

fn uptime_secs() -> u64 {
    START_TIME.get_or_init(Instant::now).elapsed().as_secs()
}
