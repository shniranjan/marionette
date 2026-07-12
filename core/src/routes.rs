//! Route handlers — relay-first / bollard-fallback pattern.
//!
//! Every handler tries the relay path when an `endpoint` query parameter is
//! provided; on failure it falls back to the local bollard client. When no
//! endpoint is given the local Docker daemon is used directly.

use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use bollard::container::{
    ListContainersOptions, RestartContainerOptions, StartContainerOptions,
    StopContainerOptions,
};
use bollard::image::ListImagesOptions;
use bollard::network::ListNetworksOptions;
use bollard::volume::ListVolumesOptions;
use relay_protocol::Message;
use uuid::Uuid;

use crate::docker;

// ── Migration handlers (Wave 3) ──────────────────────────────────────

use crate::migration::{
    AnalyzeMigrationRequest, ExecuteMigrationRequest,
    PLANS, MIGRATION_STATES, MIGRATION_EVENTS,
    MigrationState,
};

// ── Helpers ──────────────────────────────────────────────────────────

/// Query parameter shared by all handlers.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointQuery {
    #[serde(default)]
    pub endpoint: Option<String>,
}

/// Default timeout for relay commands (seconds).
const RELAY_TIMEOUT: u64 = 30;

/// Convenience error response.
fn error(code: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({"error": msg})))
}

/// Build a relay request message with a random UUID id.
fn relay_request(subtype: &str, payload: serde_json::Value) -> Message {
    Message::new_request(Uuid::new_v4().to_string(), subtype, payload)
}

/// Serialize a bollard response to JSON, mapping the error.
fn to_json<T: serde::Serialize>(value: &T) -> Result<serde_json::Value, String> {
    serde_json::to_value(value).map_err(|e| format!("Serialization error: {}", e))
}

/// Add flattened `name` field to container JSON (frontend expects `name`, Docker gives `Names[]`)
fn add_container_names(containers: &mut serde_json::Value) {
    if let Some(arr) = containers.as_array_mut() {
        for c in arr {
            if let Some(obj) = c.as_object_mut() {
                if let Some(names) = obj.get("Names").and_then(|n| n.as_array()) {
                    if let Some(first) = names.first().and_then(|n| n.as_str()) {
                        obj.insert("name".into(), serde_json::Value::String(first.trim_start_matches('/').into()));
                    }
                }
            }
        }
    }
}

// ── Containers ───────────────────────────────────────────────────────

/// GET /api/containers(?endpoint=hostname)
///
/// Lists all containers on the target Docker host. When `endpoint` is set
/// the request is forwarded through the relay; on failure it falls back to
/// the local Docker daemon.
pub async fn list_containers(
    Query(params): Query<EndpointQuery>,
) -> impl IntoResponse {
    // ── Relay path ──
    if let Some(ref host) = params.endpoint {
        if host != "local" {
            let msg = relay_request("docker.ps", serde_json::json!({"all": true}));
            match crate::ws_relay::send_relay_command(host, msg, RELAY_TIMEOUT).await {
                Ok(mut resp) => {
                    add_container_names(&mut resp.payload);
                    return Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(resp.payload));
                }
                Err(e) => tracing::warn!(%host, error = %e, "relay failed, falling back to local bollard"),
            }
        }
    }

    // ── Bollard fallback ──
    let docker = docker::local_client().map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;
    let containers = docker
        .list_containers::<String>(Some(ListContainersOptions {
            all: true,
            ..Default::default()
        }))
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let mut json = to_json(&containers)
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    add_container_names(&mut json);
    Ok(Json(json))
}

/// GET /api/containers/:id(?endpoint=hostname)
///
/// Inspects a single container by id or name.
pub async fn inspect_container(
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> impl IntoResponse {
    if let Some(ref host) = params.endpoint {
        if host != "local" {
            let msg = relay_request("docker.inspect", serde_json::json!({"id": id}));
            match crate::ws_relay::send_relay_command(host, msg, RELAY_TIMEOUT).await {
                Ok(resp) => return Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(resp.payload)),
                Err(e) => tracing::warn!(%host, error = %e, "relay failed, falling back to local bollard"),
            }
        }
    }

    let docker = docker::local_client().map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;
    let info = docker
        .inspect_container(&id, None)
        .await
        .map_err(|e| error(StatusCode::NOT_FOUND, &e.to_string()))?;

    let json = to_json(&info).map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;
    Ok(Json(json))
}

