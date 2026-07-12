//! Signed message wrapper for authenticated relay communication.
//!
//! Wraps a `relay_protocol::Message` with an optional hex-encoded HMAC-SHA256
//! signature. Before authentication (during registration), messages carry no
//! signature. After session establishment, every message in both directions
//! carries a signature over the canonical JSON of the inner Message.

use crate::auth;
use relay_protocol::Message;
use serde::{Deserialize, Serialize};

/// Wire wrapper that pairs a Message with an optional HMAC signature.
///
/// The signature (when present) is a hex-encoded HMAC-SHA256 tag computed over
/// the canonical JSON representation of the inner `message` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedMessage {
    pub message: Message,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

impl SignedMessage {
    /// Create a new `SignedMessage` with no signature (unsigned).
    pub fn new(message: Message) -> Self {
        Self {
            message,
            signature: None,
        }
    }

    /// Attach a hex-encoded HMAC-SHA256 signature computed over the canonical
    /// JSON of the inner message using `key`.
    ///
    /// Overwrites any existing signature.
    pub fn sign(&mut self, key: &[u8]) {
        let canonical = serde_json::to_string(&self.message)
            .expect("Message serialization is infallible");
        let raw = auth::sign_message(key, canonical.as_bytes());
        self.signature = Some(hex::encode(raw));
    }

    /// Verify the stored signature against the inner message using `key`.
    ///
    /// Returns `false` if this message is unsigned or the signature does not
    /// match. Uses constant-time comparison internally.
    pub fn verify(&self, key: &[u8]) -> bool {
        let sig = match &self.signature {
            Some(s) => s,
            None => return false,
        };
        let raw = match hex::decode(sig) {
            Ok(v) => v,
            Err(_) => return false,
        };
        let canonical = serde_json::to_string(&self.message)
            .expect("Message serialization is infallible");
        auth::verify_signature(key, canonical.as_bytes(), &raw)
    }

    /// Serialize this signed message to a JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("SignedMessage serialization is infallible")
    }

    /// Deserialize a signed message from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use relay_protocol::MessageType;
    use serde_json::json;

    fn make_test_message() -> Message {
        Message {
            id: "req-1".into(),
            msg_type: MessageType::Request,
            subtype: "ping".into(),
            payload: json!({"test": true}),
            timestamp: None,
            seq: None,
        }
    }

    #[test]
    fn new_message_is_unsigned() {
        let sm = SignedMessage::new(make_test_message());
        assert!(sm.signature.is_none());
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let key = vec![0x42u8; 32];
        let mut sm = SignedMessage::new(make_test_message());
        sm.sign(&key);
        assert!(sm.signature.is_some());
        assert!(sm.verify(&key));
    }

    #[test]
    fn unsigned_message_fails_verify() {
        let key = vec![0x42u8; 32];
        let sm = SignedMessage::new(make_test_message());
        assert!(!sm.verify(&key));
    }

    #[test]
    fn wrong_key_fails_verify() {
        let key_a = vec![0x42u8; 32];
        let key_b = vec![0x99u8; 32];
        let mut sm = SignedMessage::new(make_test_message());
        sm.sign(&key_a);
        assert!(!sm.verify(&key_b));
    }

    #[test]
    fn tampered_message_fails_verify() {
        let key = vec![0x42u8; 32];
        let mut sm = SignedMessage::new(make_test_message());
        sm.sign(&key);
        // Tamper with the payload
        sm.message.payload = json!({"tampered": true});
        assert!(!sm.verify(&key));
    }

    #[test]
    fn json_roundtrip_preserves_signature() {
        let key = vec![0x42u8; 32];
        let mut sm = SignedMessage::new(make_test_message());
        sm.sign(&key);

        let json = sm.to_json();
        let parsed = SignedMessage::from_json(&json).unwrap();

        assert_eq!(parsed.message.id, sm.message.id);
        assert_eq!(parsed.signature, sm.signature);
        assert!(parsed.verify(&key));
    }

    #[test]
    fn json_roundtrip_unsigned() {
        let sm = SignedMessage::new(make_test_message());
        let json = sm.to_json();
        let parsed = SignedMessage::from_json(&json).unwrap();

        assert!(parsed.signature.is_none());
        assert_eq!(parsed.message.subtype, "ping");
    }
}
