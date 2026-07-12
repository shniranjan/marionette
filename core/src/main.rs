//! Marionette-core controller-bridge per tunnel-loom spec §5.3.
//!
//! Lightweight entrypoint: health, relay WebSocket, and relay status API.

mod docker;
mod migration;
mod registry;
mod routes;
mod ws_relay;

use axum::{Json, Router, routing::{get, post}};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(env_filter)
        .init();

    tracing::info!("marionette-core controller-bridge starting");

    let app = Router::new()
        .route("/health", get(health))
        .route("/relay", get(ws_relay::relay_handler))
        .route("/api/relay/status", get(relay_status))
        .route("/api/relay", get(relay_status))
        .route("/api/system", get(system_info))
        .route("/api/system/version", get(system_version))
        .route("/api/system/audit", get(system_audit))
        .route("/api/system/events", get(system_events))
        .route("/api/system/config", get(system_config))
        .route("/api/system/config", post(update_system_config))
        .route("/api/system/token", post(generate_token))
        .route("/api/endpoints", get(list_endpoints))
        .route("/api/stacks", get(list_stacks))
        .route("/api/routes", get(list_routes))
        .route("/swarm", get(swarm_status))
        .route("/swarm/nodes", get(swarm_nodes))
        .route("/swarm/services", get(swarm_services))
        .route("/swarm/configs", get(swarm_configs))
        .route("/swarm/secrets", get(swarm_secrets))
        .route("/api/containers", get(routes::list_containers))
        .route("/api/containers/{id}", get(routes::inspect_container))
        .route("/api/containers/{id}/start", post(routes::start_container))
        .route("/api/containers/{id}/stop", post(routes::stop_container))
        .route("/api/containers/{id}/restart", post(routes::restart_container))
        .route("/api/images", get(routes::list_images))
        .route("/api/volumes", get(routes::list_volumes))
        .route("/api/networks", get(routes::list_networks))
        .route("/api/migration/analyze", post(routes::analyze_migration_handler))
        .route("/api/migration/plan/{id}", get(routes::get_migration_plan))
        .route("/api/migration/prepare", post(routes::prepare_migration_handler))
        .route("/api/migration/transfer", post(routes::transfer_migration_handler))
        .route("/api/migration/switchover", post(routes::switchover_migration_handler))
        .route("/api/migration/rollback", post(routes::rollback_migration_handler))
        .route("/api/migration/status/{id}", get(routes::migration_status_handler))
        .route("/api/migration/events/{id}", get(routes::migration_events_handler));

    let port: u16 = std::env::var("MARIONETTE_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9119);
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {}: {}", addr, e));
    tracing::info!("controller-bridge listening on {}", addr);

    axum::serve(listener, app)
        .await
        .expect("Server error");
}

// ── Handlers ────────────────────────────────────────────────────────

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

async fn relay_status() -> Json<serde_json::Value> {
    let relays = ws_relay::list_relays().await;
    Json(serde_json::to_value(relays).unwrap_or(serde_json::json!([])))
}

async fn system_info() -> Json<serde_json::Value> {
    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".into());
    let endpoints: Vec<serde_json::Value> = ws_relay::list_relays()
        .await
        .iter()
        .map(|r| serde_json::json!({
            "hostname": r.hostname,
            "connected": r.relay_connected,
        }))
        .collect();
    Json(serde_json::json!({
        "hostname": hostname,
        "version": env!("CARGO_PKG_VERSION"),
        "docker_endpoints": endpoints,
    }))
}

async fn list_endpoints() -> Json<serde_json::Value> {
    let relays = ws_relay::list_relays().await;
    let mut eps: Vec<serde_json::Value> = relays
        .iter()
        .map(|r| serde_json::json!({
            "hostname": r.hostname,
            "connected": r.relay_connected,
            "type": "remote",
        }))
        .collect();
    eps.push(serde_json::json!({
        "hostname": "local",
        "connected": true,
        "type": "local",
    }));
    Json(serde_json::json!(eps))
}

