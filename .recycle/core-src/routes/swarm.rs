use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use bollard::container::{LogOutput, LogsOptions};
use bollard::service::{
    InspectServiceOptions, ListServicesOptions, ServiceSpec, ServiceSpecMode,
    ServiceSpecModeReplicated, TaskSpec, TaskSpecContainerSpec, EndpointSpec,
    UpdateServiceOptions,
};
use bollard::secret::{ListSecretsOptions, SecretSpec};
use bollard::models::{EndpointPortConfig, EndpointPortConfigProtocolEnum};
use futures::StreamExt;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use http_body_util::{BodyExt, Full};
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;

use crate::helpers;
use crate::models::*;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn error(code: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({"error": msg})))
}

// ── Raw Docker API Helper ──────────────────────────────────────
//
// bollard 0.17.1 does not expose Swarm init/join/leave/inspect,
// node CRUD, task listing/inspection, or config CRUD.
// We use hyper + hyperlocal to hit those Docker endpoints directly.

enum RawClient {
    #[cfg(unix)]
    Unix(Client<hyperlocal::UnixConnector, Full<Bytes>>),
    Tcp(Client<hyper_util::client::legacy::connect::HttpConnector, Full<Bytes>>),
}

async fn raw_client(state: &Arc<crate::AppState>, endpoint_id: &str) -> Result<(RawClient, String), String> {
    let endpoint = state.registry.get(endpoint_id)
        .await
        .ok_or_else(|| format!("Endpoint not found: {}", endpoint_id))?;
    let connection = endpoint.connection;

    if connection.starts_with("unix://") {
        #[cfg(unix)]
        {
            let path = connection.strip_prefix("unix://").unwrap_or("/var/run/docker.sock");
            let connector = hyperlocal::UnixConnector;
            let client: Client<hyperlocal::UnixConnector, Full<Bytes>> =
                Client::builder(TokioExecutor::new()).build(connector);
            Ok((RawClient::Unix(client), format!("http+unix://{}", path)))
        }
        #[cfg(not(unix))]
        {
            Err("Unix sockets not supported on this platform".to_string())
        }
    } else if connection.starts_with("tcp://") {
        let base = connection.to_string();
        let connector = hyper_util::client::legacy::connect::HttpConnector::new();
        let client: Client<hyper_util::client::legacy::connect::HttpConnector, Full<Bytes>> =
            Client::builder(TokioExecutor::new()).build(connector);
        Ok((RawClient::Tcp(client), base))
    } else {
        Err(format!("Unsupported connection scheme: {}", connection))
    }
}

fn build_url(base: &str, path: &str) -> String {
    if base.starts_with("http+unix://") {
        // For Unix sockets, the URL host is irrelevant; hyperlocal routes by path.
        format!("http://localhost/v1.45{}", path)
    } else {
        format!("{}/v1.45{}", base, path)
    }
}

async fn raw_get(state: &Arc<crate::AppState>, endpoint_id: &str, path: &str) -> Result<serde_json::Value, String> {
    let (client, base) = raw_client(state, endpoint_id).await?;
    let url = build_url(&base, path);

    let req = hyper::Request::builder()
        .method("GET")
        .uri(&url)
        .body(Full::new(Bytes::new()))
        .map_err(|e| format!("Request build error: {}", e))?;

    let resp = match &client {
        #[cfg(unix)]
        RawClient::Unix(c) => c.request(req).await,
        RawClient::Tcp(c) => c.request(req).await,
    }
    .map_err(|e| format!("HTTP error: {}", e))?;

    let status = resp.status();
    let body_bytes = resp.into_body().collect().await
        .map_err(|e| format!("Body read error: {}", e))?.to_bytes();

    if !status.is_success() {
        let body_str = String::from_utf8_lossy(&body_bytes);
        return Err(format!("Docker API error {}: {}", status.as_u16(), body_str));
    }

    serde_json::from_slice(&body_bytes).map_err(|e| format!("JSON parse error: {}", e))
}

