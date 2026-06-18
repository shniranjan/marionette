use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Mutex;
use tracing;

use crate::models::{DockerEndpoint, EndpointStatus, Route, UserRole, UserSummary};

/// Central database for Marionette — users, endpoints, routes, audit log.
/// All tables share one SQLite database at the path configured by MARIONETTE_DB_PATH.
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Open or create the database and run schema migrations.
    pub fn new(db_path: &str) -> Self {
        if let Some(parent) = PathBuf::from(db_path).parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(db_path)
            .expect("Failed to open marionette database");

        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;"
        ).expect("Failed to set PRAGMAs");

        // ── Schema: create all tables ─────────────────────────────────
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS endpoints (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                connection TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'disconnected',
                tags TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                key_hash TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'viewer',
                active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS routes (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                target TEXT NOT NULL,
                auth_mode TEXT NOT NULL DEFAULT 'none',
                auth_value TEXT,
                tls INTEGER NOT NULL DEFAULT 0,
                active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS route_access (
                route_id TEXT NOT NULL REFERENCES routes(id) ON DELETE CASCADE,
                user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                PRIMARY KEY (route_id, user_id)
            );

            -- Audit log (may already exist from legacy AuditLog module)
            CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                action TEXT NOT NULL,
                endpoint_id TEXT NOT NULL,
                target TEXT NOT NULL,
                detail TEXT NOT NULL,
                admin_key_hash TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_log(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_log(action);
            CREATE INDEX IF NOT EXISTS idx_endpoints_name ON endpoints(name);
            CREATE INDEX IF NOT EXISTS idx_users_name ON users(name);
            CREATE INDEX IF NOT EXISTS idx_routes_path ON routes(path);"
        ).expect("Failed to create schema");

        tracing::info!("Database initialized at {}", db_path);

        Self {
            conn: Mutex::new(conn),
        }
    }

    // ── Endpoints ────────────────────────────────────────────────────

    /// Load all endpoints from the database.
    pub fn load_endpoints(&self) -> Vec<DockerEndpoint> {
        let conn = self.conn.lock().expect("DB lock poisoned");
        let mut stmt = conn
            .prepare("SELECT id, name, connection, status, tags, created_at FROM endpoints")
            .expect("Failed to prepare endpoint query");

        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let connection: String = row.get(2)?;
                let status_str: String = row.get(3)?;
                let tags_json: String = row.get(4)?;
                let _created: String = row.get(5)?;

                let status = match status_str.as_str() {
                    "connected" => EndpointStatus::Connected,
                    "error" => EndpointStatus::Error(String::new()),
                    _ => EndpointStatus::Disconnected,
                };
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

                Ok(DockerEndpoint {
                    id,
                    name,
                    connection,
                    status,
                    tags,
                })
            })
            .expect("Failed to query endpoints");

        rows.filter_map(|r| r.ok()).collect()
    }

    /// Insert or replace an endpoint in the database.
    pub fn upsert_endpoint(&self, ep: &DockerEndpoint) {
        let conn = self.conn.lock().expect("DB lock poisoned");
        let status_str = ep.status.to_string();
        let tags_json = serde_json::to_string(&ep.tags).unwrap_or_else(|_| "[]".to_string());
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO endpoints (id, name, connection, status, tags, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                name=excluded.name,
                connection=excluded.connection,
                status=excluded.status,
                tags=excluded.tags",
            rusqlite::params![ep.id, ep.name, ep.connection, status_str, tags_json, now],
        ).expect("Failed to upsert endpoint");
    }

    /// Delete an endpoint from the database.
    pub fn delete_endpoint(&self, id: &str) {
        let conn = self.conn.lock().expect("DB lock poisoned");
        conn.execute("DELETE FROM endpoints WHERE id = ?1", rusqlite::params![id])
            .expect("Failed to delete endpoint");
    }

    /// Seed default endpoints if the table is empty (first run).
    pub fn seed_endpoints(&self, endpoints: &[DockerEndpoint]) {
        let conn = self.conn.lock().expect("DB lock poisoned");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM endpoints", [], |row| row.get(0))
            .unwrap_or(0);
        if count == 0 && !endpoints.is_empty() {
            tracing::info!("Seeding {} endpoint(s) into database", endpoints.len());
            for ep in endpoints {
                let status_str = ep.status.to_string();
                let tags_json = serde_json::to_string(&ep.tags).unwrap_or_else(|_| "[]".to_string());
                let now = chrono::Utc::now().to_rfc3339();
                conn.execute(
                    "INSERT INTO endpoints (id, name, connection, status, tags, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![ep.id, ep.name, ep.connection, status_str, tags_json, now],
                ).ok();
            }
        }
    }

    // ── Users ────────────────────────────────────────────────────────

    /// Seed the admin user from MARIONETTE_KEY env var if no users exist.
    pub fn ensure_admin_user(&self, admin_key: &str) {
        let conn = self.conn.lock().expect("DB lock poisoned");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM users WHERE role = 'admin'", [], |row| row.get(0))
            .unwrap_or(0);
        if count == 0 {
            let id = uuid::Uuid::new_v4().to_string();
            let key_hash = hash_key(admin_key);
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO users (id, name, key_hash, role, active, created_at)
                 VALUES (?1, ?2, ?3, 'admin', 1, ?4)",
                rusqlite::params![id, "admin", key_hash, now],
            ).expect("Failed to seed admin user");
            tracing::info!("Seeded admin user from MARIONETTE_KEY");
        }
    }

    /// Validate an API key against stored users. Returns Some(role) if valid.
    pub fn validate_key(&self, key: &str) -> Option<UserRole> {
        let conn = self.conn.lock().expect("DB lock poisoned");
        let key_hash = hash_key(key);
        let mut stmt = conn
            .prepare("SELECT role FROM users WHERE key_hash = ?1 AND active = 1")
            .expect("Failed to prepare user query");
        let role_str: Option<String> = stmt
            .query_row(rusqlite::params![key_hash], |row| row.get(0))
            .ok();
        role_str.and_then(|r| UserRole::from_str(&r))
    }

    // ── Audit (delegates to existing audit_log table) ────────────────

    pub fn record_audit(
        &self,
        action: &str,
        endpoint_id: &str,
        target: &str,
        detail: &str,
        api_key: &str,
    ) {
        let conn = self.conn.lock().expect("DB lock poisoned");
        let key_hash = hash_key(api_key);
        let key_short = &key_hash[..12.min(key_hash.len())];
        let timestamp = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO audit_log (timestamp, action, endpoint_id, target, detail, admin_key_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![timestamp, action, endpoint_id, target, detail, key_short],
        ).ok();
    }

    // ── Routes ────────────────────────────────────────────────────────

    /// List all routes.
    pub fn list_routes(&self) -> Vec<Route> {
        let conn = self.conn.lock().expect("DB lock poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT id, path, target, auth_mode, auth_value, tls, active, created_at FROM routes",
            )
            .expect("Failed to prepare route query");
        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let path: String = row.get(1)?;
                let target: String = row.get(2)?;
                let auth_mode: String = row.get(3)?;
                let auth_value: Option<String> = row.get(4)?;
                let tls_int: i64 = row.get(5)?;
                let active_int: i64 = row.get(6)?;
                let created_at: String = row.get(7)?;
                Ok(Route {
                    id,
                    path,
                    target,
                    auth_mode,
                    auth_value,
                    tls: tls_int != 0,
                    active: active_int != 0,
                    created_at,
                })
            })
            .expect("Failed to query routes");
        rows.filter_map(|r| r.ok()).collect()
    }

    /// Get a single route by ID.
    pub fn get_route(&self, id: &str) -> Option<Route> {
        let conn = self.conn.lock().expect("DB lock poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT id, path, target, auth_mode, auth_value, tls, active, created_at FROM routes WHERE id = ?1",
            )
            .expect("Failed to prepare route query");
        stmt.query_row(rusqlite::params![id], |row| {
            let id: String = row.get(0)?;
            let path: String = row.get(1)?;
            let target: String = row.get(2)?;
            let auth_mode: String = row.get(3)?;
            let auth_value: Option<String> = row.get(4)?;
            let tls_int: i64 = row.get(5)?;
            let active_int: i64 = row.get(6)?;
            let created_at: String = row.get(7)?;
            Ok(Route {
                id,
                path,
                target,
                auth_mode,
                auth_value,
                tls: tls_int != 0,
                active: active_int != 0,
                created_at,
            })
        })
        .ok()
    }

    /// Insert or replace a route.
    pub fn upsert_route(&self, route: &Route) {
        let conn = self.conn.lock().expect("DB lock poisoned");
        let tls_int: i64 = if route.tls { 1 } else { 0 };
        let active_int: i64 = if route.active { 1 } else { 0 };
        conn.execute(
            "INSERT INTO routes (id, path, target, auth_mode, auth_value, tls, active, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                path=excluded.path,
                target=excluded.target,
                auth_mode=excluded.auth_mode,
                auth_value=excluded.auth_value,
                tls=excluded.tls,
                active=excluded.active",
            rusqlite::params![
                route.id,
                route.path,
                route.target,
                route.auth_mode,
                route.auth_value,
                tls_int,
                active_int,
                route.created_at,
            ],
        )
        .expect("Failed to upsert route");
    }

    /// Delete a route by ID.
    pub fn delete_route(&self, id: &str) {
        let conn = self.conn.lock().expect("DB lock poisoned");
        conn.execute("DELETE FROM routes WHERE id = ?1", rusqlite::params![id])
            .expect("Failed to delete route");
    }

    /// List user IDs that have access to a route.
    pub fn list_route_access(&self, route_id: &str) -> Vec<String> {
        let conn = self.conn.lock().expect("DB lock poisoned");
        let mut stmt = conn
            .prepare("SELECT user_id FROM route_access WHERE route_id = ?1")
            .expect("Failed to prepare route_access query");
        let rows = stmt
            .query_map(rusqlite::params![route_id], |row| row.get::<_, String>(0))
            .expect("Failed to query route_access");
        rows.filter_map(|r| r.ok()).collect()
    }

    /// Grant a user access to a route.
    pub fn grant_route_access(&self, route_id: &str, user_id: &str) {
        let conn = self.conn.lock().expect("DB lock poisoned");
        conn.execute(
            "INSERT OR IGNORE INTO route_access (route_id, user_id) VALUES (?1, ?2)",
            rusqlite::params![route_id, user_id],
        )
        .expect("Failed to grant route access");
    }

    /// Revoke a user's access to a route.
    pub fn revoke_route_access(&self, route_id: &str, user_id: &str) {
        let conn = self.conn.lock().expect("DB lock poisoned");
        conn.execute(
            "DELETE FROM route_access WHERE route_id = ?1 AND user_id = ?2",
            rusqlite::params![route_id, user_id],
        )
        .expect("Failed to revoke route access");
    }

    // ── Users (list) ─────────────────────────────────────────────────

    /// List all users.
    pub fn list_users(&self) -> Vec<UserSummary> {
        let conn = self.conn.lock().expect("DB lock poisoned");
        let mut stmt = conn
            .prepare("SELECT id, name, role, active, created_at FROM users")
            .expect("Failed to prepare users query");
        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let role: String = row.get(2)?;
                let active_int: i64 = row.get(3)?;
                let created_at: String = row.get(4)?;
                Ok(UserSummary {
                    id,
                    name,
                    role,
                    active: active_int != 0,
                    created_at,
                })
            })
            .expect("Failed to query users");
        rows.filter_map(|r| r.ok()).collect()
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn hash_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    format!("{:x}", hasher.finalize())
}

impl ToString for EndpointStatus {
    fn to_string(&self) -> String {
        match self {
            EndpointStatus::Connected => "connected".to_string(),
            EndpointStatus::Disconnected => "disconnected".to_string(),
            EndpointStatus::Error(_) => "error".to_string(),
        }
    }
}