/// POST /api/containers/:id/start(?endpoint=hostname)
pub async fn start_container(
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> impl IntoResponse {
    if let Some(ref host) = params.endpoint {
        if host != "local" {
            let msg = relay_request("docker.start", serde_json::json!({"container": id}));
            match crate::ws_relay::send_relay_command(host, msg, RELAY_TIMEOUT).await {
                Ok(_) => {
                    return Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(
                        serde_json::json!({"status": "started", "id": id}),
                    ));
                }
                Err(e) => tracing::warn!(%host, error = %e, "relay failed, falling back to local bollard"),
            }
        }
    }

    let docker = docker::local_client().map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;
    docker
        .start_container(&id, None::<StartContainerOptions<String>>)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({"status": "started", "id": id})))
}

/// POST /api/containers/:id/stop(?endpoint=hostname)
pub async fn stop_container(
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> impl IntoResponse {
    if let Some(ref host) = params.endpoint {
        if host != "local" {
            let msg = relay_request("docker.stop", serde_json::json!({"container": id}));
            match crate::ws_relay::send_relay_command(host, msg, RELAY_TIMEOUT).await {
                Ok(_) => {
                    return Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(
                        serde_json::json!({"status": "stopped", "id": id}),
                    ));
                }
                Err(e) => tracing::warn!(%host, error = %e, "relay failed, falling back to local bollard"),
            }
        }
    }

    let docker = docker::local_client().map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;
    docker
        .stop_container(&id, None::<StopContainerOptions>)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({"status": "stopped", "id": id})))
}

/// POST /api/containers/:id/restart(?endpoint=hostname)
pub async fn restart_container(
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> impl IntoResponse {
    if let Some(ref host) = params.endpoint {
        if host != "local" {
            let msg = relay_request("docker.restart", serde_json::json!({"container": id}));
            match crate::ws_relay::send_relay_command(host, msg, RELAY_TIMEOUT).await {
                Ok(_) => {
                    return Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(
                        serde_json::json!({"status": "restarted", "id": id}),
                    ));
                }
                Err(e) => tracing::warn!(%host, error = %e, "relay failed, falling back to local bollard"),
            }
        }
    }

    let docker = docker::local_client().map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;
    docker
        .restart_container(&id, None::<RestartContainerOptions>)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({"status": "restarted", "id": id})))
}

// ── Images ───────────────────────────────────────────────────────────

/// GET /api/images(?endpoint=hostname)
pub async fn list_images(
    Query(params): Query<EndpointQuery>,
) -> impl IntoResponse {
    if let Some(ref host) = params.endpoint {
        if host != "local" {
            let msg = relay_request("docker.images", serde_json::json!({"all": true}));
            match crate::ws_relay::send_relay_command(host, msg, RELAY_TIMEOUT).await {
                Ok(resp) => return Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(resp.payload)),
                Err(e) => tracing::warn!(%host, error = %e, "relay failed, falling back to local bollard"),
            }
        }
    }

    let docker = docker::local_client().map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;
    let images = docker
        .list_images(Some(ListImagesOptions::<String> {
            all: true,
            ..Default::default()
        }))
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let json = to_json(&images).map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;
    Ok(Json(json))
}

// ── Volumes ──────────────────────────────────────────────────────────

/// GET /api/volumes(?endpoint=hostname)
pub async fn list_volumes(
    Query(params): Query<EndpointQuery>,
) -> impl IntoResponse {
    if let Some(ref host) = params.endpoint {
        if host != "local" {
            let msg = relay_request("docker.volumes", serde_json::json!({}));
            match crate::ws_relay::send_relay_command(host, msg, RELAY_TIMEOUT).await {
                Ok(resp) => return Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(resp.payload)),
                Err(e) => tracing::warn!(%host, error = %e, "relay failed, falling back to local bollard"),
            }
        }
    }

    let docker = docker::local_client().map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;
    let resp = docker
        .list_volumes(Some(ListVolumesOptions::<String> {
            ..Default::default()
        }))
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let json = to_json(&resp).map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;
    Ok(Json(json))
}

