use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::sse::{Event, Sse},
    Json,
};
use bollard::container::ListContainersOptions;
use bollard::container::PruneContainersOptions;
use bollard::image::PruneImagesOptions;
use bollard::volume::{ListVolumesOptions, PruneVolumesOptions};
use bollard::network::{ListNetworksOptions, PruneNetworksOptions};
use bollard::system::EventsOptions;
use futures::stream::{self, Stream, StreamExt};
use std::convert::Infallible;
use std::sync::Arc;

use crate::helpers;
use crate::models::*;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn error(code: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({"error": msg})))
}

// ── System Info ───────────────────────────────────────────────

pub async fn system_info(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<SystemInfo> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    let info = docker
        .info()
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    // List containers for running/paused/stopped counts
    let containers = docker
        .list_containers::<String>(Some(ListContainersOptions {
            all: true,
            ..Default::default()
        }))
        .await
        .unwrap_or_default();

    let total = containers.len() as i64;
    let running = containers.iter().filter(|c| c.state.as_deref() == Some("running")).count() as i64;
    let paused = containers.iter().filter(|c| c.state.as_deref() == Some("paused")).count() as i64;
    let stopped = total - running - paused;

    // Bollard 0.17: SystemInfo does NOT have .volumes or .networks fields.
    // Get counts from list_volumes and list_networks separately.
    let volumes_count = docker
        .list_volumes(Some(ListVolumesOptions::<String> {
            ..Default::default()
        }))
        .await
        .map(|r| r.volumes.unwrap_or_default().len() as i64)
        .unwrap_or(0);

    let networks_count = docker
        .list_networks::<String>(Some(ListNetworksOptions {
            ..Default::default()
        }))
        .await
        .map(|r| r.len() as i64)
        .unwrap_or(0);

    let system = SystemInfo {
        containers: total,
        containers_running: running,
        containers_paused: paused,
        containers_stopped: stopped,
        images: info.images.unwrap_or(0),
        volumes: volumes_count,
        networks: networks_count,
        driver: info.driver,
        kernel_version: info.kernel_version,
        os: info.operating_system,
        architecture: info.architecture,
        cpu_count: info.ncpu.map(|n| n as u64),
        memory_bytes: info.mem_total,
        docker_version: info.server_version,
        server_time: chrono::Utc::now().to_rfc3339(),
    };

    Ok(Json(system))
}

// ── Docker Version ────────────────────────────────────────────

pub async fn docker_version(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    let version = docker
        .version()
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!(version)))
}

// ── Prune Resources ───────────────────────────────────────────

pub async fn prune_resources(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Json(body): Json<PruneRequest>,
) -> ApiResult<serde_json::Value> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    let result = match body.resource.as_str() {
        "containers" => {
            let r = docker
                .prune_containers(Some(PruneContainersOptions::<String> {
                    ..Default::default()
                }))
                .await
                .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
            serde_json::json!({
                "containers_deleted": r.containers_deleted.unwrap_or_default(),
                "space_reclaimed": r.space_reclaimed.unwrap_or(0)
            })
        }
        "images" => {
            let r = docker
                .prune_images(Some(PruneImagesOptions::<String> {
                    ..Default::default()
                }))
                .await
                .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
            serde_json::json!({
                "images_deleted": r.images_deleted.unwrap_or_default(),
                "space_reclaimed": r.space_reclaimed.unwrap_or(0)
            })
        }
        "volumes" => {
            let r = docker
                .prune_volumes(Some(PruneVolumesOptions::<String> {
                    ..Default::default()
                }))
                .await
                .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
            serde_json::json!({
                "volumes_deleted": r.volumes_deleted.unwrap_or_default(),
                "space_reclaimed": r.space_reclaimed.unwrap_or(0)
            })
        }
        "networks" => {
            let r = docker
                .prune_networks(Some(PruneNetworksOptions::<String> {
                    ..Default::default()
                }))
                .await
                .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
            serde_json::json!({
                "networks_deleted": r.networks_deleted.unwrap_or_default()
            })
        }
        other => {
            return Err(error(
                StatusCode::BAD_REQUEST,
                &format!("Unknown resource type: {}. Use containers, images, volumes, or networks", other),
            ));
        }
    };

    Ok(Json(serde_json::json!({
        "status": "pruned",
        "resource": body.resource,
        "result": result
    })))
}

// ── Docker Events (SSE Stream) ────────────────────────────────

pub async fn docker_events(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let docker = match helpers::resolve_client(&state, params.endpoint.as_deref()).await {
        Ok(d) => d,
        Err((_status, json)) => {
            let stream = stream::once(async move {
                Ok(Event::default()
                    .event("error")
                    .data(serde_json::to_string(&json.0).unwrap_or_default()))
            });
            // Box the error stream to match the type from the main branch
            let boxed: stream::BoxStream<'static, Result<Event, Infallible>> = stream.boxed();
            return Sse::new(boxed);
        }
    };

    // Bollard 0.17: docker.events() returns impl Stream<Item = Result<EventMessage, Error>>
    let event_stream = docker.events(Some(EventsOptions::<String> {
        ..Default::default()
    }));

    let sse_stream: stream::BoxStream<'static, Result<Event, Infallible>> =
        event_stream
            .filter_map(move |chunk| {
                async move {
                    match chunk {
                        Ok(event) => {
                            let data = serde_json::to_string(&event).unwrap_or_default();
                            let event_type = event
                                .action
                                .as_ref()
                                .map(|a| format!("{:?}", a))
                                .unwrap_or_else(|| "docker-event".to_string());
                            Some(Ok(Event::default()
                                .event(event_type)
                                .data(data)))
                        }
                        Err(e) => Some(Ok(Event::default()
                            .event("error")
                            .data(e.to_string()))),
                    }
                }
            })
            .boxed();

    Sse::new(sse_stream)
}

// ── Audit Log ─────────────────────────────────────────────────

pub async fn audit_log(
    State(state): State<Arc<crate::AppState>>,
) -> ApiResult<Vec<AuditEntry>> {
    let entries = state.audit_log.list().await;
    Ok(Json(entries))
}
