use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use bollard::Docker;

use crate::db::Database;
use crate::docker;
use crate::models::{DockerEndpoint, EndpointStatus};

/// Single source of truth for endpoint state.
///
/// Endpoints are stored in SQLite (via `Database`). Docker clients are cached
/// in-memory and lazily created on first use. This replaces the old dual-storage
/// pattern (HashMap + SQLite) that caused UUID mismatch bugs on restart.
pub struct EndpointRegistry {
    db: Database,
    clients: RwLock<HashMap<String, Docker>>,
    /// Default endpoint ID — the first connected or first available.
    default_id: RwLock<String>,
}

impl EndpointRegistry {
    pub fn new(db: Database, default_id: String) -> Arc<Self> {
        Arc::new(Self {
            db,
            clients: RwLock::new(HashMap::new()),
            default_id: RwLock::new(default_id),
        })
    }

    /// Initialize: load from DB, seed if empty, reconnect all clients.
    pub async fn init(self: &Arc<Self>) -> Vec<DockerEndpoint> {
        let db_endpoints = self.db.load_endpoints();

        if db_endpoints.is_empty() {
            // First run — create a local endpoint
            let local_id = uuid::Uuid::new_v4().to_string();
            let local_conn = std::env::var("DOCKER_HOST")
                .unwrap_or_else(|_| "unix:///var/run/docker.sock".to_string());

            let ep = DockerEndpoint {
                id: local_id.clone(),
                name: "local".to_string(),
                connection: local_conn,
                status: EndpointStatus::Disconnected,
                tags: vec!["default".to_string()],
                cert_path: None,
                stacks_dir: None,
            };

            self.db.seed_endpoints(&[ep.clone()]);
            *self.default_id.write().await = local_id;

            // Try to connect
            match docker::create_client(&ep.connection, None) {
                Ok(client) => {
                    self.clients.write().await.insert(ep.id.clone(), client);
                }
                Err(e) => {
                    tracing::warn!("Local Docker not available: {}", e);
                }
            }

            tracing::info!("Seeded local endpoint (first run)");
            vec![ep]
        } else {
            // Restore from DB, reconnect clients
            let mut clients = self.clients.write().await;
            clients.clear();

            for ep in &db_endpoints {
                match docker::create_client(&ep.connection, ep.cert_path.as_deref()) {
                    Ok(client) => {
                        clients.insert(ep.id.clone(), client);
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to connect endpoint '{}' ({}): {}",
                            ep.name, ep.connection, e
                        );
                    }
                }
            }

            // Set default to first endpoint with a working client, or first overall
            let default = db_endpoints
                .iter()
                .find(|ep| clients.contains_key(&ep.id))
                .or(db_endpoints.first())
                .map(|ep| ep.id.clone())
                .unwrap_or_default();
            *self.default_id.write().await = default;

            tracing::info!("Restored {} endpoint(s) from database", db_endpoints.len());
            db_endpoints
        }
    }

    /// List all endpoints (from DB, with live status from client cache).
    pub async fn list(&self) -> Vec<DockerEndpoint> {
        let clients = self.clients.read().await;
        let mut endpoints = self.db.load_endpoints();

        for ep in &mut endpoints {
            ep.status = if clients.contains_key(&ep.id) {
                EndpointStatus::Connected
            } else {
                EndpointStatus::Disconnected
            };
        }

        endpoints
    }

    /// Get a single endpoint by ID.
    pub async fn get(&self, id: &str) -> Option<DockerEndpoint> {
        let eps = self.db.load_endpoints();
        eps.into_iter().find(|ep| ep.id == id)
    }

    /// Get a Docker client for an endpoint (lazy-connect if not cached).
    pub async fn get_client(&self, id: &str) -> Result<Docker, String> {
        // Check cache first
        {
            let clients = self.clients.read().await;
            if let Some(client) = clients.get(id) {
                // Quick health check
                match tokio::time::timeout(
                    std::time::Duration::from_secs(3),
                    client.ping(),
                )
                .await
                {
                    Ok(Ok(_)) => return Ok(client.clone()),
                    _ => {
                        // Client stale — drop read lock and recreate below
                    }
                }
            }
        }

        // Not cached or stale — look up endpoint and connect
        let ep = self
            .get(id)
            .await
            .ok_or_else(|| format!("Endpoint '{}' not found", id))?;

        let client = docker::create_client(&ep.connection, ep.cert_path.as_deref())
            .map_err(|e| format!("Failed to connect '{}': {}", ep.name, e))?;

        self.clients.write().await.insert(id.to_string(), client.clone());
        Ok(client)
    }

    /// Create a new endpoint: validate, persist, cache client.
    pub async fn create(&self, ep: DockerEndpoint) -> Result<DockerEndpoint, String> {
        // Check for duplicate name
        let existing = self.list().await;
        if existing.iter().any(|e| e.name == ep.name) {
            return Err(format!("Endpoint '{}' already exists", ep.name));
        }

        // Test connection
        let client = docker::create_client(&ep.connection, ep.cert_path.as_deref())
            .map_err(|e| format!("Connection failed: {}", e))?;

        match tokio::time::timeout(std::time::Duration::from_secs(5), client.ping()).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => return Err(format!("Connection test failed: {}", e)),
            Err(_) => return Err("Connection timed out (5s)".to_string()),
        }

        // Persist to DB
        self.db.upsert_endpoint(&ep);

        // Cache client
        self.clients.write().await.insert(ep.id.clone(), client);

        Ok(ep)
    }

    /// Update an endpoint.
    pub async fn update(
        &self,
        id: &str,
        name: Option<String>,
        connection: Option<String>,
        tags: Option<Vec<String>>,
        cert_path: Option<Option<String>>,
        stacks_dir: Option<Option<String>>,
    ) -> Result<(), String> {
        let mut ep = self
            .get(id)
            .await
            .ok_or_else(|| format!("Endpoint '{}' not found", id))?;

        let conn_changed = connection.is_some();
        let cert_changed = cert_path.is_some();

        if let Some(name) = name {
            ep.name = name;
        }
        if let Some(connection) = connection {
            ep.connection = connection;
        }
        if let Some(tags) = tags {
            ep.tags = tags;
        }
        if let Some(cert_path) = cert_path {
            ep.cert_path = cert_path;
        }
        if let Some(stacks_dir) = stacks_dir {
            ep.stacks_dir = stacks_dir;
        }

        // Reconnect if connection or certs changed
        if conn_changed || cert_changed {
            let client = docker::create_client(&ep.connection, ep.cert_path.as_deref())
                .map_err(|e| format!("Connection failed: {}", e))?;

            match tokio::time::timeout(std::time::Duration::from_secs(5), client.ping()).await {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => return Err(format!("Connection test failed: {}", e)),
                Err(_) => return Err("Connection timed out (5s)".to_string()),
            }

            self.clients.write().await.insert(id.to_string(), client);
            ep.status = EndpointStatus::Connected;
        }

        self.db.upsert_endpoint(&ep);
        Ok(())
    }

    /// Delete an endpoint. Refuses to delete "local".
    pub async fn delete(&self, id: &str) -> Result<String, String> {
        let ep = self
            .get(id)
            .await
            .ok_or_else(|| format!("Endpoint '{}' not found", id))?;

        if ep.name == "local" {
            return Err("Cannot delete the default 'local' endpoint".to_string());
        }

        self.db.delete_endpoint(id);
        self.clients.write().await.remove(id);
        Ok(ep.name)
    }

    /// Reconnect an endpoint (rebuild client).
    pub async fn reconnect(&self, id: &str) -> Result<(), String> {
        let ep = self
            .get(id)
            .await
            .ok_or_else(|| format!("Endpoint '{}' not found", id))?;

        let client = docker::create_client(&ep.connection, ep.cert_path.as_deref())
            .map_err(|e| format!("Connection failed: {}", e))?;

        match tokio::time::timeout(std::time::Duration::from_secs(5), client.ping()).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => return Err(format!("Reconnect failed: {}", e)),
            Err(_) => return Err("Reconnect timed out (5s)".to_string()),
        }

        self.clients.write().await.insert(id.to_string(), client);
        Ok(())
    }

    /// Get the default endpoint ID.
    pub async fn default_endpoint(&self) -> String {
        self.default_id.read().await.clone()
    }

    /// Direct access to the database (for audit, routes, users — not endpoints).
    pub fn db(&self) -> &Database {
        &self.db
    }
}
