//! Wire message envelope as defined in specs/message-envelope.md.
//!
//! Every message on the wire is a single JSON object with a type discriminator.
//! The envelope is transport-agnostic — designed for WebSocket but works over
//! any bidirectional channel.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Maximum single message size: 1 MB (1,048,576 bytes).
/// Larger payloads must use chunked streaming events or volume transfer operations.
pub const MAX_MESSAGE_SIZE: usize = 1_048_576;

/// Message class discriminator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    /// Marionette → Relay: a command to execute
    Request,
    /// Relay → Marionette: final result of a command
    Response,
    /// Relay → Marionette: streaming data associated with a pending request
    Event,
}

/// Complete wire message envelope.
///
/// All communication between Marionette and relay agents uses this format.
/// The `id` field links requests to their responses and streaming events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message identifier. UUID v4 for new requests.
    /// Responses and events carry the id of their originating request.
    pub id: String,

    /// Message class: request, response, or event.
    #[serde(rename = "type")]
    pub msg_type: MessageType,

    /// Operation code for requests, matching op code or "error" for responses,
    /// streaming event subtype for events.
    pub subtype: String,

    /// Operation-specific payload. Empty object for parameterless operations.
    pub payload: Value,

    /// ISO 8601 UTC timestamp. Used for ordering, audit correlation, replay detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,

    /// Monotonically increasing sequence number per session.
    /// When present, receiver MUST process in order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
}

impl Message {
    /// Create a new request message.
    pub fn new_request(id: impl Into<String>, subtype: impl Into<String>, payload: Value) -> Self {
        Self {
            id: id.into(),
            msg_type: MessageType::Request,
            subtype: subtype.into(),
            payload,
            timestamp: None,
            seq: None,
        }
    }

    /// Create a new response message (success or error).
    pub fn new_response(id: impl Into<String>, subtype: impl Into<String>, payload: Value) -> Self {
        Self {
            id: id.into(),
            msg_type: MessageType::Response,
            subtype: subtype.into(),
            payload,
            timestamp: None,
            seq: None,
        }
    }

    /// Create a new streaming event message.
    pub fn new_event(id: impl Into<String>, subtype: impl Into<String>, payload: Value) -> Self {
        Self {
            id: id.into(),
            msg_type: MessageType::Event,
            subtype: subtype.into(),
            payload,
            timestamp: None,
            seq: None,
        }
    }

    /// Create an error response message.
    pub fn new_error(id: impl Into<String>, error_code: &str, message: impl Into<String>, details: Option<Value>) -> Self {
        let payload = serde_json::json!({
            "error_code": error_code,
            "message": message.into(),
            "details": details,
        });
        Self::new_response(id, "error", payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn roundtrip_request() {
        let msg = Message::new_request("test-1", "ping", json!({}));
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.msg_type, MessageType::Request);
        assert_eq!(parsed.subtype, "ping");
        assert_eq!(parsed.id, "test-1");
    }

    #[test]
    fn roundtrip_response() {
        let payload = json!({"uptime_secs": 3600, "docker_version": "26.1.3", "arch": "aarch64", "os": "linux"});
        let msg = Message::new_response("test-1", "pong", payload);
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.msg_type, MessageType::Response);
        assert_eq!(parsed.subtype, "pong");
    }

    #[test]
    fn roundtrip_event() {
        let payload = json!({"bytes_sent": 524288000, "total_bytes": 1073741824, "percent": 48.8});
        let msg = Message::new_event("xfer-1", "volume.transfer.progress", payload);
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.msg_type, MessageType::Event);
        assert_eq!(parsed.subtype, "volume.transfer.progress");
    }

    #[test]
    fn error_message() {
        let msg = Message::new_error("test-1", "AUTH.INVALID", "Token has expired", None);
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.msg_type, MessageType::Response);
        assert_eq!(parsed.subtype, "error");
    }

    #[test]
    fn seq_preserved() {
        let mut msg = Message::new_request("1", "ping", json!({}));
        msg.seq = Some(42);
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.seq, Some(42));
    }

    #[test]
    fn timestamp_preserved() {
        let mut msg = Message::new_request("1", "ping", json!({}));
        msg.timestamp = Some("2026-06-30T10:00:00Z".into());
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.timestamp.as_deref(), Some("2026-06-30T10:00:00Z"));
    }
}
