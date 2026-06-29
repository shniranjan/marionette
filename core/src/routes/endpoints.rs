use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use bollard::container::ListContainersOptions;
use std::sync::Arc;
use std::time::Instant;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

use crate::helpers;
use crate::models::*;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn error(code: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({"error": msg})))
}

// ── List Endpoints ────────────────────────────────────────────

pub async fn list_endpoints(
    State(state): State<Arc<crate::AppState>>,
) -> ApiResult<Vec<DockerEndpoint>> {
    let endpoints = state.registry.list().await;
    Ok(Json(endpoints))
}

// ── Create Endpoint ───────────────────────────────────────────

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointCreateBody {
    pub name: String,
    pub connection: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub cert_path: Option<String>,
}

pub async fn create_endpoint(
    State(state): State<Arc<crate::AppState>>,
    Json(body): Json<EndpointCreateBody>,
) -> ApiResult<DockerEndpoint> {
    let id = Uuid::new_v4().to_string();
    let endpoint = DockerEndpoint {
        id,
        name: body.name.clone(),
        connection: body.connection.clone(),
        status: EndpointStatus::Connected,
        tags: body.tags,
        cert_path: body.cert_path.clone(),
    };

    // Registry handles duplicate name check, connection test, persist, and client cache
    let endpoint = state
        .registry
        .create(endpoint)
        .await
        .map_err(|e| error(StatusCode::BAD_REQUEST, &e))?;

    // Audit
    state
        .audit_log
        .record(
            "endpoint.create",
            &endpoint.id,
            &endpoint.name,
            &format!("connection={}", &endpoint.connection),
            "gateway",
        )
        .await;

    Ok(Json(endpoint))
}

// ── Get Endpoint Detail ───────────────────────────────────────

pub async fn get_endpoint(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let endpoint = state.registry.get(&id).await.ok_or_else(|| {
        error(
            StatusCode::NOT_FOUND,
            &format!("Endpoint '{}' not found", id),
        )
    })?;

    // Test live connectivity
    let connection_status = match state.registry.get_client(&id).await {
        Ok(docker) => match timeout(Duration::from_secs(3), docker.ping()).await {
            Ok(Ok(_)) => "connected",
            Ok(Err(_)) => "disconnected",
            Err(_) => "timeout",
        },
        Err(_) => "no_client",
    };

    Ok(Json(serde_json::json!({
        "id": endpoint.id,
        "name": endpoint.name,
        "connection": endpoint.connection,
        "connection_status": connection_status,
        "tags": endpoint.tags
    })))
}

// ── Update Endpoint ───────────────────────────────────────────

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointUpdateBody {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub connection: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub cert_path: Option<Option<String>>,
}

pub async fn update_endpoint(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Json(body): Json<EndpointUpdateBody>,
) -> ApiResult<serde_json::Value> {
    // Build audit detail before handing off to registry
    let mut detail_parts: Vec<String> = Vec::new();
    if let Some(ref name) = body.name {
        detail_parts.push(format!("name={}", name));
    }
    if let Some(ref conn) = body.connection {
        detail_parts.push(format!("connection={}", conn));
    }
    if body.tags.is_some() {
        detail_parts.push("tags=updated".to_string());
    }

    // Registry handles: existence check, field updates, reconnect if needed,
    // connection test, persistence, and client cache update
    state
        .registry
        .update(&id, body.name, body.connection, body.tags, body.cert_path)
        .await
        .map_err(|e| {
            if e.contains("not found") {
                error(StatusCode::NOT_FOUND, &e)
            } else {
                error(StatusCode::BAD_REQUEST, &e)
            }
        })?;

    // Audit
    let target = state
        .registry
        .get(&id)
        .await
        .map(|e| e.name)
        .unwrap_or_else(|| id.clone());
    let detail = detail_parts.join("; ");

    state
        .audit_log
        .record("endpoint.update", &id, &target, &detail, "gateway")
        .await;

    Ok(Json(serde_json::json!({"status": "updated", "id": id})))
}

// ── Delete Endpoint ───────────────────────────────────────────

pub async fn delete_endpoint(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    // Registry handles: existence check, "local" protection, DB removal, client cache removal
    let removed_name = state.registry.delete(&id).await.map_err(|e| {
        if e.contains("not found") {
            error(StatusCode::NOT_FOUND, &e)
        } else {
            error(StatusCode::FORBIDDEN, &e)
        }
    })?;

    // Audit
    state
        .audit_log
        .record("endpoint.delete", &id, &removed_name, "removed", "gateway")
        .await;

    Ok(Json(serde_json::json!({"status": "deleted", "id": id, "name": removed_name})))
}

// ── Test Endpoint Connectivity ────────────────────────────────

