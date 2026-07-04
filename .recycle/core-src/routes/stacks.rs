use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::compose::ComposeRunner;
use crate::helpers;
use crate::models::*;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn error(code: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({"error": msg})))
}

// ── Relay helper ──────────────────────────────────────────────

/// Check if a request should be routed through the relay, and return the
/// relay hostname if so.
async fn resolve_endpoint(endpoint: &Option<String>) -> Option<String> {
    if let Some(ref ep_id) = endpoint {
        if ep_id != "local" {
            return crate::ws_relay::get_relay_for_endpoint(ep_id).await;
        }
    }
    None
}

// ── List Stacks ───────────────────────────────────────────────

pub async fn list_stacks(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
) -> ApiResult<Vec<StackSummary>> {
    // If a specific (non-local) endpoint is requested, discover stacks
    // by listing containers with Docker Compose project labels.
    if let Some(ref ep_id) = params.endpoint {
        if ep_id != "local" {
            return list_remote_stacks(&state, ep_id).await;
        }
    }

    // Local: use filesystem-based ComposeRunner
    let compose = ComposeRunner::new(state.stacks_dir.clone());
    let stacks = compose.list_stacks();
    Ok(Json(stacks))
}

/// Discover stacks on a remote endpoint using the relay agent.
/// Falls back to Docker API container-label scanning if no relay is connected.
async fn list_remote_stacks(
    state: &Arc<crate::AppState>,
    endpoint_id: &str,
) -> ApiResult<Vec<StackSummary>> {
    // Try relay first
    if let Some(relay_host) = crate::ws_relay::get_relay_for_endpoint(endpoint_id).await {
        return list_stacks_via_relay(&relay_host, endpoint_id).await;
    }

    // Fallback: Docker API container-label scanning
    list_stacks_via_docker_api(state, endpoint_id).await
}

/// Discover stacks by listing /stacks directory via relay, then running
/// compose config on each subdirectory.
async fn list_stacks_via_relay(
    relay_host: &str,
    _endpoint_id: &str,
) -> ApiResult<Vec<StackSummary>> {
    use relay_protocol::Message;

    // 1. List directories under /stacks
    let list_msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "fs.list",
        serde_json::json!({"path": "/stacks"}),
    );

    let response = crate::ws_relay::send_relay_command(relay_host, list_msg)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Relay error: {}", e)))?;

    let entries: Vec<serde_json::Value> = response
        .payload
        .get("entries")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut stacks = Vec::new();
    for entry in entries {
        let is_dir = entry.get("is_dir").and_then(|v| v.as_bool()).unwrap_or(false);
        if !is_dir {
            continue;
        }
        let name = entry.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.is_empty() || name.starts_with('.') {
            continue;
        }

        // Build summary from directory listing only (fast path).
        // Service counts and status are fetched lazily when user opens a stack.
        stacks.push(StackSummary {
            name: name.to_string(),
            services: 0,
            status: "unknown".to_string(),
            file: format!("/stacks/{}", name),
        });

    }

    Ok(Json(stacks))
}

/// Original Docker API container-label scanning fallback.
async fn list_stacks_via_docker_api(
    state: &Arc<crate::AppState>,
    endpoint_id: &str,
) -> ApiResult<Vec<StackSummary>> {
    use bollard::container::ListContainersOptions;
    use std::collections::HashMap;

    let docker = helpers::resolve_client(state, Some(endpoint_id))
        .await
        .map_err(|(s, j)| {
            error(
                s,
                &j.get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Failed to resolve endpoint"),
            )
        })?;

    let containers = docker
        .list_containers(Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        }))
        .await
        .map_err(|e| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Failed to list containers on remote: {}", e),
            )
        })?;

    let mut projects: HashMap<String, (usize, usize, String)> = HashMap::new();
    for c in &containers {
        let labels = c.labels.as_ref();
        if let Some(project) = labels.and_then(|l| l.get("com.docker.compose.project")) {
            let state_str = c.state.as_deref().unwrap_or("unknown");
            let is_running = state_str == "running";
            let entry = projects.entry(project.clone()).or_insert((0, 0, "stopped".to_string()));
            entry.0 += 1;
            if is_running {
                entry.1 += 1;
            }
        }
    }

    let stacks: Vec<StackSummary> = projects
        .into_iter()
        .map(|(name, (total, running, _))| {
            let status = if running > 0 {
                if running == total {
                    "running".to_string()
                } else {
                    "partial".to_string()
                }
            } else {
                "stopped".to_string()
            };
            StackSummary {
                name,
                services: total,
                status,
                file: format!("(remote: {})", endpoint_id),
            }
        })
        .collect();

    Ok(Json(stacks))
}

