use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use bollard::volume::{CreateVolumeOptions, ListVolumesOptions, RemoveVolumeOptions, PruneVolumesOptions};
use std::sync::Arc;

use crate::helpers;
use crate::models::*;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn error(code: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({"error": msg})))
}

// ── List Volumes ──────────────────────────────────────────────

pub async fn list_volumes(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<Vec<VolumeSummary>> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    let resp = docker
        .list_volumes(Some(ListVolumesOptions::<String> {
            ..Default::default()
        }))
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    // Bollard 0.17: v.scope is VolumeScopeEnum
    let volumes: Vec<VolumeSummary> = resp
        .volumes
        .unwrap_or_default()
        .into_iter()
        .map(|v| VolumeSummary {
            name: v.name,
            driver: v.driver,
            mountpoint: v.mountpoint,
            scope: format!("{:?}", v.scope).to_lowercase(),
            size_bytes: None,
            size_human: None,
        })
        .collect();

    Ok(Json(volumes))
}

// ── Create Volume ─────────────────────────────────────────────

pub async fn create_volume(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Json(body): Json<VolumeCreateRequest>,
) -> ApiResult<VolumeSummary> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    let vol = docker
        .create_volume(CreateVolumeOptions {
            name: body.name.clone(),
            driver: body.driver.unwrap_or_else(|| "local".to_string()),
            labels: body.labels.clone(),
            driver_opts: body.options.clone(),
        })
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(VolumeSummary {
        name: vol.name,
        driver: vol.driver,
        mountpoint: vol.mountpoint,
        scope: format!("{:?}", vol.scope).to_lowercase(),
        size_bytes: None,
        size_human: None,
    }))
}

// ── Remove Volume ─────────────────────────────────────────────

pub async fn remove_volume(
    State(state): State<Arc<crate::AppState>>,
    Path(name): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    docker
        .remove_volume(
            &name,
            Some(RemoveVolumeOptions { force: true }),
        )
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({"status": "removed", "name": name})))
}

// ── Prune Volumes ─────────────────────────────────────────────

pub async fn prune_volumes(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    let result = docker
        .prune_volumes(Some(PruneVolumesOptions::<String> {
            ..Default::default()
        }))
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({
        "status": "pruned",
        "volumes_deleted": result.volumes_deleted.unwrap_or_default(),
        "space_reclaimed": result.space_reclaimed.unwrap_or(0)
    })))
}