pub async fn test_endpoint(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    // Verify endpoint exists first
    if state.registry.get(&id).await.is_none() {
        return Err(error(
            StatusCode::NOT_FOUND,
            &format!("Endpoint '{}' not found", id),
        ));
    }

    // Try to get a live client (registry handles lazy-connect and health checks)
    let docker = match state.registry.get_client(&id).await {
        Ok(client) => client,
        Err(e) => {
            return Ok(Json(serde_json::json!({
                "status": "error",
                "error": e
            })));
        }
    };

    let start = Instant::now();
    match timeout(Duration::from_secs(5), docker.ping()).await {
        Ok(Ok(_)) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            Ok(Json(serde_json::json!({
                "status": "connected",
                "latency_ms": latency_ms
            })))
        }
        Ok(Err(e)) => Ok(Json(serde_json::json!({
            "status": "disconnected",
            "error": e.to_string()
        }))),
        Err(_) => Ok(Json(serde_json::json!({
            "status": "error",
            "error": "Connection timed out (5s)"
        }))),
    }
}

// ── Reconnect Endpoint ────────────────────────────────────────

pub async fn reconnect_endpoint(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let start = Instant::now();

    // Registry handles: existence check, client rebuild, connection test, cache update
    state.registry.reconnect(&id).await.map_err(|e| {
        if e.contains("not found") {
            error(StatusCode::NOT_FOUND, &e)
        } else {
            error(StatusCode::BAD_REQUEST, &e)
        }
    })?;

    let latency_ms = start.elapsed().as_millis() as u64;

    // Audit
    state
        .audit_log
        .record("endpoint.reconnect", &id, &id, "reconnected", "gateway")
        .await;

    Ok(Json(serde_json::json!({
        "status": "reconnected",
        "id": id,
        "latency_ms": latency_ms
    })))
}

// ── Endpoint Info (live system details for migration target) ──

pub async fn endpoint_info(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
) -> ApiResult<EndpointInfo> {
    // Fetch endpoint from registry
    let endpoint = state.registry.get(&id).await.ok_or_else(|| {
        error(
            StatusCode::NOT_FOUND,
            &format!("Endpoint '{}' not found", id),
        )
    })?;

    // Resolve Docker client
    let docker = helpers::resolve_client(&state, Some(&id)).await?;

    // Get Docker version
    let docker_version = match timeout(Duration::from_secs(5), docker.version()).await {
        Ok(Ok(ver)) => ver.version.unwrap_or_else(|| "unknown".to_string()),
        _ => "unknown".to_string(),
    };

    // Get Docker system info for disk usage
    let (disk_free_bytes, disk_total_bytes) = match timeout(Duration::from_secs(5), docker.info()).await {
        Ok(Ok(info)) => {
            let mut free: u64 = 0;
            let mut total: u64 = 0;
            // Parse driver_status for Data Space Available / Data Space Total
            // Docker returns [[key,val],[key,val]] — each inner vec is [key, val]
            if let Some(status) = info.driver_status {
                for entry in &status {
                    if entry.len() >= 2 {
                        let key = &entry[0];
                        let val = &entry[1];
                        let bytes = parse_docker_size(val);
                        if key.contains("Data Space Available") || key.contains("Data Space Free") {
                            free = bytes;
                        } else if key.contains("Data Space Total") {
                            total = bytes;
                        }
                    }
                }
            }
            (free, total)
        }
        _ => (0, 0),
    };

    // Count containers
    let container_count = match timeout(Duration::from_secs(5), docker.list_containers::<String>(Some(ListContainersOptions {
        all: true,
        ..Default::default()
    }))).await {
        Ok(Ok(containers)) => containers.len() as i64,
        _ => 0,
    };

    Ok(Json(EndpointInfo {
        id: endpoint.id.clone(),
        name: endpoint.name.clone(),
        connection: endpoint.connection.clone(),
        status: serde_json::to_value(&endpoint.status)
            .unwrap_or_default()
            .as_str()
            .unwrap_or("disconnected")
            .to_string(),
        tags: endpoint.tags.clone(),
        cert_path: endpoint.cert_path.clone(),
        docker_version,
        container_count,
        disk_free_bytes,
        disk_total_bytes,
    }))
}

/// Parse a Docker human-readable size string like "10.5 GB" or "500 MB" into bytes.
fn parse_docker_size(s: &str) -> u64 {
    let s = s.trim();
    if let Some((num_str, unit)) = s.split_once(' ') {
        let num: f64 = num_str.parse().unwrap_or(0.0);
        let multiplier = match unit.to_uppercase().as_str() {
            "B" => 1.0,
            "KB" => 1_000.0,
            "MB" => 1_000_000.0,
            "GB" => 1_000_000_000.0,
            "TB" => 1_000_000_000_000.0,
            "KIB" | "K" => 1_024.0,
            "MIB" | "M" => 1_048_576.0,
            "GIB" | "G" => 1_073_741_824.0,
            "TIB" | "T" => 1_099_511_627_776.0,
            _ => 1.0,
        };
        (num * multiplier) as u64
    } else {
        s.parse::<u64>().unwrap_or(0)
    }
}
