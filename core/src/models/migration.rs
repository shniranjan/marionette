use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationPlan {
    pub migration_id: String,
    pub source_endpoint: String,
    pub target_endpoint: String,
    pub container_name: String,
    pub container_id: String,
    pub image: String,
    pub volumes: Vec<MigrationVolume>,
    pub db_connections: Vec<DbConnection>,
    pub commands: Vec<String>,
    pub warnings: Vec<String>,
    pub estimated_size_bytes: u64,
    pub compressed: bool,
    pub env_vars: Vec<String>,
    pub has_compose_secrets: bool,
    pub start_on_target: bool,
    pub verify_connectivity: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationVolume {
    pub name: String,
    pub driver: String,
    pub driver_category: String,
    pub size_bytes: Option<u64>,
    pub shared: bool,
    pub transfer_method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbConnection {
    pub var_name: String,
    pub value_masked: String,
    pub target_container: Option<String>,
    pub on_same_host: bool,
    pub will_break: bool,
    pub fix_suggestion: String,
}

/// Result of executing a single migration command via shell.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionResult {
    pub index: usize,
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}
