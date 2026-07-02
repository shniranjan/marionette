use bollard::container::ListContainersOptions;
use bollard::Docker;
use relay_protocol::{payloads, Message, MessageType};

/// Global Docker client — connected to the host's Docker socket.
static DOCKER: std::sync::OnceLock<Docker> = std::sync::OnceLock::new();

pub fn docker_client() -> &'static Docker {
    DOCKER.get_or_init(|| {
        let host = std::env::var("DOCKER_HOST")
            .unwrap_or_else(|_| "unix:///var/run/docker.sock".into());
        Docker::connect_with_http(&host, 120, bollard::API_DEFAULT_VERSION)
            .expect(&format!("failed to connect to Docker at {}", host))
    })
}

pub async fn dispatch(msg: Message) -> Option<Message> {
    match msg.msg_type {
        MessageType::Request => handle_request(msg).await,
        _ => None,
    }
}

async fn handle_request(msg: Message) -> Option<Message> {
    match msg.subtype.as_str() {
        "ping" => handle_ping(msg).await,
        "docker.ps" => handle_docker_ps(msg).await,
        "docker.inspect" => handle_docker_inspect(msg).await,
        "host.info" => handle_host_info(msg).await,
        "fs.list" => handle_fs_list(msg).await,
        "relay.debug.state" => handle_debug_state(msg).await,
        "relay.debug.stats" => handle_debug_stats(msg).await,
        _ => Some(Message::new_error(
            msg.id,
            "RELAY.NOT_IMPLEMENTED",
            format!("Operation '{}' not yet implemented", msg.subtype),
            None,
        )),
    }
}

// ── ping ──────────────────────────────────────────────────────────

async fn handle_ping(msg: Message) -> Option<Message> {
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

async fn handle_docker_ps(msg: Message) -> Option<Message> {
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

async fn handle_docker_inspect(msg: Message) -> Option<Message> {
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

// ── host.info ─────────────────────────────────────────────────────

async fn handle_host_info(msg: Message) -> Option<Message> {
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

async fn handle_fs_list(msg: Message) -> Option<Message> {
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

// ── relay.debug.state ─────────────────────────────────────────────

async fn handle_debug_state(msg: Message) -> Option<Message> {
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

async fn handle_debug_stats(msg: Message) -> Option<Message> {
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

// ── Helpers ───────────────────────────────────────────────────────

use std::sync::OnceLock;
use std::time::Instant;
static START_TIME: OnceLock<Instant> = OnceLock::new();

fn uptime_secs() -> u64 {
    START_TIME.get_or_init(Instant::now).elapsed().as_secs()
}
