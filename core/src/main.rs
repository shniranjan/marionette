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
    // ── Logging: JSON format ────────────────────────────────────────
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(env_filter)
        .init();

    tracing::info!("marionette-core controller-bridge starting");

    // ── Router ──────────────────────────────────────────────────────
    let app = Router::new()
        // Health
        .route("/health", get(health))
        // Relay WebSocket upgrade
        .route("/relay", get(ws_relay::relay_handler))
        // Relay status API
        .route("/api/relay/status", get(relay_status))
        .route("/api/relay", get(relay_status))
        .route("/api/system", get(system_info))
        .route("/api/endpoints", get(list_endpoints))
        // ── Container routes ──────────────────────────────────
        .route("/api/containers", get(routes::list_containers))
        .route("/api/containers/{id}", get(routes::inspect_container))
        .route("/api/containers/{id}/start", post(routes::start_container))
        .route("/api/containers/{id}/stop", post(routes::stop_container))
        .route("/api/containers/{id}/restart", post(routes::restart_container))
        // ── Image routes ──────────────────────────────────────
        .route("/api/images", get(routes::list_images))
        // ── Volume routes ─────────────────────────────────────
        .route("/api/volumes", get(routes::list_volumes))
        // ── Network routes ────────────────────────────────────
        .route("/api/networks", get(routes::list_networks))
        // ── Migration routes ──────────────────────────────────
        .route("/api/migration/analyze", post(routes::analyze_migration_handler))
        .route("/api/migration/plan/{id}", get(routes::get_migration_plan))
        .route("/api/migration/prepare", post(routes::prepare_migration_handler))
        .route("/api/migration/transfer", post(routes::transfer_migration_handler))
        .route("/api/migration/switchover", post(routes::switchover_migration_handler))
        .route("/api/migration/rollback", post(routes::rollback_migration_handler))
        .route("/api/migration/status/{id}", get(routes::migration_status_handler))
        .route("/api/migration/events/{id}", get(routes::migration_events_handler));

    // ── Bind and serve ──────────────────────────────────────────────
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

/// GET /health → { "status": "ok" }
async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

/// GET /api/relay/status → list of connected relay endpoints
async fn relay_status() -> Json<serde_json::Value> {
    let relays = ws_relay::list_relays().await;
    Json(serde_json::to_value(relays).unwrap_or(serde_json::json!([])))
}

/// GET /api/system → system info (dashboard needs this)
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

/// GET /api/endpoints → list of Docker endpoints (dashboard needs this)
async fn list_endpoints() -> Json<serde_json::Value> {
    let relays = ws_relay::list_relays().await;
    let mut endpoints: Vec<serde_json::Value> = relays
        .iter()
        .map(|r| serde_json::json!({
            "hostname": r.hostname,
            "connected": r.relay_connected,
            "type": "remote",
        }))
        .collect();
    endpoints.push(serde_json::json!({
        "hostname": "local",
        "connected": true,
        "type": "local",
    }));
    Json(serde_json::json!(endpoints))
}
