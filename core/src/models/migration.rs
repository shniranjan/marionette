use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    // Wave 1: Strategy fields
    #[serde(default)]
    pub compression: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_options: Option<PostOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_overrides: Option<HashMap<String, VolumeOverride>>,
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
    pub default_transfer_method: String,
    pub options: Option<serde_json::Value>,
    // Wave 1: Volume target management fields
    #[serde(default)]
    pub target_name: Option<String>,
    #[serde(default)]
    pub target_path: Option<String>,
    #[serde(default)]
    pub target_driver: Option<String>,
    #[serde(default)]
    pub skip: bool,
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

// ── Wave 1: Strategy structs ──────────────────────────────────────

/// Post-migration options set by the user in the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PostOptions {
    #[serde(default)]
    pub start_on_target: bool,
    #[serde(default)]
    pub verify_connectivity: bool,
    #[serde(default)]
    pub remove_from_source: bool,
    #[serde(default)]
    pub rotate_credentials: bool,
}

/// Per-volume override for transfer settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VolumeOverride {
    #[serde(default)]
    pub transfer_method: Option<String>,
    #[serde(default)]
    pub custom_path: Option<String>,
    #[serde(default)]
    pub target_name: Option<String>,
    #[serde(default)]
    pub target_path: Option<String>,
    #[serde(default)]
    pub target_driver: Option<String>,
    #[serde(default)]
    pub skip: bool,
}

/// Resolution for a DB connection that would break during migration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionResolution {
    pub action: String,
    pub resolved: bool,
}
