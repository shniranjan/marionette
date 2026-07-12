//! Message validation as defined in specs/message-envelope.md.
//!
//! Every incoming message is validated in order:
//! 1. Size check (≤ 1 MB)
//! 2. Schema check (required fields present)
//! 3. Operation check (known subtype)

use crate::{ErrorCode, ErrorPayload, Message, MAX_MESSAGE_SIZE};

/// Validate a message against the protocol specification.
/// Returns Ok(()) if the message is valid, or an ErrorPayload describing the failure.
pub fn validate_message(msg: &Message) -> Result<(), ErrorPayload> {
    // 1. Size check — serialize and measure
    let json = serde_json::to_string(msg).map_err(|e| {
        ErrorPayload::new(ErrorCode::InvalidMessage, format!("Serialization error: {}", e))
    })?;

    if json.len() > MAX_MESSAGE_SIZE {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidMessage,
            format!("Message size {} bytes exceeds maximum {} bytes", json.len(), MAX_MESSAGE_SIZE),
        ));
    }

    // 2. Schema check — required fields
    if msg.id.is_empty() {
        return Err(ErrorPayload::new(ErrorCode::InvalidMessage, "Missing required field: id"));
    }
    if msg.subtype.is_empty() {
        return Err(ErrorPayload::new(ErrorCode::InvalidMessage, "Missing required field: subtype"));
    }

    // 3. Operation check — known subtype (allow "error" for responses)
    if msg.subtype != "error" && crate::operations::OperationCode::from_subtype(&msg.subtype).is_none() {
        return Err(ErrorPayload::new(
            ErrorCode::InvalidMessage,
            format!("Unknown operation subtype: '{}'", msg.subtype),
        ));
    }

    Ok(())
}

/// Convenience: validate and return the parsed OperationCode.
pub fn validate_and_parse(msg: &Message) -> Result<crate::OperationCode, ErrorPayload> {
    validate_message(msg)?;
    crate::OperationCode::from_subtype(&msg.subtype)
        .ok_or_else(|| ErrorPayload::new(ErrorCode::InvalidMessage, "Unknown operation"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn valid_request_passes() {
        let msg = Message::new_request("1", "ping", json!({}));
        assert!(validate_message(&msg).is_ok());
    }

    #[test]
    fn valid_response_passes() {
        let msg = Message::new_response("1", "pong", json!({"uptime_secs": 3600}));
        assert!(validate_message(&msg).is_ok());
    }

    #[test]
    fn error_response_passes() {
        let msg = Message::new_error("1", "AUTH.INVALID", "token expired", None);
        assert!(validate_message(&msg).is_ok());
    }

    #[test]
    fn empty_id_fails() {
        let msg = Message::new_request("", "ping", json!({}));
        let err = validate_message(&msg).unwrap_err();
        assert_eq!(err.error_code, ErrorCode::InvalidMessage);
        assert!(err.message.contains("id"));
    }

    #[test]
    fn empty_subtype_fails() {
        let msg = Message::new_request("1", "", json!({}));
        let err = validate_message(&msg).unwrap_err();
        assert_eq!(err.error_code, ErrorCode::InvalidMessage);
    }

    #[test]
    fn unknown_subtype_fails() {
        let msg = Message::new_request("1", "nonexistent.op", json!({}));
        let err = validate_message(&msg).unwrap_err();
        assert_eq!(err.error_code, ErrorCode::InvalidMessage);
        assert!(err.message.contains("Unknown"));
    }

    #[test]
    fn oversized_message_fails() {
        let big_payload = "x".repeat(MAX_MESSAGE_SIZE);
        let msg = Message::new_request("1", "ping", json!({"data": big_payload}));
        let err = validate_message(&msg).unwrap_err();
        assert_eq!(err.error_code, ErrorCode::InvalidMessage);
        assert!(err.message.contains("exceeds maximum"));
    }

    #[test]
    fn validate_and_parse_returns_op_code() {
        let msg = Message::new_request("1", "docker.ps", json!({"all": true}));
        let op = validate_and_parse(&msg).unwrap();
        assert_eq!(op, crate::OperationCode::DockerPs);
    }
}
