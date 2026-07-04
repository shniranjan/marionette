use marionette_core::relay::session::SessionManager;

#[test]
fn create_session_returns_32_byte_key() {
    let mut mgr = SessionManager::new();
    let key = mgr.create_session("session-1".into(), "endpoint-1".into());
    assert_eq!(key.len(), 32);
}

#[test]
fn sign_and_verify_roundtrip() {
    let mut mgr = SessionManager::new();
    mgr.create_session("session-1".into(), "endpoint-1".into());
    let msg = b"hello world";
    let sig = mgr.sign("session-1", msg).unwrap();
    assert_eq!(sig.len(), 32);
    assert!(mgr.verify("session-1", msg, &sig));
}

#[test]
fn tampered_message_rejected() {
    let mut mgr = SessionManager::new();
    mgr.create_session("session-1".into(), "endpoint-1".into());
    let sig = mgr.sign("session-1", b"original").unwrap();
    assert!(!mgr.verify("session-1", b"tampered", &sig));
}

#[test]
fn wrong_session_id_rejected() {
    let mut mgr = SessionManager::new();
    mgr.create_session("session-1".into(), "endpoint-1".into());
    let sig = mgr.sign("session-1", b"hello").unwrap();
    // Verify with a different session_id should fail
    assert!(!mgr.verify("session-2", b"hello", &sig));
}

#[test]
fn nonce_first_use_ok_replay_rejected() {
    let mut mgr = SessionManager::new();
    mgr.create_session("session-1".into(), "endpoint-1".into());
    // First use of a nonce succeeds
    assert!(mgr.check_nonce("session-1", "nonce-1"));
    // Replay of the same nonce fails
    assert!(!mgr.check_nonce("session-1", "nonce-1"));
    // Different nonce still succeeds
    assert!(mgr.check_nonce("session-1", "nonce-2"));
}

#[test]
fn nonce_rotation_when_over_10000() {
    let mut mgr = SessionManager::new();
    mgr.create_session("session-1".into(), "endpoint-1".into());
    // Fill up to 10001 entries (0..=10000) so len > 10_000 after the loop
    for i in 0..=10000 {
        let ok = mgr.check_nonce("session-1", &format!("nonce-{}", i));
        assert!(ok, "nonce-{} should be accepted", i);
    }
    // At this point the set has 10001 entries. Insert one more NEW nonce
    // to trigger rotation (len > 10_000 check clears the set).
    assert!(mgr.check_nonce("session-1", "nonce-fresh"));
    // After rotation, previously seen nonces should be accepted again
    assert!(mgr.check_nonce("session-1", "nonce-0"));
}
