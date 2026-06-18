mod docker;
mod models;
mod audit;
mod db;
mod compose;
mod migration;
mod routes;
mod ws;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    Router,
    routing::{delete, get, patch, post, put},
};
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::EnvFilter;

use bollard::Docker;
use docker::build_initial_endpoints;
use models::DockerEndpoint;
use audit::AuditLog;
use db::Database;

use crate::routes::endpoints;
use crate::routes::nginx;
use crate::routes::swarm;
use crate::routes::routes_config;
use crate::routes::users;

/// Shared application state.
pub struct AppState {
    pub endpoints: RwLock<HashMap<String, DockerEndpoint>>,
    pub clients: RwLock<HashMap<String, Docker>>,
    pub default_endpoint: String,
    pub db: Database,
    pub audit_log: AuditLog,
    pub stacks_dir: PathBuf,
}

#[tokio::main]
async fn main() {
    // Logging
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .init();

    tracing::info!("marionette-core starting...");

    // Docker endpoints
    let (endpoints, clients, default_endpoint) = build_initial_endpoints().await;
    tracing::info!(
        "Initialized {} Docker endpoint(s), default: {}",
        endpoints.len(),
        default_endpoint
    );

    // Stacks directory
    let stacks_dir = std::env::var("MARIONETTE_STACKS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/stacks"));
    std::fs::create_dir_all(&stacks_dir).ok();

    // Audit log database
    let db_path = std::env::var("MARIONETTE_DB_PATH")
        .unwrap_or_else(|_| "/data/marionette.db".to_string());
    tracing::info!("Audit log database: {}", db_path);

    // Application state
    let state = Arc::new(AppState {
        endpoints: RwLock::new(HashMap::new()),
        clients: RwLock::new(clients),
        default_endpoint: default_endpoint.clone(),
        db: Database::new(&db_path),
        audit_log: AuditLog::new(&db_path),
        stacks_dir,
    });

    // Migrate: load endpoints from DB, or seed from in-memory if first run
    {
        let db_endpoints = state.db.load_endpoints();
        let mut ep_map = state.endpoints.write().await;
        if db_endpoints.is_empty() {
            // First run — seed DB from the initial endpoints we just created
            let initial: Vec<DockerEndpoint> = endpoints.values().cloned().collect();
            state.db.seed_endpoints(&initial);
            *ep_map = endpoints;
            tracing::info!("Seeded endpoints from initial discovery");
        } else {
            // Restore from DB
            for ep in db_endpoints {
                ep_map.insert(ep.id.clone(), ep);
            }
            tracing::info!("Restored {} endpoint(s) from database", ep_map.len());
        }
    }

    // Ensure at least one admin user exists (from MARIONETTE_KEY or default)
    {
        let admin_key = std::env::var("MARIONETTE_KEY")
            .unwrap_or_else(|_| "admin".to_string());
        state.db.ensure_admin_user(&admin_key);
    }

    // CORS — allow all origins (auth is handled by the gateway)
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Router
    let app = Router::new()
        // Health
        .route("/health", get(health))
        // Containers
        .route("/containers", get(routes::containers::list_containers))
        .route("/containers/{id}", get(routes::containers::inspect_container))
        .route("/containers/{id}/start", post(routes::containers::start_container))
        .route("/containers/{id}/stop", post(routes::containers::stop_container))
        .route("/containers/{id}/restart", post(routes::containers::restart_container))
        .route("/containers/{id}/kill", post(routes::containers::kill_container))
        .route("/containers/{id}/pause", post(routes::containers::pause_container))
        .route("/containers/{id}/unpause", post(routes::containers::unpause_container))
        .route("/containers/{id}", delete(routes::containers::remove_container))
        .route("/containers/{id}/rename", post(routes::containers::rename_container))
        .route("/containers/{id}/logs", get(ws::logs::container_logs_ws))
        .route("/containers/{id}/stats", get(ws::stats::container_stats_ws))
        // Images
        .route("/images", get(routes::images::list_images))
        .route("/images/{id}", get(routes::images::inspect_image))
        .route("/images/pull", post(routes::images::pull_image))
        .route("/images/{id}", delete(routes::images::remove_image))
        .route("/images/{id}/history", get(routes::images::image_history))
        // Volumes
        .route("/volumes", get(routes::volumes::list_volumes))
        .route("/volumes", post(routes::volumes::create_volume))
        .route("/volumes/{name}", delete(routes::volumes::remove_volume))
        .route("/volumes/prune", post(routes::volumes::prune_volumes))
        .route("/volumes/{name}/inspect", get(routes::volumes_inspect::deep_inspect_volume))
        // Networks
        .route("/networks", get(routes::networks::list_networks))
        .route("/networks/{id}", get(routes::networks::inspect_network))
        .route("/networks", post(routes::networks::create_network))
        .route("/networks/{id}", delete(routes::networks::remove_network))
        .route("/networks/{id}/connect", post(routes::networks::connect_to_network))
        .route("/networks/{id}/disconnect", post(routes::networks::disconnect_from_network))
        .route("/networks/prune", post(routes::networks::prune_networks))
        // Stacks
        .route("/stacks", get(routes::stacks::list_stacks))
        .route("/stacks", post(routes::stacks::create_stack))
        .route("/stacks/{name}", get(routes::stacks::read_stack))
        .route("/stacks/{name}", put(routes::stacks::save_stack))
        .route("/stacks/{name}", delete(routes::stacks::remove_stack))
        .route("/stacks/{name}/deploy", post(routes::stacks::deploy_stack))
        .route("/stacks/{name}/stop", post(routes::stacks::stop_stack))
        .route("/stacks/{name}/down", post(routes::stacks::down_stack))
        .route("/stacks/{name}/deploy/stream", get(ws::deploy::deploy_stream_ws))
        // System
        .route("/system", get(routes::system::system_info))
        .route("/system/version", get(routes::system::docker_version))
        .route("/system/prune", post(routes::system::prune_resources))
        .route("/system/events", get(routes::system::docker_events))
        .route("/system/audit", get(routes::system::audit_log))
        // Swarm
        .route("/swarm/init", post(swarm::swarm_init))
        .route("/swarm/join", post(swarm::swarm_join))
        .route("/swarm/leave", post(swarm::swarm_leave))
        .route("/swarm", get(swarm::inspect_swarm))
        .route("/swarm/nodes", get(swarm::list_nodes))
        .route("/swarm/nodes/{id}", get(swarm::inspect_node))
        .route("/swarm/nodes/{id}", patch(swarm::update_node))
        .route("/swarm/nodes/{id}", delete(swarm::delete_node))
        .route("/swarm/services", get(swarm::list_services))
        .route("/swarm/services/{id}", get(swarm::inspect_service))
        .route("/swarm/services/create", post(swarm::create_service))
        .route("/swarm/services/{id}", patch(swarm::update_service))
        .route("/swarm/services/{id}", delete(swarm::delete_service))
        .route("/swarm/services/{id}/logs", get(swarm::service_logs))
        .route("/swarm/services/{id}/rollback", post(swarm::rollback_service))
        .route("/swarm/tasks", get(swarm::list_tasks))
        .route("/swarm/tasks/{id}", get(swarm::inspect_task))
        .route("/swarm/tasks/{id}/logs", get(swarm::task_logs))
        .route("/swarm/secrets", get(swarm::list_secrets))
        .route("/swarm/secrets/create", post(swarm::create_secret))
        .route("/swarm/secrets/{id}", delete(swarm::delete_secret))
        .route("/swarm/configs", get(swarm::list_configs))
        .route("/swarm/configs/create", post(swarm::create_config))
        .route("/swarm/configs/{id}", delete(swarm::delete_config))
        // Nginx LB
        .route("/nginx/upstreams", get(nginx::list_upstreams))
        .route("/nginx/config", get(nginx::get_config))
        .route("/nginx/regenerate", post(nginx::regenerate))
        .route("/nginx/test", post(nginx::test_config))
        .route("/nginx/reload", post(nginx::reload_nginx))
        // Endpoints
        .route("/endpoints", get(endpoints::list_endpoints))
        .route("/endpoints", post(endpoints::create_endpoint))
        .route("/endpoints/{id}", get(endpoints::get_endpoint))
        .route("/endpoints/{id}", patch(endpoints::update_endpoint))
        .route("/endpoints/{id}", delete(endpoints::delete_endpoint))
        .route("/endpoints/{id}/test", post(endpoints::test_endpoint))
        .route("/endpoints/{id}/reconnect", post(endpoints::reconnect_endpoint))
        // Routes (AuxGate config)
        .route("/routes", get(routes_config::list_routes))
        .route("/routes", post(routes_config::create_route))
        .route("/routes/{id}", get(routes_config::get_route))
        .route("/routes/{id}", patch(routes_config::update_route))
        .route("/routes/{id}", delete(routes_config::delete_route))
        .route("/routes/{id}/access", get(routes_config::list_route_access))
        .route("/routes/{id}/access", post(routes_config::grant_route_access))
        .route("/routes/{id}/access/{user_id}", delete(routes_config::revoke_route_access))
        // Users
        .route("/users", get(users::list_users))
        // Migration
        .route("/migration/analyze", post(migration::analyze_migration))
        .route("/migration/plan", post(migration::plan_migration))
        .route("/migration/dry-run", post(migration::dry_run_migration))
        .route("/migration/{id}", get(migration::get_migration))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:9119")
        .await
        .expect("Failed to bind to 127.0.0.1:9119");
    tracing::info!("marionette-core listening on 127.0.0.1:9119");

    axum::serve(listener, app)
        .await
        .expect("Server error");
}

/// Health check endpoint.
async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({"status": "ok", "service": "marionette-core", "version": "0.1.0"}))
}
