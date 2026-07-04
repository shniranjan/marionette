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
fn default_true() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsReadResponse {
    pub content: String,
    pub size_bytes: u64,
    pub encoding: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsWriteRequest {
    pub path: String,
    pub content: String,
    #[serde(default = "default_encoding")]
    pub encoding: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsWriteResponse {
    pub bytes_written: u64,
    pub path: String,
}

// ── Docker Lifecycle ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerStopRequest {
    pub container: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerStopResponse {
    pub container: String,
    pub stopped: bool,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerStartRequest {
    pub container: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerStartResponse {
    pub container: String,
    pub started: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerRestartRequest {
    pub container: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerRestartResponse {
    pub container: String,
    pub restarted: bool,
    pub duration_ms: u64,
}

// ── Docker Streaming ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerExecRequest {
    pub container: String,
    pub cmd: Vec<String>,
    #[serde(default = "default_true")]
    pub attach_stdout: bool,
    #[serde(default = "default_true")]
    pub attach_stderr: bool,
    #[serde(default)]
    pub attach_stdin: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerExecResponse {
    pub exit_code: i64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerLogsRequest {
    pub container: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tail: Option<u64>,
    #[serde(default)]
    pub follow: bool,
    #[serde(default)]
    pub timestamps: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    #[serde(default = "default_true")]
    pub stdout: bool,
    #[serde(default = "default_true")]
    pub stderr: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerLogsResponse {
    pub lines_streamed: u64,
    pub follow_ended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerStatsRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub containers: Option<Vec<String>>,
    #[serde(default = "default_true")]
    pub stream: bool,
    #[serde(default)]
    pub one_shot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerStatsSnapshot {
    pub container: String,
    pub timestamp: String,
    pub cpu_percent: f64,
    pub memory_usage_bytes: u64,
    pub memory_limit_bytes: u64,
    pub memory_percent: f64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub block_read_bytes: u64,
    pub block_write_bytes: u64,
    pub pids: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerStatsResponse {
    pub snapshots_sent: u64,
}

// ── Compose ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeUpRequest {
    pub project_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default = "default_true")]
    pub detach: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profiles: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<String>>,
    #[serde(default)]
    pub build: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeUpResponse {
    pub exit_code: i32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeDownRequest {
    pub project_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default)]
    pub volumes: bool,
    #[serde(default)]
    pub remove_orphans: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeDownResponse {
    pub exit_code: i32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeStopRequest {
    pub project_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeStopResponse {
    pub exit_code: i32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeLogsRequest {
    pub project_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tail: Option<u64>,
    #[serde(default)]
    pub follow: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeLogsResponse {
    pub lines_streamed: u64,
    pub follow_ended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeConfigRequest {
    pub project_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeConfigResponse {
    pub config_yaml: String,
}

// ── Image ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageEnsureRequest {
    pub image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageEnsureResponse {
    pub pulled: bool,
    pub image_id: String,
}

// ── Volume Transfer ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeTransferOutRequest {
    pub volume: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_relay: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeTransferOutResponse {
    pub transfer_id: String,
    pub bytes_transferred: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeTransferInRequest {
    pub transfer_id: String,
    pub volume: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeTransferInResponse {
    pub volume: String,
    pub bytes_received: u64,
}

// ── Relay Management ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayAuditRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayAuditResponse {
    pub entries: Vec<AuditEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub operation: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayUpdateResponse {
    pub updated: bool,
    pub new_version: String,
}

// ── Debug: Transfer ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RelayDebugTransferRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayDebugTransferResponse {
    pub active_transfers: u32,
    pub completed_transfers: u64,
}

// ── Debug: Events ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RelayDebugEventsRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayDebugEventsResponse {
    pub total_events: u64,
    pub recent_events: Vec<String>,
}

// ── Debug: Replay ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayDebugReplayRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_seq: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayDebugReplayResponse {
    pub replayed: u64,
    pub messages: Vec<serde_json::Value>,
}

// ── Debug: Config ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RelayDebugConfigRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayDebugConfigResponse {
    pub config: serde_json::Value,
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

    // ── Docker Lifecycle tests ────────────────────────────────

    #[test]
    fn docker_stop_roundtrip() {
        let req = DockerStopRequest {
            container: "nginx".into(),
            timeout_secs: Some(30),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: DockerStopRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.container, "nginx");
        assert_eq!(parsed.timeout_secs, Some(30));
    }

    #[test]
    fn docker_stop_optional_skipped() {
        let req = DockerStopRequest {
            container: "nginx".into(),
            timeout_secs: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("timeout_secs"));
    }

    #[test]
    fn docker_stop_response_roundtrip() {
        let resp = DockerStopResponse {
            container: "nginx".into(),
            stopped: true,
            duration_ms: 1523,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: DockerStopResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.stopped);
        assert_eq!(parsed.duration_ms, 1523);
    }

    #[test]
    fn docker_start_roundtrip() {
        let req = DockerStartRequest {
            container: "nginx".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("nginx"));
        let parsed: DockerStartRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.container, "nginx");
    }

    #[test]
    fn docker_start_response_roundtrip() {
        let resp = DockerStartResponse {
            container: "nginx".into(),
            started: true,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: DockerStartResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.started);
    }

    #[test]
    fn docker_restart_roundtrip() {
        let req = DockerRestartRequest {
            container: "nginx".into(),
            timeout_secs: Some(10),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: DockerRestartRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.container, "nginx");
        assert_eq!(parsed.timeout_secs, Some(10));
    }

    #[test]
    fn docker_restart_optional_skipped() {
        let req = DockerRestartRequest {
            container: "nginx".into(),
            timeout_secs: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("timeout_secs"));
    }

    #[test]
    fn docker_restart_response_roundtrip() {
        let resp = DockerRestartResponse {
            container: "nginx".into(),
            restarted: true,
            duration_ms: 2500,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: DockerRestartResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.restarted);
        assert_eq!(parsed.duration_ms, 2500);
    }

    // ── Docker Streaming tests ────────────────────────────────

    #[test]
    fn docker_exec_defaults() {
        let req = DockerExecRequest {
            container: "nginx".into(),
            cmd: vec!["echo".into(), "hello".into()],
            attach_stdout: true,
            attach_stderr: true,
            attach_stdin: false,
            workdir: None,
            env: None,
            user: None,
            timeout_secs: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"attach_stdout\":true"));
        assert!(json.contains("\"attach_stderr\":true"));
        assert!(json.contains("\"attach_stdin\":false"));
        assert!(!json.contains("workdir"));
        assert!(!json.contains("timeout_secs"));
    }

    #[test]
    fn docker_exec_roundtrip() {
        let req = DockerExecRequest {
            container: "nginx".into(),
            cmd: vec!["ls".into(), "-la".into()],
            attach_stdout: true,
            attach_stderr: true,
            attach_stdin: true,
            workdir: Some("/app".into()),
            env: None,
            user: Some("root".into()),
            timeout_secs: Some(60),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: DockerExecRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.container, "nginx");
        assert_eq!(parsed.cmd, vec!["ls", "-la"]);
        assert!(parsed.attach_stdout);
        assert!(parsed.attach_stdin);
        assert_eq!(parsed.user.as_deref(), Some("root"));
        assert_eq!(parsed.timeout_secs, Some(60));
    }

    #[test]
    fn docker_exec_env_serializes() {
        let mut env = std::collections::HashMap::new();
        env.insert("FOO".into(), "bar".into());
        let req = DockerExecRequest {
            container: "nginx".into(),
            cmd: vec!["env".into()],
            attach_stdout: true,
            attach_stderr: true,
            attach_stdin: false,
            workdir: None,
            env: Some(env),
            user: None,
            timeout_secs: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"FOO\":\"bar\""));
    }

    #[test]
    fn docker_exec_response_roundtrip() {
        let resp = DockerExecResponse {
            exit_code: 0,
            duration_ms: 125,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: DockerExecResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.exit_code, 0);
        assert_eq!(parsed.duration_ms, 125);
    }

    #[test]
    fn docker_logs_defaults() {
        let req = DockerLogsRequest {
            container: "nginx".into(),
            tail: None,
            follow: false,
            timestamps: false,
            since: None,
            until: None,
            stdout: true,
            stderr: true,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"stdout\":true"));
        assert!(json.contains("\"stderr\":true"));
        assert!(!json.contains("tail"));
        assert!(!json.contains("since"));
    }

    #[test]
    fn docker_logs_roundtrip() {
        let req = DockerLogsRequest {
            container: "nginx".into(),
            tail: Some(100),
            follow: true,
            timestamps: true,
            since: Some("2024-01-01T00:00:00Z".into()),
            until: None,
            stdout: true,
            stderr: false,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: DockerLogsRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tail, Some(100));
        assert!(parsed.follow);
        assert!(parsed.timestamps);
        assert!(!parsed.stderr);
    }

    #[test]
    fn docker_logs_response_roundtrip() {
        let resp = DockerLogsResponse {
            lines_streamed: 42,
            follow_ended: false,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: DockerLogsResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.lines_streamed, 42);
        assert!(!parsed.follow_ended);
    }

    #[test]
    fn docker_stats_defaults() {
        let req = DockerStatsRequest {
            containers: None,
            stream: true,
            one_shot: false,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"stream\":true"));
        assert!(!json.contains("containers"));
    }

    #[test]
    fn docker_stats_with_containers() {
        let req = DockerStatsRequest {
            containers: Some(vec!["nginx".into(), "redis".into()]),
            stream: false,
            one_shot: true,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("nginx"));
        assert!(json.contains("redis"));
        let parsed: DockerStatsRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.containers.unwrap().len(), 2);
    }

    #[test]
    fn docker_stats_snapshot_roundtrip() {
        let snap = DockerStatsSnapshot {
            container: "nginx".into(),
            timestamp: "2024-06-01T12:00:00Z".into(),
            cpu_percent: 12.5,
            memory_usage_bytes: 104857600,
            memory_limit_bytes: 536870912,
            memory_percent: 19.53,
            network_rx_bytes: 1024000,
            network_tx_bytes: 512000,
            block_read_bytes: 4096,
            block_write_bytes: 8192,
            pids: 5,
        };
        let json = serde_json::to_string(&snap).unwrap();
        let parsed: DockerStatsSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.container, "nginx");
        assert!((parsed.cpu_percent - 12.5).abs() < f64::EPSILON);
        assert_eq!(parsed.memory_usage_bytes, 104857600);
        assert_eq!(parsed.pids, 5);
    }

    #[test]
    fn docker_stats_response_roundtrip() {
        let resp = DockerStatsResponse {
            snapshots_sent: 150,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: DockerStatsResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.snapshots_sent, 150);
    }

    // ── Compose tests ─────────────────────────────────────────

    #[test]
    fn compose_up_defaults() {
        let req = ComposeUpRequest {
            project_dir: "/opt/stacks/nginx".into(),
            project_name: None,
            file: None,
            detach: true,
            env: None,
            profiles: None,
            services: None,
            build: false,
            timeout_secs: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"detach\":true"));
        assert!(!json.contains("project_name"));
        assert!(!json.contains("profiles"));
    }

    #[test]
    fn compose_up_roundtrip() {
        let req = ComposeUpRequest {
            project_dir: "/opt/stacks/nginx".into(),
            project_name: Some("myproject".into()),
            file: Some("docker-compose.yml".into()),
            detach: false,
            env: None,
            profiles: Some(vec!["debug".into()]),
            services: Some(vec!["web".into(), "db".into()]),
            build: true,
            timeout_secs: Some(120),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: ComposeUpRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.project_dir, "/opt/stacks/nginx");
        assert!(!parsed.detach);
        assert!(parsed.build);
        assert_eq!(parsed.services.unwrap().len(), 2);
    }

    #[test]
    fn compose_up_env_serializes() {
        let mut env = std::collections::HashMap::new();
        env.insert("API_KEY".into(), "abc123".into());
        let req = ComposeUpRequest {
            project_dir: "/opt/stacks/nginx".into(),
            project_name: None,
            file: None,
            detach: true,
            env: Some(env),
            profiles: None,
            services: None,
            build: false,
            timeout_secs: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"API_KEY\":\"abc123\""));
    }

    #[test]
    fn compose_up_response_roundtrip() {
        let resp = ComposeUpResponse {
            exit_code: 0,
            duration_ms: 5432,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ComposeUpResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.exit_code, 0);
        assert_eq!(parsed.duration_ms, 5432);
    }

    #[test]
    fn compose_down_defaults() {
        let req = ComposeDownRequest {
            project_dir: "/opt/stacks/nginx".into(),
            project_name: None,
            file: None,
            volumes: false,
            remove_orphans: false,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("project_name"));
        assert!(!json.contains("file"));
    }

    #[test]
    fn compose_down_roundtrip() {
        let req = ComposeDownRequest {
            project_dir: "/opt/stacks/nginx".into(),
            project_name: Some("myproject".into()),
            file: Some("docker-compose.override.yml".into()),
            volumes: true,
            remove_orphans: true,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: ComposeDownRequest = serde_json::from_str(&json).unwrap();
        assert!(parsed.volumes);
        assert!(parsed.remove_orphans);
        assert_eq!(parsed.file.as_deref(), Some("docker-compose.override.yml"));
    }

    #[test]
    fn compose_down_response_roundtrip() {
        let resp = ComposeDownResponse {
            exit_code: 0,
            duration_ms: 1200,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ComposeDownResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.exit_code, 0);
        assert_eq!(parsed.duration_ms, 1200);
    }

    #[test]
    fn compose_logs_roundtrip() {
        let req = ComposeLogsRequest {
            project_dir: "/opt/stacks/nginx".into(),
            project_name: None,
            file: None,
            tail: Some(50),
            follow: true,
            services: Some(vec!["web".into()]),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: ComposeLogsRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tail, Some(50));
        assert!(parsed.follow);
        assert_eq!(parsed.services.unwrap().len(), 1);
    }

    #[test]
    fn compose_logs_optional_skipped() {
        let req = ComposeLogsRequest {
            project_dir: "/opt/stacks/nginx".into(),
            project_name: None,
            file: None,
            tail: None,
            follow: false,
            services: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("tail"));
        assert!(!json.contains("services"));
    }

    #[test]
    fn compose_logs_response_roundtrip() {
        let resp = ComposeLogsResponse {
            lines_streamed: 200,
            follow_ended: true,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ComposeLogsResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.lines_streamed, 200);
        assert!(parsed.follow_ended);
    }

    #[test]
    fn compose_config_roundtrip() {
        let req = ComposeConfigRequest {
            project_dir: "/opt/stacks/nginx".into(),
            project_name: Some("nginx-stack".into()),
            file: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: ComposeConfigRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.project_dir, "/opt/stacks/nginx");
        assert_eq!(parsed.project_name.as_deref(), Some("nginx-stack"));
        assert!(parsed.file.is_none());
    }

    #[test]
    fn compose_config_optional_skipped() {
        let req = ComposeConfigRequest {
            project_dir: "/opt/stacks/nginx".into(),
            project_name: None,
            file: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("project_name"));
        assert!(!json.contains("file"));
    }

    #[test]
    fn compose_config_response_roundtrip() {
        let resp = ComposeConfigResponse {
            config_yaml: "version: '3'\nservices:\n  web:\n    image: nginx\n".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ComposeConfigResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.config_yaml.contains("nginx"));
    }

    // ── FsWrite tests ────────────────────────────────────────

    #[test]
    fn fs_write_roundtrip() {
        let req = FsWriteRequest {
            path: "/opt/stacks/nginx/compose.yml".into(),
            content: "version: '3'\n".into(),
            encoding: "utf8".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: FsWriteRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.path, "/opt/stacks/nginx/compose.yml");
        assert_eq!(parsed.content, "version: '3'\n");
        assert_eq!(parsed.encoding, "utf8");
    }

    #[test]
    fn fs_write_default_encoding() {
        let req = FsWriteRequest {
            path: "/tmp/test.txt".into(),
            content: "hello".into(),
            encoding: "utf8".into(),
        };
        assert_eq!(req.encoding, "utf8");
    }

    #[test]
    fn fs_write_response_roundtrip() {
        let resp = FsWriteResponse {
            bytes_written: 42,
            path: "/tmp/test.txt".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: FsWriteResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.bytes_written, 42);
        assert_eq!(parsed.path, "/tmp/test.txt");
    }

    // ── Image: Ensure tests ──────────────────────────────────

    #[test]
    fn image_ensure_roundtrip() {
        let req = ImageEnsureRequest {
            image: "nginx".into(),
            tag: Some("latest".into()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: ImageEnsureRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.image, "nginx");
        assert_eq!(parsed.tag.as_deref(), Some("latest"));
    }

    #[test]
    fn image_ensure_optional_skipped() {
        let req = ImageEnsureRequest {
            image: "redis".into(),
            tag: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("tag"));
    }

    #[test]
    fn image_ensure_response_roundtrip() {
        let resp = ImageEnsureResponse {
            pulled: true,
            image_id: "sha256:abc123".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ImageEnsureResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.pulled);
        assert_eq!(parsed.image_id, "sha256:abc123");
    }

    // ── Volume Transfer tests ────────────────────────────────

    #[test]
    fn volume_transfer_out_roundtrip() {
        let req = VolumeTransferOutRequest {
            volume: "nginx-data".into(),
            target_relay: Some("relay-2".into()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: VolumeTransferOutRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.volume, "nginx-data");
        assert_eq!(parsed.target_relay.as_deref(), Some("relay-2"));
    }

    #[test]
    fn volume_transfer_out_optional_skipped() {
        let req = VolumeTransferOutRequest {
            volume: "nginx-data".into(),
            target_relay: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("target_relay"));
    }

    #[test]
    fn volume_transfer_out_response_roundtrip() {
        let resp = VolumeTransferOutResponse {
            transfer_id: "xfer-001".into(),
            bytes_transferred: 1048576,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: VolumeTransferOutResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.transfer_id, "xfer-001");
        assert_eq!(parsed.bytes_transferred, 1048576);
    }

    #[test]
    fn volume_transfer_in_roundtrip() {
        let req = VolumeTransferInRequest {
            transfer_id: "xfer-001".into(),
            volume: "nginx-data".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: VolumeTransferInRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.transfer_id, "xfer-001");
        assert_eq!(parsed.volume, "nginx-data");
    }

    #[test]
    fn volume_transfer_in_response_roundtrip() {
        let resp = VolumeTransferInResponse {
            volume: "nginx-data".into(),
            bytes_received: 1048576,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: VolumeTransferInResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.volume, "nginx-data");
        assert_eq!(parsed.bytes_received, 1048576);
    }

    // ── Relay Audit tests ────────────────────────────────────

    #[test]
    fn relay_audit_roundtrip() {
        let req = RelayAuditRequest {
            limit: Some(50),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: RelayAuditRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.limit, Some(50));
    }

    #[test]
    fn relay_audit_optional_skipped() {
        let req = RelayAuditRequest { limit: None };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("limit"));
    }

    #[test]
    fn relay_audit_response_roundtrip() {
        let resp = RelayAuditResponse {
            entries: vec![
                AuditEntry {
                    timestamp: "2024-06-01T12:00:00Z".into(),
                    operation: "fs.read".into(),
                    status: "success".into(),
                },
                AuditEntry {
                    timestamp: "2024-06-01T12:01:00Z".into(),
                    operation: "docker.ps".into(),
                    status: "success".into(),
                },
            ],
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: RelayAuditResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[0].operation, "fs.read");
        assert_eq!(parsed.entries[1].timestamp, "2024-06-01T12:01:00Z");
    }

    // ── Relay Update tests ───────────────────────────────────

    #[test]
    fn relay_update_roundtrip() {
        let req = RelayUpdateRequest {
            version: Some("0.2.0".into()),
            url: Some("https://example.com/relay.tar.gz".into()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: RelayUpdateRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version.as_deref(), Some("0.2.0"));
        assert_eq!(parsed.url.as_deref(), Some("https://example.com/relay.tar.gz"));
    }

    #[test]
    fn relay_update_optional_skipped() {
        let req = RelayUpdateRequest {
            version: None,
            url: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("version"));
        assert!(!json.contains("url"));
    }

    #[test]
    fn relay_update_response_roundtrip() {
        let resp = RelayUpdateResponse {
            updated: false,
            new_version: "0.2.0".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: RelayUpdateResponse = serde_json::from_str(&json).unwrap();
        assert!(!parsed.updated);
        assert_eq!(parsed.new_version, "0.2.0");
    }

    // ── Debug: Transfer tests ────────────────────────────────

    #[test]
    fn relay_debug_transfer_request_default() {
        let req = RelayDebugTransferRequest::default();
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn relay_debug_transfer_response_roundtrip() {
        let resp = RelayDebugTransferResponse {
            active_transfers: 3,
            completed_transfers: 150,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: RelayDebugTransferResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.active_transfers, 3);
        assert_eq!(parsed.completed_transfers, 150);
    }

    // ── Debug: Events tests ──────────────────────────────────

    #[test]
    fn relay_debug_events_request_default() {
        let req = RelayDebugEventsRequest::default();
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn relay_debug_events_response_roundtrip() {
        let resp = RelayDebugEventsResponse {
            total_events: 1000,
            recent_events: vec!["event-1".into(), "event-2".into()],
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: RelayDebugEventsResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.total_events, 1000);
        assert_eq!(parsed.recent_events.len(), 2);
    }

    // ── Debug: Replay tests ──────────────────────────────────

    #[test]
    fn relay_debug_replay_roundtrip() {
        let req = RelayDebugReplayRequest {
            from_seq: Some(42),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: RelayDebugReplayRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.from_seq, Some(42));
    }

    #[test]
    fn relay_debug_replay_optional_skipped() {
        let req = RelayDebugReplayRequest { from_seq: None };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("from_seq"));
    }

    #[test]
    fn relay_debug_replay_response_roundtrip() {
        let resp = RelayDebugReplayResponse {
            replayed: 10,
            messages: vec![
                serde_json::json!({"op": "fs.read", "status": "ok"}),
                serde_json::json!({"op": "docker.ps", "status": "ok"}),
            ],
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: RelayDebugReplayResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.replayed, 10);
        assert_eq!(parsed.messages.len(), 2);
    }

    // ── Debug: Config tests ──────────────────────────────────

    #[test]
    fn relay_debug_config_request_default() {
        let req = RelayDebugConfigRequest::default();
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn relay_debug_config_response_roundtrip() {
        let resp = RelayDebugConfigResponse {
            config: serde_json::json!({"log_level": "debug", "port": 8080}),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: RelayDebugConfigResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.config["log_level"], "debug");
        assert_eq!(parsed.config["port"], 8080);
    }
}
