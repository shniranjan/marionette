//! All error codes as defined in specs/error-codes.md.
//!
//! Error codes use dot-namespaced format (e.g., `AUTH.EXPIRED`, `DOCKER.CONTAINER_NOT_FOUND`).
//! Each code maps to an HTTP-analog status for human readability.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Error response payload carrying the error code, human-readable message, and optional details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub error_code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl ErrorPayload {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { error_code: code, message: message.into(), details: None }
    }

    pub fn with_details(code: ErrorCode, message: impl Into<String>, details: Value) -> Self {
        Self { error_code: code, message: message.into(), details: Some(details) }
    }
}

/// All 31 error codes in dot-namespaced format.
///
/// Serialized with serde rename to produce the exact wire format:
/// e.g., `ErrorCode::AuthExpired` → `"AUTH.EXPIRED"`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCode {
    // Auth errors
    #[serde(rename = "AUTH.EXPIRED")]
    AuthExpired,
    #[serde(rename = "AUTH.INVALID")]
    AuthInvalid,
    #[serde(rename = "AUTH.OPERATION_DENIED")]
    AuthOperationDenied,

    // Docker errors
    #[serde(rename = "DOCKER.DAEMON_UNREACHABLE")]
    DockerDaemonUnreachable,
    #[serde(rename = "DOCKER.CONTAINER_NOT_FOUND")]
    DockerContainerNotFound,
    #[serde(rename = "DOCKER.CONTAINER_NOT_RUNNING")]
    DockerContainerNotRunning,
    #[serde(rename = "DOCKER.EXEC_DENIED")]
    DockerExecDenied,

    // Compose errors
    #[serde(rename = "COMPOSE.FILE_NOT_FOUND")]
    ComposeFileNotFound,
    #[serde(rename = "COMPOSE.DIR_NOT_ALLOWED")]
    ComposeDirNotAllowed,
    #[serde(rename = "COMPOSE.VALIDATION_ERROR")]
    ComposeValidationError,
    #[serde(rename = "COMPOSE.NOT_AVAILABLE")]
    ComposeNotAvailable,

    // Filesystem errors
    #[serde(rename = "FS.PATH_NOT_ALLOWED")]
    FsPathNotAllowed,
    #[serde(rename = "FS.FILE_NOT_FOUND")]
    FsFileNotFound,
    #[serde(rename = "FS.PERMISSION_DENIED")]
    FsPermissionDenied,
    #[serde(rename = "FS.DISK_FULL")]
    FsDiskFull,

    // Volume errors
    #[serde(rename = "VOLUME.NOT_FOUND")]
    VolumeNotFound,
    #[serde(rename = "VOLUME.TRANSFER_FAILED")]
    VolumeTransferFailed,
    #[serde(rename = "VOLUME.CHECKSUM_MISMATCH")]
    VolumeChecksumMismatch,

    // Image errors
    #[serde(rename = "IMAGE.REGISTRY_NOT_TRUSTED")]
    ImageRegistryNotTrusted,
    #[serde(rename = "IMAGE.NOT_FOUND")]
    ImageNotFound,
    #[serde(rename = "IMAGE.PLATFORM_UNAVAILABLE")]
    ImagePlatformUnavailable,

    // Protocol-level
    #[serde(rename = "TIMEOUT")]
    Timeout,
    #[serde(rename = "INVALID_MESSAGE")]
    InvalidMessage,
    #[serde(rename = "RATE_LIMITED")]
    RateLimited,
    #[serde(rename = "INTERNAL_ERROR")]
    InternalError,

    // Relay debug errors
    #[serde(rename = "RELAY.DEBUG_DISABLED")]
    RelayDebugDisabled,
    #[serde(rename = "RELAY.TRANSFER_NOT_FOUND")]
    RelayTransferNotFound,
    #[serde(rename = "RELAY.REPLAY_TIMEOUT")]
    RelayReplayTimeout,
    #[serde(rename = "RELAY.EVENT_STREAM_FULL")]
    RelayEventStreamFull,
    #[serde(rename = "RELAY.STATS_NOT_READY")]
    RelayStatsNotReady,
    #[serde(rename = "RELAY.INVALID_FILTER")]
    RelayInvalidFilter,
}

impl ErrorCode {
    /// Parse an error code string into an ErrorCode.
    pub fn from_str(s: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
    }

    /// Convert back to wire format string.
    pub fn to_str(&self) -> String {
        serde_json::to_string(self).unwrap().trim_matches('"').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_31_error_codes_serialize_roundtrip() {
        let codes = [
            ErrorCode::AuthExpired,
            ErrorCode::AuthInvalid,
            ErrorCode::AuthOperationDenied,
            ErrorCode::DockerDaemonUnreachable,
            ErrorCode::DockerContainerNotFound,
            ErrorCode::DockerContainerNotRunning,
            ErrorCode::DockerExecDenied,
            ErrorCode::ComposeFileNotFound,
            ErrorCode::ComposeDirNotAllowed,
            ErrorCode::ComposeValidationError,
            ErrorCode::ComposeNotAvailable,
            ErrorCode::FsPathNotAllowed,
            ErrorCode::FsFileNotFound,
            ErrorCode::FsPermissionDenied,
            ErrorCode::FsDiskFull,
            ErrorCode::VolumeNotFound,
            ErrorCode::VolumeTransferFailed,
            ErrorCode::VolumeChecksumMismatch,
            ErrorCode::ImageRegistryNotTrusted,
            ErrorCode::ImageNotFound,
            ErrorCode::ImagePlatformUnavailable,
            ErrorCode::Timeout,
            ErrorCode::InvalidMessage,
            ErrorCode::RateLimited,
            ErrorCode::InternalError,
            ErrorCode::RelayDebugDisabled,
            ErrorCode::RelayTransferNotFound,
            ErrorCode::RelayReplayTimeout,
            ErrorCode::RelayEventStreamFull,
            ErrorCode::RelayStatsNotReady,
            ErrorCode::RelayInvalidFilter,
        ];
        for code in &codes {
            let json = serde_json::to_string(code).unwrap();
            let parsed: ErrorCode = serde_json::from_str(&json).unwrap();
            assert_eq!(code, &parsed);
        }
    }

    #[test]
    fn auth_expired_serializes_correctly() {
        let code = ErrorCode::AuthExpired;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, r#""AUTH.EXPIRED""#);
    }

    #[test]
    fn error_payload_roundtrip() {
        let payload = ErrorPayload::with_details(
            ErrorCode::DockerContainerNotFound,
            "Container 'nginx-prod' not found on this host",
            serde_json::json!({"container": "nginx-prod"}),
        );
        let json = serde_json::to_string(&payload).unwrap();
        let parsed: ErrorPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.error_code, ErrorCode::DockerContainerNotFound);
        assert_eq!(parsed.message, "Container 'nginx-prod' not found on this host");
        assert!(parsed.details.is_some());
    }
}
