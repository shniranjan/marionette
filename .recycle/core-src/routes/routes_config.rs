use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::models::*;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn error(code: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({"error": msg})))
}

// ── List Routes ───────────────────────────────────────────────

pub async fn list_routes(
    State(state): State<Arc<crate::AppState>>,
) -> ApiResult<Vec<Route>> {
    Ok(Json(state.registry.db().list_routes()))
}

// ── Create Route ──────────────────────────────────────────────

pub async fn create_route(
    State(state): State<Arc<crate::AppState>>,
    Json(body): Json<RouteCreateRequest>,
) -> ApiResult<Route> {
    if !body.path.starts_with('/') {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Path must start with '/'",
        ));
    }

    let route = Route {
        id: Uuid::new_v4().to_string(),
        path: body.path,
        target: body.target,
        auth_mode: body.auth_mode,
        auth_value: body.auth_value,
        tls: body.tls,
        active: true,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    state.registry.db().upsert_route(&route);

    Ok(Json(route))
}

// ── Get Route ─────────────────────────────────────────────────

pub async fn get_route(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Route> {
    state
        .registry.db()
        .get_route(&id)
        .map(Json)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, &format!("Route '{}' not found", id)))
}

// ── Update Route ──────────────────────────────────────────────

pub async fn update_route(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Json(body): Json<RouteUpdateRequest>,
) -> ApiResult<Route> {
    let mut route = state
        .registry.db()
        .get_route(&id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, &format!("Route '{}' not found", id)))?;

    if let Some(path) = body.path {
        if !path.starts_with('/') {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "Path must start with '/'",
            ));
        }
        route.path = path;
    }
    if let Some(target) = body.target {
        route.target = target;
    }
    if let Some(auth_mode) = body.auth_mode {
        route.auth_mode = auth_mode;
    }
    if body.auth_value.is_some() {
        route.auth_value = body.auth_value;
    }
    if let Some(tls) = body.tls {
        route.tls = tls;
    }
    if let Some(active) = body.active {
        route.active = active;
    }

    state.registry.db().upsert_route(&route);

    Ok(Json(route))
}

// ── Delete Route ──────────────────────────────────────────────

pub async fn delete_route(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    if state.registry.db().get_route(&id).is_none() {
        return Err(error(
            StatusCode::NOT_FOUND,
            &format!("Route '{}' not found", id),
        ));
    }

    state.registry.db().delete_route(&id);

    Ok(Json(serde_json::json!({"status": "deleted", "id": id})))
}

// ── List Route Access ─────────────────────────────────────────

pub async fn list_route_access(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Vec<String>> {
    // Verify route exists
    state
        .registry.db()
        .get_route(&id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, &format!("Route '{}' not found", id)))?;

    Ok(Json(state.registry.db().list_route_access(&id)))
}

// ── Grant Route Access ────────────────────────────────────────

pub async fn grant_route_access(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Json(body): Json<RouteAccessRequest>,
) -> ApiResult<serde_json::Value> {
    // Verify route exists
    state
        .registry.db()
        .get_route(&id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, &format!("Route '{}' not found", id)))?;

    state.registry.db().grant_route_access(&id, &body.user_id);

    Ok(Json(serde_json::json!({"status": "granted", "route_id": id, "user_id": body.user_id})))
}

// ── Revoke Route Access ───────────────────────────────────────

pub async fn revoke_route_access(
    State(state): State<Arc<crate::AppState>>,
    Path((id, user_id)): Path<(String, String)>,
) -> ApiResult<serde_json::Value> {
    state.registry.db().revoke_route_access(&id, &user_id);

    Ok(Json(serde_json::json!({"status": "revoked", "route_id": id, "user_id": user_id})))
}
