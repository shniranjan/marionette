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

use crate::helpers;
use crate::models::*;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn error(code: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({"error": msg})))
}

fn extract_stack(labels: &HashMap<String, String>) -> Option<String> {
    labels
        .get("com.docker.compose.project")
        .cloned()
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

pub async fn list_containers(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<Vec<ContainerSummary>> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    let containers = docker
        .list_containers::<String>(Some(ListContainersOptions {
            all: true,
            ..Default::default()
        }))
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let summaries: Vec<ContainerSummary> = containers
        .into_iter()
        .map(|c| {
            let names = c.names.unwrap_or_default();
            let name = names
                .first()
                .map(|n| n.trim_start_matches('/').to_string())
                .unwrap_or_default();
            ContainerSummary {
                id: c.id.unwrap_or_default(),
                name,
                image: c.image.unwrap_or_default(),
                state: c.state.unwrap_or_default(),
                status: c.status.unwrap_or_default(),
                created: c.created.unwrap_or(0),
                ports: map_ports_list(&c.ports),
                stack: extract_stack(&c.labels.unwrap_or_default()),
            }
        })
        .collect();

    Ok(Json(summaries))
}

// ── Inspect Container ─────────────────────────────────────────

pub async fn inspect_container(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<ContainerDetail> {
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

// ── Rename Container ──────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct RenameBody {
    name: String,
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
