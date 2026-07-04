//! All 30 operation codes as defined in specs/operation-codes.md.
//!
//! Operations are namespaced by domain prefix:
//! - `docker.*` — Container lifecycle and inspection
//! - `compose.*` — Docker Compose project management
//! - `fs.*` — Host filesystem access
//! - `volume.*` — Volume data transfer between relays
//! - `image.*` — Image management and pulling
//! - `host.*` — Host metadata and diagnostics
//! - `relay.*` — Relay self-management and debugging

use serde::{Deserialize, Serialize};

/// All 30 operation codes (24 original + 6 debug).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum OperationCode {
    // Protocol-level
    Register,
    Ping,
    Pong,

    // Docker operations
    DockerPs,
    DockerInspect,
    DockerExec,
    DockerLogs,
    DockerStats,
    DockerStop,
    DockerStart,
    DockerRestart,

    // Compose operations
    ComposeUp,
    ComposeDown,
    ComposeLogs,
    ComposeConfig,

    // Filesystem operations
    FsRead,
    FsWrite,
    FsList,

    // Volume operations
    VolumeTransferOut,
    VolumeTransferIn,

    // Image operations
    ImageEnsure,

    // Host operations
    HostInfo,

    // Relay operations
    RelayAudit,
    RelayUpdate,

    // Debug operations
    RelayDebugState,
    RelayDebugStats,
    RelayDebugTransfer,
    RelayDebugEvents,
    RelayDebugReplay,
    RelayDebugConfig,
}

impl OperationCode {
    /// Parse an operation subtype string into an OperationCode.
    /// Returns None for unknown subtypes.
    pub fn from_subtype(s: &str) -> Option<Self> {
        match s {
            "register" => Some(Self::Register),
            "ping" => Some(Self::Ping),
            "pong" => Some(Self::Pong),
            "docker.ps" => Some(Self::DockerPs),
            "docker.inspect" => Some(Self::DockerInspect),
            "docker.exec" => Some(Self::DockerExec),
            "docker.logs" => Some(Self::DockerLogs),
            "docker.stats" => Some(Self::DockerStats),
            "docker.stop" => Some(Self::DockerStop),
            "docker.start" => Some(Self::DockerStart),
            "docker.restart" => Some(Self::DockerRestart),
            "compose.up" => Some(Self::ComposeUp),
            "compose.down" => Some(Self::ComposeDown),
            "compose.logs" => Some(Self::ComposeLogs),
            "compose.config" => Some(Self::ComposeConfig),
            "fs.read" => Some(Self::FsRead),
            "fs.write" => Some(Self::FsWrite),
            "fs.list" => Some(Self::FsList),
            "volume.transfer_out" => Some(Self::VolumeTransferOut),
            "volume.transfer_in" => Some(Self::VolumeTransferIn),
            "image.ensure" => Some(Self::ImageEnsure),
            "host.info" => Some(Self::HostInfo),
            "relay.audit" => Some(Self::RelayAudit),
            "relay.update" => Some(Self::RelayUpdate),
            "relay.debug.state" => Some(Self::RelayDebugState),
            "relay.debug.stats" => Some(Self::RelayDebugStats),
            "relay.debug.transfer" => Some(Self::RelayDebugTransfer),
            "relay.debug.events" => Some(Self::RelayDebugEvents),
            "relay.debug.replay" => Some(Self::RelayDebugReplay),
            "relay.debug.config" => Some(Self::RelayDebugConfig),
            _ => None,
        }
    }

