//! Docker operation handlers for the relay agent.
//!
//! Each handler accepts a reference to the incoming Message and an
//! unbounded sender for streaming events. Non-streaming handlers
//! return a single response; streaming handlers spawn a task that
//! sends events and return an immediate acknowledgment.

use std::sync::OnceLock;

use bollard::container::{
    ListContainersOptions, LogsOptions, LogOutput, RestartContainerOptions, StatsOptions,
    StopContainerOptions,
};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::Docker;
use futures_util::StreamExt;
use relay_protocol::{payloads, ErrorCode, ErrorPayload, Message};
use tokio::sync::mpsc;

// ── Global Docker client ──────────────────────────────────────────

static DOCKER: OnceLock<Docker> = OnceLock::new();

pub fn docker_client() -> &'static Docker {
    DOCKER.get_or_init(|| {
        let host =
            std::env::var("DOCKER_HOST").unwrap_or_else(|_| "unix:///var/run/docker.sock".into());
        tracing::info!(%host, "connecting to Docker");

        if let Some(path) = host.strip_prefix("unix://") {
            tracing::info!(%path, "using Unix socket connection");
            Docker::connect_with_unix(path, 120, bollard::API_DEFAULT_VERSION)
                .unwrap_or_else(|e| panic!("failed to connect to Docker at {}: {}", host, e))
        } else {
            Docker::connect_with_http(&host, 120, bollard::API_DEFAULT_VERSION)
                .unwrap_or_else(|e| panic!("failed to connect to Docker at {}: {}", host, e))
        }
    })
}

// ── docker.ps ─────────────────────────────────────────────────────

