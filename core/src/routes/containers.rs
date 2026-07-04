use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use bollard::container::{
    KillContainerOptions, ListContainersOptions,
    RemoveContainerOptions, RenameContainerOptions, RestartContainerOptions,
    StartContainerOptions, StopContainerOptions,
};
use bollard::models::PortBinding;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::helpers;
use crate::models::*;
use hyperlocal::UnixClientExt;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn error(code: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({"error": msg})))
}

// ── Relay helper ──────────────────────────────────────────────

/// Check if a request should be routed through the relay, and return the
/// relay hostname if so.
async fn resolve_endpoint(endpoint: &Option<String>) -> Option<String> {
    if let Some(ref ep_id) = endpoint {
        if ep_id != "local" {
            return crate::ws_relay::get_relay_for_endpoint(ep_id).await;
        }
    }
    None
}

fn extract_stack(labels: &HashMap<String, String>) -> Option<String> {
    labels
        .get("com.docker.compose.project")
        .cloned()
}

// ── Relay handler functions ───────────────────────────────────

async fn list_containers_via_relay(
    relay_host: &str,
    include_health: bool,
) -> ApiResult<Vec<ContainerSummary>> {
    use relay_protocol::Message;

    let msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "docker.ps",
        serde_json::json!({"all": true}),
    );

    let response = crate::ws_relay::send_relay_command(relay_host, msg)
        .await
        .map_err(|e| error(StatusCode::BAD_GATEWAY, &format!("Relay error: {}", e)))?;

    let containers = response
        .payload
        .get("containers")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut summaries: Vec<ContainerSummary> = containers
        .into_iter()
        .map(|c| {
            let name = c
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let labels = c
                .get("labels")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                        .collect::<HashMap<String, String>>()
                })
                .unwrap_or_default();

            ContainerSummary {
                id: c
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                name,
                image: c
                    .get("image")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                state: c
                    .get("state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                status: c
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                created: c.get("created").and_then(|v| v.as_i64()).unwrap_or(0),
                ports: vec![], // relay doesn't return detailed port info
                stack: extract_stack(&labels),
                health: None,
                labels: Some(labels),
            }
        })
        .collect();

    // Parallel health inspection when requested — skip for relay path
    if include_health && !summaries.is_empty() {
        for summary in &mut summaries {
            if let Ok(inspect_resp) =
                inspect_container_via_relay_raw(relay_host, &summary.id).await
            {
                if let Some(state) = inspect_resp.get("state") {
                    if let Some(health_status) = state.get("health").and_then(|v| v.as_str()) {
                        summary.health = Some(health_status.to_string());
                    }
                }
            }
        }
    }

    Ok(Json(summaries))
}

async fn inspect_container_via_relay_raw(
    relay_host: &str,
    container_id: &str,
) -> Result<serde_json::Value, String> {
    use relay_protocol::Message;

    let msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "docker.inspect",
        serde_json::json!({"id": container_id}),
    );

    let response = crate::ws_relay::send_relay_command(relay_host, msg).await?;
    Ok(response.payload)
}