async fn raw_get_array(state: &Arc<crate::AppState>, endpoint_id: &str, path: &str) -> Result<Vec<serde_json::Value>, String> {
    let (client, base) = raw_client(state, endpoint_id).await?;
    let url = build_url(&base, path);

    let req = hyper::Request::builder()
        .method("GET")
        .uri(&url)
        .body(Full::new(Bytes::new()))
        .map_err(|e| format!("Request build error: {}", e))?;

    let resp = match &client {
        #[cfg(unix)]
        RawClient::Unix(c) => c.request(req).await,
        RawClient::Tcp(c) => c.request(req).await,
    }
    .map_err(|e| format!("HTTP error: {}", e))?;

    let status = resp.status();
    let body_bytes = resp.into_body().collect().await
        .map_err(|e| format!("Body read error: {}", e))?.to_bytes();

    if !status.is_success() {
        let body_str = String::from_utf8_lossy(&body_bytes);
        return Err(format!("Docker API error {}: {}", status.as_u16(), body_str));
    }

    serde_json::from_slice(&body_bytes).map_err(|e| format!("JSON parse error: {}", e))
}

async fn raw_post(state: &Arc<crate::AppState>, endpoint_id: &str, path: &str, payload: &serde_json::Value) -> Result<serde_json::Value, String> {
    let (client, base) = raw_client(state, endpoint_id).await?;
    let url = build_url(&base, path);

    let body_bytes = serde_json::to_vec(payload).map_err(|e| format!("JSON serialize error: {}", e))?;

    let req = hyper::Request::builder()
        .method("POST")
        .uri(&url)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(body_bytes)))
        .map_err(|e| format!("Request build error: {}", e))?;

    let resp = match &client {
        #[cfg(unix)]
        RawClient::Unix(c) => c.request(req).await,
        RawClient::Tcp(c) => c.request(req).await,
    }
    .map_err(|e| format!("HTTP error: {}", e))?;

    let status = resp.status();
    let body_bytes = resp.into_body().collect().await
        .map_err(|e| format!("Body read error: {}", e))?.to_bytes();

    if !status.is_success() {
        let body_str = String::from_utf8_lossy(&body_bytes);
        return Err(format!("Docker API error {}: {}", status.as_u16(), body_str));
    }

    if body_bytes.is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_slice(&body_bytes).map_err(|e| format!("JSON parse error: {}", e))
}

async fn raw_delete(state: &Arc<crate::AppState>, endpoint_id: &str, path: &str) -> Result<(), String> {
    let (client, base) = raw_client(state, endpoint_id).await?;
    let url = build_url(&base, path);

    let req = hyper::Request::builder()
        .method("DELETE")
        .uri(&url)
        .body(Full::new(Bytes::new()))
        .map_err(|e| format!("Request build error: {}", e))?;

    let resp = match &client {
        #[cfg(unix)]
        RawClient::Unix(c) => c.request(req).await,
        RawClient::Tcp(c) => c.request(req).await,
    }
    .map_err(|e| format!("HTTP error: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        let body_bytes = resp.into_body().collect().await
            .map_err(|e| format!("Body read error: {}", e))?.to_bytes();
        let body_str = String::from_utf8_lossy(&body_bytes);
        return Err(format!("Docker API error {}: {}", status.as_u16(), body_str));
    }

    Ok(())
}

// ── Swarm Init ──────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct SwarmInitBody {
    #[serde(default)]
    pub advertise_addr: Option<String>,
    #[serde(default)]
    pub listen_addr: Option<String>,
    #[serde(default)]
    pub force_new_cluster: bool,
}

pub async fn swarm_init(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Json(body): Json<SwarmInitBody>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;

    let mut payload = serde_json::json!({
        "ForceNewCluster": body.force_new_cluster,
    });
    if let Some(addr) = &body.advertise_addr {
        payload["AdvertiseAddr"] = serde_json::json!(addr);
    }
    if let Some(addr) = &body.listen_addr {
        payload["ListenAddr"] = serde_json::json!(addr);
    }

    let result = raw_post(&state, &endpoint_id, "/swarm/init", &payload)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    state
        .audit_log
        .record("swarm.init", &endpoint_id, &endpoint_id, "Swarm initialized", "via-api")
        .await;

    Ok(Json(result))
}

// ── Swarm Join ──────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct SwarmJoinBody {
    pub remote_addrs: Vec<String>,
    pub join_token: String,
}

pub async fn swarm_join(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Json(body): Json<SwarmJoinBody>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;

    let payload = serde_json::json!({
        "RemoteAddrs": body.remote_addrs,
        "JoinToken": body.join_token,
    });

    raw_post(&state, &endpoint_id, "/swarm/join", &payload)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    state
        .audit_log
        .record("swarm.join", &endpoint_id, &endpoint_id, "Node joined swarm", "via-api")
        .await;

    Ok(Json(serde_json::json!({"status": "joined"})))
}

