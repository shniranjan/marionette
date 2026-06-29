use serde::{Deserialize, Serialize};

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
    pub resource: String,
}
