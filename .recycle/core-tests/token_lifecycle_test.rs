use marionette_core::db::Database;
use marionette_core::relay::auth::{
    ensure_schema, generate_registration_token, store_registration_token,
    validate_registration_token,
};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::sync::Mutex;

fn setup_db() -> Database {
    let conn = Connection::open_in_memory().unwrap();
    ensure_schema(&conn).unwrap();
    Database {
        conn: Mutex::new(conn),
    }
}

#[test]
fn generate_token_produces_valid_hex() {
    let token = generate_registration_token();
    // Plaintext should be 64 hex chars (32 bytes)
    assert_eq!(token.plaintext.len(), 64);
    // Hash should also be 64 hex chars
    assert_eq!(token.hash.len(), 64);
    // Both should be valid hex
    hex::decode(&token.plaintext).expect("plaintext should be valid hex");
    hex::decode(&token.hash).expect("hash should be valid hex");
    // Hash should be SHA-256 of plaintext
    let mut h = Sha256::new();
    h.update(token.plaintext.as_bytes());
    let computed = hex::encode(h.finalize());
    assert_eq!(token.hash, computed);
}

#[test]
fn store_and_validate_token_succeeds() {
    let db = setup_db();
    let token = generate_registration_token();
    store_registration_token(&db, "endpoint-1", &token).unwrap();
    let result = validate_registration_token(&db, &token.plaintext);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "endpoint-1");
}

#[test]
fn validate_wrong_token_fails() {
    let db = setup_db();
    let token = generate_registration_token();
    store_registration_token(&db, "endpoint-1", &token).unwrap();
    let result = validate_registration_token(
        &db,
        "wrong-token-wrong-token-wrong-token-00000000000000000000000000000000",
    );
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Invalid token");
}

#[test]
fn validate_already_consumed_token_fails() {
    let db = setup_db();
    let token = generate_registration_token();
    store_registration_token(&db, "endpoint-1", &token).unwrap();
    // First validation succeeds
    assert!(validate_registration_token(&db, &token.plaintext).is_ok());
    // Second validation fails (already consumed)
    let result = validate_registration_token(&db, &token.plaintext);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Token already consumed");
}

#[test]
fn token_marked_consumed_after_validation() {
    let db = setup_db();
    let token = generate_registration_token();
    store_registration_token(&db, "endpoint-1", &token).unwrap();
    // Validate once — should succeed
    assert!(validate_registration_token(&db, &token.plaintext).is_ok());
    // Verify consumed flag is set in DB
    let conn = db.conn.lock().unwrap();
    let consumed: bool = conn
        .query_row(
            "SELECT consumed FROM registration_tokens WHERE token_hash = ?1",
            rusqlite::params![token.hash],
            |row| row.get(0),
        )
        .unwrap();
    assert!(consumed);
}
