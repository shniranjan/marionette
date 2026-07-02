use relay_agent::auth::AuthState;
use relay_agent::signed::SignedMessage;
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

#[test]
fn signed_message_includes_signature_field() {
    let msg = Message::new_request("test-1", "ping", json!({}));
    let signed = SignedMessage::new(msg, Some("abc123".to_string()));
    let json = serde_json::to_string(&signed).unwrap();
    assert!(json.contains("signature"));
    assert!(json.contains("abc123"));
}

#[test]
fn hmac_sign_verify_roundtrip() {
    let state = AuthState {
        session_key: Some(vec![0x42u8; 32]),
        session_id: None,
    };
    let msg = b"hello world";
    let sig = state.sign(msg).unwrap();
    assert_eq!(sig.len(), 32);
    assert!(state.verify(msg, &sig));
}

#[test]
fn hmac_tampered_message_rejected() {
    let state = AuthState {
        session_key: Some(vec![0x42u8; 32]),
        session_id: None,
    };
    let sig = state.sign(b"original").unwrap();
    assert!(!state.verify(b"tampered", &sig));
}

#[test]
fn hmac_tampered_signature_rejected() {
    let state = AuthState {
        session_key: Some(vec![0x42u8; 32]),
        session_id: None,
    };
    let msg = b"test message";
    let mut sig = state.sign(msg).unwrap();
    sig[0] ^= 1;
    assert!(!state.verify(msg, &sig));
}

#[test]
fn unauthenticated_state_cannot_sign() {
    let state = AuthState::new();
    assert!(state.sign(b"msg").is_none());
    assert!(!state.is_authenticated());
    // Verify also returns false when unauthenticated
    assert!(!state.verify(b"msg", &[0u8; 32]));
}