// ── Read Stack (YML content) ──────────────────────────────────

pub async fn read_stack(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Path(name): Path<String>,
) -> ApiResult<serde_json::Value> {
    if let Some(hostname) = resolve_endpoint(&params.endpoint).await {
        return read_stack_via_relay(&hostname, &name).await;
    }

    // Local: read from filesystem
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

async fn read_stack_via_relay(relay_host: &str, name: &str) -> ApiResult<serde_json::Value> {
    use relay_protocol::Message;

    let msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "fs.read",
        serde_json::json!({"path": format!("/stacks/{}/docker-compose.yml", name)}),
    );

    let response = crate::ws_relay::send_relay_command(relay_host, msg)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Relay error: {}", e)))?;

    let content = response
        .payload
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    Ok(Json(serde_json::json!({"name": name, "content": content})))
}

// ── Save Stack YML ────────────────────────────────────────────

pub async fn save_stack(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Path(name): Path<String>,
    Json(body): Json<StackSaveRequest>,
) -> ApiResult<serde_json::Value> {
    if let Some(hostname) = resolve_endpoint(&params.endpoint).await {
        return save_stack_via_relay(&hostname, &name, &body.content).await;
    }

    // Local: write to filesystem
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

async fn save_stack_via_relay(relay_host: &str, name: &str, content: &str) -> ApiResult<serde_json::Value> {
    use relay_protocol::Message;

    let msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "fs.write",
        serde_json::json!({
            "path": format!("/stacks/{}/docker-compose.yml", name),
            "content": content,
        }),
    );

    let _response = crate::ws_relay::send_relay_command(relay_host, msg)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Relay error: {}", e)))?;

    Ok(Json(serde_json::json!({"status": "saved", "name": name})))
}

// ── Deploy Stack (docker compose up -d) ───────────────────────

pub async fn deploy_stack(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Path(name): Path<String>,
) -> ApiResult<serde_json::Value> {
    if let Some(hostname) = resolve_endpoint(&params.endpoint).await {
        return deploy_stack_via_relay(&hostname, &name).await;
    }

    // Local: use ComposeRunner
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

async fn deploy_stack_via_relay(relay_host: &str, name: &str) -> ApiResult<serde_json::Value> {
    use relay_protocol::Message;

    let msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "compose.up",
        serde_json::json!({
            "project_dir": format!("/stacks/{}", name),
            "project_name": name,
            "file": format!("/stacks/{}/docker-compose.yml", name),
            "detach": true,
        }),
    );

    let response = crate::ws_relay::send_relay_command(relay_host, msg)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Relay error: {}", e)))?;

    let exit_code = response.payload.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(-1);
    let output = if exit_code == 0 {
        "deployed successfully".to_string()
    } else {
        format!("compose up exited with code {}", exit_code)
    };

    Ok(Json(serde_json::json!({
        "status": "deployed",
        "name": name,
        "output": output
    })))
}

// ── Stop Stack (docker compose stop) ──────────────────────────

pub async fn stop_stack(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Path(name): Path<String>,
) -> ApiResult<serde_json::Value> {
    if let Some(hostname) = resolve_endpoint(&params.endpoint).await {
        return stop_stack_via_relay(&hostname, &name).await;
    }

    // Local: use ComposeRunner
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

async fn stop_stack_via_relay(relay_host: &str, name: &str) -> ApiResult<serde_json::Value> {
    use relay_protocol::Message;

    let msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "compose.stop",
        serde_json::json!({
            "project_dir": format!("/stacks/{}", name),
            "project_name": name,
            "file": format!("/stacks/{}/docker-compose.yml", name),
        }),
    );

    let response = crate::ws_relay::send_relay_command(relay_host, msg)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Relay error: {}", e)))?;

    let exit_code = response.payload.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(-1);
    let output = if exit_code == 0 {
        "stopped successfully".to_string()
    } else {
        format!("compose down exited with code {}", exit_code)
    };

    Ok(Json(serde_json::json!({
        "status": "stopped",
        "name": name,
        "output": output
    })))
}

// ── Down Stack (docker compose down) ──────────────────────────