async fn system_version() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "api_version": "1.0",
        "docker_api_version": "1.47",
    }))
}

async fn system_audit() -> Json<serde_json::Value> {
    let logs: Vec<_> = vec![
        serde_json::json!({"time": "now", "event": "marionette-core started", "level": "info"}),
    ];
    Json(serde_json::json!(logs))
}
async fn system_events() -> Json<serde_json::Value> { Json(serde_json::json!([])) }

/// GET /api/system/config → current runtime configuration
async fn system_config() -> Json<serde_json::Value> {
    let mask = |v: &str| -> String {
        if v.len() > 8 { format!("{}...{}", &v[..4], &v[v.len()-4..]) } else { "***".into() }
    };
    Json(serde_json::json!({
        "marionette_port": std::env::var("MARIONETTE_PORT").unwrap_or_else(|_| "9119".into()),
        "gateway_port": std::env::var("MARIONETTE_GATEWAY_PORT").unwrap_or_else(|_| "3000".into()),
        "relay_addr": std::env::var("MARIONETTE_RELAY_ADDR").unwrap_or_else(|_| "0.0.0.0:9120".into()),
        "stacks_dir": std::env::var("MARIONETTE_STACKS_DIR").unwrap_or_else(|_| "/stacks".into()),
        "log_level": std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        "marionette_key": mask(&std::env::var("MARIONETTE_KEY").unwrap_or_default()),
        "relay_token_set": std::env::var("RELAY_TOKEN").is_ok(),
    }))
}

/// POST /api/system/config → update settings (writes env notes)
async fn update_system_config(
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let mut updated = Vec::new();
    if let Some(v) = body.get("log_level").and_then(|v| v.as_str()) {
        std::env::set_var("RUST_LOG", v);
        updated.push("log_level");
    }
    Json(serde_json::json!({
        "status": "ok",
        "updated": updated,
        "note": "Changes apply on restart. Use docker-compose environment for permanent changes."
    }))
}

/// POST /api/system/token → generate a relay token
async fn generate_token() -> Json<serde_json::Value> {
    use rand::Rng;
    let token: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(48)
        .map(char::from)
        .collect();
    Json(serde_json::json!({
        "token": format!("mr_{}", token),
        "note": "Set RELAY_TOKEN env var and restart relay agents",
    }))
}

async fn list_stacks() -> Json<serde_json::Value> {
    let dir = std::env::var("MARIONETTE_STACKS_DIR").unwrap_or_else(|_| "/stacks".into());
    let mut stacks = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                stacks.push(serde_json::json!({
                    "name": name,
                    "path": path.to_string_lossy(),
                    "has_compose": path.join("docker-compose.yml").exists(),
                }));
            }
        }
    }
    Json(serde_json::json!(stacks))
}

async fn list_routes() -> Json<serde_json::Value> {
    Json(serde_json::json!([
        "/health", "/relay",
        "/api/relay", "/api/relay/status",
        "/api/system", "/api/system/version", "/api/system/audit", "/api/system/events",
        "/api/endpoints", "/api/stacks", "/api/routes",
        "/api/containers", "/api/containers/{id}",
        "/api/containers/{id}/start", "/api/containers/{id}/stop", "/api/containers/{id}/restart",
        "/api/images", "/api/volumes", "/api/networks",
        "/api/migration/analyze", "/api/migration/prepare", "/api/migration/transfer",
        "/api/migration/switchover", "/api/migration/rollback",
        "/api/migration/status/{id}", "/api/migration/events/{id}",
    ]))
}

async fn swarm_status() -> Json<serde_json::Value> { Json(serde_json::json!({"active": false, "nodes": 0})) }
async fn swarm_nodes() -> Json<serde_json::Value> { Json(serde_json::json!([])) }
async fn swarm_services() -> Json<serde_json::Value> { Json(serde_json::json!([])) }
async fn swarm_configs() -> Json<serde_json::Value> { Json(serde_json::json!([])) }
async fn swarm_secrets() -> Json<serde_json::Value> { Json(serde_json::json!([])) }
