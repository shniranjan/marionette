use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use bollard::network::{
    ConnectNetworkOptions, CreateNetworkOptions, DisconnectNetworkOptions,
    InspectNetworkOptions, ListNetworksOptions, PruneNetworksOptions,
};
use std::sync::Arc;

use crate::helpers;
use crate::models::*;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn error(code: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({"error": msg})))
}

// ── List Networks ─────────────────────────────────────────────

pub async fn list_networks(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<Vec<NetworkSummary>> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    let networks = docker
        .list_networks::<String>(Some(ListNetworksOptions {
            ..Default::default()
        }))
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let summaries: Vec<NetworkSummary> = networks
        .into_iter()
        .map(|n| {
            let containers: Vec<String> = n
                .containers
                .unwrap_or_default()
                .into_iter()
                .map(|(id, _info)| id)
                .collect();

            NetworkSummary {
                id: n.id.unwrap_or_default(),
                name: n.name.unwrap_or_default(),
                driver: n.driver.unwrap_or_default(),
                scope: n.scope.unwrap_or_else(|| "local".to_string()),
                internal: n.internal.unwrap_or(false),
                containers,
                labels: n.labels.unwrap_or_default(),
            }
        })
        .collect();

    Ok(Json(summaries))
}

// ── Inspect Network ───────────────────────────────────────────

pub async fn inspect_network(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<NetworkSummary> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    // Bollard 0.17: inspect_network takes 2 args: &id, Option<InspectNetworkOptions<T>>
    let net = docker
        .inspect_network::<String>(&id, None::<InspectNetworkOptions<String>>)
        .await
        .map_err(|e| error(StatusCode::NOT_FOUND, &e.to_string()))?;

    let containers: Vec<String> = net
        .containers
        .unwrap_or_default()
        .into_iter()
        .map(|(id, _info)| id)
        .collect();

    Ok(Json(NetworkSummary {
        id: net.id.unwrap_or_default(),
        name: net.name.unwrap_or_default(),
        driver: net.driver.unwrap_or_default(),
        scope: net.scope.unwrap_or_else(|| "local".to_string()),
        internal: net.internal.unwrap_or(false),
        containers,
        labels: net.labels.unwrap_or_default(),
    }))
}

// ── Create Network ────────────────────────────────────────────

pub async fn create_network(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Json(body): Json<NetworkCreateRequest>,
) -> ApiResult<serde_json::Value> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    let resp = docker
        .create_network(CreateNetworkOptions {
            name: body.name.clone(),
            driver: body.driver.unwrap_or_else(|| "bridge".to_string()),
            internal: body.internal,
            labels: body.labels.clone(),
            ..Default::default()
        })
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({
        "status": "created",
        "id": resp.id,
        "name": body.name
    })))
}

// ── Remove Network ────────────────────────────────────────────

pub async fn remove_network(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    // Bollard 0.17: remove_network takes just &id (no options)
    docker
        .remove_network(&id)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({"status": "removed", "id": id})))
}

// ── Connect Container to Network ──────────────────────────────

pub async fn connect_to_network(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
    Json(body): Json<NetworkConnectRequest>,
) -> ApiResult<serde_json::Value> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    docker
        .connect_network(
            &id,
            ConnectNetworkOptions {
                container: body.container.clone(),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({
        "status": "connected",
        "network": id,
        "container": body.container
    })))
}

// ── Disconnect Container from Network ─────────────────────────

pub async fn disconnect_from_network(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Query(params): Query<EndpointQuery>,
    Json(body): Json<NetworkConnectRequest>,
) -> ApiResult<serde_json::Value> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    docker
        .disconnect_network(
            &id,
            DisconnectNetworkOptions {
                container: body.container.clone(),
                force: true,
            },
        )
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({
        "status": "disconnected",
        "network": id,
        "container": body.container
    })))
}

// ── Prune Networks ────────────────────────────────────────────

pub async fn prune_networks(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<serde_json::Value> {
    let docker = helpers::resolve_client(&state, params.endpoint.as_deref()).await?;

    // Bollard 0.17: prune_networks takes Option<PruneNetworksOptions<T>>
    let result = docker
        .prune_networks(Some(PruneNetworksOptions::<String> {
            ..Default::default()
        }))
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({
        "status": "pruned",
        "networks_deleted": result.networks_deleted.unwrap_or_default()
    })))
}
