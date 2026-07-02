use marionette_core::relay::signed::SignedMessage;
use relay_protocol::Message;
use serde_json::json;

#[test]
fn signed_message_serialize_deserialize() {
    let msg = Message::new_request("test-1", "ping", json!({}));
    let signed = SignedMessage::new(msg.clone(), Some("abc123".to_string()));
    let json = serde_json::to_string(&signed).unwrap();
    let parsed: SignedMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.message.id, "test-1");
    assert_eq!(parsed.signature, Some("abc123".to_string()));
}

#[test]
fn unsigned_message_omits_signature_field() {
    let msg = Message::new_request("test-1", "ping", json!({}));
    let unsigned = SignedMessage::unsigned(msg);
    let json = serde_json::to_string(&unsigned).unwrap();
    // The signature field should be absent from JSON when None
    assert!(!json.contains("signature"));
}
