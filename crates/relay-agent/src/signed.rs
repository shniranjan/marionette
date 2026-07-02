use relay_protocol::Message;
use serde::{Deserialize, Serialize};

/// Wire wrapper that pairs a Message with an optional HMAC signature.
///
/// Before authentication (during registration), messages have no signature.
/// After authentication, every message in both directions carries a
/// hex-encoded HMAC-SHA256 signature over the canonical JSON of the inner Message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedMessage {
    pub message: Message,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

impl SignedMessage {
    pub fn new(message: Message, signature: Option<String>) -> Self {
        Self { message, signature }
    }

    pub fn unsigned(message: Message) -> Self {
        Self {
            message,
            signature: None,
        }
    }
}
