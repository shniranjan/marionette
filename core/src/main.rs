mod docker;
mod models;
mod audit;
mod db;
mod compose;
mod compose_diff;
mod migration;
mod routes;
mod ws;
mod registry;
mod helpers;
mod transfer;
mod switchover;

use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    Router,
    routing::{delete, get, patch, post, put},
};
use tracing_subscriber::EnvFilter;

use docker::build_initial_endpoints;
use audit::AuditLog;
use db::Database;
use registry::EndpointRegistry;

use crate::routes::endpoints;
use crate::routes::nginx;
use crate::routes::swarm;
use crate::routes::routes_config;
use crate::routes::users;

/// Shared application state.
pub struct AppState {
    pub registry: Arc<EndpointRegistry>,
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

    // Stacks directory
    let stacks_dir = std::env::var("MARIONETTE_STACKS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/stacks"));
    std::fs::create_dir_all(&stacks_dir).ok();

    // Database
    let db_path = std::env::var("MARIONETTE_DB_PATH")
        .unwrap_or_else(|_| "/data/marionette.db".to_string());
    tracing::info!("Audit log database: {}", db_path);

    let db = Database::new(&db_path);
    let audit_log = AuditLog::new(&db_path);

    // Endpoint registry — single source of truth for endpoints + clients
    let (_, _, fresh_default_id) = build_initial_endpoints().await;
    let registry = EndpointRegistry::new(db, fresh_default_id);
    let endpoints = registry.init().await;
    tracing::info!("Registry initialized with {} endpoint(s)", endpoints.len());

    // Application state
    let state = Arc::new(AppState {
        registry: registry.clone(),
        audit_log,
        stacks_dir,
    });

    // Ensure at least one admin user exists
    {
        let admin_key = std::env::var("MARIONETTE_KEY")
            .unwrap_or_else(|_| "admin".to_string());
        state.registry.db().ensure_admin_user(&admin_key);
    }

    // CORS
    let cors = tower_http::cors::CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

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
        .route("/containers/{id}/labels", patch(routes::containers::update_labels))
        .route("/containers/{id}/logs", get(ws::logs::container_logs_ws))
        .route("/containers/logs/merged", get(ws::merged_logs::merged_logs_ws))
        .route("/containers/{id}/logs/download", get(ws::logs::download_logs))
        .route("/containers/{id}/stats", get(ws::stats::container_stats_ws))
        .route("/containers/{id}/exec", get(ws::exec::container_exec_ws))
        .route("/containers/batch", post(routes::containers::batch_containers))
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
        .route("/stacks/{name}/env", get(routes::stacks::get_stack_env))
        .route("/stacks/{name}/env", put(routes::stacks::save_stack_env))
        .route("/stacks/{name}/validate", post(routes::stacks::validate_stack))
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
        .route("/endpoints/{id}/info", get(endpoints::endpoint_info))
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
        // Templates
        .route("/api/templates", get(routes::templates::list_templates))
        .route("/api/templates", post(routes::templates::create_template))
        .route("/api/templates/{id}", get(routes::templates::get_template))
        .route("/api/templates/{id}", delete(routes::templates::delete_template))
        .route("/api/templates/{id}/deploy", post(routes::templates::deploy_template))
        // Migration
        .route("/migration/analyze", post(migration::analyze_migration))
        .route("/migration/plan", post(migration::plan_migration))
        .route("/migration/dry-run", post(migration::dry_run_migration))
        .route("/migration/{id}", get(migration::get_migration))
        .route("/migration/{id}/rollback", post(migration::rollback_migration))
        .route("/migration/{id}/execute", post(migration::execute_migration))
        .route("/migration/transfer", post(migration::transfer_volumes))
        .route("/migration/compose/analyze", post(migration::analyze_compose))
        .route("/migration/compose/prepare", post(migration::prepare_compose_target))
        .route("/migration/compose/switchover", post(migration::switchover_compose))
        .route("/migration/compose/progress", get(ws::progress::switchover_progress_ws))
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