pub async fn down_stack(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Path(name): Path<String>,
) -> ApiResult<serde_json::Value> {
    if let Some(hostname) = resolve_endpoint(&params.endpoint).await {
        return down_stack_via_relay(&hostname, &name).await;
    }

    // Local: use ComposeRunner
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

async fn down_stack_via_relay(relay_host: &str, name: &str) -> ApiResult<serde_json::Value> {
    use relay_protocol::Message;

    let msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "compose.down",
        serde_json::json!({
            "project_dir": format!("/stacks/{}", name),
            "project_name": name,
            "file": format!("/stacks/{}/docker-compose.yml", name),
            "volumes": false,
        }),
    );

    let response = crate::ws_relay::send_relay_command(relay_host, msg)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Relay error: {}", e)))?;

    let exit_code = response.payload.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(-1);
    let output = if exit_code == 0 {
        "taken down successfully".to_string()
    } else {
        format!("compose down exited with code {}", exit_code)
    };

    Ok(Json(serde_json::json!({
        "status": "down",
        "name": name,
        "output": output
    })))
}

// ── Remove Stack ──────────────────────────────────────────────

pub async fn remove_stack(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Path(name): Path<String>,
) -> ApiResult<serde_json::Value> {
    if let Some(hostname) = resolve_endpoint(&params.endpoint).await {
        return remove_stack_via_relay(&hostname, &name).await;
    }

    // Local: bring down then remove directory
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

async fn remove_stack_via_relay(relay_host: &str, name: &str) -> ApiResult<serde_json::Value> {
    use relay_protocol::Message;

    // 1. Bring stack down with --volumes
    let down_msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "compose.down",
        serde_json::json!({
            "project_dir": format!("/stacks/{}", name),
            "project_name": name,
            "file": format!("/stacks/{}/docker-compose.yml", name),
            "volumes": true,
        }),
    );

    let _ = crate::ws_relay::send_relay_command(relay_host, down_msg)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Relay error: {}", e)))?;

    // 2. Remove the stack directory (overwrite with empty to emulate delete;
    //    there is no fs.delete on the relay agent, so the directory persists
    //    but the compose file is cleared.)
    let rm_msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "fs.write",
        serde_json::json!({
            "path": format!("/stacks/{}/docker-compose.yml", name),
            "content": "",
        }),
    );

    let _ = crate::ws_relay::send_relay_command(relay_host, rm_msg)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Relay error: {}", e)))?;

    Ok(Json(serde_json::json!({"status": "removed", "name": name})))
}

// ── Get Stack Env File ─────────────────────────────────────────

pub async fn get_stack_env(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Path(name): Path<String>,
) -> ApiResult<StackEnvResponse> {
    if let Some(hostname) = resolve_endpoint(&params.endpoint).await {
        return get_stack_env_via_relay(&hostname, &name).await;
    }

    // Local: read from filesystem
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

async fn get_stack_env_via_relay(relay_host: &str, name: &str) -> ApiResult<StackEnvResponse> {
    use relay_protocol::Message;

    let msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "fs.read",
        serde_json::json!({"path": format!("/stacks/{}/.env", name)}),
    );

    let response = crate::ws_relay::send_relay_command(relay_host, msg)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Relay error: {}", e)))?;

    let content = response
        .payload
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");

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
    Query(params): Query<EndpointQuery>,
    Path(name): Path<String>,
    Json(body): Json<StackEnvRequest>,
) -> ApiResult<serde_json::Value> {
    if let Some(hostname) = resolve_endpoint(&params.endpoint).await {
        return save_stack_env_via_relay(&hostname, &name, &body.variables).await;
    }

    // Local: write to filesystem
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

async fn save_stack_env_via_relay(
    relay_host: &str,
    name: &str,
    variables: &std::collections::HashMap<String, String>,
) -> ApiResult<serde_json::Value> {
    use relay_protocol::Message;

    let mut content = String::new();
    for (key, value) in variables {
        content.push_str(&format!("{}={}\n", key, value));
    }

    let msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "fs.write",
        serde_json::json!({
            "path": format!("/stacks/{}/.env", name),
            "content": content,
        }),
    );

    let _response = crate::ws_relay::send_relay_command(relay_host, msg)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Relay error: {}", e)))?;

    Ok(Json(serde_json::json!({"status": "saved", "name": name})))
}

// ── Create Stack ──────────────────────────────────────────────