// ── Swarm Leave ─────────────────────────────────────────────────

pub async fn swarm_leave(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;

    let payload = serde_json::json!({"Force": true});

    raw_post(&state, &endpoint_id, "/swarm/leave", &payload)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    state
        .audit_log
        .record("swarm.leave", &endpoint_id, &endpoint_id, "Node left swarm", "via-api")
        .await;

    Ok(Json(serde_json::json!({"status": "left"})))
}

// ── Inspect Swarm ───────────────────────────────────────────────

pub async fn inspect_swarm(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;

    let swarm = raw_get(&state, &endpoint_id, "/swarm")
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    Ok(Json(swarm))
}

// ── List Nodes ──────────────────────────────────────────────────

pub async fn list_nodes(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<Vec<serde_json::Value>> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;

    let nodes = raw_get_array(&state, &endpoint_id, "/nodes")
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    Ok(Json(nodes))
}

// ── Inspect Node ────────────────────────────────────────────────

pub async fn inspect_node(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;

    let node = raw_get(&state, &endpoint_id, &format!("/nodes/{}", id))
        .await
        .map_err(|e| error(StatusCode::NOT_FOUND, &e))?;

    Ok(Json(node))
}

// ── Update Node ─────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct UpdateNodeBody {
    pub version: i64,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub availability: Option<String>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

pub async fn update_node(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
    Json(body): Json<UpdateNodeBody>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;

    let mut spec = serde_json::json!({
        "Version": { "Index": body.version },
    });

    if let Some(role) = &body.role {
        spec["Role"] = serde_json::json!(role);
    }
    if let Some(avail) = &body.availability {
        spec["Availability"] = serde_json::json!(avail);
    }
    if !body.labels.is_empty() {
        spec["Labels"] = serde_json::json!(body.labels);
    }

    raw_post(&state, &endpoint_id, &format!("/nodes/{}/update", id), &spec)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    state
        .audit_log
        .record("swarm.update_node", &endpoint_id, &id, "Node updated", "via-api")
        .await;

    Ok(Json(serde_json::json!({"status": "updated", "id": id})))
}

// ── Delete Node ─────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct DeleteNodeQuery {
    #[serde(default)]
    endpoint: Option<String>,
    #[serde(default)]
    force: bool,
}

pub async fn delete_node(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(query): Query<DeleteNodeQuery>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, query.endpoint.as_deref()).await;

    let path = if query.force {
        format!("/nodes/{}?force=true", id)
    } else {
        format!("/nodes/{}", id)
    };

    raw_delete(&state, &endpoint_id, &path)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    state
        .audit_log
        .record("swarm.delete_node", &endpoint_id, &id, "Node deleted", "via-api")
        .await;

    Ok(Json(serde_json::json!({"status": "deleted", "id": id})))
}

// ── List Services ───────────────────────────────────────────────

pub async fn list_services(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<Vec<serde_json::Value>> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;
    let docker = helpers::resolve_client(&state, Some(&endpoint_id)).await?;

    let services = docker
        .list_services(Some(ListServicesOptions::<String> {
            ..Default::default()
        }))
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let result: Vec<serde_json::Value> = services
        .into_iter()
        .map(|s| serde_json::json!(s))
        .collect();

    Ok(Json(result))
}

// ── Inspect Service ─────────────────────────────────────────────

pub async fn inspect_service(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;
    let docker = helpers::resolve_client(&state, Some(&endpoint_id)).await?;

    let service = docker
        .inspect_service(&id, None::<InspectServiceOptions>)
        .await
        .map_err(|e| error(StatusCode::NOT_FOUND, &e.to_string()))?;

    Ok(Json(serde_json::json!(service)))
}

// ── Create Service ──────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct CreateServiceBody {
    pub name: String,
    pub image: String,
    #[serde(default)]
    pub replicas: u64,
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub ports: Vec<ServicePortSpec>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(default)]
    pub constraints: Vec<String>,
}

#[derive(serde::Deserialize)]
pub struct ServicePortSpec {
    pub published: u16,
    pub target: u16,
    #[serde(default = "default_protocol")]
    pub protocol: String,
}

fn default_protocol() -> String {
    "tcp".to_string()
}

fn proto_enum(proto: &str) -> EndpointPortConfigProtocolEnum {
    match proto {
        "udp" | "UDP" => EndpointPortConfigProtocolEnum::UDP,
        "sctp" | "SCTP" => EndpointPortConfigProtocolEnum::SCTP,
        _ => EndpointPortConfigProtocolEnum::TCP,
    }
}