// ── Networks ─────────────────────────────────────────────────────────

/// GET /api/networks(?endpoint=hostname)
pub async fn list_networks(
    Query(params): Query<EndpointQuery>,
) -> impl IntoResponse {
    if let Some(ref host) = params.endpoint {
        if host != "local" {
            let msg = relay_request("docker.networks", serde_json::json!({}));
            match crate::ws_relay::send_relay_command(host, msg, RELAY_TIMEOUT).await {
                Ok(resp) => return Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(resp.payload)),
                Err(e) => tracing::warn!(%host, error = %e, "relay failed, falling back to local bollard"),
            }
        }
    }

    let docker = docker::local_client().map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;
    let networks = docker
        .list_networks::<String>(Some(ListNetworksOptions {
            ..Default::default()
        }))
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let json = to_json(&networks).map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;
    Ok(Json(json))
}

// ── Migration handlers ───────────────────────────────────────────────

/// POST /api/migration/analyze
///
/// Inspects source stack, checks target compatibility, discovers volumes
/// and databases, and returns a plan id for downstream phases.
pub async fn analyze_migration_handler(
    Json(body): Json<AnalyzeMigrationRequest>,
) -> impl IntoResponse {
    let plan_id = Uuid::new_v4().to_string();

    let result = crate::migration::analyze_migration(
        &body.source_host,
        &body.target_host,
        &body.stack_name,
    )
    .await;

    match result {
        Ok(plan) => {
            // Store plan for downstream handlers to look up.
            PLANS.lock().await.insert(plan_id.clone(), plan.clone());
            // Set initial state.
            MIGRATION_STATES
                .lock()
                .await
                .insert(plan_id.clone(), MigrationState::Analyzed);

            Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(
                serde_json::json!({
                    "plan_id": plan_id,
                    "status": "analyzed",
                    "plan": plan,
                }),
            ))
        }
        Err(e) => Err(error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Analyze failed: {}", e),
        )),
    }
}

/// GET /api/migration/plan/:id
///
/// Returns the stored analyze plan for a given plan id.
pub async fn get_migration_plan(
    Path(id): Path<String>,
) -> impl IntoResponse {
    let plans = PLANS.lock().await;
    match plans.get(&id) {
        Some(plan) => Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(
            serde_json::json!({
                "plan_id": id,
                "plan": plan,
            }),
        )),
        None => Err(error(
            StatusCode::NOT_FOUND,
            &format!("Plan not found: {}", id),
        )),
    }
}

/// POST /api/migration/prepare
///
/// Provisions the target host: directories, compose files, images.
pub async fn prepare_migration_handler(
    Json(body): Json<ExecuteMigrationRequest>,
) -> impl IntoResponse {
    let plan = {
        let plans = PLANS.lock().await;
        match plans.get(&body.plan_id) {
            Some(p) => p.clone(),
            None => {
                return Err(error(
                    StatusCode::NOT_FOUND,
                    &format!("Plan not found: {}", body.plan_id),
                ));
            }
        }
    };

    match crate::migration::prepare_migration(
        &plan.target.hostname,
        &plan,
    )
    .await
    {
        Ok(result) => {
            MIGRATION_STATES
                .lock()
                .await
                .insert(body.plan_id.clone(), MigrationState::Prepared);

            Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(
                serde_json::json!({
                    "plan_id": body.plan_id,
                    "status": "prepared",
                    "result": result,
                }),
            ))
        }
        Err(e) => Err(error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Prepare failed: {}", e),
        )),
    }
}

/// POST /api/migration/transfer
///
/// Transfers volumes from source to target host.
pub async fn transfer_migration_handler(
    Json(body): Json<ExecuteMigrationRequest>,
) -> impl IntoResponse {
    let plan = {
        let plans = PLANS.lock().await;
        match plans.get(&body.plan_id) {
            Some(p) => p.clone(),
            None => {
                return Err(error(
                    StatusCode::NOT_FOUND,
                    &format!("Plan not found: {}", body.plan_id),
                ));
            }
        }
    };

    match crate::migration::execute_transfer(
        &plan.source.hostname,
        &plan.target.hostname,
        &plan,
        None,
    )
    .await
    {
        Ok(result) => {
            MIGRATION_STATES
                .lock()
                .await
                .insert(body.plan_id.clone(), MigrationState::Transferred);

            Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(
                serde_json::json!({
                    "plan_id": body.plan_id,
                    "status": "transferred",
                    "result": result,
                }),
            ))
        }
        Err(e) => Err(error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Transfer failed: {}", e),
        )),
    }
}

