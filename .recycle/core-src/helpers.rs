use axum::http::StatusCode;
use axum::Json;

use bollard::Docker;

use crate::AppState;

/// Resolve a Docker client for the given endpoint ID (or default).
/// This replaces the old pattern of reading state.endpoints + state.clients HashMaps.
pub async fn resolve_client(
    state: &AppState,
    endpoint_id: Option<&str>,
) -> Result<Docker, (StatusCode, Json<serde_json::Value>)> {
    let id = match endpoint_id {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => state.registry.default_endpoint().await,
    };
    state.registry.get_client(&id).await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e})),
        )
    })
}

/// Resolve an endpoint ID (or default). Returns the ID string.
pub async fn resolve_endpoint_id(
    state: &AppState,
    endpoint_id: Option<&str>,
) -> String {
    match endpoint_id {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => state.registry.default_endpoint().await,
    }
}