pub async fn handle_docker_ps(
    msg: &Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Result<Message, ErrorPayload> {
    let docker = docker_client();

    let req: payloads::DockerPsRequest = serde_json::from_value(msg.payload.clone()).map_err(
        |e| {
            ErrorPayload::new(
                ErrorCode::InvalidMessage,
                format!("Failed to parse docker.ps request: {}", e),
            )
        },
    )?;

    let mut opts = ListContainersOptions::<String> {
        all: req.all,
        limit: req.limit.map(|l| l as isize),
        ..Default::default()
    };

    if let Some(ref filter) = req.filter {
        opts.filters
            .insert("name".into(), vec![filter.clone()]);
    }

    docker
        .list_containers(Some(opts))
        .await
        .map(|containers| {
            let summaries: Vec<payloads::ContainerSummary> = containers
                .into_iter()
                .map(|c| payloads::ContainerSummary {
                    id: c
                        .id
                        .unwrap_or_default()
                        .chars()
                        .take(12)
                        .collect(),
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
            Message::new_response(&msg.id, "docker.ps", serde_json::to_value(resp).unwrap())
        })
        .map_err(|e| {
            ErrorPayload::new(
                ErrorCode::DockerDaemonUnreachable,
                format!("Failed to list containers: {}", e),
            )
        })
}

// ── docker.inspect ────────────────────────────────────────────────

pub async fn handle_docker_inspect(
    msg: &Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Result<Message, ErrorPayload> {
    let docker = docker_client();

    let req: payloads::DockerInspectRequest =
        serde_json::from_value(msg.payload.clone()).map_err(|e| {
            ErrorPayload::new(
                ErrorCode::InvalidMessage,
                format!("Failed to parse docker.inspect request: {}", e),
            )
        })?;

    if req.container.is_empty() {
        return Err(ErrorPayload::new(
            ErrorCode::DockerContainerNotFound,
            "Missing 'container' field in docker.inspect request",
        ));
    }

    docker
        .inspect_container(&req.container, None)
        .await
        .map(|info| {
            let result = serde_json::json!({
                "id": info.id.unwrap_or_default(),
                "name": info.name.unwrap_or_default().trim_start_matches('/'),
                "image": info.config.as_ref().and_then(|c| c.image.clone()).unwrap_or_default(),
                "state": {
                    "status": info.state.as_ref()
                        .and_then(|s| s.status.as_ref().map(|st| format!("{:?}", st)))
                        .unwrap_or_else(|| "unknown".into()),
                    "running": info.state.as_ref().and_then(|s| s.running).unwrap_or(false),
                    "paused": info.state.as_ref().and_then(|s| s.paused).unwrap_or(false),
                    "started_at": info.state.as_ref().and_then(|s| s.started_at.clone()),
                },
                "created": info.created,
            });
            Message::new_response(&msg.id, "docker.inspect", result)
        })
        .map_err(|e| {
            ErrorPayload::new(
                ErrorCode::DockerContainerNotFound,
                format!("Failed to inspect container '{}': {}", req.container, e),
            )
        })
}

// ── docker.stop ───────────────────────────────────────────────────

pub async fn handle_docker_stop(
    msg: &Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Result<Message, ErrorPayload> {
    let docker = docker_client();

    let req: payloads::DockerStopRequest =
        serde_json::from_value(msg.payload.clone()).map_err(|e| {
            ErrorPayload::new(
                ErrorCode::InvalidMessage,
                format!("Failed to parse docker.stop request: {}", e),
            )
        })?;

    if req.container.is_empty() {
        return Err(ErrorPayload::new(
            ErrorCode::DockerContainerNotFound,
            "Missing 'container' field",
        ));
    }

    let start = std::time::Instant::now();
    let timeout_opts = req
        .timeout_secs
        .map(|t| StopContainerOptions { t });

    docker
        .stop_container(&req.container, timeout_opts)
        .await
        .map(|_| {
            let resp = payloads::DockerStopResponse {
                container: req.container.clone(),
                stopped: true,
                duration_ms: start.elapsed().as_millis() as u64,
            };
            Message::new_response(&msg.id, "docker.stop", serde_json::to_value(resp).unwrap())
        })
        .map_err(|e| {
            ErrorPayload::new(
                ErrorCode::DockerDaemonUnreachable,
                format!("Failed to stop container '{}': {}", req.container, e),
            )
        })
}

// ── docker.start ──────────────────────────────────────────────────

pub async fn handle_docker_start(
    msg: &Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Result<Message, ErrorPayload> {
    let docker = docker_client();

    let req: payloads::DockerStartRequest =
        serde_json::from_value(msg.payload.clone()).map_err(|e| {
            ErrorPayload::new(
                ErrorCode::InvalidMessage,
                format!("Failed to parse docker.start request: {}", e),
            )
        })?;

    if req.container.is_empty() {
        return Err(ErrorPayload::new(
            ErrorCode::DockerContainerNotFound,
            "Missing 'container' field",
        ));
    }

    docker
        .start_container::<String>(&req.container, None)
        .await
        .map(|_| {
            let resp = payloads::DockerStartResponse {
                container: req.container.clone(),
                started: true,
            };
            Message::new_response(&msg.id, "docker.start", serde_json::to_value(resp).unwrap())
        })
        .map_err(|e| {
            ErrorPayload::new(
                ErrorCode::DockerDaemonUnreachable,
                format!("Failed to start container '{}': {}", req.container, e),
            )
        })
}

// ── docker.restart ────────────────────────────────────────────────

pub async fn handle_docker_restart(
    msg: &Message,
    _event_tx: &mpsc::UnboundedSender<Message>,
) -> Result<Message, ErrorPayload> {
    let docker = docker_client();

    let req: payloads::DockerRestartRequest =
        serde_json::from_value(msg.payload.clone()).map_err(|e| {
            ErrorPayload::new(
                ErrorCode::InvalidMessage,
                format!("Failed to parse docker.restart request: {}", e),
            )
        })?;

    if req.container.is_empty() {
        return Err(ErrorPayload::new(
            ErrorCode::DockerContainerNotFound,
            "Missing 'container' field",
        ));
    }

    let start = std::time::Instant::now();
    let timeout_opts = req
        .timeout_secs
        .map(|t| RestartContainerOptions { t: t as isize });

    docker
        .restart_container(&req.container, timeout_opts)
        .await
        .map(|_| {
            let resp = payloads::DockerRestartResponse {
                container: req.container.clone(),
                restarted: true,
                duration_ms: start.elapsed().as_millis() as u64,
            };
            Message::new_response(
                &msg.id,
                "docker.restart",
                serde_json::to_value(resp).unwrap(),
            )
        })
        .map_err(|e| {
            ErrorPayload::new(
                ErrorCode::DockerDaemonUnreachable,
                format!("Failed to restart container '{}': {}", req.container, e),
            )
        })
}

// ── docker.exec (streaming) ───────────────────────────────────────

pub async fn handle_docker_exec(
    msg: &Message,
    event_tx: &mpsc::UnboundedSender<Message>,
) -> Result<Message, ErrorPayload> {
    let docker = docker_client();

    let req: payloads::DockerExecRequest =
        serde_json::from_value(msg.payload.clone()).map_err(|e| {
            ErrorPayload::new(
                ErrorCode::InvalidMessage,
                format!("Failed to parse docker.exec request: {}", e),
            )
        })?;

    if req.container.is_empty() {
        return Err(ErrorPayload::new(
            ErrorCode::DockerContainerNotFound,
            "Missing 'container' field",
        ));
    }

    if req.cmd.is_empty() {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidMessage,
            "Missing 'cmd' array",
        ));
    }

    let exec_opts = CreateExecOptions {
        attach_stdout: Some(req.attach_stdout),
        attach_stderr: Some(req.attach_stderr),
        attach_stdin: Some(req.attach_stdin),
        cmd: Some(req.cmd.clone()),
        working_dir: req.workdir.clone(),
        user: req.user.clone(),
        ..Default::default()
    };

    let exec = docker.create_exec(&req.container, exec_opts).await.map_err(|e| {
        ErrorPayload::new(
            ErrorCode::DockerExecDenied,
            format!("Failed to create exec: {}", e),
        )
    })?;

    let exec_id = exec.id.clone();
    let msg_id = msg.id.clone();
    let event_tx = event_tx.clone();

    match docker.start_exec(&exec_id, None).await {
        Ok(StartExecResults::Attached { mut output, .. }) => {
            // Spawn a task to consume the output stream and send events.
            tokio::spawn(async move {
                while let Some(item) = output.next().await {
                    match item {
                        Ok(LogOutput::StdOut { message }) => {
                            let data = String::from_utf8_lossy(&message).to_string();
                            let _ = event_tx.send(Message::new_event(
                                &msg_id,
                                "docker.exec.stdout",
                                serde_json::json!({"data": data}),
                            ));
                        }
                        Ok(LogOutput::StdErr { message }) => {
                            let data = String::from_utf8_lossy(&message).to_string();
                            let _ = event_tx.send(Message::new_event(
                                &msg_id,
                                "docker.exec.stderr",
                                serde_json::json!({"data": data}),
                            ));
                        }
                        _ => {}
                    }
                }

                // Get exit code from inspect
                let exit_code = match docker_client().inspect_exec(&exec_id).await {
                    Ok(details) => details.exit_code.unwrap_or(-1),
                    Err(_) => -1,
                };
                let _ = event_tx.send(Message::new_event(
                    &msg_id,
                    "docker.exec.exit",
                    serde_json::json!({"exit_code": exit_code}),
                ));
            });

            // Return immediate acknowledgment
            Ok(Message::new_response(
                &msg.id,
                "docker.exec",
                serde_json::json!({"status": "started", "exec_id": exec.id}),
            ))
        }
        Ok(_) => Err(ErrorPayload::new(
            ErrorCode::DockerExecDenied,
            "Exec not attached",
        )),
        Err(e) => Err(ErrorPayload::new(
            ErrorCode::DockerExecDenied,
            format!("Failed to start exec: {}", e),
        )),
    }
}

// ── docker.logs (streaming) ───────────────────────────────────────

pub async fn handle_docker_logs(
    msg: &Message,
    event_tx: &mpsc::UnboundedSender<Message>,
) -> Result<Message, ErrorPayload> {
    let docker = docker_client();

    let req: payloads::DockerLogsRequest =
        serde_json::from_value(msg.payload.clone()).map_err(|e| {
            ErrorPayload::new(
                ErrorCode::InvalidMessage,
                format!("Failed to parse docker.logs request: {}", e),
            )
        })?;

    if req.container.is_empty() {
        return Err(ErrorPayload::new(
            ErrorCode::DockerContainerNotFound,
            "Missing 'container' field",
        ));
    }

    let mut opts = LogsOptions::<String> {
        follow: req.follow,
        stdout: req.stdout,
        stderr: req.stderr,
        ..Default::default()
    };

    if let Some(t) = req.tail {
        opts.tail = t.to_string();
    }
    if let Some(ref s) = req.since {
        if let Ok(ts) = s.parse::<i64>() {
            opts.since = ts;
        }
    }
    if let Some(ref u) = req.until {
        if let Ok(ts) = u.parse::<i64>() {
            opts.until = ts;
        }
    }
    if req.timestamps {
        opts.timestamps = true;
    }

    let container = req.container.clone();
    let msg_id = msg.id.clone();
    let event_tx = event_tx.clone();

    // Spawn a task to consume the log stream and send events.
    tokio::spawn(async move {
        let mut stream = docker.logs(&container, Some(opts));
        let mut count: u64 = 0;
        let mut follow_ended = false;

        while let Some(item) = stream.next().await {
            match item {
                Ok(LogOutput::StdOut { message }) => {
                    let line = String::from_utf8_lossy(&message).to_string();
                    let _ = event_tx.send(Message::new_event(
                        &msg_id,
                        "docker.logs",
                        serde_json::json!({"stream": "stdout", "line": line.trim_end()}),
                    ));
                    count += 1;
                }
                Ok(LogOutput::StdErr { message }) => {
                    let line = String::from_utf8_lossy(&message).to_string();
                    let _ = event_tx.send(Message::new_event(
                        &msg_id,
                        "docker.logs",
                        serde_json::json!({"stream": "stderr", "line": line.trim_end()}),
                    ));
                    count += 1;
                }
                Ok(LogOutput::StdIn { .. }) => {}
                Ok(LogOutput::Console { message }) => {
                    let line = String::from_utf8_lossy(&message).to_string();
                    let _ = event_tx.send(Message::new_event(
                        &msg_id,
                        "docker.logs",
                        serde_json::json!({"stream": "stdout", "line": line.trim_end()}),
                    ));
                    count += 1;
                }
                Err(e) => {
                    tracing::debug!(error = %e, "logs stream ended or errored");
                    follow_ended = true;
                    break;
                }
            }
        }

        if !req.follow {
            follow_ended = true;
        }

        let resp = payloads::DockerLogsResponse {
            lines_streamed: count,
            follow_ended,
        };
        let _ = event_tx.send(Message::new_response(
            &msg_id,
            "docker.logs",
            serde_json::to_value(resp).unwrap(),
        ));
    });

    // Return immediate acknowledgment
    Ok(Message::new_response(
        &msg.id,
        "docker.logs",
        serde_json::json!({"status": "streaming"}),
    ))
}

// ── docker.stats (streaming) ──────────────────────────────────────

pub async fn handle_docker_stats(
    msg: &Message,
    event_tx: &mpsc::UnboundedSender<Message>,
) -> Result<Message, ErrorPayload> {
    let docker = docker_client();

    let req: payloads::DockerStatsRequest =
        serde_json::from_value(msg.payload.clone()).map_err(|e| {
            ErrorPayload::new(
                ErrorCode::InvalidMessage,
                format!("Failed to parse docker.stats request: {}", e),
            )
        })?;

    let one_shot = req.one_shot;
    let should_stream = req.stream && !one_shot;

    // Get list of containers to monitor
    let target_containers: Vec<String> = match &req.containers {
        Some(containers) if !containers.is_empty() => containers.clone(),
        _ => {
            // If no containers specified, list all running containers
            match docker.list_containers::<String>(None).await {
                Ok(list) => list
                    .iter()
                    .filter_map(|c| {
                        c.names
                            .as_ref()
                            .and_then(|n| n.first())
                            .map(|n| n.trim_start_matches('/').to_string())
                    })
                    .collect(),
                Err(e) => {
                    return Err(ErrorPayload::new(
                        ErrorCode::DockerDaemonUnreachable,
                        format!("Failed to list containers for stats: {}", e),
                    ));
                }
            }
        }
    };

    if target_containers.is_empty() {
        let resp = payloads::DockerStatsResponse { snapshots_sent: 0 };
        return Ok(Message::new_response(
            &msg.id,
            "docker.stats",
            serde_json::to_value(resp).unwrap(),
        ));
    }

    let msg_id = msg.id.clone();
    let event_tx = event_tx.clone();

    // Spawn a task to consume stats streams and send events.
    tokio::spawn(async move {
        let mut snapshots: u64 = 0;

        for container_name in &target_containers {
            let opts = Some(StatsOptions {
                stream: should_stream,
                one_shot,
            });
            let mut stream = docker.stats(container_name, opts);

            while let Some(item) = stream.next().await {
                match item {
                    Ok(stats) => {
                        let mut snapshot = build_stats_snapshot(&stats);
                        snapshot.container = container_name.clone();
                        let _ = event_tx.send(Message::new_event(
                            &msg_id,
                            "docker.stats",
                            serde_json::to_value(snapshot).unwrap(),
                        ));
                        snapshots += 1;
                        if !should_stream && snapshots >= 1 {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::debug!(error = %e, "stats stream error");
                        if !should_stream {
                            break;
                        }
                    }
                }
            }
            if !should_stream && snapshots >= 1 {
                break;
            }
        }

        let resp = payloads::DockerStatsResponse {
            snapshots_sent: snapshots,
        };
        let _ = event_tx.send(Message::new_response(
            &msg_id,
            "docker.stats",
            serde_json::to_value(resp).unwrap(),
        ));
    });

    // Return immediate acknowledgment
    Ok(Message::new_response(
        &msg.id,
        "docker.stats",
        serde_json::json!({"status": "streaming"}),
    ))
}

// ── Stats snapshot builder ────────────────────────────────────────

/// Build a DockerStatsSnapshot from bollard's Stats struct.
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
    } else {
        0.0
    };

    let memory_usage = mem.usage.unwrap_or(0);
    let memory_limit = mem.limit.unwrap_or(0);
    let memory_percent = if memory_limit > 0 {
        (memory_usage as f64 / memory_limit as f64) * 100.0
    } else {
        0.0
    };

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