/// POST /api/migration/switchover
///
/// Stops source, deploys on target, polls health checks.
/// On failure the caller should use POST /api/migration/rollback.
pub async fn switchover_migration_handler(
    Json(body): Json<ExecuteMigrationRequest>,
) -> impl IntoResponse {
    let plan = {
        let plans = PLANS.lock().await;
        match plans.get(&body.plan_id) {
            Some(p) => p.clone(),
            None => {
                return Err(error(
                    StatusCode::NOT_FOUND,
                    &format!("Plan not found: {}", body.plan_id),
                ));
            }
        }
    };

    crate::migration::MIGRATION_STATES
        .lock()
        .await
        .insert(body.plan_id.clone(), MigrationState::Switching);

    match crate::migration::execute_switchover(
        &plan.source.hostname,
        &plan.target.hostname,
        &plan,
    )
    .await
    {
        Ok(result) => {
            MIGRATION_STATES
                .lock()
                .await
                .insert(body.plan_id.clone(), MigrationState::Complete);

            Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(
                serde_json::json!({
                    "plan_id": body.plan_id,
                    "status": "complete",
                    "result": result,
                }),
            ))
        }
        Err(e) => {
            MIGRATION_STATES
                .lock()
                .await
                .insert(body.plan_id.clone(), MigrationState::Failed);

            Err(error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Switchover failed: {}", e),
            ))
        }
    }
}

/// POST /api/migration/rollback
///
/// Stops target, restarts source, verifies source health.
pub async fn rollback_migration_handler(
    Json(body): Json<ExecuteMigrationRequest>,
) -> impl IntoResponse {
    let plan = {
        let plans = PLANS.lock().await;
        match plans.get(&body.plan_id) {
            Some(p) => p.clone(),
            None => {
                return Err(error(
                    StatusCode::NOT_FOUND,
                    &format!("Plan not found: {}", body.plan_id),
                ));
            }
        }
    };

    crate::migration::MIGRATION_STATES
        .lock()
        .await
        .insert(body.plan_id.clone(), MigrationState::RollingBack);

    match crate::migration::rollback_migration(
        &plan.source.hostname,
        &plan.target.hostname,
        &plan,
    )
    .await
    {
        Ok(result) => {
            MIGRATION_STATES
                .lock()
                .await
                .insert(body.plan_id.clone(), MigrationState::RolledBack);

            Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(
                serde_json::json!({
                    "plan_id": body.plan_id,
                    "status": "rolled_back",
                    "result": result,
                }),
            ))
        }
        Err(e) => {
            MIGRATION_STATES
                .lock()
                .await
                .insert(body.plan_id.clone(), MigrationState::Failed);

            Err(error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Rollback failed: {}", e),
            ))
        }
    }
}

/// GET /api/migration/status/:id
///
/// Returns the current state of a migration plan.
pub async fn migration_status_handler(
    Path(id): Path<String>,
) -> impl IntoResponse {
    let states = MIGRATION_STATES.lock().await;
    match states.get(&id) {
        Some(state) => Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(
            serde_json::json!({
                "plan_id": id,
                "state": state,
            }),
        )),
        None => Err(error(
            StatusCode::NOT_FOUND,
            &format!("Migration not found: {}", id),
        )),
    }
}

/// GET /api/migration/events/:id
///
/// Returns the event log for a migration plan.
pub async fn migration_events_handler(
    Path(id): Path<String>,
) -> impl IntoResponse {
    let events = MIGRATION_EVENTS.lock().await;
    match events.get(&id) {
        Some(evts) => Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(
            serde_json::json!({
                "plan_id": id,
                "events": evts,
            }),
        )),
        None => Ok::<_, (StatusCode, Json<serde_json::Value>)>(Json(
            serde_json::json!({
                "plan_id": id,
                "events": [],
            }),
        )),
    }
}