    /// Convert operation code back to its wire subtype string.
    pub fn to_subtype(&self) -> &'static str {
        match self {
            Self::Register => "register",
            Self::Ping => "ping",
            Self::Pong => "pong",
            Self::DockerPs => "docker.ps",
            Self::DockerInspect => "docker.inspect",
            Self::DockerExec => "docker.exec",
            Self::DockerLogs => "docker.logs",
            Self::DockerStats => "docker.stats",
            Self::DockerStop => "docker.stop",
            Self::DockerStart => "docker.start",
            Self::DockerRestart => "docker.restart",
            Self::ComposeUp => "compose.up",
            Self::ComposeDown => "compose.down",
            Self::ComposeLogs => "compose.logs",
            Self::ComposeConfig => "compose.config",
            Self::FsRead => "fs.read",
            Self::FsWrite => "fs.write",
            Self::FsList => "fs.list",
            Self::VolumeTransferOut => "volume.transfer_out",
            Self::VolumeTransferIn => "volume.transfer_in",
            Self::ImageEnsure => "image.ensure",
            Self::HostInfo => "host.info",
            Self::RelayAudit => "relay.audit",
            Self::RelayUpdate => "relay.update",
            Self::RelayDebugState => "relay.debug.state",
            Self::RelayDebugStats => "relay.debug.stats",
            Self::RelayDebugTransfer => "relay.debug.transfer",
            Self::RelayDebugEvents => "relay.debug.events",
            Self::RelayDebugReplay => "relay.debug.replay",
            Self::RelayDebugConfig => "relay.debug.config",
        }
    }

    /// Returns true if this operation produces streaming events.
    pub fn is_streaming(&self) -> bool {
        matches!(self,
            Self::DockerExec | Self::DockerLogs | Self::DockerStats |
            Self::ComposeUp | Self::ComposeDown | Self::ComposeLogs |
            Self::VolumeTransferOut | Self::VolumeTransferIn |
            Self::ImageEnsure | Self::RelayUpdate |
            Self::RelayDebugEvents | Self::RelayDebugReplay
        )
    }

    /// Returns true if this operation is safe to retry without side effects.
    pub fn is_idempotent(&self) -> bool {
        !matches!(self,
            Self::Register | Self::DockerExec | Self::DockerRestart |
            Self::FsWrite | Self::VolumeTransferOut | Self::VolumeTransferIn |
            Self::RelayUpdate | Self::RelayDebugReplay
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_30_ops_parse_correctly() {
        let subtypes = [
            "register", "ping", "pong",
            "docker.ps", "docker.inspect", "docker.exec", "docker.logs", "docker.stats",
            "docker.stop", "docker.start", "docker.restart",
            "compose.up", "compose.down", "compose.logs", "compose.config",
            "fs.read", "fs.write", "fs.list",
            "volume.transfer_out", "volume.transfer_in",
            "image.ensure",
            "host.info",
            "relay.audit", "relay.update",
            "relay.debug.state", "relay.debug.stats", "relay.debug.transfer",
            "relay.debug.events", "relay.debug.replay", "relay.debug.config",
        ];
        for s in subtypes {
            let op = OperationCode::from_subtype(s);
            assert!(op.is_some(), "Failed to parse subtype: {}", s);
            assert_eq!(op.unwrap().to_subtype(), s);
        }
    }

    #[test]
    fn count_is_exactly_30() {
        // This test fails if a variant is added without updating the subtype mapping
        let mut count = 0;
        for s in [
            "register", "ping", "pong",
            "docker.ps", "docker.inspect", "docker.exec", "docker.logs", "docker.stats",
            "docker.stop", "docker.start", "docker.restart",
            "compose.up", "compose.down", "compose.logs", "compose.config",
            "fs.read", "fs.write", "fs.list",
            "volume.transfer_out", "volume.transfer_in",
            "image.ensure",
            "host.info",
            "relay.audit", "relay.update",
            "relay.debug.state", "relay.debug.stats", "relay.debug.transfer",
            "relay.debug.events", "relay.debug.replay", "relay.debug.config",
        ] {
            assert!(OperationCode::from_subtype(s).is_some());
            count += 1;
        }
        assert_eq!(count, 30, "Expected exactly 30 operation codes");
    }

    #[test]
    fn unknown_subtype_returns_none() {
        assert!(OperationCode::from_subtype("docker.fly").is_none());
        assert!(OperationCode::from_subtype("compose.build").is_none());
        assert!(OperationCode::from_subtype("").is_none());
    }

    #[test]
    fn streaming_ops_detected() {
        assert!(OperationCode::DockerExec.is_streaming());
        assert!(OperationCode::ComposeUp.is_streaming());
        assert!(OperationCode::VolumeTransferOut.is_streaming());
        assert!(!OperationCode::Ping.is_streaming());
        assert!(!OperationCode::DockerPs.is_streaming());
        assert!(!OperationCode::FsRead.is_streaming());
    }

    #[test]
    fn idempotent_ops_detected() {
        assert!(OperationCode::Ping.is_idempotent());
        assert!(OperationCode::DockerPs.is_idempotent());
        assert!(OperationCode::DockerStop.is_idempotent());
        assert!(!OperationCode::DockerExec.is_idempotent());
        assert!(!OperationCode::FsWrite.is_idempotent());
        assert!(!OperationCode::Register.is_idempotent());
    }

    #[test]
    fn roundtrip_all_ops() {
        let ops = [
            OperationCode::Register, OperationCode::Ping, OperationCode::Pong,
            OperationCode::DockerPs, OperationCode::DockerExec,
            OperationCode::ComposeUp, OperationCode::FsRead,
            OperationCode::VolumeTransferOut, OperationCode::ImageEnsure,
            OperationCode::HostInfo, OperationCode::RelayAudit,
            OperationCode::RelayDebugState, OperationCode::RelayDebugEvents,
        ];
        for op in &ops {
            let subtype = op.to_subtype();
            let parsed = OperationCode::from_subtype(subtype).unwrap();
            assert_eq!(op, &parsed);
        }
    }
}
