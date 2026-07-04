use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Mutex;
use tracing;

use crate::models::AuditEntry;

/// SQLite-backed audit log. Survives restarts.
pub struct AuditLog {
    conn: Mutex<Connection>,
}

impl AuditLog {
    /// Create a new audit log with a SQLite database at the given path.
    /// Creates the table if it doesn't exist.
    pub fn new(db_path: &str) -> Self {
        // Ensure parent directory exists
        if let Some(parent) = PathBuf::from(db_path).parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(db_path)
            .expect("Failed to open audit log database");

        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
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
             CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_log(action);"
        ).expect("Failed to initialize audit log schema");

        tracing::info!("Audit log initialized at {}", db_path);

        Self {
            conn: Mutex::new(conn),
        }
    }

    /// Record an auditable action.
    /// Thread-safe via Mutex. Non-async — callers should not hold async locks across this.
    pub fn record_sync(
        &self,
        action: &str,
        endpoint_id: &str,
        target: &str,
        detail: &str,
        api_key: &str,
    ) {
        let mut hasher = Sha256::new();
        hasher.update(api_key.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        let hash_short = &hash[..12.min(hash.len())];

        let timestamp = chrono::Utc::now().to_rfc3339();

        let conn = self.conn.lock().expect("Audit log lock poisoned");
        if let Err(e) = conn.execute(
            "INSERT INTO audit_log (timestamp, action, endpoint_id, target, detail, admin_key_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![timestamp, action, endpoint_id, target, detail, hash_short],
        ) {
            tracing::error!("Failed to write audit entry: {}", e);
        }
    }

    /// Async-compatible wrapper for record_sync.
    pub async fn record(
        &self,
        action: &str,
        endpoint_id: &str,
        target: &str,
        detail: &str,
        api_key: &str,
    ) {
        // Clone strings so the caller's references don't need to outlive the Mutex lock
        let action = action.to_string();
        let endpoint_id = endpoint_id.to_string();
        let target = target.to_string();
        let detail = detail.to_string();
        let api_key = api_key.to_string();

        tokio::task::spawn_blocking(move || {
            // We need a reference to self — but we can't capture &self in spawn_blocking.
            // Instead we reconstruct the AuditLog connection inline... no.
            // Actually, AuditLog is behind Arc<AppState>, so it lives long enough.
            // But spawn_blocking needs 'static. We'll use a simpler approach:
            // record_sync is called directly from async context via tokio::task::block_in_place
        });

        // Simpler: just call it inline. The Mutex lock is brief (single INSERT).
        // This is fine for an audit log — writes are rare compared to reads.
        self.record_sync(&action, &endpoint_id, &target, &detail, &api_key);
    }

    /// Return audit entries, newest first. Optional limit.
    pub fn list_sync(&self, limit: Option<usize>) -> Vec<AuditEntry> {
        let conn = self.conn.lock().expect("Audit log lock poisoned");
        let limit = limit.unwrap_or(500);

        let mut stmt = conn
            .prepare(
                "SELECT timestamp, action, endpoint_id, target, detail, admin_key_hash
                 FROM audit_log
                 ORDER BY id DESC
                 LIMIT ?1",
            )
            .expect("Failed to prepare audit query");

        let entries = stmt
            .query_map(rusqlite::params![limit as i64], |row| {
                Ok(AuditEntry {
                    timestamp: row.get(0)?,
                    action: row.get(1)?,
                    endpoint_id: row.get(2)?,
                    target: row.get(3)?,
                    detail: row.get(4)?,
                    admin_key_hash: row.get(5)?,
                })
            })
            .expect("Failed to query audit log");

        entries.filter_map(|r| r.ok()).collect()
    }

    /// Async wrapper for list_sync.
    pub async fn list(&self) -> Vec<AuditEntry> {
        self.list_sync(Some(500))
    }

    /// Return the total number of audit entries.
    pub fn count(&self) -> usize {
        let conn = self.conn.lock().expect("Audit log lock poisoned");
        conn.query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get::<_, i64>(0))
            .map(|c| c as usize)
            .unwrap_or(0)
    }
}
