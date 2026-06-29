use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::compose::ComposeRunner;
use crate::models::*;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn error(code: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({"error": msg})))
}

// ── List Stacks ───────────────────────────────────────────────

pub async fn list_stacks(
    State(state): State<Arc<crate::AppState>>,
) -> ApiResult<Vec<StackSummary>> {
    let compose = ComposeRunner::new(state.stacks_dir.clone());
    let stacks = compose.list_stacks();
    Ok(Json(stacks))
}

// ── Read Stack (YML content) ──────────────────────────────────

pub async fn read_stack(
    State(state): State<Arc<crate::AppState>>,
    Path(name): Path<String>,
) -> ApiResult<serde_json::Value> {
    let dir = state.stacks_dir.join(&name);
    let compose_path = dir.join("docker-compose.yml");
    let alt_path = dir.join("compose.yml");
    let file_path = if compose_path.exists() {
        &compose_path
    } else if alt_path.exists() {
        &alt_path
    } else {
        return Err(error(StatusCode::NOT_FOUND, &format!("Stack '{}' not found", name)));
    };
    let content = tokio::fs::read_to_string(&file_path)
        .await
        .map_err(|_| error(StatusCode::NOT_FOUND, &format!("Stack '{}' not found", name)))?;

    Ok(Json(serde_json::json!({"name": name, "content": content})))
}

// ── Save Stack YML ────────────────────────────────────────────

pub async fn save_stack(
    State(state): State<Arc<crate::AppState>>,
    Path(name): Path<String>,
    Json(body): Json<StackSaveRequest>,
) -> ApiResult<serde_json::Value> {
    let dir = state.stacks_dir.join(&name);

    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let file_path = dir.join("docker-compose.yml");
    tokio::fs::write(&file_path, &body.content)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({"status": "saved", "name": name})))
}

// ── Deploy Stack (docker compose up -d) ───────────────────────

pub async fn deploy_stack(
    State(state): State<Arc<crate::AppState>>,
    Path(name): Path<String>,
) -> ApiResult<serde_json::Value> {
    let compose = ComposeRunner::new(state.stacks_dir.clone());

    let output = compose
        .run(&name, &["up", "-d"])
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    Ok(Json(serde_json::json!({
        "status": "deployed",
        "name": name,
        "output": output
    })))
}

// ── Stop Stack (docker compose stop) ──────────────────────────

pub async fn stop_stack(
    State(state): State<Arc<crate::AppState>>,
    Path(name): Path<String>,
) -> ApiResult<serde_json::Value> {
    let compose = ComposeRunner::new(state.stacks_dir.clone());

    let output = compose
        .run(&name, &["stop"])
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    Ok(Json(serde_json::json!({
        "status": "stopped",
        "name": name,
        "output": output
    })))
}

// ── Down Stack (docker compose down) ──────────────────────────

pub async fn down_stack(
    State(state): State<Arc<crate::AppState>>,
    Path(name): Path<String>,
) -> ApiResult<serde_json::Value> {
    let compose = ComposeRunner::new(state.stacks_dir.clone());

    let output = compose
        .run(&name, &["down"])
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    Ok(Json(serde_json::json!({
        "status": "down",
        "name": name,
        "output": output
    })))
}

// ── Remove Stack ──────────────────────────────────────────────

pub async fn remove_stack(
    State(state): State<Arc<crate::AppState>>,
    Path(name): Path<String>,
) -> ApiResult<serde_json::Value> {
    let dir = state.stacks_dir.join(&name);

    // First try to bring the stack down
    let compose = ComposeRunner::new(state.stacks_dir.clone());
    let _ = compose.run(&name, &["down", "--volumes"]);

    // Remove the directory
    if dir.exists() {
        tokio::fs::remove_dir_all(&dir)
            .await
            .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
    }

    Ok(Json(serde_json::json!({"status": "removed", "name": name})))
}

// ── Get Stack Env File ─────────────────────────────────────────

pub async fn get_stack_env(
    State(state): State<Arc<crate::AppState>>,
    Path(name): Path<String>,
) -> ApiResult<StackEnvResponse> {
    let env_path = state.stacks_dir.join(&name).join(".env");

    if !env_path.exists() {
        return Ok(Json(StackEnvResponse {
            variables: std::collections::HashMap::new(),
        }));
    }

    let content = tokio::fs::read_to_string(&env_path)
        .await
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to read .env file"))?;

    let mut variables = std::collections::HashMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            variables.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    Ok(Json(StackEnvResponse { variables }))
}

// ── Save Stack Env File ────────────────────────────────────────

pub async fn save_stack_env(
    State(state): State<Arc<crate::AppState>>,
    Path(name): Path<String>,
    Json(body): Json<StackEnvRequest>,
) -> ApiResult<serde_json::Value> {
    let dir = state.stacks_dir.join(&name);

    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let env_path = dir.join(".env");
    let mut content = String::new();
    for (key, value) in &body.variables {
        content.push_str(&format!("{}={}\n", key, value));
    }

    tokio::fs::write(&env_path, &content)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(serde_json::json!({"status": "saved", "name": name})))
}


pub async fn create_stack(
    State(state): State<Arc<crate::AppState>>,
    Json(body): Json<StackCreateRequest>,
) -> ApiResult<serde_json::Value> {
    let dir = state.stacks_dir.join(&body.name);

    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let file_path = dir.join("docker-compose.yml");
    tokio::fs::write(&file_path, &body.content)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    // Deploy the stack
    let compose = ComposeRunner::new(state.stacks_dir.clone());
    let output = compose
        .run(&body.name, &["up", "-d"])
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    Ok(Json(serde_json::json!({
        "status": "created_and_deployed",
        "name": body.name,
        "output": output
    })))
}
