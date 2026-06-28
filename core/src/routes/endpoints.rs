use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use std::time::Instant;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

use crate::docker::*;
use crate::models::*;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn error(code: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({"error": msg})))
}

// ── List Endpoints ────────────────────────────────────────────

pub async fn list_endpoints(
    State(state): State<Arc<crate::AppState>>,
) -> ApiResult<Vec<DockerEndpoint>> {
    let endpoints = state.endpoints.read().await;
    let list: Vec<DockerEndpoint> = endpoints.values().cloned().collect();
    Ok(Json(list))
}

// ── Create Endpoint ───────────────────────────────────────────

#[derive(serde::Deserialize)]
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
    // Check for duplicate name
    let endpoints = state.endpoints.read().await;
    if endpoints.values().any(|e| e.name == body.name) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            &format!("Endpoint '{}' already exists", body.name),
        ));
    }
    drop(endpoints);

    // Create client
    let docker = create_client(&body.connection, body.cert_path.as_deref())
        .map_err(|e| error(StatusCode::BAD_REQUEST, &e))?;

    // Test connectivity (5s timeout)
    match timeout(Duration::from_secs(5), docker.ping()).await {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => {
            return Err(error(
                StatusCode::BAD_REQUEST,
                &format!("Connection test failed: {}", e),
            ))
        }
        Err(_) => {
            return Err(error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Connection timed out (5s)",
            ))
        }
    }

    let id = Uuid::new_v4().to_string();
    let endpoint = DockerEndpoint {
        id: id.clone(),
        name: body.name.clone(),
        connection: body.connection.clone(),
        status: EndpointStatus::Connected,
        tags: body.tags,
        cert_path: body.cert_path.clone(),
    };

    // Insert into both maps
    {
        let mut endpoints = state.endpoints.write().await;
        let mut clients = state.clients.write().await;
        clients.insert(id.clone(), docker);
        endpoints.insert(id.clone(), endpoint.clone());
    }

    // Persist to database
    state.db.upsert_endpoint(&endpoint);

    // Audit
    state
        .audit_log
        .record(
            "endpoint.create",
            &id,
            &body.name,
            &format!("connection={}", &body.connection),
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
    let endpoints = state.endpoints.read().await;
    let endpoint = endpoints.get(&id).ok_or_else(|| {
        error(StatusCode::NOT_FOUND, &format!("Endpoint '{}' not found", id))
    })?;

    // Test live connectivity
    let clients = state.clients.read().await;
    let connection_status = match clients.get(&id) {
        Some(docker) => match timeout(Duration::from_secs(3), docker.ping()).await {
            Ok(Ok(_)) => "connected",
            Ok(Err(_)) => "disconnected",
            Err(_) => "timeout",
        },
        None => "no_client",
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
    let mut detail_parts: Vec<String> = Vec::new();

    // Determine effective cert_path: explicit update or fall back to existing
    let effective_cert_path = if body.cert_path.is_some() {
        body.cert_path.clone().flatten()
    } else {
        // Use existing endpoint's cert_path (read under read lock)
        let endpoints = state.endpoints.read().await;
        endpoints.get(&id).and_then(|ep| ep.cert_path.clone())
    };

    // If connection or cert_path changed, validate and recreate client
    let needs_reconnect = body.connection.is_some() || body.cert_path.is_some();
    if needs_reconnect {
        let connection_str = if let Some(ref conn) = body.connection {
            conn.clone()
        } else {
            let endpoints = state.endpoints.read().await;
            endpoints.get(&id)
                .map(|ep| ep.connection.clone())
                .unwrap_or_default()
        };
        let docker = create_client(&connection_str, effective_cert_path.as_deref())
            .map_err(|e| error(StatusCode::BAD_REQUEST, &e))?;
        // Quick connectivity test
        match timeout(Duration::from_secs(5), docker.ping()).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                return Err(error(
                    StatusCode::BAD_REQUEST,
                    &format!("Connection test failed: {}", e),
                ))
            }
            Err(_) => {
                return Err(error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Connection timed out (5s)",
                ))
            }
        }
        let mut clients = state.clients.write().await;
        clients.insert(id.clone(), docker);
        detail_parts.push(format!("connection={}", connection_str));
    }

    // Update endpoint fields
    {
        let mut endpoints = state.endpoints.write().await;
        let endpoint = endpoints.get_mut(&id).ok_or_else(|| {
            error(StatusCode::NOT_FOUND, &format!("Endpoint '{}' not found", id))
        })?;

        if let Some(name) = body.name {
            detail_parts.push(format!("name={}", name));
            endpoint.name = name;
        }
        if let Some(connection) = body.connection {
            endpoint.connection = connection;
            endpoint.status = EndpointStatus::Connected;
        }
        if let Some(tags) = body.tags {
            detail_parts.push("tags=updated".to_string());
            endpoint.tags = tags;
        }
        if body.cert_path.is_some() {
            endpoint.cert_path = body.cert_path.clone().flatten();
        }
    }

    // Persist updated endpoint to database
    {
        let endpoints = state.endpoints.read().await;
        if let Some(ep) = endpoints.get(&id) {
            state.db.upsert_endpoint(ep);
        }
    }

    // Audit
    let detail = detail_parts.join("; ");
    let target = {
        let endpoints = state.endpoints.read().await;
        endpoints
            .get(&id)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| id.clone())
    };

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
    // Refuse to delete the default "local" endpoint
    {
        let endpoints = state.endpoints.read().await;
        if let Some(ep) = endpoints.get(&id) {
            if ep.id == state.default_endpoint || ep.name == "local" {
                return Err(error(
                    StatusCode::FORBIDDEN,
                    "Cannot delete the default 'local' endpoint",
                ));
            }
        }
    }

    let removed_name;
    {
        let mut endpoints = state.endpoints.write().await;
        let mut clients = state.clients.write().await;

        let removed = endpoints
            .remove(&id)
            .ok_or_else(|| error(StatusCode::NOT_FOUND, &format!("Endpoint '{}' not found", id)))?;
        removed_name = removed.name;
        clients.remove(&id);
    }

    // Delete from database
    state.db.delete_endpoint(&id);

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
    let clients = state.clients.read().await;
    let docker = clients.get(&id).ok_or_else(|| {
        error(StatusCode::NOT_FOUND, &format!("Endpoint '{}' not found", id))
    })?;

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
    let (connection, cert_path) = {
        let endpoints = state.endpoints.read().await;
        let ep = endpoints.get(&id).ok_or_else(|| {
            error(StatusCode::NOT_FOUND, &format!("Endpoint '{}' not found", id))
        })?;
        (ep.connection.clone(), ep.cert_path.clone())
    };

    // Recreate client
    let docker = create_client(&connection, cert_path.as_deref())
        .map_err(|e| error(StatusCode::BAD_REQUEST, &e))?;

    // Test connectivity
    match timeout(Duration::from_secs(5), docker.ping()).await {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => {
            return Err(error(
                StatusCode::BAD_REQUEST,
                &format!("Reconnect failed: {}", e),
            ))
        }
        Err(_) => {
            return Err(error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Reconnect timed out (5s)",
            ))
        }
    }

    let start = Instant::now();
    // Update client and status
    {
        let mut clients = state.clients.write().await;
        let mut endpoints = state.endpoints.write().await;
        clients.insert(id.clone(), docker);
        if let Some(ep) = endpoints.get_mut(&id) {
            ep.status = EndpointStatus::Connected;
        }
    }

    // Persist status to database
    {
        let endpoints = state.endpoints.read().await;
        if let Some(ep) = endpoints.get(&id) {
            state.db.upsert_endpoint(ep);
        }
    }
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