pub async fn create_service(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Json(body): Json<CreateServiceBody>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;
    let docker = helpers::resolve_client(&state, Some(&endpoint_id)).await?;

    let replicas = if body.replicas > 0 { body.replicas } else { 1 };

    let endpoint_ports: Vec<EndpointPortConfig> = body
        .ports
        .iter()
        .map(|p| EndpointPortConfig {
            target_port: Some(p.target as i64),
            published_port: Some(p.published as i64),
            protocol: Some(proto_enum(&p.protocol)),
            ..Default::default()
        })
        .collect();

    let endpoint_spec = if endpoint_ports.is_empty() {
        EndpointSpec::default()
    } else {
        EndpointSpec {
            ports: Some(endpoint_ports),
            ..Default::default()
        }
    };

    let service_spec = ServiceSpec {
        name: Some(body.name.clone()),
        labels: Some(body.labels),
        mode: Some(ServiceSpecMode {
            replicated: Some(ServiceSpecModeReplicated {
                replicas: Some(replicas as i64),
            }),
            ..Default::default()
        }),
        task_template: Some(TaskSpec {
            container_spec: Some(TaskSpecContainerSpec {
                image: Some(body.image.clone()),
                command: if body.command.is_empty() {
                    None
                } else {
                    Some(body.command)
                },
                env: if body.env.is_empty() {
                    None
                } else {
                    Some(body.env)
                },
                ..Default::default()
            }),
            placement: if body.constraints.is_empty() {
                None
            } else {
                Some(bollard::models::TaskSpecPlacement {
                    constraints: Some(body.constraints),
                    ..Default::default()
                })
            },
            ..Default::default()
        }),
        endpoint_spec: Some(endpoint_spec),
        ..Default::default()
    };

    let result = docker
        .create_service(service_spec, None)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    state
        .audit_log
        .record(
            "swarm.create_service",
            &endpoint_id,
            &body.name,
            "Service created",
            "via-api",
        )
        .await;

    Ok(Json(serde_json::json!({"status": "created", "id": result.id, "name": body.name})))
}

// ── Update Service ──────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct UpdateServiceBody {
    pub version: u64,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub replicas: Option<u64>,
    #[serde(default)]
    pub command: Option<Vec<String>>,
    #[serde(default)]
    pub env: Option<Vec<String>>,
    #[serde(default)]
    pub ports: Option<Vec<ServicePortSpec>>,
    #[serde(default)]
    pub labels: Option<HashMap<String, String>>,
    #[serde(default)]
    pub constraints: Option<Vec<String>>,
}

pub async fn update_service(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
    Json(body): Json<UpdateServiceBody>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;
    let docker = helpers::resolve_client(&state, Some(&endpoint_id)).await?;

    // Inspect existing service to build the update spec
    let existing = docker
        .inspect_service(&id, None::<InspectServiceOptions>)
        .await
        .map_err(|e| error(StatusCode::NOT_FOUND, &e.to_string()))?;

    let existing_spec = existing.spec.unwrap_or_default();

    let endpoint_ports: Option<Vec<EndpointPortConfig>> = body.ports.map(|ports| {
        ports
            .iter()
            .map(|p| EndpointPortConfig {
                target_port: Some(p.target as i64),
                published_port: Some(p.published as i64),
                protocol: Some(proto_enum(&p.protocol)),
                ..Default::default()
            })
            .collect()
    });

    let endpoint_spec = endpoint_ports.map(|ports| EndpointSpec {
        ports: Some(ports),
        ..Default::default()
    });

    let service_spec = ServiceSpec {
        name: existing_spec.name.clone(),
        labels: body.labels.or(existing_spec.labels),
        mode: body
            .replicas
            .map(|r| ServiceSpecMode {
                replicated: Some(ServiceSpecModeReplicated {
                    replicas: Some(r as i64),
                }),
                ..Default::default()
            })
            .or(existing_spec.mode),
        task_template: Some(TaskSpec {
            container_spec: Some(TaskSpecContainerSpec {
                image: body.image.or(existing_spec
                    .task_template
                    .as_ref()
                    .and_then(|t| t.container_spec.as_ref())
                    .and_then(|c| c.image.clone())),
                command: body.command.or(existing_spec
                    .task_template
                    .as_ref()
                    .and_then(|t| t.container_spec.as_ref())
                    .and_then(|c| c.command.clone())),
                env: body.env.or(existing_spec
                    .task_template
                    .as_ref()
                    .and_then(|t| t.container_spec.as_ref())
                    .and_then(|c| c.env.clone())),
                ..Default::default()
            }),
            placement: body
                .constraints
                .map(|c| bollard::models::TaskSpecPlacement {
                    constraints: Some(c),
                    ..Default::default()
                })
                .or(existing_spec
                    .task_template
                    .as_ref()
                    .and_then(|t| t.placement.clone())),
            ..Default::default()
        }),
        endpoint_spec: endpoint_spec.or(existing_spec.endpoint_spec),
        ..Default::default()
    };

    docker
        .update_service(
            &id,
            service_spec,
            UpdateServiceOptions {
                version: body.version,
                ..Default::default()
            },
            None,
        )
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    state
        .audit_log
        .record("swarm.update_service", &endpoint_id, &id, "Service updated", "via-api")
        .await;

    Ok(Json(serde_json::json!({"status": "updated", "id": id})))
}