async fn inspect_container_via_relay(
    relay_host: &str,
    id: &str,
) -> ApiResult<ContainerDetail> {
    let info = inspect_container_via_relay_raw(relay_host, id)
        .await
        .map_err(|e| error(StatusCode::BAD_GATEWAY, &format!("Relay error: {}", e)))?;

    let detail = ContainerDetail {
        id: info
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        name: info
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .trim_start_matches('/')
            .to_string(),
        image: info
            .get("image")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        state: info
            .get("state")
            .and_then(|v| v.as_object())
            .and_then(|s| s.get("status"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        status: info
            .get("state")
            .and_then(|v| v.as_object())
            .and_then(|s| s.get("status"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        created: info.get("created").and_then(|v| v.as_i64()).unwrap_or(0).to_string(),
        platform: None,
        command: None,
        env: vec![],
        ports: vec![],
        mounts: vec![],
        networks: vec![],
        restart_policy: None,
        labels: HashMap::new(),
        stack: None,
    };

    Ok(Json(detail))
}

async fn container_action_via_relay(
    relay_host: &str,
    action: &str,
    container_id: &str,
) -> ApiResult<serde_json::Value> {
    use relay_protocol::Message;

    let msg = Message::new_request(
        Uuid::new_v4().to_string(),
        action,
        serde_json::json!({"container": container_id}),
    );

    let _response = crate::ws_relay::send_relay_command(relay_host, msg)
        .await
        .map_err(|e| error(StatusCode::BAD_GATEWAY, &format!("Relay error: {}", e)))?;

    let status_verb = match action {
        "docker.start" => "started",
        "docker.stop" => "stopped",
        "docker.restart" => "restarted",
        _ => action,
    };

    Ok(Json(serde_json::json!({"status": status_verb, "id": container_id})))
}

/// Map bollard 0.17 list_containers ports (Option<Vec<Port>>) to our PortMapping.
fn map_ports_list(
    bollard_ports: &Option<Vec<bollard::models::Port>>,
) -> Vec<PortMapping> {
    match bollard_ports {
        Some(ports) => ports
            .iter()
            .map(|p| PortMapping {
                ip: p.ip.clone(),
                private_port: p.private_port,
                public_port: p.public_port,
                port_type: format!("{:?}", p.typ).to_lowercase(),
            })
            .collect(),
        None => vec![],
    }
}

/// Map bollard 0.17 NetworkSettings.ports (Option<HashMap<String, Option<Vec<PortBinding>>>>)
/// to our PortMapping struct.
fn map_ports_inspect(
    bollard_ports: &Option<HashMap<String, Option<Vec<PortBinding>>>>,
) -> Vec<PortMapping> {
    match bollard_ports {
        Some(port_map) => port_map
            .iter()
            .flat_map(|(container_port, bindings)| {
                let private_port: u16 = container_port
                    .split('/')
                    .next()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(0);
                let port_type = container_port
                    .split('/')
                    .nth(1)
                    .unwrap_or("tcp")
                    .to_string();

                match bindings {
                    Some(bindings) => bindings
                        .iter()
                        .map(move |b| PortMapping {
                            ip: b.host_ip.clone(),
                            private_port,
                            public_port: b.host_port.as_ref()
                                .and_then(|p| p.parse().ok()),
                            port_type: port_type.clone(),
                        })
                        .collect::<Vec<_>>(),
                    None => vec![PortMapping {
                        ip: None,
                        private_port,
                        public_port: None,
                        port_type: port_type.clone(),
                    }],
                }
            })
            .collect(),
        None => vec![],
    }
}

// ── List Containers ───────────────────────────────────────────

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListContainersQuery {
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub include_health: bool,
}

pub async fn list_containers(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<ListContainersQuery>,
) -> ApiResult<Vec<ContainerSummary>> {
    // ── Try relay path ────────────────────────────────────────
    if let Some(hostname) = resolve_endpoint(&params.endpoint).await {
        return list_containers_via_relay(&hostname, params.include_health).await;
    }

    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    let containers = docker
        .list_containers::<String>(Some(ListContainersOptions {
            all: true,
            ..Default::default()
        }))
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let mut summaries: Vec<ContainerSummary> = containers
        .into_iter()
        .map(|c| {
            let names = c.names.unwrap_or_default();
            let name = names
                .first()
                .map(|n| n.trim_start_matches('/').to_string())
                .unwrap_or_default();
            let labels = c.labels.unwrap_or_default();
            ContainerSummary {
                id: c.id.unwrap_or_default(),
                name,
                image: c.image.unwrap_or_default(),
                state: c.state.unwrap_or_default(),
                status: c.status.unwrap_or_default(),
                created: c.created.unwrap_or(0),
                ports: map_ports_list(&c.ports),
                stack: extract_stack(&labels),
                health: None,
                labels: Some(labels),
            }
        })
        .collect();

    // Parallel health inspection when requested
    if params.include_health && !summaries.is_empty() {
        let health_futures: Vec<_> = summaries
            .iter()
            .map(|s| {
                let d = docker.clone();
                let id = s.id.clone();
                async move {
                    d.inspect_container(&id, None)
                        .await
                        .ok()
                        .and_then(|info| {
                            info.state
                                .as_ref()
                                .and_then(|s| s.health.as_ref())
                                .and_then(|h| h.status.as_ref())
                                .map(|status| format!("{:?}", status).to_lowercase())
                        })
                }
            })
            .collect();

        let health_results = futures::future::join_all(health_futures).await;

        for (summary, health) in summaries.iter_mut().zip(health_results) {
            summary.health = health;
        }
    }

    Ok(Json(summaries))
}

// ── Inspect Container ─────────────────────────────────────────

pub async fn inspect_container(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<ContainerDetail> {
    // ── Try relay path ────────────────────────────────────────
    if let Some(hostname) = resolve_endpoint(&params.endpoint).await {
        return inspect_container_via_relay(&hostname, &id).await;
    }

    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    let info = docker
        .inspect_container(&id, None)
        .await
        .map_err(|e| error(StatusCode::NOT_FOUND, &e.to_string()))?;

    let detail = ContainerDetail {
        id: info.id.unwrap_or_default(),
        name: info
            .name
            .unwrap_or_else(|| "unknown".to_string())
            .trim_start_matches('/')
            .to_string(),
        image: info.config.as_ref().and_then(|c| c.image.clone()).unwrap_or_default(),
        state: info
            .state
            .as_ref()
            .and_then(|s| s.status.as_ref().map(|e| format!("{:?}", e).to_lowercase()))
            .unwrap_or_default(),
        status: info
            .state
            .as_ref()
            .and_then(|s| s.status.as_ref().map(|e| format!("{:?}", e).to_lowercase()))
            .unwrap_or_default(),
        created: info.created.unwrap_or_default(),
        platform: info.platform.clone(),
        command: info.config.as_ref().and_then(|c| c.cmd.clone()).map(|cmd| cmd.join(" ")),
        env: info
            .config
            .as_ref()
            .and_then(|c| c.env.clone())
            .unwrap_or_default(),
        ports: map_ports_inspect(
            &info
                .network_settings
                .as_ref()
                .and_then(|ns| ns.ports.clone()),
        ),
        mounts: info
            .mounts
            .unwrap_or_default()
            .into_iter()
            .map(|m| Mount {
                mount_type: format!(
                    "{:?}",
                    m.typ.unwrap_or(bollard::models::MountPointTypeEnum::BIND)
                )
                .to_lowercase(),
                source: m.source.unwrap_or_default(),
                destination: m.destination.unwrap_or_default(),
                mode: m.mode,
                name: m.name,
                driver: m.driver,
            })
            .collect(),
        networks: info
            .network_settings
            .as_ref()
            .and_then(|ns| ns.networks.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|(name, net)| ContainerNetwork {
                name,
                ip_address: net.ip_address.clone(),
                gateway: net.gateway.clone(),
            })
            .collect(),
        restart_policy: info
            .host_config
            .as_ref()
            .and_then(|hc| hc.restart_policy.clone())
            .map(|rp| {
                format!("{:?}", rp.name.unwrap_or(bollard::models::RestartPolicyNameEnum::NO))
                    .to_lowercase()
            }),
        labels: info.config.as_ref().and_then(|c| c.labels.clone()).unwrap_or_default(),
        stack: extract_stack(
            &info
                .config
                .as_ref()
                .and_then(|c| c.labels.clone())
                .unwrap_or_default(),
        ),
    };

    Ok(Json(detail))
}

// ── Container Actions ─────────────────────────────────────────

pub async fn start_container(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    // ── Try relay path ────────────────────────────────────────
    if let Some(hostname) = resolve_endpoint(&params.endpoint).await {
        return container_action_via_relay(&hostname, "docker.start", &id).await;
    }

    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    docker
        .start_container(&id, None::<StartContainerOptions<String>>)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({"status": "started", "id": id})))
}

pub async fn stop_container(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    // ── Try relay path ────────────────────────────────────────
    if let Some(hostname) = resolve_endpoint(&params.endpoint).await {
        return container_action_via_relay(&hostname, "docker.stop", &id).await;
    }

    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    docker
        .stop_container(&id, None::<StopContainerOptions>)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({"status": "stopped", "id": id})))
}

pub async fn restart_container(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    // ── Try relay path ────────────────────────────────────────
    if let Some(hostname) = resolve_endpoint(&params.endpoint).await {
        return container_action_via_relay(&hostname, "docker.restart", &id).await;
    }

    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    docker
        .restart_container(&id, None::<RestartContainerOptions>)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({"status": "restarted", "id": id})))
}