pub async fn create_stack(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Json(body): Json<StackCreateRequest>,
) -> ApiResult<serde_json::Value> {
    if let Some(hostname) = resolve_endpoint(&params.endpoint).await {
        return create_stack_via_relay(&hostname, &body.name, &body.content).await;
    }

    // Local: create directory, write file, deploy
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

async fn create_stack_via_relay(relay_host: &str, name: &str, content: &str) -> ApiResult<serde_json::Value> {
    use relay_protocol::Message;

    // 1. Write compose file
    let write_msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "fs.write",
        serde_json::json!({
            "path": format!("/stacks/{}/docker-compose.yml", name),
            "content": content,
        }),
    );

    let _ = crate::ws_relay::send_relay_command(relay_host, write_msg)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Relay error: {}", e)))?;

    // 2. Deploy
    let up_msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "compose.up",
        serde_json::json!({
            "project_dir": format!("/stacks/{}", name),
            "project_name": name,
            "file": format!("/stacks/{}/docker-compose.yml", name),
            "detach": true,
        }),
    );

    let response = crate::ws_relay::send_relay_command(relay_host, up_msg)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Relay error: {}", e)))?;

    let exit_code = response.payload.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(-1);
    let output = if exit_code == 0 {
        "created and deployed successfully".to_string()
    } else {
        format!("compose up exited with code {}", exit_code)
    };

    Ok(Json(serde_json::json!({
        "status": "created_and_deployed",
        "name": name,
        "output": output
    })))
}

// ── Validate Stack Config ──────────────────────────────────────

pub async fn validate_stack(
    State(state): State<Arc<crate::AppState>>,
    Query(params): Query<EndpointQuery>,
    Path(name): Path<String>,
    Json(body): Json<StackValidateRequest>,
) -> ApiResult<StackValidateResponse> {
    if let Some(hostname) = resolve_endpoint(&params.endpoint).await {
        return validate_stack_via_relay(&hostname, &name, &body.content).await;
    }

    // Local: save content, run compose config locally
    let dir = state.stacks_dir.join(&name);

    // Ensure directory exists
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    // Save content temporarily
    let file_path = dir.join("docker-compose.yml");
    tokio::fs::write(&file_path, &body.content)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    // Run docker compose config for validation
    let output = std::process::Command::new("docker")
        .args(["compose", "config"])
        .current_dir(&dir)
        .output()
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    if output.status.success() {
        let rendered = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(Json(StackValidateResponse {
            valid: true,
            rendered: Some(rendered),
            errors: None,
            name,
        }))
    } else {
        let err_msg = String::from_utf8_lossy(&output.stderr).to_string();
        let errors: Vec<String> = err_msg
            .lines()
            .map(|l| l.to_string())
            .filter(|l| !l.is_empty())
            .collect();
        Ok(Json(StackValidateResponse {
            valid: false,
            rendered: None,
            errors: Some(errors),
            name,
        }))
    }
}

async fn validate_stack_via_relay(relay_host: &str, name: &str, content: &str) -> ApiResult<StackValidateResponse> {
    use relay_protocol::Message;

    // 1. Write compose file
    let write_msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "fs.write",
        serde_json::json!({
            "path": format!("/stacks/{}/docker-compose.yml", name),
            "content": content,
        }),
    );

    let _ = crate::ws_relay::send_relay_command(relay_host, write_msg)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Relay error: {}", e)))?;

    // 2. Run compose config
    let config_msg = Message::new_request(
        Uuid::new_v4().to_string(),
        "compose.config",
        serde_json::json!({
            "project_dir": format!("/stacks/{}", name),
            "project_name": name,
            "file": format!("/stacks/{}/docker-compose.yml", name),
        }),
    );

    let response = crate::ws_relay::send_relay_command(relay_host, config_msg)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Relay error: {}", e)))?;

    // compose.config returns config_yaml on success, or error on failure
    let rendered = response
        .payload
        .get("config_yaml")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // If we got config_yaml, it's valid. Otherwise check for error.
    if let Some(rendered) = rendered {
        Ok(Json(StackValidateResponse {
            valid: true,
            rendered: Some(rendered),
            errors: None,
            name: name.to_string(),
        }))
    } else {
        // compose.config handler returns error with "COMPOSE.CONFIG_FAILED" code
        let err_msg = response
            .payload
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("compose config failed");
        Ok(Json(StackValidateResponse {
            valid: false,
            rendered: None,
            errors: Some(vec![err_msg.to_string()]),
            name: name.to_string(),
        }))
    }
}
