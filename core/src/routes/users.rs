use axum::{extract::State, Json};
use std::sync::Arc;
use crate::models::UserSummary;

type ApiResult<T> = Result<Json<T>, (axum::http::StatusCode, Json<serde_json::Value>)>;

pub async fn list_users(
    State(state): State<Arc<crate::AppState>>,
) -> ApiResult<Vec<UserSummary>> {
    Ok(Json(state.db.list_users()))
}