pub async fn kill_container(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    docker
        .kill_container(&id, None::<KillContainerOptions<String>>)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({"status": "killed", "id": id})))
}

pub async fn pause_container(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    docker
        .pause_container(&id)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({"status": "paused", "id": id})))
}

pub async fn unpause_container(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    docker
        .unpause_container(&id)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({"status": "unpaused", "id": id})))
}

#[derive(serde::Deserialize)]
pub struct RemoveQuery {
    #[serde(default)]
    endpoint: Option<String>,
    #[serde(default)]
    force: bool,
}

pub async fn remove_container(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(query): Query<RemoveQuery>,
) -> ApiResult<serde_json::Value> {
    let docker = helpers::resolve_client(&state, query.endpoint.as_deref()).await?;

    docker
        .remove_container(
            &id,
            Some(RemoveContainerOptions {
                force: query.force,
                ..Default::default()
            }),
        )
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({"status": "removed", "id": id})))
}

// ── Batch Container Actions ───────────────────────────────────

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRequest {
    pub action: String,
    pub container_ids: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    pub success: Vec<String>,
    pub failed: Vec<BatchFailure>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchFailure {
    pub id: String,
    pub error: String,
}

pub async fn batch_containers(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Json(body): Json<BatchRequest>,
) -> ApiResult<BatchResult> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    let futures: Vec<_> = body
        .container_ids
        .iter()
        .map(|id| {
            let d = docker.clone();
            let id = id.clone();
            let action = body.action.clone();
            async move {
                let result = match action.as_str() {
                    "start" => {
                        d.start_container(&id, None::<StartContainerOptions<String>>)
                            .await
                    }
                    "stop" => {
                        d.stop_container(&id, None::<StopContainerOptions>)
                            .await
                    }
                    "restart" => {
                        d.restart_container(&id, None::<RestartContainerOptions>)
                            .await
                    }
                    _ => {
                        return (id, Err("unknown action".to_string()));
                    }
                };
                match result {
                    Ok(_) => (id, Ok(())),
                    Err(e) => (id, Err(e.to_string())),
                }
            }
        })
        .collect();

    let results = futures::future::join_all(futures).await;

    let mut success = Vec::new();
    let mut failed = Vec::new();

    for (id, result) in results {
        match result {
            Ok(()) => success.push(id),
            Err(e) => failed.push(BatchFailure { id, error: e }),
        }
    }

    Ok(Json(BatchResult { success, failed }))
}

