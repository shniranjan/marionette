use relay_protocol::Message;
use serde::{Deserialize, Serialize};

/// Signed wire message wrapper.
///
/// Every authenticated message on the relay WebSocket is wrapped in this
/// envelope with an HMAC-SHA256 signature over the canonical JSON of the
/// inner `message`.  Unsigned messages (register, error responses before
/// registration) omit the `signature` field.
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
