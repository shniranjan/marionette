//! Type-safe request/response payload types for core operations.
//!
//! Each operation has its own request and response struct. This ensures
//! compile-time type safety when both relay-agent and marionette-core
//! depend on this crate.
//!
//! Stage 0 implements the most commonly used payload types. Additional
//! payloads are added as new operations are implemented.

use serde::{Deserialize, Serialize};

// ── Health ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PingRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PongResponse {
    pub uptime_secs: u64,
    pub docker_version: String,
    pub arch: String,
    pub os: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay_version: Option<String>,
}

// ── Docker ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerPsRequest {
    #[serde(default)]
    pub all: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerSummary {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerPsResponse {
    pub containers: Vec<ContainerSummary>,
}

// ── Host ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostInfoRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostInfoResponse {
    pub relay_version: String,
    pub hostname: String,
    pub docker: DockerInfo,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerInfo {
    pub version: String,
    pub api_version: String,
    pub architecture: String,
    pub os: String,
    pub cpus: u32,
    pub memory_bytes: u64,
    pub driver: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swarm_active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compose_version: Option<String>,
}

// ── Error ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error_code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

// ── Debug: State ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RelayDebugStateRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayDebugStateResponse {
    pub state: String,
    pub uptime_secs: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_age_secs: Option<u64>,
    pub active_requests: u32,
    pub locked_volumes: Vec<String>,
    pub locked_projects: Vec<String>,
}

// ── Debug: Stats ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RelayDebugStatsRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayDebugStatsResponse {
    pub total_calls: u64,
    pub total_errors: u64,
    pub uptime_secs: u64,
}

// ── File System ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsReadRequest {
    pub path: String,
    #[serde(default = "default_encoding")]
    pub encoding: String,
}

fn default_encoding() -> String { "utf8".into() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsReadResponse {
    pub content: String,
    pub size_bytes: u64,
    pub encoding: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_request_defaults_to_empty() {
        let req = PingRequest::default();
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn pong_roundtrip() {
        let pong = PongResponse {
            uptime_secs: 3600,
            docker_version: "26.1.3".into(),
            arch: "aarch64".into(),
            os: "linux".into(),
            relay_version: Some("0.1.0".into()),
        };
        let json = serde_json::to_string(&pong).unwrap();
        let parsed: PongResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.uptime_secs, 3600);
        assert_eq!(parsed.relay_version.as_deref(), Some("0.1.0"));
        assert_eq!(parsed.arch, "aarch64");
    }

    #[test]
    fn pong_without_optional() {
        let pong = PongResponse {
            uptime_secs: 0,
            docker_version: "26.1.3".into(),
            arch: "x86_64".into(),
            os: "linux".into(),
            relay_version: None,
        };
        let json = serde_json::to_string(&pong).unwrap();
        assert!(!json.contains("relay_version"));
    }

    #[test]
    fn docker_ps_request_defaults() {
        let req = DockerPsRequest { all: false, filter: None, limit: None };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""all":false"#));
        assert!(!json.contains("filter"));
        assert!(!json.contains("limit"));
    }

    #[test]
    fn docker_ps_request_with_filter() {
        let req = DockerPsRequest { all: true, filter: Some("name=nginx".into()), limit: Some(10) };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("name=nginx"));
        assert!(json.contains("10"));
    }

    #[test]
    fn host_info_response_roundtrip() {
        let info = HostInfoResponse {
            relay_version: "0.1.0".into(),
            hostname: "docker-host-59".into(),
            docker: DockerInfo {
                version: "26.1.3".into(),
                api_version: "1.45".into(),
                architecture: "aarch64".into(),
                os: "linux".into(),
                cpus: 4,
                memory_bytes: 8589934592,
                driver: "overlay2".into(),
                swarm_active: Some(false),
                compose_version: Some("v2.27.0".into()),
            },
            uptime_secs: 86400,
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: HostInfoResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.hostname, "docker-host-59");
        assert_eq!(parsed.docker.cpus, 4);
        assert_eq!(parsed.docker.compose_version.as_deref(), Some("v2.27.0"));
    }

    #[test]
    fn fs_read_default_encoding() {
        let req = FsReadRequest { path: "/opt/stacks/nginx/compose.yml".into(), encoding: "utf8".into() };
        assert_eq!(req.encoding, "utf8");
    }

    #[test]
    fn debug_state_response_roundtrip() {
        let state = RelayDebugStateResponse {
            state: "READY".into(),
            uptime_secs: 3600,
            session_age_secs: Some(1800),
            active_requests: 2,
            locked_volumes: vec!["nginx-data".into()],
            locked_projects: vec![],
        };
        let json = serde_json::to_string(&state).unwrap();
        let parsed: RelayDebugStateResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.state, "READY");
        assert_eq!(parsed.locked_volumes.len(), 1);
    }
}