// ── Rename Container ──────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct RenameBody {
    name: String,
}

// ── Update Container Labels ───────────────────────────────────

#[derive(serde::Deserialize)]
pub struct UpdateLabelsBody {
    pub labels: std::collections::HashMap<String, String>,
}

pub async fn update_labels(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
    Json(body): Json<UpdateLabelsBody>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;
    let ep = state
        .registry
        .get(&endpoint_id)
        .await
        .ok_or_else(|| error(StatusCode::BAD_REQUEST, "Endpoint not found"))?;

    let labels_json = serde_json::json!({ "Labels": body.labels });
    let body_bytes = serde_json::to_vec(&labels_json)
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
    let path = format!("/v1.45/containers/{}/update", id);

    let result: Result<(), (StatusCode, Json<serde_json::Value>)> = if ep.connection.starts_with("unix://") {
        // Unix socket via hyperlocal
        let socket_path = ep
            .connection
            .strip_prefix("unix://")
            .unwrap_or("/var/run/docker.sock");
        let uri: hyper::Uri = hyperlocal::Uri::new(socket_path, &path).into();
        let client: hyper_util::client::legacy::Client<
            hyperlocal::UnixConnector,
            http_body_util::Full<bytes::Bytes>,
        > = hyper_util::client::legacy::Client::unix();
        let req = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(uri)
            .header(hyper::header::CONTENT_TYPE, "application/json")
            .body(http_body_util::Full::new(bytes::Bytes::from(body_bytes)))
            .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Build request error: {e}")))?;
        let resp = client
            .request(req)
            .await
            .map_err(|e| error(StatusCode::BAD_GATEWAY, &format!("Request error: {e}")))?;
        let status = resp.status();
        let body_data = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .map_err(|e| error(StatusCode::BAD_GATEWAY, &format!("Read body error: {e}")))?;
        let body_text = String::from_utf8_lossy(&body_data.to_bytes()).to_string();
        if status.is_success() {
            Ok(())
        } else {
            Err(error(
                StatusCode::BAD_GATEWAY,
                &format!("Docker API error ({}): {}", status.as_u16(), body_text),
            ))
        }
    } else {
        // TCP or TLS via reqwest
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Client build error: {e}")))?;
        let base = ep
            .connection
            .trim_end_matches('/');
        // Reqwest handles 'http://' and 'https://' natively.
        // For 'tcp://', convert to 'http://' since Docker API uses plain HTTP over TCP.
        let url = if base.starts_with("tcp://") {
            format!("http://{}{}", &base[6..], path)
        } else {
            format!("{}{}", base, path)
        };
        let resp = client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body_bytes)
            .send()
            .await
            .map_err(|e| error(StatusCode::BAD_GATEWAY, &format!("Request error: {e}")))?;
        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            let body_text = resp.text().await.unwrap_or_default();
            Err(error(
                StatusCode::BAD_GATEWAY,
                &format!("Docker API error ({}): {}", status.as_u16(), body_text),
            ))
        }
    };

    match result {
        Ok(()) => Ok(Json(
            serde_json::json!({"status": "labels_updated", "id": id}),
        )),
        Err(e) => Err(e),
    }
}

pub async fn rename_container(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
    Json(body): Json<RenameBody>,
) -> ApiResult<serde_json::Value> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    docker
        .rename_container(
            &id,
            RenameContainerOptions {
                name: body.name.clone(),
            },
        )
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({"status": "renamed", "id": id, "new_name": body.name})))
}