// ── Delete Service ──────────────────────────────────────────────

pub async fn delete_service(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;
    let docker = helpers::resolve_client(&state, Some(&endpoint_id)).await?;

    docker
        .delete_service(&id)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    state
        .audit_log
        .record("swarm.delete_service", &endpoint_id, &id, "Service deleted", "via-api")
        .await;

    Ok(Json(serde_json::json!({"status": "deleted", "id": id})))
}

// ── Service Logs ────────────────────────────────────────────────

pub async fn service_logs(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;

    // Get tasks for this service via raw API
    let tasks_json = raw_get_array(
        &state,
        &endpoint_id,
        &format!("/tasks?filters={{\"service\":[\"{}\"]}}", id),
    )
    .await
    .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    let mut all_logs = String::new();

    for task in &tasks_json {
        let container_id = task["Status"]["ContainerStatus"]["ContainerID"]
            .as_str()
            .map(|s| s.to_string());

        if let Some(cid) = container_id {
            let docker = helpers::resolve_client(&state, Some(&endpoint_id)).await?;

            let mut stream = docker.logs(
                &cid,
                Some(LogsOptions::<String> {
                    stdout: true,
                    stderr: true,
                    tail: "100".to_string(),
                    ..Default::default()
                }),
            );

            while let Some(Ok(item)) = stream.next().await {
                match item {
                    LogOutput::StdOut { message } => {
                        all_logs.push_str(&String::from_utf8_lossy(&message));
                    }
                    LogOutput::StdErr { message } => {
                        all_logs.push_str(&String::from_utf8_lossy(&message));
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(Json(serde_json::json!({"service": id, "logs": all_logs})))
}

// ── Rollback Service ────────────────────────────────────────────

pub async fn rollback_service(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;
    let docker = helpers::resolve_client(&state, Some(&endpoint_id)).await?;

    // Get current version
    let service = docker
        .inspect_service(&id, None::<InspectServiceOptions>)
        .await
        .map_err(|e| error(StatusCode::NOT_FOUND, &e.to_string()))?;

    let version = service
        .version
        .and_then(|v| v.index)
        .unwrap_or(0);

    docker
        .update_service(
            &id,
            ServiceSpec::default(),
            UpdateServiceOptions {
                version,
                rollback: true,
                ..Default::default()
            },
            None,
        )
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    state
        .audit_log
        .record(
            "swarm.rollback_service",
            &endpoint_id,
            &id,
            "Service rolled back",
            "via-api",
        )
        .await;

    Ok(Json(serde_json::json!({"status": "rolled_back", "id": id})))
}

// ── List Tasks ──────────────────────────────────────────────────

pub async fn list_tasks(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<Vec<serde_json::Value>> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;

    let tasks = raw_get_array(&state, &endpoint_id, "/tasks")
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    Ok(Json(tasks))
}

// ── Inspect Task ────────────────────────────────────────────────

pub async fn inspect_task(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;

    let task = raw_get(&state, &endpoint_id, &format!("/tasks/{}", id))
        .await
        .map_err(|e| error(StatusCode::NOT_FOUND, &e))?;

    Ok(Json(task))
}

// ── Task Logs ───────────────────────────────────────────────────

pub async fn task_logs(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;

    // Inspect the task to find the container ID via raw API
    let task = raw_get(&state, &endpoint_id, &format!("/tasks/{}", id))
        .await
        .map_err(|e| error(StatusCode::NOT_FOUND, &e))?;

    let container_id = task["Status"]["ContainerStatus"]["ContainerID"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "No container ID found for this task"))?;

    // Use bollard for log streaming
    let docker = helpers::resolve_client(&state, Some(&endpoint_id)).await?;

    let mut stream = docker.logs(
        &container_id,
        Some(LogsOptions::<String> {
            stdout: true,
            stderr: true,
            tail: "100".to_string(),
            ..Default::default()
        }),
    );

    let mut log_bytes: Vec<u8> = Vec::new();
    while let Some(Ok(item)) = stream.next().await {
        match item {
            LogOutput::StdOut { message } => log_bytes.extend_from_slice(&message),
            LogOutput::StdErr { message } => log_bytes.extend_from_slice(&message),
            _ => {}
        }
    }
    let log_str = String::from_utf8_lossy(&log_bytes).to_string();

    Ok(Json(serde_json::json!({
        "task_id": id,
        "container_id": container_id,
        "logs": log_str,
    })))
}

// ── List Secrets ────────────────────────────────────────────────

pub async fn list_secrets(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<Vec<serde_json::Value>> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;
    let docker = helpers::resolve_client(&state, Some(&endpoint_id)).await?;

    let secrets = docker
        .list_secrets(Some(ListSecretsOptions::<String> {
            ..Default::default()
        }))
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let result: Vec<serde_json::Value> = secrets
        .into_iter()
        .map(|s| serde_json::json!(s))
        .collect();

    Ok(Json(result))
}

// ── Create Secret ───────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct CreateSecretBody {
    pub name: String,
    pub data: String,
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

pub async fn create_secret(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Json(body): Json<CreateSecretBody>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;
    let docker = helpers::resolve_client(&state, Some(&endpoint_id)).await?;

    use base64::Engine;

    let data_b64 = base64::engine::general_purpose::STANDARD.encode(body.data.as_bytes());

    let secret_spec = SecretSpec {
        name: Some(body.name.clone()),
        labels: if body.labels.is_empty() {
            None
        } else {
            Some(body.labels)
        },
        data: Some(data_b64),
        driver: None,
        templating: None,
    };

    let secret = docker
        .create_secret(secret_spec)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    state
        .audit_log
        .record("swarm.create_secret", &endpoint_id, &body.name, "Secret created", "via-api")
        .await;

    Ok(Json(serde_json::json!({"status": "created", "id": secret.id, "name": body.name})))
}

// ── Delete Secret ───────────────────────────────────────────────

pub async fn delete_secret(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;
    let docker = helpers::resolve_client(&state, Some(&endpoint_id)).await?;

    docker
        .delete_secret(&id)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    state
        .audit_log
        .record("swarm.delete_secret", &endpoint_id, &id, "Secret deleted", "via-api")
        .await;

    Ok(Json(serde_json::json!({"status": "deleted", "id": id})))
}

// ── List Configs ────────────────────────────────────────────────

pub async fn list_configs(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<Vec<serde_json::Value>> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;

    let configs = raw_get_array(&state, &endpoint_id, "/configs")
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    Ok(Json(configs))
}

// ── Create Config ───────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct CreateConfigBody {
    pub name: String,
    pub data: String,
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

pub async fn create_config(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Json(body): Json<CreateConfigBody>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;

    use base64::Engine;

    let data_b64 = base64::engine::general_purpose::STANDARD.encode(body.data.as_bytes());

    let payload = serde_json::json!({
        "Name": body.name,
        "Data": data_b64,
        "Labels": body.labels,
    });

    let result = raw_post(&state, &endpoint_id, "/configs/create", &payload)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    let config_id = result["ID"].as_str().unwrap_or("unknown");

    state
        .audit_log
        .record("swarm.create_config", &endpoint_id, &body.name, "Config created", "via-api")
        .await;

    Ok(Json(serde_json::json!({"status": "created", "id": config_id, "name": body.name})))
}

// ── Delete Config ───────────────────────────────────────────────

pub async fn delete_config(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let endpoint_id = helpers::resolve_endpoint_id(&state, params.endpoint.as_deref()).await;

    raw_delete(&state, &endpoint_id, &format!("/configs/{}", id))
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    state
        .audit_log
        .record("swarm.delete_config", &endpoint_id, &id, "Config deleted", "via-api")
        .await;

    Ok(Json(serde_json::json!({"status": "deleted", "id": id})))
}
