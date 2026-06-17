use serde::{Deserialize, Serialize};
use serde::ser::SerializeMap;
use std::collections::HashMap;

// ── Docker Endpoint ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerEndpoint {
    pub id: String,
    pub name: String,
    pub connection: String,
    #[serde(skip)]
    pub status: EndpointStatus,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub enum EndpointStatus {
    Connected,
    #[default]
    Disconnected,
    Error(String),
}

impl Serialize for EndpointStatus {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            EndpointStatus::Connected => s.serialize_str("connected"),
            EndpointStatus::Disconnected => s.serialize_str("disconnected"),
            EndpointStatus::Error(e) => {
                let mut map = s.serialize_map(Some(2))?;
                map.serialize_entry("status", "error")?;
                map.serialize_entry("message", e)?;
                map.end()
            }
        }
    }
}

// ── Container ────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerSummary {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub created: i64,
    pub ports: Vec<PortMapping>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortMapping {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    pub private_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_port: Option<u16>,
    #[serde(rename = "type")]
    pub port_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerDetail {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub created: String,
    pub platform: Option<String>,
    pub command: Option<String>,
    pub env: Vec<String>,
    pub ports: Vec<PortMapping>,
    pub mounts: Vec<Mount>,
    pub networks: Vec<ContainerNetwork>,
    pub restart_policy: Option<String>,
    pub labels: HashMap<String, String>,
    pub stack: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mount {
    #[serde(rename = "type")]
    pub mount_type: String,
    pub source: String,
    pub destination: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerNetwork {
    pub name: String,
    pub ip_address: Option<String>,
    pub gateway: Option<String>,
}

// ── Image ────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSummary {
    pub id: String,
    pub repo_tags: Vec<String>,
    pub size: i64,
    pub created: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDetail {
    pub id: String,
    pub repo_tags: Vec<String>,
    pub size: i64,
    pub created: String,
    pub os: Option<String>,
    pub architecture: Option<String>,
    pub layers: Vec<ImageLayer>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageLayer {
    pub id: String,
    pub size: i64,
    pub comment: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagePullRequest {
    pub image: String,
    #[serde(default)]
    pub tag: Option<String>,
}

// ── Volume ───────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeSummary {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_human: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeDeepInspection {
    pub name: String,
    pub driver: String,
    pub driver_type: String,
    pub driver_category: String,
    pub migration_advice: String,
    pub mountpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_human: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    pub used_by: Vec<String>,
    pub shared: bool,
    pub options: HashMap<String, String>,
    pub options_sanitized: HashMap<String, String>,
    pub labels: HashMap<String, String>,
    pub scope: String,
    pub needs_chown: bool,
    pub mount_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeCreateRequest {
    pub name: String,
    #[serde(default)]
    pub driver: Option<String>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(default)]
    pub options: HashMap<String, String>,
}

// ── Network ──────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSummary {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
    pub internal: bool,
    pub containers: Vec<String>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkCreateRequest {
    pub name: String,
    #[serde(default)]
    pub driver: Option<String>,
    #[serde(default)]
    pub internal: bool,
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConnectRequest {
    pub container: String,
}

// ── Stack ────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackSummary {
    pub name: String,
    pub services: usize,
    pub status: String,
    pub file: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackSaveRequest {
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackCreateRequest {
    pub name: String,
    pub content: String,
}

// ── System ───────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub containers: i64,
    pub containers_running: i64,
    pub containers_paused: i64,
    pub containers_stopped: i64,
    pub images: i64,
    pub volumes: i64,
    pub networks: i64,
    pub driver: Option<String>,
    pub kernel_version: Option<String>,
    pub os: Option<String>,
    pub architecture: Option<String>,
    pub cpu_count: Option<u64>,
    pub memory_bytes: Option<i64>,
    pub docker_version: Option<String>,
    pub server_time: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PruneRequest {
    #[serde(default)]
    pub resource: String, // containers, images, volumes, networks
}

// ── Audit ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub timestamp: String,
    pub action: String,
    pub endpoint_id: String,
    pub target: String,
    pub detail: String,
    pub admin_key_hash: String,
}

// ── Migration ──────────────────────────────────────────────────

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

// ── Common ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointQuery {
    #[serde(default)]
    pub endpoint: Option<String>,
}
